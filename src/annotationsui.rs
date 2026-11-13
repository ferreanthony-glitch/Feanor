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
//!
//! # Les calques d'annotation et le lasso (Anthony, 2026-10-24)
//!
//! - **Le calque visé** : les trois outils (stylo, gomme, lasso) agissent sur *un* calque d'annotation, le calque actif (`Etat::calque`).
//!   Le panneau « Planche » le liste, avec « Dessin » en bas (le dessin de la case) : un clic sur un calque d'annotation le vise
//!   (et prend le stylo s'il n'y a pas d'outil d'annotation), un clic sur « Dessin » range les outils d'annotation, les outils de dessin
//!   agissent de nouveau sur la case (`vignettespanneau.rs`). Un calque caché qu'on vise est remontré.
//! - **Le lasso** (`vignettes.annotation_lasso`) : glisser dans le vide entoure des traits du calque actif (au moins un point dans le
//!   lasso, comme le lasso d'un dessin) ; un clic choisit le trait dessous ; Maj ajoute. Les traits choisis ont la boîte de
//!   sélection de SHOT (`selbox.rs` : huit poignées et un pivot) : glisser dedans déplace, un coin ou un côté met à l'échelle, hors
//!   d'un coin tourne, un bord incline. **Une étape d'annulation par geste** (« Déplacer des annotations », « Mettre à l'échelle… »,
//!   « Tourner… », « Incliner… »). Échap abandonne le geste en cours (rien n'a bougé), puis lève la sélection. Le trait déplacé est
//!   rattaché à la case la plus proche de son nouveau centre (`Annotations::transformer`).
//! - **Calques** : ajouter, renommer (double-clic), supprimer, monter, descendre, œil par calque ; chaque changement est une étape d'annulation.

use eframe::egui::{self, vec2, Color32, Modifiers, Pos2, Rect, RichText, Stroke};

use crate::annotations as ann;
use crate::app::App;
use crate::board::BoardLayout;
use crate::boardui::Xf;
use crate::selbox::{self, Grabs, Handle, Mods, SelFrame};
use crate::vector::{Affine, VPoint};

/// Les trois outils d'annotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outil {
    Stylo,
    Gomme,
    Lasso,
}

impl Outil {
    pub(crate) const TOUS: [Outil; 3] = [Outil::Stylo, Outil::Gomme, Outil::Lasso];

    pub(crate) fn icone(self) -> &'static str {
        match self {
            Outil::Stylo => egui_phosphor::regular::MARKER_CIRCLE,
            Outil::Gomme => egui_phosphor::regular::ERASER,
            Outil::Lasso => egui_phosphor::regular::LASSO,
        }
    }

    pub(crate) fn nom(self) -> &'static str {
        match self {
            Outil::Stylo => "Stylo d'annotation",
            Outil::Gomme => "Gomme d'annotation",
            Outil::Lasso => "Lasso d'annotation",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Outil::Stylo => "Trace en rouge par-dessus la planche pour montrer, corriger, entourer, flécher. N'a aucun effet sur le storyboard : ni dans les plans, ni dans le film, ni dans les exports (sauf la planche, si on le coche).",
            Outil::Gomme => "Efface des annotations, et seulement elles : le trait entier que touche le cercle.",
            Outil::Lasso => "Entoure des annotations du calque visé (ou clique-en une), puis déplace, agrandis ou tourne-les avec la boîte. Maj ajoute. Échap abandonne.",
        }
    }

    pub(crate) fn action(self) -> &'static str {
        match self {
            Outil::Stylo => "vignettes.annotation_stylo",
            Outil::Gomme => "vignettes.annotation_gomme",
            Outil::Lasso => "vignettes.annotation_lasso",
        }
    }
}

/// Le lasso en train d'être tracé : ses points (planche), et Maj tenue à l'appui (ajouter à ce qui est choisi).
struct LassoEnCours {
    points: Vec<Pos2>,
    ajout: bool,
}

/// Un geste de la boîte de sélection sur les traits choisis : la partie prise, où (planche), la boîte de départ et la transformation
/// que le geste donne maintenant (`selbox::gesture`). Rien n'est changé dans les traits avant le lâcher.
struct GesteBoite {
    handle: Handle,
    start: Pos2,
    frame: SelFrame,
    aff: Affine,
    /// Le pointeur a bougé : il y aura une étape d'annulation au lâcher.
    change: bool,
}

/// Au plus ce nombre de points de lasso sont gardés (un lasso très long est éclairci : la ligne reste fidèle, le calcul court).
const LASSO_MAX: usize = 4000;
/// À moins de tant de pixels d'écran du centre, le pivot tiré s'y remet.
const PIVOT_SNAP: f32 = 6.0;
/// Le lasso qui ne dépasse pas ces pixels d'écran est un clic (il choisit le trait dessous).
const CLIC_PX: f32 = 4.0;
/// Tolérance d'un clic sur un trait, en pixels d'écran.
const PRISE_PX: f32 = 6.0;

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
    /// Le calque d'annotation visé (l'identifiant d'un `Calque`) ; 0 ou un calque disparu : le premier.
    pub calque: u64,
    /// Les traits choisis au lasso (par identifiant : `Trace::id`).
    pub selection: Vec<u64>,
    /// Le calque qu'on tient pour le ranger, dans la liste du panneau Planche (`vignettespanneau.rs`).
    pub glisse: Option<u64>,
    /// Le calque qu'on est en train de renommer, et le texte.
    pub renomme: Option<(u64, String)>,
    /// Le point d'écran de l'appui qui commence un geste (pris par la planche, tracé à l'image d'écran même).
    debut: Option<Pos2>,
    /// Le trait en cours, en points de la planche.
    en_cours: Option<Vec<VPoint>>,
    stylet: bool,
    /// La gomme a déjà pris son étape d'annulation pour ce geste.
    gomme_prise: bool,
    /// La planche à l'image d'écran d'avant (pour rattacher les traits d'une vignette qui disparaît).
    derniere: Option<BoardLayout>,
    /// Le lasso en cours, et le geste de la boîte en cours.
    lasso: Option<LassoEnCours>,
    boite: Option<GesteBoite>,
    /// Le pivot tiré (planche) ; rien : le centre de la boîte.
    pivot: Option<Pos2>,
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
            calque: 0,
            selection: Vec::new(),
            glisse: None,
            renomme: None,
            debut: None,
            en_cours: None,
            stylet: false,
            gomme_prise: false,
            derniere: None,
            lasso: None,
            boite: None,
            pivot: None,
        }
    }
}

impl Etat {
    /// Abandonne ce que le lasso tient : le lasso en cours, le geste de la boîte (rien n'a bougé), le pivot tiré.
    fn lacher_le_geste(&mut self) {
        self.lasso = None;
        self.boite = None;
    }

    /// Plus rien n'est choisi : la boîte, le pivot et les gestes s'en vont.
    fn tout_lacher(&mut self) {
        self.lacher_le_geste();
        self.selection.clear();
        self.pivot = None;
    }
}


/// Où tombe un calque glissé dans la liste du panneau : `centres` sont les centres (y) des lignes, du haut (le calque du dessus) vers le bas,
/// `d` la ligne tenue, `y` le pointeur. Rend la ligne devant laquelle il s'insère (de 0 à `centres.len()` : la dernière, sous tout) et sa
/// place dans `Annotations::calques` (0 : tout en bas).
pub(crate) fn depot_du_calque(centres: &[f32], d: usize, y: f32) -> (usize, usize) {
    let n = centres.len();
    let ligne = centres.iter().filter(|c| **c < y).count();
    // (La ligne tenue ne compte plus une fois levée : après elle, les rangs reculent d'un.)
    let rang = if ligne > d { ligne - 1 } else { ligne };
    (ligne, n.saturating_sub(1 + rang))
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
        // (Changer d'outil lâche ce que le lasso tenait ; le calque visé, lui, reste.)
        a.tout_lacher();
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
                Outil::Lasso => "Lasso d'annotation : entoure des traits du calque visé, puis déplace-les, agrandis-les ou tourne-les.".into(),
            };
            // Le calque visé se montre : on ne trace ni ne choisit rien sur ce qu'on ne voit pas.
            self.annot_montrer_le_calque_vise();
        }
    }

    // ------------------------------------------------------------------ les calques d'annotation

    /// Le calque d'annotation visé : celui qu'on a pris s'il existe, sinon le premier ; rien tant qu'il n'y en a aucun
    /// (le premier trait en crée un).
    pub(crate) fn annot_calque_actif(&self) -> Option<u64> {
        let a = &self.doc.annotations;
        a.calque(self.vignettes.annot.calque).or_else(|| a.calques.first()).map(|c| c.id)
    }

    /// Le calque visé est caché : il se montre (un affichage, mais rien ne se fait sur ce qu'on ne voit pas).
    fn annot_montrer_le_calque_vise(&mut self) {
        if let Some(id) = self.annot_calque_actif() {
            if self.doc.annotations.regler_visible(id, true) {
                self.modified = true;
            }
        }
    }

    /// Vise le calque `id` : les trois outils y agissent. S'il n'y a pas d'outil d'annotation, le stylo est pris ; sinon l'outil reste.
    /// Ce qui était choisi au lasso est lâché.
    pub(crate) fn annot_viser_le_calque(&mut self, id: u64) {
        if self.doc.annotations.calque(id).is_none() {
            return;
        }
        if self.vignettes.annot.calque != id {
            self.vignettes.annot.tout_lacher();
        }
        self.vignettes.annot.calque = id;
        match self.vignettes.annot.outil {
            None => self.annot_choisir(Outil::Stylo),
            Some(_) => self.annot_montrer_le_calque_vise(),
        }
        if let Some(c) = self.doc.annotations.calque(id) {
            self.status = format!("Calque d'annotation « {} » visé : le stylo, la gomme et le lasso y agissent.", c.nom);
        }
    }

    /// Vise le dessin de la case : les outils d'annotation sont rangés, les outils de dessin agissent de nouveau sur la case.
    pub(crate) fn annot_viser_le_dessin(&mut self) {
        if self.vignettes.annot.outil.is_none() {
            return;
        }
        self.end_stroke();
        let a = &mut self.vignettes.annot;
        a.debut = None;
        a.en_cours = None;
        a.outil = None;
        a.tout_lacher();
        self.status = "Dessin de la case visé : les outils de dessin agissent de nouveau sur la case.".into();
    }

    /// Un calque d'annotation de plus, au-dessus des autres, visé (une étape d'annulation).
    pub(crate) fn annot_calque_nouveau(&mut self) {
        if !self.board_shown() {
            self.status = "Les calques d'annotation sont ceux de la planche : passe à la vue Vignettes.".into();
            return;
        }
        if self.doc.annotations.calques.len() >= ann::MAX_CALQUES {
            self.status = "Trop de calques d'annotation.".into();
            return;
        }
        self.avec_etape("Ajouter un calque d'annotation", |a| a.record_step());
        let Some(id) = self.doc.annotations.ajouter_calque() else { return };
        self.annot_viser_le_calque(id);
        if let Some(c) = self.doc.annotations.calque(id) {
            self.status = format!("Calque d'annotation « {} » ajouté.", c.nom);
        }
    }

    /// Renomme un calque d'annotation (une étape d'annulation, si le nom change).
    pub(crate) fn annot_calque_renommer(&mut self, id: u64, nom: &str) {
        let nom = nom.trim();
        if nom.is_empty() || self.doc.annotations.calque(id).is_none_or(|c| c.nom == nom) {
            return;
        }
        self.avec_etape("Renommer un calque d'annotation", |a| a.record_step());
        self.doc.annotations.renommer_calque(id, nom);
        self.status = format!("Calque d'annotation renommé « {nom} ».");
    }

    /// Supprime un calque d'annotation et ses traits (une étape d'annulation les rend).
    pub(crate) fn annot_calque_supprimer(&mut self, id: u64) {
        if self.doc.annotations.calque(id).is_none() {
            return;
        }
        self.avec_etape("Supprimer un calque d'annotation", |a| a.record_step());
        let n = self.doc.annotations.supprimer_calque(id).unwrap_or(0);
        self.vignettes.annot.tout_lacher();
        self.status = format!("Calque d'annotation supprimé ({n} annotation{} ; Ctrl+Z le rend).", if n > 1 { "s" } else { "" });
    }

    /// Monte ou descend un calque d'annotation d'un cran (une étape d'annulation).
    pub(crate) fn annot_calque_deplacer(&mut self, id: u64, vers_le_haut: bool) {
        let ok = {
            let a = &self.doc.annotations;
            a.calques.iter().position(|c| c.id == id).is_some_and(|k| if vers_le_haut { k + 1 < a.calques.len() } else { k > 0 })
        };
        if !ok {
            return;
        }
        self.avec_etape(if vers_le_haut { "Monter un calque d'annotation" } else { "Descendre un calque d'annotation" }, |a| a.record_step());
        self.doc.annotations.deplacer_calque(id, vers_le_haut);
    }

    /// Pose le calque `id` à la place `vers` de la liste (0 : tout en bas, le dernier : tout en haut) : le glissé du panneau Planche, **une
    /// seule étape d'annulation par dépôt**, aucune si le calque ne change pas de place.
    pub(crate) fn annot_calque_placer(&mut self, id: u64, vers: usize) {
        let a = &self.doc.annotations;
        let Some(k) = a.calques.iter().position(|c| c.id == id) else { return };
        if vers.min(a.calques.len() - 1) == k {
            return;
        }
        self.avec_etape("Ranger un calque d'annotation", |a| a.record_step());
        self.doc.annotations.placer_calque(id, vers);
        self.status = "Calque d'annotation rangé (Ctrl+Z le remet).".into();
    }

    /// Suppr : efface les traits choisis au lasso d'annotation, **une seule étape d'annulation** (« Supprimer des annotations »). Faux, sans
    /// rien faire, quand rien n'est choisi.
    pub(crate) fn annot_supprimer_selection(&mut self) -> bool {
        let ids = self.vignettes.annot.selection.clone();
        if ids.is_empty() {
            return false;
        }
        // (Un trait choisi qui n'existe plus : rien à effacer, plus rien n'est choisi.)
        if !self.doc.annotations.traces.iter().any(|t| ids.contains(&t.id)) {
            self.vignettes.annot.tout_lacher();
            return false;
        }
        self.avec_etape("Supprimer des annotations", |a| a.record_step());
        let n = self.doc.annotations.supprimer_traces(&ids);
        self.vignettes.annot.tout_lacher();
        self.status = format!("{n} annotation{} supprimée{} (Ctrl+Z les rend).", if n > 1 { "s" } else { "" }, if n > 1 { "s" } else { "" });
        true
    }

    /// Montre ou cache un calque d'annotation (un affichage : rien n'est effacé ni annulable). Cacher le calque visé range ce que le
    /// lasso tenait.
    pub(crate) fn annot_calque_basculer(&mut self, id: u64) {
        let Some(visible) = self.doc.annotations.calque(id).map(|c| c.visible) else { return };
        self.doc.annotations.regler_visible(id, !visible);
        self.modified = true;
        if visible && self.annot_calque_actif() == Some(id) {
            self.vignettes.annot.tout_lacher();
        }
        if let Some(c) = self.doc.annotations.calque(id) {
            self.status = if visible { format!("Calque « {} » caché (rien n'est effacé).", c.nom) } else { format!("Calque « {} » montré.", c.nom) };
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
        let on = a.marie;
        // (Retenu d'une séance à l'autre : le même réglage que Préférences > Vignettes.)
        self.settings.vignettes.passe_partout_cases = on;
        self.settings_a_ecrire = true;
        self.status = if on { "Passe-partout des cases : seules les vignettes se voient.".into() } else { "Passe-partout des cases retiré.".into() };
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
        // (Les traits partent, pas les calques : leurs noms, leur ordre et leur œil restent.)
        self.doc.annotations.traces.clear();
        self.vignettes.annot.tout_lacher();
        self.status = format!("{n} annotation{} effacée{} (Ctrl+Z les rétablit).", if n > 1 { "s" } else { "" }, if n > 1 { "s" } else { "" });
    }

    // ------------------------------------------------------------------ rattachement

    /// Après une opération qui a retiré des vignettes : leurs annotations passent à leur voisine (`before` : la planche d'avant).
    pub(crate) fn annot_rattacher_apres(&mut self, before: &BoardLayout) {
        if self.doc.annotations.is_empty() && self.doc.annotations.bitmap.morceaux.is_empty() {
            return;
        }
        let now = self.board_layout();
        self.doc.annotations.rattacher_manquantes(Some(before), &now);
    }

    /// À chaque image d'écran où la planche est à l'écran : si des vignettes ont changé sans que rien l'annonce (annuler, rétablir…),
    /// leurs annotations passent à la voisine (d'après la planche de l'image d'avant) ; les rangs de secours suivent.
    pub(crate) fn annot_suivre(&mut self, layout: &BoardLayout) {
        let ids = |l: &BoardLayout| l.cards.iter().map(|c| c.id).collect::<Vec<u64>>();
        // Un trait choisi qui n'existe plus (annuler, effacer, calque supprimé) n'est plus choisi ; un trait d'un autre calque non plus.
        {
            let calque = self.annot_calque_actif();
            let traces = &self.doc.annotations.traces;
            let a = &mut self.vignettes.annot;
            if !a.selection.is_empty() {
                a.selection.retain(|id| traces.iter().any(|t| t.id == *id && Some(t.calque) == calque));
                if a.selection.is_empty() {
                    a.boite = None;
                    a.pivot = None;
                }
            }
        }
        let a = &mut self.vignettes.annot;
        let same = a.derniere.as_ref().is_some_and(|d| d == layout);
        if same {
            return;
        }
        let previous = a.derniere.take();
        let cards_changed = previous.as_ref().is_none_or(|p| ids(p) != ids(layout));
        if cards_changed && !(self.doc.annotations.is_empty() && self.doc.annotations.bitmap.morceaux.is_empty()) {
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
    pub(crate) fn annot_geste(&mut self, layout: &BoardLayout, xf: Xf, events: &[egui::Event], latest: Option<Pos2>, down: bool, touch: bool, escape: bool, mods: Modifiers) {
        let outil = self.vignettes.annot.outil;
        if touch {
            self.vignettes.annot.stylet = true;
        }
        let Some(outil) = outil else {
            self.vignettes.annot.debut = None;
            self.vignettes.annot.en_cours = None;
            self.vignettes.annot.tout_lacher();
            return;
        };
        if outil != Outil::Lasso && (self.vignettes.annot.lasso.is_some() || self.vignettes.annot.boite.is_some() || !self.vignettes.annot.selection.is_empty()) {
            self.vignettes.annot.tout_lacher();
        }
        if let Some(pos) = self.vignettes.annot.debut.take() {
            self.vignettes.annot.gomme_prise = false;
            // Tracer, effacer ou choisir sur un calque caché : il se montre d'abord.
            self.annot_montrer_le_calque_vise();
            match outil {
                Outil::Stylo => {
                    let p = self.annot_point(xf.to_board(pos));
                    self.vignettes.annot.en_cours = Some(vec![p]);
                }
                Outil::Gomme => {
                    self.vignettes.annot.en_cours = Some(Vec::new());
                    self.annot_gommer(layout, xf.to_board(pos), xf.zoom);
                }
                Outil::Lasso => self.annot_lasso_appui(layout, xf, pos, mods),
            }
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
        if outil == Outil::Lasso {
            self.annot_lasso_geste(layout, xf, &points, down, escape, mods);
            return;
        }
        if self.vignettes.annot.en_cours.is_none() {
            return;
        }
        if escape {
            self.vignettes.annot.en_cours = None;
            self.status = "Trait abandonné.".into();
            return;
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
                Outil::Lasso => {}
            }
        }
        if !down {
            self.annot_finir(layout);
        }
    }

    /// La gomme passe en `p` (planche) : les traits touchés partent, avec une seule étape d'annulation pour le geste.
    fn annot_gommer(&mut self, layout: &BoardLayout, p: Pos2, zoom: f32) {
        let radius = (self.vignettes.annot.taille_gomme * 0.5).max(4.0 / zoom.max(0.05));
        // Le calque visé seulement : les traits des autres calques restent.
        let Some(calque) = self.annot_calque_actif() else { return };
        if !self.doc.annotations.touche_calque(layout, calque, p, radius) {
            return;
        }
        if !self.vignettes.annot.gomme_prise {
            self.vignettes.annot.gomme_prise = true;
            self.avec_etape("Effacer des annotations", |a| a.record_step());
        }
        let n = self.doc.annotations.effacer_en_calque(layout, calque, p, radius);
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
        let voulu = self.annot_calque_actif();
        // (L'étape d'annulation n'est posée que si le trait est gardé.)
        let snap = self.snapshot();
        if self.doc.annotations.ajouter_dans(layout, voulu, &trace, color) {
            // (Le premier trait crée le calque « Annotations » : il devient le calque visé.)
            if let Some(t) = self.doc.annotations.traces.last() {
                self.vignettes.annot.calque = t.calque;
            }
            self.avec_etape("Annotation", |a| {
                a.history.push(snap);
                a.gesture.reset();
                a.modified = true;
            });
        }
    }

    // ------------------------------------------------------------------ le lasso

    /// La boîte des traits choisis, droite, avec un peu d'air autour (le pivot tiré, s'il y en a un) ; rien s'il n'y a rien de choisi.
    fn annot_cadre(&self, layout: &BoardLayout, zoom: f32) -> Option<SelFrame> {
        let a = &self.vignettes.annot;
        if a.selection.is_empty() {
            return None;
        }
        let b = self.doc.annotations.bornes(layout, &a.selection)?;
        Some(SelFrame { rect0: b.expand(2.0 / zoom.max(0.05)), m: Affine::IDENTITY, pivot: a.pivot })
    }

    /// La boîte telle qu'on la voit : celle du geste en cours, ou la boîte au repos.
    fn annot_cadre_vu(&self, layout: &BoardLayout, zoom: f32) -> Option<SelFrame> {
        match &self.vignettes.annot.boite {
            Some(g) if g.handle != Handle::Pivot => Some(g.frame.then(&g.aff)),
            _ => self.annot_cadre(layout, zoom),
        }
    }

    /// L'appui du lasso en `pos` (écran) : sur la boîte des traits choisis, le geste de la boîte ; ailleurs, un lasso (ou un clic).
    fn annot_lasso_appui(&mut self, layout: &BoardLayout, xf: Xf, pos: Pos2, mods: Modifiers) {
        let board = xf.to_board(pos);
        if self.annot_calque_actif().is_none() {
            self.status = "Aucune annotation à choisir : trace d'abord avec le stylo d'annotation.".into();
        }
        if let Some(frame) = self.annot_cadre(layout, xf.zoom) {
            let quad = frame.corners().map(|q| xf.to_screen(q));
            let pivot = xf.to_screen(frame.pivot_world());
            // (Maj + appui dedans n'est pas un déplacement : il ajoute au choix, comme pour les traits d'un dessin.)
            if let Some(h) = selbox::hit_test(quad, pivot, pos, &Grabs::default()).filter(|h| !(*h == Handle::Move && mods.shift)) {
                self.vignettes.annot.boite = Some(GesteBoite { handle: h, start: board, frame, aff: Affine::IDENTITY, change: false });
                return;
            }
        }
        self.vignettes.annot.lasso = Some(LassoEnCours { points: vec![board], ajout: mods.shift });
    }

    /// Le geste du lasso à cette image d'écran : le lasso grandit, ou la boîte suit le pointeur ; au lâcher, il est posé. Échap abandonne
    /// le geste en cours (rien n'a bougé), puis lève la sélection.
    fn annot_lasso_geste(&mut self, layout: &BoardLayout, xf: Xf, points: &[Pos2], down: bool, escape: bool, mods: Modifiers) {
        let a = &mut self.vignettes.annot;
        if escape {
            if a.lasso.is_some() || a.boite.is_some() {
                a.lacher_le_geste();
                self.status = "Geste abandonné : rien n'a bougé.".into();
            } else if !a.selection.is_empty() {
                a.tout_lacher();
                self.status = "Sélection levée.".into();
            }
            return;
        }
        if a.boite.is_some() {
            for p in points {
                self.annot_boite_vers(xf, *p, mods);
            }
            if !down {
                self.annot_boite_finir(layout);
            }
            return;
        }
        if let Some(l) = a.lasso.as_mut() {
            let eps = 2.0 / xf.zoom.max(0.05);
            for p in points {
                let b = xf.to_board(*p);
                if l.points.last().is_none_or(|q| (*q - b).length() >= eps) && l.points.len() < LASSO_MAX {
                    l.points.push(b);
                }
            }
            if !down {
                self.annot_lasso_finir(layout, xf);
            }
        }
    }

    /// Le pointeur est en `p` (écran) pendant un geste de la boîte : la transformation du geste (`selbox::gesture`), ou le pivot tiré.
    fn annot_boite_vers(&mut self, xf: Xf, p: Pos2, mods: Modifiers) {
        let board = xf.to_board(p);
        let m = Mods::with(mods.shift, mods.ctrl || mods.command, self.keep_proportions, self.settings.point_fixe, self.settings.rotation_autour);
        let Some(g) = self.vignettes.annot.boite.as_mut() else { return };
        if g.handle == Handle::Pivot {
            let local = selbox::move_pivot(&g.frame, g.start, board, PIVOT_SNAP, xf.zoom);
            self.vignettes.annot.pivot = local;
            return;
        }
        g.aff = selbox::gesture(g.handle, &g.frame, g.start, board, m);
        g.change = true;
        self.status = crate::seltools::gesture_text(g.handle, &g.frame.then(&g.aff).params());
    }

    /// Le geste de la boîte est lâché : les traits reçoivent la transformation, une étape d'annulation au nom du geste.
    fn annot_boite_finir(&mut self, layout: &BoardLayout) {
        let Some(g) = self.vignettes.annot.boite.take() else { return };
        if g.handle == Handle::Pivot || !g.change || g.aff == Affine::IDENTITY {
            return;
        }
        let ids = self.vignettes.annot.selection.clone();
        let nom = format!("{} des annotations", g.handle.step_name());
        self.avec_etape(&nom, |a| a.record_step());
        let n = self.doc.annotations.transformer(layout, &ids, &g.aff);
        // (Les traits ont bougé : la boîte repart de leur nouvelle place, le pivot au centre.)
        self.vignettes.annot.pivot = None;
        self.status = format!("{} : {n} annotation{}.", g.handle.step_name(), if n > 1 { "s" } else { "" });
    }

    /// Le lasso est lâché : les traits du calque visé qu'il entoure sont choisis ; un lasso minuscule est un clic, qui choisit le trait dessous
    /// (Maj : l'ajoute ou le retire). Un clic dans le vide lève le choix.
    fn annot_lasso_finir(&mut self, layout: &BoardLayout, xf: Xf) {
        let Some(l) = self.vignettes.annot.lasso.take() else { return };
        let Some(calque) = self.annot_calque_actif() else { return };
        let zoom = xf.zoom.max(0.05);
        let etendue = Rect::from_points(&l.points).size().max_elem() * zoom;
        let clic = l.points.len() < 3 || etendue < CLIC_PX;
        let trouves: Vec<u64> = if clic {
            self.doc.annotations.sous_le_point(layout, calque, l.points[0], PRISE_PX / zoom).into_iter().collect()
        } else {
            self.doc.annotations.dans_le_lasso(layout, calque, &l.points)
        };
        let a = &mut self.vignettes.annot;
        if l.ajout {
            for id in trouves {
                match a.selection.iter().position(|s| *s == id) {
                    Some(k) if clic => {
                        a.selection.remove(k);
                    }
                    Some(_) => {}
                    None => a.selection.push(id),
                }
            }
        } else {
            a.selection = trouves;
        }
        a.pivot = None;
        let n = a.selection.len();
        self.status = match n {
            0 => "Rien dans le lasso : aucune annotation choisie.".into(),
            1 => "1 annotation choisie : glisse-la, ou prends un coin pour l'agrandir ou la tourner.".into(),
            n => format!("{n} annotations choisies : glisse-les, ou prends un coin pour les agrandir ou les tourner."),
        };
    }

    // ------------------------------------------------------------------ dessin

    /// Les annotations sur la planche, par-dessus tout : celles du projet, puis le trait en cours, puis le curseur de l'outil.
    pub(crate) fn annot_peindre(&self, painter: &egui::Painter, view: Rect, xf: Xf, layout: &BoardLayout, survol: Option<Pos2>) {
        let a = &self.vignettes.annot;
        if a.visible {
            // Calque par calque, du plus bas au plus haut ; un calque caché n'est pas peint. Pendant le geste de la boîte, les traits choisis se
            // voient là où le geste les mène (rien n'est changé avant le lâcher : un aperçu, pas de rejeu).
            let apercu = a.boite.as_ref().filter(|g| g.change && g.handle != Handle::Pivot).map(|g| g.aff);
            let traces = &self.doc.annotations.traces;
            for i in self.doc.annotations.ordre_de_peinture() {
                let t = &traces[i];
                // (Le calque du dessin hors des cases est peint sous les cases, hors_cadres_peindre.)
                if self.doc.annotations.calque(t.calque).is_some_and(|c| c.planche) {
                    continue;
                }
                let Some(points) = self.doc.annotations.points_planche(layout, i) else { continue };
                match apercu {
                    Some(aff) if a.selection.contains(&t.id) => crate::boardui::paint_note(painter, view, xf, &ann::transformee(&points, &aff), t.color),
                    _ => crate::boardui::paint_note(painter, view, xf, &points, t.color),
                }
            }
        }
        if let (Some(Outil::Stylo), Some(trace)) = (a.outil, a.en_cours.as_ref()) {
            crate::boardui::paint_note(painter, view, xf, trace, a.couleur);
        }
        if !self.board_shown() {
            return;
        }
        if a.outil == Some(Outil::Lasso) {
            self.annot_peindre_lasso(painter, xf, layout, survol);
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
                Outil::Lasso => {}
            }
        }
    }

    /// Le lasso en cours (une ligne fine et des points ambrés, comme celui d'un dessin) et la boîte des traits choisis : le cadre, les huit
    /// poignées carrées et le pivot rond (les mêmes que celles de l'outil Sélection, `seltools.rs`).
    fn annot_peindre_lasso(&self, painter: &egui::Painter, xf: Xf, layout: &BoardLayout, survol: Option<Pos2>) {
        use crate::theme::{SELECTION_BOITE as ACCENT, SELECTION_POIGNEE_FOND};
        let a = &self.vignettes.annot;
        if let Some(l) = &a.lasso {
            let pts: Vec<Pos2> = l.points.iter().map(|p| xf.to_screen(*p)).collect();
            if let Some(shape) = crate::contours::lasso_shape(&pts) {
                painter.add(shape);
            }
        }
        let Some(frame) = self.annot_cadre_vu(layout, xf.zoom) else { return };
        let quad = frame.corners().map(|q| xf.to_screen(q));
        let pivot = xf.to_screen(frame.pivot_world());
        if !quad.iter().chain([&pivot]).all(|p| p.is_finite()) {
            return;
        }
        let edge = Stroke::new(1.0_f32, ACCENT);
        for k in 0..4 {
            painter.line_segment([quad[k], quad[(k + 1) % 4]], edge);
        }
        let hover = a.boite.as_ref().map(|g| g.handle).or_else(|| survol.and_then(|p| selbox::hit_test(quad, pivot, p, &Grabs::default())));
        let lit = |h: Handle| hover == Some(h);
        let ppp = painter.ctx().pixels_per_point();
        let poignee = |c: Pos2, on: bool| {
            let c = selbox::snap_handle_center(c, ppp);
            let fill = if on { ACCENT } else { SELECTION_POIGNEE_FOND };
            painter.rect(selbox::handle_rect(c, selbox::HANDLE_SIDE), 0.0, fill, Stroke::new(1.0_f32, ACCENT), egui::StrokeKind::Middle);
        };
        for (k, c) in quad.iter().enumerate() {
            poignee(*c, lit(Handle::Corner(k)));
        }
        for k in 0..4 {
            poignee(quad[k] + (quad[(k + 1) % 4] - quad[k]) * 0.5, lit(Handle::Edge(k)));
        }
        if lit(Handle::Pivot) {
            painter.circle_filled(pivot, selbox::PIVOT_RADIUS, ACCENT.gamma_multiply(0.3));
        }
        painter.circle_stroke(pivot, selbox::PIVOT_RADIUS, Stroke::new(1.0_f32, ACCENT));
    }

    /// Le curseur sur la planche pendant un outil d'annotation : une croix ; avec le lasso, celui de la partie de la boîte sous le pointeur.
    pub(crate) fn annot_curseur(&self, ctx: &egui::Context, xf: Xf, layout: &BoardLayout, over: Option<Pos2>) {
        use egui::CursorIcon;
        let a = &self.vignettes.annot;
        let mut icone = CursorIcon::Crosshair;
        if a.outil == Some(Outil::Lasso) {
            if let Some(frame) = self.annot_cadre_vu(layout, xf.zoom) {
                let quad = frame.corners().map(|q| xf.to_screen(q));
                let pivot = xf.to_screen(frame.pivot_world());
                let handle = a.boite.as_ref().map(|g| g.handle).or_else(|| over.and_then(|p| selbox::hit_test(quad, pivot, p, &Grabs::default())));
                let resize = |h: Handle| match selbox::handle_direction(quad, h).map(selbox::resize_cursor_axis) {
                    Some(0) => CursorIcon::ResizeHorizontal,
                    Some(1) => CursorIcon::ResizeNwSe,
                    Some(2) => CursorIcon::ResizeVertical,
                    Some(_) => CursorIcon::ResizeNeSw,
                    None => CursorIcon::Crosshair,
                };
                icone = match handle {
                    Some(Handle::Move) => CursorIcon::Move,
                    Some(h @ (Handle::Corner(_) | Handle::Edge(_))) => resize(h),
                    Some(Handle::Skew(k)) => if k % 2 == 0 { CursorIcon::ResizeHorizontal } else { CursorIcon::ResizeVertical },
                    Some(Handle::Rotate(_)) => CursorIcon::Grab,
                    Some(Handle::Pivot) | None => CursorIcon::Crosshair,
                };
            }
        }
        ctx.set_cursor_icon(icone);
    }

    /// Le passe-partout de la planche (si on l'a allumé) : tout ce qui est hors de l'intérieur des vignettes, par-dessus tout.
    pub(crate) fn marie_peindre(&self, painter: &egui::Painter, view: Rect, xf: Xf, layout: &BoardLayout) {
        if !self.vignettes.annot.marie {
            return;
        }
        let holes: Vec<Rect> = layout.cards.iter().chain(&layout.annulees).map(|c| xf.rect(c.thumb)).collect();
        // L'opacité du passe-partout des cases (barre du bas de la planche, Préférences > Vignettes) : 100 %, opaque (Anthony, 2026-10-30).
        let [r, g, b] = self.vignettes.annot.marie_couleur;
        let alpha = (f32::from(self.settings.vignettes.passe_partout_cases_opacite.min(100)) * 2.55).round() as u8;
        let color = Color32::from_rgba_unmultiplied(r, g, b, alpha);
        // Un plan à mouvement de caméra s'ouvre sur tout ce que sa caméra balaie (A, B, C…, et l'espace entre eux), dans son vrai dessin :
        // pas seulement sur la case A (Anthony, 2026-11-06).
        let ouvertures: Vec<Vec<Pos2>> = layout.moves.iter().flat_map(|m| m.balayage.iter()).map(|p| p.iter().map(|q| xf.to_screen(*q)).collect()).collect();
        if ouvertures.is_empty() {
            for r in crate::marie::hors_des_cases(view, &holes) {
                painter.rect_filled(r, 0.0, color);
            }
        } else {
            painter.add(crate::viewbar::mesh_of_pieces(&crate::marie::couverture(view, &holes, &ouvertures), color));
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

    /// Les trois boutons, sans séparateur (dans une ligne, ou sous le trait de la colonne).
    pub(crate) fn annot_outils_ligne(&mut self, ui: &mut egui::Ui, taille: [f32; 2], icone: f32) {
        for outil in Outil::TOUS {
            let on = self.vignettes.annot.outil == Some(outil);
            let mut texte = RichText::new(outil.icone()).size(icone);
            if !on {
                texte = texte.color(couleur32(self.vignettes.annot.couleur));
            }
            let resp = ui.add_sized(taille, egui::SelectableLabel::new(on, texte));
            if on {
                // (La fenêtre de l'aide des annotations se pose à côté de l'outil pris : `aideflottante.rs`.)
                crate::aideflottante::retenir(ui.ctx(), crate::aideflottante::ANNOTATION, resp.rect);
            }
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
        // (Ce que sont les annotations, et le geste du lasso : l'animation « Annotations », `aidevignettes.rs`, partie 153.)
        ui.add_space(4.0);
        match outil {
            Outil::Stylo => {
                ui.horizontal(|ui| {
                    bulles::etiquette(ui, ph::PALETTE, "Couleur du stylo", "La couleur des annotations à venir (celles déjà tracées ne changent pas).");
                    crate::champcouleur::srgb(ui, &mut self.vignettes.annot.couleur);
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
            Outil::Lasso => {
                let n = self.vignettes.annot.selection.len();
                ui.label(RichText::new(if n == 0 { "Rien de choisi.".to_string() } else { format!("{n} annotation{} choisie{}.", if n > 1 { "s" } else { "" }, if n > 1 { "s" } else { "" }) }).small());
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
        // Les annotations dans un sous-menu : le menu du fond de la planche reste à une dizaine d'entrées
        // (simplification des barres, 2026-10-21).
        ui.menu_button("Annotations", |ui| {
            self.action_item(ui, "Stylo d'annotation", "vignettes.annotation_stylo");
            self.action_item(ui, "Gomme d'annotation", "vignettes.annotation_gomme");
            self.action_item(ui, "Lasso d'annotation", "vignettes.annotation_lasso");
            self.action_item(ui, "Nouveau calque d'annotation", "vignettes.annotation_calque_nouveau");
            let visible = self.vignettes.annot.visible;
            self.action_item(ui, if visible { "Cacher les annotations" } else { "Montrer les annotations" }, "vignettes.annotations_afficher");
            if ui.add_enabled(!self.doc.annotations.is_empty(), egui::Button::new("Effacer les annotations…")).clicked() {
                ui.close_menu();
                self.annot_demander_effacement();
            }
            let marie = self.vignettes.annot.marie;
            self.action_item(ui, if marie { "Retirer la marie-louise" } else { "Marie-louise (ne montrer que les vignettes)" }, "vignettes.marie_louise");
        });
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
