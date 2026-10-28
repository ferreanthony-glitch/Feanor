//! Commentaires sur la timeline, côté interface (voir `commentaires.rs`) : les repères (fanions)
//! dans la règle du montage et de la timeline, leur bulle, leur menu et leur déplacement à la
//! souris, le panneau Commentaires, et les commandes (ajouter, résoudre, supprimer, aller au
//! suivant). Chaque modification est une étape d'annulation qui porte son nom.

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, RichText, Sense, Stroke, StrokeKind};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::commentaires::{self, Comment, Placed, Scope, FLAG_HEAD};
use crate::dock::Pane;
use crate::theme;

/// Hauteur d'une ligne de la liste du panneau.
const ROW_H: f32 = 40.0;
/// Espace entre la liste et l'éditeur.
const GAP: f32 = 10.0;
/// Épaisseur du cadre de la liste (marge et trait, de chaque côté).
const LIST_PAD: f32 = 6.0;
/// Nombre de lignes que la liste garde au moins, même dans un panneau court.
const MIN_ROWS: usize = 3;

/// Hauteur de la liste de `rows` lignes (`pitch` : une ligne et son espacement) dans un panneau de
/// `avail` de haut, dont l'éditeur prend `editor_h` : tout ce qui reste au-dessus de l'éditeur,
/// jamais plus que la liste entière, jamais moins de trois lignes (ou de toutes si elle en a moins).
/// Plus court que ça, le panneau défile : la liste et l'éditeur ne se superposent pas.
pub(crate) fn list_height(avail: f32, editor_h: f32, rows: usize, pitch: f32) -> f32 {
    let natural = rows as f32 * pitch;
    let least = MIN_ROWS.min(rows) as f32 * pitch;
    let room = avail - editor_h - GAP - LIST_PAD;
    natural.min(room.max(least))
}

/// `text` sur une seule ligne, d'au plus `max_w` de large : coupé et fini par « … » s'il est plus long.
pub(crate) fn one_line(painter: &Painter, text: &str, font: FontId, color: Color32, max_w: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), font, color);
    job.wrap.max_width = max_w.max(0.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    painter.layout_job(job)
}

/// Les couleurs proposées : celles des marques de couleur, et un jaune de plus.
pub(crate) fn palette() -> Vec<(&'static str, [u8; 3])> {
    let mut p: Vec<(&'static str, [u8; 3])> = crate::marks::COLORS.to_vec();
    p.push(("Jaune", [250, 204, 21]));
    p
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Ce que la règle d'une bande donne à `App::comment_pointer`.
pub(crate) struct RulerView<'a> {
    /// La règle (la bande du haut où l'on dépose les repères).
    pub ruler: Rect,
    pub placed: &'a [Placed],
    /// Abscisse du mât d'un repère à l'image `pos` de la règle.
    pub pole_x: &'a dyn Fn(usize) -> f32,
    /// Image de la règle sous l'abscisse `x`.
    pub frame_at: &'a dyn Fn(f32) -> usize,
    pub scope: Scope,
    /// Un double-clic ici pose un commentaire (aucun autre repère ne s'y trouve).
    pub plain: bool,
}

/// Ce que les commentaires ont fait de la souris pendant cette image.
#[derive(Default, Clone, Copy)]
pub(crate) struct CommentPtr {
    /// Ils gardent le pointeur : la règle ne fait ni défilement, ni clic, ni glisser de son côté.
    pub busy: bool,
    /// Clic droit sur un repère : ce commentaire a son menu.
    pub menu: Option<u64>,
    /// Le repère sous la souris.
    pub hover: Option<u64>,
}

/// Dessine les repères dans la règle `ruler` : un mât et un fanion de la couleur du commentaire,
/// estompé s'il est résolu, cerclé de blanc s'il est choisi ou survolé.
pub(crate) fn paint_flags(painter: &Painter, ruler: Rect, placed: &[Placed], pole_x: &dyn Fn(usize) -> f32, sel: Option<u64>, hover: Option<u64>) {
    let top = ruler.top() + 2.0;
    for p in placed {
        let x = pole_x(p.pos);
        if x < ruler.left() - 12.0 || x > ruler.right() + 2.0 {
            continue;
        }
        let lit = Some(p.id) == sel || Some(p.id) == hover;
        let base = rgb(p.color);
        let c = if p.done { base.gamma_multiply(0.35) } else { base };
        painter.line_segment([pos2(x, top), pos2(x, ruler.bottom() - 1.0)], Stroke::new(if lit { 2.0_f32 } else { 1.5_f32 }, c));
        let flag = vec![pos2(x, top), pos2(x + 9.0, top + 4.5), pos2(x, top + 9.0)];
        let outline = if lit { Stroke::new(1.5_f32, theme::contour()) } else { Stroke::new(1.0_f32, Color32::from_black_alpha(140)) };
        painter.add(egui::Shape::convex_polygon(flag, c, outline));
    }
}

/// Une pastille de couleur cliquable ; vrai si on la clique.
fn swatch(ui: &mut egui::Ui, color: [u8; 3], name: &str, chosen: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
    ui.painter().rect_filled(rect.shrink(2.0), 4.0, rgb(color));
    if chosen {
        ui.painter().rect_stroke(rect, 4.0, Stroke::new(2.0_f32, ui.visuals().strong_text_color()), StrokeKind::Inside);
    } else if resp.hovered() {
        ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.0_f32, ui.visuals().text_color()), StrokeKind::Inside);
    }
    resp.on_hover_text(name).clicked()
}

/// Les pastilles de la palette ; la couleur cliquée.
fn swatches(ui: &mut egui::Ui, current: [u8; 3]) -> Option<[u8; 3]> {
    let mut picked = None;
    ui.horizontal_wrapped(|ui| {
        ui.set_max_width(7.0 * 24.0);
        ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
        for (name, color) in palette() {
            if swatch(ui, color, name, color == current) {
                picked = Some(color);
            }
        }
    });
    picked
}

/// Une bulle de commentaire, prête à dessiner (les textes sont copiés avant, pour ne pas garder le
/// document emprunté).
struct Tip {
    color: [u8; 3],
    author: String,
    date: String,
    place: String,
    text: String,
    done: bool,
}

fn show_tip(ui: &mut egui::Ui, tip: &Tip) {
    ui.set_max_width(280.0);
    ui.horizontal(|ui| {
        let (dot, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
        ui.painter().circle_filled(dot.center(), 5.0, rgb(tip.color));
        ui.label(RichText::new(if tip.author.is_empty() { "Sans nom" } else { tip.author.as_str() }).strong());
        ui.label(RichText::new(&tip.date).small().weak());
    });
    ui.label(RichText::new(&tip.place).small().color(theme::texte_doux()));
    if tip.text.is_empty() {
        ui.label(RichText::new("(pas encore de texte)").italics().weak());
    } else {
        ui.label(&tip.text);
    }
    if tip.done {
        ui.label(RichText::new(format!("{} Résolu", ph::CHECK_CIRCLE)).small().weak());
    }
}

impl App {
    // ------------------------------------------------------------------ commandes

    /// Une étape d'annulation qui porte le nom `name` (« Nouveau commentaire »…), faite AVANT le
    /// changement.
    pub(crate) fn comment_step(&mut self, name: &str) {
        let before = self.history.context.replace(name.to_string());
        self.record_step();
        self.history.context = before;
    }

    /// `App::edited` avec un nom d'étape : la saisie dans un champ de l'éditeur.
    fn edited_as(&mut self, resp: &egui::Response, name: &str) -> bool {
        let before = self.history.context.replace(name.to_string());
        let changed = self.edited(resp);
        self.history.context = before;
        changed
    }

    /// Texte de l'endroit d'un commentaire : « Plan 3 · 0:12:05 · image 31 ».
    fn comment_place(&self, c: &Comment) -> String {
        match (self.doc.comment_spot(c), self.doc.comment_global(c)) {
            (Some((pi, f)), Some(g)) => format!("{} · {} · image {}", crate::storyboard::panel_title(&self.doc, pi), crate::storyboard::timecode(g, self.doc.fps), self.frame_label(f)),
            _ => "Plan disparu".into(),
        }
    }

    fn tip_of(&self, c: &Comment) -> Tip {
        Tip {
            color: c.color,
            author: c.author.clone(),
            date: if c.created == 0 { String::new() } else { crate::projetui::date_label(c.created) },
            place: self.comment_place(c),
            text: commentaires::preview(&c.text, 5, 70),
            done: c.done,
        }
    }

    /// Nouveau commentaire à l'image courante : il est choisi, le panneau s'ouvre et le champ du
    /// texte prend la main pour l'écrire.
    pub(crate) fn add_comment_here(&mut self) {
        self.end_stroke();
        self.playing = false;
        let panel = self.panel().id;
        let frame = self.cur_frame.min(self.frame_count().saturating_sub(1));
        let author = commentaires::author_name(&self.settings.comment_author);
        self.comment_step("Nouveau commentaire");
        let id = self.doc.add_comment(panel, frame, &author, crate::recovery::now_secs());
        self.comment_ui.sel = Some(id);
        self.comment_ui.focus = true;
        self.comment_ui.focus_text = true;
        self.comment_ui.scroll_to = Some(id);
        self.show_pane(Pane::Comments);
        self.status = format!("Commentaire posé à l'image {} : l'écrire dans le panneau Commentaires.", self.frame_label(frame));
    }

    /// Choisit le commentaire (et y met la tête de lecture si `goto` : en storyboard, le plan
    /// devient courant).
    pub(crate) fn select_comment(&mut self, id: u64, goto: bool) {
        if self.doc.comment(id).is_none() {
            return;
        }
        self.comment_ui.sel = Some(id);
        self.comment_ui.focus = true;
        self.comment_ui.scroll_to = Some(id);
        if goto {
            self.goto_comment(id);
        }
    }

    pub(crate) fn goto_comment(&mut self, id: u64) {
        let Some((pi, frame)) = self.doc.comment(id).and_then(|c| self.doc.comment_spot(c)) else { return };
        self.end_stroke();
        self.playing = false;
        self.set_panel(pi);
        self.cur_frame = frame;
        self.follow_frame = true;
    }

    /// « Modifier » : le commentaire est choisi, le panneau s'ouvre, le champ du texte prend la main.
    pub(crate) fn open_comment_editor(&mut self, id: u64) {
        self.select_comment(id, true);
        self.comment_ui.focus_text = true;
        self.show_pane(Pane::Comments);
    }

    pub(crate) fn delete_comment(&mut self, id: u64) {
        if self.doc.comment(id).is_none() {
            return;
        }
        self.comment_step("Supprimer le commentaire");
        self.doc.remove_comment(id);
        if self.comment_ui.sel == Some(id) {
            self.comment_ui.sel = None;
            self.comment_ui.focus = false;
        }
        self.status = "Commentaire supprimé (Ctrl+Z le rétablit).".into();
    }

    /// Le dernier clic a visé un commentaire (repère ou panneau) : Suppr le supprime.
    pub(crate) fn comment_has_focus(&self) -> bool {
        commentaires::delete_target(self.comment_ui.focus, self.comment_ui.sel, &self.doc).is_some()
    }

    pub(crate) fn delete_selected_comment(&mut self) {
        match self.comment_ui.sel {
            Some(id) => self.delete_comment(id),
            None => self.status = "Aucun commentaire choisi.".into(),
        }
    }

    pub(crate) fn set_comment_done(&mut self, id: u64, done: bool) {
        if self.doc.comment(id).is_none_or(|c| c.done == done) {
            return;
        }
        self.comment_step(if done { "Commentaire résolu" } else { "Commentaire rouvert" });
        if let Some(c) = self.doc.comment_mut(id) {
            c.done = done;
        }
    }

    pub(crate) fn set_comment_color(&mut self, id: u64, color: [u8; 3]) {
        if self.doc.comment(id).is_none_or(|c| c.color == color) {
            return;
        }
        self.comment_step("Couleur du commentaire");
        if let Some(c) = self.doc.comment_mut(id) {
            c.color = color;
        }
    }

    /// Va au commentaire d'après ou d'avant la tête de lecture.
    fn step_to_comment(&mut self, forward: bool) {
        let list = self.doc.comments_in_order(self.comment_ui.show_done);
        match commentaires::step_comment(&list, self.global_frame(), forward) {
            Some(id) => {
                self.select_comment(id, true);
                self.comment_status(id);
            }
            None => self.status = if forward { "Pas de commentaire plus loin." } else { "Pas de commentaire avant." }.into(),
        }
    }

    fn comment_status(&mut self, id: u64) {
        if let Some(c) = self.doc.comment(id) {
            let who = if c.author.is_empty() { "Sans nom".to_string() } else { c.author.clone() };
            let line = commentaires::first_line(&c.text, 80);
            self.status = if line.is_empty() { format!("Commentaire de {who}.") } else { format!("Commentaire de {who} : {line}") };
        }
    }

    /// Les commandes des commentaires ; vrai si `action` en était une (`vue.commentaires`, qui
    /// ouvre le panneau, est dans la barre des panneaux).
    pub(crate) fn comment_action(&mut self, action: &str) -> bool {
        match action {
            "commentaire.ajouter" => self.add_comment_here(),
            "commentaire.resoudre" => match self.comment_ui.sel.and_then(|id| self.doc.comment(id).map(|c| (id, c.done))) {
                Some((id, done)) => self.set_comment_done(id, !done),
                None => self.status = "Aucun commentaire choisi.".into(),
            },
            "commentaire.supprimer" => self.delete_selected_comment(),
            "commentaire.precedent" => self.step_to_comment(false),
            "commentaire.suivant" => self.step_to_comment(true),
            _ => return false,
        }
        true
    }

    // ------------------------------------------------------------------ glisser un repère

    /// Lâche le repère : une étape d'annulation s'il a bougé ; sinon c'était un clic, qui y met la
    /// tête de lecture.
    pub(crate) fn end_comment_drag(&mut self) {
        let Some(d) = self.comment_ui.drag.take() else { return };
        if d.moved && self.doc.comments != d.before {
            let before = self.history.context.replace("Déplacer le commentaire".to_string());
            self.history.push(d.snap);
            self.history.context = before;
            self.modified = true;
            if let Some(c) = self.doc.comment(d.id) {
                let place = self.comment_place(c);
                self.status = format!("Commentaire déplacé : {place}.");
            }
        } else {
            self.doc.comments = d.before;
            self.goto_comment(d.id);
            self.comment_status(d.id);
        }
    }

    /// Échap pendant le geste : le repère revient où il était.
    pub(crate) fn cancel_comment_drag(&mut self) {
        if let Some(d) = self.comment_ui.drag.take() {
            self.doc.comments = d.before;
        }
    }

    // ------------------------------------------------------------------ la souris dans la règle

    /// Les repères d'une règle et la souris : prendre un repère (clic : le choisir et y aller ;
    /// glisser : le déplacer, une étape d'annulation), double-clic dessus (l'éditer) ou sur la règle
    /// (poser un commentaire), clic droit (menu), bulle au survol. À appeler une fois par image,
    /// avant les gestes de la bande, qui s'effacent quand `busy`.
    pub(crate) fn comment_pointer(&mut self, ui: &egui::Ui, resp: &egui::Response, view: &RulerView) -> CommentPtr {
        let mut out = CommentPtr::default();
        let markers: Vec<(u64, f32)> = view.placed.iter().map(|p| (p.id, (view.pole_x)(p.pos))).collect();
        let head = |p: Pos2| view.ruler.contains(p) && p.y <= view.ruler.top() + FLAG_HEAD + 2.0;
        let over = |p: Pos2| if head(p) { commentaires::marker_hits(&markers, p.x) } else { Vec::new() };
        let (pressed, down, hover, at) = ui.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.hover_pos(), i.pointer.interact_pos()));

        // Appui sur un repère : il est pris.
        if self.comment_ui.drag.is_none() && pressed && resp.hovered() && !self.modal_open() {
            if let Some((p, id)) = hover.and_then(|p| over(p).first().map(|&id| (p, id))) {
                let pole = markers.iter().find(|m| m.0 == id).map_or(p.x, |m| m.1);
                self.end_stroke();
                self.playing = false;
                self.comment_ui.sel = Some(id);
                self.comment_ui.focus = true;
                self.comment_ui.scroll_to = Some(id);
                self.comment_ui.drag = Some(commentaires::CommentDrag { id, snap: self.snapshot(), before: self.doc.comments.clone(), grab_dx: p.x - pole, press: p, moved: false });
            }
        }
        // Repère tenu : il suit la souris (image par image) une fois qu'elle s'est un peu éloignée.
        if let Some(d) = &self.comment_ui.drag {
            out.busy = true;
            let (id, grab, press) = (d.id, d.grab_dx, d.press);
            if down {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                if let Some(p) = at.filter(|p| d.moved || p.distance(press) > 4.0) {
                    let f = (view.frame_at)(p.x - grab);
                    match view.scope {
                        Scope::Film => self.doc.move_comment_global(id, f),
                        Scope::Plan(_) => self.doc.move_comment_local(id, f),
                    };
                    let moved = self.doc.comments != self.comment_ui.drag.as_ref().map_or_else(Vec::new, |d| d.before.clone());
                    if let Some(d) = self.comment_ui.drag.as_mut() {
                        d.moved = d.moved || moved;
                    }
                }
            } else {
                self.end_comment_drag();
            }
        }
        // Double-clic : sur un repère, l'éditer ; ailleurs dans la règle, poser un commentaire là.
        if resp.double_clicked() {
            if let Some(p) = at.filter(|p| view.ruler.contains(*p)) {
                out.busy = true;
                if let Some(&id) = over(p).first() {
                    self.open_comment_editor(id);
                } else if view.plain {
                    let f = (view.frame_at)(p.x);
                    match view.scope {
                        Scope::Film => self.goto_global(f),
                        Scope::Plan(_) => self.goto_frame(f),
                    }
                    self.add_comment_here();
                }
            }
        }
        // Clic droit sur un repère : son menu.
        if resp.secondary_clicked() {
            if let Some(&id) = at.and_then(|p| over(p).first().copied()).as_ref() {
                self.comment_ui.sel = Some(id);
                self.comment_ui.focus = true;
                self.comment_ui.scroll_to = Some(id);
                out.menu = Some(id);
                out.busy = true;
            }
        }
        // Survol : la bulle (auteur, date, début du texte), tout de suite ; le curseur « main ».
        if self.comment_ui.drag.is_none() && !down {
            if let Some(hits) = hover.filter(|_| resp.hovered()).map(over).filter(|h| !h.is_empty()) {
                out.hover = hits.first().copied();
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                if !ui.ctx().memory(|m| m.any_popup_open()) {
                    let tips: Vec<Tip> = hits.iter().take(3).filter_map(|&id| self.doc.comment(id).map(|c| self.tip_of(c))).collect();
                    egui::show_tooltip_at_pointer(ui.ctx(), resp.layer_id, resp.id.with("bulle_commentaire"), |ui| {
                        for (i, tip) in tips.iter().enumerate() {
                            if i > 0 {
                                ui.separator();
                            }
                            show_tip(ui, tip);
                        }
                        if hits.len() > 3 {
                            ui.label(RichText::new(format!("… et {} autre(s) à cet endroit", hits.len() - 3)).small().weak());
                        }
                    });
                }
            }
        }
        out
    }

    // ------------------------------------------------------------------ menu d'un repère

    /// Le menu d'un commentaire (clic droit sur son repère ou sur sa ligne) : Modifier, Couleur,
    /// Marquer résolu, Supprimer.
    pub(crate) fn comment_menu(&mut self, ui: &mut egui::Ui, id: u64) {
        let Some((done, color)) = self.doc.comment(id).map(|c| (c.done, c.color)) else {
            ui.label(RichText::new("Ce commentaire n'existe plus.").weak());
            return;
        };
        if ui.button("Modifier").clicked() {
            ui.close_menu();
            self.open_comment_editor(id);
        }
        ui.menu_button("Couleur", |ui| {
            if let Some(c) = swatches(ui, color) {
                ui.close_menu();
                self.set_comment_color(id, c);
            }
        });
        if ui.button(if done { "Rouvrir (non résolu)" } else { "Marquer résolu" }).clicked() {
            ui.close_menu();
            self.set_comment_done(id, !done);
        }
        ui.separator();
        if ui.button("Supprimer").clicked() {
            ui.close_menu();
            self.delete_comment(id);
        }
    }

    // ------------------------------------------------------------------ panneau Commentaires

    pub(crate) fn comments_panel(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::bulle;
        ui.horizontal_wrapped(|ui| {
            let key = self.raccourci("commentaire.ajouter");
            let resp = ui.button(format!("{}  Nouveau commentaire", ph::CHAT_CENTERED_TEXT));
            if bulle(resp, "Nouveau commentaire", "Un fanion à l'image courante, dans la règle du montage et de la timeline ; on l'écrit ici.", &key).clicked() {
                self.run_action("commentaire.ajouter");
            }
            self.command_button(ui, ph::CARET_LEFT, "commentaire.precedent", "Commentaire précédent", "Aller au commentaire d'avant la tête de lecture.", None);
            self.command_button(ui, ph::CARET_RIGHT, "commentaire.suivant", "Commentaire suivant", "Aller au commentaire d'après la tête de lecture.", None);
        });
        ui.horizontal(|ui| {
            let resp = ui.checkbox(&mut self.comment_ui.show_done, "Montrer les résolus");
            bulle(resp, "Montrer les résolus", "Décoché : les commentaires résolus quittent la liste et les règles.", "");
            let (all, shown) = (self.doc.comments_in_order(true).len(), self.doc.comments_in_order(self.comment_ui.show_done).len());
            ui.label(RichText::new(format!("{shown} / {all}")).small().weak());
        });
        ui.separator();
        // La liste (son cadre et son défilement) puis l'éditeur dessous, l'un après l'autre : la
        // liste prend la place qui reste au-dessus de l'éditeur, jamais moins de trois lignes ; si
        // le panneau est plus court que les deux ensemble, c'est le panneau qui défile (rien ne se
        // superpose, à aucune taille). Avant, l'éditeur était collé au bas du panneau et la liste
        // gardait au moins 64 points : dans un panneau court, elle passait sous l'éditeur.
        let avail = ui.available_height();
        egui::ScrollArea::vertical().id_salt("panneau_commentaires").auto_shrink([false, false]).show(ui, |ui| {
            self.comment_list(ui, avail);
            ui.add_space(GAP * 0.5);
            ui.separator();
            let editor = ui.scope(|ui| self.comment_editor(ui)).response.rect.height();
            if (editor - self.comment_ui.editor_h).abs() > 0.5 {
                self.comment_ui.editor_h = editor;
                ui.ctx().request_repaint();
            }
        });
        // Un clic dans le panneau lui donne la main : Suppr supprime le commentaire choisi.
        if self.comment_ui.sel.is_some() && ui.input(|i| i.pointer.any_pressed()) && ui.rect_contains_pointer(ui.max_rect()) {
            self.comment_ui.focus = true;
        }
    }

    /// La liste, dans l'ordre du film, dans un cadre qui a son propre défilement ; `avail` : la
    /// hauteur du panneau sous son en-tête. Seules les lignes visibles sont dessinées.
    fn comment_list(&mut self, ui: &mut egui::Ui, avail: f32) {
        let list = self.doc.comments_in_order(self.comment_ui.show_done);
        if list.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new("Aucun commentaire. Double-clic dans la règle du montage ou de la timeline, ou « Nouveau commentaire ».").small().weak());
            return;
        }
        let pitch = ROW_H + ui.spacing().item_spacing.y;
        let height = list_height(avail, self.comment_ui.editor_h, list.len(), pitch);
        let mut area = egui::ScrollArea::vertical().id_salt("liste_commentaires").auto_shrink([false, false]).min_scrolled_height(height).max_height(height);
        if let Some(i) = self.comment_ui.scroll_to.take().and_then(|id| list.iter().position(|(_, x)| *x == id)) {
            area = area.vertical_scroll_offset((i as f32 - 1.0).max(0.0) * pitch);
        }
        let mut clicked: Option<u64> = None;
        let mut toggled: Option<(u64, bool)> = None;
        let fps = self.doc.fps;
        let frame = egui::Frame::new().stroke(ui.visuals().widgets.noninteractive.bg_stroke).corner_radius(4).inner_margin(egui::Margin::same(2));
        frame.show(ui, |ui| area.show_rows(ui, ROW_H, list.len(), |ui, range| {
            for i in range {
                let (global, id) = list[i];
                let Some(c) = self.doc.comment(id).cloned() else { continue };
                let plan = self.doc.comment_spot(&c).map(|(pi, _)| crate::storyboard::panel_title(&self.doc, pi)).unwrap_or_default();
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
                let selected = self.comment_ui.sel == Some(id);
                let painter = ui.painter_at(rect);
                if selected {
                    painter.rect_filled(rect.shrink2(vec2(0.0, 1.0)), 3.0, ui.visuals().selection.bg_fill.gamma_multiply(0.45));
                } else if resp.hovered() {
                    painter.rect_filled(rect.shrink2(vec2(0.0, 1.0)), 3.0, theme::eleve());
                }
                let dim = if c.done { 0.45 } else { 1.0 };
                painter.circle_filled(pos2(rect.left() + 11.0, rect.top() + 13.0), 5.0, rgb(c.color).gamma_multiply(dim));
                let check = Rect::from_center_size(pos2(rect.right() - 14.0, rect.top() + 13.0), vec2(20.0, 20.0));
                // Chaque ligne de texte tient sur une seule ligne, coupée par « … » avant la case
                // « résolu » : la hauteur d'une ligne de la liste ne dépend jamais du texte.
                let strong = theme::texte().gamma_multiply(dim);
                let text_right = check.left() - 4.0;
                let time = one_line(&painter, &crate::storyboard::timecode(global, fps), FontId::monospace(11.0), strong, text_right - (rect.left() + 22.0));
                let who_x = rect.left() + 22.0 + time.size().x + 8.0;
                let who = if c.author.is_empty() { plan.clone() } else { format!("{plan} · {}", c.author) };
                let who = one_line(&painter, &who, FontId::proportional(10.5), theme::texte_doux(), text_right - who_x);
                painter.galley(pos2(rect.left() + 22.0, rect.top() + 5.0), time, strong);
                painter.galley(pos2(who_x, rect.top() + 6.0), who, theme::texte_doux());
                let first = commentaires::first_line(&c.text, 90);
                let (first, color) = if first.is_empty() { ("(pas encore de texte)".to_string(), theme::texte_doux()) } else { (first, strong) };
                let first = one_line(&painter, &first, FontId::proportional(11.5), color, text_right - (rect.left() + 22.0));
                painter.galley(pos2(rect.left() + 22.0, rect.top() + 21.0), first, color);
                // La case « résolu ».
                let box_resp = ui.interact(check, resp.id.with("resolu"), Sense::click());
                let frame = Rect::from_center_size(check.center(), vec2(14.0, 14.0));
                painter.rect_stroke(frame, 3.0, Stroke::new(1.0_f32, if box_resp.hovered() { theme::texte() } else { theme::texte_doux() }), StrokeKind::Inside);
                if c.done {
                    painter.text(frame.center(), Align2::CENTER_CENTER, ph::CHECK, FontId::proportional(12.0), theme::texte());
                }
                crate::bulles::bulle(box_resp.clone(), "Résolu", if c.done { "Cliquer pour rouvrir ce commentaire." } else { "Cliquer pour marquer ce commentaire résolu." }, "");
                if box_resp.clicked() {
                    toggled = Some((id, !c.done));
                } else if resp.clicked() {
                    clicked = Some(id);
                }
                resp.context_menu(|ui| self.comment_menu(ui, id));
            }
        }));
        if let Some(id) = clicked {
            self.select_comment(id, true);
            // La ligne cliquée est déjà sous les yeux : la liste ne saute pas.
            self.comment_ui.scroll_to = None;
        }
        if let Some((id, done)) = toggled {
            self.comment_ui.sel = Some(id);
            self.comment_ui.focus = true;
            self.set_comment_done(id, done);
        }
    }

    /// L'éditeur du commentaire choisi : texte sur plusieurs lignes, auteur, couleur, résolu,
    /// supprimer.
    fn comment_editor(&mut self, ui: &mut egui::Ui) {
        let Some(c) = self.comment_ui.sel.and_then(|id| self.doc.comment(id)).cloned() else {
            ui.label(RichText::new("Choisis un commentaire dans la liste ou sur un fanion de la règle pour l'écrire ici.").small().weak());
            return;
        };
        let id = c.id;
        ui.horizontal(|ui| {
            let (dot, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
            ui.painter().circle_filled(dot.center(), 5.0, rgb(c.color));
            ui.label(RichText::new(self.comment_place(&c)).small().color(theme::texte_doux()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let key = self.raccourci("commentaire.supprimer");
                let resp = ui.add(egui::Button::new(RichText::new(ph::TRASH).size(crate::bulles::TAILLE)).frame(false));
                if crate::bulles::bulle(resp, "Supprimer le commentaire", "Le retire de la timeline (Ctrl+Z le rétablit).", &key).clicked() {
                    self.delete_comment(id);
                }
            });
        });
        let mut text = c.text.clone();
        let resp = ui.add(egui::TextEdit::multiline(&mut text).id(egui::Id::new(("texte_commentaire", id))).desired_rows(4).desired_width(f32::INFINITY).hint_text("Écrire le commentaire…"));
        if std::mem::take(&mut self.comment_ui.focus_text) {
            resp.request_focus();
        }
        if self.edited_as(&resp, "Texte du commentaire") {
            if let Some(m) = self.doc.comment_mut(id) {
                m.text = text;
            }
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new("Auteur").small().weak());
            let mut author = c.author.clone();
            let resp = ui.add(egui::TextEdit::singleline(&mut author).id(egui::Id::new(("auteur_commentaire", id))).desired_width(ui.available_width()));
            if self.edited_as(&resp, "Auteur du commentaire") {
                if let Some(m) = self.doc.comment_mut(id) {
                    m.author = author;
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Couleur").small().weak());
            if let Some(color) = swatches(ui, c.color) {
                self.set_comment_color(id, color);
            }
            // Une couleur à soi.
            let mut custom = c.color;
            let resp = crate::champcouleur::srgb(ui, &mut custom);
            crate::bulles::bulle(resp.clone(), "Autre couleur", "Choisir une couleur du sélecteur.", "");
            if self.edited_as(&resp, "Couleur du commentaire") {
                if let Some(m) = self.doc.comment_mut(id) {
                    m.color = custom;
                }
            }
        });
        ui.horizontal(|ui| {
            let mut done = c.done;
            if ui.checkbox(&mut done, "Résolu").changed() {
                self.set_comment_done(id, done);
            }
            if c.created != 0 {
                ui.label(RichText::new(format!("Créé le {}", crate::projetui::date_label(c.created))).small().weak());
            }
        });
        // Une note de relecture avec son dessin : l'interrupteur qui le montre sur l'image (`relectureui.rs`).
        if c.annotation.is_some() {
            self.relecture_drawing_row(ui, id);
        }
    }
}
