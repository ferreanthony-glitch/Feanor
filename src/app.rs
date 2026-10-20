use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui::{self, vec2, Color32, Event, Key, Pos2, Rect, Vec2};

use crate::audio::AudioEngine;
use crate::brush::{BrushSettings, StrokeMode, StrokeState};
use crate::camtools::CamDrag;
use crate::layerops::Choose;
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
use crate::raster::{premultiply, unpremultiply, Tile, TILE};
use crate::render::{self, Plan, PlanItem, Renderer};
use crate::soundtrack::MenuTarget;
use crate::storyboard::MontageDrag;
use crate::thumbs::Thumbs;
use crate::vector::{Affine, Element, VectorDrawing};
use crate::view::View;
use crate::vtools::{VDrag, VSelection, VectorErase, VectorStroke};

/// Ce qu'on dit quand un outil bute sur un calque de position verrouillée (13b).
pub(crate) const POSITION_LOCKED: &str =
    "La position de ce calque est verrouillée : on ne peut ni le déplacer ni dessiner dessus.";

/// Ce qu'on dit quand un outil ou une commande de dessin vise un dossier (13b-5).
pub(crate) const FOLDER_NOT_DRAWABLE: &str = "Un dossier ne se dessine pas : choisis un de ses calques.";

/// Le mode des raccourcis (choix d'Anthony, 2026-10-17) : celui du panneau qui a la main. La
/// piste d'un calque dans le montage (`key_focus`, on y travaille les clés) ou la timeline :
/// animation ; le montage (ou la planche des vignettes) : storyboard ; sinon (dernier clic dans
/// le dessin, ou nulle part) : le mode du dessin.
pub(crate) fn key_mode_of(mode: Mode, panel_focus: bool, images_focus: bool, key_focus: bool) -> Mode {
    if key_focus {
        Mode::Animation
    } else if panel_focus {
        Mode::Storyboard
    } else if images_focus {
        Mode::Animation
    } else {
        mode
    }
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Brush,
    Pencil,
    Eraser,
    /// Cutter : coupe les traits vectoriels à leurs croisements (`cutter.rs`).
    Cutter,
    Fill,
    /// Formes (5c) : outils séparés, comme dans Storyboard Pro.
    Line,
    Rect,
    Ellipse,
    Polyline,
    Select,
    /// Sélection par couleur : la zone de la couleur cliquée (bitmap), ou tous les éléments de
    /// cette couleur (vectoriel). Même case que Sélection dans la barre d'outils.
    SelectColor,
    Picker,
    Pan,
    Camera,
    Transform,
    /// Repère 3D (2d) : tourner autour du modèle 3D du panneau.
    Ref3d,
    /// Texte (5d).
    Text,
    /// Déformer en perspective (5f).
    Perspective,
}

impl Tool {
    pub const ALL: [Tool; 18] = [
        Tool::Brush,
        Tool::Pencil,
        Tool::Eraser,
        Tool::Cutter,
        Tool::Fill,
        Tool::Line,
        Tool::Rect,
        Tool::Ellipse,
        Tool::Polyline,
        Tool::Text,
        Tool::Select,
        Tool::SelectColor,
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
            Tool::Cutter => "Cutter",
            Tool::Fill => "Remplir",
            Tool::Line => "Ligne",
            Tool::Rect => "Rectangle",
            Tool::Ellipse => "Ellipse",
            Tool::Polyline => "Polyligne",
            Tool::Select => "Sélection",
            Tool::SelectColor => "Sélection par couleur",
            Tool::Picker => "Pipette",
            Tool::Pan => "Main",
            Tool::Camera => "Caméra",
            Tool::Transform => "Transformer",
            Tool::Ref3d => "Repère 3D",
            Tool::Text => "Texte",
            Tool::Perspective => "Déformer",
        }
    }

    /// Petite description, pour la bulle d'aide de son bouton (12d).
    pub fn description(self) -> &'static str {
        match self {
            Tool::Brush => "Peindre ; la taille peut suivre la pression du stylet.",
            Tool::Pencil => "Dessiner au crayon.",
            Tool::Eraser => "Effacer ; sur un calque vectoriel, des bouts de trait ou des traits entiers.",
            Tool::Cutter => "Glisser en travers des bouts de traits vectoriels à enlever : ils sont coupés là où ils croisent les autres traits.",
            Tool::Fill => "Remplir une zone avec la couleur courante.",
            Tool::Line => "Tracer une droite, une courbe ou une courbe en S.",
            Tool::Rect => "Tracer un rectangle, vide ou plein.",
            Tool::Ellipse => "Tracer une ellipse, vide ou pleine.",
            Tool::Polyline => "Tracer une suite de segments, clic après clic.",
            Tool::Select => "Choisir au lasso, au rectangle ou au clic, puis transformer avec la boîte : glisser dedans pour déplacer, un coin ou un côté pour mettre à l'échelle, juste hors d'un coin pour tourner, un bord pour incliner, le point rond pour changer le pivot.",
            Tool::SelectColor => "Clic sur une couleur : la zone de cette couleur (bitmap), ou tous les traits et zones de cette couleur (vectoriel). Maj : ajouter.",
            Tool::Picker => "Prendre une couleur sur le dessin.",
            Tool::Pan => "Déplacer la vue.",
            Tool::Camera => "Régler le cadre de la caméra du plan : déplacer, zoomer par un coin, tourner.",
            Tool::Transform => "Déplacer, agrandir ou tourner le calque entier (tous ses dessins).",
            Tool::Ref3d => "Un modèle 3D en fil de fer comme guide, vu depuis le cadre du plan.",
            Tool::Text => "Clic : poser un texte ; Ctrl + Entrée pour le valider.",
            Tool::Perspective => "Déformer le dessin, la sélection ou la zone choisie : en enveloppe de Bézier (coins et poignées pour courber les côtés) ou en perspective (les quatre coins). Le mode se choisit dans Réglages.",
        }
    }

    /// Icône de l'outil (police d'icônes Phosphor, chargée au démarrage).
    pub fn icon(self) -> &'static str {
        use egui_phosphor::regular as ph;
        match self {
            Tool::Brush => ph::PAINT_BRUSH,
            Tool::Pencil => ph::PENCIL_SIMPLE,
            Tool::Eraser => ph::ERASER,
            Tool::Cutter => ph::KNIFE,
            Tool::Fill => ph::PAINT_BUCKET,
            Tool::Line => ph::LINE_SEGMENT,
            Tool::Rect => ph::RECTANGLE,
            Tool::Ellipse => ph::CIRCLE,
            Tool::Polyline => ph::LINE_SEGMENTS,
            Tool::Select => ph::LASSO,
            Tool::SelectColor => ph::MAGIC_WAND,
            Tool::Picker => ph::EYEDROPPER,
            Tool::Pan => ph::HAND,
            Tool::Camera => ph::VIDEO_CAMERA,
            Tool::Transform => ph::BOUNDING_BOX,
            Tool::Ref3d => ph::CUBE,
            Tool::Text => ph::TEXT_T,
            Tool::Perspective => ph::PERSPECTIVE,
        }
    }

    /// Action qui choisit cet outil (son raccourci est dans le fichier de raccourcis).
    pub fn action(self) -> &'static str {
        match self {
            Tool::Brush => "outil.pinceau",
            Tool::Pencil => "outil.crayon",
            Tool::Eraser => "outil.gomme",
            Tool::Cutter => "outil.cutter",
            Tool::Fill => "outil.remplissage",
            Tool::Line => "outil.ligne",
            Tool::Rect => "outil.rectangle",
            Tool::Ellipse => "outil.ellipse",
            Tool::Polyline => "outil.polyligne",
            Tool::Select => "outil.selection",
            Tool::SelectColor => "outil.selection_couleur",
            Tool::Picker => "outil.pipette",
            Tool::Pan => "outil.main",
            Tool::Camera => "outil.camera",
            Tool::Transform => "outil.transformer",
            Tool::Ref3d => "outil.repere3d",
            Tool::Text => "outil.texte",
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

    /// Un des outils de sélection : la sélection reste affichée et modifiable avec lui.
    pub fn is_select(self) -> bool {
        matches!(self, Tool::Select | Tool::SelectColor)
    }

    /// Une zone bitmap choisie reste avec cet outil (Anthony, 2026-10-18, axe d'ergonomie 6 : elle
    /// survit au passage sur Transformer, qui la déplace, l'agrandit, la tourne, et sur Déformer,
    /// dont l'enveloppe ne déforme alors qu'elle).
    pub fn keeps_zone(self) -> bool {
        self.is_select() || matches!(self, Tool::Transform | Tool::Perspective)
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
    /// Nouveau projet fait des plans choisis (13e).
    FromChosen,
    /// Import d'un projet Storyboard Pro (14a), qui remplace le projet ouvert.
    SbImport,
    /// Fermer le projet (Ctrl+W) : un projet vide du même format le remplace.
    Close,
    /// Ouvrir un projet récent (`App::recent_to_open`).
    OpenRecent,
    /// Importer un scénario dans un nouveau projet (`scenarioui.rs`), qui remplace le projet ouvert.
    ScenarioNew,
}

/// Calque copié (W) : sa timeline, ses dessins, et la durée du panneau d'où il vient.
#[derive(Clone)]
pub(crate) struct LayerClip {
    pub layer: Layer,
    /// Calques du dossier copié (13b-5), du plus bas au plus haut ; vide pour un calque.
    pub children: Vec<Layer>,
    /// Calques de couleur du calque copié (`calquecouleur.rs`) : ils sont collés avec lui.
    pub colours: Vec<Layer>,
    pub cels: HashMap<CelId, crate::document::Cel>,
    pub panel_len: usize,
}

pub(crate) use crate::nouveauui::NewDocDialog;

pub struct App {
    pub(crate) ctx: egui::Context,
    pub(crate) doc: Document,
    pub(crate) mode: Mode,
    pub(crate) board_view: BoardView,
    /// Planche de crayonnés rapides (`crayonnesui.rs`) : ouverte, elle prend la place du dessin en
    /// storyboard ; la planche elle-même est dans le projet (`Document::crayonnes`).
    pub(crate) crayonnes: crate::crayonnesui::Etat,
    pub(crate) cur_panel: usize,
    pub(crate) cur_layer: usize,
    pub(crate) cur_frame: usize,
    pub(crate) tool: Tool,
    pub(crate) brush: BrushSettings,
    pub(crate) pencil: BrushSettings,
    pub(crate) eraser: BrushSettings,
    /// Pointes, papiers et réglages de pinceau (ceux de SHOT et les importés).
    pub(crate) brushes: crate::brushtex::BrushLibrary,
    /// La bibliothèque complète (pinceaux importés compris) en cours de lecture sur un autre fil
    /// (`brushui::poll_brushes`) : tant qu'elle n'est pas là, `brushes` ne contient que les pinceaux
    /// de SHOT, et rien ne modifie ni n'enregistre la bibliothèque (`brushes_busy`).
    pub(crate) brushes_job: Option<std::thread::JoinHandle<crate::brushtex::BrushLibrary>>,
    /// Le dernier projet du lancement : sa présence se vérifie sur un autre fil (`recents.rs`), un
    /// disque réseau éteint ne gèle pas la première image.
    pub(crate) reopen_probe: Option<(PathBuf, std::sync::mpsc::Receiver<bool>)>,
    pub(crate) fill: FillSettings,
    pub(crate) shape: ShapeSettings,
    pub(crate) vector_erase_whole: bool,
    /// Options du pinceau et du crayon (`brushfill`) : peint derrière, remplir la forme.
    pub(crate) paint_behind: bool,
    pub(crate) fill_shapes: bool,
    /// Toucher du stylet (sensation des brosses) : la copie de travail du réglage, enregistrée
    /// dans `update` quand elle change (hors glissé), comme les options du pinceau.
    pub(crate) toucher: crate::brush::Toucher,
    /// État du panneau Pinceaux retenu d'une séance à l'autre (onglet, liste ou vignettes, dossiers).
    pub(crate) pinceaux: crate::pinceaux::Panneau,
    pub(crate) color: [u8; 3],
    /// La couleur en teinte, saturation, valeur (panneau Couleur, 6b).
    pub(crate) hsv: [f32; 3],
    /// Couleur d'arrière-plan, nuanciers et réglages du panneau Couleur (passe couleur,
    /// `colorui.rs`) ; `color` reste la couleur d'avant-plan, celle des outils.
    pub(crate) couleurs: crate::colorui::ColorState,
    pub(crate) view: View,
    pub(crate) fit_pending: bool,
    pub(crate) canvas_rect: Rect,
    pub(crate) ppp: f32,
    /// Pelure d'oignon (réglage de l'application, retenu dans `settings`).
    pub(crate) onion: crate::onion::OnionSettings,
    /// Panneau de la pelure ouvert ; décalage temporaire de ses images (sur la feuille) et mode
    /// « Décaler » (glisser sur le dessin les déplace) ; nom proposé pour ranger (13f).
    pub(crate) onion_window: bool,
    pub(crate) onion_offset: Vec2,
    pub(crate) onion_shift: bool,
    pub(crate) onion_preset_name: String,
    /// Feuille d'exposition (13g) : ce qu'elle montre (retenu dans `settings`), et la dernière
    /// image courante qu'elle a suivie.
    pub(crate) xsheet: crate::xsheet::XSheetSettings,
    pub(crate) xsheet_seen_frame: Option<usize>,
    /// Rôles des images (13g-2) : affichage dans la timeline ; menu du clic droit ouvert sur
    /// une case de calque (timeline, feuille d'exposition).
    pub(crate) roles_view: crate::rolesui::RolesView,
    pub(crate) role_menu: bool,
    pub(crate) xsheet_role_menu: bool,
    /// Flips (13h, `flipsui.rs`) : la liste (retenue dans `settings`), le flip en cours, le
    /// départ et la dernière image d'un flip manuel, le panneau ouvert.
    pub(crate) flips: crate::flips::FlipSettings,
    pub(crate) flip_run: Option<crate::flipsui::FlipRun>,
    pub(crate) flip_manual: Option<(usize, usize)>,
    pub(crate) flips_window: bool,
    /// Pile d'effets (13i, `fxui.rs`, panneau Effets) : création automatique des clés, clés
    /// montrées dans la timeline (interrupteur général), effet choisi (plan, calque, rang) : ses
    /// clés seules se montrent dans la timeline et le montage ; aucun choisi, aucun losange.
    pub(crate) fx_auto_key: bool,
    pub(crate) fx_keys_shown: bool,
    pub(crate) fx_choice: Option<crate::fxkeys::FxSel>,
    /// Clés d'effets choisies dans le montage ou la timeline (`fxkeys.rs`) et la dernière cliquée
    /// (départ des Maj + clic), clés copiées (`fx_clip_recent` : la dernière copie est la leur,
    /// Ctrl+V les vise), clés prises à la souris.
    pub(crate) fx_key_sel: Vec<crate::fxkeys::FxKeyRef>,
    pub(crate) fx_key_anchor: Option<crate::fxkeys::FxKeyRef>,
    pub(crate) fx_key_clipboard: Option<crate::fxkeys::FxClip>,
    pub(crate) fx_clip_recent: bool,
    pub(crate) fx_keys_drag: Option<crate::fxkeys::FxKeysDrag>,
    /// Effets copiés d'un calque (`fxcopie.rs`), et le panneau Effets a la main (le dernier clic
    /// était dedans : Ctrl+C et Ctrl+V visent ses effets).
    pub(crate) fx_stack_clipboard: Option<crate::fxcopie::FxStackClip>,
    pub(crate) fx_pane_focus: bool,
    /// Import vidéo en cours (13k, `videoui.rs`).
    pub(crate) video_import: Option<crate::videoui::VideoImport>,
    /// Propagation d'une peinture sur la vidéo, façon EbSynth (`propagerui.rs`).
    pub(crate) propagation: crate::propagerui::Propagation,
    /// Coloriage par zones : propager des aplats de couleur d'une image à l'autre (`coloriageui.rs`).
    pub(crate) coloriage: crate::coloriageui::Coloriage,
    /// Fenêtre d'import d'un projet Storyboard Pro et import en cours (14a, `sbui.rs`).
    pub(crate) sb_window: Option<crate::sbui::SbWindow>,
    /// Fenêtre « Importer un scénario » (`scenarioui.rs`).
    pub(crate) scenario_window: Option<crate::scenarioui::ScenarioWindow>,
    /// Fenêtre « Exporter vers Storyboard Pro » (14b, `sbexportui.rs`).
    pub(crate) sb_export: Option<crate::sbexportui::SbExportWindow>,
    /// Fenêtre d'interpolation ouverte (13l, `tweenui.rs`).
    pub(crate) tween_dialog: Option<crate::tweenui::TweenDialog>,
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
    /// Calque en cours de renommage (9b) : id, nom tapé, saisie déjà activée, où.
    pub(crate) layer_rename: Option<crate::layersui::LayerRename>,
    /// Calque que l'on fait glisser dans l'en-tête de la timeline pour changer son rang (13b).
    pub(crate) head_drag: Option<usize>,
    /// Calque que l'on fait glisser dans le panneau Calques (passe interface).
    pub(crate) panel_layer_drag: Option<usize>,
    /// Calque que l'on fait glisser dans les noms des pistes du montage (passe interface).
    pub(crate) montage_layer_drag: Option<usize>,
    /// Calques choisis en plus du calque courant (ids, Ctrl + clic dans l'en-tête, 13b).
    pub(crate) layer_sel: Vec<u64>,
    /// Fenêtre des groupes de calques ouverte, et n'y montrer que les groupes utilisés (13b-4).
    pub(crate) groups_window: bool,
    pub(crate) groups_used_only: bool,
    /// Images choisies [début, fin) sur les calques choisis (13c-2), images copiées, trou
    /// laissé par une suppression, et dernier clic dans la grille de la timeline.
    pub(crate) frame_sel: Option<(usize, usize)>,
    pub(crate) image_clipboard: Option<crate::images::ImageClip>,
    /// Clés copiées dans le montage (`montagekeys`).
    pub(crate) key_clipboard: Option<crate::montagekeys::KeyClip>,
    /// Clés choisies dans le montage, et la dernière cliquée (départ des Maj + clic).
    pub(crate) key_sel: Vec<crate::montagekeys::KeyRef>,
    pub(crate) key_anchor: Option<crate::montagekeys::KeyRef>,
    /// Plusieurs clés choisies glissées à la fois dans le montage.
    pub(crate) keys_drag: Option<crate::montagekeys::KeysDrag>,
    /// Outil pris par une touche encore tenue, et lasso tant que Ctrl est tenu (`springtools`).
    pub(crate) tool_spring: Option<crate::springtools::ToolSpring>,
    pub(crate) ctrl_lasso: Option<crate::springtools::CtrlLasso>,
    /// Choix de clés au rectangle en cours dans le montage.
    pub(crate) key_rect: Option<crate::montagekeys::KeyRect>,
    /// Une clé seule prise dans le montage (`Handle::Key`) : de combien d'images elle est glissée.
    /// Elle ne se pose qu'au lâcher ; d'ici là, un repère montre où (`montagekeys::drop_view`).
    pub(crate) key_drag_delta: isize,
    /// Le geste de cette clé seule (Maj, Alt ou rien tenu à la dernière image d'écran).
    pub(crate) key_drag_mode: crate::handles::KeyMove,
    /// Le dernier clic dans le montage était sur la piste d'un calque : copier, coller et
    /// supprimer visent la clé à l'image courante du calque courant.
    pub(crate) key_focus: bool,
    /// Commentaires sur la timeline (`commentaires.rs`, `commentairesui.rs`) : le commentaire
    /// choisi, le repère pris à la souris, le filtre des résolus.
    pub(crate) comment_ui: crate::commentaires::CommentUi,
    /// Le panneau Références (`references.rs`, `referencesui.rs`) : le tableau, la sélection, les
    /// imports en cours, le cache des images en mémoire graphique.
    pub(crate) refs: crate::referencesui::RefUi,
    pub(crate) hole: crate::images::Hole,
    pub(crate) images_focus: bool,
    /// Faux-fixe : nombre de fois que les images choisies sont jouées en tout (13c).
    pub(crate) fake_hold_times: usize,
    /// Glisser d'images choisies en cours : colonne du clic, colonne sous la souris (13c).
    pub(crate) image_drag: Option<(usize, usize)>,
    /// Choix d'images au rectangle en cours dans la grille de la timeline (chantier 3).
    pub(crate) grid_select: Option<crate::handles::GridSelect>,
    /// Plan courant (id) et sa durée à l'image d'écran d'avant : un changement de durée qui
    /// décale les plans suivants par rapport au son est signalé (chantier 4).
    pub(crate) plan_len_seen: Option<(u64, usize)>,
    /// Instance en cours de renommage dans la timeline, fenêtre du renommage intelligent (13c).
    pub(crate) instance_rename: Option<crate::images::InstanceRename>,
    pub(crate) smart_rename: Option<crate::images::SmartRename>,
    /// Numérotation des images dans la timeline : image, time code ou trame (13d).
    pub(crate) time_display: crate::cues::TimeDisplay,
    /// Zoom de la timeline (largeur d'une image), « Adapter » en attente (images [a, b) à faire
    /// tenir), croix sur les instances vides, miniatures dans les blocs (13d).
    pub(crate) timeline_cell: f32,
    pub(crate) timeline_fit: Option<(usize, usize)>,
    pub(crate) empty_x: bool,
    pub(crate) timeline_thumbs: bool,
    /// Marques de couleur (13d) : couleur choisie, « Répéter », « Continuer sur le calque
    /// suivant », seulement les couleurs utilisées.
    pub(crate) mark_color: u8,
    pub(crate) mark_repeat: bool,
    pub(crate) mark_next_layer: bool,
    pub(crate) mark_used_only: bool,
    /// Onglet du bas ouvert dans la timeline, damier de l'arrière-plan, notes dessinées (13e).
    pub(crate) bottom_tab: Option<crate::bottomtabs::BottomTab>,
    pub(crate) checker: crate::bottomtabs::Checker,
    pub(crate) notes: crate::notes::NotesState,
    /// Extrait son glissé depuis la timeline (Ctrl + glisser l'onde, 13e) : sa piste.
    pub(crate) timeline_sound_track: Option<usize>,
    /// Fenêtre « Réglages du projet » ouverte (13e, `projetui.rs`), son onglet, et la résolution en
    /// cours de réglage.
    pub(crate) properties_window: bool,
    pub(crate) settings_tab: crate::projetui::SettingsTab,
    pub(crate) res_edit: Option<crate::projetui::ResolutionEdit>,
    /// Aperçus des réglages de pinceau (grille du panneau Réglages, 6d).
    pub(crate) brush_previews: crate::brushui::BrushPreviews,
    /// Nom d'un réglage de pinceau en cours de saisie (« Mes brosses », moteur de dessin, étape 9).
    pub(crate) preset_edit: Option<crate::brushui::PresetEdit>,
    /// Remarques du dernier import de brosses : le fichier, ce qui n'a pas été repris.
    pub(crate) brush_import_notes: Option<(String, Vec<String>)>,
    /// Banc de traits (moteur de dessin, étape 1) : chaque trait est gardé, tel que reçu ; le
    /// nombre de traits enregistrés depuis l'allumage.
    pub(crate) record_strokes: Option<usize>,
    /// Repère 3D : geste en cours, fil de fer calculé (image) et calcul en cours.
    pub(crate) ref3d: crate::ref3dtool::Ref3dState,
    /// Vue à travers la caméra du plan (sinon : la feuille, avec le cadre tracé).
    pub(crate) camera_view: bool,
    /// Vue du plateau laissée en passant à la vue caméra, remise au retour (9d).
    pub(crate) stage_view: Option<View>,
    /// Cadre de format personnalisé choisi (même s'il vaut l'un des formats proposés).
    pub(crate) format_custom: bool,
    /// Plateau en 3D (9e) : affiché, outil, vue, geste en cours, textures.
    pub(crate) stage3d: crate::stage3dui::Stage3dState,
    /// Scène 3D de blocage (`scene3dui.rs`) : affichée, objets choisis, vue, envoi vers un calque.
    pub(crate) scene3d: crate::scene3dui::Scene3dState,
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
    /// Réglages de l'application (%APPDATA%\SHOT\reglages.toml).
    pub(crate) settings: crate::settings::Settings,
    /// Dernière disposition appliquée (espace de travail) ; les panneaux eux-mêmes, que l'on
    /// glisse et aimante, sont dans `dock` (affiché par egui_dock).
    pub(crate) layout: crate::workspace::Layout,
    pub(crate) dock: Option<egui_dock::DockState<crate::dock::Pane>>,
    /// Panneaux dans leur propre fenêtre Windows (5g), et celui à y mettre (clic droit sur
    /// son onglet).
    pub(crate) detached: Vec<crate::dock::DetachedPanel>,
    pub(crate) detach_request: Option<crate::dock::Pane>,
    /// La disposition d'animation dont la feuille d'exposition a été retirée en passant au
    /// storyboard (elle n'existe qu'en animation) ; elle revient à sa place au retour.
    pub(crate) exposition_masquee: Option<crate::dock::DockLayout>,
    /// L'espace de travail où l'on est (Storyboard, Animation ou Montage) : sa disposition est celle
    /// que `remember_layout` retient. Storyboard et Montage sont deux dispositions du mode
    /// storyboard (`workspace::Espace`).
    pub(crate) espace: crate::workspace::Espace,
    /// Des réglages (la disposition d'un espace qu'on vient de quitter, l'espace du côté storyboard)
    /// attendent d'être écrits : `remember_layout` s'en charge à l'image d'écran suivante.
    pub(crate) settings_a_ecrire: bool,
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
    /// Transitions du montage : choix, geste en cours, aperçu (`transitionsui.rs`).
    pub(crate) trans: crate::transitionsui::TransUi,
    /// Les envois : suivi des plans modifiés depuis le dernier, fenêtres (`envoisui.rs`).
    pub(crate) envois_ui: crate::envoisui::EnvoisUi,
    /// La fenêtre « Comparer » : deux côtés d'un même plan, en lecture seule (`comparerui.rs`).
    pub(crate) comparaison: Option<crate::comparerui::Comparaison>,
    /// La pile des versions d'un plan, ouverte en survolant « V2 » sur son bloc du montage, et la
    /// confirmation de « Supprimer les versions non affichées » (`pileversui.rs`).
    pub(crate) pile_versions: crate::pilevers::Pile,
    /// Quand le projet change, le plan dont on propose de supprimer les versions non affichées : le
    /// plan seul (`Some`) ou tout le film (`None`) ; rien : pas de confirmation en cours.
    pub(crate) purge_versions: Option<Option<u64>>,
    /// Effets des extraits de son : clé de volume choisie (`audiofxui.rs`).
    pub(crate) sfx: crate::audiofxui::SoundFxUi,
    /// La bibliothèque de bruitages : liste, recherche, écoute, sons en cours de pose (`bruitagesui.rs`).
    pub(crate) brt: crate::bruitagesui::BruitagesUi,
    /// Les suggestions sonores : portée, tâches en cours, fournisseurs (`suggestionsui.rs`).
    pub(crate) sug: crate::suggestionsui::SuggestionsUi,
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
    /// Feuille flottante de la sélection vectorielle qu'on déplace, tourne ou agrandit
    /// (`flottante.rs`).
    pub(crate) flottante: Option<crate::flottante::Flottante>,
    /// Contours à l'écran de la sélection vectorielle, gardés d'une image à l'autre
    /// (`contours.rs`) ; dans une `RefCell` : la peinture n'a que `&self`.
    pub(crate) sel_contours: std::cell::RefCell<Option<crate::contours::Contours>>,
    /// Sélection sur plusieurs calques choisis, son geste et ses feuilles flottantes
    /// (`multisel.rs`, `multitools.rs`).
    pub(crate) multi: crate::multisel::Multi,
    /// Dessins dont le rendu exact est à redemander dès qu'aucun geste n'y travaille (`rejeu.rs`).
    pub(crate) rejeu_again: std::collections::BTreeSet<CelId>,
    /// Pour chaque dessin vectoriel regardé, la version dont on a vérifié que tous les traits
    /// étaient en cache (`App::warm_current_cel`).
    pub(crate) rejeu_warm: std::collections::HashMap<CelId, u64>,
    /// Rendus exacts arrivés pendant un geste sur leur dessin, adoptés à la fin du geste.
    pub(crate) rejeu_deferred: Vec<crate::rejeu::Done>,
    /// Dessins ouverts en rendu rapide dont le rendu exact attend le repos (ou l'image affichée),
    /// d'après le réglage « traits vectoriels texturés » (`diagnostic.rs`).
    pub(crate) rejeu_idle: Vec<CelId>,
    /// Dernier instant d'activité de l'utilisateur ou de la lecture, en secondes d'egui.
    pub(crate) activity: f64,
    /// Réglages de performance posés dans le programme (`App::appliquer_performance`).
    pub(crate) perf_applied: crate::diagnostic::PerfReglages,
    /// Fenêtre « Diagnostic des performances » (`diagnosticui.rs`).
    pub(crate) diag: crate::diagnosticui::DiagnosticUi,
    /// Sélection dans un dessin bitmap, et geste en cours avec l'outil Sélection.
    pub(crate) bsel: Option<BSelection>,
    pub(crate) bdrag: Option<BDrag>,
    /// La boîte de la sélection, gardée d'une image à l'autre (rotation, pivot : `seltools.rs`).
    pub(crate) sel_box: Option<crate::seltools::SelBox>,
    /// « Garder les proportions » : la mise à l'échelle de la boîte garde la forme.
    pub(crate) keep_proportions: bool,
    /// Un champ chiffré de la boîte (Réglages) a la main : son geste continue (`seltools.rs`).
    pub(crate) field_edit_alive: bool,
    /// Mode de l'outil Déformer : enveloppe de Bézier ou perspective.
    pub(crate) deform_mode: crate::envelopetools::DeformMode,
    /// Sélection bitmap au rectangle (sinon au lasso).
    pub(crate) select_rect: bool,
    /// Zone bitmap copiée.
    pub(crate) pixel_clipboard: Option<Floating>,
    /// Calque copié (W / X).
    pub(crate) layer_clipboard: Option<LayerClip>,
    /// Les murs du trait que colore le calque de couleur courant (`calquecouleur.rs`).
    pub(crate) murs: crate::calquecouleur::Cache,
    /// Panneaux copiés (12e).
    pub(crate) panel_clipboard: Option<crate::document::PanelClip>,
    /// Le montage (ou la planche des vignettes) a la main : on y a cliqué en dernier, pas dans le
    /// dessin. Copier, coller et supprimer y visent alors les panneaux choisis (12e).
    pub(crate) panel_focus: bool,
    /// Clés de caméra choisies dans le montage ou la timeline (`camkeys.rs`) et la dernière
    /// cliquée (départ des Maj + clic), clés copiées (`cam_clip_recent` : la dernière copie est la
    /// leur, Ctrl+V les vise), clés prises à la souris (12e), et les réglages de l'effet de caméra
    /// choisi dans les réglages de la caméra (`camfxui.rs`).
    pub(crate) cam_key_sel: Vec<crate::camkeys::CamKeyRef>,
    pub(crate) cam_key_anchor: Option<crate::camkeys::CamKeyRef>,
    pub(crate) cam_key_clipboard: Option<crate::camkeys::CamClip>,
    pub(crate) cam_clip_recent: bool,
    pub(crate) cam_keys_drag: Option<crate::camkeys::CamKeysDrag>,
    /// Choix au rectangle des clés de caméra en cours (glisser depuis un espace vide de la piste).
    pub(crate) cam_rect: Option<crate::camkeys::CamRect>,
    pub(crate) cam_fx: crate::camfx::Params,
    /// Aides au cadrage de la caméra (tiers, grille d'or, spirale…, `cadrage.rs`) : les choix,
    /// retenus dans `reglages.toml`, et leur géométrie du dernier cadre.
    pub(crate) cadrage: crate::cadrage::FramingAids,
    pub(crate) cadrage_cache: std::cell::RefCell<crate::cadrage::Cache>,
    /// Poignée tenue dans la timeline du plan (passe interface, `handles.rs`).
    pub(crate) handle_drag: Option<crate::handles::HandleDrag>,
    /// Menu du dernier clic droit dans la timeline hors des cases (`handles::GridMenu`).
    pub(crate) grid_menu: Option<crate::handles::GridMenu>,
    pub(crate) shape_drag: Option<(Pos2, Pos2)>,
    /// Forme en plusieurs gestes : ligne à courber, polyligne en cours (5c).
    pub(crate) shape_edit: Option<crate::shapes::ShapeEdit>,
    /// Texte en cours de saisie, police et taille des nouveaux textes, polices du poste (5d).
    pub(crate) text_edit: Option<crate::texttools::TextEditState>,
    pub(crate) text_font: String,
    pub(crate) text_size: f32,
    pub(crate) text_fonts: Option<Vec<String>>,
    pub(crate) text_font_filter: String,
    /// Sélection par couleur : tolérance, pixels voisins seulement.
    pub(crate) color_select: crate::colorsel::ColorSelectSettings,
    /// Outils pris, du plus récent au plus ancien (cases de la barre d'outils), et case dont les
    /// outils sont affichés (groupe, case, rectangle de la case).
    pub(crate) tool_recent: Vec<Tool>,
    pub(crate) tool_flyout: Option<(usize, usize, Rect)>,
    /// Un Ctrl+V a donné un événement de collage (texte) depuis l'appui sur V (`pasteimage`).
    pub(crate) paste_event: bool,
    /// Messages : historique, dernier vu, notifications, fenêtre ouverte (`messages.rs`).
    pub(crate) messages: Vec<(f64, String)>,
    pub(crate) last_status: String,
    pub(crate) toasts: Vec<(f64, String)>,
    pub(crate) messages_window: bool,
    /// Palette de commandes ouverte (Ctrl+K, `palette.rs`).
    pub(crate) palette: Option<crate::palette::Palette>,
    /// Fenêtres Exporter… et Préférences… ouvertes (`prefsui.rs`).
    pub(crate) export_hub: bool,
    pub(crate) prefs_window: bool,
    /// Confirmation « Renuméroter les plans » ouverte (`numerotationui.rs`).
    pub(crate) renumber_confirm: bool,
    /// Fenêtre « Exporter pour le montage » (EDL, XML, OTIO : `editorialui.rs`).
    pub(crate) editorial: Option<crate::editorialui::EditorialWindow>,
    /// Relecture par le réalisateur : export de la page, import des notes, dessin du relecteur (`relectureui.rs`).
    pub(crate) relecture: crate::relectureui::RelectureUi,
    /// Fenêtre « À propos de SHOT » (`aproposui.rs`).
    pub(crate) about_window: bool,
    /// Le thème dont les couleurs sont posées dans egui : quand le réglage change, tout est
    /// reposé une fois (`theme.rs`).
    pub(crate) theme_applied: crate::theme::ReglageTheme,
    pub(crate) theme_unsaved: bool,
    /// Écran d'accueil à montrer tant qu'aucun projet n'est ouvert (`accueil.rs`).
    pub(crate) welcome: bool,
    /// Fond sous le trait (`fond.rs`) : le pinceau peint dans la strate Fond (sinon dans le
    /// trait), et le dessin dont on peint le fond en ce moment (affiché en direct).
    pub(crate) paint_fond: bool,
    pub(crate) fond_stroke: Option<CelId>,
    /// Le trait qui suit le même geste sur la coupe du fond (la gomme coupe, le pinceau recoud).
    pub(crate) fond_cut: Option<StrokeState>,
    /// Panneaux qui s'ajustent (`ajuste.rs`) : hauteur du contenu de chaque panneau à sa dernière
    /// image affichée, part posée par paire de panneaux, un bord a glissé à cette image.
    pub(crate) pane_heights: std::collections::HashMap<crate::dock::Pane, f32>,
    pub(crate) fit_set: std::collections::HashMap<(crate::dock::Pane, crate::dock::Pane), f32>,
    pub(crate) fit_busy: bool,
    /// Projet récent à ouvrir une fois « Enregistrer ? » réglé (`recents.rs`).
    pub(crate) recent_to_open: Option<PathBuf>,
    /// Les projets récents sont-ils encore là ? (Vérifié sur un autre fil : `recents.rs`.)
    pub(crate) recent_presence: crate::recents::Presence,
    /// Projet en cours d'ouverture sur un autre fil (`ouverture.rs`).
    pub(crate) open_job: Option<crate::ouverture::OpenJob>,
    /// Fenêtre « Aller à un plan » ouverte, et ce qui y est tapé (`aller.rs`).
    pub(crate) goto_plan: Option<String>,
    /// La fenêtre a reçu sa maximisation d'ouverture.
    pub(crate) maximized_once: u8,
    /// Geste du cutter en cours (points du dessin).
    pub(crate) cut_gesture: Option<Vec<Pos2>>,
    /// Enveloppe de Bézier en cours (outil Déformer, mode Enveloppe).
    pub(crate) envelope: Option<crate::envelopetools::EnvelopeEdit>,
    /// Déformation en perspective en cours (5f).
    pub(crate) persp: Option<crate::perstools::PerspEdit>,
    /// Rejeux exacts des dessins vectoriels en arrière-plan (`rejeu.rs`).
    pub(crate) rejeu: crate::rejeu::Rejeu,
    pub(crate) clipboard: Vec<Element>,
    pub(crate) renderer: Renderer,
    /// Le monde hors de la feuille à l'écran (toile infinie, `monde.rs`).
    pub(crate) monde: crate::monde::Monde,
    pub(crate) thumbs: Thumbs,
    pub(crate) montage_zoom: f32,
    /// Le zoom du montage et de la timeline reste calé sur la tête de lecture (`zoombande.rs`) :
    /// où elle était dans la vue au dernier affichage, et le défilement demandé par un zoom.
    pub(crate) strip_geo: [crate::zoombande::StripGeo; 2],
    pub(crate) strip_anchor: [Option<crate::zoombande::Anchor>; 2],
    /// « Tout voir » demandé dans le montage : le zoom s'ajuste à la prochaine image d'écran (il
    /// faut la largeur de la bande).
    pub(crate) montage_fit: bool,
    /// Largeur de la colonne de gauche du montage (`montagehead.rs`), retenue dans les réglages.
    pub(crate) montage_head_w: f32,
    pub(crate) montage_drag: Option<MontageDrag>,
    /// Rognage (`rognageui.rs`) : le bord que les flèches déplacent, l'aimantation des bords
    /// (l'aimant de la barre du montage), la fenêtre « Durée… », et le plan visé par le menu du
    /// montage avec la tête de lecture d'alors (id du plan, image du film).
    pub(crate) edge_sel: Option<crate::rognageui::EdgeSel>,
    pub(crate) snap_on: bool,
    pub(crate) duration_dialog: Option<crate::rognageui::DurationDialog>,
    pub(crate) trim_menu: Option<(u64, usize)>,
    /// Plan tel qu'il était au début du réglage de sa durée : chaque valeur intermédiaire
    /// repart de lui, rien n'est perdu avant la fin du geste.
    pub(crate) length_origin: Option<crate::rognageui::PlanBackup>,
    /// Un champ de durée est en cours d'utilisation pendant cette image d'écran.
    pub(crate) length_field_active: bool,
    pub(crate) pen_pressure: f32,
    pub(crate) pen_touch: bool,
    /// Horodatage par événement (sensation des brosses, §2.5) : egui donne la même heure à tous
    /// les événements d'une image d'écran ; `canvas_input` étale ceux du pointeur entre l'image
    /// d'avant (`last_frame_time`) et celle-ci, et `input_time` est l'instant de l'événement en
    /// cours, celui que reçoivent les traits (vitesse, lissage selon le temps).
    pub(crate) input_time: f64,
    pub(crate) last_frame_time: f64,
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
    /// Fenêtre « Nouveau calque » ouverte (13b).
    pub(crate) new_layer_dialog: Option<crate::newlayer::NewLayerForm>,
    /// Préréglages d'effets : la fenêtre de réglage, les derniers réglages, les textures des incrustations
    /// (`prereglagesui.rs`).
    pub(crate) effets_ui: crate::prereglagesui::EffetsUi,
    pub(crate) pending: Option<Pending>,
    /// Enregistrement qui s'écrit sur un autre fil (`enregistrement.rs`).
    pub(crate) saving: Option<crate::enregistrement::Enregistrement>,
    /// Enregistrement demandé pendant qu'un autre s'écrit : chemin et suite.
    pub(crate) save_queue: Option<(PathBuf, Option<Pending>)>,
    /// Action (Quitter, Nouveau, Ouvrir…) demandée pendant l'écriture : refaite à sa fin.
    pub(crate) after_save: Option<Pending>,
    /// Pourquoi le dernier enregistrement a échoué (fenêtre à fermer).
    pub(crate) save_error: Option<String>,
    /// Change à chaque projet remplacé (`load_doc`) : un enregistrement ne s'adopte que pour le
    /// projet dont il est le cliché.
    pub(crate) doc_epoch: u64,
    pub(crate) show_help: bool,
    pub(crate) allow_close: bool,
    pub(crate) export_transparent: bool,
    pub(crate) follow_frame: bool,
    /// Pendant l'affichage des panneaux : `follow_frame` de cette image d'écran. Le montage et la
    /// timeline, visibles ensemble (chantier 6), amènent tous deux l'image courante en vue.
    pub(crate) following: bool,

    pub(crate) hover_canvas: Option<Pos2>,
    /// Touches retirées des événements avant qu'egui ne s'en serve (Tab), traitées comme raccourcis.
    pub(crate) pending_keys: Vec<(Key, Option<Key>, egui::Modifiers)>,
    /// Raccourci en cours d'assignation, et la recherche de la liste des raccourcis
    /// (`raccourcisui.rs`).
    pub(crate) assign: Option<crate::raccourcisui::Assign>,
    pub(crate) keys_filter: String,
    pub(crate) keymap: Keymap,
    title: String,
    /// Moteur d'affichage utilisé (carte graphique et son interface, ou OpenGL).
    pub(crate) display_info: String,
    /// Superposition des calques par la carte graphique (absente en OpenGL ou si on la coupe).
    pub(crate) gpu: Option<GpuCanvas>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Polices du poste (outil Texte) : lues sur un autre fil dès le démarrage.
        crate::text::scan_in_background();
        if let Some(rs) = &cc.wgpu_render_state {
            let info = rs.adapter.get_info();
            eprintln!("affichage : wgpu, {} ({:?}, {:?})", info.name, info.backend, info.device_type);
            // Une erreur de la carte graphique : wgpu en fait par défaut un plantage de tout
            // SHOT ; elle est notée dans un rapport (`plantage.rs`) et l'on continue.
            rs.device.on_uncaptured_error(Box::new(|e| crate::plantage::note_gpu_error(&format!("erreur de la carte graphique : {e}"))));
            rs.device.set_device_lost_callback(|reason, message| crate::plantage::note_gpu_error(&format!("carte graphique perdue ({reason:?}) : {message}")));
        }
        Self::construire(&cc.egui_ctx, cc.wgpu_render_state.as_ref(), true)
    }

    /// Le logiciel sans fenêtre (mesures et essais de `perf_tests.rs`) : même état et mêmes
    /// panneaux que `new`, mais sans copie de secours ni séances interrompues, et les réglages
    /// s'écrivent là où `donnees::bac_a_sable` les a envoyés.
    #[cfg(test)]
    pub(crate) fn sans_fenetre(ctx: &egui::Context, rs: Option<&eframe::egui_wgpu::RenderState>) -> Self {
        // Une application qui tourne fait vieillir les demandes du cache des effets et peut le vider :
        // elle ne tourne pas pendant un essai qui compte sur cet état commun (`fxcache::monde`).
        crate::fxcache::monde::lire();
        Self::construire(ctx, rs, false)
    }

    /// L'état de départ. `reel` : lancé par l'utilisateur (copie de secours de la séance, séances
    /// interrompues à proposer) ; faux pour les essais.
    fn construire(egui_ctx: &egui::Context, wgpu_render_state: Option<&eframe::egui_wgpu::RenderState>, reel: bool) -> Self {
        // Police de l'interface (Segoe UI si Windows l'a) et icônes (Phosphor), utilisables dans
        // tout texte.
        crate::demarrage::debut();
        let mut fonts = egui::FontDefinitions::default();
        crate::theme::polices(&mut fonts);
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        egui_ctx.set_fonts(fonts);
        crate::demarrage::marque("polices (Segoe UI, icônes)");
        let settings = crate::settings::Settings::load();
        crate::demarrage::marque("réglages (Settings::load)");
        // Thème de l'interface, dense (12a, `theme.rs`) : celui des réglages.
        let theme_applied = settings.theme;
        crate::theme::installer(&theme_applied);
        crate::theme::appliquer(egui_ctx);
        crate::demarrage::marque("thème");
        let montage_head_w = crate::montagehead::clamp_head_w(settings.montage_head_w);
        let (paint_behind, fill_shapes) = (settings.paint_behind, settings.fill_shapes);
        // Diagnostic des performances : proposé une fois, sur une machine qui n'en a jamais eu
        // (jamais dans les essais).
        let diag_offer = reel && settings.diagnostic.a_proposer();
        // La bibliothèque de pinceaux : les pinceaux de SHOT tout de suite ; les importés (des
        // centaines : plusieurs secondes de calcul sur une machine chargée) se lisent sur un autre fil
        // et arrivent à une image d'écran quelconque (`poll_brushes`). La lire ici retardait la
        // première image de tout ce temps.
        let brushes = crate::brushtex::BrushLibrary::builtin();
        crate::demarrage::marque("pinceaux de SHOT (BrushLibrary::builtin)");
        let user_dir = crate::brushtex::BrushLibrary::user_dir();
        let brushes_job = std::thread::Builder::new().name("pinceaux".into()).spawn({
            let ctx = egui_ctx.clone();
            move || {
                let lib = crate::brushtex::BrushLibrary::load(user_dir.as_deref());
                ctx.request_repaint();
                lib
            }
        });
        crate::demarrage::marque("pinceaux importés : lecture lancée sur un autre fil");
        let keymap = Keymap::load(settings.keymap_preset);
        crate::demarrage::marque("raccourcis (Keymap::load)");
        let orphans = crate::recovery::root().filter(|_| reel).map(|r| crate::recovery::orphans(&r, None)).unwrap_or_default();
        let recovery = crate::recovery::root().filter(|_| reel).and_then(|r| crate::recovery::Recovery::start(&r).ok());
        crate::demarrage::marque("copie de secours (séances interrompues)");
        let mut app = Self {
            ctx: egui_ctx.clone(),
            doc: Document::with_frame(1920, 1080, 24).created_now(),
            mode: Mode::Storyboard,
            board_view: BoardView::Scene,
            crayonnes: Default::default(),
            cur_panel: 0,
            cur_layer: 0,
            cur_frame: 0,
            tool: Tool::Brush,
            brush: BrushSettings::brush(),
            // L'outil Crayon part du crayon graphite (rendu des brosses, étape 1) ; le crayon net
            // d'avant reste dans les réglages.
            pencil: BrushSettings::graphite(),
            eraser: BrushSettings::eraser(),
            brushes,
            brushes_job: brushes_job.ok(),
            reopen_probe: None,
            fill: FillSettings::default(),
            shape: ShapeSettings {
                filled: false,
                width: 4.0,
                line_mode: crate::shapes::LineMode::Straight,
                closed: true,
                last: Tool::Rect,
            },
            vector_erase_whole: false,
            paint_behind,
            fill_shapes,
            toucher: settings.toucher,
            pinceaux: settings.brushes_panel.clone(),
            // Les couleurs de la dernière fois (passe couleur).
            color: settings.colors.front,
            hsv: crate::colorui::rgb_to_hsv(settings.colors.front),
            couleurs: crate::colorui::ColorState::new(settings.colors.clone()),
            view: View::default(),
            fit_pending: true,
            canvas_rect: Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)),
            ppp: 1.0,
            onion: settings.onion.clone(),
            cadrage: settings.framing.clone(),
            onion_window: false,
            onion_offset: Vec2::ZERO,
            onion_shift: false,
            onion_preset_name: String::new(),
            xsheet: settings.xsheet.clone(),
            xsheet_seen_frame: None,
            roles_view: Default::default(),
            role_menu: false,
            xsheet_role_menu: false,
            flips: settings.flips.clone(),
            flip_run: None,
            flip_manual: None,
            flips_window: false,

            fx_auto_key: false,
            fx_keys_shown: true,
            fx_choice: None,
            fx_key_sel: Vec::new(),
            fx_key_anchor: None,
            fx_key_clipboard: None,
            fx_clip_recent: false,
            fx_keys_drag: None,
            fx_stack_clipboard: None,
            fx_pane_focus: false,
            video_import: None,
            propagation: crate::propagerui::Propagation::new(wgpu_render_state),
            coloriage: crate::coloriageui::Coloriage::new(),
            sb_window: None,
            scenario_window: None,
            sb_export: None,
            tween_dialog: None,
            light_table: false,
            light_table_strength: 0.3,
            guides: GuideState::default(),
            cam_drag: None,
            layer_drag: None,
            layer_thumbs: Default::default(),
            layer_rename: None,
            head_drag: None,
            panel_layer_drag: None,
            montage_layer_drag: None,
            layer_sel: Vec::new(),
            groups_window: false,
            groups_used_only: false,
            frame_sel: None,
            image_clipboard: None,
            key_clipboard: None,
            key_sel: Vec::new(),
            key_anchor: None,
            keys_drag: None,
            key_drag_delta: 0,
            key_drag_mode: crate::handles::KeyMove::Ripple,
            key_rect: None,
            tool_spring: None,
            ctrl_lasso: None,
            key_focus: false,
            comment_ui: Default::default(),
            refs: Default::default(),
            hole: Default::default(),
            images_focus: false,
            fake_hold_times: 2,
            image_drag: None,
            grid_select: None,
            plan_len_seen: None,
            instance_rename: None,
            smart_rename: None,
            time_display: Default::default(),
            timeline_cell: crate::timeline::CELL_W,
            timeline_fit: None,
            empty_x: true,
            timeline_thumbs: true,
            mark_color: 9,
            mark_repeat: false,
            mark_next_layer: false,
            mark_used_only: false,
            bottom_tab: None,
            checker: Default::default(),
            notes: Default::default(),
            timeline_sound_track: None,
            properties_window: false,
            settings_tab: Default::default(),
            res_edit: None,
            brush_previews: Default::default(),
            preset_edit: None,
            brush_import_notes: None,
            record_strokes: None,
            ref3d: Default::default(),
            camera_view: false,
            stage_view: None,
            format_custom: false,
            stage3d: Default::default(),
            scene3d: Default::default(),
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
            exposition_masquee: None,
            espace: crate::workspace::Espace::Storyboard,
            settings_a_ecrire: false,
            workspace_name: None,
            settings,
            // Les séances interrompues d'abord (avant d'ouvrir la nôtre).
            orphans,
            recovery,
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
            trans: Default::default(),
            envois_ui: Default::default(),
            comparaison: None,
            pile_versions: Default::default(),
            purge_versions: None,
            sfx: Default::default(),
            brt: Default::default(),
            sug: Default::default(),
            menu_target: None,
            history: History::default(),
            gesture: Gesture::default(),
            stroke: None,
            vstroke: None,
            verase: None,
            vsel: None,
            vdrag: None,
            flottante: None,
            sel_contours: Default::default(),
            multi: Default::default(),
            rejeu_again: Default::default(),
            rejeu_warm: Default::default(),
            rejeu_deferred: Vec::new(),
            rejeu_idle: Vec::new(),
            activity: 0.0,
            perf_applied: Default::default(),
            diag: crate::diagnosticui::DiagnosticUi::new(diag_offer, wgpu_render_state),
            bsel: None,
            bdrag: None,
            sel_box: None,
            keep_proportions: false,
            field_edit_alive: false,
            deform_mode: Default::default(),
            select_rect: false,
            pixel_clipboard: None,
            layer_clipboard: None,
            murs: Default::default(),
            panel_clipboard: None,
            panel_focus: false,
            cam_key_sel: Vec::new(),
            cam_key_anchor: None,
            cam_key_clipboard: None,
            cam_clip_recent: false,
            cam_keys_drag: None,
            cam_rect: None,
            cam_fx: crate::camfx::Params::new(crate::camfx::Preset::Shoulder, 24),
            cadrage_cache: Default::default(),
            handle_drag: None,
            grid_menu: None,
            shape_drag: None,
            shape_edit: None,
            text_edit: None,
            text_font: "Arial".into(),
            text_size: 48.0,
            text_fonts: None,
            text_font_filter: String::new(),
            color_select: Default::default(),
            tool_recent: Vec::new(),
            tool_flyout: None,
            cut_gesture: None,
            paste_event: false,
            maximized_once: 0,
            goto_plan: None,
            recent_to_open: None,
            recent_presence: Default::default(),
            open_job: None,
            palette: None,
            export_hub: false,
            prefs_window: false,
            renumber_confirm: false,
            editorial: None,
            relecture: Default::default(),
            about_window: false,
            theme_applied,
            theme_unsaved: false,
            welcome: true,
            paint_fond: false,
            fond_stroke: None,
            fond_cut: None,
            pane_heights: Default::default(),
            fit_set: Default::default(),
            fit_busy: false,
            messages: Vec::new(),
            last_status: String::new(),
            toasts: Vec::new(),
            messages_window: false,
            envelope: None,
            persp: None,
            rejeu: crate::rejeu::Rejeu::new(),
            clipboard: Vec::new(),
            renderer: Renderer::new(),
            monde: crate::monde::Monde::new(),
            thumbs: Thumbs::default(),
            montage_zoom: 4.0,
            strip_geo: Default::default(),
            strip_anchor: [None, None],
            montage_fit: false,
            montage_head_w,
            montage_drag: None,
            edge_sel: None,
            snap_on: true,
            duration_dialog: None,
            trim_menu: None,
            length_origin: None,
            length_field_active: false,
            pen_pressure: 1.0,
            input_time: 0.0,
            last_frame_time: 0.0,
            pen_touch: false,
            picking: false,
            path: None,
            import_source: None,
            written: HashMap::new(),
            modified: false,
            status: "Storyboard : dessine le premier plan ; P pour un nouveau plan, double-clic sur un plan pour l'animer (F1 : raccourcis).".into(),
            new_dialog: None,
            new_layer_dialog: None,
            effets_ui: Default::default(),
            pending: None,
            saving: None,
            save_queue: None,
            after_save: None,
            save_error: None,
            doc_epoch: 0,
            show_help: false,
            allow_close: false,
            export_transparent: false,
            follow_frame: false,
            following: false,

            hover_canvas: None,
            pending_keys: Vec::new(),
            assign: None,
            keys_filter: String::new(),
            keymap,
            title: String::new(),
            display_info: match wgpu_render_state {
                Some(rs) => {
                    let info = rs.adapter.get_info();
                    format!("wgpu, {} ({:?})", info.name, info.backend)
                }
                None => "OpenGL (moteur de secours)".into(),
            },
            gpu: None,
        };
        crate::demarrage::marque("état de l'application (le reste du constructeur)");
        // Calques superposés par la carte graphique, sauf PELURE_COMPOSITION=processeur.
        let cpu = std::env::var("PELURE_COMPOSITION").is_ok_and(|v| v.eq_ignore_ascii_case("processeur"));
        if !cpu {
            app.gpu = wgpu_render_state.map(GpuCanvas::new);
        }
        crate::demarrage::marque("carte graphique : pipelines des calques (GpuCanvas::new)");
        app.renderer.set_gpu(app.gpu.is_some());
        app.display_info.push_str(if app.gpu.is_some() {
            " · calques par la carte graphique"
        } else {
            " · calques par le processeur"
        });
        // La disposition de la dernière fois.
        // On démarre en storyboard, dans l'espace du côté storyboard où l'on avait quitté
        // (Storyboard, ou Montage) : l'unique disposition d'un ancien fichier de réglages est celle de
        // Storyboard. Les dispositions des autres espaces attendent qu'on y aille.
        app.espace = app.settings.storyboard_space;
        let saved = app.settings.layout_of(app.espace).cloned();
        if saved.is_some() || app.espace != crate::workspace::Espace::Storyboard {
            // (Disposition d'avant 6a : la nouvelle disposition par défaut, `modernisee`.)
            let layout = saved.unwrap_or_else(|| app.espace.defaut()).modernisee();
            // (Les cadres de composition de la dernière fois aussi.)
            app.layout.safe_frames = layout.safe_frames;
            app.layout.frame_format = layout.frame_format;
            app.apply_layout(layout);
        }
        crate::demarrage::marque("disposition des panneaux");
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
        let t = self.layer().transform;
        self.view.zoom * t.scale * (t.stretch_x * t.stretch_y).abs().sqrt()
    }

    pub(crate) fn modal_open(&self) -> bool {
        self.new_dialog.is_some()
            || self.about_window
            || self.pending.is_some()
            || self.export_dialog.is_some()
            || self.export_job.is_some()
            || !self.orphans.is_empty()
            || self.workspace_name.is_some()
            || self.open_job.is_some()
            || self.after_save.is_some()
            || self.save_error.is_some()
            || self.assign.is_some()
            || self.propagation.running()
            || self.coloriage.running()
            || self.sb_export.as_ref().is_some_and(crate::sbexportui::SbExportWindow::rendering)
            || self.editorial.as_ref().is_some_and(crate::editorialui::EditorialWindow::rendering)
            || self.relecture.rendering()
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
        // Tremblement du trait (`tremblement.rs`) : à l'écran et à l'arrêt, les calques qui animent
        // à l'arrêt suivent l'horloge ; l'export, les vignettes et la lecture, l'image.
        let rest = (onion && !self.playing).then(|| crate::tremblement::rest_frame(crate::fx::now_ms(), self.doc.fps));
        let editing = self.boil_editing();
        // Calque d'effet en cours de réglage (fenêtre des préréglages d'effets) : montré en direct, avant
        // d'exister (`prereglagesui.rs`). `cur` : le calque courant dans cette pile.
        let preview = if onion { self.effect_preview_layers() } else { None };
        let layers: &[crate::document::Layer] = preview.as_ref().map_or(self.layers(), |p| p.0.as_slice());
        let cur = preview.as_ref().map_or(self.cur_layer, |p| p.1);
        // Pour chaque calque, le nombre d'éléments posés avant lui (un calque d'effet couvre ceux d'en dessous).
        let mut starts: Vec<usize> = Vec::with_capacity(layers.len());
        for (li, layer) in layers.iter().enumerate() {
            starts.push(items.len());
            // Caché (lui ou son dossier) : rien ; sinon son opacité, avec celle de son dossier.
            let Some(mut opacity) = crate::folders::effect(layers, li) else { continue };
            // Calque d'effet (`calqueffet.rs`) : son repère, qui couvre ce qui est dessous.
            if layer.effet.is_some() {
                items.extend(crate::calqueffet::marker(layers, li, frame, opacity, &starts));
                continue;
            }
            if washed && li != cur {
                opacity *= self.light_table_strength.clamp(0.05, 1.0);
            }
            // Derrière la caméra (elle a avancé au-delà) : on ne le voit pas.
            let Some(seen) = layer.transform_seen(&camera) else { continue };
            let transform = (!seen.is_identity()).then_some(seen);
            // Pelure d'oignon : le calque courant, et ceux qui ont la leur (13b).
            let mut onion_items = Vec::new();
            if onion && self.onion.enabled && !self.playing && (li == cur || layer.onion) {
                let s = &self.onion;
                let (before, after) = crate::onion::neighbours(layer, frame, s, &self.panel().cues.bookmarks);
                // Décalage temporaire des images de la pelure (13f), sur la feuille.
                let shifted = (self.onion_offset != Vec2::ZERO).then(|| {
                    let t = transform.unwrap_or_default();
                    crate::document::LayerTransform { x: t.x + self.onion_offset.x, y: t.y + self.onion_offset.y, ..t }
                });
                // Du plus lointain au plus proche (le plus proche au-dessus) ; une image cachée
                // garde sa place dans le rang des autres.
                for (side, list) in [(false, &before), (true, &after)] {
                    for &(cel, k) in list.iter().rev().filter(|&&(_, k)| s.shown(side, k)) {
                        onion_items.push(PlanItem {
                            cel,
                            opacity: s.alpha(side, k),
                            tint: s.tint_of(side, k),
                            transform: shifted.or(transform),
                            blend: Default::default(),
                            fx: Vec::new(),
                            ctg: None,
                            fond: None,
                            pre: None,
                        });
                    }
                }
            }
            // Sous le dessin courant, ou devant (« Devant », 13f).
            let front = self.onion.front;
            if !front {
                items.append(&mut onion_items);
            }
            // Calque vidéo (13k) : l'image de la vidéo qui tombe là.
            if let Some(cel) = crate::video::cel_at(layer, frame, self.doc.fps) {
                let line = PlanItem {
                    cel,
                    transform,
                    opacity: (opacity * 255.0).round() as u8,
                    tint: None,
                    blend: layer.blend,
                    // Pile d'effets du calque (13i).
                    // (Le calque qu'on édite se montre sans tremblement : `boil_editing`.)
                    fx: match crate::fx::layer_steps(layer, frame, rest) {
                        steps if editing && (li == cur || self.multi.sel.as_ref().is_some_and(|s| s.has_layer(layer.id))) => crate::fx::without_boil(steps),
                        steps => steps,
                    },
                    // Calque CTG (13j) : rempli d'après les traits de ses sources.
                    ctg: crate::ctg::input(layers, li, frame),
                    fond: None,
                    pre: None,
                };
                // Fond sous le trait (`fond.rs`), le trait par-dessus.
                items.extend(crate::render::fond_item(layer, &line));
                match self.flottante.as_ref().filter(|f| f.src == cel).or_else(|| self.multi.float_for(cel)) {
                    // Sélection vectorielle en cours de transformation : le dessin sans elle, et sa
                    // feuille flottante par-dessus (`flottante.rs`) ; avec plusieurs calques
                    // choisis, une feuille par calque (`multisel.rs`).
                    Some(f) => items.extend(f.items(&line)),
                    None => items.push(line),
                }
                // Calque CTG courant, à l'écran seulement : ses graffitis, pâles, par-dessus.
                if onion && layer.ctg.is_some() && li == cur && !self.playing {
                    items.push(PlanItem { cel, transform, opacity: 90, tint: None, blend: Default::default(), fx: Vec::new(), ctg: None, fond: None, pre: None });
                }
            }
            items.append(&mut onion_items);
        }
        Plan {
            // Arrière-plan (13e) : le papier seulement si c'est une couleur.
            paper: if paper { self.doc.render_paper() } else { None },
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
        // Les images choisies étaient celles de l'autre plan (audit A4).
        self.frame_sel = None;
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

    /// Change de mode : l'animation a son espace de travail, le storyboard celui où l'on était de ce
    /// côté (Storyboard ou Montage, `Settings::storyboard_space`).
    pub(crate) fn set_mode(&mut self, mode: Mode) {
        let espace = match mode {
            Mode::Animation => crate::workspace::Espace::Animation,
            Mode::Storyboard => self.settings.storyboard_space,
        };
        self.set_espace(espace);
    }

    /// Va dans un espace de travail (Storyboard, Animation ou Montage) : le mode qui va avec, la
    /// disposition qu'on y avait (`aller_a_l_espace`).
    pub(crate) fn set_espace(&mut self, espace: crate::workspace::Espace) {
        use crate::workspace::Espace;
        self.cancel_montage_drag();
        self.end_stroke();
        self.playing = false;
        self.aller_a_l_espace(espace);
        let mode = self.mode;
        // Le dessin change : c'est lui qui prend la main (les raccourcis suivent le nouveau mode).
        self.panel_focus = false;
        self.images_focus = false;
        self.key_focus = false;
        if mode == Mode::Storyboard {
            // En storyboard, on reste dans les bornes du plan (la timeline permet d'aller au-delà).
            self.cur_frame = self.cur_frame.min(self.frame_count() - 1);
        }
        self.follow_frame = true;
        self.status = match espace {
            Espace::Storyboard => "Storyboard : montage des plans.".into(),
            Espace::Montage => "Montage : la bande des plans en grand (Affichage > Espaces de travail > Storyboard pour en revenir).".into(),
            Espace::Animation => format!("Animation de {} (Tab : retour au storyboard).", crate::storyboard::panel_long_title(&self.doc, self.cur_panel)),
        };
        // La feuille d'exposition n'existe qu'en animation.
        self.masquer_exposition();
    }

    pub(crate) fn select_layer(&mut self, layer: usize) {
        if layer < self.layers().len() && layer != self.cur_layer {
            self.end_stroke();
            self.cur_layer = layer;
            self.vsel = None;
        }
    }

    /// Lecture / pause (touche) : tout le montage si les raccourcis sont ceux du storyboard (le
    /// montage a la main, ou le dessin en storyboard), sinon le plan.
    pub(crate) fn toggle_play(&mut self) {
        self.toggle_play_scope(self.key_mode() == Mode::Storyboard);
    }

    /// Le mode des raccourcis : celui du panneau qui a la main (`key_mode_of`).
    pub(crate) fn key_mode(&self) -> Mode {
        key_mode_of(self.mode, self.panel_focus, self.images_focus, self.key_focus)
    }

    /// Lecture / pause du montage entier (`montage`) ou du plan courant : les boutons du montage et
    /// de la timeline, qui se voient ensemble dans les deux modes (chantier 6).
    pub(crate) fn toggle_play_scope(&mut self, montage: bool) {
        if self.playing {
            self.playing = false;
            self.stop_audio();
            return;
        }
        self.end_stroke();
        self.play_montage = montage;
        // Dans le plan : entre les points d'entrée et de sortie (13d).
        let (start, end) = self.plan_play_range();
        let at_end = if self.play_montage {
            self.global_frame() + 1 >= self.doc.total_frames()
        } else {
            self.cur_frame + 1 >= end || self.cur_frame < start
        };
        if at_end {
            if self.play_montage {
                self.set_panel(0);
                self.cur_frame = 0;
            } else {
                self.cur_frame = start;
            }
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
            let (start, end) = if self.play_montage { (0, self.frame_count()) } else { self.plan_play_range() };
            if self.cur_frame + 1 < end {
                self.cur_frame += 1;
            } else if self.play_montage && self.cur_panel + 1 < self.doc.panels.len() {
                let next = self.cur_panel + 1;
                self.set_panel(next);
            } else if self.looping {
                if self.play_montage {
                    self.set_panel(0);
                }
                self.cur_frame = start;
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
    /// En storyboard, un premier dessin sur un calque vide remplit tout le plan ; sinon il est
    /// tenu jusqu'au prochain dessin ou à la fin du plan ; en animation, sur le nombre d'images
    /// choisi (« sur 1 », « sur 2 »…) : `images::new_drawing_span`.
    pub(crate) fn ensure_cel(&mut self) -> (CelId, Option<Edit>) {
        // Calque de couleur : son dessin est celui que le miroir du trait lui donne.
        if self.layer().couleur.is_some() {
            return self.ensure_colour_cel();
        }
        if let Some(id) = self.cel_here() {
            // Cassure automatique (13c) : sur une image tenue, ailleurs qu'au début de son
            // exposition, on dessine sur une copie, jusqu'à la fin de l'exposition.
            let f = self.cur_frame;
            if self.layer().auto_break && self.layer().run_bounds(f).0 < f {
                let snap = self.snapshot();
                let copy = self.doc.duplicate_cel(id);
                let li = self.cur_layer;
                crate::images::break_instance(&mut self.layers_mut()[li].frames, f, copy);
                crate::calquecouleur::noter_copie(&mut self.doc, self.cur_panel, li, id, copy);
                self.status = "Cassure automatique : un nouveau dessin à partir de cette image.".into();
                return (copy, Some(snap));
            }
            return (id, None);
        }
        let (li, f) = (self.cur_layer, self.cur_frame);
        let snap = self.snapshot();
        let kind = self.layer().kind;
        let id = self.doc.new_cel(kind);
        let limit = self.frame_count();
        let storyboard = self.mode == Mode::Storyboard;
        let (start, end) = crate::images::new_drawing_span(&self.layer().frames, f, limit, storyboard, self.default_exposure);
        let layer = &mut self.layers_mut()[li];
        for k in start..end {
            layer.set(k, Some(id));
        }
        (id, Some(snap))
    }

    pub(crate) fn layer_editable(&mut self) -> bool {
        if self.content_refuses() {
            return false;
        }
        if self.layer().folder {
            self.status = FOLDER_NOT_DRAWABLE.into();
            return false;
        }
        if self.layer().video.is_some() {
            self.status = "Un calque vidéo ne se dessine pas : « Transformer en calque d'animation » d'abord (menu de sa ligne), ou dessine sur un calque au-dessus.".into();
            return false;
        }
        if self.layer().locked {
            let key = self.keymap.label("calque.verrouiller");
            self.status = format!("Ce calque est verrouillé ({key} pour le déverrouiller).");
            return false;
        }
        if crate::folders::locked_by_folder(self.layers(), self.cur_layer) {
            self.status = "Le dossier de ce calque est verrouillé : déverrouille-le pour dessiner.".into();
            return false;
        }
        if self.layer().position_locked {
            self.status = POSITION_LOCKED.into();
            return false;
        }
        if !self.layer().auto_create && self.cel_here().is_none() {
            let key = self.keymap.label("dessin.nouveau");
            self.status = format!("Pas de dessin à cette image, et la création automatique est éteinte : Nouveau dessin ({key}) d'abord.");
            return false;
        }
        if crate::folders::effect(self.layers(), self.cur_layer).is_some() {
            return true;
        }
        self.status = "Ce calque est masqué (lui ou son dossier) : affiche-le pour dessiner dessus.".into();
        false
    }

    /// Le calque courant est un dossier, ou le storyboard est verrouillé (13e) : les commandes de
    /// dessins et d'expositions ne s'y appliquent pas (le message le dit).
    pub(crate) fn refuse_on_folder(&mut self) -> bool {
        if self.content_refuses() {
            return true;
        }
        if self.layers().get(self.cur_layer).is_some_and(|l| l.folder) {
            self.status = FOLDER_NOT_DRAWABLE.into();
            return true;
        }
        // Un calque de couleur n'a pas d'images à lui : celles de son calque de trait.
        if self.layers().get(self.cur_layer).is_some_and(|l| l.keys_follow_line()) {
            self.status = "Les images d'un calque de couleur suivent celles de son calque de trait : pose, déplace ou supprime les clés sur le trait.".into();
            return true;
        }
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
        if !erase {
            // Elle rejoint les couleurs récentes (passe couleur).
            self.color_used(self.color);
        }
        if erase && self.cel_here().is_none() {
            self.guides.stroke = None;
            return;
        }
        // Strate Fond (`fond.rs`) : pinceau et gomme peignent dans le fond, en bitmap.
        if self.paint_fond && self.layer().fond.is_some() {
            self.begin_fond_stroke(p, pressure, pen);
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
        let stencil = self.stencil_for(cel);
        // Calque de couleur : le trait reste dans la zone fermée par le trait où il commence.
        let stencil = self.couleur_pochoir(stencil, p);
        let painting = self.tool != Tool::Eraser;
        let (behind, fill, tol) = (painting && self.paint_behind, painting && self.fill_shapes, self.close_tolerance());
        let mut stroke = StrokeState::new(cel, mode, settings, prefix, pen)
            .with_textures(textures)
            .with_feel(self.toucher)
            .with_zoom(self.layer_zoom())
            .with_secondary(self.couleurs.prefs.back)
            .on_layer(self.layer().keep_alpha, stencil)
            .with_options(behind, fill, tol);
        // Toile infinie : on peint aussi hors de la feuille (tuiles gardées à part, `Raster::outer`) ;
        // sauf dans la zone fermée d'un calque de couleur (son pochoir est celui de la feuille).
        if !(painting && self.layer().couleur.as_ref().is_some_and(|c| c.zone)) {
            stroke = stroke.on_infinite_canvas();
        }
        let mut dirty = Vec::new();
        // L'instant de l'événement (étalé dans l'image d'écran, `canvas_input`) : la vitesse du
        // trait et le lissage selon le temps en viennent.
        let time = self.input_time;
        if let Some(c) = self.doc.cels.get_mut(&cel) {
            stroke.add_point_at(&mut c.raster, p, pressure, time, &mut dirty);
        }
        self.renderer.invalidate_tiles(&dirty);
        self.stroke = Some(stroke);
        self.modified = true;
    }

    /// Un trait dans la strate Fond du dessin courant : la peinture reste dans la forme du fond
    /// (déduite du trait, gardée comme un pochoir), sous le trait.
    fn begin_fond_stroke(&mut self, p: Pos2, pressure: f32, pen: bool) {
        let (Some(settings), Some(r)) = (self.current_brush().copied(), self.layer().fond) else {
            return;
        };
        let (cel, prefix) = self.ensure_cel();
        let (w, h) = (self.doc.width, self.doc.height);
        let Some(cover) = self.doc.cels.get(&cel).map(|c| crate::fond::silhouette(&c.raster, &r)) else { return };
        if cover.iter().all(|&c| c == 0) {
            self.status = "Strate Fond : dessine d'abord le trait, le fond se déduit de lui (bascule sur Trait).".into();
            if let Some(p) = prefix {
                self.history.push(p);
            }
            return;
        }
        let mut stencil = self.stencil_for(cel);
        stencil.get_or_insert_with(|| crate::stencil::Stencil::new(w, h)).add(&crate::fond::mask(&cover, w, h), false, None);
        let erase = self.tool == Tool::Eraser;
        let mode = if erase { StrokeMode::Erase } else { StrokeMode::Paint(self.color) };
        let mut stroke = StrokeState::new(cel, mode, settings, prefix, pen)
            .with_textures(self.brushes.textures(&settings))
            .with_feel(self.toucher)
            .with_zoom(self.layer_zoom())
            .with_secondary(self.couleurs.prefs.back)
            .on_layer(false, stencil)
            .with_options(false, false, self.close_tolerance());
        // La coupe suit le même geste : la gomme coupe le fond (elle peint la coupe), le pinceau
        // le recoud (il efface la coupe).
        let cut_mode = if erase { StrokeMode::Paint([255, 255, 255]) } else { StrokeMode::Erase };
        let mut cut = StrokeState::new(cel, cut_mode, settings, None, pen)
            .with_textures(self.brushes.textures(&settings))
            .with_feel(self.toucher)
            .with_zoom(self.layer_zoom())
            .on_layer(false, None)
            .with_options(false, false, self.close_tolerance());
        let mut dirty = Vec::new();
        let time = self.input_time;
        if let Some(c) = self.doc.cels.get_mut(&cel) {
            let s = c.fond.get_or_insert_with(|| Box::new(crate::document::Strate::new(w, h)));
            stroke.add_point_at(&mut s.peinture.raster, p, pressure, time, &mut dirty);
            cut.add_point_at(&mut s.coupe, p, pressure, time, &mut dirty);
        }
        crate::fx::fond_patch(&self.doc, cel, &r, &dirty);
        self.renderer.invalidate_tiles(&dirty);
        self.fond_stroke = Some(cel);
        self.fond_cut = Some(cut);
        self.stroke = Some(stroke);
        self.modified = true;
    }

    /// Pochoirs des autres calques visibles du plan, à l'image courante (13b), pour un trait sur
    /// le dessin `painted` du calque courant ; aucun : on peint partout.
    pub(crate) fn stencil_for(&self, painted: CelId) -> Option<crate::stencil::Stencil> {
        let seen = self.doc.camera_seen(self.cur_panel, self.cur_frame);
        let mine = self.layer_seen();
        let center = self.doc.stage_center();
        let empty = crate::raster::Raster::new(self.doc.width, self.doc.height);
        let mut stencil = None;
        for (li, l) in self.layers().iter().enumerate() {
            if li == self.cur_layer || !l.stencil || crate::folders::effect(self.layers(), li).is_none() {
                continue;
            }
            let cel = l.shown_at(self.cur_frame);
            if cel == Some(painted) {
                continue;
            }
            // Derrière la caméra, on ne le voit pas : il ne cache rien.
            let Some(theirs) = l.transform_seen(&seen) else { continue };
            let to_stencil = if theirs == mine {
                None
            } else {
                let Some(inv) = theirs.affine(center).inverse() else { continue };
                Some(mine.affine(center).then(&inv))
            };
            // Image vide : un pochoir positif ne laisse rien passer, un négatif laisse tout.
            let raster = cel.and_then(|c| self.doc.cels.get(&c)).map_or(&empty, |c| &c.raster);
            stencil
                .get_or_insert_with(|| crate::stencil::Stencil::new(self.doc.width, self.doc.height))
                .add(raster, l.stencil_negative, to_stencil);
        }
        stencil
    }

    pub(crate) fn stroke_to(&mut self, p: Pos2, pressure: f32) {
        let time = self.input_time;
        let Some(stroke) = self.stroke.as_mut() else { return };
        let cel = stroke.cel;
        let Some(c) = self.doc.cels.get_mut(&cel) else { return };
        let mut dirty = Vec::new();
        // Un trait dans la strate Fond peint sa peinture et sa coupe, et le fond affiché suit.
        if self.fond_stroke == Some(cel) {
            if let Some(s) = c.fond.as_mut() {
                stroke.add_point_at(&mut s.peinture.raster, p, pressure, time, &mut dirty);
                if let Some(cut) = self.fond_cut.as_mut() {
                    cut.add_point_at(&mut s.coupe, p, pressure, time, &mut dirty);
                }
            }
            if let Some(r) = self.layer().fond {
                crate::fx::fond_patch(&self.doc, cel, &r, &dirty);
            }
        } else {
            stroke.add_point_at(&mut c.raster, p, pressure, time, &mut dirty);
        }
        self.renderer.invalidate_tiles(&dirty);
    }

    /// Termine tout geste en cours : trait, gomme vectorielle, transformation, forme.
    pub(crate) fn end_stroke(&mut self) {
        if let Some(mut stroke) = self.stroke.take() {
            // Banc de traits : le trait tel que reçu, avec son réglage.
            if let Some(n) = self.record_strokes.as_mut() {
                if stroke.raw_log.len() > 1 {
                    if let Some(dir) = crate::banc::dir() {
                        match crate::banc::append(&dir, &crate::banc::Recorded::of(&stroke)) {
                            Ok(path) => {
                                *n += 1;
                                self.status = format!("Banc de traits : {} trait(s) enregistré(s) dans {}.", n, path.display());
                            }
                            Err(e) => self.status = format!("Banc de traits : {e}"),
                        }
                    }
                }
            }
            let cel = stroke.cel;
            let mut dirty = Vec::new();
            // Un trait dans la strate Fond : fini dans son dessin, annulé par `FondTiles`.
            if self.fond_stroke.take() == Some(cel) {
                let mut cut = self.fond_cut.take();
                let mut edits = Vec::new();
                if let Some(c) = self.doc.cels.get_mut(&cel) {
                    if let Some(s) = c.fond.as_mut() {
                        edits.extend(stroke.finish(&mut s.peinture.raster, &mut dirty).map(|e| crate::fond::to_fond_edit(e, false)));
                        if let Some(cut) = cut.as_mut() {
                            edits.extend(cut.finish(&mut s.coupe, &mut dirty).map(|e| crate::fond::to_fond_edit(e, true)));
                        }
                        s.peinture.version += 1;
                    }
                    c.version += 1;
                }
                self.renderer.invalidate_tiles(&dirty);
                // Peinture et coupe : une seule étape d'annulation.
                match edits.len() {
                    0 => {}
                    1 => self.history.push(edits.pop().unwrap_or(Edit::Group(Vec::new()))),
                    _ => self.history.push(Edit::Group(edits)),
                }
            } else {
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
                        self.commit_vector_stroke(vs, std::mem::take(&mut stroke.path), std::mem::take(&mut stroke.path_pressure), std::mem::take(&mut stroke.path_time), preview)
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
        }
        self.finish_vector_erase();
        self.finish_cut();
        // (Le geste d'un champ chiffré de la boîte de sélection dure tant que le champ a la main :
        // `finish_field_edit_if_idle`.)
        if self.vdrag.is_some() && !matches!(&self.vdrag, Some(VDrag::Transform { field: true, .. })) {
            self.finish_vdrag(false);
        }
        if self.bdrag.is_some() && !matches!(&self.bdrag, Some(BDrag::Transform { field: true, .. })) {
            self.finish_bdrag();
        }
        if self.multi.drag.is_some() && !matches!(&self.multi.drag, Some(crate::multisel::MDrag::Transform { field: true, .. })) {
            self.multi_finish_drag();
        }
        self.finish_camera_drag();
        self.finish_layer_drag();
        self.finish_ref3d_drag();
        self.guides.stroke = None;
        self.shape_drag = None;
    }

    pub(crate) fn pick_color(&mut self, p: Pos2) {
        if !p.x.is_finite() || !p.y.is_finite() {
            return;
        }
        let plan = self.build_plan(self.cur_frame, false, true);
        let c = if p.x < 0.0 || p.y < 0.0 || p.x >= self.doc.width as f32 || p.y >= self.doc.height as f32 {
            // Toile infinie : la pipette prend aussi hors de la feuille (le pixel du monde, papier compris).
            let (x, y) = (p.x.floor(), p.y.floor());
            let area = render::Area::around((x, y), (x + 1.0, y + 1.0));
            match render::composite_area(&self.doc, &plan, area).first() {
                Some(&c) => c,
                None => return,
            }
        } else {
            render::composite_pixel(&self.doc, &plan, p.x as usize, p.y as usize)
        };
        let [r, g, b, _] = unpremultiply(c);
        self.color = [r, g, b];
    }

    pub(crate) fn fill_at(&mut self, p: Pos2) {
        if p.x < 0.0 || p.y < 0.0 || p.x >= self.doc.width as f32 || p.y >= self.doc.height as f32 {
            // Toile infinie : le pot de peinture cherche ses contours sur la feuille (une zone qui
            // s'étendrait à l'infini n'a pas de fin) ; hors de la feuille, il ne remplit pas.
            self.status = "Le pot de peinture remplit dans la feuille : clique sur une zone fermée de la feuille.".into();
            return;
        }
        // Strate Fond (`fondui.rs`) : la zone fermée par le trait se remplit dans le fond.
        if self.paint_fond && self.layer().fond.is_some() {
            self.fond_zone_at(p, false);
            return;
        }
        // Calque de couleur (`calquecouleurui.rs`) : la zone fermée par le trait de son calque de trait.
        if self.layer().couleur.is_some() {
            self.fill_couleur(p);
            return;
        }
        self.end_stroke();
        self.playing = false;
        if !self.layer_editable() {
            return;
        }
        self.color_used(self.color);
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

    /// Refait l'image d'un dessin vectoriel et rafraîchit l'affichage (sur le fil de l'interface :
    /// pour ce qui n'est pas un geste ; voir `rejeu_cel`).
    pub(crate) fn refresh_cel(&mut self, cel: CelId) {
        self.rejeu.cancel(cel);
        self.rejeu_idle.retain(|&c| c != cel);
        self.rejeu_again.remove(&cel);
        if let Some(c) = self.doc.cels.get_mut(&cel) {
            let tiles = c.rerender();
            self.renderer.invalidate_tiles(&tiles);
        }
    }

    /// Comme `refresh_cel`, dans `region` seulement (là où quelque chose a changé) : le coût ne
    /// grandit plus avec le nombre de traits du dessin. Les traits hors de `region` gardent leur
    /// image : si elle était encore approchée (rendu rapide à l'ouverture, aperçu d'un geste), le
    /// rendu exact en est redemandé dès que plus rien ne se passe sur ce dessin.
    pub(crate) fn refresh_cel_region(&mut self, cel: CelId, region: Rect) {
        if self.rejeu.is_pending(cel) {
            self.rejeu_again.insert(cel);
        }
        self.rejeu.cancel(cel);
        if let Some(c) = self.doc.cels.get_mut(&cel) {
            let tiles = c.rerender_region(region.expand(2.0));
            self.renderer.invalidate_tiles(&tiles);
        }
    }

    /// Demande le rendu exact du dessin vectoriel `cel` en arrière-plan (`rejeu.rs`) : son image
    /// actuelle (l'aperçu du geste, posé depuis les caches) reste affichée jusqu'à ce qu'il arrive.
    /// Demande interactive : elle passe avant la file du chargement d'un projet.
    pub(crate) fn rejeu_cel(&mut self, cel: CelId) {
        self.rejeu_with(cel, true);
    }

    pub(crate) fn rejeu_with(&mut self, cel: CelId, priority: bool) {
        // (Le rendu exact vient : il n'attend plus le repos.)
        self.rejeu_idle.retain(|&c| c != cel);
        let (w, h) = (self.doc.width, self.doc.height);
        if let Some((d, version)) = self.doc.cels.get(&cel).and_then(|c| c.vector.as_ref().map(|d| (d, c.version))) {
            self.rejeu.submit_with(cel, d, w, h, version, priority);
        }
    }

    /// Le dessin vectoriel qu'on regarde a tous ses traits en cache : sinon saisir une sélection
    /// les rejouerait sur le fil de l'interface (20 ms par trait large, 0,8 s pour 40). Ce qui
    /// manque (budget dépassé, ouverture d'un projet, autre panneau) est demandé une fois au fil de
    /// travail par version du dessin, qui ne rejoue que ces traits-là (`Rejeu::submit_with`).
    pub(crate) fn warm_current_cel(&mut self) {
        if self.playing {
            return;
        }
        // Le dessin du calque courant, et ceux des autres calques choisis (une sélection sur
        // plusieurs calques les saisit tous : `multisel.rs`).
        let frame = self.cur_frame;
        let cels: Vec<CelId> = std::iter::once(self.cel_here())
            .chain(self.layer_sel.iter().filter_map(|id| self.layers().iter().find(|l| l.id == *id)).map(|l| l.cel_at(frame)))
            .flatten()
            .collect();
        for cel in cels {
            self.warm_cel(cel);
        }
    }

    fn warm_cel(&mut self, cel: CelId) {
        let Some(c) = self.doc.cels.get(&cel) else { return };
        let Some(d) = &c.vector else { return };
        if self.rejeu_warm.get(&cel) == Some(&c.version) || self.cel_busy(cel) || self.rejeu.is_pending(cel) || self.flottante.is_some() || !self.multi.floats.is_empty() {
            return;
        }
        self.rejeu_warm.insert(cel, c.version);
        if d.missing_caches() > 0 {
            self.rejeu_cel(cel);
        }
    }

    /// Un geste, un trait ou une gomme est en cours sur ce dessin : son image est celle du geste.
    pub(crate) fn cel_busy(&self, cel: CelId) -> bool {
        self.stroke.as_ref().is_some_and(|s| s.cel == cel)
            || self.vstroke.as_ref().is_some_and(|s| s.cel == cel)
            || self.verase.as_ref().is_some_and(|e| e.cel == cel)
            || matches!(&self.vdrag, Some(VDrag::Transform { cel: c, .. }) if *c == cel)
            || self.multi.busy_on(cel)
            || self.envelope.as_ref().is_some_and(|e| e.busy_on(cel))
            || self.persp.as_ref().is_some_and(|e| e.busy_on(cel))
    }

    /// Après Ctrl+Z / Ctrl+Maj+Z : les rejeux des dessins que l'historique a refaits ne valent
    /// plus (leur version a changé), les autres continuent. Un dessin refait est exact ; on en
    /// redemande quand même le rendu : sans effet s'il l'est, juste s'il ne l'était pas.
    pub(crate) fn rejeu_after_history(&mut self) {
        for cel in crate::rejeu::outdated(&self.rejeu, |c| self.doc.cels.get(&c).map(|c| c.version)) {
            self.rejeu.cancel(cel);
            self.rejeu_again.insert(cel);
        }
    }

    /// Adopte les rejeux exacts arrivés : l'image du dessin, et les pixels exacts en cache de ses
    /// traits, d'un coup — si le dessin n'a pas changé entre-temps, et pas pendant un geste sur
    /// ce dessin (le trait en cours disparaîtrait) : le résultat attend la fin du geste. Un
    /// résultat que le dessin a dépassé est jeté et redemandé : l'image approchée ne reste jamais.
    pub(crate) fn poll_rejeu(&mut self, ctx: &egui::Context) {
        let again: Vec<CelId> = self.rejeu_again.iter().copied().filter(|&c| self.rejeu.is_pending(c) || !self.cel_busy(c)).collect();
        for cel in again {
            self.rejeu_again.remove(&cel);
            // (Une demande plus récente est déjà en route : elle suffit.)
            if !self.rejeu.is_pending(cel) {
                self.rejeu_cel(cel);
            }
        }
        for cel in self.rejeu.take_failed() {
            self.status = "Le rendu exact d'un dessin a échoué : il reste en rendu approché.".into();
            self.rejeu_again.remove(&cel);
        }
        let mut arrived = std::mem::take(&mut self.rejeu_deferred);
        arrived.extend(self.rejeu.poll());
        for done in arrived {
            if self.cel_busy(done.cel) {
                self.rejeu_deferred.push(done);
                continue;
            }
            // La feuille flottante d'un geste lâché sur ce dessin laisse la place au rendu exact.
            let waiting = self.flottante.as_ref().is_some_and(|f| f.pending.is_some() && f.src == done.cel);
            // Les feuilles d'un geste lâché sur plusieurs calques (`multisel.rs`).
            let waiting_multi = self.multi.floats.iter().any(|f| f.pending.is_some() && f.src == done.cel);
            let Some(c) = self.doc.cels.get(&done.cel) else { continue };
            if !crate::rejeu::adoptable(c, &done) {
                // Une enveloppe ou une perspective qu'on a reprise avant l'arrivée du rendu : un
                // rendu plus récent est déjà demandé, la feuille déformée l'attend.
                if waiting && self.flottante.as_ref().is_some_and(|f| f.for_warp) && self.rejeu.is_pending(done.cel) {
                    continue;
                }
                // Le dessin a changé depuis : on termine à l'ancienne ce que la feuille montrait,
                // et le rendu exact du dessin d'aujourd'hui est redemandé.
                let vector = c.vector.is_some();
                if waiting {
                    self.flush_flottante();
                }
                if waiting_multi {
                    self.multi_settle(done.cel, false);
                }
                if vector {
                    self.rejeu_cel(done.cel);
                }
                continue;
            }
            if waiting {
                self.flottante = None;
                crate::flottante::Flottante::release(&mut self.doc);
            }
            if waiting_multi {
                self.multi_settle(done.cel, true);
            }
            let Some(c) = self.doc.cels.get_mut(&done.cel) else { continue };
            let Some(d) = c.vector.as_mut() else { continue };
            for (e, exact) in d.elements.iter_mut().zip(&done.drawing.elements) {
                e.adopt_cache(exact);
            }
            let changed: Vec<usize> = (0..done.raster.tiles.len()).filter(|&i| c.raster.tiles.get(i).is_some_and(Option::is_some) || done.raster.tiles[i].is_some()).collect();
            // Le fichier du dessin dit la même chose (la géométrie n'a pas changé, les pixels en
            // cache ne s'enregistrent pas) : le premier enregistrement après l'ouverture ne le
            // réécrit pas.
            let before = c.version;
            c.raster = done.raster;
            c.version += 1;
            // (Ses traits sont tous en cache : rien à réchauffer pour cette version.)
            self.rejeu_warm.insert(done.cel, c.version);
            crate::rejeu::keep_saved(&mut self.written, done.cel, before, c.version);
            self.renderer.invalidate_tiles(&changed);
        }
        if self.rejeu.has_pending() || !self.rejeu_deferred.is_empty() || !self.rejeu_again.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
        // Fonds sous le trait calculés en arrière-plan (`fondfil.rs`).
        if crate::fx::fond_poll() {
            self.renderer.invalidate_composed();
            ctx.request_repaint();
        }
        if crate::fx::fond_pending() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    pub(crate) fn resize_brush(&mut self, factor: f32) {
        if self.tool.is_shape() {
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
        // Les calques de couleur suivent le trait tout de suite, dans la même étape d'annulation.
        self.sync_couleurs();
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

    /// Comme `note_change`, quand l'appelant enregistre lui-même l'étape (`push`, appelé seulement
    /// au début du geste `key`) : une flèche tenue qui déplace la sélection = une étape.
    pub(crate) fn gesture_change(&mut self, key: u64, push: impl FnOnce(&mut Self)) {
        if self.gesture.needs_step(key, self.history.serial()) {
            push(self);
        }
        self.gesture.started(key, self.history.serial());
        self.modified = true;
    }

    /// Le geste `key` continue pendant cette image (`gesture_change`).
    pub(crate) fn keep_gesture(&mut self, key: u64) {
        self.gesture.keep(key);
    }

    /// Enregistre `edits` (les retours en arrière d'un coup sur plusieurs dessins) comme une
    /// seule étape au nom `name` ; les appuis suivants du même geste `key` (une flèche tenue) se
    /// joignent à cette étape, qui les annule tous (`History::extend_last`).
    pub(crate) fn gesture_group(&mut self, key: u64, name: &str, edits: Vec<Edit>) {
        if edits.is_empty() {
            return;
        }
        let group = Edit::Group(edits);
        let before = self.history.context.replace(name.to_string());
        if self.gesture.needs_step(key, self.history.serial()) {
            self.history.push(group);
        } else {
            self.history.extend_last(group);
        }
        self.history.context = before;
        self.gesture.started(key, self.history.serial());
        self.modified = true;
    }

    /// Distance (en pixels du dessin) sous laquelle le bout d'un trait rejoint son début : la
    /// forme est fermée pour « remplir la forme » (15 points d'écran).
    pub(crate) fn close_tolerance(&self) -> f32 {
        15.0 / self.layer_zoom().max(1e-3)
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
        if self.refuse_on_folder() {
            return;
        }
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
        // Une copie du dessin emporte la couleur du dessin copié.
        if let (true, Some(src)) = (duplicate, current) {
            crate::calquecouleur::noter_copie(&mut self.doc, self.cur_panel, li, src, id);
        }
        self.cur_frame = at;
        self.follow_frame = true;
        self.vsel = None;
        self.commit_structure(snap);
        // Le nouveau dessin, seul, devient le choix.
        self.select_new_key(self.cur_panel, li, at, at + n);
    }

    pub(crate) fn exposure_plus(&mut self) {
        if self.refuse_on_folder() {
            return;
        }
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
        if self.refuse_on_folder() {
            return;
        }
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
        layer.shift_frames(end - 1, 1, true);
        layer.trim();
        if self.cur_frame >= end - 1 && self.cur_frame > start {
            self.cur_frame = end - 2;
        }
        self.commit_structure(snap);
    }

    /// Retire le dessin courant et toutes ses expositions ; la suite du calque recule.
    pub(crate) fn delete_image(&mut self) {
        if self.refuse_on_folder() {
            return;
        }
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
        layer.shift_frames(start, end - start, true);
        layer.trim();
        self.cur_frame = start;
        self.vsel = None;
        self.commit_structure(snap);
    }

    /// Vide le dessin courant sans toucher à la timeline.
    pub(crate) fn clear_image(&mut self) {
        self.end_stroke();
        self.drop_bitmap_selection();
        if self.content_refuses() {
            return;
        }
        if !self.layer().drawable() {
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
        let mut tiles: Vec<(usize, Option<Tile>)> = c
            .raster
            .tiles
            .iter_mut()
            .enumerate()
            .filter(|(_, t)| t.is_some())
            .map(|(i, t)| (i, t.take()))
            .collect();
        // (Toile infinie : ce qui est hors de la feuille part aussi.)
        let outside: Vec<usize> = c.raster.outer.keys().map(|&(tx, ty)| crate::raster::outer_id(tx, ty)).collect();
        for id in outside {
            tiles.push((id, c.raster.replace_slot(id, None)));
        }
        if !tiles.is_empty() {
            self.push_pixels(id, None, tiles);
        }
    }

    pub(crate) fn add_layer(&mut self, kind: LayerKind) {
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        let snap = self.snapshot();
        let n = self.layers().len() + 1;
        let name = match kind {
            LayerKind::Vector => format!("Vecto {n}"),
            LayerKind::Bitmap => format!("Calque {n}"),
        };
        let at = self.cur_layer + 1;
        // Au-dessus d'un calque rangé dans un dossier : dans ce dossier (13b-5).
        let mut layer = Layer::new(name, kind);
        layer.parent = self.layer().parent;
        self.layers_mut().insert(at, layer);
        self.cur_layer = at;
        self.vsel = None;
        self.commit_structure(snap);
    }

    /// Duplique le calque courant ; la copie, juste au-dessus, devient le calque courant.
    pub(crate) fn duplicate_layer(&mut self) {
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        let snap = self.snapshot();
        self.cur_layer = crate::layerops::duplicate_layer(&mut self.doc, self.cur_panel, self.cur_layer);
        self.layer_sel.clear();
        self.vsel = None;
        self.commit_structure(snap);
    }

    /// Monte ou descend le calque courant d'un rang (Ctrl + Haut / Bas). Un dossier se déplace
    /// avec ses calques, par-dessus le calque ou le dossier voisin ; un calque rangé qui passe le
    /// bord de son dossier en sort (13b-5).
    pub(crate) fn move_layer(&mut self, up: bool) {
        self.end_stroke();
        if self.layer().couleur.is_some() {
            self.status = "Un calque de couleur reste sous son calque de trait : déplace le trait, la couleur le suit.".into();
            return;
        }
        let id = self.layer().id;
        let mut layers = self.layers().to_vec();
        if !crate::folders::move_step(&mut layers, self.cur_layer, up) {
            return;
        }
        let snap = self.snapshot();
        *self.layers_mut() = layers;
        self.cur_layer = self.layers().iter().position(|l| l.id == id).unwrap_or(0);
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

    /// Copie le calque courant, avec ses dessins et sa timeline (W) ; un dossier, avec ses
    /// calques (13b-5).
    pub(crate) fn copy_layer(&mut self) {
        let layer = self.layer().clone();
        let children: Vec<Layer> =
            crate::folders::children_of(self.layers(), self.cur_layer).into_iter().map(|i| self.layers()[i].clone()).collect();
        let colours: Vec<Layer> =
            crate::calquecouleur::couleurs_de(self.layers(), self.cur_layer).into_iter().map(|i| self.layers()[i].clone()).collect();
        let cels = children
            .iter()
            .chain(colours.iter())
            .chain(std::iter::once(&layer))
            .flat_map(|l| l.frames.iter().flatten())
            .filter_map(|id| self.doc.cels.get(id).map(|c| (*id, c.clone())))
            .collect();
        let what = if layer.folder { "Dossier" } else { "Calque" };
        self.status = format!("{what} « {} » copié : X pour le coller dans un autre plan.", layer.name);
        self.layer_clipboard = Some(LayerClip {
            layer,
            children,
            colours,
            cels,
            panel_len: self.frame_count(),
        });
    }

    /// Calque collé : une copie de `source` avec ses propres copies des dessins (`copies` évite de
    /// copier deux fois un même dessin), à la durée du plan ; un dessin tenu jusqu'à la fin du plan
    /// d'origine reste tenu jusqu'à la fin de celui-ci.
    fn pasted_layer(&mut self, source: &Layer, clip: &LayerClip, copies: &mut HashMap<CelId, CelId>) -> Layer {
        let mut layer = Layer { id: fresh_layer_id(), scene_wide: None, ..source.clone() };
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
        layer.names = source.names.iter().filter_map(|(c, n)| copies.get(c).map(|d| (*d, n.clone()))).collect();
        layer.color_marks = source.color_marks.iter().filter_map(|(c, k)| copies.get(c).map(|d| (*d, *k))).collect();
        // Un calque de couleur collé : les copies de ses dessins de couleur.
        if let Some(c) = layer.couleur.as_mut() {
            c.relink(&HashMap::new(), copies);
        }
        let n = self.frame_count();
        let held = source.frames.len() >= clip.panel_len && source.frames.last().is_some_and(Option::is_some);
        layer.roles.retain(|f, _| *f < n);
        if layer.frames.len() > n {
            layer.frames.truncate(n);
        } else if held {
            let last = layer.frames.last().copied().flatten();
            layer.frames.resize(n, last);
        }
        layer.trim();
        layer
    }

    /// Colle le calque copié au-dessus du calque courant (X), avec ses propres copies des dessins.
    /// Dans le dossier du calque courant s'il y en a un ; un dossier copié se colle avec ses
    /// calques, au-dessus du dossier courant (pas de dossier dans un dossier).
    pub(crate) fn paste_layer(&mut self) {
        if self.content_refuses() {
            return;
        }
        let Some(clip) = self.layer_clipboard.clone() else {
            self.status = "Aucun calque copié (W pour copier le calque courant).".into();
            return;
        };
        self.end_stroke();
        self.drop_bitmap_selection();
        let snap = self.snapshot();
        let mut copies: HashMap<CelId, CelId> = HashMap::new();
        let mut layer = self.pasted_layer(&clip.layer, &clip, &mut copies);
        let mut children: Vec<Layer> = Vec::new();
        for c in &clip.children {
            let mut child = self.pasted_layer(c, &clip, &mut copies);
            child.parent = Some(layer.id);
            children.push(child);
        }
        // Ses calques de couleur reviennent avec lui, sous la copie.
        for c in &clip.colours {
            let mut colour = self.pasted_layer(c, &clip, &mut copies);
            if let Some(link) = colour.couleur.as_mut() {
                link.relink(&HashMap::from([(clip.layer.id, layer.id)]), &copies);
            }
            children.push(colour);
        }
        let cur = self.cur_layer;
        let folder_of_current = crate::folders::parent_index(self.layers(), cur);
        let at = match (layer.folder, folder_of_current) {
            // Un dossier ne va pas dans un dossier : au-dessus du dossier du calque courant.
            (true, Some(fi)) => fi + 1,
            _ => cur + 1,
        };
        layer.parent = if layer.folder { None } else { folder_of_current.map(|fi| self.layers()[fi].id) };
        for c in children.iter_mut().filter(|c| c.couleur.is_some()) {
            c.parent = layer.parent;
        }
        let name = layer.name.clone();
        let id = layer.id;
        let block: Vec<Layer> = children.into_iter().chain(std::iter::once(layer)).collect();
        for (k, l) in block.into_iter().enumerate() {
            self.layers_mut().insert(at + k, l);
        }
        crate::folders::normalize(self.layers_mut());
        self.cur_layer = self.layers().iter().position(|l| l.id == id).unwrap_or(at);
        self.vsel = None;
        self.commit_structure(snap);
        self.status = format!("« {name} » collé.");
    }

    /// Fusionne le calque courant avec celui du dessous (Ctrl+E).
    pub(crate) fn merge_layer_down(&mut self) {
        if self.content_refuses() {
            return;
        }
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
        // Une durée rendue par l'annulation n'est pas signalée.
        self.plan_len_seen = None;
        self.end_stroke();
        // Une zone bitmap déplacée est d'abord reposée : l'annulation défait alors ce déplacement.
        self.drop_bitmap_selection();
        self.multi_before_history();
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
        self.plan_len_seen = None;
        self.end_stroke();
        self.drop_bitmap_selection();
        self.multi_before_history();
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
        // Les dessins que l'historique a refaits ont été rendus directement : plus rien à attendre
        // du rejeu pour eux. Les autres (ouverture récente, geste lâché) continuent.
        self.rejeu_after_history();
        self.cur_panel = nav.panel;
        self.cur_layer = nav.layer;
        self.clamp_nav();
        self.guides.current = self.guides.current.min(self.doc.guides.len().saturating_sub(1));
        self.vsel = None;
        // (Les clés d'effets et de caméra choisies étaient celles d'avant l'annulation.)
        self.fx_key_sel.clear();
        self.fx_key_anchor = None;
        self.clear_cam_keys();
        self.length_origin = None;
        self.renderer.invalidate_all();
        self.modified = true;
    }

    // ------------------------------------------------------------------ fichiers

    pub(crate) fn request(&mut self, action: Pending) {
        self.end_stroke();
        self.playing = false;
        // Un enregistrement s'écrit : on l'attend, puis la demande est refaite (modifié depuis : la
        // boîte « Modifications non enregistrées » ; sinon, l'action). `enregistrement.rs`.
        if self.saving.is_some() {
            self.after_save = Some(action);
        } else if self.modified {
            self.pending = Some(action);
        } else {
            self.perform(action);
        }
    }

    pub(crate) fn perform(&mut self, action: Pending) {
        match action {
            // La fenêtre « Nouveau projet » (`nouveauui.rs`).
            Pending::New => self.open_new_project(),
            Pending::Open => self.open_dialog(),
            Pending::FromChosen => {
                self.welcome = false;
                self.new_project_from_chosen();
            }
            Pending::SbImport => {
                self.welcome = false;
                self.start_sb_import();
            }
            Pending::OpenRecent => {
                if let Some(path) = self.recent_to_open.take() {
                    self.open_path(path);
                }
            }
            Pending::ScenarioNew => self.scenario_new_project(),
            Pending::Close => {
                let (w, h, fps) = (self.doc.frame[0], self.doc.frame[1], self.doc.fps);
                self.load_doc(Document::with_frame(w, h, fps).created_now());
                self.path = None;
                // L'accueil revient (`accueil.rs`).
                self.welcome = true;
                self.status = "Projet fermé : un projet vide du même format le remplace.".into();
            }
            Pending::Quit => {
                self.allow_close = true;
                self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    // (Enregistrer : `enregistrement.rs`, sur un autre fil.)

    fn open_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Projet SHOT ou Storyboard Pro", &["pelure", "sboard"])
            .add_filter("Projet SHOT", &["pelure"])
            .add_filter("Projet Storyboard Pro", &["sboard"])
            .pick_file()
        else {
            return;
        };
        self.open_path(path);
    }

    /// Ouvre un projet en dossier, ou importe un ancien fichier `.pelure`. Un projet Storyboard
    /// Pro (`.sboard`) ouvre la fenêtre d'import avec lui (14a).
    pub(crate) fn open_path(&mut self, path: PathBuf) {
        if self.import_dropped_sboard(&path) {
            return;
        }
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
        // Ouverture immédiate : les dessins vectoriels texturés paraissent en rendu rapide, leur
        // rendu exact arrive en arrière-plan (`rejeu.rs`).
        // La lecture se fait sur un autre fil (`ouverture.rs`) : l'interface reste vivante.
        self.start_open(path);
    }

    pub(crate) fn load_doc(&mut self, doc: Document) {
        self.rejeu.cancel_all();
        self.rejeu_again.clear();
        self.rejeu_warm.clear();
        self.rejeu_deferred.clear();
        self.rejeu_idle.clear();
        crate::fx::fond_cancel_all();
        // (Les suggestions sonores de l'ancien projet s'arrêtent : rien n'est rendu au nouveau.)
        self.reset_suggestions();
        self.doc = doc;
        self.doc_epoch += 1;
        // (Ses dessins portent les numéros et les versions de ceux de l'ancien : les dessins
        // dérivés des effets de l'ancien ne lui servent pas.)
        crate::fx::forget_all();
        // (Un autre projet : l'effet choisi et les clés choisies étaient ceux de l'ancien.)
        self.fx_choice = None;
        self.fx_key_sel.clear();
        self.fx_key_anchor = None;
        self.clear_cam_keys();
        self.cam_key_clipboard = None;
        self.cam_clip_recent = false;
        self.comment_ui = Default::default();
        self.mode = Mode::Storyboard;
        self.cur_panel = 0;
        self.cur_layer = self.layers().len() - 1;
        self.cur_frame = 0;
        self.stroke = None;
        self.fond_stroke = None;
        self.fond_cut = None;
        self.vstroke = None;
        self.verase = None;
        self.vsel = None;
        self.vdrag = None;
        self.flottante = None;
        crate::flottante::Flottante::release(&mut self.doc);
        self.multi_cancel();
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
        self.layer_thumbs.clear();
        self.renderer.invalidate_all();
        self.fit_pending = true;
        // Vue du plateau et vue 3D recadrées sur le nouveau projet (9d, 9e).
        self.stage_view = None;
        self.stage3d.cam = None;
        self.scene3d_reset();
        self.modified = false;
        self.path = None;
        self.import_source = None;
        self.written.clear();
        self.length_origin = None;
    }

    /// Après un changement de taille de la feuille (Fichier > Réglages du projet) : ce qui
    /// pointait dans l'ancienne feuille est oublié, l'historique vidé (il ne saurait pas revenir
    /// à l'ancienne taille), la vue recadrée.
    pub(crate) fn after_stage_change(&mut self) {
        self.stroke = None;
        self.fond_stroke = None;
        self.fond_cut = None;
        self.vstroke = None;
        self.verase = None;
        self.vsel = None;
        self.vdrag = None;
        self.flottante = None;
        crate::flottante::Flottante::release(&mut self.doc);
        self.multi_cancel();
        self.bsel = None;
        self.bdrag = None;
        self.history.clear();
        self.thumbs.clear();
        self.layer_thumbs.clear();
        self.renderer.invalidate_all();
        self.fit_pending = true;
        self.stage_view = None;
        self.stage3d.cam = None;
        self.scene3d_reset();
        self.length_origin = None;
        self.modified = true;
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
        self.exactify_idle();
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
        let [w, h] = self.doc.frame;
        let all: Vec<usize> = (0..self.doc.panels.len()).collect();
        let (frames, boards) = crate::export::storyboard_of(&self.doc, &all);
        for (k, ((i, f), board)) in frames.into_iter().zip(boards).enumerate() {
            // Intertitre au début de chaque séquence.
            if f == 0 && self.doc.sequence_run(i).0 == i {
                if i > 0 {
                    html += "</main>";
                }
                html += &format!(
                    "<h2 class=\"seq\">Séquence {}</h2><main>",
                    io::html_escape(&self.doc.sequence_label(self.doc.sequence_of(i)))
                );
            }
            let plan = render::panel_plan_at(&self.doc, i, f, Some(self.doc.paper));
            let pixels = render::camera_image(&self.doc, &plan, &self.doc.panel_camera(i, f));
            let file = format!("planche_{:03}.png", k + 1);
            if let Err(e) = io::write_png_scaled(&dir.join(&file), &pixels, w, h, 960) {
                self.status = format!("Échec de l'export : {e}");
                return;
            }
            html += &format!(
                "<section><img src=\"{file}\"><h2>{}</h2><p class=\"dur\">{}</p>",
                io::html_escape(&board.title),
                board.duration
            );
            for (label, text) in [("Action", &board.action), ("Dialogue", &board.dialogue), ("Notes", &board.notes)] {
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
        self.exactify_idle();
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
        let mut pixels = render::camera_image(&self.doc, &plan, &cam);
        // Les incrustations (timecode…) sont gravées dans l'image (`incrustation.rs`).
        let boxes = crate::incrustation::boxes_for(&self.doc, self.cur_panel, self.cur_frame, true, crate::incrustation::now_secs());
        crate::incrustation::burn(&mut pixels, self.doc.frame, &boxes);
        self.status = match io::write_png(&path, &pixels, self.doc.frame[0], self.doc.frame[1]) {
            Ok(()) => format!("Image exportée : {}", path.display()),
            Err(e) => format!("Échec de l'export : {e}"),
        };
    }

    /// Pose une image (PNG/JPEG) au centre du dessin courant d'un calque bitmap.
    pub(crate) fn import_image(&mut self) {
        self.end_stroke();
        if self.layer().kind != LayerKind::Bitmap || self.layer().folder {
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
        // Un raccourci en cours d'assignation prend la touche tapée.
        if self.capture_shortcut(ctx) {
            return;
        }
        // Pas de raccourci pendant une saisie ou un dialogue.
        if ctx.wants_keyboard_input() || self.modal_open() {
            self.pending_keys.clear();
            return;
        }
        // Pendant un glisser (chantier 2) : Échap ou Ctrl+Z l'abandonnent, tout revient comme
        // avant ; les autres touches attendent la fin du geste.
        if self.dragging() {
            let mode = self.key_mode();
            let cancel = ctx.input(|i| {
                i.events.iter().any(|e| match e {
                    Event::Key { key, physical_key, pressed: true, modifiers, .. } => {
                        (*key == Key::Escape && !modifiers.any())
                            || self.keymap.find(*key, *physical_key, *modifiers, mode).is_some_and(|a| a == "edition.annuler")
                    }
                    _ => false,
                })
            });
            if cancel {
                self.cancel_drags();
            }
            self.pending_keys.clear();
            return;
        }
        // Touches retirées à egui avant qu'il ne s'en serve (Tab), puis touches de cette image.
        let mut keys: Vec<(Key, Option<Key>, egui::Modifiers, bool)> = std::mem::take(&mut self.pending_keys)
            .into_iter()
            .map(|(k, p, m)| (k, p, m, false))
            .collect();
        // Ctrl+V sans texte dans le presse-papiers (une image copiée d'un navigateur) : egui garde
        // la touche et n'envoie rien ; son relâché arrive, et sert alors de Ctrl+V (`pasteimage`).
        let (mut paste_seen, mut v_up, mut v_up_ctrl) = (false, false, false);
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
                    Event::Key { key, physical_key, pressed: false, modifiers, .. } if *key == Key::V || *physical_key == Some(Key::V) => {
                        v_up = true;
                        v_up_ctrl |= modifiers.command;
                    }
                    // egui transforme Ctrl+C / X / V en événements de presse-papiers.
                    Event::Copy => keys.push((Key::C, Some(Key::C), egui::Modifiers::COMMAND, false)),
                    Event::Cut => keys.push((Key::X, Some(Key::X), egui::Modifiers::COMMAND, false)),
                    Event::Paste(_) => {
                        paste_seen = true;
                        keys.push((Key::V, Some(Key::V), egui::Modifiers::COMMAND, false));
                    }
                    _ => {}
                }
            }
        });
        self.paste_event |= paste_seen;
        if v_up {
            if v_up_ctrl && !self.paste_event {
                keys.push((Key::V, Some(Key::V), egui::Modifiers::COMMAND, false));
            }
            self.paste_event = false;
        }
        // Une flèche tenue qui déplace la sélection reste une seule étape d'annulation.
        if ctx.input(|i| crate::nudge::ARROWS.iter().any(|k| i.key_down(*k))) {
            self.keep_gesture(crate::nudge::GESTURE);
        }
        for (key, physical, m, repeat) in keys {
            // La planche de crayonnés rapides a la main : Tab et les flèches changent de case.
            if self.crayonnes_key(key, m) {
                continue;
            }
            // Entrée, Échap, Retour arrière servent d'abord à l'outil en cours (forme à finir…).
            if !m.any() && self.tool_key(key) {
                continue;
            }
            // Une sélection sur le dessin : les flèches la déplacent (`nudge.rs`), avant de changer
            // d'image ou de calque.
            if let Some(d) = crate::nudge::arrow_step(key, m).filter(|_| self.nudge_ready()) {
                self.nudge_selection(d);
                continue;
            }
            // Les raccourcis suivent le panneau qui a la main (montage, timeline ou dessin).
            let Some(action) = self.keymap.find(key, physical, m, self.key_mode()) else { continue };
            let action = action.to_string();
            if repeat && !keymap::action_info(&action).is_some_and(|a| a.repeat) {
                continue;
            }
            // La touche d'un outil : tenue, elle le prend le temps de la tenir (`springtools`).
            let before = self.tool;
            self.run_action(&action);
            if !repeat && action.starts_with("outil.") {
                let now = ctx.input(|i| i.time);
                self.spring_after(key, before, now);
            }
        }
    }

    /// Exécute une action désignée par son identifiant stable (voir `keymap::ACTIONS`). Les étapes
    /// d'annulation qu'elle enregistre portent son nom (panneau Historique).
    pub(crate) fn run_action(&mut self, action: &str) {
        let name = crate::keymap::ACTIONS.iter().find(|a| a.id == action).map(|a| crate::historyui::step_name(a.label));
        let before = self.history.context.clone();
        if name.is_some() {
            self.history.context = name;
        }
        self.run_action_now(action);
        self.history.context = before;
    }

    fn run_action_now(&mut self, action: &str) {
        // Un texte en cours de saisie est posé avant toute commande (annuler, changer d'image…).
        self.finish_text_edit();
        // Le panneau Références a la main : Ctrl+V y colle l'image du presse-papiers.
        if self.refs.focus && matches!(action, "edition.coller" | "edition.coller_a_la_tete" | "edition.coller_sur_place") {
            self.refs_paste();
            return;
        }
        // Une image copiée ailleurs (navigateur…) passe avant le presse-papiers de SHOT, dont
        // les copies écrivent « SHOT : … » dans celui de Windows (`pasteimage.rs`).
        if action == "edition.coller" || action == "edition.coller_a_la_tete" {
            if let Some(image) = crate::pasteimage::clipboard_image() {
                self.paste_clipboard_image(image);
                return;
            }
        }
        // Un champ de la boîte de sélection (Réglages) en cours de saisie : son geste se termine
        // avant toute commande (annuler, changer d'image…).
        self.finish_field_edit();
        // Annuler ou rétablir pendant une déformation : elle repartirait d'un dessin périmé.
        if matches!(action, "edition.annuler" | "edition.retablir") {
            self.persp = None;
            self.envelope = None;
        }
        if self.workspace_action(action) {
            return;
        }
        // Planche de crayonnés rapides (`crayonnesui.rs`) : ses commandes, et annuler, rétablir,
        // tout choisir, supprimer quand elle a la main.
        if self.crayonnes_action(action) {
            return;
        }
        // Fond sous le trait (`fondui.rs`).
        if self.fond_action(action) {
            return;
        }
        // Préréglages d'effets et calques d'effets (`prereglagesui.rs`).
        if self.effet_action(action) {
            return;
        }
        // Scène 3D de blocage (`scene3dui.rs`).
        if self.scene3d_action(action) {
            return;
        }
        // Commentaires sur la timeline (`commentairesui.rs`).
        if self.comment_action(action) {
            return;
        }
        // Transitions du montage (`transitionsui.rs`) ; Suppr vise celle qui a la main.
        if self.transition_action(action) {
            return;
        }
        // Envois et plans modifiés depuis le dernier (`envoisui.rs`).
        if self.envois_action(action) {
            return;
        }
        // Versions d'un plan (`planversui.rs`), et leur comparaison (`comparerui.rs`).
        if self.version_action(action) || self.comparer_action(action) {
            return;
        }
        // Effets des extraits de son (`audiofxui.rs`) ; Suppr retire la clé de volume choisie.
        if self.sound_fx_action(action) {
            return;
        }
        // Bibliothèque de bruitages (`bruitagesui.rs`).
        if self.bruitage_action(action) {
            return;
        }
        // Suggestions sonores (`suggestionsui.rs`).
        if self.suggestion_action(action) {
            return;
        }
        // Références visuelles (`referencesui.rs`) : leurs commandes ; le panneau qui a la main
        // prend Ctrl+V (une image à coller) et Suppr avant les autres.
        if self.references_action(action) {
            return;
        }
        if action == "edition.supprimer" && self.refs_has_focus() {
            self.refs_delete_selected();
            return;
        }
        // Suppr vise le commentaire seulement s'il a la main (le dernier clic était sur lui) ;
        // sinon elle suit les règles d'avant (rien sans sélection).
        if action == "edition.supprimer" && self.comment_has_focus() {
            self.delete_selected_comment();
            return;
        }
        // Couleurs et nuanciers (passe couleur, `colorui.rs`).
        if self.color_action(action) {
            return;
        }
        // `board` : les raccourcis sont ceux du storyboard (le montage a la main, ou le dessin en
        // storyboard) ; `board_shown` : le dessin montre le storyboard (Tab, vue Vignettes).
        let board = self.key_mode() == Mode::Storyboard;
        let board_shown = self.mode == Mode::Storyboard;
        match action {
            "fichier.nouveau" => self.request(Pending::New),
            "fichier.ouvrir" => self.request(Pending::Open),
            "fichier.enregistrer" => {
                self.save();
            }
            "fichier.afficher_explorateur" => match self.path.clone() {
                Some(p) => crate::exportui::reveal_in_explorer(&p),
                None => self.status = "Projet pas encore enregistré : Fichier > Enregistrer d'abord.".into(),
            },
            "fichier.enregistrer_sous" => {
                self.save_as();
            }
            "fichier.quitter" => self.request(Pending::Quit),
            "fichier.fermer" => self.request(Pending::Close),
            "aide.palette" => self.palette = Some(Default::default()),
            "fichier.exporter" => self.export_hub = true,
            "fichier.preferences" => self.prefs_window = true,
            "aide.a_propos" => self.about_window = true,
            "aide.diagnostic" => self.open_diagnostic(),
            // Le collage pose déjà les éléments et les zones à leur place d'origine.
            "edition.coller_sur_place" => self.run_action("edition.coller"),
            // Les propriétés du plan : la fiche (action, dialogue, notes, durée…).
            "panneau.proprietes" => self.show_pane(crate::dock::Pane::Sheet),
            "scene.aller_a" => self.goto_plan = Some(String::new()),
            "calque.flou" => self.toggle_fx(self.cur_layer, crate::fx::EffectKind::GaussianBlur),
            "fichier.exporter_film" => self.open_export(crate::export::Format::Mp4),
            "fichier.exporter_images" => self.open_export(crate::export::Format::Png),
            "fichier.exporter_son" => self.open_export(crate::export::Format::Wav),
            "fichier.exporter_pdf" => self.open_export(crate::export::Format::Pdf),
            "fichier.exporter_montage" => self.open_editorial(),
            "fichier.exporter_relecture" | "fichier.importer_notes" => {
                self.relecture_action(action);
            }
            "numerotation.verrouiller" => self.toggle_numbering_lock(),
            "numerotation.renumeroter" => self.renumber_confirm = true,
            "fichier.exporter_image_courante" => self.export_frame(),
            "fichier.importer_modele3d" => self.import_model3d(),
            "edition.annuler" => self.undo(),
            "edition.retablir" => self.redo(),
            // Le panneau Effets a la main (le dernier clic était dedans) : Ctrl+C copie l'effet
            // choisi et Ctrl+V colle les effets copiés, avant tous les autres presse-papiers
            // (`fxcopie.rs`). Sans effet sur la pile, Ctrl+C suit les règles d'avant.
            "edition.copier" if self.fx_pane_has_focus() && !self.layer().fx.is_empty() => self.fx_copy_chosen(),
            "edition.coller" if self.fx_pane_has_focus() && self.fx_stack_clipboard.is_some() => {
                self.fx_paste(self.fx_paste_targets(self.cur_layer), false)
            }
            // Des clés de caméra choisies (un losange jaune de la piste Caméra) : copier, couper,
            // supprimer les visent, coller aussi tant que c'est la dernière chose copiée
            // (`camkeys.rs`).
            "edition.copier" if self.cam_keys_have_focus() => self.copy_cam_keys(),
            "edition.couper" if self.cam_keys_have_focus() => {
                self.copy_cam_keys();
                self.delete_cam_keys();
            }
            "edition.coller" if self.cam_clip_recent && self.cam_key_clipboard.is_some() && (self.key_focus || self.images_focus) => self.paste_cam_keys(),
            // Maj+Ctrl+V : les clés (caméra, effets, dessins) se collent sur la tête de lecture ;
            // le reste se colle comme Ctrl+V (voir plus bas).
            "edition.coller_a_la_tete" if self.cam_clip_recent && self.cam_key_clipboard.is_some() && (self.key_focus || self.images_focus) => {
                self.paste_cam_keys_at_head()
            }
            "edition.supprimer" if self.cam_keys_have_focus() => self.delete_cam_keys(),
            "camera.cles_copier" => self.copy_cam_keys(),
            "camera.cles_coller" => self.paste_cam_keys(),
            "camera.cles_supprimer" => self.delete_cam_keys(),
            "camera.cles_tout_choisir" => self.choose_all_cam_keys(),
            "camera.effet_appliquer" => self.apply_camera_effect(),
            "cadrage.tiers" => self.cadrage.thirds = !self.cadrage.thirds,
            "cadrage.grille_or" => self.cadrage.phi_grid = !self.cadrage.phi_grid,
            "cadrage.spirale" => self.cadrage.spiral = !self.cadrage.spiral,
            "cadrage.spirale_orientation" => {
                // (Elle s'allume si besoin : on veut voir l'orientation qu'on choisit.)
                if self.cadrage.spiral {
                    self.cadrage.next_variant();
                }
                self.cadrage.spiral = true;
                self.status = format!("Spirale d'or : orientation {}.", crate::cadrage::variant_label(self.cadrage.spiral_variant));
            }
            "cadrage.diagonales" => self.cadrage.diagonals = !self.cadrage.diagonals,
            "cadrage.croix" => self.cadrage.cross = !self.cadrage.cross,
            "cadrage.aucune" => self.cadrage.clear(),
            "cadrage.format_suivant" => self.next_frame_format(),
            // Des clés d'effets choisies (un losange violet du montage ou de la timeline) passent
            // avant les clés de dessins : copier, couper et supprimer les visent, coller aussi
            // tant que c'est la dernière chose copiée (`fxkeys.rs`).
            "edition.copier" if self.fx_keys_have_focus() => self.copy_fx_keys(),
            "edition.couper" if self.fx_keys_have_focus() => {
                self.copy_fx_keys();
                self.delete_fx_keys();
            }
            "edition.coller" if self.fx_clip_recent && self.fx_key_clipboard.is_some() && (self.key_focus || self.images_focus) => self.paste_fx_keys(),
            "edition.coller_a_la_tete" if self.fx_clip_recent && self.fx_key_clipboard.is_some() && (self.key_focus || self.images_focus) => {
                self.paste_fx_keys_at_head()
            }
            "edition.supprimer" if self.fx_keys_have_focus() => self.delete_fx_keys(),
            "fx.cles_copier" => self.copy_fx_keys(),
            "fx.cles_coller" => self.paste_fx_keys(),
            "fx.cles_supprimer" => self.delete_fx_keys(),
            // Après un clic sur la piste d'un calque dans le montage : les clés choisies, sinon
            // celle de l'image courante du calque courant (`montagekeys`).
            "edition.copier" if self.key_focus => self.copy_keys(),
            "edition.couper" if self.key_focus => {
                self.copy_keys();
                self.delete_keys(false);
            }
            "edition.coller" if self.key_focus && self.key_clipboard.is_some() => self.paste_keys(),
            "edition.coller_a_la_tete" if self.key_focus && self.key_clipboard.is_some() => self.paste_keys_at_head(),
            // Ni clés de caméra, ni clés d'effets, ni clés de dessins à coller : Maj+Ctrl+V fait
            // ce qu'il faisait avant (« Coller sur place », un alias de Ctrl+V).
            "edition.coller_a_la_tete" => self.run_action("edition.coller"),
            "edition.supprimer" if self.key_focus => self.delete_keys(false),
            "dessin.dupliquer" if self.key_focus => self.duplicate_keys(),
            // Une clé à l'image courante du calque courant (montage, timeline, dessin en animation) :
            // une copie du dessin (F5), ou vide (F4).
            "dessin.nouvelle_cle" | "dessin.nouvelle_cle_vide" => {
                let (pi, li, f) = (self.cur_panel, self.cur_layer, self.cur_frame);
                self.montage_new_key(pi, li, f, action == "dessin.nouvelle_cle");
            }
            // Ctrl+A choisit toutes les clés de la sorte qui a la main : caméra, effet, ou dessins
            // (les clés de caméra et d'effets donnent aussi `key_focus` : elles passent d'abord).
            "edition.tout_selectionner" if self.cam_keys_have_focus() => self.choose_all_cam_keys(),
            "edition.tout_selectionner" if self.fx_keys_have_focus() => self.choose_all_fx_keys(),
            "edition.tout_selectionner" if self.key_focus => self.choose_all_keys(),
            // Après un clic dans le montage (ou la planche), l'édition vise les panneaux choisis (12e).
            "edition.couper" if self.panels_have_focus() => self.cut_panels(),
            "edition.copier" if self.panels_have_focus() => self.copy_panels(),
            "edition.coller" if self.panels_have_focus() && self.panel_clipboard.is_some() => self.paste_panels(),
            // Après un Maj + clic dans la grille de la timeline, l'édition vise les images choisies (13c).
            "edition.couper" if self.images_have_focus() => self.cut_images(),
            "edition.copier" if self.images_have_focus() => self.copy_images(),
            "edition.coller" if self.images_focus && self.image_clipboard.is_some() => {
                self.paste_images(false)
            }
            "edition.supprimer" if self.images_have_focus() => self.delete_images(false),
            // Après un clic dans la grille, sans images choisies : Suppr supprime l'image courante
            // des calques choisis (audit A3) ; le dessin se vide après un clic dans le dessin.
            "edition.supprimer" if self.images_focus && !self.panels_have_focus() => self.delete_images(false),
            // Une sélection sur plusieurs calques copie, coupe et colle calque par calque.
            "edition.couper" if self.multi.sel.is_some() => self.multi_cut(),
            "edition.copier" if self.multi.sel.is_some() => {
                self.multi_copy();
            }
            "edition.coller" if self.multi.clip.is_some() && self.chosen_layers().len() >= 2 => self.multi_paste(),
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
                } else if self.multi.sel.is_some() {
                    // Plusieurs calques : ce qui est choisi sur chacun s'efface, en une étape.
                    self.multi_delete();
                } else if self.bsel.is_some() {
                    self.bitmap_erase(true);
                } else if self.vsel.as_ref().is_some_and(|s| !s.items.is_empty() && Some(s.cel) == self.cel_here()) {
                    // Des éléments vectoriels choisis, quel que soit l'outil : après un lasso pris
                    // avec Ctrl tenu, l'outil d'avant revient mais la sélection reste ; Suppr
                    // efface alors la sélection, pas tout le dessin (Anthony, 2026-10-18).
                    self.delete_selection();
                } else {
                    // Rien de choisi : Suppr ne fait rien (Anthony, 2026-10-18) ; vider tout le
                    // dessin est une commande à part (`dessin.vider`).
                    self.status = "Rien de sélectionné : Suppr n'efface rien (Édition > Vider le dessin pour tout effacer).".into();
                }
            }
            "dessin.vider" => self.clear_image(),
            "dessin.supprimer_hors_selection" => {
                if self.multi.sel.is_some() {
                    self.status = "Effacer hors de la sélection se fait sur un seul calque : choisis-en un seul (clic sans Ctrl ni Maj).".into();
                } else if self.bsel.is_some() {
                    self.bitmap_erase(false);
                } else if self.vsel.is_some() {
                    self.delete_unselected();
                } else {
                    self.status = "Rien de sélectionné : rien à garder.".into();
                }
            }
            // Dans la timeline : toutes les images de tous les calques (audit A2).
            "edition.tout_selectionner" if self.images_focus => self.select_whole_timeline(),
            "edition.tout_selectionner" => self.select_all(),
            "edition.tout_deselectionner" => {
                self.end_stroke();
                self.vsel = None;
                self.drop_bitmap_selection();
                self.drop_multi();
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
                if board_shown {
                    self.board_view = match self.board_view {
                        BoardView::Scene => BoardView::Panels,
                        BoardView::Panels => BoardView::Scene,
                    };
                } else {
                    self.set_mode(Mode::Storyboard);
                    self.board_view = BoardView::Panels;
                }
            }
            "mode.basculer" => self.set_mode(if board_shown { Mode::Animation } else { Mode::Storyboard }),
            "aide.raccourcis" => self.show_help = !self.show_help,
            "outil.pinceau" => self.set_tool(Tool::Brush),
            "outil.crayon" => self.set_tool(Tool::Pencil),
            "outil.gomme" => self.set_tool(Tool::Eraser),
            "outil.remplissage" => self.set_tool(Tool::Fill),
            "outil.pipette" => self.set_tool(Tool::Picker),
            "outil.selection" => self.set_tool(Tool::Select),
            "outil.selection_couleur" => self.set_tool(Tool::SelectColor),
            "outil.cutter" => self.set_tool(Tool::Cutter),
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
            "outil.perspective" => self.set_tool(Tool::Perspective),
            "deformer.enveloppe" => {
                self.set_deform_mode(crate::envelopetools::DeformMode::Envelope);
                self.set_tool(Tool::Perspective);
            }
            "deformer.perspective" => {
                self.set_deform_mode(crate::envelopetools::DeformMode::Perspective);
                self.set_tool(Tool::Perspective);
            }
            "selection.reinit_pivot" => self.reset_pivot(),
            "selection.proportions" => {
                self.keep_proportions = !self.keep_proportions;
                self.status = format!("Sélection : proportions gardées : {}.", if self.keep_proportions { "oui" } else { "non" });
            }
            "outil.rectangle" => self.set_tool(Tool::Rect),
            "outil.ellipse" => self.set_tool(Tool::Ellipse),
            "outil.polyligne" => self.set_tool(Tool::Polyline),
            "outil.main" => self.set_tool(Tool::Pan),
            "outil.taille_plus" => self.resize_brush(1.15),
            "outil.taille_moins" => self.resize_brush(1.0 / 1.15),
            "calque.miroir_h" => self.flip_layer(true),
            "calque.miroir_v" => self.flip_layer(false),
            "plan.retourner" => self.flip_panels(),
            "selection.miroir_h" if self.multi.sel.is_some() => self.multi_flip(true),
            "selection.miroir_v" if self.multi.sel.is_some() => self.multi_flip(false),
            "selection.miroir_h" if self.bsel.is_some() => self.bitmap_flip(true),
            "selection.miroir_v" if self.bsel.is_some() => self.bitmap_flip(false),
            "selection.miroir_h" => self.flip_selection(true),
            "selection.miroir_v" => self.flip_selection(false),
            "lecture.basculer" => self.toggle_play(),
            // Un bord de plan choisi dans le montage : les flèches le déplacent d'une image.
            "image.precedente" if self.edge_nudge_ready() => self.nudge_edge(-1),
            "image.suivante" if self.edge_nudge_ready() => self.nudge_edge(1),
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
            "scene.nouvelle" => self.new_panel(),
            "scene.dupliquer" => self.duplicate_panel(),
            "scene.supprimer" => self.delete_chosen_panels(),
            "scene.precedente" => self.step_scene(false),
            "scene.suivante" => self.step_scene(true),
            "sequence.nouvelle" => self.new_sequence(),
            "panneau.supprimer" => self.delete_chosen_panels(),
            "panneau.copier" => self.copy_panels(),
            "marque.basculer" => self.toggle_mark(),
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
            "calque.supprimer" => self.delete_chosen_layers(),
            "calque.dessus" => self.select_layer(self.cur_layer + 1),
            "calque.dessous" => self.select_layer(self.cur_layer.saturating_sub(1)),
            // Calques choisis et opérations sur plusieurs calques (13b, `layerops.rs`).
            "calque.nouveau" => self.open_new_layer_dialog(),
            "calque.dupliquer" => self.duplicate_chosen_layers(),
            "calque.supprimer_vides" => self.delete_empty_layers(),
            "calque.supprimer_caches" => self.delete_hidden_layers(),
            "calque.fusionner_choisis" => self.merge_chosen_layers(),
            "calque.fusionner_visibles" => self.merge_visible_layers(),
            "calque.fusionner_tout" => self.merge_all_layers(),
            "calque.choisir_tout" => self.choose_layers(Choose::All),
            "calque.choisir_visibles" => self.choose_layers(Choose::Visible),
            "calque.choisir_groupe" => self.choose_layers(Choose::SameGroup),
            "calque.choisir_autres_groupes" => self.choose_layers(Choose::OtherGroups),
            "calque.choisir_aucun" => self.choose_layers(Choose::Nothing),
            "calque.choisir_dessus" => self.extend_layer_choice(true),
            "calque.choisir_dessous" => self.extend_layer_choice(false),
            "calque.monter" => self.move_layer(true),
            "calque.descendre" => self.move_layer(false),
            "calque.premier" => self.select_end_layer(false),
            "calque.dernier" => self.select_end_layer(true),
            "calque.tous_visibles" => self.show_all_layers(),
            "calque.tous_caches" => self.hide_all_layers(),
            "calque.courant_seul" => self.show_current_layer_only(),
            "calque.pochoirs_eteindre" => self.stencils_off(),
            "calque.pochoirs_inverser" => self.stencils_invert(),
            "calque.pelure" => self.toggle_layer_onion(),
            "calque.couper" => self.cut_layer(),
            "calque.coller_remplacer" => self.paste_layer_replacing(),
            "groupes.editer" => self.groups_window = !self.groups_window,
            // Images de la timeline (13c-2, `images.rs`).
            "images.supprimer" => self.delete_images(false),
            "images.couper" => self.cut_images(),
            "images.copier" => self.copy_images(),
            "images.coller" => self.paste_images(false),
            "images.coller_remplacer" => self.paste_images(true),
            "images.dupliquer" => self.duplicate_images(),
            "images.inverser" => self.reverse_images(),
            "images.inserer_avant_calques" => self.insert_images(true, 1, false),
            "images.inserer_apres_calques" => self.insert_images(false, 1, false),
            "images.ajouter" => self.insert_images(false, 1, true),
            "images.inserer" => self.insert_images(true, 1, true),
            "images.supprimer_plan" => self.delete_images(true),
            "images.tout_choisir" => self.select_all_images(),
            "instance.precedente" => self.goto_instance(false),
            "instance.suivante" => self.goto_instance(true),
            "cle.precedente" => self.goto_key(false),
            "cle.suivante" => self.goto_key(true),
            "pinceau.derriere" => {
                self.paint_behind = !self.paint_behind;
                self.status = if self.paint_behind { "Peint derrière : la peinture passe sous ce qui est déjà dessiné." } else { "Peint derrière : non." }.into();
            }
            "pinceau.remplir" => {
                self.fill_shapes = !self.fill_shapes;
                self.status = if self.fill_shapes { "Remplir la forme : un trait fermé remplit son intérieur, au lâcher." } else { "Remplir la forme : non." }.into();
            }
            "instance.tete" => self.goto_instance_end(false),
            "instance.queue" => self.goto_instance_end(true),
            "instance.casser" => self.break_instance_here(),
            "instance.ajouter_avant" => self.add_instance_before(),
            "instance.exposition_plus" => self.change_chosen_exposures(true),
            "instance.exposition_moins" => self.change_chosen_exposures(false),
            "images.faux_fixe_boucle" => self.fake_hold_chosen(crate::images::Cycle::Loop),
            "images.faux_fixe_va_et_vient" => self.fake_hold_chosen(crate::images::Cycle::PingPong),
            "images.faux_fixe_hasard" => self.fake_hold_chosen(crate::images::Cycle::Random),
            "calque.etirer_tenir" => self.stretch_chosen(crate::images::Stretch::HoldLast),
            "calque.etirer_boucle" => self.stretch_chosen(crate::images::Stretch::Loop),
            "calque.etirer_va_et_vient" => self.stretch_chosen(crate::images::Stretch::PingPong),
            "calque.couper_apres" => self.cut_layers_after_current(),
            "images.retourner_h" => self.flip_images(true, false),
            "images.retourner_v" => self.flip_images(false, true),
            "images.retourner_hv" => self.flip_images(true, true),
            "instance.renommer" => match self.cel_here() {
                Some(cel) => self.start_instance_rename(self.cur_layer, cel),
                None => self.status = "Pas de dessin à cette image.".into(),
            },
            "instance.effacer_noms" => self.clear_instance_names(),
            "instance.tout_renommer" => self.renumber_instances(),
            "instance.renommer_intelligemment" => {
                self.smart_rename = Some(crate::images::SmartRename { empty: true, numbers: true, texts: true, ..Default::default() })
            }
            // Repères du plan (13d, `cues.rs`).
            "signet.basculer" => self.toggle_bookmark_here(),
            "signet.precedent" => self.goto_bookmark(false),
            "signet.suivant" => self.goto_bookmark(true),
            "signets.enlever_choisis" => self.clear_chosen_bookmarks(),
            "signets.tout_enlever" => self.clear_bookmarks(),
            "entree.poser" => self.set_cue_point(false, false),
            "entree.basculer" => self.set_cue_point(false, true),
            "sortie.poser" => self.set_cue_point(true, false),
            "sortie.basculer" => self.set_cue_point(true, true),
            "entree_sortie.enlever" => self.clear_cue_points(),
            "boucle.limites_choisies" => self.chosen_to_loop_limits(),
            "boucle.images_choisies" => self.toggle_loop_on_chosen(),
            "timeline.adapter_plan" => self.fit_timeline(crate::cues::Fit::Plan),
            "timeline.adapter_calque" => self.fit_timeline(crate::cues::Fit::Layer),
            "timeline.adapter_choix" => self.fit_timeline(crate::cues::Fit::Selection),
            "timeline.adapter_image" => self.fit_timeline(crate::cues::Fit::Image),
            "timeline.adapter_cles" => self.fit_timeline(crate::cues::Fit::Keys),
            "timeline.tout_voir" => self.fit_timeline(crate::cues::Fit::All),
            // Zoom de la bande du bas : le montage en storyboard, la timeline en animation.
            "bande.zoom_avant" => self.zoom_strip(1.25),
            "bande.zoom_arriere" => self.zoom_strip(1.0 / 1.25),
            "bande.tout_voir" if board_shown => self.montage_fit = true,
            "bande.tout_voir" => self.fit_timeline(crate::cues::Fit::All),
            // Marques de couleur (13d, `marks.rs`).
            "marque_couleur.basculer" => self.toggle_color_mark(),
            "marque_couleur.retirer" => self.remove_color_mark(),
            "marque_couleur.toutes" => self.mark_all_instances(false),
            "marque_couleur.non_marquees" => self.mark_all_instances(true),
            "marque_couleur.effacer_calque" => self.clear_color_marks(false),
            "marque_couleur.effacer_plan" => self.clear_color_marks(true),
            "marque_couleur.precedente" => self.goto_color_mark(false, false),
            "marque_couleur.suivante" => self.goto_color_mark(true, false),
            "marque_couleur.precedente_couleur" => self.goto_color_mark(false, true),
            "marque_couleur.suivante_couleur" => self.goto_color_mark(true, true),
            // Onglets du bas et arrière-plan (13e).
            "onglet.notes" => self.toggle_bottom_tab(crate::bottomtabs::BottomTab::Notes),
            "onglet.son" => self.toggle_bottom_tab(crate::bottomtabs::BottomTab::Sound),
            "onglet.groupes" => self.toggle_bottom_tab(crate::bottomtabs::BottomTab::Groups),
            "onglet.arriere_plan" => self.toggle_bottom_tab(crate::bottomtabs::BottomTab::Background),
            "onglet.image_depart" => self.toggle_bottom_tab(crate::bottomtabs::BottomTab::StartFrame),
            "fond.aucun" => self.set_background(crate::document::BackgroundKind::None),
            "fond.couleur" => self.set_background(crate::document::BackgroundKind::Color),
            "fond.damier" => self.set_background(crate::document::BackgroundKind::Checker),
            // Vue Projet (13e-4, `projetui.rs`).
            "plan.nouveau_avant" => self.new_panel_before(),
            // Passe interface : des icônes qui n'avaient pas d'action (et donc pas de raccourci).
            "plan.avancer" => self.shift_panel(false),
            "plan.reculer" => self.shift_panel(true),
            "lecture.boucle" => {
                self.looping = !self.looping;
                self.status = if self.looping { "Lecture en boucle.".into() } else { "Lecture une seule fois.".into() };
            }
            "storyboard.verrouiller" => self.toggle_storyboard_lock(),
            // Rognage des plans (`rognageui.rs`).
            "montage.duree" => self.open_duration_dialog(),
            "montage.debut_a_tete" => self.trim_head_to_playhead(),
            "montage.fin_a_tete" => self.trim_tail_to_playhead(),
            "montage.rognage_retablir" => self.restore_trimmed(),
            "montage.rognage_oublier" => self.forget_trimmed(),
            "montage.aimanter" => {
                self.snap_on = !self.snap_on;
                self.status = format!("Aimantation des bords du montage : {}.", if self.snap_on { "oui" } else { "non" });
            }
            "projet.proprietes" => self.properties_window = !self.properties_window,
            "projet.nouveau_depuis_plans" => self.request(Pending::FromChosen),
            // Panneau de la pelure d'oignon (13f, `onionui.rs`).
            "pelure.panneau" => self.onion_window = !self.onion_window,
            "pelure.devant" => self.onion.front = !self.onion.front,
            "pelure.lier" => {
                let linked = !self.onion.linked;
                self.onion.set_linked(linked);
            }
            "pelure.activer_tout" => self.onion.show_all(true),
            "pelure.desactiver_tout" => self.onion.show_all(false),
            "pelure.mode_instance" => self.set_onion_mode(crate::onion::OnionMode::Drawings),
            "pelure.mode_image" => self.set_onion_mode(crate::onion::OnionMode::Frames),
            "pelure.mode_signet" => self.set_onion_mode(crate::onion::OnionMode::Bookmarks),
            "pelure.mode_marque" => self.set_onion_mode(crate::onion::OnionMode::Marks),
            "pelure.decaler" => self.onion_shift = !self.onion_shift,
            "pelure.remettre" => self.reset_onion_offset(),
            // Feuille d'exposition (13g, `xsheetui.rs`).
            // (`vue.feuille_exposition` et `vue.historique` : barre des panneaux, `workspace_action`.)
            // Rôles des images (13g-2, `rolesui.rs`).
            "role.cle" => self.set_role(Some(crate::roles::Role::Key)),
            "role.breakdown" => self.set_role(Some(crate::roles::Role::Breakdown)),
            "role.intervalle" => self.set_role(Some(crate::roles::Role::Inbetween)),
            "role.intervalle_complete" => self.set_role(Some(crate::roles::Role::InbetweenDone)),
            "role.intervalle_special" => self.set_role(Some(crate::roles::Role::Special)),
            "role.intervalle_special_complete" => self.set_role(Some(crate::roles::Role::SpecialDone)),
            "role.aucun" => self.set_role(None),
            "role.timeline" => self.roles_view.shown = !self.roles_view.shown,
            "role.calque_courant" => self.roles_view.current_only = !self.roles_view.current_only,
            // Flips (13h, `flipsui.rs`) : `flip.jouer` et les flips par rôle durent tant que leur
            // touche est tenue.
            "flip.panneau" => self.flips_window = !self.flips_window,
            "flip.jouer" => self.hold_current_flip("flip.jouer"),
            "flip.precedent" => self.flip_step(-1),
            "flip.suivant" => self.flip_step(1),
            "flip.changer" => self.next_flip(),
            a if a.starts_with("flip.") && self.role_flip_action(a) => {}
            // Pile d'effets (13i, `fxui.rs`).
            "fx.cle_creer" => self.fx_create_key(),
            "fx.cle_supprimer" => self.fx_delete_key(),
            "fx.cle_auto" => {
                self.fx_auto_key = !self.fx_auto_key;
                self.status = format!("Création automatique des clés d'effets : {}.", if self.fx_auto_key { "oui" } else { "non" });
            }
            "fx.cles_timeline" => self.fx_keys_shown = !self.fx_keys_shown,
            // Copier-coller des effets d'un calque à un autre (`fxcopie.rs`).
            "fx.copier" | "fx.copier_tout" | "fx.coller" | "fx.remplacer" => {
                self.fx_clip_action(action);
            }
            // Tremblement du trait (`tremblement.rs`) du calque courant.
            "fx.tremblement" => self.toggle_fx(self.cur_layer, crate::fx::EffectKind::Boil),
            "fx.tremblement_repos" => self.set_boil_rest(self.cur_layer, !self.boil_rest_on(self.cur_layer)),
            // Effets du trait (`fxtrait.rs`) du calque courant, et leurs préréglages.
            a if crate::fxtrait::kind_of_action(a).is_some() => {
                if let Some(kind) = crate::fxtrait::kind_of_action(a) {
                    self.toggle_fx(self.cur_layer, kind);
                }
            }
            a if crate::fxtrait::Preset::from_action(a).is_some() => {
                if let Some(preset) = crate::fxtrait::Preset::from_action(a) {
                    self.apply_trait_preset(preset);
                }
            }
            // Effets de matière (`fxmatiere.rs`) du calque courant, leurs préréglages et « Appliquer pour de bon ».
            a if crate::fxmatiere::kind_of_action(a).is_some() => {
                if let Some(kind) = crate::fxmatiere::kind_of_action(a) {
                    self.toggle_fx(self.cur_layer, kind);
                }
            }
            a if crate::fxmatiere::Preset::from_action(a).is_some() => {
                if let Some(preset) = crate::fxmatiere::Preset::from_action(a) {
                    self.apply_matter_preset(preset);
                }
            }
            "fx.plume_appliquer" => self.bake_nib(self.cur_layer),
            "fx.choisir_aucun" => {
                self.deselect_fx();
                self.status = "Aucun effet choisi : plus de clé d'effet dans la timeline ni le montage.".into();
            }
            // Calque CTG (13j, `ctgui.rs`).
            "calque.nouveau_ctg" => self.new_ctg_layer(),
            "calque.nouveau_couleur" => self.new_colour_layer(),
            "calque.detacher_couleur" => self.detach_colour_layer(),
            "calque.transformer_ctg" => self.convert_to_ctg(),
            "calque.desactiver_ctg" => self.disable_ctg(),
            "ctg.charger_structure" => self.load_ctg_structure(false),
            "ctg.charger_structure_effacer" => self.load_ctg_structure(true),
            // Calque vidéo (13k, `videoui.rs`).
            "fichier.importer_video" => self.import_video(),
            "fichier.importer_sboard" => self.open_sb_window(),
            "fichier.importer_scenario" => self.open_scenario_window(),
            "fichier.exporter_sboard" => self.open_sb_export(),
            "pinceau.enregistrer_traits" => {
                self.record_strokes = match self.record_strokes {
                    Some(_) => {
                        self.status = "Banc de traits arrêté.".into();
                        None
                    }
                    None => {
                        self.status = "Banc de traits : chaque trait est maintenant enregistré, tel que le stylet le donne (Aide > Enregistrer mes traits pour arrêter).".into();
                        Some(0)
                    }
                };
            }
            "fichier.importer_sequence" => self.import_sequence(),
            "calque.video_en_animation" => self.video_to_animation(),
            "calque.propager" => self.open_propagation(),
            "calque.colorier_zones" => self.open_coloriage(),
            // Interpolation d'images (13l, `tweenui.rs`).
            "images.intermediaire" => self.insert_intermediates(1),
            "images.intermediaires" => self.open_tween_dialog(crate::tweenui::TweenKind::Insert),
            "images.intervalles" => self.fill_gaps(),
            "images.etirer_interpoler" => self.open_tween_dialog(crate::tweenui::TweenKind::Stretch),
            "projet.changer_cadence" => self.open_tween_dialog(crate::tweenui::TweenKind::Fps),
            // Dossiers de calques (13b-5, `folders.rs`).
            "dossier.nouveau" => self.new_folder(),
            "dossier.ranger" => self.folder_from_chosen(),
            "dossier.deposer" => self.drop_chosen_in_folder(),
            "dossier.sortir" => self.take_chosen_out(),
            "dossier.supprimer_seul" => self.delete_folders_only(),
            "dossier.joindre" => self.join_chosen_folders(),
            "groupes.focus" => self.focus_current_group(),
            "panneau.couper" => self.split_current_panel(),
            "panneau.fusionner_suivant" => self.merge_next_panel(),
            "panneau.fusionner_choisis" => self.merge_chosen_panels(),
            "scene.couper" => self.split_current_panel(),
            "scene.joindre_precedente" => self.join_previous_scene(),
            "calque.copier" => self.copy_layer(),
            "calque.coller" => self.paste_layer(),
            "calque.verrouiller" => self.toggle_layer_lock(self.cur_layer),
            "calque.fusionner_dessous" => self.merge_layer_down(),
            "calque.nouveau_fond" => self.add_scene_background(),

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
            "vue.performance" => {
                crate::perf::activer(!crate::perf::actif());
                self.ctx.request_repaint();
            }
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
        self.shape_key(key) || self.persp_key(key) || self.envelope_key(key)
    }

    /// Change d'outil : la forme en cours (courbe, polyligne) est posée avant.
    pub(crate) fn set_tool(&mut self, tool: Tool) {
        if self.tool != tool {
            self.finish_field_edit();
            self.finish_shape_edit();
            self.finish_text_edit();
            self.persp = None;
            self.envelope = None;
            self.end_stroke();
            self.tool = tool;
            if tool.is_shape() {
                self.shape.last = tool;
            }
            // La case de la barre d'outils montre le dernier outil pris parmi les siens.
            self.tool_recent.retain(|t| *t != tool);
            self.tool_recent.insert(0, tool);
        }
    }

    /// Zoom de la bande du bas par le facteur `f` : le montage en storyboard, la timeline en
    /// animation. La tête de lecture reste au même endroit de l'écran (`zoombande.rs`).
    pub(crate) fn zoom_strip(&mut self, f: f32) {
        use crate::zoombande::Strip;
        match self.mode {
            Mode::Storyboard => {
                self.anchor_strip_zoom(Strip::Montage);
                self.montage_zoom = (self.montage_zoom * f).clamp(0.5, 16.0);
            }
            Mode::Animation => {
                self.anchor_strip_zoom(Strip::Timeline);
                self.timeline_cell = (self.timeline_cell * f).clamp(crate::timeline::CELL_MIN, crate::timeline::CELL_MAX);
            }
        }
    }

    /// Zoom par pas, autour du centre de la vue.
    fn zoom_step(&mut self, factor: f32) {
        let (rect, size) = (self.canvas_rect, self.canvas_size());
        self.view.zoom_around(rect, size, rect.center(), factor);
    }

    /// Va à une image du plan courant (dans le montage en mode storyboard).
    fn goto_panel_frame(&mut self, frame: usize) {
        match self.mode {
            Mode::Storyboard => self.goto_global(self.doc.panel_start(self.cur_panel) + frame),
            Mode::Animation => self.goto_frame(frame),
        }
    }

    /// Allonge ou raccourcit le panneau d'une image (une étape d'annulation par appui).
    fn nudge_panel_length(&mut self, delta: i64) {
        if self.storyboard_refuses() {
            return;
        }
        let n = (self.frame_count() as i64 + delta).max(1) as usize;
        if n == self.frame_count() {
            return;
        }
        self.end_stroke();
        let snap = self.snapshot();
        // Raccourcir garde le reste en réserve (`rognage.rs`) : « Allonger » le rend d'abord.
        self.doc.set_duration(self.cur_panel, n);
        self.length_origin = None;
        self.commit_structure(snap);
        self.status = format!(
            "Durée du plan : {}",
            crate::storyboard::duration_label(n, self.doc.fps)
        );
    }

    /// Recharge les raccourcis (après modification du fichier).
    pub(crate) fn reload_keymap(&mut self) {
        self.keymap = Keymap::load(self.settings.keymap_preset);
        let jeu = if self.keymap.preset.is_default() { String::new() } else { format!(" Jeu « {} ».", self.keymap.preset.label()) };
        self.status = match (&self.keymap.user_file, self.keymap.warnings.len()) {
            (Some(p), 0) => format!("Raccourcis rechargés depuis {}.{jeu}", p.display()),
            (Some(p), n) => format!("Raccourcis rechargés depuis {} ({n} remarque(s), voir F1).{jeu}", p.display()),
            (None, 0) => format!("Raccourcis par défaut.{jeu}"),
            (None, n) => format!("Raccourcis par défaut ({n} remarque(s), voir F1).{jeu}"),
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
                    "Raccourcis ouverts dans le Bloc-notes ({}). Enregistre, puis « Recharger » (Fichier > Préférences…, ou la liste des raccourcis, F1).",
                    path.display()
                )
            }
            Err(e) => self.status = format!("Impossible d'ouvrir {} : {e}", path.display()),
        }
    }

    fn handle_close_request(&mut self, ctx: &egui::Context) {
        // (Pendant un enregistrement, la fermeture l'attend : `request` retient la demande.)
        if ctx.input(|i| i.viewport().close_requested()) && (self.modified || self.saving.is_some()) && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request(Pending::Quit);
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Sans titre".into());
        let title = format!("{name}{} — SHOT", if self.modified || self.saving.is_some() { " •" } else { "" });
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

    /// Fermeture normale (même sans enregistrer) : voir `App::fermeture`.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.fermeture();
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
        self.frame(ctx);
    }
}

impl App {
    /// Une image d'écran : tout ce que `update` fait une fois le rythme réglé. Les mesures de
    /// `perf_tests.rs` l'appellent directement (sans fenêtre, sans sommeil).
    pub(crate) fn frame(&mut self, ctx: &egui::Context) {
        crate::perf::begin_frame(ctx);
        // Plein écran à l'ouverture (Anthony, 2026-10-18) : la fenêtre, créée cachée, perd la
        // consigne de `with_maximized` sous Windows : elle est marquée maximisée mais garde sa taille.
        // Une fois affichée, on la remet à sa taille, puis on la maximise vraiment.
        if self.maximized_once < 4 {
            self.maximized_once += 1;
            match self.maximized_once {
                3 => ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false)),
                4 => ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true)),
                _ => {}
            }
            ctx.request_repaint();
        }
        self.ppp = ctx.pixels_per_point();
        // Thème changé dans les préférences : les couleurs sont reposées une fois, et tout
        // l'écran suit dès cette image.
        if self.settings.theme != self.theme_applied {
            self.theme_applied = self.settings.theme;
            crate::theme::installer(&self.theme_applied);
            crate::theme::appliquer(ctx);
            self.theme_unsaved = true;
        }
        // Enregistré une fois le sélecteur de couleur relâché, pas à chaque image du glissement.
        if self.theme_unsaved && !ctx.input(|i| i.pointer.any_down()) {
            self.theme_unsaved = false;
            let _ = self.settings.save();
        }
        // Réglages de performance (Préférences, Aide > Diagnostic des performances) : posés dans
        // le programme dès qu'ils changent.
        self.appliquer_performance(ctx);
        // Effets de calque : l'horloge de leurs demandes avance (calculs de fond, `fxcache.rs`).
        crate::fx::begin_frame(ctx);
        let prep = crate::perf::section("préparation (raccourcis, lecture)");
        self.clamp_nav();
        self.flush_flottante_on_input(ctx);
        // Un champ de la boîte de sélection qui n'a plus la main termine son geste.
        self.finish_field_edit_if_idle();
        self.end_length_gesture_if_idle();
        self.end_gesture_if_idle();
        self.handle_close_request(ctx);
        self.update_springs(ctx);
        // Un nouvel appui lâche le commentaire (Suppr ne le vise plus) ; s'il tombe sur un repère
        // ou dans le panneau Commentaires, l'interface le reprend pendant cette image.
        if ctx.input(|i| i.pointer.any_pressed()) {
            self.comment_ui.focus = false;
            // De même pour le panneau Effets : s'il est sous le clic, il reprend la main pendant
            // cette image (`fxcopie.rs`).
            self.fx_pane_focus = false;
            // Et pour le panneau Références (`referencesui.rs`).
            self.refs.focus = false;
        }
        self.handle_shortcuts(ctx);
        self.sync_scene_layers();
        // Calques de couleur : leurs images suivent celles du trait (`calquecouleur.rs`).
        self.sync_couleurs();
        self.warm_couleur();
        if !self.modal_open() {
            self.handle_dropped_files(ctx);
        }
        self.tick_playback(ctx);
        self.tick_flip(ctx);
        self.tick_video_import(ctx);
        self.tick_references(ctx);
        self.propagation_ui(ctx);
        self.coloriage_ui(ctx);
        if self.playing {
            let now = ctx.input(|i| i.time);
            let g = self.global_frame();
            self.play_meter.tick(now, g);
        } else {
            self.play_meter.reset();
        }
        self.validate_selection();
        self.validate_bitmap_selection();
        self.multi_sync();
        drop(prep);
        {
            let _t = crate::perf::section("arrière-plan (rejeu, fonds)");
            self.poll_rejeu(ctx);
            self.rejeu_idle_tick(ctx);
            self.warm_current_cel();
        }
        self.update_title(ctx);
        {
            let _t = crate::perf::section("vignettes (nettoyage)");
            self.thumbs.prune(&self.doc);
        }
        {
            // (Les vignettes finies sur les fils de fond arrivent à la carte graphique ; aucune n'est composée ici.)
            let _t = crate::perf::section("vignettes (arrivée)");
            self.thumbs.begin_frame(ctx);
            self.layer_thumbs.begin_frame(ctx);
        }

        {
            let _t = crate::perf::section("barre de menus");
            egui::TopBottomPanel::top("menu").show(ctx, |ui| self.menu_bar(ui));
        }
        if self.layout.command_bar {
            let _t = crate::perf::section("barre de commandes");
            egui::TopBottomPanel::top("commandes").show(ctx, |ui| self.command_bar(ui));
        }
        {
            let _t = crate::perf::section("barre d'état");
            egui::TopBottomPanel::bottom("statut").show(ctx, |ui| self.status_bar(ui));
        }
        if self.layout.tool_bar {
            let _t = crate::perf::section("barre d'outils");
            egui::SidePanel::left("barre_outils")
                .exact_width(44.0)
                .resizable(false)
                .show(ctx, |ui| self.vertical_tool_bar(ui));
        }
        // La barre des panneaux, sur le bord droit : avant les panneaux, qui prennent le reste.
        if self.settings.panel_bar {
            let _t = crate::perf::section("barre des panneaux");
            let frame = egui::Frame::side_top_panel(&ctx.style()).inner_margin(egui::Margin::symmetric(2, 4));
            egui::SidePanel::right("barre_panneaux")
                .exact_width(crate::barrepanneaux::LARGEUR)
                .resizable(false)
                .frame(frame)
                .show(ctx, |ui| self.panel_bar(ui));
        }
        // Les panneaux : onglets que l'on glisse, aimante ou fait flotter (5b).
        {
            let _t = crate::perf::section("panneaux (dock, total)");
            self.show_dock(ctx);
        }

        {
            let _t = crate::perf::section("fenêtres et dialogues");
            self.dialogs(ctx);
            self.top_view_window(ctx);
            self.tick_open(ctx);
            self.tick_reopen();
            self.poll_brushes();
            self.poll_bruitages(ctx);
            self.poll_suggestions(ctx);
            self.tick_save(ctx);
            self.envois_tick(ctx);
            self.envois_windows(ctx);
            self.comparer_fenetre(ctx);
            self.purge_versions_fenetre(ctx);
        }
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
        self.drop_stale_drags(ctx.input(|i| i.pointer.any_down()));
        self.watch_plan_length();
        if self.thumbs.pending {
            ctx.request_repaint();
        }
        // Réglages de la pelure, de la feuille d'exposition, des flips et des options du pinceau
        // (peint derrière, remplir la forme) : enregistrés quand ils
        // changent (pas pendant un glissé).
        let options = (self.paint_behind, self.fill_shapes) != (self.settings.paint_behind, self.settings.fill_shapes);
        let changed = self.onion != self.settings.onion || self.xsheet != self.settings.xsheet || self.flips != self.settings.flips || options || self.toucher != self.settings.toucher || self.pinceaux != self.settings.brushes_panel || self.cadrage != self.settings.framing;
        if changed && !ctx.input(|i| i.pointer.any_down()) {
            self.settings.framing = self.cadrage.clone();
            self.settings.onion = self.onion.clone();
            self.settings.xsheet = self.xsheet.clone();
            self.settings.flips = self.flips.clone();
            (self.settings.paint_behind, self.settings.fill_shapes) = (self.paint_behind, self.fill_shapes);
            self.settings.toucher = self.toucher;
            self.settings.brushes_panel = self.pinceaux.clone();
            if let Err(e) = self.settings.save() {
                self.status = format!("Réglages non enregistrés : {e}");
            }
        }
        // Couleurs et nuanciers : enregistrés quand ils changent (passe couleur).
        self.color_tick(ctx);
        self.tick_recovery(ctx);
        {
            let _t = crate::perf::section("disposition (mémoire)");
            self.remember_layout(ctx);
        }
        if crate::perf::actif() {
            crate::perf::file_attente("rejeu", usize::from(self.rejeu.has_pending()) + self.rejeu_deferred.len() + self.rejeu_again.len());
            crate::perf::file_attente("fonds", usize::from(crate::fx::fond_pending()));
            crate::perf::file_attente("vignettes", self.thumbs.outstanding());
            crate::perf::overlay(ctx);
        }
        crate::perf::end_frame(ctx);
        // (Minuterie du démarrage : la première interface est construite.)
        crate::demarrage::premiere_image();
    }
}

impl App {
    /// Fermeture normale (même sans enregistrer) : la copie de secours de la séance est effacée,
    /// la disposition de la fenêtre retenue.
    pub(crate) fn fermeture(&mut self) {
        // Un enregistrement qui dure encore est attendu : on ne perd jamais un projet à la fermeture.
        self.finish_save_on_exit();
        if let Some(r) = self.recovery.take() {
            r.close();
        }
        let current = self.current_layout();
        self.settings.set_layout_of(self.espace, current);
        let _ = self.settings.save();
        // Le serveur de génération sonore que SHOT a lancé s'arrête avec lui (un serveur lancé à la main reste).
        self.sug.serveur.arreter();
    }
}

