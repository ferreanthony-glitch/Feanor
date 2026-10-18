//! Aides au cadrage sur l'écran : le tracé des repères sur le cadre de la caméra (avec son zoom,
//! sa rotation, sa bascule) et le réglage de ceux qu'on veut, dans les réglages de la caméra et
//! dans un menu de la barre de vue. La géométrie est dans `cadrage.rs`.

use eframe::egui::{self, Color32, Pos2, Rect, RichText, Shape, Stroke};

use crate::app::App;
use crate::cadrage::SafeZones;
use crate::frames;

impl App {
    /// Trace les repères choisis dans `inside` (le cadre de la caméra, ou le cadre du format s'il y
    /// en a un), en pixels du cadre ; `map` les amène à l'écran en suivant la caméra. Le jeu de
    /// repères n'est recalculé que si la taille ou le choix changent.
    pub(crate) fn paint_framing_aids(&self, painter: &egui::Painter, inside: Rect, map: &dyn Fn(Pos2) -> Pos2) {
        if !self.cadrage.any_on() {
            return;
        }
        let mut cache = self.cadrage_cache.borrow_mut();
        let geometry = cache.get(inside.width(), inside.height(), &self.cadrage);
        let offset = inside.min.to_vec2();
        let (color, width) = (self.cadrage.stroke_color(), self.cadrage.width);
        let shade = Color32::from_black_alpha((100.0 * self.cadrage.opacity) as u8);
        let screen = |line: &[Pos2]| -> Vec<Pos2> { line.iter().map(|p| map(*p + offset)).collect() };
        for line in &geometry.faint {
            painter.add(Shape::line(screen(line), Stroke::new(width * 0.7, color.gamma_multiply(0.45))));
        }
        for line in &geometry.lines {
            let pts = screen(line);
            painter.add(Shape::line(pts.clone(), Stroke::new(width + 1.5, shade)));
            painter.add(Shape::line(pts, Stroke::new(width, color)));
        }
        for dot in &geometry.dots {
            painter.circle(map(*dot + offset), 2.5 + width, color, Stroke::new(1.0_f32, shade));
        }
    }

    /// Le cadre de format suivant dans la liste (aucun, puis chacun), en boucle.
    pub(crate) fn next_frame_format(&mut self) {
        let current = self.layout.frame_format.and_then(frames::sanitized);
        let next = match current.and_then(|c| frames::PRESETS.iter().position(|p| *p == c)) {
            None => Some(frames::PRESETS[0]),
            Some(i) => frames::PRESETS.get(i + 1).copied(),
        };
        self.layout.frame_format = next;
        self.format_custom = false;
        self.status = match next {
            Some(r) => format!("Cadre de format {}.", frames::label(r)),
            None => "Plus de cadre de format.".into(),
        };
    }

    /// Les aides au cadrage : un interrupteur par repère, la spirale et son orientation, les zones
    /// de sécurité, le cadre de format, puis la couleur, l'opacité et l'épaisseur des traits.
    pub(crate) fn framing_aids_ui(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::interrupteur_a;
        use egui_phosphor::regular as ph;
        ui.horizontal_wrapped(|ui| {
            let key = self.raccourci("cadrage.tiers");
            interrupteur_a(ui, &mut self.cadrage.thirds, ph::GRID_NINE, "Tiers", "Règle des tiers : grille de 3 × 3 sur le cadre.", &key, "cadrage.tiers");
            let key = self.raccourci("cadrage.grille_or");
            interrupteur_a(
                ui,
                &mut self.cadrage.phi_grid,
                ph::DOTS_NINE,
                "Grille d'or (phi)",
                "Lignes à 1 / 2,618 et 1,618 / 2,618 de la largeur et de la hauteur (proportions 1 : 0,618 : 1), un point aux quatre intersections.",
                &key,
                "cadrage.grille_or",
            );
            let key = self.raccourci("cadrage.spirale");
            interrupteur_a(
                ui,
                &mut self.cadrage.spiral,
                ph::SPIRAL,
                "Spirale d'or",
                "La spirale de Fibonacci et ses carrés, étirée aux proportions du cadre (les carrés sont des rectangles si le cadre n'est pas un rectangle d'or).",
                &key,
                "cadrage.spirale",
            );
            let label = format!("Orientation de la spirale : {}", crate::cadrage::variant_label(self.cadrage.spiral_variant));
            if self
                .bouton(ui, ph::ARROWS_CLOCKWISE, &label, "Passe à l'orientation suivante (quatre coins, avec ou sans retournement) ; allume la spirale si elle est éteinte.", "cadrage.spirale_orientation")
                .clicked()
            {
                self.run_action("cadrage.spirale_orientation");
            }
            let key = self.raccourci("cadrage.diagonales");
            interrupteur_a(
                ui,
                &mut self.cadrage.diagonals,
                ph::LINE_SEGMENTS,
                "Diagonales",
                "Les deux diagonales du cadre et leurs réciproques, tirées de chaque coin perpendiculairement à la diagonale (symétrie dynamique).",
                &key,
                "cadrage.diagonales",
            );
            let key = self.raccourci("cadrage.croix");
            interrupteur_a(ui, &mut self.cadrage.cross, ph::CROSSHAIR_SIMPLE, "Croix centrale", "Une petite croix au centre du cadre.", &key, "cadrage.croix");
            let key = self.raccourci("vue.cadres_securite");
            interrupteur_a(
                ui,
                &mut self.layout.safe_frames,
                ph::SELECTION,
                "Zones de sécurité",
                "Action et titres, tracés dans le cadre (valeurs au choix ci-dessous ; à vérifier avant une livraison).",
                &key,
                "vue.cadres_securite",
            );
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Format").small().weak());
            self.frame_format_ui(ui);
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Sécurité").small().weak());
            egui::ComboBox::from_id_salt("zones_securite")
                .selected_text(self.cadrage.safe.label())
                .width(190.0)
                .show_ui(ui, |ui| {
                    for z in SafeZones::ALL {
                        ui.selectable_value(&mut self.cadrage.safe, z, z.label());
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.color_edit_button_srgb(&mut self.cadrage.color).on_hover_text("Couleur des repères");
            let mut pct = self.cadrage.opacity * 100.0;
            if ui.add(egui::DragValue::new(&mut pct).range(5.0..=100.0).speed(1.0).max_decimals(0).suffix(" %")).on_hover_text("Opacité des repères").changed() {
                self.cadrage.opacity = pct / 100.0;
            }
            ui.add(egui::DragValue::new(&mut self.cadrage.width).range(0.5..=6.0).speed(0.05).max_decimals(1).suffix(" px")).on_hover_text("Épaisseur des traits");
            if self.bouton(ui, ph::X, "Éteindre les repères", "Tiers, grille d'or, spirale, diagonales et croix. Les zones de sécurité et le cadre de format restent.", "cadrage.aucune").clicked() {
                self.run_action("cadrage.aucune");
            }
        });
        ui.label(RichText::new("Repères d'écran : ils suivent le cadre de la caméra et ne sont jamais exportés.").small().weak());
    }

    /// Le bouton de la barre de vue : un menu avec toutes les aides au cadrage.
    pub(crate) fn framing_aids_button(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        let on = self.cadrage.any_on() || self.layout.safe_frames || self.layout.frame_format.is_some();
        let text = RichText::new(ph::RULER).size(crate::bulles::TAILLE);
        let resp = ui.menu_button(text, |ui| {
            ui.set_min_width(250.0);
            self.framing_aids_ui(ui);
        });
        if on {
            ui.painter().circle_filled(resp.response.rect.right_top() + egui::vec2(-3.0, 3.0), 2.5, crate::theme::contour());
        }
        crate::bulles::bulle(
            resp.response,
            "Aides au cadrage",
            "Tiers, grille d'or, spirale d'or, diagonales, croix centrale, zones de sécurité et cadre de format (2,39:1, 16:9…), tracés sur le cadre de la caméra.",
            "",
        );
    }
}
