//! Calque de couleur dans l'application (`calquecouleur.rs`) : créer, détacher, accorder les images
//! de la couleur à celles du trait, pot de peinture et pinceau dans la zone fermée par le trait,
//! menu du calque.

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
        let current = self.layers().get(self.cur_layer).map(|l| l.id);
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
            if let Some(i) = current.and_then(|id| self.layers().iter().position(|l| l.id == id)) {
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
            self.status = "Un calque de couleur se pose sous un calque de dessin (ni dossier, ni calque d'effet, ni vidéo, ni CTG).".into();
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

    /// Le calque de couleur courant redevient un calque bitmap ordinaire, aux images indépendantes.
    pub(crate) fn detach_colour_layer(&mut self) {
        if self.layer().couleur.is_none() {
            self.status = "Ce calque n'est pas un calque de couleur.".into();
            return;
        }
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        self.record_step();
        let li = self.cur_layer;
        self.layers_mut()[li].couleur = None;
        self.status = "Calque détaché de son trait : il garde ses dessins, ses images sont à lui.".into();
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
        let Some(link) = self.layer().couleur.as_ref().map(|c| c.trous) else { return Err("Ce n'est pas un calque de couleur.".into()) };
        let Some(li) = cc::trait_de(self.layers(), self.cur_layer) else { return Err("Le calque de trait de cette couleur a disparu.".into()) };
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
        let (cel, prefix) = self.ensure_cel();
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
        if self.tool == Tool::Eraser || !self.layer().couleur.as_ref().is_some_and(|c| c.zone) {
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
        if self.playing || !matches!(self.tool, Tool::Fill | Tool::Brush | Tool::Pencil) {
            return;
        }
        let Some(link) = self.layer().couleur.as_ref().map(|c| (c.trous, c.zone)) else { return };
        if self.tool != Tool::Fill && !link.1 {
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
                let mut zone = link.zone;
                if ui.checkbox(&mut zone, "Pinceau dans la zone fermée").on_hover_text("Un trait de pinceau ou de crayon reste dans la zone fermée par le trait où il commence (et passe sous lui).").changed() {
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
                self.action_item(ui, "Ajouter un autre calque de couleur sous ce trait", "calque.nouveau_couleur");
                self.action_item(ui, "Détacher du calque de trait", "calque.detacher_couleur");
                ui.label(egui::RichText::new("Les images suivent celles du calque de trait ; la visibilité du trait emmène celle de la couleur.").weak().small());
            }
        }
    }
}
