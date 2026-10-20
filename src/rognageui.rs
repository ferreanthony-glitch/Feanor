//! Rognage dans le montage (interface, Anthony, 2026-10-19 : « Je ne peux pas saisir le début d'un
//! shot pour le raccourcir en l'état ») : les bords des plans se prennent à la souris. Le modèle
//! (réserve, ripple, roll, glisser) est dans `rognage.rs` ; ici les gestes, la précision au clavier,
//! les commandes et la fenêtre « Durée… ».
//!
//! - **Début** : on prend le bord gauche d'un plan. Vers la droite, le plan perd des images au
//!   début (elles vont dans la réserve) ; vers la gauche, il les retrouve, puis tient son premier
//!   dessin. **Fin** : même chose au bord droit (c'était déjà là, mais détruisait ce qu'on
//!   coupait).
//! - **Ripple** par défaut : les plans suivants se serrent ou s'écartent d'autant, sans trou.
//! - **Roll** (Alt + glisser sur le bord, entre deux plans) : la coupe se déplace, l'un raccourcit,
//!   l'autre s'allonge, le total ne change pas. « Alt » est libre dans le montage (le canevas s'en
//!   sert pour la pipette) ; la touche est un choix de SHOT, à vérifier à l'usage.
//! - **Glisser le contenu** (Alt + glisser dans un plan, ou Ctrl + Alt) : le plan garde sa durée
//!   et sa place, c'est ce qu'il montre qui avance ou recule dans la réserve.
//! - **Aimantation** (aimant dans la barre du montage) : un bord se colle, à quelques pixels près,
//!   à la tête de lecture, aux autres coupes et aux clés de caméra et d'effets.
//! - **Précision** : un clic sur un bord le choisit (surligné) ; les flèches gauche et droite le
//!   déplacent d'une image (clic avec Alt : la coupe entre deux plans). « Durée… » (menu du plan)
//!   donne la durée exacte, en images ou en temps.
//!
//! Chaque geste est une étape d'annulation, nommée (« Raccourcir le début », « Allonger la fin »,
//! « Déplacer la coupe », « Glisser le contenu »), et refusé quand le storyboard est verrouillé.
//! Échap rend tout comme avant.

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, Stroke};

use crate::app::App;
use crate::commentaires::Comment;
use crate::document::{Document, Panel};
use crate::history::Edit;
use crate::rognage::{noms, parse_duration, snap_points, snap_to, Reserve, Rogner};
use crate::storyboard::{duration_label, MontageDrag};

/// À quelle distance (points d'écran) un bord s'aimante.
pub(crate) const SNAP_PX: f32 = 6.0;

/// Le bord d'un plan, ou la coupe entre deux plans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Side {
    /// Le bord gauche du plan : son début.
    Head,
    /// Le bord droit : sa fin.
    Tail,
    /// La coupe entre ce plan et le suivant (roll).
    Cut,
}

/// Ce qu'un geste fait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EdgeKind {
    Head,
    Tail,
    Roll,
    Slip,
}

/// Un plan tel qu'il était : le plan, sa réserve et ses commentaires (qui sont rangés à part).
#[derive(Clone)]
pub(crate) struct PlanBackup {
    pub panel: Panel,
    reserve: Option<Reserve>,
    comments: Vec<Comment>,
}

impl PlanBackup {
    pub(crate) fn capture(doc: &Document, pi: usize) -> PlanBackup {
        let panel = doc.panels[pi].clone();
        let id = panel.id;
        PlanBackup {
            reserve: doc.reserves.get(&id).cloned(),
            comments: doc.comments.iter().filter(|c| c.panel == id).cloned().collect(),
            panel,
        }
    }

    /// Remet le plan comme il était. Faux si le plan n'existe plus.
    pub(crate) fn restore(&self, doc: &mut Document) -> bool {
        let id = self.panel.id;
        let Some(pi) = doc.panels.iter().position(|p| p.id == id) else { return false };
        doc.panels[pi] = self.panel.clone();
        match &self.reserve {
            Some(r) => {
                doc.reserves.insert(id, r.clone());
            }
            None => {
                doc.reserves.remove(&id);
            }
        }
        doc.comments.retain(|c| c.panel != id);
        doc.comments.extend(self.comments.iter().cloned());
        true
    }
}

/// Un bord (ou le contenu) pris à la souris. Chaque mouvement repart des plans d'avant le geste :
/// rien n'est perdu tant qu'on n'a pas relâché.
pub(crate) struct EdgeDrag {
    pub kind: EdgeKind,
    /// Les plans touchés tels qu'ils étaient (deux pour un roll : le premier puis le suivant).
    backups: Vec<PlanBackup>,
    origin_x: f32,
    /// Image du film où est le bord pris, avant le geste.
    edge0: usize,
    /// Où le bord s'aimante (images du film, avant le geste) ; vide : pas d'aimantation.
    snaps: Vec<usize>,
    /// L'étape d'annulation de tout le geste.
    snap: Edit,
    /// Dernier décalage demandé, et ce qui en a été fait.
    asked: Option<isize>,
    pub applied: isize,
    /// Ce que dit le geste (barre d'état et étiquette près du bord).
    pub text: String,
    /// L'image du film (dans le film d'avant le geste) où se fait la coupe, pour le repère.
    pub ghost: Option<usize>,
}

/// La fenêtre « Durée… ».
pub(crate) struct DurationDialog {
    panel: u64,
    text: String,
    focus: bool,
}

/// Le bord (ou la coupe) que les flèches déplacent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EdgeSel {
    pub panel: u64,
    pub side: Side,
}

/// Le bord d'un plan sous l'abscisse `x` : `blocks` sont les blocs des plans dans la bande. La zone
/// de prise est de cinq points de chaque côté, moins pour un plan étroit (pour qu'on le prenne
/// encore par le milieu) ; entre deux plans, c'est la coupe la plus proche qui gagne (à égalité,
/// la fin du plan de gauche, comme avant).
pub(crate) fn edge_hit(blocks: &[Rect], x: f32) -> Option<(usize, Side)> {
    let mut best: Option<(f32, usize, Side)> = None;
    for (i, b) in blocks.iter().enumerate() {
        let tol = (b.width() * 0.4).clamp(2.0, 5.0);
        for (side, edge) in [(Side::Tail, b.right()), (Side::Head, b.left())] {
            let d = (x - edge).abs();
            if d <= tol && best.is_none_or(|(bd, _, _)| d < bd) {
                best = Some((d, i, side));
            }
        }
    }
    best.map(|(_, i, s)| (i, s))
}

/// De combien d'images un bord pris à `edge0` (image du film) a bougé quand la souris a fait
/// `moved` images depuis la prise : à l'image près, ou collé à un point de `snaps` à `within`
/// images près. Le contenu glissé (`Slip`) suit la souris à l'image près, sans aimantation.
pub(crate) fn drag_delta(kind: EdgeKind, edge0: usize, moved: f32, snaps: &[usize], within: f32) -> isize {
    if kind == EdgeKind::Slip {
        return moved.round() as isize;
    }
    let target = edge0 as f32 + moved;
    let pos = snap_to(snaps, target, within).map_or(target.round() as isize, |p| p as isize);
    pos - edge0 as isize
}

fn s(n: usize) -> &'static str {
    if n > 1 {
        "s"
    } else {
        ""
    }
}

impl App {
    // ------------------------------------------------------------------ gestes

    /// Prend un bord (`kind`) du plan `pi` (pour un roll : la coupe entre `pi` et le suivant) à
    /// l'abscisse `origin_x` de la bande. Refusé sous le verrou du storyboard.
    pub(crate) fn begin_edge_drag(&mut self, kind: EdgeKind, pi: usize, origin_x: f32) -> Option<MontageDrag> {
        if self.storyboard_refuses() || pi >= self.doc.panels.len() {
            return None;
        }
        if kind == EdgeKind::Roll && pi + 1 >= self.doc.panels.len() {
            return None;
        }
        self.end_stroke();
        self.playing = false;
        self.set_panel(pi);
        let start = self.doc.panel_start(pi);
        let len = self.doc.panels[pi].frame_count();
        let edge0 = match kind {
            EdgeKind::Head => start,
            EdgeKind::Tail => start + len,
            EdgeKind::Roll => start + len,
            EdgeKind::Slip => start,
        };
        let mut backups = vec![PlanBackup::capture(&self.doc, pi)];
        if kind == EdgeKind::Roll {
            backups.push(PlanBackup::capture(&self.doc, pi + 1));
        }
        let snaps = if self.snap_on && kind != EdgeKind::Slip { snap_points(&self.doc, self.global_frame()) } else { Vec::new() };
        Some(MontageDrag::Edge(EdgeDrag {
            kind,
            backups,
            origin_x,
            edge0,
            snaps,
            snap: self.snapshot(),
            asked: None,
            applied: 0,
            text: String::new(),
            ghost: None,
        }))
    }

    /// Le bord tenu suit la souris (abscisse `x` de la bande, `ppf` points par image).
    pub(crate) fn update_edge_drag(&mut self, x: f32) {
        let ppf = self.montage_zoom.max(0.01);
        let Some(MontageDrag::Edge(mut d)) = self.montage_drag.take() else { return };
        let delta = drag_delta(d.kind, d.edge0, (x - d.origin_x) / ppf, &d.snaps, SNAP_PX / ppf);
        if d.asked != Some(delta) {
            d.asked = Some(delta);
            for b in &d.backups {
                b.restore(&mut self.doc);
            }
            let first = d.backups[0].panel.id;
            if let Some(pi) = self.doc.panels.iter().position(|p| p.id == first) {
                let old_len = d.backups[0].panel.frame_count();
                let op = match d.kind {
                    EdgeKind::Head => Rogner::Head(delta),
                    EdgeKind::Tail => Rogner::Tail(delta),
                    EdgeKind::Roll => Rogner::Roll(delta),
                    EdgeKind::Slip => Rogner::Slip(delta),
                };
                d.applied = self.doc.rogner(pi, op).unwrap_or(0);
                d.ghost = match d.kind {
                    EdgeKind::Slip => None,
                    _ => Some((d.edge0 as isize + d.applied).max(0) as usize),
                };
                d.text = self.edge_text(d.kind, pi, d.applied, old_len);
                self.status = d.text.clone();
                self.renderer.invalidate_all();
            }
        }
        self.montage_drag = Some(MontageDrag::Edge(d));
    }

    /// La phrase d'un geste de rognage : la nouvelle durée et ce qui est caché.
    fn edge_text(&self, kind: EdgeKind, pi: usize, applied: isize, old_len: usize) -> String {
        let fps = self.doc.fps;
        let n = self.doc.panels[pi].frame_count();
        let (h, t) = self.doc.hidden(pi);
        let hidden = |h: usize, t: usize| match (h, t) {
            (0, 0) => String::new(),
            (h, 0) => format!(" · début rogné de {h} i"),
            (0, t) => format!(" · fin rognée de {t} i"),
            (h, t) => format!(" · début rogné de {h} i, fin rognée de {t} i"),
        };
        let title = crate::storyboard::panel_title(&self.doc, pi);
        let dur = format!("{} ({n} i)", duration_label(n, fps));
        // Les plans suivants se décalent par rapport au son, qui reste en place (chantier 4).
        let with_sound = |base: String| match crate::handles::sound_shift_note(&self.doc, pi, old_len, n) {
            Some(note) => format!("{base}. {note}"),
            None => base,
        };
        let a = applied.unsigned_abs();
        match kind {
            EdgeKind::Head => {
                let what = if applied >= 0 { "raccourci" } else { "allongé" };
                with_sound(format!("{title}, début {what} de {a} i : durée {dur}{}", hidden(h, t)))
            }
            EdgeKind::Tail => {
                let what = if applied >= 0 { "allongée" } else { "raccourcie" };
                with_sound(format!("{title}, fin {what} de {a} i : durée {dur}{}", hidden(h, t)))
            }
            EdgeKind::Roll => {
                let m = self.doc.panels.get(pi + 1).map_or(0, Panel::frame_count);
                format!("Coupe {:+} i : {n} i + {m} i = {} i (le total ne change pas)", applied, n + m)
            }
            EdgeKind::Slip => {
                if applied == 0 && h == 0 && t == 0 {
                    "Rien en réserve : raccourcis d'abord le début ou la fin du plan, puis fais glisser son contenu.".into()
                } else {
                    format!("{title}, contenu glissé de {:+} i : durée {dur} inchangée{}", applied, hidden(h, t))
                }
            }
        }
    }

    /// Lâche le bord : une étape d'annulation nommée si quelque chose a changé.
    pub(crate) fn finish_edge_drag(&mut self, d: EdgeDrag) {
        if d.applied == 0 {
            for b in &d.backups {
                b.restore(&mut self.doc);
            }
            self.renderer.invalidate_all();
            return;
        }
        let name = match d.kind {
            EdgeKind::Head if d.applied > 0 => noms::DEBUT_COURT,
            EdgeKind::Head => noms::DEBUT_LONG,
            EdgeKind::Tail if d.applied > 0 => noms::FIN_LONGUE,
            EdgeKind::Tail => noms::FIN_COURTE,
            EdgeKind::Roll => noms::COUPE,
            EdgeKind::Slip => noms::GLISSER,
        };
        self.after_rognage();
        self.commit_as(d.snap, name);
    }

    /// Échap (ou une annulation) pendant le geste : les plans reprennent leur état.
    pub(crate) fn cancel_edge_drag(&mut self, d: EdgeDrag) {
        for b in &d.backups {
            b.restore(&mut self.doc);
        }
        self.renderer.invalidate_all();
    }

    /// Après un rognage : ce qui visait des images du plan (choix de clés, d'images) ne vise plus
    /// rien de sûr, la tête reste dans le plan.
    pub(crate) fn after_rognage(&mut self) {
        self.vsel = None;
        self.frame_sel = None;
        self.clear_cam_keys();
        self.key_sel.clear();
        self.fx_key_sel.clear();
        let last = self.frame_count().saturating_sub(1);
        if self.mode == crate::app::Mode::Storyboard {
            self.cur_frame = self.cur_frame.min(last);
        }
        self.renderer.invalidate_all();
    }

    /// Repère de la coupe et étiquette près du bord pendant le geste. `blocks` : les blocs des
    /// plans (après le geste), `x_of` : abscisse d'une image du film, `top` : haut des blocs.
    pub(crate) fn paint_edge_drag(&self, painter: &egui::Painter, blocks: &[Rect], x_of: &dyn Fn(usize) -> f32, top: f32, bottom: f32, accent: Color32) {
        let Some(MontageDrag::Edge(d)) = &self.montage_drag else { return };
        if let Some(g) = d.ghost {
            let x = x_of(g);
            painter.line_segment([pos2(x, top - 4.0), pos2(x, bottom + 4.0)], Stroke::new(1.5_f32, accent));
            painter.circle_filled(pos2(x, top - 4.0), 3.0, accent);
        }
        if d.text.is_empty() {
            return;
        }
        let pi = self.doc.panels.iter().position(|p| p.id == d.backups[0].panel.id).unwrap_or(0);
        let Some(b) = blocks.get(pi) else { return };
        let anchor = match d.kind {
            EdgeKind::Head => pos2(b.left(), top - 8.0),
            EdgeKind::Tail | EdgeKind::Roll => pos2(b.right(), top - 8.0),
            EdgeKind::Slip => pos2(b.center().x, top - 8.0),
        };
        let galley = painter.layout_no_wrap(d.text.clone(), FontId::proportional(11.0), Color32::WHITE);
        let size = galley.size() + vec2(10.0, 4.0);
        let mut rect = Rect::from_min_size(pos2(anchor.x - size.x * 0.5, anchor.y - size.y), size);
        if rect.left() < painter.clip_rect().left() {
            rect = rect.translate(vec2(painter.clip_rect().left() - rect.left() + 2.0, 0.0));
        }
        painter.rect_filled(rect, 3.0, Color32::from_black_alpha(215));
        painter.galley(rect.min + vec2(5.0, 2.0), galley, Color32::WHITE);
    }

    // ------------------------------------------------------------------ bord choisi, flèches

    /// Un clic sur un bord le choisit (Alt : la coupe entre le plan et le suivant).
    pub(crate) fn select_edge(&mut self, pi: usize, side: Side) {
        let Some(id) = self.doc.panels.get(pi).map(|p| p.id) else { return };
        self.edge_sel = Some(EdgeSel { panel: id, side });
        self.status = match side {
            Side::Head => "Début du plan choisi : flèches gauche / droite pour le déplacer d'une image (Alt + clic : la coupe).".into(),
            Side::Tail => "Fin du plan choisie : flèches gauche / droite pour la déplacer d'une image (Alt + clic : la coupe).".into(),
            Side::Cut => "Coupe choisie : flèches gauche / droite pour la déplacer d'une image, le total ne change pas.".into(),
        };
    }

    /// Les flèches visent le bord choisi : il existe encore et le montage a la main.
    pub(crate) fn edge_nudge_ready(&self) -> bool {
        self.panel_focus && self.edge_sel.is_some_and(|e| self.doc.panels.iter().any(|p| p.id == e.panel))
    }

    /// Déplace le bord choisi d'une image (`dir` : -1 vers la gauche, 1 vers la droite).
    pub(crate) fn nudge_edge(&mut self, dir: isize) {
        let Some(sel) = self.edge_sel else { return };
        let Some(pi) = self.doc.panels.iter().position(|p| p.id == sel.panel) else {
            self.edge_sel = None;
            return;
        };
        // Vers la droite : le début se raccourcit, la fin s'allonge, la coupe avance.
        let (op, name) = match sel.side {
            Side::Head => (Rogner::Head(dir), if dir > 0 { noms::DEBUT_COURT } else { noms::DEBUT_LONG }),
            Side::Tail => (Rogner::Tail(dir), if dir > 0 { noms::FIN_LONGUE } else { noms::FIN_COURTE }),
            Side::Cut => (Rogner::Roll(dir), noms::COUPE),
        };
        if let Some(done) = self.rogner_plan(pi, op, name) {
            let n = self.doc.panels[pi].frame_count();
            let (h, t) = self.doc.hidden(pi);
            self.status = if done == 0 {
                "Le bord ne peut pas aller plus loin (un plan garde au moins une image).".into()
            } else {
                format!("{}, {name} : {} ({n} i) · {h} i cachées au début, {t} à la fin.", crate::storyboard::panel_title(&self.doc, pi), duration_label(n, self.doc.fps))
            };
        }
    }

    // ------------------------------------------------------------------ commandes

    /// Fait `op` sur le plan `pi` en une étape d'annulation nommée `name`. `None` : refusé par le
    /// verrou du storyboard (la phrase est dans la barre d'état).
    pub(crate) fn rogner_plan(&mut self, pi: usize, op: Rogner, name: &str) -> Option<isize> {
        if self.storyboard_refuses() {
            return None;
        }
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        let done = self.doc.rogner(pi, op).ok()?;
        if done != 0 {
            self.after_rognage();
            self.commit_as(snap, name);
        }
        Some(done)
    }

    /// Le plan visé par une commande de rognage et la tête de lecture : celui d'un clic droit dans
    /// le montage (la tête de lecture d'alors), sinon le plan courant.
    fn trim_target(&mut self) -> (usize, usize) {
        match self.trim_menu.take() {
            Some((id, g)) => match self.doc.panels.iter().position(|p| p.id == id) {
                Some(pi) => (pi, g),
                None => (self.cur_panel, self.global_frame()),
            },
            None => (self.cur_panel, self.global_frame()),
        }
    }

    /// « Raccourcir le début jusqu'à la tête de lecture » : l'image sous la tête devient la
    /// première du plan (la coupe est au trait de la tête de lecture, au bord gauche de l'image).
    pub(crate) fn trim_head_to_playhead(&mut self) {
        let (pi, g) = self.trim_target();
        let start = self.doc.panel_start(pi);
        let len = self.doc.panels[pi].frame_count();
        if g == start {
            self.status = "La tête de lecture est sur la première image du plan : rien à raccourcir au début.".into();
            return;
        }
        if g < start || g >= start + len {
            self.status = "Place la tête de lecture dans ce plan, après sa première image.".into();
            return;
        }
        let cut = (g - start) as isize;
        if let Some(done) = self.rogner_plan(pi, Rogner::Head(cut), noms::DEBUT_COURT) {
            let n = self.doc.panels[pi].frame_count();
            self.status = format!("Début raccourci de {done} i jusqu'à la tête de lecture : durée {} ({n} i). Les images restent en réserve.", duration_label(n, self.doc.fps));
            // La tête reste sur la même image, qui est maintenant la première du plan.
            self.goto_global(self.doc.panel_start(pi));
        }
    }

    /// « Raccourcir la fin jusqu'à la tête de lecture » : le plan s'arrête au trait de la tête de
    /// lecture (l'image sous la tête sort aussi).
    pub(crate) fn trim_tail_to_playhead(&mut self) {
        let (pi, g) = self.trim_target();
        let start = self.doc.panel_start(pi);
        let len = self.doc.panels[pi].frame_count();
        if g <= start || g >= start + len {
            self.status = "Place la tête de lecture dans ce plan, après sa première image.".into();
            return;
        }
        let cut = (start + len - g) as isize;
        if let Some(done) = self.rogner_plan(pi, Rogner::Tail(-cut), noms::FIN_COURTE) {
            let n = self.doc.panels[pi].frame_count();
            self.status = format!("Fin raccourcie de {} i jusqu'à la tête de lecture : durée {} ({n} i). Les images restent en réserve.", -done, duration_label(n, self.doc.fps));
        }
    }

    /// « Rétablir le contenu rogné » : le plan reprend tout ce qu'il avait caché.
    pub(crate) fn restore_trimmed(&mut self) {
        let (pi, _) = self.trim_target();
        let (h, t) = self.doc.hidden(pi);
        if h + t == 0 {
            self.status = "Ce plan n'a rien de rogné.".into();
            return;
        }
        if self.rogner_plan(pi, Rogner::RestoreAll, noms::RETABLIR).is_some() {
            self.status = format!("{h} image{} du début et {t} de la fin reviennent dans le plan.", s(h));
        }
    }

    /// « Oublier le contenu rogné » : la réserve du plan est vidée (plus rien à retrouver).
    pub(crate) fn forget_trimmed(&mut self) {
        let (pi, _) = self.trim_target();
        let (h, t) = self.doc.hidden(pi);
        if h + t == 0 {
            self.status = "Ce plan n'a rien de rogné.".into();
            return;
        }
        if self.rogner_plan(pi, Rogner::Forget, noms::OUBLIER).is_some() {
            self.status = format!("Contenu rogné oublié ({} image{}) : le plan est tel qu'on le voit.", h + t, s(h + t));
        }
    }

    /// Les commandes de rognage d'un menu de plan (montage, menu Storyboard, règle de la
    /// timeline) : chacune est une action (un clic droit dessus lui donne un raccourci).
    pub(crate) fn trim_menu_items(&mut self, ui: &mut egui::Ui, pi: usize) {
        self.action_item(ui, "Durée…", "montage.duree");
        self.action_item(ui, "Raccourcir le début jusqu'à la tête de lecture", "montage.debut_a_tete");
        self.action_item(ui, "Raccourcir la fin jusqu'à la tête de lecture", "montage.fin_a_tete");
        let (h, t) = self.doc.hidden(pi);
        if h + t > 0 {
            self.action_item(ui, &format!("Rétablir le contenu rogné ({h} i au début, {t} i à la fin)"), "montage.rognage_retablir");
            self.action_item(ui, "Oublier le contenu rogné", "montage.rognage_oublier");
        }
    }

    /// « Durée… » : la fenêtre où l'on tape la durée exacte du plan.
    pub(crate) fn open_duration_dialog(&mut self) {
        let (pi, _) = self.trim_target();
        if self.storyboard_refuses() {
            return;
        }
        let n = self.doc.panels[pi].frame_count();
        self.duration_dialog = Some(DurationDialog { panel: self.doc.panels[pi].id, text: n.to_string(), focus: true });
    }

    /// La fenêtre « Durée… » : un champ (images, secondes:images, +n, -n), Entrée pour valider.
    pub(crate) fn duration_dialog_ui(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.duration_dialog.take() else { return };
        let Some(pi) = self.doc.panels.iter().position(|p| p.id == dlg.panel) else { return };
        let fps = self.doc.fps;
        let current = self.doc.panels[pi].frame_count();
        let parsed = parse_duration(&dlg.text, fps, current);
        let (mut apply, mut close) = (false, false);
        egui::Window::new("Durée du plan")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(format!("{} dure {} ({current} i, {fps} i/s).", crate::storyboard::panel_title(&self.doc, pi), duration_label(current, fps)));
                let resp = ui.add(egui::TextEdit::singleline(&mut dlg.text).desired_width(160.0).hint_text("36, 1:12, +12, -4"));
                if dlg.focus {
                    resp.request_focus();
                    dlg.focus = false;
                }
                ui.label(egui::RichText::new("Des images (36), des secondes et des images (1:12 ou 1s12), ou un écart (+12, -4).").weak().small());
                match parsed {
                    Some(n) => {
                        ui.label(format!("Nouvelle durée : {} ({n} i). La fin est rognée ou prolongée ; ce qui est rogné reste en réserve.", duration_label(n, fps)));
                    }
                    None => {
                        ui.colored_label(ui.visuals().warn_fg_color, "Durée à retaper (au moins une image).");
                    }
                }
                ui.horizontal(|ui| {
                    if ui.add_enabled(parsed.is_some(), egui::Button::new("Valider")).clicked() {
                        apply = true;
                    }
                    if ui.button("Annuler").clicked() {
                        close = true;
                    }
                });
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && parsed.is_some() {
                    apply = true;
                }
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        if apply {
            if let Some(n) = parsed {
                if let Some(done) = self.rogner_plan(pi, Rogner::Duration(n), noms::DUREE) {
                    let now = self.doc.panels[pi].frame_count();
                    self.status = if done == 0 { "La durée était déjà celle-là.".into() } else { format!("Durée du plan : {} ({now} i).", duration_label(now, fps)) };
                }
            }
            return;
        }
        if !close {
            self.duration_dialog = Some(dlg);
        }
    }
}
