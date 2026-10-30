//! Le menu d'un calque, **le même partout** (clic droit dans le panneau Calques, dans la timeline, dans le montage ;
//! le « … » du panneau, le ⋮ des lignes, le menu Calques de la barre et le ☰ de la timeline) : une dizaine d'entrées au
//! premier niveau, en groupes séparés.
//!
//! ```text
//!   Nouveau calque ▸   Dupliquer   Renommer…   Supprimer
//!   ───────────────
//!   Fusionner vers le bas        Fusionner les calques choisis
//!   ───────────────
//!   Organiser ▸   Apparence ▸   Effets ▸   Ajouter un aplat
//!   ───────────────
//!   Plus ▸
//! ```
//!
//! - **Organiser** : monter, descendre, aller à un calque, dossiers, groupes.
//! - **Apparence** : afficher, verrouiller, opacité, mode de fusion, couleur d'étiquette, pelure d'oignon.
//! - **Plus** : ce qui est rare ou propre à une sorte de calque (presse-papiers, retourner, réglages avancés, calque de
//!   couleur, vidéo, avant / après les images, propagation, choisir, afficher, pochoirs, nettoyer, fusions en masse…).
//!
//! Chaque entrée qui lance une commande montre son raccourci et propose « Assigner un raccourci » au clic droit
//! (`entree`). Une entrée qui ne peut pas servir est **grisée avec sa raison en bulle** (jamais un refus muet) : c'est le
//! cas des fusions (`fusioncalques.rs`). Toutes les commandes restent dans la palette (Ctrl+K).

use eframe::egui;
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::blend::BlendMode;

/// D'où le menu s'ouvre : seules quelques entrées en dépendent (la place du champ de renommage, les pistes du montage,
/// la taille de la ligne dans la timeline).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Origine {
    /// Clic droit d'une ligne du panneau Calques, et son « … » (le calque courant).
    Panneau,
    /// Clic droit ou ⋮ de l'en-tête d'une ligne de la timeline.
    Timeline,
    /// Clic droit ou ⋮ de la colonne des calques du montage, « Calque » d'une piste.
    Montage,
    /// Menu Calques de la barre, ☰ de la timeline : le calque courant.
    Barre,
}

/// Combien d'entrées au plus au premier niveau (« pas plus d'une douzaine »).
#[cfg(test)]
pub(crate) const PREMIER_NIVEAU: usize = 12;

impl App {
    /// Menu Calques de la barre et ☰ de la timeline : le menu du calque courant.
    pub(crate) fn layer_ops_menu(&mut self, ui: &mut egui::Ui) {
        let li = self.cur_layer;
        self.layer_menu(ui, li, Origine::Barre);
    }

    /// Le « … » du panneau Calques : le menu du calque courant.
    pub(crate) fn layer_more_menu(&mut self, ui: &mut egui::Ui) {
        let li = self.cur_layer;
        self.layer_menu(ui, li, Origine::Panneau);
    }

    /// Clic droit d'un calque du panneau Calques (ou, avec `in_montage`, de la colonne du montage).
    pub(crate) fn layer_full_menu(&mut self, ui: &mut egui::Ui, li: usize, in_montage: bool) {
        self.layer_menu(ui, li, if in_montage { Origine::Montage } else { Origine::Panneau });
    }

    /// Clic droit ou ⋮ d'une ligne de la timeline.
    pub(crate) fn layer_head_menu(&mut self, ui: &mut egui::Ui, li: usize) {
        self.layer_menu(ui, li, Origine::Timeline);
    }

    /// Clic droit ou ⋮ de la colonne des calques du montage, « Calque » d'une piste.
    pub(crate) fn montage_layer_menu(&mut self, ui: &mut egui::Ui, li: usize) {
        self.layer_menu(ui, li, Origine::Montage);
    }

    /// Le calque `li` devient celui que le menu vise : les calques choisis le restent s'il en fait partie, sinon il est seul
    /// choisi (le clic droit d'une ligne de la timeline ne choisit pas la ligne).
    fn viser_calque(&mut self, li: usize) {
        if !self.is_chosen_layer(li) {
            self.layer_sel.clear();
            self.select_layer(li);
        }
    }

    /// Une entrée de menu reliée à une commande : icône, nom, raccourci ; grisée avec `raison` en bulle quand elle ne peut
    /// pas servir, sinon `aide` en bulle. Clic droit : lui assigner un raccourci. Rend vrai au clic.
    fn entree(&mut self, ui: &mut egui::Ui, icone: &str, texte: &str, action: &str, raison: Option<String>, aide: &str) -> bool {
        let shortcut = if action.is_empty() { String::new() } else { self.keymap.label(action) };
        let libre = raison.is_none() && (action.is_empty() || !self.action_bloquee(action));
        let label = if icone.is_empty() { texte.to_string() } else { format!("{icone}  {texte}") };
        let mut resp = ui.add_enabled(libre, egui::Button::new(label).shortcut_text(&shortcut));
        if let Some(why) = raison {
            resp = resp.on_disabled_hover_text(why);
        } else if !aide.is_empty() {
            resp = resp.on_hover_text(aide);
        }
        if resp.clicked() {
            ui.close_menu();
            return true;
        }
        if resp.secondary_clicked() && !action.is_empty() {
            ui.close_menu();
            self.start_assign(action);
        }
        false
    }

    /// Une entrée qui lance `action` sur le calque visé (et les calques choisis).
    fn entree_action(&mut self, ui: &mut egui::Ui, li: usize, icone: &str, texte: &str, action: &str) {
        if self.entree(ui, icone, texte, action, None, "") {
            self.viser_calque(li);
            self.run_action(action);
        }
    }

    /// Le menu du calque `li` (voir l'en-tête du fichier pour l'organisation).
    pub(crate) fn layer_menu(&mut self, ui: &mut egui::Ui, li: usize, origine: Origine) {
        let Some(l) = self.layers().get(li) else {
            ui.close_menu();
            return;
        };
        let (folder, video) = (l.folder, l.video.is_some());
        let n = self.chosen_layers().len();
        let plusieurs = |un: &str, plusieurs: &str| if n > 1 { format!("{plusieurs} ({n})") } else { un.to_string() };

        // ---- Les gestes de tous les jours.
        ui.menu_button(format!("{}  Nouveau calque", ph::PLUS), |ui| self.layer_new_menu(ui));
        self.entree_action(ui, li, ph::COPY_SIMPLE, &plusieurs("Dupliquer", "Dupliquer les calques choisis"), "calque.dupliquer");
        self.rename_item(ui, li, origine == Origine::Timeline, origine == Origine::Montage);
        self.entree_action(ui, li, ph::TRASH, &plusieurs("Supprimer", "Supprimer les calques choisis"), "calque.supprimer");
        ui.separator();

        // ---- Fusionner.
        let raison_bas = crate::fusioncalques::blocker_down(&self.doc, self.cur_panel, li);
        let aide_bas = if raison_bas.is_none() {
            format!("Le calque et celui du dessous n'en font plus qu'un, tel qu'on les voit. {}", crate::fusioncalques::resultat_de(&self.doc, self.cur_panel, &[li.wrapping_sub(1), li]))
        } else {
            String::new()
        };
        if self.entree(ui, ph::ARROW_LINE_DOWN, "Fusionner vers le bas", "calque.fusionner_dessous", raison_bas, &aide_bas) {
            self.viser_calque(li);
            self.run_action("calque.fusionner_dessous");
        }
        let choisis = self.chosen_layers();
        let raison_choisis = crate::fusioncalques::blocker_list(&self.doc, self.cur_panel, &choisis);
        let aide_choisis = if raison_choisis.is_none() {
            format!("Les {n} calques choisis n'en font plus qu'un, tels qu'on les voit. {}", crate::fusioncalques::resultat_de(&self.doc, self.cur_panel, &choisis))
        } else {
            String::new()
        };
        let titre = if n > 1 { format!("Fusionner les {n} calques choisis") } else { "Fusionner les calques choisis".to_string() };
        if self.entree(ui, ph::ARROWS_MERGE, &titre, "calque.fusionner_choisis", raison_choisis, &aide_choisis) {
            self.run_action("calque.fusionner_choisis");
        }
        ui.separator();

        // ---- Organiser, apparence, effets, aplat.
        ui.menu_button(format!("{}  Organiser", ph::ARROWS_DOWN_UP), |ui| self.organiser_menu(ui, li));
        ui.menu_button(format!("{}  Apparence", ph::EYE), |ui| self.apparence_menu(ui, li));
        if !folder {
            ui.menu_button(format!("{}  Effets", ph::SPARKLE), |ui| {
                self.fx_menu_items(ui, li);
                ui.separator();
                self.fx_switch_item(ui, li);
                self.boil_menu_items(ui, li);
            });
        }
        // (« Ajouter un aplat » / « Retirer l'aplat » : seulement pour un calque qui en prend un.)
        self.aplat_item(ui, li);
        ui.separator();

        // ---- Le reste.
        ui.menu_button(format!("{}  Plus", ph::DOTS_THREE), |ui| self.plus_calque_menu(ui, li, origine, folder, video));
    }

    /// Organiser : l'ordre des calques, les dossiers, les groupes.
    fn organiser_menu(&mut self, ui: &mut egui::Ui, li: usize) {
        self.entree_action(ui, li, ph::ARROW_UP, "Monter le calque", "calque.monter");
        self.entree_action(ui, li, ph::ARROW_DOWN, "Descendre le calque", "calque.descendre");
        ui.menu_button("Aller au calque", |ui| {
            self.entree_action(ui, li, "", "Du dessus", "calque.dessus");
            self.entree_action(ui, li, "", "Du dessous", "calque.dessous");
            self.entree_action(ui, li, "", "Le premier (le plus bas)", "calque.premier");
            self.entree_action(ui, li, "", "Le dernier (le plus haut)", "calque.dernier");
        });
        ui.separator();
        ui.menu_button("Dossiers", |ui| {
            self.entree_action(ui, li, "", "Nouveau dossier", "dossier.nouveau");
            self.entree_action(ui, li, "", "Placer les calques choisis dans un dossier", "dossier.ranger");
            self.entree_action(ui, li, "", "Déposer les calques choisis dans le dossier", "dossier.deposer");
            self.entree_action(ui, li, "", "Sortir les calques choisis de leur dossier", "dossier.sortir");
            ui.separator();
            self.entree_action(ui, li, "", "Joindre les dossiers choisis dans le dossier courant", "dossier.joindre");
            self.entree_action(ui, li, "", "Supprimer uniquement le dossier", "dossier.supprimer_seul");
            self.entree_action(ui, li, "", "Supprimer le dossier et ses calques", "calque.supprimer");
        });
        ui.menu_button("Groupes", |ui| self.groups_menu(ui));
    }

    /// Apparence : afficher, verrouiller, opacité, mode de fusion, couleur d'étiquette, pelure d'oignon.
    fn apparence_menu(&mut self, ui: &mut egui::Ui, li: usize) {
        let Some(l) = self.layers().get(li) else { return };
        let (visible, locked, onion, opacity, blend, folder) = (l.visible, l.locked, l.onion, l.opacity_at(self.cur_frame), l.blend, l.folder);
        if ui.checkbox(&mut visible.clone(), "Afficher le calque").clicked() {
            self.toggle_layer_visible(li);
        }
        let verrou = if locked { "Déverrouiller le calque" } else { "Verrouiller le calque" };
        let icone = if locked { ph::LOCK_SIMPLE_OPEN } else { ph::LOCK_SIMPLE };
        if self.entree(ui, icone, verrou, "calque.verrouiller", None, "Un calque verrouillé ne peut plus être dessiné.") {
            self.toggle_layer_lock(li);
        }
        let mut pct = opacity * 100.0;
        let resp = ui.horizontal(|ui| {
            ui.label("Opacité");
            ui.add(egui::Slider::new(&mut pct, 0.0..=100.0).suffix(" %").fixed_decimals(0))
        });
        if self.edited(&resp.inner) {
            let f = self.cur_frame;
            self.layers_mut()[li].set_opacity_at(f, (pct / 100.0).clamp(0.0, 1.0));
            self.renderer.invalidate_params();
        }
        if !folder {
            ui.menu_button(format!("Mode de fusion : {}", blend.label()), |ui| {
                for m in BlendMode::ALL {
                    if BlendMode::FAMILY_STARTS.contains(&m) {
                        ui.separator();
                    }
                    if ui.radio(m == blend, m.label()).on_hover_text(m.hint()).clicked() {
                        ui.close_menu();
                        self.set_layer_flag(li, |l| l.blend = m);
                    }
                }
            });
        }
        ui.menu_button("Couleur d'étiquette", |ui| self.group_menu(ui, li));
        if !folder && ui.checkbox(&mut onion.clone(), "Pelure d'oignon du calque").clicked() {
            self.set_layer_flag(li, |l| l.onion = !onion);
        }
    }

    /// Plus : le rare, et ce qui est propre à une sorte de calque.
    fn plus_calque_menu(&mut self, ui: &mut egui::Ui, li: usize, origine: Origine, folder: bool, video: bool) {
        ui.menu_button("Copier, coller", |ui| {
            self.entree_action(ui, li, "", "Couper le calque", "calque.couper");
            self.entree_action(ui, li, "", "Copier le calque", "calque.copier");
            self.entree_action(ui, li, "", "Coller le calque", "calque.coller");
            self.entree_action(ui, li, "", "Coller à la place du calque courant", "calque.coller_remplacer");
        });
        ui.menu_button("Retourner le calque", |ui| {
            self.entree_action(ui, li, "", "De gauche à droite", "calque.miroir_h");
            self.entree_action(ui, li, "", "De haut en bas", "calque.miroir_v");
        });
        if !folder {
            ui.menu_button("Réglages avancés", |ui| self.reglages_avances_menu(ui, li));
            ui.menu_button("Animation (avant, après, automatismes)", |ui| self.behaviour_items(ui, li));
        }
        if video {
            ui.menu_button("Calque vidéo", |ui| self.video_menu(ui, li));
        }
        // Calque de couleur sous le trait : en ajouter un, ou ses réglages (griffonner, sources).
        if !folder && !video {
            ui.menu_button("Calque de couleur", |ui| self.couleur_menu(ui, li));
        }
        // La propagation (façon EbSynth) et le coloriage par zones.
        self.propagation_item(ui, li);
        self.coloriage_item(ui, li);
        ui.separator();
        ui.menu_button("Choisir", |ui| {
            self.entree_action(ui, li, "", "Tous les calques", "calque.choisir_tout");
            self.entree_action(ui, li, "", "Les calques visibles", "calque.choisir_visibles");
            self.entree_action(ui, li, "", "Les calques du même groupe", "calque.choisir_groupe");
            self.entree_action(ui, li, "", "Les calques des autres groupes", "calque.choisir_autres_groupes");
            self.entree_action(ui, li, "", "Aucun (le calque courant seul)", "calque.choisir_aucun");
        });
        ui.menu_button("Afficher", |ui| {
            self.entree_action(ui, li, "", "Tous les calques", "calque.tous_visibles");
            self.entree_action(ui, li, "", "Aucun calque", "calque.tous_caches");
            self.entree_action(ui, li, "", "Le calque courant seul", "calque.courant_seul");
        });
        ui.menu_button("Fusionner plus de calques", |ui| {
            let (visibles, tous): (Vec<usize>, Vec<usize>) = ((0..self.layers().len()).filter(|&i| self.layers()[i].visible).collect(), (0..self.layers().len()).collect());
            for (texte, action, lis) in [("Les calques visibles", "calque.fusionner_visibles", visibles), ("Tous les calques", "calque.fusionner_tout", tous)] {
                let raison = crate::fusioncalques::blocker_list(&self.doc, self.cur_panel, &lis);
                let aide = if raison.is_none() { crate::fusioncalques::resultat_de(&self.doc, self.cur_panel, &lis) } else { "" };
                if self.entree(ui, "", texte, action, raison, aide) {
                    self.run_action(action);
                }
            }
        });
        ui.menu_button("Pochoirs", |ui| {
            self.entree_action(ui, li, "", "Éteindre tous les pochoirs", "calque.pochoirs_eteindre");
            self.entree_action(ui, li, "", "Inverser tous les pochoirs", "calque.pochoirs_inverser");
        });
        ui.menu_button("Nettoyer", |ui| {
            self.entree_action(ui, li, "", "Supprimer les calques vides", "calque.supprimer_vides");
            self.entree_action(ui, li, "", "Supprimer les calques cachés", "calque.supprimer_caches");
        });
        // Aplat sous le trait, sur plusieurs calques à la fois (le calque courant a « Ajouter un aplat » au premier niveau).
        ui.menu_button("Aplat sur plusieurs calques", |ui| self.fond_menu(ui, false));
        // Ce qui dépend de l'endroit : les pistes du montage, la taille de la ligne de la timeline.
        match origine {
            Origine::Montage => self.pistes_menu(ui, li),
            Origine::Timeline => {
                if let Some(collapsed) = self.layers().get(li).map(|l| l.collapsed) {
                    if ui.button(if collapsed { "Agrandir la ligne" } else { "Réduire la ligne" }).clicked() {
                        ui.close_menu();
                        self.layers_mut()[li].collapsed = !collapsed;
                        self.modified = true;
                    }
                }
            }
            _ => {}
        }
    }

    /// Plus > Réglages avancés : préserver la transparence, pochoir, position verrouillée, profondeur.
    fn reglages_avances_menu(&mut self, ui: &mut egui::Ui, li: usize) {
        let Some(l) = self.layers().get(li) else { return };
        let (keep_alpha, stencil, negative, fixed) = (l.keep_alpha, l.stencil, l.stencil_negative, l.position_locked);
        if ui.checkbox(&mut keep_alpha.clone(), "Préserver la transparence").on_hover_text("Le pinceau ne colore que ce qui est déjà dessiné : le vide reste vide.").clicked() {
            self.set_layer_flag(li, |l| l.keep_alpha = !keep_alpha);
        }
        if ui.checkbox(&mut stencil.clone(), "Pochoir").on_hover_text("Le dessin de ce calque limite où l'on peint sur les autres calques.").clicked() {
            self.set_layer_flag(li, |l| l.stencil = !stencil);
        }
        if ui
            .add_enabled(stencil, egui::Checkbox::new(&mut negative.clone(), "Pochoir négatif"))
            .on_hover_text("Positif : on ne peint que dans la forme du pochoir. Négatif : seulement en dehors.")
            .clicked()
        {
            self.set_layer_flag(li, |l| l.stencil_negative = !negative);
        }
        if ui
            .checkbox(&mut fixed.clone(), "Verrouiller la position")
            .on_hover_text("L'outil Transformer et le déplacement en 3D ne bougent plus le calque, et on ne peut plus dessiner dessus.")
            .clicked()
        {
            self.set_layer_flag(li, |l| l.position_locked = !fixed);
        }
        self.depth_item(ui, li);
    }
}
