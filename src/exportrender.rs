//! Le rendu des images d'un export, hors du fil de l'interface (règle « sans latence » d'Anthony).
//!
//! Avant, l'export calculait ses images sur le fil de l'interface, quarante millisecondes par image d'écran,
//! et verrouillait le projet pendant ce temps. Maintenant :
//!
//! 1. **Le cliché** (`SnapshotBuilder`). Au départ de l'export, l'interface prend une copie du projet : la
//!    structure (plans, calques, caméras, transitions, son…) est copiée tout de suite, les dessins des plans
//!    exportés par tranches de quelques millisecondes. Copier un dessin ne copie aucun pixel : les tuiles sont
//!    partagées (`Arc`, comme les vignettes de `thumbs.rs`) ; si on dessine ensuite, seule la tuile touchée est
//!    copiée (copie à la première écriture). L'export lit son cliché ; le projet est libre.
//! 2. **Les fils de rendu** (`Pipeline`). Quelques fils calculent les images avec **le code d'avant** (plan du
//!    plan, caméra, `render::camera_image`, effets, calques d'effets, transitions, incrustations gravées : les
//!    mêmes pixels), en mode exact, chacun avec sa réserve privée de dessins dérivés (`fxcache::avec_privee`),
//!    à une priorité inférieure à celle de l'interface, et en laissant des cœurs à l'interface. Une image
//!    tenue (même plan, même caméra) n'est calculée qu'une fois pour tous les fils (`ImageStore`).
//! 3. **Le passeur**. Un fil rend les images dans l'ordre du film à l'écriture (fichiers, ffmpeg, PDF…) :
//!    c'est lui qui attend quand l'écriture est en retard, jamais l'interface. Les fils de rendu n'avancent pas
//!    de plus de quelques images devant lui (mémoire bornée).
//!
//! L'interface ne fait plus que lire un compteur (`Pipeline::emitted`) et vérifier que rien n'a échoué.

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use eframe::egui::Color32;

use crate::camera::Camera;
use crate::document::{CelId, Document, Panel};
use crate::render::{self, Plan};
use crate::transitions::Zone;

/// Une image finale, partagée (une image tenue n'est qu'une seule mémoire).
pub type Image = Arc<Vec<Color32>>;

// ---------------------------------------------------------------------------------- comptes (essais)

#[cfg(test)]
thread_local! {
    /// Images composées par ce fil (essais : l'interface n'en compose aucune pendant un export).
    pub(crate) static IMAGES_ICI: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

// ------------------------------------------------------------------------------------------ cliché

/// Les dessins qu'un plan montre : ceux de ses calques, et ceux que ses calques de couleur y ajoutent (les sources
/// d'un remplissage au griffonnage sont d'autres calques du même plan ; ses griffonnages sont rangés avec eux).
fn dessins_du_plan(p: &Panel, out: &mut Vec<CelId>, seen: &mut HashSet<CelId>) {
    let mut add = |id: CelId| {
        if seen.insert(id) {
            out.push(id);
        }
    };
    for l in &p.layers {
        for id in l.frames.iter().flatten() {
            add(*id);
        }
        if let Some(c) = &l.couleur {
            for (a, b) in &c.dessins {
                add(*a);
                add(*b);
            }
            for g in c.dessins_griffonnages() {
                add(g);
            }
        }
    }
}

/// Un plan que l'export ne montre pas, mais dont la structure compte : sa durée et sa caméra (la caméra d'un plan
/// sans clé suit celle de ses voisins), son nom, sa place. Sans calques ni dessins.
fn plan_leger(p: &Panel) -> Panel {
    Panel {
        id: p.id,
        scene: p.scene,
        length: p.frame_count(),
        layers: Vec::new(),
        action: p.action.clone(),
        dialogue: p.dialogue.clone(),
        notes: p.notes.clone(),
        camera: p.camera.clone(),
        marks: p.marks.clone(),
        cues: p.cues.clone(),
        color: p.color,
    }
}

/// Le rendu exact des dessins vectoriels restés en rendu rapide (`ids`, ceux de `App::rejeu_idle`) : calculé sur
/// plusieurs cœurs. Rend, pour chaque dessin refait, (numéro, version d'avant, version d'après). L'export l'applique à son
/// cliché, sur un autre fil (le projet affiché garde son rendu rapide) ; l'application, elle, l'applique au projet
/// (`App::exactify_idle`).
pub fn rendre_exacts(doc: &mut Document, ids: &[CelId]) -> Vec<(CelId, u64, u64)> {
    let (w, h) = (doc.width, doc.height);
    let jobs: Vec<(CelId, crate::vector::VectorDrawing)> = ids.iter().filter_map(|id| doc.cels.get(id).and_then(|c| c.vector.clone()).map(|d| (*id, d))).collect();
    let done = crate::parallele::par_map(&jobs, 8, |_, (_, drawing)| {
        let mut copy = drawing.clone();
        for e in &mut copy.elements {
            if e.cache_is_preview() {
                e.invalidate_cache();
            }
        }
        let raster = copy.render(w, h);
        (copy, raster)
    });
    let mut out = Vec::with_capacity(jobs.len());
    for ((id, _), (copy, raster)) in jobs.into_iter().zip(done) {
        let Some(c) = doc.cels.get_mut(&id) else { continue };
        let Some(drawing) = c.vector.as_mut() else { continue };
        for (e, exact) in drawing.elements.iter_mut().zip(&copy.elements) {
            e.adopt_cache(exact);
        }
        let before = c.version;
        c.raster = raster;
        c.version += 1;
        out.push((id, before, c.version));
    }
    out
}

/// Le cliché d'un projet en train de se faire. La structure est copiée à la construction ; les dessins par
/// tranches (`step`), pour que l'interface ne s'arrête jamais plus que quelques millisecondes. Les dessins restés en rendu
/// rapide sont refaits exacts dans la copie (`with_exact`), sur un fil à part.
pub struct SnapshotBuilder {
    snap: Option<Document>,
    ids: Vec<CelId>,
    pos: usize,
    exact: Vec<CelId>,
    /// Le rendu exact en cours : le fil, et la copie qu'il travaille (rendue dans la case en finissant).
    exacting: Option<(JoinHandle<()>, Arc<Mutex<Option<Document>>>)>,
}

impl SnapshotBuilder {
    /// Le cliché de `doc` pour montrer les plans `panels` (les autres plans ne gardent que leur structure).
    pub fn new(doc: &Document, panels: &[usize]) -> SnapshotBuilder {
        let wanted: HashSet<usize> = panels.iter().copied().filter(|&p| p < doc.panels.len()).collect();
        let mut s = Document::new(doc.width, doc.height, doc.fps);
        s.frame = doc.frame;
        s.paper = doc.paper;
        s.sequences = doc.sequences.clone();
        s.scenes = doc.scenes.clone();
        s.panels = doc.panels.iter().enumerate().map(|(i, p)| if wanted.contains(&i) { p.clone() } else { plan_leger(p) }).collect();
        s.audio_tracks = doc.audio_tracks.clone();
        // (Les sons sont des `Arc` : copier la table ne copie aucun échantillon.)
        s.sounds = doc.sounds.clone();
        s.guides = doc.guides.clone();
        s.board = doc.board.clone();
        s.groups = doc.groups.clone();
        s.start_frame = doc.start_frame;
        s.background = doc.background;
        s.paper_b = doc.paper_b;
        s.master_volume = doc.master_volume;
        s.storyboard_locked = doc.storyboard_locked;
        s.numbering_locked = doc.numbering_locked;
        s.properties = doc.properties.clone();
        s.comments = doc.comments.clone();
        s.transitions = doc.transitions.clone();
        // Pas copiés : ce que le rendu ne lit jamais (autres versions des plans, références visuelles, rognages,
        // planche de crayonnés, envois, sons proposés).
        let mut ids = Vec::new();
        let mut seen = HashSet::new();
        let mut order: Vec<usize> = wanted.into_iter().collect();
        order.sort_unstable();
        for i in order {
            dessins_du_plan(&doc.panels[i], &mut ids, &mut seen);
        }
        SnapshotBuilder { snap: Some(s), ids, pos: 0, exact: Vec::new(), exacting: None }
    }

    /// Les dessins `ids` (restés en rendu rapide dans le projet) sont refaits exacts dans la copie, une fois copiés.
    pub fn with_exact(mut self, ids: Vec<CelId>) -> SnapshotBuilder {
        self.exact = ids;
        self
    }

    /// La construction lit encore le projet affiché (les dessins se copient) : il ne doit pas changer. Ensuite, le rendu exact
    /// travaille sur la copie.
    pub fn reads_project(&self) -> bool {
        self.pos < self.ids.len()
    }

    /// Copie des dessins pendant `budget` au plus ; vrai quand tous le sont (et que le rendu exact est fait, s'il y en a un).
    pub fn step(&mut self, doc: &Document, budget: Duration) -> bool {
        if let Some((h, _)) = &self.exacting {
            if !h.is_finished() {
                return false;
            }
            if let Some((h, slot)) = self.exacting.take() {
                // (Un rendu qui plante : l'export continue avec les dessins en rendu rapide plutôt que de s'arrêter.)
                let _ = h.join();
                self.snap = slot.lock().unwrap_or_else(|e| e.into_inner()).take();
            }
            return true;
        }
        let t0 = Instant::now();
        while self.pos < self.ids.len() {
            let id = self.ids[self.pos];
            self.pos += 1;
            if let (Some(snap), Some(c)) = (self.snap.as_mut(), doc.cels.get(&id)) {
                snap.cels.insert(id, c.clone());
            }
            if self.pos % 8 == 0 && t0.elapsed() >= budget {
                break;
            }
        }
        if self.pos < self.ids.len() {
            return false;
        }
        if self.exact.is_empty() {
            return true;
        }
        // Les dessins sont copiés : le rendu exact se fait sur un fil à part, sur la copie.
        let (ids, Some(snap)) = (std::mem::take(&mut self.exact), self.snap.take()) else { return true };
        let slot = Arc::new(Mutex::new(Some(snap)));
        let ma_case = slot.clone();
        let started = std::thread::Builder::new().name("export-exact".into()).spawn(move || {
            crate::parallele::baisser_la_priorite_ici();
            let Some(mut snap) = ma_case.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
            // (Même en cas de panique, la copie revient telle quelle.)
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rendre_exacts(&mut snap, &ids)));
            *ma_case.lock().unwrap_or_else(|e| e.into_inner()) = Some(snap);
        });
        match started {
            Ok(h) => {
                self.exacting = Some((h, slot));
                false
            }
            Err(_) => {
                // (Pas de fil pour le rendu exact : la copie reste telle quelle, en rendu rapide.)
                self.snap = slot.lock().unwrap_or_else(|e| e.into_inner()).take();
                true
            }
        }
    }

    /// Dessins à copier au total et déjà copiés (progression, essais).
    #[cfg(test)]
    pub fn progress(&self) -> (usize, usize) {
        (self.pos, self.ids.len())
    }

    pub fn finish(self) -> Arc<Document> {
        Arc::new(self.snap.unwrap_or_else(|| Document::new(1, 1, 1)))
    }
}
/// Le cliché d'un coup (essais, petits travaux).
#[cfg(test)]
pub fn snapshot_of(doc: &Document, panels: &[usize]) -> Arc<Document> {
    let mut b = SnapshotBuilder::new(doc, panels);
    b.step(doc, Duration::MAX);
    b.finish()
}

/// Les plans qu'un export de `frames` montre : ceux des images, et ceux des transitions qui les recouvrent.
pub fn panels_needed(frames: &[(usize, usize)], zones: &[Zone]) -> Vec<usize> {
    let mut seen: Vec<usize> = crate::export::panels_of(frames);
    for z in zones {
        for side in [z.out, z.inn].into_iter().flatten() {
            seen.push(side.panel);
        }
    }
    seen.sort_unstable();
    seen.dedup();
    seen
}

// ------------------------------------------------------------------------------------ les images

/// Le papier sous l'image.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Papier {
    /// Transparent.
    Aucun,
    /// L'arrière-plan du projet (couleur, ou rien pour un damier : `Document::render_paper`).
    Fond,
    /// La couleur du papier, toujours (la page HTML du storyboard).
    Feuille,
}

/// Une image déjà calculée, ou en train de l'être.
enum Place {
    EnCours,
    Prete(Image),
}

struct Entree {
    plan: Plan,
    cam: Camera,
    place: Place,
    used: u64,
}

/// Les dernières images calculées, partagées par tous les fils : une image tenue (même plan, même caméra) n'est
/// pas recalculée, et si deux fils la demandent ensemble, le second attend le premier.
struct ImageStore {
    entries: Mutex<Vec<Entree>>,
    ready: Condvar,
    clock: AtomicUsize,
    /// Images prêtes gardées au plus.
    cap: usize,
}

/// Retire l'image « en cours » si son calcul s'arrête sans la rendre (panique, annulation).
struct EnCours<'a> {
    store: &'a ImageStore,
    plan: &'a Plan,
    cam: &'a Camera,
    done: bool,
}

impl Drop for EnCours<'_> {
    fn drop(&mut self) {
        if !self.done {
            let mut st = self.store.lock();
            st.retain(|e| !(matches!(e.place, Place::EnCours) && e.plan == *self.plan && e.cam == *self.cam));
            drop(st);
            self.store.ready.notify_all();
        }
    }
}

impl ImageStore {
    fn new(cap: usize) -> ImageStore {
        ImageStore { entries: Mutex::new(Vec::new()), ready: Condvar::new(), clock: AtomicUsize::new(0), cap: cap.max(2) }
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Entree>> {
        self.entries.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn tick(&self) -> u64 {
        self.clock.fetch_add(1, Ordering::Relaxed) as u64
    }

    /// L'image de `plan` filmé par `cam` : gardée, ou calculée par `make` (une seule fois même si plusieurs fils la
    /// demandent). `None` : l'export est annulé.
    fn get(&self, plan: &Plan, cam: &Camera, cancel: &AtomicBool, make: impl FnOnce() -> Image) -> Option<Image> {
        let mut st = self.lock();
        loop {
            match st.iter_mut().find(|e| e.plan == *plan && e.cam == *cam) {
                Some(Entree { place: Place::Prete(img), used, .. }) => {
                    *used = self.tick();
                    return Some(img.clone());
                }
                Some(_) => {
                    if cancel.load(Ordering::Relaxed) {
                        return None;
                    }
                    st = self.ready.wait_timeout(st, Duration::from_millis(40)).unwrap_or_else(|e| e.into_inner()).0;
                }
                None => break,
            }
        }
        st.push(Entree { plan: plan.clone(), cam: *cam, place: Place::EnCours, used: self.tick() });
        drop(st);
        let mut guard = EnCours { store: self, plan, cam, done: false };
        let img = make();
        let mut st = self.lock();
        if let Some(e) = st.iter_mut().find(|e| matches!(e.place, Place::EnCours) && e.plan == *plan && e.cam == *cam) {
            e.place = Place::Prete(img.clone());
            e.used = self.tick();
        }
        guard.done = true;
        // Les plus anciennes images prêtes partent (jamais une image en cours).
        while st.iter().filter(|e| matches!(e.place, Place::Prete(_))).count() > self.cap {
            let Some(old) = st.iter().enumerate().filter(|(_, e)| matches!(e.place, Place::Prete(_))).min_by_key(|(_, e)| e.used).map(|(i, _)| i) else { break };
            st.remove(old);
        }
        drop(st);
        self.ready.notify_all();
        Some(img)
    }
}

/// Tout ce qu'il faut pour calculer les images d'un export, partagé par les fils de rendu.
pub struct Rendu {
    pub doc: Arc<Document>,
    papier: Papier,
    /// Graver les incrustations (timecode…) dans l'image.
    graver: bool,
    zones: Vec<Zone>,
    starts: Vec<usize>,
    /// Un seul plan, déjà fait (l'image courante exportée telle que l'écran l'a construit) : à la place de celui
    /// du plan et de l'image demandés.
    explicite: Option<(Plan, Camera)>,
    store: ImageStore,
    pub cancel: Arc<AtomicBool>,
    /// Images vraiment composées (une image tenue, ou reprise du magasin, ne compte pas).
    composees: AtomicUsize,
}

impl Rendu {
    /// `cancel` : levé, les fils qui attendent une image en cours de calcul l'abandonnent.
    pub fn new(doc: Arc<Document>, papier: Papier, graver: bool, zones: Vec<Zone>, cancel: Arc<AtomicBool>) -> Rendu {
        let starts = if zones.is_empty() { Vec::new() } else { (0..doc.panels.len()).map(|i| doc.panel_start(i)).collect() };
        Rendu { doc, papier, graver, zones, starts, explicite: None, store: ImageStore::new(fils_de_rendu().0 + 1), cancel, composees: AtomicUsize::new(0) }
    }

    /// Combien d'images ont été vraiment composées jusqu'ici.
    #[cfg(test)]
    pub fn composed(&self) -> usize {
        self.composees.load(Ordering::Relaxed)
    }

    /// Un seul plan (`plan` filmé par `cam`) au lieu de celui que le document donne.
    pub fn avec_plan(mut self, plan: Plan, cam: Camera) -> Rendu {
        self.explicite = Some((plan, cam));
        self
    }

    /// L'image (plan `p`, image `f`) sans transition, avec ses incrustations : celle d'avant (`ExportJob::render_plain`),
    /// recalculée seulement si elle a changé.
    fn simple(&self, p: usize, f: usize) -> Option<Image> {
        let doc = &*self.doc;
        let (plan, cam) = match &self.explicite {
            Some((plan, cam)) => (plan.clone(), *cam),
            None => {
                let paper = match self.papier {
                    Papier::Aucun => None,
                    Papier::Fond => doc.render_paper(),
                    Papier::Feuille => Some(doc.paper),
                };
                (render::panel_plan_at(doc, p, f, paper), doc.panel_camera(p, f))
            }
        };
        // (Le cache garde l'image sans incrustation : le timecode change à chaque image, `burned` la grave sur la
        // copie qu'on rend.)
        let img = self.store.get(&plan, &cam, &self.cancel, || {
            crate::fxcache::privee_nouvelle_image();
            self.composees.fetch_add(1, Ordering::Relaxed);
            #[cfg(test)]
            IMAGES_ICI.with(|n| n.set(n.get() + 1));
            Arc::new(render::camera_image(doc, &plan, &cam))
        })?;
        Some(if self.graver { crate::export::burned(doc, p, f, img) } else { img })
    }

    /// L'image finale (plan `p`, image `f`) : le plan, ou le mélange des deux plans d'une transition.
    pub fn image(&self, p: usize, f: usize) -> Option<Image> {
        let mix = match self.starts.get(p) {
            Some(&start) => crate::transitions::mix_in(&self.zones, start + f).map(|(_, m)| m),
            None => None,
        };
        let Some(mix) = mix else {
            return self.simple(p, f);
        };
        let a = match mix.a {
            Some((p, f)) => Some(self.simple(p, f)?),
            None => None,
        };
        let b = match mix.b {
            Some((p, f)) => Some(self.simple(p, f)?),
            None => None,
        };
        Some(Arc::new(crate::transitions::blend_images(&mix, a.as_deref().map(|v| &v[..]), b.as_deref().map(|v| &v[..]), self.doc.frame)))
    }
}

// ----------------------------------------------------------------------------------- la chaîne

/// Ce que renvoie l'envoi d'une image au fil d'écriture.
pub enum Pose {
    Ok,
    /// La file d'écriture est pleine : l'image revient, le passeur réessaie dans un instant.
    Pleine(Image),
    /// Plus personne n'écrit (le fil d'écriture s'est arrêté sur une erreur : on la lira).
    Cassee,
}

/// Fils de rendu et réserve privée de chacun, d'après la machine : on laisse deux cœurs à l'interface ; chaque fil de
/// rendu en répartit un petit nombre entre les morceaux d'une même image (`render::set_thread_cap`).
pub fn fils_de_rendu() -> (usize, usize) {
    fils_pour(crate::parallele::coeurs())
}

/// `fils_de_rendu` pour `coeurs` cœurs : (fils de rendu, cœurs pour chacun).
pub fn fils_pour(coeurs: usize) -> (usize, usize) {
    let libres = coeurs.saturating_sub(2).max(1);
    let fils = (libres / 3).clamp(1, 6);
    (fils, (libres / fils).max(1))
}

/// Combien d'images d'avance les fils de rendu peuvent prendre sur l'écriture, d'après la taille d'une image : la
/// mémoire tenue reste de l'ordre de 500 Mo en 4K (33 Mo l'image).
pub fn avance_permise(octets_image: usize, fils: usize, file: usize) -> usize {
    let total = ((512usize << 20) / octets_image.max(1)).clamp(6, 64);
    let gardees = 2 * fils + 1;
    total.saturating_sub(gardees + file).clamp(fils + 1, 2 * fils + 2)
}

/// L'état partagé par les fils de rendu et le passeur.
struct Etat {
    /// Prochaine image à prendre.
    prochaine: usize,
    /// Images passées à l'écriture.
    emises: usize,
    /// Images calculées qui attendent leur tour.
    pretes: BTreeMap<usize, Image>,
}

struct Partage {
    etat: Mutex<Etat>,
    change: Condvar,
    emises: AtomicUsize,
    annule: Arc<AtomicBool>,
    echec: Arc<Mutex<Option<String>>>,
}

impl Partage {
    fn lock(&self) -> MutexGuard<'_, Etat> {
        self.etat.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn echouer(&self, message: String) {
        let mut e = self.echec.lock().unwrap_or_else(|e| e.into_inner());
        if e.is_none() {
            *e = Some(message);
        }
        drop(e);
        self.annule.store(true, Ordering::Relaxed);
        self.change.notify_all();
    }

    fn arretee(&self) -> bool {
        self.annule.load(Ordering::Relaxed)
    }
}

/// Ce qui calcule l'image numéro `rang` d'un export (`None` : annulé).
pub type Calcul = dyn Fn(usize) -> Option<Image> + Send + Sync;

/// Les fils de rendu et le passeur d'un export.
pub struct Pipeline {
    part: Arc<Partage>,
    workers: Vec<JoinHandle<()>>,
    feeder: Option<JoinHandle<()>>,
}

impl Pipeline {
    /// Lance le rendu des images `frames` (dans l'ordre) vers `send`. `echec` reçoit la première erreur
    /// (un fil de rendu qui plante). Les fils s'arrêtent d'eux-mêmes à la dernière image ou à l'annulation
    /// (`rendu.cancel`). `file` : la profondeur de la file d'écriture (pour borner la mémoire).
    pub fn start(rendu: Arc<Rendu>, frames: Arc<Vec<(usize, usize)>>, send: Box<dyn FnMut(usize, Image) -> Pose + Send>, echec: Arc<Mutex<Option<String>>>, file: usize) -> Pipeline {
        let octets = rendu.doc.frame[0] * rendu.doc.frame[1] * 4;
        let (cancel, total) = (rendu.cancel.clone(), frames.len());
        let make: Arc<Calcul> = Arc::new(move |i| {
            let (p, f) = frames[i];
            rendu.image(p, f)
        });
        Pipeline::start_with(make, total, octets, cancel, send, echec, file)
    }

    /// Comme `start`, pour n'importe quels calculs : `make(rang)` rend l'image numéro `rang` (de 0 à `total`), ou `None`
    /// si l'export est annulé ; `octets` : la taille d'une image (pour borner la mémoire). L'export vers Storyboard Pro
    /// s'en sert pour les images de ses calques.
    pub fn start_with(make: Arc<Calcul>, total: usize, octets: usize, cancel: Arc<AtomicBool>, mut send: Box<dyn FnMut(usize, Image) -> Pose + Send>, echec: Arc<Mutex<Option<String>>>, file: usize) -> Pipeline {
        let (fils, coeurs) = fils_de_rendu();
        let avance = avance_permise(octets, fils, file);
        let limite = (crate::fxcache::limite_octets() / fils).max(96 << 20);
        let part = Arc::new(Partage {
            etat: Mutex::new(Etat { prochaine: 0, emises: 0, pretes: BTreeMap::new() }),
            change: Condvar::new(),
            emises: AtomicUsize::new(0),
            annule: cancel,
            echec,
        });
        let workers = (0..fils.min(total.max(1)))
            .filter_map(|k| {
                let (part, make) = (part.clone(), make.clone());
                std::thread::Builder::new().name(format!("export-rendu-{k}")).spawn(move || rendre(&part, &*make, total, avance, coeurs, limite)).ok()
            })
            .collect::<Vec<_>>();
        let feeder = {
            let part = part.clone();
            std::thread::Builder::new()
                .name("export-passeur".into())
                .spawn(move || {
                    passer(&part, total, &mut *send);
                    // (`send` — et avec lui l'entrée du fil d'écriture — se ferme ici.)
                })
                .ok()
        };
        // Aucun fil n'a pu partir : on le dit au lieu d'attendre pour toujours.
        if workers.is_empty() && total > 0 || feeder.is_none() {
            part.echouer("impossible de lancer le calcul des images".into());
        }
        Pipeline { part, workers, feeder }
    }

    /// Images passées à l'écriture.
    pub fn emitted(&self) -> usize {
        self.part.emises.load(Ordering::Relaxed)
    }

    /// Le passeur a fini (toutes les images sont parties, ou l'export s'est arrêté).
    pub fn finished(&self) -> bool {
        self.feeder.as_ref().is_none_or(|f| f.is_finished())
    }

    /// Les fils de rendu ont tous fini (essais).
    #[cfg(test)]
    pub fn workers_finished(&self) -> bool {
        self.workers.iter().all(|w| w.is_finished())
    }

    /// Arrête le rendu : le passeur finit tout de suite (il rend la file d'écriture) ; les fils de rendu finissent
    /// leur image en cours et s'en vont, sans que l'appelant les attende.
    pub fn cancel(&mut self) {
        self.part.annule.store(true, Ordering::Relaxed);
        self.part.change.notify_all();
        if let Some(f) = self.feeder.take() {
            let _ = f.join();
        }
        // (Les fils de rendu ne sont pas attendus : une image de 4K avec ses effets peut durer des secondes.)
        self.workers.clear();
    }

    /// Attend la fin du passeur (essais).
    #[cfg(test)]
    pub fn join_feeder(&mut self) {
        if let Some(f) = self.feeder.take() {
            let _ = f.join();
        }
    }
}

/// Un fil de rendu : prend l'image suivante (pas plus de `avance` devant ce que l'écriture a pris), la calcule, la
/// range pour le passeur.
fn rendre(part: &Partage, make: &Calcul, total: usize, avance: usize, coeurs: usize, limite: usize) {
    crate::parallele::baisser_la_priorite_ici();
    crate::render::set_thread_cap(coeurs);
    crate::fxcache::avec_privee(limite, || loop {
        let i = {
            let mut st = part.lock();
            loop {
                if part.arretee() || st.prochaine >= total {
                    return;
                }
                if st.prochaine < st.emises + avance {
                    break;
                }
                st = part.change.wait_timeout(st, Duration::from_millis(100)).unwrap_or_else(|e| e.into_inner()).0;
            }
            let i = st.prochaine;
            st.prochaine += 1;
            i
        };
        // Une image qui fait planter la composition (un projet abîmé) arrête l'export avec un message, pas SHOT.
        let made = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| make(i)));
        match made {
            Ok(Some(img)) => {
                part.lock().pretes.insert(i, img);
                part.change.notify_all();
            }
            Ok(None) => return,
            Err(_) => {
                part.echouer(format!("le calcul de l'image {} a échoué", i + 1));
                return;
            }
        }
    });
}

/// Le passeur : rend les images dans l'ordre à l'écriture.
fn passer(part: &Partage, total: usize, send: &mut dyn FnMut(usize, Image) -> Pose) {
    for i in 0..total {
        let mut img = {
            let mut st = part.lock();
            loop {
                if part.arretee() {
                    return;
                }
                if let Some(img) = st.pretes.remove(&i) {
                    break img;
                }
                st = part.change.wait_timeout(st, Duration::from_millis(100)).unwrap_or_else(|e| e.into_inner()).0;
            }
        };
        loop {
            match send(i, img) {
                Pose::Ok => break,
                Pose::Cassee => return,
                Pose::Pleine(back) => {
                    if part.arretee() {
                        return;
                    }
                    img = back;
                    std::thread::sleep(Duration::from_millis(2));
                }
            }
        }
        part.lock().emises = i + 1;
        part.emises.store(i + 1, Ordering::Relaxed);
        part.change.notify_all();
    }
}

/// Un dessin du cliché, pour les essais : le projet d'origine l'a-t-il pareil ?
#[cfg(test)]
pub(crate) fn meme_dessin(a: &crate::document::Cel, b: &crate::document::Cel) -> bool {
    a.version == b.version && a.raster.width == b.raster.width && a.raster.tiles.len() == b.raster.tiles.len() && a.raster.tiles.iter().zip(&b.raster.tiles).all(|(x, y)| match (x, y) {
        (Some(x), Some(y)) => x.same_as(y),
        (None, None) => true,
        _ => false,
    })
}

#[cfg(test)]
#[path = "exportrender_tests.rs"]
mod tests;
