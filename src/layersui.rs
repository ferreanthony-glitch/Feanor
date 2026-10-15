//! Panneau Calques (6c) : chaque calque avec la vignette de son dessin à l'image courante,
//! visibilité et verrou, opacité du calque courant en haut, commandes en icônes en bas, réglages
//! du calque courant dessous.

use std::collections::HashMap;

use eframe::egui::{self, vec2, Color32, RichText, Sense, Stroke, StrokeKind, TextureHandle};

use crate::app::App;
use crate::blend::BlendMode;
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

    pub(crate) fn layers_panel(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        // Opacité du calque courant (comme en haut d'une liste de calques).
        let li = self.cur_layer;
        let mut opacity = self.layers()[li].opacity * 100.0;
        let mut mode = self.layers()[li].blend;
        ui.horizontal(|ui| {
            // Mode de fusion (8), comme dans les logiciels de retouche.
            let resp = egui::ComboBox::from_id_salt("fusion_calque")
                .selected_text(mode.label())
                .width(120.0)
                .show_ui(ui, |ui| {
                    for m in BlendMode::ALL {
                        // Familles : normal, assombrir, éclaircir, contraste, écarts, couleur.
                        if BlendMode::FAMILY_STARTS.contains(&m) {
                            ui.separator();
                        }
                        ui.selectable_value(&mut mode, m, m.label()).on_hover_text(m.hint());
                    }
                })
                .response;
            crate::bulles::bulle(resp, "Mode de fusion", mode.hint(), "");
            crate::bulles::etiquette(ui, ph::CIRCLE_HALF, "Opacité du calque", "");
            let resp = ui.add(egui::DragValue::new(&mut opacity).range(0.0..=100.0).max_decimals(0).suffix(" %").speed(0.5));
            let resp = crate::bulles::bulle(resp, "Opacité du calque", "Glisser ou taper une valeur.", "");
            if self.edited(&resp) {
                self.layers_mut()[li].opacity = opacity / 100.0;
            }
        });
        if mode != self.layers()[li].blend {
            self.record_step();
            self.layers_mut()[li].blend = mode;
            self.renderer.invalidate_all();
        }
        ui.add_space(2.0);

        // La liste : un rang par calque, le plus haut en premier. Clic droit : renommer, toute la
        // scène, profondeur, dupliquer, verrouiller, supprimer (9b) ; double-clic sur le nom :
        // renommer.
        let mut select = None;
        let mut toggle = None;
        let mut lock = None;
        let mut rename_done: Option<bool> = None;
        let lock_key = self.raccourci("calque.verrouiller");
        let frame = self.cur_frame;
        let accent = ui.visuals().selection.bg_fill;
        for li in (0..self.layers().len()).rev() {
            let (id, visible, locked, kind, name, cel, wide) = {
                let l = &self.layers()[li];
                (l.id, l.visible, l.locked, l.kind, l.name.clone(), l.cel_at(frame), l.scene_wide.is_some())
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
                        let resp = ui.add(egui::Button::new(RichText::new(eye).size(15.0)).frame(false).min_size(vec2(20.0, 36.0)));
                        if crate::bulles::bulle(resp, "Afficher / masquer", "Montrer ou cacher le calque.", "").clicked() {
                            toggle = Some(li);
                        }
                        let pad = if locked { ph::LOCK_SIMPLE } else { ph::LOCK_SIMPLE_OPEN };
                        let pad = RichText::new(pad).size(14.0);
                        let pad = if locked { pad } else { pad.weak() };
                        let resp = ui.add(egui::Button::new(pad).frame(false).min_size(vec2(18.0, 36.0)));
                        if crate::bulles::bulle(resp, "Verrouiller / déverrouiller", "Un calque verrouillé ne peut plus être dessiné.", &lock_key).clicked() {
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
                        if resp.clicked() || resp.secondary_clicked() {
                            select = Some(li);
                        }
                        resp.context_menu(|ui| self.layer_menu(ui, li));
                        ui.vertical(|ui| {
                            let renaming = self.layer_rename.as_ref().is_some_and(|r| r.0 == id);
                            if renaming {
                                let (_, buffer, focused) = self.layer_rename.as_mut().expect("renommage en cours");
                                let mut out = egui::TextEdit::singleline(buffer).desired_width(120.0).show(ui);
                                let resp = out.response.clone();
                                if !*focused {
                                    // Tout le nom choisi : on tape le nouveau directement.
                                    resp.request_focus();
                                    let n = buffer.chars().count();
                                    out.state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                                        egui::text::CCursor::new(0),
                                        egui::text::CCursor::new(n),
                                    )));
                                    out.state.store(ui.ctx(), resp.id);
                                    *focused = true;
                                }
                                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                    rename_done = Some(false);
                                } else if resp.lost_focus() {
                                    rename_done = Some(true);
                                }
                            } else {
                                let text = if visible { RichText::new(&name) } else { RichText::new(&name).weak() };
                                let resp = ui.add(egui::Label::new(text).sense(Sense::click()).truncate());
                                if resp.clicked() || resp.secondary_clicked() {
                                    select = Some(li);
                                }
                                if resp.double_clicked() {
                                    self.layer_rename = Some((id, name.clone(), false));
                                }
                                resp.on_hover_text("Double-clic ou clic droit : renommer").context_menu(|ui| self.layer_menu(ui, li));
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
        match rename_done {
            Some(true) => self.commit_layer_rename(),
            Some(false) => self.layer_rename = None,
            None => {}
        }

        // Commandes en icônes.
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
            if self.bouton(ui, ph::BEZIER_CURVE, "Nouveau calque vectoriel", "Des traits qui restent modifiables.", "calque.nouveau_vectoriel").clicked() {
                self.add_layer(LayerKind::Vector);
            }
            if self.bouton(ui, ph::PAINT_BRUSH_BROAD, "Nouveau calque bitmap", "De la peinture en pixels, pinceaux texturés.", "calque.nouveau_bitmap").clicked() {
                self.add_layer(LayerKind::Bitmap);
            }
            if self
                .bouton(ui, ph::MOUNTAINS, "Nouveau calque de fond", "Tout en bas, commun à tous les panneaux de la scène (décor).", "calque.nouveau_fond")
                .clicked()
            {
                self.add_scene_background();
            }
            if self.bouton(ui, ph::COPY_SIMPLE, "Dupliquer le calque", "Une copie du calque courant et de ses dessins.", "").clicked() {
                self.duplicate_layer();
            }
            if self.bouton(ui, ph::TRASH, "Supprimer le calque", "", "calque.supprimer").clicked() {
                self.delete_layer();
            }
            if self.bouton(ui, ph::ARROW_UP, "Monter le calque", "D'un rang vers le haut.", "").clicked() {
                self.move_layer(true);
            }
            if self.bouton(ui, ph::ARROW_DOWN, "Descendre le calque", "D'un rang vers le bas.", "").clicked() {
                self.move_layer(false);
            }
            if self.bouton(ui, ph::COPY, "Copier le calque", "", "calque.copier").clicked() {
                self.copy_layer();
            }
            let can_paste = self.layer_clipboard.is_some();
            if self.bouton_si(ui, can_paste, ph::CLIPBOARD, "Coller le calque", "Le calque copié, au-dessus du calque courant.", "calque.coller").clicked() {
                self.paste_layer();
            }
            let can_merge = self.cur_layer > 0;
            if self
                .bouton_si(ui, can_merge, ph::ARROWS_IN_SIMPLE, "Fusionner avec le calque du dessous", "Les deux calques n'en font plus qu'un.", "calque.fusionner_dessous")
                .clicked()
            {
                self.merge_layer_down();
            }
        });
    }
    /// Pose le nouveau nom du calque en cours de renommage (une étape d'annulation).
    fn commit_layer_rename(&mut self) {
        let Some((id, name, _)) = self.layer_rename.take() else { return };
        let name = name.trim().to_string();
        let Some(li) = self.layers().iter().position(|l| l.id == id) else { return };
        if !name.is_empty() && self.layers()[li].name != name {
            self.record_step();
            self.layers_mut()[li].name = name;
        }
    }

    /// Menu d'un calque (clic droit) : renommer, toute la scène, profondeur, dupliquer,
    /// verrouiller, supprimer.
    fn layer_menu(&mut self, ui: &mut egui::Ui, li: usize) {
        let Some(l) = self.layers().get(li) else {
            ui.close_menu();
            return;
        };
        let (id, name, mut wide, mut depth, locked) = (l.id, l.name.clone(), l.scene_wide.is_some(), l.depth, l.locked);
        if ui.button("Renommer…").clicked() {
            ui.close_menu();
            self.layer_rename = Some((id, name, false));
        }
        if ui
            .checkbox(&mut wide, "Toute la scène")
            .on_hover_text(
                "Un seul dessin, montré dans chaque panneau de la scène (un décor, par exemple). \
                 Décoché : chaque panneau garde sa propre copie.",
            )
            .changed()
        {
            self.select_layer(li);
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
                 négatif = premier plan, défile plus vite.",
            );
        if self.edited(&resp) {
            self.layers_mut()[li].depth = depth;
        }
        ui.separator();
        if ui.button("Dupliquer").clicked() {
            ui.close_menu();
            self.select_layer(li);
            self.duplicate_layer();
        }
        if ui.button(if locked { "Déverrouiller" } else { "Verrouiller" }).clicked() {
            ui.close_menu();
            self.toggle_layer_lock(li);
        }
        if ui.button("Supprimer").clicked() {
            ui.close_menu();
            self.select_layer(li);
            self.delete_layer();
        }
    }
}
