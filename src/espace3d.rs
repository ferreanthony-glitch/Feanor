//! Espace 3D : un seul monde pour les calques (les « cartes ») et la caméra du plan (`docs/ESPACE3D.md`).
//!
//! Le monde : mètres, Y vers le haut, Z vers la caméra, l'origine au centre
//! de la feuille, le plan z = 0 est la feuille (le plan de référence de la caméra 2D). Un pixel de la feuille vaut
//! `k = PLATE_WIDTH_M / largeur du cadre` mètres. Ce fichier ne fait que des maths et des conversions, sans
//! interface ni carte graphique : tout se teste sans fenêtre.
//!
//! - **Une carte** est un calque du plan : un rectangle de la taille de la feuille, posé par sa `LayerTransform`
//!   (décalage, échelle, étirements, angle, inclinaison) et sa profondeur. `Monde::card_xf` en tire le placement 3D
//!   (`Xf` : les manipulateurs s'en servent) et `Monde::card_from_xf` fait le chemin inverse.
//!   Une carte à plat se pose comme avant (`Layer::transform_seen_at`, une affinité) : mêmes pixels. Une carte
//!   inclinée se pose par un maillage (`Warp`) : chaque point de la carte est projeté de l'œil de la caméra sur la
//!   feuille. La suite (bascule, panoramique, zoom, rotation de la caméra) est l'homographie d'avant (`camera.rs`).
//! - **La caméra** du plan est la caméra 3D : `SceneCam::from_plan_camera` en donne l'œil, et le chemin inverse est ici.

use std::sync::Arc;

use eframe::egui::{pos2, vec2, Pos2, Rect};

use crate::camera::{ref_distance, Camera, REF_FOCAL};
use crate::document::{CameraSeen, Layer, LayerTransform};
use crate::espace3dgizmo as gz;
use crate::espace3dmath::{self as m, Ray, V3};
use crate::render::Pre;
use crate::vector::Affine;
use crate::warp::Warp;

/// Largeur, en mètres, du cadre de la caméra par défaut du plan à la distance de référence : la feuille (le plan de
/// référence de la caméra 2D) est le plan z = 0 de l'espace, et ce plan mesure `PLATE_WIDTH_M` mètres dans le cadre. C'est
/// la seule échelle qui lie l'espace (en mètres) à la feuille (en pixels).
pub const PLATE_WIDTH_M: f32 = 6.0;

/// Position, rotation (degrés, ordre de `espace3dmath::rotation`) et taille d'une carte (ou de la caméra) dans l'espace.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xf {
    pub pos: V3,
    pub rot: V3,
    pub size: V3,
}

impl Default for Xf {
    fn default() -> Xf {
        Xf { pos: [0.0; 3], rot: [0.0; 3], size: [1.0; 3] }
    }
}

impl Xf {
    pub fn matrix(&self) -> m::M4 {
        m::trs(self.pos, self.rot, self.size)
    }
}

/// La caméra du plan vue comme une prise de vue de l'espace. Mêmes conventions que la caméra 2D du plan (`camera.rs`) :
/// panoramique positif vers la droite, bascule positive vers le haut, rotation (roulis) positive dans le sens horaire à
/// l'écran ; la focale en mm sur une pellicule de 36 mm de large, donc le même angle de champ que `camera::angle_of_view`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneCam {
    pub pos: V3,
    pub pan: f32,
    pub tilt: f32,
    pub roll: f32,
    pub focal: f32,
}

impl Default for SceneCam {
    fn default() -> SceneCam {
        SceneCam { pos: [0.0, 1.5, 6.0], pan: 0.0, tilt: -4.0, roll: 0.0, focal: REF_FOCAL }
    }
}

pub const MIN_FOCAL: f32 = 8.0;
pub const MAX_FOCAL: f32 = 300.0;
/// La bascule va jusqu'à la verticale : une plongée ou une contre-plongée totale (`docs/ESPACE-LIBRE.md`, étape A8).
/// Elle s'arrêtait à 89° tant que `SceneCam::view` tirait la droite de la caméra de la verticale du monde, ce qui n'a pas
/// de sens quand elle regarde à la verticale ; sa droite vient maintenant de son panoramique. Au-delà de 90°, la caméra
/// serait à l'envers : c'est un panoramique d'un demi-tour et une rotation. La caméra d'un plan ordinaire reste bridée
/// bien avant, par `Camera::limited` (elle doit voir la feuille).
pub const MAX_TILT: f32 = 90.0;

impl SceneCam {
    /// Vers où la caméra regarde. À panoramique et bascule nuls : vers −Z, comme la caméra 2D qui regarde la feuille de
    /// face.
    pub fn forward(&self) -> V3 {
        let (st, ct) = self.tilt.to_radians().sin_cos();
        let (sp, cp) = self.pan.to_radians().sin_cos();
        [ct * sp, st, -ct * cp]
    }

    /// Le point de vue pour une image dont `ref_width` pixels représentent les 36 mm de pellicule.
    pub fn view(&self, ref_width: f32) -> m::View {
        let forward = self.forward();
        // La droite de la caméra ne dépend que de son panoramique (c'est `cross(forward, verticale)` normalisé tant que
        // la bascule est sous 90°, et elle reste définie à 90°) : les mêmes axes que `Monde::repere_camera`.
        let (sp, cp) = self.pan.to_radians().sin_cos();
        let right0 = [cp, 0.0, sp];
        let up0 = m::cross(right0, forward);
        // Rotation du cadre dans le sens horaire (vu de derrière la caméra) : la droite du cadre descend, le haut penche
        // vers la droite.
        let (s, c) = self.roll.to_radians().sin_cos();
        let right = m::add(m::scale(right0, c), m::scale(up0, -s));
        let up = m::add(m::scale(up0, c), m::scale(right0, s));
        m::View { eye: self.pos, right, up, forward, focal: self.focal, ref_width, ortho: None }
    }

    pub fn sanitized(self) -> SceneCam {
        let fin = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        SceneCam {
            pos: self.pos.map(|v| fin(v, -1e4, 1e4, 0.0)),
            pan: fin(self.pan, -3600.0, 3600.0, 0.0),
            tilt: fin(self.tilt, -MAX_TILT, MAX_TILT, 0.0),
            roll: fin(self.roll, -3600.0, 3600.0, 0.0),
            focal: fin(self.focal, MIN_FOCAL, MAX_FOCAL, REF_FOCAL),
        }
    }

    /// Angle de champ horizontal, en degrés (celui de l'objectif de la caméra 2D).
    pub fn angle_of_view(&self) -> f32 {
        crate::camera::angle_of_view(self.focal)
    }

    /// Les quatre coins du cadre à la distance `d` devant la caméra : haut-gauche, haut-droite, bas-droite, bas-gauche.
    /// `frame` : l'image finale (pour les proportions).
    pub fn corners_at(&self, d: f32, frame: [usize; 2]) -> [V3; 4] {
        let v = self.view(frame[0].max(1) as f32);
        let half_w = d * 18.0 / self.focal.max(1.0);
        let half_h = half_w * frame[1].max(1) as f32 / frame[0].max(1) as f32;
        let at = |x: f32, y: f32| {
            let on_axis = m::add(self.pos, m::scale(v.forward, d));
            m::add(on_axis, m::add(m::scale(v.right, x), m::scale(v.up, y)))
        };
        [at(-half_w, half_h), at(half_w, half_h), at(half_w, -half_h), at(-half_w, -half_h)]
    }

    /// Regarde le point `target` (panoramique et bascule ; la rotation est gardée).
    pub fn aim_at(&mut self, target: V3) {
        let d = m::sub(target, self.pos);
        if m::length(d) < 1e-6 {
            return;
        }
        let d = m::normalize(d);
        self.tilt = d[1].clamp(-1.0, 1.0).asin().to_degrees().clamp(-MAX_TILT, MAX_TILT);
        self.pan = d[0].atan2(-d[2]).to_degrees();
    }

    /// Mètres par pixel de la feuille quand le cadre de `frame_w` pixels mesure `PLATE_WIDTH_M` mètres à la distance de
    /// référence de la caméra 2D.
    pub fn meters_per_px(frame_w: usize) -> f32 {
        PLATE_WIDTH_M / frame_w.max(1) as f32
    }

    /// La caméra 2D du plan, à l'image où elle est, posée dans l'espace : le plan z = 0 est la feuille (le plan de
    /// référence), à l'échelle de `meters_per_px`. Même focale, même panoramique, même bascule, même rotation, et l'œil là
    /// où `Monde::eye` le met : l'espace vu par cette caméra et la feuille vue par la caméra 2D se superposent.
    pub fn from_plan_camera(cam: &Camera, stage: [usize; 2], frame: [usize; 2]) -> SceneCam {
        let k = Self::meters_per_px(frame[0]);
        let d0 = crate::camera::ref_distance(frame);
        SceneCam {
            pos: [(cam.x - stage[0] as f32 / 2.0) * k, -(cam.y - stage[1] as f32 / 2.0) * k, (d0 - cam.dolly) * k],
            pan: cam.pan,
            tilt: cam.tilt,
            roll: cam.angle,
            focal: cam.focal().clamp(MIN_FOCAL, MAX_FOCAL),
        }
        .sanitized()
    }
}

/// Étiquette d'une carte dans une sélection de l'Espace 3D : le numéro de la caméra (`espace3dgizmo::CAMERA_ID`) est le
/// plus grand ; une carte est son numéro de calque avec ce bit.
pub const CARD_TAG: u64 = 1 << 62;

/// Le numéro de sélection de la carte du calque `layer_id`.
pub fn card_selection(layer_id: u64) -> u64 {
    CARD_TAG | layer_id
}

/// Le numéro de calque d'un numéro de sélection, si c'est une carte.
pub fn card_of(selection: u64) -> Option<u64> {
    (selection != gz::CAMERA_ID && selection & CARD_TAG != 0).then_some(selection & !CARD_TAG)
}

/// Cases par côté du maillage d'une carte inclinée.
pub const CARD_CELLS: usize = 64;

/// Plus près de l'œil que cette fraction de la distance de référence, un point de carte n'est pas posé.
pub const NEAR_FRACTION: f32 = 0.02;

/// Un calque est une carte : ni un dossier, ni un calque d'effet.
pub fn is_card(layer: &Layer) -> bool {
    !layer.folder && layer.effet.is_none()
}

/// Une surface à dessiner qu'on pose d'un geste dans un plan en Espace 3D (`Monde::surface_xf`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Face,
    Sol,
    Mur,
    /// Le ciel (`espace3ddecor.rs`, partie 157) : debout comme un mur, mais `CIEL_RECUL` fois plus loin, centré sur
    /// l'horizon (la hauteur de l'œil de la caméra).
    Ciel,
}

/// Le ciel se pose à `CIEL_RECUL` fois la distance d'une surface (`Monde::surface_xf`) : loin derrière tout le reste.
pub const CIEL_RECUL: f32 = 20.0;

impl Surface {
    /// Le nom du calque créé et de l'étape d'annulation.
    pub fn nom(self) -> &'static str {
        match self {
            Surface::Face => "Surface face à la caméra",
            Surface::Sol => "Surface au sol",
            Surface::Mur => "Surface en mur",
            Surface::Ciel => "Ciel",
        }
    }
}

/// Les dimensions du monde d'un plan : la feuille, le cadre, et l'échelle qui lie les pixels aux mètres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Monde {
    /// La feuille, en pixels.
    pub stage: [f32; 2],
    pub frame: [usize; 2],
    /// Mètres par pixel de la feuille (au plan de référence).
    pub k: f32,
    /// Distance de référence de la caméra, en mètres.
    pub d0: f32,
    /// La même, en pixels de la feuille (`camera::ref_distance`).
    pub d0_px: f32,
    /// Le monde d'un **plan en Espace 3D** (`Monde::du_plan`) : la profondeur de ses cartes est libre, elles passent la
    /// caméra de référence et vont derrière elle (`Layer::depth_libre_at`, `docs/INTERFACE.md`, partie 148). Faux : un plan
    /// ordinaire, dont les cartes s'arrêtent à `Layer::MIN_DEPTH`. `carte` et `profondeur` lisent selon lui.
    pub libre: bool,
}

impl Monde {
    pub fn new(stage: [usize; 2], frame: [usize; 2]) -> Monde {
        let k = SceneCam::meters_per_px(frame[0]);
        let d0_px = ref_distance(frame);
        Monde { stage: [stage[0] as f32, stage[1] as f32], frame, k, d0: d0_px * k, d0_px, libre: false }
    }

    pub fn of(doc: &crate::document::Document) -> Monde {
        Monde::new([doc.width, doc.height], doc.frame)
    }

    /// Le monde du plan `index` du projet : celui de `of`, qui sait en plus si le plan est en Espace 3D (`libre`).
    pub fn du_plan(doc: &crate::document::Document, index: usize) -> Monde {
        Monde { libre: doc.plan_en_espace(index), ..Monde::of(doc) }
    }

    // ------------------------------------------------------------------------------ caméra

    /// L'œil de la caméra, en mètres.
    pub fn eye(&self, cam: &Camera) -> V3 {
        [(cam.x - self.stage[0] / 2.0) * self.k, -(cam.y - self.stage[1] / 2.0) * self.k, (self.d0_px - cam.dolly) * self.k]
    }

    /// L'œil d'une caméra vue par le multiplan (`CameraSeen` : décalage depuis le centre de la feuille, avancée).
    pub fn eye_seen(&self, seen: &CameraSeen) -> V3 {
        [seen.offset.x * self.k, -seen.offset.y * self.k, (seen.d0 - seen.dolly) * self.k]
    }

    /// La caméra du plan comme prise de vue de l'espace (position, panoramique, bascule, rotation, focale).
    pub fn scene_cam(&self, cam: &Camera) -> SceneCam {
        SceneCam::from_plan_camera(cam, [self.stage[0] as usize, self.stage[1] as usize], self.frame)
    }

    /// La caméra du plan comme un placement de l'espace (pour les manipulateurs).
    pub fn camera_xf(&self, cam: &Camera) -> Xf {
        gz::cam_xf(&self.scene_cam(cam))
    }

    /// La caméra du plan mise au placement `xf` (son zoom est gardé).
    pub fn camera_from_xf(&self, base: &Camera, xf: &Xf) -> Camera {
        let sc = gz::cam_from_xf(&self.scene_cam(base), xf);
        Camera { zoom: base.zoom, ..self.camera_from_scene(&sc) }
    }

    /// La caméra du plan qui correspond à une caméra de la scène (le chemin inverse de `scene_cam`).
    pub fn camera_from_scene(&self, sc: &SceneCam) -> Camera {
        Camera {
            x: sc.pos[0] / self.k + self.stage[0] / 2.0,
            y: -sc.pos[1] / self.k + self.stage[1] / 2.0,
            zoom: (sc.focal / REF_FOCAL).clamp(0.05, 50.0),
            angle: sc.roll,
            dolly: self.d0_px - sc.pos[2] / self.k,
            tilt: sc.tilt,
            pan: sc.pan,
        }
    }

    // ------------------------------------------------------------------------------ plan en Espace 3D : le repère de la caméra

    /// L'œil de la **caméra de référence** : au-dessus du centre de la feuille, à la distance de référence, qui regarde
    /// la feuille de face. C'est l'œil dont on pose les cartes d'un plan en Espace 3D (`docs/ESPACE-LIBRE.md`, 2).
    pub fn eye_reference(&self) -> V3 {
        [0.0, 0.0, self.d0]
    }

    /// La caméra de référence comme caméra du plan : au centre de la feuille, sans zoom, avancée, rotation, bascule ni
    /// panoramique. Elle filme le cadre au centre de la feuille ; c'est elle qui cadre un plan en Espace 3D, une fois les
    /// cartes passées dans le repère de la caméra réelle (`repere_camera`).
    pub fn camera_reference(&self) -> Camera {
        Camera { x: self.stage[0] / 2.0, y: self.stage[1] / 2.0, zoom: 1.0, angle: 0.0, dolly: 0.0, tilt: 0.0, pan: 0.0 }
    }

    /// **Le changement de repère d'un plan en Espace 3D** : la matrice qui amène le monde devant la caméra de référence
    /// comme la caméra `cam` le voit. « La caméra ne bouge pas, c'est le monde qui tourne devant elle. »
    ///
    /// Un point `p` du monde, vu par `cam`, tombe dans l'image là où le point `repere · p`, vu par la caméra de référence,
    /// tombe (`to_frame`). On pose donc les cartes par `repere · carte` avec l'œil de référence, et tout le rendu d'avant
    /// (maillages, composition, effets) sert tel quel, quelle que soit l'orientation de la caméra.
    ///
    /// `cam` est prise **telle quelle** : ni `Camera::limited` (elle n'a plus à voir la feuille), ni la borne de
    /// `Camera::distance` (elle peut passer derrière le plan z = 0). Mêmes conventions que `Camera::homography` : le
    /// panoramique autour de la verticale, la bascule autour de l'axe horizontal de la caméra, la rotation dans le sens
    /// horaire à l'écran, l'œil de `Monde::eye`. Le zoom (la focale) est compté ici : la zone filmée reste le cadre au
    /// centre de la feuille.
    ///
    /// Caméra de référence : l'identité.
    pub fn repere_camera(&self, cam: &Camera) -> m::M4 {
        let eye = self.eye(cam);
        let (sr, cr) = cam.angle.to_radians().sin_cos();
        let (st, ct) = cam.tilt.to_radians().sin_cos();
        let (sp, cp) = cam.pan.to_radians().sin_cos();
        let zoom = cam.zoom.max(0.01);
        // Les axes de la caméra dans le monde (les colonnes de panoramique · bascule de `Camera::homography`) : sa
        // droite, son haut, son arrière (elle regarde vers −arrière).
        let right = [cp, 0.0, sp];
        let up = [-sp * st, ct, cp * st];
        let back = [-sp * ct, -st, cp * ct];
        // Dans le repère de la caméra, la rotation du cadre (roulis) puis le zoom ; la profondeur n'est pas touchée.
        let r0 = m::scale(m::add(m::scale(right, cr), m::scale(up, -sr)), zoom);
        let r1 = m::scale(m::add(m::scale(right, sr), m::scale(up, cr)), zoom);
        let r2 = back;
        // p' = L · (p − œil) + (0, 0, d0) ; matrice rangée par colonnes (`espace3dmath`).
        [
            r0[0], r1[0], r2[0], 0.0,
            r0[1], r1[1], r2[1], 0.0,
            r0[2], r1[2], r2[2], 0.0,
            -m::dot(r0, eye), -m::dot(r1, eye), -m::dot(r2, eye) + self.d0, 1.0,
        ]
    }

    /// La matrice de la carte `xf` dans le repère de la caméra `cam` : à donner à `card_warp_m`, `card_to_sheet_m` et
    /// `sheet_to_card_m` avec l'œil de référence (`eye_reference`).
    pub fn carte_dans_le_repere(&self, cam: &Camera, xf: &Xf) -> m::M4 {
        m::mul(&self.repere_camera(cam), &xf.matrix())
    }

    /// Où le point `p` du monde tombe dans l'image finale (pixels du cadre, origine en haut à gauche), filmé par la
    /// caméra `cam` prise telle quelle (libre). Rien s'il est derrière la caméra ou trop près d'elle.
    pub fn to_frame(&self, cam: &Camera, p: V3) -> Option<Pos2> {
        let s = self.to_sheet(self.eye_reference(), m::point(&self.repere_camera(cam), p))?;
        Some(pos2(s.x - self.stage[0] / 2.0 + self.frame[0] as f32 / 2.0, s.y - self.stage[1] / 2.0 + self.frame[1] as f32 / 2.0))
    }

    // ------------------------------------------------------------------------------ plan en Espace 3D : poser une surface

    /// La hauteur du **sol** d'un plan en Espace 3D : le bas du cadre de la caméra de référence, sur la feuille. La caméra
    /// de référence est donc à mi-hauteur du cadre au-dessus du sol, et une surface au sol se voit dès qu'on la pose.
    pub fn sol_y(&self) -> f32 {
        -(self.frame[1] as f32 * self.k) / 2.0
    }

    /// Le placement d'une **surface à dessiner** neuve pour la caméra `cam` (prise telle quelle), à `distance` mètres
    /// (`docs/ESPACE-LIBRE.md`, étape A6). Elle a la taille de la feuille à l'échelle où un pixel de son dessin fait un
    /// pixel de l'image à cette distance : posée face à la caméra, le cadre tombe en son centre, avec la marge de la
    /// feuille autour.
    ///
    /// - `Face` : face à la caméra, sur son axe, à `distance` ; elle suit sa bascule et sa rotation.
    /// - `Sol` : couchée au sol (`sol_y`), devant la caméra à `distance` à l'horizontale, son haut vers le fond.
    /// - `Mur` : debout sur le sol, face à la caméra à l'horizontale (sans sa bascule), à `distance`.
    /// - `Ciel` : debout, face à la caméra à l'horizontale, à `CIEL_RECUL` fois `distance` (au plus 200 fois la distance de
    ///   référence), centré sur l'horizon (la hauteur de l'œil) : sa moitié haute est le ciel, sa moitié basse, derrière le
    ///   sol, bouche le vide entre le bout du sol et l'horizon. Il a la taille de la feuille à cette distance, donc remplit
    ///   le cadre.
    ///
    /// La surface se pose où que regarde la caméra, **derrière la caméra de référence comprise** (`docs/INTERFACE.md`,
    /// partie 148 ; avant, elle y était refusée). Rien seulement si elle tomberait plus loin de la feuille que ce qu'une
    /// carte sait tenir (`Layer::MAX_DEPTH_LIBRE` fois la distance de référence, devant ou derrière).
    pub fn surface_xf(&self, cam: &Camera, sorte: Surface, distance: f32) -> Option<Xf> {
        let distance = if distance.is_finite() { distance.clamp(self.d0 * 0.05, self.d0 * 200.0) } else { self.d0 };
        let distance = if sorte == Surface::Ciel { (distance * CIEL_RECUL).min(self.d0 * 200.0) } else { distance };
        let eye = self.eye(cam);
        let (sr, cr) = cam.angle.to_radians().sin_cos();
        let (st, ct) = cam.tilt.to_radians().sin_cos();
        let (sp, cp) = cam.pan.to_radians().sin_cos();
        // Les axes de la caméra (voir `repere_camera`) et sa direction à l'horizontale.
        let right = [cp, 0.0, sp];
        let up = [-sp * st, ct, cp * st];
        let back = [-sp * ct, -st, cp * ct];
        let fond = [sp, 0.0, -cp];
        // La largeur du cadre à cette distance, puis celle de la feuille à la même échelle.
        let cadre = PLATE_WIDTH_M * (distance / self.d0) / cam.zoom.max(0.01);
        let w = cadre * self.stage[0] / self.frame[0].max(1) as f32;
        let h = w * self.stage[1] / self.stage[0].max(1.0);
        let devant = m::add([eye[0], self.sol_y(), eye[2]], m::scale(fond, distance));
        let (pos, x, y, z) = match sorte {
            Surface::Face => (
                m::add(eye, m::scale(back, -distance)),
                m::add(m::scale(right, cr), m::scale(up, -sr)),
                m::add(m::scale(right, sr), m::scale(up, cr)),
                back,
            ),
            Surface::Sol => (devant, right, fond, [0.0, 1.0, 0.0]),
            Surface::Mur => (m::add(devant, [0.0, h / 2.0, 0.0]), right, [0.0, 1.0, 0.0], m::scale(fond, -1.0)),
            Surface::Ciel => (m::add(eye, m::scale(fond, distance)), right, [0.0, 1.0, 0.0], m::scale(fond, -1.0)),
        };
        if !(pos[2].abs() <= self.d0 * Layer::MAX_DEPTH_LIBRE) {
            return None;
        }
        #[rustfmt::skip]
        let axes: m::M4 = [
            x[0], x[1], x[2], 0.0,
            y[0], y[1], y[2], 0.0,
            z[0], z[1], z[2], 0.0,
            0.0, 0.0, 0.0, 1.0,
        ];
        Some(Xf { pos, rot: m::euler_of(&axes), size: [w, h, 1.0] })
    }

    // ------------------------------------------------------------------------------ cartes

    /// Le placement 3D d'une carte : `t` sa pose, `depth` sa profondeur totale (`Layer::depth_at` dans un plan ordinaire,
    /// `Layer::depth_libre_at` dans un plan en Espace 3D : `carte` choisit).
    ///
    /// La carte est à `−d0 · depth` le long de Z, et l'agrandissement du multiplan `1 + depth` règle sa place et sa
    /// taille : vue de la caméra de référence, elle paraît telle qu'elle est dessinée. **Sous `Layer::MIN_DEPTH`** (un
    /// plan en Espace 3D seulement : un plan ordinaire ne donne jamais une telle profondeur), cet agrandissement reste
    /// celui de la borne : la carte garde sa place et sa taille dans l'espace et ne fait plus qu'avancer le long de Z,
    /// jusqu'à la caméra de référence (profondeur −1) et derrière elle (`docs/INTERFACE.md`, partie 148). Le placement est
    /// continu à la borne, et `card_from_xf_libre` en retrouve la pose et la profondeur partout.
    pub fn card_xf(&self, t: &LayerTransform, depth: f32) -> Xf {
        let s = 1.0 + depth.max(Layer::MIN_DEPTH);
        let [w, h] = self.stage;
        Xf {
            pos: [t.x * s * self.k, -t.y * s * self.k, -self.d0 * depth],
            rot: [t.tilt_x, t.tilt_y, -t.angle],
            size: [w * self.k * s * t.scale * t.stretch_x, h * self.k * s * t.scale * t.stretch_y, 1.0],
        }
    }

    /// La profondeur du calque `layer` à l'image `frame` dans ce monde : libre dans un plan en Espace 3D (`libre`), bornée
    /// à `Layer::MIN_DEPTH` sinon.
    pub fn profondeur(&self, layer: &Layer, frame: usize) -> f32 {
        layer.depth_selon(frame, self.libre)
    }

    /// Le placement 3D de la carte du calque `layer` à l'image `frame` : `card_xf` de sa pose et de sa profondeur dans ce
    /// monde (`profondeur`). C'est par ici que l'Espace 3D lit une carte, pour la montrer, la choisir ou y dessiner.
    pub fn carte(&self, layer: &Layer, frame: usize) -> Xf {
        self.card_xf(&layer.pose(frame), self.profondeur(layer, frame))
    }

    /// La pose du calque qui donne le placement `xf` à une carte dont la pose était `prev` à la profondeur
    /// `prev_depth`. Renvoie la nouvelle pose et la nouvelle profondeur totale (son `dz` est celui de `prev` : à l'appelant
    /// de répartir la profondeur, `apply_card_xf`).
    ///
    /// Taille : le placement garde la taille que la carte a dans le monde (elle rapetisse si on la recule). Si les deux
    /// axes ont changé du même rapport, c'est l'échelle uniforme qui bouge ; sinon, les étirements.
    pub fn card_from_xf(&self, prev: &LayerTransform, prev_depth: f32, xf: &Xf) -> (LayerTransform, f32) {
        self.card_from_xf_selon(prev, prev_depth, xf, false)
    }

    /// Comme `card_from_xf`, pour une carte d'un plan en Espace 3D : l'inclinaison va jusqu'au demi-tour
    /// (`MAX_TILT_LIBRE`), la carte se couche au sol ou se tourne face à la caméra où qu'elle regarde ; et sa profondeur
    /// est libre (`Layer::MAX_DEPTH_LIBRE`), la carte passe la caméra de référence et va derrière elle (`card_xf`).
    pub fn card_from_xf_libre(&self, prev: &LayerTransform, prev_depth: f32, xf: &Xf) -> (LayerTransform, f32) {
        self.card_from_xf_selon(prev, prev_depth, xf, true)
    }

    fn card_from_xf_selon(&self, prev: &LayerTransform, prev_depth: f32, xf: &Xf, libre: bool) -> (LayerTransform, f32) {
        let borne = if libre { LayerTransform::clamp_tilt_libre } else { LayerTransform::clamp_tilt };
        // La profondeur : bornée à `MIN_DEPTH` dans un plan ordinaire, libre dans un plan en Espace 3D. L'agrandissement
        // du multiplan, lui, s'arrête à la borne dans les deux cas (`card_xf`).
        let d = if libre { Layer::clamp_depth_libre(-xf.pos[2] / self.d0) } else { (-xf.pos[2] / self.d0).max(Layer::MIN_DEPTH) };
        let s = 1.0 + d.max(Layer::MIN_DEPTH);
        let s0 = 1.0 + prev_depth.max(Layer::MIN_DEPTH);
        let [w, h] = self.stage;
        // Ce que la taille demandée vaut sans l'agrandissement du multiplan, et ce que la pose d'avant aurait à ce
        // nouvel agrandissement.
        let (tx, ty) = (xf.size[0] / (w * self.k * s), xf.size[1] / (h * self.k * s));
        let comp = s0 / s;
        let (px, py) = (prev.scale * prev.stretch_x * comp, prev.scale * prev.stretch_y * comp);
        let (kx, ky) = (tx / px, ty / py);
        let same = kx > 0.0 && ky > 0.0 && (kx - ky).abs() <= 1e-4 * kx.max(ky);
        let (scale, stretch_x, stretch_y) = if same {
            ((prev.scale * comp * kx).clamp(0.01, 100.0), prev.stretch_x, prev.stretch_y)
        } else {
            ((prev.scale * comp).clamp(0.01, 100.0), LayerTransform::clamp_stretch(prev.stretch_x * kx), LayerTransform::clamp_stretch(prev.stretch_y * ky))
        };
        let t = LayerTransform {
            x: xf.pos[0] / (s * self.k),
            y: -xf.pos[1] / (s * self.k),
            scale,
            angle: -xf.rot[2],
            stretch_x,
            stretch_y,
            tilt_x: borne(xf.rot[0]),
            tilt_y: borne(xf.rot[1]),
            dz: prev.dz,
        };
        (t, d)
    }

    /// Pose la carte `xf` sur `layer` à l'image `frame` : sa clé de cette image (créée au besoin) s'il est animé, sa
    /// pose fixe et sa profondeur sinon.
    pub fn apply_card_xf(&self, layer: &mut Layer, frame: usize, xf: &Xf) {
        self.apply_card_xf_selon(layer, frame, xf, false)
    }

    /// Comme `apply_card_xf`, pour une carte d'un plan en Espace 3D (inclinaison et profondeur libres,
    /// `card_from_xf_libre`) : la carte se pose aussi derrière la caméra de référence.
    pub fn apply_card_xf_libre(&self, layer: &mut Layer, frame: usize, xf: &Xf) {
        self.apply_card_xf_selon(layer, frame, xf, true)
    }

    fn apply_card_xf_selon(&self, layer: &mut Layer, frame: usize, xf: &Xf, libre: bool) {
        let t0 = layer.pose(frame);
        let d0 = layer.depth_selon(frame, libre);
        let (mut t, d) = self.card_from_xf_selon(&t0, d0, xf, libre);
        t.dz = d - layer.depth;
        layer.set_pose_selon(frame, t, libre);
    }

    /// Les quatre coins d'une carte dans le monde : haut-gauche, haut-droite, bas-droite, bas-gauche.
    pub fn card_corners(&self, xf: &Xf) -> [V3; 4] {
        let mx = xf.matrix();
        [[-0.5, 0.5, 0.0], [0.5, 0.5, 0.0], [0.5, -0.5, 0.0], [-0.5, -0.5, 0.0]].map(|c| m::point(&mx, c))
    }

    /// Le point `p` du monde vu de l'œil `eye`, ramené sur la feuille (pixels) : là où la caméra le met avant d'être
    /// tournée. Rien s'il est derrière l'œil ou trop près de lui.
    pub fn to_sheet(&self, eye: V3, p: V3) -> Option<Pos2> {
        let dz = p[2] - eye[2];
        if eye[2] <= self.d0 * NEAR_FRACTION || dz > -self.d0 * NEAR_FRACTION {
            return None;
        }
        let t = -eye[2] / dz;
        let (x, y) = (eye[0] + (p[0] - eye[0]) * t, eye[1] + (p[1] - eye[1]) * t);
        let q = pos2(x / self.k + self.stage[0] / 2.0, -y / self.k + self.stage[1] / 2.0);
        (q.x.abs() < 1e6 && q.y.abs() < 1e6).then_some(q)
    }

    /// Le maillage qui pose la carte `xf` sur la feuille, vue de l'œil `eye` ; rien si elle est tout entière derrière
    /// l'œil. `dessin` : la taille de l'image de la carte (la feuille, d'ordinaire).
    pub fn card_warp(&self, eye: V3, xf: &Xf) -> Option<Warp> {
        self.card_warp_m(eye, &xf.matrix())
    }

    /// Comme `card_warp`, la carte étant donnée par sa matrice (du carré unité de la carte vers le monde) : celle d'un
    /// `Xf`, ou la même passée dans le repère de la caméra (`carte_dans_le_repere`, plan en Espace 3D).
    pub fn card_warp_m(&self, eye: V3, mx: &m::M4) -> Option<Warp> {
        let [w, h] = self.stage;
        // (Un pixel de plus que le dessin : le bord adouci en fait partie.)
        // (Une carte ne montre donc que sa feuille : c'est aussi là seulement qu'un trait est accepté sur elle,
        // `espace3ddessin::carte_montre`.)
        let src = Rect::from_min_size(pos2(0.0, 0.0), vec2(w, h)).expand(1.0);
        Warp::partial(src, CARD_CELLS, |p| self.to_sheet(eye, m::point(mx, [p.x / w - 0.5, -(p.y / h - 0.5), 0.0])))
    }

    /// Le point `p` de la carte (pixels de son dessin) vu de l'œil `eye`, sur la feuille (pixels).
    pub fn card_to_sheet(&self, eye: V3, xf: &Xf, p: Pos2) -> Option<Pos2> {
        self.card_to_sheet_m(eye, &xf.matrix(), p)
    }

    /// Comme `card_to_sheet`, la carte étant donnée par sa matrice.
    pub fn card_to_sheet_m(&self, eye: V3, mx: &m::M4, p: Pos2) -> Option<Pos2> {
        let [w, h] = self.stage;
        self.to_sheet(eye, m::point(mx, [p.x / w - 0.5, -(p.y / h - 0.5), 0.0]))
    }

    /// Le chemin inverse : le point `s` de la feuille, retrouvé sur la carte (pixels de son dessin), là où le regard de
    /// l'œil `eye` qui passe par `s` traverse le plan de la carte. C'est ce qui sert à dessiner sur une carte inclinée.
    pub fn sheet_to_card(&self, eye: V3, xf: &Xf, s: Pos2) -> Option<Pos2> {
        self.sheet_to_card_m(eye, &xf.matrix(), s)
    }

    /// Comme `sheet_to_card`, la carte étant donnée par sa matrice.
    pub fn sheet_to_card_m(&self, eye: V3, mx: &m::M4, s: Pos2) -> Option<Pos2> {
        let on_sheet = [(s.x - self.stage[0] / 2.0) * self.k, -(s.y - self.stage[1] / 2.0) * self.k, 0.0];
        let ray = Ray::new(eye, m::sub(on_sheet, eye));
        let (o, d) = ray.to_local(&m::inverse_affine(mx));
        if d[2].abs() < 1e-9 {
            return None;
        }
        let t = -o[2] / d[2];
        if t <= 0.0 {
            return None;
        }
        let (x, y) = (o[0] + d[0] * t, o[1] + d[1] * t);
        Some(pos2((x + 0.5) * self.stage[0], (0.5 - y) * self.stage[1]))
    }

    /// Le point de la carte (pixels de son dessin) touché par le rayon, et la distance le long du rayon ; rien si la
    /// carte est manquée ou derrière.
    pub fn pick_card(&self, xf: &Xf, ray: &Ray) -> Option<(f32, Pos2)> {
        let inv = m::inverse_affine(&xf.matrix());
        let (o, d) = ray.to_local(&inv);
        // (Un rayon abîmé, d'une vue de taille nulle : rien n'est touché.)
        if d[2].abs() < 1e-9 || !(o.iter().chain(d.iter()).all(|v| v.is_finite())) {
            return None;
        }
        let t = -o[2] / d[2];
        if t <= 0.0 {
            return None;
        }
        let (x, y) = (o[0] + d[0] * t, o[1] + d[1] * t);
        if x.abs() > 0.5 || y.abs() > 0.5 {
            return None;
        }
        Some((t, pos2((x + 0.5) * self.stage[0], (0.5 - y) * self.stage[1])))
    }
}

// ---------------------------------------------------------------------------------- plan en Espace 3D : l'ordre par distance

/// D'où l'on regarde les cartes pour les ordonner (`ordre_vu`).
pub enum Regard<'a> {
    /// La caméra du plan : son changement de repère (`Document::repere_du_plan` ; rien : la caméra de référence). C'est
    /// l'ordre de l'image.
    Camera(Option<&'a m::M4>),
    /// Un œil quelconque de l'espace et la direction où il regarde : la vue d'édition de l'Espace 3D (`espace3dview.rs`).
    Vue { eye: V3, forward: V3 },
}

/// Ce que l'ordre par distance sait d'un calque : la distance de son centre à la caméra (le long de son axe), et le plan
/// où il est posé (normale, écart), dans le repère de la caméra.
#[derive(Clone, Copy)]
struct Vu {
    dist: f32,
    plan: Option<(V3, f32)>,
}

/// Une chose à poser d'un seul tenant : un calque, ou un dossier avec ses calques (déjà ordonnés entre eux).
struct Bloc {
    membres: Vec<usize>,
    vu: Vu,
}

fn vu_du_calque(world: &Monde, layer: &Layer, frame: usize, regard: &Regard) -> Vu {
    // (L'ordre par distance ne sert qu'aux plans en Espace 3D : la profondeur de la carte y est libre, elle peut être
    // derrière la caméra de référence. Devant la borne des plans ordinaires, c'est `depth_at`.)
    let xf = world.card_xf(&layer.pose(frame), layer.depth_libre_at(frame));
    let (mx, dist) = match regard {
        // (Un calque accroché à la caméra reste dans le cadre : il se lit sans le changement de repère, comme `place_dans`.)
        Regard::Camera(repere) => {
            let mx = match repere.filter(|_| !layer.accroche) {
                Some(r) => m::mul(r, &xf.matrix()),
                None => xf.matrix(),
            };
            let centre = m::point(&mx, [0.0, 0.0, 0.0]);
            (mx, world.d0 - centre[2])
        }
        Regard::Vue { eye, forward } => {
            let mx = xf.matrix();
            let centre = m::point(&mx, [0.0, 0.0, 0.0]);
            (mx, m::dot(m::sub(centre, *eye), *forward))
        }
    };
    let (a, b, c) = (m::point(&mx, [-0.5, 0.5, 0.0]), m::point(&mx, [0.5, 0.5, 0.0]), m::point(&mx, [-0.5, -0.5, 0.0]));
    let n = m::cross(m::sub(b, a), m::sub(c, a));
    let len = m::length(n);
    let plan = (len.is_finite() && len > 1e-9).then(|| {
        let n = m::scale(n, 1.0 / len);
        // (Un seul sens par plan : la normale qui regarde vers la caméra de référence, ou sinon vers la droite, vers le haut.)
        let signe = [n[2], n[0], n[1]].into_iter().find(|v| v.abs() > 1e-6).map_or(1.0, f32::signum);
        let n = m::scale(n, signe);
        (n, m::dot(n, a))
    });
    Vu { dist: if dist.is_finite() { dist } else { 0.0 }, plan }
}

/// Deux calques sont posés dans le même plan (deux cartes à la même profondeur et de même inclinaison, par exemple).
fn meme_plan(a: (V3, f32), b: (V3, f32)) -> bool {
    const TOLERANCE: f32 = 1e-3;
    m::length(m::sub(a.0, b.0)) < TOLERANCE && (a.1 - b.1).abs() < TOLERANCE
}

/// Ordonne des blocs du plus loin au plus près. Les blocs posés dans un même plan forment un groupe, qui a une seule
/// distance (la moyenne des leurs) : ils gardent donc entre eux l'ordre où ils sont donnés (celui de la pile). Le tri
/// est stable : à distance égale, l'ordre donné.
fn ordonner(blocs: Vec<Bloc>) -> Vec<usize> {
    // Les groupes : (plan, somme des distances, nombre de blocs).
    let mut groupes: Vec<((V3, f32), f32, usize)> = Vec::new();
    let rangs: Vec<Option<usize>> = blocs
        .iter()
        .map(|b| {
            let plan = b.vu.plan?;
            let k = groupes.iter().position(|g| meme_plan(g.0, plan)).unwrap_or_else(|| {
                groupes.push((plan, 0.0, 0));
                groupes.len() - 1
            });
            groupes[k].1 += b.vu.dist;
            groupes[k].2 += 1;
            Some(k)
        })
        .collect();
    let mut cles: Vec<(f32, Bloc)> = blocs.into_iter().zip(rangs).map(|(b, k)| (k.map_or(b.vu.dist, |k| groupes[k].1 / groupes[k].2 as f32), b)).collect();
    cles.sort_by(|a, b| b.0.total_cmp(&a.0));
    cles.into_iter().flat_map(|(_, b)| b.membres).collect()
}

/// **L'ordre de pose des calques d'un plan en Espace 3D** (`docs/ESPACE-LIBRE.md`, étape A4) : du plus loin de la caméra
/// au plus près, pour que ce qui est devant cache ce qui est derrière. `ordre` : l'ordre de la pile
/// (`calqueannot::ordre`) ; `repere` : le changement de repère de la caméra du plan (`Document::repere_du_plan` ; rien :
/// la caméra de référence).
///
/// - La distance d'un calque est celle du centre de sa carte, le long de l'axe de la caméra.
/// - Les calques posés **dans un même plan** (même profondeur, même inclinaison : un trait et sa couleur, des éléments
///   d'un même décor) gardent entre eux l'ordre de la pile, où que soit la caméra.
/// - Un **dossier** se déplace d'un seul tenant, à la distance de son calque le plus loin ; ses calques sont ordonnés
///   entre eux de la même façon.
/// - Un **calque d'ajustement** a sa place dans l'ordre comme les autres (sa pose, sa profondeur) : il agit sur ce qui
///   est plus loin que lui (`calqueffet::Portees`).
/// - Les **calques d'annotation** restent posés après tous les autres, dans leur ordre.
/// - Deux cartes qui se traversent ne sont pas départagées point par point : l'une passe entière devant l'autre.
pub fn ordre_par_distance(world: &Monde, layers: &[Layer], ordre: &[usize], frame: usize, repere: Option<&m::M4>) -> Vec<usize> {
    ordre_vu(world, layers, ordre, frame, &Regard::Camera(repere))
}

/// Comme `ordre_par_distance`, d'où que l'on regarde (`Regard`) : mêmes règles (même plan = ordre de la pile, dossier en
/// bloc, annotations en dernier), la distance étant celle du centre de la carte le long du regard.
pub fn ordre_vu(world: &Monde, layers: &[Layer], ordre: &[usize], frame: usize, regard: &Regard) -> Vec<usize> {
    let vu = |li: usize| vu_du_calque(world, &layers[li], frame, regard);
    let ordinaires: Vec<usize> = ordre.iter().copied().filter(|&li| layers.get(li).is_some_and(|l| !l.annotation)).collect();
    let dans_un_dossier = |li: usize| crate::folders::parent_index(layers, li).filter(|fi| ordinaires.contains(fi));
    let mut blocs: Vec<Bloc> = Vec::new();
    for &li in &ordinaires {
        if dans_un_dossier(li).is_some() {
            continue;
        }
        if layers[li].folder {
            let enfants: Vec<usize> = ordinaires.iter().copied().filter(|&c| dans_un_dossier(c) == Some(li)).collect();
            let loin = enfants.iter().map(|&c| vu(c).dist).fold(f32::MIN, f32::max);
            let mut membres = ordonner(enfants.iter().map(|&c| Bloc { membres: vec![c], vu: vu(c) }).collect());
            membres.push(li);
            blocs.push(Bloc { membres, vu: Vu { dist: if enfants.is_empty() { 0.0 } else { loin }, plan: None } });
        } else {
            blocs.push(Bloc { membres: vec![li], vu: vu(li) });
        }
    }
    let mut out = ordonner(blocs);
    out.extend(ordre.iter().copied().filter(|&li| layers.get(li).is_some_and(|l| l.annotation)));
    out
}

/// Comment un calque se pose dans le plan d'une image : l'affinité d'avant (carte à plat), ou le maillage d'une carte
/// inclinée.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub transform: Option<LayerTransform>,
    pub pre: Option<Pre>,
}

/// La pose du calque `layer` à l'image `frame`, vu par la caméra `camera`. Rien : le calque est derrière la caméra (ou
/// une carte inclinée tout entière derrière l'œil), on ne le voit pas.
///
/// Carte à plat : exactement `Layer::transform_seen_at` (les mêmes pixels qu'avant l'Espace 3D).
///
/// (Tout ce qui compose une image passe maintenant par `place_dans`, qui sait le plan en Espace 3D : les silhouettes de
/// l'outil Trajectoire étaient le dernier appel sans repère, partie 149. Il ne reste ici que le plan ordinaire, pour les essais.)
#[cfg(test)]
pub fn place(world: &Monde, layer: &Layer, frame: usize, camera: &CameraSeen) -> Option<Placed> {
    place_dans(world, layer, frame, camera, None)
}

/// Comme `place`, pour un plan qui peut être un **plan en Espace 3D** (`docs/ESPACE-LIBRE.md`, étape A3) : `repere` est
/// alors le changement de repère de sa caméra (`Document::repere_du_plan`), et `camera` la caméra de référence.
///
/// Sans repère (plan ordinaire, ou plan en Espace 3D dont la caméra est la caméra de référence et dont aucune carte n'a
/// passé la borne des plans ordinaires) : exactement `place`.
/// Avec : chaque carte, à plat ou inclinée, est passée dans le repère de la caméra et posée par un maillage, vue de l'œil
/// de référence ; rien si elle est tout entière derrière la caméra. Un calque accroché à la caméra (`accroche.rs`) ne
/// reçoit pas le changement de repère : sa pose se lit dans le cadre, il y reste.
pub fn place_dans(world: &Monde, layer: &Layer, frame: usize, camera: &CameraSeen, repere: Option<&m::M4>) -> Option<Placed> {
    if let Some(repere) = repere.filter(|_| !layer.accroche) {
        // (Un repère veut dire « plan en Espace 3D » : la profondeur de la carte y est libre, elle peut être derrière la
        // caméra de référence, `docs/INTERFACE.md`, partie 148.)
        let xf = world.card_xf(&layer.pose(frame), layer.depth_libre_at(frame));
        let warp = world.card_warp_m(world.eye_reference(), &m::mul(repere, &xf.matrix()))?;
        return Some(Placed { transform: None, pre: Some(Pre { aff: Affine::IDENTITY, dims: None, warp: Some(Arc::new(warp)), card: true }) });
    }
    let seen = layer.transform_seen_at(camera, frame)?;
    let t = layer.pose(frame);
    if !t.is_tilted() {
        return Some(Placed { transform: (!seen.is_identity()).then_some(seen), pre: None });
    }
    let xf = world.card_xf(&t, layer.depth_at(frame));
    let warp = world.card_warp(world.eye_seen(camera), &xf)?;
    Some(Placed { transform: None, pre: Some(Pre { aff: Affine::IDENTITY, dims: None, warp: Some(Arc::new(warp)), card: true }) })
}
