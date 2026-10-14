use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui::{self, vec2, Color32, Event, Key, Pos2, Rect, Vec2};

use crate::brush::{BrushSettings, StrokeMode, StrokeState};
use crate::document::{fresh_layer_id, CelId, Document, Layer, LayerKind, Shot};
use crate::fill::{self, FillSettings};
use crate::history::{Edit, History, Nav};
use crate::io;
use crate::keymap::{self, Keymap};
use crate::raster::{premultiply, unpremultiply, Tile, TILE};
use crate::render::{self, Plan, PlanItem, Renderer};
use crate::storyboard::MontageDrag;
use crate::thumbs::Thumbs;
use crate::vector::{Element, VectorDrawing};
use crate::view::View;
use crate::vtools::{VDrag, VSelection, VectorErase, VectorStroke};

const ONION_BEFORE: [u8; 3] = [225, 45, 45];
const ONION_AFTER: [u8; 3] = [35, 160, 70];

/// Storyboard : on monte les plans. Animation : on est entré dans un plan.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Storyboard,
    Animation,
}

/// Ce qu'affiche le centre de l'écran en mode storyboard.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BoardView {
    Scene,
    Panels,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Brush,
    Pencil,
    Eraser,
    Fill,
    Shape,
    Select,
    Picker,
    Pan,
}

impl Tool {
    pub const ALL: [Tool; 8] = [
        Tool::Brush,
        Tool::Pencil,
        Tool::Eraser,
        Tool::Fill,
        Tool::Shape,
        Tool::Select,
        Tool::Picker,
        Tool::Pan,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tool::Brush => "Pinceau",
            Tool::Pencil => "Crayon",
            Tool::Eraser => "Gomme",
            Tool::Fill => "Remplir",
            Tool::Shape => "Formes",
            Tool::Select => "Sélection",
            Tool::Picker => "Pipette",
            Tool::Pan => "Main",
        }
    }

    /// Action qui choisit cet outil (son raccourci est dans le fichier de raccourcis).
    pub fn action(self) -> &'static str {
        match self {
            Tool::Brush => "outil.pinceau",
            Tool::Pencil => "outil.crayon",
            Tool::Eraser => "outil.gomme",
            Tool::Fill => "outil.remplissage",
            Tool::Shape => "outil.formes",
            Tool::Select => "outil.selection",
            Tool::Picker => "outil.pipette",
            Tool::Pan => "outil.main",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind {
    Line,
    Rect,
    Ellipse,
}

pub struct ShapeSettings {
    pub kind: ShapeKind,
    pub filled: bool,
    /// Épaisseur du trait, en pixels.
    pub width: f32,
}

pub struct OnionSettings {
    pub enabled: bool,
    pub before: usize,
    pub after: usize,
    pub opacity: f32,
}

/// Action qui attend une confirmation parce que le projet n'est pas enregistré.
#[derive(Clone, Copy)]
pub enum Pending {
    New,
    Open,
    Quit,
}

pub struct NewDocDialog {
    pub width: usize,
    pub height: usize,
    pub fps: u32,
}

pub struct App {
    pub(crate) ctx: egui::Context,
    pub(crate) doc: Document,
    pub(crate) mode: Mode,
    pub(crate) board_view: BoardView,
    pub(crate) cur_shot: usize,
    pub(crate) cur_layer: usize,
    pub(crate) cur_frame: usize,
    pub(crate) tool: Tool,
    pub(crate) brush: BrushSettings,
    pub(crate) pencil: BrushSettings,
    pub(crate) eraser: BrushSettings,
    pub(crate) fill: FillSettings,
    pub(crate) shape: ShapeSettings,
    pub(crate) vector_erase_whole: bool,
    pub(crate) color: [u8; 3],
    pub(crate) swatches: Vec<[u8; 3]>,
    pub(crate) view: View,
    pub(crate) fit_pending: bool,
    pub(crate) canvas_rect: Rect,
    pub(crate) ppp: f32,
    pub(crate) onion: OnionSettings,
    pub(crate) default_exposure: usize,
    pub(crate) playing: bool,
    play_montage: bool,
    pub(crate) looping: bool,
    play_accum: f64,
    last_time: Option<f64>,
    pub(crate) history: History,
    pub(crate) stroke: Option<StrokeState>,
    pub(crate) vstroke: Option<VectorStroke>,
    pub(crate) verase: Option<VectorErase>,
    pub(crate) vsel: Option<VSelection>,
    pub(crate) vdrag: Option<VDrag>,
    pub(crate) shape_drag: Option<(Pos2, Pos2)>,
    pub(crate) clipboard: Vec<Element>,
    pub(crate) renderer: Renderer,
    pub(crate) thumbs: Thumbs,
    pub(crate) montage_zoom: f32,
    pub(crate) montage_drag: Option<MontageDrag>,
    /// Plan tel qu'il était au début du réglage de sa durée : chaque valeur intermédiaire
    /// repart de lui, rien n'est perdu avant la fin du geste.
    pub(crate) length_origin: Option<Shot>,
    /// Un champ de durée est en cours d'utilisation pendant cette image d'écran.
    pub(crate) length_field_active: bool,
    pub(crate) pen_pressure: f32,
    pub(crate) pen_touch: bool,
    pub(crate) picking: bool,
    pub(crate) path: Option<PathBuf>,
    pub(crate) modified: bool,
    pub(crate) status: String,
    pub(crate) new_dialog: Option<NewDocDialog>,
    pub(crate) pending: Option<Pending>,
    pub(crate) show_help: bool,
    allow_close: bool,
    pub(crate) export_transparent: bool,
    pub(crate) follow_frame: bool,
    pub(crate) hover_canvas: Option<Pos2>,
    /// Touches retirées des événements avant qu'egui ne s'en serve (Tab), traitées comme raccourcis.
    pending_keys: Vec<(Key, Option<Key>, egui::Modifiers)>,
    pub(crate) keymap: Keymap,
    title: String,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        Self {
            ctx: cc.egui_ctx.clone(),
            doc: Document::new(1920, 1080, 24),
            mode: Mode::Storyboard,
            board_view: BoardView::Scene,
            cur_shot: 0,
            cur_layer: 0,
            cur_frame: 0,
            tool: Tool::Brush,
            brush: BrushSettings::brush(),
            pencil: BrushSettings::pencil(),
            eraser: BrushSettings::eraser(),
            fill: FillSettings::default(),
            shape: ShapeSettings {
                kind: ShapeKind::Rect,
                filled: false,
                width: 4.0,
            },
            vector_erase_whole: false,
            color: [24, 24, 28],
            swatches: vec![
                [24, 24, 28],
                [255, 255, 255],
                [128, 128, 128],
                [205, 45, 45],
                [240, 150, 40],
                [245, 215, 60],
                [60, 160, 80],
                [50, 110, 200],
                [130, 70, 170],
                [235, 190, 160],
            ],
            view: View::default(),
            fit_pending: true,
            canvas_rect: Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)),
            ppp: 1.0,
            onion: OnionSettings {
                enabled: true,
                before: 2,
                after: 1,
                opacity: 0.45,
            },
            default_exposure: 1,
            playing: false,
            play_montage: false,
            looping: true,
            play_accum: 0.0,
            last_time: None,
            history: History::default(),
            stroke: None,
            vstroke: None,
            verase: None,
            vsel: None,
            vdrag: None,
            shape_drag: None,
            clipboard: Vec::new(),
            renderer: Renderer::new(),
            thumbs: Thumbs::default(),
            montage_zoom: 4.0,
            montage_drag: None,
            length_origin: None,
            length_field_active: false,
            pen_pressure: 1.0,
            pen_touch: false,
            picking: false,
            path: None,
            modified: false,
            status: "Storyboard : dessine le premier plan, Inser pour en ajouter, double-clic sur un plan pour l'animer (F1 : raccourcis).".into(),
            new_dialog: None,
            pending: None,
            show_help: false,
            allow_close: false,
            export_transparent: false,
            follow_frame: false,
            hover_canvas: None,
            pending_keys: Vec::new(),
            keymap: Keymap::load(),
            title: String::new(),
        }
    }

    // ------------------------------------------------------------------ accès

    pub(crate) fn shot(&self) -> &Shot {
        &self.doc.shots[self.cur_shot]
    }

    pub(crate) fn shot_mut(&mut self) -> &mut Shot {
        &mut self.doc.shots[self.cur_shot]
    }

    pub(crate) fn layers(&self) -> &[Layer] {
        &self.doc.shots[self.cur_shot].layers
    }

    pub(crate) fn layers_mut(&mut self) -> &mut Vec<Layer> {
        &mut self.doc.shots[self.cur_shot].layers
    }

    pub(crate) fn layer(&self) -> &Layer {
        &self.layers()[self.cur_layer]
    }

    pub(crate) fn cel_here(&self) -> Option<CelId> {
        self.layer().cel_at(self.cur_frame)
    }

    pub(crate) fn frame_count(&self) -> usize {
        self.shot().frame_count()
    }

    /// Image courante dans le montage complet.
    pub(crate) fn global_frame(&self) -> usize {
        self.doc.shot_start(self.cur_shot) + self.cur_frame.min(self.frame_count() - 1)
    }

    pub(crate) fn canvas_size(&self) -> Vec2 {
        vec2(self.doc.width as f32, self.doc.height as f32)
    }

    pub(crate) fn modal_open(&self) -> bool {
        self.new_dialog.is_some() || self.pending.is_some()
    }

    pub(crate) fn current_brush(&self) -> Option<&BrushSettings> {
        match self.tool {
            Tool::Brush => Some(&self.brush),
            Tool::Pencil => Some(&self.pencil),
            Tool::Eraser => Some(&self.eraser),
            _ => None,
        }
    }

    fn current_brush_mut(&mut self) -> Option<&mut BrushSettings> {
        match self.tool {
            Tool::Brush => Some(&mut self.brush),
            Tool::Pencil => Some(&mut self.pencil),
            Tool::Eraser => Some(&mut self.eraser),
            _ => None,
        }
    }

    /// Garde plan et calque courants dans les bornes (après une annulation, un chargement…).
    fn clamp_nav(&mut self) {
        self.cur_shot = self.cur_shot.min(self.doc.shots.len() - 1);
        self.cur_layer = self.cur_layer.min(self.layers().len() - 1);
    }

    // ------------------------------------------------------------------ composition

    /// Ce qu'il faut empiler pour afficher l'image `frame` du plan courant.
    pub(crate) fn build_plan(&self, frame: usize, onion: bool, paper: bool) -> Plan {
        let mut items = Vec::new();
        for (li, layer) in self.layers().iter().enumerate() {
            if !layer.visible {
                continue;
            }
            if onion && self.onion.enabled && !self.playing && li == self.cur_layer {
                let (before, after) = onion_neighbours(layer, frame, self.onion.before, self.onion.after);
                // Du plus lointain au plus proche, pour que le plus proche soit au-dessus.
                for (k, &cel) in before.iter().enumerate().rev() {
                    items.push(PlanItem {
                        cel,
                        opacity: fade(self.onion.opacity, k, self.onion.before),
                        tint: Some(ONION_BEFORE),
                    });
                }
                for (k, &cel) in after.iter().enumerate().rev() {
                    items.push(PlanItem {
                        cel,
                        opacity: fade(self.onion.opacity, k, self.onion.after),
                        tint: Some(ONION_AFTER),
                    });
                }
            }
            if let Some(cel) = layer.cel_at(frame) {
                items.push(PlanItem {
                    cel,
                    opacity: (layer.opacity.clamp(0.0, 1.0) * 255.0).round() as u8,
                    tint: None,
                });
            }
        }
        Plan {
            paper: paper.then_some(self.doc.paper),
            items,
        }
    }

    // ------------------------------------------------------------------ navigation et lecture

    pub(crate) fn goto_frame(&mut self, frame: usize) {
        self.end_stroke();
        self.playing = false;
        self.cur_frame = frame.min(self.frame_count());
        self.follow_frame = true;
    }

    /// Comme `goto_frame`, sans limite à la fin du plan (clic dans la timeline).
    pub(crate) fn set_frame_free(&mut self, frame: usize) {
        self.end_stroke();
        self.playing = false;
        self.cur_frame = frame.min(100_000);
    }

    /// Va à une image du montage complet.
    pub(crate) fn goto_global(&mut self, global: usize) {
        self.end_stroke();
        self.playing = false;
        let (shot, frame) = self.doc.locate(global);
        self.set_shot(shot);
        self.cur_frame = frame;
        self.follow_frame = true;
    }

    pub(crate) fn step_global(&mut self, delta: i64) {
        let total = self.doc.total_frames() as i64;
        let g = (self.global_frame() as i64 + delta).clamp(0, total - 1);
        self.goto_global(g as usize);
    }

    /// Change de plan courant (sans rien changer si c'est déjà lui).
    pub(crate) fn set_shot(&mut self, index: usize) {
        let index = index.min(self.doc.shots.len() - 1);
        if index == self.cur_shot {
            return;
        }
        self.end_stroke();
        self.cur_shot = index;
        self.cur_layer = self.cur_layer.min(self.layers().len() - 1);
        self.cur_frame = 0;
        self.vsel = None;
    }

    pub(crate) fn select_shot(&mut self, index: usize) {
        self.playing = false;
        self.set_shot(index);
        self.follow_frame = true;
    }

    /// Entre dans un plan pour l'animer.
    pub(crate) fn enter_animation(&mut self, index: usize) {
        self.select_shot(index);
        self.set_mode(Mode::Animation);
    }

    pub(crate) fn set_mode(&mut self, mode: Mode) {
        self.cancel_montage_drag();
        self.end_stroke();
        self.playing = false;
        self.mode = mode;
        if mode == Mode::Storyboard {
            // En storyboard, on reste dans les bornes du plan (la timeline permet d'aller au-delà).
            self.cur_frame = self.cur_frame.min(self.frame_count() - 1);
        }
        self.follow_frame = true;
        self.status = match mode {
            Mode::Storyboard => "Storyboard : montage des plans.".into(),
            Mode::Animation => format!("Animation du plan {} (Tab : retour au storyboard).", self.cur_shot + 1),
        };
    }

    pub(crate) fn select_layer(&mut self, layer: usize) {
        if layer < self.layers().len() && layer != self.cur_layer {
            self.end_stroke();
            self.cur_layer = layer;
            self.vsel = None;
        }
    }

    pub(crate) fn toggle_play(&mut self) {
        if self.playing {
            self.playing = false;
            return;
        }
        self.end_stroke();
        self.play_montage = self.mode == Mode::Storyboard;
        let at_end = if self.play_montage {
            self.global_frame() + 1 >= self.doc.total_frames()
        } else {
            self.cur_frame + 1 >= self.frame_count()
        };
        if at_end {
            if self.play_montage {
                self.set_shot(0);
            }
            self.cur_frame = 0;
        }
        self.playing = true;
        self.last_time = None;
        self.play_accum = 0.0;
    }

    fn tick_playback(&mut self, ctx: &egui::Context) {
        if !self.playing {
            return;
        }
        let now = ctx.input(|i| i.time);
        let last = self.last_time.replace(now).unwrap_or(now);
        self.play_accum += (now - last).min(0.25);
        let frame_duration = 1.0 / self.doc.fps.max(1) as f64;
        while self.play_accum >= frame_duration {
            self.play_accum -= frame_duration;
            if self.cur_frame + 1 < self.frame_count() {
                self.cur_frame += 1;
            } else if self.play_montage && self.cur_shot + 1 < self.doc.shots.len() {
                let next = self.cur_shot + 1;
                self.set_shot(next);
            } else if self.looping {
                if self.play_montage {
                    self.set_shot(0);
                }
                self.cur_frame = 0;
            } else {
                self.playing = false;
                break;
            }
            self.follow_frame = true;
        }
        ctx.request_repaint();
    }

    // ------------------------------------------------------------------ dessin

    /// Dessin de l'image courante, créé au besoin (avec l'état d'avant, pour l'annulation).
    /// En storyboard, un nouveau dessin est tenu jusqu'au prochain dessin ou à la fin du plan ;
    /// en animation, sur le nombre d'images choisi (« sur 1 », « sur 2 »…).
    pub(crate) fn ensure_cel(&mut self) -> (CelId, Option<Edit>) {
        if let Some(id) = self.cel_here() {
            return (id, None);
        }
        let (li, f) = (self.cur_layer, self.cur_frame);
        let snap = self.snapshot();
        let kind = self.layer().kind;
        let id = self.doc.new_cel(kind);
        let limit = self.frame_count();
        let end = if f >= limit {
            f + self.default_exposure.max(1)
        } else {
            let run_end = self.layer().empty_run_end(f, limit);
            match self.mode {
                Mode::Storyboard => run_end,
                Mode::Animation => run_end.min(f + self.default_exposure.max(1)),
            }
        };
        let layer = &mut self.layers_mut()[li];
        for k in f..end {
            layer.set(k, Some(id));
        }
        (id, Some(snap))
    }

    pub(crate) fn layer_editable(&mut self) -> bool {
        if self.layer().visible {
            return true;
        }
        self.status = "Ce calque est masqué : affiche-le pour dessiner dessus.".into();
        false
    }

    /// Début d'un trait de pinceau, de crayon ou de gomme, bitmap ou vectoriel selon le calque.
    pub(crate) fn begin_paint(&mut self, p: Pos2, pressure: f32, pen: bool) {
        self.end_stroke();
        self.playing = false;
        if !self.layer_editable() {
            return;
        }
        let erase = self.tool == Tool::Eraser;
        if erase && self.cel_here().is_none() {
            return;
        }
        match (self.layer().kind, erase) {
            (LayerKind::Vector, true) => self.begin_vector_erase(p, pen),
            (LayerKind::Vector, false) => self.begin_vector_stroke(p, pressure, pen),
            (LayerKind::Bitmap, _) => self.begin_stroke(p, pressure, pen),
        }
    }

    fn begin_stroke(&mut self, p: Pos2, pressure: f32, pen: bool) {
        let Some(settings) = self.current_brush().copied() else {
            return;
        };
        let (cel, prefix) = self.ensure_cel();
        let mode = if self.tool == Tool::Eraser {
            StrokeMode::Erase
        } else {
            StrokeMode::Paint(self.color)
        };
        let mut stroke = StrokeState::new(cel, mode, settings, prefix, pen);
        let mut dirty = Vec::new();
        if let Some(c) = self.doc.cels.get_mut(&cel) {
            stroke.add_point(&mut c.raster, p, pressure, &mut dirty);
        }
        self.renderer.invalidate_tiles(&dirty);
        self.stroke = Some(stroke);
        self.modified = true;
    }

    pub(crate) fn stroke_to(&mut self, p: Pos2, pressure: f32) {
        let Some(stroke) = self.stroke.as_mut() else { return };
        let Some(c) = self.doc.cels.get_mut(&stroke.cel) else { return };
        let mut dirty = Vec::new();
        stroke.add_point(&mut c.raster, p, pressure, &mut dirty);
        self.renderer.invalidate_tiles(&dirty);
    }

    /// Termine tout geste en cours : trait, gomme vectorielle, transformation, forme.
    pub(crate) fn end_stroke(&mut self) {
        if let Some(mut stroke) = self.stroke.take() {
            let cel = stroke.cel;
            let mut dirty = Vec::new();
            let edit = match self.doc.cels.get_mut(&cel) {
                Some(c) => stroke.finish(&mut c.raster, &mut dirty),
                None => None,
            };
            self.renderer.invalidate_tiles(&dirty);
            match self.vstroke.take() {
                Some(vs) => self.commit_vector_stroke(vs, std::mem::take(&mut stroke.path)),
                None => {
                    if let Some(c) = self.doc.cels.get_mut(&cel) {
                        c.version += 1;
                    }
                    if let Some(edit) = edit {
                        self.history.push(edit);
                    }
                }
            }
        }
        self.finish_vector_erase();
        if self.vdrag.is_some() {
            self.finish_vdrag(false);
        }
        self.shape_drag = None;
    }

    pub(crate) fn pick_color(&mut self, p: Pos2) {
        if p.x < 0.0 || p.y < 0.0 || p.x >= self.doc.width as f32 || p.y >= self.doc.height as f32 {
            return;
        }
        let plan = self.build_plan(self.cur_frame, false, true);
        let c = render::composite_pixel(&self.doc, &plan, p.x as usize, p.y as usize);
        let [r, g, b, _] = unpremultiply(c);
        self.color = [r, g, b];
    }

    pub(crate) fn fill_at(&mut self, p: Pos2) {
        if p.x < 0.0 || p.y < 0.0 || p.x >= self.doc.width as f32 || p.y >= self.doc.height as f32 {
            return;
        }
        self.end_stroke();
        self.playing = false;
        if !self.layer_editable() {
            return;
        }
        match self.layer().kind {
            LayerKind::Vector => self.fill_vector(p),
            LayerKind::Bitmap => self.fill_bitmap(p),
        }
    }

    /// Image sur laquelle le pot de peinture cherche ses contours.
    pub(crate) fn fill_source(&self) -> Vec<Color32> {
        let (w, h) = (self.doc.width, self.doc.height);
        if self.fill.sample_all {
            return render::composite_full(&self.doc, &self.build_plan(self.cur_frame, false, false));
        }
        match self.cel_here() {
            Some(id) => self.doc.cels[&id].raster.to_flat(),
            None => vec![Color32::TRANSPARENT; w * h],
        }
    }

    fn fill_bitmap(&mut self, p: Pos2) {
        let (w, h) = (self.doc.width, self.doc.height);
        let source = self.fill_source();
        let tolerance = (self.fill.tolerance * 255.0).round() as u8;
        let region = fill::flood(&source, w, h, p.x as usize, p.y as usize, tolerance);
        let grown = fill::dilate(&region, w, h, self.fill.expand);
        let (cel, prefix) = self.ensure_cel();
        let color = self.color;
        let Some(c) = self.doc.cels.get_mut(&cel) else { return };
        let tiles = fill::apply(&mut c.raster, &region, &grown, color);
        self.push_pixels(cel, prefix, tiles);
    }

    pub(crate) fn push_pixels(&mut self, cel: CelId, prefix: Option<Edit>, tiles: Vec<(usize, Option<Tile>)>) {
        if let Some(c) = self.doc.cels.get_mut(&cel) {
            c.version += 1;
        }
        let pixels = Edit::Tiles { cel, tiles };
        self.history.push(match prefix {
            Some(p) => Edit::Group(vec![p, pixels]),
            None => pixels,
        });
        self.modified = true;
        self.renderer.invalidate_all();
    }

    /// Enregistre l'annulation d'une modification vectorielle (`before` : le dessin d'avant).
    pub(crate) fn push_vector(&mut self, cel: CelId, before: VectorDrawing, prefix: Option<Edit>) {
        let undo = Edit::Vector { cel, drawing: before };
        self.history.push(match prefix {
            Some(p) => Edit::Group(vec![p, undo]),
            None => undo,
        });
        self.modified = true;
    }

    /// Refait l'image d'un dessin vectoriel et rafraîchit l'affichage.
    pub(crate) fn refresh_cel(&mut self, cel: CelId) {
        if let Some(c) = self.doc.cels.get_mut(&cel) {
            let tiles = c.rerender();
            self.renderer.invalidate_tiles(&tiles);
        }
    }

    pub(crate) fn resize_brush(&mut self, factor: f32) {
        if self.tool == Tool::Shape {
            self.shape.width = (self.shape.width * factor).clamp(0.5, 200.0);
        } else if let Some(b) = self.current_brush_mut() {
            b.size = (b.size * factor).clamp(0.5, 400.0);
        }
    }

    // ------------------------------------------------------------------ timeline et calques

    pub(crate) fn snapshot(&self) -> Edit {
        Edit::Structure {
            shots: self.doc.shots.clone(),
            nav: Nav {
                shot: self.cur_shot,
                layer: self.cur_layer,
            },
        }
    }

    pub(crate) fn commit_structure(&mut self, snap: Edit) {
        self.history.push(snap);
        self.modified = true;
        self.renderer.invalidate_all();
    }

    /// Insère un dessin vide (ou une copie du dessin courant) juste après l'exposition courante.
    pub(crate) fn new_image(&mut self, duplicate: bool) {
        self.end_stroke();
        self.playing = false;
        let (li, f) = (self.cur_layer, self.cur_frame);
        let current = self.cel_here();
        if duplicate && current.is_none() {
            self.status = "Rien à dupliquer sur cette image.".into();
            return;
        }
        let snap = self.snapshot();
        let kind = self.layer().kind;
        let id = match (duplicate, current) {
            (true, Some(src)) => self.doc.duplicate_cel(src),
            _ => self.doc.new_cel(kind),
        };
        let n = self.default_exposure.max(1);
        let layer = &mut self.layers_mut()[li];
        let at = if current.is_some() { layer.run_bounds(f).1 } else { f };
        layer.insert(at, Some(id), n);
        self.cur_frame = at;
        self.follow_frame = true;
        self.vsel = None;
        self.commit_structure(snap);
    }

    pub(crate) fn exposure_plus(&mut self) {
        self.end_stroke();
        let (li, f) = (self.cur_layer, self.cur_frame);
        let layer = self.layer();
        if f >= layer.frames.len() {
            return;
        }
        let (_, end) = layer.run_bounds(f);
        let value = layer.frames[f];
        let snap = self.snapshot();
        self.layers_mut()[li].insert(end, value, 1);
        self.commit_structure(snap);
    }

    pub(crate) fn exposure_minus(&mut self) {
        self.end_stroke();
        let (li, f) = (self.cur_layer, self.cur_frame);
        let layer = self.layer();
        if f >= layer.frames.len() {
            return;
        }
        let (start, end) = layer.run_bounds(f);
        if layer.frames[f].is_some() && end - start <= 1 {
            self.status = "Ce dessin n'est exposé qu'une fois : « Supprimer » pour l'enlever.".into();
            return;
        }
        let snap = self.snapshot();
        let layer = &mut self.layers_mut()[li];
        layer.frames.remove(end - 1);
        layer.trim();
        if self.cur_frame >= end - 1 && self.cur_frame > start {
            self.cur_frame = end - 2;
        }
        self.commit_structure(snap);
    }

    /// Retire le dessin courant et toutes ses expositions ; la suite du calque recule.
    pub(crate) fn delete_image(&mut self) {
        self.end_stroke();
        let (li, f) = (self.cur_layer, self.cur_frame);
        if f >= self.layer().frames.len() {
            return;
        }
        let snap = self.snapshot();
        let layer = &mut self.layers_mut()[li];
        let (start, end) = layer.run_bounds(f);
        layer.frames.drain(start..end);
        layer.trim();
        self.cur_frame = start;
        self.vsel = None;
        self.commit_structure(snap);
    }

    /// Vide le dessin courant sans toucher à la timeline.
    pub(crate) fn clear_image(&mut self) {
        self.end_stroke();
        let Some(id) = self.cel_here() else { return };
        let Some(c) = self.doc.cels.get_mut(&id) else { return };
        if let Some(d) = c.vector.as_mut() {
            if d.elements.is_empty() {
                return;
            }
            let before = std::mem::take(d);
            self.vsel = None;
            self.refresh_cel(id);
            self.push_vector(id, before, None);
            return;
        }
        let tiles: Vec<(usize, Option<Tile>)> = c
            .raster
            .tiles
            .iter_mut()
            .enumerate()
            .filter(|(_, t)| t.is_some())
            .map(|(i, t)| (i, t.take()))
            .collect();
        if !tiles.is_empty() {
            self.push_pixels(id, None, tiles);
        }
    }

    pub(crate) fn add_layer(&mut self, kind: LayerKind) {
        self.end_stroke();
        let snap = self.snapshot();
        let n = self.layers().len() + 1;
        let name = match kind {
            LayerKind::Vector => format!("Vecto {n}"),
            LayerKind::Bitmap => format!("Calque {n}"),
        };
        let at = self.cur_layer + 1;
        self.layers_mut().insert(at, Layer::new(name, kind));
        self.cur_layer = at;
        self.vsel = None;
        self.commit_structure(snap);
    }

    pub(crate) fn duplicate_layer(&mut self) {
        self.end_stroke();
        let snap = self.snapshot();
        let source = self.layer().clone();
        let mut copies: HashMap<CelId, CelId> = HashMap::new();
        let mut frames = Vec::with_capacity(source.frames.len());
        for f in &source.frames {
            frames.push(match f {
                Some(id) => Some(match copies.get(id) {
                    Some(&c) => c,
                    None => {
                        let c = self.doc.duplicate_cel(*id);
                        copies.insert(*id, c);
                        c
                    }
                }),
                None => None,
            });
        }
        let layer = Layer {
            id: fresh_layer_id(),
            name: format!("{} copie", source.name),
            frames,
            ..source
        };
        let at = self.cur_layer + 1;
        self.layers_mut().insert(at, layer);
        self.cur_layer = at;
        self.vsel = None;
        self.commit_structure(snap);
    }

    pub(crate) fn delete_layer(&mut self) {
        self.end_stroke();
        if self.layers().len() <= 1 {
            self.status = "Il faut garder au moins un calque.".into();
            return;
        }
        let snap = self.snapshot();
        let at = self.cur_layer;
        self.layers_mut().remove(at);
        self.cur_layer = at.min(self.layers().len() - 1);
        self.vsel = None;
        self.commit_structure(snap);
    }

    pub(crate) fn move_layer(&mut self, up: bool) {
        self.end_stroke();
        let i = self.cur_layer;
        let j = if up { i + 1 } else { i.wrapping_sub(1) };
        if j >= self.layers().len() {
            return;
        }
        let snap = self.snapshot();
        self.layers_mut().swap(i, j);
        self.cur_layer = j;
        self.commit_structure(snap);
    }

    pub(crate) fn undo(&mut self) {
        self.cancel_montage_drag();
        self.end_stroke();
        let mut nav = Nav {
            shot: self.cur_shot,
            layer: self.cur_layer,
        };
        if self.history.undo(&mut self.doc, &mut nav) {
            self.after_history(nav);
        } else {
            self.status = "Rien à annuler.".into();
        }
    }

    pub(crate) fn redo(&mut self) {
        self.cancel_montage_drag();
        self.end_stroke();
        let mut nav = Nav {
            shot: self.cur_shot,
            layer: self.cur_layer,
        };
        if self.history.redo(&mut self.doc, &mut nav) {
            self.after_history(nav);
        } else {
            self.status = "Rien à rétablir.".into();
        }
    }

    fn after_history(&mut self, nav: Nav) {
        self.cur_shot = nav.shot;
        self.cur_layer = nav.layer;
        self.clamp_nav();
        self.vsel = None;
        self.length_origin = None;
        self.renderer.invalidate_all();
        self.modified = true;
    }

    // ------------------------------------------------------------------ fichiers

    pub(crate) fn request(&mut self, action: Pending) {
        self.end_stroke();
        self.playing = false;
        if self.modified {
            self.pending = Some(action);
        } else {
            self.perform(action);
        }
    }

    pub(crate) fn perform(&mut self, action: Pending) {
        match action {
            Pending::New => {
                self.new_dialog = Some(NewDocDialog {
                    width: self.doc.width,
                    height: self.doc.height,
                    fps: self.doc.fps,
                })
            }
            Pending::Open => self.open_dialog(),
            Pending::Quit => {
                self.allow_close = true;
                self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    pub(crate) fn save(&mut self) -> bool {
        self.end_stroke();
        match self.path.clone() {
            Some(p) => self.save_to(p),
            None => self.save_as(),
        }
    }

    pub(crate) fn save_as(&mut self) -> bool {
        self.end_stroke();
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "animation.pelure".into());
        let Some(mut path) = rfd::FileDialog::new()
            .add_filter("Projet Pelure", &["pelure"])
            .set_file_name(name)
            .save_file()
        else {
            return false;
        };
        if path.extension().is_none() {
            path.set_extension("pelure");
        }
        self.save_to(path)
    }

    fn save_to(&mut self, path: PathBuf) -> bool {
        match io::save_project(&self.doc, &path) {
            Ok(()) => {
                self.status = format!("Enregistré : {}", path.display());
                self.path = Some(path);
                self.modified = false;
                true
            }
            Err(e) => {
                self.status = format!("Échec de l'enregistrement : {e}");
                false
            }
        }
    }

    fn open_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Projet Pelure", &["pelure"])
            .pick_file()
        else {
            return;
        };
        self.open_path(path);
    }

    pub(crate) fn open_path(&mut self, path: PathBuf) {
        match io::load_project(&path) {
            Ok(doc) => {
                self.load_doc(doc);
                self.status = format!("Ouvert : {}", path.display());
                self.path = Some(path);
            }
            Err(e) => self.status = format!("Impossible d'ouvrir le projet : {e}"),
        }
    }

    pub(crate) fn load_doc(&mut self, doc: Document) {
        self.doc = doc;
        self.mode = Mode::Storyboard;
        self.cur_shot = 0;
        self.cur_layer = self.layers().len() - 1;
        self.cur_frame = 0;
        self.stroke = None;
        self.vstroke = None;
        self.verase = None;
        self.vsel = None;
        self.vdrag = None;
        self.playing = false;
        self.history.clear();
        self.thumbs.clear();
        self.renderer.invalidate_all();
        self.fit_pending = true;
        self.modified = false;
        self.path = None;
    }

    fn base_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image".into())
    }

    fn pick_folder(title: &str) -> Option<PathBuf> {
        rfd::FileDialog::new().set_title(title).pick_folder()
    }

    /// Écrit en PNG les images `(plan, image)` données, numérotées à partir de 1.
    fn export_frames(&mut self, dir: &std::path::Path, frames: &[(usize, usize)]) {
        let base = self.base_name();
        let paper = (!self.export_transparent).then_some(self.doc.paper);
        for (n, &(shot, frame)) in frames.iter().enumerate() {
            let plan = render::shot_plan(&self.doc.shots[shot], frame, paper);
            let pixels = render::composite_full(&self.doc, &plan);
            let path = dir.join(format!("{base}_{:04}.png", n + 1));
            if let Err(e) = io::write_png(&path, &pixels, self.doc.width, self.doc.height) {
                self.status = format!("Échec de l'export ({}) : {e}", path.display());
                return;
            }
        }
        self.status = format!("{} images exportées dans {}", frames.len(), dir.display());
    }

    pub(crate) fn export_shot_sequence(&mut self) {
        self.end_stroke();
        let Some(dir) = Self::pick_folder("Dossier où exporter le plan") else { return };
        let frames: Vec<_> = (0..self.frame_count()).map(|f| (self.cur_shot, f)).collect();
        self.export_frames(&dir, &frames);
    }

    pub(crate) fn export_montage_sequence(&mut self) {
        self.end_stroke();
        let Some(dir) = Self::pick_folder("Dossier où exporter tout le montage") else { return };
        let frames: Vec<_> = self
            .doc
            .shots
            .iter()
            .enumerate()
            .flat_map(|(s, shot)| (0..shot.frame_count()).map(move |f| (s, f)))
            .collect();
        self.export_frames(&dir, &frames);
    }

    /// Une planche par plan (première image) et une page HTML imprimable avec les indications.
    pub(crate) fn export_storyboard(&mut self) {
        self.end_stroke();
        let Some(dir) = Self::pick_folder("Dossier où exporter le storyboard") else { return };
        let title = io::html_escape(&self.base_name());
        let mut html = format!(
            "<!doctype html><html lang=\"fr\"><head><meta charset=\"utf-8\"><title>{title}</title><style>\
             body{{font-family:system-ui,sans-serif;margin:24px;color:#222}}\
             h1{{font-size:20px}}main{{display:grid;grid-template-columns:repeat(2,1fr);gap:24px}}\
             section{{border:1px solid #ccc;padding:10px;break-inside:avoid}}\
             img{{width:100%;border:1px solid #999}}h2{{font-size:15px;margin:8px 0 2px}}\
             .dur{{color:#777;font-size:12px;margin:0 0 6px}}p{{font-size:13px;margin:4px 0}}\
             .label{{font-weight:600}}</style></head><body><h1>{title} — storyboard</h1><main>"
        );
        let (w, h, fps) = (self.doc.width, self.doc.height, self.doc.fps);
        for (i, shot) in self.doc.shots.iter().enumerate() {
            let plan = render::shot_plan(shot, 0, Some(self.doc.paper));
            let pixels = render::composite_full(&self.doc, &plan);
            let file = format!("planche_{:03}.png", i + 1);
            if let Err(e) = io::write_png_scaled(&dir.join(&file), &pixels, w, h, 960) {
                self.status = format!("Échec de l'export : {e}");
                return;
            }
            let name = if shot.name.is_empty() {
                String::new()
            } else {
                format!(" — {}", io::html_escape(&shot.name))
            };
            html += &format!(
                "<section><img src=\"{file}\"><h2>Plan {}{name}</h2><p class=\"dur\">{}</p>",
                i + 1,
                crate::storyboard::duration_label(shot.frame_count(), fps)
            );
            for (label, text) in [("Action", &shot.action), ("Dialogue", &shot.dialogue), ("Notes", &shot.notes)] {
                if !text.trim().is_empty() {
                    html += &format!("<p><span class=\"label\">{label} :</span> {}</p>", io::html_escape(text));
                }
            }
            html += "</section>";
        }
        html += "</main></body></html>";
        self.status = match std::fs::write(dir.join("storyboard.html"), html) {
            Ok(()) => format!("Storyboard exporté : {}", dir.join("storyboard.html").display()),
            Err(e) => format!("Échec de l'export : {e}"),
        };
    }

    pub(crate) fn export_frame(&mut self) {
        self.end_stroke();
        let Some(mut path) = rfd::FileDialog::new()
            .add_filter("Image PNG", &["png"])
            .set_file_name(format!("image_{:04}.png", self.global_frame() + 1))
            .save_file()
        else {
            return;
        };
        if path.extension().is_none() {
            path.set_extension("png");
        }
        let plan = self.build_plan(self.cur_frame, false, !self.export_transparent);
        let pixels = render::composite_full(&self.doc, &plan);
        self.status = match io::write_png(&path, &pixels, self.doc.width, self.doc.height) {
            Ok(()) => format!("Image exportée : {}", path.display()),
            Err(e) => format!("Échec de l'export : {e}"),
        };
    }

    /// Pose une image (PNG/JPEG) au centre du dessin courant d'un calque bitmap.
    pub(crate) fn import_image(&mut self) {
        self.end_stroke();
        if self.layer().kind != LayerKind::Bitmap {
            self.status = "Choisis un calque bitmap pour importer une image.".into();
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg"])
            .pick_file()
        else {
            return;
        };
        let img = match io::load_image(&path) {
            Ok(i) => i,
            Err(e) => {
                self.status = e;
                return;
            }
        };
        if !self.layer_editable() {
            return;
        }
        let (w, h) = (self.doc.width as u32, self.doc.height as u32);
        let img = if img.width() > w || img.height() > h {
            let k = (w as f32 / img.width() as f32).min(h as f32 / img.height() as f32);
            let nw = ((img.width() as f32 * k).round() as u32).clamp(1, w);
            let nh = ((img.height() as f32 * k).round() as u32).clamp(1, h);
            image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle)
        } else {
            img
        };
        let (ox, oy) = ((w - img.width()) / 2, (h - img.height()) / 2);
        let (cel, prefix) = self.ensure_cel();
        let Some(c) = self.doc.cels.get_mut(&cel) else { return };
        let raster = &mut c.raster;
        let mut before: HashMap<usize, Option<Tile>> = HashMap::new();
        for (x, y, px) in img.enumerate_pixels() {
            let [r, g, b, a] = px.0;
            if a == 0 {
                continue;
            }
            let (cx, cy) = ((ox + x) as usize, (oy + y) as usize);
            let idx = raster.tile_index_of(cx, cy);
            if !before.contains_key(&idx) {
                before.insert(idx, raster.tiles[idx].clone());
            }
            let i = (cy % TILE) * TILE + cx % TILE;
            let tile = raster.tile_mut(idx);
            tile[i] = render::over(tile[i], premultiply(r, g, b, a), 255, None);
        }
        self.push_pixels(cel, prefix, before.into_iter().collect());
        self.status = format!("Image importée : {}", path.display());
    }

    // ------------------------------------------------------------------ vue

    pub(crate) fn rotate_view(&mut self, degrees: f32) {
        let (rect, size) = (self.canvas_rect, self.canvas_size());
        self.view
            .rotate_around(rect, size, rect.center(), degrees.to_radians());
    }

    pub(crate) fn reset_rotation(&mut self) {
        let (rect, size) = (self.canvas_rect, self.canvas_size());
        self.view.reset_rotation(rect, size);
    }

    pub(crate) fn flip_view(&mut self) {
        let (rect, size) = (self.canvas_rect, self.canvas_size());
        self.view.toggle_flip(rect, size);
    }

    /// Un pixel du dessin = un pixel de l'écran.
    pub(crate) fn zoom_actual(&mut self) {
        let (rect, size) = (self.canvas_rect, self.canvas_size());
        let factor = (1.0 / self.ppp) / self.view.zoom;
        self.view.zoom_around(rect, size, rect.center(), factor);
    }

    // ------------------------------------------------------------------ clavier

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // Un bouton cliqué garde le focus clavier : Entrée ou Espace le « recliqueraient ».
        if !ctx.wants_keyboard_input() {
            if let Some(id) = ctx.memory(|m| m.focused()) {
                ctx.memory_mut(|m| m.surrender_focus(id));
            }
        }
        // Pas de raccourci pendant une saisie, un dialogue, ou un glissé dans la bande de montage.
        if ctx.wants_keyboard_input() || self.modal_open() || self.montage_drag.is_some() {
            self.pending_keys.clear();
            return;
        }
        // Touches retirées à egui avant qu'il ne s'en serve (Tab), puis touches de cette image.
        let mut keys: Vec<(Key, Option<Key>, egui::Modifiers, bool)> = std::mem::take(&mut self.pending_keys)
            .into_iter()
            .map(|(k, p, m)| (k, p, m, false))
            .collect();
        ctx.input(|i| {
            for e in &i.events {
                match e {
                    Event::Key {
                        key,
                        physical_key,
                        pressed: true,
                        repeat,
                        modifiers,
                        ..
                    } => keys.push((*key, *physical_key, *modifiers, *repeat)),
                    // egui transforme Ctrl+C / X / V en événements de presse-papiers.
                    Event::Copy => keys.push((Key::C, Some(Key::C), egui::Modifiers::COMMAND, false)),
                    Event::Cut => keys.push((Key::X, Some(Key::X), egui::Modifiers::COMMAND, false)),
                    Event::Paste(_) => keys.push((Key::V, Some(Key::V), egui::Modifiers::COMMAND, false)),
                    _ => {}
                }
            }
        });
        for (key, physical, m, repeat) in keys {
            let Some(action) = self.keymap.find(key, physical, m, self.mode) else { continue };
            let action = action.to_string();
            if repeat && !keymap::action_info(&action).is_some_and(|a| a.repeat) {
                continue;
            }
            self.run_action(&action);
        }
    }

    /// Exécute une action désignée par son identifiant stable (voir `keymap::ACTIONS`).
    pub(crate) fn run_action(&mut self, action: &str) {
        let board = self.mode == Mode::Storyboard;
        match action {
            "fichier.nouveau" => self.request(Pending::New),
            "fichier.ouvrir" => self.request(Pending::Open),
            "fichier.enregistrer" => {
                self.save();
            }
            "fichier.enregistrer_sous" => {
                self.save_as();
            }
            "fichier.quitter" => self.request(Pending::Quit),
            "edition.annuler" => self.undo(),
            "edition.retablir" => self.redo(),
            "edition.couper" => self.cut_selection(),
            "edition.copier" => {
                self.copy_selection();
            }
            "edition.coller" => self.paste(),
            "edition.supprimer" => {
                if self.tool == Tool::Select && self.vsel.is_some() {
                    self.delete_selection();
                } else {
                    self.clear_image();
                }
            }
            "edition.tout_selectionner" => self.select_all(),
            "edition.tout_deselectionner" => {
                self.end_stroke();
                self.vsel = None;
            }
            "vue.zoom_avant" => self.zoom_step(1.25),
            "vue.zoom_arriere" => self.zoom_step(1.0 / 1.25),
            "vue.plein_ecran" => {
                let full = self.ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                self.ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!full));
            }
            "vue.rotation_horaire" => self.rotate_view(15.0),
            "vue.rotation_antihoraire" => self.rotate_view(-15.0),
            "vue.reinit_rotation" => self.reset_rotation(),
            "vue.reinit_zoom" => self.zoom_actual(),
            "vue.ajuster" => self.fit_pending = true,
            "vue.pelure" => self.onion.enabled = !self.onion.enabled,
            "vue.miroir" => self.flip_view(),
            "vue.planches" => {
                if board {
                    self.board_view = match self.board_view {
                        BoardView::Scene => BoardView::Panels,
                        BoardView::Panels => BoardView::Scene,
                    };
                } else {
                    self.set_mode(Mode::Storyboard);
                    self.board_view = BoardView::Panels;
                }
            }
            "mode.basculer" => self.set_mode(if board { Mode::Animation } else { Mode::Storyboard }),
            "aide.raccourcis" => self.show_help = !self.show_help,
            "outil.pinceau" => self.set_tool(Tool::Brush),
            "outil.crayon" => self.set_tool(Tool::Pencil),
            "outil.gomme" => self.set_tool(Tool::Eraser),
            "outil.remplissage" => self.set_tool(Tool::Fill),
            "outil.pipette" => self.set_tool(Tool::Picker),
            "outil.selection" => self.set_tool(Tool::Select),
            "outil.formes" => self.set_tool(Tool::Shape),
            "outil.main" => self.set_tool(Tool::Pan),
            "outil.taille_plus" => self.resize_brush(1.15),
            "outil.taille_moins" => self.resize_brush(1.0 / 1.15),
            "selection.miroir_h" => self.flip_selection(true),
            "selection.miroir_v" => self.flip_selection(false),
            "lecture.basculer" => self.toggle_play(),
            "image.precedente" if board => self.step_global(-1),
            "image.suivante" if board => self.step_global(1),
            "image.precedente" => self.goto_frame(self.cur_frame.saturating_sub(1)),
            "image.suivante" => self.goto_frame(self.cur_frame + 1),
            "image.premiere" => self.goto_panel_frame(0),
            "image.derniere" => self.goto_panel_frame(self.frame_count() - 1),
            "montage.debut" => self.goto_global(0),
            "montage.fin" => self.goto_global(self.doc.total_frames() - 1),
            "panneau.precedent" => self.select_shot(self.cur_shot.saturating_sub(1)),
            "panneau.suivant" => self.select_shot(self.cur_shot + 1),
            "panneau.nouveau" => self.new_shot(),
            "panneau.supprimer" => self.delete_shot(),
            "panneau.ajouter_image" => self.nudge_shot_length(1),
            "panneau.retirer_image" => self.nudge_shot_length(-1),
            "dessin.nouveau" => self.new_image(false),
            "dessin.dupliquer" => self.new_image(true),
            "dessin.supprimer" => self.delete_image(),
            "exposition.allonger" => self.exposure_plus(),
            "exposition.raccourcir" => self.exposure_minus(),
            "calque.nouveau_vectoriel" => self.add_layer(LayerKind::Vector),
            "calque.nouveau_bitmap" => self.add_layer(LayerKind::Bitmap),
            "calque.supprimer" => self.delete_layer(),
            "calque.dessus" => self.select_layer(self.cur_layer + 1),
            "calque.dessous" => self.select_layer(self.cur_layer.saturating_sub(1)),
            // « vue.pan_libre » se maintient : il est lu en continu par le canevas.
            "vue.pan_libre" => {}
            other => self.status = format!("« {other} » n'est pas encore disponible."),
        }
    }

    fn set_tool(&mut self, tool: Tool) {
        if self.tool != tool {
            self.end_stroke();
            self.tool = tool;
        }
    }

    /// Zoom par pas, autour du centre de la vue.
    fn zoom_step(&mut self, factor: f32) {
        let (rect, size) = (self.canvas_rect, self.canvas_size());
        self.view.zoom_around(rect, size, rect.center(), factor);
    }

    /// Va à une image du panneau courant (dans le montage en mode storyboard).
    fn goto_panel_frame(&mut self, frame: usize) {
        match self.mode {
            Mode::Storyboard => self.goto_global(self.doc.shot_start(self.cur_shot) + frame),
            Mode::Animation => self.goto_frame(frame),
        }
    }

    /// Allonge ou raccourcit le panneau d'une image (une étape d'annulation par appui).
    fn nudge_shot_length(&mut self, delta: i64) {
        let n = (self.frame_count() as i64 + delta).max(1) as usize;
        if n == self.frame_count() {
            return;
        }
        self.end_stroke();
        let snap = self.snapshot();
        self.shot_mut().set_length(n);
        self.length_origin = None;
        self.commit_structure(snap);
        self.status = format!(
            "Durée du panneau : {}",
            crate::storyboard::duration_label(n, self.doc.fps)
        );
    }

    /// Recharge les raccourcis (après modification du fichier).
    pub(crate) fn reload_keymap(&mut self) {
        self.keymap = Keymap::load();
        self.status = match (&self.keymap.user_file, self.keymap.warnings.len()) {
            (Some(p), 0) => format!("Raccourcis rechargés depuis {}.", p.display()),
            (Some(p), n) => format!("Raccourcis rechargés depuis {} ({n} remarque(s), voir F1).", p.display()),
            (None, 0) => "Raccourcis par défaut.".into(),
            (None, n) => format!("Raccourcis par défaut ({n} remarque(s), voir F1)."),
        };
    }

    /// Crée au besoin la copie personnelle du fichier de raccourcis, puis l'ouvre dans le Bloc-notes.
    pub(crate) fn edit_keymap(&mut self) {
        let path = match &self.keymap.user_file {
            Some(p) => p.clone(),
            None => {
                let Some(p) = Keymap::appdata_path() else {
                    self.status = "Dossier %APPDATA% introuvable.".into();
                    return;
                };
                if let Some(dir) = p.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if let Err(e) = std::fs::write(&p, keymap::DEFAULT_KEYMAP) {
                    self.status = format!("Impossible de créer {} : {e}", p.display());
                    return;
                }
                self.keymap.user_file = Some(p.clone());
                p
            }
        };
        match std::process::Command::new("notepad.exe").arg(&path).spawn() {
            Ok(_) => {
                self.status = format!(
                    "Raccourcis ouverts dans le Bloc-notes ({}). Enregistre, puis « Aide > Recharger les raccourcis ».",
                    path.display()
                )
            }
            Err(e) => self.status = format!("Impossible d'ouvrir {} : {e}", path.display()),
        }
    }

    fn handle_close_request(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && self.modified && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.end_stroke();
            self.playing = false;
            self.pending = Some(Pending::Quit);
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Sans titre".into());
        let title = format!("{name}{} — Pelure", if self.modified { " •" } else { "" });
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }
}

impl eframe::App for App {
    /// egui se sert de Tab pour passer d'un champ à l'autre ; hors d'un champ texte, on le lui
    /// retire pour en faire un raccourci comme les autres.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if ctx.wants_keyboard_input() {
            return;
        }
        raw_input.events.retain(|e| match e {
            Event::Key {
                key: Key::Tab,
                physical_key,
                pressed,
                modifiers,
                ..
            } if !modifiers.command => {
                if *pressed {
                    self.pending_keys.push((Key::Tab, *physical_key, *modifiers));
                }
                false
            }
            _ => true,
        });
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ppp = ctx.pixels_per_point();
        self.clamp_nav();
        self.end_length_gesture_if_idle();
        self.handle_close_request(ctx);
        self.handle_shortcuts(ctx);
        self.tick_playback(ctx);
        self.validate_selection();
        self.update_title(ctx);
        self.thumbs.prune(&self.doc);
        self.thumbs.pending = false;

        egui::TopBottomPanel::top("menu").show(ctx, |ui| self.menu_bar(ui));
        egui::TopBottomPanel::bottom("statut").show(ctx, |ui| self.status_bar(ui));
        match self.mode {
            Mode::Storyboard => {
                egui::TopBottomPanel::bottom("montage")
                    .resizable(true)
                    .default_height(240.0)
                    .min_height(140.0)
                    .show(ctx, |ui| self.montage_panel(ui));
            }
            Mode::Animation => {
                egui::TopBottomPanel::bottom("timeline")
                    .resizable(true)
                    .default_height(210.0)
                    .min_height(110.0)
                    .show(ctx, |ui| self.timeline_panel(ui));
            }
        }
        egui::SidePanel::left("outils")
            .resizable(false)
            .exact_width(236.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.tools_panel(ui));
            });
        egui::SidePanel::right("plan_calques")
            .default_width(250.0)
            .show(ctx, |ui| self.right_panel(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::from_gray(42)))
            .show(ctx, |ui| {
                if self.mode == Mode::Storyboard && self.board_view == BoardView::Panels {
                    self.panels_view(ui);
                } else {
                    self.canvas_panel(ui);
                }
            });

        self.dialogs(ctx);
        if self.thumbs.pending {
            ctx.request_repaint();
        }
    }
}

/// Dessins voisins (distincts du dessin courant) pour la pelure d'oignon, du plus proche au plus lointain.
fn onion_neighbours(layer: &Layer, frame: usize, n_before: usize, n_after: usize) -> (Vec<CelId>, Vec<CelId>) {
    let current = layer.cel_at(frame);
    let mut before = Vec::new();
    let mut f = frame.min(layer.frames.len());
    while f > 0 && before.len() < n_before {
        f -= 1;
        if let Some(id) = layer.frames[f] {
            if Some(id) != current && !before.contains(&id) {
                before.push(id);
            }
        }
    }
    let mut after = Vec::new();
    let mut f = frame + 1;
    while f < layer.frames.len() && after.len() < n_after {
        if let Some(id) = layer.frames[f] {
            if Some(id) != current && !after.contains(&id) {
                after.push(id);
            }
        }
        f += 1;
    }
    (before, after)
}

fn fade(opacity: f32, k: usize, n: usize) -> u8 {
    let n = n.max(1);
    (opacity * (n - k.min(n - 1)) as f32 / n as f32 * 255.0).round() as u8
}
