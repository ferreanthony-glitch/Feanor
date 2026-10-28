//! Projets lourds (essai de charge du 2026-10-23) : un générateur de projets « comme un court métrage de 7 à 10 minutes »,
//! puis des bancs de mesure et une chasse aux plantages sur ces projets. Essais `#[ignore]`, lancés à la main :
//!
//! `cargo test --release --offline --target-dir target\lourd lourd_ -- --ignored --nocapture --test-threads=1`
//!
//! Variables : `LOURD_DIR` (dossier de sortie), `LOURD_PLANS` (nombre de plans, 300), `LOURD_NOM` (nom du projet),
//! `LOURD_PROJET` (manifeste à mesurer).
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use eframe::egui::{pos2, vec2, Pos2, Rect};

use crate::brush::{BrushSettings, StrokeMode, StrokeState, Toucher};
use crate::brushtex::BrushLibrary;
use crate::camera::{Camera, CameraKey};
use crate::document::{Cel, Document, Layer, LayerKind};
use crate::fx::{Effect, EffectKind};
use crate::raster::Raster;
use crate::vector::{Element, Trace, VPoint, VectorDrawing};

// ------------------------------------------------------------------ hasard

pub(crate) struct Hasard(u64);

impl Hasard {
    pub fn new(seed: u64) -> Self {
        let mut h = Hasard(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03);
        h.u();
        h.u();
        h
    }
    pub fn u(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32) / (1u64 << 24) as f32
    }
    pub fn entre(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.u()
    }
    /// De `a` à `b` compris.
    pub fn n(&mut self, a: usize, b: usize) -> usize {
        a + ((self.u() * (b - a + 1) as f32) as usize).min(b - a)
    }
    pub fn chance(&mut self, p: f32) -> bool {
        self.u() < p
    }
    pub fn choix<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.n(0, v.len() - 1)]
    }
}

// ------------------------------------------------------------------ chemins de traits

/// Un point de trait : position sur la feuille, appui, instant.
type Pt = (Pos2, f32, f64);

/// Un chemin à partir de points de passage (une courbe de Catmull-Rom), un point tous les ~3 px, avec un appui qui monte
/// et retombe et un léger tremblement de main.
fn chemin(passages: &[Pos2], h: &mut Hasard, tremble: f32) -> Vec<Pt> {
    let mut pts: Vec<Pos2> = Vec::new();
    if passages.len() < 2 {
        return Vec::new();
    }
    for i in 0..passages.len() - 1 {
        let p0 = passages[i.saturating_sub(1)];
        let p1 = passages[i];
        let p2 = passages[i + 1];
        let p3 = passages[(i + 2).min(passages.len() - 1)];
        let n = ((p2 - p1).length() / 3.0).ceil().max(1.0) as usize;
        for k in 0..n {
            let t = k as f32 / n as f32;
            let (t2, t3) = (t * t, t * t * t);
            let x = 0.5 * ((2.0 * p1.x) + (-p0.x + p2.x) * t + (2.0 * p0.x - 5.0 * p1.x + 4.0 * p2.x - p3.x) * t2 + (-p0.x + 3.0 * p1.x - 3.0 * p2.x + p3.x) * t3);
            let y = 0.5 * ((2.0 * p1.y) + (-p0.y + p2.y) * t + (2.0 * p0.y - 5.0 * p1.y + 4.0 * p2.y - p3.y) * t2 + (-p0.y + 3.0 * p1.y - 3.0 * p2.y + p3.y) * t3);
            pts.push(pos2(x, y));
        }
    }
    pts.push(*passages.last().unwrap());
    let n = pts.len();
    let phase = h.entre(0.0, 6.0);
    let mut t = h.entre(0.0, 0.2) as f64;
    let mut out = Vec::with_capacity(n);
    for (k, p) in pts.iter().enumerate() {
        let s = k as f32 / (n - 1).max(1) as f32;
        let appui = (0.2 + 0.8 * (s * std::f32::consts::PI).sin().powf(0.6)).clamp(0.05, 1.0);
        let q = *p + vec2((s * 23.0 + phase).sin(), (s * 17.0 + phase).cos()) * tremble;
        if k > 0 {
            t += (pts[k] - pts[k - 1]).length() as f64 / 900.0;
        }
        out.push((q, appui, t));
    }
    out
}

fn ligne(a: Pos2, b: Pos2, h: &mut Hasard) -> Vec<Pt> {
    let m = a + (b - a) * 0.5 + vec2(h.entre(-1.0, 1.0), h.entre(-1.0, 1.0)) * (b - a).length() * 0.02;
    chemin(&[a, m, b], h, 0.6)
}

fn ellipse(c: Pos2, rx: f32, ry: f32, depart: f32, tour: f32, h: &mut Hasard) -> Vec<Pt> {
    let n = 10;
    let pts: Vec<Pos2> = (0..=n)
        .map(|k| {
            let a = depart + tour * k as f32 / n as f32;
            let w = 1.0 + h.entre(-0.04, 0.04);
            c + vec2(a.cos() * rx * w, a.sin() * ry * w)
        })
        .collect();
    chemin(&pts, h, 0.5)
}

/// Un personnage esquissé, debout, centré en `c` (pieds en bas), de hauteur `haut` : traits de construction (`legers`) et
/// traits de contour (`nets`).
struct Silhouette {
    legers: Vec<Vec<Pt>>,
    nets: Vec<Vec<Pt>>,
    /// Les zones à colorer (tête, torse) : centre et rayons.
    zones: Vec<(Pos2, f32, f32)>,
}

fn silhouette(c: Pos2, haut: f32, h: &mut Hasard) -> Silhouette {
    let u = haut / 8.0;
    let tete = c + vec2(h.entre(-0.3, 0.3) * u, -7.0 * u);
    let cou = tete + vec2(0.0, 1.0 * u);
    let bassin = c + vec2(h.entre(-0.5, 0.5) * u, -3.6 * u);
    let pied_g = c + vec2(-1.2 * u + h.entre(-0.6, 0.6) * u, 0.0);
    let pied_d = c + vec2(1.2 * u + h.entre(-0.6, 0.6) * u, 0.0);
    let main_g = cou + vec2(-2.4 * u + h.entre(-1.0, 1.0) * u, 2.8 * u + h.entre(-1.5, 1.0) * u);
    let main_d = cou + vec2(2.4 * u + h.entre(-1.0, 1.0) * u, 2.8 * u + h.entre(-2.5, 1.0) * u);
    let epaule_g = cou + vec2(-1.1 * u, 0.4 * u);
    let epaule_d = cou + vec2(1.1 * u, 0.4 * u);
    let mut legers = vec![
        ligne(cou, bassin, h),
        ellipse(tete, 0.75 * u, 0.9 * u, 0.0, std::f32::consts::TAU, h),
        ligne(epaule_g, epaule_d, h),
        ligne(bassin + vec2(-0.8 * u, 0.0), bassin + vec2(0.8 * u, 0.0), h),
    ];
    if h.chance(0.5) {
        legers.push(ligne(tete + vec2(-0.7 * u, 0.0), tete + vec2(0.7 * u, 0.1 * u), h));
    }
    let coude_g = epaule_g + (main_g - epaule_g) * 0.5 + vec2(-0.6 * u, 0.0);
    let coude_d = epaule_d + (main_d - epaule_d) * 0.5 + vec2(0.6 * u, 0.0);
    let genou_g = bassin + (pied_g - bassin) * 0.5 + vec2(-0.3 * u, 0.0);
    let genou_d = bassin + (pied_d - bassin) * 0.5 + vec2(0.3 * u, 0.0);
    let mut nets = vec![
        ellipse(tete, 0.8 * u, 0.95 * u, h.entre(0.0, 1.0), std::f32::consts::TAU * 0.95, h),
        chemin(&[epaule_g, coude_g, main_g], h, 0.4),
        chemin(&[epaule_d, coude_d, main_d], h, 0.4),
        chemin(&[bassin + vec2(-0.5 * u, 0.0), genou_g, pied_g], h, 0.4),
        chemin(&[bassin + vec2(0.5 * u, 0.0), genou_d, pied_d], h, 0.4),
        chemin(&[epaule_g, bassin + vec2(-0.9 * u, -0.3 * u), bassin + vec2(-0.7 * u, 0.4 * u)], h, 0.4),
        chemin(&[epaule_d, bassin + vec2(0.9 * u, -0.3 * u), bassin + vec2(0.7 * u, 0.4 * u)], h, 0.4),
        // Yeux, bouche.
        ligne(tete + vec2(-0.35 * u, -0.1 * u), tete + vec2(-0.15 * u, -0.1 * u), h),
        ligne(tete + vec2(0.15 * u, -0.1 * u), tete + vec2(0.35 * u, -0.1 * u), h),
        chemin(&[tete + vec2(-0.25 * u, 0.4 * u), tete + vec2(0.0, 0.5 * u), tete + vec2(0.25 * u, 0.38 * u)], h, 0.2),
    ];
    // Cheveux.
    for k in 0..h.n(3, 7) {
        let a = -2.6 + k as f32 * 0.35;
        let base = tete + vec2(a.cos() * 0.8 * u, a.sin() * 0.95 * u);
        nets.push(chemin(&[base, base + vec2(h.entre(-0.5, 0.5) * u, -h.entre(0.3, 0.9) * u), base + vec2(h.entre(-0.9, 0.9) * u, -h.entre(0.6, 1.2) * u)], h, 0.3));
    }
    // Pieds, mains.
    nets.push(ligne(pied_g, pied_g + vec2(-0.6 * u, 0.0), h));
    nets.push(ligne(pied_d, pied_d + vec2(0.6 * u, 0.0), h));
    nets.push(ellipse(main_g, 0.25 * u, 0.3 * u, 0.0, std::f32::consts::TAU, h));
    nets.push(ellipse(main_d, 0.25 * u, 0.3 * u, 0.0, std::f32::consts::TAU, h));
    Silhouette { legers, nets, zones: vec![(tete, 0.75 * u, 0.9 * u), (cou + (bassin - cou) * 0.5, 1.0 * u, 1.4 * u)] }
}

// ------------------------------------------------------------------ éléments et peinture

fn reglage(lib: &BrushLibrary, nom: &str, taille: f32) -> BrushSettings {
    let mut s = lib.presets.iter().find(|p| p.name == nom).unwrap_or_else(|| panic!("brosse « {nom} »")).settings;
    s.size = taille;
    s
}

/// Un trait vectoriel texturé (comme un lâcher du pinceau le pose), le long de `path`.
fn element(lib: &BrushLibrary, s: &BrushSettings, color: [u8; 3], path: &[Pt]) -> Element {
    let points = path.iter().map(|(p, pr, _)| VPoint { x: p.x, y: p.y, r: (s.size * 0.5 * if s.pressure_size { pr.max(0.2) } else { 1.0 }).max(0.5) }).collect();
    let trace = Trace { pressure: path.iter().map(|p| p.1).collect(), time: path.iter().map(|p| p.2).collect(), feel: Some(Toucher::default()), secondary: None, cache: Default::default() };
    Element::Stroke { points, color, opacity: s.opacity, hardness: s.hardness, brush: lib.for_vector(s).map(Box::new), trace }
}

/// Un trait peint par le moteur bitmap dans `r`.
fn peindre(r: &mut Raster, lib: &BrushLibrary, s: &BrushSettings, color: [u8; 3], path: &[Pt]) {
    if path.len() < 2 {
        return;
    }
    let mut st = StrokeState::new(0, StrokeMode::Paint(color), *s, None, true).with_textures(lib.textures(s)).with_feel(Toucher::default());
    let mut dirty = Vec::new();
    for &(p, pr, t) in path {
        st.add_point_at(r, p, pr, t, &mut dirty);
    }
    st.finish(r, &mut dirty);
}

// ------------------------------------------------------------------ un plan, fabriqué hors du document (sur plusieurs fils)

#[derive(Clone, Copy, PartialEq, Debug)]
enum Genre {
    Decor,
    Crayonne,
    Perso(usize),
    /// Calque de couleur du calque de trait de rang `.0` dans la liste des calques du plan.
    Couleur(usize),
    Accessoires,
    Effet(EffectKind),
}

enum Dessin {
    Bitmap(Raster),
    Vecteur(Vec<Element>),
    /// Un dessin vide (pose d'un calque d'effet).
    Pose,
}

struct CalqueGen {
    nom: String,
    genre: Genre,
    dessins: Vec<Dessin>,
    /// Image par image : le rang du dessin montré.
    frames: Vec<Option<usize>>,
    fx: Vec<Effect>,
}

struct PlanGen {
    len: usize,
    /// Débuts des cases après la première.
    marques: Vec<usize>,
    calques: Vec<CalqueGen>,
    camera: Vec<CameraKey>,
    action: String,
    dialogue: String,
    notes: String,
    textes_marques: Vec<(String, String, String)>,
    anime: bool,
    traits: usize,
}

/// Les personnages du film (pastilles) : nom, trait, aplat.
const PERSONNAGES: [(&str, [u8; 3], [u8; 3]); 6] = [
    ("Léa", [180, 40, 40], [250, 205, 190]),
    ("Max", [40, 70, 170], [200, 215, 245]),
    ("Grand-mère", [90, 60, 30], [235, 220, 190]),
    ("Le chien", [60, 120, 50], [210, 235, 200]),
    ("Le facteur", [120, 50, 140], [230, 205, 240]),
    ("Foule", [70, 70, 70], [220, 220, 220]),
];

const MOTS_ACTION: [&str; 16] = [
    "Léa traverse la cour en courant", "Max se retourne, surpris", "La porte claque derrière eux", "Le chien aboie vers la fenêtre",
    "Plan large sur la rue sous la pluie", "Grand-mère pose la tasse", "Ils se regardent sans rien dire", "Le facteur tend une lettre",
    "La foule s'écarte", "Léa grimpe sur le muret", "Gros plan sur la lettre froissée", "Max court vers la gare",
    "Le train démarre", "La lumière baisse d'un coup", "Ombre qui passe sur le mur", "Ils éclatent de rire",
];
const MOTS_DIALOGUE: [&str; 10] = [
    "LÉA : « On part. Maintenant. »", "MAX : « Attends-moi ! »", "GRAND-MÈRE : « Tu as oublié ton écharpe. »", "(silence)",
    "LE FACTEUR : « C'est pour vous. »", "LÉA : « Non… pas encore. »", "MAX : « Tu l'as vu ? »", "", "", "",
];
const MOTS_NOTES: [&str; 10] = [
    "Caméra épaule, léger tremblé", "Tenir le regard 4 images de plus", "Ralenti ½", "Raccord dans l'axe", "Son : pluie off",
    "Prévoir un décor plus large", "Flou de premier plan", "", "Couleur : heure bleue", "Reprendre la pose de la main",
];

/// Fabrique un plan (dessins compris) sans toucher au document : de quoi travailler sur plusieurs fils.
fn fabriquer_plan(seed: u64, lib: &BrushLibrary, feuille: (usize, usize), cadre: Rect) -> PlanGen {
    let mut h = Hasard::new(seed);
    // Variante « extrême » légère : beaucoup de plans, peu de pixels (décor au crayon, un personnage vectoriel, pas de bitmap de plus).
    let leger = std::env::var_os("LOURD_LEGER").is_some();
    let (w, hh) = feuille;
    let n_cases = h.n(2, 5);
    let anime = h.chance(0.08);
    let longueurs: Vec<usize> = if anime { vec![h.n(24, 48)] } else { (0..n_cases).map(|_| h.n(8, 18)).collect() };
    let len: usize = longueurs.iter().sum();
    let mut debuts = vec![0];
    for l in &longueurs[..longueurs.len() - 1] {
        debuts.push(debuts.last().unwrap() + l);
    }
    let case_de = |f: usize| debuts.iter().rposition(|d| *d <= f).unwrap_or(0);

    // Caméra : un plan sur cinq bouge.
    let mut camera = Vec::new();
    let centre = cadre.center();
    let mut marge = 0.0f32;
    if h.chance(0.2) && len > 2 {
        let c0 = Camera { x: centre.x, y: centre.y, zoom: 1.0, angle: 0.0, dolly: 0.0, tilt: 0.0, pan: 0.0 };
        let c1 = match h.n(0, 3) {
            0 => {
                // Travelling latéral.
                let dx = h.entre(-1.0, 1.0).signum() * h.entre(200.0, 520.0);
                marge = dx.abs();
                Camera { x: centre.x + dx, ..c0 }
            }
            1 => Camera { zoom: h.entre(1.25, 1.8), y: centre.y - h.entre(0.0, 80.0), ..c0 },
            2 => Camera { tilt: h.entre(-6.0, 8.0), pan: h.entre(-4.0, 4.0), ..c0 },
            _ => {
                marge = 260.0;
                Camera { x: centre.x + h.entre(-260.0, 260.0), zoom: h.entre(1.1, 1.5), angle: h.entre(-4.0, 4.0), ..c0 }
            }
        };
        camera.push(CameraKey { ease_out: h.entre(0.0, 8.0).round(), ..CameraKey::new(0, c0) });
        camera.push(CameraKey { ease_in: h.entre(0.0, 8.0).round(), ..CameraKey::new(len - 1, c1) });
    }
    let zone = cadre.expand2(vec2(marge + 80.0, 60.0));
    let mut traits = 0;

    // Calques : décor, crayonné (souvent), personnage A (+ sa couleur, une fois sur deux), personnage B (une fois sur deux),
    // accessoires (parfois), calques d'effet (un sur cinq, deux sur vingt).
    let mut calques: Vec<CalqueGen> = Vec::new();
    // Le décor : un seul dessin pour tout le plan.
    {
        let mut r = Raster::new(w, hh);
        let crayon = crate::decor::Genre::Crayon.reglage(crate::decor::Genre::Crayon.taille_defaut());
        let feutre = crate::decor::Genre::Feutre.reglage(crate::decor::Genre::Feutre.taille_defaut());
        let aplat = crate::decor::Genre::Aplat.reglage(crate::decor::Genre::Aplat.taille_defaut());
        let col = crate::decor::COULEUR_DEFAUT;
        let horizon = zone.top() + zone.height() * h.entre(0.35, 0.6);
        let fuite = pos2(h.entre(zone.left(), zone.right()), horizon);
        let mut decor_traits = vec![(crayon, ligne(pos2(zone.left(), horizon), pos2(zone.right(), horizon), &mut h))];
        for _ in 0..h.n(5, 10) {
            let bord = pos2(h.entre(zone.left(), zone.right()), if h.chance(0.5) { zone.bottom() } else { zone.top() });
            decor_traits.push((crayon, ligne(fuite, bord, &mut h)));
        }
        for _ in 0..h.n(3, 6) {
            let x0 = h.entre(zone.left(), zone.right() - 200.0);
            let bw = h.entre(120.0, 420.0);
            let top = horizon - h.entre(150.0, 600.0);
            let pts = [pos2(x0, horizon + 40.0), pos2(x0, top), pos2(x0 + bw, top + h.entre(-40.0, 40.0)), pos2(x0 + bw, horizon + 40.0)];
            for k in 0..3 {
                decor_traits.push((crayon, ligne(pts[k], pts[k + 1], &mut h)));
            }
            for _ in 0..h.n(2, 6) {
                let wx = x0 + h.entre(15.0, bw - 40.0);
                let wy = h.entre(top + 20.0, horizon - 30.0);
                decor_traits.push((crayon, ligne(pos2(wx, wy), pos2(wx + 25.0, wy), &mut h)));
            }
        }
        for _ in 0..if leger { 0 } else { h.n(4, 9) } {
            let y = h.entre(horizon, zone.bottom());
            let x0 = h.entre(zone.left(), zone.center().x);
            decor_traits.push((feutre, chemin(&[pos2(x0, y), pos2(x0 + h.entre(200.0, 700.0), y + h.entre(-20.0, 20.0)), pos2(x0 + h.entre(600.0, 1200.0), y + h.entre(-30.0, 30.0))], &mut h, 1.0)));
        }
        if !leger && h.chance(0.35) {
            let y = h.entre(zone.top() + 40.0, horizon - 100.0);
            decor_traits.push((aplat, chemin(&[pos2(zone.left() + 40.0, y), pos2(zone.center().x, y + 30.0), pos2(zone.right() - 40.0, y)], &mut h, 2.0)));
        }
        for (s, p) in &decor_traits {
            peindre(&mut r, lib, s, col, p);
        }
        traits += decor_traits.len();
        let mut fx = Vec::new();
        if h.chance(0.1) {
            let mut flou = Effect::new(EffectKind::GaussianBlur);
            flou.set_param(0, h.entre(2.0, 6.0));
            fx.push(flou);
        }
        calques.push(CalqueGen { nom: crate::decor::NOM.into(), genre: Genre::Decor, dessins: vec![Dessin::Bitmap(r)], frames: vec![Some(0); len], fx });
    }

    // Les personnages de ce plan et leurs places, case par case (ils bougent un peu d'une case à l'autre).
    let a = h.n(0, PERSONNAGES.len() - 1);
    let b = (a + h.n(1, PERSONNAGES.len() - 1)) % PERSONNAGES.len();
    let avec_b = h.chance(0.5) && !leger;
    let haut = cadre.height() * h.entre(0.45, 0.9);
    let pied_y = cadre.bottom() - cadre.height() * h.entre(0.02, 0.15);
    let place_a = pos2(cadre.left() + cadre.width() * h.entre(0.2, 0.45), pied_y);
    let place_b = pos2(cadre.left() + cadre.width() * h.entre(0.55, 0.8), pied_y);
    // Un dessin par case (ou par paire d'images si le plan est animé).
    let pas_dessin = |f: usize| if anime { f / 2 } else { case_de(f) };
    let n_dessins = if anime { len.div_ceil(2) } else { debuts.len() };
    let frames_de = || (0..len).map(|f| Some(pas_dessin(f))).collect::<Vec<_>>();

    if !leger && h.chance(0.6) {
        let s = reglage(lib, "Crayon graphite", h.entre(2.5, 5.0));
        let dessins = (0..n_dessins)
            .map(|k| {
                let mut r = Raster::new(w, hh);
                let sil = silhouette(place_a + vec2(k as f32 * 14.0, 0.0), haut, &mut h);
                for p in sil.legers.iter().chain(sil.nets.iter().take(6)) {
                    peindre(&mut r, lib, &s, [70, 70, 80], p);
                    traits += 1;
                }
                Dessin::Bitmap(r)
            })
            .collect();
        calques.push(CalqueGen { nom: "Crayonné".into(), genre: Genre::Crayonne, dessins, frames: frames_de(), fx: Vec::new() });
    }

    let couleur_a = h.chance(0.5) && !leger;
    let pinceaux = ["Encre", "Crayon graphite", "Pinceau rond", "Crayon net", "Feutre"];
    for (rang, (perso, place)) in [(a, place_a), (b, place_b)].into_iter().enumerate() {
        if rang == 1 && !avec_b {
            break;
        }
        let nom_brosse = *h.choix(&pinceaux);
        let s = reglage(lib, nom_brosse, h.entre(3.0, 9.0));
        let s_leger = BrushSettings { opacity: 0.35, ..reglage(lib, "Crayon graphite", 2.5) };
        let (_, trait_c, aplat_c) = PERSONNAGES[perso];
        let mut zones_par_dessin = Vec::new();
        let dessins: Vec<Dessin> = (0..n_dessins)
            .map(|k| {
                let decal = if anime { vec2(k as f32 * 18.0, ((k as f32) * 1.3).sin() * 20.0) } else { vec2(h.entre(-60.0, 60.0), 0.0) };
                let sil = silhouette(place + decal, haut * h.entre(0.95, 1.05), &mut h);
                let mut els: Vec<Element> = sil.legers.iter().map(|p| element(lib, &s_leger, [120, 120, 130], p)).collect();
                els.extend(sil.nets.iter().map(|p| element(lib, &s, trait_c, p)));
                if h.chance(0.25) {
                    let (c, rx, ry) = sil.zones[1];
                    let contour: Vec<[f32; 2]> = (0..16).map(|i| {
                        let t = i as f32 / 16.0 * std::f32::consts::TAU;
                        [c.x + t.cos() * rx * 1.1, c.y + t.sin() * ry]
                    }).collect();
                    els.push(Element::Fill { contours: vec![contour], color: [40, 40, 60], opacity: 0.25, gradient: None });
                }
                traits += els.len();
                zones_par_dessin.push(sil.zones.clone());
                Dessin::Vecteur(els)
            })
            .collect();
        let rang_trait = calques.len();
        calques.push(CalqueGen { nom: PERSONNAGES[perso].0.into(), genre: Genre::Perso(perso), dessins, frames: frames_de(), fx: Vec::new() });
        if rang == 0 && couleur_a {
            // Le calque de couleur : un aplat dans les zones du personnage, un dessin par dessin du trait.
            let s_aplat = BrushSettings { hardness: 1.0, opacity: 1.0, pressure_size: false, ..reglage(lib, "Pinceau rond", 40.0) };
            let dessins = zones_par_dessin
                .iter()
                .map(|zones| {
                    let mut r = Raster::new(w, hh);
                    for (c, rx, ry) in zones {
                        for k in 0..3 {
                            let y = c.y - ry * 0.6 + k as f32 * ry * 0.6;
                            peindre(&mut r, lib, &BrushSettings { size: rx.min(*ry) * 0.9, ..s_aplat }, aplat_c, &chemin(&[pos2(c.x - rx * 0.6, y), pos2(c.x, y + 4.0), pos2(c.x + rx * 0.6, y)], &mut h, 0.5));
                            traits += 1;
                        }
                    }
                    Dessin::Bitmap(r)
                })
                .collect();
            // (Sous son trait.)
            calques.insert(rang_trait, CalqueGen { nom: format!("{} couleur", PERSONNAGES[perso].0), genre: Genre::Couleur(rang_trait + 1), dessins, frames: frames_de(), fx: Vec::new() });
        }
    }

    if h.chance(0.3) {
        // Accessoires et traits de vitesse, en vectoriel.
        let s = reglage(lib, "Encre", h.entre(2.0, 5.0));
        let dessins = (0..n_dessins)
            .map(|_| {
                let els: Vec<Element> = (0..h.n(4, 12))
                    .map(|_| {
                        let a = pos2(h.entre(cadre.left(), cadre.right()), h.entre(cadre.top(), cadre.bottom()));
                        let d = vec2(h.entre(-300.0, 300.0), h.entre(-60.0, 60.0));
                        element(lib, &s, [20, 20, 20], &ligne(a, a + d, &mut h))
                    })
                    .collect();
                traits += els.len();
                Dessin::Vecteur(els)
            })
            .collect();
        calques.push(CalqueGen { nom: "Accessoires".into(), genre: Genre::Accessoires, dessins, frames: frames_de(), fx: Vec::new() });
    }

    if h.chance(0.2) {
        let mut grain = Effect::new(EffectKind::Grain);
        grain.set_param(0, h.entre(10.0, 40.0));
        calques.push(CalqueGen { nom: "Grain".into(), genre: Genre::Effet(EffectKind::Grain), dessins: vec![Dessin::Pose], frames: vec![Some(0); len], fx: vec![grain] });
        if h.chance(0.25) {
            calques.push(CalqueGen { nom: "Vignettage".into(), genre: Genre::Effet(EffectKind::Vignette), dessins: vec![Dessin::Pose], frames: vec![Some(0); len], fx: vec![Effect::new(EffectKind::Vignette)] });
        }
    }

    let texte = |h: &mut Hasard, v: &[&str]| -> String { if h.chance(0.85) { h.choix(v).to_string() } else { String::new() } };
    let action = texte(&mut h, &MOTS_ACTION);
    let dialogue = texte(&mut h, &MOTS_DIALOGUE);
    let notes = texte(&mut h, &MOTS_NOTES);
    let textes_marques = debuts[1..].iter().map(|_| (texte(&mut h, &MOTS_ACTION), texte(&mut h, &MOTS_DIALOGUE), texte(&mut h, &MOTS_NOTES))).collect();
    PlanGen { len, marques: debuts[1..].to_vec(), calques, camera, action, dialogue, notes, textes_marques, anime, traits }
}

// ------------------------------------------------------------------ le film

/// Un son WAV mono 16 bits à 48 kHz de `secondes` : quelques notes et un souffle (une musique témoin).
fn wav(secondes: f32, seed: u64) -> Vec<u8> {
    let n = (secondes * 48_000.0) as usize;
    let data_len = (n * 2) as u32;
    let mut b = Vec::with_capacity(44 + n * 2);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&48_000u32.to_le_bytes());
    b.extend_from_slice(&96_000u32.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    let mut h = Hasard::new(seed);
    let notes = [220.0f32, 261.6, 293.7, 329.6, 392.0, 440.0];
    let mut f = notes[0];
    for i in 0..n {
        if i % 12_000 == 0 {
            f = *h.choix(&notes);
        }
        let t = i as f32 / 48_000.0;
        let v = (t * f * std::f32::consts::TAU).sin() * 0.25 + (h.u() - 0.5) * 0.04;
        b.extend_from_slice(&((v * 32_000.0) as i16).to_le_bytes());
    }
    b
}

pub(crate) struct Stats {
    pub plans: usize,
    pub sequences: usize,
    pub cases: usize,
    pub calques: usize,
    pub dessins_bitmap: usize,
    pub dessins_vecteur: usize,
    pub traits: usize,
    pub tuiles: usize,
    pub images: usize,
    pub cameras: usize,
    pub animes: usize,
}

/// Le film lourd : `plans` plans en 8 séquences (au moins), avec tout ce qu'un storyboard d'Anthony porte.
pub(crate) fn film_lourd(plans: usize, seed: u64) -> (Document, Stats) {
    let t = Instant::now();
    let mut doc = Document::with_frame(1920, 1080, 24);
    let feuille = (doc.width, doc.height);
    let cadre = doc.frame_rect();
    let cadre = Rect::from_min_max(pos2(cadre.min.x, cadre.min.y), pos2(cadre.max.x, cadre.max.y));
    // Séquences de longueurs inégales.
    let n_seq = (plans / 37).max(8).min(plans);
    let mut h = Hasard::new(seed);
    let mut coupures: Vec<usize> = (1..n_seq).map(|k| k * plans / n_seq + h.n(0, 6)).collect();
    coupures.sort_unstable();
    coupures.dedup();
    let mut at = 0;
    for i in 1..plans {
        at = if coupures.contains(&i) { doc.add_sequence_after(at) } else { doc.add_scene_after(at) };
    }
    assert_eq!(doc.panels.len(), plans);

    // Les plans, sur tous les cœurs.
    let fils = std::thread::available_parallelism().map_or(8, |n| n.get());
    let suivant = AtomicUsize::new(0);
    let faits: Mutex<BTreeMap<usize, PlanGen>> = Mutex::new(BTreeMap::new());
    std::thread::scope(|sc| {
        for _ in 0..fils {
            sc.spawn(|| {
                let lib = BrushLibrary::builtin();
                loop {
                    let i = suivant.fetch_add(1, Ordering::Relaxed);
                    if i >= plans {
                        break;
                    }
                    let p = fabriquer_plan(seed * 100_003 + i as u64, &lib, feuille, cadre);
                    faits.lock().unwrap().insert(i, p);
                }
            });
        }
    });
    println!("LOURD dessins fabriqués en {:.1} s", t.elapsed().as_secs_f64());
    let faits = faits.into_inner().unwrap();

    let mut st = Stats { plans, sequences: doc.sequences.len(), cases: 0, calques: 0, dessins_bitmap: 0, dessins_vecteur: 0, traits: 0, tuiles: 0, images: 0, cameras: 0, animes: 0 };
    for (i, p) in faits {
        let mut calques: Vec<Layer> = Vec::new();
        let mut ids_par_calque: Vec<Vec<u64>> = Vec::new();
        for c in p.calques {
            let mut l = match c.genre {
                Genre::Decor => Layer::new_decor(),
                Genre::Effet(k) => Layer::new_effect(c.nom.clone(), crate::calqueffet::CalqueEffet::ajustement(if k == EffectKind::Grain { "Grain" } else { "Vignettage" })),
                Genre::Couleur(_) => Layer::new(c.nom.clone(), LayerKind::Bitmap),
                Genre::Perso(_) | Genre::Accessoires => Layer::new(c.nom.clone(), LayerKind::Vector),
                Genre::Crayonne => Layer::new(c.nom.clone(), LayerKind::Bitmap),
            };
            l.name = c.nom.clone();
            l.fx = c.fx;
            if matches!(c.genre, Genre::Crayonne) {
                l.opacity = 0.6;
            }
            if p.anime && matches!(c.genre, Genre::Perso(_)) {
                l.onion = true;
            }
            let ids: Vec<u64> = c
                .dessins
                .into_iter()
                .map(|d| match d {
                    Dessin::Bitmap(r) => {
                        st.dessins_bitmap += 1;
                        st.tuiles += r.tiles.iter().filter(|t| t.is_some()).count();
                        doc.add_cel(Cel::bitmap(r))
                    }
                    Dessin::Vecteur(els) => {
                        st.dessins_vecteur += 1;
                        let id = doc.new_cel(LayerKind::Vector);
                        let cel = doc.cels.get_mut(&id).unwrap();
                        cel.vector = Some(VectorDrawing { elements: els });
                        cel.version = 0;
                        id
                    }
                    Dessin::Pose => doc.new_cel(LayerKind::Bitmap),
                })
                .collect();
            l.frames = c.frames.iter().map(|f| f.map(|k| ids[k])).collect();
            calques.push(l);
            ids_par_calque.push(ids);
        }
        // Les calques de couleur : liés à leur trait, dessin pour dessin.
        for k in 0..calques.len() {
            if calques[k].name.ends_with(" couleur") {
                let trait_k = k + 1;
                let mut lien = crate::calquecouleur::CalqueCouleur::new(calques[trait_k].id);
                for (a, b) in ids_par_calque[trait_k].iter().zip(&ids_par_calque[k]) {
                    lien.dessins.insert(*a, *b);
                }
                calques[k].couleur = Some(lien);
            }
        }
        st.calques += calques.len();
        st.traits += p.traits;
        st.images += p.len;
        st.cases += 1 + p.marques.len();
        st.cameras += usize::from(!p.camera.is_empty());
        st.animes += usize::from(p.anime);
        let pan = &mut doc.panels[i];
        pan.layers = calques;
        pan.length = p.len;
        pan.camera = p.camera;
        pan.action = p.action;
        pan.dialogue = p.dialogue;
        pan.notes = p.notes;
        for (f, (a, d, n)) in p.marques.iter().zip(p.textes_marques) {
            doc.toggle_mark(i, *f);
            if let Some(m) = doc.panels[i].marks.iter_mut().find(|m| m.frame == *f) {
                (m.action, m.dialogue, m.notes) = (a, d, n);
            }
        }
    }

    // Pastilles (personnages).
    for (nom, trait_c, aplat) in PERSONNAGES {
        let id = doc.alloc();
        doc.personnages.push(crate::personnages::Personnage { id, name: nom.into(), line: trait_c, fill: Some(aplat) });
    }
    // Notes de la planche : textes et traits attachés à des cases.
    for i in (0..plans).step_by(11) {
        let id = doc.panels[i].id;
        doc.board.texts.push(crate::board::NoteText { pos: pos2(0.05, 0.62), text: "À revoir : la pose".into(), size: 0.07, color: [200, 40, 40], anchor: Some(id) });
    }
    for i in (3..plans).step_by(17) {
        let id = doc.panels[i].id;
        doc.board.strokes.push(crate::board::NoteStroke { points: (0..20).map(|k| VPoint { x: 0.2 + k as f32 * 0.03, y: 0.3 + (k as f32 * 0.5).sin() * 0.05, r: 0.006 }).collect(), color: [220, 40, 40], anchor: Some(id) });
    }
    // Commentaires.
    for k in 0..(plans / 5) {
        let i = (k * 7 + 3) % plans;
        let id = doc.add_comment(doc.panels[i].id, 0, if k % 3 == 0 { "Claire" } else { "Anthony" }, 1_760_000_000 + k as u64);
        doc.comment_mut(id).unwrap().text = format!("Note {k} : raccourcir de quelques images ?");
    }
    // Versions : quelques plans ont deux ou trois versions.
    for k in 0..(plans / 20).max(1) {
        let i = (k * 19 + 5) % plans;
        doc.nouvelle_version(i, 1_760_000_000);
        if k % 3 == 0 {
            doc.nouvelle_version(i, 1_760_000_100);
        }
    }
    // Une piste de son : trois musiques témoins d'une minute, bout à bout sur tout le film.
    let total = doc.total_frames();
    let mut sons = Vec::new();
    for k in 0..3 {
        let o = wav(60.0, seed + k);
        let pcm = crate::audio::decode(&o, "wav").expect("le WAV fabriqué se décode");
        sons.push(doc.add_sound(format!("musique-{}.wav", k + 1), "wav".into(), Arc::new(o), Arc::new(pcm)));
    }
    if doc.audio_tracks.is_empty() {
        doc.add_audio_track();
    }
    doc.audio_tracks[0].name = "Musique témoin".into();
    let mut debut = 0;
    let mut k = 0;
    while debut < total {
        let n = (60 * 24 - 12).min(total - debut);
        doc.add_clip(0, sons[k % 3], debut, 0, n);
        debut += n;
        k += 1;
    }
    doc.properties.title = format!("Film lourd ({plans} plans)");
    doc.properties.author = "Essai de charge".into();
    st.sequences = doc.sequences.len();
    println!("LOURD document assemblé en {:.1} s", t.elapsed().as_secs_f64());
    (doc, st)
}

fn taille_dossier(d: &Path) -> (u64, usize) {
    let mut total = 0;
    let mut n = 0;
    for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            let (t, k) = taille_dossier(&p);
            total += t;
            n += k;
        } else if let Ok(m) = e.metadata() {
            total += m.len();
            n += 1;
        }
    }
    (total, n)
}

pub(crate) fn dossier_lourd() -> PathBuf {
    std::env::var_os("LOURD_DIR").map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("shot-lourd"))
}

/// Fabrique et enregistre le projet lourd (`LOURD_PLANS` plans, 300 d'office) dans `LOURD_DIR\<LOURD_NOM>\<LOURD_NOM>.pelure`.
#[test]
#[ignore]
fn lourd_generer() {
    let plans: usize = std::env::var("LOURD_PLANS").ok().and_then(|v| v.parse().ok()).unwrap_or(300);
    let nom = std::env::var("LOURD_NOM").unwrap_or_else(|_| format!("Film-{plans}"));
    let lib = BrushLibrary::builtin();
    lib.share();
    let t = Instant::now();
    let (doc, st) = film_lourd(plans, 7);
    println!(
        "LOURD {nom} : {} plans, {} séquences, {} cases, {} calques, {} dessins bitmap ({} tuiles = {:.0} Mo de pixels), {} dessins vectoriels, {} traits, {} images ({:.1} min), {} plans à caméra, {} plans animés, {} dessins en tout, {} versions rangées",
        st.plans, st.sequences, st.cases, st.calques, st.dessins_bitmap, st.tuiles, st.tuiles as f64 * 16384.0 / 1e6, st.dessins_vecteur, st.traits, st.images, st.images as f64 / 24.0 / 60.0, st.cameras, st.animes, doc.cels.len(),
        doc.versions.values().map(|v| v.autres.len()).sum::<usize>()
    );
    let dir = dossier_lourd().join(&nom);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let manifeste = dir.join(format!("{nom}.pelure"));
    let t2 = Instant::now();
    let r = crate::project::save(&doc, &manifeste, &mut Default::default()).expect("enregistrement");
    let (octets, fichiers) = taille_dossier(&dir);
    println!("LOURD enregistré en {:.1} s ({} dessins écrits) : {} — {:.0} Mo, {} fichiers ; fabrication totale {:.1} s", t2.elapsed().as_secs_f64(), r.written, manifeste.display(), octets as f64 / 1e6, fichiers, t.elapsed().as_secs_f64());
    // (Sans lâcher le film : le tas de Windows met des minutes à rendre ses tuiles, `lourd_tas`.)
    std::process::exit(0);
}

/// Combien coûte de lâcher (drop) un film lourd : c'est ce que fait l'interface quand elle ouvre un autre projet
/// (`App::load_doc` remplace `self.doc`) ou ferme un onglet.
#[test]
#[ignore]
fn lourd_lacher() {
    let plans: usize = std::env::var("LOURD_PLANS").ok().and_then(|v| v.parse().ok()).unwrap_or(60);
    let (doc, st) = film_lourd(plans, 7);
    println!("LOURD {} dessins, {} tuiles", doc.cels.len(), st.tuiles);
    let t = Instant::now();
    let copie = doc.cels.values().take(10).cloned().collect::<Vec<_>>();
    println!("LOURD clone 10 dessins {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    drop(copie);
    let mut doc = doc;
    let (bitmaps, vecteurs): (Vec<u64>, Vec<u64>) = doc.cels.iter().map(|(id, c)| (*id, c.vector.is_some())).partition::<Vec<_>, _>(|x| !x.1).map2();
    let t = Instant::now();
    let v: Vec<Cel> = vecteurs.iter().filter_map(|id| doc.cels.remove(id)).collect();
    drop(v);
    println!("LOURD lâcher {} dessins vectoriels : {:.2} s", vecteurs.len(), t.elapsed().as_secs_f64());
    let t = Instant::now();
    let v: Vec<Cel> = bitmaps.iter().filter_map(|id| doc.cels.remove(id)).collect();
    drop(v);
    println!("LOURD lâcher {} dessins bitmap : {:.2} s", bitmaps.len(), t.elapsed().as_secs_f64());
    let t = Instant::now();
    drop(doc);
    println!("LOURD lâcher le reste : {:.2} s", t.elapsed().as_secs_f64());
}

trait Map2 {
    fn map2(self) -> (Vec<u64>, Vec<u64>);
}
impl Map2 for (Vec<(u64, bool)>, Vec<(u64, bool)>) {
    fn map2(self) -> (Vec<u64>, Vec<u64>) {
        (self.0.into_iter().map(|x| x.0).collect(), self.1.into_iter().map(|x| x.0).collect())
    }
}

// ------------------------------------------------------------------ mesures

#[repr(C)]
#[derive(Default)]
struct Pmc {
    cb: u32,
    page_faults: u32,
    peak_ws: usize,
    ws: usize,
    qpp: usize,
    qp: usize,
    qnpp: usize,
    qnp: usize,
    pagefile: usize,
    peak_pagefile: usize,
    private: usize,
}
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct FileTime(u32, u32);
#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> isize;
    fn GetProcessTimes(h: isize, c: *mut FileTime, e: *mut FileTime, k: *mut FileTime, u: *mut FileTime) -> i32;
}
// (Le nom de psapi : `exportrender_tests.rs` déclare `K32GetProcessMemoryInfo` de kernel32 avec une structure plus courte.)
#[link(name = "psapi")]
extern "system" {
    fn GetProcessMemoryInfo(h: isize, p: *mut Pmc, cb: u32) -> i32;
}

/// Mémoire du processus : (ensemble de travail, privée), en Mo.
pub(crate) fn memoire() -> (f64, f64) {
    let mut p = Pmc { cb: std::mem::size_of::<Pmc>() as u32, ..Default::default() };
    unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut p, p.cb) };
    (p.ws as f64 / 1e6, p.private as f64 / 1e6)
}

/// Temps de processeur du processus (noyau + utilisateur), en secondes.
pub(crate) fn cpu() -> f64 {
    let (mut c, mut e, mut k, mut u) = (FileTime::default(), FileTime::default(), FileTime::default(), FileTime::default());
    unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) };
    let s = |f: FileTime| ((f.1 as u64) << 32 | f.0 as u64) as f64 / 1e7;
    s(k) + s(u)
}

fn mem(titre: &str, b: &crate::perf_tests::Banc) {
    let (ws, pv) = memoire();
    let g = b.app.gpu.as_ref().map(|g| g.stats()).unwrap_or_default();
    println!("MEM {titre:<44} RSS {ws:>7.0} Mo, privée {pv:>7.0} Mo | carte : {} textures de dessin, {:.0} Mo", g.textures, g.bytes as f64 / 1e6);
}

use crate::perf_tests::{Banc, Mesure};
use eframe::egui::{self, Event, Modifiers, PointerButton};

fn tri(mut v: Vec<f64>) -> (f64, f64, f64) {
    if v.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    v.sort_by(f64::total_cmp);
    (v.iter().sum::<f64>() / v.len() as f64, v[v.len() * 95 / 100], *v.last().unwrap())
}

fn rapport(titre: &str, m: &[Mesure]) {
    let (moy, p95, max) = tri(m.iter().map(Mesure::total).collect());
    let (c, _, cmax) = tri(m.iter().map(|x| x.construction).collect());
    let (att, _, attmax) = tri(m.iter().map(|x| x.detail[3]).collect());
    let lent = |s: f64| m.iter().filter(|x| x.total() > s).count();
    println!(
        "PERF {titre:<52} total moy {moy:>7.2} p95 {p95:>7.2} max {max:>8.2} | interface moy {c:>6.2} max {cmax:>8.2} | attente carte moy {att:>5.2} max {attmax:>6.2} | >16,7 ms : {} >33 : {} >100 : {} ({} images)",
        lent(16.7),
        lent(33.3),
        lent(100.0),
        m.len()
    );
}

fn scene<F: FnMut(&mut Banc, usize) -> Vec<Event>>(titre: &str, b: &mut Banc, n: usize, dt: f64, mut f: F) -> Vec<Mesure> {
    crate::perf::activer(true);
    let m: Vec<Mesure> = (0..n)
        .map(|k| {
            let ev = f(b, k);
            b.image(ev, dt)
        })
        .collect();
    rapport(titre, &m);
    let r = crate::perf::resume();
    let tete: Vec<String> = r.sections.iter().take(7).map(|(nom, moy, max)| format!("{nom} {moy:.2}/{max:.2}")).collect();
    println!("     sections (moy/max ms) : {}", tete.join(" | "));
    crate::perf::activer(false);
    m
}

fn bouge(k: usize, r: Rect) -> Vec<Event> {
    (0..4).map(|j| Event::PointerMoved(pos2(r.left() + r.width() * (0.2 + 0.6 * (((k * 4 + j) % 97) as f32 / 97.0)), r.center().y))).collect()
}

fn appui(p: Pos2, down: bool, m: Modifiers) -> Event {
    Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: down, modifiers: m }
}

/// Un glissé de `de` à `a` en `n` images (avec `m` tenus), mesuré image par image.
fn glisse(b: &mut Banc, titre: &str, de: Pos2, a: Pos2, n: usize, m: Modifiers) -> Vec<Mesure> {
    b.touches = m;
    b.image(vec![Event::PointerMoved(de)], 1.0 / 60.0);
    let r = scene(titre, b, n + 2, 1.0 / 60.0, |_, k| {
        if k == 0 {
            vec![appui(de, true, m)]
        } else if k <= n {
            let t = k as f32 / n as f32;
            vec![Event::PointerMoved(de + (a - de) * t + vec2(0.0, (t * 9.0).sin() * 8.0))]
        } else {
            vec![appui(a, false, m)]
        }
    });
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), 1.0 / 60.0);
    r
}

/// Images jusqu'à ce que plus rien ne travaille en arrière-plan (vignettes, rejeux) et que l'écran ne demande plus rien ;
/// rend (images, ms, pire image ms).
fn jusqu_au_calme(b: &mut Banc, max_s: f64) -> (usize, f64, f64) {
    let t = Instant::now();
    let mut calmes = 0;
    let mut pire = 0.0f64;
    let mut n = 0;
    while t.elapsed().as_secs_f64() < max_s {
        let m = b.image(Vec::new(), 1.0 / 60.0);
        n += 1;
        pire = pire.max(m.total());
        let occupe = b.app.vignettes_en_cours() || b.app.rejeu.has_pending() || !b.app.rejeu_idle.is_empty();
        if !occupe && m.reveil >= std::time::Duration::from_millis(400) {
            calmes += 1;
            if calmes >= 3 {
                break;
            }
        } else {
            calmes = 0;
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    (n, t.elapsed().as_secs_f64() * 1e3, pire)
}

pub(crate) fn projet_mesure() -> PathBuf {
    std::env::var_os("LOURD_PROJET").map(PathBuf::from).expect("LOURD_PROJET : le manifeste à mesurer")
}

/// Ouvre le projet dans un banc, dans l'espace `espace` : temps de lecture, image d'adoption, vignettes et rejeux.
pub(crate) fn ouvrir_mesure(espace: crate::workspace::Espace) -> Banc {
    let p = projet_mesure();
    // `LOURD_FILS_REJEU` : autant de fils pour les rejeux du chargement (1 : comme avant les gros projets).
    if let Some(n) = std::env::var("LOURD_FILS_REJEU").ok().and_then(|v| v.parse().ok()) {
        crate::rejeu::FILS_DE_CHARGEMENT_ESSAI.store(n, Ordering::Relaxed);
        println!("     fils des rejeux du chargement : {n}");
    }
    let mut b = crate::perf_tests::banc(Document::with_frame(1920, 1080, 24), true);
    mem("banc vide", &b);
    b.app.set_espace(espace);
    b.chauffer(5);
    let t = Instant::now();
    b.app.open_path(p.clone());
    let mut pendant = Vec::new();
    while b.app.open_job.is_some() {
        // (L'image où le projet lu est adopté : ses sections sont relevées.)
        let adopte = b.app.open_job.as_ref().is_some_and(|j| j.finished());
        if adopte {
            crate::perf::activer(true);
        }
        pendant.push(b.image(Vec::new(), 1.0 / 60.0));
        if adopte {
            let r = crate::perf::resume();
            let tete: Vec<String> = r.sections.iter().take(6).map(|(nom, moy, max)| format!("{nom} {moy:.1}/{max:.1}")).collect();
            println!("     image d'adoption, sections (ms) : {}", tete.join(" | "));
            crate::perf::activer(false);
        }
        std::thread::sleep(std::time::Duration::from_millis(4));
    }
    let lu = t.elapsed().as_secs_f64();
    let adoption = pendant.last().map_or(0.0, |m| m.construction);
    rapport("ouverture : images pendant la lecture", &pendant[..pendant.len().saturating_sub(1)]);
    // Les images qui suivent l'adoption : la première construit l'interface du projet entier (disposition de la planche…).
    crate::perf::activer(true);
    let suite: Vec<f64> = (0..3).map(|_| b.image(Vec::new(), 1.0 / 60.0).construction).collect();
    let r = crate::perf::resume();
    let tete: Vec<String> = r.sections.iter().take(6).map(|(nom, moy, max)| format!("{nom} {moy:.1}/{max:.1}")).collect();
    crate::perf::activer(false);
    println!("OUVRIR lecture du projet (fil de fond) jusqu'à l'adoption : {lu:.2} s ; image d'adoption (fil de l'interface) : {adoption:.0} ms ; les trois suivantes : {:.0} / {:.0} / {:.0} ms ; {} plans, {} dessins", suite[0], suite[1], suite[2], b.app.doc.panels.len(), b.app.doc.cels.len());
    println!("     images après l'adoption, sections (moy/max ms) : {}", tete.join(" | "));
    mem("projet adopté", &b);
    let t2 = Instant::now();
    let cpu2 = cpu();
    let mut rejeux_finis = None;
    let mut premiere = None;
    let mut images = Vec::new();
    loop {
        let m = b.image(Vec::new(), 1.0 / 60.0);
        images.push(m);
        if premiere.is_none() && !b.app.vignettes_en_cours() && images.len() > 3 {
            premiere = Some(t2.elapsed().as_secs_f64());
        }
        if rejeux_finis.is_none() && !b.app.rejeu.has_pending() && b.app.rejeu_idle.is_empty() {
            rejeux_finis = Some((t2.elapsed().as_secs_f64(), cpu() - cpu2));
        }
        let occupe = b.app.vignettes_en_cours() || b.app.rejeu.has_pending() || !b.app.rejeu_idle.is_empty();
        if !occupe && images.len() > 5 && m.reveil >= std::time::Duration::from_millis(400) {
            break;
        }
        if t2.elapsed().as_secs_f64() > 1200.0 {
            println!("OUVRIR ! toujours occupé après 20 min");
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    rapport("ouverture : images jusqu'au calme (vignettes, rejeux)", &images);
    println!("OUVRIR vignettes faites {:.2} s après l'adoption ; calme (rejeux exacts finis) après {:.2} s ; total {:.2} s", premiere.unwrap_or(-1.0), t2.elapsed().as_secs_f64(), t.elapsed().as_secs_f64());
    let (rf, rcpu) = rejeux_finis.unwrap_or((-1.0, 0.0));
    println!("OUVRIR rejeux exacts finis {rf:.2} s après l'adoption ({rcpu:.1} s de processeur) ; jusqu'au calme : {:.1} s de processeur", cpu() - cpu2);
    mem("après le calme", &b);
    b
}

/// Le plan le plus lourd (dessins × calques).
fn plan_lourd(doc: &Document) -> usize {
    (0..doc.panels.len()).max_by_key(|&i| doc.panels[i].layers.iter().map(|l| l.frames.iter().flatten().count()).sum::<usize>() * doc.panels[i].layers.len()).unwrap_or(0)
}

#[test]
#[ignore]
fn lourd_mesurer() {
    use crate::app::Tool;
    use crate::workspace::Espace;
    let cpu0 = cpu();
    let mut b = ouvrir_mesure(Espace::Vignettes);
    let n = b.app.doc.panels.len();
    println!("--- Vignettes");
    let vue = b.app.vignettes.view;
    scene("vignettes : souris qui bouge", &mut b, 90, 1.0 / 60.0, |_, k| bouge(k, vue));
    scene("vignettes : molette (défilement)", &mut b, 150, 1.0 / 60.0, |_, k| {
        vec![Event::PointerMoved(vue.center()), Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, if k < 120 { -90.0 } else { 300.0 }), modifiers: Modifiers::NONE }]
    });
    let z0 = b.app.vignettes.xf.zoom;
    scene("vignettes : Ctrl + molette (zoom avant puis arrière)", &mut b, 120, 1.0 / 60.0, |b, k| {
        b.touches = Modifiers::CTRL;
        vec![Event::PointerMoved(vue.center()), Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, if k < 60 { 60.0 } else { -60.0 }), modifiers: Modifiers::CTRL }]
    });
    b.touches = Modifiers::NONE;
    println!("     zoom de la planche : {z0:.3} -> {:.3}", b.app.vignettes.xf.zoom);
    let (i, ms, pire) = jusqu_au_calme(&mut b, 120.0);
    println!("PERF vignettes après le zoom : {i} images, {ms:.0} ms jusqu'au calme, pire image {pire:.1} ms");
    // Dessiner dans une case.
    b.app.tool = Tool::Brush;
    let layout = b.app.board_layout();
    let k = layout.cards.iter().position(|c| vue.contains_rect(b.app.vignettes.xf.rect(c.thumb).shrink(2.0))).unwrap_or(0);
    let case = b.app.vignettes.xf.rect(layout.cards[k].thumb);
    println!("     case {k} à l'écran : {case:?}");
    glisse(&mut b, "vignettes : trait dans une case (40 images)", case.left_center() + vec2(case.width() * 0.1, 0.0), case.right_center() - vec2(case.width() * 0.1, 0.0), 40, Modifiers::NONE);
    let (i, ms, pire) = jusqu_au_calme(&mut b, 60.0);
    println!("PERF après le trait : {i} images, {ms:.0} ms jusqu'au calme, pire image {pire:.1} ms");
    // Mouvement de caméra (Ctrl + Maj + glisser la case).
    let cam_mod = Modifiers { ctrl: true, shift: true, command: true, ..Modifiers::NONE };
    let layout = b.app.board_layout();
    let case = b.app.vignettes.xf.rect(layout.cards[k].thumb);
    let avant = b.app.doc.panels[layout.cards[k].panel].camera.len();
    glisse(&mut b, "vignettes : glisser la caméra d'une case", case.center(), case.center() + vec2(case.width() * 0.6, 0.0), 30, cam_mod);
    println!("     clés de caméra du plan : {avant} -> {}", b.app.doc.panels.get(layout.cards[k].panel).map_or(0, |p| p.camera.len()));
    let (i, ms, pire) = jusqu_au_calme(&mut b, 60.0);
    println!("PERF après la caméra : {i} images, {ms:.0} ms jusqu'au calme, pire image {pire:.1} ms");
    // Une coupe basculée.
    let layout = b.app.board_layout();
    let t = Instant::now();
    let ok = b.app.board_toggle_cut(&layout, (k + 1).min(layout.cards.len() - 2));
    println!("PERF basculer une coupe (fil de l'interface) : {:.1} ms (changé : {ok})", t.elapsed().as_secs_f64() * 1e3);
    scene("vignettes : images après la coupe", &mut b, 10, 1.0 / 60.0, |_, _| Vec::new());
    let t = Instant::now();
    b.app.undo();
    println!("PERF annuler la coupe : {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    // Passer d'un plan à l'autre.
    scene("vignettes : changer de plan à chaque image", &mut b, 90, 1.0 / 60.0, |b, k| {
        b.app.select_panel((k * 37) % n);
        Vec::new()
    });
    mem("après les Vignettes", &b);

    println!("--- Storyboard (dessin, montage dessous)");
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(5);
    let lourd = plan_lourd(&b.app.doc);
    b.app.select_panel(lourd);
    b.app.cur_frame = 0;
    b.chauffer(5);
    println!("     plan lourd : {} ({} calques, {} dessins)", lourd, b.app.doc.panels[lourd].layers.len(), b.app.doc.panels[lourd].layers.iter().map(|l| l.frames.iter().flatten().collect::<std::collections::BTreeSet<_>>().len()).sum::<usize>());
    jusqu_au_calme(&mut b, 30.0);
    let r = b.app.canvas_rect;
    let zone = r.shrink2(vec2(r.width() * 0.2, r.height() * 0.3));
    scene("storyboard : souris qui bouge sur le dessin", &mut b, 90, 1.0 / 60.0, |_, k| bouge(k, zone));
    b.app.tool = Tool::Brush;
    b.app.cur_layer = b.app.doc.panels[lourd].layers.len() - 1;
    glisse(&mut b, "storyboard : trait sur le plan lourd (60 images)", zone.left_center(), zone.right_center(), 60, Modifiers::NONE);
    jusqu_au_calme(&mut b, 30.0);
    let montage = Rect::from_min_size(pos2(500.0, 880.0), vec2(900.0, 120.0));
    scene("storyboard : molette horizontale sur le montage", &mut b, 90, 1.0 / 60.0, |_, k| {
        vec![Event::PointerMoved(pos2(900.0, 940.0)), Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(-60.0 - (k % 5) as f32 * 10.0, 0.0), modifiers: Modifiers::NONE }]
    });
    let total = b.app.doc.total_frames();
    scene("storyboard : tête de lecture qui saute", &mut b, 120, 1.0 / 60.0, |b, k| {
        b.app.goto_global((k * 977) % total);
        bouge(k, montage)
    });
    scene("storyboard : changer de plan à chaque image", &mut b, 90, 1.0 / 60.0, |b, k| {
        b.app.select_panel((k * 37) % n);
        Vec::new()
    });
    mem("après le Storyboard", &b);

    println!("--- Montage");
    b.app.set_espace(Espace::Montage);
    b.chauffer(5);
    jusqu_au_calme(&mut b, 60.0);
    b.app.goto_global(0);
    b.app.toggle_play_scope(true);
    let m = scene("montage : lecture du film (24 images/s, 10 s)", &mut b, 240, 1.0 / 24.0, |_, _| Vec::new());
    println!("     lecture : {} images sur {} dépassent 41,7 ms ; tête à {}", m.iter().filter(|x| x.total() > 41.7).count(), m.len(), b.app.cur_frame);
    b.app.playing = false;
    mem("après la lecture", &b);

    println!("--- Animation (pelure)");
    let anime = (0..n).find(|&i| b.app.doc.panels[i].layers.iter().any(|l| l.onion)).unwrap_or(lourd);
    b.app.set_espace(Espace::Animation);
    b.app.select_panel(anime);
    b.app.onion.enabled = true;
    b.app.onion.before = 3;
    b.app.onion.after = 3;
    b.app.cur_layer = b.app.doc.panels[anime].layers.iter().position(|l| l.onion).unwrap_or(0);
    b.chauffer(5);
    jusqu_au_calme(&mut b, 30.0);
    let len = b.app.doc.panels[anime].frame_count();
    scene("animation : tête de lecture image par image (pelure 3/3)", &mut b, 90, 1.0 / 60.0, |b, k| {
        b.app.cur_frame = k % len;
        Vec::new()
    });
    b.app.cur_frame = 0;
    b.app.toggle_play_scope(false);
    scene("animation : lecture du plan (pelure allumée)", &mut b, 96, 1.0 / 24.0, |_, _| Vec::new());
    b.app.playing = false;
    b.app.cur_frame = 4;
    let r = b.app.canvas_rect;
    let zone = r.shrink2(vec2(r.width() * 0.2, r.height() * 0.3));
    glisse(&mut b, "animation : trait avec la pelure (60 images)", zone.left_center(), zone.right_center(), 60, Modifiers::NONE);
    jusqu_au_calme(&mut b, 30.0);

    println!("--- Grosse opération et annulation");
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(3);
    let ids: Vec<u64> = b.app.doc.panels.iter().skip(10).take(80).map(|p| p.id).collect();
    let t = Instant::now();
    b.app.structure_op(|d, _| d.remove_panels(&ids));
    println!("PERF supprimer 80 plans (fil de l'interface) : {:.1} ms ; {} plans restent", t.elapsed().as_secs_f64() * 1e3, b.app.doc.panels.len());
    scene("images après la suppression", &mut b, 20, 1.0 / 60.0, |_, _| Vec::new());
    let t = Instant::now();
    b.app.undo();
    println!("PERF annuler la suppression : {:.1} ms ; {} plans", t.elapsed().as_secs_f64() * 1e3, b.app.doc.panels.len());
    scene("images après l'annulation", &mut b, 20, 1.0 / 60.0, |_, _| Vec::new());
    let t = Instant::now();
    b.app.redo();
    println!("PERF rétablir : {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    let t = Instant::now();
    b.app.undo();
    println!("PERF annuler encore : {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    let (i, ms, pire) = jusqu_au_calme(&mut b, 120.0);
    println!("PERF après l'annulation : {i} images, {ms:.0} ms jusqu'au calme, pire image {pire:.1} ms");
    mem("après la grosse opération", &b);

    println!("--- Enregistrer");
    let enregistrer = |b: &mut Banc, titre: &str| {
        let t = Instant::now();
        b.app.save();
        let appel = t.elapsed().as_secs_f64() * 1e3;
        let mut pire = 0.0f64;
        let mut images = 0;
        while b.app.save_en_cours() {
            pire = pire.max(b.image(Vec::new(), 1.0 / 60.0).total());
            images += 1;
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        println!("PERF {titre} : appel (fil de l'interface) {appel:.1} ms, fin {:.0} ms, {images} images pendant, la plus lente {pire:.1} ms, modifié après : {}", t.elapsed().as_secs_f64() * 1e3, b.app.modified);
    };
    enregistrer(&mut b, "Ctrl+S après les gestes");
    enregistrer(&mut b, "Ctrl+S, rien de modifié");
    // Tout réécrire ailleurs (comme un premier enregistrement).
    let ailleurs = dossier_lourd().join("copie-enregistrement");
    let _ = std::fs::remove_dir_all(&ailleurs);
    std::fs::create_dir_all(&ailleurs).unwrap();
    let chemin = b.app.path.clone();
    b.app.path = Some(ailleurs.join("Copie.pelure"));
    b.app.written.clear();
    b.app.modified = true;
    enregistrer(&mut b, "premier enregistrement (tout écrire)");
    b.app.path = chemin;
    let _ = std::fs::remove_dir_all(&ailleurs);

    println!("--- Repos");
    let (i, ms, _) = jusqu_au_calme(&mut b, 120.0);
    println!("REPOS calme après {i} images, {ms:.0} ms");
    let m: Vec<Mesure> = (0..10).map(|_| b.image(Vec::new(), 0.5)).collect();
    println!("REPOS prochaine image demandée dans : {:?}", m.iter().map(|x| x.reveil).min());
    let c0 = cpu();
    std::thread::sleep(std::time::Duration::from_secs(10));
    println!("REPOS processeur pendant 10 s sans image : {:.2} s de calcul (fils de fond)", cpu() - c0);
    mem("au repos", &b);
    println!("CPU total du banc : {:.0} s", cpu() - cpu0);

    println!("--- Lâcher le projet (ouvrir un autre projet / fermer l'onglet)");
    let t = Instant::now();
    b.app.load_doc(Document::with_frame(1920, 1080, 24));
    println!("PERF load_doc d'un projet vide à la place du film (fil de l'interface) : {:.2} s", t.elapsed().as_secs_f64());
    let t = Instant::now();
    b.app.history.clear();
    b.chauffer(3);
    println!("PERF images qui suivent : {:.2} s", t.elapsed().as_secs_f64());
    mem("après avoir lâché le film", &b);
}

/// Les Vignettes au zoom d'origine : dessiner dans trois cases, glisser une caméra, changer de plan.
#[test]
#[ignore]
fn lourd_vignettes() {
    use crate::app::Tool;
    use crate::workspace::Espace;
    let mut b = ouvrir_mesure(Espace::Vignettes);
    let vue = b.app.vignettes.view;
    println!("     zoom de la planche : {:.3}, vue {vue:?}", b.app.vignettes.xf.zoom);
    b.app.tool = Tool::Brush;
    for essai in 0..3 {
        let layout = b.app.board_layout();
        let visibles: Vec<usize> = (0..layout.cards.len()).filter(|&k| vue.contains_rect(b.app.vignettes.xf.rect(layout.cards[k].thumb).shrink(2.0))).collect();
        let k = visibles[(essai * 7 + 3) % visibles.len()];
        let case = b.app.vignettes.xf.rect(layout.cards[k].thumb);
        println!("     case {k} (plan {}) à l'écran : {case:?}", layout.cards[k].panel);
        glisse(&mut b, &format!("vignettes 100 % : trait dans la case {k}"), case.left_center() + vec2(case.width() * 0.1, 0.0), case.right_center() - vec2(case.width() * 0.1, 0.0), 40, Modifiers::NONE);
        let (i, ms, pire) = jusqu_au_calme(&mut b, 60.0);
        println!("PERF après le trait : {i} images, {ms:.0} ms jusqu'au calme, pire image {pire:.1} ms");
    }
    let cam_mod = Modifiers { ctrl: true, shift: true, command: true, ..Modifiers::NONE };
    for essai in 0..2 {
        let layout = b.app.board_layout();
        let visibles: Vec<usize> = (0..layout.cards.len()).filter(|&k| vue.contains_rect(b.app.vignettes.xf.rect(layout.cards[k].thumb).shrink(2.0))).collect();
        let k = visibles[(essai * 5 + 1) % visibles.len()];
        let case = b.app.vignettes.xf.rect(layout.cards[k].thumb);
        let p = layout.cards[k].panel;
        let avant = b.app.doc.panels[p].camera.len();
        glisse(&mut b, &format!("vignettes 100 % : caméra de la case {k}"), case.center(), case.center() + vec2(case.width() * 0.5, case.height() * 0.2), 30, cam_mod);
        println!("     clés de caméra du plan {p} : {avant} -> {}", b.app.doc.panels.get(p).map_or(0, |p| p.camera.len()));
        let (i, ms, pire) = jusqu_au_calme(&mut b, 60.0);
        println!("PERF après la caméra : {i} images, {ms:.0} ms jusqu'au calme, pire image {pire:.1} ms");
    }
    let n = b.app.doc.panels.len();
    scene("vignettes 100 % : changer de plan à chaque image", &mut b, 90, 1.0 / 60.0, |b, k| {
        b.app.select_panel((k * 37) % n);
        Vec::new()
    });
    scene("vignettes 100 % : molette (défilement)", &mut b, 150, 1.0 / 60.0, |_, k| {
        vec![Event::PointerMoved(vue.center()), Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, if k < 120 { -90.0 } else { 300.0 }), modifiers: Modifiers::NONE }]
    });
    mem("fin", &b);
    // (Sans lâcher le film : le lâcher prend des minutes, `lourd_tas`.)
    std::process::exit(0);
}

/// La planche dézoomée (0,2) d'un gros projet : zoom arrière, trait dans une case, caméra, changer de plan (la partie
/// « Vignettes » de `lourd_mesurer`, seule).
#[test]
#[ignore]
fn lourd_planche() {
    use crate::app::Tool;
    use crate::workspace::Espace;
    let mut b = ouvrir_mesure(Espace::Vignettes);
    let n = b.app.doc.panels.len();
    let vue = b.app.vignettes.view;
    let z0 = b.app.vignettes.xf.zoom;
    scene("vignettes : Ctrl + molette (zoom arrière)", &mut b, 60, 1.0 / 60.0, |b, _| {
        b.touches = Modifiers::CTRL;
        vec![Event::PointerMoved(vue.center()), Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, -60.0), modifiers: Modifiers::CTRL }]
    });
    b.touches = Modifiers::NONE;
    println!("     zoom de la planche : {z0:.3} -> {:.3}", b.app.vignettes.xf.zoom);
    let (i, ms, pire) = jusqu_au_calme(&mut b, 120.0);
    println!("PERF vignettes après le zoom : {i} images, {ms:.0} ms jusqu'au calme, pire image {pire:.1} ms");
    mem("planche dézoomée, au calme", &b);
    scene("vignettes dézoomées : rien ne bouge", &mut b, 30, 1.0 / 60.0, |_, _| Vec::new());
    scene("vignettes dézoomées : souris qui bouge", &mut b, 60, 1.0 / 60.0, |_, k| bouge(k, vue));
    b.app.tool = Tool::Brush;
    for essai in 0..2 {
        let layout = b.app.board_layout();
        let visibles: Vec<usize> = (0..layout.cards.len()).filter(|&k| vue.contains_rect(b.app.vignettes.xf.rect(layout.cards[k].thumb).shrink(2.0))).collect();
        let k = visibles[(essai * 11 + 54) % visibles.len()];
        let case = b.app.vignettes.xf.rect(layout.cards[k].thumb);
        println!("     case {k} à l'écran : {case:?} ({} cases visibles)", visibles.len());
        glisse(&mut b, "vignettes dézoomées : trait dans une case (40 images)", case.left_center() + vec2(case.width() * 0.1, 0.0), case.right_center() - vec2(case.width() * 0.1, 0.0), 40, Modifiers::NONE);
        let (i, ms, pire) = jusqu_au_calme(&mut b, 60.0);
        println!("PERF après le trait : {i} images, {ms:.0} ms jusqu'au calme, pire image {pire:.1} ms");
    }
    let cam_mod = Modifiers { ctrl: true, shift: true, command: true, ..Modifiers::NONE };
    let layout = b.app.board_layout();
    let visibles: Vec<usize> = (0..layout.cards.len()).filter(|&k| vue.contains_rect(b.app.vignettes.xf.rect(layout.cards[k].thumb).shrink(2.0))).collect();
    let k = visibles[54 % visibles.len()];
    let case = b.app.vignettes.xf.rect(layout.cards[k].thumb);
    glisse(&mut b, "vignettes dézoomées : glisser la caméra d'une case", case.center(), case.center() + vec2(case.width() * 0.6, 0.0), 30, cam_mod);
    let (i, ms, pire) = jusqu_au_calme(&mut b, 60.0);
    println!("PERF après la caméra : {i} images, {ms:.0} ms jusqu'au calme, pire image {pire:.1} ms");
    scene("vignettes dézoomées : changer de plan à chaque image", &mut b, 90, 1.0 / 60.0, |b, k| {
        b.app.select_panel((k * 37) % n);
        Vec::new()
    });
    mem("fin", &b);
    std::process::exit(0);
}

/// Un autre projet prend la place du gros projet (`App::load_doc` : Ouvrir, Nouveau, fermer le dernier onglet) : le temps
/// pris par le fil de l'interface, puis les images qui suivent.
/// L'espace où ouvrir le projet : `LOURD_ESPACE` (storyboard, animation, montage), les Vignettes d'office.
fn espace_mesure() -> crate::workspace::Espace {
    use crate::workspace::Espace;
    match std::env::var("LOURD_ESPACE").unwrap_or_default().to_lowercase().as_str() {
        "storyboard" => Espace::Storyboard,
        "animation" => Espace::Animation,
        "montage" => Espace::Montage,
        _ => Espace::Vignettes,
    }
}

/// Mémoire et fils de `lacher.rs` encore au travail, `n` fois à `pas` d'intervalle.
fn suivre_le_lacher(b: &crate::perf_tests::Banc, n: usize, pas: std::time::Duration) {
    let t = Instant::now();
    for _ in 0..n {
        std::thread::sleep(pas);
        let (ws, pv) = memoire();
        println!("MEM {:>5.1} s après : RSS {ws:>7.0} Mo, privée {pv:>7.0} Mo ; fils qui lâchent : {} ; rejeux en attente : {}", t.elapsed().as_secs_f64(), crate::lacher::en_cours(), b.app.rejeu.has_pending());
    }
}

#[test]
#[ignore]
fn lourd_remplacer() {
    let mut b = ouvrir_mesure(espace_mesure());
    let t = Instant::now();
    b.app.load_doc(Document::with_frame(1920, 1080, 24));
    println!("PERF load_doc d'un projet vide à la place du film (fil de l'interface) : {:.3} s", t.elapsed().as_secs_f64());
    let m: Vec<Mesure> = (0..60).map(|_| b.image(Vec::new(), 1.0 / 60.0)).collect();
    rapport("images après le remplacement", &m);
    mem("juste après le remplacement", &b);
    suivre_le_lacher(&b, 6, std::time::Duration::from_secs(1));
    std::process::exit(0);
}

/// Le gros projet est dans un onglet caché, que l'on ferme (`App::fermer_onglet`, rien à enregistrer).
#[test]
#[ignore]
fn lourd_fermer_onglet() {
    let mut b = ouvrir_mesure(espace_mesure());
    assert!(b.app.nouvel_onglet(), "un onglet neuf");
    b.chauffer(3);
    // (Le film s'est ouvert dans un onglet à lui, à côté de celui du banc : `cible_projet`.)
    let i = b.app.onglet_du_projet(&projet_mesure()).expect("l'onglet du film");
    assert_ne!(i, b.app.actif, "l'onglet du film est caché");
    // (Rien à perdre : l'onglet se ferme sans passer à l'écran ni demander s'il faut enregistrer.)
    let info = b.app.infos_onglet(i);
    println!("     onglet du film ({i} sur {}) : modifié {}, écriture {}", b.app.onglets.len(), info.modifie, info.ecriture);
    b.app.onglets[i].etat.as_mut().expect("onglet rangé").modified = false;
    let t = Instant::now();
    b.app.fermer_onglet(i);
    println!("PERF fermer l'onglet caché du film (fil de l'interface) : {:.3} s ; {} onglet(s)", t.elapsed().as_secs_f64(), b.app.onglets.len());
    let m: Vec<Mesure> = (0..60).map(|_| b.image(Vec::new(), 1.0 / 60.0)).collect();
    rapport("images après la fermeture", &m);
    mem("juste après la fermeture", &b);
    suivre_le_lacher(&b, 8, std::time::Duration::from_secs(2));
    std::process::exit(0);
}

// ------------------------------------------------------------------ chasse aux plantages

static PLANTAGES: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn installer_crochet() {
    std::panic::set_hook(Box::new(|info| {
        let bt = std::backtrace::Backtrace::force_capture().to_string();
        // Les 40 premières lignes utiles (on saute la machinerie du panic).
        let tete: Vec<&str> = bt.lines().filter(|l| !l.contains("std::panic") && !l.contains("core::panic") && !l.contains("rust_begin_unwind") && !l.contains("backtrace")).take(40).collect();
        let msg = format!("{info}\n{}", tete.join("\n"));
        eprintln!("PLANTAGE {msg}");
        PLANTAGES.lock().unwrap_or_else(|e| e.into_inner()).push(msg);
    }));
}

/// Ce qui ne doit jamais arriver : un plan courant hors du film, un plan sans calque, un dessin montré qui n'existe pas.
fn incoherences(b: &Banc) -> Vec<String> {
    let d = &b.app.doc;
    let mut v = Vec::new();
    if d.panels.is_empty() {
        v.push("film sans plan".into());
        return v;
    }
    if b.app.cur_panel >= d.panels.len() {
        v.push(format!("plan courant {} sur {}", b.app.cur_panel, d.panels.len()));
    }
    for (i, p) in d.panels.iter().enumerate() {
        if p.layers.is_empty() {
            v.push(format!("plan {i} sans calque"));
        }
        for l in &p.layers {
            for c in l.frames.iter().flatten() {
                if !d.cels.contains_key(c) {
                    v.push(format!("plan {i} calque « {} » : dessin {c} absent", l.name));
                    break;
                }
            }
        }
        for m in &p.marks {
            if m.frame >= p.frame_count() {
                v.push(format!("plan {i} : marque à l'image {} pour {} images", m.frame, p.frame_count()));
            }
        }
    }
    if b.app.cur_layer >= d.panels.get(b.app.cur_panel).map_or(1, |p| p.layers.len()) {
        v.push(format!("calque courant {} hors du plan", b.app.cur_layer));
    }
    v
}

#[test]
#[ignore]
fn lourd_chasse() {
    use crate::app::Tool;
    use crate::workspace::Espace;
    installer_crochet();
    let tours: usize = std::env::var("LOURD_TOURS").ok().and_then(|v| v.parse().ok()).unwrap_or(400);
    let seed: u64 = std::env::var("LOURD_GRAINE").ok().and_then(|v| v.parse().ok()).unwrap_or(11);
    let mut b = ouvrir_mesure(Espace::Vignettes);
    let mut h = Hasard::new(seed);
    let espaces = [Espace::Vignettes, Espace::Storyboard, Espace::Animation, Espace::Montage];
    let mut lents: Vec<(String, f64)> = Vec::new();
    let mut journal: Vec<String> = Vec::new();
    let mut incoh_vues = 0;
    let t0 = Instant::now();
    for tour in 0..tours {
        let n = b.app.doc.panels.len();
        let op = h.n(0, 17);
        let i = h.n(0, n.saturating_sub(1));
        let k = h.n(1, 20);
        let nom;
        let t = Instant::now();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> String {
            match op {
                0 => {
                    let ids: Vec<u64> = b.app.doc.panels.iter().skip(i).take(k).map(|p| p.id).collect();
                    b.app.structure_op(|d, _| d.remove_panels(&ids));
                    format!("supprimer {} plans dès {i}", ids.len())
                }
                1 => {
                    let ids: Vec<u64> = b.app.doc.panels.iter().skip(i).take(k.min(6).max(2)).map(|p| p.id).collect();
                    let ok = b.app.structure_op(|d, _| d.join_plans(&ids));
                    format!("réunir {} plans dès {i} ({ok})", ids.len())
                }
                2 => {
                    b.app.structure_op(|d, _| Some(d.duplicate_scene_after(i)));
                    format!("dupliquer le plan {i}")
                }
                3 => {
                    if b.app.espace != Espace::Vignettes {
                        b.app.set_espace(Espace::Vignettes);
                        b.chauffer(2);
                    }
                    let layout = b.app.board_layout();
                    let after = h.n(0, layout.cards.len().saturating_sub(2));
                    let ok = b.app.board_toggle_cut(&layout, after);
                    format!("basculer la coupe après la case {after} ({ok})")
                }
                4 => {
                    b.app.select_panel(i);
                    b.app.record_step();
                    let len = b.app.doc.panels[i].frame_count();
                    let c = b.app.doc.panel_camera(i, 0);
                    let f = h.n(0, len - 1);
                    let cam = Camera { x: c.x + h.entre(-300.0, 300.0), zoom: h.entre(0.7, 2.0), angle: h.entre(-10.0, 10.0), tilt: h.entre(-5.0, 5.0), ..c };
                    crate::camera::set_key(&mut b.app.doc.panels[i].camera, f, cam);
                    b.app.modified = true;
                    format!("clé de caméra sur le plan {i}, image {f}")
                }
                5 => {
                    let e = *h.choix(&espaces);
                    b.app.set_espace(e);
                    format!("onglet {e:?}")
                }
                6 => {
                    for _ in 0..k {
                        b.app.undo();
                    }
                    format!("{k} annulations")
                }
                7 => {
                    for _ in 0..k {
                        b.app.redo();
                    }
                    format!("{k} rétablissements")
                }
                8 => {
                    b.app.select_panel(i);
                    b.app.tool = Tool::Brush;
                    b.chauffer(2);
                    let (rect, ou) = if b.app.espace == Espace::Vignettes {
                        let layout = b.app.board_layout();
                        let kk = layout.cards.iter().position(|c| c.panel == b.app.cur_panel).unwrap_or(0);
                        (b.app.vignettes.xf.rect(layout.cards[kk].thumb), "case")
                    } else {
                        (b.app.canvas_rect.shrink2(vec2(b.app.canvas_rect.width() * 0.2, b.app.canvas_rect.height() * 0.3)), "dessin")
                    };
                    let (de, a) = (rect.left_center() + vec2(rect.width() * 0.1, 0.0), rect.right_center() - vec2(rect.width() * 0.1, 0.0));
                    b.image(vec![Event::PointerMoved(de)], 1.0 / 60.0);
                    b.image(vec![appui(de, true, Modifiers::NONE)], 1.0 / 60.0);
                    for s in 1..=12 {
                        b.image(vec![Event::PointerMoved(de + (a - de) * (s as f32 / 12.0))], 1.0 / 60.0);
                    }
                    b.image(vec![appui(a, false, Modifiers::NONE)], 1.0 / 60.0);
                    format!("trait dans le {ou} du plan {i} ({:?})", b.app.espace)
                }
                9 => {
                    let len = b.app.doc.panels[i].frame_count();
                    let at = h.n(1, len.max(2) - 1);
                    let ok = b.app.structure_op(|d, _| d.split_plan(i, at));
                    format!("couper le plan {i} à l'image {at} ({ok})")
                }
                10 => {
                    let ids: Vec<u64> = b.app.doc.panels.iter().skip(i).take(k.min(5)).map(|p| p.id).collect();
                    let avant = b.app.doc.panels.get(h.n(0, n - 1)).map(|p| p.id);
                    let ok = b.app.structure_op(|d, _| d.move_plans(&ids, avant));
                    format!("déplacer {} plans dès {i} ({ok})", ids.len())
                }
                11 => {
                    let ok = b.app.structure_op(|d, _| {
                        d.nouvelle_version(i, 1)?;
                        Some(i)
                    });
                    format!("nouvelle version du plan {i} ({ok})")
                }
                12 => {
                    let ids: Vec<u64> = b.app.doc.panels.iter().skip(i).take(k.min(8)).map(|p| p.id).collect();
                    let clip = b.app.doc.copy_panels(&ids);
                    let ok = clip.is_some_and(|c| b.app.structure_op(|d, cur| d.paste_plans(&c, cur)));
                    format!("copier-coller {} plans ({ok})", ids.len())
                }
                13 => {
                    b.app.goto_global(h.n(0, b.app.doc.total_frames().saturating_sub(1)));
                    b.app.toggle_play_scope(true);
                    for _ in 0..20 {
                        b.image(Vec::new(), 1.0 / 24.0);
                    }
                    b.app.playing = false;
                    "lecture du montage 20 images".into()
                }
                14 => {
                    let a = *h.choix(&["panneau.supprimer", "scene.dupliquer", "panneau.fusionner_suivant", "panneau.dupliquer", "scene.couper", "panneau.nouveau", "edition.annuler", "edition.retablir", "panneau.ajouter_image", "panneau.retirer_image"]);
                    b.app.select_panel(i);
                    b.app.run_action(a);
                    format!("action {a} sur le plan {i}")
                }
                15 => {
                    if tour % 4 == 0 {
                        b.app.save();
                        while b.app.save_en_cours() {
                            b.image(Vec::new(), 1.0 / 60.0);
                            std::thread::sleep(std::time::Duration::from_millis(2));
                        }
                        format!("enregistrer ({})", b.app.status)
                    } else {
                        b.app.select_panel(i);
                        let len = b.app.doc.panels[i].frame_count();
                        let f = h.n(0, len - 1);
                        let ok = b.app.structure_op_porte(false, |d, cur| d.toggle_mark(cur, f).map(|_| cur));
                        format!("basculer la marque {f} du plan {i} ({ok})")
                    }
                }
                16 => {
                    let n = h.n(1, 400);
                    let ok = b.app.structure_op(|d, _| {
                        d.set_duration(i, n);
                        Some(i)
                    });
                    format!("durée du plan {i} : {n} images ({ok})")
                }
                _ => {
                    for _ in 0..30 {
                        let j = h.n(0, b.app.doc.panels.len() - 1);
                        b.app.select_panel(j);
                        b.image(Vec::new(), 1.0 / 60.0);
                    }
                    "30 changements de plan".into()
                }
            }
        }));
        let ms_op = t.elapsed().as_secs_f64() * 1e3;
        nom = match r {
            Ok(s) => s,
            Err(_) => format!("op {op} PLANTÉE"),
        };
        let r2 = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut pire = 0.0f64;
            for _ in 0..3 {
                pire = pire.max(b.image(Vec::new(), 1.0 / 60.0).total());
            }
            pire
        }));
        let pire = r2.unwrap_or(-1.0);
        if ms_op > 100.0 || pire > 100.0 {
            lents.push((format!("tour {tour} : {nom} (images d'après : pire {pire:.0} ms)"), ms_op));
        }
        journal.push(format!("{tour} {nom} {ms_op:.0} ms"));
        let inc = incoherences(&b);
        if !inc.is_empty() && incoh_vues < 15 {
            incoh_vues += 1;
            println!("INCOHÉRENCE après le tour {tour} « {nom} » : {}", inc.iter().take(4).cloned().collect::<Vec<_>>().join(" ; "));
        }
        if pire < 0.0 || nom.contains("PLANTÉE") {
            println!("CHASSE plantage au tour {tour} : {nom} ; avant : {}", journal.iter().rev().skip(1).take(4).cloned().collect::<Vec<_>>().join(" | "));
        }
        if tour % 25 == 0 {
            let (ws, pv) = memoire();
            println!("CHASSE tour {tour}/{tours} : {} plans, {} dessins, {} étapes d'annulation, RSS {ws:.0} Mo, privée {pv:.0} Mo, {:.0} s", b.app.doc.panels.len(), b.app.doc.cels.len(), b.app.history.undo_len(), t0.elapsed().as_secs_f64());
        }
        if PLANTAGES.lock().unwrap().len() > 25 {
            println!("CHASSE trop de plantages, arrêt");
            break;
        }
        // Rouvrir de temps en temps (ce qui a été enregistré).
        if tour % 150 == 149 {
            b.app.save();
            while b.app.save_en_cours() {
                b.image(Vec::new(), 1.0 / 60.0);
            }
            let p = b.app.path.clone().unwrap();
            let t = Instant::now();
            b.app.modified = false;
            // (Le même projet déjà ouvert serait seulement montré : on passe par un projet vide d'abord.)
            b.app.load_doc(Document::with_frame(1920, 1080, 24));
            b.app.path = None;
            let lache = t.elapsed().as_secs_f64();
            b.app.open_path(p);
            while b.app.open_job.is_some() {
                b.image(Vec::new(), 1.0 / 60.0);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            println!("CHASSE rouvert au tour {tour} : lâcher l'ancien {lache:.1} s, total {:.1} s, {} plans, statut : {}", t.elapsed().as_secs_f64(), b.app.doc.panels.len(), b.app.status);
        }
    }
    lents.sort_by(|a, b| b.1.total_cmp(&a.1));
    println!("CHASSE {} opérations lentes (> 100 ms), les pires :", lents.len());
    for (n, ms) in lents.iter().take(25) {
        println!("  {ms:>8.0} ms  {n}");
    }
    let p = PLANTAGES.lock().unwrap();
    println!("CHASSE {} plantage(s)", p.len());
    for m in p.iter() {
        println!("=== {m}");
    }
    let (ws, pv) = memoire();
    println!("CHASSE fin : RSS {ws:.0} Mo, privée {pv:.0} Mo, {:.0} s", t0.elapsed().as_secs_f64());
    std::process::exit(0);
}

/// Le tas de Windows seul, sans SHOT : `n` blocs de la taille d'une tuile partagée (16 Ko + l'en-tête d'un `Arc`),
/// alloués sur plusieurs fils comme à l'ouverture, puis rendus d'un coup.
#[test]
#[ignore]
fn lourd_tas() {
    for n in [20_000usize, 40_000, 80_000, 160_000] {
        let t = Instant::now();
        let fils = 8;
        let blocs: Vec<Vec<Arc<[u32; 4096]>>> = std::thread::scope(|sc| {
            let h: Vec<_> = (0..fils).map(|f| sc.spawn(move || (0..n / fils).map(|k| Arc::new([(k + f) as u32; 4096])).collect::<Vec<_>>())).collect();
            h.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let alloue = t.elapsed().as_secs_f64();
        // Des petites allocations entre deux (comme les traits vectoriels).
        let petits: Vec<Vec<u8>> = (0..n * 4).map(|k| vec![0u8; 24 + k % 200]).collect();
        let t = Instant::now();
        drop(blocs);
        let rendu = t.elapsed().as_secs_f64();
        drop(petits);
        println!("LOURD tas : {n} tuiles ({:.0} Mo) : allouées en {alloue:.2} s, rendues en {rendu:.2} s", n as f64 * 16384.0 / 1e6);
    }
}
