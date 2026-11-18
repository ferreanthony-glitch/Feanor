//! Le panneau Préréglages (partie 170, Anthony, 2026-11-18 : « des préréglages d'animation de caméra à intégrer directement
//! dans la timeline »).
//!
//! Une vignette par préréglage de caméra (`camfx::Preset`) : une petite scène de storyboard vue par la caméra, qui **joue le
//! mouvement du préréglage** quand le pointeur passe dessus (le vrai mouvement, celui du générateur `camfx::Generator`, avec
//! ses réglages d'origine). On la glisse sur la piste Caméra de la timeline (charge `blocscamera::PresetDrag`), ou on clique
//! dessus pour la poser à la tête de lecture : un bloc de plus, vivant, sans clé (`blocscamera.rs`).
//!
//! Les vignettes ne s'animent qu'au survol : un panneau ouvert ne fait pas redessiner FEANOR sans cesse.

use std::sync::OnceLock;

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind};

use crate::app::App;
use crate::blocscamera::PresetDrag;
use crate::camfx::{Generator, Params, Preset};
use crate::theme;

/// Largeur d'une vignette, en points.
const VIGNETTE_W: f32 = 132.0;
/// Le papier et l'encre des vignettes (un dessin de storyboard, quel que soit le thème).
const PAPIER: Color32 = Color32::from_rgb(251, 250, 246);
const ENCRE: Color32 = Color32::from_rgb(30, 30, 34);
/// Pause entre deux passages du mouvement, en secondes.
const PAUSE: f32 = 0.7;

/// Un trait de la petite scène : ses points, en part de la largeur du cadre (x vers la droite, y vers le bas, origine au centre
/// du cadre), fermé ou non, rempli d'une teinte ou non.
struct Trait {
    points: Vec<(f32, f32)>,
    ferme: bool,
    teinte: Option<Color32>,
    /// Rempli jusqu'à cette hauteur, en bandes verticales (une crête de montagnes, qui n'est pas convexe).
    jusqu_a: Option<f32>,
}

/// La petite scène : un sol, des montagnes, une maison, un arbre, un personnage et un rocher devant.
fn scene() -> &'static [Trait] {
    static SCENE: OnceLock<Vec<Trait>> = OnceLock::new();
    SCENE.get_or_init(|| {
        let ligne = |p: &[(f32, f32)]| Trait { points: p.to_vec(), ferme: false, teinte: None, jusqu_a: None };
        let forme = |p: Vec<(f32, f32)>, t: Color32| Trait { points: p, ferme: true, teinte: Some(t), jusqu_a: None };
        let rond = |cx: f32, cy: f32, rx: f32, ry: f32, n: usize| (0..n).map(|i| i as f32 / n as f32 * std::f32::consts::TAU).map(|a| (cx + a.cos() * rx, cy + a.sin() * ry)).collect::<Vec<_>>();
        let crete: Vec<(f32, f32)> = (0..=40).map(|i| -1.6 + i as f32 * 0.08).map(|x| (x, 0.055 - (0.06 + 0.03 * (x * 7.0).sin() + 0.02 * (x * 13.0).cos()))).collect();
        let mut v = vec![Trait { points: crete, ferme: false, teinte: Some(Color32::from_rgb(230, 227, 220)), jusqu_a: Some(0.06) }];
        v.push(ligne(&[(-1.6, 0.06), (1.6, 0.06)]));
        v.push(ligne(&[(0.02, 0.28), (-0.12, 0.12), (-0.24, 0.07)]));
        v.push(ligne(&[(0.14, 0.28), (-0.06, 0.12), (-0.2, 0.07)]));
        v.push(forme(vec![(-0.38, -0.03), (-0.2, -0.03), (-0.2, 0.06), (-0.38, 0.06)], Color32::from_rgb(244, 242, 236)));
        v.push(forme(vec![(-0.4, -0.03), (-0.29, -0.11), (-0.18, -0.03)], Color32::from_rgb(216, 212, 204)));
        v.push(ligne(&[(-0.31, 0.06), (-0.31, 0.02), (-0.27, 0.02), (-0.27, 0.06)]));
        v.push(forme(vec![(0.385, 0.06), (0.39, -0.02), (0.41, -0.02), (0.415, 0.06)], Color32::from_rgb(203, 198, 188)));
        v.push(forme(rond(0.4, -0.075, 0.065, 0.055, 18), Color32::from_rgb(224, 220, 211)));
        v.push(ligne(&[(0.07, 0.0), (0.06, 0.07)]));
        v.push(ligne(&[(0.09, 0.0), (0.1, 0.07)]));
        v.push(forme(vec![(0.055, -0.05), (0.105, -0.05), (0.1, 0.005), (0.06, 0.005)], Color32::from_rgb(185, 180, 170)));
        v.push(forme(rond(0.08, -0.075, 0.022, 0.025, 14), Color32::from_rgb(246, 244, 239)));
        v.push(ligne(&[(0.6, 0.06), (0.6, -0.1), (0.63, -0.1)]));
        v.push(forme(rond(-0.33, 0.24, 0.12, 0.08, 18), Color32::from_rgb(171, 166, 157)));
        v
    })
}

/// La vignette de `preset` à l'instant `t` (secondes depuis le début du passage ; `None` : l'image arrêtée), peinte dans `r`.
fn peindre_vignette(painter: &egui::Painter, r: Rect, preset: Preset, fps: u32, t: Option<f32>) {
    let params = Params::new(preset, fps);
    let g = Generator::new(&params, fps);
    let duree = params.duration.max(1);
    let rel = match t {
        Some(t) => ((t * fps as f32).floor().max(0.0) as usize).min(duree),
        // (À l'arrêt : un instant parlant, aux deux tiers du mouvement.)
        None => duree * 2 / 3,
    };
    let s = g.at(rel);
    let painter = painter.with_clip_rect(r.intersect(painter.clip_rect()));
    painter.rect_filled(r, 2.0, PAPIER);
    let w = r.width();
    let (ca, sa) = ((-s.angle).to_radians().cos(), (-s.angle).to_radians().sin());
    let vers = |(x, y): (f32, f32)| -> Pos2 {
        let (x, y) = ((x - s.dx) * s.zoom, (y - s.dy) * s.zoom);
        let (x, y) = (x * ca - y * sa, x * sa + y * ca);
        pos2(r.center().x + x * w, r.center().y + y * w)
    };
    let trait_w = (w / 110.0).max(1.0);
    for t in scene() {
        let pts: Vec<Pos2> = t.points.iter().copied().map(vers).collect();
        match (t.teinte, t.jusqu_a) {
            (Some(c), Some(bas)) => {
                for w2 in t.points.windows(2) {
                    let (a, b) = (w2[0], w2[1]);
                    painter.add(egui::Shape::convex_polygon(vec![vers(a), vers(b), vers((b.0, bas)), vers((a.0, bas))], c, Stroke::NONE));
                }
            }
            (Some(c), None) => {
                painter.add(egui::Shape::convex_polygon(pts.clone(), c, Stroke::NONE));
            }
            _ => {}
        }
        let stroke = Stroke::new(trait_w, ENCRE);
        if t.ferme {
            painter.add(egui::Shape::closed_line(pts, stroke));
        } else {
            painter.add(egui::Shape::line(pts, stroke));
        }
    }
    // Le bord du cadre, à la couleur de la caméra.
    painter.rect_stroke(r, 2.0, Stroke::new(2.0_f32, theme::camera()), StrokeKind::Inside);
}

impl App {
    /// Le panneau Préréglages.
    pub(crate) fn prereglages_panel(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Glisse un préréglage sur la piste Caméra de la timeline, ou clique dessus pour le poser à la tête de lecture.").small().weak());
        ui.add_space(4.0);
        ui.label(egui::RichText::new("CAMÉRA").small().strong());
        let fps = self.doc.fps;
        let maintenant = ui.input(|i| i.time) as f32;
        let mut pose = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 10.0);
            for pr in Preset::ALL {
                let h = VIGNETTE_W * 9.0 / 16.0;
                let (rect, resp) = ui.allocate_exact_size(vec2(VIGNETTE_W, h + 18.0), Sense::click_and_drag());
                let img = Rect::from_min_size(rect.min, vec2(VIGNETTE_W, h));
                let id = resp.id.with("survol");
                // Au survol, le mouvement joue en boucle depuis le moment où le pointeur est arrivé.
                let t = if resp.hovered() && !resp.dragged() {
                    let debut = ui.ctx().data_mut(|d| *d.get_temp_mut_or_insert_with(id, || maintenant));
                    let duree = Params::new(pr, fps).duration.max(1) as f32 / fps.max(1) as f32;
                    ui.ctx().request_repaint();
                    Some((maintenant - debut) % (duree + PAUSE))
                } else {
                    ui.ctx().data_mut(|d| d.remove::<f32>(id));
                    None
                };
                if ui.is_rect_visible(rect) {
                    peindre_vignette(ui.painter(), img, pr, fps, t);
                    if resp.hovered() {
                        ui.painter().rect_stroke(img.expand(1.5), 3.0, Stroke::new(1.5_f32, theme::contour()), StrokeKind::Outside);
                    }
                    ui.painter().text(pos2(rect.left() + 2.0, img.bottom() + 9.0), Align2::LEFT_CENTER, pr.label(), FontId::proportional(11.5), ui.visuals().text_color());
                }
                if resp.drag_started() {
                    egui::DragAndDrop::set_payload(ui.ctx(), PresetDrag { preset: pr });
                }
                let resp = crate::bulles::bulle(resp, pr.label(), &format!("{} Glisse-le sur la piste Caméra de la timeline, ou clique pour le poser à la tête de lecture.", pr.note()), "");
                if resp.clicked() {
                    pose = Some(pr);
                }
            }
        });
        if let Some(pr) = pose {
            self.ajouter_effet_camera_a_la_tete(pr);
        }
        // Le préréglage glissé suit le pointeur.
        if let Some(drag) = egui::DragAndDrop::payload::<PresetDrag>(ui.ctx()) {
            if let Some(p) = ui.ctx().pointer_latest_pos() {
                egui::Area::new(egui::Id::new("prereglage_glisse")).order(egui::Order::Tooltip).fixed_pos(p + vec2(14.0, 10.0)).interactable(false).show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label(format!("{} {}", egui_phosphor::regular::VIDEO_CAMERA, drag.preset.label()));
                    });
                });
            }
        }
    }
}
