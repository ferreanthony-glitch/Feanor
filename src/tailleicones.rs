//! Partie 179.5 (Anthony, sur la capture du Storyboard : « Les icônes sur la ligne de séquence gardent la même taille que sur le
//! reste des autres lignes. Pareil pour les icônes de l'interface de visionnage au-dessus de la timeline. Tous les icônes doivent
//! avoir cette taille. La taille des icônes et de l'interface doit être réglable dans les préférences. »).
//!
//! **Une seule taille d'icônes** pour la timeline : celle des icônes des lignes Plans et Caméra (13 points d'origine). Elle sert aux
//! en-têtes des rangées (Plans, Caméra, Calques), aux commandes de la ligne « Calques », aux réglages et indicateurs du calque actif,
//! aux pistes, et à la barre de lecture au-dessus de la timeline (début, lire, boucle, nouveau plan, la liste des plans, le son,
//! « Plus… », la légende des clés), dans le Storyboard comme dans l'Animation. Les cases qui les portent grandissent ou rapetissent
//! avec elle (`k`) ; la hauteur des rangées ne change pas (c'est le rôle de la taille de l'interface, `echelle.rs`).
//!
//! Un réglage du poste (`reglages.toml`, clé `taille_icones`, en points ; absente : 13), pas du projet. Préférences > Apparence >
//! Tailles, à côté de la taille de l'interface ; il agit à l'image suivante.
//!
//! La taille en cours est retenue pour le fil de l'interface (`poser`, à chaque image, depuis les réglages) : les fonctions de
//! disposition qui n'ont pas l'application sous la main (`layerheads::case_de_colonne`…) la lisent ici.

use std::cell::Cell;

use eframe::egui::{self, RichText};

use crate::app::App;

/// Les tailles proposées, en points.
pub const TAILLES: [u8; 6] = [11, 12, 13, 14, 15, 16];

/// La taille d'origine : celle des icônes des lignes Plans et Caméra avant la partie 179.5.
pub const TAILLE_DEFAUT: u8 = 13;

thread_local! {
    static TAILLE: Cell<f32> = const { Cell::new(TAILLE_DEFAUT as f32) };
}

/// La taille proposée la plus proche de `t` (un fichier de réglages écrit à la main).
pub fn taille_valide(t: u8) -> u8 {
    TAILLES.iter().copied().min_by_key(|e| e.abs_diff(t)).unwrap_or(TAILLE_DEFAUT)
}

/// Dans le fichier de réglages, la taille d'origine n'est pas écrite.
pub fn est_defaut(t: &u8) -> bool {
    *t == TAILLE_DEFAUT
}

pub fn defaut() -> u8 {
    TAILLE_DEFAUT
}

/// Retient la taille `t` pour les images qui suivent (sur ce fil).
pub fn poser(t: u8) {
    TAILLE.with(|c| c.set(f32::from(taille_valide(t))));
}

/// La taille des icônes de la timeline, en points.
pub fn icone() -> f32 {
    TAILLE.with(|c| c.get())
}

/// Le rapport à la taille d'origine : les cases des icônes le suivent.
pub fn k() -> f32 {
    icone() / f32::from(TAILLE_DEFAUT)
}

/// Une largeur d'origine (pour les icônes de 13 points) à la taille en cours, arrondie au point.
pub fn mesure(origine: f32) -> f32 {
    (origine * k()).round()
}

/// La case d'une icône de la barre de lecture de la timeline (20 × 20 points d'origine ; 28 × 26 avant la partie 179.5).
pub fn case_de_barre() -> [f32; 2] {
    let c = (icone() + 7.0).round();
    [c, c]
}

impl App {
    /// Préférences > Apparence > Tailles > Taille des icônes : une petite icône par taille, à sa taille, la taille choisie allumée.
    pub(crate) fn taille_icones_prefs_ui(&mut self, ui: &mut egui::Ui) {
        // (Le nom sur sa ligne, les six choix dessous : sur une seule ligne, les plus grandes sortaient de la fenêtre.)
        ui.label("Taille des icônes");
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.spacing_mut().item_spacing.x = 3.0;
            let actuelle = taille_valide(self.settings.taille_icones);
            for t in TAILLES {
                let r = ui.selectable_label(actuelle == t, RichText::new(egui_phosphor::regular::PLUS_SQUARE).size(f32::from(t)));
                let nom = if t == TAILLE_DEFAUT { format!("Icônes de {t} points (d'origine)") } else { format!("Icônes de {t} points") };
                if crate::bulles::bulle(r, &nom, "Toutes les icônes de la timeline et de sa barre de lecture, dans le Storyboard et l'Animation, tout de suite. La hauteur des rangées ne change pas : pour tout agrandir, la taille de l'interface.", "").clicked() && actuelle != t {
                    self.settings.taille_icones = t;
                    poser(t);
                    let _ = self.settings.save();
                }
            }
        });
    }
}
