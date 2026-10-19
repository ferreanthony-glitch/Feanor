//! Dessins dérivés des effets (13i) : le cache, et le calcul en arrière-plan.
//!
//! Un dessin passé par des effets est calculé une fois par jeu de paramètres (`fx::shown_id`) et
//! gardé ici, dans une réserve limitée en octets (et non en nombre : un dessin dérivé d'une
//! feuille 4K pleine pèse 33 Mo, un trait de quelques tuiles en pèse 1) : les plus anciens partent.
//! Règle « sans latence » (Anthony) : l'interface ne calcule jamais un effet pendant qu'on s'en
//! sert. L'affichage (`fx::display`) demande le calcul à un fil de fond (`worker_loop`, qui laisse
//! deux cœurs libres) et montre en attendant le dernier résultat du même dessin, ou le dessin sans
//! effet ; le résultat arrive dans le cache, et l'écran est redessiné. La lecture envoie en plus
//! les demandes des images qui viennent (`fx::prefetch`), par ordre de proximité ; une demande que
//! l'interface ne renouvelle pas (on a changé d'image) est abandonnée. L'export, les vignettes
//! d'enregistrement et les tests passent par `get_or_compute` : calcul bloquant, exact, une seule
//! fois même quand plusieurs fils le demandent ensemble.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};

use crate::document::{Cel, CelId};
use crate::fx::FxStep;
use crate::raster::{Raster, TILE_PIXELS};

/// Mémoire gardée pour les dessins dérivés (octets de pixels).
const CACHE_BYTES: usize = 768 << 20;
/// Nombre d'entrées au plus, quelle que soit leur taille.
const CACHE_ENTRIES: usize = 2048;
/// Dernier résultat gardé par dessin d'origine, pour l'afficher en attendant le suivant.
const LAST_MAX: usize = 12;
/// Copies de dessins d'origine gardées pour le fil de fond.
const SOURCES_MAX: usize = 8;
/// Une demande que l'interface n'a pas renouvelée depuis ce nombre d'images d'écran est abandonnée.
const MAX_AGE: u64 = 6;

struct Entry {
    version: u64,
    cel: Arc<Cel>,
    used: u64,
    bytes: usize,
}

/// Un calcul demandé au fil de fond.
pub(crate) struct Job {
    /// Numéro du dessin dérivé.
    pub id: CelId,
    /// Version du dessin d'origine.
    pub version: u64,
    /// Le dessin d'origine (une copie, partagée entre les demandes de la même version).
    pub src: Arc<Raster>,
    pub steps: Vec<FxStep>,
    /// Aperçu : réduction `1 << level` (0 : pleine résolution).
    pub level: u8,
    /// 0 : l'image affichée ; plus grand : plus loin dans l'avenir.
    pub prio: u32,
    /// Numéro d'image d'écran de la demande.
    pub epoch: u64,
    /// Dessin d'origine (pour `last`).
    pub origin: CelId,
    /// Ordre d'arrivée (départage deux demandes de même priorité) ; donné par `request`.
    pub seq: u64,
    /// Le projet pour lequel le calcul est demandé (`world`).
    pub world: u64,
}

struct Last {
    id: CelId,
    cel: Arc<Cel>,
    used: u64,
}

struct State {
    cache: BTreeMap<CelId, Entry>,
    bytes: usize,
    /// Calculs en cours : dessin dérivé → version.
    inflight: BTreeMap<CelId, u64>,
    queue: Vec<Job>,
    /// Dessin d'origine → son dernier dessin dérivé.
    last: BTreeMap<CelId, Last>,
    /// Dessin d'origine → (version, copie, dernier usage).
    sources: BTreeMap<CelId, (u64, Arc<Raster>, u64)>,
    /// (Dessin d'origine, niveau) → (version, copie réduite pour l'aperçu, dernier usage) : le même
    /// dessin sert à toutes les images d'une animation d'effet.
    small: BTreeMap<(CelId, u8), (u64, Arc<Raster>, u64)>,
    seq: u64,
}

impl State {
    const fn new() -> State {
        State {
            cache: BTreeMap::new(),
            bytes: 0,
            inflight: BTreeMap::new(),
            queue: Vec::new(),
            last: BTreeMap::new(),
            sources: BTreeMap::new(),
            small: BTreeMap::new(),
            seq: 0,
        }
    }

    /// Le dessin dérivé `id` est-il gardé (à cette version) ou déjà en cours de calcul ?
    fn has(&self, id: CelId, version: u64) -> bool {
        self.cache.get(&id).is_some_and(|e| e.version == version) || self.inflight.get(&id) == Some(&version)
    }
}

static STATE: Mutex<State> = Mutex::new(State::new());
/// Un calcul vient de finir (réveille ceux qui l'attendaient).
static DONE: Condvar = Condvar::new();
/// Une demande vient d'arriver (réveille le fil de fond).
static WAKE: Condvar = Condvar::new();
static TICK: AtomicU64 = AtomicU64::new(0);
static EPOCH: AtomicU64 = AtomicU64::new(0);
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// Change quand on oublie tout (`forget_all`).
static WORLD: AtomicU64 = AtomicU64::new(0);

pub(crate) fn world() -> u64 {
    WORLD.load(Ordering::Relaxed)
}
static WORKER: OnceLock<()> = OnceLock::new();
static REPAINT: OnceLock<eframe::egui::Context> = OnceLock::new();

fn lock() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

fn tick() -> u64 {
    TICK.fetch_add(1, Ordering::Relaxed)
}

/// Octets de pixels d'un dessin (les tuiles dessinées seulement).
pub(crate) fn cel_bytes(c: &Cel) -> usize {
    c.raster.tiles.iter().flatten().count() * TILE_PIXELS * 4
}

/// À chaque image d'écran : fait avancer l'horloge des demandes, et garde de quoi redessiner
/// l'écran quand le fil de fond a fini.
pub(crate) fn begin_frame(ctx: &eframe::egui::Context) {
    REPAINT.get_or_init(|| ctx.clone());
    EPOCH.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn epoch() -> u64 {
    EPOCH.load(Ordering::Relaxed)
}

/// Nombre de calculs de fond finis depuis le début (une vignette incomplète se refait quand il
/// change).
pub(crate) fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

/// Fait comme si un calcul de fond venait de finir (réveille les vignettes qui attendent).
pub(crate) fn bump_generation() {
    GENERATION.fetch_add(1, Ordering::Relaxed);
}

/// Le dessin dérivé `id` de la version `version`, s'il est gardé.
pub(crate) fn get(id: CelId, version: u64) -> Option<Arc<Cel>> {
    let mut st = lock();
    let e = st.cache.get_mut(&id).filter(|e| e.version == version)?;
    e.used = tick();
    Some(e.cel.clone())
}

/// Le dessin dérivé `id` gardé, quelle que soit sa version (avec sa version) : le fond sous le
/// trait montre l'ancien en attendant le nouveau (`fx::fond_shown`).
pub(crate) fn get_any(id: CelId) -> Option<(u64, Arc<Cel>)> {
    let mut st = lock();
    let e = st.cache.get_mut(&id)?;
    e.used = tick();
    Some((e.version, e.cel.clone()))
}

/// Range un dessin dérivé arrivé d'ailleurs (le fond calculé en arrière-plan, `fx::fond_poll`).
pub(crate) fn put(id: CelId, version: u64, cel: Arc<Cel>) {
    let dropped = insert(&mut lock(), id, version, cel, None);
    drop(dropped);
}

/// Range un dessin dérivé ; renvoie ceux que la réserve a dû lâcher (à libérer hors du verrou).
fn insert(st: &mut State, id: CelId, version: u64, cel: Arc<Cel>, origin: Option<CelId>) -> Vec<Arc<Cel>> {
    insert_within(st, id, version, cel, origin, CACHE_BYTES)
}

/// `insert`, la réserve limitée à `limit` octets.
fn insert_within(st: &mut State, id: CelId, version: u64, cel: Arc<Cel>, origin: Option<CelId>, limit: usize) -> Vec<Arc<Cel>> {
    let mut dropped = Vec::new();
    let used = tick();
    let bytes = cel_bytes(&cel);
    if let Some(old) = st.cache.insert(id, Entry { version, cel: cel.clone(), used, bytes }) {
        st.bytes -= old.bytes;
        dropped.push(old.cel);
    }
    st.bytes += bytes;
    while (st.bytes > limit || st.cache.len() > CACHE_ENTRIES) && st.cache.len() > 1 {
        let Some(old) = st.cache.iter().filter(|(k, _)| **k != id).min_by_key(|(_, e)| e.used).map(|(k, _)| *k) else { break };
        if let Some(e) = st.cache.remove(&old) {
            st.bytes -= e.bytes;
            dropped.push(e.cel);
        }
    }
    if let Some(origin) = origin {
        if let Some(prev) = st.last.insert(origin, Last { id, cel, used }) {
            dropped.push(prev.cel);
        }
        while st.last.len() > LAST_MAX {
            let Some(old) = st.last.iter().min_by_key(|(_, l)| l.used).map(|(k, _)| *k) else { break };
            if let Some(l) = st.last.remove(&old) {
                dropped.push(l.cel);
            }
        }
    }
    dropped
}

/// Retire le calcul en cours de `id` quand on en sort (même par une erreur) et réveille ceux qui
/// l'attendent.
struct Inflight {
    id: CelId,
    version: u64,
}

impl Drop for Inflight {
    fn drop(&mut self) {
        let mut st = lock();
        if st.inflight.get(&self.id) == Some(&self.version) {
            st.inflight.remove(&self.id);
        }
        drop(st);
        DONE.notify_all();
    }
}

/// Le dessin dérivé `id` de la version `version` : gardé, ou calculé par `make` (bloquant, hors
/// verrou). Si un autre fil le calcule déjà, on l'attend au lieu de le refaire.
pub(crate) fn get_or_compute(id: CelId, version: u64, origin: Option<CelId>, make: impl FnOnce() -> Cel) -> Arc<Cel> {
    let mut st = lock();
    loop {
        if let Some(e) = st.cache.get_mut(&id).filter(|e| e.version == version) {
            e.used = tick();
            return e.cel.clone();
        }
        if st.inflight.get(&id) == Some(&version) {
            st = DONE.wait(st).unwrap_or_else(|e| e.into_inner());
            continue;
        }
        break;
    }
    st.inflight.insert(id, version);
    drop(st);
    let _guard = Inflight { id, version };
    let cel = Arc::new(make());
    let dropped = insert(&mut lock(), id, version, cel.clone(), origin);
    drop(dropped);
    cel
}

/// Remplace le dessin dérivé `id` (sans changer sa version) par ce que `f` fait de l'ancien (le
/// fond refait tuile par tuile pendant qu'on peint, `fx::fond_patch`).
pub(crate) fn update(id: CelId, f: impl FnOnce(&Arc<Cel>) -> Arc<Cel>) {
    let mut st = lock();
    let Some(e) = st.cache.get_mut(&id) else { return };
    let new = f(&e.cel);
    let bytes = cel_bytes(&new);
    let old_bytes = e.bytes;
    let old = std::mem::replace(&mut e.cel, new);
    e.bytes = bytes;
    st.bytes = st.bytes + bytes - old_bytes;
    drop(st);
    drop(old);
}

/// Le dessin d'origine `cel` (version `version`) copié pour le fil de fond, une fois par version.
pub(crate) fn source(origin: CelId, cel: &Cel) -> Arc<Raster> {
    if let Some((v, a, used)) = lock().sources.get_mut(&origin) {
        if *v == cel.version {
            *used = tick();
            return a.clone();
        }
    }
    let copy = Arc::new(cel.raster.clone());
    let mut st = lock();
    let mut dropped = Vec::new();
    if let Some(old) = st.sources.insert(origin, (cel.version, copy.clone(), tick())) {
        dropped.push(old.1);
    }
    while st.sources.len() > SOURCES_MAX {
        let Some(old) = st.sources.iter().min_by_key(|(_, s)| s.2).map(|(k, _)| *k) else { break };
        if let Some(s) = st.sources.remove(&old) {
            dropped.push(s.1);
        }
    }
    drop(st);
    drop(dropped);
    copy
}

/// Le dessin d'origine de `job`, réduit pour l'aperçu `job.level` (calculé une fois par version du
/// dessin, par le fil de fond).
fn reduced_source(job: &Job) -> Arc<Raster> {
    let key = (job.origin, job.level);
    if let Some((v, a, used)) = lock().small.get_mut(&key) {
        if *v == job.version {
            *used = tick();
            return a.clone();
        }
    }
    let small = Arc::new(crate::fxproxy::downscale(&job.src, job.level));
    let mut st = lock();
    let mut dropped = Vec::new();
    if let Some(old) = st.small.insert(key, (job.version, small.clone(), tick())) {
        dropped.push(old.1);
    }
    while st.small.len() > SOURCES_MAX {
        let Some(old) = st.small.iter().min_by_key(|(_, s)| s.2).map(|(k, _)| *k) else { break };
        if let Some(s) = st.small.remove(&old) {
            dropped.push(s.1);
        }
    }
    drop(st);
    drop(dropped);
    small
}

/// Le dernier dessin dérivé du dessin d'origine `origin` (numéro sous lequel il est gardé, dessin).
pub(crate) fn fallback(origin: CelId) -> Option<(CelId, Arc<Cel>)> {
    let mut st = lock();
    let l = st.last.get_mut(&origin)?;
    l.used = tick();
    Some((l.id, l.cel.clone()))
}

/// Demande le calcul de `job` au fil de fond (sans attendre). Rien si le dessin est déjà gardé ou
/// en cours de calcul ; une demande déjà en file voit sa priorité remontée.
pub(crate) fn request(job: Job) {
    if !enqueue(&mut lock(), job) {
        return;
    }
    WORKER.get_or_init(|| {
        let _ = std::thread::Builder::new().name("effets".into()).spawn(worker_loop);
    });
    WAKE.notify_one();
}

/// Met `job` en file. Faux (rien à faire) si le dessin est déjà gardé ou en cours de calcul ; une
/// demande déjà en file voit sa priorité remontée (ou est remplacée si la version a changé).
fn enqueue(st: &mut State, mut job: Job) -> bool {
    if st.has(job.id, job.version) {
        return false;
    }
    if let Some(q) = st.queue.iter_mut().find(|q| q.id == job.id) {
        if q.version == job.version {
            q.prio = q.prio.min(job.prio);
            q.epoch = job.epoch;
        } else {
            job.seq = q.seq;
            *q = job;
        }
        return false;
    }
    st.seq += 1;
    job.seq = st.seq;
    st.queue.push(job);
    true
}

/// La demande à faire maintenant, au temps `now` : celle de plus petite priorité (puis la plus
/// ancienne). Celles que l'interface n'a pas renouvelées depuis `MAX_AGE` images d'écran, ou dont
/// le résultat est déjà là, sont abandonnées. Le calcul est noté en cours.
fn take_next(st: &mut State, now: u64) -> Option<Job> {
    st.queue.retain(|j| j.epoch + MAX_AGE >= now);
    loop {
        let best = st.queue.iter().enumerate().min_by_key(|(_, j)| (j.prio, j.seq)).map(|(i, _)| i)?;
        let job = st.queue.swap_remove(best);
        if st.has(job.id, job.version) {
            continue;
        }
        st.inflight.insert(job.id, job.version);
        return Some(job);
    }
}

/// Un dessin dérivé est-il prêt ou demandé ? (Pour ne pas le redemander à chaque image.)
#[cfg(test)]
pub(crate) fn pending() -> usize {
    let st = lock();
    st.queue.len() + st.inflight.len()
}

/// Attend que le fil de fond ait tout fini (tests).
#[cfg(test)]
pub(crate) fn settle() {
    // Attente sur la fin d'un calcul (la condition `DONE`, réveillée à chaque calcul fini), pas sur
    // une durée : le délai de 20 ms ne sert qu'à revérifier, jamais à décider.
    let start = std::time::Instant::now();
    let mut st = lock();
    while st.queue.len() + st.inflight.len() > 0 {
        assert!(start.elapsed() < std::time::Duration::from_secs(60), "le fil de fond n'a pas fini");
        st = DONE.wait_timeout(st, std::time::Duration::from_millis(20)).unwrap_or_else(|e| e.into_inner()).0;
    }
}

/// Essais : le cache des effets, son fil de fond, son horloge (`begin_frame`), l'état de l'aperçu
/// (`fx::draft_level`) et les vignettes qui attendent (`generation`) sont COMMUNS à tous les essais
/// d'un même processus, qui tournent ensemble. Un essai qui compte sur ces états (ceux de
/// `fxcache_tests.rs`, la mesure d'inactivité, les mesures de `fxperf_tests.rs`) ne peut pas les
/// partager avec un autre qui fait tourner l'application (`begin_frame` fait vieillir et abandonner
/// ses demandes), ouvre un projet (`forget_all` vide tout) ou calcule des effets. Deux rôles, pris
/// pour toute la durée de l'essai (le verrou est gardé par le fil, rendu quand il s'arrête) :
/// - `lire()` : fait tourner l'application ou calcule des effets, sans rien exiger de l'état commun
///   (pris tout seul par `App::sans_fenetre`) ; les lecteurs tournent ensemble ;
/// - `ecrire()` : exige l'état commun pour lui seul ; il attend la fin des lecteurs et des autres
///   écrivains, et personne ne démarre pendant ce temps.
#[cfg(test)]
pub(crate) mod monde {
    use std::cell::RefCell;
    use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

    static MONDE: RwLock<()> = RwLock::new(());

    enum Tenu {
        Lecture(#[allow(dead_code)] RwLockReadGuard<'static, ()>),
        Ecriture(#[allow(dead_code)] RwLockWriteGuard<'static, ()>),
    }

    thread_local! {
        static TENU: RefCell<Option<Tenu>> = const { RefCell::new(None) };
    }

    /// Cet essai peut tourner avec d'autres (rien ne le gêne, sauf un écrivain, qu'il attend).
    pub(crate) fn lire() {
        TENU.with(|t| {
            let mut t = t.borrow_mut();
            if t.is_none() {
                *t = Some(Tenu::Lecture(MONDE.read().unwrap_or_else(|e| e.into_inner())));
            }
        });
    }

    /// Cet essai veut l'état commun pour lui seul.
    pub(crate) fn ecrire() {
        let neuf = TENU.with(|t| {
            let mut t = t.borrow_mut();
            if matches!(*t, Some(Tenu::Ecriture(_))) {
                return false;
            }
            // (Un fil qui lisait déjà — `--test-threads=1` fait tourner tous les essais sur le même — rend
            // sa lecture avant de prendre l'écriture.)
            *t = None;
            *t = Some(Tenu::Ecriture(MONDE.write().unwrap_or_else(|e| e.into_inner())));
            true
        });
        if !neuf {
            return;
        }
        // Page blanche : ce que les essais d'avant ont laissé (résultats gardés, demandes, calculs en
        // cours, 250 ms d'aperçu devant une application qui défilait) n'est pas à cet essai.
        super::forget_all();
        crate::fx::reset_draft();
        super::settle();
    }
}

/// Vide la réserve (mesures).
#[cfg(test)]
pub(crate) fn clear() {
    forget_all();
}

/// Oublie tout (un autre projet s'ouvre : ses dessins reprennent les mêmes numéros et les mêmes
/// versions, ils ne doivent pas retrouver les dessins dérivés de l'ancien). Les calculs de fond en
/// cours finissent mais leur résultat est jeté.
pub(crate) fn forget_all() {
    WORLD.fetch_add(1, Ordering::Relaxed);
    let mut st = lock();
    let old = (
        std::mem::take(&mut st.cache),
        std::mem::take(&mut st.last),
        std::mem::take(&mut st.sources),
        std::mem::take(&mut st.small),
    );
    st.bytes = 0;
    st.queue.clear();
    drop(st);
    drop(old);
}

fn worker_loop() {
    // Le fil de fond laisse deux cœurs à l'interface et à la carte graphique.
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    crate::render::set_thread_cap(cores.saturating_sub(2).max(1));
    loop {
        let job = {
            let mut st = lock();
            loop {
                if let Some(job) = take_next(&mut st, epoch()) {
                    break job;
                }
                st = WAKE.wait(st).unwrap_or_else(|e| e.into_inner());
            }
        };
        let _guard = Inflight { id: job.id, version: job.version };
        // Un effet qui plante ne doit pas tuer le fil ni être redemandé sans fin : on garde alors
        // le dessin sans effet.
        let raster = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if job.level == 0 {
                crate::fx::apply_scaled(&job.src, &job.steps, 0)
            } else {
                crate::fx::apply_scaled(&reduced_source(&job), &job.steps, job.level)
            }
        }))
        .unwrap_or_else(|_| crate::fxproxy::downscale(&job.src, job.level));
        let cel = Arc::new(Cel { raster, vector: None, version: job.version, fond: None });
        // (Un autre projet s'est ouvert pendant le calcul : le résultat ne lui appartient pas.)
        if job.world == world() {
            let dropped = insert(&mut lock(), job.id, job.version, cel, Some(job.origin));
            drop(dropped);
        }
        GENERATION.fetch_add(1, Ordering::Relaxed);
        if let Some(ctx) = REPAINT.get() {
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
#[path = "fxcache_tests.rs"]
mod tests;

thread_local! {
    /// Mode « doux » (vignettes) : `fx::shown` ne bloque pas ; l'état dit si un dessin a manqué.
    static SOFT: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// Exécute `f` en mode doux : les effets non encore calculés sont demandés au fil de fond et
/// remplacés par leur dernier résultat ou le dessin sans effet. Renvoie aussi si tout était prêt.
pub(crate) fn soft_scope<R>(f: impl FnOnce() -> R) -> (R, bool) {
    let before = SOFT.with(|s| s.replace(Some(false)));
    let r = f();
    let incomplete = SOFT.with(|s| s.replace(before)).unwrap_or(false);
    (r, !incomplete)
}

pub(crate) fn soft_active() -> bool {
    SOFT.with(|s| s.get().is_some())
}

pub(crate) fn mark_incomplete() {
    SOFT.with(|s| s.set(Some(true)));
}
