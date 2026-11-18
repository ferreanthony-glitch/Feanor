//! **Les trajectoires toutes faites de la caméra** (C17, partie 172 ; Anthony, 2026-11-18 : « Oui, le lot 3 » ; sur la page de
//! choix : « Droite, arc, spirale, S : posées, puis retouchées avec les poignées »).
//!
//! Une forme posée depuis le panneau Préréglages (glissée sur la piste Caméra de la timeline, ou cliquée : à la tête de lecture)
//! devient **la trajectoire de la caméra du plan** (`Panel::trajectoire_camera`, partie 149) : un vrai chemin dans l'espace,
//! posé à plat à la hauteur de l'œil, qui se retouche ensuite avec l'outil Trajectoire dans la vue Espace 3D (ses poignées,
//! ses flèches x, y, z, ses marques, son lissage), comme un chemin tracé à la main. Il remplace la trajectoire d'avant s'il y
//! en avait une (une étape d'annulation).
//!
//! La caméra y **garde son sujet dans le cadre** (`Trajectoire::vise`) : le point du plan de référence qu'elle voit au centre
//! du cadre à la tête de lecture. Elle tourne en avançant pour le voir toujours au même endroit. Cinq marques, plus serrées
//! aux deux bouts, la font partir et arriver doucement (le temps se partage en parts égales entre les jalons).
//!
//! Le chemin se mesure à la distance du sujet (`chemin`) : un arc tourne d'un sixième de tour autour de lui, une spirale se
//! resserre en montant, un S avance vers lui en ondulant, la ligne droite passe de côté.

use crate::app::App;
use crate::camera::{self, Camera};
use crate::camespace;
use crate::trajectoire::{Cles, Support, Trajectoire};

/// Une trajectoire toute faite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Forme {
    Droite,
    Arc,
    Spirale,
    S,
}

/// Une forme glissée du panneau Préréglages vers la timeline (la charge du glisser-déposer d'egui).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FormeDrag {
    pub forme: Forme,
}

/// Les marques posées sur une trajectoire toute faite : plus serrées aux deux bouts, la caméra y part et arrive doucement.
pub const MARQUES: [f32; 5] = [0.074, 0.259, 0.5, 0.741, 0.926];

/// Durée proposée, en secondes (le chemin va de la tête de lecture jusque-là, ou jusqu'à la fin du plan).
pub const SECONDES: f32 = 4.0;

impl Forme {
    pub const ALL: [Forme; 4] = [Forme::Droite, Forme::Arc, Forme::Spirale, Forme::S];

    pub fn label(self) -> &'static str {
        match self {
            Forme::Droite => "Ligne droite",
            Forme::Arc => "Arc",
            Forme::Spirale => "Spirale",
            Forme::S => "En S",
        }
    }

    /// Ce que fait la forme, en une phrase (bulle).
    pub fn note(self) -> &'static str {
        match self {
            Forme::Droite => "La caméra file de côté sur une ligne droite, en gardant le sujet dans le cadre.",
            Forme::Arc => "La caméra tourne d'un sixième de tour autour du sujet, en le gardant dans le cadre.",
            Forme::Spirale => "La caméra tourne autour du sujet en se rapprochant et en montant.",
            Forme::S => "La caméra avance vers le sujet en ondulant de gauche à droite.",
        }
    }

    /// Le point du chemin à l'avancement `t` (0 à 1), dans son plan : (vers la droite, vers le sujet, vers le haut), en
    /// distances au sujet (`e` = 1).
    pub fn point(self, t: f32) -> [f32; 3] {
        let t = t.clamp(0.0, 1.0);
        match self {
            Forme::Droite => [0.8 * t, 0.0, 0.0],
            Forme::Arc => {
                let a = t * 60f32.to_radians();
                [a.sin(), 1.0 - a.cos(), 0.0]
            }
            Forme::Spirale => {
                let a = t * 150f32.to_radians();
                let r = 1.0 - 0.45 * t;
                [r * a.sin(), 1.0 - r * a.cos(), 0.5 * t]
            }
            Forme::S => [0.3 * (t * std::f32::consts::TAU).sin(), 0.6 * t, 0.0],
        }
    }

    /// Le chemin, en unités du chemin (`e` : la distance au sujet, dans ces unités) : ses points dans le plan, et la hauteur de
    /// chacun.
    pub fn chemin(self, e: f32) -> (Vec<[f32; 2]>, Vec<f32>) {
        let n = 96;
        let pts: Vec<[f32; 3]> = (0..=n).map(|i| self.point(i as f32 / n as f32)).map(|p| [p[0] * e, p[1] * e, p[2] * e]).collect();
        (pts.iter().map(|p| [p[0], p[1]]).collect(), pts.iter().map(|p| p[2]).collect())
    }
}

/// La trajectoire toute faite `forme` d'une caméra `cam` (celle du plan, sans chemin) dans un plan au cadre `frame`, sur une
/// feuille de `stage` pixels : posée à plat à la hauteur de l'œil (en mètres, `k` mètres par pixel de la feuille), vers le sujet
/// (le point du plan de référence au centre du cadre), qu'elle vise. Ni départ ni fin posés. Rien si le sujet est sur l'œil.
pub fn trajectoire(forme: Forme, cam: &Camera, frame: [usize; 2], stage: [f32; 2], k: f32) -> Option<Trajectoire> {
    let d0 = camera::ref_distance(frame);
    let oeil = camespace::oeil(cam, d0);
    let sujet = camespace::point_vu(cam, frame, frame[0] as f32 / 2.0, frame[1] as f32 / 2.0);
    // Vers le sujet, à plat ; sa droite.
    let (dx, dz) = (sujet[0] - oeil[0], sujet[2] - oeil[2]);
    let e = dx.hypot(dz);
    if !(e > 1e-3 * d0) {
        return None;
    }
    let v = [dx / e, 0.0, dz / e];
    let u = [-v[2], 0.0, v[0]];
    // (De la feuille aux mètres de l'espace, comme `espace3d::Monde::eye`.)
    let en_metres = |p: camespace::V3| [(p[0] - stage[0] / 2.0) * k, (p[1] + stage[1] / 2.0) * k, p[2] * k];
    let support = Support::nouveau(en_metres(oeil), u, v, k)?;
    let (points, hauteurs) = forme.chemin(e);
    let mut t = Trajectoire::nouvelle_dans_l_espace(&points, support)?;
    // Les hauteurs suivent la longueur du chemin (il a été repris au pas) : celle de la forme à la même part de sa longueur.
    if hauteurs.iter().any(|h| *h != 0.0) {
        let longueurs = |p: &[[f32; 2]]| {
            let mut l = vec![0.0f32];
            for w in p.windows(2) {
                l.push(l.last().unwrap() + (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]));
            }
            l
        };
        let (lf, lt) = (longueurs(&points), longueurs(&t.points));
        let (totf, tott) = (lf.last().copied().unwrap_or(1.0).max(1e-6), lt.last().copied().unwrap_or(1.0).max(1e-6));
        t.hauteurs = lt
            .iter()
            .map(|s| {
                let x = s / tott * totf;
                let i = lf.partition_point(|l| *l < x).clamp(1, lf.len() - 1);
                let (a, b) = (lf[i - 1], lf[i]);
                let w = if b > a { (x - a) / (b - a) } else { 0.0 };
                hauteurs[i - 1] + (hauteurs[i] - hauteurs[i - 1]) * w
            })
            .collect();
    }
    for m in MARQUES {
        t.ajouter_marque(m);
    }
    t.cles = Cles::Interpolation;
    t.vise = Some(en_metres(sujet));
    Some(t)
}

impl App {
    /// Pose la trajectoire toute faite `forme` pour la caméra du plan courant, depuis l'image `frame` (jusqu'à `SECONDES` plus
    /// loin, ou la fin du plan). Elle remplace celle d'avant. Une étape d'annulation.
    pub(crate) fn poser_trajectoire_toute_faite(&mut self, forme: Forme, frame: usize) {
        let pi = self.cur_panel;
        let Some(len) = self.doc.panels.get(pi).map(|p| p.frame_count().max(1)) else { return };
        let frame = frame.min(len - 1);
        if len < frame + 2 {
            self.status = format!("{} : il faut au moins une image après la tête de lecture pour que la caméra bouge.", forme.label());
            return;
        }
        let cam = self.doc.camera_sans_trajectoire(pi, frame);
        let m = crate::espace3d::Monde::of(&self.doc);
        let Some(mut t) = trajectoire(forme, &cam, self.doc.frame, m.stage, m.k) else {
            self.status = format!("{} : la caméra est sur son sujet, le chemin n'a pas de place.", forme.label());
            return;
        };
        let duree = ((SECONDES * self.doc.fps.max(1) as f32).round() as usize).max(1);
        t.depart = frame;
        t.fin = Some((frame + duree).min(len - 1));
        let remplace = self.doc.panels[pi].trajectoire_camera.is_some();
        self.end_stroke();
        self.record_named_step(&format!("Trajectoire de la caméra : {}", forme.label()));
        let p = &mut self.doc.panels[pi];
        let (a, b) = (t.depart, t.fin.unwrap_or(len - 1));
        p.trajectoire_camera = Some(t);
        // (Une caméra qui suit un chemin dans l'espace peut sortir des anciennes limites, partie 159.)
        p.camera_libre |= !p.espace;
        self.modified = true;
        self.status = format!(
            "{} posé pour la caméra, des images {} à {} : elle garde le sujet dans le cadre{}. Ses poignées se tirent avec l'outil Trajectoire, dans la vue Espace 3D.",
            forme.label(),
            self.frame_label(a),
            self.frame_label(b),
            if remplace { " (le chemin d'avant est remplacé ; Ctrl+Z le rend)" } else { "" }
        );
    }

    /// Pose `forme` à la tête de lecture (un clic dans le panneau Préréglages).
    pub(crate) fn poser_trajectoire_toute_faite_a_la_tete(&mut self, forme: Forme) {
        let f = self.cur_frame;
        self.poser_trajectoire_toute_faite(forme, f);
    }
}
