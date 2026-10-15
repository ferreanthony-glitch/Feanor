//! Panneau Couleur (6b) : grande pastille de la couleur courante, curseurs teinte / saturation /
//! valeur (TSV) avec leur dégradé, nuancier avec + et −.

use eframe::egui::{self, pos2, vec2, Color32, Rect, Sense, Shape, Stroke, StrokeKind};

use crate::app::App;

/// Teinte (0 à 360), saturation et valeur (0 à 1) d'une couleur.
pub fn rgb_to_hsv(c: [u8; 3]) -> [f32; 3] {
    let [r, g, b] = c.map(|v| v as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d <= 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    [h, if max <= 0.0 { 0.0 } else { d / max }, max]
}

pub fn hsv_to_rgb(hsv: [f32; 3]) -> [u8; 3] {
    let h = hsv[0].rem_euclid(360.0) / 60.0;
    let (s, v) = (hsv[1].clamp(0.0, 1.0), hsv[2].clamp(0.0, 1.0));
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [r, g, b].map(|t| ((t + m) * 255.0).round().clamp(0.0, 255.0) as u8)
}

fn to_color(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Barre dégradée que l'on clique ou glisse ; `colors` : couleurs le long de la barre (au moins
/// deux). Renvoie la nouvelle position (0 à 1) si elle a changé.
fn gradient_bar(ui: &mut egui::Ui, value: f32, colors: &[Color32]) -> Option<f32> {
    let width = (ui.available_width() - 64.0).max(60.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 16.0), Sense::click_and_drag());
    let painter = ui.painter_at(rect.expand(4.0));
    let mut mesh = egui::Mesh::default();
    let n = colors.len() - 1;
    for (i, c) in colors.iter().enumerate() {
        let x = rect.left() + rect.width() * i as f32 / n as f32;
        mesh.colored_vertex(pos2(x, rect.top()), *c);
        mesh.colored_vertex(pos2(x, rect.bottom()), *c);
        if i > 0 {
            let k = (i * 2) as u32;
            mesh.add_triangle(k - 2, k - 1, k);
            mesh.add_triangle(k - 1, k + 1, k);
        }
    }
    painter.add(Shape::mesh(mesh));
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0_f32, Color32::from_gray(80)), StrokeKind::Outside);
    let x = rect.left() + rect.width() * value.clamp(0.0, 1.0);
    let mark = Rect::from_center_size(pos2(x, rect.center().y), vec2(5.0, rect.height() + 6.0));
    painter.rect(mark, 1.0, Color32::from_black_alpha(80), Stroke::new(1.5_f32, Color32::WHITE), StrokeKind::Middle);
    let p = resp.interact_pointer_pos()?;
    (resp.dragged() || resp.clicked()).then(|| ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0))
}

impl App {
    /// Panneau Couleur.
    pub(crate) fn color_panel(&mut self, ui: &mut egui::Ui) {
        // TSV gardé à part : la teinte d'un gris ou d'un noir ne se perd pas.
        if hsv_to_rgb(self.hsv) != self.color {
            self.hsv = rgb_to_hsv(self.color);
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().interact_size = vec2(70.0, 30.0);
            if ui.color_edit_button_srgb(&mut self.color).changed() {
                self.hsv = rgb_to_hsv(self.color);
            }
            let [r, g, b] = self.color;
            let mut hex = format!("#{r:02X}{g:02X}{b:02X}");
            let resp = ui.add(egui::TextEdit::singleline(&mut hex).desired_width(72.0).font(egui::TextStyle::Monospace));
            if resp.changed() {
                let digits = hex.trim().trim_start_matches('#');
                if digits.len() == 6 {
                    if let Ok(v) = u32::from_str_radix(digits, 16) {
                        self.color = [(v >> 16) as u8, (v >> 8) as u8, v as u8];
                        self.hsv = rgb_to_hsv(self.color);
                    }
                }
            }
        });
        ui.add_space(4.0);
        let [h, s, v] = self.hsv;
        let mut next = self.hsv;
        let hues: Vec<Color32> = (0..=12).map(|k| to_color(hsv_to_rgb([k as f32 * 30.0, 1.0, 1.0]))).collect();
        let sats = [to_color(hsv_to_rgb([h, 0.0, v])), to_color(hsv_to_rgb([h, 1.0, v]))];
        let vals = [Color32::BLACK, to_color(hsv_to_rgb([h, s, 1.0]))];
        let rows: [(&str, f32, f32, &[Color32]); 3] =
            [("T", h / 360.0, 360.0, &hues), ("S", s, 100.0, &sats), ("V", v, 100.0, &vals)];
        for (k, (label, value, scale, colors)) in rows.into_iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(label).monospace()).on_hover_text(match k {
                    0 => "Teinte",
                    1 => "Saturation",
                    _ => "Valeur (clarté)",
                });
                if let Some(t) = gradient_bar(ui, value, colors) {
                    next[k] = t * if k == 0 { 360.0 } else { 1.0 };
                }
                let mut shown = value * scale;
                if ui.add(egui::DragValue::new(&mut shown).range(0.0..=scale).max_decimals(0).speed(0.5)).changed() {
                    next[k] = if k == 0 { shown } else { shown / scale };
                }
            });
        }
        if next != self.hsv {
            self.hsv = next;
            self.color = hsv_to_rgb(next);
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Nuancier");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let here = self.swatches.iter().position(|c| *c == self.color);
                use egui_phosphor::regular as ph;
                if crate::bulles::icone_si(ui, here.is_some(), ph::MINUS, "Retirer du nuancier", "Retirer la couleur courante du nuancier.", "").clicked() {
                    if let Some(i) = here {
                        self.swatches.remove(i);
                    }
                }
                if crate::bulles::icone_si(ui, here.is_none(), ph::PLUS, "Ajouter au nuancier", "Ajouter la couleur courante au nuancier.", "").clicked() {
                    self.swatches.push(self.color);
                }
            });
        });
        let mut remove = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
            for (i, c) in self.swatches.iter().enumerate() {
                let (rect, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                ui.painter().rect_filled(rect, 2.0, to_color(*c));
                let border = if *c == self.color {
                    Stroke::new(2.0_f32, Color32::WHITE)
                } else {
                    Stroke::new(1.0_f32, Color32::from_gray(70))
                };
                ui.painter().rect_stroke(rect, 2.0, border, StrokeKind::Inside);
                let resp = resp.on_hover_text("Clic : choisir · clic droit : retirer");
                if resp.clicked() {
                    self.color = *c;
                }
                if resp.secondary_clicked() {
                    remove = Some(i);
                }
            }
        });
        if let Some(i) = remove {
            self.swatches.remove(i);
        }
    }
}
