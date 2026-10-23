//! Essais du serveur local de génération sonore, SANS Python ni modèle : un faux serveur qui parle le
//! contrat du §6 de `docs/GENERATION_SON.md` (ses réponses ont été relevées sur le vrai `sidecar/serveur.py`),
//! un faux lanceur de processus, et — sous Windows — un vrai processus (`cmd`) pour l'arrêt de tout l'arbre.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::fournisseurs::{Controle, DemandeLecture, DemandeSon, DemandeVoix, EntreePlan, ErreurGen, GenerateurSon, GenerateurVoix, LecteurStoryboard};
use crate::generation::{Fabrique, FabriqueReelle};
use crate::genreglages::{GenerationSonReglages, Mode};
use crate::indications::Genre;
use crate::serveurlocal::{attendre_pret, config_des_reglages, demarrer_et_attendre, ConfigServeur, Demarrage, GestionnaireServeur, Lanceur, Processus};
use crate::sidecar::{ClientSidecar, LecteurSidecar, SonSidecar, VoixSidecar};

// ------------------------------------------------------------------------- les réponses du vrai serveur

const ETAT: &str = r#"{"version":1,"modeles":{"lecture":{"installe":true,"nom":"Qwen2.5-VL-7B-Instruct-AWQ"},"son":{"installe":false},"voix":{"installe":true,"nom":"Kokoro-82M"}}}"#;
const SON_ABSENT: &str = r#"{"erreur":{"code":"modele_absent","message":"Stable Audio Open n'est pas installé : le dossier du modèle est absent (voir docs/GENERATION_SON.md)."}}"#;
const VOIX_INCONNUE: &str = r#"{"erreur":{"code":"invalide","message":"La voix « zz_nulle » n'est pas installée. Voix disponibles : af_heart, am_michael, ff_siwis."}}"#;
const OCCUPE: &str = r#"{"erreur":{"code":"occupe","message":"Le serveur local travaille déjà sur une autre demande."}}"#;
const LECTURE: &str = r#"{"indications":[{"image":0,"images":96,"genre":"bruitage","description":"Porte qui claque","prompt":"door slams","texte":"","personnage":""},{"image":0,"images":96,"genre":"ambiance","description":"Pluie","prompt":"rain","texte":"","personnage":""},{"image":0,"images":24,"genre":"voix","description":"MARIE : « Viens ici. »","prompt":"","texte":"Viens ici.","personnage":"MARIE"},{"image":34,"images":24,"genre":"voix","description":"PAUL : « Non. »","prompt":"","texte":"Non.","personnage":"PAUL"}]}"#;

fn wav() -> Vec<u8> {
    // Un WAV 16 bits mono 24 kHz de 0,2 s (le format que Kokoro rend).
    let n = 4800u32;
    let mut v = Vec::new();
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + n * 2).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&24000u32.to_le_bytes());
    v.extend_from_slice(&48000u32.to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&(n * 2).to_le_bytes());
    for i in 0..n {
        v.extend_from_slice(&(((i as f32 * 0.05).sin() * 8000.0) as i16).to_le_bytes());
    }
    v
}

fn http(statut: &str, type_contenu: &str, corps: &[u8]) -> Vec<u8> {
    let mut r = format!("HTTP/1.1 {statut}\r\ncontent-type: {type_contenu}\r\ncontent-length: {}\r\nConnection: close\r\n\r\n", corps.len()).into_bytes();
    r.extend_from_slice(corps);
    r
}

/// Ce que le faux serveur sait faire : le contrat du vrai, route par route.
fn reponse_du_contrat(methode: &str, chemin: &str, corps: &Value) -> Vec<u8> {
    let json = |statut: &str, texte: &str| http(statut, "application/json", texte.as_bytes());
    match (methode, chemin) {
        ("GET", "/v1/etat") => json("200 OK", ETAT),
        ("POST", "/v1/son") => json("501 Not Implemented", SON_ABSENT),
        ("POST", "/v1/voix") if corps["voix"] == "zz_nulle" => json("400 Bad Request", VOIX_INCONNUE),
        ("POST", "/v1/voix") => http("200 OK", "audio/wav", &wav()),
        ("POST", "/v1/lecture") => json("200 OK", LECTURE),
        _ => json("404 Not Found", r#"{"erreur":{"code":"invalide","message":"Adresse inconnue."}}"#),
    }
}

type Comportement = Arc<dyn Fn(&str, &str, &Value) -> Vec<u8> + Send + Sync>;

/// Un faux serveur : il note ce qu'il reçoit et répond comme le dit `comportement`.
struct Faux {
    port: u16,
    requetes: Arc<Mutex<Vec<(String, String, Value)>>>,
    arret: Arc<AtomicBool>,
}

impl Faux {
    fn sur(port: u16, comportement: Comportement) -> Faux {
        let ecoute = TcpListener::bind(("127.0.0.1", port)).expect("port libre");
        ecoute.set_nonblocking(true).unwrap();
        let port = ecoute.local_addr().unwrap().port();
        let (requetes, arret) = (Arc::new(Mutex::new(Vec::new())), Arc::new(AtomicBool::new(false)));
        let (r, a) = (requetes.clone(), arret.clone());
        std::thread::spawn(move || {
            while !a.load(Ordering::Relaxed) {
                match ecoute.accept() {
                    Ok((flux, _)) => {
                        let (r, c) = (r.clone(), comportement.clone());
                        std::thread::spawn(move || servir(flux, &r, &c));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        Faux { port, requetes, arret }
    }

    fn contrat() -> Faux {
        Faux::sur(0, Arc::new(|m, c, b| reponse_du_contrat(m, c, b)))
    }
}

impl Drop for Faux {
    fn drop(&mut self) {
        self.arret.store(true, Ordering::Relaxed);
    }
}

fn servir(mut flux: TcpStream, requetes: &Mutex<Vec<(String, String, Value)>>, comportement: &Comportement) {
    flux.set_nonblocking(false).ok();
    flux.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut buf = Vec::new();
    let mut tranche = [0u8; 4096];
    let (tete, corps) = loop {
        let n = flux.read(&mut tranche).unwrap_or(0);
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&tranche[..n]);
        if let Some(c) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let tete = String::from_utf8_lossy(&buf[..c]).into_owned();
            let longueur = tete.to_ascii_lowercase().lines().find_map(|l| l.strip_prefix("content-length:").and_then(|v| v.trim().parse::<usize>().ok())).unwrap_or(0);
            if buf.len() >= c + 4 + longueur {
                break (tete, buf[c + 4..c + 4 + longueur].to_vec());
            }
        }
    };
    let mut mots = tete.lines().next().unwrap_or("").split_whitespace();
    let (methode, chemin) = (mots.next().unwrap_or("").to_string(), mots.next().unwrap_or("").to_string());
    let valeur: Value = serde_json::from_slice(&corps).unwrap_or(Value::Null);
    requetes.lock().unwrap().push((methode.clone(), chemin.clone(), valeur.clone()));
    let _ = flux.write_all(&comportement(&methode, &chemin, &valeur));
}

/// Un port où personne n'écoute, et que personne d'autre ne recevra pendant la suite. (Avant, le port
/// venait d'un `bind(":0")` refermé aussitôt : Windows le redonnait à un faux serveur d'un autre essai
/// (`Faux::contrat`), et l'essai « le serveur meurt au démarrage » tombait sur un serveur bien vivant.
/// Ici : des ports sous la plage que le système donne aux `bind(":0")` (49152 et au-delà), pris un à
/// un, chacun une seule fois.)
pub(crate) fn libre() -> u16 {
    use std::sync::atomic::AtomicU16;
    static SUIVANT: AtomicU16 = AtomicU16::new(0);
    loop {
        let n = SUIVANT.fetch_add(1, Ordering::SeqCst);
        let port = 20_000 + ((std::process::id() % 400) as u16) * 60 + n % 3_000;
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
}

fn plan(action: &str, dialogue: &str) -> EntreePlan {
    EntreePlan { plan: 3, titre: "Plan 3".into(), sequence: "1".into(), frames: 96, action: action.into(), dialogue: dialogue.into(), ..Default::default() }
}

// ----------------------------------------------------------------------------- le faux lanceur

/// Un faux processus de serveur : « vivant » tant que personne ne l'arrête, avec un faux serveur derrière.
struct FauxProcessus {
    vivant: Arc<AtomicBool>,
    _faux: Option<Faux>,
}

impl Processus for FauxProcessus {
    fn vivant(&mut self) -> bool {
        self.vivant.load(Ordering::Relaxed)
    }
    fn arreter(&mut self) {
        self.vivant.store(false, Ordering::Relaxed);
        self._faux = None;
    }
    fn pid(&self) -> u32 {
        4242
    }
}

/// Un lanceur qui compte ses lancements et ouvre un faux serveur du contrat sur le port demandé.
fn lanceur_compteur(lancements: Arc<AtomicUsize>) -> Lanceur {
    Arc::new(move |config: &ConfigServeur| {
        lancements.fetch_add(1, Ordering::SeqCst);
        // (Un serveur met un instant à écouter : le client doit attendre.)
        let port = config.port;
        let faux = Faux::sur(port, Arc::new(|m, c, b| reponse_du_contrat(m, c, b)));
        Ok(Box::new(FauxProcessus { vivant: Arc::new(AtomicBool::new(true)), _faux: Some(faux) }) as Box<dyn Processus>)
    })
}

fn config(port: u16) -> ConfigServeur {
    ConfigServeur { python: "python.exe".into(), racine: "SHOT-modeles".into(), script: "serveur.py".into(), port }
}

// ============================================================================ le contrat, côté client

#[test]
fn the_client_reads_the_state_a_real_server_gives() {
    let s = Faux::contrat();
    let e = ClientSidecar::sur_le_port(s.port).etat(&Controle::new()).unwrap();
    assert_eq!((e.version, e.lecture.installe, e.son.installe, e.voix.installe), (1, true, false, true));
    assert_eq!(e.lecture.nom, "Qwen2.5-VL-7B-Instruct-AWQ");
    let resume = e.resume();
    assert!(resume.contains("lecture prêt (Qwen2.5-VL-7B-Instruct-AWQ)") && resume.contains("sons non installé") && resume.contains("voix prêt (Kokoro-82M)"), "{resume}");
}

#[test]
fn a_voice_comes_back_as_a_wav_and_an_unknown_voice_is_an_invalid_request() {
    let s = Faux::contrat();
    let client = ClientSidecar::sur_le_port(s.port);
    let c = Controle::new();
    let voix = |v: &str| VoixSidecar(client.clone()).generer(&DemandeVoix { texte: "Viens ici.".into(), personnage: "MARIE".into(), voix: v.into(), langue: "fr".into() }, &c);
    let son = voix("ff_siwis").unwrap();
    assert_eq!((son.ext.as_str(), son.octets), ("wav", wav()));
    let e = voix("zz_nulle").unwrap_err();
    assert!(matches!(&e, ErreurGen::Invalide(m) if m.contains("Voix disponibles") && m.contains("ff_siwis")), "{e:?}");
    let envoye = s.requetes.lock().unwrap()[0].2.clone();
    assert_eq!(envoye, json!({"version": 1, "texte": "Viens ici.", "personnage": "MARIE", "voix": "ff_siwis", "langue": "fr"}));
}

#[test]
fn a_missing_sound_model_is_reported_as_not_installed() {
    let s = Faux::contrat();
    let e = SonSidecar(ClientSidecar::sur_le_port(s.port)).generer(&DemandeSon { genre: Genre::Bruitage, prompt: "door".into(), secondes: 2.0 }, &Controle::new()).unwrap_err();
    assert!(matches!(&e, ErreurGen::NonInstalle(m) if m.contains("Stable Audio")), "{e:?}");
}

#[test]
fn a_busy_server_is_reported_as_unavailable_without_launching_anything() {
    let lancements = Arc::new(AtomicUsize::new(0));
    let s = Faux::sur(0, Arc::new(|_, _, _| http("503 Service Unavailable", "application/json", OCCUPE.as_bytes())));
    let gest = GestionnaireServeur::avec_lanceur(lanceur_compteur(lancements.clone()));
    let client = ClientSidecar::sur_le_port(s.port).avec_demarrage(Demarrage { gestionnaire: gest, config: Ok(config(s.port)) });
    let e = VoixSidecar(client).generer(&DemandeVoix { texte: "x".into(), personnage: String::new(), voix: String::new(), langue: "fr".into() }, &Controle::new()).unwrap_err();
    assert!(matches!(&e, ErreurGen::Indisponible(m) if m.contains("autre demande")), "{e:?}");
    assert_eq!(lancements.load(Ordering::SeqCst), 0, "un serveur occupé n'est pas relancé");
}

#[test]
fn the_reader_gets_the_cues_and_the_voices_the_real_server_builds() {
    let s = Faux::contrat();
    let demande = DemandeLecture { plans: vec![plan("Elle ferme la porte en claquant. Il pleut dehors.", "MARIE : Viens ici.\nPAUL : Non.")], fps: 24 };
    let v = LecteurSidecar(ClientSidecar::sur_le_port(s.port)).lire_plan(&demande, 0, &Controle::new()).unwrap();
    let genres: Vec<_> = v.iter().map(|i| (i.kind, i.start, i.frames)).collect();
    assert_eq!(genres, vec![(Some(Genre::Bruitage), 0, 96), (Some(Genre::Ambiance), 0, 96), (Some(Genre::Voix), 0, 24), (Some(Genre::Voix), 34, 24)]);
    assert_eq!(v[0].description, "Porte qui claque");
    assert_eq!(v[0].prompt, "door slams");
    assert_eq!((v[2].character.as_str(), v[2].text.as_str()), ("MARIE", "Viens ici."));
    let envoye = &s.requetes.lock().unwrap()[0].2;
    assert_eq!(envoye["plan"]["images"], 96);
    assert_eq!(envoye["plan"]["dialogue"], "MARIE : Viens ici.\nPAUL : Non.");
    assert_eq!(envoye["version"], 1);
}

// ============================================================================ le lancement automatique

#[test]
fn a_local_role_starts_the_server_when_nobody_listens_and_waits_for_it() {
    let port = libre();
    let lancements = Arc::new(AtomicUsize::new(0));
    let gest = GestionnaireServeur::avec_lanceur(lanceur_compteur(lancements.clone()));
    let client = ClientSidecar::sur_le_port(port).avec_demarrage(Demarrage { gestionnaire: gest.clone(), config: Ok(config(port)) });
    let c = Controle::new();
    c.avancer(1, 3, "Viens ici.");
    let son = VoixSidecar(client.clone()).generer(&DemandeVoix { texte: "Viens ici.".into(), personnage: String::new(), voix: String::new(), langue: "fr".into() }, &c).unwrap();
    assert_eq!(son.octets, wav());
    assert_eq!(lancements.load(Ordering::SeqCst), 1);
    assert!(gest.en_marche() && gest.pid() == Some(4242));
    // Le deuxième appel trouve le serveur : rien n'est relancé.
    VoixSidecar(client).generer(&DemandeVoix { texte: "Encore.".into(), personnage: String::new(), voix: String::new(), langue: "fr".into() }, &c).unwrap();
    assert_eq!(lancements.load(Ordering::SeqCst), 1);
    // Arrêter le rend à l'état « pas lancé par SHOT ».
    assert!(gest.arreter());
    assert!(!gest.en_marche() && gest.pid().is_none());
    assert!(!gest.arreter(), "rien à arrêter la deuxième fois");
}

#[test]
fn two_requests_at_once_start_the_server_only_once() {
    let port = libre();
    let lancements = Arc::new(AtomicUsize::new(0));
    let gest = GestionnaireServeur::avec_lanceur(lanceur_compteur(lancements.clone()));
    let client = ClientSidecar::sur_le_port(port).avec_demarrage(Demarrage { gestionnaire: gest.clone(), config: Ok(config(port)) });
    let fils: Vec<_> = (0..2)
        .map(|_| {
            let client = client.clone();
            std::thread::spawn(move || VoixSidecar(client).generer(&DemandeVoix { texte: "x".into(), personnage: String::new(), voix: String::new(), langue: "fr".into() }, &Controle::new()))
        })
        .collect();
    for f in fils {
        assert!(f.join().unwrap().is_ok());
    }
    assert_eq!(lancements.load(Ordering::SeqCst), 1);
    gest.arreter();
}

#[test]
fn the_factory_starts_the_server_only_when_the_setting_allows_it() {
    let port = libre();
    let lancements = Arc::new(AtomicUsize::new(0));
    let gest = GestionnaireServeur::avec_lanceur(lanceur_compteur(lancements.clone()));
    let fabrique = FabriqueReelle { transport: Arc::new(crate::fournisseursapi::TransportABrancher), coffre: Arc::new(crate::clessecretes::CoffreMemoire::default()), serveur: gest.clone() };
    // Le démarrage automatique est coupé : « non installé », rien n'est lancé.
    let mut r = GenerationSonReglages { voix: Mode::Local, port_local: port, demarrage_auto: false, ..Default::default() };
    let demande = DemandeVoix { texte: "x".into(), personnage: String::new(), voix: String::new(), langue: "fr".into() };
    let e = fabrique.voix(&r).generer(&demande, &Controle::new()).unwrap_err();
    assert!(matches!(e, ErreurGen::NonInstalle(_)), "{e:?}");
    assert_eq!(lancements.load(Ordering::SeqCst), 0);
    // Le réglage d'origine l'autorise ; ici sans Python (le réglage pointe un fichier absent), le lancement est refusé en français.
    r.demarrage_auto = true;
    r.python_local = "C:\\nulle part\\python.exe".into();
    let e = fabrique.voix(&r).generer(&demande, &Controle::new()).unwrap_err();
    assert!(matches!(&e, ErreurGen::NonInstalle(m) if m.contains("Python introuvable") && m.contains("nulle part")), "{e:?}");
    assert_eq!(lancements.load(Ordering::SeqCst), 0);
}

#[test]
fn a_server_that_dies_while_starting_says_why() {
    let port = libre();
    let mort: Lanceur = Arc::new(|_| Ok(Box::new(FauxProcessus { vivant: Arc::new(AtomicBool::new(false)), _faux: None }) as Box<dyn Processus>));
    let gest = GestionnaireServeur::avec_lanceur(mort);
    let e = demarrer_et_attendre(&gest, &config(port), &Controle::new()).unwrap_err();
    assert!(matches!(&e, ErreurGen::NonInstalle(m) if m.contains("s'est arrêté au démarrage")), "{e:?}");
    let refuse: Lanceur = Arc::new(|_| Err("python.exe est introuvable".to_string()));
    let e = demarrer_et_attendre(&GestionnaireServeur::avec_lanceur(refuse), &config(port), &Controle::new()).unwrap_err();
    assert!(matches!(&e, ErreurGen::NonInstalle(m) if m.contains("introuvable")), "{e:?}");
}

#[test]
fn waiting_for_the_server_can_be_cancelled_and_has_a_deadline() {
    let port = libre();
    let vivant: Lanceur = Arc::new(|_| Ok(Box::new(FauxProcessus { vivant: Arc::new(AtomicBool::new(true)), _faux: None }) as Box<dyn Processus>));
    let gest = GestionnaireServeur::avec_lanceur(vivant);
    gest.demarrer(&config(port)).unwrap();
    let client = ClientSidecar::sur_le_port(port);
    let t = Instant::now();
    let e = attendre_pret(&client, &gest, &Controle::new(), Duration::from_millis(400)).unwrap_err();
    assert!(matches!(e, ErreurGen::Indisponible(_)) && t.elapsed() < Duration::from_secs(3), "{e:?}");
    let c = Controle::new();
    let c2 = c.clone();
    let fil = {
        let gest = gest.clone();
        std::thread::spawn(move || attendre_pret(&ClientSidecar::sur_le_port(port), &gest, &c2, Duration::from_secs(60)))
    };
    std::thread::sleep(Duration::from_millis(300));
    c.annuler();
    assert_eq!(fil.join().unwrap().unwrap_err(), ErreurGen::Annule);
}

#[test]
fn testing_the_server_never_starts_it() {
    let port = libre();
    let lancements = Arc::new(AtomicUsize::new(0));
    let gest = GestionnaireServeur::avec_lanceur(lanceur_compteur(lancements.clone()));
    let client = ClientSidecar::sur_le_port(port).avec_demarrage(Demarrage { gestionnaire: gest, config: Ok(config(port)) });
    assert!(matches!(client.etat(&Controle::new()), Err(ErreurGen::NonInstalle(_))));
    assert_eq!(lancements.load(Ordering::SeqCst), 0);
    let texte = crate::generation::tester_local(port, &Controle::new());
    assert!(texte.starts_with("Le serveur local ne tourne pas"), "{texte}");
}

// ============================================================================ les réglages et les chemins

#[test]
fn the_paths_default_to_shot_modeles_in_documents_and_the_settings_override_them() {
    let defaut = GenerationSonReglages::default();
    assert!(defaut.demarrage_auto && defaut.python_local.is_empty() && defaut.modeles_local.is_empty());
    let racine = crate::serveurlocal::racine_par_defaut();
    let neuve = std::path::Path::new("SHOT").join("Modèles").join("SHOT-modeles");
    let ancienne = std::path::Path::new("Work").join("SHOT-modeles");
    assert!(racine.ends_with(&neuve) || racine.ends_with(&ancienne), "{racine:?}");
    assert_eq!(crate::serveurlocal::python_des_reglages(&defaut), crate::serveurlocal::python_de(&racine));
    assert!(crate::serveurlocal::python_de(&racine).ends_with("venv\\Scripts\\python.exe") || crate::serveurlocal::python_de(&racine).ends_with("venv/Scripts/python.exe"));
    let r = GenerationSonReglages { modeles_local: " \"D:\\Modeles\\SHOT\" ".into(), ..Default::default() }.sanitized();
    assert_eq!(r.modeles_local, "D:\\Modeles\\SHOT");
    assert!(crate::serveurlocal::python_des_reglages(&r).starts_with("D:\\Modeles\\SHOT"));
    // Un fichier de réglages d'avant ces champs se lit toujours.
    let ancien: crate::settings::Settings = toml::from_str("[generation_son]\nlecture = \"local\"").unwrap();
    assert!(ancien.generation_son.demarrage_auto && ancien.generation_son.python_local.is_empty());
}

/// Le dossier des modèles a déménagé (2026-10-23) dans `Documents\SHOT\Modèles\SHOT-modeles` : c'est le défaut ;
/// l'ancien `Documents\Work\SHOT-modeles` ne sert que s'il est le seul des deux à exister.
#[test]
fn the_default_models_folder_moved_to_documents_shot_and_falls_back_to_the_old_one() {
    let docs = std::env::temp_dir().join(format!("shot-modeles-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&docs);
    let neuve = docs.join("SHOT").join("Modèles").join("SHOT-modeles");
    let ancienne = docs.join("Work").join("SHOT-modeles");
    // Ni l'un ni l'autre : le nouveau (c'est lui que les Préférences proposent).
    assert_eq!(crate::serveurlocal::racine_par_defaut_dans(&docs), neuve);
    // Seulement l'ancien : l'ancien.
    std::fs::create_dir_all(&ancienne).unwrap();
    assert_eq!(crate::serveurlocal::racine_par_defaut_dans(&docs), ancienne);
    // Les deux (la jonction laissée à l'ancienne place) : le nouveau.
    std::fs::create_dir_all(&neuve).unwrap();
    assert_eq!(crate::serveurlocal::racine_par_defaut_dans(&docs), neuve);
    // Le Python suit la racine.
    assert_eq!(crate::serveurlocal::python_de(&neuve), neuve.join("venv").join("Scripts").join("python.exe"));
    let _ = std::fs::remove_dir_all(&docs);
}

#[test]
fn the_launch_configuration_needs_an_existing_python_and_finds_the_script() {
    assert!(crate::serveurlocal::script_serveur().is_some_and(|s| s.is_file() && s.ends_with("serveur.py")), "sidecar/serveur.py est dans le dépôt");
    let dossier = std::env::temp_dir().join(format!("shot-serveurlocal-{}", std::process::id()));
    std::fs::create_dir_all(&dossier).unwrap();
    let faux_python = dossier.join("python.exe");
    std::fs::write(&faux_python, b"x").unwrap();
    let r = GenerationSonReglages { python_local: faux_python.display().to_string(), modeles_local: dossier.display().to_string(), port_local: 9123, ..Default::default() };
    let c = config_des_reglages(&r).unwrap();
    assert_eq!((c.python, c.racine, c.port), (faux_python, dossier.clone(), 9123));
    let absent = GenerationSonReglages { python_local: dossier.join("absent.exe").display().to_string(), ..Default::default() };
    assert!(config_des_reglages(&absent).unwrap_err().contains("Python introuvable"));
    std::fs::remove_dir_all(&dossier).ok();
}

// ======================================================================= un vrai processus (Windows)

#[cfg(windows)]
mod processus_reel {
    use super::*;
    use std::process::Command;

    /// Les lignes de commande des processus `ping.exe` en cours (PowerShell : présent sur tout Windows).
    fn pings() -> String {
        let sortie = Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", "(Get-CimInstance Win32_Process -Filter \"name='ping.exe'\" | ForEach-Object { $_.CommandLine }) -join '|'"]).output().unwrap();
        String::from_utf8_lossy(&sortie.stdout).into_owned()
    }

    #[test]
    fn stopping_a_started_process_stops_everything_it_started_and_a_lost_handle_does_too() {
        // « cmd » lance « ping » comme le lanceur du venv lance le vrai Python : un arbre de deux processus.
        let marque = format!("{}", 30_000 + std::process::id() % 20_000);
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", &format!("ping -n {marque} 127.0.0.1 >nul")]);
        let mut p = crate::serveurlocal::sys::lancer(cmd, std::path::Path::new("cmd.exe")).unwrap();
        assert!(p.vivant() && p.pid() > 0);
        let t = Instant::now();
        while !pings().contains(&marque) {
            assert!(t.elapsed() < Duration::from_secs(20), "le petit-fils n'a pas démarré");
        }
        p.arreter();
        assert!(!p.vivant());
        let t = Instant::now();
        while pings().contains(&marque) {
            assert!(t.elapsed() < Duration::from_secs(20), "le petit-fils tourne encore : {}", pings());
        }
        // Lâcher le processus sans l'arrêter à la main (le gestionnaire qui disparaît) tue aussi l'arbre.
        let marque2 = format!("{}", 50_000 + std::process::id() % 5_000);
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", &format!("ping -n {marque2} 127.0.0.1 >nul")]);
        let p2 = crate::serveurlocal::sys::lancer(cmd, std::path::Path::new("cmd.exe")).unwrap();
        let t = Instant::now();
        while !pings().contains(&marque2) {
            assert!(t.elapsed() < Duration::from_secs(20));
        }
        drop(p2);
        let t = Instant::now();
        while pings().contains(&marque2) {
            assert!(t.elapsed() < Duration::from_secs(20), "le petit-fils survit au processus perdu");
        }
    }
}
