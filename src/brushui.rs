//! Réglages nommés, papier et grain des pinceaux (3d-2), dans le panneau Réglages ; les
//! réglages en grille, chacun avec l'aperçu de son trait et sa taille (6d).

use std::collections::HashMap;

use eframe::egui::{self, pos2, vec2, Color32, RichText, Sense, TextureHandle, Vec2};

use crate::app::{App, Tool};
use crate::brush::{stroke_preview, BrushSettings};
use crate::brushtex::{BrushLibrary, Target};

/// Case d'un réglage dans la grille, et l'aperçu du trait qu'elle contient.
const TILE: Vec2 = vec2(58.0, 48.0);
const PREVIEW: Vec2 = vec2(54.0, 30.0);

/// Hauteur d'un grand aperçu du panneau Pinceaux (11c), en points ; sa largeur suit le panneau,
/// par paliers de `WIDE_STEP` (pour ne pas refaire les aperçus à chaque point quand on tire un bord).
const WIDE_HEIGHT: f32 = 38.0;
const WIDE_STEP: f32 = 24.0;

/// Aperçus des réglages (par numéro de réglage), refaits quand le réglage ou l'échelle de
/// l'écran change : petits pour la grille des Réglages, grands pour le panneau Pinceaux.
#[derive(Default)]
pub(crate) struct BrushPreviews {
    cache: HashMap<usize, (BrushSettings, f32, TextureHandle)>,
    wide: HashMap<usize, (BrushSettings, [usize; 2], TextureHandle)>,
}

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
            ui.label(RichText::new(selected).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("Importer…")
                    .on_hover_text("Importer des pinceaux d'un fichier de préréglages Toon Boom (.xml)")
                    .clicked()
                {
                    self.import_brushes();
                }
            });
        });
        // Grille des réglages, par provenance : aperçu du trait et taille.
        let several = presets.iter().any(|p| p.2 != presets[0].2);
        let mut start = 0;
        while start < presets.len() {
            let source = presets[start].2.clone();
            let end = presets[start..].iter().position(|p| p.2 != source).map_or(presets.len(), |n| start + n);
            if several {
                ui.label(RichText::new(&source).small().weak());
            }
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
                for (i, name, _) in &presets[start..end] {
                    if self.preset_tile(ui, *i, name, &source, current).clicked() {
                        pick = Some(*i);
                    }
                }
            });
            start = end;
        }
        ui.add_space(2.0);
        if let Some(i) = pick {
            let settings = self.brushes.presets[i].settings;
            if let Some(b) = self.tool_brush_mut() {
                *b = settings;
            }
        }
        self.grain_ui(ui);
    }

    /// Case d'un réglage de la grille : aperçu du trait et taille ; nom et provenance au survol.
    fn preset_tile(&mut self, ui: &mut egui::Ui, index: usize, name: &str, source: &str, current: BrushSettings) -> egui::Response {
        let settings = self.brushes.presets[index].settings;
        let ppp = ui.ctx().pixels_per_point();
        let fresh = self.brush_previews.cache.get(&index).is_some_and(|(s, p, _)| *s == settings && *p == ppp);
        if !fresh {
            let (w, h) = ((PREVIEW.x * ppp).round() as usize, (PREVIEW.y * ppp).round() as usize);
            let alpha = stroke_preview(&settings, self.brushes.textures(&settings), w, h);
            let pixels = alpha.iter().map(|&a| Color32::from_rgba_premultiplied(a, a, a, a)).collect();
            let tex = ui.ctx().load_texture(
                format!("pinceau {index}"),
                egui::ColorImage { size: [w, h], pixels },
                egui::TextureOptions::LINEAR,
            );
            self.brush_previews.cache.insert(index, (settings, ppp, tex));
        }
        let tex = self.brush_previews.cache[&index].2.id();
        let (rect, resp) = ui.allocate_exact_size(TILE, Sense::click());
        let v = ui.visuals();
        let chosen = settings == current;
        let bg = if chosen {
            v.selection.bg_fill
        } else if resp.hovered() {
            v.widgets.hovered.weak_bg_fill
        } else {
            v.extreme_bg_color
        };
        let ink = if chosen { v.selection.stroke.color } else { v.text_color() };
        let painter = ui.painter();
        painter.rect_filled(rect, 4.0, bg);
        let image = egui::Rect::from_min_size(rect.min + vec2((TILE.x - PREVIEW.x) * 0.5, 2.0), PREVIEW);
        let (papier, trait_) = self.preview_colors(index);
        painter.rect_filled(image, 2.0, papier);
        painter.image(tex, image, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), trait_);
        let size = if settings.size < 10.0 { format!("{:.1}", settings.size) } else { format!("{:.0}", settings.size) };
        painter.text(
            pos2(rect.center().x, rect.bottom() - 8.0),
            egui::Align2::CENTER_CENTER,
            size,
            egui::FontId::proportional(11.0),
            ink,
        );
        resp.on_hover_text(format!("{name}\n{source}"))
    }

    /// Panneau Pinceaux (11c) : les réglages nommés en liste, chacun avec un grand aperçu de son
    /// trait tracé par le moteur, son nom, sa taille, et s'il est texturé ou importé. Ceux de la
    /// gomme avec la gomme, sinon ceux du pinceau ; en choisir un avec un autre outil que le
    /// pinceau ou la gomme prend le pinceau. Le choix est le même que dans la grille des Réglages.
    pub(crate) fn brushes_panel(&mut self, ui: &mut egui::Ui) {
        let target = if self.tool == Tool::Eraser { Target::Eraser } else { Target::Brush };
        let current = match target {
            Target::Eraser => self.eraser,
            Target::Brush => self.brush,
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(if target == Target::Eraser { "Gommes" } else { "Pinceaux" }).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("Importer…")
                    .on_hover_text("Importer des pinceaux d'un fichier de préréglages Toon Boom (.xml) ; ils restent pour les fois suivantes")
                    .clicked()
                {
                    self.import_brushes();
                }
            });
        });
        let presets: Vec<(usize, String, String)> = self
            .brushes
            .presets
            .iter()
            .enumerate()
            .filter(|(_, p)| p.target == target)
            .map(|(i, p)| (i, p.name.clone(), p.source.clone()))
            .collect();
        let mut pick = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let mut last_source = String::new();
            for (i, name, source) in &presets {
                if *source != last_source {
                    ui.add_space(4.0);
                    let title = if source == "Pelure" { "Pelure".to_string() } else { format!("Importés de {source}") };
                    ui.label(RichText::new(title).small().weak());
                    last_source = source.clone();
                }
                if self.preset_row(ui, *i, name, source, current).clicked() {
                    pick = Some(*i);
                }
            }
            if presets.is_empty() {
                ui.label(RichText::new("Aucun réglage pour cet outil.").weak());
            }
        });
        if let Some(i) = pick {
            let settings = self.brushes.presets[i].settings;
            match target {
                Target::Eraser => self.eraser = settings,
                Target::Brush => {
                    self.brush = settings;
                    if self.tool != Tool::Brush {
                        self.set_tool(Tool::Brush);
                    }
                }
            }
            self.status = format!("Réglage « {} ».", self.brushes.presets[i].name);
        }
    }

    /// Ligne d'un réglage du panneau Pinceaux : nom, étiquettes, taille, puis l'aperçu du trait
    /// sur toute la largeur.
    fn preset_row(&mut self, ui: &mut egui::Ui, index: usize, name: &str, source: &str, current: BrushSettings) -> egui::Response {
        let settings = self.brushes.presets[index].settings;
        let ppp = ui.ctx().pixels_per_point();
        let width = ui.available_width().max(60.0);
        // Largeur de l'aperçu par paliers ; taille en pixels réels de l'écran.
        let shown = ((width - 12.0) / WIDE_STEP).floor().max(2.0) * WIDE_STEP;
        let px = [(shown * ppp).round() as usize, (WIDE_HEIGHT * ppp).round() as usize];
        let fresh = self.brush_previews.wide.get(&index).is_some_and(|(s, size, _)| *s == settings && *size == px);
        if !fresh {
            let alpha = stroke_preview(&settings, self.brushes.textures(&settings), px[0], px[1]);
            let pixels = alpha.iter().map(|&a| Color32::from_rgba_premultiplied(a, a, a, a)).collect();
            let tex = ui.ctx().load_texture(format!("pinceau large {index}"), egui::ColorImage { size: px, pixels }, egui::TextureOptions::LINEAR);
            self.brush_previews.wide.insert(index, (settings, px, tex));
        }
        let tex = self.brush_previews.wide[&index].2.id();
        let textured = settings.tip.is_some() || (settings.paper.is_some() && settings.grain > 0.0);
        let (rect, resp) = ui.allocate_exact_size(vec2(width, 22.0 + WIDE_HEIGHT + 6.0), Sense::click());
        let v = ui.visuals();
        let chosen = settings == current;
        let bg = if chosen {
            v.selection.bg_fill
        } else if resp.hovered() {
            v.widgets.hovered.weak_bg_fill
        } else {
            v.extreme_bg_color
        };
        let ink = if chosen { v.selection.stroke.color } else { v.text_color() };
        // Sur la ligne choisie (fond de sélection), le texte secondaire prend la couleur du nom.
        let weak = if chosen { ink } else { v.weak_text_color() };
        let painter = ui.painter();
        painter.rect_filled(rect.shrink2(vec2(0.0, 1.0)), 4.0, bg);
        // Nom, étiquettes, taille.
        let font = egui::FontId::proportional(13.0);
        let small = egui::FontId::proportional(10.5);
        let mut x = rect.left() + 8.0;
        let y = rect.top() + 12.0;
        let galley = painter.layout_no_wrap(name.to_string(), font, ink);
        let w = galley.size().x;
        painter.galley(pos2(x, y - galley.size().y / 2.0), galley, ink);
        x += w + 8.0;
        let mut tag = |text: &str| {
            let g = painter.layout_no_wrap(text.to_string(), small.clone(), weak);
            let r = egui::Rect::from_min_size(pos2(x, y - 7.0), g.size() + vec2(8.0, 2.0));
            painter.rect_filled(r, 3.0, v.widgets.noninteractive.bg_fill);
            painter.galley(r.min + vec2(4.0, 1.0), g, weak);
            x = r.right() + 4.0;
        };
        if textured {
            tag("texture");
        }
        if source != "Pelure" {
            tag("importé");
        }
        let size = if settings.size < 10.0 { format!("{:.1} px", settings.size) } else { format!("{:.0} px", settings.size) };
        painter.text(pos2(rect.right() - 8.0, y), egui::Align2::RIGHT_CENTER, size, small, weak);
        // L'aperçu du trait.
        let image = egui::Rect::from_min_size(pos2(rect.left() + 6.0, rect.top() + 22.0), vec2(shown, WIDE_HEIGHT));
        let (papier, trait_) = self.preview_colors(index);
        painter.rect_filled(image, 3.0, papier);
        painter.image(tex, image, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), trait_);
        resp.on_hover_text(format!("{name}\n{source}"))
    }

    /// Couleurs d'un aperçu (12b) : le papier du projet, et le trait dans la couleur courante ; une
    /// gomme en gris (le trait qu'elle enlève).
    fn preview_colors(&self, index: usize) -> (Color32, Color32) {
        let [r, g, b] = self.doc.paper;
        let gomme = self.brushes.presets.get(index).is_some_and(|p| p.target == Target::Eraser);
        let [cr, cg, cb] = self.color;
        let trait_ = if gomme { Color32::from_gray(150) } else { Color32::from_rgb(cr, cg, cb) };
        (Color32::from_rgb(r, g, b), trait_)
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
                self.brushes.share();
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
