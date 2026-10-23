//! La mémoire vidéo que le système accorde à SHOT, lue toutes les 1,5 s : le cache des textures de dessin (`gpu.rs`)
//! se fait plus petit quand SHOT approche de ce budget, et regrandit quand il y a de la place.
//!
//! Pourquoi (audit du 2026-10-23, point 5) : le cache visait un poids fixe (1 à 2 Go, choisi par le diagnostic), sans
//! regarder ce que les autres logiciels prennent. Mémoire vidéo à 6,9 Go sur 8 (Storyboard Pro 3,5 Go, SHOT 1,5 Go,
//! le bureau de Windows), chaque image recomposée attendait la carte 25 à 46 ms au lieu de 2 ; à 5,4 Go, 1,4 à 2,5 ms.
//! Sous Windows (WDDM), un processus qui dépasse son budget voit ses textures déplacées en mémoire centrale.
//!
//! Comment : `IDXGIAdapter3::QueryVideoMemoryInfo` (segment local) donne le budget du processus — le système le
//! calcule d'après ce que prennent tous les autres — et ce que SHOT utilise (toutes ses allocations sur cette carte,
//! par Direct3D 12 ou Vulkan). DXGI est chargé à la demande (`LoadLibraryW`), sans caisse de plus ; un fil le lit
//! toutes les 1,5 s, jamais le fil de l'interface. La carte est retrouvée par ses numéros PCI (ceux de l'adaptateur
//! wgpu). Si rien ne répond (autre système, ancien Windows, carte introuvable), le cache garde son poids réglé,
//! comme avant.
//!
//! La règle (`cible_cache`, fonction pure) : SHOT sans son cache de dessins (`utilise − cache`) plus le cache doit
//! tenir sous le budget, moins une marge ; le cache ne descend pas sous un plancher (les dessins de l'image affichée
//! ne partent de toute façon jamais, `gpu.rs`). La cible n'est refaite qu'à chaque nouvelle lecture, avec le poids
//! du cache à ce moment : entre deux lectures, ce qu'on libère ne fait pas croire à une place qui n'est pas encore là.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

/// Intervalle entre deux lectures du budget.
pub(crate) const INTERVALLE: Duration = Duration::from_millis(1500);
/// Marge gardée sous le budget : un dixième, au moins 256 Mo (les textures de travail d'une image, l'affichage, egui).
const MARGE_MIN: u64 = 256 << 20;
/// Le cache ne descend pas plus bas (les dessins de l'image affichée restent même au-delà).
pub(crate) const PLANCHER: u64 = 64 << 20;

/// Une lecture : le budget accordé à SHOT et ce qu'il utilise, sur le segment local de la carte (octets).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Lecture {
    /// Numéro de la lecture (augmente à chaque lecture réussie).
    pub seq: u64,
    pub budget: u64,
    pub utilise: u64,
}

/// La mémoire que le cache des dessins peut viser, d'après le budget accordé à SHOT, ce que SHOT utilise en tout, et ce
/// que le cache des dessins tient (`propre`, compris dans `utilise`) : SHOT hors cache, plus le cache, sous le budget
/// moins la marge. Jamais sous `PLANCHER`.
pub(crate) fn cible_cache(budget: u64, utilise: u64, propre: u64) -> u64 {
    let hors_cache = utilise.saturating_sub(propre);
    let marge = (budget / 10).max(MARGE_MIN);
    budget.saturating_sub(hors_cache).saturating_sub(marge).max(PLANCHER)
}

/// Le poids que le cache vise : celui des réglages, ou moins si la mémoire vidéo manque.
pub(crate) fn effectif(regle: u64, plafond: Option<u64>) -> u64 {
    plafond.map_or(regle, |p| regle.min(p))
}

// ---------------------------------------------------------------------------------------------- lectures partagées

static SEQ: AtomicU64 = AtomicU64::new(0);
static BUDGET: AtomicU64 = AtomicU64::new(0);
static UTILISE: AtomicU64 = AtomicU64::new(0);
static SUIVI: std::sync::OnceLock<()> = std::sync::OnceLock::new();
/// Les dernières décisions, pour la fenêtre de diagnostic (la plus récente à la fin).
static JOURNAL: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

#[cfg(test)]
thread_local! {
    /// Essais : la lecture que voit ce fil (rien : aucune), ou les vraies lectures si `REELLES`.
    static ESSAI: std::cell::Cell<Option<Lecture>> = const { std::cell::Cell::new(None) };
    static REELLES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Essais : ce fil voit cette lecture (rien : aucune lecture, le comportement d'avant).
#[cfg(test)]
pub(crate) fn poser_lecture_essai(l: Option<Lecture>) {
    ESSAI.with(|c| c.set(l));
}

/// Mesures : ce fil voit les vraies lectures de la carte (`on`, le fil qui les fait est lancé pour `info`), ou celles
/// posées par l'essai.
#[cfg(test)]
pub(crate) fn lectures_reelles_pour_essai(info: &eframe::egui_wgpu::wgpu::AdapterInfo, on: bool) {
    REELLES.with(|c| c.set(on));
    if on {
        lancer(info);
    }
}

/// La dernière lecture, s'il y en a une.
pub(crate) fn lecture() -> Option<Lecture> {
    #[cfg(test)]
    {
        if !REELLES.with(std::cell::Cell::get) {
            return ESSAI.with(std::cell::Cell::get);
        }
    }
    let seq = SEQ.load(Ordering::Acquire);
    (seq > 0).then(|| Lecture { seq, budget: BUDGET.load(Ordering::Relaxed), utilise: UTILISE.load(Ordering::Relaxed) })
}

/// Commence à suivre le budget de la carte `info` (une fois par processus ; rien dans les essais, qui posent leurs
/// lectures).
pub(crate) fn suivre(info: &eframe::egui_wgpu::wgpu::AdapterInfo) {
    #[cfg(not(test))]
    lancer(info);
    #[cfg(test)]
    let _ = info;
}

fn lancer(info: &eframe::egui_wgpu::wgpu::AdapterInfo) {
    let (vendor, device) = (info.vendor, info.device);
    SUIVI.get_or_init(|| {
        let _ = std::thread::Builder::new().name("memoire-video".into()).spawn(move || {
            crate::parallele::fil_sous_la_normale();
            let Some(carte) = dxgi::Carte::trouver(vendor, device) else {
                noter("Mémoire vidéo : budget du système illisible sur cette carte, le cache garde son poids réglé.".into());
                return;
            };
            loop {
                match carte.lire() {
                    Some((budget, utilise)) => {
                        BUDGET.store(budget, Ordering::Relaxed);
                        UTILISE.store(utilise, Ordering::Relaxed);
                        SEQ.fetch_add(1, Ordering::Release);
                    }
                    None => {
                        // (Une lecture qui échoue garde la précédente ; la suivante réessaie.)
                    }
                }
                std::thread::sleep(INTERVALLE);
            }
        });
    });
}

/// Retient une décision (la fenêtre de diagnostic montre les dernières).
pub(crate) fn noter(texte: String) {
    let mut j = JOURNAL.lock().unwrap_or_else(|e| e.into_inner());
    j.push_back(texte);
    while j.len() > 6 {
        j.pop_front();
    }
}

/// Les dernières décisions, la plus récente à la fin.
pub(crate) fn journal() -> Vec<String> {
    JOURNAL.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect()
}

/// Le plafond du cache a changé de plus d'un vingtième (ou apparaît, ou disparaît) : c'est une décision à noter.
pub(crate) fn vaut_d_etre_note(avant: Option<u64>, apres: Option<u64>, regle: u64) -> bool {
    let (a, b) = (effectif(regle, avant), effectif(regle, apres));
    a.abs_diff(b) > a.max(b) / 20
}

/// La ligne du journal pour une nouvelle cible.
pub(crate) fn decision(l: Lecture, propre: u64, regle: u64, cible: u64) -> String {
    let mo = |o: u64| o >> 20;
    let vise = effectif(regle, Some(cible));
    if vise < regle {
        format!(
            "Mémoire vidéo : budget {} Mo, SHOT en utilise {} Mo (dont {} Mo de dessins) : cache des dessins ramené à {} Mo (réglé : {} Mo).",
            mo(l.budget),
            mo(l.utilise),
            mo(propre),
            mo(vise),
            mo(regle)
        )
    } else {
        format!("Mémoire vidéo : budget {} Mo, SHOT en utilise {} Mo : cache des dessins à son poids réglé ({} Mo).", mo(l.budget), mo(l.utilise), mo(regle))
    }
}

// ---------------------------------------------------------------------------------------------- DXGI

#[cfg(windows)]
mod dxgi {
    use std::ffi::c_void;

    #[repr(C)]
    struct Guid(u32, u16, u16, [u8; 8]);

    const IID_IDXGIFACTORY1: Guid = Guid(0x770a_ae78, 0xf26f, 0x4dba, [0xa8, 0x29, 0x25, 0x3c, 0x83, 0xd1, 0xb3, 0x87]);
    const IID_IDXGIADAPTER3: Guid = Guid(0x6459_67a4, 0x1392, 0x4310, [0xa7, 0x98, 0x80, 0x53, 0xce, 0x3e, 0x93, 0xfd]);
    const DXGI_ADAPTER_FLAG_SOFTWARE: u32 = 2;
    const DXGI_MEMORY_SEGMENT_GROUP_LOCAL: u32 = 0;

    #[repr(C)]
    struct AdapterDesc1 {
        description: [u16; 128],
        vendor_id: u32,
        device_id: u32,
        sub_sys_id: u32,
        revision: u32,
        dedicated_video_memory: usize,
        dedicated_system_memory: usize,
        shared_system_memory: usize,
        luid: [u32; 2],
        flags: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct VideoMemoryInfo {
        budget: u64,
        current_usage: u64,
        available_for_reservation: u64,
        current_reservation: u64,
    }

    type QueryInterface = unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> i32;
    type Release = unsafe extern "system" fn(*mut c_void) -> u32;
    type EnumAdapters1 = unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32;
    type GetDesc1 = unsafe extern "system" fn(*mut c_void, *mut AdapterDesc1) -> i32;
    type QueryVideoMemoryInfo = unsafe extern "system" fn(*mut c_void, u32, u32, *mut VideoMemoryInfo) -> i32;
    type CreateFactory1 = unsafe extern "system" fn(*const Guid, *mut *mut c_void) -> i32;

    extern "system" {
        fn LoadLibraryW(name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }

    /// L'entrée `i` de la table des fonctions de l'objet COM `obj`.
    unsafe fn methode(obj: *mut c_void, i: usize) -> *const c_void {
        let vtable = *(obj as *const *const *const c_void);
        *vtable.add(i)
    }

    unsafe fn relacher(obj: *mut c_void) {
        if !obj.is_null() {
            let release: Release = std::mem::transmute(methode(obj, 2));
            release(obj);
        }
    }

    /// La carte (son interface `IDXGIAdapter3`), gardée par le fil qui la lit.
    pub(super) struct Carte(*mut c_void);

    impl Drop for Carte {
        fn drop(&mut self) {
            unsafe { relacher(self.0) };
        }
    }

    impl Carte {
        /// La première carte matérielle de ces numéros PCI qui sait dire son budget (Windows 10 et suivants).
        pub(super) fn trouver(vendor: u32, device: u32) -> Option<Carte> {
            unsafe {
                let nom: Vec<u16> = "dxgi.dll\0".encode_utf16().collect();
                let dll = LoadLibraryW(nom.as_ptr());
                if dll.is_null() {
                    return None;
                }
                let f = GetProcAddress(dll, c"CreateDXGIFactory1".as_ptr() as *const u8);
                if f.is_null() {
                    return None;
                }
                let create: CreateFactory1 = std::mem::transmute(f);
                let mut factory: *mut c_void = std::ptr::null_mut();
                if create(&IID_IDXGIFACTORY1, &mut factory) < 0 || factory.is_null() {
                    return None;
                }
                let enum_adapters: EnumAdapters1 = std::mem::transmute(methode(factory, 12));
                let mut trouvee = None;
                for i in 0..16 {
                    let mut adapter: *mut c_void = std::ptr::null_mut();
                    if enum_adapters(factory, i, &mut adapter) < 0 || adapter.is_null() {
                        break;
                    }
                    let get_desc: GetDesc1 = std::mem::transmute(methode(adapter, 10));
                    let mut desc: AdapterDesc1 = std::mem::zeroed();
                    let bonne = get_desc(adapter, &mut desc) >= 0 && desc.flags & DXGI_ADAPTER_FLAG_SOFTWARE == 0 && desc.vendor_id == vendor && desc.device_id == device;
                    if bonne {
                        let qi: QueryInterface = std::mem::transmute(methode(adapter, 0));
                        let mut a3: *mut c_void = std::ptr::null_mut();
                        if qi(adapter, &IID_IDXGIADAPTER3, &mut a3) >= 0 && !a3.is_null() {
                            trouvee = Some(Carte(a3));
                        }
                    }
                    relacher(adapter);
                    if trouvee.is_some() {
                        break;
                    }
                }
                relacher(factory);
                let carte = trouvee?;
                // (Une carte qui ne répond pas dès la première fois ne répondra pas mieux ensuite.)
                carte.lire()?;
                Some(carte)
            }
        }

        /// Le budget du processus et ce qu'il utilise, sur le segment local (octets).
        pub(super) fn lire(&self) -> Option<(u64, u64)> {
            unsafe {
                let query: QueryVideoMemoryInfo = std::mem::transmute(methode(self.0, 14));
                let mut info = VideoMemoryInfo::default();
                (query(self.0, 0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut info) >= 0 && info.budget > 0).then_some((info.budget, info.current_usage))
            }
        }
    }
}

#[cfg(not(windows))]
mod dxgi {
    pub(super) struct Carte;

    impl Carte {
        pub(super) fn trouver(_vendor: u32, _device: u32) -> Option<Carte> {
            None
        }

        pub(super) fn lire(&self) -> Option<(u64, u64)> {
            None
        }
    }
}
