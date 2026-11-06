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
    avec_version(doc, index, crate::storyboard::panel_title(doc, index))
}

/// `titre_avec_version`, les numéros du film déjà trouvés (un titre par bloc visible du montage, à chaque image d'écran).
pub(crate) fn titre_avec_version_n(doc: &Document, numeros: &crate::numerotation::Numeros, index: usize) -> String {
    avec_version(doc, index, crate::storyboard::panel_title_n(doc, numeros, index))
}

fn avec_version(doc: &Document, index: usize, titre: String) -> String {
    let plan = doc.panels[index].id;
    if doc.nombre_de_versions(plan) > 1 {
        format!("{titre} · {}", etiquette(doc.numero_de_version(plan)))
    } else {
        titre
    }
}

/// Le titre long d'un plan (« Séquence 1 — plan 3 ») et sa version quand il en a plusieurs : « … · V2 ».
pub(crate) fn titre_long_avec_version(doc: &Document, index: usize) -> String {
    let titre = crate::storyboard::panel_long_title(doc, index);
    let plan = doc.panels[index].id;
    if doc.nombre_de_versions(plan) > 1 {
        format!("{titre} · {}", etiquette(doc.numero_de_version(plan)))
    } else {
        titre
    }
}

/// Ce que change une durée de plan qui passe de `avant` à `apres` images quand on change de version (chaque
/// version garde la sienne) : rien si elle est la même ; sinon les plans suivants reculent ou avancent d'autant
/// (comme pour toute durée qui change), le son reste en place.
pub(crate) fn phrase_duree(avant: usize, apres: usize, fps: u32) -> String {
    use crate::storyboard::duration_label;
    if avant == apres {
        return format!("Même durée ({}).", duration_label(apres, fps));
    }
    let (sens, ecart) = if apres > avant { ("reculent", apres - avant) } else { ("avancent", avant - apres) };
    format!(
        "Durée du plan : {} → {} ; les plans suivants {sens} de {ecart} i, le son reste en place.",
        duration_label(avant, fps),
        duration_label(apres, fps)
    )
}

impl App {
    /// Une opération sur les versions du plan `index`, avec une étape d'annulation nommée `name`. `op`
    /// rend faux si rien n'a changé.
    fn version_op(&mut self, name: &str, index: usize, op: impl FnOnce(&mut Document) -> bool) -> bool {
        if self.content_refuses() {
            return false;
        }
        // Un glisser du montage en cours tient une copie du document d'avant : il s'abandonne d'abord (comme Ctrl+Z).
        if self.dragging() {
            self.cancel_drags();
        }
        self.end_stroke();
        self.poser_les_selections();
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
            // Chaque version a sa durée : plus courte, la tête de lecture ne reste pas au-delà de la fin, et ce
            // qui visait des images de l'autre version (clés choisies, sélection) ne vise plus rien de sûr.
            self.after_rognage();
            self.cur_frame = self.cur_frame.min(self.frame_count().saturating_sub(1));
        }
        true
    }

    /// La durée du plan `index` en images (0 s'il n'existe pas).
    fn duree_du_plan(&self, index: usize) -> usize {
        self.doc.panels.get(index).map_or(0, crate::document::Panel::frame_count)
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

    /// « Nouvelle version, en partant d'un plan vide » : le plan `index` est gardé tel qu'il est comme version plus
    /// ancienne, et la nouvelle version active repart d'un calque de dessin vide (même durée, même cadre, même
    /// caméra : voir `Document::nouvelle_version_vide`).
    pub(crate) fn nouvelle_version_vide_du_plan(&mut self, index: usize) {
        let Some(plan) = self.doc.panels.get(index).map(|p| p.id) else { return };
        let ancien = self.doc.numero_de_version(plan);
        let now = crate::recovery::now_secs();
        let mut neuf = 0;
        if self.version_op("Nouvelle version du plan, vide", index, |d| match d.nouvelle_version_vide(index, now) {
            Some(n) => {
                neuf = n;
                true
            }
            None => false,
        }) {
            self.status = format!("Nouvelle version vide du plan : {}, même durée et même caméra. La {} est gardée (« Passer à la version » la ramène).", etiquette(neuf), etiquette(ancien));
        } else if !self.doc.storyboard_locked {
            self.status = "Nouvelle version impossible : le plan a déjà le plus grand nombre de versions.".into();
        }
    }

    /// Passe le plan `index` à sa version rangée `numero`.
    pub(crate) fn passer_a_la_version(&mut self, index: usize, numero: u32) {
        let Some(plan) = self.doc.panels.get(index).map(|p| p.id) else { return };
        let ancien = self.doc.numero_de_version(plan);
        let avant = self.duree_du_plan(index);
        if self.version_op(&format!("Passer à la version {}", etiquette(numero)), index, |d| d.changer_de_version(index, numero)) {
            let duree = phrase_duree(avant, self.duree_du_plan(index), self.doc.fps);
            self.status = format!("Le plan est maintenant en {} ; la {} est gardée. {duree}{}", etiquette(numero), etiquette(ancien), self.note_reserve(plan));
        }
    }

    /// Une copie de la version rangée `numero` devient la version active du plan `index`.
    pub(crate) fn restaurer_la_version(&mut self, index: usize, numero: u32) {
        let Some(plan) = self.doc.panels.get(index).map(|p| p.id) else { return };
        let ancien = self.doc.numero_de_version(plan);
        let now = crate::recovery::now_secs();
        let mut neuf = 0;
        let avant = self.duree_du_plan(index);
        if self.version_op(&format!("Restaurer la version {}", etiquette(numero)), index, |d| match d.restaurer_version(index, numero, now) {
            Some(n) => {
                neuf = n;
                true
            }
            None => false,
        }) {
            let duree = phrase_duree(avant, self.duree_du_plan(index), self.doc.fps);
            self.status = format!("La {} est restaurée en {} ; la {} est gardée. {duree}{}", etiquette(numero), etiquette(neuf), etiquette(ancien), self.note_reserve(plan));
        }
    }

    /// Supprime la version rangée `numero` du plan `index` (une étape d'annulation la ramène).
    pub(crate) fn supprimer_la_version(&mut self, index: usize, numero: u32) {
        let Some(plan) = self.doc.panels.get(index).map(|p| p.id) else { return };
        // Une version qu'une autre version de séquence montre ne se supprime pas (`variantes.rs`).
        let ailleurs = self.doc.version_montree_ailleurs(plan, numero);
        if !ailleurs.is_empty() && !self.content_refuses() {
            let lettres: Vec<String> = ailleurs.iter().map(|n| crate::variantes::lettre(*n)).collect();
            self.status = format!(
                "La version {} est montrée par la version de séquence {} : elle ne se supprime pas. Faites montrer une autre version à celle-ci, ou supprimez-la d'abord.",
                etiquette(numero),
                lettres.join(", ")
            );
            return;
        }
        if self.version_op(&format!("Supprimer la version {}", etiquette(numero)), index, |d| d.supprimer_version(plan, numero)) {
            self.status = format!("Version {} supprimée (Ctrl+Z la ramène).", etiquette(numero));
        }
    }

    /// Les commandes `version.*` ; vrai si `action` en était une.
    pub(crate) fn version_action(&mut self, action: &str) -> bool {
        let index = self.cur_panel;
        match action {
            "version.nouvelle" => self.nouvelle_version_du_plan(index),
            "version.vide" => self.nouvelle_version_vide_du_plan(index),
            "version.purger" => self.purge_versions_demander(None),
            "version.purger_plan" => {
                let plan = self.doc.panels.get(index).map(|p| p.id);
                self.purge_versions_demander(plan);
            }
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
        let autres: Vec<(u32, u64, usize)> = self.doc.versions_du_plan(plan).map(|v| v.autres.iter().map(|r| (r.numero, r.date, r.panel.frame_count())).collect()).unwrap_or_default();
        let fps = self.doc.fps;
        let duree_active = crate::storyboard::duration_label(self.duree_du_plan(index), fps);
        let (mut passer, mut restaurer, mut supprimer, mut nouvelle, mut vide, mut comparer) = (None, None, None, false, false, None);
        let sequence = self.doc.sequence_of(index);
        let ailleurs = !self.doc.variantes_montrant(sequence, plan).is_empty();
        let mut mettre_a_jour = false;
        let parts: Vec<String> = autres.iter().map(|(n, _, _)| crate::variantesui::montree_par(&self.doc, sequence, plan, *n)).collect();
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(etiquette(actuelle)).strong()).on_hover_text("La version active du plan : celle que montrent le montage, l'export et les envois.");
                if !autres.is_empty() {
                    ui.label(RichText::new(format!("sur {} · {duree_active}", autres.len() + 1)).color(theme::texte_doux()))
                        .on_hover_text("Chaque version a sa durée : changer de version change la durée du plan dans le montage.");
                }
                let r = self.raccourci("version.nouvelle");
                if bulles::icone(ui, ph::GIT_FORK, "Nouvelle version, en dupliquant", "Garde le plan tel qu'il est comme version plus ancienne et continue sur une copie.", &r).clicked() {
                    nouvelle = true;
                }
                let r = self.raccourci("version.vide");
                if bulles::icone(ui, ph::FILE_PLUS, "Nouvelle version, plan vide", "Garde le plan tel qu'il est et repart d'un calque vide : même durée, même cadre, même caméra.", &r).clicked() {
                    vide = true;
                }
                if ailleurs && bulles::icone(ui, ph::GIT_MERGE, "Mettre à jour dans les autres versions de séquence", "Les autres versions de la séquence où ce plan apparaît (celles que tu coches) montreront la version d'aujourd'hui.", &self.raccourci("variante.mettre_a_jour")).clicked() {
                    mettre_a_jour = true;
                }
            });
            for (k, &(numero, date, duree)) in autres.iter().enumerate() {
                ui.horizontal(|ui| {
                    let quand = if date > 0 { format!(" · {}", date_courte(date)) } else { String::new() };
                    let duree = crate::storyboard::duration_label(duree, fps);
                    let par = if parts[k].is_empty() { String::new() } else { format!(" · {}", parts[k]) };
                    ui.label(RichText::new(format!("{}{quand} · {duree}{par}", etiquette(numero))).color(theme::texte_doux()));
                    if bulles::icone(ui, ph::SWAP, "Passer à cette version", "Elle devient la version active ; la version d'aujourd'hui est gardée.", "").clicked() {
                        passer = Some(numero);
                    }
                    if bulles::icone(ui, ph::COLUMNS, "Comparer à la version d'aujourd'hui", "Côte à côte, en volet, en fondu ou en superposition (lecture seule).", "").clicked() {
                        comparer = Some(numero);
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
        if vide {
            self.nouvelle_version_vide_du_plan(index);
        }
        if mettre_a_jour {
            self.mettre_a_jour_demander(index);
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
        if let Some(n) = comparer {
            self.comparer_ouvrir(index, Some(crate::comparer::Source::Version(n)), Some(crate::comparer::Source::Actuelle));
        }
    }

    /// Les deux « Nouvelle version » du clic droit sur un plan : en dupliquant la version en cours, ou en partant d'un
    /// plan vide. Directement dans « Version du plan », sans sous-menu de plus (Anthony, 2026-10-28).
    pub(crate) fn nouvelle_version_menu(&mut self, ui: &mut egui::Ui, index: usize) {
        let r = self.raccourci("version.nouvelle");
        let b = ui.add(egui::Button::new("Nouvelle version (copie de celle-ci)").shortcut_text(r));
        if bulles::bulle(b, "Nouvelle version, en dupliquant", "Garde le plan tel qu'il est comme version plus ancienne et continue sur une copie.", "").clicked() {
            ui.close_menu();
            self.nouvelle_version_du_plan(index);
        }
        let r = self.raccourci("version.vide");
        let b = ui.add(egui::Button::new("Nouvelle version vide").shortcut_text(r));
        if bulles::bulle(b, "Nouvelle version vide", "Garde le plan tel qu'il est et repart d'un calque de dessin vide : même durée, même cadre, même caméra.", "").clicked() {
            ui.close_menu();
            self.nouvelle_version_vide_du_plan(index);
        }
    }

    /// Le sous-menu « Version du plan » du clic droit sur le plan `index` ; `avec_nouvelle` : il porte aussi
    /// « Nouvelle version › » (la bande l'a déjà au premier niveau du menu).
    pub(crate) fn version_menu(&mut self, ui: &mut egui::Ui, index: usize, avec_nouvelle: bool) {
        let Some(plan) = self.doc.panels.get(index).map(|p| p.id) else { return };
        let actuelle = self.doc.numero_de_version(plan);
        let autres: Vec<(u32, u64, usize)> = self.doc.versions_du_plan(plan).map(|v| v.autres.iter().map(|r| (r.numero, r.date, r.panel.frame_count())).collect()).unwrap_or_default();
        let fps = self.doc.fps;
        let sequence = self.doc.sequence_of(index);
        let ailleurs = !self.doc.variantes_montrant(sequence, plan).is_empty();
        ui.menu_button(format!("Version du plan ({})", etiquette(actuelle)), |ui| {
            if avec_nouvelle {
                self.nouvelle_version_menu(ui, index);
            }
            for (k, (numero, date, duree)) in autres.into_iter().enumerate() {
                if avec_nouvelle || k > 0 {
                    ui.separator();
                }
                let quand = if date > 0 { format!(" · {}", date_courte(date)) } else { String::new() };
                let duree = crate::storyboard::duration_label(duree, fps);
                let par = crate::variantesui::montree_par(&self.doc, sequence, plan, numero);
                let par = if par.is_empty() { par } else { format!(" · {par}") };
                ui.label(RichText::new(format!("{}{quand} · {duree}{par}", etiquette(numero))).color(theme::texte_doux()));
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
            ui.separator();
            if ui.button("Comparer le plan…").on_hover_text("Deux versions, ou aujourd'hui et un envoi (lecture seule).").clicked() {
                ui.close_menu();
                self.comparer_ouvrir(index, None, None);
            }
            if ailleurs {
                let b = ui.button("Mettre à jour dans les autres versions de séquence…");
                if bulles::bulle(b, "Mettre à jour dans les autres versions de séquence", "Les autres versions de la séquence où ce plan apparaît montreront sa version d'aujourd'hui (celles que tu coches).", "").clicked() {
                    ui.close_menu();
                    self.mettre_a_jour_demander(index);
                }
            }
        });
    }

    // ------------------------------------------------------------------ supprimer les versions non affichées

    /// Les deux entrées « Supprimer les versions non affichées… » (tout le film, le plan courant), grisées quand il
    /// n'y a rien à supprimer.
    pub(crate) fn purge_items(&mut self, ui: &mut egui::Ui) {
        let courant = self.doc.panels.get(self.cur_panel).map(|p| p.id);
        for (texte, action, seulement, bulle) in [
            ("Supprimer les versions non affichées…", "version.purger", None, "Toutes les versions rangées de tous les plans ; la version affichée de chaque plan reste. Une confirmation dit combien. Les états gardés par les envois (dossier versions/) ne sont pas touchés. Ctrl+Z les ramène."),
            ("Supprimer les versions non affichées du plan courant…", "version.purger_plan", courant, "Seulement les autres versions du plan courant. Les états gardés par les envois ne sont pas touchés. Ctrl+Z les ramène."),
        ] {
            let rien = self.doc.versions_non_affichees(seulement).0 == 0;
            let r = self.raccourci(action);
            let b = ui.add_enabled(!rien, egui::Button::new(texte).shortcut_text(r));
            let clique = b.clicked();
            let droit = b.secondary_clicked();
            bulles::bulle(b, texte.trim_end_matches('…'), bulle, "");
            if clique {
                ui.close_menu();
                self.run_action(action);
            } else if droit {
                ui.close_menu();
                self.start_assign(action);
            }
        }
    }

    /// Demande confirmation avant de supprimer les versions non affichées : de ce plan (`Some`) ou de tout le film.
    pub(crate) fn purge_versions_demander(&mut self, seulement: Option<u64>) {
        if self.content_refuses() {
            return;
        }
        if self.doc.versions_non_affichees(seulement).0 == 0 {
            self.status = "Aucune version à supprimer : chaque plan n'a que sa version affichée.".into();
            return;
        }
        self.purge_versions = Some(seulement);
    }

    /// Supprime les versions non affichées (une étape d'annulation).
    pub(crate) fn purge_versions_faire(&mut self, seulement: Option<u64>) {
        let mut compte = (0, 0);
        let index = self.cur_panel;
        if self.version_op("Supprimer les versions non affichées", index, |d| {
            compte = d.supprimer_versions_non_affichees(seulement);
            compte.0 > 0
        }) {
            self.status = format!("{} (Ctrl+Z les ramène).", phrase_purge(compte, true));
        }
    }

    /// La confirmation « 7 versions de 4 plans seront supprimées ».
    pub(crate) fn purge_versions_fenetre(&mut self, ctx: &egui::Context) {
        let Some(seulement) = self.purge_versions else { return };
        let compte = self.doc.versions_non_affichees(seulement);
        if compte.0 == 0 || self.doc.storyboard_locked {
            self.purge_versions = None;
            return;
        }
        let (mut faire, mut fermer) = (false, false);
        egui::Window::new("Supprimer les versions non affichées").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0)).show(ctx, |ui| {
            ui.set_max_width(380.0);
            ui.label(RichText::new(phrase_purge(compte, false)).strong());
            ui.label("La version affichée de chaque plan reste telle quelle. Les états gardés par les envois (dossier versions/) ne sont pas touchés. Ctrl+Z ramène les versions supprimées.");
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(RichText::new("Supprimer").color(theme::erreur())).stroke(egui::Stroke::new(1.0_f32, theme::erreur()))).clicked() {
                    faire = true;
                }
                if ui.button("Annuler").clicked() {
                    fermer = true;
                }
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            fermer = true;
        }
        if faire {
            self.purge_versions = None;
            self.purge_versions_faire(seulement);
        } else if fermer {
            self.purge_versions = None;
        }
    }
}

/// « 7 versions de 4 plans seront supprimées » (`fait` faux) ou « 7 versions de 4 plans supprimées » (`fait`).
pub(crate) fn phrase_purge((versions, plans): (usize, usize), fait: bool) -> String {
    let v = if versions > 1 { "versions" } else { "version" };
    let p = if plans > 1 { "plans" } else { "plan" };
    let fin = match (fait, versions > 1) {
        (false, true) => " seront supprimées",
        (false, false) => " sera supprimée",
        (true, true) => " supprimées",
        (true, false) => " supprimée",
    };
    format!("{versions} {v} de {plans} {p}{fin}")
}
