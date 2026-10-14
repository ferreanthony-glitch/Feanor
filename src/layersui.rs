//! Panneau Calques (6c) : chaque calque avec la vignette de son dessin à l'image courante,
//! visibilité et verrou, opacité du calque courant en haut, commandes en icônes en bas, réglages
//! du calque courant dessous.

use std::collections::HashMap;

use eframe::egui::{self, vec2, Color32, RichText, Sense, Stroke, StrokeKind, TextureHandle};

use crate::app::App;
use crate::document::{CelId, LayerKind};
use crate::render::over;

/// Taille des vignettes de calque.
const THUMB: [usize; 2] = [64, 36];
/// Marque des calques de toute la scène (6f).
pub(crate) const SCENE_WIDE: Color32 = Color32::from_rgb(214, 170, 92);

/// Vignettes des dessins (par dessin et version), refaites quand le dessin change.
#[derive(Default)]
pub(crate) struct LayerThumbs {
    cache: HashMap<CelId, (u64, TextureHandle)>,
}

impl App {
    /// Vignette du dessin `cel` : la partie filmée (le cadre), sur fond blanc.
    fn layer_thumb(&mut self, ctx: &egui::Context, cel: CelId) -> Option<TextureHandle> {
        let c = self.doc.cels.get(&cel)?;
        if let Some((v, t)) = self.layer_thumbs.cache.get(&cel) {
            if *v == c.version {
                return Some(t.clone());
            }
        }
        let fr = self.doc.frame_rect();
        let [w, h] = THUMB;
        let h = ((w as f32 * fr.height() / fr.width().max(1.0)).round() as usize).clamp(8, h * 2);
        let mut pixels = Vec::with_capacity(w * h);
        // Quatre échantillons par point de la vignette.
        for y in 0..h {
            for x in 0..w {
                let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
                for (dx, dy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                    let sx = fr.min.x + (x as f32 + dx) / w as f32 * fr.width();
                    let sy = fr.min.y + (y as f32 + dy) / h as f32 * fr.height();
                    let (sx, sy) = (sx as usize, sy as usize);
                    let p = if sx < c.raster.width && sy < c.raster.height { c.raster.get(sx, sy) } else { Color32::TRANSPARENT };
                    let q = over(Color32::WHITE, p, 255, None);
                    r += q.r() as u32;
                    g += q.g() as u32;
                    b += q.b() as u32;
                }
                pixels.push(Color32::from_rgb((r / 4) as u8, (g / 4) as u8, (b / 4) as u8));
            }
        }
        let image = egui::ColorImage { size: [w, h], pixels };
        let tex = ctx.load_texture(format!("calque {cel}"), image, egui::TextureOptions::LINEAR);
        if self.layer_thumbs.cache.len() > 512 {
            self.layer_thumbs.cache.clear();
        }
        self.layer_thumbs.cache.insert(cel, (c.version, tex.clone()));
        Some(tex)
    }

    /// Calques de toute la scène accordés d'un panneau à l'autre (6f), à chaque image ; le
    /// calque courant reste le même.
    pub(crate) fn sync_scene_layers(&mut self) {
        let id = self.layers().get(self.cur_layer).map(|l| l.id);
        if self.doc.sync_scene_layers(self.cur_panel, self.cur_frame) {
            if let Some(i) = id.and_then(|id| self.layers().iter().position(|l| l.id == id)) {
                self.cur_layer = i;
            }
            self.clamp_nav();
            self.renderer.invalidate_all();
        }
    }

    /// Nouveau calque bitmap « Fond » tout en bas, commun à tous les panneaux de la scène.
    pub(crate) fn add_scene_background(&mut self) {
        self.end_stroke();
        self.drop_bitmap_selection();
        let snap = self.snapshot();
        self.layers_mut().insert(0, crate::document::Layer::new("Fond", LayerKind::Bitmap));
        self.doc.set_scene_wide(self.cur_panel, 0, self.cur_frame, true);
        self.cur_layer = 0;
        self.vsel = None;
        self.commit_structure(snap);
        self.status = "Calque « Fond » : ce qu'on y dessine se voit dans tous les panneaux de la scène.".into();
    }

    /// Étend le calque courant à toute la scène, ou le rend à ce seul panneau (les autres panneaux
    /// gardent alors leur copie).
    pub(crate) fn toggle_scene_wide(&mut self) {
        self.end_stroke();
        self.drop_bitmap_selection();
        let snap = self.snapshot();
        let on = self.layer().scene_wide.is_none();
        self.doc.set_scene_wide(self.cur_panel, self.cur_layer, self.cur_frame, on);
        self.commit_structure(snap);
        let name = self.layer().name.clone();
        self.status = if on {
            format!("Calque « {name} » étendu à toute la scène : un seul dessin, dans chaque panneau.")
        } else {
            format!("Calque « {name} » rendu à ce panneau ; les autres panneaux gardent leur copie.")
        };
    }

    fn layer_hint(&self, text: &str, action: &str) -> String {
        let key = self.keymap.label(action);
        if key.is_empty() {
            text.to_string()
        } else {
            format!("{text} ({key})")
        }
    }

    pub(crate) fn layers_panel(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        // Opacité du calque courant (comme en haut d'une liste de calques).
        let li = self.cur_layer;
        let mut opacity = self.layers()[li].opacity * 100.0;
        ui.horizontal(|ui| {
            ui.label("Opacité");
            let resp = ui.add(egui::DragValue::new(&mut opacity).range(0.0..=100.0).max_decimals(0).suffix(" %").speed(0.5));
            if self.edited(&resp) {
                self.layers_mut()[li].opacity = opacity / 100.0;
            }
        });
        ui.add_space(2.0);

        let mut select = None;
        let mut toggle = None;
        let mut lock = None;
        let lock_hint = self.layer_hint("Verrouiller / déverrouiller : on ne peut plus dessiner sur le calque", "calque.verrouiller");
        let frame = self.cur_frame;
        let accent = ui.visuals().selection.bg_fill;
        for li in (0..self.layers().len()).rev() {
            let (visible, locked, kind, name, cel, wide) = {
                let l = &self.layers()[li];
                (l.visible, l.locked, l.kind, l.name.clone(), l.cel_at(frame), l.scene_wide.is_some())
            };
            let current = li == self.cur_layer;
            let thumb = cel.and_then(|c| self.layer_thumb(ui.ctx(), c));
            egui::Frame::new()
                .fill(if current { accent.gamma_multiply(0.55) } else { Color32::TRANSPARENT })
                .corner_radius(3.0)
                .inner_margin(egui::Margin::symmetric(3, 2))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = vec2(3.0, 0.0);
                        let eye = if visible { ph::EYE } else { ph::EYE_SLASH };
                        if ui
                            .add(egui::Button::new(RichText::new(eye).size(15.0)).frame(false).min_size(vec2(20.0, 36.0)))
                            .on_hover_text("Afficher / masquer")
                            .clicked()
                        {
                            toggle = Some(li);
                        }
                        let pad = if locked { ph::LOCK_SIMPLE } else { ph::LOCK_SIMPLE_OPEN };
                        let pad = RichText::new(pad).size(14.0);
                        let pad = if locked { pad } else { pad.weak() };
                        if ui.add(egui::Button::new(pad).frame(false).min_size(vec2(18.0, 36.0))).on_hover_text(&lock_hint).clicked() {
                            lock = Some(li);
                        }
                        // Vignette du dessin à l'image courante (vide : cadre gris).
                        let (rect, resp) = ui.allocate_exact_size(vec2(THUMB[0] as f32, THUMB[1] as f32), Sense::click());
                        match &thumb {
                            Some(t) => {
                                ui.painter().image(
                                    t.id(),
                                    rect,
                                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                    if visible { Color32::WHITE } else { Color32::from_gray(120) },
                                );
                            }
                            None => {
                                ui.painter().rect_filled(rect, 0.0, Color32::from_gray(60));
                            }
                        }
                        let border = if current { Stroke::new(2.0_f32, Color32::WHITE) } else { Stroke::new(1.0_f32, Color32::from_gray(80)) };
                        ui.painter().rect_stroke(rect, 0.0, border, StrokeKind::Inside);
                        if resp.clicked() {
                            select = Some(li);
                        }
                        ui.vertical(|ui| {
                            let text = if visible { RichText::new(&name) } else { RichText::new(&name).weak() };
                            if ui.add(egui::Label::new(text).sense(Sense::click()).truncate()).clicked() {
                                select = Some(li);
                            }
                            let kind = match kind {
                                LayerKind::Vector => "vectoriel",
                                LayerKind::Bitmap => "bitmap",
                            };
                            if wide {
                                ui.label(RichText::new(format!("{kind} · toute la scène")).small().color(SCENE_WIDE))
                                    .on_hover_text("Le même dessin dans tous les panneaux de la scène");
                            } else {
                                ui.label(RichText::new(kind).small().weak());
                            }
                        });
                    });
                });
        }
        if let Some(li) = toggle {
            self.record_step();
            let layer = &mut self.layers_mut()[li];
            layer.visible = !layer.visible;
        }
        if let Some(li) = lock {
            self.toggle_layer_lock(li);
        }
        if let Some(li) = select {
            self.select_layer(li);
        }

        // Commandes en icônes.
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
            let icon = |s: &str| RichText::new(s).size(16.0);
            if ui.button(RichText::new(format!("{} V", ph::PLUS))).on_hover_text("Nouveau calque vectoriel (traits modifiables)").clicked() {
                self.add_layer(LayerKind::Vector);
            }
            if ui.button(RichText::new(format!("{} B", ph::PLUS))).on_hover_text("Nouveau calque bitmap (peinture en pixels)").clicked() {
                self.add_layer(LayerKind::Bitmap);
            }
            let fond_hint = self.layer_hint(
                "Nouveau calque de fond, tout en bas, commun à tous les panneaux de la scène (décor)",
                "calque.nouveau_fond",
            );
            if ui.button(RichText::new(format!("{} Fond", ph::PLUS))).on_hover_text(fond_hint).clicked() {
                self.add_scene_background();
            }
            if ui.button(icon(ph::COPY_SIMPLE)).on_hover_text("Dupliquer le calque").clicked() {
                self.duplicate_layer();
            }
            if ui.button(icon(ph::TRASH)).on_hover_text("Supprimer le calque").clicked() {
                self.delete_layer();
            }
            if ui.button(icon(ph::ARROW_UP)).on_hover_text("Monter le calque").clicked() {
                self.move_layer(true);
            }
            if ui.button(icon(ph::ARROW_DOWN)).on_hover_text("Descendre le calque").clicked() {
                self.move_layer(false);
            }
            let copy_hint = self.layer_hint("Copier le calque", "calque.copier");
            if ui.button(icon(ph::COPY)).on_hover_text(copy_hint).clicked() {
                self.copy_layer();
            }
            let paste_hint = self.layer_hint("Coller le calque copié au-dessus du calque courant", "calque.coller");
            if ui.add_enabled(self.layer_clipboard.is_some(), egui::Button::new(icon(ph::CLIPBOARD))).on_hover_text(paste_hint).clicked() {
                self.paste_layer();
            }
            let merge_hint = self.layer_hint("Fusionner avec le calque du dessous", "calque.fusionner_dessous");
            if ui.add_enabled(self.cur_layer > 0, egui::Button::new(icon(ph::ARROWS_IN_SIMPLE))).on_hover_text(merge_hint).clicked() {
                self.merge_layer_down();
            }
        });

        // Réglages du calque courant : on modifie des copies, `edited` enregistre l'annulation.
        ui.separator();
        let li = self.cur_layer;
        let layer = &self.layers()[li];
        let (mut name, mut visible, mut depth) = (layer.name.clone(), layer.visible, layer.depth);
        ui.horizontal(|ui| {
            ui.label("Nom");
            let resp = ui.text_edit_singleline(&mut name);
            if self.edited(&resp) {
                self.layers_mut()[li].name = name;
            }
        });
        let resp = ui.checkbox(&mut visible, "Visible");
        if self.edited(&resp) {
            self.layers_mut()[li].visible = visible;
        }
        let mut wide = self.layers()[li].scene_wide.is_some();
        if ui
            .checkbox(&mut wide, "Toute la scène")
            .on_hover_text(
                "Un seul dessin, montré dans chaque panneau de la scène (un décor, par exemple). \
                 Décoché : chaque panneau garde sa propre copie.",
            )
            .changed()
        {
            self.toggle_scene_wide();
        }
        let resp = ui
            .horizontal(|ui| {
                ui.label("Profondeur");
                ui.add(egui::DragValue::new(&mut depth).speed(0.01).range(crate::document::Layer::MIN_DEPTH..=1000.0).max_decimals(2))
            })
            .inner
            .on_hover_text(
                "Distance du calque quand la caméra se déplace (multiplan) :\n\
                 0 = plan de référence, défile avec la caméra ;\n\
                 1 = deux fois plus loin, défile deux fois moins vite ;\n\
                 négatif = premier plan, défile plus vite.\n\
                 Le zoom de la caméra n'en tient pas compte.",
            );
        if self.edited(&resp) {
            self.layers_mut()[li].depth = depth;
        }
    }
}
