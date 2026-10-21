//! Nuanciers personnels dans le panneau Couleur (passe couleur) : choisir le nuancier montré, le
//! créer, le renommer, le dupliquer, le supprimer, l'importer, l'exporter ; ses pastilles (clic :
//! couleur d'avant-plan ; glisser : ranger ; clic droit : menu ; nom dans la bulle). Modèle et
//! formats de fichier : `nuancier.rs`. Les nuanciers sont des réglages de l'application : ils ne
//! passent pas par l'annulation, comme les espaces de travail.

use eframe::egui::{self, vec2, RichText, Sense, Stroke};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::colorui::{swatch_cell, Rename, Renamed, Target};
use crate::couleur;
use crate::nuancier::{self, Palette};

/// Côté d'une pastille du nuancier, en points (le nuancier passe en premier dans le panneau Couleur : des
/// pastilles qu'on vise sans y penser).
const SIDE: f32 = 22.0;

/// Champ d'un nom en cours de saisie ; tout le texte est choisi à l'ouverture. `Some(true)` :
/// validé (Entrée, clic ailleurs) ; `Some(false)` : abandonné (Échap).
fn rename_field(ui: &mut egui::Ui, r: &mut Rename, width: f32) -> Option<bool> {
    let mut out = egui::TextEdit::singleline(&mut r.text).desired_width(width).show(ui);
    let resp = out.response.clone();
    if !r.focused {
        resp.request_focus();
        let n = r.text.chars().count();
        out.state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n))));
        out.state.store(ui.ctx(), resp.id);
        r.focused = true;
        return None;
    }
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        return Some(false);
    }
    resp.lost_focus().then_some(true)
}

impl App {
    /// Le nuancier montré dans le panneau.
    pub(crate) fn shown_palette(&self) -> &Palette {
        let p = &self.couleurs.prefs;
        &p.palettes[p.shown.min(p.palettes.len() - 1)]
    }

    fn shown_palette_mut(&mut self) -> &mut Palette {
        let p = &mut self.couleurs.prefs;
        let i = p.shown.min(p.palettes.len() - 1);
        &mut p.palettes[i]
    }

    /// La pastille du nuancier que visent les commandes qui le modifient : celle du menu ouvert
    /// ou sous la souris, sinon celle de la couleur d'avant-plan.
    fn palette_target(&self) -> Option<usize> {
        match self.target() {
            Some(Target::Palette(i)) => Some(i),
            Some(Target::Recent(_) | Target::Neutre(_)) => None,
            None => self.shown_palette().position(self.color),
        }
    }

    /// Commandes des nuanciers ; vrai si `action` en était une.
    pub(crate) fn palette_action(&mut self, action: &str) -> bool {
        match action {
            "nuancier.ajouter" => {
                let c = match self.target() {
                    Some(Target::Recent(i)) => self.couleurs.prefs.recent.get(i).copied().unwrap_or(self.color),
                    _ => self.color,
                };
                self.add_to_palette(c);
            }
            "nuancier.nouveau" => {
                let p = &mut self.couleurs.prefs;
                p.shown = nuancier::create(&mut p.palettes, p.shown);
                // On le nomme tout de suite.
                self.start_palette_rename();
            }
            "nuancier.renommer" => self.start_palette_rename(),
            "nuancier.dupliquer" => {
                let p = &mut self.couleurs.prefs;
                if let Some(i) = nuancier::duplicate(&mut p.palettes, p.shown) {
                    p.shown = i;
                    self.status = format!("Nuancier dupliqué : « {} ».", self.shown_palette().name);
                }
            }
            "nuancier.supprimer" => self.couleurs.confirm_delete = Some(self.couleurs.prefs.shown),
            "nuancier.importer" => self.import_palette(),
            "nuancier.exporter" => self.export_palette(),
            "pastille.renommer" | "pastille.remplacer" | "pastille.retirer" => {
                let Some(i) = self.palette_target().filter(|&i| i < self.shown_palette().swatches.len()) else {
                    self.no_target();
                    return true;
                };
                match action {
                    "pastille.renommer" => {
                        let text = self.shown_palette().swatches[i].name.clone();
                        self.couleurs.rename = Some(Rename::new(Renamed::Swatch(self.couleurs.prefs.shown, i), text));
                    }
                    "pastille.remplacer" => {
                        let c = self.color;
                        match self.shown_palette().position(c) {
                            Some(j) if j != i => self.status = format!("{} est déjà dans ce nuancier.", couleur::hex(c)),
                            _ => self.shown_palette_mut().swatches[i].color = c,
                        }
                    }
                    _ => {
                        let s = self.shown_palette_mut().swatches.remove(i);
                        let what = if s.name.is_empty() { couleur::hex(s.color) } else { s.name };
                        self.status = format!("Pastille {what} retirée du nuancier.");
                    }
                }
            }
            _ => return false,
        }
        true
    }

    /// Ajoute une couleur à la fin du nuancier montré (pas deux fois la même).
    pub(crate) fn add_to_palette(&mut self, c: [u8; 3]) {
        let name = self.shown_palette().name.clone();
        self.status = match self.shown_palette_mut().add(c, "") {
            Ok(_) => format!("{} ajoutée au nuancier « {name} ».", couleur::hex(c)),
            Err(_) => format!("{} est déjà dans le nuancier « {name} ».", couleur::hex(c)),
        };
    }

    fn start_palette_rename(&mut self) {
        let i = self.couleurs.prefs.shown;
        let text = self.shown_palette().name.clone();
        self.couleurs.rename = Some(Rename::new(Renamed::Palette(i), text));
    }

    /// Fin d'une saisie de nom : gardé si `commit`.
    fn finish_rename(&mut self, commit: bool) {
        let Some(r) = self.couleurs.rename.take() else { return };
        if !commit {
            return;
        }
        let palettes = &mut self.couleurs.prefs.palettes;
        match r.what {
            Renamed::Palette(i) => {
                if let Err(e) = nuancier::rename(palettes, i, &r.text) {
                    self.status = format!("Nom refusé : {e}.");
                }
            }
            Renamed::Swatch(p, i) => {
                if let Some(s) = palettes.get_mut(p).and_then(|p| p.swatches.get_mut(i)) {
                    s.name = r.text.trim().to_string();
                }
            }
        }
    }

    fn import_palette(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Importer un nuancier")
            .add_filter("Nuancier GIMP ou Photoshop", &["gpl", "aco"])
            .add_filter("GIMP (.gpl)", &["gpl"])
            .add_filter("Photoshop (.aco)", &["aco"])
            .pick_file()
        else {
            return;
        };
        match nuancier::read_file(&path) {
            Ok(imported) => {
                let mut palette = imported.palette;
                let p = &mut self.couleurs.prefs;
                palette.name = nuancier::unique_name(&p.palettes, palette.name.trim());
                let (name, n) = (palette.name.clone(), palette.swatches.len());
                p.palettes.push(palette);
                p.shown = p.palettes.len() - 1;
                let skipped = match imported.skipped {
                    0 => String::new(),
                    k => format!(" ; {k} laissées de côté (espace de couleur inconnu ou ligne illisible)"),
                };
                self.status = format!("Nuancier « {name} » importé : {n} couleurs{skipped}.");
            }
            Err(e) => self.status = format!("Nuancier non importé : {e}."),
        }
    }

    fn export_palette(&mut self) {
        let palette = self.shown_palette().clone();
        let Some(path) = rfd::FileDialog::new()
            .set_title("Exporter le nuancier (l'extension choisit le format)")
            .add_filter("GIMP (.gpl)", &["gpl"])
            .add_filter("Photoshop (.aco)", &["aco"])
            .set_file_name(format!("{}.gpl", nuancier::file_name(&palette)))
            .save_file()
        else {
            return;
        };
        self.status = match nuancier::write_file(&path, &palette) {
            Ok(written) => format!("Nuancier « {} » exporté : {}.", palette.name, written.display()),
            Err(e) => format!("Nuancier non exporté : {e}."),
        };
    }

    /// Commandes du nuancier (bouton ⋯ et clic droit sur son nom).
    fn palette_menu(&mut self, ui: &mut egui::Ui) {
        self.action_item(ui, "Ajouter la couleur d'avant-plan", "nuancier.ajouter");
        ui.separator();
        self.action_item(ui, "Nouveau nuancier", "nuancier.nouveau");
        self.action_item(ui, "Renommer le nuancier…", "nuancier.renommer");
        self.action_item(ui, "Dupliquer le nuancier", "nuancier.dupliquer");
        self.action_item(ui, "Supprimer le nuancier…", "nuancier.supprimer");
        ui.separator();
        self.action_item(ui, "Importer un nuancier (GIMP .gpl, Photoshop .aco)…", "nuancier.importer");
        self.action_item(ui, "Exporter le nuancier (GIMP .gpl, Photoshop .aco)…", "nuancier.exporter");
    }

    /// Menu d'une pastille du nuancier (après les commandes communes à toutes les pastilles).
    pub(crate) fn palette_swatch_menu(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        self.action_item(ui, "Nommer la couleur…", "pastille.renommer");
        self.action_item(ui, "Remplacer par la couleur d'avant-plan", "pastille.remplacer");
        self.action_item(ui, "Retirer du nuancier", "pastille.retirer");
    }

    /// Les nuanciers : celui qui est montré (à choisir parmi les autres) et ses commandes (nouveau, renommer, dupliquer,
    /// supprimer, importer, exporter, ajouter ou retirer une couleur). Sous « Plus de couleurs » dans le panneau.
    pub(crate) fn palette_manager(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            crate::bulles::etiquette(ui, ph::SWATCHES, "Nuancier", "Tes nuanciers : choisis celui qui est montré ; clic droit sur son nom : ses commandes.");
            let renaming = matches!(self.couleurs.rename, Some(Rename { what: Renamed::Palette(_), .. }));
            if renaming {
                let done = self.couleurs.rename.as_mut().and_then(|r| rename_field(ui, r, 130.0));
                if let Some(commit) = done {
                    self.finish_rename(commit);
                }
            } else {
                let shown = self.couleurs.prefs.shown.min(self.couleurs.prefs.palettes.len() - 1);
                let names: Vec<String> = self.couleurs.prefs.palettes.iter().map(|p| p.name.clone()).collect();
                let mut chosen = shown;
                let resp = egui::ComboBox::from_id_salt("nuancier_montre")
                    .selected_text(&names[shown])
                    .width(crate::dockui::fit_w(ui, 130.0))
                    .show_ui(ui, |ui| {
                        for (i, n) in names.iter().enumerate() {
                            ui.selectable_value(&mut chosen, i, n);
                        }
                    })
                    .response;
                let resp = crate::bulles::bulle(resp, "Nuancier montré", "Choisir parmi tes nuanciers ; clic droit : ses commandes.", "");
                resp.context_menu(|ui| self.palette_menu(ui));
                if chosen != shown {
                    self.couleurs.prefs.shown = chosen;
                    self.couleurs.rename = None;
                    self.couleurs.confirm_delete = None;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let resp = ui.menu_button(RichText::new(ph::DOTS_THREE).size(crate::bulles::TAILLE), |ui| self.palette_menu(ui)).response;
                crate::bulles::bulle(resp, "Commandes du nuancier", "Nouveau, renommer, dupliquer, supprimer, importer, exporter.", "");
                let here = self.shown_palette().position(self.color);
                if self
                    .bouton_si(ui, here.is_some(), ph::MINUS, "Retirer du nuancier", "La pastille sous la souris, sinon celle de la couleur d'avant-plan.", "pastille.retirer")
                    .clicked()
                {
                    self.palette_action("pastille.retirer");
                }
                if self.bouton_si(ui, here.is_none(), ph::PLUS, "Ajouter au nuancier", "Ajouter la couleur d'avant-plan à la fin du nuancier.", "nuancier.ajouter").clicked() {
                    self.add_to_palette(self.color);
                }
            });
        });
        self.delete_confirmation(ui);
    }

    /// Les couleurs du nuancier montré, et le nom de la pastille qu'on est en train de taper (panneau Couleur : elles
    /// sont toujours visibles, juste sous les neutres).
    pub(crate) fn palette_swatches(&mut self, ui: &mut egui::Ui) {
        self.swatch_grid(ui);
        // Nom de la pastille en cours de saisie.
        if let Some(Rename { what: Renamed::Swatch(p, i), .. }) = self.couleurs.rename {
            let color = self.couleurs.prefs.palettes.get(p).and_then(|p| p.swatches.get(i)).map(|s| s.color);
            match color {
                Some(c) if p == self.couleurs.prefs.shown => {
                    ui.horizontal(|ui| {
                        swatch_cell(ui, c, SIDE, false, Sense::hover());
                        let done = self.couleurs.rename.as_mut().and_then(|r| rename_field(ui, r, 150.0));
                        if let Some(commit) = done {
                            self.finish_rename(commit);
                        }
                    });
                }
                _ => self.couleurs.rename = None,
            }
        }
    }

    /// « Supprimer le nuancier ? » sous son nom, avant de le faire (rien ne l'annulerait).
    fn delete_confirmation(&mut self, ui: &mut egui::Ui) {
        let Some(i) = self.couleurs.confirm_delete else { return };
        let Some(p) = self.couleurs.prefs.palettes.get(i) else {
            self.couleurs.confirm_delete = None;
            return;
        };
        let text = format!("Supprimer le nuancier « {} » ({} couleurs) ? Ctrl+Z ne le rendra pas.", p.name, p.swatches.len());
        ui.horizontal_wrapped(|ui| {
            ui.label(text);
            if ui.button("Supprimer").clicked() {
                let name = self.couleurs.prefs.palettes[i].name.clone();
                let prefs = &mut self.couleurs.prefs;
                prefs.shown = nuancier::delete(&mut prefs.palettes, i);
                self.couleurs.confirm_delete = None;
                self.couleurs.rename = None;
                self.status = format!("Nuancier « {name} » supprimé.");
            }
            if ui.button("Annuler").clicked() {
                self.couleurs.confirm_delete = None;
            }
        });
    }

    /// Les pastilles du nuancier montré. Glisser une pastille la range ailleurs (un trait montre
    /// où elle va ; Échap abandonne). Le panneau et la fenêtre de la barre d'outils en ont
    /// chacun une grille : un glissé n'est suivi que par la grille où il a commencé.
    pub(crate) fn swatch_grid(&mut self, ui: &mut egui::Ui) {
        let grid = ui.id().with("pastilles_du_nuancier");
        let swatches = self.shown_palette().swatches.clone();
        let mut rects = Vec::with_capacity(swatches.len());
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
            for (i, s) in swatches.iter().enumerate() {
                let resp = swatch_cell(ui, s.color, SIDE, s.color == self.color, Sense::click_and_drag());
                rects.push(resp.rect);
                if resp.drag_started() {
                    self.couleurs.drag = Some((grid, i));
                }
                self.target_cell(resp, Target::Palette(i), s.color, &s.name);
            }
            if swatches.is_empty() {
                ui.label(RichText::new("vide : clic droit sur le carré de couleur, « Ajouter au nuancier »").weak());
            }
        });
        let Some(from) = self.couleurs.drag.filter(|(g, _)| *g == grid).map(|(_, i)| i) else { return };
        let (pointer, down, escape) = ui.input(|i| (i.pointer.interact_pos(), i.pointer.primary_down(), i.key_pressed(egui::Key::Escape)));
        let slot = pointer.and_then(|p| nuancier::drop_slot(&rects, p));
        if escape || from >= swatches.len() {
            self.couleurs.drag = None;
        } else if !down {
            self.couleurs.drag = None;
            if let Some(slot) = slot {
                nuancier::move_item(&mut self.shown_palette_mut().swatches, from, slot);
            }
        } else if let Some(slot) = slot {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            let r = rects.get(slot).copied().unwrap_or_else(|| rects[rects.len() - 1].translate(vec2(SIDE + 2.0, 0.0)));
            let x = r.left() - 1.5;
            ui.painter().line_segment([egui::pos2(x, r.top() - 2.0), egui::pos2(x, r.bottom() + 2.0)], Stroke::new(2.0_f32, crate::theme::accent_texte()));
        }
    }
}
