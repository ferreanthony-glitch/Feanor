//! Outil Texte (5d) : un clic pose un texte (ou rouvre un texte existant d'un calque vectoriel),
//! on le tape dans une petite fenêtre sous lui, l'aperçu s'affiche sur le dessin dans sa police.
//! Ctrl+Entrée (ou un clic ailleurs, ou un autre outil) le pose ; Échap annule. Sur un calque
//! vectoriel, le texte reste un élément modifiable ; sur un calque bitmap, il devient des pixels.

use eframe::egui::{self, pos2, Color32, Key, Pos2, Rect, Shape, Stroke, TextureHandle};

use crate::app::App;
use crate::document::{CelId, LayerKind};
use crate::raster::Raster;
use crate::vector::{self, Affine, Element, VectorDrawing};

/// Ce dont dépend l'aperçu.
type PreviewKey = (String, String, u32, [u8; 3], [u32; 6]);

/// Texte en cours de saisie.
pub(crate) struct TextEditState {
    /// Texte existant que l'on modifie : son dessin, son rang, et le dessin d'avant (le texte en
    /// est retiré pendant la saisie).
    existing: Option<(CelId, usize, VectorDrawing)>,
    pub text: String,
    pub font: String,
    pub size: f32,
    pub color: [u8; 3],
    place: Affine,
    preview: Option<(PreviewKey, TextureHandle, Rect)>,
    /// Donner le clavier à la saisie (à l'ouverture).
    focus: bool,
}

impl TextEditState {
    fn element(&self) -> Element {
        Element::text(self.text.clone(), self.font.clone(), self.size, self.color, self.place)
    }

    fn key(&self) -> PreviewKey {
        (
            self.text.clone(),
            self.font.clone(),
            self.size.to_bits(),
            self.color,
            self.place.to_array().map(f32::to_bits),
        )
    }
}

impl App {
    pub(crate) fn text_press(&mut self, p: Pos2) {
        self.playing = false;
        // Un clic hors de la saisie pose le texte en cours.
        if self.text_edit.is_some() {
            self.finish_text_edit();
            return;
        }
        self.end_stroke();
        if !self.layer_editable() {
            return;
        }
        // Un texte sous le clic (calque vectoriel) : on le rouvre.
        if self.layer().kind == LayerKind::Vector {
            if let Some(cel) = self.cel_here() {
                let tol = 4.0 / self.layer_zoom().max(1e-3);
                let found = self.drawing(cel).and_then(|d| {
                    let i = d.hit_test(p, tol)?;
                    matches!(d.elements[i], Element::Text { .. }).then(|| (i, d.clone()))
                });
                if let Some((i, before)) = found {
                    let Element::Text { text, font, size, color, place, .. } = before.elements[i].clone() else { return };
                    if let Some(d) = self.drawing_mut(cel) {
                        d.elements.remove(i);
                    }
                    self.vsel = None;
                    self.refresh_cel(cel);
                    self.text_edit = Some(TextEditState {
                        existing: Some((cel, i, before)),
                        text,
                        font: font.clone(),
                        size,
                        color,
                        place: Affine::from_array(place),
                        preview: None,
                        focus: true,
                    });
                    self.text_font = font;
                    self.text_size = size;
                    // Sa couleur devient la couleur courante (le panneau Couleur la règle).
                    self.color = color;
                    return;
                }
            }
        }
        self.text_edit = Some(TextEditState {
            existing: None,
            text: String::new(),
            font: self.text_font.clone(),
            size: self.text_size,
            color: self.color,
            place: Affine::translate(p.to_vec2()),
            preview: None,
            focus: true,
        });
    }

    /// Pose le texte en cours (rien s'il est vide).
    pub(crate) fn finish_text_edit(&mut self) {
        let Some(st) = self.text_edit.take() else { return };
        let empty = st.text.trim().is_empty();
        if !empty {
            // Couleurs récentes (passe couleur).
            self.color_used(st.color);
        }
        match st.existing {
            Some((cel, i, before)) => {
                if !empty {
                    let element = Element::text(st.text.clone(), st.font.clone(), st.size, st.color, st.place);
                    if let Some(d) = self.drawing_mut(cel) {
                        let at = i.min(d.elements.len());
                        d.elements.insert(at, element);
                    }
                }
                self.refresh_cel(cel);
                if self.drawing(cel).is_some_and(|d| *d != before) {
                    self.push_vector(cel, before, None);
                }
            }
            None if !empty => {
                let element = st.element();
                self.commit_element(element);
            }
            None => {}
        }
    }

    /// Annule la saisie : un texte existant revient tel qu'il était.
    pub(crate) fn cancel_text_edit(&mut self) {
        let Some(st) = self.text_edit.take() else { return };
        if let Some((cel, _, before)) = st.existing {
            if let Some(d) = self.drawing_mut(cel) {
                *d = before;
            }
            self.refresh_cel(cel);
        }
        self.status = "Texte annulé.".into();
    }

    /// Aperçu du texte en cours dans sa police : refait quand le texte ou ses réglages changent.
    pub(crate) fn update_text_preview(&mut self, ctx: &egui::Context) {
        let (w, h) = (self.doc.width, self.doc.height);
        let Some(st) = &mut self.text_edit else { return };
        let key = st.key();
        if st.preview.as_ref().is_some_and(|(k, _, _)| *k == key) {
            return;
        }
        let element = st.element();
        let bounds = element.bounds().expand(2.0).intersect(Rect::from_min_size(Pos2::ZERO, egui::vec2(w as f32, h as f32)));
        if !bounds.is_positive() || st.text.is_empty() {
            st.preview = None;
            return;
        }
        let r = Rect::from_min_max(bounds.min.floor(), bounds.max.ceil());
        let mut raster = Raster::new(w, h);
        vector::render_element_clipped(&mut raster, &element, r);
        let (x0, y0) = (r.min.x as usize, r.min.y as usize);
        let (iw, ih) = ((r.width() as usize).max(1), (r.height() as usize).max(1));
        let mut pixels = Vec::with_capacity(iw * ih);
        for y in 0..ih {
            for x in 0..iw {
                pixels.push(raster.get((x0 + x).min(w - 1), (y0 + y).min(h - 1)));
            }
        }
        let image = egui::ColorImage { size: [iw, ih], pixels };
        let texture = ctx.load_texture("texte", image, egui::TextureOptions::LINEAR);
        st.preview = Some((key, texture, r));
    }

    /// L'aperçu et la boîte du texte en cours, sur le dessin.
    pub(crate) fn paint_text_overlay(&self, painter: &egui::Painter) {
        let Some(st) = &self.text_edit else { return };
        let to_s = |p: Pos2| self.layer_to_screen(p);
        if let Some((_, texture, r)) = &st.preview {
            let mut mesh = egui::Mesh::with_texture(texture.id());
            let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()];
            let uvs = [pos2(0.0, 0.0), pos2(1.0, 0.0), pos2(1.0, 1.0), pos2(0.0, 1.0)];
            for (p, uv) in corners.into_iter().zip(uvs) {
                mesh.vertices.push(egui::epaint::Vertex { pos: to_s(p), uv, color: Color32::WHITE });
            }
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            painter.add(Shape::mesh(mesh));
        }
        // Boîte du texte (au moins une ligne de haut, même vide).
        let laid = crate::text::layout(if st.text.is_empty() { " " } else { &st.text }, &st.font, st.size);
        let b = laid.bounds.union(Rect::from_min_size(Pos2::ZERO, egui::vec2(st.size * 0.3, st.size)));
        let quad: Vec<Pos2> = [b.left_top(), b.right_top(), b.right_bottom(), b.left_bottom(), b.left_top()]
            .iter()
            .map(|p| to_s(st.place.apply(*p)))
            .collect();
        painter.extend(Shape::dashed_line(&quad, Stroke::new(1.0_f32, Color32::from_rgb(70, 165, 255)), 4.0, 3.0));
    }

    /// Petite fenêtre de saisie sous le texte en cours.
    pub(crate) fn text_edit_window(&mut self, ctx: &egui::Context) {
        let Some(st) = &self.text_edit else { return };
        let laid = crate::text::layout(if st.text.is_empty() { " " } else { &st.text }, &st.font, st.size);
        let b = laid.bounds.union(Rect::from_min_size(Pos2::ZERO, egui::vec2(0.0, st.size)));
        let anchor = self.layer_to_screen(st.place.apply(b.left_bottom())) + egui::vec2(0.0, 8.0);
        let mut done = false;
        let mut cancel = false;
        let Some(st) = &mut self.text_edit else { return };
        egui::Area::new(egui::Id::new("saisie_texte"))
            .fixed_pos(anchor)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let resp = ui.add(
                        egui::TextEdit::multiline(&mut st.text)
                            .desired_rows(2)
                            .desired_width(280.0)
                            .hint_text("Texte…"),
                    );
                    if st.focus {
                        resp.request_focus();
                        st.focus = false;
                    }
                    let (enter_ctrl, escape) =
                        ui.input(|i| (i.key_pressed(Key::Enter) && i.modifiers.command, i.key_pressed(Key::Escape)));
                    if enter_ctrl {
                        // (Ctrl+Entrée a peut-être ajouté un retour à la ligne.)
                        if st.text.ends_with('\n') {
                            st.text.pop();
                        }
                        done = true;
                    }
                    cancel |= escape;
                    ui.horizontal(|ui| {
                        use egui_phosphor::regular as ph;
                        done |= crate::bulles::icone(ui, ph::CHECK, "Poser le texte", "Ctrl + Entrée, ou un clic ailleurs.", "").clicked();
                        cancel |= crate::bulles::icone(ui, ph::X, "Annuler", "Annuler la saisie (Échap).", "").clicked();
                    });
                });
            });
        if cancel {
            self.cancel_text_edit();
        } else if done {
            self.finish_text_edit();
        }
    }

    /// Réglages de l'outil Texte (panneau Réglages).
    pub(crate) fn text_options_ui(&mut self, ui: &mut egui::Ui) {
        let fonts = self.text_fonts.get_or_insert_with(crate::text::font_names).clone();
        let mut chosen = None;
        egui::ComboBox::from_label("Police")
            .selected_text(&self.text_font)
            .width(170.0)
            .height(420.0)
            .show_ui(ui, |ui| {
                ui.add(egui::TextEdit::singleline(&mut self.text_font_filter).hint_text("Chercher…"));
                let filter = self.text_font_filter.to_lowercase();
                for name in fonts.iter().filter(|n| filter.is_empty() || n.to_lowercase().contains(&filter)) {
                    if ui.selectable_label(*name == self.text_font, name).clicked() {
                        chosen = Some(name.clone());
                    }
                }
            });
        if let Some(name) = chosen {
            self.text_font = name;
        }
        ui.add(
            egui::DragValue::new(&mut self.text_size)
                .speed(0.5)
                .range(4.0..=1000.0)
                .max_decimals(1)
                .prefix("Taille ")
                .suffix(" px"),
        );
        // Les réglages valent aussi pour le texte en cours.
        if let Some(st) = &mut self.text_edit {
            st.font = self.text_font.clone();
            st.size = self.text_size;
            st.color = self.color;
        }
        let used = crate::text::layout("A", &self.text_font, 12.0).font.clone();
        if used != self.text_font {
            ui.label(
                egui::RichText::new(format!("« {} » n'est pas sur ce poste : texte dessiné en {used}.", self.text_font))
                    .small()
                    .color(crate::theme::avertissement()),
            );
        }
        ui.label(
            egui::RichText::new(
                "Clic : nouveau texte (sur un texte d'un calque vectoriel : le modifier). Ctrl+Entrée : \
                 poser ; Échap : annuler. Sur un calque vectoriel, le texte reste modifiable (Sélection : \
                 déplacer, agrandir, tourner). Une police absente du poste est remplacée par celle de SHOT.",
            )
            .small()
            .weak(),
        );
    }
}
