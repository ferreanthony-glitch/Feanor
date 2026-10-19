//! Les clés des services en ligne (Anthony, 2026-10-19) : elles sont saisies dans Fichier >
//! Préférences > Génération sonore et rangées dans le **Gestionnaire d'identifiants de Windows**
//! (Credential Manager), jamais dans un fichier de SHOT : ni `reglages.toml`, ni le projet, ni un
//! rapport d'erreur. Une clé ne s'affiche pas, ne s'écrit dans aucun journal (`Cle` se montre masquée,
//! même dans `{:?}`), et elle est lue au moment de l'appel seulement, sur le fil de travail.
//!
//! L'accès à Windows est déclaré à la main (`advapi32` : `CredWriteW`, `CredReadW`, `CredDeleteW`),
//! comme `materiel.rs` le fait pour `kernel32` : aucune caisse de plus. Les essais utilisent
//! `CoffreMemoire`, qui ne touche pas au gestionnaire d'identifiants.

#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// Un service en ligne qui demande une clé.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Service {
    /// L'API de Claude (Anthropic) : lire le storyboard.
    #[serde(rename = "claude")]
    Claude,
    /// ElevenLabs : bruitages (Sound Effects) et voix (TTS).
    #[serde(rename = "elevenlabs")]
    ElevenLabs,
    /// L'API de Stable Audio (Stability AI) : bruitages et ambiances.
    #[serde(rename = "stableaudio")]
    StableAudio,
}

impl Service {
    pub const ALL: [Service; 3] = [Service::Claude, Service::ElevenLabs, Service::StableAudio];

    pub fn nom(self) -> &'static str {
        match self {
            Service::Claude => "Claude (Anthropic)",
            Service::ElevenLabs => "ElevenLabs",
            Service::StableAudio => "Stable Audio (Stability AI)",
        }
    }

    /// Le nom court, pour les messages (« la clé de ElevenLabs »).
    pub fn court(self) -> &'static str {
        match self {
            Service::Claude => "Claude",
            Service::ElevenLabs => "ElevenLabs",
            Service::StableAudio => "Stable Audio",
        }
    }

    fn cible(self) -> &'static str {
        match self {
            Service::Claude => "claude",
            Service::ElevenLabs => "elevenlabs",
            Service::StableAudio => "stableaudio",
        }
    }
}

/// Une clé : ses octets sont effacés quand elle disparaît, et elle ne se montre jamais.
pub struct Cle {
    octets: Vec<u8>,
}

impl Cle {
    pub fn new(texte: &str) -> Cle {
        Cle { octets: texte.trim().as_bytes().to_vec() }
    }

    /// Le texte de la clé, pour l'en-tête d'une requête : à ne pas garder.
    pub fn voir(&self) -> &str {
        std::str::from_utf8(&self.octets).unwrap_or("")
    }

    pub fn est_vide(&self) -> bool {
        self.octets.is_empty()
    }
}

impl Drop for Cle {
    fn drop(&mut self) {
        for b in &mut self.octets {
            // (Écriture volatile : le compilateur ne retire pas l'effacement d'un tampon qu'on lâche.)
            // SAFETY : `b` est une référence valide vers un octet du tampon.
            unsafe { std::ptr::write_volatile(b, 0) };
        }
    }
}

impl std::fmt::Debug for Cle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cle(<masquée, {} octets>)", self.octets.len())
    }
}

/// Où les clés sont rangées. `Send + Sync` : le fil de travail la lit au moment de l'appel.
pub trait Coffre: Send + Sync {
    /// La clé du service, s'il y en a une.
    fn lire(&self, service: Service) -> Result<Option<Cle>, String>;
    fn ecrire(&self, service: Service, cle: &Cle) -> Result<(), String>;
    fn effacer(&self, service: Service) -> Result<(), String>;

    fn presente(&self, service: Service) -> bool {
        self.lire(service).is_ok_and(|c| c.is_some_and(|c| !c.est_vide()))
    }
}

/// Le coffre des essais, en mémoire.
#[cfg(test)]
#[derive(Default)]
pub struct CoffreMemoire {
    cles: Mutex<HashMap<Service, String>>,
}

#[cfg(test)]
impl Coffre for CoffreMemoire {
    fn lire(&self, service: Service) -> Result<Option<Cle>, String> {
        Ok(self.cles.lock().map_err(|_| "coffre indisponible")?.get(&service).map(|t| Cle::new(t)))
    }

    fn ecrire(&self, service: Service, cle: &Cle) -> Result<(), String> {
        self.cles.lock().map_err(|_| "coffre indisponible")?.insert(service, cle.voir().to_string());
        Ok(())
    }

    fn effacer(&self, service: Service) -> Result<(), String> {
        self.cles.lock().map_err(|_| "coffre indisponible")?.remove(&service);
        Ok(())
    }
}

/// Le Gestionnaire d'identifiants de Windows : un identifiant « SHOT/<service> » par service, rangé
/// pour cet utilisateur de cet ordinateur.
pub struct CoffreWindows {
    prefixe: String,
}

impl CoffreWindows {
    pub fn new() -> CoffreWindows {
        CoffreWindows { prefixe: "SHOT".into() }
    }

    /// Avec un autre préfixe de nom (les essais ne touchent pas aux vraies clés de l'utilisateur).
    #[cfg(test)]
    pub fn avec_prefixe(prefixe: &str) -> CoffreWindows {
        CoffreWindows { prefixe: prefixe.into() }
    }

    fn cible(&self, service: Service) -> String {
        format!("{}/{}", self.prefixe, service.cible())
    }
}

impl Default for CoffreWindows {
    fn default() -> Self {
        Self::new()
    }
}

impl Coffre for CoffreWindows {
    fn lire(&self, service: Service) -> Result<Option<Cle>, String> {
        sys::lire(&self.cible(service))
    }

    fn ecrire(&self, service: Service, cle: &Cle) -> Result<(), String> {
        sys::ecrire(&self.cible(service), cle.voir().as_bytes())
    }

    fn effacer(&self, service: Service) -> Result<(), String> {
        sys::effacer(&self.cible(service))
    }
}

#[cfg(windows)]
mod sys {
    use super::Cle;
    use std::ffi::c_void;

    /// CREDENTIALW.
    #[repr(C)]
    struct Credential {
        flags: u32,
        kind: u32,
        target_name: *mut u16,
        comment: *mut u16,
        last_written: [u32; 2],
        blob_size: u32,
        blob: *mut u8,
        persist: u32,
        attribute_count: u32,
        attributes: *mut c_void,
        target_alias: *mut u16,
        user_name: *mut u16,
    }

    #[link(name = "advapi32")]
    extern "system" {
        fn CredWriteW(credential: *const Credential, flags: u32) -> i32;
        fn CredReadW(target_name: *const u16, kind: u32, flags: u32, credential: *mut *mut Credential) -> i32;
        fn CredDeleteW(target_name: *const u16, kind: u32, flags: u32) -> i32;
        fn CredFree(buffer: *mut c_void);
    }

    const CRED_TYPE_GENERIC: u32 = 1;
    /// Rangé pour cet utilisateur sur cet ordinateur (ne suit pas le profil itinérant).
    const CRED_PERSIST_LOCAL_MACHINE: u32 = 2;
    /// Taille la plus grande d'un secret (CRED_MAX_CREDENTIAL_BLOB_SIZE).
    const MAX_BLOB: usize = 2560;
    const ERROR_NOT_FOUND: i32 = 1168;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn ecrire(cible: &str, octets: &[u8]) -> Result<(), String> {
        if octets.is_empty() {
            return Err("la clé est vide".into());
        }
        if octets.len() > MAX_BLOB {
            return Err(format!("la clé est trop longue ({} octets, au plus {MAX_BLOB})", octets.len()));
        }
        let mut nom = wide(cible);
        let mut utilisateur = wide("SHOT");
        let mut secret = octets.to_vec();
        let c = Credential {
            flags: 0,
            kind: CRED_TYPE_GENERIC,
            target_name: nom.as_mut_ptr(),
            comment: std::ptr::null_mut(),
            last_written: [0; 2],
            blob_size: secret.len() as u32,
            blob: secret.as_mut_ptr(),
            persist: CRED_PERSIST_LOCAL_MACHINE,
            attribute_count: 0,
            attributes: std::ptr::null_mut(),
            target_alias: std::ptr::null_mut(),
            user_name: utilisateur.as_mut_ptr(),
        };
        // SAFETY : `c` est un CREDENTIALW complet dont les pointeurs visent des tampons vivants
        // (terminés par zéro pour les noms) pendant tout l'appel ; Windows en garde sa propre copie.
        let ok = unsafe { CredWriteW(&c, 0) };
        let erreur = std::io::Error::last_os_error();
        secret.iter_mut().for_each(|b| *b = 0);
        if ok == 0 {
            return Err(format!("le Gestionnaire d'identifiants de Windows refuse ({erreur})"));
        }
        Ok(())
    }

    pub fn lire(cible: &str) -> Result<Option<Cle>, String> {
        let nom = wide(cible);
        let mut sortie: *mut Credential = std::ptr::null_mut();
        // SAFETY : `nom` est terminé par zéro ; `sortie` reçoit un tampon que `CredFree` libère plus bas.
        let ok = unsafe { CredReadW(nom.as_ptr(), CRED_TYPE_GENERIC, 0, &mut sortie) };
        if ok == 0 {
            let e = std::io::Error::last_os_error();
            return if e.raw_os_error() == Some(ERROR_NOT_FOUND) { Ok(None) } else { Err(format!("lecture impossible dans le Gestionnaire d'identifiants ({e})")) };
        }
        // SAFETY : l'appel a réussi, `sortie` vise un CREDENTIALW valide dont le secret a `blob_size`
        // octets ; il est copié avant d'être libéré.
        let cle = unsafe {
            let c = &*sortie;
            let octets = if c.blob.is_null() || c.blob_size == 0 { Vec::new() } else { std::slice::from_raw_parts(c.blob, c.blob_size as usize).to_vec() };
            CredFree(sortie as *mut c_void);
            octets
        };
        Ok(Some(Cle { octets: cle }))
    }

    pub fn effacer(cible: &str) -> Result<(), String> {
        let nom = wide(cible);
        // SAFETY : `nom` est terminé par zéro.
        let ok = unsafe { CredDeleteW(nom.as_ptr(), CRED_TYPE_GENERIC, 0) };
        if ok == 0 {
            let e = std::io::Error::last_os_error();
            if e.raw_os_error() != Some(ERROR_NOT_FOUND) {
                return Err(format!("effacement impossible dans le Gestionnaire d'identifiants ({e})"));
            }
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod sys {
    use super::Cle;

    const SANS: &str = "le Gestionnaire d'identifiants n'existe que sous Windows";

    pub fn ecrire(_: &str, _: &[u8]) -> Result<(), String> {
        Err(SANS.into())
    }

    pub fn lire(_: &str) -> Result<Option<Cle>, String> {
        Err(SANS.into())
    }

    pub fn effacer(_: &str) -> Result<(), String> {
        Err(SANS.into())
    }
}
