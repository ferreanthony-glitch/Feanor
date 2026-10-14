//! Outils Tampon et Tampon de duplication (5e), sur un calque bitmap. Moteur dans `stamp.rs`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{self, vec2, Color32, Pos2, Stroke, TextureHandle, Vec2};

use crate::app::{App, Tool};
use crate::document::{CelId, LayerKind};
use crate::history::Edit;
use crate::raster::Tile;
use crate::stamp::{self, CloneSource, CloneStroke, Cycle, StampImage, StampSettings, StampTrail};

/// Tampons disponibles (ceux de Pelure, puis les images importées), ceux choisis, réglages.
pub(crate) struct StampLibrary {
    pub images: Vec<Arc<StampImage>>,
    pub selected: Vec<usize>,
    pub settings: StampSettings,
    /// Image du cycle où reprendre (si on ne repart pas de la première à chaque trait).
    pub next_image: usize,
    thumbs: HashMap<usize, TextureHandle>,
}

/// Dossier des images de tampons importées (%APPDATA%\Pelure\tampons).
fn user_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Pelure").join("tampons"))
}

/// Une image de tampon (PNG, JPEG) ; réduite si elle dépasse 512 pixels de côté.
fn load_image(path: &std::path::Path) -> Result<StampImage, String> {
    let img = image::open(path).map_err(|e| format!("{} : {e}", path.display()))?;
    let img = if img.width().max(img.height()) > 512 { img.thumbnail(512, 512) } else { img };
    let rgba = img.to_rgba8();
    let name = path.file_stem().map_or_else(|| "image".into(), |s| s.to_string_lossy().into_owned());
    Ok(StampImage::from_rgba(name, rgba.width() as usize, rgba.height() as usize, rgba.as_raw()))
}

impl StampLibrary {
    pub fn load() -> StampLibrary {
        let mut images = stamp::builtin();
        if let Some(dir) = user_dir() {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
                paths.sort();
                for p in paths {
                    if let Ok(img) = load_image(&p) {
                        images.push(Arc::new(img));
                    }
                }
            }
        }
        StampLibrary { images, selected: vec![1], settings: StampSettings::default(), next_image: 0, thumbs: HashMap::new() }
    }
}

/// Réglages du tampon de duplication.
pub(crate) struct CloneSettings {
    pub size: f32,
    pub hardness: f32,
    pub opacity: f32,
    /// La source suit le pinceau d'un trait à l'autre (sinon chaque trait repart de la source).
    pub aligned: bool,
    /// Copier ce que l'on voit (tous les calques) plutôt que le seul calque courant.
    pub sample_all: bool,
    pub source: Option<Pos2>,
    /// Décalage du pinceau vers la source, fixé au premier trait.
    pub offset: Option<Vec2>,
}

impl Default for CloneSettings {
    fn default() -> Self {
        CloneSettings { size: 40.0, hardness: 0.5, opacity: 1.0, aligned: true, sample_all: false, source: None, offset: None }
    }
}

/// Trait de tampon en cours.
pub(crate) struct StampStroke {
    cel: CelId,
    prefix: Option<Edit>,
    trail: StampTrail,
    images: Vec<Arc<StampImage>>,
    settings: StampSettings,
    color: [u8; 3],
    before: HashMap<usize, Option<Tile>>,
    pub pen: bool,
}

/// Trait de tampon de duplication en cours.
pub(crate) struct CloneStrokeState {
    cel: CelId,
    prefix: Option<Edit>,
    stroke: CloneStroke,
    offset: Vec2,
}

impl App {
    /// Vrai si le calque courant accepte un outil de pixels (sinon message).
    fn bitmap_tool_ready(&mut self, what: &str) -> bool {
        if !self.layer_editable() {
            return false;
        }
        if self.layer().kind != LayerKind::Bitmap {
            self.status = format!("{what} peint sur un calque bitmap (+ Bitmap dans les calques).");
            return false;
        }
        true
    }

    pub(crate) fn stamp_press(&mut self, p: Pos2, pressure: f32, pen: bool) {
        self.end_stroke();
        self.playing = false;
        if !self.bitmap_tool_ready("Le tampon") {
            return;
        }
        let images: Vec<Arc<StampImage>> =
            self.stamps.selected.iter().filter_map(|&i| self.stamps.images.get(i).cloned()).collect();
        if images.is_empty() {
            self.status = "Choisis au moins un tampon dans le panneau Réglages.".into();
            return;
        }
        let settings = self.stamps.settings;
        let (cel, prefix) = self.ensure_cel();
        let first = if settings.reset_cycle { 0 } else { self.stamps.next_image };
        let seed = self.ctx.input(|i| i.time).to_bits();
        let trail = StampTrail::new(settings, images.len(), seed, first);
        self.stamp_stroke = Some(StampStroke { cel, prefix, trail, images, settings, color: self.color, before: HashMap::new(), pen });
        self.stamp_to(p, pressure);
        self.modified = true;
    }

    pub(crate) fn stamp_to(&mut self, p: Pos2, pressure: f32) {
        let Some(st) = &mut self.stamp_stroke else { return };
        let Some(c) = self.doc.cels.get_mut(&st.cel) else { return };
        let mut dirty = Vec::new();
        for d in st.trail.add_point(p, pressure) {
            dirty.extend(stamp::place(&mut c.raster, &st.images[d.image], &d, &st.settings, st.color, &mut st.before));
        }
        self.renderer.invalidate_tiles(&dirty);
    }

    /// Fin d'un trait de tampon ou de duplication : une étape d'annulation.
    pub(crate) fn finish_stamps(&mut self) {
        if let Some(st) = self.stamp_stroke.take() {
            self.stamps.next_image = st.trail.next_image();
            self.push_tiles_edit(st.cel, st.prefix, st.before);
        }
        if let Some(cs) = self.clone_stroke.take() {
            if self.clone.aligned {
                self.clone.offset = Some(cs.offset);
            }
            self.push_tiles_edit(cs.cel, cs.prefix, cs.stroke.before);
        }
    }

    fn push_tiles_edit(&mut self, cel: CelId, prefix: Option<Edit>, before: HashMap<usize, Option<Tile>>) {
        let tiles: Vec<(usize, Option<Tile>)> = before.into_iter().collect();
        if tiles.is_empty() && prefix.is_none() {
            return;
        }
        if let Some(c) = self.doc.cels.get_mut(&cel) {
            c.version += 1;
        }
        let edit = Edit::Tiles { cel, tiles };
        self.history.push(match prefix {
            Some(p) => Edit::Group(vec![p, edit]),
            None => edit,
        });
        self.modified = true;
    }

    /// Tampon de duplication : Ctrl + clic pose la source ; sinon on peint.
    pub(crate) fn clone_press(&mut self, p: Pos2, ctrl: bool) {
        self.end_stroke();
        self.playing = false;
        if ctrl {
            self.clone.source = Some(p);
            self.clone.offset = None;
            self.status = "Source du tampon de duplication posée : peins ailleurs pour la recopier.".into();
            return;
        }
        if !self.bitmap_tool_ready("Le tampon de duplication") {
            return;
        }
        let Some(src) = self.clone.source else {
            self.status = "Tampon de duplication : Ctrl + clic pour choisir d'abord la source.".into();
            return;
        };
        let offset = match (self.clone.aligned, self.clone.offset) {
            (true, Some(o)) => o,
            _ => src - p,
        };
        let (cel, prefix) = self.ensure_cel();
        let source = if self.clone.sample_all {
            let plan = self.build_plan(self.cur_frame, false, false);
            CloneSource::Composite {
                pixels: crate::render::composite_full(&self.doc, &plan),
                width: self.doc.width,
                height: self.doc.height,
            }
        } else {
            let raster = self.doc.cels.get(&cel).map(|c| c.raster.clone()).unwrap_or_else(|| crate::raster::Raster::new(self.doc.width, self.doc.height));
            CloneSource::Layer(raster)
        };
        let s = &self.clone;
        let stroke = CloneStroke::new(source, offset, s.size, s.hardness, s.opacity);
        self.clone_stroke = Some(CloneStrokeState { cel, prefix, stroke, offset });
        self.clone_to(p);
        self.modified = true;
    }

    pub(crate) fn clone_to(&mut self, p: Pos2) {
        let Some(cs) = &mut self.clone_stroke else { return };
        let Some(c) = self.doc.cels.get_mut(&cs.cel) else { return };
        let dirty = cs.stroke.add_point(&mut c.raster, p);
        self.renderer.invalidate_tiles(&dirty);
    }

    /// Cercle du tampon (et viseur de la source du tampon de duplication) ; vrai si dessiné.
    pub(crate) fn paint_stamp_cursor(&self, ctx: &egui::Context, painter: &egui::Painter, p: Pos2) -> bool {
        let size = match self.tool {
            Tool::Stamp => self.stamps.settings.size,
            Tool::Clone => self.clone.size,
            _ => return false,
        };
        ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
        let r = (size * 0.5 * self.layer_zoom()).max(1.5);
        painter.circle_stroke(p, r, Stroke::new(1.0_f32, Color32::from_black_alpha(170)));
        painter.circle_stroke(p, r + 1.0, Stroke::new(1.0_f32, Color32::from_white_alpha(150)));
        if self.tool == Tool::Clone {
            // La source : là où elle est, ou là où elle suivra le pinceau.
            let at = match (self.clone.offset.or(self.clone_stroke.as_ref().map(|c| c.offset)), self.hover_canvas) {
                (Some(o), Some(h)) if self.clone.aligned || self.clone_stroke.is_some() => Some(self.to_layer(h) + o),
                _ => self.clone.source,
            };
            if let Some(s) = at {
                let q = self.layer_to_screen(s);
                let st = Stroke::new(1.5_f32, Color32::from_rgb(255, 120, 60));
                painter.line_segment([q - vec2(8.0, 0.0), q + vec2(8.0, 0.0)], st);
                painter.line_segment([q - vec2(0.0, 8.0), q + vec2(0.0, 8.0)], st);
                painter.circle_stroke(q, r, Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 120, 60, 120)));
            }
        }
        true
    }

    /// Importe des images (PNG, JPEG) comme tampons ; elles sont gardées pour les fois suivantes.
    fn import_stamps(&mut self) {
        let Some(paths) = rfd::FileDialog::new()
            .set_title("Importer des images de tampon")
            .add_filter("Images", &["png", "jpg", "jpeg"])
            .pick_files()
        else {
            return;
        };
        let dir = user_dir();
        let mut added = 0;
        for path in paths {
            match load_image(&path) {
                Ok(img) => {
                    if let (Some(dir), Some(name)) = (&dir, path.file_name()) {
                        let _ = std::fs::create_dir_all(dir);
                        let _ = std::fs::copy(&path, dir.join(name));
                    }
                    self.stamps.images.push(Arc::new(img));
                    added += 1;
                }
                Err(e) => self.status = format!("Image non importée : {e}"),
            }
        }
        if added > 0 {
            let n = self.stamps.images.len();
            self.stamps.selected = ((n - added)..n).collect();
            self.status = format!("{added} image(s) de tampon importée(s) et choisie(s).");
        }
    }

    fn stamp_thumb(&mut self, ctx: &egui::Context, i: usize) -> Option<TextureHandle> {
        if let Some(t) = self.stamps.thumbs.get(&i) {
            return Some(t.clone());
        }
        let img = self.stamps.images.get(i)?.clone();
        let image = egui::ColorImage { size: [img.w, img.h], pixels: img.pixels.clone() };
        let t = ctx.load_texture(format!("tampon {i}"), image, egui::TextureOptions::LINEAR);
        self.stamps.thumbs.insert(i, t.clone());
        Some(t)
    }

    /// Réglages de l'outil Tampon (panneau Réglages).
    pub(crate) fn stamp_options_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
            for i in 0..self.stamps.images.len() {
                let Some(tex) = self.stamp_thumb(ui.ctx(), i) else { continue };
                let selected = self.stamps.selected.contains(&i);
                let name = self.stamps.images[i].name.clone();
                let img = egui::Image::new((tex.id(), vec2(36.0, 36.0)));
                let resp = ui.add(egui::ImageButton::new(img).selected(selected)).on_hover_text(format!(
                    "{name}\nClic : choisir ce tampon · Ctrl + clic : l'ajouter au cycle"
                ));
                if resp.clicked() {
                    if ui.input(|i| i.modifiers.command) {
                        if selected && self.stamps.selected.len() > 1 {
                            self.stamps.selected.retain(|&k| k != i);
                        } else if !selected {
                            self.stamps.selected.push(i);
                        }
                    } else {
                        self.stamps.selected = vec![i];
                    }
                }
            }
        });
        if ui.button("Importer des images…").on_hover_text("PNG avec transparence, ou JPEG ; gardées pour les fois suivantes").clicked() {
            self.import_stamps();
        }
        let s = &mut self.stamps.settings;
        let pct = |v: f64, _: std::ops::RangeInclusive<usize>| format!("{:.0} %", v * 100.0);
        ui.add(egui::Slider::new(&mut s.size, 2.0..=1000.0).logarithmic(true).text("Taille max"));
        ui.add(egui::Slider::new(&mut s.min_size, 0.0..=1.0).custom_formatter(pct).text("Taille min (pression)"));
        ui.add(egui::Slider::new(&mut s.spacing, 0.05..=10.0).logarithmic(true).custom_formatter(pct).text("Espacement"));
        ui.add(egui::Slider::new(&mut s.angle, -180.0..=180.0).suffix("°").text("Angle"));
        ui.add(egui::Slider::new(&mut s.roundness, 0.05..=1.0).custom_formatter(pct).text("Rondeur"));
        ui.label(egui::RichText::new("Au hasard").weak());
        ui.add(egui::Slider::new(&mut s.random_size, 0.0..=1.0).custom_formatter(pct).text("Taille"));
        ui.add(egui::Slider::new(&mut s.random_angle, 0.0..=360.0).suffix("°").text("Angle"));
        ui.add(egui::Slider::new(&mut s.random_spacing, 0.0..=1.0).custom_formatter(pct).text("Espacement"));
        ui.add(egui::Slider::new(&mut s.opacity, 0.0..=1.0).custom_formatter(pct).text("Opacité"));
        ui.horizontal(|ui| {
            ui.checkbox(&mut s.flip_x, "Miroir ↔");
            ui.checkbox(&mut s.flip_y, "Miroir ↕");
        });
        ui.checkbox(&mut s.tint, "Teinter les images en couleurs").on_hover_text(
            "Les tampons blancs de Pelure prennent toujours la couleur courante ; une image importée garde \
             ses couleurs, sauf avec cette case.",
        );
        if self.stamps.selected.len() > 1 {
            ui.horizontal(|ui| {
                ui.label("Cycle");
                ui.selectable_value(&mut s.cycle, Cycle::InOrder, "Dans l'ordre");
                ui.selectable_value(&mut s.cycle, Cycle::Random, "Au hasard");
            });
            ui.checkbox(&mut s.reset_cycle, "Reprendre le cycle à chaque trait");
        }
        ui.label(
            egui::RichText::new("Sur un calque bitmap. Ctrl + clic sur un tampon : plusieurs images en cycle.")
                .small()
                .weak(),
        );
    }

    /// Réglages du tampon de duplication (panneau Réglages).
    pub(crate) fn clone_options_ui(&mut self, ui: &mut egui::Ui) {
        let c = &mut self.clone;
        let pct = |v: f64, _: std::ops::RangeInclusive<usize>| format!("{:.0} %", v * 100.0);
        ui.add(egui::Slider::new(&mut c.size, 1.0..=500.0).logarithmic(true).text("Taille"));
        ui.add(egui::Slider::new(&mut c.hardness, 0.0..=1.0).text("Dureté"));
        ui.add(egui::Slider::new(&mut c.opacity, 0.0..=1.0).custom_formatter(pct).text("Opacité"));
        ui.checkbox(&mut c.aligned, "Aligné").on_hover_text(
            "La source suit le pinceau d'un trait à l'autre ; sinon chaque trait repart de la source.",
        );
        ui.label("Copier :");
        ui.radio_value(&mut c.sample_all, false, "le calque courant");
        ui.radio_value(&mut c.sample_all, true, "tous les calques visibles");
        ui.label(
            egui::RichText::new(match c.source {
                Some(_) => "Peins : ce qui est sous le viseur orange est recopié sous le pinceau. Ctrl + clic : \
                            nouvelle source.",
                None => "Ctrl + clic sur le dessin : choisir la source, puis peindre ailleurs.",
            })
            .small()
            .weak(),
        );
    }
}
