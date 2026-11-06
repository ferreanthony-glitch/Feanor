//! Barre au-dessus du dessin (9d) : vue du plateau (toute la feuille, avec le cadre de la caméra)
//! ou vue caméra (ce que filme la caméra du plan), et cadres de composition tracés dans le
//! cadre de la caméra : zones de sécurité et cadre d'un format de diffusion (9:16, 1:1…).

use eframe::egui::{self, vec2, Align2, Color32, FontId, Pos2, Rect, Shape, Stroke, Vec2};

use crate::app::App;
use crate::frames;

/// Hauteur de la barre.
pub(crate) const VIEW_BAR_H: f32 = 28.0;

/// Ce que font les touches avec l'outil `tool`, en une ligne sous le dessin (Anthony,
/// 2026-10-18 : les gestes à touche, comme l'enveloppe ou le lasso de Ctrl, ne se devinent pas).
pub(crate) fn tool_hint(tool: crate::app::Tool) -> &'static str {
    use crate::app::Tool;
    match tool {
        Tool::Brush | Tool::Pencil | Tool::Eraser => "Alt + clic : pipette · Ctrl tenu : lasso · Espace : déplacer la vue · ^ $ : taille",
        Tool::Mixer => "Glisser : étaler et mêler ce qui est peint (bitmap) · Alt + clic : pipette · ^ $ : taille",
        Tool::Fill => "Clic : remplir · Alt + clic : pipette",
        Tool::Gradient => "Glisser : tracer l'axe (Maj : par 15°) · dans la sélection, sinon dans la zone fermée · Alt : tout le calque",
        Tool::Cutter => "Glisser en travers des bouts à enlever · clic : un bout",
        Tool::Select => "Lasso (Maj : ajouter · Ctrl+Maj : tous les calques) · boîte : dedans déplacer · coin ou côté : échelle (Maj : proportions) · hors d'un coin : tourner · bord : incliner · point rond : pivot · flèches : 1 px (Maj : 10)",
        Tool::SelectColor => "Clic : une couleur (Maj : ajouter) · la boîte se manie comme avec la Sélection",
        Tool::Line => "Glisser : tracer · Maj : par 15°",
        Tool::Rect | Tool::Ellipse => "Glisser : tracer · Maj : carré ou cercle",
        Tool::Polyline => "Un clic par point (Maj : par 15°) · double-clic ou Entrée : terminer",
        Tool::Text => "Clic : poser un texte · Ctrl + Entrée : valider · Échap : annuler",
        Tool::Transform => "Coin : agrandir (Maj : proportions · Ctrl : autre point fixe) · côté : étirer · rond ou hors d'un coin : tourner · Ctrl+R : annuler",
        Tool::Perspective => "Enveloppe ou perspective (Réglages) · tirer un coin ou une poignée ronde · l'intérieur : déplacer · Entrée : terminer",
        Tool::Grab => "Appui : prendre les points du cercle · glisser : les tirer (plus ou moins selon la distance au centre) · rayon, force, atténuation : Réglages",
        Tool::Pull => "Glisser : les points sous le cercle suivent le mouvement, comme un doigt · rayon, force, atténuation : Réglages",
        Tool::Picker => "Clic : prendre la couleur",
        Tool::Pan => "Glisser : déplacer la vue",
        Tool::Camera => "Glisser : déplacer (Maj : à l'horizontale ou à la verticale) · coin : zoomer · rond : tourner (Maj : 15°)",
    }
}
/// Hauteur de la barre des commandes de la vue, sous le dessin.
pub(crate) const TOOLS_BAR_H: f32 = 28.0;
/// Couleur des zones de sécurité (celle du cadre de format est un réglage : `Settings::cadre_format_couleur`).
const SAFE: Color32 = crate::theme::CADRAGE_SECURITE;

/// Voile blanc d'opacité `a` (couleur prémultipliée : `Color32::from_white_alpha` passe par
/// l'espace linéaire et donne un voile bien plus clair).
pub(crate) fn veil(a: u8) -> Color32 {
    Color32::from_rgba_premultiplied(a, a, a, a)
}

/// Polygone convexe `poly` coupé au rectangle `r` (le morceau qui est dedans, convexe aussi).
pub(crate) fn clip_to_rect(poly: &[Pos2], r: Rect) -> Vec<Pos2> {
    let mut pts = poly.to_vec();
    // Quatre demi-plans : x ≥ gauche, x ≤ droite, y ≥ haut, y ≤ bas.
    let sides: [(fn(Pos2) -> f32, f32, f32); 4] = [(|p| p.x, r.min.x, 1.0), (|p| p.x, r.max.x, -1.0), (|p| p.y, r.min.y, 1.0), (|p| p.y, r.max.y, -1.0)];
    for (coord, limit, sign) in sides {
        if pts.is_empty() {
            break;
        }
        let inside = |p: Pos2| (coord(p) - limit) * sign >= 0.0;
        let mut out = Vec::with_capacity(pts.len() + 2);
        for k in 0..pts.len() {
            let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
            if inside(a) {
                out.push(a);
            }
            if inside(a) != inside(b) {
                let t = (limit - coord(a)) / (coord(b) - coord(a));
                out.push(a + (b - a) * t);
            }
        }
        pts = out;
    }
    pts
}

/// Bande entre deux polygones convexes, `inner` dans `outer`, d'une couleur : `outer` moins `inner`, en morceaux
/// convexes qui ne se recouvrent pas (le voile est translucide). On coupe `outer` le long de chaque côté de `inner` :
/// ce qui est au-delà du côté est un morceau, le reste est coupé par le côté suivant. Aucun appariement de sommets
/// entre les deux polygones, dont l'ordre et la place changent avec la rotation et le miroir de la vue (un
/// appariement coin à coin croisait la bande : de grands triangles de travers).
pub(crate) fn nested_band(outer: &[Pos2], inner: &[Pos2], color: Color32) -> Shape {
    mesh_of_pieces(&difference_convexe(outer, inner), color)
}

/// Des morceaux convexes d'une couleur, en un seul maillage (sans recouvrement ni bord adouci : pas de fine ligne entre eux).
pub(crate) fn mesh_of_pieces(pieces: &[Vec<Pos2>], color: Color32) -> Shape {
    let mut mesh = egui::Mesh::default();
    for piece in pieces.iter().filter(|p| p.len() >= 3) {
        let base = mesh.vertices.len() as u32;
        for p in piece {
            mesh.colored_vertex(*p, color);
        }
        for k in 1..piece.len() as u32 - 1 {
            mesh.add_triangle(base, base + k, base + k + 1);
        }
    }
    Shape::mesh(mesh)
}

/// `outer` moins `inner` (deux polygones convexes), en morceaux convexes qui ne se recouvrent pas : `outer` coupé le long de
/// chaque côté de `inner`, ce qui est au-delà du côté est un morceau. `inner` n'a pas à être dans `outer`.
pub(crate) fn difference_convexe(outer: &[Pos2], inner: &[Pos2]) -> Vec<Vec<Pos2>> {
    if inner.len() < 3 {
        return vec![outer.to_vec()];
    }
    let mut pieces: Vec<Vec<Pos2>> = Vec::new();
    // Sens du polygone intérieur (le « dedans » d'un côté est à sa gauche ou à sa droite).
    let turn: f32 = (0..inner.len()).map(|i| inner[i].x * inner[(i + 1) % inner.len()].y - inner[(i + 1) % inner.len()].x * inner[i].y).sum();
    let sign = if turn >= 0.0 { 1.0 } else { -1.0 };
    let mut rest = outer.to_vec();
    for i in 0..inner.len() {
        if rest.len() < 3 {
            break;
        }
        let (a, b) = (inner[i], inner[(i + 1) % inner.len()]);
        if (b - a).length() < 1e-6 {
            continue;
        }
        // Distance signée au côté : positive du côté du dedans.
        let side = |p: Pos2| sign * ((b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x));
        let (beyond, inside) = split_convex(&rest, &side);
        if beyond.len() >= 3 {
            pieces.push(beyond);
        }
        rest = inside;
    }
    pieces
}

/// Coupe un polygone convexe en deux le long de `side` = 0 : (le morceau où `side` ≤ 0, celui où `side` ≥ 0). Les
/// points de coupe sont calculés de la même façon des deux côtés : les morceaux se joignent exactement.
fn split_convex(poly: &[Pos2], side: &dyn Fn(Pos2) -> f32) -> (Vec<Pos2>, Vec<Pos2>) {
    let (mut neg, mut pos) = (Vec::with_capacity(poly.len() + 2), Vec::with_capacity(poly.len() + 2));
    for k in 0..poly.len() {
        let (p, q) = (poly[k], poly[(k + 1) % poly.len()]);
        let (dp, dq) = (side(p), side(q));
        if dp <= 0.0 {
            neg.push(p);
        }
        if dp >= 0.0 {
            pos.push(p);
        }
        if (dp < 0.0 && dq > 0.0) || (dp > 0.0 && dq < 0.0) {
            let cut = p + (q - p) * (dp / (dp - dq));
            neg.push(cut);
            pos.push(cut);
        }
    }
    (neg, pos)
}

/// Bande entre deux quadrilatères (l'un dans l'autre), d'une couleur. Les sommets ne sont pas
/// appariés un à un : une vue tournée ou en miroir met les coins de `inner` ailleurs que ceux d'un
/// rectangle aligné sur l'écran (`outer`), et l'appariement croiserait la bande (grands triangles
/// de travers). La bande est donc découpée côté par côté (`nested_band`).
pub(crate) fn band(outer: [Pos2; 4], inner: [Pos2; 4], color: Color32) -> Shape {
    nested_band(&outer, &inner, color)
}

/// Le rectangle de l'écran `rect` (marge de 16 points) élargi pour contenir les points `inner` : le
/// bord extérieur du voile qui entoure un cadre, dans l'ordre haut-gauche, haut-droite, bas-droite,
/// bas-gauche.
pub(crate) fn around_screen(rect: Rect, inner: &[Pos2]) -> [Pos2; 4] {
    let mut r = rect.expand(16.0);
    for p in inner {
        r.extend_with(*p);
    }
    [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()]
}

impl App {
    /// Barre sous le dessin, réduite à des icônes (simplification des barres, 2026-10-21) : le cadrage (format, zones de
    /// sécurité), les repères de composition (et l'aspect de leurs traits, 2026-10-23), puis ajuster, zoom, rotation, miroir, pelure d'oignon, table lumineuse. Les réglages
    /// fins (le pourcentage du zoom, l'angle, combien de dessins avant et après, la force de la table lumineuse) sont dans
    /// de petits panneaux qui s'ouvrent au clic, au-dessus de l'icône. Les vues (plateau, caméra, Espace 3D) et la règle
    /// des guides sont dans la barre au-dessus du dessin.
    pub(crate) fn view_tools_bar(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        ui.painter().rect_filled(ui.max_rect(), 0.0, crate::theme::entete());
        // Dans l'Espace 3D, on ne dessine pas à plat : cette barre n'a rien à offrir.
        let in3d = self.espace3d.on && !self.camera_view;
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing = vec2(crate::barres::ECART, 0.0);
            ui.add_space(4.0);
            if in3d {
                return;
            }
            let sep = |ui: &mut egui::Ui| {
                ui.add_space(crate::barres::MARGE_TRAIT);
                ui.separator();
                ui.add_space(crate::barres::MARGE_TRAIT);
            };
            self.cadrage_reglage(ui);
            self.reperes_reglage(ui);
            sep(ui);
            self.command_button(ui, ph::ARROWS_OUT, "vue.ajuster", "Ajuster à la fenêtre", "Toute la feuille dans la vue.", None);
            self.zoom_reglage(ui);
            self.rotation_reglage(ui);
            self.command_button(ui, ph::FLIP_HORIZONTAL, "vue.miroir", "Miroir de la vue", "Voir le dessin retourné, pour vérifier ses proportions ; le dessin ne change pas.", Some(self.view.flip));
            sep(ui);
            self.pelure_reglage(ui);
            self.table_lumineuse_reglage(ui);
            sep(ui);
            self.passe_partout_reglage(ui);
            // Ce que font les touches avec l'outil courant (Anthony, 2026-10-18, axe d'ergonomie 4).
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                // Sur l'aplat (`fondui.rs`), ce que font les outils : « Aplat de <calque> ».
                let aplat = self.aplat_en_cible().then(|| self.aplat_indication());
                let hint = match self.tool {
                    crate::app::Tool::Fill | crate::app::Tool::Gradient | crate::app::Tool::Cutter | crate::app::Tool::Eraser | crate::app::Tool::Brush | crate::app::Tool::Mixer | crate::app::Tool::Pencil | crate::app::Tool::Select if aplat.is_some() => {
                        aplat.as_deref().unwrap_or_default()
                    }
                    // Plusieurs calques choisis (Ctrl ou Maj + clic sur leur ligne).
                    crate::app::Tool::Select if self.chosen_layers().len() >= 2 => "Calques choisis : le lasso prend sur tous · une boîte commune : dedans déplacer · coin : échelle · hors d'un coin : tourner · bord : incliner · point rond : pivot · flèches : 1 px (Maj : 10) · Suppr",
                    crate::app::Tool::Transform if self.chosen_layers().len() >= 2 => "Calques choisis ensemble : glisser dans la boîte : déplacer · coin : agrandir · rond : tourner · Ctrl+R : remettre en place",
                    crate::app::Tool::Perspective if self.chosen_layers().len() >= 2 => "Enveloppe ou perspective : le calque courant seul (choisis un seul calque pour déformer)",
                    t => tool_hint(t),
                };
                ui.add(egui::Label::new(egui::RichText::new(hint).small().weak()).truncate()).on_hover_text(hint);
            });
        });
    }

    /// Un petit panneau de réglages posé au-dessus de la case `resp` de la barre du bas : il s'ouvre et se ferme à son
    /// clic, se ferme aussi d'un clic ailleurs ou d'Échap. Le contenu reçoit l'application.
    pub(crate) fn panneau_de_case(&mut self, ui: &mut egui::Ui, id: &str, resp: &egui::Response, largeur: f32, contenu: impl FnOnce(&mut App, &mut egui::Ui)) {
        let pid = ui.id().with(("panneau_de_case", id));
        if resp.clicked() {
            ui.memory_mut(|m| m.toggle_popup(pid));
        }
        egui::popup::popup_above_or_below_widget(ui, pid, resp, egui::AboveOrBelow::Above, egui::popup::PopupCloseBehavior::CloseOnClickOutside, |ui| {
            ui.set_min_width(largeur);
            contenu(self, ui);
        });
    }

    /// Le cadrage, tout à gauche : une icône (allumée quand un format ou les zones de sécurité sont tracés), et au clic
    /// le format et les zones de sécurité (`cadrageui.rs`). Les repères de composition sont juste à droite
    /// (`reperes_reglage`), les guides de perspective dans la règle, au-dessus du dessin.
    pub(crate) fn cadrage_reglage(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        let on = self.framing_any_on();
        let resp = self.case_icone(ui, ph::CROP, Some(on), "Cadrage", "Le format (16:9, 2,39:1…) et les zones de sécurité.", "");
        self.panneau_de_case(ui, "cadrage", &resp, 250.0, |app, ui| app.cadrage_ui(ui));
    }

    /// Les repères de composition, juste à droite de Cadrage (Anthony, 2026-10-23) : une icône de la même taille (allumée
    /// quand un repère est tracé), et au clic les tiers, la grille d'or, la spirale et son orientation, les diagonales, la
    /// croix, « Éteindre les repères » et l'aspect de leurs traits (`reperes_menu_ui`).
    pub(crate) fn reperes_reglage(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        let on = self.reperes_any_on();
        let resp = self.case_icone(
            ui,
            ph::GRID_FOUR,
            Some(on),
            "Repères de composition",
            "Tiers, grille d'or, spirale d'or, diagonales, croix centrale, et la couleur, l'opacité et l'épaisseur de leurs traits.",
            "",
        );
        self.panneau_de_case(ui, "reperes", &resp, 260.0, |app, ui| app.reperes_menu_ui(ui));
    }

    /// Le zoom : une icône, et au clic le pourcentage (qu'on glisse ou qu'on tape), « − » et « + », ajuster et 100 %.
    fn zoom_reglage(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        let resp = self.case_icone(ui, ph::MAGNIFYING_GLASS, Some(false), "Zoom", "Le pourcentage de zoom, plus et moins, 100 %.", "");
        self.panneau_de_case(ui, "zoom", &resp, 190.0, |app, ui| {
            ui.horizontal(|ui| {
                if app.bouton(ui, ph::MAGNIFYING_GLASS_MINUS, "Zoom arrière", "", "vue.zoom_arriere").clicked() {
                    app.run_action("vue.zoom_arriere");
                }
                let mut pct = (app.view.zoom * app.ppp * 100.0).clamp(1.0, 6400.0);
                let avant = pct;
                let r = ui.add(egui::DragValue::new(&mut pct).range(1.0..=6400.0).speed(1.0).max_decimals(0).suffix(" %"));
                if crate::bulles::bulle(r, "Zoom", "Glisser, ou double-clic pour taper un pourcentage.", "").changed() && avant > 0.0 {
                    app.zoom_by(pct / avant);
                }
                if app.bouton(ui, ph::MAGNIFYING_GLASS_PLUS, "Zoom avant", "", "vue.zoom_avant").clicked() {
                    app.run_action("vue.zoom_avant");
                }
            });
            ui.horizontal(|ui| {
                if ui.button("100 %").on_hover_text("Un pixel du dessin pour un pixel de l'écran.").clicked() {
                    app.run_action("vue.reinit_zoom");
                }
                if ui.button("Ajuster").on_hover_text("Toute la feuille dans la vue.").clicked() {
                    app.run_action("vue.ajuster");
                }
            });
        });
    }

    /// La rotation de la vue : une icône (allumée quand la vue est tournée), et au clic l'angle, un quart de
    /// tour de chaque côté et « Droite ».
    fn rotation_reglage(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        let tournee = self.view.rotation.to_degrees().abs() > 0.05;
        let resp = self.case_icone(ui, ph::ARROW_ARC_RIGHT, Some(tournee), "Rotation de la vue", "Tourner la vue pour dessiner plus à l'aise ; le dessin ne change pas.", "");
        self.panneau_de_case(ui, "rotation", &resp, 190.0, |app, ui| {
            ui.horizontal(|ui| {
                if app.bouton(ui, ph::ARROW_ARC_LEFT, "Tourner la vue", "De 15° dans le sens antihoraire ; le dessin ne change pas.", "vue.rotation_antihoraire").clicked() {
                    app.run_action("vue.rotation_antihoraire");
                }
                let mut degrees = app.view.rotation.to_degrees();
                let r = ui.add(egui::DragValue::new(&mut degrees).range(-180.0..=180.0).speed(0.5).max_decimals(0).suffix("°"));
                if crate::bulles::bulle(r, "Angle de la vue", "Glisser, ou double-clic pour taper un angle.", "").changed() {
                    let delta = degrees.to_radians() - app.view.rotation;
                    app.rotate_view(delta.to_degrees());
                }
                if app.bouton(ui, ph::ARROW_ARC_RIGHT, "Tourner la vue", "De 15° dans le sens horaire ; le dessin ne change pas.", "vue.rotation_horaire").clicked() {
                    app.run_action("vue.rotation_horaire");
                }
            });
            if ui.button("Remettre droite").on_hover_text("La vue retrouve l'angle zéro.").clicked() {
                app.run_action("vue.reinit_rotation");
            }
        });
    }

    /// La pelure d'oignon : l'icône l'allume et l'éteint, la petite flèche à côté ouvre combien de dessins avant et
    /// après, et la fenêtre de tous ses réglages.
    fn pelure_reglage(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        self.command_button(ui, ph::CIRCLES_THREE, "vue.pelure", "Pelure d'oignon", "Par défaut, les dessins d'avant en rouge, ceux d'après en vert.", Some(self.onion.enabled));
        let resp = self.petite_fleche(ui, "Réglages de la pelure d'oignon", "Combien de dessins avant et après, et tous ses réglages.");
        self.panneau_de_case(ui, "pelure", &resp, 170.0, |app, ui| {
            let max = crate::onion::MAX_NEIGHBOURS;
            ui.horizontal(|ui| {
                ui.label("Avant");
                let r = ui.add(egui::DragValue::new(&mut app.onion.before).range(0..=max).prefix("‹ "));
                crate::bulles::bulle(r, "Avant", "Nombre de dessins (ou d'images) montrés avant.", "");
            });
            ui.horizontal(|ui| {
                ui.label("Après");
                let r = ui.add(egui::DragValue::new(&mut app.onion.after).range(0..=max).suffix(" ›"));
                crate::bulles::bulle(r, "Après", "Nombre de dessins (ou d'images) montrés après.", "");
            });
            let key = app.raccourci("pelure.panneau");
            let r = ui.button(format!("{}  Tous les réglages…", ph::GEAR));
            if crate::bulles::bulle(r, "Réglages de la pelure d'oignon", "Mode, devant, couleurs, opacité de chaque image, réglages rangés, décalage.", &key).clicked() {
                app.onion_window = !app.onion_window;
            }
        });
    }

    /// La table lumineuse : l'icône l'allume et l'éteint, la petite flèche à côté règle la force des autres calques.
    fn table_lumineuse_reglage(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        self.command_button(ui, ph::LIGHTBULB, "vue.table_lumineuse", "Table lumineuse", "Les autres calques du plan estompés ; le calque courant reste net.", Some(self.light_table));
        let resp = self.petite_fleche(ui, "Réglage de la table lumineuse", "Combien les autres calques restent visibles.");
        self.panneau_de_case(ui, "table_lumineuse", &resp, 190.0, |app, ui| {
            ui.horizontal(|ui| {
                ui.label("Autres calques");
                let r = ui.add(egui::Slider::new(&mut app.light_table_strength, 0.05..=0.8).custom_formatter(|v, _| format!("{:.0} %", v * 100.0)));
                crate::bulles::bulle(r, "Autres calques", "Combien ils restent visibles.", "");
            });
        });
    }

    /// Le passe-partout autour du cadre de la caméra (Anthony, 2026-10-24) : l'icône l'allume et l'éteint (la commande
    /// `camera.masque`, avec « Assigner un raccourci » au clic droit), et à côté un petit curseur règle son opacité, en pour
    /// cent (100 % d'origine : opaque). La couleur (noir d'origine) est dans Préférences > Dessin. Les deux sont retenus dans
    /// les réglages du poste.
    pub(crate) fn passe_partout_reglage(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        let on = self.settings.passe_partout_actif;
        self.command_button(ui, ph::SELECTION_INVERSE, "camera.masque", "Passe-partout", "Couvrir ce qui est hors du cadre de la caméra, noir d'origine (la couleur se règle dans Préférences > Dessin).", Some(on));
        let mut pct = f32::from(self.settings.passe_partout);
        let resp = ui
            .scope(|ui| {
                ui.spacing_mut().slider_width = 64.0;
                ui.add(egui::Slider::new(&mut pct, 0.0..=100.0).suffix(" %").fixed_decimals(0))
            })
            .inner;
        let resp = crate::bulles::bulle(resp, "Opacité du passe-partout", "À quel point le passe-partout cache le hors-champ : 100 %, opaque ; 0 %, rien n'est couvert.", "");
        if resp.changed() {
            self.settings.passe_partout = pct.round().clamp(0.0, 100.0) as u8;
        }
        if resp.drag_stopped() || (resp.changed() && !resp.dragged()) {
            let _ = self.settings.save();
        }
    }

    /// Une petite flèche collée à une icône, qui ouvre ses réglages fins.
    fn petite_fleche(&self, ui: &mut egui::Ui, nom: &str, description: &str) -> egui::Response {
        let text = egui::RichText::new(egui_phosphor::regular::CARET_UP).size(10.0);
        let resp = ui.add_sized([12.0, crate::barres::CASE[1]], egui::Button::new(text).frame(false));
        crate::bulles::bulle(resp, nom, description, "")
    }

    /// Multiplie le zoom de la vue par `factor`, autour du centre du dessin.
    pub(crate) fn zoom_by(&mut self, factor: f32) {
        let (rect, size) = (self.canvas_rect, self.canvas_size());
        self.view.zoom_around(rect, size, rect.center(), factor);
    }

    /// Vue caméra (`on`) ou retour à la vue du plateau telle qu'on l'avait laissée.
    pub(crate) fn set_camera_view(&mut self, on: bool) {
        if on == self.camera_view {
            return;
        }
        if on {
            self.stage_view = Some(self.view);
        } else {
            match self.stage_view.take() {
                Some(v) => self.view = v,
                None => self.fit_pending = true,
            }
        }
        self.camera_view = on;
        self.status = format!("Vue {}.", if on { "caméra" } else { "du plateau" });
    }

    /// La barre au-dessus du dessin : les trois vues (le plateau, ce que filme la caméra, l'Espace 3D) et la règle des
    /// guides de perspective. Le format, les zones de sécurité et les repères de composition sont à gauche de la barre
    /// sous le dessin (Cadrage et l'icône des repères). En Espace 3D, ses outils.
    pub(crate) fn view_bar(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        ui.painter().rect_filled(ui.max_rect(), 0.0, crate::theme::entete());
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing = vec2(crate::barres::ECART, 0.0);
            ui.add_space(4.0);
            let cam = self.camera_view;
            let in3d = self.espace3d.on && !cam;
            if self.case_icone(ui, ph::BOUNDING_BOX, Some(!cam && !in3d), "Vue du plateau", "Toute la feuille, avec le cadre de la caméra.", "camera.vue").clicked() {
                if self.espace3d.on {
                    self.set_espace3d(false);
                }
                self.set_camera_view(false);
            }
            if self.case_icone(ui, ph::VIDEO_CAMERA, Some(cam), "Vue caméra", "Ce que filme la caméra du plan.", "camera.vue").clicked() {
                self.set_camera_view(true);
            }
            if self
                .case_icone(
                    ui,
                    ph::CUBE,
                    Some(in3d),
                    "Espace 3D",
                    "Les calques (cartes) et la caméra du plan dans un seul espace : les déplacer, les tourner, les mettre à l'échelle, pour la profondeur et la parallaxe.",
                    "espace3d.basculer",
                )
                .clicked()
            {
                self.run_action("espace3d.basculer");
            }
            ui.add_space(crate::barres::MARGE_TRAIT);
            ui.separator();
            ui.add_space(crate::barres::MARGE_TRAIT);
            if in3d {
                self.espace3d_tools(ui);
                return;
            }
            self.guides_button(ui);
            // Les pastilles (classique, personnages, couleurs neutres, brosses du décor, « + », panneau Brosses dédiées) : à l'extrémité
            // droite de la barre, rangée de cases de la même hauteur et des mêmes écarts que les icônes ; trop étroite, elle défile.
            self.pastilles_rangee(ui);
        });
    }

    /// Choix du cadre de format : aucun, l'un des formats proposés, ou un format personnalisé.
    pub(crate) fn frame_format_ui(&mut self, ui: &mut egui::Ui) {
        let current = self.layout.frame_format.and_then(frames::sanitized);
        // (Un format qui n'est pas proposé, retenu d'une séance précédente, est personnalisé.)
        let custom = current.is_some_and(|r| self.format_custom || !frames::is_preset(r));
        let shown = match current {
            None => "Aucun".to_string(),
            Some(_) if custom => "Personnalisé".to_string(),
            Some(r) => frames::label(r),
        };
        let resp = egui::ComboBox::from_id_salt(ui.id().with("format_cadre"))
            .selected_text(format!("{} {shown}", egui_phosphor::regular::CROP))
            .width(96.0)
            .show_ui(ui, |ui| {
                if ui.selectable_label(current.is_none(), "Aucun").clicked() {
                    self.layout.frame_format = None;
                    self.format_custom = false;
                }
                for r in frames::PRESETS {
                    if ui.selectable_label(current == Some(r) && !custom, frames::label(r)).clicked() {
                        self.layout.frame_format = Some(r);
                        self.format_custom = false;
                    }
                }
                if ui.selectable_label(custom, "Personnalisé…").clicked() {
                    self.layout.frame_format = Some(current.unwrap_or([2.39, 1.0]));
                    self.format_custom = true;
                }
            })
            .response;
        crate::bulles::bulle(resp, "Format", "Cadre d'un format de diffusion, tracé dans le cadre de la caméra (hors-format estompé).", "");
        if let (true, Some(mut r)) = (custom, current) {
            fn drag(v: &mut f32) -> egui::DragValue<'_> {
                egui::DragValue::new(v).range(0.01..=1000.0).speed(0.01).max_decimals(2)
            }
            let a = ui.add(drag(&mut r[0])).on_hover_text("Largeur relative");
            ui.label(":");
            let b = ui.add(drag(&mut r[1])).on_hover_text("Hauteur relative");
            if a.changed() || b.changed() {
                self.layout.frame_format = frames::sanitized(r);
            }
        }
    }

    /// Cadres de composition par-dessus l'image : cadre du format (hors-format estompé) et zones
    /// de sécurité (dans le cadre du format s'il y en a un, sinon dans celui de la caméra). Ils
    /// suivent la caméra : position, zoom, rotation.
    pub(crate) fn paint_frame_overlays(&self, painter: &egui::Painter, rect: Rect) {
        let format = self.layout.frame_format.and_then(frames::sanitized);
        if format.is_none() && !self.layout.safe_frames && !self.cadrage.any_on() {
            return;
        }
        let (cam, frame, size) = (self.camera_montree(), self.doc.frame, self.canvas_size());
        let quad = |r: Rect| {
            [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()]
                .map(|p| self.view.to_screen(rect, size, cam.to_stage(frame, p.x, p.y)))
        };
        let full = Rect::from_min_size(Pos2::ZERO, vec2(frame[0] as f32, frame[1] as f32));
        let mut inside = full;
        if let Some(ratio) = format {
            let r = frames::format_rect(frame, ratio);
            let q = quad(r);
            painter.add(band(quad(full), q, Color32::from_black_alpha(140)));
            painter.add(Shape::closed_line(q.to_vec(), Stroke::new(3.0_f32, Color32::from_black_alpha(90))));
            // (Sans étiquette « 16:9 » dans le champ, Anthony 2026-10-28 : le choix est dans Cadrage.)
            let [r_, g_, b_] = self.settings.cadre_format_couleur;
            painter.add(Shape::closed_line(q.to_vec(), Stroke::new(1.5_f32, Color32::from_rgb(r_, g_, b_))));
            inside = r;
        }
        // Aides au cadrage (`cadrage.rs`) : tiers, grille d'or, spirale, diagonales, croix, dans le
        // cadre du format s'il y en a un.
        self.paint_framing_aids(painter, inside, &|p| self.view.to_screen(rect, size, cam.to_stage(frame, p.x, p.y)));
        if self.layout.safe_frames {
            let (action, title) = self.cadrage.safe.parts();
            for (part, name, corner) in [(action, "action", 0), (title, "titres", 3)] {
                let q = quad(frames::safe_rect(inside, part));
                let pts = vec![q[0], q[1], q[2], q[3], q[0]];
                painter.add(Shape::line(pts.clone(), Stroke::new(2.5_f32, veil(40))));
                painter.extend(Shape::dashed_line(&pts, Stroke::new(1.2_f32, SAFE), 6.0, 4.0));
                let (at, align) = if corner == 0 {
                    (q[0] + vec2(3.0, 2.0), Align2::LEFT_TOP)
                } else {
                    (q[3] + vec2(3.0, -2.0), Align2::LEFT_BOTTOM)
                };
                painter.text(at, align, name, FontId::proportional(10.0), SAFE);
            }
            // Croix au centre.
            let c = quad(Rect::from_center_size(inside.center(), Vec2::ZERO))[0];
            let s = Stroke::new(1.0_f32, SAFE);
            painter.line_segment([c - vec2(7.0, 0.0), c + vec2(7.0, 0.0)], s);
            painter.line_segment([c - vec2(0.0, 7.0), c + vec2(0.0, 7.0)], s);
        }
    }
}
