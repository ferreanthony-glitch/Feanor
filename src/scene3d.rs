//! Scène 3D de blocage : les objets simples (cube, plan, cylindre, sphère, cône, escalier, mannequin,
//! modèle importé), leur placement, la caméra du plateau, la lumière, et le rangement de tout cela
//! par plan et par séquence.
//!
//! Le modèle, en deux mots :
//! - chaque **plan** a sa scène (`PlanScene`) : ses objets (personnages, accessoires), sa caméra du
//!   plateau, sa lumière, son style de rendu, et le dessin du calque « Fond 3D » où elle est
//!   envoyée ;
//! - chaque **séquence** peut avoir un **décor** (`Scenes3d::decors`, une liste d'objets) que tous
//!   ses plans montrent (`PlanScene::use_decor`) : on place un décor une fois, chaque plan y pose ses
//!   personnages et sa caméra. Le décor se modifie depuis n'importe quel plan de la séquence
//!   (« Modifier : le décor »), et tous les plans qui l'utilisent suivent.
//!
//! Tout est rangé dans `Document::scenes3d` et `Structure::scenes3d` : une étape d'annulation
//! (`Edit::Structure`) rétablit les scènes avec le reste, sans autre mécanisme. Enregistrement :
//! `scenes3d.json` à côté du manifeste (`scene3dstore.rs`), sans changer `VERSION`.
//!
//! Les fonctions de ce fichier sont pures (ni interface ni carte graphique) : elles se testent sans
//! fenêtre.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::camera::{Camera, REF_FOCAL};
use crate::scene3dmath::{self as m, V3};

/// Numéro de version des scènes : chaque modification en prend un neuf, unique pour la séance. Un
/// rendu sait ainsi s'il date de la scène d'aujourd'hui, de celle d'avant l'annulation, ou d'une
/// autre : deux scènes qui portent le même numéro sont identiques.
static REV: AtomicU64 = AtomicU64::new(1);

pub fn next_rev() -> u64 {
    REV.fetch_add(1, Ordering::Relaxed)
}

/// Largeur, en mètres, du cadre de la caméra par défaut du plan à la distance de référence : la
/// feuille (le plan de référence de la caméra 2D) correspond au plan z = 0 de la scène, et ce plan
/// mesure `PLATE_WIDTH_M` mètres dans le cadre. C'est la seule échelle qui lie la scène (en mètres)
/// au plateau (en pixels de la feuille).
pub const PLATE_WIDTH_M: f32 = 6.0;

/// Nombre maximal d'objets par liste (un fichier abîmé ne gèle pas l'interface).
pub const MAX_OBJECTS: usize = 2000;
pub const MIN_STEPS: u32 = 2;
pub const MAX_STEPS: u32 = 40;

// ------------------------------------------------------------------------------------ objets

/// Position, rotation (degrés, ordre de `scene3dmath::rotation`) et taille d'un objet.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Xf {
    #[serde(rename = "position")]
    pub pos: V3,
    #[serde(rename = "rotation", default)]
    pub rot: V3,
    #[serde(rename = "taille")]
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

    /// Valeurs finies et bornées (un fichier abîmé, une saisie absurde).
    pub fn sanitized(self) -> Xf {
        let fin = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        Xf {
            pos: self.pos.map(|v| fin(v, -1e5, 1e5, 0.0)),
            rot: self.rot.map(|v| fin(v, -1e4, 1e4, 0.0)),
            size: self.size.map(|v| {
                let v = fin(v, -1e4, 1e4, 1.0);
                // Une taille nulle rendrait la matrice non inversible (plus de sélection possible).
                if v.abs() < 1e-3 { 1e-3_f32.copysign(if v == 0.0 { 1.0 } else { v }) } else { v }
            }),
        }
    }
}

/// Les articulations du mannequin (rotations relatives à l'os parent, voir `scene3dmesh`).
pub const JOINTS: usize = 10;
pub const JOINT_NAMES: [&str; JOINTS] = [
    "Colonne",
    "Tête",
    "Épaule gauche",
    "Coude gauche",
    "Épaule droite",
    "Coude droit",
    "Hanche gauche",
    "Genou gauche",
    "Hanche droite",
    "Genou droit",
];
pub mod joint {
    pub const SPINE: usize = 0;
    pub const HEAD: usize = 1;
    pub const SHOULDER_L: usize = 2;
    pub const ELBOW_L: usize = 3;
    pub const SHOULDER_R: usize = 4;
    pub const ELBOW_R: usize = 5;
    pub const HIP_L: usize = 6;
    pub const KNEE_L: usize = 7;
    pub const HIP_R: usize = 8;
    pub const KNEE_R: usize = 9;
}

/// Poses proposées pour le mannequin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pose {
    #[serde(rename = "debout")]
    Standing,
    #[serde(rename = "assis")]
    Sitting,
    #[serde(rename = "marche")]
    Walking,
    #[serde(rename = "bras_leves")]
    ArmsUp,
    #[serde(rename = "libre")]
    Custom,
}

impl Pose {
    pub const PRESETS: [Pose; 4] = [Pose::Standing, Pose::Sitting, Pose::Walking, Pose::ArmsUp];

    pub fn label(self) -> &'static str {
        match self {
            Pose::Standing => "Debout",
            Pose::Sitting => "Assis",
            Pose::Walking => "Marche",
            Pose::ArmsUp => "Bras levés",
            Pose::Custom => "Libre",
        }
    }

    /// Angles des articulations de la pose (degrés [avant+, torsion, écarter+]). Pour un membre, X
    /// positif porte son bout vers l'avant (le genou qui plie est donc négatif) ; pour la colonne et
    /// la tête, X positif penche vers l'avant ; Z positif écarte un membre du corps (ou penche la
    /// colonne vers la gauche du personnage).
    pub fn joints(self) -> [[f32; 3]; JOINTS] {
        use joint::*;
        let mut j = [[0.0; 3]; JOINTS];
        match self {
            Pose::Standing | Pose::Custom => {
                j[SHOULDER_L] = [0.0, 0.0, 7.0];
                j[SHOULDER_R] = [0.0, 0.0, 7.0];
                j[ELBOW_L] = [8.0, 0.0, 0.0];
                j[ELBOW_R] = [8.0, 0.0, 0.0];
            }
            Pose::Sitting => {
                j[SPINE] = [4.0, 0.0, 0.0];
                j[HIP_L] = [90.0, 0.0, 3.0];
                j[HIP_R] = [90.0, 0.0, 3.0];
                j[KNEE_L] = [-90.0, 0.0, 0.0];
                j[KNEE_R] = [-90.0, 0.0, 0.0];
                j[SHOULDER_L] = [30.0, 0.0, 6.0];
                j[SHOULDER_R] = [30.0, 0.0, 6.0];
                j[ELBOW_L] = [55.0, 0.0, 0.0];
                j[ELBOW_R] = [55.0, 0.0, 0.0];
            }
            Pose::Walking => {
                j[SPINE] = [3.0, 4.0, 0.0];
                j[HIP_L] = [-22.0, 0.0, 2.0];
                j[KNEE_L] = [-28.0, 0.0, 0.0];
                j[HIP_R] = [26.0, 0.0, 2.0];
                j[KNEE_R] = [-6.0, 0.0, 0.0];
                j[SHOULDER_L] = [24.0, 0.0, 6.0];
                j[ELBOW_L] = [22.0, 0.0, 0.0];
                j[SHOULDER_R] = [-22.0, 0.0, 6.0];
                j[ELBOW_R] = [12.0, 0.0, 0.0];
            }
            Pose::ArmsUp => {
                j[SHOULDER_L] = [165.0, 0.0, 12.0];
                j[SHOULDER_R] = [165.0, 0.0, 12.0];
                j[ELBOW_L] = [6.0, 0.0, 0.0];
                j[ELBOW_R] = [6.0, 0.0, 0.0];
                j[HEAD] = [-8.0, 0.0, 0.0];
            }
        }
        j
    }
}

/// Un mannequin : silhouette humaine de capsules articulées. Les pieds sont toujours posés sur le sol
/// de l'objet (y = 0 dans son repère), quelle que soit la pose.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mannequin {
    /// Taille debout, en mètres.
    #[serde(rename = "taille")]
    pub height: f32,
    #[serde(rename = "pose")]
    pub pose: Pose,
    #[serde(rename = "articulations")]
    pub joints: [[f32; 3]; JOINTS],
}

impl Mannequin {
    pub const MIN_HEIGHT: f32 = 0.3;
    pub const MAX_HEIGHT: f32 = 4.0;

    pub fn new(pose: Pose, height: f32) -> Mannequin {
        Mannequin { height: height.clamp(Self::MIN_HEIGHT, Self::MAX_HEIGHT), pose, joints: pose.joints() }
    }

    pub fn set_pose(&mut self, pose: Pose) {
        self.pose = pose;
        self.joints = pose.joints();
    }

    /// Une articulation changée à la main : la pose devient libre.
    pub fn set_joint(&mut self, which: usize, rot: V3) {
        if which < JOINTS {
            self.joints[which] = rot.map(|v| if v.is_finite() { v.clamp(-360.0, 360.0) } else { 0.0 });
            self.pose = Pose::Custom;
        }
    }

    pub fn sanitized(mut self) -> Mannequin {
        self.height = if self.height.is_finite() { self.height.clamp(Self::MIN_HEIGHT, Self::MAX_HEIGHT) } else { 1.75 };
        for j in &mut self.joints {
            *j = j.map(|v| if v.is_finite() { v.clamp(-360.0, 360.0) } else { 0.0 });
        }
        self
    }
}

/// La forme d'un objet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "forme")]
pub enum Shape3d {
    #[serde(rename = "cube")]
    Cube,
    #[serde(rename = "plan")]
    Plane,
    #[serde(rename = "cylindre")]
    Cylinder,
    #[serde(rename = "sphere")]
    Sphere,
    #[serde(rename = "cone")]
    Cone,
    #[serde(rename = "escalier")]
    Stairs {
        #[serde(rename = "marches")]
        steps: u32,
    },
    #[serde(rename = "mannequin")]
    Mannequin(Box<Mannequin>),
    /// Un modèle importé (`Document::models`, fichier `.pelure3d`).
    #[serde(rename = "modele")]
    Model {
        #[serde(rename = "numero")]
        id: u64,
    },
}

impl Shape3d {
    /// Les formes qu'on ajoute d'un clic (le modèle importé, lui, vient du menu Fichier).
    pub fn basics() -> [Shape3d; 7] {
        [
            Shape3d::Cube,
            Shape3d::Plane,
            Shape3d::Cylinder,
            Shape3d::Sphere,
            Shape3d::Cone,
            Shape3d::Stairs { steps: 5 },
            Shape3d::Mannequin(Box::new(Mannequin::new(Pose::Standing, 1.75))),
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Shape3d::Cube => "Cube",
            Shape3d::Plane => "Plan",
            Shape3d::Cylinder => "Cylindre",
            Shape3d::Sphere => "Sphère",
            Shape3d::Cone => "Cône",
            Shape3d::Stairs { .. } => "Escalier",
            Shape3d::Mannequin(_) => "Mannequin",
            Shape3d::Model { .. } => "Modèle",
        }
    }

    /// Identifiant de commande (`scene3d.ajouter_<…>`) de la forme.
    pub fn action(&self) -> &'static str {
        match self {
            Shape3d::Cube => "scene3d.ajouter_cube",
            Shape3d::Plane => "scene3d.ajouter_plan",
            Shape3d::Cylinder => "scene3d.ajouter_cylindre",
            Shape3d::Sphere => "scene3d.ajouter_sphere",
            Shape3d::Cone => "scene3d.ajouter_cone",
            Shape3d::Stairs { .. } => "scene3d.ajouter_escalier",
            Shape3d::Mannequin(_) => "scene3d.ajouter_mannequin",
            Shape3d::Model { .. } => "scene3d.ajouter_modele",
        }
    }

    /// Taille et hauteur de départ d'un objet neuf de cette forme (posé sur le sol).
    pub fn default_size(&self) -> V3 {
        match self {
            Shape3d::Cube | Shape3d::Sphere => [1.0, 1.0, 1.0],
            Shape3d::Plane => [10.0, 1.0, 10.0],
            Shape3d::Cylinder => [0.6, 1.0, 0.6],
            Shape3d::Cone => [0.8, 1.0, 0.8],
            Shape3d::Stairs { .. } => [1.2, 0.9, 1.8],
            Shape3d::Mannequin(_) | Shape3d::Model { .. } => [1.0, 1.0, 1.0],
        }
    }

    /// Hauteur, dans le repère de l'objet, du bas de sa forme (pour la poser sur le sol) ; 0 pour les
    /// formes dont l'origine est déjà au bas.
    pub fn local_bottom(&self) -> f32 {
        match self {
            Shape3d::Plane | Shape3d::Mannequin(_) | Shape3d::Model { .. } => 0.0,
            _ => -0.5,
        }
    }

    pub fn sanitized(self) -> Shape3d {
        match self {
            Shape3d::Stairs { steps } => Shape3d::Stairs { steps: steps.clamp(MIN_STEPS, MAX_STEPS) },
            Shape3d::Mannequin(mq) => Shape3d::Mannequin(Box::new(mq.sanitized())),
            other => other,
        }
    }
}

/// Un objet de la scène.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Obj3d {
    pub id: u64,
    #[serde(rename = "nom")]
    pub name: String,
    #[serde(flatten)]
    pub shape: Shape3d,
    #[serde(rename = "placement")]
    pub xf: Xf,
    #[serde(rename = "couleur")]
    pub color: [u8; 3],
    /// Ombré en niveaux de gris, quel que soit le style du plan.
    #[serde(rename = "gris", default, skip_serializing_if = "is_false")]
    pub gray: bool,
    #[serde(rename = "cache", default, skip_serializing_if = "is_false")]
    pub hidden: bool,
    #[serde(rename = "verrouille", default, skip_serializing_if = "is_false")]
    pub locked: bool,
    /// Objets du même groupe : choisis, déplacés, tournés ensemble.
    #[serde(rename = "groupe", default, skip_serializing_if = "Option::is_none")]
    pub group: Option<u64>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Couleur d'un objet neuf selon sa forme.
pub fn default_color(shape: &Shape3d) -> [u8; 3] {
    match shape {
        Shape3d::Plane => [150, 150, 145],
        Shape3d::Mannequin(_) => [205, 190, 170],
        Shape3d::Stairs { .. } => [175, 165, 150],
        Shape3d::Model { .. } => [180, 180, 185],
        _ => [200, 200, 205],
    }
}

// ------------------------------------------------------------------------- lumière, caméra, style

/// Lumière directionnelle unique, plus une lumière ambiante.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Light {
    /// Degrés autour de la scène (0 : la lumière vient de +Z, du côté de la caméra par défaut).
    #[serde(rename = "azimut")]
    pub azimuth: f32,
    /// Hauteur au-dessus de l'horizon (90 : à la verticale).
    #[serde(rename = "hauteur")]
    pub elevation: f32,
    #[serde(rename = "intensite")]
    pub intensity: f32,
    #[serde(rename = "ambiante")]
    pub ambient: f32,
}

impl Default for Light {
    fn default() -> Light {
        Light { azimuth: 35.0, elevation: 50.0, intensity: 0.85, ambient: 0.35 }
    }
}

impl Light {
    /// Direction vers la lumière (longueur 1).
    pub fn direction(&self) -> V3 {
        let (a, e) = (self.azimuth.to_radians(), self.elevation.clamp(-90.0, 90.0).to_radians());
        [e.cos() * a.sin(), e.sin(), e.cos() * a.cos()]
    }

    pub fn sanitized(self) -> Light {
        let fin = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        Light {
            azimuth: fin(self.azimuth, -3600.0, 3600.0, 35.0),
            elevation: fin(self.elevation, -90.0, 90.0, 50.0),
            intensity: fin(self.intensity, 0.0, 3.0, 0.85),
            ambient: fin(self.ambient, 0.0, 1.0, 0.35),
        }
    }
}

/// La caméra du plateau : une prise de vue posée dans la scène. Mêmes conventions que la caméra 2D
/// du plan (`camera.rs`) : panoramique positif vers la droite, bascule positive vers le haut,
/// rotation (roulis) positive dans le sens horaire à l'écran ; la focale en mm sur une pellicule de
/// 36 mm de large, donc le même angle de champ que `camera::angle_of_view`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SceneCam {
    #[serde(rename = "position")]
    pub pos: V3,
    #[serde(rename = "panoramique")]
    pub pan: f32,
    #[serde(rename = "bascule")]
    pub tilt: f32,
    #[serde(rename = "rotation", default)]
    pub roll: f32,
    #[serde(rename = "focale")]
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
    /// Vers où la caméra regarde. À panoramique et bascule nuls : vers −Z, comme la caméra 2D qui
    /// regarde la feuille de face.
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
        // Rotation du cadre dans le sens horaire (vu de derrière la caméra) : la droite du cadre
        // descend, le haut penche vers la droite.
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

    /// Les quatre coins du cadre à la distance `d` devant la caméra : haut-gauche, haut-droite,
    /// bas-droite, bas-gauche. `frame` : l'image finale (pour les proportions).
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

    /// Mètres par pixel de la feuille quand le cadre de `frame_w` pixels mesure `PLATE_WIDTH_M` mètres
    /// à la distance de référence de la caméra 2D.
    pub fn meters_per_px(frame_w: usize) -> f32 {
        PLATE_WIDTH_M / frame_w.max(1) as f32
    }

    /// La caméra 2D du plan, à l'image où elle est, posée dans la scène : le plan z = 0 de la scène
    /// est la feuille (le plan de référence), à l'échelle de `meters_per_px`. Même focale, même
    /// panoramique, même bascule, même rotation, et l'œil là où `stage3d::World::eye` le met : la
    /// scène vue par cette caméra et la feuille vue par la caméra 2D se superposent.
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

/// Style de rendu (vue d'édition et envoi vers un calque).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderStyle {
    /// Couleurs des objets, ombrées par la lumière.
    #[serde(rename = "couleur")]
    Color,
    /// Niveaux de gris ombrés.
    #[serde(rename = "gris")]
    Gray,
    /// Contours noirs sur des aplats clairs : le plus facile à dessiner par-dessus.
    #[serde(rename = "contours")]
    Outline,
}

impl RenderStyle {
    pub const ALL: [RenderStyle; 3] = [RenderStyle::Color, RenderStyle::Gray, RenderStyle::Outline];

    pub fn label(self) -> &'static str {
        match self {
            RenderStyle::Color => "Couleurs",
            RenderStyle::Gray => "Niveaux de gris",
            RenderStyle::Outline => "Contours",
        }
    }
}

/// Zone rendue dans le calque : le cadre seul, ou toute la feuille (la caméra 2D peut alors
/// s'éloigner, se décaler ou tourner sans sortir du rendu).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderArea {
    #[serde(rename = "cadre")]
    Frame,
    #[serde(rename = "feuille")]
    Sheet,
}

impl RenderArea {
    pub fn label(self) -> &'static str {
        match self {
            RenderArea::Frame => "Le cadre",
            RenderArea::Sheet => "Toute la feuille",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RenderSettings {
    #[serde(rename = "style")]
    pub style: RenderStyle,
    /// Traits noirs autour des objets et aux cassures (toujours avec le style Contours).
    #[serde(rename = "traits", default)]
    pub lines: bool,
    /// Épaisseur des traits, en pixels de l'image.
    #[serde(rename = "epaisseur", default = "default_line_px")]
    pub line_px: f32,
    #[serde(rename = "zone", default = "default_area")]
    pub area: RenderArea,
}

fn default_line_px() -> f32 {
    2.0
}
fn default_area() -> RenderArea {
    RenderArea::Sheet
}

impl Default for RenderSettings {
    fn default() -> RenderSettings {
        RenderSettings { style: RenderStyle::Gray, lines: true, line_px: 2.0, area: RenderArea::Sheet }
    }
}

impl RenderSettings {
    pub fn sanitized(mut self) -> RenderSettings {
        self.line_px = if self.line_px.is_finite() { self.line_px.clamp(0.5, 12.0) } else { 2.0 };
        self
    }

    /// Les traits sont dessinés (style Contours, ou option « Traits »).
    pub fn has_lines(&self) -> bool {
        self.lines || self.style == RenderStyle::Outline
    }
}

// --------------------------------------------------------------------------------- rangement

/// La scène d'un plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanScene {
    /// Les objets propres au plan (personnages, accessoires).
    #[serde(rename = "objets", default)]
    pub objects: Vec<Obj3d>,
    /// Montrer le décor de la séquence.
    #[serde(rename = "decor", default = "yes")]
    pub use_decor: bool,
    #[serde(rename = "camera", default)]
    pub cam: SceneCam,
    #[serde(rename = "lumiere", default)]
    pub light: Light,
    #[serde(rename = "rendu", default)]
    pub render: RenderSettings,
    /// Le dessin du calque « Fond 3D » où la scène est envoyée (rien : pas encore envoyée). Le
    /// calque se retrouve par ce dessin ; s'il est supprimé, le lien se défait.
    #[serde(rename = "dessin_fond", default, skip_serializing_if = "Option::is_none")]
    pub bake_cel: Option<u64>,
    /// Remettre à jour le calque quand la scène change.
    #[serde(rename = "suivre", default = "yes")]
    pub follow: bool,
}

fn yes() -> bool {
    true
}

impl Default for PlanScene {
    fn default() -> PlanScene {
        PlanScene {
            objects: Vec::new(),
            use_decor: true,
            cam: SceneCam::default(),
            light: Light::default(),
            render: RenderSettings::default(),
            bake_cel: None,
            follow: true,
        }
    }
}

/// Toutes les scènes du projet.
#[derive(Clone, Debug, Default)]
pub struct Scenes3d {
    /// Par numéro de plan.
    pub plans: BTreeMap<u64, PlanScene>,
    /// Les décors, par numéro de séquence.
    pub decors: BTreeMap<u64, Vec<Obj3d>>,
    /// Prochain numéro d'objet (commun aux plans et aux décors : un numéro désigne un seul objet).
    pub next_id: u64,
    /// Version de la scène (voir `next_rev`) ; pas enregistrée.
    pub rev: u64,
}

impl PartialEq for Scenes3d {
    /// Même contenu (la version et le compteur ne comptent pas).
    fn eq(&self, other: &Scenes3d) -> bool {
        self.plans == other.plans && self.decors == other.decors
    }
}

/// Où l'on modifie : les objets du plan, ou le décor de sa séquence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Target {
    #[default]
    Plan,
    Decor,
}

impl Scenes3d {
    pub fn is_empty(&self) -> bool {
        self.plans.is_empty() && self.decors.values().all(Vec::is_empty)
    }

    /// Un numéro d'objet neuf.
    pub fn alloc_id(&mut self) -> u64 {
        let used = self
            .plans
            .values()
            .flat_map(|p| p.objects.iter())
            .chain(self.decors.values().flatten())
            .map(|o| o.id)
            .max()
            .unwrap_or(0);
        self.next_id = self.next_id.max(used + 1).max(1);
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn touch(&mut self) {
        self.rev = next_rev();
    }

    /// Les modèles importés que les scènes montrent.
    pub fn models_used(&self) -> impl Iterator<Item = u64> + '_ {
        self.plans.values().flat_map(|p| p.objects.iter()).chain(self.decors.values().flatten()).filter_map(|o| match o.shape {
            Shape3d::Model { id } => Some(id),
            _ => None,
        })
    }

    pub fn plan(&self, panel: u64) -> Option<&PlanScene> {
        self.plans.get(&panel)
    }

    /// La scène du plan, créée vide si elle n'existe pas.
    pub fn plan_mut(&mut self, panel: u64) -> &mut PlanScene {
        self.plans.entry(panel).or_default()
    }

    /// Les objets que le plan `panel` (de la séquence `seq`) montre : ceux du décor (s'il est utilisé)
    /// puis les siens, chacun avec « est du décor ».
    pub fn visible_objects(&self, panel: u64, seq: u64) -> Vec<(&Obj3d, bool)> {
        let plan = self.plans.get(&panel);
        let mut out: Vec<(&Obj3d, bool)> = Vec::new();
        if plan.is_none_or(|p| p.use_decor) {
            if let Some(d) = self.decors.get(&seq) {
                out.extend(d.iter().map(|o| (o, true)));
            }
        }
        if let Some(p) = plan {
            out.extend(p.objects.iter().map(|o| (o, false)));
        }
        out
    }

    /// La liste que `target` désigne pour ce plan.
    pub fn list(&self, panel: u64, seq: u64, target: Target) -> &[Obj3d] {
        match target {
            Target::Plan => self.plans.get(&panel).map_or(&[], |p| &p.objects),
            Target::Decor => self.decors.get(&seq).map_or(&[], |d| d.as_slice()),
        }
    }

    pub fn list_mut(&mut self, panel: u64, seq: u64, target: Target) -> &mut Vec<Obj3d> {
        match target {
            Target::Plan => &mut self.plan_mut(panel).objects,
            Target::Decor => self.decors.entry(seq).or_default(),
        }
    }

    /// Un objet par son numéro, dans le plan ou son décor.
    pub fn find(&self, panel: u64, seq: u64, id: u64) -> Option<(&Obj3d, Target)> {
        [Target::Plan, Target::Decor].into_iter().find_map(|t| self.list(panel, seq, t).iter().find(|o| o.id == id).map(|o| (o, t)))
    }

    pub fn find_mut(&mut self, panel: u64, seq: u64, id: u64) -> Option<&mut Obj3d> {
        let target = self.find(panel, seq, id)?.1;
        self.list_mut(panel, seq, target).iter_mut().find(|o| o.id == id)
    }

    /// Ajoute un objet de forme `shape` à `at` (x et z ; il est posé sur le sol). Renvoie son numéro.
    pub fn add(&mut self, panel: u64, seq: u64, target: Target, shape: Shape3d, at: V3) -> Option<u64> {
        if self.list(panel, seq, target).len() >= MAX_OBJECTS {
            return None;
        }
        let shape = shape.sanitized();
        let size = shape.default_size();
        let xf = Xf { pos: [at[0], -shape.local_bottom() * size[1], at[2]], rot: [0.0; 3], size };
        let id = self.alloc_id();
        let same = self.list(panel, seq, target).iter().filter(|o| o.shape.label() == shape.label()).count();
        let name = format!("{} {}", shape.label(), same + 1);
        let color = default_color(&shape);
        self.list_mut(panel, seq, target).push(Obj3d { id, name, shape, xf, color, gray: false, hidden: false, locked: false, group: None });
        self.touch();
        Some(id)
    }

    /// Retire les objets `ids`.
    pub fn delete(&mut self, panel: u64, seq: u64, ids: &[u64]) -> usize {
        let mut n = 0;
        for t in [Target::Plan, Target::Decor] {
            let list = self.list_mut(panel, seq, t);
            let before = list.len();
            list.retain(|o| !ids.contains(&o.id));
            n += before - list.len();
        }
        if n > 0 {
            self.touch();
        }
        n
    }

    /// Copie les objets `ids` (décalés de `offset`), les copies d'un même groupe formant un groupe
    /// neuf. Renvoie les numéros des copies.
    pub fn duplicate(&mut self, panel: u64, seq: u64, ids: &[u64], offset: V3) -> Vec<u64> {
        let mut copies = Vec::new();
        let mut groups: BTreeMap<u64, u64> = BTreeMap::new();
        for t in [Target::Plan, Target::Decor] {
            let originals: Vec<Obj3d> = self.list(panel, seq, t).iter().filter(|o| ids.contains(&o.id)).cloned().collect();
            for mut o in originals {
                if self.list(panel, seq, t).len() >= MAX_OBJECTS {
                    break;
                }
                o.id = self.alloc_id();
                o.xf.pos = m::add(o.xf.pos, offset);
                o.name = copy_name(&o.name);
                if let Some(g) = o.group {
                    let fresh = match groups.get(&g) {
                        Some(f) => *f,
                        None => {
                            let f = self.alloc_id();
                            groups.insert(g, f);
                            f
                        }
                    };
                    o.group = Some(fresh);
                }
                copies.push(o.id);
                self.list_mut(panel, seq, t).push(o);
            }
        }
        if !copies.is_empty() {
            self.touch();
        }
        copies
    }

    /// Réunit les objets `ids` (au moins deux) en un groupe.
    pub fn group(&mut self, panel: u64, seq: u64, ids: &[u64]) -> Option<u64> {
        let found: Vec<u64> = ids.iter().copied().filter(|id| self.find(panel, seq, *id).is_some()).collect();
        if found.len() < 2 {
            return None;
        }
        let g = self.alloc_id();
        for id in found {
            if let Some(o) = self.find_mut(panel, seq, id) {
                o.group = Some(g);
            }
        }
        self.touch();
        Some(g)
    }

    /// Défait le groupe des objets `ids`.
    pub fn ungroup(&mut self, panel: u64, seq: u64, ids: &[u64]) -> usize {
        let mut n = 0;
        for id in ids {
            if let Some(o) = self.find_mut(panel, seq, *id) {
                if o.group.take().is_some() {
                    n += 1;
                }
            }
        }
        if n > 0 {
            self.touch();
        }
        n
    }

    /// `ids` complétés par les objets de leurs groupes.
    pub fn expand_groups(&self, panel: u64, seq: u64, ids: &[u64]) -> Vec<u64> {
        let mut out: Vec<u64> = ids.to_vec();
        let groups: Vec<u64> = ids.iter().filter_map(|id| self.find(panel, seq, *id).and_then(|(o, _)| o.group)).collect();
        for t in [Target::Plan, Target::Decor] {
            for o in self.list(panel, seq, t) {
                if o.group.is_some_and(|g| groups.contains(&g)) && !out.contains(&o.id) {
                    out.push(o.id);
                }
            }
        }
        out
    }

    /// Remet chaque donnée dans des bornes sûres (à la lecture d'un fichier, ou après une saisie).
    pub fn sanitize(&mut self) {
        for o in self.plans.values_mut().flat_map(|p| p.objects.iter_mut()).chain(self.decors.values_mut().flatten()) {
            o.xf = o.xf.sanitized();
            let shape = std::mem::replace(&mut o.shape, Shape3d::Cube);
            o.shape = shape.sanitized();
            if o.name.chars().count() > 120 {
                o.name = o.name.chars().take(120).collect();
            }
        }
        for p in self.plans.values_mut() {
            p.cam = p.cam.sanitized();
            p.light = p.light.sanitized();
            p.render = p.render.sanitized();
            p.objects.truncate(MAX_OBJECTS);
        }
        for d in self.decors.values_mut() {
            d.truncate(MAX_OBJECTS);
        }
        let used = self.plans.values().flat_map(|p| p.objects.iter()).chain(self.decors.values().flatten()).map(|o| o.id).max().unwrap_or(0);
        self.next_id = self.next_id.max(used + 1).max(1);
    }
}

/// Nom d'une copie : « Cube 1 » → « Cube 1 copie ».
fn copy_name(name: &str) -> String {
    if name.ends_with(" copie") || name.contains(" copie ") {
        format!("{name} +")
    } else {
        format!("{name} copie")
    }
}
