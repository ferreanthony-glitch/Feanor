//! Partie 179.13 (Anthony : « Ajoute dans les paramètres la possibilité d'ajuster la taille de chaque ligne. Séquence, plan, caméra,
//! calques. ») et 179.14 (« Ajoute aussi dans les réglages la possibilité de donner une couleur à chaque ligne. »).
//!
//! La hauteur et la couleur des lignes de la timeline du Storyboard : Séquence, Plans, Caméra, Calques (chaque calque). Des
//! réglages du poste (`reglages.toml`), dans Préférences > Apparence > Tailles ; d'origine, les hauteurs d'avant (20, 26, 22, 40)
//! et les couleurs du thème. L'espace Montage garde les siennes.
//!
//! Une ligne ne descend jamais sous la taille des icônes et une marge (`min_ligne`) : ses icônes restent lisibles et cliquables ;
//! un calque, qui a deux rangées, jamais sous deux fois cela.
//!
//! La couleur teinte le fond de la ligne (sa case dans la colonne de gauche et sa rangée dans la bande) : elle est fondue avec le
//! fond du thème (`teinte`, 35 %), pour que le texte et les icônes, écrits dans les couleurs du thème, restent lisibles quelle que
//! soit la couleur choisie. Les blocs des plans, des séquences, des clés, la couleur d'étiquette d'un calque gardent les leurs.
//!
//! Comme la taille des icônes (`tailleicones.rs`), les valeurs en cours sont retenues pour le fil de l'interface (`poser`, à
//! chaque image, depuis les réglages) : la disposition des rangées (`montagerows::Rows`) les lit ici.

use std::cell::Cell;

use eframe::egui::{self, Color32, RichText};

use crate::app::App;
use crate::settings::Settings;

pub const SEQ_DEFAUT: u8 = crate::montagerows::SB_SEQ_H as u8;
pub const PLANS_DEFAUT: u8 = crate::montagerows::SB_PLAN_H as u8;
pub const CAMERA_DEFAUT: u8 = crate::montagerows::SB_CAM_H as u8;
pub const CALQUES_DEFAUT: u8 = crate::montagerows::LAYER_H as u8;

/// Les hauteurs proposées pour chaque ligne, en points.
pub const SEQ_CHOIX: [u8; 5] = [18, 20, 24, 28, 32];
pub const PLANS_CHOIX: [u8; 6] = [22, 26, 32, 40, 48, 56];
pub const CAMERA_CHOIX: [u8; 5] = [18, 22, 26, 30, 36];
pub const CALQUES_CHOIX: [u8; 6] = [36, 40, 44, 48, 56, 64];

/// La part de la couleur d'une ligne dans son fond (le reste : le fond du thème).
pub const PART_DE_LA_COULEUR: f32 = 0.35;

pub fn seq_defaut() -> u8 {
    SEQ_DEFAUT
}
pub fn plans_defaut() -> u8 {
    PLANS_DEFAUT
}
pub fn camera_defaut() -> u8 {
    CAMERA_DEFAUT
}
pub fn calques_defaut() -> u8 {
    CALQUES_DEFAUT
}
pub fn seq_est_defaut(v: &u8) -> bool {
    *v == SEQ_DEFAUT
}
pub fn plans_est_defaut(v: &u8) -> bool {
    *v == PLANS_DEFAUT
}
pub fn camera_est_defaut(v: &u8) -> bool {
    *v == CAMERA_DEFAUT
}
pub fn calques_est_defaut(v: &u8) -> bool {
    *v == CALQUES_DEFAUT
}

/// Les quatre lignes réglables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ligne {
    Sequence,
    Plans,
    Camera,
    Calques,
}

impl Ligne {
    pub const TOUTES: [Ligne; 4] = [Ligne::Sequence, Ligne::Plans, Ligne::Camera, Ligne::Calques];

    pub fn nom(self) -> &'static str {
        match self {
            Ligne::Sequence => "Séquence",
            Ligne::Plans => "Plans",
            Ligne::Camera => "Caméra",
            Ligne::Calques => "Calques",
        }
    }

    fn rang(self) -> usize {
        self as usize
    }

    pub fn choix(self) -> &'static [u8] {
        match self {
            Ligne::Sequence => &SEQ_CHOIX,
            Ligne::Plans => &PLANS_CHOIX,
            Ligne::Camera => &CAMERA_CHOIX,
            Ligne::Calques => &CALQUES_CHOIX,
        }
    }

    pub fn defaut(self) -> u8 {
        match self {
            Ligne::Sequence => SEQ_DEFAUT,
            Ligne::Plans => PLANS_DEFAUT,
            Ligne::Camera => CAMERA_DEFAUT,
            Ligne::Calques => CALQUES_DEFAUT,
        }
    }

    /// La hauteur réglée (non bornée) dans `s`.
    pub fn reglee(self, s: &Settings) -> u8 {
        match self {
            Ligne::Sequence => s.hauteur_sequence,
            Ligne::Plans => s.hauteur_plans,
            Ligne::Camera => s.hauteur_camera,
            Ligne::Calques => s.hauteur_calques,
        }
    }

    pub fn regler(self, s: &mut Settings, v: u8) {
        match self {
            Ligne::Sequence => s.hauteur_sequence = v,
            Ligne::Plans => s.hauteur_plans = v,
            Ligne::Camera => s.hauteur_camera = v,
            Ligne::Calques => s.hauteur_calques = v,
        }
    }

    pub fn couleur(self, s: &Settings) -> Option<[u8; 3]> {
        match self {
            Ligne::Sequence => s.couleur_ligne_sequence,
            Ligne::Plans => s.couleur_ligne_plans,
            Ligne::Camera => s.couleur_ligne_camera,
            Ligne::Calques => s.couleur_ligne_calques,
        }
    }

    pub fn colorer(self, s: &mut Settings, c: Option<[u8; 3]>) {
        match self {
            Ligne::Sequence => s.couleur_ligne_sequence = c,
            Ligne::Plans => s.couleur_ligne_plans = c,
            Ligne::Camera => s.couleur_ligne_camera = c,
            Ligne::Calques => s.couleur_ligne_calques = c,
        }
    }
}

thread_local! {
    static HAUTEURS: Cell<[f32; 4]> = const { Cell::new([SEQ_DEFAUT as f32, PLANS_DEFAUT as f32, CAMERA_DEFAUT as f32, CALQUES_DEFAUT as f32]) };
    static COULEURS: Cell<[Option<[u8; 3]>; 4]> = const { Cell::new([None; 4]) };
}

/// La hauteur proposée la plus proche de `v` pour la ligne `l` (un fichier de réglages écrit à la main).
pub fn valide(l: Ligne, v: u8) -> u8 {
    l.choix().iter().copied().min_by_key(|c| c.abs_diff(v)).unwrap_or(l.defaut())
}

/// Retient les hauteurs et les couleurs de `s` pour les images qui suivent (sur ce fil).
pub fn poser(s: &Settings) {
    HAUTEURS.with(|h| h.set(Ligne::TOUTES.map(|l| f32::from(valide(l, l.reglee(s))))));
    COULEURS.with(|c| c.set(Ligne::TOUTES.map(|l| l.couleur(s))));
}

/// La hauteur la plus petite d'une ligne : une case d'icône (la taille des icônes et 5 points).
pub fn min_ligne() -> f32 {
    (crate::tailleicones::icone() + 5.0).round()
}

/// La hauteur de la ligne `l` à l'écran (jamais sous `min_ligne`, deux fois pour un calque).
pub fn hauteur(l: Ligne) -> f32 {
    let v = HAUTEURS.with(|h| h.get())[l.rang()];
    let min = if l == Ligne::Calques { 2.0 * min_ligne() } else { min_ligne() };
    v.max(min)
}

/// Le fond `base` d'une ligne `l`, teinté de sa couleur s'il y en a une (fondue à `PART_DE_LA_COULEUR` avec le fond du thème).
pub fn teinte(l: Ligne, base: Color32) -> Color32 {
    match COULEURS.with(|c| c.get())[l.rang()] {
        Some([r, g, b]) => base.lerp_to_gamma(Color32::from_rgb(r, g, b), PART_DE_LA_COULEUR),
        None => base,
    }
}

impl App {
    /// Préférences > Apparence > Tailles > Hauteur et couleur des lignes de la timeline : pour chaque ligne, ses hauteurs (la choisie
    /// allumée), sa pastille de couleur (le champ de couleur commun de FEANOR) et « Par défaut » quand elle en a une.
    pub(crate) fn lignes_timeline_prefs_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("Lignes de la timeline");
        let mut change = false;
        for l in Ligne::TOUTES {
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(format!("Ligne {}", l.nom()));
                let actuelle = valide(l, l.reglee(&self.settings));
                for &v in l.choix() {
                    let r = ui.selectable_label(actuelle == v, RichText::new(format!("{v}")));
                    let nom = if v == l.defaut() { format!("{} : {v} points (d'origine)", l.nom()) } else { format!("{} : {v} points", l.nom()) };
                    if crate::bulles::bulle(r, &nom, "La hauteur de cette ligne de la timeline du Storyboard, tout de suite. Elle ne descend jamais sous la taille des icônes.", "").clicked() && actuelle != v {
                        l.regler(&mut self.settings, v);
                        change = true;
                    }
                }
                ui.add_space(8.0);
                let theme = Color32::from_gray(90);
                let mut c = l.couleur(&self.settings).unwrap_or([theme.r(), theme.g(), theme.b()]);
                let resp = crate::champcouleur::srgb(ui, &mut c);
                let resp = crate::bulles::bulle(resp, &format!("Couleur de la ligne {}", l.nom()), "Teinte le fond de la ligne, dans la colonne et dans la bande (fondue avec le fond, pour garder le texte lisible). Les plans, les séquences, les clés et les calques gardent leurs couleurs.", "");
                if self.edited(&resp) {
                    l.colorer(&mut self.settings, Some(c));
                    change = true;
                }
                if l.couleur(&self.settings).is_some() {
                    let r = ui.small_button("Par défaut");
                    if crate::bulles::bulle(r, &format!("Couleur d'origine de la ligne {}", l.nom()), "La couleur du thème, comme avant.", "").clicked() {
                        l.colorer(&mut self.settings, None);
                        change = true;
                    }
                }
            });
        }
        if change {
            poser(&self.settings);
            let _ = self.settings.save();
        }
    }
}
