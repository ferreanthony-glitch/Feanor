//! Panneaux qui s'ajustent à ce qu'ils montrent (Anthony, 2026-10-18, axe d'ergonomie 8 :
//! « que les panneaux s'adaptent d'eux-mêmes en fonction de ce qu'ils affichent ») : quand deux
//! panneaux sont l'un sur l'autre, celui qui montre des réglages (réglages de l'outil,
//! fiche, effets, icônes des outils) prend la hauteur de son contenu et se réduit tout seul ; la
//! couleur, les pinceaux, les calques, le dessin et la bande, qui savent se servir de toute la
//! place, prennent le reste. Un bord tiré à la main reste où on l'a mis, d'une séance à l'autre,
//! jusqu'à « Réajuster les panneaux » (Affichage > Panneaux).

use eframe::egui;
use egui_dock::{DockState, Node, NodeIndex, Tree};

use crate::app::App;
use crate::dock::Pane;

/// Ce que demande une partie d'une colonne (en points).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Want {
    /// La hauteur de son contenu, barre d'onglets comprise.
    Fit(f32),
    /// Toute la place qui reste, au moins `min` ; `content` : la hauteur de son contenu quand on
    /// la connaît (la couleur), pour partager équitablement quand les deux ne tiennent pas.
    Flex { min: f32, content: Option<f32> },
}

/// La hauteur de la partie qui s'ajuste (`fit` points de contenu), à côté d'une partie souple.
fn share(fit: f32, min: f32, content: Option<f32>, room: f32) -> f32 {
    let h = match content {
        // Les deux ne tiennent pas : chacune selon son contenu (borné à la colonne).
        Some(c) if fit + c > room => room * fit / (fit + c.min(room)),
        _ => fit,
    };
    h.min(room - min)
}

/// Hauteur d'une partie, au moins : la barre d'onglets et une ligne.
pub const MIN_FIT: f32 = 56.0;

/// Les panneaux qui prennent la hauteur de leur contenu. Le panneau Espace 3D en est : c'est un onglet du groupe de
/// Réglages (`docs/INTERFACE.md`, partie 152.3), et devant, il prend la hauteur de ses cartes et de sa caméra au lieu de
/// garder celle que Réglages avait ; la couleur dessous prend le reste.
pub fn fits(pane: Pane) -> bool {
    matches!(pane, Pane::Tools | Pane::ToolOptions | Pane::Sheet | Pane::Effects | Pane::Vignettes | Pane::Espace3d)
}

/// Les panneaux dont on mesure le contenu : ceux qui s'ajustent, et la couleur (souple, mais dont
/// la hauteur ne dépend pas de celle du panneau ; les calques, eux, remplissent toujours le leur).
pub fn measured(pane: Pane) -> bool {
    fits(pane) || matches!(pane, Pane::Color)
}

/// Hauteur minimale d'un panneau qui prend le reste.
pub fn flex_min(pane: Pane) -> f32 {
    match pane {
        Pane::Color => 200.0,
        Pane::Layers | Pane::XSheet => 140.0,
        Pane::Canvas => 240.0,
        Pane::Montage | Pane::Timeline => 160.0,
        _ => 120.0,
    }
}

/// La hauteur de la partie du haut d'une colonne de `total` points (`sep` : l'épaisseur du bord
/// entre les deux), ou rien si aucune des deux ne s'ajuste. Deux parties qui s'ajustent : celle du
/// haut a sa hauteur et celle du bas le reste, ou chacune sa part si elles ne tiennent pas.
pub fn top_height(total: f32, sep: f32, top: Want, bottom: Want) -> Option<f32> {
    let room = total - sep;
    if room < 2.0 * MIN_FIT {
        return None;
    }
    let h = match (top, bottom) {
        (Want::Fit(a), Want::Flex { min, content }) => share(a, min, content, room),
        (Want::Flex { min, content }, Want::Fit(b)) => room - share(b, min, content, room),
        (Want::Fit(a), Want::Fit(b)) if a + b <= room => a,
        (Want::Fit(a), Want::Fit(b)) => room * a / (a + b),
        // Une liste qui sait prendre toute la place (le scénario, la bibliothèque, l'historique…, devant dans le groupe des
        // réglages de la colonne unique, partie 161) au-dessus de la couleur : la couleur a la hauteur de son contenu, la liste
        // le reste ; chacune garde son minimum.
        (Want::Flex { min: top_min, content: None }, Want::Flex { min, content: Some(c) }) => room - c.max(min).min(room - top_min),
        (Want::Flex { .. }, Want::Flex { .. }) => return None,
    };
    Some(h.clamp(MIN_FIT, room - MIN_FIT))
}

/// La part (`fraction` d'egui_dock) qui donne `top` points à la partie du haut.
pub fn fraction_for(total: f32, sep: f32, top: f32) -> f32 {
    ((top + sep * 0.5) / total).clamp(0.05, 0.95)
}

/// Interface sans animations (réglage du diagnostic des performances) : les bords sautent.
static ANIMATIONS_OFF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_animations_off(off: bool) {
    ANIMATIONS_OFF.store(off, std::sync::atomic::Ordering::Relaxed);
}

/// Un pas de `current` vers `target` : le bord glisse en quelques images au lieu de sauter.
pub fn ease(current: f32, target: f32) -> f32 {
    ease_with(current, target, ANIMATIONS_OFF.load(std::sync::atomic::Ordering::Relaxed))
}

/// `ease`; `instant` : d'un coup.
pub fn ease_with(current: f32, target: f32, instant: bool) -> f32 {
    let d = target - current;
    if instant || d.abs() < 1.0 {
        target
    } else {
        current + d * 0.45
    }
}

/// Le nœud `at` de `tree`, s'il existe.
fn node(tree: &Tree<Pane>, at: NodeIndex) -> Option<&Node<Pane>> {
    (at.0 < tree.len()).then(|| &tree[at])
}

/// Le panneau qui nomme une partie (le premier onglet de son premier groupe) : la paire de deux
/// parties reste la même quand on change d'onglet.
fn first_pane(tree: &Tree<Pane>, at: NodeIndex) -> Option<Pane> {
    match node(tree, at)? {
        Node::Leaf { tabs, .. } => tabs.first().copied(),
        Node::Vertical { .. } | Node::Horizontal { .. } => first_pane(tree, at.left()),
        Node::Empty => None,
    }
}

/// La partie `at` montre-t-elle l'un des panneaux `panes` ?
fn has_any(tree: &Tree<Pane>, at: NodeIndex, panes: &[Pane]) -> bool {
    match node(tree, at) {
        Some(Node::Leaf { tabs, .. }) => tabs.iter().any(|t| panes.contains(t)),
        Some(Node::Vertical { .. } | Node::Horizontal { .. }) => has_any(tree, at.left(), panes) || has_any(tree, at.right(), panes),
        _ => false,
    }
}

/// Le nombre de colonnes côte à côte de la partie `at`.
fn colonnes(tree: &Tree<Pane>, at: NodeIndex) -> usize {
    match node(tree, at) {
        Some(Node::Leaf { .. }) => 1,
        Some(Node::Horizontal { .. }) => colonnes(tree, at.left()) + colonnes(tree, at.right()),
        Some(Node::Vertical { .. }) => colonnes(tree, at.left()).max(colonnes(tree, at.right())),
        _ => 0,
    }
}

/// La largeur que prend une partie de côté de `colonnes` colonnes, à côté du dessin (partie 161) : `dock::COLONNE` par
/// colonne, en laissant au dessin au moins la moitié de la place.
pub fn largeur_de_cote(total: f32, colonnes: usize) -> f32 {
    (crate::dock::COLONNE * colonnes.max(1) as f32).min(total * 0.5)
}

/// La part (`fraction` d'egui_dock : celle de la partie de gauche) qui donne `w` points à la partie de droite (`right`), ou à
/// celle de gauche.
pub fn fraction_for_width(total: f32, sep: f32, w: f32, right: bool) -> f32 {
    let left = if right { total - w - sep * 0.5 } else { w + sep * 0.5 };
    (left / total).clamp(0.05, 0.95)
}

impl App {
    /// La colonne des panneaux garde sa largeur (`dock::COLONNE`, partie 161), quelle que soit la largeur de la fenêtre : la
    /// place gagnée va à l'image. Seules sont tenues les parties de côté posées à côté du dessin (ni le dessin ni la bande en
    /// elles) ; un bord que l'on a tiré à la main est retenu et n'est plus touché, comme les hauteurs (`fit_panes`). La
    /// largeur est prise d'un coup : c'est la fenêtre qui a changé, pas le contenu d'un panneau.
    pub(crate) fn fit_columns(&mut self, ctx: &egui::Context, state: &mut DockState<Pane>, sep: f32) {
        const DESSIN_OU_BANDE: [Pane; 3] = [Pane::Canvas, Pane::Montage, Pane::Timeline];
        let pressed = ctx.input(|i| i.pointer.any_down());
        let tree = state.main_surface_mut();
        let mut fixed_now = Vec::new();
        let mut moved = false;
        for i in 0..tree.len() {
            let at = NodeIndex(i);
            let Some(Node::Horizontal { rect, fraction, .. }) = node(tree, at) else { continue };
            let (rect, fraction) = (*rect, *fraction);
            let (Some(a), Some(b)) = (first_pane(tree, at.left()), first_pane(tree, at.right())) else { continue };
            // Le côté tenu : celui qui n'a ni le dessin ni la bande, à côté de celui qui a le dessin.
            let droite = !has_any(tree, at.right(), &DESSIN_OU_BANDE) && has_any(tree, at.left(), &[Pane::Canvas]);
            let gauche = !has_any(tree, at.left(), &DESSIN_OU_BANDE) && has_any(tree, at.right(), &[Pane::Canvas]);
            if !droite && !gauche {
                continue;
            }
            let key = (a, b);
            if self.settings.panes_fixed.contains(&key) {
                continue;
            }
            // Un bord qui n'est plus où on l'a mis : on l'a tiré à la main.
            if let Some(set) = self.fit_set.get(&key) {
                if (set - fraction).abs() > 1e-4 {
                    self.fit_set.remove(&key);
                    fixed_now.push(key);
                    continue;
                }
            }
            if pressed || !rect.is_finite() || rect.width() <= 0.0 {
                continue;
            }
            let cote = if droite { at.right() } else { at.left() };
            let w = largeur_de_cote(rect.width(), colonnes(tree, cote));
            let f = fraction_for_width(rect.width(), sep, w, droite);
            if (f - fraction).abs() * rect.width() < 0.5 {
                self.fit_set.insert(key, fraction);
                continue;
            }
            if let Node::Horizontal { fraction, .. } = &mut tree[at] {
                *fraction = f;
            }
            self.fit_set.insert(key, f);
            moved = true;
        }
        if moved {
            ctx.request_repaint();
        }
        if !fixed_now.is_empty() {
            self.settings.panes_fixed.extend(fixed_now);
            let _ = self.settings.save();
        }
    }

    /// Ce que demande la partie `at` d'une colonne : la hauteur de son contenu (mesurée à
    /// l'affichage, `pane_heights`) si son onglet devant s'ajuste, sinon le reste.
    fn want(&self, tree: &Tree<Pane>, at: NodeIndex, body_margin: f32) -> Option<Want> {
        match node(tree, at)? {
            Node::Leaf { rect, viewport, tabs, active, collapsed, .. } => {
                let pane = *tabs.get(active.0)?;
                if *collapsed {
                    return None;
                }
                // Barre d'onglets : ce qui sépare la partie du corps de l'onglet.
                let bar = if viewport.is_finite() && viewport.height() > 0.0 && viewport.height() <= rect.height() {
                    rect.height() - viewport.height()
                } else {
                    30.0
                };
                let content = self.pane_heights.get(&pane).filter(|_| measured(pane)).map(|h| h + bar + body_margin + 2.0);
                Some(match content {
                    Some(h) if fits(pane) => Want::Fit(h),
                    _ => Want::Flex { min: flex_min(pane), content },
                })
            }
            Node::Vertical { .. } | Node::Horizontal { .. } => Some(Want::Flex { min: 200.0, content: None }),
            Node::Empty => None,
        }
    }

    /// Après l'affichage des panneaux : chaque colonne de deux parties dont l'une s'ajuste glisse
    /// vers la hauteur de son contenu ; un bord que l'on a tiré est retenu et n'est plus touché.
    pub(crate) fn fit_panes(&mut self, ctx: &egui::Context, state: &mut DockState<Pane>, sep: f32, body_margin: f32) {
        self.fit_busy = false;
        if !self.settings.panes_fit {
            return;
        }
        // (Pas pendant un clic ou un glissé : rien ne bouge sous le pointeur.)
        let pressed = ctx.input(|i| i.pointer.any_down());
        let tree = state.main_surface_mut();
        let mut fixed_now = Vec::new();
        for i in 0..tree.len() {
            let at = NodeIndex(i);
            let Some(Node::Vertical { rect, fraction, .. }) = node(tree, at) else { continue };
            let (rect, fraction) = (*rect, *fraction);
            let (Some(a), Some(b)) = (first_pane(tree, at.left()), first_pane(tree, at.right())) else { continue };
            let key = (a, b);
            if self.settings.panes_fixed.contains(&key) {
                continue;
            }
            // Un bord qui n'est plus où on l'a mis : on l'a tiré à la main.
            if let Some(set) = self.fit_set.get(&key) {
                if (set - fraction).abs() > 1e-4 {
                    self.fit_set.remove(&key);
                    fixed_now.push(key);
                    continue;
                }
            }
            if pressed || !rect.is_finite() || rect.height() <= 0.0 {
                continue;
            }
            let (Some(top), Some(bottom)) = (self.want(tree, at.left(), body_margin), self.want(tree, at.right(), body_margin)) else { continue };
            let Some(target) = top_height(rect.height(), sep, top, bottom) else { continue };
            let current = rect.height() * fraction - sep * 0.5;
            let next = ease(current, target);
            if (next - current).abs() < 0.5 {
                continue;
            }
            let f = fraction_for(rect.height(), sep, next);
            if let Node::Vertical { fraction, .. } = &mut tree[at] {
                *fraction = f;
            }
            self.fit_set.insert(key, f);
            self.fit_busy = true;
        }
        if self.fit_busy {
            ctx.request_repaint();
        }
        if !fixed_now.is_empty() {
            self.settings.panes_fixed.extend(fixed_now);
            let _ = self.settings.save();
        }
    }

    /// « Réajuster les panneaux » : les bords tirés à la main sont oubliés, les panneaux
    /// s'ajustent de nouveau.
    pub(crate) fn refit_panes(&mut self) {
        self.settings.panes_fixed.clear();
        self.settings.panes_fit = true;
        self.fit_set.clear();
        self.status = match self.settings.save() {
            Ok(()) => "Panneaux réajustés à leur contenu.".into(),
            Err(e) => format!("Réglages non enregistrés : {e}"),
        };
    }
}
