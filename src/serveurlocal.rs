//! Le serveur local de génération sonore, vu de SHOT (Anthony, 2026-10-19) : où il est, comment SHOT le
//! **lance** (un processus enfant, sans fenêtre de console), l'**attend** et l'**arrête**.
//!
//! - Le serveur est le script `sidecar/serveur.py` (voir `docs/GENERATION_SON.md`), lancé avec le Python du
//!   venv de `SHOT-modeles` (`<racine>\venv\Scripts\python.exe -I serveur.py --racine <racine> --port N`).
//!   Le Python et le dossier des modèles sont des **réglages** ; par défaut `Documents\SHOT\Modèles\SHOT-modeles`
//!   (l'ancien `Documents\Work\SHOT-modeles` s'il est le seul des deux à exister).
//!   Aucun chemin d'utilisateur n'est écrit dans le code : le dossier Documents vient de l'environnement.
//! - **Rien ne bloque le fil de l'interface** : démarrer, attendre que le serveur réponde et arrêter se font
//!   sur des fils de travail (`genprefsui.rs`, `sidecar.rs`). Charger un modèle ne bloque pas non plus :
//!   le serveur répond `/v1/etat` pendant qu'il charge, et SHOT n'attend que dans son fil de travail.
//! - **Un serveur lancé par SHOT s'arrête avec SHOT** : le processus est placé dans un « objet de travail »
//!   Windows qui le tue (et tue le Python de base que le lanceur du venv démarre) dès que SHOT se ferme,
//!   même s'il plante ; en plus, le serveur surveille le numéro de processus de SHOT. Un serveur lancé
//!   à la main n'est jamais arrêté par SHOT.
//! - Le lancement réel passe par un `Lanceur` : les essais en glissent un faux, sans Python.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::fournisseurs::{Controle, ErreurGen};
use crate::genreglages::GenerationSonReglages;
use crate::sidecar::{ClientSidecar, EtatSidecar};

/// Le temps qu'on laisse au serveur pour répondre après son lancement.
pub const DELAI_DEMARRAGE: Duration = Duration::from_secs(120);

// ------------------------------------------------------------------------------------ la configuration

/// Tout ce qu'il faut pour lancer le serveur.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigServeur {
    pub python: PathBuf,
    pub racine: PathBuf,
    pub script: PathBuf,
    pub port: u16,
}

/// `Documents` de l'utilisateur (d'après son profil), sinon son dossier personnel.
fn documents() -> PathBuf {
    crate::nouveau::default_folder()
}

/// Le dossier `SHOT-modeles` par défaut : `Documents\SHOT\Modèles\SHOT-modeles` (rangement d'Anthony, 2026-10-23) ;
/// s'il n'existe pas et que l'ancien `Documents\Work\SHOT-modeles` existe, l'ancien. Regardé une fois par séance (les
/// Préférences le montrent à chaque image d'écran).
pub fn racine_par_defaut() -> PathBuf {
    static RACINE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    RACINE.get_or_init(|| racine_par_defaut_dans(&documents())).clone()
}

/// `racine_par_defaut` pour le dossier Documents `documents`.
pub fn racine_par_defaut_dans(documents: &Path) -> PathBuf {
    let neuve = documents.join("SHOT").join("Modèles").join("SHOT-modeles");
    let ancienne = documents.join("Work").join("SHOT-modeles");
    if !neuve.is_dir() && ancienne.is_dir() {
        ancienne
    } else {
        neuve
    }
}

/// Le Python du venv d'une racine `SHOT-modeles`.
pub fn python_de(racine: &Path) -> PathBuf {
    racine.join("venv").join("Scripts").join("python.exe")
}

/// Le dossier des modèles voulu par les réglages (vide : celui par défaut).
pub fn racine_des_reglages(r: &GenerationSonReglages) -> PathBuf {
    if r.modeles_local.trim().is_empty() {
        racine_par_defaut()
    } else {
        PathBuf::from(r.modeles_local.trim())
    }
}

/// Le Python voulu par les réglages (vide : celui du venv de la racine).
pub fn python_des_reglages(r: &GenerationSonReglages) -> PathBuf {
    if r.python_local.trim().is_empty() {
        python_de(&racine_des_reglages(r))
    } else {
        PathBuf::from(r.python_local.trim())
    }
}

/// `sidecar/serveur.py` : à côté de l'exécutable (ou d'un de ses dossiers parents), sinon dans le dossier
/// des sources qui a compilé SHOT.
pub fn script_serveur() -> Option<PathBuf> {
    let mut candidats: Vec<PathBuf> = Vec::new();
    if let Some(exe) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        candidats.extend(exe.ancestors().take(6).map(|d| d.join("sidecar").join("serveur.py")));
    }
    candidats.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("sidecar").join("serveur.py"));
    candidats.into_iter().find(|c| c.is_file())
}

/// La configuration de lancement d'après les réglages, ou ce qui manque (en français).
pub fn config_des_reglages(r: &GenerationSonReglages) -> Result<ConfigServeur, String> {
    let python = python_des_reglages(r);
    if !python.is_file() {
        return Err(format!(
            "Python introuvable : {}. Choisis le Python du serveur local dans Fichier > Préférences > Génération sonore.",
            python.display()
        ));
    }
    let script = script_serveur().ok_or_else(|| "le script du serveur local (sidecar/serveur.py) est introuvable à côté de SHOT.".to_string())?;
    Ok(ConfigServeur { python, racine: racine_des_reglages(r), script, port: r.port_local })
}

// ------------------------------------------------------------------------------------- les processus

/// Un processus de serveur lancé par SHOT.
pub trait Processus: Send {
    fn vivant(&mut self) -> bool;
    fn arreter(&mut self);
    fn pid(&self) -> u32;
}

/// Ce qui lance un processus de serveur (le vrai : `lancer_reel` ; les essais : un faux).
pub type Lanceur = Arc<dyn Fn(&ConfigServeur) -> Result<Box<dyn Processus>, String> + Send + Sync>;

/// Le journal du serveur : `%LOCALAPPDATA%\SHOT\serveur_son.log`.
pub fn fichier_journal() -> PathBuf {
    crate::donnees::local().unwrap_or_else(std::env::temp_dir).join("serveur_son.log")
}

/// La fin du journal du serveur (de quoi dire pourquoi il s'est arrêté).
pub fn fin_du_journal(chemin: &Path, octets: u64) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(chemin) else { return String::new() };
    let taille = f.metadata().map(|m| m.len()).unwrap_or(0);
    let _ = f.seek(SeekFrom::Start(taille.saturating_sub(octets)));
    let mut brut = Vec::new();
    let _ = f.read_to_end(&mut brut);
    let texte = String::from_utf8_lossy(&brut).into_owned();
    // (Les dernières lignes non vides, la plus parlante en dernier.)
    let lignes: Vec<&str> = texte.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    lignes[lignes.len().saturating_sub(3)..].join(" / ")
}

/// Le gestionnaire du serveur lancé par SHOT : au plus un processus à la fois.
pub struct GestionnaireServeur {
    lanceur: Lanceur,
    courant: Mutex<Option<Box<dyn Processus>>>,
    journal: PathBuf,
}

impl GestionnaireServeur {
    /// Le gestionnaire réel : lance `python -I sidecar/serveur.py`.
    #[cfg_attr(test, allow(dead_code))]
    pub fn reel() -> Arc<GestionnaireServeur> {
        GestionnaireServeur::avec_lanceur(Arc::new(lancer_reel))
    }

    pub fn avec_lanceur(lanceur: Lanceur) -> Arc<GestionnaireServeur> {
        Arc::new(GestionnaireServeur { lanceur, courant: Mutex::new(None), journal: fichier_journal() })
    }

    /// Un gestionnaire qui ne lance jamais rien (essais de l'interface).
    #[cfg(test)]
    pub fn inactif() -> Arc<GestionnaireServeur> {
        GestionnaireServeur::avec_lanceur(Arc::new(|_| Err("le lancement du serveur est coupé dans cet essai".to_string())))
    }

    fn verrou(&self) -> std::sync::MutexGuard<'_, Option<Box<dyn Processus>>> {
        self.courant.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Le serveur lancé par SHOT tourne-t-il encore ?
    pub fn en_marche(&self) -> bool {
        let mut g = self.verrou();
        let vivant = g.as_mut().map(|p| p.vivant());
        match vivant {
            Some(true) => true,
            Some(false) => {
                *g = None;
                false
            }
            None => false,
        }
    }

    pub fn pid(&self) -> Option<u32> {
        if self.en_marche() {
            self.verrou().as_ref().map(|p| p.pid())
        } else {
            None
        }
    }

    /// Lance le serveur s'il ne tourne pas déjà (lancé par SHOT). `Ok(true)` : lancé à l'instant.
    pub fn demarrer(&self, config: &ConfigServeur) -> Result<bool, String> {
        // (Le verrou reste pris pendant le lancement : deux demandes en même temps ne lancent qu'un serveur.)
        let mut g = self.verrou();
        if g.as_mut().is_some_and(|p| p.vivant()) {
            return Ok(false);
        }
        *g = Some((self.lanceur)(config)?);
        Ok(true)
    }

    /// Arrête le serveur lancé par SHOT. Vrai s'il y en avait un.
    pub fn arreter(&self) -> bool {
        let p = self.verrou().take();
        match p {
            Some(mut p) => {
                p.arreter();
                true
            }
            None => false,
        }
    }

    /// La fin du journal (pourquoi le serveur s'est arrêté).
    pub fn fin_du_journal(&self) -> String {
        fin_du_journal(&self.journal, 4000)
    }
}

impl Drop for GestionnaireServeur {
    fn drop(&mut self) {
        self.arreter();
    }
}

// ------------------------------------------------------------------------------------- l'attente

/// Attend que le serveur lancé par SHOT réponde à `/v1/etat` (au plus `delai`). Annulable. Si le
/// processus s'arrête avant, l'erreur dit pourquoi (fin du journal).
pub fn attendre_pret(client: &ClientSidecar, gestionnaire: &GestionnaireServeur, controle: &Controle, delai: Duration) -> Result<EtatSidecar, ErreurGen> {
    let debut = Instant::now();
    loop {
        controle.verifier()?;
        match client.etat(controle) {
            Ok(e) => return Ok(e),
            Err(ErreurGen::NonInstalle(_)) | Err(ErreurGen::Indisponible(_)) => {}
            Err(e) => return Err(e),
        }
        if !gestionnaire.en_marche() {
            let fin = gestionnaire.fin_du_journal();
            return Err(ErreurGen::NonInstalle(format!("le serveur local s'est arrêté au démarrage{}", if fin.is_empty() { String::new() } else { format!(" ({fin})") })));
        }
        if debut.elapsed() > delai {
            return Err(ErreurGen::Indisponible(format!("le serveur local ne répond pas après {} s", delai.as_secs())));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Lance le serveur (s'il ne tourne pas) et attend qu'il réponde. Pour le bouton « Démarrer » des Préférences.
pub fn demarrer_et_attendre(gestionnaire: &GestionnaireServeur, config: &ConfigServeur, controle: &Controle) -> Result<EtatSidecar, ErreurGen> {
    let client = ClientSidecar::sur_le_port(config.port);
    // Quelque chose répond déjà sur ce port (un serveur lancé à la main, par exemple) : rien à lancer.
    if let Ok(e) = client.etat(controle) {
        return Ok(e);
    }
    gestionnaire.demarrer(config).map_err(ErreurGen::NonInstalle)?;
    attendre_pret(&client, gestionnaire, controle, DELAI_DEMARRAGE)
}

/// Le lancement automatique d'un client : si le serveur ne répond pas, SHOT le lance et l'attend.
#[derive(Clone)]
pub struct Demarrage {
    pub gestionnaire: Arc<GestionnaireServeur>,
    pub config: Result<ConfigServeur, String>,
}

impl std::fmt::Debug for Demarrage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Demarrage({})", if self.config.is_ok() { "prêt" } else { "impossible" })
    }
}

impl Demarrage {
    /// Appelé quand la connexion est refusée : lance le serveur et attend qu'il soit prêt.
    pub fn assurer(&self, client: &ClientSidecar, controle: &Controle) -> Result<(), ErreurGen> {
        let config = self.config.as_ref().map_err(|m| ErreurGen::NonInstalle(format!("aucun serveur local ne répond, et SHOT ne peut pas le lancer : {m}")))?;
        let (fait, total, _) = controle.lire();
        controle.avancer(fait, total, "Démarrage du serveur local…");
        self.gestionnaire.demarrer(config).map_err(|m| ErreurGen::NonInstalle(format!("le serveur local ne répond pas et n'a pas pu être lancé : {m}")))?;
        attendre_pret(client, &self.gestionnaire, controle, DELAI_DEMARRAGE).map(|_| ())
    }
}

// --------------------------------------------------------------------------- le vrai lancement

#[cfg_attr(test, allow(dead_code))]
fn lancer_reel(config: &ConfigServeur) -> Result<Box<dyn Processus>, String> {
    use std::process::{Command, Stdio};

    let journal = fichier_journal();
    if let Some(dossier) = journal.parent() {
        let _ = std::fs::create_dir_all(dossier);
    }
    let sortie = std::fs::File::create(&journal).map_err(|e| format!("le journal du serveur ({}) est inaccessible : {e}", journal.display()))?;
    let erreurs = sortie.try_clone().map_err(|e| e.to_string())?;
    let mut cmd = Command::new(&config.python);
    cmd.arg("-I")
        .arg(&config.script)
        .arg("--racine")
        .arg(&config.racine)
        .arg("--port")
        .arg(config.port.to_string())
        .arg("--parent-pid")
        .arg(std::process::id().to_string())
        .env("HF_HUB_OFFLINE", "1")
        .env("TRANSFORMERS_OFFLINE", "1")
        .stdin(Stdio::null())
        .stdout(sortie)
        .stderr(erreurs);
    #[cfg(windows)]
    {
        return sys::lancer(cmd, &config.python);
    }
    #[cfg(not(windows))]
    {
        let enfant = cmd.spawn().map_err(|e| format!("impossible de lancer {} : {e}", config.python.display()))?;
        Ok(Box::new(ProcessusSimple(enfant)))
    }
}

#[cfg(not(windows))]
struct ProcessusSimple(std::process::Child);

#[cfg(not(windows))]
impl Processus for ProcessusSimple {
    fn vivant(&mut self) -> bool {
        matches!(self.0.try_wait(), Ok(None))
    }
    fn arreter(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
    fn pid(&self) -> u32 {
        self.0.id()
    }
}

#[cfg(windows)]
pub(crate) mod sys {
    //! Windows : processus sans console, dans un objet de travail qui le tue avec SHOT. Les fonctions de
    //! `kernel32` et `ntdll` sont déclarées à la main, comme `materiel.rs` le fait : aucune caisse de plus.
    use std::ffi::c_void;
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::process::CommandExt;
    use std::path::Path;
    use std::process::{Child, Command};

    use super::Processus;

    type Handle = *mut c_void;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const CREATE_SUSPENDED: u32 = 0x0000_0004;
    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;
    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: i32 = 9;

    #[repr(C)]
    struct BasicLimit {
        per_process_user_time: i64,
        per_job_user_time: i64,
        limit_flags: u32,
        minimum_working_set: usize,
        maximum_working_set: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[repr(C)]
    struct ExtendedLimit {
        basic: BasicLimit,
        io: [u64; 6],
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory: usize,
        peak_job_memory: usize,
    }

    extern "system" {
        fn CreateJobObjectW(attributes: *mut c_void, name: *const u16) -> Handle;
        fn SetInformationJobObject(job: Handle, class: i32, info: *mut c_void, length: u32) -> i32;
        fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
        fn TerminateJobObject(job: Handle, code: u32) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
    }

    #[link(name = "ntdll")]
    extern "system" {
        fn NtResumeProcess(process: Handle) -> i32;
    }

    /// Un objet de travail « tuer à la fermeture » : le fermer (ou mourir) tue tout ce qu'il contient.
    struct Travail(usize);

    impl Travail {
        fn nouveau() -> Option<Travail> {
            unsafe {
                let job = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
                if job.is_null() {
                    return None;
                }
                let mut info: ExtendedLimit = std::mem::zeroed();
                info.basic.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(job, JOB_OBJECT_EXTENDED_LIMIT_INFORMATION, &mut info as *mut ExtendedLimit as *mut c_void, std::mem::size_of::<ExtendedLimit>() as u32);
                if ok == 0 {
                    CloseHandle(job);
                    return None;
                }
                Some(Travail(job as usize))
            }
        }

        fn prendre(&self, processus: Handle) -> bool {
            unsafe { AssignProcessToJobObject(self.0 as Handle, processus) != 0 }
        }

        fn tuer(&self) {
            unsafe {
                TerminateJobObject(self.0 as Handle, 1);
            }
        }
    }

    impl Drop for Travail {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0 as Handle);
            }
        }
    }

    pub struct ProcessusWindows {
        enfant: Child,
        travail: Option<Travail>,
    }

    impl Processus for ProcessusWindows {
        fn vivant(&mut self) -> bool {
            matches!(self.enfant.try_wait(), Ok(None))
        }

        fn arreter(&mut self) {
            if let Some(t) = &self.travail {
                t.tuer();
            }
            let _ = self.enfant.kill();
            let _ = self.enfant.wait();
            if self.travail.is_none() {
                // (Sans objet de travail, `taskkill /T` tue l'arbre : le lanceur du venv a un enfant.)
                let _ = Command::new("taskkill").args(["/PID", &self.enfant.id().to_string(), "/T", "/F"]).creation_flags(CREATE_NO_WINDOW).output();
            }
        }

        fn pid(&self) -> u32 {
            self.enfant.id()
        }
    }

    impl Drop for ProcessusWindows {
        fn drop(&mut self) {
            self.arreter();
        }
    }

    /// Lance `cmd` sans console, suspendu ; le met dans l'objet de travail ; puis le laisse partir.
    /// (Suspendu : le lanceur du venv démarre le vrai Python tout de suite, avant qu'on ait pu l'enrôler.)
    pub fn lancer(mut cmd: Command, python: &Path) -> Result<Box<dyn Processus>, String> {
        let travail = Travail::nouveau();
        cmd.creation_flags(CREATE_NO_WINDOW | if travail.is_some() { CREATE_SUSPENDED } else { 0 });
        let enfant = cmd.spawn().map_err(|e| format!("impossible de lancer {} : {e}", python.display()))?;
        let mut travail = travail;
        if let Some(t) = &travail {
            let handle = enfant.as_raw_handle() as Handle;
            let pris = t.prendre(handle);
            let reprise = unsafe { NtResumeProcess(handle) };
            if reprise < 0 {
                let mut p = ProcessusWindows { enfant, travail: travail.take() };
                p.arreter();
                return Err("le processus du serveur n'a pas pu être relancé après sa mise en place".into());
            }
            if !pris {
                travail = None;
            }
        }
        Ok(Box::new(ProcessusWindows { enfant, travail }))
    }
}
