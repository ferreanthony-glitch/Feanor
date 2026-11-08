//! Fichier > Exporter en vidéo / Exporter en image / Exporter le son / Exporter pour la production (Anthony,
//! 2026-11-08 : « la fenêtre Exporter est trop complexe »). Quatre sous-menus ; chaque entrée ouvre tout de suite la boîte
//! de réglages de son export (ou le dialogue d'enregistrement quand l'export n'a rien à régler), sans passer par la grande
//! fenêtre. Le rangement et les cas douteux sont décidés dans `docs/INTERFACE.md`, partie 130.
//!
//! Les actions et leurs raccourcis sont ceux d'avant : seul l'endroit d'où on les lance change.

use eframe::egui;

use crate::app::App;

/// Une entrée : le texte du menu et l'action. Un texte vide est un trait de séparation.
pub(crate) type Entree = (&'static str, &'static str);

pub(crate) const SEPARATEUR: Entree = ("", "");

/// Les quatre sous-menus de Fichier, dans l'ordre où ils se lisent.
pub(crate) const SOUS_MENUS: [(&str, &[Entree]); 4] = [
    (
        "Exporter en vidéo",
        &[
            ("Film (MP4, ProRes, DNxHR)…", "fichier.exporter_film"),
            ("GIF animé…", "fichier.exporter_gif"),
        ],
    ),
    (
        "Exporter en image",
        &[
            ("Image courante…", "fichier.exporter_image_courante"),
            ("Une image par image du film…", "fichier.exporter_images"),
            ("Une image par plan…", "fichier.exporter_images_par_plan"),
            ("Une image par dessin clé…", "fichier.exporter_images_cles"),
            SEPARATEUR,
            ("Storyboard en PDF…", "fichier.exporter_pdf"),
            ("Planche des vignettes…", "fichier.exporter_planche"),
            ("Color script…", "fichier.exporter_colorscript"),
        ],
    ),
    (
        "Exporter le son",
        &[
            ("Son mixé…", "fichier.exporter_son"),
            ("Un son par piste…", "fichier.exporter_son_par_piste"),
            ("Un son par plan…", "fichier.exporter_son_par_plan"),
        ],
    ),
    (
        "Exporter pour la production",
        &[
            ("Pour le montage (EDL, XML, FCPXML, OpenTimelineIO)…", "fichier.exporter_montage"),
            ("Calques séparés…", "fichier.exporter_calques"),
            ("Caméra (JSON, CSV)…", "fichier.exporter_camera"),
            SEPARATEUR,
            ("Fiche technique (XLSX, CSV)…", "fichier.exporter_fiche"),
            ("Liste des plans (CSV, JSON)…", "fichier.exporter_liste_plans"),
            ("Rapport de numérotation…", "fichier.exporter_renumerotation"),
            SEPARATEUR,
            ("Relecture (la page à envoyer au réalisateur)…", "fichier.exporter_relecture"),
            ("Journal des différences…", "envoi.journal"),
            ("Paquet de livraison…", "fichier.exporter_paquet"),
            ("Kitsu…", "fichier.kitsu_publier"),
            SEPARATEUR,
            ("Noms des plans, série et épisode…", "projet.noms_production"),
        ],
    ),
];

impl App {
    /// Les quatre sous-menus d'export de Fichier.
    pub(crate) fn export_menus(&mut self, ui: &mut egui::Ui) {
        for (titre, entrees) in SOUS_MENUS {
            ui.menu_button(titre, |ui| {
                for (texte, action) in entrees {
                    if texte.is_empty() {
                        ui.separator();
                    } else {
                        self.action_item(ui, texte, action);
                    }
                }
            });
        }
    }
}
