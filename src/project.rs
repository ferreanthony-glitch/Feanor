//! Projet en dossier (format 3).
//!
//! ```text
//! MonFilm/
//!   MonFilm.pelure        manifeste JSON lisible : réglages, séquences > scènes > panneaux > calques,
//!                         pistes son
//!   dessins/<id>.png      dessins bitmap
//!   dessins/<id>.json     dessins vectoriels
//!   sons/<id>.<ext>       fichiers son importés, copiés tels quels
//!   modeles/<id>.pelure3d modèles 3D de repère (format 5), maillage soudé (voir `model3d`)
//! ```
//!
//! Un enregistrement n'écrit que les dessins modifiés depuis le précédent, puis le manifeste.
//! Chaque fichier est écrit à côté puis renommé : un plantage en cours d'enregistrement laisse
//! l'ancienne version intacte. Les dessins qui ne servent plus sont effacés en dernier.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::document::{
    fresh_layer_id, AudioTrack, Behaviour, Cel, CelId, Document, Layer, LayerKind, LayerTransform, Panel, Ref3d, Scene, SceneWide,
    Sequence, Sound, SoundClip, Structure,
};
use crate::brushtex::{GrayImage, TextureKind};
use crate::camera::{Camera, CameraKey};
use crate::guides::Guide;
use crate::io::encode_png;
use crate::model3d::Model3d;
use crate::view3d::{Cam3d, MAX_ELEVATION};
use crate::raster::Raster;
use crate::vector::VectorDrawing;

pub const FORMAT: &str = "pelure-projet";
/// 4 (phase 2b) : la feuille de dessin est plus grande que le cadre de la caméra (`cadre`).
/// 5 (phase 2d) : modèles 3D (`modeles`) et repère 3D par panneau (`repere3d`).
/// 6 (phase 5d) : textes dans les dessins vectoriels (éléments `Text`, inconnus des versions
/// précédentes : elles refusent le projet plutôt que de perdre ces dessins).
/// 7 (phase 6f) : calques de toute la scène (`toute_la_scene`) ; une version précédente en ferait
/// des calques séparés qui partagent un même dessin.
/// 8 (phase 7) : notes de la planche (`planche`), traits vectoriels texturés (`pinceau` dans les
/// dessins, textures dans `textures/`), qu'une version précédente perdrait.
/// 9 (phase 8) : notes attachées à une vignette (`attache`), textes de la planche
/// (`planche_textes`), mode de fusion des calques (`fusion`).
/// 10 (phase 9) : caméra avancée (`avance`) et adoucis en images (`adoucir_entree`, `adoucir_sortie`).
/// 11 (9f) : bascule et panoramique de la caméra (`bascule`, `panoramique`), qu'une version
/// précédente ignorerait (le film changerait sans prévenir).
/// 12 (13a) : un seul panneau par scène, le plan (les panneaux d'un projet plus ancien sont mis
/// bout à bout à l'ouverture), et marques d'image (`marques`) qu'une version précédente perdrait.
/// 13 (13b) : réglages de l'en-tête des calques (pelure d'oignon du calque, préserver la
/// transparence, pochoir, position verrouillée, groupe, ligne réduite) et groupes de couleur
/// (`groupes`), qu'une version précédente perdrait.
/// 14 (13b-5) : dossiers de calques (`dossier`, `dans_dossier`) ; une version précédente en
/// ferait des calques vides et perdrait l'opacité et l'affichage des dossiers.
/// 15 (13c) : pré / post-comportement des calques (`avant`, `apres`), qu'une version
/// précédente ignorerait (le film changerait sans prévenir) ; cassure et création automatiques
/// (`cassure_auto`, `creation_auto`) ; noms d'instance (`noms`).
/// 16 (13d) : signets et points d'entrée / sortie des plans (`signets`, `entree`, `sortie`),
/// image de départ du projet (`image_de_depart`), marques de couleur des dessins
/// (`marques_couleur`).
/// 17 (13e) : arrière-plan (`fond` : aucun, couleur ou damier, couleur B), notes dessinées des
/// plans (`notes_dessinees`), verrou du storyboard et propriétés du projet.
/// 18 (13g-2) : rôles des images d'un calque (`roles` : image → clé, breakdown, intervalle…).
/// 19 (13i) : pile d'effets d'un calque (`effets` : sorte, actif, valeurs, clés).
/// 20 (13j) : calque CTG (`ctg` : rangs des calques sources).
/// 21 (13k) : calque vidéo (`video` : id, nom, images, ips, début, extension ; images dans
/// `videos/<id>/`).
/// 22 (2026-10-18) : étirement des calques (`etirement_x`, `etirement_y`), qu'une version précédente
/// ignorerait (le calque reprendrait ses proportions sans prévenir).
/// 23 (2026-10-18) : fond sous le trait (`fond` d'un calque ; la strate Fond d'un dessin dans
/// `dessins/<id>_fond.png`).
/// 24 (2026-10-18, rendu des brosses, étape 5) : les traits vectoriels gardent leur `trace`
/// (appui et instant par point en entiers — `appui` au 1/65535, `instants` en dixièmes de
/// milliseconde depuis le premier —, toucher, couleur B) et leur réglage complet, et sont rejoués
/// par le moteur bitmap. Comme pour tout nouveau format, une version précédente de Pelure **refuse**
/// d'ouvrir le projet (`load` : « créé par une version plus récente ») ; elle ne le lit pas en
/// rendu approché.
pub const VERSION: u32 = 24;
const DRAWINGS: &str = "dessins";
const SOUNDS: &str = "sons";
/// Images des calques vidéo (13k) : `videos/<id>/000001.jpg`…
const VIDEOS: &str = "videos";
/// Modèles 3D de repère (2d), dans le format de Pelure.
const MODELS: &str = "modeles";
/// Textures des traits vectoriels texturés (7b) : `pointe_<clé>.png`, `papier_<clé>.png`, la clé
/// venant du contenu (une texture ne change jamais sous la même clé).
const TEXTURES: &str = "textures";
/// Formats de son acceptés à l'import.
pub const SOUND_EXTS: [&str; 5] = ["wav", "mp3", "flac", "ogg", "oga"];

#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    /// Taille de la feuille de dessin (et de chaque fichier de `dessins/`).
    largeur: usize,
    hauteur: usize,
    /// Cadre de la caméra = taille de l'image finale. Absent (format 3) : la feuille était le
    /// cadre ; elle est alors doublée à l'ouverture, dessin au centre.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cadre: Option<[usize; 2]>,
    cadence: u32,
    papier: [u8; 3],
    sequences: Vec<SequenceEntry>,
    /// Fichiers son, copiés tels quels dans `sons/` (ajouté en 1b ; absent = aucun son).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sons: Vec<SoundEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pistes_son: Vec<TrackEntry>,
    /// Guides de perspective (ajouté en 1c).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    guides: Vec<Guide>,
    /// Modèles 3D de repère (format 5).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    modeles: Vec<ModelEntry>,
    /// Notes de la planche (format 8) : traits de la vue Vignettes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    planche: Vec<NoteEntry>,
    /// Textes de la planche (format 9).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    planche_textes: Vec<NoteTextEntry>,
    /// Groupes de couleur des calques (format 13 ; absents : ceux d'un projet neuf).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    groupes: Vec<GroupEntry>,
    /// Numéro de la première image (format 16 ; absent : 1).
    #[serde(default = "one_usize", skip_serializing_if = "is_one_usize")]
    image_de_depart: usize,
    /// Arrière-plan (format 17 ; absent : la couleur du papier).
    #[serde(default, skip_serializing_if = "BackgroundEntry::is_default")]
    fond: BackgroundEntry,
    /// Volume général du son, piste maîtresse (format 17 ; absent : 1).
    #[serde(default = "one", skip_serializing_if = "is_one_f32")]
    volume_general: f32,
    /// Storyboard verrouillé (format 17 ; absent : non).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    storyboard_verrouille: bool,
    /// Propriétés du projet (format 17 ; absentes : vides).
    #[serde(default, skip_serializing_if = "PropertiesEntry::is_empty")]
    proprietes: PropertiesEntry,
}

/// Propriétés du projet (format 17). Dates en secondes depuis 1970.
#[derive(Serialize, Deserialize, Default, Clone, PartialEq)]
struct PropertiesEntry {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    titre: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    informations: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    auteur: String,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    cree: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    modifie: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    notes: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    copyright: String,
    /// Couleur des fonds sous le trait qu'on allume (format 23) ; absente : blanc.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fond: Option<[u8; 3]>,
}

impl PropertiesEntry {
    fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

fn is_zero_u64(v: &u64) -> bool {
    *v == 0
}

fn is_one_f32(v: &f32) -> bool {
    *v == 1.0
}

/// Arrière-plan (format 17) : sorte et couleur B du damier (la couleur A est `papier`).
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
struct BackgroundEntry {
    #[serde(default)]
    sorte: crate::document::BackgroundKind,
    #[serde(default = "default_paper_b")]
    couleur_b: [u8; 3],
}

impl Default for BackgroundEntry {
    fn default() -> Self {
        Self { sorte: Default::default(), couleur_b: crate::document::DEFAULT_PAPER_B }
    }
}

impl BackgroundEntry {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

fn default_paper_b() -> [u8; 3] {
    crate::document::DEFAULT_PAPER_B
}

fn one_usize() -> usize {
    1
}

fn is_one_usize(v: &usize) -> bool {
    *v == 1
}

/// Un groupe de couleur des calques (format 13), dans l'ordre des numéros de groupe.
#[derive(Serialize, Deserialize)]
struct GroupEntry {
    nom: String,
    couleur: [u8; 3],
    #[serde(default = "yes")]
    montre: bool,
}

/// Un trait de note : couleur et points (x, y, rayon), en coordonnées de la planche, ou, s'il
/// est attaché à une vignette (format 9, `attache` : id du panneau), relatifs à elle (en
/// largeurs de vignette).
#[derive(Serialize, Deserialize)]
struct NoteEntry {
    couleur: [u8; 3],
    points: Vec<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    attache: Option<u64>,
}

/// Un texte de note (format 9) : coin haut gauche, taille, mêmes règles d'attache.
#[derive(Serialize, Deserialize)]
struct NoteTextEntry {
    texte: String,
    position: [f32; 2],
    taille: f32,
    couleur: [u8; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    attache: Option<u64>,
}

#[derive(Serialize, Deserialize)]
struct ModelEntry {
    id: u64,
    /// Nom du fichier d'origine (affiché).
    #[serde(default)]
    nom: String,
}

/// Repère 3D d'un panneau (format 5) : modèle, caméra en orbite, affichage.
#[derive(Serialize, Deserialize)]
struct Ref3dEntry {
    modele: u64,
    cible: [f32; 3],
    azimut: f32,
    elevation: f32,
    distance: f32,
    focale: f32,
    #[serde(default = "yes")]
    visible: bool,
    #[serde(default = "yes")]
    aretes_cachees: bool,
    #[serde(default = "default_ref_color")]
    couleur: [u8; 3],
    #[serde(default = "one")]
    opacite: f32,
}

fn default_ref_color() -> [u8; 3] {
    [40, 150, 230]
}

#[derive(Serialize, Deserialize)]
struct SoundEntry {
    id: u64,
    /// Nom du fichier d'origine (affiché).
    #[serde(default)]
    nom: String,
    /// Fichier dans `sons/`.
    fichier: String,
}

#[derive(Serialize, Deserialize)]
struct TrackEntry {
    id: u64,
    #[serde(default)]
    nom: String,
    #[serde(default)]
    muette: bool,
    #[serde(default = "one")]
    volume: f32,
    #[serde(default)]
    extraits: Vec<ClipEntry>,
    /// Piste verrouillée (format 17 ; absent : non).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    verrouillee: bool,
}

/// Un extrait de son, en images du montage.
#[derive(Serialize, Deserialize)]
struct ClipEntry {
    id: u64,
    son: u64,
    debut: usize,
    /// Images sautées au début du son.
    #[serde(default)]
    decalage: usize,
    duree: usize,
}

#[derive(Serialize, Deserialize)]
struct SequenceEntry {
    id: u64,
    #[serde(default)]
    nom: String,
    scenes: Vec<SceneEntry>,
}

#[derive(Serialize, Deserialize)]
struct SceneEntry {
    id: u64,
    #[serde(default)]
    nom: String,
    panneaux: Vec<PanelEntry>,
}

#[derive(Serialize, Deserialize)]
struct PanelEntry {
    id: u64,
    #[serde(default)]
    duree: usize,
    #[serde(default)]
    action: String,
    #[serde(default)]
    dialogue: String,
    #[serde(default)]
    notes: String,
    calques: Vec<LayerEntry>,
    /// Images clés de la caméra (absent : caméra au centre de la feuille).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    camera: Vec<CameraEntry>,
    /// Repère 3D (format 5 ; absent : aucun).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    repere3d: Option<Ref3dEntry>,
    /// Marques d'image du plan (format 12 ; absent : aucune).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    marques: Vec<MarkEntry>,
    /// Signets, points d'entrée et de sortie (format 16 ; absents : aucun).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    signets: Vec<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    entree: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sortie: Option<usize>,
    /// Notes dessinées sur la timeline (format 17 ; absentes : aucune).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    notes_dessinees: Vec<TimeNoteEntry>,
}

/// Une note dessinée (format 17) : couleur (0 bleu, 1 rouge, 2 noir), épaisseur, points
/// (image, hauteur de 0 à 1).
#[derive(Serialize, Deserialize)]
struct TimeNoteEntry {
    couleur: u8,
    epaisseur: f32,
    points: Vec<[f32; 2]>,
}

/// Une marque d'image (13a) : une image de storyboard à l'image `image` du plan, avec ses textes.
#[derive(Serialize, Deserialize)]
struct MarkEntry {
    id: u64,
    image: usize,
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    couleur: u8,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    action: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    dialogue: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    notes: String,
}

fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}

#[derive(Serialize, Deserialize)]
struct TransformEntry {
    #[serde(default)]
    x: f32,
    #[serde(default)]
    y: f32,
    #[serde(default = "one")]
    echelle: f32,
    #[serde(default)]
    rotation: f32,
    /// Étirement en largeur et en hauteur (22 ; absent = aucun).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    etirement_x: f32,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    etirement_y: f32,
}

fn is_one(v: &f32) -> bool {
    *v == 1.0
}

/// Une image clé de caméra : à l'image `image` du panneau, centre (`x`, `y`) en pixels de la
/// feuille, zoom, rotation en degrés.
#[derive(Serialize, Deserialize)]
struct CameraEntry {
    image: usize,
    x: f32,
    y: f32,
    #[serde(default = "one")]
    zoom: f32,
    #[serde(default)]
    rotation: f32,
    /// Mouvement adouci à cette clé.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    doux: bool,
    /// Avancée de la caméra vers la feuille, en pixels (format 10 ; absent : 0).
    #[serde(default, skip_serializing_if = "is_zero")]
    avance: f32,
    /// Adoucis d'entrée et de sortie, en images (format 10 ; absent : 0).
    #[serde(default, skip_serializing_if = "is_zero")]
    adoucir_entree: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    adoucir_sortie: f32,
    /// Bascule (vers le haut) et panoramique (vers la droite), en degrés (format 11 ; absents : 0).
    #[serde(default, skip_serializing_if = "is_zero")]
    bascule: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    panoramique: f32,
}

#[derive(Serialize, Deserialize)]
struct LayerEntry {
    nom: String,
    /// « vectoriel » ou « bitmap ».
    #[serde(rename = "type")]
    kind: String,
    #[serde(default = "yes")]
    visible: bool,
    #[serde(default = "one")]
    opacite: f32,
    /// Ajouté en 1e (absent = non verrouillé).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    verrouille: bool,
    /// Transformation du calque (2c ; absent = aucune) : décalage en pixels de la feuille,
    /// échelle, rotation en degrés, autour du centre de la feuille.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    transformation: Option<TransformEntry>,
    /// Profondeur multiplan (2c ; absent = 0, plan de référence).
    #[serde(default, skip_serializing_if = "is_zero")]
    profondeur: f32,
    /// Calque de toute la scène (6f ; absent = calque du seul panneau) : ses exemplaires, un par
    /// panneau de la scène, portent le même numéro.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    toute_la_scene: Option<u64>,
    /// Mode de fusion (format 9 ; absent : normal).
    #[serde(default, skip_serializing_if = "is_normal")]
    fusion: crate::blend::BlendMode,
    /// Réglages de l'en-tête du calque (format 13 ; absents : éteints, groupe 0) : pelure
    /// d'oignon du calque, préserver la transparence, pochoir (et négatif), position
    /// verrouillée, groupe de couleur, ligne réduite dans la timeline.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pelure: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    transparence: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pochoir: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pochoir_negatif: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    position_verrouillee: bool,
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    groupe: u8,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    replie: bool,
    /// Dossier de calques (format 14 ; absent : calque ordinaire), et rang dans le plan du
    /// dossier qui range ce calque (absent : hors dossier).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    dossier: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dans_dossier: Option<usize>,
    /// Pré / post-comportement (format 15 ; absents : rien).
    #[serde(default, skip_serializing_if = "is_nothing")]
    avant: Behaviour,
    #[serde(default, skip_serializing_if = "is_nothing")]
    apres: Behaviour,
    /// Cassure automatique d'instance (absente : non) et création automatique d'image
    /// (absente : oui), format 15.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    cassure_auto: bool,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    creation_auto: bool,
    /// Noms d'instance (format 15) : numéro de dessin → nom.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    noms: std::collections::BTreeMap<u64, String>,
    /// Marques de couleur (format 16) : numéro de dessin → rang de la couleur.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    marques_couleur: std::collections::BTreeMap<u64, u8>,
    /// Rôles des images (format 18) : image du calque → rôle.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    roles: crate::roles::Roles,
    /// Pile d'effets (format 19) : sorte, actif, valeurs, clés (image, valeurs).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    effets: Vec<crate::fx::Effect>,
    /// Calque CTG (format 20) : rangs de ses calques sources dans le plan (aucun : tous les
    /// autres calques de dessin) ; absent : calque ordinaire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ctg: Option<CtgEntry>,
    /// Calque vidéo (format 21) ; absent : calque ordinaire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    video: Option<VideoEntry>,
    /// Fond sous le trait (format 23) : couleur, débord, fermeture des trous, bord ; absent : pas
    /// de fond. Ce qu'on a peint dans le fond d'un dessin : `dessins/<id>_fond.png`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fond: Option<crate::fond::FondReglages>,
    /// Les expositions dans l'ordre : quel dessin (fichier `dessins/<id>.*`, ou aucun) et pendant
    /// combien d'images.
    expositions: Vec<Exposure>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct VideoEntry {
    id: u64,
    nom: String,
    images: usize,
    ips: f64,
    #[serde(default)]
    debut: usize,
    ext: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct CtgEntry {
    #[serde(default)]
    sources: Vec<usize>,
    /// Fermeture automatique des trous, en pixels (absente : celle d'un calque CTG neuf).
    #[serde(default = "default_ctg_gap")]
    trou: u8,
}

fn default_ctg_gap() -> u8 {
    crate::ctg::DEFAULT_GAP
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
struct Exposure {
    dessin: Option<u64>,
    images: usize,
}

fn to_exposures(frames: &[Option<u64>]) -> Vec<Exposure> {
    let mut out: Vec<Exposure> = Vec::new();
    for &f in frames {
        match out.last_mut() {
            Some(last) if last.dessin == f => last.images += 1,
            _ => out.push(Exposure { dessin: f, images: 1 }),
        }
    }
    out
}

fn from_exposures(runs: &[Exposure]) -> Vec<Option<u64>> {
    runs.iter()
        .flat_map(|r| std::iter::repeat_n(r.dessin, r.images.min(1_000_000)))
        .collect()
}

fn yes() -> bool {
    true
}

fn one() -> f32 {
    1.0
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

fn is_true(b: &bool) -> bool {
    *b
}

fn is_nothing(b: &Behaviour) -> bool {
    *b == Behaviour::Nothing
}

fn is_normal(m: &crate::blend::BlendMode) -> bool {
    *m == crate::blend::BlendMode::Normal
}

/// Dossier et manifeste correspondant au nom choisi dans la boîte « Enregistrer sous ».
/// « C:\Films\Bus.pelure » → dossier « C:\Films\Bus\ », manifeste « C:\Films\Bus\Bus.pelure »
/// (ou directement dans « C:\Films\Bus\ » si l'on a choisi un fichier qui s'y trouve déjà).
pub fn target_for(chosen: &Path) -> (PathBuf, PathBuf) {
    let stem = chosen
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "projet".into());
    let parent = chosen.parent().map(Path::to_path_buf).unwrap_or_default();
    let dir = if parent.file_name().is_some_and(|n| n.to_string_lossy() == stem) {
        parent
    } else {
        parent.join(&stem)
    };
    let manifest = dir.join(format!("{stem}.pelure"));
    (dir, manifest)
}

/// Écrire là est-il sans danger ? (pas d'écrasement d'un autre projet ni de données étrangères)
pub fn check_target(manifest: &Path, current: Option<&Path>) -> Result<(), String> {
    if current == Some(manifest) {
        return Ok(());
    }
    let dir = manifest.parent().unwrap_or(Path::new("."));
    if manifest.exists() {
        return Err(format!(
            "un projet existe déjà à cet endroit ({}). Choisis un autre nom.",
            manifest.display()
        ));
    }
    for sub in [DRAWINGS, SOUNDS, MODELS, VIDEOS] {
        if dir.join(sub).exists() {
            return Err(format!(
                "le dossier {} contient déjà un dossier « {sub} ». Choisis un autre nom.",
                dir.display()
            ));
        }
    }
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, bytes).map_err(|e| format!("{} : {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("{} : {e}", path.display()))
}

/// La peinture de la strate Fond du dessin `id` (format 23), à côté de lui.
fn fond_path(dir: &Path, id: CelId) -> PathBuf {
    dir.join(DRAWINGS).join(format!("{id}_fond.png"))
}

/// La coupe de la strate Fond du dessin `id` (où le fond est retiré à la gomme).
fn coupe_path(dir: &Path, id: CelId) -> PathBuf {
    dir.join(DRAWINGS).join(format!("{id}_coupe.png"))
}

fn drawing_path(dir: &Path, id: CelId, vector: bool) -> PathBuf {
    dir.join(DRAWINGS).join(format!("{id}.{}", if vector { "json" } else { "png" }))
}

fn sound_file(sound: &Sound) -> String {
    format!("{}.{}", sound.id, sound.ext)
}

/// Sons utilisés par au moins un extrait.
fn used_sounds(doc: &Document) -> BTreeSet<u64> {
    doc.audio_tracks
        .iter()
        .flat_map(|t| t.clips.iter().map(|c| c.sound))
        .filter(|id| doc.sounds.contains_key(id))
        .collect()
}

/// Textures citées par les traits vectoriels texturés des dessins `used` : (sorte, clé).
fn used_textures(doc: &Document, used: &BTreeSet<CelId>) -> BTreeSet<(TextureKind, u32)> {
    let mut out = BTreeSet::new();
    for e in used.iter().filter_map(|id| doc.cels.get(id)).filter_map(|c| c.vector.as_ref()).flat_map(|v| &v.elements) {
        if let crate::vector::Element::Stroke { brush: Some(b), .. } = e {
            out.extend(b.tip.map(|k| (TextureKind::Tip, k)));
            out.extend(b.paper.map(|k| (TextureKind::Paper, k)));
        }
    }
    out
}

fn texture_file(dir: &Path, kind: TextureKind, key: u32) -> PathBuf {
    dir.join(TEXTURES).join(format!("{}_{key}.png", kind.file_word()))
}

/// Lit les textures du projet et les met à la disposition des traits vectoriels.
fn load_textures(dir: &Path, warnings: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir.join(TEXTURES)) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some((word, key)) = path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.split_once('_')) else { continue };
        let kind = match word {
            "pointe" => TextureKind::Tip,
            "papier" => TextureKind::Paper,
            _ => continue,
        };
        let Ok(key) = key.parse::<u32>() else { continue };
        match image::open(&path) {
            Ok(img) => {
                let gray = img.to_luma8();
                let values = gray.as_raw().iter().map(|&v| v as f32 / 255.0).collect();
                crate::brushtex::share_texture(kind, key, GrayImage::new(gray.width() as usize, gray.height() as usize, values));
            }
            Err(e) => warnings.push(format!("texture {} illisible ({e}) : traits lisses à la place", path.display())),
        }
    }
}

/// Efface les textures du dossier qui ne servent plus.
fn remove_unused_textures(dir: &Path, used: &BTreeSet<(TextureKind, u32)>) -> usize {
    let Ok(entries) = fs::read_dir(dir.join(TEXTURES)) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some((word, key)) = path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.split_once('_')) else { continue };
        let kind = match word {
            "pointe" => TextureKind::Tip,
            "papier" => TextureKind::Paper,
            _ => continue,
        };
        let is_png = path.extension().and_then(|e| e.to_str()) == Some("png");
        if let (true, Ok(key)) = (is_png, key.parse::<u32>()) {
            if !used.contains(&(kind, key)) && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

pub struct SaveReport {
    pub written: usize,
    pub removed: usize,
}

/// Enregistre le projet. `written` retient la version de chaque dessin déjà écrit dans ce dossier.
/// En cas d'échec, `written` est vidé : le prochain enregistrement réécrira tout.
pub fn save(doc: &Document, manifest_path: &Path, written: &mut HashMap<CelId, u64>) -> Result<SaveReport, String> {
    let job = prepare_save(doc, manifest_path, written, None)?;
    job.run().inspect_err(|_| written.clear())
}

/// Un dessin à écrire : déjà converti (vectoriel), copié tel quel (bitmap, converti en PNG à
/// l'écriture, la partie lente), ou à recopier d'un autre dossier où il est déjà écrit.
enum DrawingData {
    Bytes(Vec<u8>),
    Raster(Raster),
    CopyFrom(PathBuf),
}

/// Dossier d'un projet déjà enregistré, et versions des dessins qui y sont écrits : un dessin
/// inchangé depuis s'y recopie au lieu d'être réencodé.
pub struct Reuse<'a> {
    pub manifest_path: &'a Path,
    pub written: &'a HashMap<CelId, u64>,
}

/// Ce qu'un enregistrement doit écrire, préparé sur le fil de l'interface (copie des dessins
/// modifiés seulement) : l'écriture, elle, peut se faire sur un autre fil (sauvegarde de
/// secours) pendant qu'on continue à dessiner.
pub struct SaveJob {
    manifest_path: PathBuf,
    drawings: Vec<(PathBuf, DrawingData)>,
    sounds: Vec<(PathBuf, Arc<Vec<u8>>)>,
    models: Vec<(PathBuf, Arc<Model3d>)>,
    textures: Vec<(PathBuf, GrayImage)>,
    /// Images des calques vidéo à recopier dans le dossier du projet (13k) : d'où, vers où.
    videos: Vec<(crate::video::VideoClip, PathBuf, PathBuf)>,
    manifest: Vec<u8>,
    /// Des dessins du dossier n'ont pas pu être lus à l'ouverture : leurs fichiers (et les textures
    /// qu'ils pourraient utiliser) ne sont pas effacés.
    has_unreadable: bool,
    /// Dessins à garder dans le dossier : ceux du projet, et les illisibles.
    used: BTreeSet<CelId>,
    /// Dessins qui ont une strate Fond peinte (format 23).
    fonds: BTreeSet<CelId>,
    used_sounds: BTreeSet<u64>,
    used_models: BTreeSet<u64>,
    used_textures: BTreeSet<(TextureKind, u32)>,
}

/// Prépare l'enregistrement de `doc` dans `manifest_path`. `written` est mis à jour comme si
/// l'écriture allait réussir (à vider si elle échoue).
pub fn prepare_save(
    doc: &Document,
    manifest_path: &Path,
    written: &mut HashMap<CelId, u64>,
    reuse: Option<Reuse>,
) -> Result<SaveJob, String> {
    let dir = manifest_path.parent().ok_or("chemin de projet invalide")?;
    let reuse_dir = reuse.as_ref().and_then(|r| r.manifest_path.parent());
    let used = doc.used_cels();
    written.retain(|id, _| used.contains(id));
    let mut drawings = Vec::new();
    // Les dessins qui ont une strate Fond peinte (format 23) : les autres `<id>_fond.png` partent.
    let fonds: BTreeSet<CelId> = used.iter().copied().filter(|id| doc.cels.get(id).and_then(|c| c.fond.as_ref()).is_some_and(|f| !f.is_empty())).collect();
    for &id in &used {
        let Some(cel) = doc.cels.get(&id) else { continue };
        let vector = cel.vector.is_some();
        let path = drawing_path(dir, id, vector);
        if written.get(&id) == Some(&cel.version) && path.exists() && (!fonds.contains(&id) || fond_path(dir, id).exists()) {
            continue;
        }
        let same_elsewhere = reuse_dir.zip(reuse.as_ref()).filter(|(_, r)| r.written.get(&id) == Some(&cel.version)).map(|(d, _)| d);
        let elsewhere = same_elsewhere.map(|d| drawing_path(d, id, vector)).filter(|p| p.exists());
        let data = match (&cel.vector, elsewhere) {
            (_, Some(from)) => DrawingData::CopyFrom(from),
            (Some(v), None) => DrawingData::Bytes(serde_json::to_vec(v).map_err(|e| e.to_string())?),
            (None, None) => DrawingData::Raster(cel.raster.clone()),
        };
        drawings.push((path, data));
        // La strate Fond : sa peinture et sa coupe, à côté du dessin.
        if let (true, Some(s)) = (fonds.contains(&id), cel.fond.as_ref()) {
            let parts: [(fn(&Path, CelId) -> PathBuf, &Raster); 2] = [(fond_path, &s.peinture.raster), (coupe_path, &s.coupe)];
            for (path_of, raster) in parts {
                let data = match same_elsewhere.map(|d| path_of(d, id)).filter(|p| p.exists()) {
                    Some(from) => DrawingData::CopyFrom(from),
                    None => DrawingData::Raster(raster.clone()),
                };
                drawings.push((path_of(dir, id), data));
            }
        }
        written.insert(id, cel.version);
    }
    // Un son ne change jamais : on ne l'écrit que s'il n'est pas déjà dans le dossier.
    let used_sounds = used_sounds(doc);
    let sounds = used_sounds
        .iter()
        .map(|id| &doc.sounds[id])
        .map(|s| (dir.join(SOUNDS).join(sound_file(s)), s.bytes.clone()))
        .filter(|(path, _)| !path.exists())
        .collect();
    // Un modèle 3D non plus ; il est converti à l'écriture (sur l'autre fil), s'il manque.
    let used_models = used_models(doc);
    let models = used_models
        .iter()
        .map(|id| (model_file(dir, *id), doc.models[id].clone()))
        .filter(|(path, _)| !path.exists())
        .collect();
    // Une texture non plus (sa clé vient de son contenu) : écrite seulement si elle manque.
    let used_textures = used_textures(doc, &used);
    let textures = used_textures
        .iter()
        .map(|&(kind, key)| (texture_file(dir, kind, key), kind, key))
        .filter(|(path, _, _)| !path.exists())
        .filter_map(|(path, kind, key)| crate::brushtex::shared_image(kind, key).map(|img| (path, img)))
        .collect();
    // Images des calques vidéo : recopiées si elles ne sont pas déjà dans ce dossier.
    let mut videos = Vec::new();
    for v in doc.panels.iter().flat_map(|p| &p.layers).filter_map(|l| l.video.as_ref()) {
        let to = dir.join(VIDEOS).join(v.id.to_string());
        if let Some(from) = crate::video::dir_of(v.id).filter(|from| *from != to) {
            if !videos.iter().any(|(w, _, _): &(crate::video::VideoClip, PathBuf, PathBuf)| w.id == v.id) {
                videos.push((v.clone(), from, to));
            }
        }
    }
    let manifest = serde_json::to_vec_pretty(&manifest_of(doc)).map_err(|e| e.to_string())?;
    let unreadable = unreadable_in(dir);
    Ok(SaveJob {
        manifest_path: manifest_path.to_path_buf(),
        drawings,
        sounds,
        models,
        textures,
        videos,
        manifest,
        has_unreadable: !unreadable.is_empty(),
        used: used.union(&unreadable).copied().collect(),
        fonds: fonds.union(&unreadable).copied().collect(),
        used_sounds,
        used_models,
        used_textures,
    })
}

impl SaveJob {
    /// Écrit les dessins, puis les sons, puis le manifeste (en dernier : un manifeste ne cite
    /// jamais un dessin pas encore écrit), puis efface ce qui ne sert plus.
    pub fn run(self) -> Result<SaveReport, String> {
        let dir = self.manifest_path.parent().ok_or("chemin de projet invalide")?;
        fs::create_dir_all(dir.join(DRAWINGS)).map_err(|e| format!("{} : {e}", dir.display()))?;
        let mut count = 0;
        for (path, data) in &self.drawings {
            let bytes = match data {
                DrawingData::Bytes(b) => b.clone(),
                DrawingData::Raster(r) => encode_png(&r.to_rgba(), r.width, r.height)?,
                DrawingData::CopyFrom(from) => fs::read(from).map_err(|e| format!("{} : {e}", from.display()))?,
            };
            write_atomic(path, &bytes)?;
            count += 1;
        }
        if !self.sounds.is_empty() {
            fs::create_dir_all(dir.join(SOUNDS)).map_err(|e| format!("{} : {e}", dir.display()))?;
        }
        for (path, bytes) in &self.sounds {
            write_atomic(path, bytes)?;
            count += 1;
        }
        if !self.models.is_empty() {
            fs::create_dir_all(dir.join(MODELS)).map_err(|e| format!("{} : {e}", dir.display()))?;
        }
        for (path, model) in &self.models {
            write_atomic(path, &crate::model3d::to_bytes(&model.mesh))?;
            count += 1;
        }
        if !self.textures.is_empty() {
            fs::create_dir_all(dir.join(TEXTURES)).map_err(|e| format!("{} : {e}", dir.display()))?;
        }
        for (path, img) in &self.textures {
            let gray = image::GrayImage::from_raw(img.width as u32, img.height as u32, crate::brushtex::quantized(img))
                .ok_or("texture de taille invalide")?;
            let mut bytes = Vec::new();
            gray.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).map_err(|e| e.to_string())?;
            write_atomic(path, &bytes)?;
            count += 1;
        }
        for (clip, from, to) in &self.videos {
            count += crate::video::copy_frames(clip, from, to)?;
        }
        write_atomic(&self.manifest_path, &self.manifest)?;
        let removed = remove_unused(dir, &self.used, &self.fonds)
            + remove_unused_sounds(dir, &self.used_sounds)
            + remove_unused_models(dir, &self.used_models)
            + if self.has_unreadable { 0 } else { remove_unused_textures(dir, &self.used_textures) };
        Ok(SaveReport { written: count, removed })
    }
}

/// Efface les modèles 3D du dossier qui ne servent plus (seulement « <nombre>.pelure3d »).
fn remove_unused_models(dir: &Path, used: &BTreeSet<u64>) -> usize {
    let Ok(entries) = fs::read_dir(dir.join(MODELS)) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let id = path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.parse::<u64>().ok());
        if let (Some(id), "pelure3d") = (id, ext) {
            if !used.contains(&id) && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

/// Efface les sons du dossier qui ne servent plus (seulement les fichiers « <nombre>.<ext> »).
fn remove_unused_sounds(dir: &Path, used: &BTreeSet<u64>) -> usize {
    let Ok(entries) = fs::read_dir(dir.join(SOUNDS)) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        let id = path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.parse::<u64>().ok());
        if let Some(id) = id {
            if SOUND_EXTS.contains(&ext.as_str()) && !used.contains(&id) && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

/// Efface les dessins du dossier qui ne servent plus (seulement les fichiers « <nombre>.png/json »),
/// et les strates Fond « <nombre>_fond.png » et « <nombre>_coupe.png » des dessins qui n'en ont plus.
fn remove_unused(dir: &Path, used: &BTreeSet<CelId>, fonds: &BTreeSet<CelId>) -> usize {
    let Ok(entries) = fs::read_dir(dir.join(DRAWINGS)) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if let (Some(id), "png") = (stem.strip_suffix("_fond").or_else(|| stem.strip_suffix("_coupe")).and_then(|s| s.parse::<CelId>().ok()), ext) {
            if !fonds.contains(&id) && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
            continue;
        }
        let id = stem.parse::<CelId>().ok();
        if let (Some(id), "png" | "json") = (id, ext) {
            if !used.contains(&id) && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

fn manifest_of(doc: &Document) -> Manifest {
    let mut sequences: Vec<SequenceEntry> = Vec::new();
    for (i, panel) in doc.panels.iter().enumerate() {
        let seq = doc.sequence_of(i);
        if sequences.last().is_none_or(|s| s.id != seq) {
            let nom = doc.sequences.iter().find(|s| s.id == seq).map(|s| s.name.clone()).unwrap_or_default();
            sequences.push(SequenceEntry {
                id: seq,
                nom,
                scenes: Vec::new(),
            });
        }
        let scenes = &mut sequences.last_mut().expect("séquence ajoutée juste avant").scenes;
        if scenes.last().is_none_or(|s| s.id != panel.scene) {
            let nom = doc.scene(panel.scene).map(|s| s.name.clone()).unwrap_or_default();
            scenes.push(SceneEntry {
                id: panel.scene,
                nom,
                panneaux: Vec::new(),
            });
        }
        scenes.last_mut().expect("scène ajoutée juste avant").panneaux.push(PanelEntry {
            id: panel.id,
            duree: panel.length,
            action: panel.action.clone(),
            dialogue: panel.dialogue.clone(),
            notes: panel.notes.clone(),
            calques: panel
                .layers
                .iter()
                .map(|l| LayerEntry {
                    nom: l.name.clone(),
                    kind: match l.kind {
                        LayerKind::Vector => "vectoriel".into(),
                        LayerKind::Bitmap => "bitmap".into(),
                    },
                    visible: l.visible,
                    opacite: l.opacity,
                    verrouille: l.locked,
                    transformation: (!l.transform.is_identity()).then(|| TransformEntry {
                        x: l.transform.x,
                        y: l.transform.y,
                        echelle: l.transform.scale,
                        rotation: l.transform.angle,
                        etirement_x: l.transform.stretch_x,
                        etirement_y: l.transform.stretch_y,
                    }),
                    profondeur: l.depth,
                    toute_la_scene: l.scene_wide.map(|w| w.key),
                    fusion: l.blend,
                    pelure: l.onion,
                    transparence: l.keep_alpha,
                    pochoir: l.stencil,
                    pochoir_negatif: l.stencil_negative,
                    position_verrouillee: l.position_locked,
                    groupe: l.group,
                    replie: l.collapsed,
                    dossier: l.folder,
                    dans_dossier: l.parent.and_then(|p| panel.layers.iter().position(|f| f.folder && f.id == p)),
                    avant: l.pre,
                    apres: l.post,
                    cassure_auto: l.auto_break,
                    creation_auto: l.auto_create,
                    // Seulement les noms des dessins encore dans la timeline du calque.
                    noms: l.names.iter().filter(|(id, _)| l.frames.contains(&Some(**id))).map(|(id, n)| (*id, n.clone())).collect(),
                    marques_couleur: l.color_marks.iter().filter(|(id, _)| l.frames.contains(&Some(**id))).map(|(id, k)| (*id, *k)).collect(),
                    roles: l.roles.clone(),
                    effets: l.fx.clone(),
                    ctg: l.ctg.as_ref().map(|c| CtgEntry {
                        sources: c.sources.iter().filter_map(|id| panel.layers.iter().position(|s| s.id == *id)).collect(),
                        trou: c.gap,
                    }),
                    video: l.video.as_ref().map(|v| VideoEntry { id: v.id, nom: v.name.clone(), images: v.frames, ips: v.fps, debut: v.start, ext: v.ext.clone() }),
                    fond: l.fond,
                    expositions: to_exposures(&l.frames),
                })
                .collect(),
            camera: panel
                .camera
                .iter()
                .map(|k| CameraEntry {
                    image: k.frame,
                    x: k.cam.x,
                    y: k.cam.y,
                    zoom: k.cam.zoom,
                    rotation: k.cam.angle,
                    doux: k.smooth,
                    avance: k.cam.dolly,
                    adoucir_entree: k.ease_in,
                    adoucir_sortie: k.ease_out,
                    bascule: k.cam.tilt,
                    panoramique: k.cam.pan,
                })
                .collect(),
            repere3d: panel.ref3d.as_ref().map(|r| Ref3dEntry {
                modele: r.model,
                cible: r.cam.target,
                azimut: r.cam.azimuth,
                elevation: r.cam.elevation,
                distance: r.cam.distance,
                focale: r.cam.focal,
                visible: r.visible,
                aretes_cachees: r.hidden_lines,
                couleur: r.color,
                opacite: r.opacity,
            }),
            marques: panel
                .marks
                .iter()
                .map(|m| MarkEntry {
                    id: m.id,
                    image: m.frame,
                    couleur: m.color,
                    action: m.action.clone(),
                    dialogue: m.dialogue.clone(),
                    notes: m.notes.clone(),
                })
                .collect(),
            signets: panel.cues.bookmarks.clone(),
            entree: panel.cues.in_point,
            sortie: panel.cues.out_point,
            notes_dessinees: panel
                .cues
                .notes
                .iter()
                .map(|n| TimeNoteEntry { couleur: n.color, epaisseur: n.width, points: n.points.clone() })
                .collect(),
        });
    }
    let used = used_sounds(doc);
    Manifest {
        format: FORMAT.into(),
        version: VERSION,
        largeur: doc.width,
        hauteur: doc.height,
        cadre: Some(doc.frame),
        cadence: doc.fps,
        papier: doc.paper,
        sequences,
        sons: used
            .iter()
            .map(|id| {
                let s = &doc.sounds[id];
                SoundEntry {
                    id: s.id,
                    nom: s.name.clone(),
                    fichier: sound_file(s),
                }
            })
            .collect(),
        pistes_son: doc
            .audio_tracks
            .iter()
            .map(|t| TrackEntry {
                id: t.id,
                nom: t.name.clone(),
                muette: t.muted,
                volume: t.volume,
                verrouillee: t.locked,
                extraits: t
                    .clips
                    .iter()
                    .filter(|c| used.contains(&c.sound))
                    .map(|c| ClipEntry {
                        id: c.id,
                        son: c.sound,
                        debut: c.start,
                        decalage: c.offset,
                        duree: c.length,
                    })
                    .collect(),
            })
            .collect(),
        guides: doc.guides.clone(),
        modeles: used_models(doc)
            .iter()
            .map(|id| ModelEntry {
                id: *id,
                nom: doc.models[id].name.clone(),
            })
            .collect(),
        planche: doc
            .board
            .strokes
            .iter()
            .map(|s| NoteEntry {
                couleur: s.color,
                points: s.points.iter().map(|p| [p.x, p.y, p.r]).collect(),
                attache: s.anchor,
            })
            .collect(),
        planche_textes: doc
            .board
            .texts
            .iter()
            .map(|t| NoteTextEntry {
                texte: t.text.clone(),
                position: [t.pos.x, t.pos.y],
                taille: t.size,
                couleur: t.color,
                attache: t.anchor,
            })
            .collect(),
        groupes: if doc.groups == crate::groups::defaults() {
            Vec::new()
        } else {
            doc.groups
                .iter()
                .map(|g| GroupEntry { nom: g.name.clone(), couleur: g.color, montre: g.shown })
                .collect()
        },
        image_de_depart: doc.start_frame,
        fond: BackgroundEntry { sorte: doc.background, couleur_b: doc.paper_b },
        volume_general: doc.master_volume,
        storyboard_verrouille: doc.storyboard_locked,
        proprietes: {
            let p = &doc.properties;
            PropertiesEntry {
                titre: p.title.clone(),
                informations: p.info.clone(),
                auteur: p.author.clone(),
                cree: p.created,
                modifie: p.modified,
                notes: p.notes.clone(),
                copyright: p.copyright.clone(),
                fond: p.fond,
            }
        },
    }
}

/// Modèles 3D cités par au moins un panneau.
fn used_models(doc: &Document) -> BTreeSet<u64> {
    doc.panels
        .iter()
        .filter_map(|p| p.ref3d.as_ref().map(|r| r.model))
        .filter(|id| doc.models.contains_key(id))
        .collect()
}

fn model_file(dir: &Path, id: u64) -> PathBuf {
    dir.join(MODELS).join(format!("{id}.pelure3d"))
}

pub struct Loaded {
    pub doc: Document,
    /// Dessins présents dans le dossier, à la version chargée (rien à réécrire au prochain enregistrement).
    pub written: HashMap<CelId, u64>,
    /// Problèmes non bloquants (dessin manquant ou illisible…).
    pub warnings: Vec<String>,
    /// Ancien format (3) converti à l'ouverture : feuille doublée autour du cadre.
    pub converted: bool,
    /// Dessins vectoriels affichés en rendu rapide (`load_quick`) : leur rendu exact est à
    /// demander au rejeu en arrière-plan (`rejeu.rs`).
    pub approx: Vec<CelId>,
}

/// Ouvre le projet, dessins vectoriels rendus exactement (le moteur rejoué sur chaque trait : ce
/// que l'application affiche une fois le rejeu terminé).
pub fn load(manifest_path: &Path) -> Result<Loaded, String> {
    load_with(manifest_path, false)
}

/// Ouvre le projet tout de suite : les dessins vectoriels texturés sont affichés en rendu rapide
/// (traits lisses, sans rejeu du moteur) et listés dans `approx`, pour un rendu exact en
/// arrière-plan. Règle d'Anthony : rien ne bloque l'interface.
pub fn load_quick(manifest_path: &Path) -> Result<Loaded, String> {
    load_with(manifest_path, true)
}

fn load_with(manifest_path: &Path, quick: bool) -> Result<Loaded, String> {
    let text = fs::read_to_string(manifest_path).map_err(|e| e.to_string())?;
    let manifest: Manifest =
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("manifeste illisible : {e}"))?;
    if manifest.format != FORMAT {
        return Err("ce fichier n'est pas un projet Pelure".into());
    }
    if manifest.version > VERSION {
        return Err(format!(
            "projet créé par une version plus récente de Pelure (format {})",
            manifest.version
        ));
    }
    let dir = manifest_path.parent().unwrap_or(Path::new("."));
    let (w, h) = (manifest.largeur, manifest.hauteur);
    if w == 0 || h == 0 {
        return Err("taille d'image invalide dans le manifeste".into());
    }
    let mut warnings = Vec::new();
    // Les textures d'abord : les dessins vectoriels texturés en ont besoin pour leur rendu.
    load_textures(dir, &mut warnings);
    let mut cels: HashMap<CelId, Cel> = HashMap::new();
    let mut missing: BTreeSet<CelId> = BTreeSet::new();
    let referenced: BTreeSet<CelId> = manifest
        .sequences
        .iter()
        .flat_map(|s| &s.scenes)
        .flat_map(|s| &s.panneaux)
        .flat_map(|p| &p.calques)
        .flat_map(|l| l.expositions.iter().filter_map(|r| r.dessin))
        .collect();
    let mut approx = Vec::new();
    for id in referenced {
        match load_drawing(dir, id, w, h, quick) {
            Ok((cel, approximate)) => {
                if approximate {
                    approx.push(id);
                }
                cels.insert(id, cel);
            }
            Err(e) => {
                warnings.push(e);
                missing.insert(id);
            }
        }
    }

    let mut sounds: HashMap<u64, Sound> = HashMap::new();
    for entry in &manifest.sons {
        match load_sound(dir, entry) {
            Ok(sound) => {
                sounds.insert(sound.id, sound);
            }
            Err(e) => warnings.push(e),
        }
    }
    let mut models: HashMap<u64, Model3d> = HashMap::new();
    for entry in &manifest.modeles {
        match fs::read(model_file(dir, entry.id))
            .map_err(|e| e.to_string())
            .and_then(|b| crate::model3d::from_bytes(&b))
        {
            Ok(mesh) => {
                models.insert(entry.id, Model3d::new(entry.id, entry.nom.clone(), mesh));
            }
            Err(e) => warnings.push(format!("modèle 3D « {} » illisible ({e})", entry.nom)),
        }
    }
    let audio_tracks = manifest
        .pistes_son
        .iter()
        .map(|t| AudioTrack {
            id: t.id,
            name: t.nom.clone(),
            muted: t.muette,
            volume: t.volume.clamp(0.0, 4.0),
            clips: t
                .extraits
                .iter()
                .filter(|c| sounds.contains_key(&c.son) && c.duree > 0)
                .map(|c| SoundClip {
                    id: c.id,
                    sound: c.son,
                    start: c.debut,
                    offset: c.decalage,
                    length: c.duree,
                })
                .collect(),
            locked: t.verrouillee,
        })
        .collect();

    let mut structure = Structure {
        sequences: Vec::new(),
        scenes: Vec::new(),
        panels: Vec::new(),
        audio_tracks,
        guides: manifest
            .guides
            .into_iter()
            .filter(|g| (1..=3).contains(&g.points.len()) && g.points.iter().flatten().all(|v| v.is_finite()))
            .collect(),
        paper: manifest.papier,
        fps: manifest.cadence.max(1),
        board: crate::board::Board {
            strokes: manifest
                .planche
                .into_iter()
                .map(|n| crate::board::NoteStroke {
                    color: n.couleur,
                    points: n
                        .points
                        .into_iter()
                        .filter(|p| p.iter().all(|v| v.is_finite()))
                        .map(|[x, y, r]| crate::vector::VPoint { x, y, r: r.clamp(1e-4, 200.0) })
                        .collect(),
                    anchor: n.attache,
                })
                .filter(|s| !s.points.is_empty())
                .collect(),
            texts: manifest
                .planche_textes
                .into_iter()
                .filter(|t| t.position.iter().all(|v| v.is_finite()) && t.taille.is_finite() && t.taille > 0.0)
                .map(|t| crate::board::NoteText {
                    pos: eframe::egui::pos2(t.position[0], t.position[1]),
                    text: t.texte,
                    size: t.taille.min(500.0),
                    color: t.couleur,
                    anchor: t.attache,
                })
                .collect(),
        },
        groups: crate::groups::completed(
            manifest
                .groupes
                .into_iter()
                .map(|g| crate::groups::LayerGroup { name: g.nom, color: g.couleur, shown: g.montre })
                .collect(),
        ),
        start_frame: manifest.image_de_depart.min(1_000_000),
        background: manifest.fond.sorte,
        paper_b: manifest.fond.couleur_b,
        master_volume: if manifest.volume_general.is_finite() { manifest.volume_general.clamp(0.0, 4.0) } else { 1.0 },
        storyboard_locked: manifest.storyboard_verrouille,
        properties: {
            let p = manifest.proprietes;
            crate::document::Properties {
                title: p.titre,
                info: p.informations,
                author: p.auteur,
                created: p.cree,
                modified: p.modifie,
                notes: p.notes,
                copyright: p.copyright,
                fond: p.fond,
            }
        },
    };
    for seq in manifest.sequences {
        structure.sequences.push(Sequence {
            id: seq.id,
            name: seq.nom,
        });
        for scene in seq.scenes {
            structure.scenes.push(Scene {
                id: scene.id,
                name: scene.nom,
                sequence: seq.id,
            });
            // Numéros des calques de toute la scène : neufs pour la session.
            let mut keys: HashMap<u64, u64> = HashMap::new();
            for p in scene.panneaux {
                let parents: Vec<Option<usize>> = p.calques.iter().map(|l| l.dans_dossier).collect();
                let ctg_sources: Vec<Option<(Vec<usize>, u8)>> = p.calques.iter().map(|l| l.ctg.as_ref().map(|c| (c.sources.clone(), c.trou))).collect();
                let mut layers: Vec<Layer> = p
                    .calques
                    .into_iter()
                    .map(|l| Layer {
                        blend: l.fusion,
                        scene_wide: l.toute_la_scene.map(|k| SceneWide {
                            key: *keys.entry(k).or_insert_with(fresh_layer_id),
                            scene: scene.id,
                        }),
                        id: fresh_layer_id(),
                        name: l.nom,
                        kind: if l.kind == "bitmap" {
                            LayerKind::Bitmap
                        } else {
                            LayerKind::Vector
                        },
                        visible: l.visible,
                        opacity: l.opacite.clamp(0.0, 1.0),
                        locked: l.verrouille,
                        transform: l
                            .transformation
                            .as_ref()
                            .filter(|t| [t.x, t.y, t.echelle, t.rotation, t.etirement_x, t.etirement_y].iter().all(|v| v.is_finite()))
                            .map(|t| LayerTransform {
                                x: t.x,
                                y: t.y,
                                scale: t.echelle.clamp(0.01, 100.0),
                                angle: t.rotation,
                                stretch_x: LayerTransform::clamp_stretch(t.etirement_x),
                                stretch_y: LayerTransform::clamp_stretch(t.etirement_y),
                            })
                            .unwrap_or_default(),
                        depth: if l.profondeur.is_finite() {
                            l.profondeur.clamp(Layer::MIN_DEPTH, 1000.0)
                        } else {
                            0.0
                        },
                        frames: from_exposures(&l.expositions)
                            .into_iter()
                            .map(|f| f.filter(|id| !missing.contains(id)))
                            .collect(),
                        onion: l.pelure,
                        keep_alpha: l.transparence,
                        stencil: l.pochoir,
                        stencil_negative: l.pochoir_negatif,
                        position_locked: l.position_verrouillee,
                        group: if (l.groupe as usize) < crate::groups::COUNT { l.groupe } else { 0 },
                        collapsed: l.replie,
                        folder: l.dossier,
                        parent: None,
                        pre: l.avant,
                        post: l.apres,
                        auto_break: l.cassure_auto,
                        auto_create: l.creation_auto,
                        names: l.noms,
                        color_marks: l.marques_couleur.into_iter().filter(|(_, k)| (*k as usize) < crate::marks::COLORS.len()).collect(),
                        roles: l.roles,
                        fx: l.effets,
                        ctg: None,
                        // Calque vidéo (format 21) : ses images dans `videos/<id>/`.
                        video: l.video.filter(|v| v.images > 0 && v.ips > 0.0 && matches!(v.ext.as_str(), "jpg" | "png")).map(|v| {
                            crate::video::register_dir(v.id, dir.join(VIDEOS).join(v.id.to_string()));
                            crate::video::VideoClip { id: v.id, name: v.nom, frames: v.images, fps: v.ips, start: v.debut, ext: v.ext }
                        }),
                        // Fond sous le trait (format 23).
                        fond: l.fond,
                    })
                    .collect();
                // Dossiers (format 14) : le rang du dossier devient l'id de son calque.
                for (i, fi) in parents.into_iter().enumerate() {
                    if let Some(id) = fi.and_then(|fi| layers.get(fi)).filter(|f| f.folder).map(|f| f.id) {
                        layers[i].parent = Some(id);
                    }
                }
                // Calques CTG (format 20) : les rangs des sources deviennent leurs id.
                for (i, entry) in ctg_sources.into_iter().enumerate() {
                    if let Some((sources, gap)) = entry {
                        let ids = sources.into_iter().filter(|&k| k != i).filter_map(|k| layers.get(k).map(|l| l.id)).collect();
                        layers[i].ctg = Some(crate::ctg::Ctg { sources: ids, gap: gap.min(50) });
                    }
                }
                crate::folders::normalize(&mut layers);
                if layers.is_empty() {
                    layers.push(Layer::new("Dessin", LayerKind::Vector));
                }
                let mut camera: Vec<CameraKey> = Vec::new();
                for k in &p.camera {
                    let finite =
                        [k.x, k.y, k.zoom, k.rotation, k.avance, k.adoucir_entree, k.adoucir_sortie, k.bascule, k.panoramique];
                    if finite.iter().all(|v| v.is_finite()) {
                        let cam = Camera {
                            x: k.x,
                            y: k.y,
                            zoom: k.zoom.clamp(0.05, 50.0),
                            angle: k.rotation,
                            dolly: k.avance,
                            tilt: k.bascule.clamp(-85.0, 85.0),
                            pan: k.panoramique.clamp(-85.0, 85.0),
                        };
                        crate::camera::set_key(&mut camera, k.image, cam);
                        if let Some(x) = camera.iter_mut().find(|x| x.frame == k.image) {
                            x.smooth = k.doux;
                            x.ease_in = k.adoucir_entree.max(0.0);
                            x.ease_out = k.adoucir_sortie.max(0.0);
                        }
                    }
                }
                structure.panels.push(Panel {
                    id: p.id,
                    scene: scene.id,
                    length: p.duree,
                    layers,
                    action: p.action,
                    dialogue: p.dialogue,
                    notes: p.notes,
                    camera,
                    ref3d: p.repere3d.as_ref().filter(|r| models.contains_key(&r.modele)).and_then(ref3d_of),
                    marks: marks_of(p.marques),
                    cues: crate::document::Cues {
                        bookmarks: {
                            let mut b = p.signets;
                            b.sort_unstable();
                            b.dedup();
                            b
                        },
                        in_point: p.entree,
                        out_point: p.sortie,
                        notes: p
                            .notes_dessinees
                            .into_iter()
                            .filter(|n| n.epaisseur.is_finite() && n.points.iter().flatten().all(|v| v.is_finite()))
                            .map(|n| crate::document::TimeNote {
                                color: n.couleur.min(2),
                                width: n.epaisseur.clamp(0.5, 40.0),
                                points: n.points.into_iter().map(|[x, y]| [x, y.clamp(0.0, 1.0)]).collect(),
                            })
                            .collect(),
                    },
                });
            }
        }
    }
    if structure.panels.is_empty() {
        return Err("le projet ne contient aucun plan".into());
    }
    let frame = manifest.cadre;
    let mut written: HashMap<CelId, u64> = cels.keys().map(|&id| (id, 0)).collect();
    let mut doc = Document::from_parts(w, h, structure, cels, sounds);
    for model in models.into_values() {
        doc.insert_model(model);
    }
    // Projet d'avant les plans (13a) : chaque scène devient un plan, ses panneaux bout à bout.
    if !doc.is_plans() {
        let n = doc.panels.len();
        warnings.extend(doc.into_plans());
        warnings.push(format!(
            "Projet converti en plans : {n} panneaux mis bout à bout en {} plans ; le début de chaque ancien panneau est une marque d'image.",
            doc.panels.len()
        ));
    }
    let converted = match frame {
        Some([fw, fh]) if fw > 0 && fh > 0 && fw <= w && fh <= h => {
            doc.frame = [fw, fh];
            false
        }
        // Format 3 : la feuille était le cadre. On la double, dessin au centre ; les fichiers de
        // dessins seront tous réécrits à la nouvelle taille au prochain enregistrement.
        _ => {
            doc.enlarge_stage(w * 2, h * 2);
            doc.frame = [w, h];
            written.clear();
            true
        }
    };
    // Les dessins illisibles ne sont plus dans le projet, mais leurs fichiers restent : un
    // enregistrement ne les efface jamais (on pourra les récupérer ou les rouvrir avec une
    // version qui les lit).
    // (Chacun a déjà son avertissement, plus haut.)
    keep_unreadable(dir, &missing);
    Ok(Loaded {
        doc,
        written,
        warnings,
        converted,
        approx,
    })
}

/// Dessins dont le fichier n'a pas pu être lu à l'ouverture, par dossier de projet : le prochain
/// enregistrement ne les efface pas.
static UNREADABLE: std::sync::Mutex<BTreeMap<PathBuf, BTreeSet<CelId>>> = std::sync::Mutex::new(BTreeMap::new());

fn keep_unreadable(dir: &Path, ids: &BTreeSet<CelId>) {
    let mut m = UNREADABLE.lock().unwrap_or_else(|e| e.into_inner());
    if ids.is_empty() {
        m.remove(dir);
    } else {
        m.insert(dir.to_path_buf(), ids.clone());
    }
}

fn unreadable_in(dir: &Path) -> BTreeSet<CelId> {
    UNREADABLE.lock().unwrap_or_else(|e| e.into_inner()).get(dir).cloned().unwrap_or_default()
}

/// Marques d'image relues du manifeste : triées, après la première image, une seule par image.
fn marks_of(entries: Vec<MarkEntry>) -> Vec<crate::document::ImageMark> {
    let mut marks: Vec<crate::document::ImageMark> = entries
        .into_iter()
        .filter(|m| m.image > 0)
        .map(|m| crate::document::ImageMark {
            id: m.id,
            frame: m.image,
            color: m.couleur,
            action: m.action,
            dialogue: m.dialogue,
            notes: m.notes,
        })
        .collect();
    marks.sort_by_key(|m| m.frame);
    marks.dedup_by_key(|m| m.frame);
    marks
}

/// Repère 3D relu du manifeste ; `None` si ses valeurs sont absurdes.
fn ref3d_of(r: &Ref3dEntry) -> Option<Ref3d> {
    let values = [r.azimut, r.elevation, r.distance, r.focale, r.opacite];
    if !values.iter().chain(&r.cible).all(|v| v.is_finite()) || r.distance <= 0.0 || r.focale <= 0.0 {
        return None;
    }
    Some(Ref3d {
        model: r.modele,
        cam: Cam3d {
            target: r.cible,
            azimuth: r.azimut,
            elevation: r.elevation.clamp(-MAX_ELEVATION, MAX_ELEVATION),
            distance: r.distance,
            focal: r.focale,
        },
        visible: r.visible,
        hidden_lines: r.aretes_cachees,
        color: r.couleur,
        opacity: r.opacite.clamp(0.0, 1.0),
    })
}

fn load_sound(dir: &Path, entry: &SoundEntry) -> Result<Sound, String> {
    let shown = if entry.nom.is_empty() { &entry.fichier } else { &entry.nom };
    // Le nom de fichier vient du manifeste : on refuse tout ce qui sortirait du dossier `sons`.
    let file = Path::new(&entry.fichier);
    if file.components().count() != 1 || file.file_name().is_none() {
        return Err(format!("son « {shown} » : nom de fichier invalide"));
    }
    let bytes = fs::read(dir.join(SOUNDS).join(file)).map_err(|e| format!("son « {shown} » introuvable ({e})"))?;
    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let pcm = crate::audio::decode(&bytes, &ext).map_err(|e| format!("son « {shown} » : {e}"))?;
    Ok(Sound {
        id: entry.id,
        name: entry.nom.clone(),
        ext,
        bytes: Arc::new(bytes),
        pcm: Arc::new(pcm),
    })
}

/// Le dessin `id`, avec sa strate Fond s'il en a une (format 23 ; illisible ou d'une autre taille :
/// ignorée, le dessin reste) ; et s'il est affiché en rendu rapide (`quick`, dessin vectoriel
/// texturé).
fn load_drawing(dir: &Path, id: CelId, w: usize, h: usize, quick: bool) -> Result<(Cel, bool), String> {
    let (mut cel, approximate) = load_line(dir, id, w, h, quick)?;
    let read = |path: PathBuf| -> Option<Raster> {
        let img = image::open(&path).ok()?.to_rgba8();
        (img.width() as usize == w && img.height() as usize == h).then(|| Raster::from_rgba(w, h, img.as_raw()))
    };
    let (paint, cut) = (read(fond_path(dir, id)), read(coupe_path(dir, id)));
    if paint.is_some() || cut.is_some() {
        let mut s = crate::document::Strate::new(w, h);
        if let Some(r) = paint {
            s.peinture.raster = r;
        }
        if let Some(r) = cut {
            s.coupe = r;
        }
        cel.fond = Some(Box::new(s));
    }
    Ok((cel, approximate))
}

fn load_line(dir: &Path, id: CelId, w: usize, h: usize, quick: bool) -> Result<(Cel, bool), String> {
    let vector_path = drawing_path(dir, id, true);
    if vector_path.exists() {
        let text = fs::read_to_string(&vector_path).map_err(|e| format!("dessin {id} illisible : {e}"))?;
        let drawing: VectorDrawing =
            serde_json::from_str(&text).map_err(|e| format!("dessin {id} illisible : {e}"))?;
        let textured = drawing.elements.iter().any(|e| matches!(e, crate::vector::Element::Stroke { brush: Some(_), .. }));
        let approximate = quick && textured;
        let raster = if approximate { drawing.render_quick(w, h) } else { drawing.render(w, h) };
        return Ok((
            Cel {
                raster,
                vector: Some(drawing),
                version: 0,
                fond: None,
            },
            approximate,
        ));
    }
    let png_path = drawing_path(dir, id, false);
    if png_path.exists() {
        let img = image::open(&png_path)
            .map_err(|e| format!("dessin {id} illisible : {e}"))?
            .to_rgba8();
        if img.width() as usize != w || img.height() as usize != h {
            return Err(format!("dessin {id} : taille différente de celle du projet, ignoré"));
        }
        return Ok((Cel::bitmap(Raster::from_rgba(w, h, img.as_raw())), false));
    }
    Err(format!("dessin {id} introuvable dans « {DRAWINGS} »"))
}
