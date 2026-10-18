//! Panneau Couleur (6b : curseurs teinte / saturation / valeur avec leur dégradé, code
//! hexadécimal, nuancier avec + et −).
//!
//! Passe couleur (2026-10-16), de haut en bas :
//! - deux couleurs, avant-plan et arrière-plan, comme dans Photoshop (deux carrés qui se
//!   chevauchent ; un clic sur celui de derrière les échange) ; les outils dessinent avec
//!   l'avant-plan, `App::color` ;
//! - la roue chromatique (`wheel_ui`, géométrie dans `roue.rs`) ;
//! - les curseurs TSV, RVB ou TSL, l'opacité du pinceau ;
//! - les couleurs récentes ;
//! - le nuancier montré (`nuancierui.rs`).
//!
//! Le même choix de couleur s'ouvre au pied de la barre d'outils. Conversions : `couleur.rs`.

use eframe::egui::{self, pos2, vec2, Color32, Id, Rect, Response, Sense, Shape, Stroke, StrokeKind, Vec2};
use egui_phosphor::regular as ph;

use crate::app::{App, Tool};
pub use crate::couleur::{hsv_to_rgb, rgb_to_hsv};
use crate::couleur::{self, hsl_to_hsv, hsv_to_hsl, ColorSettings, SliderMode};
use crate::roue::{Wheel, WheelShape, Zone};

/// Une pastille que vise une commande : une couleur récente (rang dans la rangée), ou une pastille
/// du nuancier montré (son rang).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Target {
    Recent(usize),
    Palette(usize),
}

/// Ce dont on tape le nom : un nuancier (rang), ou une pastille (rang du nuancier, rang).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Renamed {
    Palette(usize),
    Swatch(usize, usize),
}

/// Un nom en cours de saisie (`nuancierui.rs`).
pub(crate) struct Rename {
    pub what: Renamed,
    pub text: String,
    /// Le champ a déjà pris le clavier (tout le texte choisi à l'ouverture).
    pub focused: bool,
}

impl Rename {
    pub fn new(what: Renamed, text: String) -> Rename {
        Rename { what, text, focused: false }
    }
}

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
    /// La pastille visée par les commandes « pastille.* » : celle dont le menu est ouvert, sinon
    /// celle sous la souris (vues à l'image précédente ; `*_now` : pendant cette image).
    menu: Option<Target>,
    menu_now: Option<Target>,
    hovered: Option<Target>,
    hovered_now: Option<Target>,
    /// Pastille du nuancier que l'on fait glisser : la grille où le geste a commencé, son rang.
    pub drag: Option<(Id, usize)>,
    /// Nom en cours de saisie (nuancier ou pastille).
    pub rename: Option<Rename>,
    /// Nuancier dont on demande la suppression (confirmation sous son nom).
    pub confirm_delete: Option<usize>,
}

impl ColorState {
    pub fn new(prefs: ColorSettings) -> ColorState {
        let prefs = prefs.sanitized();
        ColorState {
            back_hsv: rgb_to_hsv(prefs.back),
            prefs,
            hex_edit: None,
            menu: None,
            menu_now: None,
            hovered: None,
            hovered_now: None,
            drag: None,
            rename: None,
            confirm_delete: None,
        }
    }
}

/// Une ligne de curseur : lettre, nom, valeur (0 à 1), échelle des chiffres, dégradé.
struct Channel<'a> {
    letter: &'a str,
    name: &'a str,
    value: f32,
    scale: f32,
    colors: Vec<Color32>,
}

/// Les curseurs (barre dégradée et chiffres) ; renvoie le rang du curseur changé et sa nouvelle
/// valeur (0 à 1), et les réponses des lettres et des barres (pour leur clic droit).
fn channel_rows(ui: &mut egui::Ui, channels: &[Channel]) -> (Option<(usize, f32)>, Vec<Response>) {
    let mut out = None;
    let mut parts = Vec::new();
    for (k, c) in channels.iter().enumerate() {
        ui.horizontal(|ui| {
            let label = ui.add(egui::Label::new(egui::RichText::new(c.letter).monospace()).sense(Sense::click()));
            parts.push(crate::bulles::bulle(label, c.name, "Clic droit : TSV, RVB ou TSL.", ""));
            let (bar, moved) = gradient_bar(ui, c.value, &c.colors);
            parts.push(bar);
            if let Some(t) = moved {
                out = Some((k, t));
            }
            let mut shown = c.value * c.scale;
            if ui.add(egui::DragValue::new(&mut shown).range(0.0..=c.scale).max_decimals(0).speed(0.5)).changed() {
                out = Some((k, shown / c.scale));
            }
        });
    }
    (out, parts)
}

/// Une pastille de `side` points : sa couleur, bordée du contour du thème (blanc ou presque noir)
/// si `chosen`.
pub(crate) fn swatch_cell(ui: &mut egui::Ui, c: [u8; 3], side: f32, chosen: bool, sense: Sense) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(side, side), sense);
    ui.painter().rect_filled(rect, 2.0, to_color(c));
    let border = if chosen { Stroke::new(2.0_f32, crate::theme::contour()) } else { Stroke::new(1.0_f32, crate::theme::bord()) };
    ui.painter().rect_stroke(rect, 2.0, border, StrokeKind::Inside);
    resp
}

pub(crate) fn to_color(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Barre dégradée que l'on clique ou glisse (bouton gauche) ; `colors` : couleurs le long de la
/// barre (au moins deux). Renvoie sa réponse, et la nouvelle position (0 à 1) si elle a changé.
fn gradient_bar(ui: &mut egui::Ui, value: f32, colors: &[Color32]) -> (Response, Option<f32>) {
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
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0_f32, crate::theme::bord()), StrokeKind::Outside);
    let x = rect.left() + rect.width() * value.clamp(0.0, 1.0);
    let mark = Rect::from_center_size(pos2(x, rect.center().y), vec2(5.0, rect.height() + 6.0));
    painter.rect(mark, 1.0, Color32::from_black_alpha(80), Stroke::new(1.5_f32, Color32::WHITE), StrokeKind::Middle);
    let moved = resp
        .interact_pointer_pos()
        .filter(|_| resp.dragged_by(egui::PointerButton::Primary) || resp.clicked())
        .map(|p| ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0));
    (resp, moved)
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
            "couleur.curseurs_tsv" => self.couleurs.prefs.sliders = SliderMode::Hsv,
            "couleur.curseurs_rvb" => self.couleurs.prefs.sliders = SliderMode::Rgb,
            "couleur.curseurs_tsl" => self.couleurs.prefs.sliders = SliderMode::Hsl,
            "couleur.recentes_effacer" => {
                self.couleurs.prefs.recent.clear();
                self.status = "Couleurs récentes effacées.".into();
            }
            "pastille.avant_plan" | "pastille.arriere_plan" => match self.target_color() {
                Some(c) if action == "pastille.avant_plan" => self.set_front(c),
                Some(c) => self.set_back(c),
                None => self.no_target(),
            },
            // Nuanciers et pastilles du nuancier (`nuancierui.rs`).
            _ => return self.palette_action(action),
        }
        true
    }

    /// La pastille visée : celle du menu ouvert, sinon celle sous la souris.
    pub(crate) fn target(&self) -> Option<Target> {
        self.couleurs.menu.or(self.couleurs.hovered)
    }

    fn target_color(&self) -> Option<[u8; 3]> {
        match self.target()? {
            Target::Recent(i) => self.couleurs.prefs.recent.get(i).copied(),
            Target::Palette(i) => self.shown_palette().swatches.get(i).map(|s| s.color),
        }
    }

    pub(crate) fn no_target(&mut self) {
        self.status = "Vise d'abord une pastille : la souris dessus, ou clic droit.".into();
    }

    /// Une couleur vient de servir à dessiner : elle passe en tête des couleurs récentes.
    pub(crate) fn color_used(&mut self, c: [u8; 3]) {
        couleur::remember(&mut self.couleurs.prefs.recent, c);
    }

    /// Échange les couleurs d'avant-plan et d'arrière-plan.
    pub(crate) fn swap_colors(&mut self) {
        std::mem::swap(&mut self.color, &mut self.couleurs.prefs.back);
        std::mem::swap(&mut self.hsv, &mut self.couleurs.back_hsv);
        // Une couleur changée ailleurs (pipette, texte…) : sa teinte suit.
        self.sync_hsv();
        let back = self.couleurs.prefs.back;
        self.set_back(back);
    }

    /// Noir à l'avant-plan, blanc à l'arrière-plan.
    pub(crate) fn default_colors(&mut self) {
        self.set_front(couleur::NOIR);
        self.set_back(couleur::BLANC);
    }

    /// Nouvelle couleur d'avant-plan.
    pub(crate) fn set_front(&mut self, c: [u8; 3]) {
        self.color = c;
        self.sync_hsv();
    }

    /// Nouvelle couleur d'avant-plan, réglée en TSV (la teinte d'un gris reste).
    fn set_hsv(&mut self, hsv: [f32; 3]) {
        self.hsv = hsv;
        self.color = hsv_to_rgb(hsv);
    }

    /// Nouvelle couleur d'arrière-plan.
    pub(crate) fn set_back(&mut self, c: [u8; 3]) {
        self.couleurs.prefs.back = c;
        if hsv_to_rgb(self.couleurs.back_hsv) != c {
            self.couleurs.back_hsv = couleur::hsv_following(c, self.couleurs.back_hsv);
        }
    }

    /// Les deux couleurs, comme dans Photoshop : le carré d'avant-plan devant, celui
    /// d'arrière-plan derrière, décalé. Un clic sur celui de derrière les échange ; clic droit :
    /// intervertir, couleurs par défaut, copier le code. `side` : côté d'un carré. Renvoie la
    /// réponse du carré d'avant-plan : à chacun de décider ce que fait son clic, que `click` dit
    /// dans sa bulle.
    pub(crate) fn front_back(&mut self, ui: &mut egui::Ui, side: f32, click: &str) -> Response {
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
            &format!("{} · les outils dessinent avec elle ; {click}clic droit : menu.", couleur::hex(self.color)),
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
        self.action_item(ui, "Ajouter la couleur d'avant-plan au nuancier", "nuancier.ajouter");
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
        let resp = crate::bulles::bulle(resp, "Code hexadécimal", "#RRGGBB, ou #RGB en abrégé ; la couleur change dès que le code est complet. Clic droit : copier.", "");
        resp.context_menu(|ui| self.action_item(ui, "Copier le code de la couleur d'avant-plan", "couleur.copier_code"));
    }

    /// Les deux carrés, leurs boutons et le code hexadécimal, sur une ligne.
    fn front_back_row(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            self.front_back(ui, 30.0, "");
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
            self.hsv = couleur::hsv_following(self.color, self.hsv);
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

    /// Choix des curseurs : TSV, RVB ou TSL (`rtl` : posés de droite à gauche, le premier le
    /// plus à droite).
    fn slider_modes(&mut self, ui: &mut egui::Ui, rtl: bool) {
        let labels = ["couleur.curseurs_tsv", "couleur.curseurs_rvb", "couleur.curseurs_tsl"].map(|a| self.keymap.label(a));
        let p = &mut self.couleurs.prefs;
        let mut modes = [
            (SliderMode::Hsv, "TSV", "Curseurs TSV", "Teinte, saturation, valeur (clarté).", 0, "couleur.curseurs_tsv"),
            (SliderMode::Rgb, "RVB", "Curseurs RVB", "Rouge, vert, bleu, de 0 à 255.", 1, "couleur.curseurs_rvb"),
            (SliderMode::Hsl, "TSL", "Curseurs TSL", "Teinte, saturation, luminosité : à mi-course, la couleur pure ; en haut, le blanc.", 2, "couleur.curseurs_tsl"),
        ];
        if rtl {
            modes.reverse();
        }
        for (mode, short, name, hint, k, action) in modes {
            crate::bulles::choix_a(ui, &mut p.sliders, mode, short, name, hint, &labels[k], action);
        }
    }

    /// Choisir la couleur d'avant-plan : réglages de la roue et des curseurs, la roue (`wheel`
    /// points de large au plus), les curseurs, l'opacité du pinceau.
    fn color_picker(&mut self, ui: &mut egui::Ui, wheel: f32) {
        // Étroit (panneau rétréci) : les réglages de la roue, puis le choix des curseurs dessous,
        // au lieu de se chevaucher.
        if ui.available_width() >= 240.0 {
            ui.horizontal(|ui| {
                self.wheel_options(ui);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| self.slider_modes(ui, true));
            });
        } else {
            ui.horizontal_wrapped(|ui| self.wheel_options(ui));
            ui.horizontal_wrapped(|ui| self.slider_modes(ui, false));
        }
        if self.couleurs.prefs.wheel_shown {
            let size = ui.available_width().clamp(60.0, wheel);
            ui.vertical_centered(|ui| self.color_wheel(ui, size));
            ui.add_space(4.0);
        }
        self.color_sliders(ui);
        self.opacity_row(ui);
    }

    /// Curseurs de la couleur d'avant-plan : TSV, RVB ou TSL, chacun avec son dégradé et ses
    /// chiffres (degrés pour la teinte, pour cent, ou 0 à 255).
    fn color_sliders(&mut self, ui: &mut egui::Ui) {
        self.sync_hsv();
        let [h, s, v] = self.hsv;
        let hsv = |c: [f32; 3]| to_color(hsv_to_rgb(c));
        let hues: Vec<Color32> = (0..=12).map(|k| hsv([k as f32 * 30.0, 1.0, 1.0])).collect();
        let hue = |colors: Vec<Color32>| Channel { letter: "T", name: "Teinte", value: h / 360.0, scale: 360.0, colors };
        let parts = match self.couleurs.prefs.sliders {
            SliderMode::Hsv => {
                let rows = [
                    hue(hues),
                    Channel { letter: "S", name: "Saturation", value: s, scale: 100.0, colors: vec![hsv([h, 0.0, v]), hsv([h, 1.0, v])] },
                    Channel { letter: "V", name: "Valeur (clarté)", value: v, scale: 100.0, colors: vec![Color32::BLACK, hsv([h, s, 1.0])] },
                ];
                let (changed, parts) = channel_rows(ui, &rows);
                if let Some((k, t)) = changed {
                    let mut next = self.hsv;
                    next[k] = t * if k == 0 { 360.0 } else { 1.0 };
                    self.set_hsv(next);
                }
                parts
            }
            SliderMode::Rgb => {
                let c = self.color;
                let with = |k: usize, x: u8| {
                    let mut c = c;
                    c[k] = x;
                    to_color(c)
                };
                let names = [("R", "Rouge"), ("V", "Vert"), ("B", "Bleu")];
                let rows: Vec<Channel> = names
                    .iter()
                    .enumerate()
                    .map(|(k, &(letter, name))| Channel {
                        letter,
                        name,
                        value: c[k] as f32 / 255.0,
                        scale: 255.0,
                        colors: vec![with(k, 0), with(k, 255)],
                    })
                    .collect();
                let (changed, parts) = channel_rows(ui, &rows);
                if let Some((k, t)) = changed {
                    let mut next = c;
                    next[k] = (t * 255.0).round().clamp(0.0, 255.0) as u8;
                    self.set_front(next);
                }
                parts
            }
            SliderMode::Hsl => {
                let [_, sl, l] = hsv_to_hsl(self.hsv);
                let hsl = |c: [f32; 3]| to_color(couleur::hsl_to_rgb(c));
                let rows = [
                    hue(hues),
                    Channel { letter: "S", name: "Saturation (TSL)", value: sl, scale: 100.0, colors: vec![hsl([h, 0.0, l]), hsl([h, 1.0, l])] },
                    Channel {
                        letter: "L",
                        name: "Luminosité",
                        value: l,
                        scale: 100.0,
                        colors: vec![Color32::BLACK, hsl([h, sl, 0.5]), Color32::WHITE],
                    },
                ];
                let (changed, parts) = channel_rows(ui, &rows);
                if let Some((k, t)) = changed {
                    let mut next = [h, sl, l];
                    next[k] = t * if k == 0 { 360.0 } else { 1.0 };
                    self.set_hsv(hsl_to_hsv(next));
                }
                parts
            }
        };
        // Clic droit sur une lettre ou une barre : changer de curseurs.
        for part in parts {
            part.context_menu(|ui| self.slider_menu(ui));
        }
    }

    /// Menu des curseurs (clic droit sur une lettre ou une barre).
    fn slider_menu(&mut self, ui: &mut egui::Ui) {
        self.action_item(ui, "Curseurs TSV (teinte, saturation, valeur)", "couleur.curseurs_tsv");
        self.action_item(ui, "Curseurs RVB (rouge, vert, bleu)", "couleur.curseurs_rvb");
        self.action_item(ui, "Curseurs TSL (teinte, saturation, luminosité)", "couleur.curseurs_tsl");
    }

    /// Opacité : la couleur n'en a pas, le pinceau et le crayon si (la même que dans leurs
    /// réglages). Pour un autre outil, la ligne est grisée.
    fn opacity_row(&mut self, ui: &mut egui::Ui) {
        let color = to_color(self.color);
        let brush = match self.tool {
            Tool::Brush => Some(&mut self.brush),
            Tool::Pencil => Some(&mut self.pencil),
            _ => None,
        };
        ui.horizontal(|ui| {
            let label = ui.add(egui::Label::new(egui::RichText::new(ph::DROP_HALF).size(crate::bulles::TAILLE)).sense(Sense::hover()));
            crate::bulles::bulle(
                label,
                "Opacité du pinceau",
                "La couleur elle-même n'a pas d'opacité : c'est celle du pinceau ou du crayon, la même que dans leurs réglages.",
                "",
            );
            match brush {
                Some(b) => {
                    if let (_, Some(t)) = gradient_bar(ui, b.opacity, &[Color32::TRANSPARENT, color]) {
                        b.opacity = t;
                    }
                    let mut pct = b.opacity * 100.0;
                    if ui.add(egui::DragValue::new(&mut pct).range(0.0..=100.0).max_decimals(0).speed(0.5).suffix(" %")).changed() {
                        b.opacity = pct / 100.0;
                    }
                }
                None => {
                    ui.label(egui::RichText::new("pinceau et crayon seulement").weak());
                }
            }
        });
    }

    /// Les dernières couleurs avec lesquelles on a dessiné, sur une rangée (autant qu'il en tient).
    fn recent_row(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
            let label = ui.add(egui::Label::new(egui::RichText::new(ph::CLOCK_COUNTER_CLOCKWISE).size(crate::bulles::TAILLE)).sense(Sense::click()));
            let label = crate::bulles::bulle(
                label,
                "Couleurs récentes",
                "Les dernières couleurs avec lesquelles on a dessiné (pinceau, crayon, remplissage, formes, texte). Clic sur une couleur : avant-plan ; clic droit : menu.",
                "",
            );
            label.context_menu(|ui| self.action_item(ui, "Effacer les couleurs récentes", "couleur.recentes_effacer"));
            let side = 16.0;
            let fits = ((ui.available_width() + 2.0) / (side + 2.0)).floor().max(0.0) as usize;
            let recent = self.couleurs.prefs.recent.clone();
            if recent.is_empty() {
                ui.label(egui::RichText::new("aucune pour l'instant").weak());
            }
            for (i, c) in recent.into_iter().enumerate().take(fits) {
                let resp = swatch_cell(ui, c, side, c == self.color, Sense::click());
                self.target_cell(resp, Target::Recent(i), c, "");
            }
        });
    }

    /// Ce que fait une pastille (couleur récente ou du nuancier) : clic, avant-plan ; souris
    /// dessus ou clic droit, elle devient la pastille visée par les commandes « pastille.* ». Sa
    /// bulle : son nom et son code.
    pub(crate) fn target_cell(&mut self, resp: Response, target: Target, c: [u8; 3], name: &str) {
        let title = if name.is_empty() { couleur::hex(c) } else { format!("{name} · {}", couleur::hex(c)) };
        let help = match target {
            Target::Recent(_) => "Clic : couleur d'avant-plan ; clic droit : menu.",
            Target::Palette(_) => "Clic : couleur d'avant-plan ; glisser : la ranger ailleurs ; clic droit : menu (nommer, retirer…).",
        };
        let resp = crate::bulles::bulle(resp, &title, help, "");
        if resp.hovered() {
            self.couleurs.hovered_now = Some(target);
        }
        if resp.clicked() {
            self.set_front(c);
        }
        resp.context_menu(|ui| self.target_menu(ui, target));
    }

    /// Menu d'une pastille visée.
    fn target_menu(&mut self, ui: &mut egui::Ui, target: Target) {
        self.couleurs.menu = Some(target);
        self.couleurs.menu_now = Some(target);
        self.action_item(ui, "Couleur d'avant-plan", "pastille.avant_plan");
        self.action_item(ui, "Couleur d'arrière-plan", "pastille.arriere_plan");
        match target {
            Target::Recent(_) => {
                self.action_item(ui, "Ajouter au nuancier", "nuancier.ajouter");
                ui.separator();
                self.action_item(ui, "Effacer les couleurs récentes", "couleur.recentes_effacer");
            }
            Target::Palette(_) => self.palette_swatch_menu(ui),
        }
    }

    /// Panneau Couleur.
    pub(crate) fn color_panel(&mut self, ui: &mut egui::Ui) {
        self.front_back_row(ui);
        ui.add_space(4.0);
        self.color_picker(ui, 240.0);
        ui.add_space(2.0);
        self.recent_row(ui);
        ui.add_space(6.0);
        // Nuanciers personnels (`nuancierui.rs`).
        self.palette_section(ui);
    }

    /// À chaque image : les réglages de la couleur sont enregistrés quand ils changent (pas
    /// pendant un glissé ni une saisie).
    pub(crate) fn color_tick(&mut self, ctx: &egui::Context) {
        // La pastille visée pour l'image suivante : menu ouvert, souris dessus.
        self.couleurs.menu = self.couleurs.menu_now.take();
        self.couleurs.hovered = self.couleurs.hovered_now.take();
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
        let front = self.front_back(ui, 22.0, "clic : la roue, les curseurs et le nuancier à côté ; ");
        let popup = Id::new("couleur_barre_outils");
        if front.clicked() {
            ui.memory_mut(|m| m.toggle_popup(popup));
        }
        if !ui.memory(|m| m.is_popup_open(popup)) {
            return;
        }
        // À droite de la barre d'outils, à la hauteur des carrés.
        let at = pos2(ui.clip_rect().right().max(front.rect.right()) + 6.0, front.rect.top());
        let area = egui::Area::new(popup)
            .kind(egui::UiKind::Popup)
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .constrain(true)
            .show(ui.ctx(), |ui| {
                let max_height = (ui.ctx().screen_rect().height() - 40.0).max(200.0);
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    egui::ScrollArea::vertical().max_height(max_height).show(ui, |ui| {
                        ui.set_width(250.0);
                        self.front_back_row(ui);
                        ui.add_space(4.0);
                        self.color_picker(ui, 200.0);
                        ui.add_space(2.0);
                        self.recent_row(ui);
                        ui.add_space(4.0);
                        // Les pastilles du nuancier montré (ses commandes sont dans le panneau Couleur).
                        self.swatch_grid(ui);
                    });
                });
            });
        let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if escape || (front.clicked_elsewhere() && area.response.clicked_elsewhere()) {
            ui.memory_mut(|m| m.close_popup());
        }
    }
}
