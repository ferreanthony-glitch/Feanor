//! Le mode concentration (passe « simplicité », 2026-10-21) : une touche cache tous les panneaux et toutes les barres et ne
//! laisse que le dessin, avec une barre minimale ; la même touche remet tout, exactement comme avant.
//!
//! C'est une **couche posée par-dessus**, pas une disposition : tant qu'il est allumé, l'image d'écran ne dessine pas les
//! menus, les onglets de projets, les barres, la barre d'état ni le dock (`App::frame`), et le dessin prend toute la
//! fenêtre. Le dock, les dispositions des quatre espaces de travail, les panneaux tirés à la main : rien de cela n'est
//! touché, ni lu ni écrit autrement qu'à l'ordinaire (`remember_layout` continue de relire le dock, qui n'a pas bougé).
//! Les ensembles de panneaux propres à chaque onglet (travail d'un autre chantier) ne sont donc pas concernés.
//!
//! **Sans latence** : allumer ou éteindre change un booléen ; l'image suivante dessine l'autre mise en page, rien n'est
//! recalculé de plus qu'à un changement d'onglet. Les panneaux posés dans leur propre fenêtre (sur un autre écran) restent
//! où ils sont : les fermer puis les rouvrir coûterait des fenêtres Windows, et ils ne gênent pas le dessin.
//!
//! La touche est `vue.concentration` (Z, comme « zen », libre dans le jeu de SHOT comme dans celui de Storyboard Pro).
//! Aussi dans le menu Affichage, la palette de commandes (Ctrl+K) et le bouton de la barre minimale.

use eframe::egui::{self, vec2, Align, Layout, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::dock::Pane;

/// Hauteur de la barre minimale, en points.
pub const BARRE_H: f32 = 26.0;

impl App {
    /// Le mode concentration est-il à l'écran ? Jamais sur l'écran d'accueil : sans projet, il n'y a rien à garder.
    pub(crate) fn concentration_active(&self) -> bool {
        self.confort.concentration && !self.welcome_shown()
    }

    /// Allume ou éteint le mode concentration (la touche, le menu, le bouton de la barre minimale).
    pub(crate) fn basculer_concentration(&mut self) {
        if !self.confort.concentration && self.welcome_shown() {
            self.status = "Le mode concentration attend qu'un projet soit ouvert (ou « Dessiner des vignettes »).".into();
            return;
        }
        self.confort.concentration = !self.confort.concentration;
        // Un geste en cours ne traverse pas le changement d'écran.
        self.end_stroke();
        self.status = if self.confort.concentration {
            let touche = self.raccourci("vue.concentration");
            if touche.is_empty() {
                "Mode concentration : seul le dessin reste. Le bouton en haut à droite remet tout.".into()
            } else {
                format!("Mode concentration : seul le dessin reste. {touche} remet tout.")
            }
        } else {
            "Mode concentration quitté : les panneaux et les barres sont revenus.".into()
        };
    }

    /// Ce que l'image d'écran dessine à la place des menus, des barres et du dock : la barre minimale, puis le dessin.
    pub(crate) fn concentration_ui(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("concentration_barre").exact_height(BARRE_H).show(ctx, |ui| self.concentration_barre(ui));
        // (Le panneau Références n'est pas à l'écran : ses dépôts de fichiers ne le visent plus.)
        self.refs.pane_rect = None;
        egui::CentralPanel::default().frame(egui::Frame::new()).show(ctx, |ui| {
            self.pane_ui(ui, Pane::Canvas);
        });
        self.show_detached(ctx);
    }

    /// La barre minimale : où l'on est, l'outil et la couleur, annuler et rétablir, et la sortie.
    fn concentration_barre(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
            ui.label(RichText::new(format!("{}  {}", self.tool.icon(), self.tool.label())).strong());
            let (zone, _) = ui.allocate_exact_size(vec2(14.0, 14.0), egui::Sense::hover());
            let [r, g, b] = self.color;
            ui.painter().rect_filled(zone, 3.0, egui::Color32::from_rgb(r, g, b));
            ui.painter().rect_stroke(zone, 3.0, egui::Stroke::new(1.0_f32, crate::theme::bord()), egui::StrokeKind::Inside);
            ui.separator();
            ui.label(RichText::new(crate::planversui::titre_long_avec_version(&self.doc, self.cur_panel)).color(crate::theme::texte_doux()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self.bouton(ui, ph::ARROWS_IN, "Quitter le mode concentration", "Les panneaux et les barres reviennent exactement comme avant.", "vue.concentration").clicked() {
                    self.basculer_concentration();
                }
                ui.separator();
                if self.bouton(ui, ph::ARROW_CLOCKWISE, "Rétablir", "", "edition.retablir").clicked() {
                    self.run_action("edition.retablir");
                }
                if self.bouton(ui, ph::ARROW_COUNTER_CLOCKWISE, "Annuler", "", "edition.annuler").clicked() {
                    self.run_action("edition.annuler");
                }
            });
        });
    }
}
