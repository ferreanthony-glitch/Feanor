//! Pistes son dans l'interface : import, lecture calée sur la carte son, son au défilement,
//! pistes de la bande de montage et formes d'onde.

use std::path::Path;
use std::sync::Arc;

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Sense, Stroke, StrokeKind};

use crate::app::App;
use crate::audio::{self, AudioEngine, Pcm};
use crate::document::{AudioTrack, Document, SoundClip};
use crate::history::Edit;
use crate::project::SOUND_EXTS;
use crate::storyboard::{duration_label, timecode, MontageDrag};
use crate::theme;

/// Hauteur d'une piste son dans la bande de montage.
pub(crate) const TRACK_H: f32 = 42.0;
/// Largeur de la colonne des noms de pistes.
pub(crate) const HEADERS_W: f32 = 104.0;
/// Espace entre les panneaux et la première piste son.
pub(crate) const LANE_GAP: f32 = 4.0;
/// Ce qu'on dit quand on touche à une piste verrouillée (13e).
const TRACK_LOCKED: &str = "Cette piste son est verrouillée : déverrouille-la pour la changer.";

/// Partie d'un extrait saisie à la souris.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum ClipEdge {
    Move,
    Left,
    Right,
}

/// Extrait de son glissé dans la bande de montage.
pub(crate) struct ClipDrag {
    pub id: u64,
    pub edge: ClipEdge,
    pub origin: Pos2,
    /// Pistes au début du geste : chaque mouvement repart d'elles ; un geste abandonné les rétablit.
    pub before: Vec<AudioTrack>,
    pub snap: Edit,
}

/// Ce qu'on a cliqué du bouton droit dans la bande de montage.
#[derive(Clone, Copy)]
pub(crate) enum MenuTarget {
    Panel(usize),
    Lane { track: usize, clip: Option<u64>, frame: usize },
}

impl App {
    // ------------------------------------------------------------------ sortie audio et lecture

    /// Sortie audio, ouverte à la première utilisation. `None` : pas de sortie utilisable.
    fn audio_engine(&mut self) -> Option<&AudioEngine> {
        if self.audio.as_ref().is_some_and(|e| !e.ok()) {
            // Sortie en panne (casque débranché…) : on rouvre la sortie par défaut.
            self.audio = None;
            self.audio_tried = false;
        }
        if self.audio.is_none() && !self.audio_tried {
            self.audio_tried = true;
            match AudioEngine::start() {
                Ok(e) => self.audio = Some(e),
                Err(e) => self.status = format!("Pas de sortie audio ({e}) : lecture sans son."),
            }
        }
        self.audio.as_ref()
    }

    fn samples_per_frame(&self) -> f64 {
        audio::samples_per_frame(self.doc.fps)
    }

    /// Image de départ et plage jouée (en images du montage) : tout le montage, ou le panneau.
    fn play_range(&self) -> (usize, (usize, usize)) {
        if self.play_montage {
            (self.global_frame(), (0, self.doc.total_frames()))
        } else {
            // Entre les points d'entrée et de sortie du plan (13d).
            let s = self.doc.panel_start(self.cur_panel);
            let (a, b) = self.plan_play_range();
            (s + self.cur_frame.clamp(a, b - 1), (s + a, s + b))
        }
    }

    /// Lance le son avec l'image (au début de la lecture). Sans son, l'image suit l'horloge de
    /// l'ordinateur comme avant.
    pub(crate) fn start_audio_playback(&mut self) {
        self.audio_clock = false;
        if !self.sound_on {
            return;
        }
        let clips = audio::mix_clips(&self.doc);
        if clips.is_empty() {
            return;
        }
        // Une sortie absente au lancement précédent a pu être branchée depuis.
        self.audio_tried = false;
        let spf = self.samples_per_frame();
        let (from, (a, b)) = self.play_range();
        let looping = self.looping;
        let at = |f: usize| (f as f64 * spf) as u64;
        if let Some(engine) = self.audio_engine() {
            engine.play(clips, at(from), (at(a), at(b)), looping);
            self.audio_clock = true;
        }
    }

    pub(crate) fn stop_audio(&mut self) {
        if std::mem::take(&mut self.audio_clock) {
            if let Some(engine) = &self.audio {
                engine.stop();
            }
        }
    }

    /// Pendant la lecture avec son : l'image suit la position du son. Renvoie faux s'il faut
    /// suivre l'horloge de l'ordinateur (pas de son, ou sortie tombée en panne).
    pub(crate) fn tick_audio_clock(&mut self) -> bool {
        if !self.audio_clock {
            return false;
        }
        // Suivre les changements faits pendant la lecture : extrait déplacé, piste muette, son coupé.
        let clips = if self.sound_on {
            audio::mix_clips(&self.doc)
        } else {
            Vec::new()
        };
        let spf = self.samples_per_frame();
        let Some(engine) = self.audio.as_ref().filter(|e| e.ok()) else {
            self.audio_clock = false;
            self.status = "Sortie audio interrompue : la lecture continue sans son.".into();
            return false;
        };
        engine.update(clips, self.looping);
        let pos = engine.position();
        let still_playing = engine.is_playing();
        let g = (pos / spf).floor().max(0.0) as usize;
        if self.play_montage {
            let (p, f) = self.doc.locate(g.min(self.doc.total_frames() - 1));
            if p != self.cur_panel || f != self.cur_frame {
                self.set_panel(p);
                self.cur_frame = f;
                self.follow_frame = true;
            }
        } else {
            let f = g.saturating_sub(self.doc.panel_start(self.cur_panel)).min(self.frame_count() - 1);
            if f != self.cur_frame {
                self.cur_frame = f;
                self.follow_frame = true;
            }
        }
        if !still_playing {
            self.playing = false;
            self.audio_clock = false;
        }
        true
    }

    /// Son au défilement : une image de son chaque fois que l'image courante change hors lecture.
    pub(crate) fn scrub_audio(&mut self) {
        let g = self.global_frame();
        if self.heard_frame.replace(g) == Some(g) || self.playing || !self.sound_scrub || !self.sound_on {
            return;
        }
        let clips = audio::mix_clips(&self.doc);
        if clips.is_empty() {
            return;
        }
        let spf = self.samples_per_frame();
        if let Some(engine) = self.audio_engine() {
            engine.scrub(clips, (g as f64 * spf) as u64, spf.ceil() as u64);
        }
    }

    // ------------------------------------------------------------------ opérations

    /// Opération sur les pistes son, avec annulation. `op` renvoie faux si rien n'a changé.
    fn audio_op(&mut self, op: impl FnOnce(&mut Document) -> bool) -> bool {
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        if !op(&mut self.doc) {
            return false;
        }
        self.commit_structure(snap);
        true
    }

    /// Boîte de choix d'un fichier son, puis import (sur la piste `track` si elle est libre,
    /// à l'image `at` du montage ou, à défaut, à l'image courante).
    pub(crate) fn import_sound(&mut self, track: Option<usize>, at: Option<usize>) {
        self.end_stroke();
        self.playing = false;
        let Some(path) = rfd::FileDialog::new()
            .set_title("Importer un son")
            .add_filter("Son (WAV, MP3, FLAC, OGG)", &SOUND_EXTS)
            .pick_file()
        else {
            return;
        };
        self.import_sound_path(&path, track, at);
    }

    pub(crate) fn import_sound_path(&mut self, path: &Path, track: Option<usize>, at: Option<usize>) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "son".into());
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !SOUND_EXTS.contains(&ext.as_str()) {
            self.status = format!("« {name} » : format de son non géré (WAV, MP3, FLAC ou OGG).");
            return;
        }
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.status = format!("Impossible de lire « {name} » : {e}");
                return;
            }
        };
        let pcm = match audio::decode(&bytes, &ext) {
            Ok(p) => p,
            Err(e) => {
                self.status = format!("Impossible d'importer « {name} » : {e}");
                return;
            }
        };
        let start = at.unwrap_or_else(|| self.global_frame());
        let mut clip = None;
        self.audio_op(|d| {
            let sound = d.add_sound(name.clone(), ext, Arc::new(bytes), Arc::new(pcm));
            clip = d.place_sound(sound, start, track);
            clip.is_some()
        });
        let Some(clip) = clip else { return };
        self.sel_clip = Some(clip);
        let fps = self.doc.fps;
        let len = self.doc.clip(clip).map_or(0, |c| c.length);
        let mut msg = format!(
            "Son importé : « {name} » ({}), à {}.",
            duration_label(len, fps),
            timecode(start, fps)
        );
        if start + len > self.doc.total_frames() {
            msg.push_str(" Il dépasse la fin des plans : la lecture s'arrête à la fin des plans.");
        }
        self.status = msg;
    }

    /// Fichiers son lâchés sur la fenêtre : importés à l'image courante ; une vidéo devient un
    /// calque vidéo (13k) ; un projet Storyboard Pro ouvre la fenêtre d'import (14a).
    pub(crate) fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let paths: Vec<_> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        for path in paths {
            if !self.import_dropped_sboard(&path) && !self.import_dropped_video(&path) {
                self.import_sound_path(&path, None, None);
            }
        }
    }

    pub(crate) fn new_audio_track(&mut self) {
        if self.audio_op(|d| {
            d.add_audio_track();
            true
        }) {
            self.status = format!("Nouvelle piste : {}.", self.doc.audio_tracks.last().map_or("", |t| &t.name));
        }
    }

    pub(crate) fn delete_audio_track(&mut self, track: usize) {
        if self.doc.audio_tracks.get(track).is_some_and(|t| t.locked) {
            self.status = TRACK_LOCKED.into();
            return;
        }
        self.audio_op(|d| {
            if track < d.audio_tracks.len() {
                d.audio_tracks.remove(track);
                true
            } else {
                false
            }
        });
    }

    /// Copie de la piste `track` et de ses extraits, juste en dessous (13e).
    pub(crate) fn duplicate_track(&mut self, track: usize) {
        if self.audio_op(|d| d.duplicate_audio_track(track).is_some()) {
            self.status = "Piste son dupliquée.".into();
        }
    }

    /// Ctrl + glisser l'onde dans la timeline (13e) : l'extrait de la piste `track` sous la
    /// souris se décale ; `lanes_top` suit la souris pour que la piste ne change pas.
    pub(crate) fn drag_timeline_sound(&mut self, track: usize, p: Pos2, ppf: f32) {
        let lanes_top = p.y - (track as f32 + 0.5) * TRACK_H;
        self.update_clip_drag(p, ppf, lanes_top);
    }

    /// Fin d'un glisser d'extrait commencé dans la timeline.
    pub(crate) fn end_timeline_sound_drag(&mut self) {
        if let Some(MontageDrag::Clip(d)) = self.montage_drag.take() {
            self.finish_clip_drag(d);
        }
    }

    pub(crate) fn delete_clip(&mut self, id: u64) {
        if self.doc.clip_locked(id) {
            self.status = TRACK_LOCKED.into();
            return;
        }
        if self.audio_op(|d| d.remove_clip(id)) && self.sel_clip == Some(id) {
            self.sel_clip = None;
        }
    }

    /// Coupe à l'image `at` du montage l'extrait `only`, ou à défaut tous les extraits qui passent
    /// par cette image.
    pub(crate) fn split_sound(&mut self, at: usize, only: Option<u64>) {
        let ids: Vec<u64> = match only {
            Some(id) => vec![id],
            None => self
                .doc
                .audio_tracks
                .iter()
                // Une piste verrouillée (13e) ne se coupe pas.
                .filter(|t| !t.locked)
                .flat_map(|t| t.clips.iter())
                .filter(|c| c.start < at && at < c.end())
                .map(|c| c.id)
                .collect(),
        };
        let ids: Vec<u64> = ids.into_iter().filter(|&id| !self.doc.clip_locked(id)).collect();
        let done = self.audio_op(|d| ids.iter().fold(false, |any, &id| d.split_clip(id, at).is_some() || any));
        self.status = if done {
            format!("Son coupé à {}.", timecode(at, self.doc.fps))
        } else {
            "Aucun son à couper à cette image.".into()
        };
    }

    /// « Couper le son à l'image courante » : l'extrait sélectionné, sinon tous ceux sous la tête
    /// de lecture.
    pub(crate) fn split_sound_at_playhead(&mut self) {
        let at = self.global_frame();
        let only = self.sel_clip.filter(|&id| self.doc.clip(id).is_some_and(|c| c.start < at && at < c.end()));
        self.split_sound(at, only);
    }

    // ------------------------------------------------------------------ glisser un extrait

    /// Piste sous le pointeur (dans la bande de montage) et extrait touché, avec la partie saisie.
    pub(crate) fn lane_hit(&self, p: Pos2, left: f32, ppf: f32, lanes_top: f32) -> Option<(usize, Option<(u64, ClipEdge)>)> {
        if p.y < lanes_top {
            return None;
        }
        let t = ((p.y - lanes_top) / TRACK_H) as usize;
        let track = self.doc.audio_tracks.get(t)?;
        let hit = track.clips.iter().find_map(|c| {
            let (x0, x1) = (left + c.start as f32 * ppf, left + c.end() as f32 * ppf);
            if p.x < x0 - 1.0 || p.x > x1 + 1.0 {
                return None;
            }
            let grip = 5.0f32.min((x1 - x0) / 3.0);
            let edge = if p.x <= x0 + grip {
                ClipEdge::Left
            } else if p.x >= x1 - grip {
                ClipEdge::Right
            } else {
                ClipEdge::Move
            };
            Some((c.id, edge))
        });
        Some((t, hit))
    }

    pub(crate) fn begin_clip_drag(&mut self, id: u64, edge: ClipEdge, origin: Pos2) {
        self.sel_clip = Some(id);
        if self.doc.clip_locked(id) {
            self.status = TRACK_LOCKED.into();
            return;
        }
        self.montage_drag = Some(MontageDrag::Clip(ClipDrag {
            id,
            edge,
            origin,
            before: self.doc.audio_tracks.clone(),
            snap: self.snapshot(),
        }));
    }

    /// Déplace ou raccourcit l'extrait glissé. Une place déjà prise est refusée : l'extrait reste
    /// à sa dernière position possible.
    pub(crate) fn update_clip_drag(&mut self, p: Pos2, ppf: f32, lanes_top: f32) {
        let Some(MontageDrag::Clip(d)) = &self.montage_drag else { return };
        let (id, edge, origin) = (d.id, d.edge, d.origin);
        let before = d.before.clone();
        let found = before
            .iter()
            .enumerate()
            .find_map(|(t, tr)| tr.clips.iter().find(|c| c.id == id).map(|c| (t, c.clone())));
        let Some((t0, orig)) = found else {
            self.montage_drag = None;
            return;
        };
        let fps = self.doc.fps;
        let total = self
            .doc
            .sounds
            .get(&orig.sound)
            .map_or(orig.offset + orig.length, |s| s.frames(fps));
        let delta = ((p.x - origin.x) / ppf).round() as i64;
        let (track, start, end, keep) = match edge {
            ClipEdge::Move => {
                let t = (((p.y - lanes_top) / TRACK_H).floor().max(0.0) as usize).min(before.len() - 1);
                let s = (orig.start as i64 + delta).max(0) as usize;
                (t, s, s + orig.length, true)
            }
            ClipEdge::Left => {
                // On ne peut pas découvrir de son avant le début du fichier.
                let min = (orig.start as i64 - orig.offset as i64).max(0);
                let s = (orig.start as i64 + delta).clamp(min, orig.end() as i64 - 1) as usize;
                (t0, s, orig.end(), false)
            }
            ClipEdge::Right => {
                let max = orig.start + total.saturating_sub(orig.offset);
                let e = (orig.end() as i64 + delta).clamp(orig.start as i64 + 1, max.max(orig.start + 1) as i64) as usize;
                (t0, orig.start, e, false)
            }
        };
        let last = std::mem::replace(&mut self.doc.audio_tracks, before);
        if !self.doc.set_clip_span(id, track, start, end, keep) {
            self.doc.audio_tracks = last;
        }
        if let Some(c) = self.doc.clip(id) {
            let name = self.doc.sounds.get(&c.sound).map_or("son", |s| s.name.as_str());
            self.status = format!(
                "« {name} » : à {}, durée {}",
                timecode(c.start, fps),
                duration_label(c.length, fps)
            );
        }
    }

    pub(crate) fn finish_clip_drag(&mut self, d: ClipDrag) {
        if self.doc.audio_tracks != d.before {
            self.history.push(d.snap);
            self.modified = true;
        }
    }

    // ------------------------------------------------------------------ affichage

    /// Pistes son sous les panneaux, dans la bande de montage. `left` = abscisse de l'image 0.
    pub(crate) fn paint_lanes(&self, painter: &Painter, r: Rect, left: f32, ppf: f32, lanes_top: f32, select: Color32) {
        let fps = self.doc.fps;
        for (t, track) in self.doc.audio_tracks.iter().enumerate() {
            let y = lanes_top + t as f32 * TRACK_H;
            let lane = Rect::from_min_max(pos2(r.left(), y + 1.0), pos2(r.right(), y + TRACK_H - 1.0));
            painter.rect_filled(lane, 0.0, theme::RANGEE_SOMBRE);
            for clip in &track.clips {
                let cr = Rect::from_min_max(
                    pos2(left + clip.start as f32 * ppf + 0.5, lane.top() + 1.0),
                    pos2(left + clip.end() as f32 * ppf - 0.5, lane.bottom() - 1.0),
                );
                if !painter.clip_rect().intersects(cr) {
                    continue;
                }
                let selected = self.sel_clip == Some(clip.id);
                let fill = match (track.muted, selected) {
                    (true, _) => theme::CONTROLE,
                    (false, true) => theme::SON_CHOISI,
                    (false, false) => theme::SON_EXTRAIT,
                };
                painter.rect_filled(cr, 3.0, fill);
                if let Some(sound) = self.doc.sounds.get(&clip.sound) {
                    let wave = if track.muted {
                        theme::TEXTE_DOUX
                    } else {
                        theme::SON_ONDE
                    };
                    let inner = cr.shrink2(vec2(0.0, 2.0));
                    paint_wave(&painter.with_clip_rect(inner), inner, &sound.pcm, clip, left, ppf, fps, wave);
                    painter.with_clip_rect(cr.shrink(2.0)).text(
                        pos2(cr.left() + 4.0, cr.top() + 2.0),
                        Align2::LEFT_TOP,
                        &sound.name,
                        FontId::proportional(10.0),
                        theme::TEXTE_FORT,
                    );
                }
                if selected {
                    painter.rect_stroke(cr, 3.0, Stroke::new(2.0_f32, select), StrokeKind::Inside);
                }
            }
        }
    }

    /// Colonne des noms de pistes, à gauche de la bande de montage. `lanes_top` : distance entre
    /// le haut de la bande et la première piste. Renvoie la colonne (les noms des autres pistes
    /// s'y ajoutent au-dessus).
    pub(crate) fn track_headers(&mut self, ui: &mut egui::Ui, lanes_top: f32) -> Rect {
        let n = self.doc.audio_tracks.len();
        let (rect, _) = ui.allocate_exact_size(vec2(HEADERS_W, lanes_top + n as f32 * TRACK_H), Sense::hover());
        let visuals = ui.visuals().clone();
        let painter = ui.painter_at(rect);
        let mut toggle = None;
        for t in 0..n {
            let track = &self.doc.audio_tracks[t];
            let lane = Rect::from_min_size(
                pos2(rect.left() + 2.0, rect.top() + lanes_top + t as f32 * TRACK_H + 1.0),
                vec2(HEADERS_W - 8.0, TRACK_H - 2.0),
            );
            let mute = Rect::from_min_size(pos2(lane.left() + 5.0, lane.bottom() - 18.0), vec2(20.0, 15.0));
            painter.rect_filled(lane, 3.0, theme::ETIQUETTE);
            let text = if track.muted {
                visuals.weak_text_color()
            } else {
                visuals.text_color()
            };
            painter.with_clip_rect(lane.shrink(2.0)).text(
                pos2(lane.left() + 6.0, lane.top() + 3.0),
                Align2::LEFT_TOP,
                &track.name,
                FontId::proportional(12.0),
                text,
            );
            painter.rect_filled(
                mute,
                2.0,
                if track.muted {
                    theme::SON_MUET
                } else {
                    theme::CONTROLE_SURVOL
                },
            );
            painter.text(mute.center(), Align2::CENTER_CENTER, "M", FontId::monospace(10.0), Color32::WHITE);
            if track.locked {
                painter.text(pos2(lane.right() - 6.0, lane.top() + 3.0), Align2::RIGHT_TOP, "🔒", FontId::proportional(11.0), visuals.text_color());
            }
            painter.text(
                pos2(mute.right() + 6.0, mute.center().y),
                Align2::LEFT_CENTER,
                format!("{:.0} %", track.volume * 100.0),
                FontId::monospace(10.0),
                visuals.weak_text_color(),
            );
            let resp = ui
                .interact(lane, ui.id().with(("piste_son", track.id)), Sense::click())
                .on_hover_text("M : rendre la piste muette. Clic droit : nom, volume, import, suppression.");
            if resp.clicked() && resp.interact_pointer_pos().is_some_and(|p| mute.expand(2.0).contains(p)) {
                toggle = Some(t);
            }
            resp.context_menu(|ui| self.track_menu(ui, t));
        }
        if let Some(t) = toggle {
            self.record_step();
            let track = &mut self.doc.audio_tracks[t];
            track.muted = !track.muted;
        }
        rect
    }

    /// Menu d'une piste son (nom, muette, volume, import, nouvelle piste, suppression).
    pub(crate) fn track_menu(&mut self, ui: &mut egui::Ui, t: usize) {
        let Some(track) = self.doc.audio_tracks.get(t) else {
            ui.close_menu();
            return;
        };
        // Copies modifiées par les champs ; `edited` enregistre l'annulation (une étape par geste).
        let (mut name, mut muted, mut pct) = (track.name.clone(), track.muted, track.volume * 100.0);
        ui.horizontal(|ui| {
            ui.label("Nom");
            let resp = ui.add(egui::TextEdit::singleline(&mut name).desired_width(120.0));
            if self.edited(&resp) {
                self.doc.audio_tracks[t].name = name;
            }
        });
        let resp = ui.checkbox(&mut muted, "Piste muette");
        if self.edited(&resp) {
            self.doc.audio_tracks[t].muted = muted;
        }
        let mut locked = self.doc.audio_tracks[t].locked;
        let resp = ui.checkbox(&mut locked, "Piste verrouillée").on_hover_text("Ses extraits ne se déplacent, ne se coupent ni ne se suppriment plus.");
        if self.edited(&resp) {
            self.doc.audio_tracks[t].locked = locked;
        }
        let resp = ui.add(egui::Slider::new(&mut pct, 0.0..=200.0).suffix(" %").text("Volume"));
        if self.edited(&resp) {
            self.doc.audio_tracks[t].volume = pct / 100.0;
        }
        ui.separator();
        if ui.button("Importer un son sur cette piste…").clicked() {
            ui.close_menu();
            self.import_sound(Some(t), None);
        }
        if ui.button("Nouvelle piste son").clicked() {
            ui.close_menu();
            self.new_audio_track();
        }
        if ui.button("Dupliquer la piste").clicked() {
            ui.close_menu();
            self.duplicate_track(t);
        }
        if ui.button("Supprimer la piste").clicked() {
            ui.close_menu();
            self.delete_audio_track(t);
        }
    }

    /// Menu du clic droit dans une piste de la bande de montage.
    pub(crate) fn lane_menu(&mut self, ui: &mut egui::Ui, track: usize, clip: Option<u64>, frame: usize) {
        let fps = self.doc.fps;
        if let Some(id) = clip.filter(|&id| self.doc.clip(id).is_some()) {
            if ui.button(format!("Couper le son ici ({})", timecode(frame, fps))).clicked() {
                ui.close_menu();
                self.split_sound(frame, Some(id));
            }
            if ui.button("Supprimer l'extrait").clicked() {
                ui.close_menu();
                self.delete_clip(id);
            }
            ui.separator();
        }
        if ui.button(format!("Importer un son ici ({})…", timecode(frame, fps))).clicked() {
            ui.close_menu();
            self.import_sound(Some(track), Some(frame));
        }
        ui.separator();
        self.track_menu(ui, track);
    }
}

/// Forme d'onde d'un extrait dans `rect`. `left` = abscisse de l'image 0 du montage, `ppf` =
/// largeur d'une image. Une colonne de pixel = l'amplitude maximale du son sous elle.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_wave(
    painter: &Painter,
    rect: Rect,
    pcm: &Pcm,
    clip: &SoundClip,
    left: f32,
    ppf: f32,
    fps: u32,
    color: Color32,
) {
    let visible = rect.intersect(painter.clip_rect());
    if !visible.is_positive() {
        return;
    }
    let spf = audio::samples_per_frame(fps) as f32;
    let mid = rect.center().y;
    let half = (rect.height() * 0.5 - 1.0).max(1.0);
    // Image du son = image du montage + `base`.
    let base = clip.offset as f32 - clip.start as f32;
    let mut x = visible.left().floor();
    while x < visible.right() {
        let f0 = (x - left) / ppf + base;
        let f1 = (x + 1.0 - left) / ppf + base;
        if f1 > 0.0 {
            let s0 = (f0.max(0.0) * spf) as usize;
            let s1 = ((f1 * spf) as usize).max(s0 + 1);
            let a = pcm.peak_between(s0, s1);
            if a > 0.004 {
                let h = (a * half).max(0.5);
                painter.line_segment([pos2(x + 0.5, mid - h), pos2(x + 0.5, mid + h)], Stroke::new(1.0_f32, color));
            }
        }
        x += 1.0;
    }
}
