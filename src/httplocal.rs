//! Un client HTTP/1.1 minimal, **pour cet ordinateur seulement** (Anthony, 2026-10-19) : il parle au
//! serveur local de génération sonore (`sidecar.rs`) sur `127.0.0.1`. Aucune caisse HTTP n'est dans le
//! cache hors ligne, et le local n'a pas besoin de TLS : un `TcpStream`, une requête, une réponse.
//!
//! - **Seul le local** : une adresse qui n'est pas de boucle locale (`127.0.0.1`, `::1`) est refusée
//!   avant toute connexion. Rien de ce qu'on envoie ici ne quitte l'ordinateur.
//! - **Annulable** : la lecture se fait par tranches de 100 ms ; dès que `Controle` est annulé, la
//!   connexion est fermée (le serveur y voit l'arrêt) et l'appel rend `Annule`.
//! - **Borné** : délai total, taille de réponse (128 Mo).
//! - **Comprend** les réponses à `Content-Length`, `Transfer-Encoding: chunked` et à fermeture de
//!   connexion. Ne gère ni les redirections, ni les connexions réutilisées (`Connection: close`).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use crate::fournisseurs::{Controle, ErreurGen};

/// La plus grande réponse acceptée, en octets.
pub const MAX_REPONSE: usize = 128 << 20;
/// Le temps qu'on attend la connexion (un serveur local répond tout de suite, ou pas du tout).
const DELAI_CONNEXION: Duration = Duration::from_secs(2);
/// La durée d'une tranche de lecture entre deux vérifications de l'annulation.
const TRANCHE: Duration = Duration::from_millis(100);

/// Une réponse HTTP.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReponseHttp {
    pub statut: u16,
    /// Noms en minuscules.
    pub entetes: Vec<(String, String)>,
    pub corps: Vec<u8>,
}

impl ReponseHttp {
    pub fn entete(&self, nom: &str) -> Option<&str> {
        self.entetes.iter().find(|(n, _)| n == nom).map(|(_, v)| v.as_str())
    }

    pub fn reussie(&self) -> bool {
        (200..300).contains(&self.statut)
    }
}

/// Envoie une requête à `adresse` (de boucle locale) et rend la réponse. `corps` : le type de contenu
/// et les octets. `delai` : le temps total accordé.
pub fn requete(adresse: SocketAddr, methode: &str, chemin: &str, corps: Option<(&str, &[u8])>, controle: &Controle, delai: Duration) -> Result<ReponseHttp, ErreurGen> {
    if !adresse.ip().is_loopback() {
        return Err(ErreurGen::Invalide(format!("{adresse} n'est pas une adresse locale : le serveur de génération sonore doit tourner sur cet ordinateur")));
    }
    controle.verifier()?;
    let debut = Instant::now();
    let mut flux = TcpStream::connect_timeout(&adresse, DELAI_CONNEXION).map_err(|e| match e.kind() {
        std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::TimedOut => ErreurGen::NonInstalle(format!("aucun serveur local n'écoute sur {adresse}")),
        _ => ErreurGen::Indisponible(format!("connexion à {adresse} impossible ({e})")),
    })?;
    let _ = flux.set_nodelay(true);
    let _ = flux.set_write_timeout(Some(Duration::from_secs(10)));
    let _ = flux.set_read_timeout(Some(TRANCHE));
    let mut tete = format!("{methode} {chemin} HTTP/1.1\r\nHost: {adresse}\r\nConnection: close\r\nAccept: application/json, audio/*\r\nUser-Agent: SHOT\r\n");
    if let Some((type_contenu, octets)) = corps {
        tete.push_str(&format!("Content-Type: {type_contenu}\r\nContent-Length: {}\r\n", octets.len()));
    } else if methode != "GET" {
        tete.push_str("Content-Length: 0\r\n");
    }
    tete.push_str("\r\n");
    let envoi = flux.write_all(tete.as_bytes()).and_then(|_| match corps {
        Some((_, octets)) => flux.write_all(octets),
        None => Ok(()),
    });
    envoi.map_err(|e| ErreurGen::Indisponible(format!("envoi impossible ({e})")))?;
    let mut recu: Vec<u8> = Vec::new();
    let mut tranche = vec![0u8; 64 * 1024];
    loop {
        // (Abandonner la connexion, c'est l'annuler : `flux` se ferme en sortant.)
        controle.verifier()?;
        if debut.elapsed() > delai {
            return Err(ErreurGen::Indisponible(format!("pas de réponse de {adresse} en {} s", delai.as_secs())));
        }
        match flux.read(&mut tranche) {
            Ok(0) => {
                return match analyser(&recu, true) {
                    Some(r) => r,
                    None => Err(ErreurGen::Indisponible("le serveur a fermé la connexion sans répondre".into())),
                };
            }
            Ok(n) => {
                recu.extend_from_slice(&tranche[..n]);
                if recu.len() > MAX_REPONSE {
                    return Err(ErreurGen::Invalide("réponse trop grosse".into()));
                }
                if let Some(r) = analyser(&recu, false) {
                    return r;
                }
            }
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut | std::io::ErrorKind::Interrupted) => {}
            Err(e) => return Err(ErreurGen::Indisponible(format!("lecture impossible ({e})"))),
        }
    }
}

fn trouver(haystack: &[u8], aiguille: &[u8]) -> Option<usize> {
    haystack.windows(aiguille.len()).position(|w| w == aiguille)
}

/// Lit ce qui est arrivé : `None` tant que la réponse n'est pas complète. `fin` : le serveur a fermé
/// la connexion (une réponse sans longueur est alors complète).
fn analyser(recu: &[u8], fin: bool) -> Option<Result<ReponseHttp, ErreurGen>> {
    // (L'en-tête tient dans les premiers 64 Ko : inutile de chercher plus loin à chaque tranche reçue.)
    let coupe = trouver(&recu[..recu.len().min(64 * 1024)], b"\r\n\r\n")?;
    let tete = String::from_utf8_lossy(&recu[..coupe]).into_owned();
    let corps = &recu[coupe + 4..];
    let mut lignes = tete.split("\r\n");
    let statut = lignes.next().unwrap_or("");
    let mut mots = statut.split_whitespace();
    let (Some(version), Some(code)) = (mots.next(), mots.next()) else {
        return Some(Err(ErreurGen::Invalide("réponse HTTP illisible".into())));
    };
    if !version.starts_with("HTTP/") {
        return Some(Err(ErreurGen::Invalide("ce n'est pas un serveur HTTP".into())));
    }
    let Ok(statut) = code.parse::<u16>() else {
        return Some(Err(ErreurGen::Invalide("réponse HTTP illisible".into())));
    };
    let entetes: Vec<(String, String)> = lignes.filter_map(|l| l.split_once(':')).map(|(n, v)| (n.trim().to_ascii_lowercase(), v.trim().to_string())).collect();
    let valeur = |nom: &str| entetes.iter().find(|(n, _)| n == nom).map(|(_, v)| v.as_str());
    let complete = |corps: Vec<u8>| Some(Ok(ReponseHttp { statut, entetes: entetes.clone(), corps }));
    if statut == 204 || statut == 304 || (100..200).contains(&statut) {
        return complete(Vec::new());
    }
    if valeur("transfer-encoding").is_some_and(|v| v.to_ascii_lowercase().contains("chunked")) {
        return match decoder_morceaux(corps) {
            Some(Ok(c)) => complete(c),
            Some(Err(e)) => Some(Err(e)),
            None if fin => Some(Err(ErreurGen::Indisponible("réponse tronquée".into()))),
            None => None,
        };
    }
    if let Some(longueur) = valeur("content-length") {
        let Ok(n) = longueur.parse::<usize>() else {
            return Some(Err(ErreurGen::Invalide("Content-Length illisible".into())));
        };
        if n > MAX_REPONSE {
            return Some(Err(ErreurGen::Invalide("réponse trop grosse".into())));
        }
        return if corps.len() >= n {
            complete(corps[..n].to_vec())
        } else if fin {
            Some(Err(ErreurGen::Indisponible("réponse tronquée".into())))
        } else {
            None
        };
    }
    fin.then(|| ReponseHttp { statut, entetes: entetes.clone(), corps: corps.to_vec() }).map(Ok)
}

/// Le corps d'une réponse « chunked » : `None` tant que le dernier morceau (taille 0) n'est pas là.
fn decoder_morceaux(mut corps: &[u8]) -> Option<Result<Vec<u8>, ErreurGen>> {
    let mut out = Vec::new();
    loop {
        let fin_ligne = trouver(corps, b"\r\n")?;
        let ligne = String::from_utf8_lossy(&corps[..fin_ligne]);
        let taille = ligne.split(';').next().unwrap_or("").trim();
        let Ok(n) = usize::from_str_radix(taille, 16) else {
            return Some(Err(ErreurGen::Invalide("morceau de réponse illisible".into())));
        };
        corps = &corps[fin_ligne + 2..];
        if n == 0 {
            return Some(Ok(out));
        }
        if corps.len() < n + 2 {
            return None;
        }
        out.extend_from_slice(&corps[..n]);
        if out.len() > MAX_REPONSE {
            return Some(Err(ErreurGen::Invalide("réponse trop grosse".into())));
        }
        corps = &corps[n + 2..];
    }
}
