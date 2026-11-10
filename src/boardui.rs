//! Vue Vignettes : l'écran de la planche (la mise en page est dans `board.rs`).
//!
//! Vignettes simples (Anthony, 2026-10-21). Du **papier blanc infini** (plus de carte : on le déplace dans tous les sens), où les
//! images de storyboard se suivent de gauche à droite, rangée par rangée, et où des **cases pâles** (trois rangées au moins, une
//! rangée de plus dès qu'on dessine dans la dernière) montrent où dessiner. Rien autour des images que ce qui sert :
//! un filet gris d'un point (plus foncé pour l'image courante), le numéro du plan au-dessus de sa première
//! image, une courte barre entre deux images (grise : même plan ; noire : coupe), une bande de note dessous.
//!
//! - **On dessine tout de suite** : un trait qui commence dans une image (pleine ou vide) la rend courante
//!   à l'appui et dessine dedans, sans clic de choix avant. Un trait qui commence ailleurs ne dessine rien.
//!   C'est le dessin lui-même qui est dans l'image courante (`card_canvas`), donc le même moteur, le même
//!   rendu progressif : aucune latence de plus. Pendant le geste, le trait **déborde** à l'écran, pâle (`debord.rs`) ; au
//!   lâcher, l'image ne montre que son cadre et une marge de 12 % de sa largeur, de chaque côté, où le trait reste visible (`thumbs::texture_case`). La toile
//!   infinie garde tout le tracé, et l'export cadre comme avant.
//! - **Cases pâles** après la dernière image : dessiner dans l'une d'elles fait le plan (celles qui la précèdent deviennent des plans
//!   vides, pour garder l'ordre ; si la barre qui précède la première est grise, une image de plus du dernier plan) ; la création et
//!   le trait ne font qu'une étape d'annulation.
//! - **Barres** : un clic sur une barre grise coupe le plan à cet endroit (la vignette de droite commence un
//!   nouveau plan) ; sur une barre noire, il réunit les deux plans (la vignette de droite devient une image du
//!   plan de gauche). Une seule étape d'annulation, nommée.
//! - **Notes** : un clic sur l'icône d'écriture au bout de la bande, ou sur **la ligne Notes** quand ce champ est coché, écrit la note du
//!   plan (première image) ou de la marque d'image ; Entrée valide, Maj + Entrée passe à la ligne, Échap abandonne. **Une ligne par champ
//!   coché** (action, dialogue, notes), coupée par « … » ; aucun champ coché : aucune ligne. Le texte entier est gardé, visible pendant
//!   la saisie et dans la fiche du plan.
//! - Molette : défiler ; Ctrl + molette : zoom ; Espace, bouton du milieu ou outil Main : déplacer la planche.
//!   Maj + clic sur une image : choisir des plans (au lieu de dessiner). **Ctrl + Maj + glisser**, ou **T tenue** (`vignettes.camera`),
//!   dans une case : tirer les cadres de sa caméra (`vignettecamui.rs`), jamais un plan. **Ctrl seul + glisser** : le lasso, dans le
//!   dessin de la case, comme partout. Maj + appui sur le **cadre** d'une
//!   image, puis glisser : déplacer son plan entier, entre deux plans (`boardmove.rs`) ; avec Alt, sa copie. Clic droit : menus. Les
//!   notes libres de la planche (texte, gomme, sélection) restent ; on ne les trace plus au pinceau.

use std::collections::HashMap;
use std::time::Instant;

use eframe::egui::{
    self, pos2, text::LayoutJob, vec2, Align2, Color32, CursorIcon, Event, FontId, Key, KeyboardShortcut, Modifiers, PointerButton, Pos2, Rect,
    Sense, Stroke, StrokeKind, Vec2,
};

use crate::app::{App, BoardView, Mode, Tool};
use crate::board::{self, BoardLayout, BoardSettings, CardPlace, Hit, NoteRef, MAX_COLUMNS, NOTE_TEXT_SIZE};
use crate::boardsens::Champ;
use crate::history::Nav;
use crate::vector::VPoint;

/// Le papier : blanc, sans bord, à perte de vue (plus de carte ni de table autour).
const PAPER: Color32 = Color32::WHITE;
const INK: Color32 = Color32::from_gray(30);
/// Filet d'une image (gris moyen, comme la maquette), de l'image courante, d'une case pâle.
const BORDER: Color32 = Color32::from_gray(154);
pub(crate) const BORDER_CURRENT: Color32 = Color32::from_gray(80);
const GHOST: Color32 = Color32::from_gray(228);
const GHOST_HOT: Color32 = Color32::from_gray(150);
/// Barre de continuité (même plan : un trait très pâle) et de coupe (noire).
const BAR_JOINED: Color32 = Color32::from_gray(230);
const BAR_CUT: Color32 = Color32::from_gray(26);
/// Le point entre deux cases d'un même plan (partie 113, proposition B) : visible sur le papier, discret.
pub(crate) const BAR_POINT: Color32 = Color32::from_gray(185);
/// Texte d'une note (gris discret), et le filet très pâle sous chaque bande.
const NOTE_INK: Color32 = Color32::from_gray(100);
const HAIRLINE: Color32 = Color32::from_gray(236);
/// Largeur (en points d'écran) sous laquelle une barre est élargie pour rester facile à toucher.
const BAR_TOUCH: f32 = 14.0;
/// Côté (points d'écran) de l'icône d'écriture au bout d'une bande de note.
const NOTE_ICONE: f32 = 16.0;
/// Marge du texte dans une bande de note, en points de la planche.
const NOTE_PAD: f32 = 1.0;
/// Rayon de la gomme des notes libres : la taille de la gomme, réduite (les notes sont à l'échelle de la planche).
const NOTE_SCALE: f32 = 0.5;
const ZOOM_MIN: f32 = 0.2;
const ZOOM_MAX: f32 = 6.0;

/// Geste en cours sur le fond de la planche.
pub(crate) enum BoardDrag {
    /// Gomme sur les notes ; `recorded` : l'étape d'annulation est prise.
    Erase { recorded: bool },
    /// Gomme partie d'une part de trait posée par-dessus une case (partie 122) : elle n'efface que ces parts-là.
    EraseDessus { recorded: bool },
    /// Rectangle de choix ; `add` : ajouter à ce qui est déjà choisi.
    Rubber { start: Pos2, now: Pos2, add: bool },
    /// Notes choisies glissées ; `last` : le pointeur à l'image précédente (planche).
    MoveNotes { last: Pos2, recorded: bool },
}

/// Texte de note de la planche (outil Texte) en cours d'écriture : nouveau (`index` : aucun) ou existant ;
/// coin haut gauche et taille sur la planche.
pub(crate) struct NoteTextEdit {
    index: Option<usize>,
    pos: Pos2,
    size: f32,
    buffer: String,
    focused: bool,
}

/// Qui tient l'appui en cours : les gestes de la planche et le dessin ne se mélangent pas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Press {
    /// Aucun appui pris : le fond de la planche reçoit le geste (choix au lasso, gomme, notes).
    #[default]
    None,
    /// Le dessin de l'image : le trait va dans la vignette courante.
    Card,
    /// Choix de plans (Ctrl ou Maj + clic).
    Select,
    /// Une barre vient d'être basculée.
    Bar,
    /// Une bande de note vient de s'ouvrir (un appui sur son icône d'écriture : rien à dessiner).
    Note,
    /// Un appui qui ne fait rien (dehors de toute image avec un outil de dessin, case vide refusée…).
    Dead,
    /// Maj + appui sur le cadre d'une vignette : le plan se déplace (`PlanMove`).
    Move,
    /// Le stylo ou la gomme d'annotation tient l'appui (`annotationsui.rs`) : le dessin de la vignette ne reçoit rien.
    Annot,
    /// Un appui sur l'étiquette de version (« V2 », en haut à droite d'une case) : il épingle la pile des versions, rien à dessiner.
    Version,
    /// Ctrl + appui, calque vectoriel : la sélection de la planche (lasso ou geste de sa boîte, partie 125) ; le dessin de la case ne reçoit rien.
    SelPlanche,
}

/// Un plan (ou plusieurs plans choisis) pris par le cadre d'une de ses vignettes (Maj + appui), qu'on glisse ailleurs.
/// Tout le plan bouge d'un bloc : ses vignettes, ses marques d'image, ses dessins, ses clés de caméra, ses notes…
/// (l'opération du montage, `Document::move_plans`). Le repère de pose n'est jamais qu'entre deux plans.
pub(crate) struct PlanMove {
    /// Les plans pris, dans l'ordre du film.
    pub ids: Vec<u64>,
    /// Le pointeur à l'appui et maintenant (écran).
    pub grab: Pos2,
    pub now: Pos2,
    /// Le pointeur a quitté le seuil : le geste est un déplacement (sinon, un simple clic sur le cadre ne fait rien).
    pub active: bool,
    /// Échap : le geste est abandonné, le lâcher ne fait rien.
    pub cancelled: bool,
    /// Où le plan se poserait si l'on lâchait ici.
    pub target: Option<board::DropSlot>,
    /// On glisse la copie du plan ; l'original reste où il est, la copie se pose à l'endroit du lâcher. (Plus aucun geste ne le demande
    /// depuis la partie 113 : Maj + Alt est retiré ; « Dupliquer le plan » reste au clic droit, à l'anneau et à sa touche.)
    pub copie: bool,
    /// L'appui a pris la case `k` avec ces touches : un lâcher sans mouvement est un clic, qui choisit le plan (partie 113, proposition A).
    pub clic: Option<(usize, Modifiers)>,
}

/// Distance (points d'écran) dont il faut glisser un cadre avant que ce soit un déplacement.
pub(crate) const MOVE_THRESHOLD: f32 = 5.0;

/// La ligne de texte en cours d'écriture : le champ `champ` (action, dialogue ou notes) de l'image `id` (le plan, ou sa marque d'image).
pub(crate) struct NoteEdit {
    pub id: u64,
    pub champ: Champ,
    pub buffer: String,
    focused: bool,
    had_focus: bool,
}

/// Un plan (ou une image) fait en dessinant dans une case vide : `base_len` étapes d'annulation juste après
/// la création ; ce qui s'y ajoute avant le lâcher (le trait) se joint à elle.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Created {
    pub base_len: usize,
}

/// Sur quoi le menu du clic droit a été ouvert.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuTarget {
    /// La case `case` (l'identifiant du plan pour sa première case, celui de sa marque d'image sinon) du plan `plan`. Des
    /// identifiants, pas des rangs : le menu reste ouvert pendant un Ctrl+Z ou l'arrivée du montage d'un équipier, qui donnent
    /// le rang du plan à un autre (« Animer cette image » animait le plan voisin, Anthony 2026-10-22). `cadre` : le cadre de
    /// caméra sous le clic droit, s'il y en a un (« Retirer ce cadre »).
    Card { plan: u64, case: u64, cadre: Option<usize> },
    /// La case d'un plan écarté (« Reprendre ce plan »).
    Annule(u64),
    /// Le titre de la séquence `0` (« Renommer la séquence… »).
    Titre(u64),
    Background,
}

/// Passage planche ↔ écran.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Xf {
    pub origin: Pos2,
    pub zoom: f32,
}

impl Xf {
    pub(crate) fn to_screen(self, p: Pos2) -> Pos2 {
        self.origin + p.to_vec2() * self.zoom
    }
    pub(crate) fn rect(self, r: Rect) -> Rect {
        Rect::from_min_max(self.to_screen(r.min), self.to_screen(r.max))
    }
    pub(crate) fn to_board(self, s: Pos2) -> Pos2 {
        ((s - self.origin) / self.zoom).to_pos2()
    }
}

/// L'état de la vue Vignettes qui n'est pas dans le projet.
pub(crate) struct Vignettes {
    pub press: Press,
    pub note: Option<NoteEdit>,
    /// La première case pâle, une fois remplie, fait une image de plus du dernier plan (barre grise devant elle).
    pub empty_joins: bool,
    pub created: Option<Created>,
    pub menu: Option<MenuTarget>,
    /// La planche telle qu'elle est à l'écran à la dernière image : le passage planche ↔ écran et la zone de la vue.
    pub xf: Xf,
    pub view: Rect,
    /// Un plan pris par le cadre d'une de ses vignettes (Maj + appui) et glissé.
    pub moving: Option<PlanMove>,
    /// Maj tenue, le pointeur est sur le cadre d'une vignette : on peut la prendre (curseur « main »).
    pub frame_hot: bool,
    /// Le numéro de chaque plan à l'écran, à l'image d'avant (sa zone, le plan, s'il est écarté) : un clic dessus l'écarte ou le reprend (`planannuleui.rs`).
    pub numeros_zones: Vec<(Rect, u64, bool)>,
    /// Un appui sur un numéro, en attente de son lâcher.
    pub numero_attente: Option<crate::planannuleui::NumeroAttente>,
    /// Maj tenue, sans appui : la peau de la planche (les numéros cachés se montrent, on peut cliquer dessus).
    pub peau_maj: bool,
    /// Le trait qui déborde de la vignette courante, tant que le geste dure (`debord.rs`).
    pub debord: Option<crate::debord::Debord>,
    /// Ctrl + Maj + glisser un cadre (ou T tenue) : le mouvement de caméra en cours (`vignettecamui.rs`).
    pub cam: Option<crate::vignettecamui::CamGeste>,
    /// La zone de la feuille que montre la tuile en train d'être dessinée (`vignettecam::Zone`) : le dessin la prend
    /// pour sa vue, au lieu de la vue à travers la caméra.
    pub zone: Option<crate::vignettecam::Zone>,
    /// L'enveloppe de la tuile en train d'être dessinée, à l'écran (vide : le dessin est dans une case rectangulaire).
    pub enveloppe: Vec<Pos2>,
    /// De combien la planche a glissé (`BoardLayout::decalage`) à la dernière image : la vue compense, la case A reste où elle est.
    pub decalage: Vec2,
    /// La touche `vignettes.camera` (T) est tenue : tout glissé sur une case tire la caméra (lu à chaque image).
    pub camera_tenue: bool,
    /// L'appui (écran) qui vient de tomber sur la bande de note d'une case et qui dessine : le dessin de la case l'accepte, même hors de
    /// son cadre. Effacé à la fin de l'image d'écran.
    pub appui_bande: Option<Pos2>,
    /// Cet appui hors des cases fait un trait qui ira au papier au lâcher (hors du champ de toute case, ou sur une case écartée) :
    /// « Peint sur l'aplat » ne le détourne pas, le papier n'a pas d'aplat (partie 133). Effacé avec `appui_bande`.
    pub appui_papier: bool,
    /// Les annotations : outil, couleur, affichage, marie-louise (`annotationsui.rs`).
    pub annot: crate::annotationsui::Etat,
    /// Les images déposées sur la planche, en préparation sur un fil de fond (`deposer.rs`).
    pub depots: Vec<crate::deposer::Travail>,
    /// Les fenêtres d'export de la planche et du color script (`plancheui.rs`, `colorscriptui.rs`).
    pub planche: crate::plancheui::Etat,
    pub colorscript: crate::colorscriptui::Etat,
    /// La dernière disposition de la planche et ce dont elle dépend (`board::CacheDisposition`) : `board_layout` ne la refait que
    /// si quelque chose a changé.
    pub disposition: std::cell::RefCell<board::CacheDisposition>,
    /// Les étiquettes de version (« V2 ») posées à cette image d'écran sur les cases des plans qui ont plusieurs versions : leur zone
    /// et leur plan. La pile des versions s'y accroche (`pileversui.rs`, la même que celle du montage).
    pub etiquettes: Vec<crate::pileversui::EtiquetteBloc>,
    /// Le prochain cadrage de la planche est celui d'ouverture (2 rangées de 6 cases) : vrai à l'ouverture et à la création d'un projet
    /// (App::load_doc), jamais quand on revient sur l'onglet.
    pub ouverture_a_faire: bool,
    /// La barre du bas de la planche, à l'écran (`vignettesprefs::planche_barre_basse`).
    pub barre_basse: Rect,
    /// Le geste qui dure dans la case vive, et ce qu'il laisse à son lâcher (horschamp.rs) ; le trait qui déborde, gardé jusqu'à ce que
    /// le trait soit réparti (pas de trou d'une image d'écran au lâcher).
    pub suivi: Option<crate::horschamp::Suivi>,
    pub fini: Option<crate::horschamp::Fini>,
    /// Les cases dont le refus a été dit pendant le trait en cours (`horschamp_prevenir`) : une fois par case.
    pub refus_dits: Vec<u64>,
    /// La dernière phrase dite pendant le trait : elle part s'il s'arrête sans rien poser (`refus_oublier`).
    pub refus_phrase: Option<String>,
    pub debord_garde: Option<crate::debord::Debord>,
    pub debord_tenu: Option<crate::debord::Tenu>,
    /// Le geste de gomme de la planche en cours : ce qu'il efface de ce que les cases ont dessiné hors de leur cadre (`gommedebord.rs`, partie 131).
    pub gomme_debord: Option<crate::gommedebord::GommeDebord>,
    /// Le cadrage d'ouverture : la taille de la vue pour laquelle il a été calculé, son zoom, son décalage (une fois rangé), et s'il vient d'être
    /// calculé. Vide dès que le zoom ou le défilement ont bougé.
    pub ouverture_suivi: Option<(Vec2, f32, Vec2, bool)>,
    /// Les images d'écran qu'il reste au cadrage d'ouverture pour suivre la vue.
    pub ouverture_images: u8,
    /// Le titre de séquence en cours de renommage, et la séquence qu'on glisse dans la liste du panneau (`planchesequencesui.rs`).
    pub titre_edit: Option<crate::planchesequencesui::TitreEdit>,
    pub seq_glisse: Option<u64>,
    /// Les textures du dessin hors des cases, à la carte graphique (`horschamptex.rs`).
    pub hc_tex: std::cell::RefCell<crate::horschamptex::Textures>,
    /// Aucun plan n'est choisi sur la planche (Anthony, 2026-11-05) : pas de cadre noir de sélection, et le dessin n'est lié à aucun plan, il
    /// part d'où l'on veut et se répartit au lâcher (horschamp.rs). Vrai à l'ouverture, après un trait, par Échap et par un clic sur le
    /// papier ; faux seulement après un geste voulu : Maj ou Ctrl + clic, le lasso, un clic avec l'outil Caméra. Le plan courant, lui,
    /// reste valide pour les panneaux (Calques…).
    pub libre: bool,
    /// Le plan dont la caméra est choisie (l'outil Caméra des Vignettes : un clic sur une case) ; ses cadres restent soulignés.
    pub camera_choisie: Option<u64>,
    /// Une commande vient de choisir le plan courant (plan suivant, nouveau plan… ; partie 113) : à l'image suivante, la planche défile
    /// jusqu'à sa case si elle n'est pas dans la vue.
    pub montrer_courante: bool,
    /// « Cadrer tout » vient d'être demandé (partie 113) : le prochain zoom nul met toute la planche dans la vue, en hauteur aussi.
    pub cadrer_tout: bool,
    /// Un appui est tombé sur une barre (partie 113, décision 11) : la barre ne répond qu'au lâcher d'un clic qui n'a pas bougé ; un appui
    /// qui bouge est un trait, parti dès son premier point.
    pub barre_attente: Option<BarreAttente>,
    /// Ce que Maj permet sur la planche a été dit une fois dans la ligne d'état (partie 113, proposition A).
    pub maj_dit: bool,
    /// Le « + » de la peau de Maj : un cadre de caméra de plus (partie 116, `cadreplus.rs`).
    pub plus: crate::cadreplus::Etat,
    /// La sélection de la planche : des traits entiers, choisis au lasso de Ctrl, dans les cases comme sur le papier (partie 125, `vignettesel.rs`).
    pub selp: Option<crate::vignettesel::SelPlanche>,
    /// Les textures de la couche bitmap de la planche (`planchebitmap.rs`, partie 127).
    pub pb_tex: std::cell::RefCell<crate::planchebitmap::Textures>,
    /// La sélection de la planche sur un calque bitmap (partie 127, `vignetteselbmp.rs`).
    pub selb: Option<crate::vignetteselbmp::SelBitmap>,
    /// Les textures de son aperçu.
    pub selb_tex: std::cell::RefCell<crate::planchebitmap::Textures>,
    /// Ce que Ctrl+C a pris de la sélection bitmap de la planche (`vignetteselbmp.rs`) : Ctrl+V le repose à sa place, réparti.
    pub selb_presse: Option<Vec<crate::vignetteselbmp::Piece>>,
}

/// Un appui sur une barre, en attente de son lâcher (partie 113, décision 11).
#[derive(Clone, Copy, Debug)]
pub(crate) struct BarreAttente {
    /// La barre : après la case `after`.
    pub after: usize,
    /// Le point d'appui (écran).
    pub origine: Pos2,
    /// Le nombre d'étapes d'annulation avant l'appui.
    pub base_len: usize,
    /// L'appui a aussi lancé un trait dans la case la plus proche (un outil de dessin) : au lâcher d'un clic, ce point de trait est retiré.
    pub dessine: bool,
}

impl Vignettes {
    /// L'état d'une séance qui commence, le passe-partout des cases allumé ou non (retenu dans les réglages).
    pub(crate) fn avec_passe_partout(on: bool) -> Vignettes {
        let mut v = Vignettes::default();
        v.annot.marie = on;
        v
    }
}

impl Default for Vignettes {
    fn default() -> Self {
        Vignettes {
            press: Press::None,
            note: None,
            empty_joins: false,
            created: None,
            menu: None,
            xf: Xf::default(),
            view: Rect::NOTHING,
            moving: None,
            frame_hot: false,
            numeros_zones: Vec::new(),
            numero_attente: None,
            peau_maj: false,
            debord: None,
            cam: None,
            zone: None,
            enveloppe: Vec::new(),
            decalage: Vec2::ZERO,
            camera_tenue: false,
            appui_bande: None,
            appui_papier: false,
            annot: Default::default(),
            depots: Vec::new(),
            planche: Default::default(),
            colorscript: Default::default(),
            disposition: Default::default(),
            etiquettes: Vec::new(),
            ouverture_a_faire: true,
            barre_basse: Rect::NOTHING,
            suivi: None,
            fini: None,
            refus_dits: Vec::new(),
            refus_phrase: None,
            debord_garde: None,
            debord_tenu: None,
            gomme_debord: None,
            selp: None,
            ouverture_suivi: None,
            ouverture_images: 0,
            titre_edit: None,
            seq_glisse: None,
            hc_tex: Default::default(),
            pb_tex: Default::default(),
            selb: None,
            selb_tex: Default::default(),
            selb_presse: None,
            libre: true,
            camera_choisie: None,
            montrer_courante: false,
            cadrer_tout: false,
            barre_attente: None,
            maj_dit: false,
            plus: Default::default(),
        }
    }
}

/// Le texte d'un champ du plan (l'action, le dialogue, les notes), mis en page : `size` points, `width` de large, **une seule ligne**, coupée
/// par « … » si le texte est plus long (Anthony, 2026-10-24). `etiquette` : plusieurs champs sont montrés, le texte est précédé de l'étiquette
/// colorée de son champ (« Action », « Dialogue », « Note »), un peu plus petite ; un champ seul n'en a pas, comme avant le panneau
/// « Vignettes ».
fn champ_job(champ: Champ, text: &str, size: f32, width: f32, encre: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = width.max(1.0);
    // (L'intitulé est toujours là : une ligne vide se reconnaît à lui.)
    job.append(&format!("{}  ", champ.etiquette()), 0.0, egui::TextFormat::simple(FontId::proportional((size * 0.82).max(1.0)), encre));
    job.append(text, 0.0, egui::TextFormat::simple(FontId::proportional(size.max(1.0)), NOTE_INK));
    job.wrap.max_rows = 1;
    job.wrap.overflow_character = Some('…');
    job
}

impl App {
    /// La planche est-elle à l'écran (storyboard, vue Vignettes) ?
    pub(crate) fn board_shown(&self) -> bool {
        self.mode == Mode::Storyboard && self.board_view == BoardView::Panels
    }

    /// La disposition de la planche, avec ses réglages et les champs de texte cochés (une ligne chacun) : sur le papier infini, une seule
    /// page, les cases pâles qui montrent où dessiner, et la place que prend le geste de caméra en cours (`board::Reserve`).
    pub(crate) fn board_layout(&self) -> BoardLayout {
        let s = BoardSettings { rows: 0, ..self.layout.board_settings() };
        let hold = self.vignettes.cam.as_ref().filter(|g| g.actif && !g.annule).and_then(|g| g.hold);
        let opt = board::Options { hold, sens: self.layout.board_sens, textes: self.layout.board_textes, ..board::Options::pale(self.layout.board_min_rows) };
        // (Gardée tant que rien de ce dont elle dépend ne change : `board::CacheDisposition`. Un essai de performance mesure aussi
        // l'ancien chemin, refaite à chaque appel : `perf::ancien`.)
        // (Plusieurs séquences : la planche par séquences, `planchesequences.rs` ; une seule : la planche d'avant, sans titre.)
        let vue = crate::planchesequences::vue(&self.doc);
        if crate::perf::ancien() {
            return match &vue {
                Some(v) => crate::planchesequences::layout_sequences(&self.doc, &s, self.vignettes.empty_joins, &opt, v),
                None => board::layout_with(&self.doc, &s, &HashMap::new(), self.vignettes.empty_joins, &opt),
            };
        }
        let mut cache = self.vignettes.disposition.borrow_mut();
        board::layout_cache_sequences(&self.doc, &s, &HashMap::new(), self.vignettes.empty_joins, &opt, vue.as_ref(), &mut cache)
    }

    /// La planche a glissé (`BoardLayout::decalage` : une enveloppe de caméra dépasse la page à gauche ou en haut) : la vue défile
    /// d'autant en sens inverse, et les cases, A comprise, restent où elles sont à l'écran.
    pub(crate) fn suivre_decalage(&mut self, layout: &BoardLayout) {
        let d = layout.decalage - self.vignettes.decalage;
        self.vignettes.decalage = layout.decalage;
        if d != Vec2::ZERO && self.board_zoom > 0.0 {
            self.board_pan -= d * self.board_zoom;
        }
    }

    /// Après une opération qui a retiré des panneaux : les notes qui leur étaient attachées
    /// restent où elles étaient (`before` : la planche d'avant), libres.
    pub(crate) fn release_removed_notes(&mut self, before: &BoardLayout) {
        // (Un plan écarté n'est pas parti : ses notes l'attendent, celles de ses cases de continuité aussi, `planannule.rs`. Et une case d'un plan du
        // film qui est toujours là ne perd pas les siennes.)
        let vivants: std::collections::HashSet<u64> = self.doc.panels.iter().chain(self.doc.annules.plans.iter().map(|g| &g.panel)).flat_map(|p| std::iter::once(p.id).chain(p.marks.iter().map(|m| m.id))).collect();
        let gone: Vec<u64> = before.cards.iter().map(|c| c.id).filter(|id| !vivants.contains(id)).collect();
        if !gone.is_empty() {
            self.doc.board.release(before, &gone);
        }
        // Les annotations d'une vignette qui n'est plus là passent à sa voisine (`annotations.rs`).
        self.annot_rattacher_apres(before);
    }

    /// La page à la largeur de la vue, en haut de la planche (fait à l'image suivante, quand la
    /// vue est connue : un zoom nul le demande).
    pub(crate) fn board_fit(&mut self) {
        self.board_zoom = 0.0;
        self.board_pan = Vec2::ZERO;
    }

    /// « Cadrer tout » (barre du bas) : toute la planche dans la vue, en largeur **et** en hauteur (partie 113). Avant, le bouton ne
    /// mettait la page qu'à la largeur de la vue : sur 300 plans, il montrait les sept premières rangées en disant « Toute la page dans la
    /// vue ». Le zoom ne descend pas sous `ZOOM_MIN` : au-delà, la planche reste à cadrer à la main.
    pub(crate) fn planche_cadrer_tout(&mut self) {
        self.board_fit();
        self.vignettes.cadrer_tout = true;
    }

    /// Zoom par cran (raccourcis), autour du coin haut gauche de la vue.
    pub(crate) fn board_zoom_step(&mut self, factor: f32) {
        if self.board_zoom <= 0.0 {
            self.board_zoom = 1.0;
        }
        let z = (self.board_zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
        self.board_pan *= z / self.board_zoom;
        self.board_zoom = z;
    }

    fn board_zoom_at(&mut self, view: Rect, p: Pos2, factor: f32) {
        let b = (p - view.min - self.board_pan) / self.board_zoom;
        let z = (self.board_zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
        self.board_pan = p - view.min - b * z;
        self.board_zoom = z;
    }

    /// La case `k` de `layout` (faite en début d'image, quand le film avait `plans` plans) désigne-t-elle toujours le même plan ?
    /// Un menu peint au milieu des cases change parfois le film : les rangs d'avant ne désignent plus les mêmes plans (ou plus
    /// aucun), et le reste de l'image ne doit pas les lire. (Un rang et un identifiant par case : rien de lourd par image.)
    pub(crate) fn case_tient(&self, layout: &BoardLayout, plans: usize, k: usize) -> bool {
        self.doc.panels.len() == plans && layout.cards.get(k).is_none_or(|c| self.doc.panels.get(c.panel).is_some_and(|p| p.id == c.plan))
    }

    /// Le rang du plan `plan` et l'image où commence sa case `case` (le plan lui-même pour sa première case, sa marque d'image
    /// sinon), dans le film d'aujourd'hui ; rien si la case n'y est plus.
    pub(crate) fn case_du_menu(&self, plan: u64, case: u64) -> Option<(usize, usize)> {
        let i = self.doc.panels.iter().position(|p| p.id == plan)?;
        let p = &self.doc.panels[i];
        let frame = if case == plan { 0 } else { p.marks.iter().find(|m| m.id == case)?.frame };
        Some((i, frame))
    }

    /// Les panneaux choisis, dans l'ordre du film (le panneau courant en fait toujours partie).
    pub(crate) fn board_selection(&self) -> Vec<u64> {
        let cur = self.doc.panels[self.cur_panel].id;
        self.doc.panels.iter().map(|p| p.id).filter(|id| *id == cur || self.board_sel.contains(id)).collect()
    }

    /// Rend courante l'image de storyboard qui commence à l'image `frame` du plan `i`.
    pub(crate) fn select_board(&mut self, i: usize, frame: usize) {
        self.select_panel(i);
        self.cur_frame = frame;
        self.follow_frame = true;
    }

    /// Retire les plans `ids` (une étape d'annulation).
    pub(crate) fn delete_panels(&mut self, ids: &[u64]) {
        if self.storyboard_refuses() || !self.partage_peut_toucher(ids) {
            return;
        }
        self.end_stroke();
        let snap = self.snapshot();
        let before = self.board_layout();
        // Ce que la suppression emporte, dit après coup : les dessins des plans (et ce que leur rognage garde en réserve).
        let dessins = {
            let mut vus = std::collections::BTreeSet::new();
            for p in self.doc.panels.iter().filter(|p| ids.contains(&p.id)) {
                vus.extend(p.layers.iter().filter(|l| l.scene_wide.is_none()).flat_map(|l| l.frames.iter().flatten().copied()));
                vus.extend(self.doc.reserves.get(&p.id).into_iter().flat_map(|r| r.cels()));
            }
            vus.len()
        };
        let en_corbeille = self.doc.corbeille.len();
        match self.doc.remove_panels(ids) {
            Some(i) => {
                self.release_removed_notes(&before);
                // Le plan courant d'abord : `commit_structure` lit ses calques, et l'ancien rang peut ne plus
                // exister (supprimer le dernier plan plantait, Anthony 2026-10-22).
                self.cur_panel = i;
                self.clamp_nav();
                self.commit_structure(snap);
                self.board_sel.clear();
                self.thumbs.prune(&self.doc);
                // Les plans supprimés vont à la corbeille (`corbeille.rs`) : la phrase le dit, avec ce qui part.
                let envoyes = self.doc.corbeille.len().saturating_sub(en_corbeille);
                let (n, d) = (ids.len(), dessins);
                let avec = if d == 0 { String::new() } else { format!(" ({})", crate::rognage::dessins_texte(d)) };
                self.status = match (n, envoyes > 0) {
                    (1, true) => format!("Plan envoyé à la corbeille{avec}."),
                    (n, true) => format!("{n} plans envoyés à la corbeille{avec}."),
                    (1, false) => "Plan supprimé.".into(),
                    (n, false) => format!("{n} plans supprimés."),
                };
            }
            None => self.status = "Il faut garder au moins un plan.".into(),
        }
    }

    /// Retire les notes choisies (Suppr).
    pub(crate) fn delete_board_notes(&mut self) {
        if self.board_notes.is_empty() {
            return;
        }
        self.record_step();
        let sel = std::mem::take(&mut self.board_notes);
        self.doc.board.remove(&sel);
        self.status = "Notes supprimées.".into();
    }

    /// Ce que couvre chaque texte de note, sur la planche (pour les choisir et les toucher).
    fn text_rects(&self, painter: &egui::Painter, layout: &BoardLayout) -> Vec<Option<Rect>> {
        (0..self.doc.board.texts.len())
            .map(|i| {
                let (pos, size) = self.doc.board.text_place(layout, i)?;
                let t = &self.doc.board.texts[i];
                let shown = if t.text.is_empty() { " " } else { t.text.as_str() };
                let galley = painter.layout_no_wrap(shown.to_string(), FontId::proportional(size.max(1.0)), INK);
                Some(Rect::from_min_size(pos, galley.size()))
            })
            .collect()
    }

    /// La note sous le point `p` (planche) : un texte d'abord, sinon un trait.
    fn note_at(&self, layout: &BoardLayout, texts: &[Option<Rect>], p: Pos2, tol: f32) -> Option<NoteRef> {
        if let Some(i) = texts.iter().rposition(|r| r.is_some_and(|r| r.expand(tol).contains(p))) {
            return Some(NoteRef::Text(i));
        }
        self.doc.board.stroke_at(layout, p, tol).map(NoteRef::Stroke)
    }

    /// Ce que couvre une note, sur la planche.
    fn note_bounds(&self, layout: &BoardLayout, texts: &[Option<Rect>], n: NoteRef) -> Option<Rect> {
        match n {
            NoteRef::Stroke(i) => self.doc.board.stroke_points(layout, i).map(|p| board::points_bounds(&p)),
            NoteRef::Text(i) => texts.get(i).copied().flatten(),
        }
    }

    // ------------------------------------------------------------------ notes sous les images

    /// Le texte de la bande de note de l'image `place` : les notes du plan (première image) ou de sa marque.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn card_notes(&self, place: &CardPlace) -> &str {
        // (Une case d'une planche faite avant un changement du film peut désigner un plan qui n'est plus là : rien.)
        let Some(p) = self.doc.panels.get(place.panel) else { return "" };
        if place.frame == 0 {
            &p.notes
        } else {
            p.marks.iter().find(|m| m.id == place.id).map_or("", |m| m.notes.as_str())
        }
    }

    /// L'icône d'écriture de la case `place`, à l'écran : tout au bout (à droite) de sa bande de texte. Elle écrit toujours le champ Notes ;
    /// la ligne Notes (quand ce champ est coché) l'ouvre aussi, `ligne_notes`. Le reste de la bande dessine dans la case.
    pub(crate) fn note_icone_rect(&self, place: &CardPlace) -> Rect {
        let strip = self.vignettes.xf.rect(place.note);
        Rect::from_min_size(pos2(strip.right() - NOTE_ICONE - 1.0, strip.top()), vec2(NOTE_ICONE, NOTE_ICONE))
    }

    /// La ligne du champ `champ` de la case `place`, sur la planche, quand ce champ est coché : un clic dessus l'écrit en place (Action et
    /// Dialogue depuis la partie 113, proposition C : raconter sans quitter la planche).
    pub(crate) fn ligne_du_champ(&self, place: &CardPlace, champ: Champ) -> Option<Rect> {
        let i = self.layout.board_textes.ligne_du(champ)?;
        place.a_du_texte().then(|| place.ligne_de_texte(i))
    }

    /// Le champ dont la ligne, sous la case `place`, est sous le point `p` (planche).
    pub(crate) fn champ_sous(&self, place: &CardPlace, p: Pos2) -> Option<Champ> {
        Champ::TOUS.into_iter().find(|c| self.ligne_du_champ(place, *c).is_some_and(|r| r.contains(p)))
    }

    /// Le texte du champ `champ` de la case `place` (le plan, ou sa marque d'image).
    fn texte_de_la_case(&self, place: &CardPlace, champ: Champ) -> &str {
        crate::boardsens::texte_du_champ(&self.doc, place.panel, place.id, place.frame, champ)
    }

    /// L'icône d'écriture, discrète : seulement quand le pointeur est sur la case ou sa bande (pas pendant un geste), et toujours sur la case dont
    /// la note s'écrit. Une bulle d'une seconde dit ce qu'elle fait.
    fn paint_note_icone(&self, ui: &mut egui::Ui, painter: &egui::Painter, view: Rect, place: &CardPlace) {
        // (Aucun champ coché : pas de zone de texte, pas d'icône.)
        let icone = self.note_icone_rect(place);
        if !place.a_du_texte() || !icone.intersects(view) || self.vignettes.note.as_ref().is_some_and(|e| e.id == place.id) {
            return;
        }
        let xf = self.vignettes.xf;
        let zone = xf.rect(place.thumb).union(xf.rect(place.note)).expand(3.0).union(icone);
        let (hover, down) = ui.input(|i| (i.pointer.hover_pos(), i.pointer.any_down()));
        if down || !hover.is_some_and(|p| zone.contains(p) && view.contains(p)) {
            return;
        }
        let dessus = hover.is_some_and(|p| icone.contains(p));
        let accent = crate::theme::accent_texte();
        let clip = painter.with_clip_rect(view);
        if dessus {
            clip.rect_filled(icone, 3.0, accent.gamma_multiply(0.18));
        }
        clip.text(icone.center(), Align2::CENTER_CENTER, egui_phosphor::regular::NOTE_PENCIL, FontId::proportional(13.0), if dessus { accent } else { NOTE_INK });
        let r = ui.interact(icone, egui::Id::new(("icone_note", place.id)), Sense::hover());
        crate::bulles::bulle(r, "Écrire la note", "Écrire la note de cette case. Échap, ou un clic ailleurs, la ferme.", &self.raccourci("vignettes.note"));
    }

    /// « Écrire la note » au clavier : ouvre la note de la case courante (la commande `vignettes.note`).
    pub(crate) fn note_ecrire_courante(&mut self) {
        if !self.board_shown() {
            self.status = "Les notes de cases s'écrivent dans les Vignettes.".into();
            return;
        }
        if self.layout.board_textes.lignes() == 0 {
            self.status = "Coche un champ (Notes) dans le panneau Planche : sans champ coché, la planche n'a pas de ligne de texte.".into();
            return;
        }
        let layout = self.board_layout();
        let f = self.cur_frame.min(self.frame_count().saturating_sub(1));
        let Some(c) = layout.cards.iter().find(|c| c.panel == self.cur_panel && (c.frame..c.end).contains(&f)).copied() else { return };
        self.commit_card_note();
        self.open_card_note(&c);
    }

    /// Ouvre la bande de note de l'image `place` pour l'écrire.
    fn open_card_note(&mut self, place: &CardPlace) {
        self.open_card_champ(place, Champ::Notes);
    }

    /// Ouvre la ligne du champ `champ` de l'image `place` pour l'écrire en place (partie 113, proposition C).
    fn open_card_champ(&mut self, place: &CardPlace, champ: Champ) {
        if self.contenu_verrouille() {
            return;
        }
        let buffer = self.texte_de_la_case(place, champ).to_string();
        self.vignettes.note = Some(NoteEdit { id: place.id, champ, buffer, focused: false, had_focus: false });
    }

    /// Pose le texte en cours d'écriture dans le plan (ou la marque d'image) : son champ (action, dialogue, notes). Une étape
    /// d'annulation, nommée, seulement si le texte a changé.
    pub(crate) fn commit_card_note(&mut self) {
        let Some(ed) = self.vignettes.note.take() else { return };
        let text = ed.buffer.trim_end().to_string();
        let Some(pi) = self.doc.panels.iter().position(|p| p.id == ed.id || p.marks.iter().any(|m| m.id == ed.id)) else { return };
        let on_plan = self.doc.panels[pi].id == ed.id;
        let champ = ed.champ;
        let lire = |p: &crate::document::Panel| -> String {
            if on_plan {
                match champ {
                    Champ::Action => p.action.clone(),
                    Champ::Dialogue => p.dialogue.clone(),
                    Champ::Notes => p.notes.clone(),
                }
            } else {
                p.marks.iter().find(|m| m.id == ed.id).map_or(String::new(), |m| match champ {
                    Champ::Action => m.action.clone(),
                    Champ::Dialogue => m.dialogue.clone(),
                    Champ::Notes => m.notes.clone(),
                })
            }
        };
        let now = lire(&self.doc.panels[pi]);
        if now == text || (now.trim().is_empty() && text.trim().is_empty()) {
            return;
        }
        if self.contenu_verrouille() {
            return;
        }
        let nom = match champ {
            Champ::Action => "Action de vignette",
            Champ::Dialogue => "Dialogue de vignette",
            Champ::Notes => "Note de vignette",
        };
        self.avec_etape(nom, |a| a.record_step());
        let ecrire = |s: &mut String| *s = text.clone();
        let p = &mut self.doc.panels[pi];
        if on_plan {
            match champ {
                Champ::Action => ecrire(&mut p.action),
                Champ::Dialogue => ecrire(&mut p.dialogue),
                Champ::Notes => ecrire(&mut p.notes),
            }
        } else if let Some(m) = p.marks.iter_mut().find(|m| m.id == ed.id) {
            match champ {
                Champ::Action => ecrire(&mut m.action),
                Champ::Dialogue => ecrire(&mut m.dialogue),
                Champ::Notes => ecrire(&mut m.notes),
            }
        }
        self.status = format!("{} de la vignette enregistré{}.", champ.nom(), if champ == Champ::Notes { "es" } else { "" });
    }

    /// Exécute `f` en nommant les étapes d'annulation qu'elle enregistre.
    pub(crate) fn avec_etape<R>(&mut self, nom: &str, f: impl FnOnce(&mut Self) -> R) -> R {
        let avant = self.history.context.replace(nom.to_string());
        let r = f(self);
        self.history.context = avant;
        r
    }

    /// La bande de note en cours d'écriture, au-dessus de la planche, à la place de la bande.
    fn card_note_editor(&mut self, ui: &mut egui::Ui, xf: Xf, layout: &BoardLayout) {
        let Some(id) = self.vignettes.note.as_ref().map(|e| e.id) else { return };
        let Some(place) = layout.card_of(id).copied() else {
            self.vignettes.note = None;
            return;
        };
        // L'éditeur remplace la ligne du champ qu'on écrit (la bande entière si ce champ n'est pas coché : l'icône écrit quand même les notes).
        let champ = self.vignettes.note.as_ref().map_or(Champ::Notes, |e| e.champ);
        let strip = xf.rect(self.ligne_du_champ(&place, champ).unwrap_or(place.note));
        let icone = self.note_icone_rect(&place);
        let Some(ed) = self.vignettes.note.as_mut() else { return };
        let font = FontId::proportional((board::note_font_size(ed.buffer.chars().count()) * xf.zoom).clamp(9.0, 40.0));
        let accent = crate::theme::accent_texte();
        let mut outcome = None;
        egui::Area::new(egui::Id::new("vignette_note")).order(egui::Order::Foreground).fixed_pos(strip.min - vec2(2.0, 1.0)).show(ui.ctx(), |ui| {
            egui::Frame::new().fill(Color32::WHITE).stroke(Stroke::new(1.0_f32, accent)).inner_margin(egui::Margin::symmetric(2, 1)).show(ui, |ui| {
                let resp = ui.add(
                    egui::TextEdit::multiline(&mut ed.buffer)
                        .font(font)
                        .text_color(NOTE_INK)
                        .frame(false)
                        .desired_rows(1)
                        .desired_width((strip.width() - 4.0).max(60.0))
                        .return_key(Some(KeyboardShortcut::new(Modifiers::SHIFT, Key::Enter))),
                );
                if !ed.focused {
                    resp.request_focus();
                    ed.focused = true;
                }
                // L'icône reste sur la case dont la note s'écrit.
                ui.painter().text(icone.center(), Align2::CENTER_CENTER, egui_phosphor::regular::NOTE_PENCIL, FontId::proportional(13.0), accent);
                if resp.has_focus() {
                    ed.had_focus = true;
                }
                let (enter, esc) = ui.input(|i| (i.key_pressed(Key::Enter) && !i.modifiers.shift, i.key_pressed(Key::Escape)));
                if esc {
                    outcome = Some(false);
                } else if enter || (ed.had_focus && resp.lost_focus()) {
                    outcome = Some(true);
                }
            });
        });
        match outcome {
            Some(true) => self.commit_card_note(),
            Some(false) => self.vignettes.note = None,
            None => {}
        }
    }

    // ------------------------------------------------------------------ appuis sur la planche

    /// Les outils qui font un plan quand on dessine dans une case vide.
    pub(crate) fn tool_draws_in_empty(&self) -> bool {
        matches!(self.tool, Tool::Brush | Tool::Pencil | Tool::Line | Tool::Rect | Tool::Ellipse)
    }

    /// Le dessin a la main (copier, coller, supprimer visent le dessin, pas les plans) et un outil pris par une touche
    /// tenue sert : ce que fait `canvas_panel` quand l'appui lui vient de l'interface, ici l'appui est vu plus tôt.
    pub(crate) fn draw_has_focus(&mut self) {
        self.panel_focus = false;
        self.images_focus = false;
        self.key_focus = false;
        self.spring_used();
    }

    /// Un appui du bouton gauche (ou droit) en `pos` (écran) sur la planche. Rend vrai si le document a
    /// changé de forme (plans, images) : la mise en page est à refaire.
    fn board_press(&mut self, layout: &BoardLayout, xf: Xf, pos: Pos2, m: Modifiers, button: PointerButton) -> bool {
        let p = xf.to_board(pos);
        let hit = board::hit(layout, p, BAR_TOUCH / xf.zoom.max(1e-3));
        if button == PointerButton::Secondary {
            if let Some(g) = layout.annulee_en(p).and_then(|k| layout.annulees.get(k)) {
                self.vignettes.menu = Some(MenuTarget::Annule(g.plan));
                return false;
            }
            self.vignettes.menu = Some(match hit {
                Hit::Thumb(k) => {
                    let c = layout.cards[k];
                    let chosen = c.panel == self.cur_panel || self.board_sel.contains(&c.plan);
                    if !chosen {
                        self.board_sel = vec![c.plan];
                        self.select_board(c.panel, c.frame);
                    }
                    // Le cadre de caméra sous le clic (« Retirer ce cadre »).
                    let (quads, simple) = self.cadres_sur_la_planche(layout, k);
                    let cadre = if simple { None } else { crate::vignettecam::cadre_sous(&quads, p) };
                    MenuTarget::Card { plan: c.plan, case: c.id, cadre }
                }
                _ => match layout.titre_en(p) {
                    Some(g) => MenuTarget::Titre(layout.groupes[g].sequence),
                    None => MenuTarget::Background,
                },
            });
            return false;
        }
        if button != PointerButton::Primary {
            return false;
        }
        // Le « + » de Maj (partie 116) passe avant tout : il n'est là que tenu par Maj, T ou l'outil Caméra, et jamais pendant un appui.
        if self.plus_appui(layout, pos) {
            return false;
        }
        // L'étiquette de version (« V2 ») d'une case : un appui dessus épingle la pile des versions (pour un stylet qui ne survole pas), comme
        // sur le bloc du montage ; il ne dessine rien dans la case.
        if let Some(plan) = self.pile_versions.etiquettes.iter().find(|(z, _)| z.contains(pos)).map(|(_, p)| *p) {
            self.commit_note_text(layout);
            self.commit_card_note();
            self.pile_epingler(plan);
            self.vignettes.press = Press::Version;
            return false;
        }
        // La gomme sur une part de trait restée par-dessus une case (partie 122) : ce geste n'efface que ces parts-là, ce qu'on voit au-dessus ; le geste suivant, au
        // même endroit, efface le dessin du plan.
        if self.tool == Tool::Eraser && !m.shift && !m.ctrl && !m.command && self.doc.annotations.touche_dessus(layout, p, self.note_eraser_radius(xf.zoom)) {
            self.commit_note_text(layout);
            self.commit_card_note();
            self.board_drag = Some(BoardDrag::EraseDessus { recorded: false });
            self.vignettes.press = Press::Dead;
            return false;
        }
        // Le numéro d'un plan (annuler, rétablir : au lâcher d'un clic), et une case annulée (on n'y dessine pas) : `planannuleui.rs`.
        if self.plan_annule_appui(layout, pos, p, m) {
            return false;
        }
        // L'icône d'écriture, au bout de la bande de note d'une case : seul un appui dessus ouvre la note (Anthony, 2026-10-22 : « les notes ne
        // doivent s'activer qu'en cliquant sur un bouton, sinon ça empêche parfois de dessiner »). Elle prime sur tout ce qu'il y a dessous.
        // La ligne Notes de la case, quand ce champ est coché, ouvre la note de même (Anthony, 2026-10-24 : « un clic sur la ligne Notes
        // l'écrit ») ; sans modificateur ni touche de caméra tenue : Maj, Ctrl et T gardent leur geste. Les lignes Action et Dialogue, elles,
        // dessinent dans la case comme le reste de la bande.
        // (Depuis la partie 113, proposition C, les lignes Action et Dialogue s'écrivent en place de même.)
        let simple = !m.shift && !m.ctrl && !m.command && !m.alt && !self.vignettes.camera_tenue;
        let ouvre = |c: &CardPlace| -> Option<Champ> {
            if !c.a_du_texte() {
                return None;
            }
            if self.note_icone_rect(c).contains(pos) {
                return Some(Champ::Notes);
            }
            if simple {
                self.champ_sous(c, p)
            } else {
                None
            }
        };
        if let Some((k, champ)) = layout.cards.iter().enumerate().find_map(|(k, c)| ouvre(c).map(|ch| (k, ch))) {
            self.commit_note_text(layout);
            self.commit_card_note();
            let c = layout.cards[k];
            self.board_sel = vec![c.plan];
            self.board_notes.clear();
            self.select_board(c.panel, c.frame);
            self.open_card_champ(&c, champ);
            self.vignettes.press = Press::Note;
            return false;
        }
        // Tout appui ailleurs que dans la bande en cours d'écriture la pose (la bande n'est écrite que par son éditeur, au-dessus de la planche).
        self.commit_card_note();
        // Aucun plan n'est choisi d'office : dessiner n'en choisit pas (le plan sous le trait n'est courant que pour les panneaux). Seul l'outil
        // Caméra, Maj ou Ctrl + clic et le lasso en choisissent un, volontairement.
        let outil_camera = self.tool == Tool::Camera;
        // (Maj + clic garde le choix d'avant : sans choix il prend la case cliquée, avec un choix il l'étend à cette case.)
        if !outil_camera && !m.shift {
            self.vignettes.libre = true;
            self.vignettes.camera_choisie = None;
        }
        // Maj seule (partie 113, proposition A) : la planche change de peau, et tout ce qui n'est pas dessiner se fait avec Maj. Sur une
        // case : un coin (de la case ou d'un cadre de caméra) tire la caméra ; le bord d'un cadre B, C… déplace ce cadre ; partout
        // ailleurs dans la case, glisser déplace le plan, et un clic sans mouvement le choisit (au lâcher). Maj + Alt (la copie) est retiré
        // (décision 5) : « Dupliquer le plan » reste au clic droit, à l'anneau et à sa touche.
        if m.shift && !m.ctrl && !m.command {
            if let Some(k) = layout.card_near(p, self.marge_camera()) {
                self.commit_note_text(layout);
                if self.maj_prend_la_camera(layout, xf, k, p) {
                    self.vignettes.press = if self.cam_press(layout, xf, k, pos, false) { Press::Select } else { Press::Dead };
                    return false;
                }
                self.start_plan_move(layout, k, pos, false);
                if let Some(mv) = self.vignettes.moving.as_mut() {
                    mv.clic = Some((k, m));
                }
                return false;
            }
        }
        // Ctrl + Maj, ou T tenue : un appui sur une case, son empreinte ou sa marge est TOUJOURS un geste de caméra (jamais un
        // plan qu'on déplace, ni le dessin, ni le lasso). Un appui qui rate un cadre en prend quand même un (`vignettecam::choisit`).
        // (Ctrl seul est le lasso, comme partout : plus bas. Ctrl + Alt, « un cadre de plus », est retiré depuis la partie 113, décision 5 :
        // « Ajouter un cadre » est au clic droit de la case.)
        let cam_mode = ((m.command || m.ctrl) && m.shift) || self.vignettes.camera_tenue || outil_camera;
        if cam_mode {
            if let Some(k) = layout.card_near(p, self.marge_camera()) {
                self.commit_note_text(layout);
                let ajoute = false;
                // L'outil Caméra : un clic choisit la caméra de la case et rend son plan courant, sans autre appui ; un glissé qui part du
                // premier appui déplace le cadre (le geste de caméra ordinaire, une étape d'annulation).
                if outil_camera {
                    let c = layout.cards[k];
                    self.board_sel = vec![c.plan];
                    self.board_notes.clear();
                    self.vignettes.libre = false;
                    self.vignettes.camera_choisie = Some(c.plan);
                    let f = self.cur_frame.min(self.frame_count().saturating_sub(1));
                    if !(c.panel == self.cur_panel && (c.frame..c.end).contains(&f)) {
                        self.select_board(c.panel, c.frame);
                    }
                }
                if self.cam_press(layout, xf, k, pos, ajoute) {
                    self.vignettes.press = Press::Select;
                } else {
                    // Pas de mouvement possible ici : un glissé ne fait rien.
                    self.vignettes.press = Press::Dead;
                }
                return false;
            }
        }
        // Ctrl seul (Alt : la pipette) : le lasso, dans le dessin de la case, comme partout (`springtools.rs`).
        let lasso = (m.command || m.ctrl) && !m.shift && !m.alt;
        let selecting = m.shift;
        match hit {
            // Une barre ne répond qu'au lâcher d'un clic qui n'a pas bougé (partie 113, décision 11) : un appui qui bouge est un trait,
            // parti dès son premier point dans la case la plus proche, comme depuis le papier (`appui_bande`) ; au lâcher sans
            // mouvement, ce point de trait est retiré et la barre bascule (`barre_au_lacher`).
            Hit::Bar(after) => {
                self.commit_note_text(layout);
                let dessine = !cam_mode && !lasso && !selecting && matches!(self.tool, Tool::Brush | Tool::Pencil | Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Polyline);
                self.vignettes.barre_attente = Some(BarreAttente { after, origine: pos, base_len: self.history.undo_len(), dessine });
                if dessine {
                    let proche = layout.cards.iter().enumerate().min_by(|a, b| distance_rect(a.1.thumb, p).total_cmp(&distance_rect(b.1.thumb, p))).map(|(k, _)| k);
                    if let Some(k) = proche {
                        let c = layout.cards[k];
                        self.vignettes.appui_bande = Some(pos);
                        self.noter_appui_papier(layout, p);
                        self.board_sel = vec![c.plan];
                        self.board_notes.clear();
                        let f = self.cur_frame.min(self.frame_count().saturating_sub(1));
                        if !(c.panel == self.cur_panel && (c.frame..c.end).contains(&f)) {
                            self.select_board(c.panel, c.frame);
                        }
                        self.vignettes.press = Press::Card;
                        self.draw_has_focus();
                        self.pastille_avant_trait();
                        return false;
                    }
                }
                self.vignettes.press = Press::Bar;
                false
            }
            // Une bande de note n'est plus une zone d'écriture : un appui dessus dessine dans sa case, comme n'importe où près d'elle (le
            // trait qui part de la marge sous la case n'ouvre rien, `note_icone_rect` écrit la note).
            Hit::Thumb(k) | Hit::Note(k) => {
                self.commit_note_text(layout);
                let c = layout.cards[k];
                if matches!(hit, Hit::Note(_)) && !selecting {
                    self.vignettes.appui_bande = Some(pos);
                    self.noter_appui_papier(layout, p);
                }
                if selecting {
                    // Maj + appui : choisir des plans. (Ctrl + Maj, ou T tenue : un geste de caméra, plus haut.)
                    self.board_select_click(layout, k, m);
                    self.vignettes.press = Press::Select;
                } else {
                    self.board_sel = vec![c.plan];
                    self.board_notes.clear();
                    let f = self.cur_frame.min(self.frame_count().saturating_sub(1));
                    if !(c.panel == self.cur_panel && (c.frame..c.end).contains(&f)) {
                        self.select_board(c.panel, c.frame);
                    }
                    self.vignettes.press = Press::Card;
                    self.draw_has_focus();
                    if lasso {
                        self.prendre_le_lasso();
                    } else if !cam_mode {
                        // Une brosse du décor choisie : le trait part dans le calque Décor du plan (`decorui.rs`) ; un personnage : dans son
                        // calque (`calquespastilles.rs`).
                        self.pastille_avant_trait();
                    }
                }
                false
            }
            // Le titre d'une séquence : un double-clic le renomme (plus bas) ; un appui dessus ne dessine rien.
            Hit::Titre(_) => {
                self.commit_note_text(layout);
                self.vignettes.press = Press::Dead;
                false
            }
            Hit::Empty(e) => {
                self.commit_note_text(layout);
                if !cam_mode && !lasso && self.tool_draws_in_empty() && self.board_create_from_empty(e) {
                    self.vignettes.press = Press::Card;
                    self.draw_has_focus();
                    self.pastille_avant_trait();
                    true
                } else {
                    self.vignettes.press = Press::Dead;
                    if !cam_mode && !lasso {
                        self.dire_l_outil_sans_effet(true);
                    }
                    false
                }
            }
            Hit::Nothing => {
                // Hors de toute image, toute la planche se dessine (Anthony, 2026-10-30) : le trait part dans le dessin de la case la plus
                // proche, et au lâcher il va à qui revient son premier point (partie 131, horschamp.rs) : la case dont la marge le contient,
                // sinon le papier, entier.
                if !cam_mode && !lasso && !selecting && matches!(self.tool, Tool::Brush | Tool::Pencil | Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Polyline) {
                    // La marge de **la case pâle suivante** vaut son cadre (partie 131 : le bout d'un trait ne la fait plus naître, on la fait
                    // naître en y posant le stylet, et sa marge est aussi large que celle d'une vraie case) : son plan naît, si le point n'est
                    // dans le champ d'aucune case. Seulement la première case pâle de sa séquence, comme pour le bout d'un trait avant (partie
                    // 113, décision 8) : un appui dans la marge d'une case pâle lointaine ne fait pas naître des plans vides devant elle, il
                    // est sur le papier. Poser le stylet **dans** une case pâle lointaine garde sa place, comme avant.
                    let debord = self.settings.vignettes.debord_fraction();
                    if self.tool_draws_in_empty() && !crate::horschamp::champs(layout, debord).iter().any(|c| c.contains(p)) && layout.annulee_en(p).is_none() {
                        let mut premieres: Vec<usize> = layout.groupes.iter().filter(|g| g.vides.start < g.vides.end).map(|g| g.vides.start).collect();
                        if premieres.is_empty() {
                            premieres.push(0);
                        }
                        if let Some(e) = premieres.into_iter().find(|e| layout.empty.get(*e).is_some_and(|r| r.expand(debord * r.width()).contains(p))) {
                            self.commit_note_text(layout);
                            if self.board_create_from_empty(e) {
                                self.vignettes.appui_bande = Some(pos);
                                self.vignettes.press = Press::Card;
                                self.draw_has_focus();
                                self.pastille_avant_trait();
                                return true;
                            }
                        }
                    }
                    let proche = layout.cards.iter().enumerate().min_by(|a, b| distance_rect(a.1.thumb, p).total_cmp(&distance_rect(b.1.thumb, p))).map(|(k, _)| k);
                    if let Some(k) = proche {
                        self.commit_note_text(layout);
                        let c = layout.cards[k];
                        self.vignettes.appui_bande = Some(pos);
                        self.noter_appui_papier(layout, p);
                        self.board_sel = vec![c.plan];
                        self.board_notes.clear();
                        let f = self.cur_frame.min(self.frame_count().saturating_sub(1));
                        if !(c.panel == self.cur_panel && (c.frame..c.end).contains(&f)) {
                            self.select_board(c.panel, c.frame);
                        }
                        self.vignettes.press = Press::Card;
                        self.draw_has_focus();
                        self.pastille_avant_trait();
                        return false;
                    }
                }
                if matches!(self.tool, Tool::Brush | Tool::Pencil | Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Polyline | Tool::Fill | Tool::Cutter) {
                    self.vignettes.press = Press::Dead;
                    self.dire_l_outil_sans_effet(false);
                }
                false
            }
        }
    }

    /// Un appui hors des cases qui dessine, au point `p` de la planche : son trait ira-t-il au papier au lâcher (partie 131 : hors du champ de
    /// toute case, ou sur une case écartée) ? Alors « Peint sur l'aplat » ne le détourne pas (partie 133, `aplatsur.rs`).
    pub(crate) fn noter_appui_papier(&mut self, layout: &BoardLayout, p: Pos2) {
        let debord = self.settings.vignettes.debord_fraction();
        self.vignettes.appui_papier = layout.annulee_en(p).is_some() || !crate::horschamp::champs(layout, debord).iter().any(|c| c.contains(p));
    }

    /// Un appui qui ne fait rien le dit (partie 113) : le pot de peinture et le cutter n'agissent que dans une case dessinée ; `vide` :
    /// l'appui est tombé dans une case pâle (un outil qui ne fait pas de plan).
    fn dire_l_outil_sans_effet(&mut self, vide: bool) {
        let outil = match self.tool {
            Tool::Fill => "Le pot de peinture",
            Tool::Cutter => "Le cutter",
            _ => return,
        };
        let fait = if self.tool == Tool::Fill { "remplit une zone" } else { "coupe un trait" };
        self.status = if vide {
            format!("{outil} {fait} dans une case déjà dessinée : dessine d'abord dans cette case (pinceau ou crayon).")
        } else {
            format!("{outil} {fait} dans une case : clique dans une case.")
        };
    }

    /// Maj tenue, sans appui en cours (partie 113, proposition A) : la planche change de peau. Chaque case visible prend un filet d'accent
    /// et une poignée à chaque coin (le coin tire la caméra) ; les cadres B, C… d'un mouvement ont les leurs. Rien n'est lu ni changé dans
    /// le projet ; Maj relâchée, tout redevient du papier.
    fn peindre_peau_maj(&self, painter: &egui::Painter, view: Rect, xf: Xf, layout: &BoardLayout) {
        let accent = crate::theme::accent();
        let clip = painter.with_clip_rect(view);
        for (k, place) in layout.cards.iter().enumerate() {
            if !xf.rect(place.foot).expand(20.0).intersects(view) {
                continue;
            }
            let (cadres, _) = self.cadres_sur_la_planche(layout, k);
            let mut premier = true;
            for q in &cadres {
                let pts: Vec<Pos2> = q.iter().map(|p| xf.to_screen(*p)).collect();
                clip.add(egui::Shape::closed_line(pts.clone(), Stroke::new(if premier { 1.5_f32 } else { 1.0 }, accent.gamma_multiply(if premier { 0.9 } else { 0.6 }))));
                premier = false;
                if self.cam_possible(place) {
                    for c in &pts {
                        clip.add(egui::Shape::circle_filled(*c, 4.0, Color32::WHITE));
                        clip.add(egui::Shape::circle_stroke(*c, 4.0, Stroke::new(1.3_f32, accent)));
                    }
                }
            }
        }
    }

    /// Le lâcher d'un appui tombé sur une barre (partie 113, décision 11) : s'il n'a pas bougé, c'est un clic : le point de trait qu'il a
    /// pu laisser est retiré (aucune étape ne reste) et la barre bascule (coupe, réunit, ou fait la case de continuité). Un appui qui
    /// a bougé est un trait, et la barre n'a rien fait. Vrai si le film a changé.
    fn barre_au_lacher(&mut self, layout: &BoardLayout, latest: Option<Pos2>, any_down: bool) -> bool {
        let Some(a) = self.vignettes.barre_attente else { return false };
        if latest.is_some_and(|p| p.distance(a.origine) > MOVE_THRESHOLD) {
            self.vignettes.barre_attente = None;
            return false;
        }
        if any_down {
            return false;
        }
        self.vignettes.barre_attente = None;
        if a.dessine {
            self.retirer_le_point_de_trait(a.base_len);
        }
        self.vignettes.press = Press::Bar;
        self.board_toggle_cut(layout, a.after)
    }

    /// Choisir des plans (comme sur l'ancien titre) : Ctrl ajoute ou retire celui de la vignette `k`, Maj prend l'intervalle.
    pub(crate) fn board_select_click(&mut self, layout: &BoardLayout, k: usize, m: Modifiers) {
        let c = layout.cards[k];
        // Rien n'était choisi : le plan cliqué l'est maintenant (il devient le plan courant, seul), les clics suivants en ajoutent.
        if std::mem::replace(&mut self.vignettes.libre, false) {
            self.board_sel = vec![c.plan];
            self.select_board(c.panel, c.frame);
            return;
        }
        let current = c.panel == self.cur_panel;
        if m.command || m.ctrl {
            if let Some(at) = self.board_sel.iter().position(|s| *s == c.plan) {
                self.board_sel.remove(at);
            } else if !current {
                self.board_sel.push(c.plan);
            }
        } else {
            let (a, b) = (self.cur_panel.min(c.panel), self.cur_panel.max(c.panel));
            self.board_sel = self.doc.panels[a..=b].iter().map(|p| p.id).collect();
        }
    }

    /// Coupe le plan `plan` à l'image `frame` (la barre entre deux vignettes d'un même plan) : une étape d'annulation, nommée. Vrai si
    /// le document a changé.
    pub(crate) fn couper_entre_vignettes(&mut self, plan: u64, frame: usize) -> bool {
        let Some(index) = self.doc.panels.iter().position(|p| p.id == plan) else { return false };
        let ok = self.avec_etape("Couper le plan entre deux vignettes", |s| s.structure_op_dessins(|d, _| d.split_plan(index, frame)));
        if ok {
            self.cur_frame = 0;
            self.board_sel = vec![self.doc.panels[self.cur_panel].id];
            self.status = format!("Plan coupé : la vignette de droite commence le {}.", crate::storyboard::panel_title(&self.doc, self.cur_panel).to_lowercase());
        }
        ok
    }

    /// Bascule la barre après l'image `after` : grise → noire coupe le plan là (l'image de droite commence un
    /// plan neuf), noire → grise réunit les deux plans (l'image de droite devient une image du plan de gauche).
    /// La dernière barre, devant la première case vide, dit ce que fera cette case : un plan ou une image.
    /// Une seule étape d'annulation, nommée. Vrai si le document a changé.
    pub(crate) fn board_toggle_cut(&mut self, layout: &BoardLayout, after: usize) -> bool {
        // (La barre devant une case annulée ne coupe ni ne réunit : `planannule.rs`.)
        if layout.barres_mortes.contains(&after) {
            self.status = "La case suivante est écartée, cette barre ne réunit rien : un clic sur son numéro la reprend".into();
            return false;
        }
        let Some(a) = layout.cards.get(after).copied() else { return false };
        let Some(b) = layout.cards.get(after + 1).copied().filter(|_| !layout.vers_pale(after)) else {
            // Devant la première case vide (de la séquence) : un clic fait tout de suite la case de continuité (une image de plus du dernier
            // plan, avec la copie de son dessin), et la case vide d'après reste une coupe (Anthony, 2026-10-30).
            return self.board_continuite_de(Some(a.plan));
        };
        if b.plan == a.plan {
            // Couper : la marque de la vignette de droite devient le début d'un plan neuf.
            if self.storyboard_refuses() || !self.partage_peut_toucher(&[a.plan]) {
                return false;
            }
            // Un plan qui a des versions rangées : la question « avec quelle partie ? » d'abord (`coupeversions.rs`).
            if self.coupe_versions_necessaire(a.plan, b.frame) {
                return self.coupe_versions_ouvrir(a.plan, b.frame, crate::coupeversions::Refaire::Vignettes { plan: a.plan, frame: b.frame });
            }
            self.couper_entre_vignettes(a.plan, b.frame)
        } else {
            // Réunir : le début du plan de droite devient une marque d'image du plan de gauche.
            if self.storyboard_refuses() || !self.partage_peut_toucher(&[a.plan, b.plan]) {
                return false;
            }
            // Deux calques de même nom : la fenêtre le demande d'abord (`fusionnoms.rs`) ; fausse tant qu'elle attend la réponse.
            let Some(a_id) = self.doc.panels.get(a.panel).map(|p| p.id) else { return false };
            self.jonction_demander(crate::fusionnoms::Jonction::Case { a: a_id, fin: a.end })
        }
    }

    /// Clic sur la barre qui précède la première case vide (Anthony, 2026-10-30) : la case devient tout de suite une image de plus du
    /// dernier plan, avec **la copie du dessin de l'image d'avant** (décor compris), sans attendre un trait. Rien d'autre ne change : la
    /// case vide d'après est une coupe, comme toute case vide. Une seule étape d'annulation, nommée.
    ///
    /// Pour le plan `plan` (le dernier de sa séquence : la barre devant la case pâle d'une séquence) ; rien : le dernier plan du film.
    pub(crate) fn board_continuite_de(&mut self, plan: Option<u64>) -> bool {
        if self.storyboard_refuses() || self.doc.panels.is_empty() {
            return false;
        }
        let mut frame_of_image = 0;
        let cartes_avant: Vec<u64> = self.board_layout().cards.iter().map(|c| c.id).collect();
        let marque = self.history.marque();
        let ok = self.avec_etape("Continuité avec le plan précédent", |s| {
            s.structure_op(|d, _| {
                let last = plan.and_then(|id| d.panels.iter().position(|p| p.id == id)).unwrap_or(d.panels.len() - 1);
                let old = d.panels[last].frame_count();
                let image = board::plan_boards(d, last).last().map_or(1, |b| b.2 - b.1).max(1);
                d.set_duration(last, old + image);
                d.toggle_mark(last, old)?;
                d.decor_case_ajoutee(last, old);
                d.cles_de_case(last, old);
                frame_of_image = old;
                Some(last)
            })
        });
        self.vignettes.empty_joins = false;
        if !ok {
            return false;
        }
        self.cur_frame = frame_of_image;
        self.follow_frame = true;
        self.board_sel = vec![self.doc.panels[self.cur_panel].id];
        // La case née prend les traits du papier qui passaient là (partie 128, `caseprend.rs`), dans la même étape.
        self.cases_nees_prennent_le_papier(&cartes_avant, marque);
        self.status = "Continuité : la case montre déjà le dessin de la précédente.".into();
        true
    }

    /// Dessiner dans la case vide `e` (0 : la première) fait un plan neuf, avec ceux qui la précèdent (des plans
    /// vides, pour garder sa place) ; ou, si la barre devant la première case vide est grise, une image de plus
    /// du dernier plan. Le plan fait est courant, prêt à recevoir le trait de l'appui en cours. Vrai si fait.
    pub(crate) fn board_create_from_empty(&mut self, e: usize) -> bool {
        if self.storyboard_refuses() {
            return false;
        }
        let joins = self.vignettes.empty_joins;
        // La case pâle de quelle séquence ? Le plan se fait juste après le dernier plan de sa séquence, dans la séquence ; `e` compte depuis la
        // première case pâle de cette séquence (celles qui la précèdent deviennent des plans vides).
        let (apres, e, cartes_avant) = {
            let layout = self.board_layout();
            let (rang, e) = layout.vide_de(e, self.doc.panels.len());
            (self.doc.panels.get(rang).map(|p| p.id), e, layout.cards.iter().map(|c| c.id).collect::<Vec<u64>>())
        };
        let made = e + 1;
        let mut frame_of_image = 0;
        let marque = self.history.marque();
        let ok = self.avec_etape(if joins && e == 0 { "Nouvelle image dessinée" } else { "Nouveau plan dessiné" }, |s| {
            s.structure_op(|d, _| {
                let mut last = apres.and_then(|id| d.panels.iter().position(|p| p.id == id)).unwrap_or(d.panels.len() - 1);
                let mut first_plan = 0;
                if joins {
                    // Une image de plus au dernier plan : il s'allonge de la durée de sa dernière image.
                    let old = d.panels[last].frame_count();
                    let image = board::plan_boards(d, last).last().map_or(1, |b| b.2 - b.1).max(1);
                    d.set_duration(last, old + image);
                    d.toggle_mark(last, old)?;
                    // « Report du décor » éteint : la case de plus repart d'un décor vide (`decor.rs`).
                    d.decor_case_ajoutee(last, old);
                    // Et une clé de dessin de plus sur chaque calque, à l'image de la case (`cases.rs`).
                    d.cles_de_case(last, old);
                    frame_of_image = old;
                    first_plan = 1;
                }
                for _ in first_plan..made {
                    last = d.add_scene_after(last);
                }
                Some(last)
            })
        });
        // La continuité ne vaut que pour cette case : la suivante est une coupe (pas de cascade, Anthony, 2026-10-30).
        self.vignettes.empty_joins = false;
        if !ok {
            return false;
        }
        self.cur_frame = if joins && e == 0 { frame_of_image } else { 0 };
        self.follow_frame = true;
        self.board_sel = vec![self.doc.panels[self.cur_panel].id];
        // Les cases nées (celle qu'on dessine, les plans vides devant elle) prennent les traits du papier qui passaient là (partie 128,
        // `caseprend.rs`), dans la même étape que la création.
        self.cases_nees_prennent_le_papier(&cartes_avant, marque);
        self.vignettes.created = Some(Created { base_len: self.history.undo_len() });
        // (Les cases pâles sautées sont devenues des plans vides : la ligne d'état le dit, partie 113.)
        let vides = if joins && e == 0 { 0 } else { made - 1 };
        self.status = match (joins && e == 0, vides) {
            (true, _) => "Nouvelle image du plan : dessine-la.".into(),
            (false, 0) => "Nouveau plan : dessine-le.".into(),
            // (Pas de parenthèse ni de point avant le compte : la barre d'état le montre en entier.)
            (false, 1) => "Nouveau plan et un plan vide avant lui, pour garder sa place : dessine-le.".into(),
            (false, n) => format!("Nouveau plan et {n} plans vides avant lui, pour garder sa place : dessine-le."),
        };
        true
    }

    /// Une commande pendant le trait qui vient de faire un plan, une image ou le calque Décor (`Vignettes::created`), ou un
    /// changement d'espace de travail ou d'onglet : le trait se termine et la création prend son étape tout de suite. Sinon la
    /// création attendait le lâcher sur la planche — jamais vu si l'on avait quitté les Vignettes — et prenait dans son étape
    /// tout ce qui s'était fait depuis (la commande, puis d'autres gestes ailleurs) : annuler défaisait tout d'un coup (passe
    /// de débogage au hasard, 2026-10-23).
    pub(crate) fn poser_la_creation(&mut self) {
        if self.vignettes.created.is_none() {
            return;
        }
        self.end_stroke();
        self.finish_creation(false);
    }

    /// Au lâcher : la création d'une case vide et le trait ne font qu'une étape d'annulation ; si rien n'a été
    /// tracé (un simple clic), le plan fait est défait sans laisser de trace. (Aussi pour le calque Décor fait par le premier
    /// trait d'une brosse du décor sur le dessin du Storyboard, `canvas_input`.)
    pub(crate) fn finish_creation(&mut self, any_down: bool) {
        let Some(c) = self.vignettes.created else { return };
        if any_down || self.stroke.is_some() || self.shape_drag.is_some() {
            return;
        }
        self.vignettes.created = None;
        let n = self.history.undo_len().saturating_sub(c.base_len);
        if n >= 1 {
            self.history.merge_last(n + 1);
        } else {
            let mut nav = Nav { panel: self.cur_panel, layer: self.cur_layer };
            if self.history.undo_and_forget(&mut self.doc, &mut nav) {
                self.cur_panel = nav.panel.min(self.doc.panels.len() - 1);
                self.cur_layer = nav.layer;
                self.revalider_rangs();
                self.cur_frame = self.cur_frame.min(self.frame_count().saturating_sub(1));
                self.board_sel.clear();
                self.renderer.invalidate_all();
                self.thumbs.prune(&self.doc);
            }
        }
    }

    // ------------------------------------------------------------------ la vue

    pub(crate) fn board_view(&mut self, ui: &mut egui::Ui) {
        // (Les pastilles des personnages : une rangée à l'extrémité droite de la barre du haut de la planche, posée en fin d'image :
        // `pastilles_rangee_flottante`, `personnagesui.rs`.)
        let haut_droite = egui::pos2(ui.max_rect().right(), ui.available_rect_before_wrap().top());
        // Le débord du trait hors des cases suit les Préférences (il fait partie de la clé des vignettes : elles se refont en fond).
        self.thumbs.debord = Some(self.settings.vignettes.debord_fraction());
        // (Ce que les vignettes savent des marges des cases vaut pour cet état du projet : `board_marges`. L'historique ne suffit pas à le
        // dire : un dessin vectoriel refait en arrière-plan change ses pixels, donc jusqu'où il va, sans étape de plus ; la version de
        // chaque dessin, elle, change alors.)
        let versions = self.doc.cels.values().fold(0u64, |s, c| s.wrapping_add(c.version));
        self.thumbs.regler_serie(self.history.serial() ^ versions.rotate_left(32));
        // (Les cases ne montrent pas les calques d'annotation du plan : on ne dessine pas dedans depuis la planche.)
        self.ecarter_le_calque_d_annotation();
        self.board_toolbar(ui);
        self.variantes_barre(ui);
        // La barre du bas (passe-partout des cases, zoom) prend le bas de la vue, comme sous le dessin (Anthony, 2026-10-30).
        let plein = ui.available_rect_before_wrap();
        let haut_barre = crate::vignettesprefs::BARRE_BASSE_H;
        let barre = Rect::from_min_max(pos2(plein.left(), plein.bottom() - haut_barre), plein.max);
        let view = Rect::from_min_max(plein.min, pos2(plein.right(), (plein.bottom() - haut_barre).max(plein.top() + 1.0)));
        self.vignettes.barre_basse = barre;
        self.vignettes.view = view;
        self.planche_barre_basse(ui, barre);
        self.vignettes.etiquettes.clear();
        let bg = ui.allocate_rect(view, Sense::click_and_drag());
        // L'anneau sous le stylet joue aussi sur la planche (`anneauui.rs`) : glisser lance une commande, maintenir le montre. Le clic
        // bref reste celui d'avant : le menu de la case, de la barre ou de la planche.
        if self.anneau_en_service() && !self.modal_open() {
            self.anneau_noter_le_dessin(&bg, view, false);
        }
        let painter = ui.painter_at(view);
        painter.rect_filled(view, 0.0, PAPER);
        self.board_sel.retain(|id| self.doc.panels.iter().any(|p| p.id == *id));
        let (ns, nt) = (self.doc.board.strokes.len(), self.doc.board.texts.len());
        self.board_notes.retain(|n| match *n {
            NoteRef::Stroke(i) => i < ns,
            NoteRef::Text(i) => i < nt,
        });
        let mut layout = self.board_layout();
        self.suivre_decalage(&layout);
        if self.board_zoom <= 0.0 {
            // En lignes la page tient à la largeur de la vue (les rangées s'ajoutent dessous) ; à la japonaise, à sa hauteur (les colonnes
            // s'ajoutent à droite).
            if std::mem::take(&mut self.vignettes.ouverture_a_faire) {
                // À l'ouverture d'un projet : 2 rangées de 6 cases (réglable), textes compris, cadrées sur le haut de la planche ou sur le plan
                // courant s'il est plus loin (Anthony, 2026-10-30).
                self.ouverture_cadrer(&layout, view);
            } else {
                let largeur = view.width() / layout.size.x.max(1.0);
                let hauteur = view.height() / layout.size.y.max(1.0);
                // « Cadrer tout » : la planche entière, dans les deux sens (partie 113) ; sinon, à la largeur (en lignes) ou à la hauteur (à la japonaise).
                let tient = if std::mem::take(&mut self.vignettes.cadrer_tout) {
                    largeur.min(hauteur)
                } else if layout.sens.japonais() {
                    hauteur
                } else {
                    largeur
                };
                self.board_zoom = tient.clamp(ZOOM_MIN, 1.5);
                self.board_pan = Vec2::ZERO;
                self.vignettes.ouverture_suivi = None;
            }
        } else if let Some((taille, z, p, _)) = self.vignettes.ouverture_suivi {
            // (Les panneaux se rangent pendant les premières images d'un projet qui s'ouvre : tant que personne n'a touché au zoom ni au défilement,
            // le cadrage d'ouverture suit la vue, pendant une dizaine d'images seulement ; ensuite le zoom ne bouge plus.)
            self.vignettes.ouverture_images = self.vignettes.ouverture_images.saturating_sub(1);
            if self.board_zoom != z || self.board_pan != p || self.vignettes.ouverture_images == 0 {
                self.vignettes.ouverture_suivi = None;
            } else if (taille - view.size()).abs().max_elem() > 1.0 {
                self.ouverture_cadrer(&layout, view);
            }
        }
        // Les annotations d'une vignette qui a disparu passent à sa voisine (`annotationsui.rs`).
        self.annot_suivre(&layout);

        let (events, hover, scroll, zoom_delta, mods, space, any_down, escape, cam_key) = ui.input(|i| {
            (
                i.events.clone(),
                i.pointer.hover_pos(),
                i.smooth_scroll_delta,
                i.zoom_delta(),
                i.modifiers,
                self.keymap.held(i, "vue.pan_libre"),
                i.pointer.any_down(),
                i.key_pressed(Key::Escape),
                self.keymap.held_any(i, "vignettes.camera"),
            )
        });
        let writing = self.board_text.is_some() || self.vignettes.note.is_some();
        // T tenue (comme Espace pour la main) : tout glissé sur une case tire la caméra ; relâchée, l'outil d'avant est là (on n'en a pas changé).
        self.vignettes.camera_tenue = (cam_key || (self.vignettes.camera_tenue && any_down && self.vignettes.cam.is_some())) && !writing && !self.modal_open();
        let pan_mode = self.tool == Tool::Pan || (space && !writing);
        let over = hover.filter(|p| view.contains(*p) && ui.ctx().layer_id_at(*p) == Some(bg.layer_id));
        // Pression du stylet (Windows Ink : événements « tactiles »).
        let touch = events.iter().rev().find_map(|e| match e {
            Event::Touch { force: Some(f), .. } => Some(f.clamp(0.0, 1.0)),
            _ => None,
        });
        if let Some(f) = touch {
            self.pen_pressure = f;
        }

        // Molette : défiler ; Ctrl + molette (ou pincement) : zoom autour du pointeur.
        if let Some(p) = over {
            if (zoom_delta - 1.0).abs() > 1e-4 {
                self.board_zoom_at(view, p, zoom_delta);
            } else if scroll != Vec2::ZERO {
                self.board_pan += scroll;
            }
        }
        if bg.dragged_by(PointerButton::Middle) || (pan_mode && bg.dragged_by(PointerButton::Primary)) {
            self.board_pan += bg.drag_delta();
        }
        // Une commande vient de choisir le plan courant : sa case entre dans la vue (le moins de défilement possible), partie 113.
        if std::mem::take(&mut self.vignettes.montrer_courante) && self.board_zoom > 0.0 {
            let f = self.cur_frame.min(self.frame_count().saturating_sub(1));
            if let Some(c) = layout.cards.iter().find(|c| c.panel == self.cur_panel && (c.frame..c.end).contains(&f)) {
                let xf = Xf { origin: view.min + self.board_pan, zoom: self.board_zoom };
                let r = xf.rect(c.card.union(c.note)).expand(12.0);
                let glisse = |bas: f32, haut: f32, min: f32, max: f32| if bas < min { min - bas } else if haut > max { (max - haut).max(min - bas) } else { 0.0 };
                let d = vec2(glisse(r.left(), r.right(), view.left(), view.right()), glisse(r.top(), r.bottom(), view.top(), view.bottom()));
                if d != Vec2::ZERO {
                    self.board_pan += d;
                    self.vignettes.ouverture_suivi = None;
                }
            }
        }
        self.clamp_board_pan(view, layout.size);
        if let Some(s) = self.vignettes.ouverture_suivi.as_mut().filter(|s| s.3) {
            s.2 = self.board_pan;
            s.3 = false;
        }
        let xf = Xf { origin: view.min + self.board_pan, zoom: self.board_zoom };
        self.vignettes.xf = xf;

        // Les appuis de cette image, avant de dessiner : une image devient courante, une case vide devient un
        // plan, et le trait qui commence dans l'appui est dessiné dans la même image d'écran (aucun retard).
        if !pan_mode && !self.modal_open() {
            // (Un seul appui du bouton principal par pression : un stylet ou un pavé tactile double parfois l'événement de la souris par le sien,
            // deux appuis dans la même image d'écran sans lâcher entre eux. Le second, rejoué sur une planche que le premier vient de
            // changer, tombait sur la barre d'à côté : « décocher une coupe décoche la suivante aussi ».)
            let mut appui_pris = false;
            for e in &events {
                if let Event::PointerButton { button: PointerButton::Primary, pressed: false, .. } = e {
                    appui_pris = false;
                }
                let Event::PointerButton { pos, button, pressed: true, modifiers } = e else { continue };
                if !view.contains(*pos) || ui.ctx().layer_id_at(*pos) != Some(bg.layer_id) {
                    continue;
                }
                // Un geste de l'anneau sous le stylet est en cours (`anneauui.rs`) : la pointe posée pendant que le bouton du stylet
                // est tenu ne dessine ni ne choisit rien sur la planche.
                if *button == PointerButton::Primary && self.anneau.geste.is_some() {
                    continue;
                }
                if *button == PointerButton::Primary {
                    if appui_pris {
                        continue;
                    }
                    appui_pris = true;
                }
                // (Chaque appui repart de rien : un appui d'avant, resté là parce que la vue n'était pas à l'écran au lâcher, ne compte plus.)
                if *button == PointerButton::Primary {
                    self.vignettes.press = Press::None;
                    // Le stylo ou la gomme d'annotation prend l'appui avant tout : cases, barres, notes, fond (`annotationsui.rs`).
                    if self.annot_actif() {
                        self.commit_note_text(&layout);
                        self.commit_card_note();
                        self.vignettes.press = Press::Annot;
                        self.annot_appui(*pos);
                        continue;
                    }
                    // Ctrl seul sur un calque vectoriel : la sélection de la planche, dans une case comme sur le papier (partie 125). Tout autre appui la lève.
                    // (Un calque bitmap : la sélection de ses pixels, partie 127, `vignetteselbmp.rs`.)
                    let pris = if self.selb_possible() {
                        self.selp_lever();
                        self.selb_appui(xf, *pos, *modifiers)
                    } else {
                        self.vignettes.selb = None;
                        self.selp_appui(&layout, xf, *pos, *modifiers)
                    };
                    if pris {
                        self.commit_note_text(&layout);
                        self.commit_card_note();
                        self.vignettes.press = Press::SelPlanche;
                        // (Ctrl + appui dans une case la rend courante, comme avant : le lasso y était pris dans son dessin.)
                        if let Some(c) = layout.cards.iter().find(|c| c.thumb.contains(xf.to_board(*pos))).copied() {
                            self.board_sel = vec![c.plan];
                            self.board_notes.clear();
                            self.vignettes.libre = false;
                            let f = self.cur_frame.min(self.frame_count().saturating_sub(1));
                            if !(c.panel == self.cur_panel && (c.frame..c.end).contains(&f)) {
                                self.select_board(c.panel, c.frame);
                            }
                        }
                        continue;
                    }
                }
                if self.board_press(&layout, xf, *pos, *modifiers, *button) {
                    layout = self.board_layout();
                    self.suivre_decalage(&layout);
                    self.clamp_board_pan(view, layout.size);
                }
            }
        }
        let xf = Xf { origin: view.min + self.board_pan, zoom: self.board_zoom };
        self.vignettes.xf = xf;
        // Le trait qui déborde est recalculé par la vignette courante, à chaque image où un geste dure.
        self.vignettes.debord = None;
        // Maj tenue sur une case : elle se prend (curseur « main », pas de cercle de pinceau), sauf à un coin, où c'est la caméra
        // (partie 113, proposition A).
        let maj_seule = !pan_mode && !self.modal_open() && !writing && mods.shift && !mods.ctrl && !mods.command;
        self.vignettes.frame_hot = self.vignettes.moving.is_some()
            || (maj_seule && over.is_some_and(|p| layout.card_near(xf.to_board(p), self.marge_camera()).is_some_and(|k| !self.maj_prend_la_camera(&layout, xf, k, xf.to_board(p)))));
        // Ce que Maj permet, dit une fois par séance, la première fois qu'on la tient sur la planche.
        if maj_seule && over.is_some() && !any_down && !self.vignettes.maj_dit {
            self.vignettes.maj_dit = true;
            self.status = "Maj tenue : glisser une case déplace le plan, un clic le choisit, un coin tire la caméra, un clic sur une barre coupe ou réunit, le + ajoute un cadre.".into();
        }
        // Le « + » d'un cadre de plus : Maj, T ou l'outil Caméra, pas pendant un appui ni un geste (partie 116).
        let plus_voulu = (maj_seule || (!pan_mode && !self.modal_open() && !writing && !mods.ctrl && !mods.command && (self.vignettes.camera_tenue || self.tool == Tool::Camera)))
            && !any_down
            && self.vignettes.cam.is_none()
            && self.vignettes.moving.is_none()
            && self.vignettes.plus.geste.is_none()
            && !self.annot_actif();
        self.plus_mettre_a_jour(&layout, xf, view, over, plus_voulu);

        let texts = self.text_rects(&painter, &layout);
        let tol = 5.0 / xf.zoom;
        let free = self.vignettes.press == Press::None;

        // Gestes commencés sur le fond (hors de toute image, de barre et de note).
        if bg.drag_started_by(PointerButton::Primary) && !pan_mode && free {
            self.commit_note_text(&layout);
            let start = xf.to_board(ui.input(|i| i.pointer.press_origin()).unwrap_or(view.center()));
            let add = mods.ctrl || mods.shift;
            // (Un geste de gomme neuf : rien ne reste d'un geste d'avant qui ne se serait pas fini, `gommedebord.rs`.)
            self.gomme_debords_oublier();
            self.board_drag = Some(match self.tool {
                Tool::Eraser => BoardDrag::Erase { recorded: false },
                Tool::Select => match self.note_at(&layout, &texts, start, tol) {
                    Some(n) => {
                        if !self.board_notes.contains(&n) {
                            if !add {
                                self.board_notes.clear();
                            }
                            self.board_notes.push(n);
                        }
                        BoardDrag::MoveNotes { last: start, recorded: false }
                    }
                    None => BoardDrag::Rubber { start, now: start, add },
                },
                _ => BoardDrag::Rubber { start, now: start, add },
            });
        }
        // Un double-clic sur le titre d'une séquence le renomme (`planchesequencesui.rs`).
        if let Some(g) = bg.interact_pointer_pos().filter(|_| bg.double_clicked()).and_then(|p| layout.titre_en(xf.to_board(p))) {
            self.commit_note_text(&layout);
            let seq = layout.groupes[g].sequence;
            self.titre_ouvrir(seq);
        }
        if free {
            if let Some(p) = bg.interact_pointer_pos().filter(|_| bg.clicked()).map(|p| xf.to_board(p)) {
                self.board_click(&layout, &texts, p, mods);
            }
            if let Some(p) = bg.interact_pointer_pos().filter(|_| bg.double_clicked()).map(|p| xf.to_board(p)) {
                if let Some(NoteRef::Text(i)) = self.note_at(&layout, &texts, p, tol) {
                    self.edit_note_text(&layout, i);
                }
            }
        }
        if escape && !writing && self.vignettes.moving.is_none() {
            match self.board_drag {
                Some(BoardDrag::Rubber { .. }) => self.board_drag = None,
                _ => {
                    self.board_notes.clear();
                    // Échap retire aussi le plan choisi (rien d'autre n'est en cours ici).
                    if self.vignettes.cam.is_none() {
                        self.vignettes.libre = true;
                        self.vignettes.camera_choisie = None;
                    }
                }
            }
        }
        let latest = ui.input(|i| i.pointer.latest_pos());
        let pointer = latest.map(|p| xf.to_board(p));
        self.update_board_drag(&layout, pointer, xf.zoom);
        self.update_plan_move(&layout, xf, latest, escape);
        self.plus_suivre(&layout, xf, latest);
        self.update_cam_geste(latest, escape);
        self.cam_garder_la_place(&layout);
        // Le geste du stylo ou de la gomme d'annotation, point après point (`annotationsui.rs`).
        self.annot_geste(&layout, xf, &events, latest, any_down, touch.is_some(), escape, mods);
        // Le lasso ou le geste de la sélection de la planche (`vignettesel.rs`).
        // (Un appui hors de la planche, dans un panneau, le montage, la timeline : le choix est levé.)
        let dehors = ui.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !view.contains(p)));
        self.selp_geste(&layout, xf, latest, any_down, escape, dehors);
        self.selb_geste(&layout, xf, latest, any_down, escape, dehors);

        // Le papier est blanc partout (rien autour des cases) ; les barres, les cases pâles qui montrent où dessiner, les vignettes.
        let hovered = over.map(|p| board::hit(&layout, xf.to_board(p), BAR_TOUCH / xf.zoom.max(1e-3)));
        // (Maj tenue, sans appui en cours : la planche montre ses barres en entier, et plus loin les poignées des cases ; rien pendant un trait.)
        let peau_maj = maj_seule && !any_down && self.vignettes.cam.is_none();
        self.vignettes.peau_maj = peau_maj;
        self.paint_bars(ui, &painter, view, xf, &layout, hovered, peau_maj);
        let ghost_hot = if pan_mode { None } else if let Some(Hit::Empty(e)) = hovered { Some(e) } else { None };
        for (e, slot) in layout.empty.iter().enumerate() {
            let r = xf.rect(*slot);
            if r.intersects(view) {
                // (Une case pâle : un filet très léger ; sous la souris, un peu plus marqué.)
                let color = if ghost_hot == Some(e) { GHOST_HOT } else { GHOST };
                painter.rect_stroke(r, (3.0 * xf.zoom).clamp(1.0, 6.0), Stroke::new(1.0_f32, color), StrokeKind::Inside);
            }
        }
        if bg.hovered() || hovered.is_some() {
            let drawing = matches!(self.tool, Tool::Brush | Tool::Pencil | Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Polyline);
            let icon = match hovered {
                _ if pan_mode => CursorIcon::Grab,
                Some(Hit::Bar(_)) => CursorIcon::PointingHand,
                // Une ligne de texte (action, dialogue, notes) s'écrit d'un clic (sans Maj ni Ctrl) : le curseur de texte.
                Some(Hit::Note(k)) if over.is_some_and(|p| self.champ_sous(&layout.cards[k], xf.to_board(p)).is_some()) => CursorIcon::Text,
                Some(Hit::Empty(_)) if drawing => CursorIcon::Crosshair,
                Some(Hit::Thumb(_) | Hit::Note(_)) if drawing || self.tool == Tool::Eraser => CursorIcon::Crosshair,
                _ if self.tool == Tool::Text => CursorIcon::Text,
                _ if self.tool == Tool::Eraser => CursorIcon::Crosshair,
                _ => CursorIcon::Default,
            };
            // (La vignette courante, peinte après, remplace ce curseur par celui de son outil quand elle l'a sous la souris.)
            ui.ctx().set_cursor_icon(icon);
        }
        let deadline = Instant::now() + crate::thumbs::THUMB_TIME;
        // Les cases annulées, sous le dessin hors des cases : un trait qui les traverse reste visible (`planannuleui.rs`).
        self.peindre_cases_annulees(ui, &painter, view, xf, &layout, deadline);
        // **La feuille ou le film** (partie 131). La planche est une grande feuille où des caméras sont posées : **rien n'y est caché par une
        // case**. Le papier (le dessin hors des cases) et ce que chaque case a dessiné hors de son cadre se peignent **par-dessus les images de
        // toutes les cases**, sous leurs filets, leurs numéros et leurs notes. Avec le passe-partout, la planche montre le film : chaque case
        // ne montre que son plan, et tout le reste passe dessous, comme avant (les images le recouvrent, le passe-partout cache le dehors).
        let film = self.vignettes.annot.marie;
        if film {
            self.hors_cadres_peindre(&painter, view, xf, &layout, false);
        }
        // (Le temps des demandes de vignettes se compte d'ici, pour les marges et les cases : peindre le dessin hors des cases ne le prend plus. Avant, des
        // milliers de morceaux de la couche bitmap dans la vue usaient ces 2 ms à chaque image, et les vignettes ne se refaisaient plus : partie 127.)
        let deadline = Instant::now() + crate::thumbs::THUMB_TIME;
        // Les tuiles des empreintes d'abord (leur rectangle déborde de leur enveloppe : les cases qui s'y trouvent se peignent par-dessus).
        // Le menu du clic droit de la case courante se montre pendant qu'on peint les tuiles et les cases : « Réunir avec le plan
        // précédent », « Supprimer »… y changent le film, et la mise en page de cette image ne vaut plus. On s'arrête là, l'image
        // suivante la refait (Anthony, 2026-10-22 : réunir un plan à mouvement de caméra plantait sur un rang de plan disparu ; la
        // tuile d'un plan à mouvement d'après lui plantait de même, `case_tient`).
        let plans = self.doc.panels.len();
        let mut tient = true;
        // Les vignettes de toutes les cases (une seule demande par image d'écran). Le film : leurs marges d'abord, sur le papier, sous les images.
        let cases_tex = self.board_marges(ui, &painter, view, xf, &layout, pan_mode, deadline, plans, film);
        for k in 0..layout.cards.len() {
            if !self.case_tient(&layout, plans, k) {
                tient = false;
                break;
            }
            self.board_tuile(ui, &painter, view, xf, &layout, k, pan_mode, cases_tex[k], plans);
        }
        // (Les numéros des plans, trouvés une fois pour toutes les cases : un par case refaisait l'ordre du film.)
        let numeros = crate::numerotation::Numeros::pour_l_ecran(&self.doc);
        // (Où se posent les numéros : la marge au-dessus de chaque A, ou une place libre : `numerosmarge.rs`.)
        let mut marges = crate::numerosmarge::Marges::pour(&layout, xf, view);
        self.vignettes.numeros_zones.clear();
        // Les images de toutes les cases ; puis (la feuille) le papier et ce que chaque case a dessiné hors de son cadre, par-dessus ; puis les
        // filets, les numéros et les notes des cases, par-dessus tout le dessin.
        for k in (0..layout.cards.len()).take_while(|_| tient) {
            if !self.case_tient(&layout, plans, k) {
                tient = false;
                break;
            }
            self.board_card(ui, &painter, view, xf, &layout, k, pan_mode, cases_tex[k], &numeros, &mut marges, plans, PasseCase::Image);
        }
        if tient && !film {
            self.hors_cadres_peindre(&painter, view, xf, &layout, false);
            self.board_debords(&painter, view, xf, &layout, &cases_tex, pan_mode);
        }
        for k in (0..layout.cards.len()).take_while(|_| tient) {
            if !self.case_tient(&layout, plans, k) {
                tient = false;
                break;
            }
            self.board_card(ui, &painter, view, xf, &layout, k, pan_mode, cases_tex[k], &numeros, &mut marges, plans, PasseCase::Habillage);
        }
        if !tient || !self.case_tient(&layout, plans, layout.cards.len().saturating_sub(1)) {
            ui.ctx().request_repaint();
            return;
        }

        // Ce qui recouvre une case qui ne pouvait pas le recevoir : par-dessus les images (partie 122).
        self.hors_cadres_peindre(&painter, view, xf, &layout, true);
        self.horschamp_prevenir(&layout);
        self.peindre_numeros_annules(&painter, view, xf, &layout, &numeros, &mut marges);
        // Les titres des séquences, au-dessus de leur première rangée.
        self.titres_peindre(&painter, view, xf, &layout);
        // Notes libres par-dessus, puis ce qui est choisi, puis les repères du geste.
        for i in 0..self.doc.board.strokes.len() {
            if let Some(points) = self.doc.board.stroke_points(&layout, i) {
                paint_note(&painter, view, xf, &points, self.doc.board.strokes[i].color);
            }
        }
        let editing = self.board_text.as_ref().and_then(|e| e.index);
        for i in 0..self.doc.board.texts.len() {
            let Some((pos, size)) = self.doc.board.text_place(&layout, i) else { continue };
            if editing == Some(i) {
                continue;
            }
            let t = &self.doc.board.texts[i];
            let [r, g, b] = t.color;
            painter.text(xf.to_screen(pos), Align2::LEFT_TOP, &t.text, FontId::proportional(size * xf.zoom), Color32::from_rgb(r, g, b));
        }
        let accent = crate::theme::accent_texte();
        for &n in &self.board_notes {
            if let Some(b) = self.note_bounds(&layout, &texts, n) {
                let r = xf.rect(b).expand(3.0);
                painter.rect_stroke(r, 2.0, Stroke::new(1.5_f32, accent), StrokeKind::Outside);
                let anchored = match n {
                    NoteRef::Stroke(i) => self.doc.board.strokes[i].anchor.is_some(),
                    NoteRef::Text(i) => self.doc.board.texts[i].anchor.is_some(),
                };
                if anchored {
                    painter.text(r.right_top() + vec2(2.0, -2.0), Align2::LEFT_BOTTOM, egui_phosphor::regular::PAPERCLIP, FontId::proportional(13.0), accent);
                }
            }
        }
        // Le trait en cours qui sort de sa case, par-dessus tout ; puis le plan qu'on déplace (fantôme et repère).
        // (Au lâcher d'un trait qui sort de sa case, le trait qui déborde reste peint jusqu'à ce qu'il soit réparti : horschamp_partager.)
        let debord = self.vignettes.debord.take().or_else(|| self.vignettes.fini.as_ref().and_then(|_| self.vignettes.debord_garde.clone()));
        if let Some(d) = debord {
            crate::debord::paint(&painter, view, &d);
            self.vignettes.debord_garde = Some(d);
        }
        // Le trait qu'on vient de répartir : tenu tant que ses parts ne sont pas rendues (les demandes de cette image sont déjà en file).
        if let Some(t) = &self.vignettes.debord_tenu {
            let pret = self.thumbs.outstanding() == 0 && !self.thumbs.pending && !self.rejeu.has_pending();
            if pret || t.xf != xf || t.etapes != self.history.undo_len() || t.debut.elapsed() > crate::debord::TENU_MAX {
                self.vignettes.debord_tenu = None;
            } else {
                crate::debord::paint(&painter, view, &t.debord);
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
            }
        }
        // Les annotations, par-dessus tout ; puis la marie-louise, si elle est allumée, qui masque ce qui déborde (`annotationsui.rs`).
        self.annot_peindre(&painter, view, xf, &layout, over);
        self.selp_peindre(&painter, view, &layout, over);
        self.selb_peindre(&painter, view, over);
        self.marie_peindre(&painter, view, xf, &layout);
        // Des images qu'on glisse vers la planche : la case visée, et le lâcher des références (`deposer.rs`).
        self.depot_survol(ui.ctx(), &painter, xf, &layout, over);
        self.paint_plan_move(ui.ctx(), &painter, view, xf, &layout);
        self.paint_board_drag(&painter, xf);
        // L'outil Caméra : le cadre sous le pointeur se signale, celui de la caméra choisie reste souligné (ignettecamui.rs).
        if self.tool == Tool::Camera && !pan_mode {
            self.paint_cadres_camera(&painter, view, xf, &layout, over);
        }
        // Maj tenue : la peau de la planche, ses poignées (partie 113, proposition A).
        if peau_maj {
            self.peindre_peau_maj(&painter, view, xf, &layout);
        }
        self.plus_peindre(&painter, over);
        if let (Some(p), Tool::Eraser, false) = (over, self.tool, pan_mode) {
            if matches!(hovered, Some(Hit::Nothing)) || matches!(self.board_drag, Some(BoardDrag::Erase { .. } | BoardDrag::EraseDessus { .. })) {
                let r = self.note_eraser_radius(xf.zoom) * xf.zoom;
                painter.circle_stroke(p, r, Stroke::new(1.0_f32, INK));
            }
        }
        bg.context_menu(|ui| match self.vignettes.menu {
            Some(MenuTarget::Card { plan, case, cadre }) => match self.case_du_menu(plan, case) {
                Some((panel, frame)) => self.card_menu(ui, panel, frame, cadre),
                // (La case n'est plus là : le menu se ferme plutôt que de viser une autre case.)
                None => {
                    self.vignettes.menu = None;
                    ui.close_menu();
                }
            },
            Some(MenuTarget::Annule(plan)) => self.annule_menu(ui, plan),
            Some(MenuTarget::Titre(seq)) => {
                self.titre_menu(ui, seq);
                self.board_menu(ui, &layout, &texts);
            }
            _ => self.board_menu(ui, &layout, &texts),
        });
        self.note_text_editor(ui, xf, &layout);
        self.card_note_editor(ui, xf, &layout);
        self.titre_editeur(ui, xf, &layout);
        // Le curseur d'un cadre pris ou à prendre passe avant celui du dessin de la vignette courante (peint plus tôt).
        if self.vignettes.plus.survole {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        } else if self.vignettes.frame_hot {
            ui.ctx().set_cursor_icon(if self.vignettes.moving.as_ref().is_some_and(|m| m.active) { CursorIcon::Grabbing } else { CursorIcon::Grab });
        } else if let (Some(p), true) = (over, ((mods.ctrl || mods.command) && mods.shift) || self.vignettes.camera_tenue || self.tool == Tool::Camera) {
            // Ctrl + Maj (ou T) tenue sur une case : un cadre de caméra à prendre (une petite caméra suit le pointeur).
            if !pan_mode && self.vignettes.cam.is_none() {
                self.cam_curseur(ui.ctx(), &layout, xf, p, mods.alt);
            }
        }
        // Le stylo ou la gomme d'annotation : curseur en croix (le cercle de l'outil est peint avec les annotations).
        if self.annot_actif() && over.is_some() && !pan_mode {
            self.annot_curseur(ui.ctx(), xf, &layout, over);
        }
        if !any_down {
            self.finish_board_drag(&layout, &texts);
        }
        self.vignettes.appui_bande = None;
        self.vignettes.appui_papier = false;
        // Un appui tombé sur une barre : au lâcher sans mouvement, la barre bascule et le point de trait est retiré (décision 11).
        if self.barre_au_lacher(&layout, latest, any_down) {
            ui.ctx().request_repaint();
        }
        if self.numero_au_lacher(latest, any_down) {
            ui.ctx().request_repaint();
        }
        // Le trait qui vient de finir : sa part hors du champ de sa case est répartie (horschamp.rs), une seule étape d'annulation.
        self.horschamp_partager(&layout);
        self.finish_creation(any_down);
        self.finish_plan_move(any_down);
        self.plus_lacher(latest, any_down);
        self.finish_cam_geste(&layout, any_down);
        // Choisir des plans (Ctrl ou Maj + clic) donne la main aux plans : copier, coller, supprimer (12e).
        if self.vignettes.press == Press::Select {
            self.panel_focus = true;
            self.images_focus = false;
            self.key_focus = false;
        }
        // La pile des versions (la même que celle du montage) : survoler l'étiquette « V2 » d'une case l'ouvre au-dessus d'elle, un clic sur une
        // carte passe le plan à cette version (`passer_a_la_version`, la même étape d'annulation que la timeline). Fermée, elle ne coûte rien.
        let occupe = (any_down && !matches!(self.vignettes.press, Press::None | Press::Version)) || self.vignettes.moving.as_ref().is_some_and(|m| m.active) || self.vignettes.cam.as_ref().is_some_and(|g| g.actif);
        let etiquettes = std::mem::take(&mut self.vignettes.etiquettes);
        self.pile_versions_ui(ui, &etiquettes, occupe, view);
        if !any_down {
            self.vignettes.press = Press::None;
        }
        // Les pastilles des personnages, en rangée à l'extrémité droite de la barre du haut de la planche (au-dessus d'elle : ses clics
        // ne vont pas à la planche) ; elle ne prend que la place que les réglages de la planche laissent libre.
        let reglages: Option<Rect> = ui.data(|d| d.get_temp(egui::Id::new("vignettes_reglages")));
        let gauche_libre = reglages.map(|r| r.right() + 10.0).filter(|x| *x < haut_droite.x - 200.0).unwrap_or(view.left() + 380.0);
        self.pastilles_rangee_flottante(ui.ctx(), haut_droite, haut_droite.x - gauche_libre);
    }

    /// Les barres entre les images : une coupe est un trait court noir ; deux cases d'un même plan ont **un point** gris dans leur écart
    /// (partie 113, proposition B : la barre grise d'avant, 1,5 point très pâle, ne se voyait pas sur le papier) ; sous la souris, l'accent et une
    /// bulle. `maj` : Maj tenue, chaque barre se montre en entier, plus épaisse (la planche change de peau).
    #[allow(clippy::too_many_arguments)]
    fn paint_bars(&self, ui: &mut egui::Ui, painter: &egui::Painter, view: Rect, xf: Xf, layout: &BoardLayout, hovered: Option<Hit>, maj: bool) {
        for b in &layout.bars {
            let zone = xf.rect(b.zone);
            if !zone.intersects(view) {
                continue;
            }
            // (La barre de coupe est un peu plus épaisse que la grise : 2 contre 1,5.) À la japonaise, la barre est horizontale : son épaisseur est sa hauteur.
            let width = (b.bar.size().min_elem() * xf.zoom).clamp(1.0, 3.5);
            let hot = hovered == Some(Hit::Bar(b.after));
            let color = if hot {
                crate::theme::accent()
            } else if b.cut {
                BAR_CUT
            } else {
                BAR_JOINED
            };
            let r = xf.rect(b.bar);
            if maj {
                // La peau de Maj : la barre sur toute la hauteur de son écart, épaisse, dans la couleur de son état (gris moyen pour « même plan »).
                let z = zone;
                let c = if hot { crate::theme::accent() } else if b.cut { BAR_CUT } else { Color32::from_gray(170) };
                let trait_ = if layout.sens.japonais() { [pos2(z.left() + 2.0, z.center().y), pos2(z.right() - 2.0, z.center().y)] } else { [pos2(z.center().x, z.top() + 2.0), pos2(z.center().x, z.bottom() - 2.0)] };
                painter.line_segment(trait_, Stroke::new(width + 2.0, c));
            } else if !b.cut && !hot {
                painter.circle_filled(r.center(), (2.4 * xf.zoom).clamp(1.5, 3.5), BAR_POINT);
            } else {
                let trait_ = if layout.sens.japonais() { [pos2(r.left(), r.center().y), pos2(r.right(), r.center().y)] } else { [pos2(r.center().x, r.top()), pos2(r.center().x, r.bottom())] };
                painter.line_segment(trait_, Stroke::new(if hot { width + 1.0 } else { width }, color));
            }
            if hot {
                let resp = ui.interact(board::zone_tactile(zone, layout.sens, BAR_TOUCH), ui.id().with(("planche_barre", b.after)), Sense::hover());
                let (nom, texte) = if layout.vers_pale(b.after) {
                    ("Prochaine case : nouveau plan", "Noire : dessiner dans la case vide fait un nouveau plan. Clic : en faire tout de suite une image de plus du dernier plan, avec son dessin.")
                } else if b.cut {
                    ("Coupe", "Noire : les deux vignettes sont deux plans. Clic : les réunir en un seul plan.")
                } else {
                    ("Même plan", "Grise : la vignette de droite est une image du même plan. Clic : couper le plan ici.")
                };
                crate::bulles::bulle(resp, nom, texte, "");
            }
        }
    }

    /// Clic sur le fond : texte (outil Texte), choix d'une note (Sélection), sinon rien de choisi.
    fn board_click(&mut self, layout: &BoardLayout, texts: &[Option<Rect>], p: Pos2, mods: Modifiers) {
        let tol = 5.0 / self.board_zoom;
        let add = mods.ctrl || mods.shift;
        match self.tool {
            Tool::Text => {
                if self.board_text.is_some() {
                    self.commit_note_text(layout);
                } else if let Some(NoteRef::Text(i)) = self.note_at(layout, texts, p, tol) {
                    self.edit_note_text(layout, i);
                } else {
                    let size = NOTE_TEXT_SIZE;
                    self.board_text = Some(NoteTextEdit { index: None, pos: p, size, buffer: String::new(), focused: false });
                }
            }
            Tool::Select => match self.note_at(layout, texts, p, tol) {
                Some(n) if add => {
                    if let Some(k) = self.board_notes.iter().position(|m| *m == n) {
                        self.board_notes.remove(k);
                    } else {
                        self.board_notes.push(n);
                    }
                }
                Some(n) => self.board_notes = vec![n],
                None if !add => {
                    self.board_notes.clear();
                    self.board_sel.clear();
                    self.vignettes.libre = true;
                    self.vignettes.camera_choisie = None;
                }
                None => {}
            },
            _ => {
                self.commit_note_text(layout);
                if !add {
                    self.board_sel.clear();
                    self.board_notes.clear();
                    // Un clic sur le papier retire le choix : plus de cadre de sélection, plus de caméra choisie.
                    self.vignettes.libre = true;
                    self.vignettes.camera_choisie = None;
                }
            }
        }
    }

    /// Ouvre le texte `i` pour le modifier.
    fn edit_note_text(&mut self, layout: &BoardLayout, i: usize) {
        self.commit_note_text(layout);
        if let Some((pos, size)) = self.doc.board.text_place(layout, i) {
            let buffer = self.doc.board.texts[i].text.clone();
            self.board_text = Some(NoteTextEdit { index: Some(i), pos, size, buffer, focused: false });
        }
    }

    /// Pose le texte en cours (vide : rien, ou le texte existant est retiré). Une étape d'annulation.
    pub(crate) fn commit_note_text(&mut self, layout: &BoardLayout) {
        let Some(ed) = self.board_text.take() else { return };
        let text = ed.buffer.trim_end().to_string();
        match ed.index {
            Some(i) if i < self.doc.board.texts.len() => {
                if text.trim().is_empty() {
                    self.record_step();
                    self.doc.board.remove(&[NoteRef::Text(i)]);
                    self.board_notes.clear();
                } else if self.doc.board.texts[i].text != text {
                    self.record_step();
                    self.doc.board.texts[i].text = text;
                }
            }
            _ if !text.trim().is_empty() => {
                self.record_step();
                let anchor = self
                    .layout
                    .board_attach
                    .then(|| board::nearest_card(layout, Rect::from_min_size(ed.pos, vec2(40.0, ed.size))))
                    .flatten();
                self.doc.board.add_text(layout, ed.pos, text, ed.size, self.color, anchor);
            }
            _ => {}
        }
    }

    /// Le cadre où l'on écrit un texte de note, à sa place sur la planche.
    fn note_text_editor(&mut self, ui: &mut egui::Ui, xf: Xf, layout: &BoardLayout) {
        let Some(ed) = self.board_text.as_mut() else { return };
        let font = FontId::proportional((ed.size * xf.zoom).clamp(8.0, 72.0));
        let mut done = None;
        egui::Area::new(egui::Id::new("planche_texte"))
            .order(egui::Order::Foreground)
            .fixed_pos(xf.to_screen(ed.pos) - vec2(6.0, 6.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let resp = ui.add(
                        egui::TextEdit::multiline(&mut ed.buffer)
                            .font(font)
                            .desired_rows(2)
                            .desired_width(300.0)
                            .hint_text("Note…"),
                    );
                    if !ed.focused {
                        resp.request_focus();
                        ed.focused = true;
                    }
                    let (enter, esc) = ui.input(|i| (i.modifiers.ctrl && i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
                    ui.horizontal(|ui| {
                        use egui_phosphor::regular as ph;
                        if crate::bulles::icone(ui, ph::CHECK, "Poser la note", "Ctrl + Entrée.", "").clicked() || enter {
                            done = Some(true);
                        }
                        if crate::bulles::icone(ui, ph::X, "Annuler", "Annuler la saisie (Échap).", "").clicked() || esc {
                            done = Some(false);
                        }
                    });
                });
            });
        match done {
            Some(true) => self.commit_note_text(layout),
            Some(false) => self.board_text = None,
            None => {}
        }
    }

    /// Les réglages de la planche, petits et pâles : ils ne se montrent qu'à la souris (« la partie Vignettes doit être la
    /// plus épurée possible, rien de distraction »). Taille des vignettes, colonnes, zoom ; « attacher les notes » est au clic
    /// droit sur le fond, et les rangées pâles sont dans Préférences (le papier est infini : plus de pages ni de lignes par page).
    fn board_toolbar(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::{bulle, etiquette};
        use egui_phosphor::regular as ph;
        let memo = egui::Id::new("vignettes_reglages");
        let avant: Option<Rect> = ui.data(|d| d.get_temp(memo));
        let touche = avant.is_some_and(|r| ui.ctx().pointer_hover_pos().is_some_and(|p| r.contains(p)));
        let zone = ui
            .scope(|ui| {
                ui.set_opacity(if touche { 1.0 } else { 0.3 });
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.add_space(6.0);
                    // (La taille des cases n'est plus ici, partie 113, décision 3 : elle faisait double emploi avec le zoom à l'écran ; elle est
                    // dans Préférences > Vignettes, où elle change la planche et son export.)
                    let japonais = self.layout.board_sens.japonais();
                    let (nom, description) = if japonais { ("Cases par colonne", "Vignettes par colonne (6 au plus) : la colonne suivante est à droite.") } else { ("Colonnes", "Vignettes par ligne (6 au plus).") };
                    etiquette(ui, ph::COLUMNS, nom, description);
                    let resp = ui.add(egui::DragValue::new(&mut self.layout.board_columns).range(1..=MAX_COLUMNS).speed(0.05));
                    bulle(resp, nom, description, "");

                });
            })
            .response
            .rect;
        ui.data_mut(|d| d.insert_temp(memo, zone));
    }
    /// Le papier est infini : on le déplace dans tous les sens. Seule une borne très large (quelques fois la planche et la vue)
    /// empêche de se perdre à l'infini ; « Zoom de la planche » (un clic) revient à la page.
    fn clamp_board_pan(&mut self, view: Rect, size: Vec2) {
        let s = size * self.board_zoom;
        let (lx, ly) = (s.x + view.width() * 4.0, s.y + view.height() * 4.0);
        self.board_pan.x = self.board_pan.x.clamp(-lx, lx);
        self.board_pan.y = self.board_pan.y.clamp(-ly, ly);
    }

    fn note_eraser_radius(&self, zoom: f32) -> f32 {
        (self.eraser.size * 0.5 * NOTE_SCALE).max(4.0 / zoom)
    }

    fn update_board_drag(&mut self, layout: &BoardLayout, pointer: Option<Pos2>, zoom: f32) {
        let Some(p) = pointer else { return };
        let radius = self.note_eraser_radius(zoom);
        if let Some(BoardDrag::Rubber { now, .. }) = &mut self.board_drag {
            *now = p;
        }
        // La gomme de la planche efface aussi ce que les cases ont dessiné hors de leur cadre (partie 131, `gommedebord.rs`) : ce qu'on voit sur le
        // papier se gomme sur le papier. Tout le geste ne fait qu'une étape d'annulation (regroupée à son lâcher, `finish_board_drag`).
        if matches!(self.board_drag, Some(BoardDrag::Erase { .. })) {
            self.gomme_debords_commencer();
            self.gomme_debords(layout, p, radius);
        }
        // Gomme et déplacement de notes : l'étape d'annulation au premier changement du geste.
        if let Some(BoardDrag::Erase { recorded }) = self.board_drag {
            if self.doc.board.stroke_at(layout, p, radius).is_some() {
                if !recorded {
                    self.record_step();
                    self.board_drag = Some(BoardDrag::Erase { recorded: true });
                }
                self.doc.board.erase_at(layout, p, radius);
                self.board_notes.clear();
            }
        }
        if let Some(BoardDrag::EraseDessus { recorded }) = self.board_drag {
            if self.doc.annotations.touche_dessus(layout, p, radius) {
                if !recorded {
                    self.record_step();
                    self.board_drag = Some(BoardDrag::EraseDessus { recorded: true });
                }
                self.doc.annotations.effacer_dessus(layout, p, radius);
                self.modified = true;
            }
        }
        // Hors de toute case, la gomme efface aussi les traits du dessin hors des cases (le trait entier que touche le cercle), s'ils sont montrés.
        if let (true, Some(id)) = (matches!(self.board_drag, Some(BoardDrag::Erase { .. })), self.doc.annotations.hors_cadres_montre()) {
            if self.doc.annotations.touche_calque(layout, id, p, radius) {
                if !matches!(self.board_drag, Some(BoardDrag::Erase { recorded: true })) {
                    self.record_step();
                    self.board_drag = Some(BoardDrag::Erase { recorded: true });
                }
                self.doc.annotations.effacer_en_calque(layout, id, p, radius);
                self.modified = true;
            }
        }
        if let Some(BoardDrag::MoveNotes { last, recorded }) = self.board_drag {
            let delta = p - last;
            if delta != Vec2::ZERO {
                if !recorded {
                    self.record_step();
                }
                self.board_drag = Some(BoardDrag::MoveNotes { last: p, recorded: true });
                let sel = self.board_notes.clone();
                self.doc.board.translate(layout, &sel, delta);
            }
        }
        // (Le compte d'étapes après tout ce que cette image du geste a posé : la gomme sait ainsi si on a annulé pendant le geste.)
        self.gomme_debords_noter();
    }

    fn finish_board_drag(&mut self, layout: &BoardLayout, texts: &[Option<Rect>]) {
        if let Some(BoardDrag::Rubber { start, now, add }) = self.board_drag.take() {
            let r = Rect::from_two_pos(start, now);
            let found: Vec<u64> = board::cards_in(layout, r).into_iter().map(|i| self.doc.panels[i].id).collect();
            // (Un lasso qui prend des plans est un choix voulu : le cadre de sélection revient.)
            if !found.is_empty() {
                self.vignettes.libre = false;
            }
            let mut notes = self.doc.board.strokes_in(layout, r);
            notes.extend((0..texts.len()).filter(|&i| texts[i].is_some_and(|t| t.intersects(r))).map(NoteRef::Text));
            if add {
                for id in found {
                    if !self.board_sel.contains(&id) {
                        self.board_sel.push(id);
                    }
                }
                for n in notes {
                    if !self.board_notes.contains(&n) {
                        self.board_notes.push(n);
                    }
                }
            } else {
                if let Some(&first) = found.first() {
                    if let Some(i) = self.doc.panels.iter().position(|p| p.id == first) {
                        self.select_panel(i);
                    }
                }
                self.board_sel = found;
                self.board_notes = notes;
            }
        } else {
            self.board_drag = None;
        }
        // (La gomme de la planche : tout ce que le geste a effacé, papier et dessins des cases, ne fait qu'une étape.)
        self.gomme_debords_finir();
    }

    /// La tuile de l'empreinte d'un mouvement de caméra (Anthony, 2026-10-22 : tout l'espace entre A et B se dessine) : une seule
    /// image qui couvre l'enveloppe des cadres. Le plan courant y est vivant (le dessin lui-même, vu par la zone de la feuille :
    /// un trait y tombe sous le stylet, ailleurs que dans les cadres comme dedans) ; les autres plans montrent leur vignette de
    /// tuile, composée sur un fil de fond. Hors de l'enveloppe, la tuile ne garde que le trait qui déborde. Peinte avant les cases :
    /// son rectangle déborde de l'enveloppe, les cases qui s'y trouvent passent par-dessus.
    #[allow(clippy::too_many_arguments)]
    fn board_tuile(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, view: Rect, xf: Xf, layout: &BoardLayout, k: usize, pan_mode: bool, case: Option<crate::thumbs::CaseTex>, plans: usize) {
        let Some(mv) = layout.move_of(k) else { return };
        let place = &layout.cards[k];
        let r = xf.rect(mv.tuile.rect);
        if !r.intersects(view) {
            return;
        }
        let i = place.panel;
        let current = i == self.cur_panel && {
            let f = self.cur_frame.min(self.frame_count().saturating_sub(1));
            (place.frame..place.end).contains(&f)
        };
        let live = current && !pan_mode;
        let hull: Vec<Pos2> = mv.enveloppe.iter().map(|p| xf.to_screen(*p)).collect();
        // La texture est-elle celle de la géométrie d'aujourd'hui (même zone, même enveloppe) ? Pendant un geste de caméra et juste après,
        // elle est celle d'avant (la tuile se recompose sur un fil de fond, jamais sur celui de l'interface) : posée telle quelle, ses
        // bords de coupe et son papier seraient ceux d'avant le geste.
        let a_jour = case.as_ref().is_some_and(|c| c.geom == Some(crate::thumbs::Geom::de(&mv.tuile.zone, &mv.tuile.masque)));
        let pts_par_feuille = mv.tuile.zone.echelle * xf.zoom;
        if live {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r).id_salt("planche_vive_tuile"));
            child.set_clip_rect(r.intersect(view));
            self.vignettes.zone = Some(mv.tuile.zone);
            self.vignettes.enveloppe = hull.clone();
            self.card_canvas(&mut child, r, place.id);
            self.vignettes.zone = None;
            self.vignettes.enveloppe.clear();
            // (Le menu du clic droit du dessin a pu changer le film : comme dans `board_card`.)
            if !self.case_tient(layout, plans, k) {
                ui.ctx().request_repaint();
                return;
            }
            // Hors de l'enveloppe (les coins de la tuile) : le papier de la planche, et ce que la vignette garde du trait qui déborde.
            // Tant que la tuile n'est pas refaite pour les nouveaux cadres, rien de tout cela : le dessin vivant montre tout le trait
            // dans la tuile, rien n'est coupé à une ligne d'avant le geste ; la marge revient dès que la tuile est prête.
            if let Some(c) = case.as_ref().filter(|_| a_jour) {
                let clip = painter.with_clip_rect(r.intersect(view));
                let coins = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()];
                let dehors = crate::viewbar::difference_convexe(&coins, &hull);
                clip.add(crate::viewbar::mesh_of_pieces(&dehors, PAPER));
                let full = c.rect(r);
                let mut mesh = egui::Mesh::with_texture(c.id);
                for piece in dehors.iter().filter(|p| p.len() >= 3) {
                    let base = mesh.vertices.len() as u32;
                    for p in piece {
                        mesh.vertices.push(egui::epaint::Vertex { pos: *p, uv: pos2((p.x - full.min.x) / full.width().max(1e-3), (p.y - full.min.y) / full.height().max(1e-3)), color: Color32::WHITE });
                    }
                    for j in 1..piece.len() as u32 - 1 {
                        mesh.add_triangle(base, base + j, base + j + 1);
                    }
                }
                clip.add(egui::Shape::mesh(mesh));
            }
        } else {
            painter.add(egui::Shape::convex_polygon(hull.clone(), Color32::WHITE, Stroke::NONE));
            if let Some(c) = &case {
                // Posée là où est la feuille qu'elle montre, même si les cadres ont bougé depuis qu'elle a été composée.
                // Le dessin dans l'enveloppe, par-dessus les marges des autres ; sa propre marge est déjà sur le papier (`board_marges`).
                let full = c.rect_tuile(r, &mv.tuile.zone, pts_par_feuille);
                peint_texture_dans(painter, c.id, full, &hull);
            }
        }
    }

    /// La vignette de chaque case à l'écran (une seule demande par image d'écran : les cases s'en servent ensuite). `peindre` (le film, sous le
    /// passe-partout) : leurs marges sont peintes ici, sur le papier, avant toute image (Anthony, 2026-11-06 : « des lignes qui se coupent entre
    /// les plans ») : ce qui dépasse d'un cadre passe sous l'image des autres cases, et le passe-partout, peint plus tard, le cache comme tout
    /// ce que la caméra ne montre pas. Sinon (la feuille, partie 131) elles se peignent après les images, par-dessus : `board_debords`. La même
    /// texture que la case, aucune recomposition.
    #[allow(clippy::too_many_arguments)]
    fn board_marges(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, view: Rect, xf: Xf, layout: &BoardLayout, pan_mode: bool, deadline: Instant, plans: usize, peindre: bool) -> Vec<Option<crate::thumbs::CaseTex>> {
        let ppp = ui.ctx().pixels_per_point();
        let debord = self.settings.vignettes.debord_fraction();
        // Une case montre son dessin jusqu'où il va (partie 131) : sa vignette compte dès que son cadre et sa marge touchent la vue, comme avant,
        // **ou que son dessin y arrive de plus loin**, dans la limite de ce qu'une vignette peut couvrir (`render::MARGE_MAX_CADRES` cadres de
        // chaque côté). Pour une case hors de la vue, cela se sait sans la demander : la boîte de l'encre de son plan (`Thumbs::debord_de_case`,
        // gardée tant que le projet ne change pas).
        let loin = crate::render::MARGE_MAX_CADRES as f32;
        #[allow(clippy::too_many_arguments)]
        let compte = |thumbs: &mut crate::thumbs::Thumbs, doc: &crate::document::Document, zone: Rect, largeur_case: f32, panel: usize, frame: usize, id: u64, tuile: Option<&crate::vignettecam::Zone>| -> bool {
            if zone.expand(debord * largeur_case + 1.0).intersects(view) {
                return true;
            }
            if !zone.expand2(zone.size() * loin + vec2(1.0, 1.0)).intersects(view) {
                return false;
            }
            let d = thumbs.debord_de_case(doc, panel, frame, id, tuile).map(|v| v.min(loin));
            Rect::from_min_max(zone.min - vec2(d[0] * zone.width(), d[1] * zone.height()), zone.max + vec2(d[2] * zone.width(), d[3] * zone.height())).expand(2.0).intersects(view)
        };
        let cur = self.cur_frame.min(self.frame_count().saturating_sub(1));
        let toute = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        let mut cases = vec![None; layout.cards.len()];
        for k in 0..layout.cards.len() {
            if !self.case_tient(layout, plans, k) {
                break;
            }
            let place = &layout.cards[k];
            let i = place.panel;
            let thumb = xf.rect(place.thumb);
            let courante = i == self.cur_panel && (place.frame..place.end).contains(&cur) && !pan_mode;
            if let Some(mv) = layout.move_of(k) {
                let r = xf.rect(mv.tuile.rect);
                if !compte(&mut self.thumbs, &self.doc, r, thumb.width(), i, place.frame, place.id, Some(&mv.tuile.zone)) {
                    continue;
                }
                let width = crate::thumbs::width_for(r.width() * ppp);
                // (La marge de la tuile est 12 % de la largeur d'une case, comme celle des autres cases, pas 12 % de la tuile entière.)
                let part_case = place.thumb.width() / mv.tuile.rect.width().max(1e-3);
                let case = self.thumbs.texture_case(ui.ctx(), &self.doc, i, place.frame, place.id, width, Some((&mv.tuile.zone, &mv.tuile.masque, part_case)), deadline);
                if let Some(c) = case.as_ref().filter(|_| peindre) {
                    let a_jour = c.geom == Some(crate::thumbs::Geom::de(&mv.tuile.zone, &mv.tuile.masque));
                    if courante {
                        // (Le dessin vivant remplit la tuile : seule sa bordure reste à peindre, et pas d'une texture d'avant le geste.)
                        if a_jour {
                            paint_ring(painter, r, c);
                        }
                    } else {
                        painter.image(c.id, c.rect_tuile(r, &mv.tuile.zone, mv.tuile.zone.echelle * xf.zoom), toute, Color32::WHITE);
                    }
                }
                cases[k] = case;
            } else {
                if !compte(&mut self.thumbs, &self.doc, thumb, thumb.width(), i, place.frame, place.id, None) {
                    continue;
                }
                let width = crate::thumbs::width_for(thumb.width() * ppp);
                let case = self.thumbs.texture_case(ui.ctx(), &self.doc, i, place.frame, place.id, width, None, deadline);
                if let Some(c) = case.as_ref().filter(|_| peindre) {
                    paint_ring(painter, thumb, c);
                }
                cases[k] = case;
            }
        }
        cases
    }

    /// **La feuille** (partie 131) : ce que chaque case a dessiné hors de son cadre, peint **par-dessus les images de toutes les cases** (après
    /// elles, avant leurs filets, leurs numéros et leurs notes) : sur la planche, le bord d'une case ne coupe rien et ne cache rien. `cases` :
    /// les vignettes de `board_marges`. Une case ordinaire : tout ce que sa vignette porte autour de son cadre. La case d'un mouvement de
    /// caméra : tout ce que sa tuile porte hors de son enveloppe (dedans, c'est son image, déjà peinte ; la tuile de la case courante n'est
    /// posée que si elle est celle de la géométrie d'aujourd'hui, comme dans `board_tuile`).
    fn board_debords(&self, painter: &egui::Painter, view: Rect, xf: Xf, layout: &BoardLayout, cases: &[Option<crate::thumbs::CaseTex>], pan_mode: bool) {
        let cur = self.cur_frame.min(self.frame_count().saturating_sub(1));
        for (k, c) in cases.iter().enumerate() {
            let (Some(c), Some(place)) = (c, layout.cards.get(k)) else { continue };
            match layout.move_of(k) {
                None => {
                    let thumb = xf.rect(place.thumb);
                    if c.rect(thumb).intersects(view) {
                        paint_ring(painter, thumb, c);
                    }
                }
                Some(mv) => {
                    let courante = place.panel == self.cur_panel && (place.frame..place.end).contains(&cur) && !pan_mode;
                    if courante && c.geom != Some(crate::thumbs::Geom::de(&mv.tuile.zone, &mv.tuile.masque)) {
                        continue;
                    }
                    let full = c.rect_tuile(xf.rect(mv.tuile.rect), &mv.tuile.zone, mv.tuile.zone.echelle * xf.zoom);
                    if !full.intersects(view) {
                        continue;
                    }
                    let hull: Vec<Pos2> = mv.enveloppe.iter().map(|p| xf.to_screen(*p)).collect();
                    let coins = [full.left_top(), full.right_top(), full.right_bottom(), full.left_bottom()];
                    let dehors = crate::viewbar::difference_convexe(&coins, &hull);
                    let mut mesh = egui::Mesh::with_texture(c.id);
                    for piece in dehors.iter().filter(|p| p.len() >= 3) {
                        let base = mesh.vertices.len() as u32;
                        for p in piece {
                            mesh.vertices.push(egui::epaint::Vertex { pos: *p, uv: pos2((p.x - full.min.x) / full.width().max(1e-3), (p.y - full.min.y) / full.height().max(1e-3)), color: Color32::WHITE });
                        }
                        for j in 1..piece.len() as u32 - 1 {
                            mesh.add_triangle(base, base + j, base + j + 1);
                        }
                    }
                    painter.add(egui::Shape::mesh(mesh));
                }
            }
        }
    }

    /// Une vignette (une image de storyboard) : son numéro de plan s'il commence, l'image (le dessin lui-même
    /// pour l'image courante), son filet, sa bande de note. Les gestes sont pris plus haut (`board_press`).
    #[allow(clippy::too_many_arguments)]
    fn board_card(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, view: Rect, xf: Xf, layout: &BoardLayout, k: usize, pan_mode: bool, case: Option<crate::thumbs::CaseTex>, numeros: &crate::numerotation::Numeros, marges: &mut crate::numerosmarge::Marges, plans: usize, passe: PasseCase) {
        let place = &layout.cards[k];
        let mv = layout.move_of(k);
        let card = xf.rect(place.card);
        let thumb = xf.rect(place.thumb);
        let note = xf.rect(place.note);
        if !card.intersects(view) && !note.intersects(view) && !xf.rect(place.foot).intersects(view) {
            return;
        }
        let i = place.panel;
        let plan_current = i == self.cur_panel;
        // L'image courante : celle du plan courant où est la tête de lecture.
        let current = plan_current && {
            let f = self.cur_frame.min(self.frame_count().saturating_sub(1));
            (place.frame..place.end).contains(&f)
        };
        // Plusieurs plans choisis (Ctrl, Maj, lasso) : toutes leurs images, sauf l'image courante, ont un liseré d'accent.
        let cur_id = self.doc.panels[self.cur_panel].id;
        let chosen_more = !current && self.board_sel.iter().any(|id| *id != cur_id) && (plan_current || self.board_sel.contains(&place.plan));
        let accent = crate::theme::accent();

        // Le panneau courant : le dessin lui-même ; les autres : leur vignette.
        let live = current && !pan_mode;
        let clip = painter.with_clip_rect(view);
        // (Deux passes, partie 131 : les images de toutes les cases d'abord ; leurs filets, leurs numéros et leurs notes après ce que la planche
        // peint par-dessus les images, le papier et le dessin des cases hors de leur cadre.)
        match mv {
            _ if passe == PasseCase::Habillage => {}
            // La case d'un plan dont la caméra bouge (deux clés au moins) montre ses cadres à l'échelle des autres cases : une tuile
            // par rangée de l'empreinte, qui montre la feuille et ce qu'on a dessiné autour d'elle (`vignettecam.rs`) ; l'outil Caméra,
            // lui, recadre à l'ancienne (une seule image).
            Some(_) => {}
            None => {
                // (La marge de la case est déjà sur le papier : `board_marges`. Ici, son image seule.)
                if live {
                    // La vue caméra laisse une marge autour du cadre : on l'agrandit d'autant (et on coupe
                    // à la vignette) pour que l'image remplisse la vignette. Pour recadrer, toute la feuille.
                    let area = Rect::from_center_size(thumb.center(), thumb.size() / 0.96);
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area).id_salt("planche_vive"));
                    child.set_clip_rect(thumb.intersect(view));
                    self.card_canvas(&mut child, thumb, place.id);
                    // Le menu du clic droit du dessin s'ouvre là-dedans (`canvas_panel`) : « Réunir avec le plan précédent », « Couper »…
                    // peuvent avoir changé le film. La suite de la case lit `place` et `i`, de la planche du début de l'image : s'ils ne
                    // désignent plus le même plan, la case s'arrête là (la planche se refait à l'image suivante).
                    if !self.case_tient(layout, plans, k) {
                        ui.ctx().request_repaint();
                        return;
                    }
                } else {
                    painter.rect_filled(thumb, 0.0, Color32::WHITE);
                    if let Some(c) = &case {
                        peint_texture_dans(painter, c.id, c.rect(thumb), &[thumb.left_top(), thumb.right_top(), thumb.right_bottom(), thumb.left_bottom()]);
                    }
                }
            }
        }
        if passe == PasseCase::Image {
            return;
        }
        // Le filet d'un point autour de l'image ; plus foncé pour l'image courante. Rien d'autre autour.
        // (Épaisseur et couleur du filet : Préférences > Vignettes ; la case courante est plus foncée.)
        let filet = match self.settings.vignettes.couleur_filet {
            Some([r, g, b]) => Color32::from_rgb(r, g, b),
            None if current && !self.vignettes.libre => BORDER_CURRENT,
            None => BORDER,
        };
        let filet = if current && !self.vignettes.libre && self.settings.vignettes.couleur_filet.is_some() { filet.gamma_multiply(0.7) } else { filet };
        clip.rect_stroke(thumb, 0.0, Stroke::new(self.settings.vignettes.epaisseur_cadre, filet), StrokeKind::Outside);
        // Les cadres du mouvement de caméra (A, B, C…), le voile hors de A et ce qui les relie, fins, par-dessus.
        if let Some(mv) = mv {
            self.paint_empreinte(painter, view, xf, place, mv);
        }
        // Un plan choisi, seul ou parmi d'autres : le même liseré d'accent (partie 113, décision 10 : le filet un peu plus foncé seul ne se
        // voyait pas).
        if chosen_more || (current && !self.vignettes.libre) {
            clip.rect_stroke(thumb.expand(1.5), 0.0, Stroke::new(1.5_f32, accent), StrokeKind::Outside);
        }
        self.paint_card_note(&clip, xf, place, note);
        self.paint_note_icone(ui, painter, view, place);
        // Le numéro du plan, dans la marge au-dessus de sa première image, peint après l'image : la marge du dessin de la case courante ne le
        // recouvre pas (2026-10-30). Sa place est fixe (le coin haut-gauche de A) tant qu'elle est libre ; sinon `numerosmarge.rs` en trouve une
        // qui ne recouvre ni image, ni cadre, ni zone balayée, ni texte (2026-11-06). Pas de pastille d'envoi : elles sont au montage.
        if place.starts_plan() && card.height() > thumb.height() + 3.0 {
            // Le numéro suit le zoom de la planche mais reste lisible de très loin comme de très près (Anthony, 2026-10-30) : la taille à
            // l'écran est la taille d'origine (`LABEL_H` × `NUMERO_FRACTION`) multipliée par le zoom, bornée entre 15 et 72 points d'écran
            // (avant : 9 minimum, illisible à six cases par rangée), le tout multiplié par le réglage « Taille des numéros »
            // (Préférences > Vignettes). Un fond clair derrière (réglable) le garde lisible sur n'importe quel dessin qui déborde. Quand la
            // marge est trop étroite pour cette taille (petit zoom), il s'amincit plutôt que de passer sur une image.
            let reglages = &self.settings.vignettes;
            let echelle = reglages.numeros_echelle();
            let font0 = crate::vignettesprefs::taille_numero(xf.zoom, echelle);
            let ink = Color32::from_gray(15);
            let clip = painter.with_clip_rect(view);
            // (Maj tenue montre les numéros cachés : on peut cliquer dessus pour annuler ou rétablir, `planannuleui.rs`.)
            let afficher = reglages.numeros_visibles || self.vignettes.peau_maj;
            let galley0 = clip.layout_no_wrap(numeros.scene_label(&self.doc, place.scene), FontId::proportional(font0), ink);
            let marge0 = (font0 * 0.18).max(2.0);
            // La durée du plan (réglage), la pastille des notes à traiter et l'étiquette de version suivent le numéro : leur place compte aussi.
            let duree = reglages.afficher_duree.then(|| {
                let n = self.doc.panels.get(i).map_or(0, |p| p.frame_count());
                let fps = self.doc.fps.max(1) as usize;
                let texte = format!("{}+{:02}", n / fps, n % fps);
                let g = clip.layout_no_wrap(texte.clone(), FontId::proportional((font0 * 0.6).max(10.0)), Color32::from_gray(90));
                (texte, g.size().x, (font0 * 0.6).max(10.0))
            });
            let marque = if self.doc.comments.is_empty() { None } else { self.doc.plan_mark(place.plan).filter(|m| m.counts.open() > 0) };
            let avec_version = reglages.pastille_version && crate::pilevers::etiquette_visible(self.doc.nombre_de_versions(place.plan));
            let fond_numeros = reglages.fond_numeros;
            let rayon = |sc: f32| (font0 * 0.42).clamp(7.0, 12.0) * sc;
            // (Taille de tout ce qui se pose à la suite, à l'échelle `sc` ; rien sous le plus petit corps lisible.)
            let mesure = |sc: f32| -> Vec2 {
                if font0 * sc < crate::numerosmarge::CORPS_MIN {
                    return Vec2::ZERO;
                }
                let haut = galley0.size().y * sc + marge0 * sc * 2.0;
                let mut large = if afficher { galley0.size().x * sc + marge0 * sc * 2.0 + 4.0 } else { 0.0 };
                if let Some((_, w, df)) = &duree {
                    large += w * ((font0 * sc * 0.6).max(10.0 * sc) / df) + 6.0;
                }
                if marque.is_some() {
                    large += rayon(sc) * 2.0 + 6.0;
                }
                if avec_version {
                    large += 30.0;
                }
                if !afficher && duree.is_none() && marque.is_none() && !avec_version { Vec2::ZERO } else { vec2(large, haut) }
            };
            // (La zone du plan : celle de son mouvement de caméra, qui peut monter au-dessus de A.)
            let haut_zone = layout.move_of(k).map_or(thumb.top(), |m| m.enveloppe.iter().map(|q| xf.to_screen(*q).y).fold(thumb.top(), f32::min));
            if let Some((libre, sc)) = marges.choisir(thumb, haut_zone, mesure) {
                let font = font0 * sc;
                let marge = marge0 * sc;
                let galley = clip.layout_no_wrap(numeros.scene_label(&self.doc, place.scene), FontId::proportional(font), ink);
                let pastille = Rect::from_min_size(libre.min, if afficher { galley.size() + vec2(marge * 2.0, marge * 2.0) } else { vec2(0.0, galley.size().y + marge * 2.0) });
                if afficher {
                    if fond_numeros {
                        clip.rect_filled(pastille, marge, Color32::from_white_alpha(235));
                        clip.rect_stroke(pastille, marge, Stroke::new(1.0_f32, Color32::from_gray(190)), StrokeKind::Inside);
                    }
                    let at = pastille.min + vec2(marge, marge);
                    let gras = (font * 0.035).max(0.4);
                    for d in [vec2(0.0, 0.0), vec2(gras, 0.0), vec2(0.0, gras)] {
                        clip.galley(at + d, galley.clone(), ink);
                    }
                }
                if afficher {
                    self.vignettes.numeros_zones.push((pastille.expand(2.0), place.plan, false));
                }
                let mut droite = if afficher { pastille.right() + 4.0 } else { pastille.left() };
                // La durée du plan, à côté du numéro (réglage), en petit.
                if let Some((texte, _, _)) = &duree {
                    let g = clip.layout_no_wrap(texte.clone(), FontId::proportional((font * 0.6).max(10.0 * sc)), Color32::from_gray(90));
                    clip.galley(pos2(droite, pastille.center().y - g.size().y * 0.5), g.clone(), Color32::from_gray(90));
                    droite += g.size().x + 6.0;
                }
                // Des notes encore à traiter sur ce plan : la même petite pastille qu'au montage, hors du cadre, entre la durée et la
                // version (`fildecisionui.rs`). Rien sans note à traiter.
                if let Some(m) = marque {
                    let r = rayon(sc);
                    let centre = pos2(droite + r + 1.0, pastille.center().y);
                    crate::fildecisionui::peindre_marque(&clip, centre, &m, r);
                    if ui.rect_contains_pointer(Rect::from_center_size(centre, vec2(r * 2.0 + 2.0, r * 2.0 + 2.0)).intersect(view)) {
                        egui::show_tooltip_at_pointer(ui.ctx(), ui.layer_id(), egui::Id::new(("note_marque", place.plan)), |ui| {
                            ui.label(crate::fildecisionui::marque_phrase(&m));
                        });
                    }
                    droite += r * 2.0 + 6.0;
                }
                // L'étiquette de version (« V2 »), **hors du cadre**, juste à droite du numéro : jamais sur l'image. Rien pour un plan à une
                // seule version, ni si le réglage est éteint.
                if avec_version {
                    let haut = (font0 * 0.75).clamp(14.0, 28.0);
                    if let Some(e) = self.peindre_etiquette_hors_cadre(&clip, pos2(droite, pastille.center().y), haut, i) {
                        self.vignettes.etiquettes.push(e);
                    }
                }
            }
        }

        // (L'étiquette de version « V2 » est hors du cadre, à côté du numéro du plan : plus haut. La survoler ouvre la pile des versions,
        // `pile_versions_ui`, en fin d'image.)
    }
    /// Le texte sous l'image (à droite à la japonaise) : **une ligne par champ coché** (action, dialogue, notes, dans l'ordre de la fiche),
    /// chacune coupée par « … » si le texte est trop long, et un filet très pâle dessous. Aucun champ coché : rien du tout. Pendant la
    /// saisie, l'éditeur remplace la ligne Notes (`card_note_editor`).
    fn paint_card_note(&self, painter: &egui::Painter, xf: Xf, place: &CardPlace, strip: Rect) {
        if !place.a_du_texte() {
            return;
        }
        // Sous la zone, un filet très pâle (sans cadre : la maquette) ; le texte, petit et gris, au-dessus. (À la japonaise la zone
        // est à droite de la case, le filet est sous sa dernière ligne de même.)
        let y = strip.bottom() - 1.0;
        painter.line_segment([pos2(strip.left(), y), pos2(strip.right(), y)], Stroke::new(1.0_f32, HAIRLINE));
        let textes = self.layout.board_textes;
        let edite = self.vignettes.note.as_ref().filter(|e| e.id == place.id).map(|e| e.champ);
        let width = (place.note.width() - NOTE_PAD * 2.0) * xf.zoom;
        for (i, champ) in textes.champs().into_iter().enumerate() {
            if edite == Some(champ) {
                continue;
            }
            // Chaque ligne cochée est toujours visible, même vide (Anthony, 2026-10-30) : son intitulé (« Note », « Action », « Dialogue »)
            // dans sa couleur propre (Préférences > Vignettes), puis le texte du champ.
            let text = crate::boardsens::texte_du_champ(&self.doc, place.panel, place.id, place.frame, champ);
            let size = board::note_font_size(text.chars().count()) * xf.zoom;
            if size < 4.0 {
                return;
            }
            let [r, g, b] = self.settings.vignettes.couleur_texte(champ);
            let y = strip.min.y + i as f32 * board::NOTE_LINE_H * xf.zoom;
            // Un filet pâle de sa couleur sous la ligne : on voit où elle est, même vide.
            let ligne_y = y + board::NOTE_LINE_H * xf.zoom - 1.0;
            painter.line_segment([pos2(strip.left(), ligne_y), pos2(strip.right(), ligne_y)], Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(r, g, b, 70)));
            let galley = painter.layout_job(champ_job(champ, &crate::boardsens::une_ligne(text), size, width, Color32::from_rgb(r, g, b)));
            painter.galley(pos2(strip.min.x + NOTE_PAD * xf.zoom, y), galley, NOTE_INK);
        }
    }

    /// Le dessin dans la vignette du panneau courant : vue à travers la caméra, cadrée sur la
    /// vignette ; avec l'outil Caméra, toute la feuille et le cadre, pour recadrer. La vue du
    /// dessin en grand est remise telle quelle ensuite.
    fn card_canvas(&mut self, ui: &mut egui::Ui, thumb: Rect, carte: u64) {
        let (view, camera_view, canvas_rect) = (self.view, self.camera_view, self.canvas_rect);
        // (L'outil Caméra des Vignettes tire les cadres A, B, C… sur la planche, il ne recadre plus la feuille dans la case : oard_press.)
        let reframe = false;
        // (Une grande case de mouvement de caméra montre la feuille, pas la vue à travers la caméra : `vignettes.zone`.)
        self.camera_view = !reframe && self.vignettes.zone.is_none();
        if reframe {
            self.view.fit(ui.max_rect(), self.canvas_size());
        }
        self.in_card = true;
        self.canvas_panel(ui);
        self.in_card = false;
        // (Comment le geste se pose sur la planche, pour le répartir au lâcher : horschamp.rs.)
        self.horschamp_suivre(carte);
        // Le trait qui déborde de la case, tant que la main est en bas : calculé ici, pendant que la vue du dessin est
        // celle de la vignette ; peint en dernier par la planche (`debord.rs`). Rien si le geste reste dans la case.
        let shift = ui.input(|i| i.modifiers.shift);
        self.vignettes.debord = if self.stroke.is_some() || self.shape_drag.is_some() || self.shape_edit.is_some() { self.debord_collect(thumb, (!self.vignettes.enveloppe.is_empty()).then_some(self.vignettes.enveloppe.as_slice()), shift) } else { None };
        (self.view, self.camera_view, self.canvas_rect) = (view, camera_view, canvas_rect);
    }

    /// Menu d'une vignette (clic droit) : sur son plan (l'image qui commence à `frame`), ou sur
    /// tous les plans choisis.
    pub(crate) fn card_menu(&mut self, ui: &mut egui::Ui, i: usize, frame: usize, cadre: Option<usize>) {
        let item = |app: &mut App, ui: &mut egui::Ui, text: &str, f: &dyn Fn(&mut App)| {
            if ui.button(text).clicked() {
                ui.close_menu();
                app.select_board(i, frame);
                f(app);
            }
        };
        item(self, ui, "Animer cette image", &|a| {
            a.set_mode(crate::app::Mode::Animation);
        });
        // Les réglages de l'outil sous le curseur (`reglagesdroit.rs`) : sur une case, le clic droit garde son menu, qui les offre.
        if self.reglages_droit_permis() && ui.button("Réglages de l'outil…").clicked() {
            ui.close_menu();
            if let Some(p) = ui.ctx().pointer_latest_pos() {
                self.ouvrir_reglages_droit(p);
            }
        }
        // La caméra du plan : un cadre de plus (C, D… : le glissé ne crée plus rien), ou celui qu'on a visé en moins.
        let possible = frame == 0 || self.doc.panels.get(i).is_some_and(|p| p.camera.len() < 2);
        if let Some(rang) = cadre.filter(|_| possible) {
            item(self, ui, "Retirer ce cadre", &move |a| a.cam_retirer_cadre(i, rang));
        }
        if possible {
            item(self, ui, "Ajouter un cadre", &|a| a.cam_ajouter_cadre(i));
        }
        // Film partagé : prendre, rendre, reprendre (`cooperationui.rs`), en une seule entrée.
        if self.partage.is_some() {
            ui.menu_button("Équipe", |ui| self.equipe_menu_items(ui, i));
        }
        item(self, ui, "Nouveau plan après", &|a| {
            a.new_panel();
            a.vignettes_montrer_le_plan_courant();
        });
        item(self, ui, "Dupliquer le plan", &|a| {
            a.duplicate_panel();
            a.vignettes_montrer_le_plan_courant();
        });
        ui.separator();
        self.panel_clipboard_items(ui);
        ui.separator();
        if frame > 0 {
            item(self, ui, "Faire de cette image un nouveau plan", &|a| a.split_current_panel());
        } else if i > 0 {
            item(self, ui, "Réunir avec le plan précédent", &|a| a.join_previous_scene());
        }
        // Les versions du plan, avec « Nouvelle version › » dedans ; le rare sous « Plus » (une dizaine d'entrées au
        // premier niveau, simplification des barres, 2026-10-21).
        self.version_menu(ui, i, true);
        ui.menu_button("Plus", |ui| {
            // (Le geste de tous les jours est le clic sur le numéro, ou Maj + clic ; ceci est le chemin de secours : `planannuleui.rs`.)
            if let Some(plan) = self.doc.panels.get(i).map(|p| p.id) {
                if ui.button(crate::planannuleui::MENU_ECARTER).clicked() {
                    ui.close_menu();
                    self.plan_ecarter_ou_reprendre(plan, true);
                }
            }
            // Les séquences (`planchesequencesui.rs`) : une neuve, avec les cases choisies, couper ici, renommer.
            ui.menu_button("Séquence", |ui| self.sequence_menu_items(ui));
            self.plan_color_menu(ui);
            // Plusieurs plans choisis : une séquence à eux, une durée pour tous (`sequenceplans.rs`, `dureeplans.rs`).
            let n = self.board_selection().len();
            if n > 1 {
                self.action_item(ui, &format!("Nouvelle séquence avec les {n} plans choisis"), "sequence.nouvelle_avec_plans");
                self.action_item(ui, &format!("Changer la durée des {n} plans choisis…"), "montage.duree_plans");
            }
            item(self, ui, "Recadrer (outil Caméra)", &|a| a.set_tool(Tool::Camera));
            item(self, ui, "Réinitialiser le cadrage", &|a| a.run_action("camera.reinitialiser"));
        });
    }

    /// Menu du clic droit sur le dessin d'une vignette (la vignette courante reçoit le clic avant le fond).
    pub(crate) fn card_context_menu(&mut self, ui: &mut egui::Ui) {
        match self.vignettes.menu {
            Some(MenuTarget::Card { plan, case, cadre }) => match self.case_du_menu(plan, case) {
                Some((panel, frame)) => self.card_menu(ui, panel, frame, cadre),
                None => {
                    self.vignettes.menu = None;
                    ui.close_menu();
                }
            },
            _ => {
                let (panel, frame) = (self.cur_panel, self.cur_frame);
                self.card_menu(ui, panel, frame, None);
            }
        }
    }

    /// Menu du fond de la planche (clic droit) : les notes choisies, puis la planche.
    fn board_menu(&mut self, ui: &mut egui::Ui, layout: &BoardLayout, texts: &[Option<Rect>]) {
        if !self.board_notes.is_empty() {
            let sel = self.board_notes.clone();
            // Les notes choisies : un seul sous-menu (menu court, simplification des barres, 2026-10-21).
            ui.menu_button(format!("{} note(s) choisie(s)", sel.len()), |ui| self.board_notes_menu(ui, layout, texts, &sel));
            ui.separator();
        }
        self.board_background_menu(ui);
    }

    /// Ce qu'on fait des notes choisies de la planche : attacher, détacher, couleur, taille du texte, supprimer.
    fn board_notes_menu(&mut self, ui: &mut egui::Ui, layout: &BoardLayout, texts: &[Option<Rect>], sel: &[NoteRef]) {
        let sel = sel.to_vec();
        {
            if ui.button(format!("{} Attacher à la vignette la plus proche", egui_phosphor::regular::PAPERCLIP)).clicked() {
                ui.close_menu();
                self.record_step();
                for n in &sel {
                    let target = self.note_bounds(layout, texts, *n).and_then(|b| board::nearest_card(layout, b));
                    self.doc.board.set_anchor(layout, &[*n], target);
                }
            }
            if ui.button("Détacher (laisser sur la planche)").clicked() {
                ui.close_menu();
                self.record_step();
                self.doc.board.set_anchor(layout, &sel, None);
            }
            if ui.button("Couleur courante").clicked() {
                ui.close_menu();
                self.record_step();
                self.doc.board.recolor(&sel, self.color);
            }
            let has_text = sel.iter().any(|n| matches!(n, NoteRef::Text(_)));
            for (label, k) in [("Texte plus grand", 1.25), ("Texte plus petit", 0.8)] {
                if has_text && ui.button(label).clicked() {
                    ui.close_menu();
                    self.record_step();
                    for n in &sel {
                        if let NoteRef::Text(i) = n {
                            if let Some(t) = self.doc.board.texts.get_mut(*i) {
                                t.size = (t.size * k).clamp(0.01, 500.0);
                            }
                        }
                    }
                }
            }
            if ui.button("Supprimer les notes choisies (Suppr)").clicked() {
                ui.close_menu();
                self.delete_board_notes();
            }
        }
    }

    /// Le reste du menu du fond de la planche : un plan, les notes, les annotations et l'export, la page.
    pub(crate) fn board_background_menu(&mut self, ui: &mut egui::Ui) {
        if ui.button("Nouveau plan").clicked() {
            ui.close_menu();
            self.new_panel();
        }
        self.action_item(ui, "Nouvelle séquence à la fin du film", "sequence.nouvelle_fin");
        // (« Attacher les nouvelles notes » et « Effacer toutes les notes » sont partis, partie 113, décision 7 : les notes libres de la planche
        // n'ont plus d'outil dans les Vignettes ; celles d'un projet restent lisibles, et la gomme les efface.)
        ui.separator();
        // Annotations, marie-louise et export de la planche (`annotationsui.rs`).
        self.annot_menu(ui);
        ui.separator();
        if ui.button("Tout choisir (plans)").clicked() {
            ui.close_menu();
            self.board_sel = self.doc.panels.iter().map(|p| p.id).collect();
        }
        if ui.button(if self.layout.board_sens.japonais() { "Planche à la hauteur de la vue" } else { "Page à la largeur de la vue" }).clicked() {
            ui.close_menu();
            self.board_fit();
        }
    }

    /// Repères du geste en cours : lasso de choix.
    fn paint_board_drag(&self, painter: &egui::Painter, xf: Xf) {
        let accent = crate::theme::accent_texte();
        if let Some(BoardDrag::Rubber { start, now, .. }) = &self.board_drag {
            let r = Rect::from_two_pos(xf.to_screen(*start), xf.to_screen(*now));
            painter.rect_filled(r, 0.0, accent.gamma_multiply(0.15));
            painter.rect_stroke(r, 0.0, Stroke::new(1.0_f32, accent), StrokeKind::Inside);
        }
    }
}


/// Ce que `board_card` peint d'une case à cette passe (partie 131) : son image ; ou ce qui l'habille (filet, cadres de caméra, liseré, note,
/// numéro), peint après le dessin que la planche pose par-dessus les images.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PasseCase {
    Image,
    Habillage,
}

/// La texture `id` (de rectangle `full` à l'écran) peinte seulement dans le polygone convexe `poly` : un maillage dont les coordonnées
/// de texture suivent l'écran. Le même cliché, coupé net : aucune recomposition.
fn peint_texture_dans(painter: &egui::Painter, id: egui::TextureId, full: Rect, poly: &[Pos2]) {
    let morceau = crate::viewbar::clip_to_rect(poly, full);
    if morceau.len() < 3 || full.width() < 1e-3 || full.height() < 1e-3 {
        return;
    }
    let mut mesh = egui::Mesh::with_texture(id);
    for p in &morceau {
        mesh.vertices.push(egui::epaint::Vertex { pos: *p, uv: pos2((p.x - full.min.x) / full.width(), (p.y - full.min.y) / full.height()), color: Color32::WHITE });
    }
    for j in 1..morceau.len() as u32 - 1 {
        mesh.add_triangle(0, j, j + 1);
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Ce qu'une vignette montre autour de son cadre : le dessin du plan hors du cadre, jusqu'où il va (partie 131 ; le reste de la texture
/// est le cadre, que la vignette courante ne peint pas : c'est le dessin vivant qui l'occupe). Les marges ne sont pas égales : chacune va
/// jusqu'à l'encre de son côté.
fn paint_ring(painter: &egui::Painter, thumb: Rect, c: &crate::thumbs::CaseTex) {
    let full = c.rect(thumb);
    let (w, h) = (c.taille[0].max(1) as f32, c.taille[1].max(1) as f32);
    let [g, t, d, b] = c.marges.map(|m| m as f32);
    let (u0, v0, u1, v1) = (g / w, t / h, 1.0 - d / w, 1.0 - b / h);
    let uv = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(pos2(x0, y0), pos2(x1, y1));
    let strips = [
        (Rect::from_min_max(full.min, pos2(full.max.x, thumb.min.y)), uv(0.0, 0.0, 1.0, v0)),
        (Rect::from_min_max(pos2(full.min.x, thumb.max.y), full.max), uv(0.0, v1, 1.0, 1.0)),
        (Rect::from_min_max(pos2(full.min.x, thumb.min.y), pos2(thumb.min.x, thumb.max.y)), uv(0.0, v0, u0, v1)),
        (Rect::from_min_max(pos2(thumb.max.x, thumb.min.y), pos2(full.max.x, thumb.max.y)), uv(u1, v0, 1.0, v1)),
    ];
    for (r, uv) in strips {
        if r.width() > 0.0 && r.height() > 0.0 {
            painter.image(c.id, r, uv, Color32::WHITE);
        }
    }
}

/// Un trait de note à l'écran : des lignes continues par tronçons d'épaisseur presque égale
/// (la pression fait varier l'épaisseur), bouts arrondis.
pub(crate) fn paint_note(painter: &egui::Painter, view: Rect, xf: Xf, points: &[VPoint], color: [u8; 3]) {
    paint_note_a(painter, view, xf, points, color, 255);
}

/// La distance d'un point à un rectangle (0 dedans).
pub(crate) fn distance_rect(r: Rect, p: Pos2) -> f32 {
    let dx = (r.left() - p.x).max(p.x - r.right()).max(0.0);
    let dy = (r.top() - p.y).max(p.y - r.bottom()).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

/// `paint_note` à l'opacité `alpha` (le dessin hors des cases garde celle du pinceau).
pub(crate) fn paint_note_a(painter: &egui::Painter, view: Rect, xf: Xf, points: &[VPoint], color: [u8; 3], alpha: u8) {
    if points.is_empty() || !xf.rect(board::points_bounds(points)).intersects(view) {
        return;
    }
    let color = Color32::from_rgba_unmultiplied(color[0], color[1], color[2], alpha);
    let mut run: Vec<Pos2> = Vec::new();
    let mut run_w = 0.0;
    let flush = |run: &[Pos2], w: f32| {
        if run.len() >= 2 {
            painter.add(egui::Shape::line(run.to_vec(), Stroke::new(w * 2.0, color)));
        }
    };
    for p in points {
        let s = xf.to_screen(p.pos());
        let w = (p.r * xf.zoom).max(0.4);
        if run.last().is_some_and(|l| l.distance(s) < 1.0) {
            continue;
        }
        if !run.is_empty() && (w - run_w).abs() > 0.3 {
            flush(&run, run_w);
            let last = *run.last().expect("tronçon non vide");
            run.clear();
            run.push(last);
        }
        if run.len() <= 1 {
            run_w = w;
        }
        run.push(s);
    }
    flush(&run, run_w);
    let (a, b) = (points[0], points[points.len() - 1]);
    painter.circle_filled(xf.to_screen(a.pos()), (a.r * xf.zoom).max(0.4), color);
    painter.circle_filled(xf.to_screen(b.pos()), (b.r * xf.zoom).max(0.4), color);
}
