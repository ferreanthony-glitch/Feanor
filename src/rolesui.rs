//! Rôles des images (13g-2) dans l'interface : poser ou retirer un rôle sur les images visées
//! (les images choisies des calques choisis, sinon la case courante), par le clic droit de la
//! timeline ou de la feuille d'exposition, ou par le clavier ; montrer les symboles dans la
//! timeline (tous les calques ou le calque courant seulement, à l'opacité choisie).
//!
//! Un rôle n'est qu'un repère : permis sous le storyboard verrouillé, comme les marques de
//! couleur.

use eframe::egui;

use crate::app::App;
use crate::roles::Role;

/// Réglages d'affichage des rôles dans la timeline (TVPaint : afficher / cacher les rôles, sur
/// le calque courant seulement, transparence des rôles).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RolesView {
    pub shown: bool,
    pub current_only: bool,
    pub opacity: f32,
}

impl Default for RolesView {
    fn default() -> Self {
        Self { shown: true, current_only: false, opacity: 1.0 }
    }
}

impl App {
    /// Pose `role` (rien : retire le rôle) sur les images visées des calques choisis (une étape
    /// d'annulation). Les images vides ont un rôle comme les autres.
    pub(crate) fn set_role(&mut self, role: Option<Role>) {
        let (a, b) = self.image_range();
        let b = b.min(self.frame_count().max(a + 1));
        let lis: Vec<usize> = self.chosen_layers().into_iter().filter(|&li| !self.layers()[li].folder).collect();
        if lis.is_empty() {
            self.status = crate::app::FOLDER_NOT_DRAWABLE.into();
            return;
        }
        self.record_step();
        for &li in &lis {
            let roles = &mut self.layers_mut()[li].roles;
            for f in a..b {
                match role {
                    Some(r) => {
                        roles.insert(f, r);
                    }
                    None => {
                        roles.remove(&f);
                    }
                }
            }
        }
        let n = (b - a) * lis.len();
        let s = if n > 1 { "s" } else { "" };
        self.status = match role {
            Some(r) => format!("Rôle « {} » posé sur {n} image{s}.", r.label()),
            None => format!("Rôle retiré de {n} image{s}."),
        };
    }

    /// Le menu des rôles (clic droit dans la timeline ou la feuille d'exposition).
    pub(crate) fn roles_menu(&mut self, ui: &mut egui::Ui) {
        let (a, b) = self.image_range();
        let what = if b - a > 1 { format!("les {} images choisies", b - a) } else { "l'image courante".into() };
        ui.label(egui::RichText::new(format!("Rôle de {what}, calques choisis :")).weak());
        for r in Role::ALL {
            self.action_item(ui, r.label(), r.action());
        }
        self.action_item(ui, "Aucun rôle", "role.aucun");
        ui.separator();
        ui.checkbox(&mut self.roles_view.shown, "Montrer les rôles dans la timeline");
        ui.add_enabled(self.roles_view.shown, egui::Checkbox::new(&mut self.roles_view.current_only, "Sur le calque courant seulement"));
        ui.add_enabled_ui(self.roles_view.shown, |ui| {
            ui.horizontal(|ui| {
                ui.label("Opacité des rôles");
                ui.add(egui::Slider::new(&mut self.roles_view.opacity, 0.1..=1.0).show_value(false));
            });
        });
    }

    /// Clic droit sur la case (`li`, `f`) : elle devient la case visée, sauf si elle fait déjà
    /// partie des images choisies des calques choisis (le menu vise alors tout le choix).
    pub(crate) fn aim_cell(&mut self, li: usize, f: usize) {
        let (a, b) = self.image_range();
        let inside = f >= a && f < b && self.chosen_layers().contains(&li);
        if !inside {
            self.select_layer(li);
            self.frame_sel = None;
            self.set_frame_free(f);
        }
    }
}
