//! Outils « à ressort » (Anthony, 2026-10-17 : « chaque outil peut être utilisé par pression
//! continue d'une touche, au relâché on revient sur l'outil précédent » ; « ajoute l'outil
//! lasso au raccourci Ctrl »).
//! - La touche d'un outil tenue (au moins `HOLD` secondes, ou l'outil utilisé sur le dessin
//!   pendant qu'on la tient) : au relâché, l'outil d'avant revient. Une pression brève change
//!   d'outil, comme avant (sinon on ne pourrait plus changer d'outil d'une touche).
//! - Ctrl tenu sur le dessin (seul, ou avec Maj : Ctrl+Maj, « tous les calques » et le cumul de la
//!   sélection, `selobjet.rs`) : l'outil Sélection (son lasso), le temps de le tenir. Une
//!   autre touche (Ctrl+Z…) ou la molette pendant qu'on le tient : ce n'était pas pour le lasso.
//!   Une zone bitmap choisie pendant qu'on le tient garde l'outil Sélection au relâché (elle
//!   n'existe qu'avec lui) ; des traits vectoriels choisis restent choisis avec l'outil d'avant.
//! - Le retour à l'outil d'avant attend que le bouton de la souris soit relâché (un trait ou un
//!   lasso en cours n'est pas coupé).

use eframe::egui::{self, Event, Key};

use crate::app::{App, Tool};

/// Une touche d'outil tenue au moins si longtemps (secondes) ramène l'outil d'avant au relâché.
pub const HOLD: f64 = 0.35;
/// Ctrl tenu seul si longtemps (secondes) sur le dessin : le lasso (tout de suite si on appuie
/// sur le dessin).
pub const CTRL_DELAY: f64 = 0.15;

/// Une touche d'outil enfoncée : l'outil d'avant, quand, si le dessin a servi, et quand elle a
/// été relâchée.
pub(crate) struct ToolSpring {
    key: Key,
    prev: Tool,
    since: f64,
    used: bool,
    released: Option<f64>,
}

/// Ctrl enfoncé : depuis quand, l'outil d'avant si le lasso est pris, et s'il servait à autre
/// chose (une autre touche, la molette).
pub(crate) struct CtrlLasso {
    since: f64,
    prev: Option<Tool>,
    spoiled: bool,
}

/// Au relâché d'une touche d'outil tenue `held` secondes (`used` : le dessin a servi entre-temps) :
/// revient-on à l'outil d'avant ?
pub fn back_on_release(held: f64, used: bool) -> bool {
    used || held >= HOLD
}

impl App {
    /// Une action lancée par la touche `key` a changé d'outil (avant : `before`) : la touche
    /// devient un ressort.
    pub(crate) fn spring_after(&mut self, key: Key, before: Tool, now: f64) {
        if self.tool != before {
            self.tool_spring = Some(ToolSpring { key, prev: before, since: now, used: false, released: None });
        }
    }

    /// On appuie sur le dessin : l'outil pris par une touche tenue sert.
    pub(crate) fn spring_used(&mut self) {
        if let Some(s) = &mut self.tool_spring {
            s.used = true;
        }
    }

    /// Une fois par image d'écran, avant les raccourcis : les touches d'outil relâchées et Ctrl.
    pub(crate) fn update_springs(&mut self, ctx: &egui::Context) {
        let (now, released, other_key, ctrl, wheel, down, pressed) = ctx.input(|i| {
            let released: Vec<Key> = i
                .events
                .iter()
                .filter_map(|e| match e {
                    Event::Key { key, pressed: false, .. } => Some(*key),
                    _ => None,
                })
                .collect();
            let other = i.events.iter().any(|e| matches!(e, Event::Key { pressed: true, .. }));
            let m = i.modifiers;
            let wheel = i.zoom_delta() != 1.0 || i.raw_scroll_delta != egui::Vec2::ZERO;
            // (Ctrl+Maj compte aussi : « tous les calques » et le cumul de la sélection, `selobjet.rs`.)
            (i.time, released, other, m.ctrl && !m.alt, wheel, i.pointer.any_down(), i.pointer.any_pressed())
        });
        // Touche d'outil relâchée : l'outil d'avant revient (une fois la souris relâchée).
        if let Some(s) = &mut self.tool_spring {
            if s.released.is_none() && released.contains(&s.key) {
                s.released = Some(now);
            }
        }
        if let Some(at) = self.tool_spring.as_ref().and_then(|s| s.released) {
            if !down {
                let s = self.tool_spring.take().expect("ressort");
                if back_on_release(at - s.since, s.used) && self.tool != s.prev {
                    self.set_tool(s.prev);
                    self.status = format!("Retour à l'outil {}.", s.prev.label());
                }
            }
        }
        // Ctrl seul tenu sur le dessin : le lasso.
        if !ctrl {
            if let Some(c) = &self.ctrl_lasso {
                if down && c.prev.is_some() {
                    return;
                }
                let prev = c.prev;
                self.ctrl_lasso = None;
                if let Some(prev) = prev.filter(|_| self.tool == Tool::Select) {
                    if self.bsel.is_some() {
                        // Une zone bitmap n'existe qu'avec l'outil Sélection : il reste, sinon elle
                        // disparaîtrait au relâché de Ctrl (on la déplace aux flèches, on la pose
                        // en changeant d'outil).
                        self.status = format!("Zone choisie : l'outil Sélection reste (flèches pour la déplacer ; {} pour la poser).", prev.label());
                    } else {
                        self.set_tool(prev);
                    }
                }
            }
            return;
        }
        let c = self.ctrl_lasso.get_or_insert(CtrlLasso { since: now, prev: None, spoiled: false });
        c.spoiled |= other_key || wheel;
        let (since, taken, spoiled) = (c.since, c.prev.is_some(), c.spoiled);
        let on_canvas = self.hover_canvas.is_some();
        // Pas au milieu d'un trait : seulement souris relâchée, ou un appui qui commence.
        let free = !down || pressed;
        let ready = !taken && !spoiled && on_canvas && free && (pressed || now - since >= CTRL_DELAY);
        // Avec l'outil Transformer, Ctrl sert à l'enveloppe de Bézier (`envelopetools.rs`).
        if ready && self.tool != Tool::Select && self.tool != Tool::Transform {
            let prev = self.tool;
            self.set_tool(Tool::Select);
            if let Some(c) = &mut self.ctrl_lasso {
                c.prev = Some(prev);
            }
            self.status = "Lasso tant que Ctrl est tenu.".into();
        }
    }
}
