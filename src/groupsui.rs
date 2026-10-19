//! Groupes de couleur des calques (13b-4), comme l'onglet Groupes de TVPaint : une fenêtre
//! qui liste les 13 groupes (couleur, nom, montré ou caché dans la timeline, nombre de calques
//! du plan, et pour chacun : choisir, verrouiller ses calques, cacher les calques des autres
//! groupes…), et le menu « Groupes » des commandes en masse (sur tous les calques, ceux du
//! groupe du calque courant, ou ceux des autres groupes).
//!
//! Cacher un groupe ne touche pas à l'image : ses calques n'ont plus de ligne dans la timeline
//! (`layerheads::rows_of`). Le mot de passe d'un groupe de TVPaint n'est pas repris.

use eframe::egui::{self, vec2, Align2, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles::bulle;
use crate::document::Layer;

/// Sur quels calques agit une commande de groupe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scope {
    All,
    CurrentGroup,
    OtherGroups,
}

impl Scope {
    fn suffix(self) -> &'static str {
        match self {
            Scope::All => "tous les calques",
            Scope::CurrentGroup => "les calques du groupe courant",
            Scope::OtherGroups => "les calques des autres groupes",
        }
    }
}

/// Rangs des calques de `layers` visés par `scope`, le groupe courant étant `group`.
pub(crate) fn layers_in(layers: &[Layer], group: u8, scope: Scope) -> Vec<usize> {
    (0..layers.len())
        .filter(|&i| match scope {
            Scope::All => true,
            Scope::CurrentGroup => layers[i].group == group,
            Scope::OtherGroups => layers[i].group != group,
        })
        .collect()
}

/// Un bouton de menu ; vrai s'il est cliqué (le menu se ferme).
fn item(ui: &mut egui::Ui, text: &str) -> bool {
    let clicked = ui.button(text).clicked();
    if clicked {
        ui.close_menu();
    }
    clicked
}

impl App {
    /// Groupe du calque courant.
    fn current_group(&self) -> u8 {
        self.layers().get(self.cur_layer).map_or(0, |l| l.group)
    }

    /// Montre ou cache le groupe `g` dans la timeline (une étape d'annulation).
    pub(crate) fn set_group_shown(&mut self, g: usize, shown: bool) {
        if self.doc.groups.get(g).is_none_or(|x| x.shown == shown) {
            return;
        }
        self.record_step();
        self.doc.groups[g].shown = shown;
    }

    /// Montre dans la timeline les groupes pour lesquels `show(g)` est vrai, cache les autres.
    fn show_groups(&mut self, show: impl Fn(usize) -> bool, status: &str) {
        self.record_step();
        for (g, group) in self.doc.groups.iter_mut().enumerate() {
            group.shown = show(g);
        }
        self.status = status.into();
    }

    /// Change un réglage des calques visés par `scope` (une étape d'annulation).
    fn set_scope(&mut self, scope: Scope, change: impl Fn(&mut Layer), what: &str) {
        let lis = layers_in(self.layers(), self.current_group(), scope);
        if lis.is_empty() {
            self.status = format!("Aucun calque : {}.", scope.suffix());
            return;
        }
        self.end_stroke();
        self.record_step();
        for li in lis {
            change(&mut self.layers_mut()[li]);
        }
        self.renderer.invalidate_all();
        self.status = format!("{what} : {}.", scope.suffix());
    }

    /// Le calque courant et les calques choisis passent dans le groupe `g`.
    fn apply_group(&mut self, g: usize) {
        let chosen = self.chosen_layers();
        self.record_step();
        for li in chosen {
            self.layers_mut()[li].group = g as u8;
        }
        self.status = format!("Groupe « {} » appliqué.", self.doc.groups[g].name);
    }

    /// Menu « Groupes » : commandes en masse (TVPaint, menu de groupe).
    pub(crate) fn groups_menu(&mut self, ui: &mut egui::Ui) {
        self.action_item(ui, "Éditer les groupes…", "groupes.editer");
        ui.separator();
        let cur = self.current_group() as usize;
        if item(ui, "Afficher tous les groupes") {
            self.show_groups(|_| true, "Tous les groupes sont montrés dans la timeline.");
        }
        if item(ui, "Afficher le groupe courant") {
            self.set_group_shown(cur, true);
        }
        if item(ui, "Cacher le groupe courant") {
            self.set_group_shown(cur, false);
        }
        if item(ui, "Afficher les autres groupes") {
            let shown: Vec<bool> = self.doc.groups.iter().map(|g| g.shown).collect();
            self.show_groups(|g| g != cur || shown[g], "Les autres groupes sont montrés.");
        }
        if item(ui, "Cacher les autres groupes") {
            let shown: Vec<bool> = self.doc.groups.iter().map(|g| g.shown).collect();
            self.show_groups(|g| g == cur && shown[g], "Les autres groupes sont cachés de la timeline.");
        }
        self.action_item(ui, "Focus sur le groupe du calque courant", "groupes.focus");
        ui.separator();
        self.action_item(ui, "Choisir les calques du groupe courant", "calque.choisir_groupe");
        self.action_item(ui, "Choisir les calques des autres groupes", "calque.choisir_autres_groupes");
        ui.separator();
        for (scope, label) in [(Scope::All, "Tous les calques"), (Scope::CurrentGroup, "Groupe courant"), (Scope::OtherGroups, "Autres groupes")] {
            ui.menu_button(label, |ui| {
                if item(ui, "Pelure d'oignon : allumer") {
                    self.set_scope(scope, |l| l.onion = true, "Pelure d'oignon allumée");
                }
                if item(ui, "Pelure d'oignon : éteindre") {
                    self.set_scope(scope, |l| l.onion = false, "Pelure d'oignon éteinte");
                }
                ui.separator();
                if item(ui, "Verrouiller") {
                    self.set_scope(scope, |l| l.locked = true, "Verrouillés");
                }
                if item(ui, "Déverrouiller") {
                    self.set_scope(scope, |l| l.locked = false, "Déverrouillés");
                }
                ui.separator();
                if item(ui, "Préserver la transparence : activer") {
                    self.set_scope(scope, |l| l.keep_alpha = true, "Transparence préservée");
                }
                if item(ui, "Préserver la transparence : désactiver") {
                    self.set_scope(scope, |l| l.keep_alpha = false, "Transparence plus préservée");
                }
                ui.separator();
                if item(ui, "Afficher (activer les calques)") {
                    self.set_scope(scope, |l| l.visible = true, "Affichés");
                }
                if item(ui, "Cacher (désactiver les calques)") {
                    self.set_scope(scope, |l| l.visible = false, "Cachés");
                }
                ui.separator();
                if item(ui, "Pochoir : activer") {
                    self.set_scope(scope, |l| l.stencil = true, "Pochoir activé");
                }
                if item(ui, "Pochoir : désactiver") {
                    self.set_scope(scope, |l| l.stencil = false, "Pochoir désactivé");
                }
                if item(ui, "Pochoir : inverser") {
                    self.set_scope(scope, |l| l.stencil_negative = !l.stencil_negative, "Pochoir inversé");
                }
                if scope != Scope::All {
                    ui.separator();
                    if item(ui, "Fusionner ces calques") {
                        let lis = layers_in(self.layers(), cur as u8, scope);
                        self.merge_layer_list(lis);
                    }
                }
            });
        }
    }

    /// Focus sur le groupe du calque courant : la timeline ne montre que lui.
    pub(crate) fn focus_current_group(&mut self) {
        let cur = self.current_group() as usize;
        let name = self.doc.groups[cur].name.clone();
        self.show_groups(|g| g == cur, &format!("La timeline ne montre que le groupe « {name} »."));
    }

    /// La fenêtre des groupes, si elle est ouverte.
    pub(crate) fn groups_window_ui(&mut self, ctx: &egui::Context) {
        if !self.groups_window {
            return;
        }
        let mut open = true;
        egui::Window::new("Groupes de calques")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_pos(ctx.screen_rect().center() - vec2(200.0, 220.0))
            .pivot(Align2::LEFT_TOP)
            .show(ctx, |ui| self.groups_list(ui));
        if !open {
            self.groups_window = false;
        }
    }

    fn groups_list(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Montrer tous").on_hover_text("Tous les groupes dans la timeline.").clicked() {
                self.show_groups(|_| true, "Tous les groupes sont montrés dans la timeline.");
            }
            if ui.button("Cacher tous").on_hover_text("Aucun groupe dans la timeline.").clicked() {
                self.show_groups(|_| false, "Tous les groupes sont cachés de la timeline.");
            }
            ui.checkbox(&mut self.groups_used_only, "Seulement les couleurs utilisées");
        });
        ui.separator();
        let counts: Vec<usize> = (0..self.doc.groups.len()).map(|g| self.layers().iter().filter(|l| l.group as usize == g).count()).collect();
        let cur = self.current_group() as usize;
        egui::Grid::new("liste_groupes").num_columns(5).spacing([6.0, 4.0]).show(ui, |ui| {
            for g in 0..self.doc.groups.len() {
                if self.groups_used_only && counts[g] == 0 {
                    continue;
                }
                // Couleur et nom (une étape d'annulation par geste).
                let mut color = self.doc.groups[g].color;
                let resp = ui.color_edit_button_srgb(&mut color);
                if self.edited(&resp) {
                    self.doc.groups[g].color = color;
                }
                let mut name = self.doc.groups[g].name.clone();
                let text = if g == cur { RichText::new("●").strong() } else { RichText::new(" ") };
                ui.label(text).on_hover_text("Groupe du calque courant");
                let resp = ui.add_sized([130.0, 18.0], egui::TextEdit::singleline(&mut name));
                if self.edited(&resp) {
                    self.doc.groups[g].name = name;
                }
                // Montré dans la timeline.
                let shown = self.doc.groups[g].shown;
                let icon = RichText::new(if shown { ph::EYE } else { ph::EYE_SLASH }).size(crate::bulles::TAILLE);
                let resp = ui.add(egui::Button::new(icon).frame(false));
                if bulle(resp, "Montrer / cacher dans la timeline", "Les calques du groupe gardent leur image ; seule leur ligne de la timeline disparaît.", "").clicked() {
                    self.set_group_shown(g, !shown);
                }
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{} calque{}", counts[g], if counts[g] > 1 { "s" } else { "" })).weak());
                    let resp = ui.add(egui::Button::new(RichText::new(ph::ARROW_FAT_LINE_LEFT).size(13.0)).frame(false));
                    if bulle(resp, "Appliquer au calque", "Le calque courant et les calques choisis passent dans ce groupe.", "").clicked() {
                        self.apply_group(g);
                    }
                    let resp = ui.menu_button(RichText::new(ph::DOTS_THREE_VERTICAL).size(13.0), |ui| self.group_row_menu(ui, g)).response;
                    bulle(resp, "Commandes du groupe", "Choisir, verrouiller, cacher ou afficher les calques de ce groupe.", "");
                });
                ui.end_row();
            }
        });
    }

    /// Menu d'un groupe dans la fenêtre des groupes.
    fn group_row_menu(&mut self, ui: &mut egui::Ui, g: usize) {
        let ids: Vec<u64> = self.layers().iter().filter(|l| l.group as usize == g).map(|l| l.id).collect();
        let lis: Vec<usize> = (0..self.layers().len()).filter(|&i| self.layers()[i].group as usize == g).collect();
        if item(ui, "Choisir les calques de ce groupe") {
            for id in &ids {
                if !self.layer_sel.contains(id) {
                    self.layer_sel.push(*id);
                }
            }
        }
        if item(ui, "Ne plus choisir les calques de ce groupe") {
            self.layer_sel.retain(|id| !ids.contains(id));
        }
        ui.separator();
        if item(ui, "Verrouiller les calques de ce groupe") {
            self.record_step();
            for &li in &lis {
                self.layers_mut()[li].locked = true;
            }
        }
        if item(ui, "Déverrouiller les calques de ce groupe") {
            self.record_step();
            for &li in &lis {
                self.layers_mut()[li].locked = false;
            }
        }
        ui.separator();
        if item(ui, "Cacher les calques des autres groupes (timeline)") {
            let name = self.doc.groups[g].name.clone();
            self.show_groups(|x| x == g, &format!("La timeline ne montre que le groupe « {name} »."));
        }
        if item(ui, "Activer les calques des autres groupes") {
            self.record_step();
            for l in self.layers_mut().iter_mut().filter(|l| l.group as usize != g) {
                l.visible = true;
            }
        }
        if item(ui, "Désactiver les calques des autres groupes") {
            self.record_step();
            for l in self.layers_mut().iter_mut().filter(|l| l.group as usize != g) {
                l.visible = false;
            }
        }
    }
}
