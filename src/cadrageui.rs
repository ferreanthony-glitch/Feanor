//! Aides au cadrage sur l'écran : le tracé des repères sur le cadre de la caméra (avec son zoom,
//! sa rotation, sa bascule) et leurs réglages, en deux endroits (Anthony, 2026-10-21) :
//! - **la règle**, dans la barre au-dessus du dessin, est le seul lieu des guides : les repères de composition
//!   (tiers, grille d'or, spirale, diagonales, croix) et les guides de perspective du projet (`guides.rs`) ;
//! - **le menu Cadrage**, à gauche de la barre sous le dessin (`viewbar.rs`) : le format, les zones de sécurité et
//!   l'aspect des traits (couleur, opacité, épaisseur).
//! La géométrie est dans `cadrage.rs`.

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
            painter.add(Shape::line(pts.clone(), Stroke::new(width + 1.0, shade)));
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

    /// La règle, au-dessus du dessin : le seul endroit des guides. En haut les repères de composition (tiers, grille
    /// d'or, spirale et son orientation, diagonales, croix), dessous les guides de perspective du projet (ce que
    /// faisait le panneau Guides, retiré le 2026-10-21 : afficher, aligner, verrouiller, lignes de référence, gomme,
    /// lignes du curseur, la liste, un guide de plus à 1, 2 ou 3 points de fuite, son nom, le supprimer).
    /// Le format, les zones de sécurité et l'aspect des traits sont dans le menu Cadrage de la barre du bas.
    pub(crate) fn guides_menu_ui(&mut self, ui: &mut egui::Ui) {
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
            if self.bouton(ui, ph::X, "Éteindre les repères", "Tiers, grille d'or, spirale, diagonales et croix. Les guides de perspective, les zones de sécurité et le cadre de format restent.", "cadrage.aucune").clicked() {
                self.run_action("cadrage.aucune");
            }
        });
        ui.separator();
        ui.label(RichText::new("Perspective").small().weak());
        self.guides_ui(ui);
        ui.separator();
        ui.label(RichText::new("Aides d'écran : elles suivent le cadre de la caméra et ne sont jamais exportées.").small().weak());
    }

    /// Le bouton de la règle, dans la barre au-dessus du dessin : le menu des guides. Il s'allume dès qu'un repère ou
    /// le guide de perspective est tracé.
    pub(crate) fn guides_button(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        let on = self.guides_any_on();
        let [w, h] = crate::barres::CASE;
        let text = crate::bulles::encre_etat(RichText::new(ph::RULER).size(crate::barres::ICONE), on);
        let resp = egui::menu::menu_custom_button(ui, egui::Button::new(text).selected(on).frame(on).min_size(egui::vec2(w, h)), |ui| {
            ui.set_min_width(260.0);
            self.guides_menu_ui(ui);
        });
        crate::bulles::bulle(
            resp.response,
            "Guides",
            "Tiers, grille d'or, spirale d'or, diagonales, croix centrale, et les guides de perspective du projet (1, 2 ou 3 points de fuite, aligner les traits, verrouiller).",
            "",
        );
    }

    /// Un repère de composition ou le guide de perspective est tracé (la règle s'allume).
    pub(crate) fn guides_any_on(&self) -> bool {
        self.cadrage.any_on() || self.guides.enabled
    }

    /// Un cadre de format ou les zones de sécurité sont tracés (l'icône Cadrage de la barre du bas s'allume).
    pub(crate) fn framing_any_on(&self) -> bool {
        self.layout.safe_frames || self.layout.frame_format.and_then(frames::sanitized).is_some()
    }

    /// Le menu Cadrage, petit panneau de la barre du bas : le format (cadre d'un format de diffusion), les zones de
    /// sécurité et l'aspect des traits des repères (couleur, opacité, épaisseur). Pas de sous-menu ni de liste
    /// déroulante : le panneau se ferme dès qu'on clique hors de lui, et un sous-menu est « hors de lui ».
    pub(crate) fn cadrage_ui(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::{bulle, interrupteur_a};
        use egui_phosphor::regular as ph;
        ui.label(RichText::new("Format").small().weak());
        let current = self.layout.frame_format.and_then(frames::sanitized);
        let custom = current.is_some_and(|r| self.format_custom || !frames::is_preset(r));
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            let r = ui.selectable_label(current.is_none(), "Aucun");
            if bulle(r, "Aucun format", "Pas de cadre de format : toute l'image de la caméra.", "").clicked() {
                self.layout.frame_format = None;
                self.format_custom = false;
            }
            for f in frames::PRESETS {
                let r = ui.selectable_label(current == Some(f) && !custom, frames::label(f));
                if bulle(r, &format!("Format {}", frames::label(f)), "Cadre d'un format de diffusion, tracé dans le cadre de la caméra (hors-format estompé).", "").clicked() {
                    self.layout.frame_format = Some(f);
                    self.format_custom = false;
                }
            }
            let r = ui.selectable_label(custom, "Personnalisé");
            if bulle(r, "Format personnalisé", "Largeur et hauteur relatives au choix, ci-dessous.", "").clicked() {
                self.layout.frame_format = Some(current.unwrap_or([2.39, 1.0]));
                self.format_custom = true;
            }
        });
        if let (true, Some(mut r)) = (custom, current) {
            ui.horizontal(|ui| {
                let a = ui.add(egui::DragValue::new(&mut r[0]).range(0.01..=1000.0).speed(0.01).max_decimals(2));
                bulle(a.clone(), "Largeur relative", "Glisser ou taper une valeur.", "");
                ui.label(":");
                let b = ui.add(egui::DragValue::new(&mut r[1]).range(0.01..=1000.0).speed(0.01).max_decimals(2));
                bulle(b.clone(), "Hauteur relative", "Glisser ou taper une valeur.", "");
                if a.changed() || b.changed() {
                    self.layout.frame_format = frames::sanitized(r);
                }
            });
        }
        ui.separator();
        ui.horizontal(|ui| {
            let key = self.raccourci("vue.cadres_securite");
            interrupteur_a(
                ui,
                &mut self.layout.safe_frames,
                ph::SELECTION,
                "Zones de sécurité",
                "Action et titres, tracés dans le cadre (valeurs au choix à côté ; à vérifier avant une livraison).",
                &key,
                "vue.cadres_securite",
            );
            ui.label(RichText::new("Zones de sécurité").small().weak());
        });
        for z in SafeZones::ALL {
            ui.selectable_value(&mut self.cadrage.safe, z, z.label());
        }
        ui.separator();
        ui.label(RichText::new("Aspect des traits des repères").small().weak());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            const COULEURS: [[u8; 3]; 6] = [[90, 220, 255], [255, 255, 255], [255, 220, 60], [255, 90, 200], [110, 230, 120], [255, 90, 70]];
            for c in COULEURS {
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
                ui.painter().rect_filled(rect, 3.0, Color32::from_rgb(c[0], c[1], c[2]));
                if self.cadrage.color == c {
                    ui.painter().rect_stroke(rect.expand(1.5), 3.0, Stroke::new(1.5_f32, ui.visuals().text_color()), egui::StrokeKind::Outside);
                }
                if bulle(resp, "Couleur des repères", "Tiers, grille d'or, spirale, diagonales et croix.", "").clicked() {
                    self.cadrage.color = c;
                }
            }
        });
        ui.horizontal(|ui| {
            let mut pct = self.cadrage.opacity * 100.0;
            let resp = ui.add(egui::DragValue::new(&mut pct).range(5.0..=100.0).speed(1.0).max_decimals(0).suffix(" %"));
            if bulle(resp, "Opacité des repères", "Glisser ou taper une valeur.", "").changed() {
                self.cadrage.opacity = pct / 100.0;
            }
            let resp = ui.add(egui::DragValue::new(&mut self.cadrage.width).range(0.5..=6.0).speed(0.05).max_decimals(1).suffix(" px"));
            bulle(resp, "Épaisseur des traits", "Glisser ou taper une valeur.", "");
        });
        // Une couleur à soi : le sélecteur se déplie ici, dans le panneau (un sous-menu le fermerait).
        egui::CollapsingHeader::new(RichText::new("Autre couleur").small().weak()).id_salt("cadrage_autre_couleur").show(ui, |ui| {
            let [r, g, b] = self.cadrage.color;
            let mut c = Color32::from_rgb(r, g, b);
            if egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::Opaque) {
                self.cadrage.color = [c.r(), c.g(), c.b()];
            }
        });
        ui.label(RichText::new("Aides d'écran : elles suivent le cadre de la caméra et ne sont jamais exportées.").small().weak());
    }
}
