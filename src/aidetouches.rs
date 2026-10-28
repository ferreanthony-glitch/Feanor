//! L'aide-mémoire des raccourcis (passe « simplicité », 2026-10-21) : tant qu'on tient sa touche (`aide.aide_memoire`, R
//! comme « raccourcis »), une carte légère se pose sur le dessin et montre les raccourcis de l'outil courant, de l'onglet
//! courant (Vignettes, Storyboard, Animation, Montage) et ceux qui valent partout. On relâche : elle s'en va. Rien ne se clique.
//!
//! Les touches sont lues **dans le fichier de raccourcis en vigueur** (`Keymap`) : un raccourci changé dans Préférences,
//! dans « Assigner un raccourci… » ou dans le fichier personnel se lit changé ici aussi, et une commande qui n'a plus de
//! touche n'a plus de ligne. Les listes ci-dessous ne choisissent que **quelles commandes** montrer, pas leurs touches. La
//! liste complète, avec recherche, est toujours sous F1.
//!
//! Sans latence : la carte est dessinée en une image d'écran avec des textes que le fichier de raccourcis connaît déjà.

use eframe::egui::{self, vec2, Align2, RichText};

use crate::app::{App, Tool};
use crate::keymap::Keymap;
use crate::theme;
use crate::workspace::Espace;

/// Une ligne de la carte : ce que fait la commande, et ses touches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ligne {
    pub nom: String,
    pub touches: String,
}

/// Une colonne de la carte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub titre: String,
    pub lignes: Vec<Ligne>,
}

/// La première ligne de la colonne de l'outil : sa propre touche.
pub const CHOISIR_L_OUTIL: &str = "Choisir cet outil";

/// Ce qui vaut partout : (action, ce qu'elle fait en peu de mots).
pub const PARTOUT: &[(&str, &str)] = &[
    ("edition.annuler", "Annuler"),
    ("edition.retablir", "Rétablir"),
    ("fichier.enregistrer", "Enregistrer"),
    ("vue.concentration", "Mode concentration"),
    ("vue.pan_libre", "Déplacer la vue (tenir)"),
    ("vue.ajuster", "Ajuster à la fenêtre"),
    ("aide.palette", "Chercher une commande"),
    ("aide.raccourcis", "Tous les raccourcis"),
];

/// Les commandes de l'onglet : (action, ce qu'elle fait en peu de mots).
pub fn commandes_de_l_onglet(espace: Espace) -> &'static [(&'static str, &'static str)] {
    match espace {
        Espace::Vignettes => &[
            ("panneau.nouveau", "Nouveau Plan"),
            ("panneau.precedent", "Plan précédent"),
            ("panneau.suivant", "Plan suivant"),
            ("vignettes.camera", "Caméra : tenir, puis glisser (Ctrl + Maj aussi)"),
            ("vignettes.annotation_stylo", "Stylo d'annotation"),
            ("vignettes.annotation_gomme", "Gomme d'annotation"),
            ("vignettes.annotation_lasso", "Lasso d'annotation"),
            ("vignettes.annotations_afficher", "Montrer ou cacher les annotations"),
            ("vignettes.marie_louise", "Marie-louise"),
            ("vue.zoom_avant", "Zoom avant"),
            ("vue.zoom_arriere", "Zoom arrière"),
        ],
        Espace::Storyboard => &[
            ("panneau.nouveau", "Nouveau Plan"),
            ("panneau.dupliquer", "Dupliquer le Plan"),
            ("panneau.couper", "Couper le Plan ici"),
            ("panneau.precedent", "Plan précédent"),
            ("panneau.suivant", "Plan suivant"),
            ("sequence.nouvelle", "Nouvelle séquence"),
            ("mode.basculer", "Animer le Plan"),
            ("lecture.basculer", "Lecture / pause"),
        ],
        Espace::Animation => &[
            ("lecture.basculer", "Lecture / pause"),
            ("image.precedente", "Image précédente"),
            ("image.suivante", "Image suivante"),
            ("cle.precedente", "Clé précédente"),
            ("cle.suivante", "Clé suivante"),
            ("dessin.nouveau", "Nouveau dessin"),
            ("exposition.allonger", "Tenir une image de plus"),
            ("vue.pelure", "Pelure d'oignon"),
            ("mode.basculer", "Retour au storyboard"),
        ],
        Espace::Montage => &[
            ("lecture.basculer", "Lecture / pause"),
            ("montage.debut", "Début du film"),
            ("montage.fin", "Fin du film"),
            ("panneau.couper", "Couper le Plan ici"),
            ("montage.aimanter", "Aimanter les bords"),
            ("bande.zoom_avant", "Zoom des Pistes +"),
            ("bande.zoom_arriere", "Zoom des Pistes -"),
            ("bande.tout_voir", "Tout voir"),
        ],
    }
}

/// Les commandes de l'outil courant : (action, ce qu'elle fait en peu de mots). L'outil lui-même vient en premier.
pub fn commandes_de_l_outil(tool: Tool) -> Vec<(&'static str, &'static str)> {
    let mut v: Vec<(&'static str, &'static str)> = vec![(tool.action(), CHOISIR_L_OUTIL)];
    let suite: &[(&str, &str)] = match tool {
        Tool::Brush | Tool::Pencil | Tool::Eraser => &[
            ("outil.taille_plus", "Plus gros"),
            ("outil.taille_moins", "Plus fin"),
            ("pinceau.derriere", "Peindre derrière"),
            ("pinceau.sur_aplat", "Peint sur l'aplat"),
            ("pinceau.fond", "Aplat sous le trait"),
            ("couleur.intervertir", "Intervertir les couleurs"),
        ],
        Tool::Fill => &[("couleur.intervertir", "Intervertir les couleurs"), ("couleur.defaut", "Noir et blanc")],
        Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Polyline => &[("outil.taille_plus", "Trait plus gros"), ("outil.taille_moins", "Trait plus fin"), ("couleur.intervertir", "Intervertir les couleurs")],
        Tool::Select | Tool::SelectColor => &[
            ("edition.tout_selectionner", "Tout sélectionner"),
            ("edition.tout_deselectionner", "Tout désélectionner"),
            ("edition.copier", "Copier"),
            ("edition.coller", "Coller"),
            ("edition.supprimer", "Supprimer"),
            ("selection.miroir_h", "Retourner de gauche à droite"),
            ("selection.remplir", "Remplir de la couleur"),
        ],
        Tool::Camera => &[("camera.cle_ici", "Clé de caméra ici"), ("camera.reinitialiser", "Réinitialiser la caméra")],
        Tool::Transform => &[("calque.reinit_transformation", "Annuler la transformation")],
        Tool::Grab | Tool::Pull => &[("outil.taille_plus", "Rayon plus grand"), ("outil.taille_moins", "Rayon plus petit")],
        Tool::Cutter | Tool::Picker | Tool::Pan | Tool::Text | Tool::Perspective => &[],
    };
    v.extend_from_slice(suite);
    v
}

/// Des gestes à la souris ou au stylet, qui ne passent pas par le fichier de raccourcis : (ce qu'ils font, le geste).
pub fn gestes_de_l_outil(tool: Tool) -> &'static [(&'static str, &'static str)] {
    match tool {
        Tool::Brush | Tool::Pencil | Tool::Eraser => &[("Pipette", "Alt + clic")],
        Tool::Text => &[("Valider le texte", "Ctrl + Entrée")],
        Tool::SelectColor => &[("Ajouter à la sélection", "Maj + clic")],
        _ => &[],
    }
}

/// Les lignes d'une liste de commandes : seulement celles qui ont une touche aujourd'hui.
pub fn lignes_de(keymap: &Keymap, commandes: &[(&str, &str)]) -> Vec<Ligne> {
    let mut vues: Vec<Ligne> = Vec::new();
    for (action, nom) in commandes {
        let touches = keymap.keys_text(action);
        if touches.is_empty() || vues.iter().any(|l| l.nom == *nom && l.touches == touches) {
            continue;
        }
        vues.push(Ligne { nom: (*nom).to_string(), touches });
    }
    vues
}

/// Les trois colonnes de la carte : l'outil courant, l'onglet courant, ce qui vaut partout. Une colonne sans ligne est omise.
pub fn sections(keymap: &Keymap, tool: Tool, espace: Espace) -> Vec<Section> {
    let mut outil = lignes_de(keymap, &commandes_de_l_outil(tool));
    // Un outil sans touche (la caméra, la pipette…) s'y trouve quand même, avec ce qu'il faut savoir.
    if !outil.iter().any(|l| l.nom == CHOISIR_L_OUTIL) {
        outil.insert(0, Ligne { nom: CHOISIR_L_OUTIL.to_string(), touches: "pas de touche".to_string() });
    }
    outil.extend(gestes_de_l_outil(tool).iter().map(|(nom, geste)| Ligne { nom: (*nom).to_string(), touches: (*geste).to_string() }));
    let candidates = [
        Section { titre: tool.label().to_string(), lignes: outil },
        Section { titre: espace.nom().to_string(), lignes: lignes_de(keymap, commandes_de_l_onglet(espace)) },
        Section { titre: "Partout".to_string(), lignes: lignes_de(keymap, PARTOUT) },
    ];
    candidates.into_iter().filter(|s| !s.lignes.is_empty()).collect()
}

impl App {
    /// Chaque image d'écran : la touche est-elle tenue ? Si oui, la carte est dessinée. Un test d'état par image, rien d'autre.
    pub(crate) fn aide_touches_tick(&mut self, ctx: &egui::Context) {
        let tenue = !self.welcome_shown() && !ctx.wants_keyboard_input() && !self.modal_open() && ctx.input(|i| self.keymap.held(i, "aide.aide_memoire"));
        self.confort.aide_touches = tenue;
        if tenue {
            self.aide_touches_ui(ctx);
        }
    }

    /// La carte, posée au milieu du dessin, que la souris traverse.
    fn aide_touches_ui(&self, ctx: &egui::Context) {
        let sections = sections(&self.keymap, self.tool, self.espace);
        let touche = self.keymap.label("aide.aide_memoire");
        let centre = self.canvas_rect.center();
        egui::Area::new(egui::Id::new("aide_touches")).order(egui::Order::Tooltip).interactable(false).pivot(Align2::CENTER_CENTER).fixed_pos(centre).show(ctx, |ui| {
            egui::Frame::new()
                .fill(theme::fenetre().gamma_multiply(0.94))
                .stroke(egui::Stroke::new(1.0_f32, theme::bord()))
                .corner_radius(10.0)
                .inner_margin(egui::Margin::same(14))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Raccourcis").strong().size(16.0).color(theme::texte_fort()));
                        ui.label(RichText::new(format!("{} · {}", self.tool.label(), self.espace.nom())).color(theme::texte_doux()));
                        if !touche.is_empty() {
                            ui.add_space(24.0);
                            ui.label(RichText::new(format!("{touche} relâchée : la carte s'en va")).small().weak());
                        }
                    });
                    ui.add_space(8.0);
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing = vec2(28.0, 0.0);
                        for (k, s) in sections.iter().enumerate() {
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&s.titre).strong().color(theme::accent_texte()));
                                ui.add_space(2.0);
                                egui::Grid::new(("aide_touches_colonne", k)).num_columns(2).spacing([14.0, 3.0]).show(ui, |ui| {
                                    for l in &s.lignes {
                                        ui.label(RichText::new(&l.nom).color(theme::texte()));
                                        ui.label(RichText::new(&l.touches).monospace().color(theme::texte_fort()).background_color(theme::controle()));
                                        ui.end_row();
                                    }
                                });
                            });
                        }
                    });
                });
        });
    }
}
