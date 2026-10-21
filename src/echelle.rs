//! La taille de l'interface (passe « simplicité », 2026-10-21) : 90 %, 100 %, 125 % ou 150 %, au choix dans
//! Préférences > Apparence. Elle agit tout de suite et se retient dans les réglages de l'application (`reglages.toml`,
//! clé `echelle_interface`, en pour cent ; absente : 100 %).
//!
//! C'est le « facteur de zoom » d'egui (`Context::set_zoom_factor`), celui qui multiplie les pixels par point de l'écran :
//! tout ce que SHOT dessine en points grandit ou rapetisse, le texte, les boutons, les panneaux. Le dessin, lui, reste le
//! dessin : sa vue (`View`) et les positions du stylet sont en points, comme avant ; ce qui passe par les pixels (la
//! composition sur la carte graphique, les vignettes, les aperçus de pinceaux, le « zoom % » de la barre d'état, « 1 pixel
//! du dessin = 1 pixel de l'écran ») lit `Context::pixels_per_point`, qui contient déjà le facteur. egui-winit divise les
//! positions de la souris et du stylet par ce même nombre (`pixels_per_point(egui_ctx, window)`) : un point d'écran reste
//! un point du dessin à toutes les échelles.
//!
//! Le raccourci d'egui (Ctrl +, Ctrl -, Ctrl 0 qui changent ce facteur sans rien retenir) est coupé : Ctrl+0 est « Ajuster à
//! la fenêtre » dans SHOT, et il remettait en plus l'interface à 100 %.

use eframe::egui::{self, RichText};

use crate::app::App;

/// Les tailles proposées, en pour cent.
pub const ECHELLES: [u16; 4] = [90, 100, 125, 150];

/// La taille d'origine.
pub const ECHELLE_DEFAUT: u16 = 100;

/// La taille proposée la plus proche de `pour_cent` (un fichier de réglages écrit à la main, ou d'un autre SHOT).
pub fn echelle_valide(pour_cent: u16) -> u16 {
    ECHELLES.iter().copied().min_by_key(|e| e.abs_diff(pour_cent)).unwrap_or(ECHELLE_DEFAUT)
}

/// Le facteur de zoom d'egui pour cette taille.
pub fn facteur(pour_cent: u16) -> f32 {
    f32::from(echelle_valide(pour_cent)) / 100.0
}

/// Dans le fichier de réglages, l'échelle d'origine n'est pas écrite.
pub fn est_defaut(pour_cent: &u16) -> bool {
    *pour_cent == ECHELLE_DEFAUT
}

pub fn defaut() -> u16 {
    ECHELLE_DEFAUT
}

impl App {
    /// Pose la taille choisie dans egui si elle n'y est pas encore : un test d'égalité par image, rien d'autre. Un changement
    /// prend effet à l'image suivante (egui recalcule les pixels par point au début de l'image).
    pub(crate) fn appliquer_echelle(&mut self, ctx: &egui::Context) {
        let cible = facteur(self.settings.echelle);
        if (ctx.zoom_factor() - cible).abs() > 1e-4 {
            ctx.set_zoom_factor(cible);
            // La vue du dessin reste calée sur ce qui est affiché : elle se recadre d'une image à l'autre toute seule
            // (les positions sont en points), rien à refaire ici.
            ctx.request_repaint();
        }
    }

    /// Préférences > Apparence > Taille de l'interface.
    pub(crate) fn echelle_prefs_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Échelle");
            let actuelle = echelle_valide(self.settings.echelle);
            for e in ECHELLES {
                let r = ui.selectable_label(actuelle == e, RichText::new(format!("{e} %")));
                if crate::bulles::bulle(r, &format!("{e} %"), "Agrandit ou rapetisse les textes, les boutons et les panneaux, tout de suite. Le dessin garde sa taille.", "").clicked() && actuelle != e {
                    self.settings.echelle = e;
                    let _ = self.settings.save();
                }
            }
        });
        ui.label(RichText::new("100 % : la taille d'origine. Pour un grand écran ou une vue fatiguée, 125 % ou 150 %.").small().weak());
    }
}
