//! Espace 3D : un seul monde pour les calques (les « cartes »), les objets 3D et la caméra du plan
//! (`docs/ESPACE3D.md`).
//!
//! Le monde est celui de la scène 3D de blocage : mètres, Y vers le haut, Z vers la caméra, l'origine au centre
//! de la feuille, le plan z = 0 est la feuille (le plan de référence de la caméra 2D). Un pixel de la feuille vaut
//! `k = PLATE_WIDTH_M / largeur du cadre` mètres. Ce fichier ne fait que des maths et des conversions, sans
//! interface ni carte graphique : tout se teste sans fenêtre.
//!
//! - **Une carte** est un calque du plan : un rectangle de la taille de la feuille, posé par sa `LayerTransform`
//!   (décalage, échelle, étirements, angle, inclinaison) et sa profondeur. `Monde::card_xf` en tire le placement 3D
//!   (`Xf`, celui des objets : les mêmes manipulateurs s'en servent) et `Monde::card_from_xf` fait le chemin inverse.
//!   Une carte à plat se pose comme avant (`Layer::transform_seen_at`, une affinité) : mêmes pixels. Une carte
//!   inclinée se pose par un maillage (`Warp`) : chaque point de la carte est projeté de l'œil de la caméra sur la
//!   feuille. La suite (bascule, panoramique, zoom, rotation de la caméra) est l'homographie d'avant (`camera.rs`).
//! - **La caméra** du plan est la caméra 3D : `SceneCam::from_plan_camera` en donne l'œil, et le chemin inverse est ici.

use std::sync::Arc;

use eframe::egui::{pos2, vec2, Pos2, Rect};

use crate::camera::{ref_distance, Camera, REF_FOCAL};
use crate::document::{CameraSeen, Layer, LayerTransform};
use crate::render::Pre;
use crate::scene3d::{SceneCam, Xf};
use crate::scene3dgizmo as gz;
use crate::scene3dmath::{self as m, Ray, V3};
use crate::vector::Affine;
use crate::warp::Warp;

/// Étiquette d'une carte dans une sélection de l'Espace 3D : les numéros d'objets sont petits, celui de la caméra
/// (`scene3dgizmo::CAMERA_ID`) est le plus grand ; une carte est son numéro de calque avec ce bit.
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

    /// La caméra du plan comme la scène 3D la voit (position, panoramique, bascule, rotation, focale).
    pub fn scene_cam(&self, cam: &Camera) -> SceneCam {
        SceneCam::from_plan_camera(cam, [self.stage[0] as usize, self.stage[1] as usize], self.frame)
    }

    /// La caméra du plan comme un objet de l'espace (pour les manipulateurs).
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

    /// Le point de la caméra de blocage d'un ancien projet (`PlanScene::cam`), pour une feuille de ce plan : la caméra du
    /// plan qui voit la même chose (position, panoramique, bascule, rotation, focale). Sert à reprendre l'ancienne caméra.
    pub fn plan_camera_of(&self, sc: &SceneCam) -> Camera {
        self.camera_from_scene(sc)
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
        let [w, h] = self.stage;
        let mx = xf.matrix();
        // (Un pixel de plus que le dessin : le bord adouci en fait partie.)
        let src = Rect::from_min_size(pos2(0.0, 0.0), vec2(w, h)).expand(1.0);
        Warp::partial(src, CARD_CELLS, |p| self.to_sheet(eye, m::point(&mx, [p.x / w - 0.5, -(p.y / h - 0.5), 0.0])))
    }

    /// Le point `p` de la carte (pixels de son dessin) vu de l'œil `eye`, sur la feuille (pixels).
    pub fn card_to_sheet(&self, eye: V3, xf: &Xf, p: Pos2) -> Option<Pos2> {
        let [w, h] = self.stage;
        self.to_sheet(eye, m::point(&xf.matrix(), [p.x / w - 0.5, -(p.y / h - 0.5), 0.0]))
    }

    /// Le chemin inverse : le point `s` de la feuille, retrouvé sur la carte (pixels de son dessin), là où le regard de
    /// l'œil `eye` qui passe par `s` traverse le plan de la carte. C'est ce qui sert à dessiner sur une carte inclinée.
    pub fn sheet_to_card(&self, eye: V3, xf: &Xf, s: Pos2) -> Option<Pos2> {
        let on_sheet = [(s.x - self.stage[0] / 2.0) * self.k, -(s.y - self.stage[1] / 2.0) * self.k, 0.0];
        let ray = Ray::new(eye, m::sub(on_sheet, eye));
        let (o, d) = ray.to_local(&m::inverse_affine(&xf.matrix()));
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
