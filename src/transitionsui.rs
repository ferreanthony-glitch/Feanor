//! Les transitions dans l'interface (`transitions.rs` pour le modèle et le rendu) : leur forme dans le
//! montage (un nœud papillon posé sur la coupe, choisissable, dont on tire les bords), leurs menus,
//! leurs commandes (palette Ctrl+K, raccourcis) et l'aperçu à l'écran.
//!
//! Vocabulaire : « transition » (l'objet), « fondu enchaîné », « fondu au noir », « fondu au blanc »,
//! « volet », « noir » (les sortes), « coupe » (la jointure de deux plans), « calage » (où la zone se
//! pose par rapport à la coupe). Voir `docs/INTERFACE.md`.

use eframe::egui::{self, pos2, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};

use crate::app::App;
use crate::document::Document;
use crate::history::Edit;
use crate::theme;
use crate::transitions::{self, Align, Kind, Mix, Transition, Zone, MAX_FRAMES};

/// Hauteur d'une transition dans le montage, en points.
pub(crate) const BADGE_H: f32 = 16.0;
/// Largeur de chaque poignée de bord, en points.
const GRIP: f32 = 5.0;
/// Largeur minimale du nœud (une transition de 2 images en zoom arrière reste visible et se prend).
const BADGE_MIN_W: f32 = 18.0;
/// Distance de la souris à une coupe pour que le clic droit vise la coupe, en points.
const CUT_REACH: f32 = 4.0;

/// État de l'interface des transitions (un seul champ dans `App`).
#[derive(Default)]
pub(crate) struct TransUi {
    /// La transition choisie.
    pub sel: Option<u64>,
    /// Le dernier clic du montage était sur une transition : Suppr la vise.
    pub focus: bool,
    /// Un bord en train d'être tiré.
    pub drag: Option<TransDrag>,
    /// À l'arrêt, ne pas montrer le mélange à l'écran (on dessine sur le plan sans voir l'autre).
    pub hide_at_rest: bool,
}

/// Un bord de transition tiré à la souris : une seule étape d'annulation au lâcher.
pub(crate) struct TransDrag {
    id: u64,
    left: bool,
    cut: usize,
    align: Align,
    snap: Edit,
    before: Vec<Transition>,
}

/// La forme d'une transition dans le montage et ses zones de prise.
#[derive(Clone, Debug)]
pub(crate) struct Badge {
    pub id: u64,
    pub kind: Kind,
    pub zone: Zone,
    pub body: Rect,
    pub grip_left: Rect,
    pub grip_right: Rect,
}

/// Ce que la souris touche d'une transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BadgeHit {
    Body,
    GripLeft,
    GripRight,
}

/// Les formes des zones : `left` = abscisse de l'image 0, `ppf` = largeur d'une image, `mid_y` =
/// ordonnée du milieu des plans.
pub(crate) fn badges(zones: &[Zone], left: f32, ppf: f32, mid_y: f32) -> Vec<Badge> {
    zones
        .iter()
        .map(|z| {
            let (mut x0, mut x1) = (left + z.start as f32 * ppf, left + z.end as f32 * ppf);
            if x1 - x0 < BADGE_MIN_W {
                let c = (x0 + x1) * 0.5;
                (x0, x1) = (c - BADGE_MIN_W * 0.5, c + BADGE_MIN_W * 0.5);
            }
            let body = Rect::from_min_max(pos2(x0, mid_y - BADGE_H * 0.5), pos2(x1, mid_y + BADGE_H * 0.5));
            // Des poignées seulement si la forme est assez large pour qu'il reste un corps.
            let g = if x1 - x0 >= GRIP * 4.0 { GRIP } else { 0.0 };
            Badge {
                id: z.id,
                kind: z.kind,
                zone: *z,
                body,
                grip_left: Rect::from_min_max(body.min, pos2(x0 + g, body.max.y)),
                grip_right: Rect::from_min_max(pos2(x1 - g, body.min.y), body.max),
            }
        })
        .collect()
}

/// La transition sous `p`, et la partie touchée (la dernière dessinée passe avant).
pub(crate) fn badge_hit(badges: &[Badge], p: Pos2) -> Option<(usize, BadgeHit)> {
    let i = badges.iter().rposition(|b| b.body.expand2(vec2(1.0, 2.0)).contains(p))?;
    let b = &badges[i];
    let hit = if b.grip_left.width() > 0.0 && b.grip_left.contains(p) {
        BadgeHit::GripLeft
    } else if b.grip_right.width() > 0.0 && b.grip_right.contains(p) {
        BadgeHit::GripRight
    } else {
        BadgeHit::Body
    };
    Some((i, hit))
}

/// La coupe (0 à `n`) que le point `p` touche, d'après les blocs des plans : leur bord gauche, ou le
/// bord droit du dernier.
pub(crate) fn cut_at(blocks: &[Rect], p: Pos2, top: f32, bottom: f32) -> Option<usize> {
    if p.y < top || p.y > bottom {
        return None;
    }
    (0..=blocks.len()).find(|&c| {
        let x = match blocks.get(c) {
            Some(b) => b.left() - 1.0,
            None => blocks.last().map_or(f32::NAN, |b| b.right() + 1.0),
        };
        (p.x - x).abs() <= CUT_REACH
    })
}

fn with_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// Dessine une transition : un nœud papillon pour le fondu enchaîné, un losange pour le passage par
/// le noir ou le blanc, une rampe au bord du film, un carré divisé pour le volet, un bloc plein pour
/// le noir.
pub(crate) fn paint_badge(painter: &Painter, b: &Badge, selected: bool) {
    let r = b.body;
    let (x0, x1, y0, y1) = (r.left(), r.right(), r.top(), r.bottom());
    let (xc, ym) = ((x0 + x1) * 0.5, (y0 + y1) * 0.5);
    let ink = theme::texte_fort();
    let line = Stroke::new(1.0_f32, with_alpha(ink, 230));
    let edge = b.zone.out.is_none() || b.zone.inn.is_none();
    let poly = |pts: Vec<Pos2>, fill: Color32| painter.add(Shape::convex_polygon(pts, fill, line));
    match b.kind {
        Kind::Dissolve => {
            let fill = with_alpha(theme::accent_texte(), 215);
            poly(vec![pos2(x0, y0), pos2(xc, ym), pos2(x0, y1)], fill);
            poly(vec![pos2(x1, y0), pos2(xc, ym), pos2(x1, y1)], fill);
        }
        Kind::Black | Kind::White => {
            let fill = if b.kind == Kind::Black { Color32::from_rgb(10, 10, 12) } else { Color32::from_rgb(245, 245, 245) };
            if !edge {
                poly(vec![pos2(x0, ym), pos2(xc, y0), pos2(x1, ym), pos2(xc, y1)], fill);
            } else if b.zone.out.is_none() {
                // Début du film : de la couleur vers l'image.
                poly(vec![pos2(x0, y0), pos2(x1, y1), pos2(x0, y1)], fill);
            } else {
                poly(vec![pos2(x1, y0), pos2(x1, y1), pos2(x0, y1)], fill);
            }
        }
        Kind::Wipe => {
            painter.rect_filled(r, 2.0, with_alpha(theme::accent_texte(), 70));
            poly(vec![pos2(x0, y0), pos2(x1, y0), pos2(x0, y1)], with_alpha(theme::accent_texte(), 215));
            painter.rect_stroke(r, 2.0, line, StrokeKind::Inside);
        }
        Kind::CutBlack => {
            painter.rect_filled(r, 2.0, Color32::from_rgb(10, 10, 12));
            painter.rect_stroke(r, 2.0, line, StrokeKind::Inside);
        }
    }
    // Les poignées de bord : deux traits, comme les bords des plans.
    for grip in [b.grip_left, b.grip_right] {
        if grip.width() > 0.0 {
            let x = if grip == b.grip_left { grip.left() + 1.0 } else { grip.right() - 1.0 };
            painter.line_segment([pos2(x, y0 + 2.0), pos2(x, y1 - 2.0)], Stroke::new(2.0_f32, with_alpha(ink, 200)));
        }
    }
    if selected {
        painter.rect_stroke(r.expand(2.0), 3.0, Stroke::new(2.0_f32, theme::contour()), StrokeKind::Outside);
    }
}

/// « Fondu enchaîné · 12 images, centrée sur la coupe » : la bulle d'une transition.
pub(crate) fn badge_tip(b: &Badge, t: Option<&Transition>) -> String {
    let n = b.zone.len();
    match t {
        Some(t) if t.between_plans() => format!("{} · {} image{}, {}", b.kind.name(), n, if n > 1 { "s" } else { "" }, t.align.name().to_lowercase()),
        _ => format!("{} · {} image{}", b.kind.name(), n, if n > 1 { "s" } else { "" }),
    }
}

/// Le texte de la barre d'état pour une transition posée.
fn describe(doc: &Document, id: u64) -> String {
    match doc.transition(id) {
        Some(t) => format!("{} de {} image{} (Suppr pour la retirer, clic droit pour la changer).", t.kind.name(), t.frames, if t.frames > 1 { "s" } else { "" }),
        None => String::new(),
    }
}

impl App {
    // ------------------------------------------------------------------ opérations

    /// Une opération sur les transitions, avec une étape d'annulation nommée `name`. `op` renvoie
    /// faux si rien n'a changé.
    fn trans_op(&mut self, name: &str, op: impl FnOnce(&mut Document) -> bool) -> bool {
        self.end_stroke();
        let snap = self.snapshot();
        if !op(&mut self.doc) {
            return false;
        }
        let before = std::mem::replace(&mut self.history.context, Some(name.to_string()));
        self.commit_structure(snap);
        self.history.context = before;
        true
    }

    /// Pose ou change la transition de la coupe `cut` ; la rend choisie.
    pub(crate) fn set_transition(&mut self, cut: usize, kind: Kind) {
        let between = {
            let (o, i) = transitions::cut_plans(&self.doc, cut);
            o.is_some() && i.is_some()
        };
        if !kind.allowed(between) {
            self.status = format!("« {} » ne se pose qu'entre deux plans : au bord du film, choisis un fondu au noir ou au blanc.", kind.name());
            return;
        }
        let existed = {
            let (o, i) = transitions::cut_plans(&self.doc, cut);
            self.doc.transition_on(o, i).map(|t| t.kind)
        };
        let name = match existed {
            Some(_) => format!("Transition : {}", kind.name().to_lowercase()),
            None => format!("Ajouter une transition : {}", kind.name().to_lowercase()),
        };
        let mut id = None;
        if self.trans_op(&name, |d| {
            id = d.set_transition(cut, kind);
            existed != Some(kind) && id.is_some()
        }) || id.is_some()
        {
            self.trans.sel = id;
            self.trans.focus = true;
            if let Some(id) = id {
                self.status = describe(&self.doc, id);
            }
        }
    }

    pub(crate) fn delete_transition(&mut self, id: u64) {
        if self.trans_op("Supprimer la transition", |d| d.remove_transition(id)) {
            if self.trans.sel == Some(id) {
                self.trans.sel = None;
            }
            self.status = "Transition supprimée.".into();
        }
    }

    pub(crate) fn set_transition_align(&mut self, id: u64, align: Align) {
        self.trans_op("Calage de la transition", |d| match d.transitions.iter_mut().find(|t| t.id == id) {
            Some(t) if t.align != align => {
                t.align = align;
                true
            }
            _ => false,
        });
    }

    /// La coupe visée par une commande : celle de la transition choisie, sinon la plus proche de la
    /// tête de lecture.
    fn target_cut(&self) -> usize {
        if let Some(t) = self.trans.sel.and_then(|id| self.doc.transition(id)) {
            if let Some(c) = self.cut_of(t) {
                return c;
            }
        }
        self.doc.nearest_cut(self.global_frame())
    }

    /// Le numéro de la coupe qu'habille `t`, si ses plans se suivent.
    fn cut_of(&self, t: &Transition) -> Option<usize> {
        (0..=self.doc.panels.len()).find(|&c| {
            let (o, i) = transitions::cut_plans(&self.doc, c);
            (o, i) == (t.out, t.inn)
        })
    }

    // ------------------------------------------------------------------ commandes

    /// Les commandes `transition.*` ; vrai si `action` en était une.
    pub(crate) fn transition_action(&mut self, action: &str) -> bool {
        let kind = match action {
            "transition.ajouter" => {
                let cut = self.doc.nearest_cut(self.global_frame());
                let (o, i) = transitions::cut_plans(&self.doc, cut);
                match self.doc.transition_on(o, i) {
                    Some(t) => {
                        // Déjà une transition ici : on la choisit, et la palette dit où elle est.
                        let id = t.id;
                        self.trans.sel = Some(id);
                        self.trans.focus = true;
                        self.status = format!("Il y a déjà une transition ici. {}", describe(&self.doc, id));
                    }
                    None => {
                        let kind = if o.is_some() && i.is_some() { Kind::Dissolve } else { Kind::Black };
                        self.set_transition(cut, kind);
                    }
                }
                return true;
            }
            "transition.supprimer" => {
                let id = self.trans.sel.filter(|&id| self.doc.transition(id).is_some()).or_else(|| {
                    let (o, i) = transitions::cut_plans(&self.doc, self.doc.nearest_cut(self.global_frame()));
                    self.doc.transition_on(o, i).map(|t| t.id)
                });
                match id {
                    Some(id) => self.delete_transition(id),
                    None => self.status = "Aucune transition à supprimer près de la tête de lecture.".into(),
                }
                return true;
            }
            "transition.apercu" => {
                self.trans.hide_at_rest = !self.trans.hide_at_rest;
                self.status = format!("Aperçu des transitions à l'arrêt : {}.", if self.trans.hide_at_rest { "non" } else { "oui" });
                return true;
            }
            // Suppr : la transition choisie, seulement si le dernier clic du montage l'a visée.
            "edition.supprimer" => {
                // (`panel_focus` : le montage a la main ; un clic dans le dessin le lui reprend.)
                if self.trans.focus && self.panel_focus {
                    if let Some(id) = self.trans.sel.filter(|&id| self.doc.transition(id).is_some()) {
                        self.delete_transition(id);
                        return true;
                    }
                }
                return false;
            }
            "transition.fondu_enchaine" => Kind::Dissolve,
            "transition.fondu_noir" => Kind::Black,
            "transition.fondu_blanc" => Kind::White,
            "transition.volet" => Kind::Wipe,
            "transition.noir" => Kind::CutBlack,
            _ => return false,
        };
        let cut = self.target_cut();
        self.set_transition(cut, kind);
        true
    }

    /// Le bouton « Ajouter une transition » de la barre du montage (un seul appel, pour que la barre
    /// puisse le ranger où elle veut).
    pub(crate) fn transition_button(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        if self
            .bouton(
                ui,
                ph::GRADIENT,
                "Ajouter une transition",
                "À la coupe la plus proche de la tête de lecture : un fondu enchaîné entre deux plans, un fondu au noir au bord du film. Clic droit sur une coupe pour choisir la sorte.",
                "transition.ajouter",
            )
            .clicked()
        {
            self.run_action("transition.ajouter");
        }
    }

    // ------------------------------------------------------------------ menus

    /// Le menu du clic droit sur une transition.
    pub(crate) fn transition_menu(&mut self, ui: &mut egui::Ui, id: u64) {
        let Some(t) = self.doc.transition(id).cloned() else {
            ui.close_menu();
            return;
        };
        let Some(cut) = self.cut_of(&t) else {
            ui.label("Les plans de cette transition ne se suivent plus.");
            if ui.button("Supprimer la transition").clicked() {
                ui.close_menu();
                self.delete_transition(id);
            }
            return;
        };
        self.trans.sel = Some(id);
        for kind in Kind::ALL.into_iter().filter(|k| k.allowed(t.between_plans())) {
            let resp = ui.selectable_label(t.kind == kind, kind.name());
            if crate::bulles::bulle(resp, kind.name(), kind.description(), "").clicked() {
                ui.close_menu();
                self.set_transition(cut, kind);
            }
        }
        ui.separator();
        let mut frames = t.frames;
        let resp = ui.add(egui::DragValue::new(&mut frames).range(1..=MAX_FRAMES).suffix(" images").prefix("Durée "));
        if resp.changed() {
            // Une seule étape pour tout le geste (glisser, taper).
            let before = std::mem::replace(&mut self.history.context, Some("Durée de la transition".into()));
            let edited = self.edited(&resp);
            self.history.context = before;
            if edited {
                if let Some(t) = self.doc.transitions.iter_mut().find(|t| t.id == id) {
                    t.frames = frames.clamp(1, MAX_FRAMES);
                }
            }
        }
        if t.between_plans() {
            ui.separator();
            for align in Align::ALL {
                if ui.selectable_label(t.align == align, align.name()).clicked() {
                    ui.close_menu();
                    self.set_transition_align(id, align);
                }
            }
        }
        ui.separator();
        let del = self.keymap.label("edition.supprimer");
        if ui.add(egui::Button::new("Supprimer la transition").shortcut_text(del)).clicked() {
            ui.close_menu();
            self.delete_transition(id);
        }
    }

    /// Le menu du clic droit sur une coupe : « Transition ▸ ».
    pub(crate) fn cut_menu(&mut self, ui: &mut egui::Ui, cut: usize) {
        self.cut_menu_named(ui, cut, "Transition");
    }

    /// Le même sous-menu, sous un autre titre (menu d'un plan : début, fin).
    pub(crate) fn cut_menu_named(&mut self, ui: &mut egui::Ui, cut: usize, title: &str) {
        let (o, i) = transitions::cut_plans(&self.doc, cut);
        let between = o.is_some() && i.is_some();
        let here = self.doc.transition_on(o, i).cloned();
        ui.menu_button(title, |ui| {
            for kind in Kind::ALL.into_iter().filter(|k| k.allowed(between)) {
                let on = here.as_ref().is_some_and(|t| t.kind == kind);
                let resp = ui.selectable_label(on, kind.name());
                if crate::bulles::bulle(resp, kind.name(), kind.description(), "").clicked() {
                    ui.close_menu();
                    self.set_transition(cut, kind);
                }
            }
            if let Some(t) = &here {
                ui.separator();
                if ui.button("Supprimer la transition").clicked() {
                    ui.close_menu();
                    self.delete_transition(t.id);
                }
            }
        });
        if let Some(t) = &here {
            if ui.button("Modifier la transition…").clicked() {
                // (Le même menu, ouvert sur la transition : on la choisit, le clic droit sur sa forme règle.)
                ui.close_menu();
                self.trans.sel = Some(t.id);
                self.status = describe(&self.doc, t.id);
            }
        }
    }

    // ------------------------------------------------------------------ montage

    /// Les formes des transitions de la bande de montage.
    pub(crate) fn trans_badges(&self, left: f32, ppf: f32, mid_y: f32) -> Vec<Badge> {
        if self.doc.transitions.is_empty() {
            return Vec::new();
        }
        badges(&transitions::zones(&self.doc), left, ppf, mid_y)
    }

    pub(crate) fn paint_transitions(&self, painter: &Painter, badges: &[Badge]) {
        for b in badges {
            if painter.clip_rect().intersects(b.body) {
                paint_badge(painter, b, self.trans.sel == Some(b.id));
            }
        }
    }

    /// Le dernier appui de la souris dans la bande : le focus de Suppr suit.
    pub(crate) fn trans_track_focus(&mut self, press: Option<Pos2>, badges: &[Badge]) {
        self.trans.focus = press.is_some_and(|p| badge_hit(badges, p).is_some());
    }

    /// Début d'un glisser : un bord de transition se prend. Vrai si la transition a pris le geste.
    pub(crate) fn trans_begin_drag(&mut self, origin: Pos2, badges: &[Badge]) -> bool {
        let Some((i, hit)) = badge_hit(badges, origin) else { return false };
        let b = &badges[i];
        self.trans.sel = Some(b.id);
        self.trans.focus = true;
        self.cancel_edge_selection();
        if hit == BadgeHit::Body || self.doc.storyboard_locked {
            return true;
        }
        self.playing = false;
        self.trans.drag = Some(TransDrag {
            id: b.id,
            left: hit == BadgeHit::GripLeft,
            cut: b.zone.cut,
            align: self.doc.transition(b.id).map_or(Align::Centre, |t| t.align),
            snap: self.snapshot(),
            before: self.doc.transitions.clone(),
        });
        true
    }

    /// Le bord tiré suit la souris (image `g` du film sous le pointeur).
    pub(crate) fn trans_drag_to(&mut self, g: usize) {
        let Some(d) = &self.trans.drag else { return };
        let n = d.align.frames_for_edge(d.cut, g, d.left);
        let id = d.id;
        if let Some(t) = self.doc.transitions.iter_mut().find(|t| t.id == id) {
            t.frames = n;
            self.status = format!("{} : {} image{}.", t.kind.name(), n, if n > 1 { "s" } else { "" });
        }
    }

    /// Fin du geste : une étape d'annulation, si la durée a changé.
    pub(crate) fn trans_end_drag(&mut self) {
        let Some(d) = self.trans.drag.take() else { return };
        if self.doc.transitions != d.before {
            let before = std::mem::replace(&mut self.history.context, Some("Durée de la transition".into()));
            self.history.push(d.snap);
            self.history.context = before;
            self.modified = true;
        }
    }

    /// Échap, annulation ou changement de mode pendant le geste : la durée revient.
    pub(crate) fn trans_cancel_drag(&mut self) {
        if let Some(d) = self.trans.drag.take() {
            self.doc.transitions = d.before;
        }
    }

    /// Clic sur une transition : elle est choisie, la tête de lecture va à sa coupe. Vrai si la
    /// transition a pris le clic.
    pub(crate) fn trans_click(&mut self, p: Pos2, badges: &[Badge]) -> bool {
        let Some((i, _)) = badge_hit(badges, p) else { return false };
        let b = &badges[i];
        self.trans.sel = Some(b.id);
        self.trans.focus = true;
        self.goto_global(b.zone.cut.min(self.doc.total_frames() - 1));
        self.status = describe(&self.doc, b.id);
        true
    }

    /// Ce que vise un clic droit : la forme d'une transition (son menu), ou une coupe entre deux plans
    /// (« Transition ▸ »).
    pub(crate) fn trans_menu_target(&mut self, p: Pos2, badges: &[Badge], blocks: &[Rect], top: f32, bottom: f32) -> Option<crate::soundtrack::MenuTarget> {
        use crate::soundtrack::MenuTarget;
        if let Some((i, _)) = badge_hit(badges, p) {
            self.trans.sel = Some(badges[i].id);
            self.trans.focus = true;
            return Some(MenuTarget::Transition(badges[i].id));
        }
        cut_at(blocks, p, top, bottom).map(MenuTarget::Cut)
    }

    /// Les bords de plans (rognage) ne gardent pas leur choix quand on prend une transition.
    fn cancel_edge_selection(&mut self) {
        self.edge_sel = None;
    }

    // ------------------------------------------------------------------ aperçu à l'écran

    /// Le mélange à montrer à l'écran pour l'image courante du montage, s'il y a lieu : pendant la
    /// lecture, et à l'arrêt tant que l'aperçu n'est pas coupé (`transition.apercu`). Jamais pendant
    /// un trait, une sélection déplacée ou en mode Animation (on dessine le plan, pas le film).
    pub(crate) fn transition_preview(&self) -> Option<(Zone, Mix)> {
        if self.doc.transitions.is_empty() || self.stroke.is_some() {
            return None;
        }
        if !self.playing && (self.trans.hide_at_rest || self.mode == crate::app::Mode::Animation) {
            return None;
        }
        self.doc.mix_at(self.global_frame())
    }

    /// Ce que le dessin superpose pendant une transition : le plan sortant (ou le seul plan visible),
    /// l'entrant s'il y en a un, où est la feuille de chacun à l'écran (en vue caméra, chaque plan est
    /// vu par sa propre caméra), et comment les mêler. `current` : le plan de l'image courante,
    /// déjà construit (avec ce que l'écran y ajoute) ; les autres viennent de `panel_plan_at`, comme
    /// pour l'export.
    pub(crate) fn mix_setup(&self, mix: &Mix, current: &crate::render::Plan, rect: Rect, size: egui::Vec2) -> MixSetup {
        let is_current = |(p, f): (usize, usize)| p == self.cur_panel && f == self.cur_frame;
        let plan_of = |at: (usize, usize)| {
            if is_current(at) {
                current.clone()
            } else {
                crate::render::panel_plan_at(&self.doc, at.0, at.1, self.doc.render_paper())
            }
        };
        let stage = [pos2(0.0, 0.0), pos2(size.x, 0.0), pos2(size.x, size.y), pos2(0.0, size.y)];
        let place = |at: (usize, usize)| {
            let view = if self.camera_view && !is_current(at) {
                crate::camtools::view_through(&self.doc.panel_camera(at.0, at.1), self.doc.frame, size, rect)
            } else {
                self.view
            };
            let h = stage.map(|c| view.to_screen_h(rect, size, c));
            (h.map(|c| c.0), view.persp.is_some().then(|| h.map(|c| c.1)))
        };
        let layer = |at: (usize, usize)| {
            let (corners, ws) = place(at);
            (plan_of(at), corners, ws)
        };
        let (first, second) = match (mix.a, mix.b) {
            (Some(a), Some(b)) => (layer(a), Some(layer(b))),
            (Some(a), None) => (layer(a), None),
            (None, Some(b)) => (layer(b), None),
            // (Une zone sans plan n'existe pas : `Zone::mix` n'en rend pas.)
            (None, None) => (layer((self.cur_panel, self.cur_frame)), None),
        };
        let fr = self.doc.frame_rect();
        let sheet = self.doc.width.max(1) as f32;
        MixSetup {
            first: first.0,
            first_corners: first.1,
            first_ws: first.2,
            second,
            display: crate::gpu::DisplayMix {
                wipe: mix.wipe,
                m: mix.m,
                grey: mix.color[0] as f32 / 255.0,
                k: mix.k,
                wipe_u: [fr.left() / sheet, fr.right() / sheet],
            },
        }
    }
}

/// Le résultat de `mix_setup`.
pub(crate) struct MixSetup {
    pub first: crate::render::Plan,
    pub first_corners: [Pos2; 4],
    pub first_ws: Option<[f32; 4]>,
    pub second: Option<(crate::render::Plan, [Pos2; 4], Option<[f32; 4]>)>,
    pub display: crate::gpu::DisplayMix,
}
