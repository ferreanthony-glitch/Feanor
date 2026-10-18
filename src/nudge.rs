//! Déplacer la sélection aux flèches (Anthony, 2026-10-17 : « quand un objet est sélectionné,
//! il faut rendre possible le déplacement par les flèches directionnelles : déplacement au
//! pixel, déplacement par 10 pixels quand Maj est maintenu »).
//! - Avec une sélection affichée sur le dessin (traits vectoriels choisis, quel que soit l'outil ;
//!   zone bitmap, avec l'outil Sélection), et quand le dessin a la main (pas le montage ni la
//!   timeline) : une flèche déplace
//!   la sélection d'un pixel du dessin, Maj + flèche de 10. Les flèches avec Ctrl ou Alt gardent
//!   leurs raccourcis ; sans sélection, les flèches changent d'image ou de calque comme avant.
//! - Annulation : en vectoriel, une étape par flèche tenue (`App::gesture_change`) ; en bitmap,
//!   les pixels restent soulevés et se reposent en une étape, comme après un glisser.

use eframe::egui::{vec2, Key, Modifiers, Vec2};

use crate::app::App;
use crate::vector::Affine;

/// Les quatre flèches.
pub(crate) const ARROWS: [Key; 4] = [Key::ArrowLeft, Key::ArrowRight, Key::ArrowUp, Key::ArrowDown];
/// Geste d'annulation des déplacements aux flèches.
pub(crate) const GESTURE: u64 = 0x6e75_6467_6573_656c;
/// Pas avec Maj, en pixels.
pub(crate) const BIG_STEP: f32 = 10.0;

/// Déplacement d'une flèche (en pixels du dessin, y vers le bas) : 1, ou `BIG_STEP` avec Maj ;
/// aucun pour une autre touche, ou avec Ctrl ou Alt.
pub(crate) fn arrow_step(key: Key, m: Modifiers) -> Option<Vec2> {
    if m.ctrl || m.command || m.mac_cmd || m.alt {
        return None;
    }
    let dir = match key {
        Key::ArrowLeft => vec2(-1.0, 0.0),
        Key::ArrowRight => vec2(1.0, 0.0),
        Key::ArrowUp => vec2(0.0, -1.0),
        Key::ArrowDown => vec2(0.0, 1.0),
        _ => return None,
    };
    Some(dir * if m.shift { BIG_STEP } else { 1.0 })
}

impl App {
    /// Les flèches déplacent-elles la sélection ? Une sélection affichée sur le dessin, et le dessin
    /// a la main. Des traits vectoriels choisis restent choisis (et affichés) avec un autre outil,
    /// par exemple après le lasso de Ctrl, Ctrl relâché : les flèches les déplacent aussi (Anthony,
    /// 2026-10-17 : « quand un objet est sous sélection, il n'est pas déplaçable en tapant sur les
    /// flèches »). Une zone bitmap n'existe qu'avec l'outil Sélection.
    pub(crate) fn nudge_ready(&self) -> bool {
        let drawing_has_focus = !self.panel_focus && !self.images_focus && !self.key_focus;
        let vector = self.vsel.as_ref().is_some_and(|s| !s.items.is_empty() && Some(s.cel) == self.cel_here());
        let bitmap = self.bsel.is_some() && self.tool.keeps_zone();
        drawing_has_focus && (vector || bitmap)
    }

    /// Déplace la sélection de `d` pixels du dessin.
    pub(crate) fn nudge_selection(&mut self, d: Vec2) {
        if !self.layer_editable() {
            return;
        }
        if self.bsel.is_some() {
            // Bitmap : les pixels sont soulevés (s'ils ne l'étaient pas) et reposés plus loin.
            if !self.lift_selection() {
                return;
            }
            let aff0 = self.bsel.as_ref().and_then(|s| s.lifted.as_ref()).map_or(Affine::IDENTITY, |l| l.aff);
            self.place_floating(aff0.then(&Affine::translate(d)));
        } else {
            let Some((cel, items)) = self.vsel.as_ref().map(|s| (s.cel, s.items.clone())) else { return };
            let Some(before) = self.drawing(cel).cloned() else { return };
            let aff = Affine::translate(d);
            if let Some(drawing) = self.drawing_mut(cel) {
                for i in items {
                    if let Some(e) = drawing.elements.get_mut(i) {
                        *e = e.transformed(&aff);
                    }
                }
            }
            self.refresh_cel(cel);
            self.gesture_change(GESTURE, |app| app.push_vector(cel, before, None));
        }
        self.status = format!("Sélection déplacée de {} px (flèches ; Maj + flèche : {} px).", d.length().round(), BIG_STEP);
    }
}
