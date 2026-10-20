//! Interpolation d'images dans l'interface (13l) : insérer des images intermédiaires entre le
//! dessin courant et le suivant, remplir les images vides entre deux dessins par des
//! intervalles, étirer des images en interpolant, changer la cadence du projet en gardant les
//! durées. Sur les calques choisis, une étape d'annulation chacune. Voir `tween.rs`.

use std::collections::HashMap;

use eframe::egui::{self, vec2, Align2};

use crate::app::App;
use crate::document::{Cel, CelId, LayerKind};
use crate::images::run_at;
use crate::tween::{self, Slot};

/// Ce que règle la fenêtre d'interpolation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TweenKind {
    Insert,
    Stretch,
    Fps,
}

/// La fenêtre d'interpolation ouverte.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TweenDialog {
    kind: TweenKind,
    count: usize,
    length: usize,
    fps: u32,
    interpolate: bool,
}

impl App {
    /// Un nouveau dessin entre les dessins `a` et `b` à `t` : vrai intervalle si les deux sont
    /// vectoriels, sinon fondu.
    fn tween_cel(&mut self, a: CelId, b: CelId, t: f32) -> Option<CelId> {
        let (ca, cb) = (self.doc.cels.get(&a)?, self.doc.cels.get(&b)?);
        let cel = match (&ca.vector, &cb.vector) {
            (Some(va), Some(vb)) => {
                let mut c = Cel::new(self.doc.width, self.doc.height, LayerKind::Vector);
                c.vector = Some(tween::tween_vector(va, vb, t));
                c.rerender();
                c
            }
            _ => Cel::bitmap(tween::blend_raster(&ca.raster, &cb.raster, t)),
        };
        Some(self.doc.add_cel(cel))
    }

    /// Les images `slots` faites dessins (un intervalle par (a, b, t) distinct).
    fn realize(&mut self, slots: &[Slot]) -> Vec<Option<CelId>> {
        let mut made: HashMap<(CelId, CelId, u32), CelId> = HashMap::new();
        slots
            .iter()
            .map(|s| match *s {
                Slot::Keep(c) => c,
                Slot::Tween(a, b, t) => {
                    if let Some(&c) = made.get(&(a, b, t.to_bits())) {
                        return Some(c);
                    }
                    let c = self.tween_cel(a, b, t)?;
                    made.insert((a, b, t.to_bits()), c);
                    Some(c)
                }
            })
            .collect()
    }

    /// « Insérer des images intermédiaires » : `n` intervalles entre le dessin courant et le
    /// suivant, sur les calques choisis, chacun exposé selon « en N ».
    pub(crate) fn insert_intermediates(&mut self, n: usize) {
        if self.content_refuses() || self.refuse_on_folder() {
            return;
        }
        self.end_stroke();
        let f = self.cur_frame;
        let exp = self.default_exposure.max(1);
        let snap = self.snapshot();
        let mut done = 0;
        for li in self.image_layers() {
            let frames = self.layers()[li].frames.clone();
            let Some(a) = frames.get(f).copied().flatten() else { continue };
            let (_, e) = run_at(&frames, f);
            let Some(b) = frames[e.min(frames.len())..].iter().flatten().next().copied() else { continue };
            let mut row = Vec::with_capacity(n * exp);
            for k in 1..=n {
                let c = self.tween_cel(a, b, k as f32 / (n + 1) as f32);
                row.extend(std::iter::repeat_n(c, exp));
            }
            let layer = &mut self.layers_mut()[li];
            crate::images::insert_frames(&mut layer.frames, e, &row);
            layer.shift_frames(e, row.len(), false);
            done += 1;
        }
        if done == 0 {
            self.status = "Il faut un dessin à l'image courante et un dessin plus loin sur le calque.".into();
            return;
        }
        self.renderer.invalidate_all();
        self.commit_structure(snap);
        self.status = format!("{n} image{} intermédiaire{} insérée{}.", s(n), s(n), s(n));
    }

    /// « Remplir les vides par des intervalles » : chaque image vide entre deux dessins (dans
    /// les images choisies, sinon autour de l'image courante) reçoit un intervalle.
    pub(crate) fn fill_gaps(&mut self) {
        if self.content_refuses() || self.refuse_on_folder() {
            return;
        }
        self.end_stroke();
        let chosen = self.frame_sel.filter(|(a, b)| a < b);
        let snap = self.snapshot();
        let mut filled = 0;
        for li in self.image_layers() {
            let frames = self.layers()[li].frames.clone();
            let (lo, hi) = match chosen {
                Some(r) => r,
                None => {
                    // Les images vides autour de l'image courante, ou juste après son dessin.
                    let f = self.cur_frame;
                    let start = if frames.get(f).copied().flatten().is_some() { run_at(&frames, f).1 } else { f };
                    let end = (start..frames.len()).find(|&g| frames[g].is_some()).unwrap_or(start);
                    (start, end)
                }
            };
            let mut new = frames.clone();
            let mut g = lo;
            while g < hi.min(frames.len()) {
                if frames[g].is_some() {
                    g += 1;
                    continue;
                }
                // La suite d'images vides [s0, h) qui contient g, entre le dessin d'avant et celui
                // d'après ; seules ses images dans [lo, hi) sont remplies.
                let mut s0 = g;
                while s0 > 0 && frames[s0 - 1].is_none() {
                    s0 -= 1;
                }
                let h = (g..frames.len()).find(|&k| frames[k].is_some()).unwrap_or(frames.len());
                let before = s0.checked_sub(1).and_then(|k| frames[k]);
                let after = frames.get(h).copied().flatten();
                if let (Some(a), Some(b)) = (before, after) {
                    let span = (h - (s0 - 1)) as f32;
                    // Avec des images choisies : seulement celles-là ; sinon toute la suite.
                    let (from, to) = if chosen.is_some() { (g, h.min(hi)) } else { (s0, h) };
                    for k in from..to {
                        new[k] = self.tween_cel(a, b, (k - (s0 - 1)) as f32 / span);
                        filled += 1;
                    }
                }
                g = h;
            }
            self.layers_mut()[li].frames = new;
        }
        if filled == 0 {
            self.status = "Aucune image vide entre deux dessins ici (choisis des images, ou place-toi dans un vide).".into();
            return;
        }
        self.renderer.invalidate_all();
        self.commit_structure(snap);
        self.status = format!("{filled} intervalle{} dessiné{}.", s(filled), s(filled));
    }

    /// « Étirer en interpolant » : les images choisies (sinon tout le calque) durent `m`
    /// images ; les images en plus sont des intervalles (ou le dessin tenu).
    pub(crate) fn stretch_interpolating(&mut self, m: usize, interpolate: bool) {
        if self.content_refuses() || self.refuse_on_folder() {
            return;
        }
        self.end_stroke();
        let chosen = self.frame_sel.filter(|(a, b)| a < b);
        let snap = self.snapshot();
        let mut done = 0;
        for li in self.image_layers() {
            let frames = self.layers()[li].frames.clone();
            let (a, b) = chosen.unwrap_or((0, frames.len()));
            let b = b.min(frames.len());
            if a >= b || m == 0 {
                continue;
            }
            let slots = tween::resample(&frames[a..b], m, interpolate);
            let row = self.realize(&slots);
            let layer = &mut self.layers_mut()[li];
            layer.frames.splice(a..b, row);
            let n = b - a;
            if m > n {
                layer.shift_frames(b, m - n, false);
            } else if m < n {
                layer.shift_frames(a + m, n - m, true);
            }
            layer.trim();
            done += 1;
        }
        if done == 0 {
            self.status = "Rien à étirer sur les calques choisis.".into();
            return;
        }
        self.frame_sel = chosen.map(|(a, _)| (a, a + m));
        self.renderer.invalidate_all();
        self.commit_structure(snap);
        self.status = format!("Images étirées sur {m} image{}.", s(m));
    }

    /// « Changer la cadence » : tout le projet passe à `fps` images par seconde en gardant les
    /// durées. Images des calques, durées des plans, clés de caméra, marques, repères, notes,
    /// rôles, clés d'effets, débuts des vidéos et extraits de son suivent ; les images ajoutées
    /// sont des intervalles si `interpolate` (sinon les dessins sont tenus).
    pub(crate) fn change_fps(&mut self, fps: u32, interpolate: bool) {
        let from = self.doc.fps.max(1);
        let fps = fps.clamp(1, 120);
        if fps == from {
            self.status = "La cadence est déjà celle-là.".into();
            return;
        }
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        let r = |f: usize| tween::rescale_frame(f, from, fps);
        // Le contenu rogné des plans (`rognage.rs`) est compté dans l'ancienne cadence : oublié.
        let forgotten: usize = self.doc.reserves.values().map(|r| r.head.len + r.tail.len).sum();
        self.doc.reserves.clear();
        for pi in 0..self.doc.panels.len() {
            let layers = self.doc.panels[pi].layers.clone();
            let mut new_layers = Vec::with_capacity(layers.len());
            for mut l in layers {
                if !l.frames.is_empty() {
                    let slots = tween::resample(&l.frames, r(l.frames.len()).max(1), interpolate);
                    l.frames = self.realize(&slots);
                }
                // (Rôles, clés d'effets et de position, début de la vidéo : la règle commune au collage d'un autre projet.)
                crate::adaptation::retemps_reglages_calque(&mut l, &r);
                new_layers.push(l);
            }
            let p = &mut self.doc.panels[pi];
            p.layers = new_layers;
            crate::adaptation::retemps_reglages_plan(p, &r, fps as f32 / from as f32);
        }
        for t in &mut self.doc.audio_tracks {
            for c in &mut t.clips {
                (c.start, c.length, c.offset) = (r(c.start), r(c.length).max(1), r(c.offset));
            }
        }
        self.doc.fps = fps;
        self.cur_frame = r(self.cur_frame).min(self.frame_count().saturating_sub(1));
        self.frame_sel = None;
        self.renderer.invalidate_all();
        self.thumbs.clear();
        self.commit_structure(snap);
        let how = if interpolate { "images ajoutées interpolées" } else { "dessins tenus" };
        self.status = format!("Projet passé de {from} à {fps} images par seconde, durées gardées ({how}).");
        if forgotten > 0 {
            self.status.push_str(&format!(" Le contenu rogné des plans ({forgotten} i) est oublié."));
        }
    }

    /// Ouvre la fenêtre d'interpolation pour `kind`.
    pub(crate) fn open_tween_dialog(&mut self, kind: TweenKind) {
        let (a, b) = self.frame_sel.filter(|(a, b)| a < b).unwrap_or((0, self.layer().frames.len()));
        self.tween_dialog = Some(TweenDialog { kind, count: 1, length: ((b - a) * 2).max(2), fps: self.doc.fps, interpolate: true });
    }

    /// La fenêtre d'interpolation, si elle est ouverte.
    pub(crate) fn tween_window_ui(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.tween_dialog else { return };
        let title = match d.kind {
            TweenKind::Insert => "Insérer des images intermédiaires",
            TweenKind::Stretch => "Étirer en interpolant",
            TweenKind::Fps => "Changer la cadence",
        };
        let (mut keep, mut go) = (true, false);
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .default_pos(ctx.screen_rect().center() - vec2(160.0, 80.0))
            .pivot(Align2::LEFT_TOP)
            .show(ctx, |ui| {
                match d.kind {
                    TweenKind::Insert => {
                        ui.horizontal(|ui| {
                            ui.label("Images intermédiaires");
                            ui.add(egui::DragValue::new(&mut d.count).range(1..=48));
                        });
                        ui.label(egui::RichText::new("Entre le dessin courant et le suivant, sur les calques choisis ; chacune exposée selon « en N ».").weak().small());
                    }
                    TweenKind::Stretch => {
                        ui.horizontal(|ui| {
                            ui.label("Nouvelle durée");
                            ui.add(egui::DragValue::new(&mut d.length).range(1..=100_000).suffix(" images"));
                        });
                        ui.checkbox(&mut d.interpolate, "Interpoler les images ajoutées (sinon : dessins tenus)");
                        ui.label(egui::RichText::new("Les images choisies, sinon tout le calque, sur les calques choisis.").weak().small());
                    }
                    TweenKind::Fps => {
                        ui.horizontal(|ui| {
                            ui.label("Nouvelle cadence");
                            ui.add(egui::DragValue::new(&mut d.fps).range(1..=120).suffix(" i/s"));
                        });
                        ui.checkbox(&mut d.interpolate, "Interpoler les images ajoutées (sinon : dessins tenus)");
                        ui.label(egui::RichText::new("Tout le projet, en gardant les durées (le champ « i/s » de la timeline, lui, ne change que la vitesse).").weak().small());
                    }
                }
                ui.horizontal(|ui| {
                    go = ui.button("Appliquer").clicked();
                    keep = !ui.button("Annuler").clicked();
                });
            });
        self.tween_dialog = (keep && !go).then_some(d);
        if go {
            match d.kind {
                TweenKind::Insert => self.insert_intermediates(d.count),
                TweenKind::Stretch => self.stretch_interpolating(d.length, d.interpolate),
                TweenKind::Fps => self.change_fps(d.fps, d.interpolate),
            }
        }
    }
}

fn s(n: usize) -> &'static str {
    if n > 1 {
        "s"
    } else {
        ""
    }
}
