//! Projet en dossier (format 3).
//!
//! ```text
//! MonFilm/
//!   MonFilm.pelure        manifeste JSON lisible : réglages, séquences > scènes > panneaux > calques,
//!                         pistes son ; numérotation verrouillée (`numerotation_verrouillee`, `numero` des
//!                         séquences et des plans : champs facultatifs, `numerotation.rs`)
//!   dessins/<id>.png      dessins bitmap
//!   dessins/<id>.json     dessins vectoriels
//!   sons/<id>.<ext>       fichiers son importés, copiés tels quels
//!   modeles/<id>.pelure3d modèles 3D de repère (format 5), maillage soudé (voir `model3d`)
//!   commentaires.json     commentaires sur la timeline (`commentaires.rs`), sans changer `VERSION` :
//!                         absent, il n'y a pas de commentaires ; une version qui ne le connaît pas
//!                         le laisse en place (elle n'efface que le contenu de dessins/, sons/…)
//!   references.json      références visuelles (`references.rs`), de même sans changer `VERSION` ;
//!   references/<id>.<ext> leurs fichiers (images copiées telles quelles, images extraites des vidéos
//!                         dans references/<id>/) : une version qui ne les connaît pas les laisse là
//!   scenes3d.json        scènes 3D de blocage, par plan, et décors par séquence (`scene3d.rs`,
//!                         `scene3dstore.rs`), de même sans changer `VERSION` ; le calque « Fond 3D »
//!                         où une scène est envoyée est un calque bitmap ordinaire
//!   rognages.json         contenu rogné au début et à la fin des plans (`rognage.rs`), de même sans
//!                         changer `VERSION` ; ses dessins restent dans dessins/ (une version qui ne
//!                         le connaît pas les efface à l'enregistrement : le plan reste tel qu'il est)
//!   transitions.json      transitions du montage (`transitions.rs`), de même sans changer `VERSION` :
//!                         une version qui ne le connaît pas le laisse en place et montre le film
//!                         sans les fondus
//!   effets_son.json       réglages d'effets des extraits et des pistes son (`audiofx.rs`), de même
//!   sons_proposes.json    sons proposés par la lecture du storyboard (`indications.rs`), de même sans
//!                         changer `VERSION` ; les sons générés, eux, restent dans le cache de SHOT
//!                         jusqu'à leur acceptation (copiés alors dans sons/)
//!   crayonnes.json        la planche de crayonnés rapides (`crayonnes.rs`), de même sans changer
//!                         `VERSION` : absent, il n'y a pas de planche ; une version qui ne le connaît
//!                         pas le laisse en place
//!   envois.json           les envois (`envois.rs`) : nom, date et empreinte de chaque plan, de même sans
//!                         changer `VERSION` ; absent, il n'y a pas d'envoi
//!   versions/             les instantanés des envois et leurs dessins, rangés par contenu
//!                         (`versions/dessins/<empreinte>.png`) : jamais effacés
//!   plans_versions.json   les autres versions de chaque plan (`planvers.rs`), de même sans changer
//!                         `VERSION` ; leurs dessins sont dans dessins/ ; absent, aucun plan n'a de version
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
/// 24 (2026-10-18) : couleur de repère des plans (`couleur` : rang dans la palette des marques).
/// 25 (2026-10-18, rendu des brosses, étape 5) : les traits vectoriels gardent leur `trace`
/// (appui et instant par point en entiers — `appui` au 1/65535, `instants` en dixièmes de
/// milliseconde depuis le premier —, toucher, couleur B) et leur réglage complet, et sont rejoués
/// par le moteur bitmap. Comme pour tout nouveau format, une version précédente de Pelure **refuse**
/// d'ouvrir le projet (`load` : « créé par une version plus récente ») ; elle ne le lit pas en
/// rendu approché.
/// 26 (2026-10-19) : tremblement du trait des calques vectoriels (effet `tremblement` de la pile
/// `effets` : intensité, fréquence, variantes, vitesse, épaisseur, graine, animer à l'arrêt ; voir
/// `tremblement.rs`). Une version précédente refuse le projet au lieu de montrer le dessin sans
/// son tremblement (elle ne connaît pas la sorte d'effet) : le film changerait sans prévenir.
/// 27 (2026-10-19) : calques d'effets (`calque_effet` d'un calque : ajustement, qui applique sa pile
/// d'effets à tout ce qui est dessous, ou incrustation de texte sur l'image finale — timecode, numéro
/// d'image, nom du plan… ; voir `calqueffet.rs`) et sept effets « Cinéma » dans la pile `effets`
/// (flash, image d'impact, lueur diffuse, secousse, vignettage, grain, aberration chromatique ; voir
/// `fxcine.rs`). Une version précédente refuse le projet : elle en ferait des calques vides et
/// perdrait l'effet, le film changerait sans prévenir.
/// 30 (2026-10-20) : calques de couleur (`calque_couleur` d'un calque : rang du calque de trait, dessins de
/// couleur qui répondent aux dessins du trait, fermeture des trous, pinceau dans la zone ; voir
/// `calquecouleur.rs`) et toile infinie (`dessins_hors`). Une version précédente refuse le projet : elle en
/// ferait des calques bitmap ordinaires aux images indépendantes, et la couleur ne suivrait plus le trait.
/// (Les numéros 28 et 29 sont ceux d'autres chantiers menés en parallèle, restés sans changement de format.)
/// 31 (2026-10-20) : effets du trait des calques vectoriels dans la pile `effets` (écriture du trait,
/// crayonné multiple, respiration, ondulation du trait, pointillés, désagrégation ; voir
/// `fxtrait.rs`). Une version précédente refuse le projet : elle ne connaît pas ces sortes d'effet et
/// montrerait le dessin sans eux, le film changerait sans prévenir.
/// 32 (2026-10-20) : effets de matière dans la pile `effets` (liseré, trait sec, encre qui bave, dégradé le
/// long du trait, changer de plume ; voir `fxmatiere.rs`). Même raisonnement que pour la 31 : une version
/// précédente refuse le projet au lieu d'en montrer le dessin sans liseré, sans dégradé ni plume (et la
/// plume est désignée par un numéro tiré du nom du réglage de pinceau). Un projet de version 31 ou moins
/// s'ouvre tel quel.
/// 33 (2026-10-20, notes d'Anthony « calques / effets / montage ») : interrupteur « fx » d'un calque
/// (`effets_eteints` : la pile d'effets est éteinte d'un coup, sans être vidée). Une version précédente
/// refuse le projet : elle montrerait les effets d'un calque que le réalisateur a éteints, le film
/// changerait sans prévenir. Le champ est absent (donc allumé) tant qu'on n'éteint rien. Un projet de
/// version 32 ou moins s'ouvre tel quel, effets allumés. (Une autre branche peut aussi prendre le numéro 33
/// pour ses propres champs : chacun a son nom, les deux se lisent sous le même numéro.)
pub const VERSION: u32 = 33;
const DRAWINGS: &str = "dessins";
/// Fils qui lisent ou écrivent les dessins d'un projet en parallèle : au-delà de huit, le disque et
/// la mémoire (une feuille décodée pèse 33 Mo) limitent plus que le processeur.
const LOAD_THREADS: usize = 8;
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
    /// Numérotation des plans verrouillée (`numerotation.rs` ; absent : suivie, sans changer `VERSION`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    numerotation_verrouillee: bool,
    /// Propriétés du projet (format 17 ; absentes : vides).
    #[serde(default, skip_serializing_if = "PropertiesEntry::is_empty")]
    proprietes: PropertiesEntry,
    /// Dessins bitmap qui ont quelque chose hors de la feuille (format 30, toile infinie) : leur atlas
    /// `dessins/<id>_hors.png` et son index `<id>_hors.json` ; absent : aucun.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dessins_hors: Vec<u64>,
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
    /// Numéro propre, en numérotation verrouillée (`numerotation.rs` ; absent : le rang).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    numero: String,
    scenes: Vec<SceneEntry>,
}

#[derive(Serialize, Deserialize)]
struct SceneEntry {
    id: u64,
    #[serde(default)]
    nom: String,
    /// Numéro propre du plan, en numérotation verrouillée (absent : le rang).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    numero: String,
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
    /// Couleur de repère du plan, rang dans `marks::COLORS` (format 24 ; absente : aucune).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    couleur: Option<u8>,
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
    /// Interrupteur « fx » du calque (format 33) : vrai, la pile `effets` est éteinte d'un coup sans être
    /// vidée. Absent : allumée.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    effets_eteints: bool,
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
    /// Calque d'effet (format 27) : ajustement ou incrustation ; absent : calque ordinaire. Ses
    /// expositions sont les images où il agit (un dessin vide tenu).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    calque_effet: Option<crate::calqueffet::Entree>,
    /// Calque de couleur (format 30) : il colore le calque de trait de ce rang ; absent : calque
    /// ordinaire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    calque_couleur: Option<CouleurEntry>,
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
    /// Image du plan où commence la vidéo (négative : le début du plan a été rogné).
    #[serde(default)]
    debut: i64,
    ext: String,
}

/// Le lien d'un calque de couleur avec son calque de trait (format 30).
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct CouleurEntry {
    /// Rang, dans le plan, du calque de trait.
    #[serde(rename = "trait")]
    ligne: usize,
    /// Les couples (dessin du trait, dessin de couleur qui lui répond).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dessins: Vec<[u64; 2]>,
    /// Fermeture des trous (pixels de chaque côté du trou).
    #[serde(default = "default_couleur_trous")]
    trous: u8,
    /// Un trait de pinceau reste dans la zone fermée où il commence.
    #[serde(default = "yes")]
    zone: bool,
}

fn default_couleur_trous() -> u8 {
    crate::calquecouleur::TROUS
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
    // Au plus la durée d'un plan (`rognage::MAX_PLAN`) : un manifeste abîmé qui annonce des milliards
    // d'images remplirait la mémoire.
    runs.iter()
        .flat_map(|r| std::iter::repeat_n(r.dessin, r.images.min(crate::rognage::MAX_PLAN)))
        .take(crate::rognage::MAX_PLAN)
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
    for sub in [DRAWINGS, SOUNDS, MODELS, VIDEOS, crate::references::DIR] {
        if dir.join(sub).exists() {
            return Err(format!(
                "le dossier {} contient déjà un dossier « {sub} ». Choisis un autre nom.",
                dir.display()
            ));
        }
    }
    Ok(())
}

/// Écrit `bytes` dans `path` sans jamais laisser un fichier à moitié écrit : tout va d'abord dans
/// `<path>.tmp`, qui prend la place de `path` d'un seul geste (renommage, atomique sur Windows).
/// Un échec (disque plein, droits) ou un plantage au milieu laisse `path` tel qu'il était.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_atomic_with(path, bytes, false)
}

/// Comme `write_atomic`, en attendant que le disque ait reçu les octets avant le renommage : pour
/// le manifeste et les petits fichiers qui le suivent (une coupure de courant ne laisse pas un
/// fichier vide à la place d'un bon).
pub(crate) fn write_atomic_synced(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_atomic_with(path, bytes, true)
}

/// Une texture de trait (pointe ou papier) en PNG de niveaux de gris, comme `textures/` la garde.
pub(crate) fn texture_png(img: &GrayImage) -> Result<Vec<u8>, String> {
    let gray = image::GrayImage::from_raw(img.width as u32, img.height as u32, crate::brushtex::quantized(img)).ok_or("texture de taille invalide")?;
    let mut bytes = Vec::new();
    gray.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(bytes)
}

fn write_atomic_with(path: &Path, bytes: &[u8], sync: bool) -> Result<(), String> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let written = (|| -> std::io::Result<()> {
        let mut file = fs::File::create(&tmp)?;
        std::io::Write::write_all(&mut file, bytes)?;
        if sync {
            file.sync_all()?;
        }
        Ok(())
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(format!("{} : {e}", tmp.display()));
    }
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("{} : {e}", path.display())
    })
}

/// La peinture de la strate Fond du dessin `id` (format 23), à côté de lui.
fn fond_path(dir: &Path, id: CelId) -> PathBuf {
    dir.join(DRAWINGS).join(format!("{id}_fond.png"))
}

/// La coupe de la strate Fond du dessin `id` (où le fond est retiré à la gomme).
fn coupe_path(dir: &Path, id: CelId) -> PathBuf {
    dir.join(DRAWINGS).join(format!("{id}_coupe.png"))
}

/// Ce que le dessin bitmap `id` a hors de la feuille (toile infinie, format 30) : un PNG qui range
/// ses tuiles côte à côte (« atlas »), et leurs coordonnées de tuile dans un petit fichier JSON.
fn hors_png_path(dir: &Path, id: CelId) -> PathBuf {
    dir.join(DRAWINGS).join(format!("{id}_hors.png"))
}

fn hors_json_path(dir: &Path, id: CelId) -> PathBuf {
    dir.join(DRAWINGS).join(format!("{id}_hors.json"))
}

/// Les coordonnées des tuiles de l'atlas hors-feuille, dans l'ordre de l'atlas (de gauche à droite,
/// puis de haut en bas), et sa largeur en tuiles.
#[derive(Serialize, Deserialize)]
struct HorsIndex {
    colonnes: usize,
    tuiles: Vec<[i32; 2]>,
}

/// Le plus de tuiles qu'un atlas hors-feuille accepte à la lecture (un fichier abîmé qui en annonce
/// des millions ne doit pas remplir la mémoire).
const MAX_HORS_TUILES: usize = 60_000;

/// L'atlas des tuiles `tiles`, et son index.
fn hors_atlas(tiles: &[((i32, i32), crate::raster::Tile)]) -> (Raster, HorsIndex) {
    let cols = (tiles.len() as f64).sqrt().ceil().max(1.0) as usize;
    let rows = tiles.len().div_ceil(cols).max(1);
    let mut atlas = Raster::new(cols * crate::raster::TILE, rows * crate::raster::TILE);
    for (i, (_, tile)) in tiles.iter().enumerate() {
        let id = (i / cols) * atlas.tiles_x + i % cols;
        atlas.tiles[id] = Some(tile.clone());
    }
    (atlas, HorsIndex { colonnes: cols, tuiles: tiles.iter().map(|((x, y), _)| [*x, *y]).collect() })
}

/// Remet dans le dessin `cel` ce qu'il avait hors de la feuille (rien, sans bruit, si les fichiers
/// sont abîmés : un avertissement le dit).
fn read_hors(dir: &Path, id: CelId, cel: &mut Cel) -> Result<(), String> {
    let text = fs::read_to_string(hors_json_path(dir, id)).map_err(|e| format!("dessin {id} : ce qu'il a hors de la feuille est illisible ({e})"))?;
    let index: HorsIndex = serde_json::from_str(&text).map_err(|e| format!("dessin {id} : ce qu'il a hors de la feuille est illisible ({e})"))?;
    if index.tuiles.len() > MAX_HORS_TUILES || index.colonnes == 0 || index.colonnes > 4096 {
        return Err(format!("dessin {id} : ce qu'il a hors de la feuille est démesuré"));
    }
    let tile = crate::raster::TILE;
    let atlas = read_rgba(&hors_png_path(dir, id), |pw, ph, data| {
        (pw % tile == 0 && ph % tile == 0 && pw / tile >= index.colonnes && (ph / tile) * index.colonnes >= index.tuiles.len()).then(|| Raster::from_rgba(pw, ph, data))
    })
    .map_err(|e| format!("dessin {id} : ce qu'il a hors de la feuille est illisible ({e})"))?
    .ok_or_else(|| format!("dessin {id} : l'image de ce qu'il a hors de la feuille ne correspond pas à son index"))?;
    for (i, [tx, ty]) in index.tuiles.iter().enumerate() {
        let slot = (i / index.colonnes) * atlas.tiles_x + i % index.colonnes;
        if let Some(Some(t)) = atlas.tiles.get(slot) {
            cel.raster.absorb_beyond(*tx, *ty, t.clone());
        }
    }
    Ok(())
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

/// Les dessins bitmap parmi `used` qui ont quelque chose hors de la feuille (toile infinie). Un dessin
/// vectoriel ne garde rien à part : ses traits se rendent à la lecture, hors de la feuille aussi.
fn hors_of(doc: &Document, used: &BTreeSet<CelId>) -> BTreeSet<CelId> {
    used.iter().copied().filter(|id| doc.cels.get(id).is_some_and(|c| c.vector.is_none() && c.raster.has_beyond())).collect()
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
    /// Les instantanés d'envois écrits (`envois.rs`) : l'application libère leur mémoire.
    pub snapshots: Vec<u64>,
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
    /// Les tuiles d'un dessin hors de la feuille (toile infinie), rangées en atlas à l'écriture.
    Atlas(Vec<((i32, i32), crate::raster::Tile)>),
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
    /// Fichiers de sons et de modèles 3D du dossier que cette version n'a pas pu lire à l'ouverture :
    /// l'enregistrement ne les efface pas (ils ne sont plus dans le projet, mais ils sont à
    /// Anthony : une autre version les lira, ou il les réparera).
    kept: BTreeSet<PathBuf>,
    /// Dessins à garder dans le dossier : ceux du projet, et les illisibles.
    used: BTreeSet<CelId>,
    /// Dessins qui ont une strate Fond peinte (format 23).
    fonds: BTreeSet<CelId>,
    /// Dessins bitmap qui ont quelque chose hors de la feuille (toile infinie, format 30).
    hors: BTreeSet<CelId>,
    used_sounds: BTreeSet<u64>,
    used_models: BTreeSet<u64>,
    used_textures: BTreeSet<(TextureKind, u32)>,
    /// Le fichier des commentaires (`commentaires.json`) ; rien : il n'y en a plus, le fichier part.
    comments: Option<Vec<u8>>,
    /// Le fichier du contenu rogné (`rognages.json`) ; rien : il n'y en a plus, le fichier part.
    reserves: Option<Vec<u8>>,
    /// Le fichier des transitions (`transitions.json`) ; rien : il n'y en a plus, le fichier part.
    transitions: Option<Vec<u8>>,
    /// Le fichier des effets des sons (`effets_son.json`) ; rien : il n'y en a plus, le fichier part.
    sound_fx: Option<Vec<u8>>,
    /// Le fichier des sons proposés (`sons_proposes.json`) ; rien : il n'y en a plus, le fichier part.
    sons_proposes: Option<Vec<u8>>,
    /// Les références visuelles : fichiers à recopier dans `references/`, `references.json`, et ce
    /// qui peut s'effacer (`references.rs`).
    references: crate::references::SavePlan,
    /// La planche de crayonnés (`crayonnes.json`) : une copie, mise en fichier par le fil
    /// d'écriture (l'encodage des peintures ne se fait pas sur le fil de l'interface) ; rien :
    /// la planche est vide, le fichier part (s'il est lisible).
    crayonnes: Option<crate::crayonnes::Planche>,
    /// Le fichier des scènes 3D (`scenes3d.json`) ; rien : il n'y en a plus, le fichier part.
    scenes3d: Option<Vec<u8>>,
    /// Le fichier des envois (`envois.json`) ; rien : il n'y en a pas (un fichier illisible reste là).
    envois: Option<Vec<u8>>,
    /// Le fichier des autres versions des plans (`plans_versions.json`) ; rien : il n'y en a pas.
    plans_versions: Option<Vec<u8>>,
    /// Un `plans_versions.json` que cette version ne sait pas lire est là : ses dessins sont dans `dessins/`
    /// et rien ne les cite, l'enregistrement n'en efface aucun.
    keep_drawings: bool,
    /// Les instantanés d'envois à écrire dans `versions/`, les dessins de `dessins/` qui ont déjà
    /// leur contenu, et l'ancien dossier d'où recopier `versions/` (« Enregistrer sous », `envois.rs`).
    snapshots: Vec<Arc<crate::envois::Instantane>>,
    snapshot_reuse: HashMap<u64, PathBuf>,
    versions_from: Option<PathBuf>,
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
    let hors = hors_of(doc, &used);
    // Les fichiers déjà là, d'une seule lecture du dossier : un `exists` par dessin coûtait 0,15 ms
    // chacun sur le fil de l'interface (450 ms pour un enregistrement de 3 000 dessins où rien
    // n'avait changé).
    let present: std::collections::HashSet<std::ffi::OsString> =
        fs::read_dir(dir.join(DRAWINGS)).map(|entries| entries.flatten().map(|e| e.file_name()).collect()).unwrap_or_default();
    let there = |p: &Path| p.file_name().is_some_and(|n| present.contains(n));
    for &id in &used {
        let Some(cel) = doc.cels.get(&id) else { continue };
        let vector = cel.vector.is_some();
        let path = drawing_path(dir, id, vector);
        if written.get(&id) == Some(&cel.version)
            && there(&path)
            && (!fonds.contains(&id) || there(&fond_path(dir, id)))
            && (!hors.contains(&id) || (there(&hors_png_path(dir, id)) && there(&hors_json_path(dir, id))))
        {
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
        // Ce que le dessin a hors de la feuille (toile infinie) : son atlas et son index, à côté.
        if hors.contains(&id) {
            let elsewhere = |path_of: fn(&Path, CelId) -> PathBuf| same_elsewhere.map(|d| path_of(d, id)).filter(|p| p.exists());
            match (elsewhere(hors_png_path), elsewhere(hors_json_path)) {
                (Some(png), Some(json)) => {
                    drawings.push((hors_png_path(dir, id), DrawingData::CopyFrom(png)));
                    drawings.push((hors_json_path(dir, id), DrawingData::CopyFrom(json)));
                }
                _ => {
                    let tiles = cel.raster.beyond_sheet_tiles();
                    let index = HorsIndex { colonnes: (tiles.len() as f64).sqrt().ceil().max(1.0) as usize, tuiles: tiles.iter().map(|((x, y), _)| [*x, *y]).collect() };
                    drawings.push((hors_json_path(dir, id), DrawingData::Bytes(serde_json::to_vec(&index).map_err(|e| e.to_string())?)));
                    drawings.push((hors_png_path(dir, id), DrawingData::Atlas(tiles)));
                }
            }
        }
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
    for v in doc.panels.iter().chain(doc.plans_ranges()).flat_map(|p| &p.layers).filter_map(|l| l.video.as_ref()) {
        let to = dir.join(VIDEOS).join(v.id.to_string());
        if let Some(from) = crate::video::dir_of(v.id).filter(|from| *from != to) {
            if !videos.iter().any(|(w, _, _): &(crate::video::VideoClip, PathBuf, PathBuf)| w.id == v.id) {
                videos.push((v.clone(), from, to));
            }
        }
    }
    let manifest = serde_json::to_vec_pretty(&manifest_of(doc)).map_err(|e| e.to_string())?;
    let unreadable = unreadable_in(dir);
    let keep_drawings = crate::planvers::illisible(dir);
    // Les instantanés d'envois pas encore écrits (`envois.rs`) : un dessin du projet qui a déjà leur contenu
    // dans `dessins/` est recopié (les dessins s'écrivent avant eux), pas encodé de nouveau ; un autre
    // dossier que celui du projet (« Enregistrer sous ») reçoit aussi les instantanés déjà écrits.
    let snapshots = doc.envois.pending.clone();
    let snapshot_reuse = crate::envois::dessins_reutilisables(&snapshots, written, &|id| Some(drawing_path(dir, id, false)));
    let versions_from = if reuse.is_none() {
        doc.envois.dossier.as_ref().filter(|d| d.as_path() != dir).map(|d| d.join(crate::envois::DIR)).filter(|v| v.is_dir())
    } else {
        None
    };
    Ok(SaveJob {
        manifest_path: manifest_path.to_path_buf(),
        drawings,
        sounds,
        models,
        textures,
        videos,
        manifest,
        has_unreadable: !unreadable.is_empty() || keep_drawings,
        kept: kept_files_in(dir),
        used: used.union(&unreadable).copied().collect(),
        fonds: fonds.union(&unreadable).copied().collect(),
        hors: hors.union(&unreadable).copied().collect(),
        used_sounds,
        used_models,
        used_textures,
        comments: crate::commentaires::to_bytes(doc)?,
        reserves: crate::rognage::to_bytes(doc)?,
        transitions: crate::transitions::to_bytes(doc)?,
        sound_fx: crate::audiofx::to_bytes(doc)?,
        sons_proposes: crate::indications::to_bytes(doc)?,
        references: crate::references::plan_save(doc, dir)?,
        crayonnes: (!doc.crayonnes.is_blank()).then(|| doc.crayonnes.clone()),
        scenes3d: crate::scene3dstore::to_bytes(doc)?,
        envois: crate::envois::to_bytes(doc)?,
        plans_versions: crate::planvers::to_bytes(doc)?,
        keep_drawings,
        snapshots,
        snapshot_reuse,
        versions_from,
    })
}

/// Les sons proposés, lus dans `sons_proposes.json` (absent : aucun). Un fichier illisible ou d'une
/// version plus récente n'empêche pas d'ouvrir le projet : on le dit, sans propositions, et il reste
/// en place (`write_sidecar`).
fn load_sons_proposes(dir: &Path, warnings: &mut Vec<String>) -> crate::indications::SonsProposes {
    let path = dir.join(crate::indications::FILE);
    match fs::read_to_string(&path) {
        Ok(text) => match crate::indications::parse(&text) {
            Ok(mut s) => {
                crate::indications::verifier_fichiers(&mut s);
                s
            }
            Err(e) => {
                warnings.push(format!("{} : {e} ; les sons proposés ne sont pas chargés.", crate::indications::FILE));
                Default::default()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
        Err(e) => {
            warnings.push(format!("{} : {e} ; les sons proposés ne sont pas chargés.", path.display()));
            Default::default()
        }
    }
}

/// La planche de crayonnés du projet, lue dans `crayonnes.json` (absent : une planche neuve). Un
/// fichier illisible n'empêche pas d'ouvrir le projet : on le dit, sans planche ; il reste en place
/// (`write_sidecar`).
fn load_crayonnes(dir: &Path, warnings: &mut Vec<String>) -> crate::crayonnes::Planche {
    let path = dir.join(crate::crayonnes::FILE);
    match fs::read_to_string(&path) {
        Ok(text) => crate::crayonnes::parse(&text).unwrap_or_else(|e| {
            warnings.push(format!("{} : {e} ; la planche de crayonnés n'est pas chargée.", crate::crayonnes::FILE));
            Default::default()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
        Err(e) => {
            warnings.push(format!("{} : {e} ; la planche de crayonnés n'est pas chargée.", path.display()));
            Default::default()
        }
    }
}

/// Les effets des sons, lus dans `effets_son.json` (absent : aucun). Un fichier illisible ou d'une
/// version plus récente n'empêche pas d'ouvrir le projet : on le dit, sans effets, et il reste en
/// place (`write_sidecar`).
fn load_sound_fx(dir: &Path, warnings: &mut Vec<String>) -> Option<crate::audiofx::Loaded> {
    let path = dir.join(crate::audiofx::FILE);
    match fs::read_to_string(&path) {
        Ok(text) => match crate::audiofx::parse(&text) {
            Ok(l) => Some(l),
            Err(e) => {
                warnings.push(format!("{} : {e} ; les effets des sons ne sont pas chargés.", crate::audiofx::FILE));
                None
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            warnings.push(format!("{} : {e} ; les effets des sons ne sont pas chargés.", path.display()));
            None
        }
    }
}

/// Les transitions du montage, lues dans `transitions.json` (absent : aucune). Un fichier illisible
/// ou d'une version plus récente n'empêche pas d'ouvrir le projet : on le dit, sans transitions, et
/// il reste en place (`write_sidecar`).
fn load_transitions(dir: &Path, warnings: &mut Vec<String>) -> Vec<crate::transitions::Transition> {
    let path = dir.join(crate::transitions::FILE);
    match fs::read_to_string(&path) {
        Ok(text) => crate::transitions::parse(&text).unwrap_or_else(|e| {
            warnings.push(format!("{} : {e} ; les transitions ne sont pas chargées.", crate::transitions::FILE));
            Vec::new()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            warnings.push(format!("{} : {e} ; les transitions ne sont pas chargées.", path.display()));
            Vec::new()
        }
    }
}

/// Le contenu rogné des plans, lu dans `rognages.json` (absent : rien n'est rogné). Un fichier
/// illisible n'empêche pas d'ouvrir le projet : on le dit, sans contenu rogné.
fn load_reserves(dir: &Path, warnings: &mut Vec<String>) -> Option<crate::rognage::Loaded> {
    let path = dir.join(crate::rognage::FILE);
    match fs::read_to_string(&path) {
        Ok(text) => match crate::rognage::parse(&text) {
            Ok(r) => Some(r),
            Err(e) => {
                warnings.push(format!("{} : {e} ; le contenu rogné des plans n'est pas chargé.", crate::rognage::FILE));
                None
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            warnings.push(format!("{} : {e} ; le contenu rogné des plans n'est pas chargé.", path.display()));
            None
        }
    }
}

/// Les scènes 3D du projet, lues dans `scenes3d.json` (absent : aucune scène). Un fichier illisible
/// (abîmé, ou d'une version plus récente) n'empêche pas d'ouvrir le projet : on le dit, sans scènes ;
/// il reste en place (`write_sidecar`).
fn load_scenes3d(dir: &Path, warnings: &mut Vec<String>) -> crate::scene3d::Scenes3d {
    let path = dir.join(crate::scene3dstore::FILE);
    match fs::read(&path) {
        Ok(bytes) => String::from_utf8(bytes)
            .map_err(|_| "le fichier n'est pas du texte (UTF-8)".to_string())
            .and_then(|text| crate::scene3dstore::parse(&text))
            .unwrap_or_else(|e| {
                warnings.push(format!("{} : {e} ; les scènes 3D ne sont pas chargées.", crate::scene3dstore::FILE));
                Default::default()
            }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
        Err(e) => {
            warnings.push(format!("{} : {e} ; les scènes 3D ne sont pas chargées.", path.display()));
            Default::default()
        }
    }
}

/// Les commentaires du projet, lus dans `commentaires.json` (absent : aucun commentaire). Un
/// fichier illisible n'empêche pas d'ouvrir le projet : on le dit, sans commentaires.
fn load_comments(dir: &Path, warnings: &mut Vec<String>) -> Vec<crate::commentaires::Comment> {
    let path = dir.join(crate::commentaires::FILE);
    match fs::read_to_string(&path) {
        Ok(text) => crate::commentaires::parse(&text).unwrap_or_else(|e| {
            warnings.push(format!("{} : {e} ; les commentaires ne sont pas chargés.", crate::commentaires::FILE));
            Vec::new()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            warnings.push(format!("{} : {e} ; les commentaires ne sont pas chargés.", path.display()));
            Vec::new()
        }
    }
}

/// Écrit le fichier annexe `path` (`bytes`), ou le retire s'il n'y a plus rien à y mettre — sans
/// jamais détruire un fichier que cette version ne sait pas lire (`lisible` en juge). Un
/// `commentaires.json` écrit par une version plus récente, ou abîmé, s'ouvre sans ses commentaires
/// (avec un avertissement) : « aucun commentaire » ne veut alors pas dire « fichier à effacer ». Il
/// reste en place s'il n'y a rien à écrire ; sinon il est mis de côté sous le nom `<fichier>.illisible`
/// avant que le neuf prenne sa place.
pub(crate) fn write_sidecar(path: &Path, bytes: Option<&[u8]>, lisible: impl Fn(&str) -> bool) -> Result<(), String> {
    let existant = match fs::read(path) {
        Ok(b) => Some(String::from_utf8(b).ok().filter(|t| lisible(t.trim_start_matches('\u{feff}'))).is_some()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => Some(false),
    };
    match (bytes, existant) {
        (Some(b), Some(false)) => {
            let mut aside = None;
            for k in 1..100 {
                let mut name = path.as_os_str().to_owned();
                name.push(if k == 1 { ".illisible".to_string() } else { format!(".illisible{k}") });
                let candidate = PathBuf::from(name);
                if !candidate.exists() {
                    aside = Some(candidate);
                    break;
                }
            }
            if let Some(to) = aside {
                fs::rename(path, &to).map_err(|e| format!("{} : {e}", path.display()))?;
            }
            write_atomic_synced(path, b)
        }
        (Some(b), _) => write_atomic_synced(path, b),
        (None, Some(false)) => Ok(()),
        (None, _) => {
            let _ = fs::remove_file(path);
            Ok(())
        }
    }
}

impl SaveJob {
    /// Écrit les dessins, puis les sons, puis le manifeste (en dernier : un manifeste ne cite
    /// jamais un dessin pas encore écrit), puis efface ce qui ne sert plus.
    pub fn run(self) -> Result<SaveReport, String> {
        self.run_with(None, LOAD_THREADS)
    }

    /// Comme `run`, sur au plus `threads` fils, en comptant dans `progress` les dessins écrits (un
    /// autre fil l'affiche pendant que celui-ci écrit : `enregistrement.rs`).
    pub fn run_with(self, progress: Option<&Progress>, threads: usize) -> Result<SaveReport, String> {
        let dir = self.manifest_path.parent().ok_or("chemin de projet invalide")?;
        fs::create_dir_all(dir.join(DRAWINGS)).map_err(|e| format!("{} : {e}", dir.display()))?;
        let mut count = 0;
        if let Some(p) = progress {
            p.total.store(self.drawings.len(), std::sync::atomic::Ordering::Relaxed);
        }
        // Les dessins s'encodent et s'écrivent en parallèle (11 ms de PNG chacun) ; la première
        // erreur, dans l'ordre des dessins, est celle qui est rendue.
        let written = crate::parallele::par_map(&self.drawings, threads, |_, (path, data)| -> Result<(), String> {
            let bytes: std::borrow::Cow<[u8]> = match data {
                DrawingData::Bytes(b) => std::borrow::Cow::Borrowed(b),
                DrawingData::Raster(r) => std::borrow::Cow::Owned(encode_png(&r.to_rgba(), r.width, r.height)?),
                DrawingData::Atlas(tiles) => {
                    let (atlas, _) = hors_atlas(tiles);
                    std::borrow::Cow::Owned(encode_png(&atlas.to_rgba(), atlas.width, atlas.height)?)
                }
                DrawingData::CopyFrom(from) => std::borrow::Cow::Owned(fs::read(from).map_err(|e| format!("{} : {e}", from.display()))?),
            };
            let result = write_atomic(path, &bytes);
            if let Some(p) = progress {
                p.done.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            result
        });
        for result in written {
            result?;
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
            write_atomic(path, &texture_png(img)?)?;
            count += 1;
        }
        for (clip, from, to) in &self.videos {
            count += crate::video::copy_frames(clip, from, to)?;
        }
        // Les commentaires : leur fichier à côté du manifeste (sans commentaire, il part).
        // Ni l'un ni l'autre ne détruit un fichier que cette version ne sait pas lire (`write_sidecar`),
        // et les deux s'écrivent atomiquement et sur le disque (`write_atomic_synced`).
        write_sidecar(&dir.join(crate::commentaires::FILE), self.comments.as_deref(), |t| crate::commentaires::parse(t).is_ok())?;
        // Le contenu rogné des plans : son fichier à côté du manifeste (rien de rogné, il part).
        write_sidecar(&dir.join(crate::rognage::FILE), self.reserves.as_deref(), |t| crate::rognage::parse(t).is_ok())?;
        // Les transitions du montage, de même.
        write_sidecar(&dir.join(crate::transitions::FILE), self.transitions.as_deref(), |t| crate::transitions::parse(t).is_ok())?;
        // Et les effets des sons.
        write_sidecar(&dir.join(crate::audiofx::FILE), self.sound_fx.as_deref(), |t| crate::audiofx::parse(t).is_ok())?;
        // Et les sons proposés par la lecture du storyboard.
        write_sidecar(&dir.join(crate::indications::FILE), self.sons_proposes.as_deref(), |t| crate::indications::parse(t).is_ok())?;
        // Les références visuelles : leurs fichiers dans `references/`, puis `references.json`.
        count += self.references.write(dir)?;
        // La planche de crayonnés rapides : de même (mise en fichier ici, sur le fil d'écriture).
        let crayonnes = match &self.crayonnes {
            Some(p) => crate::crayonnes::bytes_of(p)?,
            None => None,
        };
        write_sidecar(&dir.join(crate::crayonnes::FILE), crayonnes.as_deref(), |t| crate::crayonnes::parse(t).is_ok())?;
        // Les scènes 3D de blocage (`scene3dstore.rs`), de même.
        write_sidecar(&dir.join(crate::scene3dstore::FILE), self.scenes3d.as_deref(), |t| crate::scene3dstore::parse(t).is_ok())?;
        // Les envois : les instantanés d'abord (dessins puis fichier), puis la liste qui les cite.
        count += crate::envois::ecrire(dir, &self.snapshots, &self.snapshot_reuse, self.versions_from.as_deref())?;
        write_sidecar(&dir.join(crate::envois::FILE), self.envois.as_deref(), |t| crate::envois::parse(t).is_ok())?;
        // Les autres versions des plans (leurs dessins sont ceux de `dessins/`, déjà écrits).
        write_sidecar(&dir.join(crate::planvers::FILE), self.plans_versions.as_deref(), |t| crate::planvers::parse(t).is_ok())?;
        write_atomic_synced(&self.manifest_path, &self.manifest)?;
        let removed = if self.keep_drawings { 0 } else { remove_unused(dir, &self.used, &self.fonds, &self.hors) }
            + remove_unused_sounds(dir, &self.used_sounds, &self.kept)
            + remove_unused_models(dir, &self.used_models, &self.kept)
            + if self.has_unreadable { 0 } else { remove_unused_textures(dir, &self.used_textures) }
            + self.references.cleanup(dir);
        Ok(SaveReport { written: count, removed, snapshots: self.snapshots.iter().map(|s| s.id).collect() })
    }
}

/// Efface les modèles 3D du dossier qui ne servent plus (seulement « <nombre>.pelure3d »).
fn remove_unused_models(dir: &Path, used: &BTreeSet<u64>, kept: &BTreeSet<PathBuf>) -> usize {
    let Ok(entries) = fs::read_dir(dir.join(MODELS)) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if kept.contains(&path) {
            continue;
        }
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
fn remove_unused_sounds(dir: &Path, used: &BTreeSet<u64>, kept: &BTreeSet<PathBuf>) -> usize {
    let Ok(entries) = fs::read_dir(dir.join(SOUNDS)) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if kept.contains(&path) {
            continue;
        }
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
fn remove_unused(dir: &Path, used: &BTreeSet<CelId>, fonds: &BTreeSet<CelId>, hors: &BTreeSet<CelId>) -> usize {
    let Ok(entries) = fs::read_dir(dir.join(DRAWINGS)) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        // Ce qu'un dessin a hors de la feuille : « <nombre>_hors.png » et « <nombre>_hors.json ».
        if let (Some(id), "png" | "json") = (stem.strip_suffix("_hors").and_then(|s| s.parse::<CelId>().ok()), ext) {
            if !hors.contains(&id) && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
            continue;
        }
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

/// L'entrée du manifeste pour un plan : tout ce que le fichier garde de lui. `key` donne le numéro sous
/// lequel chaque dessin y est cité (le même, pour un enregistrement ; l'empreinte du dessin, pour un
/// instantané d'envoi, qui se lit alors sans rien devoir aux numéros de la séance).
///
/// `digest` : ne garde que ce qui compte quand on se demande si le plan **a changé** (`envois.rs`) — ce
/// qui se voit et ce qui dure (dessins, calques réglés, effets, caméra, textes du storyboard, marques
/// d'image, durée) ; pas ce qui aide à travailler (couleur de repère du plan, signets, entrée et
/// sortie, notes dessinées sur la timeline, repère 3D, noms et marques de couleur des dessins, rôles
/// des images, verrous, calque replié, groupe de couleur, pelure d'oignon et pochoir, nom du calque).
fn panel_entry(panel: &Panel, key: &dyn Fn(CelId) -> CelId, garde: &dyn Fn(CelId) -> bool, digest: bool) -> PanelEntry {
    let mut entry = PanelEntry {
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
                noms: l.names.iter().filter(|(id, _)| l.frames.contains(&Some(**id))).map(|(id, n)| (key(*id), n.clone())).collect(),
                marques_couleur: l.color_marks.iter().filter(|(id, _)| l.frames.contains(&Some(**id))).map(|(id, k)| (key(*id), *k)).collect(),
                roles: l.roles.clone(),
                effets: l.fx.clone(),
                effets_eteints: !l.fx_enabled,
                ctg: l.ctg.as_ref().map(|c| CtgEntry {
                    sources: c.sources.iter().filter_map(|id| panel.layers.iter().position(|s| s.id == *id)).collect(),
                    trou: c.gap,
                }),
                video: l.video.as_ref().map(|v| VideoEntry { id: v.id, nom: v.name.clone(), images: v.frames, ips: v.fps, debut: v.start, ext: v.ext.clone() }),
                fond: l.fond,
                calque_effet: l.effet.as_ref().map(crate::calqueffet::Entree::from),
                calque_couleur: l.couleur.as_ref().and_then(|c| {
                    Some(CouleurEntry {
                        ligne: panel.layers.iter().position(|s| s.id == c.trait_id)?,
                        dessins: c.dessins.iter().filter(|(t, k)| garde(**t) && garde(**k)).map(|(t, k)| [key(*t), key(*k)]).collect(),
                        trous: c.trous,
                        zone: c.zone,
                    })
                }),
                expositions: to_exposures(&l.frames.iter().map(|f| f.map(key)).collect::<Vec<_>>()),
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
        couleur: panel.color,
        notes_dessinees: panel
            .cues
            .notes
            .iter()
            .map(|n| TimeNoteEntry { couleur: n.color, epaisseur: n.width, points: n.points.clone() })
            .collect(),
    };
    if digest {
        entry.signets.clear();
        (entry.entree, entry.sortie, entry.couleur) = (None, None, None);
        entry.notes_dessinees.clear();
        // Le repère 3D est un guide pour dessiner (ni à l'export ni dans les vignettes).
        entry.repere3d = None;
        // Les numéros du plan et de ses marques disent qui il est, pas ce qu'il contient : deux versions d'un
        // même plan (`planvers.rs`) se comparent sans eux.
        entry.id = 0;
        for m in &mut entry.marques {
            m.id = 0;
        }
        // Les numéros de calques de toute la scène sont neufs à chaque séance : seule leur présence compte.
        for l in &mut entry.calques {
            (l.nom, l.verrouille, l.pelure, l.transparence, l.pochoir, l.pochoir_negatif) = (String::new(), false, false, false, false, false);
            (l.position_verrouillee, l.groupe, l.replie, l.cassure_auto, l.creation_auto) = (false, 0, false, false, true);
            l.noms.clear();
            l.marques_couleur.clear();
            l.roles.clear();
            l.toute_la_scene = l.toute_la_scene.map(|_| 1);
        }
    }
    entry
}

/// Les octets qui disent ce qu'est un plan, pour l'empreinte « le plan a-t-il changé ? » (`envois.rs`) :
/// son entrée du manifeste réduite à ce qui compte (`panel_entry`), les dessins cités par leur
/// empreinte (`key`).
pub(crate) fn digest_bytes(panel: &Panel, key: &dyn Fn(CelId) -> CelId) -> Vec<u8> {
    serde_json::to_vec(&panel_entry(panel, key, &|c| dans_le_plan(panel, c), true)).unwrap_or_default()
}

/// L'entrée complète d'un plan pour un instantané (envoi, version) : le même JSON que dans le
/// manifeste, les dessins cités par `key`.
pub(crate) fn snapshot_value(panel: &Panel, key: &dyn Fn(CelId) -> CelId) -> Result<serde_json::Value, String> {
    serde_json::to_value(panel_entry(panel, key, &|c| dans_le_plan(panel, c), false)).map_err(|e| e.to_string())
}

/// Un instantané ne garde des couples trait / couleur (`calque_couleur`) que ceux dont les deux dessins
/// sont dans une timeline du plan : ce sont les seuls dessins qu'il emporte.
fn dans_le_plan(panel: &Panel, cel: CelId) -> bool {
    panel.layers.iter().any(|l| l.frames.contains(&Some(cel)))
}

/// En-tête d'un instantané : la feuille, le cadre, la cadence et le papier du projet au moment où il a été
/// pris (les dessins qu'il cite ont cette taille, même si le projet a changé de résolution depuis).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Tete {
    pub largeur: usize,
    pub hauteur: usize,
    pub cadre: [usize; 2],
    pub cadence: u32,
    pub papier: [u8; 3],
}

impl Tete {
    pub fn de(doc: &Document) -> Tete {
        Tete { largeur: doc.width, hauteur: doc.height, cadre: doc.frame, cadence: doc.fps, papier: doc.paper }
    }
}

/// Relit des plans d'un instantané : `entrees` (les valeurs de `snapshot_value`) deviennent un petit projet
/// dont les dessins sont ceux de `dir/dessins/<empreinte>.png|json`. C'est l'ouverture ordinaire d'un
/// projet (`load_manifest`), donc avec ses protections : un dessin manquant ou abîmé est un
/// avertissement, le plan s'ouvre quand même. `scene` : le numéro de scène que prend le plan.
pub(crate) fn charger_plans(dir: &Path, tete: &Tete, scene: u64, entrees: Vec<serde_json::Value>, quick: bool) -> Result<Loaded, String> {
    let panneaux: Vec<PanelEntry> = entrees
        .into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| format!("plan illisible : {e}")))
        .collect::<Result<_, _>>()?;
    // Une vidéo de l'instantané ne doit pas faire perdre au projet en cours le dossier de la sienne
    // (`video::register_dir` est commun à tout le programme).
    let videos: Vec<(u64, Option<PathBuf>)> =
        panneaux.iter().flat_map(|p| &p.calques).filter_map(|l| l.video.as_ref()).map(|v| (v.id, crate::video::dir_of(v.id))).collect();
    let manifest = Manifest {
        format: FORMAT.into(),
        version: VERSION,
        largeur: tete.largeur,
        hauteur: tete.hauteur,
        cadre: Some(tete.cadre),
        cadence: tete.cadence.max(1),
        papier: tete.papier,
        sequences: vec![SequenceEntry { id: 1, nom: String::new(), numero: String::new(), scenes: vec![SceneEntry { id: scene.max(1), nom: String::new(), numero: String::new(), panneaux }] }],
        sons: Vec::new(),
        pistes_son: Vec::new(),
        guides: Vec::new(),
        modeles: Vec::new(),
        planche: Vec::new(),
        planche_textes: Vec::new(),
        groupes: Vec::new(),
        image_de_depart: 1,
        fond: BackgroundEntry::default(),
        volume_general: 1.0,
        storyboard_verrouille: false,
        numerotation_verrouillee: false,
        proprietes: PropertiesEntry::default(),
        // Les instantanés ne gardent que la feuille de leurs dessins (pas leur hors-feuille, `monde.rs`).
        dessins_hors: Vec::new(),
    };
    let loaded = load_manifest(manifest, dir, quick, None);
    for (id, previous) in videos {
        if let Some(p) = previous {
            crate::video::register_dir(id, p);
        }
    }
    loaded
}

fn manifest_of(doc: &Document) -> Manifest {
    let mut sequences: Vec<SequenceEntry> = Vec::new();
    // Les dessins qui seront enregistrés : un calque de couleur n'écrit que les couples dont les deux
    // dessins le sont.
    let drawn = doc.used_cels();
    for (i, panel) in doc.panels.iter().enumerate() {
        let seq = doc.sequence_of(i);
        if sequences.last().is_none_or(|s| s.id != seq) {
            let nom = doc.sequences.iter().find(|s| s.id == seq).map(|s| s.name.clone()).unwrap_or_default();
            let numero = doc.sequences.iter().find(|s| s.id == seq).map(|s| s.number.clone()).unwrap_or_default();
            sequences.push(SequenceEntry {
                id: seq,
                nom,
                numero,
                scenes: Vec::new(),
            });
        }
        let scenes = &mut sequences.last_mut().expect("séquence ajoutée juste avant").scenes;
        if scenes.last().is_none_or(|s| s.id != panel.scene) {
            let nom = doc.scene(panel.scene).map(|s| s.name.clone()).unwrap_or_default();
            let numero = doc.scene(panel.scene).map(|s| s.number.clone()).unwrap_or_default();
            scenes.push(SceneEntry {
                id: panel.scene,
                nom,
                numero,
                panneaux: Vec::new(),
            });
        }
        scenes.last_mut().expect("scène ajoutée juste avant").panneaux.push(panel_entry(panel, &|c| c, &|c| drawn.contains(&c), false));
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
        numerotation_verrouillee: doc.numbering_locked,
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
        dessins_hors: hors_of(doc, &drawn).into_iter().collect(),
    }
}

/// Modèles 3D cités par au moins un panneau.
fn used_models(doc: &Document) -> BTreeSet<u64> {
    // Les modèles du repère 3D des plans, et ceux que les scènes 3D de blocage montrent.
    doc.panels
        .iter()
        .chain(doc.plans_ranges())
        .filter_map(|p| p.ref3d.as_ref().map(|r| r.model))
        .chain(doc.scenes3d.models_used())
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

/// Le plus grand numéro de séquence, de scène, de plan, de marque, de piste, d'extrait ou de son du manifeste.
fn top_id(m: &Manifest) -> u64 {
    let plans = m.sequences.iter().flat_map(|q| std::iter::once(q.id).chain(q.scenes.iter().flat_map(|s| std::iter::once(s.id).chain(s.panneaux.iter().flat_map(|p| std::iter::once(p.id).chain(p.marques.iter().map(|k| k.id)))))));
    let sons = m.pistes_son.iter().flat_map(|t| std::iter::once(t.id).chain(t.extraits.iter().map(|c| c.id))).chain(m.sons.iter().map(|s| s.id));
    plans.chain(sons).max().unwrap_or(0)
}

/// Un projet enregistré avec des identifiants en double (deux plans collés qui gardaient le même,
/// une scène reprise deux fois…) se relit avec tous ses plans : chaque entrée du manifeste est un
/// plan, une scène, une séquence à part entière. Le premier garde son numéro, les suivants en
/// reçoivent un neuf (au-dessus de `floor` et de tout ce que le manifeste contient). Renvoie le
/// nombre d'identifiants changés.
fn unique_ids(manifest: &mut Manifest, floor: u64) -> usize {
    let mut top = floor;
    for q in &manifest.sequences {
        top = top.max(q.id);
        for s in &q.scenes {
            top = top.max(s.id);
            for p in &s.panneaux {
                top = top.max(p.id).max(p.marques.iter().map(|m| m.id).max().unwrap_or(0));
            }
        }
    }
    top = top.max(manifest.pistes_son.iter().flat_map(|t| std::iter::once(t.id).chain(t.extraits.iter().map(|c| c.id))).max().unwrap_or(0));
    top = top.max(manifest.sons.iter().map(|s| s.id).max().unwrap_or(0));
    let mut changed = 0;
    let mut take = |seen: &mut BTreeSet<u64>, id: &mut u64| {
        if !seen.insert(*id) {
            top += 1;
            *id = top;
            seen.insert(top);
            changed += 1;
        }
    };
    let (mut seqs, mut scenes, mut cards) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    for q in &mut manifest.sequences {
        take(&mut seqs, &mut q.id);
        for s in &mut q.scenes {
            take(&mut scenes, &mut s.id);
            for p in &mut s.panneaux {
                take(&mut cards, &mut p.id);
                for m in &mut p.marques {
                    take(&mut cards, &mut m.id);
                }
            }
        }
    }
    changed
}

/// Ouvre le projet, dessins vectoriels rendus exactement (le moteur rejoué sur chaque trait : ce
/// que l'application affiche une fois le rejeu terminé).
pub fn load(manifest_path: &Path) -> Result<Loaded, String> {
    load_with(manifest_path, false, None)
}

/// Comme `load_quick_progress`, sans suivre l'avancement (essais).
#[cfg(test)]
pub fn load_quick(manifest_path: &Path) -> Result<Loaded, String> {
    load_with(manifest_path, true, None)
}

/// Ouvre le projet tout de suite : les dessins vectoriels texturés sont affichés en rendu rapide
/// (traits lisses, sans rejeu du moteur) et listés dans `approx`, pour un rendu exact en
/// arrière-plan. Règle d'Anthony : rien ne bloque l'interface. `progress` tient le compte des
/// dessins lus / à lire : l'interface l'affiche pendant qu'un autre fil ouvre le projet
/// (`ouverture.rs`).
pub fn load_quick_progress(manifest_path: &Path, progress: &Progress) -> Result<Loaded, String> {
    load_with(manifest_path, true, Some(progress))
}

/// Avancement d'une ouverture, lu par l'interface pendant qu'un autre fil travaille.
#[derive(Default)]
pub struct Progress {
    pub done: std::sync::atomic::AtomicUsize,
    pub total: std::sync::atomic::AtomicUsize,
}

fn load_with(manifest_path: &Path, quick: bool, progress: Option<&Progress>) -> Result<Loaded, String> {
    // (Des messages en français qui disent quel fichier : « stream did not contain valid UTF-8 »
    // ou « Le chemin d'accès spécifié est introuvable » ne disent pas ce qui ne va pas.)
    let octets = fs::read(manifest_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!("projet introuvable : {}", manifest_path.display())
        } else {
            format!("manifeste illisible : {} ({e})", manifest_path.display())
        }
    })?;
    let text = String::from_utf8(octets).map_err(|_| "manifeste illisible : le fichier n'est pas du texte (UTF-8), il est abîmé".to_string())?;
    let manifest: Manifest =
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("manifeste illisible : {e}"))?;
    if manifest.format != FORMAT {
        return Err("ce fichier n'est pas un projet SHOT".into());
    }
    if manifest.version > VERSION {
        return Err(format!(
            "projet créé par une version plus récente de SHOT (format {})",
            manifest.version
        ));
    }
    let dir = manifest_path.parent().unwrap_or(Path::new("."));
    load_manifest(manifest, dir, quick, progress)
}

/// La suite de l'ouverture, une fois le manifeste lu : les dessins de `dir/dessins/`, les sons, les
/// annexes, puis le document. (Les instantanés des envois passent aussi par là, `charger_plans`.)
fn load_manifest(mut manifest: Manifest, dir: &Path, quick: bool, progress: Option<&Progress>) -> Result<Loaded, String> {
    let (w, h) = (manifest.largeur, manifest.hauteur);
    if w == 0 || h == 0 {
        return Err("taille d'image invalide dans le manifeste".into());
    }
    // (Un manifeste abîmé qui annonce une feuille de milliards de pixels ferait demander à SHOT des
    // milliards de tuiles : il tomberait sans un mot.)
    if w > crate::vector::MAX_SIDE * 2 || h > crate::vector::MAX_SIDE * 2 {
        return Err(format!("taille d'image démesurée dans le manifeste ({w} × {h})"));
    }
    let mut warnings = Vec::new();
    // Les textures d'abord : les dessins vectoriels texturés en ont besoin pour leur rendu.
    load_textures(dir, &mut warnings);
    // Les autres versions des plans (`plans_versions.json`) : leurs plans passent par la même lecture que
    // les autres, dans une séquence à part qu'on retire ensuite du film (`planvers.rs`).
    let mut versions = crate::planvers::lire(dir, &mut warnings);
    let mut a_ranger: Option<(u64, u64, Vec<(u64, u32)>)> = None;
    if let Some(f) = versions.as_mut() {
        let (mut cles, mut panneaux) = (Vec::new(), Vec::new());
        for p in &mut f.plans {
            for v in &mut p.autres {
                match serde_json::from_value::<PanelEntry>(std::mem::take(&mut v.entree)) {
                    Ok(e) => {
                        cles.push((p.plan, v.numero));
                        panneaux.push(e);
                    }
                    Err(e) => warnings.push(format!("version {} d'un plan : illisible ({e}), ignorée.", crate::planvers::etiquette(v.numero))),
                }
            }
        }
        if !panneaux.is_empty() {
            let top = top_id(&manifest);
            manifest.sequences.push(SequenceEntry { id: top + 1, nom: String::new(), numero: String::new(), scenes: vec![SceneEntry { id: top + 2, nom: String::new(), numero: String::new(), panneaux }] });
            a_ranger = Some((top + 1, top + 2, cles));
        }
    }
    let mut cels: HashMap<CelId, Cel> = HashMap::new();
    let mut missing: BTreeSet<CelId> = BTreeSet::new();
    // Le contenu rogné des plans (`rognages.json`) : ses dessins se chargent avec les autres.
    let reserves = load_reserves(dir, &mut warnings);
    let mut referenced: BTreeSet<CelId> = manifest
        .sequences
        .iter()
        .flat_map(|s| &s.scenes)
        .flat_map(|s| &s.panneaux)
        .flat_map(|p| &p.calques)
        .flat_map(|l| l.expositions.iter().filter_map(|r| r.dessin))
        .collect();
    if let Some(r) = &reserves {
        referenced.extend(r.cels());
    }
    let mut approx = Vec::new();
    // Les dessins se lisent en parallèle (décodage PNG ou rendu des traits : 25 ms chacun, un par
    // cœur) ; les résultats reviennent dans l'ordre des identifiants, comme en une seule boucle.
    let referenced: Vec<CelId> = referenced.into_iter().collect();
    if let Some(p) = progress {
        p.total.store(referenced.len(), std::sync::atomic::Ordering::Relaxed);
    }
    let hors: BTreeSet<CelId> = manifest.dessins_hors.iter().copied().collect();
    let loaded = crate::parallele::par_map(&referenced, LOAD_THREADS, |_, &id| {
        let r = load_drawing(dir, id, w, h, quick, hors.contains(&id));
        if let Some(p) = progress {
            p.done.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        r
    });
    for (id, result) in referenced.into_iter().zip(loaded) {
        match result {
            Ok((cel, approximate, warning)) => {
                if approximate {
                    approx.push(id);
                }
                warnings.extend(warning);
                cels.insert(id, cel);
            }
            Err(e) => {
                warnings.push(e);
                missing.insert(id);
            }
        }
    }

    let mut sounds: HashMap<u64, Sound> = HashMap::new();
    // Fichiers de sons et de modèles illisibles : l'enregistrement ne les efface pas.
    let mut unreadable_files: BTreeSet<PathBuf> = BTreeSet::new();
    for entry in &manifest.sons {
        match load_sound(dir, entry) {
            Ok(sound) => {
                sounds.insert(sound.id, sound);
            }
            Err(e) => {
                if Path::new(&entry.fichier).components().count() == 1 {
                    unreadable_files.insert(dir.join(SOUNDS).join(&entry.fichier));
                }
                warnings.push(e);
            }
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
            Err(e) => {
                unreadable_files.insert(model_file(dir, entry.id));
                warnings.push(format!("modèle 3D « {} » illisible ({e})", entry.nom));
            }
        }
    }
    let audio_tracks = manifest
        .pistes_son
        .iter()
        .map(|t| AudioTrack {
            id: t.id,
            name: t.nom.clone(),
            muted: t.muette,
            // (Le solo vient de `effets_son.json`.)
            solo: false,
            volume: t.volume.clamp(0.0, 4.0),
            clips: t
                .extraits
                .iter()
                .filter(|c| sounds.contains_key(&c.son) && c.duree > 0)
                .map(|c| SoundClip::new(c.id, c.son, c.debut, c.decalage, c.duree))
                .collect(),
            locked: t.verrouillee,
        })
        .collect();

    let renumbered = unique_ids(&mut manifest, cels.keys().chain(sounds.keys()).copied().max().unwrap_or(0));
    if renumbered > 0 {
        warnings.push(format!("{renumbered} identifiant(s) de plan, de scène ou de séquence en double ont été renumérotés : aucun plan n'est perdu."));
    }
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
        numbering_locked: manifest.numerotation_verrouillee,
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
        // Les scènes 3D viennent de leur fichier (`scenes3d.json`, `load_scenes3d`).
        scenes3d: Default::default(),
        // Les commentaires viennent de leur fichier, après les plans (`load_comments`).
        comments: Vec::new(),
        // La réserve du rognage aussi (`rognages.json`, `load_reserves`).
        reserves: Default::default(),
        // Les transitions : `transitions.json`, `load_transitions`.
        transitions: Vec::new(),
        // Les références visuelles aussi (`references.json`, `references::load`).
        references: Vec::new(),
        // Les versions des plans : `plans_versions.json`, `planvers::installer`.
        versions: Default::default(),
    };
    for seq in manifest.sequences {
        structure.sequences.push(Sequence {
            id: seq.id,
            name: seq.nom,
            number: seq.numero,
        });
        for scene in seq.scenes {
            structure.scenes.push(Scene {
                id: scene.id,
                name: scene.nom,
                number: scene.numero,
                sequence: seq.id,
            });
            // Numéros des calques de toute la scène : neufs pour la session.
            let mut keys: HashMap<u64, u64> = HashMap::new();
            for p in scene.panneaux {
                let parents: Vec<Option<usize>> = p.calques.iter().map(|l| l.dans_dossier).collect();
                let ctg_sources: Vec<Option<(Vec<usize>, u8)>> = p.calques.iter().map(|l| l.ctg.as_ref().map(|c| (c.sources.clone(), c.trou))).collect();
                let couleurs: Vec<Option<CouleurEntry>> = p.calques.iter().map(|l| l.calque_couleur.clone()).collect();
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
                        fx_enabled: !l.effets_eteints,
                        ctg: None,
                        // Calque vidéo (format 21) : ses images dans `videos/<id>/`.
                        video: l.video.filter(|v| v.images > 0 && v.ips > 0.0 && matches!(v.ext.as_str(), "jpg" | "png")).map(|v| {
                            crate::video::register_dir(v.id, dir.join(VIDEOS).join(v.id.to_string()));
                            crate::video::VideoClip { id: v.id, name: v.nom, frames: v.images, fps: v.ips, start: v.debut, ext: v.ext }
                        }),
                        // Fond sous le trait (format 23).
                        fond: l.fond,
                        // Calque d'effet (format 27).
                        effet: l.calque_effet.map(crate::calqueffet::Entree::into_model),
                        // Calque de couleur (format 30) : le lien vient après (rang du trait → son id).
                        couleur: None,
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
                // Calques de couleur (format 30) : le rang du calque de trait devient son id, les
                // couples de dessins sont gardés si leurs deux dessins sont là.
                for (i, entry) in couleurs.into_iter().enumerate() {
                    let Some(e) = entry else { continue };
                    let Some(line) = layers.get(e.ligne).filter(|_| e.ligne != i).map(|l| l.id) else { continue };
                    let mut c = crate::calquecouleur::CalqueCouleur::new(line);
                    c.dessins = e.dessins.into_iter().filter(|[t, k]| cels.contains_key(t) && cels.contains_key(k)).map(|[t, k]| (t, k)).collect();
                    c.trous = e.trous.min(50);
                    c.zone = e.zone;
                    layers[i].couleur = Some(c);
                }
                crate::calquecouleur::reparer(&mut layers);
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
                    length: p.duree.min(crate::rognage::MAX_PLAN),
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
                    color: p.couleur.filter(|k| (*k as usize) < crate::marks::COLORS.len()),
                });
            }
        }
    }
    // Les plans rangés des versions ne sont pas des plans du film : mis de côté pour `planvers::installer`.
    let mut rangees: Vec<Panel> = Vec::new();
    if let Some((sequence, scene, _)) = &a_ranger {
        let (gardes, extra): (Vec<Panel>, Vec<Panel>) = std::mem::take(&mut structure.panels).into_iter().partition(|p| p.scene != *scene);
        structure.panels = gardes;
        rangees = extra;
        structure.scenes.retain(|s| s.id != *scene);
        structure.sequences.retain(|s| s.id != *sequence);
    }
    if structure.panels.is_empty() {
        return Err("le projet ne contient aucun plan".into());
    }
    // Les scènes 3D, dans leur fichier à côté du manifeste (absent : aucune scène).
    structure.scenes3d = load_scenes3d(dir, &mut warnings);
    // Les commentaires, dans leur fichier à côté du manifeste (absent : aucun commentaire).
    structure.comments = load_comments(dir, &mut warnings);
    // Les transitions du montage, de même (`transitions.json`).
    structure.transitions = load_transitions(dir, &mut warnings);
    // Les références visuelles : `references.json` et le dossier `references/` (absents : aucune).
    structure.references = crate::references::load(dir, &mut warnings);
    let frame = manifest.cadre;
    let mut written: HashMap<CelId, u64> = cels.keys().map(|&id| (id, 0)).collect();
    let mut doc = Document::from_parts(w, h, structure, cels, sounds);
    // Les effets des sons (fondus, volume, égaliseur, écho, solo : `effets_son.json`).
    if let Some(fx) = load_sound_fx(dir, &mut warnings) {
        fx.install(&mut doc);
    }
    // Les sons proposés (`sons_proposes.json`).
    doc.sons_proposes = load_sons_proposes(dir, &mut warnings);
    for model in models.into_values() {
        doc.insert_model(model);
    }
    if let Some(r) = reserves {
        r.install(&mut doc, &missing);
    }
    // La planche de crayonnés rapides, dans son fichier (absent : aucune).
    doc.crayonnes = load_crayonnes(dir, &mut warnings);
    // Les envois (`envois.json`, absent : aucun) ; leurs instantanés ne se lisent qu'à la demande.
    doc.envois = crate::envois::load(dir, &mut warnings);
    // Les autres versions des plans.
    if let (Some(f), Some((_, _, cles))) = (versions, a_ranger) {
        crate::planvers::installer(&mut doc, f, cles, rangees, &mut warnings);
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
    keep_files(dir, unreadable_files);
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

/// Fichiers de sons et de modèles 3D illisibles à l'ouverture, par dossier de projet.
static KEPT_FILES: std::sync::Mutex<BTreeMap<PathBuf, BTreeSet<PathBuf>>> = std::sync::Mutex::new(BTreeMap::new());

fn keep_files(dir: &Path, files: BTreeSet<PathBuf>) {
    let mut m = KEPT_FILES.lock().unwrap_or_else(|e| e.into_inner());
    if files.is_empty() {
        m.remove(dir);
    } else {
        m.insert(dir.to_path_buf(), files);
    }
}

fn kept_files_in(dir: &Path) -> BTreeSet<PathBuf> {
    KEPT_FILES.lock().unwrap_or_else(|e| e.into_inner()).get(dir).cloned().unwrap_or_default()
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
fn load_drawing(dir: &Path, id: CelId, w: usize, h: usize, quick: bool, hors: bool) -> Result<(Cel, bool, Option<String>), String> {
    let (mut cel, approximate) = load_line(dir, id, w, h, quick)?;
    // Ce que le dessin a hors de la feuille (toile infinie) : un fichier illisible n'empêche pas d'ouvrir
    // le dessin, il perd cette part et le dit.
    let warning = if hors && cel.vector.is_none() { read_hors(dir, id, &mut cel).err() } else { None };
    let read = |path: PathBuf| -> Option<Raster> {
        // (Pas même un `exists` d'avance : presque aucun dessin n'a de strate Fond.)
        read_rgba(&path, |pw, ph, data| (pw == w && ph == h).then(|| Raster::from_rgba(w, h, data))).ok().flatten()
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
    Ok((cel, approximate, warning))
}

/// Coordonnée la plus lointaine qu'un élément vectoriel garde (une feuille fait 16 384 pixels au
/// plus ; un trait traîné bien hors du dessin reste loin en deçà), et rayon le plus gros.
const MAX_COORD: f32 = 1.0e5;
const MAX_RADIUS: f32 = 5_000.0;

/// Le rang du premier élément du dessin dont les valeurs sont absurdes (non finies, ou hors de ce
/// qu'un geste peut produire), s'il y en a un. Un fichier abîmé qui annonce un trait de 3,4e38 pixels
/// de long bloquait la lecture à le rejouer ; mieux vaut dire que le dessin est illisible (son fichier
/// reste tel quel, comme tout dessin illisible).
fn implausible(d: &VectorDrawing) -> Option<usize> {
    use crate::vector::Element;
    let coord = |v: f32| v.is_finite() && v.abs() <= MAX_COORD;
    d.elements.iter().position(|e| match e {
        Element::Stroke { points, opacity, hardness, .. } => {
            !points.iter().all(|p| coord(p.x) && coord(p.y) && p.r.is_finite() && (0.0..=MAX_RADIUS).contains(&p.r)) || !opacity.is_finite() || !hardness.is_finite()
        }
        Element::Fill { contours, opacity, .. } => !contours.iter().flatten().all(|p| coord(p[0]) && coord(p[1])) || !opacity.is_finite(),
        Element::Text { size, place, opacity, .. } => !(size.is_finite() && *size > 0.0 && *size <= MAX_RADIUS) || !place.iter().all(|v| v.is_finite() && v.abs() <= MAX_COORD * 10.0) || !opacity.is_finite(),
    })
}

fn load_line(dir: &Path, id: CelId, w: usize, h: usize, quick: bool) -> Result<(Cel, bool), String> {
    let vector_path = drawing_path(dir, id, true);
    if vector_path.exists() {
        let text = fs::read_to_string(&vector_path).map_err(|e| format!("dessin {id} illisible : {e}"))?;
        let drawing: VectorDrawing =
            serde_json::from_str(&text).map_err(|e| format!("dessin {id} illisible : {e}"))?;
        if let Some(k) = implausible(&drawing) {
            return Err(format!("dessin {id} illisible : valeurs hors limites dans l'élément {} ; son fichier n'est pas modifié", k + 1));
        }
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
        let raster = read_rgba(&png_path, |pw, ph, data| (pw == w && ph == h).then(|| Raster::from_rgba(w, h, data)))
            .map_err(|e| format!("dessin {id} illisible : {e}"))?;
        return match raster {
            Some(r) => Ok((Cel::bitmap(r), false)),
            None => Err(format!("dessin {id} : taille différente de celle du projet, ignoré")),
        };
    }
    Err(format!("dessin {id} introuvable dans « {DRAWINGS} »"))
}

thread_local! {
    /// Tampon de décodage, gardé par fil : une feuille décodée pèse 33 Mo, et en allouer (et
    /// remplir de zéros) une à chaque dessin coûtait autant que le décodage lui-même quand
    /// plusieurs fils lisent à la fois.
    static DECODE_BUF: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Lit une image PNG et la donne à `use_it` en octets RGBA (largeur, hauteur, octets). Un PNG RGBA 8
/// bits, comme ceux de SHOT, est décodé dans le tampon du fil ; tout autre PNG passe par
/// `image::open` comme avant.
fn read_rgba<R>(path: &Path, use_it: impl FnOnce(usize, usize, &[u8]) -> R) -> Result<R, String> {
    use image::ImageDecoder;
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    if let Ok(decoder) = image::codecs::png::PngDecoder::new(std::io::Cursor::new(&bytes)) {
        if decoder.color_type() == image::ColorType::Rgba8 {
            let (w, h) = decoder.dimensions();
            // L'en-tête d'un fichier abîmé peut annoncer des dimensions démesurées : le tampon
            // demandé (des dizaines de Go) ferait tomber SHOT sans un mot (échec d'allocation).
            // Une feuille ne dépasse pas `MAX_SIDE` de côté (8 192).
            if w as usize > crate::vector::MAX_SIDE * 2 || h as usize > crate::vector::MAX_SIDE * 2 || decoder.total_bytes() > (1 << 30) {
                return Err(format!("image démesurée ({w} × {h})"));
            }
            let len = decoder.total_bytes() as usize;
            return DECODE_BUF.with(|buf| {
                let mut buf = buf.borrow_mut();
                buf.resize(len, 0);
                decoder.read_image(&mut buf[..len]).map_err(|e| e.to_string())?;
                let result = use_it(w as usize, h as usize, &buf[..len]);
                // Une feuille démesurée (image importée) ne reste pas en mémoire.
                if buf.capacity() > 64 << 20 {
                    *buf = Vec::new();
                }
                Ok(result)
            });
        }
    }
    let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?.into_rgba8();
    Ok(use_it(img.width() as usize, img.height() as usize, img.as_raw()))
}
