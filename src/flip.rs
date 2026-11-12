//! Retourner (Anthony, 2026-10-18 : « créer la possibilité de flipper un plan, flipper un calque et
//! flipper une sélection »). La sélection : `selection.miroir_h` / `miroir_v` (Alt+H, Alt+V). Ici :
//! - un calque, de gauche à droite ou de haut en bas, sur place (autour du centre de son contenu),
//!   par la transformation du calque (étirement négatif) : ses dessins ne changent pas ;
//! - les plans choisis, de gauche à droite autour du milieu du cadre (sans touche par défaut) : tous leurs calques et leur caméra, sauf les calques de toute la scène,
//!   partagés avec les autres plans de la scène.

use eframe::egui::{pos2, Pos2};

use crate::app::App;
use crate::camera::Camera;
use crate::document::LayerTransform;

/// La transformation `t` (pivot : centre de la feuille) suivie d'un miroir autour de la verticale
/// x = `axis` de la feuille : la rotation s'inverse, la largeur change de signe, et l'inclinaison d'une carte autour de
/// la verticale (Espace 3D) aussi (celle autour de l'horizontale ne change pas).
pub fn mirror_transform_x(t: LayerTransform, pivot: Pos2, axis: f32) -> LayerTransform {
    let p = t.affine(pivot).apply(pivot);
    let target = pos2(2.0 * axis - p.x, p.y);
    let t1 = LayerTransform { angle: -t.angle, stretch_x: -t.stretch_x, tilt_y: -t.tilt_y, ..t };
    let q = t1.affine(pivot).apply(pivot);
    LayerTransform { x: t1.x + target.x - q.x, y: t1.y + target.y - q.y, ..t1 }
}

/// Comme `mirror_transform_x`, autour de l'horizontale y = `axis` de la feuille : la rotation s'inverse, la hauteur change de signe,
/// et l'inclinaison d'une carte autour de l'horizontale aussi (`choixtout.rs`, plusieurs calques retournés ensemble).
pub fn mirror_transform_y(t: LayerTransform, pivot: Pos2, axis: f32) -> LayerTransform {
    let p = t.affine(pivot).apply(pivot);
    let target = pos2(p.x, 2.0 * axis - p.y);
    let t1 = LayerTransform { angle: -t.angle, stretch_y: -t.stretch_y, tilt_x: -t.tilt_x, ..t };
    let q = t1.affine(pivot).apply(pivot);
    LayerTransform { x: t1.x + target.x - q.x, y: t1.y + target.y - q.y, ..t1 }
}

/// La transformation `t` retournée sur place : le point `center` du dessin (le milieu de son
/// contenu) reste où il est sur la feuille ; de gauche à droite (`horizontal`) ou de haut en bas,
/// dans le repère du calque.
pub fn flipped_in_place(t: LayerTransform, pivot: Pos2, center: Pos2, horizontal: bool) -> LayerTransform {
    let before = t.affine(pivot).apply(center);
    let t1 = if horizontal { LayerTransform { stretch_x: -t.stretch_x, ..t } } else { LayerTransform { stretch_y: -t.stretch_y, ..t } };
    let after = t1.affine(pivot).apply(center);
    LayerTransform { x: t1.x + before.x - after.x, y: t1.y + before.y - after.y, ..t1 }
}

/// Une position de caméra retournée autour de la verticale x = `axis` : elle regarde l'image
/// retournée comme elle regardait l'originale.
pub fn mirror_camera_x(c: Camera, axis: f32) -> Camera {
    Camera { x: 2.0 * axis - c.x, angle: -c.angle, pan: -c.pan, ..c }
}

impl App {
    /// Retourne le calque courant (`horizontal` : de gauche à droite), sur place.
    pub(crate) fn flip_layer(&mut self, horizontal: bool) {
        if self.layer().position_locked {
            self.status = crate::app::POSITION_LOCKED.into();
            return;
        }
        if self.content_refuses() || !self.layer_editable() {
            return;
        }
        let center = self.layer_content_box(self.cur_layer, self.cur_frame).unwrap_or_else(|| self.doc.frame_rect()).center();
        let pivot = self.doc.stage_center();
        self.end_stroke();
        self.record_step();
        let li = self.cur_layer;
        // (Un calque animé se retourne à toutes ses clés : le mouvement est retourné avec lui.)
        let l = &mut self.layers_mut()[li];
        l.transform = flipped_in_place(l.transform, pivot, center, horizontal);
        for k in &mut l.pos_keys {
            k.t = flipped_in_place(k.t, pivot, center, horizontal);
        }
        // (Un chemin tracé dans l'espace n'est pas sur la feuille : il reste où il est, et les clés de la carte retournée sont
        // reposées d'après lui, partie 149.)
        let dans_l_espace = l.trajectoire.as_ref().is_some_and(|t| !t.sur_la_feuille());
        if let Some(t) = l.trajectoire.as_mut().filter(|t| t.sur_la_feuille()) {
            t.retourner(horizontal);
        }
        if dans_l_espace {
            self.trajectoire_recalculer(li);
        }
        self.renderer.invalidate_all();
        self.status = format!("Calque retourné {} (ses dessins ne changent pas ; Ctrl+R annule la transformation).", if horizontal { "de gauche à droite" } else { "de haut en bas" });
    }

    /// Retourne de gauche à droite les plans choisis (sinon le plan courant) : leurs calques et leur
    /// caméra, autour du milieu du cadre.
    pub(crate) fn flip_panels(&mut self) {
        if self.storyboard_refuses() {
            return;
        }
        let ids = self.board_selection();
        let targets: Vec<usize> = if ids.is_empty() {
            vec![self.cur_panel]
        } else {
            self.doc.panels.iter().enumerate().filter(|(_, p)| ids.contains(&p.id)).map(|(i, _)| i).collect()
        };
        self.end_stroke();
        self.drop_bitmap_selection();
        self.record_step();
        let (pivot, axis) = (self.doc.stage_center(), self.doc.frame_rect().center().x);
        // (Les chemins tracés dans l'espace, partie 149 : leur plan est vu dans le même miroir, dressé au milieu du cadre.)
        let world = crate::espace3d::Monde::of(&self.doc);
        let miroir = (axis - world.stage[0] / 2.0) * world.k;
        let mut shared = 0;
        let mut dans_l_espace: Vec<(usize, usize)> = Vec::new();
        for &pi in &targets {
            let p = &mut self.doc.panels[pi];
            for (li, l) in p.layers.iter_mut().enumerate() {
                if l.scene_wide.is_some() {
                    shared += 1;
                    continue;
                }
                l.transform = mirror_transform_x(l.transform, pivot, axis);
                for k in &mut l.pos_keys {
                    k.t = mirror_transform_x(k.t, pivot, axis);
                }
                if let Some(t) = &mut l.trajectoire {
                    match t.support {
                        Some(s) => {
                            t.support = Some(s.miroir_x(miroir));
                            dans_l_espace.push((pi, li));
                        }
                        None => t.retourner(true),
                    }
                }
            }
            for k in &mut p.camera {
                k.cam = mirror_camera_x(k.cam, axis);
            }
            if let Some(t) = p.trajectoire_camera.as_mut() {
                t.support = t.support.map(|s| s.miroir_x(miroir));
            }
        }
        for (pi, li) in dans_l_espace {
            self.trajectoire_recalculer_dans(pi, li);
        }
        self.renderer.invalidate_all();
        let n = targets.len();
        self.status = format!(
            "{} retourné{} de gauche à droite{}.",
            if n > 1 { format!("{n} plans") } else { "Plan".to_string() },
            if n > 1 { "s" } else { "" },
            if shared > 0 { " (pas les calques de toute la scène, partagés avec les autres plans)" } else { "" }
        );
    }
}
