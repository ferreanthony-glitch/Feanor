//! La taille de l'interface (passe « simplicité », 2026-10-21) : 90 %, 100 %, 125 % ou 150 %, au choix dans
//! Préférences > Apparence. Elle agit tout de suite et se retient dans les réglages de l'application (`reglages.toml`,
//! clé `echelle_interface`, en pour cent ; absente : la taille d'origine, **90 % depuis la partie 195** — Anthony : « Taille
//! d'origine de l'interface 90% » ; 100 % avant. Une taille choisie et écrite, 100 %, 125 % ou 150 %, reste la sienne ; « Par
//! défaut », à côté de la liste, remet 90 %).
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

/// La taille d'origine (partie 195 : 90 % ; 100 % avant).
pub const ECHELLE_DEFAUT: u16 = 90;

/// La taille des essais sans fenêtre (`App::sans_fenetre`) : celle de leurs mesures, d'avant la partie 195.
pub const ECHELLE_ESSAIS: u16 = 100;

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

    /// Préférences > Apparence > Tailles : la taille de l'interface, puis (partie 179.5) celle des icônes de la timeline.
    /// Partie 179.17 (Anthony : « pour le choix des valeurs dans réglages, propose plutôt un menu déroulant plutôt que plusieurs
    /// boutons. on gagnera de l'espace » ; « Aligne les ») : une grille de quatre colonnes, le nom, une liste déroulante de la même
    /// largeur pour chaque réglage, puis la pastille de couleur et « Par défaut » des lignes de la timeline, chacun dans sa colonne.
    pub(crate) fn echelle_prefs_ui(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("prefs_tailles").num_columns(4).spacing([10.0, 4.0]).show(ui, |ui| {
            ui.label("Taille de l'interface");
            let actuelle = echelle_valide(self.settings.echelle);
            let choix: Vec<(String, u16)> = ECHELLES.iter().map(|&e| (format!("{e} %"), e)).collect();
            let aide = "Agrandit ou rapetisse les textes, les boutons et les panneaux, tout de suite. Le dessin garde sa taille.";
            if let Some(e) = liste_de_valeurs(ui, "prefs_taille_interface", "Taille de l'interface", aide, &choix, actuelle) {
                self.settings.echelle = e;
                let _ = self.settings.save();
            }
            // (Partie 195 : « Par défaut » remet la taille d'origine, 90 %.)
            ui.label("");
            let r = ui.add_enabled(actuelle != ECHELLE_DEFAUT, egui::Button::new("Par défaut").small());
            if crate::bulles::bulle(r, "Taille d'origine de l'interface", "Remet la taille de l'interface à 90 %, sa taille d'origine.", "").clicked() {
                self.retablir_echelle();
            }
            ui.end_row();
            self.taille_icones_prefs_ui(ui);
            // (Parties 179.13 et 179.14 : la hauteur et la couleur de chaque ligne de la timeline.)
            self.lignes_timeline_prefs_ui(ui);
            // (Partie 205 : la vignette du dessin et la durée dans les clés assez larges des lignes de calques, éteintes d'origine.)
            self.cles_timeline_prefs_ui(ui);
        });
        ui.label(RichText::new(format!("90 % et {} points : les tailles d'origine. Pour un grand écran ou une vue fatiguée, 100 %, 125 % ou 150 %.", crate::tailleicones::TAILLE_DEFAUT)).small().weak());
    }

    /// « Par défaut » de la taille de l'interface : 90 % (partie 195).
    pub(crate) fn retablir_echelle(&mut self) {
        self.settings.echelle = ECHELLE_DEFAUT;
        let _ = self.settings.save();
    }
}

/// Partie 179.17 : la largeur commune des listes déroulantes de Préférences > Apparence > Tailles (elles sont alignées en colonne).
/// (Assez large pour « 40 pt (d'origine) » : toutes les listes ont la même largeur.)
pub(crate) const LARGEUR_LISTE: f32 = 128.0;

/// Partie 179.17 : une liste déroulante compacte (`egui::ComboBox`) de `choix` (texte, valeur), la valeur `actuelle` écrite dessus ;
/// sa bulle a le nom `nom`. Rend la valeur choisie quand elle change.
pub(crate) fn liste_de_valeurs<T: Copy + PartialEq>(ui: &mut egui::Ui, id: &str, nom: &str, aide: &str, choix: &[(String, T)], actuelle: T) -> Option<T> {
    let texte = choix.iter().find(|(_, v)| *v == actuelle).map_or_else(String::new, |(t, _)| t.clone());
    let mut nouvelle = actuelle;
    let resp = egui::ComboBox::from_id_salt(id).width(LARGEUR_LISTE).selected_text(texte).show_ui(ui, |ui| {
        for (t, v) in choix {
            ui.selectable_value(&mut nouvelle, *v, t.as_str());
        }
    });
    crate::bulles::bulle(resp.response, nom, aide, "");
    (nouvelle != actuelle).then_some(nouvelle)
}
