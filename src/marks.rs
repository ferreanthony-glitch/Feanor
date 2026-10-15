//! Marques de couleur (13d) : les « marques d'image » de TVPaint, posées sur les dessins d'un
//! calque (`Layer::color_marks`) et montrées par un trait de couleur en haut de leurs blocs.
//! Dans Pelure, « marque d'image » désigne déjà les images de storyboard d'un plan (13a) :
//! celles-ci s'appellent donc « marques de couleur ».
//!
//! Poser / retirer sur l'instance courante, sur toutes les instances du calque ou sur celles
//! qui n'en ont pas ; retirer celles du calque ou du plan ; aller à la précédente / suivante
//! (toutes, ou de la couleur choisie), avec « Répéter » (on revient à la première) et
//! « Continuer sur le calque suivant ». Une marque tient à un dessin : une instance vide n'en
//! porte pas.

use eframe::egui::{self, Color32, RichText};

use crate::app::App;
use crate::document::Layer;

/// Couleurs des marques (celles des groupes de calques, sans « Défaut »).
pub(crate) const COLORS: [(&str, [u8; 3]); 12] = [
    ("Bleu foncé", [38, 64, 150]),
    ("Vert foncé", [30, 110, 50]),
    ("Cyan foncé", [20, 110, 125]),
    ("Rouge foncé", [140, 30, 35]),
    ("Violet", [110, 55, 160]),
    ("Brun", [120, 80, 40]),
    ("Bleu", [60, 125, 235]),
    ("Vert", [70, 190, 80]),
    ("Cyan", [60, 200, 215]),
    ("Rouge", [225, 60, 60]),
    ("Magenta", [215, 70, 195]),
    ("Orange", [245, 140, 40]),
];

pub(crate) fn color(k: u8) -> Color32 {
    let [r, g, b] = COLORS.get(k as usize).map_or([200, 200, 200], |c| c.1);
    Color32::from_rgb(r, g, b)
}

/// Débuts des instances marquées du calque (de la couleur `only`, ou de toutes), dans l'ordre.
pub(crate) fn marked_starts(layer: &Layer, only: Option<u8>) -> Vec<usize> {
    let mut out = Vec::new();
    for (f, v) in layer.frames.iter().enumerate() {
        let start = f == 0 || layer.frames[f - 1] != *v;
        let Some(id) = v else { continue };
        let ok = layer.color_marks.get(id).is_some_and(|k| only.is_none_or(|o| o == *k));
        if start && ok {
            out.push(f);
        }
    }
    out
}

/// Marque suivante (`forward`) ou précédente après / avant l'image `f` ; avec `repeat`, on
/// repart de l'autre bout.
pub(crate) fn step_mark(starts: &[usize], f: usize, forward: bool, repeat: bool) -> Option<usize> {
    let found = if forward { starts.iter().copied().find(|&s| s > f) } else { starts.iter().rev().copied().find(|&s| s < f) };
    found.or_else(|| {
        if !repeat {
            return None;
        }
        if forward { starts.first().copied() } else { starts.last().copied() }
    })
}

impl App {
    /// Change les marques du calque courant (une étape d'annulation).
    fn change_marks(&mut self, change: impl FnOnce(&mut Layer) -> usize, what: &str) {
        // Pas `refuse_on_folder` : une marque n'est qu'un repère, permise sous le storyboard
        // verrouillé (13e).
        if self.layer().folder {
            self.status = crate::app::FOLDER_NOT_DRAWABLE.into();
            return;
        }
        self.record_step();
        let li = self.cur_layer;
        let n = change(&mut self.layers_mut()[li]);
        self.status = format!("{what} ({n}).");
    }

    /// Marque de la couleur choisie sur l'instance courante : posée, ou retirée si c'était
    /// déjà celle-là.
    pub(crate) fn toggle_color_mark(&mut self) {
        let Some(id) = self.cel_here() else {
            self.status = "Pas de dessin à cette image (une instance vide ne porte pas de marque).".into();
            return;
        };
        let k = self.mark_color;
        self.change_marks(
            move |l| {
                if l.color_marks.get(&id) == Some(&k) {
                    l.color_marks.remove(&id);
                } else {
                    l.color_marks.insert(id, k);
                }
                1
            },
            "Marque de couleur posée ou retirée",
        );
    }

    /// Retire la marque de l'instance courante.
    pub(crate) fn remove_color_mark(&mut self) {
        let Some(id) = self.cel_here() else { return };
        self.change_marks(move |l| usize::from(l.color_marks.remove(&id).is_some()), "Marque retirée");
    }

    /// Marque de la couleur choisie sur toutes les instances du calque (`unmarked_only` :
    /// seulement celles qui n'en ont pas).
    pub(crate) fn mark_all_instances(&mut self, unmarked_only: bool) {
        let k = self.mark_color;
        self.change_marks(
            move |l| {
                let ids = crate::images::drawings_in_order(&l.frames);
                let mut n = 0;
                for id in ids {
                    if !unmarked_only || !l.color_marks.contains_key(&id) {
                        l.color_marks.insert(id, k);
                        n += 1;
                    }
                }
                n
            },
            "Instances marquées",
        );
    }

    /// Retire les marques du calque courant, ou (`plan`) de tous les calques du plan.
    pub(crate) fn clear_color_marks(&mut self, plan: bool) {
        if !plan {
            self.change_marks(
                |l| {
                    let n = l.color_marks.len();
                    l.color_marks.clear();
                    n
                },
                "Marques du calque retirées",
            );
            return;
        }
        self.record_step();
        let mut n = 0;
        for l in self.layers_mut() {
            n += l.color_marks.len();
            l.color_marks.clear();
        }
        self.status = format!("Marques du plan retirées ({n}).");
    }

    /// Va à la marque suivante (`forward`) ou précédente du calque courant (`by_color` : de la
    /// couleur choisie seulement) ; au bout, « Répéter » repart de l'autre bout et « Continuer
    /// sur le calque suivant » passe aux calques d'en dessous dans la timeline.
    pub(crate) fn goto_color_mark(&mut self, forward: bool, by_color: bool) {
        let only = by_color.then_some(self.mark_color);
        let starts = marked_starts(self.layer(), only);
        let repeat = self.mark_repeat && !self.mark_next_layer;
        if let Some(f) = step_mark(&starts, self.cur_frame, forward, repeat) {
            self.goto_frame(f);
            return;
        }
        if self.mark_next_layer {
            let below: Vec<usize> = if forward { (0..self.cur_layer).rev().collect() } else { (self.cur_layer + 1..self.layers().len()).collect() };
            for li in below {
                let s = marked_starts(&self.layers()[li], only);
                let f = if forward { s.first() } else { s.last() };
                if let Some(&f) = f {
                    self.select_layer(li);
                    self.goto_frame(f);
                    return;
                }
            }
        }
        self.status = "Pas d'autre marque.".into();
    }

    /// Sous-menu des marques de couleur (menu Repères).
    pub(crate) fn color_marks_menu(&mut self, ui: &mut egui::Ui) {
        let used: Vec<u8> = self.layers().iter().flat_map(|l| l.color_marks.values().copied()).collect();
        ui.checkbox(&mut self.mark_used_only, "Seulement les couleurs utilisées");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            for (k, (name, _)) in COLORS.iter().enumerate() {
                let k = k as u8;
                if self.mark_used_only && !used.contains(&k) && k != self.mark_color {
                    continue;
                }
                let chosen = self.mark_color == k;
                let text = RichText::new(if chosen { "■" } else { "□" }).color(color(k)).size(18.0);
                if ui.add(egui::Button::new(text).frame(false)).on_hover_text(*name).clicked() {
                    self.mark_color = k;
                }
            }
        });
        let name = COLORS[self.mark_color as usize % COLORS.len()].0;
        ui.label(RichText::new(format!("Couleur choisie : {name}")).weak());
        self.action_item(ui, "Poser / retirer sur l'instance courante", "marque_couleur.basculer");
        self.action_item(ui, "Retirer la marque de l'instance courante", "marque_couleur.retirer");
        self.action_item(ui, "Marquer toutes les instances du calque", "marque_couleur.toutes");
        self.action_item(ui, "Marquer les instances non marquées", "marque_couleur.non_marquees");
        self.action_item(ui, "Retirer les marques du calque", "marque_couleur.effacer_calque");
        self.action_item(ui, "Retirer les marques du plan", "marque_couleur.effacer_plan");
        ui.separator();
        self.action_item(ui, "Marque précédente", "marque_couleur.precedente");
        self.action_item(ui, "Marque suivante", "marque_couleur.suivante");
        self.action_item(ui, "Marque précédente de cette couleur", "marque_couleur.precedente_couleur");
        self.action_item(ui, "Marque suivante de cette couleur", "marque_couleur.suivante_couleur");
        ui.checkbox(&mut self.mark_repeat, "Répéter (revenir à la première au bout)");
        ui.checkbox(&mut self.mark_next_layer, "Continuer sur le calque suivant");
    }
}
