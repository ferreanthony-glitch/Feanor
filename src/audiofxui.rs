//! Les effets sonores dans l'interface (`audiofx.rs` pour le calcul) : sur les extraits de la bande de
//! montage, les poignées des fondus aux coins du haut, la ligne des clés de volume (double-clic pour
//! poser une clé, glisser pour la régler, Suppr pour la retirer) et la croix du fondu enchaîné ;
//! dans les menus, les réglages de l'extrait et le solo des pistes ; les commandes `son.*`.
//!
//! Tout geste est une étape d'annulation à nom français.

use eframe::egui::{self, pos2, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};

use crate::app::App;
use crate::audiofx::{ClipFx, EQ_RANGE, MAX_GAIN};
use crate::document::{AudioTrack, SoundClip};
use crate::soundtrack::{ClipEdge, TRACK_H};
use crate::theme;

/// Le gain tout en haut de l'extrait (le milieu est le gain 1 : pas de changement).
pub(crate) const GAIN_TOP: f32 = 2.0;
/// Fondu posé par la commande « fondu d'entrée / de sortie », en images.
pub(crate) const DEFAULT_FADE: usize = 6;
/// Rayon de prise d'une clé, d'une poignée, en points.
const REACH: f32 = 6.0;
/// Hauteur de la bande du haut d'un extrait où l'on prend les poignées des fondus.
const GRIP_BAND: f32 = 12.0;
/// Largeur sous laquelle un extrait n'a plus de poignées (trop étroit pour qu'on y vise).
const MIN_WIDTH: f32 = 24.0;

/// État de l'interface des effets sonores (un seul champ dans `App`).
#[derive(Default)]
pub(crate) struct SoundFxUi {
    /// La clé de volume choisie : (extrait, rang).
    pub key: Option<(u64, usize)>,
}

/// Le rectangle d'un extrait dans la bande de montage (voir `App::paint_lanes`).
pub(crate) fn clip_rect(track: usize, clip: &SoundClip, left: f32, ppf: f32, lanes_top: f32) -> Rect {
    let y = lanes_top + track as f32 * TRACK_H;
    Rect::from_min_max(pos2(left + clip.start as f32 * ppf + 0.5, y + 2.0), pos2(left + clip.end() as f32 * ppf - 0.5, y + TRACK_H - 2.0))
}

/// L'ordonnée du gain `v` dans l'extrait `cr` (le gain 1 au milieu, `GAIN_TOP` en haut, 0 en bas).
pub(crate) fn gain_y(cr: Rect, v: f32) -> f32 {
    let inner = cr.shrink2(vec2(0.0, 2.0));
    inner.bottom() - (v / GAIN_TOP).clamp(0.0, 1.0) * inner.height()
}

/// Le gain à l'ordonnée `y` (l'inverse de `gain_y`), collé à 1 près de l'unité.
pub(crate) fn gain_at_y(cr: Rect, y: f32) -> f32 {
    let inner = cr.shrink2(vec2(0.0, 2.0));
    let v = ((inner.bottom() - y) / inner.height().max(1.0) * GAIN_TOP).clamp(0.0, MAX_GAIN.min(GAIN_TOP));
    if (v - 1.0).abs() < 0.03 {
        1.0
    } else {
        v
    }
}

/// Le point d'une clé de volume dans l'extrait.
pub(crate) fn key_point(cr: Rect, clip: &SoundClip, i: usize, ppf: f32) -> Pos2 {
    let k = clip.fx.keys[i];
    pos2((cr.left() + k.t * ppf).clamp(cr.left(), cr.right()), gain_y(cr, k.v))
}

/// Ce qu'un point touche des effets de l'extrait `clip` (piste `track`) : une clé de volume (extrait
/// choisi), une poignée de fondu (extrait choisi, ou qui a déjà son fondu) ou la croix d'un fondu
/// enchaîné. `junction` : le fondu enchaîné réel avec le suivant, en images.
pub(crate) fn fx_hit(selected: bool, clip: &SoundClip, junction: f64, track: usize, p: Pos2, left: f32, ppf: f32, lanes_top: f32) -> Option<ClipEdge> {
    let cr = clip_rect(track, clip, left, ppf, lanes_top);
    if cr.width() < MIN_WIDTH || p.x < cr.left() - 1.0 || p.x > cr.right() + 1.0 {
        return None;
    }
    let fx = &clip.fx;
    if selected {
        let best = (0..fx.keys.len())
            .map(|i| (i, key_point(cr, clip, i, ppf).distance(p)))
            .filter(|&(_, d)| d <= REACH)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((i, _)) = best {
            return Some(ClipEdge::Key(i));
        }
    }
    if p.y <= cr.top() + GRIP_BAND {
        let fade_in = (fx.fade_in as f32 * ppf).min(cr.width());
        let fade_out = (fx.fade_out as f32 * ppf).min(cr.width());
        let near_in = (selected || fx.fade_in > 0).then(|| (p.x - (cr.left() + fade_in)).abs()).filter(|d| *d <= REACH);
        let near_out = (selected || fx.fade_out > 0).then(|| (p.x - (cr.right() - fade_out)).abs()).filter(|d| *d <= REACH);
        match (near_in, near_out) {
            (Some(a), Some(b)) => return Some(if a <= b { ClipEdge::FadeIn } else { ClipEdge::FadeOut }),
            (Some(_), None) => return Some(ClipEdge::FadeIn),
            (None, Some(_)) => return Some(ClipEdge::FadeOut),
            (None, None) => {}
        }
    }
    if junction > 0.0 {
        let x = left + clip.end() as f32 * ppf - (junction as f32 * 0.5) * ppf;
        if (p.x - x).abs() <= REACH && (p.y - cr.center().y).abs() <= 10.0 {
            return Some(ClipEdge::Xfade);
        }
    }
    None
}

fn white(a: u8) -> Color32 {
    Color32::from_white_alpha(a)
}

/// Les effets d'un extrait par-dessus sa forme d'onde : fondus (le coin qui disparaît s'assombrit),
/// ligne du volume, clés, poignées, croix du fondu enchaîné.
pub(crate) fn paint_clip_fx(painter: &Painter, cr: Rect, clip: &SoundClip, junction: f64, selected: bool, key_sel: Option<usize>, ppf: f32) {
    let fx = &clip.fx;
    let shade = Color32::from_black_alpha(95);
    let line = Stroke::new(1.5_f32, white(220));
    let clipped = painter.with_clip_rect(cr);
    if fx.fade_in > 0 {
        let x = (cr.left() + fx.fade_in as f32 * ppf).min(cr.right());
        clipped.add(Shape::convex_polygon(vec![cr.left_top(), pos2(x, cr.top()), cr.left_bottom()], shade, Stroke::NONE));
        clipped.line_segment([cr.left_bottom(), pos2(x, cr.top())], line);
    }
    if fx.fade_out > 0 {
        let x = (cr.right() - fx.fade_out as f32 * ppf).max(cr.left());
        clipped.add(Shape::convex_polygon(vec![pos2(x, cr.top()), cr.right_top(), cr.right_bottom()], shade, Stroke::NONE));
        clipped.line_segment([pos2(x, cr.top()), cr.right_bottom()], line);
    }
    // La ligne du volume : toujours si l'extrait a des clés, et à l'unité quand il est choisi (pour
    // viser le double-clic).
    if !fx.keys.is_empty() {
        let mut pts: Vec<Pos2> = Vec::with_capacity(fx.keys.len() + 2);
        pts.push(pos2(cr.left(), gain_y(cr, fx.keys[0].v)));
        pts.extend((0..fx.keys.len()).map(|i| key_point(cr, clip, i, ppf)));
        pts.push(pos2(cr.right(), gain_y(cr, fx.keys[fx.keys.len() - 1].v)));
        clipped.add(Shape::line(pts, Stroke::new(1.5_f32, theme::accent_texte())));
        if selected || key_sel.is_some() {
            for i in 0..fx.keys.len() {
                let pt = key_point(cr, clip, i, ppf);
                let on = key_sel == Some(i);
                painter.circle_filled(pt, if on { 4.5 } else { 3.5 }, if on { theme::contour() } else { theme::accent_texte() });
                painter.circle_stroke(pt, if on { 4.5 } else { 3.5 }, Stroke::new(1.0_f32, Color32::from_black_alpha(160)));
            }
        }
    } else if selected && cr.width() >= MIN_WIDTH {
        let y = gain_y(cr, 1.0);
        clipped.line_segment([pos2(cr.left(), y), pos2(cr.right(), y)], Stroke::new(1.0_f32, white(70)));
    }
    if selected && cr.width() >= MIN_WIDTH {
        for x in [(cr.left() + fx.fade_in as f32 * ppf).min(cr.right() - 4.0), (cr.right() - fx.fade_out as f32 * ppf).max(cr.left() + 4.0)] {
            let r = Rect::from_center_size(pos2(x, cr.top() + 5.0), vec2(8.0, 8.0));
            painter.rect_filled(r, 1.5, Color32::WHITE);
            painter.rect_stroke(r, 1.5, Stroke::new(1.0_f32, Color32::from_black_alpha(200)), StrokeKind::Outside);
        }
    }
    // Le fondu enchaîné avec le suivant : une croix centrée sur la jonction, un carré pour la tirer.
    if junction > 0.0 {
        let half = junction as f32 * 0.5 * ppf;
        let jx = cr.right() + 0.5;
        let (top, bottom) = (cr.top() + 3.0, cr.bottom() - 3.0);
        let x = Stroke::new(1.5_f32, white(235));
        painter.line_segment([pos2(jx - half, bottom), pos2(jx + half, top)], x);
        painter.line_segment([pos2(jx - half, top), pos2(jx + half, bottom)], x);
        let r = Rect::from_center_size(pos2(jx - half, cr.center().y), vec2(7.0, 7.0));
        painter.rect_filled(r, 1.5, Color32::WHITE);
        painter.rect_stroke(r, 1.5, Stroke::new(1.0_f32, Color32::from_black_alpha(200)), StrokeKind::Outside);
    }
}

impl App {
    // ------------------------------------------------------------------ opérations

    /// Opération sur les extraits avec une étape d'annulation nommée `name`.
    fn fx_op(&mut self, name: &str, op: impl FnOnce(&mut crate::document::Document) -> bool) -> bool {
        let before = std::mem::replace(&mut self.history.context, Some(name.to_string()));
        let done = self.audio_op(op);
        self.history.context = before;
        done
    }

    /// Modifie les effets de l'extrait `id`.
    pub(crate) fn edit_clip_fx(&mut self, id: u64, name: &str, f: impl FnOnce(&mut ClipFx)) -> bool {
        if self.doc.clip_locked(id) {
            self.status = "Cette piste son est verrouillée : déverrouille-la pour la changer.".into();
            return false;
        }
        let done = self.fx_op(name, |d| match d.find_clip(id) {
            Some((t, c)) => {
                let fx = &mut d.audio_tracks[t].clips[c].fx;
                let before = fx.clone();
                f(fx);
                fx.sanitize();
                *fx != before
            }
            None => false,
        });
        if done {
            // L'égaliseur et l'écho se calculent à part : on le demande tout de suite.
            let _ = crate::audio::mix_clips(&self.doc);
        }
        done
    }

    /// Pose une clé de volume à `t` images du début de l'extrait, au gain `v`. Renvoie son rang.
    pub(crate) fn add_volume_key(&mut self, id: u64, t: f32, v: f32) -> Option<usize> {
        let mut at = None;
        self.edit_clip_fx(id, "Clé de volume", |fx| at = Some(fx.set_key(t, v)));
        let at = at?;
        self.sfx.key = Some((id, at));
        self.sel_clip = Some(id);
        Some(at)
    }

    /// Retire la clé de volume choisie.
    pub(crate) fn delete_volume_key(&mut self) -> bool {
        let Some((id, i)) = self.sfx.key.take() else { return false };
        self.edit_clip_fx(id, "Supprimer la clé de volume", |fx| {
            if i < fx.keys.len() {
                fx.keys.remove(i);
            }
        })
    }

    /// Une piste en solo ou non.
    pub(crate) fn toggle_solo(&mut self, track: usize) {
        let Some(on) = self.doc.audio_tracks.get(track).map(|t| t.solo) else { return };
        let name = if on { "Retirer le solo de la piste" } else { "Piste en solo" };
        self.fx_op(name, |d| {
            d.audio_tracks[track].solo = !on;
            true
        });
    }

    // ------------------------------------------------------------------ gestes

    /// Les effets d'un extrait glissés (poignée de fondu, clé, croix) : l'état d'avant le geste est
    /// remis, puis la valeur suit la souris. Renvoie faux si ce n'est pas un geste d'effet.
    pub(crate) fn update_fx_drag(&mut self, id: u64, edge: ClipEdge, origin: Pos2, p: Pos2, ppf: f32, lanes_top: f32, before: Vec<AudioTrack>) -> bool {
        if !matches!(edge, ClipEdge::FadeIn | ClipEdge::FadeOut | ClipEdge::Xfade | ClipEdge::Key(_)) {
            return false;
        }
        let Some((t0, orig)) = before.iter().enumerate().find_map(|(t, tr)| tr.clips.iter().find(|c| c.id == id).map(|c| (t, c.clone()))) else { return true };
        self.doc.audio_tracks = before;
        let Some((t, c)) = self.doc.find_clip(id) else { return true };
        let delta = (p.x - origin.x) / ppf;
        let len = orig.length;
        let fps = self.doc.fps;
        let fx = &mut self.doc.audio_tracks[t].clips[c].fx;
        let text = match edge {
            ClipEdge::FadeIn => {
                fx.fade_in = ((orig.fx.fade_in as f32 + delta).round().max(0.0) as usize).min(len.saturating_sub(orig.fx.fade_out));
                format!("Fondu d'entrée : {}", crate::storyboard::duration_label(fx.fade_in, fps))
            }
            ClipEdge::FadeOut => {
                fx.fade_out = ((orig.fx.fade_out as f32 - delta).round().max(0.0) as usize).min(len.saturating_sub(orig.fx.fade_in));
                format!("Fondu de sortie : {}", crate::storyboard::duration_label(fx.fade_out, fps))
            }
            ClipEdge::Xfade => {
                // La poignée est à gauche de la jonction : la tirer à gauche allonge le fondu.
                fx.xfade = ((orig.fx.xfade as f32 - 2.0 * delta).round().max(0.0) as usize).min(2 * len);
                format!("Fondu enchaîné : {}", crate::storyboard::duration_label(fx.xfade, fps))
            }
            ClipEdge::Key(i) => {
                let cr = clip_rect(t0, &orig, 0.0, ppf, lanes_top);
                if let Some(k) = orig.fx.keys.get(i) {
                    let v = gain_at_y(cr, p.y);
                    fx.move_key(i, k.t + delta, v);
                    let now = fx.keys.get(i).copied().unwrap_or(*k);
                    format!("Clé de volume : {:.0} % à {}", now.v * 100.0, crate::storyboard::duration_label(now.t.round() as usize, fps))
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        };
        fx.sanitize();
        self.status = text;
        true
    }

    /// Nom de l'étape d'annulation d'un geste d'effet terminé.
    pub(crate) fn fx_drag_name(edge: ClipEdge) -> Option<&'static str> {
        match edge {
            ClipEdge::FadeIn => Some("Fondu d'entrée"),
            ClipEdge::FadeOut => Some("Fondu de sortie"),
            ClipEdge::Xfade => Some("Fondu enchaîné du son"),
            ClipEdge::Key(_) => Some("Clé de volume"),
            _ => None,
        }
    }

    /// Double-clic sur un extrait : une clé de volume là où l'on a cliqué (au gain de la courbe, ou à
    /// la hauteur de la souris).
    pub(crate) fn double_click_clip(&mut self, track: usize, id: u64, p: Pos2, left: f32, ppf: f32, lanes_top: f32) {
        let Some(clip) = self.doc.clip(id).cloned() else { return };
        let cr = clip_rect(track, &clip, left, ppf, lanes_top);
        if cr.width() < MIN_WIDTH {
            return;
        }
        let t = ((p.x - cr.left()) / ppf).clamp(0.0, clip.length as f32);
        let v = gain_at_y(cr, p.y);
        self.add_volume_key(id, t, v);
        self.status = format!("Clé de volume posée à {} : {:.0} % (glisse-la, Suppr la retire).", crate::storyboard::duration_label(t.round() as usize, self.doc.fps), v * 100.0);
    }

    // ------------------------------------------------------------------ commandes

    /// Les commandes `son.*` des effets ; vrai si `action` en était une. Suppr retire la clé de
    /// volume choisie quand le montage a la main.
    pub(crate) fn sound_fx_action(&mut self, action: &str) -> bool {
        match action {
            "edition.supprimer" => {
                if self.panel_focus && self.sfx.key.is_some_and(|(id, i)| self.sel_clip == Some(id) && self.doc.clip(id).is_some_and(|c| i < c.fx.keys.len())) {
                    self.delete_volume_key();
                    return true;
                }
                false
            }
            "son.fondu_entree" | "son.fondu_sortie" | "son.enchaine" | "son.cle_volume" | "son.effets_reinit" => {
                let Some(id) = self.sel_clip.filter(|&id| self.doc.clip(id).is_some()) else {
                    self.status = "Choisis d'abord un extrait de son dans le montage.".into();
                    return true;
                };
                match action {
                    "son.fondu_entree" => {
                        self.edit_clip_fx(id, "Fondu d'entrée", |fx| fx.fade_in = if fx.fade_in > 0 { 0 } else { DEFAULT_FADE });
                    }
                    "son.fondu_sortie" => {
                        self.edit_clip_fx(id, "Fondu de sortie", |fx| fx.fade_out = if fx.fade_out > 0 { 0 } else { DEFAULT_FADE });
                    }
                    "son.enchaine" => self.toggle_sound_crossfade(id),
                    "son.cle_volume" => {
                        let g = self.global_frame();
                        if let Some(c) = self.doc.clip(id).cloned() {
                            if g >= c.start && g < c.end() {
                                let t = (g - c.start) as f32;
                                let v = c.fx.key_gain(t);
                                self.add_volume_key(id, t, v);
                            } else {
                                self.status = "La tête de lecture est hors de l'extrait choisi.".into();
                            }
                        }
                    }
                    _ => {
                        self.edit_clip_fx(id, "Retirer les effets du son", |fx| *fx = ClipFx::default());
                    }
                }
                true
            }
            "son.solo" => {
                let track = self.sel_clip.and_then(|id| self.doc.find_clip(id)).map(|(t, _)| t);
                match track {
                    Some(t) => self.toggle_solo(t),
                    None => self.status = "Choisis d'abord un extrait de son : sa piste passe en solo.".into(),
                }
                true
            }
            _ => false,
        }
    }

    /// Pose ou retire le fondu enchaîné d'un extrait avec le suivant (qui doit le toucher).
    pub(crate) fn toggle_sound_crossfade(&mut self, id: u64) {
        let Some((t, c)) = self.doc.find_clip(id) else { return };
        let track = &self.doc.audio_tracks[t];
        let touches = track.clips.get(c + 1).is_some_and(|n| n.start == track.clips[c].end());
        let on = track.clips[c].fx.xfade > 0;
        if !on && !touches {
            self.status = "Le fondu enchaîné relie deux extraits qui se touchent sur la même piste : place l'extrait suivant contre celui-ci.".into();
            return;
        }
        self.edit_clip_fx(id, "Fondu enchaîné du son", |fx| fx.xfade = if on { 0 } else { crate::audiofx::DEFAULT_XFADE });
        if !on {
            let frames = crate::audiofx::junction_frames(&self.doc, &self.doc.audio_tracks[t], c);
            if frames <= 0.0 {
                self.status = "Pas de son de l'autre côté de la coupe : le fondu enchaîné reste une coupe sèche (rogne les extraits moins pour garder du son de part et d'autre).".into();
            }
        }
    }

    // ------------------------------------------------------------------ menu

    /// Le sous-menu « Effets du son » d'un extrait.
    pub(crate) fn clip_fx_menu(&mut self, ui: &mut egui::Ui, id: u64) {
        let Some(clip) = self.doc.clip(id).cloned() else { return };
        let fx = clip.fx.clone();
        ui.menu_button("Effets du son", |ui| {
            ui.set_min_width(250.0);
            // Volume de l'extrait.
            let mut pct = fx.gain * 100.0;
            let resp = ui.add(egui::Slider::new(&mut pct, 0.0..=MAX_GAIN * 100.0).suffix(" %").text("Volume"));
            self.fx_field(&resp, id, "Volume de l'extrait", move |f| f.gain = pct / 100.0);
            // Fondus.
            let (mut fi, mut fo) = (fx.fade_in, fx.fade_out);
            ui.horizontal(|ui| {
                ui.label("Fondu d'entrée");
                let resp = ui.add(egui::DragValue::new(&mut fi).range(0..=clip.length).suffix(" i"));
                self.fx_field(&resp, id, "Fondu d'entrée", move |f| f.fade_in = fi);
                ui.label("sortie");
                let resp = ui.add(egui::DragValue::new(&mut fo).range(0..=clip.length).suffix(" i"));
                self.fx_field(&resp, id, "Fondu de sortie", move |f| f.fade_out = fo);
            });
            // Fondu enchaîné avec le suivant.
            let (t, c) = self.doc.find_clip(id).unwrap_or((0, 0));
            let touches = self.doc.audio_tracks.get(t).is_some_and(|tr| tr.clips.get(c + 1).is_some_and(|n| tr.clips[c].end() == n.start));
            ui.add_enabled_ui(touches || fx.xfade > 0, |ui| {
                let mut x = fx.xfade;
                ui.horizontal(|ui| {
                    let mut on = x > 0;
                    if ui.checkbox(&mut on, "Fondu enchaîné avec le suivant").on_hover_text("Les deux extraits doivent se toucher : le fondu est centré sur la coupe et utilise le son de chaque côté (puissance égale).").changed() {
                        self.toggle_sound_crossfade(id);
                    }
                    if x > 0 {
                        let resp = ui.add(egui::DragValue::new(&mut x).range(1..=2 * clip.length.max(1)).suffix(" i"));
                        self.fx_field(&resp, id, "Fondu enchaîné du son", move |f| f.xfade = x);
                    }
                });
            });
            ui.separator();
            // Clés de volume.
            ui.horizontal(|ui| {
                ui.label(format!("Clés de volume : {}", fx.keys.len()));
                if ui.button("Une clé ici").on_hover_text("À la tête de lecture (ou double-clic sur l'extrait).").clicked() {
                    ui.close_menu();
                    self.run_action("son.cle_volume");
                }
                if ui.add_enabled(!fx.keys.is_empty(), egui::Button::new("Retirer")).clicked() {
                    ui.close_menu();
                    self.edit_clip_fx(id, "Supprimer les clés de volume", |f| f.keys.clear());
                }
            });
            ui.separator();
            // Égaliseur.
            ui.label("Égaliseur");
            for (band, name) in ["Graves", "Médiums", "Aigus"].into_iter().enumerate() {
                let mut db = fx.eq[band];
                let resp = ui.add(egui::Slider::new(&mut db, -EQ_RANGE..=EQ_RANGE).suffix(" dB").text(name));
                self.fx_field(&resp, id, "Égaliseur du son", move |f| f.eq[band] = db);
            }
            ui.separator();
            // Écho.
            ui.label("Écho");
            let mut mix = fx.echo.mix * 100.0;
            let resp = ui.add(egui::Slider::new(&mut mix, 0.0..=100.0).suffix(" %").text("Niveau"));
            self.fx_field(&resp, id, "Écho du son", move |f| f.echo.mix = mix / 100.0);
            let mut delay = fx.echo.delay_ms;
            let resp = ui.add(egui::Slider::new(&mut delay, 20.0..=1500.0).suffix(" ms").text("Délai"));
            self.fx_field(&resp, id, "Écho du son", move |f| f.echo.delay_ms = delay);
            let mut back = fx.echo.feedback * 100.0;
            let resp = ui.add(egui::Slider::new(&mut back, 0.0..=90.0).suffix(" %").text("Retour"));
            self.fx_field(&resp, id, "Écho du son", move |f| f.echo.feedback = back / 100.0);
            ui.separator();
            if ui.add_enabled(!fx.is_default(), egui::Button::new("Retirer tous les effets")).clicked() {
                ui.close_menu();
                self.edit_clip_fx(id, "Retirer les effets du son", |f| *f = ClipFx::default());
            }
        });
    }

    /// Un champ ou un curseur de réglage d'un extrait : une étape d'annulation par geste, nommée.
    fn fx_field(&mut self, resp: &egui::Response, id: u64, name: &str, f: impl FnOnce(&mut ClipFx)) {
        if !resp.changed() {
            if resp.has_focus() || resp.dragged() {
                self.edited(resp);
            }
            return;
        }
        let before = std::mem::replace(&mut self.history.context, Some(name.to_string()));
        let edited = self.edited(resp);
        self.history.context = before;
        if edited {
            if let Some((t, c)) = self.doc.find_clip(id) {
                let fx = &mut self.doc.audio_tracks[t].clips[c].fx;
                f(fx);
                fx.sanitize();
            }
            let _ = crate::audio::mix_clips(&self.doc);
        }
    }
}
