//! Le panneau du Cutter (partie 169), dans le dessin validé par Anthony le 2026-11-18 (une première version en grandes tuiles
//! était « trop grosse et peu professionnelle… moins gadget, plus pro ») : le libellé à gauche en gris discret, le contrôle à
//! droite. Mode, geste et portée en boutons segmentés d'une ligne, texte seul ; un filet ; l'écart toléré et les bouts effilés,
//! dont le petit dessin sur toute la largeur est lui-même le curseur (on glisse la poignée sur le trait) ; en mode Dépassements
//! seulement, la longueur des dépassements en curseur ordinaire ; un filet ; « Précis », les valeurs en chiffres. Les explications
//! sont dans les bulles. Tout est peint aux couleurs du thème ; aucune image. Les réglages sont ceux du poste (`Settings::cutter`),
//! écrits dès qu'ils changent.

use eframe::egui::{self, pos2, vec2, Align, Align2, Color32, FontId, Layout, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui};

use crate::app::App;
use crate::cutter::{Geste, Mode, Portee, DEPASSEMENT_MAX, DEPASSEMENT_MIN, ECART_MAX, EFFILE_MAX};
use crate::theme;

/// Largeur de la colonne des libellés.
const LIBELLE: f32 = 62.0;
/// Hauteur d'un bouton segmenté.
const SEGMENT_H: f32 = 20.0;

fn rouge() -> Color32 {
    theme::alerte()
}

fn fin(c: Color32) -> Stroke {
    Stroke::new(1.0_f32, c)
}

fn pointilles(p: &Painter, a: Pos2, b: Pos2, c: Color32, w: f32) {
    p.extend(Shape::dashed_line(&[a, b], Stroke::new(w, c), 2.5, 2.0));
}

/// `a` mêlé de `b` à `t`.
fn teinte(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// « 3 px », « 0,5 px ».
fn px(v: f32) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{v:.0} px")
    } else {
        format!("{v:.1} px").replace('.', ",")
    }
}

// ------------------------------------------------------------------ le bouton segmenté

/// Un segment : sa valeur, son texte, le nom et l'effet de sa bulle.
type Segment<'a, T> = (T, &'a str, &'a str, &'a str);

/// Un bouton segmenté d'une ligne, texte seul, toute la largeur. Le segment choisi : un gris un peu plus clair, le texte clair et
/// un trait turquoise de 2 px dessous ; les autres restent sans fond.
fn segmente<T: PartialEq + Copy>(ui: &mut Ui, valeur: &mut T, segments: &[Segment<T>]) {
    let w = ui.available_width().max(60.0);
    let (rect, _) = ui.allocate_exact_size(vec2(w, SEGMENT_H), Sense::hover());
    let n = segments.len().max(1);
    let largeur = w / n as f32;
    for (k, (v, texte, nom, effet)) in segments.iter().enumerate() {
        let r = Rect::from_min_size(pos2(rect.left() + largeur * k as f32, rect.top()), vec2(largeur, SEGMENT_H));
        let resp = ui.interact(r, ui.id().with(("segment_cutter", *nom)), Sense::click());
        let on = *valeur == *v;
        if ui.is_rect_visible(r) {
            let p = ui.painter();
            let (premier, dernier) = (if k == 0 { 4 } else { 0 }, if k + 1 == n { 4 } else { 0 });
            let arrondi = egui::CornerRadius { nw: premier, sw: premier, ne: dernier, se: dernier };
            if on {
                p.rect_filled(r, arrondi, teinte(theme::panneau(), Color32::WHITE, 0.12));
                p.line_segment([pos2(r.left() + 1.0, r.bottom() - 1.0), pos2(r.right() - 1.0, r.bottom() - 1.0)], Stroke::new(2.0_f32, theme::rehaut()));
            } else if resp.hovered() {
                p.rect_filled(r, arrondi, theme::controle_survol());
            }
            let encre = if on { theme::texte_fort() } else { theme::texte_doux() };
            p.text(r.center(), Align2::CENTER_CENTER, *texte, FontId::proportional(11.0), encre);
            if k > 0 {
                p.line_segment([r.left_top(), r.left_bottom()], fin(theme::bord_fort()));
            }
        }
        if crate::bulles::bulle(resp, nom, effet, "").clicked() && !on {
            *valeur = *v;
        }
    }
    ui.painter().rect_stroke(rect, 4.0, fin(theme::bord_fort()), egui::StrokeKind::Inside);
}

// ------------------------------------------------------------------ les dessins qui sont des curseurs

fn poignee(p: &Painter, c: Pos2, survol: bool) {
    p.circle(c, if survol { 5.0 } else { 4.0 }, theme::texte_fort(), Stroke::new(1.5_f32, theme::panneau()));
}

/// L'écart toléré : un trait horizontal s'arrête avant un trait vertical ; on glisse son bout (la poignée) ; l'écart toléré en
/// pointillés turquoise.
fn ecart_ui(ui: &mut Ui, ecart: &mut f32) -> egui::Response {
    let w = ui.available_width().max(100.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 20.0), Sense::click_and_drag());
    let vx = rect.right() - 12.0;
    let gauche = rect.left() + 6.0;
    let echelle = (vx - 3.0 - gauche - 24.0) / ECART_MAX;
    let y = rect.center().y;
    if let Some(pt) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
        *ecart = (((vx - 3.0 - pt.x) / echelle) * 2.0).round().clamp(0.0, ECART_MAX * 2.0) / 2.0;
    }
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let c = theme::texte();
        p.line_segment([pos2(vx, rect.top() + 1.0), pos2(vx, rect.bottom() - 1.0)], Stroke::new(1.5_f32, c));
        let bout = pos2(vx - 3.0 - *ecart * echelle, y);
        p.line_segment([pos2(gauche, y), bout], Stroke::new(1.5_f32, c));
        if *ecart > 0.0 {
            pointilles(p, bout, pos2(vx, y), theme::rehaut(), 1.5);
        }
        poignee(p, bout, resp.hovered() || resp.dragged());
    }
    resp
}

/// Les bouts effilés : un trait plein dont le bout droit (repère rouge en pointillés : la coupe) s'affine ; la poignée, au début de
/// la pointe, règle la longueur.
fn effile_ui(ui: &mut Ui, effile: &mut f32) -> egui::Response {
    let w = ui.available_width().max(100.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 18.0), Sense::click_and_drag());
    let x0 = rect.left() + 6.0;
    let x1 = rect.right() - 12.0;
    let echelle = (x1 - x0 - 12.0) / EFFILE_MAX;
    let y = rect.center().y;
    if let Some(pt) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
        *effile = ((x1 - pt.x) / echelle).round().clamp(0.0, EFFILE_MAX);
    }
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let c = theme::texte();
        let rayon = 3.0;
        let l = *effile * echelle;
        let demi = |x: f32| {
            let d = x1 - x;
            if l > 0.0 && d < l {
                let t = (d / l).clamp(0.0, 1.0);
                rayon * (0.12 + 0.88 * t * (2.0 - t))
            } else {
                rayon
            }
        };
        // (Une seule bande de triangles : un trait plein, sans couture.)
        let n = 64;
        let mut mesh = egui::Mesh::default();
        for k in 0..=n {
            let x = x0 + (x1 - x0) * k as f32 / n as f32;
            mesh.colored_vertex(pos2(x, y - demi(x)), c);
            mesh.colored_vertex(pos2(x, y + demi(x)), c);
        }
        for k in 0..n as u32 {
            let i = 2 * k;
            mesh.add_triangle(i, i + 1, i + 2);
            mesh.add_triangle(i + 1, i + 3, i + 2);
        }
        p.add(Shape::mesh(mesh));
        pointilles(p, pos2(x1 + 1.5, rect.top() + 1.0), pos2(x1 + 1.5, rect.bottom() - 1.0), rouge(), 1.5);
        poignee(p, pos2(x1 - l, y), resp.hovered() || resp.dragged());
    }
    resp
}

/// Une ligne : le libellé à gauche (gris discret, colonne de 62 px), le contrôle à droite.
fn ligne(ui: &mut Ui, nom: &str, contenu: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.add_sized(vec2(LIBELLE, SEGMENT_H), egui::Label::new(egui::RichText::new(nom).size(11.0).color(theme::texte_doux())).truncate());
        contenu(ui);
    });
}

/// Le titre d'un dessin-curseur : le libellé à gauche, la valeur à droite.
fn titre_valeur(ui: &mut Ui, nom: &str, valeur: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(nom).size(11.0).color(theme::texte_doux()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(egui::RichText::new(valeur).size(11.0).color(theme::texte()));
        });
    });
}

impl App {
    /// Le panneau du Cutter.
    pub(crate) fn cutter_options_ui(&mut self, ui: &mut Ui) {
        let avant = self.settings.cutter;
        let mut r = avant;
        ui.spacing_mut().item_spacing.y = 4.0;
        ligne(ui, "Mode", |ui| {
            segmente(
                ui,
                &mut r.mode,
                &[
                    (Mode::Couper, "Couper", "Couper", "Le geste enlève les bouts de traits qu'il touche, jusqu'aux croisements."),
                    (
                        Mode::Depassements,
                        "Dépassements",
                        "Nettoyer les dépassements",
                        "Dans la zone que le geste touche ou entoure, les petits bouts qui dépassent après le dernier croisement d'un trait partent, s'ils sont plus courts que la longueur choisie. Un morceau entre deux croisements ne part jamais.",
                    ),
                ],
            )
        });
        ligne(ui, "Geste", |ui| {
            segmente(
                ui,
                &mut r.geste,
                &[
                    (Geste::Travers, "Glisser", "Glisser en travers", "On glisse en travers des bouts à enlever (ou on clique sur l'un d'eux)."),
                    (Geste::Lasso, "Lasso", "Lasso", "On entoure : les morceaux entièrement dans le lasso partent. En mode Dépassements, le lasso délimite la zone."),
                ],
            )
        });
        ligne(ui, "Portée", |ui| {
            segmente(
                ui,
                &mut r.portee,
                &[
                    (Portee::Calque, "Calque actif", "Calque actif", "On coupe contre les traits du calque actif seulement."),
                    (Portee::Visibles, "Calques visibles", "Tous les calques visibles", "Les traits des autres calques vectoriels visibles servent aussi de lignes de coupe ; ils ne sont pas modifiés."),
                ],
            )
        });
        ui.separator();
        titre_valeur(ui, "Écart toléré", &px(r.ecart));
        let resp = ecart_ui(ui, &mut r.ecart);
        crate::bulles::bulle(
            resp,
            "Écart toléré",
            "Glisse le bout du trait. Un trait qui s'arrête à moins de cet écart d'un autre compte comme s'il le croisait : l'autre trait est coupé au point d'approche. À 0, un trait qui ne touche pas n'est pas coupé.",
            "",
        );
        let effile = if r.effile > 0.0 { px(r.effile) } else { "net".into() };
        titre_valeur(ui, "Bouts effilés", &effile);
        let resp = effile_ui(ui, &mut r.effile);
        crate::bulles::bulle(resp, "Bouts effilés", "Glisse la poignée : le bout laissé par une coupe (le repère rouge) s'affine sur cette longueur. Tout à droite : net.", "");
        if r.mode == Mode::Depassements {
            titre_valeur(ui, "Dépassements enlevés jusqu'à", &px(r.depassement));
            ui.spacing_mut().slider_width = ui.available_width().max(60.0);
            let resp = ui.add(egui::Slider::new(&mut r.depassement, DEPASSEMENT_MIN..=DEPASSEMENT_MAX).show_value(false).step_by(1.0));
            crate::bulles::bulle(resp, "Dépassements enlevés jusqu'à", "Les bouts qui dépassent après le dernier croisement d'un trait, plus courts que cette longueur, partent.", "");
        }
        ui.separator();
        if crate::panneauxvisuels::precis_entete(ui, "Cutter", "Précis") {
            ui.indent("cutter_precis", |ui| {
                egui::Grid::new("cutter_precis_grille").num_columns(2).show(ui, |ui| {
                    ui.label("Écart toléré");
                    let resp = ui.add(egui::DragValue::new(&mut r.ecart).range(0.0..=ECART_MAX).speed(0.1).suffix(" px"));
                    crate::bulles::bulle(resp, "Écart toléré (px)", "En pixels du dessin. 0 : seuls les vrais croisements comptent.", "");
                    ui.end_row();
                    ui.label("Bouts effilés");
                    let resp = ui.add(egui::DragValue::new(&mut r.effile).range(0.0..=EFFILE_MAX).speed(0.2).suffix(" px"));
                    crate::bulles::bulle(resp, "Bouts effilés (px)", "Longueur de l'effilé, en pixels du dessin. 0 : net.", "");
                    ui.end_row();
                    ui.label("Dépassements jusqu'à");
                    let resp = ui.add(egui::DragValue::new(&mut r.depassement).range(DEPASSEMENT_MIN..=DEPASSEMENT_MAX).speed(0.2).suffix(" px"));
                    crate::bulles::bulle(resp, "Dépassements enlevés jusqu'à (px)", "Mode Dépassements : les bouts plus courts que cette longueur partent.", "");
                    ui.end_row();
                });
            });
        }
        if r != avant {
            self.settings.cutter = r.nettoyes();
            self.settings_a_ecrire = true;
        }
    }
}
