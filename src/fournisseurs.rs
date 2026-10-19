//! Les fournisseurs de la génération sonore (Anthony, 2026-10-19) : trois rôles, chacun avec son
//! fournisseur — local (un serveur sur cet ordinateur, `sidecar.rs`) ou service en ligne
//! (`fournisseursapi.rs`) :
//!
//! 1. `LecteurStoryboard` : le storyboard (vignette, textes, durée, transitions) → une liste
//!    d'indications sonores ; hors modèle, `lecturesb::LecteurRegles` le fait par des règles ;
//! 2. `GenerateurSon` : une description (anglais) → un bruitage, une ambiance ou une musique ;
//! 3. `GenerateurVoix` : une réplique et une voix → une voix témoin.
//!
//! Tout appel se fait **hors du fil de l'interface** (`generation.rs`), se laisse annuler (`Controle`)
//! et rend compte de son avancement. Aucun fournisseur ne connaît l'interface, et l'interface ne
//! connaît que ces traits : les essais mettent des faux à leur place (`fournisseurs_tests.rs`), sans
//! réseau ni modèle.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::indications::{Genre, IndicationBrute};

// ------------------------------------------------------------------------------------ erreurs

/// Pourquoi un appel n'a rien donné. Les messages sont ceux que le réalisateur lit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErreurGen {
    /// Annulé par l'utilisateur (ce n'est pas une panne).
    Annule,
    /// Le serveur local ou le modèle n'est pas installé.
    NonInstalle(String),
    /// Le fournisseur ne répond pas (ou est occupé).
    Indisponible(String),
    /// La connexion au service en ligne n'est pas branchée dans cette version.
    ABrancher(String),
    /// Pas de clé pour ce service (Préférences > Génération sonore).
    CleManquante(String),
    /// Le service refuse la demande (clé fausse, quota, contenu refusé…).
    Refuse(String),
    /// La demande ou la réponse n'est pas exploitable.
    Invalide(String),
}

impl std::fmt::Display for ErreurGen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErreurGen::Annule => write!(f, "annulé"),
            ErreurGen::NonInstalle(m) => write!(f, "non installé : {m}"),
            ErreurGen::Indisponible(m) => write!(f, "indisponible : {m}"),
            ErreurGen::ABrancher(m) => write!(f, "connexion en ligne à brancher : {m}"),
            ErreurGen::CleManquante(m) => write!(f, "clé manquante : {m}"),
            ErreurGen::Refuse(m) => write!(f, "refusé par le service : {m}"),
            ErreurGen::Invalide(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for ErreurGen {}

// --------------------------------------------------------------------------- contrôle d'un appel

/// L'avancement d'une tâche, lu par l'interface pendant que le fil de travail l'écrit.
#[derive(Default)]
pub struct Progres {
    fait: AtomicUsize,
    total: AtomicUsize,
    texte: Mutex<String>,
}

/// Ce qu'un appel reçoit pour rester annulable et rendre compte : un drapeau d'annulation que
/// l'interface lève, un avancement qu'elle lit. Se clone (les clones partagent le même état).
#[derive(Clone, Default)]
pub struct Controle {
    annule: Arc<AtomicBool>,
    progres: Arc<Progres>,
}

impl Controle {
    pub fn new() -> Controle {
        Controle::default()
    }

    /// Demande l'arrêt : le fil de travail s'arrête à la prochaine occasion (une requête en vol est
    /// coupée en fermant la connexion).
    pub fn annuler(&self) {
        self.annule.store(true, Ordering::Relaxed);
    }

    pub fn est_annule(&self) -> bool {
        self.annule.load(Ordering::Relaxed)
    }

    /// `Err(Annule)` si l'arrêt est demandé : à appeler entre deux étapes.
    pub fn verifier(&self) -> Result<(), ErreurGen> {
        if self.est_annule() {
            Err(ErreurGen::Annule)
        } else {
            Ok(())
        }
    }

    /// `fait` étapes sur `total`, et ce qu'on est en train de faire.
    pub fn avancer(&self, fait: usize, total: usize, texte: &str) {
        self.progres.fait.store(fait, Ordering::Relaxed);
        self.progres.total.store(total, Ordering::Relaxed);
        if let Ok(mut t) = self.progres.texte.lock() {
            texte.clone_into(&mut t);
        }
    }

    /// (étapes faites, total, texte).
    pub fn lire(&self) -> (usize, usize, String) {
        let texte = self.progres.texte.lock().map(|t| t.clone()).unwrap_or_default();
        (self.progres.fait.load(Ordering::Relaxed), self.progres.total.load(Ordering::Relaxed), texte)
    }
}

// ------------------------------------------------------------------------------- les demandes

/// Les textes d'une image de storyboard posée dans le plan (une marque d'image).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MarqueTexte {
    /// L'image du plan où elle commence.
    pub frame: usize,
    pub action: String,
    pub dialogue: String,
    pub notes: String,
}

/// Tout ce que la lecture du storyboard sait d'un plan. Sans rien de propre à un fournisseur : le
/// lecteur local, Claude et les règles reçoivent la même chose.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EntreePlan {
    /// Le numéro du plan dans le projet (l'identifiant que les propositions gardent).
    pub plan: u64,
    /// « Plan 3 ».
    pub titre: String,
    pub sequence: String,
    /// Première image du plan dans le film, et sa durée en images.
    pub debut: usize,
    pub frames: usize,
    /// Les trois champs de la fiche du plan.
    pub action: String,
    pub dialogue: String,
    pub notes: String,
    /// Les images de storyboard posées dans le plan, avec leurs propres textes.
    pub marques: Vec<MarqueTexte>,
    /// La transition qui amène ce plan, et celle qui le quitte (« Fondu enchaîné, 12 images »).
    pub transition_entrante: Option<String>,
    pub transition_sortante: Option<String>,
    /// Le texte des commentaires de la timeline sur ce plan, pas encore résolus.
    pub commentaires: Vec<String>,
    /// Une petite vignette du plan (PNG), si la lecture doit voir l'image.
    pub vignette_png: Option<Vec<u8>>,
}

/// La lecture demandée : les plans, dans l'ordre du film.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DemandeLecture {
    pub plans: Vec<EntreePlan>,
    pub fps: u32,
}

/// Un son à générer : bruitage, ambiance ou musique d'intention.
#[derive(Clone, Debug, PartialEq)]
pub struct DemandeSon {
    pub genre: Genre,
    /// La description (anglais).
    pub prompt: String,
    pub secondes: f32,
}

/// Une voix à générer.
#[derive(Clone, Debug, PartialEq)]
pub struct DemandeVoix {
    pub texte: String,
    pub personnage: String,
    /// La voix choisie chez le fournisseur ; vide : celle des préférences.
    pub voix: String,
    /// La langue du texte (« fr »).
    pub langue: String,
}

/// Un son rendu par un générateur : les octets d'un fichier (WAV, MP3…) et son extension.
#[derive(Clone, Debug, PartialEq)]
pub struct SonGenere {
    pub octets: Vec<u8>,
    pub ext: String,
}

// ---------------------------------------------------------------------------------- les traits

/// Rôle 1 : le storyboard → des indications sonores.
pub trait LecteurStoryboard: Send + Sync {
    /// Le nom du fournisseur, pour les messages (« règles », « Qwen2.5-VL (local) », « Claude »).
    fn nom(&self) -> String;
    /// Ce que le fournisseur envoie hors de cet ordinateur, s'il en envoie (une phrase pour la fenêtre
    /// de confirmation) ; rien pour un lecteur local.
    fn envoie_en_ligne(&self) -> Option<String> {
        None
    }
    /// Lit le plan `index` de `demande` (les autres plans donnent le contexte).
    fn lire_plan(&self, demande: &DemandeLecture, index: usize, controle: &Controle) -> Result<Vec<IndicationBrute>, ErreurGen>;
}

/// Rôle 2 : une description → un bruitage, une ambiance ou une musique.
pub trait GenerateurSon: Send + Sync {
    fn nom(&self) -> String;
    fn envoie_en_ligne(&self) -> Option<String> {
        None
    }
    fn generer(&self, demande: &DemandeSon, controle: &Controle) -> Result<SonGenere, ErreurGen>;
}

/// Rôle 3 : une réplique → une voix témoin.
pub trait GenerateurVoix: Send + Sync {
    fn nom(&self) -> String;
    fn envoie_en_ligne(&self) -> Option<String> {
        None
    }
    fn generer(&self, demande: &DemandeVoix, controle: &Controle) -> Result<SonGenere, ErreurGen>;
}
