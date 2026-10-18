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

use std::collections::HashMap;
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

#[derive(Default)]
struct File {
    jobs: Vec<Job>,
    fermee: bool,
}

type Boite = Arc<(Mutex<File>, Condvar)>;

/// Le rendu exact `done` peut-il remplacer l'image de `cel` ? Oui si le dessin est celui qui a été
/// rendu : même version, mêmes éléments.
pub fn adoptable(cel: &crate::document::Cel, done: &Done) -> bool {
    cel.version == done.version
        && cel.vector.as_ref().is_some_and(|d| d.elements.len() == done.drawing.elements.len() && d.elements.iter().zip(&done.drawing.elements).all(|(a, b)| a.same_as(b)))
}

/// La file des rejeux exacts en attente, et son fil de travail.
pub struct Rejeu {
    boite: Boite,
    done_tx: Sender<Msg>,
    rx: Receiver<Msg>,
    worker: JoinHandle<()>,
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
        let (m, cv) = &*self.boite;
        if let Ok(mut f) = m.lock() {
            f.fermee = true;
        }
        cv.notify_all();
    }
}

/// Le rendu d'un dessin (VectorDrawing::render, ou un autre pour les essais).
pub type Renderer = fn(&VectorDrawing, usize, usize) -> Raster;

fn spawn_worker(boite: Boite, done_tx: Sender<Msg>, render: Renderer) -> JoinHandle<()> {
    thread::Builder::new()
        .name("rejeu".into())
        .spawn(move || loop {
            let job = {
                let (m, cv) = &*boite;
                let mut f = m.lock().unwrap_or_else(|e| e.into_inner());
                loop {
                    if f.fermee {
                        return;
                    }
                    // Les demandes interactives d'abord (la plus récente), sinon la plus ancienne
                    // (l'ordre du chargement).
                    let at = f.jobs.iter().rposition(|j| j.priority).or(if f.jobs.is_empty() { None } else { Some(0) });
                    if let Some(i) = at {
                        break f.jobs.remove(i);
                    }
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
        })
        .expect("fil de rejeu")
}

impl Rejeu {
    pub fn new() -> Self {
        Self::with_renderer(VectorDrawing::render)
    }

    /// Avec un autre rendu (essais : un rendu qui panique).
    pub fn with_renderer(render: Renderer) -> Self {
        let boite: Boite = Default::default();
        let (done_tx, rx) = channel::<Msg>();
        let worker = spawn_worker(boite.clone(), done_tx.clone(), render);
        Rejeu { boite, done_tx, rx, worker, render, next_gen: 1, pending: HashMap::new(), failed: Vec::new() }
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
        // Un fil mort (panic hors du rendu) est recréé.
        if self.worker.is_finished() {
            self.worker = spawn_worker(self.boite.clone(), self.done_tx.clone(), self.render);
        }
        let gen = self.next_gen;
        self.next_gen += 1;
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
        self.pending.insert(cel, (gen, version));
        let (m, cv) = &*self.boite;
        {
            let mut f = m.lock().unwrap_or_else(|e| e.into_inner());
            f.jobs.retain(|j| j.cel != cel);
            f.jobs.push(Job { cel, gen, version, priority, drawing: copy, w, h });
        }
        cv.notify_one();
        gen
    }

    /// Le dessin a été rendu directement (ou remplacé) : un rejeu en attente ne vaut plus.
    pub fn cancel(&mut self, cel: CelId) {
        if self.pending.remove(&cel).is_some() {
            let (m, _) = &*self.boite;
            m.lock().unwrap_or_else(|e| e.into_inner()).jobs.retain(|j| j.cel != cel);
        }
    }

    /// Plus rien à attendre (autre projet).
    pub fn cancel_all(&mut self) {
        self.pending.clear();
        let (m, _) = &*self.boite;
        m.lock().unwrap_or_else(|e| e.into_inner()).jobs.clear();
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