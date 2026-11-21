//! Partie 199 : les couleurs d'identification du décor 3D et ses aspects, dans l'interface (`couleurs3d.rs`, `docs/INTERFACE.md`
//! partie 199).
//!
//! - **La palette** (`pastilles`) : « Aucune » puis les dix couleurs, en pastilles rondes ; leurs noms en bulle. Elle est en haut de
//!   Propriétés > Objet (`couleur_d_identification_ui`) et sous la puce de chaque objet du sommaire « Scène 3D » (`puce_de_couleur`).
//! - **Une étape d'annulation** par couleur posée (« Couleur d'identification »), par œil et par cadenas d'un objet
//!   (`decor3d_changer_objet`) ; les objets copiés ou dupliqués (parties 190, 191) gardent leur couleur : c'est un champ de l'objet.
//! - **L'aspect de la vue** (`aspect_de_la_vue_ui`, barre de la vue de l'onglet 3D) et **celui des plans** (« Dans les plans »,
//!   Propriétés > Décor, et `decor_dans_les_plans_ui` dans la barre au-dessus du dessin du Storyboard).
//!
//! Sans latence : une couleur change la couleur de base que la carte graphique écrit à chaque passe (la vue) ; dans les images des
//! plans, l'empreinte du décor (`decor3d::contenu`) change et l'image se redessine une fois par le chemin du décor, les maillages
//! restent à la carte.

use eframe::egui::{self, pos2, vec2, Color32, Rect, Sense, Stroke};

use crate::app::App;
use crate::couleurs3d::{self, Rendu};

/// Le diamètre d'une pastille de la palette.
pub(crate) const PASTILLE: f32 = 16.0;

impl App {
    /// Change l'objet `id` du décor de la séquence du plan courant par `f`, en une étape d'annulation nommée `nom` (le décor passe
    /// par `decor3d_appliquer` : l'aimant, les cartes aimantées).
    pub(crate) fn decor3d_changer_objet(&mut self, id: u64, nom: &str, f: impl Fn(&mut crate::decor3d::Objet3d)) {
        self.decor3d_changer_objets(&[id], nom, f);
    }

    /// `decor3d_changer_objet`, pour plusieurs objets à la fois (une seule étape).
    pub(crate) fn decor3d_changer_objets(&mut self, ids: &[u64], nom: &str, f: impl Fn(&mut crate::decor3d::Objet3d)) {
        let seq = self.doc.env_cle(self.cur_panel);
        let Some(mut d) = self.doc.decor_du_plan(self.cur_panel).cloned() else { return };
        let mut un = false;
        for id in ids {
            if let Some(o) = d.objet_mut(*id) {
                f(o);
                un = true;
            }
        }
        if !un {
            return;
        }
        self.decor3d_retouche_finir();
        let snap = self.snapshot();
        self.decor3d_appliquer(seq, d);
        let avant = self.history.context.replace(nom.to_string());
        self.commit_structure(snap);
        self.history.context = avant;
    }

    /// La couleur d'identification `k` (rien : aucune) pour les objets `ids` du décor : une étape « Couleur d'identification ».
    pub(crate) fn couleur_d_identification(&mut self, ids: &[u64], k: Option<u8>) {
        let k = k.filter(|k| couleurs3d::couleur(*k).is_some());
        if ids.iter().all(|id| self.e3_objet(*id).is_some_and(|o| o.couleur == k)) {
            return;
        }
        self.decor3d_changer_objets(ids, "Couleur d'identification", |o| o.couleur = k);
        let n = ids.len();
        self.status = format!("Couleur d'identification : {}{}.", couleurs3d::nom(k), if n > 1 { format!(" ({n} objets)") } else { String::new() });
    }

    /// La palette : « Aucune » puis les dix couleurs, en pastilles ; celle de `actuelle` entourée. Rend le choix cliqué.
    pub(crate) fn pastilles(&self, ui: &mut egui::Ui, actuelle: Option<u8>) -> Option<Option<u8>> {
        let mut choix = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            for k in std::iter::once(None).chain((0..couleurs3d::PALETTE.len() as u8).map(Some)) {
                let (rect, resp) = ui.allocate_exact_size(vec2(PASTILLE, PASTILLE), Sense::click());
                let p = ui.painter();
                let c = rect.center();
                match k.and_then(couleurs3d::couleur) {
                    Some(rgb) => {
                        p.circle_filled(c, PASTILLE * 0.5 - 1.5, couleurs3d::color32(rgb));
                    }
                    None => {
                        // « Aucune » : un rond vide, barré.
                        p.circle_stroke(c, PASTILLE * 0.5 - 2.0, Stroke::new(1.2_f32, ui.visuals().weak_text_color()));
                        p.line_segment([c + vec2(-4.0, 4.0), c + vec2(4.0, -4.0)], Stroke::new(1.2_f32, ui.visuals().weak_text_color()));
                    }
                }
                if k == actuelle {
                    p.circle_stroke(c, PASTILLE * 0.5, Stroke::new(1.5_f32, ui.visuals().strong_text_color()));
                } else if resp.hovered() {
                    p.circle_stroke(c, PASTILLE * 0.5, Stroke::new(1.0_f32, ui.visuals().text_color()));
                }
                let nom = format!("Couleur : {}", couleurs3d::nom(k));
                if crate::bulles::bulle(resp, &nom, "La couleur d'identification de l'objet : l'aspect Couleurs le montre ainsi, dans la vue et, si son décor le dit, dans les plans.", "").clicked() {
                    choix = Some(k);
                }
            }
        });
        choix
    }

    /// Propriétés > Objet, en haut : « Couleur d'identification » et la palette, pour les objets `ids` choisis.
    pub(crate) fn couleur_d_identification_ui(&mut self, ui: &mut egui::Ui, ids: &[u64]) {
        let actuelle = ids.first().and_then(|id| self.e3_objet(*id)).and_then(|o| o.couleur).filter(|_| {
            let k = ids.first().and_then(|id| self.e3_objet(*id)).and_then(|o| o.couleur);
            ids.iter().all(|id| self.e3_objet(*id).is_some_and(|o| o.couleur == k))
        });
        ui.label(egui::RichText::new(format!("Couleur d'identification · {}", couleurs3d::nom(actuelle))).strong());
        if let Some(k) = self.pastilles(ui, actuelle) {
            self.couleur_d_identification(ids, k);
        }
        ui.add_space(4.0);
    }

    /// La puce de couleur d'un objet dans le sommaire « Scène 3D », dans `rect` : un clic ouvre la palette sous elle.
    pub(crate) fn puce_de_couleur(&mut self, ui: &mut egui::Ui, rect: Rect, id: u64) {
        let k = self.e3_objet(id).and_then(|o| o.couleur);
        let resp = ui.interact(rect, ui.id().with(("puce_couleur", id)), Sense::click());
        let p = ui.painter();
        match k.and_then(couleurs3d::couleur) {
            Some(rgb) => {
                p.rect_filled(rect.shrink(2.0), 3.0, couleurs3d::color32(rgb));
            }
            None => {
                p.rect_stroke(rect.shrink(2.5), 3.0, Stroke::new(1.0_f32, ui.visuals().weak_text_color().gamma_multiply(0.7)), egui::StrokeKind::Inside);
            }
        }
        let resp = crate::bulles::bulle(resp, "Couleur d'identification", "Un clic : la palette (aucune, ou l'une des dix couleurs). L'aspect Couleurs montre l'objet dans cette couleur.", "");
        let popup = ui.make_persistent_id(("palette_objet", id));
        if resp.clicked() {
            ui.memory_mut(|m| m.toggle_popup(popup));
        }
        let choix = egui::popup_below_widget(ui, popup, &resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
            ui.set_min_width(6.0 * (PASTILLE + 4.0));
            ui.label(egui::RichText::new("Couleur d'identification").small().strong());
            self.pastilles(ui, k)
        })
        .flatten();
        if let Some(c) = choix {
            ui.memory_mut(|m| m.close_popup());
            self.couleur_d_identification(&[id], c);
        }
    }

    /// Trois boutons, un par aspect, celui de `actuel` allumé ; `prefixe` : le début du nom de leurs bulles. Rend le choix cliqué.
    pub(crate) fn segment_d_aspect(ui: &mut egui::Ui, actuel: Rendu, prefixe: &str) -> Option<Rendu> {
        let mut choix = None;
        for r in Rendu::TOUS {
            let on = actuel == r;
            let resp = ui.selectable_label(on, crate::bulles::encre_etat(egui::RichText::new(r.nom()), on));
            if crate::bulles::bulle(resp, &format!("{prefixe} : {}", r.nom()), r.aide(), "").clicked() && !on {
                choix = Some(r);
            }
        }
        choix
    }

    /// L'aspect de la vue (barre de la vue de l'onglet 3D) : un réglage du poste, pour la vue de l'onglet 3D, la vue Espace 3D et la vue à
    /// côté ; les cartes ne changent jamais (ce sont des dessins).
    pub(crate) fn aspect_de_la_vue_ui(&mut self, ui: &mut egui::Ui) {
        if let Some(r) = Self::segment_d_aspect(ui, self.settings.aspect_vue_3d, "Aspect") {
            self.settings.aspect_vue_3d = r;
            let _ = self.settings.save();
            self.status = format!("Aspect de la vue : {}.", r.nom());
        }
    }

    /// « Dans les plans » : l'aspect `r` du décor de la séquence du plan courant dans les images des plans (une étape d'annulation).
    pub(crate) fn decor3d_rendu_dans_les_plans(&mut self, r: Rendu) {
        let seq = self.doc.env_cle(self.cur_panel);
        let Some(mut d) = self.doc.decor_du_plan(self.cur_panel).cloned() else { return };
        if d.aspect.rendu == r {
            return;
        }
        d.aspect.rendu = r;
        self.decor3d_retouche_finir();
        let snap = self.snapshot();
        self.decor3d_appliquer(seq, d);
        let avant = self.history.context.replace("Aspect du décor dans les plans".into());
        self.commit_structure(snap);
        self.history.context = avant;
        self.status = format!("Décor de la séquence, dans les plans : {}.", r.nom());
    }

    /// La barre au-dessus du dessin (hors de l'onglet 3D) : « Décor » et ses trois aspects dans les plans, quand la séquence du plan
    /// courant a un décor 3D.
    pub(crate) fn decor_dans_les_plans_ui(&mut self, ui: &mut egui::Ui) {
        let Some(actuel) = self.doc.decor_du_plan(self.cur_panel).map(|d| d.aspect.rendu) else { return };
        ui.label(egui::RichText::new("Décor").small().weak());
        if let Some(r) = Self::segment_d_aspect(ui, actuel, "Décor dans les plans") {
            self.decor3d_rendu_dans_les_plans(r);
        }
    }
}

/// Un aperçu de la palette dans `rect` (pour les essais et les captures) : ses couleurs côte à côte.
#[allow(dead_code)]
pub(crate) fn bande_de_la_palette(painter: &egui::Painter, rect: Rect) {
    let w = rect.width() / couleurs3d::PALETTE.len() as f32;
    for (k, c) in couleurs3d::PALETTE.iter().enumerate() {
        painter.rect_filled(Rect::from_min_size(pos2(rect.left() + k as f32 * w, rect.top()), vec2(w, rect.height())), 0.0, Color32::from_rgb(c[0], c[1], c[2]));
    }
}
