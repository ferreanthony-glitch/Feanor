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
/// La bascule et le panoramique restent loin de la verticale (la caméra se retournerait).
pub const MAX_TILT: f32 = 89.0;

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
        let right0 = m::normalize(m::cross(forward, [0.0, 1.0, 0.0]));
        let up0 = m::cross(right0, forward);
        // Rotation du cadre dans le sens horaire (vu de derrière la caméra) : la droite du cadre descend, le haut penche
        // vers la droite.
        let (s, c) = self.roll.to_radians().sin_cos();
        let right = m::add(m::scale(right0, c), m::scale(up0, -s));
        let up = m::add(m::scale(up0, c), m::scale(right0, s));
        m::View { eye: self.pos, right, up, forward, focal: self.focal, ref_width }
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
}

impl Monde {
    pub fn new(stage: [usize; 2], frame: [usize; 2]) -> Monde {
        let k = SceneCam::meters_per_px(frame[0]);
        let d0_px = ref_distance(frame);
        Monde { stage: [stage[0] as f32, stage[1] as f32], frame, k, d0: d0_px * k, d0_px }
    }

    pub fn of(doc: &crate::document::Document) -> Monde {
        Monde::new([doc.width, doc.height], doc.frame)
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

    // ------------------------------------------------------------------------------ cartes

    /// Le placement 3D d'une carte : `t` sa pose, `depth` sa profondeur totale (`Layer::depth_at`).
    pub fn card_xf(&self, t: &LayerTransform, depth: f32) -> Xf {
        let s = 1.0 + depth.max(Layer::MIN_DEPTH);
        let [w, h] = self.stage;
        Xf {
            pos: [t.x * s * self.k, -t.y * s * self.k, -self.d0 * depth.max(Layer::MIN_DEPTH)],
            rot: [t.tilt_x, t.tilt_y, -t.angle],
            size: [w * self.k * s * t.scale * t.stretch_x, h * self.k * s * t.scale * t.stretch_y, 1.0],
        }
    }

    /// La pose du calque qui donne le placement `xf` à une carte dont la pose était `prev` à la profondeur
    /// `prev_depth`. Renvoie la nouvelle pose et la nouvelle profondeur totale (son `dz` est celui de `prev` : à l'appelant
    /// de répartir la profondeur, `apply_card_xf`).
    ///
    /// Taille : le placement garde la taille que la carte a dans le monde (elle rapetisse si on la recule). Si les deux
    /// axes ont changé du même rapport, c'est l'échelle uniforme qui bouge ; sinon, les étirements.
    pub fn card_from_xf(&self, prev: &LayerTransform, prev_depth: f32, xf: &Xf) -> (LayerTransform, f32) {
        let d = (-xf.pos[2] / self.d0).max(Layer::MIN_DEPTH);
        let s = 1.0 + d;
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
            tilt_x: LayerTransform::clamp_tilt(xf.rot[0]),
            tilt_y: LayerTransform::clamp_tilt(xf.rot[1]),
            dz: prev.dz,
        };
        (t, d)
    }

    /// Pose la carte `xf` sur `layer` à l'image `frame` : sa clé de cette image (créée au besoin) s'il est animé, sa
    /// pose fixe et sa profondeur sinon.
    pub fn apply_card_xf(&self, layer: &mut Layer, frame: usize, xf: &Xf) {
        let t0 = layer.pose(frame);
        let d0 = layer.depth_at(frame);
        let (mut t, d) = self.card_from_xf(&t0, d0, xf);
        t.dz = d - layer.depth;
        layer.set_pose(frame, t);
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
        if d[2].abs() < 1e-9 {
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
pub fn place(world: &Monde, layer: &Layer, frame: usize, camera: &CameraSeen) -> Option<Placed> {
    let seen = layer.transform_seen_at(camera, frame)?;
    let t = layer.pose(frame);
    if !t.is_tilted() {
        return Some(Placed { transform: (!seen.is_identity()).then_some(seen), pre: None });
    }
    let xf = world.card_xf(&t, layer.depth_at(frame));
    let warp = world.card_warp(world.eye_seen(camera), &xf)?;
    Some(Placed { transform: None, pre: Some(Pre { aff: Affine::IDENTITY, dims: None, warp: Some(Arc::new(warp)), card: true }) })
}
