//! Tenir la mémoire d'un épisode : ranger les pixels des dessins qu'on ne regarde pas (chantier « mémoire d'un épisode »,
//! `docs/ENDURANCE.md` §6).
//!
//! Un épisode de 200 plans tenait 6,8 Go à l'ouverture et 9 à 10 Go après une séance, dont plus de 4 Go de tuiles de pixels que
//! personne ne regardait. Une tuile peut maintenant n'être tenue que par un tampon (`tuilepack.rs`, `Tile`) : une copie
//! **exacte** de ses pixels, dix fois plus petite pour du trait, qui se rouvre toute seule à la première lecture (une microseconde).
//! Rien n'est écrit sur le disque et rien n'est jeté : un dessin modifié et pas enregistré, un plan supprimé que l'annulation peut
//! rendre, un dessin que l'export va lire, tout reste exact ; seule la forme en mémoire change.
//!
//! Ici, la tenue : à chaque image d'écran, un petit tour des dessins du projet (au plus une demi-milliseconde) :
//! 1. les dessins **chauds** (le plan courant et ses voisins, les derniers plans visités, ce que la lecture va montrer) ne sont pas touchés ;
//! 2. pour les autres, les tuiles dont le tampon est fait et que le dessin est seul à tenir lâchent leurs pixels tout de suite ;
//! 3. les tuiles dont le tampon n'est pas fait (un dessin qu'on vient de regarder, de tracer) sont confiées à un fil de fond, sous la priorité de
//!    l'interface, qui en fait les tampons ; une fois fait, au tour suivant, leurs pixels partent.
//!
//! Une tuile que tient aussi une étape d'annulation, un cliché d'enregistrement ou une vignette en cours garde ses pixels jusqu'à ce
//! qu'elle soit seule : rien d'autre ne la protège, et rien n'en dépend. Aucun lecteur n'a à savoir : lire une tuile rangée la rouvre.

use std::collections::{HashSet, VecDeque};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};

use crate::document::{CelId, Document};
use crate::raster::Raster;

/// Temps accordé au tour des dessins, par image d'écran.
const TEMPS_PAR_IMAGE: Duration = Duration::from_micros(500);
/// Dessins confiés au fil de fond d'un coup.
const LOT: usize = 24;
/// Dessins en route chez le fil de fond, au plus (au-delà, on attend ses réponses).
const EN_ROUTE_MAX: usize = 256;
/// Quand un tour entier n'a plus rien trouvé à ranger, on attend ce temps avant d'en refaire un.
const REPOS: Duration = Duration::from_secs(2);
/// Plans visités gardés chauds en plus du plan courant et de ses voisins.
const RECENTS: usize = 4;
/// Plans gardés chauds de chaque côté du plan courant.
const VOISINS: usize = 2;
/// Secondes de lecture gardées chaudes devant la tête de lecture.
const LECTURE_S: usize = 3;

struct Lot {
    dessins: Vec<(CelId, Vec<Raster>)>,
}

/// L'état de la tenue des dessins d'un projet (un par application).
pub struct Gel {
    actif: bool,
    envoi: Option<Sender<Lot>>,
    retour: Option<Receiver<Vec<CelId>>>,
    en_route: HashSet<CelId>,
    ids: Vec<CelId>,
    curseur: usize,
    /// Dans ce tour, un dessin a eu des tuiles à ranger ou à lâcher.
    a_trouve: bool,
    repos_jusqu: Option<Instant>,
    /// Les derniers plans visités (leurs numéros), le plus récent devant.
    recents: VecDeque<u64>,
    /// Tuiles dont les pixels sont partis depuis le début (diagnostic et essais).
    pub lachees: u64,
    /// Dessins confiés au fil de fond depuis le début.
    pub confies: u64,
}

impl Default for Gel {
    fn default() -> Self {
        Gel {
            // `SHOT_SANS_GEL=1` : comme avant (mesures avant / après, ou si un jour un cas pose problème).
            actif: std::env::var_os("SHOT_SANS_GEL").is_none(),
            envoi: None,
            retour: None,
            en_route: HashSet::new(),
            ids: Vec::new(),
            curseur: 0,
            a_trouve: false,
            repos_jusqu: None,
            recents: VecDeque::new(),
            lachees: 0,
            confies: 0,
        }
    }
}

impl Gel {
    /// Essais : tenir ou ne pas tenir.
    #[cfg(test)]
    pub fn activer(&mut self, oui: bool) {
        self.actif = oui;
    }

    pub fn est_actif(&self) -> bool {
        self.actif
    }

    /// Reste-t-il du travail en route (le caller redemande une image d'écran bientôt) ?
    pub fn occupe(&self) -> bool {
        self.actif && (!self.en_route.is_empty() || (self.repos_jusqu.is_none() && (self.curseur < self.ids.len() || self.a_trouve)))
    }

    fn fil(&mut self) -> &Sender<Lot> {
        if self.envoi.is_none() {
            let (tx, rx) = channel::<Lot>();
            let (tx_retour, rx_retour) = channel::<Vec<CelId>>();
            let _ = std::thread::Builder::new().name("tuiles-rangees".into()).spawn(move || {
                crate::parallele::fil_sous_la_normale();
                while let Ok(lot) = rx.recv() {
                    let mut faits = Vec::with_capacity(lot.dessins.len());
                    for (id, rasters) in lot.dessins {
                        for r in &rasters {
                            r.ranger_les_tuiles();
                        }
                        // (Les copies sont lâchées avant la réponse : les dessins du projet redeviennent seuls à tenir leurs tuiles.)
                        drop(rasters);
                        faits.push(id);
                    }
                    if tx_retour.send(faits).is_err() {
                        return;
                    }
                }
            });
            self.envoi = Some(tx);
            self.retour = Some(rx_retour);
        }
        self.envoi.as_ref().expect("le fil vient d'être lancé")
    }

    /// Note que le plan de numéro `plan` est regardé (les plans visités en dernier restent chauds).
    pub fn visite(&mut self, plan: u64) {
        if self.recents.front() == Some(&plan) {
            return;
        }
        self.recents.retain(|&p| p != plan);
        self.recents.push_front(plan);
        self.recents.truncate(RECENTS);
    }

    /// Les dessins chauds : ceux du plan courant et de ses voisins, des derniers plans visités, et de ce que la lecture va montrer.
    pub fn chauds(&self, doc: &Document, courant: usize, lecture: Option<usize>) -> HashSet<CelId> {
        let mut plans: Vec<usize> = Vec::new();
        let n = doc.panels.len();
        if n > 0 {
            let c = courant.min(n - 1);
            plans.extend(c.saturating_sub(VOISINS)..=(c + VOISINS).min(n - 1));
        }
        for id in &self.recents {
            if let Some(i) = doc.panels.iter().position(|p| p.id == *id) {
                plans.push(i);
            }
        }
        if let Some(mut images) = lecture {
            // Ce que la lecture montrera dans les secondes qui viennent.
            let mut i = courant;
            while images > 0 && i < n {
                plans.push(i);
                images = images.saturating_sub(doc.panels[i].frame_count().max(1));
                i += 1;
            }
        }
        plans.sort_unstable();
        plans.dedup();
        let mut chauds = HashSet::new();
        for i in plans {
            for l in &doc.panels[i].layers {
                chauds.extend(l.frames.iter().flatten().copied());
            }
        }
        chauds
    }

    /// Le tour de cette image d'écran. `courant` : le plan courant ; `lecture` : la lecture est en cours (la cadence du projet).
    pub fn tenir(&mut self, doc: &mut Document, courant: usize, lecture: Option<u32>) {
        if !self.actif {
            return;
        }
        let debut = Instant::now();
        let lecture_images = lecture.map(|fps| fps as usize * LECTURE_S);
        let chauds = self.chauds(doc, courant, lecture_images);
        // 1. Les réponses du fil de fond : les tampons de ces dessins sont faits, leurs pixels peuvent partir.
        if let Some(rx) = &self.retour {
            loop {
                match rx.try_recv() {
                    Ok(ids) => {
                        for id in ids {
                            self.en_route.remove(&id);
                            if !chauds.contains(&id) {
                                if let Some(c) = doc.cels.get_mut(&id) {
                                    self.lachees += c.lacher_les_pixels() as u64;
                                }
                            }
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        // Le fil est mort : il sera relancé à la demande suivante.
                        self.envoi = None;
                        self.retour = None;
                        self.en_route.clear();
                        break;
                    }
                }
            }
        }
        if let Some(t) = self.repos_jusqu {
            if Instant::now() < t {
                return;
            }
            self.repos_jusqu = None;
        }
        // 2. Un nouveau tour : les numéros des dessins du projet.
        if self.curseur >= self.ids.len() {
            if !self.ids.is_empty() && !self.a_trouve {
                // Un tour entier qui n'a rien trouvé à ranger ni à lâcher : repos (le premier tour d'un projet repart tout de suite).
                self.repos_jusqu = Some(Instant::now() + REPOS);
                self.ids.clear();
                self.curseur = 0;
                return;
            }
            self.ids = doc.cels.keys().copied().collect();
            // (L'ordre du dictionnaire change d'un tour à l'autre : sans importance, tout est repris.)
            self.curseur = 0;
            self.a_trouve = false;
            if self.ids.is_empty() {
                self.repos_jusqu = Some(Instant::now() + REPOS);
                return;
            }
        }
        // 3. Le tour, tant qu'il reste du temps.
        let mut lot: Vec<(CelId, Vec<Raster>)> = Vec::new();
        let mut vus = 0usize;
        while self.curseur < self.ids.len() {
            if vus % 8 == 0 && vus > 0 && debut.elapsed() >= TEMPS_PAR_IMAGE {
                break;
            }
            vus += 1;
            let id = self.ids[self.curseur];
            self.curseur += 1;
            if chauds.contains(&id) || self.en_route.contains(&id) {
                continue;
            }
            let Some(c) = doc.cels.get_mut(&id) else { continue };
            if c.a_des_tuiles_lachables() {
                // (Une tuile que d'autres tiennent ne part pas : elle ne compte pas, sinon le tour ne finirait jamais.)
                let n = c.lacher_les_pixels();
                self.lachees += n as u64;
                self.a_trouve |= n > 0;
            }
            if c.a_des_tuiles_a_ranger() && self.en_route.len() + lot.len() < EN_ROUTE_MAX {
                lot.push((id, c.copies_des_images()));
                self.a_trouve = true;
                if lot.len() >= LOT {
                    self.envoyer(std::mem::take(&mut lot));
                }
            }
        }
        if !lot.is_empty() {
            self.envoyer(lot);
        }
    }

    fn envoyer(&mut self, lot: Vec<(CelId, Vec<Raster>)>) {
        let ids: Vec<CelId> = lot.iter().map(|(id, _)| *id).collect();
        self.confies += ids.len() as u64;
        let envoi = self.fil().send(Lot { dessins: lot });
        if envoi.is_ok() {
            self.en_route.extend(ids);
        } else {
            // (Le fil est mort : relancé au tour suivant ; les copies envoyées se perdent avec lui, sans effet sur les dessins.)
            self.envoi = None;
            self.retour = None;
        }
    }
}
