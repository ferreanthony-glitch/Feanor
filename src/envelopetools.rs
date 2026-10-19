//! Enveloppe de Bézier (Anthony, 2026-10-18 : « maintenir Ctrl permet de saisir une poignée de
//! Bézier des coins pour transformer la forme » ; choix B, « comme l'outil Perspective »).
//!
//! Depuis la refonte de la boîte de sélection (2026-10-18, « singer le système de Storyboard Pro »,
//! `selbox.rs`), l'enveloppe n'est plus ce que fait Ctrl + un coin après une sélection : c'est un
//! mode de l'outil **Déformer** (Enveloppe | Perspective, choisi dans Réglages ou par les actions
//! `deformer.enveloppe` et `deformer.perspective`). Un clic sur le dessin ouvre l'enveloppe : ses
//! coins, et deux poignées par coin (rondes) qui courbent les côtés ; l'intérieur la déplace.
//! Chaque lâcher refait la déformation depuis le dessin d'origine (pas de perte d'un geste à
//! l'autre) et compte pour une étape d'annulation. Entrée, Échap, un clic ailleurs, un autre outil
//! ou une autre image terminent. Sur un calque vectoriel avec des éléments choisis, l'enveloppe ne
//! déforme qu'eux (Anthony, 2026-10-18 : « l'enveloppe doit aussi pouvoir ne déformer que la
//! sélection, comme l'outil Perspective le fait en vectoriel ») ; sur une zone bitmap choisie, que
//! ses pixels, bords adoucis compris ; sans sélection, tout le dessin. Comportement exact de
//! Storyboard Pro à vérifier. Géométrie dans `envelope.rs`.

use eframe::egui::{self, vec2, Color32, Key, Pos2, Rect, Shape, Stroke, StrokeKind};

use crate::app::App;
use crate::document::CelId;
use crate::envelope::{self, Envelope};
use crate::history::Edit;
use crate::raster::Raster;
use crate::vector::VectorDrawing;

/// Rayon de prise des coins et des poignées, en pixels d'écran.
const GRAB: f32 = 9.0;
const ACCENT: Color32 = crate::theme::POIGNEE_DEFORMER;

/// Mode de l'outil Déformer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DeformMode {
    /// Enveloppe de Bézier : coins et poignées qui courbent les côtés.
    Envelope,
    /// Perspective : les quatre coins.
    #[default]
    Perspective,
}

enum Original {
    Bitmap(Raster),
    /// Le dessin, et les éléments déformés (tous si aucun).
    Vector(VectorDrawing, Option<Vec<usize>>),
    /// Le dessin bitmap et la zone choisie, seule déformée.
    Masked(Raster, crate::pixsel::PixelMask),
}

impl Original {
    /// Les deux feuilles de l'aperçu d'un dessin bitmap : ce qui reste, et ce qui se déforme. Un
    /// dessin vectoriel n'en a pas : sa feuille flottante (`flottante.rs`), faite des pixels en
    /// cache des traits, est posée par un maillage que dessine la carte graphique (`warp.rs`).
    fn split(&self, src: Rect) -> (Raster, Raster) {
        match self {
            Original::Bitmap(orig) => envelope::split_rect(orig, src),
            Original::Masked(orig, mask) => envelope::split_masked(orig, mask),
            Original::Vector(..) => (Raster::new(1, 1), Raster::new(1, 1)),
        }
    }

    /// Le résultat exact d'un dessin bitmap déformé par `env`.
    fn exact(&self, src: Rect, env: &Envelope) -> Option<Raster> {
        match self {
            Original::Bitmap(orig) => Some(envelope::warp_raster(orig, src, env)),
            Original::Masked(orig, mask) => Some(envelope::warp_raster_masked(orig, mask, env)),
            Original::Vector(..) => None,
        }
    }
}

/// Ce que l'on tire : un coin, une poignée, ou toute l'enveloppe.
#[derive(Clone, Copy)]
pub(crate) enum Grip {
    Corner(usize),
    Handle(usize),
    All,
}

pub(crate) struct EnvelopeEdit {
    cel: CelId,
    src: Rect,
    env: Envelope,
    original: Original,
    drag: Option<(Grip, Pos2, Envelope)>,
    /// L'aperçu pendant le geste : ce qui reste, ce qui se déforme (`Original::split`).
    base: Raster,
    sheet: Raster,
    /// La zone de la feuille que l'aperçu a couverte (à rafraîchir avec la suivante).
    shown: Rect,
    /// L'image exacte posée par le dernier lâcher de la séance (bitmap) : l'état d'avant le geste
    /// suivant, pour son annulation.
    committed: crate::envelope::Committed,
}

impl EnvelopeEdit {
    /// Un coin, une poignée ou l'enveloppe est tiré sur le dessin `cel` : son image est l'aperçu.
    pub(crate) fn busy_on(&self, cel: CelId) -> bool {
        self.cel == cel && self.drag.is_some()
    }

    fn new(cel: CelId, src: Rect, original: Original) -> EnvelopeEdit {
        let env = Envelope::identity(src);
        let (base, sheet) = original.split(src);
        EnvelopeEdit { cel, src, env, original, drag: None, base, sheet, shown: src, committed: Default::default() }
    }

    /// L'image de l'aperçu : la feuille déformée par le maillage de l'enveloppe, posée sur le reste.
    fn preview(&self) -> Raster {
        let mut img = self.base.clone();
        img.composite_over(&envelope::warp_raster_fast(&self.sheet, self.src, |p| self.env.map(self.src, p)));
        img
    }
}

/// Boîte de l'enveloppe (ses courbes restent dans l'enveloppe convexe de leurs points de contrôle).
fn env_box(env: &Envelope) -> Rect {
    let mut pts: Vec<Pos2> = env.corners.to_vec();
    pts.extend_from_slice(&env.handles);
    Rect::from_points(&pts)
}

impl App {
    /// Le tour de l'enveloppe à l'écran (côtés échantillonnés).
    fn envelope_outline(&self, env: &Envelope) -> Vec<Pos2> {
        (0..4).flat_map(|k| (0..16).map(move |i| (k, i as f32 / 16.0))).map(|(k, t)| self.layer_to_screen(bezier_at(env, k, t))).collect()
    }

    pub(crate) fn envelope_grip(&self, screen: Pos2) -> Option<Grip> {
        let e = self.envelope.as_ref()?;
        if let Some(i) = (0..8).find(|&i| self.layer_to_screen(e.env.handles[i]).distance(screen) <= GRAB) {
            return Some(Grip::Handle(i));
        }
        if let Some(k) = (0..4).find(|&k| self.layer_to_screen(e.env.corners[k]).distance(screen) <= GRAB) {
            return Some(Grip::Corner(k));
        }
        crate::vector::point_in_polygon(screen, &self.envelope_outline(&e.env)).then_some(Grip::All)
    }

    /// Appui avec l'outil Déformer en mode Enveloppe : sur un coin, une poignée ou l'intérieur de
    /// l'enveloppe ouverte, on la tire ; ailleurs elle se termine et une nouvelle s'ouvre sur ce
    /// qui est à déformer (comme la perspective) : les éléments choisis, la zone bitmap choisie,
    /// sinon tout le dessin du calque.
    pub(crate) fn envelope_press(&mut self, p: Pos2, screen: Pos2) {
        self.playing = false;
        if let Some(grip) = self.envelope_grip(screen) {
            if let Some(e) = &mut self.envelope {
                e.drag = Some((grip, p, e.env));
            }
            return;
        }
        self.envelope = None;
        self.end_stroke();
        if !self.layer_editable() {
            return;
        }
        let here = self.cel_here();
        // La zone bitmap choisie, ou les traits choisis : elle seule ; sinon tout le dessin.
        if self.bsel.as_ref().is_some_and(|s| Some(s.cel) == here) {
            self.envelope_on_zone();
            return;
        }
        let chosen = self.vsel.as_ref().filter(|s| Some(s.cel) == here && !s.items.is_empty()).map(|s| s.items.clone());
        let Some(cel) = here else {
            self.status = "Enveloppe : rien à déformer sur cette image.".into();
            return;
        };
        let Some(c) = self.doc.cels.get(&cel) else { return };
        let src = match &chosen {
            Some(_) => match self.selection_bounds() {
                Some(b) => b,
                None => return,
            },
            None => self.content_box(),
        };
        let n = chosen.as_ref().map(|i| i.len());
        let original = match &c.vector {
            Some(d) => Original::Vector(d.clone(), chosen),
            None => Original::Bitmap(c.raster.clone()),
        };
        self.envelope = Some(EnvelopeEdit::new(cel, src, original));
        let what = match n {
            Some(n) => format!("Enveloppe sur la sélection ({n} élément{})", if n > 1 { "s" } else { "" }),
            None => "Enveloppe".to_string(),
        };
        self.status = format!(
            "{what} : tire les coins, et leurs poignées rondes pour courber les côtés ; l'intérieur pour tout déplacer. \
             Entrée, Échap ou clic ailleurs : terminer ; Ctrl+Z annule chaque geste."
        );
    }

    /// L'enveloppe sur la zone bitmap choisie. Une zone déjà déplacée est d'abord posée là où on la
    /// voit.
    fn envelope_on_zone(&mut self) {
        let Some((cel, lifted, mask0)) = self.bsel.as_ref().map(|s| (s.cel, s.lifted.is_some(), s.mask.clone())) else { return };
        let mask = if lifted {
            let outline = self.bitmap_selection_outline();
            self.drop_bitmap_selection();
            crate::pixsel::PixelMask::from_polygons(&outline, self.doc.width, self.doc.height)
        } else {
            Some(mask0)
        };
        self.bsel = None;
        self.bdrag = None;
        let (Some(mask), Some(c)) = (mask, self.doc.cels.get(&cel)) else { return };
        let src = mask.bounds();
        self.envelope = Some(EnvelopeEdit::new(cel, src, Original::Masked(c.raster.clone(), mask)));
        self.status = "Enveloppe sur la zone choisie : tire les coins, et leurs poignées rondes pour courber les côtés ; \
                       l'intérieur pour tout déplacer. Entrée, Échap ou clic ailleurs : terminer ; Ctrl+Z annule chaque geste."
            .into();
    }

    /// Appui avec l'outil Déformer : l'enveloppe ou la perspective, selon le mode.
    pub(crate) fn deform_press(&mut self, p: Pos2, screen: Pos2) {
        match self.deform_mode {
            DeformMode::Envelope => {
                self.persp = None;
                self.envelope_press(p, screen);
            }
            DeformMode::Perspective => {
                self.envelope = None;
                // La perspective ne déforme pas une zone bitmap : l'enveloppe, oui.
                if self.bsel.is_some() {
                    self.status = "Perspective : une zone choisie se déforme avec l'enveloppe (mode Enveloppe, dans Réglages).".into();
                    return;
                }
                self.persp_press(p, screen);
            }
        }
    }

    /// Change le mode de l'outil Déformer : la déformation ouverte se termine.
    pub(crate) fn set_deform_mode(&mut self, mode: DeformMode) {
        if self.deform_mode != mode {
            self.deform_mode = mode;
            self.envelope = None;
            self.persp = None;
        }
    }

    /// Curseur de l'outil Déformer.
    pub(crate) fn deform_cursor(&self, screen: Pos2) -> egui::CursorIcon {
        match self.deform_mode {
            DeformMode::Perspective => self.persp_cursor(screen),
            DeformMode::Envelope => self.envelope_cursor(screen).unwrap_or(egui::CursorIcon::Crosshair),
        }
    }

    /// « Remettre droit » : l'enveloppe revient à son rectangle (une étape d'annulation).
    fn envelope_reset(&mut self) {
        let Some(e) = &mut self.envelope else { return };
        e.drag = Some((Grip::All, Pos2::ZERO, e.env));
        e.env = Envelope::identity(e.src);
        self.envelope_release();
    }

    /// Réglages de l'outil Déformer (panneau Réglages) : le mode, puis ses commandes.
    pub(crate) fn deform_options_ui(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::choix_a;
        use egui_phosphor::regular as ph;
        let mut mode = self.deform_mode;
        ui.horizontal(|ui| {
            let (ke, kp) = (self.raccourci("deformer.enveloppe"), self.raccourci("deformer.perspective"));
            choix_a(ui, &mut mode, DeformMode::Envelope, ph::BEZIER_CURVE, "Enveloppe", "Courber le dessin : coins et poignées rondes qui courbent les côtés.", &ke, "deformer.enveloppe");
            choix_a(ui, &mut mode, DeformMode::Perspective, ph::PERSPECTIVE, "Perspective", "Mettre en perspective : tirer les quatre coins.", &kp, "deformer.perspective");
        });
        self.set_deform_mode(mode);
        match self.deform_mode {
            DeformMode::Perspective => self.persp_options_ui(ui),
            DeformMode::Envelope => {
                let editing = self.envelope.is_some();
                ui.horizontal(|ui| {
                    if crate::bulles::icone_si(ui, editing, ph::ARROW_COUNTER_CLOCKWISE, "Remettre droit", "Les coins et les poignées reviennent à leur place.", "").clicked() {
                        self.envelope_reset();
                    }
                    if crate::bulles::icone_si(ui, editing, ph::CHECK, "Terminer", "Garder la déformation et quitter (Entrée).", "").clicked() {
                        self.envelope = None;
                    }
                });
                ui.label(
                    egui::RichText::new(
                        "Clic sur le dessin : son enveloppe apparaît (sur un calque vectoriel, la sélection s'il y \
                         en a une ; sur un calque bitmap, la zone choisie). Tire un coin ou une poignée ronde pour \
                         courber les côtés, l'intérieur pour déplacer. Ctrl+Z annule chaque geste.",
                    )
                    .small()
                    .weak(),
                );
            }
        }
    }

    /// Mouvement du pointeur ; vrai si un coin, une poignée ou l'enveloppe est tiré. Le dessin
    /// suit : ses pixels (ceux de l'original, ou des caches des traits choisis) sont déformés par le
    /// maillage de l'enveloppe à chaque image, jamais rejoués.
    pub(crate) fn envelope_drag(&mut self, p: Pos2) -> bool {
        let Some(e) = &mut self.envelope else { return false };
        let Some((grip, start, env0)) = e.drag else { return false };
        let d = p - start;
        let mut env = env0;
        match grip {
            Grip::Corner(k) => env.move_corner(k, d),
            Grip::Handle(i) => env.handles[i] = env0.handles[i] + d,
            Grip::All => env.translate(d),
        }
        if env == e.env {
            return true;
        }
        e.env = env;
        if matches!(e.original, Original::Vector(..)) {
            self.envelope_float();
        } else {
            self.envelope_preview();
        }
        true
    }

    /// Dessin vectoriel : la feuille flottante des éléments déformés est posée par le maillage de
    /// l'enveloppe du moment. Rien n'est refait, rien n'est renvoyé à la carte graphique : seul le
    /// maillage change (`warp.rs`).
    fn envelope_float(&mut self) {
        let Some(e) = self.envelope.take() else { return };
        if let Original::Vector(drawing, items) = &e.original {
            self.warp_float(e.cel, drawing, items.as_deref(), e.src, 8, |p| e.env.map(e.src, p));
        }
        self.envelope = Some(e);
    }

    /// Montre le dessin bitmap déformé par l'enveloppe du moment (depuis l'original).
    fn envelope_preview(&mut self) {
        let Some(e) = &mut self.envelope else { return };
        let img = e.preview();
        let region = e.shown.union(env_box(&e.env)).union(e.src).expand(2.0);
        e.shown = env_box(&e.env).union(e.src);
        let cel = e.cel;
        let Some(c) = self.doc.cels.get_mut(&cel) else { return };
        let tiles = crate::vector::tiles_in(&c.raster, region);
        c.raster = img;
        c.version += 1;
        self.renderer.invalidate_tiles(&tiles);
    }

    /// Lâcher : la déformation est appliquée depuis le dessin d'origine (une étape d'annulation).
    /// En vectoriel, l'aperçu reste affiché et le rendu exact des traits déformés arrive en
    /// arrière-plan (`rejeu.rs`).
    pub(crate) fn envelope_release(&mut self) {
        let Some(e) = &mut self.envelope else { return };
        let Some((_, _, env0)) = e.drag.take() else { return };
        if env0 == e.env {
            return;
        }
        let (cel, src, env) = (e.cel, e.src, e.env);
        match &e.original {
            Original::Bitmap(orig) | Original::Masked(orig, _) => {
                let Some(new) = e.original.exact(src, &env) else { return };
                let Some(c) = self.doc.cels.get_mut(&cel) else { return };
                // L'étape d'annulation : les tuiles de l'original que la déformation a changées
                // (l'image affichée est l'aperçu rapide : remplacée par le résultat exact).
                // (« Avant » : l'image d'avant CE geste, résultat exact du précédent s'il y en a eu un
                // dans la séance, sinon l'original ; sinon Ctrl+Z sauterait le geste d'avant.)
                let Some((before, changed)) = e.committed.step(orig, &c.raster, &new) else { return };
                c.raster = new;
                c.version += 1;
                self.renderer.invalidate_tiles(&changed);
                self.history.push(Edit::Tiles { cel, tiles: before });
                self.modified = true;
            }
            Original::Vector(drawing, items) => {
                let new = envelope::warp_drawing(drawing, items.as_deref(), src, &env);
                let Some(d) = self.drawing_mut(cel) else { return };
                let before = std::mem::replace(d, new);
                self.rejeu_cel(cel);
                self.push_vector(cel, before, None);
                // La feuille déformée reste affichée jusqu'à l'arrivée du rendu exact.
                self.envelope_float();
                self.warp_settle(cel);
            }
        }
    }

    /// Entrée ou Échap terminent l'enveloppe ; vrai si la touche a servi.
    pub(crate) fn envelope_key(&mut self, key: Key) -> bool {
        if self.envelope.is_none() || !matches!(key, Key::Enter | Key::Escape) {
            return false;
        }
        self.envelope = None;
        self.status = "Enveloppe terminée.".into();
        true
    }

    /// L'enveloppe suit le dessin : changer d'image, de calque ou d'outil la termine.
    pub(crate) fn check_envelope(&mut self) {
        let tool_ok = self.tool == crate::app::Tool::Perspective && self.deform_mode == DeformMode::Envelope;
        if self.envelope.as_ref().is_some_and(|e| Some(e.cel) != self.cel_here() || !tool_ok) {
            self.envelope = None;
        }
    }

    /// Côtés, grille courbée, poignées et coins de l'enveloppe.
    pub(crate) fn paint_envelope_overlay(&self, painter: &egui::Painter) {
        let Some(e) = &self.envelope else { return };
        let to_s = |p: Pos2| self.layer_to_screen(p);
        let grid = Stroke::new(1.0_f32, ACCENT.gamma_multiply(0.45));
        for k in 1..4 {
            let t = k as f32 / 4.0;
            painter.add(Shape::line((0..=24).map(|j| to_s(e.env.point(t, j as f32 / 24.0))).collect(), grid));
            painter.add(Shape::line((0..=24).map(|j| to_s(e.env.point(j as f32 / 24.0, t))).collect(), grid));
        }
        let outline = self.envelope_outline(&e.env);
        painter.add(Shape::closed_line(outline.clone(), Stroke::new(2.5_f32, Color32::from_black_alpha(120))));
        painter.add(Shape::closed_line(outline, Stroke::new(1.0_f32, ACCENT)));
        // Chaque poignée reliée à son coin.
        for k in 0..4 {
            let (a, b) = (to_s(e.env.corners[k]), to_s(e.env.corners[(k + 1) % 4]));
            let (h0, h1) = (to_s(e.env.handles[2 * k]), to_s(e.env.handles[2 * k + 1]));
            painter.line_segment([a, h0], Stroke::new(1.0_f32, ACCENT));
            painter.line_segment([b, h1], Stroke::new(1.0_f32, ACCENT));
            for h in [h0, h1] {
                painter.circle(h, 4.5, Color32::WHITE, Stroke::new(1.5_f32, ACCENT));
            }
        }
        for c in e.env.corners.map(to_s) {
            painter.rect(Rect::from_center_size(c, vec2(9.0, 9.0)), 1.0, Color32::WHITE, Stroke::new(1.5_f32, ACCENT), StrokeKind::Middle);
        }
    }

    /// Curseur au-dessus de l'enveloppe (aucun : elle n'est pas ouverte).
    pub(crate) fn envelope_cursor(&self, screen: Pos2) -> Option<egui::CursorIcon> {
        let e = self.envelope.as_ref()?;
        Some(match (e.drag.is_some(), self.envelope_grip(screen)) {
            (true, _) => egui::CursorIcon::Grabbing,
            (false, Some(Grip::Corner(_) | Grip::Handle(_))) => egui::CursorIcon::Grab,
            (false, Some(Grip::All)) => egui::CursorIcon::Move,
            (false, None) => egui::CursorIcon::Default,
        })
    }
}

/// Le point `t` du côté `k` de l'enveloppe.
fn bezier_at(env: &Envelope, k: usize, t: f32) -> Pos2 {
    let p = env.edge(k);
    let s = 1.0 - t;
    (p[0].to_vec2() * (s * s * s) + p[1].to_vec2() * (3.0 * s * s * t) + p[2].to_vec2() * (3.0 * s * t * t) + p[3].to_vec2() * (t * t * t)).to_pos2()
}
