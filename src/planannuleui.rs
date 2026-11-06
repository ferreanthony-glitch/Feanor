//! Le geste du plan annulé sur la planche (partie 114, le modèle est dans `planannule.rs`) : un clic sur le numéro du plan l'annule ou le
//! rétablit ; Maj tenue montre les numéros cachés (touche N) et Maj + clic sur le numéro prime sur « Maj + clic choisit le plan » ; le clic
//! droit de la case reste, en chemin secondaire. Comme la barre entre les cases (partie 113), **le numéro ne répond qu'au lâcher d'un appui qui n'a
//! pas bougé** : un appui qui bouge est un trait (sans Maj) ou un plan qu'on déplace (avec Maj), et n'annule rien.

use std::time::Instant;

use eframe::egui::{self, pos2, vec2, Color32, FontId, Modifiers, Pos2, Rect, Stroke, StrokeKind};

use crate::app::{App, Tool};
use crate::board::BoardLayout;
use crate::boardui::{distance_rect, Press, Xf, MOVE_THRESHOLD};
use crate::history::Nav;

/// La croix d'un plan annulé.
pub(crate) const CROIX: Color32 = Color32::from_rgb(214, 32, 32);

/// Un appui sur un numéro de plan, en attente de son lâcher.
#[derive(Clone, Copy, Debug)]
pub(crate) struct NumeroAttente {
    pub plan: u64,
    pub origine: Pos2,
    pub base_len: usize,
    /// L'appui a lancé un trait (sans Maj, un outil de dessin) : au lâcher d'un clic, ce point de trait est retiré.
    pub dessine: bool,
}

impl App {
    /// Annule le plan `plan`, ou le rétablit s'il l'est déjà : une seule étape d'annulation, qui rend tout. Vrai si le film a changé.
    pub(crate) fn plan_annuler_ou_retablir(&mut self, plan: u64) -> bool {
        if self.partage.is_some() {
            self.status = "Dans un film partagé, un plan ne s'annule pas, les autres postes ne le sauraient pas : supprime-le, ou reviens à une copie à toi".into();
            return false;
        }
        let annule = self.doc.annules.plans.iter().any(|g| g.panel.id == plan);
        let nom = if annule { "Rétablir le plan" } else { "Annuler le plan" };
        let titre = crate::numerotation::Numeros::of(&self.doc);
        let scene = self.doc.annules.plans.iter().find(|g| g.panel.id == plan).map(|g| g.panel.scene).or_else(|| self.doc.panels.iter().find(|p| p.id == plan).map(|p| p.scene));
        let numero = scene.map_or_else(String::new, |s| titre.scene_label(&self.doc, s));
        // Y a-t-il du son après le début de ce plan ? Il ne bouge pas : la ligne d'état le dit.
        let debut = self.doc.panels.iter().position(|p| p.id == plan).map(|i| self.doc.panel_start(i));
        let son_apres = debut.is_some_and(|d| self.doc.audio_tracks.iter().flat_map(|t| &t.clips).any(|c| c.end() > d));
        crate::variantes::prendre_refus();
        let ok = self.avec_etape(nom, |a| if annule { a.structure_op_dessins(|d, _| d.retablir_plan(plan)) } else { a.structure_op_dessins(|d, _| d.annuler_plan(plan)) });
        if !ok {
            if let Some(refus) = crate::variantes::prendre_refus() {
                self.status = refus;
            }
            return false;
        }
        self.board_sel.retain(|id| *id != plan);
        self.thumbs.prune(&self.doc);
        self.status = if annule {
            format!("Plan {numero} rétabli à sa place, tout y est")
        } else if self.doc.film_vide() {
            format!("Plan {numero} annulé, tous les plans le sont et plus rien ne sort : un clic sur un numéro en rétablit un")
        } else if son_apres {
            format!("Plan {numero} annulé, rien n'est effacé et le son reste à sa place : un clic sur son numéro le rétablit")
        } else {
            format!("Plan {numero} annulé, rien n'est effacé : un clic sur son numéro le rétablit")
        };
        true
    }

    /// Le plan dont le numéro est sous le point `pos` (écran), à l'image d'avant. Ctrl garde le lasso et la caméra.
    fn numero_sous(&self, pos: Pos2, m: Modifiers) -> Option<u64> {
        if m.ctrl || m.command || m.alt || self.vignettes.camera_tenue {
            return None;
        }
        self.vignettes.numeros_zones.iter().rev().find(|(z, _)| z.contains(pos)).map(|(_, p)| *p)
    }

    /// Un appui sur le numéro d'un plan ou dans une case annulée (`boardui::board_press`). Vrai si l'appui est pris ici.
    pub(crate) fn plan_annule_appui(&mut self, layout: &BoardLayout, pos: Pos2, p: Pos2, m: Modifiers) -> bool {
        if let Some(plan) = self.numero_sous(pos, m) {
            self.commit_note_text(layout);
            self.commit_card_note();
            let mut dessine = false;
            let maj = m.shift;
            if maj {
                // Maj + glisser depuis le numéro déplace le plan, comme depuis la case ; sans mouvement, c'est un clic sur le numéro.
                if let Some(k) = layout.cards.iter().position(|c| c.plan == plan) {
                    self.start_plan_move(layout, k, pos, false);
                } else {
                    self.vignettes.press = Press::Dead;
                }
            } else if matches!(self.tool, Tool::Brush | Tool::Pencil | Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Polyline) {
                dessine = self.trait_depuis_le_papier(layout, pos, p);
            } else {
                self.vignettes.press = Press::Dead;
            }
            self.vignettes.numero_attente = Some(NumeroAttente { plan, origine: pos, base_len: self.history.undo_len(), dessine });
            return true;
        }
        // Dessiner dans une case annulée : refusé, et dit (rien ne s'y perd, rien ne s'y mélange). Le numéro la rétablit.
        if layout.annulee_en(p).is_some() {
            self.commit_note_text(layout);
            self.commit_card_note();
            self.vignettes.press = Press::Dead;
            self.status = "Ce plan est annulé, rien ne s'y dessine : un clic sur son numéro le rétablit".into();
            return true;
        }
        false
    }

    /// Un appui hors de toute case part en trait dans la case la plus proche (comme depuis le papier). Faux s'il n'y a aucune case.
    fn trait_depuis_le_papier(&mut self, layout: &BoardLayout, pos: Pos2, p: Pos2) -> bool {
        let proche = layout.cards.iter().enumerate().min_by(|a, b| distance_rect(a.1.thumb, p).total_cmp(&distance_rect(b.1.thumb, p))).map(|(k, _)| k);
        let Some(k) = proche else {
            self.vignettes.press = Press::Dead;
            return false;
        };
        let c = layout.cards[k];
        self.vignettes.appui_bande = Some(pos);
        self.board_sel = vec![c.plan];
        self.board_notes.clear();
        let f = self.cur_frame.min(self.frame_count().saturating_sub(1));
        if !(c.panel == self.cur_panel && (c.frame..c.end).contains(&f)) {
            self.select_board(c.panel, c.frame);
        }
        self.vignettes.press = Press::Card;
        self.draw_has_focus();
        self.pastille_avant_trait();
        true
    }

    /// Le lâcher d'un appui tombé sur un numéro : sans mouvement, c'est un clic, le plan s'annule ou se rétablit, et le point de trait qu'il a pu
    /// laisser est retiré. Vrai si le film a changé.
    pub(crate) fn numero_au_lacher(&mut self, latest: Option<Pos2>, any_down: bool) -> bool {
        let Some(a) = self.vignettes.numero_attente else { return false };
        if latest.is_some_and(|p| p.distance(a.origine) > MOVE_THRESHOLD) {
            self.vignettes.numero_attente = None;
            return false;
        }
        if any_down {
            return false;
        }
        self.vignettes.numero_attente = None;
        if a.dessine {
            self.retirer_le_point_de_trait(a.base_len);
        }
        // (Maj : le plan pris ne se déplace plus, et le clic ne le choisit pas : le numéro prime.)
        self.vignettes.moving = None;
        self.vignettes.press = Press::Dead;
        self.plan_annuler_ou_retablir(a.plan)
    }

    /// Le point de trait d'un clic est retiré : les étapes reviennent à ce qu'elles étaient, rien à répartir.
    pub(crate) fn retirer_le_point_de_trait(&mut self, base_len: usize) {
        self.end_stroke();
        self.vignettes.fini = None;
        self.vignettes.suivi = None;
        self.vignettes.debord_garde = None;
        self.vignettes.debord_tenu = None;
        let mut nav = Nav { panel: self.cur_panel, layer: self.cur_layer };
        while self.history.undo_len() > base_len && self.history.undo_and_forget(&mut self.doc, &mut nav) {}
        self.cur_panel = nav.panel.min(self.doc.panels.len().saturating_sub(1));
        self.cur_layer = nav.layer;
        self.revalider_rangs();
        self.cur_frame = self.cur_frame.min(self.frame_count().saturating_sub(1));
        self.renderer.invalidate_all();
        self.thumbs.prune(&self.doc);
    }

    /// Les cases annulées, sous le dessin hors des cases (un trait qui les traverse reste visible) : l'image pâlie, le filet, la croix rouge.
    pub(crate) fn peindre_cases_annulees(&mut self, ui: &egui::Ui, painter: &egui::Painter, view: Rect, xf: Xf, layout: &BoardLayout, deadline: Instant) {
        if layout.annulees.is_empty() {
            return;
        }
        let ppp = ui.ctx().pixels_per_point();
        let clip = painter.with_clip_rect(view);
        for c in &layout.annulees {
            let thumb = xf.rect(c.thumb);
            if !thumb.intersects(view) {
                continue;
            }
            clip.rect_filled(thumb, 0.0, Color32::WHITE);
            if let Some(g) = self.doc.annules.plans.get(c.panel).cloned() {
                let width = crate::thumbs::width_for(thumb.width() * ppp);
                if let Some(t) = self.thumbs.texture_rangee_a(ui.ctx(), &self.doc, &g.panel, c.frame, c.id, width, deadline) {
                    clip.image(t, thumb, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::from_white_alpha(150));
                }
            }
            clip.rect_stroke(thumb, 0.0, Stroke::new(self.settings.vignettes.epaisseur_cadre, Color32::from_gray(154)), StrokeKind::Outside);
            // Les cadres d'un mouvement de caméra : leur place, en gris (le dessin de A seul est montré).
            if let Some(m) = c.mv.and_then(|m| layout.moves.get(m)) {
                for q in m.quads().iter().skip(1) {
                    clip.add(egui::Shape::closed_line(q.iter().map(|p| xf.to_screen(*p)).collect(), Stroke::new(1.0_f32, Color32::from_gray(190))));
                }
            }
            let large = (thumb.width() * 0.014).clamp(2.0, 5.0);
            let s = Stroke::new(large, CROIX);
            clip.line_segment([thumb.left_top(), thumb.right_bottom()], s);
            clip.line_segment([thumb.right_top(), thumb.left_bottom()], s);
        }
    }

    /// Les numéros des plans annulés (en rouge) : au-dessus de leur première case, où l'on clique pour rétablir. Maj tenue les montre même
    /// cachés.
    pub(crate) fn peindre_numeros_annules(&mut self, painter: &egui::Painter, view: Rect, xf: Xf, layout: &BoardLayout, numeros: &crate::numerotation::Numeros, marges: &mut crate::numerosmarge::Marges) {
        if !(self.settings.vignettes.numeros_visibles || self.vignettes.peau_maj) {
            return;
        }
        let clip = painter.with_clip_rect(view);
        let font0 = crate::vignettesprefs::taille_numero(xf.zoom, self.settings.vignettes.numeros_echelle());
        for c in layout.annulees.iter().filter(|c| c.starts_plan()) {
            let thumb = xf.rect(c.thumb);
            if !thumb.expand(40.0).intersects(view) {
                continue;
            }
            let texte = numeros.scene_label(&self.doc, c.scene);
            let g0 = clip.layout_no_wrap(texte.clone(), FontId::proportional(font0), CROIX);
            let marge0 = (font0 * 0.18).max(2.0);
            let mesure = |sc: f32| if font0 * sc < crate::numerosmarge::CORPS_MIN { egui::Vec2::ZERO } else { g0.size() * sc + vec2(marge0, marge0) * sc * 2.0 };
            if let Some((libre, sc)) = marges.choisir(thumb, thumb.top(), mesure) {
                let g = clip.layout_no_wrap(texte, FontId::proportional(font0 * sc), CROIX);
                if self.settings.vignettes.fond_numeros {
                    clip.rect_filled(libre, marge0 * sc, Color32::from_white_alpha(235));
                    clip.rect_stroke(libre, marge0 * sc, Stroke::new(1.0_f32, CROIX), StrokeKind::Inside);
                }
                clip.galley(libre.min + vec2(marge0, marge0) * sc, g, CROIX);
                self.vignettes.numeros_zones.push((libre.expand(2.0), c.plan));
            }
        }
    }

    /// Le menu du clic droit d'une case annulée.
    pub(crate) fn annule_menu(&mut self, ui: &mut egui::Ui, plan: u64) {
        if ui.button("Rétablir ce plan").clicked() {
            ui.close_menu();
            self.plan_annuler_ou_retablir(plan);
        }
    }

    /// Un film qui a des plans annulés ne se partage pas : il faut d'abord les rétablir. Vrai s'il faut refuser.
    pub(crate) fn partage_refuse_annules(&mut self) -> bool {
        if self.doc.annules.plans.is_empty() {
            return false;
        }
        self.status = "Ce film a des plans annulés, un film partagé ne les connaît pas : rétablis-les d'abord".into();
        true
    }

    /// Tout est annulé : le Storyboard montre un état vide clair (un seul texte, un bouton vers la planche), et rien ne s'y dessine.
    pub(crate) fn etat_film_vide(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        self.canvas_rect = rect;
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_gray(52));
        let c = rect.center();
        painter.text(c - vec2(0.0, 26.0), egui::Align2::CENTER_CENTER, TEXTE_FILM_VIDE, FontId::proportional(18.0), Color32::from_gray(230));
        painter.text(c, egui::Align2::CENTER_CENTER, "Rétablis un plan dans les Vignettes : un clic sur son numéro.", FontId::proportional(14.0), Color32::from_gray(190));
        let bouton = Rect::from_center_size(c + vec2(0.0, 36.0), vec2(180.0, 28.0));
        if ui.put(bouton, egui::Button::new("Aller aux Vignettes")).clicked() {
            self.set_espace(crate::workspace::Espace::Vignettes);
        }
        let _ = response;
    }
}

/// Le titre de l'état vide du Storyboard.
pub(crate) const TEXTE_FILM_VIDE: &str = "Tous les plans sont annulés.";
/// Ce qu'un export répond alors.
pub(crate) const REFUS_EXPORT: &str = "Tous les plans sont annulés, rien à exporter ni à envoyer : rétablis un plan dans les Vignettes, un clic sur son numéro";
