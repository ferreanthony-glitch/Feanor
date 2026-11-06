//! Le stylet lu auprès de Windows (Anthony, 2026-11-06 : « le stylet en entier »). Premier pas : ÉCOUTER, rien d'autre.
//!
//! Aujourd'hui FEANOR ne reçoit du stylet que l'appui. winit traite lui-même les messages `WM_POINTER*` de Windows, lit
//! `GetPointerPenInfo` et n'en garde que `pressure` ; le bouton du stylet, le stylet retourné (la gomme au dos),
//! l'inclinaison et la rotation sont lus par Windows puis perdus avant d'arriver à egui.
//!
//! Ce module pose une **écoute** sur la fenêtre de FEANOR : la procédure de fenêtre est remplacée par `ecoute`, qui lit
//! le stylet pendant le message (le seul moment où Windows le donne), range la lecture dans des atomiques, puis rend le
//! message **inchangé** à la procédure d'origine, celle de winit. Rien n'est retenu ni modifié : le dessin reçoit
//! exactement ce qu'il recevait. L'écoute n'est posée qu'à la première ouverture du banc (`styletui.rs`) : une séance où
//! l'on n'ouvre pas le banc est la même qu'avant, au message près.
//!
//! Ce que le banc doit apprendre, devant la tablette d'Anthony, avant de brancher quoi que ce soit :
//! - le bouton du stylet se voit-il **pendant que la pointe touche** (`Jalon::BoutonPose`) ? C'est la condition des
//!   gestes « bouton tenu » ;
//! - le stylet retourné s'annonce-t-il (`Retourne`, `GommePosee`) ? C'est la gomme au dos du stylet ;
//! - l'inclinaison arrive-t-elle, et jusqu'où (`Penche`) ? C'est l'entrée « inclinaison » des pinceaux (`brush.rs`) ;
//! - par quel chemin le bouton du stylet devient-il un clic droit (un message de souris, venu du pilote ou de Windows) ?
//!
//! Limites connues de ce premier pas : une lecture par message (winit, lui, lit tout l'historique d'une trame : il faudra
//! en faire autant pour brancher l'inclinaison sur chaque point d'un trait) ; la fenêtre principale seulement.
//!
//! Tout ce qui se décide ici (`Lecture::decoder`, `jalons_de`, `Releve::texte`) est séparé de Windows pour être essayé
//! sans tablette.

use std::sync::atomic::{AtomicI32, AtomicU32, Ordering::Relaxed};

/// Drapeaux du stylet (`PEN_FLAG_*` de Windows) : le bouton du fût, le stylet retourné, la gomme qui touche.
pub const BOUTON: u32 = 1;
pub const RETOURNE: u32 = 2;
pub const GOMME: u32 = 4;

/// Ce que le stylet dit savoir mesurer (`PEN_MASK_*`).
pub const MESURE_APPUI: u32 = 1;
pub const MESURE_ROTATION: u32 = 2;
pub const MESURE_PENCHE_X: u32 = 4;
pub const MESURE_PENCHE_Y: u32 = 8;

/// Drapeaux du pointeur (`POINTER_FLAG_*`) : à portée de la tablette, en contact.
pub const A_PORTEE: u32 = 0x2;
pub const EN_CONTACT: u32 = 0x4;

/// L'appui le plus fort que Windows annonce.
pub const APPUI_PLEIN: u32 = 1024;

/// En dessous de cet angle (degrés), le stylet est tenu droit : le bruit de la mesure ne compte pas pour « penché ».
pub const PENCHE_MIN: i32 = 8;

/// Une lecture du stylet, telle que Windows la donne pendant un message.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Lecture {
    /// Le stylet est assez près de la tablette pour être vu.
    pub portee: bool,
    /// La pointe (ou la gomme) touche.
    pub contact: bool,
    /// Le bouton du fût est enfoncé.
    pub bouton: bool,
    /// Le stylet est tenu à l'envers, en l'air.
    pub retourne: bool,
    /// La gomme au dos du stylet touche.
    pub gomme: bool,
    /// L'appui, de 0 à 1.
    pub appui: f32,
    /// L'inclinaison en degrés : gauche-droite (positif vers la droite), puis avant-arrière (positif vers soi).
    pub penche: [i32; 2],
    /// La rotation du stylet sur lui-même, en degrés (0 à 359).
    pub rotation: u32,
    /// Ce que le stylet dit savoir mesurer (`MESURE_*`).
    pub mesures: u32,
}

impl Lecture {
    /// Lit les valeurs brutes de `POINTER_PEN_INFO`.
    pub fn decoder(pointeur: u32, stylet: u32, mesures: u32, appui: u32, rotation: u32, penche: [i32; 2]) -> Self {
        Lecture {
            portee: pointeur & A_PORTEE != 0,
            contact: pointeur & EN_CONTACT != 0,
            bouton: stylet & BOUTON != 0,
            retourne: stylet & RETOURNE != 0,
            gomme: stylet & GOMME != 0,
            appui: appui.min(APPUI_PLEIN) as f32 / APPUI_PLEIN as f32,
            penche: penche.map(|a| a.clamp(-90, 90)),
            rotation: rotation % 360,
            mesures,
        }
    }

    /// Le stylet présente son dos (la gomme), en l'air ou posé.
    pub fn dos(&self) -> bool {
        self.retourne || self.gomme
    }

    /// Le plus grand des deux angles d'inclinaison, en degrés.
    pub fn angle(&self) -> u32 {
        self.penche[0].unsigned_abs().max(self.penche[1].unsigned_abs())
    }
}

/// Ce que le banc demande d'essayer. Chaque jalon se coche dès qu'une lecture le montre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Jalon {
    Survol,
    Appui,
    Penche,
    BoutonEnLAir,
    BoutonPose,
    Retourne,
    GommePosee,
    Rotation,
}

impl Jalon {
    pub const TOUS: [Jalon; 8] = [Jalon::Survol, Jalon::Appui, Jalon::Penche, Jalon::BoutonEnLAir, Jalon::BoutonPose, Jalon::Retourne, Jalon::GommePosee, Jalon::Rotation];

    pub fn bit(self) -> u32 {
        1 << self as u32
    }

    /// Le geste à faire.
    pub fn consigne(self) -> &'static str {
        match self {
            Jalon::Survol => "Approche le stylet sans toucher la tablette",
            Jalon::Appui => "Pose la pointe et appuie, de léger à fort",
            Jalon::Penche => "Couche le stylet sur le côté, pointe posée",
            Jalon::BoutonEnLAir => "Presse le bouton du stylet, pointe en l'air",
            Jalon::BoutonPose => "Presse le bouton du stylet, pointe posée",
            Jalon::Retourne => "Retourne le stylet et approche la gomme",
            Jalon::GommePosee => "Pose la gomme du stylet sur la tablette",
            Jalon::Rotation => "Tourne le stylet sur lui-même (peu de stylets le mesurent)",
        }
    }

    /// Le nom court, pour le relevé.
    pub fn nom(self) -> &'static str {
        match self {
            Jalon::Survol => "survol",
            Jalon::Appui => "appui",
            Jalon::Penche => "inclinaison",
            Jalon::BoutonEnLAir => "bouton, pointe en l'air",
            Jalon::BoutonPose => "bouton, pointe posée",
            Jalon::Retourne => "stylet retourné",
            Jalon::GommePosee => "gomme posée",
            Jalon::Rotation => "rotation",
        }
    }
}

/// Les jalons qu'une lecture montre (bits de `Jalon::bit`).
pub fn jalons_de(l: &Lecture) -> u32 {
    let mut j = 0;
    let mut si = |vrai: bool, jalon: Jalon| {
        if vrai {
            j |= jalon.bit();
        }
    };
    si(l.portee && !l.contact, Jalon::Survol);
    si(l.contact && l.appui > 0.0, Jalon::Appui);
    si(l.angle() >= PENCHE_MIN as u32, Jalon::Penche);
    si(l.bouton && !l.contact, Jalon::BoutonEnLAir);
    si(l.bouton && l.contact, Jalon::BoutonPose);
    si(l.dos(), Jalon::Retourne);
    si(l.dos() && l.contact, Jalon::GommePosee);
    si(l.rotation != 0, Jalon::Rotation);
    j
}

/// Les clics de souris arrivés à la fenêtre : en tout, et ceux que Windows marque comme venant d'un stylet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Clics {
    pub gauche: u32,
    pub gauche_du_stylet: u32,
    pub droit: u32,
    pub droit_du_stylet: u32,
}

/// Ce que FEANOR reçoit aujourd'hui par le chemin habituel (winit puis egui), pendant que le banc est ouvert.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Recu {
    /// Événements de stylet (ou de doigt) reçus.
    pub touchers: u32,
    /// Le plus fort appui reçu avec eux.
    pub appui_max: f32,
    /// Appuis du bouton principal, du bouton secondaire (le clic droit) et du bouton du milieu.
    pub principal: u32,
    pub secondaire: u32,
    pub milieu: u32,
}

/// Tout ce que l'écoute a noté depuis la dernière remise à zéro.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Releve {
    /// La dernière lecture.
    pub lecture: Lecture,
    /// Les jalons vus (bits de `Jalon::bit`).
    pub jalons: u32,
    /// Le plus fort appui et la plus forte inclinaison (degrés) vus.
    pub appui_max: f32,
    pub penche_max: u32,
    /// Messages de stylet lus, messages de doigt vus.
    pub messages: u32,
    pub doigts: u32,
    pub clics: Clics,
}

impl Releve {
    pub fn vu(&self, j: Jalon) -> bool {
        self.jalons & j.bit() != 0
    }

    /// Le relevé en clair : ce qu'Anthony renvoie après son essai (fichier et presse-papiers).
    pub fn texte(&self, recu: &Recu, ecoute: Option<bool>, quand: &str) -> String {
        let oui = |b: bool| if b { "oui" } else { "non" };
        let mut t = format!("Relevé du stylet, FEANOR {}, {quand}\n\n", env!("CARGO_PKG_VERSION"));
        t += &match ecoute {
            Some(true) => "Écoute posée sur la fenêtre : oui\n".to_string(),
            Some(false) => "Écoute posée sur la fenêtre : NON (Windows a refusé)\n".to_string(),
            None => "Écoute posée sur la fenêtre : pas encore demandée\n".to_string(),
        };
        t += &format!("Messages de stylet lus : {}\nMessages de doigt vus : {}\n\n", self.messages, self.doigts);
        t += "Ce que Windows dit du stylet\n";
        for j in Jalon::TOUS {
            t += &format!("- {} : {}\n", j.nom(), if self.vu(j) { "vu" } else { "pas vu" });
        }
        t += &format!("- appui le plus fort : {:.0} %\n- inclinaison la plus forte : {}°\n", self.appui_max * 100.0, self.penche_max);
        let m = self.lecture.mesures;
        t += &format!(
            "- le stylet dit mesurer : appui {}, inclinaison {}, rotation {}\n\n",
            oui(m & MESURE_APPUI != 0),
            oui(m & (MESURE_PENCHE_X | MESURE_PENCHE_Y) != 0),
            oui(m & MESURE_ROTATION != 0)
        );
        t += "Clics de souris arrivés à la fenêtre\n";
        t += &format!("- gauche : {} (dont {} venus du stylet)\n", self.clics.gauche, self.clics.gauche_du_stylet);
        t += &format!("- droit : {} (dont {} venus du stylet)\n\n", self.clics.droit, self.clics.droit_du_stylet);
        t += "Ce que FEANOR reçoit aujourd'hui\n";
        t += &format!("- événements de stylet : {} (appui le plus fort : {:.0} %)\n", recu.touchers, recu.appui_max * 100.0);
        t += &format!("- bouton principal : {}, clic droit : {}, bouton du milieu : {}\n", recu.principal, recu.secondaire, recu.milieu);
        t
    }
}

// ------------------------------------------------------------------------------------------------ ce que l'écoute a noté

static POINTEUR: AtomicU32 = AtomicU32::new(0);
static STYLET: AtomicU32 = AtomicU32::new(0);
static MESURES: AtomicU32 = AtomicU32::new(0);
static APPUI: AtomicU32 = AtomicU32::new(0);
static ROTATION: AtomicU32 = AtomicU32::new(0);
static PENCHE_X: AtomicI32 = AtomicI32::new(0);
static PENCHE_Y: AtomicI32 = AtomicI32::new(0);
static JALONS: AtomicU32 = AtomicU32::new(0);
static APPUI_MAX: AtomicU32 = AtomicU32::new(0);
static PENCHE_MAX: AtomicU32 = AtomicU32::new(0);
static MESSAGES: AtomicU32 = AtomicU32::new(0);
static DOIGTS: AtomicU32 = AtomicU32::new(0);
static CLICS: [AtomicU32; 4] = [AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)];

/// Range une lecture brute du stylet. Appelée pendant un message de Windows : rien que des atomiques, aucune attente,
/// rien qui puisse échouer.
pub fn noter(pointeur: u32, stylet: u32, mesures: u32, appui: u32, rotation: u32, penche: [i32; 2]) {
    POINTEUR.store(pointeur, Relaxed);
    STYLET.store(stylet, Relaxed);
    MESURES.store(mesures, Relaxed);
    APPUI.store(appui, Relaxed);
    ROTATION.store(rotation, Relaxed);
    PENCHE_X.store(penche[0], Relaxed);
    PENCHE_Y.store(penche[1], Relaxed);
    let l = Lecture::decoder(pointeur, stylet, mesures, appui, rotation, penche);
    JALONS.fetch_or(jalons_de(&l), Relaxed);
    if l.contact {
        APPUI_MAX.fetch_max(appui.min(APPUI_PLEIN), Relaxed);
    }
    PENCHE_MAX.fetch_max(l.angle(), Relaxed);
    MESSAGES.fetch_add(1, Relaxed);
}

/// Le stylet a quitté la tablette : la dernière lecture n'est plus ni à portée ni en contact.
pub fn noter_le_depart() {
    POINTEUR.fetch_and(!(A_PORTEE | EN_CONTACT), Relaxed);
    APPUI.store(0, Relaxed);
}

/// Un message de doigt est passé (écran tactile).
pub fn noter_un_doigt() {
    DOIGTS.fetch_add(1, Relaxed);
}

/// Un clic de souris est arrivé à la fenêtre ; `du_stylet` : Windows le marque comme venant d'un stylet.
pub fn noter_un_clic(droit: bool, du_stylet: bool) {
    let i = if droit { 2 } else { 0 };
    CLICS[i].fetch_add(1, Relaxed);
    if du_stylet {
        CLICS[i + 1].fetch_add(1, Relaxed);
    }
}

/// Ce que l'écoute a noté jusqu'ici.
pub fn lire() -> Releve {
    let lecture = Lecture::decoder(POINTEUR.load(Relaxed), STYLET.load(Relaxed), MESURES.load(Relaxed), APPUI.load(Relaxed), ROTATION.load(Relaxed), [PENCHE_X.load(Relaxed), PENCHE_Y.load(Relaxed)]);
    Releve {
        lecture,
        jalons: JALONS.load(Relaxed),
        appui_max: APPUI_MAX.load(Relaxed) as f32 / APPUI_PLEIN as f32,
        penche_max: PENCHE_MAX.load(Relaxed),
        messages: MESSAGES.load(Relaxed),
        doigts: DOIGTS.load(Relaxed),
        clics: Clics { gauche: CLICS[0].load(Relaxed), gauche_du_stylet: CLICS[1].load(Relaxed), droit: CLICS[2].load(Relaxed), droit_du_stylet: CLICS[3].load(Relaxed) },
    }
}

/// Remet le relevé à zéro (« Recommencer »). L'écoute, elle, reste posée.
pub fn effacer() {
    for a in [&POINTEUR, &STYLET, &MESURES, &APPUI, &ROTATION, &JALONS, &APPUI_MAX, &PENCHE_MAX, &MESSAGES, &DOIGTS] {
        a.store(0, Relaxed);
    }
    PENCHE_X.store(0, Relaxed);
    PENCHE_Y.store(0, Relaxed);
    for c in &CLICS {
        c.store(0, Relaxed);
    }
}

/// Un clic de souris vient-il d'un stylet ? Windows le signe dans l'information jointe au message
/// (`GetMessageExtraInfo`) : 0xFF5157xx, le bit 0x80 mis pour un doigt, à zéro pour un stylet.
pub fn clic_du_stylet(information: u32) -> bool {
    information & 0xFFFF_FF80 == 0xFF51_5700
}

// ------------------------------------------------------------------------------------------------ l'écoute, chez Windows

/// La fenêtre de FEANOR chez Windows (0 si elle n'est pas connue), lue au démarrage.
pub fn fenetre_de(cc: &eframe::CreationContext<'_>) -> isize {
    #[cfg(windows)]
    {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        if let Ok(RawWindowHandle::Win32(w)) = cc.window_handle().map(|h| h.as_raw()) {
            return w.hwnd.get();
        }
    }
    let _ = cc;
    0
}

/// Pose l'écoute sur la fenêtre. Vrai si elle est posée (ou l'était déjà). À appeler depuis le fil de l'interface, celui
/// qui possède la fenêtre. Sans effet (faux) hors de Windows, ou pour une fenêtre inconnue.
pub fn ecouter(fenetre: isize) -> bool {
    systeme::ecouter(fenetre)
}

/// La fenêtre écoutée (0 : aucune).
#[cfg_attr(not(test), allow(dead_code))]
pub fn ecoutee() -> isize {
    systeme::ecoutee()
}

#[cfg(all(windows, target_pointer_width = "64"))]
mod systeme {
    use std::sync::atomic::{AtomicIsize, Ordering::Relaxed};

    const GWLP_WNDPROC: i32 = -4;
    const WM_NCDESTROY: u32 = 0x0082;
    const WM_LBUTTONDOWN: u32 = 0x0201;
    const WM_RBUTTONDOWN: u32 = 0x0204;
    const WM_POINTERUPDATE: u32 = 0x0245;
    const WM_POINTERDOWN: u32 = 0x0246;
    const WM_POINTERUP: u32 = 0x0247;
    const WM_POINTERENTER: u32 = 0x0249;
    const WM_POINTERLEAVE: u32 = 0x024A;
    const PT_TOUCH: u32 = 2;
    const PT_PEN: u32 = 3;

    #[repr(C)]
    #[derive(Clone, Copy)]
    #[allow(dead_code)]
    struct Point {
        x: i32,
        y: i32,
    }

    /// `POINTER_INFO` de Windows, champ pour champ (la plupart ne servent qu'à tenir la place des suivants).
    #[repr(C)]
    #[derive(Clone, Copy)]
    #[allow(dead_code)]
    struct PointerInfo {
        pointer_type: u32,
        pointer_id: u32,
        frame_id: u32,
        pointer_flags: u32,
        source_device: isize,
        hwnd_target: isize,
        pixel: Point,
        himetric: Point,
        pixel_raw: Point,
        himetric_raw: Point,
        time: u32,
        history_count: u32,
        input_data: i32,
        key_states: u32,
        performance_count: u64,
        button_change_type: i32,
    }

    /// `POINTER_PEN_INFO` de Windows, champ pour champ.
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct PointerPenInfo {
        info: PointerInfo,
        pen_flags: u32,
        pen_mask: u32,
        pressure: u32,
        rotation: u32,
        tilt_x: i32,
        tilt_y: i32,
    }

    // Les tailles de Windows sur 64 bits : une structure mal recopiée se voit à la compilation, pas devant la tablette.
    const _: () = assert!(std::mem::size_of::<PointerInfo>() == 96);
    const _: () = assert!(std::mem::size_of::<PointerPenInfo>() == 120);

    #[link(name = "user32")]
    extern "system" {
        fn SetWindowLongPtrW(fenetre: isize, index: i32, valeur: isize) -> isize;
        fn CallWindowProcW(procedure: isize, fenetre: isize, message: u32, w: usize, l: isize) -> isize;
        fn DefWindowProcW(fenetre: isize, message: u32, w: usize, l: isize) -> isize;
        fn IsWindow(fenetre: isize) -> i32;
        fn GetWindowThreadProcessId(fenetre: isize, processus: *mut u32) -> u32;
        fn GetPointerType(pointeur: u32, sorte: *mut u32) -> i32;
        fn GetPointerPenInfo(pointeur: u32, info: *mut PointerPenInfo) -> i32;
        fn GetMessageExtraInfo() -> isize;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThreadId() -> u32;
    }

    /// La fenêtre écoutée (0 : aucune) et sa procédure d'origine, celle de winit.
    static FENETRE: AtomicIsize = AtomicIsize::new(0);
    static ORIGINE: AtomicIsize = AtomicIsize::new(0);

    pub fn ecoutee() -> isize {
        FENETRE.load(Relaxed)
    }

    pub fn ecouter(fenetre: isize) -> bool {
        if fenetre == 0 {
            return false;
        }
        let deja = FENETRE.load(Relaxed);
        if deja != 0 {
            return deja == fenetre;
        }
        // SAFETY : la fenêtre existe et appartient à ce fil (vérifié juste avant) : ses messages ne sont distribués que
        // par lui, donc aucun ne peut passer entre le remplacement de la procédure et la mémorisation de l'ancienne.
        // `ecoute` a la signature d'une procédure de fenêtre et rend tout message à la procédure d'origine.
        unsafe {
            if IsWindow(fenetre) == 0 || GetWindowThreadProcessId(fenetre, std::ptr::null_mut()) != GetCurrentThreadId() {
                return false;
            }
            let origine = SetWindowLongPtrW(fenetre, GWLP_WNDPROC, ecoute as *const () as isize);
            if origine == 0 {
                return false;
            }
            ORIGINE.store(origine, Relaxed);
            FENETRE.store(fenetre, Relaxed);
        }
        true
    }

    /// La procédure posée sur la fenêtre : elle lit, puis rend le message tel quel à la procédure d'origine. Elle ne
    /// retient aucun message, n'en change aucun et ne peut pas paniquer (des atomiques et des appels à Windows).
    extern "system" fn ecoute(fenetre: isize, message: u32, w: usize, l: isize) -> isize {
        match message {
            WM_POINTERUPDATE | WM_POINTERDOWN | WM_POINTERUP | WM_POINTERENTER | WM_POINTERLEAVE => pointeur(message, w),
            WM_LBUTTONDOWN | WM_RBUTTONDOWN => {
                // SAFETY : `GetMessageExtraInfo` lit l'information jointe au message en cours de ce fil, sans rien changer.
                let information = unsafe { GetMessageExtraInfo() } as u32;
                super::noter_un_clic(message == WM_RBUTTONDOWN, super::clic_du_stylet(information));
            }
            _ => {}
        }
        let origine = ORIGINE.load(Relaxed);
        // SAFETY : Windows appelle `ecoute` pour un message de la fenêtre écoutée, avec ses quatre valeurs ; elles sont
        // rendues telles quelles à la procédure d'origine, que `SetWindowLongPtrW` a donnée pour cette fenêtre.
        unsafe {
            if origine == 0 {
                return DefWindowProcW(fenetre, message, w, l);
            }
            if message == WM_NCDESTROY {
                // La fenêtre s'en va : la procédure d'origine reprend sa place avant son dernier message.
                SetWindowLongPtrW(fenetre, GWLP_WNDPROC, origine);
                FENETRE.store(0, Relaxed);
            }
            CallWindowProcW(origine, fenetre, message, w, l)
        }
    }

    /// Lit le stylet pendant un message `WM_POINTER*` (avant winit, qui saute ensuite le reste de la trame).
    fn pointeur(message: u32, w: usize) {
        let id = (w & 0xFFFF) as u32;
        let mut sorte = 0u32;
        // SAFETY : les deux appels remplissent la valeur qu'on leur donne (un entier, puis une structure de la taille
        // que Windows attend, vérifiée à la compilation) et rendent 0 quand le pointeur n'est pas connu.
        let info = unsafe {
            if GetPointerType(id, &mut sorte) == 0 {
                return;
            }
            if sorte == PT_TOUCH {
                return super::noter_un_doigt();
            }
            if sorte != PT_PEN {
                return;
            }
            let mut info: PointerPenInfo = std::mem::zeroed();
            if GetPointerPenInfo(id, &mut info) == 0 {
                return;
            }
            info
        };
        super::noter(info.info.pointer_flags, info.pen_flags, info.pen_mask, info.pressure, info.rotation, [info.tilt_x, info.tilt_y]);
        if message == WM_POINTERLEAVE {
            super::noter_le_depart();
        }
    }
}

#[cfg(not(all(windows, target_pointer_width = "64")))]
mod systeme {
    pub fn ecouter(_fenetre: isize) -> bool {
        false
    }

    pub fn ecoutee() -> isize {
        0
    }
}
