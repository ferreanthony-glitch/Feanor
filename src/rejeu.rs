//! Rejeu exact en arrière-plan (Anthony, 2026-10-18 : « ma priorité est d'avoir un logiciel qui
//! réponde toujours très rapidement, sans latence » ; règle : pendant un geste, jamais de rejeu du
//! moteur sur le fil de l'interface).
//!
//! Un trait vectoriel texturé se rend exactement en rejouant le moteur bitmap (`vector.rs`), ce qui
//! coûte quelques millisecondes par trait : trop pour le refaire à chaque image d'un glissé, et
//! trop pour bloquer l'interface au lâcher d'un dessin de cinquante traits. Pendant un geste, on
//! montre donc les pixels en cache, déformés à la volée (translation copiée, affine
//! rééchantillonnée, enveloppe et perspective par maillage, feuille flottante `flottante.rs`) ;
//! au lâcher, le rendu exact du dessin est demandé ici, à un fil de travail, et l'aperçu reste
//! affiché jusqu'à ce qu'il arrive.
//!
//! Chaque demande porte un numéro (`gen`) par dessin : une demande plus récente, ou un rendu
//! direct du dessin (`App::refresh_cel`, annulation), rend les précédentes caduques — elles sont
//! retirées de la file, et leurs résultats jetés. Un résultat adopté (`App::poll_rejeu`) remplace
//! l'image du dessin et donne aux traits leurs pixels exacts en cache (ceux du rendu), d'un coup.
//! Le fil ne rejoue que les traits sans pixels exacts (jamais rendus, évincés du budget, ou
//! déformés par un aperçu) : un trait de plus coûte un trait, pas le dessin entier — c'est ce qui
//! permet de lui confier la pose d'un trait tracé au lever du stylet (`commit_vector_stroke`).
//!
//! Deux priorités : les demandes interactives (un geste vient de finir) passent avant la file du
//! chargement d'un projet. Un panic du rendu n'arrête pas le fil : la demande est signalée perdue
//! (`take_failed`), et un fil mort est recréé à la demande suivante.
//!
//! Gros projets (essai de charge du 2026-10-23, `docs/PERFORMANCE.md`, section 25) : un fil, à priorité normale, ne
//! prend que les demandes interactives — il est toujours libre pour un geste ; la file du chargement (des milliers de
//! dessins à l'ouverture d'un film) a ses fils à elle, autant que de cœurs moins deux, sous la priorité de l'interface.
//! Les copies des dessins à rejouer à l'ouverture se font sur le fil de la lecture (`Demande`, `ouverture.rs`) et
//! entrent dans la file d'un coup (`submit_batch`) : l'image d'adoption ne copie plus rien. La file est indexée par
//! dessin : remplacer ou retirer une demande ne la parcourt plus.

use std::collections::{HashMap, VecDeque};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::document::CelId;
use crate::raster::Raster;
use crate::vector::VectorDrawing;

struct Job {
    cel: CelId,
    gen: u64,
    version: u64,
    /// Demande interactive (un geste vient de finir) : passe avant le chargement.
    priority: bool,
    drawing: VectorDrawing,
    w: usize,
    h: usize,
}

/// Un dessin à rejouer, copié pour le fil de travail : ses traits aux pixels déformés (aperçus) les ont perdus, pour être
/// refaits exactement. La copie d'un film entier coûte : à l'ouverture, elle se fait sur le fil de la lecture.
pub struct Demande {
    pub cel: CelId,
    /// La version du dessin copié (`Cel::version`).
    pub version: u64,
    drawing: VectorDrawing,
}

impl Demande {
    pub fn new(cel: CelId, drawing: &VectorDrawing, version: u64) -> Demande {
        // Les traits dont les pixels en cache sont exacts (rendus à leur place, signature juste)
        // les gardent : le fil les pose tels quels, et ne rejoue que les autres — un trait de plus
        // à chaque lâcher ne coûte pas le dessin entier. Les aperçus (pixels déformés) repartent de
        // zéro, dans un cache à part. Un cache vide ou périmé est refait par le fil, dans le cache
        // partagé avec le dessin de l'interface : le trait y trouve ses pixels exacts.
        let mut copy = drawing.clone();
        for e in &mut copy.elements {
            if e.cache_is_preview() {
                e.invalidate_cache();
            }
        }
        Demande { cel, version, drawing: copy }
    }
}

/// Les demandes des dessins vectoriels `ids` de `doc`, copiés (à faire hors du fil de l'interface : la lecture d'un projet).
pub fn demandes(doc: &crate::document::Document, ids: &[CelId]) -> Vec<Demande> {
    crate::parallele::par_map(ids, usize::MAX, |_, id| doc.cels.get(id).and_then(|c| c.vector.as_ref().map(|d| Demande::new(*id, d, c.version)))).into_iter().flatten().collect()
}

/// Un rendu exact terminé : le dessin tel qu'il a été rendu (ses traits ont leurs pixels en
/// cache) et son image.
pub struct Done {
    pub cel: CelId,
    pub gen: u64,
    /// La version du dessin (`Cel::version`) au moment de la demande.
    pub version: u64,
    pub drawing: VectorDrawing,
    pub raster: Raster,
    /// Durée du rendu, pour les mesures.
    #[allow(dead_code)]
    pub took: Duration,
}

enum Msg {
    Done(Done),
    /// Le rendu a paniqué.
    Failed { cel: CelId, gen: u64 },
}

/// Les demandes en attente : une par dessin (la plus récente), et l'ordre où les prendre.
#[derive(Default)]
struct File {
    jobs: HashMap<CelId, Job>,
    /// Les demandes interactives, la plus récente à la fin (elle passe la première), et celles du chargement, la plus
    /// ancienne devant : (dessin, numéro de la demande). Une entrée dont la demande a été remplacée ou retirée est sautée.
    interactives: Vec<(CelId, u64)>,
    chargement: VecDeque<(CelId, u64)>,
    fermee: bool,
    /// Le fil des gestes rend une demande : un fil du chargement libre peut alors prendre la demande interactive suivante.
    /// Libre, le fil des gestes les prend toutes (un fil du chargement, sous la priorité de l'interface, ralentirait un
    /// geste quand les cœurs sont pris).
    interactif_occupe: bool,
}

impl File {
    fn mettre(&mut self, job: Job) {
        let k = (job.cel, job.gen);
        if job.priority {
            self.interactives.push(k);
        } else {
            self.chargement.push_back(k);
        }
        self.jobs.insert(job.cel, job);
    }

    fn retirer(&mut self, cel: CelId) {
        self.jobs.remove(&cel);
        // (Les entrées d'ordre restent, sautées plus tard ; une file vide les oublie toutes.)
        if self.jobs.is_empty() {
            self.interactives.clear();
            self.chargement.clear();
        }
    }

    fn vider(&mut self) {
        self.jobs.clear();
        self.interactives.clear();
        self.chargement.clear();
    }

    /// La demande interactive la plus récente ; sinon, pour un fil du chargement (`chargement`), la plus ancienne du
    /// chargement. Un fil du chargement ne prend une demande interactive que si le fil des gestes est occupé.
    fn prendre(&mut self, chargement: bool) -> Option<Job> {
        if !chargement || self.interactif_occupe {
            while let Some((cel, gen)) = self.interactives.pop() {
                if self.jobs.get(&cel).is_some_and(|j| j.gen == gen) {
                    return self.jobs.remove(&cel);
                }
            }
        }
        if !chargement {
            return None;
        }
        while let Some((cel, gen)) = self.chargement.pop_front() {
            if self.jobs.get(&cel).is_some_and(|j| j.gen == gen) {
                return self.jobs.remove(&cel);
            }
        }
        None
    }

    /// Reste-t-il une demande interactive valable ?
    fn interactive_en_attente(&self) -> bool {
        self.interactives.iter().any(|(cel, gen)| self.jobs.get(cel).is_some_and(|j| j.gen == *gen))
    }
}

/// La file, et ce qui réveille ses fils : celui des gestes (`interactif`), ceux du chargement (`chargement`).
#[derive(Default)]
struct Reveils {
    file: Mutex<File>,
    interactif: Condvar,
    chargement: Condvar,
}

type Boite = Arc<Reveils>;

/// Mesures : un autre nombre de fils du chargement (0 : celui de `fils_de_chargement`), pour comparer avec un seul fil.
#[cfg(test)]
pub(crate) static FILS_DE_CHARGEMENT_ESSAI: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Fils du chargement : les cœurs moins deux (l'interface et le fil des gestes), au moins un.
fn fils_de_chargement() -> usize {
    #[cfg(test)]
    {
        let n = FILS_DE_CHARGEMENT_ESSAI.load(std::sync::atomic::Ordering::Relaxed);
        if n > 0 {
            return n;
        }
    }
    crate::parallele::coeurs().saturating_sub(2).max(1)
}

/// Le rendu exact `done` peut-il remplacer l'image de `cel` ? Oui si le dessin est celui qui a été
/// rendu : même version, mêmes éléments.
pub fn adoptable(cel: &crate::document::Cel, done: &Done) -> bool {
    cel.version == done.version
        && cel.vector.as_ref().is_some_and(|d| d.elements.len() == done.drawing.elements.len() && d.elements.iter().zip(&done.drawing.elements).all(|(a, b)| a.same_as(b)))
}

/// La file des rejeux exacts en attente, et ses fils de travail.
pub struct Rejeu {
    boite: Boite,
    done_tx: Sender<Msg>,
    rx: Receiver<Msg>,
    /// Le fil des demandes interactives (toujours là, à la priorité de l'interface).
    worker: JoinHandle<()>,
    /// Les fils du chargement, lancés à la première demande du chargement (un onglet qui n'ouvre rien n'en a pas).
    chargement: Vec<JoinHandle<()>>,
    /// Combien de fils du chargement au plus (`None` : les cœurs moins deux, au plafond des Préférences du moment).
    fils_chargement: Option<usize>,
    render: Renderer,
    next_gen: u64,
    /// Les dessins dont un rejeu est attendu : numéro de la demande et version du dessin.
    pending: HashMap<CelId, (u64, u64)>,
    /// Les dessins dont le rendu a paniqué, pas encore signalés.
    failed: Vec<CelId>,
}

impl Default for Rejeu {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Rejeu {
    fn drop(&mut self) {
        if let Ok(mut f) = self.boite.file.lock() {
            f.fermee = true;
        }
        self.boite.interactif.notify_all();
        self.boite.chargement.notify_all();
    }
}

/// Le rendu d'un dessin (VectorDrawing::render, ou un autre pour les essais).
pub type Renderer = fn(&VectorDrawing, usize, usize) -> Raster;

/// Un fil de travail : celui des gestes (`chargement` faux) ne prend que les demandes interactives ; ceux du chargement
/// prennent d'abord une demande interactive s'il en reste une pendant que le fil des gestes est occupé, sinon la plus
/// ancienne du chargement, sous la priorité de l'interface.
fn spawn_worker(boite: Boite, done_tx: Sender<Msg>, render: Renderer, chargement: bool) -> JoinHandle<()> {
    thread::Builder::new()
        .name(if chargement { "rejeu-chargement".into() } else { "rejeu".into() })
        .spawn(move || {
            if chargement {
                crate::parallele::fil_sous_la_normale();
            }
            loop {
                let job = {
                    let mut f = boite.file.lock().unwrap_or_else(|e| e.into_inner());
                    if !chargement {
                        f.interactif_occupe = false;
                    }
                    loop {
                        if f.fermee {
                            return;
                        }
                        if let Some(job) = f.prendre(chargement) {
                            if !chargement {
                                // Occupé : une autre demande interactive qui attend peut aller à un fil du chargement libre.
                                f.interactif_occupe = true;
                                if f.interactive_en_attente() {
                                    boite.chargement.notify_one();
                                }
                            }
                            break job;
                        }
                        let cv = if chargement { &boite.chargement } else { &boite.interactif };
                        f = cv.wait(f).unwrap_or_else(|e| e.into_inner());
                    }
                };
                let t = Instant::now();
                let (cel, gen, version) = (job.cel, job.gen, job.version);
                let msg = match catch_unwind(AssertUnwindSafe(|| render(&job.drawing, job.w, job.h))) {
                    Ok(raster) => Msg::Done(Done { cel, gen, version, drawing: job.drawing, raster, took: t.elapsed() }),
                    Err(_) => Msg::Failed { cel, gen },
                };
                if done_tx.send(msg).is_err() {
                    return;
                }
            }
        })
        .expect("fil de rejeu")
}

impl Rejeu {
    pub fn new() -> Self {
        Self::with_renderer(VectorDrawing::render)
    }

    /// Avec un autre rendu (essais : un rendu qui panique).
    pub fn with_renderer(render: Renderer) -> Self {
        Self::avec(render, None)
    }

    /// Avec `fils_chargement` fils pour la file du chargement (essais).
    #[cfg(test)]
    pub fn with_renderer_and_threads(render: Renderer, fils_chargement: usize) -> Self {
        Self::avec(render, Some(fils_chargement.max(1)))
    }

    fn avec(render: Renderer, fils_chargement: Option<usize>) -> Self {
        let boite: Boite = Default::default();
        let (done_tx, rx) = channel::<Msg>();
        let worker = spawn_worker(boite.clone(), done_tx.clone(), render, false);
        Rejeu { boite, done_tx, rx, worker, chargement: Vec::new(), fils_chargement, render, next_gen: 1, pending: HashMap::new(), failed: Vec::new() }
    }

    /// Demande le rendu exact de `drawing` (l'image `w` × `h` du dessin `cel`), interactive : une
    /// copie, dont les pixels en cache déformés (aperçus) sont oubliés pour que le fil les refasse
    /// exactement. Remplace toute demande en attente pour ce dessin.
    #[allow(dead_code)]
    pub fn submit(&mut self, cel: CelId, drawing: &VectorDrawing, w: usize, h: usize) -> u64 {
        self.submit_with(cel, drawing, w, h, 0, true)
    }

    /// Comme `submit`, avec la version du dessin (rendue avec le résultat) et la priorité
    /// (`false` : chargement d'un projet, derrière tout geste).
    pub fn submit_with(&mut self, cel: CelId, drawing: &VectorDrawing, w: usize, h: usize, version: u64, priority: bool) -> u64 {
        self.submit_batch(vec![Demande::new(cel, drawing, version)], w, h, priority);
        self.next_gen - 1
    }

    /// Plusieurs demandes d'un coup (l'ouverture d'un projet : des milliers), déjà copiées : la file est prise une fois,
    /// ses fils réveillés une fois. Chacune remplace la demande en attente pour son dessin.
    pub fn submit_batch(&mut self, demandes: Vec<Demande>, w: usize, h: usize, priority: bool) {
        if demandes.is_empty() {
            return;
        }
        // Un fil mort (panic hors du rendu) est recréé ; ceux du chargement naissent à sa première demande.
        if self.worker.is_finished() {
            self.worker = spawn_worker(self.boite.clone(), self.done_tx.clone(), self.render, false);
        }
        if !priority {
            self.chargement.retain(|h| !h.is_finished());
            let fils = self.fils_chargement.unwrap_or_else(fils_de_chargement);
            while self.chargement.len() < fils {
                self.chargement.push(spawn_worker(self.boite.clone(), self.done_tx.clone(), self.render, true));
            }
        }
        let n = demandes.len();
        let occupe = {
            let mut f = self.boite.file.lock().unwrap_or_else(|e| e.into_inner());
            for d in demandes {
                let gen = self.next_gen;
                self.next_gen += 1;
                self.pending.insert(d.cel, (gen, d.version));
                f.mettre(Job { cel: d.cel, gen, version: d.version, priority, drawing: d.drawing, w, h });
            }
            f.interactif_occupe
        };
        let cv = if priority { &self.boite.interactif } else { &self.boite.chargement };
        if n == 1 {
            cv.notify_one();
        } else {
            cv.notify_all();
        }
        // (Une demande interactive va à un fil du chargement libre seulement si celui des gestes est occupé : libre, il la
        // prend lui-même, à la priorité de l'interface. S'il prend une demande et qu'il en reste, il réveille un fil du
        // chargement lui-même.)
        if priority && occupe && !self.chargement.is_empty() {
            self.boite.chargement.notify_one();
        }
    }

    /// Le dessin a été rendu directement (ou remplacé) : un rejeu en attente ne vaut plus.
    pub fn cancel(&mut self, cel: CelId) {
        if self.pending.remove(&cel).is_some() {
            self.boite.file.lock().unwrap_or_else(|e| e.into_inner()).retirer(cel);
        }
    }

    /// Plus rien à attendre (autre projet).
    pub fn cancel_all(&mut self) {
        self.pending.clear();
        self.boite.file.lock().unwrap_or_else(|e| e.into_inner()).vider();
    }

    #[allow(dead_code)]
    pub fn is_pending(&self, cel: CelId) -> bool {
        self.pending.contains_key(&cel)
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Les dessins dont un rejeu est attendu, avec la version du dessin demandée.
    pub fn pending_versions(&self) -> Vec<(CelId, u64)> {
        self.pending.iter().map(|(&c, &(_, v))| (c, v)).collect()
    }

    /// Les dessins dont le rendu a paniqué depuis la dernière fois (à redemander ou signaler).
    pub fn take_failed(&mut self) -> Vec<CelId> {
        std::mem::take(&mut self.failed)
    }

    fn accept(&mut self, m: Msg) -> Option<Done> {
        match m {
            Msg::Done(d) => {
                if self.pending.get(&d.cel).map(|p| p.0) == Some(d.gen) {
                    self.pending.remove(&d.cel);
                    Some(d)
                } else {
                    None
                }
            }
            Msg::Failed { cel, gen } => {
                if self.pending.get(&cel).map(|p| p.0) == Some(gen) {
                    self.pending.remove(&cel);
                    self.failed.push(cel);
                }
                None
            }
        }
    }

    /// Les rendus arrivés et encore valables (les caducs sont jetés).
    pub fn poll(&mut self) -> Vec<Done> {
        let mut out = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(m) => out.extend(self.accept(m)),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        out
    }

    /// Attend (au plus `timeout`) qu'un rendu valable arrive ; rend ceux arrivés. Pour les essais.
    #[allow(dead_code)]
    pub fn wait(&mut self, timeout: Duration) -> Vec<Done> {
        let deadline = Instant::now() + timeout;
        let mut out = Vec::new();
        while out.is_empty() && self.has_pending() {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            match self.rx.recv_timeout(deadline - now) {
                Ok(m) => out.extend(self.accept(m)),
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
            }
        }
        out.extend(self.poll());
        out
    }
}

/// Les dessins dont un rejeu est attendu et dont la version n'est plus celle demandée (`version_of`
/// donne la version actuelle d'un dessin, `None` s'il n'existe plus) : ce que l'historique ou une
/// modification directe a dépassé. Les autres rejeux continuent.
pub fn outdated(rejeu: &Rejeu, version_of: impl Fn(CelId) -> Option<u64>) -> Vec<CelId> {
    rejeu.pending_versions().into_iter().filter(|&(cel, v)| version_of(cel) != Some(v)).map(|(cel, _)| cel).collect()
}

/// Le rendu exact du dessin `cel` est adopté (sa version passe de `before` à `after`) : si son
/// fichier était à jour, il l'est encore (la géométrie n'a pas changé, les pixels en cache ne
/// s'enregistrent pas), donc le premier enregistrement après l'ouverture ne réécrit pas tout.
pub fn keep_saved(written: &mut HashMap<CelId, u64>, cel: CelId, before: u64, after: u64) {
    if written.get(&cel) == Some(&before) {
        written.insert(cel, after);
    }
}