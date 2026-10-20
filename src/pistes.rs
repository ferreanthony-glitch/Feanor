//! Pistes repliables du montage (Anthony, 2026-10-20 : « un système de dépliage / repliage des
//! calques dans la timeline pour gagner de la place tout en gardant cela lisible »).
//!
//! Une piste a deux hauteurs :
//! - dépliée (`LAYER_H`, 40) : celle d'avant, avec l'œil, le verrou, la pelure, le menu ⋮ et la barre
//!   d'opacité dans la colonne de gauche ;
//! - repliée (`FOLDED_H`, 16, la hauteur de la piste Caméra) : une bande fine. À gauche, le triangle,
//!   le nom (11 points) et l'œil et le verrou quand la colonne a la place ; à droite, les dessins en
//!   barres minces (leurs limites d'exposition restent visibles), la barre du calque en trait fin, les
//!   losanges d'effets (violets, en haut) et de position (verts, au milieu) en petit. Tout se prend
//!   comme avant : choisir, glisser un dessin ou une clé, rectangle, lâcher, clic droit.
//!
//! L'état est celui de `Layer::collapsed`, le même que celui des « lignes réduites » de la timeline de
//! l'animation (13b) : un calque réduit là est replié ici, et inversement ; pour un dossier, `collapsed`
//! veut dire « fermé » (ses calques n'ont plus de piste), comme le triangle du panneau Calques. Le
//! drapeau est déjà enregistré avec le projet (`replie` du manifeste) : ni champ neuf ni changement de
//! version du format.
//!
//! Une piste (rangée r) traverse tous les plans : chaque plan a ses calques dans son ordre (6f). Deux
//! façons de la replier, au choix d'Anthony (2026-10-20, Préférences > Montage, `ReplisPistes`) :
//!
//! - **Commun à tous les plans** (d'office) : la hauteur de la rangée ne dépend PAS du plan courant
//!   (elle sauterait à chaque clic sur un autre plan) mais de tous : une rangée est repliée dès qu'un de
//!   ses calques l'est (« replié l'emporte »), et le triangle plie ou déplie d'un coup le calque de cette
//!   rangée dans chaque plan.
//! - **Propre à chaque plan** : chaque plan retient ses replis (les `collapsed` de ses propres calques). Les
//!   hauteurs suivent le plan courant ; les cases des autres plans se dessinent dans ces hauteurs. Le
//!   triangle et « tout replier / tout déplier » ne touchent que le plan courant (« … dans tous les plans »
//!   est au menu du clic droit). Les hauteurs changent quand la souris est relâchée (`PistesUi::layout_plan`),
//!   comme le dépliage du calque courant. Rien n'écrit dans les plans qu'on ne tient pas : utile à plusieurs.
//!
//! Passer d'un mode à l'autre n'écrit rien dans le projet : le mode commun calcule seulement son « replié
//! l'emporte » pour l'affichage. Un dossier ne concerne que son plan dans les deux modes : la rangée d'un
//! dossier fermé cache celles de ses calques, d'après le plan courant.
//!
//! « Déplier le calque courant » (réglage, allumé d'office, dans les deux modes) : la piste du calque
//! courant se montre dépliée, les autres gardent leur état. Elle se déplie quand la souris est relâchée,
//! jamais pendant un geste (sinon les pistes d'en dessous glisseraient sous le pointeur) ; replier cette
//! piste à la main ou « Replier toutes les pistes » la laissent repliée jusqu'au prochain changement de
//! calque.
//!
//! Replier ou déplier est de l'affichage : pas d'étape d'annulation (comme les lignes réduites de la
//! timeline), mais le projet est marqué modifié puisque l'état s'enregistre avec lui.

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, Response, RichText, Sense, Ui};
use egui_phosphor::regular as ph;
use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::document::Panel;
use crate::montagerows::{layer_in_row, LAYER_H};
use crate::theme;

/// Hauteur d'une piste repliée : celle de la piste Caméra.
pub(crate) const FOLDED_H: f32 = 16.0;
/// Rayon des losanges d'une piste repliée (4,5 et 5,5 dans une piste dépliée).
const MINI_D: f32 = 3.0;
/// Largeur du triangle dans la colonne de gauche.
pub(crate) const CHEVRON_W: f32 = 14.0;
/// Largeur de l'œil et du verrou d'une piste repliée.
const MINI_ICON_W: f32 = 14.0;
/// Place gardée au nom d'une piste repliée avant l'œil et le verrou.
const FOLDED_NAME_MIN: f32 = 40.0;

/// Une piste visible : la rangée de calques (0 : en haut de la pile), son haut (depuis le haut des
/// pistes de calques) et sa hauteur.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Lane {
    pub row: usize,
    pub top: f32,
    pub h: f32,
    pub folded: bool,
}

impl Lane {
    /// Les losanges d'effets (violets) se prennent jusqu'à cette hauteur depuis le haut de la piste ;
    /// sous elle, ceux de position, puis le dessin.
    pub fn fx_hit_max(&self) -> f32 {
        if self.folded {
            7.0
        } else {
            crate::fxkeys::STRIP
        }
    }

    /// Hauteur des losanges d'effets, depuis le haut de la piste.
    pub fn fx_y(&self) -> f32 {
        if self.folded {
            5.0
        } else {
            9.0
        }
    }

    /// Hauteur des losanges de position, depuis le haut de la piste (le milieu d'une piste dépliée).
    pub fn pos_y(&self) -> f32 {
        if self.folded {
            9.0
        } else {
            App::pos_marker_y(0.0, self.h)
        }
    }

    /// Distance verticale à laquelle on prend un losange de position.
    pub fn pos_hit(&self) -> f32 {
        if self.folded {
            4.0
        } else {
            crate::poskeysui::HIT_H
        }
    }

    /// Depuis le haut de la piste, la hauteur où commence la barre qui décale le calque.
    pub fn bar_from(&self) -> f32 {
        if self.folded {
            self.h - 5.0
        } else {
            self.h - 3.0 - crate::montagerows::BAR_H - 1.0
        }
    }
}

/// Une piste dépliée (pour les essais).
#[cfg(test)]
pub(crate) fn full_lane() -> Lane {
    Lane { row: 0, top: 0.0, h: LAYER_H, folded: false }
}

/// Le calque `(plan, calque)` — ids — qui s'est déplié tout seul en devenant courant.
pub(crate) type AutoOpen = Option<(u64, u64)>;

/// Comment les pistes se replient (réglage de l'application, `reglages.toml` : `repliage_pistes`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplisPistes {
    /// Une piste traverse tous les plans : repliée dès qu'un de ses calques l'est, repliée ou dépliée
    /// dans chaque plan d'un coup.
    #[default]
    Commun,
    /// Chaque plan retient ses replis : les hauteurs suivent le plan courant, qui est seul touché.
    ParPlan,
}

impl ReplisPistes {
    pub fn est_commun(&self) -> bool {
        *self == ReplisPistes::Commun
    }
}

/// Ce qu'il faut savoir pour dire si une rangée est repliée : le mode, et dans le mode « par plan » le
/// plan (son rang) dont les replis font les hauteurs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Replis {
    pub mode: ReplisPistes,
    pub plan: usize,
}

impl Replis {
    /// Le mode commun : le plan n'y compte pas.
    #[cfg(test)]
    pub const COMMUN: Replis = Replis { mode: ReplisPistes::Commun, plan: 0 };
}

/// Ce que le montage retient des pistes en dehors du document : quel calque s'est déplié tout seul.
#[derive(Default)]
pub(crate) struct PistesUi {
    pub auto_open: AutoOpen,
    /// Le dernier calque courant vu : la piste ne se déplie qu'au changement.
    seen: AutoOpen,
    /// Le plan (son id) dont les replis font les hauteurs en mode « propre à chaque plan » : le plan
    /// courant au dernier relâchement de la souris (sinon les pistes d'en dessous glisseraient sous le
    /// pointeur quand un clic change de plan). Vide : le plan courant.
    pub layout_plan: Option<u64>,
}

/// La rangée `row` est cachée : son calque, dans le plan courant `cur`, est rangé dans un dossier fermé
/// (sauf s'il est le calque courant `cur_layer` : comme dans le panneau Calques, on ne le perd pas de vue).
pub(crate) fn row_hidden(panels: &[Panel], cur: usize, cur_layer: usize, row: usize) -> bool {
    let Some(layers) = panels.get(cur).map(|p| &p.layers) else { return false };
    let Some(k) = layer_in_row(layers, row) else { return false };
    k != cur_layer && layers[k].parent.is_some_and(|p| layers.iter().any(|f| f.folder && f.id == p && f.collapsed))
}

/// Le calque qui s'est déplié tout seul est à la rangée `row` (de son plan ; `only` : dans ce plan-là
/// seulement, en mode « par plan »).
fn auto_open_at(panels: &[Panel], auto: AutoOpen, row: usize, only: Option<usize>) -> bool {
    let Some((panel, layer)) = auto else { return false };
    panels.iter().enumerate().any(|(i, p)| (only.is_none() || only == Some(i)) && p.id == panel && layer_in_row(&p.layers, row).is_some_and(|k| p.layers[k].id == layer && !p.layers[k].folder))
}

/// La rangée `row` est repliée selon `replis`. Commun : un de ses calques l'est (dans un plan quelconque).
/// Par plan : le calque de cette rangée dans le plan `replis.plan` l'est (pas de calque à cette rangée :
/// dépliée). Dans les deux cas, pas repliée si le calque courant s'y est déplié tout seul.
pub(crate) fn row_folded_in(panels: &[Panel], row: usize, auto: AutoOpen, replis: Replis) -> bool {
    match replis.mode {
        ReplisPistes::Commun => !auto_open_at(panels, auto, row, None) && panels.iter().any(|p| layer_in_row(&p.layers, row).is_some_and(|k| p.layers[k].collapsed)),
        ReplisPistes::ParPlan => {
            !auto_open_at(panels, auto, row, Some(replis.plan))
                && panels.get(replis.plan).is_some_and(|p| layer_in_row(&p.layers, row).is_some_and(|k| p.layers[k].collapsed))
        }
    }
}

/// `row_folded_in` en mode commun (pour les essais).
#[cfg(test)]
pub(crate) fn row_folded(panels: &[Panel], row: usize, auto: AutoOpen) -> bool {
    row_folded_in(panels, row, auto, Replis::COMMUN)
}

/// Les pistes visibles en mode commun, de haut en bas : `count` rangées au plus, celles d'un dossier fermé en
/// moins (pour les essais ; l'application passe par `lanes_in`).
#[cfg(test)]
pub(crate) fn lanes(panels: &[Panel], cur: usize, cur_layer: usize, count: usize, auto: AutoOpen) -> Vec<Lane> {
    lanes_in(panels, cur, cur_layer, count, auto, Replis::COMMUN)
}

/// `lanes` selon le mode de repliage.
pub(crate) fn lanes_in(panels: &[Panel], cur: usize, cur_layer: usize, count: usize, auto: AutoOpen, replis: Replis) -> Vec<Lane> {
    let mut top = 0.0;
    let mut out = Vec::new();
    for row in 0..count {
        if row_hidden(panels, cur, cur_layer, row) {
            continue;
        }
        let folded = row_folded_in(panels, row, auto, replis);
        let h = if folded { FOLDED_H } else { LAYER_H };
        out.push(Lane { row, top, h, folded });
        top += h;
    }
    out
}

/// Un losange plein de rayon `d` (un petit en piste repliée).
pub(crate) fn mini_diamond(painter: &egui::Painter, c: egui::Pos2, d: f32, fill: Color32, edge: Color32, width: f32, circle: bool) {
    let stroke = egui::Stroke::new(width, edge);
    if circle {
        painter.circle(c, d * 0.9, fill, stroke);
    } else {
        painter.add(egui::Shape::convex_polygon(vec![pos2(c.x, c.y - d), pos2(c.x + d, c.y), pos2(c.x, c.y + d), pos2(c.x - d, c.y)], fill, stroke));
    }
}

/// Icône sans cadre, de la taille d'une piste repliée, qui s'éclaire sous la souris.
fn mini_icon(ui: &mut Ui, rect: Rect, id: egui::Id, icon: &str, size: f32, color: Color32) -> Response {
    let resp = ui.interact(rect, id, Sense::click());
    let ink = if resp.hovered() { theme::texte_fort() } else { color };
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, icon, FontId::proportional(size), ink);
    resp
}

/// Où va chaque chose dans la colonne de gauche d'une piste repliée de rectangle `r`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FoldedHead {
    pub chevron: Rect,
    pub name: Rect,
    /// L'œil puis le verrou, autant qu'il en tient.
    pub icons: Vec<Rect>,
}

pub(crate) fn folded_layout(r: Rect) -> FoldedHead {
    let chevron = Rect::from_min_size(pos2(r.left() + 4.0, r.top()), vec2(CHEVRON_W, r.height()));
    let n = (((r.right() - 2.0 - chevron.right() - FOLDED_NAME_MIN) / MINI_ICON_W).floor().max(0.0) as usize).min(2);
    let icons: Vec<Rect> = (0..n).map(|k| Rect::from_min_size(pos2(r.right() - 2.0 - (n - k) as f32 * MINI_ICON_W, r.top()), vec2(MINI_ICON_W, r.height()))).collect();
    let name_right = icons.first().map_or(r.right() - 2.0, |i| i.left()) - 2.0;
    let name = Rect::from_min_max(pos2(chevron.right() + 1.0, r.top()), pos2(name_right.max(chevron.right() + 1.0), r.bottom()));
    FoldedHead { chevron, name, icons }
}

impl App {
    // ------------------------------------------------------------------ l'état

    /// La piste du calque courant se déplie quand il change (réglage « Déplier le calque courant »), pas
    /// pendant un geste : `pointer_down` (la souris est tenue) la retient.
    /// Le plan dont les replis font les hauteurs (mode « par plan ») suit le plan courant au même moment.
    pub(crate) fn pistes_follow_current(&mut self, pointer_down: bool) {
        if pointer_down {
            return;
        }
        let plan = self.doc.panels.get(self.cur_panel).map(|p| p.id);
        if self.pistes.layout_plan != plan {
            self.pistes.layout_plan = plan;
        }
        let cur = self.doc.panels.get(self.cur_panel).and_then(|p| p.layers.get(self.cur_layer).map(|l| (p.id, l.id)));
        if cur == self.pistes.seen {
            return;
        }
        self.pistes.seen = cur;
        self.pistes.auto_open = cur;
    }

    /// Le calque qui se déplie tout seul, si le réglage est allumé.
    pub(crate) fn pistes_auto_open(&self) -> AutoOpen {
        self.settings.montage_auto_open.then_some(self.pistes.auto_open).flatten()
    }

    /// La rangée du calque `li` du plan courant.
    pub(crate) fn track_row_of(&self, li: usize) -> usize {
        self.layers().len().saturating_sub(li + 1)
    }

    /// Comment les rangées se replient maintenant : le mode des Préférences et le plan dont les replis font
    /// les hauteurs (celui du dernier relâchement de la souris ; le plan courant s'il n'existe plus).
    pub(crate) fn pistes_replis(&self) -> Replis {
        let plan = self.pistes.layout_plan.and_then(|id| self.doc.panels.iter().position(|p| p.id == id)).unwrap_or(self.cur_panel);
        Replis { mode: self.settings.montage_folds, plan }
    }

    /// Le repliage est propre à chaque plan (et non commun à tous).
    pub(crate) fn pistes_par_plan(&self) -> bool {
        self.settings.montage_folds == ReplisPistes::ParPlan
    }

    /// La rangée `row` est repliée à l'écran (réglage du calque courant compris).
    pub(crate) fn track_is_folded(&self, row: usize) -> bool {
        row_folded_in(&self.doc.panels, row, self.pistes_auto_open(), self.pistes_replis())
    }

    /// Replie ou déplie la piste de la rangée `row`. Un calque ordinaire : le calque de cette rangée dans
    /// chaque plan (mode commun) ou dans le plan courant seulement (mode « par plan ») ; un dossier : celui
    /// du plan courant seulement (fermé, il cache les pistes de ses calques). Faux si rien n'existe à cette
    /// rangée.
    pub(crate) fn set_track_folded(&mut self, row: usize, folded: bool) -> bool {
        let cur = self.cur_panel;
        let per_plan = self.pistes_par_plan();
        let Some(is_folder) = self.doc.panels.get(cur).and_then(|p| layer_in_row(&p.layers, row).map(|k| p.layers[k].folder)) else { return false };
        if folded && auto_open_at(&self.doc.panels, self.pistes.auto_open, row, per_plan.then_some(cur)) {
            self.pistes.auto_open = None;
        }
        let mut changed = false;
        for (pi, p) in self.doc.panels.iter_mut().enumerate() {
            if (is_folder || per_plan) && pi != cur {
                continue;
            }
            if let Some(k) = layer_in_row(&p.layers, row) {
                let l = &mut p.layers[k];
                if l.folder == is_folder && l.collapsed != folded {
                    l.collapsed = folded;
                    changed = true;
                }
            }
        }
        if changed {
            self.modified = true;
        }
        true
    }

    /// Le triangle d'une piste : replie ou déplie cette piste (Alt : toutes les pistes).
    pub(crate) fn toggle_track_fold(&mut self, row: usize, all: bool) {
        let folded = self.track_is_folded(row);
        if all {
            self.set_all_tracks_folded(!folded);
        } else {
            self.set_track_folded(row, !folded);
        }
    }

    /// Replie toutes les pistes (les dossiers restent comme ils sont), ou déplie tout (les dossiers
    /// fermés s'ouvrent : plus aucune piste cachée) : dans tous les plans en mode commun, dans le plan
    /// courant en mode « par plan ». Renvoie le nombre de calques changés.
    pub(crate) fn set_all_tracks_folded(&mut self, folded: bool) -> usize {
        let all_plans = !self.pistes_par_plan();
        self.set_all_tracks_folded_in(folded, all_plans)
    }

    /// `set_all_tracks_folded` dans tous les plans (`all_plans`) ou dans le plan courant seulement.
    pub(crate) fn set_all_tracks_folded_in(&mut self, folded: bool, all_plans: bool) -> usize {
        let mut changed = 0;
        let cur = self.cur_panel;
        for (pi, p) in self.doc.panels.iter_mut().enumerate() {
            if !all_plans && pi != cur {
                continue;
            }
            for l in &mut p.layers {
                if l.folder && folded {
                    continue;
                }
                if l.collapsed != folded {
                    l.collapsed = folded;
                    changed += 1;
                }
            }
        }
        if folded {
            self.pistes.auto_open = None;
        }
        if changed > 0 {
            self.modified = true;
        }
        let scope = if all_plans && self.pistes_par_plan() {
            " de tous les plans"
        } else if self.pistes_par_plan() {
            " de ce plan"
        } else {
            ""
        };
        self.status = format!("Toutes les pistes{scope} sont {}.", if folded { "repliées" } else { "dépliées" });
        changed
    }

    /// Le repliage des pistes : commun à tous les plans, ou propre à chaque plan. N'écrit rien dans le
    /// projet : les replis de chaque plan restent tels quels, seul l'affichage change.
    pub(crate) fn set_replis_pistes(&mut self, mode: ReplisPistes) {
        if self.settings.montage_folds == mode {
            return;
        }
        self.settings.montage_folds = mode;
        // Les hauteurs suivent le plan courant tout de suite : on vient de cliquer un réglage, pas une piste.
        self.pistes.layout_plan = self.doc.panels.get(self.cur_panel).map(|p| p.id);
        if let Err(e) = self.settings.save() {
            self.status = format!("Réglages non enregistrés : {e}");
            return;
        }
        self.status = match mode {
            ReplisPistes::Commun => "Repliage des pistes : commun à tous les plans.".into(),
            ReplisPistes::ParPlan => "Repliage des pistes : propre à chaque plan.".into(),
        };
    }

    /// Bascule entre les deux façons de replier les pistes (action `piste.replis_par_plan`).
    pub(crate) fn toggle_replis_pistes(&mut self) {
        let next = if self.pistes_par_plan() { ReplisPistes::Commun } else { ReplisPistes::ParPlan };
        self.set_replis_pistes(next);
    }

    /// « Déplier le calque courant » : allumé, éteint.
    pub(crate) fn toggle_auto_open(&mut self) {
        self.settings.montage_auto_open = !self.settings.montage_auto_open;
        if self.settings.montage_auto_open {
            // Le calque courant se déplie tout de suite.
            self.pistes.seen = None;
        }
        if let Err(e) = self.settings.save() {
            self.status = format!("Réglages non enregistrés : {e}");
        }
        self.status = format!("Le calque courant {} sa piste tout seul.", if self.settings.montage_auto_open { "déplie" } else { "ne déplie plus" });
    }

    // ------------------------------------------------------------------ les losanges d'une piste repliée

    /// Les losanges d'effets (violets, en haut) et de position (verts, plus bas) du calque `k` du plan
    /// `p`, en petit, dans une piste repliée dont le haut est `lane_top`. Les mêmes conditions que dans
    /// une piste dépliée (`paint_fx_markers`, `paint_pos_markers`) ; les mêmes couleurs.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_mini_markers(&self, painter: &egui::Painter, p: &Panel, k: usize, x_of: impl Fn(usize) -> f32, lane_top: f32, lane: &Lane, visible: (f32, f32)) {
        let layer = &p.layers[k];
        let max = p.frame_count();
        let on_screen = |x: f32| x >= visible.0 - 8.0 && x <= visible.1 + 8.0;
        if let Some(sel) = self.fx_shown(p.id, layer) {
            let chosen = self.fx_chosen_frames(p.id, layer.id);
            for f in crate::fxkeys::marker_frames(&layer.fx[sel.effect]) {
                let x = x_of(f);
                if f >= max || !on_screen(x) {
                    continue;
                }
                let on = chosen.contains(&f);
                let (d, edge, width) = if on { (MINI_D + 1.2, theme::contour(), 1.6) } else { (MINI_D, Color32::from_black_alpha(200), 1.0) };
                mini_diamond(painter, pos2(x, lane_top + lane.fx_y()), d, crate::fxkeys::color(), edge, width, false);
            }
        }
        let current_layer = self.layers().get(self.cur_layer).is_some_and(|l| l.id == layer.id);
        for key in &layer.pos_keys {
            let x = x_of(key.frame);
            if key.frame >= max || !on_screen(x) {
                continue;
            }
            let on = self.pos_ui.sel.contains(&(layer.id, key.frame)) || (key.frame == self.cur_frame && current_layer);
            let (d, edge, width) = if on { (MINI_D + 1.2, Color32::WHITE, 1.5) } else { (MINI_D, Color32::from_black_alpha(190), 1.0) };
            mini_diamond(painter, pos2(x, lane_top + lane.pos_y()), d, theme::POIGNEE_CALQUE, edge, width, key.is_eased());
        }
    }

    // ------------------------------------------------------------------ la colonne de gauche

    /// Le triangle d'une piste (dépliée ou repliée) : un clic la plie ou la déplie, Alt + clic toutes les
    /// pistes. Pour un dossier : ferme ou ouvre le dossier.
    pub(crate) fn track_chevron(&mut self, ui: &mut Ui, rect: Rect, li: usize, folded: bool, size: f32, color: Color32) {
        let Some((id, folder)) = self.layers().get(li).map(|l| (l.id, l.folder)) else { return };
        let icon = if folded { ph::CARET_RIGHT } else { ph::CARET_DOWN };
        let resp = mini_icon(ui, rect, egui::Id::new(("triangle_piste", id)), icon, size, color);
        let (what, why) = match (folder, folded) {
            (true, true) => ("Ouvrir le dossier", "Montre les pistes de ses calques. Alt + clic : déplier toutes les pistes"),
            (true, false) => ("Fermer le dossier", "Cache les pistes de ses calques (ils restent à l'image). Alt + clic : replier toutes les pistes"),
            (false, true) => ("Déplier la piste", "Montre les réglages du calque et rend la piste plus haute. Alt + clic : déplier toutes les pistes"),
            (false, false) => ("Replier la piste", "Réduit la piste à une bande fine : le nom et les dessins restent visibles. Alt + clic : replier toutes les pistes"),
        };
        // Mode « par plan » : seul le plan courant est touché, dit dans la bulle.
        let why = if self.pistes_par_plan() { format!("{why} de ce plan. Les autres plans gardent leurs replis.") } else { format!("{why}.") };
        if self.aide(resp, what, &why, "piste.basculer").clicked() {
            let all = ui.input(|i| i.modifiers.alt);
            let row = self.track_row_of(li);
            self.toggle_track_fold(row, all);
        }
    }

    /// Le nom du calque `li` (champ de renommage compris) et l'œil et le verrou d'une piste repliée,
    /// dans `r`. Le fond de la rangée (clics, glissés, menu) est déjà posé.
    pub(crate) fn montage_folded_head(&mut self, ui: &mut Ui, r: Rect, li: usize, name_color: Color32) {
        let Some(l) = self.layers().get(li) else { return };
        let (id, name, visible, locked) = (l.id, l.name.clone(), l.visible, l.locked);
        let at = folded_layout(r);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r).id_salt(("tete_piste_repliee", id)));
        let ui = &mut child;
        let fg = ui.visuals().text_color();
        self.track_chevron(ui, at.chevron, li, true, 12.0, fg.gamma_multiply(0.7));
        let renaming = self.layer_rename.as_ref().is_some_and(|n| n.id == id && n.in_montage);
        if renaming {
            let mut field = ui.new_child(egui::UiBuilder::new().max_rect(at.name.expand2(vec2(0.0, 2.0))).id_salt(("nom_calque_montage", id)));
            let done = self.layer_rename_field(&mut field, at.name.width());
            self.finish_layer_rename(done);
        } else {
            let ink = if visible { name_color } else { name_color.gamma_multiply(0.5) };
            ui.painter().with_clip_rect(at.name.expand2(vec2(0.0, 1.0))).text(pos2(at.name.left(), at.name.center().y - 0.5), Align2::LEFT_CENTER, &name, FontId::proportional(11.0), ink);
        }
        let off = fg.gamma_multiply(0.35);
        if let Some(slot) = at.icons.first() {
            let resp = mini_icon(ui, *slot, egui::Id::new(("oeil_piste", id)), if visible { ph::EYE } else { ph::EYE_SLASH }, 12.0, if visible { fg } else { off });
            if crate::bulles::bulle(resp, "Afficher / cacher", "Montrer ou cacher le calque (et sa couleur, si c'est un trait).", "").clicked() {
                self.toggle_layer_visible(li);
            }
        }
        if let Some(slot) = at.icons.get(1) {
            let resp = mini_icon(ui, *slot, egui::Id::new(("verrou_piste", id)), if locked { ph::LOCK_SIMPLE } else { ph::LOCK_SIMPLE_OPEN }, 12.0, if locked { fg } else { off });
            if self.aide(resp, "Verrouiller / déverrouiller", "Un calque verrouillé ne peut plus être dessiné.", "calque.verrouiller").clicked() {
                self.toggle_layer_lock(li);
            }
        }
    }

    /// Un double-clic sur la colonne d'une piste hors du nom la plie ou la déplie (sur le nom : le
    /// renommer, comme avant). `pos` : le point cliqué ; `name_rect` : où est le nom ; `size` : sa taille.
    pub(crate) fn double_click_is_on_name(&self, ui: &Ui, pos: egui::Pos2, name_rect: Rect, name: &str, size: f32) -> bool {
        if !name_rect.contains(pos) {
            return false;
        }
        let width = ui.fonts(|f| f.layout_no_wrap(name.to_string(), FontId::proportional(size), Color32::WHITE).size().x);
        pos.x <= name_rect.left() + width + 4.0
    }

    // ------------------------------------------------------------------ les menus

    /// « Replier / déplier les pistes » : dans le menu du calque `li` (clic droit sur sa piste).
    pub(crate) fn pistes_menu(&mut self, ui: &mut Ui, li: usize) {
        let row = self.track_row_of(li);
        let folded = self.track_is_folded(row);
        ui.menu_button("Replier / déplier les pistes", |ui| {
            if ui.button(if folded { "Déplier cette piste" } else { "Replier cette piste" }).clicked() {
                ui.close_menu();
                self.set_track_folded(row, !folded);
            }
            let par_plan = self.pistes_par_plan();
            self.action_item(ui, if par_plan { "Replier toutes les pistes de ce plan" } else { "Replier toutes les pistes" }, "piste.replier_tout");
            self.action_item(ui, if par_plan { "Déplier toutes les pistes de ce plan" } else { "Déplier toutes les pistes" }, "piste.deplier_tout");
            if par_plan {
                if ui.button("Replier les pistes de tous les plans").clicked() {
                    ui.close_menu();
                    self.set_all_tracks_folded_in(true, true);
                }
                if ui.button("Déplier les pistes de tous les plans").clicked() {
                    ui.close_menu();
                    self.set_all_tracks_folded_in(false, true);
                }
            }
            ui.separator();
            let mut auto = self.settings.montage_auto_open;
            if ui.checkbox(&mut auto, "Déplier le calque courant tout seul").on_hover_text("La piste du calque courant se montre dépliée ; les autres gardent leur état.").clicked() {
                self.run_action("piste.auto_deplier");
            }
            ui.separator();
            ui.label(RichText::new("Repliage des pistes").small().weak());
            self.replis_choices(ui);
        });
    }

    /// Les deux façons de replier, en deux lignes à choisir (menu du clic droit sur une piste).
    fn replis_choices(&mut self, ui: &mut Ui) {
        for (mode, nom, aide) in [(ReplisPistes::Commun, "Commun à tous les plans", REPLIS_COMMUN), (ReplisPistes::ParPlan, "Propre à chaque plan", REPLIS_PAR_PLAN)] {
            let on = self.settings.montage_folds == mode;
            let resp = ui.selectable_label(on, nom);
            if crate::bulles::bulle(resp, nom, aide, "").clicked() {
                ui.close_menu();
                self.set_replis_pistes(mode);
            }
        }
    }

    /// La section « Montage » de Fichier > Préférences… : le repliage des pistes (deux icônes à choisir, une
    /// phrase de bulle pour chacune) et « Déplier le calque courant ».
    pub(crate) fn montage_prefs_ui(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Montage").strong());
        ui.horizontal(|ui| {
            let label = ui.label("Repliage des pistes");
            crate::bulles::bulle(label, "Repliage des pistes", &format!("{REPLIS_COMMUN} {REPLIS_PAR_PLAN}"), "");
            let mut mode = self.settings.montage_folds;
            let a = crate::bulles::choix(ui, &mut mode, ReplisPistes::Commun, ph::LINK_SIMPLE, "Commun à tous les plans", REPLIS_COMMUN, "");
            let b = crate::bulles::choix(ui, &mut mode, ReplisPistes::ParPlan, ph::LINK_SIMPLE_BREAK, "Propre à chaque plan", REPLIS_PAR_PLAN, "");
            if a.changed() || b.changed() {
                self.set_replis_pistes(mode);
            }
            ui.label(RichText::new(if mode == ReplisPistes::Commun { "commun à tous les plans" } else { "propre à chaque plan" }).small().weak());
        });
        let mut auto = self.settings.montage_auto_open;
        let resp = ui.checkbox(&mut auto, "Déplier le calque courant tout seul").on_hover_text("La piste du calque courant se montre dépliée ; les autres gardent leur état.");
        if resp.clicked() {
            self.run_action("piste.auto_deplier");
        }
    }
}

/// Une phrase pour chaque façon de replier (bulles, Préférences, menu).
pub(crate) const REPLIS_COMMUN: &str = "Commun à tous les plans : une piste est repliée dès qu'un de ses calques l'est, et le triangle la plie dans chaque plan.";
pub(crate) const REPLIS_PAR_PLAN: &str = "Propre à chaque plan : chaque plan retient ses replis, les hauteurs suivent le plan courant et le triangle ne touche que lui.";
