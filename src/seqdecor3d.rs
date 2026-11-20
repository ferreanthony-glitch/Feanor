//! Partie 201 (`docs/INTERFACE.md` partie 201) : **les objets du décor d'une séquence à l'autre**, dans l'onglet 3D.
//!
//! - **Le sélecteur de séquence**, en tête de « Scène 3D » (« Séquence 3 · La cuisine ▾ ») : sa liste ouverte, un clic va au premier
//!   plan d'une séquence, comme le Storyboard.
//! - **Le menu d'une ligne d'objet** (clic droit ; sur un objet choisi parmi d'autres, tous les choisis) : « Copier vers la séquence ▸ »
//!   et « Déplacer vers la séquence ▸ », par le chemin de la partie 190 (`decor3d_vers_sequence` : une étape d'annulation ; déplacer,
//!   c'est copier puis enlever, dans la même étape).
//! - **Glisser des lignes d'objets** sur une séquence de la liste ouverte du sélecteur, ou sur une ligne de séquence du panneau Scène du
//!   Storyboard (`sceneui.rs`, racine Film) : lâcher déplace, Alt copie ; la séquence sous le pointeur s'éclaire.
//!
//! Ce qui suit les objets : leurs clés (partie 200), leurs couleurs (199), comme une copie ; « Caché dans le plan » vise des plans de
//! leur séquence : il part vers une autre séquence (`Document::poser_copies_3d`). Les lampes sont au décor, pas aux objets : elles
//! restent.

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, RichText, Sense, Stroke};
use egui_phosphor::regular as ph;

use crate::app::App;

/// Ce qu'on glisse depuis « Scène 3D » : des objets du décor de la séquence `seq`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlisseDecor3d {
    pub seq: u64,
    pub ids: Vec<u64>,
}

/// Hauteur d'une ligne de la liste des séquences.
const LIGNE_SEQ: f32 = 22.0;

// Ce que le sélecteur a peint (essais) : les lignes de la liste, (séquence, rectangle).
#[cfg(test)]
thread_local! {
    pub(crate) static SEQUENCES_LISTE: std::cell::RefCell<Vec<(u64, Rect)>> = const { std::cell::RefCell::new(Vec::new()) };
}

impl App {
    /// Le nom d'une séquence dans le sélecteur : « Séquence 3 · La cuisine » (son nom s'il en a un), « Séquence 3 ».
    pub(crate) fn o3_nom_de_sequence(&self, seq: u64) -> String {
        let code = self.doc.sequence_code(seq);
        let nom = self.doc.sequences.iter().find(|s| s.id == seq).map(|s| s.name.trim().to_string()).unwrap_or_default();
        if nom.is_empty() || nom == code {
            format!("Séquence {code}")
        } else {
            format!("Séquence {code} · {nom}")
        }
    }

    /// Le premier plan de la séquence `seq`.
    pub(crate) fn premier_plan_de(&self, seq: u64) -> Option<usize> {
        (0..self.doc.panels.len()).find(|&i| self.doc.sequence_of(i) == seq)
    }

    /// Va à la séquence `seq` : son premier plan (comme un clic sur sa ligne dans le Storyboard).
    pub(crate) fn o3_aller_a_la_sequence(&mut self, seq: u64) {
        if let Some(pi) = self.premier_plan_de(seq) {
            if self.doc.sequence_of(self.cur_panel) != seq {
                self.select_panel(pi);
            }
            self.status = format!("{} : son premier plan.", self.o3_nom_de_sequence(seq));
        }
    }

    /// Les objets que vise un geste sur la ligne de l'objet `id` : tous les objets choisis s'il en est, sinon lui seul.
    pub(crate) fn o3_objets_du_geste(&self, id: u64) -> Vec<u64> {
        let choisis = self.e3_objets_choisis();
        if choisis.contains(&id) {
            choisis
        } else {
            vec![id]
        }
    }

    /// Lâcher des objets glissés sur la séquence `vers` : les déplacer, ou (Alt) les copier ; une étape d'annulation.
    pub(crate) fn o3_lacher_les_objets(&mut self, g: &GlisseDecor3d, vers: u64, copier: bool) {
        if g.seq == vers {
            return;
        }
        // (Le décor glissé est celui de la séquence courante : le chemin de la partie 190 le lit là.)
        if self.doc.sequence_of(self.cur_panel) != g.seq {
            return;
        }
        self.decor3d_vers_sequence(&g.ids, vers, !copier);
    }

    /// La tête de « Scène 3D » (partie 201) : le sélecteur de séquence, et sa liste ouverte (aller à une séquence ; y lâcher des objets).
    pub(crate) fn o3_selecteur_de_sequence(&mut self, ui: &mut egui::Ui) {
        if self.doc.panels.is_empty() {
            return;
        }
        let seq = self.doc.sequence_of(self.cur_panel);
        // (Un glisser d'objets ouvre la liste : on y voit où les lâcher.)
        let glisse = egui::DragAndDrop::has_payload_of_type::<GlisseDecor3d>(ui.ctx());
        let ouverte = self.o3.sequences_ouvertes || glisse;
        let texte = format!("{} {}", self.o3_nom_de_sequence(seq), if ouverte { ph::CARET_UP } else { ph::CARET_DOWN });
        let r = ui.add(egui::Button::new(RichText::new(texte).strong()).frame(false));
        if crate::bulles::bulle(r, "Séquence de l'onglet 3D", "Ouvre la liste des séquences du film : un clic va au premier plan de l'une d'elles. Glisser des objets du décor sur une séquence de la liste les y déplace (Alt : les y copie).", "").clicked() {
            self.o3.sequences_ouvertes = !self.o3.sequences_ouvertes;
        }
        #[cfg(test)]
        SEQUENCES_LISTE.with(|l| l.borrow_mut().clear());
        if !ouverte {
            return;
        }
        let mut aller = None;
        let mut lacher = None;
        for q in self.doc.sequence_order() {
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), LIGNE_SEQ), Sense::click());
            #[cfg(test)]
            SEQUENCES_LISTE.with(|l| l.borrow_mut().push((q, rect)));
            let p = ui.painter_at(rect);
            let survol = resp.dnd_hover_payload::<GlisseDecor3d>().filter(|g| g.seq != q);
            if survol.is_some() {
                p.rect_filled(rect.shrink2(vec2(2.0, 1.0)), 3.0, crate::theme::surface_active(crate::theme::accent(), 0.55));
                p.rect_stroke(rect.shrink2(vec2(2.0, 1.0)), 3.0, Stroke::new(1.5_f32, crate::theme::accent()), egui::StrokeKind::Inside);
            } else if q == seq {
                p.rect_filled(rect.shrink2(vec2(2.0, 1.0)), 3.0, crate::theme::surface_active(crate::theme::accent(), 0.3));
            } else if resp.hovered() {
                p.rect_filled(rect.shrink2(vec2(2.0, 1.0)), 3.0, ui.visuals().widgets.hovered.weak_bg_fill);
            }
            let n = self.doc.decors3d.par_sequence.get(&q).map_or(0, |d| d.objets.len());
            let nom = self.o3_nom_de_sequence(q);
            p.text(pos2(rect.left() + 22.0, rect.center().y), Align2::LEFT_CENTER, ph::FILM_SLATE, FontId::proportional(12.0), ui.visuals().weak_text_color());
            let t = p.text(pos2(rect.left() + 40.0, rect.center().y), Align2::LEFT_CENTER, &nom, FontId::proportional(12.0), if q == seq { ui.visuals().strong_text_color() } else { ui.visuals().text_color() });
            if n > 0 {
                p.text(pos2(t.right() + 6.0, rect.center().y), Align2::LEFT_CENTER, format!("{n} objet{} 3D", if n > 1 { "s" } else { "" }), FontId::proportional(10.5), ui.visuals().weak_text_color());
            }
            if let Some(g) = survol {
                let alt = ui.input(|i| i.modifiers.alt);
                p.text(pos2(rect.right() - 6.0, rect.center().y), Align2::RIGHT_CENTER, if alt { format!("copier {} ici", g.ids.len()) } else { format!("déplacer {} ici", g.ids.len()) }, FontId::proportional(10.5), crate::theme::accent());
            }
            if let Some(g) = resp.dnd_release_payload::<GlisseDecor3d>() {
                lacher = Some(((*g).clone(), q, ui.input(|i| i.modifiers.alt)));
            } else if crate::bulles::bulle(resp, &format!("Aller à la {}", nom.to_lowercase()), "Son premier plan, comme dans le Storyboard.", "").clicked() {
                aller = Some(q);
            }
        }
        if let Some((g, q, alt)) = lacher {
            self.o3_lacher_les_objets(&g, q, alt);
            self.o3.sequences_ouvertes = false;
        } else if let Some(q) = aller {
            self.o3.sequences_ouvertes = false;
            self.o3_aller_a_la_sequence(q);
        }
        ui.separator();
    }

    /// Le menu du clic droit d'une ligne d'objet de « Scène 3D » : copier ou déplacer l'objet (ou les objets choisis) vers une autre
    /// séquence.
    pub(crate) fn o3_menu_d_objet(&mut self, resp: &egui::Response, id: u64) {
        let seq = self.doc.sequence_of(self.cur_panel);
        let autres: Vec<(u64, String)> = self.doc.sequence_order().into_iter().filter(|&q| q != seq).map(|q| (q, self.o3_nom_de_sequence(q))).collect();
        let ids = self.o3_objets_du_geste(id);
        let mut geste = None;
        resp.context_menu(|ui| {
            let n = ids.len();
            ui.label(RichText::new(if n > 1 { format!("{n} objets du décor") } else { self.e3_objet(id).map_or_else(String::new, |o| o.nom.clone()) }).small().weak());
            for (deplacer, titre) in [(false, "Copier vers la séquence"), (true, "Déplacer vers la séquence")] {
                ui.add_enabled_ui(!autres.is_empty(), |ui| {
                    ui.menu_button(titre, |ui| {
                        for (q, nom) in &autres {
                            if ui.button(nom).clicked() {
                                geste = Some((*q, deplacer));
                                ui.close_menu();
                            }
                        }
                    })
                    .response
                    .on_disabled_hover_text("Le film n'a pas d'autre séquence.");
                });
            }
        });
        if let Some((q, deplacer)) = geste {
            self.decor3d_vers_sequence(&ids, q, deplacer);
        }
    }

    /// Une ligne d'objet de « Scène 3D » qu'on commence à glisser : elle porte ses objets (ou les objets choisis).
    pub(crate) fn o3_glisser_une_ligne(&mut self, resp: &egui::Response, id: u64) {
        if resp.drag_started() {
            let g = GlisseDecor3d { seq: self.doc.sequence_of(self.cur_panel), ids: self.o3_objets_du_geste(id) };
            resp.dnd_set_drag_payload(g);
        }
        if let Some(g) = egui::DragAndDrop::payload::<GlisseDecor3d>(&resp.ctx) {
            if resp.dragged() {
                if let Some(p) = resp.ctx.pointer_latest_pos() {
                    let nom = if g.ids.len() > 1 { format!("{} objets", g.ids.len()) } else { self.e3_objet(g.ids[0]).map_or_else(String::new, |o| o.nom.clone()) };
                    let painter = resp.ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("glisse_decor3d")));
                    let r = Rect::from_min_size(p + vec2(12.0, 6.0), vec2(8.0 * nom.chars().count() as f32 + 28.0, 20.0));
                    painter.rect_filled(r, 4.0, Color32::from_black_alpha(190));
                    painter.text(pos2(r.left() + 8.0, r.center().y), Align2::LEFT_CENTER, format!("{} {nom}", ph::CUBE), FontId::proportional(11.5), Color32::WHITE);
                }
            }
        }
    }

    /// Le panneau Scène du Storyboard (racine Film) : une ligne de séquence où l'on peut lâcher des objets du décor glissés depuis
    /// « Scène 3D ». Vrai si quelque chose y a été lâché (le clic de la ligne n'est alors pas un clic).
    pub(crate) fn scene_lacher_sur_sequence(&mut self, ui: &egui::Ui, resp: &egui::Response, rect: Rect, seq: u64) -> bool {
        if let Some(g) = resp.dnd_hover_payload::<GlisseDecor3d>().filter(|g| g.seq != seq) {
            let p = ui.painter();
            p.rect_stroke(rect.shrink(1.0), 3.0, Stroke::new(1.5_f32, crate::theme::accent()), egui::StrokeKind::Inside);
            let alt = ui.input(|i| i.modifiers.alt);
            p.text(pos2(rect.right() - 6.0, rect.center().y), Align2::RIGHT_CENTER, if alt { format!("copier {} ici", g.ids.len()) } else { format!("déplacer {} ici", g.ids.len()) }, FontId::proportional(10.5), crate::theme::accent());
        }
        if let Some(g) = resp.dnd_release_payload::<GlisseDecor3d>() {
            let alt = ui.input(|i| i.modifiers.alt);
            self.o3_lacher_les_objets(&g, seq, alt);
            return true;
        }
        false
    }
}
