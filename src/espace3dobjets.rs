//! Les objets de l'Espace 3D dans l'image finale (`docs/ESPACE3D.md`, « Les objets dans l'image »).
//!
//! Les objets d'un plan (cube, mannequin, modèle…, plus le décor de sa séquence) sont rendus par le rasteriseur de la scène
//! (`scene3drender.rs`), vus de l'œil de la caméra du plan à l'image demandée : la feuille telle que cet œil la voit, sans
//! zoom ni rotation, comme pour un calque en profondeur. La caméra du plan ajoute ensuite son zoom, sa rotation, sa bascule et
//! son panoramique à tout le plan. L'image des objets est donc un élément du plan comme un autre, posé **sous tous les
//! calques** (`render::PlanItem` à `FondPart::Objets`), et un calque d'ajustement placé au-dessus la couvre.
//!
//! Le calcul dépend de la scène (sa version), de la feuille, du cadre et de l'œil : il change quand la caméra se déplace.
//! - À l'écran (`fx::differe`), il part sur un autre fil (la moitié des cœurs) et l'écran garde, en attendant, le dernier
//!   rendu du plan ; `poll` pose le nouveau et demande un affichage. Jamais d'attente sur le fil de l'interface.
//! - Partout ailleurs (export, enregistrement, essais), le rendu est fait tout de suite, exact.
//!
//! Les rendus sont gardés (les derniers : une dizaine). Ils ne sont jamais enregistrés dans le projet.

use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crate::blend::BlendMode;
use crate::document::{CameraSeen, Cel, CelId, Document};
use crate::espace3d::Monde;
use crate::fx::Shown;
use crate::model3d::Model3d;
use crate::raster::Raster;
use crate::render::{FondPart, PlanItem};
use crate::scene3d::Scenes3d;

/// Ce qui fait l'image des objets d'un plan : la scène (sa version), la feuille et le cadre, l'œil de la caméra (les
/// bits de ses trois coordonnées, en mètres).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ObjetsCle {
    pub panel: u64,
    pub seq: u64,
    pub rev: u64,
    pub eye: [u32; 3],
    pub stage: [usize; 2],
    pub frame: [usize; 2],
}

impl ObjetsCle {
    pub fn eye(&self) -> [f32; 3] {
        self.eye.map(f32::from_bits)
    }
}

/// Numéro du dessin dérivé des objets (bit de poids fort mis : il ne tombe jamais sur un vrai dessin).
pub fn id_of(key: &ObjetsCle) -> CelId {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    "objets3d".hash(&mut h);
    key.hash(&mut h);
    h.finish() | (1 << 63)
}

/// La clé des objets du plan `index` à l'image où la caméra est `camera`, ou rien : aucun objet à montrer (pas de scène pour
/// ce plan, « Objets dans l'image » éteint — un plan d'avant l'Espace 3D —, ou aucun objet visible).
pub fn key_for(doc: &Document, index: usize, camera: &CameraSeen) -> Option<ObjetsCle> {
    let panel = doc.panels.get(index)?;
    if !doc.scenes3d.plan(panel.id).is_some_and(|p| p.in_image) {
        return None;
    }
    let seq = doc.sequence_of(index);
    if !doc.scenes3d.visible_objects(panel.id, seq).iter().any(|(o, _)| !o.hidden) {
        return None;
    }
    let eye = Monde::of(doc).eye_seen(camera);
    Some(ObjetsCle { panel: panel.id, seq, rev: doc.scenes3d.rev, eye: eye.map(f32::to_bits), stage: [doc.width, doc.height], frame: doc.frame })
}

/// L'élément du plan qui montre les objets : à poser tout en bas, avant le premier calque.
pub fn item(key: &ObjetsCle) -> PlanItem {
    PlanItem { cel: id_of(key), opacity: 255, tint: None, transform: None, blend: BlendMode::Normal, fx: Vec::new(), ctg: None, fond: Some(FondPart::Objets(*key)), pre: None }
}

/// Lissage des bords selon la taille de la feuille : le rendu d'une image du film se refait souvent.
fn supersampling(stage: [usize; 2]) -> usize {
    if stage[0] * stage[1] <= 2_500_000 {
        2
    } else {
        1
    }
}

/// Le rendu des objets : le dessin de la feuille.
fn compute(scenes: &Scenes3d, models: &HashMap<u64, Arc<Model3d>>, key: &ObjetsCle, threads: usize) -> Raster {
    let plate = crate::scene3drender::render_plate_eye(scenes, key.panel, key.seq, models, key.stage, key.frame, key.eye(), supersampling(key.stage), threads);
    crate::scene3drender::plate_raster(&plate, key.stage)
}

fn cel_of(raster: Raster, key: &ObjetsCle) -> Arc<Cel> {
    // (La version est le numéro du rendu : deux rendus différents n'ont jamais la même, la carte graphique le sait.)
    Arc::new(Cel { raster, vector: None, version: id_of(key), fond: None })
}

// ------------------------------------------------------------------------------------ les rendus gardés

/// Ce que le fil de fond a besoin pour un rendu : une copie de la scène, des modèles.
struct Entrees {
    key: ObjetsCle,
    scenes: Scenes3d,
    models: HashMap<u64, Arc<Model3d>>,
}

struct Travail {
    key: ObjetsCle,
    handle: JoinHandle<Raster>,
}

#[derive(Default)]
struct Garde {
    /// Rendus finis, par numéro.
    finis: HashMap<u64, Arc<Cel>>,
    ordre: VecDeque<u64>,
    /// Le dernier rendu fini de chaque plan : ce que l'écran montre en attendant le suivant.
    dernier: HashMap<u64, Arc<Cel>>,
    en_cours: Option<Travail>,
    /// Le prochain rendu à lancer (le plus récent demandé gagne).
    attente: Option<Entrees>,
    /// Les versions de scène dont un rendu vient d'arriver et que personne n'a encore signalées à l'écran (`poll`).
    arrives: Vec<u64>,
}

const GARDES: usize = 12;

static GARDE: Mutex<Option<Garde>> = Mutex::new(None);

fn garde<R>(f: impl FnOnce(&mut Garde) -> R) -> R {
    let mut g = GARDE.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(Garde::default))
}

fn ranger(g: &mut Garde, key: &ObjetsCle, cel: Arc<Cel>) {
    let id = id_of(key);
    if g.finis.insert(id, cel.clone()).is_none() {
        g.ordre.push_back(id);
    }
    while g.ordre.len() > GARDES {
        if let Some(old) = g.ordre.pop_front() {
            g.finis.remove(&old);
        }
    }
    g.dernier.insert(key.panel, cel);
}

fn coeurs() -> usize {
    (std::thread::available_parallelism().map_or(2, |n| n.get()) / 2).max(1)
}

fn lancer(g: &mut Garde) {
    if g.en_cours.is_some() {
        return;
    }
    let Some(e) = g.attente.take() else { return };
    let threads = coeurs();
    let key = e.key;
    let handle = std::thread::spawn(move || compute(&e.scenes, &e.models, &e.key, threads));
    g.en_cours = Some(Travail { key, handle });
}

/// Les objets tels que la carte graphique ou le processeur les composent : le rendu s'il est prêt ; à l'écran
/// (`fx::differe`), sinon le dernier rendu du plan en attendant (le nouveau part sur un autre fil) ; ailleurs, calculé tout
/// de suite.
pub fn shown(doc: &Document, key: &ObjetsCle) -> Option<Shown<'static>> {
    let id = id_of(key);
    if let Some(cel) = garde(|g| g.finis.get(&id).cloned()) {
        return Some(Shown::Fx(cel));
    }
    // Un projet qui n'a pas la scène de cette clé (une copie sans scène) ne la rend pas : elle ne range rien sous son numéro.
    if doc.scenes3d.rev != key.rev {
        return garde(|g| g.dernier.get(&key.panel).cloned()).map(Shown::Fx);
    }
    if crate::fx::differe_actif() {
        garde(|g| {
            let deja = g.en_cours.as_ref().is_some_and(|t| t.key == *key) || g.attente.as_ref().is_some_and(|e| e.key == *key);
            if !deja {
                g.attente = Some(Entrees { key: *key, scenes: doc.scenes3d.clone(), models: doc.models.clone() });
                lancer(g);
            }
            g.dernier.get(&key.panel).cloned().map(Shown::Fx)
        })
    } else {
        let raster = compute(&doc.scenes3d, &doc.models, key, coeurs().max(2));
        let cel = cel_of(raster, key);
        garde(|g| ranger(g, key, cel.clone()));
        Some(Shown::Fx(cel))
    }
}

/// Pose les rendus finis et lance le suivant ; vrai si un rendu de la scène de version `rev` (celle du projet affiché) est
/// arrivé : il faut redessiner. À appeler une fois par image d'écran.
pub fn poll(rev: u64) -> bool {
    garde(|g| {
        if g.en_cours.as_ref().is_some_and(|t| t.handle.is_finished()) {
            if let Some(t) = g.en_cours.take() {
                if let Ok(raster) = t.handle.join() {
                    let cel = cel_of(raster, &t.key);
                    ranger(g, &t.key, cel);
                    crate::fx::bump_fond_epoch();
                    g.arrives.push(t.key.rev);
                    if g.arrives.len() > 64 {
                        g.arrives.remove(0);
                    }
                }
            }
        }
        lancer(g);
        let had = g.arrives.contains(&rev);
        g.arrives.retain(|r| *r != rev);
        had
    })
}

/// Un rendu de la scène de version `rev` est en cours ou attend son tour.
pub fn pending(rev: u64) -> bool {
    garde(|g| g.en_cours.as_ref().is_some_and(|t| t.key.rev == rev) || g.attente.as_ref().is_some_and(|e| e.key.rev == rev))
}

/// Un autre projet s'ouvre : les rendus de l'ancien ne sont pas les siens.
pub fn forget_all() {
    garde(|g| {
        g.finis.clear();
        g.ordre.clear();
        g.dernier.clear();
        g.attente = None;
        // (Le rendu en cours finit tout seul ; il sera rangé sous sa clé, que plus rien ne demande.)
    });
}

/// Essais : attend la fin des rendus de fond et les pose.
#[cfg(test)]
pub(crate) fn wait_idle() {
    let start = std::time::Instant::now();
    while garde(|g| g.en_cours.is_some() || g.attente.is_some()) {
        poll(0);
        std::thread::sleep(std::time::Duration::from_millis(2));
        assert!(start.elapsed() < std::time::Duration::from_secs(60), "le rendu des objets ne finit pas");
    }
}

/// Essais : oublie tout (un essai ne voit pas les rendus d'un autre).
#[cfg(test)]
pub(crate) fn clear_for_tests() {
    wait_idle();
    garde(|g| *g = Garde::default());
}
