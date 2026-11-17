//! Les dessins des panneaux de la partie 163 branchés sur le projet : chaque dessin, quand on le manipule, change directement le projet
//! (ou l'affichage), par le même chemin que le champ en chiffres qu'il remplace, avec la même étape d'annulation. Les dessins et les
//! petits calculs sont dans `panneauxvisuels.rs`.
//!
//! - 163.1, Fiche du plan : la durée en barre qu'on étire (la poignée de fin du plan de la timeline, `Handle::PlanEnd` : Ctrl tenue,
//!   les clés suivent, une étape par geste), les versions en vignettes.
//! - 163.2, Vignettes : le mouvement de caméra de la case courante en pictogrammes, son démarrage et son arrêt en courbes.
//! - 163.3, Vignettes : la planche (sens de lecture, cases par ligne, textes sous les cases) en dessins.
//! - 163.4, la fenêtre d'export : les familles en vignettes, un aperçu de la première page ou image.

use std::time::Instant;

use eframe::egui::{self, pos2, vec2, Align2, Color32, CursorIcon, FontId, Rect, RichText, Sense, Stroke, StrokeKind};

use crate::app::App;
use crate::boardsens::{Champ, Sens};
use crate::camera::CameraKey;
use crate::handles::Handle;
use crate::panneauxvisuels::{self as pv, Courbe, Famille, Mouvement};
use crate::planvers::etiquette;
use crate::storyboard::duration_label;
use crate::theme;

/// La clé (mémoire d'egui) de l'étendue figée de la barre de durée pendant qu'on la tire.
fn cle_barre() -> egui::Id {
    egui::Id::new("fiche_duree_barre_163")
}

impl App {
    // ------------------------------------------------------------------ 163.1 : la durée, une barre qu'on étire

    /// La barre de durée de la Fiche du plan : graduée en secondes, la durée dessus (« 1 s 20 i · 44 images »), une poignée au bout.
    /// La tirer est exactement la poignée de fin du plan de la timeline (`Handle::PlanEnd`) : ce qui sort va en réserve, Ctrl tenue fait
    /// suivre toutes les clés, une étape d'annulation par geste, Échap abandonne.
    pub(crate) fn fiche_duree_barre(&mut self, ui: &mut egui::Ui) {
        let fps = self.doc.fps;
        let len = self.frame_count();
        let tenue = self.handle_drag.as_ref().is_some_and(|d| d.handle == Handle::PlanEnd);
        let figee = ui.ctx().data(|d| d.get_temp::<usize>(cle_barre()));
        if !tenue && figee.is_some() {
            ui.ctx().data_mut(|d| d.remove::<usize>(cle_barre()));
        }
        let etendue = figee.filter(|_| tenue).unwrap_or_else(|| pv::etendue_barre(len, fps));
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width().max(80.0), 34.0), Sense::click_and_drag());
        let barre = Rect::from_min_size(rect.min, vec2(rect.width(), 22.0));
        let (g, d) = (barre.left() + 5.0, barre.right() - 7.0);
        let x_de = |n: usize| g + (d - g) * n as f32 / etendue.max(1) as f32;
        let fin = x_de(len).min(d + 40.0);
        let poignee = Rect::from_center_size(pos2(fin, barre.center().y), vec2(7.0, 26.0));
        if ui.is_rect_visible(rect) {
            let p = ui.painter_at(rect.expand2(vec2(0.0, 2.0)));
            p.rect_filled(barre, 3.0, theme::champ());
            let plein = Rect::from_min_max(barre.min, pos2(fin, barre.bottom()));
            p.rect_filled(plein, 3.0, theme::selection());
            // Les secondes : un trait chacune, leur nombre dessous quand il a la place.
            let fps_u = fps.max(1) as usize;
            let secondes = etendue / fps_u;
            let pas_px = (d - g) / secondes.max(1) as f32;
            let tous = if pas_px >= 16.0 { 1 } else if pas_px * 5.0 >= 16.0 { 5 } else { 10 };
            for s in 1..=secondes {
                let x = x_de(s * fps_u);
                let dedans = x < fin;
                let c = if dedans { theme::sur_selection().gamma_multiply(0.35) } else { theme::graduation_seconde() };
                p.line_segment([pos2(x, barre.top() + 3.0), pos2(x, barre.bottom() - 3.0)], Stroke::new(1.0_f32, c));
                if s % tous == 0 {
                    p.text(pos2(x, barre.bottom() + 1.0), Align2::CENTER_TOP, format!("{s} s"), FontId::proportional(9.0), theme::texte_doux());
                }
            }
            let texte = pv::texte_duree(len, fps);
            let police = FontId::proportional(11.0);
            let galee = p.layout_no_wrap(texte.clone(), police.clone(), Color32::WHITE);
            let (pos, c) = if galee.size().x + 12.0 < plein.width() {
                (pos2(barre.left() + 6.0, barre.center().y), theme::sur_selection())
            } else {
                (pos2(fin + 8.0, barre.center().y), theme::texte())
            };
            p.text(pos, Align2::LEFT_CENTER, texte, police, c);
            let vive = tenue || resp.hovered();
            p.rect_filled(poignee, 2.0, if vive { theme::accent() } else { theme::accent().gamma_multiply(0.85) });
        }
        if resp.hovered() || tenue {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
        }
        let resp = crate::bulles::bulle(resp, crate::handles::BULLE_DUREE_NOM, crate::handles::BULLE_DUREE_AIDE, "");
        // Comme la poignée de la timeline : le geste ne se prend qu'à la poignée (un appui ailleurs sur la barre ne fait rien).
        let sur_la_poignee = ui.input(|i| i.pointer.press_origin()).is_some_and(|p| poignee.expand2(vec2(8.0, 4.0)).contains(p));
        if resp.drag_started() && sur_la_poignee && self.handle_drag.is_none() && self.begin_handle_drag(Handle::PlanEnd) {
            ui.ctx().data_mut(|d| d.insert_temp(cle_barre(), etendue));
        }
        let a_nous = self.handle_drag.as_ref().is_some_and(|d| d.handle == Handle::PlanEnd) && ui.ctx().data(|d| d.get_temp::<usize>(cle_barre())).is_some();
        if a_nous {
            if let Some(pt) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
                let n = pv::duree_a(pt.x, g, d, etendue);
                self.drag_handle(n, n);
            }
            // Lâché ici ou ailleurs : le geste se pose (une étape).
            if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
                self.end_handle_drag();
                ui.ctx().data_mut(|d| d.remove::<usize>(cle_barre()));
            }
        }
    }

    // ------------------------------------------------------------------ 163.1 : les versions en vignettes

    /// Les versions du plan courant en petites vignettes : la miniature de la première image du plan dans cette version, « V1 · 2s 13 »,
    /// la version active allumée ; un clic passe à une autre version ; « + nouvelle » en crée une (en dupliquant). Au clic droit d'une
    /// vignette : comparer, restaurer, supprimer, ou (la version active) les deux nouvelles versions et la mise à jour des autres versions
    /// de séquence.
    pub(crate) fn fiche_versions(&mut self, ui: &mut egui::Ui) {
        let index = self.cur_panel;
        let Some(plan) = self.doc.panels.get(index).map(|p| p.id) else { return };
        let fps = self.doc.fps;
        let mut cartes = crate::pilevers::cartes(&self.doc, plan);
        if cartes.is_empty() {
            cartes.push(crate::pilevers::Carte { numero: self.doc.numero_de_version(plan), actuelle: true, date: 0, duree: self.frame_count() });
        }
        let sequence = self.doc.sequence_of(index);
        let ailleurs = !self.doc.variantes_montrant(sequence, plan).is_empty();
        let ctx = ui.ctx().clone();
        let deadline = Instant::now() + crate::thumbs::THUMB_TIME;
        // (Les largeurs de vignettes déjà faites ailleurs : celle de la bande des plans pour la version active, celle de la pile des
        // versions du montage pour les autres. Une largeur neuve ferait composer, puis repeindre, une vignette de plus au repos.)
        let largeur_tex = crate::thumbs::width_for(80.0);
        let textures: Vec<Option<egui::TextureId>> = cartes
            .iter()
            .map(|c| {
                if c.actuelle {
                    self.thumbs.texture(&ctx, &self.doc, index, crate::thumbs::STRIP_WIDTH, deadline)
                } else {
                    match self.doc.versions_du_plan(plan).and_then(|v| v.rangee(c.numero)) {
                        Some(r) => self.thumbs.texture_rangee(&ctx, &self.doc, &r.panel, largeur_tex, deadline),
                        None => None,
                    }
                }
            })
            .collect();
        let rapport = self.doc.frame[0] as f32 / self.doc.frame[1].max(1) as f32;
        let w = pv::largeur_tuiles(ui, 3, 4.0).min(88.0);
        let haut = ((w - 8.0) / rapport).clamp(24.0, 50.0);
        let (mut passer, mut nouvelle) = (None, false);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            for (c, tex) in cartes.iter().zip(&textures) {
                let nom = format!("{} · {}", etiquette(c.numero), duration_label(c.duree, fps));
                let resp = pv::tuile(ui, w, haut, c.actuelle, &nom, |p, r, encre| {
                    let image = pv::au_rapport(r, rapport);
                    match tex {
                        Some(t) => {
                            p.image(*t, image, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                        }
                        None => {
                            p.rect_filled(image, 0.0, encre.gamma_multiply(0.15));
                        }
                    }
                    p.rect_stroke(image, 0.0, Stroke::new(1.0_f32, encre.gamma_multiply(0.4)), StrokeKind::Outside);
                });
                let quand = if c.date > 0 { format!(" Gardée le {}.", crate::envois::date_courte(c.date)) } else { String::new() };
                let par = crate::variantesui::montree_par(&self.doc, sequence, plan, c.numero);
                let par = if par.is_empty() { String::new() } else { format!(" Montrée par {par}.") };
                let (titre, aide) = if c.actuelle {
                    (format!("Version {} (active)", etiquette(c.numero)), format!("La version active du plan : celle que montrent le montage, l'export et les envois.{quand}{par} Clic droit : nouvelle version, comparer."))
                } else {
                    (format!("Version {}", etiquette(c.numero)), format!("Un clic y passe : elle devient la version active, celle d'aujourd'hui est gardée.{quand}{par} Clic droit : comparer, restaurer, supprimer."))
                };
                let resp = crate::bulles::bulle(resp, &titre, &aide, "");
                if resp.clicked() && !c.actuelle {
                    passer = Some(c.numero);
                }
                let numero = c.numero;
                let actuelle = c.actuelle;
                resp.context_menu(|ui| {
                    if actuelle {
                        self.nouvelle_version_menu(ui, index);
                        if ailleurs && ui.button("Mettre à jour dans les autres versions de séquence…").clicked() {
                            ui.close_menu();
                            self.mettre_a_jour_demander(index);
                        }
                        ui.separator();
                        if ui.button("Comparer le plan…").on_hover_text("Deux versions, ou aujourd'hui et un envoi (lecture seule).").clicked() {
                            ui.close_menu();
                            self.comparer_ouvrir(index, None, None);
                        }
                    } else {
                        if ui.button(format!("Passer à la {}", etiquette(numero))).clicked() {
                            ui.close_menu();
                            self.passer_a_la_version(index, numero);
                        }
                        if ui.button("Comparer à la version d'aujourd'hui").on_hover_text("Côte à côte, en volet, en fondu ou en superposition (lecture seule).").clicked() {
                            ui.close_menu();
                            self.comparer_ouvrir(index, Some(crate::comparer::Source::Version(numero)), Some(crate::comparer::Source::Actuelle));
                        }
                        if ui.button(format!("Restaurer la {}", etiquette(numero))).on_hover_text("Une copie d'elle devient une nouvelle version active ; rien n'est perdu.").clicked() {
                            ui.close_menu();
                            self.restaurer_la_version(index, numero);
                        }
                        if ui.button(format!("Supprimer la {}", etiquette(numero))).on_hover_text("Elle sort du plan (Ctrl+Z la ramène).").clicked() {
                            ui.close_menu();
                            self.supprimer_la_version(index, numero);
                        }
                    }
                });
            }
            let resp = pv::tuile(ui, w, haut, false, "+ nouvelle", |p, r, encre| {
                let m = r.center();
                let t = r.height().min(r.width()) * 0.28;
                p.line_segment([m - vec2(t, 0.0), m + vec2(t, 0.0)], Stroke::new(1.5_f32, encre));
                p.line_segment([m - vec2(0.0, t), m + vec2(0.0, t)], Stroke::new(1.5_f32, encre));
            });
            let r = self.raccourci("version.nouvelle");
            let resp = crate::bulles::bulle(resp, "Nouvelle version, en dupliquant", "Garde le plan tel qu'il est comme version plus ancienne et continue sur une copie. Clic droit : une nouvelle version vide.", &r);
            if resp.clicked() {
                nouvelle = true;
            }
            resp.context_menu(|ui| self.nouvelle_version_menu(ui, index));
        });
        if let Some(n) = passer {
            self.passer_a_la_version(index, n);
        }
        if nouvelle {
            self.nouvelle_version_du_plan(index);
        }
    }

    // ------------------------------------------------------------------ 163.2 : le mouvement de caméra de la case courante

    /// L'image où commence la case courante des Vignettes (la case du plan courant qui contient la tête de lecture).
    pub(crate) fn debut_de_la_case_courante(&self) -> usize {
        let cases = crate::vignettecam::cases_du_plan(&self.doc, self.cur_panel);
        cases.iter().copied().filter(|&c| c <= self.cur_frame).max().unwrap_or(0)
    }

    /// Pose le mouvement `m` sur la case qui commence à l'image `debut` du plan `pi` : le cadre B part du cadre A (la case) ; un
    /// mouvement qui a déjà ses cadres garde A et ses cadres d'après, seul B change. Une étape d'annulation, nommée. Mêmes refus et même
    /// coupe du plan que « Ajouter un cadre » (`cam_ajouter_cadre_case`).
    pub(crate) fn vignettes_poser_mouvement(&mut self, pi: usize, debut: usize, m: Mouvement) {
        let Some(id) = self.doc.panels.get(pi).map(|p| p.id) else { return };
        self.poser_le_geste_de_camera();
        if !self.doc.camera_en_cadres(pi) {
            self.status = crate::vignettecamui::CAMERA_SANS_CADRES.into();
            return;
        }
        if self.content_refuses() || !self.partage_peut_toucher(&[id]) {
            return;
        }
        let Some(mvt) = self.mouvement_de_case(pi, debut) else {
            self.status = self.refus_camera_en_route(pi, debut);
            return;
        };
        let (keys0, a) = self.cles_du_mouvement(pi, &mvt);
        let frame = self.doc.frame;
        // Au bout de ce que la caméra sait faire (panoramique au plus loin, zoom arrière au plus large) : B serait A, rien n'est posé.
        let depuis = if keys0.len() >= 2 { keys0[0].cam } else { a };
        if pv::mouvements_de(frame, &depuis, &pv::cadre_b(m, frame, depuis)).is_empty() {
            self.status = format!("{} : la caméra ne peut pas aller plus loin dans ce sens ; rien n'est changé.", m.nom());
            return;
        }
        if keys0.len() >= 2 && keys0[1].cam == pv::cadre_b(m, frame, keys0[0].cam) {
            self.status = format!("Le mouvement est déjà « {} ».", m.nom());
            return;
        }
        let keys = if keys0.len() >= 2 {
            let mut k = keys0.clone();
            k[1].cam = pv::cadre_b(m, frame, k[0].cam);
            k
        } else {
            match crate::vignettecam::avec_nouveau_cadre(&[], a, pv::cadre_b(m, frame, a), mvt.len) {
                Some(k) => k,
                None => {
                    self.status = if debut == 0 { "Ce plan est trop court pour un mouvement : allonge-le d'abord." } else { "Cette case est trop courte pour un mouvement : allonge-la d'abord." }.into();
                    return;
                }
            }
        };
        let avant = if keys0.len() >= 2 { keys0.len() } else { 0 };
        if avant == 0 && self.doc.panels[pi].marks.iter().any(|mk| mk.frame > debut) && self.storyboard_refuses() {
            return;
        }
        let orig = self.doc.panels[pi].camera.clone();
        let tenait = if mvt.rangs.is_empty() { a } else { orig[mvt.rangs.end - 1].cam };
        let nouvelles = crate::vignettecam::recolle(&orig, &mvt.rangs, debut, &keys, tenait);
        if nouvelles == orig {
            self.status = format!("Le mouvement est déjà « {} ».", m.nom());
            return;
        }
        self.end_stroke();
        let snap = self.snapshot();
        self.doc.panels[pi].camera = nouvelles;
        let scinde = avant == 0 && self.scinder_derriere_le_mouvement(pi, &orig, debut);
        self.etape_camera(snap, &format!("Mouvement de caméra : {}", m.nom()));
        let (premier, second) = (crate::vignettecam::lettre(mvt.lettre0), crate::vignettecam::lettre(mvt.lettre0 + 1));
        self.status = format!("Mouvement de caméra : {} ({premier} → {second}). Ctrl+Z le reprend.", m.nom().to_lowercase());
        if scinde {
            self.status += " Les cases qui suivaient forment un plan à part (une coupe après celle-ci).";
        }
    }

    /// Fait jouer la courbe `c` (démarrage et arrêt) à chaque intervalle du mouvement de la case qui commence à l'image `debut` du plan
    /// `pi` : les adoucis de ses clés (`ease_out`, `ease_in`), ceux que montrent les poignées d'adoucis de la piste de caméra. Une étape.
    pub(crate) fn vignettes_poser_courbe(&mut self, pi: usize, debut: usize, c: Courbe) {
        let Some(id) = self.doc.panels.get(pi).map(|p| p.id) else { return };
        self.poser_le_geste_de_camera();
        if self.content_refuses() || !self.partage_peut_toucher(&[id]) {
            return;
        }
        let Some(mvt) = self.mouvement_de_case(pi, debut).filter(|m| m.porte && m.rangs.len() >= 2) else {
            self.status = "Pas encore de mouvement sur cette case : choisis d'abord le mouvement (Travelling, Zoom…).".into();
            return;
        };
        let orig = self.doc.panels[pi].camera.clone();
        let nouvelles = pv::avec_la_courbe(&orig, mvt.rangs.clone(), c);
        if nouvelles == orig {
            self.status = format!("Le mouvement est déjà « {} ».", c.nom());
            return;
        }
        self.end_stroke();
        let snap = self.snapshot();
        self.doc.panels[pi].camera = nouvelles;
        self.etape_camera(snap, &format!("Démarrage et arrêt : {}", c.nom()));
        self.status = format!("Démarrage et arrêt de la caméra : {}. Ctrl+Z le reprend.", c.nom().to_lowercase());
    }

    /// La partie « Mouvement » du panneau Vignettes, pour la case courante : les mouvements que la caméra sait faire, en pictogrammes (celui
    /// d'aujourd'hui allumé), comment il démarre et s'arrête, en courbes ; « Précis » : les cadres et leurs images, les adoucis en images.
    /// Les cadres A et B se voient sur la case elle-même. Rien quand la caméra de ce plan ne se dit pas en cadres (partie 159).
    pub(crate) fn vignettes_mouvement(&mut self, ui: &mut egui::Ui) {
        let pi = self.cur_panel;
        if pi >= self.doc.panels.len() || !self.doc.camera_en_cadres(pi) {
            return;
        }
        let debut = self.debut_de_la_case_courante();
        ui.add_space(2.0);
        ui.separator();
        let Some(mvt) = self.mouvement_de_case(pi, debut) else {
            ui.label(RichText::new("Mouvement de caméra").strong());
            ui.label(RichText::new("La caméra est en route sur cette case : son mouvement se règle sur la case où il commence.").small().color(theme::texte_doux()));
            return;
        };
        let keys = self.doc.panels[pi].camera.clone();
        let porte = mvt.porte && mvt.rangs.len() >= 2;
        let titre = if porte {
            let n = mvt.rangs.len();
            format!("Mouvement de caméra · {} → {}", crate::vignettecam::lettre(mvt.lettre0), crate::vignettecam::lettre(mvt.lettre0 + n - 1))
        } else {
            "Mouvement de caméra".to_string()
        };
        ui.label(RichText::new(titre).strong());
        let allumes = if porte { pv::mouvements_de(self.doc.frame, &keys[mvt.rangs.start].cam, &keys[mvt.rangs.start + 1].cam) } else { Vec::new() };
        pv::titre(ui, "Le mouvement");
        let w = pv::largeur_tuiles(ui, 3, 4.0);
        let mut pose = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            for m in Mouvement::TOUS {
                let on = allumes.contains(&m);
                let resp = pv::tuile(ui, w, 30.0, on, m.nom(), |p, r, c| m.dessiner(p, r, c));
                if crate::bulles::bulle(resp, &format!("Mouvement : {}", m.nom()), m.description(), "").clicked() {
                    pose = Some(m);
                }
            }
        });
        let courbe = if porte { pv::courbe_des(&keys, mvt.rangs.clone()) } else { None };
        pv::titre(ui, "Comment ça démarre et s'arrête");
        let mut demarre = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            for c in Courbe::TOUTES {
                let resp = pv::tuile(ui, w, 28.0, courbe == Some(c), c.nom(), |p, r, e| c.dessiner(p, r, e));
                if crate::bulles::bulle(resp, &format!("Démarrage : {}", c.nom()), c.description(), "").clicked() {
                    demarre = Some(c);
                }
            }
        });
        pv::precis(ui, "Mouvement de caméra", |ui| self.vignettes_mouvement_precis(ui, pi, debut));
        if let Some(m) = pose {
            self.vignettes_poser_mouvement(pi, debut, m);
        }
        if let Some(c) = demarre {
            self.vignettes_poser_courbe(pi, debut, c);
        }
    }

    /// « Précis » du mouvement : chaque cadre et l'image du plan où il est ; pour chaque passage d'un cadre au suivant, ses adoucis de
    /// départ et d'arrivée en images (une étape par saisie) ; « Ajouter un cadre ».
    fn vignettes_mouvement_precis(&mut self, ui: &mut egui::Ui, pi: usize, debut: usize) {
        let Some(mvt) = self.mouvement_de_case(pi, debut) else { return };
        if mvt.porte && mvt.rangs.len() >= 2 {
            let keys: Vec<CameraKey> = self.doc.panels[pi].camera[mvt.rangs.clone()].to_vec();
            egui::Grid::new("mouvement_precis").num_columns(3).spacing([6.0, 2.0]).show(ui, |ui| {
                for (k, key) in keys.iter().enumerate() {
                    let lettre = crate::vignettecam::lettre(mvt.lettre0 + k);
                    ui.label(format!("Cadre {lettre}"));
                    ui.label(RichText::new(format!("image {}", key.frame + 1)).color(theme::texte_doux()));
                    ui.label("");
                    ui.end_row();
                    if let Some(suivante) = keys.get(k + 1) {
                        let span = suivante.frame.saturating_sub(key.frame).max(1) as f32;
                        let (mut depart, mut arrivee) = crate::camera::interval_eases(key, suivante);
                        ui.label(RichText::new(format!("{lettre} → {}", crate::vignettecam::lettre(mvt.lettre0 + k + 1))).color(theme::texte_doux()));
                        let r1 = ui.add(egui::DragValue::new(&mut depart).range(0.0..=span).speed(0.1).max_decimals(1).prefix("départ ").suffix(" i"));
                        let r1 = crate::bulles::bulle(r1, "Adouci de départ", "Sur combien d'images la caméra accélère en quittant ce cadre (0 : elle part d'un coup).", "");
                        let r2 = ui.add(egui::DragValue::new(&mut arrivee).range(0.0..=span).speed(0.1).max_decimals(1).prefix("arrivée ").suffix(" i"));
                        let r2 = crate::bulles::bulle(r2, "Adouci d'arrivée", "Sur combien d'images la caméra ralentit avant le cadre suivant (0 : elle s'arrête d'un coup).", "");
                        ui.end_row();
                        let i = mvt.rangs.start + k;
                        if self.edited(&r1) {
                            if let Some(c) = self.doc.panels[pi].camera.get_mut(i) {
                                c.ease_out = depart;
                                c.smooth = false;
                            }
                        }
                        if self.edited(&r2) {
                            if let Some(c) = self.doc.panels[pi].camera.get_mut(i + 1) {
                                c.ease_in = arrivee;
                                c.smooth = false;
                            }
                        }
                    }
                }
            });
        } else {
            ui.label(RichText::new("Pas encore de mouvement : un pictogramme le pose, ou Ctrl + Maj + glisser dans la case.").small().color(theme::texte_doux()));
        }
        let resp = ui.button("Ajouter un cadre");
        if crate::bulles::bulle(resp, "Ajouter un cadre", "Un cadre de plus après le dernier, qu'on glisse ensuite dans la case (comme au clic droit de la case).", "").clicked() {
            self.cam_ajouter_cadre_case(pi, debut);
        }
    }

    // ------------------------------------------------------------------ 163.3 : la planche en dessins

    /// Le haut du panneau Vignettes en dessins : le sens de lecture (deux pages), les cases par ligne (une mini-planche dont on tire le
    /// bord), les textes sous les cases (une case et ses lignes, cochables). « Précis » : les séquences montrées, les numéros de plan.
    pub(crate) fn vignettes_planche_visuelle(&mut self, ui: &mut egui::Ui) {
        // Le sens de lecture.
        pv::titre(ui, "Sens de lecture");
        let w = pv::largeur_tuiles(ui, 2, 4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            for sens in Sens::TOUS {
                let on = self.layout.board_sens == sens;
                let resp = pv::tuile(ui, w, 36.0, on, sens.nom(), |p, r, c| pv::dessiner_sens(p, r, c, sens.japonais()));
                let action = Self::sens_action(sens);
                if self.aide(resp, sens.nom(), sens.description(), action).clicked() {
                    self.run_action(action);
                }
            }
        });
        ui.add_space(2.0);
        // Les cases par ligne (par colonne, à la japonaise).
        let japonais = self.layout.board_sens.japonais();
        let (nom, description) = if japonais {
            ("Cases par colonne", "Combien de cases dans une colonne avant la suivante, à droite (six au plus). Tire le bas de la mini-planche, ou clique où elle doit s'arrêter.")
        } else {
            ("Cases par ligne", "Combien de cases dans une ligne avant la suivante, dessous (six au plus). Tire le bord droit de la mini-planche, ou clique où elle doit s'arrêter.")
        };
        pv::titre(ui, nom);
        self.mini_planche(ui, nom, description, japonais);
        ui.add_space(2.0);
        // Les textes sous les cases.
        pv::titre(ui, if japonais { "Textes à droite des cases" } else { "Textes sous les cases" });
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(96.0, 62.0), Sense::hover());
            let p = ui.painter_at(rect);
            p.rect_filled(rect, 3.0, theme::champ());
            let case = if japonais { Rect::from_min_size(rect.min + vec2(4.0, 6.0), vec2(48.0, 30.0)) } else { Rect::from_min_size(rect.min + vec2(14.0, 4.0), vec2(68.0, 30.0)) };
            p.rect_filled(case, 0.0, theme::texte().gamma_multiply(0.85));
            let mut y = if japonais { case.top() + 2.0 } else { case.bottom() + 4.0 };
            for champ in Champ::TOUS {
                if !self.layout.board_textes.montre(champ) {
                    continue;
                }
                let [r, g, b] = champ.encre();
                let (x0, x1) = if japonais { (case.right() + 4.0, rect.right() - 4.0) } else { (case.left(), case.right() - 12.0) };
                p.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(3.0_f32, Color32::from_rgb(r, g, b)));
                p.line_segment([pos2(x0, y + 4.0), pos2(x0 + (x1 - x0) * 0.6, y + 4.0)], Stroke::new(2.0_f32, Color32::from_rgb(r, g, b).gamma_multiply(0.6)));
                y += 9.0;
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                for champ in Champ::TOUS {
                    let mut on = self.layout.board_textes.montre(champ);
                    let [r, g, b] = champ.encre();
                    let resp = ui.checkbox(&mut on, RichText::new(champ.nom()).color(Color32::from_rgb(r, g, b)));
                    let action = Self::texte_action(champ);
                    let description = if japonais {
                        format!("Montrer le champ « {} » du plan à droite de chaque case (étiqueté quand plusieurs champs sont montrés).", champ.nom())
                    } else {
                        format!("Montrer le champ « {} » du plan sous chaque case (étiqueté quand plusieurs champs sont montrés).", champ.nom())
                    };
                    if self.aide(resp, champ.nom(), &description, action).changed() {
                        self.vignettes_basculer_texte(champ);
                    }
                }
            });
        });
        pv::precis(ui, "Planche", |ui| {
            // Les séquences : lesquelles la planche montre, dans quel ordre (`planchesequencesui.rs`).
            self.vignettes_sequences(ui);
            // Les numéros de plan : montrés d'origine ; la touche N les cache ou les montre (planche et export).
            let mut numeros = self.settings.vignettes.numeros_visibles;
            let resp = ui.checkbox(&mut numeros, "Numéros de plan");
            if self.aide(resp, "Numéros de plan", "Montrer le numéro de chaque plan dans la marge, au-dessus de sa première case (et dans l'export de la planche). Un affichage : rien n'est changé dans le projet.", "vignettes.numeros").changed() {
                self.vignettes_basculer_numeros();
            }
            let resp = ui.add(egui::DragValue::new(&mut self.layout.board_columns).range(1..=crate::board::MAX_COLUMNS).speed(0.05).prefix(format!("{nom} : ")));
            crate::bulles::bulle(resp, &format!("{nom} (en chiffres)"), description, "");
        });
    }

    /// La mini-planche : autant de cases que de cases par ligne (une rangée de plus, en pâle, dessous), une poignée au bout. Tirer la
    /// poignée (ou cliquer) change le nombre de cases par ligne, entre une et six. Un réglage de l'application, comme le champ d'avant.
    fn mini_planche(&mut self, ui: &mut egui::Ui, nom: &str, description: &str, japonais: bool) {
        let max = crate::board::MAX_COLUMNS;
        let n = self.layout.board_columns.clamp(1, max);
        let largeur = ui.available_width().max(120.0);
        let hauteur = if japonais { 112.0 } else { 64.0 };
        let (rect, resp) = ui.allocate_exact_size(vec2(largeur, hauteur), Sense::click_and_drag());
        let texte = format!("{n} case{} par {}", if n > 1 { "s" } else { "" }, if japonais { "colonne" } else { "ligne" });
        // La géométrie : une case et son écart, dans le sens où la planche se remplit.
        let (debut, pas, case) = if japonais {
            let pas = (hauteur - 26.0) / max as f32;
            (rect.top() + 6.0, pas, vec2(pas * 1.5, pas - 3.0))
        } else {
            let pas = (largeur - 26.0) / max as f32;
            // (Deux rangées et le texte tiennent dans la hauteur, quelle que soit la largeur du panneau.)
            (rect.left() + 8.0, pas, vec2(pas - 4.0, ((pas - 4.0) * 0.6).min((hauteur - 6.0 - 4.0 - 18.0) / 2.0)))
        };
        if let Some(pt) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
            let pos = if japonais { pt.y } else { pt.x };
            let neuf = pv::cases_a(pos, debut, pas, max);
            if neuf != self.layout.board_columns {
                self.layout.board_columns = neuf;
                self.status = format!("Vignettes : {neuf} case{} par {}.", if neuf > 1 { "s" } else { "" }, if japonais { "colonne" } else { "ligne" });
            }
        }
        if resp.hovered() || resp.dragged() {
            ui.ctx().set_cursor_icon(if japonais { CursorIcon::ResizeVertical } else { CursorIcon::ResizeHorizontal });
        }
        if ui.is_rect_visible(rect) {
            let p = ui.painter_at(rect);
            p.rect_filled(rect, 3.0, theme::champ());
            let papier = theme::texte().gamma_multiply(0.85);
            for rang in 0..2 {
                let pale = rang == 1;
                let combien = if pale { n.min(3) } else { n };
                for k in 0..combien {
                    let q = if japonais {
                        Rect::from_min_size(pos2(rect.left() + 8.0 + rang as f32 * (case.x + 6.0), debut + k as f32 * pas), case)
                    } else {
                        Rect::from_min_size(pos2(debut + k as f32 * pas, rect.top() + 6.0 + rang as f32 * (case.y + 4.0)), case)
                    };
                    p.rect_filled(q, 0.0, if pale { papier.gamma_multiply(0.35) } else { papier });
                }
            }
            let bout = debut + n as f32 * pas;
            let poignee = if japonais { Rect::from_min_size(pos2(rect.left() + 6.0, bout - 2.0), vec2(case.x * 2.0 + 10.0, 5.0)) } else { Rect::from_min_size(pos2(bout - 3.0, rect.top() + 4.0), vec2(5.0, case.y * 2.0 + 8.0)) };
            p.rect_filled(poignee, 2.0, theme::accent());
            let at = if japonais { pos2(rect.right() - 6.0, rect.bottom() - 4.0) } else { pos2(rect.left() + 8.0, rect.bottom() - 4.0) };
            let align = if japonais { Align2::RIGHT_BOTTOM } else { Align2::LEFT_BOTTOM };
            p.text(at, align, texte, FontId::proportional(11.0), theme::texte());
        }
        crate::bulles::bulle(resp, nom, description, "");
    }

    // ------------------------------------------------------------------ 163.4 : la fenêtre d'export

    /// Les familles d'export en vignettes, en haut de la fenêtre d'export. Un clic choisit la famille (son format d'origine) ; « Pour le
    /// montage » a sa propre fenêtre : vrai quand on l'a choisie (la fenêtre d'export se ferme et la sienne s'ouvre à sa place).
    pub(crate) fn export_tuiles(&mut self, ui: &mut egui::Ui, s: &mut crate::export::ExportSettings) -> bool {
        use crate::export::Format;
        let ici = Famille::de(s.format);
        let mut montage = false;
        let w = pv::largeur_tuiles(ui, 4, 6.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            for f in Famille::TOUTES {
                let resp = pv::tuile(ui, w, 48.0, ici == Some(f), f.nom(), |p, r, c| f.dessiner(p, r, c));
                if crate::bulles::bulle(resp, &format!("Export : {}", f.nom()), f.description(), "").clicked() && ici != Some(f) {
                    match f {
                        Famille::Storyboard => s.format = Format::Pdf,
                        Famille::Film => s.format = Format::Mp4,
                        Famille::Images => s.format = Format::Png,
                        Famille::Montage => montage = true,
                    }
                }
            }
        });
        montage
    }

    /// L'aperçu, avant d'exporter : la première page du storyboard (ses cases, les premiers plans de l'étendue), ou la première image.
    /// Les vignettes viennent des fils de fond, comme partout : une case qui n'est pas prête reste grise.
    pub(crate) fn export_apercu(&mut self, ui: &mut egui::Ui, s: &crate::export::ExportSettings, frames: &[(usize, usize)]) {
        use crate::export::Format;
        let plans = crate::export::panels_of(frames);
        let ctx = ui.ctx().clone();
        let deadline = Instant::now() + crate::thumbs::THUMB_TIME;
        let rapport = self.doc.frame[0] as f32 / self.doc.frame[1].max(1) as f32;
        let (zone, _) = ui.allocate_exact_size(vec2(ui.available_width(), 104.0), Sense::hover());
        let p = ui.painter_at(zone);
        p.rect_filled(zone, 4.0, theme::champ());
        // La page et l'image sont au papier du projet (dans tous les thèmes : une page reste une page) ; les textes, une encre sourde.
        let [pr, pg, pb] = self.doc.paper;
        let papier = Color32::from_rgb(pr, pg, pb);
        let sourd = theme::texte_doux();
        if s.format == Format::Pdf {
            let (cols, rangs) = crate::pdfmise::grid(s.pdf.per_page_ok(), s.pdf.landscape);
            let page = pv::au_rapport(zone.shrink(6.0), if s.pdf.landscape { 1.414 } else { 0.707 });
            p.rect_filled(page, 1.0, papier);
            let marge = page.width() * 0.06;
            let interieur = page.shrink(marge);
            let (cw, ch) = (interieur.width() / cols.max(1) as f32, interieur.height() / rangs.max(1) as f32);
            let largeur_tex = crate::thumbs::width_for(cw);
            for k in 0..cols * rangs {
                let (c, r) = ((k % cols) as f32, (k / cols) as f32);
                let cellule = Rect::from_min_size(interieur.min + vec2(c * cw, r * ch), vec2(cw, ch)).shrink(2.0);
                let image = pv::au_rapport(Rect::from_min_size(cellule.min, vec2(cellule.width(), cellule.height() * 0.7)), rapport);
                match plans.get(k).and_then(|&pi| self.thumbs.texture(&ctx, &self.doc, pi, largeur_tex, deadline)) {
                    Some(t) => {
                        p.image(t, image, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    }
                    None if k < plans.len() => {
                        p.rect_filled(image, 0.0, sourd.gamma_multiply(0.3));
                    }
                    None => {}
                }
                if k < plans.len() {
                    p.rect_stroke(image, 0.0, Stroke::new(0.5_f32, sourd), StrokeKind::Outside);
                    let y = image.bottom() + 3.0;
                    p.line_segment([pos2(image.left(), y), pos2(image.left() + image.width() * 0.7, y)], Stroke::new(1.5_f32, sourd.gamma_multiply(0.7)));
                }
            }
            p.text(zone.left_bottom() + vec2(6.0, -4.0), Align2::LEFT_BOTTOM, "Première page", FontId::proportional(10.0), theme::texte_doux());
        } else {
            let image = pv::au_rapport(zone.shrink(8.0), rapport);
            let largeur_tex = crate::thumbs::width_for(image.width());
            match plans.first().and_then(|&pi| self.thumbs.texture(&ctx, &self.doc, pi, largeur_tex, deadline)) {
                Some(t) => {
                    p.image(t, image, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
                None => {
                    p.rect_filled(image, 0.0, papier);
                }
            }
            p.rect_stroke(image, 0.0, Stroke::new(1.0_f32, theme::bord_fort()), StrokeKind::Outside);
            p.text(zone.left_bottom() + vec2(6.0, -4.0), Align2::LEFT_BOTTOM, "Première image", FontId::proportional(10.0), theme::texte_doux());
        }
    }
}

/// Le texte du gros bouton de la fenêtre d'export : ce qu'on va obtenir.
pub(crate) fn texte_exporter(f: crate::export::Format) -> &'static str {
    use crate::export::Format;
    match f {
        Format::Pdf => "Exporter le storyboard…",
        Format::Gif => "Exporter le GIF…",
        Format::Mp4 | Format::ProRes | Format::ProRes4444 | Format::DnxHr => "Exporter le film…",
        Format::Png | Format::Stills => "Exporter les images…",
        _ => "Exporter…",
    }
}
