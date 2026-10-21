//! Interface des annotations de la vue Vignettes (le modèle et le fichier : `annotations.rs`).
//!
//! - **Deux outils**, dans la colonne d'outils quand la planche est à l'écran : le stylo d'annotation (rouge d'origine,
//!   sa couleur et sa taille à lui, dans les Réglages) et la gomme d'annotation. Prendre un autre outil les range.
//!   Aucune touche par défaut (deux commandes de la palette, sans touche : `vignettes.annotation_stylo`,
//!   `vignettes.annotation_gomme`).
//! - **Le stylo passe par-dessus tout** (cases, barres, bandes de note, marges, fond) : l'appui est pris par la planche
//!   avant les cases (`Press::Annot`), donc le dessin de la case courante ne reçoit jamais le trait. Le trait n'est pas
//!   dans le dessin, ni dans les plans, ni dans le film : voir `annotations.rs`. Il est peint pendant le geste, point
//!   après point (aucun calcul de plus qu'une ligne brisée), et posé au lâcher en une étape d'annulation, « Annotation ».
//! - **La gomme** retire le trait entier que le cercle touche ; une étape d'annulation par geste, « Effacer des
//!   annotations ».
//! - **Cacher / montrer** (`vignettes.annotations_afficher`) : un affichage, rien n'est effacé ; prendre le stylo les
//!   montre. **Effacer les annotations** (`vignettes.annotations_effacer`) demande confirmation, s'annule d'un Ctrl+Z.
//! - **Marie-louise** (`vignettes.marie_louise`, `marie.rs`) : affichage d'un passe-partout noir (ou d'une couleur) par-dessus tout ce
//!   qui est hors de l'intérieur des vignettes, traits qui débordent compris ; l'export de la planche a la même option.
//! - Quand une vignette disparaît, ses annotations passent à sa voisine (`Annotations::rattacher_manquantes`), au
//!   moment même (`release_removed_notes`) et, pour les changements que rien n'annonce (annuler, rétablir, ouvrir),
//!   à l'image d'écran suivante (`annot_suivre`).

use eframe::egui::{self, vec2, Color32, Pos2, Rect, RichText, Stroke};

use crate::annotations::{self as ann, Annotations};
use crate::app::App;
use crate::board::BoardLayout;
use crate::boardui::Xf;
use crate::vector::VPoint;

/// Les deux outils d'annotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outil {
    Stylo,
    Gomme,
}

impl Outil {
    pub(crate) fn icone(self) -> &'static str {
        match self {
            Outil::Stylo => egui_phosphor::regular::MARKER_CIRCLE,
            Outil::Gomme => egui_phosphor::regular::ERASER,
        }
    }

    pub(crate) fn nom(self) -> &'static str {
        match self {
            Outil::Stylo => "Stylo d'annotation",
            Outil::Gomme => "Gomme d'annotation",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Outil::Stylo => "Trace en rouge par-dessus la planche pour montrer, corriger, entourer, flécher. N'a aucun effet sur le storyboard : ni dans les plans, ni dans le film, ni dans les exports (sauf la planche, si on le coche).",
            Outil::Gomme => "Efface des annotations, et seulement elles : le trait entier que touche le cercle.",
        }
    }

    pub(crate) fn action(self) -> &'static str {
        match self {
            Outil::Stylo => "vignettes.annotation_stylo",
            Outil::Gomme => "vignettes.annotation_gomme",
        }
    }
}

/// L'état des annotations d'un onglet (hors du projet, sauf les traits eux-mêmes : `Document::annotations`).
pub(crate) struct Etat {
    pub outil: Option<Outil>,
    pub couleur: [u8; 3],
    /// Taille du stylo et de la gomme, en points de la planche.
    pub taille: f32,
    pub taille_gomme: f32,
    /// Les annotations se montrent (un affichage : rien n'est effacé).
    pub visible: bool,
    /// Le passe-partout se montre sur la planche, et sa couleur.
    pub marie: bool,
    pub marie_couleur: [u8; 3],
    /// La confirmation de « Effacer les annotations » est à l'écran.
    pub confirmer: bool,
    /// Le point d'écran de l'appui qui commence un geste (pris par la planche, tracé à l'image d'écran même).
    debut: Option<Pos2>,
    /// Le trait en cours, en points de la planche.
    en_cours: Option<Vec<VPoint>>,
    stylet: bool,
    /// La gomme a déjà pris son étape d'annulation pour ce geste.
    gomme_prise: bool,
    /// La planche à l'image d'écran d'avant (pour rattacher les traits d'une vignette qui disparaît).
    derniere: Option<BoardLayout>,
}

impl Default for Etat {
    fn default() -> Self {
        Etat {
            outil: None,
            couleur: ann::COULEUR_PAR_DEFAUT,
            taille: ann::TAILLE_PAR_DEFAUT,
            taille_gomme: 22.0,
            visible: true,
            marie: false,
            marie_couleur: crate::marie::COULEUR_PAR_DEFAUT,
            confirmer: false,
            debut: None,
            en_cours: None,
            stylet: false,
            gomme_prise: false,
            derniere: None,
        }
    }
}


fn couleur32(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

impl App {
    /// Un outil d'annotation est pris et la planche est à l'écran.
    pub(crate) fn annot_actif(&self) -> bool {
        self.board_shown() && self.vignettes.annot.outil.is_some()
    }

    /// Prend l'outil `outil`, ou le range s'il est déjà pris.
    pub(crate) fn annot_choisir(&mut self, outil: Outil) {
        if !self.board_shown() {
            self.status = "Les annotations se tracent sur la planche : passe à la vue Vignettes.".into();
            return;
        }
        self.end_stroke();
        self.commit_card_note();
        let a = &mut self.vignettes.annot;
        a.debut = None;
        a.en_cours = None;
        if a.outil == Some(outil) {
            a.outil = None;
            self.status = format!("{} rangé.", outil.nom());
        } else {
            a.outil = Some(outil);
            if !a.visible {
                a.visible = true;
            }
            self.status = match outil {
                Outil::Stylo => "Stylo d'annotation : trace par-dessus la planche. Rien de ce que tu traces n'entre dans le storyboard.".into(),
                Outil::Gomme => "Gomme d'annotation : elle n'efface que les annotations.".into(),
            };
        }
    }

    /// Cacher ou montrer les annotations (un affichage).
    pub(crate) fn annot_basculer_visibles(&mut self) {
        let a = &mut self.vignettes.annot;
        a.visible = !a.visible;
        self.status = if a.visible { "Annotations montrées.".into() } else { "Annotations cachées (rien n'est effacé).".into() };
    }

    /// Montrer ou cacher le passe-partout sur la planche.
    pub(crate) fn marie_basculer(&mut self) {
        let a = &mut self.vignettes.annot;
        a.marie = !a.marie;
        self.status = if a.marie { "Marie-louise : seules les vignettes se voient.".into() } else { "Marie-louise retirée.".into() };
    }

    /// Demande de confirmation avant d'effacer toutes les annotations.
    pub(crate) fn annot_demander_effacement(&mut self) {
        if self.doc.annotations.is_empty() {
            self.status = "Il n'y a aucune annotation à effacer.".into();
            return;
        }
        self.vignettes.annot.confirmer = true;
    }

    /// Efface toutes les annotations : une étape d'annulation, « Effacer les annotations ».
    pub(crate) fn annot_effacer_tout(&mut self) {
        self.vignettes.annot.confirmer = false;
        if self.doc.annotations.is_empty() {
            return;
        }
        let n = self.doc.annotations.traces.len();
        self.avec_etape("Effacer les annotations", |a| a.record_step());
        self.doc.annotations = Annotations::default();
        self.status = format!("{n} annotation{} effacée{} (Ctrl+Z les rétablit).", if n > 1 { "s" } else { "" }, if n > 1 { "s" } else { "" });
    }

    // ------------------------------------------------------------------ rattachement

    /// Après une opération qui a retiré des vignettes : leurs annotations passent à leur voisine (`before` : la planche d'avant).
    pub(crate) fn annot_rattacher_apres(&mut self, before: &BoardLayout) {
        if self.doc.annotations.is_empty() {
            return;
        }
        let now = self.board_layout();
        self.doc.annotations.rattacher_manquantes(Some(before), &now);
    }

    /// À chaque image d'écran où la planche est à l'écran : si des vignettes ont changé sans que rien l'annonce (annuler, rétablir…),
    /// leurs annotations passent à la voisine (d'après la planche de l'image d'avant) ; les rangs de secours suivent.
    pub(crate) fn annot_suivre(&mut self, layout: &BoardLayout) {
        let ids = |l: &BoardLayout| l.cards.iter().map(|c| c.id).collect::<Vec<u64>>();
        let a = &mut self.vignettes.annot;
        let same = a.derniere.as_ref().is_some_and(|d| d == layout);
        if same {
            return;
        }
        let previous = a.derniere.take();
        let cards_changed = previous.as_ref().is_none_or(|p| ids(p) != ids(layout));
        if cards_changed && !self.doc.annotations.is_empty() {
            let moved = self.doc.annotations.rattacher_manquantes(previous.as_ref(), layout);
            self.doc.annotations.noter_rangs(layout);
            if moved > 0 {
                self.modified = true;
            }
        }
        self.vignettes.annot.derniere = Some(layout.clone());
    }

    // ------------------------------------------------------------------ gestes

    /// La planche a pris un appui du bouton gauche en `pos` (écran) pour le stylo ou la gomme : le geste commence à cette image d'écran.
    pub(crate) fn annot_appui(&mut self, pos: Pos2) {
        self.vignettes.annot.debut = Some(pos);
    }

    fn annot_pression(&self) -> f32 {
        if self.vignettes.annot.stylet {
            self.pen_pressure.clamp(0.05, 1.0)
        } else {
            1.0
        }
    }

    fn annot_point(&self, p_planche: Pos2) -> VPoint {
        let r = (self.vignettes.annot.taille * 0.5 * (0.25 + 0.75 * self.annot_pression())).max(0.3);
        VPoint { x: p_planche.x, y: p_planche.y, r }
    }

    /// Le geste de l'image d'écran : début, points intermédiaires, fin. `events` : ceux de l'image ; `latest` : le pointeur ; `down` : un
    /// bouton est tenu ; `touch` : la pression du stylet reçue pendant l'image.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn annot_geste(&mut self, layout: &BoardLayout, xf: Xf, events: &[egui::Event], latest: Option<Pos2>, down: bool, touch: bool, escape: bool) {
        let outil = self.vignettes.annot.outil;
        if touch {
            self.vignettes.annot.stylet = true;
        }
        let Some(outil) = outil else {
            self.vignettes.annot.debut = None;
            self.vignettes.annot.en_cours = None;
            return;
        };
        if let Some(pos) = self.vignettes.annot.debut.take() {
            self.vignettes.annot.gomme_prise = false;
            match outil {
                Outil::Stylo => {
                    let p = self.annot_point(xf.to_board(pos));
                    self.vignettes.annot.en_cours = Some(vec![p]);
                }
                Outil::Gomme => {
                    self.vignettes.annot.en_cours = Some(Vec::new());
                    self.annot_gommer(layout, xf.to_board(pos), xf.zoom);
                }
            }
        }
        if self.vignettes.annot.en_cours.is_none() {
            return;
        }
        if escape {
            self.vignettes.annot.en_cours = None;
            self.status = "Trait abandonné.".into();
            return;
        }
        let mut points: Vec<Pos2> = events
            .iter()
            .filter_map(|e| match e {
                egui::Event::PointerMoved(p) => Some(*p),
                egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, .. } => Some(*pos),
                _ => None,
            })
            .collect();
        if down {
            points.extend(latest);
        }
        for p in points {
            let board = xf.to_board(p);
            match outil {
                Outil::Stylo => {
                    let pt = self.annot_point(board);
                    let eps = 0.7 / xf.zoom.max(0.05);
                    if let Some(trace) = self.vignettes.annot.en_cours.as_mut() {
                        if trace.last().is_none_or(|l| (l.pos() - board).length() >= eps) && trace.len() < ann::MAX_POINTS * 4 {
                            trace.push(pt);
                        }
                    }
                }
                Outil::Gomme => self.annot_gommer(layout, board, xf.zoom),
            }
        }
        if !down {
            self.annot_finir(layout);
        }
    }

    /// La gomme passe en `p` (planche) : les traits touchés partent, avec une seule étape d'annulation pour le geste.
    fn annot_gommer(&mut self, layout: &BoardLayout, p: Pos2, zoom: f32) {
        let radius = (self.vignettes.annot.taille_gomme * 0.5).max(4.0 / zoom.max(0.05));
        if !self.doc.annotations.touche(layout, p, radius) {
            return;
        }
        if !self.vignettes.annot.gomme_prise {
            self.vignettes.annot.gomme_prise = true;
            self.avec_etape("Effacer des annotations", |a| a.record_step());
        }
        let n = self.doc.annotations.effacer_en(layout, p, radius);
        if n > 0 {
            self.modified = true;
        }
    }

    /// Le bouton est lâché : le trait est posé (une étape d'annulation), ou le geste de gomme est fini.
    fn annot_finir(&mut self, layout: &BoardLayout) {
        let Some(trace) = self.vignettes.annot.en_cours.take() else { return };
        self.vignettes.annot.stylet = false;
        if self.vignettes.annot.outil != Some(Outil::Stylo) || trace.is_empty() {
            return;
        }
        let trace = ann::epurer(&trace, 0.4);
        if self.doc.annotations.traces.len() >= ann::MAX_TRACES {
            self.status = "Trop d'annotations : efface-en avant d'en tracer d'autres.".into();
            return;
        }
        let color = self.vignettes.annot.couleur;
        // (L'étape d'annulation n'est posée que si le trait est gardé.)
        let snap = self.snapshot();
        if self.doc.annotations.ajouter(layout, &trace, color) {
            self.avec_etape("Annotation", |a| {
                a.history.push(snap);
                a.gesture.reset();
                a.modified = true;
            });
        }
    }

    // ------------------------------------------------------------------ dessin

    /// Les annotations sur la planche, par-dessus tout : celles du projet, puis le trait en cours, puis le curseur de l'outil.
    pub(crate) fn annot_peindre(&self, painter: &egui::Painter, view: Rect, xf: Xf, layout: &BoardLayout, survol: Option<Pos2>) {
        let a = &self.vignettes.annot;
        if a.visible {
            for i in 0..self.doc.annotations.traces.len() {
                if let Some(points) = self.doc.annotations.points_planche(layout, i) {
                    crate::boardui::paint_note(painter, view, xf, &points, self.doc.annotations.traces[i].color);
                }
            }
        }
        if let (Some(Outil::Stylo), Some(trace)) = (a.outil, a.en_cours.as_ref()) {
            crate::boardui::paint_note(painter, view, xf, trace, a.couleur);
        }
        if !self.board_shown() {
            return;
        }
        if let (Some(outil), Some(p)) = (a.outil, survol) {
            let size = if outil == Outil::Stylo { a.taille } else { a.taille_gomme };
            let r = (size * 0.5 * xf.zoom).max(2.0);
            match outil {
                Outil::Stylo => {
                    painter.circle_stroke(p, r + 1.0, Stroke::new(1.0_f32, Color32::from_black_alpha(90)));
                    painter.circle_stroke(p, r, Stroke::new(1.0_f32, couleur32(a.couleur)));
                }
                Outil::Gomme => {
                    painter.circle_stroke(p, r, Stroke::new(1.0_f32, Color32::from_gray(40)));
                }
            }
        }
    }

    /// Le passe-partout de la planche (si on l'a allumé) : tout ce qui est hors de l'intérieur des vignettes, par-dessus tout.
    pub(crate) fn marie_peindre(&self, painter: &egui::Painter, view: Rect, xf: Xf, layout: &BoardLayout) {
        if !self.vignettes.annot.marie {
            return;
        }
        let holes: Vec<Rect> = layout.cards.iter().map(|c| xf.rect(c.thumb)).collect();
        let color = couleur32(self.vignettes.annot.marie_couleur);
        for r in crate::marie::hors_des_cases(view, &holes) {
            painter.rect_filled(r, 0.0, color);
        }
    }

    // ------------------------------------------------------------------ interface

    /// Les deux outils dans une colonne d'outils : l'icône dans la couleur du stylo (le repère « annotation »).
    pub(crate) fn annot_outils_ui(&mut self, ui: &mut egui::Ui, taille: [f32; 2], icone: f32) {
        if !self.board_shown() {
            return;
        }
        ui.add_space(3.0);
        let (rect, _) = ui.allocate_exact_size(vec2(26.0, 1.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 0.0, crate::theme::bord());
        ui.add_space(3.0);
        self.annot_outils_ligne(ui, taille, icone);
    }

    /// Les deux boutons, sans séparateur (dans une ligne, ou sous le trait de la colonne).
    pub(crate) fn annot_outils_ligne(&mut self, ui: &mut egui::Ui, taille: [f32; 2], icone: f32) {
        for outil in [Outil::Stylo, Outil::Gomme] {
            let on = self.vignettes.annot.outil == Some(outil);
            let mut texte = RichText::new(outil.icone()).size(icone);
            if !on {
                texte = texte.color(couleur32(self.vignettes.annot.couleur));
            }
            let resp = ui.add_sized(taille, egui::SelectableLabel::new(on, texte));
            if self.aide(resp, outil.nom(), outil.description(), outil.action()).clicked() {
                self.annot_choisir(outil);
            }
        }
    }

    /// Les réglages du stylo et de la gomme (panneau des réglages de l'outil).
    pub(crate) fn annot_reglages_ui(&mut self, ui: &mut egui::Ui) {
        use crate::bulles;
        use egui_phosphor::regular as ph;
        let Some(outil) = self.vignettes.annot.outil else { return };
        ui.horizontal(|ui| {
            ui.heading(format!("{}  {}", outil.icone(), outil.nom()));
        });
        ui.label(RichText::new("Par-dessus la planche, sans effet sur le storyboard.").small().weak());
        ui.add_space(4.0);
        match outil {
            Outil::Stylo => {
                ui.horizontal(|ui| {
                    bulles::etiquette(ui, ph::PALETTE, "Couleur du stylo", "La couleur des annotations à venir (celles déjà tracées ne changent pas).");
                    ui.color_edit_button_srgb(&mut self.vignettes.annot.couleur);
                    if bulles::icone(ui, ph::ARROW_COUNTER_CLOCKWISE, "Rouge d'origine", "Reprendre le rouge du stylo.", "").clicked() {
                        self.vignettes.annot.couleur = ann::COULEUR_PAR_DEFAUT;
                    }
                });
                ui.horizontal(|ui| {
                    bulles::etiquette(ui, ph::ARROWS_OUT_SIMPLE, "Taille du stylo", "En points de la planche ; la pression du stylet l'amincit.");
                    let resp = ui.add(egui::Slider::new(&mut self.vignettes.annot.taille, ann::TAILLE_MIN..=ann::TAILLE_MAX).logarithmic(true));
                    bulles::bulle(resp, "Taille du stylo", "En points de la planche ; la pression du stylet l'amincit.", "");
                });
            }
            Outil::Gomme => {
                ui.horizontal(|ui| {
                    bulles::etiquette(ui, ph::ARROWS_OUT_SIMPLE, "Taille de la gomme", "Le cercle qui efface : tout trait d'annotation qu'il touche part en entier.");
                    let resp = ui.add(egui::Slider::new(&mut self.vignettes.annot.taille_gomme, 6.0..=120.0).logarithmic(true));
                    bulles::bulle(resp, "Taille de la gomme", "Le cercle qui efface : tout trait d'annotation qu'il touche part en entier.", "");
                });
            }
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let visible = self.vignettes.annot.visible;
            if self.bouton_etat(ui, !visible, if visible { ph::EYE } else { ph::EYE_SLASH }, if visible { "Cacher les annotations" } else { "Montrer les annotations" }, "Un affichage : rien n'est effacé.", "vignettes.annotations_afficher").clicked() {
                self.annot_basculer_visibles();
            }
            let any = !self.doc.annotations.is_empty();
            if self.bouton_si(ui, any, ph::TRASH, "Effacer les annotations", "Efface toutes les annotations de la planche (une confirmation d'abord ; Ctrl+Z les rétablit).", "vignettes.annotations_effacer").clicked() {
                self.annot_demander_effacement();
            }
            let on = self.vignettes.annot.marie;
            if self.bouton_etat(ui, on, ph::FRAME_CORNERS, "Marie-louise", "Cache tout ce qui est hors des vignettes : seules les images se voient, les traits qui débordent aussi sont masqués.", "vignettes.marie_louise").clicked() {
                self.marie_basculer();
            }
        });
        let n = self.doc.annotations.traces.len();
        ui.label(RichText::new(if n == 0 { "Aucune annotation.".to_string() } else { format!("{n} annotation{}.", if n > 1 { "s" } else { "" }) }).small().weak());
    }

    /// Le bas du menu du clic droit sur le fond de la planche.
    pub(crate) fn annot_menu(&mut self, ui: &mut egui::Ui) {
        self.action_item(ui, "Stylo d'annotation", "vignettes.annotation_stylo");
        self.action_item(ui, "Gomme d'annotation", "vignettes.annotation_gomme");
        let visible = self.vignettes.annot.visible;
        self.action_item(ui, if visible { "Cacher les annotations" } else { "Montrer les annotations" }, "vignettes.annotations_afficher");
        if ui.add_enabled(!self.doc.annotations.is_empty(), egui::Button::new("Effacer les annotations…")).clicked() {
            ui.close_menu();
            self.annot_demander_effacement();
        }
        let marie = self.vignettes.annot.marie;
        self.action_item(ui, if marie { "Retirer la marie-louise" } else { "Marie-louise (ne montrer que les vignettes)" }, "vignettes.marie_louise");
        ui.separator();
        self.action_item(ui, "Exporter la planche des vignettes…", "fichier.exporter_planche");
    }

    /// La confirmation de « Effacer les annotations ».
    pub(crate) fn annot_fenetre(&mut self, ctx: &egui::Context) {
        if !self.vignettes.annot.confirmer {
            return;
        }
        let n = self.doc.annotations.traces.len();
        if n == 0 {
            self.vignettes.annot.confirmer = false;
            return;
        }
        let mut oui = false;
        let mut non = false;
        let modal = egui::Modal::new(egui::Id::new("effacer_les_annotations")).show(ctx, |ui| {
            ui.set_width(340.0);
            ui.heading("Effacer les annotations ?");
            ui.add_space(4.0);
            ui.label(format!("{n} annotation{} de la planche {} effacée{}. Les plans, les dessins et le film ne changent pas. Ctrl+Z les rétablit.", if n > 1 { "s" } else { "" }, if n > 1 { "seront" } else { "sera" }, if n > 1 { "s" } else { "" }));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                oui = ui.button(format!("{} Effacer", egui_phosphor::regular::TRASH)).clicked();
                non = ui.button("Garder").clicked();
            });
        });
        if oui {
            self.annot_effacer_tout();
        } else if non || modal.should_close() {
            self.vignettes.annot.confirmer = false;
        }
    }
}
