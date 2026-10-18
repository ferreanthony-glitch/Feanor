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
                "Le pinceau et le crayon peignent dans le fond, sous le trait (la peinture reste dans sa forme, et recoud le fond là où il était coupé) ; la gomme coupe le fond.",
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
        ui.horizontal_wrapped(|ui| {
            ui.label("Ce fond aussi sur");
            if ui.small_button("Les calques choisis").on_hover_text("Les calques choisis (Ctrl ou Maj + clic) prennent ce fond.").clicked() {
                self.run_action("fond.calques_choisis");
            }
            if ui.small_button("Tout le plan").on_hover_text("Tous les calques de dessin du plan, sauf les calques de fond du décor.").clicked() {
                self.run_action("fond.tout_le_plan");
            }
        });
        if self.paint_fond {
            ui.label(RichText::new("Strate Fond : la peinture reste dans la forme, sous le trait ; la gomme coupe le fond, le pinceau le recoud.").small().weak());
        }
    }

    /// Sous-menu « Fond sous le trait » (menu Calques).
    pub(crate) fn fond_menu(&mut self, ui: &mut egui::Ui) {
        let label = if self.layer().fond.is_some() { "Retirer du calque courant" } else { "Sur le calque courant" };
        self.action_item(ui, label, "pinceau.fond");
        self.action_item(ui, "Sur les calques choisis", "fond.calques_choisis");
        self.action_item(ui, "Sur tout le plan", "fond.tout_le_plan");
        ui.separator();
        self.action_item(ui, "Retirer des calques choisis", "fond.retirer_choisis");
        self.action_item(ui, "Couleur du projet à tous les fonds", "fond.couleur_partout");
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
                let fresh = self.new_fond();
                self.record_step();
                self.layers_mut()[li].fond = if on { None } else { Some(fresh) };
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
            // Plusieurs calques d'un coup (Anthony, 2026-10-18 : « appliquer le fond à tous les
            // calques choisis ou à tout un plan en une commande »).
            "fond.calques_choisis" | "fond.tout_le_plan" | "fond.retirer_choisis" => {
                if self.content_refuses() {
                    return true;
                }
                let whole = action == "fond.tout_le_plan";
                let remove = action == "fond.retirer_choisis";
                let targets: Vec<usize> = if whole {
                    // Tout le plan : ses calques de dessin, sauf les calques de fond du décor
                    // (ceux de toute la scène).
                    (0..self.layers().len()).filter(|&li| self.layers()[li].scene_wide.is_none()).collect()
                } else {
                    self.chosen_layers()
                };
                let targets: Vec<usize> = targets.into_iter().filter(|&li| takes_fond(&self.layers()[li])).collect();
                if targets.is_empty() {
                    self.status = "Fond sous le trait : aucun calque de dessin visé (ni dossier, ni CTG, ni vidéo).".into();
                    return true;
                }
                let fresh = self.layer().fond.unwrap_or_else(|| self.new_fond());
                self.record_step();
                for &li in &targets {
                    self.layers_mut()[li].fond = if remove { None } else { Some(fresh) };
                }
                if remove && self.layer().fond.is_none() {
                    self.paint_fond = false;
                }
                self.renderer.invalidate_all();
                let n = targets.len();
                let s = if n > 1 { "s" } else { "" };
                self.status = match (remove, whole) {
                    (true, _) => format!("Fond sous le trait : retiré de {n} calque{s}."),
                    (false, true) => format!("Fond sous le trait : posé sur les {n} calque{s} de dessin du plan (pas sur les calques de fond du décor)."),
                    (false, false) => format!("Fond sous le trait : posé sur {n} calque{s}."),
                };
            }
            // Couleur du projet donnée à tous les fonds déjà posés.
            "fond.couleur_partout" => {
                let couleur = self.new_fond().couleur;
                let n: usize = self.doc.panels.iter().flat_map(|p| &p.layers).filter(|l| l.fond.is_some_and(|f| f.couleur != couleur)).count();
                if n == 0 {
                    self.status = "Tous les fonds du projet ont déjà cette couleur.".into();
                    return true;
                }
                self.record_step();
                for l in self.doc.panels.iter_mut().flat_map(|p| p.layers.iter_mut()) {
                    if let Some(f) = l.fond.as_mut() {
                        f.couleur = couleur;
                    }
                }
                self.renderer.invalidate_all();
                self.status = format!("Couleur des fonds : {n} calque(s) changé(s) dans le projet.");
            }
            _ => return false,
        }
        true
    }

    /// Un fond neuf : la couleur du projet (Réglages du projet), blanc d'office.
    pub(crate) fn new_fond(&self) -> FondReglages {
        let d = FondReglages::default();
        FondReglages { couleur: self.doc.properties.fond.unwrap_or(d.couleur), ..d }
    }
}

/// Le calque peut avoir un fond sous son trait : un calque de dessin (ni dossier, ni CTG, ni
/// vidéo).
fn takes_fond(l: &crate::document::Layer) -> bool {
    !l.folder && l.ctg.is_none() && l.video.is_none()
}
