//! Panneau Couleur (6b) : grande pastille de la couleur courante, curseurs teinte / saturation /
//! valeur (TSV) avec leur dégradé, nuancier avec + et −.
//!
//! Passe couleur (2026-10-16) : deux couleurs, avant-plan et arrière-plan, comme dans Photoshop
//! (deux carrés qui se chevauchent ; un clic sur celui de derrière les échange). Les outils
//! dessinent avec la couleur d'avant-plan, `App::color`. Les conversions sont dans `couleur.rs`.

use eframe::egui::{self, pos2, vec2, Color32, Id, Rect, Response, Sense, Shape, Stroke, StrokeKind, Vec2};
use egui_phosphor::regular as ph;

use crate::app::App;
pub use crate::couleur::{hsv_to_rgb, rgb_to_hsv};
use crate::couleur::{self, ColorSettings};
use crate::roue::{Wheel, WheelShape, Zone};

/// Ce que le panneau Couleur garde en plus de la couleur d'avant-plan (`App::color`, `App::hsv`).
pub(crate) struct ColorState {
    /// Ce qui est retenu dans `reglages.toml` ; `prefs.back` est la couleur d'arrière-plan.
    /// (`prefs.front` est recopié d'`App::color` avant chaque enregistrement.)
    pub prefs: ColorSettings,
    /// L'arrière-plan en teinte, saturation, valeur (la teinte d'un gris ne se perd pas à
    /// l'échange).
    pub back_hsv: [f32; 3],
    /// Code hexadécimal en cours de saisie : le champ qui l'a, et le texte tapé.
    hex_edit: Option<(Id, String)>,
}

impl ColorState {
    pub fn new(prefs: ColorSettings) -> ColorState {
        ColorState { back_hsv: rgb_to_hsv(prefs.back), prefs, hex_edit: None }
    }
}

pub(crate) fn to_color(c: [u8; 3]) -> Color32 {
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

/// Roue chromatique de `size` points de côté, sur la couleur `hsv` : un clic ou un glissé dans
/// l'anneau change la teinte, dedans la saturation et la valeur ; le geste garde la partie où il
/// a commencé (on peut sortir de l'anneau sans changer de partie). Dessinée à chaque image
/// d'après `hsv` : les repères suivent la couleur, d'où qu'elle change.
pub(crate) fn wheel_ui(ui: &mut egui::Ui, size: f32, hsv: &mut [f32; 3], shape: WheelShape) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());
    let wheel = Wheel::in_rect(rect);
    // La partie visée est retenue à l'appui, pour tout le geste.
    let zone_id = resp.id.with("partie");
    // Bouton gauche seulement : le droit ouvre le menu sans toucher à la couleur.
    let held = resp.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_down());
    if held || resp.clicked() {
        let zone = match ui.data(|d| d.get_temp::<Option<Zone>>(zone_id)) {
            Some(z) => z,
            None => {
                let z = resp.interact_pointer_pos().and_then(|p| wheel.zone(p));
                ui.data_mut(|d| d.insert_temp(zone_id, z));
                z
            }
        };
        if let (Some(zone), Some(p)) = (zone, resp.interact_pointer_pos()) {
            match zone {
                Zone::Ring => hsv[0] = wheel.hue_at(p),
                Zone::Inside => (hsv[1], hsv[2]) = wheel.sv_at(shape, hsv[0], p),
            }
        }
    }
    if !held {
        ui.data_mut(|d| d.remove::<Option<Zone>>(zone_id));
    }
    let [h, s, v] = *hsv;
    let painter = ui.painter_at(rect.expand(2.0));
    painter.add(Shape::mesh(wheel.ring_mesh(96)));
    painter.add(Shape::mesh(match shape {
        WheelShape::Square => wheel.square_mesh(h, 24),
        WheelShape::Triangle => wheel.triangle_mesh(h, 24),
    }));
    // Repères : un trait en travers de l'anneau, un rond dedans (noir et blanc : visibles sur
    // toute couleur).
    let seg = wheel.ring_segment(h);
    painter.line_segment(seg, Stroke::new(4.0_f32, Color32::BLACK));
    painter.line_segment(seg, Stroke::new(2.0_f32, Color32::WHITE));
    let p = wheel.sv_point(shape, h, s, v);
    painter.circle_stroke(p, 5.5, Stroke::new(3.0_f32, Color32::BLACK));
    painter.circle_stroke(p, 5.5, Stroke::new(1.5_f32, Color32::WHITE));
    resp
}

/// Un carré de couleur bordé (sombre dehors, clair dedans : il se voit sur tout fond).
fn color_square(painter: &egui::Painter, rect: Rect, c: [u8; 3]) {
    painter.rect_filled(rect, 1.0, to_color(c));
    painter.rect_stroke(rect, 1.0, Stroke::new(1.0_f32, Color32::from_gray(15)), StrokeKind::Outside);
    painter.rect_stroke(rect, 1.0, Stroke::new(1.0_f32, Color32::from_gray(200)), StrokeKind::Inside);
}

impl App {
    /// Commandes de la couleur (passe couleur) ; vrai si `action` en était une.
    pub(crate) fn color_action(&mut self, action: &str) -> bool {
        match action {
            "couleur.intervertir" => self.swap_colors(),
            "couleur.defaut" => self.default_colors(),
            "couleur.copier_code" => {
                let code = couleur::hex(self.color);
                self.ctx.copy_text(code.clone());
                self.status = format!("Code {code} copié.");
            }
            "couleur.roue" => self.couleurs.prefs.wheel_shown = !self.couleurs.prefs.wheel_shown,
            "couleur.roue_carre" => {
                self.couleurs.prefs.wheel = WheelShape::Square;
                self.couleurs.prefs.wheel_shown = true;
            }
            "couleur.roue_triangle" => {
                self.couleurs.prefs.wheel = WheelShape::Triangle;
                self.couleurs.prefs.wheel_shown = true;
            }
            _ => return false,
        }
        true
    }

    /// Échange les couleurs d'avant-plan et d'arrière-plan.
    pub(crate) fn swap_colors(&mut self) {
        std::mem::swap(&mut self.color, &mut self.couleurs.prefs.back);
        std::mem::swap(&mut self.hsv, &mut self.couleurs.back_hsv);
        // Une couleur changée ailleurs (pipette, texte…) : sa teinte suit.
        if hsv_to_rgb(self.hsv) != self.color {
            self.hsv = rgb_to_hsv(self.color);
        }
        if hsv_to_rgb(self.couleurs.back_hsv) != self.couleurs.prefs.back {
            self.couleurs.back_hsv = rgb_to_hsv(self.couleurs.prefs.back);
        }
    }

    /// Noir à l'avant-plan, blanc à l'arrière-plan.
    pub(crate) fn default_colors(&mut self) {
        self.set_front(couleur::NOIR);
        self.set_back(couleur::BLANC);
    }

    /// Nouvelle couleur d'avant-plan.
    pub(crate) fn set_front(&mut self, c: [u8; 3]) {
        self.color = c;
        if hsv_to_rgb(self.hsv) != c {
            self.hsv = rgb_to_hsv(c);
        }
    }

    /// Nouvelle couleur d'arrière-plan.
    pub(crate) fn set_back(&mut self, c: [u8; 3]) {
        self.couleurs.prefs.back = c;
        if hsv_to_rgb(self.couleurs.back_hsv) != c {
            self.couleurs.back_hsv = rgb_to_hsv(c);
        }
    }

    /// Les deux couleurs, comme dans Photoshop : le carré d'avant-plan devant, celui
    /// d'arrière-plan derrière, décalé. Un clic sur celui de derrière les échange ; clic droit :
    /// intervertir, couleurs par défaut, copier le code. `side` : côté d'un carré. Renvoie la
    /// réponse du carré d'avant-plan (à chacun de décider ce que fait son clic).
    pub(crate) fn front_back(&mut self, ui: &mut egui::Ui, side: f32) -> Response {
        let (rect, _) = ui.allocate_exact_size(vec2(side * 1.5, side * 1.5), Sense::hover());
        let front_rect = Rect::from_min_size(rect.min, vec2(side, side));
        let back_rect = Rect::from_min_size(rect.min + vec2(side * 0.5, side * 0.5), vec2(side, side));
        let id = ui.id().with("avant_arriere");
        // Le carré de devant est déclaré après : c'est lui qui reçoit le clic là où ils se
        // chevauchent.
        let back = ui.interact(back_rect, id.with("arriere"), Sense::click());
        let front = ui.interact(front_rect, id.with("avant"), Sense::click());
        let painter = ui.painter();
        color_square(painter, back_rect, self.couleurs.prefs.back);
        color_square(painter, front_rect, self.color);
        let swap = self.raccourci("couleur.intervertir");
        let back = crate::bulles::bulle(
            back,
            "Couleur d'arrière-plan",
            &format!("{} · clic : l'échanger avec la couleur d'avant-plan ; clic droit : menu.", couleur::hex(self.couleurs.prefs.back)),
            &swap,
        );
        let front = crate::bulles::bulle(
            front,
            "Couleur d'avant-plan",
            &format!("{} · les outils dessinent avec elle ; clic droit : menu.", couleur::hex(self.color)),
            "",
        );
        if back.clicked() {
            self.swap_colors();
        }
        back.context_menu(|ui| self.front_back_menu(ui));
        front.context_menu(|ui| self.front_back_menu(ui));
        front
    }

    /// Menu du clic droit sur les deux carrés.
    fn front_back_menu(&mut self, ui: &mut egui::Ui) {
        self.action_item(ui, "Intervertir les couleurs", "couleur.intervertir");
        self.action_item(ui, "Couleurs par défaut (noir, blanc)", "couleur.defaut");
        ui.separator();
        self.action_item(ui, "Copier le code de la couleur d'avant-plan", "couleur.copier_code");
    }

    /// Champ du code hexadécimal de la couleur d'avant-plan : le texte tapé est gardé tant que le
    /// champ a le clavier ; chaque code complet (#RRGGBB ou #RGB) change la couleur.
    fn hex_field(&mut self, ui: &mut egui::Ui) {
        let id = ui.id().with("code_hexa");
        let mut text = match &self.couleurs.hex_edit {
            Some((owner, t)) if *owner == id => t.clone(),
            _ => couleur::hex(self.color),
        };
        let resp = ui.add(egui::TextEdit::singleline(&mut text).id(id).desired_width(72.0).font(egui::TextStyle::Monospace));
        if resp.changed() {
            if let Some(c) = couleur::parse_hex(&text) {
                self.set_front(c);
            }
        }
        if resp.has_focus() {
            self.couleurs.hex_edit = Some((id, text));
        } else if self.couleurs.hex_edit.as_ref().is_some_and(|(owner, _)| *owner == id) {
            self.couleurs.hex_edit = None;
        }
        crate::bulles::bulle(resp, "Code hexadécimal", "#RRGGBB, ou #RGB en abrégé ; la couleur change dès que le code est complet.", "");
    }

    /// Les deux carrés, leurs boutons et le code hexadécimal, sur une ligne.
    fn front_back_row(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            self.front_back(ui, 30.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                ui.horizontal(|ui| {
                    if self.bouton(ui, ph::SWAP, "Intervertir les couleurs", "Échanger l'avant-plan et l'arrière-plan.", "couleur.intervertir").clicked() {
                        self.swap_colors();
                    }
                    if self.bouton(ui, ph::SQUARE_HALF, "Couleurs par défaut", "Noir à l'avant-plan, blanc à l'arrière-plan.", "couleur.defaut").clicked() {
                        self.default_colors();
                    }
                    if self.bouton(ui, ph::COPY, "Copier le code", "Copier le code hexadécimal de la couleur d'avant-plan.", "couleur.copier_code").clicked() {
                        self.color_action("couleur.copier_code");
                    }
                });
                self.hex_field(ui);
            });
        });
    }

    /// TSV gardé à part : la teinte d'un gris ou d'un noir ne se perd pas. Il ne suit la couleur
    /// que si elle a changé ailleurs (pipette, nuancier, texte…).
    fn sync_hsv(&mut self) {
        if hsv_to_rgb(self.hsv) != self.color {
            self.hsv = rgb_to_hsv(self.color);
        }
    }

    /// Roue chromatique de la couleur d'avant-plan (`wheel_ui`), avec sa bulle et son menu.
    fn color_wheel(&mut self, ui: &mut egui::Ui, size: f32) {
        self.sync_hsv();
        let mut hsv = self.hsv;
        let resp = wheel_ui(ui, size, &mut hsv, self.couleurs.prefs.wheel);
        if hsv != self.hsv {
            self.hsv = hsv;
            self.color = hsv_to_rgb(hsv);
        }
        let resp = crate::bulles::bulle(
            resp,
            "Roue chromatique",
            "Anneau : la teinte ; dedans : saturation et valeur. Clic droit : carré ou triangle.",
            "",
        );
        resp.context_menu(|ui| self.wheel_menu(ui));
    }

    /// Menu de la roue (clic droit), aussi dans le menu des options.
    fn wheel_menu(&mut self, ui: &mut egui::Ui) {
        self.action_item(ui, "Carré saturation / valeur", "couleur.roue_carre");
        self.action_item(ui, "Triangle saturation / valeur", "couleur.roue_triangle");
        ui.separator();
        let text = if self.couleurs.prefs.wheel_shown { "Cacher la roue" } else { "Montrer la roue" };
        self.action_item(ui, text, "couleur.roue");
    }

    /// Réglages de la roue : la montrer, carré ou triangle.
    fn wheel_options(&mut self, ui: &mut egui::Ui) {
        let p = &mut self.couleurs.prefs;
        let (roue, carre, triangle) = (self.keymap.label("couleur.roue"), self.keymap.label("couleur.roue_carre"), self.keymap.label("couleur.roue_triangle"));
        crate::bulles::interrupteur_a(ui, &mut p.wheel_shown, ph::CIRCLE_HALF, "Roue chromatique", "Montrer ou cacher la roue des teintes.", &roue, "couleur.roue");
        if p.wheel_shown {
            crate::bulles::choix_a(ui, &mut p.wheel, WheelShape::Square, ph::SQUARE, "Carré", "Dans l'anneau, un carré : saturation de gauche à droite, valeur de bas en haut.", &carre, "couleur.roue_carre");
            crate::bulles::choix_a(ui, &mut p.wheel, WheelShape::Triangle, ph::TRIANGLE, "Triangle", "Dans l'anneau, un triangle : la teinte pure, le blanc et le noir à ses sommets.", &triangle, "couleur.roue_triangle");
        }
    }

    /// Choisir la couleur d'avant-plan : réglages de la roue, la roue (`wheel` points de large au
    /// plus), les curseurs.
    fn color_picker(&mut self, ui: &mut egui::Ui, wheel: f32) {
        ui.horizontal(|ui| self.wheel_options(ui));
        if self.couleurs.prefs.wheel_shown {
            let size = ui.available_width().clamp(100.0, wheel);
            ui.vertical_centered(|ui| self.color_wheel(ui, size));
            ui.add_space(4.0);
        }
        self.color_sliders(ui);
    }

    /// Curseurs de la couleur d'avant-plan.
    fn color_sliders(&mut self, ui: &mut egui::Ui) {
        self.sync_hsv();
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
    }

    /// Panneau Couleur.
    pub(crate) fn color_panel(&mut self, ui: &mut egui::Ui) {
        self.front_back_row(ui);
        ui.add_space(4.0);
        self.color_picker(ui, 240.0);
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Nuancier");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let here = self.swatches.iter().position(|c| *c == self.color);
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

    /// À chaque image : les réglages de la couleur sont enregistrés quand ils changent (pas
    /// pendant un glissé ni une saisie).
    pub(crate) fn color_tick(&mut self, ctx: &egui::Context) {
        self.couleurs.prefs.front = self.color;
        if self.couleurs.prefs != self.settings.colors && !ctx.input(|i| i.pointer.any_down()) && !ctx.wants_keyboard_input() {
            self.settings.colors = self.couleurs.prefs.clone();
            if let Err(e) = self.settings.save() {
                self.status = format!("Réglages non enregistrés : {e}");
            }
        }
    }

    /// Au pied de la barre d'outils : les deux couleurs ; un clic sur l'avant-plan ouvre, à côté,
    /// le code et les curseurs.
    pub(crate) fn tool_bar_colors(&mut self, ui: &mut egui::Ui) {
        let front = self.front_back(ui, 22.0);
        let popup = Id::new("couleur_barre_outils");
        if front.clicked() {
            ui.memory_mut(|m| m.toggle_popup(popup));
        }
        if !ui.memory(|m| m.is_popup_open(popup)) {
            return;
        }
        let area = egui::Area::new(popup)
            .kind(egui::UiKind::Popup)
            .order(egui::Order::Foreground)
            .fixed_pos(front.rect.right_top() + vec2(10.0, 0.0))
            .constrain(true)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(250.0);
                    self.front_back_row(ui);
                    ui.add_space(4.0);
                    self.color_picker(ui, 200.0);
                });
            });
        let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if escape || (front.clicked_elsewhere() && area.response.clicked_elsewhere()) {
            ui.memory_mut(|m| m.close_popup());
        }
    }
}
