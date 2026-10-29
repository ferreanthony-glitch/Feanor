//! Légende et filtre des clés (simplification des barres, 2026-10-21). Dans le montage et la timeline, chaque sorte de
//! clé a sa couleur : le début d'un dessin, le losange jaune de la caméra, le violet d'un effet, le vert d'une
//! position, la transition entre deux plans. Une petite icône « ? » au bout de l'en-tête de la bande les rappelle au
//! survol ; ses cases cachent les sortes dont on n'a pas besoin pour l'instant (la caméra quand on dessine, les effets
//! quand on cale un mouvement…).
//!
//! Le filtre est un AFFICHAGE : une sorte cachée n'est ni dessinée ni saisie à la souris (on ne prend pas ce qu'on ne
//! voit pas), mais le projet ne change pas, rien n'est supprimé, et la lecture et l'export montrent tout. Il est
//! retenu dans les réglages de l'application (`Settings::key_filter`), pas dans le projet. Les clés de dessin sont le
//! contenu même des pistes : elles ne se cachent pas, la légende les rappelle.

use eframe::egui::{self, pos2, vec2, Align2, Color32, Rect, RichText, Sense, Shape, Stroke};
use egui_phosphor::regular as ph;
use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::theme;

/// Les sortes de clés que la légende nomme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Sorte {
    /// Le début d'un dessin sur la piste d'un calque.
    Dessin,
    /// Le losange jaune de la piste Caméra.
    Camera,
    /// Le losange violet d'un effet.
    Effet,
    /// Le losange vert de la position d'un calque.
    Position,
    /// Une transition entre deux plans (fondu, volet, noir).
    Transition,
    /// Le losange orange du bas de la piste d'un calque : un point de l'opacité du calque (`opacitecles.rs`).
    Opacite,
}

impl Sorte {
    pub(crate) const TOUTES: [Sorte; 6] = [Sorte::Dessin, Sorte::Camera, Sorte::Effet, Sorte::Position, Sorte::Opacite, Sorte::Transition];

    pub(crate) fn nom(self) -> &'static str {
        match self {
            Sorte::Dessin => "Clé de dessin",
            Sorte::Camera => "Clé de caméra",
            Sorte::Effet => "Clé d'effet",
            Sorte::Position => "Clé de position",
            Sorte::Opacite => "Clé d'opacité",
            Sorte::Transition => "Transition",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Sorte::Dessin => "Le début d'un dessin sur la piste d'un calque. Toujours visible.",
            Sorte::Camera => "Losange jaune de la piste Caméra : un point du mouvement de caméra.",
            Sorte::Effet => "Losange violet en haut de la piste du calque : un point de l'effet choisi.",
            Sorte::Position => "Losange vert au milieu de la piste : un point du déplacement du calque.",
            Sorte::Opacite => "Losange orange en bas de la piste : un point de l'opacité du calque (il apparaît, s'efface).",
            Sorte::Transition => "Entre deux plans : fondu enchaîné, volet, fondu au noir.",
        }
    }

    /// Sa couleur sur la bande.
    pub(crate) fn couleur(self) -> Color32 {
        match self {
            Sorte::Dessin => theme::fond_calque(crate::document::LayerKind::Vector, false),
            Sorte::Camera => theme::camera(),
            Sorte::Effet => crate::fxkeys::color(),
            Sorte::Position => theme::POIGNEE_CALQUE,
            Sorte::Opacite => theme::CLE_OPACITE,
            Sorte::Transition => theme::accent_texte(),
        }
    }

    /// Peut-on la cacher ? Les clés de dessin sont le contenu des pistes : non.
    pub(crate) fn masquable(self) -> bool {
        self != Sorte::Dessin
    }
}

/// Quelles sortes de clés la bande montre (vrai : montrée). Tout est montré d'office.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FiltreCles {
    #[serde(rename = "camera")]
    pub camera: bool,
    #[serde(rename = "effets")]
    pub effets: bool,
    #[serde(rename = "positions")]
    pub positions: bool,
    #[serde(rename = "transitions")]
    pub transitions: bool,
    #[serde(rename = "opacites")]
    pub opacites: bool,
}

impl Default for FiltreCles {
    fn default() -> Self {
        FiltreCles { camera: true, effets: true, positions: true, transitions: true, opacites: true }
    }
}

impl FiltreCles {
    /// Cette sorte est-elle montrée ?
    pub(crate) fn montre(&self, s: Sorte) -> bool {
        match s {
            Sorte::Dessin => true,
            Sorte::Camera => self.camera,
            Sorte::Effet => self.effets,
            Sorte::Position => self.positions,
            Sorte::Opacite => self.opacites,
            Sorte::Transition => self.transitions,
        }
    }

    /// Montre ou cache cette sorte (les clés de dessin ne se cachent pas).
    pub(crate) fn regle(&mut self, s: Sorte, vue: bool) {
        match s {
            Sorte::Dessin => {}
            Sorte::Camera => self.camera = vue,
            Sorte::Effet => self.effets = vue,
            Sorte::Position => self.positions = vue,
            Sorte::Opacite => self.opacites = vue,
            Sorte::Transition => self.transitions = vue,
        }
    }

    /// Tout est montré (le réglage d'origine, qui n'écrit rien dans le fichier).
    pub fn est_complet(&self) -> bool {
        *self == FiltreCles::default()
    }
}

/// La pastille d'une sorte, dessinée dans `r` : un rectangle (un dessin), un losange (une clé), deux triangles (une
/// transition).
fn pastille(painter: &egui::Painter, r: Rect, s: Sorte) {
    let (c, color) = (r.center(), s.couleur());
    let contour = Stroke::new(1.0_f32, Color32::from_black_alpha(140));
    match s {
        Sorte::Dessin => {
            // Les dessins d'un calque vectoriel (bleu) et d'un calque bitmap (vert), comme dans les pistes.
            let (h, w) = (r.height() * 0.6, r.width() * 0.5);
            let a = Rect::from_min_size(pos2(r.left(), c.y - h * 0.5), vec2(w - 1.0, h));
            let b = Rect::from_min_size(pos2(r.left() + w + 1.0, c.y - h * 0.5), vec2(w - 1.0, h));
            painter.rect_filled(a, 2.0, color);
            painter.rect_filled(b, 2.0, theme::fond_calque(crate::document::LayerKind::Bitmap, false));
        }
        Sorte::Transition => {
            let (w, h) = (r.width() * 0.5, r.height() * 0.5);
            painter.add(Shape::convex_polygon(vec![pos2(c.x - w, c.y - h), pos2(c.x, c.y), pos2(c.x - w, c.y + h)], color, contour));
            painter.add(Shape::convex_polygon(vec![pos2(c.x + w, c.y - h), pos2(c.x, c.y), pos2(c.x + w, c.y + h)], color, contour));
        }
        _ => {
            let d = r.width() * 0.5;
            painter.add(Shape::convex_polygon(vec![pos2(c.x, c.y - d), pos2(c.x + d, c.y), pos2(c.x, c.y + d), pos2(c.x - d, c.y)], color, contour));
        }
    }
}

impl App {
    /// Cette sorte de clé est-elle montrée sur la bande (filtre d'affichage) ?
    pub(crate) fn cle_vue(&self, s: Sorte) -> bool {
        self.settings.key_filter.montre(s)
    }

    /// L'icône « ? » au bout de l'en-tête du montage et de la timeline : au survol, la légende des clés avec une case par
    /// sorte à cacher. Allumée quand une sorte est cachée (pour ne pas chercher longtemps une clé qu'on a cachée).
    pub(crate) fn legende_cles_ui(&mut self, ui: &mut egui::Ui) {
        let filtre_actif = !self.settings.key_filter.est_complet();
        let text = RichText::new(ph::QUESTION).size(crate::barres::ICONE);
        let resp = ui.add_sized(crate::barres::CASE, egui::SelectableLabel::new(filtre_actif, text));
        let id = egui::Id::new("legende_des_cles");
        let ctx = ui.ctx().clone();
        // Ouverte tant que la souris est sur l'icône ou sur la légende (qui touche l'icône : on peut y aller sans la
        // perdre) ; la place de la légende est celle de l'image d'avant.
        let rect_avant: Option<Rect> = ctx.data(|d| d.get_temp(id));
        let pointeur = ctx.input(|i| i.pointer.hover_pos());
        let sur_la_legende = pointeur.zip(rect_avant).is_some_and(|(p, r)| r.expand(6.0).contains(p));
        if !(resp.hovered() || sur_la_legende) {
            ctx.data_mut(|d| d.remove::<Rect>(id));
            return;
        }
        let mut change = false;
        let mut filtre = self.settings.key_filter;
        let zone = egui::Area::new(id.with("zone"))
            .order(egui::Order::Foreground)
            .fixed_pos(resp.rect.right_bottom())
            .pivot(Align2::RIGHT_TOP)
            .show(&ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(300.0);
                    ui.label(RichText::new("Les clés").strong());
                    for s in Sorte::TOUTES {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                            pastille(ui.painter(), r, s);
                            if s.masquable() {
                                let mut vue = filtre.montre(s);
                                let case = ui.checkbox(&mut vue, s.nom());
                                if crate::bulles::bulle(case, s.nom(), s.description(), "").changed() {
                                    filtre.regle(s, vue);
                                    change = true;
                                }
                            } else {
                                // (Alignée sur les autres : la place de la case qu'elle n'a pas.)
                                ui.add_space(ui.spacing().icon_width + ui.spacing().icon_spacing);
                                crate::bulles::bulle(ui.label(s.nom()), s.nom(), s.description(), "");
                            }
                        });
                    }
                    ui.label(RichText::new("Les cases cachent seulement l'affichage : rien n'est supprimé.").small().weak());
                });
            });
        ctx.data_mut(|d| d.insert_temp(id, zone.response.rect));
        if change {
            self.settings.key_filter = filtre;
            let _ = self.settings.save();
        }
    }
}
