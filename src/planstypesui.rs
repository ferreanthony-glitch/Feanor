//! Les plans types dans l'interface (partie 176, `planstypes.rs` pour ce qu'ils posent) : le clic sur une vignette du panneau
//! Préréglages, le dépôt d'une vignette glissée sur le montage.

use eframe::egui::{self, pos2, Rect, Stroke, StrokeKind};

use crate::app::App;
use crate::planstypes::{PlanType, PlanTypeDrag};
use crate::theme;

impl App {
    /// Pose le plan type `t` : un plan neuf sur la coupe `cible` (0 à `panels.len()`), ou, pour un type qui se pose sur un plan
    /// (lieu et date, générique de début), des calques sur le plan de rang `cible`. Une étape d'annulation ; le plan posé devient
    /// le plan courant, son calque principal le calque courant.
    pub(crate) fn poser_plan_type(&mut self, t: PlanType, cible: usize) {
        if self.doc.panels.is_empty() {
            return;
        }
        let avant = self.history.context.replace(format!("Plan type : {}", t.label().to_lowercase()));
        let pose = if t.sur_un_plan() {
            let i = cible.min(self.doc.panels.len() - 1);
            self.structure_op_porte(false, |d, _| d.poser_sur_le_plan(t, i).map(|_| i))
        } else {
            self.structure_op(|d, _| d.poser_plan_type(t, cible))
        };
        self.history.context = avant;
        if !pose {
            return;
        }
        self.cur_layer = self.layers().len().saturating_sub(1);
        self.layer_sel.clear();
        self.cur_frame = 0;
        self.status = match t {
            PlanType::LieuEtDate => "Lieu et date : son texte se change dans les réglages du calque « Lieu et date » ; il s'écrit lettre à lettre.".into(),
            PlanType::GeneriqueDebut => "Générique de début : trois noms l'un après l'autre ; l'outil Texte les retouche (un dessin par nom).".into(),
            PlanType::Logo => "Logo studio : choisis ton image, elle se pose sur le calque Logo, qui apparaît en douceur.".into(),
            _ => format!("{} : un plan neuf, tout prêt ; Ctrl+Z le retire.", t.label()),
        };
        // Le logo : le calque Logo est le calque courant, on choisit l'image tout de suite (pas pendant les essais).
        if t == PlanType::Logo && !cfg!(test) {
            if let Some(i) = self.layers().iter().position(|l| l.name == "Logo") {
                self.cur_layer = i;
                self.import_image();
            }
        }
    }

    /// Clic sur une vignette de plan type : un plan neuf juste après le plan courant ; un type qui se pose sur un plan, sur le
    /// plan courant.
    pub(crate) fn poser_plan_type_a_la_tete(&mut self, t: PlanType) {
        let cible = if t.sur_un_plan() { self.cur_panel } else { self.cur_panel + 1 };
        self.poser_plan_type(t, cible);
    }

    /// Un plan type glissé depuis le panneau Préréglages, au-dessus du montage : la coupe la plus proche du pointeur s'ouvre (un
    /// trait et un rond « + »), ou, pour un type qui se pose sur un plan, le plan sous le pointeur s'éclaire ; au lâcher, il s'y
    /// pose.
    pub(crate) fn plan_type_depot(&mut self, ui: &egui::Ui, zone: Rect, blocks: &[Rect], top: f32, bottom: f32) {
        let ctx = ui.ctx().clone();
        let Some(drag) = egui::DragAndDrop::payload::<PlanTypeDrag>(&ctx) else { return };
        let Some(p) = ctx.pointer_latest_pos() else { return };
        if !zone.contains(p) || blocks.is_empty() {
            return;
        }
        let painter = ui.painter_at(zone);
        let cible = if drag.plan.sur_un_plan() {
            let i = blocks.iter().position(|b| p.x >= b.left() - 1.0 && p.x <= b.right() + 1.0).unwrap_or_else(|| if p.x < blocks[0].left() { 0 } else { blocks.len() - 1 });
            painter.rect_stroke(blocks[i].expand(2.0), 4.0, Stroke::new(3.0_f32, theme::contour()), StrokeKind::Outside);
            i
        } else {
            let Some((cut, x)) = crate::transitionsui::coupe_la_plus_proche(blocks, p.x) else { return };
            painter.line_segment([pos2(x, top - 4.0), pos2(x, bottom + 4.0)], Stroke::new(3.0_f32, theme::contour()));
            let c = pos2(x, top - 6.0);
            painter.circle_filled(c, 7.0, theme::contour());
            let s = Stroke::new(2.0_f32, egui::Color32::WHITE);
            painter.line_segment([c - egui::vec2(3.5, 0.0), c + egui::vec2(3.5, 0.0)], s);
            painter.line_segment([c - egui::vec2(0.0, 3.5), c + egui::vec2(0.0, 3.5)], s);
            cut
        };
        if ctx.input(|i| i.pointer.any_released()) {
            if let Some(d) = egui::DragAndDrop::take_payload::<PlanTypeDrag>(&ctx) {
                self.poser_plan_type(d.plan, cible);
            }
        }
    }
}
