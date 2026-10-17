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

/// Bulle du bouton « Importer des pinceaux » (Réglages et panneau Pinceaux).
const IMPORT_HINT: &str = "Brosses TVPaint (.tvpx, panneau .bin, réglage .ini) ou préréglages Toon Boom (.xml) ; elles restent pour les fois suivantes.";

/// Nom d'un réglage en cours de saisie (moteur de dessin, étape 9) : enregistrer le réglage
/// courant dans « Mes brosses », ou renommer le réglage `rename`.
pub(crate) struct PresetEdit {
    pub name: String,
    pub target: Target,
    pub rename: Option<usize>,
    focused: bool,
}

/// Ce que demande le menu d'un réglage (clic droit).
#[derive(Clone, Copy)]
enum PresetMenu {
    Rename,
    Duplicate,
    Update,
    Delete,
}

/// Le menu d'un réglage : renommer, dupliquer dans « Mes brosses », remplacer par le réglage
/// courant, supprimer. Ceux de Pelure ne se dupliquent que.
fn preset_menu(ui: &mut egui::Ui, source: &str) -> Option<PresetMenu> {
    let mut out = None;
    let own = source != "Pelure";
    if own && ui.button("Renommer…").clicked() {
        out = Some(PresetMenu::Rename);
    }
    if ui.button("Dupliquer dans « Mes brosses »").clicked() {
        out = Some(PresetMenu::Duplicate);
    }
    if own && ui.button("Remplacer par le réglage courant").clicked() {
        out = Some(PresetMenu::Update);
    }
    if own && ui.button("Supprimer").clicked() {
        out = Some(PresetMenu::Delete);
    }
    if out.is_some() {
        ui.close_menu();
    }
    out
}

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

    /// Garde la bibliothèque (importés et « Mes brosses ») ; l'erreur s'il y en a une.
    fn save_brushes(&self) -> Option<String> {
        match BrushLibrary::user_dir() {
            Some(dir) => self.brushes.save(&dir).err(),
            None => Some("dossier %APPDATA% introuvable".into()),
        }
    }

    /// Commence l'enregistrement du réglage courant dans « Mes brosses ».
    fn start_save_preset(&mut self, target: Target) {
        let name = self.brushes.free_name("Ma brosse", target);
        self.preset_edit = Some(PresetEdit { name, target, rename: None, focused: false });
    }

    /// La ligne du nom en cours de saisie (enregistrer ou renommer), s'il y en a une pour cet outil.
    fn preset_edit_row(&mut self, ui: &mut egui::Ui, target: Target, current: BrushSettings) {
        let Some(edit) = self.preset_edit.as_mut().filter(|e| e.target == target) else { return };
        let mut done = None;
        ui.horizontal(|ui| {
            ui.label(if edit.rename.is_some() { "Nouveau nom" } else { "Enregistrer sous" });
            let resp = ui.add(egui::TextEdit::singleline(&mut edit.name).desired_width(140.0));
            if !edit.focused {
                resp.request_focus();
                edit.focused = true;
            }
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button("OK").clicked() || enter {
                done = Some(true);
            }
            if ui.button("Annuler").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                done = Some(false);
            }
        });
        if done == Some(true) {
            let Some(edit) = self.preset_edit.take() else { return };
            let name = edit.name.trim().to_string();
            match edit.rename {
                Some(i) if i < self.brushes.presets.len() && !name.is_empty() => self.brushes.presets[i].name = name.clone(),
                Some(_) => {}
                None => {
                    self.brushes.save_mine(&name, target, current);
                }
            }
            let err = self.save_brushes();
            self.status = format!("Réglage « {name} » gardé dans « Mes brosses »{}", err.map_or(String::new(), |e| format!(" (pas pour la prochaine fois : {e})")));
        } else if done == Some(false) {
            self.preset_edit = None;
        }
    }

    /// Fait ce que demande le menu du réglage `i`.
    fn apply_preset_menu(&mut self, i: usize, action: PresetMenu, target: Target, current: BrushSettings) {
        let Some(p) = self.brushes.presets.get(i).cloned() else { return };
        match action {
            PresetMenu::Rename => {
                self.preset_edit = Some(PresetEdit { name: p.name, target, rename: Some(i), focused: false });
                return;
            }
            PresetMenu::Duplicate => {
                let name = self.brushes.free_name(&p.name, target);
                self.brushes.save_mine(&name, target, p.settings);
                self.status = format!("« {name} » ajouté à « Mes brosses ».");
            }
            PresetMenu::Update => {
                self.brushes.presets[i].settings = current;
                self.status = format!("« {} » prend le réglage courant.", p.name);
            }
            PresetMenu::Delete => {
                self.brushes.presets.remove(i);
                // Les aperçus sont rangés par rang : ils se refont.
                self.brush_previews = BrushPreviews::default();
                self.status = format!("« {} » supprimé.", p.name);
            }
        }
        if let Some(e) = self.save_brushes() {
            self.status = format!("{} (pas gardé pour la prochaine fois : {e})", self.status);
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
        let mut menu = None;
        let mut save = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new(selected).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::bulles::icone(ui, egui_phosphor::regular::FILE_ARROW_DOWN, "Importer des pinceaux", IMPORT_HINT, "").clicked() {
                    self.import_brushes();
                }
                save = crate::bulles::icone(ui, egui_phosphor::regular::FLOPPY_DISK, "Enregistrer le réglage", "Garder le réglage courant sous un nom, dans « Mes brosses » (clic droit sur un réglage : renommer, dupliquer, remplacer, supprimer).", "").clicked();
            });
        });
        if save {
            self.start_save_preset(target);
        }
        self.preset_edit_row(ui, target, current);
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
                    let resp = self.preset_tile(ui, *i, name, &source, current);
                    if resp.clicked() {
                        pick = Some(*i);
                    }
                    resp.context_menu(|ui| {
                        if let Some(a) = preset_menu(ui, &source) {
                            menu = Some((*i, a));
                        }
                    });
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
        if let Some((i, a)) = menu {
            self.apply_preset_menu(i, a, target, current);
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
        crate::bulles::bulle(resp, &name, &source, "")
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
        let mut save = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new(if target == Target::Eraser { "Gommes" } else { "Pinceaux" }).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::bulles::icone(ui, egui_phosphor::regular::FILE_ARROW_DOWN, "Importer des pinceaux", IMPORT_HINT, "").clicked() {
                    self.import_brushes();
                }
                save = crate::bulles::icone(ui, egui_phosphor::regular::FLOPPY_DISK, "Enregistrer le réglage", "Garder le réglage courant sous un nom, dans « Mes brosses » (clic droit sur un réglage : renommer, dupliquer, remplacer, supprimer).", "").clicked();
            });
        });
        if save {
            self.start_save_preset(target);
        }
        self.preset_edit_row(ui, target, current);
        let presets: Vec<(usize, String, String)> = self
            .brushes
            .presets
            .iter()
            .enumerate()
            .filter(|(_, p)| p.target == target)
            .map(|(i, p)| (i, p.name.clone(), p.source.clone()))
            .collect();
        let mut pick = None;
        let mut menu = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let mut last_source = String::new();
            for (i, name, source) in &presets {
                if *source != last_source {
                    ui.add_space(4.0);
                    let title = if source == "Pelure" || source == crate::brushtex::MINE { source.clone() } else { format!("Importés de {source}") };
                    ui.label(RichText::new(title).small().weak());
                    last_source = source.clone();
                }
                let resp = self.preset_row(ui, *i, name, source, current);
                if resp.clicked() {
                    pick = Some(*i);
                }
                resp.context_menu(|ui| {
                    if let Some(a) = preset_menu(ui, source) {
                        menu = Some((*i, a));
                    }
                });
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
        if let Some((i, a)) = menu {
            self.apply_preset_menu(i, a, target, current);
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
        if source == crate::brushtex::MINE {
            tag("à moi");
        } else if source != "Pelure" {
            tag("importé");
        }
        let size = if settings.size < 10.0 { format!("{:.1} px", settings.size) } else { format!("{:.0} px", settings.size) };
        painter.text(pos2(rect.right() - 8.0, y), egui::Align2::RIGHT_CENTER, size, small, weak);
        // L'aperçu du trait.
        let image = egui::Rect::from_min_size(pos2(rect.left() + 6.0, rect.top() + 22.0), vec2(shown, WIDE_HEIGHT));
        let (papier, trait_) = self.preview_colors(index);
        painter.rect_filled(image, 3.0, papier);
        painter.image(tex, image, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), trait_);
        crate::bulles::bulle(resp, &name, &source, "")
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
        let mut import = false;
        let mut import_tip = false;
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
            // Une image comme papier (moteur de dessin, étape 7) : par exemple un papier PNG de
            // TVPaint, choisi par Anthony.
            import = crate::bulles::icone(ui, egui_phosphor::regular::IMAGE_SQUARE, "Importer un papier", "Une image (PNG, JPG…) devient un papier : son gris est le relief.", "").clicked();
        });
        if b.paper.is_some() {
            let resp = ui.add(egui::Slider::new(&mut b.grain, 0.0..=1.0).text("Grain"));
            crate::bulles::bulle(resp, "Grain", "Le grain reste fixe sur le dessin ; appuyer plus fort remplit les creux du papier.", "");
            let resp = ui.add(egui::Slider::new(&mut b.paper_scale, 0.1..=8.0).logarithmic(true).text("Taille du papier"));
            crate::bulles::bulle(resp, "Taille du papier", "Plus grand : un grain plus large.", "");
            ui.horizontal(|ui| {
                let resp = ui.add(egui::Slider::new(&mut b.paper_angle, -180.0..=180.0).suffix("°").text("Angle"));
                crate::bulles::bulle(resp, "Angle du papier", "Tourne le grain du papier.", "");
                ui.checkbox(&mut b.paper_invert, "Inverser").on_hover_text("Les creux du papier deviennent des sommets.");
            });
        }
        ui.horizontal(|ui| {
            match &tip_name {
                Some(tip) => {
                    ui.label(RichText::new(format!("Pointe : {tip}")).small());
                    if crate::bulles::icone(ui, egui_phosphor::regular::CIRCLE, "Pointe ronde", "Revenir à une pointe ronde.", "").clicked() {
                        b.tip = None;
                    }
                }
                None => {
                    ui.label(RichText::new("Pointe ronde").small());
                }
            }
            // Une image comme pointe (moteur de dessin, étape 8) : une seule, ou plusieurs dessins
            // côte à côte (pointe en plusieurs cases).
            import_tip = crate::bulles::icone(ui, egui_phosphor::regular::PAINT_BRUSH_HOUSEHOLD, "Importer une pointe", "Une image devient la pointe : sa transparence, ou son noir sur fond blanc. Plusieurs dessins côte à côte : une pointe en plusieurs cases.", "").clicked();
        });
        if b.tip.is_some() {
            ui.horizontal(|ui| {
                let resp = ui.add(egui::Slider::new(&mut b.tip_cells, 1..=16).text("Cases"));
                crate::bulles::bulle(resp, "Cases", "Nombre de dessins côte à côte dans l'image de la pointe ; chaque empreinte en prend un.", "");
                if b.tip_cells > 1 {
                    egui::ComboBox::from_id_salt("pointe_ordre").selected_text(b.tip_order.label()).width(120.0).show_ui(ui, |ui| {
                        for o in crate::brush::TipOrder::ALL {
                            ui.selectable_value(&mut b.tip_order, o, o.label());
                        }
                    });
                }
            });
        }
        if import {
            self.import_paper();
        }
        if import_tip {
            self.import_tip();
        }
    }

    /// Une image choisie devient une pointe de la bibliothèque, gardée pour les fois suivantes, et
    /// la pointe de l'outil courant.
    fn import_tip(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Image à prendre comme pointe")
            .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "tif", "tiff", "webp"])
            .pick_file()
        else {
            return;
        };
        let img = match image::open(&path) {
            Ok(i) => i,
            Err(e) => {
                self.status = format!("Image illisible : {e}");
                return;
            }
        };
        let name = path.file_stem().map_or("pointe".into(), |n| n.to_string_lossy().into_owned());
        let id = self.brushes.add_tip(&name, crate::brushtex::GrayImage::tip_from_image(&img, 2048));
        self.brushes.share();
        if let Some(b) = self.tool_brush_mut() {
            b.tip = Some(id);
            b.tip_cells = 1;
        }
        let saved = match BrushLibrary::user_dir() {
            Some(dir) => self.brushes.save(&dir).err(),
            None => Some("dossier %APPDATA% introuvable".into()),
        };
        self.status = format!(
            "Pointe « {name} » importée (plusieurs dessins côte à côte : régler « Cases »){}",
            saved.map_or(String::new(), |e| format!(" ; non gardée pour la prochaine fois : {e}"))
        );
    }

    /// Importe des brosses TVPaint ; le compte rendu (ce qui n'est pas repris) va dans la fenêtre
    /// des remarques d'import.
    fn import_tvpaint(&mut self, path: &std::path::Path, ext: &str) {
        let source = path.file_name().map_or("import".into(), |n| n.to_string_lossy().into_owned());
        self.brushes.presets.retain(|p| p.source != source);
        let res = crate::tvpbrush::tvpaint_resources();
        let result = match ext {
            "tvpx" => std::fs::read(path)
                .map_err(|e| format!("lecture impossible : {e}"))
                .and_then(|b| crate::tvpbrush::Panel::from_tvpx(&b))
                .map(|p| crate::tvpbrush::import_panel(&p, &source, &mut self.brushes, res.as_deref())),
            "bin" => crate::tvpbrush::Panel::from_bin(path).map(|p| crate::tvpbrush::import_panel(&p, &source, &mut self.brushes, res.as_deref())),
            _ => crate::tvpbrush::import_ini(path, &source, &mut self.brushes),
        };
        match result {
            Ok(r) => {
                self.brushes.share();
                self.brush_previews = BrushPreviews::default();
                let saved = self.save_brushes();
                self.status = format!(
                    "{} brosse(s) TVPaint importée(s) ({} pointe(s), {} papier(s)){}{}",
                    r.presets,
                    r.tips,
                    r.papers,
                    if r.notes.is_empty() { String::new() } else { format!(" ; {} remarque(s) dans la fenêtre d'import", r.notes.len()) },
                    saved.map_or(String::new(), |e| format!(" ; non gardées pour la prochaine fois : {e}")),
                );
                self.brush_import_notes = (!r.notes.is_empty()).then(|| (source, r.notes));
            }
            Err(e) => self.status = format!("Import impossible : {e}"),
        }
    }

    /// La fenêtre des remarques du dernier import de brosses (ce qui n'a pas pu être repris).
    pub(crate) fn brush_import_window(&mut self, ctx: &egui::Context) {
        let Some((source, notes)) = &self.brush_import_notes else { return };
        let mut open = true;
        egui::Window::new(format!("Import de « {source} »")).open(&mut open).resizable(true).default_width(460.0).show(ctx, |ui| {
            ui.label(RichText::new("Ce qui n'a pas pu être repris tel quel :").strong());
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                for n in notes {
                    ui.label(format!("• {n}"));
                }
            });
        });
        if !open {
            self.brush_import_notes = None;
        }
    }

    /// Une image choisie devient un papier de la bibliothèque, gardé pour les fois suivantes, et
    /// le papier de l'outil courant.
    fn import_paper(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Image à prendre comme papier")
            .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "tif", "tiff", "webp"])
            .pick_file()
        else {
            return;
        };
        let img = match image::open(&path) {
            Ok(i) => i,
            Err(e) => {
                self.status = format!("Image illisible : {e}");
                return;
            }
        };
        let name = path.file_stem().map_or("papier".into(), |n| n.to_string_lossy().into_owned());
        let id = self.brushes.add_paper(&name, crate::brushtex::GrayImage::from_image(&img, 1024));
        self.brushes.share();
        if let Some(b) = self.tool_brush_mut() {
            b.paper = Some(id);
            if b.grain <= 0.0 {
                b.grain = 1.0;
            }
        }
        let saved = match BrushLibrary::user_dir() {
            Some(dir) => self.brushes.save(&dir).err(),
            None => Some("dossier %APPDATA% introuvable".into()),
        };
        self.status = format!("Papier « {name} » importé{}", saved.map_or(String::new(), |e| format!(" ; non gardé pour la prochaine fois : {e}")));
    }

    /// Importe un fichier de brosses : TVPaint (`.tvpx`, `.bin`, `.ini`, moteur de dessin,
    /// étape 10) ou Toon Boom (`.xml`) ; réimporter le même fichier remplace ses pinceaux.
    pub(crate) fn import_brushes(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Brosses à importer")
            .add_filter("Brosses TVPaint et Toon Boom", &["tvpx", "bin", "ini", "xml"])
            .add_filter("TVPaint (export de panneau, panneau, réglage)", &["tvpx", "bin", "ini"])
            .add_filter("Préréglages Toon Boom (XML)", &["xml"])
            .pick_file()
        else {
            return;
        };
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if matches!(ext.as_str(), "tvpx" | "bin" | "ini") {
            self.import_tvpaint(&path, &ext);
            return;
        }
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
