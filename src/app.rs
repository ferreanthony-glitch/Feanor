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

/// Les calques du plan courant à un instant (`App::empreinte_calques`) : de quoi savoir si une liste peinte rang par rang vaut
/// encore (`App::calques_tiennent`).
pub(crate) struct EmpreinteCalques {
    plan: u64,
    ids: Vec<u64>,
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
    /// Mélangeur (`melangeur.rs`) : la brosse courante en « Lisser », qui entraîne et mêle ce qui est déjà peint, sans poser de couleur.
    Mixer,
    Pencil,
    Eraser,
    /// Cutter : coupe les traits vectoriels à leurs croisements (`cutter.rs`).
    Cutter,
    Fill,
    /// Dégradé (`degrade.rs`, `degradeui.rs`) : glisser trace l'axe, de la couleur d'avant-plan à celle d'arrière-plan.
    Gradient,
    /// Formes (5c) : outils séparés, comme des outils à part.
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
    /// Texte (5d).
    Text,
    /// Déformer en perspective (5f).
    Perspective,
    /// Grab : saisir les points sous le pinceau et les tirer (`sculpt.rs`, `sculptapp.rs`).
    Grab,
    /// Pull : étirer le trait avec le pinceau, comme un doigt (`sculpt.rs`, `sculptapp.rs`).
    Pull,
}

impl Tool {
    pub const ALL: [Tool; 21] = [
        Tool::Brush,
        Tool::Mixer,
        Tool::Pencil,
        Tool::Eraser,
        Tool::Cutter,
        Tool::Fill,
        Tool::Gradient,
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
        Tool::Grab,
        Tool::Pull,
    ];

    /// Les outils des Vignettes (partie 113, décision 2) : le pinceau, le crayon, la gomme, la caméra de la planche, et la main. Les
    /// autres restent au Storyboard ; sur la planche, choisir se fait avec Ctrl + glisser (le lasso), sans changer d'outil.
    pub fn permis_dans_les_vignettes(self) -> bool {
        matches!(self, Tool::Brush | Tool::Pencil | Tool::Eraser | Tool::Camera | Tool::Pan)
    }

    pub fn label(self) -> &'static str {
        match self {
            Tool::Brush => "Pinceau",
            Tool::Mixer => "Mélangeur",
            Tool::Pencil => "Crayon",
            Tool::Eraser => "Gomme",
            Tool::Cutter => "Cutter",
            Tool::Fill => "Remplir",
            Tool::Gradient => "Dégradé",
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
            Tool::Text => "Texte",
            Tool::Perspective => "Déformer",
            Tool::Grab => "Grab",
            Tool::Pull => "Pull",
        }
    }

    /// Petite description, pour la bulle d'aide de son bouton (12d).
    pub fn description(self) -> &'static str {
        match self {
            Tool::Brush => "Peindre ; la taille peut suivre la pression du stylet.",
            Tool::Mixer => "Mélanger ce qui est déjà peint : la brosse courante entraîne et étale les couleurs qu'elle traverse, sans en poser de neuve. Sur un calque bitmap.",
            Tool::Pencil => "Dessiner au crayon.",
            Tool::Eraser => "Effacer ; sur un calque vectoriel, des bouts de trait ou des traits entiers.",
            Tool::Cutter => "Glisser en travers des bouts de traits vectoriels à enlever : ils sont coupés là où ils croisent les autres traits.",
            Tool::Fill => "Remplir une zone avec la couleur courante.",
            Tool::Gradient => "Glisser pour tracer l'axe d'un dégradé, de la couleur d'avant-plan à celle d'arrière-plan : dans la sélection, dans la zone fermée où l'on appuie, ou (Alt) sur tout le calque. Sur un calque vectoriel, il reste un réglage de la zone.",
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
            Tool::Text => "Clic : poser un texte ; Ctrl + Entrée pour le valider.",
            Tool::Perspective => "Déformer le dessin, la sélection ou la zone choisie : en enveloppe de Bézier (coins et poignées pour courber les côtés) ou en perspective (les quatre coins). Le mode se choisit dans Réglages.",
            Tool::Grab => "Saisir les points du dessin sous le cercle du pinceau et les tirer d'un coup : chaque point suit le curseur, plus ou moins selon sa distance au centre (atténuation, force et rayon dans Réglages). Plusieurs clés choisies : la même déformation sur chacune.",
            Tool::Pull => "Étirer le dessin en glissant, comme un doigt : à chaque instant, les points sous le cercle du pinceau suivent le mouvement (atténuation, force et rayon dans Réglages). Plusieurs clés choisies : la même déformation sur chacune.",
        }
    }

    /// Icône de l'outil (police d'icônes Phosphor, chargée au démarrage).
    pub fn icon(self) -> &'static str {
        use egui_phosphor::regular as ph;
        match self {
            Tool::Brush => ph::PAINT_BRUSH,
            Tool::Mixer => ph::DROP_HALF_BOTTOM,
            Tool::Pencil => ph::PENCIL_SIMPLE,
            Tool::Eraser => ph::ERASER,
            Tool::Cutter => ph::KNIFE,
            Tool::Fill => ph::PAINT_BUCKET,
            Tool::Gradient => ph::GRADIENT,
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
            Tool::Text => ph::TEXT_T,
            Tool::Perspective => ph::PERSPECTIVE,
            Tool::Grab => ph::HAND_GRABBING,
            Tool::Pull => ph::HAND_POINTING,
        }
    }

    /// Action qui choisit cet outil (son raccourci est dans le fichier de raccourcis).
    pub fn action(self) -> &'static str {
        match self {
            Tool::Brush => "outil.pinceau",
            Tool::Mixer => "outil.melangeur",
            Tool::Pencil => "outil.crayon",
            Tool::Eraser => "outil.gomme",
            Tool::Cutter => "outil.cutter",
            Tool::Fill => "outil.remplissage",
            Tool::Gradient => "outil.degrade",
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
            Tool::Text => "outil.texte",
            Tool::Perspective => "outil.perspective",
            Tool::Grab => "outil.grab",
            Tool::Pull => "outil.pull",
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
    /// D'où il vient (cadre, feuille, cadence) : le calque se colle aussi dans un autre onglet, adapté à sa feuille
    /// (`adaptation.rs`).
    pub origine: crate::adaptation::Origine,
    /// D'où il vient : l'onglet et le fichier du projet. Collé dans le même projet, un calque de personnage garde son lien au
    /// personnage ; dans un autre projet, il devient un calque ordinaire.
    pub projet: (u64, Option<std::path::PathBuf>),
    /// Les autres calques choisis avec lui (Ctrl ou Maj + clic dans le panneau Calques), du plus bas au plus haut : copiés et
    /// collés ensemble, dans le même ordre.
    pub extras: Vec<LayerClip>,
}

pub(crate) use crate::nouveauui::NewDocDialog;

pub struct App {
    pub(crate) ctx: egui::Context,
    pub(crate) doc: Document,
    pub(crate) mode: Mode,
    pub(crate) board_view: BoardView,
    /// État de la vue Vignettes (`boardui.rs`) : appuis, bande de note en cours d'écriture, case vide…
    pub(crate) vignettes: crate::boardui::Vignettes,
    /// Brosse par personnage (`personnagesui.rs`) : le personnage qui dessine, la fenêtre « Modifier » et le fil
    /// des fonds de trait ; les personnages eux-mêmes sont dans le projet (`Document::personnages`).
    pub(crate) perso: crate::personnagesui::Etat,
    /// Mon panneau (essai du Labo, `monpanneauui.rs`) : la recherche du « + », le nom qu'on saisit, le bouton qu'on glisse.
    pub(crate) mon_panneau: crate::monpanneauui::Etat,
    /// Le panneau Scénario (`scenariopanneau.rs`) : les hauteurs gardées, le plan montré, le défilement demandé.
    pub(crate) scenario_ui: crate::scenariopanneau::Etat,
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
    pub(crate) reopen_probe: Option<(Vec<PathBuf>, std::sync::mpsc::Receiver<Vec<PathBuf>>)>,
    pub(crate) fill: FillSettings,
    /// Réglages de l'outil Dégradé (`degrade.rs`) : forme, fin, opacité, inverser, tout le calque.
    pub(crate) degrade: crate::degrade::Reglages,
    /// L'interface des Réglages du dégradé : pastille choisie, sélecteur de couleur, retouches déjà proposées (`degradeui.rs`).
    pub(crate) degrade_ui: crate::degradeui::EtatUi,
    /// Le geste du dégradé en cours : l'axe tiré, ce qu'il remplit, son aperçu (`degradeui.rs`).
    pub(crate) degrade_geste: Option<crate::degradeui::Geste>,
    pub(crate) shape: ShapeSettings,
    pub(crate) vector_erase_whole: bool,
    /// Option du pinceau et du crayon (`brushfill`) : « Peindre derrière » (la peinture passe sous ce qui est déjà dessiné
    /// sur le calque). (« Remplir la forme » n'existe plus, 2026-10-23 : il faisait double emploi avec l'aplat.)
    pub(crate) paint_behind: bool,
    /// Option du pinceau et du crayon : « Peint sur l'aplat » (la peinture va directement dans l'aplat du dessin, calque de
    /// couleur ou strate, pas sur le trait). Exclusive avec `paint_behind`.
    pub(crate) paint_on_aplat: bool,    /// Toucher du stylet (sensation des brosses) : la copie de travail du réglage, enregistrée
    /// dans `update` quand elle change (hors glissé), comme les options du pinceau.
    pub(crate) toucher: crate::brush::Toucher,
    /// État de la liste des pinceaux (dans les réglages de l'outil) retenu d'une séance à l'autre (dépliée, liste ou
    /// vignettes, dossiers).
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
    /// Les autres effets choisis avec lui, sur le même calque (Ctrl / Maj + clic sur leur nom) : leurs
    /// rangs dans la pile. Seul `fx_choice` montre ses clés ; tous se copient d'un coup (`fxcopie.rs`).
    pub(crate) fx_multi: Vec<usize>,
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
    /// La dernière chose copiée est un ou des effets (`fx_pile_recent`), et le panneau Calques a la main
    /// (`layers_pane_focus`, le dernier clic était dedans) : Ctrl+V y colle ces effets sur le calque cliqué.
    pub(crate) fx_pile_recent: bool,
    pub(crate) layers_pane_focus: bool,
    /// La dernière chose copiée est un calque (copy_layer) : Ctrl+V, depuis le panneau Calques, le colle (même dans un autre plan).
    pub(crate) layer_clip_recent: bool,
    /// Import vidéo en cours (13k, `videoui.rs`).
    pub(crate) video_import: Option<crate::videoui::VideoImport>,
    /// Propagation d'une peinture sur la vidéo, façon EbSynth (`propagerui.rs`).
    pub(crate) propagation: crate::propagerui::Propagation,
    /// Coloriage par zones : propager des aplats de couleur d'une image à l'autre (`coloriageui.rs`).
    pub(crate) coloriage: crate::coloriageui::Coloriage,
    /// La fusion de calques en tâche de fond, quand elle est longue (`fusiontache.rs`).
    pub(crate) fusion: Option<crate::fusiontache::Tache>,
    /// Fenêtre « Importer un scénario » (`scenarioui.rs`).
    pub(crate) scenario_window: Option<crate::scenarioui::ScenarioWindow>,
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
    /// Tracé droit (Maj tenue pendant un trait, `tracedroit.rs`).
    pub(crate) droit: crate::tracedroit::Droit,
    /// Clés de position choisies dans la timeline et clés en cours de glisser (`poskeysui.rs`).
    pub(crate) pos_ui: crate::poskeysui::PosUi,
    /// Clés d'opacité choisies dans la timeline et clés en cours de glisser (`opacitecleselui.rs`).
    pub(crate) op_ui: crate::opacitecleselui::OpUi,
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
    /// Vue à travers la caméra du plan (sinon : la feuille, avec le cadre tracé).
    pub(crate) camera_view: bool,
    /// Vue du plateau laissée en passant à la vue caméra, remise au retour (9d).
    pub(crate) stage_view: Option<View>,
    /// Cadre de format personnalisé choisi (même s'il vaut l'un des formats proposés).
    pub(crate) format_custom: bool,
    /// Espace 3D (`espace3dui.rs`, `docs/ESPACE3D.md`) : affiché, cartes choisies, vue.
    pub(crate) espace3d: crate::espace3dui::Espace3dState,
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
    /// Le projet de l'export en cours (numéro d'onglet, nom) : l'export suit son projet quand on change d'onglet.
    pub(crate) export_projet: Option<(u64, String)>,
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
    /// La vue caméra a été mise par l'entrée dans l'espace Montage (elle s'éteint à la sortie).
    pub(crate) vue_camera_du_montage: bool,
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
    pub(crate) play_accum: f64,
    /// Vitesse de la lecture, signée (J, K, L du montage, jkl.rs) : 1 = la cadence du film, négative = en arrière.
    pub(crate) play_speed: f32,
    pub(crate) last_time: Option<f64>,
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
    /// Les fenêtres des variantes de séquence : confirmation de suppression, renommage (`variantesui.rs`).
    pub(crate) variantes_ui: crate::variantesui::VariantesUi,
    /// Effets des extraits de son : clé de volume choisie (`audiofxui.rs`).
    pub(crate) sfx: crate::audiofxui::SoundFxUi,
    /// La bibliothèque de bruitages : liste, recherche, écoute, sons en cours de pose (`bruitagesui.rs`).
    pub(crate) brt: crate::bruitagesui::BruitagesUi,
    /// Le panneau Bibliothèque : le dossier montré, ses vignettes, l'aperçu ouvert (`biblioui.rs`).
    pub(crate) biblio: crate::biblioui::BiblioUi,
    /// Les suggestions sonores : portée, tâches en cours, fournisseurs (`suggestionsui.rs`).
    pub(crate) sug: crate::suggestionsui::SuggestionsUi,
    /// Ce qu'on a cliqué du bouton droit dans la bande de montage.
    pub(crate) menu_target: Option<MenuTarget>,
    pub(crate) history: History,
    /// Geste de réglage en cours (champ, curseur, point de fuite) : une étape d'annulation par geste.
    pub(crate) gesture: Gesture,
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
    /// Mode concentration, conseils d'un seul passage, aide-mémoire des raccourcis, réglage du stylet (`confort.rs`).
    pub(crate) confort: crate::confort::Confort,
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
    /// Réglages de Grab et de Pull (rayon, force, atténuation, curseur), un jeu par outil (`sculpt.rs`).
    pub(crate) sculpte: crate::sculpt::SculpteReglages,
    /// Sélection bitmap au rectangle (sinon au lasso).
    pub(crate) select_rect: bool,
    /// Zone bitmap copiée.
    pub(crate) pixel_clipboard: Option<Floating>,
    /// Calque copié (W / X).
    pub(crate) layer_clipboard: Option<LayerClip>,
    /// Ce qu'a copié la sélection sur plusieurs calques (`multisel.rs`) : commun à tous les onglets, comme les
    /// autres presse-papiers (la dernière copie est celle qui se colle).
    pub(crate) multi_clip: Option<crate::multisel::MultiClip>,
    /// Coller ce qui vient d'un autre projet (`collage.rs`, `adaptation.rs`) : la question, le calcul, d'où viennent les copies.
    pub(crate) collage: crate::collage::Etat,
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
    /// Poignée de lissage de la caméra prise à la souris (lissage.rs).
    pub(crate) ease_drag: Option<crate::lissage::EaseDrag>,
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
    /// Préférences > Vignettes : « Plus de réglages » est déplié (partie 113, décision 4).
    pub(crate) prefs_plus_vignettes: bool,
    /// Confirmation « Renuméroter les plans » ouverte (`numerotationui.rs`).
    pub(crate) renumber_confirm: bool,
    /// Fenêtre « Exporter pour le montage » (EDL, XML, OTIO : `editorialui.rs`).
    pub(crate) editorial: Option<crate::editorialui::EditorialWindow>,
    /// Fenêtre « Fiche technique » (tableur CSV / XLSX : `ficheui.rs`).
    pub(crate) fiche: Option<crate::ficheui::FicheWindow>,
    /// Fenêtres du pipeline de production : liste des plans, paquet de livraison, noms et production, modèles d'épisode.
    pub(crate) liste_plans: Option<crate::listeplansui::ListeWindow>,
    pub(crate) paquet: Option<crate::paquetui::PaquetWindow>,
    /// Fenêtre « Journal des différences » entre deux envois (`journalui.rs`).
    pub(crate) journal: Option<crate::journalui::JournalFenetre>,
    /// Le pont Kitsu (kitsu.rs, kitsuui.rs) : éteint d'origine.
    pub(crate) kitsu: crate::kitsuui::Kitsu,
    pub(crate) noms_fenetre: Option<crate::nommageui::NomsWindow>,
    pub(crate) modele_episode: Option<crate::modeleepisodeui::ModeleWindow>,
    /// Relecture par le réalisateur : export de la page, import des notes, dessin du relecteur (`relectureui.rs`).
    pub(crate) relecture: crate::relectureui::RelectureUi,
    /// Travailler à plusieurs : la session du film partagé de ce projet (rien : projet classique) et l'état de son interface
    /// (`cooperation.rs`, `cooperationui.rs`).
    pub(crate) partage: Option<crate::cooperation::Session>,
    pub(crate) equipe_ui: crate::cooperationui::EquipeUi,
    /// Fenêtre « À propos de SHOT » (`aproposui.rs`).
    pub(crate) about_window: bool,
    /// Clic droit dans le dessin : la fenêtre flottante des réglages de l'outil, quand elle est ouverte (`reglagesdroit.rs`).
    pub(crate) reglages_droit: Option<crate::reglagesdroit::ReglagesDroit>,
    /// L'anneau sous le stylet : le geste en cours et ce qu'il montre (`anneauui.rs`).
    pub(crate) anneau: crate::anneauui::Etat,
    /// Le compteur d'usage : le compte des commandes de ce poste et le journal des dernières (`usageui.rs`).
    pub(crate) usage: crate::usageui::Etat,
    /// Noter une friction sans quitter le dessin : la note en cours (`frictionui.rs`).
    pub(crate) friction: crate::frictionui::Etat,
    /// Le banc du stylet : la fenêtre de Windows où écouter, et ce que FEANOR reçoit pendant l'essai (`styletui.rs`).
    pub(crate) stylet: crate::styletui::Etat,
    /// Le thème dont les couleurs sont posées dans egui : quand le réglage change, tout est
    /// reposé une fois (`theme.rs`).
    pub(crate) theme_applied: crate::theme::ReglageTheme,
    pub(crate) theme_unsaved: bool,
    /// Écran d'accueil à montrer tant qu'aucun projet n'est ouvert (`accueil.rs`).
    pub(crate) welcome: bool,
    /// L'état de la fenêtre d'accueil : le formulaire « Nouveau projet », le récent choisi (`accueilfenetre.rs`, `accueil.rs`).
    pub(crate) accueil_etat: crate::accueilfenetre::AccueilEtat,
    /// La fenêtre native : compacte (l'accueil) ou de travail (`accueilfenetre.rs`).
    pub(crate) fenetre: crate::accueilfenetre::Fenetre,
    /// Aplat sous le trait (`fond.rs`, `fondui.rs`) : le calque (son numéro) dont l'aplat est la cible du dessin, quand on a
    /// cliqué sur sa sous-ligne du panneau Calques ; le pinceau, la gomme et le pot de peinture travaillent alors l'aplat
    /// seul, sinon le trait. Propre à un projet (`onglets.rs`). Le dessin dont on peint l'aplat en ce moment (affiché en
    /// direct) vient juste après.
    pub(crate) aplat_cible: Option<u64>,
    pub(crate) fond_stroke: Option<CelId>,
    /// Le trait qui suit le même geste sur la coupe du fond (la gomme coupe, le pinceau recoud).
    pub(crate) fond_cut: Option<StrokeState>,
    /// Plusieurs clés choisies, l'aplat visé : le même trait en cours sur l'aplat des autres clés, et les remplissages qui s'y
    /// calculent en arrière-plan (`aplatmulti.rs`). Propres à un projet.
    pub(crate) aplat_miroirs: crate::aplatmulti::Miroirs,
    pub(crate) aplat_travaux: crate::aplatmulti::Travaux,
    /// L'aplat intelligent (platintel.rs) : le dessin d'avant le trait en cours, les calculs en route.
    pub(crate) aplat_intel: crate::aplatintel::Etat,
    /// La couleur de l'aplat choisie dans le sélecteur, retenue jusqu'au lâcher, et la portée du changement de couleur de l'aplat
    /// (la clé, le plan, tout le storyboard : `aplatcouleur.rs`). La portée est un réglage de l'application.
    pub(crate) aplat_couleur_attente: Option<crate::aplatcouleur::CouleurAttente>,
    pub(crate) aplat_portee: crate::aplatcouleur::AplatPortee,
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
    /// Geste de Grab ou de Pull en cours (`sculptapp.rs`).
    pub(crate) sculpt_geste: Option<crate::sculptapp::Geste>,
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
    /// La tenue des dessins en mémoire : les pixels des dessins qu'on ne regarde pas se rangent (`gel.rs`).
    pub(crate) gel: crate::gel::Gel,
    pub(crate) montage_zoom: f32,
    /// Pistes repliables du montage : le calque qui s'est déplié tout seul (`pistes.rs`).
    pub(crate) pistes: crate::pistes::PistesUi,
    /// Les clés de tous les calques de chaque plan, en traits sur son bloc du montage, gardées d'un changement du document à l'autre
    /// (`clesplan.rs`).
    pub(crate) cles_blocs: crate::clesplan::ClesBlocs,
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
    /// La fenêtre « Changer la durée des plans choisis… » (`dureeplans.rs`).
    pub(crate) duree_plans: Option<crate::dureeplans::DureePlans>,
    /// La question posée avant de réunir deux plans qui ont un calque de même nom (`fusionnoms.rs`).
    pub(crate) fusion_noms: Option<crate::fusionnoms::FusionNoms>,
    /// La fenêtre de la corbeille du projet (`corbeilleui.rs`).
    pub(crate) corbeille_ui: Option<crate::corbeilleui::Fenetre>,
    /// Une restauration de la corbeille dont les dessins se relisent sur un autre fil (`corbeilleui.rs`).
    pub(crate) corbeille_charge: Option<crate::corbeilleui::Chargement>,
    /// La question posée avant de couper un plan qui a des versions rangées (`coupeversions.rs`), et sa réponse d'avance (les essais et
    /// le hasard : sans elle, chaque coupe attendrait un clic ; `None` : la fenêtre se montre, comme pour l'utilisateur).
    pub(crate) coupe_versions: Option<crate::coupeversions::CoupeEnAttente>,
    pub(crate) coupe_versions_auto: Option<crate::variantes::Cote>,
    /// Réponse d'avance à cette question (les essais et le hasard : sans elle, chaque jonction attendrait un clic).
    /// `None` : la fenêtre se montre, comme pour l'utilisateur.
    pub(crate) fusion_noms_auto: Option<crate::fusionnoms::Reponse>,
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
    /// Manifeste du projet en dossier (`MonFilm/MonFilm.shot` ; un projet d'avant, `MonFilm.pelure`, jusqu'à son
    /// prochain enregistrement, `enregistrement.rs`).
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
    /// « Enregistrer sous » a visé un projet qui existe déjà : la fenêtre « Remplacer » attend la réponse (`remplacer.rs`).
    pub(crate) remplacement: Option<crate::remplacer::Remplacement>,
    /// Quitter a été confirmé (« Enregistrer » réussi, ou « Ne pas enregistrer » choisi). Sans cela, une fenêtre qui se
    /// ferme malgré tout (le système, un plantage de l'affichage) ne doit pas effacer la copie de secours d'un projet modifié.
    pub(crate) sortie_confirmee: bool,
    /// Change à chaque projet remplacé (`load_doc`) : un enregistrement ne s'adopte que pour le
    /// projet dont il est le cliché.
    pub(crate) doc_epoch: u64,
    pub(crate) show_help: bool,
    pub(crate) allow_close: bool,
    /// Le moment où SHOT a prévenu qu'un export en cours serait arrêté par « Quitter » (un deuxième « Quitter » dans les
    /// quinze secondes quitte pour de bon).
    pub(crate) quitter_prevenu: Option<std::time::Instant>,
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
    /// Les onglets de projets (`onglets.rs`) : le projet affiché est dans les champs ci-dessus ; celui des
    /// autres onglets est rangé dans leur `EtatProjet`.
    pub(crate) onglets: Vec<crate::onglets::Onglet>,
    /// Le rang de l'onglet affiché, et le numéro du prochain onglet.
    pub(crate) actif: usize,
    pub(crate) prochain_onglet: u64,
    /// Chaque onglet a sa copie de secours (faux dans les essais).
    pub(crate) secours_actif: bool,
    /// La session (les projets ouverts) est à retenir ; faux tant que celle de la séance d'avant se rouvre.
    pub(crate) session_a_ecrire: bool,
    /// Les onglets fermés dont la copie de secours, l'écriture en cours ou le film partagé se ferment sur un autre fil
    /// (`jeter_onglet`) : `fermeture` les attend, pour que SHOT ne coupe jamais une écriture en quittant.
    pub(crate) fermetures_d_onglets: Vec<std::thread::JoinHandle<()>>,
    pub(crate) session_prete: bool,
    /// Les projets de la séance d'avant qui restent à rouvrir, et celui à afficher à la fin.
    pub(crate) session_file: std::collections::VecDeque<PathBuf>,
    pub(crate) session_affichee: Option<PathBuf>,
    /// Plusieurs projets à enregistrer ou à fermer d'un coup (quitter, fermer les autres).
    pub(crate) groupe: Option<crate::onglets::Groupe>,
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
            // Carte perdue : en plus du rapport, une copie de secours tout de suite (`recoveryui::secours_carte_perdue`).
            // (« Destroyed » : la carte est rendue par SHOT lui-même, à la fermeture ; rien à sauver.)
            rs.device.set_device_lost_callback(|reason, message| {
                let what = format!("carte graphique perdue ({reason:?}) : {message}");
                if matches!(reason, eframe::egui_wgpu::wgpu::DeviceLostReason::Destroyed) {
                    crate::plantage::note_gpu_error(&what);
                } else {
                    crate::plantage::note_device_lost(&what);
                }
            });
        }
        let mut app = Self::construire(&cc.egui_ctx, cc.wgpu_render_state.as_ref(), true);
        // La fenêtre de Windows, et l'écoute du stylet posée dessus (`stylet.rs`) : le bouton du stylet et la gomme au dos
        // en viennent (`styletlien.rs`). Le réglage peut l'éteindre ; le banc (`styletui.rs`) la pose alors lui-même.
        app.stylet.hwnd = crate::stylet::fenetre_de(cc);
        if app.settings.stylet_windows {
            app.stylet.ecoute = Some(crate::stylet::ecouter(app.stylet.hwnd));
        }
        app
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
        crate::marque::polices(&mut fonts);
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        crate::marque::polices_interface(&mut fonts);
        egui_ctx.set_fonts(fonts);
        crate::marque::marquer_polices(egui_ctx);
        crate::demarrage::marque("polices (Segoe UI, icônes)");
        let settings = crate::settings::Settings::load();
        crate::demarrage::marque("réglages (Settings::load)");
        // Thème de l'interface, dense (12a, `theme.rs`) : celui des réglages.
        let theme_applied = settings.theme;
        crate::theme::installer(&theme_applied);
        crate::theme::appliquer(egui_ctx);
        // La taille de l'interface (`echelle.rs`) : celle des réglages, dès la première image. Le raccourci d'egui qui la change
        // (Ctrl +, Ctrl -, Ctrl 0) est coupé : Ctrl+0 est « Ajuster à la fenêtre », et la taille se choisit dans Préférences.
        egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        egui_ctx.set_zoom_factor(crate::echelle::facteur(settings.echelle));
        crate::demarrage::marque("thème");
        let montage_head_w = crate::montagehead::clamp_head_w(settings.montage_head_w);
        // (« Peindre derrière » et « Peint sur l'aplat » sont exclusifs : un fichier de réglages qui aurait les deux garde le premier.)
        let paint_behind = settings.paint_behind;
        let paint_on_aplat = settings.paint_on_aplat && !paint_behind;
        // Diagnostic des performances : proposé une fois, sur une machine qui n'en a jamais eu
        // (jamais dans les essais).
        // (Plus au lancement : « Régler SHOT pour cette machine ? » est la dernière étape de la visite « Premier jour », et dans le menu
        // Aide ; `premierjour.rs`.)
        let diag_offer = false;
        // La visite « Premier jour » : proposée sur une machine neuve (aucun conseil vu, aucun diagnostic), jamais dans les essais.
        let visite_offre = reel && settings.premier_jour.a_proposer(settings.conseils.est_vide() && settings.diagnostic.a_proposer());
        // La bibliothèque de pinceaux : les pinceaux de SHOT tout de suite ; les importés (des
        // centaines : plusieurs secondes de calcul sur une machine chargée) se lisent sur un autre fil
        // et arrivent à une image d'écran quelconque (`poll_brushes`). La lire ici retardait la
        // première image de tout ce temps.
        let brushes = crate::brushtex::BrushLibrary::builtin();
        crate::demarrage::marque("pinceaux de FEANOR (BrushLibrary::builtin)");
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
        let keymap = Keymap::load();
        crate::demarrage::marque("raccourcis (Keymap::load)");
        let orphans = crate::recovery::root().filter(|_| reel).map(|r| crate::recovery::orphans(&r, None)).unwrap_or_default();
        let recovery = crate::recovery::root().filter(|_| reel).and_then(|r| crate::recovery::Recovery::start(&r).ok());
        crate::demarrage::marque("copie de secours (séances interrompues)");
        let mut app = Self {
            ctx: egui_ctx.clone(),
            doc: Document::with_frame(1920, 1080, 24).created_now(),
            mode: Mode::Storyboard,
            board_view: BoardView::Scene,
            // (Le passe-partout des cases est retenu d'une séance à l'autre : ReglagesVignettes::passe_partout_cases.)
            vignettes: crate::boardui::Vignettes::avec_passe_partout(settings.vignettes.passe_partout_cases),
            perso: Default::default(),
            mon_panneau: Default::default(),
            scenario_ui: Default::default(),
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
            degrade: crate::degrade::Reglages::default(),
            degrade_ui: Default::default(),
            degrade_geste: None,
            shape: ShapeSettings {
                filled: false,
                width: 4.0,
                line_mode: crate::shapes::LineMode::Straight,
                closed: true,
                last: Tool::Rect,
            },
            vector_erase_whole: false,
            paint_behind,
            paint_on_aplat,
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
            fx_multi: Vec::new(),
            fx_key_sel: Vec::new(),
            fx_key_anchor: None,
            fx_key_clipboard: None,
            fx_clip_recent: false,
            fx_keys_drag: None,
            fx_stack_clipboard: None,
            fx_pane_focus: false,
            fx_pile_recent: false,
            layers_pane_focus: false,
            layer_clip_recent: false,
            video_import: None,
            propagation: crate::propagerui::Propagation::new(wgpu_render_state),
            coloriage: crate::coloriageui::Coloriage::new(),
            fusion: None,
            scenario_window: None,
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
            droit: Default::default(),
            pos_ui: Default::default(),
            op_ui: Default::default(),
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
            camera_view: false,
            stage_view: None,
            format_custom: false,
            espace3d: Default::default(),
            in_card: false,
            // Nul : la page sera mise à la largeur de la vue au premier affichage.
            board_zoom: 0.0,
            board_pan: Vec2::ZERO,
            board_sel: Vec::new(),
            board_drag: None,
            board_notes: Vec::new(),
            board_text: None,
            top_view: false,
            top_drag: None,
            play_meter: Default::default(),
            frame_start: None,
            export_dialog: None,
            export_last: Default::default(),
            export_job: None,
            export_projet: None,
            ffmpeg: None,
            layout: crate::workspace::Layout::default(),
            dock: None,
            detached: Vec::new(),
            detach_request: None,
            exposition_masquee: None,
            espace: crate::workspace::Espace::Storyboard,
            vue_camera_du_montage: false,
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
            play_speed: 1.0,
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
            variantes_ui: Default::default(),
            sfx: Default::default(),
            brt: Default::default(),
            biblio: Default::default(),
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
            confort: crate::confort::Confort { visite_offre, ..crate::confort::Confort::new(reel) },
            bsel: None,
            bdrag: None,
            sel_box: None,
            keep_proportions: false,
            field_edit_alive: false,
            deform_mode: Default::default(),
            sculpte: Default::default(),
            select_rect: false,
            pixel_clipboard: None,
            layer_clipboard: None,
            multi_clip: None,
            collage: Default::default(),
            murs: Default::default(),
            panel_clipboard: None,
            panel_focus: false,
            cam_key_sel: Vec::new(),
            cam_key_anchor: None,
            cam_key_clipboard: None,
            cam_clip_recent: false,
            cam_keys_drag: None,
            ease_drag: None,
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
            sculpt_geste: None,
            paste_event: false,
            maximized_once: 0,
            goto_plan: None,
            recent_to_open: None,
            recent_presence: Default::default(),
            open_job: None,
            palette: None,
            export_hub: false,
            prefs_window: false,
            prefs_plus_vignettes: false,
            renumber_confirm: false,
            editorial: None,
            fiche: None,
            liste_plans: None,
            paquet: None,
            journal: None,
            kitsu: Default::default(),
            noms_fenetre: None,
            modele_episode: None,
            relecture: Default::default(),
            partage: None,
            equipe_ui: Default::default(),
            about_window: false,
            reglages_droit: None,
            anneau: Default::default(),
            usage: crate::usageui::Etat::avec(Self::usage_lire()),
            friction: Default::default(),
            stylet: Default::default(),
            theme_applied,
            theme_unsaved: false,
            welcome: true,
            accueil_etat: Default::default(),
            fenetre: Default::default(),
            aplat_cible: None,
            fond_stroke: None,
            fond_cut: None,
            aplat_miroirs: Default::default(),
            aplat_travaux: Default::default(),
            aplat_intel: Default::default(),
            aplat_couleur_attente: None,
            aplat_portee: Default::default(),
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
            gel: Default::default(),
            montage_zoom: 4.0,
            pistes: Default::default(),
            cles_blocs: Default::default(),
            strip_geo: Default::default(),
            strip_anchor: [None, None],
            montage_fit: false,
            montage_head_w,
            montage_drag: None,
            edge_sel: None,
            snap_on: true,
            duration_dialog: None,
            duree_plans: None,
            fusion_noms: None,
            corbeille_ui: None,
            corbeille_charge: None,
            coupe_versions: None,
            // (Les essais coupent d'un geste, les versions avec la première partie ; ceux de `coupeversions` retirent cette réponse.)
            coupe_versions_auto: if cfg!(test) { Some(crate::variantes::Cote::Premiere) } else { None },
            // (Les essais répondent d'avance « Fusionner » : leurs jonctions se font d'un seul geste, comme avant la question.)
            fusion_noms_auto: if cfg!(test) { Some(crate::fusionnoms::Reponse::Fusionner) } else { None },
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
            status: crate::sansprojet::STATUT_DE_DEPART.into(),
            new_dialog: None,
            new_layer_dialog: None,
            effets_ui: Default::default(),
            pending: None,
            saving: None,
            save_queue: None,
            after_save: None,
            save_error: None,
            remplacement: None,
            sortie_confirmee: false,
            doc_epoch: 0,
            show_help: false,
            allow_close: false,
            quitter_prevenu: None,
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
            onglets: Vec::new(),
            actif: 0,
            prochain_onglet: 1,
            secours_actif: reel,
            session_a_ecrire: false,
            fermetures_d_onglets: Vec::new(),
            session_prete: true,
            session_file: Default::default(),
            session_affichee: None,
            groupe: None,
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
        // Onglets de projets (`onglets.rs`) : les champs de projet deviennent ceux du premier onglet, vierge
        // (sa copie de secours, ouverte plus haut, lui reste).
        let recovery = app.recovery.take();
        app.onglets_init();
        app.recovery = recovery;
        // La disposition de la dernière fois.
        // On démarre en storyboard, dans l'espace du côté storyboard où l'on avait quitté
        // (Storyboard, ou Montage) : l'unique disposition d'un ancien fichier de réglages est celle de
        // Storyboard. Les dispositions des autres espaces attendent qu'on y aille.
        app.espace = app.settings.storyboard_space;
        let saved = app.settings.layout_of(app.espace).cloned();
        // (Sans disposition retenue, le Storyboard prend aussi la sienne d'origine, avec l'onglet Scénario.)
        {
            // (Disposition d'avant 6a : la nouvelle disposition par défaut, `modernisee`.)
            let layout = saved.unwrap_or_else(|| app.espace.defaut()).modernisee();
            // (Les cadres de composition de la dernière fois aussi.)
            app.layout.safe_frames = layout.safe_frames;
            app.layout.frame_format = layout.frame_format;
            app.apply_layout(layout);
        }
        // (Le lecteur de l'espace Montage montre la vue caméra, comme quand on y entre.)
        if app.espace == crate::workspace::Espace::Montage {
            app.set_camera_view(true);
            app.vue_camera_du_montage = true;
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
        self.layer().transform_seen_at(&seen, self.cur_frame).unwrap_or(self.layer().pose(self.cur_frame))
    }

    /// Transformation du calque courant (de son dessin vers la feuille).
    pub(crate) fn layer_affine(&self) -> Affine {
        self.layer_seen().affine(self.doc.stage_center())
    }

    /// Le calque courant est une carte inclinée (Espace 3D) : le monde, son placement, l'œil de la caméra. Il ne se pose
    /// plus par une affinité mais par une projection (`espace3d.rs`).
    fn tilted_card(&self) -> Option<(crate::espace3d::Monde, crate::espace3d::Xf, [f32; 3])> {
        let l = self.layer();
        let t = l.pose(self.cur_frame);
        if !t.is_tilted() {
            return None;
        }
        let world = crate::espace3d::Monde::of(&self.doc);
        let xf = world.card_xf(&t, l.depth_at(self.cur_frame));
        let eye = world.eye_seen(&self.doc.camera_seen(self.cur_panel, self.cur_frame));
        Some((world, xf, eye))
    }

    /// Point de la feuille → point du dessin du calque courant (là où le pinceau doit peindre). Sur une carte inclinée, le
    /// regard de la caméra qui passe par ce point de la feuille traverse la carte au point où l'on peint.
    pub(crate) fn to_layer(&self, stage: Pos2) -> Pos2 {
        if let Some((world, xf, eye)) = self.tilted_card() {
            return world.sheet_to_card(eye, &xf, stage).unwrap_or(stage);
        }
        if self.layer_seen().is_identity() {
            return stage;
        }
        self.layer_affine().inverse().map_or(stage, |inv| inv.apply(stage))
    }

    /// Point du dessin du calque courant → écran.
    pub(crate) fn layer_to_screen(&self, p: Pos2) -> Pos2 {
        if let Some((world, xf, eye)) = self.tilted_card() {
            let stage = world.card_to_sheet(eye, &xf, p).unwrap_or(p);
            return self.view.to_screen(self.canvas_rect, self.canvas_size(), stage);
        }
        let stage = if self.layer_seen().is_identity() {
            p
        } else {
            self.layer_affine().apply(p)
        };
        self.view.to_screen(self.canvas_rect, self.canvas_size(), stage)
    }

    /// Pixels d'écran par pixel du dessin du calque courant.
    pub(crate) fn layer_zoom(&self) -> f32 {
        let t = self.layer().pose(self.cur_frame);
        self.view.zoom * t.scale * (t.stretch_x * t.stretch_y).abs().sqrt()
    }

    pub(crate) fn modal_open(&self) -> bool {
        self.new_dialog.is_some()
            || self.about_window
            || self.pending.is_some()
            || self.groupe.is_some()
            || self.export_dialog.is_some()
            // (L'export ne tient la main que le temps de copier le projet, quelques images d'écran : ensuite il travaille
            // sur sa copie et le projet est libre.)
            || self.export_job.as_ref().is_some_and(crate::export::ExportJob::needs_project)
            || !self.orphans.is_empty()
            || self.workspace_name.is_some()
            || self.open_job.is_some()
            || self.after_save.is_some()
            || self.save_error.is_some()
            || self.remplacement.is_some()
            || self.assign.is_some()
            || self.propagation.running()
            || self.coloriage.running()
            || self.editorial.as_ref().is_some_and(crate::editorialui::EditorialWindow::rendering)
            || self.relecture.rendering()
            || self.fiche.as_ref().is_some_and(crate::ficheui::FicheWindow::rendering)
            || self.paquet.as_ref().is_some_and(crate::paquetui::PaquetWindow::rendering)
            || self.kitsu.rendering()
            || self.vignettes.annot.confirmer
            || self.collage_occupe()
            || self.coupe_versions.is_some()
    }

    /// Les réglages de la brosse de l'outil courant. Le Mélangeur prend ceux du pinceau, en « Lisser »
    /// (`BrushSettings::as_mixer`).
    pub(crate) fn current_brush(&self) -> Option<BrushSettings> {
        match self.tool {
            Tool::Brush => Some(self.brush),
            Tool::Mixer => Some(self.brush.as_mixer()),
            Tool::Pencil => Some(self.pencil),
            Tool::Eraser => Some(self.eraser),
            _ => None,
        }
    }

    fn current_brush_mut(&mut self) -> Option<&mut BrushSettings> {
        match self.tool {
            Tool::Brush | Tool::Mixer => Some(&mut self.brush),
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

    /// Les plans et les calques choisis qui n'existent plus (une annulation, deux plans réunis, une copie annulée) ne restent
    /// pas choisis : `board_sel.len() > 1` disait « plusieurs plans choisis » au color script et à la planche quand l'un
    /// d'eux avait disparu (passe de débogage au hasard, 2026-10-22, `hasard_tests.rs`). Les calques choisis d'un autre
    /// plan restent : chaque usage ne prend que ceux du plan courant. (Par `revalider_rangs` : ils ne disparaissent que par un
    /// changement de structure, une annulation, un projet ouvert ou un changement venu d'un équipier, jamais d'une image à l'autre.)
    fn oublier_les_choix_disparus(&mut self) {
        let doc = &self.doc;
        if self.board_sel.len() > 8 {
            let ids: std::collections::HashSet<u64> = doc.panels.iter().map(|p| p.id).collect();
            self.board_sel.retain(|id| ids.contains(id));
        } else {
            self.board_sel.retain(|id| doc.panels.iter().any(|p| p.id == *id));
        }
        let cur = &doc.panels[self.cur_panel].layers;
        self.layer_sel.retain(|id| cur.iter().any(|l| l.id == *id) || doc.panels.iter().any(|p| p.layers.iter().any(|l| l.id == *id)));
    }

    /// Les calques du plan courant tels qu'ils sont maintenant (`calques_tiennent`).
    pub(crate) fn empreinte_calques(&self) -> EmpreinteCalques {
        EmpreinteCalques { plan: self.panel().id, ids: self.layers().iter().map(|l| l.id).collect() }
    }

    /// Les calques du plan courant sont-ils toujours ceux de `avant` (même plan, même nombre, et le calque de rang `li` le même) ?
    /// Une liste de calques peinte rang par rang montre le menu du clic droit de chaque rang au milieu des autres : « Supprimer »
    /// un dossier, « Fusionner »… changent les calques, et les rangs d'après ne désignent plus les mêmes calques (ou plus aucun).
    /// La liste s'arrête alors ; l'image suivante la refait. (Un identifiant par rang : rien de lourd par image.)
    pub(crate) fn calques_tiennent(&self, avant: &EmpreinteCalques, li: usize) -> bool {
        let layers = self.layers();
        self.panel().id == avant.plan && layers.len() == avant.ids.len() && avant.ids.get(li).is_none_or(|id| layers[li].id == *id)
    }

    /// Après un changement du film qui ne vient pas d'ici (le montage ou un plan d'un équipier, `cooperationui.rs`) : le plan et le
    /// calque courants sont retrouvés par leur identifiant, pas par leur rang (le rang du plan qu'on regardait est peut-être devenu
    /// celui d'un autre plan). `avant` : les identifiants du plan et du calque courants d'avant le changement.
    pub(crate) fn retrouver_nav(&mut self, avant: (u64, u64)) {
        if let Some(i) = self.doc.panels.iter().position(|p| p.id == avant.0) {
            self.cur_panel = i;
            if let Some(li) = self.doc.panels[i].layers.iter().position(|l| l.id == avant.1) {
                self.cur_layer = li;
            }
        }
        self.clamp_nav();
        self.revalider_rangs();
    }

    /// Ce que l'application garde par rang ou par identifiant, recalé sur le film d'aujourd'hui, en un seul endroit : après chaque
    /// changement de structure (`commit_structure`), une annulation (`after_history`), un projet ouvert, un changement venu d'un
    /// équipier. Les menus ouverts sur un plan qui n'est plus là se ferment, les plans choisis disparus sont lâchés, les glissés de
    /// calques dont le rang n'existe plus s'arrêtent. (Quelques recherches dans les plans et les calques : rien de lourd.)
    pub(crate) fn revalider_rangs(&mut self) {
        self.clamp_nav();
        let n = self.layers().len();
        for drag in [&mut self.head_drag, &mut self.panel_layer_drag, &mut self.montage_layer_drag] {
            if drag.is_some_and(|li| li >= n) {
                *drag = None;
            }
        }
        self.oublier_les_choix_disparus();
        if let Some(crate::boardui::MenuTarget::Card { plan, case, .. }) = self.vignettes.menu {
            if self.case_du_menu(plan, case).is_none() {
                self.vignettes.menu = None;
            }
        }
    }

    // ------------------------------------------------------------------ composition

    /// Ce qu'il faut empiler pour afficher l'image `frame` du plan courant. `onion` : affichage
    /// de travail (pelure d'oignon et table lumineuse), sinon l'image telle qu'elle est.
    pub(crate) fn build_plan(&self, frame: usize, onion: bool, paper: bool) -> Plan {
        self.build_plan_avec(frame, onion, paper, true)
    }

    /// Comme `build_plan`, avec ou sans les calques d'annotation du plan : à l'écran ils y sont, l'export de l'image courante ne les prend que
    /// si « Avec les annotations » est coché (`Settings::export_annotations`).
    pub(crate) fn build_plan_avec(&self, frame: usize, onion: bool, paper: bool, annotations: bool) -> Plan {
        let mut items = Vec::new();
        // Table lumineuse : les autres calques sont estompés, le calque courant reste net.
        let washed = onion && self.light_table && !self.playing;
        // Multiplan : les calques en profondeur défilent selon la position de la caméra.
        let camera = self.doc.camera_seen(self.cur_panel, frame);
        let world = crate::espace3d::Monde::of(&self.doc);
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
        let mut starts: Vec<usize> = vec![0; layers.len()];
        // À l'écran, les calques d'annotation du plan (`calqueannot.rs`) viennent après tous les autres : par-dessus tout, même un calque d'effet.
        for li in crate::calqueannot::ordre(layers, annotations) {
            let layer = &layers[li];
            starts[li] = items.len();
            // Caché (lui ou son dossier) : rien ; sinon son opacité, avec celle de son dossier.
            let Some(mut opacity) = crate::folders::effect_at(layers, li, frame) else { continue };
            // Calque d'effet (`calqueffet.rs`) : son repère, qui couvre ce qui est dessous.
            if layer.effet.is_some() {
                items.extend(crate::calqueffet::marker(layers, li, frame, opacity, &starts));
                continue;
            }
            if washed && li != cur {
                opacity *= self.light_table_strength.clamp(0.05, 1.0);
            }
            // Derrière la caméra (elle a avancé au-delà) : on ne le voit pas.
            // (Carte inclinée de l'Espace 3D : posée par un maillage, `espace3d::place`.)
            let Some(placed) = crate::espace3d::place(&world, layer, frame, &camera) else { continue };
            let (transform, card_pre) = (placed.transform, placed.pre);
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
                            pre: card_pre.clone(),
                        });
                    }
                }
            }
            // Couleur unie du calque (`couleurunie.rs`) : sous ses dessins et sa pelure d'oignon.
            items.extend(crate::render::unie_item(layer, (opacity * 255.0).round() as u8));
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
                    ctg: None,
                    fond: None,
                    pre: card_pre.clone(),
                };
                // Remplissage au griffonnage d'un calque de couleur (`griffonnage.rs`), puis fond sous le trait
                // (`fond.rs`), la peinture libre ou le trait par-dessus.
                items.extend(crate::render::griffon_item(layers, li, frame, &line));
                items.extend(crate::render::fond_item(layer, &line));
                match self.flottante.as_ref().filter(|f| f.src == cel).or_else(|| self.multi.float_for(cel)) {
                    // Sélection vectorielle en cours de transformation : le dessin sans elle, et sa
                    // feuille flottante par-dessus (`flottante.rs`) ; avec plusieurs calques
                    // choisis, une feuille par calque (`multisel.rs`).
                    Some(f) => items.extend(f.items(&line)),
                    None => items.push(line),
                }
                // Calque de couleur courant en mode « Griffonner », à l'écran seulement : ses griffonnages, pâles, par-dessus
                // (c'est ce qu'on pose avec le pinceau ; le remplissage est dessous).
                if onion && li == cur && !self.playing {
                    if let Some(g) = layer.couleur.as_ref().filter(|c| c.griffonner).and_then(|c| c.griffonnages.get(&cel).copied()) {
                        items.push(PlanItem { cel: g, transform, opacity: 90, tint: None, blend: Default::default(), fx: Vec::new(), ctg: None, fond: None, pre: card_pre.clone() });
                    }
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
        let avant = self.espace;
        self.cancel_montage_drag();
        self.poser_le_geste_de_camera();
        self.poser_la_creation();
        self.end_stroke();
        self.playing = false;
        // Les Vignettes s'ouvrent sans plan choisi.
        if espace == Espace::Vignettes {
            self.vignettes.libre = true;
            self.vignettes.camera_choisie = None;
        }
        self.aller_a_l_espace(espace);
        // L'onglet Vignettes, c'est toujours la planche (partie 113) : une disposition retenue qui dirait le dessin (un ancien réglage,
        // ou l'espace qu'on rouvre alors qu'on y est déjà) n'y change rien. Et l'outil en main y est l'un des siens (décision 2) : un autre
        // laisse la place au pinceau.
        if espace == Espace::Vignettes {
            self.board_view = BoardView::Panels;
            if !Tool::permis_dans_les_vignettes(self.tool) {
                let avant = self.tool.label();
                self.set_tool(Tool::Brush);
                self.status = format!("Vignettes : le pinceau remplace {avant} (pinceau, crayon, gomme et caméra seulement).");
                self.masquer_exposition();
                return;
            }
        }
        // Le lecteur de l'espace Montage montre le film tel que la caméra le filme ; en sortir remet
        // la vue du plateau, si c'est lui qui l'avait changée (un réglage de vue : rien dans le projet).
        if espace == Espace::Montage && avant != Espace::Montage && !self.camera_view {
            self.set_camera_view(true);
            self.vue_camera_du_montage = true;
        } else if avant == Espace::Montage && espace != Espace::Montage && std::mem::take(&mut self.vue_camera_du_montage) {
            self.set_camera_view(false);
        }
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
            // (Une seule astuce courte par espace ; le détail est dans la bulle de l'onglet.)
            Espace::Vignettes => "Vignettes : dessine dans les cases.".into(),
            Espace::Storyboard => "Storyboard : monte les plans.".into(),
            Espace::Montage => "Montage : le film en haut, les pistes en bas.".into(),
            Espace::Animation => format!("Animation de {}.", crate::storyboard::panel_long_title(&self.doc, self.cur_panel)),
        };
        // La feuille d'exposition n'existe qu'en animation.
        self.masquer_exposition();
    }

    pub(crate) fn select_layer(&mut self, layer: usize) {
        if layer < self.layers().len() && layer != self.cur_layer {
            self.end_stroke();
            self.cur_layer = layer;
            // Un calque choisi à la main lâche la brosse du décor : on dessine là où l'on a cliqué (`decorui.rs`).
            self.decor_calque_choisi();
            // Le calque d'un personnage choisi devient son « dernier calque utilisé » du plan (`calquespastilles.rs`).
            self.noter_calque_de_personnage();
            // Changer de calque, c'est revenir au trait (la cible aplat suit la sous-ligne choisie).
            self.aplat_cible = None;
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
            self.play_speed = 1.0;
            return;
        }
        self.end_stroke();
        self.play_speed = 1.0;
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

    pub(crate) fn tick_playback(&mut self, ctx: &egui::Context) {
        if !self.playing {
            // La lecture a pu être arrêtée par n'importe quelle action : le son s'arrête avec.
            self.stop_audio();
            self.play_speed = 1.0;
            return;
        }
        if self.tick_audio_clock() {
            ctx.request_repaint();
            return;
        }
        let now = ctx.input(|i| i.time);
        let last = self.last_time.replace(now).unwrap_or(now);
        let vitesse = self.play_speed;
        self.play_accum += (now - last).min(0.25) * vitesse.abs() as f64;
        let frame_duration = 1.0 / self.doc.fps.max(1) as f64;
        while self.play_accum >= frame_duration {
            self.play_accum -= frame_duration;
            let (start, end) = if self.play_montage { (0, self.frame_count()) } else { self.plan_play_range() };
            if vitesse < 0.0 {
                // En arrière (J) : image par image jusqu'au début du plan, puis le plan d'avant.
                if self.cur_frame > start {
                    self.cur_frame -= 1;
                } else if self.play_montage && self.cur_panel > 0 {
                    let prev = self.cur_panel - 1;
                    self.set_panel(prev);
                    self.cur_frame = self.frame_count().saturating_sub(1);
                } else if self.looping {
                    if self.play_montage {
                        let last = self.doc.panels.len() - 1;
                        self.set_panel(last);
                        self.cur_frame = self.frame_count().saturating_sub(1);
                    } else {
                        self.cur_frame = end.max(1) - 1;
                    }
                } else {
                    self.playing = false;
                    break;
                }
            } else if self.cur_frame + 1 < end {
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
        // Calque de couleur : son dessin est celui que le miroir du trait lui donne ; en mode « Griffonner », c'est le
        // dessin de griffonnages qui s'y rattache (`calquecouleurui.rs`).
        if let Some(c) = self.layer().couleur.as_ref() {
            return if c.griffonner { self.ensure_scribble_cel() } else { self.ensure_colour_cel() };
        }
        self.ensure_plain_cel()
    }

    /// `ensure_cel` d'un calque ordinaire — ou d'un calque de couleur libre, qui n'a pas de trait dont suivre les
    /// images : ses dessins se créent comme ceux d'un calque ordinaire.
    pub(crate) fn ensure_plain_cel(&mut self) -> (CelId, Option<Edit>) {
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
            self.status_avec_action(
                "Un calque vidéo ne se dessine pas : transforme-le en calque d'animation (bouton ci-contre), ou dessine sur un calque au-dessus.".into(),
                crate::messages::ActionMessage::VideoEnAnimation,
            );
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

    /// Début d'un trait de pinceau, de crayon ou de gomme, bitmap ou vectoriel selon le calque, sur le calque courant (ou sur
    /// son aplat quand sa sous-ligne est choisie). `begin_paint` (`aplatsur.rs`) le précède : « Peint sur l'aplat » y
    /// détourne le trait vers l'aplat.
    pub(crate) fn begin_paint_ici(&mut self, p: Pos2, pressure: f32, pen: bool) {
        self.end_stroke();
        self.playing = false;
        // On dessine : Suppr doit de nouveau viser le dessin, pas un extrait de son.
        self.sel_clip = None;
        if !self.layer_editable() {
            return;
        }
        let erase = self.tool == Tool::Eraser;
        // (Le Mélangeur ne pose aucune couleur : ni personnage, ni couleur récente.)
        if !erase && self.tool != Tool::Mixer {
            // Un personnage qui a un fond le donne au calque (`personnagesui.rs`).
            self.perso_avant_trait();
            // Elle rejoint les couleurs récentes (passe couleur).
            self.color_used(self.color);
        }
        if erase && self.cel_here().is_none() {
            self.guides.stroke = None;
            return;
        }
        // Aplat (`fond.rs`) : si sa sous-ligne est choisie, pinceau et gomme peignent dans l'aplat, en bitmap.
        if self.aplat_en_cible() {
            self.begin_fond_stroke(p, pressure, pen);
            return;
        }
        match (self.layer().kind, erase) {
            // Le Mélangeur lit les pixels du calque : il n'a rien à mêler sur un calque vectoriel.
            (LayerKind::Vector, false) if self.tool == Tool::Mixer => {
                self.status = "Le Mélangeur agit sur un calque bitmap.".into();
            }
            (LayerKind::Vector, true) => self.begin_vector_erase(p, pen),
            (LayerKind::Vector, false) => self.begin_vector_stroke(p, pressure, pen),
            (LayerKind::Bitmap, _) => self.begin_stroke(p, pressure, pen),
        }
    }

    fn begin_stroke(&mut self, p: Pos2, pressure: f32, pen: bool) {
        let Some(settings) = self.current_brush() else {
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
        // (Plus de « Remplir la forme » : le moteur garde l'option, l'application ne l'allume plus.)
        let (behind, fill, tol) = (painting && self.paint_behind && self.tool != Tool::Mixer, false, self.close_tolerance());
        let mut stroke = StrokeState::new(cel, mode, settings, prefix, pen)
            .with_textures(textures)
            .with_feel(self.toucher)
            .with_zoom(self.layer_zoom())
            .with_secondary(self.couleurs.prefs.back)
            .on_layer(self.layer().keep_alpha, stencil)
            .with_options(behind, fill, tol);
        // Toile infinie : on peint aussi hors de la feuille (tuiles gardées à part, `Raster::outer`) ;
        // sauf dans la zone fermée d'un calque de couleur (son pochoir est celui de la feuille), et pour des griffonnages
        // (ils remplissent des zones de la feuille).
        if !self.layer().couleur.as_ref().is_some_and(|c| c.griffonner || (painting && c.zone)) {
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
        let (Some(settings), Some(r)) = (self.current_brush(), self.layer().fond) else {
            return;
        };
        let (cel, prefix) = self.ensure_cel();
        let (w, h) = (self.doc.width, self.doc.height);
        let Some(cover) = self.doc.cels.get(&cel).map(|c| crate::fond::silhouette_monde(&c.raster, &r).feuille(w, h)) else { return };
        if cover.iter().all(|&c| c == 0) {
            self.status = "Aplat : dessine d'abord le trait, l'aplat se déduit de lui (clique sur le calque pour revenir au trait).".into();
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
            let Some(theirs) = l.transform_seen_at(&seen, self.cur_frame) else { continue };
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
        // (Plusieurs clés choisies, l'aplat visé : le même point sur l'aplat des autres clés, `aplatmulti.rs`.)
        self.miroirs_point(p, pressure);
    }

    /// Termine tout geste en cours : trait, gomme vectorielle, transformation, forme.
    pub(crate) fn end_stroke(&mut self) {
        // Un trait (ou une gomme vectorielle) est-il en cours ? L'aplat intelligent le suit à sa fin (platintel.rs).
        let trait_en_cours = self.stroke.is_some() || self.verase.is_some();
        // Un segment droit en attente (Maj tenue) est posé avant de finir le trait.
        self.droit_flush();
        // Le trait d'un personnage (`personnagesui.rs`) : son dessin et les tuiles d'avant, pour le fond qu'il gagne.
        let mut trait_de_personnage: Option<(CelId, Vec<(usize, Option<crate::raster::Tile>)>)> = None;
        // Un trait de pastille (pas dans l'aplat) : sa couleur d'aplat devient celle du calque (`perso_aligner_couleur_aplat`).
        let mut trait_de_pastille = false;
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
            // (Sa couleur et son opacité, pour la part du trait qui reste sur la planche : horschamp.rs.)
            let paint = stroke.tip_paint();
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
                // Peinture et coupe : une seule étape d'annulation, nommée « Aplat : … ».
                let nom = if self.tool == Tool::Eraser { "Aplat : gomme" } else { "Aplat : peinture" };
                let avant = self.history.context.replace(nom.to_string());
                match edits.len() {
                    0 => {}
                    1 => self.history.push(edits.pop().unwrap_or(Edit::Group(Vec::new()))),
                    _ => self.history.push(Edit::Group(edits)),
                }
                self.history.context = avant;
            } else {
                let edit = match self.doc.cels.get_mut(&cel) {
                    Some(c) => stroke.finish(&mut c.raster, &mut dirty),
                    None => None,
                };
                self.renderer.invalidate_tiles(&dirty);
                // (Sans pastille, le trait suit tout de même la teinte de la clé de son dessin, s'il en a une : `persofond::teinte_de_la_cle`.)
                trait_de_personnage = crate::personnagesui::tuiles_de(edit.as_ref()).map(|t| (cel, t));
                trait_de_pastille = self.perso.actif.is_some();
                match self.vstroke.take() {
                    Some(vs) => {
                        // Les pixels d'avant l'aperçu servent à poser le trait définitif sans tout redessiner.
                        let preview = match edit {
                            Some(Edit::Tiles { tiles, .. }) => Some(tiles),
                            _ => None,
                        };
                        self.commit_vector_stroke(vs, std::mem::take(&mut stroke.path), std::mem::take(&mut stroke.path_pressure), std::mem::take(&mut stroke.path_time), preview);
                        self.horschamp_noter_vecteur(cel)
                    }
                    None => {
                        if let Some(c) = self.doc.cels.get_mut(&cel) {
                            c.version += 1;
                        }
                        if let Some(edit) = edit {
                            // (La recette du pinceau, pour la part du trait qui sort de la case : elle garde sa texture sur la planche.)
                            let (recette, appui, instants) = self.horschamp_recette(&stroke);
                            self.horschamp_noter_bitmap(cel, &edit, stroke.path.clone(), paint, recette, appui, instants);
                            self.history.push(edit);
                        }
                    }
                }
            }
        }
        // Les autres clés choisies finissent le même trait ; leurs étapes rejoignent celle du trait (`aplatmulti.rs`).
        self.miroirs_finish();
        // (Avant le fil de fond : son numéro d'étape compte l'étape du trait, couleur du calque comprise.)
        if trait_de_pastille {
            self.perso_aligner_couleur_aplat();
        }
        // Le fond que le trait d'un personnage vient de gagner se calcule sur le fil de fond.
        if let Some((cel, tiles)) = trait_de_personnage {
            self.perso_apres_trait(cel, tiles);
        }
        self.finish_vector_erase();
        self.finish_cut();
        self.sculpt_finish();
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
        self.guides.stroke = None;
        self.shape_drag = None;
        // (Le dégradé se pose au lâcher, `degrade_release`, avant ceci : ce qui reste est un tracé abandonné ; une retouche en cours
        // s'écrit ; un dégradé posé reste retouchable jusqu'à ce qu'on change d'outil, de calque ou d'image.)
        self.degrade_abandonner();
        // Aplat intelligent : le trait fini, l'aplat de la clé suit (en arrière-plan, platintel.rs).
        if trait_en_cours {
            self.intel_apres_trait();
        }
    }

    pub(crate) fn pick_color(&mut self, p: Pos2) {
        if let Some(c) = self.couleur_du_monde(p) {
            self.color = c;
        }
    }

    /// La couleur du dessin au point `p` de la feuille (la pipette de l'outil, sans toucher à la couleur de dessin).
    pub(crate) fn couleur_du_monde(&self, p: Pos2) -> Option<[u8; 3]> {
        if !p.x.is_finite() || !p.y.is_finite() {
            return None;
        }
        let plan = self.build_plan(self.cur_frame, false, true);
        let c = if p.x < 0.0 || p.y < 0.0 || p.x >= self.doc.width as f32 || p.y >= self.doc.height as f32 {
            // Toile infinie : la pipette prend aussi hors de la feuille (le pixel du monde, papier compris).
            let (x, y) = (p.x.floor(), p.y.floor());
            let area = render::Area::around((x, y), (x + 1.0, y + 1.0));
            *render::composite_area(&self.doc, &plan, area).first()?
        } else {
            render::composite_pixel(&self.doc, &plan, p.x as usize, p.y as usize)
        };
        let [r, g, b, _] = unpremultiply(c);
        Some([r, g, b])
    }

    /// La pipette d'un champ de couleur, sans capture d'écran (`champcouleur.rs`) : la couleur du dessin sous le
    /// point `pos` de l'écran, s'il est dans le dessin.
    pub(crate) fn couleur_sous_le_curseur(&self, pos: Pos2) -> Option<[u8; 3]> {
        if !self.canvas_rect.contains(pos) {
            return None;
        }
        let stage = self.view.to_canvas(self.canvas_rect, self.canvas_size(), pos);
        self.couleur_du_monde(stage)
    }

    pub(crate) fn fill_at(&mut self, p: Pos2) {
        if p.x < 0.0 || p.y < 0.0 || p.x >= self.doc.width as f32 || p.y >= self.doc.height as f32 {
            // Toile infinie : le pot de peinture cherche ses contours sur la feuille (une zone qui
            // s'étendrait à l'infini n'a pas de fin) ; hors de la feuille, il ne remplit pas.
            self.status = "Le pot de peinture remplit dans la feuille : clique sur une zone fermée de la feuille.".into();
            return;
        }
        // Aplat (`fondui.rs`) : si sa sous-ligne est choisie, la zone fermée par le trait se remplit dans l'aplat.
        // (Plusieurs clés choisies : la même zone se remplit sur l'aplat des autres, en arrière-plan, `aplatmulti.rs`.)
        if self.aplat_en_cible() {
            let avant = self.history.serial();
            self.fond_zone_at(p, false);
            self.aplat_autres_cles(p, false, avant);
            return;
        }
        // Calque de couleur (`calquecouleurui.rs`) : la zone fermée par le trait de son calque de trait.
        if self.layer().couleur.is_some() {
            let avant = self.history.serial();
            self.fill_couleur(p);
            self.aplat_autres_cles(p, false, avant);
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
        if self.fill.sample_all && self.layer().pose(self.cur_frame).is_identity() {
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
            || self.sculpt_occupe(cel)
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

    /// Le tour de la tenue des dessins (`gel.rs`) : les pixels des dessins qu'on ne regarde pas se rangent en tampons exacts. Au plus une
    /// demi-milliseconde par image d'écran ; rien à faire quand tout est rangé.
    pub(crate) fn tenir_les_dessins(&mut self, ctx: &egui::Context) {
        let _t = crate::perf::section("dessins rangés (tour)");
        if let Some(p) = self.doc.panels.get(self.cur_panel) {
            self.gel.visite(p.id);
        }
        self.gel.caches_communs = true;
        let lecture = self.playing.then_some(self.doc.fps);
        self.gel.tenir(&mut self.doc, self.cur_panel, lecture);
        self.gel.tenir_l_historique(&mut self.history);
        // (Pas pendant un geste ni une lecture : le pointeur est relâché, rien ne joue, aucun trait ne se rejoue.)
        let calme = !self.playing && !ctx.input(|i| i.pointer.any_down()) && self.rejeu_again.is_empty();
        self.gel.elaguer(&mut self.doc, &self.history, calme);
        let mut occupe = self.gel.occupe();
        // Les projets des autres onglets : leurs dessins se rangent aussi (le plan qu'on y regardait en dernier reste ouvert).
        for o in &mut self.onglets {
            if let Some(e) = o.etat.as_mut() {
                // (Les caches communs sont ceux du projet affiché : pas de passage depuis un onglet caché.)
                e.gel.caches_communs = false;
                e.gel.tenir(&mut e.doc, e.cur_panel, None);
                e.gel.tenir_l_historique(&mut e.history);
                occupe |= e.gel.occupe();
            }
        }
        if occupe {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
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
            // (Toile infinie : les tuiles hors de la feuille comptent aussi, sinon l'écran garde l'image d'avant de tout ce qui
            // est loin de la feuille, `Renderer::take_world_dirty`.)
            let changed: Vec<usize> = c.raster.ids_either(&done.raster);
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
        // Remplissages au griffonnage (calques de couleur) arrivés de l'arrière-plan : l'image composée par le
        // processeur est à refaire (`fxcache.rs`, `griffonnage.rs`).
        if crate::fx::fond_poll() | crate::fx::griffon_poll() {
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
        } else if matches!(self.tool, Tool::Grab | Tool::Pull) {
            // Le rayon du cercle de Grab et de Pull (`sculpt.rs`).
            let r = self.sculpte.de_mut(self.tool);
            r.rayon = (r.rayon * factor).clamp(crate::sculpt::Reglages::RAYON_MIN, crate::sculpt::Reglages::RAYON_MAX);
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
        // Les cases d'un plan suivent leurs clés quand on les retouche au montage ou dans la timeline (`cases.rs`) : dans la même étape
        // d'annulation que la retouche.
        if let Edit::Structure { structure, .. } = &snap {
            self.doc.cases_suivent_les_cles(&structure.panels);
        }
        self.history.push(snap);
        self.modified = true;
        // Ce qui est gardé par rang d'abord, sur le film d'après le changement : `sync_couleurs` lit les calques du plan courant
        // (supprimer le dernier plan plantait là, Anthony 2026-10-22).
        self.revalider_rangs();
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
        // (Une copie du dessin part avec le fond que son dernier trait vient de gagner : `attendre_fonds_de_traits`.)
        self.attendre_fonds_de_traits();
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
            LayerKind::Vector => format!("Vectoriel {n}"),
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

    /// Duplique le calque courant ; la copie, juste au-dessus, devient le calque courant. (Les menus passent par
    /// `duplicate_chosen_layers`, qui copie aussi les calques choisis.)
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn duplicate_layer(&mut self) {
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        self.attendre_fonds_de_traits();
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
        if self.layer().keys_follow_line() {
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

    /// Le projet de ce moment : l'onglet et le fichier (`LayerClip::projet`).
    fn projet_courant(&self) -> (u64, Option<std::path::PathBuf>) {
        (self.onglets.get(self.actif).map_or(0, |o| o.id), self.path.clone())
    }

    /// La copie du calque `li` (un dossier : avec ses calques ; un calque de trait : avec ses calques de couleur), avec ses dessins.
    fn layer_clip_of(&self, li: usize) -> LayerClip {
        let layer = self.layers()[li].clone();
        let children: Vec<Layer> = crate::folders::children_of(self.layers(), li).into_iter().map(|i| self.layers()[i].clone()).collect();
        let colours: Vec<Layer> = crate::calquecouleur::couleurs_de(self.layers(), li).into_iter().map(|i| self.layers()[i].clone()).collect();
        let cels = children
            .iter()
            .chain(colours.iter())
            .chain(std::iter::once(&layer))
            .flat_map(|l| crate::calquecouleur::dessins_du_calque(l))
            .filter_map(|id| self.doc.cels.get(&id).map(|c| (id, c.clone())))
            .collect();
        LayerClip {
            layer,
            children,
            colours,
            cels,
            panel_len: self.frame_count(),
            origine: crate::adaptation::Origine::de(&self.doc),
            projet: self.projet_courant(),
            extras: Vec::new(),
        }
    }

    /// Copie les calques choisis (le calque courant seul, ou tous ceux qu'on a choisis par Ctrl ou Maj + clic), avec leurs dessins,
    /// leur timeline, leur aplat, leurs effets, leur mode de fusion et leur opacité (W, ou Ctrl+C quand le panneau Calques a la
    /// main) ; un dossier, avec ses calques (13b-5).
    ///
    /// Un calque de couleur lié à son trait ne se copie pas sans lui (retour d'Anthony, 2026-10-24 : collé dans un autre plan, il
    /// aurait perdu son trait, et ses images ne suivaient plus rien) : c'est le trait qui est copié, avec tous ses calques de couleur.
    /// Un calque dont le dossier est aussi choisi part avec le dossier.
    pub(crate) fn copy_layer(&mut self) {
        self.end_stroke();
        self.attendre_fonds_de_traits();
        let choisis = self.chosen_layers();
        let mut tops: Vec<usize> = choisis.iter().map(|&li| crate::calquecouleur::trait_de(self.layers(), li).unwrap_or(li)).collect();
        if choisis.len() > 1 {
            tops.retain(|&li| crate::folders::parent_index(self.layers(), li).is_none_or(|fi| !choisis.contains(&fi)));
        }
        tops.sort_unstable();
        tops.dedup();
        let avec_son_trait = tops.len() == 1 && tops[0] != self.cur_layer && choisis.len() == 1;
        let mut clips: Vec<LayerClip> = tops.iter().map(|&li| self.layer_clip_of(li)).collect();
        let n = clips.len();
        let mut clip = clips.remove(0);
        clip.extras = clips;
        let what = if clip.layer.folder { "Dossier" } else { "Calque" };
        let avec = if avec_son_trait { " (le calque de couleur emmène son trait)" } else { "" };
        self.status = if n > 1 {
            format!("{n} calques copiés : Ctrl+V dans le panneau Calques (ou X) pour les coller, ici ou dans un autre plan.")
        } else {
            format!("{what} « {} » copié{avec} : X (ou Ctrl+V dans le panneau Calques) pour le coller dans un autre plan.", clip.layer.name)
        };
        // La dernière chose copiée : Ctrl+V depuis le panneau Calques le colle (`layer_clip_recent`).
        self.layer_clip_recent = true;
        self.layer_clipboard = Some(clip);
    }
    /// Calque collé : une copie de `source` avec ses propres copies des dessins (`copies` évite de
    /// copier deux fois un même dessin), à la durée du plan ; un dessin tenu jusqu'à la fin du plan
    /// d'origine reste tenu jusqu'à la fin de celui-ci. `meme_projet` : la copie vient de ce projet-ci, un calque de personnage garde
    /// alors son lien au personnage (s'il existe encore) ; ailleurs, c'est un calque ordinaire.
    fn pasted_layer(&mut self, source: &Layer, clip: &LayerClip, copies: &mut HashMap<CelId, CelId>, meme_projet: bool) -> Layer {
        let mut layer = Layer { id: fresh_layer_id(), scene_wide: None, decor: false, ..source.clone() };
        if let Some(pid) = layer.personnage {
            if !(meme_projet && self.doc.personnages.iter().any(|p| p.id == pid)) {
                layer.personnage = None;
            }
        }
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
        // Un calque de couleur collé : les copies de ses dessins de couleur et de ses griffonnages.
        self.doc.copier_griffonnages_avec(std::slice::from_ref(source), copies, |d, g| clip.cels.get(&g).map(|c| d.add_cel(c.clone())));
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

    /// Colle le ou les calques copiés au-dessus du calque courant (X, ou Ctrl+V dans le panneau Calques), avec leurs propres copies
    /// des dessins, **en une seule étape d'annulation**. Dans le dossier du calque courant s'il y en a un ; un dossier copié se
    /// colle avec ses calques, au-dessus du dossier courant (pas de dossier dans un dossier). Un calque de même nom dans le plan
    /// prend « Nom (2) », « Nom (3) »… (`layerops::nom_libre`).
    pub(crate) fn paste_layer(&mut self) {
        if self.content_refuses() {
            return;
        }
        let Some(clip) = self.layer_clipboard.clone() else {
            self.status = "Aucun calque copié (W pour copier le calque courant).".into();
            return;
        };
        // Copié dans un autre onglet, d'une autre feuille : adapté (`adaptation.rs`, `collage.rs`) ; l'original reste
        // dans le presse-papiers.
        let clip = match self.collage_preparer(crate::collage::Colle::Calque(clip), crate::collage::Reprise::Calque) {
            crate::collage::Preparation::Pret(crate::collage::Colle::Calque(c)) => c,
            _ => return,
        };
        self.end_stroke();
        self.drop_bitmap_selection();
        let snap = self.snapshot();
        let meme_projet = clip.projet == self.projet_courant();
        let mut copies: HashMap<CelId, CelId> = HashMap::new();
        let cur = self.cur_layer;
        let folder_of_current = crate::folders::parent_index(self.layers(), cur);
        // Un dossier ne va pas dans un dossier : tout va au-dessus du dossier du calque courant.
        let au_dessus_du_dossier = folder_of_current.is_some() && std::iter::once(&clip).chain(clip.extras.iter()).any(|c| c.layer.folder);
        let at = match (au_dessus_du_dossier, folder_of_current) {
            (true, Some(fi)) => fi + 1,
            _ => cur + 1,
        };
        let parent_commun = if au_dessus_du_dossier { None } else { folder_of_current.map(|fi| self.layers()[fi].id) };
        let mut block: Vec<Layer> = Vec::new();
        let mut mains: Vec<(u64, String)> = Vec::new();
        for c in std::iter::once(&clip).chain(clip.extras.iter()) {
            let mut layer = self.pasted_layer(&c.layer, c, &mut copies, meme_projet);
            // Les numéros des calques collés, par ceux des calques copiés : les calques de couleur collés colorient les traits collés.
            let mut ids: HashMap<u64, u64> = HashMap::from([(c.layer.id, layer.id)]);
            let mut children: Vec<Layer> = Vec::new();
            for ch in &c.children {
                let mut child = self.pasted_layer(ch, c, &mut copies, meme_projet);
                child.parent = Some(layer.id);
                ids.insert(ch.id, child.id);
                children.push(child);
            }
            // Ses calques de couleur reviennent avec lui, sous la copie.
            let mut colours: Vec<Layer> = Vec::new();
            for co in &c.colours {
                let colour = self.pasted_layer(co, c, &mut copies, meme_projet);
                ids.insert(co.id, colour.id);
                colours.push(colour);
            }
            // Les liens des calques de couleur : leur trait collé (jamais celui du plan d'où ils viennent), et seulement les sources
            // qui existent dans ce plan (celles d'un autre plan n'y sont pas : un lien qui pend).
            let ici: std::collections::HashSet<u64> = self.layers().iter().map(|l| l.id).chain(ids.values().copied()).collect();
            for l in std::iter::once(&mut layer).chain(children.iter_mut()).chain(colours.iter_mut()) {
                if let Some(link) = l.couleur.as_mut() {
                    link.relink(&ids, &copies);
                    link.sources.retain(|s| ici.contains(s));
                }
            }
            layer.parent = if layer.folder { None } else { parent_commun };
            for co in colours.iter_mut() {
                co.parent = layer.parent;
            }
            mains.push((layer.id, layer.name.clone()));
            block.extend(children.into_iter().chain(colours).chain(std::iter::once(layer)));
        }
        // Des noms qui ne se répètent pas dans le plan : « Nom (2) » pour le collé.
        let mut pris: std::collections::HashSet<String> = self.layers().iter().map(|l| l.name.clone()).collect();
        for l in block.iter_mut() {
            l.name = crate::layerops::nom_libre(&pris, &l.name);
            pris.insert(l.name.clone());
        }
        let n = mains.len();
        let ids_mains: Vec<u64> = mains.iter().map(|(id, _)| *id).collect();
        let noms: Vec<String> = block.iter().filter(|l| ids_mains.contains(&l.id)).map(|l| l.name.clone()).collect();
        for (k, l) in block.into_iter().enumerate() {
            self.layers_mut().insert(at + k, l);
        }
        crate::folders::normalize(self.layers_mut());
        let courant = *ids_mains.last().unwrap_or(&0);
        self.cur_layer = self.layers().iter().position(|l| l.id == courant).unwrap_or(at);
        // Plusieurs calques collés : ils sont choisis ensemble (le dernier est le courant).
        self.layer_sel = if n > 1 { ids_mains } else { Vec::new() };
        self.vsel = None;
        self.commit_structure(snap);
        self.status = if n > 1 { format!("{n} calques collés : {}.", noms.join(", ")) } else { format!("« {} » collé.", noms.first().map_or("", String::as_str)) };
    }
    /// Fusionne le calque courant avec celui du dessous (Ctrl+E).
    pub(crate) fn merge_layer_down(&mut self) {
        let li = self.cur_layer;
        if li == 0 {
            self.status = "Il n'y a pas de calque en dessous.".into();
            return;
        }
        self.fusionner(vec![li - 1, li]);
    }
    pub(crate) fn undo(&mut self) {
        self.cancel_montage_drag();
        // Une durée rendue par l'annulation n'est pas signalée.
        self.plan_len_seen = None;
        self.end_stroke();
        // (Le trait qu'on annule en plein geste n'est pas à répartir : `horschamp.rs`.)
        self.vignettes.fini = None;
        self.vignettes.debord_garde = None;
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
        self.revalider_rangs();
        self.guides.current = self.guides.current.min(self.doc.guides.len().saturating_sub(1));
        self.vsel = None;
        // (Les clés d'effets et de caméra choisies étaient celles d'avant l'annulation.)
        self.fx_key_sel.clear();
        self.fx_key_anchor = None;
        self.clear_cam_keys();
        self.length_origin = None;
        self.renderer.invalidate_all();
        self.modified = true;
        // Film partagé : annuler ne ressuscite pas un vieux plan de quelqu'un d'autre (`cooperationui.rs`).
        self.partage_apres_historique();
    }

    // ------------------------------------------------------------------ fichiers

    pub(crate) fn request(&mut self, action: Pending) {
        self.end_stroke();
        self.playing = false;
        // La fenêtre « Un projet existe déjà ici » attend sa réponse : la question est déjà posée, rien ne passe avant elle.
        if self.remplacement.is_some() {
            return;
        }
        // Quitter : si d'autres onglets ont quelque chose à perdre, une seule boîte les liste tous (`onglets.rs`) ;
        // sinon la question est celle de toujours, pour le projet affiché.
        if matches!(action, Pending::Quit) && self.d_autres_onglets_a_perdre() {
            self.demander_a_quitter();
            return;
        }
        // Un projet qui arrive dans un onglet à lui (ou dans l'onglet vierge) n'écrase rien : pas de question.
        let nouveau_projet = matches!(
            action,
            Pending::New | Pending::Open | Pending::OpenRecent | Pending::FromChosen | Pending::ScenarioNew
        );
        if nouveau_projet && !self.remplace_le_projet() {
            self.perform(action);
            return;
        }
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
            Pending::FromChosen => self.new_project_from_chosen(),
            Pending::OpenRecent => {
                if let Some(path) = self.recent_to_open.take() {
                    self.open_path(path);
                }
            }
            Pending::ScenarioNew => self.scenario_new_project(),
            // Ferme l'onglet affiché (le dernier est remplacé par un projet vide et l'accueil revient,
            // `accueil.rs`) : `onglets.rs`.
            Pending::Close => self.fermer_l_onglet_affiche(),
            Pending::Quit => {
                if self.quitter_arrete_un_export() {
                    return;
                }
                self.sortie_confirmee = true;
                self.allow_close = true;
                self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    // (Enregistrer : `enregistrement.rs`, sur un autre fil.)

    fn open_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Projet FEANOR", &crate::project::EXTENSIONS)
            .pick_file()
        else {
            return;
        };
        self.open_path(path);
    }

    /// Ouvre un projet en dossier (`.shot`, ou l'ancien nom `.pelure`), ou importe un ancien fichier `.pelure` d'un seul
    /// tenant.
    pub(crate) fn open_path(&mut self, path: PathBuf) {
        // Un projet converti depuis (`Bus.pelure` devenu `Bus.shot`), ou les deux noms après un plantage au milieu de la
        // conversion : le manifeste à ouvrir (le plus récent, `project::manifeste_a_ouvrir`).
        let path = crate::project::manifeste_a_ouvrir(&path);
        // Déjà ouvert dans un onglet : on le montre au lieu de l'ouvrir une seconde fois (`onglets.rs`).
        if self.montrer_si_deja_ouvert(&path) {
            return;
        }
        if io::is_legacy_file(&path) {
            match io::load_legacy(&path) {
                Ok(mut doc) => {
                    // Comme un projet au format 3 : la feuille devient le double du cadre.
                    let (w, h) = (doc.width, doc.height);
                    doc.enlarge_stage(w * 2, h * 2);
                    doc.frame = [w, h];
                    if !self.charger_projet(doc) {
                        return;
                    }
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
        // (Un autre projet prend la place : si l'ancien était un film partagé, ce qu'on y tenait est rendu.)
        self.lacher_la_session();
        self.rejeu.cancel_all();
        self.rejeu_again.clear();
        self.rejeu_warm.clear();
        self.rejeu_deferred.clear();
        self.rejeu_idle.clear();
        crate::fx::fond_cancel_all();
        // (Les suggestions sonores de l'ancien projet s'arrêtent : rien n'est rendu au nouveau.)
        self.reset_suggestions();
        // (L'ancien projet, son historique et ses dessins dérivés sont rendus sur un autre fil, à la fin : ceux d'un film
        // de 300 plans tiennent des centaines de milliers de blocs, `lacher.rs`.)
        let ancien = std::mem::replace(&mut self.doc, doc);
        // (Un plan a toujours un calque : `Document::un_calque_par_plan`.)
        self.doc.un_calque_par_plan();
        // (Une époque que ni cet onglet ni un autre n'a eue : les tâches de fond de l'ancien projet ne lui répondent plus.)
        self.doc_epoch = crate::onglets::nouvelle_epoque();
        // (Ses dessins portent les numéros et les versions de ceux de l'ancien : les dessins
        // dérivés des effets de l'ancien ne lui servent pas.)
        let fx_anciens = crate::fx::forget_all_rendre();
        // (Un autre projet : l'effet choisi et les clés choisies étaient ceux de l'ancien.)
        self.fx_choice = None;
        self.fx_multi.clear();
        self.fx_key_sel.clear();
        self.fx_key_anchor = None;
        self.clear_cam_keys();
        // (Les clés de caméra copiées restent : ce sont des valeurs, sans numéro de projet — elles se collent dans
        // l'autre onglet, `onglets.rs`.)
        self.comment_ui = Default::default();
        // (Un projet qui s'ouvre : la planche des Vignettes se recadre sur ses 2 rangées de 6 cases, `vignettesprefs::ouverture_cadre`.)
        self.board_zoom = 0.0;
        self.board_pan = Vec2::ZERO;
        self.vignettes.ouverture_a_faire = true;
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
        let etapes = self.history.take_steps();
        self.gel.oublier_le_projet();
        self.thumbs.clear();
        self.layer_thumbs.clear();
        self.renderer.invalidate_all();
        self.fit_pending = true;
        crate::lacher::en_fond((ancien, etapes, fx_anciens));
        // Vue du plateau et vue 3D recadrées sur le nouveau projet (9d, 9e).
        self.stage_view = None;
        self.espace3d_reset();
        self.modified = false;
        self.path = None;
        self.import_source = None;
        self.written.clear();
        self.length_origin = None;
        // Ce qui était choisi ou visé l'était dans l'ancien projet, dont les numéros de plans et de calques reviennent dans le
        // nouveau (chaque projet compte depuis le début) : rien ne reste choisi, aucun menu ne vise plus rien.
        self.board_sel.clear();
        self.board_notes.clear();
        self.board_text = None;
        self.layer_sel.clear();
        self.frame_sel = None;
        self.key_sel.clear();
        self.key_anchor = None;
        self.trim_menu = None;
        self.vignettes.menu = None;
        self.vignettes.note = None;
        self.revalider_rangs();
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
        self.espace3d_reset();
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

    pub(crate) fn export_frame(&mut self) {
        self.end_stroke();
        if !self.export_libre() {
            return;
        }
        let Some(mut path) = self.dialogue_enregistrer("image", "Exporter l'image courante", ("Image PNG", &["png"]), &format!("image_{:04}.png", self.global_frame() + 1)) else {
            return;
        };
        if path.extension().is_none() {
            path.set_extension("png");
        }
        // Le plan que montre l'écran (sans pelure ni table lumineuse) se fait ici, sans pixel ; l'image, avec ses
        // incrustations (timecode…) gravées, se calcule sur un autre fil (`ExportJob::still`).
        let plan = self.build_plan_avec(self.cur_frame, false, !self.export_transparent, self.settings.export_annotations);
        let cam = self.doc.panel_camera(self.cur_panel, self.cur_frame);
        let job = crate::export::ExportJob::still(&path, plan, cam, self.cur_panel, self.cur_frame, self.doc.frame);
        self.start_job(job);
    }
    /// Pose une image (PNG/JPEG) au centre du dessin courant d'un calque bitmap ; sur un calque vectoriel, elle devient un objet
    /// du dessin (`imagevec.rs`).
    pub(crate) fn import_image(&mut self) {
        self.end_stroke();
        let vectoriel = self.layer().kind == LayerKind::Vector && !self.layer().folder;
        if !vectoriel && (self.layer().kind != LayerKind::Bitmap || self.layer().folder) {
            self.status = "Choisis un calque bitmap ou vectoriel pour importer une image.".into();
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg"])
            .pick_file()
        else {
            return;
        };
        self.import_image_fichier(&path);
    }

    /// Pose l'image `path` (PNG/JPEG) sur le calque courant, comme « Importer une image » (le choix du fichier en moins) :
    /// aussi pour une image de la bibliothèque (`biblioui.rs`).
    pub(crate) fn import_image_fichier(&mut self, path: &std::path::Path) {
        self.end_stroke();
        let vectoriel = self.layer().kind == LayerKind::Vector && !self.layer().folder;
        if !vectoriel && (self.layer().kind != LayerKind::Bitmap || self.layer().folder) {
            self.status = "Choisis un calque bitmap ou vectoriel pour importer une image.".into();
            return;
        }
        let img = match io::load_image(path) {
            Ok(i) => i,
            Err(e) => {
                self.status = e;
                return;
            }
        };
        if vectoriel {
            let (w, h) = (img.width() as usize, img.height() as usize);
            self.paste_image_as_object((w, h, img.into_raw()));
            return;
        }
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

    pub(crate) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
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
        // (Sur l'accueil, le curseur est d'office dans le nom du nouveau projet : les raccourcis qui ne s'écrivent pas, Ctrl+O,
        // Ctrl+N, Ctrl+W, F1, F11…, passent quand même ; `accueilfenetre::raccourci_hors_saisie`.)
        let saisie = ctx.wants_keyboard_input() && !(self.accueil_visible() && crate::accueilfenetre::raccourci_hors_saisie(ctx));
        if saisie || self.modal_open() {
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
        if !self.sans_projet() && ctx.input(|i| crate::nudge::ARROWS.iter().any(|k| i.key_down(*k))) {
            self.keep_gesture(crate::nudge::GESTURE);
        }
        for (key, physical, m, repeat) in keys {
            // Espace, flèches, Retour arrière et Entrée, la souris sur le panneau Bibliothèque : ce sont les siennes (`biblioui.rs`).
            if self.biblio.touche_prise(ctx, key, m) {
                continue;
            }
            // Entrée, Échap, Retour arrière servent d'abord à l'outil en cours (forme à finir…).
            let sans_projet = self.sans_projet();
            if !sans_projet && !m.any() && self.tool_key(key) {
                continue;
            }
            // Une sélection sur le dessin : les flèches la déplacent (`nudge.rs`), avant de changer
            // d'image ou de calque.
            if let Some(d) = crate::nudge::arrow_step(key, m).filter(|_| !sans_projet && self.nudge_ready()) {
                self.nudge_selection(d);
                continue;
            }
            // Ctrl+F, pointeur sur le panneau Scénario : c'est la recherche du panneau (`scenariorecherche.rs`), pas « Aller à un plan ».
            if key == Key::F && m.command && !m.shift && !m.alt && self.scenario_ui.recherche_prise(ctx) {
                continue;
            }
            // Les raccourcis suivent le panneau qui a la main (montage, timeline ou dessin).
            let Some(action) = self.keymap.find_in(key, physical, m, self.key_mode(), self.espace == crate::workspace::Espace::Montage) else { continue };
            let action = action.to_string();
            if repeat && !keymap::action_info(&action).is_some_and(|a| a.repeat) {
                continue;
            }
            // La touche d'un outil : tenue, elle le prend le temps de la tenir (`springtools`).
            let before = self.tool;
            // Le compteur d'usage (`usageui.rs`) : partie du clavier ; une touche tenue qui se répète ne compte qu'une fois.
            self.usage.source = Some(crate::usage::Source::Clavier);
            self.usage.muet = repeat;
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
        // Aucun projet ouvert : une commande qui en vise un ne part pas (`sansprojet.rs`). Le document caché ne change pas.
        if self.action_bloquee(action) {
            self.status = "Crée ou ouvre d'abord un projet.".into();
            return;
        }
        // (Le rapport d'une erreur dit quelle commande venait d'être lancée, `plantage.rs`.)
        crate::plantage::noter_action(action);
        // Le compteur d'usage du poste et le journal des dernières commandes (`usageui.rs`).
        self.usage_noter(action);
        let name = crate::keymap::ACTIONS.iter().find(|a| a.id == action).map(|a| crate::historyui::step_name(a.label));
        let before = self.history.context.clone();
        if name.is_some() {
            self.history.context = name;
        }
        // Dans les Vignettes, les outils sont le pinceau, le crayon, la gomme et la caméra (décision 2 de l'analyse) : une touche d'un autre
        // outil ne change rien et le dit. Le lasso de Ctrl (`prendre_le_lasso`) et la main (Espace, l'outil Main) ne passent pas par ici.
        if let Some(refuse) = self.outil_refuse_dans_les_vignettes(action) {
            self.status = format!("Dans les Vignettes : pinceau, crayon, gomme et caméra ({} reste au Storyboard, Maj+Tab). Choisir : Ctrl + glisser.", refuse.label());
            self.history.context = before;
            return;
        }
        self.run_action_now(action);
        self.history.context = before;
        self.vignettes_apres_la_commande(action);
    }

    /// L'outil que `action` prendrait et que les Vignettes refusent (partie 113, décision 2) : tout sauf le pinceau, le crayon, la gomme,
    /// la caméra et la main. Les formes (ligne, rectangle, ellipse, polyligne) sont refusées aussi : la planche est faite pour le croquis
    /// à main levée, et une forme posée sur une case pâle y faisait naître un plan sans trait.
    fn outil_refuse_dans_les_vignettes(&self, action: &str) -> Option<Tool> {
        if self.espace != crate::workspace::Espace::Vignettes {
            return None;
        }
        let outil = match action {
            "outil.formes" => Some(self.shape.last),
            "outil.ligne_droite" | "outil.ligne_courbe" | "outil.ligne_s" => Some(Tool::Line),
            "deformer.enveloppe" | "deformer.perspective" => Some(Tool::Perspective),
            a => Tool::ALL.into_iter().find(|t| t.action() == a),
        }?;
        (!Tool::permis_dans_les_vignettes(outil)).then_some(outil)
    }

    /// Sur la planche des Vignettes, après une commande (partie 113) : une commande qui choisit un plan (plan suivant, précédent, nouveau,
    /// dupliqué) le montre, cadre et planche défilée jusqu'à sa case : sans cela le plan courant changeait sans qu'on le voie.
    fn vignettes_apres_la_commande(&mut self, action: &str) {
        if !self.board_shown() {
            return;
        }
        if matches!(action, "panneau.suivant" | "panneau.precedent" | "scene.suivante" | "scene.precedente" | "panneau.nouveau" | "scene.nouvelle" | "panneau.dupliquer" | "scene.dupliquer" | "plan.nouveau_avant") {
            self.vignettes_montrer_le_plan_courant();
        }
    }

    /// Le plan courant est choisi et se voit sur la planche : cadre noir, et la planche défile jusqu'à sa case à l'image suivante.
    pub(crate) fn vignettes_montrer_le_plan_courant(&mut self) {
        if let Some(p) = self.doc.panels.get(self.cur_panel) {
            self.board_sel = vec![p.id];
        }
        self.vignettes.libre = false;
        self.vignettes.camera_choisie = None;
        self.vignettes.montrer_courante = true;
    }

    fn run_action_now(&mut self, action: &str) {
        // « Durée d'un nouveau plan » (Préférences > Montage) : lue par les plans que la commande crée (`dureeplans.rs`).
        self.doc.duree_plan_neuf = self.settings.new_plan_secs;
        // Tout le film est annulé : aucun export, aucun envoi (`planannule.rs`), un seul point pour toutes les sorties.
        if (action.starts_with("fichier.exporter") || action == "fichier.kitsu_publier") && self.doc.film_vide() {
            self.status = crate::planannuleui::REFUS_EXPORT.into();
            return;
        }
        // Un texte en cours de saisie est posé avant toute commande (annuler, changer d'image…).
        self.finish_text_edit();
        // Une autre copie (dessin, plans, images, clés…) prend la place des effets copiés pour Ctrl+V
        // depuis le panneau Calques ; la copie d'effets, plus bas, la redonne (`fxcopie.rs`).
        if matches!(action, "edition.copier" | "edition.couper") {
            self.fx_pile_recent = false;
            self.layer_clip_recent = false;
        }
        // Le panneau Références a la main : Ctrl+V y colle l'image du presse-papiers.
        if self.refs.focus && matches!(action, "edition.coller" | "edition.coller_a_la_tete" | "edition.coller_sur_place") {
            self.refs_paste();
            return;
        }
        // Une image copiée ailleurs (navigateur…) passe avant le presse-papiers de SHOT, dont
        // les copies écrivent « FEANOR : … » dans celui de Windows (`pasteimage.rs`).
        // (Le panneau Calques a la main et un calque vient d'être copié : Ctrl+V colle ce calque, pas une image restée dans le
        // presse-papiers de Windows.)
        let calque_a_coller = self.layers_pane_focus && self.layer_clip_recent && self.layer_clipboard.is_some();
        if (action == "edition.coller" || action == "edition.coller_a_la_tete") && !calque_a_coller {
            if let Some(image) = crate::pasteimage::clipboard_image() {
                self.paste_clipboard_image(image);
                return;
            }
        }
        // Un champ de la boîte de sélection (Réglages) en cours de saisie : son geste se termine
        // avant toute commande (annuler, changer d'image…).
        self.finish_field_edit();
        // De même pour un mouvement de caméra en cours dans les Vignettes (`vignettecamui.rs`) ; pas pour les touches qui se
        // tiennent pendant un geste (la touche de la caméra elle-même, la main, les vues temporaires, l'aide-mémoire).
        if !matches!(action, "vignettes.camera" | "vue.pan_libre" | "vue.rotation_temporaire" | "vue.zoom_temporaire_avant" | "vue.zoom_temporaire_arriere" | "aide.aide_memoire") {
            self.poser_le_geste_de_camera();
            self.poser_la_creation();
            // Un déplacement de sélection lâché dont la feuille flottante attend encore le rendu exact se termine d'abord, comme à
            // un clic ou une touche (`flush_flottante_on_input`) : la commande part d'un dessin juste. Sinon « Supprimer la
            // sélection » lancée sans clic ni touche laissait la feuille en attente, qui reposait plus tard ses éléments d'avant
            // aux rangs d'avant (l'élément supprimé revenait, un autre disparaissait, sans étape d'annulation).
            self.flush_flottante();
            self.multi_flush();
            // Une zone bitmap transformée dont le rendu exact se calcule encore.
            self.flush_bitmap_float();
        }
        // Annuler ou rétablir pendant une déformation : elle repartirait d'un dessin périmé.
        if matches!(action, "edition.annuler" | "edition.retablir") {
            self.persp = None;
            self.envelope = None;
        }
        if self.workspace_action(action) {
            return;
        }
        // Mode concentration, aide-mémoire, conseils, réglage du stylet (`confort.rs`).
        if self.confort_action(action) {
            return;
        }
        // Fond sous le trait (`fondui.rs`).
        if self.fond_action(action) {
            return;
        }
        // Brosse par personnage (`personnagesui.rs`).
        if self.perso_action(action) {
            return;
        }
        // Préréglages d'effets et calques d'effets (`prereglagesui.rs`).
        if self.effet_action(action) {
            return;
        }
        // Espace 3D (`espace3dui.rs`).
        if self.espace3d_action(action) {
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
        // Travailler à plusieurs : prendre, rendre, l'équipe, partager (`cooperationui.rs`).
        if self.cooperation_action(action) {
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
        // Variantes d'une séquence (`variantesui.rs`).
        if self.variante_action(action) {
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
        // Panneau Bibliothèque de fichiers (`biblioui.rs`) ; Retour arrière y remonte d'un dossier quand il a la main.
        if self.bibliotheque_action(action) {
            return;
        }
        if action == "edition.supprimer" && self.biblio.focus && self.pane_shown(crate::dock::Pane::Bibliotheque) {
            // (Supprimer ne supprime rien dans la bibliothèque : elle est en lecture seule.)
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
            // Onglets de projets (`onglets.rs`).
            "onglet.nouveau" => {
                self.nouvel_onglet();
            }
            "onglet.suivant" => self.onglet_voisin(1),
            "onglet.precedent" => self.onglet_voisin(-1),
            "onglet.fermer_autres" => {
                let garde = self.onglets[self.actif].id;
                self.fermer_les_autres(garde);
            }
            "aide.palette" => self.palette = Some(Default::default()),
            "fichier.exporter" => self.export_hub = true,
            "fichier.preferences" => self.prefs_window = true,
            "fichier.corbeille" => self.ouvrir_la_corbeille(),
            "aide.a_propos" => self.about_window = true,
            "aide.diagnostic" => self.open_diagnostic(),
            // Le compteur d'usage et les notes de friction (`usageui.rs`, `frictionui.rs`).
            "aide.habitudes" => self.usage.fenetre = !self.usage.fenetre,
            "aide.stylet_transmet" => self.stylet_basculer(),
            "aide.noter_friction" => self.friction_commencer(),
            "aide.ouvrir_frictions" => self.friction_ouvrir_le_dossier(),
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
            "fichier.exporter_gif" => self.open_export(crate::export::Format::Gif),
            "fichier.exporter_montage" => self.open_editorial(),
            "fichier.exporter_fiche" => self.open_fiche(),
            "fichier.exporter_liste_plans" => self.open_liste_plans(),
            "fichier.exporter_paquet" => self.open_paquet(),
            "fichier.exporter_renumerotation" => self.exporter_renumerotation(),
        "fichier.kitsu_publier" => self.open_kitsu(crate::kitsuui::Section::Publier),
        "fichier.kitsu_retours" => self.open_kitsu(crate::kitsuui::Section::Retours),
            "projet.noms_production" => self.open_noms(),
            "projet.enregistrer_modele_episode" => self.open_modele_episode(),
            "fichier.exporter_relecture" | "fichier.importer_notes" => {
                self.relecture_action(action);
            }
            "numerotation.verrouiller" => self.toggle_numbering_lock(),
            "numerotation.renumeroter" => self.renumber_confirm = true,
            "fichier.exporter_image_courante" => self.export_frame(),
            // Vignettes : annotations par-dessus la planche et marie-louise (`annotationsui.rs`).
            "vignettes.annotation_stylo" => self.annot_choisir(crate::annotationsui::Outil::Stylo),
            "vignettes.annotation_gomme" => self.annot_choisir(crate::annotationsui::Outil::Gomme),
            "vignettes.annotation_lasso" => self.annot_choisir(crate::annotationsui::Outil::Lasso),
            "vignettes.annotation_calque_nouveau" => self.annot_calque_nouveau(),
            "vignettes.annotations_afficher" => self.annot_basculer_visibles(),
            "vignettes.annotations_effacer" => self.annot_demander_effacement(),
            "vignettes.marie_louise" => self.marie_basculer(),
            "vignettes.note" => self.note_ecrire_courante(),
            // Le panneau « Vignettes » : sens de lecture et textes à côté des cases (`vignettespanneau.rs`).
            "vignettes.sens_lignes" => self.vignettes_regler_sens(crate::boardsens::Sens::Lignes),
            "vignettes.sens_japonais" => self.vignettes_regler_sens(crate::boardsens::Sens::Japonais),
            "vignettes.numeros" => self.vignettes_basculer_numeros(),
            "vignettes.texte_action" => self.vignettes_basculer_texte(crate::boardsens::Champ::Action),
            "vignettes.texte_dialogue" => self.vignettes_basculer_texte(crate::boardsens::Champ::Dialogue),
            "vignettes.texte_notes" => self.vignettes_basculer_texte(crate::boardsens::Champ::Notes),
            "fichier.exporter_planche" => self.open_planche(),
            "fichier.exporter_colorscript" => self.open_colorscript(),
            "fichier.exporter_images_par_plan" => {
                self.open_export(crate::export::Format::Stills);
                // (Cette entrée est « par plan » : un choix « par clé » retenu d'une séance à l'autre ne s'y invite pas.)
                if let Some(d) = self.export_dialog.as_mut().filter(|d| d.stills.per == crate::exportstills::StillsPer::Cle) {
                    d.stills.per = crate::exportstills::StillsPer::Plan;
                }
            }
            "fichier.exporter_images_cles" => {
                self.open_export(crate::export::Format::Stills);
                if let Some(d) = self.export_dialog.as_mut() {
                    d.stills.per = crate::exportstills::StillsPer::Cle;
                }
            }
            "fichier.exporter_son_par_piste" => self.open_export(crate::export::Format::Stems),
            "fichier.exporter_son_par_plan" => self.open_export(crate::export::Format::SonPlans),
            "fichier.exporter_calques" => self.open_export(crate::export::Format::Layers),
            "fichier.exporter_camera" => self.open_export(crate::export::Format::Camera),
            "edition.annuler" => self.undo(),
            "edition.retablir" => self.redo(),
            // Le panneau Effets a la main (le dernier clic était dedans) : Ctrl+C copie l'effet
            // choisi et Ctrl+V colle les effets copiés, avant tous les autres presse-papiers
            // (`fxcopie.rs`). Sans effet sur la pile, Ctrl+C suit les règles d'avant.
            "edition.copier" if self.fx_pane_has_focus() && !self.layer().fx.is_empty() => self.fx_copy_chosen(),
            // Coller des effets : le panneau Effets a la main, ou le panneau Calques l'a et la dernière
            // copie est celle d'effets (on a cliqué le calque où les coller).
            "edition.coller" if self.fx_paste_has_hand() => self.fx_paste(self.fx_paste_targets(self.cur_layer), false),
            // Le panneau Calques a la main (le dernier clic était dedans) : Ctrl+C copie le calque courant, et Ctrl+V colle le dernier
            // calque copié (ici ou dans un autre plan), comme W et X (retour d'Anthony, 2026-10-24 : « copier-coller un calque sur un autre
            // plan ne fait rien » : Ctrl+C copiait la sélection du dessin, et Ctrl+V n'avait rien à coller). Un clic ailleurs rend ces
            // touches à ce qui est cliqué (clés, images, plans, dessin) ; après une autre copie, Ctrl+V garde son sens d'avant.
            "edition.copier" if self.layers_pane_focus => self.copy_layer(),
            "edition.coller" if self.layers_pane_focus && self.layer_clip_recent && self.layer_clipboard.is_some() => self.paste_layer(),
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
            // Des clés de position choisies dans la timeline (`poskeysui.rs`).
            "edition.supprimer" if self.pos_keys_have_focus() => self.pos_keys_delete(),
            "calque.cle_position" => self.pos_key_here(),
            "calque.cle_position_lisser" => self.pos_key_smooth(),
            "calque.cle_position_supprimer" => self.pos_keys_delete(),
            "calque.cles_position_supprimer" => self.pos_keys_clear(),
            // Les clés d'opacité (`opacitecleselui.rs`).
            "edition.supprimer" if self.op_keys_have_focus() => self.op_keys_delete(),
            "calque.cle_opacite" => self.op_key_here(),
            "calque.cle_opacite_lisser" => self.op_key_smooth(),
            "calque.cle_opacite_supprimer" => self.op_keys_delete(),
            "calque.cles_opacite_supprimer" => self.op_keys_clear(),
            "calque.nouveau_couleur_unie" => self.nouveau_calque_couleur_unie(),
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
            // Les images copiées de la timeline (grille) : Maj+Ctrl+V les pose sur la tête de lecture, sans rien repousser.
            "edition.coller_a_la_tete" if self.images_focus && self.image_clipboard.is_some() => self.paste_images_at_head(),
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
            "edition.coller" if self.multi_clip.is_some() && self.chosen_layers().len() >= 2 => self.multi_paste(),
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
            // Des annotations choisies au lasso d'annotation : Suppr les efface, en une étape (`annotationsui.rs`).
            "edition.supprimer" if self.board_shown() && !self.vignettes.annot.selection.is_empty() => {
                self.annot_supprimer_selection();
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
            // Maj+Suppr : une zone bitmap choisie est remplie de la couleur courante (`remplirsel.rs`) ;
            // sinon la touche garde son ancien sens : effacer hors de la sélection.
            "selection.remplir" if self.bsel.is_some() && self.multi.sel.is_none() => {
                self.bitmap_fill_selection();
            }
            "dessin.supprimer_hors_selection" | "selection.remplir" => {
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
            // Maj+Tab depuis l'espace Vignettes : le dessin du Storyboard (partie 113). Avant, la planche cédait la place au dessin seul
            // sous l'onglet Vignettes, d'où rien ne la ramenait (l'onglet, déjà allumé, ne faisait rien), et la disposition retenue
            // rouvrait les Vignettes sur le dessin à la séance suivante.
            "vue.planches" if self.espace == crate::workspace::Espace::Vignettes => {
                self.set_espace(crate::workspace::Espace::Storyboard);
                self.board_view = BoardView::Scene;
            }
            // Et depuis partout ailleurs, Maj+Tab mène aux Vignettes (décision 1 de l'analyse) : le pont dans les deux sens. L'ancienne
            // « vue vignettes » de la disposition du Storyboard (la planche au milieu des panneaux du Storyboard) n'existe plus : la
            // planche, c'est l'espace Vignettes.
            "vue.planches" => self.set_espace(crate::workspace::Espace::Vignettes),
            // Tab : entrer dans le plan pour l'animer, et en sortir (décision 1). Du Montage, il revient au storyboard (le montage
            // n'est pas un côté de la bascule : « Animer ce plan », dans sa barre, entre dans le plan). Des Vignettes, il entre dans le
            // plan courant ; de l'animation, il revient d'où l'on est entré (`set_mode` suit `Settings::storyboard_space`).
            "mode.basculer" if self.espace == crate::workspace::Espace::Montage => self.set_espace(crate::workspace::Espace::Storyboard),
            "mode.basculer" => self.set_mode(if self.mode == Mode::Storyboard { Mode::Animation } else { Mode::Storyboard }),
            "aide.raccourcis" => self.show_help = !self.show_help,
            "outil.pinceau" => self.set_tool(Tool::Brush),
            "outil.crayon" => self.set_tool(Tool::Pencil),
            "outil.gomme" => self.set_tool(Tool::Eraser),
            "outil.remplissage" => self.set_tool(Tool::Fill),
            "outil.degrade" => self.set_tool(Tool::Gradient),
            "outil.melangeur" => self.set_tool(Tool::Mixer),
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
            "outil.grab" => self.set_tool(Tool::Grab),
            "outil.pull" => self.set_tool(Tool::Pull),
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
            "montage.lecture_avant" => self.lecture_touche(crate::jkl::Touche::L),
            "montage.lecture_arriere" => self.lecture_touche(crate::jkl::Touche::J),
            "montage.lecture_arret" => self.lecture_touche(crate::jkl::Touche::K),
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
                // (Exclusif avec « Peint sur l'aplat ».)
                self.paint_on_aplat &= !self.paint_behind;
                self.status = if self.paint_behind { "Peindre derrière : la peinture passe sous ce qui est déjà dessiné." } else { "Peindre derrière : non." }.into();
            }
            "pinceau.sur_aplat" => {
                self.paint_on_aplat = !self.paint_on_aplat;
                self.paint_behind &= !self.paint_on_aplat;
                self.status = if self.paint_on_aplat {
                    "Peint sur l'aplat : le pinceau et le crayon peignent dans l'aplat du dessin (calque de couleur, sinon l'aplat sous le trait), pas sur le trait."
                } else {
                    "Peint sur l'aplat : non."
                }
                .into();
            }
            // (« pinceau.remplir », l'ancien « Remplir la forme », est maintenant un double de « pinceau.fond » : `fond_action`.)
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
            // Pistes repliables du montage (`pistes.rs`).
            "piste.basculer" => {
                let row = self.track_row_of(self.cur_layer);
                self.toggle_track_fold(row, false);
            }
            "piste.replier_tout" => {
                self.set_all_tracks_folded(true);
            }
            "piste.deplier_tout" => {
                self.set_all_tracks_folded(false);
            }
            "piste.auto_deplier" => self.toggle_auto_open(),
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
            // Calque de couleur (`calquecouleurui.rs`) : en ajouter un, le détacher, griffonner.
            "calque.nouveau_couleur" => self.new_colour_layer(),
            "calque.nouveau_annotation" => self.nouveau_calque_d_annotation(),
            "calque.detacher_couleur" => self.detach_colour_layer(),
            "calque.griffonner" => self.toggle_griffonner(),
            // Calque vidéo (13k, `videoui.rs`).
            "fichier.importer_video" => self.import_video(),
            "fichier.importer_scenario" => self.open_scenario_window(),
            "scenario.creer_plans" => self.open_plans_window(),
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
            // Bouton « fx » de la ligne du calque (`fxui.rs`).
            "calque.effets" => self.toggle_layer_fx(self.cur_layer),
            "groupes.focus" => self.focus_current_group(),
            "panneau.couper" => self.split_current_panel(),
            "panneau.fusionner_suivant" => self.merge_next_panel(),
            "panneau.fusionner_choisis" => self.merge_chosen_panels(),
            "sequence.nouvelle_avec_plans" => self.new_sequence_from_chosen(),
            // La planche par séquences (`planchesequencesui.rs`).
            "sequence.nouvelle_fin" => self.sequence_nouvelle_fin(),
            "sequence.nouvelle_avec_cases" => self.new_sequence_from_cases(),
            "sequence.couper_ici" => self.sequence_couper_ici(),
            "sequence.renommer" => self.sequence_renommer_courante(),
            "sequence.afficher_toutes" => self.affichage_toutes(),
            "sequence.afficher_courante" => self.affichage_courante_seule(),
            "sequence.afficher_aucune_autre" => self.affichage_aucune_autre(),
            "sequence.afficher_ordre_film" => self.affichage_ordre_du_film(),
            "montage.duree_plans" => self.open_plans_duration_dialog(),
            "scene.couper" => self.split_current_panel(),
            "scene.joindre_precedente" => self.join_previous_scene(),
            "calque.copier" => self.copy_layer(),
            "calque.coller" => self.paste_layer(),
            "calque.verrouiller" => self.toggle_layer_lock(self.cur_layer),
            "calque.fusionner_dessous" => self.merge_layer_down(),

            "outil.camera" => self.set_tool(Tool::Camera),
            "outil.transformer" => self.set_tool(Tool::Transform),
            "calque.reinit_transformation" => self.reset_layer_transform(),
            "camera.vue" => self.set_camera_view(!self.camera_view),
            "vue.cadres_securite" => self.layout.safe_frames = !self.layout.safe_frames,
            // Le passe-partout autour du cadre de la caméra : allumé ou éteint, retenu dans les réglages du poste.
            "camera.masque" => {
                self.settings.passe_partout_actif = !self.settings.passe_partout_actif;
                let _ = self.settings.save();
            }
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
            // « vignettes.camera » se maintient aussi : la planche des Vignettes le lit en continu (`boardui.rs`).
            "vignettes.camera" => {}
            other => self.status = format!("« {other} » n'est pas encore disponible."),
        }
    }

    /// Touche pour l'outil en cours (voir `handle_shortcuts`) ; vrai si elle a servi.
    fn tool_key(&mut self, key: Key) -> bool {
        self.shape_key(key) || self.persp_key(key) || self.envelope_key(key) || self.keys_escape(key)
    }

    /// Change d'outil : la forme en cours (courbe, polyligne) est posée avant.
    pub(crate) fn set_tool(&mut self, tool: Tool) {
        // Prendre un outil de dessin range le stylo et la gomme d'annotation (`annotationsui.rs`).
        self.vignettes.annot.outil = None;
        if self.tool != tool {
            self.finish_field_edit();
            self.finish_shape_edit();
            self.finish_text_edit();
            self.persp = None;
            self.envelope = None;
            self.end_stroke();
            // Quitter l'outil Caméra des Vignettes pour un autre : plus de caméra choisie, et plus de plan choisi (dessiner est libre).
            if self.tool == Tool::Camera && tool != Tool::Camera {
                self.vignettes.camera_choisie = None;
                self.vignettes.libre = true;
            }
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
        self.keymap = Keymap::load();
        self.status = match (&self.keymap.user_file, self.keymap.warnings.len()) {
            (Some(p), 0) => format!("Raccourcis rechargés depuis {}.", p.display()),
            (Some(p), n) => format!("Raccourcis rechargés depuis {} ({n} remarque(s), voir F1).", p.display()),
            (None, 0) => "Raccourcis par défaut.".to_string(),
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
                    "Raccourcis ouverts dans le Bloc-notes ({}). Enregistre, puis « Recharger » (Fichier > Préférences…, ou la liste des raccourcis, F1).",
                    path.display()
                )
            }
            Err(e) => self.status = format!("Impossible d'ouvrir {} : {e}", path.display()),
        }
    }

    fn handle_close_request(&mut self, ctx: &egui::Context) {
        // (Pendant un enregistrement, la fermeture l'attend : `request` retient la demande.)
        // (Tous les onglets comptent : un projet modifié dans un onglet caché retient aussi la fermeture.)
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            if self.infos_onglets().iter().any(|i| i.a_perdre()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.request(Pending::Quit);
            } else if self.quitter_arrete_un_export() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }
    }

    /// Les exports qui tournent encore (le film, le montage, la fiche, la page de relecture), par leur nom.
    pub(crate) fn exports_en_cours(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.export_job.as_ref().is_some_and(|j| j.outcome.is_none()) {
            v.push("l'export");
        }
        if self.editorial.as_ref().is_some_and(crate::editorialui::EditorialWindow::busy) {
            v.push("l'export pour le montage");
        }
        if self.fiche.as_ref().is_some_and(crate::ficheui::FicheWindow::busy) {
            v.push("la fiche technique");
        }
        if self.paquet.as_ref().is_some_and(crate::paquetui::PaquetWindow::busy) {
            v.push("le paquet de livraison");
        }
        if self.journal.as_ref().is_some_and(crate::journalui::JournalFenetre::busy) {
            v.push("le journal des différences");
        }
        if self.kitsu.busy() {
            v.push("le pont Kitsu");
        }
        if self.onglets.iter().enumerate().any(|(i, _)| if i == self.actif { self.relecture.export_en_cours() } else { self.onglets[i].etat.as_ref().is_some_and(|e| e.relecture.export_en_cours()) }) {
            v.push("la page de relecture");
        }
        v
    }

    /// « Quitter » arrêterait un export en cours : la première fois, SHOT le dit au lieu de quitter ; un deuxième « Quitter »
    /// dans les quinze secondes quitte pour de bon (l'export est arrêté, ses fichiers inachevés effacés). Vrai : on ne quitte pas.
    fn quitter_arrete_un_export(&mut self) -> bool {
        let en_cours = self.exports_en_cours();
        if en_cours.is_empty() {
            return false;
        }
        if self.quitter_prevenu.is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(15)) {
            return false;
        }
        self.quitter_prevenu = Some(std::time::Instant::now());
        self.status = format!("Quitter arrêterait {} : attends sa fin, ou refais « Quitter » pour l'arrêter.", en_cours.join(" et "));
        true
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        // Sans projet ouvert : le titre est celui du logiciel (`sansprojet.rs`).
        if self.sans_projet() {
            if self.title != "FEANOR" {
                ctx.send_viewport_cmd(egui::ViewportCommand::Title("FEANOR".into()));
                self.title = "FEANOR".into();
            }
            return;
        }
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Sans titre".into());
        let title = format!("{name}{} — FEANOR", if self.modified || self.saving.is_some() { " •" } else { "" });
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
        // Le stylet lu auprès de Windows, avant tout : son bouton devient le clic droit, retourné il prend la Gomme (`styletlien.rs`).
        self.stylet_brancher(raw_input);
        // Pipette d'un champ de couleur armée : les clics et Échap sont pour elle seule (`champcouleur.rs`).
        crate::champcouleur::filtrer_entree(ctx, raw_input);
        // Texte écrit sur la planche : Échap et Ctrl+Entrée le posent, et ne vont à rien d'autre (egui ôterait le curseur
        // dès Échap, et le raccourci de l'application le verrait).
        self.refs_filter_input(raw_input);
        if ctx.wants_keyboard_input() {
            return;
        }
        raw_input.events.retain(|e| match e {
            // (Ctrl+Tab aussi : egui y verrait un changement de champ ; ici c'est l'onglet suivant, `onglets.rs`.)
            Event::Key {
                key: Key::Tab,
                physical_key,
                pressed,
                modifiers,
                ..
            } => {
                if *pressed {
                    self.pending_keys.push((Key::Tab, *physical_key, *modifiers));
                }
                false
            }
            _ => true,
        });
    }

    /// Le fond derrière tout ce que l'interface peint : celui d'eframe, sauf quand la fenêtre à part de la planche de
    /// références est transparente (`planchref.rs`) : alors rien ne le remplit.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        if self.settings.references_fenetre.transparent && self.detached.iter().any(|d| d.panneau == crate::dock::Pane::References) {
            return [0.0, 0.0, 0.0, 0.0];
        }
        // Thèmes de la marque : la marge entre la barre d'outils et les panneaux prend la couleur des panneaux,
        // sinon elle ferait une bande noire dans le thème clair.
        let p = crate::theme::palette();
        if p.identite {
            return p.panneau.to_normalized_gamma_f32();
        }
        egui::Color32::from_rgba_unmultiplied(12, 12, 12, 180).to_normalized_gamma_f32()
    }

    /// Fermeture normale (même sans enregistrer) : voir `App::fermeture`.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.fermeture();
        self.sortie_sans_rendre();
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
        // La fenêtre perd le focus : une retouche de dégradé en cours s'écrit d'abord (rien ne reste « défait »).
        if ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::WindowFocused(false)))) {
            self.degrade_abandonner();
        }
        // Pipette d'un champ de couleur (`champcouleur.rs`) : sans capture d'écran, la couleur du dessin sous le curseur.
        if let Some(pos) = crate::champcouleur::surveiller(ctx) {
            let couleur = self.couleur_sous_le_curseur(pos);
            if couleur.is_none() {
                self.status = "La pipette n'a pas pu lire cet endroit : clique dans le dessin.".into();
            }
            crate::champcouleur::fournir_repli(ctx, couleur);
        }
        // Plein écran à l'ouverture (Anthony, 2026-10-18) : la fenêtre, créée cachée, perd la
        // consigne de `with_maximized` sous Windows : elle est marquée maximisée mais garde sa taille.
        // Une fois affichée, on la remet à sa taille, puis on la maximise vraiment.
        // (Sans objet quand la fenêtre est l'accueil compact : elle n'est pas maximisée, `accueilfenetre.rs`.)
        self.regler_la_fenetre(ctx);
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
        // Taille de l'interface choisie dans les préférences (`echelle.rs`).
        self.appliquer_echelle(ctx);
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
        // Le tremblement du trait par défaut (Préférences > Dessin) : posé là où le rendu le lit.
        self.appliquer_tremblement_defaut(ctx);
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
            // Et le panneau Calques (`layersui.rs`) : Ctrl+V y colle des effets copiés.
            self.layers_pane_focus = false;
            // Et pour le panneau Références (`referencesui.rs`).
            self.refs.focus = false;
            // Et la bibliothèque de fichiers (`biblioui.rs`) : elle le reprend si le clic est chez elle.
            self.biblio.focus = false;
        }
        // Échap ferme la fenêtre des réglages du clic droit et ne fait rien d'autre (`reglagesdroit.rs`).
        self.reglages_droit_echap(ctx);
        // De même pour un geste de l'anneau sous le stylet (`anneauui.rs`).
        self.anneau_echap(ctx);
        // Et pour une note de friction en cours (`frictionui.rs`).
        self.friction_echap(ctx);
        // Le compteur d'usage : l'heure de cette image, les clics d'icônes de la précédente, l'écriture au repos (`usageui.rs`).
        self.usage_suivre(ctx);
        self.handle_shortcuts(ctx);
        self.sync_scene_layers();
        // Calques de couleur : leurs images suivent celles du trait (`calquecouleur.rs`).
        self.sync_couleurs();
        self.warm_couleur();
        if !self.modal_open() {
            if self.sans_projet() {
                self.deposer_sans_projet(ctx);
            } else {
                self.handle_dropped_files(ctx);
            }
        }
        self.tick_playback(ctx);
        self.tick_flip(ctx);
        self.tick_video_import(ctx);
        self.tick_references(ctx);
        self.tick_personnages();
        self.tick_aplat();
        self.poll_aplat_multi();
        self.tick_aplat_couleur(ctx);
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
            // (Le rendu exact d'une zone bitmap transformée, calculé sur un fil : `pixtools.rs`.)
            self.poll_bitmap_settle();
            self.rejeu_idle_tick(ctx);
            self.warm_current_cel();
            self.tenir_les_dessins(ctx);
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

        // Mode concentration (`concentration.rs`) : ni menus, ni barres, ni panneaux ; le dessin seul, sous une barre
        // minimale. Une couche par-dessus : le dock et les dispositions ne sont pas touchés.
        let concentre = self.concentration_active();
        // Sans projet ouvert (`sansprojet.rs`) : les menus, la barre d'état et l'accueil sur un fond neutre, rien d'autre. La
        // bande des onglets ne paraît que s'il y en a plusieurs (un seul onglet sans projet n'est pas un onglet à montrer).
        let sans_projet = self.sans_projet();
        // L'accueil compact (`accueilfenetre.rs`) : une petite fenêtre qui ne montre que la page d'accueil, sans menus ni barre d'état.
        let compact = self.accueil_compact();
        if !concentre && !compact {
            let _t = crate::perf::section("barre de menus");
            egui::TopBottomPanel::top("menu").show(ctx, |ui| self.menu_bar(ui));
        }
        if !concentre && !(sans_projet && self.onglets.len() <= 1) {
            // La bande des onglets de projets (`ongletsui.rs`), sous les menus.
            let _t = crate::perf::section("onglets");
            egui::TopBottomPanel::top("onglets").show(ctx, |ui| self.onglets_bar(ui));
        }
        if self.layout.command_bar && !concentre && !sans_projet {
            let _t = crate::perf::section("barre de commandes");
            egui::TopBottomPanel::top("commandes").show(ctx, |ui| self.command_bar(ui));
        }
        if !concentre && !compact {
            let _t = crate::perf::section("barre d'état");
            egui::TopBottomPanel::bottom("statut").show(ctx, |ui| self.status_bar(ui));
        }
        if self.layout.tool_bar && !concentre && !sans_projet {
            let _t = crate::perf::section("barre d'outils");
            egui::SidePanel::left("barre_outils")
                .exact_width(40.0)
                .resizable(false)
                .show(ctx, |ui| self.vertical_tool_bar(ui));
        }
        // La barre des panneaux, sur le bord droit : avant les panneaux, qui prennent le reste.
        if self.settings.panel_bar && !concentre && !sans_projet {
            let _t = crate::perf::section("barre des panneaux");
            let frame = egui::Frame::side_top_panel(&ctx.style()).inner_margin(egui::Margin::symmetric(2, 4));
            egui::SidePanel::right("barre_panneaux")
                .exact_width(crate::barrepanneaux::LARGEUR)
                .resizable(false)
                .frame(frame)
                .show(ctx, |ui| self.panel_bar(ui));
        }
        // Les panneaux : onglets que l'on glisse, aimante ou fait flotter (5b).
        if concentre {
            let _t = crate::perf::section("mode concentration");
            self.concentration_ui(ctx);
        } else if sans_projet {
            let _t = crate::perf::section("zone de travail vide");
            // (La page d'accueil ; un fond neutre le temps qu'un projet arrive.)
            if self.accueil_visible() {
                self.accueil_page(ctx);
            } else {
                self.zone_vide(ctx);
            }
        } else {
            let _t = crate::perf::section("panneaux (dock, total)");
            self.show_dock(ctx);
        }

        {
            let _t = crate::perf::section("fenêtres et dialogues");
            self.dialogs(ctx);
            // Conseils d'un seul passage, aide-mémoire des raccourcis (tenu), réglage du stylet (`confort.rs`).
            self.visite_ui(ctx);
            self.conseils_ui(ctx);
            self.aide_touches_tick(ctx);
            self.reglage_stylet_ui(ctx);
            self.top_view_window(ctx);
            self.tick_open(ctx);
            self.tick_reopen();
            self.poll_brushes();
            self.poll_bruitages(ctx);
            self.poll_bibliotheque(ctx);
            self.poll_suggestions(ctx);
            self.tick_collage(ctx);
            self.collage_fenetre(ctx);
            self.tick_save(ctx);
            self.fusion_tick(ctx);
            // Travailler à plusieurs : ce que la session a préparé est posé (`cooperationui.rs`).
            self.cooperation_tick(ctx);
            // Les onglets cachés : une écriture finie est adoptée, rien d'autre ne tourne pour eux.
            self.tick_onglets(ctx);
            self.groupe_fenetre(ctx);
            self.tick_session_ouverture();
            self.envois_tick(ctx);
            self.envois_windows(ctx);
            self.comparer_fenetre(ctx);
            self.purge_versions_fenetre(ctx);
            self.variantes_fenetres(ctx);
            // Le nom d'une séquence qu'on renomme depuis le montage ou la liste du panneau Planche (`planchesequencesui.rs`).
            self.sequence_fenetre(ctx);
            // « Mes habitudes » et la note de friction en cours (`usageui.rs`, `frictionui.rs`).
            self.habitudes_fenetre(ctx);
            self.friction_fenetre(ctx);
            // Le banc du stylet : ce que Windows en dit, à côté de ce que FEANOR reçoit (`styletui.rs`).
            self.stylet_fenetre(ctx);
            // L'anneau sous le stylet (`anneauui.rs`) : avant la fenêtre des réglages, que son clic bref ouvre dans la même image.
            self.anneau_fenetre(ctx);
            // Clic droit dans le dessin : les réglages de l'outil sous le curseur, par-dessus tout (`reglagesdroit.rs`).
            self.reglages_droit_fenetre(ctx);
        }
        // Export en cours : le projet se copie par tranches de quelques millisecondes au départ, puis l'export tourne sur
        // d'autres fils ; ici on ne fait que le regarder, à peu près dix fois par seconde (la barre d'avancement).
        if let Some(job) = &mut self.export_job {
            job.step(&self.doc, std::time::Duration::from_millis(8));
            if job.outcome.is_none() {
                if job.needs_project() {
                    ctx.request_repaint();
                } else {
                    ctx.request_repaint_after(std::time::Duration::from_millis(100));
                }
            }
        }
        self.export_windows(ctx);
        self.recovery_window(ctx);
        self.workspace_name_window(ctx);
        self.personnages_fenetre(ctx);
        self.scrub_audio();
        self.drop_stale_drags(ctx.input(|i| i.pointer.any_down()));
        self.dire_le_refus_des_versions();
        self.suivre_sorte_pinceau();
        self.watch_plan_length();
        if self.thumbs.pending {
            ctx.request_repaint();
        }
        // Réglages de la pelure, de la feuille d'exposition, des flips et des options du pinceau
        // (peint sur l'aplat) : enregistrés quand ils changent (pas pendant un glissé).
        let options = self.paint_behind != self.settings.paint_behind || self.paint_on_aplat != self.settings.paint_on_aplat;
        let changed = self.onion != self.settings.onion || self.xsheet != self.settings.xsheet || self.flips != self.settings.flips || options || self.toucher != self.settings.toucher || self.pinceaux != self.settings.brushes_panel || self.cadrage != self.settings.framing;
        if changed && !ctx.input(|i| i.pointer.any_down()) {
            self.settings.framing = self.cadrage.clone();
            self.settings.onion = self.onion.clone();
            self.settings.xsheet = self.xsheet.clone();
            self.settings.flips = self.flips.clone();
            self.settings.paint_behind = self.paint_behind;
            self.settings.paint_on_aplat = self.paint_on_aplat;
            self.settings.toucher = self.toucher;
            self.settings.brushes_panel = self.pinceaux.clone();
            if let Err(e) = self.settings.save() {
                self.status = format!("Réglages non enregistrés : {e}");
            }
        }
        // Couleurs et nuanciers : enregistrés quand ils changent (passe couleur).
        self.color_tick(ctx);
        // (Un seul atome lu par image : la carte graphique perdue fait partir la copie de secours tout de suite.)
        if crate::plantage::carte_perdue_a_signaler() {
            self.secours_carte_perdue();
        }
        self.tick_recovery(ctx);
        self.tick_session();
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
        // Même règle pour chaque onglet, dont la copie de secours est effacée (`onglets.rs`).
        self.retenir_la_session();
        self.fermer_tous_les_onglets();
        let current = self.current_layout();
        self.settings.set_layout_of(self.espace, current);
        let _ = self.settings.save();
        // Le compte des commandes du poste (`usageui.rs`).
        self.usage_ecrire();
        // Le serveur de génération sonore que SHOT a lancé s'arrête avec lui (un serveur lancé à la main reste).
        self.sug.serveur.arreter();
    }

    /// À la sortie de SHOT, après `fermeture` (les écritures sont finies, les copies de secours fermées, les réglages
    /// écrits) : les documents des onglets, leurs historiques et les dessins dérivés des effets ne sont pas rendus bloc par
    /// bloc — un film de 300 plans en tient des centaines de milliers —, le système reprend la mémoire du processus d'un
    /// coup quand il se termine. Tout le reste se lâche comme avant (exports arrêtés, fils et serveurs prévenus).
    pub(crate) fn sortie_sans_rendre(&mut self) {
        let garder = |doc: &mut Document, history: &mut crate::history::History| {
            std::mem::forget(std::mem::replace(doc, Document::new(TILE, TILE, 24)));
            std::mem::forget(history.take_steps());
        };
        garder(&mut self.doc, &mut self.history);
        for t in &mut self.onglets {
            if let Some(e) = t.etat.as_mut() {
                garder(&mut e.doc, &mut e.history);
                std::mem::forget(e.reserve_fx.take());
            }
        }
        std::mem::forget(crate::fx::forget_all_rendre());
    }
}

