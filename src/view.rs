use std::f32::consts::{PI, TAU};

use eframe::egui::{vec2, Pos2, Rect, Vec2};

/// Cadrage du canevas à l'écran : zoom, rotation, miroir et décalage.
#[derive(Clone, Copy)]
pub struct View {
    pub zoom: f32,
    /// En radians.
    pub rotation: f32,
    pub flip: bool,
    /// Décalage du centre du canevas par rapport au centre de la zone d'affichage.
    pub pan: Vec2,
}

impl Default for View {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            rotation: 0.0,
            flip: false,
            pan: Vec2::ZERO,
        }
    }
}

fn rotate(v: Vec2, angle: f32) -> Vec2 {
    let (s, c) = angle.sin_cos();
    vec2(v.x * c - v.y * s, v.x * s + v.y * c)
}

impl View {
    pub fn to_screen(&self, rect: Rect, size: Vec2, p: Pos2) -> Pos2 {
        let mut v = p.to_vec2() - size * 0.5;
        if self.flip {
            v.x = -v.x;
        }
        rect.center() + self.pan + rotate(v, self.rotation) * self.zoom
    }

    pub fn to_canvas(&self, rect: Rect, size: Vec2, s: Pos2) -> Pos2 {
        let v = (s - rect.center() - self.pan) / self.zoom;
        let mut v = rotate(v, -self.rotation);
        if self.flip {
            v.x = -v.x;
        }
        (size * 0.5 + v).to_pos2()
    }

    /// Applique `change` en gardant immobile le point du canevas situé sous `anchor`.
    fn keep_anchor(&mut self, rect: Rect, size: Vec2, anchor: Pos2, change: impl FnOnce(&mut Self)) {
        let c = self.to_canvas(rect, size, anchor);
        change(self);
        let moved = self.to_screen(rect, size, c);
        self.pan += anchor - moved;
    }

    pub fn zoom_around(&mut self, rect: Rect, size: Vec2, anchor: Pos2, factor: f32) {
        self.keep_anchor(rect, size, anchor, |v| {
            v.zoom = (v.zoom * factor).clamp(0.02, 64.0)
        });
    }

    pub fn rotate_around(&mut self, rect: Rect, size: Vec2, anchor: Pos2, delta: f32) {
        self.keep_anchor(rect, size, anchor, |v| {
            v.rotation = (v.rotation + delta + PI).rem_euclid(TAU) - PI
        });
    }

    pub fn reset_rotation(&mut self, rect: Rect, size: Vec2) {
        self.keep_anchor(rect, size, rect.center(), |v| v.rotation = 0.0);
    }

    pub fn toggle_flip(&mut self, rect: Rect, size: Vec2) {
        self.keep_anchor(rect, size, rect.center(), |v| v.flip = !v.flip);
    }

    pub fn fit(&mut self, rect: Rect, size: Vec2) {
        self.zoom = (rect.width() / size.x).min(rect.height() / size.y) * 0.92;
        self.pan = Vec2::ZERO;
        self.rotation = 0.0;
    }
}
