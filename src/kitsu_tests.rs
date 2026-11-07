//! Essais du pont Kitsu (`kitsu.rs`, `kitsuui.rs`). **Aucun essai ne contacte un vrai serveur** : tout parle à un faux serveur HTTP écrit ici
//! (`std::net::TcpListener` sur 127.0.0.1), qui imite les adresses de l'API de Kitsu telles que `kitsu::ep` les écrit. Le vrai `curl.exe` de
//! Windows fait les requêtes, comme dans l'application. Ces essais prouvent la logique du pont (créer, mettre à jour, publier une seule fois, lire
//! sans doublon, arrêter, erreurs), pas que les adresses de `ep` sont celles d'une vraie instance (voir « À vérifier sur une vraie instance »).

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::document::Document;
use crate::editorial_tests::film;
use crate::exports_v1_tests::dossier_neuf;
use crate::fildecision::{Source, Status};
use crate::kitsu::{self, Action, Client, Erreur, Fichiers, Ids, Progres, Reglages};

// ------------------------------------------------------------------------------------------------- le faux serveur

#[derive(Default)]
struct Etat {
    episodes: Vec<Value>,
    sequences: Vec<Value>,
    shots: Vec<Value>,
    tasks: Vec<Value>,
    comments: Vec<Value>,
    previews: Vec<Value>,
    /// (id de la prévisualisation, octets reçus, le corps est un multipart avec un fichier).
    uploads: Vec<(String, usize, bool)>,
    /// « MÉTHODE chemin » de chaque requête, dans l'ordre.
    journal: Vec<String>,
    suivant: u32,
    jeton: String,
    /// Nombre de téléversements à refuser (500) avant d'accepter.
    echec_upload: u32,
    /// Attente avant chaque réponse, en millisecondes.
    retard_ms: u64,
    /// Les en-têtes d'autorisation vus (pour vérifier que le mot de passe ne voyage que pour la connexion).
    corps_connexion: Vec<String>,
}

struct Faux {
    url: String,
    etat: Arc<Mutex<Etat>>,
    stop: Arc<AtomicBool>,
}

impl Drop for Faux {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Faux {
    fn demarrer() -> Faux {
        let ecoute = TcpListener::bind("127.0.0.1:0").expect("port");
        ecoute.set_nonblocking(true).unwrap();
        let url = format!("http://127.0.0.1:{}", ecoute.local_addr().unwrap().port());
        let etat = Arc::new(Mutex::new(Etat { jeton: "tok-1".into(), ..Default::default() }));
        let stop = Arc::new(AtomicBool::new(false));
        let (e2, s2) = (etat.clone(), stop.clone());
        std::thread::spawn(move || {
            while !s2.load(Ordering::Relaxed) {
                match ecoute.accept() {
                    Ok((flux, _)) => {
                        let e3 = e2.clone();
                        std::thread::spawn(move || {
                            let _ = servir(flux, &e3);
                        });
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(4)),
                }
            }
        });
        Faux { url, etat, stop }
    }

    fn etat(&self) -> std::sync::MutexGuard<'_, Etat> {
        self.etat.lock().unwrap()
    }

    fn nb_ecritures(&self) -> usize {
        self.etat().journal.iter().filter(|j| !j.starts_with("GET ")).count()
    }
}

fn servir(mut flux: TcpStream, etat: &Arc<Mutex<Etat>>) -> std::io::Result<()> {
    // (Sous Windows, une connexion acceptée par un écouteur non bloquant l'est aussi : on la remet bloquante.)
    flux.set_nonblocking(false)?;
    flux.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut tampon = Vec::new();
    let mut bloc = [0u8; 8192];
    let fin_entetes = loop {
        let n = flux.read(&mut bloc)?;
        if n == 0 {
            return Ok(());
        }
        tampon.extend_from_slice(&bloc[..n]);
        if let Some(p) = tampon.windows(4).position(|w| w == b"\r\n\r\n") {
            break p + 4;
        }
    };
    let entetes = String::from_utf8_lossy(&tampon[..fin_entetes]).into_owned();
    let mut lignes = entetes.lines();
    let requete = lignes.next().unwrap_or("").to_string();
    let mut champs: HashMap<String, String> = HashMap::new();
    for l in lignes {
        if let Some((k, v)) = l.split_once(':') {
            champs.insert(k.trim().to_lowercase(), v.trim().to_string());
        }
    }
    let longueur: usize = champs.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
    while tampon.len() < fin_entetes + longueur {
        let n = flux.read(&mut bloc)?;
        if n == 0 {
            break;
        }
        tampon.extend_from_slice(&bloc[..n]);
    }
    let corps = tampon[fin_entetes..].to_vec();
    let mut mots = requete.split_whitespace();
    let (methode, chemin) = (mots.next().unwrap_or("").to_string(), mots.next().unwrap_or("").to_string());
    let retard = etat.lock().unwrap().retard_ms;
    if retard > 0 {
        std::thread::sleep(Duration::from_millis(retard));
    }
    let (code, texte) = route(etat, &methode, &chemin, champs.get("authorization").map(String::as_str), &corps);
    let rep = format!("HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{texte}", texte.len());
    flux.write_all(rep.as_bytes())?;
    flux.flush()?;
    // Fermeture polie : on dit « fini », puis on laisse le client finir de lire (sinon Windows réinitialise la connexion).
    let _ = flux.shutdown(std::net::Shutdown::Write);
    flux.set_read_timeout(Some(Duration::from_millis(1500)))?;
    while matches!(flux.read(&mut bloc), Ok(n) if n > 0) {}
    Ok(())
}

fn nouvel_id(e: &mut Etat, prefixe: &str) -> String {
    e.suivant += 1;
    format!("{prefixe}-{}", e.suivant)
}

fn route(etat: &Arc<Mutex<Etat>>, methode: &str, chemin: &str, auth: Option<&str>, corps: &[u8]) -> (u16, String) {
    let mut e = etat.lock().unwrap();
    e.journal.push(format!("{methode} {chemin}"));
    let Some(p) = chemin.strip_prefix("/api") else { return (404, "{}".into()) };
    let json_corps = || serde_json::from_slice::<Value>(corps).unwrap_or(Value::Null);
    if methode == "POST" && p == "/auth/login" {
        let v = json_corps();
        e.corps_connexion.push(String::from_utf8_lossy(corps).into_owned());
        return if v["email"] == "a@b.c" && v["password"] == "pw" { (200, json!({ "access_token": e.jeton, "login": true }).to_string()) } else { (400, json!({ "login": false, "message": "Mauvais identifiants" }).to_string()) };
    }
    if auth != Some(format!("Bearer {}", e.jeton).as_str()) {
        return (401, json!({ "message": "jeton refusé" }).to_string());
    }
    let seg: Vec<&str> = p.trim_start_matches('/').split('/').collect();
    let ok = |v: Value| (200, v.to_string());
    match (methode, seg.as_slice()) {
        ("GET", ["auth", "authenticated"]) => ok(json!({ "authenticated": true })),
        ("GET", ["data", "projects"]) => ok(json!([{ "id": "p1", "name": "Mon film" }, { "id": "p2", "name": "Autre" }])),
        ("GET", ["data", "task-types"]) => ok(json!([
            { "id": "tt-sb", "name": "Storyboard", "for_entity": "Shot" },
            { "id": "tt-an", "name": "Animation", "for_entity": "Shot" },
            { "id": "tt-dec", "name": "Concept", "for_entity": "Asset" }
        ])),
        ("GET", ["data", "task-status"]) => ok(json!([
            { "id": "st-todo", "name": "Todo", "short_name": "TODO", "color": "#999999" },
            { "id": "st-wip", "name": "Work In Progress", "short_name": "WIP", "color": "#3273dc" },
            { "id": "st-wfa", "name": "Waiting For Approval", "short_name": "WFA", "color": "#ab26ff" },
            { "id": "st-retake", "name": "Retake", "short_name": "RTK", "color": "#ff3860" },
            { "id": "st-done", "name": "Done", "short_name": "DONE", "color": "#22d160" }
        ])),
        ("GET", ["data", "projects", "p1", "episodes"]) => ok(json!(e.episodes)),
        ("GET", ["data", "projects", "p1", "sequences"]) => ok(json!(e.sequences)),
        ("GET", ["data", "projects", "p1", "shots"]) => ok(json!(e.shots)),
        ("POST", ["data", "projects", "p1", "episodes"]) => {
            let id = nouvel_id(&mut e, "ep");
            let v = json!({ "id": id, "name": json_corps()["name"] });
            e.episodes.push(v.clone());
            ok(v)
        }
        ("POST", ["data", "projects", "p1", "sequences"]) => {
            let id = nouvel_id(&mut e, "seq");
            let c = json_corps();
            let v = json!({ "id": id, "name": c["name"], "parent_id": c["episode_id"] });
            e.sequences.push(v.clone());
            ok(v)
        }
        ("POST", ["data", "projects", "p1", "shots"]) => {
            let id = nouvel_id(&mut e, "shot");
            let c = json_corps();
            if !e.sequences.iter().any(|s| s["id"] == c["sequence_id"]) {
                return (400, json!({ "message": "séquence inconnue" }).to_string());
            }
            let v = json!({ "id": id, "name": c["name"], "parent_id": c["sequence_id"], "nb_frames": c["nb_frames"], "description": c["description"], "data": c["data"] });
            e.shots.push(v.clone());
            ok(v)
        }
        ("PUT", ["data", "shots", id]) => {
            let c = json_corps();
            let Some(s) = e.shots.iter_mut().find(|s| s["id"] == *id) else { return (404, "{}".into()) };
            for k in ["nb_frames", "description", "data"] {
                s[k] = c[k].clone();
            }
            ok(s.clone())
        }
        ("GET", ["data", "shots", id, "tasks"]) => ok(json!(e.tasks.iter().filter(|t| t["entity_id"] == *id).cloned().collect::<Vec<_>>())),
        ("POST", ["actions", "projects", "p1", "task-types", tt, "shots", "create-tasks"]) => {
            let ids = json_corps();
            let mut out = Vec::new();
            for shot in ids.as_array().cloned().unwrap_or_default() {
                let id = nouvel_id(&mut e, "task");
                let t = json!({ "id": id, "entity_id": shot, "task_type_id": tt, "task_status_id": "st-todo" });
                e.tasks.push(t.clone());
                out.push(t);
            }
            ok(json!(out))
        }
        ("GET", ["data", "tasks", id, "comments"]) => ok(json!(e.comments.iter().filter(|c| c["object_id"] == *id).cloned().collect::<Vec<_>>())),
        ("POST", ["actions", "tasks", id, "comment"]) => {
            let c = json_corps();
            if !e.tasks.iter().any(|t| t["id"] == *id) {
                return (404, "{}".into());
            }
            let cid = nouvel_id(&mut e, "com");
            let v = json!({ "id": cid, "object_id": id, "text": c["comment"], "task_status_id": c["task_status_id"], "created_at": "2026-10-30T10:00:00", "person": { "full_name": "SHOT Bot" }, "replies": [] });
            e.comments.push(v.clone());
            if let Some(t) = e.tasks.iter_mut().find(|t| t["id"] == *id) {
                t["task_status_id"] = c["task_status_id"].clone();
            }
            ok(v)
        }
        ("POST", ["actions", "tasks", _, "comments", com, "add-preview"]) => {
            let pid = nouvel_id(&mut e, "prev");
            let v = json!({ "id": pid, "comment_id": com });
            e.previews.push(v.clone());
            ok(v)
        }
        ("POST", ["pictures", "preview-files", pid]) => {
            if e.echec_upload > 0 {
                e.echec_upload -= 1;
                return (500, json!({ "message": "panne du serveur" }).to_string());
            }
            let multipart = corps.windows(9).any(|w| w == b"filename=");
            e.uploads.push((pid.to_string(), corps.len(), multipart));
            ok(json!({ "id": pid }))
        }
        _ => (404, json!({ "message": format!("route inconnue {methode} {p}") }).to_string()),
    }
}

// ----------------------------------------------------------------------------------------------------- utilitaires

fn reglages() -> Reglages {
    Reglages {
        actif: true,
        serveur: "http://127.0.0.1:1".into(),
        email: "a@b.c".into(),
        projet_id: "p1".into(),
        projet_nom: "Mon film".into(),
        tache_id: "tt-sb".into(),
        tache_nom: "Storyboard".into(),
        statut_a_faire: "st-todo".into(),
        statut_discussion: "st-wip".into(),
        statut_fait: "st-done".into(),
        statut_refuse: "st-retake".into(),
        statut_publication: "st-wfa".into(),
        ..Default::default()
    }
}

fn client<'a>(f: &Faux, annule: &'a AtomicBool) -> Client<'a> {
    Client { serveur: f.url.clone(), jeton: Some("tok-1".into()), annule }
}

fn film_kitsu() -> Document {
    let mut doc = film(24);
    doc.properties.episode = "2".into();
    doc.panels[0].action = "Léa entre.".into();
    doc
}

fn empreintes(doc: &Document, valeur: u64) -> HashMap<u64, u64> {
    doc.panels.iter().map(|p| (p.id, valeur)).collect()
}

fn cibles_du(doc: &Document, emp: &HashMap<u64, u64>) -> Vec<kitsu::CiblePlan> {
    let tous: Vec<usize> = (0..doc.panels.len()).collect();
    kitsu::cibles(doc, &tous, 1001, emp)
}

fn png_bidon(dir: &Path, nom: &str) -> PathBuf {
    let p = dir.join(nom);
    let mut o = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    o.extend(std::iter::repeat(7u8).take(300));
    std::fs::write(&p, o).unwrap();
    p
}

fn fichiers_pour(doc: &Document, dir: &Path) -> Fichiers {
    let mut f = Fichiers::default();
    for p in &doc.panels {
        f.vignettes.insert(p.id, png_bidon(dir, &format!("v{}.png", p.id)));
    }
    f
}

/// Calcule puis exécute une publication, comme la fenêtre : lecture de Kitsu, plan, exécution.
fn publier(f: &Faux, doc: &Document, emp: &HashMap<u64, u64>, dir: &Path, rg: &Reglages) -> Result<(kitsu::Plan, kitsu::Rapport), Erreur> {
    let annule = AtomicBool::new(false);
    let c = client(f, &annule);
    let distant = c.distant(&rg.projet_id)?;
    let mut ids = Ids::lire(dir).map_err(Erreur::Reglage)?.unwrap_or_default().pour(&f.url, &rg.projet_id);
    let plan = kitsu::calculer(cibles_du(doc, emp), kitsu::nom_episode(doc), &ids, &distant, true);
    let fichiers = fichiers_pour(doc, dir);
    let prog = Progres::default();
    let r = kitsu::executer(&c, rg, &plan, &fichiers, &mut ids, Some(dir), &prog).inspect_err(|e| eprintln!("échec {e:?} ; journal : {:?}", f.etat().journal))?;
    Ok((plan, r))
}

// ----------------------------------------------------------------------------------------------------------- essais

#[test]
fn the_server_address_is_checked_and_the_password_never_goes_over_plain_http() {
    assert_eq!(kitsu::serveur_valide(" https://kitsu.exemple.com/ ").unwrap(), "https://kitsu.exemple.com");
    assert_eq!(kitsu::serveur_valide("https://kitsu.exemple.com/api").unwrap(), "https://kitsu.exemple.com");
    assert_eq!(kitsu::serveur_valide("http://127.0.0.1:8080").unwrap(), "http://127.0.0.1:8080");
    assert_eq!(kitsu::serveur_valide("http://localhost").unwrap(), "http://localhost");
    for mauvais in ["", "kitsu.exemple.com", "ftp://x.y", "http://kitsu.exemple.com", "http://192.168.1.5", "https://", "https://a b.com", "https://user:pw@h.com", "https://h.com/?x=1", "https://h\".com"] {
        assert!(kitsu::serveur_valide(mauvais).is_err(), "« {mauvais} » doit être refusée");
    }
}

#[test]
fn the_token_is_encrypted_by_dpapi_and_reread_only_for_the_same_server() {
    let dir = dossier_neuf("jeton");
    crate::donnees::bac_a_sable(&dir);
    kitsu::jeton::oublier();
    assert_eq!(kitsu::jeton::relire("https://a.b"), None);
    kitsu::jeton::garder("https://a.b", "eyJ.secret.token").unwrap();
    let brut = std::fs::read(kitsu::jeton::fichier().unwrap()).unwrap();
    assert!(!String::from_utf8_lossy(&brut).contains("secret"), "le fichier ne contient pas le jeton en clair");
    assert!(!brut.windows(6).any(|w| w == b"secret"), "ni en octets");
    assert_eq!(kitsu::jeton::relire("https://a.b").as_deref(), Some("eyJ.secret.token"));
    assert_eq!(kitsu::jeton::relire("https://autre.b"), None, "un jeton ne sert qu'à son serveur");
    kitsu::jeton::oublier();
    assert_eq!(kitsu::jeton::relire("https://a.b"), None);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn login_and_lists_work_and_a_wrong_password_is_refused() {
    let f = Faux::demarrer();
    let annule = AtomicBool::new(false);
    let jeton = Client::connexion(&f.url, " a@b.c ", "pw", &annule).unwrap();
    assert_eq!(jeton, "tok-1");
    assert_eq!(Client::connexion(&f.url, "a@b.c", "faux", &annule), Err(Erreur::NonAutorise));
    assert!(matches!(Client::connexion(&f.url, "", "", &annule), Err(Erreur::Reglage(_))));
    // Le mot de passe a voyagé dans le corps de la requête (entrée standard de curl), jamais dans une adresse.
    assert!(f.etat().journal.iter().all(|j| !j.contains("pw")));
    let c = client(&f, &annule);
    c.verifier_jeton().unwrap();
    let projets = c.projets().unwrap();
    assert_eq!(projets.iter().map(|p| p.nom.as_str()).collect::<Vec<_>>(), ["Mon film", "Autre"]);
    let types = c.types_de_tache().unwrap();
    assert_eq!(types.len(), 3);
    let statuts = c.statuts().unwrap();
    assert_eq!(statuts.iter().map(|s| s.court.as_str()).collect::<Vec<_>>(), ["todo", "wip", "wfa", "rtk", "done"]);
    // Les statuts usuels sont proposés d'office.
    let mut r = Reglages::default();
    r.remplir_defauts(&statuts);
    assert_eq!((r.statut_a_faire.as_str(), r.statut_discussion.as_str(), r.statut_fait.as_str(), r.statut_publication.as_str()), ("st-todo", "st-wip", "st-done", "st-wfa"));
    assert_eq!(r.statut_shot("st-done"), Some(Status::Done));
    assert_eq!(r.statut_shot("st-inconnu"), None);
}

#[test]
fn an_expired_token_is_a_401_the_window_can_explain() {
    let f = Faux::demarrer();
    f.etat().jeton = "autre".into();
    let annule = AtomicBool::new(false);
    let c = client(&f, &annule);
    assert_eq!(c.projets(), Err(Erreur::NonAutorise));
    assert_eq!(c.distant("p1").map(|_| ()), Err(Erreur::NonAutorise));
    assert!(Erreur::NonAutorise.to_string().contains("connecte-toi"));
}

#[test]
fn an_unreachable_server_is_a_clear_network_error_not_a_hang() {
    // Un port où personne n'écoute.
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let annule = AtomicBool::new(false);
    let t0 = Instant::now();
    let r = Client::connexion(&format!("http://127.0.0.1:{port}"), "a@b.c", "pw", &annule);
    assert!(matches!(&r, Err(Erreur::Reseau(m)) if m.contains("refusée") || m.contains("curl")), "{r:?}");
    assert!(t0.elapsed() < Duration::from_secs(12));
}

#[test]
fn a_first_publication_creates_the_episode_sequences_and_shots_then_previews_each_shot() {
    let f = Faux::demarrer();
    let dir = dossier_neuf("premiere");
    let doc = film_kitsu();
    let emp = empreintes(&doc, 11);
    let (plan, r) = publier(&f, &doc, &emp, &dir, &reglages()).unwrap();
    let c = plan.compte();
    assert_eq!((c.episodes_a_creer, c.sequences_a_creer, c.a_creer, c.a_mettre_a_jour, c.apercus), (1, 2, 4, 0, 4));
    assert_eq!(plan.phrase(), "1 épisode à créer, 2 séquences à créer, 4 plans à créer, 0 à mettre à jour, 4 prévisualisations");
    assert_eq!((r.episodes_crees, r.sequences_creees, r.plans_crees, r.plans_mis_a_jour, r.apercus), (1, 2, 4, 0, 4));
    let e = f.etat();
    assert_eq!(e.episodes.len(), 1);
    assert_eq!(e.episodes[0]["name"], "EP02");
    assert_eq!(e.sequences.len(), 2);
    assert!(e.sequences.iter().all(|s| s["parent_id"] == e.episodes[0]["id"]), "les séquences sont dans l'épisode");
    assert_eq!(e.shots.len(), 4);
    // Les noms suivent le modèle de noms, les images d'entrée et de sortie sont celles de la liste des plans (1001, sortie incluse).
    let noms: Vec<String> = (0..4).map(|i| doc.nom_de_plan(i)).collect();
    assert_eq!(e.shots.iter().map(|s| s["name"].as_str().unwrap().to_string()).collect::<Vec<_>>(), noms);
    assert_eq!(e.shots[0]["nb_frames"], 24);
    assert_eq!(e.shots[0]["data"], json!({ "frame_in": 1001, "frame_out": 1024, "fps": 24 }));
    assert_eq!(e.shots[1]["data"]["frame_in"], 1025);
    assert_eq!(e.shots[0]["description"], "Léa entre.");
    // Une tâche storyboard créée par plan, un commentaire avec la marque de SHOT, une prévisualisation envoyée en multipart.
    assert_eq!(e.tasks.len(), 4);
    assert!(e.tasks.iter().all(|t| t["task_type_id"] == "tt-sb" && t["task_status_id"] == "st-wfa"), "la tâche passe au statut de publication");
    assert_eq!(e.comments.len(), 4);
    assert!(e.comments.iter().all(|c| c["text"].as_str().unwrap().starts_with(kitsu::MARQUE)));
    assert_eq!(e.previews.len(), 4);
    assert_eq!(e.uploads.len(), 4);
    assert!(e.uploads.iter().all(|(_, n, multipart)| *multipart && *n > 300));
    drop(e);
    // Les identifiants sont à côté du projet, par identifiant stable de plan.
    let ids = Ids::lire(&dir).unwrap().unwrap();
    assert_eq!(ids.serveur, f.url);
    assert_eq!(ids.plans.len(), 4);
    assert!(doc.panels.iter().all(|p| ids.plans.contains_key(&p.id.to_string())));
    assert_eq!(ids.sequences.len(), 2);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn publishing_again_changes_nothing_and_only_what_changed_is_sent() {
    let f = Faux::demarrer();
    let dir = dossier_neuf("idempotent");
    let mut doc = film_kitsu();
    let emp = empreintes(&doc, 11);
    publier(&f, &doc, &emp, &dir, &reglages()).unwrap();
    let apres_un = f.nb_ecritures();
    // Deuxième fois, rien n'a bougé : rien à faire, aucune écriture.
    let (plan, r) = publier(&f, &doc, &emp, &dir, &reglages()).unwrap();
    assert!(plan.est_vide(), "{}", plan.phrase());
    assert_eq!(plan.phrase(), "0 plan à créer, 0 à mettre à jour, 0 prévisualisation");
    assert_eq!((r.plans_crees, r.plans_mis_a_jour, r.apercus, r.inchanges), (0, 0, 0, 4));
    assert_eq!(f.nb_ecritures(), apres_un, "aucune écriture de plus");
    // Un plan change (texte et dessin) : une mise à jour, une prévisualisation, pas un plan de plus.
    doc.panels[1].action = "Léa sort.".into();
    let mut emp2 = emp.clone();
    emp2.insert(doc.panels[1].id, 99);
    let (plan, r) = publier(&f, &doc, &emp2, &dir, &reglages()).unwrap();
    let c = plan.compte();
    assert_eq!((c.a_creer, c.a_mettre_a_jour, c.apercus, c.inchanges), (0, 1, 1, 3));
    assert_eq!((r.plans_mis_a_jour, r.apercus), (1, 1));
    {
        let e = f.etat();
        assert_eq!(e.shots.len(), 4);
        assert_eq!(e.shots[1]["description"], "Léa sort.");
        assert_eq!(e.comments.len(), 5, "un seul commentaire de plus");
        assert_eq!(e.uploads.len(), 5);
    }
    // Seule la durée change : mise à jour des images, pas de prévisualisation (le dessin est le même).
    doc.panels[2].set_length(40);
    let (plan, _) = publier(&f, &doc, &emp2, &dir, &reglages()).unwrap();
    let c = plan.compte();
    assert_eq!((c.a_mettre_a_jour, c.apercus), (2, 0), "le plan allongé et le suivant, dont les images décalent");
    {
        let e = f.etat();
        assert_eq!(e.shots[2]["nb_frames"], 40);
        assert_eq!(e.shots[3]["data"]["frame_in"], 1001 + 24 + 12 + 40);
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_lost_identity_file_matches_by_name_and_never_duplicates() {
    let f = Faux::demarrer();
    let dir = dossier_neuf("perdu");
    let doc = film_kitsu();
    let emp = empreintes(&doc, 11);
    publier(&f, &doc, &emp, &dir, &reglages()).unwrap();
    std::fs::remove_file(dir.join(kitsu::FICHIER)).unwrap();
    let (plan, r) = publier(&f, &doc, &emp, &dir, &reglages()).unwrap();
    let c = plan.compte();
    assert_eq!((c.episodes_a_creer, c.sequences_a_creer, c.a_creer), (0, 0, 0), "retrouvés par leur nom : {}", plan.phrase());
    assert_eq!(r.plans_crees, 0);
    let e = f.etat();
    assert_eq!((e.episodes.len(), e.sequences.len(), e.shots.len(), e.tasks.len()), (1, 2, 4, 4));
    drop(e);
    // Les identifiants d'un autre serveur ou d'un autre projet ne valent rien ici.
    let ids = Ids::lire(&dir).unwrap().unwrap();
    assert!(ids.clone().pour(&f.url, "p1") == ids);
    assert!(ids.clone().pour(&f.url, "p2").plans.is_empty());
    assert!(ids.pour("https://autre", "p1").plans.is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_failed_upload_is_retried_without_a_second_comment() {
    let f = Faux::demarrer();
    let dir = dossier_neuf("reprise");
    let doc = film_kitsu();
    let emp = empreintes(&doc, 11);
    f.etat().echec_upload = 1;
    let r = publier(&f, &doc, &emp, &dir, &reglages());
    assert!(matches!(r, Err(Erreur::Http(500, ref m)) if m.contains("panne")), "{r:?}");
    {
        let e = f.etat();
        assert_eq!((e.shots.len(), e.comments.len(), e.uploads.len()), (4, 1, 0), "les plans sont créés, un commentaire attend son fichier");
    }
    let ids = Ids::lire(&dir).unwrap().unwrap();
    assert!(ids.plans.values().any(|p| p.en_cours.is_some()), "la publication à moitié faite est notée");
    // On recommence : le commentaire en attente est repris, jamais republié.
    let (_, r) = publier(&f, &doc, &emp, &dir, &reglages()).unwrap();
    assert_eq!(r.apercus, 4);
    let e = f.etat();
    assert_eq!((e.shots.len(), e.comments.len(), e.uploads.len()), (4, 4, 4));
    assert_eq!(e.previews.len(), 4, "une seule prévisualisation par plan");
    drop(e);
    let ids = Ids::lire(&dir).unwrap().unwrap();
    assert!(ids.plans.values().all(|p| p.en_cours.is_none() && !p.apercu.is_empty()));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn stopping_kills_the_request_at_once_and_what_was_done_stays_known() {
    let f = Faux::demarrer();
    f.etat().retard_ms = 3000;
    let annule = Arc::new(AtomicBool::new(false));
    let (a2, url) = (annule.clone(), f.url.clone());
    let t0 = Instant::now();
    let h = std::thread::spawn(move || Client { serveur: url, jeton: Some("tok-1".into()), annule: &a2 }.distant("p1"));
    std::thread::sleep(Duration::from_millis(300));
    annule.store(true, Ordering::Relaxed);
    let r = h.join().unwrap();
    assert_eq!(r.map(|_| ()), Err(Erreur::Annule));
    assert!(t0.elapsed() < Duration::from_millis(2500), "curl est tué, on n'attend pas la réponse ({:?})", t0.elapsed());

    // Arrêt entre deux étapes d'une publication : ce qui est fait est dans kitsu.json et la fois suivante ne double rien.
    let f = Faux::demarrer();
    let dir = dossier_neuf("arret");
    let doc = film_kitsu();
    let emp = empreintes(&doc, 11);
    let annule = AtomicBool::new(false);
    let c = client(&f, &annule);
    let distant = c.distant("p1").unwrap();
    let mut ids = Ids::default().pour(&f.url, "p1");
    let plan = kitsu::calculer(cibles_du(&doc, &emp), kitsu::nom_episode(&doc), &ids, &distant, true);
    let prog = Progres::default();
    annule.store(true, Ordering::Relaxed);
    let r = kitsu::executer(&c, &reglages(), &plan, &fichiers_pour(&doc, &dir), &mut ids, Some(&dir), &prog);
    assert_eq!(r, Err(Erreur::Annule));
    assert_eq!(f.nb_ecritures(), 0, "arrêté avant d'écrire");
    annule.store(false, Ordering::Relaxed);
    let (_, r) = publier(&f, &doc, &emp, &dir, &reglages()).unwrap();
    assert_eq!(r.plans_crees, 4);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_dry_run_counts_what_would_be_done_without_writing() {
    let f = Faux::demarrer();
    let doc = film_kitsu();
    let emp = empreintes(&doc, 5);
    let annule = AtomicBool::new(false);
    let c = client(&f, &annule);
    let distant = c.distant("p1").unwrap();
    let plan = kitsu::calculer(cibles_du(&doc, &emp), kitsu::nom_episode(&doc), &Ids::default(), &distant, true);
    assert_eq!(plan.phrase(), "1 épisode à créer, 2 séquences à créer, 4 plans à créer, 0 à mettre à jour, 4 prévisualisations");
    assert_eq!(plan.apercus_a_faire().len(), 4);
    assert!(plan.plans.iter().all(|p| p.action == Action::Creer));
    assert_eq!(f.nb_ecritures(), 0, "l'aperçu ne fait que lire");
    // Sans prévisualisation demandée : aucune.
    let sans = kitsu::calculer(cibles_du(&doc, &emp), None, &Ids::default(), &distant, false);
    assert_eq!(sans.compte().apercus, 0);
    assert_eq!(sans.compte().episodes_a_creer, 0, "pas d'épisode : les séquences sont au niveau du projet");
}

#[test]
fn comments_come_back_as_notes_without_duplicates_and_never_override_a_decided_status() {
    let f = Faux::demarrer();
    let dir = dossier_neuf("retours");
    let mut doc = film_kitsu();
    let emp = empreintes(&doc, 11);
    publier(&f, &doc, &emp, &dir, &reglages()).unwrap();
    // La production commente la tâche du plan 1 : une demande de reprise avec une réponse, une validation ; le commentaire de SHOT est ignoré.
    let tache = f.etat().tasks[1]["id"].as_str().unwrap().to_string();
    {
        let mut e = f.etat();
        e.comments.push(json!({ "id": "c-ext-1", "object_id": tache, "text": "Le plan démarre trop tard.", "task_status_id": "st-retake", "created_at": "2026-10-30T14:05:09.123456", "person": { "full_name": "Claire Martin" }, "replies": [
            { "id": "r-1", "text": "Je regarde.", "date": "2026-10-30T15:00:00", "person": { "first_name": "Zoé", "last_name": "Roy" } } ] }));
        e.comments.push(json!({ "id": "c-ext-2", "object_id": tache, "text": "Ok pour le raccord.", "task_status_id": "st-done", "created_at": "2026-10-31T09:00:00", "person": { "full_name": "Claire Martin" }, "replies": [] }));
        e.tasks[1]["task_status_id"] = json!("st-done");
    }
    let rg = reglages();
    let annule = AtomicBool::new(false);
    let c = client(&f, &annule);
    let ids = Ids::lire(&dir).unwrap().unwrap();
    let panels: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    let retours = kitsu::lire_retours(&c, &rg, &ids, &panels, &Progres::default()).unwrap();
    assert_eq!(retours.len(), 4);
    let r1 = retours.iter().find(|r| r.panel == doc.panels[1].id).unwrap();
    assert_eq!(r1.statut_nom, "Done");
    assert_eq!(r1.notes.len(), 2, "le commentaire publié par SHOT n'est pas relu");
    assert_eq!(r1.notes[0].auteur, "Claire Martin");
    assert_eq!(r1.notes[0].reponses[0].auteur, "Zoé Roy");
    assert!(retours.iter().filter(|r| r.panel != doc.panels[1].id).all(|r| r.notes.is_empty() && r.statut_nom == "Waiting For Approval"));

    // Import dans le fil de décision.
    let avant = doc.comments.len();
    let prev = kitsu::previsualiser(&doc, &retours);
    assert_eq!((prev.notes, prev.reponses), (2, 1));
    let s = kitsu::appliquer(&mut doc, &retours, &rg, 1_800_000_000);
    assert_eq!((s.notes, s.reponses), (2, 1));
    assert_eq!(doc.comments.len(), avant + 2);
    let n1 = doc.comments.iter().find(|c| c.follow.external.as_deref() == Some("c-ext-1")).unwrap();
    assert_eq!((n1.follow.source, n1.panel, n1.author.as_str()), (Source::Kitsu, doc.panels[1].id, "Claire Martin"));
    assert_eq!(n1.text, "Le plan démarre trop tard.");
    assert_eq!(n1.status(), Status::Refused, "Retake dans Kitsu : refusé, avec la raison");
    assert!(n1.follow.reason.contains("Kitsu"));
    assert_eq!(n1.created, kitsu::lire_date_iso("2026-10-30T14:05:09"));
    assert_eq!(n1.follow.replies.len(), 1);
    assert_eq!((n1.follow.replies[0].author.as_str(), n1.follow.replies[0].external.as_deref(), n1.follow.replies[0].source), ("Zoé Roy", Some("r-1"), Source::Kitsu));
    let n2 = doc.comments.iter().find(|c| c.follow.external.as_deref() == Some("c-ext-2")).unwrap();
    assert_eq!(n2.status(), Status::Done);

    // Une décision prise dans SHOT n'est jamais écrasée, et relire ne double rien.
    let id1 = n1.id;
    doc.comment_mut(id1).unwrap().appliquer_statut(Status::Discussion, "", 5);
    assert_eq!(kitsu::previsualiser(&doc, &retours).nouveau(), 0);
    let s = kitsu::appliquer(&mut doc, &retours, &rg, 1_800_000_100);
    assert_eq!(s.nouveau(), 0);
    assert_eq!(doc.comments.len(), avant + 2);
    assert_eq!(doc.comment(id1).unwrap().status(), Status::Discussion, "le statut décidé dans SHOT reste");
    // Une réponse arrivée depuis rejoint la note existante, une seule fois.
    f.etat().comments.iter_mut().find(|c| c["id"] == "c-ext-1").unwrap()["replies"] = json!([{ "id": "r-1", "text": "Je regarde.", "date": "2026-10-30T15:00:00" }, { "id": "r-2", "text": "Corrigé.", "date": "2026-11-01T08:00:00" }]);
    let mut retours = kitsu::lire_retours(&c, &rg, &ids, &panels, &Progres::default()).unwrap();
    for _ in 0..2 {
        kitsu::appliquer(&mut doc, &retours, &rg, 1_800_000_200);
    }
    assert_eq!(doc.comment(id1).unwrap().follow.replies.len(), 2);
    // Le statut de la tâche est gardé pour la fiche du plan.
    let mut ids2 = ids.clone();
    kitsu::noter_statuts(&mut ids2, &retours);
    assert_eq!(ids2.plans[&doc.panels[1].id.to_string()].statut, "Done");
    assert_eq!(ids2.plans[&doc.panels[1].id.to_string()].statut_couleur, "#22d160");
    retours.clear();
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn reading_comments_on_a_project_never_published_reads_nothing() {
    let f = Faux::demarrer();
    let annule = AtomicBool::new(false);
    let c = client(&f, &annule);
    let doc = film_kitsu();
    let panels: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    let r = kitsu::lire_retours(&c, &reglages(), &Ids::default(), &panels, &Progres::default()).unwrap();
    assert!(r.is_empty());
    assert!(matches!(kitsu::lire_retours(&c, &Reglages::default(), &Ids::default(), &panels, &Progres::default()), Err(Erreur::Reglage(_))));
}

#[test]
fn iso_dates_are_read() {
    assert_eq!(kitsu::lire_date_iso("1970-01-02T00:00:01"), 86_401);
    assert_eq!(kitsu::lire_date_iso("2026-10-30T14:05:09.123456"), kitsu::lire_date_iso("2026-10-30 14:05:09"));
    assert_eq!(kitsu::lire_date_iso("2026-10-30T14:05:09Z"), kitsu::lire_date_iso("2026-10-30T14:05:09"));
    assert_eq!(kitsu::lire_date_iso("nimporte quoi"), 0);
    assert_eq!(kitsu::lire_date_iso(""), 0);
}

#[test]
fn the_identity_file_is_absent_until_used_and_a_foreign_file_is_not_overwritten() {
    let dir = dossier_neuf("ids");
    assert!(Ids::lire(&dir).unwrap().is_none());
    assert!(!dir.join(kitsu::FICHIER).exists());
    std::fs::write(dir.join(kitsu::FICHIER), "{ pas du json").unwrap();
    assert!(Ids::lire(&dir).is_err(), "un fichier abîmé est une erreur (on ne l'écrase pas en silence)");
    std::fs::write(dir.join(kitsu::FICHIER), r#"{"format":"autre"}"#).unwrap();
    assert!(Ids::lire(&dir).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn settings_are_off_by_default_and_hold_no_secret() {
    let s = crate::settings::Settings::default();
    assert!(!s.kitsu.actif && !s.kitsu.rester_connecte && s.kitsu.serveur.is_empty());
    let mut s2 = crate::settings::Settings::default();
    s2.kitsu = reglages();
    let t = toml::to_string(&s2).unwrap();
    assert!(t.contains("[kitsu]"));
    assert!(!t.to_lowercase().contains("password") && !t.contains("motdepasse") && !t.contains("jeton"), "ni mot de passe ni jeton dans reglages.toml");
    let relu: crate::settings::Settings = toml::from_str(&t).unwrap();
    assert_eq!(relu.kitsu, s2.kitsu);
}

// ------------------------------------------------------------------------------- de bout en bout, par la fenêtre

#[test]
fn the_window_publishes_with_real_thumbnails_then_reads_back_in_one_undo_step() {
    let f = Faux::demarrer();
    let dir = dossier_neuf("fenetre");
    crate::donnees::bac_a_sable(&dir);
    let ctx = eframe::egui::Context::default();
    let mut app = crate::app::App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = film_kitsu();
    app.cur_panel = 0;
    // Un projet enregistré (les identifiants se gardent dans son dossier).
    let manifeste = dir.join("Film").join("Film.shot");
    std::fs::create_dir_all(manifeste.parent().unwrap()).unwrap();
    app.path = Some(manifeste.clone());
    app.settings.kitsu = Reglages { serveur: f.url.clone(), ..reglages() };

    let mut t = 0.0;
    let mut image = |app: &mut crate::app::App| {
        t += 0.2;
        let input = eframe::egui::RawInput { screen_rect: Some(eframe::egui::Rect::from_min_size(eframe::egui::pos2(0.0, 0.0), eframe::egui::vec2(1280.0, 900.0))), time: Some(t), ..Default::default() };
        let _ = ctx.run(input, |ctx| {
            app.envois_tick(ctx);
            app.kitsu_window(ctx);
        });
        std::thread::sleep(Duration::from_millis(15));
    };
    app.run_action("fichier.kitsu_publier");
    assert!(app.kitsu.ouvert);
    // Les empreintes se calculent (fenêtre ouverte), pas de réseau avant le clic.
    for _ in crate::perf_tests::attente() {
        image(&mut app);
        if app.envois_empreintes_a_jour() {
            break;
        }
    }
    assert!(app.envois_empreintes_a_jour(), "les empreintes sont prêtes");
    assert_eq!(f.etat().journal.len(), 0, "rien n'est parti sans clic");

    // Connexion (jeton posé comme après « Tester la connexion »), puis aperçu.
    app.kitsu.poser_session_pour_essai(&f.url, "tok-1");
    let mut k = std::mem::take(&mut app.kitsu);
    app.kitsu_apercu(&mut k, &ctx);
    app.kitsu = k;
    attendre(&mut app, &mut image);
    let phrase = app.kitsu.message_pour_essai();
    assert_eq!(phrase, "1 épisode à créer, 2 séquences à créer, 4 plans à créer, 0 à mettre à jour, 4 prévisualisations");
    assert_eq!(f.nb_ecritures(), 0);

    // Publier : les vraies vignettes (tâche d'export habituelle) partent en prévisualisation.
    let mut k = std::mem::take(&mut app.kitsu);
    app.kitsu_publier(&mut k, &ctx);
    app.kitsu = k;
    attendre(&mut app, &mut image);
    assert!(app.kitsu.message_pour_essai().starts_with("Kitsu à jour : 4 plan(s) créé(s)"), "{}", app.kitsu.message_pour_essai());
    {
        let e = f.etat();
        assert_eq!((e.shots.len(), e.comments.len(), e.uploads.len()), (4, 4, 4));
        // Un vrai PNG : plus qu'une poignée d'octets.
        assert!(e.uploads.iter().all(|(_, n, multipart)| *multipart && *n > 100));
    }
    assert!(dir.join("Film").join(kitsu::FICHIER).exists());
    // Le dossier temporaire des prévisualisations est effacé.
    let temp = crate::donnees::local().unwrap().join("Kitsu");
    assert!(std::fs::read_dir(&temp).map(|mut d| d.next().is_none()).unwrap_or(true), "dossier temporaire effacé");

    // Un deuxième aperçu : tout est à jour.
    let mut k = std::mem::take(&mut app.kitsu);
    app.kitsu_apercu(&mut k, &ctx);
    app.kitsu = k;
    attendre(&mut app, &mut image);
    assert_eq!(app.kitsu.message_pour_essai(), "0 plan à créer, 0 à mettre à jour, 0 prévisualisation");

    // La production répond ; la lecture des retours pose les notes, en une seule étape d'annulation.
    let tache = f.etat().tasks[0]["id"].as_str().unwrap().to_string();
    f.etat().comments.push(json!({ "id": "c-ext-9", "object_id": tache, "text": "Plus de nuit ici.", "task_status_id": "st-todo", "created_at": "2026-10-30T14:05:09", "person": { "full_name": "Claire" }, "replies": [] }));
    let avant = app.doc.comments.len();
    let pas_avant = app.history.serial();
    let mut k = std::mem::take(&mut app.kitsu);
    app.kitsu_retours(&mut k, &ctx);
    app.kitsu = k;
    attendre(&mut app, &mut image);
    assert_eq!(app.doc.comments.len(), avant + 1);
    assert!(app.history.serial() > pas_avant);
    let note = app.doc.comments.last().unwrap();
    assert_eq!((note.follow.source, note.follow.external.as_deref(), note.text.as_str()), (Source::Kitsu, Some("c-ext-9"), "Plus de nuit ici."));
    assert_eq!(app.kitsu.marques.get(&app.doc.panels[0].id).map(|m| m.0.as_str()), Some("Waiting For Approval"), "le statut de la tâche est lu pour la fiche du plan");
    // Relire : pas de doublon, pas d'étape d'annulation de plus.
    let serial = app.history.serial();
    let mut k = std::mem::take(&mut app.kitsu);
    app.kitsu_retours(&mut k, &ctx);
    app.kitsu = k;
    attendre(&mut app, &mut image);
    assert_eq!(app.doc.comments.len(), avant + 1);
    assert_eq!(app.history.serial(), serial);
    // Ctrl+Z défait l'import d'un coup.
    app.undo();
    assert_eq!(app.doc.comments.len(), avant, "une étape d'annulation défait toutes les notes de Kitsu");
    std::fs::remove_dir_all(&dir).ok();
}

fn attendre(app: &mut crate::app::App, image: &mut impl FnMut(&mut crate::app::App)) {
    let t0 = Instant::now();
    loop {
        image(app);
        if !app.kitsu.busy() {
            break;
        }
        assert!(t0.elapsed() < Duration::from_secs(60), "le travail ne finit pas : {}", app.kitsu.message_pour_essai());
    }
}

#[test]
fn nothing_goes_online_when_the_bridge_is_off_and_commands_open_the_window() {
    let dir = dossier_neuf("eteint");
    crate::donnees::bac_a_sable(&dir);
    let ctx = eframe::egui::Context::default();
    let mut app = crate::app::App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = film_kitsu();
    for action in ["fichier.kitsu_publier", "fichier.kitsu_retours"] {
        app.kitsu.ouvert = false;
        app.run_action(action);
        assert!(app.kitsu.ouvert, "{action}");
        for _ in 0..3 {
            let input = eframe::egui::RawInput { screen_rect: Some(eframe::egui::Rect::from_min_size(eframe::egui::pos2(0.0, 0.0), eframe::egui::vec2(1280.0, 900.0))), time: Some(0.0), ..Default::default() };
            let _ = ctx.run(input, |ctx| app.kitsu_window(ctx));
        }
    }
    // Éteint : « Aperçu » refuse avant tout réseau.
    let mut k = std::mem::take(&mut app.kitsu);
    app.kitsu_apercu(&mut k, &ctx);
    assert!(!k.busy());
    assert!(k.message_pour_essai().contains("Allume"), "{}", k.message_pour_essai());
    app.kitsu = k;
    std::fs::remove_dir_all(&dir).ok();
}
