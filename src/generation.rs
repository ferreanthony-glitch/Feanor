//! Le moteur de la génération sonore (Anthony, 2026-10-19) : les tâches qui lisent le storyboard et
//! génèrent les sons, **hors du fil de l'interface**, annulables, avec leur avancement.
//!
//! - **Une tâche = un fil** (`lancer_lecture`, `lancer_generation`) qui parle à un fournisseur
//!   (`fournisseurs.rs`) et rend ses résultats **au fur et à mesure** par un canal (`Msg`) : un plan lu,
//!   un son prêt. L'interface les reprend à chaque image d'écran (`Tache::poll`) et les range dans
//!   `Document::sons_proposes`. Annuler lève le drapeau du `Controle` : le fil s'arrête, la requête en
//!   vol est coupée.
//! - **Les sons générés** sont écrits par le fil de travail dans le cache de SHOT (`dossier_cache`),
//!   jamais par le fil de l'interface ; le projet n'en reçoit un que lorsqu'il est accepté.
//! - **Une erreur qui ne passera pas** (serveur absent, clé refusée, connexion à brancher) arrête la
//!   tâche ; une réponse bancale pour un plan ou un son laisse la suite continuer.
//! - **`Fabrique`** donne les fournisseurs selon les réglages ; les essais en glissent de faux.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;

use crate::clessecretes::{Coffre, Service};
use crate::fournisseurs::{Controle, DemandeLecture, DemandeSon, DemandeVoix, ErreurGen, GenerateurSon, GenerateurVoix, LecteurStoryboard};
use crate::fournisseursapi::{LecteurClaude, SonElevenLabs, SonStableAudio, Transport, VoixElevenLabs};
use crate::genreglages::{GenerationSonReglages, Mode, Role};
use crate::indications::{Fusion, Genre, IndicationBrute, SonsProposes, Statut};
use crate::lecturerules::LecteurRegles;
use crate::serveurlocal::{config_des_reglages, Demarrage, GestionnaireServeur};
use crate::sidecar::{ClientSidecar, LecteurSidecar, SonSidecar, VoixSidecar};

// ------------------------------------------------------------------------------------ fabrique

/// Donne le fournisseur de chaque rôle selon les réglages.
pub trait Fabrique: Send + Sync {
    fn lecteur(&self, reglages: &GenerationSonReglages) -> Arc<dyn LecteurStoryboard>;
    fn son(&self, reglages: &GenerationSonReglages) -> Arc<dyn GenerateurSon>;
    fn voix(&self, reglages: &GenerationSonReglages) -> Arc<dyn GenerateurVoix>;
}

/// Les vrais fournisseurs : le serveur local sur son port, ou un service en ligne par le transport donné.
pub struct FabriqueReelle {
    pub transport: Arc<dyn Transport>,
    pub coffre: Arc<dyn Coffre>,
    /// Le serveur local que SHOT lance au besoin (réglage `demarrage_auto`).
    pub serveur: Arc<GestionnaireServeur>,
}

impl FabriqueReelle {
    /// Le client du serveur local : il le lance tout seul s'il ne répond pas et que les réglages le permettent.
    fn client_local(&self, r: &GenerationSonReglages) -> ClientSidecar {
        let client = ClientSidecar::sur_le_port(r.port_local);
        if r.demarrage_auto {
            client.avec_demarrage(Demarrage { gestionnaire: self.serveur.clone(), config: config_des_reglages(r) })
        } else {
            client
        }
    }
}

impl Fabrique for FabriqueReelle {
    fn lecteur(&self, r: &GenerationSonReglages) -> Arc<dyn LecteurStoryboard> {
        match r.lecture {
            Mode::Regles => Arc::new(LecteurRegles),
            Mode::Local => Arc::new(LecteurSidecar(self.client_local(r))),
            Mode::EnLigne => Arc::new(LecteurClaude { transport: self.transport.clone(), coffre: self.coffre.clone(), modele: r.modele_claude.clone() }),
        }
    }

    fn son(&self, r: &GenerationSonReglages) -> Arc<dyn GenerateurSon> {
        match (r.son, r.son_service) {
            (Mode::EnLigne, Service::StableAudio) => Arc::new(SonStableAudio { transport: self.transport.clone(), coffre: self.coffre.clone() }),
            (Mode::EnLigne, _) => Arc::new(SonElevenLabs { transport: self.transport.clone(), coffre: self.coffre.clone() }),
            _ => Arc::new(SonSidecar(self.client_local(r))),
        }
    }

    fn voix(&self, r: &GenerationSonReglages) -> Arc<dyn GenerateurVoix> {
        match r.voix {
            Mode::EnLigne => Arc::new(VoixElevenLabs { transport: self.transport.clone(), coffre: self.coffre.clone() }),
            _ => Arc::new(VoixSidecar(self.client_local(r))),
        }
    }
}

// ------------------------------------------------------------------------------- les messages

/// Ce qu'un fil de travail rend à l'interface.
#[derive(Debug)]
pub enum Msg {
    /// Le plan `plan` est lu.
    Lecture { plan: u64, resultat: Result<Vec<IndicationBrute>, ErreurGen> },
    /// Le son de la proposition `cue` est prêt (le fichier écrit), ou n'a pas pu l'être.
    Son { cue: u64, resultat: Result<PathBuf, ErreurGen> },
}

/// Un son à générer, pris dans une proposition.
#[derive(Clone, Debug, PartialEq)]
pub struct Travail {
    pub cue: u64,
    pub genre: Genre,
    pub prompt: String,
    pub secondes: f32,
    pub texte: String,
    pub personnage: String,
    pub voix: String,
}

/// Une erreur après laquelle il est inutile de continuer (rien ne changera au plan suivant).
fn fatale(e: &ErreurGen) -> bool {
    !matches!(e, ErreurGen::Invalide(_))
}

/// Lit les plans de `demande` l'un après l'autre sur un fil à part.
pub fn lancer_lecture(lecteur: Arc<dyn LecteurStoryboard>, demande: DemandeLecture, controle: Controle) -> Receiver<Msg> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let n = demande.plans.len();
        for i in 0..n {
            if controle.est_annule() {
                break;
            }
            controle.avancer(i, n, &demande.plans[i].titre);
            let plan = demande.plans[i].plan;
            let resultat = lecteur.lire_plan(&demande, i, &controle);
            let arret = matches!(&resultat, Err(e) if fatale(e));
            if matches!(resultat, Err(ErreurGen::Annule)) || tx.send(Msg::Lecture { plan, resultat }).is_err() || arret {
                break;
            }
        }
        controle.avancer(n, n, "");
    });
    rx
}

/// Le cache des sons générés : `%LOCALAPPDATA%\SHOT\sons_generes`.
pub fn dossier_cache() -> PathBuf {
    crate::donnees::local().unwrap_or_else(std::env::temp_dir).join("sons_generes")
}

static COMPTEUR: AtomicU64 = AtomicU64::new(0);

/// Génère les sons de `travaux` l'un après l'autre sur un fil à part, chacun écrit dans `dossier`.
pub fn lancer_generation(son: Arc<dyn GenerateurSon>, voix: Arc<dyn GenerateurVoix>, travaux: Vec<Travail>, dossier: PathBuf, controle: Controle) -> Receiver<Msg> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let n = travaux.len();
        let serie = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        for (i, t) in travaux.into_iter().enumerate() {
            if controle.est_annule() {
                break;
            }
            controle.avancer(i, n, &t.texte_court());
            let genere = if t.genre.est_voix() {
                voix.generer(&DemandeVoix { texte: t.texte.clone(), personnage: t.personnage.clone(), voix: t.voix.clone(), langue: "fr".into() }, &controle)
            } else {
                son.generer(&DemandeSon { genre: t.genre, prompt: t.prompt.clone(), secondes: t.secondes }, &controle)
            };
            let resultat = genere.and_then(|g| {
                let chemin = dossier.join(format!("{serie:x}-{}-{}.{}", t.cue, COMPTEUR.fetch_add(1, Ordering::Relaxed), g.ext));
                std::fs::create_dir_all(&dossier).map_err(|e| ErreurGen::Invalide(format!("le cache des sons est inaccessible ({e})")))?;
                crate::project::write_atomic(&chemin, &g.octets).map_err(|e| ErreurGen::Invalide(format!("écriture du son impossible ({e})")))?;
                Ok(chemin)
            });
            let arret = matches!(&resultat, Err(e) if fatale(e));
            if matches!(resultat, Err(ErreurGen::Annule)) || tx.send(Msg::Son { cue: t.cue, resultat }).is_err() || arret {
                break;
            }
        }
        controle.avancer(n, n, "");
    });
    rx
}

impl Travail {
    fn texte_court(&self) -> String {
        let s = if self.genre.est_voix() { &self.texte } else { &self.prompt };
        s.chars().take(60).collect()
    }
}

// ------------------------------------------------------------------------------------- tâches

/// Ce que les tâches ont rapporté depuis la dernière fois que l'interface l'a lu.
#[derive(Debug, Default)]
pub struct Rapport {
    /// Les erreurs, en français, à dire.
    pub erreurs: Vec<String>,
    /// Les propositions ont changé (de quoi enregistrer le projet).
    pub change: bool,
}

/// Une tâche en cours : son fil, son contrôle, et de quoi remettre les propositions dans leur état
/// d'avant si elle s'arrête avant de les avoir toutes traitées.
pub struct Tache {
    pub role: Role,
    pub titre: String,
    pub controle: Controle,
    /// Ce que la tâche a fait jusqu'ici : propositions ajoutées, retirées, déjà connues (une lecture),
    /// sons prêts (une génération).
    pub totaux: Fusion,
    pub prets: usize,
    rx: Receiver<Msg>,
    /// Les propositions confiées à la tâche et l'état où elles étaient (rien, pour une lecture).
    avant: HashMap<u64, Statut>,
    traitees: HashSet<u64>,
}

impl Tache {
    pub fn new(role: Role, titre: String, controle: Controle, rx: Receiver<Msg>, avant: HashMap<u64, Statut>) -> Tache {
        Tache { role, titre, controle, totaux: Fusion::default(), prets: 0, rx, avant, traitees: HashSet::new() }
    }

    /// Reprend les messages arrivés et les range dans `sons`. Vrai quand la tâche est finie (le fil
    /// s'est arrêté) : les propositions qu'elle n'a pas traitées reprennent leur état d'avant.
    pub fn poll(&mut self, sons: &mut SonsProposes, rapport: &mut Rapport) -> bool {
        loop {
            match self.rx.try_recv() {
                Ok(Msg::Lecture { plan, resultat }) => match resultat {
                    Ok(brutes) => {
                        let f = sons.fusionner(plan, brutes);
                        self.totaux.ajoutees += f.ajoutees;
                        self.totaux.retirees += f.retirees;
                        self.totaux.connues += f.connues;
                        rapport.change |= f.ajoutees + f.retirees > 0;
                    }
                    Err(e) => rapport.erreurs.push(format!("{} : {e}", self.titre)),
                },
                Ok(Msg::Son { cue, resultat }) => {
                    self.traitees.insert(cue);
                    let avant = self.avant.get(&cue).copied().unwrap_or(Statut::Proposee);
                    let Some(i) = sons.get_mut(cue) else { continue };
                    match resultat {
                        Ok(chemin) => {
                            i.file = Some(chemin);
                            // (Rejetée ou acceptée pendant la génération : elle le reste.)
                            if i.status == Statut::EnGeneration {
                                i.status = Statut::Prete;
                            }
                            self.prets += 1;
                            rapport.change = true;
                        }
                        Err(ErreurGen::Annule) => {
                            if i.status == Statut::EnGeneration {
                                i.status = avant;
                            }
                        }
                        Err(e) => {
                            rapport.erreurs.push(format!("« {} » : {e}", i.titre()));
                            if i.status == Statut::EnGeneration {
                                i.status = avant;
                            }
                        }
                    }
                }
                Err(TryRecvError::Empty) => return false,
                Err(TryRecvError::Disconnected) => {
                    // Le fil est fini : ce qu'il n'a pas rendu (annulation, arrêt sur erreur, fil tombé)
                    // retrouve son état d'avant.
                    let mut perdues = 0;
                    for (&cue, &avant) in &self.avant {
                        if !self.traitees.contains(&cue) {
                            if let Some(i) = sons.get_mut(cue).filter(|i| i.status == Statut::EnGeneration) {
                                i.status = avant;
                                perdues += 1;
                            }
                        }
                    }
                    if perdues > 0 && !self.controle.est_annule() && rapport.erreurs.is_empty() {
                        rapport.erreurs.push(format!("{} : la génération s'est arrêtée avant la fin", self.titre));
                    }
                    return true;
                }
            }
        }
    }
}

// ------------------------------------------------------------------------------------- test

/// Le bouton « Tester » du serveur local : une phrase pour la fenêtre des Préférences.
pub fn tester_local(port: u16, controle: &Controle) -> String {
    match ClientSidecar::sur_le_port(port).etat(controle) {
        Ok(etat) => etat.resume(),
        Err(ErreurGen::NonInstalle(m)) => format!("Le serveur local ne tourne pas : {m}. Démarre-le ici, ou voir docs/GENERATION_SON.md."),
        Err(e) => format!("Le serveur local ne répond pas comme prévu : {e}."),
    }
}
