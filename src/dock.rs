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
    /// Réglages de pinceau, chacun avec l'aperçu de son trait (11c), en onglet avec la couleur.
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
    /// Guides de perspective.
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
    /// fond, leur ordre. En onglet avec la couleur et les pinceaux dans la disposition par défaut.
    #[serde(rename = "personnages")]
    Personnages,
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
}

impl Pane {
    pub const ALL: [Pane; 17] = [
        Pane::Canvas,
        Pane::Tools,
        Pane::Color,
        Pane::Brushes,
        Pane::ToolOptions,
        Pane::Guides,
        Pane::Sheet,
        Pane::Layers,
        Pane::Montage,
        Pane::XSheet,
        Pane::Effects,
        Pane::History,
        Pane::Comments,
        Pane::Bruitages,
        Pane::SuggestionsSonores,
        Pane::References,
        Pane::Personnages,
    ];

    /// Les panneaux fermés dans la disposition par défaut, qui ne s'ouvrent qu'à la demande.
    pub fn on_demand(self) -> bool {
        matches!(
            self,
            Pane::XSheet | Pane::History | Pane::Comments | Pane::Bruitages | Pane::SuggestionsSonores | Pane::References
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
            Pane::SuggestionsSonores => "Suggestions sonores",
            Pane::References => "Références",
            Pane::Personnages => "Personnages",
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
            Pane::SuggestionsSonores => ph::SPARKLE,
            Pane::References => ph::IMAGES,
            Pane::Personnages => ph::USERS_THREE,
        }
    }

    pub fn group(self) -> Group {
        match self {
            Pane::Canvas => Group::Center,
            Pane::Tools | Pane::Color | Pane::Brushes | Pane::Personnages | Pane::ToolOptions | Pane::View | Pane::Guides | Pane::Sheet => Group::Left,
            Pane::Layers
            | Pane::XSheet
            | Pane::Effects
            | Pane::History
            | Pane::Comments
            | Pane::Bruitages
            | Pane::SuggestionsSonores
            | Pane::References => Group::Right,
            Pane::Montage | Pane::Timeline => Group::Bottom,
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

/// Colonne des réglages (6a, à droite) : réglages de l'outil, vue et guides en onglets, la
/// couleur dessous ; les icônes des outils au-dessus si on les a rouvertes en panneau (elles sont
/// d'ordinaire dans la barre verticale).
fn left_column(only: &[Pane]) -> Option<DockNode> {
    let keep = |list: Vec<Pane>| -> Option<DockNode> {
        let list: Vec<Pane> = list.into_iter().filter(|p| only.contains(p)).collect();
        (!list.is_empty()).then(|| tabs(list))
    };
    let parts = [(0.3, keep(vec![Pane::Tools])), (0.55, keep(SETTINGS_TABS.to_vec())), (1.0, keep(COLOR_TABS.to_vec()))];
    stack(&parts)
}

/// Les onglets des réglages : réglages de l'outil, guides, fiche du panneau (9b) ; la vue est
/// sous le dessin depuis le 2026-10-17.
const SETTINGS_TABS: [Pane; 3] = [Pane::ToolOptions, Pane::Guides, Pane::Sheet];

/// Les onglets du bas de la colonne : couleur et pinceaux (11c), et les personnages (brosse par personnage).
const COLOR_TABS: [Pane; 3] = [Pane::Color, Pane::Brushes, Pane::Personnages];

/// Remet `pane` parmi les onglets `tabs` de sa famille : à la fin pour les réglages ; pour la couleur, les pinceaux et les
/// personnages, à leur rang (couleur, pinceaux, personnages), quels que soient ceux qui sont restés.
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

/// Version de la disposition : 1 depuis 6a (outils dans la barre verticale, calques à côté du
/// dessin, réglages et couleur à droite) ; 2 depuis 9b (la fiche avec les réglages) ; 3 depuis 11c
/// (le panneau Pinceaux, en onglet avec la couleur) ; 4 depuis le chantier 6 (la timeline, panneau à
/// part sous le montage) ; 5 le même jour (en onglets) ; 6 le même jour (une seule bande, comme
/// avant : le montage en storyboard, la timeline en animation) ; 7 le même jour (le panneau
/// Effets, sous les réglages) ; 8 le même jour (choix d'Anthony : les calques avec les effets
/// dessous, puis les réglages avec la couleur dessous).
pub const LAYOUT_VERSION: u32 = 8;

/// La bande du bas : une seule (Anthony, 2026-10-17 : « je ne veux pas de double timeline ») ;
/// elle montre le montage en storyboard, la timeline en animation.
fn bottom_band(panes: &[Pane]) -> Option<DockNode> {
    (panes.contains(&Pane::Montage) || panes.contains(&Pane::Timeline)).then(|| tabs(vec![Pane::Montage]))
}

/// Hauteur de la bande du bas (en points de la fenêtre de référence), et celle de la version 4
/// (montage et timeline l'un sous l'autre).
const BAND_HEIGHT: f32 = 290.0;
const BAND_HEIGHT_V4: f32 = 420.0;

/// Empile de haut en bas des parties (part de chacune dans ce qui reste), en sautant les absentes.
fn stack(parts: &[(f32, Option<DockNode>)]) -> Option<DockNode> {
    let present: Vec<(f32, DockNode)> = parts.iter().filter_map(|(f, n)| n.clone().map(|n| (*f, n))).collect();
    let mut iter = present.into_iter().rev();
    let (_, mut node) = iter.next()?;
    for (f, top) in iter {
        node = column(f, top, node);
    }
    Some(node)
}

/// Colonne des calques (6a, à droite du dessin), sur toute la hauteur depuis 9b ; la feuille
/// d'exposition (13g) y vient en onglet ; les effets dessous (disposition 8, choix d'Anthony :
/// « A, avec les Effets sous les Calques »).
fn right_column(only: &[Pane]) -> Option<DockNode> {
    // (Tous les panneaux de la colonne de droite hors les effets : sans les calques, l'historique, les
    // commentaires, les bruitages, les suggestions sonores et les références y forment leurs onglets,
    // au lieu de ne pas s'ouvrir.)
    let list: Vec<Pane> = Pane::ALL.into_iter().filter(|p| p.group() == Group::Right && *p != Pane::Effects && only.contains(p)).collect();
    let layers = (!list.is_empty()).then(|| tabs(list));
    let effects = only.contains(&Pane::Effects).then(|| tabs(vec![Pane::Effects]));
    stack(&[(0.6, layers), (1.0, effects)])
}

/// Taille de fenêtre de référence pour passer des tailles en pixels (anciennes dispositions) aux
/// parts de la nouvelle (largeur hors barre d'outils verticale).
const REF_WIDTH: f32 = 1454.0;
const REF_HEIGHT: f32 = 820.0;
/// Largeur de la colonne des réglages (quatre onglets depuis 9b).
const TOOLS_WIDTH: f32 = 350.0;
/// Part du lecteur, dans l'espace Montage, de la largeur au-dessus de la bande (le reste : la fiche
/// du plan et les panneaux du montage).
const MONTAGE_LECTEUR_PART: f32 = 0.7;

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
        let mut center = tabs(vec![Pane::Canvas]);
        // À droite du dessin : les calques, puis les réglages (avec la fiche) et la couleur.
        let mut settings = Vec::new();
        if l.tools {
            settings.extend([Pane::ToolOptions, Pane::Guides, Pane::Color, Pane::Brushes, Pane::Personnages]);
            if l.sheet {
                settings.push(Pane::Sheet);
            }
        }
        let tools = left_column(&settings);
        // La feuille d'exposition n'est pas ouverte d'office.
        let layers = if l.sheet { right_column(&[Pane::Layers, Pane::Effects]) } else { None };
        let layers_w = l.sheet_width.clamp(160.0, 900.0);
        let side = match (layers, tools) {
            (Some(a), Some(b)) => Some((row(layers_w / (layers_w + TOOLS_WIDTH), a, b), layers_w + TOOLS_WIDTH)),
            (Some(a), None) => Some((a, layers_w)),
            (None, Some(b)) => Some((b, TOOLS_WIDTH)),
            (None, None) => None,
        };
        if let Some((side, w)) = side {
            center = row(1.0 - w / REF_WIDTH, center, side);
        }
        if l.strip {
            let h = l.montage_height.clamp(140.0, 1200.0);
            let band = bottom_band(&[Pane::Montage]).unwrap_or_else(|| tabs(vec![Pane::Montage]));
            center = column(1.0 - h / REF_HEIGHT, center, band);
        }
        DockLayout { principal: center, ..DockLayout::empty() }.sanitized()
    }

    /// La disposition d'origine de l'espace Montage (le troisième onglet, en haut à droite) : le
    /// lecteur du film en grand en haut à gauche (le dessin, vu à travers la caméra) ; à sa droite la
    /// fiche du plan (durée, textes) et, dessous, les panneaux du travail de montage en onglets
    /// (bruitages, suggestions sonores, commentaires, historique) ; en bas, sur toute la largeur, la
    /// bande du montage avec toutes ses pistes. Ni outils, ni couleur, ni pinceaux, ni calques, ni
    /// effets, ni guides : ceux-là restent à la demande (barre des panneaux, menu Affichage).
    /// `bande_hauteur` : hauteur de la bande, en points de la fenêtre de référence.
    pub fn pour_le_montage(bande_hauteur: f32) -> DockLayout {
        let lecteur = tabs(vec![Pane::Canvas]);
        let cote = column(
            0.4,
            tabs(vec![Pane::Sheet]),
            tabs(vec![Pane::Bruitages, Pane::SuggestionsSonores, Pane::Comments, Pane::History]),
        );
        let haut = row(MONTAGE_LECTEUR_PART, lecteur, cote);
        let h = bande_hauteur.clamp(140.0, 1200.0);
        DockLayout { principal: column(1.0 - h / REF_HEIGHT, haut, tabs(vec![Pane::Montage])), ..DockLayout::empty() }.sanitized()
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
        if self.version < 3 && !self.contains(Pane::Brushes) {
            self.open(Pane::Brushes);
        }
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
            self.principal = DockLayout::from_legacy(&Layout::default()).principal;
        }
        self.version = LAYOUT_VERSION;
        self.sanitized()
    }

    /// Le dessin seul.
    fn empty() -> DockLayout {
        DockLayout { version: LAYOUT_VERSION, principal: tabs(vec![Pane::Canvas]), fenetres: Vec::new(), detachees: Vec::new() }
    }

    pub fn panes(&self) -> Vec<Pane> {
        let mut out = Vec::new();
        self.principal.collect(&mut out);
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

    /// Chaque panneau une seule fois, le dessin toujours là (dans la fenêtre principale), parts
    /// bornées, panneaux flottants vides retirés.
    pub fn sanitized(self) -> DockLayout {
        // L'ancienne timeline à part (versions 4 et 5) et l'ancien panneau Vue (ses outils sont
        // sous le dessin) sont enlevés partout.
        let mut seen = vec![Pane::Timeline, Pane::View];
        let mut principal = self.principal.sanitized(&mut seen);
        let mut fenetres = Vec::new();
        for mut w in self.fenetres {
            // Le dessin ne flotte pas.
            if let Some(n) = w.panneaux.without(Pane::Canvas).and_then(|n| n.sanitized(&mut seen)) {
                w.panneaux = n;
                if ![w.x, w.y, w.largeur, w.hauteur].iter().all(|v| v.is_finite()) || w.largeur < 20.0 || w.hauteur < 20.0 {
                    (w.x, w.y, w.largeur, w.hauteur) = (0.0, 0.0, 0.0, 0.0);
                }
                fenetres.push(w);
            }
        }
        let mut detachees = Vec::new();
        for mut d in self.detachees {
            if d.panneau == Pane::Canvas || seen.contains(&d.panneau) {
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
        DockLayout { version: self.version, principal: principal.unwrap_or_else(|| tabs(vec![Pane::Canvas])), fenetres, detachees }
    }

    /// Ferme un panneau (le dessin ne se ferme pas).
    pub fn close(&mut self, pane: Pane) {
        if pane == Pane::Canvas {
            return;
        }
        let layout = std::mem::replace(self, DockLayout::empty());
        let mut seen = vec![pane];
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
        }
        .sanitized();
    }

    /// Rouvre un panneau à sa place : avec les panneaux de son groupe s'il en reste, sinon dans
    /// la colonne (ou la bande) de son groupe, recréée à côté du dessin.
    pub fn open(&mut self, pane: Pane) {
        if self.contains(pane) {
            return;
        }
        let group = pane.group();
        // Un onglet des réglages revient avec les autres onglets des réglages ; les pinceaux et la
        // couleur, ensemble.
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
        // L'ancienne timeline à part n'existe plus : c'est la bande.
        if pane == Pane::Timeline {
            return self.open(Pane::Montage);
        }
        // Les effets : sous les calques (dans la fenêtre principale), sinon dans la colonne des
        // calques.
        if pane == Pane::Effects && self.principal_has(Pane::Layers) {
            let mut under = |n: DockNode| column(0.6, n, tabs(vec![Pane::Effects]));
            self.principal.wrap_tabs_of(Pane::Layers, &mut under);
            return;
        }
        // Les calques rouverts quand les effets sont restés seuls : au-dessus d'eux.
        if pane == Pane::Layers && self.principal_has(Pane::Effects) {
            let mut above = |n: DockNode| column(0.6, tabs(vec![Pane::Layers]), n);
            self.principal.wrap_tabs_of(Pane::Effects, &mut above);
            return;
        }
        if group != Group::Bottom {
            if let Some(t) = self.principal.tabs_of_group(group) {
                t.push(pane);
                return;
            }
            if let Some(t) = self.fenetres.iter_mut().find_map(|w| w.panneaux.tabs_of_group(group)) {
                t.push(pane);
                return;
            }
        }
        self.insert_group(group, &[pane]);
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

    /// Recrée la colonne (ou la bande) de `group` avec `panes`, à côté du dessin.
    fn insert_group(&mut self, group: Group, panes: &[Pane]) {
        match group {
            Group::Center => {}
            Group::Bottom => {
                let Some(band) = bottom_band(panes) else { return };
                let main = std::mem::replace(&mut self.principal, tabs(Vec::new()));
                self.principal = column(1.0 - BAND_HEIGHT / REF_HEIGHT, main, band);
            }
            Group::Left => {
                // La colonne des réglages, tout à droite (au-dessus de la bande s'il y en a une).
                let Some(col) = left_column(panes) else { return };
                let main = std::mem::replace(&mut self.principal, tabs(Vec::new()));
                self.principal = match main {
                    DockNode::Column { part, haut, bas } if bas.first_tabs().iter().any(|p| p.group() == Group::Bottom) => {
                        column(part, row(0.8, *haut, col), *bas)
                    }
                    other => row(0.8, other, col),
                };
            }
            Group::Right => {
                let Some(right) = right_column(panes) else { return };
                let mut right = Some(right);
                self.principal
                    .wrap_tabs_of(Pane::Canvas, &mut |canvas| row(0.78, canvas, right.take().unwrap_or_else(|| tabs(Vec::new()))));
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
    DockLayout { version: LAYOUT_VERSION, principal, fenetres, detachees: Vec::new() }.sanitized()
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
