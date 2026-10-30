//! L'écran de la planche de références (`planchref.rs`), en plus du tableau de `referencesui.rs` : les icônes pour poser
//! un post-it, un texte, un titre ou un cadre, ranger, recadrer ; la barre de réglages de ce qui est choisi ; les menus
//! du clic droit ; « Importer la planche d'un autre projet… » ; et la fenêtre à part (détacher, plein écran sur un
//! écran, fenêtre transparente, toujours au-dessus).
//!
//! - **Une étape d'annulation par geste** : chaque commande passe par `refs_step` / `refs_apply` (un nom en français).
//! - **Fenêtre à part** : les réglages (`FenetreReglage`) sont dans `Settings::references_fenetre` et se retiennent d'une
//!   séance à l'autre. `dockui::show_detached` en fait la fenêtre (position, plein écran, transparence, niveau) ; les
//!   commandes d'ici ne font que changer les réglages et prévenir la fenêtre. Un plein écran sur « l'écran 2 » déplace
//!   d'abord la fenêtre sur cet écran (la liste des écrans vient de Windows, `planchref::ecrans`), puis demande le plein
//!   écran quelques images plus tard : winit choisit l'écran où la fenêtre se trouve alors.

use super::*;

use crate::dock::Pane;
use crate::planchref::{self, FenetreReglage, NOTE_COLORS, NOTE_COLOR_NAMES};

/// Une planche d'un autre projet que le fil copie dans celui-ci : les éléments à ajouter ensuite.
pub(crate) struct BoardImport {
    handle: JoinHandle<Result<usize, String>>,
    items: Vec<Reference>,
    epoch: u64,
    name: String,
    warnings: Vec<String>,
}

/// La fenêtre à part de la planche : son identité pour egui (elle change quand la transparence change, pour que
/// Windows refasse la fenêtre).
pub(crate) fn refs_viewport(transparent: bool) -> egui::ViewportId {
    egui::ViewportId::from_hash_of(("panneau détaché", Pane::References, transparent))
}

impl App {
    fn refs_author(&self) -> String {
        crate::commentaires::author_name(&self.settings.comment_author)
    }

    fn refs_window(&self) -> FenetreReglage {
        self.settings.references_fenetre.clone().nettoye()
    }

    fn refs_set_window(&mut self, f: impl FnOnce(&mut FenetreReglage)) {
        let before = self.settings.references_fenetre.clone();
        f(&mut self.settings.references_fenetre);
        self.settings.references_fenetre = self.settings.references_fenetre.clone().nettoye();
        if self.settings.references_fenetre != before {
            let _ = self.settings.save();
        }
    }

    fn refs_detached_now(&self) -> bool {
        self.detached.iter().any(|d| d.panneau == Pane::References)
    }

    /// Les écrans de l'ordinateur (relus au plus toutes les trois secondes, ou tout de suite si `fresh`).
    pub(crate) fn refs_screens(&mut self, fresh: bool) -> Vec<planchref::Ecran> {
        let now = self.ctx.input(|i| i.time);
        if fresh || self.refs.screens.1.is_empty() || now - self.refs.screens.0 > 3.0 {
            self.refs.screens = (now, planchref::ecrans());
        }
        self.refs.screens.1.clone()
    }

    // ------------------------------------------------------------------ les commandes

    /// Les commandes de la planche (celles de `referencesui.rs` sont traitées avant) ; vrai si `action` en était une.
    pub(crate) fn planchref_action(&mut self, action: &str) -> bool {
        let needs_board = matches!(action, "references.postit" | "references.texte" | "references.titre" | "references.cadre" | "references.ranger" | "references.tout_selectionner" | "references.importer_planche");
        if needs_board {
            self.show_pane(Pane::References);
        }
        match action {
            "references.postit" => self.refs_add_block(Kind::Note, None),
            "references.texte" => self.refs_add_block(Kind::Text, None),
            "references.titre" => self.refs_add_block(Kind::Title, None),
            "references.cadre" => self.refs_add_block(Kind::Frame, None),
            "references.ranger" => self.refs_arrange(),
            "references.recadrer" => self.refs_toggle_crop(),
            "references.retourner_h" => self.refs_flip(true),
            "references.retourner_v" => self.refs_flip(false),
            "references.noir_blanc" => {
                let on = !self.refs_selected().iter().filter_map(|i| self.doc.reference(*i)).all(|r| r.grey);
                self.refs_apply("Références en noir et blanc", |x| {
                    if x.kind == Kind::Image {
                        x.grey = on
                    }
                });
            }
            "references.devant" => self.refs_order(true),
            "references.derriere" => self.refs_order(false),
            "references.tout_selectionner" => {
                self.refs.layout = Layout::Board;
                self.refs.sel = self.refs_filtered().iter().map(|r| r.id).collect();
                self.refs.focus = true;
            }
            "references.importer_planche" => self.refs_import_board_dialog(),
            "references.rectangle" => {
                self.refs.marquee_default = !self.refs.marquee_default;
                self.status = if self.refs.marquee_default {
                    "Planche : tirer sur le fond dessine un rectangle de sélection (espace ou bouton du milieu : déplacer la vue)."
                } else {
                    "Planche : tirer sur le fond déplace la vue (Maj ou Ctrl + tirer : rectangle de sélection)."
                }
                .into();
            }
            "references.detacher" => {
                if self.refs_detached_now() {
                    self.status = "La planche est déjà dans sa fenêtre : la fermer la remet à sa place.".into();
                } else {
                    self.detach_request = Some(Pane::References);
                }
            }
            "references.plein_ecran" => {
                if self.refs_window().fullscreen {
                    self.refs_leave_fullscreen();
                } else {
                    let screen = self.refs_window().screen;
                    self.refs_go_fullscreen(screen);
                }
            }
            "references.ecran_2" => self.refs_go_fullscreen(2),
            "references.transparente" => {
                let on = !self.refs_window().transparent;
                self.refs_set_window(|w| w.transparent = on);
                if on && !self.refs_detached_now() {
                    self.detach_request = Some(Pane::References);
                }
                self.status = if on { "Fenêtre de la planche transparente (la fenêtre est refaite pour que Windows la rende transparente)." } else { "Fenêtre de la planche opaque." }.into();
            }
            "references.toujours_dessus" => {
                let on = !self.refs_window().on_top;
                self.refs_set_window(|w| w.on_top = on);
                let level = if on { egui::WindowLevel::AlwaysOnTop } else { egui::WindowLevel::Normal };
                let id = refs_viewport(self.refs_window().transparent);
                self.ctx.send_viewport_cmd_to(id, egui::ViewportCommand::WindowLevel(level));
            }
            _ => return false,
        }
        true
    }

    /// Chaque image d'écran : la planche d'un autre projet qui finit de se copier, et la fenêtre à part reprise des réglages.
    pub(crate) fn planchref_tick(&mut self, ctx: &egui::Context) {
        // La fenêtre à part retenue dans les réglages revient d'une séance à l'autre.
        if !self.refs.window_restored {
            self.refs.window_restored = true;
            self.refs.sync_wait = 3;
            if self.refs_window().detached && !self.refs_detached_now() {
                self.detach_request = Some(Pane::References);
            }
        } else if self.refs.sync_wait > 0 {
            self.refs.sync_wait -= 1;
            ctx.request_repaint();
        } else if self.detach_request.is_none() {
            let now = self.refs_detached_now();
            if self.settings.references_fenetre.detached != now {
                self.refs_set_window(|w| w.detached = now);
            }
        }
        // Les images d'écran qu'on attend avant le plein écran.
        if self.refs.fs_delay > 0 {
            self.refs.fs_delay -= 1;
            ctx.request_repaint();
        }
        let Some(b) = self.refs.board_import.as_ref() else { return };
        if !b.handle.is_finished() {
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        let Some(b) = self.refs.board_import.take() else { return };
        let result = b.handle.join().unwrap_or_else(|_| Err("la copie des images a échoué".into()));
        if b.epoch != self.doc_epoch {
            return;
        }
        match result {
            Err(e) => self.status = format!("Import de la planche impossible : {e}"),
            Ok(_) => {
                let n = b.items.len();
                self.refs_step("Importer la planche d'un autre projet");
                self.refs.sel = b.items.iter().map(|r| r.id).collect();
                for r in b.items {
                    self.doc.add_reference(r);
                }
                self.refs.view = None;
                self.refs.layout = Layout::Board;
                self.status = format!("{n} élément(s) importé(s) de « {} », à droite de ceux qui y étaient{}.", b.name, if b.warnings.is_empty() { String::new() } else { format!(" ({} avertissement(s) : {})", b.warnings.len(), b.warnings[0]) });
            }
        }
    }

    /// Un post-it, un texte, un titre ou un cadre neuf : au centre de ce que le tableau montre (ou en `at`). Un cadre
    /// demandé quand des éléments sont choisis se pose autour d'eux.
    pub(crate) fn refs_add_block(&mut self, kind: Kind, at: Option<Pos2>) {
        self.refs.layout = Layout::Board;
        let id = self.doc.new_reference_id();
        let author = self.refs_author();
        if kind == Kind::Frame && at.is_none() {
            let ids = self.refs_selected();
            if !ids.is_empty() {
                self.refs_step("Mettre la sélection dans un cadre");
                if self.doc.frame_around(&ids, id, &author).is_some() {
                    if let Some(f) = self.doc.reference_mut(id) {
                        f.color = 3;
                    }
                    self.refs.sel = vec![id];
                    self.refs.text_focus = Some(id);
                    self.status = "Cadre posé autour de la sélection : donne-lui un nom ; déplacer le cadre emporte ce qu'il contient.".into();
                }
                return;
            }
        }
        let center = self.refs.view.map(|v| v.center).or_else(|| references::bounds_of(&self.doc.references).map(|b| b.center())).unwrap_or(Pos2::ZERO);
        let spot = references::free_spot(&self.doc.references, at.unwrap_or(center));
        let mut r = Reference::block(kind, id, spot, "", if kind == Kind::Frame { 3 } else { 0 });
        r.stamp(&author);
        r.plans.extend(self.refs_link_target());
        self.refs_step(match kind {
            Kind::Note => "Ajouter un post-it",
            Kind::Text => "Ajouter un texte",
            Kind::Title => "Ajouter un titre",
            Kind::Frame => "Ajouter un cadre",
            Kind::Image => "Ajouter une référence",
        });
        self.doc.add_reference(r);
        self.refs.sel = vec![id];
        self.refs.text_focus = Some(id);
        self.status = match kind {
            Kind::Note => "Post-it posé : écris dedans (en bas), change sa couleur, déplace-le.",
            Kind::Text => "Texte posé : écris-le (en bas).",
            Kind::Title => "Titre posé : écris-le (en bas).",
            _ => "Cadre posé : donne-lui un nom, tire ses coins ; il emporte ce qu'il contient quand on le déplace.",
        }
        .into();
    }

    /// Range la sélection (au moins deux éléments, ou un cadre) ou, sans sélection, tout ce que le filtre laisse voir.
    pub(crate) fn refs_arrange(&mut self) {
        let mut ids = self.refs_selected();
        let all = ids.is_empty();
        if all {
            ids = self.refs_filtered().iter().filter(|r| r.kind != Kind::Frame).map(|r| r.id).collect();
        }
        if ids.len() < 2 && !ids.iter().any(|i| self.doc.reference(*i).is_some_and(|r| r.kind == Kind::Frame)) {
            self.status = "Rien à ranger : choisis au moins deux éléments, ou un cadre.".into();
            return;
        }
        self.refs_step(if all { "Ranger toute la planche" } else { "Ranger les éléments" });
        if self.doc.arrange(&ids) {
            let now = planchref::now_secs();
            for id in &ids {
                if let Some(r) = self.doc.reference_mut(*id) {
                    r.touch(now);
                }
            }
            if all {
                self.refs.view = None;
            }
            self.status = format!("{} élément(s) rangé(s), serrés, avec leurs tailles d'origine (Ctrl+Z défait).", ids.len());
        } else {
            self.status = "Rien à ranger : un cadre vide, ou moins de deux éléments.".into();
        }
    }

    /// Entre dans le recadrage de l'image choisie (ou en sort).
    pub(crate) fn refs_toggle_crop(&mut self) {
        let ids = self.refs_selected();
        match ids.as_slice() {
            [id] if self.doc.reference(*id).is_some_and(|r| r.kind == Kind::Image) => {
                self.refs.layout = Layout::Board;
                self.refs.crop_mode = if self.refs.crop_mode == Some(*id) { None } else { Some(*id) };
                if self.refs.crop_mode.is_some() {
                    self.status = "Recadrer : tire les poignées pour ne garder qu'une partie de l'image ; l'image entière reste dans le projet (Échap ou Entrée pour finir).".into();
                }
            }
            _ => self.status = "Recadrer : choisis une seule image.".into(),
        }
    }

    fn refs_flip(&mut self, horizontal: bool) {
        let on = !self.refs_selected().iter().filter_map(|i| self.doc.reference(*i)).filter(|r| r.kind == Kind::Image).all(|r| if horizontal { r.flip_h } else { r.flip_v });
        self.refs_apply("Retourner l'image", |x| {
            if x.kind != Kind::Image {
                return;
            }
            if horizontal {
                x.flip_h = on
            } else {
                x.flip_v = on
            }
        });
    }

    /// Met la sélection devant (ou derrière) les autres, dans l'ordre où elle était.
    fn refs_order(&mut self, front: bool) {
        let ids = self.refs_selected();
        if ids.is_empty() {
            self.status = "Aucun élément choisi.".into();
            return;
        }
        self.refs_step(if front { "Mettre devant" } else { "Mettre derrière" });
        let order: Vec<u64> = self.doc.references.iter().map(|r| r.id).filter(|i| ids.contains(i)).collect();
        if front {
            for id in order {
                self.doc.raise_reference(id);
            }
        } else {
            for id in order.into_iter().rev() {
                self.doc.lower_reference(id);
            }
        }
    }

    fn refs_import_board_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new().set_title("Importer la planche d'un autre projet").add_filter("Projet SHOT", &crate::project::EXTENSIONS).pick_file() else { return };
        self.refs_import_board(path);
    }

    /// Copie la planche du projet `manifest` à côté de celle-ci : les éléments (numéros et identifiants neufs) à droite de ce
    /// qui y est déjà, les images dans `references/` de ce projet (sur un autre fil).
    pub(crate) fn refs_import_board(&mut self, manifest: PathBuf) {
        if self.refs.board_import.is_some() {
            self.status = "Une planche est déjà en cours d'import.".into();
            return;
        }
        let dest = self.refs_dest_dir();
        let after = references::bounds_of(&self.doc.references);
        let imported = {
            let doc = &mut self.doc;
            planchref::read_other_board(&manifest, &dest, after, || doc.new_reference_id())
        };
        match imported {
            Err(e) => self.status = format!("Import de la planche impossible : {e}"),
            Ok(i) => {
                let ctx = self.ctx.clone();
                let copies = i.copies;
                let handle = std::thread::spawn(move || {
                    let mut n = 0;
                    for (from, to) in copies {
                        n += references::copy_entry(&from, &to)?;
                    }
                    ctx.request_repaint();
                    Ok(n)
                });
                let name = manifest.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                self.refs.board_import = Some(BoardImport { handle, items: i.items, epoch: self.doc_epoch, name, warnings: i.warnings });
                self.status = "Import de la planche…".into();
            }
        }
    }

    // ------------------------------------------------------------------ la fenêtre à part

    /// Plein écran de la fenêtre à part sur l'écran `screen` (0 : celui où elle est ; 1, 2… : cet écran). Détache d'abord
    /// la planche si elle ne l'est pas.
    pub(crate) fn refs_go_fullscreen(&mut self, screen: u8) {
        if !self.refs_detached_now() {
            self.detach_request = Some(Pane::References);
        }
        let mut moved = false;
        if screen > 0 {
            let list = self.refs_screens(true);
            match planchref::ecran_numero(&list, screen) {
                Some(e) => {
                    let ppp = self.ctx.pixels_per_point().max(0.25);
                    let to = pos2((e.x + 40) as f32 / ppp, (e.y + 40) as f32 / ppp);
                    let id = refs_viewport(self.refs_window().transparent);
                    self.ctx.send_viewport_cmd_to(id, egui::ViewportCommand::OuterPosition(to));
                    moved = true;
                }
                None => {
                    self.status = if list.is_empty() { "Écrans : Windows n'en dit rien, plein écran sur l'écran où est la fenêtre.".into() } else { format!("Il n'y a pas d'écran {screen} : plein écran sur l'écran où est la fenêtre.") };
                }
            }
        }
        self.refs_set_window(|w| {
            w.fullscreen = true;
            w.screen = screen;
        });
        // La fenêtre doit d'abord arriver sur l'écran : le plein écran part quelques images plus tard.
        self.refs.fs_delay = if moved { 4 } else { 0 };
        self.ctx.request_repaint();
    }

    pub(crate) fn refs_leave_fullscreen(&mut self) {
        self.refs_set_window(|w| w.fullscreen = false);
        let id = refs_viewport(self.refs_window().transparent);
        self.ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Fullscreen(false));
    }

    /// F11 (plein écran) et Échap (en sortir) quand la fenêtre à part a la main.
    pub(crate) fn refs_window_keys(&mut self, ctx: &egui::Context, focused: bool) {
        if !focused {
            return;
        }
        let (f11, esc) = ctx.input(|i| (i.key_pressed(Key::F11), i.key_pressed(Key::Escape)));
        if f11 {
            self.run_action("references.plein_ecran");
        } else if esc && self.refs_window().fullscreen && self.refs.drag.is_none() && self.refs.crop_mode.is_none() && !ctx.wants_keyboard_input() {
            self.refs_leave_fullscreen();
        }
    }

    // ------------------------------------------------------------------ les icônes

    /// Poser un post-it, un texte, un titre, un cadre.
    pub(crate) fn refs_add_icons(&mut self, ui: &mut egui::Ui) {
        if self.bouton(ui, ph::NOTE, "Post-it", "Pose un post-it de couleur au centre de la planche, pour une idée, un rappel, une consigne. Le texte s'écrit dans la barre du bas.", "references.postit").clicked() {
            self.run_action("references.postit");
        }
        if self.bouton(ui, ph::TEXT_T, "Texte", "Pose un texte libre, sans fond, sur la planche.", "references.texte").clicked() {
            self.run_action("references.texte");
        }
        if self.bouton(ui, ph::TEXT_AA, "Titre", "Pose un grand titre pour nommer une zone de la planche.", "references.titre").clicked() {
            self.run_action("references.titre");
        }
        if self.bouton(ui, ph::BOUNDING_BOX, "Cadre", "Un cadre nommé pour regrouper (« Décor forêt », « Perso Léa »). Si des éléments sont choisis, il se pose autour d'eux. Déplacer un cadre emporte ce qu'il contient.", "references.cadre").clicked() {
            self.run_action("references.cadre");
        }
    }

    /// Rectangle de sélection, ranger, et le menu des commandes plus rares.
    pub(crate) fn refs_board_icons(&mut self, ui: &mut egui::Ui) {
        let on = self.refs.marquee_default;
        if self.bouton_etat(ui, on, ph::SELECTION, "Rectangle de sélection", "Tirer sur le fond dessine un rectangle qui choisit ce qu'il touche (sinon il déplace la vue ; Maj ou Ctrl + tirer le font aussi, espace ou bouton du milieu déplacent toujours la vue).", "references.rectangle").clicked() {
            self.run_action("references.rectangle");
        }
        if self.bouton(ui, ph::STACK, "Ranger", "Empile serré la sélection (ou toute la planche s'il n'y a pas de sélection), en gardant les tailles. Sur un cadre : range ce qu'il contient et le referme dessus.", "references.ranger").clicked() {
            self.run_action("references.ranger");
        }
        ui.menu_button(RichText::new(ph::DOTS_THREE).size(crate::bulles::TAILLE), |ui| {
            if ui.button("Tout sélectionner").clicked() {
                ui.close_menu();
                self.run_action("references.tout_selectionner");
            }
            if ui.button("Importer la planche d'un autre projet…").on_hover_text("Copie ses éléments et ses images à droite de ceux de cette planche").clicked() {
                ui.close_menu();
                self.run_action("references.importer_planche");
            }
        })
        .response
        .on_hover_text("Plus de commandes");
    }

    /// Détacher la planche ; dans sa fenêtre : plein écran, écran, toujours au-dessus, transparente.
    pub(crate) fn refs_window_icons(&mut self, ui: &mut egui::Ui, detached: bool) {
        use crate::bulles;
        ui.separator();
        if !detached {
            if self.bouton(ui, ph::APP_WINDOW, "Détacher", "Met la planche dans sa propre fenêtre, à poser sur un autre écran ; elle peut y passer en plein écran, rester toujours au-dessus ou devenir transparente. Fermer la fenêtre remet la planche à sa place.", "references.detacher").clicked() {
                self.run_action("references.detacher");
            }
            return;
        }
        let rf = self.refs_window();
        let (icon, name) = if rf.fullscreen { (ph::ARROWS_IN_SIMPLE, "Quitter le plein écran") } else { (ph::ARROWS_OUT_SIMPLE, "Plein écran") };
        if self.bouton_etat(ui, rf.fullscreen, icon, name, "La fenêtre prend tout l'écran où elle se trouve (ou celui choisi à côté). F11 aussi, Échap pour en sortir.", "references.plein_ecran").clicked() {
            self.run_action("references.plein_ecran");
        }
        let screens = self.refs_screens(false);
        if screens.len() > 1 {
            // (« Cet écran » dit lequel : celui où la fenêtre est.)
            let ppp = ui.ctx().pixels_per_point();
            let here = ui.ctx().input(|i| i.viewport().outer_rect).and_then(|r| planchref::ecran_de(&screens, (r.center().x * ppp) as i32, (r.center().y * ppp) as i32));
            let this = here.map_or("Cet écran".to_string(), |n| format!("Cet écran ({})", n + 1));
            let shown = if rf.screen == 0 { this.clone() } else { format!("Écran {}", rf.screen) };
            let mut pick: Option<u8> = None;
            let resp = egui::ComboBox::from_id_salt("refs_ecran").selected_text(shown).width(110.0).show_ui(ui, |ui| {
                if ui.selectable_label(rf.screen == 0, this).clicked() {
                    pick = Some(0);
                }
                for n in 1..=screens.len() as u8 {
                    if ui.selectable_label(rf.screen == n, format!("Écran {n}")).clicked() {
                        pick = Some(n);
                    }
                }
            });
            bulles::bulle(resp.response, "Écran du plein écran", "Envoie la fenêtre en plein écran sur cet écran (Écran 1 : l'écran principal de Windows).", "");
            if let Some(n) = pick {
                self.refs_go_fullscreen(n);
            }
        }
        if self.bouton_etat(ui, rf.on_top, ph::PICTURE_IN_PICTURE, "Toujours au-dessus", "Garde cette fenêtre devant les autres, pour la lire en dessinant.", "references.toujours_dessus").clicked() {
            self.run_action("references.toujours_dessus");
        }
        if self.bouton_etat(ui, rf.transparent, ph::GHOST, "Fenêtre transparente", "Le fond de la fenêtre devient transparent : on voit ce qui est derrière, la planche reste lisible par-dessus. L'opacité se règle à côté. (Fait refaire la fenêtre.)", "references.transparente").clicked() {
            self.run_action("references.transparente");
        }
        if rf.transparent {
            let mut pct = rf.opacity as i32;
            let resp = ui.add(egui::DragValue::new(&mut pct).range(20..=100).suffix(" %").speed(0.5));
            bulles::bulle(resp.clone(), "Opacité de la fenêtre", "De 20 à 100 % : le fond et tout ce que la planche montre deviennent plus ou moins transparents.", "");
            if resp.changed() {
                self.refs_set_window(|w| w.opacity = pct.clamp(20, 100) as u8);
            }
        }
    }

    // ------------------------------------------------------------------ la barre de ce qui est choisi

    /// Les réglages de l'élément choisi en dernier : vue, miroirs, noir et blanc, recadrage, ordre, plan ; puis texte ou nom,
    /// échelle, rotation, opacité, couleur, étiquettes, et la lecture d'une vidéo.
    pub(crate) fn refs_selection_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        use crate::bulles;
        let ids = self.refs_selected();
        let Some(&id) = ids.last() else { return };
        let Some(r) = self.doc.reference(id).cloned() else { return };
        let ppp = ctx.pixels_per_point();
        let plan = self.refs_plan_id();
        let linked = plan.is_some_and(|p| ids.iter().all(|i| self.doc.reference(*i).is_some_and(|x| x.plans.contains(&p))));
        let is_image = r.kind == Kind::Image;
        let arrangeable = ids.len() > 1 || r.kind == Kind::Frame;
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if bulles::icone(ui, ph::ARROWS_IN, "Ajuster", "Cadre le tableau sur cet élément.", "").clicked() {
                self.refs_zoom_to(id, false, ppp);
            }
            if is_image {
                if bulles::icone(ui, ph::MAGNIFYING_GLASS, "100 %", "Un pixel de l'image fait un pixel de l'écran.", "").clicked() {
                    self.refs_zoom_to(id, true, ppp);
                }
                if self.bouton_etat(ui, r.flip_h, ph::FLIP_HORIZONTAL, "Retourner de gauche à droite", "Un miroir, pour juger un dessin d'un œil neuf.", "references.retourner_h").clicked() {
                    self.run_action("references.retourner_h");
                }
                if self.bouton_etat(ui, r.flip_v, ph::FLIP_VERTICAL, "Retourner de haut en bas", "Un miroir vertical.", "references.retourner_v").clicked() {
                    self.run_action("references.retourner_v");
                }
                if self.bouton_etat(ui, r.grey, ph::CIRCLE_HALF, "Noir et blanc", "Vérifier les valeurs : la luminosité de chaque point (Rec. 709), sans la couleur.", "references.noir_blanc").clicked() {
                    self.run_action("references.noir_blanc");
                }
                let cropping = self.refs.crop_mode == Some(id);
                if ids.len() == 1 && self.bouton_etat(ui, cropping, ph::CROP, "Recadrer", "Ne garde qu'une partie de l'image : l'image entière reste dans le projet, on peut rétablir. Tire les poignées ; Échap ou Entrée pour finir.", "references.recadrer").clicked() {
                    self.run_action("references.recadrer");
                }
                if r.crop.is_some() && bulles::icone(ui, ph::ARROW_COUNTER_CLOCKWISE, "Rétablir l'image entière", "Annule le recadrage (l'image garde sa place).", "").clicked() {
                    self.refs_apply("Rétablir l'image entière", |x| x.set_crop(None));
                    self.refs.crop_mode = None;
                }
            }
            if arrangeable && self.bouton(ui, ph::STACK, "Ranger", "Empile serré les éléments choisis, en gardant leurs tailles. Sur un cadre : range ce qu'il contient et le referme dessus.", "references.ranger").clicked() {
                self.run_action("references.ranger");
            }
            if self.bouton(ui, ph::ARROW_FAT_LINES_UP, "Mettre devant", "Passe l'élément devant les autres (un cadre reste derrière ce qu'il contient).", "references.devant").clicked() {
                self.run_action("references.devant");
            }
            if self.bouton(ui, ph::ARROW_FAT_LINES_DOWN, "Mettre derrière", "Passe l'élément derrière les autres.", "references.derriere").clicked() {
                self.run_action("references.derriere");
            }
            if self.bouton_etat(ui, linked, ph::PUSH_PIN, "Lier au plan courant", "L'élément suit ce plan : le filtre « références du plan courant » ne montre qu'eux. Un autre clic le délie.", "references.lier_plan").clicked() {
                self.run_action("references.lier_plan");
            }
            if self.bouton(ui, ph::TRASH, "Supprimer", "Retire l'élément de la planche (Ctrl+Z le rétablit).", "references.supprimer").clicked() {
                self.run_action("references.supprimer");
            }
        });
        // Texte ou nom.
        if r.kind.has_text() {
            let mut text = r.text.clone();
            let eid = egui::Id::new(("texte_reference", id));
            let hint = match r.kind {
                Kind::Frame => "Nom du cadre (Décor forêt, Perso Léa…)",
                Kind::Title => "Le titre",
                Kind::Text => "Le texte",
                _ => "Écris dans le post-it",
            };
            let rows = if r.kind == Kind::Note { 3 } else { 2 };
            let resp = ui.add(egui::TextEdit::multiline(&mut text).id(eid).hint_text(hint).desired_rows(rows).desired_width(ui.available_width()));
            if self.refs.text_focus == Some(id) {
                resp.request_focus();
                self.refs.text_focus = None;
            }
            if self.refs_edited(&resp, "Écrire dans l'élément") {
                if let Some(x) = self.doc.reference_mut(id) {
                    x.text = text.chars().take(planchref::TEXT_MAX).collect();
                    x.touch(planchref::now_secs());
                }
            }
        }
        // Nom (images), échelle, rotation, opacité.
        ui.horizontal_wrapped(|ui| {
            if is_image {
                let mut name = r.name.clone();
                let resp = ui.add(egui::TextEdit::singleline(&mut name).id(egui::Id::new(("nom_reference", id))).hint_text("Nom").desired_width(120.0));
                if self.refs_edited(&resp, "Renommer la référence") {
                    if let Some(x) = self.doc.reference_mut(id) {
                        x.name = name.chars().take(200).collect();
                        x.touch(planchref::now_secs());
                    }
                }
            }
            let mut percent = r.scale * 100.0;
            let resp = ui.add(egui::DragValue::new(&mut percent).speed(1.0).range(references::SCALE_MIN * 100.0..=references::SCALE_MAX * 100.0).suffix(" %").max_decimals(1));
            bulles::bulle(resp.clone(), "Échelle", "La taille sur le tableau, en pour cent (d'une image : de ses pixels ; d'un texte : de sa taille d'origine).", "");
            if self.refs_edited(&resp, "Redimensionner la référence") {
                for i in &ids {
                    if let Some(x) = self.doc.reference_mut(*i) {
                        x.scale = (percent / 100.0).clamp(references::SCALE_MIN, references::SCALE_MAX);
                    }
                }
            }
            if r.kind != Kind::Frame {
                let mut degrees = r.rotation;
                let resp = ui.add(egui::DragValue::new(&mut degrees).speed(0.5).range(-180.0..=180.0).suffix("°").max_decimals(1));
                bulles::bulle(resp.clone(), "Rotation", "En degrés, dans le sens des aiguilles d'une montre.", "");
                if self.refs_edited(&resp, "Tourner la référence") {
                    for i in &ids {
                        if let Some(x) = self.doc.reference_mut(*i) {
                            x.rotation = references::normalize_degrees(degrees);
                        }
                    }
                }
            }
            let mut pct = (r.opacity * 100.0).round();
            let resp = ui.add(egui::DragValue::new(&mut pct).speed(0.7).range(5.0..=100.0).suffix(" %").prefix("opacité ").max_decimals(0));
            bulles::bulle(resp.clone(), "Opacité", "100 % : opaque. Plus bas, l'élément laisse voir ce qui est derrière lui.", "");
            if self.refs_edited(&resp, "Opacité de l'élément") {
                for i in &ids {
                    if let Some(x) = self.doc.reference_mut(*i) {
                        x.opacity = (pct / 100.0).clamp(0.05, 1.0);
                    }
                }
            }
            if ids.len() > 1 {
                ui.label(RichText::new(format!("{} choisis", ids.len())).small().weak());
            }
        });
        // La couleur d'un post-it, d'un texte, d'un titre, d'un cadre.
        if r.kind.has_text() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(ph::PALETTE).size(bulles::TAILLE).color(theme::texte_doux()));
                for (i, c) in NOTE_COLORS.iter().enumerate() {
                    let plain = i == 0 && matches!(r.kind, Kind::Text | Kind::Title);
                    let (rect, resp) = ui.allocate_exact_size(vec2(17.0, 17.0), Sense::click());
                    let fill = if plain { theme::texte() } else { Color32::from_rgb(c[0], c[1], c[2]) };
                    ui.painter().rect_filled(rect, 3.0, fill);
                    if r.color as usize == i {
                        ui.painter().rect_stroke(rect.expand(1.5), 3.0, Stroke::new(2.0_f32, theme::contour()), StrokeKind::Outside);
                    }
                    let name = if plain { "Normal" } else { NOTE_COLOR_NAMES[i] };
                    bulles::bulle(resp.clone(), name, "La couleur du post-it, du texte, du titre ou du cadre.", "");
                    if resp.clicked() {
                        self.refs_apply("Couleur de l'élément", |x| {
                            if x.kind.has_text() {
                                x.color = i as u8
                            }
                        });
                    }
                }
            });
        }
        // Étiquettes : validées en quittant le champ.
        ui.horizontal(|ui| {
            ui.label(RichText::new(ph::TAG).size(bulles::TAILLE).color(theme::texte_doux()));
            if self.refs.tags_buf.0 != id {
                self.refs.tags_buf = (id, r.tags.join(", "));
            }
            let resp = ui.add(egui::TextEdit::singleline(&mut self.refs.tags_buf.1).id(egui::Id::new(("etiquettes_reference", id))).hint_text("Étiquettes : décor, lumière, rue…").desired_width(ui.available_width()));
            if resp.lost_focus() {
                let tags = references::parse_tags(&self.refs.tags_buf.1);
                if tags != r.tags {
                    self.refs_step("Étiquettes de la référence");
                    if let Some(x) = self.doc.reference_mut(id) {
                        x.tags = tags.clone();
                        x.touch(planchref::now_secs());
                    }
                }
                self.refs.tags_buf.1 = tags.join(", ");
            }
        });
        if r.is_video() {
            self.refs_transport(ui, ctx, &r);
        }
        ui.separator();
    }

    // ------------------------------------------------------------------ le menu du clic droit

    /// Le menu d'un clic droit sur le tableau, la grille ou la liste.
    pub(crate) fn refs_menu(&mut self, ui: &mut egui::Ui, ppp: f32) {
        let target = self.refs.menu_target.filter(|id| self.doc.reference(*id).is_some());
        if let Some(id) = target {
            let r = self.doc.reference(id).cloned();
            let Some(r) = r else { return };
            let is_image = r.kind == Kind::Image;
            let ids = self.refs_selected();
            if ui.button("Ajuster à l'écran").clicked() {
                self.refs_zoom_to(id, false, ppp);
                ui.close_menu();
            }
            if is_image && ui.button("Taille réelle (100 %)").clicked() {
                self.refs_zoom_to(id, true, ppp);
                ui.close_menu();
            }
            if r.kind.has_text() && ui.button("Modifier le texte").clicked() {
                self.refs.text_focus = Some(id);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Mettre devant").clicked() {
                self.run_action("references.devant");
                ui.close_menu();
            }
            if ui.button("Mettre derrière").clicked() {
                self.run_action("references.derriere");
                ui.close_menu();
            }
            if ids.len() > 1 || r.kind == Kind::Frame {
                if ui.button("Ranger").clicked() {
                    self.run_action("references.ranger");
                    ui.close_menu();
                }
            }
            if r.kind != Kind::Frame && ui.button("Mettre dans un cadre").clicked() {
                self.run_action("references.cadre");
                ui.close_menu();
            }
            if is_image {
                ui.separator();
                if ui.button("Recadrer").clicked() {
                    self.run_action("references.recadrer");
                    ui.close_menu();
                }
                if r.crop.is_some() && ui.button("Rétablir l'image entière").clicked() {
                    self.refs_apply("Rétablir l'image entière", |x| x.set_crop(None));
                    ui.close_menu();
                }
                if ui.button("Retourner de gauche à droite").clicked() {
                    self.run_action("references.retourner_h");
                    ui.close_menu();
                }
                if ui.button("Retourner de haut en bas").clicked() {
                    self.run_action("references.retourner_v");
                    ui.close_menu();
                }
                if ui.button("Noir et blanc (oui / non)").clicked() {
                    self.run_action("references.noir_blanc");
                    ui.close_menu();
                }
            }
            ui.menu_button("Opacité", |ui| {
                for pct in [100, 75, 50, 25] {
                    if ui.button(format!("{pct} %")).clicked() {
                        self.refs_apply("Opacité de l'élément", |x| x.opacity = pct as f32 / 100.0);
                        ui.close_menu();
                    }
                }
            });
            if r.kind.has_text() {
                ui.menu_button("Couleur", |ui| {
                    for (i, name) in NOTE_COLOR_NAMES.iter().enumerate() {
                        if ui.button(*name).clicked() {
                            self.refs_apply("Couleur de l'élément", |x| {
                                if x.kind.has_text() {
                                    x.color = i as u8
                                }
                            });
                            ui.close_menu();
                        }
                    }
                });
            }
            if r.kind != Kind::Frame && ui.button("Remettre droit").clicked() {
                self.refs_apply("Tourner la référence", |x| x.rotation = 0.0);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Lier au plan courant (ou délier)").clicked() {
                self.refs_toggle_link();
                ui.close_menu();
            }
            if ui.button("Supprimer").clicked() {
                self.refs_delete_selected();
                ui.close_menu();
            }
        } else {
            let at = self.refs.menu_at;
            if ui.button("Post-it ici").clicked() {
                ui.close_menu();
                self.refs_add_block(Kind::Note, at);
            }
            if ui.button("Texte ici").clicked() {
                ui.close_menu();
                self.refs_add_block(Kind::Text, at);
            }
            if ui.button("Titre ici").clicked() {
                ui.close_menu();
                self.refs_add_block(Kind::Title, at);
            }
            if ui.button("Cadre ici").clicked() {
                ui.close_menu();
                self.refs_add_block(Kind::Frame, at.or(Some(Pos2::ZERO)));
            }
            ui.separator();
            if ui.button("Ranger toute la planche").clicked() {
                ui.close_menu();
                self.run_action("references.ranger");
            }
            if ui.button("Tout voir").clicked() {
                self.run_action("references.tout_voir");
                ui.close_menu();
            }
            if ui.button("Tout sélectionner").clicked() {
                ui.close_menu();
                self.run_action("references.tout_selectionner");
            }
            ui.separator();
            if ui.button("Importer des images…").clicked() {
                ui.close_menu();
                self.run_action("references.importer_image");
            }
            if ui.button("Importer une vidéo…").clicked() {
                ui.close_menu();
                self.run_action("references.importer_video");
            }
            if ui.button("Coller une image").clicked() {
                ui.close_menu();
                self.run_action("references.coller");
            }
            if ui.button("Importer la planche d'un autre projet…").clicked() {
                ui.close_menu();
                self.run_action("references.importer_planche");
            }
        }
    }
}
