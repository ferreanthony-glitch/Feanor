//! Brosse par personnage, côté interface (`personnages.rs`, `persofond.rs`) : la colonne de pastilles le long du
//! dessin, le panneau « Personnages », la fenêtre « Modifier », et ce que fait le choix d'un personnage.
//!
//! - **La colonne** (Anthony, 2026-10-21 : plus de barre au-dessus du dessin) : une colonne ronde et discrète au bord
//!   droit du dessin (Storyboard) ou de la planche (Vignettes), sous le titre du plan. De haut en bas : la pastille
//!   « classique » (2026-10-23 ; avant « aucun personnage » : le pinceau de base, « Pinceau rond », sans personnage ni
//!   décor ; un anneau barré, sans texte : la bulle le dit), une pastille ronde par personnage (anneau = couleur du
//!   trait, dedans = couleur de l'aplat), un petit trait, les couleurs neutres, les brosses du décor (Storyboard et
//!   Vignettes, `decorui.rs`), un « + » et l'icône qui ouvre le panneau. Un clic choisit : la couleur du trait devient celle du personnage et l'aplat sous le
//!   trait s'allume avec le sien (un nouveau trait gagne son aplat tout de suite, `persofond.rs`) ; un clic sur la
//!   pastille choisie la lâche. Clic droit : modifier, dupliquer, supprimer, monter, descendre. Elle se montre ou se
//!   cache par espace de travail (`Layout::character_bar`, `Espace::barre_personnages_par_defaut`). C'est une `Area` au-dessus du
//!   dessin : ses clics ne vont pas au dessin.
//! - **Le panneau** : la liste (nom, couleur du trait, fond ou non, couleur du fond), qu'on range en glissant
//!   par la poignée, puis les couleurs neutres.
//! - **Choisir à la main une autre couleur** (panneau Couleur, pipette, nuancier, échange des deux couleurs) lâche
//!   le personnage : la barre n'en montre plus aucun de choisi.
//!
//! Les personnages sont ceux du projet (annulables) ; les couleurs neutres sont un réglage de l'application.

use eframe::egui::{self, pos2, vec2, Color32, Id, Rect, Response, RichText, Sense, Stroke};
use egui_phosphor::regular as ph;

use crate::app::{App, Tool};
use crate::decor::Genre;
use crate::dock::Pane;
use crate::document::{CelId, Strate};
use crate::history::Edit;
use crate::personnages::{self, Neutre, Personnage};
use crate::persofond::{Fil, Fini, Travail};
use crate::raster::Tile;

/// Diamètre d'une pastille.
pub(crate) const PASTILLE: f32 = 20.0;
/// Côté d'une case de la colonne (une pastille et son halo).
const CASE_H: f32 = PASTILLE + 6.0;
/// Largeur de la colonne des pastilles : une case, et un souffle de chaque côté.
pub(crate) const COLONNE_L: f32 = CASE_H + 2.0;
/// Écart de la colonne au bord droit du dessin.
const COLONNE_BORD: f32 = 6.0;
/// Le pinceau que rend la pastille « classique » : le premier pinceau de SHOT, réglage de départ de l'outil Pinceau.
pub(crate) const PINCEAU_DE_BASE: &str = "Pinceau rond";

/// Une case de la colonne des pastilles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Case {
    /// La pastille « classique » : le pinceau de base, sans personnage ni décor (`perso_classique`).
    Classique,
    /// Un personnage ou une couleur neutre.
    Choix(Choix),
    /// « + » : un personnage de plus.
    Plus,
    /// L'icône du panneau Personnages.
    Panneau,
    /// Une brosse du décor (Storyboard et Vignettes, `decorui.rs`).
    Decor(Genre),
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

/// Ce qui est choisi dans la barre : un personnage du projet, ou une couleur neutre (son rang).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Choix {
    Personnage(u64),
    Neutre(usize),
}

/// L'état de la brosse par personnage d'un projet (rangé avec l'onglet).
#[derive(Default)]
pub(crate) struct Etat {
    /// Le personnage ou la couleur neutre qui dessine ; rien : la couleur est libre.
    pub actif: Option<Choix>,
    /// La couleur de trait posée quand on l'a choisi : si la couleur courante en diffère, quelqu'un a choisi une
    /// autre couleur à la main et le personnage est lâché.
    pub posee: [u8; 3],
    /// La fenêtre « Modifier » est ouverte sur ce personnage ou cette couleur neutre.
    pub edition: Option<Choix>,
    /// Le fil des fonds de trait (`persofond.rs`), lancé au premier trait.
    pub(crate) fil: Option<Fil>,
    /// Où sont les cases de la colonne des pastilles à l'écran, à la dernière image (les essais y cliquent).
    pub(crate) cases: Vec<(Rect, Case)>,
    /// La brosse du décor qui dessine (Vignettes) : tant qu'elle est choisie, aucun personnage ne l'est, et les traits vont dans le
    /// calque Décor du plan, quel que soit le calque courant (`decorui.rs`).
    pub(crate) decor: Option<Genre>,
    /// Ce que le choix du décor a pris à l'utilisateur et lui rendra quand il le lâche : outil, pinceau, crayon, couleur, calque.
    pub(crate) decor_avant: Option<crate::decorui::Avant>,
    /// La couleur et la taille posées par le dernier choix (ou suivi) du décor, pour voir quand les Préférences changent.
    pub(crate) decor_posee: Option<([u8; 3], f32)>,
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

/// Une pastille : l'anneau est la couleur du trait, l'intérieur celle du fond (rayé s'il n'y en a pas) ; un halo
/// de la couleur d'accent quand elle est choisie.
fn pastille(ui: &mut egui::Ui, ligne: [u8; 3], fond: Option<[u8; 3]>, choisie: bool) -> Response {
    pastille_taille(ui, ligne, fond, choisie, PASTILLE)
}

/// Diamètre d'une couleur neutre dans la barre (Anthony, 2026-10-28 : il prenait les couleurs neutres d'un projet neuf pour des
/// personnages et attendait leur calque) : nettement plus petite qu'un personnage, pour qu'on ne les confonde plus.
const PASTILLE_NEUTRE: f32 = 12.0;

/// Une pastille de diamètre `d`, dans une case de la hauteur d'une pastille de personnage (la rangée reste alignée).
fn pastille_taille(ui: &mut egui::Ui, ligne: [u8; 3], fond: Option<[u8; 3]>, choisie: bool, d: f32) -> Response {
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
    }
    resp
}

/// Le texte de la bulle d'une pastille.
fn description(ligne: [u8; 3], fond: Option<[u8; 3]>, choisie: bool, personnage: bool) -> String {
    let fond = match fond {
        Some(f) => format!("aplat {}", crate::couleur::hex(f)),
        None => "sans aplat".to_string(),
    };
    // (Un personnage dessine dans son calque, créé au premier trait ; une couleur neutre, dans le calque courant.)
    let clic = match (choisie, personnage) {
        (true, _) => "Clic : lâcher",
        (false, true) => "Clic : dessiner avec, dans un calque à son nom",
        (false, false) => "Clic : dessiner avec",
    };
    format!("Trait {} · {fond}. {clic} ; clic droit : modifier, dupliquer, supprimer, monter, descendre.", crate::couleur::hex(ligne))
}

impl App {
    // ------------------------------------------------------------------------------ ce qui est choisi

    /// Les couleurs de ce qui est choisi : (trait, fond).
    pub(crate) fn perso_couleurs(&self, c: Choix) -> Option<([u8; 3], Option<[u8; 3]>)> {
        match c {
            Choix::Personnage(id) => self.doc.personnages.iter().find(|p| p.id == id).map(|p| (p.line, p.fill)),
            Choix::Neutre(i) => self.settings.neutral_colors.get(i).map(|n| (n.line, n.fill)),
        }
    }

    /// Le nom de ce qui est choisi.
    fn perso_nom(&self, c: Choix) -> Option<String> {
        match c {
            Choix::Personnage(id) => self.doc.personnages.iter().find(|p| p.id == id).map(|p| p.name.clone()),
            Choix::Neutre(i) => self.settings.neutral_colors.get(i).map(|n| n.name.clone()),
        }
    }

    /// Les couleurs du personnage qui dessine, s'il y en a un.
    pub(crate) fn perso_couleurs_actives(&self) -> Option<([u8; 3], Option<[u8; 3]>)> {
        self.perso.actif.and_then(|c| self.perso_couleurs(c))
    }

    /// La couleur de fond du personnage qui dessine, sinon `autre` (le pot de peinture de la strate Fond).
    pub(crate) fn perso_fond_ou(&self, autre: [u8; 3]) -> [u8; 3] {
        self.perso_couleurs_actives().and_then(|(_, fond)| fond).unwrap_or(autre)
    }

    /// Tous les choix de la barre, dans l'ordre : les personnages, puis les couleurs neutres.
    fn perso_tous(&self) -> Vec<Choix> {
        let persos = self.doc.personnages.iter().map(|p| Choix::Personnage(p.id));
        persos.chain((0..self.settings.neutral_colors.len()).map(Choix::Neutre)).collect()
    }

    /// Choisit un personnage ou une couleur neutre : la couleur du trait devient la sienne, et ses traits auront
    /// son fond. Un outil qui ne dessine pas cède la place au pinceau.
    pub(crate) fn perso_choisir(&mut self, c: Choix) {
        let Some((ligne, fond)) = self.perso_couleurs(c) else { return };
        // Un personnage remplace la brosse du décor : l'outil, le pinceau, le calque d'avant reviennent d'abord.
        self.decor_lacher(true);
        self.end_stroke();
        // Un personnage dessine le trait ET son aplat (comme prévu) : on revient donc au trait si la sous-ligne d'un aplat
        // était choisie. (Cliquer ensuite sur l'aplat le reprend seul, avec la couleur d'aplat du personnage.)
        self.aplat_cible = None;
        if !matches!(self.tool, Tool::Brush | Tool::Pencil | Tool::Fill | Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Polyline | Tool::Text) {
            self.set_tool(Tool::Brush);
        }
        self.set_front(ligne);
        self.perso.actif = Some(c);
        self.perso.posee = ligne;
        // Un personnage qui a déjà son calque dans ce plan : il devient le calque courant (`calquespastilles.rs`).
        if let Choix::Personnage(id) = c {
            self.perso_rendre_son_calque(id);
        }
        let nom = self.perso_nom(c).unwrap_or_default();
        self.status = match (c, fond) {
            // (Le calque du personnage naît au premier trait, pas au clic : choix d'Anthony, 2026-10-28.)
            (Choix::Personnage(_), Some(_)) => format!("« {nom} » : le trait et l'aplat de ce personnage, dans son calque (créé au premier trait s'il n'existe pas encore)."),
            (Choix::Personnage(_), None) => format!("« {nom} » : le trait de ce personnage, sans aplat, dans son calque (créé au premier trait s'il n'existe pas encore)."),
            (Choix::Neutre(_), _) => format!("Couleur neutre « {nom} » : elle dessine dans le calque courant, sans créer de calque."),
        };
    }

    /// Lâche le personnage : la couleur reste, mais elle est libre.
    pub(crate) fn perso_lacher(&mut self) {
        // (La brosse du décor se lâche de la même façon : « aucun personnage » est la couleur libre.)
        if self.perso.decor.is_some() {
            self.decor_lacher(true);
            self.status = "Couleur libre : plus de brosse du décor.".into();
            return;
        }
        if self.perso.actif.take().is_some() {
            self.status = "Couleur libre : plus de personnage choisi.".into();
        }
    }

    /// La pastille « classique » (Anthony, 2026-10-23) : ce que faisait « aucun personnage » (plus de personnage ni de
    /// brosse du décor, la couleur reste libre), et le retour au pinceau de base : l'outil Pinceau au réglage
    /// « Pinceau rond », le premier pinceau de SHOT et le réglage de départ de l'outil (`BrushLibrary::builtin`).
    pub(crate) fn perso_classique(&mut self) {
        self.perso_lacher();
        self.end_stroke();
        self.aplat_cible = None;
        let rond = self.brushes.presets.iter().find(|p| p.target == crate::brushtex::Target::Brush && p.source == crate::pinceaux::PELURE && p.name == PINCEAU_DE_BASE);
        match rond {
            Some(p) => {
                self.brush = p.settings;
                self.brush_previews.base = Some((p.target, p.name.clone(), p.source.clone()));
            }
            None => self.brush = crate::brush::BrushSettings::brush(),
        }
        self.brush_previews.memory.clear();
        if self.tool != Tool::Brush {
            self.set_tool(Tool::Brush);
        }
        self.status = format!("Classique : le pinceau de base (« {PINCEAU_DE_BASE} »), sans personnage ni décor.");
    }

    /// Le personnage suivant (ou précédent) de la barre, en tournant.
    fn perso_voisin(&mut self, delta: isize) {
        let tous = self.perso_tous();
        if tous.is_empty() {
            self.status = "Aucun personnage : ajoute-en un (+ dans la colonne des pastilles).".into();
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
    fn perso_etape(&mut self, nom: &str) {
        let avant = self.history.context.replace(nom.to_string());
        self.record_step();
        self.history.context = avant;
    }

    /// Suit un champ ou un sélecteur de couleur d'un personnage (`App::edited`, une étape par geste, nommée) ; une
    /// couleur neutre, elle, est un réglage de l'application, sans annulation.
    fn perso_edited(&mut self, resp: &Response, c: Choix) -> bool {
        match c {
            Choix::Personnage(_) => {
                let avant = self.history.context.replace("Modifier le personnage".to_string());
                let fait = self.edited(resp);
                self.history.context = avant;
                fait
            }
            Choix::Neutre(_) => resp.changed(),
        }
    }

    /// Le nom, le trait et le fond de ce qui est choisi.
    fn perso_lire(&self, c: Choix) -> Option<(String, [u8; 3], Option<[u8; 3]>)> {
        let nom = self.perso_nom(c)?;
        let (ligne, fond) = self.perso_couleurs(c)?;
        Some((nom, ligne, fond))
    }

    /// Pose ces valeurs sur ce personnage (le projet change) ou cette couleur neutre (les réglages changent).
    pub(crate) fn perso_ecrire(&mut self, c: Choix, nom: String, ligne: [u8; 3], fond: Option<[u8; 3]>) {
        let nom = personnages::nom_net(&nom);
        match c {
            Choix::Personnage(id) => {
                if let Some(p) = self.doc.personnages.iter_mut().find(|p| p.id == id) {
                    let ancien = std::mem::replace(&mut p.name, nom.clone());
                    (p.line, p.fill) = (ligne, fond);
                    self.modified = true;
                    // Renommer le personnage renomme ses calques (`calquespastilles.rs`).
                    self.doc.renommer_calques_de_personnage(id, &ancien, &nom);
                }
            }
            Choix::Neutre(i) => {
                if let Some(n) = self.settings.neutral_colors.get_mut(i) {
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
        self.doc.personnages.push(Personnage { id, name, line, fill: Some(fill) });
        self.perso_choisir(Choix::Personnage(id));
        Some(id)
    }

    /// Une couleur neutre de plus (réglage de l'application).
    pub(crate) fn perso_ajouter_neutre(&mut self) -> Option<usize> {
        if self.settings.neutral_colors.len() >= personnages::MAX_NEUTRES {
            self.status = format!("{} couleurs neutres au plus.", personnages::MAX_NEUTRES);
            return None;
        }
        let n = self.settings.neutral_colors.len() + 1;
        self.settings.neutral_colors.push(Neutre { name: format!("Neutre {n}"), line: [128, 128, 128], fill: None });
        self.settings_a_ecrire = true;
        Some(self.settings.neutral_colors.len() - 1)
    }

    /// Duplique ce personnage ou cette couleur neutre, juste après lui.
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
            Choix::Neutre(i) => {
                if i >= self.settings.neutral_colors.len() || self.settings.neutral_colors.len() >= personnages::MAX_NEUTRES {
                    return;
                }
                let mut copie = self.settings.neutral_colors[i].clone();
                copie.name = personnages::nom_net(&format!("{} (copie)", copie.name));
                self.settings.neutral_colors.insert(i + 1, copie);
                self.settings_a_ecrire = true;
                // Les rangs suivants ont bougé : un choix de couleur neutre plus loin suit.
                if let Some(Choix::Neutre(a)) = self.perso.actif {
                    if a > i {
                        self.perso.actif = Some(Choix::Neutre(a + 1));
                    }
                }
            }
        }
    }

    /// Supprime ce personnage ou cette couleur neutre. Ce qui est déjà dessiné ne change pas (un trait ne se
    /// souvient pas de son personnage).
    pub(crate) fn perso_supprimer(&mut self, c: Choix) {
        match c {
            Choix::Personnage(id) => {
                if self.content_refuses() || !self.doc.personnages.iter().any(|p| p.id == id) {
                    return;
                }
                self.perso_etape("Supprimer le personnage");
                self.doc.personnages.retain(|p| p.id != id);
            }
            Choix::Neutre(i) => {
                if i >= self.settings.neutral_colors.len() {
                    return;
                }
                self.settings.neutral_colors.remove(i);
                self.settings_a_ecrire = true;
                if let Some(Choix::Neutre(a)) = self.perso.actif {
                    if a > i {
                        self.perso.actif = Some(Choix::Neutre(a - 1));
                    }
                }
            }
        }
        if self.perso.actif == Some(c) {
            self.perso.actif = None;
        }
        if self.perso.edition == Some(c) {
            self.perso.edition = None;
        }
    }

    /// Déplace ce personnage ou cette couleur neutre de `delta` places (négatif : vers la gauche).
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
            Choix::Neutre(i) => {
                let actif = match self.perso.actif {
                    Some(Choix::Neutre(a)) => Some(a),
                    _ => None,
                };
                let Some(to) = personnages::deplacer(&mut self.settings.neutral_colors, i, delta) else { return };
                self.settings_a_ecrire = true;
                // Le choix suit sa couleur.
                if let Some(a) = actif {
                    let suivi = if a == i {
                        to
                    } else if i < to && (i..=to).contains(&a) {
                        a - 1
                    } else if to < i && (to..=i).contains(&a) {
                        a + 1
                    } else {
                        a
                    };
                    self.perso.actif = Some(Choix::Neutre(suivi));
                }
            }
        }
    }

    /// Range le personnage du rang `from` à la place du rang `to` (glisser dans le panneau).
    fn perso_glisser(&mut self, from: usize, to: usize) {
        if from == to || from >= self.doc.personnages.len() || to >= self.doc.personnages.len() || self.content_refuses() {
            return;
        }
        self.perso_etape("Ranger les personnages");
        let p = self.doc.personnages.remove(from);
        self.doc.personnages.insert(to, p);
    }

    // ------------------------------------------------------------------------ commandes, tic, fond

    /// La barre est-elle montrée dans cet espace de travail ?
    pub(crate) fn barre_perso_visible(&self) -> bool {
        self.layout.character_bar.unwrap_or_else(|| self.espace.barre_personnages_par_defaut())
    }

    /// Les commandes de la brosse par personnage ; vrai si `action` en était une.
    pub(crate) fn perso_action(&mut self, action: &str) -> bool {
        match action {
            "perso.barre" => {
                self.layout.character_bar = Some(!self.barre_perso_visible());
                self.status = if self.barre_perso_visible() { "Pastilles des personnages montrées dans cet espace de travail." } else { "Pastilles des personnages cachées dans cet espace de travail." }.into();
            }
            "perso.ajouter" => {
                if let Some(id) = self.perso_ajouter() {
                    self.perso.edition = Some(Choix::Personnage(id));
                }
            }
            "perso.suivant" => self.perso_voisin(1),
            "perso.precedent" => self.perso_voisin(-1),
            "perso.aucun" => self.perso_lacher(),
            "perso.classique" => self.perso_classique(),
            _ => return false,
        }
        true
    }

    /// À chaque image d'écran : les fonds de trait arrivés sont posés, et le personnage choisi suit sa couleur (ou
    /// est lâché si quelqu'un a pris une autre couleur à la main, ou si le personnage n'existe plus). Au repos, rien.
    pub(crate) fn tick_personnages(&mut self) {
        self.decor_tick();
        if let Some(fil) = self.perso.fil.as_mut() {
            if fil.en_cours() > 0 {
                for f in fil.recevoir() {
                    self.perso_poser(f);
                }
            }
        }
        let Some(choix) = self.perso.actif else { return };
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
            self.perso.actif = None;
            return;
        }
        let Some((_, Some(_))) = self.perso_couleurs(c) else { return };
        if self.aplat_en_cible() || self.layer().fond.is_some() || self.griffonne_ici() {
            return;
        }
        let li = self.cur_layer;
        let avant = self.history.context.replace("Aplat du personnage".to_string());
        self.toggle_fond(li);
        self.history.context = avant;
        // Ce qui était déjà dessiné sur ce calque n'est pas à un personnage : il reste sans fond (le fond du calque
        // se pose sur tout son trait), coupé comme le ferait un personnage sans fond.
        let Some(reglages) = self.layer().fond else { return };
        let Some(cel) = self.cel_here() else { return };
        let Some(c) = self.doc.cels.get(&cel).filter(|c| !c.raster.is_blank()) else { return };
        let apres = c.raster.clone();
        let avant = crate::raster::Raster::new(apres.width, apres.height);
        self.perso_envoyer(Travail { cel, serial: u64::MAX, reglages, couleur: None, avant, apres });
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
        let Some((_, fond)) = self.perso_couleurs_actives() else { return };
        let Some(reglages) = self.layers().get(self.cur_layer).and_then(|l| l.fond) else { return };
        let Some(c) = self.doc.cels.get(&cel) else { return };
        let apres = c.raster.clone();
        let mut avant = apres.clone();
        for (i, t) in tiles {
            avant.replace_slot(i, t);
        }
        let serial = self.history.serial();
        self.perso_envoyer(Travail { cel, serial, reglages, couleur: fond, avant, apres });
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
        if !edits.is_empty() && self.history.serial() == f.serial {
            self.history.extend_last(Edit::Group(edits));
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

    // ------------------------------------------------------------------------------- la barre

    /// Les pastilles (Anthony, 2026-10-21 : une colonne ronde et discrète à droite du dessin ; 2026-10-24 : une rangée dans la barre
    /// du haut, voir plus bas).
    ///
    /// Dans l'ordre : « classique » (la couleur libre), les personnages, les couleurs neutres, les brosses du décor, « + » (un
    /// personnage de plus), et l'icône du panneau Personnages. Dans la barre, ce sont de vrais éléments de la barre : cliquer une pastille
    /// ne trace rien. Rien n'est calculé de lourd : une dizaine de cercles.
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
        let neutres: Vec<Neutre> = self.settings.neutral_colors.clone();
        let decor = self.decor_offert();
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
                        self.colonne_contenu(ui, &persos, &neutres, decor, &mut cases);
                    });
                });
            });
        self.perso.cases = cases;
        sortie.content_size.x
    }
    /// Les pastilles de la colonne, de haut en bas.
    fn colonne_contenu(&mut self, ui: &mut egui::Ui, persos: &[Personnage], neutres: &[Neutre], decor: bool, cases: &mut Vec<(Rect, Case)>) {
        let r = self.pastille_classique(ui, persos.is_empty());
        cases.push((r.rect, Case::Classique));
        trait_de_colonne(ui);
        for p in persos {
            let r = self.pastille_de_colonne(ui, Choix::Personnage(p.id), &p.name, p.line, p.fill);
            cases.push((r.rect, Case::Choix(Choix::Personnage(p.id))));
        }
        if !persos.is_empty() && !neutres.is_empty() {
            trait_de_colonne(ui);
        }
        for (i, n) in neutres.iter().enumerate() {
            let r = self.pastille_de_colonne(ui, Choix::Neutre(i), &n.name, n.line, n.fill);
            cases.push((r.rect, Case::Choix(Choix::Neutre(i))));
        }
        // Les brosses du décor (Storyboard et Vignettes, `decor_offert`), après un petit trait : des carrés arrondis,
        // pour ne pas les confondre avec les personnages (des ronds).
        if decor {
            trait_de_colonne(ui);
            for g in Genre::ALL {
                let r = self.pastille_decor(ui, g);
                cases.push((r.rect, Case::Decor(g)));
            }
        }
        trait_de_colonne(ui);
        let key = self.raccourci("perso.ajouter");
        let r = crate::bulles::icone(ui, ph::PLUS, "Nouveau personnage", "Ajouter un personnage : son nom, sa couleur de trait et sa couleur d'aplat.", &key);
        if r.clicked() {
            self.run_action("perso.ajouter");
        }
        cases.push((r.rect, Case::Plus));
        let ouvert = self.pane_shown(Pane::Personnages);
        let key = self.raccourci("vue.personnages");
        let r = ui.add(egui::Button::new(crate::bulles::symbole_etat(ph::USERS_THREE, ouvert)).selected(ouvert).frame(ouvert).min_size(vec2(COLONNE_L - 4.0, 0.0)));
        if crate::bulles::bulle(r.clone(), "Panneau Personnages", "Les noms, les couleurs de trait et d'aplat, l'ordre des personnages et les couleurs neutres. Un clic l'ouvre ou le ferme.", &key).clicked() {
            self.run_action("vue.personnages");
        }
        cases.push((r.rect, Case::Panneau));
    }

    /// La pastille « classique » (avant : « aucun personnage ») : un anneau avec un petit pinceau, allumé quand ni
    /// personnage ni brosse du décor ne dessine ; un clic revient au pinceau de base sans personnage ni décor
    /// (`perso_classique`). Pas de texte : la bulle le dit.
    fn pastille_classique(&mut self, ui: &mut egui::Ui, sans_personnage: bool) -> Response {
        let libre = self.perso.actif.is_none() && self.perso.decor.is_none();
        let (rect, resp) = ui.allocate_exact_size(vec2(PASTILLE + 6.0, PASTILLE + 6.0), Sense::click());
        if ui.is_rect_visible(rect) {
            let c = rect.center();
            let r = PASTILLE / 2.0 - 2.0;
            let p = ui.painter();
            if libre {
                p.circle_filled(c, r + 4.5, crate::theme::accent_texte());
                p.circle_filled(c, r + 1.5, crate::theme::entete());
            } else if resp.hovered() {
                p.circle_stroke(c, r + 3.0, Stroke::new(1.5_f32, crate::theme::bord()));
            }
            let encre = crate::theme::texte_doux();
            p.circle_stroke(c, r, Stroke::new(1.5_f32, encre));
            p.text(c, egui::Align2::CENTER_CENTER, ph::PAINT_BRUSH, egui::FontId::proportional(11.0), encre);
        }
        let description = if sans_personnage {
            "Le pinceau de base (« Pinceau rond ») avec la couleur libre, sans personnage ni brosse du décor : il dessine dans le calque courant, sans rien créer. Le « + » en bas de la colonne ajoute un personnage."
        } else {
            "Le pinceau de base (« Pinceau rond ») avec la couleur libre, sans personnage ni brosse du décor : il dessine dans le calque courant, sans rien créer. Un clic sur la pastille choisie la lâche aussi, sans changer de pinceau."
        };
        let resp = crate::bulles::bulle(resp, "Classique", description, &self.raccourci("perso.classique"));
        if resp.clicked() {
            self.perso_classique();
        }
        resp
    }

    /// Une pastille de la colonne : clic = choisir (ou lâcher), clic droit = son menu.
    fn pastille_de_colonne(&mut self, ui: &mut egui::Ui, c: Choix, nom: &str, ligne: [u8; 3], fond: Option<[u8; 3]>) -> Response {
        let choisie = self.perso.actif == Some(c);
        let d = if matches!(c, Choix::Neutre(_)) { PASTILLE_NEUTRE } else { PASTILLE };
        let resp = pastille_taille(ui, ligne, fond, choisie, d);
        let resp = crate::bulles::bulle(resp, if nom.is_empty() { "Sans nom" } else { nom }, &description(ligne, fond, choisie, matches!(c, Choix::Personnage(_))), "");
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
        let (rang, total) = match c {
            Choix::Personnage(id) => (self.doc.personnages.iter().position(|p| p.id == id), self.doc.personnages.len()),
            Choix::Neutre(i) => (Some(i), self.settings.neutral_colors.len()),
        };
        let Some(rang) = rang else {
            ui.close_menu();
            return;
        };
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

    /// Une ligne de réglages : le nom, la couleur du trait, le fond (ou non) et sa couleur.
    fn perso_ligne(&mut self, ui: &mut egui::Ui, c: Choix) {
        let Some((mut nom, mut ligne, mut fond)) = self.perso_lire(c) else { return };
        let mut change = false;
        let resp = ui.add(egui::TextEdit::singleline(&mut nom).desired_width(110.0).char_limit(personnages::NOM_MAX).hint_text("Nom"));
        let resp = crate::bulles::bulle(resp, "Nom", "Le nom, montré dans la bulle de la pastille.", "");
        change |= self.perso_edited(&resp, c);
        let resp = ui.color_edit_button_srgb(&mut ligne);
        let resp = crate::bulles::bulle(resp, "Couleur du trait", "La couleur dont ce personnage dessine.", "");
        change |= self.perso_edited(&resp, c);
        let mut avec = fond.is_some();
        let resp = crate::bulles::interrupteur(ui, &mut avec, ph::PAINT_BUCKET, "Avec un aplat", "L'aplat : la couleur sous le trait, dans tout ce que le trait enferme. Éteint, le personnage dessine sans aplat.", "");
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
            let resp = ui.color_edit_button_srgb(f);
            let resp = crate::bulles::bulle(resp, "Couleur de l'aplat", "La couleur de l'aplat sous le trait de ce personnage.", "");
            change |= self.perso_edited(&resp, c);
        }
        if change {
            self.perso_ecrire(c, nom, ligne, fond);
        }
    }

    /// La fenêtre « Modifier » d'un personnage ou d'une couleur neutre (clic droit sur sa pastille).
    pub(crate) fn personnages_fenetre(&mut self, ctx: &egui::Context) {
        let Some(c) = self.perso.edition else { return };
        if self.perso_couleurs(c).is_none() {
            self.perso.edition = None;
            return;
        }
        let titre = match c {
            Choix::Personnage(_) => "Modifier le personnage",
            Choix::Neutre(_) => "Modifier la couleur neutre",
        };
        let mut ouverte = true;
        egui::Window::new(titre).id(Id::new("perso_modifier")).collapsible(false).resizable(false).open(&mut ouverte).show(ctx, |ui| {
            ui.horizontal(|ui| {
                self.perso_ligne(ui, c);
            });
            if matches!(c, Choix::Neutre(_)) {
                ui.label(RichText::new("Une couleur neutre est un réglage de SHOT : elle sert dans tous les projets.").small().weak());
            }
        });
        if !ouverte {
            self.perso.edition = None;
        }
    }

    /// Le panneau Personnages : la liste des personnages du projet (qu'on range en glissant la poignée), puis les
    /// couleurs neutres de l'application.
    pub(crate) fn personnages_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.bouton(ui, ph::PLUS, "Nouveau personnage", "Ajouter un personnage : son nom, sa couleur de trait et sa couleur d'aplat.", "perso.ajouter").clicked() {
                self.run_action("perso.ajouter");
            }
            if self.bouton_si(ui, self.perso.actif.is_some(), ph::X_CIRCLE, "Couleur libre", "Lâcher le personnage : le trait garde sa couleur, mais elle n'est plus celle d'un personnage.", "perso.aucun").clicked() {
                self.perso_lacher();
            }
            let n = self.doc.personnages.len();
            ui.label(RichText::new(format!("{n} personnage{}", if n > 1 { "s" } else { "" })).small().weak());
        });
        ui.add_space(2.0);
        if self.doc.personnages.is_empty() {
            ui.label(RichText::new("Aucun personnage. Ajoute-en un : on le choisit d'un clic, et ses traits ont sa couleur de trait et son aplat.").weak());
        }
        let ids: Vec<u64> = self.doc.personnages.iter().map(|p| p.id).collect();
        let mut glisse: Option<(usize, usize)> = None;
        for (i, id) in ids.iter().copied().enumerate() {
            let c = Choix::Personnage(id);
            let (ligne, fond) = self.perso_couleurs(c).unwrap_or(([0; 3], None));
            let (zone, depose) = ui.dnd_drop_zone::<usize, _>(egui::Frame::NONE, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    let poignee = ui.dnd_drag_source(Id::new(("perso_poignee", id)), i, |ui| ui.label(RichText::new(ph::DOTS_SIX_VERTICAL).size(crate::bulles::TAILLE).weak())).response;
                    crate::bulles::bulle(poignee, "Ranger", "Glisser pour changer la place du personnage dans la barre.", "");
                    let choisie = self.perso.actif == Some(c);
                    if pastille(ui, ligne, fond, choisie).clicked() {
                        if choisie {
                            self.perso_lacher();
                        } else {
                            self.perso_choisir(c);
                        }
                    }
                    self.perso_ligne(ui, c);
                    if crate::bulles::icone(ui, ph::TRASH, "Supprimer le personnage", "Le retirer de la barre. Ce qui est déjà dessiné ne change pas.", "").clicked() {
                        self.perso_supprimer(c);
                    }
                });
            });
            if let Some(src) = depose {
                glisse = Some((*src, i));
            } else if zone.response.dnd_hover_payload::<usize>().is_some() {
                let r = zone.response.rect;
                ui.painter().hline(r.x_range(), r.top(), Stroke::new(2.0_f32, crate::theme::accent_texte()));
            }
        }
        if let Some((from, to)) = glisse {
            self.perso_glisser(from, to);
        }
        ui.add_space(6.0);
        ui.separator();
        ui.label(RichText::new("Couleurs neutres").strong());
        ui.label(RichText::new("Pour tous les projets : un trait, avec ou sans aplat, qui n'est pas un personnage.").small().weak());
        let n = self.settings.neutral_colors.len();
        for i in 0..n {
            let c = Choix::Neutre(i);
            let Some((ligne, fond)) = self.perso_couleurs(c) else { continue };
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                let choisie = self.perso.actif == Some(c);
                if pastille(ui, ligne, fond, choisie).clicked() {
                    if choisie {
                        self.perso_lacher();
                    } else {
                        self.perso_choisir(c);
                    }
                }
                self.perso_ligne(ui, c);
                if crate::bulles::icone(ui, ph::TRASH, "Supprimer la couleur neutre", "La retirer de la barre.", "").clicked() {
                    self.perso_supprimer(c);
                }
            });
            // (Supprimée : les rangs ont bougé, on reprend à l'image suivante.)
            if self.settings.neutral_colors.len() != n {
                break;
            }
        }
        ui.horizontal(|ui| {
            if crate::bulles::icone(ui, ph::PLUS, "Nouvelle couleur neutre", "Ajouter une couleur neutre (un trait, avec ou sans aplat).", "").clicked() {
                self.perso_ajouter_neutre();
            }
            if crate::bulles::icone(ui, ph::ARROW_COUNTER_CLOCKWISE, "Couleurs neutres d'origine", "Remettre le noir, le gris et le rouge de correction.", "").clicked() {
                self.settings.neutral_colors = personnages::neutres_par_defaut();
                self.settings_a_ecrire = true;
                if matches!(self.perso.actif, Some(Choix::Neutre(_))) {
                    self.perso.actif = None;
                }
            }
        });
    }
}
