//! Fond sous le trait, côté interface (`fond.rs`) : l'option du pinceau et du crayon (choix
//! d'Anthony, 2026-10-18 : « une option de pinceau, comme le peint derrière »), allumée pour le
//! calque courant ; dessous, ses réglages et la bascule Trait / Fond (où peignent le pinceau, le
//! crayon et la gomme : dans le trait, ou dans le fond, sous le trait, sans jamais le toucher).

use eframe::egui::{self, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::fond::FondReglages;

impl App {
    /// « Fond sous le trait » (pinceau et crayon), à côté de « Peint derrière ».
    pub(crate) fn fond_toggle_ui(&mut self, ui: &mut egui::Ui) {
        let on = self.layer().fond.is_some();
        let resp = self.bouton_etat(
            ui,
            on,
            ph::USER_SQUARE,
            "Fond sous le trait",
            "Le calque prend un fond sous son trait : tout ce que le trait enferme, plus un léger débord, en une seule \
             forme qui se recalcule à chaque trait. On peut changer sa couleur, et peindre dedans (bascule Fond) sans \
             toucher au trait.",
            "pinceau.fond",
        );
        if resp.clicked() {
            self.run_action("pinceau.fond");
        }
    }

    /// Les réglages du fond du calque courant et la bascule Trait / Fond (pinceau, crayon, gomme).
    pub(crate) fn fond_options_ui(&mut self, ui: &mut egui::Ui) {
        let Some(r) = self.layer().fond else { return };
        ui.horizontal(|ui| {
            ui.label("Peindre dans");
            let mut fond = self.paint_fond;
            let key = self.raccourci("pinceau.strate_fond");
            crate::bulles::choix_a(ui, &mut fond, false, ph::PEN_NIB, "Le trait", "Le pinceau, le crayon et la gomme travaillent le trait.", &key, "pinceau.strate_fond");
            crate::bulles::choix_a(
                ui,
                &mut fond,
                true,
                ph::USER_SQUARE,
                "Le fond",
                "Le pinceau, le crayon et la gomme peignent dans le fond, sous le trait : la peinture reste dans sa forme.",
                &key,
                "pinceau.strate_fond",
            );
            if fond != self.paint_fond {
                self.run_action("pinceau.strate_fond");
            }
        });
        let mut next = r;
        let mut changed = None;
        ui.horizontal(|ui| {
            ui.label("Couleur du fond");
            let resp = ui.color_edit_button_srgb(&mut next.couleur);
            if self.edited(&resp) {
                changed = Some(next);
            }
        });
        let mut debord = next.debord as f32;
        let resp = ui.add(egui::Slider::new(&mut debord, 0.0..=30.0).text("Débord (px)").fixed_decimals(0));
        let resp = crate::bulles::bulle(resp, "Débord", "Pixels de fond au-delà du bord extérieur du trait.", "");
        if self.edited(&resp) {
            next.debord = debord.round() as u8;
            changed = Some(next);
        }
        let mut trous = next.trous as f32;
        let resp = ui.add(egui::Slider::new(&mut trous, 0.0..=40.0).text("Fermer les trous (px)").fixed_decimals(0));
        let resp = crate::bulles::bulle(resp, "Fermer les trous", "Un trou du trait jusqu'à deux fois cette valeur ne laisse pas fuir le fond.", "");
        if self.edited(&resp) {
            next.trous = trous.round() as u8;
            changed = Some(next);
        }
        let resp = ui.checkbox(&mut next.bord, "Le bord de l'image ferme la forme");
        let resp = crate::bulles::bulle(resp, "Le bord de l'image ferme la forme", "Un personnage coupé par le cadre (plan poitrine) a son fond jusqu'au bord.", "");
        if self.edited(&resp) {
            changed = Some(next);
        }
        if let Some(n) = changed {
            let li = self.cur_layer;
            self.layers_mut()[li].fond = Some(n);
            self.renderer.invalidate_all();
        }
        if self.paint_fond {
            ui.label(RichText::new("Strate Fond : la peinture reste dans la forme du fond, sous le trait.").small().weak());
        }
    }

    /// Commandes du fond ; vrai si `action` en était une.
    pub(crate) fn fond_action(&mut self, action: &str) -> bool {
        match action {
            "pinceau.fond" => {
                if self.refuse_on_folder() {
                    return true;
                }
                let li = self.cur_layer;
                let on = self.layers()[li].fond.is_some();
                self.record_step();
                self.layers_mut()[li].fond = if on { None } else { Some(FondReglages::default()) };
                if on {
                    self.paint_fond = false;
                }
                self.renderer.invalidate_all();
                self.status = if on {
                    "Fond sous le trait : retiré de ce calque (Ctrl+Z le remet, avec ce qu'on y avait peint).".into()
                } else {
                    "Fond sous le trait : chaque dessin du calque a maintenant un fond, recalculé à chaque trait.".into()
                };
            }
            "pinceau.strate_fond" => {
                if self.layer().fond.is_none() {
                    self.status = "Ce calque n'a pas de fond : allume d'abord « Fond sous le trait ».".into();
                    return true;
                }
                self.end_stroke();
                self.paint_fond = !self.paint_fond;
                self.status = if self.paint_fond { "Peindre dans : le fond (sous le trait)." } else { "Peindre dans : le trait." }.into();
            }
            _ => return false,
        }
        true
    }
}
