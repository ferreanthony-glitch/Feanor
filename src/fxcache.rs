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
use crate::vector::VectorDrawing;

/// Mémoire gardée pour les dessins dérivés (octets de pixels), avant tout réglage.
const CACHE_BYTES: usize = 768 << 20;
/// La réserve en vigueur : `CACHE_BYTES`, ou ce que Aide > Diagnostic des performances ou les
/// Préférences ont réglé (`set_limit`).
static LIMIT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(CACHE_BYTES);

/// La réserve en vigueur, en octets (essais).
#[cfg(test)]
pub(crate) fn limit() -> usize {
    LIMIT.load(Ordering::Relaxed)
}

/// La réserve en vigueur, en octets : les onglets cachés n'en gardent pas plus, tous ensemble (`onglets.rs`).
pub(crate) fn limite_octets() -> usize {
    LIMIT.load(Ordering::Relaxed)
}

/// Règle la réserve à `bytes` octets ; les plus anciens dessins partent tout de suite si elle est
/// déjà dépassée.
pub(crate) fn set_limit(bytes: usize) {
    let bytes = bytes.max(1 << 20);
    if LIMIT.swap(bytes, Ordering::Relaxed) == bytes {
        return;
    }
    let mut dropped = Vec::new();
    {
        let mut st = lock();
        while st.bytes > bytes && !st.cache.is_empty() {
            let Some(old) = st.cache.iter().min_by_key(|(_, e)| e.used).map(|(k, _)| *k) else { break };
            if let Some(e) = st.cache.remove(&old) {
                st.bytes -= e.bytes;
                dropped.push(e.cel);
            }
        }
    }
    drop(dropped);
}
/// Nombre d'entrées au plus, quelle que soit leur taille.
const CACHE_ENTRIES: usize = 2048;
/// Dernier résultat gardé par dessin d'origine, pour l'afficher en attendant le suivant : au plus ce nombre de dessins, et au plus
/// LAST_OCTETS octets de pixels (la plupart sont aussi dans la réserve : ce ne sont alors que des références). Douze seulement, c'était
/// trop peu (retour d'Anthony, 2026-10-24 : l'aplat disparaît pendant qu'il retouche) : les vignettes d'une planche ou d'une bande de
/// montage calculent chacune leur remplissage, et treize dessins plus récents faisaient partir le résultat d'avant du dessin qu'on
/// retouchait, qui ne montrait plus rien le temps du recalcul.
const LAST_MAX: usize = 256;
const LAST_OCTETS: usize = 192 << 20;
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
    /// Le dessin vectoriel d'origine, pour les effets du trait (`fxtrait.rs`) ; rien sans eux.
    pub vector: Option<Arc<VectorDrawing>>,
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
    /// Calque d'effet d'ajustement (`calqueffet.rs`) : ce qui est dessous, à composer puis passer par
    /// les effets (`steps` reste vide, `src` est vide). Rien : le dessin `src` passé par `steps`.
    pub base: Option<Arc<crate::calqueffet::Snapshot>>,
    /// Remplissage au griffonnage (`griffonnage.rs`) : les dessins des sources (les traits qui ferment les zones) et la
    /// fermeture des trous. `src` est alors le dessin de griffonnages, et le dessin dérivé est son remplissage, passé par
    /// `steps`. Jamais en aperçu réduit (`level` 0).
    pub griffon: Option<(Vec<Arc<Raster>>, u8)>,
}

struct Last {
    id: CelId,
    cel: Arc<Cel>,
    used: u64,
    bytes: usize,
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
    /// Dessin d'origine → (version, copie du dessin vectoriel, dernier usage).
    vsources: BTreeMap<CelId, (u64, Arc<VectorDrawing>, u64)>,
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
            vsources: BTreeMap::new(),
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

thread_local! {
    /// Le « monde » qu'un fil de fond a reçu avec son travail (vignettes, `avec_monde`), à la place du monde
    /// du moment : une vignette demandée pour un onglet qui n'est plus affiché ne demande rien au nouveau.
    static MONDE_DU_FIL: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

/// Le monde des dessins dérivés : change à chaque `forget_all` et à chaque changement d'onglet. Un travail
/// de fond demandé dans un monde dont le résultat arrive dans un autre est jeté.
pub(crate) fn world() -> u64 {
    MONDE_DU_FIL.with(std::cell::Cell::get).unwrap_or_else(|| WORLD.load(Ordering::Relaxed))
}

/// Ce fil travaille pour un monde qui n'est plus le courant : ce qu'il lit ici ne lui appartient pas.
fn monde_perime() -> bool {
    MONDE_DU_FIL.with(std::cell::Cell::get).is_some_and(|w| w != WORLD.load(Ordering::Relaxed))
}

/// Exécute `f` pour le monde `w` (celui où la demande est partie, sur le fil de l'interface) : les
/// demandes qu'elle fait portent ce monde, et tant que le monde courant est un autre, elle ne lit rien
/// du cache et ne demande aucun calcul.
pub(crate) fn avec_monde<R>(w: u64, f: impl FnOnce() -> R) -> R {
    struct Garde(Option<u64>);
    impl Drop for Garde {
        fn drop(&mut self) {
            MONDE_DU_FIL.with(|m| m.set(self.0));
        }
    }
    let _garde = Garde(MONDE_DU_FIL.with(|m| m.replace(Some(w))));
    f()
}
static WORKER: OnceLock<()> = OnceLock::new();
static REPAINT: OnceLock<eframe::egui::Context> = OnceLock::new();

fn lock() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

fn tick() -> u64 {
    TICK.fetch_add(1, Ordering::Relaxed)
}

/// Le cache garde-t-il des dessins dérivés ?
pub(crate) fn non_vide() -> bool {
    !lock().cache.is_empty()
}

/// Range les pixels des dessins dérivés que personne n'a demandés depuis au moins `repos` usages (chantier « mémoire d'une séance » : 41
/// dessins dérivés de la taille de la feuille tenaient 790 Mo de pixels après une longue séance, presque tous inutiles à l'écran). Les
/// tuiles se rouvrent toutes seules à la lecture (`Tile`) : un dessin dérivé rangé se lit comme un autre. Pour avoir un `&mut` sur un
/// dessin que le cache est seul à tenir, on le sort du cache le temps du rangement (hors verrou : l'interface ne l'attend jamais) ; un
/// dessin que quelqu'un lit encore (un rendu, un export) n'est pas touché. À lancer d'un fil de fond ; traite au plus `max` dessins.
/// Rend (dessins traités, tuiles rangées).
pub(crate) fn ranger_les_inactifs(repos: u64, max: usize) -> (usize, usize) {
    // 1. Les candidats, sortis du cache avec les références qui les doublent (`last`), sous le verrou, sans rien calculer.
    let mut sortis: Vec<(CelId, u64, usize, u64, Arc<Cel>, Vec<CelId>)> = Vec::new();
    {
        let mut st = lock();
        let maintenant = TICK.load(Ordering::Relaxed);
        let ids: Vec<CelId> = st
            .cache
            .iter()
            .filter(|(_, e)| maintenant.saturating_sub(e.used) >= repos && (e.cel.a_des_tuiles_a_ranger() || e.cel.a_des_tuiles_lachables()))
            .map(|(id, _)| *id)
            .take(max)
            .collect();
        for id in ids {
            let Some(e) = st.cache.remove(&id) else { continue };
            // (Les « derniers résultats » qui tiennent le même dessin : sortis aussi, remis avec lui.)
            let origines: Vec<CelId> = st.last.iter().filter(|(_, l)| Arc::ptr_eq(&l.cel, &e.cel)).map(|(o, _)| *o).collect();
            let mut lasts = Vec::new();
            for o in origines {
                if let Some(l) = st.last.remove(&o) {
                    lasts.push((o, l));
                }
            }
            // Un lecteur tient encore le dessin (en dehors du cache) : on le laisse tel quel.
            if Arc::strong_count(&e.cel) != 1 + lasts.len() {
                st.cache.insert(id, e);
                for (o, l) in lasts {
                    st.last.insert(o, l);
                }
                continue;
            }
            let ids_last: Vec<CelId> = lasts.iter().map(|(o, _)| *o).collect();
            drop(lasts);
            sortis.push((id, e.version, e.bytes, e.used, e.cel, ids_last));
        }
    }
    // 2. Le rangement, hors verrou : le cache n'a plus d'autre référence que la nôtre.
    let mut tuiles = 0;
    let traites = sortis.len();
    let mut a_remettre = Vec::with_capacity(traites);
    for (id, version, bytes, used, mut cel, origines) in sortis {
        if let Some(c) = Arc::get_mut(&mut cel) {
            tuiles += c.geler();
        }
        a_remettre.push((id, version, bytes, used, cel, origines));
    }
    // 3. On remet tout, sauf ce qu'un calcul plus récent a remplacé entre-temps.
    let mut st = lock();
    for (id, version, bytes, used, cel, origines) in a_remettre {
        if st.cache.contains_key(&id) {
            st.bytes = st.bytes.saturating_sub(bytes);
            continue;
        }
        for o in origines {
            if !st.last.contains_key(&o) {
                st.last.insert(o, Last { id, cel: cel.clone(), used, bytes });
            }
        }
        st.cache.insert(id, Entry { version, cel, used, bytes });
    }
    (traites, tuiles)
}

/// Mesures : (dessins dérivés distincts gardés, octets réellement tenus en mémoire par leurs tuiles, copies d'origine gardées pour le fil de
/// fond, octets de ces copies).
#[cfg(test)]
pub(crate) fn octets_en_memoire() -> (usize, usize, usize, usize) {
    let st = lock();
    let mut vus: std::collections::HashSet<*const Cel> = std::collections::HashSet::new();
    let mut octets = 0;
    for c in st.cache.values().map(|e| &e.cel).chain(st.last.values().map(|l| &l.cel)) {
        if vus.insert(Arc::as_ptr(c)) {
            octets += c.octets_en_memoire();
        }
    }
    let sources = st.sources.len() + st.small.len();
    let oct_sources = st.sources.values().map(|s| s.1.octets_en_memoire()).sum::<usize>() + st.small.values().map(|s| s.1.octets_en_memoire()).sum::<usize>();
    (vus.len(), octets, sources, oct_sources)
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

// ------------------------------------------------------------------ la réserve privée d'un fil d'export

/// Un dessin dérivé de la réserve privée.
struct EntreePrivee {
    version: u64,
    cel: Arc<Cel>,
    used: u64,
    bytes: usize,
}

/// La réserve d'un fil d'export (`exportrender.rs`). L'export calcule ses effets sur des fils à lui, avec une
/// copie du projet : il ne touche ni à la réserve commune (il ne chasse pas de l'écran les dessins que
/// l'interface vient de calculer, et n'y range pas ceux d'un projet que l'utilisateur est en train de
/// retoucher), ni à ses calculs de fond. Chaque fil a la sienne, limitée en octets ; les dessins touchés pendant
/// l'image en cours ne partent jamais (l'image en cours les relit : elle dépasse alors la limite plutôt que de
/// se tromper).
struct Privee {
    cache: BTreeMap<CelId, EntreePrivee>,
    bytes: usize,
    limit: usize,
    /// Horloge au début de l'image en cours.
    image_from: u64,
}

impl Privee {
    fn get(&mut self, id: CelId, version: Option<u64>) -> Option<(u64, Arc<Cel>)> {
        let e = self.cache.get_mut(&id).filter(|e| version.is_none_or(|v| e.version == v))?;
        e.used = tick();
        Some((e.version, e.cel.clone()))
    }

    fn insert(&mut self, id: CelId, version: u64, cel: Arc<Cel>) {
        let bytes = cel_bytes(&cel);
        if let Some(old) = self.cache.insert(id, EntreePrivee { version, cel, used: tick(), bytes }) {
            self.bytes -= old.bytes;
        }
        self.bytes += bytes;
        while self.bytes > self.limit {
            let from = self.image_from;
            let Some(old) = self.cache.iter().filter(|(k, e)| **k != id && e.used < from).min_by_key(|(_, e)| e.used).map(|(k, _)| *k) else { break };
            if let Some(e) = self.cache.remove(&old) {
                self.bytes -= e.bytes;
            }
        }
    }
}

thread_local! {
    static PRIVEE: std::cell::RefCell<Option<Privee>> = const { std::cell::RefCell::new(None) };
}

/// Exécute `f` avec une réserve privée de `limit` octets pour ce fil : tous les dessins dérivés qu'il demande
/// (effets, fonds, calques d'effets) y sont calculés et gardés, jamais dans la réserve commune.
pub(crate) fn avec_privee<R>(limit: usize, f: impl FnOnce() -> R) -> R {
    struct Garde;
    impl Drop for Garde {
        fn drop(&mut self) {
            PRIVEE.with(|p| *p.borrow_mut() = None);
        }
    }
    PRIVEE.with(|p| *p.borrow_mut() = Some(Privee { cache: BTreeMap::new(), bytes: 0, limit, image_from: tick() }));
    let _garde = Garde;
    f()
}

/// Une nouvelle image commence sur ce fil : les dessins gardés jusqu'ici peuvent partir si la limite l'exige.
pub(crate) fn privee_nouvelle_image() {
    PRIVEE.with(|p| {
        if let Some(p) = p.borrow_mut().as_mut() {
            p.image_from = tick();
        }
    });
}

/// Ce fil travaille-t-il avec une réserve privée ?
pub(crate) fn privee_active() -> bool {
    PRIVEE.with(|p| p.borrow().is_some())
}

/// `f` sur la réserve privée du fil, s'il en a une.
fn dans_privee<R>(f: impl FnOnce(&mut Privee) -> R) -> Option<R> {
    PRIVEE.with(|p| p.borrow_mut().as_mut().map(f))
}

/// Octets gardés par la réserve privée de ce fil (essais).
#[cfg(test)]
pub(crate) fn privee_octets() -> usize {
    dans_privee(|p| p.bytes).unwrap_or(0)
}

/// Le dessin dérivé `id` de la version `version`, s'il est gardé.
pub(crate) fn get(id: CelId, version: u64) -> Option<Arc<Cel>> {
    if let Some(r) = dans_privee(|p| p.get(id, Some(version)).map(|(_, c)| c)) {
        return r;
    }
    if monde_perime() {
        return None;
    }
    let mut st = lock();
    let e = st.cache.get_mut(&id).filter(|e| e.version == version)?;
    e.used = tick();
    Some(e.cel.clone())
}

/// Le dessin dérivé `id` gardé, quelle que soit sa version (avec sa version) : le fond sous le
/// trait montre l'ancien en attendant le nouveau (`fx::fond_shown`).
pub(crate) fn get_any(id: CelId) -> Option<(u64, Arc<Cel>)> {
    if let Some(r) = dans_privee(|p| p.get(id, None)) {
        return r;
    }
    if monde_perime() {
        return None;
    }
    let mut st = lock();
    let e = st.cache.get_mut(&id)?;
    e.used = tick();
    Some((e.version, e.cel.clone()))
}

/// Range un dessin dérivé arrivé d'ailleurs (le fond calculé en arrière-plan, `fx::fond_poll`).
pub(crate) fn put(id: CelId, version: u64, cel: Arc<Cel>) {
    if privee_active() || monde_perime() {
        return;
    }
    let dropped = insert(&mut lock(), id, version, cel, None);
    drop(dropped);
}

/// Range un dessin dérivé ; renvoie ceux que la réserve a dû lâcher (à libérer hors du verrou).
fn insert(st: &mut State, id: CelId, version: u64, cel: Arc<Cel>, origin: Option<CelId>) -> Vec<Arc<Cel>> {
    insert_within(st, id, version, cel, origin, LIMIT.load(Ordering::Relaxed))
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
        if let Some(prev) = st.last.insert(origin, Last { id, cel, used, bytes }) {
            dropped.push(prev.cel);
        }
        while (st.last.len() > LAST_MAX || st.last.values().map(|l| l.bytes).sum::<usize>() > LAST_OCTETS) && st.last.len() > 1 {
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
    // Un fil d'export : sa réserve à lui (rien n'est partagé, rien n'est attendu).
    if privee_active() {
        if let Some(Some((_, cel))) = dans_privee(|p| p.get(id, Some(version))) {
            return cel;
        }
        let cel = Arc::new(make());
        dans_privee(|p| p.insert(id, version, cel.clone()));
        return cel;
    }
    // (Un fil qui travaille pour un onglet qui n'est plus affiché ne touche pas au cache de celui qui l'est.)
    if monde_perime() {
        return Arc::new(make());
    }
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
    if privee_active() || monde_perime() {
        return;
    }
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
    if monde_perime() {
        return Arc::new(cel.raster.clone());
    }
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

/// Le dessin vectoriel d'origine `cel` (version `version`) copié pour le fil de fond, une fois par
/// version : les effets du trait en lisent les traits. Rien si le dessin n'est pas vectoriel. La copie
/// partage les pixels gardés de chaque trait (leur cache est une référence partagée) : elle ne coûte
/// que les points.
pub(crate) fn vector_source(origin: CelId, cel: &Cel) -> Option<Arc<VectorDrawing>> {
    let vector = cel.vector.as_ref()?;
    if monde_perime() {
        return Some(Arc::new(vector.clone()));
    }
    if let Some((v, a, used)) = lock().vsources.get_mut(&origin) {
        if *v == cel.version {
            *used = tick();
            return Some(a.clone());
        }
    }
    let copy = Arc::new(vector.clone());
    let mut st = lock();
    let mut dropped = Vec::new();
    if let Some(old) = st.vsources.insert(origin, (cel.version, copy.clone(), tick())) {
        dropped.push(old.1);
    }
    while st.vsources.len() > SOURCES_MAX {
        let Some(old) = st.vsources.iter().min_by_key(|(_, s)| s.2).map(|(k, _)| *k) else { break };
        if let Some(s) = st.vsources.remove(&old) {
            dropped.push(s.1);
        }
    }
    drop(st);
    drop(dropped);
    Some(copy)
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
    if privee_active() || monde_perime() {
        return None;
    }
    let mut st = lock();
    let l = st.last.get_mut(&origin)?;
    l.used = tick();
    Some((l.id, l.cel.clone()))
}

/// Le dessin d'origine `copie` est une copie de `source` (une clé neuve faite par copie du dessin : ses griffonnages sont ceux de la
/// clé d'avant, et ses traits aussi) : en attendant son propre calcul, son remplissage à l'écran est celui de `source`, au lieu de
/// rien (l'aplat de la clé neuve ne disparaît pas le temps qu'il se calcule). Sans effet si `source` n'a pas de dernier résultat ou si
/// `copie` en a déjà un.
pub(crate) fn heriter(source: CelId, copie: CelId) {
    if privee_active() || monde_perime() {
        return;
    }
    let mut st = lock();
    if st.last.contains_key(&copie) {
        return;
    }
    let Some(l) = st.last.get(&source) else { return };
    let neuf = Last { id: l.id, cel: l.cel.clone(), used: tick(), bytes: l.bytes };
    st.last.insert(copie, neuf);
}

/// Le dessin dérivé `id` de la version `version` est-il déjà demandé (en file ou en cours) ? Si oui, la demande est
/// renouvelée (elle ne vieillit pas) et vrai est rendu : on évite de refaire la demande, et la copie de ses dessins
/// d'origine, à chaque image d'écran qui attend le résultat.
pub(crate) fn renew(id: CelId, version: u64, prio: u32) -> bool {
    if privee_active() || monde_perime() {
        return false;
    }
    let mut st = lock();
    if st.inflight.get(&id) == Some(&version) {
        return true;
    }
    match st.queue.iter_mut().find(|q| q.id == id && q.version == version) {
        Some(q) => {
            q.prio = q.prio.min(prio);
            q.epoch = epoch();
            true
        }
        None => false,
    }
}

/// Demande le calcul de `job` au fil de fond (sans attendre). Rien si le dessin est déjà gardé ou
/// en cours de calcul ; une demande déjà en file voit sa priorité remontée.
pub(crate) fn request(job: Job) {
    // (Un travail de fond qui a perdu son monde — l'onglet a changé — ne demande rien. Un fil d'export non
    // plus : il calcule lui-même, tout de suite.)
    if privee_active() || monde_perime() {
        return;
    }
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
    // (Une demande abandonnée ne viendra pas : comme après un calcul fini, les vignettes incomplètes qui
    // l'attendaient (`generation`) se refont, et la redemandent si elles en ont encore besoin. Sans cela,
    // une vignette dont la demande d'effet vieillissait avant que le fil la prenne — fil occupé, machine
    // chargée — restait incomplète pour toujours.)
    if drop_aged(st, now) {
        GENERATION.fetch_add(1, Ordering::Relaxed);
    }
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

/// Abandonne les demandes que l'interface n'a pas renouvelées depuis `MAX_AGE` images d'écran (au temps
/// `now`) ; vrai s'il y en avait.
fn drop_aged(st: &mut State, now: u64) -> bool {
    let before = st.queue.len();
    st.queue.retain(|j| j.epoch + MAX_AGE >= now);
    st.queue.len() < before
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
        // (Les formes de fond gardées par `fx.rs` (`COVERS`, rangées par numéro de dessin et par version) et les fonds en
        // calcul sont à oublier aussi : deux essais ont les mêmes numéros et les mêmes versions, et un essai retrouvait la
        // forme de fond du dessin d'un autre, un fond manquant sur trois ou quatre exécutions sous charge.)
        crate::fx::fond_cancel_all();
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
        std::mem::take(&mut st.vsources),
    );
    st.bytes = 0;
    st.queue.clear();
    drop(st);
    drop(old);
}

/// Les dessins dérivés d'un projet mis de côté pendant qu'un autre onglet est affiché (onglets de
/// projets, `onglets.rs`) : les numéros de dessin sont ceux d'un projet, il ne faut ni les mélanger avec
/// ceux de l'autre, ni les recalculer au retour.
pub(crate) struct Reserve {
    cache: BTreeMap<CelId, Entry>,
    last: BTreeMap<CelId, Last>,
    sources: BTreeMap<CelId, (u64, Arc<Raster>, u64)>,
    small: BTreeMap<(CelId, u8), (u64, Arc<Raster>, u64)>,
    vsources: BTreeMap<CelId, (u64, Arc<VectorDrawing>, u64)>,
    bytes: usize,
}

impl Reserve {
    /// Octets de pixels gardés.
    pub(crate) fn bytes(&self) -> usize {
        self.bytes
    }
}

/// Le projet affiché s'en va : ses dessins dérivés sont rangés dans la réserve rendue, le cache est vide
/// pour le suivant, et les calculs en cours (demandés dans l'ancien monde) seront jetés à leur arrivée.
pub(crate) fn mettre_de_cote() -> Reserve {
    WORLD.fetch_add(1, Ordering::Relaxed);
    let mut st = lock();
    let r = Reserve {
        cache: std::mem::take(&mut st.cache),
        last: std::mem::take(&mut st.last),
        sources: std::mem::take(&mut st.sources),
        small: std::mem::take(&mut st.small),
        vsources: std::mem::take(&mut st.vsources),
        bytes: std::mem::take(&mut st.bytes),
    };
    st.queue.clear();
    r
}

/// Le projet qui revient reprend ses dessins dérivés (le cache est celui de l'onglet qui part : vide, ou
/// vidé par `mettre_de_cote`).
pub(crate) fn rendre(r: Reserve) {
    let mut st = lock();
    let old = (
        std::mem::replace(&mut st.cache, r.cache),
        std::mem::replace(&mut st.last, r.last),
        std::mem::replace(&mut st.sources, r.sources),
        std::mem::replace(&mut st.small, r.small),
        std::mem::replace(&mut st.vsources, r.vsources),
    );
    st.bytes = r.bytes;
    st.queue.clear();
    drop(st);
    drop(old);
}

fn worker_loop() {
    loop {
        // Le fil de fond laisse deux cœurs à l'interface et à la carte graphique (relu à chaque
        // calcul : « fils de calcul » se règle sans redémarrer).
        crate::render::set_thread_cap(crate::parallele::coeurs().saturating_sub(2).max(1));
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
        let raster = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match &job.base {
            Some(base) => base.render(job.level),
            None if job.griffon.is_some() => {
                let (sources, gap) = job.griffon.as_ref().expect("testé juste avant");
                let lines: Vec<&Raster> = sources.iter().map(|s| &**s).collect();
                crate::fx::apply(&crate::griffonnage::fill(&lines, &job.src, *gap as usize), &job.steps)
            }
            None if job.level == 0 => crate::fx::apply_with(&job.src, job.vector.as_deref(), &job.steps, 0),
            None => crate::fx::apply_scaled(&reduced_source(&job), &job.steps, job.level),
        }))
        .unwrap_or_else(|_| match &job.base {
            // (Un effet qui plante : ce qui est dessous, sans lui.)
            Some(base) => std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| base.compose(job.level))).unwrap_or_else(|_| Raster::new(0, 0)),
            // (Un remplissage qui plante : rien. Surtout pas les griffonnages eux-mêmes, qui sont des taches de couleur.)
            None if job.griffon.is_some() => Raster::new(job.src.width, job.src.height),
            None => crate::fxproxy::downscale(&job.src, job.level),
        });
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

/// Comme `mark_incomplete`, seulement en mode doux (rien ailleurs).
pub(crate) fn mark_incomplete_if_soft() {
    if soft_active() {
        mark_incomplete();
    }
}
