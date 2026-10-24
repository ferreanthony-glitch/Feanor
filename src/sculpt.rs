//! Outils Grab et Pull (Anthony, 2026-10-24 : les outils « Grab » et « Pull » du mode sculpture de
//! Grease Pencil de Blender, « avec les mêmes options »). Le cœur est ici, sans interface (l'outil
//! dans l'application : `sculptapp.rs`). Code à nous : on a vu ce que font les outils, pas lu leur code.
//!
//! * **Grab** : à l'appui, on prend les points du dessin qui sont dans le rayon du pinceau, chacun avec
//!   un poids tiré de la courbe d'atténuation et de sa distance au point d'appui. Pendant le glisser,
//!   les points pris bougent de `décalage × poids × force`. L'ensemble de points est fixé à l'appui.
//! * **Pull** : continu. À chaque pas, les points qui sont **maintenant** sous le pinceau bougent du
//!   mouvement du curseur (`décalage × poids × force`), comme un doigt qui étire le trait. L'ensemble
//!   change avec le pinceau.
//!
//! Sur un calque vectoriel, ce sont les points des traits (et des zones remplies) qui bougent : les
//! épaisseurs et les appuis restent ; un long écart entre deux points reçoit des points de plus, pour
//! que le trait étiré reste lisse (`densifier`). Sur un calque bitmap, c'est une déformation façon
//! « liquéfier » : les pixels du rayon sont déplacés du même décalage pondéré. L'aplat du dessin passe
//! par les mêmes fonctions : il suit.
//!
//! Sans latence : rien ici ne rejoue le moteur de brosses. Un geste ne touche que les pixels du
//! cercle du pinceau (et les points qui y sont) ; le rendu exact des traits déformés vient d'un fil de
//! travail après le lâcher (`rejeu.rs`).

use eframe::egui::{pos2, Color32, Pos2, Rect, Vec2};

use crate::raster::{Raster, TILE, WORLD};
use crate::vector::{Element, VPoint, VectorDrawing};

// ---------------------------------------------------------------------------------------------- réglages

/// La courbe d'atténuation : le poids d'un point selon sa distance au centre du pinceau.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Attenuation {
    /// Descente en S (le poids tombe doucement du centre, plat au bord).
    #[default]
    Douce,
    /// Descente en ligne droite.
    Lineaire,
    /// Plein poids partout dans le cercle, rien au-delà.
    Constante,
    /// Pic au centre, qui tombe vite (le carré d'une descente droite).
    Pointue,
}

impl Attenuation {
    pub const ALL: [Attenuation; 4] = [Attenuation::Douce, Attenuation::Lineaire, Attenuation::Constante, Attenuation::Pointue];

    pub fn label(self) -> &'static str {
        match self {
            Attenuation::Douce => "Douce",
            Attenuation::Lineaire => "Linéaire",
            Attenuation::Constante => "Constante",
            Attenuation::Pointue => "Pointue",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Attenuation::Douce => "L'effet tombe doucement du centre vers le bord du cercle.",
            Attenuation::Lineaire => "L'effet tombe en ligne droite du centre vers le bord.",
            Attenuation::Constante => "Tout le cercle bouge pareil, d'un bloc ; rien en dehors.",
            Attenuation::Pointue => "L'effet se concentre au centre et tombe vite.",
        }
    }

    /// Icône du choix.
    pub fn icon(self) -> &'static str {
        use egui_phosphor::regular as ph;
        match self {
            Attenuation::Douce => ph::WAVE_SINE,
            Attenuation::Lineaire => ph::WAVE_TRIANGLE,
            Attenuation::Constante => ph::WAVE_SQUARE,
            Attenuation::Pointue => ph::MOUNTAINS,
        }
    }

    /// Le poids (de 0 à 1) à la distance relative `t` du centre : 0 au centre, 1 au bord du cercle.
    /// Au bord et au-delà, rien.
    pub fn weight(self, t: f32) -> f32 {
        if t.is_nan() || t >= 1.0 {
            return 0.0;
        }
        let t = t.max(0.0);
        let s = 1.0 - t;
        match self {
            Attenuation::Douce => s * s * (3.0 - 2.0 * s),
            Attenuation::Lineaire => s,
            Attenuation::Constante => 1.0,
            Attenuation::Pointue => s * s,
        }
    }
}

/// Les réglages d'un des deux outils, comme dans le panneau de Blender : rayon, force (chacun peut suivre la
/// pression du stylet), atténuation, cercle du pinceau affiché.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reglages {
    /// Rayon du pinceau, en pixels du dessin.
    pub rayon: f32,
    /// Le rayon suit la pression du stylet.
    pub rayon_appui: bool,
    /// Force, de 0 (rien ne bouge) à 1 (le centre suit le curseur).
    pub force: f32,
    /// La force suit la pression du stylet.
    pub force_appui: bool,
    pub attenuation: Attenuation,
    /// Montrer le cercle du pinceau sur le dessin.
    pub curseur: bool,
}

impl Reglages {
    pub const RAYON_MIN: f32 = 2.0;
    pub const RAYON_MAX: f32 = 400.0;

    /// Grab : le centre suit le curseur.
    pub fn grab() -> Reglages {
        Reglages { rayon: 60.0, rayon_appui: false, force: 1.0, force_appui: false, attenuation: Attenuation::Douce, curseur: true }
    }

    /// Pull : un peu moins que le curseur, pour que le trait tire sans coller au doigt.
    pub fn pull() -> Reglages {
        Reglages { rayon: 40.0, rayon_appui: false, force: 0.7, force_appui: false, attenuation: Attenuation::Douce, curseur: true }
    }

    /// Rayon et force à la pression `appui` (1 à la souris). La pression n'agit que si son interrupteur est allumé.
    pub fn effectifs(&self, appui: f32) -> (f32, f32) {
        let appui = if appui.is_finite() { appui.clamp(0.05, 1.0) } else { 1.0 };
        let rayon = if self.rayon_appui { self.rayon * appui } else { self.rayon };
        let force = if self.force_appui { self.force * appui } else { self.force };
        (rayon.clamp(1.0, Self::RAYON_MAX), force.clamp(0.0, 1.0))
    }
}

/// Les réglages des deux outils (chacun garde les siens, comme dans Blender).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SculpteReglages {
    pub grab: Reglages,
    pub pull: Reglages,
}

impl Default for SculpteReglages {
    fn default() -> Self {
        SculpteReglages { grab: Reglages::grab(), pull: Reglages::pull() }
    }
}

impl SculpteReglages {
    /// Les réglages de l'outil (Grab, sinon Pull).
    pub fn de(&self, tool: crate::app::Tool) -> &Reglages {
        if tool == crate::app::Tool::Pull {
            &self.pull
        } else {
            &self.grab
        }
    }

    pub fn de_mut(&mut self, tool: crate::app::Tool) -> &mut Reglages {
        if tool == crate::app::Tool::Pull {
            &mut self.pull
        } else {
            &mut self.grab
        }
    }
}

// ---------------------------------------------------------------------------------------------- calques vectoriels

/// Écart le plus grand entre deux points d'un trait dans le cercle du pinceau : au-delà, des points sont ajoutés.
pub fn pas_de(rayon: f32) -> f32 {
    (rayon / 8.0).max(2.0)
}

/// Au plus tant de points ajoutés dans un seul écart (une forme tracée en deux points reste raisonnable).
const MAX_AJOUTS: usize = 400;

/// Où est un point : l'élément du dessin, son contour (zones remplies ; 0 pour un trait) et son rang.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Emplacement {
    pub element: usize,
    pub contour: usize,
    pub point: usize,
}

/// Un point pris par Grab : où il est, d'où il part, et son poids.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prise {
    pub at: Emplacement,
    pub depart: Pos2,
    pub poids: f32,
}

fn lerp_point(a: VPoint, b: VPoint, t: f32) -> VPoint {
    VPoint { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t, r: a.r + (b.r - a.r) * t }
}

/// Distance du point `p` au segment `a`–`b`.
fn dist_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    let t = if len2 > 0.0 { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (a + ab * t).distance(p)
}

/// Combien de morceaux pour couper un écart de `len` en morceaux de `pas` au plus (1 : rien à ajouter).
fn morceaux(len: f32, pas: f32) -> usize {
    if !(len > pas) {
        return 1;
    }
    ((len / pas).ceil() as usize).clamp(1, MAX_AJOUTS + 1)
}

/// Ajoute des points sur les écarts trop longs des traits et des zones remplies qui passent à moins de `rayon` de
/// `centre`, sans changer leur forme : les points ajoutés sont sur le segment, avec le rayon, l'appui et l'instant
/// interpolés. Rend vrai si un élément a changé (ses pixels en cache sont alors à refaire).
pub fn densifier(d: &mut VectorDrawing, centre: Pos2, rayon: f32, pas: f32) -> bool {
    let mut changed = false;
    for e in d.elements.iter_mut() {
        changed |= densifier_element(e, centre, rayon, pas);
    }
    changed
}

/// `densifier` pour un seul élément.
fn densifier_element(e: &mut Element, centre: Pos2, rayon: f32, pas: f32) -> bool {
    let mut changed = false;
    {
        match e {
            Element::Stroke { points, trace, .. } if points.len() >= 2 => {
                let n = points.len();
                let a_couper = |i: usize| {
                    let (a, b) = (points[i].pos(), points[i + 1].pos());
                    morceaux(a.distance(b), pas) > 1 && dist_segment(centre, a, b) <= rayon
                };
                if !(0..n - 1).any(a_couper) {
                    return false;
                }
                let alignee = trace.aligned(n);
                let mut out: Vec<VPoint> = Vec::with_capacity(n + 16);
                let mut samples: Vec<(f32, f64)> = Vec::new();
                for i in 0..n - 1 {
                    out.push(points[i]);
                    if alignee {
                        samples.extend(trace.sample(n, i, 0.0));
                    }
                    let (a, b) = (points[i], points[i + 1]);
                    let k = if a_couper(i) { morceaux(a.pos().distance(b.pos()), pas) } else { 1 };
                    for j in 1..k {
                        let t = j as f32 / k as f32;
                        out.push(lerp_point(a, b, t));
                        if alignee {
                            samples.extend(trace.sample(n, i, t));
                        }
                    }
                }
                out.push(points[n - 1]);
                if alignee {
                    samples.extend(trace.sample(n, n - 1, 0.0));
                    *trace = trace.with_samples(&samples);
                } else {
                    *trace = trace.without_samples();
                }
                *points = out;
                changed = true;
            }
            Element::Fill { contours, .. } => {
                for c in contours.iter_mut() {
                    let n = c.len();
                    if n < 2 {
                        continue;
                    }
                    let seg = |i: usize| (pos2(c[i][0], c[i][1]), pos2(c[(i + 1) % n][0], c[(i + 1) % n][1]));
                    let a_couper = |i: usize| {
                        let (a, b) = seg(i);
                        morceaux(a.distance(b), pas) > 1 && dist_segment(centre, a, b) <= rayon
                    };
                    if !(0..n).any(a_couper) {
                        continue;
                    }
                    let mut out: Vec<[f32; 2]> = Vec::with_capacity(n + 16);
                    for i in 0..n {
                        out.push(c[i]);
                        let (a, b) = seg(i);
                        let k = if a_couper(i) { morceaux(a.distance(b), pas) } else { 1 };
                        for j in 1..k {
                            let t = j as f32 / k as f32;
                            out.push([a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t]);
                        }
                    }
                    *c = out;
                    changed = true;
                }
            }
            _ => {}
        }
    }
    changed
}

/// Les points du dessin qui sont dans le cercle (`centre`, `rayon`), avec leur poids (Grab). Les textes ne sont pas
/// déformés (ils restent rigides).
pub fn prendre(d: &VectorDrawing, centre: Pos2, rayon: f32, att: Attenuation) -> Vec<Prise> {
    let mut out = Vec::new();
    let mut add = |element: usize, contour: usize, point: usize, p: Pos2| {
        let w = att.weight(p.distance(centre) / rayon);
        if w > 0.0 {
            out.push(Prise { at: Emplacement { element, contour, point }, depart: p, poids: w });
        }
    };
    for (ei, e) in d.elements.iter().enumerate() {
        match e {
            Element::Stroke { points, .. } => {
                for (pi, p) in points.iter().enumerate() {
                    add(ei, 0, pi, p.pos());
                }
            }
            Element::Fill { contours, .. } => {
                for (ci, c) in contours.iter().enumerate() {
                    for (pi, p) in c.iter().enumerate() {
                        add(ei, ci, pi, pos2(p[0], p[1]));
                    }
                }
            }
            Element::Text { .. } => {}
        }
    }
    out
}

/// Grab : chaque point pris va à `départ + décalage × poids × force`. Repart toujours des points de départ (le
/// décalage est celui depuis l'appui) : revenir au point d'appui remet tout en place. Les pixels en cache des
/// traits touchés sont à refaire.
pub fn grab_appliquer(d: &mut VectorDrawing, prises: &[Prise], delta: Vec2, force: f32) {
    let mut dernier: Option<usize> = None;
    for p in prises {
        let Some(e) = d.elements.get_mut(p.at.element) else { continue };
        let q = p.depart + delta * (p.poids * force);
        match e {
            Element::Stroke { points, .. } => {
                if let Some(pt) = points.get_mut(p.at.point) {
                    pt.x = q.x;
                    pt.y = q.y;
                }
            }
            Element::Fill { contours, .. } => {
                if let Some(pt) = contours.get_mut(p.at.contour).and_then(|c| c.get_mut(p.at.point)) {
                    *pt = [q.x, q.y];
                }
            }
            Element::Text { .. } => {}
        }
        if dernier != Some(p.at.element) {
            e.invalidate_cache();
            dernier = Some(p.at.element);
        }
    }
}

/// Les boîtes des éléments d'un dessin (les points seuls, sans l'épaisseur), tenues à jour pendant un geste de Pull :
/// un pas ne regarde que les éléments dont la boîte touche le cercle du pinceau, et pas tous les points d'un gros
/// dessin. Un texte n'a pas de boîte (il ne bouge jamais).
pub struct Boites(Vec<Rect>);

fn boite(e: &Element) -> Rect {
    let mut r = Rect::NOTHING;
    match e {
        Element::Stroke { points, .. } => {
            for p in points {
                r.extend_with(p.pos());
            }
        }
        Element::Fill { contours, .. } => {
            for p in contours.iter().flatten() {
                r.extend_with(pos2(p[0], p[1]));
            }
        }
        Element::Text { .. } => {}
    }
    r
}

impl Boites {
    pub fn de(d: &VectorDrawing) -> Boites {
        Boites(d.elements.iter().map(boite).collect())
    }
}

/// Un pas de Pull : les points qui sont dans le cercle (`centre`, `rayon`) maintenant bougent de
/// `décalage × poids × force`. Des points sont d'abord ajoutés aux écarts trop longs du cercle. `boites` : celles du
/// dessin, tenues à jour par le pas. Rend vrai si quelque chose a bougé.
pub fn pull_pas(d: &mut VectorDrawing, boites: &mut Boites, centre: Pos2, delta: Vec2, rayon: f32, att: Attenuation, force: f32) -> bool {
    if delta.length() * force <= 1e-4 {
        return false;
    }
    let portee = rayon + delta.length();
    let pas = pas_de(rayon);
    let mut moved = false;
    for (i, e) in d.elements.iter_mut().enumerate() {
        if boites.0.get(i).is_none_or(|b| b.distance_to_pos(centre) > portee) {
            continue;
        }
        let mut touched = densifier_element(e, centre, portee, pas);
        match e {
            Element::Stroke { points, .. } => {
                for p in points.iter_mut() {
                    let w = att.weight(p.pos().distance(centre) / rayon);
                    if w > 0.0 {
                        p.x += delta.x * w * force;
                        p.y += delta.y * w * force;
                        moved = true;
                        touched = true;
                    }
                }
            }
            Element::Fill { contours, .. } => {
                for p in contours.iter_mut().flatten() {
                    let w = att.weight(pos2(p[0], p[1]).distance(centre) / rayon);
                    if w > 0.0 {
                        p[0] += delta.x * w * force;
                        p[1] += delta.y * w * force;
                        moved = true;
                        touched = true;
                    }
                }
            }
            Element::Text { .. } => {}
        }
        if touched {
            e.invalidate_cache();
            boites.0[i] = boite(e);
        }
    }
    moved
}

// ---------------------------------------------------------------------------------------------- pixels

/// La courbe d'atténuation en table, indexée par le carré de la distance relative (pas de racine carrée par pixel) :
/// c'est ce qui tient un pas sur les pixels à quelques millisecondes.
struct Courbe {
    table: [f32; 1026],
}

impl Courbe {
    const N: f32 = 1024.0;

    fn new(att: Attenuation) -> Courbe {
        let mut table = [0.0; 1026];
        for (i, w) in table.iter_mut().enumerate().take(1025) {
            *w = att.weight((i as f32 / Self::N).sqrt());
        }
        Courbe { table }
    }

    /// Le poids à la distance relative `t` au carré (0 au-delà du bord).
    #[inline]
    fn poids2(&self, t2: f32) -> f32 {
        if t2 >= 1.0 {
            return 0.0;
        }
        let x = t2 * Self::N;
        let i = x as usize;
        let f = x - i as f32;
        self.table[i] + (self.table[i + 1] - self.table[i]) * f
    }
}

/// Le rectangle `r` en pixels entiers (arrondi vers l'extérieur), ramené dans le monde.
fn pixels(r: Rect) -> (i64, i64, i64, i64) {
    let f = |v: f32, up: bool| -> i64 {
        let v = if up { v.ceil() } else { v.floor() };
        (if v.is_finite() { v as i64 } else { 0 }).clamp(-WORLD, WORLD)
    };
    (f(r.min.x, false), f(r.min.y, false), f(r.max.x, true), f(r.max.y, true))
}

/// Une copie des pixels d'une zone du dessin : on lit dedans pendant qu'on écrit au même endroit.
struct Zone {
    x0: i64,
    y0: i64,
    w: usize,
    h: usize,
    px: Vec<Color32>,
}

impl Zone {
    fn copie(r: &Raster, x0: i64, y0: i64, x1: i64, y1: i64) -> Zone {
        let (w, h) = ((x1 - x0).max(0) as usize, (y1 - y0).max(0) as usize);
        let mut px = vec![Color32::TRANSPARENT; w * h];
        if w > 0 && h > 0 {
            let t = TILE as i64;
            for ty in y0.div_euclid(t)..=(y1 - 1).div_euclid(t) {
                for tx in x0.div_euclid(t)..=(x1 - 1).div_euclid(t) {
                    let Some(tile) = r.tile_at(tx as i32, ty as i32) else { continue };
                    let (ox, oy) = (tx * t, ty * t);
                    let (xa, xb) = (x0.max(ox), x1.min(ox + t));
                    for y in y0.max(oy)..y1.min(oy + t) {
                        let (src, dst) = (((y - oy) as usize) * TILE + (xa - ox) as usize, ((y - y0) as usize) * w + (xa - x0) as usize);
                        let n = (xb - xa) as usize;
                        px[dst..dst + n].copy_from_slice(&tile[src..src + n]);
                    }
                }
            }
        }
        Zone { x0, y0, w, h, px }
    }

    fn pixel(&self, x: i64, y: i64) -> [f32; 4] {
        let (lx, ly) = (x - self.x0, y - self.y0);
        if lx < 0 || ly < 0 || lx as usize >= self.w || ly as usize >= self.h {
            return [0.0; 4];
        }
        let c = self.px[ly as usize * self.w + lx as usize];
        [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32]
    }

    /// Le pixel en (`x`, `y`) du dessin, lissé (bilinéaire, prémultiplié) : le même échantillonnage que
    /// `render::sample`, sur la copie.
    #[inline]
    fn echantillon(&self, x: f32, y: f32) -> Color32 {
        let (fx, fy) = (x - 0.5, y - 0.5);
        let (ix, iy) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - ix, fy - iy);
        let (ix, iy) = (ix as i64, iy as i64);
        let (lx, ly) = (ix - self.x0, iy - self.y0);
        let dedans = lx >= 0 && ly >= 0 && (lx as usize) + 1 < self.w && (ly as usize) + 1 < self.h;
        let (a, b, c, d) = if dedans {
            let i = ly as usize * self.w + lx as usize;
            let (a, b, c, d) = (self.px[i], self.px[i + 1], self.px[i + self.w], self.px[i + self.w + 1]);
            // (Quatre pixels vides : le résultat l'est, sans rien calculer.)
            if (a.a() | b.a() | c.a() | d.a()) == 0 {
                return Color32::TRANSPARENT;
            }
            let v = |c: Color32| [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32];
            (v(a), v(b), v(c), v(d))
        } else {
            (self.pixel(ix, iy), self.pixel(ix + 1, iy), self.pixel(ix, iy + 1), self.pixel(ix + 1, iy + 1))
        };
        let mut px = [0u8; 4];
        for k in 0..4 {
            let top = a[k] + (b[k] - a[k]) * tx;
            let bottom = c[k] + (d[k] - c[k]) * tx;
            px[k] = (top + (bottom - top) * ty).round().clamp(0.0, 255.0) as u8;
        }
        Color32::from_rgba_premultiplied(px[0], px[1], px[2], px[3])
    }
}

/// Écrit les pixels `x0..x1` de la ligne `y` (toile infinie comprise) : `f(x)` rend la couleur à poser, ou rien. La
/// tuile est prise une fois par tronçon de ligne ; un pixel transparent ne crée pas de tuile.
fn ecrire_ligne(r: &mut Raster, y: i64, x0: i64, x1: i64, mut f: impl FnMut(i64) -> Option<Color32>) {
    if y.abs() > WORLD {
        return;
    }
    let t = TILE as i64;
    let ty = y.div_euclid(t) as i32;
    let rang = y.rem_euclid(t) as usize * TILE;
    let mut x = x0.max(-WORLD);
    let x1 = x1.min(WORLD + 1);
    while x < x1 {
        let tx = x.div_euclid(t);
        let fin = ((tx + 1) * t).min(x1);
        let id = r.id_of(tx as i32, ty);
        let existe = r.slot(id).is_some();
        // Les couleurs du tronçon d'abord : la tuile n'est prise (et créée) que s'il y a quelque chose à y poser.
        let mut couleurs: [Option<Color32>; TILE] = [None; TILE];
        let mut a_poser = false;
        for (k, xx) in (x..fin).enumerate() {
            couleurs[k] = f(xx).filter(|c| c.a() != 0 || existe);
            a_poser |= couleurs[k].is_some();
        }
        if a_poser {
            let tile = r.tile_mut(id);
            for (k, xx) in (x..fin).enumerate() {
                if let Some(c) = couleurs[k] {
                    tile[rang + xx.rem_euclid(t) as usize] = c;
                }
            }
        }
        x = fin;
    }
}

/// Remet les tuiles de `dst` qui recouvrent `zone` telles qu'elles sont dans `orig` (rien ne se recopie : les
/// tuiles sont partagées).
pub fn restaurer(dst: &mut Raster, orig: &Raster, zone: Rect) {
    if zone == Rect::NOTHING || !zone.is_finite() {
        return;
    }
    let (x0, y0, x1, y1) = pixels(zone);
    for id in dst.ids_covering(x0, y0, x1, y1) {
        let tile = orig.slot(id).cloned();
        dst.replace_slot(id, tile);
    }
}

/// Les numéros des tuiles qui recouvrent `zone`.
pub fn tuiles_de(r: &Raster, zone: Rect) -> Vec<usize> {
    if zone == Rect::NOTHING || !zone.is_finite() {
        return Vec::new();
    }
    let (x0, y0, x1, y1) = pixels(zone);
    r.ids_covering(x0, y0, x1, y1)
}

/// Combien de fois on reprend l'inverse du pas de Pull (la descente converge vite : le pas est petit devant le rayon).
const ITERATIONS: usize = 3;

/// Un pas de Pull sur des pixels : les pixels du cercle (`centre`, `rayon`) bougent de `décalage × poids × force`,
/// comme les points du dessin vectoriel (`pull_pas`, même poids à la même distance de `centre`). On cherche pour
/// chaque pixel d'arrivée d'où il vient (le poids est celui de l'endroit d'où il vient : quelques reprises),
/// ce qui ne laisse aucun trou. Rend le rectangle touché. Rien hors du cercle ne change.
pub fn pousser(r: &mut Raster, centre: Pos2, delta: Vec2, rayon: f32, att: Attenuation, force: f32) -> Rect {
    let m = delta * force;
    if m.length() < 0.02 || rayon < 1.0 || !centre.x.is_finite() || !centre.y.is_finite() {
        return Rect::NOTHING;
    }
    let reach = Rect::from_center_size(centre, Vec2::splat(2.0 * rayon));
    let (x0, y0, x1, y1) = pixels(reach.expand(1.0));
    let marge = m.length().ceil() as i64 + 2;
    let zone = Zone::copie(r, x0 - marge, y0 - marge, x1 + marge, y1 + marge);
    let courbe = Courbe::new(att);
    let (r2, inv_r2) = (rayon * rayon, 1.0 / (rayon * rayon));
    // La corde du cercle sur la ligne `y` (colonnes de `x0`), s'il y en a une.
    let corde = |y: i64| -> Option<(i64, i64)> {
        let qy = y as f32 + 0.5;
        let h2 = r2 - (qy - centre.y) * (qy - centre.y);
        if h2 <= 0.0 {
            return None;
        }
        let h = h2.sqrt();
        let (xa, xb) = (((centre.x - h - 0.5).ceil() as i64).max(x0), ((centre.x + h - 0.5).floor() as i64 + 1).min(x1));
        (xa < xb).then_some((xa, xb))
    };
    // D'où vient chaque pixel d'arrivée : calculé d'abord, dans une mémoire à part (un gros cercle se partage entre les
    // coeurs, qui lisent la copie et écrivent chacun leurs lignes), puis posé dans le dessin.
    let calculer = |y: i64, ligne: &mut [Color32]| {
        let Some((xa, xb)) = corde(y) else { return };
        let qy = y as f32 + 0.5;
        for x in xa..xb {
            let q = pos2(x as f32 + 0.5, qy);
            let mut s = q;
            for _ in 0..ITERATIONS {
                let (ex, ey) = (s.x - centre.x, s.y - centre.y);
                let w = courbe.poids2((ex * ex + ey * ey) * inv_r2);
                s = pos2(q.x - m.x * w, q.y - m.y * w);
            }
            ligne[(x - x0) as usize] = zone.echantillon(s.x, s.y);
        }
    };
    let (bw, bh) = ((x1 - x0) as usize, (y1 - y0) as usize);
    let mut tampon = vec![Color32::TRANSPARENT; bw * bh];
    let coeurs = std::thread::available_parallelism().map_or(1, |n| n.get()).min(8);
    if bw * bh >= PARALLELE_MIN && coeurs > 1 {
        let lignes = bh.div_ceil(coeurs);
        std::thread::scope(|s| {
            for (k, bloc) in tampon.chunks_mut(lignes * bw).enumerate() {
                let calculer = &calculer;
                s.spawn(move || {
                    for (j, ligne) in bloc.chunks_mut(bw).enumerate() {
                        calculer(y0 + (k * lignes + j) as i64, ligne);
                    }
                });
            }
        });
    } else {
        for (j, ligne) in tampon.chunks_mut(bw).enumerate() {
            calculer(y0 + j as i64, ligne);
        }
    }
    for y in y0..y1 {
        let Some((xa, xb)) = corde(y) else { continue };
        let ligne = &tampon[(y - y0) as usize * bw..(y - y0 + 1) as usize * bw];
        ecrire_ligne(r, y, xa, xb, |x| {
            let (dx, dy) = (x as f32 + 0.5 - centre.x, y as f32 + 0.5 - centre.y);
            (dx * dx + dy * dy < r2).then(|| ligne[(x - x0) as usize])
        });
    }
    reach.expand(1.0)
}

/// Au moins tant de pixels (la boîte du cercle) pour que le calcul d'un pas se partage entre les coeurs.
const PARALLELE_MIN: usize = 40_000;

/// Remplit le triangle `[(arrivée, départ); 3]` ligne par ligne : chaque pixel dont le centre est dedans prend la
/// couleur de la copie à son point de départ (interpolé). `garde` : les pixels à ne pas toucher.
fn remplir_triangle(zone: &Zone, dst: &mut Raster, tri: [(Pos2, Pos2); 3], garde: &dyn Fn(Pos2) -> bool) {
    let [(pa, fa), (pb, fb), (pc, fc)] = tri;
    if ![pa, pb, pc, fa, fb, fc].iter().all(|p| p.x.is_finite() && p.y.is_finite()) {
        return;
    }
    let den = (pb.y - pc.y) * (pa.x - pc.x) + (pc.x - pb.x) * (pa.y - pc.y);
    if den.abs() < 1e-6 {
        return;
    }
    let (ymin, ymax) = (pa.y.min(pb.y).min(pc.y), pa.y.max(pb.y).max(pc.y));
    let (ylo, yhi) = (((ymin - 0.5).ceil() as i64).max(-WORLD), ((ymax - 0.5).floor() as i64).min(WORLD));
    for y in ylo..=yhi {
        let yc = y as f32 + 0.5;
        let (mut xl, mut xr) = (f32::MAX, f32::MIN);
        for (e0, e1) in [(pa, pb), (pb, pc), (pc, pa)] {
            if (e0.y <= yc && yc < e1.y) || (e1.y <= yc && yc < e0.y) {
                let x = e0.x + (yc - e0.y) * (e1.x - e0.x) / (e1.y - e0.y);
                xl = xl.min(x);
                xr = xr.max(x);
            }
        }
        if xl > xr {
            continue;
        }
        let (xlo, xhi) = (((xl - 0.5 - 1e-3).ceil() as i64).max(-WORLD), ((xr - 0.5 + 1e-3).floor() as i64).min(WORLD));
        ecrire_ligne(dst, y, xlo, xhi + 1, |x| {
            let p = pos2(x as f32 + 0.5, yc);
            if !garde(p) {
                return None;
            }
            let l1 = ((pb.y - pc.y) * (p.x - pc.x) + (pc.x - pb.x) * (p.y - pc.y)) / den;
            let l2 = ((pc.y - pa.y) * (p.x - pc.x) + (pa.x - pc.x) * (p.y - pc.y)) / den;
            let l3 = 1.0 - l1 - l2;
            let f = (fa.to_vec2() * l1 + fb.to_vec2() * l2 + fc.to_vec2() * l3).to_pos2();
            let c = zone.echantillon(f.x, f.y);
            (c.a() > 0).then_some(c)
        });
    }
}

/// Grab sur des pixels : `dst` doit être, dans la zone du cercle, tel que `orig` (voir `restaurer`) ; le cercle
/// (`centre`, `rayon`) de `orig` est déformé comme une feuille de caoutchouc dont chaque point va à
/// `départ + décalage × poids × force` (les mêmes déplacements que les points d'un dessin vectoriel). Un maillage
/// de petites cases sur la boîte du cercle, chaque case posée par deux triangles ; les pixels à plus de
/// `rayon + décalage` du centre ne sont jamais touchés. Rend le rectangle touché (ce qu'il faudra
/// `restaurer` au pas suivant).
pub fn saisir_raster(orig: &Raster, dst: &mut Raster, centre: Pos2, rayon: f32, att: Attenuation, force: f32, delta: Vec2) -> Rect {
    let m = delta * force;
    if m.length() < 0.01 || rayon < 1.0 || !centre.x.is_finite() || !centre.y.is_finite() || !m.x.is_finite() || !m.y.is_finite() {
        return Rect::NOTHING;
    }
    let reach = Rect::from_center_size(centre, Vec2::splat(2.0 * rayon)).expand(2.0);
    let (x0, y0, x1, y1) = pixels(reach);
    let (w, h) = ((x1 - x0) as f32, (y1 - y0) as f32);
    if w <= 0.0 || h <= 0.0 {
        return Rect::NOTHING;
    }
    let n = ((w.max(h) / 2.0).ceil() as usize).clamp(6, 40);
    let (cw, ch) = (w / n as f32, h / n as f32);
    // Les sommets du maillage : d'où ils viennent, où ils vont.
    let mut from = Vec::with_capacity((n + 1) * (n + 1));
    let mut to = Vec::with_capacity((n + 1) * (n + 1));
    let mut moved = Vec::with_capacity((n + 1) * (n + 1));
    for j in 0..=n {
        for i in 0..=n {
            let p = pos2(x0 as f32 + cw * i as f32, y0 as f32 + ch * j as f32);
            let wgt = att.weight(p.distance(centre) / rayon);
            from.push(p);
            to.push(p + m * wgt);
            moved.push(wgt > 0.0);
        }
    }
    let at = |i: usize, j: usize| j * (n + 1) + i;
    let mobile = |i: usize, j: usize| moved[at(i, j)] || moved[at(i + 1, j)] || moved[at(i, j + 1)] || moved[at(i + 1, j + 1)];
    let ecart = m.length();
    let portee2 = (rayon + ecart + 1.0) * (rayon + ecart + 1.0);
    let garde = |p: Pos2| (p - centre).length_sq() < portee2;
    let zone = Zone::copie(orig, x0 - 2, y0 - 2, x1 + 2, y1 + 2);
    // Ce que les cases qui bougent couvraient est effacé, puis posé de nouveau là où les triangles le mettent.
    let colonne: Vec<usize> = (x0..x1).map(|x| (((x as f32 + 0.5 - x0 as f32) / cw) as usize).min(n - 1)).collect();
    for y in y0..y1 {
        let qy = y as f32 + 0.5;
        let j = (((qy - y0 as f32) / ch) as usize).min(n - 1);
        ecrire_ligne(dst, y, x0, x1, |x| {
            let i = colonne[(x - x0) as usize];
            (mobile(i, j) && garde(pos2(x as f32 + 0.5, qy))).then_some(Color32::TRANSPARENT)
        });
    }
    for j in 0..n {
        for i in 0..n {
            if !mobile(i, j) {
                continue;
            }
            let v = |i: usize, j: usize| (to[at(i, j)], from[at(i, j)]);
            let (a, b, c, d) = (v(i, j), v(i + 1, j), v(i, j + 1), v(i + 1, j + 1));
            remplir_triangle(&zone, dst, [a, b, d], &garde);
            remplir_triangle(&zone, dst, [a, d, c], &garde);
        }
    }
    let mut touched = reach;
    for p in &to {
        touched.extend_with(*p);
    }
    touched.expand(2.0)
}
