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
}

/// Images (panneau, image) qui viennent : en lecture, la seconde qui suit (en bouclant si la
/// lecture boucle) ; à l'arrêt, les images voisines et les panneaux d'à côté (pour avancer,
/// reculer ou défiler sans attente).
pub fn upcoming_frames(doc: &Document, at: Position) -> Vec<(usize, usize)> {
    let mut frames = Vec::new();
    let count = doc.panels[at.panel].frame_count();
    if at.playing {
        let n = doc.fps.max(1) as usize;
        let (start, total) = if at.montage {
            (doc.panel_start(at.panel) + at.frame, doc.total_frames())
        } else {
            (at.frame, count)
        };
        for k in 1..=n {
            let mut h = start + k;
            if h >= total {
                if !at.looping {
                    break;
                }
                h %= total;
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
pub fn upcoming_cels(doc: &Document, at: Position, shown: &Plan, cap: usize) -> Vec<CelId> {
    let mut out = Vec::new();
    if cap == 0 {
        return out;
    }
    for (p, f) in upcoming_frames(doc, at) {
        for layer in doc.panels[p].layers.iter().filter(|l| l.visible) {
            let Some(id) = layer.cel_at(f) else { continue };
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

impl App {
    pub(crate) fn play_position(&self) -> Position {
        Position {
            panel: self.cur_panel,
            frame: self.cur_frame,
            playing: self.playing,
            montage: self.play_montage,
            looping: self.looping,
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
