//! La durée des plans (Anthony, 2026-10-28) : celle d'un plan neuf, et « Changer la durée des plans choisis… ».
//!
//! - **Durée d'un nouveau plan** (Préférences > Montage) : en secondes dans les réglages (2 s d'origine, ce que SHOT donnait déjà), montrée
//!   aussi en images à la cadence du projet ouvert. Tout plan créé ensuite (nouveau plan, plan avant, nouvelle séquence, case dessinée dans
//!   une case vide de la planche) la reçoit : `Document::make_panel` la lit dans `Document::duree_plan_neuf`, que `App` pose avant ses
//!   commandes. Les plans qui existent gardent leur durée ; ce qu'un import donne à ses plans ne change pas.
//! - **Changer la durée des plans choisis…** (clic droit sur un plan du montage ou une vignette, menu « Plans », action `montage.duree_plans`
//!   sans touche) : une petite fenêtre, un champ qui part de la durée d'un nouveau plan, et tous les plans choisis prennent cette durée d'un
//!   coup. C'est « Durée… » (`rognageui.rs`) pour plusieurs plans : la même règle (`Document::set_duration`), donc la fin de chaque plan est
//!   rognée (ce qui sort reste en réserve, `rognage.rs`) ou prolongée (le dernier dessin est tenu, la réserve revient d'abord). Le champ lit
//!   les mêmes écritures (36, 1:12, 1s12) et un écart (+12, -4) s'ajoute à la durée de chaque plan. Le son ne bouge pas : s'il suit, la
//!   barre d'état prévient que les plans suivants se décalent par rapport à lui (`handles::sound_shift_note`). Une seule étape d'annulation ;
//!   refusé sous le verrou du storyboard.

use eframe::egui::{self, vec2, Align2};

use crate::app::App;
use crate::rognage::{parse_duration, Rogner, MAX_PLAN};
use crate::storyboard::duration_label;

/// Ce que SHOT donnait à un plan neuf avant le réglage : deux secondes.
pub(crate) const SECONDES_D_ORIGINE: f32 = 2.0;
/// Les bornes du réglage, en secondes.
pub(crate) const SECONDES_MIN: f32 = 0.05;
pub(crate) const SECONDES_MAX: f32 = 3600.0;

pub(crate) fn secondes_d_origine() -> f32 {
    SECONDES_D_ORIGINE
}

pub(crate) fn est_d_origine(s: &f32) -> bool {
    (*s - SECONDES_D_ORIGINE).abs() < 1e-4
}

/// Un réglage lu d'un fichier, rendu sûr : un nombre, dans les bornes (sinon la durée d'origine).
pub(crate) fn secondes_valides(s: f32) -> f32 {
    if s.is_finite() {
        s.clamp(SECONDES_MIN, SECONDES_MAX)
    } else {
        SECONDES_D_ORIGINE
    }
}

/// Le nombre d'images de `secs` secondes à `fps` images par seconde : au moins une, au plus la durée maximale d'un plan.
pub(crate) fn images(secs: f32, fps: u32) -> usize {
    let n = (secs * fps.max(1) as f32).round();
    if n.is_finite() {
        (n as usize).clamp(1, MAX_PLAN)
    } else {
        1
    }
}

/// La durée commune des plans `ids` (en images), rien s'ils n'ont pas tous la même.
pub(crate) fn duree_commune(doc: &crate::document::Document, ids: &[u64]) -> Option<usize> {
    let mut d = ids.iter().filter_map(|id| doc.panels.iter().find(|p| p.id == *id)).map(|p| p.frame_count());
    let first = d.next()?;
    d.all(|n| n == first).then_some(first)
}

/// La fenêtre « Changer la durée des plans choisis… » : les plans visés (leurs numéros), le texte du champ, le focus à donner au champ.
pub(crate) struct DureePlans {
    pub ids: Vec<u64>,
    pub text: String,
    pub focus: bool,
}

impl App {
    /// « Changer la durée des plans choisis… » : ouvre la fenêtre, le champ prérempli de la durée d'un nouveau plan.
    pub(crate) fn open_plans_duration_dialog(&mut self) {
        if self.storyboard_refuses() {
            return;
        }
        let ids = self.board_selection();
        // Le champ montre la durée commune des plans choisis ; s'ils diffèrent, il est vide (« durées différentes », partie 138).
        let text = duree_commune(&self.doc, &ids).map(|n| n.to_string()).unwrap_or_default();
        self.duree_plans = Some(DureePlans { ids, text, focus: true });
    }

    /// Les durées que `text` donne aux plans `ids` : (indice du plan, durée actuelle, durée voulue). Rien si le texte n'est pas une durée.
    pub(crate) fn durees_voulues(&self, ids: &[u64], text: &str) -> Option<Vec<(usize, usize, usize)>> {
        let fps = self.doc.fps;
        let mut out = Vec::new();
        for id in ids {
            let Some(pi) = self.doc.panels.iter().position(|p| p.id == *id) else { continue };
            let now = self.doc.panels[pi].frame_count();
            out.push((pi, now, parse_duration(text, fps, now)?));
        }
        (!out.is_empty()).then_some(out)
    }

    /// La phrase qui prévient que le son ne suit pas : les plans d'après les premiers changements se décalent, le son reste.
    pub(crate) fn plans_sound_note(&self, voulues: &[(usize, usize, usize)]) -> Option<String> {
        let &(pi, old, _) = voulues.iter().find(|(_, o, n)| o != n)?;
        let total: isize = voulues.iter().map(|&(_, o, n)| n as isize - o as isize).sum();
        let new = (old as isize + total).max(0) as usize;
        crate::handles::sound_shift_note(&self.doc, pi, old, new)
    }

    /// Donne aux plans `ids` la durée que dit `text` (voir le haut de ce fichier), en une étape d'annulation. Renvoie combien de plans ont
    /// changé de durée ; rien si le texte n'est pas une durée ou si le storyboard est verrouillé.
    pub(crate) fn change_plans_duration(&mut self, ids: &[u64], text: &str) -> Option<usize> {
        if self.storyboard_refuses() {
            return None;
        }
        // Film partagé : un plan tenu par quelqu'un d'autre est laissé de côté, les autres changent (partie 138).
        let mut libres = Vec::new();
        let mut tenus = 0usize;
        for &id in ids {
            if self.partage_peut_toucher(&[id]) {
                libres.push(id);
            } else {
                tenus += 1;
            }
        }
        if libres.is_empty() {
            return None;
        }
        let ids = &libres[..];
        let Some(voulues) = self.durees_voulues(ids, text) else {
            self.status = "Durée à retaper (au moins une image : 36, 1:12, +12, -4).".into();
            return None;
        };
        let note = self.plans_sound_note(&voulues);
        self.end_stroke();
        self.poser_les_selections();
        self.playing = false;
        let snap = self.snapshot();
        let mut changes = 0;
        for &(pi, _, n) in &voulues {
            if self.doc.rogner(pi, Rogner::Duration(n)).ok()? != 0 {
                changes += 1;
            }
        }
        let fps = self.doc.fps;
        if changes == 0 {
            self.status = "Ces plans avaient déjà cette durée.".into();
            return Some(0);
        }
        self.after_rognage();
        self.length_origin = None;
        self.commit_as(snap, "Durée des plans");
        let n = voulues.len();
        let pl = if n > 1 { "s" } else { "" };
        let uniforme = voulues.iter().all(|&(pi, _, _)| self.doc.panels[pi].frame_count() == self.doc.panels[voulues[0].0].frame_count());
        let mut msg = if uniforme {
            let d = self.doc.panels[voulues[0].0].frame_count();
            format!("{n} plan{pl} passé{pl} à {} ({d} i)", crate::clesduree::duree_texte(d, fps))
        } else {
            format!("{n} plan{pl} : durée changée selon leur durée d'avant")
        };
        // Les dernières clés s'étirent en allongeant ; un calque verrouillé reste tel quel (partie 138).
        let laisses: usize = voulues.iter().filter(|&&(pi, old, _)| self.doc.panels[pi].frame_count() > old).map(|&(pi, _, _)| crate::clesduree::verrouilles_courts(&self.doc.panels[pi], self.doc.panels[pi].frame_count())).sum();
        if laisses > 0 {
            msg += &format!(" ; {laisses} calque{} verrouillé{} laissé{} sans étirer {} dernière clé", if laisses > 1 { "s" } else { "" }, if laisses > 1 { "s" } else { "" }, if laisses > 1 { "s" } else { "" }, if laisses > 1 { "leur" } else { "sa" });
        }
        if tenus > 0 {
            msg += &format!(" ; {tenus} plan{} tenu{} par quelqu'un d'autre laissé{} de côté", if tenus > 1 { "s" } else { "" }, if tenus > 1 { "s" } else { "" }, if tenus > 1 { "s" } else { "" });
        }
        self.status = format!("{msg}.{}", note.map(|t| format!(" {t}")).unwrap_or_default());
        Some(changes)
    }

    /// La fenêtre « Changer la durée des plans choisis… » : un champ (images, secondes:images, +n, -n), Entrée pour valider.
    pub(crate) fn plans_duration_dialog_ui(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.duree_plans.take() else { return };
        // (Un plan qui n'existe plus — annulation, film partagé — n'est plus visé.)
        dlg.ids.retain(|id| self.doc.panels.iter().any(|p| p.id == *id));
        if dlg.ids.is_empty() {
            return;
        }
        let fps = self.doc.fps;
        let voulues = self.durees_voulues(&dlg.ids, &dlg.text);
        let (mut apply, mut close) = (false, false);
        egui::Window::new("Durée des plans choisis")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                let total: usize = dlg.ids.iter().filter_map(|id| self.doc.panels.iter().find(|p| p.id == *id)).map(|p| p.frame_count()).sum();
                ui.label(format!("{} plan{} choisi{} : {} en tout ({total} i, {fps} i/s).", dlg.ids.len(), if dlg.ids.len() > 1 { "s" } else { "" }, if dlg.ids.len() > 1 { "s" } else { "" }, duration_label(total, fps)));
                let commune = duree_commune(&self.doc, &dlg.ids);
                let hint = if commune.is_none() { "durées différentes" } else { "36, 1:12, +12, -4" };
                let resp = ui.add(egui::TextEdit::singleline(&mut dlg.text).desired_width(160.0).hint_text(hint));
                if dlg.focus {
                    resp.request_focus();
                    dlg.focus = false;
                }
                ui.label(egui::RichText::new("La durée de chaque plan : des images (36), des secondes et des images (1:12 ou 1s12), ou un écart (+12, -4). Le champ montre la durée commune des plans choisis. En allongeant, la dernière clé de chaque calque s'étire jusqu'à la nouvelle fin.").weak().small());
                match &voulues {
                    Some(v) => {
                        let apres: usize = v.iter().map(|&(_, _, n)| n).sum();
                        ui.label(format!("Nouvelle durée en tout : {} ({apres} i). La fin de chaque plan est rognée ou prolongée ; ce qui est rogné reste en réserve.", duration_label(apres, fps)));
                        if let Some(note) = self.plans_sound_note(v) {
                            ui.colored_label(ui.visuals().warn_fg_color, note);
                        }
                    }
                    None => {
                        ui.colored_label(ui.visuals().warn_fg_color, "Durée à retaper (au moins une image).");
                    }
                }
                ui.horizontal(|ui| {
                    if ui.add_enabled(voulues.is_some(), egui::Button::new("Valider")).clicked() {
                        apply = true;
                    }
                    if ui.button("Annuler").clicked() {
                        close = true;
                    }
                });
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && voulues.is_some() {
                    apply = true;
                }
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        if apply {
            self.change_plans_duration(&dlg.ids, &dlg.text);
            return;
        }
        if !close {
            self.duree_plans = Some(dlg);
        }
    }

    /// Préférences > Montage > « Plans neufs » : la durée d'un nouveau plan, en secondes et en images (à la cadence du projet ouvert).
    pub(crate) fn new_plan_duration_pref_ui(&mut self, ui: &mut egui::Ui) {
        let fps = self.doc.fps.max(1);
        let mut secs = self.settings.new_plan_secs;
        let mut frames = images(secs, fps);
        let avant = secs;
        ui.horizontal_wrapped(|ui| {
            let label = ui.label("Durée d'un nouveau plan");
            crate::bulles::bulle(label, "Durée d'un nouveau plan", "Ce que reçoit chaque plan créé ensuite. Les plans qui existent déjà gardent leur durée.", "");
            let a = ui.add(egui::DragValue::new(&mut secs).speed(0.05).range(SECONDES_MIN..=SECONDES_MAX).max_decimals(2).suffix(" s"));
            ui.label("ou");
            let b = ui.add(egui::DragValue::new(&mut frames).speed(0.5).range(1..=MAX_PLAN).suffix(" i"));
            if b.changed() && !a.changed() {
                secs = frames as f32 / fps as f32;
            }
            ui.label(egui::RichText::new(format!("à {fps} i/s, la cadence du projet ouvert")).small().weak());
            if !est_d_origine(&secs) && ui.small_button("Rétablir 2 s").clicked() {
                secs = SECONDES_D_ORIGINE;
            }
        });
        let secs = secondes_valides(secs);
        if (secs - avant).abs() > 1e-6 {
            self.settings.new_plan_secs = secs;
            let _ = self.settings.save();
        }
    }
}
