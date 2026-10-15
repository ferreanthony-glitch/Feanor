use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui::{self, vec2, Color32, Event, Key, Pos2, Rect, Vec2};

use crate::audio::AudioEngine;
use crate::brush::{BrushSettings, StrokeMode, StrokeState};
use crate::camtools::CamDrag;
use crate::layertools::LayerDrag;
use crate::document::{fresh_layer_id, CelId, Document, Layer, LayerKind, Panel};
use crate::fill::{self, FillSettings};
use crate::gpu::GpuCanvas;
use crate::guides::GuideState;
use crate::history::{Edit, Gesture, History, Nav};
use crate::io;
use crate::keymap::{self, Keymap};
use crate::pixsel::Floating;
use crate::pixtools::{BDrag, BSelection};
use crate::project;
use crate::raster::{premultiply, unpremultiply, Tile, TILE};
use crate::render::{self, Plan, PlanItem, Renderer};
use crate::soundtrack::MenuTarget;
use crate::storyboard::MontageDrag;
use crate::thumbs::Thumbs;
use crate::vector::{Affine, Element, VectorDrawing};
use crate::view::View;
use crate::vtools::{VDrag, VSelection, VectorErase, VectorStroke};

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
    /// Formes (5c) : outils séparés, comme dans Storyboard Pro.
    Line,
    Rect,
    Ellipse,
    Polyline,
    Select,
    Picker,
    Pan,
    Camera,
    Transform,
    /// Repère 3D (2d) : tourner autour du modèle 3D du panneau.
    Ref3d,
    /// Texte (5d).
    Text,
    /// Tampon, comme dans Storyboard Pro, et tampon de duplication (5e).
    Stamp,
    Clone,
    /// Déformer en perspective (5f).
    Perspective,
}

impl Tool {
    pub const ALL: [Tool; 18] = [
        Tool::Brush,
        Tool::Pencil,
        Tool::Eraser,
        Tool::Fill,
        Tool::Stamp,
        Tool::Clone,
        Tool::Line,
        Tool::Rect,
        Tool::Ellipse,
        Tool::Polyline,
        Tool::Text,
        Tool::Select,
        Tool::Picker,
        Tool::Pan,
        Tool::Camera,
        Tool::Transform,
        Tool::Perspective,
        Tool::Ref3d,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tool::Brush => "Pinceau",
            Tool::Pencil => "Crayon",
            Tool::Eraser => "Gomme",
            Tool::Fill => "Remplir",
            Tool::Line => "Ligne",
            Tool::Rect => "Rectangle",
            Tool::Ellipse => "Ellipse",
            Tool::Polyline => "Polyligne",
            Tool::Select => "Sélection",
            Tool::Picker => "Pipette",
            Tool::Pan => "Main",
            Tool::Camera => "Caméra",
            Tool::Transform => "Transformer",
            Tool::Ref3d => "Repère 3D",
            Tool::Text => "Texte",
            Tool::Stamp => "Tampon",
            Tool::Clone => "Tampon de duplication",
            Tool::Perspective => "Perspective",
        }
    }

    /// Petite description, pour la bulle d'aide de son bouton (12d).
    pub fn description(self) -> &'static str {
        match self {
            Tool::Brush => "Peindre ; la taille peut suivre la pression du stylet.",
            Tool::Pencil => "Dessiner au crayon.",
            Tool::Eraser => "Effacer ; sur un calque vectoriel, des bouts de trait ou des traits entiers.",
            Tool::Fill => "Remplir une zone avec la couleur courante.",
            Tool::Line => "Tracer une droite, une courbe ou une courbe en S.",
            Tool::Rect => "Tracer un rectangle, vide ou plein.",
            Tool::Ellipse => "Tracer une ellipse, vide ou pleine.",
            Tool::Polyline => "Tracer une suite de segments, clic après clic.",
            Tool::Select => "Choisir une zone (lasso ou rectangle) pour la déplacer, la copier ou l'effacer.",
            Tool::Picker => "Prendre une couleur sur le dessin.",
            Tool::Pan => "Déplacer la vue.",
            Tool::Camera => "Régler le cadre de la caméra du panneau : déplacer, zoomer par un coin, tourner.",
            Tool::Transform => "Déplacer, agrandir ou tourner le calque entier (tous ses dessins).",
            Tool::Ref3d => "Un modèle 3D en fil de fer comme guide, vu depuis le cadre du panneau.",
            Tool::Text => "Clic : poser un texte ; Ctrl + Entrée pour le valider.",
            Tool::Stamp => "Poser des images le long du trait, dans l'ordre ou au hasard.",
            Tool::Clone => "Ctrl + clic pour la source, puis peindre : recopie une autre partie du dessin.",
            Tool::Perspective => "Tirer les quatre coins du dessin pour le mettre en perspective.",
        }
    }

    /// Icône de l'outil (police d'icônes Phosphor, chargée au démarrage).
    pub fn icon(self) -> &'static str {
        use egui_phosphor::regular as ph;
        match self {
            Tool::Brush => ph::PAINT_BRUSH,
            Tool::Pencil => ph::PENCIL_SIMPLE,
            Tool::Eraser => ph::ERASER,
            Tool::Fill => ph::PAINT_BUCKET,
            Tool::Line => ph::LINE_SEGMENT,
            Tool::Rect => ph::RECTANGLE,
            Tool::Ellipse => ph::CIRCLE,
            Tool::Polyline => ph::LINE_SEGMENTS,
            Tool::Select => ph::LASSO,
            Tool::Picker => ph::EYEDROPPER,
            Tool::Pan => ph::HAND,
            Tool::Camera => ph::VIDEO_CAMERA,
            Tool::Transform => ph::BOUNDING_BOX,
            Tool::Ref3d => ph::CUBE,
            Tool::Text => ph::TEXT_T,
            Tool::Stamp => ph::STAMP,
            Tool::Clone => ph::COPY,
            Tool::Perspective => ph::PERSPECTIVE,
        }
    }

    /// Action qui choisit cet outil (son raccourci est dans le fichier de raccourcis).
    pub fn action(self) -> &'static str {
        match self {
            Tool::Brush => "outil.pinceau",
            Tool::Pencil => "outil.crayon",
            Tool::Eraser => "outil.gomme",
            Tool::Fill => "outil.remplissage",
            Tool::Line => "outil.ligne",
            Tool::Rect => "outil.rectangle",
            Tool::Ellipse => "outil.ellipse",
            Tool::Polyline => "outil.polyligne",
            Tool::Select => "outil.selection",
            Tool::Picker => "outil.pipette",
            Tool::Pan => "outil.main",
            Tool::Camera => "outil.camera",
            Tool::Transform => "outil.transformer",
            Tool::Ref3d => "outil.repere3d",
            Tool::Text => "outil.texte",
            Tool::Stamp => "outil.tampon",
            Tool::Clone => "outil.tampon_duplication",
            Tool::Perspective => "outil.perspective",
        }
    }

    /// Ligne, rectangle ou ellipse (tracés d'un seul glisser).
    pub fn shape_kind(self) -> Option<ShapeKind> {
        match self {
            Tool::Line => Some(ShapeKind::Line),
            Tool::Rect => Some(ShapeKind::Rect),
            Tool::Ellipse => Some(ShapeKind::Ellipse),
            _ => None,
        }
    }

    /// Un des outils de forme.
    pub fn is_shape(self) -> bool {
        self.shape_kind().is_some() || self == Tool::Polyline
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind {
    Line,
    Rect,
    Ellipse,
}

pub struct ShapeSettings {
    /// Rectangle, ellipse, polyligne fermée : remplie plutôt que tracée.
    pub filled: bool,
    /// Épaisseur du trait, en pixels.
    pub width: f32,
    /// Outil Ligne : droite, courbe ou courbe en S.
    pub line_mode: crate::shapes::LineMode,
    /// Outil Polyligne : refermer la forme en la terminant.
    pub closed: bool,
    /// Dernier outil de forme utilisé (action « outil.formes »).
    pub last: Tool,
}

/// Action qui attend une confirmation parce que le projet n'est pas enregistré.
#[derive(Clone, Copy)]
pub enum Pending {
    New,
    Open,
    Quit,
}

/// Calque copié (W) : sa timeline, ses dessins, et la durée du panneau d'où il vient.
#[derive(Clone)]
pub(crate) struct LayerClip {
    pub layer: Layer,
    pub cels: HashMap<CelId, crate::document::Cel>,
    pub panel_len: usize,
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
    pub(crate) cur_panel: usize,
    pub(crate) cur_layer: usize,
    pub(crate) cur_frame: usize,
    pub(crate) tool: Tool,
    pub(crate) brush: BrushSettings,
    pub(crate) pencil: BrushSettings,
    pub(crate) eraser: BrushSettings,
    /// Pointes, papiers et réglages de pinceau (ceux de Pelure et les importés).
    pub(crate) brushes: crate::brushtex::BrushLibrary,
    pub(crate) fill: FillSettings,
    pub(crate) shape: ShapeSettings,
    pub(crate) vector_erase_whole: bool,
    pub(crate) color: [u8; 3],
    /// La couleur en teinte, saturation, valeur (panneau Couleur, 6b).
    pub(crate) hsv: [f32; 3],
    pub(crate) swatches: Vec<[u8; 3]>,
    pub(crate) view: View,
    pub(crate) fit_pending: bool,
    pub(crate) canvas_rect: Rect,
    pub(crate) ppp: f32,
    /// Pelure d'oignon (réglage de l'application, retenu dans `settings`).
    pub(crate) onion: crate::onion::OnionSettings,
    /// Table lumineuse : les calques autres que le calque courant sont estompés.
    pub(crate) light_table: bool,
    /// Opacité gardée par les calques estompés (0 à 1).
    pub(crate) light_table_strength: f32,
    /// Guides de perspective : affichage, alignement, verrouillage, glisser d'un point.
    pub(crate) guides: GuideState,
    /// Réglage de la caméra à la souris (outil Caméra).
    pub(crate) cam_drag: Option<CamDrag>,
    /// Transformation d'un calque à la souris (outil Transformer).
    pub(crate) layer_drag: Option<LayerDrag>,
    /// Vignettes des calques (panneau Calques, 6c).
    pub(crate) layer_thumbs: crate::layersui::LayerThumbs,
    /// Calque en cours de renommage (9b) : id, nom tapé, saisie déjà activée.
    pub(crate) layer_rename: Option<(u64, String, bool)>,
    /// Aperçus des réglages de pinceau (grille du panneau Réglages, 6d).
    pub(crate) brush_previews: crate::brushui::BrushPreviews,
    /// Repère 3D : geste en cours, fil de fer calculé (image) et calcul en cours.
    pub(crate) ref3d: crate::ref3dtool::Ref3dState,
    /// Vue à travers la caméra du panneau (sinon : la feuille, avec le cadre tracé).
    pub(crate) camera_view: bool,
    /// Vue du plateau laissée en passant à la vue caméra, remise au retour (9d).
    pub(crate) stage_view: Option<View>,
    /// Cadre de format personnalisé choisi (même s'il vaut l'un des formats proposés).
    pub(crate) format_custom: bool,
    /// Plateau en 3D (9e) : affiché, outil, vue, geste en cours, textures.
    pub(crate) stage3d: crate::stage3dui::Stage3dState,
    /// Le dessin est affiché dans la vignette choisie de la vue Vignettes (6e).
    pub(crate) in_card: bool,
    /// Planche de la vue Vignettes (7) : zoom, décalage, panneaux choisis (ids), geste en cours.
    pub(crate) board_zoom: f32,
    pub(crate) board_pan: Vec2,
    pub(crate) board_sel: Vec<u64>,
    pub(crate) board_drag: Option<crate::boardui::BoardDrag>,
    /// Notes choisies sur la planche, et texte de note en cours d'écriture (8).
    pub(crate) board_notes: Vec<crate::board::NoteRef>,
    pub(crate) board_text: Option<crate::boardui::NoteTextEdit>,
    /// Hors-champ estompé sur la feuille.
    pub(crate) camera_mask: bool,
    /// Fenêtre « Vue de dessus » (profondeur des calques) et son glisser en cours.
    pub(crate) top_view: bool,
    pub(crate) top_drag: Option<crate::topview::TopDrag>,
    /// Cadence réellement affichée pendant la lecture.
    pub(crate) play_meter: crate::playback::PlayMeter,
    /// Début de l'image précédente (rythme des images).
    frame_start: Option<std::time::Instant>,
    /// Fenêtre d'export ouverte, derniers choix, export en cours.
    pub(crate) export_dialog: Option<crate::export::ExportSettings>,
    pub(crate) export_last: crate::export::ExportSettings,
    pub(crate) export_job: Option<crate::export::ExportJob>,
    /// ffmpeg trouvé (ou choisi) et ce qu'il sait encoder ; cherché à la première ouverture.
    pub(crate) ffmpeg: Option<Option<(PathBuf, Result<crate::export::Encoders, String>)>>,
    /// Réglages de l'application (%APPDATA%\Pelure\reglages.toml).
    pub(crate) settings: crate::settings::Settings,
    /// Dernière disposition appliquée (espace de travail) ; les panneaux eux-mêmes, que l'on
    /// glisse et aimante, sont dans `dock` (affiché par egui_dock).
    pub(crate) layout: crate::workspace::Layout,
    pub(crate) dock: Option<egui_dock::DockState<crate::dock::Pane>>,
    /// Panneaux dans leur propre fenêtre Windows (5g), et celui à y mettre (clic droit sur
    /// son onglet).
    pub(crate) detached: Vec<crate::dock::DetachedPanel>,
    pub(crate) detach_request: Option<crate::dock::Pane>,
    /// Nom en cours de saisie pour enregistrer l'espace de travail.
    pub(crate) workspace_name: Option<String>,
    /// Copie de secours de cette séance, et séances interrompues à proposer au démarrage.
    pub(crate) recovery: Option<crate::recovery::Recovery>,
    pub(crate) orphans: Vec<crate::recovery::Orphan>,
    pub(crate) default_exposure: usize,
    pub(crate) playing: bool,
    pub(crate) play_montage: bool,
    pub(crate) looping: bool,
    play_accum: f64,
    last_time: Option<f64>,
    /// Sortie audio (ouverte à la première lecture avec du son).
    pub(crate) audio: Option<AudioEngine>,
    pub(crate) audio_tried: bool,
    /// La lecture en cours suit la position du son (sinon l'horloge de l'ordinateur).
    pub(crate) audio_clock: bool,
    /// Son pendant la lecture.
    pub(crate) sound_on: bool,
    /// Son au défilement (une image de son à chaque changement d'image).
    pub(crate) sound_scrub: bool,
    /// Dernière image entendue au défilement.
    pub(crate) heard_frame: Option<usize>,
    /// Extrait de son sélectionné dans la bande de montage.
    pub(crate) sel_clip: Option<u64>,
    /// Ce qu'on a cliqué du bouton droit dans la bande de montage.
    pub(crate) menu_target: Option<MenuTarget>,
    pub(crate) history: History,
    /// Geste de réglage en cours (champ, curseur, point de fuite) : une étape d'annulation par geste.
    gesture: Gesture,
    pub(crate) stroke: Option<StrokeState>,
    pub(crate) vstroke: Option<VectorStroke>,
    pub(crate) verase: Option<VectorErase>,
    pub(crate) vsel: Option<VSelection>,
    pub(crate) vdrag: Option<VDrag>,
    /// Sélection dans un dessin bitmap, et geste en cours avec l'outil Sélection.
    pub(crate) bsel: Option<BSelection>,
    pub(crate) bdrag: Option<BDrag>,
    /// Sélection bitmap au rectangle (sinon au lasso).
    pub(crate) select_rect: bool,
    /// Zone bitmap copiée.
    pub(crate) pixel_clipboard: Option<Floating>,
    /// Calque copié (W / X).
    pub(crate) layer_clipboard: Option<LayerClip>,
    /// Panneaux copiés (12e).
    pub(crate) panel_clipboard: Option<crate::document::PanelClip>,
    /// Le montage (ou la planche des vignettes) a la main : on y a cliqué en dernier, pas dans le
    /// dessin. Copier, coller et supprimer y visent alors les panneaux choisis (12e).
    pub(crate) panel_focus: bool,
    /// Clé de caméra que l'on fait glisser dans la timeline ou le montage (12e).
    pub(crate) cam_key_drag: Option<crate::camtools::CamKeyDrag>,
    pub(crate) shape_drag: Option<(Pos2, Pos2)>,
    /// Forme en plusieurs gestes : ligne à courber, polyligne en cours (5c).
    pub(crate) shape_edit: Option<crate::shapes::ShapeEdit>,
    /// Texte en cours de saisie, police et taille des nouveaux textes, polices du poste (5d).
    pub(crate) text_edit: Option<crate::texttools::TextEditState>,
    pub(crate) text_font: String,
    pub(crate) text_size: f32,
    pub(crate) text_fonts: Option<Vec<String>>,
    pub(crate) text_font_filter: String,
    /// Tampons (5e) : bibliothèque et réglages, trait en cours ; tampon de duplication.
    pub(crate) stamps: crate::stamptools::StampLibrary,
    pub(crate) stamp_stroke: Option<crate::stamptools::StampStroke>,
    pub(crate) clone: crate::stamptools::CloneSettings,
    pub(crate) clone_stroke: Option<crate::stamptools::CloneStrokeState>,
    /// Déformation en perspective en cours (5f).
    pub(crate) persp: Option<crate::perstools::PerspEdit>,
    pub(crate) clipboard: Vec<Element>,
    pub(crate) renderer: Renderer,
    pub(crate) thumbs: Thumbs,
    pub(crate) montage_zoom: f32,
    pub(crate) montage_drag: Option<MontageDrag>,
    /// Plan tel qu'il était au début du réglage de sa durée : chaque valeur intermédiaire
    /// repart de lui, rien n'est perdu avant la fin du geste.
    pub(crate) length_origin: Option<Panel>,
    /// Un champ de durée est en cours d'utilisation pendant cette image d'écran.
    pub(crate) length_field_active: bool,
    pub(crate) pen_pressure: f32,
    pub(crate) pen_touch: bool,
    pub(crate) picking: bool,
    /// Manifeste du projet en dossier (`MonFilm/MonFilm.pelure`).
    pub(crate) path: Option<PathBuf>,
    /// Ancien fichier `.pelure` importé, pas encore enregistré en dossier.
    pub(crate) import_source: Option<PathBuf>,
    /// Version de chaque dessin déjà écrit dans le dossier du projet.
    pub(crate) written: HashMap<CelId, u64>,
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
    /// Moteur d'affichage utilisé (carte graphique et son interface, ou OpenGL).
    pub(crate) display_info: String,
    /// Superposition des calques par la carte graphique (absente en OpenGL ou si on la coupe).
    pub(crate) gpu: Option<GpuCanvas>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Thème sombre et dense, repris de Pelure Studio (12a).
        crate::theme::appliquer(&cc.egui_ctx);
        // Police de l'interface (Segoe UI si Windows l'a) et icônes (Phosphor), utilisables dans
        // tout texte.
        let mut fonts = egui::FontDefinitions::default();
        crate::theme::polices(&mut fonts);
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);
        // Polices du poste (outil Texte) : lues sur un autre fil dès le démarrage.
        crate::text::scan_in_background();
        if let Some(rs) = &cc.wgpu_render_state {
            let info = rs.adapter.get_info();
            eprintln!("affichage : wgpu, {} ({:?}, {:?})", info.name, info.backend, info.device_type);
        }
        let settings = crate::settings::Settings::load();
        let mut app = Self {
            ctx: cc.egui_ctx.clone(),
            doc: Document::with_frame(1920, 1080, 24),
            mode: Mode::Storyboard,
            board_view: BoardView::Scene,
            cur_panel: 0,
            cur_layer: 0,
            cur_frame: 0,
            tool: Tool::Brush,
            brush: BrushSettings::brush(),
            pencil: BrushSettings::pencil(),
            eraser: BrushSettings::eraser(),
            brushes: crate::brushtex::BrushLibrary::load(crate::brushtex::BrushLibrary::user_dir().as_deref()),
            fill: FillSettings::default(),
            shape: ShapeSettings {
                filled: false,
                width: 4.0,
                line_mode: crate::shapes::LineMode::Straight,
                closed: true,
                last: Tool::Rect,
            },
            vector_erase_whole: false,
            color: [24, 24, 28],
            hsv: crate::colorui::rgb_to_hsv([24, 24, 28]),
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
            onion: settings.onion.clone(),
            light_table: false,
            light_table_strength: 0.3,
            guides: GuideState::default(),
            cam_drag: None,
            layer_drag: None,
            layer_thumbs: Default::default(),
            layer_rename: None,
            brush_previews: Default::default(),
            ref3d: Default::default(),
            camera_view: false,
            stage_view: None,
            format_custom: false,
            stage3d: Default::default(),
            in_card: false,
            // Nul : la page sera mise à la largeur de la vue au premier affichage.
            board_zoom: 0.0,
            board_pan: Vec2::ZERO,
            board_sel: Vec::new(),
            board_drag: None,
            board_notes: Vec::new(),
            board_text: None,
            camera_mask: true,
            top_view: false,
            top_drag: None,
            play_meter: Default::default(),
            frame_start: None,
            export_dialog: None,
            export_last: Default::default(),
            export_job: None,
            ffmpeg: None,
            layout: crate::workspace::Layout::default(),
            dock: None,
            detached: Vec::new(),
            detach_request: None,
            workspace_name: None,
            settings,
            // Les séances interrompues d'abord (avant d'ouvrir la nôtre).
            orphans: crate::recovery::root().map(|r| crate::recovery::orphans(&r, None)).unwrap_or_default(),
            recovery: crate::recovery::root().and_then(|r| crate::recovery::Recovery::start(&r).ok()),
            default_exposure: 1,
            playing: false,
            play_montage: false,
            looping: true,
            play_accum: 0.0,
            last_time: None,
            audio: None,
            audio_tried: false,
            audio_clock: false,
            sound_on: true,
            sound_scrub: true,
            heard_frame: Some(0),
            sel_clip: None,
            menu_target: None,
            history: History::default(),
            gesture: Gesture::default(),
            stroke: None,
            vstroke: None,
            verase: None,
            vsel: None,
            vdrag: None,
            bsel: None,
            bdrag: None,
            select_rect: false,
            pixel_clipboard: None,
            layer_clipboard: None,
            panel_clipboard: None,
            panel_focus: false,
            cam_key_drag: None,
            shape_drag: None,
            shape_edit: None,
            text_edit: None,
            text_font: "Arial".into(),
            text_size: 48.0,
            text_fonts: None,
            text_font_filter: String::new(),
            stamps: crate::stamptools::StampLibrary::load(),
            stamp_stroke: None,
            clone: Default::default(),
            clone_stroke: None,
            persp: None,
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
            import_source: None,
            written: HashMap::new(),
            modified: false,
            status: "Storyboard : dessine le premier panneau ; P pour un nouveau panneau, double-clic sur un panneau pour l'animer (F1 : raccourcis).".into(),
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
            display_info: match &cc.wgpu_render_state {
                Some(rs) => {
                    let info = rs.adapter.get_info();
                    format!("wgpu, {} ({:?})", info.name, info.backend)
                }
                None => "OpenGL (moteur de secours)".into(),
            },
            gpu: None,
        };
        // Calques superposés par la carte graphique, sauf PELURE_COMPOSITION=processeur.
        let cpu = std::env::var("PELURE_COMPOSITION").is_ok_and(|v| v.eq_ignore_ascii_case("processeur"));
        if !cpu {
            app.gpu = cc.wgpu_render_state.as_ref().map(GpuCanvas::new);
        }
        app.renderer.set_gpu(app.gpu.is_some());
        app.display_info.push_str(if app.gpu.is_some() {
            " · calques par la carte graphique"
        } else {
            " · calques par le processeur"
        });
        // La disposition de la dernière fois.
        if let Some(mut layout) = app.settings.layout.clone() {
            // Disposition d'avant 6a : la nouvelle disposition par défaut (barre d'outils
            // verticale, calques à côté du dessin), en gardant les panneaux affichés.
            if layout.is_before_6a() {
                let d = layout.dock_layout();
                layout = crate::workspace::Layout {
                    tools: d.contains(crate::dock::Pane::ToolOptions),
                    sheet: d.contains(crate::dock::Pane::Layers) || d.contains(crate::dock::Pane::Sheet),
                    strip: d.contains(crate::dock::Pane::Strip),
                    panneaux: None,
                    ..crate::workspace::Layout::default()
                };
            }
            // (Les cadres de composition de la dernière fois aussi.)
            app.layout.safe_frames = layout.safe_frames;
            app.layout.frame_format = layout.frame_format;
            app.apply_layout(layout);
        }
        app
    }

    // ------------------------------------------------------------------ accès

    pub(crate) fn panel(&self) -> &Panel {
        &self.doc.panels[self.cur_panel]
    }

    pub(crate) fn panel_mut(&mut self) -> &mut Panel {
        &mut self.doc.panels[self.cur_panel]
    }

    pub(crate) fn layers(&self) -> &[Layer] {
        &self.doc.panels[self.cur_panel].layers
    }

    pub(crate) fn layers_mut(&mut self) -> &mut Vec<Layer> {
        &mut self.doc.panels[self.cur_panel].layers
    }

    pub(crate) fn layer(&self) -> &Layer {
        &self.layers()[self.cur_layer]
    }

    pub(crate) fn cel_here(&self) -> Option<CelId> {
        self.layer().cel_at(self.cur_frame)
    }

    pub(crate) fn frame_count(&self) -> usize {
        self.panel().frame_count()
    }

    /// Image courante dans le montage complet.
    pub(crate) fn global_frame(&self) -> usize {
        self.doc.panel_start(self.cur_panel) + self.cur_frame.min(self.frame_count() - 1)
    }

    pub(crate) fn canvas_size(&self) -> Vec2 {
        vec2(self.doc.width as f32, self.doc.height as f32)
    }

    /// Transformation du calque courant telle qu'on la voit à l'image courante : la sienne, plus le
    /// défilement multiplan si le calque est en profondeur et la caméra décalée.
    pub(crate) fn layer_seen(&self) -> crate::document::LayerTransform {
        let seen = self.doc.camera_seen(self.cur_panel, self.cur_frame);
        self.layer().transform_seen(&seen).unwrap_or(self.layer().transform)
    }

    /// Transformation du calque courant (de son dessin vers la feuille).
    pub(crate) fn layer_affine(&self) -> Affine {
        self.layer_seen().affine(self.doc.stage_center())
    }

    /// Point de la feuille → point du dessin du calque courant (là où le pinceau doit peindre).
    pub(crate) fn to_layer(&self, stage: Pos2) -> Pos2 {
        if self.layer_seen().is_identity() {
            return stage;
        }
        self.layer_affine().inverse().map_or(stage, |inv| inv.apply(stage))
    }

    /// Point du dessin du calque courant → écran.
    pub(crate) fn layer_to_screen(&self, p: Pos2) -> Pos2 {
        let stage = if self.layer_seen().is_identity() {
            p
        } else {
            self.layer_affine().apply(p)
        };
        self.view.to_screen(self.canvas_rect, self.canvas_size(), stage)
    }

    /// Pixels d'écran par pixel du dessin du calque courant.
    pub(crate) fn layer_zoom(&self) -> f32 {
        self.view.zoom * self.layer().transform.scale
    }

    pub(crate) fn modal_open(&self) -> bool {
        self.new_dialog.is_some()
            || self.pending.is_some()
            || self.export_dialog.is_some()
            || self.export_job.is_some()
            || !self.orphans.is_empty()
            || self.workspace_name.is_some()
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
    pub(crate) fn clamp_nav(&mut self) {
        self.cur_panel = self.cur_panel.min(self.doc.panels.len() - 1);
        self.cur_layer = self.cur_layer.min(self.layers().len() - 1);
    }

    // ------------------------------------------------------------------ composition

    /// Ce qu'il faut empiler pour afficher l'image `frame` du plan courant. `onion` : affichage
    /// de travail (pelure d'oignon et table lumineuse), sinon l'image telle qu'elle est.
    pub(crate) fn build_plan(&self, frame: usize, onion: bool, paper: bool) -> Plan {
        let mut items = Vec::new();
        // Table lumineuse : les autres calques sont estompés, le calque courant reste net.
        let washed = onion && self.light_table && !self.playing;
        // Multiplan : les calques en profondeur défilent selon la position de la caméra.
        let camera = self.doc.camera_seen(self.cur_panel, frame);
        for (li, layer) in self.layers().iter().enumerate() {
            if !layer.visible {
                continue;
            }
            let mut opacity = layer.opacity.clamp(0.0, 1.0);
            if washed && li != self.cur_layer {
                opacity *= self.light_table_strength.clamp(0.05, 1.0);
            }
            // Derrière la caméra (elle a avancé au-delà) : on ne le voit pas.
            let Some(seen) = layer.transform_seen(&camera) else { continue };
            let transform = (!seen.is_identity()).then_some(seen);
            if onion && self.onion.enabled && !self.playing && li == self.cur_layer {
                let s = &self.onion;
                let (before, after) = crate::onion::neighbours(layer, frame, s);
                // Sous le dessin courant, du plus lointain au plus proche (le plus proche au-dessus).
                for (list, n, color) in [(&before, s.before, s.before_color), (&after, s.after, s.after_color)] {
                    for &(cel, k) in list.iter().rev() {
                        items.push(PlanItem {
                            cel,
                            opacity: crate::onion::fade(s.opacity, k, n),
                            tint: s.tinted.then_some(color),
                            transform,
                            blend: Default::default(),
                        });
                    }
                }
            }
            if let Some(cel) = layer.cel_at(frame) {
                items.push(PlanItem {
                    cel,
                    transform,
                    opacity: (opacity * 255.0).round() as u8,
                    tint: None,
                    blend: layer.blend,
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
        let (panel, frame) = self.doc.locate(global);
        self.set_panel(panel);
        self.cur_frame = frame;
        self.follow_frame = true;
    }

    pub(crate) fn step_global(&mut self, delta: i64) {
        let total = self.doc.total_frames() as i64;
        let g = (self.global_frame() as i64 + delta).clamp(0, total - 1);
        self.goto_global(g as usize);
    }

    /// Change de plan courant (sans rien changer si c'est déjà lui).
    pub(crate) fn set_panel(&mut self, index: usize) {
        let index = index.min(self.doc.panels.len() - 1);
        if index == self.cur_panel {
            return;
        }
        self.end_stroke();
        self.cur_panel = index;
        self.cur_layer = self.cur_layer.min(self.layers().len() - 1);
        self.cur_frame = 0;
        self.vsel = None;
    }

    pub(crate) fn select_panel(&mut self, index: usize) {
        self.playing = false;
        self.set_panel(index);
        self.follow_frame = true;
    }

    /// Entre dans un plan pour l'animer.
    pub(crate) fn enter_animation(&mut self, index: usize) {
        self.select_panel(index);
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
            Mode::Storyboard => "Storyboard : montage des panneaux.".into(),
            Mode::Animation => format!("Animation de {} (Tab : retour au storyboard).", crate::storyboard::panel_long_title(&self.doc, self.cur_panel)),
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
            self.stop_audio();
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
                self.set_panel(0);
            }
            self.cur_frame = 0;
        }
        self.playing = true;
        self.last_time = None;
        self.play_accum = 0.0;
        self.start_audio_playback();
    }

    fn tick_playback(&mut self, ctx: &egui::Context) {
        if !self.playing {
            // La lecture a pu être arrêtée par n'importe quelle action : le son s'arrête avec.
            self.stop_audio();
            return;
        }
        if self.tick_audio_clock() {
            ctx.request_repaint();
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
            } else if self.play_montage && self.cur_panel + 1 < self.doc.panels.len() {
                let next = self.cur_panel + 1;
                self.set_panel(next);
            } else if self.looping {
                if self.play_montage {
                    self.set_panel(0);
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
        if self.layer().locked {
            let key = self.keymap.label("calque.verrouiller");
            self.status = format!("Ce calque est verrouillé ({key} pour le déverrouiller).");
            return false;
        }
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
        // On dessine : Suppr doit de nouveau viser le dessin, pas un extrait de son.
        self.sel_clip = None;
        if !self.layer_editable() {
            return;
        }
        let erase = self.tool == Tool::Eraser;
        if erase && self.cel_here().is_none() {
            self.guides.stroke = None;
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
        let textures = self.brushes.textures(&settings);
        let mut stroke = StrokeState::new(cel, mode, settings, prefix, pen).with_textures(textures);
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
                Some(vs) => {
                    // Les pixels d'avant l'aperçu servent à poser le trait définitif sans tout redessiner.
                    let preview = match edit {
                        Some(Edit::Tiles { tiles, .. }) => Some(tiles),
                        _ => None,
                    };
                    self.commit_vector_stroke(vs, std::mem::take(&mut stroke.path), preview)
                }
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
        if self.bdrag.is_some() {
            self.finish_bdrag();
        }
        self.finish_camera_drag();
        self.finish_layer_drag();
        self.finish_ref3d_drag();
        self.finish_stamps();
        self.guides.stroke = None;
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
        // Les contours de tous les calques sont pris sur la feuille : sur un calque transformé, ils
        // ne tomberaient pas au bon endroit de son dessin ; on prend alors le calque seul.
        if self.fill.sample_all && self.layer().transform.is_identity() {
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

    /// Comme `refresh_cel`, dans `region` seulement (là où quelque chose a changé) : le coût ne
    /// grandit plus avec le nombre de traits du dessin.
    pub(crate) fn refresh_cel_region(&mut self, cel: CelId, region: Rect) {
        if let Some(c) = self.doc.cels.get_mut(&cel) {
            let tiles = c.rerender_region(region.expand(2.0));
            self.renderer.invalidate_tiles(&tiles);
        }
    }

    pub(crate) fn resize_brush(&mut self, factor: f32) {
        if self.tool == Tool::Stamp {
            self.stamps.settings.size = (self.stamps.settings.size * factor).clamp(2.0, 1000.0);
        } else if self.tool == Tool::Clone {
            self.clone.size = (self.clone.size * factor).clamp(1.0, 500.0);
        } else if self.tool.is_shape() {
            self.shape.width = (self.shape.width * factor).clamp(0.5, 200.0);
        } else if let Some(b) = self.current_brush_mut() {
            b.size = (b.size * factor).clamp(0.5, 400.0);
        }
    }

    // ------------------------------------------------------------------ timeline et calques

    pub(crate) fn snapshot(&self) -> Edit {
        Edit::Structure {
            structure: self.doc.structure(),
            nav: Nav {
                panel: self.cur_panel,
                layer: self.cur_layer,
            },
        }
    }

    pub(crate) fn commit_structure(&mut self, snap: Edit) {
        self.history.push(snap);
        self.modified = true;
        self.renderer.invalidate_all();
    }

    /// Enregistre l'étape d'annulation d'un changement de réglage (nom, texte, opacité…), à
    /// appeler AVANT le changement. Un même geste (même champ ou curseur, sans autre étape
    /// entre-temps) n'en enregistre qu'une : taper une phrase ou glisser un curseur = une étape.
    pub(crate) fn note_change(&mut self, gesture: egui::Id) {
        let key = gesture.value();
        if self.gesture.needs_step(key, self.history.serial()) {
            let snap = self.snapshot();
            self.history.push(snap);
        }
        self.gesture.started(key, self.history.serial());
        self.modified = true;
    }

    /// Un clic qui change un réglage (œil d'un calque, piste muette…) : une étape à lui seul.
    pub(crate) fn record_step(&mut self) {
        let snap = self.snapshot();
        self.history.push(snap);
        self.gesture.reset();
        self.modified = true;
    }

    /// Suit un champ ou un curseur qui modifie une copie d'une valeur du document. Renvoie vrai si
    /// la copie vient de changer : l'étape d'annulation est alors enregistrée, et l'appelant
    /// reporte la nouvelle valeur dans le document.
    pub(crate) fn edited(&mut self, resp: &egui::Response) -> bool {
        // Le geste dure tant qu'on tape dans le champ, qu'on tire le curseur ou que la fenêtre de
        // choix (couleur) reste ouverte.
        let busy = resp.has_focus() || resp.dragged() || resp.ctx.memory(|m| m.any_popup_open());
        if busy {
            self.gesture.keep(resp.id.value());
        }
        if resp.changed() {
            self.note_change(resp.id);
            return true;
        }
        false
    }

    /// En début d'image : le geste est fini quand plus aucun champ ni glissé ne le prolonge.
    fn end_gesture_if_idle(&mut self) {
        self.gesture.end_frame(self.guides.drag.is_some());
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
        self.drop_bitmap_selection();
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
        self.drop_bitmap_selection();
        if self.layer().locked {
            self.layer_editable();
            return;
        }
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
        // La copie d'un calque de toute la scène est un calque du seul panneau.
        let layer = Layer {
            id: fresh_layer_id(),
            name: format!("{} copie", source.name),
            frames,
            scene_wide: None,
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
        // Un calque de toute la scène part de tous ses panneaux.
        match self.layer().scene_wide {
            Some(w) => self.doc.remove_scene_layer(w),
            None => {
                self.layers_mut().remove(at);
            }
        }
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

    /// Verrouille ou déverrouille le calque `li` du panneau courant (Alt+L : le calque courant).
    pub(crate) fn toggle_layer_lock(&mut self, li: usize) {
        if li >= self.layers().len() {
            return;
        }
        self.end_stroke();
        self.drop_bitmap_selection();
        self.vsel = None;
        self.record_step();
        let layer = &mut self.layers_mut()[li];
        layer.locked = !layer.locked;
        self.status = format!(
            "Calque « {} » {}.",
            layer.name,
            if layer.locked { "verrouillé" } else { "déverrouillé" }
        );
    }

    /// Copie le calque courant, avec ses dessins et sa timeline (W).
    pub(crate) fn copy_layer(&mut self) {
        let layer = self.layer().clone();
        let cels = layer
            .frames
            .iter()
            .flatten()
            .filter_map(|id| self.doc.cels.get(id).map(|c| (*id, c.clone())))
            .collect();
        self.status = format!("Calque « {} » copié : X pour le coller dans un autre panneau.", layer.name);
        self.layer_clipboard = Some(LayerClip {
            layer,
            cels,
            panel_len: self.frame_count(),
        });
    }

    /// Colle le calque copié au-dessus du calque courant (X), avec ses propres copies des dessins.
    /// Il est coupé à la durée du panneau ; un dessin tenu jusqu'à la fin du panneau d'origine
    /// reste tenu jusqu'à la fin de celui-ci.
    pub(crate) fn paste_layer(&mut self) {
        let Some(clip) = self.layer_clipboard.clone() else {
            self.status = "Aucun calque copié (W pour copier le calque courant).".into();
            return;
        };
        self.end_stroke();
        self.drop_bitmap_selection();
        let snap = self.snapshot();
        let mut copies: HashMap<CelId, CelId> = HashMap::new();
        let mut layer = Layer {
            id: fresh_layer_id(),
            scene_wide: None,
            ..clip.layer.clone()
        };
        for f in layer.frames.iter_mut() {
            *f = f.and_then(|old| {
                if let Some(&c) = copies.get(&old) {
                    return Some(c);
                }
                let cel = clip.cels.get(&old)?.clone();
                let c = self.doc.add_cel(cel);
                copies.insert(old, c);
                Some(c)
            });
        }
        let n = self.frame_count();
        let held = clip.layer.frames.len() >= clip.panel_len && clip.layer.frames.last().is_some_and(Option::is_some);
        if layer.frames.len() > n {
            layer.frames.truncate(n);
        } else if held {
            let last = layer.frames.last().copied().flatten();
            layer.frames.resize(n, last);
        }
        layer.trim();
        let name = layer.name.clone();
        let at = self.cur_layer + 1;
        self.layers_mut().insert(at, layer);
        self.cur_layer = at;
        self.vsel = None;
        self.commit_structure(snap);
        self.status = format!("Calque « {name} » collé.");
    }

    /// Fusionne le calque courant avec celui du dessous (Ctrl+E).
    pub(crate) fn merge_layer_down(&mut self) {
        self.end_stroke();
        self.drop_bitmap_selection();
        let (li, kinds) = (self.cur_layer, self.layers().get(self.cur_layer.wrapping_sub(1)).map(|b| (b.kind, self.layer().kind)));
        let snap = self.snapshot();
        match self.doc.merge_layer_down(self.cur_panel, li) {
            Ok(()) => {
                self.cur_layer = li - 1;
                self.vsel = None;
                self.commit_structure(snap);
                self.status = match kinds {
                    Some((LayerKind::Vector, LayerKind::Vector)) => "Calques fusionnés (vectoriel).".into(),
                    Some((LayerKind::Bitmap, LayerKind::Bitmap)) => "Calques fusionnés.".into(),
                    _ => "Calques fusionnés en un calque bitmap : les traits vectoriels ne sont plus modifiables.".into(),
                };
            }
            Err(e) => self.status = e,
        }
    }

    pub(crate) fn undo(&mut self) {
        self.cancel_montage_drag();
        self.end_stroke();
        // Une zone bitmap déplacée est d'abord reposée : l'annulation défait alors ce déplacement.
        self.drop_bitmap_selection();
        let mut nav = Nav {
            panel: self.cur_panel,
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
        self.drop_bitmap_selection();
        let mut nav = Nav {
            panel: self.cur_panel,
            layer: self.cur_layer,
        };
        if self.history.redo(&mut self.doc, &mut nav) {
            self.after_history(nav);
        } else {
            self.status = "Rien à rétablir.".into();
        }
    }

    fn after_history(&mut self, nav: Nav) {
        self.cur_panel = nav.panel;
        self.cur_layer = nav.layer;
        self.clamp_nav();
        self.guides.current = self.guides.current.min(self.doc.guides.len().saturating_sub(1));
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
                    width: self.doc.frame[0],
                    height: self.doc.frame[1],
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

    /// « Enregistrer sous » : on choisit un nom, Pelure crée le dossier du projet à ce nom.
    pub(crate) fn save_as(&mut self) -> bool {
        self.end_stroke();
        let name = format!("{}.pelure", self.base_name_or("Mon film"));
        let mut dialog = rfd::FileDialog::new()
            .set_title("Enregistrer le projet (un dossier à ce nom sera créé)")
            .add_filter("Projet Pelure", &["pelure"])
            .set_file_name(name);
        if let Some(dir) = self.import_source.as_ref().and_then(|p| p.parent()) {
            dialog = dialog.set_directory(dir);
        }
        let Some(chosen) = dialog.save_file() else {
            return false;
        };
        let (_, manifest) = project::target_for(&chosen);
        if let Err(e) = project::check_target(&manifest, self.path.as_deref()) {
            self.status = format!("Enregistrement annulé : {e}");
            return false;
        }
        if self.path.as_ref() != Some(&manifest) {
            // Nouveau dossier : tous les dessins y seront écrits.
            self.written.clear();
        }
        self.save_to(manifest)
    }

    fn save_to(&mut self, manifest: PathBuf) -> bool {
        match project::save(&self.doc, &manifest, &mut self.written) {
            Ok(report) => {
                self.status = format!(
                    "Enregistré : {} ({} dessin(s) écrit(s){}).",
                    manifest.display(),
                    report.written,
                    if report.removed > 0 {
                        format!(", {} inutilisé(s) effacé(s)", report.removed)
                    } else {
                        String::new()
                    }
                );
                self.path = Some(manifest);
                self.import_source = None;
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

    /// Ouvre un projet en dossier, ou importe un ancien fichier `.pelure`.
    pub(crate) fn open_path(&mut self, path: PathBuf) {
        if io::is_legacy_file(&path) {
            match io::load_legacy(&path) {
                Ok(mut doc) => {
                    // Comme un projet au format 3 : la feuille devient le double du cadre.
                    let (w, h) = (doc.width, doc.height);
                    doc.enlarge_stage(w * 2, h * 2);
                    doc.frame = [w, h];
                    self.load_doc(doc);
                    self.status = format!(
                        "Ancien format importé ({}). « Enregistrer » créera le dossier du projet.",
                        path.display()
                    );
                    self.import_source = Some(path);
                    self.modified = true;
                }
                Err(e) => self.status = format!("Impossible d'ouvrir le projet : {e}"),
            }
            return;
        }
        match project::load(&path) {
            Ok(loaded) => {
                self.load_doc(loaded.doc);
                self.written = loaded.written;
                self.status = if loaded.warnings.is_empty() {
                    format!("Ouvert : {}", path.display())
                } else {
                    format!("Ouvert avec {} problème(s) : {}", loaded.warnings.len(), loaded.warnings.join(" ; "))
                };
                if loaded.converted {
                    // Rien n'est réécrit sur le disque avant « Enregistrer ».
                    self.modified = true;
                    self.status.push_str(
                        " — Ancien format : la feuille de dessin a été agrandie autour du cadre (même cadrage). \
                         « Enregistrer » le passe au nouveau format.",
                    );
                }
                self.path = Some(path);
            }
            Err(e) => self.status = format!("Impossible d'ouvrir le projet : {e}"),
        }
    }

    pub(crate) fn load_doc(&mut self, doc: Document) {
        self.doc = doc;
        self.mode = Mode::Storyboard;
        self.cur_panel = 0;
        self.cur_layer = self.layers().len() - 1;
        self.cur_frame = 0;
        self.stroke = None;
        self.vstroke = None;
        self.verase = None;
        self.vsel = None;
        self.vdrag = None;
        self.bsel = None;
        self.bdrag = None;
        self.playing = false;
        self.stop_audio();
        self.sel_clip = None;
        self.menu_target = None;
        self.montage_drag = None;
        self.heard_frame = Some(0);
        self.history.clear();
        self.thumbs.clear();
        self.renderer.invalidate_all();
        self.fit_pending = true;
        // Vue du plateau et vue 3D recadrées sur le nouveau projet (9d, 9e).
        self.stage_view = None;
        self.stage3d.cam = None;
        self.modified = false;
        self.path = None;
        self.import_source = None;
        self.written.clear();
        self.length_origin = None;
    }

    pub(crate) fn base_name(&self) -> String {
        self.base_name_or("image")
    }

    /// Nom du projet (ou de l'ancien fichier importé), sinon `fallback`.
    pub(crate) fn base_name_or(&self, fallback: &str) -> String {
        self.path
            .as_ref()
            .or(self.import_source.as_ref())
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| fallback.into())
    }

    pub(crate) fn pick_folder(title: &str) -> Option<PathBuf> {
        rfd::FileDialog::new().set_title(title).pick_folder()
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
             .label{{font-weight:600}}h2.seq{{font-size:18px;margin:28px 0 10px;break-after:avoid}}\
             </style></head><body><h1>{title} — storyboard</h1>"
        );
        let ([w, h], fps) = (self.doc.frame, self.doc.fps);
        for (i, panel) in self.doc.panels.iter().enumerate() {
            // Intertitre au début de chaque séquence.
            if self.doc.sequence_run(i).0 == i {
                if i > 0 {
                    html += "</main>";
                }
                html += &format!(
                    "<h2 class=\"seq\">Séquence {}</h2><main>",
                    io::html_escape(&self.doc.sequence_label(self.doc.sequence_of(i)))
                );
            }
            let plan = render::panel_plan_at(&self.doc, i, 0, Some(self.doc.paper));
            let pixels = render::camera_image(&self.doc, &plan, &self.doc.panel_camera(i, 0));
            let file = format!("planche_{:03}.png", i + 1);
            if let Err(e) = io::write_png_scaled(&dir.join(&file), &pixels, w, h, 960) {
                self.status = format!("Échec de l'export : {e}");
                return;
            }
            html += &format!(
                "<section><img src=\"{file}\"><h2>{}</h2><p class=\"dur\">{}</p>",
                io::html_escape(&crate::storyboard::panel_long_title(&self.doc, i)),
                crate::storyboard::duration_label(panel.frame_count(), fps)
            );
            for (label, text) in [("Action", &panel.action), ("Dialogue", &panel.dialogue), ("Notes", &panel.notes)] {
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
        let cam = self.doc.panel_camera(self.cur_panel, self.cur_frame);
        let pixels = render::camera_image(&self.doc, &plan, &cam);
        self.status = match io::write_png(&path, &pixels, self.doc.frame[0], self.doc.frame[1]) {
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
            // Entrée, Échap, Retour arrière servent d'abord à l'outil en cours (forme à finir…).
            if !m.any() && self.tool_key(key) {
                continue;
            }
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
        // Un texte en cours de saisie est posé avant toute commande (annuler, changer d'image…).
        self.finish_text_edit();
        // Annuler ou rétablir pendant une déformation : elle repartirait d'un dessin périmé.
        if matches!(action, "edition.annuler" | "edition.retablir") {
            self.persp = None;
        }
        if self.workspace_action(action) {
            return;
        }
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
            "fichier.exporter_film" => self.open_export(crate::export::Format::Mp4),
            "fichier.exporter_images" => self.open_export(crate::export::Format::Png),
            "fichier.exporter_son" => self.open_export(crate::export::Format::Wav),
            "fichier.exporter_pdf" => self.open_export(crate::export::Format::Pdf),
            "fichier.exporter_image_courante" => self.export_frame(),
            "fichier.importer_modele3d" => self.import_model3d(),
            "edition.annuler" => self.undo(),
            "edition.retablir" => self.redo(),
            // Après un clic dans le montage (ou la planche), l'édition vise les panneaux choisis (12e).
            "edition.couper" if self.panels_have_focus() => self.cut_panels(),
            "edition.copier" if self.panels_have_focus() => self.copy_panels(),
            "edition.coller" if self.panels_have_focus() && self.panel_clipboard.is_some() => self.paste_panels(),
            "edition.couper" if self.bsel.is_some() => self.bitmap_cut(),
            "edition.couper" => self.cut_selection(),
            "edition.copier" if self.bsel.is_some() => {
                self.bitmap_copy();
            }
            "edition.copier" => {
                self.copy_selection();
            }
            "edition.coller" => {
                if self.layer().kind == LayerKind::Bitmap && self.pixel_clipboard.is_some() {
                    self.bitmap_paste();
                } else if self.layer().kind == LayerKind::Bitmap && !self.clipboard.is_empty() {
                    self.status = "Des éléments vectoriels se collent dans un calque vectoriel.".into();
                } else if self.layer().kind == LayerKind::Vector && self.clipboard.is_empty() && self.pixel_clipboard.is_some() {
                    self.status = "Une zone bitmap se colle dans un calque bitmap.".into();
                } else {
                    self.paste();
                }
            }
            "edition.supprimer" if self.board_shown() && !self.board_notes.is_empty() => self.delete_board_notes(),
            "edition.supprimer" => {
                if let Some(id) = self.sel_clip.filter(|&id| board && self.doc.clip(id).is_some()) {
                    self.delete_clip(id);
                } else if self.panels_have_focus() {
                    self.delete_chosen_panels();
                } else if self.bsel.is_some() {
                    self.bitmap_erase(true);
                } else if self.tool == Tool::Select && self.vsel.is_some() {
                    self.delete_selection();
                } else {
                    self.clear_image();
                }
            }
            "dessin.supprimer_hors_selection" => {
                if self.bsel.is_some() {
                    self.bitmap_erase(false);
                } else if self.vsel.is_some() {
                    self.delete_unselected();
                } else {
                    self.status = "Rien de sélectionné : rien à garder.".into();
                }
            }
            "edition.tout_selectionner" => self.select_all(),
            "edition.tout_deselectionner" => {
                self.end_stroke();
                self.vsel = None;
                self.drop_bitmap_selection();
            }
            // Sur la planche (vue Vignettes), le zoom et l'ajustement sont ceux de la planche.
            "vue.zoom_avant" if self.board_shown() => self.board_zoom_step(1.25),
            "vue.zoom_arriere" if self.board_shown() => self.board_zoom_step(1.0 / 1.25),
            "vue.ajuster" | "vue.reinit_zoom" if self.board_shown() => self.board_fit(),
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
            "outil.formes" => self.set_tool(self.shape.last),
            "outil.ligne" => self.set_tool(Tool::Line),
            "outil.ligne_droite" | "outil.ligne_courbe" | "outil.ligne_s" => {
                self.shape.line_mode = match action {
                    "outil.ligne_droite" => crate::shapes::LineMode::Straight,
                    "outil.ligne_courbe" => crate::shapes::LineMode::Curve,
                    _ => crate::shapes::LineMode::SCurve,
                };
                self.set_tool(Tool::Line);
            }
            "outil.texte" => self.set_tool(Tool::Text),
            "outil.tampon" => self.set_tool(Tool::Stamp),
            "outil.tampon_duplication" => self.set_tool(Tool::Clone),
            "outil.perspective" => self.set_tool(Tool::Perspective),
            "outil.rectangle" => self.set_tool(Tool::Rect),
            "outil.ellipse" => self.set_tool(Tool::Ellipse),
            "outil.polyligne" => self.set_tool(Tool::Polyline),
            "outil.main" => self.set_tool(Tool::Pan),
            "outil.taille_plus" => self.resize_brush(1.15),
            "outil.taille_moins" => self.resize_brush(1.0 / 1.15),
            "selection.miroir_h" if self.bsel.is_some() => self.bitmap_flip(true),
            "selection.miroir_v" if self.bsel.is_some() => self.bitmap_flip(false),
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
            "panneau.precedent" => self.select_panel(self.cur_panel.saturating_sub(1)),
            "panneau.suivant" => self.select_panel(self.cur_panel + 1),
            "panneau.nouveau" => self.new_panel(),
            "panneau.dupliquer" => self.duplicate_panel(),
            "scene.nouvelle" => self.new_scene(),
            "scene.dupliquer" => self.duplicate_scene(),
            "scene.supprimer" => self.delete_scene(),
            "scene.precedente" => self.step_scene(false),
            "scene.suivante" => self.step_scene(true),
            "sequence.nouvelle" => self.new_sequence(),
            "panneau.supprimer" => self.delete_chosen_panels(),
            "panneau.copier" => self.copy_panels(),
            "panneau.coller" => self.paste_panels(),
            "panneau.ajouter_image" => self.nudge_panel_length(1),
            "panneau.retirer_image" => self.nudge_panel_length(-1),
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
            "panneau.couper" => self.split_current_panel(),
            "panneau.fusionner_suivant" => self.merge_next_panel(),
            "scene.couper" => self.split_current_scene(),
            "scene.joindre_precedente" => self.join_previous_scene(),
            "calque.copier" => self.copy_layer(),
            "calque.coller" => self.paste_layer(),
            "calque.verrouiller" => self.toggle_layer_lock(self.cur_layer),
            "calque.fusionner_dessous" => self.merge_layer_down(),
            "calque.nouveau_fond" => self.add_scene_background(),
            "calque.toute_la_scene" => self.toggle_scene_wide(),
            "outil.camera" => self.set_tool(Tool::Camera),
            "outil.repere3d" => self.set_tool(Tool::Ref3d),
            "outil.transformer" => self.set_tool(Tool::Transform),
            "calque.reinit_transformation" => self.reset_layer_transform(),
            "camera.vue" => self.set_camera_view(!self.camera_view),
            "vue.cadres_securite" => self.layout.safe_frames = !self.layout.safe_frames,
            "vue.3d" => self.set_stage3d(!(self.stage3d.on && !self.camera_view)),
            "vue3d.recadrer" => {
                self.set_stage3d(true);
                self.fit_stage3d(35.0, 20.0);
            }
            "outil3d.orbite" | "outil3d.glisser" | "outil3d.avancer" | "outil3d.deplacer" => {
                if let Some(t) = crate::stage3dui::Nav3d::ALL.into_iter().find(|t| t.action() == action) {
                    if !self.stage3d.on || self.camera_view {
                        self.set_stage3d(true);
                    }
                    self.stage3d.tool = t;
                }
            }
            "camera.masque" => self.camera_mask = !self.camera_mask,
            "camera.reinitialiser" => self.reset_camera(),
            "camera.cle_debut" => self.add_camera_key(0),
            "camera.cle_ici" => self.add_camera_key(self.cur_frame),
            "camera.cle_fin" => self.add_camera_key(self.frame_count().saturating_sub(1)),
            "camera.cle_supprimer" => self.remove_camera_key_here(),
            "camera.cle_adoucir" => self.toggle_camera_key_smooth(),
            "vue.dessus" => self.top_view = !self.top_view,
            "vue.table_lumineuse" => {
                self.light_table = !self.light_table;
                self.status = format!("Table lumineuse : {}.", if self.light_table { "oui" } else { "non" });
            }
            "guide.activer" => {
                self.guides.enabled = !self.guides.enabled;
                if self.guides.enabled && self.doc.guides.is_empty() {
                    self.status = "Aucun guide dans ce projet : ajoutes-en un (Outils > Guides).".into();
                }
            }
            "guide.aligner" => {
                self.guides.align = !self.guides.align;
                if self.guides.align {
                    self.guides.enabled = true;
                }
                self.status = format!("Traits alignés sur le guide : {}.", if self.guides.align { "oui" } else { "non" });
            }
            "guide.verrouiller" => {
                self.guides.locked = !self.guides.locked;
                self.status = format!("Guide verrouillé : {}.", if self.guides.locked { "oui" } else { "non" });
            }
            "guide.lignes_reference" => self.guides.reference = !self.guides.reference,
            "guide.gomme" => self.guides.eraser = !self.guides.eraser,
            "guide.curseur_complet" => self.guides.full_cursor = !self.guides.full_cursor,
            "son.importer" => self.import_sound(None, None),
            "son.nouvelle_piste" => self.new_audio_track(),
            "son.couper" => self.split_sound_at_playhead(),
            "son.defilement" => {
                self.sound_scrub = !self.sound_scrub;
                self.status = format!("Son au défilement : {}.", if self.sound_scrub { "oui" } else { "non" });
            }
            "son.lecture" => {
                self.sound_on = !self.sound_on;
                self.status = format!("Son pendant la lecture : {}.", if self.sound_on { "oui" } else { "non" });
            }
            // « vue.pan_libre » se maintient : il est lu en continu par le canevas.
            "vue.pan_libre" => {}
            other => self.status = format!("« {other} » n'est pas encore disponible."),
        }
    }

    /// Touche pour l'outil en cours (voir `handle_shortcuts`) ; vrai si elle a servi.
    fn tool_key(&mut self, key: Key) -> bool {
        self.shape_key(key) || self.persp_key(key)
    }

    /// Change d'outil : la forme en cours (courbe, polyligne) est posée avant.
    pub(crate) fn set_tool(&mut self, tool: Tool) {
        if self.tool != tool {
            self.finish_shape_edit();
            self.finish_text_edit();
            self.persp = None;
            self.end_stroke();
            self.tool = tool;
            if tool.is_shape() {
                self.shape.last = tool;
            }
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
            Mode::Storyboard => self.goto_global(self.doc.panel_start(self.cur_panel) + frame),
            Mode::Animation => self.goto_frame(frame),
        }
    }

    /// Allonge ou raccourcit le panneau d'une image (une étape d'annulation par appui).
    fn nudge_panel_length(&mut self, delta: i64) {
        let n = (self.frame_count() as i64 + delta).max(1) as usize;
        if n == self.frame_count() {
            return;
        }
        self.end_stroke();
        let snap = self.snapshot();
        self.panel_mut().set_length(n);
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
        // Rythme des images : l'affichage n'attend plus la synchro de l'écran. Une image suit
        // chaque mouvement du stylet (500 par seconde au plus) ; sans entrée (lecture, export,
        // animations), 125 par seconde suffisent : la carte ne tourne pas pour rien.
        let input = ctx.input(|i| !i.events.is_empty());
        let min = std::time::Duration::from_micros(if input { 2_000 } else { 8_000 });
        if let Some(t) = self.frame_start {
            let elapsed = t.elapsed();
            if elapsed < min {
                std::thread::sleep(min - elapsed);
            }
        }
        self.frame_start = Some(std::time::Instant::now());
        self.ppp = ctx.pixels_per_point();
        self.clamp_nav();
        self.end_length_gesture_if_idle();
        self.end_gesture_if_idle();
        self.handle_close_request(ctx);
        self.handle_shortcuts(ctx);
        self.sync_scene_layers();
        if !self.modal_open() {
            self.handle_dropped_files(ctx);
        }
        self.tick_playback(ctx);
        if self.playing {
            let now = ctx.input(|i| i.time);
            let g = self.global_frame();
            self.play_meter.tick(now, g);
        } else {
            self.play_meter.reset();
        }
        self.validate_selection();
        self.validate_bitmap_selection();
        self.update_title(ctx);
        self.thumbs.prune(&self.doc);
        self.thumbs.pending = false;

        egui::TopBottomPanel::top("menu").show(ctx, |ui| self.menu_bar(ui));
        if self.layout.command_bar {
            egui::TopBottomPanel::top("commandes").show(ctx, |ui| self.command_bar(ui));
        }
        egui::TopBottomPanel::bottom("statut").show(ctx, |ui| self.status_bar(ui));
        if self.layout.tool_bar {
            egui::SidePanel::left("barre_outils")
                .exact_width(44.0)
                .resizable(false)
                .show(ctx, |ui| self.vertical_tool_bar(ui));
        }
        // Les panneaux : onglets que l'on glisse, aimante ou fait flotter (5b).
        self.show_dock(ctx);

        self.dialogs(ctx);
        self.top_view_window(ctx);
        // Export en cours : quelques images par image d'écran, la fenêtre reste vivante.
        if let Some(job) = &mut self.export_job {
            job.step(&self.doc, std::time::Duration::from_millis(40));
            if job.outcome.is_none() {
                ctx.request_repaint();
            }
        }
        self.export_windows(ctx);
        self.recovery_window(ctx);
        self.workspace_name_window(ctx);
        self.scrub_audio();
        if self.thumbs.pending {
            ctx.request_repaint();
        }
        // Réglages de la pelure : enregistrés quand ils changent (pas pendant un glissé).
        if self.onion != self.settings.onion && !ctx.input(|i| i.pointer.any_down()) {
            self.settings.onion = self.onion.clone();
            if let Err(e) = self.settings.save() {
                self.status = format!("Réglages non enregistrés : {e}");
            }
        }
        self.tick_recovery(ctx);
        self.remember_layout(ctx);
    }

    /// Fermeture normale (même sans enregistrer) : la copie de secours de la séance est effacée,
    /// la disposition de la fenêtre retenue.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(r) = self.recovery.take() {
            r.close();
        }
        self.settings.layout = Some(self.current_layout());
        let _ = self.settings.save();
    }
}

