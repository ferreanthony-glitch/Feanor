//! Fenêtres modulables (5b, choix d'Anthony) : chaque panneau est un onglet que l'on glisse,
//! qui s'aimante sur un bord ou dans un autre panneau, ou flotte dans la fenêtre de Pelure.
//!
//! L'affichage et les gestes viennent de la bibliothèque egui_dock. La disposition, elle, est
//! retenue dans un format propre à Pelure (`DockLayout` : un arbre d'onglets et de partages, plus
//! les panneaux flottants) : les espaces de travail enregistrés ne dépendent pas de la version
//! de la bibliothèque, et ce format se teste sans interface.

use egui_dock::{DockState, Node, NodeIndex, Split, Surface, TabIndex, Tree};
use eframe::egui::{pos2, vec2};
use serde::{Deserialize, Serialize};

use crate::colonne::{Colonne, PanneauPile};
use crate::workspace::Layout;

/// Un panneau de la fenêtre.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Pane {
    /// Le dessin (ou les vignettes du storyboard). Toujours présent.
    #[serde(rename = "dessin")]
    Canvas,
    /// Les icônes des outils.
    #[serde(rename = "outils")]
    Tools,
    #[serde(rename = "couleur")]
    Color,
    /// Ancien panneau Pinceaux (11c), en onglet avec la couleur : sa liste de brosses est depuis le 2026-10-24 dans le
    /// panneau Réglages de l'outil, derrière le bouton à droite de l'aperçu du trait (Anthony : « un seul panneau »).
    /// Gardé pour lire les dispositions enregistrées ; `sanitized` l'enlève, sans laisser de trou.
    #[serde(rename = "pinceaux")]
    Brushes,
    /// Réglages de l'outil courant.
    #[serde(rename = "reglages")]
    ToolOptions,
    /// Ancien panneau Vue (ajuster, zoom, miroir, rotation, table lumineuse) : ses outils sont
    /// sous le dessin depuis le 2026-10-17 (Anthony : « la fenêtre Vue devient inutile »). Gardé
    /// pour lire les dispositions enregistrées ; `sanitized` l'enlève.
    #[serde(rename = "vue")]
    View,
    /// Ancien panneau Guides (perspective) : ses fonctions sont dans le menu de la règle, au-dessus du dessin
    /// (Anthony, 2026-10-21 : la règle est le seul lieu des guides). Gardé pour lire les dispositions
    /// enregistrées ; `sanitized` l'enlève.
    #[serde(rename = "guides")]
    Guides,
    /// Fiche du panneau (textes, durée, caméra).
    #[serde(rename = "fiche")]
    Sheet,
    #[serde(rename = "calques")]
    Layers,
    /// La bande du bas : le montage (tous les plans, leurs pistes, le son) en storyboard, la
    /// timeline du plan en animation (on y entre par un double-clic sur le plan).
    #[serde(rename = "bande")]
    Montage,
    /// Ancienne timeline à part (chantier 6, le 2026-10-17) : retirée le même jour à la demande
    /// d'Anthony (« je ne veux pas de double timeline »). Gardée pour lire les dispositions
    /// enregistrées entre-temps ; `sanitized` l'enlève.
    #[serde(rename = "timeline")]
    Timeline,
    /// Feuille d'exposition du plan (13g). Fermée dans la disposition par défaut.
    #[serde(rename = "feuille_expo")]
    XSheet,
    /// Effets du calque courant (pile d'effets non destructifs, 13i) : un panneau de l'espace de
    /// travail (Anthony, 2026-10-17 : « la partie effets doit être une fenêtre dans l'espace de
    /// travail »), sous les calques depuis la disposition 8 (son choix).
    #[serde(rename = "effets")]
    Effects,
    /// Les étapes d'annulation, par leur nom (axe d'ergonomie 7, `historyui.rs`). Fermé dans la
    /// disposition par défaut ; il s'ouvre en onglet avec les calques.
    #[serde(rename = "historique")]
    History,
    /// Les commentaires de la timeline (`commentairesui.rs`) : la liste, et l'éditeur du commentaire
    /// choisi (texte, auteur, couleur). Fermé dans la disposition par défaut ; il s'ouvre dans la
    /// colonne de droite, en onglet avec les calques.
    #[serde(rename = "commentaires")]
    Comments,
    /// La bibliothèque de bruitages (`bruitages.rs`) : les dossiers de sons de l'utilisateur, à
    /// chercher, écouter et poser dans le montage. Fermé dans la disposition par défaut ; il s'ouvre
    /// dans la colonne de droite, en onglet avec les calques.
    #[serde(rename = "bruitages")]
    Bruitages,
    /// Les préréglages (`panneauprereglages.rs`, partie 170) : les mouvements de caméra tout faits, en vignettes animées, à
    /// glisser sur la piste Caméra de la timeline. Fermé dans la disposition par défaut ; il s'ouvre dans la colonne de
    /// droite, en onglet avec les calques.
    #[serde(rename = "prereglages")]
    Prereglages,
    /// Les suggestions sonores (`suggestionsui.rs`) : SHOT lit le storyboard et propose des bruitages,
    /// des ambiances et des voix témoins, qu'on écoute, accepte ou supprime. Fermé dans la disposition
    /// par défaut ; il s'ouvre dans la colonne de droite, en onglet avec les calques.
    #[serde(rename = "suggestions_sonores")]
    SuggestionsSonores,
    /// Les références visuelles du projet (`references.rs`, `referencesui.rs`) : un tableau libre
    /// d'images et de vidéos à regarder pendant qu'on dessine. Fermé dans la disposition par défaut ;
    /// il s'ouvre dans la colonne de droite, et se détache dans sa fenêtre pour un deuxième écran.
    #[serde(rename = "references")]
    References,
    /// Les personnages du projet et les couleurs neutres (`personnagesui.rs`) : leurs couleurs de trait et de
    /// fond, leur ordre. En onglet avec la couleur dans la disposition par défaut.
    #[serde(rename = "personnages")]
    Personnages,
    /// Le panneau « Vignettes » (`vignettespanneau.rs`, 2026-10-24) : le sens de lecture de la planche (en lignes, ou à la japonaise) et
    /// les textes montrés à côté des cases (action, dialogue, notes). Il est dans la colonne de droite de l'espace Vignettes ; fermé
    /// ailleurs, où il s'ouvre à la demande (il règle la planche de l'espace Vignettes).
    #[serde(rename = "vignettes")]
    Vignettes,
    /// « Mon panneau » (`monpanneau.rs`, essai du menu Labo, 2026-10-28) : les commandes, les brosses et les outils perso que
    /// l'utilisateur y a rangés. Fermé d'office ; s'ouvre depuis le menu Labo, et se place où l'on veut comme les autres.
    #[serde(rename = "mon_panneau")]
    MonPanneau,
    /// Le panneau « Scénario » (`scenariopanneau.rs`, 2026-10-30) : le scénario importé, lisible comme un scénario, lié aux plans
    /// qui en viennent. En onglet avec la couleur dans l'espace Vignettes, avec les calques dans le Storyboard ; fermé ailleurs
    /// (il s'ouvre tout seul après un import).
    #[serde(rename = "scenario")]
    Scenario,
    /// Le panneau « Bibliothèque » (`biblio.rs`, `biblioui.rs`, 2026-10-30) : explorer les fichiers de l'ordinateur, et lire les
    /// images, vidéos et sons dans un aperçu intégré. Lecture seule ; fermé d'office, il s'ouvre dans la colonne de droite.
    #[serde(rename = "bibliotheque")]
    Bibliotheque,
    /// Le panneau « Espace 3D » (`espace3dpanneau.rs`, 2026-11-12, `docs/INTERFACE.md` partie 150) : les réglages de la vue
    /// Espace 3D (options de la vue, cartes, caméra du plan), sortis du panneau Réglages, qui montre toujours l'outil choisi.
    /// Sa place (Anthony, 2026-11-13, partie 152 : « Onglet de Réglages ») : un onglet du groupe des réglages de l'outil,
    /// après les onglets qui y sont (Réglages, la fiche du plan). Il y arrive, et passe devant, quand la vue s'ouvre ;
    /// Réglages repasse devant quand elle se ferme, et l'onglet reste. Rangé ailleurs à la main, il reste où on l'a mis.
    #[serde(rename = "espace3d")]
    Espace3d,
    /// La colonne des panneaux en pile (`colonne.rs`, `colonneui.rs`, partie 194) : un seul panneau d'egui_dock, à droite du
    /// dessin et au-dessus de la bande du bas, qui dessine lui-même sa pile (`DockLayout::colonne`). Pas dans `Pane::ALL` : il
    /// ne s'ouvre ni ne se ferme à la main, il est là tant que la pile a un panneau.
    #[serde(rename = "colonne")]
    Colonne,
    /// Le panneau « Scène » (`scene.rs`, `sceneui.rs`, partie 197) : du film jusqu'aux calques, la colonne de la timeline sans le
    /// temps. En haut de la pile d'origine du Storyboard ; dans l'Animation, par « + Panneau ».
    #[serde(rename = "scene")]
    Scene,
}

/// Où un panneau se range quand on le rouvre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    /// Colonne de gauche : outils, couleur, réglages, vue, guides.
    Left,
    /// Colonne de droite : fiche et calques.
    Right,
    /// En bas, sur toute la largeur : le montage ou la timeline, selon le mode.
    Bottom,
    Center,
    /// Les réglages de la planche des Vignettes : une petite colonne à droite du dessin.
    Planche,
}

impl Pane {
    pub const ALL: [Pane; 21] = [
        Pane::Canvas,
        Pane::Tools,
        Pane::Scene,
        Pane::Color,
        Pane::ToolOptions,
        Pane::Espace3d,
        Pane::Sheet,
        Pane::Montage,
        Pane::XSheet,
        Pane::Effects,
        Pane::History,
        Pane::Comments,
        Pane::Bruitages,
        Pane::Prereglages,
        Pane::SuggestionsSonores,
        Pane::References,
        Pane::Personnages,
        Pane::Vignettes,
        Pane::MonPanneau,
        Pane::Scenario,
        Pane::Bibliotheque,
    ];

    /// Les panneaux fermés dans la disposition par défaut, qui ne s'ouvrent qu'à la demande. (Le panneau Espace 3D en est
    /// depuis la partie 152 : il arrive en onglet des réglages de l'outil quand la vue Espace 3D s'ouvre, ou quand on
    /// l'ouvre à la main, et y reste.)
    pub fn on_demand(self) -> bool {
        matches!(
            self,
            Pane::XSheet | Pane::History | Pane::Comments | Pane::Bruitages | Pane::Prereglages | Pane::SuggestionsSonores | Pane::References | Pane::Vignettes | Pane::MonPanneau | Pane::Scenario | Pane::Bibliotheque | Pane::Espace3d | Pane::Scene
        )
    }

    /// Les panneaux qui n'existent qu'en animation (Anthony, 2026-10-18 : « Exposition n'est
    /// disponible que dans la partie animation ») : en storyboard, leur icône, leur entrée de menu et
    /// leur action sont masquées, et le panneau n'est pas montré (`App::masquer_exposition`).
    pub fn animation_only(self) -> bool {
        matches!(self, Pane::XSheet)
    }

    pub fn name(self) -> &'static str {
        match self {
            Pane::Canvas => "Dessin",
            Pane::Tools => "Outils",
            Pane::Color => "Couleur",
            Pane::Brushes => "Pinceaux",
            Pane::ToolOptions => "Réglages de l'outil",
            Pane::View => "Vue",
            Pane::Guides => "Guides",
            Pane::Sheet => "Fiche du plan",
            Pane::Layers => "Calques",
            Pane::Montage => "Montage et timeline",
            Pane::Timeline => "Timeline",
            Pane::XSheet => "Feuille d'exposition",
            Pane::Effects => "Effets",
            Pane::History => "Historique",
            Pane::Comments => "Commentaires",
            Pane::Bruitages => "Bruitages",
            Pane::Prereglages => "Préréglages",
            Pane::SuggestionsSonores => "Suggestions sonores",
            Pane::References => "Références",
            Pane::Personnages => "Brosses dédiées",
            Pane::Vignettes => "Planche",
            Pane::MonPanneau => "Mon panneau",
            Pane::Scenario => "Scénario",
            Pane::Bibliotheque => "Bibliothèque",
            Pane::Espace3d => "Espace 3D",
            Pane::Colonne => "Colonne des panneaux",
            Pane::Scene => "Scène",
        }
    }

    /// Nom court, pour l'onglet.
    pub fn tab_name(self) -> &'static str {
        match self {
            Pane::ToolOptions => "Réglages",
            Pane::Sheet => "Fiche",
            Pane::XSheet => "Exposition",
            Pane::SuggestionsSonores => "Suggestions",
            p => p.name(),
        }
    }

    pub fn icon(self) -> &'static str {
        use egui_phosphor::regular as ph;
        match self {
            Pane::Canvas => ph::FRAME_CORNERS,
            Pane::Tools => ph::TOOLBOX,
            Pane::Color => ph::PALETTE,
            Pane::Brushes => ph::PAINT_BRUSH,
            Pane::ToolOptions => ph::SLIDERS_HORIZONTAL,
            Pane::View => ph::MAGNIFYING_GLASS,
            Pane::Guides => ph::RULER,
            Pane::Sheet => ph::CLIPBOARD_TEXT,
            Pane::Layers => ph::STACK,
            Pane::Montage => ph::FILM_STRIP,
            Pane::Timeline => ph::ROWS,
            Pane::XSheet => ph::TABLE,
            Pane::Effects => ph::MAGIC_WAND,
            Pane::History => ph::CLOCK_COUNTER_CLOCKWISE,
            Pane::Comments => ph::CHAT_TEXT,
            Pane::Bruitages => ph::WAVEFORM,
            Pane::Prereglages => ph::FILM_STRIP,
            Pane::SuggestionsSonores => ph::SPARKLE,
            Pane::References => ph::IMAGES,
            Pane::Personnages => ph::USERS_THREE,
            Pane::Vignettes => ph::SQUARES_FOUR,
            Pane::MonPanneau => ph::STAR,
            Pane::Scenario => ph::FILM_SCRIPT,
            Pane::Bibliotheque => ph::BOOKS,
            // (Le cube du bouton de la vue, dans la barre au-dessus du dessin.)
            Pane::Espace3d => ph::CUBE,
            Pane::Colonne => ph::SIDEBAR_SIMPLE,
            Pane::Scene => ph::TREE_VIEW,
        }
    }

    pub fn group(self) -> Group {
        match self {
            // (La colonne n'est d'aucun groupe qu'on rouvre : elle vient avec ses panneaux, `DockLayout::placer_la_colonne`.)
            Pane::Canvas | Pane::Colonne => Group::Center,
            Pane::Tools | Pane::Color | Pane::Brushes | Pane::Personnages | Pane::ToolOptions | Pane::View | Pane::Guides | Pane::Sheet | Pane::Espace3d => Group::Left,
            Pane::Layers
            | Pane::Scene
            | Pane::XSheet
            | Pane::Effects
            | Pane::History
            | Pane::Comments
            | Pane::Bruitages
            | Pane::Prereglages
            | Pane::SuggestionsSonores
            | Pane::MonPanneau
            | Pane::Scenario
            | Pane::Bibliotheque
            | Pane::References => Group::Right,
            Pane::Montage | Pane::Timeline => Group::Bottom,
            Pane::Vignettes => Group::Planche,
        }
    }
}

impl Group {
    pub fn panes(self) -> Vec<Pane> {
        Pane::ALL.into_iter().filter(|p| p.group() == self).collect()
    }
}

/// Un nœud de la disposition : un groupe d'onglets, ou deux parties côte à côte / l'une sur
/// l'autre (`part` : la part de la première, entre 0 et 1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DockNode {
    #[serde(rename = "onglets")]
    Tabs {
        panneaux: Vec<Pane>,
        #[serde(default)]
        actif: usize,
    },
    #[serde(rename = "cote_a_cote")]
    Row {
        part: f32,
        gauche: Box<DockNode>,
        droite: Box<DockNode>,
    },
    #[serde(rename = "dessus_dessous")]
    Column {
        part: f32,
        haut: Box<DockNode>,
        bas: Box<DockNode>,
    },
}

/// Panneaux flottants (position et taille en points de la fenêtre ; 0 = à placer).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FloatingPanel {
    pub panneaux: DockNode,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub largeur: f32,
    #[serde(default)]
    pub hauteur: f32,
}

/// Panneau dans sa propre fenêtre Windows (5g, par exemple sur un deuxième écran) : place de la
/// fenêtre et taille de son contenu, en points de l'écran (0 = à placer).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DetachedPanel {
    pub panneau: Pane,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub largeur: f32,
    #[serde(default)]
    pub hauteur: f32,
}

/// La disposition des panneaux.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DockLayout {
    /// `LAYOUT_VERSION` ; absent (0) : disposition d'avant 6a.
    #[serde(default)]
    pub version: u32,
    pub principal: DockNode,
    #[serde(default)]
    pub fenetres: Vec<FloatingPanel>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub detachees: Vec<DetachedPanel>,
    /// La pile de la colonne des panneaux (partie 194), que montre le panneau `Pane::Colonne` de `principal`. Chaque espace de
    /// travail a la sienne, puisqu'il a sa disposition.
    #[serde(default, skip_serializing_if = "Colonne::is_empty")]
    pub colonne: Colonne,
}

fn tabs(panes: Vec<Pane>) -> DockNode {
    DockNode::Tabs { panneaux: panes, actif: 0 }
}

fn row(part: f32, a: DockNode, b: DockNode) -> DockNode {
    DockNode::Row { part, gauche: Box::new(a), droite: Box::new(b) }
}

fn column(part: f32, a: DockNode, b: DockNode) -> DockNode {
    DockNode::Column { part, haut: Box::new(a), bas: Box::new(b) }
}

/// Les onglets des réglages : réglages de l'outil, fiche du panneau (9b) ; la vue est sous le dessin depuis
/// le 2026-10-17, et les guides dans le menu de la règle depuis le 2026-10-21.
const SETTINGS_TABS: [Pane; 2] = [Pane::ToolOptions, Pane::Sheet];

/// Les onglets du bas de la colonne : la couleur et les personnages (brosse par personnage). Les pinceaux n'y sont plus :
/// leur liste est dans les réglages de l'outil. Le panneau Espace 3D n'y est plus non plus (il y a été un jour, le
/// 2026-11-12, partie 150 : « en dessous de réglages, à la place de la couleur ») : il est un onglet du groupe des
/// réglages de l'outil (`DockLayout::open`, partie 152).
const COLOR_TABS: [Pane; 2] = [Pane::Color, Pane::Personnages];

/// Remet `pane` parmi les onglets `tabs` de sa famille : à la fin pour les réglages ; pour la couleur et les
/// personnages, à leur rang (couleur, personnages), quels que soient ceux qui sont restés.
fn place_in_family(tabs: &mut Vec<Pane>, pane: Pane, family: &[Pane]) {
    if family != COLOR_TABS {
        tabs.push(pane);
        return;
    }
    let rank = |p: Pane| family.iter().position(|f| *f == p);
    let mine = rank(pane);
    let before = tabs.iter().rposition(|p| matches!((rank(*p), mine), (Some(a), Some(b)) if a < b));
    let after = tabs.iter().position(|p| matches!((rank(*p), mine), (Some(a), Some(b)) if a > b));
    let at = match (before, after) {
        (Some(i), _) => i + 1,
        (None, Some(j)) => j,
        (None, None) => tabs.len(),
    };
    tabs.insert(at, pane);
}

/// Largeur de la colonne des panneaux, en points de l'écran (partie 161) : celle de la maquette (240 pixels pour une fenêtre de
/// 1 440, barre des icônes comprise), un peu plus pour que la palette de couleurs, les curseurs et la liste de la bibliothèque
/// (nom, durée, taille) tiennent sans être coupés.
/// Elle ne suit pas la largeur de la fenêtre : l'image prend toute la place gagnée (`App::fit_columns`, `ajuste.rs`). Tirer
/// le bord à la main la fixe où on l'a mise, comme une hauteur.
pub const COLONNE: f32 = 290.0;

/// Version de la disposition : 1 depuis 6a (outils dans la barre verticale, calques à côté du
/// dessin, réglages et couleur à droite) ; 2 depuis 9b (la fiche avec les réglages) ; 3 depuis 11c
/// (le panneau Pinceaux, en onglet avec la couleur) ; 4 depuis le chantier 6 (la timeline, panneau à
/// part sous le montage) ; 5 le même jour (en onglets) ; 6 le même jour (une seule bande, comme
/// avant : le montage en storyboard, la timeline en animation) ; 7 le même jour (le panneau
/// Effets, sous les réglages) ; 8 le même jour (choix d'Anthony : les calques avec les effets
/// dessous, puis les réglages avec la couleur dessous) ; 9 depuis la partie 161 (une seule colonne étroite : deux colonnes de
/// panneaux côte à côte se fondent en une, `DockNode::une_seule_colonne`) ; 10 depuis la partie 194 (la colonne est une pile de
/// panneaux, `DockLayout::colonne`, que montre un seul panneau `Pane::Colonne` ; la bande du bas a toute la largeur, sous elle :
/// `DockLayout::vers_la_pile`).
pub const LAYOUT_VERSION: u32 = 10;

/// La bande du bas : une seule (Anthony, 2026-10-17 : « je ne veux pas de double timeline ») ;
/// elle montre le montage en storyboard, la timeline en animation.
fn bottom_band(panes: &[Pane]) -> Option<DockNode> {
    (panes.contains(&Pane::Montage) || panes.contains(&Pane::Timeline)).then(|| tabs(vec![Pane::Montage]))
}

/// Hauteur de la bande du bas (en points de la fenêtre de référence), et celle de la version 4
/// (montage et timeline l'un sous l'autre).
const BAND_HEIGHT: f32 = 290.0;
const BAND_HEIGHT_V4: f32 = 420.0;

/// Le panneau devant d'un groupe d'onglets (`actif` borné).
fn cote_actif(panes: &[Pane], actif: usize) -> Option<Pane> {
    panes.get(actif.min(panes.len().saturating_sub(1))).copied()
}

/// Taille de fenêtre de référence pour passer des tailles en pixels (anciennes dispositions) aux
/// parts de la nouvelle (largeur hors barre d'outils verticale).
const REF_WIDTH: f32 = 1454.0;
const REF_HEIGHT: f32 = 820.0;
// (La largeur de la colonne des réglages, 350 points, et la part du lecteur dans l'espace Montage, 0,7, ont laissé la place à
// `COLONNE`, partie 161.)

impl DockNode {
    fn collect(&self, out: &mut Vec<Pane>) {
        match self {
            DockNode::Tabs { panneaux, .. } => out.extend(panneaux.iter().copied()),
            DockNode::Row { gauche: a, droite: b, .. } | DockNode::Column { haut: a, bas: b, .. } => {
                a.collect(out);
                b.collect(out);
            }
        }
    }

    /// Onglets du premier groupe (en haut à gauche).
    fn first_tabs(&self) -> Vec<Pane> {
        match self {
            DockNode::Tabs { panneaux, .. } => panneaux.clone(),
            DockNode::Row { gauche: a, .. } | DockNode::Column { haut: a, .. } => a.first_tabs(),
        }
    }

    /// Le montage et la timeline posés l'un sous l'autre par la version 4 repassent en onglets ;
    /// la bande retrouve sa hauteur d'avant si elle avait celle de la version 4.
    fn strips_as_tabs(self) -> DockNode {
        let pair = |n: &DockNode| match n {
            DockNode::Column { haut, bas, .. } => {
                let only = |n: &DockNode, p: Pane| matches!(n, DockNode::Tabs { panneaux, .. } if panneaux.as_slice() == [p]);
                only(haut, Pane::Montage) && only(bas, Pane::Timeline)
            }
            _ => false,
        };
        let band = || tabs(vec![Pane::Montage, Pane::Timeline]);
        if pair(&self) {
            return band();
        }
        match self {
            DockNode::Column { part, haut, bas } if pair(&bas) => {
                let v4 = 1.0 - BAND_HEIGHT_V4 / REF_HEIGHT;
                let part = if (part - v4).abs() < 0.02 { 1.0 - BAND_HEIGHT / REF_HEIGHT } else { part };
                column(part, haut.strips_as_tabs(), band())
            }
            DockNode::Column { part, haut, bas } => column(part, haut.strips_as_tabs(), bas.strips_as_tabs()),
            DockNode::Row { part, gauche, droite } => row(part, gauche.strips_as_tabs(), droite.strips_as_tabs()),
            t => t,
        }
    }

    /// Une partie de côté : ni le dessin, ni la bande du bas.
    fn est_un_cote(&self) -> bool {
        let mut v = Vec::new();
        self.collect(&mut v);
        !v.iter().any(|p| matches!(p, Pane::Canvas | Pane::Montage | Pane::Timeline))
    }

    /// Les onglets du premier groupe (en haut à gauche), à changer.
    fn first_tabs_mut(&mut self) -> &mut Vec<Pane> {
        match self {
            DockNode::Tabs { panneaux, .. } => panneaux,
            DockNode::Row { gauche: a, .. } | DockNode::Column { haut: a, .. } => a.first_tabs_mut(),
        }
    }

    /// Une seule colonne de panneaux (partie 161) : deux parties de côté posées côte à côte (aucune n'a le dessin ni la bande)
    /// n'en font plus qu'une. Les onglets de celle de gauche rejoignent le groupe du haut de celle de droite, derrière ceux qui
    /// y sont (dans la disposition d'avant : les effets et le scénario derrière les réglages de l'outil et la fiche du plan).
    /// Seules sont fondues les colonnes de côté posées à côté du dessin : une rangée de panneaux rangée à la main ailleurs (sous le
    /// dessin, dans un coin) reste comme elle est.
    pub(crate) fn une_seule_colonne(self) -> DockNode {
        fn fondre(n: DockNode) -> DockNode {
            match n {
                DockNode::Row { part, gauche, droite } => {
                    let (g, mut d) = (fondre(*gauche), fondre(*droite));
                    if g.est_un_cote() && d.est_un_cote() {
                        let mut venus = Vec::new();
                        g.collect(&mut venus);
                        d.first_tabs_mut().extend(venus);
                        d
                    } else {
                        row(part, g, d)
                    }
                }
                t => t,
            }
        }
        let a_le_dessin = |n: &DockNode| {
            let mut v = Vec::new();
            n.collect(&mut v);
            v.contains(&Pane::Canvas)
        };
        match self {
            DockNode::Row { part, gauche, droite } => {
                let (g, d) = (*gauche, *droite);
                match (a_le_dessin(&g), a_le_dessin(&d)) {
                    (true, false) if d.est_un_cote() => row(part, g.une_seule_colonne(), fondre(d)),
                    (false, true) if g.est_un_cote() => row(part, fondre(g), d.une_seule_colonne()),
                    _ => row(part, g.une_seule_colonne(), d.une_seule_colonne()),
                }
            }
            DockNode::Column { part, haut, bas } => column(part, haut.une_seule_colonne(), bas.une_seule_colonne()),
            t => t,
        }
    }

    /// Sans `pane` ; rien si plus aucun onglet.
    fn without(self, pane: Pane) -> Option<DockNode> {
        self.sanitized(&mut vec![pane])
    }

    /// Doublons et panneaux de `seen` retirés, groupes vides supprimés, parts bornées.
    fn sanitized(self, seen: &mut Vec<Pane>) -> Option<DockNode> {
        match self {
            DockNode::Tabs { panneaux, actif } => {
                let active = panneaux.get(actif).copied();
                let mut kept = Vec::new();
                for p in panneaux {
                    if !seen.contains(&p) {
                        seen.push(p);
                        kept.push(p);
                    }
                }
                let actif = active.and_then(|a| kept.iter().position(|p| *p == a)).unwrap_or(0);
                (!kept.is_empty()).then_some(DockNode::Tabs { panneaux: kept, actif })
            }
            DockNode::Row { part, gauche, droite } => {
                let part = if part.is_finite() { part.clamp(0.05, 0.95) } else { 0.5 };
                match (gauche.sanitized(seen), droite.sanitized(seen)) {
                    (Some(a), Some(b)) => Some(row(part, a, b)),
                    (a, b) => a.or(b),
                }
            }
            DockNode::Column { part, haut, bas } => {
                let part = if part.is_finite() { part.clamp(0.05, 0.95) } else { 0.5 };
                match (haut.sanitized(seen), bas.sanitized(seen)) {
                    (Some(a), Some(b)) => Some(column(part, a, b)),
                    (a, b) => a.or(b),
                }
            }
        }
    }

    /// Le groupe d'onglets qui contient un des panneaux `panes`, s'il y en a un.
    fn tabs_with(&mut self, panes: &[Pane]) -> Option<&mut Vec<Pane>> {
        match self {
            DockNode::Tabs { panneaux, .. } => panneaux.iter().any(|p| panes.contains(p)).then_some(panneaux),
            DockNode::Row { gauche: a, droite: b, .. } | DockNode::Column { haut: a, bas: b, .. } => {
                match a.tabs_with(panes) {
                    Some(t) => Some(t),
                    None => b.tabs_with(panes),
                }
            }
        }
    }

    /// Le groupe d'onglets qui contient un panneau de `group`, s'il y en a un.
    fn tabs_of_group(&mut self, group: Group) -> Option<&mut Vec<Pane>> {
        match self {
            DockNode::Tabs { panneaux, .. } => panneaux.iter().any(|p| p.group() == group).then_some(panneaux),
            DockNode::Row { gauche: a, droite: b, .. } | DockNode::Column { haut: a, bas: b, .. } => {
                match a.tabs_of_group(group) {
                    Some(t) => Some(t),
                    None => b.tabs_of_group(group),
                }
            }
        }
    }

    /// Le groupe d'onglets qui contient `pane` : ses onglets et le rang de celui qui est devant.
    fn groupe_de(&mut self, pane: Pane) -> Option<(&Vec<Pane>, &mut usize)> {
        match self {
            DockNode::Tabs { panneaux, actif } => panneaux.contains(&pane).then_some((&*panneaux, actif)),
            DockNode::Row { gauche: a, droite: b, .. } | DockNode::Column { haut: a, bas: b, .. } => match a.groupe_de(pane) {
                Some(g) => Some(g),
                None => b.groupe_de(pane),
            },
        }
    }

    /// `pane` est un onglet d'un groupe qui a aussi l'un des panneaux `autres`.
    fn en_onglet_avec(&self, pane: Pane, autres: &[Pane]) -> bool {
        match self {
            DockNode::Tabs { panneaux, .. } => panneaux.contains(&pane) && panneaux.iter().any(|p| autres.contains(p)),
            DockNode::Row { gauche: a, droite: b, .. } | DockNode::Column { haut: a, bas: b, .. } => a.en_onglet_avec(pane, autres) || b.en_onglet_avec(pane, autres),
        }
    }

    /// `pane` est seul dans son groupe d'onglets, juste sous le groupe d'onglets qui contient `cible`.
    fn seul_sous(&self, pane: Pane, cible: Pane) -> bool {
        match self {
            DockNode::Tabs { .. } => false,
            DockNode::Column { haut, bas, .. } => {
                let ici = matches!(&**bas, DockNode::Tabs { panneaux, .. } if panneaux.as_slice() == [pane]) && matches!(&**haut, DockNode::Tabs { panneaux, .. } if panneaux.contains(&cible));
                ici || haut.seul_sous(pane, cible) || bas.seul_sous(pane, cible)
            }
            DockNode::Row { gauche, droite, .. } => gauche.seul_sous(pane, cible) || droite.seul_sous(pane, cible),
        }
    }

    /// Remplace le groupe d'onglets qui contient `pane` par `f(ce groupe)`. Faux s'il n'y est pas.
    fn wrap_tabs_of(&mut self, pane: Pane, f: &mut dyn FnMut(DockNode) -> DockNode) -> bool {
        match self {
            DockNode::Tabs { panneaux, .. } if panneaux.contains(&pane) => {
                let old = std::mem::replace(self, tabs(Vec::new()));
                *self = f(old);
                true
            }
            DockNode::Tabs { .. } => false,
            DockNode::Row { gauche: a, droite: b, .. } | DockNode::Column { haut: a, bas: b, .. } => {
                a.wrap_tabs_of(pane, f) || b.wrap_tabs_of(pane, f)
            }
        }
    }
}

impl DockLayout {
    /// La disposition d'avant les fenêtres modulables (panneaux affichés ou non, tailles en
    /// pixels), ou celle d'un espace de travail de Pelure.
    pub fn from_legacy(l: &Layout) -> DockLayout {
        // À droite du dessin, la colonne en pile (partie 194) : la Couleur avec les Brosses dédiées en haut, puis Propriétés, qui
        // grandit, avec en onglets d'icônes les Réglages de l'outil, la Fiche du plan et les Effets. (Le panneau Espace 3D n'est
        // pas dans la disposition d'origine : il arrive en onglet de Propriétés quand la vue Espace 3D s'ouvre, et y reste tant
        // qu'elle est ouverte. `espace3dpanneau.rs`, parties 152 et 159.) La feuille d'exposition n'est pas ouverte d'office.
        // (Partie 160 : plus de panneau Calques, les calques sont dans la colonne de la timeline ; les effets restent.)
        let mut pile = Vec::new();
        let mut onglets = Vec::new();
        if l.tools {
            pile.push(PanneauPile::groupe(COLOR_TABS.to_vec()));
            onglets.push(Pane::ToolOptions);
            if l.sheet {
                onglets.push(Pane::Sheet);
            }
        }
        if l.sheet {
            onglets.push(Pane::Effects);
        }
        if !onglets.is_empty() {
            pile.push(PanneauPile::proprietes(onglets));
        }
        let colonne = Colonne::new(pile);
        let mut center = tabs(vec![Pane::Canvas]);
        if !colonne.is_empty() {
            center = row(1.0 - COLONNE / REF_WIDTH, center, tabs(vec![Pane::Colonne]));
        }
        // La bande du bas, sur toute la largeur de la fenêtre, sous la colonne.
        if l.strip {
            let h = l.montage_height.clamp(140.0, 1200.0);
            center = column(1.0 - h / REF_HEIGHT, center, tabs(vec![Pane::Montage]));
        }
        DockLayout { principal: center, colonne, ..DockLayout::empty() }.sanitized()
    }

    /// La disposition d'origine de l'espace Montage (le troisième onglet, en haut à droite) : le
    /// lecteur du film en grand en haut à gauche (le dessin, vu à travers la caméra) ; à sa droite la colonne, la
    /// fiche du plan (durée, textes) en haut et, dessous, Propriétés avec les panneaux du travail de montage en onglets
    /// (bruitages, commentaires, historique ; les suggestions sonores, expérimentales, n'y sont plus d'office depuis le 2026-10-30) ;
    /// en bas, sur toute la largeur, la bande du montage avec toutes ses pistes. Ni outils, ni couleur, ni pinceaux, ni calques, ni
    /// effets, ni guides : ceux-là restent à la demande (« + Panneau », menu Affichage).
    /// La fiche du plan est un onglet de Propriétés sorti en panneau (partie 194) : la fermer la remet en onglet.
    /// `bande_hauteur` : hauteur de la bande, en points de la fenêtre de référence.
    pub fn pour_le_montage(bande_hauteur: f32) -> DockLayout {
        let colonne = Colonne::new(vec![
            PanneauPile { sorti: true, ..PanneauPile::simple(Pane::Sheet) },
            PanneauPile::proprietes(vec![Pane::Bruitages, Pane::Comments, Pane::History]),
        ]);
        // (La colonne garde la largeur `COLONNE` quelle que soit la fenêtre, `App::fit_columns` ; la part n'est que celle du départ.)
        let haut = row(1.0 - COLONNE / REF_WIDTH, tabs(vec![Pane::Canvas]), tabs(vec![Pane::Colonne]));
        let h = bande_hauteur.clamp(140.0, 1200.0);
        DockLayout { principal: column(1.0 - h / REF_HEIGHT, haut, tabs(vec![Pane::Montage])), colonne, ..DockLayout::empty() }.sanitized()
    }

    /// La disposition d'origine de l'espace Vignettes (le premier onglet, en haut à droite) : la planche en grand,
    /// et à sa droite la colonne : le panneau « Planche » (sens de lecture, textes) en haut, la couleur, puis Propriétés avec
    /// les réglages de l'outil (le pinceau, avec sa liste de brosses) et le scénario. Ni bande (montage, timeline), ni calques,
    /// ni effets, ni fiche du plan, ni guides : « la partie Vignettes doit être la plus épurée possible, rien de distraction ».
    /// Les Références restent à la demande (« + Panneau ») ; les personnages sont dans leur barre au-dessus de la planche.
    pub fn pour_les_vignettes() -> DockLayout {
        let colonne = Colonne::new(vec![
            PanneauPile::simple(Pane::Vignettes),
            PanneauPile::simple(Pane::Color),
            PanneauPile::proprietes(vec![Pane::ToolOptions, Pane::Scenario]),
        ]);
        DockLayout { principal: row(1.0 - COLONNE / REF_WIDTH, tabs(vec![Pane::Canvas]), tabs(vec![Pane::Colonne])), colonne, ..DockLayout::empty() }.sanitized()
    }

    /// La disposition d'origine de l'espace Storyboard : la disposition standard, avec le scénario en onglet de Propriétés après
    /// les effets (fermé dans les autres espaces ; il s'ouvre tout seul après un import).
    pub fn pour_le_storyboard() -> DockLayout {
        let mut d = DockLayout::from_legacy(&Layout::default());
        if let Some(i) = d.colonne.proprietes() {
            d.colonne.panneaux[i].panneaux.push(Pane::Scenario);
        }
        // Partie 197 : le panneau Scène en haut de la pile (Scène, Couleur, Propriétés), comme dans la maquette validée. Une pile déjà
        // enregistrée ne le reçoit pas : « + Panneau » ou « Rétablir la disposition ».
        d.colonne.panneaux.insert(0, crate::colonne::PanneauPile::simple(Pane::Scene));
        d.sanitized()
    }

    /// Une disposition enregistrée par une version précédente, mise à jour : avant 6a, le
    /// panneau Outils part (barre verticale) ; avant 9b, la fiche rejoint les onglets des réglages ;
    /// avant 11c, le panneau Pinceaux arrive à côté de la couleur ; avant le chantier 6, la timeline
    /// arrive avec le montage (la bande montrait l'un ou l'autre selon le mode) ; en version 4, le
    /// montage et la timeline posés l'un sous l'autre repassent en onglets, à la hauteur d'avant.
    pub fn upgraded(mut self) -> DockLayout {
        if self.version < 1 {
            self.close(Pane::Tools);
        }
        if self.version < 2 && self.contains(Pane::Sheet) && !self.detachees.iter().any(|d| d.panneau == Pane::Sheet) {
            self.close(Pane::Sheet);
            self.open(Pane::Sheet);
        }
        // (Avant 11c, le panneau Pinceaux arrivait à côté de la couleur : il n'existe plus, ses brosses sont dans les
        // réglages de l'outil, et `sanitized` retire l'onglet des dispositions qui l'ont.)
        // (Les versions 4 et 5 avaient une timeline à part, que `sanitized` enlève : si elle
        // était seule en bas, le montage revient à sa place.)
        if (4..6).contains(&self.version) && self.contains(Pane::Timeline) && !self.contains(Pane::Montage) {
            self.open(Pane::Montage);
        }

        if self.version < 5 {
            let principal = std::mem::replace(&mut self.principal, tabs(Vec::new()));
            self.principal = principal.strips_as_tabs();
            for w in &mut self.fenetres {
                let n = std::mem::replace(&mut w.panneaux, tabs(Vec::new()));
                w.panneaux = n.strips_as_tabs();
            }
        }
        // Le panneau Effets arrive sous les calques (versions 7 et 8), une fois le reste converti.
        if self.version < 6 && !self.contains(Pane::Effects) {
            self.open(Pane::Effects);
        }
        // Dispositions de ce jour (6, 7) : la nouvelle disposition choisie par Anthony (« A, avec
        // les Effets sous les Calques ») ; les panneaux flottants ou dans leur fenêtre restent.
        if (6..8).contains(&self.version) {
            let d = DockLayout::from_legacy(&Layout::default());
            (self.principal, self.colonne) = (d.principal, d.colonne);
        }
        // Partie 161 : une seule colonne étroite ; deux colonnes de panneaux côte à côte se fondent en une (les panneaux
        // flottants et ceux dans leur fenêtre ne bougent pas).
        if self.version < 9 {
            let principal = std::mem::replace(&mut self.principal, tabs(Vec::new()));
            self.principal = principal.une_seule_colonne();
        }
        // Partie 194 : la colonne devient une pile de panneaux, la bande du bas passe sous elle, sur toute la largeur.
        if self.version < 10 {
            self.vers_la_pile();
        }
        self.version = LAYOUT_VERSION;
        self.sanitized()
    }

    /// Partie 194 : les groupes d'onglets de côté de la fenêtre principale (ceux qui n'ont ni le dessin, ni la bande du bas) entrent
    /// dans la pile de la colonne, sans perdre un panneau. Les panneaux de réglages (Réglages de l'outil, Fiche du plan, Effets,
    /// Scénario, Espace 3D, Historique, Bibliothèque…) deviennent, dans leur ordre, les onglets de Propriétés, celui qui était
    /// devant dans le groupe du haut restant devant ; chaque groupe de la Couleur, des Brosses dédiées ou de la Planche devient un
    /// panneau de la pile, au-dessus de Propriétés, dans l'ordre où ils étaient. Les panneaux flottants et ceux dans leur fenêtre ne
    /// bougent pas. La colonne se pose à droite du dessin, et la bande du bas sous les deux, sur toute la largeur.
    pub(crate) fn vers_la_pile(&mut self) {
        fn groupes(n: &DockNode, out: &mut Vec<(Vec<Pane>, usize)>) {
            match n {
                DockNode::Tabs { panneaux, actif } => {
                    if !panneaux.is_empty() && panneaux.iter().all(|p| !crate::colonne::hors_colonne(*p) || matches!(p, Pane::View | Pane::Guides | Pane::Brushes | Pane::Layers)) {
                        out.push((panneaux.clone(), *actif));
                    }
                }
                DockNode::Row { gauche: a, droite: b, .. } | DockNode::Column { haut: a, bas: b, .. } => {
                    groupes(a, out);
                    groupes(b, out);
                }
            }
        }
        let mut cote = Vec::new();
        groupes(&self.principal, &mut cote);
        if cote.is_empty() {
            self.bande_en_bas();
            return;
        }
        let mut seuls: Vec<PanneauPile> = Vec::new();
        let mut onglets: Vec<Pane> = Vec::new();
        let mut devant: Option<Pane> = None;
        let mut partis: Vec<Pane> = Vec::new();
        for (panes, actif) in &cote {
            partis.extend(panes.iter().copied());
            let actif_pane = cote_actif(panes.as_slice(), *actif).filter(|p| !crate::colonne::hors_colonne(*p));
            let panes: Vec<Pane> = panes.iter().copied().filter(|p| !crate::colonne::hors_colonne(*p)).collect();
            let (a_part, reglages): (Vec<Pane>, Vec<Pane>) = panes.iter().copied().partition(|p| !crate::colonne::en_onglet(*p));
            if !a_part.is_empty() {
                let rang = actif_pane.and_then(|a| a_part.iter().position(|p| *p == a)).unwrap_or(0);
                seuls.push(PanneauPile { actif: rang, ..PanneauPile::groupe(a_part) });
            }
            if !reglages.is_empty() && devant.is_none() {
                devant = actif_pane.filter(|a| reglages.contains(a)).or(reglages.first().copied());
            }
            onglets.extend(reglages);
        }
        // (Ce qu'une étape plus ancienne de la mise à jour aurait déjà posé dans la pile s'y ajoute.)
        let deja = std::mem::take(&mut self.colonne);
        let mut pile = seuls;
        if !onglets.is_empty() {
            let mut props = PanneauPile::proprietes(onglets);
            props.actif = devant.and_then(|d| props.panneaux.iter().position(|p| *p == d)).unwrap_or(0);
            pile.push(props);
        }
        let mut colonne = Colonne::new(pile);
        for p in deja.panes() {
            colonne.ouvrir(p);
        }
        self.colonne = colonne;
        let mut seen = partis;
        self.principal = std::mem::replace(&mut self.principal, tabs(Vec::new())).sanitized(&mut seen).unwrap_or_else(|| tabs(vec![Pane::Canvas]));
        if !self.principal_has(Pane::Canvas) {
            self.principal = row(0.25, std::mem::replace(&mut self.principal, tabs(Vec::new())), tabs(vec![Pane::Canvas]));
        }
        self.placer_la_colonne();
        self.bande_en_bas();
    }

    /// La colonne (`Pane::Colonne`) dans la fenêtre principale, si la pile a un panneau et qu'elle n'y est pas : à droite du
    /// dessin, au-dessus de la bande du bas.
    pub(crate) fn placer_la_colonne(&mut self) {
        if self.colonne.is_empty() || self.principal_has(Pane::Colonne) {
            return;
        }
        let mut cote = Some(tabs(vec![Pane::Colonne]));
        self.principal.wrap_tabs_of(Pane::Canvas, &mut |canvas| row(1.0 - COLONNE / REF_WIDTH, canvas, cote.take().unwrap_or_else(|| tabs(Vec::new()))));
    }

    /// La bande du bas (le montage, la timeline) sur toute la largeur de la fenêtre, sous le dessin et la colonne (partie 194) :
    /// seule dans son groupe et rangée ailleurs, elle repasse en bas, à la hauteur qu'elle avait.
    pub(crate) fn bande_en_bas(&mut self) {
        let seule = |n: &DockNode| matches!(n, DockNode::Tabs { panneaux, .. } if panneaux.as_slice() == [Pane::Montage]);
        if let DockNode::Column { bas, .. } = &self.principal {
            if seule(bas) {
                return;
            }
        }
        fn part_de(n: &DockNode, seule: &dyn Fn(&DockNode) -> bool) -> Option<f32> {
            match n {
                DockNode::Tabs { .. } => None,
                DockNode::Column { part, haut, bas } => if seule(bas) { Some(*part) } else { part_de(haut, seule).or_else(|| part_de(bas, seule)) },
                DockNode::Row { gauche, droite, .. } => part_de(gauche, seule).or_else(|| part_de(droite, seule)),
            }
        }
        let mut panes = Vec::new();
        self.principal.collect(&mut panes);
        if !panes.contains(&Pane::Montage) || self.principal.tabs_with(&[Pane::Montage]).is_some_and(|t| t.len() > 1) {
            return;
        }
        let part = part_de(&self.principal, &seule).unwrap_or(1.0 - BAND_HEIGHT / REF_HEIGHT);
        let Some(reste) = std::mem::replace(&mut self.principal, tabs(Vec::new())).without(Pane::Montage) else {
            self.principal = tabs(vec![Pane::Montage]);
            return;
        };
        self.principal = column(part, reste, tabs(vec![Pane::Montage]));
    }

    /// Le dessin seul.
    fn empty() -> DockLayout {
        DockLayout { version: LAYOUT_VERSION, principal: tabs(vec![Pane::Canvas]), fenetres: Vec::new(), detachees: Vec::new(), colonne: Colonne::default() }
    }

    /// Tous les panneaux : la fenêtre principale, la pile de la colonne (onglets de Propriétés compris), les panneaux
    /// flottants, ceux dans leur fenêtre.
    pub fn panes(&self) -> Vec<Pane> {
        let mut out = Vec::new();
        self.principal.collect(&mut out);
        out.extend(self.colonne.panes());
        for w in &self.fenetres {
            w.panneaux.collect(&mut out);
        }
        out.extend(self.detachees.iter().map(|d| d.panneau));
        out
    }

    /// Met `pane` dans sa propre fenêtre (le dessin reste dans la fenêtre principale).
    pub fn detach(&mut self, pane: Pane) {
        if pane == Pane::Canvas || self.detachees.iter().any(|d| d.panneau == pane) {
            return;
        }
        self.close(pane);
        self.detachees.push(DetachedPanel { panneau: pane, x: 0.0, y: 0.0, largeur: 0.0, hauteur: 0.0 });
    }

    pub fn contains(&self, pane: Pane) -> bool {
        self.panes().contains(&pane)
    }

    /// La disposition du storyboard : sans les panneaux d'animation seule (la feuille d'exposition).
    /// `None` s'il n'y en avait aucun d'ouvert.
    pub fn sans_animation_seule(&self) -> Option<DockLayout> {
        let mut out = self.clone();
        let mut any = false;
        for p in self.panes().into_iter().filter(|p| p.animation_only()) {
            out.close(p);
            any = true;
        }
        any.then_some(out)
    }

    /// Le retour en animation : `saved` est la disposition d'avant le storyboard, `self` celle
    /// d'aujourd'hui. Si rien n'a bougé entre-temps (aux parts près), on retrouve `saved` à
    /// l'identique ; sinon la feuille d'exposition se rouvre à sa place d'ordinaire, dans la
    /// disposition d'aujourd'hui.
    pub fn retour_animation(&self, saved: &DockLayout) -> DockLayout {
        let wanted: Vec<Pane> = saved.panes().into_iter().filter(|p| p.animation_only() && !self.contains(*p)).collect();
        if wanted.is_empty() {
            return self.clone();
        }
        if saved.sans_animation_seule().is_some_and(|s| s.close_to(self)) {
            return saved.clone();
        }
        let mut out = self.clone();
        for p in wanted {
            out.open(p);
        }
        out
    }

    /// Cette disposition (celle d'un espace de travail qu'on rejoint) avec les panneaux dans leur
    /// propre fenêtre `courantes` : une fenêtre à part est posée sur un autre écran, elle ne suit pas
    /// le mode. Font exception les panneaux qui n'existent qu'en animation (la feuille d'exposition
    /// détachée revient en animation, et elle quitte l'écran en storyboard) et ceux que cette
    /// disposition détache et qui ne sont pas détachés maintenant : ils gardent leur fenêtre d'ici.
    pub fn avec_fenetres_detachees(mut self, courantes: &[DetachedPanel]) -> DockLayout {
        let propres: Vec<DetachedPanel> = std::mem::take(&mut self.detachees)
            .into_iter()
            .filter(|d| d.panneau.animation_only() || !courantes.iter().any(|c| c.panneau == d.panneau))
            .collect();
        let suivent: Vec<DetachedPanel> = courantes.iter().filter(|d| !d.panneau.animation_only()).cloned().collect();
        for d in &suivent {
            // Le panneau quitte l'arbre (il y serait en double), sa fenêtre reste où elle est.
            self.close(d.panneau);
        }
        for d in &propres {
            self.close(d.panneau);
        }
        self.detachees = suivent;
        self.detachees.extend(propres);
        self.sanitized()
    }

    /// `pane` est dans la fenêtre principale (pas flottant ni détaché).
    fn principal_has(&self, pane: Pane) -> bool {
        let mut out = Vec::new();
        self.principal.collect(&mut out);
        out.contains(&pane)
    }

    /// `avant` repasse devant `pane`, dans le groupe d'onglets qu'ils partagent (fenêtre principale, onglets de Propriétés ou
    /// panneau flottant) : seulement si `pane` y est devant et si `avant` y est encore. Sinon rien ne change (les onglets ont été
    /// rangés autrement entre-temps). Vrai si c'est fait.
    ///
    /// Sert au panneau Espace 3D, que la vue ouverte met devant Réglages, son voisin d'onglets (`espace3dpanneau.rs`) : la
    /// disposition retenue garde l'onglet que l'utilisateur avait devant, pas celui que la vue y a mis pour un moment.
    pub fn remettre_devant(&mut self, pane: Pane, avant: Pane) -> bool {
        if let Some(i) = self.colonne.rang_de(pane) {
            let p = &mut self.colonne.panneaux[i];
            return match (p.devant() == Some(pane), p.panneaux.iter().position(|q| *q == avant)) {
                (true, Some(rang)) => {
                    p.actif = rang;
                    true
                }
                _ => false,
            };
        }
        let groupe = match self.principal.groupe_de(pane) {
            Some(g) => Some(g),
            None => self.fenetres.iter_mut().find_map(|w| w.panneaux.groupe_de(pane)),
        };
        let Some((onglets, actif)) = groupe else { return false };
        match (onglets.get(*actif) == Some(&pane), onglets.iter().position(|p| *p == avant)) {
            (true, Some(rang)) => {
                *actif = rang;
                true
            }
            _ => false,
        }
    }

    /// Retire `pane` de **la place que la partie 150 donnait au panneau Espace 3D** : un onglet du groupe de la couleur
    /// ou des brosses dédiées (fenêtre principale, panneau de la pile ou panneau flottant), ou, sans elles, seul juste sous les
    /// réglages de l'outil. Ailleurs (un autre groupe, un panneau flottant à lui, sa propre fenêtre, un onglet de Propriétés), il
    /// a été rangé à la main ou mis là par la vue : il y reste. Un onglet du groupe des réglages de l'outil reste aussi, même si
    /// la couleur y est avec lui : c'est sa place depuis la partie 152. Vrai s'il a été retiré.
    pub fn retirer_de_la_place_150(&mut self, pane: Pane) -> bool {
        let avec_la_couleur = |n: &DockNode| n.en_onglet_avec(pane, &COLOR_TABS) && !n.en_onglet_avec(pane, &[Pane::ToolOptions]);
        let pile = &self.colonne.panneaux;
        let dans_la_pile = pile.iter().any(|p| !p.est_proprietes() && p.panneaux.contains(&pane) && p.panneaux.iter().any(|q| COLOR_TABS.contains(q)))
            || pile.windows(2).any(|w| w[0].panneaux.contains(&Pane::ToolOptions) && w[1].panneaux.as_slice() == [pane]);
        let la = dans_la_pile || avec_la_couleur(&self.principal) || self.fenetres.iter().any(|w| avec_la_couleur(&w.panneaux)) || self.principal.seul_sous(pane, Pane::ToolOptions);
        if la {
            self.close(pane);
        }
        la
    }

    /// Chaque panneau une seule fois, le dessin toujours là (dans la fenêtre principale), parts
    /// bornées, panneaux flottants vides retirés ; la pile de la colonne remise d'aplomb, et la colonne dans la fenêtre
    /// principale dès que la pile a un panneau (elle ne flotte pas).
    pub fn sanitized(self) -> DockLayout {
        // L'ancienne timeline à part (versions 4 et 5), l'ancien panneau Vue (ses outils sont sous le
        // dessin), l'ancien panneau Guides (dans le menu de la règle) et l'ancien panneau Pinceaux (sa liste est dans
        // les réglages de l'outil) sont enlevés partout.
        // (Et, depuis la partie 160, l'ancien panneau Calques : ses commandes sont sur la ligne du calque courant et dans la barre
        // d'actions de la colonne des calques.)
        let mut seen = vec![Pane::Timeline, Pane::View, Pane::Guides, Pane::Brushes, Pane::Layers];
        let mut principal = self.principal.sanitized(&mut seen);
        // (Un panneau à la fois dans un onglet d'egui_dock et dans la pile reste où egui_dock le montre.)
        let colonne = self.colonne.nettoyee(&mut seen);
        let mut fenetres = Vec::new();
        for mut w in self.fenetres {
            // Le dessin et la colonne ne flottent pas.
            if let Some(n) = w.panneaux.without(Pane::Canvas).and_then(|n| n.without(Pane::Colonne)).and_then(|n| n.sanitized(&mut seen)) {
                w.panneaux = n;
                if ![w.x, w.y, w.largeur, w.hauteur].iter().all(|v| v.is_finite()) || w.largeur < 20.0 || w.hauteur < 20.0 {
                    (w.x, w.y, w.largeur, w.hauteur) = (0.0, 0.0, 0.0, 0.0);
                }
                fenetres.push(w);
            }
        }
        let mut detachees = Vec::new();
        for mut d in self.detachees {
            if matches!(d.panneau, Pane::Canvas | Pane::Colonne) || seen.contains(&d.panneau) {
                continue;
            }
            seen.push(d.panneau);
            if ![d.x, d.y, d.largeur, d.hauteur].iter().all(|v| v.is_finite()) || d.largeur < 40.0 || d.hauteur < 40.0 {
                (d.x, d.y, d.largeur, d.hauteur) = (0.0, 0.0, 0.0, 0.0);
            }
            detachees.push(d);
        }
        if !seen.contains(&Pane::Canvas) {
            principal = Some(match principal {
                Some(p) => row(0.25, p, tabs(vec![Pane::Canvas])),
                None => tabs(vec![Pane::Canvas]),
            });
        }
        let mut out = DockLayout { version: self.version, principal: principal.unwrap_or_else(|| tabs(vec![Pane::Canvas])), fenetres, detachees, colonne };
        out.placer_la_colonne();
        out
    }

    /// Ferme un panneau (le dessin ne se ferme pas, la colonne non plus : elle part avec son dernier panneau).
    pub fn close(&mut self, pane: Pane) {
        if matches!(pane, Pane::Canvas | Pane::Colonne) {
            return;
        }
        let layout = std::mem::replace(self, DockLayout::empty());
        let mut colonne = layout.colonne;
        colonne.retirer(pane);
        // (Plus un seul panneau de FEANOR dans la pile, pas même un onglet de Propriétés : la colonne part.)
        if colonne.panes().is_empty() {
            colonne = Colonne::default();
        }
        let mut seen = vec![pane];
        if colonne.is_empty() {
            seen.push(Pane::Colonne);
        }
        let principal = layout.principal.sanitized(&mut seen);
        let fenetres = layout
            .fenetres
            .into_iter()
            .filter_map(|mut w| {
                w.panneaux = w.panneaux.sanitized(&mut seen)?;
                Some(w)
            })
            .collect();
        let detachees = layout.detachees.into_iter().filter(|d| d.panneau != pane).collect();
        *self = DockLayout {
            version: layout.version,
            principal: principal.unwrap_or_else(|| tabs(vec![Pane::Canvas])),
            fenetres,
            detachees,
            colonne,
        }
        .sanitized();
    }

    /// Rouvre un panneau à sa place (partie 194) : avec les panneaux de sa famille s'ils sont rangés hors de la colonne (dans
    /// la fenêtre principale ou un panneau flottant) ; sinon dans la pile de la colonne (`Colonne::ouvrir` : un onglet de
    /// Propriétés devant, ou un panneau de la pile), la colonne revenant à droite du dessin s'il le faut. La bande du bas
    /// revient sous le dessin et la colonne, sur toute la largeur.
    pub fn open(&mut self, pane: Pane) {
        // (L'ancien panneau Calques ne s'ouvre plus, partie 160 : `vue.calques` montre la colonne des calques de la timeline.)
        if self.contains(pane) || matches!(pane, Pane::Layers | Pane::Canvas | Pane::Colonne | Pane::View | Pane::Guides) {
            return;
        }
        // L'ancienne timeline à part n'existe plus : c'est la bande.
        if pane == Pane::Timeline {
            return self.open(Pane::Montage);
        }
        // L'ancien panneau Pinceaux non plus : ses brosses sont dans les réglages de l'outil.
        if pane == Pane::Brushes {
            return self.open(Pane::ToolOptions);
        }
        if pane == Pane::Montage {
            return self.insert_group(Group::Bottom, &[pane]);
        }
        // Le panneau Espace 3D : un onglet du groupe des réglages de l'outil (Anthony, 2026-11-13, partie 152 : « Onglet de
        // Réglages ») ; Réglages rangé hors de la colonne, à côté de lui, après les onglets qui y sont.
        if pane == Pane::Espace3d {
            if let Some(t) = self.principal.tabs_with(&[Pane::ToolOptions]) {
                t.push(pane);
                return;
            }
        }
        // Un onglet des réglages revient avec les autres onglets des réglages, la couleur et les brosses dédiées ensemble, quand
        // ils sont rangés hors de la colonne (dans un groupe de la fenêtre principale, ou un panneau flottant).
        for family in [&SETTINGS_TABS[..], &COLOR_TABS[..]] {
            if !family.contains(&pane) {
                continue;
            }
            if let Some(t) = self.principal.tabs_with(family) {
                place_in_family(t, pane, family);
                return;
            }
            if let Some(t) = self.fenetres.iter_mut().find_map(|w| w.panneaux.tabs_with(family)) {
                place_in_family(t, pane, family);
                return;
            }
        }
        self.colonne.ouvrir(pane);
        self.placer_la_colonne();
    }

    /// `pane`, rangé hors de la colonne (un onglet d'egui_dock, un panneau flottant, sa fenêtre à part), revient dans la pile
    /// (clic droit sur sa poignée : « Dans la colonne »).
    pub fn ranger_dans_la_colonne(&mut self, pane: Pane) {
        if crate::colonne::hors_colonne(pane) || self.colonne.contient(pane) {
            return;
        }
        self.close(pane);
        self.colonne.ouvrir(pane);
        self.placer_la_colonne();
    }

    /// Affiche ou masque tout un groupe (actions « Panneau Outils / Fiche / Bande »).
    pub fn toggle_group(&mut self, group: Group) {
        let panes = group.panes();
        if panes.iter().any(|p| self.contains(*p)) {
            for p in panes {
                self.close(p);
            }
        } else {
            // La feuille d'exposition (13g) et l'historique ne s'ouvrent qu'à la demande.
            let panes: Vec<Pane> = panes.into_iter().filter(|p| !p.on_demand()).collect();
            self.insert_group(group, &panes);
        }
    }

    /// Remet les panneaux `panes` de `group` : la bande en bas, sur toute la largeur ; les autres dans la pile de la colonne.
    fn insert_group(&mut self, group: Group, panes: &[Pane]) {
        match group {
            Group::Center => {}
            Group::Bottom => {
                let Some(band) = bottom_band(panes) else { return };
                let main = std::mem::replace(&mut self.principal, tabs(Vec::new()));
                self.principal = column(1.0 - BAND_HEIGHT / REF_HEIGHT, main, band);
            }
            Group::Left | Group::Right | Group::Planche => {
                for p in panes {
                    self.open(*p);
                }
            }
        }
        let version = self.version;
        *self = DockLayout { version, ..std::mem::replace(self, DockLayout::empty()) }.sanitized();
    }

    /// Deux dispositions égales, aux parts près (elles bougent d'un rien quand on tire un bord).
    pub fn close_to(&self, other: &DockLayout) -> bool {
        fn same(a: &DockNode, b: &DockNode) -> bool {
            match (a, b) {
                (DockNode::Tabs { panneaux: p, actif: x }, DockNode::Tabs { panneaux: q, actif: y }) => p == q && x == y,
                (DockNode::Row { part: f, gauche: a1, droite: b1 }, DockNode::Row { part: g, gauche: a2, droite: b2 })
                | (DockNode::Column { part: f, haut: a1, bas: b1 }, DockNode::Column { part: g, haut: a2, bas: b2 }) => {
                    (f - g).abs() < 0.002 && same(a1, a2) && same(b1, b2)
                }
                _ => false,
            }
        }
        same(&self.principal, &other.principal)
            && self.colonne.proche_de(&other.colonne)
            && self.fenetres.len() == other.fenetres.len()
            && self.fenetres.iter().zip(&other.fenetres).all(|(a, b)| {
                same(&a.panneaux, &b.panneaux)
                    && [(a.x, b.x), (a.y, b.y), (a.largeur, b.largeur), (a.hauteur, b.hauteur)]
                        .iter()
                        .all(|(u, v)| (u - v).abs() < 1.0)
            })
            && self.detachees.len() == other.detachees.len()
            && self.detachees.iter().zip(&other.detachees).all(|(a, b)| {
                a.panneau == b.panneau
                    && [(a.x, b.x), (a.y, b.y), (a.largeur, b.largeur), (a.hauteur, b.hauteur)]
                        .iter()
                        .all(|(u, v)| (u - v).abs() < 1.0)
            })
    }
}

// ---------------------------------------------------------------------- egui_dock ↔ Pelure

/// Textes de la bibliothèque, en français.
fn french() -> egui_dock::Translations {
    let mut t = egui_dock::Translations::english();
    t.tab_context_menu.close_button = "Fermer".into();
    t.tab_context_menu.eject_button = "Détacher (panneau flottant)".into();
    let l = &mut t.leaf;
    l.close_button_disabled_tooltip = "Ce groupe contient le dessin, qui ne se ferme pas.".into();
    l.close_all_button = "Fermer le panneau flottant".into();
    l.close_all_button_menu_hint = "Clic droit pour fermer ce panneau flottant.".into();
    l.close_all_button_modifier_hint = "Maj + clic pour fermer ce panneau flottant.".into();
    l.close_all_button_modifier_menu_hint = "Maj + clic, ou clic droit, pour fermer ce panneau flottant.".into();
    l.close_all_button_disabled_tooltip = "Ce panneau flottant contient le dessin, qui ne se ferme pas.".into();
    l.minimize_button = "Réduire".into();
    l.minimize_button_menu_hint = "Clic droit pour réduire.".into();
    l.minimize_button_modifier_hint = "Maj + clic pour réduire.".into();
    l.minimize_button_modifier_menu_hint = "Maj + clic, ou clic droit, pour réduire.".into();
    t
}

/// L'état affiché par egui_dock, construit depuis une disposition.
pub fn to_state(layout: &DockLayout) -> DockState<Pane> {
    let mut state = DockState::new(layout.principal.first_tabs()).with_translations(french());
    build(state.main_surface_mut(), NodeIndex::root(), &layout.principal);
    for w in &layout.fenetres {
        let s = state.add_window(w.panneaux.first_tabs());
        if let Some(tree) = state.get_surface_mut(s).and_then(|x| x.node_tree_mut()) {
            build(tree, NodeIndex::root(), &w.panneaux);
        }
        if w.largeur > 0.0 && w.hauteur > 0.0 {
            if let Some(ws) = state.get_window_state_mut(s) {
                ws.set_position(pos2(w.x, w.y));
                ws.set_size(vec2(w.largeur, w.hauteur));
            }
        }
    }
    state
}

/// Le nœud `at` de `tree` contient déjà les onglets de `node.first_tabs()` : on le partage
/// comme `node`, de proche en proche.
fn build(tree: &mut Tree<Pane>, at: NodeIndex, node: &DockNode) {
    match node {
        DockNode::Tabs { panneaux, actif } => {
            tree.set_active_tab(at, TabIndex((*actif).min(panneaux.len().saturating_sub(1))));
        }
        DockNode::Row { part, gauche, droite } => {
            let [a, b] = tree.split(at, Split::Right, part.clamp(0.05, 0.95), Node::leaf_with(droite.first_tabs()));
            build(tree, a, gauche);
            build(tree, b, droite);
        }
        DockNode::Column { part, haut, bas } => {
            let [a, b] = tree.split(at, Split::Below, part.clamp(0.05, 0.95), Node::leaf_with(bas.first_tabs()));
            build(tree, a, haut);
            build(tree, b, bas);
        }
    }
}

/// Identifiant de la fenêtre egui d'un panneau flottant (celui qu'egui_dock 0.16 lui donne) : sa
/// place se lit dans la mémoire d'egui, egui_dock ne la retient pas. Si une autre version de la
/// bibliothèque change ce nom, la place n'est simplement plus retenue.
pub fn window_id(surface: usize) -> eframe::egui::Id {
    eframe::egui::Id::new(format!("window {:?}", egui_dock::SurfaceIndex(surface)))
}

/// La disposition telle qu'egui_dock l'affiche (après les gestes de l'utilisateur).
/// `window_rect(i)` : place à l'écran du panneau flottant de la surface `i`, si on la connaît.
pub fn from_state(state: &DockState<Pane>, window_rect: &dyn Fn(usize) -> Option<eframe::egui::Rect>) -> DockLayout {
    let principal = read(state.main_surface(), NodeIndex::root()).unwrap_or_else(|| tabs(vec![Pane::Canvas]));
    let mut fenetres = Vec::new();
    for (i, surface) in state.iter_surfaces().enumerate() {
        if let Surface::Window(tree, ws) = surface {
            if let Some(n) = read(tree, NodeIndex::root()) {
                let r = window_rect(i).unwrap_or_else(|| ws.rect());
                let known = r.is_finite() && r.width() > 0.0 && r.height() > 0.0;
                fenetres.push(FloatingPanel {
                    panneaux: n,
                    x: if known { r.min.x } else { 0.0 },
                    y: if known { r.min.y } else { 0.0 },
                    largeur: if known { r.width() } else { 0.0 },
                    hauteur: if known { r.height() } else { 0.0 },
                });
            }
        }
    }
    DockLayout { version: LAYOUT_VERSION, principal, fenetres, detachees: Vec::new(), colonne: Colonne::default() }.sanitized()
}

fn read(tree: &Tree<Pane>, at: NodeIndex) -> Option<DockNode> {
    if at.0 >= tree.len() {
        return None;
    }
    let pair = |a: Option<DockNode>, b: Option<DockNode>, f: &dyn Fn(DockNode, DockNode) -> DockNode| match (a, b) {
        (Some(a), Some(b)) => Some(f(a, b)),
        (a, b) => a.or(b),
    };
    match &tree[at] {
        Node::Empty => None,
        Node::Leaf { tabs: t, active, .. } => (!t.is_empty()).then(|| DockNode::Tabs { panneaux: t.clone(), actif: active.0 }),
        Node::Horizontal { fraction, .. } => {
            let f = *fraction;
            pair(read(tree, at.left()), read(tree, at.right()), &|a, b| row(f, a, b))
        }
        Node::Vertical { fraction, .. } => {
            let f = *fraction;
            pair(read(tree, at.left()), read(tree, at.right()), &|a, b| column(f, a, b))
        }
    }
}
