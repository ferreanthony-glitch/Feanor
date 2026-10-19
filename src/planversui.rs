//! Les versions d'un plan dans l'interface (`planvers.rs` pour le modèle et le fichier) : « Nouvelle
//! version du plan », passer d'une version à l'autre, restaurer, supprimer ; la ligne « Version » de la fiche
//! du plan, le sous-menu du clic droit sur un plan.
//!
//! **Un geste = une étape d'annulation, nommée** (panneau Historique) : « Nouvelle version du plan »,
//! « Passer à la version v1 », « Restaurer la version v1 », « Supprimer la version v1 ». Chacune copie la
//! structure du document (pas les dessins) avant d'agir, comme toutes les opérations sur les plans
//! (`App::snapshot`, `commit_structure`) ; aucune ne touche un pixel sur le fil de l'interface.
//!
//! Le storyboard verrouillé refuse tout cela (les dessins et les calques des plans ne changent plus).

use eframe::egui::{self, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles;
use crate::document::Document;
use crate::envois::date_courte;
use crate::planvers::etiquette;
use crate::theme;

/// Le titre d'un plan dans la bande et sur la planche : « Plan 3 », et « Plan 3 · v2 » quand il a plusieurs
/// versions.
pub(crate) fn titre_avec_version(doc: &Document, index: usize) -> String {
    let titre = crate::storyboard::panel_title(doc, index);
    let plan = doc.panels[index].id;
    if doc.nombre_de_versions(plan) > 1 {
        format!("{titre} · {}", etiquette(doc.numero_de_version(plan)))
    } else {
        titre
    }
}

impl App {
    /// Une opération sur les versions du plan `index`, avec une étape d'annulation nommée `name`. `op`
    /// rend faux si rien n'a changé.
    fn version_op(&mut self, name: &str, index: usize, op: impl FnOnce(&mut Document) -> bool) -> bool {
        if self.content_refuses() {
            return false;
        }
        self.end_stroke();
        let snap = self.snapshot();
        if !op(&mut self.doc) {
            return false;
        }
        let before = std::mem::replace(&mut self.history.context, Some(name.to_string()));
        self.commit_structure(snap);
        self.history.context = before;
        // Le plan courant garde sa place, mais ses calques ne sont plus les mêmes : la navigation se recale.
        if self.cur_panel == index {
            self.clamp_nav();
        }
        true
    }

    /// Ce qui reste attaché au plan et non à la version : on le dit.
    fn note_reserve(&self, plan: u64) -> &'static str {
        if self.doc.reserves.contains_key(&plan) {
            " Le contenu rogné reste attaché au plan, pas à la version."
        } else {
            ""
        }
    }

    /// « Nouvelle version du plan » : le plan `index` est gardé tel qu'il est comme version plus ancienne, et on
    /// continue sur une copie.
    pub(crate) fn nouvelle_version_du_plan(&mut self, index: usize) {
        let Some(plan) = self.doc.panels.get(index).map(|p| p.id) else { return };
        let ancien = self.doc.numero_de_version(plan);
        let now = crate::recovery::now_secs();
        let mut neuf = 0;
        if self.version_op("Nouvelle version du plan", index, |d| match d.nouvelle_version(index, now) {
            Some(n) => {
                neuf = n;
                true
            }
            None => false,
        }) {
            self.status = format!("Nouvelle version du plan : {}. La {} est gardée (« Passer à la version » la ramène).", etiquette(neuf), etiquette(ancien));
        } else if !self.doc.storyboard_locked {
            self.status = "Nouvelle version impossible : le plan a déjà le plus grand nombre de versions.".into();
        }
    }

    /// Passe le plan `index` à sa version rangée `numero`.
    pub(crate) fn passer_a_la_version(&mut self, index: usize, numero: u32) {
        let Some(plan) = self.doc.panels.get(index).map(|p| p.id) else { return };
        let ancien = self.doc.numero_de_version(plan);
        if self.version_op(&format!("Passer à la version {}", etiquette(numero)), index, |d| d.changer_de_version(index, numero)) {
            self.status = format!("Le plan est maintenant en {} ; la {} est gardée.{}", etiquette(numero), etiquette(ancien), self.note_reserve(plan));
        }
    }

    /// Une copie de la version rangée `numero` devient la version active du plan `index`.
    pub(crate) fn restaurer_la_version(&mut self, index: usize, numero: u32) {
        let Some(plan) = self.doc.panels.get(index).map(|p| p.id) else { return };
        let ancien = self.doc.numero_de_version(plan);
        let now = crate::recovery::now_secs();
        let mut neuf = 0;
        if self.version_op(&format!("Restaurer la version {}", etiquette(numero)), index, |d| match d.restaurer_version(index, numero, now) {
            Some(n) => {
                neuf = n;
                true
            }
            None => false,
        }) {
            self.status = format!("La {} est restaurée en {} ; la {} est gardée.{}", etiquette(numero), etiquette(neuf), etiquette(ancien), self.note_reserve(plan));
        }
    }

    /// Supprime la version rangée `numero` du plan `index` (une étape d'annulation la ramène).
    pub(crate) fn supprimer_la_version(&mut self, index: usize, numero: u32) {
        let Some(plan) = self.doc.panels.get(index).map(|p| p.id) else { return };
        if self.version_op(&format!("Supprimer la version {}", etiquette(numero)), index, |d| d.supprimer_version(plan, numero)) {
            self.status = format!("Version {} supprimée (Ctrl+Z la ramène).", etiquette(numero));
        }
    }

    /// Les commandes `version.*` ; vrai si `action` en était une.
    pub(crate) fn version_action(&mut self, action: &str) -> bool {
        let index = self.cur_panel;
        match action {
            "version.nouvelle" => self.nouvelle_version_du_plan(index),
            "version.precedente" | "version.suivante" => {
                let Some(plan) = self.doc.panels.get(index).map(|p| p.id) else { return true };
                match self.doc.version_voisine(plan, if action == "version.suivante" { 1 } else { -1 }) {
                    Some(n) if n != self.doc.numero_de_version(plan) => self.passer_a_la_version(index, n),
                    _ => self.status = "Ce plan n'a qu'une version : « Nouvelle version du plan » en crée une autre.".into(),
                }
            }
            _ => return false,
        }
        true
    }

    // ------------------------------------------------------------------ la fiche et le menu

    /// La ligne « Version » de la fiche du plan : la version active, un bouton pour en créer une, et pour chaque
    /// autre version de quoi y passer, la restaurer ou la supprimer.
    pub(crate) fn version_ligne(&mut self, ui: &mut egui::Ui) {
        let index = self.cur_panel;
        let Some(plan) = self.doc.panels.get(index).map(|p| p.id) else { return };
        let actuelle = self.doc.numero_de_version(plan);
        let autres: Vec<(u32, u64)> = self.doc.versions_du_plan(plan).map(|v| v.autres.iter().map(|r| (r.numero, r.date)).collect()).unwrap_or_default();
        let (mut passer, mut restaurer, mut supprimer, mut nouvelle) = (None, None, None, false);
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(etiquette(actuelle)).strong()).on_hover_text("La version active du plan : celle que montrent le montage, l'export et les envois.");
                if !autres.is_empty() {
                    ui.label(RichText::new(format!("sur {}", autres.len() + 1)).color(theme::texte_doux()));
                }
                let r = self.raccourci("version.nouvelle");
                if bulles::icone(ui, ph::GIT_FORK, "Nouvelle version du plan", "Garde le plan tel qu'il est comme version plus ancienne et continue sur une copie.", &r).clicked() {
                    nouvelle = true;
                }
            });
            for &(numero, date) in &autres {
                ui.horizontal(|ui| {
                    let quand = if date > 0 { format!(" · {}", date_courte(date)) } else { String::new() };
                    ui.label(RichText::new(format!("{}{quand}", etiquette(numero))).color(theme::texte_doux()));
                    if bulles::icone(ui, ph::SWAP, "Passer à cette version", "Elle devient la version active ; la version d'aujourd'hui est gardée.", "").clicked() {
                        passer = Some(numero);
                    }
                    if bulles::icone(ui, ph::ARROW_COUNTER_CLOCKWISE, "Restaurer cette version", "Une copie d'elle devient une nouvelle version active ; rien n'est perdu.", "").clicked() {
                        restaurer = Some(numero);
                    }
                    if bulles::icone(ui, ph::TRASH, "Supprimer cette version", "Elle sort du plan (Ctrl+Z la ramène).", "").clicked() {
                        supprimer = Some(numero);
                    }
                });
            }
        });
        if nouvelle {
            self.nouvelle_version_du_plan(index);
        }
        if let Some(n) = passer {
            self.passer_a_la_version(index, n);
        }
        if let Some(n) = restaurer {
            self.restaurer_la_version(index, n);
        }
        if let Some(n) = supprimer {
            self.supprimer_la_version(index, n);
        }
    }

    /// Le sous-menu « Version du plan » du clic droit sur le plan `index`.
    pub(crate) fn version_menu(&mut self, ui: &mut egui::Ui, index: usize) {
        let Some(plan) = self.doc.panels.get(index).map(|p| p.id) else { return };
        let actuelle = self.doc.numero_de_version(plan);
        let autres: Vec<(u32, u64)> = self.doc.versions_du_plan(plan).map(|v| v.autres.iter().map(|r| (r.numero, r.date)).collect()).unwrap_or_default();
        ui.menu_button(format!("Version du plan ({})", etiquette(actuelle)), |ui| {
            if ui.button("Nouvelle version du plan").on_hover_text("Garde le plan tel qu'il est et continue sur une copie.").clicked() {
                ui.close_menu();
                self.nouvelle_version_du_plan(index);
            }
            for (numero, date) in autres {
                ui.separator();
                let quand = if date > 0 { format!(" · {}", date_courte(date)) } else { String::new() };
                ui.label(RichText::new(format!("{}{quand}", etiquette(numero))).color(theme::texte_doux()));
                if ui.button(format!("Passer à la {}", etiquette(numero))).clicked() {
                    ui.close_menu();
                    self.passer_a_la_version(index, numero);
                }
                if ui.button(format!("Restaurer la {}", etiquette(numero))).on_hover_text("Une copie d'elle devient la version active.").clicked() {
                    ui.close_menu();
                    self.restaurer_la_version(index, numero);
                }
                if ui.button(format!("Supprimer la {}", etiquette(numero))).clicked() {
                    ui.close_menu();
                    self.supprimer_la_version(index, numero);
                }
            }
        });
    }
}
