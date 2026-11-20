//! Le décor 3D d'une séquence (parties 180 et 181, `docs/INTERFACE.md`) : des **objets** glTF posés pour une **séquence**, que
//! tous ses plans montrent, filmés par la caméra de chaque plan, dans la vue Espace 3D et dans l'image du plan (écran et
//! exports).
//!
//! Première étape du moteur de prévisualisation qu'Anthony a choisi le 2026-11-19 (`docs/ESPACE3D.md`) :
//! - **L'élément le plus bas** de l'image du plan (`render::plan_with_camera_layers`, et le dessin à l'écran,
//!   `App::build_plan_avec`), juste au-dessus du papier ; chaque calque se pose dessus. Juste en profondeur en lui-même (tampon
//!   de profondeur partagé par tous ses objets) et, depuis la partie 183, **avec les cartes** (`profondeur.rs`) : son rendu garde
//!   sa profondeur (`calculer`, `profondeur_du_rendu`, tirée du tampon de `decor3dgpu`, quatre échantillons par pixel depuis la
//!   partie 193), et une carte est cachée là où le décor est plus près qu'elle (en partie au bord, partie 193).
//! - **Vu de l'œil de la caméra, aligné sur la feuille** (`decor3drendu::Prise::de_la_feuille`), comme une carte en
//!   profondeur : l'homographie de la caméra 2D (panoramique, bascule, zoom, rotation) s'applique ensuite à tout le plan, le décor
//!   défile avec la parallaxe des cartes. Un plan « en Espace 3D » (`Document::repere_du_plan`) pose le décor comme ses cartes :
//!   passé dans le repère de la caméra, vu de l'œil de référence.
//! - À l'écran avec une carte graphique, c'est elle qui dessine (`decor3dgpu.rs`, passe de `GpuCanvas`) ; sans carte, le
//!   processeur sur un autre fil (`montre`), qui ne fait jamais attendre l'interface ; à l'export, l'image exacte, tout de suite
//!   (la carte graphique si FEANOR en a une, `poser_carte`, sinon le processeur).
//!
//! **Plusieurs objets** (partie 181) : un décor de séquence est une liste d'objets (`Objet3d`), chacun un modèle et sa place :
//! position de son **origine** (un point du modèle, d'origine le centre du dessous de sa boîte : « posé au sol » = l'origine
//! sur le sol), rotation autour de la verticale et échelle autour d'elle, et « aimanté au sol » (`decor3daimant.rs`). Importer
//! un fichier, ou prendre un modèle de la banque 3D (`banque3d.rs`), **ajoute** un objet. Dix arbres pareils sont un seul
//! modèle : un seul fichier dans le projet (nommé par l'empreinte de son contenu), une seule lecture à l'ouverture, un seul
//! envoi à la carte graphique (les objets partagent le même `Arc<Modele3d>`).
//!
//! **Dans le projet, sans changer `VERSION`** : `decors3d.json` à côté du manifeste (version 2 : par séquence, l'aspect et les
//! objets — nom, fichier d'origine, place, origine, aimanté, ce qui a été allégé), et les modèles dans
//! `decors/<empreinte>.glb` (toujours un `.glb` autonome, `decor3dgltf::empaqueter`), jamais modifiés sur place. Un fichier de
//! la partie 180 (version 1, un décor par séquence, `decors/<id>.glb`) se lit comme une liste d'un objet, à la même place au
//! millimètre près ; il est réécrit en version 2 au prochain enregistrement. Une version de FEANOR qui ne connaît pas la
//! version 2 la laisse là, n'efface rien, et montre les plans sans décor.
//!
//! **Annuler** : ajouter, enlever, dupliquer, déplacer un objet sont des étapes d'annulation (`Structure::decors3d`) ; les
//! modèles sont partagés (`Arc`), une étape ne les recopie pas.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::decor3dgltf::{Mesures, Scene3d};
use crate::decor3drendu::{Aspect, Prise};
use crate::document::{Cel, CelId, Document};
use crate::espace3d::Monde;
use crate::espace3dmath::{self as m, M4, V3};
use crate::raster::Raster;

/// Le fichier des décors, à côté du manifeste.
pub const FILE: &str = "decors3d.json";
/// Le dossier des modèles (`decors/<empreinte>.glb`).
pub const DIR: &str = "decors";
/// La version de `decors3d.json` (1 : partie 180, un décor par séquence ; 2 : partie 181, des objets).
pub const VERSION_FICHIER: u32 = 2;

/// Les **plafonds de sûreté** : contre les plantages et le manque de mémoire, pas contre la lenteur (le budget de lenteur est
/// celui de la machine, `decor3dbudget.rs`). Un fichier glTF est une donnée étrangère : tout ce qu'il annonce est vérifié contre
/// ces plafonds et contre la longueur réelle de ses tampons **avant** de réserver la mémoire.
pub struct Plafonds {
    /// Taille du fichier, refusé avant d'être lu.
    pub fichier: u64,
    pub sommets: usize,
    pub indices: usize,
    pub triangles: usize,
    /// Côté d'une texture, lu dans l'en-tête de l'image avant de la décoder.
    pub cote_texture: u32,
    /// Textures décodées, en tout.
    pub octets_textures: u64,
}

pub const PLAFONDS: Plafonds = Plafonds { fichier: 1 << 30, sommets: 16_000_000, indices: 48_000_000, triangles: 16_000_000, cote_texture: 16384, octets_textures: 2 << 30 };

/// Où un objet est posé dans l'espace : la position (mètres) de son origine, la rotation autour de la verticale (degrés) et
/// l'échelle, autour de l'origine.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Place3d {
    pub position: V3,
    #[serde(default)]
    pub rotation_y: f32,
    #[serde(default = "un")]
    pub echelle: f32,
}

fn un() -> f32 {
    1.0
}

impl Default for Place3d {
    fn default() -> Place3d {
        Place3d { position: [0.0; 3], rotation_y: 0.0, echelle: 1.0 }
    }
}

impl Place3d {
    /// Le point (0, 0, 0) du modèle à `position`, tourné puis mis à l'échelle autour de lui.
    pub fn matrice(&self) -> M4 {
        m::trs(self.position, [0.0, self.rotation_y, 0.0], [self.echelle_sure(); 3])
    }

    /// L'échelle, jamais nulle ni négative.
    pub fn echelle_sure(&self) -> f32 {
        if self.echelle.is_finite() && self.echelle > 0.0 {
            self.echelle
        } else {
            1.0
        }
    }

    /// La place d'origine d'un décor importé par la partie 180 (le point (0, 0, 0) du modèle comme origine) : le bas de sa
    /// boîte posé sur le sol (`Monde::sol_y`), centré en x, tout entier derrière la feuille (z ≤ 0). Les mètres du glTF sont
    /// gardés tels quels.
    pub fn d_origine(scene: &Scene3d, monde: &Monde) -> Place3d {
        let (lo, hi) = scene.bornes();
        Place3d { position: [-(lo[0] + hi[0]) / 2.0, monde.sol_y() - lo[1], -hi[2]], rotation_y: 0.0, echelle: 1.0 }
    }
}

/// Un modèle chargé : ce que FEANOR montre, et les octets du `.glb` gardé dans le projet.
pub struct Modele3d {
    /// Un numéro unique pour toute la séance (pas celui du décor, que deux projets ouverts peuvent partager) : la carte
    /// graphique et les rendus gardés reconnaissent le modèle par lui.
    pub uid: u64,
    pub scene: Arc<Scene3d>,
    pub octets: Arc<Vec<u8>>,
    /// L'empreinte du contenu (`empreinte`) : le nom de son fichier dans le projet (`decors/<empreinte>.glb`). Deux objets du
    /// même contenu partagent le fichier.
    pub empreinte: String,
    /// La boîte de la scène, calculée une fois (l'aimant la lit à chaque retouche).
    pub bornes: (V3, V3),
}

/// L'empreinte d'un modèle : les 16 premiers chiffres hexadécimaux du SHA-256 de ses octets (stable d'une machine et d'une
/// version de FEANOR à l'autre, contrairement au hachage de Rust).
pub fn empreinte(octets: &[u8]) -> String {
    crate::sha256::de_octets(octets)[..16].to_string()
}

impl Modele3d {
    /// (Calcule l'empreinte : sur le fil de l'interface, seulement pour de petits modèles ; sinon `avec_empreinte`.)
    pub fn new(scene: Arc<Scene3d>, octets: Arc<Vec<u8>>) -> Modele3d {
        let e = empreinte(&octets);
        Modele3d::avec_empreinte(scene, octets, e)
    }

    pub fn avec_empreinte(scene: Arc<Scene3d>, octets: Arc<Vec<u8>>, empreinte: String) -> Modele3d {
        static SUIVANT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let bornes = scene.bornes();
        Modele3d { uid: SUIVANT.fetch_add(1, std::sync::atomic::Ordering::Relaxed), scene, octets, empreinte, bornes }
    }

    /// Le fichier du modèle dans le projet.
    pub fn fichier(&self) -> String {
        format!("{DIR}/{}.glb", self.empreinte)
    }
}

/// Le centre de la face du dessous de la boîte d'une scène : l'origine d'un objet neuf.
pub fn origine_au_sol(bornes: (V3, V3)) -> V3 {
    let (lo, hi) = bornes;
    [(lo[0] + hi[0]) / 2.0, lo[1], (lo[2] + hi[2]) / 2.0]
}

/// Un objet du décor d'une séquence.
#[derive(Clone)]
pub struct Objet3d {
    /// Son numéro dans le projet (`Document::alloc`).
    pub id: u64,
    pub nom: String,
    /// Le nom du fichier d'origine (jamais modifié), ou « Banque 3D : … ».
    pub source: String,
    /// Ce qui a été allégé ou simplifié à l'import (« textures 4096 → 2048 », « simplifié : 6,2 M → 1,9 M triangles »).
    pub allege: Vec<String>,
    /// Où est son origine, tourné et mis à l'échelle autour d'elle.
    pub place: Place3d,
    /// L'origine : un point dans le repère du modèle.
    pub origine: V3,
    /// Aimanté au sol : sa hauteur est calculée (`decor3daimant.rs`), posée sur le sol ou sur les objets sous lui.
    pub aimante: bool,
    pub modele: Arc<Modele3d>,
    /// Partie 199 : sa couleur d'identification (le rang dans `couleurs3d::PALETTE`) ; rien : aucune (d'origine).
    pub couleur: Option<u8>,
    /// Partie 198, enregistrés depuis la partie 199 : caché par son œil dans « Scène 3D » (ni la vue ni les images des plans ne le
    /// dessinent), verrouillé par son cadenas (il ne se choisit plus dans la vue).
    pub cache: bool,
    pub verrou: bool,
    /// Partie 200 : ses clés d'animation, triées par image de la séquence (`miseenscene3d.rs`) ; aucune : immobile à `place`. Avec
    /// des clés, `place` est la pose à l'image courante (`App::decor3d_suivre_l_image`), les images des plans lisent les clés.
    pub cles: Vec<crate::miseenscene3d::CleObjet>,
    /// Partie 200 : les plans (leurs numéros, `Panel::id`) dont l'image ne le montre pas, « Caché dans le plan » : il reste dans le
    /// décor de la séquence et dans les vues de l'onglet 3D (sauf « Caméra du plan » de ce plan).
    pub caches_dans: Vec<u64>,
}

impl std::fmt::Debug for Objet3d {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Objet3d({} « {} », {:?}, origine {:?})", self.id, self.nom, self.place, self.origine)
    }
}

impl Objet3d {
    /// Un objet neuf : l'origine au centre du dessous de sa boîte, aimanté au sol.
    pub fn neuf(id: u64, nom: String, source: String, allege: Vec<String>, modele: Arc<Modele3d>) -> Objet3d {
        let origine = origine_au_sol(modele.bornes);
        Objet3d { id, nom, source, allege, place: Place3d::default(), origine, aimante: true, modele, couleur: None, cache: false, verrou: false, cles: Vec::new(), caches_dans: Vec::new() }
    }

    /// Partie 200 : animé par des clés.
    pub fn anime(&self) -> bool {
        !self.cles.is_empty()
    }

    /// Partie 200 : sa place à l'image `image` de la séquence : celle de ses clés, adoucie entre deux (`miseenscene3d::place_a`) ;
    /// sans clé, `place`.
    pub fn place_a(&self, image: usize) -> Place3d {
        crate::miseenscene3d::place_a(&self.cles, image).unwrap_or(self.place)
    }

    /// `matrice`, à l'image `image` de la séquence.
    pub fn matrice_a(&self, image: usize) -> M4 {
        m::mul(&self.place_a(image).matrice(), &m::translation(m::scale(self.origine, -1.0)))
    }

    /// Partie 200 : caché dans l'image du plan `plan` (`Panel::id`).
    pub fn cache_dans(&self, plan: u64) -> bool {
        self.caches_dans.contains(&plan)
    }

    /// Du repère du modèle au monde : l'origine à `place.position`, la rotation et l'échelle autour d'elle.
    pub fn matrice(&self) -> M4 {
        m::mul(&self.place.matrice(), &m::translation(m::scale(self.origine, -1.0)))
    }

    /// Le point `p` du modèle dans le monde.
    pub fn au_monde(&self, p: V3) -> V3 {
        m::point(&self.matrice(), p)
    }

    /// Change l'origine **sans bouger l'objet** : sa position devient celle du nouveau point dans le monde (comme « origine
    /// vers la géométrie » d'un outil 3D).
    pub fn changer_origine(&mut self, o: V3) {
        if !o.iter().all(|v| v.is_finite()) {
            return;
        }
        self.place.position = self.au_monde(o);
        // (Partie 200 : et à chacune de ses clés, l'objet ne bouge à aucune image.)
        let ancienne = self.origine;
        for k in &mut self.cles {
            let mx = m::mul(&k.place.matrice(), &m::translation(m::scale(ancienne, -1.0)));
            k.place.position = m::point(&mx, o);
        }
        self.origine = o;
    }

    /// La boîte de l'objet dans le monde (ses huit coins placés).
    pub fn bornes_monde(&self) -> (V3, V3) {
        let (a, b) = self.modele.bornes;
        let mx = self.matrice();
        let coins = (0..8).map(|i| m::point(&mx, [0, 1, 2].map(|k| if i >> k & 1 == 0 { a[k] } else { b[k] })));
        m::bounds_of(coins).unwrap_or(([0.0; 3], [0.0; 3]))
    }

    /// Posé sur le sol, son origine en `x`, `z` (la hauteur suit l'origine : le dessous de sa boîte sur le sol).
    pub fn poser_au_sol(&mut self, x: f32, z: f32, sol_y: f32) {
        let e = self.place.echelle_sure();
        let (lo, _) = self.modele.bornes;
        self.place.position = [x, sol_y + e * (self.origine[1] - lo[1]), z];
    }
}

/// Le décor d'une séquence : ses objets et leur aspect commun (une seule lumière, un seul ciel).
#[derive(Clone)]
pub struct Decor3d {
    /// Un numéro unique pour la séance (gardé par les copies de l'annulation) : la texture de la carte graphique qui montre ce
    /// décor (`id_texture`), redessinée quand un objet bouge, jamais recréée.
    pub uid: u64,
    pub aspect: Aspect,
    pub objets: Vec<Objet3d>,
    /// Partie 200 : ses lampes (`miseenscene3d::Lampe3d`, au plus `MAX_LAMPES`), en plus du soleil et du ciel.
    pub lampes: Vec<crate::miseenscene3d::Lampe3d>,
    /// Partie 202 : son cyclo (`cyclos::Cyclo`), le ciel et l'horizon autour du décor, et leur lumière.
    pub cyclo: Option<crate::cyclos::Cyclo>,
}

impl std::fmt::Debug for Decor3d {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Decor3d({}, {:?})", self.uid, self.objets)
    }
}

impl Decor3d {
    pub fn new(aspect: Aspect) -> Decor3d {
        static SUIVANT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Decor3d { uid: SUIVANT.fetch_add(1, std::sync::atomic::Ordering::Relaxed), aspect, objets: Vec::new(), lampes: Vec::new(), cyclo: None }
    }

    /// Partie 202 : sans objet, sans lampe, sans cyclo (un décor vide ne s'enregistre pas, ne se montre pas). Des lampes ou un cyclo
    /// suffisent à le garder : une séquence sans objet 3D a son ciel et sa lumière.
    pub fn est_vide(&self) -> bool {
        self.objets.is_empty() && self.lampes.is_empty() && self.cyclo.is_none()
    }

    /// Partie 202 : il se montre (`Document::decor_du_plan`) : des objets, ou un cyclo. Des lampes seules se gardent mais ne font
    /// pas un décor (comme avant la partie 202).
    pub fn se_montre(&self) -> bool {
        !self.objets.is_empty() || self.cyclo.is_some()
    }

    /// Partie 200 : les objets que montre l'image du plan `plan` (`Panel::id`) : ceux que cet ordinateur montre, sans ceux « cachés
    /// dans le plan ».
    pub fn vus_dans(&self, plan: u64) -> impl Iterator<Item = &Objet3d> + '_ {
        self.vus().filter(move |o| !o.cache_dans(plan))
    }

    /// Partie 200 : un de ses objets est animé.
    pub fn anime(&self) -> bool {
        self.objets.iter().any(Objet3d::anime)
    }

    pub fn lampe(&self, id: u64) -> Option<&crate::miseenscene3d::Lampe3d> {
        self.lampes.iter().find(|l| l.id == id)
    }

    pub fn lampe_mut(&mut self, id: u64) -> Option<&mut crate::miseenscene3d::Lampe3d> {
        self.lampes.iter_mut().find(|l| l.id == id)
    }

    /// Les objets que cet ordinateur montre (pas ceux masqués en attendant le choix d'Anthony, `masquer`).
    pub fn vus(&self) -> impl Iterator<Item = &Objet3d> + '_ {
        // (Partie 198 : ni ceux cachés par leur œil dans « Scène 3D ».)
        self.objets.iter().filter(|o| !masque(o.modele.uid) && !o.cache)
    }

    pub fn objet(&self, id: u64) -> Option<&Objet3d> {
        self.objets.iter().find(|o| o.id == id)
    }

    pub fn objet_mut(&mut self, id: u64) -> Option<&mut Objet3d> {
        self.objets.iter_mut().find(|o| o.id == id)
    }

    /// Le poids de ce que l'écran dessine : triangles (chaque objet compte, même s'il partage son modèle) et octets de
    /// textures (un modèle partagé ne compte qu'une fois : la carte le garde une fois).
    pub fn poids(&self) -> Poids {
        let mut p = Poids::default();
        let mut vus = BTreeSet::new();
        for o in self.vus() {
            p.triangles += o.modele.scene.triangles() as u64;
            if vus.insert(o.modele.uid) {
                p.octets_textures += o.modele.scene.octets_textures();
            }
        }
        p
    }

    /// Le modèle de même empreinte déjà dans ce décor (un objet neuf le partage).
    pub fn modele_d_empreinte(&self, e: &str) -> Option<Arc<Modele3d>> {
        self.objets.iter().find(|o| o.modele.empreinte == e).map(|o| o.modele.clone())
    }
}

/// Le poids d'un décor de séquence, comparé au budget de la machine (`verdict_sequence`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Poids {
    pub triangles: u64,
    pub octets_textures: u64,
}

/// Les décors du projet, par numéro de séquence. Dans `Structure` : l'annulation les rétablit avec le reste.
#[derive(Clone, Default, Debug)]
pub struct Decors3d {
    pub par_sequence: BTreeMap<u64, Decor3d>,
    /// Les entrées de `decors3d.json` qui n'ont pas pu être lues (fichier manquant ou abîmé ; un objet à la fois) :
    /// réécrites telles quelles, leurs fichiers jamais effacés.
    gardees: Vec<serde_json::Value>,
    /// `decors3d.json` était illisible (abîmé, d'une version plus récente) : il n'est pas remplacé, rien n'est effacé de
    /// `decors/`.
    illisible: bool,
}

impl Decors3d {
    pub fn is_empty(&self) -> bool {
        self.par_sequence.values().all(Decor3d::est_vide) && self.gardees.is_empty()
    }

    /// Le modèle de même empreinte déjà dans le projet (un objet neuf le partage : une lecture, un envoi à la carte).
    pub fn modele_d_empreinte(&self, e: &str) -> Option<Arc<Modele3d>> {
        self.par_sequence.values().find_map(|d| d.modele_d_empreinte(e))
    }

    /// L'objet `id`, et sa séquence.
    pub fn objet(&self, id: u64) -> Option<(u64, &Objet3d)> {
        self.par_sequence.iter().find_map(|(s, d)| d.objet(id).map(|o| (*s, o)))
    }
}

impl Document {
    /// Le décor 3D de la séquence du plan `index` (rien : la séquence n'a aucun objet).
    pub fn decor_du_plan(&self, index: usize) -> Option<&Decor3d> {
        (index < self.panels.len()).then(|| self.decors3d.par_sequence.get(&self.sequence_of(index))).flatten().filter(|d| d.se_montre())
    }
}

// ------------------------------------------------------------------------------------------------ le fichier

#[derive(Serialize, Deserialize)]
struct Fichier {
    version: u32,
    decors: Vec<serde_json::Value>,
}

/// Une séquence (version 2).
#[derive(Serialize, Deserialize)]
struct EntreeSequence {
    sequence: u64,
    #[serde(default)]
    aspect: Aspect,
    objets: Vec<serde_json::Value>,
    /// Partie 200 : les lampes de la séquence. Absentes d'un fichier d'avant : aucune ; un FEANOR d'avant ignore la clé.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    lampes: Vec<crate::miseenscene3d::Lampe3d>,
    /// Partie 202 : le cyclo de la séquence. Absent : aucun ; un FEANOR d'avant ignore la clé.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cyclo: Option<crate::cyclos::Cyclo>,
}

/// Un objet (version 2).
#[derive(Serialize, Deserialize)]
struct EntreeObjet {
    id: u64,
    nom: String,
    #[serde(default)]
    fichier_d_origine: String,
    fichier: String,
    #[serde(flatten)]
    place: Place3d,
    /// (Absente : le centre du dessous de la boîte, la position corrigée pour que l'objet ne bouge pas.)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    origine: Option<V3>,
    #[serde(default)]
    aimante: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    allege: Vec<String>,
    /// Partie 199 : la couleur d'identification (rang dans la palette) ; l'œil et le cadenas de « Scène 3D ». Absents d'un fichier
    /// d'avant : aucune, montré, libre ; un FEANOR d'avant les ignore (rien n'interdit les clés inconnues ici).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    couleur: Option<u8>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    cache: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    verrou: bool,
    /// Partie 200 : ses clés d'animation (image de la séquence et place), et les plans dont l'image ne le montre pas. Absents d'un
    /// fichier d'avant : immobile, dans tous les plans ; un FEANOR d'avant les ignore.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    cles: Vec<crate::miseenscene3d::CleObjet>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    caches_dans: Vec<u64>,
}

/// Un décor de la partie 180 (version 1) : un seul modèle par séquence, sa place celle du point (0, 0, 0) du modèle.
#[derive(Deserialize)]
struct EntreeV1 {
    sequence: u64,
    id: u64,
    nom: String,
    #[serde(default)]
    origine: String,
    fichier: String,
    #[serde(flatten)]
    place: Place3d,
    #[serde(default)]
    aspect: Aspect,
    #[serde(default)]
    allege: Vec<String>,
}

/// `decors/<nom>.glb`, le nom fait de lettres et de chiffres seulement (rien qui sorte du dossier).
fn fichier_valide(f: &str) -> bool {
    f.strip_prefix(DIR).and_then(|r| r.strip_prefix('/')).and_then(|r| r.strip_suffix(".glb")).is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric()))
}

/// Un nom de fichier de `decors/` que FEANOR a écrit (un numéro de la partie 180, ou une empreinte) : seul ce genre de fichier
/// peut être effacé par le ménage.
fn a_nous(nom: &str) -> bool {
    nom.strip_suffix(".glb").is_some_and(|s| !s.is_empty() && (s.chars().all(|c| c.is_ascii_digit()) || (s.len() == 16 && s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()))))
}

pub fn parse(texte: &str) -> Result<(), String> {
    let f: Fichier = serde_json::from_str(texte).map_err(|e| e.to_string())?;
    if f.version > VERSION_FICHIER {
        return Err(format!("décors 3D d'une version plus récente de FEANOR ({})", f.version));
    }
    Ok(())
}

/// Le contenu de `decors3d.json` ; rien : aucun décor (le fichier part).
pub fn to_bytes(doc: &Document) -> Result<Option<Vec<u8>>, String> {
    let d = &doc.decors3d;
    if d.is_empty() {
        return Ok(None);
    }
    let mut decors: Vec<serde_json::Value> = Vec::new();
    // (Partie 202 : un décor sans objet mais avec des lampes ou un cyclo s'enregistre aussi.)
    for (&sequence, x) in d.par_sequence.iter().filter(|(_, x)| !x.est_vide()) {
        let objets = x
            .objets
            .iter()
            .map(|o| {
                serde_json::to_value(EntreeObjet {
                    id: o.id,
                    nom: o.nom.clone(),
                    fichier_d_origine: o.source.clone(),
                    fichier: o.modele.fichier(),
                    place: o.place,
                    origine: Some(o.origine),
                    aimante: o.aimante,
                    allege: o.allege.clone(),
                    couleur: o.couleur,
                    cache: o.cache,
                    verrou: o.verrou,
                    cles: o.cles.clone(),
                    caches_dans: o.caches_dans.clone(),
                })
                .map_err(|e| e.to_string())
            })
            .collect::<Result<_, _>>()?;
        decors.push(serde_json::to_value(EntreeSequence { sequence, aspect: x.aspect, objets, lampes: x.lampes.clone(), cyclo: x.cyclo }).map_err(|e| e.to_string())?);
    }
    decors.extend(d.gardees.iter().cloned());
    let mut out = serde_json::to_vec_pretty(&Fichier { version: VERSION_FICHIER, decors }).map_err(|e| e.to_string())?;
    out.push(b'\n');
    Ok(Some(out))
}

/// Lit le modèle `fichier` du projet `dir`, une fois par fichier (`lus`) : les objets du même fichier partagent le modèle.
fn lire_modele(dir: &Path, fichier: &str, lus: &mut HashMap<String, Arc<Modele3d>>) -> Result<Arc<Modele3d>, String> {
    if !fichier_valide(fichier) {
        return Err(format!("fichier « {fichier} » inattendu"));
    }
    if let Some(mdl) = lus.get(fichier) {
        return Ok(mdl.clone());
    }
    let octets = std::fs::read(dir.join(fichier)).map_err(|err| format!("{fichier} : {err}"))?;
    let scene = lire_sans_paniquer(&octets)?.0;
    // (Un fichier déjà nommé par son empreinte la garde ; un numéro de la partie 180 prend la sienne : il sera réécrit sous ce
    // nom au prochain enregistrement, et l'ancien effacé.)
    let stem = fichier.trim_start_matches(DIR).trim_start_matches('/').trim_end_matches(".glb");
    let e = if stem.len() == 16 && stem.chars().all(|c| c.is_ascii_hexdigit()) { stem.to_string() } else { empreinte(&octets) };
    let mdl = Arc::new(Modele3d::avec_empreinte(Arc::new(scene), Arc::new(octets), e));
    lus.insert(fichier.to_string(), mdl.clone());
    Ok(mdl)
}

/// Lit les décors du projet `dir` (absent : aucun). Un fichier ou un modèle illisible n'empêche pas d'ouvrir le projet : on le
/// dit, le plan se montre sans cet objet, et rien n'est effacé.
pub fn load(dir: &Path, warnings: &mut Vec<String>) -> Decors3d {
    let mut out = Decors3d::default();
    let texte = match std::fs::read_to_string(dir.join(FILE)) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return out,
        Err(e) => {
            warnings.push(format!("{FILE} : {e} ; les décors 3D ne sont pas chargés."));
            out.illisible = true;
            return out;
        }
    };
    let f: Fichier = match serde_json::from_str::<Fichier>(texte.trim_start_matches('\u{feff}')) {
        Ok(f) if f.version <= VERSION_FICHIER => f,
        Ok(f) => {
            warnings.push(format!("{FILE} vient d'une version plus récente de FEANOR ({}) : les décors 3D ne sont pas chargés.", f.version));
            out.illisible = true;
            return out;
        }
        Err(e) => {
            warnings.push(format!("{FILE} : {e} ; les décors 3D ne sont pas chargés."));
            out.illisible = true;
            return out;
        }
    };
    let mut lus: HashMap<String, Arc<Modele3d>> = HashMap::new();
    let illisible = |warnings: &mut Vec<String>, e: String| warnings.push(format!("objet du décor 3D illisible ({e}) : la séquence se montre sans lui, son fichier reste dans le projet."));
    for v in f.decors {
        if v.get("objets").is_some() {
            // Version 2 : une séquence et ses objets, lus un à un (un objet abîmé n'emporte pas les autres).
            let s = match serde_json::from_value::<EntreeSequence>(v.clone()) {
                Ok(s) => s,
                Err(e) => {
                    illisible(warnings, e.to_string());
                    out.gardees.push(v);
                    continue;
                }
            };
            for ov in s.objets {
                let lu = serde_json::from_value::<EntreeObjet>(ov.clone()).map_err(|e| e.to_string()).and_then(|e| lire_modele(dir, &e.fichier, &mut lus).map(|mdl| (e, mdl)));
                match lu {
                    Ok((e, modele)) => {
                        let couleur = e.couleur.filter(|k| crate::couleurs3d::couleur(*k).is_some());
                        let mut o = Objet3d { id: e.id, nom: e.nom, source: e.fichier_d_origine, allege: e.allege, place: e.place, origine: [0.0; 3], aimante: e.aimante, modele, couleur, cache: e.cache, verrou: e.verrou, cles: crate::miseenscene3d::cles_valides(e.cles), caches_dans: e.caches_dans };
                        match e.origine.filter(|p| p.iter().all(|c| c.is_finite())) {
                            Some(p) => o.origine = p,
                            None => o.changer_origine(origine_au_sol(o.modele.bornes)),
                        }
                        let d = out.par_sequence.entry(s.sequence).or_insert_with(|| Decor3d::new(s.aspect));
                        if d.lampes.is_empty() {
                            d.lampes = crate::miseenscene3d::lampes_valides(s.lampes.clone());
                        }
                        if d.cyclo.is_none() {
                            d.cyclo = s.cyclo.map(crate::cyclos::Cyclo::borne);
                        }
                        d.objets.push(o);
                    }
                    Err(e) => {
                        illisible(warnings, e);
                        out.gardees.push(serde_json::json!({ "sequence": s.sequence, "aspect": s.aspect, "objets": [ov] }));
                    }
                }
            }
            // (Partie 200 : les lampes d'une séquence dont aucun objet n'a pu être lu restent dans le fichier, telles quelles.)
            // (Partie 202 : et un décor sans objet, ses lampes ou son cyclo seuls, est un décor.)
            if (!s.lampes.is_empty() || s.cyclo.is_some()) && !out.par_sequence.contains_key(&s.sequence) {
                let mut d = Decor3d::new(s.aspect);
                d.lampes = crate::miseenscene3d::lampes_valides(s.lampes.clone());
                d.cyclo = s.cyclo.map(crate::cyclos::Cyclo::borne);
                out.par_sequence.insert(s.sequence, d);
            }
        } else {
            // Version 1 (partie 180) : un objet, son origine au centre du dessous de sa boîte, à la même place exactement ;
            // pas aimanté (Anthony l'avait placé à la main).
            let lu = serde_json::from_value::<EntreeV1>(v.clone()).map_err(|e| e.to_string()).and_then(|e| {
                if e.fichier != format!("{DIR}/{}.glb", e.id) {
                    return Err(format!("fichier « {} » inattendu", e.fichier));
                }
                lire_modele(dir, &e.fichier, &mut lus).map(|mdl| (e, mdl))
            });
            match lu {
                Ok((e, modele)) => {
                    let mut o = Objet3d { id: e.id, nom: e.nom, source: e.origine, allege: e.allege, place: e.place, origine: [0.0; 3], aimante: false, modele, couleur: None, cache: false, verrou: false, cles: Vec::new(), caches_dans: Vec::new() };
                    o.changer_origine(origine_au_sol(o.modele.bornes));
                    out.par_sequence.entry(e.sequence).or_insert_with(|| Decor3d::new(e.aspect)).objets.push(o);
                }
                Err(e) => {
                    illisible(warnings, e);
                    out.gardees.push(v);
                }
            }
        }
    }
    out
}

/// Lit un glTF sans jamais faire tomber FEANOR : une panique du lecteur (un fichier tordu que les vérifications n'ont pas vu)
/// devient un message.
pub fn lire_sans_paniquer(octets: &[u8]) -> Result<(Scene3d, Mesures), String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| crate::decor3dgltf::lire(octets, None))).unwrap_or_else(|_| Err("fichier 3D illisible (lecture interrompue)".into()))
}

/// Les plus grands numéros cités (`Document::from_parts` : un numéro neuf ne les reprend pas).
pub fn ids(d: &Decors3d) -> impl Iterator<Item = u64> + '_ {
    let gardes = d.gardees.iter().flat_map(|v| {
        let mut n: Vec<u64> = v["id"].as_u64().into_iter().collect();
        n.extend(v["objets"].as_array().into_iter().flatten().filter_map(|o| o["id"].as_u64()));
        n
    });
    d.par_sequence.values().flat_map(|x| x.objets.iter().map(|o| o.id).chain(x.lampes.iter().map(|l| l.id))).chain(gardes)
}

/// Ce que l'enregistrement fait des décors (préparé sur le fil de l'interface, joué sur celui de l'enregistrement).
#[derive(Default)]
pub(crate) struct SavePlan {
    /// Rien à faire (un projet partagé dont je ne tiens pas le jeton) : ni écrire ni effacer.
    actif: bool,
    ecrire: Vec<(PathBuf, Arc<Vec<u8>>)>,
    json: Option<Vec<u8>>,
    utilises: BTreeSet<String>,
    proteger: bool,
}

pub(crate) fn plan_save(doc: &Document, dir: &Path) -> Result<SavePlan, String> {
    let d = &doc.decors3d;
    let mut plan = SavePlan { actif: true, json: to_bytes(doc)?, proteger: d.illisible || !d.gardees.is_empty(), ..SavePlan::default() };
    for o in d.par_sequence.values().flat_map(|x| &x.objets) {
        let nom = format!("{}.glb", o.modele.empreinte);
        let chemin = dir.join(DIR).join(&nom);
        // (Un modèle ne change jamais sous son nom, nommé par son contenu : déjà là, il n'est pas réécrit ; dix objets du même
        // modèle, un seul fichier.)
        if plan.utilises.insert(nom) && !chemin.exists() {
            plan.ecrire.push((chemin, o.modele.octets.clone()));
        }
    }
    Ok(plan)
}

impl SavePlan {
    /// Écrit les modèles qui manquent, puis `decors3d.json` (ou le retire s'il n'y a plus de décor), sans jamais détruire un
    /// fichier illisible.
    pub(crate) fn write(&self, dir: &Path) -> Result<usize, String> {
        if !self.actif {
            return Ok(0);
        }
        let mut n = 0;
        for (chemin, octets) in &self.ecrire {
            if let Some(parent) = chemin.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("{} : {e}", parent.display()))?;
            }
            crate::project::write_atomic(chemin, octets)?;
            n += 1;
        }
        if !(self.proteger && self.json.is_none()) {
            crate::project::write_sidecar(&dir.join(FILE), self.json.as_deref(), |t| parse(t).is_ok())?;
        }
        Ok(n)
    }

    /// Efface de `decors/` les modèles que plus rien ne cite (les noms que FEANOR écrit seulement, `a_nous`). Rien si le
    /// fichier des décors n'a pas été lu en entier.
    pub(crate) fn cleanup(&self, dir: &Path) -> usize {
        if !self.actif || self.proteger {
            return 0;
        }
        let Ok(entries) = std::fs::read_dir(dir.join(DIR)) else { return 0 };
        let mut n = 0;
        for e in entries.flatten() {
            let nom = e.file_name().to_string_lossy().into_owned();
            if a_nous(&nom) && !self.utilises.contains(&nom) && std::fs::remove_file(e.path()).is_ok() {
                n += 1;
            }
        }
        n
    }
}

// ------------------------------------------------------------------------------------------- dans l'image du plan

/// Ce qui fait l'image du décor d'un plan : le décor (et l'empreinte de ses objets vus et de leurs places), le repère où on le
/// regarde, l'œil, la feuille et le cadre, l'aspect. L'image se calcule avec la clé et les objets seuls (`objets_de`).
#[derive(Clone, Copy, Debug)]
pub struct DecorCle {
    /// `Decor3d::uid`.
    pub decor: u64,
    /// L'empreinte des objets vus (modèles et matrices, `contenu`) : un objet qui bouge fait une autre image.
    pub contenu: u64,
    /// Le repère du plan « en Espace 3D » (`Document::repere_du_plan`), ou l'identité.
    pub repere: M4,
    pub oeil: V3,
    pub feuille: [usize; 2],
    pub cadre: [usize; 2],
    pub aspect: Aspect,
    /// Partie 200 : le plan (`Panel::id`) et l'image de la séquence dont `contenu` est l'empreinte : de quoi retrouver les poses de
    /// ses objets animés et ceux « cachés dans le plan » (`objets_de`). Hors de l'égalité et du hachage : deux clés de même
    /// empreinte font la même image (un décor immobile garde sa seule image d'un plan et d'une image à l'autre, comme avant).
    pub plan: u64,
    pub image: usize,
}

impl DecorCle {
    fn bits(&self) -> ([u32; 16], [u32; 3], [u32; 7]) {
        let a = &self.aspect;
        // (Partie 199 : l'aspect « Dans les plans » aussi : passer de l'un à l'autre fait une autre image, et d'autres vignettes.)
        (self.repere.map(f32::to_bits), self.oeil.map(f32::to_bits), [a.facettes as u32, a.ciel as u32, a.brume as u32, a.ombres as u32, a.soleil_azimut.to_bits(), a.soleil_hauteur.to_bits(), a.rendu as u32])
    }

    pub fn monde(&self) -> Monde {
        Monde::new(self.feuille, self.cadre)
    }

    pub fn prise(&self) -> Option<Prise> {
        Prise::de_la_feuille(&self.monde(), self.oeil)
    }
}

impl PartialEq for DecorCle {
    fn eq(&self, o: &Self) -> bool {
        self.decor == o.decor && self.contenu == o.contenu && self.feuille == o.feuille && self.cadre == o.cadre && self.bits() == o.bits()
    }
}

impl Hash for DecorCle {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.decor.hash(h);
        self.contenu.hash(h);
        self.feuille.hash(h);
        self.cadre.hash(h);
        self.bits().hash(h);
    }
}

/// L'empreinte des objets que cet ordinateur montre : leurs modèles et leurs matrices.
pub fn contenu(d: &Decor3d) -> u64 {
    contenu_a(d, None, 0)
}

/// Partie 200 : `contenu` pour l'image `image` de la séquence et le plan `plan` (rien : tous les plans) : les poses de ses objets
/// animés à cette image, sans ceux cachés dans ce plan, et ses lampes. Un décor immobile, sans objet caché dans le plan ni lampe, a
/// exactement l'empreinte d'avant (la même suite de hachages) : il garde son image et ses vignettes, au même coût.
pub fn contenu_a(d: &Decor3d, plan: Option<u64>, image: usize) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    // (Partie 199 : et leurs couleurs d'identification, quand les plans les montrent : changer une couleur redessine l'image une fois,
    // sans rien refaire des maillages.)
    let couleurs = d.aspect.rendu == crate::couleurs3d::Rendu::Couleurs;
    for o in d.vus().filter(|o| plan.is_none_or(|p| !o.cache_dans(p))) {
        o.modele.uid.hash(&mut h);
        o.matrice_a(image).map(f32::to_bits).hash(&mut h);
        if couleurs {
            o.couleur.hash(&mut h);
        }
    }
    // (Partie 200 : les lampes, seulement quand il y en a : un décor sans lampe ne hache rien de plus.)
    if d.lampes.iter().any(|l| !l.cache) {
        "lampes".hash(&mut h);
        for l in d.lampes.iter().filter(|l| !l.cache) {
            l.empreinte(&mut h);
        }
    }
    // (Partie 202 : le cyclo, seulement quand il y en a un : ses réglages, sa rotation, et si son image est prête.)
    if let Some(c) = &d.cyclo {
        "cyclo".hash(&mut h);
        c.cle().hash(&mut h);
        c.rotation.to_bits().hash(&mut h);
        crate::cyclos::texture(c.cle()).is_some().hash(&mut h);
    }
    h.finish()
}

/// Partie 202 : le cyclo du décor `d`, prêt pour le rendu (son image demandée : faite tout de suite hors du fil de l'interface,
/// sur un autre fil sinon) ; rien sans cyclo.
pub fn cyclo_vu(d: &Decor3d) -> crate::cyclos::CycloVu {
    match &d.cyclo {
        Some(c) => crate::cyclos::CycloVu { pret: crate::cyclos::assurer(c).is_some(), ..c.vu() },
        None => Default::default(),
    }
}

/// L'aspect du décor `d` pour un rendu dans le repère `repere` : le sien, ses lampes (partie 200) et son cyclo (partie 202).
pub fn aspect_vu(d: &Decor3d, repere: &M4) -> Aspect {
    Aspect { lampes: crate::miseenscene3d::lampes_vues(d, repere), cyclo: cyclo_vu(d), ..d.aspect }
}

/// Partie 202 : un modèle sans rien : la pose d'un décor sans objet vu (son cyclo, ses lampes), pour que son aspect aille au rendu.
pub fn modele_vide() -> Arc<Modele3d> {
    static VIDE: std::sync::OnceLock<Arc<Modele3d>> = std::sync::OnceLock::new();
    VIDE.get_or_init(|| Arc::new(Modele3d::avec_empreinte(Arc::new(Scene3d { primitives: Vec::new(), textures: Vec::new() }), Arc::new(Vec::new()), "vide".into()))).clone()
}

/// Numéro du dessin dérivé du décor (bit de poids fort mis : il ne tombe jamais sur un vrai dessin).
pub fn id_of(key: &DecorCle) -> CelId {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    "decor3d".hash(&mut h);
    key.hash(&mut h);
    h.finish() | (1 << 63)
}

/// Le numéro de la texture de la carte graphique qui montre le décor `decor` (`Decor3d::uid`) : une par décor (pas par position
/// de la caméra ni des objets : ce qui bouge redessine la même texture, il n'en crée pas).
pub fn id_texture(decor: u64) -> CelId {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    "decor3d texture".hash(&mut h);
    decor.hash(&mut h);
    h.finish() | (1 << 63)
}

/// L'élément du plan `index` qui montre son décor, à l'image `frame` du plan où la caméra est `camera` (et `repere`, pour un plan
/// en Espace 3D) ; rien : la séquence n'a pas d'objet vu dans ce plan, ou l'œil n'est pas devant la feuille.
pub fn item(doc: &Document, index: usize, frame: usize, camera: &crate::document::CameraSeen, repere: Option<&M4>) -> Option<crate::render::PlanItem> {
    let d = doc.decor_du_plan(index)?;
    let plan = doc.panels[index].id;
    // (Partie 202 : sans objet vu, son cyclo suffit : le ciel, et sa lumière sur les cartes.)
    if d.vus_dans(plan).next().is_none() && d.cyclo.is_none() {
        return None;
    }
    let monde = Monde::of(doc);
    let (repere, oeil) = match repere {
        Some(r) => (*r, monde.eye_reference()),
        None => (m::IDENT, monde.eye_seen(camera)),
    };
    // (Partie 200 : l'image de la séquence, pour les objets animés ; un décor immobile n'en a pas besoin, elle ne change rien.)
    let image = if d.anime() { doc.image_de_sequence(index, frame) } else { 0 };
    let aspect = aspect_vu(d, &repere);
    let key = DecorCle { decor: d.uid, contenu: contenu_a(d, Some(plan), image), repere, oeil, feuille: [doc.width, doc.height], cadre: doc.frame, aspect, plan, image };
    key.prise()?;
    Some(crate::render::PlanItem {
        cel: id_of(&key),
        opacity: 255,
        tint: None,
        transform: None,
        blend: crate::blend::BlendMode::Normal,
        fx: Vec::new(),
        ctg: None,
        fond: Some(crate::render::FondPart::Decor(key)),
        pre: None,
        profondeur: None, ombre: None,
    })
}

/// Les modèles masqués sur cet ordinateur pour la séance (plus lourds que son budget, en attendant le choix d'Anthony dans la
/// fenêtre « Ce décor est lourd ») : ni l'image du plan ni la vue ne les dessinent, rien ne ralentit.
static MASQUES: Mutex<BTreeSet<u64>> = Mutex::new(BTreeSet::new());

pub fn masquer(uid: u64, oui: bool) {
    let mut g = MASQUES.lock().unwrap_or_else(|e| e.into_inner());
    if oui {
        g.insert(uid);
    } else {
        g.remove(&uid);
    }
}

pub fn masque(uid: u64) -> bool {
    MASQUES.lock().unwrap_or_else(|e| e.into_inner()).contains(&uid)
}

/// Un objet à dessiner : son modèle, sa matrice dans le repère où on le regarde, et sa couleur d'identification (partie 199).
pub type Pose3d = (Arc<Modele3d>, M4, Option<u8>);

/// Les objets que ce décor montre, à dessiner (`Pose3d`), dans le repère du décor.
pub fn poses(d: &Decor3d) -> Vec<Pose3d> {
    d.vus().map(|o| (o.modele.clone(), o.matrice(), o.couleur)).collect()
}

/// Partie 200 : `poses`, sans les objets cachés dans le plan `plan` (rien : aucun n'est retiré).
pub fn poses_sauf(d: &Decor3d, plan: Option<u64>) -> Vec<Pose3d> {
    let v: Vec<Pose3d> = d.vus().filter(|o| plan.is_none_or(|p| !o.cache_dans(p))).map(|o| (o.modele.clone(), o.matrice(), o.couleur)).collect();
    // (Partie 202 : sans objet, la pose vide : le cyclo va au rendu.)
    if v.is_empty() && d.cyclo.is_some() {
        return vec![(modele_vide(), m::IDENT, None)];
    }
    v
}

/// Les objets du décor de la clé `key`, tels qu'ils étaient quand la clé a été faite ; rien : le décor a changé depuis (ou n'est
/// plus dans le projet), la clé ne vaut plus.
pub fn objets_de(doc: &Document, key: &DecorCle) -> Option<Vec<Pose3d>> {
    let d = doc.decors3d.par_sequence.values().find(|d| d.uid == key.decor)?;
    // (Partie 200 : à l'image et dans le plan de la clé : les objets animés à leur pose, sans ceux cachés dans ce plan.)
    (contenu_a(d, Some(key.plan), key.image) == key.contenu).then(|| {
        let v: Vec<Pose3d> = d.vus_dans(key.plan).map(|o| (o.modele.clone(), m::mul(&key.repere, &o.matrice_a(key.image)), o.couleur)).collect();
        // (Partie 202 : sans objet vu, la pose vide : l'aspect du décor, son ciel, va au rendu.)
        if v.is_empty() {
            vec![(modele_vide(), m::IDENT, None)]
        } else {
            v
        }
    })
}

/// Les triangles que dessinent ces objets (chaque objet compte).
pub fn triangles_de(objets: &[Pose3d]) -> u64 {
    objets.iter().map(|(mdl, _, _)| mdl.scene.triangles() as u64).sum()
}

/// Ce que la carte graphique dessine pour ces objets : un dessin par objet ; les objets d'un même modèle ont le même `id`, le
/// modèle part à la carte une fois.
pub fn dessins_gpu(objets: &[Pose3d], aspect: Aspect) -> Vec<crate::decor3dgpu::Dessin> {
    // (Partie 199 : chaque objet dans sa teinte pour l'aspect `aspect.rendu` ; le modèle à la carte, lui, ne change pas.)
    objets.iter().map(|(mdl, mx, k)| crate::decor3dgpu::Dessin { id: mdl.uid, scene: mdl.scene.clone(), matrice: *mx, aspect, teinte: crate::couleurs3d::Teinte::de(aspect.rendu, *k) }).collect()
}

// -------------------------------------------------------------------------------- l'image calculée (exports, secours)

/// La carte graphique que l'export emprunte pour rendre le décor comme à l'écran (`poser_carte`) ; rien : le processeur.
static CARTE: Mutex<Option<(eframe::egui_wgpu::wgpu::Device, eframe::egui_wgpu::wgpu::Queue, Option<crate::decor3dgpu::Moteur>)>> = Mutex::new(None);

/// L'application a une carte graphique : l'export du décor s'en servira (la même image qu'à l'écran).
pub fn poser_carte(rs: &eframe::egui_wgpu::RenderState) {
    *CARTE.lock().unwrap_or_else(|e| e.into_inner()) = Some((rs.device.clone(), rs.queue.clone(), None));
}

/// Le budget 3D de cette machine, tel que l'application le tient (`decor3dbudget.rs`) : il choisit la qualité des ombres à
/// l'écran et à l'export.
static BUDGET: Mutex<Option<crate::decor3dbudget::BudgetMachine>> = Mutex::new(None);

pub fn poser_budget(b: Option<crate::decor3dbudget::BudgetMachine>) {
    *BUDGET.lock().unwrap_or_else(|e| e.into_inner()) = b;
}

pub fn budget() -> Option<crate::decor3dbudget::BudgetMachine> {
    BUDGET.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// La qualité des ombres d'un décor de `triangles` triangles (tous ses objets) sur cette machine : la meilleure qui tient dans
/// son budget (sans budget mesuré : pas d'ombres, rien ne ralentit avant la mesure).
pub fn qualite(triangles: u64) -> crate::decor3dbudget::Qualite {
    #[cfg(test)]
    if let Some(q) = QUALITE_ESSAI.with(|q| q.get()) {
        return q;
    }
    BUDGET.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map_or(crate::decor3dbudget::Qualite::Sans, |b| b.qualite_pour(triangles))
}

/// **Les bords doux** (partie 193) d'un décor de `triangles` triangles dessiné avec les ombres `q` tiennent-ils dans le budget de
/// cette machine (sa passe plus la profondeur à quatre échantillons) ? Sans budget mesuré : non, rien ne ralentit avant la mesure
/// (les bords sont nets, comme à la partie 183). Les essais les forcent (`essai_bords`).
pub fn bords_doux(triangles: u64, q: crate::decor3dbudget::Qualite) -> bool {
    #[cfg(test)]
    if let Some(b) = BORDS_ESSAI.with(|b| b.get()) {
        return b;
    }
    match BUDGET.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        Some(b) => b.bords_doux_pour(triangles, q),
        None => false,
    }
}

/// Essais : les bords doux pour ce fil (rien : selon le budget).
#[cfg(test)]
pub(crate) fn essai_bords(b: Option<bool>) {
    BORDS_ESSAI.with(|c| c.set(b));
}

#[cfg(test)]
thread_local! {
    /// Essais (partie 193) : les bords doux de ce fil, forcés.
    static BORDS_ESSAI: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    /// Essais (partie 185) : la qualité des ombres de ce fil, sans toucher au budget de la machine que les autres essais voient.
    static QUALITE_ESSAI: std::cell::Cell<Option<crate::decor3dbudget::Qualite>> = const { std::cell::Cell::new(None) };
    /// Essais : la carte graphique que l'export de ce fil emprunte (`poser_carte` vaut pour tous les fils).
    static CARTE_ESSAI: std::cell::RefCell<Option<(eframe::egui_wgpu::wgpu::Device, eframe::egui_wgpu::wgpu::Queue, Option<crate::decor3dgpu::Moteur>)>> = const { std::cell::RefCell::new(None) };
}

/// Essais : la qualité des ombres et la carte graphique prêtée à l'export, pour ce fil seulement.
#[cfg(test)]
pub(crate) fn essai_poser(qualite: Option<crate::decor3dbudget::Qualite>, carte: Option<&eframe::egui_wgpu::RenderState>) {
    QUALITE_ESSAI.with(|q| q.set(qualite));
    CARTE_ESSAI.with(|c| *c.borrow_mut() = carte.map(|rs| (rs.device.clone(), rs.queue.clone(), None)));
}

/// Lissage des bords selon la taille de la feuille (le rendu par le processeur).
fn sur_echantillonnage(feuille: [usize; 2]) -> usize {
    if feuille[0] * feuille[1] <= 2_500_000 {
        2
    } else {
        1
    }
}

/// L'image du décor pour la clé `key` et ses objets : la feuille entière, transparente hors du décor (opaque avec le ciel), et sa
/// profondeur (partie 183, `profondeur.rs` : l'inverse de la distance à l'œil à chaque pixel, 0 là où rien n'est dessiné).
///
/// Et, rendu par la carte graphique avec ses ombres, son soleil (partie 185, `ombres::Soleil`) : ce que les ombres des cartes lisent
/// à la composition de l'export. Le rendu de secours par le processeur n'a pas d'ombres, donc pas de soleil.
pub fn calculer(objets: &[Pose3d], key: &DecorCle, fils: usize) -> (Raster, Option<crate::profondeur::ProfDecor>, Option<crate::ombres::Soleil>) {
    let [w, h] = key.feuille;
    let Some(prise) = key.prise() else { return (Raster::new(w, h), None, None) };
    let par_la_carte = |device: &eframe::egui_wgpu::wgpu::Device, queue: &eframe::egui_wgpu::wgpu::Queue, mt: &mut Option<crate::decor3dgpu::Moteur>| {
        let mt = mt.get_or_insert_with(|| crate::decor3dgpu::Moteur::new(device, queue));
        let d = dessins_gpu(objets, key.aspect);
        let q = if key.aspect.ombres { qualite(triangles_de(objets)) } else { crate::decor3dbudget::Qualite::Sans };
        // (Les bords doux, partie 193 : comme à l'écran, si le décor tient dans le budget avec eux ; une carte qui ne partage pas la
        // profondeur ne les lit pas.)
        let quatre = bords_doux(triangles_de(objets), q);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| crate::decor3dgpu::rendre_hors_ecran_tout(device, queue, mt, &d, &prise, [w as u32, h as u32], key.aspect.ciel, q, 1, true, true, quatre)));
        match r {
            Ok(Some((rgba, _, prof, ombres))) => {
                let soleil = ombres.and_then(|o| crate::ombres::Lumiere::de(&o.eclairage, &key.aspect, &prise).map(|lum| crate::ombres::Soleil { lum, soleil: o.soleil, carte: o.carte }));
                Some((Raster::from_rgba(w, h, &rgba), prof, soleil))
            }
            _ => None,
        }
    };
    #[cfg(test)]
    {
        let r = CARTE_ESSAI.with(|c| c.borrow_mut().as_mut().and_then(|(device, queue, mt)| par_la_carte(device, queue, mt)));
        if let Some(r) = r {
            return r;
        }
    }
    // La carte graphique d'abord, si l'application en a une (la même image qu'à l'écran).
    {
        let mut g = CARTE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((device, queue, mt)) = g.as_mut() {
            if let Some(r) = par_la_carte(device, queue, mt) {
                return r;
            }
        }
    }
    let scenes: Vec<(&Scene3d, M4)> = objets.iter().map(|(mdl, mx, _)| (&*mdl.scene, *mx)).collect();
    // (Partie 199 : sans carte graphique aussi, chaque objet dans sa teinte : l'export par le processeur montre le même aspect.)
    let teintes: Vec<crate::couleurs3d::Teinte> = objets.iter().map(|(_, _, k)| crate::couleurs3d::Teinte::de(key.aspect.rendu, *k)).collect();
    let (rgba, prof) = crate::decor3drendu::rendre_objets_prof_teintes(&scenes, &teintes, &prise, [w, h], sur_echantillonnage(key.feuille), fils, &key.aspect);
    (Raster::from_rgba(w, h, &rgba), Some(prof), None)
}

fn cel_of(raster: Raster, key: &DecorCle) -> Arc<Cel> {
    // (La version est le numéro du rendu : deux rendus différents n'ont jamais la même, la carte graphique le sait.)
    Arc::new(Cel { raster, vector: None, version: id_of(key), fond: None })
}

/// La profondeur du rendu du décor dont le dessin porte la version `version` (`cel_of` : le numéro du rendu), s'il est gardé :
/// la composition par le processeur y cache les cartes (partie 183, `profondeur.rs`).
pub fn profondeur_du_rendu(version: u64) -> Option<crate::profondeur::Profondeurs> {
    garde(|g| g.profs.get(&version).cloned())
}

/// Le soleil du rendu du décor dont le dessin porte la version `version`, s'il a été rendu par la carte graphique avec ses ombres
/// et est gardé : la composition par le processeur y pose les ombres des cartes (partie 185, `ombres.rs`).
pub fn soleil_du_rendu(version: u64) -> Option<Arc<crate::ombres::Soleil>> {
    garde(|g| g.soleils.get(&version).cloned())
}

type Rendu = (Raster, Option<crate::profondeur::ProfDecor>, Option<crate::ombres::Soleil>);

struct Travail {
    key: DecorCle,
    handle: std::thread::JoinHandle<Option<Rendu>>,
}

#[derive(Default)]
struct Garde {
    finis: HashMap<CelId, Arc<Cel>>,
    ordre: VecDeque<CelId>,
    /// Le dernier rendu fini de chaque décor : ce que l'écran montre en attendant le suivant.
    dernier: HashMap<u64, Arc<Cel>>,
    /// La profondeur de chaque rendu gardé (dans `finis` ou `dernier`), par le numéro du rendu (partie 183).
    profs: HashMap<CelId, crate::profondeur::Profondeurs>,
    /// Le soleil de chaque rendu gardé, s'il en a un (partie 185).
    soleils: HashMap<CelId, Arc<crate::ombres::Soleil>>,
    en_cours: Option<Travail>,
    attente: Option<(DecorCle, Vec<Pose3d>)>,
    arrive: bool,
}

const GARDES: usize = 6;
static GARDE: Mutex<Option<Garde>> = Mutex::new(None);
/// Le fil de l'interface : là, on n'attend jamais un rendu du décor.
static FIL_INTERFACE: std::sync::OnceLock<std::thread::ThreadId> = std::sync::OnceLock::new();

/// Le fil qui appelle est celui de l'interface (l'application le dit au démarrage).
pub fn poser_fil_interface() {
    let _ = FIL_INTERFACE.set(std::thread::current().id());
}

pub(crate) fn sur_l_interface() -> bool {
    FIL_INTERFACE.get().is_some_and(|id| *id == std::thread::current().id())
}

fn garde<R>(f: impl FnOnce(&mut Garde) -> R) -> R {
    let mut g = GARDE.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(Garde::default))
}

fn ranger(g: &mut Garde, key: &DecorCle, cel: Arc<Cel>, prof: Option<crate::profondeur::ProfDecor>, soleil: Option<crate::ombres::Soleil>) {
    let id = id_of(key);
    match soleil {
        Some(s) => {
            g.soleils.insert(id, Arc::new(s));
        }
        None => {
            g.soleils.remove(&id);
        }
    }
    if g.finis.insert(id, cel.clone()).is_none() {
        g.ordre.push_back(id);
    }
    match prof {
        Some(p) => {
            g.profs.insert(id, Arc::new(p));
        }
        None => {
            g.profs.remove(&id);
        }
    }
    while g.ordre.len() > GARDES {
        if let Some(old) = g.ordre.pop_front() {
            g.finis.remove(&old);
        }
    }
    g.dernier.insert(key.decor, cel);
    // (Une profondeur ne reste que pour un rendu encore gardé.)
    let (finis, dernier) = (&g.finis, &g.dernier);
    g.profs.retain(|v, _| finis.contains_key(v) || dernier.values().any(|c| c.version == *v));
    g.soleils.retain(|v, _| finis.contains_key(v) || dernier.values().any(|c| c.version == *v));
}

fn coeurs() -> usize {
    (std::thread::available_parallelism().map_or(2, |n| n.get()) / 2).max(1)
}

fn lancer(g: &mut Garde) {
    if g.en_cours.is_some() {
        return;
    }
    let Some((key, objets)) = g.attente.take() else { return };
    let threads = coeurs();
    let handle = std::thread::spawn(move || std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| calculer(&objets, &key, threads))).ok());
    g.en_cours = Some(Travail { key, handle });
}

/// Le décor tel que le processeur le compose : prêt, ou, à l'écran (`fx::differe`, ou le fil de l'interface), le dernier
/// rendu de ce décor en attendant le nouveau, calculé sur un autre fil (jamais d'attente) ; ailleurs (export, essais), calculé
/// tout de suite, exact.
pub fn montre(doc: &Document, key: &DecorCle) -> Option<crate::fx::Shown<'static>> {
    let id = id_of(key);
    if let Some(cel) = garde(|g| g.finis.get(&id).cloned()) {
        return Some(crate::fx::Shown::Fx(cel));
    }
    let objets = objets_de(doc, key)?;
    if crate::fx::differe_actif() || sur_l_interface() {
        return garde(|g| {
            let deja = g.en_cours.as_ref().is_some_and(|t| t.key == *key) || g.attente.as_ref().is_some_and(|e| e.0 == *key);
            if !deja {
                g.attente = Some((*key, objets));
                lancer(g);
            }
            g.dernier.get(&key.decor).cloned().map(crate::fx::Shown::Fx)
        });
    }
    let (raster, prof, soleil) = calculer(&objets, key, coeurs().max(2));
    let cel = cel_of(raster, key);
    garde(|g| ranger(g, key, cel.clone(), prof, soleil));
    Some(crate::fx::Shown::Fx(cel))
}

/// Pose les rendus finis et lance le suivant ; vrai si un rendu vient d'arriver (il faut redessiner). Une fois par image
/// d'écran.
pub fn poll() -> bool {
    garde(|g| {
        if g.en_cours.as_ref().is_some_and(|t| t.handle.is_finished()) {
            if let Some(t) = g.en_cours.take() {
                if let Ok(Some((raster, prof, soleil))) = t.handle.join() {
                    let cel = cel_of(raster, &t.key);
                    ranger(g, &t.key, cel, prof, soleil);
                    g.arrive = true;
                    crate::fx::decor_arrive();
                }
            }
        }
        lancer(g);
        std::mem::take(&mut g.arrive)
    })
}

/// Un rendu du décor est en cours ou attend son tour.
pub fn pending() -> bool {
    garde(|g| g.en_cours.is_some() || g.attente.is_some())
}

/// Un autre projet s'ouvre : les rendus de l'ancien ne sont pas les siens.
pub fn forget_all() {
    garde(|g| {
        g.finis.clear();
        g.ordre.clear();
        g.dernier.clear();
        g.profs.clear();
        g.soleils.clear();
        g.attente = None;
    });
    crate::ombres::oublier();
}

/// Essais : attend la fin des rendus de fond.
#[cfg(test)]
pub(crate) fn wait_idle() {
    let start = std::time::Instant::now();
    while pending() {
        poll();
        std::thread::sleep(std::time::Duration::from_millis(2));
        assert!(start.elapsed() < std::time::Duration::from_secs(60), "le rendu du décor ne finit pas");
    }
}

// ------------------------------------------------------------------------------------------------ l'import

/// Un fichier lu et mesuré, prêt à devenir un décor (sur un autre fil : `decor3dui.rs`).
#[derive(Clone)]
pub struct Analyse {
    pub nom: String,
    /// Le nom du fichier d'origine.
    pub origine: String,
    /// Le `.glb` autonome à garder tel quel (« Importer tel quel »).
    pub glb: Arc<Vec<u8>>,
    pub scene: Arc<Scene3d>,
    pub mesures: Mesures,
    /// Le temps d'une image de ce décor sur la carte de cette machine (millisecondes, à la taille de la feuille), s'il a pu
    /// être mesuré.
    pub cout_ms: Option<f32>,
    /// L'empreinte de `glb` (`empreinte`), calculée ici, sur l'autre fil.
    pub empreinte: String,
}

/// Lit et mesure le fichier `chemin`. Refusé avant d'être lu s'il dépasse `PLAFONDS.fichier`.
pub fn analyser(chemin: &Path) -> Result<Analyse, String> {
    let taille = std::fs::metadata(chemin).map_err(|e| format!("{} : {e}", chemin.display()))?.len();
    if taille > PLAFONDS.fichier {
        return Err(format!("fichier de {} : FEANOR n'accepte pas de décor de plus de {} (réduis-le dans un outil 3D).", octets_lisibles(taille), octets_lisibles(PLAFONDS.fichier)));
    }
    let octets = std::fs::read(chemin).map_err(|e| format!("{} : {e}", chemin.display()))?;
    let origine = chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let nom = chemin.file_stem().map(|n| n.to_string_lossy().into_owned()).filter(|n| !n.is_empty()).unwrap_or_else(|| "Décor".into());
    analyser_octets(&octets, chemin.parent(), nom, origine)
}

/// Comme `analyser`, depuis les octets (essais) ; `dossier` : celui du fichier, pour un `.gltf` et ses annexes.
pub fn analyser_octets(octets: &[u8], dossier: Option<&Path>, nom: String, origine: String) -> Result<Analyse, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (scene, mesures) = crate::decor3dgltf::lire(octets, dossier)?;
        let glb = crate::decor3dgltf::empaqueter(octets, dossier)?;
        let e = empreinte(&glb);
        Ok(Analyse { nom, origine, glb: Arc::new(glb), scene: Arc::new(scene), mesures, cout_ms: None, empreinte: e })
    }))
    .unwrap_or_else(|_| Err("fichier 3D illisible (lecture interrompue)".into()))
}

/// Ce qu'on fait d'un fichier mesuré, comparé au budget de la machine.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    /// Il tient : importé tout de suite.
    Direct,
    /// Il dépasse : la fenêtre « Ce décor est lourd ». `tel_quel` : « Importer tel quel » est permis (seul le poids du
    /// fichier dépasse, pas le temps d'une image ni la mémoire).
    Lourd { raisons: Vec<String>, tel_quel: bool, simplifier: bool },
}

/// Le fichier est lourd à partir de ce poids (des cartes et des données que FEANOR n'utilise pas, d'ordinaire) : l'alléger est
/// proposé, sans obligation.
pub const FICHIER_LOURD: u64 = 256 << 20;

pub fn octets_lisibles(o: u64) -> String {
    if o >= 1 << 30 {
        format!("{:.1} Go", o as f64 / (1u64 << 30) as f64).replace('.', ",")
    } else if o >= 1 << 20 {
        format!("{:.0} Mo", o as f64 / (1u64 << 20) as f64)
    } else {
        format!("{} Ko", (o >> 10).max(1))
    }
}

/// Partie 198.5 : en deçà de cette part du budget de la machine, en triangles **et** en textures, le temps mesuré d'une image du
/// décor ne compte pas : il n'est alors que le coût fixe d'une passe à la taille de la feuille (le ciel, la brume, la profondeur), que
/// l'étalonnage a déjà mesuré et compté dans le budget (`decor3dbudget::Cout::fixe`), et une mesure qui tombe au mauvais moment (la
/// carte occupée ailleurs, un pilote qui se réveille) ne fait pas juger lourd un modèle de quelques centaines de triangles.
pub const PART_SANS_CHRONO: u64 = 20;

/// Le modèle est loin sous le budget de la machine (moins d'un vingtième, 5 %, en triangles et en textures) : son temps mesuré ne
/// le juge pas.
pub fn loin_sous_le_budget(m: &Mesures, budget: &crate::decor3dbudget::BudgetMachine) -> bool {
    (m.triangles as u64).saturating_mul(PART_SANS_CHRONO) <= budget.triangles && m.octets_textures.saturating_mul(PART_SANS_CHRONO) <= budget.textures_mo << 20
}

pub fn verdict(m: &Mesures, budget: &crate::decor3dbudget::BudgetMachine, cout_ms: Option<f32>) -> Verdict {
    use crate::decor3dbudget::grand_nombre;
    let mut raisons = Vec::new();
    let (mut lent, mut memoire) = (false, false);
    // (Partie 198.5 : le temps mesuré ne compte que s'il dépasse strictement le budget d'une passe, et seulement pour un modèle qui
    // n'est pas loin sous le budget en triangles et en textures.)
    let cout_ms = cout_ms.filter(|_| !loin_sous_le_budget(m, budget));
    if m.triangles as u64 > budget.triangles {
        raisons.push(format!("{} triangles : cet ordinateur en dessine {} sans ralentir.", grand_nombre(m.triangles as u64), grand_nombre(budget.triangles)));
        lent = true;
    } else if let Some(ms) = cout_ms.filter(|ms| *ms > crate::decor3dbudget::BudgetMachine::ms_par_passe()) {
        raisons.push(format!("une image de ce décor prend {ms:.2} ms à la carte graphique : au-delà des {:.2} ms qu'on lui laisse.", crate::decor3dbudget::BudgetMachine::ms_par_passe()));
        lent = true;
    }
    if m.octets_textures > budget.textures_mo << 20 {
        raisons.push(format!("{} de textures en mémoire : cet ordinateur en garde {} Mo sans gêne.", octets_lisibles(m.octets_textures), budget.textures_mo));
        memoire = true;
    }
    if m.octets_fichier > FICHIER_LOURD {
        raisons.push(format!("fichier de {}{}.", octets_lisibles(m.octets_fichier), if m.cartes_inutiles > 0 { format!(", dont {} de cartes que FEANOR n'utilise pas", octets_lisibles(m.octets_cartes_inutiles)) } else { String::new() }));
    }
    if raisons.is_empty() {
        Verdict::Direct
    } else {
        Verdict::Lourd { raisons, tel_quel: !lent && !memoire, simplifier: lent }
    }
}

/// Le verdict d'un objet qu'on **ajoute** au décor d'une séquence qui pèse déjà `deja` (partie 181) : le budget de la machine
/// vaut pour tout le décor, la somme de ses objets. `partage` : l'objet reprend un modèle déjà là (ses textures ne comptent
/// pas deux fois, la carte les garde une fois).
pub fn verdict_sequence(m: &Mesures, deja: Poids, partage: bool, budget: &crate::decor3dbudget::BudgetMachine, cout_ms: Option<f32>) -> Verdict {
    if deja == Poids::default() {
        return verdict(m, budget, cout_ms);
    }
    let mut tout = m.clone();
    tout.triangles = m.triangles.saturating_add(deja.triangles as usize);
    tout.octets_textures = if partage { 0 } else { m.octets_textures } + deja.octets_textures;
    match verdict(&tout, budget, cout_ms) {
        Verdict::Direct => Verdict::Direct,
        Verdict::Lourd { mut raisons, tel_quel, simplifier } => {
            for r in raisons.iter_mut().filter(|r| r.contains("triangles") || r.contains("textures en mémoire")) {
                *r = format!("avec les objets déjà posés dans la séquence, {r}");
            }
            Verdict::Lourd { raisons, tel_quel, simplifier }
        }
    }
}

/// La séquence est déjà au bout du budget de la machine : aucun objet ne peut s'y ajouter sans ralentir, même simplifié
/// (le message à montrer) ; rien : il reste de la place.
pub fn sequence_pleine(deja: Poids, budget: &crate::decor3dbudget::BudgetMachine) -> Option<String> {
    use crate::decor3dbudget::grand_nombre;
    if deja.triangles > 0 && deja.triangles >= budget.triangles {
        return Some(format!("le décor de la séquence a déjà {} triangles, ce que cet ordinateur dessine au plus sans ralentir : enlève ou simplifie un objet avant d'en ajouter un autre.", grand_nombre(deja.triangles)));
    }
    if deja.octets_textures > 0 && deja.octets_textures >= budget.textures_mo << 20 {
        return Some(format!("le décor de la séquence a déjà {} de textures, ce que cet ordinateur garde au plus : enlève ou allège un objet avant d'en ajouter un autre.", octets_lisibles(deja.octets_textures)));
    }
    None
}

/// Ce qu'on demande à l'allègement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// Côté maximal des textures ; rien : gardées.
    pub cote_max: Option<u32>,
    /// Simplifier la géométrie jusqu'à tant de triangles ; rien : la forme reste.
    pub triangles: Option<usize>,
}

/// Le décor allégé (et peut-être simplifié), prêt à importer.
#[derive(Clone)]
pub struct Allege {
    pub glb: Arc<Vec<u8>>,
    /// L'empreinte de `glb`, calculée sur le fil de l'allègement.
    pub empreinte: String,
    pub scene: Arc<Scene3d>,
    pub mesures: Mesures,
    pub notes: Vec<String>,
}

/// Allège `a` selon `o` : un `.glb` neuf écrit par FEANOR (`decor3dgltf::ecrire_glb`), relu pour vérifier. Le fichier d'origine
/// n'est pas touché.
pub fn alleger(a: &Analyse, o: Options, suivi: &crate::decor3dsimplifier::Suivi) -> Result<Allege, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut notes = Vec::new();
        let simplifiee;
        let scene: &Scene3d = match o.triangles.filter(|&t| t < a.scene.triangles()) {
            Some(t) => {
                simplifiee = crate::decor3dsimplifier::simplifier(&a.scene, t, suivi)?;
                notes.push(format!("simplifié : {} → {} triangles", crate::decor3dbudget::grand_nombre(a.scene.triangles() as u64), crate::decor3dbudget::grand_nombre(simplifiee.triangles() as u64)));
                &simplifiee
            }
            None => &a.scene,
        };
        if let Some(c) = o.cote_max.filter(|&c| a.mesures.plus_grand_cote > c) {
            notes.push(format!("textures {} → {c}", a.mesures.plus_grand_cote));
        }
        let mm = &a.mesures;
        if mm.cartes_inutiles > 0 {
            notes.push(format!("cartes de normales, de rugosité, d'occlusion ou d'émission retirées ({})", mm.cartes_inutiles));
        }
        if mm.uv_en_plus + mm.tangentes + mm.couleurs_sommets > 0 {
            notes.push("coordonnées de texture en plus, tangentes et couleurs de sommets retirées".into());
        }
        if mm.animations + mm.peaux + mm.morphoses > 0 {
            notes.push("animations, squelettes et morphoses retirés (le fichier d'origine les garde)".into());
        }
        let glb = crate::decor3dgltf::ecrire_glb(scene, crate::decor3dgltf::Ecriture { cote_max: o.cote_max })?;
        let (relue, mut mesures) = crate::decor3dgltf::lire(&glb, None)?;
        mesures.octets_fichier = glb.len() as u64;
        let e = empreinte(&glb);
        Ok(Allege { glb: Arc::new(glb), empreinte: e, scene: Arc::new(relue), mesures, notes })
    }))
    .unwrap_or_else(|_| Err("allègement interrompu".into()))
}
