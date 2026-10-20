//! Fond sous le trait, côté interface (`fond.rs`) : un réglage du **calque**, avec son bouton sur la ligne
//! du calque (choix d'Anthony, 2026-10-20 : « depuis le calque, en bouton, et pas depuis le pinceau » ;
//! le 2026-10-18 c'était une option du pinceau et du crayon, à côté de « Peint derrière ») ; dans les
//! réglages du pinceau, la bascule Trait / Fond (où peignent le pinceau, le crayon et la gomme : dans le
//! trait, ou dans le fond, sous le trait, sans jamais le toucher) et les réglages du fond du calque courant.

use eframe::egui::{self, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::fond::FondReglages;

impl App {
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
            let hint = match self.tool {
                crate::app::Tool::Fill => "Strate Fond : un clic remplit de la couleur courante la zone fermée par le trait, sous le trait (et y recoud le fond s'il était coupé).",
                crate::app::Tool::Cutter => "Strate Fond : un clic retire du fond la zone fermée par le trait (le pot de peinture ou le pinceau la remet).",
                _ => "Strate Fond : la peinture reste dans la forme, sous le trait ; la gomme coupe le fond, le pinceau le recoud.",
            };
            ui.label(RichText::new(hint).small().weak());
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

    /// Allume ou éteint le fond sous le trait du calque `li` (une étape d'annulation, le calque courant ne
    /// change pas). Le fond est un réglage du **calque** : le bouton de sa ligne (panneau Calques, en-tête de
    /// la timeline, barre de commandes du panneau) et l'action `pinceau.fond` (qui vise le calque courant) font
    /// la même chose. Avant le 2026-10-20 c'était un bouton des réglages du pinceau et du crayon ; il n'a
    /// jamais été retenu dans les réglages de pinceau (c'était déjà ce champ du calque, `Layer::fond`) : les
    /// projets et les réglages enregistrés se lisent donc sans aucune conversion.
    pub(crate) fn toggle_fond(&mut self, li: usize) {
        if self.content_refuses() {
            return;
        }
        if !self.layers().get(li).is_some_and(takes_fond) {
            self.status = "Fond sous le trait : ce calque n'en prend pas (ni dossier, ni CTG, ni vidéo).".into();
            return;
        }
        self.end_stroke();
        let on = self.layers()[li].fond.is_some();
        let fresh = self.new_fond();
        self.record_step();
        self.layers_mut()[li].fond = if on { None } else { Some(fresh) };
        if on && li == self.cur_layer {
            self.paint_fond = false;
        }
        self.renderer.invalidate_all();
        self.status = if on {
            "Fond sous le trait : retiré de ce calque (Ctrl+Z le remet, avec ce qu'on y avait peint).".into()
        } else {
            "Fond sous le trait : chaque dessin du calque a maintenant un fond, recalculé à chaque trait.".into()
        };
    }

    /// Le bouton « Fond sous le trait » d'un calque : allumé quand le calque en a un. Il est sur la ligne du
    /// calque (panneau Calques) et dans les commandes du panneau ; rien pour un dossier, un calque CTG ou vidéo
    /// (ils n'en prennent pas).
    pub(crate) fn fond_layer_button(&mut self, ui: &mut egui::Ui, li: usize, small: bool) -> Option<egui::Response> {
        let l = self.layers().get(li)?;
        if !takes_fond(l) {
            return None;
        }
        let on = l.fond.is_some();
        let size = if small { 12.0 } else { crate::bulles::TAILLE };
        let key = self.raccourci("pinceau.fond");
        let about = if on {
            "Ce calque a un fond sous son trait : tout ce que le trait enferme, plus un léger débord, en une seule forme qui se recalcule à chaque trait. Clic : le retirer. Ses réglages (couleur, débord, trous) sont dans les réglages du pinceau, et la bascule Trait / Fond dit où l'on peint."
        } else {
            "Donner un fond à ce calque : tout ce que le trait enferme, plus un léger débord, en une seule forme sous le trait, qui se recalcule à chaque trait. On peut changer sa couleur, et peindre dedans (bascule Fond) sans toucher au trait."
        };
        let resp = ui.add(egui::Button::new(crate::bulles::encre_etat(RichText::new(ph::USER_SQUARE).size(size), on)).selected(on).frame(on).min_size(egui::vec2(18.0, 0.0)));
        let resp = crate::bulles::bulle(resp, "Fond sous le trait", about, &key);
        crate::raccourcisui::shortcut_menu(&resp, "pinceau.fond", &key);
        Some(resp)
    }

    /// Commandes du fond ; vrai si `action` en était une.
    pub(crate) fn fond_action(&mut self, action: &str) -> bool {
        match action {
            "pinceau.fond" => {
                if self.refuse_on_folder() {
                    return true;
                }
                self.toggle_fond(self.cur_layer);
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

    /// Pot de peinture (`cut` faux) ou cutter (`cut`) dans la strate Fond, au point `p` du dessin
    /// (Anthony, 2026-10-18 : « le pot de peinture dans le fond, pareil pour le cutter dans le fond
    /// qui coupe des zones fermées ») : la zone fermée par le trait où l'on clique est remplie de la
    /// couleur courante (et le fond y est recousu s'il était coupé), ou retirée du fond.
    pub(crate) fn fond_zone_at(&mut self, p: egui::Pos2, cut: bool) {
        self.end_stroke();
        self.playing = false;
        if !self.layer_editable() {
            return;
        }
        let Some(r) = self.layer().fond else { return };
        let Some(cel) = self.cel_here() else {
            self.status = "Strate Fond : pas de dessin à cette image.".into();
            return;
        };
        if p.x < 0.0 || p.y < 0.0 {
            return;
        }
        let Some(mask) = self.doc.cels.get(&cel).and_then(|c| crate::fond::zone(&c.raster, p.x as usize, p.y as usize, r.trous)) else {
            self.status = "Strate Fond : clique dans une zone fermée par le trait (pas sur le trait).".into();
            return;
        };
        let color = egui::Color32::from_rgb(self.color[0], self.color[1], self.color[2]);
        let (w, h) = (self.doc.width, self.doc.height);
        let Some(c) = self.doc.cels.get_mut(&cel) else { return };
        let s = c.fond.get_or_insert_with(|| Box::new(crate::document::Strate::new(w, h)));
        let mut edits = Vec::new();
        if cut {
            let old = crate::fond::paint_mask(&mut s.coupe, &mask, Some(egui::Color32::WHITE));
            edits.push(crate::history::Edit::FondTiles { cel, tiles: old, coupe: true });
        } else {
            let old = crate::fond::paint_mask(&mut s.peinture.raster, &mask, Some(color));
            edits.push(crate::history::Edit::FondTiles { cel, tiles: old, coupe: false });
            let old = crate::fond::paint_mask(&mut s.coupe, &mask, None);
            if !old.is_empty() {
                edits.push(crate::history::Edit::FondTiles { cel, tiles: old, coupe: true });
            }
        }
        s.peinture.version += 1;
        c.version += 1;
        self.history.push(crate::history::Edit::Group(edits));
        self.modified = true;
        self.renderer.invalidate_all();
        if !cut {
            self.color_used(self.color);
        }
        self.status = if cut {
            "Strate Fond : zone retirée du fond (le pot de peinture ou le pinceau la remet).".into()
        } else {
            "Strate Fond : zone remplie, sous le trait.".into()
        };
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
