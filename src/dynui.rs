//! Panneau des connexions d'une brosse (moteur de dessin, étape 2) : pour la taille, l'opacité,
//! l'angle et la rondeur, la source suivie, « Inverser », la réponse minimale et la courbe de
//! réponse, que l'on modifie à la souris (glisser un point ; double-clic : ajouter ; clic droit :
//! retirer).

use eframe::egui::{self, pos2, vec2, Color32, Rect, Sense, Shape, Stroke};

use crate::brush::{BrushSettings, Connection, Curve, Source};
use crate::bulles::bulle;
use crate::theme;

/// Taille de l'éditeur de courbe.
const CURVE_W: f32 = 132.0;
const CURVE_H: f32 = 84.0;

/// La section « Forme et dispersion » (moteur de dessin, étape 6) : rondeur et angle de la
/// pointe, hasards de taille et d'angle, dispersion des empreintes, nombre par pas, retournements
/// d'une pointe en image.
pub(crate) fn shape_ui(ui: &mut egui::Ui, s: &mut BrushSettings) {
    let used = s.roundness < 1.0 || s.angle != 0.0 || s.angle_jitter > 0.0 || s.size_jitter > 0.0 || s.scatter > 0.0 || s.scatter_along > 0.0 || s.count > 1 || s.flip_x || s.flip_y;
    egui::CollapsingHeader::new("Forme et dispersion").id_salt("forme_brosse").default_open(used).show(ui, |ui| {
        let resp = ui.add(egui::Slider::new(&mut s.roundness, 0.05..=1.0).text("Rondeur"));
        bulle(resp, "Rondeur", "Pointe aplatie (petit) ou ronde (1).", "");
        let resp = ui.add(egui::Slider::new(&mut s.angle, -180.0..=180.0).suffix("°").text("Angle"));
        bulle(resp, "Angle", "Angle de la pointe aplatie ou en image.", "");
        let resp = ui.add(egui::Slider::new(&mut s.angle_jitter, 0.0..=360.0).suffix("°").text("Angle au hasard"));
        bulle(resp, "Angle au hasard", "Chaque empreinte tourne au hasard dans cet écart.", "");
        let resp = ui.add(egui::Slider::new(&mut s.size_jitter, 0.0..=1.0).text("Taille au hasard"));
        bulle(resp, "Taille au hasard", "Chaque empreinte grandit ou rapetisse au hasard, jusqu'à cette part de la taille.", "");
        let resp = ui.add(egui::Slider::new(&mut s.scatter, 0.0..=4.0).text("Dispersion"));
        bulle(resp, "Dispersion", "Chaque empreinte s'écarte au hasard en travers du trait, jusqu'à ce nombre de diamètres.", "");
        let resp = ui.add(egui::Slider::new(&mut s.scatter_along, 0.0..=4.0).text("Dispersion le long"));
        bulle(resp, "Dispersion le long", "Chaque empreinte s'écarte au hasard le long du trait.", "");
        let resp = ui.add(egui::Slider::new(&mut s.count, 1..=16).text("Nombre"));
        bulle(resp, "Nombre", "Empreintes posées à chaque pas, chacune dispersée à part.", "");
        if s.tip.is_some() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut s.flip_x, "Retourner au hasard ↔").on_hover_text("La pointe en image est retournée horizontalement une empreinte sur deux, au hasard.");
                ui.checkbox(&mut s.flip_y, "↕").on_hover_text("La pointe en image est retournée verticalement, au hasard.");
            });
        }
    });
}

/// La section « Peinture humide » (moteur de dessin, étape 11) : mélange avec la couleur déjà
/// posée, et charge de la brosse.
pub(crate) fn wet_ui(ui: &mut egui::Ui, s: &mut BrushSettings, vector: bool) {
    let used = s.mix > 0.0 || s.charge < 1.0;
    egui::CollapsingHeader::new("Peinture humide").id_salt("humide_brosse").default_open(used).show(ui, |ui| {
        let resp = ui.add(egui::Slider::new(&mut s.mix, 0.0..=1.0).text("Mélange"));
        bulle(resp, "Mélange", "La brosse prend la couleur déjà posée sous elle, la mêle à la sienne et l'entraîne (0 : peinture qui couvre, comme avant).", "");
        let resp = ui.add(egui::Slider::new(&mut s.charge, 0.0..=1.0).text("Charge"));
        bulle(resp, "Charge", "La peinture dans la brosse : 1, elle ne s'épuise pas ; moins, elle s'épuise le long du trait, puis la brosse ne fait plus qu'étaler.", "");
        ui.label(
            egui::RichText::new(if vector {
                "Calque vectoriel : la peinture humide ne vaut que pendant le tracé ; elle est faite pour un calque bitmap."
            } else {
                "Modèle de Pelure (la formule de TVPaint n'est pas publiée) : à comparer avec tes outils de peinture."
            })
            .small()
            .weak(),
        );
    });
}

/// La section « Connexions » des réglages d'une brosse. `vector` : calque vectoriel, où le trait
/// gardé ne retient que la taille.
pub(crate) fn dynamics_ui(ui: &mut egui::Ui, s: &mut BrushSettings, vector: bool) {
    let n = [s.dynamics.size, s.dynamics.opacity, s.dynamics.angle, s.dynamics.roundness].iter().filter(|c| c.active()).count();
    let title = if n > 0 { format!("Connexions ({n})") } else { "Connexions".to_string() };
    egui::CollapsingHeader::new(title).id_salt("connexions_brosse").default_open(n > 0).show(ui, |ui| {
        ui.label(
            egui::RichText::new("Chaque réglage peut suivre l'appui, la vitesse, la direction du trait, un fondu ou le hasard, par une courbe (comme les connexions de TVPaint).")
                .small()
                .weak(),
        );
        let legacy_size = s.pressure_size;
        let legacy_opacity = s.pressure_opacity;
        connection_row(ui, "Taille", "taille", &mut s.dynamics.size, legacy_size.then_some(s.min_size));
        connection_row(ui, "Opacité", "opacite", &mut s.dynamics.opacity, legacy_opacity.then_some(s.min_opacity));
        connection_row(ui, "Angle", "angle", &mut s.dynamics.angle, None);
        connection_row(ui, "Rondeur", "rondeur", &mut s.dynamics.roundness, None);
        if s.dynamics.uses(Source::Speed) {
            let resp = ui.add(egui::Slider::new(&mut s.dynamics.speed_max, 100.0..=8000.0).logarithmic(true).suffix(" px/s").text("Vitesse max"));
            bulle(resp, "Vitesse max", "Vitesse du trait à laquelle la source « Vitesse » est au plus haut.", "");
        }
        if s.dynamics.uses(Source::Fade) {
            let resp = ui.add(egui::Slider::new(&mut s.dynamics.fade_length, 10.0..=3000.0).logarithmic(true).suffix(" px").text("Longueur du fondu"));
            bulle(resp, "Longueur du fondu", "Longueur tracée au bout de laquelle le fondu est complet.", "");
        }
        if vector && n > 0 {
            ui.label(
                egui::RichText::new("Calque vectoriel : le trait gardé retient la taille ; l'opacité, l'angle et la rondeur connectés ne valent que pour un calque bitmap.")
                    .small()
                    .weak(),
            );
        }
    });
}

/// Une ligne : le réglage, sa source ; puis, si elle est choisie, « Inverser », la réponse
/// minimale et la courbe. `legacy` : la pression simple déjà réglée (son minimum), reprise quand
/// on choisit une source.
fn connection_row(ui: &mut egui::Ui, label: &str, id: &str, c: &mut Connection, legacy: Option<f32>) {
    ui.horizontal(|ui| {
        ui.add_sized(vec2(56.0, 18.0), egui::Label::new(label));
        let before = c.source;
        egui::ComboBox::from_id_salt(("source", id)).selected_text(c.source.label()).width(96.0).show_ui(ui, |ui| {
            for s in Source::ALL {
                ui.selectable_value(&mut c.source, s, s.label()).on_hover_text(s.help());
            }
        });
        // Première source choisie : on part de la pression simple (même minimum).
        if before == Source::None && c.active() {
            if let Some(min) = legacy {
                c.min = min;
            }
        }
        if c.active() {
            ui.checkbox(&mut c.invert, "Inverser").on_hover_text("Plus la source est forte, plus le réglage baisse.");
        }
    });
    if !c.active() {
        if legacy.is_some() && id == "taille" {
            note(ui, "La taille suit déjà l'appui (bouton à côté du curseur Taille) ; une source ici la remplace.");
        }
        if legacy.is_some() && id == "opacite" {
            note(ui, "L'opacité suit déjà l'appui (bouton à côté du curseur Opacité) ; une source ici la remplace.");
        }
        return;
    }
    ui.horizontal(|ui| {
        ui.add_space(60.0);
        let resp = curve_editor(ui, &mut c.curve, id);
        bulle(resp, "Courbe de réponse", "Glisser un point ; double-clic : ajouter un point ; clic droit sur un point : le retirer.", "");
        ui.vertical(|ui| {
            let resp = ui.add(egui::Slider::new(&mut c.min, 0.0..=1.0).text("Mini"));
            bulle(resp, "Réponse minimale", "Ce que vaut le réglage quand la courbe est en bas, en part de sa valeur.", "");
            if ui.small_button("Droite").on_hover_text("Revenir à la courbe droite.").clicked() {
                c.curve = Curve::linear();
            }
        });
    });
}

fn note(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).small().weak());
}

/// L'éditeur d'une courbe de réponse : entrée en x (de gauche à droite), réponse en y (de bas en
/// haut). Les bouts restent aux bords gauche et droit ; un point reste entre ses voisins.
pub(crate) fn curve_editor(ui: &mut egui::Ui, curve: &mut Curve, id: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(CURVE_W, CURVE_H), Sense::click_and_drag());
    let inner = rect.shrink(5.0);
    let to_screen = |p: [f32; 2]| pos2(inner.left() + p[0] * inner.width(), inner.bottom() - p[1] * inner.height());
    let to_curve = |q: egui::Pos2| [((q.x - inner.left()) / inner.width()).clamp(0.0, 1.0), ((inner.bottom() - q.y) / inner.height()).clamp(0.0, 1.0)];
    let drag_id = ui.id().with(("courbe_point", id));
    let mut pts = curve.points().to_vec();
    let near = |pts: &[[f32; 2]], q: egui::Pos2| pts.iter().position(|p| to_screen(*p).distance(q) <= 7.0);

    // Glisser un point (le plus proche du clic).
    if resp.drag_started() {
        let held = resp.interact_pointer_pos().and_then(|q| near(&pts, q));
        ui.data_mut(|d| d.insert_temp(drag_id, held));
    }
    let held: Option<usize> = ui.data(|d| d.get_temp(drag_id)).flatten();
    if let (Some(i), Some(q)) = (held, resp.interact_pointer_pos().filter(|_| resp.dragged())) {
        let mut p = to_curve(q);
        let last = pts.len() - 1;
        if i == 0 {
            p[0] = 0.0;
        } else if i == last {
            p[0] = 1.0;
        } else {
            p[0] = p[0].clamp(pts[i - 1][0] + 0.01, pts[i + 1][0] - 0.01);
        }
        pts[i] = p;
        *curve = Curve::from_points(&pts);
    }
    if resp.drag_stopped() {
        ui.data_mut(|d| d.remove::<Option<usize>>(drag_id));
    }
    // Double-clic : un point de plus ; clic droit sur un point (pas un bout) : retiré.
    if resp.double_clicked() && pts.len() < Curve::MAX {
        if let Some(q) = resp.interact_pointer_pos() {
            pts.push(to_curve(q));
            *curve = Curve::from_points(&pts);
        }
    }
    if resp.secondary_clicked() {
        if let Some(i) = resp.interact_pointer_pos().and_then(|q| near(&pts, q)).filter(|&i| i > 0 && i + 1 < pts.len()) {
            pts.remove(i);
            *curve = Curve::from_points(&pts);
        }
    }

    // Dessin : fond, quadrillage, la courbe, ses points.
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3.0, theme::RANGEE_SOMBRE);
    let grid = Stroke::new(1.0_f32, theme::GRADUATION);
    for k in 1..4 {
        let t = k as f32 / 4.0;
        painter.line_segment([pos2(inner.left() + t * inner.width(), inner.top()), pos2(inner.left() + t * inner.width(), inner.bottom())], grid);
        painter.line_segment([pos2(inner.left(), inner.bottom() - t * inner.height()), pos2(inner.right(), inner.bottom() - t * inner.height())], grid);
    }
    let line: Vec<egui::Pos2> = curve.points().iter().map(|p| to_screen(*p)).collect();
    painter.add(Shape::line(line, Stroke::new(1.5_f32, theme::IMAGE_COURANTE)));
    let hover = resp.hover_pos().and_then(|q| near(curve.points(), q));
    for (i, p) in curve.points().iter().enumerate() {
        let lit = Some(i) == held || Some(i) == hover;
        painter.circle(to_screen(*p), if lit { 4.5 } else { 3.5 }, if lit { Color32::WHITE } else { theme::IMAGE_COURANTE }, Stroke::new(1.0_f32, theme::FOND));
    }
    if hover.is_some() || held.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    let _ = Rect::NOTHING;
    resp
}
