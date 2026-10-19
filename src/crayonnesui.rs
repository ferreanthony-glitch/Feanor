//! L'écran des crayonnés rapides (`crayonnes.rs` : le modèle, les gestes, le fichier). La planche
//! prend la place du dessin dans le panneau central (comme la vue Vignettes) : une barre de commandes
//! en haut, la page de cases au milieu, une ligne d'aide en bas.
//!
//! - On dessine directement sur la page, avec le crayon, le pinceau ou la gomme du logiciel : le
//!   geste est lu ici, avant que la page ne soit peinte, et l'aperçu du trait est posé dans l'image de
//!   la case à chaque point reçu (même moteur que le dessin, aucun rejeu pendant le geste). Les textures
//!   de l'écran ne reçoivent que les tuiles touchées.
//! - Un trait appartient à la case où il commence ; sa case est un dessin à ses bornes.
//! - Le numéro de chaque case est une poignée : un clic choisit (Maj : jusqu'à, Ctrl : ajoute ou
//!   retire), un glisser change l'ordre. Un clic droit sur une case ouvre ses commandes.
//! - Tab / Maj+Tab et les flèches changent de case quand la planche a la main (`crayonnes_key`).

use std::collections::HashMap;

use eframe::egui::{self, pos2, vec2, Align2, Color32, ColorImage, CursorIcon, Event, FontId, Key, PointerButton, Rect, RichText, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, TouchPhase};
use egui_phosphor::regular as ph;

use crate::app::{App, Mode, Tool};
use crate::bulles;
use crate::crayonnes::{self, Cell, Live, Move, Outil, PEdit, PageHistory, PlanOptions, Placement, Planche, Style, Undone, CAPTION_MAX, GRIDS};
use crate::raster::Raster;

/// Hauteur de la barre de commandes et de la ligne d'aide, en points.
const BAR_H: f32 = 38.0;
const FOOT_H: f32 = 22.0;
/// Pause après laquelle la case suivante s'ouvre toute seule (si l'option est allumée), en secondes.
pub(crate) const AUTO_DELAY: f64 = 1.2;

/// Les trois encres de la barre : noir et deux gris.
pub(crate) const COLORS: [([u8; 3], &str); 3] = [([25, 25, 25], "Noir"), ([95, 95, 95], "Gris foncé"), ([170, 170, 170], "Gris clair")];
/// Les trois tailles du trait (pixels de la case), leur nom et la taille de leur icône.
pub(crate) const SIZES: [(f32, &str, f32); 3] = [(2.5, "Trait fin", 8.0), (5.0, "Trait moyen", 12.0), (10.0, "Trait gros", 16.0)];

/// Les textures de l'écran pour une case : l'encre et la peinture (créées quand il y a quelque chose
/// à montrer).
#[derive(Default)]
struct Textures {
    ink: Option<TextureHandle>,
    paint: Option<TextureHandle>,
}

/// Ce que la planche garde de la séance (rien de tout ça n'est dans le projet, sauf la planche).
pub(crate) struct Etat {
    /// La planche remplace le dessin dans le panneau central (en storyboard).
    pub open: bool,
    /// Le projet pour lequel l'état est valable (`App::doc_epoch`) : un autre projet le remet à zéro.
    epoch: u64,
    pub page: usize,
    /// La case courante (rang dans la planche) : celle qu'on vient de toucher, ou où l'on est allé.
    pub current: usize,
    /// Les cases choisies (leurs identifiants) et la dernière cliquée (départ des Maj + clic).
    pub selected: Vec<u64>,
    anchor: Option<usize>,
    pub color: usize,
    pub size: usize,
    pub history: PageHistory,
    /// Le geste en cours.
    pub live: Option<Live>,
    /// La case suivante s'ouvre toute seule après une pause (éteint au départ).
    pub auto_next: bool,
    /// L'instant où le dernier trait s'est fini (pour la case suivante automatique).
    last_stroke: Option<f64>,
    /// La planche a la main : le dernier appui était dedans (annuler, Suppr, Tab la visent).
    pub focus: bool,
    textures: HashMap<u64, Textures>,
    /// La case dont le numéro est glissé (pour changer l'ordre).
    reorder: Option<usize>,
    /// La fenêtre « Créer les plans » et ses réglages.
    pub dialog: bool,
    pub opts: PlanOptions,
    /// La planche telle qu'elle était quand une légende a pris la main (une étape d'annulation à la fin).
    caption_before: Option<Box<Planche>>,
    /// Les cases de la page, telles que peintes à la dernière image (essais, gestes).
    pub cells: Vec<Cell>,
}

impl Default for Etat {
    fn default() -> Self {
        Etat {
            open: false,
            epoch: 0,
            page: 0,
            current: 0,
            selected: Vec::new(),
            anchor: None,
            color: 0,
            size: 1,
            history: PageHistory::default(),
            live: None,
            auto_next: false,
            last_stroke: None,
            focus: false,
            textures: HashMap::new(),
            reorder: None,
            dialog: false,
            opts: PlanOptions::default(),
            caption_before: None,
            cells: Vec::new(),
        }
    }
}

/// Une commande du menu d'une case (clic droit).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cmd {
    Choose,
    InsertAfter,
    InsertBefore,
    Clear,
    Remove,
    Forward,
    Back,
    ToggleUsed,
    CreateOne,
}

/// Envoie `tiles` (ou tout) de `raster` dans la texture `slot` ; crée la texture s'il y a quelque
/// chose à montrer. `tiles` est vide quand rien n'a changé.
fn sync_layer(ctx: &egui::Context, slot: &mut Option<TextureHandle>, name: String, raster: &Raster, tiles: &[usize], all: bool) {
    let options = TextureOptions::LINEAR;
    let content = raster.tiles.iter().any(Option::is_some);
    if let (Some(texture), false) = (slot.as_mut(), all) {
        if tiles.is_empty() {
            return;
        }
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
        for &i in tiles.iter().filter(|&&i| i < raster.tiles.len()) {
            let (ox, oy, tw, th) = raster.tile_bounds(i);
            (x0, y0, x1, y1) = (x0.min(ox), y0.min(oy), x1.max(ox + tw), y1.max(oy + th));
        }
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let mut pixels = Vec::with_capacity((x1 - x0) * (y1 - y0));
        for y in y0..y1 {
            for x in x0..x1 {
                pixels.push(raster.get(x, y));
            }
        }
        texture.set_partial([x0, y0], ColorImage { size: [x1 - x0, y1 - y0], pixels }, options);
        return;
    }
    // Tout est à envoyer (case relue, annulée, vidée) ; ou la texture manque alors qu'il y a un dessin.
    if slot.is_none() && !content {
        return;
    }
    let image = ColorImage { size: [raster.width, raster.height], pixels: raster.to_flat() };
    match slot {
        Some(t) => t.set(image, options),
        None => *slot = Some(ctx.load_texture(name, image, options)),
    }
}

impl App {
    // ------------------------------------------------------------------ état

    /// La planche est à l'écran : elle a pris la place du dessin (en storyboard).
    pub(crate) fn crayonnes_shown(&self) -> bool {
        self.crayonnes.open && self.mode == Mode::Storyboard
    }

    /// La planche a la main : le dernier appui était dedans, aucune bande (montage, timeline, effets)
    /// ne l'a reprise depuis.
    pub(crate) fn crayonnes_has_focus(&self) -> bool {
        self.crayonnes_shown() && self.crayonnes.focus && !self.panel_focus && !self.images_focus && !self.key_focus && !self.fx_pane_focus
    }

    /// L'outil de la planche : celui du logiciel (le crayon, le pinceau, la gomme), le crayon pour tout autre.
    pub(crate) fn crayonnes_tool(&self) -> Outil {
        match self.tool {
            Tool::Brush => Outil::Pinceau,
            Tool::Eraser => Outil::Gomme,
            _ => Outil::Crayon,
        }
    }

    /// Le style choisi dans la barre ; `frame` : la case où l'on trace (pour le zoom du lissage).
    fn crayonnes_style(&self, frame: Rect) -> Style {
        let size = self.doc.crayonnes.size;
        Style {
            color: COLORS[self.crayonnes.color.min(COLORS.len() - 1)].0,
            size: SIZES[self.crayonnes.size.min(SIZES.len() - 1)].0,
            brush: self.brush,
            textures: self.brushes.textures(&self.brush),
            toucher: self.toucher,
            zoom: frame.width() / size[0].max(1) as f32,
        }
    }

    /// Met l'état d'accord avec le projet : un autre projet repart de zéro, la planche neuve a sa
    /// première page, la case courante et la page restent dans les bornes.
    fn crayonnes_prepare(&mut self) {
        if self.crayonnes.epoch != self.doc_epoch {
            let old = std::mem::take(&mut self.crayonnes);
            self.crayonnes = Etat { open: old.open, auto_next: old.auto_next, color: old.color, size: old.size, epoch: self.doc_epoch, ..Etat::default() };
        }
        let frame = self.doc.frame;
        self.doc.crayonnes.ensure(frame);
        let planche = &self.doc.crayonnes;
        let st = &mut self.crayonnes;
        st.current = st.current.min(planche.len().saturating_sub(1));
        st.page = st.page.min(planche.page_count() - 1);
        st.selected.retain(|id| planche.index_of(*id).is_some());
        if let Some(r) = st.reorder {
            if r >= planche.len() {
                st.reorder = None;
            }
        }
    }

    /// Les cases à traiter : les choisies, dans l'ordre de la planche.
    pub(crate) fn crayonnes_picks(&self) -> Vec<usize> {
        self.doc.crayonnes.picks(&self.crayonnes.selected)
    }

    /// Va à la case `index` (et à sa page).
    pub(crate) fn crayonnes_goto(&mut self, index: usize) {
        let planche = &self.doc.crayonnes;
        let index = index.min(planche.len().saturating_sub(1));
        self.crayonnes.current = index;
        self.crayonnes.page = planche.page_of(index);
        self.crayonnes.anchor = Some(index);
        self.status = format!("Case {} sur {} (page {} sur {}).", index + 1, planche.len(), self.crayonnes.page + 1, planche.page_count());
    }

    /// Change de case (Tab, flèches).
    pub(crate) fn crayonnes_move(&mut self, mv: Move) {
        let planche = &self.doc.crayonnes;
        let next = crayonnes::navigate(self.crayonnes.current, planche.len(), planche.grid.0, mv);
        if next == self.crayonnes.current {
            if matches!(mv, Move::Next | Move::Right) {
                self.status = "Dernière case de la planche : « Ajouter une page » pour continuer.".into();
            }
            return;
        }
        self.crayonnes_goto(next);
    }

    /// Change de page (la case courante passe à la première de la page si elle n'y est plus).
    pub(crate) fn crayonnes_page(&mut self, delta: isize) {
        let planche = &self.doc.crayonnes;
        let last = planche.page_count() - 1;
        let page = (self.crayonnes.page as isize + delta).clamp(0, last as isize) as usize;
        if page == self.crayonnes.page {
            return;
        }
        self.crayonnes.page = page;
        let range = planche.page_range(page);
        if !range.contains(&self.crayonnes.current) {
            self.crayonnes.current = range.start;
        }
        self.status = format!("Page {} sur {}.", page + 1, planche.page_count());
    }

    /// Une modification de la planche entière (ordre, cases ajoutées ou retirées, vidées, grille) : une
    /// étape d'annulation de la planche. `f` dit si quelque chose a changé.
    pub(crate) fn crayonnes_edit(&mut self, f: impl FnOnce(&mut Planche) -> bool) -> bool {
        self.crayonnes_end_live();
        let before = Box::new(self.doc.crayonnes.clone());
        if !f(&mut self.doc.crayonnes) {
            return false;
        }
        self.crayonnes.history.push(PEdit::Whole(before));
        self.modified = true;
        true
    }

    pub(crate) fn crayonnes_add_page(&mut self) {
        let mut first = 0;
        if self.crayonnes_edit(|p| {
            first = p.add_page();
            true
        }) {
            self.crayonnes_goto(first);
            self.status = format!("Page {} ajoutée.", self.crayonnes.page + 1);
        }
    }

    pub(crate) fn crayonnes_insert(&mut self, after: Option<usize>) {
        let cur = self.crayonnes.current;
        let mut at = 0;
        let before = after.is_none();
        if self.crayonnes_edit(|p| {
            at = if before { p.insert_before(cur) } else { p.insert_after(after.unwrap_or(cur)) };
            true
        }) {
            self.crayonnes_goto(at);
        }
    }

    pub(crate) fn crayonnes_remove(&mut self, index: usize) {
        if self.crayonnes_edit(|p| p.remove(index)) {
            let len = self.doc.crayonnes.len();
            self.crayonnes.selected.retain(|id| self.doc.crayonnes.index_of(*id).is_some());
            self.crayonnes_goto(index.min(len.saturating_sub(1)));
            self.status = format!("Case {} supprimée.", index + 1);
        }
    }

    /// Vide le dessin des cases choisies (sinon de la case courante).
    pub(crate) fn crayonnes_clear(&mut self) {
        let mut targets = self.crayonnes_picks();
        if targets.is_empty() {
            targets.push(self.crayonnes.current);
        }
        let n = targets.len();
        if self.crayonnes_edit(|p| targets.iter().fold(false, |any, &i| p.clear_case(i) | any)) {
            self.status = if n == 1 { "Case vidée (Ctrl+Z pour la rendre).".into() } else { format!("{n} cases vidées (Ctrl+Z pour les rendre).") };
        }
    }

    /// Choisit toutes les cases qui ont un dessin ou une légende.
    pub(crate) fn crayonnes_choose_all(&mut self) {
        let planche = &self.doc.crayonnes;
        let filled = planche.filled();
        if filled.is_empty() {
            self.status = "Aucune case n'est dessinée : rien à choisir.".into();
            return;
        }
        self.crayonnes.selected = filled.iter().map(|&i| planche.cases[i].id).collect();
        self.status = format!("{} case(s) choisie(s).", filled.len());
    }

    pub(crate) fn crayonnes_set_grid(&mut self, cols: usize, rows: usize) {
        let cur = self.crayonnes.current;
        if self.crayonnes_edit(|p| p.set_grid(cols, rows)) {
            self.crayonnes_goto(cur);
        }
    }

    /// Un geste en cours se termine (le trait lisse prend la place de l'aperçu).
    pub(crate) fn crayonnes_end_live(&mut self) {
        let Some(live) = self.crayonnes.live.take() else { return };
        if let Some(edit) = live.end(&mut self.doc.crayonnes) {
            self.crayonnes.history.push(edit);
            self.modified = true;
        }
        if self.crayonnes.auto_next {
            self.crayonnes.last_stroke = Some(self.input_time);
        }
    }

    // ------------------------------------------------------------------ annuler, rétablir

    pub(crate) fn crayonnes_undo(&mut self) {
        self.crayonnes_end_live();
        match self.crayonnes.history.undo(&mut self.doc.crayonnes) {
            Undone::Nothing => self.status = "Rien à annuler dans la planche.".into(),
            Undone::Page => self.modified = true,
            // Des plans avaient été faits : leur création est une étape du projet, qu'on défait si
            // c'est toujours la dernière (sinon les coches reviennent seules).
            Undone::Plans(depth) => {
                let (undo, _) = self.history.names();
                if undo.len() == depth && undo.last().copied() == Some(crayonnes::PLANS_STEP) {
                    self.undo();
                }
                self.modified = true;
            }
        }
    }

    pub(crate) fn crayonnes_redo(&mut self) {
        self.crayonnes_end_live();
        match self.crayonnes.history.redo(&mut self.doc.crayonnes) {
            Undone::Nothing => self.status = "Rien à rétablir dans la planche.".into(),
            Undone::Page => self.modified = true,
            Undone::Plans(depth) => {
                let (undo, redo) = self.history.names();
                if undo.len() + 1 == depth && redo.first().copied() == Some(crayonnes::PLANS_STEP) {
                    self.redo();
                }
                self.modified = true;
            }
        }
    }

    // ------------------------------------------------------------------ créer les plans

    /// « Créer les plans » : un plan par case choisie (voir `crayonnes::create_plans`), en une seule
    /// étape d'annulation du projet ; les cases restent sur la planche, cochées.
    pub(crate) fn crayonnes_create_plans(&mut self, opts: PlanOptions) -> bool {
        if self.storyboard_refuses() {
            return false;
        }
        self.crayonnes_end_live();
        let picks = self.crayonnes_picks();
        if picks.is_empty() {
            self.status = "Choisis d'abord des cases (clic sur leur numéro, Maj, Ctrl, ou « Toutes »).".into();
            return false;
        }
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        let planche = std::mem::take(&mut self.doc.crayonnes);
        let used_before: Vec<(u64, bool)> = picks.iter().filter_map(|&i| planche.cases.get(i)).map(|c| (c.id, c.used)).collect();
        let created = crayonnes::create_plans(&mut self.doc, &planche, &picks, self.cur_panel, &opts);
        self.doc.crayonnes = planche;
        let Some(created) = created else {
            self.status = "Aucun plan n'a pu être créé.".into();
            return false;
        };
        self.doc.crayonnes.mark_used(&picks, true);
        self.cur_panel = created.first;
        self.cur_frame = 0;
        self.cur_layer = self.layers().len().saturating_sub(1);
        self.vsel = None;
        self.follow_frame = true;
        // (Son nom dans l'historique du projet : c'est lui que la planche reconnaît pour le défaire.)
        let context = self.history.context.replace(crayonnes::PLANS_STEP.to_string());
        self.commit_structure(snap);
        self.history.context = context;
        let depth = self.history.names().0.len();
        self.crayonnes.history.push(PEdit::Plans { used: used_before, depth });
        let n = created.ids.len();
        self.status = format!("{n} plan(s) créé(s) d'après la planche (un seul Ctrl+Z les défait).");
        true
    }

    // ------------------------------------------------------------------ commandes et touches

    /// Ouvre ou ferme la planche.
    fn crayonnes_toggle(&mut self) {
        if self.crayonnes_shown() {
            self.crayonnes_end_live();
            self.crayonnes.open = false;
            self.crayonnes.dialog = false;
            self.status = "Planche de crayonnés fermée : le dessin du plan est revenu.".into();
            return;
        }
        self.end_stroke();
        self.playing = false;
        if self.mode != Mode::Storyboard {
            self.set_mode(Mode::Storyboard);
        }
        self.crayonnes.open = true;
        self.crayonnes.focus = true;
        self.panel_focus = false;
        self.images_focus = false;
        self.key_focus = false;
        // Les croquis se font au crayon (trait vectoriel, net à toute échelle) ; la gomme reste la gomme.
        if self.tool != Tool::Eraser {
            self.run_action("outil.crayon");
        }
        self.status = "Planche de crayonnés : dessine dans une case, Tab pour la suivante ; choisis des cases (clic sur le numéro) puis « Créer les plans ».".into();
    }

    /// Les commandes de la planche et les commandes d'édition quand elle a la main. Vrai si l'action
    /// est prise ici.
    pub(crate) fn crayonnes_action(&mut self, action: &str) -> bool {
        let own = action == "storyboard.crayonnes" || action.starts_with("crayonnes.");
        let edit = self.crayonnes_has_focus() && matches!(action, "edition.annuler" | "edition.retablir" | "edition.tout_selectionner" | "edition.supprimer");
        if !own && !edit {
            return false;
        }
        // Une commande finit d'abord le geste en cours.
        self.crayonnes_end_live();
        if action == "storyboard.crayonnes" {
            self.crayonnes_toggle();
            return true;
        }
        if own && !self.crayonnes_shown() {
            self.status = "Ouvre d'abord la planche de crayonnés (Storyboard > Planche de crayonnés).".into();
            return true;
        }
        let cur = self.crayonnes.current;
        match action {
            "crayonnes.case_suivante" => self.crayonnes_move(Move::Next),
            "crayonnes.case_precedente" => self.crayonnes_move(Move::Prev),
            "crayonnes.page_suivante" => self.crayonnes_page(1),
            "crayonnes.page_precedente" => self.crayonnes_page(-1),
            "crayonnes.ajouter_page" => self.crayonnes_add_page(),
            "crayonnes.inserer_case" => self.crayonnes_insert(Some(cur)),
            "crayonnes.supprimer_case" => self.crayonnes_remove(cur),
            "crayonnes.vider_case" => self.crayonnes_clear(),
            "crayonnes.tout_choisir" | "edition.tout_selectionner" => self.crayonnes_choose_all(),
            "crayonnes.creer_plans" => {
                if self.crayonnes_picks().is_empty() {
                    self.status = "Choisis d'abord des cases (clic sur leur numéro, Maj, Ctrl, ou « Toutes »).".into();
                } else {
                    self.crayonnes.dialog = true;
                }
            }
            "crayonnes.case_auto" => {
                self.crayonnes.auto_next = !self.crayonnes.auto_next;
                self.crayonnes.last_stroke = None;
                self.status = if self.crayonnes.auto_next { "Case suivante automatique : après une pause, la case suivante s'ouvre.".into() } else { "Case suivante automatique éteinte.".into() };
            }
            "edition.annuler" => self.crayonnes_undo(),
            "edition.retablir" => self.crayonnes_redo(),
            "edition.supprimer" => self.crayonnes_clear(),
            _ => return false,
        }
        true
    }

    /// Une touche quand la planche a la main : Tab / Maj+Tab et les flèches changent de case, Page
    /// précédente / suivante de page. Vrai si la touche est prise.
    pub(crate) fn crayonnes_key(&mut self, key: Key, m: egui::Modifiers) -> bool {
        if !self.crayonnes_has_focus() || m.command || m.alt || m.ctrl {
            return false;
        }
        // Pendant un trait, ces touches ne font rien (Tab ne doit pas passer en animation au milieu d'un geste).
        let moving = matches!(key, Key::Tab | Key::ArrowRight | Key::ArrowLeft | Key::ArrowUp | Key::ArrowDown | Key::PageDown | Key::PageUp);
        if self.crayonnes.live.is_some() {
            return moving;
        }
        match key {
            Key::Tab if m.shift => self.crayonnes_move(Move::Prev),
            Key::Tab => self.crayonnes_move(Move::Next),
            Key::ArrowRight => self.crayonnes_move(Move::Right),
            Key::ArrowLeft => self.crayonnes_move(Move::Left),
            Key::ArrowUp => self.crayonnes_move(Move::Up),
            Key::ArrowDown => self.crayonnes_move(Move::Down),
            Key::PageDown => self.crayonnes_page(1),
            Key::PageUp => self.crayonnes_page(-1),
            _ => return false,
        }
        true
    }

    /// Une commande du menu d'une case.
    fn crayonnes_run(&mut self, cmd: Cmd, index: usize) {
        match cmd {
            Cmd::Choose => {
                let id = self.doc.crayonnes.cases[index].id;
                self.crayonnes.selected = vec![id];
                self.crayonnes_goto(index);
            }
            Cmd::InsertAfter => self.crayonnes_insert(Some(index)),
            Cmd::InsertBefore => {
                self.crayonnes.current = index;
                self.crayonnes_insert(None);
            }
            Cmd::Clear => {
                let id = self.doc.crayonnes.cases[index].id;
                if !self.crayonnes.selected.contains(&id) {
                    self.crayonnes.selected = vec![id];
                }
                self.crayonnes_clear();
            }
            Cmd::Remove => self.crayonnes_remove(index),
            Cmd::Forward => {
                if self.crayonnes_edit(|p| p.move_case(index, index + 1)) {
                    self.crayonnes_goto(index + 1);
                }
            }
            Cmd::Back => {
                if index > 0 && self.crayonnes_edit(|p| p.move_case(index, index - 1)) {
                    self.crayonnes_goto(index - 1);
                }
            }
            Cmd::ToggleUsed => {
                let used = !self.doc.crayonnes.cases[index].used;
                self.crayonnes_edit(|p| {
                    p.mark_used(&[index], used);
                    true
                });
            }
            Cmd::CreateOne => {
                let id = self.doc.crayonnes.cases[index].id;
                self.crayonnes.selected = vec![id];
                self.crayonnes_goto(index);
                self.crayonnes.dialog = true;
            }
        }
    }

    // ------------------------------------------------------------------ l'écran

    /// La planche, dans le panneau central.
    pub(crate) fn crayonnes_panel(&mut self, ui: &mut egui::Ui) {
        self.crayonnes_prepare();
        let rect = ui.available_rect_before_wrap();
        ui.allocate_rect(rect, Sense::hover());
        let bar = Rect::from_min_size(rect.min, vec2(rect.width(), BAR_H));
        let foot = Rect::from_min_max(pos2(rect.left(), (rect.bottom() - FOOT_H).max(bar.bottom())), rect.max);
        let area = Rect::from_min_max(pos2(rect.left(), bar.bottom()), pos2(rect.right(), foot.top()));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bar).id_salt("crayonnes_barre"));
        self.crayonnes_bar(&mut child);
        self.crayonnes_grid(ui, area);
        ui.painter().rect_filled(foot, 0.0, crate::theme::entete());
        ui.painter().text(
            foot.left_center() + vec2(8.0, 0.0),
            Align2::LEFT_CENTER,
            "Tab, flèches : autre case · numéro d'une case : choisir (Maj : jusqu'à, Ctrl : ajouter) · glisser le numéro : changer l'ordre · clic droit : commandes de la case",
            FontId::proportional(11.5),
            crate::theme::texte_doux(),
        );
        let ctx = ui.ctx().clone();
        self.crayonnes_dialog(&ctx);
    }

    /// La barre de commandes de la planche.
    fn crayonnes_bar(&mut self, ui: &mut egui::Ui) {
        ui.painter().rect_filled(ui.max_rect(), 0.0, crate::theme::entete());
        let mut act: Option<&'static str> = None;
        let mut undo = None;
        let mut grid = None;
        egui::ScrollArea::horizontal().id_salt("crayonnes_barre_defil").auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                let sep = |ui: &mut egui::Ui| {
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);
                };
                ui.add_space(6.0);
                // Les outils : ceux du logiciel.
                let tool = self.crayonnes_tool();
                let tools = [
                    (Outil::Crayon, ph::PENCIL_SIMPLE, "Crayon", "Un trait lisse, vectoriel : il reste net quand la case devient un plan.", "outil.crayon"),
                    (Outil::Pinceau, ph::PAINT_BRUSH, "Pinceau", "Le pinceau du logiciel (bitmap, avec ses textures) : la case devient un calque bitmap, rééchantillonné.", "outil.pinceau"),
                    (Outil::Gomme, ph::ERASER, "Gomme", "Coupe les traits et efface la peinture de la case.", "outil.gomme"),
                ];
                for (t, icon, name, what, action) in tools {
                    if self.bouton_etat(ui, tool == t, icon, name, what, action).clicked() {
                        act = Some(action);
                    }
                }
                sep(ui);
                // Trois tailles.
                for (k, (_, name, icon)) in SIZES.iter().enumerate() {
                    let on = self.crayonnes.size == k;
                    let resp = ui.add(egui::Button::new(bulles::symbole_etat(ph::CIRCLE, on).size(*icon)).selected(on).min_size(vec2(24.0, 0.0)));
                    if bulles::bulle(resp, name, "L'épaisseur du crayon, du pinceau et de la gomme dans les cases.", "").clicked() {
                        self.crayonnes.size = k;
                    }
                }
                sep(ui);
                // Le noir et deux gris.
                for (k, (rgb, name)) in COLORS.iter().enumerate() {
                    let (rect, resp) = ui.allocate_exact_size(vec2(20.0, 22.0), Sense::click());
                    let c = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                    ui.painter().circle_filled(rect.center(), 7.0, c);
                    let ring = if self.crayonnes.color == k { crate::theme::accent_texte() } else { crate::theme::bord() };
                    ui.painter().circle_stroke(rect.center(), 8.5, Stroke::new(if self.crayonnes.color == k { 2.0_f32 } else { 1.0_f32 }, ring));
                    if bulles::bulle(resp, name, "La couleur du crayon et du pinceau.", "").clicked() {
                        self.crayonnes.color = k;
                    }
                }
                sep(ui);
                let (can_undo, can_redo) = (self.crayonnes.history.can_undo(), self.crayonnes.history.can_redo());
                if self.bouton_si(ui, can_undo, ph::ARROW_COUNTER_CLOCKWISE, "Annuler", "Défait le dernier geste ou la dernière modification de la planche.", "edition.annuler").clicked() {
                    undo = Some(false);
                }
                if self.bouton_si(ui, can_redo, ph::ARROW_CLOCKWISE, "Rétablir", "Refait ce qui vient d'être annulé.", "edition.retablir").clicked() {
                    undo = Some(true);
                }
                sep(ui);
                if self.bouton(ui, ph::BROOM, "Vider la case", "Efface le dessin de la case courante (ou des cases choisies) ; la légende reste.", "crayonnes.vider_case").clicked() {
                    act = Some("crayonnes.vider_case");
                }
                if self.bouton(ui, ph::PLUS_SQUARE, "Insérer une case", "Une case vide juste après la case courante ; les suivantes reculent d'un cran.", "crayonnes.inserer_case").clicked() {
                    act = Some("crayonnes.inserer_case");
                }
                if self.bouton(ui, ph::TRASH, "Supprimer la case", "Retire la case courante de la planche (les suivantes avancent).", "crayonnes.supprimer_case").clicked() {
                    act = Some("crayonnes.supprimer_case");
                }
                sep(ui);
                // Les pages.
                let (page, pages) = (self.crayonnes.page, self.doc.crayonnes.page_count());
                if self.bouton_si(ui, page > 0, ph::CARET_LEFT, "Page précédente", "", "crayonnes.page_precedente").clicked() {
                    act = Some("crayonnes.page_precedente");
                }
                ui.label(RichText::new(format!("Page {} / {}", page + 1, pages)).color(crate::theme::texte()));
                if self.bouton_si(ui, page + 1 < pages, ph::CARET_RIGHT, "Page suivante", "", "crayonnes.page_suivante").clicked() {
                    act = Some("crayonnes.page_suivante");
                }
                if self.bouton(ui, ph::FILE_PLUS, "Ajouter une page", "Une page de cases vides à la fin de la planche.", "crayonnes.ajouter_page").clicked() {
                    act = Some("crayonnes.ajouter_page");
                }
                // La grille.
                let (cols, rows) = self.doc.crayonnes.grid;
                egui::ComboBox::from_id_salt("crayonnes_grille").width(64.0).selected_text(format!("{cols} × {rows}")).show_ui(ui, |ui| {
                    for (c, r) in GRIDS {
                        if ui.selectable_label((c, r) == (cols, rows), format!("{c} × {r}")).clicked() {
                            grid = Some((c, r));
                        }
                    }
                });
                if self.bouton_etat(ui, self.crayonnes.auto_next, ph::TIMER, "Case suivante automatique", "Après une pause d'une seconde environ, la case suivante devient la case courante (éteint au départ).", "crayonnes.case_auto").clicked() {
                    act = Some("crayonnes.case_auto");
                }
                sep(ui);
                // Choisir et créer.
                if self.bouton(ui, ph::SELECTION_ALL, "Toutes les cases dessinées", "Choisit toutes les cases qui ont un dessin ou une légende.", "crayonnes.tout_choisir").clicked() {
                    act = Some("crayonnes.tout_choisir");
                }
                let n = self.crayonnes_picks().len();
                let label = if n == 0 { "Créer les plans".to_string() } else { format!("Créer {n} plan{}", if n > 1 { "s" } else { "" }) };
                let resp = ui.add_enabled(n > 0, egui::Button::new(RichText::new(format!("{} {label}", ph::FILM_STRIP))));
                if bulles::bulle(resp, "Créer les plans", "Un plan par case choisie, dans l'ordre de la planche : le dessin sur un calque « Crayonné », la légende en texte d'action.", &self.raccourci("crayonnes.creer_plans")).clicked() {
                    act = Some("crayonnes.creer_plans");
                }
                sep(ui);
                if self.bouton(ui, ph::X, "Fermer la planche", "Revient au dessin du plan.", "storyboard.crayonnes").clicked() {
                    act = Some("storyboard.crayonnes");
                }
            });
        });
        if let Some(redo) = undo {
            self.crayonnes.focus = true;
            if redo {
                self.crayonnes_redo();
            } else {
                self.crayonnes_undo();
            }
        }
        if let Some((c, r)) = grid {
            self.crayonnes_set_grid(c, r);
        }
        if let Some(a) = act {
            self.crayonnes.focus = true;
            self.run_action(a);
        }
    }

    /// La page de cases : les gestes d'abord (le trait est dans l'image avant qu'elle soit peinte),
    /// puis les textures, la peinture, les numéros, les légendes.
    fn crayonnes_grid(&mut self, ui: &mut egui::Ui, area: Rect) {
        let ctx = ui.ctx().clone();
        let (cols, rows) = self.doc.crayonnes.grid;
        let size = self.doc.crayonnes.size;
        let aspect = size[0] as f32 / size[1].max(1) as f32;
        let all = crayonnes::layout(area, cols, rows, aspect);
        let range = self.doc.crayonnes.page_range(self.crayonnes.page);
        let first = range.start;
        let shown = range.len().min(all.len());
        let cells: Vec<Cell> = all[..shown].to_vec();
        self.crayonnes.cells = cells.clone();

        self.crayonnes_input(&ctx, area, ui.layer_id(), &cells, first);
        self.crayonnes_auto_next(&ctx);

        let painter = ui.painter_at(area);
        painter.rect_filled(area, 0.0, crate::theme::fond());
        // Les textures des cases hors de la page sont libérées (la mémoire reste bornée).
        let visible: Vec<u64> = (first..first + shown).map(|i| self.doc.crayonnes.cases[i].id).collect();
        self.crayonnes.textures.retain(|id, _| visible.contains(id));

        let tip = self.crayonnes.live.as_ref().and_then(|l| l.tip().map(|t| (l.case, t)));
        let selected = self.crayonnes.selected.clone();
        let (current, reorder) = (self.crayonnes.current, self.crayonnes.reorder);
        let accent = crate::theme::accent_texte();
        let pointer = ctx.input(|i| i.pointer.latest_pos());
        let drop_slot = reorder.and_then(|_| pointer.and_then(|p| crayonnes::hit(&cells, p)));
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        let mut cmd: Option<(Cmd, usize)> = None;
        let mut clicked: Option<(usize, egui::Modifiers)> = None;
        let mut drag_start: Option<usize> = None;
        let mut dropped: Option<(usize, Option<usize>)> = None;
        let mods = ctx.input(|i| i.modifiers);

        for (slot, cell) in cells.iter().enumerate() {
            let index = first + slot;
            // Textures : seulement ce qui a changé.
            let case = &mut self.doc.crayonnes.cases[index];
            case.render_if_stale();
            let (id, used) = (case.id, case.used);
            let d = case.dirty.take();
            let tex = self.crayonnes.textures.entry(id).or_default();
            sync_layer(&ctx, &mut tex.paint, format!("crayonnes_peinture_{id}"), &case.paint.raster, &d.paint, d.all);
            sync_layer(&ctx, &mut tex.ink, format!("crayonnes_encre_{id}"), &case.ink.raster, &d.ink, d.all);

            let frame = cell.frame;
            painter.rect_filled(frame.translate(vec2(2.0, 3.0)), 2.0, Color32::from_black_alpha(80));
            painter.rect_filled(frame, 1.0, Color32::from_gray(252));
            if let Some(t) = &tex.paint {
                painter.image(t.id(), frame, uv, Color32::WHITE);
            }
            if let Some(t) = &tex.ink {
                painter.image(t.id(), frame, uv, Color32::WHITE);
            }
            // Le bout du trait en cours, relié à la pointe (le lissage le laisse un peu en retard).
            if let Some((owner, ((last, raw, r), (rgb, alpha)))) = tip {
                if owner == id {
                    let k = frame.width() / size[0].max(1) as f32;
                    let to_screen = |p: egui::Pos2| frame.min + p.to_vec2() * k;
                    let color = Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], (alpha * 255.0).round() as u8);
                    let clipped = painter.with_clip_rect(frame);
                    let w = (r * k).max(0.5);
                    if to_screen(last).distance(to_screen(raw)) > 0.5 {
                        clipped.line_segment([to_screen(last), to_screen(raw)], Stroke::new(2.0 * w, color));
                    }
                    clipped.circle_filled(to_screen(raw), w, color);
                }
            }
            let is_selected = selected.contains(&id);
            painter.rect_stroke(frame, 1.0, Stroke::new(1.0_f32, crate::theme::bord()), StrokeKind::Outside);
            if is_selected {
                painter.rect_stroke(frame.expand(2.0), 2.0, Stroke::new(3.0_f32, accent), StrokeKind::Outside);
            } else if index == current {
                painter.rect_stroke(frame.expand(2.0), 2.0, Stroke::new(1.5_f32, accent.gamma_multiply(0.6)), StrokeKind::Outside);
            }
            if reorder == Some(index) {
                painter.rect_filled(frame, 1.0, Color32::from_black_alpha(90));
            }
            if drop_slot == Some(slot) && reorder != Some(index) {
                let bar = Rect::from_min_max(pos2(frame.left() - 7.0, frame.top()), pos2(frame.left() - 3.0, frame.bottom()));
                painter.rect_filled(bar, 2.0, accent);
            }
            // La coche : des plans ont été faits d'après cette case.
            if used {
                let c = pos2(frame.right() - 11.0, frame.top() + 11.0);
                painter.circle_filled(c, 9.5, crate::theme::succes());
                painter.text(c, Align2::CENTER_CENTER, ph::CHECK, FontId::proportional(12.0), Color32::WHITE);
            }
            // Le numéro : une poignée (choisir, changer l'ordre).
            let badge = cell.badge;
            painter.rect_filled(badge, 3.0, if is_selected { accent } else { crate::theme::entete().gamma_multiply(0.92) });
            let ink = if is_selected { crate::theme::sur_accent() } else { crate::theme::texte() };
            painter.text(badge.center(), Align2::CENTER_CENTER, format!("{}", index + 1), FontId::proportional(12.5), ink);
            // (La case d'abord, la poignée du numéro ensuite : le dernier posé est dessus.)
            let body = ui.interact(frame, ui.id().with(("crayonnes_case", id)), Sense::click());
            let handle = ui.interact(badge, ui.id().with(("crayonnes_numero", id)), Sense::click_and_drag());
            let handle = bulles::bulle(handle, &format!("Case {}", index + 1), "Clic : choisir (Maj : jusqu'à cette case, Ctrl : ajouter ou retirer). Glisser : changer l'ordre des cases.", "");
            if handle.clicked() {
                clicked = Some((index, mods));
            }
            if handle.drag_started() {
                drag_start = Some(index);
            }
            if handle.drag_stopped() {
                dropped = Some((index, pointer.and_then(|p| crayonnes::hit(&cells, p)).map(|s| first + s)));
            }
            // Clic droit : les commandes de la case.
            for resp in [&body, &handle] {
                resp.context_menu(|ui| {
                    if let Some(c) = self.crayonnes_case_menu(ui, index) {
                        cmd = Some((c, index));
                    }
                });
            }
        }

        // Les légendes : une ligne de texte sous chaque case, qui devient le texte d'action du plan.
        let mut caption_focus = false;
        for (slot, cell) in cells.iter().enumerate() {
            let index = first + slot;
            let mut text = self.doc.crayonnes.cases[index].caption.clone();
            let edit = egui::TextEdit::singleline(&mut text)
                .hint_text("Légende…")
                .char_limit(CAPTION_MAX)
                .font(egui::TextStyle::Small)
                .desired_width(cell.caption.width());
            let resp = ui.put(cell.caption, edit);
            if resp.gained_focus() {
                self.crayonnes.caption_before = Some(Box::new(self.doc.crayonnes.clone()));
                self.crayonnes.current = index;
                self.crayonnes.focus = false;
            }
            if resp.changed() {
                self.doc.crayonnes.cases[index].caption = text;
                self.modified = true;
            }
            if resp.has_focus() {
                caption_focus = true;
            }
            if resp.lost_focus() {
                if let Some(before) = self.crayonnes.caption_before.take() {
                    if before.cases.iter().zip(&self.doc.crayonnes.cases).any(|(a, b)| a.caption != b.caption) {
                        self.crayonnes.history.push(PEdit::Whole(before));
                    }
                }
            }
        }
        let _ = caption_focus;

        // Le curseur : une croix sur les cases.
        if let Some(p) = pointer {
            if ctx.layer_id_at(p) == Some(ui.layer_id()) && crayonnes::hit(&cells, p).is_some() && self.crayonnes.reorder.is_none() {
                ctx.set_cursor_icon(CursorIcon::Crosshair);
            }
        }
        // Le numéro glissé : une étiquette suit la souris.
        if let (Some(from), Some(p)) = (reorder, pointer) {
            let r = Rect::from_center_size(p + vec2(14.0, 14.0), vec2(34.0, 22.0));
            painter.rect_filled(r, 4.0, accent);
            painter.text(r.center(), Align2::CENTER_CENTER, format!("{}", from + 1), FontId::proportional(13.0), crate::theme::sur_accent());
            ctx.set_cursor_icon(CursorIcon::Grabbing);
        }

        // Ce que les poignées et les menus ont demandé.
        if let Some(index) = drag_start {
            self.crayonnes_end_live();
            self.crayonnes.focus = true;
            self.crayonnes.reorder = Some(index);
        }
        if let Some((from, to)) = dropped {
            self.crayonnes.reorder = None;
            if let Some(to) = to.filter(|&t| t != from) {
                if self.crayonnes_edit(|p| p.move_case(from, to)) {
                    self.crayonnes_goto(to);
                    self.status = format!("Case {} passée à la place {}.", from + 1, to + 1);
                }
            }
        }
        if let Some((index, m)) = clicked {
            self.crayonnes.focus = true;
            let ids: Vec<u64> = self.doc.crayonnes.cases.iter().map(|c| c.id).collect();
            let (sel, anchor) = crayonnes::choose(&self.crayonnes.selected, self.crayonnes.anchor, &ids, index, m.command || m.ctrl, m.shift);
            self.crayonnes.selected = sel;
            self.crayonnes.anchor = anchor;
            self.crayonnes.current = index;
            self.status = format!("{} case(s) choisie(s).", self.crayonnes.selected.len());
        }
        if let Some((c, index)) = cmd {
            self.crayonnes_run(c, index);
        }
    }

    /// Le menu du clic droit sur une case.
    fn crayonnes_case_menu(&mut self, ui: &mut egui::Ui, index: usize) -> Option<Cmd> {
        let mut out = None;
        let mut item = |ui: &mut egui::Ui, text: &str, cmd: Cmd, enabled: bool| {
            if ui.add_enabled(enabled, egui::Button::new(text)).clicked() {
                out = Some(cmd);
                ui.close_menu();
            }
        };
        let (len, used) = (self.doc.crayonnes.len(), self.doc.crayonnes.cases[index].used);
        item(ui, "Choisir cette case", Cmd::Choose, true);
        item(ui, "Créer le plan de cette case…", Cmd::CreateOne, true);
        ui.separator();
        item(ui, "Insérer une case avant", Cmd::InsertBefore, true);
        item(ui, "Insérer une case après", Cmd::InsertAfter, true);
        item(ui, "Reculer la case (vers le début)", Cmd::Back, index > 0);
        item(ui, "Avancer la case (vers la fin)", Cmd::Forward, index + 1 < len);
        ui.separator();
        item(ui, "Vider le dessin", Cmd::Clear, true);
        item(ui, "Supprimer la case", Cmd::Remove, true);
        item(ui, if used { "Retirer la coche (case utilisée)" } else { "Cocher la case (utilisée)" }, Cmd::ToggleUsed, true);
        out
    }

    /// Les gestes sur la page : choisir (Maj ou Ctrl + appui), tracer (appui, déplacements, lâcher).
    fn crayonnes_input(&mut self, ctx: &egui::Context, area: Rect, layer: egui::LayerId, cells: &[Cell], first: usize) {
        let (events, primary_down, now) = ctx.input(|i| (i.events.clone(), i.pointer.primary_down(), i.time));
        let size = self.doc.crayonnes.size;
        let modal = self.modal_open();
        // Horodatage par événement, comme le dessin (`canvas.rs`) : le lissage selon le temps en vient.
        let pointer_events = events.iter().filter(|e| matches!(e, Event::PointerMoved(_) | Event::PointerButton { .. })).count();
        let mut k = 0;
        for event in &events {
            if matches!(event, Event::PointerMoved(_) | Event::PointerButton { .. }) {
                k += 1;
                self.input_time = crate::brush::spread_time(self.last_frame_time, now, k, pointer_events);
            }
            match event {
                Event::Touch { phase, force, .. } => {
                    if let Some(f) = force {
                        self.pen_pressure = f.clamp(0.0, 1.0);
                    }
                    if *phase == TouchPhase::Start {
                        self.pen_touch = true;
                    }
                }
                Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers, .. } => {
                    let pen = std::mem::take(&mut self.pen_touch);
                    if modal || !area.contains(*pos) || ctx.layer_id_at(*pos) != Some(layer) {
                        continue;
                    }
                    let Some(slot) = crayonnes::hit(cells, *pos) else { continue };
                    // Le numéro est une poignée (choisir, glisser) : elle prend l'appui.
                    if cells[slot].badge.contains(*pos) {
                        continue;
                    }
                    let index = first + slot;
                    // La planche reprend la main (aux bandes, aux légendes).
                    self.panel_focus = false;
                    self.images_focus = false;
                    self.key_focus = false;
                    self.fx_pane_focus = false;
                    self.crayonnes.focus = true;
                    if ctx.wants_keyboard_input() {
                        if let Some(id) = ctx.memory(|m| m.focused()) {
                            ctx.memory_mut(|m| m.surrender_focus(id));
                        }
                    }
                    self.crayonnes.current = index;
                    self.crayonnes.page = self.doc.crayonnes.page_of(index);
                    self.crayonnes.last_stroke = None;
                    // Maj ou Ctrl + appui dans une case : la choisir, sans tracer.
                    if modifiers.shift || modifiers.command || modifiers.ctrl {
                        let ids: Vec<u64> = self.doc.crayonnes.cases.iter().map(|c| c.id).collect();
                        let (sel, anchor) = crayonnes::choose(&self.crayonnes.selected, self.crayonnes.anchor, &ids, index, modifiers.command || modifiers.ctrl, modifiers.shift);
                        self.crayonnes.selected = sel;
                        self.crayonnes.anchor = anchor;
                        continue;
                    }
                    self.crayonnes_end_live();
                    let tool = self.crayonnes_tool();
                    let style = self.crayonnes_style(cells[slot].frame);
                    let pressure = if pen { self.pen_pressure } else { 1.0 };
                    let local = crayonnes::to_local(cells[slot].frame, size, *pos);
                    let time = self.input_time;
                    self.crayonnes.live = self.doc.crayonnes.begin(index, tool, &style, local, pressure, pen, time);
                }
                Event::PointerMoved(pos) => {
                    let Some(live) = self.crayonnes.live.as_mut() else { continue };
                    let Some(index) = self.doc.crayonnes.index_of(live.case) else { continue };
                    let Some(cell) = index.checked_sub(first).and_then(|s| cells.get(s)) else { continue };
                    let pressure = if live.pen { self.pen_pressure } else { 1.0 };
                    let local = crayonnes::to_local(cell.frame, size, *pos);
                    live.to(&mut self.doc.crayonnes, local, pressure, self.input_time);
                }
                Event::PointerButton { button: PointerButton::Primary, pressed: false, .. } | Event::PointerGone => {
                    self.crayonnes_end_live();
                }
                _ => {}
            }
        }
        self.input_time = now;
        self.last_frame_time = now;
        // Relâchement manqué (perte de focus…) : on clôt le geste quand même.
        if !primary_down {
            self.crayonnes_end_live();
        }
    }

    /// Après une pause, la case suivante devient la case courante (si l'option est allumée).
    fn crayonnes_auto_next(&mut self, ctx: &egui::Context) {
        let Some(t) = self.crayonnes.last_stroke else { return };
        if !self.crayonnes.auto_next || self.crayonnes.live.is_some() {
            self.crayonnes.last_stroke = None;
            return;
        }
        let waited = ctx.input(|i| i.time) - t;
        if waited >= AUTO_DELAY {
            self.crayonnes.last_stroke = None;
            self.crayonnes_move(Move::Next);
        } else {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(AUTO_DELAY - waited + 0.02));
        }
    }

    /// La fenêtre « Créer les plans ».
    fn crayonnes_dialog(&mut self, ctx: &egui::Context) {
        if !self.crayonnes.dialog {
            return;
        }
        let n = self.crayonnes_picks().len();
        let mut opts = self.crayonnes.opts;
        let (mut create, mut cancel) = (false, false);
        egui::Window::new("Créer les plans").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.label(format!("{n} case{} choisie{} : un plan chacune, dans l'ordre de la planche.", if n > 1 { "s" } else { "" }, if n > 1 { "s" } else { "" }));
            ui.add_space(6.0);
            ui.radio_value(&mut opts.placement, Placement::AfterCurrent, "Après le plan courant");
            ui.radio_value(&mut opts.placement, Placement::NewSequence, "Dans une nouvelle séquence");
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Durée de chaque plan");
                ui.add(egui::DragValue::new(&mut opts.seconds).range(0.1..=600.0).speed(0.1).fixed_decimals(1).suffix(" s"));
            });
            ui.add_space(4.0);
            ui.label(RichText::new("Le dessin de la case va sur un calque « Crayonné » du plan, mis à l'échelle du cadre ; sa légende devient le texte d'action. Les cases restent sur la planche, cochées. Un seul Ctrl+Z défait tout.").small().weak());
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(n > 0, egui::Button::new("Créer")).clicked() {
                    create = true;
                }
                if ui.button("Annuler").clicked() {
                    cancel = true;
                }
            });
        });
        self.crayonnes.opts = opts;
        if create || cancel {
            self.crayonnes.dialog = false;
        }
        if create {
            self.crayonnes_create_plans(opts);
        }
    }
}
