//! Les **services en ligne** de la génération sonore (Anthony, 2026-10-19) : Claude pour lire le
//! storyboard, ElevenLabs (Sound Effects, puis TTS) ou l'API de Stable Audio pour les sons, ElevenLabs
//! pour les voix témoins.
//!
//! Ce module sait **construire** chaque requête et **lire** chaque réponse (formats des API, erreurs
//! traduites en français) ; il ne sait pas encore **l'envoyer** : le transport HTTPS (`Transport`) est
//! « à brancher ». La compilation hors ligne de SHOT n'a aucune caisse TLS (ni `ureq`, ni `reqwest`) et
//! cette passe n'en télécharge pas ; `TransportABrancher` rend donc une erreur claire à la place. Brancher
//! un transport, c'est écrire un `impl Transport` — par exemple sur WinHTTP, qui fait le TLS pour
//! Windows sans aucune caisse — et le donner à `FabriqueReelle` (`generation.rs`) ; rien d'autre ne change.
//!
//! Les clés viennent du `Coffre` (Gestionnaire d'identifiants de Windows) au moment de l'appel, et ne
//! sont ni gardées ni montrées : `RequeteHttps` cache les en-têtes secrets dans `{:?}`.
//!
//! Les adresses et les formats ci-dessous sont ceux des documentations des services au moment de
//! l'écriture, **à vérifier** avant le premier vrai appel (voir `docs/GENERATION_SON.md`) : aucune
//! n'a pu être essayée, faute de réseau pendant les essais.

use std::sync::Arc;

use serde_json::{json, Value};

use crate::base64;
use crate::clessecretes::{Cle, Coffre, Service};
use crate::fournisseurs::{Controle, DemandeLecture, DemandeSon, DemandeVoix, ErreurGen, GenerateurSon, GenerateurVoix, LecteurStoryboard, SonGenere};
use crate::httplocal::ReponseHttp;
use crate::indications::IndicationBrute;
use crate::lecturesb;

/// Le modèle de Claude qui lit les plans, sauf réglage contraire (Préférences).
pub const MODELE_CLAUDE: &str = "claude-sonnet-5-5";
/// La version de l'API de Claude que la requête annonce.
pub const VERSION_API_CLAUDE: &str = "2023-06-01";
/// Les mots qu'un modèle peut écrire avant de s'arrêter : une lecture de plan en tient dans 2 000.
const JETONS_MAX: u32 = 2_048;
/// Le plus long bruitage d'ElevenLabs, en secondes.
pub const ELEVENLABS_SON_MAX_S: f32 = 22.0;
/// Le plus court.
pub const ELEVENLABS_SON_MIN_S: f32 = 0.5;
/// Les durées de Stable Audio (secondes).
pub const STABLE_AUDIO_MIN_S: f32 = 1.0;
pub const STABLE_AUDIO_MAX_S: f32 = 190.0;

// ------------------------------------------------------------------------------------ transport

/// Une requête HTTPS prête à partir.
#[derive(Clone, PartialEq, Eq)]
pub struct RequeteHttps {
    pub methode: &'static str,
    pub url: String,
    /// Noms en minuscules.
    pub entetes: Vec<(String, String)>,
    pub corps: Vec<u8>,
}

impl RequeteHttps {
    #[cfg(test)]
    pub fn entete(&self, nom: &str) -> Option<&str> {
        self.entetes.iter().find(|(n, _)| n == nom).map(|(_, v)| v.as_str())
    }
}

/// Les en-têtes qui portent une clé : jamais montrés.
const SECRETS: &[&str] = &["authorization", "x-api-key", "xi-api-key", "api-key"];

impl std::fmt::Debug for RequeteHttps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let entetes: Vec<String> = self.entetes.iter().map(|(n, v)| if SECRETS.contains(&n.as_str()) { format!("{n}: <masqué>") } else { format!("{n}: {v}") }).collect();
        write!(f, "{} {} [{}] ({} octets)", self.methode, self.url, entetes.join(", "), self.corps.len())
    }
}

/// Ce qui envoie une requête HTTPS (TLS compris) et rend la réponse.
pub trait Transport: Send + Sync {
    fn envoyer(&self, requete: &RequeteHttps, controle: &Controle) -> Result<ReponseHttp, ErreurGen>;
}

/// Le transport de cette version : aucun. Chaque appel dit pourquoi.
pub struct TransportABrancher;

pub const MESSAGE_A_BRANCHER: &str = "cette version de FEANOR ne sait pas encore envoyer de requête aux services en ligne (aucun module TLS \
dans la compilation hors ligne). Les requêtes sont prêtes ; en attendant, utilise le serveur local ou la lecture par règles.";

impl Transport for TransportABrancher {
    fn envoyer(&self, _: &RequeteHttps, controle: &Controle) -> Result<ReponseHttp, ErreurGen> {
        controle.verifier()?;
        Err(ErreurGen::ABrancher(MESSAGE_A_BRANCHER.into()))
    }
}

/// La clé du service, lue dans le coffre au moment de l'appel.
fn cle_de(coffre: &dyn Coffre, service: Service) -> Result<Cle, ErreurGen> {
    match coffre.lire(service) {
        Ok(Some(c)) if !c.est_vide() => Ok(c),
        Ok(_) => Err(ErreurGen::CleManquante(format!("aucune clé pour {} (Fichier > Préférences > Génération sonore > Saisir la clé…)", service.court()))),
        Err(e) => Err(ErreurGen::Indisponible(format!("le coffre des clés ne répond pas ({e})"))),
    }
}

/// Ce que le service a dit de son refus : son message, borné.
fn message_service(r: &ReponseHttp) -> String {
    let v: Value = serde_json::from_slice(&r.corps).unwrap_or(Value::Null);
    let brut = v["error"]["message"].as_str().or_else(|| v["detail"]["message"].as_str()).or_else(|| v["detail"].as_str()).or_else(|| v["message"].as_str()).or_else(|| v["errors"][0].as_str()).unwrap_or("");
    brut.chars().take(300).collect()
}

/// L'erreur d'une réponse qui n'est pas un succès.
pub fn erreur_service(service: Service, r: &ReponseHttp) -> ErreurGen {
    let detail = message_service(r);
    let avec = |texte: &str| if detail.is_empty() { texte.to_string() } else { format!("{texte} ({detail})") };
    match r.statut {
        401 | 403 => ErreurGen::Refuse(avec(&format!("la clé de {} est refusée", service.court()))),
        402 => ErreurGen::Refuse(avec(&format!("le compte {} n'a plus de crédit", service.court()))),
        404 => ErreurGen::Invalide(avec(&format!("{} ne connaît pas ce modèle ou cette voix", service.court()))),
        400 | 413 | 422 => ErreurGen::Invalide(avec(&format!("{} refuse la demande", service.court()))),
        429 => ErreurGen::Refuse(avec(&format!("{} limite les demandes (quota ou trop de demandes à la fois)", service.court()))),
        500..=599 => ErreurGen::Indisponible(avec(&format!("{} a un problème ({})", service.court(), r.statut))),
        s => ErreurGen::Invalide(avec(&format!("réponse inattendue de {} ({s})", service.court()))),
    }
}

fn executer(transport: &dyn Transport, requete: &RequeteHttps, controle: &Controle, service: Service) -> Result<ReponseHttp, ErreurGen> {
    controle.verifier()?;
    let r = transport.envoyer(requete, controle)?;
    if r.reussie() {
        Ok(r)
    } else {
        Err(erreur_service(service, &r))
    }
}

/// La requête du bouton « Tester la connexion » : une lecture qui ne coûte rien et ne dit rien du projet
/// (la liste des modèles de Claude, le compte ElevenLabs, le solde de Stability).
pub fn requete_test(service: Service, cle: &Cle) -> RequeteHttps {
    let (url, entete) = match service {
        Service::Claude => ("https://api.anthropic.com/v1/models", ("x-api-key".to_string(), cle.voir().to_string())),
        Service::ElevenLabs => ("https://api.elevenlabs.io/v1/user", ("xi-api-key".to_string(), cle.voir().to_string())),
        Service::StableAudio => ("https://api.stability.ai/v1/user/balance", ("authorization".to_string(), format!("Bearer {}", cle.voir()))),
    };
    let mut entetes = vec![entete, ("accept".to_string(), "application/json".to_string())];
    if service == Service::Claude {
        entetes.push(("anthropic-version".into(), VERSION_API_CLAUDE.into()));
    }
    RequeteHttps { methode: "GET", url: url.into(), entetes, corps: Vec::new() }
}

/// Teste la connexion à `service` : une phrase pour la fenêtre des Préférences.
pub fn tester(service: Service, coffre: &dyn Coffre, transport: &dyn Transport, controle: &Controle) -> String {
    let cle = match cle_de(coffre, service) {
        Ok(c) => c,
        Err(e) => return format!("{e}."),
    };
    let requete = requete_test(service, &cle);
    drop(cle);
    match executer(transport, &requete, controle, service) {
        Ok(_) => format!("Connexion à {} réussie : la clé est acceptée.", service.court()),
        Err(ErreurGen::ABrancher(m)) => format!("Clé enregistrée pour {}, mais {m}", service.court()),
        Err(e) => format!("Connexion à {} impossible : {e}.", service.court()),
    }
}

// --------------------------------------------------------------------------------------- Claude

/// Rôle 1 en ligne : Claude lit un plan (vignette et textes) et rend des indications en JSON.
pub struct LecteurClaude {
    pub transport: Arc<dyn Transport>,
    pub coffre: Arc<dyn Coffre>,
    pub modele: String,
}

/// La requête qui fait lire le plan `index` à Claude.
pub fn requete_claude(demande: &DemandeLecture, index: usize, modele: &str, cle: &Cle) -> RequeteHttps {
    let plan = &demande.plans[index];
    let mut contenu = Vec::new();
    if let Some(png) = &plan.vignette_png {
        contenu.push(json!({"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": base64::encode(png)}}));
    }
    contenu.push(json!({"type": "text", "text": serde_json::to_string_pretty(&lecturesb::plan_en_json(demande, index)).unwrap_or_default()}));
    let corps = json!({
        "model": modele,
        "max_tokens": JETONS_MAX,
        "system": lecturesb::CONSIGNES,
        "messages": [{"role": "user", "content": contenu}],
    });
    RequeteHttps {
        methode: "POST",
        url: "https://api.anthropic.com/v1/messages".into(),
        entetes: vec![
            ("content-type".into(), "application/json".into()),
            ("x-api-key".into(), cle.voir().to_string()),
            ("anthropic-version".into(), VERSION_API_CLAUDE.into()),
        ],
        corps: serde_json::to_vec(&corps).unwrap_or_default(),
    }
}

/// Les indications d'une réponse de l'API de Claude.
pub fn indications_de_reponse_claude(r: &ReponseHttp, frames: usize) -> Result<Vec<IndicationBrute>, ErreurGen> {
    let v: Value = serde_json::from_slice(&r.corps).map_err(|e| ErreurGen::Invalide(format!("réponse de Claude illisible ({e})")))?;
    if v["stop_reason"].as_str() == Some("max_tokens") {
        return Err(ErreurGen::Invalide("la réponse de Claude a été coupée (trop longue)".into()));
    }
    let texte: String = v["content"].as_array().map(|blocs| blocs.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n")).unwrap_or_default();
    lecturesb::indications_depuis_texte(&texte, frames)
}

impl LecteurStoryboard for LecteurClaude {
    fn nom(&self) -> String {
        format!("Claude ({})", self.modele)
    }

    fn envoie_en_ligne(&self) -> Option<String> {
        Some("les vignettes et les textes des plans (action, dialogue, notes, commentaires) partent vers l'API de Claude (Anthropic)".into())
    }

    fn lire_plan(&self, demande: &DemandeLecture, index: usize, controle: &Controle) -> Result<Vec<IndicationBrute>, ErreurGen> {
        controle.verifier()?;
        let plan = demande.plans.get(index).ok_or_else(|| ErreurGen::Invalide("plan inconnu".into()))?;
        let cle = cle_de(self.coffre.as_ref(), Service::Claude)?;
        let requete = requete_claude(demande, index, &self.modele, &cle);
        drop(cle);
        let r = executer(self.transport.as_ref(), &requete, controle, Service::Claude)?;
        indications_de_reponse_claude(&r, plan.frames)
    }
}

// ----------------------------------------------------------------------------------- ElevenLabs

/// Rôle 2 en ligne : ElevenLabs Sound Effects.
pub struct SonElevenLabs {
    pub transport: Arc<dyn Transport>,
    pub coffre: Arc<dyn Coffre>,
}

/// La requête d'un bruitage (texte anglais, durée de 0,5 à 22 s).
pub fn requete_son_elevenlabs(demande: &DemandeSon, cle: &Cle) -> RequeteHttps {
    let corps = json!({
        "text": demande.prompt,
        "duration_seconds": demande.secondes.clamp(ELEVENLABS_SON_MIN_S, ELEVENLABS_SON_MAX_S),
        "prompt_influence": 0.3,
    });
    RequeteHttps {
        methode: "POST",
        url: "https://api.elevenlabs.io/v1/sound-generation?output_format=mp3_44100_128".into(),
        entetes: vec![("content-type".into(), "application/json".into()), ("xi-api-key".into(), cle.voir().to_string()), ("accept".into(), "audio/mpeg".into())],
        corps: serde_json::to_vec(&corps).unwrap_or_default(),
    }
}

/// Le son d'une réponse d'ElevenLabs ou de Stable Audio : des octets MP3.
pub fn son_de_reponse(r: &ReponseHttp) -> Result<SonGenere, ErreurGen> {
    if r.corps.is_empty() {
        return Err(ErreurGen::Invalide("le service a rendu un son vide".into()));
    }
    let type_contenu = r.entete("content-type").unwrap_or("").to_ascii_lowercase();
    if type_contenu.starts_with("application/json") {
        return Err(ErreurGen::Invalide(format!("le service a rendu du texte au lieu d'un son ({})", message_service(r))));
    }
    let ext = if type_contenu.contains("wav") { "wav" } else if type_contenu.contains("flac") { "flac" } else if type_contenu.contains("ogg") { "ogg" } else { "mp3" };
    Ok(SonGenere { octets: r.corps.clone(), ext: ext.into() })
}

impl GenerateurSon for SonElevenLabs {
    fn nom(&self) -> String {
        "ElevenLabs Sound Effects".into()
    }

    fn envoie_en_ligne(&self) -> Option<String> {
        Some("les descriptions des sons partent vers ElevenLabs".into())
    }

    fn generer(&self, demande: &DemandeSon, controle: &Controle) -> Result<SonGenere, ErreurGen> {
        controle.verifier()?;
        let cle = cle_de(self.coffre.as_ref(), Service::ElevenLabs)?;
        let requete = requete_son_elevenlabs(demande, &cle);
        drop(cle);
        son_de_reponse(&executer(self.transport.as_ref(), &requete, controle, Service::ElevenLabs)?)
    }
}

/// Rôle 3 en ligne : ElevenLabs Text to Speech.
pub struct VoixElevenLabs {
    pub transport: Arc<dyn Transport>,
    pub coffre: Arc<dyn Coffre>,
}

/// La requête d'une voix. `demande.voix` est l'identifiant d'une voix d'ElevenLabs (lettres, chiffres,
/// « - » et « _ » : il entre dans l'adresse).
pub fn requete_voix_elevenlabs(demande: &DemandeVoix, cle: &Cle) -> Result<RequeteHttps, ErreurGen> {
    let voix = demande.voix.trim();
    if voix.is_empty() {
        return Err(ErreurGen::Invalide("aucune voix choisie : indique l'identifiant d'une voix ElevenLabs (Préférences > Génération sonore, ou « Modifier » sur l'indication)".into()));
    }
    if !voix.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(ErreurGen::Invalide("l'identifiant de la voix ne peut contenir que des lettres, des chiffres, « - » et « _ »".into()));
    }
    let corps = json!({"text": demande.texte, "model_id": "eleven_multilingual_v2"});
    Ok(RequeteHttps {
        methode: "POST",
        url: format!("https://api.elevenlabs.io/v1/text-to-speech/{voix}?output_format=mp3_44100_128"),
        entetes: vec![("content-type".into(), "application/json".into()), ("xi-api-key".into(), cle.voir().to_string()), ("accept".into(), "audio/mpeg".into())],
        corps: serde_json::to_vec(&corps).unwrap_or_default(),
    })
}

impl GenerateurVoix for VoixElevenLabs {
    fn nom(&self) -> String {
        "ElevenLabs Text to Speech".into()
    }

    fn envoie_en_ligne(&self) -> Option<String> {
        Some("les répliques des dialogues partent vers ElevenLabs".into())
    }

    fn generer(&self, demande: &DemandeVoix, controle: &Controle) -> Result<SonGenere, ErreurGen> {
        controle.verifier()?;
        let cle = cle_de(self.coffre.as_ref(), Service::ElevenLabs)?;
        let requete = requete_voix_elevenlabs(demande, &cle)?;
        drop(cle);
        son_de_reponse(&executer(self.transport.as_ref(), &requete, controle, Service::ElevenLabs)?)
    }
}

// ------------------------------------------------------------------------------- Stable Audio

/// Rôle 2 en ligne : l'API de Stable Audio (Stability AI).
pub struct SonStableAudio {
    pub transport: Arc<dyn Transport>,
    pub coffre: Arc<dyn Coffre>,
}

const FRONTIERE: &str = "----shot-formulaire-3f9c1a7d";

/// Un formulaire `multipart/form-data` fait de champs texte.
pub fn formulaire(champs: &[(&str, String)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (nom, valeur) in champs {
        out.extend_from_slice(format!("--{FRONTIERE}\r\nContent-Disposition: form-data; name=\"{nom}\"\r\n\r\n{valeur}\r\n").as_bytes());
    }
    out.extend_from_slice(format!("--{FRONTIERE}--\r\n").as_bytes());
    out
}

/// La requête d'un son chez Stable Audio (formulaire : texte, durée, MP3).
pub fn requete_son_stable_audio(demande: &DemandeSon, cle: &Cle) -> RequeteHttps {
    let secondes = demande.secondes.clamp(STABLE_AUDIO_MIN_S, STABLE_AUDIO_MAX_S);
    RequeteHttps {
        methode: "POST",
        url: "https://api.stability.ai/v2beta/audio/stable-audio-2/text-to-audio".into(),
        entetes: vec![
            ("authorization".into(), format!("Bearer {}", cle.voir())),
            ("content-type".into(), format!("multipart/form-data; boundary={FRONTIERE}")),
            ("accept".into(), "audio/*".into()),
        ],
        corps: formulaire(&[("prompt", demande.prompt.clone()), ("duration", format!("{secondes:.1}")), ("output_format", "mp3".into())]),
    }
}

impl GenerateurSon for SonStableAudio {
    fn nom(&self) -> String {
        "Stable Audio (API)".into()
    }

    fn envoie_en_ligne(&self) -> Option<String> {
        Some("les descriptions des sons partent vers l'API de Stable Audio (Stability AI)".into())
    }

    fn generer(&self, demande: &DemandeSon, controle: &Controle) -> Result<SonGenere, ErreurGen> {
        controle.verifier()?;
        let cle = cle_de(self.coffre.as_ref(), Service::StableAudio)?;
        let requete = requete_son_stable_audio(demande, &cle);
        drop(cle);
        son_de_reponse(&executer(self.transport.as_ref(), &requete, controle, Service::StableAudio)?)
    }
}
