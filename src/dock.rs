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
    /// Réglages de l'outil courant.
    #[serde(rename = "reglages")]
    ToolOptions,
    /// Vue : ajuster, zoom, miroir, rotation, table lumineuse.
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
    /// Bande de montage (storyboard) ou timeline (animation).
    #[serde(rename = "bande")]
    Strip,
}

/// Où un panneau se range quand on le rouvre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    /// Colonne de gauche : outils, couleur, réglages, vue, guides.
    Left,
    /// Colonne de droite : fiche et calques.
    Right,
    /// En bas, sur toute la largeur : montage ou timeline.
    Bottom,
    Center,
}

impl Pane {
    pub const ALL: [Pane; 9] = [
        Pane::Canvas,
        Pane::Tools,
        Pane::Color,
        Pane::ToolOptions,
        Pane::View,
        Pane::Guides,
        Pane::Sheet,
        Pane::Layers,
        Pane::Strip,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Pane::Canvas => "Dessin",
            Pane::Tools => "Outils",
            Pane::Color => "Couleur",
            Pane::ToolOptions => "Réglages de l'outil",
            Pane::View => "Vue",
            Pane::Guides => "Guides",
            Pane::Sheet => "Fiche du panneau",
            Pane::Layers => "Calques",
            Pane::Strip => "Montage et timeline",
        }
    }

    /// Nom court, pour l'onglet.
    pub fn tab_name(self) -> &'static str {
        match self {
            Pane::ToolOptions => "Réglages",
            Pane::Sheet => "Fiche",
            p => p.name(),
        }
    }

    pub fn icon(self) -> &'static str {
        use egui_phosphor::regular as ph;
        match self {
            Pane::Canvas => ph::FRAME_CORNERS,
            Pane::Tools => ph::TOOLBOX,
            Pane::Color => ph::PALETTE,
            Pane::ToolOptions => ph::SLIDERS_HORIZONTAL,
            Pane::View => ph::MAGNIFYING_GLASS,
            Pane::Guides => ph::RULER,
            Pane::Sheet => ph::CLIPBOARD_TEXT,
            Pane::Layers => ph::STACK,
            Pane::Strip => ph::FILM_STRIP,
        }
    }

    pub fn group(self) -> Group {
        match self {
            Pane::Canvas => Group::Center,
            Pane::Tools | Pane::Color | Pane::ToolOptions | Pane::View | Pane::Guides => Group::Left,
            Pane::Sheet | Pane::Layers => Group::Right,
            Pane::Strip => Group::Bottom,
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

/// La disposition des panneaux.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DockLayout {
    pub principal: DockNode,
    #[serde(default)]
    pub fenetres: Vec<FloatingPanel>,
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

/// Colonne de gauche par défaut : les outils en haut, la couleur, puis réglages, vue et guides
/// en onglets.
fn left_column(only: &[Pane]) -> Option<DockNode> {
    let keep = |list: Vec<Pane>| -> Option<DockNode> {
        let list: Vec<Pane> = list.into_iter().filter(|p| only.contains(p)).collect();
        (!list.is_empty()).then(|| tabs(list))
    };
    let parts = [
        (0.27, keep(vec![Pane::Tools])),
        (0.22, keep(vec![Pane::Color])),
        (1.0, keep(vec![Pane::ToolOptions, Pane::View, Pane::Guides])),
    ];
    stack(&parts)
}

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

fn right_column(only: &[Pane]) -> Option<DockNode> {
    let sheet = only.contains(&Pane::Sheet).then(|| tabs(vec![Pane::Sheet]));
    let layers = only.contains(&Pane::Layers).then(|| tabs(vec![Pane::Layers]));
    stack(&[(0.55, sheet), (1.0, layers)])
}

/// Taille de fenêtre de référence pour passer des tailles en pixels (anciennes dispositions) aux
/// parts de la nouvelle.
const REF_WIDTH: f32 = 1500.0;
const REF_HEIGHT: f32 = 860.0;
const LEFT_WIDTH: f32 = 250.0;

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
        let mut width = REF_WIDTH;
        if l.tools {
            if let Some(left) = left_column(&Group::Left.panes()) {
                center = row(LEFT_WIDTH / width, left, center);
            }
            width -= LEFT_WIDTH;
        }
        if l.sheet {
            if let Some(right) = right_column(&Group::Right.panes()) {
                let canvas_part = 1.0 - l.sheet_width.clamp(160.0, 900.0) / width.max(1.0);
                // La colonne de droite est à droite du dessin (et de la colonne de gauche).
                center = match center {
                    DockNode::Row { part, gauche, droite } => row(part, *gauche, row(canvas_part, *droite, right)),
                    c => row(canvas_part, c, right),
                };
            }
        }
        if l.strip {
            let part = 1.0 - l.montage_height.clamp(140.0, 1200.0) / REF_HEIGHT;
            center = column(part, center, tabs(vec![Pane::Strip]));
        }
        DockLayout { principal: center, fenetres: Vec::new() }.sanitized()
    }

    pub fn panes(&self) -> Vec<Pane> {
        let mut out = Vec::new();
        self.principal.collect(&mut out);
        for w in &self.fenetres {
            w.panneaux.collect(&mut out);
        }
        out
    }

    pub fn contains(&self, pane: Pane) -> bool {
        self.panes().contains(&pane)
    }

    /// Chaque panneau une seule fois, le dessin toujours là (dans la fenêtre principale), parts
    /// bornées, panneaux flottants vides retirés.
    pub fn sanitized(self) -> DockLayout {
        let mut seen = Vec::new();
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
        if !seen.contains(&Pane::Canvas) {
            principal = Some(match principal {
                Some(p) => row(0.25, p, tabs(vec![Pane::Canvas])),
                None => tabs(vec![Pane::Canvas]),
            });
        }
        DockLayout { principal: principal.unwrap_or_else(|| tabs(vec![Pane::Canvas])), fenetres }
    }

    /// Ferme un panneau (le dessin ne se ferme pas).
    pub fn close(&mut self, pane: Pane) {
        if pane == Pane::Canvas {
            return;
        }
        let layout = std::mem::replace(self, DockLayout { principal: tabs(vec![Pane::Canvas]), fenetres: Vec::new() });
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
        *self = DockLayout { principal: principal.unwrap_or_else(|| tabs(vec![Pane::Canvas])), fenetres }.sanitized();
    }

    /// Rouvre un panneau à sa place : avec les panneaux de son groupe s'il en reste, sinon dans
    /// la colonne (ou la bande) de son groupe, recréée à côté du dessin.
    pub fn open(&mut self, pane: Pane) {
        if self.contains(pane) {
            return;
        }
        let group = pane.group();
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
            self.insert_group(group, &panes);
        }
    }

    /// Recrée la colonne (ou la bande) de `group` avec `panes`, à côté du dessin.
    fn insert_group(&mut self, group: Group, panes: &[Pane]) {
        match group {
            Group::Center => {}
            Group::Bottom => {
                let main = std::mem::replace(&mut self.principal, tabs(Vec::new()));
                self.principal = column(1.0 - 290.0 / REF_HEIGHT, main, tabs(panes.to_vec()));
            }
            Group::Left => {
                let Some(left) = left_column(panes) else { return };
                let mut left = Some(left);
                self.principal
                    .wrap_tabs_of(Pane::Canvas, &mut |canvas| row(0.2, left.take().unwrap_or_else(|| tabs(Vec::new())), canvas));
            }
            Group::Right => {
                let Some(right) = right_column(panes) else { return };
                let mut right = Some(right);
                self.principal
                    .wrap_tabs_of(Pane::Canvas, &mut |canvas| row(0.78, canvas, right.take().unwrap_or_else(|| tabs(Vec::new()))));
            }
        }
        *self = std::mem::replace(self, DockLayout { principal: tabs(vec![Pane::Canvas]), fenetres: Vec::new() }).sanitized();
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
    DockLayout { principal, fenetres }.sanitized()
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
