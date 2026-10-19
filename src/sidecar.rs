//! Le client du **serveur local** de génération sonore (« sidecar », Anthony, 2026-10-19) : un petit
//! serveur Python que la phase 2 installera avec les modèles (Qwen2.5-VL pour lire le storyboard,
//! Stable Audio Open pour les bruitages et les ambiances, Kokoro-82M pour les voix témoins) et qui
//! écoute sur `127.0.0.1`. SHOT lui parle en JSON sur HTTP (`httplocal.rs`) ; le contrat — adresses,
//! requêtes, réponses, erreurs — est dans `docs/GENERATION_SON.md`.
//!
//! Tant que le serveur n'est pas installé, chaque appel rend `ErreurGen::NonInstalle`, et le bouton
//! « Tester » des Préférences dit « non installé » sans rien casser.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use serde_json::{json, Value};

use crate::base64;
use crate::fournisseurs::{Controle, DemandeLecture, DemandeSon, DemandeVoix, ErreurGen, GenerateurSon, GenerateurVoix, LecteurStoryboard, SonGenere};
use crate::httplocal::{requete, ReponseHttp};
use crate::indications::IndicationBrute;
use crate::lecturesb;
use crate::serveurlocal::Demarrage;

/// Le port où le serveur local écoute, sauf réglage contraire.
pub const PORT_PAR_DEFAUT: u16 = 8765;
/// La version du contrat que SHOT parle.
pub const VERSION_CONTRAT: u32 = 1;
const DELAI_ETAT: Duration = Duration::from_secs(5);
const DELAI_LECTURE: Duration = Duration::from_secs(600);
const DELAI_SON: Duration = Duration::from_secs(600);
const DELAI_VOIX: Duration = Duration::from_secs(300);

/// Où en est un modèle du serveur.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EtatModele {
    pub installe: bool,
    pub nom: String,
}

/// Ce que le serveur dit de lui (`GET /v1/etat`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EtatSidecar {
    pub version: u32,
    pub lecture: EtatModele,
    pub son: EtatModele,
    pub voix: EtatModele,
}

impl EtatSidecar {
    /// Une phrase pour le bouton « Tester » : ce qui est prêt, ce qui manque.
    pub fn resume(&self) -> String {
        let un = |nom: &str, m: &EtatModele| {
            if m.installe {
                format!("{nom} prêt{}", if m.nom.is_empty() { String::new() } else { format!(" ({})", m.nom) })
            } else {
                format!("{nom} non installé")
            }
        };
        format!("Serveur local prêt : {}, {}, {}.", un("lecture", &self.lecture), un("sons", &self.son), un("voix", &self.voix))
    }
}

/// Le client du serveur local.
#[derive(Clone, Debug)]
pub struct ClientSidecar {
    pub adresse: SocketAddr,
    /// Si le serveur ne répond pas : SHOT le lance et l'attend (réglage « démarrage automatique »).
    /// Jamais pour `etat` (le bouton « Tester » ne lance rien).
    pub demarrage: Option<Demarrage>,
}

impl ClientSidecar {
    /// Le serveur de cet ordinateur, sur le port `port`.
    pub fn sur_le_port(port: u16) -> ClientSidecar {
        ClientSidecar { adresse: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port), demarrage: None }
    }

    /// Le même client, qui lance le serveur au besoin.
    pub fn avec_demarrage(mut self, demarrage: Demarrage) -> ClientSidecar {
        self.demarrage = Some(demarrage);
        self
    }

    /// Une requête, avec le lancement du serveur si personne n'écoute (la connexion refusée, pas une
    /// réponse « non installé » du serveur lui-même).
    fn envoyer(&self, methode: &str, chemin: &str, octets: Option<&[u8]>, controle: &Controle, delai: Duration) -> Result<ReponseHttp, ErreurGen> {
        let une_fois = || requete(self.adresse, methode, chemin, octets.map(|b| ("application/json", b)), controle, delai);
        match (une_fois(), &self.demarrage) {
            (Err(ErreurGen::NonInstalle(_)), Some(d)) => {
                d.assurer(&ClientSidecar::sur_le_port(self.adresse.port()), controle)?;
                une_fois()
            }
            (r, _) => r,
        }
    }

    fn appeler(&self, methode: &str, chemin: &str, corps: Option<&Value>, controle: &Controle, delai: Duration) -> Result<ReponseHttp, ErreurGen> {
        let octets = corps.map(|c| serde_json::to_vec(c).map_err(|e| ErreurGen::Invalide(e.to_string()))).transpose()?;
        let reponse = self.envoyer(methode, chemin, octets.as_deref(), controle, delai)?;
        if reponse.reussie() {
            Ok(reponse)
        } else {
            Err(erreur_de_reponse(&reponse))
        }
    }

    fn json(&self, methode: &str, chemin: &str, corps: Option<&Value>, controle: &Controle, delai: Duration) -> Result<Value, ErreurGen> {
        let r = self.appeler(methode, chemin, corps, controle, delai)?;
        serde_json::from_slice(&r.corps).map_err(|e| ErreurGen::Invalide(format!("réponse du serveur local illisible ({e})")))
    }

    /// `GET /v1/etat` : le serveur répond-il, et quels modèles a-t-il ?
    pub fn etat(&self, controle: &Controle) -> Result<EtatSidecar, ErreurGen> {
        // (Sans lancement automatique, même si le client en a un : on demande, on ne démarre rien.)
        let r = ClientSidecar::sur_le_port(self.adresse.port()).appeler("GET", "/v1/etat", None, controle, DELAI_ETAT)?;
        let v: Value = serde_json::from_slice(&r.corps).map_err(|e| ErreurGen::Invalide(format!("réponse du serveur local illisible ({e})")))?;
        let modele = |role: &str| {
            let m = &v["modeles"][role];
            EtatModele { installe: m["installe"].as_bool().unwrap_or(false), nom: m["nom"].as_str().unwrap_or("").chars().take(80).collect() }
        };
        Ok(EtatSidecar { version: v["version"].as_u64().unwrap_or(0) as u32, lecture: modele("lecture"), son: modele("son"), voix: modele("voix") })
    }

    fn son(&self, chemin: &str, corps: &Value, controle: &Controle, delai: Duration) -> Result<SonGenere, ErreurGen> {
        let r = self.appeler("POST", chemin, Some(corps), controle, delai)?;
        son_depuis_reponse(&r)
    }
}

/// Le son d'une réponse : des octets `audio/*`, ou du JSON `{"audio_b64", "format"}`.
pub fn son_depuis_reponse(r: &ReponseHttp) -> Result<SonGenere, ErreurGen> {
    let type_contenu = r.entete("content-type").unwrap_or("").to_ascii_lowercase();
    if let Some(audio) = type_contenu.strip_prefix("audio/") {
        let ext = match audio.split(';').next().unwrap_or("").trim() {
            "wav" | "x-wav" | "wave" | "vnd.wave" => "wav",
            "mpeg" | "mp3" => "mp3",
            "flac" | "x-flac" => "flac",
            "ogg" | "vorbis" => "ogg",
            _ => return Err(ErreurGen::Invalide(format!("format de son non géré ({type_contenu})"))),
        };
        return controler(SonGenere { octets: r.corps.clone(), ext: ext.into() });
    }
    let v: Value = serde_json::from_slice(&r.corps).map_err(|e| ErreurGen::Invalide(format!("réponse du serveur local illisible ({e})")))?;
    let octets = v["audio_b64"].as_str().and_then(base64::decode).ok_or_else(|| ErreurGen::Invalide("la réponse n'a pas de son (audio_b64)".into()))?;
    let ext = match v["format"].as_str().unwrap_or("wav") {
        f @ ("wav" | "mp3" | "flac" | "ogg") => f.to_string(),
        f => return Err(ErreurGen::Invalide(format!("format de son non géré ({f})"))),
    };
    controler(SonGenere { octets, ext })
}

fn controler(son: SonGenere) -> Result<SonGenere, ErreurGen> {
    if son.octets.is_empty() {
        return Err(ErreurGen::Invalide("le serveur a rendu un son vide".into()));
    }
    Ok(son)
}

/// L'erreur d'une réponse qui n'est pas un succès : `{"erreur": {"code", "message"}}`, sinon le code HTTP.
pub fn erreur_de_reponse(r: &ReponseHttp) -> ErreurGen {
    let v: Value = serde_json::from_slice(&r.corps).unwrap_or(Value::Null);
    let message: String = v["erreur"]["message"].as_str().unwrap_or("").chars().take(300).collect();
    let avec = |defaut: &str| if message.is_empty() { defaut.to_string() } else { message.clone() };
    match v["erreur"]["code"].as_str().unwrap_or("") {
        "modele_absent" | "non_installe" => ErreurGen::NonInstalle(avec("modèle absent")),
        "occupe" | "indisponible" => ErreurGen::Indisponible(avec("serveur occupé")),
        "invalide" => ErreurGen::Invalide(avec("demande invalide")),
        _ => match r.statut {
            501 => ErreurGen::NonInstalle(avec("fonction non installée")),
            429 | 503 => ErreurGen::Indisponible(avec("serveur occupé")),
            400..=499 => ErreurGen::Invalide(avec(&format!("demande refusée par le serveur local ({})", r.statut))),
            s => ErreurGen::Indisponible(avec(&format!("erreur du serveur local ({s})"))),
        },
    }
}

// ----------------------------------------------------------------------------------- les rôles

/// Rôle 1 : Qwen2.5-VL, par le serveur local.
pub struct LecteurSidecar(pub ClientSidecar);

impl LecteurStoryboard for LecteurSidecar {
    fn nom(&self) -> String {
        "Qwen2.5-VL (local)".into()
    }

    fn lire_plan(&self, demande: &DemandeLecture, index: usize, controle: &Controle) -> Result<Vec<IndicationBrute>, ErreurGen> {
        controle.verifier()?;
        let plan = demande.plans.get(index).ok_or_else(|| ErreurGen::Invalide("plan inconnu".into()))?;
        let corps = json!({
            "version": VERSION_CONTRAT,
            "langue": "fr",
            "consignes": lecturesb::CONSIGNES,
            "plan": lecturesb::plan_en_json(demande, index),
            "vignette_png_b64": plan.vignette_png.as_deref().map(base64::encode),
        });
        let v = self.0.json("POST", "/v1/lecture", Some(&corps), controle, DELAI_LECTURE)?;
        let liste = v["indications"].as_array().ok_or_else(|| ErreurGen::Invalide("la réponse du serveur local n'a pas de liste d'indications".into()))?;
        Ok(lecturesb::indications_depuis_liste(liste, plan.frames))
    }
}

/// Rôle 2 : Stable Audio Open, par le serveur local.
pub struct SonSidecar(pub ClientSidecar);

impl GenerateurSon for SonSidecar {
    fn nom(&self) -> String {
        "Stable Audio Open (local)".into()
    }

    fn generer(&self, demande: &DemandeSon, controle: &Controle) -> Result<SonGenere, ErreurGen> {
        controle.verifier()?;
        let corps = json!({
            "version": VERSION_CONTRAT,
            "genre": serde_json::to_value(demande.genre).unwrap_or(Value::Null),
            "prompt": demande.prompt,
            "duree_s": demande.secondes,
        });
        self.0.son("/v1/son", &corps, controle, DELAI_SON)
    }
}

/// Rôle 3 : Kokoro-82M, par le serveur local.
pub struct VoixSidecar(pub ClientSidecar);

impl GenerateurVoix for VoixSidecar {
    fn nom(&self) -> String {
        "Kokoro-82M (local)".into()
    }

    fn generer(&self, demande: &DemandeVoix, controle: &Controle) -> Result<SonGenere, ErreurGen> {
        controle.verifier()?;
        let corps = json!({
            "version": VERSION_CONTRAT,
            "texte": demande.texte,
            "personnage": demande.personnage,
            "voix": demande.voix,
            "langue": demande.langue,
        });
        self.0.son("/v1/voix", &corps, controle, DELAI_VOIX)
    }
}
