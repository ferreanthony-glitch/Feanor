//! Partie 198 : les panneaux de l'onglet 3D (`onglet3d.rs`, `docs/INTERFACE.md` partie 198).
//!
//! - **Scène 3D** (`scene3d_panel`) : le sommaire de la séquence (`onglet3d::sommaire`) : le décor, les caméras de ses plans (le plan
//!   courant marqué), les lumières, les cartes du plan courant ; une recherche ; les groupes se replient. Une icône, un nom, le
//!   cadenas d'un objet, l'œil. Un clic choisit dans la vue (par les mêmes chemins qu'un clic dans la vue : `e3_choisir_objet`,
//!   `e3_choisir_la_camera`, `e3_follow_card`) ; une caméra d'un autre plan mène à ce plan.
//! - **Propriétés** : ses onglets Objet, Caméra, Lumières, Monde et Décor montrent ce que le panneau Espace 3D et le réglage du décor
//!   montraient (`e3_etat_calque`, `e3_etat_camera`, `decor3d_reglages_de`, `e3_environnement_ui`), rangé par sujet.
//! - **La timeline de la séquence** (`timeline_sequence_panel`) : ses plans en blocs, la règle, la tête de lecture, les clés de
//!   caméra de chaque plan, le compteur, la lecture ; un clic sur un plan ou un glisser sur la règle change de plan et d'image.
//! - **La banque 3D** : celle de la Bibliothèque (`banque3d_ui`) ; un clic pose le modèle dans le décor par le chemin d'un import.

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, RichText, Sense, Stroke};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::decor3dui::PartiesDecor;
use crate::dock::Pane;
use crate::onglet3d::{nom_de, sommaire, Groupe3d, Ligne3d};

/// Hauteur d'une ligne du sommaire.
const LIGNE_H: f32 = 22.0;
/// Hauteurs des rangées de la timeline de la séquence : la règle, les plans, les clés de caméra, l'objet choisi.
const REGLE_H: f32 = 18.0;
const PLANS_H: f32 = 30.0;
const RANGEE_H: f32 = 18.0;
/// La largeur des noms des rangées, à gauche.
const NOMS_W: f32 = 96.0;

// Ce que les panneaux ont peint à la dernière image, pour les essais.
#[cfg(test)]
thread_local! {
    pub(crate) static LIGNES_S3D: std::cell::RefCell<Vec<(Ligne3d, Rect)>> = const { std::cell::RefCell::new(Vec::new()) };
    pub(crate) static TIMELINE_SEQ: std::cell::RefCell<Option<(Rect, Vec<(usize, Rect)>)>> = const { std::cell::RefCell::new(None) };
    /// Partie 200 : les losanges des clés de l'objet choisi (image de la séquence, rectangle).
    pub(crate) static CLES_SEQ: std::cell::RefCell<Vec<(usize, Rect)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Partie 201 : le pas de la règle de la timeline de la séquence et ses graduations peintes (images de la séquence).
    pub(crate) static GRADUATIONS_SEQ: std::cell::RefCell<(usize, Vec<usize>)> = const { std::cell::RefCell::new((0, Vec::new())) };
}

impl App {
    // ------------------------------------------------------------------------ Scène 3D

    pub(crate) fn scene3d_panel(&mut self, ui: &mut egui::Ui) {
        if self.panneau_film_vide(ui) {
            return;
        }
        #[cfg(test)]
        LIGNES_S3D.with(|l| l.borrow_mut().clear());
        // (Partie 201 : le sélecteur de séquence, en tête.)
        self.o3_selecteur_de_sequence(ui);
        ui.horizontal(|ui| {
            ui.label(RichText::new(ph::MAGNIFYING_GLASS).weak());
            let r = ui.add(egui::TextEdit::singleline(&mut self.o3.recherche).hint_text("Chercher dans la scène").desired_width(f32::INFINITY));
            crate::bulles::bulle(r, "Chercher dans la scène", "Ne garde que les objets, caméras, lumières et cartes dont le nom contient ce texte.", "");
        });
        let lignes = sommaire(&self.doc, self.cur_panel, &self.o3.recherche, &self.o3.replies);
        egui::ScrollArea::vertical().id_salt("scene3d_liste").auto_shrink([false, false]).show(ui, |ui| {
            for l in lignes {
                // (Partie 201 : une ligne d'objet se glisse vers une séquence.)
                let sens = if matches!(l, Ligne3d::Objet(_)) { Sense::click_and_drag() } else { Sense::click() };
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), LIGNE_H), sens);
                #[cfg(test)]
                LIGNES_S3D.with(|v| v.borrow_mut().push((l, rect)));
                if !ui.is_rect_visible(rect) {
                    continue;
                }
                self.scene3d_ligne(ui, rect, resp, l);
            }
        });
    }

    /// Ce qui est choisi dans la vue, pour une ligne du sommaire.
    pub(crate) fn scene3d_choisie(&self, l: Ligne3d) -> bool {
        let sel = &self.espace3d.sel;
        match l {
            Ligne3d::Objet(id) => sel.contains(&crate::poignees3d::objet_selection(id)),
            Ligne3d::Lampe(id) => sel.contains(&crate::miseenscene3dui::lampe_selection(id)),
            Ligne3d::Camera(pi) => pi == self.cur_panel && sel.contains(&crate::espace3dgizmo::CAMERA_ID),
            Ligne3d::Carte(_, id) => sel.contains(&crate::espace3d::card_selection(id)),
            _ => false,
        }
    }

    fn scene3d_ligne(&mut self, ui: &mut egui::Ui, rect: Rect, resp: egui::Response, l: Ligne3d) {
        let p = ui.painter_at(rect);
        let texte = ui.visuals().text_color();
        let doux = ui.visuals().weak_text_color();
        if let Ligne3d::Groupe(g, n) = l {
            let replie = self.o3.replies.contains(&g);
            p.text(pos2(rect.left() + 8.0, rect.center().y), Align2::LEFT_CENTER, if replie { ph::CARET_RIGHT } else { ph::CARET_DOWN }, FontId::proportional(12.0), doux);
            let t = p.text(pos2(rect.left() + 24.0, rect.center().y), Align2::LEFT_CENTER, g.nom(), FontId::proportional(12.5), ui.visuals().strong_text_color());
            p.text(pos2(t.right() + 6.0, rect.center().y), Align2::LEFT_CENTER, n.to_string(), FontId::proportional(10.5), doux);
            let nom = if replie { format!("Déplier « {} »", g.nom()) } else { format!("Replier « {} »", g.nom()) };
            if crate::bulles::bulle(resp, &nom, "", "").clicked() {
                match self.o3.replies.iter().position(|x| *x == g) {
                    Some(k) => {
                        self.o3.replies.remove(k);
                    }
                    None => self.o3.replies.push(g),
                }
            }
            return;
        }
        let choisie = self.scene3d_choisie(l);
        let courante = matches!(l, Ligne3d::Camera(pi) if pi == self.cur_panel);
        if choisie {
            p.rect_filled(rect.shrink2(vec2(2.0, 1.0)), 3.0, crate::theme::surface_active(crate::theme::accent(), 0.45));
        } else if resp.hovered() {
            p.rect_filled(rect.shrink2(vec2(2.0, 1.0)), 3.0, ui.visuals().widgets.hovered.weak_bg_fill);
        }
        let (icone, couleur) = match l {
            Ligne3d::Objet(_) => (ph::CUBE, Color32::from_rgb(160, 160, 160)),
            Ligne3d::Camera(_) => (ph::VIDEO_CAMERA, crate::theme::camera()),
            Ligne3d::Soleil => (ph::SUN, Color32::from_rgb(250, 205, 80)),
            Ligne3d::Ciel => (ph::CLOUD, Color32::from_rgb(140, 180, 230)),
            Ligne3d::Lampe(id) => (ph::LIGHTBULB, self.o3_lampe(id).map_or(texte, |l| crate::couleurs3d::color32(l.couleur))),
            Ligne3d::Carte(li, _) => (ph::CARDS, self.layers().get(li).map_or(texte, crate::layerheads::encre_de_sorte)),
            Ligne3d::Groupe(..) => unreachable!(),
        };
        let x = rect.left() + 22.0;
        p.text(pos2(x, rect.center().y), Align2::LEFT_CENTER, icone, FontId::proportional(crate::tailleicones::icone()), couleur);
        let mut nom = nom_de(&self.doc, self.cur_panel, l);
        if courante {
            nom.push_str(" · plan courant");
        }
        let encre = if courante || choisie { ui.visuals().strong_text_color() } else { texte };
        // À droite : l'œil, et le cadenas d'un objet.
        let w = crate::layerheads::icon_w();
        let oeil = Rect::from_center_size(pos2(rect.right() - 6.0 - w * 0.5, rect.center().y), vec2(w, 18.0));
        let cadenas = oeil.translate(vec2(-w, 0.0));
        // (Partie 199 : un objet a, avant son cadenas, la puce de sa couleur d'identification ; un clic ouvre la palette.)
        let puce = Rect::from_center_size(pos2(cadenas.left() - 10.0, rect.center().y), vec2(14.0, 14.0));
        let fin_du_nom = match l {
            Ligne3d::Objet(_) => puce.left(),
            Ligne3d::Lampe(_) => cadenas.left(),
            _ => oeil.left(),
        } - 4.0;
        p.with_clip_rect(Rect::from_min_max(pos2(x + 18.0, rect.top()), pos2(fin_du_nom, rect.bottom()))).text(pos2(x + 20.0, rect.center().y), Align2::LEFT_CENTER, &nom, FontId::proportional(12.0), encre);
        if let Ligne3d::Objet(id) = l {
            self.puce_de_couleur(ui, puce, id);
            // (Partie 201 : son menu, copier ou déplacer vers une autre séquence ; et le glisser.)
            self.o3_menu_d_objet(&resp, id);
            self.o3_glisser_une_ligne(&resp, id);
        }
        let visible = self.scene3d_visible(l);
        let r = crate::layerheads::icon_at(ui, oeil, if visible { ph::EYE } else { ph::EYE_SLASH }, if visible { texte } else { doux.gamma_multiply(0.6) });
        if crate::bulles::bulle(r, "Afficher / cacher", "Montrer ou cacher dans la vue (une carte : comme l'œil de son calque ; un objet du décor : aussi dans les images des plans, gardé dans le projet).", "").clicked() {
            self.scene3d_basculer_oeil(l);
        }
        if let Ligne3d::Lampe(id) = l {
            // (Partie 200 : une lampe s'enlève d'ici, à la place du cadenas des objets.)
            let r = crate::layerheads::icon_at(ui, cadenas, ph::TRASH, doux.gamma_multiply(0.8));
            if crate::bulles::bulle(r, "Enlever la lampe", "Enlève la lampe du décor de la séquence (Ctrl+Z la rend).", "").clicked() {
                self.o3_enlever_lampe(id);
                return;
            }
        }
        if let Ligne3d::Objet(id) = l {
            let verrou = self.e3_objet(id).is_some_and(|o| o.verrou);
            let r = crate::layerheads::icon_at(ui, cadenas, if verrou { ph::LOCK_SIMPLE } else { ph::LOCK_SIMPLE_OPEN }, if verrou { texte } else { doux.gamma_multiply(0.6) });
            if crate::bulles::bulle(r, "Verrouiller / déverrouiller", "Un objet verrouillé ne se choisit plus dans la vue : on ne le déplace pas par erreur (gardé dans le projet).", "").clicked() {
                self.scene3d_basculer_verrou(id);
            }
        }
        let resp = crate::bulles::bulle(resp, &nom_de(&self.doc, self.cur_panel, l), "Un clic : le choisir dans la vue (une caméra d'un autre plan : y aller). Maj : l'ajouter à ce qui est choisi.", "");
        if resp.clicked() {
            let maj = ui.input(|i| i.modifiers.shift);
            self.scene3d_choisir(l, maj);
        }
    }

    /// Ce que montre l'œil d'une ligne.
    fn scene3d_visible(&self, l: Ligne3d) -> bool {
        match l {
            Ligne3d::Objet(id) => self.e3_objet(id).is_some_and(|o| !o.cache),
            Ligne3d::Camera(pi) => !self.o3.cameras_cachees.contains(&self.doc.panels[pi].id),
            Ligne3d::Soleil => self.o3.montrer_lumieres,
            Ligne3d::Ciel => self.doc.decor_du_plan(self.cur_panel).is_some_and(|d| d.aspect.ciel),
            Ligne3d::Lampe(id) => self.o3_lampe(id).is_some_and(|l| !l.cache),
            Ligne3d::Carte(li, _) => self.layers().get(li).is_some_and(|l| l.visible),
            Ligne3d::Groupe(..) => true,
        }
    }

    /// Un clic sur une ligne du sommaire : la choisir dans la vue.
    pub(crate) fn scene3d_choisir(&mut self, l: Ligne3d, ajouter: bool) {
        match l {
            Ligne3d::Objet(id) => {
                if self.e3_objet(id).is_some_and(|o| o.verrou) {
                    self.status = "Cet objet est verrouillé (son cadenas, dans Scène 3D).".into();
                    return;
                }
                self.e3_choisir_objet(id, ajouter);
            }
            Ligne3d::Camera(pi) => {
                if pi != self.cur_panel {
                    self.select_panel(pi);
                }
                self.e3_choisir_la_camera(false);
            }
            Ligne3d::Carte(li, id) => {
                if self.layers().get(li).is_some_and(|l| l.id == id) {
                    let s = crate::espace3d::card_selection(id);
                    if ajouter {
                        if !self.espace3d.sel.contains(&s) {
                            self.espace3d.sel.push(s);
                        }
                    } else {
                        self.espace3d.sel = vec![s];
                    }
                    self.e3_follow_card(s);
                }
            }
            Ligne3d::Soleil | Ligne3d::Ciel => {
                self.colonne.montrer(crate::dock::Pane::Lumieres3d);
            }
            Ligne3d::Lampe(id) => {
                let s = crate::miseenscene3dui::lampe_selection(id);
                if ajouter {
                    if !self.espace3d.sel.contains(&s) {
                        self.espace3d.sel.push(s);
                    }
                } else {
                    self.espace3d.sel = vec![s];
                }
                self.o3.montrer_lumieres = true;
                if self.colonne.contient(crate::dock::Pane::Lumieres3d) {
                    self.colonne.montrer(crate::dock::Pane::Lumieres3d);
                }
            }
            Ligne3d::Groupe(..) => {}
        }
    }

    /// L'œil d'une ligne : une carte, celui de son calque (`agir_colonne`) ; un objet, caché dans la vue et l'image (pour la séance) ;
    /// une caméra, cachée dans la vue ; le soleil, sa marque dans la vue ; le ciel, celui du décor (une étape d'annulation).
    pub(crate) fn scene3d_basculer_oeil(&mut self, l: Ligne3d) {
        let seq = self.doc.sequence_of(self.cur_panel);
        match l {
            Ligne3d::Objet(id) => {
                // (Partie 199 : gardé dans le projet, une étape d'annulation.)
                let cache = self.e3_objet(id).is_some_and(|o| o.cache);
                self.decor3d_changer_objet(id, if cache { "Montrer un objet du décor" } else { "Cacher un objet du décor" }, |o| o.cache = !cache);
                if !cache {
                    let s = crate::poignees3d::objet_selection(id);
                    self.espace3d.sel.retain(|x| *x != s);
                }
            }
            Ligne3d::Camera(pi) => {
                let id = self.doc.panels[pi].id;
                match self.o3.cameras_cachees.iter().position(|x| *x == id) {
                    Some(k) => {
                        self.o3.cameras_cachees.remove(k);
                    }
                    None => self.o3.cameras_cachees.push(id),
                }
            }
            Ligne3d::Soleil => self.o3.montrer_lumieres = !self.o3.montrer_lumieres,
            Ligne3d::Lampe(id) => self.o3_basculer_lampe(id),
            Ligne3d::Ciel => {
                if let Some(mut d) = self.doc.decor_du_plan(self.cur_panel).cloned() {
                    d.aspect.ciel = !d.aspect.ciel;
                    self.decor3d_retoucher(seq, d);
                    self.decor3d_retouche_finir();
                }
            }
            Ligne3d::Carte(li, id) => {
                if self.layers().get(li).is_some_and(|l| l.id == id) {
                    self.agir_colonne(crate::layerheads::Colonne::Afficher, li);
                }
            }
            Ligne3d::Groupe(..) => {}
        }
    }

    /// Le cadenas d'un objet du décor (gardé dans le projet depuis la partie 199, une étape d'annulation) : verrouillé, il ne se
    /// choisit plus dans la vue, et il est lâché s'il l'était.
    pub(crate) fn scene3d_basculer_verrou(&mut self, id: u64) {
        let verrou = self.e3_objet(id).is_some_and(|o| o.verrou);
        self.decor3d_changer_objet(id, if verrou { "Déverrouiller un objet du décor" } else { "Verrouiller un objet du décor" }, |o| o.verrou = !verrou);
        if !verrou {
            let s = crate::poignees3d::objet_selection(id);
            self.espace3d.sel.retain(|x| *x != s);
        }
    }

    // ------------------------------------------------------------------------ Propriétés

    /// L'onglet Objet : la carte ou l'objet du décor choisis.
    pub(crate) fn objet3d_panel(&mut self, ui: &mut egui::Ui) {
        if self.panneau_film_vide(ui) {
            return;
        }
        let cartes: Vec<usize> = self.espace3d.sel.iter().filter_map(|&s| self.e3_card_layer(s)).collect();
        if let [li] = cartes.as_slice() {
            self.e3_etat_calque(ui, *li);
            return;
        }
        let objets = self.e3_objets_choisis();
        if !objets.is_empty() {
            // (Partie 199 : en haut, la couleur d'identification de l'objet choisi, ou des objets choisis ensemble.)
            self.couleur_d_identification_ui(ui, &objets);
            self.decor3d_reglages_de(ui, PartiesDecor { objet: true, ..PartiesDecor::RIEN });
            // (Partie 200 : « Dans les plans » et « Animation », pour l'objet de la liste.)
            if let Some(id) = self.decor3d.choisi.filter(|id| objets.contains(id)).or(objets.first().copied()) {
                self.o3_objet_mise_en_scene_ui(ui, id);
            }
            return;
        }
        ui.label(RichText::new("Choisis un objet du décor ou une carte, dans Scène 3D ou dans la vue.").small().weak());
    }

    /// L'onglet Caméra : la caméra du plan choisie.
    pub(crate) fn camera3d_panel(&mut self, ui: &mut egui::Ui) {
        if self.panneau_film_vide(ui) {
            return;
        }
        if self.espace3d.sel.contains(&crate::espace3dgizmo::CAMERA_ID) {
            self.e3_etat_camera(ui);
            return;
        }
        ui.label(RichText::new("Choisis une caméra, dans Scène 3D ou dans la vue.").small().weak());
        if ui.button(format!("{}  Choisir la caméra du plan", ph::VIDEO_CAMERA)).clicked() {
            self.espace3d_pick_camera(false);
        }
    }

    /// L'onglet Lumières : le soleil et le ciel du décor.
    pub(crate) fn lumieres3d_panel(&mut self, ui: &mut egui::Ui) {
        if self.panneau_film_vide(ui) {
            return;
        }
        self.decor3d_reglages_de(ui, PartiesDecor { soleil: true, ciel: true, ..PartiesDecor::RIEN });
        let mut on = self.o3.montrer_lumieres;
        if ui.checkbox(&mut on, "Montrer le soleil et les lampes dans la vue").clicked() {
            self.o3.montrer_lumieres = on;
        }
        // (Partie 200 : les lampes de la séquence.)
        self.o3_lampes_ui(ui);
    }

    /// L'onglet Monde : le ciel, la brume, les facettes du décor, la grille, le fond de la vue.
    pub(crate) fn monde3d_panel(&mut self, ui: &mut egui::Ui) {
        if self.panneau_film_vide(ui) {
            return;
        }
        self.decor3d_reglages_de(ui, PartiesDecor { monde: true, ..PartiesDecor::RIEN });
        let mut grille = self.espace3d.show_grid;
        if ui.checkbox(&mut grille, "Grille du sol").clicked() {
            self.e3_maillage_basculer();
        }
        ui.separator();
        self.e3_environnement_ui(ui);
    }

    /// L'onglet Décor : son poids face au budget de l'ordinateur, les ombres, les bords doux, ses objets (en ajouter, copier vers
    /// une autre séquence, enlever).
    pub(crate) fn decor3d_onglet_panel(&mut self, ui: &mut egui::Ui) {
        if self.panneau_film_vide(ui) {
            return;
        }
        self.decor3d_reglages_de(ui, PartiesDecor { decor: true, ..PartiesDecor::RIEN });
    }

    // ------------------------------------------------------------------------ la timeline de la séquence

    pub(crate) fn timeline_sequence_panel(&mut self, ui: &mut egui::Ui) {
        if self.panneau_film_vide(ui) || self.doc.panels.is_empty() {
            return;
        }
        let (a, z) = self.doc.sequence_run(self.cur_panel);
        let debut = self.doc.panel_start(a);
        let fin = self.doc.panel_start(z - 1) + self.doc.panels[z - 1].frame_count();
        let n = (fin - debut).max(1);
        let g = self.global_frame();
        // La ligne du haut : les onglets de la bande, lecture, le compteur, la séquence.
        ui.horizontal(|ui| {
            self.o3_onglets_de_la_bande(ui);
            ui.separator();
            let r = ui.add(egui::Button::new(if self.playing { ph::PAUSE } else { ph::PLAY }).frame(false));
            if crate::bulles::bulle(r, "Lire / pause", "Lit le film depuis la tête de lecture.", "lecture.basculer").clicked() {
                self.toggle_play_scope(true);
            }
            ui.label(RichText::new(format!("image {} / {}", g.saturating_sub(debut) + 1, n)).monospace());
            ui.label(RichText::new(format!("Séquence {} · {} plan{}", self.doc.sequence_label(self.doc.sequence_of(self.cur_panel)), z - a, if z - a > 1 { "s" } else { "" })).small().weak());
            // (Partie 200 : « ◆ Clé » pour l'objet choisi, au bout de la ligne ; partie 201 : le zoom de la timeline, avant lui.)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.o3_bouton_cle(ui);
                ui.separator();
                self.o3_reglette_du_zoom(ui);
            });
        });
        let w = ui.available_width();
        let hauteur = REGLE_H + PLANS_H + 2.0 * RANGEE_H + 4.0;
        let (rect, resp) = ui.allocate_exact_size(vec2(w, hauteur), Sense::click_and_drag());
        let p = ui.painter_at(rect);
        let piste = Rect::from_min_max(pos2(rect.left() + NOMS_W, rect.top()), rect.max);
        // (Partie 201 : la piste montre la séquence au zoom de la timeline, à partir de son image `debut_seq`.)
        let fen = crate::timelineseq::Fenetre::de(piste, debut, n, self.o3.zoom_seq, self.o3.debut_seq);
        self.o3.debut_seq = fen.depart;
        // (`bande_survolee` : les raccourcis du zoom de l'image zooment la bande quand le pointeur est dessus, partie 192.)
        let dessus = ui.rect_contains_pointer(rect);
        if dessus {
            self.bande_survolee = true;
        }
        self.o3_gestes_de_la_timeline(ui, dessus, fen);
        let fen = crate::timelineseq::Fenetre::de(piste, debut, n, self.o3.zoom_seq, self.o3.debut_seq);
        let x_de = |f: usize| fen.x_de(f);
        let f_de = |x: f32| fen.f_de(x);
        let doux = ui.visuals().weak_text_color();
        p.rect_filled(rect, 0.0, crate::theme::panneau());
        let p_noms = p.clone();
        let p = p.with_clip_rect(piste.intersect(rect));
        // Les noms des rangées.
        let regle = Rect::from_min_size(rect.min, vec2(w, REGLE_H));
        let plans = Rect::from_min_size(pos2(rect.left(), regle.bottom()), vec2(w, PLANS_H));
        let cles = Rect::from_min_size(pos2(rect.left(), plans.bottom() + 2.0), vec2(w, RANGEE_H));
        let objet = Rect::from_min_size(pos2(rect.left(), cles.bottom() + 2.0), vec2(w, RANGEE_H));
        for (r, nom) in [(plans, "Plans"), (cles, "Caméra")] {
            p_noms.text(pos2(r.left() + 6.0, r.center().y), Align2::LEFT_CENTER, nom, FontId::proportional(11.0), doux);
        }
        // La règle : ses graduations selon le zoom (partie 201 : des images, des secondes ou des minutes), seulement celles qu'on voit.
        p.rect_filled(Rect::from_min_max(pos2(piste.left(), regle.top()), pos2(piste.right(), regle.bottom())), 0.0, crate::theme::regle());
        let fps = self.doc.fps.max(1) as usize;
        let pas = crate::timelineseq::pas_de_la_regle(fen.points_par_image(), fps);
        let premiere = (fen.depart.floor() as usize / pas) * pas;
        let derniere = ((fen.depart + fen.visibles).ceil() as usize).min(n);
        let mut graduations = Vec::new();
        let mut f = premiere;
        while f <= derniere && f < n {
            let x = x_de(debut + f);
            p.line_segment([pos2(x, regle.bottom() - 5.0), pos2(x, regle.bottom())], Stroke::new(1.0_f32, doux));
            p.text(pos2(x + 2.0, regle.top() + 1.0), Align2::LEFT_TOP, crate::timelineseq::chiffre(f, fps, pas), FontId::proportional(9.5), doux);
            graduations.push(f);
            f += pas;
        }
        #[cfg(test)]
        GRADUATIONS_SEQ.with(|g| *g.borrow_mut() = (pas, graduations.clone()));
        let _ = &graduations;
        // Les plans, en blocs ; les clés de caméra de chacun.
        let mut blocs = Vec::new();
        for pi in a..z {
            let s = self.doc.panel_start(pi);
            let e = s + self.doc.panels[pi].frame_count();
            let b = Rect::from_min_max(pos2(x_de(s) + 1.0, plans.top() + 2.0), pos2(x_de(e) - 1.0, plans.bottom() - 2.0));
            blocs.push((pi, b));
            let courant = pi == self.cur_panel;
            p.rect_filled(b, 3.0, if courant { crate::theme::surface_active(crate::theme::accent(), 0.6) } else { crate::theme::bloc() });
            if courant {
                p.rect_stroke(b, 3.0, Stroke::new(1.5_f32, crate::theme::contour()), egui::StrokeKind::Inside);
            }
            p.with_clip_rect(b.shrink(2.0)).text(pos2(b.left() + 5.0, b.center().y), Align2::LEFT_CENTER, crate::storyboard::panel_title(&self.doc, pi), FontId::proportional(11.0), ui.visuals().text_color());
            for k in &self.doc.panels[pi].camera {
                let c = pos2(x_de(s + k.frame) + 0.5, cles.center().y);
                p.add(egui::Shape::convex_polygon(vec![c + vec2(0.0, -5.0), c + vec2(5.0, 0.0), c + vec2(0.0, 5.0), c + vec2(-5.0, 0.0)], crate::theme::camera(), Stroke::NONE));
            }
        }
        // (Partie 200 : les clés de l'objet choisi ; « Objet choisi » sans objet.)
        if self.o3_objet_des_cles().is_none() {
            p.text(pos2(objet.left() + 6.0, objet.center().y), Align2::LEFT_CENTER, "Objet choisi", FontId::proportional(11.0), doux);
        }
        self.o3_rangee_des_cles(ui, Rect::from_min_max(objet.min, pos2(piste.left(), objet.bottom())), Rect::from_min_max(pos2(piste.left(), objet.top()), objet.max), fen);
        // La tête de lecture.
        let xg = x_de(g.clamp(debut, fin - 1));
        p.line_segment([pos2(xg, regle.top()), pos2(xg, rect.bottom())], Stroke::new(2.0_f32, crate::timeline::cur_frame()));
        p.add(egui::Shape::convex_polygon(vec![pos2(xg - 5.0, regle.top()), pos2(xg + 5.0, regle.top()), pos2(xg, regle.top() + 7.0)], crate::timeline::cur_frame(), Stroke::NONE));
        #[cfg(test)]
        TIMELINE_SEQ.with(|t| *t.borrow_mut() = Some((piste, blocs.clone())));
        // Les gestes : un clic sur un plan y mène (à son image sous le pointeur) ; un glisser sur la règle mène la tête de lecture.
        if let Some(pos) = resp.interact_pointer_pos().filter(|p| piste.contains(*p) || resp.dragged()) {
            let dans_la_regle = resp.dragged() || pos.y < regle.bottom();
            if (dans_la_regle && (resp.is_pointer_button_down_on() || resp.clicked())) || resp.clicked() {
                self.playing = false;
                let cible = f_de(pos.x);
                if cible != self.global_frame() {
                    self.goto_global(cible);
                }
            }
        }
        crate::bulles::bulle(resp, "Timeline de la séquence", "Ses plans, la tête de lecture, les clés de caméra de chaque plan. Un clic sur un plan y mène ; glisser sur la règle déplace la tête de lecture. La vue et la caméra du plan suivent.", "");
    }

    /// La banque 3D, dans la bande du bas de l'onglet 3D.
    pub(crate) fn banque3d_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| self.o3_onglets_de_la_bande(ui));
        self.banque3d_ui(ui);
    }

    /// Le panneau de la bande du bas de l'onglet 3D qui est devant (l'onglet actif de son groupe d'egui_dock), et les panneaux de ce
    /// groupe. (Lus avant que les panneaux ne se peignent : pendant qu'ils se peignent, egui_dock a la disposition, `dockui.rs`.)
    pub(crate) fn o3_lire_la_bande(&self) -> (Vec<Pane>, Option<Pane>) {
        let Some(state) = self.dock.as_ref() else { return (Vec::new(), None) };
        let Some((s, n, _)) = state.find_tab(&Pane::TimelineSequence).or_else(|| state.find_tab(&Pane::Banque3d)) else { return (Vec::new(), None) };
        match &state[s][n] {
            egui_dock::Node::Leaf { tabs, active, .. } => (tabs.clone(), tabs.get(active.0).copied()),
            _ => (Vec::new(), None),
        }
    }

    /// Le panneau de la bande du bas qui est devant.
    pub(crate) fn o3_bande_devant(&self) -> Option<Pane> {
        if self.dock.is_some() {
            self.o3_lire_la_bande().1
        } else {
            self.o3.bande.1
        }
    }

    /// Partie 198 (vu à l'écran : les groupes d'egui_dock n'ont plus de noms d'onglets depuis la partie 160, la Banque 3D restait
    /// cachée derrière la timeline) : en tête de la bande, ses deux onglets, « Timeline de la séquence » et « Banque 3D », comme la
    /// maquette (`renderBh`) ; celui qui est devant est allumé ; un clic met l'autre devant. Rien quand la bande n'en a qu'un.
    pub(crate) fn o3_onglets_de_la_bande(&mut self, ui: &mut egui::Ui) {
        let (panes, devant) = self.o3.bande.clone();
        if panes.len() < 2 {
            return;
        }
        for (pane, description) in [
            (Pane::TimelineSequence, "Les plans de la séquence, la tête de lecture, les clés de caméra de chaque plan."),
            (Pane::Banque3d, "Les modèles 3D à poser dans le décor de la séquence : un clic, devant la caméra ; glisser sur la vue, au sol."),
        ] {
            if !panes.contains(&pane) {
                continue;
            }
            let on = devant == Some(pane);
            let r = ui.selectable_label(on, crate::bulles::encre_etat(RichText::new(pane.name()).strong(), on));
            if crate::bulles::bulle(r, &format!("Onglet {}", pane.name()), description, "").clicked() && !on {
                // (Posé à l'image suivante, quand egui_dock a rendu la disposition : `o3_suivre`.)
                self.o3.bande_demandee = Some(pane);
                ui.ctx().request_repaint();
            }
        }
    }
}
