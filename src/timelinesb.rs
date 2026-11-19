//! Partie 178 : la timeline de l'espace Storyboard dans l'organisation dessinée par Anthony (2026-11-19 : « pour l'instant
//! impossible de poser des presets de caméra sans cette timeline dédiée à la caméra »), puis validée sur une maquette.
//!
//! De haut en bas, sur toute la longueur du film (`montagerows::Rows`, `DispoStoryboard`) :
//! 1. **Séquence** : la ligne la plus fine possible (16 points), les blocs de séquence ;
//! 2. **Plan** (26 points) : les plans bout à bout, leur miniature, leur nom, leur durée, leur version ; sur la coupe entre deux
//!    plans, le symbole ⋈ de la transition s'il y en a une (un clic : son menu, celui de la partie 175) ;
//! 3. **⊕ / ⊖ Caméra** : repliée, la ligne fine (22 points) de ses clés ; dépliée, la rangée des préréglages de caméra en plus
//!    (24 points ; deux rangs bas quand deux préréglages se chevauchent), où l'on pose, déplace, étire et règle les blocs comme
//!    dans la timeline de l'Animation (`blocscamera.rs`). « On déplie uniquement pour poser des préréglages. »
//! 4. **⊕ / ⊖ Calques** : repliée, une ligne de résumé (les clés de chaque plan) ; dépliée, les pistes des calques, comme avant.
//!
//! Les traits de coupe des plans traversent la caméra, ses préréglages et les calques. Chaque rangée a, à gauche, un en-tête avec ses
//! icônes, qui lancent les actions existantes (le fichier de raccourcis ; leur bulle donne le nom, le raccourci et ce qu'elles font).
//! Les états replié / déplié sont des réglages d'affichage du poste (`Settings::storyboard_camera_depliee`,
//! `storyboard_calques_replies`), pas du projet. L'espace Montage garde sa disposition.

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Painter, Rect, RichText, Sense, Shape, Stroke, StrokeKind};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::montagerows::Rows;
use crate::theme;
use crate::transitionsui::Badge;

/// La taille du symbole ⋈ d'une transition sur sa coupe (assez grand pour l'attraper à la souris).
pub(crate) const SYMBOLE: [f32; 2] = [18.0, 16.0];
/// La largeur d'une icône d'en-tête de rangée, et l'écart de centre à centre.
const CASE_W: f32 = 18.0;
const PAS: f32 = 21.0;
/// Partie 179 : la marge entre la dernière icône d'un en-tête et le bord droit de la rangée (celle des colonnes de la ligne
/// « Calques », `layerheads::MARGE_COLONNES`).
const MARGE_DROITE: f32 = 3.0;

/// La forme d'une transition, dans le Storyboard, devient son symbole : un petit rectangle centré sur sa coupe (`left` : l'abscisse
/// de l'image 0 du film, `ppf` : la largeur d'une image), sans poignées. Elle ne couvre plus les plans.
pub(crate) fn badge_en_symbole(b: &mut Badge, left: f32, ppf: f32, mid_y: f32) {
    let cx = left + b.zone.cut as f32 * ppf;
    let body = Rect::from_center_size(pos2(cx, mid_y), vec2(SYMBOLE[0], SYMBOLE[1]));
    b.body = body;
    b.grip_left = Rect::from_min_max(body.min, pos2(body.min.x, body.max.y));
    b.grip_right = Rect::from_min_max(pos2(body.max.x, body.min.y), body.max);
}

/// Les abscisses des coupes des plans `blocks` (le bord gauche de chacun et le bord droit du dernier ; deux bords voisins à moins
/// d'un point n'en font qu'un).
pub(crate) fn abscisses_des_coupes(blocks: &[Rect]) -> Vec<f32> {
    let mut xs: Vec<f32> = Vec::with_capacity(blocks.len() + 1);
    for b in blocks {
        for x in [b.left() - 1.0, b.right() + 1.0] {
            if !xs.iter().any(|&y| (y - x).abs() < 1.0) {
                xs.push(x);
            }
        }
    }
    xs
}

/// Les traits de coupe des plans, de la rangée Caméra au bas des pistes de calques (point 5 de la demande d'Anthony).
pub(crate) fn peindre_coupes(painter: &Painter, r: Rect, blocks: &[Rect], rows: &Rows) {
    let (y0, y1) = (r.top() + rows.camera_top(), r.top() + rows.background_top());
    let clip = painter.clip_rect();
    let trait_ = Stroke::new(1.0_f32, theme::texte_doux().gamma_multiply(0.55));
    for x in abscisses_des_coupes(blocks) {
        if x >= clip.left() - 1.0 && x <= clip.right() + 1.0 {
            painter.line_segment([pos2(x, y0), pos2(x, y1)], trait_);
        }
    }
}

/// Le nœud papillon ⋈ dans `r` (le symbole d'une transition, peint : la police n'a pas ce signe).
fn peindre_noeud(painter: &Painter, r: Rect, encre: Color32) {
    let (c, w, h) = (r.center(), r.width() * 0.32, r.height() * 0.3);
    let gauche = vec![pos2(c.x - w, c.y - h), pos2(c.x, c.y), pos2(c.x - w, c.y + h)];
    let droite = vec![pos2(c.x + w, c.y - h), pos2(c.x + w, c.y + h), pos2(c.x, c.y)];
    painter.add(Shape::convex_polygon(gauche, encre, Stroke::NONE));
    painter.add(Shape::convex_polygon(droite, encre, Stroke::NONE));
}

impl App {
    /// Une icône sans cadre d'un en-tête de rangée, dans `case` ; sa bulle (nom, raccourci de `action`, description), clic droit :
    /// assigner un raccourci. Grisée (et sans effet au clic) quand `actif` est faux.
    #[allow(clippy::too_many_arguments)]
    fn icone_d_entete(&mut self, ui: &mut egui::Ui, case: Rect, symbole: &str, nom: &str, description: &str, action: &str, actif: bool) -> bool {
        let encre = ui.visuals().text_color();
        let encre = if actif { encre } else { encre.gamma_multiply(0.35) };
        let resp = ui.put(case, egui::Button::new(RichText::new(symbole).size(crate::tailleicones::icone()).color(encre)).frame(false));
        crate::monpanneauui::apprendre(ui.ctx(), action, symbole, nom, description);
        self.aide(resp, nom, description, action).clicked() && actif
    }

    /// Un menu d'en-tête de rangée : son icône dans `case` ; un clic l'ouvre.
    fn menu_d_entete(&mut self, ui: &mut egui::Ui, case: Rect, symbole: &str, nom: &str, description: &str, contenu: impl FnOnce(&mut App, &mut egui::Ui)) {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(case).id_salt(("menu_entete_sb", nom)));
        child.spacing_mut().button_padding = vec2(0.0, 0.0);
        let encre = child.visuals().text_color();
        let bouton = egui::Button::new(RichText::new(symbole).size(crate::tailleicones::icone()).color(encre)).frame(false).min_size(case.size());
        let resp = egui::menu::menu_custom_button(&mut child, bouton, |ui| contenu(self, ui)).response;
        crate::bulles::bulle(resp, nom, description, "");
    }

    /// Les cases des `n` icônes d'un en-tête de rangée `r`, de gauche à droite, collées au bord droit de la rangée (partie 179,
    /// Anthony : « Justifie toutes les icônes à droite » : la dernière finit où finissent les colonnes de la ligne « Calques »,
    /// `layerheads::case_de_colonne`).
    fn cases_d_entete(r: Rect, n: usize) -> Vec<Rect> {
        // (Partie 179.5 : les cases suivent la taille des icônes, `tailleicones.rs` ; 18 et 21 points pour des icônes de 13.)
        let (case_w, pas) = (crate::tailleicones::mesure(CASE_W), crate::tailleicones::mesure(PAS));
        let derniere = r.right() - MARGE_DROITE - case_w;
        (0..n).map(|k| Rect::from_min_size(pos2(derniere - (n - 1 - k) as f32 * pas, r.top() + 1.0), vec2(case_w, (r.height() - 2.0).max(10.0)))).collect()
    }

    /// Le mot d'une rangée (« Plans », « Caméra »…) à l'abscisse `x` de `r` ; rend son bord droit.
    fn mot_d_entete(ui: &egui::Ui, r: Rect, x: f32, mot: &str, encre: Color32) -> f32 {
        let g = ui.painter().layout_no_wrap(mot.to_string(), FontId::proportional(11.5), encre);
        let w = g.size().x;
        ui.painter().with_clip_rect(r).galley(pos2(x, r.center().y - g.size().y * 0.5), g, encre);
        x + w
    }

    /// Partie 179.11 (Anthony : « C'est vrai qu'il manque une ligne de séquence ») : l'en-tête de la rangée des séquences, au format
    /// de Plans et Caméra : « Séquence », puis collées à droite : nouvelle séquence, renommer la séquence, ses versions (menu),
    /// « ⋯ » (nouvelle séquence avec les plans choisis, couper la séquence au plan courant). Il n'existe ni « dupliquer », ni
    /// « supprimer », ni couleur de séquence : une séquence vidée de ses plans s'en va d'elle-même.
    pub(crate) fn entete_sequences_sb(&mut self, ui: &mut egui::Ui, r: Rect) {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r).id_salt("entete_sequences_sb"));
        child.set_clip_rect(r.intersect(ui.clip_rect()));
        let ui = &mut child;
        let doux = ui.visuals().weak_text_color();
        Self::mot_d_entete(ui, r, r.left() + 6.0, "Séquence", doux);
        let c = Self::cases_d_entete(r, 4);
        let mut lancer: Option<&str> = None;
        if self.icone_d_entete(ui, c[0], ph::STACK_PLUS, "Nouvelle séquence", "Après la séquence courante, avec un plan.", "sequence.nouvelle", true) {
            lancer = Some("sequence.nouvelle");
        }
        if self.icone_d_entete(ui, c[1], ph::PENCIL_SIMPLE, "Renommer la séquence", "Le nom de la séquence courante (son titre sur la planche).", "sequence.renommer", true) {
            lancer = Some("sequence.renommer");
        }
        if let Some(seq) = self.sequence_courante() {
            self.menu_d_entete(ui, c[2], ph::STACK, "Versions de la séquence", "Nouvelle version (en gardant ou en copiant les plans, ou vide), passer à une autre, renommer, comparer, supprimer.", |app, ui| {
                app.variantes_menu_contenu(ui, seq);
            });
        }
        self.menu_d_entete(ui, c[3], ph::DOTS_THREE, "Autres commandes des séquences", "Nouvelle séquence avec les plans choisis, couper la séquence au plan courant. Aussi au clic droit sur une séquence.", |app, ui| {
            app.action_item(ui, "Nouvelle séquence avec les plans choisis", "sequence.nouvelle_avec_plans");
            app.action_item(ui, "Couper la séquence ici (le plan courant la commence)", "sequence.couper_ici");
        });
        if let Some(action) = lancer {
            self.run_action(action);
        }
    }

    /// L'en-tête de la rangée des plans : « Plans », puis nouveau plan, dupliquer, versions, transitions (le panneau Préréglages,
    /// onglet Montage), couper le plan à la tête de lecture, « ⋯ » (le reste des actions de plan).
    pub(crate) fn entete_plans_sb(&mut self, ui: &mut egui::Ui, r: Rect) {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r).id_salt("entete_plans_sb"));
        child.set_clip_rect(r.intersect(ui.clip_rect()));
        let ui = &mut child;
        let doux = ui.visuals().weak_text_color();
        Self::mot_d_entete(ui, r, r.left() + 6.0, "Plans", doux);
        let c = Self::cases_d_entete(r, 6);
        let mut lancer: Option<&str> = None;
        if self.icone_d_entete(ui, c[0], ph::PLUS_SQUARE, "Nouveau plan", "Après le plan courant, dans la même séquence.", "panneau.nouveau", true) {
            lancer = Some("panneau.nouveau");
        }
        if self.icone_d_entete(ui, c[1], ph::COPY, "Dupliquer le plan", "Une copie du plan courant, juste après lui.", "panneau.dupliquer", true) {
            lancer = Some("panneau.dupliquer");
        }
        let cur = self.cur_panel;
        self.menu_d_entete(ui, c[2], ph::GIT_BRANCH, "Versions du plan", "Nouvelle version, passer à une autre, la restaurer, comparer. Sur le plan, l'étiquette « V2 ▸ » les déplie.", |app, ui| {
            app.version_menu(ui, cur, true);
        });
        let transitions = self.icone_d_entete(
            ui,
            c[3],
            ph::GRADIENT,
            "Transitions",
            "Le panneau Préréglages, onglet Montage : les transitions, à glisser sur une coupe entre deux plans (le symbole ⋈ y apparaît), et les plans types.",
            "",
            true,
        );
        if transitions {
            crate::panneauprereglages::choisir_onglet(ui.ctx(), crate::panneauprereglages::Onglet::Montage);
            self.show_pane(crate::dock::Pane::Prereglages);
        }
        if self.icone_d_entete(ui, c[4], ph::SCISSORS, "Couper le plan à la tête de lecture", "Deux plans : l'image courante commence le second.", "panneau.couper", true) {
            lancer = Some("panneau.couper");
        }
        let locked = self.doc.storyboard_locked;
        let aide = if locked {
            "Le storyboard est verrouillé (le cadenas) : ce menu le déverrouille. Aussi : nouveau plan avant, supprimer, réunir, durée, fiche du plan, séquence, couleur."
        } else {
            "Nouveau plan avant, supprimer, réunir, durée, fiche du plan, nouvelle séquence, couleur du plan, réglages du projet, verrouiller le storyboard. Aussi au clic droit sur un plan."
        };
        self.menu_d_entete(ui, c[5], if locked { ph::LOCK } else { ph::DOTS_THREE }, "Autres commandes des plans", aide, |app, ui| {
            app.action_item(ui, "Nouveau plan avant le plan courant", "plan.nouveau_avant");
            app.action_item(ui, "Supprimer les plans choisis", "panneau.supprimer");
            ui.separator();
            app.action_item(ui, "Réunir le plan et le suivant", "panneau.fusionner_suivant");
            app.action_item(ui, "Réunir avec le plan précédent", "scene.joindre_precedente");
            app.action_item(ui, "Durée du plan…", "montage.duree");
            app.action_item(ui, "Nom et fiche du plan…", "panneau.proprietes");
            app.plan_color_menu(ui);
            ui.separator();
            app.action_item(ui, "Nouvelle séquence", "sequence.nouvelle");
            app.action_item(ui, "Renommer la séquence…", "sequence.renommer");
            ui.separator();
            app.action_item(ui, "Réglages du projet…", "projet.proprietes");
            app.action_item(ui, if app.doc.storyboard_locked { "Déverrouiller le storyboard" } else { "Verrouiller le storyboard" }, "storyboard.verrouiller");
        });
        if let Some(action) = lancer {
            self.run_action(action);
        }
    }

    /// Partie 178 : la rangée Caméra du Storyboard dépliée (la rangée des préréglages sous ses clés), ou repliée.
    pub(crate) fn basculer_camera_sb(&mut self) {
        self.settings.storyboard_camera_depliee = !self.settings.storyboard_camera_depliee;
        let _ = self.settings.save();
        self.status = if self.settings.storyboard_camera_depliee {
            "Caméra dépliée : la rangée des préréglages, où poser, glisser, étirer les préréglages de caméra.".into()
        } else {
            "Caméra repliée : la ligne de ses clés (un trait jaune y montre les préréglages posés).".into()
        };
    }

    /// Partie 178 : la rangée Calques du Storyboard repliée en une ligne de résumé, ou dépliée (les pistes des calques).
    pub(crate) fn basculer_calques_sb(&mut self) {
        self.settings.storyboard_calques_replies = !self.settings.storyboard_calques_replies;
        let _ = self.settings.save();
        self.status = if self.settings.storyboard_calques_replies {
            "Calques repliés : une ligne de résumé, les clés de chaque plan (⊕ ou le même raccourci les déplie).".into()
        } else {
            "Calques dépliés : les pistes des calques du plan courant.".into()
        };
    }

    /// L'en-tête de la rangée Caméra (`r`) : ⊕ / ⊖ (déplier ou replier la rangée des préréglages), « Caméra », puis poser une clé de
    /// caméra, préréglages (le panneau Préréglages), remettre la caméra à zéro, « ⋯ » (convertir en clés, copier la caméra…).
    pub(crate) fn entete_camera_sb(&mut self, ui: &mut egui::Ui, r: Rect, camera_seule: bool) {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r).id_salt("entete_camera_sb"));
        child.set_clip_rect(r.intersect(ui.clip_rect()));
        let ui = &mut child;
        let depliee = self.settings.storyboard_camera_depliee;
        let plie = Rect::from_min_size(pos2(r.left() + 3.0, r.top() + 1.0), vec2(16.0, r.height() - 2.0));
        let (nom, quoi) = if depliee {
            ("Replier la caméra", "La caméra sur la ligne de ses clés seulement ; un trait jaune y montre les préréglages posés.")
        } else {
            ("Déplier la caméra", "La rangée des préréglages de caméra sous ses clés : y poser, glisser, étirer, régler les préréglages (comme dans la timeline de l'Animation).")
        };
        let resp = ui.put(plie, egui::Button::new(RichText::new(if depliee { ph::MINUS_CIRCLE } else { ph::PLUS_CIRCLE }).size(crate::tailleicones::icone()).color(ui.visuals().weak_text_color())).frame(false));
        if crate::bulles::bulle(resp, nom, quoi, "").clicked() {
            self.basculer_camera_sb();
        }
        let encre = if camera_seule { ui.visuals().strong_text_color() } else { ui.visuals().text_color() };
        Self::mot_d_entete(ui, r, plie.right() + 3.0, "Caméra", encre);
        let c = Self::cases_d_entete(r, 4);
        let ici = self.camera_key_here().is_none();
        let mut lancer: Option<&str> = None;
        let quoi = if ici { "Une clé de caméra à l'image courante : elle se pose sur la ligne de la caméra." } else { "Il y a déjà une clé de caméra à cette image." };
        if self.icone_d_entete(ui, c[0], ph::DIAMOND, "Clé de caméra ici", quoi, "camera.cle_ici", ici) {
            lancer = Some("camera.cle_ici");
        }
        if self.icone_d_entete(ui, c[1], ph::FILM_STRIP, "Préréglages de caméra", "Le panneau Préréglages, onglet Caméra : glisser un préréglage sur la caméra d'un plan, ou cliquer pour le poser à la tête de lecture. La caméra se déplie.", "vue.prereglages", true) {
            crate::panneauprereglages::choisir_onglet(ui.ctx(), crate::panneauprereglages::Onglet::Camera);
            self.show_pane(crate::dock::Pane::Prereglages);
            if !self.settings.storyboard_camera_depliee {
                self.basculer_camera_sb();
            }
        }
        let des_cles = self.doc.panels.get(self.cur_panel).is_some_and(|p| !p.camera.is_empty());
        if self.icone_d_entete(ui, c[2], ph::ARROW_COUNTER_CLOCKWISE, "Remettre la caméra à zéro", "Les clés de caméra du plan s'en vont : le cadre revient au centre, sans zoom ni rotation (les préréglages restent ; Ctrl+Z).", "camera.reinitialiser", des_cles) {
            lancer = Some("camera.reinitialiser");
        }
        let effets = self.doc.panels.get(self.cur_panel).map_or(0, |p| p.effets_camera.len());
        self.menu_d_entete(ui, c[3], ph::DOTS_THREE, "Autres commandes de la caméra", "Convertir les préréglages en clés, copier et coller les clés de caméra, clés au début et à la fin, adoucir…", |app, ui| {
            let resp = ui.add_enabled(effets > 0, egui::Button::new("Convertir les préréglages en clés").shortcut_text(app.keymap.label("camera.effet_appliquer")));
            if crate::bulles::bulle(resp, "Convertir les préréglages en clés", "Tous les préréglages du plan deviennent des clés de caméra, avec la même image ; une étape d'annulation.", "").clicked() {
                ui.close_menu();
                let pi = app.cur_panel;
                app.convertir_effets_en_cles(pi);
            }
            app.action_item(ui, "Copier les clés de caméra choisies", "camera.cles_copier");
            app.action_item(ui, "Coller les clés de caméra", "camera.cles_coller");
            ui.separator();
            app.grid_menu_ui(ui, crate::handles::GridMenu::Camera);
        });
        if let Some(action) = lancer {
            self.run_action(action);
        }
    }

    /// L'en-tête de la rangée des préréglages (`r`, la caméra dépliée) : son nom et le nombre de préréglages du plan courant.
    pub(crate) fn entete_prereglages_sb(&mut self, ui: &mut egui::Ui, r: Rect) {
        let n = self.doc.panels.get(self.cur_panel).map_or(0, |p| p.effets_camera.len());
        let glisse = egui::DragAndDrop::has_payload_of_type::<crate::blocscamera::PresetDrag>(ui.ctx());
        let texte = match n {
            _ if glisse => "Lâche le préréglage ici".to_string(),
            0 => "Préréglages".to_string(),
            1 => "Préréglages · 1 sur ce plan".to_string(),
            n => format!("Préréglages · {n} sur ce plan"),
        };
        let painter = ui.painter_at(r);
        let c = pos2(r.left() + 25.0, r.center().y);
        painter.rect_filled(Rect::from_center_size(c, vec2(7.0, 7.0)), 1.0, theme::camera());
        painter.text(pos2(c.x + 9.0, c.y), Align2::LEFT_CENTER, texte, FontId::proportional(11.0), ui.visuals().weak_text_color());
        let resp = ui.interact(r, ui.id().with("entete_prereglages_sb"), Sense::hover());
        crate::bulles::bulle(
            resp,
            "Préréglages de caméra",
            "Les blocs jaunes : glisser un préréglage du panneau Préréglages dessus (ou sur la ligne de la caméra) ; un bloc se glisse par son corps, s'étire par ses bords ; un clic le choisit (le panneau Caméra le règle), Suppr l'enlève, clic droit : retirer, convertir en clés.",
            "",
        );
    }

    /// L'en-tête de la rangée Calques (`r`) : ⊕ / ⊖ (la rangée repliée en une ligne de résumé, ou dépliée), « Calques », puis, collées
    /// à droite, les commandes des calques (partie 179.5 : elles ont pris la place des icônes des sept colonnes de la partie 174).
    pub(crate) fn entete_calques_sb(&mut self, ui: &mut egui::Ui, r: Rect, replies: bool) {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r).id_salt("entete_calques_sb"));
        child.set_clip_rect(r.intersect(ui.clip_rect()));
        let ui = &mut child;
        // (Partie 179 : le clic droit de la ligne, hors d'un bouton, est celui de l'ancienne barre du bas : nouveau calque, coller.)
        let vide = ui.interact(r, ui.id().with("entete_calques_sb_vide"), Sense::click());
        self.menu_vide_des_calques(&vide);
        let plie = Rect::from_min_size(pos2(r.left() + 3.0, r.top() + 1.0), vec2(16.0, r.height() - 2.0));
        let (nom, quoi) = if replies {
            ("Déplier la rangée Calques", "Les pistes des calques du plan courant (le calque actif sur deux rangées).")
        } else {
            ("Replier la rangée Calques", "Une seule ligne de résumé (les clés de chaque plan) : la timeline laisse plus de place au dessin.")
        };
        let resp = ui.put(plie, egui::Button::new(RichText::new(if replies { ph::PLUS_CIRCLE } else { ph::MINUS_CIRCLE }).size(crate::tailleicones::icone()).color(ui.visuals().weak_text_color())).frame(false));
        if self.aide(resp, nom, quoi, crate::layerheads::ACTION_REPLIER).clicked() {
            self.basculer_calques_sb();
        }
        let n = self.layers().iter().filter(|l| !l.folder).count();
        let mot = if replies { format!("Calques · {n}") } else { "Calques".to_string() };
        let apres_le_mot = Self::mot_d_entete(ui, r, plie.right() + 3.0, &mot, ui.visuals().weak_text_color());
        // Partie 179 (Anthony : la barre du bas de la colonne entourée, une flèche vers cette ligne) : les commandes des calques sur
        // cette ligne (`barrecalques.rs`). Partie 179.5 (le groupe entouré, une flèche vers la droite, les icônes des colonnes barrées) :
        // justifiées à droite, à la place des icônes des colonnes, au même bord que les icônes des lignes Plans et Caméra. Les icônes
        // des colonnes ont quitté la ligne : la piste du calque actif montre chaque réglage en icône (pâle quand il est éteint).
        self.actions_des_calques_en_ligne(ui, r, apres_le_mot + 6.0, r.right() - MARGE_DROITE);
    }

    /// La rangée des préréglages de la timeline du Storyboard : les blocs de chaque plan visible (`blocs` : les plans à l'écran),
    /// pris à la souris comme dans la timeline de l'Animation. `lacher` : un clic ailleurs dans la bande lâche le bloc choisi.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prereglages_bande_sb(&mut self, ui: &mut egui::Ui, painter: &Painter, r: Rect, blocks: &[Rect], ppf: f32, rows: &Rows, lacher: bool) {
        if lacher {
            self.blocs.choisi = None;
        }
        let Some(sb) = rows.sb.filter(|s| s.prereglages_h > 0.0) else { return };
        let haut = r.top() + rows.prereglages_top();
        let cam = r.top() + rows.camera_top();
        let clip = painter.clip_rect();
        for (i, b) in blocks.iter().enumerate() {
            let band = Rect::from_min_max(pos2(b.left() - 1.0, haut), pos2(b.right() + 1.0, haut + sb.prereglages_h));
            // (Un bloc qu'on tire dans un plan sorti de l'écran garde son geste : on ne saute que les plans hors de vue sans geste.)
            let tenu = self.blocs.glisse.is_some_and(|g| g.panel == self.doc.panels[i].id);
            if !tenu && !band.expand(8.0).intersects(clip) {
                continue;
            }
            let cles = Rect::from_min_max(pos2(b.left() - 1.0, cam), pos2(b.right() + 1.0, cam + rows.cam_h()));
            // (La partie pâle d'un bloc plus long que son plan ne passe pas sur le plan suivant.)
            let p = painter.with_clip_rect(band.intersect(clip));
            self.blocs_camera_du_plan(ui, &p, i, band, cles, b.left() - 1.0, ppf, sb.rang_h, true);
        }
    }

    /// Le symbole ⋈ de chaque transition sur sa coupe (`badges`, déjà en symboles : `badge_en_symbole`), et sous lui un trait fin de
    /// la durée de la transition (`left`, `ppf` : l'image 0 du film et la largeur d'une image ; `bas` : le bas de la rangée des plans).
    /// Un clic sur le symbole ouvre le menu de la transition (sorte, durée, sens, calage, supprimer : partie 175) ; le clic droit aussi.
    pub(crate) fn transitions_sb(&mut self, ui: &mut egui::Ui, painter: &Painter, badges: &[Badge], left: f32, ppf: f32, bas: f32) {
        let clip = painter.clip_rect();
        for b in badges {
            if !clip.intersects(b.body.expand(4.0)) {
                continue;
            }
            // La durée, en trait fin au bas de la rangée des plans (décalée comme le symbole s'il y a des versions dépliées).
            let dx = b.body.center().x - (left + b.zone.cut as f32 * ppf);
            let (x0, x1) = (left + b.zone.start as f32 * ppf + dx, left + b.zone.end as f32 * ppf + dx);
            if x1 > x0 {
                painter.rect_filled(Rect::from_min_max(pos2(x0, bas - 3.0), pos2(x1, bas - 1.0)), 1.0, theme::accent_texte().gamma_multiply(0.8));
            }
            let id = b.id;
            let kind = b.kind;
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(b.body).id_salt(("transition_sb", id)));
            child.set_clip_rect(b.body.expand(1.0).intersect(clip));
            child.spacing_mut().button_padding = vec2(0.0, 0.0);
            let bouton = egui::Button::new("").min_size(b.body.size()).corner_radius(3.0);
            let resp = egui::menu::menu_custom_button(&mut child, bouton, |ui| self.transition_menu(ui, id)).response;
            let choisie = self.trans.sel == Some(id);
            let encre = if resp.hovered() || choisie { theme::contour() } else { theme::texte_fort() };
            peindre_noeud(child.painter(), resp.rect, encre);
            if choisie {
                child.painter().rect_stroke(resp.rect, 3.0, Stroke::new(1.5_f32, theme::contour()), StrokeKind::Inside);
            }
            if resp.clicked() || resp.secondary_clicked() {
                self.trans.sel = Some(id);
                self.trans.focus = true;
                self.panel_focus = true;
            }
            resp.context_menu(|ui| self.transition_menu(ui, id));
            let nom = format!("Transition : {}", kind.name());
            crate::bulles::bulle(resp, &nom, "Clic : son menu (sorte, durée, sens, calage, supprimer). Glisser un préréglage de transition sur une coupe la pose ou la change ; Suppr retire la transition choisie.", "");
        }
    }
}
