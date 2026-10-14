//! Réglages nommés, papier et grain des pinceaux (3d-2), dans le panneau Outils.

use eframe::egui::{self, RichText};

use crate::app::{App, Tool};
use crate::brush::BrushSettings;
use crate::brushtex::{BrushLibrary, Target};

impl App {
    fn tool_brush_mut(&mut self) -> Option<&mut BrushSettings> {
        match self.tool {
            Tool::Brush => Some(&mut self.brush),
            Tool::Pencil => Some(&mut self.pencil),
            Tool::Eraser => Some(&mut self.eraser),
            _ => None,
        }
    }

    /// Choix d'un réglage nommé, import, papier et grain, pour l'outil courant (calque bitmap).
    pub(crate) fn brush_library_ui(&mut self, ui: &mut egui::Ui) {
        let target = match self.tool {
            Tool::Eraser => Target::Eraser,
            Tool::Brush => Target::Brush,
            _ => return self.grain_ui(ui),
        };
        let Some(current) = self.tool_brush_mut().copied() else { return };
        let presets: Vec<(usize, String, String)> = self
            .brushes
            .presets
            .iter()
            .enumerate()
            .filter(|(_, p)| p.target == target)
            .map(|(i, p)| (i, p.name.clone(), p.source.clone()))
            .collect();
        let selected = self
            .brushes
            .presets
            .iter()
            .find(|p| p.target == target && p.settings == current)
            .map_or("(réglé à la main)".to_string(), |p| p.name.clone());
        let mut pick = None;
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("reglage_pinceau")
                .selected_text(selected)
                .width(150.0)
                .show_ui(ui, |ui| {
                    let mut last_source = String::new();
                    for (i, name, source) in &presets {
                        if *source != last_source {
                            if !last_source.is_empty() {
                                ui.separator();
                            }
                            ui.label(RichText::new(source).small().weak());
                            last_source = source.clone();
                        }
                        if ui.selectable_label(false, name).clicked() {
                            pick = Some(*i);
                        }
                    }
                });
            if ui
                .small_button("Importer…")
                .on_hover_text("Importer des pinceaux d'un fichier de préréglages Toon Boom (.xml)")
                .clicked()
            {
                self.import_brushes();
            }
        });
        if let Some(i) = pick {
            let settings = self.brushes.presets[i].settings;
            if let Some(b) = self.tool_brush_mut() {
                *b = settings;
            }
        }
        self.grain_ui(ui);
    }

    /// Papier, grain et pointe en image du pinceau courant.
    fn grain_ui(&mut self, ui: &mut egui::Ui) {
        let papers = self.brushes.papers();
        let tip_name = self.tool_brush_mut().and_then(|b| b.tip).and_then(|id| self.brushes.tip_name(id).map(String::from));
        let Some(b) = self.tool_brush_mut() else { return };
        ui.horizontal(|ui| {
            ui.label("Papier");
            let name = b
                .paper
                .and_then(|id| papers.iter().find(|p| p.0 == id))
                .map_or("lisse".to_string(), |p| p.1.clone());
            egui::ComboBox::from_id_salt("papier_pinceau").selected_text(name).width(120.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut b.paper, None, "lisse");
                for (id, name) in &papers {
                    ui.selectable_value(&mut b.paper, Some(*id), name);
                }
            });
        });
        if b.paper.is_some() {
            ui.add(egui::Slider::new(&mut b.grain, 0.0..=1.0).text("Grain"))
                .on_hover_text("Le grain reste fixe sur le dessin ; appuyer plus fort remplit les creux du papier.");
        }
        if let Some(tip) = tip_name {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Pointe : {tip}")).small());
                if ui.small_button("ronde").on_hover_text("Revenir à une pointe ronde").clicked() {
                    b.tip = None;
                }
            });
        }
    }

    /// Importe un fichier de pinceaux Toon Boom ; réimporter le même fichier remplace ses pinceaux.
    pub(crate) fn import_brushes(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Pinceaux Toon Boom à importer")
            .add_filter("Préréglages de pinceaux (XML)", &["xml"])
            .pick_file()
        else {
            return;
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                self.status = format!("Lecture impossible : {e}");
                return;
            }
        };
        let source = path.file_name().map_or("import".into(), |n| n.to_string_lossy().into_owned());
        self.brushes.presets.retain(|p| p.source != source);
        match crate::tbbrush::import(&text, &source, &mut self.brushes) {
            Ok(r) => {
                let saved = match BrushLibrary::user_dir() {
                    Some(dir) => self.brushes.save(&dir).err(),
                    None => Some("dossier %APPDATA% introuvable".into()),
                };
                self.status = format!(
                    "{} pinceaux importés ({} pointes, {} papiers){}{}",
                    r.presets,
                    r.tips,
                    r.papers,
                    if r.skipped.is_empty() { String::new() } else { format!(" ; textures ignorées : {}", r.skipped.join(", ")) },
                    saved.map_or(String::new(), |e| format!(" ; non gardés pour la prochaine fois : {e}")),
                );
            }
            Err(e) => self.status = format!("Import impossible : {e}"),
        }
    }
}
