//! Clic droit dans le dessin : les réglages de l'outil sous le curseur (Anthony, 2026-10-28 : « clic droit avec n'importe quel
//! outil ouvre une fenêtre flottante des réglages, qui se referme aussitôt qu'on a cliqué en dehors de la fenêtre »).
//!
//! La fenêtre n'a pas de réglages à elle : elle dessine le panneau Réglages de l'outil (`App::pane_ui`, `Pane::ToolOptions`),
//! donc un changement fait ici est celui du panneau, et inversement. Elle s'ouvre sur un clic droit « dans le vide » du dessin :
//! - l'outil Sélection (et la sélection par couleur) et la Pipette gardent le clic droit d'avant (le menu du dessin) ;
//! - dans les Vignettes, un clic droit sur une case garde le menu de la case, qui a une entrée de plus pour ouvrir la fenêtre ;
//! - les menus des cases, des barres, des notes, de la timeline, des icônes, des pastilles et des panneaux ne changent pas.
//!
//! Fermer : un clic (ou l'appui du stylet) n'importe où hors de la fenêtre, un nouveau clic droit, ou Échap. Ce clic ne fait rien
//! d'autre : une zone transparente sur tout l'écran, sous la fenêtre et par-dessus le reste, le prend (comme `egui::Modal`), si bien que
//! le dessin, la planche et les panneaux ne le voient pas (ils regardent quelle couche est sous le pointeur).
//! Rien n'est recalculé à l'ouverture : la fenêtre est une image de plus du panneau, et au repos elle ne demande aucune repeinture.

use eframe::egui::{self, pos2, vec2, Event, Id, Key, LayerId, Modifiers, Order, Pos2, Rect, Sense, TouchPhase};

use crate::app::{App, Tool};
use crate::dock::Pane;

/// Largeur du contenu : celle de la colonne des réglages (350, moins ses marges).
const LARGEUR: f32 = 330.0;
/// Marge gardée entre la fenêtre et le bord de l'écran.
const MARGE: f32 = 6.0;

/// La fenêtre ouverte : où le clic droit est tombé, et ce qu'on sait d'elle après sa dernière image.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ReglagesDroit {
    /// Le point du clic droit (le coin haut gauche de la fenêtre, avant de la ramener dans l'écran).
    pub pos: Pos2,
    /// Sa place à l'écran à la dernière image (rien avant la première : elle est alors peinte invisible, pour mesurer sa taille).
    pub rect: Option<Rect>,
}

fn couche() -> LayerId {
    LayerId::new(Order::Foreground, Id::new("reglages_clic_droit"))
}

impl App {
    /// Le clic droit dans le dessin ouvre les réglages (tous les outils, sauf la Sélection et la Pipette), hors Espace 3D (qui a
    /// ses commandes et son clic droit de caméra) et hors boîte de dialogue.
    pub(crate) fn reglages_droit_permis(&self) -> bool {
        !self.tool.is_select() && self.tool != Tool::Picker && !(self.espace3d.on && !self.camera_view) && !self.modal_open()
    }

    /// Ouvre la fenêtre sous `pos` (ou la fait venir sous le nouveau point).
    pub(crate) fn ouvrir_reglages_droit(&mut self, pos: Pos2) {
        if self.reglages_droit_permis() {
            self.reglages_droit = Some(ReglagesDroit { pos, rect: None });
        }
    }

    /// Le clic droit du dessin vient d'avoir lieu (`response.secondary_clicked()`) : ouvre la fenêtre, sauf pendant un trait.
    pub(crate) fn clic_droit_dans_le_dessin(&mut self, pos: Option<Pos2>) {
        if self.stroke.is_some() || self.erase_pen().is_some() {
            return;
        }
        if let Some(pos) = pos {
            self.ouvrir_reglages_droit(pos);
        }
    }

    /// Échap ferme la fenêtre, et ne fait rien d'autre : à appeler avant les raccourcis. (Un menu ou une liste ouverts dans la fenêtre
    /// se ferment d'abord, avec le même Échap.)
    pub(crate) fn reglages_droit_echap(&mut self, ctx: &egui::Context) {
        if self.reglages_droit.is_some() && !ctx.memory(|m| m.any_popup_open()) && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.reglages_droit = None;
        }
    }

    /// La fenêtre, une fois par image (après les panneaux). Peinte en dernier : elle est au-dessus de tout, ses menus et ses listes
    /// (qui naissent plus tard) au-dessus d'elle.
    pub(crate) fn reglages_droit_fenetre(&mut self, ctx: &egui::Context) {
        let Some(etat) = self.reglages_droit else { return };
        // Un outil qui n'ouvre pas la fenêtre (touche de la Sélection…), une boîte de dialogue : elle se ferme.
        if !self.reglages_droit_permis() {
            self.reglages_droit = None;
            return;
        }
        let ecran = ctx.screen_rect();
        // Un appui hors de la fenêtre (souris, stylet ou clic droit) la ferme ; un appui sur ce qui est au-dessus d'elle (un menu, une liste
        // ouverts depuis la fenêtre) ou dedans, non. Seulement une fois la fenêtre peinte : l'appui qui l'ouvre ne la ferme pas.
        if let Some(rect) = etat.rect {
            let events = ctx.input(|i| i.events.clone());
            let dehors = events.iter().any(|e| {
                let pos = match e {
                    Event::PointerButton { pos, pressed: true, .. } | Event::Touch { pos, phase: TouchPhase::Start, .. } => *pos,
                    _ => return false,
                };
                ctx.layer_id_at(pos) == Some(couche()) && !rect.contains(pos)
            });
            if dehors {
                self.reglages_droit = None;
                return;
            }
        }
        // Première image : sa taille n'est pas connue, elle est peinte invisible puis montrée à l'image suivante, à sa place.
        let mesure = etat.rect.map(|r| r.size());
        let taille = mesure.unwrap_or(vec2(LARGEUR + 24.0, 0.0));
        let haut_max = (ecran.height() - 2.0 * MARGE).max(120.0);
        let place = pos2(
            etat.pos.x.min(ecran.right() - taille.x - MARGE).max(ecran.left() + MARGE),
            etat.pos.y.min(ecran.bottom() - taille.y.min(haut_max) - MARGE).max(ecran.top() + MARGE),
        );
        let mut rect = None;
        egui::Area::new(Id::new("reglages_clic_droit"))
            .order(Order::Foreground)
            .fixed_pos(ecran.min)
            .constrain(false)
            .show(ctx, |ui| {
                // La zone transparente qui prend le clic du dehors (sous la fenêtre : ses réglages sont posés après elle).
                ui.allocate_rect(ecran, Sense::click_and_drag());
                let mut fenetre = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(place, vec2(LARGEUR + 24.0, haut_max))));
                if mesure.is_none() {
                    fenetre.set_invisible();
                }
                let cadre = egui::Frame::popup(ui.style()).show(&mut fenetre, |ui| {
                    ui.set_width(LARGEUR);
                    // Trop haute pour l'écran : elle défile.
                    egui::ScrollArea::vertical().id_salt("reglages_clic_droit_defile").max_height(haut_max - 24.0).auto_shrink([false, true]).show(ui, |ui| {
                        self.pane_ui(ui, Pane::ToolOptions);
                    });
                });
                rect = Some(cadre.response.rect);
            });
        let Some(rect) = rect else { return };
        if mesure.is_none() {
            ctx.request_repaint();
        }
        if let Some(e) = self.reglages_droit.as_mut() {
            e.rect = Some(rect);
        }
    }

}
