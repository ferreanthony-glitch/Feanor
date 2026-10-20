//! Le panneau Bruitages (`bruitages.rs` pour la bibliothèque) : ajouter ses dossiers de sons, chercher,
//! écouter d'un clic, poser dans le montage en glissant le son sur une piste ou d'un double-clic
//! (à la tête de lecture).
//!
//! Le son posé est lu et décodé sur un fil à part, puis copié dans le projet comme tout son importé
//! (`App::place_imported_sound`) : l'interface ne attend jamais le disque.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, TryRecvError};

use eframe::egui::{self, pos2, vec2, Align2, FontId, Rect, RichText, Sense, Stroke};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::audio::{AudioEngine, Pcm};
use crate::bruitages::{duration_text, Bibliotheque, Sortie};
use crate::soundtrack::TRACK_H;
use crate::theme;

/// Hauteur d'une ligne de la liste, en points.
const ROW_H: f32 = 22.0;

/// Un son glissé depuis la liste vers le montage (la charge du glisser-déposer d'egui).
#[derive(Clone, Debug)]
pub(crate) struct SoundDrag {
    pub path: PathBuf,
}

/// Un son qu'on pose : lu et décodé sur un fil à part, posé dès qu'il est prêt.
struct Insertion {
    path: PathBuf,
    track: Option<usize>,
    at: usize,
    rx: Receiver<Result<(Vec<u8>, Pcm), String>>,
}

/// L'état de l'interface des bruitages (un seul champ dans `App`).
#[derive(Default)]
pub(crate) struct BruitagesUi {
    pub lib: Bibliotheque,
    pending: Vec<Insertion>,
    /// La piste où le dernier bruitage a été posé (à défaut d'extrait choisi).
    last_track: Option<usize>,
}

/// La sortie audio de SHOT pour l'écoute. `busy` : le projet joue ; l'écoute ne l'arrête alors pas.
struct Moteur<'a> {
    engine: &'a AudioEngine,
    busy: bool,
}

impl Sortie for Moteur<'_> {
    fn jouer(&mut self, pcm: std::sync::Arc<Pcm>) {
        let (clips, len) = crate::bruitages::extrait_ecoute(pcm);
        self.engine.play(clips, 0, (0, len), false);
    }
    fn arreter(&mut self) {
        if !self.busy {
            self.engine.stop();
        }
    }
    fn sonne(&self) -> bool {
        self.engine.is_playing()
    }
}

/// Pas de sortie audio : rien ne sonne.
struct SansSortie;

impl Sortie for SansSortie {
    fn jouer(&mut self, _: std::sync::Arc<Pcm>) {}
    fn arreter(&mut self) {}
    fn sonne(&self) -> bool {
        false
    }
}

/// Le nom d'un chemin pour l'affichage (le dernier élément).
fn short(p: &std::path::Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}

/// La piste et l'image visées par un point de la bande de montage : l'image sous le point, et la
/// piste son si le point est dans les pistes (sinon la première piste libre sera prise).
pub(crate) fn drop_target(p: egui::Pos2, left: f32, ppf: f32, lanes_top: f32, tracks: usize) -> (Option<usize>, usize) {
    let frame = ((p.x - left) / ppf).floor().max(0.0) as usize;
    let track = (p.y >= lanes_top).then(|| ((p.y - lanes_top) / TRACK_H) as usize).filter(|t| *t < tracks);
    (track, frame)
}

impl App {
    // ------------------------------------------------------------------ à chaque image d'écran

    /// Reprend le parcours des dossiers, les durées lues, l'écoute et les sons en cours de pose.
    pub(crate) fn poll_bruitages(&mut self, ctx: &egui::Context) {
        let active = {
            let b = &mut self.brt;
            let changed = b.lib.poll();
            if changed {
                ctx.request_repaint();
            }
            b.lib.scanning() || b.lib.ecoute.courant().is_some() || !b.pending.is_empty()
        };
        if self.brt.lib.ecoute.courant().is_some() {
            let busy = self.playing;
            let mut ecoute = std::mem::take(&mut self.brt.lib.ecoute);
            let error = match self.audio.as_ref().filter(|e| e.ok()) {
                Some(engine) => ecoute.suivre(&mut Moteur { engine, busy }),
                None => ecoute.suivre(&mut SansSortie),
            };
            self.brt.lib.ecoute = ecoute;
            if let Some(e) = error {
                self.status = format!("Écoute impossible : {e}");
            }
        }
        // Les sons décodés se posent.
        let mut ready = Vec::new();
        let mut i = 0;
        while i < self.brt.pending.len() {
            match self.brt.pending[i].rx.try_recv() {
                Ok(result) => ready.push((self.brt.pending.remove(i), result)),
                Err(TryRecvError::Empty) => i += 1,
                Err(TryRecvError::Disconnected) => {
                    let lost = self.brt.pending.remove(i);
                    self.status = format!("« {} » : le chargement s'est interrompu.", short(&lost.path));
                }
            }
        }
        for (job, result) in ready {
            match result {
                Ok((bytes, pcm)) => {
                    let name = job.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "bruitage".into());
                    let ext = job.path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
                    let before = std::mem::replace(&mut self.history.context, Some("Insérer un bruitage".into()));
                    self.place_imported_sound(name, ext, bytes, pcm, job.track, job.at);
                    self.history.context = before;
                    self.brt.last_track = self.sel_clip.and_then(|id| self.doc.find_clip(id)).map(|(t, _)| t);
                }
                Err(e) => self.status = format!("Impossible d'insérer « {} » : {e}", short(&job.path)),
            }
        }
        if active || !self.brt.pending.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(60));
        }
    }

    // ------------------------------------------------------------------ opérations

    /// Plus aucun son en cours de pose (essais, et changement d'onglet : `onglets.rs`).
    pub(crate) fn brt_pending_is_empty(&self) -> bool {
        self.brt.pending.is_empty()
    }

    /// Pose le son `path` dans le montage, à l'image `at` (la tête de lecture par défaut) de la piste
    /// `track` (celle de l'extrait choisi, sinon la dernière utilisée, sinon la première libre).
    pub(crate) fn insert_bruitage(&mut self, path: PathBuf, track: Option<usize>, at: Option<usize>) {
        if !crate::bruitages::is_sound(&path) {
            self.status = format!("« {} » : format de son non géré (WAV, MP3, FLAC ou OGG).", short(&path));
            return;
        }
        let at = at.unwrap_or_else(|| self.global_frame());
        let track = track
            .or_else(|| self.sel_clip.and_then(|id| self.doc.find_clip(id)).map(|(t, _)| t))
            .or(self.brt.last_track)
            .filter(|&t| t < self.doc.audio_tracks.len() && !self.doc.audio_tracks[t].locked);
        let (tx, rx) = channel();
        let p = path.clone();
        std::thread::spawn(move || {
            let result = std::fs::read(&p).map_err(|e| format!("lecture impossible ({e})")).and_then(|bytes| {
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
                crate::audio::decode(&bytes, &ext).map(|pcm| (bytes, pcm))
            });
            let _ = tx.send(result);
        });
        self.status = format!("Chargement de « {} »…", short(&path));
        self.brt.pending.push(Insertion { path, track, at, rx });
    }

    /// Écoute (ou arrête) un son de la bibliothèque.
    pub(crate) fn listen_bruitage(&mut self, path: &std::path::Path) {
        // Le projet qui joue s'arrête : la sortie audio est à l'écoute.
        if self.playing {
            self.playing = false;
            self.stop_audio();
        }
        let mut ecoute = std::mem::take(&mut self.brt.lib.ecoute);
        if self.audio_engine().is_none() && !ecoute.joue() {
            self.brt.lib.ecoute = ecoute;
            return;
        }
        match self.audio.as_ref() {
            Some(engine) => ecoute.basculer(path, &mut Moteur { engine, busy: false }),
            None => ecoute.basculer(path, &mut SansSortie),
        }
        self.brt.lib.ecoute = ecoute;
    }

    /// Ajoute un dossier de sons (boîte de choix), et relance le parcours.
    fn add_bruitage_folder(&mut self) {
        let Some(dir) = rfd::FileDialog::new().set_title("Dossier de sons à ajouter à la bibliothèque").pick_folder() else { return };
        if self.settings.bruitages.add(dir.clone()) {
            let _ = self.settings.save();
            self.rescan_bruitages();
            self.status = format!("Dossier ajouté : {}", dir.display());
        } else {
            self.status = format!("« {} » est déjà dans la bibliothèque (ou dans l'un de ses dossiers).", dir.display());
        }
    }

    pub(crate) fn rescan_bruitages(&mut self) {
        let folders = self.settings.bruitages.folders.clone();
        self.brt.lib.scan(&folders);
    }

    /// Les commandes `bruitage.*` ; vrai si `action` en était une.
    pub(crate) fn bruitage_action(&mut self, action: &str) -> bool {
        match action {
            "bruitage.inserer" | "bruitage.ecouter" => {
                let Some(path) = self.brt.lib.selected.clone() else {
                    self.status = "Choisis d'abord un son dans le panneau Bruitages.".into();
                    self.show_pane(crate::dock::Pane::Bruitages);
                    return true;
                };
                if action == "bruitage.inserer" {
                    self.insert_bruitage(path, None, None);
                } else {
                    self.listen_bruitage(&path);
                }
                true
            }
            _ => false,
        }
    }

    // ------------------------------------------------------------------ le panneau

    pub(crate) fn bruitages_panel(&mut self, ui: &mut egui::Ui) {
        // La bibliothèque se parcourt à la première ouverture du panneau, jamais au lancement.
        if !self.brt.lib.started && !self.settings.bruitages.folders.is_empty() {
            self.rescan_bruitages();
        }
        ui.horizontal(|ui| {
            if crate::bulles::icone(ui, ph::FOLDER_SIMPLE_PLUS, "Ajouter un dossier", "Un dossier de tes sons (WAV, MP3, FLAC, OGG), sous-dossiers compris. Il est retenu d'un projet à l'autre.", "").clicked() {
                self.add_bruitage_folder();
            }
            let resp = ui.add_enabled(!self.settings.bruitages.folders.is_empty(), egui::Button::new(RichText::new(ph::ARROWS_CLOCKWISE).size(17.0)).frame(false));
            if crate::bulles::bulle(resp, "Actualiser", "Relire les dossiers (sons ajoutés ou retirés depuis).", "").clicked() {
                self.rescan_bruitages();
            }
            let n = self.brt.lib.entries.len();
            let text = if self.brt.lib.scanning() {
                format!("{n} sons…")
            } else if self.brt.lib.truncated {
                format!("{n} sons (liste coupée)")
            } else {
                format!("{n} son{}", if n > 1 { "s" } else { "" })
            };
            if self.brt.lib.scanning() {
                ui.spinner();
            }
            ui.label(RichText::new(text).weak());
        });
        // Les dossiers : repliés dès qu'il y en a, avec de quoi en retirer.
        let folders = self.settings.bruitages.folders.clone();
        if !folders.is_empty() {
            egui::CollapsingHeader::new(RichText::new(format!("Dossiers ({})", folders.len())).small()).id_salt("dossiers_bruitages").default_open(false).show(ui, |ui| {
                let mut remove = None;
                for (i, f) in folders.iter().enumerate() {
                    ui.horizontal(|ui| {
                        if crate::bulles::bulle(ui.small_button(ph::X), "Retirer le dossier", "Retire le dossier de la bibliothèque (ses fichiers ne sont pas touchés).", "").clicked() {
                            remove = Some(i);
                        }
                        ui.label(RichText::new(f.display().to_string()).small()).on_hover_text(f.display().to_string());
                    });
                }
                if let Some(i) = remove {
                    self.settings.bruitages.remove(i);
                    let _ = self.settings.save();
                    self.rescan_bruitages();
                }
            });
        }
        ui.add(egui::TextEdit::singleline(&mut self.brt.lib.search).hint_text("Chercher un bruitage…").desired_width(f32::INFINITY));
        ui.add_space(2.0);
        if folders.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new("Aucun dossier de sons.").strong());
            ui.label("Ajoute le dossier où tu ranges tes bruitages : tu pourras les chercher, les écouter, puis les glisser sur une piste son du montage ou double-cliquer pour les poser à la tête de lecture.");
            return;
        }
        let count = self.brt.lib.visible().len();
        if count == 0 {
            ui.add_space(8.0);
            ui.label(RichText::new(if self.brt.lib.scanning() { "Lecture des dossiers…" } else if self.brt.lib.entries.is_empty() { "Aucun son dans ces dossiers." } else { "Aucun son ne correspond." }).weak());
            return;
        }
        let mut action: Option<RowAction> = None;
        let height = ui.available_height();
        egui::ScrollArea::vertical().id_salt("liste_bruitages").auto_shrink([false, false]).max_height(height).show_rows(ui, ROW_H, count, |ui, range| {
            self.brt.lib.want_durations(range.clone());
            for r in range {
                let Some(&i) = self.brt.lib.visible().get(r) else { continue };
                let entry = self.brt.lib.entries[i].clone();
                let seconds = self.brt.lib.sonde.get(&entry.path);
                let state = {
                    let e = &self.brt.lib.ecoute;
                    if e.courant() == Some(&entry.path) {
                        if e.charge() {
                            Listening::Loading
                        } else {
                            Listening::Playing
                        }
                    } else {
                        Listening::No
                    }
                };
                let selected = self.brt.lib.selected.as_ref() == Some(&entry.path);
                if let Some(a) = row(ui, &entry, seconds, state, selected) {
                    action = Some(a);
                }
            }
        });
        match action {
            Some(RowAction::Select(p)) => self.brt.lib.selected = Some(p),
            Some(RowAction::Listen(p)) => {
                self.brt.lib.selected = Some(p.clone());
                self.listen_bruitage(&p);
            }
            Some(RowAction::Insert(p)) => {
                self.brt.lib.selected = Some(p.clone());
                self.insert_bruitage(p, None, None);
            }
            Some(RowAction::Drag(p)) => {
                self.brt.lib.selected = Some(p.clone());
                egui::DragAndDrop::set_payload(ui.ctx(), SoundDrag { path: p });
            }
            None => {}
        }
        // Le son glissé suit le pointeur.
        if let Some(drag) = egui::DragAndDrop::payload::<SoundDrag>(ui.ctx()) {
            if let Some(p) = ui.ctx().pointer_latest_pos() {
                egui::Area::new(egui::Id::new("bruitage_glisse")).order(egui::Order::Tooltip).fixed_pos(p + vec2(14.0, 10.0)).interactable(false).show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label(format!("{} {}", ph::WAVEFORM, short(&drag.path)));
                    });
                });
            }
        }
    }

    // ------------------------------------------------------------------ poser dans le montage

    /// Un son glissé sur la bande de montage : un repère à l'image visée pendant le survol, la pose
    /// au lâcher (sur la piste visée, sinon la première libre).
    pub(crate) fn bruitage_drop(&mut self, ui: &egui::Ui, resp: &egui::Response, left: f32, ppf: f32, lanes_top: f32) {
        let tracks = self.doc.audio_tracks.len();
        let top = resp.rect.top();
        if resp.dnd_hover_payload::<SoundDrag>().is_some() {
            if let Some(p) = ui.input(|i| i.pointer.hover_pos()) {
                let (track, frame) = drop_target(p, left, ppf, lanes_top, tracks);
                let x = left + frame as f32 * ppf;
                let stroke = Stroke::new(2.0_f32, theme::contour());
                let painter = ui.painter_at(resp.rect);
                painter.line_segment([pos2(x, top), pos2(x, resp.rect.bottom())], stroke);
                if let Some(t) = track {
                    let y = lanes_top + t as f32 * TRACK_H;
                    painter.rect_stroke(Rect::from_min_max(pos2(x, y + 1.0), pos2(x + 8.0 * ppf, y + TRACK_H - 1.0)), 3.0, stroke, egui::StrokeKind::Inside);
                }
            }
        }
        if let Some(drag) = resp.dnd_release_payload::<SoundDrag>() {
            if let Some(p) = ui.input(|i| i.pointer.interact_pos().or(i.pointer.hover_pos())) {
                let (track, frame) = drop_target(p, left, ppf, lanes_top, tracks);
                self.insert_bruitage(drag.path.clone(), track, Some(frame));
            }
        }
    }

    /// Le même dépôt sur la ligne du son de la timeline (en animation) : `left` est l'abscisse de
    /// l'image 0 du montage, `cell` la largeur d'une image.
    pub(crate) fn bruitage_drop_row(&mut self, ui: &egui::Ui, area: Rect, left: f32, cell: f32) {
        let ctx = ui.ctx();
        if !egui::DragAndDrop::has_payload_of_type::<SoundDrag>(ctx) {
            return;
        }
        let Some(p) = ui.input(|i| i.pointer.hover_pos()) else { return };
        if !area.contains(p) {
            return;
        }
        let frame = ((p.x - left) / cell).floor().max(0.0) as usize;
        let x = left + frame as f32 * cell;
        ui.painter_at(area).line_segment([pos2(x, area.top()), pos2(x, area.bottom())], Stroke::new(2.0_f32, theme::contour()));
        if ui.input(|i| i.pointer.any_released()) {
            if let Some(drag) = egui::DragAndDrop::take_payload::<SoundDrag>(ctx) {
                self.insert_bruitage(drag.path.clone(), None, Some(frame));
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Listening {
    No,
    Loading,
    Playing,
}

enum RowAction {
    Select(PathBuf),
    Listen(PathBuf),
    Insert(PathBuf),
    Drag(PathBuf),
}

/// Une ligne de la liste : le bouton d'écoute, le nom, la durée.
fn row(ui: &mut egui::Ui, entry: &crate::bruitages::Entry, seconds: Option<Option<f64>>, state: Listening, selected: bool) -> Option<RowAction> {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    if selected {
        painter.rect_filled(rect, 3.0, ui.visuals().selection.bg_fill.gamma_multiply(0.55));
    } else if resp.hovered() {
        painter.rect_filled(rect, 3.0, theme::eleve());
    }
    let play = Rect::from_min_size(rect.min + vec2(2.0, 1.0), vec2(ROW_H, ROW_H - 2.0));
    let icon = match state {
        Listening::Playing => ph::STOP,
        Listening::Loading => ph::DOTS_THREE,
        Listening::No => ph::PLAY,
    };
    painter.text(play.center(), Align2::CENTER_CENTER, icon, FontId::proportional(14.0), if state == Listening::No { theme::texte_doux() } else { theme::accent_texte() });
    let dur = duration_text(seconds);
    let dur_w = if dur.is_empty() { 0.0 } else { 52.0 };
    painter.text(pos2(rect.right() - 4.0, rect.center().y), Align2::RIGHT_CENTER, &dur, FontId::monospace(11.0), theme::texte_doux());
    let text_rect = Rect::from_min_max(pos2(play.right() + 4.0, rect.top()), pos2(rect.right() - dur_w - 4.0, rect.bottom()));
    let mut name = entry.name.clone();
    if !entry.rel.is_empty() {
        name = format!("{name}  ·  {}", entry.rel);
    }
    painter.with_clip_rect(text_rect).text(pos2(text_rect.left(), rect.center().y), Align2::LEFT_CENTER, name, FontId::proportional(12.5), if selected { theme::texte_fort() } else { theme::texte() });
    let resp = resp.on_hover_text(format!("{}\nClic : choisir · bouton ▶ : écouter · double-clic : poser à la tête de lecture · glisser sur une piste son : poser là", entry.path.display()));
    let p = entry.path.clone();
    if resp.drag_started() {
        return Some(RowAction::Drag(p));
    }
    if resp.double_clicked() {
        return Some(RowAction::Insert(p));
    }
    if resp.clicked() {
        let on_play = resp.interact_pointer_pos().is_some_and(|q| play.expand(2.0).contains(q));
        return Some(if on_play { RowAction::Listen(p) } else { RowAction::Select(p) });
    }
    None
}
