//! Calque de couleur dans l'application (`calquecouleur.rs`) : créer, détacher, accorder les images
//! de la couleur à celles du trait, pot de peinture et pinceau dans la zone fermée par le trait,
//! mode « Griffonner » (`griffonnage.rs` : les couleurs posées remplissent les zones fermées par le trait), menu du
//! calque.

use eframe::egui::{self, Color32, Pos2};

use crate::app::{App, Tool};
use crate::calquecouleur::{self as cc, Cle, Zone};
use crate::document::CelId;
use crate::history::Edit;

impl App {
    /// Accorde les calques de couleur de tous les plans : ils sont remis sous leur trait, et leurs
    /// images suivent celles du trait. Rien à faire (un coup d'œil) quand il n'y en a pas. Le calque
    /// courant reste le même calque.
    pub(crate) fn sync_couleurs(&mut self) {
        // (Sans paniquer si le plan courant vient de disparaître : l'appelant le remet juste après.)
        let current = self.doc.panels.get(self.cur_panel).and_then(|p| p.layers.get(self.cur_layer)).map(|l| l.id);
        let (mut any, mut here) = (false, false);
        for pi in 0..self.doc.panels.len() {
            if !self.doc.panels[pi].layers.iter().any(|l| l.couleur.is_some()) {
                continue;
            }
            let mut changed = cc::reparer(&mut self.doc.panels[pi].layers);
            if changed {
                crate::folders::normalize(&mut self.doc.panels[pi].layers);
            }
            changed |= cc::refleter(&mut self.doc, pi);
            any |= changed;
            here |= changed && pi == self.cur_panel;
        }
        if here {
            if let Some(i) = current.and_then(|id| self.doc.panels.get(self.cur_panel)?.layers.iter().position(|l| l.id == id)) {
                self.cur_layer = i;
            }
        }
        if any {
            self.modified = true;
            self.renderer.invalidate_all();
        }
    }

    /// Nouveau calque de couleur sous le calque de trait courant (ou sous le trait du calque de
    /// couleur courant).
    pub(crate) fn new_colour_layer(&mut self) {
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        let line = cc::trait_de(self.layers(), self.cur_layer).unwrap_or(self.cur_layer);
        if !self.layers().get(line).is_some_and(cc::peut_porter) {
            self.status = "Un calque de couleur se pose sous un calque de dessin (ni dossier, ni calque d'effet, ni vidéo).".into();
            return;
        }
        let snap = self.snapshot();
        let n = cc::couleurs_de(self.layers(), line).len() + 1;
        let base = self.layers()[line].name.clone();
        let name = if n == 1 { format!("{base} couleur") } else { format!("{base} couleur {n}") };
        let layer = cc::nouveau(&self.layers()[line], name);
        // Juste sous le trait : le trait monte d'un cran.
        self.layers_mut().insert(line, layer);
        self.cur_layer = line;
        self.layer_sel.clear();
        self.vsel = None;
        self.commit_structure(snap);
        self.status = format!("Calque de couleur sous « {base} » : colorie dessous, le trait reste intact (Ctrl+Z annule).");
    }

    /// Le calque de couleur courant redevient un calque bitmap ordinaire, aux images indépendantes. S'il a des griffonnages,
    /// il reste un calque de couleur, **libre** : ses images sont à lui, et son ancien calque de trait devient une de ses
    /// sources (le remplissage garde ses murs).
    pub(crate) fn detach_colour_layer(&mut self) {
        let Some(link) = self.layer().couleur.clone() else {
            self.status = "Ce calque n'est pas un calque de couleur.".into();
            return;
        };
        if !link.est_lie() {
            self.status = "Ce calque de couleur est déjà libre : ses images sont à lui.".into();
            return;
        }
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        self.record_step();
        let li = self.cur_layer;
        if link.remplissage || !link.griffonnages.is_empty() {
            if let Some(c) = self.layers_mut()[li].couleur.as_mut() {
                if !c.sources.contains(&c.trait_id) {
                    c.sources.push(c.trait_id);
                }
                c.trait_id = 0;
                c.dessins.clear();
            }
            self.status = "Calque libéré de son trait : ses images sont à lui, ses griffonnages restent et le trait devient une de ses sources.".into();
        } else {
            self.layers_mut()[li].couleur = None;
            self.status = "Calque détaché de son trait : il garde ses dessins, ses images sont à lui.".into();
        }
    }

    /// Allume ou éteint le mode « Griffonner » du calque de couleur courant (une étape d'annulation) : le pinceau, le
    /// crayon, la gomme et le pot posent alors des griffonnages, qui remplissent les zones fermées par le trait.
    pub(crate) fn toggle_griffonner(&mut self) {
        let Some(on) = self.layer().couleur.as_ref().map(|c| c.griffonner) else {
            self.status = "Griffonner se fait sur un calque de couleur (Calque > Nouveau calque > De couleur).".into();
            return;
        };
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        self.record_step();
        let li = self.cur_layer;
        if let Some(c) = self.layers_mut()[li].couleur.as_mut() {
            c.griffonner = !on;
        }
        self.status = if on {
            "Griffonner éteint : le pinceau peint à la main, dans la zone fermée par le trait.".into()
        } else {
            "Griffonner : les couleurs que tu poses remplissent d'un coup la zone fermée par le trait (elles se refont si le trait change).".into()
        };
    }

    /// Le dessin de griffonnages de l'image courante du calque de couleur courant, créé au besoin (avec l'état d'avant,
    /// pour l'annulation) : il répond au dessin de peinture de l'image, qui existe d'abord (`ensure_colour_cel`).
    pub(crate) fn ensure_scribble_cel(&mut self) -> (CelId, Option<Edit>) {
        let (paint, prefix) = self.ensure_colour_cel();
        let li = self.cur_layer;
        let existing = self.layers()[li].couleur.as_ref().and_then(|c| c.griffonnages.get(&paint).copied()).filter(|g| self.doc.cels.contains_key(g));
        if let Some(g) = existing {
            return (g, prefix);
        }
        let snap = prefix.unwrap_or_else(|| self.snapshot());
        let g = self.doc.new_cel(crate::document::LayerKind::Bitmap);
        if let Some(c) = self.layers_mut()[li].couleur.as_mut() {
            c.griffonnages.insert(paint, g);
            c.remplissage = true;
        }
        (g, Some(snap))
    }

    /// Pot de peinture en mode « Griffonner » : un tampon de la couleur courante, un petit disque de griffonnage, au point
    /// `p` — la zone fermée qu'il touche se remplit, et se refait si le trait change. Une étape d'annulation.
    fn griffon_tampon(&mut self, p: Pos2) {
        self.end_stroke();
        self.playing = false;
        if !self.layer_editable() {
            return;
        }
        self.color_used(self.color);
        let color = Color32::from_rgb(self.color[0], self.color[1], self.color[2]);
        let (cel, prefix) = self.ensure_scribble_cel();
        let Some(c) = self.doc.cels.get_mut(&cel) else { return };
        let tiles = crate::griffonnage::tampon(&mut c.raster, p.x, p.y, crate::griffonnage::RAYON_TAMPON, color);
        self.push_pixels(cel, prefix, tiles);
        self.status = "Griffonnage posé : la zone fermée par le trait où il tombe se remplit (le pinceau en mode Griffonner fait pareil, d'un trait).".into();
    }

    /// Montre ou cache le calque `li` ; un calque de trait emmène ses calques de couleur (la couleur
    /// seule se cache ensuite avec son propre œil). Une étape d'annulation.
    pub(crate) fn toggle_layer_visible(&mut self, li: usize) {
        if li >= self.layers().len() {
            return;
        }
        self.end_stroke();
        self.record_step();
        let visible = !self.layers()[li].visible;
        let mut all = cc::couleurs_de(self.layers(), li);
        all.push(li);
        for k in all {
            self.layers_mut()[k].visible = visible;
        }
        self.renderer.invalidate_all();
    }

    /// Le dessin de couleur de l'image courante, créé au besoin : à une image où le trait n'a rien,
    /// un dessin de trait vide est posé (la couleur y suit). Avec l'état d'avant pour l'annulation.
    pub(crate) fn ensure_colour_cel(&mut self) -> (CelId, Option<Edit>) {
        // Un calque de couleur libre n'a pas de trait dont suivre les images : des dessins à lui, comme un calque ordinaire.
        if self.layer().couleur.as_ref().is_some_and(|c| !c.est_lie()) {
            return self.ensure_plain_cel();
        }
        cc::refleter(&mut self.doc, self.cur_panel);
        if let Some(id) = self.cel_here() {
            return (id, None);
        }
        let (ci, f) = (self.cur_layer, self.cur_frame);
        let snap = self.snapshot();
        if let Some(li) = cc::trait_de(self.layers(), ci) {
            let kind = self.layers()[li].kind;
            let id = self.doc.new_cel(kind);
            let limit = self.frame_count();
            let storyboard = self.mode == crate::app::Mode::Storyboard;
            let (start, end) = crate::images::new_drawing_span(&self.layers()[li].frames, f, limit, storyboard, self.default_exposure);
            for k in start..end {
                self.layers_mut()[li].set(k, Some(id));
            }
            cc::refleter(&mut self.doc, self.cur_panel);
            if let Some(cel) = self.cel_here() {
                return (cel, Some(snap));
            }
        }
        // (Le trait a disparu entre-temps : un dessin à la volée, comme sur un calque ordinaire.)
        let cel = self.doc.new_cel(crate::document::LayerKind::Bitmap);
        self.layers_mut()[ci].set(f, Some(cel));
        (cel, Some(snap))
    }

    /// Les murs et la zone fermée du trait où tombe le point `p` du dessin, pour le calque de couleur
    /// courant.
    pub(crate) fn zone_couleur(&mut self, p: Pos2) -> Result<Zone, String> {
        if p.x < 0.0 || p.y < 0.0 || p.x >= self.doc.width as f32 || p.y >= self.doc.height as f32 {
            return Err("Hors de la feuille.".into());
        }
        let Some(link) = self.layer().couleur.as_ref().map(|c| (c.trous, c.est_lie())) else { return Err("Ce n'est pas un calque de couleur.".into()) };
        let (link, lie) = link;
        let Some(li) = cc::trait_de(self.layers(), self.cur_layer) else {
            return Err(if lie {
                "Le calque de trait de cette couleur a disparu.".into()
            } else {
                "Ce calque de couleur est libre, sans calque de trait : passe en mode Griffonner pour remplir des zones.".into()
            });
        };
        let line_name = self.layers()[li].name.clone();
        let Some(cel) = self.layers()[li].cel_at(self.cur_frame) else {
            return Err(format!("« {line_name} » n'a pas de dessin à cette image : pas de trait pour fermer la zone."));
        };
        let Some(c) = self.doc.cels.get(&cel) else { return Err("Dessin du trait introuvable.".into()) };
        let cle = Cle { cel, version: c.version, trous: link };
        let Some(murs) = self.murs.obtenir(cle, &c.raster) else {
            return Err(format!("« {line_name} » est vide à cette image : dessine d'abord le trait."));
        };
        murs.zone(p.x as usize, p.y as usize)
            .ok_or_else(|| "Clique dans une zone fermée par le trait (pas sur le trait lui-même).".to_string())
    }

    /// Pot de peinture sur un calque de couleur : la zone fermée par le trait où l'on clique prend la
    /// couleur courante, sous le trait. Une étape d'annulation.
    pub(crate) fn fill_couleur(&mut self, p: Pos2) {
        // En mode « Griffonner » : un tampon de griffonnage, la zone se remplit toute seule.
        if self.layer().couleur.as_ref().is_some_and(|c| c.griffonner) {
            self.griffon_tampon(p);
            return;
        }
        self.end_stroke();
        self.playing = false;
        if !self.layer_editable() {
            return;
        }
        let zone = match self.zone_couleur(p) {
            Ok(z) => z,
            Err(e) => {
                self.status = e;
                return;
            }
        };
        self.color_used(self.color);
        let color = Color32::from_rgb(self.color[0], self.color[1], self.color[2]);
        // (La peinture libre : le mode « Griffonner » est éteint ici.)
        let (cel, prefix) = self.ensure_colour_cel();
        let Some(c) = self.doc.cels.get_mut(&cel) else { return };
        let tiles = zone.peindre(&mut c.raster, Some(color));
        self.push_pixels(cel, prefix, tiles);
        self.status = if zone.dehors {
            "Zone remplie sous le trait (elle fuit par un trou : tout le dehors est colorié ; Ctrl+Z, ou agrandis la fermeture des trous).".into()
        } else {
            "Zone remplie sous le trait.".into()
        };
    }

    /// Le pochoir d'un trait de pinceau ou de crayon qui commence en `p` sur un calque de couleur : la
    /// zone fermée par le trait où il commence (`stencil` : les pochoirs déjà là). Le trait peint
    /// partout si le calque ne limite pas à la zone, si le point est sur le trait, ou pour la gomme.
    pub(crate) fn couleur_pochoir(&mut self, stencil: Option<crate::stencil::Stencil>, p: Pos2) -> Option<crate::stencil::Stencil> {
        // (Un griffonnage n'est pas gardé dans une zone : il remplit celles qu'il touche.)
        if self.tool == Tool::Eraser || !self.layer().couleur.as_ref().is_some_and(|c| c.zone && !c.griffonner) {
            return stencil;
        }
        let Ok(zone) = self.zone_couleur(p) else { return stencil };
        let (w, h) = (self.doc.width, self.doc.height);
        let mut s = stencil.unwrap_or_else(|| crate::stencil::Stencil::new(w, h));
        s.add(&zone.pochoir(w, h), false, None);
        Some(s)
    }

    /// Sur un calque de couleur, avec un outil qui s'en sert : les murs du trait se calculent d'avance
    /// sur un autre fil, pour que le premier clic ou trait n'attende pas.
    pub(crate) fn warm_couleur(&mut self) {
        self.murs.poll();
        if self.playing || !matches!(self.tool, Tool::Fill | Tool::Gradient | Tool::Brush | Tool::Pencil) {
            return;
        }
        let Some(link) = self.layer().couleur.as_ref().map(|c| (c.trous, c.zone && !c.griffonner)) else { return };
        if self.layer().couleur.as_ref().is_some_and(|c| c.griffonner) {
            // (En mode « Griffonner », ni le pot ni le pinceau n'ont besoin des murs du trait : le remplissage se calcule à part.)
            return;
        }
        if !matches!(self.tool, Tool::Fill | Tool::Gradient) && !link.1 {
            return;
        }
        let Some(li) = cc::trait_de(self.layers(), self.cur_layer) else { return };
        let Some(cel) = self.layers()[li].cel_at(self.cur_frame) else { return };
        let Some(c) = self.doc.cels.get(&cel) else { return };
        // (Un trait vectoriel en cours de rendu exact change de version : le calcul repart.)
        self.murs.prechauffer(Cle { cel, version: c.version, trous: link.0 }, &c.raster);
        if self.murs.calcule() {
            self.ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    /// Le sous-menu « Calque de couleur » d'une ligne de calque : sur un calque de trait, en ajouter un ;
    /// sur un calque de couleur, ses réglages.
    pub(crate) fn couleur_menu(&mut self, ui: &mut egui::Ui, li: usize) {
        let Some(l) = self.layers().get(li) else { return };
        match l.couleur.clone() {
            None if cc::peut_porter(l) => {
                self.action_item(ui, "Ajouter un calque de couleur sous ce trait", "calque.nouveau_couleur");
            }
            None => {
                ui.label(egui::RichText::new("Un calque de couleur se pose sous un calque de dessin ordinaire.").weak().small());
            }
            Some(link) => {
                // Griffonner : les couleurs posées remplissent les zones (mode, `calque.griffonner`).
                let mut griffonne = link.griffonner;
                let key = self.raccourci("calque.griffonner");
                let resp = ui.checkbox(&mut griffonne, "Griffonner (remplir les zones d'un coup)").on_hover_text(format!(
                    "Les couleurs que tu poses (pinceau, crayon, gomme, pot) remplissent d'un coup la zone fermée par le trait où elles tombent, sous le trait ; l'aplat se refait tout seul quand le trait change. La gomme efface alors les griffonnages.{}",
                    if key.is_empty() { String::new() } else { format!(" ({key})") }
                ));
                if resp.changed() {
                    self.record_step();
                    if let Some(c) = self.layers_mut()[li].couleur.as_mut() {
                        c.griffonner = griffonne;
                    }
                }
                let mut zone = link.zone;
                if ui.checkbox(&mut zone, "Pinceau dans la zone fermée").on_hover_text("En peinture à la main (pas en mode Griffonner) : un trait de pinceau ou de crayon reste dans la zone fermée par le trait où il commence (et passe sous lui).").changed() {
                    self.record_step();
                    if let Some(c) = self.layers_mut()[li].couleur.as_mut() {
                        c.zone = zone;
                    }
                }
                ui.horizontal(|ui| {
                    ui.label("Fermer les trous jusqu'à");
                    let mut gap = link.trous as u32 * 2;
                    let resp = ui.add(egui::DragValue::new(&mut gap).range(0..=100).suffix(" px")).on_hover_text("Un trou du trait plus petit que cela ne laisse pas fuir la couleur (0 : pas de fermeture).");
                    if self.edited(&resp) {
                        if let Some(c) = self.layers_mut()[li].couleur.as_mut() {
                            c.trous = (gap / 2) as u8;
                        }
                    }
                });
                ui.separator();
                ui.menu_button("Sources supplémentaires", |ui| self.couleur_sources_menu(ui, li));
                ui.separator();
                if link.est_lie() {
                    self.action_item(ui, "Ajouter un autre calque de couleur sous ce trait", "calque.nouveau_couleur");
                    self.action_item(ui, "Détacher du calque de trait", "calque.detacher_couleur");
                    ui.label(egui::RichText::new("Les images suivent celles du calque de trait ; la visibilité du trait emmène celle de la couleur.").weak().small());
                } else {
                    ui.label(egui::RichText::new("Calque libre : ses images sont à lui, et ses zones sont fermées par ses sources.").weak().small());
                }
            }
        }
    }

    /// Les sources supplémentaires du calque de couleur `li` : d'autres calques de dessin dont les traits ferment aussi
    /// les zones du remplissage au griffonnage (son calque de trait, lui, ferme toujours les siennes). Une étape
    /// d'annulation par case cochée.
    fn couleur_sources_menu(&mut self, ui: &mut egui::Ui, li: usize) {
        let Some(link) = self.layers().get(li).and_then(|l| l.couleur.clone()) else { return };
        if let Some(t) = cc::trait_de(self.layers(), li) {
            let name = self.layers()[t].name.clone();
            ui.label(egui::RichText::new(format!("« {name} » (son trait) ferme toujours les zones.")).weak().small());
        } else {
            ui.label(egui::RichText::new("Les traits des calques cochés ferment les zones.").weak().small());
        }
        let candidates: Vec<(u64, String)> = self
            .layers()
            .iter()
            .enumerate()
            .rev()
            .filter(|(k, l)| *k != li && !l.folder && l.effet.is_none() && l.video.is_none() && l.id != link.trait_id)
            .map(|(_, l)| (l.id, l.name.clone()))
            .collect();
        if candidates.is_empty() {
            ui.label(egui::RichText::new("Pas d'autre calque de dessin.").weak().small());
        }
        for (id, name) in candidates {
            let mut on = link.sources.contains(&id);
            if ui.checkbox(&mut on, name).changed() {
                self.record_step();
                if let Some(c) = self.layers_mut()[li].couleur.as_mut() {
                    if on {
                        c.sources.push(id);
                    } else {
                        c.sources.retain(|s| *s != id);
                    }
                }
            }
        }
    }

    /// Le bouton « Griffonner » des réglages d'outil (pinceau, crayon, gomme, pot) quand le calque courant est un calque de
    /// couleur : il allume ou éteint le mode du calque, avec une phrase qui dit ce qui se passe.
    pub(crate) fn griffonner_options_ui(&mut self, ui: &mut egui::Ui) {
        let Some(on) = self.layer().couleur.as_ref().map(|c| c.griffonner) else { return };
        use egui_phosphor::regular as ph;
        ui.separator();
        ui.horizontal(|ui| {
            let resp = self.bouton_etat(
                ui,
                on,
                ph::SCRIBBLE_LOOP,
                "Griffonner",
                "Les couleurs que tu poses remplissent d'un coup la zone fermée par le trait où elles tombent, sous le trait ; elles se refont si le trait change. La gomme efface les griffonnages.",
                "calque.griffonner",
            );
            if resp.clicked() {
                self.toggle_griffonner();
            }
            ui.label(egui::RichText::new(if on { "Griffonner : la couleur remplit la zone." } else { "Peinture à la main." }).weak().small());
        });
    }
}
