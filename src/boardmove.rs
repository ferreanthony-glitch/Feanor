//! Déplacer un plan dans la vue Vignettes (Anthony, 2026-10-21) : Maj + appui sur le **cadre** d'une vignette
//! (son filet, à quelques points près), puis glisser.
//!
//! Règles :
//! - tirer **n'importe quelle** vignette d'un plan déplace **tout le plan** d'un bloc (ses vignettes, ses marques
//!   d'image, ses dessins, ses clés de caméra, son décor, ses notes, ses versions, ses commentaires…) : c'est
//!   l'opération du montage, `Document::move_plans`, reprise telle quelle (`App::move_plans_before`) ;
//! - le repère de pose n'est **jamais qu'entre deux plans**, jamais à l'intérieur d'un plan (on ne coupe pas un
//!   plan par mégarde). Pour sortir une vignette d'un plan, on coupe d'abord sa barre, puis on la déplace ;
//! - retour : le curseur devient une main sur le cadre (Maj tenue), le plan suit le pointeur en fantôme (ses
//!   vignettes gardent leurs places les unes par rapport aux autres), un trait d'accent dit où il se posera ;
//!   Échap abandonne ; une seule étape d'annulation, « Déplacer le plan » ;
//! - d'une séquence à l'autre, comme au montage : le plan rejoint la séquence du plan devant lequel on le pose
//!   (la barre d'état le dit). Un storyboard verrouillé ou un plan tenu par un autre refuse, avec un message ;
//! - Maj + appui **dedans** garde son sens (choisir des plans) ; un simple clic sur le cadre ne fait rien.

use std::time::Instant;

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect, Stroke};

use crate::app::App;
use crate::board::{self, BoardLayout};
use crate::boardui::{Press, PlanMove, Xf, MOVE_THRESHOLD};

impl App {
    /// Maj + appui sur le cadre de la vignette `k` : prend son plan (ou tous les plans choisis, si c'est l'un d'eux).
    pub(crate) fn start_plan_move(&mut self, layout: &BoardLayout, k: usize, pos: Pos2, copie: bool) {
        if self.storyboard_refuses() {
            self.vignettes.press = Press::Dead;
            return;
        }
        let plan = layout.cards[k].plan;
        let chosen = self.board_sel.len() > 1 && self.board_sel.contains(&plan);
        let ids: Vec<u64> = if chosen { self.doc.panels.iter().map(|p| p.id).filter(|id| self.board_sel.contains(id)).collect() } else { vec![plan] };
        self.vignettes.moving = Some(PlanMove { ids, grab: pos, now: pos, active: false, cancelled: false, target: None, copie });
        self.vignettes.press = Press::Move;
    }

    /// À chaque image : le pointeur suit, le geste devient un déplacement au-delà du seuil, Échap l'abandonne, et le
    /// repère de pose est l'endroit entre deux plans le plus proche du pointeur.
    pub(crate) fn update_plan_move(&mut self, layout: &BoardLayout, xf: Xf, pointer: Option<Pos2>, escape: bool) {
        let Some(mv) = self.vignettes.moving.as_mut() else { return };
        let mut abandoned = false;
        if escape && !mv.cancelled {
            (mv.cancelled, mv.active, mv.target) = (true, false, None);
            abandoned = true;
        }
        if let Some(p) = pointer {
            mv.now = p;
            if !mv.cancelled && !mv.active && mv.grab.distance(p) > MOVE_THRESHOLD {
                mv.active = true;
            }
        }
        if mv.active {
            let order: Vec<u64> = self.doc.panels.iter().map(|p| p.id).collect();
            mv.target = if mv.copie {
                board::nearest_slot_copie(&board::drop_slots(layout), xf.to_board(mv.now))
            } else {
                board::nearest_slot(&board::drop_slots(layout), &order, &mv.ids, xf.to_board(mv.now))
            };
        }
        if abandoned {
            self.status = "Déplacement du plan abandonné.".into();
        }
    }

    /// Au lâcher : le plan se pose au repère (une étape d'annulation), sauf si le geste a été abandonné ou n'a pas bougé.
    pub(crate) fn finish_plan_move(&mut self, any_down: bool) {
        if any_down {
            return;
        }
        let Some(mv) = self.vignettes.moving.take() else { return };
        if mv.cancelled || !mv.active {
            return;
        }
        match mv.target {
            Some(slot) if mv.copie => {
                self.board_dupliquer_plans(&mv.ids, slot.before);
            }
            Some(slot) => {
                self.board_move_plans(&mv.ids, slot.before);
            }
            None => self.status = "Le plan reste à sa place.".into(),
        }
    }

    /// Déplace les plans `ids` devant le plan `before` (aucun : à la fin) : l'opération du montage, en une étape
    /// d'annulation nommée. Vrai si l'ordre a changé.
    pub(crate) fn board_move_plans(&mut self, ids: &[u64], before: Option<u64>) -> bool {
        if self.storyboard_refuses() || !self.partage_peut_toucher(ids) {
            return false;
        }
        let Some(first) = self.doc.panels.iter().position(|p| ids.contains(&p.id)) else { return false };
        let sequence = self.doc.sequence_of(first);
        let name = if ids.len() == 1 { "Déplacer le plan" } else { "Déplacer les plans" };
        if !self.avec_etape(name, |s| s.move_plans_before(ids, before)) {
            self.status = "Le plan reste à sa place.".into();
            return false;
        }
        let Some(i) = self.doc.panels.iter().position(|p| p.id == ids[0]) else { return true };
        self.board_sel = ids.to_vec();
        self.select_board(i, 0);
        let title = crate::storyboard::panel_title(&self.doc, i);
        self.status = if ids.len() > 1 {
            format!("{} plans déplacés.", ids.len())
        } else if self.doc.sequence_of(i) != sequence {
            format!("{title} déplacé dans la séquence {}.", self.doc.sequence_label(self.doc.sequence_of(i)))
        } else {
            format!("{title} déplacé.")
        };
        true
    }

    /// Maj + Alt, au lâcher : les plans `ids` sont dupliqués (la copie d'un plan est celle de « Dupliquer le plan » : `duplicate_scene_after`,
    /// ses dessins, ses clés de caméra, ses notes) et les copies se posent devant le plan `before` (aucun : à la fin), dans l'ordre de
    /// lecture. Une seule étape d'annulation, nommée. Vrai si fait.
    pub(crate) fn board_dupliquer_plans(&mut self, ids: &[u64], before: Option<u64>) -> bool {
        if self.storyboard_refuses() || !self.partage_peut_toucher(ids) {
            return false;
        }
        let nom = if ids.len() == 1 { "Dupliquer le plan" } else { "Dupliquer les plans" };
        let mut copies: Vec<u64> = Vec::new();
        let ok = self.avec_etape(nom, |s| {
            s.structure_op_dessins(|d, _| {
                for id in ids {
                    let i = d.panels.iter().position(|p| p.id == *id)?;
                    let n = {
                        let (a, b) = d.scene_run(i);
                        b - a
                    };
                    let first = d.duplicate_scene_after(i);
                    copies.extend(d.panels[first..first + n].iter().map(|p| p.id));
                }
                // (L'ordre ne change pas quand la copie tombe où elle est déjà : rien à déplacer, la copie est bien posée.)
                d.move_plans(&copies, before);
                d.panels.iter().position(|p| p.id == copies[0])
            })
        });
        if !ok {
            return false;
        }
        let Some(i) = self.doc.panels.iter().position(|p| p.id == copies[0]) else { return true };
        self.board_sel = copies.clone();
        self.select_board(i, 0);
        self.status = if ids.len() > 1 { format!("{} plans dupliqués.", ids.len()) } else { format!("{} dupliqué.", crate::storyboard::panel_title(&self.doc, i)) };
        true
    }

    /// Le plan qu'on déplace : ses vignettes d'origine s'effacent un peu, un fantôme de ses vignettes suit le pointeur
    /// (leurs places relatives gardées), et un trait d'accent marque l'endroit de la pose, entre deux plans.
    pub(crate) fn paint_plan_move(&mut self, ctx: &egui::Context, painter: &egui::Painter, view: Rect, xf: Xf, layout: &BoardLayout) {
        let Some(mv) = self.vignettes.moving.as_ref().filter(|m| m.active && !m.cancelled) else { return };
        let accent = crate::theme::accent();
        let clip = painter.with_clip_rect(view);
        let moved: Vec<_> = layout.cards.iter().filter(|c| mv.ids.contains(&c.plan)).copied().collect();
        // Les vignettes d'origine, effacées.
        // (Une copie : l'original reste où il est, net.)
        for c in moved.iter().filter(|_| !mv.copie) {
            clip.rect_filled(xf.rect(c.thumb), 0.0, Color32::from_white_alpha(165));
        }
        // Le repère de pose.
        if let Some(s) = mv.target {
            // (Un trait vertical entre deux cases d'une ligne ; horizontal entre deux cases d'une colonne, à la japonaise.)
            let (a, b) = (xf.to_screen(s.a), xf.to_screen(s.b));
            let dir = (b - a).normalized();
            let dir = if dir.x.is_finite() && dir.y.is_finite() { dir } else { vec2(0.0, 1.0) };
            let (a, b) = (a - dir * 5.0, b + dir * 5.0);
            clip.line_segment([a, b], Stroke::new(3.5_f32, accent));
            clip.circle_filled(a, 4.0, accent);
            clip.circle_filled(b, 4.0, accent);
        }
        // Le fantôme.
        let shift = mv.now - mv.grab;
        let deadline = Instant::now();
        for c in &moved {
            let r = xf.rect(c.thumb).translate(shift);
            clip.rect_filled(r.translate(vec2(2.0, 3.0)), 0.0, Color32::from_black_alpha(40));
            clip.rect_filled(r, 0.0, Color32::from_white_alpha(215));
            let width = crate::thumbs::width_for(r.width() * ctx.pixels_per_point());
            if let Some(tex) = self.thumbs.texture_at(ctx, &self.doc, c.panel, c.frame, c.id, width, deadline) {
                clip.image(tex, r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::from_white_alpha(185));
            }
            clip.rect_stroke(r, 0.0, Stroke::new(2.0_f32, accent), egui::StrokeKind::Outside);
        }
    }
}
