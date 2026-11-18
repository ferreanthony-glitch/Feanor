//! La flèche ou la cible d'un effet de caméra vivant, **dans le cadre** (partie 171, Anthony, 2026-11-18 : « Oui, le lot 2 » ;
//! sur la page de choix, « la caméra glisse sur le décor, dans le sens d'une flèche que tu tires dans l'image », « tu cliques
//! le point d'arrivée, la caméra y va »).
//!
//! Avec l'outil Caméra, l'effet que règle la section « Effets de caméra » (le bloc choisi dans la timeline, sinon le dernier
//! posé) montre sa poignée s'il en a une (`camfx::Preset::poignee`) : la flèche du chemin de la caméra (panoramiques, Ken
//! Burns, véhicule, arrêt net), la flèche d'où elle part (panoramique de découverte) ou la cible qu'elle vise (zooms vers un
//! point, par paliers, de tension, dézoom choc, recadrage sec). Elle se tire comme les poignées du cadre : un geste, une étape
//! d'annulation ; l'effet change aussitôt, sans clé. Le cadre en pointillés montre où la caméra arrive (ou d'où elle part).
//!
//! La poignée se pose dans le cadre de la caméra **au début de l'effet**, sans lui (`Document::camera_avant_l_effet`) : c'est
//! de là que l'effet part. `Params::vers` se compte en part de la largeur du cadre, dans l'image (il tourne avec le cadre).

use eframe::egui::{self, vec2, Color32, Pos2, Rect, Shape, Stroke};

use crate::app::App;
use crate::camera::Camera;
use crate::camfx::{self, Poignee, Vivant};

/// Rayon de prise de la poignée, en points de l'écran.
const PRISE: f32 = 11.0;

/// Une flèche de `de` à `a`, à la pointe de `tete` points quelle que soit sa longueur (celle d'egui grandit avec la flèche).
pub(crate) fn fleche(painter: &egui::Painter, de: Pos2, a: Pos2, trait_: Stroke, tete: f32) {
    let d = a - de;
    let n = d.length();
    if n < 1.0 {
        return;
    }
    let dir = d / n;
    let tete = tete.min(n * 0.6);
    let cote = |angle: f32| {
        let (s, c) = angle.sin_cos();
        a - vec2(dir.x * c - dir.y * s, dir.x * s + dir.y * c) * tete
    };
    let (g, d2) = (cote(0.45), cote(-0.45));
    painter.line_segment([de, a], trait_);
    painter.add(Shape::line(vec![g, a, d2], trait_));
}

impl App {
    /// L'effet vivant du plan courant dont l'outil Caméra montre la poignée : celui que règle la section « Effets de caméra »,
    /// s'il a une flèche ou une cible. Son rang dans la liste du plan, et lui.
    pub(crate) fn effet_a_poignee(&self) -> Option<(usize, Vivant)> {
        let rang = self.rang_effet_vivant()?;
        let v = self.doc.panels.get(self.cur_panel)?.effets_camera.get(rang).copied()?;
        (v.params.preset.poignee() != Poignee::Aucune).then_some((rang, v))
    }

    /// Le cadre dans lequel la poignée de l'effet `rang` se tire : la caméra du plan au début de l'effet, sans lui.
    fn cadre_de_la_poignee(&self, rang: usize, v: &Vivant) -> Camera {
        let derniere = self.frame_count().saturating_sub(1);
        self.doc.camera_avant_l_effet(self.cur_panel, rang, v.start.min(derniere))
    }

    /// Sur la feuille : le centre du cadre `cam` et le point de l'image à `vers` (en part de la largeur du cadre) de ce centre.
    fn points_de_la_poignee(&self, cam: &Camera, vers: [f32; 2]) -> (Pos2, Pos2) {
        let [fw, fh] = self.doc.frame;
        let (cx, cy) = (fw as f32 / 2.0, fh as f32 / 2.0);
        (cam.to_stage(self.doc.frame, cx, cy), cam.to_stage(self.doc.frame, cx + vers[0] * fw as f32, cy + vers[1] * fw as f32))
    }

    fn ecran(&self, p: Pos2) -> Pos2 {
        self.view.to_screen(self.canvas_rect, self.canvas_size(), p)
    }

    /// La poignée de l'effet est sous le pointeur (position à l'écran) : son rang et sa flèche à l'appui.
    pub(crate) fn poignee_effet_at(&self, screen: Pos2) -> Option<(usize, [f32; 2])> {
        let (rang, v) = self.effet_a_poignee()?;
        let cam = self.cadre_de_la_poignee(rang, &v);
        let (_, bout) = self.points_de_la_poignee(&cam, v.params.vers);
        (self.ecran(bout).distance(screen) <= PRISE).then_some((rang, v.params.vers))
    }

    /// Tire la poignée de l'effet `rang` : `vers0` à l'appui en `debut`, le pointeur en `p` (sur la feuille). Maj : la flèche
    /// reste droite ou verticale. L'effet change aussitôt (l'appelant garde l'étape d'annulation de l'appui). Vrai s'il a changé.
    pub(crate) fn tirer_poignee_effet(&mut self, rang: usize, vers0: [f32; 2], debut: Pos2, p: Pos2, shift: bool) -> bool {
        let Some(v) = self.doc.panels.get(self.cur_panel).and_then(|pl| pl.effets_camera.get(rang).copied()) else { return false };
        let cam = self.cadre_de_la_poignee(rang, &v);
        // De la feuille à l'image : on défait la rotation du cadre et on remet le grossissement (caméra tournée : à peu près).
        let d = p - debut;
        let (s, c) = (-cam.angle).to_radians().sin_cos();
        let mag = cam.magnification(self.doc.frame).max(1e-3);
        let fw = self.doc.frame[0].max(1) as f32;
        let d_image = vec2(d.x * c - d.y * s, d.x * s + d.y * c) * mag / fw;
        let mut vers = [vers0[0] + d_image.x, vers0[1] + d_image.y];
        if shift {
            if vers[0].abs() >= vers[1].abs() {
                vers[1] = 0.0;
            } else {
                vers[0] = 0.0;
            }
        }
        let vers = vers.map(|x| x.clamp(-camfx::VERS_MAX, camfx::VERS_MAX));
        if vers == v.params.vers {
            return false;
        }
        let fps = self.doc.fps;
        if let Some(e) = self.doc.panels.get_mut(self.cur_panel).and_then(|pl| pl.effets_camera.get_mut(rang)) {
            e.params.vers = vers;
            *e = e.sanitized(fps);
        }
        // (Le prochain effet repart de ces réglages, comme après un champ de la section.)
        self.cam_fx = camfx::Params { vers, ..v.params };
        self.modified = true;
        let quoi = match v.params.preset.poignee() {
            Poignee::Cible => "cible",
            Poignee::Depart => "départ",
            _ => "flèche",
        };
        self.status = format!(
            "{} : {quoi} à {:+.2} × {:+.2} largeur de cadre du centre. Maj : droite ou verticale ; Ctrl+Z défait le geste.",
            v.params.preset.label(),
            vers[0],
            vers[1]
        );
        true
    }

    /// La poignée de l'effet, avec l'outil Caméra : la flèche (ou la cible), le cadre en pointillés où la caméra arrive (ou
    /// d'où elle part), et la prise ronde au bout.
    pub(crate) fn peindre_poignee_effet(&self, painter: &egui::Painter, rect: Rect) {
        let Some((rang, v)) = self.effet_a_poignee() else { return };
        let size = self.canvas_size();
        let to_s = |p: Pos2| self.view.to_screen(rect, size, p);
        let cam = self.cadre_de_la_poignee(rang, &v);
        let (centre, bout) = self.points_de_la_poignee(&cam, v.params.vers);
        let (c, b) = (to_s(centre), to_s(bout));
        let couleur = crate::theme::camera();
        let trait_ = Stroke::new(2.0_f32, couleur);
        let halo = Stroke::new(4.0_f32, Color32::from_black_alpha(110));
        let poignee = v.params.preset.poignee();
        // Le cadre de l'arrivée (ou du départ) : la caméra centrée sur le bout, avec le zoom de l'effet s'il en a un.
        let zoom = if v.params.preset.a_zoom() && !v.params.preset.est_une_avancee() { v.params.zoom_factor } else { 1.0 };
        let fantome = Camera { x: cam.x + (bout.x - centre.x), y: cam.y + (bout.y - centre.y), zoom: (cam.zoom * zoom).clamp(0.05, 50.0), ..cam };
        let coins: Vec<Pos2> = fantome.corners(self.doc.frame).iter().map(|p| to_s(*p)).collect();
        let mut boucle = coins.clone();
        boucle.push(coins[0]);
        painter.extend(Shape::dashed_line(&boucle, Stroke::new(1.5_f32, couleur), 6.0, 4.0));
        // La flèche : du centre au bout (le chemin), ou du bout au centre (le départ) ; la cible n'a qu'un trait discret.
        let tirer = |de: Pos2, a: Pos2| {
            for s in [halo, trait_] {
                fleche(painter, de, a, s, 14.0);
            }
        };
        match poignee {
            Poignee::Fleche => tirer(c, b),
            Poignee::Depart => tirer(b, c),
            Poignee::Cible => {
                painter.add(Shape::dashed_line(&[c, b], Stroke::new(1.0_f32, couleur), 4.0, 4.0));
                painter.line_segment([b - vec2(9.0, 0.0), b + vec2(9.0, 0.0)], trait_);
                painter.line_segment([b - vec2(0.0, 9.0), b + vec2(0.0, 9.0)], trait_);
            }
            Poignee::Aucune => {}
        }
        painter.circle(b, 6.0, Color32::WHITE, Stroke::new(2.0_f32, couleur));
        let nom = match poignee {
            Poignee::Cible => "Cible",
            Poignee::Depart => "Départ",
            _ => "Flèche",
        };
        // Le nom, sur un fond sombre : il se lit sur n'importe quel dessin.
        let galee = painter.layout_no_wrap(format!("{nom} · {}", v.params.preset.label()), egui::FontId::proportional(11.0), couleur);
        let place = Rect::from_min_size(b + vec2(10.0, -10.0 - galee.size().y), galee.size()).expand(3.0);
        painter.rect_filled(place, 3.0, Color32::from_black_alpha(170));
        painter.galley(place.min + vec2(3.0, 3.0), galee, couleur);
    }
}
