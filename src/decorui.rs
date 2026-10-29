//! Les brosses perso de type **Décor**, côté interface (`decor.rs` pour le calque et le partage entre les cases ; `personnagesui.rs` pour la
//! barre et le panneau où elles vivent avec les autres brosses perso).
//!
//! # Ce que c'est (2026-10-29, §70 de `docs/INTERFACE.md`)
//!
//! Avant, trois pinceaux de décor fixes (crayon, feutre gris, aplat) d'une seule couleur, réglés dans Préférences > Vignettes. Maintenant, ce
//! sont des **brosses perso** comme les autres (`personnages::Brosse`, un réglage de l'application) : chacune a son nom, sa couleur de trait,
//! sa couleur d'aplat (ou pas), et, parce que ce sont des pinceaux de décor, **un pinceau** (crayon, feutre ou pinceau plein) et **une
//! taille**. Trois brosses Décor d'origine, en niveaux de gris, reprennent les trois anciens pinceaux (Décor 1 : crayon 5 px, Décor 2 :
//! feutre 24 px, Décor 3 : pinceau plein 90 px). Dans la barre, ce sont de petits carrés arrondis (`personnagesui.rs`), aux couleurs de leur
//! brosse. Elles sont proposées dans tout le côté storyboard (`Mode::Storyboard`, `decor_offert`), pas dans l'animation (le décor y est un
//! calque par plan, et un plan du storyboard est ce que montrent ses cases).
//!
//! # Ce que fait le choix
//!
//! Choisir une brosse Décor prend l'outil qu'il faut (Crayon pour le crayon, Pinceau pour le feutre et le pinceau plein), le réglage à la taille
//! de la brosse et sa couleur de trait ; l'outil, les deux pinceaux, la couleur et le calque courant d'avant sont mis de côté (`Avant`) et
//! reviennent quand on lâche la brosse (clic sur elle, ou le choix d'un personnage ou d'une classique). Tant qu'elle est choisie, **chaque
//! trait part dans le calque Décor du plan où il commence** (`decor_avant_trait`, à l'appui sur une case ou sur le dessin) : le calque est créé
//! à ce premier trait, tout en bas du plan, et le trait et la création ne font qu'une étape d'annulation (`Vignettes::created`, la même
//! mécanique que pour un plan fait en dessinant dans une case vide). Choisir à la main un autre calque, ou une autre couleur, lâche la brosse.
//! Le calque Décor créé est vectoriel d'origine, bitmap si Préférences > Vignettes (« Calque Décor ») le demande ; celui d'un plan qui en a
//! déjà un garde sa sorte (`Document::assurer_decor`). La couleur d'aplat de la brosse suit la règle des brosses sans calque à leur nom
//! (partie 33) : elle devient celle de l'aplat du calque Décor. Les personnages ont leur calque aussi : `calquespastilles.rs`.
//!
//! Changer la couleur, le pinceau ou la taille d'une brosse Décor choisie la fait suivre tout de suite (`decor_tick`).

use eframe::egui::Color32;

use crate::app::{App, Tool};
use crate::brush::BrushSettings;
use crate::decor::Genre;
use crate::personnagesui::Choix;
use crate::vignettesreglages::en_couleur;

/// Ce que le choix du décor a pris à l'utilisateur et lui rendra.
#[derive(Clone, Debug)]
pub(crate) struct Avant {
    pub tool: Tool,
    pub brush: BrushSettings,
    pub pencil: BrushSettings,
    pub color: [u8; 3],
    /// Le calque courant (son numéro) quand le décor a été choisi.
    pub layer: Option<u64>,
}

/// Ce que la brosse Décor `i` a posé : couleur du trait, taille, pinceau (pour voir quand le panneau les change).
pub(crate) type Posee = ([u8; 3], f32, Genre);

impl App {
    /// La couleur des cadres de caméra (Préférences > Vignettes).
    pub(crate) fn cadre_couleur(&self) -> Color32 {
        en_couleur(self.settings.vignettes.couleur_cadre)
    }

    /// Le rang de la brosse Décor qui dessine, s'il y en a une.
    pub(crate) fn decor_actif(&self) -> Option<usize> {
        match self.perso.actif {
            Some(Choix::Decor(i)) => Some(i),
            _ => None,
        }
    }

    /// Ce que pose la brosse Décor `i` : (couleur du trait, taille, pinceau).
    fn decor_voulu(&self, i: usize) -> Option<Posee> {
        self.settings.brosses_decor.get(i).map(|b| (b.line, b.taille_decor(), b.genre()))
    }

    /// Choisit la brosse Décor `i`.
    pub(crate) fn decor_choisir(&mut self, i: usize) {
        let Some(nom) = self.settings.brosses_decor.get(i).map(|b| b.name.clone()) else { return };
        self.end_stroke();
        if self.decor_actif().is_none() {
            self.perso.decor_avant = Some(Avant { tool: self.tool, brush: self.brush, pencil: self.pencil, color: self.color, layer: self.layers().get(self.cur_layer).map(|l| l.id) });
        }
        // Le décor remplace le personnage ou la classique choisis (et leur aplat).
        self.aplat_cible = None;
        self.perso.actif = Some(Choix::Decor(i));
        self.decor_poser(i);
        self.status = format!("« {nom} » : une brosse Décor, ses traits vont dans le calque Décor du plan (créé au premier trait s'il n'existe pas encore).");
    }

    /// Prend l'outil, le réglage et la couleur de la brosse Décor `i`.
    fn decor_poser(&mut self, i: usize) {
        let Some((couleur, taille, genre)) = self.decor_voulu(i) else { return };
        let reglage = genre.reglage(taille);
        if genre.est_crayon() {
            self.pencil = reglage;
            self.set_tool(Tool::Pencil);
        } else {
            self.brush = reglage;
            self.set_tool(Tool::Brush);
        }
        self.set_front(couleur);
        self.perso.posee = couleur;
        self.perso.decor_posee = Some((couleur, taille, genre));
    }

    /// Lâche la brosse Décor : l'outil, les pinceaux et la couleur d'avant reviennent ; `calque` : le calque courant aussi (sinon on
    /// vient de choisir un calque soi-même).
    pub(crate) fn decor_lacher(&mut self, calque: bool) {
        self.decor_lacher_avec(calque, true);
    }

    pub(crate) fn decor_lacher_avec(&mut self, calque: bool, couleur: bool) {
        if self.decor_actif().is_none() {
            return;
        }
        self.end_stroke();
        self.perso.actif = None;
        self.perso.decor_posee = None;
        let Some(a) = self.perso.decor_avant.take() else { return };
        self.brush = a.brush;
        self.pencil = a.pencil;
        self.set_tool(a.tool);
        if couleur {
            self.set_front(a.color);
        }
        if calque {
            self.decor_rendre_le_calque(a.layer);
        }
    }

    /// Le calque courant ne reste pas le calque Décor : celui d'avant (s'il est de ce plan), sinon le plus haut qui se dessine.
    fn decor_rendre_le_calque(&mut self, avant: Option<u64>) {
        if let Some(i) = avant.and_then(|id| self.layers().iter().position(|l| l.id == id)) {
            self.cur_layer = i;
        } else if self.layers().get(self.cur_layer).is_some_and(|l| l.decor) {
            if let Some(i) = self.layers().iter().rposition(|l| !l.decor && !l.folder && l.effet.is_none()) {
                self.cur_layer = i;
            }
        }
    }

    /// Un calque choisi à la main (panneau Calques, en-tête de la timeline) lâche la brosse Décor : on dessine où on a cliqué.
    pub(crate) fn decor_calque_choisi(&mut self) {
        self.decor_lacher(false);
    }

    /// Les brosses Décor sont-elles proposées ici ? Tout le côté storyboard : la planche des Vignettes et le dessin du Storyboard
    /// (pas l'animation, ni l'Espace 3D).
    pub(crate) fn decor_offert(&self) -> bool {
        self.mode == crate::app::Mode::Storyboard && !(self.espace3d.on && !self.camera_view)
    }

    /// À chaque image : la brosse Décor suit son réglage (couleur, taille, pinceau), se lâche quand on quitte le côté storyboard (l'animation),
    /// quand elle est supprimée, ou quand une autre couleur est prise à la main (elle reste).
    pub(crate) fn decor_tick(&mut self) {
        let Some(i) = self.decor_actif() else { return };
        let Some(voulu) = self.decor_voulu(i) else {
            self.decor_lacher(true);
            return;
        };
        if !self.decor_offert() {
            self.decor_lacher(true);
            return;
        }
        if self.color != self.perso.posee {
            self.decor_lacher_avec(true, false);
            return;
        }
        if self.perso.decor_posee != Some(voulu) {
            self.decor_poser(i);
        }
    }

    /// À l'appui d'un trait dans une case de la planche, ou sur le dessin du Storyboard : si une brosse Décor est choisie, le plan a
    /// son calque Décor (créé au besoin, en bas du plan) et il est le calque courant. La création et le trait ne font qu'une étape
    /// d'annulation (`finish_creation`, au lâcher).
    pub(crate) fn decor_avant_trait(&mut self) {
        if self.decor_actif().is_none() || !matches!(self.tool, Tool::Brush | Tool::Pencil | Tool::Eraser | Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Polyline | Tool::Fill) {
            return;
        }
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        let frame = self.cur_frame.min(self.frame_count().saturating_sub(1));
        self.doc.report_decor = self.settings.vignettes.report_decor;
        // Le calque Décor neuf est vectoriel, ou bitmap si Préférences > Vignettes le demande ; un décor qui existe déjà garde sa sorte.
        self.doc.decor_bitmap = self.settings.vignettes.decor_bitmap;
        let snap = self.snapshot();
        let existait = self.doc.decor_de(self.cur_panel).is_some();
        let Some((at, change)) = self.doc.assurer_decor(self.cur_panel, frame) else { return };
        self.cur_layer = at;
        if !change {
            return;
        }
        // Le calque Décor neuf naît avec l'aplat de la brosse (la sous-ligne « Aplat » allumée à sa couleur), dans la même étape que le
        // trait : comme le calque d'un personnage, pas une étape de plus. (Sans aplat, ou pour un décor qui existe déjà, rien de plus.)
        if !existait {
            let fond = self.perso_couleurs_actives().and_then(|(_, fond)| fond);
            let reglages = fond.map(|couleur| crate::fond::FondReglages { couleur, ..self.new_fond() });
            if let Some(l) = self.layers_mut().get_mut(at) {
                l.fond = reglages;
            }
        }
        self.modified = true;
        if self.vignettes.created.is_none() {
            // Le calque et le premier trait : une étape (la création se joint au trait au lâcher, `finish_creation`).
            let nom = "Dessiner le décor";
            self.avec_etape(nom, |s| s.commit_structure(snap));
            self.vignettes.created = Some(crate::boardui::Created { base_len: self.history.undo_len() });
        } else {
            // Dans une case qu'on vient de faire : l'étape de la case défait tout, le calque avec.
            self.sync_couleurs();
            self.renderer.invalidate_all();
        }
    }
}
