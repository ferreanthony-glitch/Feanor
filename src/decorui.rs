//! Les brosses du décor, côté interface (`decor.rs` pour le calque et le partage entre les cases).
//!
//! # Où elles sont, et pourquoi
//!
//! Dans la **colonne des pastilles** de la planche (`personnagesui::pastilles_colonne`), après un petit trait, sous les couleurs
//! neutres : trois carrés arrondis (crayon, feutre gris, aplat), de la couleur du décor. La colonne est déjà le « avec quoi je dessine ? »
//! de la planche (personnage, couleur neutre ou couleur libre) : le décor y est un choix de plus, sans rien ajouter à la barre d'outils ni
//! aux Vignettes. Des carrés, pas des ronds, pour qu'on ne les prenne pas pour des personnages. Elles sont aussi dans la colonne du
//! dessin du Storyboard (Anthony, 2026-10-23 : les mêmes brosses, réglées dans Préférences > Vignettes) : tout le côté storyboard
//! (`Mode::Storyboard`, `decor_offert`), pas l'animation (le décor y est un calque par plan, et un plan du storyboard est ce que montrent
//! ses cases). Dans le dessin, le trait part aussi dans le calque Décor du plan courant, créé au premier trait en bas du plan, tenu sur
//! toutes ses images (le même dessin que dans ses cases) ; un simple clic qui ne trace rien ne laisse pas de calque vide.
//!
//! # Ce que fait le choix
//!
//! Choisir une brosse du décor prend l'outil qu'il faut (Crayon pour le crayon, Pinceau pour le feutre et l'aplat), son réglage à la
//! taille des Préférences et la couleur du décor ; l'outil, les deux pinceaux, la couleur et le calque courant d'avant sont mis de côté
//! (`Avant`) et reviennent quand on lâche le décor (clic sur sa pastille, « classique », ou un personnage). Tant qu'elle est choisie,
//! **chaque trait part dans le calque Décor du plan où il commence** (`decor_avant_trait`, à l'appui sur une case) : le calque est créé
//! à ce premier trait, tout en bas du plan, et le trait et la création ne font qu'une étape d'annulation (`Vignettes::created`, la même
//! mécanique que pour un plan fait en dessinant dans une case vide). Choisir à la main un autre calque, ou une autre couleur, lâche le décor.

use eframe::egui::{self, vec2, Color32, Rect, Response, Sense, Stroke, StrokeKind};

use crate::app::{App, Tool};
use crate::brush::BrushSettings;
use crate::decor::Genre;
use crate::personnagesui::PASTILLE;
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

impl App {
    /// La couleur des cadres de caméra (Préférences > Vignettes).
    pub(crate) fn cadre_couleur(&self) -> Color32 {
        en_couleur(self.settings.vignettes.couleur_cadre)
    }

    /// Choisit la brosse `g` du décor.
    pub(crate) fn decor_choisir(&mut self, g: Genre) {
        self.end_stroke();
        if self.perso.decor.is_none() {
            self.perso.decor_avant = Some(Avant { tool: self.tool, brush: self.brush, pencil: self.pencil, color: self.color, layer: self.layers().get(self.cur_layer).map(|l| l.id) });
        }
        // Le décor remplace le personnage choisi (et son aplat).
        self.perso.actif = None;
        self.aplat_cible = None;
        self.perso.decor = Some(g);
        self.decor_poser(g);
        self.status = format!("{} : ses traits vont dans le calque Décor du plan.", g.nom());
    }

    /// Prend l'outil, le réglage et la couleur de la brosse `g` selon les Préférences.
    fn decor_poser(&mut self, g: Genre) {
        let (couleur, taille) = (self.settings.vignettes.couleur_decor, self.settings.vignettes.taille_decor(g));
        let reglage = g.reglage(taille);
        if g.est_crayon() {
            self.pencil = reglage;
            self.set_tool(Tool::Pencil);
        } else {
            self.brush = reglage;
            self.set_tool(Tool::Brush);
        }
        self.set_front(couleur);
        self.perso.posee = couleur;
        self.perso.decor_posee = Some((couleur, taille));
    }

    /// Lâche la brosse du décor : l'outil, les pinceaux et la couleur d'avant reviennent ; `calque` : le calque courant aussi (sinon on
    /// vient de choisir un calque soi-même).
    pub(crate) fn decor_lacher(&mut self, calque: bool) {
        self.decor_lacher_avec(calque, true);
    }

    fn decor_lacher_avec(&mut self, calque: bool, couleur: bool) {
        if self.perso.decor.take().is_none() {
            return;
        }
        self.perso.decor_posee = None;
        self.end_stroke();
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

    /// Un calque choisi à la main (panneau Calques, en-tête de la timeline) lâche le décor : on dessine où on a cliqué.
    pub(crate) fn decor_calque_choisi(&mut self) {
        self.decor_lacher(false);
    }

    /// Les brosses du décor sont-elles proposées ici ? Tout le côté storyboard : la planche des Vignettes et le dessin du Storyboard
    /// (pas l'animation, ni l'Espace 3D).
    pub(crate) fn decor_offert(&self) -> bool {
        self.mode == crate::app::Mode::Storyboard && !(self.espace3d.on && !self.camera_view)
    }

    /// À chaque image : le décor suit les Préférences (couleur, taille), se lâche quand on quitte le côté storyboard (l'animation),
    /// ou quand une autre couleur est prise à la main (elle reste).
    pub(crate) fn decor_tick(&mut self) {
        let Some(g) = self.perso.decor else { return };
        if !self.decor_offert() {
            self.decor_lacher(true);
            return;
        }
        if self.color != self.perso.posee {
            self.decor_lacher_avec(true, false);
            return;
        }
        let voulu = (self.settings.vignettes.couleur_decor, self.settings.vignettes.taille_decor(g));
        if self.perso.decor_posee != Some(voulu) {
            self.decor_poser(g);
        }
    }

    /// À l'appui d'un trait dans une case de la planche, ou sur le dessin du Storyboard : si une brosse du décor est choisie, le plan a
    /// son calque Décor (créé au besoin, en bas du plan) et il est le calque courant. La création et le trait ne font qu'une étape
    /// d'annulation (`finish_creation`, au lâcher).
    pub(crate) fn decor_avant_trait(&mut self) {
        if self.perso.decor.is_none() || !matches!(self.tool, Tool::Brush | Tool::Pencil | Tool::Eraser | Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Polyline | Tool::Fill) {
            return;
        }
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        let frame = self.cur_frame.min(self.frame_count().saturating_sub(1));
        self.doc.report_decor = self.settings.vignettes.report_decor;
        let snap = self.snapshot();
        let Some((at, change)) = self.doc.assurer_decor(self.cur_panel, frame) else { return };
        self.cur_layer = at;
        if !change {
            return;
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

    /// Une pastille de la colonne : un carré arrondi de la couleur du décor, avec le signe de sa brosse. Clic : choisir (ou lâcher) ;
    /// clic droit : régler les brosses du décor dans les Préférences.
    pub(crate) fn pastille_decor(&mut self, ui: &mut egui::Ui, g: Genre) -> Response {
        let choisie = self.perso.decor == Some(g);
        let couleur = en_couleur(self.settings.vignettes.couleur_decor);
        let (rect, resp) = ui.allocate_exact_size(vec2(PASTILLE + 6.0, PASTILLE + 6.0), Sense::click());
        if ui.is_rect_visible(rect) {
            let carre = Rect::from_center_size(rect.center(), vec2(PASTILLE, PASTILLE));
            let p = ui.painter();
            if choisie {
                p.rect_filled(carre.expand(3.0), 8.0, crate::theme::accent_texte());
            } else if resp.hovered() {
                p.rect_stroke(carre.expand(2.0), 7.0, Stroke::new(1.5_f32, crate::theme::bord()), StrokeKind::Outside);
            }
            p.rect_filled(carre, 6.0, couleur);
            let dedans = carre.shrink(3.5);
            p.rect_filled(dedans, 3.5, crate::theme::entete());
            let (a, b) = (dedans.left_bottom() + vec2(2.5, -2.5), dedans.right_top() + vec2(-2.5, 2.5));
            match g {
                Genre::Crayon => {
                    p.line_segment([a, b], Stroke::new(1.5_f32, couleur));
                }
                Genre::Feutre => {
                    p.line_segment([a, b], Stroke::new(4.0_f32, couleur));
                }
                Genre::Aplat => {
                    p.rect_filled(dedans.shrink(2.5), 1.5, couleur);
                }
            }
            p.rect_stroke(carre, 6.0, Stroke::new(1.0_f32, Color32::from_black_alpha(90)), StrokeKind::Inside);
        }
        let clic = if choisie { "Clic : lâcher le décor" } else { "Clic : dessiner le décor avec" };
        let resp = crate::bulles::bulle(resp, g.nom(), &format!("{} {clic} ; clic droit : régler les brosses du décor.", g.description()), "");
        if resp.clicked() {
            if choisie {
                self.decor_lacher(true);
                self.status = "Couleur libre : plus de brosse du décor.".into();
            } else {
                self.decor_choisir(g);
            }
        }
        resp.clone().context_menu(|ui| {
            if ui.button("Régler les brosses du décor…").clicked() {
                self.settings.prefs_onglet = crate::prefsonglets::Onglet::Vignettes;
                self.prefs_window = true;
                ui.close_menu();
            }
        });
        resp
    }
}
