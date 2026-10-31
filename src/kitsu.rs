//! Le pont Kitsu (pipeline, étape 2) : publier les plans de SHOT dans Kitsu (le logiciel libre de suivi de production de CGWire) et en lire les
//! retours. Tout ce qui parle à Kitsu est ici, **sans interface** ; la fenêtre est `kitsuui.rs`. Détails et liste « À vérifier sur une vraie
//! instance » : `docs/INTERFACE.md` partie 93.
//!
//! # Règles
//!
//! - **Rien ne part sans un clic.** Le pont est éteint d'origine (`Reglages::actif`) ; aucune fonction d'ici n'est appelée au démarrage.
//! - **HTTP** : SHOT n'a pas de client HTTP et n'en ajoute pas (aucune caisse de plus). On lance `curl.exe`, livré avec Windows 10 et 11
//!   (`System32`), qui parle HTTPS avec les certificats du système (Schannel). Choix expliqué dans `docs/INTERFACE.md` §93 : une seule dépendance
//!   système, HTTPS réel, délais, arrêt immédiat (le processus est tué). Le **mot de passe, le jeton et le corps de la requête passent par
//!   l'entrée standard de curl** (`-K -`) : jamais sur la ligne de commande, que les autres processus peuvent lire.
//! - **Pas de redirection suivie** (le jeton ne suit pas un renvoi vers un autre serveur) ; une adresse en `http://` n'est acceptée que pour la
//!   machine elle-même (essais) : le mot de passe ne part jamais en clair sur le réseau.
//! - **Jeton** : le mot de passe ne vit que dans la mémoire de la séance (jamais écrit). Le jeton d'accès n'est gardé sur disque que si l'on
//!   coche « Rester connecté » : chiffré par DPAPI de Windows (`CryptProtectData`, lié au compte Windows), dans les données de l'application,
//!   jamais dans le projet.
//! - **Les adresses de l'API (table `ep`)** viennent de la documentation publique de Kitsu/Zou, de mémoire : elles n'ont pas pu être vérifiées
//!   en ligne. Chacune est marquée « À CONFIRMER sur une instance » et elles sont **toutes ici**, à un seul endroit.
//! - **Identité en ligne** : `kitsu.json`, à côté du projet (absent tant qu'on n'a rien publié ; le format du projet ne change pas). Il garde
//!   les identifiants Kitsu par identifiant stable de plan et de séquence de SHOT. Publier de nouveau ne double rien : on retrouve d'abord ce qui
//!   existe (par identifiant gardé, sinon par nom), on compare à ce que Kitsu montre, et seul ce qui diffère est envoyé.

use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::commentaires::Comment;
use crate::document::Document;
use crate::fildecision::{Follow, Reply, Source, Status};

/// Nom du fichier d'identités, dans le dossier du projet.
pub const FICHIER: &str = "kitsu.json";
const FORMAT: &str = "shot-kitsu";
const VERSION: u32 = 1;
/// Début du texte de chaque commentaire que SHOT publie : à la lecture en retour, ils sont reconnus et laissés de côté.
pub const MARQUE: &str = "[SHOT]";
/// Couleur d'une note venue de Kitsu dans le panneau Commentaires.
const COULEUR_NOTE: [u8; 3] = [120, 160, 230];

// ------------------------------------------------------------------------------------------------ les adresses de l'API

/// Toutes les adresses de l'API de Kitsu (Zou), après `<serveur>/api`. **À CONFIRMER sur une instance** : écrites de mémoire d'après la
/// documentation publique ; si l'une diffère, c'est ici qu'on la corrige.
pub mod ep {
    /// POST `{ email, password }` → `access_token`. À CONFIRMER sur une instance.
    pub fn connexion() -> String {
        "/auth/login".into()
    }
    /// GET → `{ authenticated: true }` (le jeton est-il encore bon ?). À CONFIRMER sur une instance.
    pub fn authentifie() -> String {
        "/auth/authenticated".into()
    }
    /// GET : les productions. À CONFIRMER sur une instance.
    pub fn projets() -> String {
        "/data/projects".into()
    }
    /// GET : les types de tâche (Storyboard, Animation…). À CONFIRMER sur une instance.
    pub fn types_de_tache() -> String {
        "/data/task-types".into()
    }
    /// GET : les statuts de tâche (Todo, WIP, WFA, Retake, Done…). À CONFIRMER sur une instance.
    pub fn statuts() -> String {
        "/data/task-status".into()
    }
    /// GET (liste) et POST `{ name }` (création) des épisodes. À CONFIRMER sur une instance.
    pub fn episodes(projet: &str) -> String {
        format!("/data/projects/{projet}/episodes")
    }
    /// GET (liste) et POST `{ name, episode_id }` (création) des séquences. À CONFIRMER sur une instance.
    pub fn sequences(projet: &str) -> String {
        format!("/data/projects/{projet}/sequences")
    }
    /// GET (liste) et POST `{ name, sequence_id, nb_frames, description, data: { frame_in, frame_out, fps } }` (création) des plans.
    /// À CONFIRMER sur une instance.
    pub fn plans(projet: &str) -> String {
        format!("/data/projects/{projet}/shots")
    }
    /// PUT `{ nb_frames, description, data }` : mise à jour d'un plan. À CONFIRMER sur une instance.
    pub fn plan(id: &str) -> String {
        format!("/data/shots/{id}")
    }
    /// GET : les tâches d'un plan. À CONFIRMER sur une instance.
    pub fn taches_du_plan(id: &str) -> String {
        format!("/data/shots/{id}/tasks")
    }
    /// POST `[id du plan]` : crée la tâche d'un type pour ces plans. À CONFIRMER sur une instance.
    pub fn creer_taches(projet: &str, type_de_tache: &str) -> String {
        format!("/actions/projects/{projet}/task-types/{type_de_tache}/shots/create-tasks")
    }
    /// GET : les commentaires d'une tâche. À CONFIRMER sur une instance.
    pub fn commentaires(tache: &str) -> String {
        format!("/data/tasks/{tache}/comments")
    }
    /// POST `{ task_status_id, comment }` → le commentaire (`id`). À CONFIRMER sur une instance.
    pub fn commenter(tache: &str) -> String {
        format!("/actions/tasks/{tache}/comment")
    }
    /// POST `{}` → la prévisualisation (`id`). À CONFIRMER sur une instance.
    pub fn ajouter_apercu(tache: &str, commentaire: &str) -> String {
        format!("/actions/tasks/{tache}/comments/{commentaire}/add-preview")
    }
    /// POST multipart, champ `file` : le fichier de la prévisualisation. À CONFIRMER sur une instance.
    pub fn televerser_apercu(apercu: &str) -> String {
        format!("/pictures/preview-files/{apercu}")
    }
}

// ------------------------------------------------------------------------------------------------------- les erreurs

#[derive(Clone, Debug, PartialEq)]
pub enum Erreur {
    /// Pas de réponse : réseau, nom du serveur, certificat, délai (le texte dit lequel).
    Reseau(String),
    /// Le serveur a répondu par une erreur.
    Http(u16, String),
    /// 401 : le jeton est refusé ou périmé, ou les identifiants sont faux.
    NonAutorise,
    /// La réponse n'a pas la forme attendue (une adresse de l'API à confirmer ?).
    Format(String),
    /// L'utilisateur a arrêté.
    Annule,
    /// Pas assez d'informations pour commencer (réglage manquant…).
    Reglage(String),
}

impl std::fmt::Display for Erreur {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Erreur::Reseau(m) => write!(f, "Kitsu injoignable : {m}"),
            Erreur::Http(code, m) if m.is_empty() => write!(f, "Kitsu a répondu par une erreur {code}."),
            Erreur::Http(code, m) => write!(f, "Kitsu a répondu par une erreur {code} : {m}"),
            Erreur::NonAutorise => write!(f, "Kitsu refuse la connexion (identifiants faux, ou session expirée) : connecte-toi de nouveau."),
            Erreur::Format(m) => write!(f, "Réponse de Kitsu inattendue : {m} (une adresse de l'API est peut-être à corriger, voir docs/INTERFACE.md partie 93)."),
            Erreur::Annule => write!(f, "Arrêté."),
            Erreur::Reglage(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for Erreur {}

// ------------------------------------------------------------------------------------------------------------- HTTP

/// L'adresse du serveur, ramenée à `https://hote[:port][/chemin]` sans `/` final ni `/api` ; une erreur lisible sinon.
pub fn serveur_valide(texte: &str) -> Result<String, String> {
    let t = texte.trim().trim_end_matches('/');
    let t = t.strip_suffix("/api").unwrap_or(t).trim_end_matches('/');
    if t.is_empty() {
        return Err("Indique l'adresse du serveur Kitsu (par exemple https://kitsu.monstudio.com).".into());
    }
    let (schema, reste) = if let Some(r) = t.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = t.strip_prefix("http://") {
        (false, r)
    } else {
        return Err("L'adresse doit commencer par https:// (ou http:// pour cet ordinateur seulement).".into());
    };
    if reste.is_empty() || reste.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, '"' | '\\' | '?' | '#' | '@')) {
        return Err("Adresse du serveur illisible : https://hôte ou https://hôte:port, sans identifiant ni paramètre.".into());
    }
    if !schema && !hote_local(reste) {
        return Err("Une adresse en http:// enverrait le mot de passe en clair : utilise https:// (http:// n'est accepté que pour cet ordinateur).".into());
    }
    Ok(t.to_string())
}

fn hote_local(reste: &str) -> bool {
    let hote = reste.split('/').next().unwrap_or("");
    let hote = if let Some(h) = hote.strip_prefix('[') { h.split(']').next().unwrap_or("") } else { hote.split(':').next().unwrap_or("") };
    matches!(hote, "localhost" | "127.0.0.1" | "::1")
}

fn chemin_curl() -> PathBuf {
    if let Some(root) = std::env::var_os("SystemRoot") {
        let p = PathBuf::from(root).join("System32").join("curl.exe");
        if p.is_file() {
            return p;
        }
    }
    PathBuf::from("curl")
}

/// Une chaîne dans un fichier de réglages de curl : entre guillemets, `\` et `"` protégés, retours à la ligne en `\n`.
fn echapper(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

pub enum Corps<'a> {
    Aucun,
    Json(&'a Value),
    /// Un fichier envoyé en multipart sous le champ `champ`.
    Fichier { champ: &'a str, chemin: &'a Path, mime: &'a str },
}

#[derive(Clone, Debug)]
pub struct Reponse {
    pub code: u16,
    pub corps: String,
}

fn traduire_curl(code: i32, message: &str) -> String {
    let base = match code {
        6 => "le nom du serveur est introuvable (adresse ? connexion Internet ?)",
        7 => "connexion refusée (serveur éteint, mauvaise adresse ou mauvais port)",
        28 => "délai dépassé (le serveur ne répond pas)",
        35 | 51 | 58 | 59 | 60 | 77 | 80 | 82 | 83 | 90 | 91 => "la connexion chiffrée (HTTPS) a échoué : certificat du serveur refusé ou protocole non pris en charge",
        _ => "",
    };
    let m = message.trim().trim_start_matches("curl:").trim();
    if base.is_empty() {
        format!("curl a échoué (code {code}) : {m}")
    } else if m.is_empty() {
        base.to_string()
    } else {
        format!("{base} [{m}]")
    }
}

/// Une requête, par `curl.exe`. `annule` est surveillé toutes les 40 ms : curl est alors tué sur-le-champ.
pub fn appeler(methode: &str, url: &str, jeton: Option<&str>, corps: Corps<'_>, annule: &AtomicBool) -> Result<Reponse, Erreur> {
    if annule.load(Ordering::Relaxed) {
        return Err(Erreur::Annule);
    }
    let mut cfg = String::new();
    cfg.push_str("silent\nshow-error\n");
    cfg.push_str("connect-timeout = 15\nmax-time = 300\n");
    cfg.push_str("proto = \"=http,https\"\n");
    cfg.push_str(&format!("url = {}\n", echapper(url)));
    cfg.push_str(&format!("request = {}\n", echapper(methode)));
    cfg.push_str("header = \"Expect:\"\nheader = \"Accept: application/json\"\nheader = \"User-Agent: SHOT-pont-kitsu\"\n");
    if let Some(j) = jeton {
        if j.chars().any(|c| c.is_control()) {
            return Err(Erreur::Reglage("Le jeton de connexion est abîmé : connecte-toi de nouveau.".into()));
        }
        cfg.push_str(&format!("header = {}\n", echapper(&format!("Authorization: Bearer {j}"))));
    }
    match corps {
        Corps::Aucun => {}
        Corps::Json(v) => {
            cfg.push_str("header = \"Content-Type: application/json\"\n");
            cfg.push_str(&format!("data-raw = {}\n", echapper(&v.to_string())));
        }
        Corps::Fichier { champ, chemin, mime } => {
            let spec = format!("{champ}=@\"{}\";type={mime}", chemin.display().to_string().replace('\\', "/").replace('"', ""));
            cfg.push_str(&format!("form = {}\n", echapper(&spec)));
        }
    }
    cfg.push_str("write-out = \"\\n%{http_code}\"\n");

    let mut cmd = Command::new(chemin_curl());
    cmd.args(["-K", "-"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|e| Erreur::Reseau(format!("curl.exe est introuvable ou n'a pas pu démarrer ({e}) : il est livré avec Windows 10 et 11.")))?;
    if let Some(mut si) = child.stdin.take() {
        let octets = cfg.into_bytes();
        // Écrit sur un fil : un corps gros ne bloque pas la surveillance de l'arrêt.
        std::thread::spawn(move || {
            let _ = si.write_all(&octets);
        });
    }
    let mut out = child.stdout.take().ok_or_else(|| Erreur::Reseau("sortie de curl illisible".into()))?;
    let mut err = child.stderr.take().ok_or_else(|| Erreur::Reseau("erreurs de curl illisibles".into()))?;
    let lu_out = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = out.read_to_end(&mut v);
        v
    });
    let lu_err = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = err.read_to_end(&mut v);
        v
    });
    let t0 = Instant::now();
    let statut = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {}
            Err(e) => return Err(Erreur::Reseau(format!("curl : {e}"))),
        }
        if annule.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Erreur::Annule);
        }
        if t0.elapsed() > Duration::from_secs(330) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Erreur::Reseau("délai dépassé".into()));
        }
        std::thread::sleep(Duration::from_millis(40));
    };
    let sortie = String::from_utf8_lossy(&lu_out.join().unwrap_or_default()).into_owned();
    let erreurs = String::from_utf8_lossy(&lu_err.join().unwrap_or_default()).into_owned();
    if !statut.success() {
        return Err(Erreur::Reseau(traduire_curl(statut.code().unwrap_or(-1), &erreurs)));
    }
    let (corps, code) = sortie.rsplit_once('\n').ok_or_else(|| Erreur::Reseau("réponse vide".into()))?;
    let code: u16 = code.trim().parse().map_err(|_| Erreur::Reseau("code de réponse illisible".into()))?;
    Ok(Reponse { code, corps: corps.to_string() })
}

/// Le message d'erreur d'un corps de réponse de Kitsu (`{ "message": … }`), coupé.
fn message_de(corps: &str) -> String {
    let m = serde_json::from_str::<Value>(corps)
        .ok()
        .and_then(|v| v.get("message").or_else(|| v.get("error")).and_then(|m| m.as_str().map(str::to_string)))
        .unwrap_or_default();
    m.chars().take(300).collect()
}

// ---------------------------------------------------------------------------------------------------------- le client

pub struct Client<'a> {
    /// `https://hote` : sans `/api`.
    pub serveur: String,
    pub jeton: Option<String>,
    pub annule: &'a AtomicBool,
}

/// Un identifiant que Kitsu rend, sûr à mettre dans une adresse.
fn id_sur(s: &str) -> bool {
    !s.is_empty() && s.len() <= 80 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn texte(v: &Value, cle: &str) -> String {
    v.get(cle).and_then(Value::as_str).unwrap_or("").to_string()
}

fn texte_ou(v: &Value, cles: &[&str]) -> Option<String> {
    cles.iter().find_map(|c| v.get(*c).and_then(Value::as_str)).filter(|s| !s.is_empty()).map(str::to_string)
}

impl<'a> Client<'a> {
    pub fn new(serveur: &str, jeton: Option<String>, annule: &'a AtomicBool) -> Result<Client<'a>, Erreur> {
        Ok(Client { serveur: serveur_valide(serveur).map_err(Erreur::Reglage)?, jeton, annule })
    }

    fn url(&self, chemin: &str) -> String {
        format!("{}/api{}", self.serveur, chemin)
    }

    fn json(&self, methode: &str, chemin: &str, corps: Corps<'_>) -> Result<Value, Erreur> {
        let r = appeler(methode, &self.url(chemin), self.jeton.as_deref(), corps, self.annule)?;
        match r.code {
            200..=299 => {
                if r.corps.trim().is_empty() {
                    Ok(Value::Null)
                } else {
                    serde_json::from_str(&r.corps).map_err(|e| Erreur::Format(format!("{chemin} : {e}")))
                }
            }
            401 => Err(Erreur::NonAutorise),
            300..=399 => Err(Erreur::Http(r.code, "redirection : utilise l'adresse exacte du serveur (https://…)".into())),
            code => Err(Erreur::Http(code, message_de(&r.corps))),
        }
    }

    pub fn lire(&self, chemin: &str) -> Result<Value, Erreur> {
        self.json("GET", chemin, Corps::Aucun)
    }

    pub fn envoyer(&self, methode: &str, chemin: &str, corps: &Value) -> Result<Value, Erreur> {
        self.json(methode, chemin, Corps::Json(corps))
    }

    fn liste(&self, chemin: &str) -> Result<Vec<Value>, Erreur> {
        match self.lire(chemin)? {
            Value::Array(a) => Ok(a),
            Value::Null => Ok(Vec::new()),
            _ => Err(Erreur::Format(format!("{chemin} : une liste était attendue"))),
        }
    }

    fn id_de(v: &Value, quoi: &str) -> Result<String, Erreur> {
        let id = texte(v, "id");
        if id_sur(&id) {
            Ok(id)
        } else {
            Err(Erreur::Format(format!("{quoi} : identifiant absent ou étrange")))
        }
    }

    /// Ouvre une session : rend le jeton d'accès. Le mot de passe ne sert qu'ici.
    pub fn connexion(serveur: &str, email: &str, mot_de_passe: &str, annule: &AtomicBool) -> Result<String, Erreur> {
        let c = Client::new(serveur, None, annule)?;
        if email.trim().is_empty() || mot_de_passe.is_empty() {
            return Err(Erreur::Reglage("Indique l'adresse e-mail et le mot de passe.".into()));
        }
        let r = appeler("POST", &c.url(&ep::connexion()), None, Corps::Json(&json!({ "email": email.trim(), "password": mot_de_passe })), annule)?;
        match r.code {
            200..=299 => {
                let v: Value = serde_json::from_str(&r.corps).map_err(|e| Erreur::Format(format!("connexion : {e}")))?;
                texte_ou(&v, &["access_token"]).ok_or_else(|| Erreur::Format("connexion : pas de jeton dans la réponse".into()))
            }
            400 | 401 | 403 => Err(Erreur::NonAutorise),
            code => Err(Erreur::Http(code, message_de(&r.corps))),
        }
    }

    /// Le jeton est-il encore accepté ?
    pub fn verifier_jeton(&self) -> Result<(), Erreur> {
        let v = self.lire(&ep::authentifie())?;
        if v.get("authenticated").and_then(Value::as_bool).unwrap_or(true) {
            Ok(())
        } else {
            Err(Erreur::NonAutorise)
        }
    }

    pub fn projets(&self) -> Result<Vec<Element>, Erreur> {
        Ok(self.liste(&ep::projets())?.iter().filter(|v| !v.get("canceled").and_then(Value::as_bool).unwrap_or(false)).filter_map(Element::de).collect())
    }

    pub fn types_de_tache(&self) -> Result<Vec<TypeTache>, Erreur> {
        Ok(self
            .liste(&ep::types_de_tache())?
            .iter()
            .filter_map(|v| Some(TypeTache { id: Self::id_de(v, "type de tâche").ok()?, nom: texte(v, "name"), pour: texte(v, "for_entity") }))
            .collect())
    }

    pub fn statuts(&self) -> Result<Vec<StatutKitsu>, Erreur> {
        Ok(self
            .liste(&ep::statuts())?
            .iter()
            .filter_map(|v| Some(StatutKitsu { id: Self::id_de(v, "statut").ok()?, nom: texte(v, "name"), court: texte(v, "short_name").to_lowercase(), couleur: texte(v, "color") }))
            .collect())
    }

    /// Ce que le projet contient déjà côté Kitsu.
    pub fn distant(&self, projet: &str) -> Result<Distant, Erreur> {
        if !id_sur(projet) {
            return Err(Erreur::Reglage("Choisis d'abord le projet Kitsu.".into()));
        }
        let episodes = self.liste(&ep::episodes(projet))?.iter().filter_map(Element::de).collect();
        let sequences = self
            .liste(&ep::sequences(projet))?
            .iter()
            .filter_map(|v| {
                Some(SeqDistante { id: Self::id_de(v, "séquence").ok()?, nom: texte(v, "name"), episode: texte_ou(v, &["episode_id", "parent_id"]) })
            })
            .collect();
        let plans = self
            .liste(&ep::plans(projet))?
            .iter()
            .filter_map(|v| {
                Some(PlanDistant {
                    id: Self::id_de(v, "plan").ok()?,
                    nom: texte(v, "name"),
                    sequence: texte_ou(v, &["sequence_id", "parent_id"]),
                    images: v.get("nb_frames").and_then(Value::as_i64),
                    description: texte(v, "description"),
                    data: v.get("data").filter(|d| d.is_object()).cloned().unwrap_or_else(|| json!({})),
                })
            })
            .collect();
        Ok(Distant { episodes, sequences, plans })
    }

    pub fn creer_episode(&self, projet: &str, nom: &str) -> Result<String, Erreur> {
        Self::id_de(&self.envoyer("POST", &ep::episodes(projet), &json!({ "name": nom }))?, "épisode créé")
    }

    pub fn creer_sequence(&self, projet: &str, nom: &str, episode: Option<&str>) -> Result<String, Erreur> {
        let mut corps = json!({ "name": nom });
        if let Some(e) = episode {
            corps["episode_id"] = json!(e);
        }
        Self::id_de(&self.envoyer("POST", &ep::sequences(projet), &corps)?, "séquence créée")
    }

    pub fn creer_plan(&self, projet: &str, sequence: &str, c: &CiblePlan) -> Result<String, Erreur> {
        let corps = json!({
            "name": c.nom,
            "sequence_id": sequence,
            "nb_frames": c.images,
            "description": c.description,
            "data": { "frame_in": c.entree, "frame_out": c.sortie, "fps": c.cadence },
        });
        Self::id_de(&self.envoyer("POST", &ep::plans(projet), &corps)?, "plan créé")
    }

    pub fn mettre_a_jour_plan(&self, id: &str, c: &CiblePlan, data_distante: &Value) -> Result<(), Erreur> {
        let mut data = if data_distante.is_object() { data_distante.clone() } else { json!({}) };
        data["frame_in"] = json!(c.entree);
        data["frame_out"] = json!(c.sortie);
        data["fps"] = json!(c.cadence);
        self.envoyer("PUT", &ep::plan(id), &json!({ "nb_frames": c.images, "description": c.description, "data": data }))?;
        Ok(())
    }

    /// La tâche du type `type_de_tache` du plan : la trouve, sinon la crée.
    pub fn tache_du_plan(&self, projet: &str, plan: &str, type_de_tache: &str) -> Result<String, Erreur> {
        let trouvee = |l: &[Value]| l.iter().find(|t| texte(t, "task_type_id") == type_de_tache).and_then(|t| Self::id_de(t, "tâche").ok());
        if let Some(id) = trouvee(&self.liste(&ep::taches_du_plan(plan))?) {
            return Ok(id);
        }
        let creees = self.envoyer("POST", &ep::creer_taches(projet, type_de_tache), &json!([plan]))?;
        let l = creees.as_array().cloned().unwrap_or_default();
        if let Some(id) = trouvee(&l).or_else(|| l.first().and_then(|t| Self::id_de(t, "tâche").ok())) {
            return Ok(id);
        }
        // Créée mais pas montrée dans la réponse : on relit.
        trouvee(&self.liste(&ep::taches_du_plan(plan))?).ok_or_else(|| Erreur::Format("le plan n'a pas de tâche de ce type et Kitsu n'a pas pu la créer".into()))
    }

    pub fn commenter(&self, tache: &str, statut: &str, texte_: &str) -> Result<String, Erreur> {
        Self::id_de(&self.envoyer("POST", &ep::commenter(tache), &json!({ "task_status_id": statut, "comment": texte_ }))?, "commentaire publié")
    }

    pub fn ajouter_apercu(&self, tache: &str, commentaire: &str) -> Result<String, Erreur> {
        Self::id_de(&self.envoyer("POST", &ep::ajouter_apercu(tache, commentaire), &json!({}))?, "prévisualisation")
    }

    pub fn televerser_apercu(&self, apercu: &str, fichier: &Path) -> Result<(), Erreur> {
        let mime = match fichier.extension().and_then(|e| e.to_str()).map(str::to_lowercase).as_deref() {
            Some("png") => "image/png",
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("mp4") => "video/mp4",
            Some("mov") => "video/quicktime",
            _ => "application/octet-stream",
        };
        self.json("POST", &ep::televerser_apercu(apercu), Corps::Fichier { champ: "file", chemin: fichier, mime })?;
        Ok(())
    }

    /// Les commentaires d'une tâche, tels que Kitsu les rend.
    pub fn commentaires(&self, tache: &str) -> Result<Vec<Value>, Erreur> {
        self.liste(&ep::commentaires(tache))
    }

    pub fn taches_du_plan(&self, plan: &str) -> Result<Vec<Value>, Erreur> {
        self.liste(&ep::taches_du_plan(plan))
    }
}

// ------------------------------------------------------------------------------------------- ce que Kitsu montre

#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub id: String,
    pub nom: String,
}

impl Element {
    fn de(v: &Value) -> Option<Element> {
        let id = texte(v, "id");
        id_sur(&id).then(|| Element { id, nom: texte(v, "name") })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypeTache {
    pub id: String,
    pub nom: String,
    /// « Shot », « Asset »… (peut manquer).
    pub pour: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StatutKitsu {
    pub id: String,
    pub nom: String,
    /// `short_name` en minuscules : todo, wip, wfa, retake, done…
    pub court: String,
    pub couleur: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SeqDistante {
    pub id: String,
    pub nom: String,
    pub episode: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanDistant {
    pub id: String,
    pub nom: String,
    pub sequence: Option<String>,
    pub images: Option<i64>,
    pub description: String,
    pub data: Value,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Distant {
    pub episodes: Vec<Element>,
    pub sequences: Vec<SeqDistante>,
    pub plans: Vec<PlanDistant>,
}

// ----------------------------------------------------------------------------------------------------- les réglages

/// Les réglages du pont, dans `reglages.toml` (par poste, pas par projet). Ni mot de passe ni jeton ici.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Reglages {
    /// Le pont est allumé : sans cela, aucun bouton ne parle à Kitsu.
    pub actif: bool,
    pub serveur: String,
    pub email: String,
    pub rester_connecte: bool,
    pub projet_id: String,
    pub projet_nom: String,
    /// Le type de tâche du storyboard.
    pub tache_id: String,
    pub tache_nom: String,
    /// Les statuts de tâche de Kitsu qui répondent aux statuts d'une note de SHOT (identifiants de statut ; vide : pas choisi).
    pub statut_a_faire: String,
    pub statut_discussion: String,
    pub statut_fait: String,
    pub statut_refuse: String,
    /// Le statut où l'on met la tâche quand SHOT y publie (d'ordinaire « en attente de validation »).
    pub statut_publication: String,
    pub vignettes: bool,
    pub videos: bool,
    pub film_entier: bool,
}

impl Default for Reglages {
    fn default() -> Self {
        Reglages {
            actif: false,
            serveur: String::new(),
            email: String::new(),
            rester_connecte: false,
            projet_id: String::new(),
            projet_nom: String::new(),
            tache_id: String::new(),
            tache_nom: String::new(),
            statut_a_faire: String::new(),
            statut_discussion: String::new(),
            statut_fait: String::new(),
            statut_refuse: String::new(),
            statut_publication: String::new(),
            vignettes: true,
            videos: false,
            film_entier: true,
        }
    }
}

impl Reglages {
    pub fn statut_de(&self, s: Status) -> &str {
        match s {
            Status::Todo => &self.statut_a_faire,
            Status::Discussion => &self.statut_discussion,
            Status::Done => &self.statut_fait,
            Status::Refused => &self.statut_refuse,
        }
    }

    pub fn regler_statut(&mut self, s: Status, id: String) {
        match s {
            Status::Todo => self.statut_a_faire = id,
            Status::Discussion => self.statut_discussion = id,
            Status::Done => self.statut_fait = id,
            Status::Refused => self.statut_refuse = id,
        }
    }

    /// Le statut de note de SHOT qui répond à ce statut de tâche de Kitsu (rien : aucun ne lui répond). Dans l'ordre À faire, En discussion,
    /// Fait, Refusé.
    pub fn statut_shot(&self, kitsu: &str) -> Option<Status> {
        if kitsu.is_empty() {
            return None;
        }
        [Status::Todo, Status::Discussion, Status::Done, Status::Refused].into_iter().find(|s| self.statut_de(*s) == kitsu)
    }

    /// Remplit ce qui est vide avec les statuts usuels de Kitsu (todo, wip, done, retake, wfa), d'après les noms courts.
    pub fn remplir_defauts(&mut self, statuts: &[StatutKitsu]) {
        let trouve = |court: &str| statuts.iter().find(|s| s.court == court).map(|s| s.id.clone());
        for (s, court) in [(Status::Todo, "todo"), (Status::Discussion, "wip"), (Status::Done, "done"), (Status::Refused, "retake")] {
            if self.statut_de(s).is_empty() {
                if let Some(id) = trouve(court) {
                    self.regler_statut(s, id);
                }
            }
        }
        if self.statut_publication.is_empty() {
            if let Some(id) = trouve("wfa") {
                self.statut_publication = id;
            }
        }
    }

    /// Ce qu'il manque avant de publier.
    pub fn manque_pour_publier(&self) -> Option<String> {
        if self.projet_id.is_empty() {
            return Some("Choisis le projet Kitsu.".into());
        }
        if self.tache_id.is_empty() {
            return Some("Choisis le type de tâche du storyboard.".into());
        }
        if (self.vignettes || self.videos) && self.statut_publication.is_empty() {
            return Some("Choisis le statut où mettre la tâche à la publication.".into());
        }
        None
    }
}

// -------------------------------------------------------------------------------------- le fichier d'identités

/// Ce que Kitsu a rendu pour un plan de SHOT, et ce qui lui a été envoyé.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlanIds {
    pub id: String,
    pub nom: String,
    /// La tâche du storyboard (retrouvée quand elle manque).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub tache: String,
    /// L'empreinte du plan à la dernière prévisualisation envoyée (hexadécimal).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub apercu: String,
    /// Une publication commencée et pas finie : on la reprend sans republier un commentaire.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub en_cours: Option<EnCours>,
    /// Les commentaires que SHOT a publiés (reconnus et laissés de côté à la lecture).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub publies: Vec<String>,
    /// Le dernier statut de la tâche lu dans Kitsu.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub statut: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub statut_couleur: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnCours {
    pub empreinte: String,
    pub commentaire: String,
    /// Prévisualisation créée et fichier envoyé, par sorte (vignette, video).
    pub vignette: Option<String>,
    pub vignette_ok: bool,
    pub video: Option<String>,
    pub video_ok: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SeqIds {
    pub id: String,
    pub nom: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ids {
    pub format: String,
    pub version: u32,
    pub serveur: String,
    pub projet: String,
    /// Les épisodes par nom.
    pub episodes: BTreeMap<String, String>,
    /// Les séquences par identifiant de séquence de SHOT.
    pub sequences: BTreeMap<String, SeqIds>,
    /// Les plans par identifiant de plan de SHOT.
    pub plans: BTreeMap<String, PlanIds>,
}

impl Default for Ids {
    fn default() -> Self {
        Ids { format: FORMAT.into(), version: VERSION, serveur: String::new(), projet: String::new(), episodes: BTreeMap::new(), sequences: BTreeMap::new(), plans: BTreeMap::new() }
    }
}

impl Ids {
    /// Lit `kitsu.json` du dossier du projet ; rien s'il n'existe pas. Un fichier qui ne se lit pas est une erreur (on ne l'écrase pas en silence).
    pub fn lire(dossier: &Path) -> Result<Option<Ids>, String> {
        let p = dossier.join(FICHIER);
        let t = match std::fs::read_to_string(&p) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("{} : {e}", p.display())),
        };
        let ids: Ids = serde_json::from_str(&t).map_err(|e| format!("{} : {e}", p.display()))?;
        if ids.format != FORMAT {
            return Err(format!("{} n'est pas un fichier d'identités Kitsu de SHOT.", p.display()));
        }
        Ok(Some(ids))
    }

    pub fn ecrire(&self, dossier: &Path) -> Result<(), String> {
        let t = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        crate::export::write_atomic(&dossier.join(FICHIER), t.as_bytes())
    }

    /// Les identifiants ne valent que pour ce serveur et ce projet Kitsu : sinon on repart de zéro (on retrouvera les plans par leur nom).
    pub fn pour(mut self, serveur: &str, projet: &str) -> Ids {
        if self.serveur != serveur || self.projet != projet {
            self = Ids { serveur: serveur.to_string(), projet: projet.to_string(), ..Ids::default() };
        }
        self
    }
}

// ------------------------------------------------------------------------------- le jeton, chiffré par Windows

/// Le jeton gardé, chiffré par DPAPI (`CryptProtectData` : seul ce compte Windows sur ce poste peut le relire), avec l'adresse du serveur.
pub mod jeton {
    use std::path::PathBuf;

    const ENTROPIE: &[u8] = b"SHOT-pont-kitsu-1";

    pub fn fichier() -> Option<PathBuf> {
        crate::donnees::roaming().map(|d| d.join("kitsu.jeton"))
    }

    #[cfg(windows)]
    mod ffi {
        use std::ffi::c_void;

        #[repr(C)]
        pub struct Blob {
            pub cb: u32,
            pub pb: *mut u8,
        }

        #[link(name = "crypt32")]
        extern "system" {
            pub fn CryptProtectData(entree: *const Blob, description: *const u16, entropie: *const Blob, reserve: *mut c_void, invite: *mut c_void, drapeaux: u32, sortie: *mut Blob) -> i32;
            pub fn CryptUnprotectData(entree: *const Blob, description: *mut *mut u16, entropie: *const Blob, reserve: *mut c_void, invite: *mut c_void, drapeaux: u32, sortie: *mut Blob) -> i32;
        }

        #[link(name = "kernel32")]
        extern "system" {
            pub fn LocalFree(h: *mut c_void) -> *mut c_void;
        }
    }

    #[cfg(windows)]
    fn transformer(donnees: &[u8], chiffrer: bool) -> Result<Vec<u8>, String> {
        use std::ptr::null_mut;
        let mut entree = ffi::Blob { cb: donnees.len() as u32, pb: donnees.as_ptr() as *mut u8 };
        let mut entropie = ffi::Blob { cb: ENTROPIE.len() as u32, pb: ENTROPIE.as_ptr() as *mut u8 };
        let mut sortie = ffi::Blob { cb: 0, pb: null_mut() };
        // SAFETY : les blobs pointent vers des tampons vivants le temps de l'appel ; la sortie est allouée par Windows et libérée par LocalFree.
        let ok = unsafe {
            if chiffrer {
                ffi::CryptProtectData(&mut entree, std::ptr::null(), &mut entropie, null_mut(), null_mut(), 1, &mut sortie)
            } else {
                ffi::CryptUnprotectData(&mut entree, null_mut(), &mut entropie, null_mut(), null_mut(), 1, &mut sortie)
            }
        };
        if ok == 0 || sortie.pb.is_null() {
            return Err("Windows n'a pas pu chiffrer ou déchiffrer le jeton (DPAPI).".into());
        }
        // SAFETY : `sortie` décrit `cb` octets valides jusqu'au LocalFree.
        let v = unsafe { std::slice::from_raw_parts(sortie.pb, sortie.cb as usize).to_vec() };
        unsafe {
            ffi::LocalFree(sortie.pb as *mut _);
        }
        Ok(v)
    }

    #[cfg(not(windows))]
    fn transformer(_: &[u8], _: bool) -> Result<Vec<u8>, String> {
        Err("Le jeton ne peut être gardé que sous Windows (DPAPI).".into())
    }

    pub fn chiffrer(clair: &[u8]) -> Result<Vec<u8>, String> {
        transformer(clair, true)
    }

    pub fn dechiffrer(chiffre: &[u8]) -> Result<Vec<u8>, String> {
        transformer(chiffre, false)
    }

    /// Garde le jeton de `serveur` (remplace l'ancien).
    pub fn garder(serveur: &str, jeton: &str) -> Result<(), String> {
        let p = fichier().ok_or("dossier %APPDATA% introuvable")?;
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{} : {e}", d.display()))?;
        }
        let clair = serde_json::json!({ "serveur": serveur, "jeton": jeton }).to_string();
        let c = chiffrer(clair.as_bytes())?;
        crate::export::write_atomic(&p, &c)
    }

    /// Le jeton gardé, s'il est pour ce serveur et lisible.
    pub fn relire(serveur: &str) -> Option<String> {
        let c = std::fs::read(fichier()?).ok()?;
        let clair = dechiffrer(&c).ok()?;
        let v: serde_json::Value = serde_json::from_slice(&clair).ok()?;
        (v.get("serveur")?.as_str()? == serveur).then(|| v.get("jeton")?.as_str().map(str::to_string)).flatten()
    }

    /// Oublie le jeton gardé.
    pub fn oublier() {
        if let Some(p) = fichier() {
            let _ = std::fs::remove_file(p);
        }
    }
}

// ------------------------------------------------------------------------------------------- ce que SHOT veut

/// Un plan tel que Kitsu doit le voir.
#[derive(Clone, Debug, PartialEq)]
pub struct CiblePlan {
    /// Rang du panneau.
    pub rang: usize,
    /// Identifiant stable du plan dans SHOT.
    pub panel: u64,
    /// Identifiant stable de sa séquence.
    pub sequence: u64,
    pub sequence_nom: String,
    pub nom: String,
    pub description: String,
    pub images: usize,
    pub entree: i64,
    pub sortie: i64,
    pub cadence: u32,
    /// L'empreinte du contenu du plan (`envoisui`), si elle est connue.
    pub empreinte: Option<u64>,
}

/// Le nom de l'épisode dans Kitsu : celui du projet (un nombre devient « EP01 »), rien si le projet n'en a pas.
pub fn nom_episode(doc: &Document) -> Option<String> {
    let e = doc.properties.episode.trim();
    if e.is_empty() {
        return None;
    }
    Some(match e.parse::<u32>() {
        Ok(n) => format!("EP{n:02}"),
        Err(_) => e.to_string(),
    })
}

/// Les plans `plans` (rangs), dans l'ordre du film, avec leurs noms (modèle de noms de §87), leurs images d'entrée et de sortie.
pub fn cibles(doc: &Document, plans: &[usize], depart: i64, empreintes: &HashMap<u64, u64>) -> Vec<CiblePlan> {
    crate::listeplans::construire(doc, plans, depart, None, false)
        .into_iter()
        .map(|l| {
            let panel = doc.panels[l.panel].id;
            CiblePlan {
                rang: l.panel,
                panel,
                sequence: doc.sequence_of(l.panel),
                sequence_nom: l.sequence,
                nom: l.plan,
                description: l.description,
                images: l.images,
                entree: l.entree,
                sortie: l.sortie,
                cadence: l.cadence,
                empreinte: empreintes.get(&panel).copied(),
            }
        })
        .collect()
}

fn hex(n: u64) -> String {
    format!("{n:016x}")
}

// ------------------------------------------------------------------------------------------------- le plan d'action

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Creer,
    MettreAJour,
    Rien,
}

#[derive(Clone, Debug)]
pub struct SeqPlan {
    pub cle: u64,
    pub nom: String,
    /// `None` : à créer.
    pub id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct PlanPlan {
    /// Index dans `Plan::cibles`.
    pub cible: usize,
    pub id: Option<String>,
    pub action: Action,
    pub apercu: bool,
    pub data_distante: Value,
}

/// Ce qu'une publication ferait, calculé sans rien écrire : de quoi l'afficher avant, puis l'exécuter.
#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub cibles: Vec<CiblePlan>,
    pub episode: Option<String>,
    pub episode_id: Option<String>,
    pub episode_a_creer: bool,
    pub sequences: Vec<SeqPlan>,
    pub plans: Vec<PlanPlan>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Compte {
    pub episodes_a_creer: usize,
    pub sequences_a_creer: usize,
    pub a_creer: usize,
    pub a_mettre_a_jour: usize,
    pub apercus: usize,
    pub inchanges: usize,
}

impl Plan {
    pub fn compte(&self) -> Compte {
        let mut c = Compte { episodes_a_creer: usize::from(self.episode_a_creer), sequences_a_creer: self.sequences.iter().filter(|s| s.id.is_none()).count(), ..Default::default() };
        for p in &self.plans {
            match p.action {
                Action::Creer => c.a_creer += 1,
                Action::MettreAJour => c.a_mettre_a_jour += 1,
                Action::Rien => c.inchanges += 1,
            }
            c.apercus += usize::from(p.apercu);
        }
        c
    }

    /// « 12 plans à créer, 5 à mettre à jour, 3 prévisualisations ».
    pub fn phrase(&self) -> String {
        let c = self.compte();
        let s = |n: usize, un: &str, plusieurs: &str| if n > 1 { format!("{n} {plusieurs}") } else { format!("{n} {un}") };
        let mut morceaux = Vec::new();
        if c.episodes_a_creer > 0 {
            morceaux.push("1 épisode à créer".to_string());
        }
        if c.sequences_a_creer > 0 {
            morceaux.push(format!("{} à créer", s(c.sequences_a_creer, "séquence", "séquences")));
        }
        morceaux.push(format!("{} à créer", s(c.a_creer, "plan", "plans")));
        morceaux.push(format!("{} à mettre à jour", c.a_mettre_a_jour));
        morceaux.push(s(c.apercus, "prévisualisation", "prévisualisations"));
        morceaux.join(", ")
    }

    /// Rien à faire.
    pub fn est_vide(&self) -> bool {
        let c = self.compte();
        c.episodes_a_creer + c.sequences_a_creer + c.a_creer + c.a_mettre_a_jour + c.apercus == 0
    }

    /// Les plans (identifiants de SHOT) dont il faut fabriquer une prévisualisation.
    pub fn apercus_a_faire(&self) -> Vec<u64> {
        self.plans.iter().filter(|p| p.apercu).map(|p| self.cibles[p.cible].panel).collect()
    }
}

/// Compare ce que SHOT veut à ce que Kitsu montre. `avec_apercus` : on veut envoyer des prévisualisations. Rien n'est écrit.
pub fn calculer(cibles: Vec<CiblePlan>, episode: Option<String>, ids: &Ids, distant: &Distant, avec_apercus: bool) -> Plan {
    let mut plan = Plan { episode: episode.clone(), ..Default::default() };
    // L'épisode.
    if let Some(nom) = &episode {
        let connu = ids.episodes.get(nom).filter(|id| distant.episodes.iter().any(|e| &e.id == *id)).cloned();
        let id = connu.or_else(|| distant.episodes.iter().find(|e| e.nom.trim().eq_ignore_ascii_case(nom)).map(|e| e.id.clone()));
        plan.episode_a_creer = id.is_none();
        plan.episode_id = id;
    }
    let mut vues: Vec<u64> = Vec::new();
    for c in &cibles {
        if vues.contains(&c.sequence) {
            continue;
        }
        vues.push(c.sequence);
        let dans_episode = |s: &SeqDistante| if plan.episode.is_some() { s.episode == plan.episode_id } else { s.episode.is_none() };
        let connu = ids.sequences.get(&c.sequence.to_string()).filter(|s| distant.sequences.iter().any(|d| d.id == s.id)).map(|s| s.id.clone());
        let id = connu.or_else(|| {
            if plan.episode_a_creer {
                return None;
            }
            distant.sequences.iter().find(|s| s.nom.trim().eq_ignore_ascii_case(c.sequence_nom.trim()) && dans_episode(s)).map(|s| s.id.clone())
        });
        plan.sequences.push(SeqPlan { cle: c.sequence, nom: c.sequence_nom.clone(), id });
    }
    for (i, c) in cibles.iter().enumerate() {
        let seq_id = plan.sequences.iter().find(|s| s.cle == c.sequence).and_then(|s| s.id.clone());
        let connu = ids.plans.get(&c.panel.to_string()).filter(|p| !p.id.is_empty()).and_then(|p| distant.plans.iter().find(|d| d.id == p.id));
        let par_nom = || seq_id.as_ref().and_then(|sid| distant.plans.iter().find(|d| d.sequence.as_ref() == Some(sid) && d.nom.trim().eq_ignore_ascii_case(c.nom.trim())));
        let trouve = connu.or_else(par_nom);
        let (id, action, data) = match trouve {
            None => (None, Action::Creer, json!({})),
            Some(d) => {
                let egal = d.images == Some(c.images as i64)
                    && d.description.trim() == c.description.trim()
                    && d.data.get("frame_in").and_then(Value::as_i64) == Some(c.entree)
                    && d.data.get("frame_out").and_then(Value::as_i64) == Some(c.sortie)
                    && d.data.get("fps").and_then(|f| f.as_i64().or_else(|| f.as_f64().map(|x| x as i64)).or_else(|| f.as_str().and_then(|s| s.parse().ok()))) == Some(c.cadence as i64);
                (Some(d.id.clone()), if egal { Action::Rien } else { Action::MettreAJour }, d.data.clone())
            }
        };
        let deja = ids.plans.get(&c.panel.to_string()).map(|p| p.apercu.as_str()).unwrap_or("");
        let apercu = avec_apercus && (id.is_none() || deja.is_empty() || c.empreinte.map(hex).as_deref() != Some(deja));
        plan.plans.push(PlanPlan { cible: i, id, action, apercu, data_distante: data });
    }
    plan.cibles = cibles;
    plan
}

// ------------------------------------------------------------------------------------------------- la publication

/// L'avancement, lu par l'interface pendant que le fil travaille.
#[derive(Default)]
pub struct Progres {
    pub fait: AtomicUsize,
    pub total: AtomicUsize,
    texte: Mutex<String>,
}

impl Progres {
    pub fn dire(&self, t: &str) {
        if let Ok(mut m) = self.texte.lock() {
            *m = t.to_string();
        }
    }

    pub fn etat(&self) -> (String, f32) {
        let t = self.texte.lock().map(|m| m.clone()).unwrap_or_default();
        let total = self.total.load(Ordering::Relaxed).max(1);
        (t, (self.fait.load(Ordering::Relaxed) as f32 / total as f32).min(1.0))
    }

    fn avancer(&self) {
        self.fait.fetch_add(1, Ordering::Relaxed);
    }
}

/// Les fichiers de prévisualisation prêts, par identifiant de plan de SHOT.
#[derive(Clone, Debug, Default)]
pub struct Fichiers {
    pub vignettes: HashMap<u64, PathBuf>,
    pub videos: HashMap<u64, PathBuf>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rapport {
    pub episodes_crees: usize,
    pub sequences_creees: usize,
    pub plans_crees: usize,
    pub plans_mis_a_jour: usize,
    pub apercus: usize,
    pub inchanges: usize,
    pub avertissements: Vec<String>,
}

impl Rapport {
    pub fn phrase(&self) -> String {
        format!(
            "Kitsu à jour : {} plan(s) créé(s), {} mis à jour, {} prévisualisation(s) envoyée(s), {} inchangé(s){}.",
            self.plans_crees,
            self.plans_mis_a_jour,
            self.apercus,
            self.inchanges,
            if self.avertissements.is_empty() { String::new() } else { format!(" ({} avertissement(s))", self.avertissements.len()) }
        )
    }
}

fn arret(annule: &AtomicBool) -> Result<(), Erreur> {
    if annule.load(Ordering::Relaxed) {
        Err(Erreur::Annule)
    } else {
        Ok(())
    }
}

/// Fait ce que dit `plan` : crée ce qui manque, met à jour ce qui diffère, publie les prévisualisations. `ids` est écrit dans `dossier` après chaque
/// étape (un arrêt ou une panne ne perd rien : la fois suivante retrouve ce qui existe). Le rapport d'un arrêt n'est pas rendu : l'erreur dit `Annule`.
pub fn executer(client: &Client<'_>, rg: &Reglages, plan: &Plan, fichiers: &Fichiers, ids: &mut Ids, dossier: Option<&Path>, prog: &Progres) -> Result<Rapport, Erreur> {
    let projet = rg.projet_id.as_str();
    let mut r = Rapport::default();
    let c = plan.compte();
    prog.total.store((c.episodes_a_creer + c.sequences_a_creer + c.a_creer + c.a_mettre_a_jour + c.apercus).max(1), Ordering::Relaxed);
    prog.fait.store(0, Ordering::Relaxed);
    let garder = |ids: &Ids| -> Result<(), Erreur> {
        match dossier {
            Some(d) => ids.ecrire(d).map_err(|e| Erreur::Reglage(format!("kitsu.json n'a pas pu être écrit : {e}"))),
            None => Ok(()),
        }
    };
    // L'épisode.
    let mut episode_id = plan.episode_id.clone();
    if plan.episode_a_creer {
        arret(client.annule)?;
        let nom = plan.episode.clone().unwrap_or_default();
        prog.dire(&format!("Création de l'épisode {nom}"));
        let id = client.creer_episode(projet, &nom)?;
        ids.episodes.insert(nom, id.clone());
        episode_id = Some(id);
        r.episodes_crees += 1;
        garder(ids)?;
        prog.avancer();
    } else if let (Some(nom), Some(id)) = (&plan.episode, &episode_id) {
        ids.episodes.insert(nom.clone(), id.clone());
    }
    // Les séquences.
    let mut seq_ids: HashMap<u64, String> = HashMap::new();
    for s in &plan.sequences {
        arret(client.annule)?;
        let id = match &s.id {
            Some(id) => id.clone(),
            None => {
                prog.dire(&format!("Création de la séquence {}", s.nom));
                let id = client.creer_sequence(projet, &s.nom, episode_id.as_deref())?;
                r.sequences_creees += 1;
                prog.avancer();
                id
            }
        };
        ids.sequences.insert(s.cle.to_string(), SeqIds { id: id.clone(), nom: s.nom.clone() });
        seq_ids.insert(s.cle, id);
        garder(ids)?;
    }
    // Les plans.
    let mut plan_ids: HashMap<usize, String> = HashMap::new();
    for p in &plan.plans {
        arret(client.annule)?;
        let cible = &plan.cibles[p.cible];
        let id = match (p.action, &p.id) {
            (Action::Creer, _) | (_, None) => {
                prog.dire(&format!("Création du plan {}", cible.nom));
                let seq = seq_ids.get(&cible.sequence).ok_or_else(|| Erreur::Format("séquence introuvable pour le plan".into()))?;
                let id = client.creer_plan(projet, seq, cible)?;
                r.plans_crees += 1;
                prog.avancer();
                id
            }
            (Action::MettreAJour, Some(id)) => {
                prog.dire(&format!("Mise à jour du plan {}", cible.nom));
                client.mettre_a_jour_plan(id, cible, &p.data_distante)?;
                r.plans_mis_a_jour += 1;
                prog.avancer();
                id.clone()
            }
            (Action::Rien, Some(id)) => {
                r.inchanges += 1;
                id.clone()
            }
        };
        let entree = ids.plans.entry(cible.panel.to_string()).or_default();
        if entree.id != id {
            *entree = PlanIds { id: id.clone(), ..Default::default() };
        }
        entree.nom = cible.nom.clone();
        plan_ids.insert(p.cible, id);
        garder(ids)?;
    }
    // Les prévisualisations.
    for p in plan.plans.iter().filter(|p| p.apercu) {
        arret(client.annule)?;
        let cible = &plan.cibles[p.cible];
        let Some(plan_id) = plan_ids.get(&p.cible).cloned() else { continue };
        prog.dire(&format!("Prévisualisation du plan {}", cible.nom));
        let vignette = fichiers.vignettes.get(&cible.panel).filter(|_| rg.vignettes);
        let video = fichiers.videos.get(&cible.panel).filter(|_| rg.videos);
        if vignette.is_none() && video.is_none() {
            r.avertissements.push(format!("{} : aucune image à envoyer.", cible.nom));
            prog.avancer();
            continue;
        }
        let cle = cible.panel.to_string();
        let empreinte = cible.empreinte.map(hex).unwrap_or_default();
        let tache = {
            let connue = ids.plans.get(&cle).map(|e| e.tache.clone()).unwrap_or_default();
            if connue.is_empty() {
                let t = client.tache_du_plan(projet, &plan_id, &rg.tache_id)?;
                ids.plans.entry(cle.clone()).or_default().tache = t.clone();
                garder(ids)?;
                t
            } else {
                connue
            }
        };
        // Reprise d'une publication commencée avec la même empreinte, sinon un commentaire neuf.
        let reprise = ids.plans.get(&cle).and_then(|e| e.en_cours.clone()).filter(|e| e.empreinte == empreinte && !e.commentaire.is_empty());
        let mut etat = match reprise {
            Some(e) => e,
            None => {
                let texte = format!("{MARQUE} Plan {} : {} image(s) ({}–{}), mis à jour depuis SHOT.", cible.nom, cible.images, cible.entree, cible.sortie);
                let com = client.commenter(&tache, &rg.statut_publication, &texte)?;
                let e = EnCours { empreinte: empreinte.clone(), commentaire: com.clone(), ..Default::default() };
                let entree = ids.plans.entry(cle.clone()).or_default();
                entree.publies.push(com);
                let trop = entree.publies.len().saturating_sub(200);
                entree.publies.drain(..trop);
                entree.en_cours = Some(e.clone());
                garder(ids)?;
                e
            }
        };
        let commentaire_id = etat.commentaire.clone();
        for (sorte, fichier) in [("vignette", vignette), ("video", video)] {
            let Some(fichier) = fichier else { continue };
            let vignette_ = sorte == "vignette";
            if if vignette_ { etat.vignette_ok } else { etat.video_ok } {
                continue;
            }
            arret(client.annule)?;
            if (if vignette_ { &etat.vignette } else { &etat.video }).is_none() {
                let id = client.ajouter_apercu(&tache, &commentaire_id)?;
                if vignette_ {
                    etat.vignette = Some(id);
                } else {
                    etat.video = Some(id);
                }
                ids.plans.entry(cle.clone()).or_default().en_cours = Some(etat.clone());
                garder(ids)?;
            }
            let id = (if vignette_ { etat.vignette.clone() } else { etat.video.clone() }).unwrap_or_default();
            client.televerser_apercu(&id, fichier)?;
            if vignette_ {
                etat.vignette_ok = true;
            } else {
                etat.video_ok = true;
            }
            ids.plans.entry(cle.clone()).or_default().en_cours = Some(etat.clone());
            garder(ids)?;
        }
        let entree = ids.plans.entry(cle.clone()).or_default();
        entree.apercu = empreinte;
        entree.en_cours = None;
        garder(ids)?;
        r.apercus += 1;
        prog.avancer();
    }
    prog.dire("Terminé");
    Ok(r)
}

// ----------------------------------------------------------------------------------------------- la lecture en retour

#[derive(Clone, Debug, PartialEq)]
pub struct RetourReponse {
    pub id: String,
    pub texte: String,
    pub auteur: String,
    pub cree: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RetourNote {
    pub id: String,
    pub texte: String,
    pub auteur: String,
    pub cree: u64,
    /// Le statut de tâche posé avec ce commentaire.
    pub statut: String,
    pub reponses: Vec<RetourReponse>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RetourPlan {
    /// Identifiant du plan dans SHOT.
    pub panel: u64,
    pub tache: String,
    pub statut_id: String,
    pub statut_nom: String,
    pub statut_couleur: String,
    pub notes: Vec<RetourNote>,
}

/// « 2026-10-30T14:05:09.123456 » (ou avec `Z`, un décalage, un espace) en secondes depuis 1970 ; 0 si illisible.
pub fn lire_date_iso(t: &str) -> u64 {
    let t = t.trim();
    let (jour, heure) = t.split_once(['T', ' ']).unwrap_or((t, "00:00:00"));
    let j: Vec<&str> = jour.split('-').collect();
    let (Some(y), Some(m), Some(d)) = (j.first().and_then(|x| x.parse::<i64>().ok()), j.get(1).and_then(|x| x.parse::<u32>().ok()), j.get(2).and_then(|x| x.parse::<u32>().ok())) else { return 0 };
    if !(1970..=9999).contains(&y) || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return 0;
    }
    let heure = heure.split(['Z', '+', '.']).next().unwrap_or("").trim_end_matches('-');
    let h: Vec<u64> = heure.split(':').filter_map(|x| x.parse().ok()).collect();
    let (hh, mm, ss) = (h.first().copied().unwrap_or(0), h.get(1).copied().unwrap_or(0), h.get(2).copied().unwrap_or(0));
    (crate::envois::jours_depuis_1970(y, m, d).max(0) as u64) * 86_400 + hh.min(23) * 3600 + mm.min(59) * 60 + ss.min(59)
}

fn auteur_de(v: &Value) -> String {
    let p = v.get("person").filter(|p| p.is_object());
    if let Some(p) = p {
        if let Some(n) = texte_ou(p, &["full_name"]) {
            return n;
        }
        let n = format!("{} {}", texte(p, "first_name"), texte(p, "last_name")).trim().to_string();
        if !n.is_empty() {
            return n;
        }
    }
    "Kitsu".into()
}

/// Lit, pour chaque plan déjà publié, le statut de sa tâche storyboard et ses commentaires (sans ceux que SHOT a publiés). Ne change rien dans Kitsu.
pub fn lire_retours(client: &Client<'_>, rg: &Reglages, ids: &Ids, plans: &[u64], prog: &Progres) -> Result<Vec<RetourPlan>, Erreur> {
    if rg.tache_id.is_empty() {
        return Err(Erreur::Reglage("Choisis le type de tâche du storyboard.".into()));
    }
    prog.dire("Lecture des statuts");
    let statuts = client.statuts()?;
    let connus: Vec<(u64, &PlanIds)> = plans.iter().filter_map(|p| ids.plans.get(&p.to_string()).filter(|i| !i.id.is_empty()).map(|i| (*p, i))).collect();
    prog.total.store(connus.len().max(1), Ordering::Relaxed);
    prog.fait.store(0, Ordering::Relaxed);
    let mut out = Vec::new();
    for (panel, i) in connus {
        arret(client.annule)?;
        prog.dire(&format!("Lecture du plan {}", i.nom));
        let taches = client.taches_du_plan(&i.id)?;
        let Some(t) = taches.iter().find(|t| texte(t, "task_type_id") == rg.tache_id) else {
            prog.avancer();
            continue;
        };
        let tache = Client::id_de(t, "tâche")?;
        let statut_id = texte(t, "task_status_id");
        let st = statuts.iter().find(|s| s.id == statut_id);
        let mut notes = Vec::new();
        for c in client.commentaires(&tache)? {
            let Ok(id) = Client::id_de(&c, "commentaire") else { continue };
            let texte_c = texte(&c, "text");
            if texte_c.trim_start().starts_with(MARQUE) || i.publies.contains(&id) || texte_c.trim().is_empty() {
                continue;
            }
            let reponses = c
                .get("replies")
                .and_then(Value::as_array)
                .map(|l| {
                    l.iter()
                        .filter_map(|r| {
                            let rid = texte(r, "id");
                            (id_sur(&rid) && !texte(r, "text").trim().is_empty()).then(|| RetourReponse { id: rid, texte: texte(r, "text"), auteur: auteur_de(r), cree: lire_date_iso(&texte_ou(r, &["date", "created_at"]).unwrap_or_default()) })
                        })
                        .collect()
                })
                .unwrap_or_default();
            notes.push(RetourNote { id, texte: texte_c, auteur: auteur_de(&c), cree: lire_date_iso(&texte(&c, "created_at")), statut: texte(&c, "task_status_id"), reponses });
        }
        out.push(RetourPlan { panel, tache, statut_id, statut_nom: st.map(|s| s.nom.clone()).unwrap_or_default(), statut_couleur: st.map(|s| s.couleur.clone()).unwrap_or_default(), notes });
        prog.avancer();
    }
    prog.dire("Terminé");
    Ok(out)
}

/// Garde le statut lu de chaque tâche dans `kitsu.json` (pour la fiche du plan).
pub fn noter_statuts(ids: &mut Ids, retours: &[RetourPlan]) {
    for r in retours {
        if let Some(i) = ids.plans.get_mut(&r.panel.to_string()) {
            i.statut = r.statut_nom.clone();
            i.statut_couleur = r.statut_couleur.clone();
            if i.tache.is_empty() {
                i.tache = r.tache.clone();
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Resume {
    pub notes: usize,
    pub reponses: usize,
    pub deja: usize,
}

impl Resume {
    pub fn nouveau(&self) -> usize {
        self.notes + self.reponses
    }

    pub fn phrase(&self) -> String {
        if self.nouveau() == 0 {
            return format!("Rien de nouveau dans Kitsu ({} déjà dans le projet).", self.deja);
        }
        format!("{} note(s) et {} réponse(s) venues de Kitsu ({} déjà dans le projet).", self.notes, self.reponses, self.deja)
    }
}

fn note_existante<'a>(doc: &'a Document, panel: u64, id: &str) -> Option<&'a Comment> {
    doc.comments.iter().find(|c| c.panel == panel && c.follow.source == Source::Kitsu && c.follow.external.as_deref() == Some(id))
}

/// Compte ce que `appliquer` ajouterait, sans rien changer (pour ne prendre une étape d'annulation que s'il y a quelque chose).
pub fn previsualiser(doc: &Document, retours: &[RetourPlan]) -> Resume {
    let mut s = Resume::default();
    for r in retours.iter().filter(|r| doc.panels.iter().any(|p| p.id == r.panel)) {
        for n in &r.notes {
            match note_existante(doc, r.panel, &n.id) {
                None => {
                    s.notes += 1;
                    s.reponses += n.reponses.len();
                }
                Some(c) => {
                    for rep in &n.reponses {
                        if c.follow.replies.iter().any(|x| x.external.as_deref() == Some(rep.id.as_str())) {
                            s.deja += 1;
                        } else {
                            s.reponses += 1;
                        }
                    }
                    s.deja += 1;
                }
            }
        }
    }
    s
}

/// Pose les notes de Kitsu dans le fil de décision : une note par commentaire (`source = kitsu`, `externe` = id du commentaire), ses réponses ; une
/// note déjà là n'est jamais touchée (son statut décidé dans SHOT reste), seules ses réponses manquantes s'ajoutent. L'appelant a pris l'étape
/// d'annulation avant.
pub fn appliquer(doc: &mut Document, retours: &[RetourPlan], rg: &Reglages, now: u64) -> Resume {
    let sum = previsualiser(doc, retours);
    for r in retours {
        if !doc.panels.iter().any(|p| p.id == r.panel) {
            continue;
        }
        for n in &r.notes {
            let reponses = |n: &RetourNote, deja: &[Reply]| -> Vec<Reply> {
                n.reponses
                    .iter()
                    .filter(|x| !deja.iter().any(|d| d.external.as_deref() == Some(x.id.as_str())))
                    .map(|x| Reply { author: x.auteur.clone(), text: x.texte.chars().take(crate::commentaires::TEXT_MAX).collect(), created: if x.cree > 0 { x.cree } else { now }, source: Source::Kitsu, external: Some(x.id.clone()) })
                    .collect()
            };
            if let Some(i) = doc.comments.iter().position(|c| c.panel == r.panel && c.follow.source == Source::Kitsu && c.follow.external.as_deref() == Some(n.id.as_str())) {
                let c = &mut doc.comments[i];
                let nouvelles = reponses(n, &c.follow.replies);
                let place = crate::fildecision::REPLIES_MAX.saturating_sub(c.follow.replies.len());
                c.follow.replies.extend(nouvelles.into_iter().take(place));
                continue;
            }
            let id = doc.alloc();
            let (envoi, version) = doc.stamp_follow(r.panel);
            let mut follow = Follow { source: Source::Kitsu, external: Some(n.id.clone()), send: envoi, plan_version: version, ..Default::default() };
            follow.replies = reponses(n, &[]);
            follow.replies.truncate(crate::fildecision::REPLIES_MAX);
            let mut c = Comment {
                id,
                panel: r.panel,
                frame: 0,
                text: n.texte.trim().chars().take(crate::commentaires::TEXT_MAX).collect(),
                author: n.auteur.clone(),
                color: COULEUR_NOTE,
                created: if n.cree > 0 && n.cree <= now + 86_400 { n.cree } else { now },
                done: false,
                note: None,
                annotation: None,
                follow,
            };
            match rg.statut_shot(&n.statut).unwrap_or(Status::Todo) {
                Status::Todo => {}
                s => {
                    let raison = if s == Status::Refused { "Statut Kitsu : à refaire" } else { "" };
                    c.appliquer_statut(s, raison, now);
                }
            }
            doc.comments.push(c);
        }
    }
    sum
}
