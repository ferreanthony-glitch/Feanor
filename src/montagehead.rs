//! Colonne de gauche du montage : sa largeur et les réglages des calques (Anthony, 2026-10-17 :
//! « ajoute la possibilité d'étirer la fenêtre caméra et calques en ajoutant [à côté des noms]
//! les options de calques, options de fusion, opacité, option d'animation en boucle, etc. En
//! clic droit tout ce qui a trait aux calques. […] une barre d'étirage pour laisser la place
//! aux options »).
//! - Une barre au bord droit de la colonne l'élargit ou la rétrécit ; la largeur est retenue
//!   dans les réglages. Double-clic : tout montrer, ou revenir à la colonne étroite.
//! - La rangée d'un calque ne montre que l'œil, la pastille de sa sorte et le nom (`pistes::montage_head`, 2026-10-28) : l'opacité,
//!   le verrou, le mode de fusion, la pelure d'oignon et le reste sont au clic droit (`montage_layer_menu`) et dans le panneau
//!   Calques. La largeur sert aux noms longs.
//! - Clic droit sur la rangée : tout ce qui touche au calque (`montage_layer_menu`).


use eframe::egui::{self, pos2, CursorIcon, Rect, Sense, Stroke};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles::bulle;
use crate::document::{Behaviour, LayerKind};
use crate::soundtrack::HEADERS_W;

/// Largeur la plus petite de la colonne : celle d'avant (les noms seuls).
pub(crate) const HEAD_MIN: f32 = HEADERS_W;
/// Largeur la plus grande.
pub(crate) const HEAD_MAX: f32 = 520.0;
/// Largeur où tout se voit (double-clic sur la barre).
pub(crate) const HEAD_ALL: f32 = 330.0;
/// Largeur de la barre qu'on tire, au bord droit de la colonne.
const GRIP_W: f32 = 6.0;

/// Largeur de colonne permise (une valeur lue dans les réglages peut être n'importe quoi).
pub(crate) fn clamp_head_w(w: f32) -> f32 {
    if w.is_finite() {
        w.clamp(HEAD_MIN, HEAD_MAX)
    } else {
        HEAD_MIN
    }
}

/// Icône d'un pré / post-comportement.
pub(crate) fn behaviour_icon(b: Behaviour) -> &'static str {
    match b {
        Behaviour::Nothing => ph::MINUS,
        Behaviour::Loop => ph::REPEAT,
        Behaviour::PingPong => ph::ARROWS_LEFT_RIGHT,
        Behaviour::Hold => ph::PAUSE,
    }
}

impl App {
    /// La barre qui étire la colonne `column` (son bord droit), posée après tout le reste de la
    /// colonne pour rester au-dessus.
    pub(crate) fn head_width_grip(&mut self, ui: &mut egui::Ui, column: Rect) {
        let grip = Rect::from_min_max(pos2(column.right() - GRIP_W, column.top()), pos2(column.right(), column.bottom()));
        let resp = ui.interact(grip, ui.id().with("etirer_colonne_montage"), Sense::click_and_drag());
        let hot = resp.hovered() || resp.dragged();
        let color = if hot { ui.visuals().selection.stroke.color } else { ui.visuals().widgets.noninteractive.bg_stroke.color };
        let x = column.right() - GRIP_W * 0.5;
        ui.painter().line_segment([pos2(x, column.top()), pos2(x, column.bottom())], Stroke::new(if hot { 2.0_f32 } else { 1.0 }, color));
        if hot {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
        }
        if resp.dragged() {
            self.montage_head_w = clamp_head_w(self.montage_head_w + resp.drag_delta().x);
        }
        let resp = bulle(
            resp,
            "Largeur de la colonne",
            "Tirer pour élargir la colonne et lire les noms longs des calques. Double-clic : l'élargir, ou revenir à la colonne étroite.",
            "",
        );
        if resp.double_clicked() {
            self.montage_head_w = if self.montage_head_w < HEAD_ALL - 1.0 { HEAD_ALL } else { HEAD_MIN };
        }
        if resp.drag_stopped() || resp.double_clicked() {
            self.settings.montage_head_w = self.montage_head_w;
            if let Err(e) = self.settings.save() {
                self.status = format!("Réglages non enregistrés : {e}");
            }
        }
    }

    /// « Propager la peinture… » (`propagerui.rs`) pour le calque `li` : toujours dans le menu,
    /// grisé avec la raison quand ce calque ne peut pas être propagé (Anthony, 2026-10-17 : « je
    /// ne trouve plus la propagation de couleur » : l'entrée n'apparaissait que sur un calque
    /// bitmap ayant un autre calque à suivre, et le calque par défaut est vectoriel).
    pub(crate) fn propagation_item(&mut self, ui: &mut egui::Ui, li: usize) {
        let blocker = match self.layers().get(li) {
            None => Some("Aucun calque."),
            Some(l) if l.video.is_some() => Some("C'est un calque vidéo : peins sur un calque bitmap au-dessus, puis propage ce calque-là."),
            Some(l) if l.folder || l.kind != LayerKind::Bitmap => {
                Some("La propagation se fait sur un calque bitmap : crée-en un (Calques > Nouveau calque > Bitmap) au-dessus de la vidéo ou de l'animation, et peins dessus.")
            }
            Some(_) if crate::propager::guides(&self.doc, self.cur_panel, li).is_empty() => Some("Rien à suivre : il faut un autre calque dans le plan (vidéo, animation…)."),
            Some(_) => None,
        };
        if blocker.is_none() && li == self.cur_layer {
            // L'entrée de `propagerui` (elle agit sur le calque courant).
            self.propagation_menu(ui, li);
            return;
        }
        ui.separator();
        let shortcut = self.keymap.label("calque.propager");
        let resp = ui.add_enabled(blocker.is_none(), egui::Button::new("Propager la peinture…").shortcut_text(&shortcut));
        if let Some(why) = blocker {
            resp.on_disabled_hover_text(why);
        } else if resp.clicked() {
            ui.close_menu();
            self.select_layer(li);
            self.run_action("calque.propager");
        }
    }
}
