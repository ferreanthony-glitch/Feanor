//! Brosses dédiées, côté interface (`personnages.rs`, `persofond.rs`) : les pastilles de la barre du haut, le panneau « Brosses dédiées », la
//! fenêtre « Modifier », et ce que fait le choix d'une brosse.
//!
//! # Une seule sorte de chose : la brosse dédiée (Anthony, 2026-10-29, §70 de `docs/INTERFACE.md`)
//!
//! Une brosse dédiée a un nom, un **type** (`personnages::Type`), une couleur de trait et une couleur d'aplat (ou pas d'aplat). Trois types :
//!
//! - **Personnage** (du projet, annulable) : son premier trait crée un calque vectoriel à son nom, avec son aplat (`calquespastilles.rs`) ;
//! - **Classique** (du poste, `Settings::brosses_classiques`) : elle dessine dans le calque courant, sans rien créer ; sa couleur d'aplat
//!   devient celle de l'aplat du calque courant ordinaire (partie 33) ;
//! - **Décor** (du poste, `Settings::brosses_decor`) : elle dessine dans le calque Décor du plan, créé au premier trait (`decorui.rs`) ; elle
//!   a de plus un pinceau (crayon, feutre ou pinceau plein) et une taille.
//!
//! D'origine : 3 Personnage (Perso 1 à 3, couleurs), 3 Classique (Gris 1 à 3), 3 Décor (Décor 1 à 3, gris plus clairs). Il n'y a plus de
//! pastille « classique » (⊘) : **un clic sur la brosse choisie la lâche**, et la couleur est libre, dans le calque courant.
//!
//! # La barre (Storyboard, Vignettes, Animation quand elle la montre)
//!
//! Trois groupes, séparés d'un petit trait, de gauche à droite : les **Personnages** (ronds de taille normale), les **Classiques** (petits
//! ronds), les **Décors** (petits carrés arrondis, aux couleurs de leur brosse ; seulement là où le décor est proposé, `decor_offert`) ;
//! puis le « + » (un petit menu : brosse personnage, classique ou décor) et l'icône du panneau. Un clic choisit : la couleur du trait devient
//! celle de la brosse et l'aplat sous le trait s'allume avec le sien (un nouveau trait gagne son aplat tout de suite, `persofond.rs`) ; un
//! clic sur la brosse choisie la lâche. Clic droit : modifier, dupliquer, supprimer, monter, descendre. La barre se montre ou se cache par
//! espace de travail (`Layout::character_bar`, `Espace::barre_personnages_par_defaut`). Les bulles disent le type et ce que fait la brosse.
//! - **Le panneau « Brosses dédiées »** : trois sections (Personnages, Classiques, Décors), chacune avec son « + », ses lignes (nom, couleur
//!   du trait, aplat oui ou non, couleur d'aplat, et pour un Décor le pinceau et la taille, supprimer ; la poignée range dans la section)
//!   et son « ↺ » (les valeurs d'origine de ce type).
//! - **Choisir à la main une autre couleur** (panneau Couleur, pipette, nuancier, échange des deux couleurs) lâche la brosse : la barre
//!   n'en montre plus aucune de choisie.
//!
//! Les brosses Personnage sont celles du projet (annulables) ; les Classiques et les Décors sont un réglage de l'application.

use eframe::egui::{self, pos2, vec2, Color32, Id, Rect, Response, RichText, Sense, Stroke, StrokeKind};
use egui_phosphor::regular as ph;

use crate::app::{App, Tool};
use crate::dock::Pane;
use crate::document::{CelId, Strate};
use crate::history::Edit;
use crate::personnages::{self, Brosse, Personnage, Type};
use crate::persofond::{Fil, Fini, Travail};
use crate::raster::Tile;

/// Diamètre d'une pastille de personnage.
pub(crate) const PASTILLE: f32 = 20.0;
/// Côté d'une case de la colonne (une pastille et son halo).
const CASE_H: f32 = PASTILLE + 6.0;
/// Largeur de la colonne des pastilles : une case, et un souffle de chaque côté.
pub(crate) const COLONNE_L: f32 = CASE_H + 2.0;
/// Écart de la colonne au bord droit du dessin.
const COLONNE_BORD: f32 = 6.0;

/// Une case de la rangée des pastilles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Case {
    /// Une brosse dédiée : un personnage, une classique ou un décor.
    Choix(Choix),
    /// « + » : le menu des trois types.
    Plus,
    /// Une ligne du menu du « + » (visible quand il est ouvert).
    Ajouter(Type),
    /// L'icône du panneau Brosses dédiées.
    Panneau,
}

/// Un petit trait entre deux groupes de pastilles : horizontal dans une colonne (la planche des Vignettes, d'avant), vertical dans la
/// rangée de la barre au-dessus du dessin.
fn trait_de_colonne(ui: &mut egui::Ui) {
    let stroke = Stroke::new(1.0_f32, crate::theme::bord());
    if ui.layout().main_dir().is_horizontal() {
        let (rect, _) = ui.allocate_exact_size(vec2(5.0, COLONNE_L - 12.0), Sense::hover());
        ui.painter().vline(rect.center().x, rect.y_range(), stroke);
    } else {
        let (rect, _) = ui.allocate_exact_size(vec2(COLONNE_L - 12.0, 5.0), Sense::hover());
        ui.painter().hline(rect.x_range(), rect.center().y, stroke);
    }
}

/// Ce qui est choisi dans la barre : une brosse dédiée. Personnage : son numéro dans le projet ; Classique et Décor : leur rang dans leur
/// liste du poste.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Choix {
    Personnage(u64),
    Classique(usize),
    Decor(usize),
}

impl Choix {
    pub(crate) fn type_(self) -> Type {
        match self {
            Choix::Personnage(_) => Type::Personnage,
            Choix::Classique(_) => Type::Classique,
            Choix::Decor(_) => Type::Decor,
        }
    }
}

/// L'état des brosses dédiées d'un projet (rangé avec l'onglet).
#[derive(Default)]
pub(crate) struct Etat {
    /// La brosse dédiée qui dessine ; rien : la couleur est libre.
    pub actif: Option<Choix>,
    /// La couleur de trait posée quand on l'a choisie : si la couleur courante en diffère, quelqu'un a choisi une
    /// autre couleur à la main et la brosse est lâchée.
    pub posee: [u8; 3],
    /// La fenêtre « Modifier » est ouverte sur cette brosse.
    pub edition: Option<Choix>,
    /// Le fil des fonds de trait (`persofond.rs`), lancé au premier trait.
    pub(crate) fil: Option<Fil>,
    /// Où sont les cases de la rangée des pastilles à l'écran, à la dernière image (les essais y cliquent).
    pub(crate) cases: Vec<(Rect, Case)>,
    /// Ce que le choix d'une brosse Décor a pris à l'utilisateur et lui rendra quand il la lâche : outil, couleur, calque.
    pub(crate) decor_avant: Option<crate::decorui::Avant>,
    /// La couleur de trait posée par le dernier choix (ou suivi) de la brosse Décor, pour voir quand le panneau la change.
    pub(crate) decor_posee: Option<crate::decorui::Posee>,
    /// Le pinceau, le crayon et l'outil de l'utilisateur, mis de côté tant qu'une brosse **spécifique** est choisie (`brossesdediees.rs`).
    pub(crate) pris: Option<crate::brossesdediees::Pris>,
    /// Ce que la brosse spécifique choisie a posé (outil, réglage), pour voir quand l'éditeur ou l'utilisateur le change.
    pub(crate) pose: Option<crate::brossesdediees::Pose>,
    /// Le changement de couleur d'aplat d'un personnage, en attente ou en route sur ses calques (`aplatperso.rs`).
    pub(crate) aplat: crate::aplatperso::Etat,
    /// Le dernier calque utilisé (dessiné ou choisi) de chaque personnage dans chaque plan : (numéro du plan, numéro du personnage) vers
    /// le numéro du calque. En mémoire seulement, jamais dans le fichier (`calquespastilles.rs`).
    pub(crate) dernier_calque: std::collections::HashMap<(u64, u64), u64>,
}

fn to_color(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Les tuiles d'avant d'une étape d'annulation de dessin (le trait qui vient de finir), où qu'elles soient.
pub(crate) fn tuiles_de(edit: Option<&Edit>) -> Option<Vec<(usize, Option<Tile>)>> {
    match edit? {
        Edit::Tiles { tiles, .. } => Some(tiles.clone()),
        Edit::Group(edits) => edits.iter().find_map(|e| tuiles_de(Some(e))),
        _ => None,
    }
}

/// Une pastille : l'anneau est la couleur du trait, l'intérieur celle de l'aplat (rayé s'il n'y en a pas) ; un halo
/// de la couleur d'accent quand elle est choisie.
fn pastille(ui: &mut egui::Ui, ligne: [u8; 3], fond: Option<[u8; 3]>, choisie: bool, specifique: Option<bool>) -> Response {
    pastille_taille(ui, ligne, fond, choisie, PASTILLE, specifique)
}

/// Diamètre d'une brosse Classique dans la barre (Anthony, 2026-10-28 : il prenait les couleurs neutres d'un projet neuf pour des
/// personnages et attendait leur calque) : nettement plus petite qu'un personnage, pour qu'on ne les confonde plus.
const PASTILLE_CLASSIQUE: f32 = 12.0;

/// Une pastille de diamètre `d`, dans une case de la hauteur d'une pastille de personnage (la rangée reste alignée). `specifique` : la brosse a
/// sa propre brosse (§107), et un petit signe au coin le dit (un trait fin : le crayon, un trait épais : le pinceau) ; rien : elle suit le pinceau.
fn pastille_taille(ui: &mut egui::Ui, ligne: [u8; 3], fond: Option<[u8; 3]>, choisie: bool, d: f32, specifique: Option<bool>) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(d + 6.0, PASTILLE + 6.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let c = rect.center();
        let r = d / 2.0;
        let p = ui.painter();
        if choisie {
            p.circle_filled(c, r + 3.0, crate::theme::accent_texte());
        } else if resp.hovered() {
            p.circle_stroke(c, r + 2.0, Stroke::new(1.5_f32, crate::theme::bord()));
        }
        p.circle_filled(c, r, to_color(ligne));
        let dedans = r - (3.5 * d / PASTILLE).max(2.0);
        match fond {
            Some(f) => {
                p.circle_filled(c, dedans, to_color(f));
            }
            None => {
                p.circle_filled(c, dedans, crate::theme::entete());
                let d = dedans * 0.7;
                p.line_segment([c + vec2(-d, d), c + vec2(d, -d)], Stroke::new(1.5_f32, crate::theme::texte_doux()));
            }
        }
        p.circle_stroke(c, r, Stroke::new(1.0_f32, Color32::from_black_alpha(90)));
        if let Some(crayon) = specifique {
            // Le signe de la brosse spécifique, au coin bas droit : un petit rond sur le fond de la barre, avec un trait (fin : crayon, épais : pinceau).
            let k = if d < PASTILLE { 3.5 } else { 4.5 };
            let coin = c + vec2(r * 0.8, r * 0.8);
            p.circle_filled(coin, k + 1.0, crate::theme::entete());
            p.circle_stroke(coin, k + 1.0, Stroke::new(1.0_f32, crate::theme::bord()));
            let a = k * 0.55;
            p.line_segment([coin + vec2(-a, a), coin + vec2(a, -a)], Stroke::new(if crayon { 1.0_f32 } else { 2.2_f32 }, crate::theme::texte()));
        }
    }
    resp
}

/// Une pastille de brosse Décor : un carré arrondi (l'anneau est la couleur du trait, l'intérieur celle de l'aplat), avec, si elle a sa propre
/// brosse (§107), le signe de son outil tracé de la couleur du trait (un trait fin : le crayon, un trait épais : le pinceau). Des carrés, pas des ronds, pour qu'on ne les prenne pas pour des personnages.
fn pastille_carree(ui: &mut egui::Ui, ligne: [u8; 3], fond: Option<[u8; 3]>, choisie: bool, specifique: Option<bool>) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(PASTILLE + 6.0, PASTILLE + 6.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let carre = Rect::from_center_size(rect.center(), vec2(PASTILLE, PASTILLE));
        let p = ui.painter();
        if choisie {
            p.rect_filled(carre.expand(3.0), 8.0, crate::theme::accent_texte());
        } else if resp.hovered() {
            p.rect_stroke(carre.expand(2.0), 7.0, Stroke::new(1.5_f32, crate::theme::bord()), StrokeKind::Outside);
        }
        let encre = to_color(ligne);
        p.rect_filled(carre, 6.0, encre);
        let dedans = carre.shrink(3.5);
        p.rect_filled(dedans, 3.5, fond.map_or_else(crate::theme::entete, to_color));
        let (a, b) = (dedans.left_bottom() + vec2(2.5, -2.5), dedans.right_top() + vec2(-2.5, 2.5));
        // (Le signe n'est tracé que pour une brosse spécifique : une brosse qui suit le pinceau n'a pas de pinceau à elle à montrer.)
        match specifique {
            Some(true) => {
                p.line_segment([a, b], Stroke::new(1.5_f32, encre));
            }
            Some(false) => {
                p.line_segment([a, b], Stroke::new(4.0_f32, encre));
            }
            None => {}
        }
        p.rect_stroke(carre, 6.0, Stroke::new(1.0_f32, Color32::from_black_alpha(90)), StrokeKind::Inside);
    }
    resp
}

/// Le texte de la bulle d'une pastille : le type, ce que fait la brosse, ses couleurs, puis ce que font le clic et le clic droit.
/// `pinceau` : ce qui dessine, en mots (« Suit le pinceau » ou le nom de la brosse spécifique ; `App::dediee_etiquette`).
pub(crate) fn description(t: Type, ligne: [u8; 3], fond: Option<[u8; 3]>, choisie: bool, pinceau: &str) -> String {
    let fond = match fond {
        Some(f) => format!("aplat {}", crate::couleur::hex(f)),
        None => "sans aplat".to_string(),
    };
    let pinceau = format!(" Dessine avec : {pinceau}.");
    let clic = if choisie { "Clic : lâcher la brosse (retour à la couleur libre, dans le calque courant)" } else { "Clic : dessiner avec" };
    format!("Brosse {} : {}. Trait {} · {fond}.{pinceau} {clic} ; clic droit : modifier, dupliquer, supprimer, monter, descendre.", t.nom().to_lowercase(), t.effet(), crate::couleur::hex(ligne))
}

/// L'action qui ajoute une brosse du type `t`.
fn action_ajouter(t: Type) -> &'static str {
    match t {
        Type::Personnage => "perso.ajouter",
        Type::Classique => "perso.ajouter_classique",
        Type::Decor => "perso.ajouter_decor",
    }
}

impl App {
    // ------------------------------------------------------------------------------ ce qui est choisi

    /// Les couleurs de ce qui est choisi : (trait, aplat).
    pub(crate) fn perso_couleurs(&self, c: Choix) -> Option<([u8; 3], Option<[u8; 3]>)> {
        match c {
            Choix::Personnage(id) => self.doc.personnages.iter().find(|p| p.id == id).map(|p| (p.line, p.fill)),
            Choix::Classique(i) => self.settings.brosses_classiques.get(i).map(|n| (n.line, n.fill)),
            Choix::Decor(i) => self.settings.brosses_decor.get(i).map(|n| (n.line, n.fill)),
        }
    }

    /// Le nom de ce qui est choisi.
    pub(crate) fn perso_nom(&self, c: Choix) -> Option<String> {
        match c {
            Choix::Personnage(id) => self.doc.personnages.iter().find(|p| p.id == id).map(|p| p.name.clone()),
            Choix::Classique(i) => self.settings.brosses_classiques.get(i).map(|n| n.name.clone()),
            Choix::Decor(i) => self.settings.brosses_decor.get(i).map(|n| n.name.clone()),
        }
    }

    /// Les couleurs de la brosse qui dessine, s'il y en a une.
    pub(crate) fn perso_couleurs_actives(&self) -> Option<([u8; 3], Option<[u8; 3]>)> {
        self.perso.actif.and_then(|c| self.perso_couleurs(c))
    }

    /// La couleur d'aplat de la brosse qui dessine, sinon `autre` (le pot de peinture de la strate Aplat).
    pub(crate) fn perso_fond_ou(&self, autre: [u8; 3]) -> [u8; 3] {
        self.perso_couleurs_actives().and_then(|(_, fond)| fond).unwrap_or(autre)
    }

    /// La liste du poste du type `t` (Classique ou Décor) ; rien pour les Personnages (ceux du projet).
    fn liste_poste(&self, t: Type) -> Option<&Vec<Brosse>> {
        match t {
            Type::Personnage => None,
            Type::Classique => Some(&self.settings.brosses_classiques),
            Type::Decor => Some(&self.settings.brosses_decor),
        }
    }

    fn liste_poste_mut(&mut self, t: Type) -> Option<&mut Vec<Brosse>> {
        match t {
            Type::Personnage => None,
            Type::Classique => Some(&mut self.settings.brosses_classiques),
            Type::Decor => Some(&mut self.settings.brosses_decor),
        }
    }

    /// Les brosses du type `t`, dans l'ordre : leurs choix.
    pub(crate) fn perso_du_type(&self, t: Type) -> Vec<Choix> {
        match t {
            Type::Personnage => self.doc.personnages.iter().map(|p| Choix::Personnage(p.id)).collect(),
            Type::Classique => (0..self.settings.brosses_classiques.len()).map(Choix::Classique).collect(),
            Type::Decor => (0..self.settings.brosses_decor.len()).map(Choix::Decor).collect(),
        }
    }

    /// Tous les choix de la barre, dans l'ordre : les personnages, les classiques, puis les décors (là où le décor est proposé).
    /// (Aussi les pastilles du second anneau, `anneauui.rs`.)
    pub(crate) fn perso_tous(&self) -> Vec<Choix> {
        let mut tous = self.perso_du_type(Type::Personnage);
        tous.extend(self.perso_du_type(Type::Classique));
        if self.decor_offert() {
            tous.extend(self.perso_du_type(Type::Decor));
        }
        tous
    }

    /// Choisit une brosse dédiée : la couleur du trait devient la sienne, et ses traits auront son aplat. Un outil qui ne dessine pas cède
    /// la place au pinceau. Une brosse Décor prend en plus l'outil, le pinceau et la taille qu'il lui faut (`decorui.rs`).
    pub(crate) fn perso_choisir(&mut self, c: Choix) {
        let Some((ligne, fond)) = self.perso_couleurs(c) else { return };
        if let Choix::Decor(i) = c {
            self.decor_choisir(i);
            return;
        }
        // Un personnage ou une classique remplace la brosse Décor : l'outil, le pinceau, le calque d'avant reviennent d'abord.
        self.decor_lacher(true);
        self.end_stroke();
        // Une brosse dessine le trait ET son aplat (comme prévu) : on revient donc au trait si la sous-ligne d'un aplat
        // était choisie. (Cliquer ensuite sur l'aplat le reprend seul, avec la couleur d'aplat de la brosse.)
        self.aplat_cible = None;
        if !matches!(self.tool, Tool::Brush | Tool::Pencil | Tool::Fill | Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Polyline | Tool::Text) {
            self.set_tool(Tool::Brush);
        }
        self.set_front(ligne);
        self.perso.actif = Some(c);
        self.perso.posee = ligne;
        // Un personnage : son calque devient le calque courant (celui-ci s'il est déjà de ce personnage, sinon le dernier utilisé dans ce
        // plan, sinon le plus haut ; `calquespastilles.rs`).
        if let Choix::Personnage(id) = c {
            self.perso_rendre_son_calque(id);
        }
        // Une brosse spécifique prend son pinceau (ou son crayon) ; une brosse qui suit le pinceau rend celui de l'utilisateur, s'il était de côté.
        self.dediee_suivre(true);
        let nom = self.perso_nom(c).unwrap_or_default();
        self.status = match (c, fond) {
            // (Le calque du personnage naît au premier trait, pas au clic : choix d'Anthony, 2026-10-28.)
            (Choix::Personnage(_), Some(_)) => format!("« {nom} » : brosse Personnage, le trait et l'aplat de ce personnage, dans son calque (créé au premier trait s'il n'existe pas encore)."),
            (Choix::Personnage(_), None) => format!("« {nom} » : brosse Personnage, le trait de ce personnage, sans aplat, dans son calque (créé au premier trait s'il n'existe pas encore)."),
            _ => format!("« {nom} » : brosse Classique, elle dessine dans le calque courant, sans créer de calque."),
        };
    }

    /// Lâche la brosse : la couleur reste, mais elle est libre, dans le calque courant.
    pub(crate) fn perso_lacher(&mut self) {
        if self.decor_actif().is_some() {
            self.decor_lacher(true);
            self.status = "Couleur libre : plus de brosse dédiée choisie.".into();
            return;
        }
        if self.perso.actif.take().is_some() {
            self.status = "Couleur libre : plus de brosse dédiée choisie.".into();
        }
        // (Une brosse spécifique lâchée rend le pinceau de l'utilisateur tel qu'il était.)
        self.dediee_rendre();
    }

    /// La brosse suivante (ou précédente) de la barre, en tournant.
    fn perso_voisin(&mut self, delta: isize) {
        let tous = self.perso_tous();
        if tous.is_empty() {
            self.status = "Aucune brosse dédiée : ajoutes-en une (+ dans la barre des pastilles).".into();
            return;
        }
        let rang = self.perso.actif.and_then(|a| tous.iter().position(|c| *c == a));
        let next = match rang {
            Some(r) => (r as isize + delta).rem_euclid(tous.len() as isize) as usize,
            None if delta >= 0 => 0,
            None => tous.len() - 1,
        };
        self.perso_choisir(tous[next]);
    }

    // -------------------------------------------------------------------------- les changements

    /// Enregistre l'étape d'annulation d'un changement de personnage (à appeler AVANT le changement).
    pub(crate) fn perso_etape(&mut self, nom: &str) {
        let avant = self.history.context.replace(nom.to_string());
        self.record_step();
        self.history.context = avant;
    }

    /// Suit un champ ou un sélecteur de couleur d'un personnage (`App::edited`, une étape par geste, nommée) ; une
    /// classique ou un décor, eux, sont un réglage de l'application, sans annulation.
    pub(crate) fn perso_edited(&mut self, resp: &Response, c: Choix) -> bool {
        match c {
            Choix::Personnage(_) => {
                let avant = self.history.context.replace("Modifier le personnage".to_string());
                let fait = self.edited(resp);
                self.history.context = avant;
                fait
            }
            Choix::Classique(_) | Choix::Decor(_) => resp.changed(),
        }
    }

    /// Le nom, le trait et l'aplat de ce qui est choisi.
    fn perso_lire(&self, c: Choix) -> Option<(String, [u8; 3], Option<[u8; 3]>)> {
        let nom = self.perso_nom(c)?;
        let (ligne, fond) = self.perso_couleurs(c)?;
        Some((nom, ligne, fond))
    }

    /// Pose ces valeurs sur ce personnage (le projet change) ou cette brosse du poste (les réglages changent).
    pub(crate) fn perso_ecrire(&mut self, c: Choix, nom: String, ligne: [u8; 3], fond: Option<[u8; 3]>) {
        let nom = personnages::nom_net(&nom);
        match c {
            Choix::Personnage(id) => {
                let mut fond_avant = None;
                if let Some(p) = self.doc.personnages.iter_mut().find(|p| p.id == id) {
                    let ancien = std::mem::replace(&mut p.name, nom.clone());
                    fond_avant = Some(p.fill);
                    (p.line, p.fill) = (ligne, fond);
                    self.modified = true;
                    // Renommer le personnage renomme ses calques (`calquespastilles.rs`).
                    self.doc.renommer_calques_de_personnage(id, &ancien, &nom);
                }
                // Une autre couleur d'aplat : tous ses calques, dans tous les plans et toutes les versions de plan, la prennent, avec
                // la peinture de leur strate d'aplat (`aplatperso.rs`), à la fin du geste.
                if let (Some(avant), Some(apres)) = (fond_avant, fond) {
                    if avant != Some(apres) {
                        self.aplat_perso_demander(id, avant, apres);
                    }
                }
            }
            Choix::Classique(i) | Choix::Decor(i) => {
                if let Some(n) = self.liste_poste_mut(c.type_()).and_then(|l| l.get_mut(i)) {
                    (n.name, n.line, n.fill) = (nom, ligne, fond);
                    self.settings_a_ecrire = true;
                }
            }
        }
    }

    /// Un personnage de plus (une étape d'annulation) ; rend son numéro. Il est choisi tout de suite.
    pub(crate) fn perso_ajouter(&mut self) -> Option<u64> {
        if self.content_refuses() {
            return None;
        }
        if self.doc.personnages.len() >= personnages::MAX_PERSONNAGES {
            self.status = format!("{} personnages au plus.", personnages::MAX_PERSONNAGES);
            return None;
        }
        self.perso_etape("Nouveau personnage");
        let (line, fill) = personnages::couleurs_neuves(self.doc.personnages.len());
        let id = self.doc.alloc();
        let name = personnages::nom_neuf(&self.doc.personnages);
        self.doc.personnages.push(Personnage { id, name, line, fill: Some(fill), brosse: None });
        self.perso_choisir(Choix::Personnage(id));
        Some(id)
    }

    /// Une brosse du poste de plus, du type `t` (Classique ou Décor) : réglage de l'application, sans annulation. Rend son rang ; elle est
    /// choisie tout de suite.
    pub(crate) fn perso_ajouter_poste(&mut self, t: Type) -> Option<usize> {
        let (max, nom) = match t {
            Type::Classique => (personnages::MAX_CLASSIQUES, "classiques"),
            Type::Decor => (personnages::MAX_DECORS, "décors"),
            Type::Personnage => return None,
        };
        let liste = self.liste_poste(t)?;
        if liste.len() >= max {
            self.status = format!("{max} brosses {nom} au plus.");
            return None;
        }
        let name = personnages::nom_neuf_poste(liste, t);
        let brosse = match t {
            Type::Decor => Brosse::decor(&name, [0x80, 0x80, 0x80], Some([0xD0, 0xD0, 0xD0])),
            _ => Brosse::classique(&name, [0x50, 0x50, 0x50], Some([0xC0, 0xC0, 0xC0])),
        };
        let liste = self.liste_poste_mut(t)?;
        liste.push(brosse);
        let rang = liste.len() - 1;
        self.settings_a_ecrire = true;
        self.perso_choisir(if t == Type::Decor { Choix::Decor(rang) } else { Choix::Classique(rang) });
        Some(rang)
    }

    /// Une brosse Classique de plus (les essais).
    #[cfg(test)]
    pub(crate) fn perso_ajouter_classique(&mut self) -> Option<usize> {
        self.perso_ajouter_poste(Type::Classique)
    }

    /// Remet les valeurs d'origine du type `t` (le bouton « ↺ » de sa section du panneau).
    /// - Classique et Décor : réglages de l'application, sans annulation ; la liste redevient les trois gris d'origine.
    /// - Personnage : ceux du projet. Perso 1, Perso 2 et Perso 3 retrouvent leurs couleurs d'origine (ceux qu'on a retirés reviennent, au bout
    ///   de la liste), une seule étape d'annulation ; les autres personnages, renommés ou non, ne sont pas touchés.
    pub(crate) fn perso_d_origine(&mut self, t: Type) {
        match t {
            Type::Personnage => self.perso_personnages_d_origine(),
            Type::Classique | Type::Decor => {
                if matches!(self.perso.actif, Some(c) if c.type_() == t) {
                    self.perso_lacher();
                }
                if matches!(self.perso.edition, Some(c) if c.type_() == t) {
                    self.perso.edition = None;
                }
                *self.liste_poste_mut(t).expect("une liste de poste") = if t == Type::Decor { personnages::decors_par_defaut() } else { personnages::classiques_par_defaut() };
                self.settings_a_ecrire = true;
                self.status = format!("Brosses {} d'origine remises.", t.nom().to_lowercase());
            }
        }
    }

    fn perso_personnages_d_origine(&mut self) {
        if self.content_refuses() {
            return;
        }
        let origine = personnages::personnages_d_origine();
        let manquants = origine.iter().filter(|(n, _, _)| !self.doc.personnages.iter().any(|p| p.name == *n)).count();
        let change = origine.iter().any(|(n, l, f)| self.doc.personnages.iter().find(|p| p.name == *n).is_some_and(|p| p.line != *l || p.fill != Some(*f)));
        if !change && manquants == 0 {
            self.status = "Les personnages d'origine (Perso 1 à 3) sont déjà comme à l'origine.".into();
            return;
        }
        if self.doc.personnages.len() + manquants > personnages::MAX_PERSONNAGES {
            self.status = format!("{} personnages au plus.", personnages::MAX_PERSONNAGES);
            return;
        }
        self.perso_etape("Personnages d'origine");
        for (nom, line, fill) in origine {
            match self.doc.personnages.iter().find(|p| p.name == nom).map(|p| p.id) {
                Some(id) => self.perso_ecrire(Choix::Personnage(id), nom.into(), line, Some(fill)),
                None => {
                    let id = self.doc.alloc();
                    self.doc.personnages.push(Personnage { id, name: nom.into(), line, fill: Some(fill), brosse: None });
                    self.modified = true;
                }
            }
        }
        self.status = "Personnages d'origine remis (Perso 1 à 3) ; les autres ne changent pas.".into();
    }

    /// La brosse du poste suivie dans la liste : le choix d'une classique ou d'un décor plus loin dans la liste suit sa brosse quand
    /// les rangs bougent (`f` : l'ancien rang vers le nouveau).
    fn perso_suivre_rang(&mut self, t: Type, f: impl Fn(usize) -> usize) {
        self.perso.actif = match (self.perso.actif, t) {
            (Some(Choix::Classique(a)), Type::Classique) => Some(Choix::Classique(f(a))),
            (Some(Choix::Decor(a)), Type::Decor) => Some(Choix::Decor(f(a))),
            (autre, _) => autre,
        };
        // (La fenêtre « Modifier » d'un rang qui a bougé se referme : elle montrerait une autre brosse.)
        if matches!(self.perso.edition, Some(c) if c.type_() == t && t != Type::Personnage) {
            self.perso.edition = None;
        }
    }

    /// Duplique cette brosse, juste après elle.
    pub(crate) fn perso_dupliquer(&mut self, c: Choix) {
        match c {
            Choix::Personnage(id) => {
                let Some(i) = self.doc.personnages.iter().position(|p| p.id == id) else { return };
                if self.content_refuses() || self.doc.personnages.len() >= personnages::MAX_PERSONNAGES {
                    return;
                }
                self.perso_etape("Dupliquer le personnage");
                let mut copie = self.doc.personnages[i].clone();
                copie.id = self.doc.alloc();
                copie.name = personnages::nom_de_copie(&self.doc.personnages, &copie.name);
                self.doc.personnages.insert(i + 1, copie);
            }
            Choix::Classique(i) | Choix::Decor(i) => {
                let t = c.type_();
                let max = if t == Type::Decor { personnages::MAX_DECORS } else { personnages::MAX_CLASSIQUES };
                let Some(liste) = self.liste_poste_mut(t) else { return };
                if i >= liste.len() || liste.len() >= max {
                    return;
                }
                let mut copie = liste[i].clone();
                copie.name = personnages::nom_net(&format!("{} (copie)", copie.name));
                liste.insert(i + 1, copie);
                self.settings_a_ecrire = true;
                self.perso_suivre_rang(t, |a| if a > i { a + 1 } else { a });
            }
        }
    }

    /// Supprime cette brosse. Ce qui est déjà dessiné ne change pas (un trait ne se souvient pas de sa brosse ; les calques d'un personnage
    /// supprimé restent, et deviennent des calques ordinaires).
    pub(crate) fn perso_supprimer(&mut self, c: Choix) {
        match c {
            Choix::Personnage(id) => {
                if self.content_refuses() || !self.doc.personnages.iter().any(|p| p.id == id) {
                    return;
                }
                self.perso_etape("Supprimer le personnage");
                self.doc.personnages.retain(|p| p.id != id);
            }
            Choix::Classique(i) | Choix::Decor(i) => {
                let t = c.type_();
                if self.liste_poste(t).is_none_or(|l| i >= l.len()) {
                    return;
                }
                // Une brosse supprimée est d'abord lâchée (la brosse Décor rend l'outil, les pinceaux et le calque d'avant).
                if self.perso.actif == Some(c) {
                    self.perso_lacher();
                }
                if let Some(liste) = self.liste_poste_mut(t) {
                    liste.remove(i);
                }
                self.settings_a_ecrire = true;
                self.perso_suivre_rang(t, |a| if a > i { a - 1 } else { a });
            }
        }
        if self.perso.actif == Some(c) {
            self.perso.actif = None;
        }
        if self.perso.edition == Some(c) {
            self.perso.edition = None;
        }
    }

    /// Déplace cette brosse de `delta` places dans sa liste (négatif : vers la gauche).
    pub(crate) fn perso_deplacer(&mut self, c: Choix, delta: isize) {
        match c {
            Choix::Personnage(id) => {
                let Some(i) = self.doc.personnages.iter().position(|p| p.id == id) else { return };
                if self.content_refuses() || personnages::deplacer(&mut self.doc.personnages.clone(), i, delta).is_none() {
                    return;
                }
                self.perso_etape("Ranger les personnages");
                personnages::deplacer(&mut self.doc.personnages, i, delta);
            }
            Choix::Classique(i) | Choix::Decor(i) => {
                let t = c.type_();
                let Some(to) = self.liste_poste_mut(t).and_then(|l| personnages::deplacer(l, i, delta)) else { return };
                self.settings_a_ecrire = true;
                // Le choix suit sa brosse.
                self.perso_suivre_rang(t, |a| {
                    if a == i {
                        to
                    } else if i < to && (i..=to).contains(&a) {
                        a - 1
                    } else if to < i && (to..=i).contains(&a) {
                        a + 1
                    } else {
                        a
                    }
                });
            }
        }
    }

    /// Range la brosse du rang `from` à la place du rang `to`, dans la liste du type `t` (glisser dans le panneau).
    fn perso_glisser(&mut self, t: Type, from: usize, to: usize) {
        let tous = self.perso_du_type(t);
        if from == to || from >= tous.len() || to >= tous.len() {
            return;
        }
        self.perso_deplacer(tous[from], to as isize - from as isize);
    }

    // ------------------------------------------------------------------------ commandes, tic, fond

    /// La barre est-elle montrée dans cet espace de travail ?
    pub(crate) fn barre_perso_visible(&self) -> bool {
        self.layout.character_bar.unwrap_or_else(|| self.espace.barre_personnages_par_defaut())
    }

    /// Les commandes des brosses dédiées ; vrai si `action` en était une.
    pub(crate) fn perso_action(&mut self, action: &str) -> bool {
        match action {
            "perso.barre" => {
                self.layout.character_bar = Some(!self.barre_perso_visible());
                self.status = if self.barre_perso_visible() { "Pastilles des brosses dédiées montrées dans cet espace de travail." } else { "Pastilles des brosses dédiées cachées dans cet espace de travail." }.into();
            }
            "perso.ajouter" => {
                // La brosse arrive tout de suite dans la barre, sans fenêtre « Modifier » (Anthony, 2026-10-28) : son nom et
                // ses couleurs se changent dans le panneau Brosses dédiées ou au clic droit sur sa pastille.
                if let Some(id) = self.perso_ajouter() {
                    let nom = self.perso_nom(Choix::Personnage(id)).unwrap_or_default();
                    self.status = format!("« {nom} » ajouté (brosse Personnage). Clic droit sur sa pastille ou panneau Brosses dédiées : son nom et ses couleurs.");
                }
            }
            "perso.ajouter_classique" | "perso.ajouter_decor" => {
                let t = if action == "perso.ajouter_decor" { Type::Decor } else { Type::Classique };
                if let Some(i) = self.perso_ajouter_poste(t) {
                    let nom = self.liste_poste(t).and_then(|l| l.get(i)).map(|b| b.name.clone()).unwrap_or_default();
                    self.status = format!("« {nom} » ajoutée (brosse {}). Clic droit sur sa pastille ou panneau Brosses dédiées : son nom et ses couleurs.", t.nom());
                }
            }
            "perso.suivant" => self.perso_voisin(1),
            "perso.precedent" => self.perso_voisin(-1),
            "perso.aucun" => self.perso_lacher(),
            _ => return false,
        }
        true
    }

    /// À chaque image d'écran : les fonds de trait arrivés sont posés, et la brosse choisie suit sa couleur (ou
    /// est lâchée si quelqu'un a pris une autre couleur à la main, ou si elle n'existe plus). Au repos, rien.
    pub(crate) fn tick_personnages(&mut self) {
        self.tick_personnages_corps();
        // Le pinceau : posé, gardé ou rendu selon la brosse qui reste choisie après tout cela (`brossesdediees.rs`).
        self.dediee_suivre(false);
    }

    fn tick_personnages_corps(&mut self) {
        self.decor_tick();
        self.tick_aplat_perso();
        if let Some(fil) = self.perso.fil.as_mut() {
            if fil.en_cours() > 0 {
                for f in fil.recevoir() {
                    self.perso_poser(f);
                }
            }
        }
        let Some(choix) = self.perso.actif else { return };
        // (Une brosse Décor se suit dans `decor_tick`, qui sait aussi rendre l'outil et le calque d'avant.)
        if matches!(choix, Choix::Decor(_)) {
            return;
        }
        match self.perso_couleurs(choix) {
            None => self.perso.actif = None,
            Some(_) if self.color != self.perso.posee => self.perso.actif = None,
            Some((ligne, _)) if ligne != self.perso.posee => {
                self.set_front(ligne);
                self.perso.posee = ligne;
            }
            Some(_) => {}
        }
    }

    /// Le calque courant est un calque de couleur en mode Griffonner : ses traits sont des griffonnages qui remplissent
    /// des zones, pas des traits dont on déduirait un aplat. Un personnage y donne sa couleur de trait, sans aplat.
    fn griffonne_ici(&self) -> bool {
        self.layer().couleur.as_ref().is_some_and(|c| c.griffonner)
    }

    /// Au début d'un trait de pinceau ou de crayon : le calque prend un aplat sous le trait si le personnage en a un
    /// (une étape d'annulation à part, « Aplat du personnage »), et un personnage lâché par une couleur prise à la
    /// main est lâché avant de dessiner.
    pub(crate) fn perso_avant_trait(&mut self) {
        let Some(c) = self.perso.actif else { return };
        if self.color != self.perso.posee {
            // (Une brosse Décor lâchée par une couleur prise à la main rend aussi l'outil et les pinceaux d'avant, sans la couleur.)
            if self.decor_actif().is_some() {
                self.decor_lacher_avec(true, false);
            } else {
                self.perso.actif = None;
                self.dediee_rendre();
            }
            return;
        }
        let Some((_, Some(fond_pastille))) = self.perso_couleurs(c) else { return };
        // L'aplat que ce trait aura : celui du personnage du calque si c'est un calque de personnage (`aplatperso.rs` : rien d'autre ne
        // change sa couleur), sinon celui de la pastille.
        let fond_ici = if self.aplat_vient_du_personnage(self.cur_layer) { self.aplat_couleur_imposee(self.cur_layer) } else { Some(fond_pastille) };
        let Some(fond_ici) = fond_ici else { return };
        if self.aplat_en_cible() || self.layer().fond.is_some() || self.griffonne_ici() {
            return;
        }
        let li = self.cur_layer;
        let avant = self.history.context.replace("Aplat du personnage".to_string());
        self.toggle_fond(li);
        self.history.context = avant;
        // L'aplat du calque naît de la couleur d'aplat de la pastille (et non du blanc d'un aplat neuf) : les morceaux que
        // l'aplat gagnera plus tard, sans que la pastille y ait peint, ont la même couleur que le reste.
        if let Some(f) = self.layers_mut().get_mut(li).and_then(|l| l.fond.as_mut()) {
            f.couleur = fond_ici;
        }
        // Ce qui était déjà dessiné sur ce calque n'est pas à un personnage : il reste sans fond (le fond du calque
        // se pose sur tout son trait), coupé comme le ferait un personnage sans fond.
        let Some(reglages) = self.layer().fond else { return };
        let Some(cel) = self.cel_here() else { return };
        let Some(c) = self.doc.cels.get(&cel).filter(|c| !c.raster.is_blank()) else { return };
        let apres = c.raster.clone();
        let avant = crate::raster::Raster::new(apres.width, apres.height);
        self.perso_envoyer(Travail { cel, serial: u64::MAX, etape: 0, reglages, couleur: None, avant, apres, suivre: None });
    }

    /// La couleur d'aplat de la pastille qui dessine devient la « Couleur de l'aplat » du calque (Anthony, 2026-10-28 : « la couleur
    /// d'aplat du pinceau change la couleur d'aplat du calque »). Elle était blanche (celle d'un aplat neuf) pendant que la pastille
    /// peignait son aplat dans la strate : tout morceau d'aplat gagné ensuite sans pinceau de pastille (gomme, transformation,
    /// Cutter) retombait sur la couleur du calque, blanche, à côté du reste. Dans la même étape d'annulation que le trait (la
    /// structure d'avant se range à la suite de l'étape du trait). Une couleur neutre à aplat fait de même, dans le calque courant.
    /// Sans effet pour une pastille sans aplat, la pastille « classique » (aucune pastille), la gomme, ni quand l'aplat lui-même
    /// est visé.
    ///
    /// Le choix : la couleur du calque change, pas celle des pixels déjà peints (une peinture blanche volontaire, les yeux, ne
    /// devient pas brune) ; les clés du calque dont l'aplat n'était pas peint prennent la couleur, comme celles des autres
    /// traits de la pastille.
    ///
    /// **Jamais pour un calque de personnage** (Anthony, 2026-10-29 : « sur les calques personnages, l'aplat ne change de couleur que si
    /// on modifie les paramètres de couleur de l'aplat dans le panneau Brosses dédiées ») : un trait fait là avec une autre pastille ne change
    /// pas sa couleur d'aplat ; au plus, elle est remise à celle de son propre personnage si elle en avait une autre (`aplatperso.rs`).
    pub(crate) fn perso_aligner_couleur_aplat(&mut self) {
        if matches!(self.tool, Tool::Eraser | Tool::Mixer) || self.aplat_en_cible() || self.griffonne_ici() {
            return;
        }
        let li = self.cur_layer;
        let fond = if self.aplat_vient_du_personnage(li) {
            let Some(propre) = self.aplat_couleur_imposee(li) else { return };
            propre
        } else {
            let Some((_, Some(fond))) = self.perso_couleurs_actives() else { return };
            fond
        };
        let Some(r) = self.layers().get(li).and_then(|l| l.fond) else { return };
        if r.couleur == fond {
            return;
        }
        let avant = self.snapshot();
        if let Some(f) = self.layers_mut().get_mut(li).and_then(|l| l.fond.as_mut()) {
            f.couleur = fond;
        }
        self.history.extend_last(avant);
        self.modified = true;
        self.renderer.invalidate_all();
    }

    /// Confie un trait au fil de fond (lancé au premier).
    fn perso_envoyer(&mut self, travail: Travail) {
        let ctx = self.ctx.clone();
        self.perso.fil.get_or_insert_with(|| Fil::new(ctx)).envoyer(travail);
    }

    /// À la fin d'un trait de pinceau ou de crayon (`end_stroke`) : si un personnage dessinait et que le calque a un
    /// fond, le fil de fond calcule les pixels que le trait a gagnés. `tiles` : les tuiles du dessin d'avant le trait.
    pub(crate) fn perso_apres_trait(&mut self, cel: CelId, tiles: Vec<(usize, Option<Tile>)>) {
        if self.tool == Tool::Eraser || self.aplat_en_cible() || self.griffonne_ici() {
            return;
        }
        let (fond, sans_pastille) = match self.perso_couleurs_actives() {
            Some((_, fond)) => (fond, false),
            // Sans pastille sur un calque ordinaire : le trait suit la teinte de la clé du dessin, s'il en a une (`suivre`).
            None if !self.aplat_vient_du_personnage(self.cur_layer) => (None, true),
            None => return,
        };
        // Sur un calque de personnage, l'aplat que le trait gagne est celui de ce personnage, quelle que soit la pastille.
        let fond = if self.aplat_vient_du_personnage(self.cur_layer) { self.aplat_couleur_imposee(self.cur_layer) } else { fond };
        let Some(reglages) = self.layers().get(self.cur_layer).and_then(|l| l.fond) else { return };
        let Some(c) = self.doc.cels.get(&cel) else { return };
        // (Un dessin sans peinture d'aplat n'a pas de teinte à suivre : rien n'est envoyé, le cas courant.)
        let suivre = if sans_pastille {
            match c.fond.as_deref().map(|s| s.peinture.raster.clone()).filter(|r| !r.is_blank()) {
                Some(r) => Some(r),
                None => return,
            }
        } else {
            None
        };
        let apres = c.raster.clone();
        let mut avant = apres.clone();
        for (i, t) in tiles {
            avant.replace_slot(i, t);
        }
        let serial = self.history.serial();
        let etape = self.history.last_id();
        self.perso_envoyer(Travail { cel, serial, etape, reglages, couleur: fond, avant, apres, suivre });
    }

    /// Pose le fond d'un trait de personnage : les pixels gagnés prennent la couleur de fond du personnage dans la
    /// peinture du fond (ou sont coupés du fond, s'il n'en a pas). Dans l'étape d'annulation du trait si rien ne
    /// s'est passé depuis ; sinon sans étape à lui (voir `persofond.rs`).
    pub(crate) fn perso_poser(&mut self, f: Fini) {
        if f.est_vide() {
            return;
        }
        let Some(c) = self.doc.cels.get_mut(&f.cel) else { return };
        let (w, h) = (c.raster.width, c.raster.height);
        if f.masque.len() != w * h {
            return;
        }
        let existait = c.fond.is_some();
        let s = c.fond.get_or_insert_with(|| Box::new(Strate::new(w, h)));
        let mut edits = Vec::new();
        let mut note = |old: Vec<(usize, Option<Tile>)>, coupe: bool| {
            if !old.is_empty() {
                edits.push(Edit::FondTiles { cel: f.cel, tiles: old, coupe });
            }
        };
        match f.couleur {
            Some(col) => {
                note(crate::fond::paint_mask(&mut s.peinture.raster, &f.masque, Some(to_color(col))), false);
                note(crate::fond::paint_mask(&mut s.coupe, &f.masque, None), true);
            }
            None => {
                note(crate::fond::paint_mask(&mut s.coupe, &f.masque, Some(Color32::WHITE)), true);
                note(crate::fond::paint_mask(&mut s.peinture.raster, &f.masque, None), false);
            }
        }
        s.peinture.version += 1;
        c.version += 1;
        if !edits.is_empty() {
            if self.history.serial() == f.serial {
                self.history.extend_last(Edit::Group(edits));
            } else {
                match self.history.etat_de_l_etape(f.etape) {
                    // L'étape du trait est encore à annuler : le fond la rejoint, les autres étapes ne bougent pas.
                    Some(true) => {
                        self.history.extend_step(f.etape, Edit::Group(edits));
                    }
                    // Le trait a été défait entre-temps : son fond est rangé avec son étape (il revient en la rétablissant) et
                    // l'écran reste comme il est (la forme du trait n'y est plus).
                    Some(false) => {
                        // (Les éditions de fond ne touchent pas à la navigation.)
                        let mut nav = crate::history::Nav { panel: self.cur_panel, layer: self.cur_layer };
                        let retour = Edit::Group(edits).apply(&mut self.doc, &mut nav);
                        self.history.extend_step(f.etape, retour);
                        // (Le dessin n'avait pas de strate : elle n'a pas à rester, vide.)
                        if let Some(c) = self.doc.cels.get_mut(&f.cel).filter(|_| !existait) {
                            if c.fond.as_deref().is_some_and(|s| s.peinture.raster.is_blank() && s.coupe.is_blank()) {
                                c.fond = None;
                            }
                        }
                    }
                    None => {}
                }
            }
        }
        self.modified = true;
        self.renderer.invalidate_all();
    }

    /// Attend que les fonds de trait en route soient posés (les essais).
    #[cfg(test)]
    pub(crate) fn perso_attendre(&mut self) {
        let Some(fil) = self.perso.fil.as_mut() else { return };
        for f in fil.attendre(std::time::Duration::from_secs(20)) {
            self.perso_poser(f);
        }
    }

    /// Avant de copier des dessins (couper, réunir, dupliquer, ranger une version, copier des clés ou un calque) ou d'enregistrer : le
    /// fond que leur dernier trait de personnage vient de gagner se pose d'abord, pour que la copie et le fichier l'aient. Au repos, rien
    /// (un seul test). Sinon on attend au plus 2 secondes un calcul qui tient en général en quelques images : jamais pendant un geste de
    /// dessin, seulement au moment d'une commande.
    pub(crate) fn attendre_fonds_de_traits(&mut self) {
        let Some(fil) = self.perso.fil.as_mut() else { return };
        if fil.en_cours() == 0 {
            return;
        }
        for f in fil.attendre(std::time::Duration::from_secs(2)) {
            self.perso_poser(f);
        }
    }

    // ------------------------------------------------------------------------------- la barre

    /// Les pastilles (Anthony, 2026-10-21 : une colonne ronde et discrète à droite du dessin ; 2026-10-24 : une rangée dans la barre
    /// du haut, voir plus bas).
    ///
    /// Dans l'ordre : les Personnages, les Classiques, les Décors, « + » (le menu des trois types) et l'icône du panneau Brosses dédiées. Dans
    /// la barre, ce sont de vrais éléments de la barre : cliquer une pastille ne trace rien. Rien n'est calculé de lourd : une dizaine de cercles.
    ///
    /// **Depuis le 2026-10-24 (Anthony, d'après une copie d'écran) : une rangée, plus une colonne.** Les mêmes pastilles, dans le même
    /// ordre, vont à l'extrémité droite de la barre du haut de la vue (`viewbar::view_bar`, à droite des icônes des vues et de la règle),
    /// et à celle de la barre du haut de la planche des Vignettes (`pastilles_rangee_flottante`). Le dessin ne leur laisse plus de colonne.
    /// Même hauteur de case et mêmes écarts que les icônes de la barre (`barres::CASE`, `barres::ECART`). Quand la barre est trop étroite,
    /// la rangée défile (jamais à la ligne sur le dessin) ; les bulles à 1 s, les clics droits, le « + » et le bouton du panneau sont ceux
    /// d'avant.
    pub(crate) fn pastilles_rangee(&mut self, ui: &mut egui::Ui) {
        if !self.barre_perso_visible() {
            self.perso.cases.clear();
            return;
        }
        // La rangée se pose à l'extrémité droite de la place libre de la barre. Sa largeur est celle de l'image d'avant (elle est
        // mesurée à chaque image : une zone qui défile ne se pose pas bien de droite à gauche) ; trop large pour la place, elle défile.
        let id = Id::new("pastilles_rangee_largeur");
        let dispo = ui.available_rect_before_wrap();
        let max = (dispo.width() - COLONNE_BORD).max(CASE_H);
        let connu: f32 = ui.data(|d| d.get_temp(id)).unwrap_or(max);
        let largeur = connu.clamp(CASE_H, max);
        let zone = Rect::from_min_size(pos2(dispo.right() - COLONNE_BORD - largeur, dispo.top()), vec2(largeur, dispo.height()));
        let mut rangee = ui.new_child(egui::UiBuilder::new().max_rect(zone).layout(egui::Layout::left_to_right(egui::Align::Center)));
        let mesuree = self.rangee_contenu(&mut rangee, largeur);
        ui.data_mut(|d| d.insert_temp(id, mesuree));
        if (mesuree - connu).abs() > 0.5 {
            ui.ctx().request_repaint();
        }
    }

    /// La même rangée, flottante, à l'extrémité droite de la barre du haut de la planche des Vignettes : `haut_droite` est le coin haut
    /// droit de la planche, `largeur_max` ce que la barre de réglages laisse libre. C'est une `Area` : ce qui est dessous ne reçoit pas ses clics.
    pub(crate) fn pastilles_rangee_flottante(&mut self, ctx: &egui::Context, haut_droite: egui::Pos2, largeur_max: f32) {
        if !self.barre_perso_visible() {
            self.perso.cases.clear();
            return;
        }
        let id = Id::new("pastilles_rangee_planche_largeur");
        let max = largeur_max.max(CASE_H);
        let connu: f32 = ctx.data(|d| d.get_temp(id)).unwrap_or(max);
        let largeur = connu.clamp(CASE_H, max);
        let place = pos2(haut_droite.x - COLONNE_BORD - largeur, haut_droite.y + 1.0);
        let mesuree = egui::Area::new(Id::new("pastilles_rangee_planche"))
            .order(egui::Order::Middle)
            .fixed_pos(place)
            .show(ctx, |ui| {
                ui.set_max_width(largeur);
                self.rangee_contenu(ui, largeur)
            })
            .inner;
        ctx.data_mut(|d| d.insert_temp(id, mesuree));
        if (mesuree - connu).abs() > 0.5 {
            ctx.request_repaint();
        }
    }

    /// Les pastilles, de gauche à droite, dans une zone qui défile si `largeur` ne leur suffit pas ; rend la largeur de leur contenu.
    fn rangee_contenu(&mut self, ui: &mut egui::Ui, largeur: f32) -> f32 {
        let persos: Vec<Personnage> = self.doc.personnages.clone();
        let classiques: Vec<Brosse> = self.settings.brosses_classiques.clone();
        let decors: Vec<Brosse> = if self.decor_offert() { self.settings.brosses_decor.clone() } else { Vec::new() };
        let mut cases: Vec<(Rect, Case)> = Vec::new();
        let fond = crate::theme::entete().gamma_multiply(0.62);
        let sortie = egui::ScrollArea::horizontal()
            .id_salt("pastilles_rangee")
            .max_width(largeur)
            .max_height(CASE_H)
            // (Elle se resserre sur son contenu dans les deux sens : une zone qui prendrait toute la hauteur libre d'une `Area` couvrirait,
            // dans la planche, tout ce qui est dessous.)
            .auto_shrink([true, true])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                egui::Frame::NONE.fill(fond).corner_radius(CASE_H / 2.0).inner_margin(egui::Margin::symmetric(6, 0)).show(ui, |ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing = vec2(crate::barres::ECART, 0.0);
                        self.colonne_contenu(ui, &persos, &classiques, &decors, &mut cases);
                    });
                });
            });
        self.perso.cases = cases;
        sortie.content_size.x
    }

    /// Les pastilles de la rangée : les Personnages, les Classiques, les Décors, « + » et le panneau, un petit trait entre deux groupes.
    fn colonne_contenu(&mut self, ui: &mut egui::Ui, persos: &[Personnage], classiques: &[Brosse], decors: &[Brosse], cases: &mut Vec<(Rect, Case)>) {
        let mut avant = false;
        for p in persos {
            let r = self.pastille_de_colonne(ui, Choix::Personnage(p.id), &p.name, p.line, p.fill);
            cases.push((r.rect, Case::Choix(Choix::Personnage(p.id))));
        }
        avant |= !persos.is_empty();
        if avant && !classiques.is_empty() {
            trait_de_colonne(ui);
        }
        for (i, n) in classiques.iter().enumerate() {
            let r = self.pastille_de_colonne(ui, Choix::Classique(i), &n.name, n.line, n.fill);
            cases.push((r.rect, Case::Choix(Choix::Classique(i))));
        }
        avant |= !classiques.is_empty();
        // Les brosses Décor (Storyboard et Vignettes, `decor_offert`) : des carrés arrondis, pour ne pas les confondre avec les personnages
        // (des ronds).
        if avant && !decors.is_empty() {
            trait_de_colonne(ui);
        }
        for (i, n) in decors.iter().enumerate() {
            let r = self.pastille_de_colonne(ui, Choix::Decor(i), &n.name, n.line, n.fill);
            cases.push((r.rect, Case::Choix(Choix::Decor(i))));
        }
        trait_de_colonne(ui);
        let r = self.bouton_plus(ui, cases);
        cases.push((r.rect, Case::Plus));
        let ouvert = self.pane_shown(Pane::Personnages);
        let key = self.raccourci("vue.personnages");
        let r = ui.add(egui::Button::new(crate::bulles::symbole_etat(ph::USERS_THREE, ouvert)).selected(ouvert).frame(ouvert).min_size(vec2(COLONNE_L - 4.0, 0.0)));
        if crate::bulles::bulle(r.clone(), "Panneau Brosses dédiées", "Les trois sortes de brosses dédiées (Personnages, Classiques, Décors) : noms, couleurs de trait et d'aplat, avec quoi chacune dessine (ton pinceau, ou sa propre brosse), ordre. Un clic l'ouvre ou le ferme.", &key).clicked() {
            self.run_action("vue.personnages");
        }
        cases.push((r.rect, Case::Panneau));
    }

    /// Le « + » : un petit menu « Brosse personnage / classique / décor ». Choisir une ligne ajoute la brosse tout de suite (elle est
    /// choisie), sans fenêtre « Modifier ».
    fn bouton_plus(&mut self, ui: &mut egui::Ui, cases: &mut Vec<(Rect, Case)>) -> Response {
        let reponse = crate::bulles::icone(ui, ph::PLUS, "Nouvelle brosse dédiée", "Ajouter une brosse dédiée : à un personnage, à un décor, ou classique. Son nom et ses couleurs se règlent dans le panneau Brosses dédiées ; d'origine, elle dessine avec ton pinceau.", &self.raccourci("perso.ajouter"));
        let id = Id::new("perso_plus_menu");
        if reponse.clicked() {
            ui.memory_mut(|m| m.toggle_popup(id));
        }
        egui::popup_below_widget(ui, id, &reponse, egui::popup::PopupCloseBehavior::CloseOnClick, |ui| {
            ui.set_min_width(150.0);
            for t in Type::ALL {
                let (nom, aide) = match t {
                    Type::Personnage => ("Brosse personnage", "Une brosse qui dessine dans un calque à son nom, créé au premier trait (du projet)."),
                    Type::Classique => ("Brosse classique", "Une brosse qui dessine dans le calque courant, sans rien créer (réglage de FEANOR, pour tous les projets)."),
                    Type::Decor => ("Brosse décor", "Une brosse qui dessine dans le calque Décor du plan, créé au premier trait (réglage de FEANOR, pour tous les projets)."),
                };
                let r = ui.button(nom).on_hover_text(aide);
                cases.push((r.rect, Case::Ajouter(t)));
                if r.clicked() {
                    self.run_action(action_ajouter(t));
                }
            }
        });
        reponse
    }
    /// Une pastille de la rangée : clic = choisir (ou lâcher), clic droit = son menu. Ronde et normale pour un personnage, petite pour une
    /// classique, un carré arrondi pour un décor.
    fn pastille_de_colonne(&mut self, ui: &mut egui::Ui, c: Choix, nom: &str, ligne: [u8; 3], fond: Option<[u8; 3]>) -> Response {
        let choisie = self.perso.actif == Some(c);
        let specifique = self.dediee_outil(c);
        let resp = match c {
            Choix::Personnage(_) => pastille_taille(ui, ligne, fond, choisie, PASTILLE, specifique),
            Choix::Classique(_) => pastille_taille(ui, ligne, fond, choisie, PASTILLE_CLASSIQUE, specifique),
            Choix::Decor(_) => pastille_carree(ui, ligne, fond, choisie, specifique),
        };
        let resp = crate::bulles::bulle(resp, if nom.is_empty() { "Sans nom" } else { nom }, &description(c.type_(), ligne, fond, choisie, &self.dediee_etiquette(c)), "");
        if resp.clicked() {
            if choisie {
                self.perso_lacher();
            } else {
                self.perso_choisir(c);
            }
        }
        resp.clone().context_menu(|ui| self.pastille_menu(ui, c));
        resp
    }

    /// Le menu du clic droit sur une pastille.
    fn pastille_menu(&mut self, ui: &mut egui::Ui, c: Choix) {
        let tous = self.perso_du_type(c.type_());
        let Some(rang) = tous.iter().position(|x| *x == c) else {
            ui.close_menu();
            return;
        };
        let total = tous.len();
        if ui.button("Modifier…").clicked() {
            self.perso.edition = Some(c);
            ui.close_menu();
        }
        if ui.button("Dupliquer").clicked() {
            self.perso_dupliquer(c);
            ui.close_menu();
        }
        ui.separator();
        if ui.add_enabled(rang > 0, egui::Button::new("Monter")).clicked() {
            self.perso_deplacer(c, -1);
            ui.close_menu();
        }
        if ui.add_enabled(rang + 1 < total, egui::Button::new("Descendre")).clicked() {
            self.perso_deplacer(c, 1);
            ui.close_menu();
        }
        ui.separator();
        if ui.button("Supprimer").clicked() {
            self.perso_supprimer(c);
            ui.close_menu();
        }
    }

    // ------------------------------------------------------------------- édition et panneau

    /// Une ligne de réglages : le nom, la couleur du trait, l'aplat (ou non) et sa couleur, puis avec quoi elle dessine (« Suit le pinceau » ou
    /// « Brosse spécifique » et ses réglages).
    fn perso_ligne(&mut self, ui: &mut egui::Ui, c: Choix) {
        let Some((mut nom, mut ligne, mut fond)) = self.perso_lire(c) else { return };
        let mut change = false;
        let resp = ui.add(egui::TextEdit::singleline(&mut nom).desired_width(110.0).char_limit(personnages::NOM_MAX).hint_text("Nom"));
        let resp = crate::bulles::bulle(resp, "Nom", "Le nom, montré dans la bulle de la pastille.", "");
        change |= self.perso_edited(&resp, c);
        let resp = crate::champcouleur::srgb(ui, &mut ligne);
        let resp = crate::bulles::bulle(resp, "Couleur du trait", "La couleur dont cette brosse dessine.", "");
        change |= self.perso_edited(&resp, c);
        let mut avec = fond.is_some();
        let resp = crate::bulles::interrupteur(ui, &mut avec, ph::PAINT_BUCKET, "Avec un aplat", "L'aplat : la couleur sous le trait, dans tout ce que le trait enferme. Éteint, la brosse dessine sans aplat.", "");
        if resp.changed() {
            let avant = self.history.context.replace("Modifier le personnage".to_string());
            if matches!(c, Choix::Personnage(_)) {
                self.record_step();
            }
            self.history.context = avant;
            fond = avec.then(|| fond.unwrap_or_else(|| personnages::fond_assorti(ligne)));
            change = true;
        }
        if let Some(f) = fond.as_mut() {
            let resp = crate::champcouleur::srgb(ui, f);
            let resp = crate::bulles::bulle(resp, "Couleur de l'aplat", "La couleur de l'aplat sous le trait de cette brosse.", "");
            change |= self.perso_edited(&resp, c);
        }
        if change {
            self.perso_ecrire(c, nom, ligne, fond);
        }
        // Avec quoi elle dessine : le pinceau courant (d'origine) ou sa propre brosse (§107, rossesdediees.rs).
        self.dediee_reglages(ui, c);
    }

    /// La fenêtre « Modifier » d'une brosse dédiée (clic droit sur sa pastille).
    pub(crate) fn personnages_fenetre(&mut self, ctx: &egui::Context) {
        let Some(c) = self.perso.edition else { return };
        if self.perso_couleurs(c).is_none() {
            self.perso.edition = None;
            return;
        }
        let titre = format!("Modifier la brosse {}", c.type_().nom().to_lowercase());
        let mut ouverte = true;
        egui::Window::new(titre).id(Id::new("perso_modifier")).collapsible(false).resizable(false).open(&mut ouverte).show(ctx, |ui| {
            ui.horizontal(|ui| {
                self.perso_ligne(ui, c);
            });
            if c.type_() != Type::Personnage {
                ui.label(RichText::new("Une brosse Classique ou Décor est un réglage de FEANOR : elle sert dans tous les projets.").small().weak());
            }
        });
        if !ouverte {
            self.perso.edition = None;
        }
    }

    /// Le panneau « Brosses dédiées » : trois sections, Personnages (du projet), Classiques et Décors (de l'application) ; dans chacune son « + »,
    /// ses lignes (qu'on range en glissant la poignée) et son « ↺ ».
    pub(crate) fn personnages_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.bouton_si(ui, self.perso.actif.is_some(), ph::X_CIRCLE, "Couleur libre", "Lâcher la brosse dédiée : le trait garde sa couleur, mais elle n'est plus celle d'une brosse. Un clic sur la brosse choisie, dans la barre, fait pareil.", "perso.aucun").clicked() {
                self.perso_lacher();
            }
        });
        for t in Type::ALL {
            ui.add_space(4.0);
            ui.separator();
            self.perso_section(ui, t);
        }
    }

    /// Une section du panneau : le titre, son « + » et son « ↺ », une phrase, puis les lignes.
    fn perso_section(&mut self, ui: &mut egui::Ui, t: Type) {
        let ids = self.perso_du_type(t);
        ui.horizontal(|ui| {
            ui.label(RichText::new(t.titre()).strong());
            let (nom_plus, aide_plus) = match t {
                Type::Personnage => ("Nouveau personnage", "Ajouter une brosse Personnage : son nom, sa couleur de trait et sa couleur d'aplat. Son calque naît au premier trait."),
                Type::Classique => ("Nouvelle brosse classique", "Ajouter une brosse Classique : un trait, avec ou sans aplat, dans le calque courant."),
                Type::Decor => ("Nouvelle brosse décor", "Ajouter une brosse Décor : un trait et un aplat, dans le calque Décor du plan. D'origine, elle dessine avec ton pinceau."),
            };
            if crate::bulles::icone(ui, ph::PLUS, nom_plus, aide_plus, "").clicked() {
                self.run_action(action_ajouter(t));
            }
            let aide_origine = match t {
                Type::Personnage => "Perso 1, Perso 2 et Perso 3 retrouvent leurs couleurs d'origine (ceux qu'on a retirés reviennent au bout de la liste). Les autres personnages ne changent pas. Une seule étape d'annulation.",
                Type::Classique => "Remettre les trois gris d'origine (le trait plus foncé que l'aplat, de plus en plus clairs).",
                Type::Decor => "Remettre les trois brosses Décor d'origine : trois niveaux de gris plus clairs que les classiques, qui suivent ton pinceau.",
            };
            if crate::bulles::icone(ui, ph::ARROW_COUNTER_CLOCKWISE, &format!("{} d'origine", t.titre()), aide_origine, "").clicked() {
                self.perso_d_origine(t);
            }
            let n = ids.len();
            ui.label(RichText::new(format!("{n} brosse{}", if n > 1 { "s" } else { "" })).small().weak());
        });
        let phrase = match t {
            Type::Personnage => "Du projet. Son premier trait crée un calque vectoriel à son nom, avec son aplat. Dessine avec ton pinceau, ou avec sa propre brosse.",
            Type::Classique => "Pour tous les projets. Dessine dans le calque courant, sans rien créer. Dessine avec ton pinceau, ou avec sa propre brosse.",
            Type::Decor => "Pour tous les projets. Dessine dans le calque Décor du plan, créé au premier trait. Dessine avec ton pinceau, ou avec sa propre brosse.",
        };
        ui.label(RichText::new(phrase).small().weak());
        if ids.is_empty() {
            ui.label(RichText::new(format!("Aucune brosse {}. Ajoute-en une avec le « + ».", t.nom().to_lowercase())).weak());
        }
        let mut glisse: Option<(usize, usize)> = None;
        let mut supprimee = None;
        for (i, c) in ids.iter().copied().enumerate() {
            let (ligne, fond) = self.perso_couleurs(c).unwrap_or(([0; 3], None));
            let specifique = self.dediee_outil(c);
            let (zone, depose) = ui.dnd_drop_zone::<(Type, usize), _>(egui::Frame::NONE, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    let poignee = ui.dnd_drag_source(Id::new(("perso_poignee", t.nom(), i)), (t, i), |ui| ui.label(RichText::new(ph::DOTS_SIX_VERTICAL).size(crate::bulles::TAILLE).weak())).response;
                    crate::bulles::bulle(poignee, "Ranger", "Glisser pour changer la place de la brosse dans la barre (dans sa section).", "");
                    let choisie = self.perso.actif == Some(c);
                    let r = match c {
                        Choix::Decor(_) => pastille_carree(ui, ligne, fond, choisie, specifique),
                        _ => pastille(ui, ligne, fond, choisie, specifique),
                    };
                    if r.clicked() {
                        if choisie {
                            self.perso_lacher();
                        } else {
                            self.perso_choisir(c);
                        }
                    }
                    self.perso_ligne(ui, c);
                    if crate::bulles::icone(ui, ph::TRASH, "Supprimer la brosse", "La retirer de la barre. Ce qui est déjà dessiné ne change pas.", "").clicked() {
                        supprimee = Some(c);
                    }
                });
            });
            if let Some(src) = depose {
                // (Une poignée lâchée dans une autre section ne range rien.)
                if src.0 == t {
                    glisse = Some((src.1, i));
                }
            } else if zone.response.dnd_hover_payload::<(Type, usize)>().is_some_and(|p| p.0 == t) {
                let r = zone.response.rect;
                ui.painter().hline(r.x_range(), r.top(), Stroke::new(2.0_f32, crate::theme::accent_texte()));
            }
        }
        if let Some((from, to)) = glisse {
            self.perso_glisser(t, from, to);
        }
        if let Some(c) = supprimee {
            self.perso_supprimer(c);
        }
    }
}
