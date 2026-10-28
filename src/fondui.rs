//! Aplat sous le trait, côté interface (`fond.rs` ; dans le code, c'est encore `Layer::fond` : le format des
//! projets ne change pas). Choix d'Anthony, 2026-10-21 : « le fond derrière le trait s'active depuis le calque,
//! plus depuis les réglages du pinceau. Une fois activé, un sous-calque apparaît dans le panneau de calques
//! (uniquement, pas sur la timeline), en petit. Quand on clique sur ce calque on peut gommer ou dessiner sur le
//! fond uniquement. » Et il s'appelle **Aplat**.
//!
//! * L'aplat est un réglage du **calque** : son bouton sur la ligne du calque, le clic droit du calque
//!   (« Ajouter un aplat » / « Retirer l'aplat »), les actions `pinceau.fond` et `fond.*` (leurs raccourcis restent).
//!   Plus rien dans les réglages du pinceau, du crayon, de la gomme ni du pot de peinture.
//! * Une fois posé, une **sous-ligne** fine (`layersui.rs`, panneau Calques seulement) suit son calque. Un clic dessus
//!   en fait la cible du dessin (`App::aplat_cible`) : pinceau, crayon, gomme, pot de peinture et cutter travaillent
//!   l'aplat seul ; un clic sur le calque revient au trait. Ses réglages (couleur, débord, trous) sont dans Réglages
//!   quand la sous-ligne est choisie, et dans son clic droit.

use eframe::egui::{self, RichText};
use egui_phosphor::regular as ph;

use crate::app::{App, Tool};
use crate::fond::FondReglages;

/// Hauteur de la sous-ligne de l'aplat dans le panneau Calques : moins de la moitié d'un rang de calque (36 px + marges).
pub(crate) const SOUS_LIGNE_H: f32 = 16.0;

impl App {
    /// La cible du dessin est l'aplat du calque courant : sa sous-ligne a été choisie, et le calque en a toujours un.
    pub(crate) fn aplat_en_cible(&self) -> bool {
        self.layers().get(self.cur_layer).is_some_and(|l| l.fond.is_some() && self.aplat_cible == Some(l.id))
    }

    /// Le pinceau ne vise plus l'aplat si sa sous-ligne n'existe plus (aplat retiré, annulation, autre projet…).
    /// À chaque image d'écran ; au repos, rien.
    pub(crate) fn tick_aplat(&mut self) {
        if self.aplat_cible.is_some() && !self.aplat_en_cible() {
            self.aplat_cible = None;
        }
    }

    /// Un clic sur la sous-ligne de l'aplat du calque `li` : il devient la cible du dessin (le calque aussi). Un outil qui
    /// ne dessine pas cède la place au pinceau. Un personnage choisi garde ses couleurs : c'est sa couleur d'aplat que
    /// prennent le pinceau et le pot de peinture sur l'aplat (`perso_fond_ou`).
    pub(crate) fn select_aplat(&mut self, li: usize) {
        let Some(id) = self.layers().get(li).filter(|l| l.fond.is_some()).map(|l| l.id) else { return };
        self.end_stroke();
        if li != self.cur_layer {
            self.layer_sel.clear();
            self.select_layer(li);
        }
        self.aplat_cible = Some(id);
        // (L'outil Sélection reste : il prend les pixels de l'aplat et la boîte les transforme, `multitools.rs`.)
        if !matches!(self.tool, Tool::Brush | Tool::Pencil | Tool::Eraser | Tool::Fill | Tool::Gradient | Tool::Cutter | Tool::Select) {
            self.set_tool(Tool::Brush);
        }
        self.status = self.aplat_indication();
    }

    /// « Aplat de <calque> » et ce que fait l'outil courant sur l'aplat (barre d'état du canevas, ligne du bas).
    pub(crate) fn aplat_indication(&self) -> String {
        let nom = self.layers().get(self.cur_layer).map(|l| l.name.as_str()).unwrap_or_default();
        let quoi = match self.tool {
            Tool::Fill => "clic : remplir la zone fermée par le trait, sous le trait",
            Tool::Gradient => "glisser : un dégradé dans la zone fermée par le trait, sous le trait (Alt : tout l'aplat)",
            Tool::Cutter => "clic : retirer la zone fermée de l'aplat",
            Tool::Eraser => "la gomme coupe l'aplat",
            Tool::Select => "lasso, rectangle, clic : prendre les pixels de l'aplat, la boîte les déplace, les met à l'échelle, les tourne (Ctrl+A : tout, sur plusieurs clés aussi)",
            _ => "peindre sous le trait, dans la forme (recoud l'aplat coupé)",
        };
        format!("Aplat de {nom} · {quoi} · clic sur le calque : revenir au trait")
    }

    /// La sous-ligne de l'aplat du calque `li`, fine (moins de la moitié d'un rang), en retrait sous le rang du calque :
    /// une pastille de la couleur de l'aplat et « Aplat ». Surlignée comme un calque choisi quand l'aplat est la cible du
    /// dessin. `hue` : la couleur de la sorte du calque ; `retrait` : le décalage en plus (calque rangé dans un dossier,
    /// calque de couleur). Le clic et le clic droit sont à l'appelant (`layersui.rs`).
    pub(crate) fn aplat_subrow(&mut self, ui: &mut egui::Ui, li: usize, hue: egui::Color32, retrait: f32) -> egui::Response {
        let (nom, couleur) = match self.layers().get(li) {
            Some(l) => (l.name.clone(), l.fond.map_or([255, 255, 255], |f| f.couleur)),
            None => (String::new(), [255, 255, 255]),
        };
        let vise = li == self.cur_layer && self.aplat_en_cible();
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), SOUS_LIGNE_H), egui::Sense::click());
        let resp = crate::bulles::bulle(
            resp,
            &format!("Aplat de {nom}"),
            "L'aplat sous le trait de ce calque. Clic : peindre, gommer ou remplir l'aplat seul, sans toucher au trait. Clic sur le calque : revenir au trait. Clic droit : ses réglages, ou le retirer.",
            "",
        );
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            if vise {
                painter.rect_filled(rect, 3.0, hue.gamma_multiply(0.55));
            } else if resp.hovered() {
                painter.rect_filled(rect, 3.0, hue.gamma_multiply(0.12));
            }
            let pastille = egui::Rect::from_center_size(egui::pos2(rect.left() + 28.0 + retrait, rect.center().y), egui::vec2(14.0, 9.0));
            painter.rect_filled(pastille, 2.0, egui::Color32::from_rgb(couleur[0], couleur[1], couleur[2]));
            painter.rect_stroke(pastille, 2.0, egui::Stroke::new(1.0_f32, crate::theme::bord()), egui::StrokeKind::Inside);
            let encre = if vise { crate::theme::texte() } else { crate::theme::texte_doux() };
            painter.text(egui::pos2(pastille.right() + 6.0, rect.center().y), egui::Align2::LEFT_CENTER, "Aplat", egui::FontId::proportional(10.5), encre);
        }
        resp
    }

    /// Les réglages de l'aplat du calque `li` : couleur, débord, fermeture des trous, bord de l'image. Une étape
    /// d'annulation « Aplat : réglages » par geste. `aussi_sur` : les boutons qui donnent cet aplat à d'autres calques (ils
    /// partent de l'aplat du calque courant : seulement quand `li` l'est).
    pub(crate) fn aplat_reglages_ui(&mut self, ui: &mut egui::Ui, li: usize, aussi_sur: bool) {
        let Some(r) = self.layers().get(li).and_then(|l| l.fond) else { return };
        let mut next = r;
        let mut changed = None;
        // La couleur n'est pas posée à chaque image du glissé : elle l'est au lâcher, sur la portée choisie (`aplatcouleur.rs`).
        self.aplat_couleur_ui(ui, li, r.couleur);
        let mut debord = next.debord as f32;
        let resp = ui.add(egui::Slider::new(&mut debord, 0.0..=30.0).text("Débord (px)").fixed_decimals(0));
        let resp = crate::bulles::bulle(resp, "Débord", "Pixels d'aplat au-delà du bord extérieur du trait.", "");
        if self.aplat_edited(&resp) {
            next.debord = debord.round() as u8;
            changed = Some(next);
        }
        let mut trous = next.trous as f32;
        let resp = ui.add(egui::Slider::new(&mut trous, 0.0..=40.0).text("Fermer les trous (px)").fixed_decimals(0));
        let resp = crate::bulles::bulle(resp, "Fermer les trous", "Un trou du trait jusqu'à deux fois cette valeur ne laisse pas fuir l'aplat.", "");
        if self.aplat_edited(&resp) {
            next.trous = trous.round() as u8;
            changed = Some(next);
        }
        let resp = ui.checkbox(&mut next.bord, "Le bord de l'image ferme la forme");
        let resp = crate::bulles::bulle(resp, "Le bord de l'image ferme la forme", "Un personnage coupé par le cadre (plan poitrine) a son aplat jusqu'au bord.", "");
        if self.aplat_edited(&resp) {
            changed = Some(next);
        }
        // Aplat intelligent (`aplatintel.rs`) : l'aplat suit le trait en gardant la couleur de chaque zone fermée, sans IA.
        let resp = ui.checkbox(&mut next.intelligent, "Aplat intelligent");
        let resp = crate::bulles::bulle(
            resp,
            "Aplat intelligent",
            "L'aplat suit le trait : après une retouche du trait, ou sur une clé qui suit une clé coloriée, chaque zone fermée (une peau, un œil, une ombre) garde sa couleur, retrouvée par sa position et sa forme. Pas d'IA ; calculé en arrière-plan. Éteint, rien ne change.",
            "",
        );
        if self.aplat_edited(&resp) {
            changed = Some(next);
        }
        if r.intelligent && li == self.cur_layer {
            let resp = ui.small_button("Colorier comme la clé précédente");
            let key = self.raccourci("aplat.colorier_comme_precedente");
            let resp = crate::bulles::bulle(
                resp,
                "Colorier comme la clé précédente",
                "Les clés choisies dans la timeline (sinon la clé qu'on voit) prennent les couleurs de la clé coloriée d'avant, zone par zone. Plusieurs clés : chacune suit la précédente, en chaîne.",
                &key,
            );
            crate::raccourcisui::shortcut_menu(&resp, "aplat.colorier_comme_precedente", &key);
            if resp.clicked() {
                self.run_action("aplat.colorier_comme_precedente");
            }
        }
        if let Some(n) = changed {
            self.layers_mut()[li].fond = Some(n);
            self.renderer.invalidate_all();
        }
        if aussi_sur {
            ui.horizontal_wrapped(|ui| {
                ui.label("Cet aplat aussi sur");
                if ui.small_button("Les calques choisis").on_hover_text("Les calques choisis (Ctrl ou Maj + clic) prennent cet aplat.").clicked() {
                    self.run_action("fond.calques_choisis");
                }
                if ui.small_button("Tout le plan").on_hover_text("Tous les calques de dessin du plan, sauf les calques de fond du décor.").clicked() {
                    self.run_action("fond.tout_le_plan");
                }
            });
        }
    }

    /// `edited`, avec l'étape d'annulation nommée « Aplat : réglages ».
    fn aplat_edited(&mut self, resp: &egui::Response) -> bool {
        let avant = self.history.context.replace("Aplat : réglages".to_string());
        let fait = self.edited(resp);
        self.history.context = avant;
        fait
    }

    /// Le haut de Réglages quand la sous-ligne de l'aplat est choisie : « Aplat de <calque> », ses réglages, ce que fait
    /// l'outil dessus. Rien sinon.
    pub(crate) fn aplat_section_ui(&mut self, ui: &mut egui::Ui) {
        if !self.aplat_en_cible() {
            return;
        }
        let li = self.cur_layer;
        let nom = self.layers()[li].name.clone();
        ui.label(RichText::new(format!("Aplat de {nom}")).strong());
        self.aplat_reglages_ui(ui, li, true);
        ui.label(RichText::new(self.aplat_indication()).small().weak());
        ui.separator();
    }

    /// Clic droit sur la sous-ligne de l'aplat du calque `li` : ses réglages, et le retirer.
    pub(crate) fn aplat_menu(&mut self, ui: &mut egui::Ui, li: usize) {
        let Some(nom) = self.layers().get(li).map(|l| l.name.clone()) else {
            ui.close_menu();
            return;
        };
        ui.label(RichText::new(format!("Aplat de « {nom} »")).weak().small());
        self.aplat_reglages_ui(ui, li, false);
        ui.separator();
        if ui.button("Retirer l'aplat").clicked() {
            ui.close_menu();
            self.toggle_fond(li);
        }
    }

    /// « Ajouter un aplat » ou « Retirer l'aplat » pour le calque `li` (clic droit d'un calque, partout où il s'ouvre) ;
    /// rien pour un dossier ou un calque vidéo.
    pub(crate) fn aplat_item(&mut self, ui: &mut egui::Ui, li: usize) {
        let Some(on) = self.layers().get(li).filter(|l| takes_fond(l)).map(|l| l.fond.is_some()) else { return };
        if ui.button(if on { "Retirer l'aplat" } else { "Ajouter un aplat" }).clicked() {
            ui.close_menu();
            self.toggle_fond(li);
        }
    }

    /// Sous-menu « Aplat sous le trait » (menu Calques). `courant` : avec la ligne du calque courant (les clics droits
    /// des calques l'ont déjà, en haut de leur menu).
    pub(crate) fn fond_menu(&mut self, ui: &mut egui::Ui, courant: bool) {
        if courant {
            let label = if self.layer().fond.is_some() { "Retirer l'aplat du calque courant" } else { "Ajouter un aplat au calque courant" };
            self.action_item(ui, label, "pinceau.fond");
        }
        self.action_item(ui, "Sur les calques choisis", "fond.calques_choisis");
        self.action_item(ui, "Sur tout le plan", "fond.tout_le_plan");
        ui.separator();
        self.action_item(ui, "Retirer des calques choisis", "fond.retirer_choisis");
        self.action_item(ui, "Couleur du projet à tous les aplats", "fond.couleur_partout");
    }

    /// Allume ou éteint l'aplat sous le trait du calque `li` (une étape d'annulation « Aplat : ajouter » ou « Aplat :
    /// retirer », le calque courant ne change pas). L'aplat est un réglage du **calque** : le bouton de sa ligne (panneau
    /// Calques, barre de commandes du panneau), le clic droit du calque et l'action `pinceau.fond` (qui vise le calque
    /// courant) font la même chose. Il a toujours été ce champ du calque (`Layer::fond`) et jamais un réglage de pinceau :
    /// les projets et les réglages enregistrés se lisent donc sans aucune conversion.
    pub(crate) fn toggle_fond(&mut self, li: usize) {
        if self.content_refuses() {
            return;
        }
        if !self.layers().get(li).is_some_and(takes_fond) {
            self.status = "Aplat : ce calque n'en prend pas (ni dossier, ni vidéo).".into();
            return;
        }
        self.end_stroke();
        let on = self.layers()[li].fond.is_some();
        let fresh = self.new_fond();
        // (Le nom posé par le trait d'un personnage, « Aplat du personnage », est gardé.)
        let nom = if on { "Aplat : retirer" } else { "Aplat : ajouter" };
        let garde = self.history.context.as_deref() == Some("Aplat du personnage");
        if garde {
            self.record_step();
        } else {
            self.record_named_step(nom);
        }
        let id = self.layers()[li].id;
        self.layers_mut()[li].fond = if on { None } else { Some(fresh) };
        if on && self.aplat_cible == Some(id) {
            self.aplat_cible = None;
        }
        self.renderer.invalidate_all();
        self.status = if on {
            "Aplat : retiré de ce calque (Ctrl+Z le remet, avec ce qu'on y avait peint).".into()
        } else {
            "Aplat : chaque dessin du calque a maintenant un aplat, recalculé à chaque trait. Clique sur sa sous-ligne (panneau Calques) pour le peindre ou le gommer seul.".into()
        };
    }

    /// Le bouton « Aplat sous le trait » d'un calque : allumé quand le calque en a un. Il est sur la ligne du calque
    /// (panneau Calques) et dans les commandes du panneau ; rien pour un dossier ou un calque vidéo (ils n'en
    /// prennent pas).
    pub(crate) fn fond_layer_button(&mut self, ui: &mut egui::Ui, li: usize, small: bool) -> Option<egui::Response> {
        let l = self.layers().get(li)?;
        if !takes_fond(l) {
            return None;
        }
        let on = l.fond.is_some();
        let size = if small { 12.0 } else { crate::bulles::TAILLE };
        let key = self.raccourci("pinceau.fond");
        let about = if on {
            "Ce calque a un aplat sous son trait : tout ce que le trait enferme, plus un léger débord, en une seule forme qui se recalcule à chaque trait. Clic : le retirer. Sa sous-ligne, juste dessous, sert à peindre ou gommer l'aplat seul ; ses réglages sont dans Réglages quand elle est choisie."
        } else {
            "Donner un aplat à ce calque : tout ce que le trait enferme, plus un léger débord, en une seule forme sous le trait, qui se recalcule à chaque trait. Une sous-ligne apparaît sous le calque : un clic dessus, et l'on peint, gomme ou remplit l'aplat sans toucher au trait."
        };
        let resp = ui.add(egui::Button::new(crate::bulles::encre_etat(RichText::new(ph::USER_SQUARE).size(size), on)).selected(on).frame(on).min_size(egui::vec2(18.0, 0.0)));
        let resp = crate::bulles::bulle(resp, "Aplat sous le trait", about, &key);
        crate::raccourcisui::shortcut_menu(&resp, "pinceau.fond", &key);
        Some(resp)
    }

    /// Commandes de l'aplat ; vrai si `action` en était une.
    pub(crate) fn fond_action(&mut self, action: &str) -> bool {
        match action {
            // (Ce sont des alias : les raccourcis d'avant continuent d'allumer l'aplat des calques visés. « pinceau.remplir »,
            // l'ancien « Remplir la forme » retiré le 2026-10-23, en est un double : `keymap::ALIASES`.)
            // L'aplat intelligent (`aplatintel.rs`) : les clés choisies, chacune comme la clé coloriée d'avant, en chaîne.
            "aplat.colorier_comme_precedente" => self.colorier_comme_precedente(),
            "pinceau.fond" | "pinceau.remplir" => {
                if self.refuse_on_folder() {
                    return true;
                }
                self.toggle_fond(self.cur_layer);
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
                    self.status = "Aplat : aucun calque de dessin visé (ni dossier, ni vidéo).".into();
                    return true;
                }
                let fresh = self.layer().fond.unwrap_or_else(|| self.new_fond());
                self.record_named_step(if remove { "Aplat : retirer" } else { "Aplat : ajouter" });
                for &li in &targets {
                    self.layers_mut()[li].fond = if remove { None } else { Some(fresh) };
                }
                self.tick_aplat();
                self.renderer.invalidate_all();
                let n = targets.len();
                let s = if n > 1 { "s" } else { "" };
                self.status = match (remove, whole) {
                    (true, _) => format!("Aplat : retiré de {n} calque{s}."),
                    (false, true) => format!("Aplat : posé sur les {n} calque{s} de dessin du plan (pas sur les calques de fond du décor)."),
                    (false, false) => format!("Aplat : posé sur {n} calque{s}."),
                };
            }
            // Couleur du projet donnée à tous les aplats déjà posés.
            "fond.couleur_partout" => {
                let couleur = self.new_fond().couleur;
                let n: usize = self.doc.panels.iter().flat_map(|p| &p.layers).filter(|l| l.fond.is_some_and(|f| f.couleur != couleur)).count();
                if n == 0 {
                    self.status = "Tous les aplats du projet ont déjà cette couleur.".into();
                    return true;
                }
                self.record_named_step("Aplat : couleur du projet");
                for l in self.doc.panels.iter_mut().flat_map(|p| p.layers.iter_mut()) {
                    if let Some(f) = l.fond.as_mut() {
                        f.couleur = couleur;
                    }
                }
                self.renderer.invalidate_all();
                self.status = format!("Couleur des aplats : {n} calque(s) changé(s) dans le projet.");
            }
            _ => return false,
        }
        true
    }

    /// Pot de peinture (`cut` faux) ou cutter (`cut`) sur l'aplat, au point `p` du dessin
    /// (Anthony, 2026-10-18 : « le pot de peinture dans le fond, pareil pour le cutter dans le fond
    /// qui coupe des zones fermées ») : la zone fermée par le trait où l'on clique est remplie de la
    /// couleur courante (et l'aplat y est recousu s'il était coupé), ou retirée de l'aplat.
    pub(crate) fn fond_zone_at(&mut self, p: egui::Pos2, cut: bool) {
        self.end_stroke();
        self.playing = false;
        if !self.layer_editable() {
            return;
        }
        let Some(r) = self.layer().fond else { return };
        let Some(cel) = self.cel_here() else {
            self.status = "Aplat : pas de dessin à cette image.".into();
            return;
        };
        if p.x < 0.0 || p.y < 0.0 {
            return;
        }
        let Some(mask) = self.doc.cels.get(&cel).and_then(|c| crate::fond::zone(&c.raster, p.x as usize, p.y as usize, r.trous)) else {
            self.status = "Aplat : clique dans une zone fermée par le trait (pas sur le trait).".into();
            return;
        };
        // Un personnage choisi (`personnagesui.rs`) : le pot de peinture sur l'aplat prend sa couleur d'aplat.
        let fill = self.perso_fond_ou(self.color);
        let color = egui::Color32::from_rgb(fill[0], fill[1], fill[2]);
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
        let avant = self.history.context.replace(if cut { "Aplat : retirer la zone" } else { "Aplat : remplir" }.to_string());
        self.history.push(crate::history::Edit::Group(edits));
        self.history.context = avant;
        self.modified = true;
        self.renderer.invalidate_all();
        if !cut {
            self.color_used(fill);
        }
        self.status = if cut {
            "Aplat : zone retirée de l'aplat (le pot de peinture ou le pinceau la remet).".into()
        } else {
            "Aplat : zone remplie, sous le trait.".into()
        };
    }

    /// Un aplat neuf : la couleur du projet (Réglages du projet), blanc d'office.
    pub(crate) fn new_fond(&self) -> FondReglages {
        let d = FondReglages::default();
        FondReglages { couleur: self.doc.properties.fond.unwrap_or(d.couleur), ..d }
    }
}

/// Le calque peut avoir un aplat sous son trait : un calque de dessin (ni dossier, ni
/// vidéo).
pub(crate) fn takes_fond(l: &crate::document::Layer) -> bool {
    !l.folder && l.video.is_none()
}
