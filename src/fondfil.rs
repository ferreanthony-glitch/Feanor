//! Fond sous le trait calculé en arrière-plan (Anthony, 2026-10-18 : « sans latence » ; sur un
//! calque à fond, le ralentissement commençait après une dizaine de traits).
//!
//! La forme du fond (`fond::silhouette` : distances exactes et remplissage, 30 à 100 ms en
//! 1920×1080, de plus en plus cher à mesure que le dessin grandit) puis sa composition
//! (`fond::compose`) ne se font plus sur le fil de l'interface quand le fond est affiché : le fond
//! précédent reste à l'écran, et le nouveau le remplace dès qu'il est prêt (`fx::fond_poll`). Le
//! résultat est exactement celui du calcul direct, qui reste celui de l'export et de
//! l'enregistrement.
//!
//! Même schéma que `rejeu.rs` : chaque demande porte un numéro (`gen`) par fond ; une demande plus
//! récente, ou un calcul direct, rend les précédentes caduques. Les demandes en attente sont
//! remplacées (une seule par fond, donc une mémoire bornée pendant un glissé qui en fait une par
//! image), et un résultat caduc est jeté à l'arrivée.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, TryRecvError};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::document::CelId;
use crate::fond::FondReglages;
use crate::raster::Raster;

/// Ce qu'il faut pour calculer un fond.
pub struct Demande {
    /// Numéro du fond (`fx::shown_id`).
    pub id: CelId,
    /// Version du fond affiché (dessin et strate Fond) : celle que le résultat porte.
    pub version: u64,
    /// Version du dessin dont la forme est tirée.
    pub src_version: u64,
    pub reglages: FondReglages,
    pub width: usize,
    pub height: usize,
    /// La forme, si elle est déjà connue pour cette version du dessin (seule la strate Fond a
    /// changé) ; sinon on la tire de `trait_`.
    pub forme: Option<Arc<Vec<u8>>>,
    pub trait_: Option<Raster>,
    pub peinture: Option<Raster>,
    pub coupe: Option<Raster>,
}

/// Un fond calculé.
pub struct Fini {
    pub id: CelId,
    pub gen: u64,
    pub version: u64,
    pub src_version: u64,
    pub forme: Arc<Vec<u8>>,
    pub raster: Raster,
    /// Durée du calcul, pour les mesures.
    #[allow(dead_code)]
    pub took: Duration,
}

/// Le calcul lui-même (celui du direct : `fx::fond_shown`).
pub fn calculer(d: &Demande) -> (Arc<Vec<u8>>, Raster) {
    let forme = match &d.forme {
        Some(f) => f.clone(),
        None => match &d.trait_ {
            Some(t) => Arc::new(crate::fond::silhouette(t, &d.reglages)),
            None => Arc::new(vec![0u8; d.width * d.height]),
        },
    };
    let raster = crate::fond::compose(&forme, d.width, d.height, d.reglages.couleur, d.peinture.as_ref(), d.coupe.as_ref());
    (forme, raster)
}

struct Job {
    gen: u64,
    demande: Demande,
}

#[derive(Default)]
struct File {
    jobs: Vec<Job>,
    fermee: bool,
}

type Boite = Arc<(Mutex<File>, Condvar)>;

/// La file des fonds en attente, et son fil de travail.
pub struct FondFil {
    boite: Boite,
    rx: Receiver<Fini>,
    next_gen: u64,
    /// Les fonds attendus : numéro de la demande et version demandée.
    pending: HashMap<CelId, (u64, u64)>,
}

impl Default for FondFil {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for FondFil {
    fn drop(&mut self) {
        let (m, cv) = &*self.boite;
        if let Ok(mut f) = m.lock() {
            f.fermee = true;
        }
        cv.notify_all();
    }
}

impl FondFil {
    pub fn new() -> Self {
        let boite: Boite = Default::default();
        let (done_tx, rx) = channel::<Fini>();
        let travail = boite.clone();
        thread::Builder::new()
            .name("fond".into())
            .spawn(move || loop {
                let job = {
                    let (m, cv) = &*travail;
                    let mut f = m.lock().unwrap_or_else(|e| e.into_inner());
                    loop {
                        if f.fermee {
                            return;
                        }
                        // La plus récente d'abord : les autres sont caduques ou le seront.
                        if let Some(j) = f.jobs.pop() {
                            break j;
                        }
                        f = cv.wait(f).unwrap_or_else(|e| e.into_inner());
                    }
                };
                let t = Instant::now();
                let (forme, raster) = calculer(&job.demande);
                let d = &job.demande;
                let fini = Fini { id: d.id, gen: job.gen, version: d.version, src_version: d.src_version, forme, raster, took: t.elapsed() };
                if done_tx.send(fini).is_err() {
                    return;
                }
            })
            .expect("fil du fond");
        FondFil { boite, rx, next_gen: 1, pending: HashMap::new() }
    }

    /// Demande un fond : remplace toute demande en attente pour ce fond.
    pub fn submit(&mut self, demande: Demande) -> u64 {
        let gen = self.next_gen;
        self.next_gen += 1;
        self.pending.insert(demande.id, (gen, demande.version));
        let (m, cv) = &*self.boite;
        {
            let mut f = m.lock().unwrap_or_else(|e| e.into_inner());
            f.jobs.retain(|j| j.demande.id != demande.id);
            f.jobs.push(Job { gen, demande });
        }
        cv.notify_one();
        gen
    }

    /// La version du fond `id` qu'on attend, s'il y en a une.
    pub fn pending_version(&self, id: CelId) -> Option<u64> {
        self.pending.get(&id).map(|p| p.1)
    }

    /// Le fond `id` a été calculé directement (ou n'est plus d'actualité) : ce qu'on en attend ne
    /// vaut plus.
    pub fn cancel(&mut self, id: CelId) {
        if self.pending.remove(&id).is_some() {
            let (m, _) = &*self.boite;
            m.lock().unwrap_or_else(|e| e.into_inner()).jobs.retain(|j| j.demande.id != id);
        }
    }

    /// Plus rien à attendre (autre projet).
    pub fn cancel_all(&mut self) {
        self.pending.clear();
        let (m, _) = &*self.boite;
        m.lock().unwrap_or_else(|e| e.into_inner()).jobs.clear();
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    fn accept(&mut self, d: Fini) -> Option<Fini> {
        if self.pending.get(&d.id).map(|p| p.0) == Some(d.gen) {
            self.pending.remove(&d.id);
            Some(d)
        } else {
            None
        }
    }

    /// Les fonds arrivés et encore valables (les caducs sont jetés).
    pub fn poll(&mut self) -> Vec<Fini> {
        let mut out = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(d) => out.extend(self.accept(d)),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        out
    }

    /// Attend (au plus `timeout`) qu'un fond valable arrive ; rend ceux arrivés. Pour les essais.
    #[allow(dead_code)]
    pub fn wait(&mut self, timeout: Duration) -> Vec<Fini> {
        let deadline = Instant::now() + timeout;
        let mut out = Vec::new();
        while out.is_empty() && self.has_pending() {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            match self.rx.recv_timeout(deadline - now) {
                Ok(d) => out.extend(self.accept(d)),
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
            }
        }
        out.extend(self.poll());
        out
    }
}
