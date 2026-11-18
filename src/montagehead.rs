//! Colonne de gauche du montage : sa largeur et les réglages des calques (Anthony, 2026-10-17 :
//! « ajoute la possibilité d'étirer la fenêtre caméra et calques en ajoutant [à côté des noms]
//! les options de calques, options de fusion, opacité, option d'animation en boucle, etc. En
//! clic droit tout ce qui a trait aux calques. […] une barre d'étirage pour laisser la place
//! aux options »).
//! - Une barre au bord droit de la colonne l'élargit ou la rétrécit ; la largeur est retenue
//!   dans les réglages. Double-clic : tout montrer, ou revenir à la colonne étroite.
//! - La rangée d'un calque ne montre que l'œil, la pastille de sa sorte et le nom (`pistes::montage_head`, 2026-10-28) ; celle du
//!   calque actif porte toutes ses options sur deux rangées (`pistes::piste_active_head`, partie 160). Tout est aussi au clic
//!   droit (`montage_layer_menu`). La largeur sert aux noms longs, et à la vignette du calque actif.
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
/// Partie 173.5 (Anthony : « Reviens sur la taille de la barre latérale. Plus petite et ajustable ») : la largeur d'origine de la
/// colonne de la bande hors de l'espace Montage, là où il a tracé son trait (environ 72 % des 474 points d'avant).
pub(crate) const LARGEUR_STORYBOARD: f32 = 340.0;
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

/// Partie 165.5 : la largeur avant la première mesure de la barre (partie 173.5 : la plus longue de ses deux lignes, estimée).
const LARGEUR_AVANT_MESURE: f32 = 250.0;

/// Partie 165.5 (Anthony : « toujours garder cette colonne avec un maximum d'information. On peut verrouiller la taille minimale
/// de la colonne s'il faut. ») : la colonne ne descend jamais sous la largeur de la ligne des boutons de `barre` (mesurée à son
/// dessin, `barres::largeur_mesuree`), ni sous celle où la piste du calque actif montre sa vignette (`pistes::LARGEUR_VIGNETTE`).
/// Une colonne plus étroite (réglage d'avant, glissé) est élargie.
pub(crate) fn largeur_min_colonne(barre: crate::barres::Barre) -> f32 {
    // (Dans l'espace Montage, la barre reste au-dessus de la bande : la colonne garde ses bornes d'avant.)
    if barre != crate::barres::Barre::Montage {
        return HEAD_MIN;
    }
    // Partie 173.5 (Anthony : « Plus petite et ajustable ») : la barre passe sur deux lignes quand la colonne est trop étroite pour
    // une (`barre_sur_deux_lignes`) ; le minimum est la plus longue de ses deux lignes, et la place des options de la piste active.
    let mesure = crate::barres::largeur_mesuree(barre);
    let barre = if mesure > 0.0 { mesure } else { LARGEUR_AVANT_MESURE };
    barre.max(crate::pistes::LARGEUR_PISTE_MIN).max(crate::pistes::LARGEUR_VIGNETTE + 8.0).max(HEAD_MIN)
}

/// Partie 173.5 : la colonne large de `w` est trop étroite pour la barre du montage sur une ligne : elle est sur deux lignes, et
/// l'en-tête de la bande gagne la hauteur d'une ligne de boutons (`ENTETE_DEUX_LIGNES`).
pub(crate) fn barre_sur_deux_lignes(w: f32) -> bool {
    let une = crate::barres::largeur_une_ligne(crate::barres::Barre::Montage);
    let une = if une > 0.0 { une } else { LARGEUR_UNE_LIGNE_AVANT_MESURE };
    w + 0.5 < une
}

/// Partie 173.5 : la hauteur que l'en-tête de la bande gagne quand la barre du montage est sur deux lignes (une case de bouton, 26
/// points, et son écart), sous la ligne des séquences.
pub(crate) const ENTETE_DEUX_LIGNES: f32 = 28.0;

/// Partie 173.5 : la barre du montage sur une ligne avant sa première mesure (celle d'avant : 474 points mesurés).
const LARGEUR_UNE_LIGNE_AVANT_MESURE: f32 = 480.0;

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
        let color = if hot { crate::theme::accent_texte() } else { ui.visuals().widgets.noninteractive.bg_stroke.color };
        let x = column.right() - GRIP_W * 0.5;
        ui.painter().line_segment([pos2(x, column.top()), pos2(x, column.bottom())], Stroke::new(if hot { 2.0_f32 } else { 1.0 }, color));
        if hot {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
        }
        // (Jamais sous la largeur des boutons, partie 165.5 : le glissé s'arrête là.)
        let min = largeur_min_colonne(crate::barres::Barre::du_montage_dans(self.espace));
        if resp.dragged() {
            self.montage_head_w = clamp_head_w(self.montage_head_w + resp.drag_delta().x).max(min);
        }
        // (Partie 173.5 : hors de l'espace Montage, la colonne a sa largeur à elle, d'origine plus étroite : `storyboard_head_w`.)
        let storyboard = self.espace != crate::workspace::Espace::Montage;
        let resp = bulle(
            resp,
            "Largeur de la colonne",
            "Tirer pour élargir la colonne et lire les noms longs des calques. Double-clic : l'élargir, ou revenir à la colonne étroite.",
            "",
        );
        if resp.double_clicked() {
            self.montage_head_w = if storyboard {
                // (Hors de l'espace Montage : de la largeur d'origine à la plus large, et retour.)
                if self.montage_head_w < LARGEUR_STORYBOARD + 1.0 { HEAD_MAX } else { LARGEUR_STORYBOARD.max(min) }
            } else {
                (if self.montage_head_w < HEAD_ALL.max(min) - 1.0 { HEAD_ALL.max(min + 60.0) } else { HEAD_MIN }).max(min)
            };
        }
        if storyboard {
            self.settings.storyboard_head_w = self.montage_head_w;
        }
        if resp.drag_stopped() || resp.double_clicked() {
            if !storyboard {
                self.settings.montage_head_w = self.montage_head_w;
            }
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
