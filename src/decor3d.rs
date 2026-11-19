//! Le décor 3D d'une séquence (partie 180, `docs/INTERFACE.md`) : un modèle glTF importé pour une **séquence**, que tous ses
//! plans montrent, filmé par la caméra de chaque plan, dans la vue Espace 3D et dans l'image du plan (écran et exports).
//!
//! Première étape du moteur de prévisualisation qu'Anthony a choisi le 2026-11-19 (`docs/ESPACE3D.md`) :
//! - **Derrière toutes les cartes** : le décor est l'élément le plus bas de l'image du plan (`render::plan_with_camera_layers`),
//!   juste au-dessus du papier ; chaque calque se pose dessus. Juste en profondeur en lui-même (tampon de profondeur), pas avec
//!   les cartes : la profondeur partagée entre le décor et les cartes est une étape suivante.
//! - **Vu de l'œil de la caméra, aligné sur la feuille** (`decor3drendu::Prise::de_la_feuille`), comme une carte en
//!   profondeur : l'homographie de la caméra 2D (panoramique, bascule, zoom, rotation) s'applique ensuite à tout le plan, le décor
//!   défile avec la parallaxe des cartes. Un plan « en Espace 3D » (`Document::repere_du_plan`) pose le décor comme ses cartes :
//!   passé dans le repère de la caméra, vu de l'œil de référence.
//! - À l'écran avec une carte graphique, c'est elle qui dessine (`decor3dgpu.rs`, passe de `GpuCanvas`) ; sans carte, le
//!   processeur sur un autre fil (`montre`), qui ne fait jamais attendre l'interface ; à l'export, l'image exacte, tout de suite
//!   (la carte graphique si FEANOR en a une, `poser_carte`, sinon le processeur).
//!
//! **Dans le projet, sans changer `VERSION`** : `decors3d.json` à côté du manifeste (séquence, nom, fichier d'origine, place,
//! aspect, ce qui a été allégé), et le modèle dans `decors/<id>.glb` (toujours un `.glb` autonome, `decor3dgltf::empaqueter`),
//! jamais modifié sur place. Une version de FEANOR qui ne les connaît pas les laisse là et montre les plans sans décor.
//!
//! **Annuler** : importer, enlever, déplacer un décor sont des étapes d'annulation (`Structure::decors3d`) ; les modèles sont
//! partagés (`Arc`), une étape ne les recopie pas.

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
/// Le dossier des modèles (`decors/<id>.glb`).
pub const DIR: &str = "decors";
const VERSION_FICHIER: u32 = 1;

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

/// Où le décor est posé dans l'espace : position (mètres), rotation autour de la verticale (degrés), échelle.
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
    pub fn matrice(&self) -> M4 {
        let e = if self.echelle.is_finite() && self.echelle > 0.0 { self.echelle } else { 1.0 };
        m::trs(self.position, [0.0, self.rotation_y, 0.0], [e; 3])
    }

    /// La place d'origine d'un décor importé : le bas de sa boîte posé sur le sol (`Monde::sol_y`), centré en x, tout entier
    /// derrière la feuille (z ≤ 0). Les mètres du glTF sont gardés tels quels.
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
}

impl Modele3d {
    pub fn new(scene: Arc<Scene3d>, octets: Arc<Vec<u8>>) -> Modele3d {
        static SUIVANT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Modele3d { uid: SUIVANT.fetch_add(1, std::sync::atomic::Ordering::Relaxed), scene, octets }
    }
}

/// Le décor d'une séquence.
#[derive(Clone)]
pub struct Decor3d {
    /// Son numéro : aussi celui de son fichier (`decors/<id>.glb`).
    pub id: u64,
    pub nom: String,
    /// Le nom du fichier d'origine (jamais modifié).
    pub origine: String,
    /// Ce qui a été allégé ou simplifié à l'import (« textures 4096 → 2048 », « simplifié : 6,2 M → 1,9 M triangles »).
    pub allege: Vec<String>,
    pub place: Place3d,
    pub aspect: Aspect,
    pub modele: Arc<Modele3d>,
}

impl std::fmt::Debug for Decor3d {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Decor3d({} « {} », {:?})", self.id, self.nom, self.place)
    }
}

/// Les décors du projet, par numéro de séquence. Dans `Structure` : l'annulation les rétablit avec le reste.
#[derive(Clone, Default, Debug)]
pub struct Decors3d {
    pub par_sequence: BTreeMap<u64, Decor3d>,
    /// Les entrées de `decors3d.json` qui n'ont pas pu être lues (fichier manquant ou abîmé) : réécrites telles quelles, leurs
    /// fichiers jamais effacés.
    gardees: Vec<serde_json::Value>,
    /// `decors3d.json` était illisible (abîmé, d'une version plus récente) : il n'est pas remplacé, rien n'est effacé de
    /// `decors/`.
    illisible: bool,
}

impl Decors3d {
    pub fn is_empty(&self) -> bool {
        self.par_sequence.is_empty() && self.gardees.is_empty()
    }
}

impl Document {
    /// Le décor 3D de la séquence du plan `index`.
    pub fn decor_du_plan(&self, index: usize) -> Option<&Decor3d> {
        (index < self.panels.len()).then(|| self.decors3d.par_sequence.get(&self.sequence_of(index))).flatten()
    }
}

// ------------------------------------------------------------------------------------------------ le fichier

#[derive(Serialize, Deserialize)]
struct Fichier {
    version: u32,
    decors: Vec<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
struct Entree {
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    allege: Vec<String>,
}

fn nom_de_fichier(id: u64) -> String {
    format!("{DIR}/{id}.glb")
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
    let mut decors: Vec<serde_json::Value> = d
        .par_sequence
        .iter()
        .map(|(&sequence, x)| {
            serde_json::to_value(Entree { sequence, id: x.id, nom: x.nom.clone(), origine: x.origine.clone(), fichier: nom_de_fichier(x.id), place: x.place, aspect: x.aspect, allege: x.allege.clone() })
                .map_err(|e| e.to_string())
        })
        .collect::<Result<_, _>>()?;
    decors.extend(d.gardees.iter().cloned());
    let mut out = serde_json::to_vec_pretty(&Fichier { version: VERSION_FICHIER, decors }).map_err(|e| e.to_string())?;
    out.push(b'\n');
    Ok(Some(out))
}

/// Lit les décors du projet `dir` (absent : aucun). Un fichier ou un modèle illisible n'empêche pas d'ouvrir le projet : on le
/// dit, le plan se montre sans ce décor, et rien n'est effacé.
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
    for v in f.decors {
        let lu = serde_json::from_value::<Entree>(v.clone()).map_err(|e| e.to_string()).and_then(|e| {
            if e.fichier != nom_de_fichier(e.id) {
                return Err(format!("fichier « {} » inattendu", e.fichier));
            }
            let octets = std::fs::read(dir.join(&e.fichier)).map_err(|err| format!("{} : {err}", e.fichier))?;
            let scene = lire_sans_paniquer(&octets)?.0;
            Ok((e, octets, scene))
        });
        match lu {
            Ok((e, octets, scene)) => {
                let modele = Arc::new(Modele3d::new(Arc::new(scene), Arc::new(octets)));
                out.par_sequence.insert(e.sequence, Decor3d { id: e.id, nom: e.nom, origine: e.origine, allege: e.allege, place: e.place, aspect: e.aspect, modele });
            }
            Err(e) => {
                warnings.push(format!("décor 3D illisible ({e}) : la séquence se montre sans lui, son fichier reste dans le projet."));
                out.gardees.push(v);
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
    d.par_sequence.values().map(|x| x.id).chain(d.gardees.iter().filter_map(|v| v["id"].as_u64()))
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
    for x in d.par_sequence.values() {
        let nom = format!("{}.glb", x.id);
        let chemin = dir.join(DIR).join(&nom);
        plan.utilises.insert(nom);
        // (Un modèle ne change jamais sous son nom : déjà là, il n'est pas réécrit.)
        if !chemin.exists() {
            plan.ecrire.push((chemin, x.modele.octets.clone()));
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

    /// Efface de `decors/` les modèles que plus rien ne cite (`<nombre>.glb` seulement). Rien si le fichier des décors n'a pas
    /// été lu en entier.
    pub(crate) fn cleanup(&self, dir: &Path) -> usize {
        if !self.actif || self.proteger {
            return 0;
        }
        let Ok(entries) = std::fs::read_dir(dir.join(DIR)) else { return 0 };
        let mut n = 0;
        for e in entries.flatten() {
            let nom = e.file_name().to_string_lossy().into_owned();
            let a_nous = nom.strip_suffix(".glb").is_some_and(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()));
            if a_nous && !self.utilises.contains(&nom) && std::fs::remove_file(e.path()).is_ok() {
                n += 1;
            }
        }
        n
    }
}

// ------------------------------------------------------------------------------------------- dans l'image du plan

/// Ce qui fait l'image du décor d'un plan : le modèle, sa pose dans le repère où on le regarde, l'œil, la feuille et le cadre,
/// l'aspect. Tout est là : l'image se calcule avec la clé et le modèle seuls.
#[derive(Clone, Copy, Debug)]
pub struct DecorCle {
    /// `Modele3d::uid`.
    pub modele: u64,
    pub pose: M4,
    pub oeil: V3,
    pub feuille: [usize; 2],
    pub cadre: [usize; 2],
    pub aspect: Aspect,
}

impl DecorCle {
    fn bits(&self) -> ([u32; 16], [u32; 3], [u32; 6]) {
        let a = &self.aspect;
        (self.pose.map(f32::to_bits), self.oeil.map(f32::to_bits), [a.facettes as u32, a.ciel as u32, a.brume as u32, a.ombres as u32, a.soleil_azimut.to_bits(), a.soleil_hauteur.to_bits()])
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
        self.modele == o.modele && self.feuille == o.feuille && self.cadre == o.cadre && self.bits() == o.bits()
    }
}

impl Hash for DecorCle {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.modele.hash(h);
        self.feuille.hash(h);
        self.cadre.hash(h);
        self.bits().hash(h);
    }
}

/// Numéro du dessin dérivé du décor (bit de poids fort mis : il ne tombe jamais sur un vrai dessin).
pub fn id_of(key: &DecorCle) -> CelId {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    "decor3d".hash(&mut h);
    key.hash(&mut h);
    h.finish() | (1 << 63)
}

/// Le numéro de la texture de la carte graphique qui montre le décor `modele` : un par décor (pas par position de la caméra :
/// la caméra qui bouge redessine la même texture, elle n'en crée pas).
pub fn id_texture(modele: u64) -> CelId {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    "decor3d texture".hash(&mut h);
    modele.hash(&mut h);
    h.finish() | (1 << 63)
}

/// L'élément du plan `index` qui montre son décor, à l'image où la caméra est `camera` (et `repere`, pour un plan en Espace
/// 3D) ; rien : la séquence n'a pas de décor, ou l'œil n'est pas devant la feuille.
pub fn item(doc: &Document, index: usize, camera: &crate::document::CameraSeen, repere: Option<&M4>) -> Option<crate::render::PlanItem> {
    let d = doc.decor_du_plan(index).filter(|d| !masque(d.modele.uid))?;
    let monde = Monde::of(doc);
    let (pose, oeil) = match repere {
        Some(r) => (m::mul(r, &d.place.matrice()), monde.eye_reference()),
        None => (d.place.matrice(), monde.eye_seen(camera)),
    };
    let key = DecorCle { modele: d.modele.uid, pose, oeil, feuille: [doc.width, doc.height], cadre: doc.frame, aspect: d.aspect };
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
    })
}

/// Les décors masqués sur cet ordinateur pour la séance (plus lourds que son budget, en attendant le choix d'Anthony dans la
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

/// Le modèle de numéro unique `uid` dans le projet.
pub fn modele(doc: &Document, uid: u64) -> Option<(Arc<Modele3d>, Aspect)> {
    doc.decors3d.par_sequence.values().find(|d| d.modele.uid == uid).map(|d| (d.modele.clone(), d.aspect))
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

/// La qualité des ombres d'un décor sur cette machine : la meilleure qui tient dans son budget (sans budget mesuré : pas
/// d'ombres, rien ne ralentit avant la mesure).
pub fn qualite(scene: &Scene3d) -> crate::decor3dbudget::Qualite {
    BUDGET.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map_or(crate::decor3dbudget::Qualite::Sans, |b| b.qualite_pour(scene.triangles() as u64))
}

/// Lissage des bords selon la taille de la feuille (le rendu par le processeur).
fn sur_echantillonnage(feuille: [usize; 2]) -> usize {
    if feuille[0] * feuille[1] <= 2_500_000 {
        2
    } else {
        1
    }
}

/// L'image du décor pour la clé `key` : la feuille entière, transparente hors du décor (opaque avec le ciel).
pub fn calculer(modele: &Modele3d, key: &DecorCle, fils: usize) -> Raster {
    let [w, h] = key.feuille;
    let Some(prise) = key.prise() else { return Raster::new(w, h) };
    // La carte graphique d'abord, si l'application en a une (la même image qu'à l'écran).
    {
        let mut g = CARTE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((device, queue, mt)) = g.as_mut() {
            let mt = mt.get_or_insert_with(|| crate::decor3dgpu::Moteur::new(device, queue));
            let d = [crate::decor3dgpu::Dessin { id: key.modele, scene: modele.scene.clone(), matrice: key.pose, aspect: key.aspect }];
            let q = if key.aspect.ombres { qualite(&modele.scene) } else { crate::decor3dbudget::Qualite::Sans };
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| crate::decor3dgpu::rendre_hors_ecran(device, queue, mt, &d, &prise, [w as u32, h as u32], key.aspect.ciel, q, 1)));
            if let Ok(Some((rgba, _))) = r {
                return Raster::from_rgba(w, h, &rgba);
            }
        }
    }
    let rgba = crate::decor3drendu::rendre(&modele.scene, &key.pose, &prise, [w, h], sur_echantillonnage(key.feuille), fils, &key.aspect);
    Raster::from_rgba(w, h, &rgba)
}

fn cel_of(raster: Raster, key: &DecorCle) -> Arc<Cel> {
    // (La version est le numéro du rendu : deux rendus différents n'ont jamais la même, la carte graphique le sait.)
    Arc::new(Cel { raster, vector: None, version: id_of(key), fond: None })
}

struct Travail {
    key: DecorCle,
    handle: std::thread::JoinHandle<Option<Raster>>,
}

#[derive(Default)]
struct Garde {
    finis: HashMap<CelId, Arc<Cel>>,
    ordre: VecDeque<CelId>,
    /// Le dernier rendu fini de chaque décor : ce que l'écran montre en attendant le suivant.
    dernier: HashMap<u64, Arc<Cel>>,
    en_cours: Option<Travail>,
    attente: Option<(DecorCle, Arc<Modele3d>)>,
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

fn sur_l_interface() -> bool {
    FIL_INTERFACE.get().is_some_and(|id| *id == std::thread::current().id())
}

fn garde<R>(f: impl FnOnce(&mut Garde) -> R) -> R {
    let mut g = GARDE.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(Garde::default))
}

fn ranger(g: &mut Garde, key: &DecorCle, cel: Arc<Cel>) {
    let id = id_of(key);
    if g.finis.insert(id, cel.clone()).is_none() {
        g.ordre.push_back(id);
    }
    while g.ordre.len() > GARDES {
        if let Some(old) = g.ordre.pop_front() {
            g.finis.remove(&old);
        }
    }
    g.dernier.insert(key.modele, cel);
}

fn coeurs() -> usize {
    (std::thread::available_parallelism().map_or(2, |n| n.get()) / 2).max(1)
}

fn lancer(g: &mut Garde) {
    if g.en_cours.is_some() {
        return;
    }
    let Some((key, modele)) = g.attente.take() else { return };
    let threads = coeurs();
    let handle = std::thread::spawn(move || std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| calculer(&modele, &key, threads))).ok());
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
    let (modele, _) = modele(doc, key.modele)?;
    if crate::fx::differe_actif() || sur_l_interface() {
        return garde(|g| {
            let deja = g.en_cours.as_ref().is_some_and(|t| t.key == *key) || g.attente.as_ref().is_some_and(|e| e.0 == *key);
            if !deja {
                g.attente = Some((*key, modele));
                lancer(g);
            }
            g.dernier.get(&key.modele).cloned().map(crate::fx::Shown::Fx)
        });
    }
    let cel = cel_of(calculer(&modele, key, coeurs().max(2)), key);
    garde(|g| ranger(g, key, cel.clone()));
    Some(crate::fx::Shown::Fx(cel))
}

/// Pose les rendus finis et lance le suivant ; vrai si un rendu vient d'arriver (il faut redessiner). Une fois par image
/// d'écran.
pub fn poll() -> bool {
    garde(|g| {
        if g.en_cours.as_ref().is_some_and(|t| t.handle.is_finished()) {
            if let Some(t) = g.en_cours.take() {
                if let Ok(Some(raster)) = t.handle.join() {
                    let cel = cel_of(raster, &t.key);
                    ranger(g, &t.key, cel);
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
        g.attente = None;
    });
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
        Ok(Analyse { nom, origine, glb: Arc::new(glb), scene: Arc::new(scene), mesures, cout_ms: None })
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

pub fn verdict(m: &Mesures, budget: &crate::decor3dbudget::BudgetMachine, cout_ms: Option<f32>) -> Verdict {
    use crate::decor3dbudget::grand_nombre;
    let mut raisons = Vec::new();
    let (mut lent, mut memoire) = (false, false);
    if m.triangles as u64 > budget.triangles {
        raisons.push(format!("{} triangles : cet ordinateur en dessine {} sans ralentir.", grand_nombre(m.triangles as u64), grand_nombre(budget.triangles)));
        lent = true;
    } else if let Some(ms) = cout_ms.filter(|ms| *ms > crate::decor3dbudget::BudgetMachine::ms_par_passe()) {
        raisons.push(format!("une image de ce décor prend {ms:.1} ms à la carte graphique : au-delà des {:.1} ms qu'on lui laisse.", crate::decor3dbudget::BudgetMachine::ms_par_passe()));
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
        Ok(Allege { glb: Arc::new(glb), scene: Arc::new(relue), mesures, notes })
    }))
    .unwrap_or_else(|_| Err("allègement interrompu".into()))
}
