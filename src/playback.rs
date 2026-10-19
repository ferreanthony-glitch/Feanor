//! Lecture fluide (3a) : les dessins des images qui viennent sont envoyés d'avance à la carte
//! graphique, par petits morceaux à chaque image d'écran, pour qu'un changement de panneau ou de
//! dessin ne fige pas la lecture ; et la cadence réellement affichée est mesurée.

use crate::app::App;
use crate::document::{CelId, Document};
use crate::render::Plan;

/// Où en est la lecture, pour savoir quelles images viennent ensuite.
#[derive(Clone, Copy)]
pub struct Position {
    pub panel: usize,
    pub frame: usize,
    pub playing: bool,
    /// Lecture de tout le montage (sinon : du panneau seul).
    pub montage: bool,
    pub looping: bool,
    /// Lecture du plan seul : images [début, fin) lues (points d'entrée et de sortie, 13d).
    pub range: (usize, usize),
}

/// Images (panneau, image) qui viennent : en lecture, la seconde qui suit (en bouclant si la
/// lecture boucle) ; à l'arrêt, les images voisines et les panneaux d'à côté (pour avancer,
/// reculer ou défiler sans attente).
pub fn upcoming_frames(doc: &Document, at: Position) -> Vec<(usize, usize)> {
    let mut frames = Vec::new();
    let count = doc.panels[at.panel].frame_count();
    if at.playing {
        let n = doc.fps.max(1) as usize;
        let (start, (first, total)) = if at.montage {
            (doc.panel_start(at.panel) + at.frame, (0, doc.total_frames()))
        } else {
            (at.frame, (at.range.0.min(count.saturating_sub(1)), at.range.1.min(count).max(1)))
        };
        for k in 1..=n {
            let mut h = start + k;
            if h >= total {
                if !at.looping || total <= first {
                    break;
                }
                h = first + (h - total) % (total - first);
            }
            frames.push(if at.montage { doc.locate(h) } else { (at.panel, h) });
        }
    } else {
        for f in [at.frame + 1, at.frame.wrapping_sub(1)] {
            if f < count {
                frames.push((at.panel, f));
            }
        }
        if at.panel + 1 < doc.panels.len() {
            frames.push((at.panel + 1, 0));
        }
        if let Some(p) = at.panel.checked_sub(1) {
            frames.push((p, doc.panels[p].frame_count() - 1));
            frames.push((p, 0));
        }
    }
    frames
}

/// Dessins visibles des images à venir, dans l'ordre où ils serviront, sans ceux déjà affichés,
/// au plus `cap`.
#[cfg(test)]
pub fn upcoming_cels(doc: &Document, at: Position, shown: &Plan, cap: usize) -> Vec<CelId> {
    upcoming_cels_with(doc, at, shown, cap, false)
}

/// Comme `upcoming_cels`. `gpu_fx` : les effets de calque passent par la carte graphique (`gpufx.rs`) ;
/// un calque dont toute la pile y passe envoie son dessin d'origine, pas un dessin dérivé.
pub fn upcoming_cels_with(doc: &Document, at: Position, shown: &Plan, cap: usize, gpu_fx: bool) -> Vec<CelId> {
    let mut out = Vec::new();
    if cap == 0 {
        return out;
    }
    for (p, f) in upcoming_frames(doc, at) {
        let layers = &doc.panels[p].layers;
        for layer in (0..layers.len()).filter(|&li| crate::folders::effect(layers, li).is_some()).map(|li| &layers[li]) {
            // Un calque d'effet n'a pas de dessin à envoyer (ses images sont des expositions vides).
            if layer.effet.is_some() {
                continue;
            }
            let Some(id) = layer.shown_at(f) else { continue };
            // Un calque à effets montre son dessin dérivé (`upcoming_derived`), pas le dessin même.
            let steps = crate::fx::layer_steps(layer, f, None);
            if !steps.is_empty() && !(gpu_fx && layer.ctg.is_none() && layer.video.is_none() && crate::gpufx::steps_supported(&steps)) {
                continue;
            }
            if out.contains(&id) || shown.items.iter().any(|i| i.cel == id) {
                continue;
            }
            out.push(id);
            if out.len() >= cap {
                return out;
            }
        }
    }
    out
}

/// Les effets des images qui viennent (13i) : leurs dessins dérivés sont demandés au fil de fond,
/// la plus proche d'abord (`fx::prefetch`), et ceux qui sont déjà calculés sont rendus pour être
/// envoyés d'avance à la carte graphique, au plus `cap`. `level` : aperçu des effets (`fx::draft_level`).
#[cfg(test)]
pub fn upcoming_derived(doc: &Document, at: Position, level: u8, cap: usize) -> Vec<crate::gpu::Ahead> {
    upcoming_derived_with(doc, at, level, cap, false)
}

/// Comme `upcoming_derived`. `gpu_fx` : les effets de calque passent par la carte graphique
/// (`gpufx.rs`) : les piles qui y passent entièrement ne sont pas demandées au fil de fond (la carte
/// les dessine à l'image), pas plus que les calques d'ajustement dont tous les repères y passent.
pub fn upcoming_derived_with(doc: &Document, at: Position, level: u8, cap: usize, gpu_fx: bool) -> Vec<crate::gpu::Ahead> {
    let mut out: Vec<crate::gpu::Ahead> = Vec::new();
    for (k, (p, f)) in upcoming_frames(doc, at).into_iter().enumerate() {
        // (Avec le papier de l'écran : le dessin d'un calque d'effet au plus bas le compte.)
        let plan = crate::render::panel_plan_at(doc, p, f, doc.render_paper());
        let on_gpu = |item: &crate::render::PlanItem| gpu_fx && crate::gpu::gpu_fx_item(doc, item);
        let adjust_on_gpu = gpu_fx && crate::gpufx::adjustments_supported(&plan);
        for item in plan.items.iter().filter(|i| !on_gpu(i)) {
            crate::fx::prefetch(doc, std::slice::from_ref(item), k as u32 + 1, level);
        }
        // Les calques d'effets de cette image : leurs dessins sont demandés au fil de fond, et ceux qui
        // sont prêts partent d'avance à la carte graphique (`calqueffet.rs`).
        if !adjust_on_gpu {
            for (id, cel) in crate::calqueffet::ahead(doc, &plan, k as u32 + 1, level) {
                if out.len() < cap && !out.iter().any(|a| a.id == id) {
                    out.push(crate::gpu::Ahead { id, cel: Some(cel) });
                }
            }
        }
        for item in plan.items.iter().filter(|i| !on_gpu(i)) {
            if out.len() >= cap {
                break;
            }
            if let Some((id, cel)) = crate::fx::ready(doc, item, level) {
                if !out.iter().any(|a| a.id == id) {
                    out.push(crate::gpu::Ahead { id, cel: Some(cel) });
                }
            }
        }
    }
    out
}

impl App {
    pub(crate) fn play_position(&self) -> Position {
        Position {
            panel: self.cur_panel,
            frame: self.cur_frame,
            playing: self.playing,
            montage: self.play_montage,
            looping: self.looping,
            range: self.plan_play_range(),
        }
    }
}

/// Cadence réellement affichée pendant la lecture, mesurée sur chaque seconde.
#[derive(Default)]
pub struct PlayMeter {
    since: Option<f64>,
    last: Option<usize>,
    shown: u32,
    skipped: u32,
    /// Dernière mesure : images affichées par seconde, images sautées pendant cette seconde.
    pub rate: Option<(f32, u32)>,
}

impl PlayMeter {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// À chaque image d'écran pendant la lecture : `g` = image du montage affichée, `now` en
    /// secondes.
    pub fn tick(&mut self, now: f64, g: usize) {
        let since = *self.since.get_or_insert(now);
        if self.last != Some(g) {
            if let Some(l) = self.last {
                self.shown += 1;
                if g > l {
                    self.skipped += (g - l - 1) as u32;
                }
            }
            self.last = Some(g);
        }
        let elapsed = now - since;
        if elapsed >= 1.0 {
            self.rate = Some((self.shown as f32 / elapsed as f32, self.skipped));
            self.since = Some(now);
            self.shown = 0;
            self.skipped = 0;
        }
    }
}
