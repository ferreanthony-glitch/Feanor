//! Détection du matériel pour Aide > Diagnostic des performances (`diagnostic.rs`).
//!
//! Tout vient du système ou de caisses déjà là, sans rien télécharger ni installer :
//! - **processeur** : nom lu dans le registre de Windows, cœurs logiques par la bibliothèque
//!   standard, cœurs physiques par `GetLogicalProcessorInformation`, instructions AVX2 / FMA /
//!   BMI2 par `is_x86_feature_detected` (SHOT est compilé pour x86-64-v3, `.cargo/config.toml`) ;
//! - **mémoire vive** : `GlobalMemoryStatusEx` (kernel32), totale et disponible ;
//! - **carte graphique** : les informations de l'adaptateur wgpu (nom, API, pilote, genre) ; la
//!   mémoire vidéo est lue dans le registre de Windows (`HardwareInformation.qwMemorySize` de la
//!   carte qui porte le même nom), « inconnue » si elle n'y est pas (wgpu 24 ne la donne pas) ;
//! - **écran** : taille de la fenêtre, du moniteur et pixels par point, donnés par egui ;
//! - **disque** : un court essai d'écriture et de lecture dans le dossier temporaire.
//!
//! Les appels système sont déclarés ici (quelques fonctions de kernel32 et d'advapi32, que la
//! bibliothèque standard lie déjà) : aucune caisse de plus. Hors Windows, ces mesures manquent
//! (`None`) et le diagnostic continue avec le reste.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const MO: u64 = 1 << 20;

/// Le processeur.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Processeur {
    pub nom: String,
    /// Fils d'exécution (hyper-threading compris).
    pub logiques: usize,
    /// Cœurs physiques, si Windows les donne.
    pub physiques: Option<usize>,
    pub avx2: bool,
    pub fma: bool,
    pub bmi2: bool,
}

impl Processeur {
    /// x86-64-v3 : AVX2, FMA et BMI2, ce pour quoi SHOT est compilé.
    pub fn x86_64_v3(&self) -> bool {
        self.avx2 && self.fma && self.bmi2
    }
}

/// La mémoire vive, en octets.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Memoire {
    pub totale: u64,
    pub disponible: u64,
}

/// Le genre de carte graphique.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GenreCarte {
    Dediee,
    Integree,
    Virtuelle,
    /// Rendu par le processeur (pas de vraie carte).
    Logicielle,
    #[default]
    Autre,
}

impl GenreCarte {
    pub fn nom(self) -> &'static str {
        match self {
            GenreCarte::Dediee => "carte dédiée",
            GenreCarte::Integree => "graphisme intégré au processeur",
            GenreCarte::Virtuelle => "carte virtuelle",
            GenreCarte::Logicielle => "rendu logiciel (sans carte graphique)",
            GenreCarte::Autre => "genre inconnu",
        }
    }
}

/// La carte graphique qui affiche SHOT.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Carte {
    pub nom: String,
    /// « Dx12 », « Vulkan »…
    pub api: String,
    pub pilote: String,
    pub genre: GenreCarte,
    /// Mémoire vidéo en octets, `None` : inconnue.
    pub vram: Option<u64>,
}

/// L'écran.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ecran {
    /// Fenêtre de SHOT, en points.
    pub fenetre: [f32; 2],
    /// Moniteur, en points, si egui le donne.
    pub moniteur: Option<[f32; 2]>,
    pub ppp: f32,
}

impl Ecran {
    /// Pixels du moniteur (ou de la fenêtre à défaut).
    pub fn pixels(&self) -> [u32; 2] {
        let [w, h] = self.moniteur.unwrap_or(self.fenetre);
        [(w * self.ppp).round() as u32, (h * self.ppp).round() as u32]
    }
}

/// Le disque (dossier temporaire), en Mo par seconde.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Disque {
    pub ecriture: f64,
    pub lecture: f64,
    /// Quantité écrite puis relue, en Mo.
    pub taille_mo: usize,
}

/// Tout ce qu'on sait de la machine.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Materiel {
    pub processeur: Processeur,
    pub memoire: Option<Memoire>,
    pub carte: Option<Carte>,
    pub ecran: Option<Ecran>,
    pub disque: Option<Disque>,
}

pub fn en_go(octets: u64) -> String {
    format!("{:.1} Go", octets as f64 / (1u64 << 30) as f64).replace('.', ",")
}

impl Materiel {
    /// Le processeur et la mémoire (rapide, sur n'importe quel fil).
    pub fn systeme() -> Materiel {
        Materiel { processeur: processeur(), memoire: memoire(), ..Default::default() }
    }

    /// Une ligne : processeur, mémoire vive, carte graphique.
    pub fn resume(&self) -> String {
        let mut v = Vec::new();
        if !self.processeur.nom.is_empty() {
            v.push(self.processeur.nom.clone());
        }
        v.push(format!("{} fils", self.processeur.logiques));
        if let Some(m) = self.memoire {
            v.push(format!("{} de mémoire vive", en_go(m.totale)));
        }
        if let Some(c) = &self.carte {
            v.push(match c.vram {
                Some(r) => format!("{} ({})", c.nom, en_go(r)),
                None => c.nom.clone(),
            });
        }
        v.join(" · ")
    }

    /// Lignes lisibles, pour la fenêtre et le rapport : (intitulé, valeur).
    pub fn lignes(&self) -> Vec<(&'static str, String)> {
        let p = &self.processeur;
        let mut v = Vec::new();
        let coeurs = match p.physiques {
            Some(n) => format!("{n} cœurs physiques, {} fils", p.logiques),
            None => format!("{} fils", p.logiques),
        };
        let nom = if p.nom.is_empty() { "processeur inconnu" } else { p.nom.as_str() };
        v.push(("Processeur", format!("{nom} — {coeurs}")));
        let oui = |b: bool| if b { "oui" } else { "non" };
        v.push(("Instructions", format!("AVX2 : {}, FMA : {}, BMI2 : {} — x86-64-v3 : {}", oui(p.avx2), oui(p.fma), oui(p.bmi2), oui(p.x86_64_v3()))));
        v.push((
            "Mémoire vive",
            match self.memoire {
                Some(m) => format!("{} au total, {} disponibles", en_go(m.totale), en_go(m.disponible)),
                None => "inconnue".into(),
            },
        ));
        v.push((
            "Carte graphique",
            match &self.carte {
                Some(c) => format!("{} — {}, {}", c.nom, c.api, c.genre.nom()),
                None => "inconnue (affichage OpenGL de secours)".into(),
            },
        ));
        if let Some(c) = &self.carte {
            v.push(("Pilote", if c.pilote.is_empty() { "inconnu".into() } else { c.pilote.clone() }));
            v.push(("Mémoire vidéo", c.vram.map_or("inconnue".to_string(), en_go)));
        }
        v.push((
            "Écran",
            match self.ecran {
                Some(e) => {
                    let [w, h] = e.pixels();
                    format!("{w} × {h} pixels, {} pixel par point", format!("{:.2}", e.ppp).replace('.', ","))
                }
                None => "inconnu".into(),
            },
        ));
        v.push((
            "Disque (dossier temporaire)",
            match &self.disque {
                Some(d) => format!("écriture {:.0} Mo/s, lecture {:.0} Mo/s ({} Mo)", d.ecriture, d.lecture, d.taille_mo),
                None => "pas encore mesuré".into(),
            },
        ));
        v
    }
}

// ---------------------------------------------------------------------- processeur

#[cfg(target_arch = "x86_64")]
fn instructions() -> (bool, bool, bool) {
    (std::arch::is_x86_feature_detected!("avx2"), std::arch::is_x86_feature_detected!("fma"), std::arch::is_x86_feature_detected!("bmi2"))
}

#[cfg(not(target_arch = "x86_64"))]
fn instructions() -> (bool, bool, bool) {
    (false, false, false)
}

pub fn processeur() -> Processeur {
    let (avx2, fma, bmi2) = instructions();
    Processeur {
        nom: sys::nom_processeur().unwrap_or_default(),
        logiques: std::thread::available_parallelism().map_or(1, |n| n.get()),
        physiques: sys::coeurs_physiques(),
        avx2,
        fma,
        bmi2,
    }
}

pub fn memoire() -> Option<Memoire> {
    sys::memoire()
}

// ---------------------------------------------------------------------- carte graphique

/// La carte qui affiche SHOT, d'après l'adaptateur wgpu.
pub fn carte(info: &eframe::egui_wgpu::wgpu::AdapterInfo) -> Carte {
    use eframe::egui_wgpu::wgpu::DeviceType;
    let genre = match info.device_type {
        DeviceType::DiscreteGpu => GenreCarte::Dediee,
        DeviceType::IntegratedGpu => GenreCarte::Integree,
        DeviceType::VirtualGpu => GenreCarte::Virtuelle,
        DeviceType::Cpu => GenreCarte::Logicielle,
        DeviceType::Other => GenreCarte::Autre,
    };
    let pilote = [info.driver.as_str(), info.driver_info.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" ");
    let vram = (genre != GenreCarte::Logicielle).then(|| sys::memoire_video(&info.name)).flatten();
    Carte { nom: info.name.clone(), api: format!("{:?}", info.backend), pilote, genre, vram }
}

// ---------------------------------------------------------------------- disque

/// Écrit puis relit `taille_mo` Mo dans `dossier`, et efface le fichier. `None` si le dossier
/// refuse ou si `annuler` est levé.
pub fn mesurer_disque(dossier: &Path, taille_mo: usize, annuler: &AtomicBool) -> Option<Disque> {
    /// Le fichier d'essai disparaît quoi qu'il arrive.
    struct Efface(PathBuf);
    impl Drop for Efface {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let chemin = dossier.join(format!("shot-diagnostic-{}.tmp", std::process::id()));
    let _efface = Efface(chemin.clone());
    // Un bloc de 1 Mo de bruit (un disque qui compresse ne le réduirait pas).
    let mut bloc = vec![0u8; MO as usize];
    let mut x = 0x9E37_79B9_7F4A_7C15u64;
    for o in bloc.iter_mut() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *o = x as u8;
    }
    let t = Instant::now();
    {
        let mut f = std::fs::File::create(&chemin).ok()?;
        for _ in 0..taille_mo {
            if annuler.load(Ordering::Relaxed) {
                return None;
            }
            f.write_all(&bloc).ok()?;
        }
        f.sync_all().ok()?;
    }
    let ecriture = taille_mo as f64 / t.elapsed().as_secs_f64().max(1e-6);
    let t = Instant::now();
    {
        let mut f = std::fs::File::open(&chemin).ok()?;
        let mut lu = 0usize;
        loop {
            if annuler.load(Ordering::Relaxed) {
                return None;
            }
            let n = f.read(&mut bloc).ok()?;
            if n == 0 {
                break;
            }
            lu += n;
        }
        if lu < taille_mo * MO as usize {
            return None;
        }
    }
    let lecture = taille_mo as f64 / t.elapsed().as_secs_f64().max(1e-6);
    Some(Disque { ecriture, lecture, taille_mo })
}

// ---------------------------------------------------------------------- Windows

#[cfg(windows)]
mod sys {
    use super::Memoire;

    #[repr(C)]
    struct MemoryStatusEx {
        length: u32,
        memory_load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page: u64,
        avail_page: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended_virtual: u64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct ProcessorInfo {
        mask: usize,
        relationship: u32,
        reserved: [u64; 2],
    }

    extern "system" {
        fn GlobalMemoryStatusEx(buffer: *mut MemoryStatusEx) -> i32;
        fn GetLogicalProcessorInformation(buffer: *mut ProcessorInfo, length: *mut u32) -> i32;
    }

    #[link(name = "advapi32")]
    extern "system" {
        fn RegOpenKeyExW(key: isize, sub_key: *const u16, options: u32, access: u32, result: *mut isize) -> i32;
        fn RegQueryValueExW(key: isize, name: *const u16, reserved: *mut u32, kind: *mut u32, data: *mut u8, length: *mut u32) -> i32;
        fn RegCloseKey(key: isize) -> i32;
    }

    /// HKEY_LOCAL_MACHINE (0x80000002, étendu au signe comme le fait le type HKEY).
    const HKLM: isize = 0x8000_0002u32 as i32 as isize;
    const KEY_READ: u32 = 0x0002_0019;
    const RELATION_PROCESSOR_CORE: u32 = 0;

    pub fn memoire() -> Option<Memoire> {
        let mut s = MemoryStatusEx {
            length: std::mem::size_of::<MemoryStatusEx>() as u32,
            memory_load: 0,
            total_phys: 0,
            avail_phys: 0,
            total_page: 0,
            avail_page: 0,
            total_virtual: 0,
            avail_virtual: 0,
            avail_extended_virtual: 0,
        };
        // SAFETY : `s` est une structure MEMORYSTATUSEX complète dont `length` est renseigné.
        let ok = unsafe { GlobalMemoryStatusEx(&mut s) };
        (ok != 0 && s.total_phys > 0).then_some(Memoire { totale: s.total_phys, disponible: s.avail_phys })
    }

    pub fn coeurs_physiques() -> Option<usize> {
        let mut length = 0u32;
        // SAFETY : premier appel avec un tampon nul, pour apprendre la taille nécessaire.
        unsafe { GetLogicalProcessorInformation(std::ptr::null_mut(), &mut length) };
        let size = std::mem::size_of::<ProcessorInfo>() as u32;
        if length == 0 || length % size != 0 {
            return None;
        }
        let mut buffer = vec![ProcessorInfo { mask: 0, relationship: u32::MAX, reserved: [0; 2] }; (length / size) as usize];
        // SAFETY : `buffer` a exactement `length` octets.
        let ok = unsafe { GetLogicalProcessorInformation(buffer.as_mut_ptr(), &mut length) };
        if ok == 0 {
            return None;
        }
        let n = buffer.iter().filter(|p| p.relationship == RELATION_PROCESSOR_CORE).count();
        (n > 0).then_some(n)
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// La valeur `name` de la clé `sub_key` (sous HKEY_LOCAL_MACHINE) : son type et ses octets.
    fn registre(sub_key: &str, name: &str) -> Option<(u32, Vec<u8>)> {
        let sub = wide(sub_key);
        let nom = wide(name);
        let mut key = 0isize;
        // SAFETY : chaînes terminées par zéro, `key` reçoit la clé ouverte, refermée plus bas.
        if unsafe { RegOpenKeyExW(HKLM, sub.as_ptr(), 0, KEY_READ, &mut key) } != 0 {
            return None;
        }
        let mut kind = 0u32;
        let mut length = 0u32;
        // SAFETY : sans tampon, l'appel donne la taille de la valeur.
        let mut found = unsafe { RegQueryValueExW(key, nom.as_ptr(), std::ptr::null_mut(), &mut kind, std::ptr::null_mut(), &mut length) } == 0;
        let mut data = vec![0u8; length as usize];
        if found && length > 0 {
            // SAFETY : `data` a `length` octets.
            found = unsafe { RegQueryValueExW(key, nom.as_ptr(), std::ptr::null_mut(), &mut kind, data.as_mut_ptr(), &mut length) } == 0;
            data.truncate(length as usize);
        }
        // SAFETY : `key` a été ouverte plus haut.
        unsafe { RegCloseKey(key) };
        (found && !data.is_empty()).then_some((kind, data))
    }

    fn texte(sub_key: &str, name: &str) -> Option<String> {
        let (kind, data) = registre(sub_key, name)?;
        // REG_SZ (1) ou REG_EXPAND_SZ (2) : UTF-16, terminé par zéro.
        if kind != 1 && kind != 2 {
            return None;
        }
        let units: Vec<u16> = data.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&u| u != 0).collect();
        let s = String::from_utf16_lossy(&units).trim().to_string();
        (!s.is_empty()).then_some(s)
    }

    pub fn nom_processeur() -> Option<String> {
        texte(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0", "ProcessorNameString").map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
    }

    /// La mémoire vidéo de la carte graphique nommée `nom` : le registre la garde sous la classe
    /// « Display » (`HardwareInformation.qwMemorySize`, 64 bits, ou `MemorySize`, 32 bits).
    pub fn memoire_video(nom: &str) -> Option<u64> {
        const CLASSE: &str = r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";
        let minuscule = nom.to_lowercase();
        for k in 0..16 {
            let sub = format!(r"{CLASSE}\{k:04}");
            let Some(desc) = texte(&sub, "DriverDesc") else { continue };
            let desc = desc.to_lowercase();
            if desc != minuscule && !minuscule.contains(&desc) && !desc.contains(&minuscule) {
                continue;
            }
            if let Some((_, d)) = registre(&sub, "HardwareInformation.qwMemorySize") {
                if d.len() >= 8 {
                    let v = u64::from_le_bytes(d[..8].try_into().ok()?);
                    if v > 0 {
                        return Some(v);
                    }
                }
            }
            if let Some((_, d)) = registre(&sub, "HardwareInformation.MemorySize") {
                if d.len() >= 4 {
                    let v = u32::from_le_bytes(d[..4].try_into().ok()?) as u64;
                    if v > 0 {
                        return Some(v);
                    }
                }
            }
        }
        None
    }
}

#[cfg(not(windows))]
mod sys {
    use super::Memoire;

    pub fn memoire() -> Option<Memoire> {
        None
    }
    pub fn coeurs_physiques() -> Option<usize> {
        None
    }
    pub fn nom_processeur() -> Option<String> {
        None
    }
    pub fn memoire_video(_nom: &str) -> Option<u64> {
        None
    }
}
