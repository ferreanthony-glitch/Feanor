//! Rejeu exact en arrière-plan (Anthony, 2026-10-18 : « ma priorité est d'avoir un logiciel qui
//! réponde toujours très rapidement, sans latence » ; règle : pendant un geste, jamais de rejeu du
//! moteur sur le fil de l'interface).
//!
//! Un trait vectoriel texturé se rend exactement en rejouant le moteur bitmap (`vector.rs`), ce qui
//! coûte quelques millisecondes par trait : trop pour le refaire à chaque image d'un glissé, et
//! trop pour bloquer l'interface au lâcher d'un dessin de cinquante traits. Pendant un geste, on
//! montre donc les pixels en cache, déformés à la volée (translation copiée, affine
//! rééchantillonnée, enveloppe et perspective par maillage) ; au lâcher, le rendu exact du dessin
//! est demandé ici, à un fil de travail, et l'aperçu reste affiché jusqu'à ce qu'il arrive.
//!
//! Chaque demande porte un numéro (`gen`) par dessin : une demande plus récente, ou un rendu
//! direct du dessin (`App::refresh_cel`, annulation), rend les précédentes caduques — le fil les
//! saute, et leurs résultats sont jetés. Un résultat adopté (`App::poll_rejeu`) remplace l'image
//! du dessin et donne aux traits leurs pixels exacts en cache (ceux du rendu), d'un coup.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::document::CelId;
use crate::raster::Raster;
use crate::vector::VectorDrawing;

struct Job {
    cel: CelId,
    gen: u64,
    drawing: VectorDrawing,
    w: usize,
    h: usize,
}

/// Un rendu exact terminé : le dessin tel qu'il a été rendu (ses traits ont leurs pixels en
/// cache) et son image.
pub struct Done {
    pub cel: CelId,
    pub gen: u64,
    pub drawing: VectorDrawing,
    pub raster: Raster,
    /// Durée du rendu, pour les mesures.
    #[allow(dead_code)]
    pub took: Duration,
}

type Latest = Arc<Mutex<HashMap<CelId, u64>>>;

/// La file des rejeux exacts en attente, et son fil de travail.
pub struct Rejeu {
    tx: Sender<Job>,
    rx: Receiver<Done>,
    /// Le dernier numéro demandé par dessin ; un rendu direct l'augmente aussi (plus rien à
    /// attendre).
    latest: Latest,
    next_gen: u64,
    /// Les dessins dont un rejeu est attendu, avec son numéro.
    pending: HashMap<CelId, u64>,
}

impl Default for Rejeu {
    fn default() -> Self {
        Self::new()
    }
}

impl Rejeu {
    pub fn new() -> Self {
        let (tx, jobs) = channel::<Job>();
        let (done_tx, rx) = channel::<Done>();
        let latest: Latest = Default::default();
        let seen = latest.clone();
        thread::Builder::new()
            .name("rejeu".into())
            .spawn(move || {
                for job in jobs {
                    let current = |job: &Job| seen.lock().map(|l| l.get(&job.cel) == Some(&job.gen)).unwrap_or(false);
                    if !current(&job) {
                        continue;
                    }
                    let t = Instant::now();
                    let raster = job.drawing.render(job.w, job.h);
                    if !current(&job) {
                        continue;
                    }
                    if done_tx.send(Done { cel: job.cel, gen: job.gen, drawing: job.drawing, raster, took: t.elapsed() }).is_err() {
                        return;
                    }
                }
            })
            .expect("fil de rejeu");
        Rejeu { tx, rx, latest, next_gen: 1, pending: HashMap::new() }
    }

    /// Demande le rendu exact de `drawing` (l'image `w` × `h` du dessin `cel`) : une copie, dont
    /// les pixels en cache sont oubliés pour que le fil les refasse exactement. Remplace toute
    /// demande en attente pour ce dessin.
    pub fn submit(&mut self, cel: CelId, drawing: &VectorDrawing, w: usize, h: usize) -> u64 {
        let gen = self.next_gen;
        self.next_gen += 1;
        let mut copy = drawing.clone();
        for e in &mut copy.elements {
            e.invalidate_cache();
        }
        if let Ok(mut l) = self.latest.lock() {
            l.insert(cel, gen);
        }
        self.pending.insert(cel, gen);
        let _ = self.tx.send(Job { cel, gen, drawing: copy, w, h });
        gen
    }

    /// Le dessin a été rendu directement (ou remplacé) : un rejeu en attente ne vaut plus.
    pub fn cancel(&mut self, cel: CelId) {
        if self.pending.remove(&cel).is_some() {
            if let Ok(mut l) = self.latest.lock() {
                l.insert(cel, self.next_gen);
                self.next_gen += 1;
            }
        }
    }

    /// Plus rien à attendre (annulation, autre projet).
    pub fn cancel_all(&mut self) {
        let cels: Vec<CelId> = self.pending.keys().copied().collect();
        for c in cels {
            self.cancel(c);
        }
    }

    #[allow(dead_code)]
    pub fn is_pending(&self, cel: CelId) -> bool {
        self.pending.contains_key(&cel)
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    fn accept(&mut self, d: Done) -> Option<Done> {
        if self.pending.get(&d.cel) == Some(&d.gen) {
            self.pending.remove(&d.cel);
            Some(d)
        } else {
            None
        }
    }

    /// Les rendus arrivés et encore valables (les caducs sont jetés).
    pub fn poll(&mut self) -> Vec<Done> {
        let mut out = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(d) => out.extend(self.accept(d)),
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
                Ok(d) => out.extend(self.accept(d)),
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
            }
        }
        out.extend(self.poll());
        out
    }
}
