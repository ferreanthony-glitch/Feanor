//! Le montage redimensionnable (retour d'audit, 2026-10-30 : « le panneau de montage est très compact, la piste Son 1 est
//! coupée en bas, le bouton M/S est à peine visible »).
//!
//! - **Entre le dessin et le montage** : une poignée évidente (une petite pastille au milieu du bord, qui s'allume au
//!   survol). Le bord se tire comme avant (egui_dock, curseur de redimensionnement) ; la hauteur choisie est celle de la
//!   disposition de l'espace de travail, donc retenue par espace (`workspaceui.rs`). La bande garde une hauteur minimale
//!   (`BANDE_MIN`) et le dessin aussi (`DESSIN_MIN`) : on ne peut pas la réduire à rien.
//! - **Dans le montage** : la bande défile verticalement (jamais une piste coupée), avec une barre toujours visible
//!   quand il y a de quoi défiler ; la hauteur des pistes son se règle (petite, moyenne, grande) d'un clic dans la barre du
//!   zoom ou dans Préférences > Montage. Les boutons M, S et le verrou restent lisibles à toute hauteur.
//! - Rien de lourd : aucune écriture ni calcul pendant le glissé, juste la fraction du bord.

use std::cell::Cell;

use eframe::egui::{self, pos2, vec2, Color32, CornerRadius, Id, Order, Rect};
use egui_dock::{DockState, Node, NodeIndex};
use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::dock::Pane;

/// Hauteur au moins de la bande du montage (onglet compris), en points : la règle, la caméra et une piste de calque y
/// tiennent ; les pistes son s'atteignent en faisant défiler.
pub(crate) const BANDE_MIN: f32 = 200.0;
/// Hauteur au moins de ce qui est au-dessus de la bande (le dessin).
pub(crate) const DESSIN_MIN: f32 = 200.0;

/// La hauteur des pistes son : petite, moyenne (d'origine) ou grande.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Taille {
    Petite,
    #[default]
    Moyenne,
    Grande,
}

impl Taille {
    pub const TOUTES: [Taille; 3] = [Taille::Petite, Taille::Moyenne, Taille::Grande];

    pub fn nom(self) -> &'static str {
        match self {
            Taille::Petite => "Petite",
            Taille::Moyenne => "Moyenne",
            Taille::Grande => "Grande",
        }
    }

    /// La hauteur d'une piste son, en points.
    pub fn piste_son(self) -> f32 {
        match self {
            Taille::Petite => 34.0,
            Taille::Moyenne => crate::soundtrack::TRACK_H,
            Taille::Grande => 64.0,
        }
    }

    /// La taille d'après : petite, moyenne, grande, puis de nouveau petite.
    pub fn suivante(self) -> Taille {
        match self {
            Taille::Petite => Taille::Moyenne,
            Taille::Moyenne => Taille::Grande,
            Taille::Grande => Taille::Petite,
        }
    }
}

/// Le réglage retenu dans les réglages de l'application.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reglage {
    #[serde(default, rename = "son")]
    pub son: Taille,
}

impl Reglage {
    pub fn est_defaut(&self) -> bool {
        *self == Reglage::default()
    }
}

thread_local! {
    /// La hauteur d'une piste son à cette image d'écran (posée au début du dessin du montage, `App::poser_hauteur_pistes`).
    /// Par fil : l'interface est dessinée par un seul fil, et chaque essai a le sien.
    static PISTE_SON: Cell<f32> = const { Cell::new(crate::soundtrack::TRACK_H) };
}

/// La hauteur d'une piste son, en points.
pub(crate) fn piste_son() -> f32 {
    PISTE_SON.with(Cell::get)
}

pub(crate) fn poser_piste_son(h: f32) {
    PISTE_SON.with(|c| c.set(h));
}

/// Les boutons M (muet), S (solo) et verrou d'une piste son dont la case des noms est `lane` : sur la ligne du bas, de
/// 16 points de haut au moins, à toute hauteur de piste.
pub(crate) fn boutons_piste(lane: Rect) -> [Rect; 3] {
    let h = if lane.height() < 38.0 { 16.0 } else { 19.0 };
    let y = lane.bottom() - h - 1.5;
    let w = 23.0;
    let un = |k: f32| Rect::from_min_size(pos2(lane.left() + 4.0 + k * (w + 3.0), y), vec2(w, h));
    [un(0.0), un(1.0), un(2.0)]
}

#[cfg(test)]
thread_local! {
    /// La hauteur du contenu défilant du montage et celle de ce qu'on en voit, à la dernière image (essais).
    pub(crate) static DEFILEMENT: Cell<(f32, f32)> = const { Cell::new((0.0, 0.0)) };
}

/// La fraction du bord (part de la hauteur donnée à la partie du haut) bornée pour que le haut ait `haut_min` points au
/// moins et le bas `bas_min` : si la place ne suffit pas aux deux, le bas garde sa part.
pub(crate) fn fraction_bornee(total: f32, fraction: f32, haut_min: f32, bas_min: f32) -> f32 {
    if !(total.is_finite() && total > 1.0) {
        return fraction;
    }
    let max = 1.0 - bas_min / total;
    let min = (haut_min / total).min(max);
    fraction.clamp(min, max.max(0.05))
}

/// Le haut d'un nœud est-il (en premier onglet) le dessin, et le bas le montage ou la timeline ?
fn bord_du_montage(tree: &egui_dock::Tree<Pane>, at: NodeIndex) -> bool {
    let first = |n: NodeIndex| -> Option<Pane> {
        let mut at = n;
        loop {
            if at.0 >= tree.len() {
                return None;
            }
            match &tree[at] {
                Node::Leaf { tabs, .. } => return tabs.first().copied(),
                Node::Vertical { .. } | Node::Horizontal { .. } => at = at.left(),
                Node::Empty => return None,
            }
        }
    };
    matches!(first(at.right()), Some(Pane::Montage | Pane::Timeline))
}

impl App {
    /// Pose la hauteur des pistes son de ce réglage (au début du dessin du montage ; les essais qui calculent des rangées
    /// sans dessiner passent aussi par `montage_rows`).
    pub(crate) fn poser_hauteur_pistes(&self) {
        poser_piste_son(self.settings.track_height.son.piste_son());
    }

    /// Après l'affichage des panneaux : le bord entre le dessin et la bande du montage garde ses hauteurs minimales, et sa
    /// poignée se dessine (une pastille au milieu, plus claire au survol et pendant le glissé).
    pub(crate) fn poignee_du_montage(&mut self, ctx: &egui::Context, state: &mut DockState<Pane>) {
        let tree = state.main_surface_mut();
        let mut poignee: Option<(Rect, f32)> = None;
        for i in 0..tree.len() {
            let at = NodeIndex(i);
            let Some(Node::Vertical { rect, fraction, .. }) = (at.0 < tree.len()).then(|| &tree[at]) else { continue };
            let (rect, fraction) = (*rect, *fraction);
            if !bord_du_montage(tree, at) || !rect.is_finite() || rect.height() <= 0.0 {
                continue;
            }
            // (Seulement quand la fenêtre est assez haute pour les deux minima : sinon le bord d'egui_dock reste seul juge.)
            if rect.height() > BANDE_MIN + DESSIN_MIN + 8.0 {
                let f = fraction_bornee(rect.height(), fraction, DESSIN_MIN, BANDE_MIN);
                if (f - fraction).abs() > 1e-5 {
                    if let Node::Vertical { fraction, .. } = &mut tree[at] {
                        *fraction = f;
                    }
                    ctx.request_repaint();
                }
                poignee = Some((rect, f));
            } else {
                poignee = Some((rect, fraction));
            }
            break;
        }
        let Some((rect, fraction)) = poignee else { return };
        let y = rect.top() + rect.height() * fraction;
        let zone = Rect::from_center_size(pos2(rect.center().x, y), vec2(rect.width(), 10.0));
        // Allumée au survol, et pendant un glissé commencé sur le bord (le pointeur appuyé tout près de lui).
        let actif = ctx.input(|i| {
            let p = i.pointer.hover_pos().or(i.pointer.latest_pos());
            p.is_some_and(|p| zone.contains(p) || (i.pointer.primary_down() && zone.expand2(vec2(0.0, 40.0)).contains(p)))
        });
        let pastille = Rect::from_center_size(pos2(rect.center().x, y), vec2(if actif { 64.0 } else { 44.0 }, if actif { 5.0 } else { 4.0 }));
        let couleur: Color32 = if actif { crate::theme::accent_texte() } else { crate::theme::texte_fort().gamma_multiply(0.45) };
        let painter = ctx.layer_painter(egui::LayerId::new(Order::Foreground, Id::new("poignee_montage")));
        painter.rect_filled(pastille, CornerRadius::same(2), couleur);
    }

    /// Le bouton de la barre du zoom : la hauteur des pistes son (petite, moyenne, grande), un clic passe à la suivante.
    pub(crate) fn hauteur_pistes_bouton(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::bulle;
        let t = self.settings.track_height.son;
        let icone = match t {
            Taille::Petite => egui_phosphor::regular::ROWS,
            Taille::Moyenne => egui_phosphor::regular::LIST,
            Taille::Grande => egui_phosphor::regular::SQUARES_FOUR,
        };
        let b = egui::Button::new(egui::RichText::new(icone).size(11.0)).frame(false).min_size(vec2(16.0, 14.0));
        let texte = format!("Hauteur des pistes son : {}. Un clic passe à la taille suivante (petite, moyenne, grande).", t.nom().to_lowercase());
        if bulle(ui.add(b), "Hauteur des pistes son", &texte, "").clicked() {
            self.regler_hauteur_pistes(t.suivante());
        }
    }

    /// Change la hauteur des pistes son et la retient.
    pub(crate) fn regler_hauteur_pistes(&mut self, t: Taille) {
        if self.settings.track_height.son != t {
            self.settings.track_height.son = t;
            let _ = self.settings.save();
        }
        self.poser_hauteur_pistes();
    }

    /// Préférences > Montage > Pistes : la hauteur des pistes son.
    pub(crate) fn hauteur_pistes_pref_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let label = ui.label("Hauteur des pistes son");
            crate::bulles::bulle(label, "Hauteur des pistes son", "Petite, moyenne ou grande : une piste plus haute montre mieux la forme d'onde et les boutons M, S et le verrou. Aussi dans la barre sous le montage.", "");
            let mut t = self.settings.track_height.son;
            for v in Taille::TOUTES {
                ui.selectable_value(&mut t, v, v.nom());
            }
            self.regler_hauteur_pistes(t);
        });
    }
}
