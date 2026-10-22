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
//! - **Notes** : un clic sur la bande sous une image écrit la note du plan (première image) ou de la marque
//!   d'image ; Entrée valide, Maj + Entrée passe à la ligne, Échap abandonne. Trois lignes au plus à l'écran,
//!   puis « … » ; le texte entier est gardé, visible pendant la saisie et dans la fiche du plan.
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
use crate::board::{self, BoardLayout, BoardSettings, CardPlace, Hit, NoteRef, CARD_MAX, CARD_MIN, MAX_COLUMNS, NOTE_MAX_LINES, NOTE_TEXT_SIZE};
use crate::history::Nav;
use crate::vector::VPoint;

/// Le papier : blanc, sans bord, à perte de vue (plus de carte ni de table autour).
const PAPER: Color32 = Color32::WHITE;
const INK: Color32 = Color32::from_gray(30);
/// Filet d'une image (gris moyen, comme la maquette), de l'image courante, d'une case pâle.
const BORDER: Color32 = Color32::from_gray(154);
const BORDER_CURRENT: Color32 = Color32::from_gray(80);
const GHOST: Color32 = Color32::from_gray(228);
const GHOST_HOT: Color32 = Color32::from_gray(150);
/// Barre de continuité (même plan : un trait très pâle) et de coupe (noire).
const BAR_JOINED: Color32 = Color32::from_gray(230);
const BAR_CUT: Color32 = Color32::from_gray(26);
/// Texte d'une note (gris discret), et le filet très pâle sous chaque bande.
const NOTE_INK: Color32 = Color32::from_gray(100);
const HAIRLINE: Color32 = Color32::from_gray(236);
/// Largeur (en points d'écran) sous laquelle une barre est élargie pour rester facile à toucher.
const BAR_TOUCH: f32 = 14.0;
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
    /// Une bande de note vient de s'ouvrir.
    Note,
    /// Un appui qui ne fait rien (dehors de toute image avec un outil de dessin, case vide refusée…).
    Dead,
    /// Maj + appui sur le cadre d'une vignette : le plan se déplace (`PlanMove`).
    Move,
    /// Le stylo ou la gomme d'annotation tient l'appui (`annotationsui.rs`) : le dessin de la vignette ne reçoit rien.
    Annot,
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
    /// Maj + Alt : on glisse la copie du plan ; l'original reste où il est, la copie se pose à l'endroit du lâcher (n'importe où entre
    /// deux plans, y compris juste devant ou juste après l'original).
    pub copie: bool,
}

/// Distance (points d'écran) dont il faut glisser un cadre avant que ce soit un déplacement.
pub(crate) const MOVE_THRESHOLD: f32 = 5.0;
/// Tolérance (points d'écran) de part et d'autre du filet d'une vignette pour la prendre par son cadre.
const FRAME_TOUCH: f32 = 5.0;

/// La bande de note en cours d'écriture : celle de l'image `id` (le plan, ou sa marque d'image).
pub(crate) struct NoteEdit {
    pub id: u64,
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
    /// `cadre` : le cadre de caméra sous le clic droit, s'il y en a un (« Retirer ce cadre »).
    Card { panel: usize, frame: usize, cadre: Option<usize> },
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
    /// Lignes de note affichées de chaque image qui a une note (mesurées à chaque image d'écran).
    pub lines: HashMap<u64, usize>,
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
    /// Les annotations : outil, couleur, affichage, marie-louise (`annotationsui.rs`).
    pub annot: crate::annotationsui::Etat,
    /// Les images déposées sur la planche, en préparation sur un fil de fond (`deposer.rs`).
    pub depots: Vec<crate::deposer::Travail>,
    /// Les fenêtres d'export de la planche et du color script (`plancheui.rs`, `colorscriptui.rs`).
    pub planche: crate::plancheui::Etat,
    pub colorscript: crate::colorscriptui::Etat,
}

impl Default for Vignettes {
    fn default() -> Self {
        Vignettes {
            press: Press::None,
            note: None,
            lines: HashMap::new(),
            empty_joins: false,
            created: None,
            menu: None,
            xf: Xf::default(),
            view: Rect::NOTHING,
            moving: None,
            frame_hot: false,
            debord: None,
            cam: None,
            zone: None,
            enveloppe: Vec::new(),
            decalage: Vec2::ZERO,
            camera_tenue: false,
            annot: Default::default(),
            depots: Vec::new(),
            planche: Default::default(),
            colorscript: Default::default(),
        }
    }
}

/// Le texte d'une note, mis en page : `size` points, `width` de large, trois lignes au plus et « … ».
fn note_job(text: &str, size: f32, width: f32, color: Color32) -> LayoutJob {
    let mut job = LayoutJob::simple(text.to_string(), FontId::proportional(size.max(1.0)), color, width.max(1.0));
    job.wrap.max_rows = NOTE_MAX_LINES;
    job.wrap.overflow_character = Some('…');
    job
}

impl App {
    /// La planche est-elle à l'écran (storyboard, vue Vignettes) ?
    pub(crate) fn board_shown(&self) -> bool {
        self.mode == Mode::Storyboard && self.board_view == BoardView::Panels
    }

    /// La disposition de la planche, avec ses réglages et les lignes de note mesurées : sur le papier infini, une seule page, les
    /// cases pâles qui montrent où dessiner, et la place que prend le geste de caméra en cours (`board::Reserve`).
    pub(crate) fn board_layout(&self) -> BoardLayout {
        let s = BoardSettings { rows: 0, ..self.layout.board_settings() };
        let hold = self.vignettes.cam.as_ref().filter(|g| g.actif && !g.annule).and_then(|g| g.hold);
        let opt = board::Options { hold, ..board::Options::pale(self.layout.board_min_rows) };
        board::layout_with(&self.doc, &s, &self.vignettes.lines, self.vignettes.empty_joins, &opt)
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
        let gone: Vec<u64> =
            before.cards.iter().map(|c| c.id).filter(|id| !self.doc.panels.iter().any(|p| p.id == *id)).collect();
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
                self.status = match ids.len() {
                    1 => "Plan supprimé.".into(),
                    n => format!("{n} plans supprimés."),
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
    pub(crate) fn card_notes(&self, place: &CardPlace) -> &str {
        let p = &self.doc.panels[place.panel];
        if place.frame == 0 {
            &p.notes
        } else {
            p.marks.iter().find(|m| m.id == place.id).map_or("", |m| m.notes.as_str())
        }
    }

    /// Mesure le nombre de lignes (1 à 3) de chaque note, à la largeur des vignettes : la mise en page
    /// s'en sert pour descendre la rangée suivante juste ce qu'il faut.
    fn measure_notes(&mut self, ctx: &egui::Context, settings: &BoardSettings) {
        let width = (settings.card_w.clamp(CARD_MIN, CARD_MAX) - NOTE_PAD * 2.0).max(10.0);
        let mut lines = HashMap::new();
        for p in &self.doc.panels {
            let marks = p.marks.iter().map(|m| (m.id, m.notes.as_str()));
            for (id, text) in std::iter::once((p.id, p.notes.as_str())).chain(marks) {
                if text.trim().is_empty() {
                    continue;
                }
                let size = board::note_font_size(text.chars().count());
                let galley = ctx.fonts(|f| f.layout_job(note_job(text, size, width, NOTE_INK)));
                lines.insert(id, galley.rows.len().clamp(1, NOTE_MAX_LINES));
            }
        }
        self.vignettes.lines = lines;
    }

    /// Ouvre la bande de note de l'image `place` pour l'écrire.
    fn open_card_note(&mut self, place: &CardPlace) {
        if self.content_refuses() {
            return;
        }
        let buffer = self.card_notes(place).to_string();
        self.vignettes.note = Some(NoteEdit { id: place.id, buffer, focused: false, had_focus: false });
    }

    /// Pose la note en cours d'écriture dans le plan (ou la marque d'image). Une étape d'annulation, nommée,
    /// seulement si le texte a changé.
    pub(crate) fn commit_card_note(&mut self) {
        let Some(ed) = self.vignettes.note.take() else { return };
        let text = ed.buffer.trim_end().to_string();
        let Some(pi) = self.doc.panels.iter().position(|p| p.id == ed.id || p.marks.iter().any(|m| m.id == ed.id)) else { return };
        let on_plan = self.doc.panels[pi].id == ed.id;
        let now = if on_plan { self.doc.panels[pi].notes.as_str() } else { self.doc.panels[pi].marks.iter().find(|m| m.id == ed.id).map_or("", |m| m.notes.as_str()) };
        if now == text || (now.trim().is_empty() && text.trim().is_empty()) {
            return;
        }
        if self.content_refuses() {
            return;
        }
        self.avec_etape("Note de vignette", |a| a.record_step());
        if on_plan {
            self.doc.panels[pi].notes = text;
        } else if let Some(m) = self.doc.panels[pi].marks.iter_mut().find(|m| m.id == ed.id) {
            m.notes = text;
        }
        self.status = "Note de la vignette enregistrée.".into();
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
        let strip = xf.rect(place.note);
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
    fn tool_draws_in_empty(&self) -> bool {
        matches!(self.tool, Tool::Brush | Tool::Pencil | Tool::Line | Tool::Rect | Tool::Ellipse)
    }

    /// Le dessin a la main (copier, coller, supprimer visent le dessin, pas les plans) et un outil pris par une touche
    /// tenue sert : ce que fait `canvas_panel` quand l'appui lui vient de l'interface, ici l'appui est vu plus tôt.
    fn draw_has_focus(&mut self) {
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
                    MenuTarget::Card { panel: c.panel, frame: c.frame, cadre }
                }
                _ => MenuTarget::Background,
            });
            return false;
        }
        if button != PointerButton::Primary {
            return false;
        }
        // Tout appui ailleurs que dans la bande en cours d'écriture la pose.
        if !matches!(hit, Hit::Note(k) if self.vignettes.note.as_ref().is_some_and(|e| e.id == layout.cards[k].id)) {
            self.commit_card_note();
        }
        // Maj + appui sur le CADRE d'une vignette (son filet, à quelques points près) : on prend son plan pour le
        // déplacer ; Maj + Alt : on prend sa copie (le plan se duplique, la copie se pose où l'on lâche). Maj + appui dedans garde son
        // sens : choisir des plans.
        if m.shift && !m.ctrl && !m.command {
            if let Some(k) = board::frame_hit(layout, p, FRAME_TOUCH / xf.zoom.max(1e-3)) {
                self.commit_note_text(layout);
                self.start_plan_move(layout, k, pos, m.alt);
                return false;
            }
        }
        // Ctrl + Maj, ou T tenue : un appui sur une case, son empreinte ou sa marge est TOUJOURS un geste de caméra (jamais un
        // plan qu'on déplace, ni le dessin, ni le lasso). Un appui qui rate un cadre en prend quand même un (`vignettecam::choisit`).
        // (Ctrl seul est le lasso, comme partout : plus bas.)
        let cam_mode = ((m.command || m.ctrl) && m.shift) || self.vignettes.camera_tenue;
        if cam_mode {
            if let Some(k) = layout.card_near(p, self.marge_camera()) {
                self.commit_note_text(layout);
                let ajoute = m.alt;
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
            Hit::Bar(after) => {
                self.commit_note_text(layout);
                self.vignettes.press = Press::Bar;
                self.board_toggle_cut(layout, after)
            }
            Hit::Thumb(k) => {
                self.commit_note_text(layout);
                let c = layout.cards[k];
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
                        // Une brosse du décor choisie : le trait part dans le calque Décor du plan (`decorui.rs`).
                        self.decor_avant_trait();
                    }
                }
                false
            }
            Hit::Note(k) => {
                self.commit_note_text(layout);
                let c = layout.cards[k];
                self.board_sel = vec![c.plan];
                self.select_board(c.panel, c.frame);
                if self.vignettes.note.as_ref().is_none_or(|e| e.id != c.id) {
                    self.open_card_note(&c);
                }
                self.vignettes.press = Press::Note;
                false
            }
            Hit::Empty(e) => {
                self.commit_note_text(layout);
                if !cam_mode && !lasso && self.tool_draws_in_empty() && self.board_create_from_empty(e) {
                    self.vignettes.press = Press::Card;
                    self.draw_has_focus();
                    self.decor_avant_trait();
                    true
                } else {
                    self.vignettes.press = Press::Dead;
                    false
                }
            }
            Hit::Nothing => {
                // Dehors de toute image : les outils de dessin ne tracent rien ; le fond reçoit le reste.
                if matches!(self.tool, Tool::Brush | Tool::Pencil | Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Polyline | Tool::Fill | Tool::Cutter) {
                    self.vignettes.press = Press::Dead;
                }
                false
            }
        }
    }

    /// Choisir des plans (comme sur l'ancien titre) : Ctrl ajoute ou retire celui de la vignette `k`, Maj prend l'intervalle.
    pub(crate) fn board_select_click(&mut self, layout: &BoardLayout, k: usize, m: Modifiers) {
        let c = layout.cards[k];
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

    /// Bascule la barre après l'image `after` : grise → noire coupe le plan là (l'image de droite commence un
    /// plan neuf), noire → grise réunit les deux plans (l'image de droite devient une image du plan de gauche).
    /// La dernière barre, devant la première case vide, dit ce que fera cette case : un plan ou une image.
    /// Une seule étape d'annulation, nommée. Vrai si le document a changé.
    pub(crate) fn board_toggle_cut(&mut self, layout: &BoardLayout, after: usize) -> bool {
        let Some(a) = layout.cards.get(after).copied() else { return false };
        let Some(b) = layout.cards.get(after + 1).copied() else {
            self.vignettes.empty_joins = !self.vignettes.empty_joins;
            self.status = if self.vignettes.empty_joins {
                "La prochaine case vide fera une image de plus de ce plan (barre grise).".into()
            } else {
                "La prochaine case vide fera un nouveau plan (barre noire).".into()
            };
            return false;
        };
        if b.plan == a.plan {
            // Couper : la marque de la vignette de droite devient le début d'un plan neuf.
            if self.storyboard_refuses() || !self.partage_peut_toucher(&[a.plan]) {
                return false;
            }
            let ok = self.avec_etape("Couper le plan entre deux vignettes", |s| s.structure_op(|d, _| d.split_plan(a.panel, b.frame)));
            if ok {
                self.cur_frame = 0;
                self.board_sel = vec![self.doc.panels[self.cur_panel].id];
                self.status = format!("Plan coupé : la vignette de droite commence le {}.", crate::storyboard::panel_title(&self.doc, self.cur_panel).to_lowercase());
            }
            ok
        } else {
            // Réunir : le début du plan de droite devient une marque d'image du plan de gauche.
            if self.storyboard_refuses() || !self.partage_peut_toucher(&[a.plan, b.plan]) {
                return false;
            }
            let ok = self.avec_etape("Réunir deux vignettes en un plan", |s| {
                s.structure_op(|d, _| {
                    let debut = d.panels.get(a.panel)?.frame_count();
                    let i = d.join_next_plan(a.panel)?;
                    // La case de droite devient une clé de dessin de plus sur chaque calque du plan (`cases.rs`).
                    d.cles_de_case(i, debut);
                    Some(i)
                })
            });
            if ok {
                self.cur_frame = a.end;
                self.board_sel = vec![self.doc.panels[self.cur_panel].id];
                self.status = format!("Plans réunis : la vignette de droite est une image du {}.", crate::storyboard::panel_title(&self.doc, self.cur_panel).to_lowercase());
            } else {
                self.status = "Ces deux plans ne sont pas dans la même séquence : impossible de les réunir.".into();
            }
            ok
        }
    }

    /// Dessiner dans la case vide `e` (0 : la première) fait un plan neuf, avec ceux qui la précèdent (des plans
    /// vides, pour garder sa place) ; ou, si la barre devant la première case vide est grise, une image de plus
    /// du dernier plan. Le plan fait est courant, prêt à recevoir le trait de l'appui en cours. Vrai si fait.
    pub(crate) fn board_create_from_empty(&mut self, e: usize) -> bool {
        if self.storyboard_refuses() {
            return false;
        }
        let joins = self.vignettes.empty_joins;
        let made = e + 1;
        let mut frame_of_image = 0;
        let ok = self.avec_etape(if joins && e == 0 { "Nouvelle image dessinée" } else { "Nouveau plan dessiné" }, |s| {
            s.structure_op(|d, _| {
                let mut last = d.panels.len() - 1;
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
        if !ok {
            return false;
        }
        self.cur_frame = if joins && e == 0 { frame_of_image } else { 0 };
        self.follow_frame = true;
        self.board_sel = vec![self.doc.panels[self.cur_panel].id];
        self.vignettes.created = Some(Created { base_len: self.history.undo_len() });
        self.status = if joins && e == 0 { "Nouvelle image du plan : dessine-la.".into() } else { "Nouveau plan : dessine-le.".into() };
        true
    }

    /// Au lâcher : la création d'une case vide et le trait ne font qu'une étape d'annulation ; si rien n'a été
    /// tracé (un simple clic), le plan fait est défait sans laisser de trace.
    fn finish_creation(&mut self, any_down: bool) {
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
                self.cur_frame = self.cur_frame.min(self.frame_count().saturating_sub(1));
                self.board_sel.clear();
                self.renderer.invalidate_all();
                self.thumbs.prune(&self.doc);
            }
        }
    }

    // ------------------------------------------------------------------ la vue

    pub(crate) fn board_view(&mut self, ui: &mut egui::Ui) {
        // (Les pastilles des personnages : une colonne au bord droit de la planche, posée en fin d'image, plus de barre
        // au-dessus : `pastilles_colonne`, `personnagesui.rs`.)
        // Le débord du trait hors des cases suit les Préférences (il fait partie de la clé des vignettes : elles se refont en fond).
        self.thumbs.debord = Some(self.settings.vignettes.debord_fraction());
        self.board_toolbar(ui);
        self.variantes_barre(ui);
        let view = ui.available_rect_before_wrap();
        self.vignettes.view = view;
        let bg = ui.allocate_rect(view, Sense::click_and_drag());
        let painter = ui.painter_at(view);
        painter.rect_filled(view, 0.0, PAPER);
        self.board_sel.retain(|id| self.doc.panels.iter().any(|p| p.id == *id));
        let (ns, nt) = (self.doc.board.strokes.len(), self.doc.board.texts.len());
        self.board_notes.retain(|n| match *n {
            NoteRef::Stroke(i) => i < ns,
            NoteRef::Text(i) => i < nt,
        });
        let settings = self.layout.board_settings();
        self.measure_notes(ui.ctx(), &settings);
        let mut layout = self.board_layout();
        self.suivre_decalage(&layout);
        if self.board_zoom <= 0.0 {
            self.board_zoom = (view.width() / layout.size.x.max(1.0)).clamp(ZOOM_MIN, 1.5);
            self.board_pan = Vec2::ZERO;
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
        self.clamp_board_pan(view, layout.size);
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
        // Maj tenue sur le cadre d'une vignette : elle se prend (curseur « main », pas de cercle de pinceau).
        let tol_frame = FRAME_TOUCH / xf.zoom.max(1e-3);
        self.vignettes.frame_hot = self.vignettes.moving.is_some()
            || (!pan_mode && !self.modal_open() && mods.shift && !mods.ctrl && !mods.command && over.is_some_and(|p| board::frame_hit(&layout, xf.to_board(p), tol_frame).is_some()));

        let texts = self.text_rects(&painter, &layout);
        let tol = 5.0 / xf.zoom;
        let free = self.vignettes.press == Press::None;

        // Gestes commencés sur le fond (hors de toute image, de barre et de note).
        if bg.drag_started_by(PointerButton::Primary) && !pan_mode && free {
            self.commit_note_text(&layout);
            let start = xf.to_board(ui.input(|i| i.pointer.press_origin()).unwrap_or(view.center()));
            let add = mods.ctrl || mods.shift;
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
                _ => self.board_notes.clear(),
            }
        }
        let latest = ui.input(|i| i.pointer.latest_pos());
        let pointer = latest.map(|p| xf.to_board(p));
        self.update_board_drag(&layout, pointer, xf.zoom);
        self.update_plan_move(&layout, xf, latest, escape);
        self.update_cam_geste(latest, escape);
        self.cam_garder_la_place(&layout);
        // Le geste du stylo ou de la gomme d'annotation, point après point (`annotationsui.rs`).
        self.annot_geste(&layout, xf, &events, latest, any_down, touch.is_some(), escape);

        // Le papier est blanc partout (rien autour des cases) ; les barres, les cases pâles qui montrent où dessiner, les vignettes.
        let hovered = over.map(|p| board::hit(&layout, xf.to_board(p), BAR_TOUCH / xf.zoom.max(1e-3)));
        self.paint_bars(ui, &painter, view, xf, &layout, hovered);
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
                Some(Hit::Note(_)) => CursorIcon::Text,
                Some(Hit::Empty(_)) if drawing => CursorIcon::Crosshair,
                Some(Hit::Thumb(_)) if drawing || self.tool == Tool::Eraser => CursorIcon::Crosshair,
                _ if self.tool == Tool::Text => CursorIcon::Text,
                _ if self.tool == Tool::Eraser => CursorIcon::Crosshair,
                _ => CursorIcon::Default,
            };
            // (La vignette courante, peinte après, remplace ce curseur par celui de son outil quand elle l'a sous la souris.)
            ui.ctx().set_cursor_icon(icon);
        }
        let deadline = Instant::now() + crate::thumbs::THUMB_TIME;
        // Les tuiles des empreintes d'abord (leur rectangle déborde de leur enveloppe : les cases qui s'y trouvent se peignent par-dessus).
        for k in 0..layout.cards.len() {
            self.board_tuile(ui, &painter, view, xf, &layout, k, pan_mode, deadline);
        }
        for k in 0..layout.cards.len() {
            self.board_card(ui, &painter, view, xf, &layout, k, pan_mode, deadline);
        }

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
        if let Some(d) = self.vignettes.debord.take() {
            crate::debord::paint(&painter, view, &d);
        }
        // Les annotations, par-dessus tout ; puis la marie-louise, si elle est allumée, qui masque ce qui déborde (`annotationsui.rs`).
        self.annot_peindre(&painter, view, xf, &layout, over);
        self.marie_peindre(&painter, view, xf, &layout);
        // Des images qu'on glisse vers la planche : la case visée, et le lâcher des références (`deposer.rs`).
        self.depot_survol(ui.ctx(), &painter, xf, &layout, over);
        self.paint_plan_move(ui.ctx(), &painter, view, xf, &layout);
        self.paint_board_drag(&painter, xf);
        if let (Some(p), Tool::Eraser, false) = (over, self.tool, pan_mode) {
            if matches!(hovered, Some(Hit::Nothing)) || matches!(self.board_drag, Some(BoardDrag::Erase { .. })) {
                let r = self.note_eraser_radius(xf.zoom) * xf.zoom;
                painter.circle_stroke(p, r, Stroke::new(1.0_f32, INK));
            }
        }
        bg.context_menu(|ui| match self.vignettes.menu {
            Some(MenuTarget::Card { panel, frame, cadre }) => self.card_menu(ui, panel, frame, cadre),
            _ => self.board_menu(ui, &layout, &texts),
        });
        self.note_text_editor(ui, xf, &layout);
        self.card_note_editor(ui, xf, &layout);
        // Le curseur d'un cadre pris ou à prendre passe avant celui du dessin de la vignette courante (peint plus tôt).
        if self.vignettes.frame_hot {
            ui.ctx().set_cursor_icon(if self.vignettes.moving.as_ref().is_some_and(|m| m.active) { CursorIcon::Grabbing } else { CursorIcon::Grab });
        } else if let (Some(p), true) = (over, ((mods.ctrl || mods.command) && mods.shift) || self.vignettes.camera_tenue) {
            // Ctrl + Maj (ou T) tenue sur une case : un cadre de caméra à prendre (une petite caméra suit le pointeur).
            if !pan_mode && self.vignettes.cam.is_none() {
                self.cam_curseur(ui.ctx(), &layout, xf, p, mods.alt);
            }
        }
        // Le stylo ou la gomme d'annotation : curseur en croix (le cercle de l'outil est peint avec les annotations).
        if self.annot_actif() && over.is_some() && !pan_mode {
            ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
        }
        if !any_down {
            self.finish_board_drag(&layout, &texts);
        }
        self.finish_creation(any_down);
        self.finish_plan_move(any_down);
        self.finish_cam_geste(&layout, any_down);
        // Choisir des plans (Ctrl ou Maj + clic) donne la main aux plans : copier, coller, supprimer (12e).
        if self.vignettes.press == Press::Select {
            self.panel_focus = true;
            self.images_focus = false;
            self.key_focus = false;
        }
        if !any_down {
            self.vignettes.press = Press::None;
        }
        // Les pastilles des personnages, en colonne au bord droit de la planche (au-dessus d'elle : ses clics ne vont
        // pas à la planche).
        self.pastilles_colonne(ui.ctx(), view, 8.0);
    }

    /// Les barres entre les images : un trait court, gris ou noir ; sous la souris, l'accent et une bulle.
    fn paint_bars(&self, ui: &mut egui::Ui, painter: &egui::Painter, view: Rect, xf: Xf, layout: &BoardLayout, hovered: Option<Hit>) {
        for b in &layout.bars {
            let zone = xf.rect(b.zone);
            if !zone.intersects(view) {
                continue;
            }
            // (La barre de coupe est un peu plus épaisse que la grise : 2 contre 1,5.)
            let width = (b.bar.width() * xf.zoom).clamp(1.0, 3.5);
            let hot = hovered == Some(Hit::Bar(b.after));
            let color = if hot {
                crate::theme::accent()
            } else if b.cut {
                BAR_CUT
            } else {
                BAR_JOINED
            };
            let r = xf.rect(b.bar);
            painter.line_segment([pos2(r.center().x, r.top()), pos2(r.center().x, r.bottom())], Stroke::new(if hot { width + 1.0 } else { width }, color));
            if hot {
                let resp = ui.interact(zone.expand2(vec2(((BAR_TOUCH - zone.width()) * 0.5).max(0.0), 0.0)), ui.id().with(("planche_barre", b.after)), Sense::hover());
                let (nom, texte) = if b.after + 1 >= layout.cards.len() {
                    if self.vignettes.empty_joins {
                        ("Prochaine case : image du plan", "Grise : la prochaine case vide fera une image de plus du dernier plan. Clic : en faire un nouveau plan.")
                    } else {
                        ("Prochaine case : nouveau plan", "Noire : la prochaine case vide fera un plan. Clic : en faire une image de plus du dernier plan.")
                    }
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
                }
                None => {}
            },
            _ => {
                self.commit_note_text(layout);
                if !add {
                    self.board_sel.clear();
                    self.board_notes.clear();
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
                    etiquette(ui, ph::ARROWS_OUT_SIMPLE, "Taille des vignettes", "");
                    let mut size = self.layout.card_size;
                    let resp = ui.add(egui::Slider::new(&mut size, CARD_MIN..=CARD_MAX).show_value(false));
                    if bulle(resp, "Taille des vignettes", "", "").changed() {
                        self.layout.card_size = size;
                    }
                    etiquette(ui, ph::COLUMNS, "Colonnes", "Vignettes par ligne (6 au plus).");
                    let resp = ui.add(egui::DragValue::new(&mut self.layout.board_columns).range(1..=MAX_COLUMNS).speed(0.05));
                    bulle(resp, "Colonnes", "Vignettes par ligne (6 au plus).", "");
                    let resp = ui.button(format!("{:.0} %", self.board_zoom * 100.0));
                    if bulle(resp, "Zoom de la planche", "Ctrl + molette. Clic : la page à la largeur de la vue.", "").clicked() {
                        self.board_fit();
                    }
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
    }

    fn finish_board_drag(&mut self, layout: &BoardLayout, texts: &[Option<Rect>]) {
        if let Some(BoardDrag::Rubber { start, now, add }) = self.board_drag.take() {
            let r = Rect::from_two_pos(start, now);
            let found: Vec<u64> = board::cards_in(layout, r).into_iter().map(|i| self.doc.panels[i].id).collect();
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
    }

    /// La tuile de l'empreinte d'un mouvement de caméra (Anthony, 2026-10-22 : tout l'espace entre A et B se dessine) : une seule
    /// image qui couvre l'enveloppe des cadres. Le plan courant y est vivant (le dessin lui-même, vu par la zone de la feuille :
    /// un trait y tombe sous le stylet, ailleurs que dans les cadres comme dedans) ; les autres plans montrent leur vignette de
    /// tuile, composée sur un fil de fond. Hors de l'enveloppe, la tuile ne garde que le trait qui déborde. Peinte avant les cases :
    /// son rectangle déborde de l'enveloppe, les cases qui s'y trouvent passent par-dessus.
    #[allow(clippy::too_many_arguments)]
    fn board_tuile(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, view: Rect, xf: Xf, layout: &BoardLayout, k: usize, pan_mode: bool, deadline: Instant) {
        let Some(mv) = layout.move_of(k).filter(|_| self.tool != Tool::Camera) else { return };
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
        let ppp = ui.ctx().pixels_per_point();
        let hull: Vec<Pos2> = mv.enveloppe.iter().map(|p| xf.to_screen(*p)).collect();
        let width = crate::thumbs::width_for(r.width() * ppp);
        // (La marge de la tuile est 12 % de la largeur d'une case, comme celle des autres cases, pas 12 % de la tuile entière.)
        let part_case = place.thumb.width() / mv.tuile.rect.width().max(1e-3);
        let case = self.thumbs.texture_case(ui.ctx(), &self.doc, i, place.frame, place.id, width, Some((&mv.tuile.zone, &mv.tuile.masque, part_case)), deadline);
        // La texture est-elle celle de la géométrie d'aujourd'hui (même zone, même enveloppe) ? Pendant un geste de caméra et juste après,
        // elle est celle d'avant (la tuile se recompose sur un fil de fond, jamais sur celui de l'interface) : posée telle quelle, ses
        // bords de coupe et son papier seraient ceux d'avant le geste.
        let a_jour = case.as_ref().is_some_and(|c| c.geom == Some(crate::thumbs::Geom::de(&mv.tuile.zone, &mv.tuile.masque)));
        let pts_par_feuille = mv.tuile.zone.echelle * xf.zoom;
        let toute = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        if live {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r).id_salt("planche_vive_tuile"));
            child.set_clip_rect(r.intersect(view));
            self.vignettes.zone = Some(mv.tuile.zone);
            self.vignettes.enveloppe = hull.clone();
            self.card_canvas(&mut child, r);
            self.vignettes.zone = None;
            self.vignettes.enveloppe.clear();
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
                paint_ring(painter, r, c);
            }
        } else {
            painter.add(egui::Shape::convex_polygon(hull.clone(), Color32::WHITE, Stroke::NONE));
            if let Some(c) = &case {
                // Posée là où est la feuille qu'elle montre, même si les cadres ont bougé depuis qu'elle a été composée.
                let full = c.rect_tuile(r, &mv.tuile.zone, pts_par_feuille);
                if self.marge_ouverte(i) {
                    painter.image(c.id, full, toute, Color32::WHITE);
                } else {
                    // Un plan qu'on ne dessine pas : l'enveloppe et quelques points, rien de plus (la marge de 12 % ne s'ouvre que
                    // pour le plan où l'on dessine). La même texture, coupée : aucune recomposition.
                    let serre = crate::vignettecam::elargi(&hull, MARGE_SERREE);
                    peint_texture_dans(painter, c.id, full, &serre);
                }
            }
        }
    }

    /// La marge de 12 % autour d'une case (et d'une tuile) n'est ouverte que pour le plan où l'on dessine (le plan courant, ou un trait en
    /// cours) ; les autres plans sont coupés net autour de leurs cadres, à quelques points près (`MARGE_SERREE`).
    pub(crate) fn marge_ouverte(&self, panel: usize) -> bool {
        panel == self.cur_panel
    }
    /// Une vignette (une image de storyboard) : son numéro de plan s'il commence, l'image (le dessin lui-même
    /// pour l'image courante), son filet, sa bande de note. Les gestes sont pris plus haut (`board_press`).
    #[allow(clippy::too_many_arguments)]
    fn board_card(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, view: Rect, xf: Xf, layout: &BoardLayout, k: usize, pan_mode: bool, deadline: Instant) {
        let place = &layout.cards[k];
        let mv = layout.move_of(k).filter(|_| self.tool != Tool::Camera);
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

        // Le numéro du plan, au-dessus de sa première image : rien d'autre (pas de pastille d'envoi ni de version : elles sont au montage).
        if place.starts_plan() && card.height() > thumb.height() + 6.0 {
            let label = Rect::from_min_max(card.min, pos2(card.right(), thumb.top()));
            let font = (11.0 * xf.zoom).clamp(7.0, 22.0);
            let ink = Color32::from_gray(70);
            let clip = painter.with_clip_rect(label.intersect(view));
            let galley = clip.layout_no_wrap(self.doc.scene_label(place.scene), FontId::proportional(font), ink);
            let at = pos2(label.left() + 1.0, label.top() + 1.0);
            clip.galley(at, galley, ink);
        }

        // Le panneau courant : le dessin lui-même ; les autres : leur vignette.
        let live = current && !pan_mode;
        let clip = painter.with_clip_rect(view);
        let ppp = ui.ctx().pixels_per_point();
        match mv {
            // La case d'un plan dont la caméra bouge (deux clés au moins) montre ses cadres à l'échelle des autres cases : une tuile
            // par rangée de l'empreinte, qui montre la feuille et ce qu'on a dessiné autour d'elle (`vignettecam.rs`) ; l'outil Caméra,
            // lui, recadre à l'ancienne (une seule image).
            Some(_) => {}
            None => {
                // La vignette de la case, avec sa petite marge autour du cadre où le trait qui déborde reste visible (comme la maquette) :
                // l'image courante la tient à jour en arrière-plan ; sa marge est peinte par-dessus le dessin vivant.
                let width = crate::thumbs::width_for(thumb.width() * ppp);
                let case = self.thumbs.texture_case(ui.ctx(), &self.doc, i, place.frame, place.id, width, None, deadline);
                if live {
                    // La vue caméra laisse une marge autour du cadre : on l'agrandit d'autant (et on coupe
                    // à la vignette) pour que l'image remplisse la vignette. Pour recadrer, toute la feuille.
                    let area = if self.tool == Tool::Camera { thumb } else { Rect::from_center_size(thumb.center(), thumb.size() / 0.96) };
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area).id_salt("planche_vive"));
                    child.set_clip_rect(thumb.intersect(view));
                    self.card_canvas(&mut child, thumb);
                    // (Sa vignette se tient à jour en arrière-plan : quand une autre image prend la main, elle montre déjà le dernier trait.)
                    if let Some(c) = &case {
                        paint_ring(painter, thumb, c);
                    }
                } else {
                    painter.rect_filled(thumb, 0.0, Color32::WHITE);
                    if let Some(c) = &case {
                        if self.marge_ouverte(i) {
                            painter.image(c.id, c.rect(thumb), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                        } else {
                            // Un plan qu'on ne dessine pas : la case et quelques points, pas la marge de 12 % (la même texture, coupée).
                            let t = thumb.expand(MARGE_SERREE);
                            peint_texture_dans(painter, c.id, c.rect(thumb), &[t.left_top(), t.right_top(), t.right_bottom(), t.left_bottom()]);
                        }
                    }
                }
            }
        }
        // Le filet d'un point autour de l'image ; plus foncé pour l'image courante. Rien d'autre autour.
        clip.rect_stroke(thumb, 0.0, Stroke::new(1.0_f32, if current { BORDER_CURRENT } else { BORDER }), StrokeKind::Outside);
        // Les cadres du mouvement de caméra (A, B, C…), le voile hors de A et ce qui les relie, fins, par-dessus.
        if let Some(mv) = mv {
            self.paint_empreinte(painter, view, xf, place, mv);
        }
        if chosen_more {
            clip.rect_stroke(thumb.expand(1.5), 0.0, Stroke::new(1.5_f32, accent), StrokeKind::Outside);
        }
        self.paint_card_note(&clip, xf, place, note);
    }
    /// La bande de note sous l'image : le texte (trois lignes au plus, « … » au bout), ou un filet très pâle
    /// quand elle est vide. Pendant la saisie, l'éditeur la remplace (`card_note_editor`).
    fn paint_card_note(&self, painter: &egui::Painter, xf: Xf, place: &CardPlace, strip: Rect) {
        let text = self.card_notes(place);
        if self.vignettes.note.as_ref().is_some_and(|e| e.id == place.id) {
            return;
        }
        // Sous chaque bande, un filet très pâle (sans cadre : la maquette) ; le texte, petit et gris, au-dessus.
        let y = strip.bottom() - 1.0;
        painter.line_segment([pos2(strip.left(), y), pos2(strip.right(), y)], Stroke::new(1.0_f32, HAIRLINE));
        if text.trim().is_empty() {
            return;
        }
        let size = board::note_font_size(text.chars().count()) * xf.zoom;
        if size < 4.0 {
            return;
        }
        let job = note_job(text, size, (place.note.width() - NOTE_PAD * 2.0) * xf.zoom, NOTE_INK);
        let galley = painter.layout_job(job);
        painter.galley(strip.min + vec2(NOTE_PAD * xf.zoom, 0.0), galley, NOTE_INK);
    }

    /// Le dessin dans la vignette du panneau courant : vue à travers la caméra, cadrée sur la
    /// vignette ; avec l'outil Caméra, toute la feuille et le cadre, pour recadrer. La vue du
    /// dessin en grand est remise telle quelle ensuite.
    fn card_canvas(&mut self, ui: &mut egui::Ui, thumb: Rect) {
        let (view, camera_view, canvas_rect) = (self.view, self.camera_view, self.canvas_rect);
        let reframe = self.tool == Tool::Camera;
        // (Une grande case de mouvement de caméra montre la feuille, pas la vue à travers la caméra : `vignettes.zone`.)
        self.camera_view = !reframe && self.vignettes.zone.is_none();
        if reframe {
            self.view.fit(ui.max_rect(), self.canvas_size());
        }
        self.in_card = true;
        self.canvas_panel(ui);
        self.in_card = false;
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
        item(self, ui, "Nouveau plan après", &|a| a.new_panel());
        item(self, ui, "Dupliquer le plan", &|a| a.duplicate_panel());
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
            self.plan_color_menu(ui);
            item(self, ui, "Recadrer (outil Caméra)", &|a| a.set_tool(Tool::Camera));
            item(self, ui, "Réinitialiser le cadrage", &|a| a.run_action("camera.reinitialiser"));
        });
    }

    /// Menu du clic droit sur le dessin d'une vignette (la vignette courante reçoit le clic avant le fond).
    pub(crate) fn card_context_menu(&mut self, ui: &mut egui::Ui) {
        match self.vignettes.menu {
            Some(MenuTarget::Card { panel, frame, cadre }) if panel < self.doc.panels.len() => self.card_menu(ui, panel, frame, cadre),
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
        ui.checkbox(&mut self.layout.board_attach, "Attacher les nouvelles notes à la vignette la plus proche");
        ui.separator();
        // Annotations, marie-louise et export de la planche (`annotationsui.rs`).
        self.annot_menu(ui);
        ui.separator();
        if ui.button("Tout choisir (plans)").clicked() {
            ui.close_menu();
            self.board_sel = self.doc.panels.iter().map(|p| p.id).collect();
        }
        if ui.button("Page à la largeur de la vue").clicked() {
            ui.close_menu();
            self.board_fit();
        }
        ui.separator();
        let any = !self.doc.board.strokes.is_empty() || !self.doc.board.texts.is_empty();
        if ui.add_enabled(any, egui::Button::new("Effacer toutes les notes")).clicked() {
            ui.close_menu();
            self.record_step();
            self.doc.board = Default::default();
            self.board_notes.clear();
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

/// Ce qu'une case ou une tuile garde du trait qui déborde quand on n'y dessine pas : quelques points d'écran autour de ses cadres.
pub(crate) const MARGE_SERREE: f32 = 4.0;

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

/// La marge d'une vignette, autour de son cadre : le trait qui déborde un peu (le reste de la texture est le cadre, que la
/// vignette courante ne peint pas : c'est le dessin vivant qui l'occupe).
fn paint_ring(painter: &egui::Painter, thumb: Rect, c: &crate::thumbs::CaseTex) {
    let full = c.rect(thumb);
    let (w, h, m) = (c.taille[0].max(1) as f32, c.taille[1].max(1) as f32, c.marge as f32);
    let (ux, uy) = (m / w, m / h);
    let uv = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(pos2(x0, y0), pos2(x1, y1));
    let strips = [
        (Rect::from_min_max(full.min, pos2(full.max.x, thumb.min.y)), uv(0.0, 0.0, 1.0, uy)),
        (Rect::from_min_max(pos2(full.min.x, thumb.max.y), full.max), uv(0.0, 1.0 - uy, 1.0, 1.0)),
        (Rect::from_min_max(pos2(full.min.x, thumb.min.y), pos2(thumb.min.x, thumb.max.y)), uv(0.0, uy, ux, 1.0 - uy)),
        (Rect::from_min_max(pos2(thumb.max.x, thumb.min.y), pos2(full.max.x, thumb.max.y)), uv(1.0 - ux, uy, 1.0, 1.0 - uy)),
    ];
    for (r, uv) in strips {
        painter.image(c.id, r, uv, Color32::WHITE);
    }
}

/// Un trait de note à l'écran : des lignes continues par tronçons d'épaisseur presque égale
/// (la pression fait varier l'épaisseur), bouts arrondis.
pub(crate) fn paint_note(painter: &egui::Painter, view: Rect, xf: Xf, points: &[VPoint], color: [u8; 3]) {
    if points.is_empty() || !xf.rect(board::points_bounds(points)).intersects(view) {
        return;
    }
    let color = Color32::from_rgb(color[0], color[1], color[2]);
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
