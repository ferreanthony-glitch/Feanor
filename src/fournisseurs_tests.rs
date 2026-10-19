//! Tests des fournisseurs de la génération sonore, **sans réseau ni modèle** : la lecture par règles,
//! la relecture des réponses d'un modèle, le client HTTP local (contre un petit serveur de l'essai sur
//! 127.0.0.1), le contrat du serveur local, les requêtes des services en ligne (construites et relues,
//! jamais envoyées), les clés, les réglages et les tâches avec des fournisseurs simulés.
//!
//! Les fournisseurs simulés (`LecteurSimule`, `SonSimule`, `VoixSimulee`, `FabriqueSimulee`) servent
//! aussi aux essais de l'interface (`suggestions_tests.rs`).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::base64;
use crate::clessecretes::{Cle, Coffre, CoffreMemoire, Service};
use crate::fournisseurs::{Controle, DemandeLecture, DemandeSon, DemandeVoix, EntreePlan, ErreurGen, GenerateurSon, GenerateurVoix, LecteurStoryboard, MarqueTexte, SonGenere};
use crate::fournisseursapi::{self, LecteurClaude, RequeteHttps, SonElevenLabs, SonStableAudio, Transport, TransportABrancher, VoixElevenLabs};
use crate::generation::{lancer_generation, lancer_lecture, Fabrique, Rapport, Tache, Travail};
use crate::genreglages::{GenerationSonReglages, Mode, Role};
use crate::httplocal::{requete, ReponseHttp};
use crate::indications::{Genre, IndicationBrute, SonsProposes, Statut};
use crate::lecturerules::{lire, separer_replique, LecteurRegles};
use crate::lecturesb::{extraire_json, indications_depuis_texte, CONSIGNES};
use crate::sidecar::{ClientSidecar, LecteurSidecar, SonSidecar, VoixSidecar};

// ============================================================================ fournisseurs simulés

/// Un WAV 16 bits mono à 48 kHz de `secondes` secondes (un souffle faible).
pub(crate) fn wav_mono(secondes: f32) -> Vec<u8> {
    let n = (secondes * 48_000.0) as usize;
    let data = (n * 2) as u32;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&48_000u32.to_le_bytes());
    b.extend_from_slice(&96_000u32.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    for k in 0..n {
        b.extend_from_slice(&(((k % 50) as i16 - 25) * 20).to_le_bytes());
    }
    b
}

/// Attend, par petits pas, que le travail soit annulé ou que `delai` passe.
fn patienter(controle: &Controle, delai: Duration) -> Result<(), ErreurGen> {
    let fin = Instant::now() + delai;
    while Instant::now() < fin {
        controle.verifier()?;
        std::thread::sleep(Duration::from_millis(2));
    }
    controle.verifier()
}

type Reponse = Box<dyn Fn(&EntreePlan) -> Vec<IndicationBrute> + Send + Sync>;

/// Un lecteur qui répond ce qu'on lui dit de répondre, et note ce qu'il a reçu.
pub(crate) struct LecteurSimule {
    /// Pour chaque appel : le rang du plan, et s'il avait une vignette.
    pub appels: Mutex<Vec<(usize, bool)>>,
    /// Les vignettes reçues, dans l'ordre.
    pub vignettes: Mutex<Vec<Option<Vec<u8>>>>,
    pub reponse: Reponse,
    pub delai: Duration,
    pub echec: Mutex<Option<ErreurGen>>,
    pub en_ligne: bool,
}

impl LecteurSimule {
    pub(crate) fn new(reponse: impl Fn(&EntreePlan) -> Vec<IndicationBrute> + Send + Sync + 'static) -> LecteurSimule {
        LecteurSimule { appels: Mutex::new(Vec::new()), vignettes: Mutex::new(Vec::new()), reponse: Box::new(reponse), delai: Duration::ZERO, echec: Mutex::new(None), en_ligne: false }
    }
}

impl LecteurStoryboard for LecteurSimule {
    fn nom(&self) -> String {
        "lecteur simulé".into()
    }

    fn envoie_en_ligne(&self) -> Option<String> {
        self.en_ligne.then(|| "les vignettes et les textes partent vers le lecteur simulé".to_string())
    }

    fn lire_plan(&self, demande: &DemandeLecture, index: usize, controle: &Controle) -> Result<Vec<IndicationBrute>, ErreurGen> {
        self.appels.lock().unwrap().push((index, demande.plans[index].vignette_png.is_some()));
        self.vignettes.lock().unwrap().push(demande.plans[index].vignette_png.clone());
        patienter(controle, self.delai)?;
        if let Some(e) = self.echec.lock().unwrap().clone() {
            return Err(e);
        }
        Ok((self.reponse)(&demande.plans[index]))
    }
}

/// Un générateur de bruitages simulé : rend un WAV de la durée demandée (ou de `secondes` si donnée).
pub(crate) struct SonSimule {
    pub appels: Mutex<Vec<DemandeSon>>,
    pub secondes: Option<f32>,
    /// Ne rend rien tant qu'on ne l'annule pas.
    pub bloque: AtomicBool,
    /// Les prompts qui échouent d'une erreur sans gravité (réponse inexploitable), puis ceux qui échouent pour de bon.
    pub invalides: Mutex<Vec<String>>,
    pub fatale: Mutex<Option<ErreurGen>>,
    pub en_ligne: bool,
}

impl SonSimule {
    pub(crate) fn new() -> SonSimule {
        SonSimule { appels: Mutex::new(Vec::new()), secondes: None, bloque: AtomicBool::new(false), invalides: Mutex::new(Vec::new()), fatale: Mutex::new(None), en_ligne: false }
    }
}

impl GenerateurSon for SonSimule {
    fn nom(&self) -> String {
        "sons simulés".into()
    }

    fn envoie_en_ligne(&self) -> Option<String> {
        self.en_ligne.then(|| "les descriptions partent vers le générateur simulé".to_string())
    }

    fn generer(&self, demande: &DemandeSon, controle: &Controle) -> Result<SonGenere, ErreurGen> {
        self.appels.lock().unwrap().push(demande.clone());
        // (Bloqué jusqu'à ce qu'on le débloque, ou qu'on annule.)
        while self.bloque.load(Ordering::Relaxed) {
            controle.verifier()?;
            std::thread::sleep(Duration::from_millis(2));
        }
        controle.verifier()?;
        if let Some(e) = self.fatale.lock().unwrap().clone() {
            return Err(e);
        }
        if self.invalides.lock().unwrap().contains(&demande.prompt) {
            return Err(ErreurGen::Invalide("réponse inexploitable".into()));
        }
        Ok(SonGenere { octets: wav_mono(self.secondes.unwrap_or(demande.secondes)), ext: "wav".into() })
    }
}

/// Un générateur de voix simulé.
pub(crate) struct VoixSimulee {
    pub appels: Mutex<Vec<DemandeVoix>>,
    pub secondes: f32,
    pub en_ligne: bool,
}

impl VoixSimulee {
    pub(crate) fn new() -> VoixSimulee {
        VoixSimulee { appels: Mutex::new(Vec::new()), secondes: 1.0, en_ligne: false }
    }
}

impl GenerateurVoix for VoixSimulee {
    fn nom(&self) -> String {
        "voix simulées".into()
    }

    fn envoie_en_ligne(&self) -> Option<String> {
        self.en_ligne.then(|| "les répliques partent vers la voix simulée".to_string())
    }

    fn generer(&self, demande: &DemandeVoix, controle: &Controle) -> Result<SonGenere, ErreurGen> {
        self.appels.lock().unwrap().push(demande.clone());
        controle.verifier()?;
        Ok(SonGenere { octets: wav_mono(self.secondes), ext: "wav".into() })
    }
}

/// Les trois fournisseurs simulés, que les essais gardent pour regarder ce qu'ils ont reçu.
pub(crate) struct FabriqueSimulee {
    pub lecteur: Arc<LecteurSimule>,
    pub son: Arc<SonSimule>,
    pub voix: Arc<VoixSimulee>,
}

impl FabriqueSimulee {
    pub(crate) fn new(lecteur: LecteurSimule) -> FabriqueSimulee {
        FabriqueSimulee { lecteur: Arc::new(lecteur), son: Arc::new(SonSimule::new()), voix: Arc::new(VoixSimulee::new()) }
    }
}

impl Fabrique for FabriqueSimulee {
    fn lecteur(&self, _: &GenerationSonReglages) -> Arc<dyn LecteurStoryboard> {
        self.lecteur.clone()
    }

    fn son(&self, _: &GenerationSonReglages) -> Arc<dyn GenerateurSon> {
        self.son.clone()
    }

    fn voix(&self, _: &GenerationSonReglages) -> Arc<dyn GenerateurVoix> {
        self.voix.clone()
    }
}

// =================================================================================== lecture par règles

fn plan(action: &str, dialogue: &str, notes: &str, frames: usize) -> EntreePlan {
    EntreePlan { plan: 9, titre: "Plan 1".into(), sequence: "1".into(), frames, action: action.into(), dialogue: dialogue.into(), notes: notes.into(), ..Default::default() }
}

fn descriptions(v: &[IndicationBrute]) -> Vec<&str> {
    v.iter().map(|i| i.description.as_str()).collect()
}

#[test]
fn the_rules_find_sound_effects_ambiences_and_music_in_the_action_text() {
    let v = lire(&plan("La porte claque. Il pleut dehors. Des pas lourds dans le couloir. Elle joue du piano.", "", "", 96), 24);
    let d = descriptions(&v);
    for attendu in ["Porte qui claque", "Pluie", "Bruits de pas", "Musique d'intention : piano"] {
        assert!(d.contains(&attendu), "{attendu} dans {d:?}");
    }
    let genre = |nom: &str| v.iter().find(|i| i.description == nom).unwrap().kind;
    assert_eq!(genre("Porte qui claque"), Some(Genre::Bruitage));
    assert_eq!(genre("Pluie"), Some(Genre::Ambiance));
    assert_eq!(genre("Musique d'intention : piano"), Some(Genre::Musique));
    // Une proposition de bruitage a son prompt anglais ; tout commence à l'image 0 du plan.
    let porte = v.iter().find(|i| i.description == "Porte qui claque").unwrap();
    assert!(porte.prompt.contains("door"), "{porte:?}");
    assert!(v.iter().all(|i| i.start == 0));
}

#[test]
fn the_rules_durations_follow_the_kind_of_sound() {
    let v = lire(&plan("La porte claque. Il pleut.", "", "", 96), 24);
    let porte = v.iter().find(|i| i.description == "Porte qui claque").unwrap();
    assert_eq!(porte.frames, 36, "1,5 s à 24 i/s");
    let pluie = v.iter().find(|i| i.description == "Pluie").unwrap();
    assert_eq!(pluie.frames, 96, "une ambiance couvre le plan");
    // Un plan très long : au plus trente secondes d'ambiance.
    let long = lire(&plan("Il pleut.", "", "", 24 * 120), 24);
    assert_eq!(long[0].frames, 24 * 30);
}

#[test]
fn the_rules_ignore_case_accents_and_punctuation_and_whole_words_only() {
    let v = lire(&plan("UNE EXPLOSION ! Détonation… Éclair. Le CAFÉ du coin.", "", "", 48), 24);
    let d = descriptions(&v);
    assert!(d.contains(&"Explosion") && d.contains(&"Coup de tonnerre") && d.contains(&"Café, restaurant"), "{d:?}");
    // « vent » n'est pas « avent » ni « ventre » ; « mer » n'est pas « mère ».
    assert!(lire(&plan("Sa mère mange au ventre de l'avent.", "", "", 48), 24).is_empty());
    // Un début de mot cherché avec « * » : « explos* » trouve « explosions ».
    assert!(!lire(&plan("Des explosions.", "", "", 48), 24).is_empty());
}

#[test]
fn the_rules_skip_a_word_that_is_denied() {
    assert!(lire(&plan("Sans musique. Pas de pluie. Aucun vent.", "", "", 48), 24).is_empty());
    let v = lire(&plan("Pas de pluie, mais du vent.", "", "", 48), 24);
    assert_eq!(descriptions(&v), vec!["Vent"]);
}

#[test]
fn what_a_character_says_never_triggers_a_sound_effect() {
    let v = lire(&plan("Marie attend.", "MARIE : Il pleut des cordes, et la porte claque !", "", 48), 24);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].kind, Some(Genre::Voix));
}

#[test]
fn dialogue_lines_become_voices_one_after_the_other() {
    let dialogue = "MARIE : Viens ici.\n- Non.\nPAUL (off) : « Tant pis. »\n\n…";
    let v = lire(&plan("", dialogue, "", 240), 24);
    assert_eq!(v.len(), 3, "{v:#?}");
    assert!(v.iter().all(|i| i.kind == Some(Genre::Voix)));
    assert_eq!((v[0].character.as_str(), v[0].text.as_str()), ("MARIE", "Viens ici."));
    assert_eq!((v[1].character.as_str(), v[1].text.as_str()), ("", "Non."));
    assert_eq!((v[2].character.as_str(), v[2].text.as_str()), ("PAUL", "Tant pis."), "didascalie et guillemets retirés");
    assert_eq!(v[0].start, 0);
    assert!(v[1].start >= v[0].start + v[0].frames && v[2].start >= v[1].start + v[1].frames, "les répliques se suivent : {:?}", v.iter().map(|i| (i.start, i.frames)).collect::<Vec<_>>());
    assert!(v[0].description.starts_with("MARIE") && v[1].description.starts_with("Voix"));
    assert!(v.iter().all(|i| i.prompt.is_empty()), "pas de prompt pour une voix");
    // Une voix dure au moins une seconde.
    assert!(v.iter().all(|i| i.frames >= 24));
}

#[test]
fn lines_are_split_into_a_character_and_a_line_only_when_it_looks_like_a_name() {
    assert_eq!(separer_replique("MARIE : Bonjour"), ("MARIE".into(), "Bonjour".into()));
    assert_eq!(separer_replique("Marie Dupont: Salut"), ("Marie Dupont".into(), "Salut".into()));
    assert_eq!(separer_replique("  — « Allons-y. »"), (String::new(), "Allons-y.".into()));
    assert_eq!(separer_replique("Il dit : bonjour"), (String::new(), "Il dit : bonjour".into()), "un verbe n'est pas un nom");
    assert_eq!(separer_replique("12:30 rendez-vous"), (String::new(), "12:30 rendez-vous".into()));
    assert_eq!(separer_replique("Ah ! non : rien"), (String::new(), "Ah ! non : rien".into()));
    assert_eq!(separer_replique("PAUL :").0, "", "un nom sans réplique n'est pas un nom");
}

#[test]
fn written_sound_directions_are_taken_whether_the_rules_know_them_or_not() {
    let notes = "SFX : porte qui grince, tic-tac de pendule\nAmbiance : ronronnement du frigo\nMusique : valse lente";
    let v = lire(&plan("", "", notes, 100), 24);
    let d = descriptions(&v);
    assert!(d.contains(&"Porte qui s'ouvre en grinçant"), "{d:?}: connue des règles, donc décrite par elles");
    let tic = v.iter().find(|i| i.description == "Tic-tac de pendule").expect("reprise telle quelle");
    assert_eq!((tic.kind, tic.prompt.as_str()), (Some(Genre::Bruitage), "tic-tac de pendule"));
    let frigo = v.iter().find(|i| i.description == "Ronronnement du frigo").unwrap();
    assert_eq!((frigo.kind, frigo.frames), (Some(Genre::Ambiance), 100));
    assert_eq!(v.iter().find(|i| i.description == "Valse lente").unwrap().kind, Some(Genre::Musique));
    assert_eq!(v.len(), 4, "pas de doublon : {d:?}");
}

#[test]
fn the_storyboard_images_of_a_plan_place_their_sounds_at_their_own_frame() {
    let mut p = plan("Un homme marche. Il pleut.", "", "", 48);
    p.marques = vec![
        MarqueTexte { frame: 12, action: "Une explosion ! Il pleut toujours.".into(), dialogue: "PAUL : Courons !".into(), notes: String::new() },
        MarqueTexte { frame: 400, action: "Un coup de feu.".into(), ..Default::default() },
    ];
    let v = lire(&p, 24);
    let ou = |nom: &str| v.iter().filter(|i| i.description == nom).map(|i| i.start).collect::<Vec<_>>();
    assert_eq!(ou("Explosion"), vec![12]);
    assert_eq!(ou("Pluie"), vec![0], "une seule ambiance de pluie par plan");
    assert_eq!(ou("Coup de feu"), vec![47], "une image au-delà du plan est ramenée sur sa dernière image");
    assert_eq!(v.iter().find(|i| i.kind == Some(Genre::Voix)).unwrap().start, 12);
    assert!(v.windows(2).all(|w| w[0].start <= w[1].start), "dans l'ordre du temps");
}

#[test]
fn the_rules_make_one_music_per_plan_and_at_most_forty_cues() {
    let v = lire(&plan("Du piano, puis un violon, une chanson, une fanfare.", "", "", 48), 24);
    assert_eq!(v.iter().filter(|i| i.kind == Some(Genre::Musique)).count(), 1);
    let dialogue = (0..100).map(|k| format!("PAUL : Réplique numéro {k}.")).collect::<Vec<_>>().join("\n");
    assert_eq!(lire(&plan("", &dialogue, "", 48), 24).len(), 40);
    assert!(lire(&plan("", "", "", 48), 24).is_empty());
    assert!(lire(&plan("   ", "  ", "\n", 48), 24).is_empty());
}

#[test]
fn the_rule_reader_is_a_reader_like_the_others_and_stops_when_cancelled() {
    let demande = DemandeLecture { plans: vec![plan("La porte claque.", "", "", 48)], fps: 24 };
    let c = Controle::new();
    let r = LecteurRegles.lire_plan(&demande, 0, &c).unwrap();
    assert_eq!(r.len(), 1);
    assert!(LecteurRegles.envoie_en_ligne().is_none());
    c.annuler();
    assert_eq!(LecteurRegles.lire_plan(&demande, 0, &c), Err(ErreurGen::Annule));
    assert!(matches!(LecteurRegles.lire_plan(&demande, 3, &Controle::new()), Err(ErreurGen::Invalide(_))));
}

// ============================================================================= réponse d'un modèle

#[test]
fn a_model_reply_is_read_with_suspicion() {
    let reponse = r#"Voici mes idées :
```json
{"indications": [
  {"image": 2, "images": 30, "genre": "bruitage", "description": "Porte", "prompt": "door slam"},
  {"image": 500, "images": 10, "genre": "SFX", "description": "Klaxon", "prompt": "car horn"},
  {"image": 0, "images": 48, "genre": "ambiance", "description": "Pluie", "prompt": "rain"},
  {"image": 0, "images": 5, "genre": "inconnu", "description": "Écartée"},
  {"image": 0, "images": 5, "genre": "voix", "description": "Sans texte"},
  {"image": 4, "images": 20, "genre": "voix", "texte": "Bonjour.", "personnage": "MARIE"},
  {"genre": "bruitage"},
  {"image": 0.0, "genre": "musique", "description": "Valse"}
]}
```
J'espère que cela aide."#;
    let v = indications_depuis_texte(reponse, 48).unwrap();
    let d = descriptions(&v);
    assert_eq!(d, vec!["Porte", "Klaxon", "Pluie", "", "Valse"], "sorte inconnue, voix sans texte et indication vide écartées");
    assert_eq!(v[1].start, 47, "une image hors du plan est ramenée sur sa dernière image");
    assert_eq!(v[3].text, "Bonjour.");
    assert_eq!(v[3].character, "MARIE");
    assert_eq!(v[4].frames, 48, "sans durée : jusqu'à la fin du plan");
    // Une liste seule est acceptée ; du texte sans JSON, non.
    assert_eq!(indications_depuis_texte("[{\"image\": 1, \"images\": 4, \"genre\": \"bruitage\", \"description\": \"x\"}]", 10).unwrap().len(), 1);
    assert!(matches!(indications_depuis_texte("Je ne sais pas.", 10), Err(ErreurGen::Invalide(_))));
    assert!(matches!(indications_depuis_texte("{ pas du json }", 10), Err(ErreurGen::Invalide(_))));
    assert!(matches!(indications_depuis_texte("{\"autre\": 1}", 10), Err(ErreurGen::Invalide(_))));
    assert_eq!(extraire_json("avant {\"a\": [1]} après"), Some("{\"a\": [1]}"));
}

// =============================================================================== client HTTP local

struct Serveur {
    port: u16,
    requetes: Arc<Mutex<Vec<Vec<u8>>>>,
    ferme: Arc<AtomicBool>,
}

fn http(statut: &str, entetes: &[(&str, &str)], corps: &[u8]) -> Vec<u8> {
    let mut r = format!("HTTP/1.1 {statut}\r\n");
    for (n, v) in entetes {
        r.push_str(&format!("{n}: {v}\r\n"));
    }
    r.push_str("\r\n");
    let mut b = r.into_bytes();
    b.extend_from_slice(corps);
    b
}

fn http_json(statut: &str, v: &Value) -> Vec<u8> {
    let corps = serde_json::to_vec(v).unwrap();
    http(statut, &[("Content-Type", "application/json"), ("Content-Length", &corps.len().to_string())], &corps)
}

fn lire_requete(flux: &mut TcpStream) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut tranche = [0u8; 4096];
    flux.set_read_timeout(Some(Duration::from_secs(5))).ok();
    loop {
        let n = flux.read(&mut tranche).unwrap_or(0);
        if n == 0 {
            return buf;
        }
        buf.extend_from_slice(&tranche[..n]);
        if let Some(c) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let tete = String::from_utf8_lossy(&buf[..c]).to_ascii_lowercase();
            let longueur = tete.lines().find_map(|l| l.strip_prefix("content-length:")).and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(0);
            if buf.len() >= c + 4 + longueur {
                return buf;
            }
        }
    }
}

/// Un serveur qui accepte une connexion par réponse, lit la requête, répond, et ferme.
fn serveur(reponses: Vec<Vec<u8>>) -> Serveur {
    let ecoute = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = ecoute.local_addr().unwrap().port();
    let requetes = Arc::new(Mutex::new(Vec::new()));
    let r = requetes.clone();
    std::thread::spawn(move || {
        for reponse in reponses {
            let Ok((mut flux, _)) = ecoute.accept() else { return };
            let req = lire_requete(&mut flux);
            r.lock().unwrap().push(req);
            let _ = flux.write_all(&reponse);
            let _ = flux.flush();
        }
    });
    Serveur { port, requetes, ferme: Arc::new(AtomicBool::new(false)) }
}

/// Un serveur qui lit la requête puis ne répond jamais ; il note le moment où le client coupe.
fn serveur_muet() -> Serveur {
    let ecoute = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = ecoute.local_addr().unwrap().port();
    let requetes = Arc::new(Mutex::new(Vec::new()));
    let ferme = Arc::new(AtomicBool::new(false));
    let (r, f) = (requetes.clone(), ferme.clone());
    std::thread::spawn(move || {
        let Ok((mut flux, _)) = ecoute.accept() else { return };
        let req = lire_requete(&mut flux);
        r.lock().unwrap().push(req);
        let mut tmp = [0u8; 64];
        flux.set_read_timeout(Some(Duration::from_secs(10))).ok();
        if matches!(flux.read(&mut tmp), Ok(0)) {
            f.store(true, Ordering::Relaxed);
        }
    });
    Serveur { port, requetes, ferme }
}

fn adresse(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

fn requete_texte(s: &Serveur, k: usize) -> String {
    String::from_utf8_lossy(&s.requetes.lock().unwrap()[k]).into_owned()
}

fn corps_json(s: &Serveur, k: usize) -> Value {
    let t = requete_texte(s, k);
    serde_json::from_str(t.split("\r\n\r\n").nth(1).unwrap()).unwrap()
}

const DELAI: Duration = Duration::from_secs(5);

#[test]
fn the_local_client_sends_a_well_formed_request_and_reads_a_length_delimited_reply() {
    let s = serveur(vec![http_json("200 OK", &json!({"ok": true}))]);
    let corps = br#"{"a":1}"#;
    let r = requete(adresse(s.port), "POST", "/v1/test", Some(("application/json", corps)), &Controle::new(), DELAI).unwrap();
    assert_eq!((r.statut, r.reussie()), (200, true));
    assert_eq!(r.entete("content-type"), Some("application/json"));
    assert_eq!(serde_json::from_slice::<Value>(&r.corps).unwrap(), json!({"ok": true}));
    let t = requete_texte(&s, 0);
    assert!(t.starts_with("POST /v1/test HTTP/1.1\r\n"), "{t}");
    for ligne in ["Host: 127.0.0.1:", "Connection: close", "Content-Type: application/json", "Content-Length: 7"] {
        assert!(t.contains(ligne), "{ligne} dans {t}");
    }
    assert!(t.ends_with("{\"a\":1}"));
}

#[test]
fn the_local_client_reads_chunked_and_close_delimited_replies() {
    let morceaux = http("200 OK", &[("Transfer-Encoding", "chunked"), ("Content-Type", "audio/wav")], b"5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n");
    let jusqu_a_la_fin = http("200 OK", &[("Content-Type", "text/plain")], b"jusqu'a la fermeture");
    let sans_corps = http("204 No Content", &[], b"");
    let s = serveur(vec![morceaux, jusqu_a_la_fin, sans_corps]);
    let c = Controle::new();
    assert_eq!(requete(adresse(s.port), "GET", "/a", None, &c, DELAI).unwrap().corps, b"hello world");
    assert_eq!(requete(adresse(s.port), "GET", "/b", None, &c, DELAI).unwrap().corps, b"jusqu'a la fermeture");
    let r = requete(adresse(s.port), "GET", "/c", None, &c, DELAI).unwrap();
    assert_eq!((r.statut, r.corps.len()), (204, 0));
    assert!(!requete_texte(&s, 0).contains("Content-Length"), "pas de longueur sur un GET");
}

#[test]
fn the_local_client_reports_a_missing_server_and_refuses_other_machines() {
    // Rien n'écoute sur ce port : « non installé », sans panne.
    let libre = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let e = requete(adresse(libre), "GET", "/", None, &Controle::new(), DELAI).unwrap_err();
    assert!(matches!(&e, ErreurGen::NonInstalle(m) if m.contains("127.0.0.1")), "{e:?}");
    // Une adresse qui n'est pas de cet ordinateur n'est même pas essayée.
    for hote in ["192.168.1.20:8765", "8.8.8.8:80", "10.0.0.1:80"] {
        let a: SocketAddr = hote.parse().unwrap();
        let e = requete(a, "GET", "/", None, &Controle::new(), DELAI).unwrap_err();
        assert!(matches!(&e, ErreurGen::Invalide(m) if m.contains("adresse locale")), "{hote} : {e:?}");
    }
    // L'adresse locale en IPv6 est permise (essai de la règle seulement, pas de connexion).
    let a6: SocketAddr = "[::1]:1".parse().unwrap();
    assert!(a6.ip().is_loopback());
}

#[test]
fn the_local_client_has_a_deadline_a_size_limit_and_rejects_non_http() {
    let muet = serveur_muet();
    let t = Instant::now();
    let e = requete(adresse(muet.port), "GET", "/", None, &Controle::new(), Duration::from_millis(300)).unwrap_err();
    assert!(matches!(e, ErreurGen::Indisponible(_)), "{e:?}");
    assert!(t.elapsed() < Duration::from_secs(3));
    // Une réponse qui annonce plus que la limite est refusée sans l'attendre.
    let trop = http("200 OK", &[("Content-Length", &(crate::httplocal::MAX_REPONSE + 1).to_string())], b"x");
    let s = serveur(vec![trop, b"ce n'est pas du HTTP\r\n\r\n".to_vec(), http("200 OK", &[("Content-Length", "50")], b"court")]);
    let c = Controle::new();
    assert!(matches!(requete(adresse(s.port), "GET", "/", None, &c, DELAI), Err(ErreurGen::Invalide(m)) if m.contains("trop grosse")));
    assert!(matches!(requete(adresse(s.port), "GET", "/", None, &c, DELAI), Err(ErreurGen::Invalide(_))));
    // Une réponse coupée avant sa longueur annoncée.
    assert!(matches!(requete(adresse(s.port), "GET", "/", None, &c, DELAI), Err(ErreurGen::Indisponible(m)) if m.contains("tronquée")));
}

#[test]
fn cancelling_closes_the_connection_to_a_server_that_does_not_answer() {
    let muet = serveur_muet();
    let c = Controle::new();
    let c2 = c.clone();
    let fil = std::thread::spawn(move || requete(adresse(muet.port), "POST", "/v1/son", Some(("application/json", b"{}")), &c2, Duration::from_secs(60)));
    std::thread::sleep(Duration::from_millis(250));
    let t = Instant::now();
    c.annuler();
    let r = fil.join().unwrap();
    assert_eq!(r, Err(ErreurGen::Annule));
    assert!(t.elapsed() < Duration::from_secs(2), "l'annulation est prompte : {:?}", t.elapsed());
    // Le serveur voit la connexion se fermer (c'est ainsi qu'il sait qu'on a renoncé).
    let debut = Instant::now();
    while !muet.ferme.load(Ordering::Relaxed) && debut.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(muet.ferme.load(Ordering::Relaxed), "le serveur n'a pas vu la fermeture");
    // Déjà annulé : aucune connexion n'est ouverte.
    assert_eq!(requete(adresse(1), "GET", "/", None, &c, DELAI), Err(ErreurGen::Annule));
}

// =============================================================== le contrat du serveur local

#[test]
fn the_local_state_is_read_and_summed_up() {
    let etat = json!({"version": 1, "modeles": {"lecture": {"installe": true, "nom": "Qwen2.5-VL-7B"}, "son": {"installe": false}, "voix": {"installe": true, "nom": "Kokoro-82M"}}});
    let s = serveur(vec![http_json("200 OK", &etat)]);
    let e = ClientSidecar::sur_le_port(s.port).etat(&Controle::new()).unwrap();
    assert_eq!(e.version, 1);
    assert!(e.lecture.installe && !e.son.installe && e.voix.installe);
    assert!(requete_texte(&s, 0).starts_with("GET /v1/etat HTTP/1.1"));
    let resume = e.resume();
    assert!(resume.contains("lecture prêt (Qwen2.5-VL-7B)") && resume.contains("sons non installé") && resume.contains("voix prêt"), "{resume}");
}

#[test]
fn server_errors_become_clear_french_errors() {
    let erreur = |statut: &str, code: &str| http_json(statut, &json!({"erreur": {"code": code, "message": "le détail"}}));
    let s = serveur(vec![
        erreur("501 Not Implemented", "non_installe"),
        erreur("503 Service Unavailable", "occupe"),
        erreur("422 Unprocessable Entity", "invalide"),
        http("500 Internal Server Error", &[("Content-Length", "0")], b""),
        http("404 Not Found", &[("Content-Length", "0")], b""),
    ]);
    let client = ClientSidecar::sur_le_port(s.port);
    let c = Controle::new();
    let demande = DemandeSon { genre: Genre::Bruitage, prompt: "door".into(), secondes: 2.0 };
    let essai = || SonSidecar(client.clone()).generer(&demande, &c);
    assert!(matches!(essai(), Err(ErreurGen::NonInstalle(m)) if m == "le détail"));
    assert!(matches!(essai(), Err(ErreurGen::Indisponible(m)) if m == "le détail"));
    assert!(matches!(essai(), Err(ErreurGen::Invalide(m)) if m == "le détail"));
    assert!(matches!(essai(), Err(ErreurGen::Indisponible(m)) if m.contains("500")));
    assert!(matches!(essai(), Err(ErreurGen::Invalide(m)) if m.contains("404")));
}

#[test]
fn the_local_sound_and_voice_requests_follow_the_contract() {
    let wav = wav_mono(0.5);
    let en_json = http_json("200 OK", &json!({"audio_b64": base64::encode(&wav), "format": "wav"}));
    let en_audio = http("200 OK", &[("Content-Type", "audio/wav"), ("Content-Length", &wav.len().to_string())], &wav);
    let s = serveur(vec![en_json, en_audio]);
    let client = ClientSidecar::sur_le_port(s.port);
    let c = Controle::new();
    let son = SonSidecar(client.clone()).generer(&DemandeSon { genre: Genre::Ambiance, prompt: "rain".into(), secondes: 4.5 }, &c).unwrap();
    assert_eq!((son.ext.as_str(), son.octets.clone()), ("wav", wav.clone()));
    assert!(requete_texte(&s, 0).starts_with("POST /v1/son HTTP/1.1"));
    assert_eq!(corps_json(&s, 0), json!({"version": 1, "genre": "ambiance", "prompt": "rain", "duree_s": 4.5}));
    let voix = VoixSidecar(client).generer(&DemandeVoix { texte: "Bonjour.".into(), personnage: "MARIE".into(), voix: "ff_siwis".into(), langue: "fr".into() }, &c).unwrap();
    assert_eq!(voix.octets, wav, "une réponse audio/wav brute est reconnue aussi");
    assert!(requete_texte(&s, 1).starts_with("POST /v1/voix HTTP/1.1"));
    assert_eq!(corps_json(&s, 1), json!({"version": 1, "texte": "Bonjour.", "personnage": "MARIE", "voix": "ff_siwis", "langue": "fr"}));
}

#[test]
fn a_sound_reply_must_be_a_sound() {
    let r = |ct: &str, corps: &[u8]| ReponseHttp { statut: 200, entetes: vec![("content-type".into(), ct.into())], corps: corps.to_vec() };
    assert!(crate::sidecar::son_depuis_reponse(&r("audio/wav", b"")).is_err(), "son vide");
    assert!(crate::sidecar::son_depuis_reponse(&r("audio/mpeg", b"x")).is_ok_and(|s| s.ext == "mp3"));
    assert!(crate::sidecar::son_depuis_reponse(&r("audio/x-inconnu", b"x")).is_err());
    assert!(crate::sidecar::son_depuis_reponse(&r("application/json", b"{\"format\": \"wav\"}")).is_err(), "pas de son dans le JSON");
    assert!(crate::sidecar::son_depuis_reponse(&r("application/json", b"{\"audio_b64\": \"AAAA\", \"format\": \"exe\"}")).is_err(), "format refusé");
    assert!(crate::sidecar::son_depuis_reponse(&r("application/json", b"{\"audio_b64\": \"AAAA\"}")).is_ok());
}

#[test]
fn the_local_reader_sends_the_plan_and_its_thumbnail_and_reads_the_cues() {
    let reponse = json!({"indications": [
        {"image": 3, "images": 20, "genre": "bruitage", "description": "Porte", "prompt": "door slam"},
        {"image": 0, "images": 9, "genre": "ambiance", "description": "Pluie", "prompt": "rain"},
        {"image": 0, "images": 9, "genre": "tuba", "description": "Écarté"},
    ]});
    let s = serveur(vec![http_json("200 OK", &reponse)]);
    let mut p = plan("La porte claque.", "MARIE : Oh.", "SFX : x", 48);
    p.vignette_png = Some(vec![0x89, b'P', b'N', b'G', 1, 2, 3]);
    p.transition_entrante = Some("Fondu enchaîné, 12 images".into());
    let demande = DemandeLecture { plans: vec![plan("Avant.", "", "", 10), p], fps: 24 };
    let v = LecteurSidecar(ClientSidecar::sur_le_port(s.port)).lire_plan(&demande, 1, &Controle::new()).unwrap();
    assert_eq!(descriptions(&v), vec!["Porte", "Pluie"]);
    assert!(requete_texte(&s, 0).starts_with("POST /v1/lecture HTTP/1.1"));
    let corps = corps_json(&s, 0);
    assert_eq!(corps["version"], 1);
    assert_eq!(corps["langue"], "fr");
    assert_eq!(corps["consignes"], CONSIGNES);
    assert_eq!(corps["plan"]["action"], "La porte claque.");
    assert_eq!(corps["plan"]["dialogue"], "MARIE : Oh.");
    assert_eq!(corps["plan"]["images"], 48);
    assert_eq!(corps["plan"]["transition_entrante"], "Fondu enchaîné, 12 images");
    assert_eq!(corps["plan"]["plan_precedent"]["action"], "Avant.");
    assert_eq!(base64::decode(corps["vignette_png_b64"].as_str().unwrap()).unwrap(), vec![0x89, b'P', b'N', b'G', 1, 2, 3]);
}

#[test]
fn with_no_server_installed_every_local_role_says_not_installed() {
    let libre = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let client = ClientSidecar::sur_le_port(libre);
    let c = Controle::new();
    let demande = DemandeLecture { plans: vec![plan("x", "", "", 10)], fps: 24 };
    assert!(matches!(LecteurSidecar(client.clone()).lire_plan(&demande, 0, &c), Err(ErreurGen::NonInstalle(_))));
    assert!(matches!(SonSidecar(client.clone()).generer(&DemandeSon { genre: Genre::Bruitage, prompt: "x".into(), secondes: 1.0 }, &c), Err(ErreurGen::NonInstalle(_))));
    assert!(matches!(VoixSidecar(client.clone()).generer(&DemandeVoix { texte: "x".into(), personnage: String::new(), voix: String::new(), langue: "fr".into() }, &c), Err(ErreurGen::NonInstalle(_))));
    let texte = crate::generation::tester_local(libre, &c);
    assert!(texte.starts_with("Non installé"), "{texte}");
}

// ================================================================================ services en ligne

/// Un transport qui note les requêtes et rend une réponse choisie. Il ne sort jamais de l'essai.
struct TransportSimule {
    requetes: Mutex<Vec<RequeteHttps>>,
    reponse: Mutex<ReponseHttp>,
}

impl TransportSimule {
    fn new(statut: u16, ct: &str, corps: Vec<u8>) -> Arc<TransportSimule> {
        Arc::new(TransportSimule { requetes: Mutex::new(Vec::new()), reponse: Mutex::new(ReponseHttp { statut, entetes: vec![("content-type".into(), ct.into())], corps }) })
    }
}

impl Transport for TransportSimule {
    fn envoyer(&self, r: &RequeteHttps, _: &Controle) -> Result<ReponseHttp, ErreurGen> {
        self.requetes.lock().unwrap().push(r.clone());
        Ok(self.reponse.lock().unwrap().clone())
    }
}

const CLE: &str = "sk-essai-SECRET-1234567890";

fn coffre_avec(service: Service) -> Arc<CoffreMemoire> {
    let c = Arc::new(CoffreMemoire::default());
    c.ecrire(service, &Cle::new(CLE)).unwrap();
    c
}

fn corps_requete(r: &RequeteHttps) -> Value {
    serde_json::from_slice(&r.corps).unwrap()
}

#[test]
fn the_claude_request_carries_the_instructions_the_thumbnail_and_the_plan() {
    let mut p = plan("La porte claque.", "MARIE : Oh.", "", 48);
    p.vignette_png = Some(vec![0x89, b'P', b'N', b'G']);
    let demande = DemandeLecture { plans: vec![p], fps: 24 };
    let r = fournisseursapi::requete_claude(&demande, 0, "claude-modele-essai", &Cle::new(CLE));
    assert_eq!((r.methode, r.url.as_str()), ("POST", "https://api.anthropic.com/v1/messages"));
    assert_eq!(r.entete("x-api-key"), Some(CLE));
    assert_eq!(r.entete("anthropic-version"), Some("2023-06-01"));
    assert_eq!(r.entete("content-type"), Some("application/json"));
    let v = corps_requete(&r);
    assert_eq!(v["model"], "claude-modele-essai");
    assert_eq!(v["system"], CONSIGNES);
    assert!(v["max_tokens"].as_u64().unwrap() >= 1024);
    let contenu = &v["messages"][0]["content"];
    assert_eq!(contenu[0]["type"], "image");
    assert_eq!(contenu[0]["source"]["media_type"], "image/png");
    assert_eq!(base64::decode(contenu[0]["source"]["data"].as_str().unwrap()).unwrap(), vec![0x89, b'P', b'N', b'G']);
    assert_eq!(contenu[1]["type"], "text");
    assert!(contenu[1]["text"].as_str().unwrap().contains("La porte claque."));
    // Sans vignette : le texte seul.
    let sans = DemandeLecture { plans: vec![plan("x", "", "", 5)], fps: 24 };
    let v = corps_requete(&fournisseursapi::requete_claude(&sans, 0, "m", &Cle::new(CLE)));
    assert_eq!(v["messages"][0]["content"].as_array().unwrap().len(), 1);
}

#[test]
fn a_key_never_shows_in_a_debug_print_of_a_request_or_of_the_key() {
    let demande = DemandeLecture { plans: vec![plan("x", "", "", 5)], fps: 24 };
    let claude = fournisseursapi::requete_claude(&demande, 0, "m", &Cle::new(CLE));
    let sfx = fournisseursapi::requete_son_elevenlabs(&DemandeSon { genre: Genre::Bruitage, prompt: "x".into(), secondes: 2.0 }, &Cle::new(CLE));
    let stable = fournisseursapi::requete_son_stable_audio(&DemandeSon { genre: Genre::Bruitage, prompt: "x".into(), secondes: 2.0 }, &Cle::new(CLE));
    for r in [&claude, &sfx, &stable] {
        let texte = format!("{r:?}");
        assert!(!texte.contains("SECRET") && !texte.contains(CLE), "{texte}");
        assert!(texte.contains("<masqué>"), "{texte}");
    }
    let cle = format!("{:?}", Cle::new(CLE));
    assert!(!cle.contains("SECRET") && cle.contains("masquée"), "{cle}");
}

#[test]
fn the_claude_reply_is_read_and_its_errors_are_explained() {
    let texte = "```json\n{\"indications\": [{\"image\": 1, \"images\": 12, \"genre\": \"bruitage\", \"description\": \"Porte\", \"prompt\": \"door\"}]}\n```";
    let ok = json!({"content": [{"type": "text", "text": texte}], "stop_reason": "end_turn"});
    let r = ReponseHttp { statut: 200, entetes: vec![], corps: serde_json::to_vec(&ok).unwrap() };
    let v = fournisseursapi::indications_de_reponse_claude(&r, 48).unwrap();
    assert_eq!(descriptions(&v), vec!["Porte"]);
    let coupe = json!({"content": [{"type": "text", "text": "{\"indications\": ["}], "stop_reason": "max_tokens"});
    let r = ReponseHttp { statut: 200, entetes: vec![], corps: serde_json::to_vec(&coupe).unwrap() };
    assert!(matches!(fournisseursapi::indications_de_reponse_claude(&r, 48), Err(ErreurGen::Invalide(m)) if m.contains("coupée")));
    // Les erreurs de l'API.
    let erreur = |statut: u16, message: &str| ReponseHttp { statut, entetes: vec![], corps: serde_json::to_vec(&json!({"type": "error", "error": {"type": "x", "message": message}})).unwrap() };
    assert!(matches!(fournisseursapi::erreur_service(Service::Claude, &erreur(401, "invalid x-api-key")), ErreurGen::Refuse(m) if m.contains("clé de Claude est refusée") && m.contains("invalid x-api-key")));
    assert!(matches!(fournisseursapi::erreur_service(Service::ElevenLabs, &erreur(429, "")), ErreurGen::Refuse(m) if m.contains("limite")));
    assert!(matches!(fournisseursapi::erreur_service(Service::Claude, &erreur(404, "model: nope")), ErreurGen::Invalide(m) if m.contains("modèle")));
    assert!(matches!(fournisseursapi::erreur_service(Service::StableAudio, &erreur(402, "")), ErreurGen::Refuse(m) if m.contains("crédit")));
    assert!(matches!(fournisseursapi::erreur_service(Service::Claude, &erreur(529, "overloaded")), ErreurGen::Indisponible(_)));
    assert!(matches!(fournisseursapi::erreur_service(Service::Claude, &erreur(418, "")), ErreurGen::Invalide(_)));
}

#[test]
fn the_online_readers_and_generators_use_the_transport_with_the_key_from_the_vault() {
    // Claude, de bout en bout contre un transport simulé.
    let texte = "{\"indications\": [{\"image\": 0, \"images\": 12, \"genre\": \"ambiance\", \"description\": \"Pluie\", \"prompt\": \"rain\"}]}";
    let t = TransportSimule::new(200, "application/json", serde_json::to_vec(&json!({"content": [{"type": "text", "text": texte}], "stop_reason": "end_turn"})).unwrap());
    let lecteur = LecteurClaude { transport: t.clone(), coffre: coffre_avec(Service::Claude), modele: "m".into() };
    let demande = DemandeLecture { plans: vec![plan("Il pleut.", "", "", 24)], fps: 24 };
    let v = lecteur.lire_plan(&demande, 0, &Controle::new()).unwrap();
    assert_eq!(descriptions(&v), vec!["Pluie"]);
    assert_eq!(t.requetes.lock().unwrap()[0].entete("x-api-key"), Some(CLE));
    assert!(lecteur.envoie_en_ligne().is_some_and(|m| m.contains("vignettes") && m.contains("Claude")));
    // ElevenLabs, bruitage puis voix.
    let mp3 = vec![0xFF, 0xFB, 0x90, 0x00, 1, 2, 3];
    let t = TransportSimule::new(200, "audio/mpeg", mp3.clone());
    let son = SonElevenLabs { transport: t.clone(), coffre: coffre_avec(Service::ElevenLabs) };
    let g = son.generer(&DemandeSon { genre: Genre::Bruitage, prompt: "door slam".into(), secondes: 3.0 }, &Controle::new()).unwrap();
    assert_eq!((g.ext.as_str(), g.octets), ("mp3", mp3.clone()));
    let voix = VoixElevenLabs { transport: t.clone(), coffre: coffre_avec(Service::ElevenLabs) };
    voix.generer(&DemandeVoix { texte: "Bonjour.".into(), personnage: "M".into(), voix: "Voix_1-a".into(), langue: "fr".into() }, &Controle::new()).unwrap();
    let reqs = t.requetes.lock().unwrap();
    assert_eq!(reqs[0].url, "https://api.elevenlabs.io/v1/sound-generation?output_format=mp3_44100_128");
    assert_eq!(reqs[0].entete("xi-api-key"), Some(CLE));
    assert_eq!(corps_requete(&reqs[0])["text"], "door slam");
    assert_eq!(reqs[1].url, "https://api.elevenlabs.io/v1/text-to-speech/Voix_1-a?output_format=mp3_44100_128");
    assert_eq!(corps_requete(&reqs[1])["text"], "Bonjour.");
    assert_eq!(corps_requete(&reqs[1])["model_id"], "eleven_multilingual_v2");
}

#[test]
fn elevenlabs_durations_are_clamped_and_a_voice_id_cannot_change_the_address() {
    let sfx = |s: f32| corps_requete(&fournisseursapi::requete_son_elevenlabs(&DemandeSon { genre: Genre::Bruitage, prompt: "x".into(), secondes: s }, &Cle::new(CLE)))["duration_seconds"].as_f64().unwrap();
    assert_eq!((sfx(60.0), sfx(0.1), sfx(3.0)), (22.0, 0.5, 3.0));
    let demande = |voix: &str| DemandeVoix { texte: "x".into(), personnage: String::new(), voix: voix.into(), langue: "fr".into() };
    assert!(matches!(fournisseursapi::requete_voix_elevenlabs(&demande(""), &Cle::new(CLE)), Err(ErreurGen::Invalide(m)) if m.contains("aucune voix")));
    for mauvaise in ["../user", "a/b", "voix?x=1", "voix 2", "é"] {
        assert!(fournisseursapi::requete_voix_elevenlabs(&demande(mauvaise), &Cle::new(CLE)).is_err(), "{mauvaise}");
    }
    assert!(fournisseursapi::requete_voix_elevenlabs(&demande("21m00Tcm4TlvDq8ikWAM"), &Cle::new(CLE)).is_ok());
}

#[test]
fn the_stable_audio_request_is_a_multipart_form_with_a_bearer_key() {
    let r = fournisseursapi::requete_son_stable_audio(&DemandeSon { genre: Genre::Ambiance, prompt: "rain on a roof".into(), secondes: 500.0 }, &Cle::new(CLE));
    assert_eq!(r.url, "https://api.stability.ai/v2beta/audio/stable-audio-2/text-to-audio");
    assert_eq!(r.entete("authorization"), Some(format!("Bearer {CLE}").as_str()));
    assert_eq!(r.entete("accept"), Some("audio/*"));
    let ct = r.entete("content-type").unwrap();
    let frontiere = ct.strip_prefix("multipart/form-data; boundary=").unwrap();
    let corps = String::from_utf8(r.corps.clone()).unwrap();
    assert!(corps.starts_with(&format!("--{frontiere}\r\n")) && corps.ends_with(&format!("--{frontiere}--\r\n")), "{corps}");
    for champ in ["name=\"prompt\"\r\n\r\nrain on a roof\r\n", "name=\"duration\"\r\n\r\n190.0\r\n", "name=\"output_format\"\r\n\r\nmp3\r\n"] {
        assert!(corps.contains(champ), "{champ} dans {corps}");
    }
    let t = TransportSimule::new(200, "audio/mpeg", vec![0xFF, 0xFB, 1, 2]);
    let g = SonStableAudio { transport: t, coffre: coffre_avec(Service::StableAudio) }.generer(&DemandeSon { genre: Genre::Bruitage, prompt: "x".into(), secondes: 2.0 }, &Controle::new()).unwrap();
    assert_eq!(g.ext, "mp3");
}

#[test]
fn a_service_that_answers_text_instead_of_a_sound_is_an_error() {
    let t = TransportSimule::new(200, "application/json", b"{\"detail\": \"quota\"}".to_vec());
    let son = SonElevenLabs { transport: t, coffre: coffre_avec(Service::ElevenLabs) };
    assert!(matches!(son.generer(&DemandeSon { genre: Genre::Bruitage, prompt: "x".into(), secondes: 2.0 }, &Controle::new()), Err(ErreurGen::Invalide(m)) if m.contains("quota")));
    let t = TransportSimule::new(200, "audio/mpeg", vec![]);
    let son = SonElevenLabs { transport: t, coffre: coffre_avec(Service::ElevenLabs) };
    assert!(son.generer(&DemandeSon { genre: Genre::Bruitage, prompt: "x".into(), secondes: 2.0 }, &Controle::new()).is_err());
}

#[test]
fn without_a_key_nothing_is_sent_and_the_message_says_where_to_put_it() {
    let t = TransportSimule::new(200, "audio/mpeg", vec![1]);
    let son = SonElevenLabs { transport: t.clone(), coffre: Arc::new(CoffreMemoire::default()) };
    let e = son.generer(&DemandeSon { genre: Genre::Bruitage, prompt: "x".into(), secondes: 2.0 }, &Controle::new()).unwrap_err();
    assert!(matches!(&e, ErreurGen::CleManquante(m) if m.contains("ElevenLabs") && m.contains("Préférences")), "{e:?}");
    assert!(t.requetes.lock().unwrap().is_empty(), "rien n'est parti");
    // Une clé vide compte pour une absence.
    let c = Arc::new(CoffreMemoire::default());
    c.ecrire(Service::ElevenLabs, &Cle::new("   ")).unwrap();
    let voix = VoixElevenLabs { transport: t.clone(), coffre: c };
    assert!(matches!(voix.generer(&DemandeVoix { texte: "x".into(), personnage: String::new(), voix: "v".into(), langue: "fr".into() }, &Controle::new()), Err(ErreurGen::CleManquante(_))));
    assert!(t.requetes.lock().unwrap().is_empty());
}

#[test]
fn the_default_online_transport_is_marked_to_be_plugged_in_and_says_so() {
    let coffre = coffre_avec(Service::Claude);
    let lecteur = LecteurClaude { transport: Arc::new(TransportABrancher), coffre: coffre.clone(), modele: "m".into() };
    let demande = DemandeLecture { plans: vec![plan("x", "", "", 5)], fps: 24 };
    let e = lecteur.lire_plan(&demande, 0, &Controle::new()).unwrap_err();
    assert!(matches!(&e, ErreurGen::ABrancher(m) if m.contains("TLS") && m.contains("serveur local")), "{e:?}");
    let message = e.to_string();
    assert!(message.contains("à brancher") && !message.contains(CLE), "{message}");
    // Le bouton « Tester la connexion » le dit aussi, sans montrer la clé.
    let t = fournisseursapi::tester(Service::Claude, coffre.as_ref(), &TransportABrancher, &Controle::new());
    assert!(t.starts_with("Clé enregistrée pour Claude, mais") && !t.contains(CLE), "{t}");
    let t = fournisseursapi::tester(Service::ElevenLabs, coffre.as_ref(), &TransportABrancher, &Controle::new());
    assert!(t.contains("aucune clé"), "{t}");
    // Un transport qui répond : la connexion est réussie ; une clé refusée est dite.
    let bon = TransportSimule::new(200, "application/json", b"{}".to_vec());
    assert!(fournisseursapi::tester(Service::Claude, coffre.as_ref(), bon.as_ref(), &Controle::new()).contains("réussie"));
    assert_eq!(bon.requetes.lock().unwrap()[0].url, "https://api.anthropic.com/v1/models");
    let refus = TransportSimule::new(401, "application/json", b"{}".to_vec());
    assert!(fournisseursapi::tester(Service::Claude, coffre.as_ref(), refus.as_ref(), &Controle::new()).contains("refusée"));
}

// ===================================================================================== les clés

#[test]
fn the_memory_vault_keeps_one_key_per_service() {
    let c = CoffreMemoire::default();
    assert!(!c.presente(Service::Claude));
    c.ecrire(Service::Claude, &Cle::new("  abc  ")).unwrap();
    assert_eq!(c.lire(Service::Claude).unwrap().unwrap().voir(), "abc", "les espaces autour sont retirés");
    assert!(c.presente(Service::Claude) && !c.presente(Service::ElevenLabs));
    c.effacer(Service::Claude).unwrap();
    assert!(!c.presente(Service::Claude));
    c.effacer(Service::Claude).unwrap();
}

/// Le vrai Gestionnaire d'identifiants de Windows, sous un nom d'essai (jamais « SHOT/… ») que l'essai efface.
#[cfg(windows)]
#[test]
fn the_windows_credential_manager_stores_reads_and_deletes_a_key() {
    let c = crate::clessecretes::CoffreWindows::avec_prefixe(&format!("SHOT-essai-{}", std::process::id()));
    let _ = c.effacer(Service::StableAudio);
    assert!(c.lire(Service::StableAudio).unwrap().is_none(), "rien au départ");
    c.ecrire(Service::StableAudio, &Cle::new("valeur-d-essai-é-1234")).unwrap();
    assert_eq!(c.lire(Service::StableAudio).unwrap().unwrap().voir(), "valeur-d-essai-é-1234");
    c.ecrire(Service::StableAudio, &Cle::new("seconde-valeur")).unwrap();
    assert_eq!(c.lire(Service::StableAudio).unwrap().unwrap().voir(), "seconde-valeur", "écrire remplace");
    assert!(c.ecrire(Service::StableAudio, &Cle::new(&"x".repeat(5000))).is_err(), "trop long pour le gestionnaire");
    c.effacer(Service::StableAudio).unwrap();
    assert!(c.lire(Service::StableAudio).unwrap().is_none());
    c.effacer(Service::StableAudio).unwrap();
}

// ================================================================================== les réglages

#[test]
fn the_generation_settings_have_serde_defaults_that_work_without_installing_anything() {
    let d = GenerationSonReglages::default();
    assert_eq!((d.lecture, d.son, d.voix), (Mode::Regles, Mode::Local, Mode::Local), "la lecture par règles marche tout de suite");
    assert_eq!((d.lecture_service, d.son_service, d.voix_service), (Service::Claude, Service::ElevenLabs, Service::ElevenLabs));
    assert_eq!(d.port_local, 8765);
    assert!(d.roles_en_ligne().is_empty(), "rien ne part en ligne par défaut");
    assert_eq!(d.voix_par_defaut(), "ff_siwis");
    // Des réglages d'avant la fonction : rien ne change.
    let vieux: crate::settings::Settings = toml::from_str("copie_secours_minutes = 2").unwrap();
    assert_eq!(vieux.generation_son, d);
    // Une section à moitié écrite se complète.
    let moitie: crate::settings::Settings = toml::from_str("[generation_son]\nlecture = \"local\"\nport_local = 9000").unwrap();
    assert_eq!((moitie.generation_son.lecture, moitie.generation_son.port_local, moitie.generation_son.son), (Mode::Local, 9000, Mode::Local));
    // Aller-retour.
    let mut s = crate::settings::Settings::default();
    s.generation_son.lecture = Mode::EnLigne;
    s.generation_son.son = Mode::EnLigne;
    s.generation_son.son_service = Service::StableAudio;
    s.generation_son.voix_en_ligne = "21m00Tcm4TlvDq8ikWAM".into();
    let texte = toml::to_string(&s).unwrap();
    assert!(texte.contains("[generation_son]") && texte.contains("lecture = \"en_ligne\"") && texte.contains("sons_service = \"stableaudio\""), "{texte}");
    let relu: crate::settings::Settings = toml::from_str(&texte).unwrap();
    assert_eq!(relu.generation_son, s.generation_son);
    assert_eq!(s.generation_son.roles_en_ligne(), vec![(Role::Lecture, Service::Claude), (Role::Son, Service::StableAudio)]);
}

#[test]
fn the_generation_settings_never_hold_a_key_and_are_brought_back_in_range() {
    let texte = toml::to_string(&crate::settings::Settings::default()).unwrap();
    let section = texte.split("[generation_son]").nth(1).unwrap();
    for interdit in ["cle =", "key", "secret", "token", "password", "mot_de_passe"] {
        assert!(!section.to_lowercase().contains(interdit), "{interdit} dans {section}");
    }
    let mut r = GenerationSonReglages { son: Mode::Regles, voix: Mode::Regles, port_local: 0, modele_claude: "  ".into(), voix_locale: format!(" {} ", "v".repeat(300)), ..Default::default() };
    r.lecture_service = Service::StableAudio;
    r.son_service = Service::Claude;
    r.voix_service = Service::StableAudio;
    let r = r.sanitized();
    assert_eq!((r.son, r.voix), (Mode::Local, Mode::Local), "les règles ne génèrent ni sons ni voix");
    assert_eq!((r.lecture_service, r.son_service, r.voix_service), (Service::Claude, Service::ElevenLabs, Service::ElevenLabs));
    assert_eq!(r.port_local, 8765);
    assert_eq!(r.modele_claude, fournisseursapi::MODELE_CLAUDE);
    assert_eq!(r.voix_locale.chars().count(), 80);
    // Un service qui ne sait pas faire le rôle n'est pas accepté.
    let mut r = GenerationSonReglages::default();
    r.definir_service(Role::Voix, Service::Claude);
    assert_eq!(r.voix_service, Service::ElevenLabs);
    r.definir_service(Role::Son, Service::StableAudio);
    assert_eq!(r.son_service, Service::StableAudio);
}

// ============================================================================ tâches simulées

fn attendre(tache: &mut Tache, sons: &mut SonsProposes) -> Rapport {
    let mut rapport = Rapport::default();
    let debut = Instant::now();
    while !tache.poll(sons, &mut rapport) {
        assert!(debut.elapsed() < Duration::from_secs(20), "tâche trop longue");
        std::thread::sleep(Duration::from_millis(2));
    }
    rapport
}

fn dossier(nom: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("shot-test-generation-{nom}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    d
}

fn travail(cue: u64, genre: Genre, prompt: &str) -> Travail {
    Travail { cue, genre, prompt: prompt.into(), secondes: 1.0, texte: if genre.est_voix() { prompt.into() } else { String::new() }, personnage: String::new(), voix: "v".into() }
}

#[test]
fn a_reading_task_reads_the_plans_in_a_thread_and_merges_each_one() {
    let lecteur = Arc::new(LecteurSimule::new(|p| vec![IndicationBrute { start: 0, frames: 10, kind: Some(Genre::Bruitage), description: format!("Son de {}", p.titre), ..Default::default() }]));
    let demande = DemandeLecture { plans: vec![EntreePlan { plan: 1, titre: "Plan 1".into(), frames: 10, ..Default::default() }, EntreePlan { plan: 2, titre: "Plan 2".into(), frames: 10, ..Default::default() }], fps: 24 };
    let controle = Controle::new();
    let rx = lancer_lecture(lecteur.clone(), demande, controle.clone());
    let mut tache = Tache::new(Role::Lecture, "Lecture".into(), controle.clone(), rx, Default::default());
    let mut sons = SonsProposes::default();
    let rapport = attendre(&mut tache, &mut sons);
    assert!(rapport.erreurs.is_empty() && rapport.change);
    assert_eq!(tache.totaux.ajoutees, 2);
    assert_eq!(sons.du_plan(1).next().unwrap().description, "Son de Plan 1");
    assert_eq!(sons.du_plan(2).next().unwrap().description, "Son de Plan 2");
    // L'avancement va jusqu'au bout.
    let (fait, total, _) = controle.lire();
    assert_eq!((fait, total), (2, 2));
}

#[test]
fn a_reading_task_stops_on_a_hard_error_but_goes_on_after_a_bad_reply() {
    let demande = |n: u64| DemandeLecture { plans: (1..=n).map(|k| EntreePlan { plan: k, titre: format!("Plan {k}"), frames: 10, ..Default::default() }).collect(), fps: 24 };
    // Un serveur qui n'est pas installé : on s'arrête au premier plan, avec un seul message.
    let lecteur = Arc::new(LecteurSimule::new(|_| vec![]));
    *lecteur.echec.lock().unwrap() = Some(ErreurGen::NonInstalle("aucun serveur".into()));
    let controle = Controle::new();
    let mut tache = Tache::new(Role::Lecture, "Lecture".into(), controle.clone(), lancer_lecture(lecteur.clone(), demande(3), controle), Default::default());
    let rapport = attendre(&mut tache, &mut SonsProposes::default());
    assert_eq!(rapport.erreurs.len(), 1, "{:?}", rapport.erreurs);
    assert!(rapport.erreurs[0].contains("non installé"));
    assert_eq!(lecteur.appels.lock().unwrap().len(), 1, "les plans suivants ne sont pas essayés");
    // Une réponse inexploitable pour un plan : on continue.
    let lecteur = Arc::new(LecteurSimule::new(|_| vec![]));
    *lecteur.echec.lock().unwrap() = Some(ErreurGen::Invalide("réponse illisible".into()));
    let controle = Controle::new();
    let mut tache = Tache::new(Role::Lecture, "Lecture".into(), controle.clone(), lancer_lecture(lecteur.clone(), demande(3), controle), Default::default());
    let rapport = attendre(&mut tache, &mut SonsProposes::default());
    assert_eq!(rapport.erreurs.len(), 3);
    assert_eq!(lecteur.appels.lock().unwrap().len(), 3);
}

#[test]
fn cancelling_a_reading_stops_it_between_two_plans() {
    let mut lecteur = LecteurSimule::new(|_| vec![]);
    lecteur.delai = Duration::from_millis(60);
    let lecteur = Arc::new(lecteur);
    let demande = DemandeLecture { plans: (1..=50).map(|k| EntreePlan { plan: k, frames: 10, ..Default::default() }).collect(), fps: 24 };
    let controle = Controle::new();
    let mut tache = Tache::new(Role::Lecture, "Lecture".into(), controle.clone(), lancer_lecture(lecteur.clone(), demande, controle.clone()), Default::default());
    std::thread::sleep(Duration::from_millis(150));
    controle.annuler();
    let t = Instant::now();
    let rapport = attendre(&mut tache, &mut SonsProposes::default());
    assert!(t.elapsed() < Duration::from_secs(2));
    assert!(rapport.erreurs.is_empty(), "annuler n'est pas une erreur : {:?}", rapport.erreurs);
    assert!(lecteur.appels.lock().unwrap().len() < 10, "{}", lecteur.appels.lock().unwrap().len());
}

#[test]
fn a_generation_task_writes_each_sound_to_the_cache_and_marks_the_cue_ready() {
    let dir = dossier("ecriture");
    let mut sons = SonsProposes::default();
    let a = sons.ajouter(1, IndicationBrute { frames: 24, kind: Some(Genre::Bruitage), description: "Porte".into(), prompt: "door".into(), ..Default::default() });
    let b = sons.ajouter(1, IndicationBrute { frames: 24, kind: Some(Genre::Voix), description: "Voix".into(), text: "Bonjour.".into(), ..Default::default() });
    for id in [a, b] {
        sons.get_mut(id).unwrap().status = Statut::EnGeneration;
    }
    let (son, voix) = (Arc::new(SonSimule::new()), Arc::new(VoixSimulee::new()));
    let controle = Controle::new();
    let rx = lancer_generation(son.clone(), voix.clone(), vec![travail(a, Genre::Bruitage, "door"), travail(b, Genre::Voix, "Bonjour.")], dir.clone(), controle.clone());
    let avant = [(a, Statut::Proposee), (b, Statut::Proposee)].into_iter().collect();
    let mut tache = Tache::new(Role::Son, "Génération".into(), controle, rx, avant);
    let rapport = attendre(&mut tache, &mut sons);
    assert!(rapport.erreurs.is_empty() && tache.prets == 2);
    for id in [a, b] {
        let i = sons.get(id).unwrap();
        assert_eq!(i.status, Statut::Prete);
        let f = i.file.as_ref().expect("fichier");
        assert!(f.starts_with(&dir) && f.is_file(), "{f:?}");
        assert!(crate::audio::decode(&std::fs::read(f).unwrap(), "wav").is_ok());
    }
    assert_ne!(sons.get(a).unwrap().file, sons.get(b).unwrap().file, "un fichier par son");
    assert_eq!(son.appels.lock().unwrap().len(), 1, "le bruitage va au générateur de sons");
    assert_eq!(voix.appels.lock().unwrap()[0].texte, "Bonjour.", "la réplique va au générateur de voix");
    assert_eq!(voix.appels.lock().unwrap()[0].voix, "v");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cancelling_a_generation_restores_every_cue_and_leaves_no_file() {
    let dir = dossier("annulation");
    let mut sons = SonsProposes::default();
    let ids: Vec<u64> = (0..3).map(|k| sons.ajouter(1, IndicationBrute { frames: 24, kind: Some(Genre::Bruitage), description: format!("Son {k}"), prompt: format!("p{k}"), ..Default::default() })).collect();
    // Le premier avait déjà un son prêt (une régénération), les autres étaient seulement proposés.
    sons.get_mut(ids[0]).unwrap().file = Some(std::path::PathBuf::from("C:/cache/ancien.wav"));
    let avant: std::collections::HashMap<u64, Statut> = [(ids[0], Statut::Prete), (ids[1], Statut::Proposee), (ids[2], Statut::Rejetee)].into_iter().collect();
    for (&id, _) in &avant {
        sons.get_mut(id).unwrap().status = Statut::EnGeneration;
    }
    let son = Arc::new(SonSimule::new());
    son.bloque.store(true, Ordering::Relaxed);
    let controle = Controle::new();
    let travaux = ids.iter().map(|&id| travail(id, Genre::Bruitage, "p")).collect();
    let rx = lancer_generation(son.clone(), Arc::new(VoixSimulee::new()), travaux, dir.clone(), controle.clone());
    let mut tache = Tache::new(Role::Son, "Génération".into(), controle.clone(), rx, avant);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(son.appels.lock().unwrap().len(), 1, "le premier son est en cours");
    controle.annuler();
    let t = Instant::now();
    let rapport = attendre(&mut tache, &mut sons);
    assert!(t.elapsed() < Duration::from_secs(2));
    assert!(rapport.erreurs.is_empty(), "{:?}", rapport.erreurs);
    assert_eq!(sons.get(ids[0]).unwrap().status, Statut::Prete);
    assert_eq!(sons.get(ids[1]).unwrap().status, Statut::Proposee);
    assert_eq!(sons.get(ids[2]).unwrap().status, Statut::Rejetee);
    assert_eq!(sons.get(ids[0]).unwrap().file.as_deref(), Some(std::path::Path::new("C:/cache/ancien.wav")), "l'ancien son est gardé");
    assert!(!dir.exists() || std::fs::read_dir(&dir).unwrap().next().is_none(), "aucun fichier écrit");
    assert_eq!(son.appels.lock().unwrap().len(), 1, "les suivants n'ont pas été essayés");
}

#[test]
fn a_hard_error_stops_a_generation_and_a_bad_reply_does_not() {
    let dir = dossier("erreurs");
    let mut sons = SonsProposes::default();
    let ids: Vec<u64> = (0..3).map(|k| sons.ajouter(1, IndicationBrute { frames: 24, kind: Some(Genre::Bruitage), description: format!("Son {k}"), prompt: format!("p{k}"), ..Default::default() })).collect();
    for &id in &ids {
        sons.get_mut(id).unwrap().status = Statut::EnGeneration;
    }
    let avant: std::collections::HashMap<u64, Statut> = ids.iter().map(|&i| (i, Statut::Proposee)).collect();
    // Le deuxième échoue d'une réponse inexploitable : le troisième est quand même généré.
    let son = Arc::new(SonSimule::new());
    son.invalides.lock().unwrap().push("p1".into());
    let controle = Controle::new();
    let travaux = ids.iter().enumerate().map(|(k, &id)| travail(id, Genre::Bruitage, &format!("p{k}"))).collect();
    let mut tache = Tache::new(Role::Son, "Génération".into(), controle.clone(), lancer_generation(son, Arc::new(VoixSimulee::new()), travaux, dir.clone(), controle), avant.clone());
    let rapport = attendre(&mut tache, &mut sons);
    assert_eq!(rapport.erreurs.len(), 1);
    assert_eq!([Statut::Prete, Statut::Proposee, Statut::Prete], [sons.get(ids[0]).unwrap().status, sons.get(ids[1]).unwrap().status, sons.get(ids[2]).unwrap().status]);
    // Une clé refusée : on s'arrête, et ce qui n'a pas été essayé retrouve son état.
    for &id in &ids {
        let i = sons.get_mut(id).unwrap();
        i.status = Statut::EnGeneration;
        i.file = None;
    }
    let son = Arc::new(SonSimule::new());
    *son.fatale.lock().unwrap() = Some(ErreurGen::Refuse("clé refusée".into()));
    let controle = Controle::new();
    let travaux = ids.iter().map(|&id| travail(id, Genre::Bruitage, "p")).collect();
    let mut tache = Tache::new(Role::Son, "Génération".into(), controle.clone(), lancer_generation(son.clone(), Arc::new(VoixSimulee::new()), travaux, dir.clone(), controle), avant);
    let rapport = attendre(&mut tache, &mut sons);
    assert_eq!(rapport.erreurs.len(), 1, "{:?}", rapport.erreurs);
    assert!(sons.indications.iter().all(|i| i.status == Statut::Proposee), "{:?}", sons.indications.iter().map(|i| i.status).collect::<Vec<_>>());
    assert_eq!(son.appels.lock().unwrap().len(), 1);
    std::fs::remove_dir_all(&dir).ok();
}
