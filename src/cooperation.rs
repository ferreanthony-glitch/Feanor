//! La session de travail à plusieurs (`docs/COOPERATION.md`, §5 à §10).
//!
//! Deux moitiés qui ne se parlent que par messages :
//!
//! - le **moteur** ([`Moteur`]) tourne sur un fil de fond : il balaie le dépôt, lit les fiches des équipiers, décide qui tient
//!   quoi (`verrous.rs`), lit et décode les plans qui ont changé, fusionne les commentaires, écrit ma fiche (le bail). Il ne
//!   touche jamais au document ;
//! - la **session** ([`Session`]) est du côté de l'interface : elle tient mes prises, pose dans le document ce que le moteur a
//!   préparé (un plan lu, la structure, les commentaires), et répond aux questions de l'interface (« puis-je modifier ce
//!   plan ? ») sans aucune entrée-sortie.
//!
//! Le moteur n'a pas besoin de son fil pour fonctionner : [`Session::pomper`] fait un tour à la main, ce qui rend les essais
//! déterministes (deux clients, un dossier, une horloge qu'on avance).

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::depot::{CopieEnConflit, Curseur, Depot, Membre};
use crate::document::{CelId, Document};
use crate::equipe::Identite;
use crate::partagecom::{self, Enregistrement, Suivi};
use crate::project::partage_io::{self, BlocPartage, Brouillon, ContexteEcriture, DonneesPlan, EtatPlan, Ouverture, PlanCharge, PlanTenu, ResultatPartage, Signature};
use crate::verrous::{self, Cible, EtatPrise, Evaluation, Fiche, Observations, Presence, Prise, Remplace, Rendue};

// ---------------------------------------------------------------------------------- réglages et horloge

/// Les durées du protocole.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Delais {
    /// Une prise sans battement depuis ce temps est périmée.
    pub bail: Duration,
    /// Entre deux balayages du dépôt.
    pub balayage: Duration,
    /// Entre deux battements de ma fiche.
    pub battement: Duration,
    /// Une prise toute récente reste « provisoire » ce temps (une prise plus ancienne chez l'autre peut encore arriver).
    pub confirmation: Duration,
    /// Au plus une écriture de la fiche pour un simple changement de plan regardé.
    pub presence: Duration,
    /// Après ce temps d'attente, une prise qui attend son plan propose de forcer.
    pub patience: Duration,
}

impl Default for Delais {
    fn default() -> Self {
        Delais { bail: Duration::from_secs(600), balayage: Duration::from_secs(3), battement: Duration::from_secs(30), confirmation: Duration::from_secs(8), presence: Duration::from_secs(5), patience: Duration::from_secs(60) }
    }
}

/// L'heure, que les essais peuvent régler.
#[derive(Clone, Debug, Default)]
pub(crate) struct Horloge(Option<Arc<AtomicU64>>);

impl Horloge {
    pub fn reelle() -> Horloge {
        Horloge(None)
    }

    /// Une horloge réglée à la main, à `ms` (les essais).
    #[cfg(test)]
    pub fn reglable(ms: u64) -> Horloge {
        Horloge(Some(Arc::new(AtomicU64::new(ms))))
    }

    pub fn maintenant(&self) -> u64 {
        match &self.0 {
            Some(a) => a.load(Ordering::Relaxed),
            None => crate::equipe::maintenant_ms(),
        }
    }

    /// Avance une horloge réglable.
    #[cfg(test)]
    pub fn avancer(&self, ms: u64) {
        if let Some(a) = &self.0 {
            a.fetch_add(ms, Ordering::Relaxed);
        }
    }
}

// ---------------------------------------------------------------------------------- messages

/// Ce que l'interface demande au moteur.
enum Commande {
    /// J'ai pris cette cible (`pret` : je n'ai rien à attendre, c'est mon propre plan ou il est à jour).
    Prendre { prise: Prise, pret: bool },
    /// Je rends cette cible, avec la révision de ce que j'ai écrit en dernier.
    Rendre { cible: Cible, revision: u64 },
    /// La révision de ce que j'ai écrit pour une cible.
    Revision { cible: Cible, revision: u64 },
    /// Un plan que j'ai écrit : le moteur ne le relira pas comme s'il venait d'un autre.
    PlanEcrit { plan: u64, revision: u64, hash_fichier: u64 },
    StructureEcrite { revision: u64, hash_fichier: u64 },
    Presence { plan: Option<u64>, mode: String },
    /// Relire ce plan (même s'il n'a pas l'air d'avoir changé).
    Recharger(u64),
    /// Cette copie en conflit est traitée.
    CopieTraitee(CopieEnConflit),
    /// Cette prise n'attend plus son fichier (la synchronisation est trop lente : on prend l'état d'ici).
    Forcer(Cible),
    /// Écrire tout de suite ma fiche et balayer.
    Reveiller,
    Arret,
}

/// Ce que le moteur dit à l'interface.
pub(crate) enum Evenement {
    /// Un plan lu, décodé, à poser.
    PlanRecu(Box<PlanCharge>),
    /// Un plan dont le fichier est illisible pour l'instant, ou dont des dessins manquent.
    PlanEnAttente { plan: u64, raison: String },
    /// La structure du montage a changé ailleurs.
    StructureRecue(Box<StructureRecue>),
    /// Les commentaires de chacun (fichiers lus).
    Commentaires(Vec<(String, Vec<Enregistrement>)>),
    /// Ce plan (ce montage) que j'attendais est à jour : on peut le modifier.
    Pret(Cible),
    /// J'avais pris cette cible, quelqu'un d'autre l'a avant moi (ou m'a repris).
    PrisePerdue { cible: Cible, par: Option<Signature> },
    /// Quelqu'un a pris, rendu ou repris quelque chose.
    Changement { cible: Cible, avant: Option<String>, apres: Option<String> },
    /// Un autre poste a écrit dans une cible que je tiens.
    Intrus { cible: Cible, par: Option<Signature> },
}

/// La structure lue : un petit document (plans en coquille) à confronter au vrai.
pub(crate) struct StructureRecue {
    pub doc: Document,
    pub bloc: BlocPartage,
    pub hash_fichier: u64,
}

// ---------------------------------------------------------------------------------- ce que l'interface voit

/// Un équipier tel que l'interface le montre.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EquipierVu {
    pub id: String,
    pub nom: String,
    pub couleur: [u8; 3],
    pub en_ligne: bool,
    pub moi: bool,
    pub plan: Option<u64>,
    pub mode: String,
    pub tient: Vec<Cible>,
}

/// Ce que la session sait d'un plan du film (hors prises).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct InfoPlan {
    pub attente: Option<String>,
    pub incomplet: bool,
    pub revision: u64,
    pub signature: Option<Signature>,
}

/// L'instantané publié par le moteur : lu par l'interface, jamais modifié.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Vue {
    /// L'état de chaque cible que quelqu'un a prise (les autres sont libres).
    pub etats: BTreeMap<Cible, EtatPrise>,
    /// Le plus grand numéro de révision annoncé pour chaque cible.
    pub annonces: BTreeMap<Cible, u64>,
    pub equipe: Vec<EquipierVu>,
    pub plans: BTreeMap<u64, InfoPlan>,
    pub copies: Vec<CopieEnConflit>,
    pub brouillons: Vec<String>,
    /// Le dépôt ne répond pas (dossier réseau coupé…) : le message.
    pub hors_ligne: Option<String>,
    /// Prises qui attendent leur fichier depuis plus que la patience.
    pub synchro_lente: BTreeSet<Cible>,
    /// Combien de fois la vue a été publiée (l'interface sait qu'elle a changé).
    pub numero: u64,
}

impl Vue {
    pub fn etat(&self, cible: Cible) -> &EtatPrise {
        self.etats.get(&cible).unwrap_or(&EtatPrise::Libre)
    }
}

/// Un message pour la liste de la fenêtre Équipe.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Notification {
    pub quand: u64,
    pub texte: String,
}

// ---------------------------------------------------------------------------------- le moteur

/// Le suivi d'un plan côté moteur.
#[derive(Clone, Debug, Default)]
struct PlanSuivi {
    revision: u64,
    hash_fichier: u64,
    attente: Option<String>,
    incomplet: bool,
    signature: Option<Signature>,
    /// Quand relire ce plan (illisible, ou des dessins manquent).
    reessayer_a: Option<u64>,
    essais: u32,
}

/// Mon exemplaire de ma fiche, tenu à jour par les commandes de l'interface.
#[derive(Clone, Debug, Default)]
struct MaFiche {
    prises: BTreeMap<Cible, Prise>,
    rendues: BTreeMap<Cible, Rendue>,
    presence: Option<(Option<u64>, String)>,
}

pub(crate) struct Moteur {
    depot: Arc<dyn Depot>,
    moi: Identite,
    prefixe: u64,
    delais: Delais,
    horloge: Horloge,
    curseur: Curseur,
    rx: Receiver<Commande>,
    tx: Sender<Evenement>,
    vue: Arc<Mutex<Arc<Vue>>>,
    reveil: Option<eframe::egui::Context>,
    fiche: MaFiche,
    sequence: u64,
    derniere_ecriture: u64,
    derniere_presence_ecrite: Option<(Option<u64>, String)>,
    derniere_presence_a: u64,
    fiche_a_ecrire: bool,
    autres: Vec<Fiche>,
    membres: Vec<Membre>,
    observations: Observations,
    plans: BTreeMap<u64, PlanSuivi>,
    tete: crate::project::Tete,
    structure_revision: u64,
    structure_hash_fichier: u64,
    structure_a_reessayer: bool,
    sons: HashMap<u64, crate::document::Sound>,
    modeles: HashMap<u64, Arc<crate::model3d::Model3d>>,
    commentaires_hash: u64,
    commentaires_a_relire: bool,
    copies: Vec<CopieEnConflit>,
    brouillons: Vec<String>,
    en_attente_de_pret: BTreeMap<Cible, u64>,
    pertes_signalees: BTreeSet<Cible>,
    precedents: BTreeMap<Cible, Option<(String, String)>>,
    hors_ligne: Option<String>,
    publiee: Option<Vue>,
    numero: u64,
    premier_tour: bool,
    fiches_a_relire: bool,
    arret: bool,
}

impl Moteur {
    fn ma_fiche(&self, now: u64) -> Fiche {
        Fiche {
            format: verrous::FORMAT_FICHE.into(),
            version: verrous::VERSION_FICHE,
            equipier: verrous::Equipier { id: self.moi.id.clone(), nom: self.moi.nom.clone(), couleur: self.moi.couleur, machine: self.moi.machine.clone(), prefixe: self.prefixe },
            seance: self.moi.seance.clone(),
            renouvele_le: now,
            sequence: self.sequence,
            prises: self.fiche.prises.values().cloned().collect(),
            rendues: self.fiche.rendues.values().cloned().collect(),
            presence: self.fiche.presence.as_ref().map(|(plan, mode)| Presence { plan: *plan, mode: mode.clone(), vu_le: now }),
        }
    }

    fn envoyer(&self, e: Evenement) {
        let _ = self.tx.send(e);
        if let Some(c) = &self.reveil {
            c.request_repaint();
        }
    }

    /// Un tour : les commandes, le balayage, les lectures, le battement, la vue. Rend faux quand il faut s'arrêter.
    pub(crate) fn tour(&mut self) -> bool {
        let now = self.horloge.maintenant();
        while let Ok(c) = self.rx.try_recv() {
            self.commande(c, now);
        }
        if self.arret {
            self.ecrire_fiche(now);
            return false;
        }
        match self.depot.surveiller(&mut self.curseur) {
            Ok(releve) => {
                self.hors_ligne = None;
                self.suite_du_balayage(releve, now);
            }
            Err(e) => self.hors_ligne = Some(e),
        }
        self.premier_tour = false;
        self.battement(now);
        self.publier(now);
        true
    }

    fn commande(&mut self, c: Commande, now: u64) {
        match c {
            Commande::Prendre { prise, pret } => {
                let (cible, pris_le) = (prise.cible, prise.pris_le);
                self.fiche.rendues.remove(&cible);
                self.fiche.prises.insert(cible, prise);
                self.pertes_signalees.remove(&cible);
                if !pret {
                    self.en_attente_de_pret.insert(cible, pris_le.min(now));
                }
                self.fiche_a_ecrire = true;
            }
            Commande::Rendre { cible, revision } => {
                self.fiche.prises.remove(&cible);
                self.en_attente_de_pret.remove(&cible);
                self.pertes_signalees.remove(&cible);
                self.fiche.rendues.insert(cible, Rendue { cible, le: now, revision });
                self.fiche_a_ecrire = true;
            }
            Commande::Revision { cible, revision } => {
                if let Some(p) = self.fiche.prises.get_mut(&cible) {
                    p.revision = revision;
                    self.fiche_a_ecrire = true;
                }
            }
            Commande::PlanEcrit { plan, revision, hash_fichier } => {
                let s = self.plans.entry(plan).or_default();
                s.revision = revision;
                s.hash_fichier = hash_fichier;
                s.attente = None;
                s.incomplet = false;
                s.reessayer_a = None;
            }
            Commande::StructureEcrite { revision, hash_fichier } => {
                self.structure_revision = revision;
                self.structure_hash_fichier = hash_fichier;
            }
            Commande::Presence { plan, mode } => {
                self.fiche.presence = Some((plan, mode));
            }
            Commande::Recharger(plan) => {
                let s = self.plans.entry(plan).or_default();
                s.hash_fichier = 0;
                s.reessayer_a = Some(now);
            }
            Commande::CopieTraitee(c) => self.copies.retain(|x| x != &c),
            Commande::Forcer(c) => {
                self.en_attente_de_pret.remove(&c);
            }
            Commande::Reveiller => {
                self.fiche_a_ecrire = true;
                self.fiches_a_relire = true;
            }
            Commande::Arret => self.arret = true,
        }
    }

    fn tiens(&self, cible: Cible) -> bool {
        self.fiche.prises.contains_key(&cible)
    }

    fn suite_du_balayage(&mut self, releve: crate::depot::Releve, now: u64) {
        if releve.fiches || releve.membres || self.premier_tour || self.fiches_a_relire {
            self.fiches_a_relire = false;
            self.relire_fiches(now);
        }
        for p in &releve.plans {
            // (Je ne relis pas un plan que je tiens et dont j'ai vérifié le fichier : c'est le mien. Un plan que je viens de
            // prendre et que j'attends, si.)
            let c = Cible::Plan(*p);
            if !self.tiens(c) || self.en_attente_de_pret.contains_key(&c) {
                self.plans.entry(*p).or_default().reessayer_a = Some(now);
            }
        }
        // Les plans à lire : changés, ou à réessayer.
        let a_lire: Vec<u64> = self.plans.iter().filter(|(_, s)| s.reessayer_a.is_some_and(|t| t <= now)).map(|(p, _)| *p).collect();
        for p in a_lire {
            self.lire_plan(p, now);
        }
        if releve.structure || self.structure_a_reessayer {
            self.lire_structure(now);
        }
        if releve.commentaires || self.commentaires_a_relire || self.premier_tour {
            self.lire_commentaires();
        }
        if releve.brouillons || self.premier_tour {
            self.brouillons = self.depot.lister_brouillons().unwrap_or_default();
        }
        for c in releve.copies {
            if !self.copies.contains(&c) {
                self.copies.push(c);
            }
        }
        self.verifier_les_prets(now);
    }

    fn relire_fiches(&mut self, now: u64) {
        let Ok(fiches) = self.depot.lire_fiches() else { return };
        if let Ok(m) = self.depot.lire_membres() {
            self.membres = m;
        }
        for f in &fiches {
            self.observations.voir(&f.cle(), f.sequence, now);
        }
        self.autres = fiches.into_iter().filter(|f| f.cle() != self.moi.cle()).collect();
    }

    /// Lit le plan `p` (si son fichier a changé) et le donne à l'interface.
    fn lire_plan(&mut self, p: u64, now: u64) {
        let suivi = self.plans.entry(p).or_default().clone();
        let fichier = match self.depot.lire_plan(p) {
            Ok(Some(f)) => f,
            Ok(None) => {
                self.plans.get_mut(&p).expect("suivi").reessayer_a = None;
                return;
            }
            Err(e) => {
                self.reessayer(p, now, Some(e));
                return;
            }
        };
        let hash = crate::empreinte::de_octets(&fichier.octets);
        if hash == suivi.hash_fichier && suivi.attente.is_none() && !suivi.incomplet {
            self.plans.get_mut(&p).expect("suivi").reessayer_a = None;
            return;
        }
        match partage_io::charger_plan(self.depot.racine(), &self.tete, p, &fichier.octets, true) {
            Ok(pc) => {
                let incomplet = pc.incomplet();
                let s = self.plans.get_mut(&p).expect("suivi");
                s.hash_fichier = hash;
                s.revision = pc.revision;
                s.signature = pc.signature.clone();
                s.incomplet = incomplet;
                s.attente = incomplet.then(|| "des dessins du plan ne sont pas encore arrivés".to_string());
                let retenter = incomplet;
                if retenter {
                    self.reessayer(p, now, Some("des dessins du plan ne sont pas encore arrivés".into()));
                } else {
                    let s = self.plans.get_mut(&p).expect("suivi");
                    s.reessayer_a = None;
                    s.essais = 0;
                }
                // L'interface reçoit aussi le plan incomplet (le plan s'affiche tel qu'il est), marqué en attente.
                self.envoyer(Evenement::PlanRecu(Box::new(pc)));
            }
            Err(raison) => self.reessayer(p, now, Some(raison)),
        }
    }

    /// Un fichier à moitié arrivé, ou des dessins qui manquent : on réessaie, de plus en plus tard.
    fn reessayer(&mut self, p: u64, now: u64, raison: Option<String>) {
        let attente = self.delais.balayage.as_millis() as u64;
        let s = self.plans.entry(p).or_default();
        s.essais += 1;
        s.reessayer_a = Some(now + (attente * u64::from(s.essais.min(20))).min(60_000));
        if let Some(r) = raison {
            if s.attente.as_deref() != Some(&r) {
                s.attente = Some(r.clone());
                self.envoyer(Evenement::PlanEnAttente { plan: p, raison: r });
            }
        }
    }

    fn lire_structure(&mut self, _now: u64) {
        let Ok(Some(f)) = self.depot.lire_structure() else { return };
        let hash = crate::empreinte::de_octets(&f.octets);
        if hash == self.structure_hash_fichier {
            self.structure_a_reessayer = false;
            return;
        }
        if self.tiens(Cible::Montage) {
            // Je tiens le montage : l'écriture d'un autre poste est une anomalie (une course perdue, un client en retard).
            self.structure_a_reessayer = false;
            self.structure_hash_fichier = hash;
            let par = serde_json::from_slice::<serde_json::Value>(&f.octets).ok().and_then(|v| serde_json::from_value::<Signature>(v["partage"]["enregistre_par"].clone()).ok());
            self.envoyer(Evenement::Intrus { cible: Cible::Montage, par });
            return;
        }
        match partage_io::charger_structure(self.depot.racine(), &f.octets, &self.sons, &self.modeles) {
            Ok(rec) => {
                self.structure_a_reessayer = false;
                self.structure_hash_fichier = hash;
                self.structure_revision = rec.bloc.revision;
                self.sons.extend(rec.doc.sounds.iter().map(|(k, v)| (*k, v.clone())));
                self.modeles.extend(rec.doc.models.iter().map(|(k, v)| (*k, v.clone())));
                // Les plans que la structure cite et que je ne connais pas : à lire.
                let now = self.horloge.maintenant();
                for pl in &rec.bloc.plans {
                    if !self.plans.contains_key(&pl.plan) {
                        self.plans.insert(pl.plan, PlanSuivi { reessayer_a: Some(now), ..Default::default() });
                    }
                }
                self.envoyer(Evenement::StructureRecue(Box::new(StructureRecue { doc: rec.doc, bloc: rec.bloc, hash_fichier: hash })));
            }
            // (Le fichier est peut-être à moitié arrivé, ou cite des fichiers pas encore là : plus tard.)
            Err(_) => self.structure_a_reessayer = true,
        }
    }

    fn lire_commentaires(&mut self) {
        let Ok(fichiers) = self.depot.lire_commentaires() else { return };
        let mut octets_tous: Vec<u8> = Vec::new();
        let mut par_auteur = Vec::new();
        for (nom, o) in &fichiers {
            octets_tous.extend(o);
            if let Ok(v) = std::str::from_utf8(o).map_err(|e| e.to_string()).and_then(partagecom::lire) {
                par_auteur.push((nom.clone(), v));
            }
        }
        let hash = crate::empreinte::de_octets(&octets_tous);
        self.commentaires_a_relire = false;
        if hash != self.commentaires_hash {
            self.commentaires_hash = hash;
            self.envoyer(Evenement::Commentaires(par_auteur));
        }
    }

    /// Les prises qui attendaient leur fichier : prêtes quand le plan lu ici est celui que le porteur d'avant a écrit en dernier.
    fn verifier_les_prets(&mut self, now: u64) {
        let attend: Vec<Cible> = self.en_attente_de_pret.keys().copied().collect();
        for c in attend {
            let ev = self.evaluer(c, now);
            let pret = match c {
                Cible::Plan(p) => self.plans.get(&p).is_none_or(|s| s.revision >= ev.revision_annoncee && !s.incomplet && s.attente.is_none() && s.reessayer_a.is_none()),
                Cible::Montage => self.structure_revision >= ev.revision_annoncee && !self.structure_a_reessayer,
            };
            if pret {
                self.en_attente_de_pret.remove(&c);
                self.envoyer(Evenement::Pret(c));
            }
        }
    }

    fn toutes_les_fiches(&self, now: u64) -> Vec<Fiche> {
        let mut v = self.autres.clone();
        v.push(self.ma_fiche(now));
        v
    }

    fn evaluer(&self, cible: Cible, now: u64) -> Evaluation {
        verrous::evaluer(&self.toutes_les_fiches(now), cible, now, self.delais.bail.as_millis() as u64, &self.observations)
    }

    /// Ma fiche : réécrite quand elle a changé, et à chaque battement.
    fn battement(&mut self, now: u64) {
        let presence_changee = self.fiche.presence != self.derniere_presence_ecrite;
        let presence_ok = presence_changee && now.saturating_sub(self.derniere_presence_a) >= self.delais.presence.as_millis() as u64;
        let battement = now.saturating_sub(self.derniere_ecriture) >= self.delais.battement.as_millis() as u64;
        if self.fiche_a_ecrire || presence_ok || battement || self.derniere_ecriture == 0 {
            self.ecrire_fiche(now);
        }
    }

    fn ecrire_fiche(&mut self, now: u64) {
        self.sequence += 1;
        let f = self.ma_fiche(now);
        match self.depot.ecrire_fiche(&f) {
            Ok(()) => {
                self.derniere_ecriture = now;
                self.derniere_presence_a = now;
                self.derniere_presence_ecrite = self.fiche.presence.clone();
                self.fiche_a_ecrire = false;
                self.hors_ligne = None;
            }
            Err(e) => {
                self.hors_ligne = Some(e);
                // (On réessaiera au prochain tour.)
                self.fiche_a_ecrire = true;
            }
        }
    }

    /// Calcule la vue, signale les changements et la publie si elle a changé.
    fn publier(&mut self, now: u64) {
        let bail = self.delais.bail.as_millis() as u64;
        let fiches = self.toutes_les_fiches(now);
        let mut cibles: BTreeSet<Cible> = self.plans.keys().map(|p| Cible::Plan(*p)).collect();
        cibles.insert(Cible::Montage);
        for f in &fiches {
            cibles.extend(f.prises.iter().map(|p| p.cible));
        }
        let moi = self.moi.cle();
        let confirmation = self.delais.confirmation.as_millis() as u64;
        let mut etats = BTreeMap::new();
        let mut annonces = BTreeMap::new();
        let mut changements = Vec::new();
        let mut pertes = Vec::new();
        for c in cibles {
            let ev = verrous::evaluer(&fiches, c, now, bail, &self.observations);
            let ma_prise = self.fiche.prises.contains_key(&c);
            let etat = verrous::etat_pour(&moi, ma_prise, &ev, now, confirmation);
            // Qui tient maintenant, pour dire ce qui change.
            let titulaire = ev.titulaire.as_ref().filter(|t| t.cle != moi).map(|t| (t.equipier.nom.clone(), t.cle.seance.clone()));
            if self.precedents.get(&c) != Some(&titulaire) {
                let avant = self.precedents.get(&c).cloned().flatten();
                if !(self.precedents.is_empty() && titulaire.is_none()) && (self.precedents.contains_key(&c) || titulaire.is_some()) {
                    changements.push((c, avant.map(|a| a.0), titulaire.as_ref().map(|t| t.0.clone())));
                }
                self.precedents.insert(c, titulaire);
            }
            if let EtatPrise::Perdu(par) = &etat {
                if !self.pertes_signalees.contains(&c) {
                    pertes.push((c, par.as_ref().map(|r| Signature { id: r.equipier.id.clone(), nom: r.equipier.nom.clone(), le: r.prise.pris_le })));
                }
            }
            if !matches!(etat, EtatPrise::Libre) {
                etats.insert(c, etat);
            }
            if ev.revision_annoncee > 0 {
                annonces.insert(c, ev.revision_annoncee);
            }
        }
        if !self.premier_tour {
            for (cible, avant, apres) in changements {
                self.envoyer(Evenement::Changement { cible, avant, apres });
            }
        }
        for (cible, par) in pertes {
            self.pertes_signalees.insert(cible);
            self.envoyer(Evenement::PrisePerdue { cible, par });
        }
        // L'équipe : tous les équipiers vus, en ligne s'ils ont écrit récemment.
        let mut equipe: BTreeMap<String, EquipierVu> = BTreeMap::new();
        for f in &fiches {
            let moi_meme = f.cle() == moi;
            let en_ligne = moi_meme || f.renouvele_le + 90_000 >= now || self.observations.dernier_changement(&f.cle()).is_some_and(|t| now.saturating_sub(t) <= 90_000);
            let tient: Vec<Cible> = f.prises.iter().map(|p| p.cible).filter(|c| !f.rendues.iter().any(|r| r.cible == *c && f.prises.iter().any(|p| p.cible == *c && r.le >= p.pris_le))).collect();
            let entree = equipe.entry(format!("{}.{}", f.equipier.id, f.seance)).or_insert(EquipierVu { id: f.equipier.id.clone(), nom: f.equipier.nom.clone(), couleur: f.equipier.couleur, en_ligne, moi: moi_meme, plan: None, mode: String::new(), tient: Vec::new() });
            if let Some(p) = &f.presence {
                entree.plan = p.plan;
                entree.mode = p.mode.clone();
            }
            entree.tient = tient;
        }
        let mut equipe: Vec<EquipierVu> = equipe.into_values().collect();
        // Les équipiers hors ligne qui ne tiennent rien n'encombrent pas la liste.
        equipe.retain(|e| e.en_ligne || !e.tient.is_empty());
        equipe.sort_by(|a, b| b.moi.cmp(&a.moi).then(b.en_ligne.cmp(&a.en_ligne)).then(a.nom.cmp(&b.nom)));
        let patience = self.delais.patience.as_millis() as u64;
        let synchro_lente: BTreeSet<Cible> = self.en_attente_de_pret.iter().filter(|(_, depuis)| now.saturating_sub(**depuis) >= patience).map(|(c, _)| *c).collect();
        let plans = self.plans.iter().map(|(p, s)| (*p, InfoPlan { attente: s.attente.clone(), incomplet: s.incomplet, revision: s.revision, signature: s.signature.clone() })).collect();
        let vue = Vue { etats, annonces, equipe, plans, copies: self.copies.clone(), brouillons: self.brouillons.clone(), hors_ligne: self.hors_ligne.clone(), synchro_lente, numero: 0 };
        if self.publiee.as_ref() != Some(&vue) {
            self.numero += 1;
            let publiee = Vue { numero: self.numero, ..vue.clone() };
            *self.vue.lock().unwrap_or_else(|e| e.into_inner()) = Arc::new(publiee);
            self.publiee = Some(vue);
            if let Some(c) = &self.reveil {
                c.request_repaint();
            }
        }
    }
}

// ---------------------------------------------------------------------------------- la session

/// Pourquoi on ne peut pas modifier.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Refus {
    /// Le plan est à quelqu'un d'autre.
    PrisPar(String),
    /// La prise de quelqu'un est périmée : à reprendre (après confirmation).
    Perimee { nom: String, depuis_minutes: u64 },
    /// Personne ne le tient.
    Libre,
    /// Je viens de le prendre : je vérifie qu'il est à jour.
    Synchronisation,
    /// Le fichier du plan n'est pas (tout) arrivé.
    EnAttente(String),
    /// Le plan n'existe pas dans le film.
    Inconnu,
}

impl Refus {
    /// La phrase pour la barre d'état (`plan` : « le plan 12 »).
    pub(crate) fn phrase(&self, plan: &str) -> String {
        match self {
            Refus::PrisPar(n) => format!("{plan} est à {n}. Tu peux le regarder, le jouer et le commenter."),
            Refus::Perimee { nom, depuis_minutes } => format!("{plan} était à {nom}, sans nouvelles depuis {depuis_minutes} min : « Reprendre » (clic droit) si tu veux le prendre."),
            Refus::Libre => format!("{plan} est libre : clic droit > Prendre ce plan."),
            Refus::Synchronisation => format!("Un instant : je vérifie que {plan} est à jour avant de te le laisser."),
            Refus::EnAttente(r) => format!("{plan} n'est pas prêt : {r}."),
            Refus::Inconnu => format!("{plan} n'existe pas dans le film."),
        }
    }
}

/// Ce que la pastille d'un plan dit de sa prise (le montage et la planche la peignent).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Pastille {
    /// Je le tiens (provisoire : prise toute récente ; pret : son fichier est vérifié).
    Moi { provisoire: bool, pret: bool },
    Autre { nom: String, couleur: [u8; 3] },
    Perimee { nom: String, couleur: [u8; 3] },
    /// Le fichier du plan n'est pas (tout) arrivé.
    Attente,
}

/// La session d'un film partagé, côté interface.
pub(crate) struct Session {
    pub(crate) moi: Identite,
    pub(crate) projet: String,
    depot: Arc<dyn Depot>,
    delais: Delais,
    horloge: Horloge,
    cmd: Sender<Commande>,
    evt: Receiver<Evenement>,
    vue: Arc<Mutex<Arc<Vue>>>,
    moteur: Arc<Mutex<Moteur>>,
    fil: Option<JoinHandle<()>>,
    arret: Arc<AtomicBool>,
    /// Mes prises.
    mes_prises: BTreeMap<Cible, Prise>,
    /// Celles dont le fichier est à jour : on peut les modifier.
    prets: BTreeSet<Cible>,
    /// L'état de chaque plan tel que la dernière lecture ou écriture l'a laissé.
    pub(crate) plans: BTreeMap<u64, EtatPlan>,
    pub(crate) bloc: BlocPartage,
    pub(crate) hash_structure: u64,
    pub(crate) structure_revision: u64,
    structure_hash_fichier: u64,
    suivi: Suivi,
    /// Les plans reçus avant que la structure ne les cite.
    pour_plus_tard: HashMap<u64, PlanCharge>,
    /// Le dernier état reçu des plans que je ne tiens pas (pour que « annuler » ne ressuscite pas un vieux plan).
    derniers_recus: HashMap<u64, crate::document::Panel>,
    /// La version de chaque dessin d'un plan étranger quand je l'ai posé (pour voir qu'on y a touché).
    versions_posees: HashMap<u64, Vec<(CelId, u64)>>,
    pub(crate) notifications: VecDeque<Notification>,
    brouillons_a_ecrire: Vec<Brouillon>,
    /// Plans que je viens de rendre : leur état dans le document devient la base du garde-fou (à la prochaine pose).
    a_renoter: BTreeSet<u64>,
    /// Mes prises à rendre une fois l'enregistrement fini.
    rendre_apres: BTreeSet<Cible>,
    /// Les dessins à demander au rejeu (rendu exact) après la pose d'un plan.
    pub(crate) a_rejouer: Vec<CelId>,
    /// Ce que l'interface doit faire de ce que la session vient de poser.
    pub(crate) pertes: Vec<(Cible, String)>,
    /// ermer a été fait (une seule fois).
    ferme: bool,
    /// Ce que l'interface a dit regarder, pour n'envoyer la présence que quand elle change.
    presence_dite: Option<(Option<u64>, String)>,
    /// Le numéro d'historique à la dernière vérification des plans neufs (pres_structure).
    pub(crate) serial_vu: u64,
}

/// Ce que `sonder` a changé : l'interface réagit.
#[derive(Default, Debug)]
pub(crate) struct Effets {
    /// Des plans (ou la structure) ont été posés : caches à refaire.
    pub documents_changes: bool,
    /// Messages pour la barre d'état.
    pub messages: Vec<String>,
    /// Des prises perdues (l'historique est à vider).
    pub prises_perdues: Vec<Cible>,
    /// Les commentaires ont été fusionnés.
    pub commentaires: bool,
}

impl Session {
    /// Rejoint le film. Choisit (ou relit) mon préfixe de numéros, écrit ma fiche, démarre le moteur. `fil` : le moteur tourne
    /// sur un fil de fond (faux dans les essais, qui le font tourner à la main avec [`Session::pomper`]).
    pub(crate) fn rejoindre(depot: Arc<dyn Depot>, moi: Identite, o: Ouverture, doc: &mut Document, delais: Delais, horloge: Horloge, fil: bool, reveil: Option<eframe::egui::Context>) -> Result<Session, String> {
        let maintenant = horloge.maintenant();
        let membres = depot.lire_membres().unwrap_or_default();
        let fiches = depot.lire_fiches().unwrap_or_default();
        let prefixe = membres.iter().find(|m| m.equipier.id == moi.id).map(|m| m.equipier.prefixe).filter(|p| *p != 0).unwrap_or_else(|| {
            let pris: BTreeSet<u64> = membres.iter().map(|m| m.equipier.prefixe).chain(fiches.iter().map(|f| f.equipier.prefixe)).collect();
            loop {
                let p = crate::equipe::aleatoire() & 0xF_FFFF;
                if p != 0 && !pris.contains(&p) {
                    break p;
                }
            }
        });
        let equipier = verrous::Equipier { id: moi.id.clone(), nom: moi.nom.clone(), couleur: moi.couleur, machine: moi.machine.clone(), prefixe };
        let deja = membres.iter().find(|m| m.equipier.id == moi.id);
        if deja.is_none_or(|m| m.equipier != equipier) {
            depot.ecrire_membre(&Membre::neuf(equipier, deja.map_or(maintenant, |m| m.depuis)))?;
        }
        doc.set_id_prefix(prefixe);
        let (cmd, rx) = channel();
        let (tx, evt) = channel();
        let vue = Arc::new(Mutex::new(Arc::new(Vue::default())));
        let mut curseur = Curseur::default();
        // Les fichiers que l'ouverture vient de lire : le premier balayage ne signale que ce qui a changé depuis.
        for (p, e) in &o.plans {
            if let Some(m) = e.marque {
                curseur.poser(&format!("plan:{p}"), m);
            }
        }
        if let Some(m) = o.marque_structure {
            curseur.poser("structure", m);
        }
        let mut suivi = Suivi::default();
        let mon_fichier = o.commentaires.iter().find(|(q, _)| q == &moi.id);
        if let Some((_, enr)) = mon_fichier {
            suivi.miens = enr.iter().map(|e| (e.commentaire.id, e.clone())).collect();
        }
        suivi.base = partagecom::fusionner(&o.commentaires);
        let moteur = Moteur {
            depot: depot.clone(),
            moi: moi.clone(),
            prefixe,
            delais: delais.clone(),
            horloge: horloge.clone(),
            curseur,
            rx,
            tx,
            vue: vue.clone(),
            reveil,
            fiche: MaFiche::default(),
            sequence: 0,
            derniere_ecriture: 0,
            derniere_presence_ecrite: None,
            derniere_presence_a: 0,
            fiche_a_ecrire: true,
            autres: Vec::new(),
            membres,
            observations: Observations::default(),
            plans: o.plans.iter().map(|(p, e)| (*p, PlanSuivi { revision: e.revision, hash_fichier: e.hash_fichier, attente: e.attente.clone(), incomplet: e.incomplet, signature: e.signature.clone(), reessayer_a: (e.attente.is_some() || e.incomplet).then_some(maintenant), essais: 0 })).collect(),
            tete: o.tete.clone(),
            structure_revision: o.bloc.revision,
            structure_hash_fichier: o.hash_fichier_structure,
            structure_a_reessayer: false,
            sons: doc.sounds.clone(),
            modeles: doc.models.clone(),
            commentaires_hash: 0,
            commentaires_a_relire: false,
            copies: Vec::new(),
            brouillons: Vec::new(),
            en_attente_de_pret: BTreeMap::new(),
            pertes_signalees: BTreeSet::new(),
            precedents: BTreeMap::new(),
            hors_ligne: None,
            publiee: None,
            numero: 0,
            premier_tour: true,
            fiches_a_relire: true,
            arret: false,
        };
        let moteur = Arc::new(Mutex::new(moteur));
        let arret = Arc::new(AtomicBool::new(false));
        let mut s = Session {
            moi,
            projet: o.bloc.projet.clone(),
            depot,
            delais,
            horloge,
            cmd,
            evt,
            vue,
            moteur,
            fil: None,
            arret,
            mes_prises: BTreeMap::new(),
            prets: BTreeSet::new(),
            plans: o.plans,
            bloc: o.bloc,
            hash_structure: o.hash_structure,
            structure_revision: 0,
            structure_hash_fichier: o.hash_fichier_structure,
            suivi,
            pour_plus_tard: HashMap::new(),
            derniers_recus: HashMap::new(),
            versions_posees: HashMap::new(),
            notifications: VecDeque::new(),
            brouillons_a_ecrire: Vec::new(),
            a_renoter: BTreeSet::new(),
            rendre_apres: BTreeSet::new(),
            a_rejouer: Vec::new(),
            pertes: Vec::new(),
            ferme: false,
            presence_dite: None,
            serial_vu: 0,
        };
        s.structure_revision = s.bloc.revision;
        // Les plans tels que je les ai lus sont mon « dernier état reçu ».
        for p in &doc.panels {
            if !s.plans.get(&p.id).is_some_and(|e| e.attente.is_some()) {
                s.derniers_recus.insert(p.id, p.clone());
            }
        }
        s.noter_versions_posees(doc);
        // Un premier tour tout de suite : ma fiche est écrite, les fiches des autres sont lues.
        s.pomper();
        s.reprendre_mes_anciennes_prises();
        if fil {
            s.demarrer_le_fil();
        }
        Ok(s)
    }

    fn demarrer_le_fil(&mut self) {
        let moteur = self.moteur.clone();
        let arret = self.arret.clone();
        let balayage = self.delais.balayage;
        let fil = std::thread::Builder::new().name("cooperation".into()).spawn(move || {
            while !arret.load(Ordering::Relaxed) {
                let continuer = moteur.lock().unwrap_or_else(|e| e.into_inner()).tour();
                if !continuer {
                    break;
                }
                // (Dormir par petits pas : l'arrêt est pris en compte tout de suite.)
                let mut dormi = Duration::ZERO;
                while dormi < balayage && !arret.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(50));
                    dormi += Duration::from_millis(50);
                }
            }
        });
        self.fil = fil.ok();
    }

    /// Un tour du moteur, à la main (essais) — ou pour presser une lecture.
    pub(crate) fn pomper(&self) {
        self.moteur.lock().unwrap_or_else(|e| e.into_inner()).tour();
    }

    /// L'instantané courant.
    pub(crate) fn vue(&self) -> Arc<Vue> {
        self.vue.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub(crate) fn identite_signature(&self) -> Signature {
        Signature { id: self.moi.id.clone(), nom: self.moi.nom.clone(), le: self.horloge.maintenant() }
    }

    pub(crate) fn depot(&self) -> Arc<dyn Depot> {
        self.depot.clone()
    }

    // ------------------------------------------------------------------ mes prises

    /// Est-ce que je tiens cette cible (prise faite, même si son fichier n'est pas encore vérifié) ?
    pub(crate) fn tiens(&self, cible: Cible) -> bool {
        self.mes_prises.contains_key(&cible)
    }

    /// Ma prise est toute récente : une prise plus ancienne chez un autre peut encore arriver (§5.1).
    pub(crate) fn provisoire(&self, cible: Cible) -> bool {
        let now = self.horloge.maintenant();
        self.mes_prises.get(&cible).is_some_and(|p| now.saturating_sub(p.pris_le) < self.delais.confirmation.as_millis() as u64)
    }

    pub(crate) fn mes_cibles(&self) -> Vec<Cible> {
        self.mes_prises.keys().copied().collect()
    }

    /// Puis-je modifier ce plan ? Sinon pourquoi.
    pub(crate) fn peut_modifier_plan(&self, plan: u64) -> Result<(), Refus> {
        let v = self.vue();
        let c = Cible::Plan(plan);
        if let Some(e) = self.plans.get(&plan) {
            if let Some(r) = &e.attente {
                return Err(Refus::EnAttente(r.clone()));
            }
            if e.incomplet {
                return Err(Refus::EnAttente("des dessins du plan ne sont pas encore arrivés".into()));
            }
        }
        if let Some(i) = v.plans.get(&plan) {
            if let Some(r) = i.attente.as_ref().filter(|_| !self.tiens(c)) {
                return Err(Refus::EnAttente(r.clone()));
            }
        }
        if self.tiens(c) {
            return if self.prets.contains(&c) { Ok(()) } else { Err(Refus::Synchronisation) };
        }
        Err(self.refus_pour(&v, c))
    }

    /// Puis-je modifier le montage ?
    pub(crate) fn peut_modifier_montage(&self) -> Result<(), Refus> {
        let c = Cible::Montage;
        if self.tiens(c) {
            return if self.prets.contains(&c) { Ok(()) } else { Err(Refus::Synchronisation) };
        }
        Err(self.refus_pour(&self.vue(), c))
    }

    fn refus_pour(&self, v: &Vue, c: Cible) -> Refus {
        match v.etat(c) {
            EtatPrise::Autre(r) => Refus::PrisPar(r.equipier.nom.clone()),
            EtatPrise::Perimee(l) => {
                let r = &l[0];
                Refus::Perimee { nom: r.equipier.nom.clone(), depuis_minutes: self.horloge.maintenant().saturating_sub(r.renouvele_le) / 60_000 }
            }
            EtatPrise::Perdu(Some(r)) => Refus::PrisPar(r.equipier.nom.clone()),
            _ => Refus::Libre,
        }
    }

    /// Prend ces cibles. Chaque résultat dit pourquoi si elle est refusée. `reprendre` : confirmer qu'on reprend une prise
    /// périmée. `neuf` : ce sont des plans que je viens de créer (rien à vérifier).
    pub(crate) fn prendre(&mut self, cibles: &[Cible], reprendre: bool) -> Vec<(Cible, Result<(), Refus>)> {
        let v = self.vue();
        let now = self.horloge.maintenant();
        let mut rendu = Vec::new();
        for &c in cibles {
            if self.tiens(c) {
                rendu.push((c, Ok(())));
                continue;
            }
            if let Cible::Plan(p) = c {
                if !self.plans.contains_key(&p) && !v.plans.contains_key(&p) {
                    rendu.push((c, Err(Refus::Inconnu)));
                    continue;
                }
                if let Some(r) = self.plans.get(&p).and_then(|e| e.attente.clone()).or_else(|| v.plans.get(&p).and_then(|i| i.attente.clone())) {
                    rendu.push((c, Err(Refus::EnAttente(r))));
                    continue;
                }
            }
            let mut remplace = Vec::new();
            match v.etat(c) {
                EtatPrise::Libre | EtatPrise::Perdu(None) => {}
                EtatPrise::Perimee(l) => {
                    if !reprendre {
                        let r = &l[0];
                        rendu.push((c, Err(Refus::Perimee { nom: r.equipier.nom.clone(), depuis_minutes: now.saturating_sub(r.renouvele_le) / 60_000 })));
                        continue;
                    }
                    remplace = l.iter().map(|r| Remplace { equipier: r.cle.equipier.clone(), seance: r.cle.seance.clone(), pris_le: r.prise.pris_le }).collect();
                }
                EtatPrise::Autre(r) | EtatPrise::Perdu(Some(r)) => {
                    rendu.push((c, Err(Refus::PrisPar(r.equipier.nom.clone()))));
                    continue;
                }
                EtatPrise::Moi { .. } => {}
            }
            let annoncee = v.annonces.get(&c).copied().unwrap_or(0);
            let (lue, complet) = match c {
                Cible::Plan(p) => self.plans.get(&p).map_or((0, true), |e| (e.revision, !e.incomplet && e.attente.is_none())),
                Cible::Montage => (self.structure_revision, true),
            };
            let pret = lue >= annoncee && complet;
            let prise = Prise { cible: c, pris_le: now, revision: lue.max(annoncee), remplace };
            self.mes_prises.insert(c, prise.clone());
            if pret {
                self.prets.insert(c);
            } else {
                self.prets.remove(&c);
            }
            let _ = self.cmd.send(Commande::Prendre { prise, pret });
            rendu.push((c, Ok(())));
        }
        let _ = self.cmd.send(Commande::Reveiller);
        rendu
    }

    /// Après une opération du montage (nouveau plan, copie, coupe, suppression…) : les plans neufs sont à moi dès leur
    /// naissance, ceux qui ont disparu ne sont plus tenus. Rend les plans neufs.
    pub(crate) fn apres_structure(&mut self, doc: &Document) -> Vec<u64> {
        let presents: BTreeSet<u64> = doc.panels.iter().map(|p| p.id).collect();
        let neufs: Vec<(u64, u64)> = doc.panels.iter().filter(|p| !self.plans.contains_key(&p.id)).map(|p| (p.id, p.scene)).collect();
        for &(p, s) in &neufs {
            self.prendre_neuf(p, s);
        }
        let disparus: Vec<u64> = self.plans.keys().copied().filter(|p| !presents.contains(p)).collect();
        for p in &disparus {
            self.rendre(&[Cible::Plan(*p)]);
            self.plans.remove(p);
            self.derniers_recus.remove(p);
            self.versions_posees.remove(p);
        }
        neufs.into_iter().map(|(p, _)| p).collect()
    }

    /// Prend un plan que je viens de créer (copie, coupe, nouveau plan) : il est à moi dès sa naissance, sans rien à vérifier.
    pub(crate) fn prendre_neuf(&mut self, plan: u64, scene: u64) {
        let c = Cible::Plan(plan);
        // (Nul autre ne peut l'avoir pris : son numéro est de mon préfixe. Pas de confirmation à attendre.)
        let now = self.horloge.maintenant().saturating_sub(self.delais.confirmation.as_millis() as u64);
        let prise = Prise { cible: c, pris_le: now, revision: 0, remplace: Vec::new() };
        self.mes_prises.insert(c, prise.clone());
        self.prets.insert(c);
        self.plans.entry(plan).or_insert_with(|| EtatPlan { scene, ..Default::default() });
        let _ = self.cmd.send(Commande::Prendre { prise, pret: true });
        let _ = self.cmd.send(Commande::PlanEcrit { plan, revision: 0, hash_fichier: 0 });
    }

    /// Rend ces cibles. Ce que j'ai modifié et pas enregistré doit l'être avant (l'appelant s'en assure).
    pub(crate) fn rendre(&mut self, cibles: &[Cible]) {
        for &c in cibles {
            if self.mes_prises.remove(&c).is_some() {
                self.prets.remove(&c);
                self.rendre_apres.remove(&c);
                let revision = match c {
                    Cible::Plan(p) => {
                        self.a_renoter.insert(p);
                        self.plans.get(&p).map_or(0, |e| e.revision)
                    }
                    Cible::Montage => self.structure_revision,
                };
                let _ = self.cmd.send(Commande::Rendre { cible: c, revision });
            }
        }
        let _ = self.cmd.send(Commande::Reveiller);
    }

    /// Rend ces cibles quand l'enregistrement en cours (ou le prochain) sera fini.
    pub(crate) fn rendre_apres_enregistrement(&mut self, cibles: &[Cible]) {
        let miennes: Vec<Cible> = cibles.iter().copied().filter(|c| self.tiens(*c)).collect();
        self.rendre_apres.extend(miennes);
    }

    /// Mes prises plus anciennes (une séance qui s'est mal arrêtée, la mienne) : reprises sans question.
    fn reprendre_mes_anciennes_prises(&mut self) {
        let v = self.vue();
        let mut cibles = Vec::new();
        for (c, e) in &v.etats {
            if let EtatPrise::Perimee(l) = e {
                if l.iter().all(|r| r.cle.equipier == self.moi.id) {
                    cibles.push(*c);
                }
            }
        }
        if !cibles.is_empty() {
            self.prendre(&cibles, true);
        }
    }

    /// La synchronisation de cette prise est trop lente : on prend le plan tel qu'on l'a (ce que l'autre a écrit de plus
    /// arrivera comme une copie en conflit ou un brouillon).
    pub(crate) fn forcer_la_synchronisation(&mut self, cible: Cible) {
        if self.tiens(cible) {
            self.prets.insert(cible);
            let _ = self.cmd.send(Commande::Forcer(cible));
        }
    }

    /// Ce que je regarde (la présence, §9).
    pub(crate) fn regarde(&self, plan: Option<u64>, mode: &str) {
        let _ = self.cmd.send(Commande::Presence { plan, mode: mode.to_string() });
    }

    /// Comme `regarde`, mais n'écrit que quand ce que je regarde a changé (appelé à chaque image).
    pub(crate) fn regarde_si_change(&mut self, plan: Option<u64>, mode: &str) {
        let dit = (plan, mode.to_string());
        if self.presence_dite.as_ref() != Some(&dit) {
            self.regarde(plan, mode);
            self.presence_dite = Some(dit);
        }
    }

    /// Mes prises encore provisoires (une prise plus ancienne chez un autre peut encore arriver) : un enregistrement ne les écrit pas.
    pub(crate) fn en_confirmation(&self) -> Vec<Cible> {
        self.mes_prises.keys().copied().filter(|c| self.provisoire(*c)).collect()
    }

    /// Mes prises qui attendent que leur fichier soit à jour.
    pub(crate) fn en_verification(&self) -> Vec<Cible> {
        self.mes_prises.keys().copied().filter(|c| !self.prets.contains(c)).collect()
    }

    /// Les plans tenus par quelqu'un (pour les pastilles du montage) : plan → (équipier, ma prise ?).
    pub(crate) fn pastille(&self, plan: u64) -> Option<Pastille> {
        let v = self.vue();
        let c = Cible::Plan(plan);
        if let Some(i) = v.plans.get(&plan) {
            if i.attente.is_some() && !self.tiens(c) {
                return Some(Pastille::Attente);
            }
        }
        if self.tiens(c) {
            return Some(Pastille::Moi { provisoire: self.provisoire(c), pret: self.prets.contains(&c) });
        }
        match v.etat(c) {
            EtatPrise::Autre(r) => Some(Pastille::Autre { nom: r.equipier.nom.clone(), couleur: r.equipier.couleur }),
            EtatPrise::Perimee(l) => Some(Pastille::Perimee { nom: l[0].equipier.nom.clone(), couleur: l[0].equipier.couleur }),
            EtatPrise::Perdu(Some(r)) => Some(Pastille::Autre { nom: r.equipier.nom.clone(), couleur: r.equipier.couleur }),
            _ => None,
        }
    }

    pub(crate) fn recharger(&self, plan: u64) {
        let _ = self.cmd.send(Commande::Recharger(plan));
    }

    pub(crate) fn copie_traitee(&self, c: &CopieEnConflit) {
        let _ = self.cmd.send(Commande::CopieTraitee(c.clone()));
    }

    pub(crate) fn notifier(&mut self, texte: String) {
        self.notifications.push_back(Notification { quand: self.horloge.maintenant(), texte });
        while self.notifications.len() > 60 {
            self.notifications.pop_front();
        }
    }

    // ------------------------------------------------------------------ poser ce que le moteur a préparé

    /// À chaque image : pose dans le document ce que le moteur a préparé. Rapide (rien n'est décodé ici).
    pub(crate) fn sonder(&mut self, doc: &mut Document) -> Effets {
        let mut effets = Effets::default();
        // (Un plan rendu : ce que le document en montre devient la base de comparaison des retouches « sans prise ».)
        for p in std::mem::take(&mut self.a_renoter) {
            if !self.tiens(Cible::Plan(p)) {
                self.noter_versions_plan(p, doc);
            }
        }
        while let Ok(e) = self.evt.try_recv() {
            self.evenement(e, doc, &mut effets);
        }
        effets
    }

    fn titre(doc: &Document, plan: u64) -> String {
        match doc.panels.iter().position(|p| p.id == plan) {
            Some(i) => crate::storyboard::panel_title(doc, i).to_lowercase(),
            None => "un plan".into(),
        }
    }

    fn evenement(&mut self, e: Evenement, doc: &mut Document, fx: &mut Effets) {
        match e {
            Evenement::PlanRecu(pc) => self.poser_plan(*pc, doc, fx),
            Evenement::PlanEnAttente { plan, raison } => {
                self.plans.entry(plan).or_default().attente = Some(raison);
            }
            Evenement::StructureRecue(sr) => self.poser_structure(*sr, doc, fx),
            Evenement::Commentaires(fichiers) => {
                self.fusionner_commentaires(doc, &fichiers);
                fx.commentaires = true;
            }
            Evenement::Pret(c) => {
                if self.tiens(c) {
                    self.prets.insert(c);
                }
            }
            Evenement::PrisePerdue { cible, par } => self.perdre(cible, par, doc, fx),
            Evenement::Changement { cible, avant, apres } => {
                let t = match cible {
                    Cible::Plan(p) => Self::titre(doc, p),
                    Cible::Montage => "le montage".into(),
                };
                let texte = match (avant, apres) {
                    (_, Some(a)) => format!("{a} a pris {t}."),
                    (Some(a), None) => format!("{a} a rendu {t}."),
                    (None, None) => return,
                };
                self.notifier(texte);
            }
            Evenement::Intrus { cible, par } => {
                let t = match cible {
                    Cible::Plan(p) => Self::titre(doc, p),
                    Cible::Montage => "le montage".into(),
                };
                let qui = par.map_or("un autre poste".to_string(), |p| p.nom);
                let m = format!("{qui} a écrit dans {t} alors que tu le tiens : ton prochain enregistrement l'écrasera (son travail est dans le dossier du projet si tu en as besoin).");
                self.notifier(m.clone());
                fx.messages.push(m);
            }
        }
    }

    /// Un plan lu ailleurs : il remplace celui du document, sauf si je le tiens.
    fn poser_plan(&mut self, pc: PlanCharge, doc: &mut Document, fx: &mut Effets) {
        let plan = pc.plan;
        let c = Cible::Plan(plan);
        if self.tiens(c) && self.prets.contains(&c) {
            // (Un autre poste a écrit dans un plan que je tiens et que j'ai vérifié : je ne le remplace pas.)
            return;
        }
        let etat = EtatPlan {
            scene: pc.scene,
            revision: pc.revision,
            hash_contenu: pc.hash_contenu,
            hash_fichier: pc.hash_fichier,
            attente: pc.incomplet().then(|| "des dessins du plan ne sont pas encore arrivés".to_string()),
            incomplet: pc.incomplet(),
            signature: pc.signature.clone(),
            marque: None,
            dessins: pc.dessins.clone(),
        };
        // Ce que j'avais fait dans ce plan sans le tenir est mis de côté avant d'être remplacé (§10).
        if !self.tiens(c) {
            self.garde_fou_plan(plan, doc);
        }
        let signature = pc.signature.clone();
        let nouveau = !self.plans.get(&plan).is_some_and(|e| e.hash_fichier == pc.hash_fichier);
        self.a_rejouer.extend(pc.approx.iter().copied());
        let copie_panel = pc.panel.clone();
        match doc.installer_plan(pc) {
            Ok(_) => {
                self.derniers_recus.insert(plan, copie_panel);
                self.noter_versions_plan(plan, doc);
                self.plans.insert(plan, etat);
                fx.documents_changes = true;
                if nouveau {
                    if let Some(s) = signature.filter(|s| s.id != self.moi.id) {
                        let t = Self::titre(doc, plan);
                        self.notifier(format!("{} a enregistré {t}.", s.nom));
                    }
                }
            }
            Err(pc) => {
                // La structure ne cite pas encore ce plan : on le garde pour quand elle arrivera.
                self.plans.insert(plan, etat);
                self.pour_plus_tard.insert(plan, *pc);
            }
        }
    }

    /// Un équipier a changé la structure du montage.
    fn poser_structure(&mut self, sr: StructureRecue, doc: &mut Document, fx: &mut Effets) {
        if self.tiens(Cible::Montage) && self.prets.contains(&Cible::Montage) {
            return;
        }
        let StructureRecue { doc: stub, bloc, hash_fichier } = sr;
        if self.tiens(Cible::Montage) {
            // (Je viens de la prendre et elle n'est pas encore vérifiée : la structure lue est la base.)
        }
        let retires = doc.adopter_structure(stub, &mut self.pour_plus_tard);
        for p in &retires {
            if self.tiens(Cible::Plan(*p)) {
                let t = Self::titre(doc, *p);
                fx.messages.push(format!("{t} a été supprimé du montage alors que tu le tenais."));
            }
            self.plans.remove(p);
            self.derniers_recus.remove(p);
            self.versions_posees.remove(p);
            self.mes_prises.remove(&Cible::Plan(*p));
            self.prets.remove(&Cible::Plan(*p));
        }
        // Les plans reçus avant la structure se posent maintenant.
        let en_attente: Vec<u64> = self.pour_plus_tard.keys().copied().collect();
        for p in en_attente {
            if doc.panels.iter().any(|x| x.id == p) {
                if let Some(pc) = self.pour_plus_tard.remove(&p) {
                    self.poser_plan(pc, doc, fx);
                }
            }
        }
        for pl in &bloc.plans {
            self.plans.entry(pl.plan).or_insert_with(|| EtatPlan { scene: pl.scene, attente: Some("le fichier du plan n'est pas encore arrivé".into()), ..Default::default() }).scene = pl.scene;
        }
        // Les commentaires des plans qui viennent d'arriver.
        self.revoir_commentaires(doc);
        self.structure_revision = bloc.revision;
        self.structure_hash_fichier = hash_fichier;
        self.bloc = bloc;
        self.hash_structure = partage_io::empreinte_structure(doc, &self.projet).unwrap_or(0);
        if let Some(s) = self.bloc.enregistre_par.clone().filter(|s| s.id != self.moi.id) {
            self.notifier(format!("{} a modifié le montage.", s.nom));
        }
        fx.documents_changes = true;
    }

    /// J'avais pris cette cible, un autre est arrivé avant moi : mon travail va dans un brouillon, la cible reprend l'état des autres.
    fn perdre(&mut self, cible: Cible, par: Option<Signature>, doc: &mut Document, fx: &mut Effets) {
        let qui = par.as_ref().map_or("quelqu'un".to_string(), |p| p.nom.clone());
        if !self.tiens(cible) {
            return;
        }
        match cible {
            Cible::Plan(p) => {
                let t = Self::titre(doc, p);
                self.mettre_de_cote_plan(doc, p, "perdu");
                fx.messages.push(format!("{qui} avait pris {t} avant toi. Ton travail est gardé dans un brouillon que le porteur du plan peut importer comme version."));
                self.notifier(format!("Tu as perdu {t} au profit de {qui} : ton travail est mis de côté."));
                // Le plan reprend le contenu des autres dès qu'il arrive (et ce que j'y ai fait est déjà en lieu sûr).
                self.versions_posees.remove(&p);
                self.recharger(p);
                self.plans.entry(p).or_default().hash_fichier = 0;
            }
            Cible::Montage => {
                fx.messages.push(format!("{qui} avait pris le montage avant toi : tes changements du montage ne sont pas enregistrés."));
                self.notifier(format!("Tu as perdu le jeton du montage au profit de {qui}."));
            }
        }
        self.mes_prises.remove(&cible);
        self.prets.remove(&cible);
        let revision = match cible {
            Cible::Plan(p) => self.plans.get(&p).map_or(0, |e| e.revision),
            Cible::Montage => self.structure_revision,
        };
        let _ = self.cmd.send(Commande::Rendre { cible, revision });
        fx.prises_perdues.push(cible);
        self.pertes.push((cible, qui));
    }

    /// Met de côté le travail d'un plan : le fichier de plan complet, au nom du brouillon. Les dessins que j'ai modifiés ou
    /// créés depuis que j'ai reçu le plan prennent des numéros neufs (à moi) : ils s'écrivent à côté des autres sans rien
    /// écraser de ce que le porteur du plan a dans `dessins/`.
    fn mettre_de_cote_plan(&mut self, doc: &mut Document, plan: u64, raison: &str) {
        let Some(i) = doc.panels.iter().position(|p| p.id == plan) else { return };
        let mut data = DonneesPlan::de(doc, i, &BTreeMap::new());
        let vus: HashMap<CelId, u64> = self.versions_posees.get(&plan).map(|v| v.iter().copied().collect()).unwrap_or_default();
        let modifies: Vec<CelId> = data
            .cels()
            .into_iter()
            .filter(|id| match (doc.cels.get(id), vus.get(id)) {
                (Some(c), Some(v)) => c.version != *v,
                (Some(_), None) => true,
                _ => false,
            })
            .collect();
        let mut remap: HashMap<CelId, CelId> = HashMap::new();
        let mut nouveaux = BTreeSet::new();
        for id in modifies {
            let copie = doc.cels[&id].clone();
            let neuf = doc.alloc();
            doc.cels.insert(neuf, copie);
            remap.insert(id, neuf);
            nouveaux.insert(neuf);
        }
        partage_io::remapper_dessins(&mut data.panel, &remap);
        let scene = data.panel.scene;
        let Ok(fichier) = data.fichier(scene, self.plans.get(&plan).map_or(0, |e| e.revision), Some(self.identite_signature())) else { return };
        let Ok(octets) = fichier.octets() else { return };
        let nom = format!("plan-{plan}.{}.{}.{raison}.json", self.moi.id, self.horloge.maintenant());
        self.brouillons_a_ecrire.push(Brouillon { nom, octets, cels: nouveaux });
    }

    /// Les dessins d'un plan que je ne tiens pas ont-ils été touchés ? (Une retouche qui aurait passé les barrières.)
    fn garde_fou_plan(&mut self, plan: u64, doc: &mut Document) {
        let Some(vus) = self.versions_posees.get(&plan) else { return };
        let touche = vus.iter().any(|(id, v)| doc.cels.get(id).is_some_and(|c| c.version != *v));
        if touche {
            self.mettre_de_cote_plan(doc, plan, "sans-prise");
            let t = Self::titre(doc, plan);
            self.notifier(format!("Tu avais modifié {t} sans le tenir : ton travail est mis de côté dans un brouillon."));
        }
    }

    fn noter_versions_posees(&mut self, doc: &Document) {
        let plans: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
        for p in plans {
            self.noter_versions_plan(p, doc);
        }
    }

    fn noter_versions_plan(&mut self, plan: u64, doc: &Document) {
        if let Some(p) = doc.panels.iter().find(|x| x.id == plan) {
            let v: Vec<(CelId, u64)> = crate::empreinte::dessins_du_plan(p).into_iter().filter_map(|id| doc.cels.get(&id).map(|c| (id, c.version))).collect();
            self.versions_posees.insert(plan, v);
        }
    }

    /// Après « annuler » ou « rétablir » : les plans que je ne tiens pas reprennent leur dernier état reçu (§10).
    pub(crate) fn rafraichir_etranger(&mut self, doc: &mut Document) {
        let mut a_recharger = Vec::new();
        for i in 0..doc.panels.len() {
            let id = doc.panels[i].id;
            if self.tiens(Cible::Plan(id)) {
                continue;
            }
            if let Some(p) = self.derniers_recus.get(&id) {
                let scene = doc.panels[i].scene;
                let mut p = p.clone();
                p.scene = scene;
                doc.panels[i] = p;
            }
            if let Some(vus) = self.versions_posees.get(&id) {
                if vus.iter().any(|(c, v)| doc.cels.get(c).is_some_and(|x| x.version != *v)) {
                    a_recharger.push(id);
                }
            }
        }
        for p in a_recharger {
            self.recharger(p);
        }
        self.revoir_commentaires(doc);
    }

    // ------------------------------------------------------------------ les commentaires

    fn plans_existants(doc: &Document) -> std::collections::HashSet<u64> {
        doc.panels.iter().map(|p| p.id).collect()
    }

    /// Ce que j'ai changé dans les commentaires devient mes enregistrements.
    pub(crate) fn relever_commentaires(&mut self, doc: &Document) -> bool {
        let existent = Self::plans_existants(doc);
        self.suivi.relever(&doc.comments, &|p| existent.contains(&p), self.horloge.maintenant())
    }

    fn fusionner_commentaires(&mut self, doc: &mut Document, fichiers: &[(String, Vec<Enregistrement>)]) {
        self.relever_commentaires(doc);
        let existent = Self::plans_existants(doc);
        let fusion = self.suivi.fusionner(&self.moi.id, fichiers, &|p| existent.contains(&p));
        doc.comments = fusion;
    }

    /// Les commentaires qui n'avaient pas de plan (la structure n'était pas arrivée) apparaissent.
    fn revoir_commentaires(&mut self, doc: &mut Document) {
        self.relever_commentaires(doc);
        let existent = Self::plans_existants(doc);
        let mut tous = self.suivi.base.clone();
        for (id, e) in &self.suivi.miens {
            if e.supprime {
                tous.remove(id);
            } else {
                tous.insert(*id, e.commentaire.clone());
            }
        }
        doc.comments = tous.into_values().filter(|c| existent.contains(&c.panel)).collect();
    }

    // ------------------------------------------------------------------ enregistrer

    /// Ce que l'enregistrement doit savoir (`projet_partage::preparer`). Appelé sur le fil de l'interface, au cliché.
    /// `forcer` : on quitte, ce qui n'est pas écrit serait perdu — les prises encore en confirmation s'écrivent quand même.
    pub(crate) fn contexte_ecriture(&mut self, doc: &Document, forcer: bool) -> ContexteEcriture {
        let mut tenus = Vec::new();
        for (c, _) in self.mes_prises.iter() {
            let Cible::Plan(p) = c else { continue };
            // (Un plan pas encore vérifié, ou dont la prise est provisoire, ne s'écrit pas : §10.)
            if !self.prets.contains(c) || (self.provisoire(*c) && !forcer) {
                continue;
            }
            let Some(panel) = doc.panels.iter().find(|x| x.id == *p) else { continue };
            let e = self.plans.get(p).cloned().unwrap_or_default();
            tenus.push(PlanTenu { plan: *p, scene: panel.scene, revision: e.revision, hash: e.hash_contenu, dessins: e.dessins.clone() });
        }
        let jeton = (self.prets.contains(&Cible::Montage) && self.tiens(Cible::Montage) && (forcer || !self.provisoire(Cible::Montage))).then_some(self.structure_revision);
        self.relever_commentaires(doc);
        let commentaires = if self.suivi.a_ecrire { self.suivi.fichier(&self.moi.id).ok() } else { None };
        ContexteEcriture {
            moi: self.identite_signature(),
            projet: self.projet.clone(),
            depot: self.depot.clone(),
            tenus,
            jeton,
            commentaires,
            brouillons: std::mem::take(&mut self.brouillons_a_ecrire),
        }
    }

    /// L'écriture a réussi : les bases de comparaison avancent, mes prises annoncent les révisions écrites.
    pub(crate) fn enregistrement_fini(&mut self, r: &ResultatPartage, doc: &Document) {
        for w in &r.plans_ecrits {
            let (plan, revision, hash_fichier) = (w.plan, w.revision, w.hash_fichier);
            let e = self.plans.entry(plan).or_default();
            e.revision = revision;
            e.hash_contenu = w.hash_contenu;
            e.hash_fichier = hash_fichier;
            e.dessins = w.dessins.clone();
            e.attente = None;
            e.incomplet = false;
            let _ = self.cmd.send(Commande::PlanEcrit { plan, revision, hash_fichier });
            let _ = self.cmd.send(Commande::Revision { cible: Cible::Plan(plan), revision });
            if let Some(p) = self.mes_prises.get_mut(&Cible::Plan(plan)) {
                p.revision = revision;
            }
        }
        for w in &r.plans_inchanges {
            let e = self.plans.entry(w.plan).or_default();
            e.hash_contenu = w.hash_contenu;
            e.dessins = w.dessins.clone();
        }
        if let Some((revision, hash_fichier)) = r.structure {
            self.structure_revision = revision;
            self.structure_hash_fichier = hash_fichier;
            self.hash_structure = partage_io::empreinte_structure(doc, &self.projet).unwrap_or(0);
            let _ = self.cmd.send(Commande::StructureEcrite { revision, hash_fichier });
            let _ = self.cmd.send(Commande::Revision { cible: Cible::Montage, revision });
            if let Some(p) = self.mes_prises.get_mut(&Cible::Montage) {
                p.revision = revision;
            }
        }
        if r.commentaires_ecrits {
            self.suivi.a_ecrire = false;
        }
        let rendre: Vec<Cible> = std::mem::take(&mut self.rendre_apres).into_iter().collect();
        self.rendre(&rendre);
        let _ = self.cmd.send(Commande::Reveiller);
    }

    /// Mon fichier de commentaires est-il à écrire ?
    pub(crate) fn commentaires_a_ecrire(&self) -> bool {
        self.suivi.a_ecrire
    }

    // ------------------------------------------------------------------ brouillons et copies

    /// Importe un brouillon ou une copie en conflit comme version d'un plan que je tiens. Rend le numéro de la version.
    pub(crate) fn importer_comme_version(&mut self, doc: &mut Document, plan: u64, octets: &[u8], de: &str) -> Result<u32, String> {
        if !self.tiens(Cible::Plan(plan)) {
            return Err("prends d'abord le plan pour y importer une version".into());
        }
        let racine = self.depot.racine().to_path_buf();
        let tete = crate::project::Tete::de(doc);
        let pc = partage_io::charger_plan(&racine, &tete, plan, octets, false)?;
        let now = crate::equipe::maintenant_ms() / 1000;
        let i = doc.panels.iter().position(|p| p.id == plan).ok_or("plan inconnu")?;
        // Le contenu du fichier devient une version rangée : une copie de ses dessins sous des numéros à moi.
        let numero = doc.ranger_comme_version(i, pc.panel, pc.cels, de, now).ok_or("trop de versions pour ce plan")?;
        Ok(numero)
    }

    pub(crate) fn lire_brouillon(&self, nom: &str) -> Result<Vec<u8>, String> {
        self.depot.lire_brouillon(nom)
    }

    pub(crate) fn brouillon_importe(&self, nom: &str) {
        let _ = self.depot.ranger_brouillon(nom);
        let _ = self.cmd.send(Commande::Reveiller);
    }

    // ------------------------------------------------------------------ fermer

    /// Rend tout ce que je tiens, écrit ma fiche une dernière fois et arrête le moteur.
    pub(crate) fn fermer(&mut self) {
        if self.ferme {
            return;
        }
        self.ferme = true;
        let cibles: Vec<Cible> = self.mes_prises.keys().copied().collect();
        self.rendre(&cibles);
        let _ = self.cmd.send(Commande::Arret);
        if let Some(fil) = self.fil.take() {
            // (Le fil s'arrête en moins d'un pas de sommeil ; au pire on ne l'attend pas.)
            self.arret.store(true, Ordering::Relaxed);
            let debut = std::time::Instant::now();
            while !fil.is_finished() && debut.elapsed() < Duration::from_millis(1500) {
                std::thread::sleep(Duration::from_millis(10));
            }
            if fil.is_finished() {
                let _ = fil.join();
            }
        } else {
            // Pas de fil : un dernier tour à la main écrit les rendus.
            self.pomper();
        }
        // La fiche finale : le fil, arrêté d'un drapeau, n'a pas forcément vu la commande `Arret` ; on écrit ce qu'il faut.
        if let Ok(mut m) = self.moteur.lock() {
            let now = m.horloge.maintenant();
            while let Ok(c) = m.rx.try_recv() {
                m.commande(c, now);
            }
            m.ecrire_fiche(now);
        }
    }

}

impl Drop for Session {
    fn drop(&mut self) {
        self.fermer();
    }
}
