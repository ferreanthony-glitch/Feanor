//! Le panneau Préréglages (partie 170, Anthony, 2026-11-18 : « des préréglages d'animation de caméra à intégrer directement
//! dans la timeline »).
//!
//! Une vignette par préréglage de caméra (`camfx::Preset`) : une petite scène de storyboard vue par la caméra, qui **joue le
//! mouvement du préréglage** quand le pointeur passe dessus (le vrai mouvement, celui du générateur `camfx::Generator`, avec
//! ses réglages d'origine). On la glisse sur la piste Caméra de la timeline (charge `blocscamera::PresetDrag`), ou on clique
//! dessus pour la poser à la tête de lecture : un bloc de plus, vivant, sans clé (`blocscamera.rs`).
//!
//! Les vignettes ne s'animent qu'au survol : un panneau ouvert ne fait pas redessiner FEANOR sans cesse.
//!
//! Depuis le lot 2 (partie 171), les vignettes sont rangées par famille (`camfx::Famille`), et la petite scène a de la
//! profondeur : un travelling y fait passer le rocher vite et les montagnes à peine. Une cible y vise le visage du
//! personnage (la maison pour le zoom vers un point), comme sur la page de choix.
//!
//! Depuis le lot 3 (partie 172), la petite scène est **filmée par la vraie caméra** de l'effet (`camfx::cuire_vite` sur une
//! caméra de base, puis `camespace::image_de`) : un panoramique, une orbite, une grue ou un survol s'y voient en perspective,
//! chaque trait à sa profondeur. Les trajectoires toutes faites (`trajformes`) ont leur famille, à la fin.

use std::sync::OnceLock;

use eframe::egui::{self, pos2, vec2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind};

use crate::app::App;
use crate::blocscamera::PresetDrag;
use crate::camera::{self, Camera};
use crate::camespace;
use crate::camfx::{self, Params, Preset, Vivant};
use crate::theme;
use crate::trajformes::{self, Forme, FormeDrag};

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
    /// Profondeur : la distance à la caméra, en part de celle du plan de référence, moins 1 (0 : au plan de référence ; 4 : cinq
    /// fois plus loin ; négative : plus près). Un travelling fait grandir chacun à sa vitesse.
    profondeur: f32,
}

/// Les profondeurs des traits de la scène, dans l'ordre de `scene` : les montagnes et l'horizon au loin, la route, la maison,
/// l'arbre, le personnage au plan de référence, le poteau, le rocher devant.
const PROFONDEURS: [f32; 15] = [4.0, 4.0, 0.4, 0.4, 0.8, 0.8, 0.8, 0.6, 0.6, 0.0, 0.0, 0.0, 0.0, 0.3, -0.5];

/// Le visage du personnage et la maison, là où visent les cibles des vignettes (en part de la largeur du cadre).
const VISAGE: [f32; 2] = [0.08, -0.075];
const MAISON: [f32; 2] = [-0.29, -0.01];

/// La petite scène : un sol, des montagnes, une maison, un arbre, un personnage et un rocher devant.
fn scene() -> &'static [Trait] {
    static SCENE: OnceLock<Vec<Trait>> = OnceLock::new();
    SCENE.get_or_init(|| {
        let ligne = |p: &[(f32, f32)]| Trait { points: p.to_vec(), ferme: false, teinte: None, jusqu_a: None, profondeur: 0.0 };
        let forme = |p: Vec<(f32, f32)>, t: Color32| Trait { points: p, ferme: true, teinte: Some(t), jusqu_a: None, profondeur: 0.0 };
        let rond = |cx: f32, cy: f32, rx: f32, ry: f32, n: usize| (0..n).map(|i| i as f32 / n as f32 * std::f32::consts::TAU).map(|a| (cx + a.cos() * rx, cy + a.sin() * ry)).collect::<Vec<_>>();
        let crete: Vec<(f32, f32)> = (0..=40).map(|i| -1.6 + i as f32 * 0.08).map(|x| (x, 0.055 - (0.06 + 0.03 * (x * 7.0).sin() + 0.02 * (x * 13.0).cos()))).collect();
        let mut v = vec![Trait { points: crete, ferme: false, teinte: Some(Color32::from_rgb(230, 227, 220)), jusqu_a: Some(0.06), profondeur: 0.0 }];
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
        for (t, z) in v.iter_mut().zip(PROFONDEURS) {
            t.profondeur = z;
        }
        v
    })
}

/// Les réglages que joue la vignette de `preset` : ceux d'origine, la cible sur le visage (ou la maison).
fn reglages_de_vignette(preset: Preset, fps: u32) -> Params {
    let p = Params::new(preset, fps);
    match (preset.poignee(), preset) {
        (_, Preset::ZoomPoint) => Params { vers: MAISON, ..p },
        (crate::camfx::Poignee::Cible, _) => Params { vers: VISAGE, ..p },
        _ => p,
    }
}

/// Le cadre des vignettes : une image de 1600 × 900 pixels, filmée d'une feuille de la même taille (partie 172 : la petite
/// scène est vue par une vraie caméra, celle que l'effet donne, comme dans le plan).
const CADRE: [usize; 2] = [1600, 900];

/// La caméra des vignettes au repos : au centre de la feuille, de face.
fn camera_de_base() -> Camera {
    Camera::centered(CADRE[0], CADRE[1])
}

/// Le point de l'espace (`camespace`, pixels de la feuille) du point (`x`, `y`) d'un trait à la profondeur `z` : posé comme
/// un calque en profondeur, que la caméra de base voit tel qu'il est dessiné.
fn point_de_la_scene(x: f32, y: f32, z: f32) -> camespace::V3 {
    let (w, h) = (CADRE[0] as f32, CADRE[1] as f32);
    let d0 = camera::ref_distance(CADRE);
    [w / 2.0 + x * w * (1.0 + z), -(h / 2.0) - y * w * (1.0 + z), -d0 * z]
}

/// La caméra de la vignette de `preset` à l'image `rel` de son mouvement : celle que montrerait l'effet vivant posé sur un plan
/// à la caméra de base (`camfx::cuire_vite`).
fn camera_de_vignette(preset: Preset, fps: u32, rel: usize) -> Camera {
    let params = reglages_de_vignette(preset, fps);
    let base = camera_de_base();
    let v = Vivant { params, start: 0 };
    let keys = camfx::cuire_vite(&v, fps, CADRE, params.duration + 1, &[], &|_| base).unwrap_or_default();
    camera::camera_at(&keys, rel, base)
}

/// La caméra de la vignette de `forme` à l'avancement `t` (0 à 1) de son chemin : elle part de la caméra de base et garde le
/// visage du personnage à sa place, comme une trajectoire toute faite garde son sujet (`trajformes`).
fn camera_de_forme(forme: Forme, t: f32) -> Camera {
    let base = camera_de_base();
    let d0 = camera::ref_distance(CADRE);
    let (fw, fh) = (CADRE[0] as f32, CADRE[1] as f32);
    let (u, v) = (fw / 2.0 + VISAGE[0] * fw, fh / 2.0 + VISAGE[1] * fw);
    let sujet = camespace::point_vu(&base, CADRE, u, v);
    let oeil = camespace::oeil(&base, d0);
    let (dx, dz) = (sujet[0] - oeil[0], sujet[2] - oeil[2]);
    let e = dx.hypot(dz).max(1.0);
    let devant = [dx / e, 0.0, dz / e];
    let droite = [-devant[2], 0.0, devant[0]];
    let p = forme.point(camera::ease(t, 1.0, 0.5, 0.5));
    let d = [0, 1, 2].map(|i| (p[0] * droite[i] + p[1] * devant[i]) * e + if i == 1 { p[2] * e } else { 0.0 });
    let portee = camespace::deplacer(&base, CADRE, d);
    camespace::viser(&portee, CADRE, sujet, u, v).unwrap_or(portee)
}

/// La petite scène filmée par `cam`, peinte dans `r`.
fn peindre_scene(painter: &egui::Painter, r: Rect, cam: &Camera) {
    let painter = painter.with_clip_rect(r.intersect(painter.clip_rect()));
    painter.rect_filled(r, 2.0, PAPIER);
    let w = r.width();
    let (cx, cy) = (CADRE[0] as f32 / 2.0, CADRE[1] as f32 / 2.0);
    let echelle = w / CADRE[0] as f32;
    // (Un point derrière la caméra, ou si près d'elle qu'il partirait à l'infini, n'est pas dessiné.)
    let ecran = |x: f32, y: f32, z: f32| -> Option<Pos2> {
        let [u, v] = camespace::image_de(cam, CADRE, point_de_la_scene(x, y, z))?;
        let q = pos2(r.center().x + (u - cx) * echelle, r.center().y + (v - cy) * echelle);
        (q.x.abs() < 1e5 && q.y.abs() < 1e5).then_some(q)
    };
    let stroke = Stroke::new((w / 110.0).max(1.0), ENCRE);
    for t in scene() {
        let z = t.profondeur;
        let pts: Vec<Option<Pos2>> = t.points.iter().map(|&(x, y)| ecran(x, y, z)).collect();
        match (t.teinte, t.jusqu_a) {
            (Some(c), Some(bas)) => {
                for w2 in t.points.windows(2) {
                    let (a, b) = (w2[0], w2[1]);
                    if let (Some(p1), Some(p2), Some(p3), Some(p4)) = (ecran(a.0, a.1, z), ecran(b.0, b.1, z), ecran(b.0, bas, z), ecran(a.0, bas, z)) {
                        painter.add(egui::Shape::convex_polygon(vec![p1, p2, p3, p4], c, Stroke::NONE));
                    }
                }
            }
            (Some(c), None) => {
                if let Some(tous) = pts.iter().copied().collect::<Option<Vec<Pos2>>>() {
                    painter.add(egui::Shape::convex_polygon(tous, c, Stroke::NONE));
                }
            }
            _ => {}
        }
        if t.ferme && pts.iter().all(|p| p.is_some()) {
            painter.add(egui::Shape::closed_line(pts.iter().flatten().copied().collect(), stroke));
            continue;
        }
        // Le trait, en morceaux là où il passe derrière la caméra.
        let mut morceau: Vec<Pos2> = Vec::new();
        let boucle = if t.ferme { pts.first() } else { None };
        for p in pts.iter().chain(boucle) {
            match p {
                Some(q) => morceau.push(*q),
                None => {
                    if morceau.len() >= 2 {
                        painter.add(egui::Shape::line(std::mem::take(&mut morceau), stroke));
                    }
                    morceau.clear();
                }
            }
        }
        if morceau.len() >= 2 {
            painter.add(egui::Shape::line(morceau, stroke));
        }
    }
    // Le bord du cadre, à la couleur de la caméra.
    painter.rect_stroke(r, 2.0, Stroke::new(2.0_f32, theme::camera()), StrokeKind::Inside);
}

/// Une vignette du panneau : un préréglage d'effet de caméra, ou une trajectoire toute faite (partie 172).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Tuile {
    Effet(Preset),
    Forme(Forme),
}

impl Tuile {
    fn label(self) -> &'static str {
        match self {
            Tuile::Effet(p) => p.label(),
            Tuile::Forme(f) => f.label(),
        }
    }

    fn note(self) -> String {
        match self {
            Tuile::Effet(p) => format!("{} Glisse-le sur la piste Caméra de la timeline, ou clique pour le poser à la tête de lecture.", p.note()),
            Tuile::Forme(f) => format!(
                "{} Glisse-la sur la piste Caméra de la timeline, ou clique pour la poser à la tête de lecture : elle devient le chemin de la caméra du plan, que l'outil Trajectoire retouche dans la vue Espace 3D (poignées, flèches x, y, z).",
                f.note()
            ),
        }
    }

    /// La durée de son mouvement, en images.
    fn duree(self, fps: u32) -> usize {
        match self {
            Tuile::Effet(p) => Params::new(p, fps).duration.max(1),
            Tuile::Forme(_) => ((trajformes::SECONDES * fps.max(1) as f32).round() as usize).max(1),
        }
    }

    /// Peinte dans `r` à l'image `rel` de son mouvement.
    fn peindre(self, painter: &egui::Painter, r: Rect, fps: u32, rel: usize) {
        let cam = match self {
            Tuile::Effet(p) => camera_de_vignette(p, fps, rel),
            Tuile::Forme(f) => camera_de_forme(f, rel as f32 / self.duree(fps) as f32),
        };
        peindre_scene(painter, r, &cam);
    }
}

/// Les vignettes de `tuiles`, côte à côte : chacune joue son mouvement au survol, se glisse vers la timeline (charge
/// `PresetDrag` ou `FormeDrag`) ; rend celle qui est cliquée.
fn vignettes(ui: &mut egui::Ui, tuiles: &[Tuile], fps: u32, maintenant: f32) -> Option<Tuile> {
    let mut pose = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 10.0);
        for &tuile in tuiles {
            let h = VIGNETTE_W * 9.0 / 16.0;
            // (Sous l'image, le nom sur deux lignes au plus : « Caméra à l'épaule (found footage) » ne déborde pas sur la voisine.)
            let (rect, resp) = ui.allocate_exact_size(vec2(VIGNETTE_W, h + 30.0), Sense::click_and_drag());
            let img = Rect::from_min_size(rect.min, vec2(VIGNETTE_W, h));
            let id = resp.id.with("survol");
            let duree = tuile.duree(fps);
            // Au survol, le mouvement joue en boucle depuis le moment où le pointeur est arrivé.
            let rel = if resp.hovered() && !resp.dragged() {
                let debut = ui.ctx().data_mut(|d| *d.get_temp_mut_or_insert_with(id, || maintenant));
                let secondes = duree as f32 / fps.max(1) as f32;
                ui.ctx().request_repaint();
                let t = (maintenant - debut) % (secondes + PAUSE);
                ((t * fps as f32).floor().max(0.0) as usize).min(duree)
            } else {
                ui.ctx().data_mut(|d| d.remove::<f32>(id));
                // (À l'arrêt : un instant parlant, aux deux tiers du mouvement.)
                duree * 2 / 3
            };
            if ui.is_rect_visible(rect) {
                tuile.peindre(ui.painter(), img, fps, rel);
                if resp.hovered() {
                    ui.painter().rect_stroke(img.expand(1.5), 3.0, Stroke::new(1.5_f32, theme::contour()), StrokeKind::Outside);
                }
                let nom = ui.painter().layout(tuile.label().to_string(), FontId::proportional(11.0), ui.visuals().text_color(), VIGNETTE_W - 4.0);
                ui.painter().with_clip_rect(rect).galley(pos2(rect.left() + 2.0, img.bottom() + 3.0), nom, ui.visuals().text_color());
            }
            if resp.drag_started() {
                match tuile {
                    Tuile::Effet(preset) => egui::DragAndDrop::set_payload(ui.ctx(), PresetDrag { preset }),
                    Tuile::Forme(forme) => egui::DragAndDrop::set_payload(ui.ctx(), FormeDrag { forme }),
                }
            }
            let resp = crate::bulles::bulle(resp, tuile.label(), &tuile.note(), "");
            if resp.clicked() {
                pose = Some(tuile);
            }
        }
    });
    pose
}

impl App {
    /// Le panneau Préréglages.
    pub(crate) fn prereglages_panel(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Glisse un préréglage sur la piste Caméra de la timeline, ou clique dessus pour le poser à la tête de lecture.").small().weak());
        let fps = self.doc.fps;
        let maintenant = ui.input(|i| i.time) as f32;
        let mut pose = None;
        // Une famille après l'autre (partie 171), sous le titre de la caméra.
        for famille in crate::camfx::Famille::ALL {
            ui.add_space(6.0);
            ui.label(egui::RichText::new(format!("CAMÉRA · {}", famille.label().to_uppercase())).small().strong());
            let tuiles: Vec<Tuile> = famille.presets().map(Tuile::Effet).collect();
            pose = vignettes(ui, &tuiles, fps, maintenant).or(pose);
        }
        // Les trajectoires toutes faites (C17, partie 172) : le chemin de la caméra du plan, à retoucher dans la vue Espace 3D.
        ui.add_space(6.0);
        ui.label(egui::RichText::new("CAMÉRA · TRAJECTOIRES").small().strong());
        let formes: Vec<Tuile> = Forme::ALL.into_iter().map(Tuile::Forme).collect();
        pose = vignettes(ui, &formes, fps, maintenant).or(pose);
        match pose {
            Some(Tuile::Effet(pr)) => self.ajouter_effet_camera_a_la_tete(pr),
            Some(Tuile::Forme(f)) => self.poser_trajectoire_toute_faite_a_la_tete(f),
            None => {}
        }
        // Le préréglage glissé suit le pointeur.
        let glisse = egui::DragAndDrop::payload::<PresetDrag>(ui.ctx()).map(|d| d.preset.label()).or_else(|| egui::DragAndDrop::payload::<FormeDrag>(ui.ctx()).map(|d| d.forme.label()));
        if let Some(nom) = glisse {
            if let Some(p) = ui.ctx().pointer_latest_pos() {
                egui::Area::new(egui::Id::new("prereglage_glisse")).order(egui::Order::Tooltip).fixed_pos(p + vec2(14.0, 10.0)).interactable(false).show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label(format!("{} {nom}", egui_phosphor::regular::VIDEO_CAMERA));
                    });
                });
            }
        }
    }
}
