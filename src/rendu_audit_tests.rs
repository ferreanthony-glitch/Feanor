//! Audit du rendu des brosses (2026-10-18, `docs/AUDIT-RENDU-BROSSES.md`) : des planches PNG
//! rendues par le moteur de Pelure sur papier blanc, pour juger à l'œil ce qui fait « numérique »
//! et non « traditionnel ». Essai ignoré, à lancer à la main :
//!
//! `cargo test --release --offline --target-dir target\audit rendu_audit -- --ignored --nocapture`
//!
//! Les images vont dans `audit-rendu/` à la racine du dépôt, avec `mesures.txt` (chiffres et
//! réglages) et `LISEZMOI.txt` (comment lire les planches). Chaque planche : quatre exercices en
//! colonnes (rampe de pression, hachures, courbe en S, zone remplie par passages croisés), des
//! rangées à 100 % puis les mêmes en gros plan ×4.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use eframe::egui::{pos2, vec2, Pos2};
use image::{Rgb, RgbImage};

use crate::brush::{BrushSettings, Connection, Curve, Source, StrokeMode, StrokeState, Toucher};
use crate::brushtex::{grain_factor, procedural_paper, BrushLibrary, BrushTextures, GrayImage, Target, PAPER_COARSE};
use crate::raster::Raster;
use crate::vector::{self, Element, VPoint};

/// Une position du stylet : point, appui (0 à 1), instant (secondes).
type Sample = (Pos2, f32, f64);

/// Un trait : ses échantillons et sa couleur.
struct Trait {
    points: Vec<Sample>,
    color: [u8; 3],
}

/// Une case d'exercice, en pixels du dessin.
const CELL_W: usize = 260;
const CELL_H: usize = 170;
/// Marge entre les cases, et place des étiquettes.
const MARGE: usize = 10;
const ENTETE: usize = 26;
const GAUCHE: usize = 118;
/// Gros plan : facteur et fenêtre (largeur × hauteur, en pixels du dessin).
const ZOOM: usize = 4;
const FENETRE: (usize, usize) = (64, 42);
/// Encre et seconde couleur des passages croisés.
const NOIR: [u8; 3] = [20, 20, 20];
const BLEU: [u8; 3] = [40, 70, 160];
/// Taille au-delà de laquelle une brosse importée est ramenée pour tenir dans la case.
const TAILLE_MAX: f32 = 36.0;

fn out_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("audit-rendu")
}

// ------------------------------------------------------------------------------------ gestes

fn bell(t: f32) -> f32 {
    (t * std::f32::consts::PI).sin()
}

/// Un léger tremblé de la main, déterministe, en travers du trait.
fn tremble(t: f32) -> f32 {
    0.45 * (t * 37.0).sin() + 0.25 * (t * 91.0 + 1.3).sin()
}

/// Échantillonne une courbe `f(t)` (t de 0 à 1) tous les `step` pixels environ, à `speed(t)`
/// pixels par seconde, avec l'appui `pressure(t)` ; les instants partent de `t0`.
fn sample(f: impl Fn(f32) -> Pos2, pressure: impl Fn(f32) -> f32, speed: impl Fn(f32) -> f32, step: f32, t0: f64) -> Vec<Sample> {
    let n = 400;
    let coarse: Vec<Pos2> = (0..=n).map(|k| f(k as f32 / n as f32)).collect();
    let total: f32 = coarse.windows(2).map(|w| w[0].distance(w[1])).sum();
    let count = ((total / step).ceil() as usize).max(2);
    let mut out = Vec::with_capacity(count + 1);
    let mut time = t0;
    let mut prev: Option<Pos2> = None;
    for k in 0..=count {
        let t = k as f32 / count as f32;
        let d = f((t + 0.01).min(1.0)) - f((t - 0.01).max(0.0));
        let len = d.length().max(1e-3);
        let p = f(t) + vec2(-d.y, d.x) / len * tremble(t);
        if let Some(q) = prev {
            time += (q.distance(p) / speed(t).max(1.0)) as f64;
        }
        prev = Some(p);
        out.push((p, pressure(t).clamp(0.0, 1.0), time));
    }
    out
}

/// A. Rampe de pression : une ligne presque droite, l'appui de 2 % à 100 %.
fn rampe() -> Vec<Trait> {
    let pts = sample(|t| pos2(16.0 + t * (CELL_W as f32 - 32.0), CELL_H as f32 * 0.5 + (t * 6.0).sin() * 3.0), |t| 0.02 + 0.98 * t, |_| 350.0, 1.5, 0.0);
    vec![Trait { points: pts, color: NOIR }]
}

/// B. Hachures : sept traits obliques rapides, appui en cloche léger → moyen ; puis cinq traits
/// croisés, plus légers encore.
fn hachures() -> Vec<Trait> {
    let mut v = Vec::new();
    let mut t0 = 0.0;
    for k in 0..7 {
        let x0 = 30.0 + k as f32 * 26.0;
        let pts = sample(move |t| pos2(x0 + t * 40.0, 30.0 + t * 85.0), |t| 0.15 + 0.55 * bell(t), |_| 700.0, 1.5, t0);
        t0 += 0.4;
        v.push(Trait { points: pts, color: NOIR });
    }
    for k in 0..5 {
        let y0 = 55.0 + k as f32 * 20.0;
        let pts = sample(move |t| pos2(40.0 + t * 180.0, y0 - t * 12.0), |t| 0.1 + 0.3 * bell(t), |_| 900.0, 1.5, t0);
        t0 += 0.4;
        v.push(Trait { points: pts, color: NOIR });
    }
    v
}

/// C. Courbe en S : lente et légère aux bouts, rapide et appuyée au milieu.
fn courbe_s() -> Vec<Trait> {
    let (a, b, c, d) = (pos2(20.0, 150.0), pos2(150.0, 150.0), pos2(110.0, 20.0), pos2(240.0, 20.0));
    let bez = move |t: f32| {
        let u = 1.0 - t;
        pos2(
            u * u * u * a.x + 3.0 * u * u * t * b.x + 3.0 * u * t * t * c.x + t * t * t * d.x,
            u * u * u * a.y + 3.0 * u * u * t * b.y + 3.0 * u * t * t * c.y + t * t * t * d.y,
        )
    };
    let pts = sample(bez, |t| 0.12 + 0.8 * bell(t), |t| 150.0 + 900.0 * bell(t), 1.5, 0.0);
    vec![Trait { points: pts, color: NOIR }]
}

/// D. Zone remplie par passages croisés : un zigzag horizontal (noir, appui 45 %), puis un zigzag
/// vertical (bleu) qui recouvre la moitié droite — l'accumulation dans un trait et d'un trait à
/// l'autre, et ce que deux couleurs font l'une sur l'autre.
fn croises() -> Vec<Trait> {
    let zig = |t: f32, n: f32| {
        let s = t * n;
        let k = s.floor();
        let f = s - k;
        if k as i32 % 2 == 0 { f } else { 1.0 - f }
    };
    let h = sample(move |t| pos2(30.0 + zig(t, 6.0) * 150.0, 30.0 + t * 110.0), |_| 0.45, |_| 600.0, 1.5, 0.0);
    let v = sample(move |t| pos2(110.0 + t * 120.0, 40.0 + zig(t, 5.0) * 90.0), |_| 0.45, |_| 600.0, 1.5, 2.0);
    vec![Trait { points: h, color: NOIR }, Trait { points: v, color: BLEU }]
}

fn exercices() -> [Vec<Trait>; 4] {
    [rampe(), hachures(), courbe_s(), croises()]
}

/// Fenêtre du gros plan de chaque exercice (coin haut gauche) : le début léger de la rampe, le
/// cœur des hachures, le milieu du S, la frontière des passages croisés.
const FENETRES: [(usize, usize); 4] = [(24, 64), (100, 60), (100, 64), (90, 60)];

// ------------------------------------------------------------------------------------- rendu

/// Trace les traits en bitmap, comme sur un calque bitmap de Pelure ; rend les chemins (ligne
/// médiane et rayons) que garderait un calque vectoriel.
fn trace_bitmap(raster: &mut Raster, s: &BrushSettings, tex: &BrushTextures, traits: &[Trait], feel: Toucher) -> Vec<Vec<VPoint>> {
    let mut paths = Vec::new();
    for t in traits {
        let mut st = StrokeState::new(0, StrokeMode::Paint(t.color), *s, None, true).with_textures(tex.clone()).with_feel(feel);
        let mut dirty = Vec::new();
        for &(p, pr, time) in &t.points {
            st.add_point_at(raster, p, pr, time, &mut dirty);
        }
        st.finish(raster, &mut dirty);
        paths.push(std::mem::take(&mut st.path));
    }
    paths
}

/// Pose les traits comme un calque vectoriel au lâcher (`commit_vector_stroke`) : chemin simplifié
/// à 0,25 px, élément `Stroke` rendu par `vector::render_element`.
fn trace_vector(raster: &mut Raster, s: &BrushSettings, lib: &BrushLibrary, paths: &[Vec<VPoint>], traits: &[Trait], hardness: f32) {
    let brush = lib.for_vector(s).map(Box::new);
    for (path, t) in paths.iter().zip(traits) {
        let points = vector::simplify(path, 0.25);
        if points.is_empty() {
            continue;
        }
        let e = Element::Stroke { points, color: t.color, opacity: s.opacity, hardness, brush: brush.clone() };
        vector::render_element(raster, &e);
    }
}

/// Une rangée de la planche : son nom et ses quatre cases.
struct Rangee {
    nom: String,
    cases: [Raster; 4],
}

fn rangee_bitmap(nom: &str, s: &BrushSettings, tex: &BrushTextures, feel: Toucher) -> (Rangee, [Vec<Vec<VPoint>>; 4]) {
    let ex = exercices();
    let mut cases = Vec::new();
    let mut paths = Vec::new();
    for traits in &ex {
        let mut r = Raster::new(CELL_W, CELL_H);
        paths.push(trace_bitmap(&mut r, s, tex, traits, feel));
        cases.push(r);
    }
    let cases: [Raster; 4] = cases.try_into().ok().expect("quatre cases");
    let paths: [Vec<Vec<VPoint>>; 4] = paths.try_into().ok().expect("quatre chemins");
    (Rangee { nom: nom.into(), cases }, paths)
}

fn rangee_vector(nom: &str, s: &BrushSettings, lib: &BrushLibrary, paths: &[Vec<Vec<VPoint>>; 4], hardness: f32) -> Rangee {
    let ex = exercices();
    let mut cases = Vec::new();
    for (traits, p) in ex.iter().zip(paths) {
        let mut r = Raster::new(CELL_W, CELL_H);
        trace_vector(&mut r, s, lib, p, traits, hardness);
        cases.push(r);
    }
    Rangee { nom: nom.into(), cases: cases.try_into().ok().expect("quatre cases") }
}

// ---------------------------------------------------------------------------------- planches

/// Pixel prémultiplié posé sur du blanc.
fn sur_blanc(c: eframe::egui::Color32) -> Rgb<u8> {
    let a = 255 - c.a() as u32;
    Rgb([(c.r() as u32 + a).min(255) as u8, (c.g() as u32 + a).min(255) as u8, (c.b() as u32 + a).min(255) as u8])
}

/// Pose `r` à 1:1 en (ox, oy).
fn poser(img: &mut RgbImage, r: &Raster, ox: usize, oy: usize) {
    for y in 0..r.height {
        for x in 0..r.width {
            let (px, py) = ((ox + x) as u32, (oy + y) as u32);
            if px < img.width() && py < img.height() {
                img.put_pixel(px, py, sur_blanc(r.get(x, y)));
            }
        }
    }
}

/// Pose la fenêtre `(fx, fy)` de `r`, agrandie `ZOOM` fois (pixels francs), en (ox, oy).
fn poser_zoom(img: &mut RgbImage, r: &Raster, (fx, fy): (usize, usize), ox: usize, oy: usize) {
    for y in 0..FENETRE.1 {
        for x in 0..FENETRE.0 {
            let (sx, sy) = ((fx + x).min(r.width - 1), (fy + y).min(r.height - 1));
            let c = sur_blanc(r.get(sx, sy));
            for dy in 0..ZOOM {
                for dx in 0..ZOOM {
                    let (px, py) = ((ox + x * ZOOM + dx) as u32, (oy + y * ZOOM + dy) as u32);
                    if px < img.width() && py < img.height() {
                        img.put_pixel(px, py, c);
                    }
                }
            }
        }
    }
}

/// Écrit `text` en gris dans l'image (lettres de la police par défaut de Pelure, remplies).
fn etiquette(img: &mut RgbImage, x: f32, y: f32, text: &str, size: f32) {
    let laid = crate::text::layout(text, "Segoe UI", size);
    let (w, h) = (img.width() as usize, img.height() as usize);
    for g in &laid.glyphs {
        let contours: Vec<Vec<[f32; 2]>> = g.iter().map(|c| c.iter().map(|p| [p[0] + x, p[1] + y]).collect()).collect();
        let Some((x0, y0, cw, ch, cov)) = vector::polygon_coverage(&contours, w, h) else { continue };
        for ly in 0..ch {
            for lx in 0..cw {
                let a = cov[ly * cw + lx].clamp(0.0, 1.0);
                if a <= 0.0 {
                    continue;
                }
                let px = img.get_pixel_mut((x0 + lx) as u32, (y0 + ly) as u32);
                for c in 0..3 {
                    px[c] = (px[c] as f32 * (1.0 - a) + 50.0 * a) as u8;
                }
            }
        }
    }
}

fn cadre(img: &mut RgbImage, x: usize, y: usize, w: usize, h: usize) {
    let g = Rgb([205, 205, 205]);
    for i in 0..w {
        img.put_pixel((x + i) as u32, y as u32, g);
        img.put_pixel((x + i) as u32, (y + h - 1) as u32, g);
    }
    for j in 0..h {
        img.put_pixel(x as u32, (y + j) as u32, g);
        img.put_pixel((x + w - 1) as u32, (y + j) as u32, g);
    }
}

/// Assemble la planche : titre, une rangée par `Rangee` à 100 %, puis les mêmes en gros plan ×4.
fn planche(titre: &str, sous_titre: &str, rangees: &[Rangee]) -> RgbImage {
    let cols = 4;
    let rows = rangees.len() * 2;
    let w = GAUCHE + cols * (CELL_W + MARGE) + MARGE;
    let h = ENTETE + 20 + rows * (CELL_H + MARGE) + MARGE;
    let mut img = RgbImage::from_pixel(w as u32, h as u32, Rgb([255, 255, 255]));
    etiquette(&mut img, MARGE as f32, 4.0, titre, 17.0);
    etiquette(&mut img, MARGE as f32, 24.0, sous_titre, 11.0);
    let noms = ["A rampe de pression", "B hachures", "C courbe en S", "D passages croisés"];
    for (c, n) in noms.iter().enumerate() {
        etiquette(&mut img, (GAUCHE + c * (CELL_W + MARGE) + 4) as f32, (ENTETE + 6) as f32, n, 11.0);
    }
    let top = ENTETE + 20;
    for (i, r) in rangees.iter().enumerate() {
        let y = top + i * (CELL_H + MARGE);
        etiquette(&mut img, MARGE as f32, (y + 4) as f32, &format!("{} 100 %", r.nom), 11.0);
        for (c, case) in r.cases.iter().enumerate() {
            let x = GAUCHE + c * (CELL_W + MARGE);
            poser(&mut img, case, x, y);
            cadre(&mut img, x, y, CELL_W, CELL_H);
        }
    }
    for (i, r) in rangees.iter().enumerate() {
        let y = top + (rangees.len() + i) * (CELL_H + MARGE);
        etiquette(&mut img, MARGE as f32, (y + 4) as f32, &format!("{} ×{ZOOM}", r.nom), 11.0);
        for (c, case) in r.cases.iter().enumerate() {
            let x = GAUCHE + c * (CELL_W + MARGE);
            poser_zoom(&mut img, case, FENETRES[c], x, y);
            cadre(&mut img, x, y, FENETRE.0 * ZOOM + 2, FENETRE.1 * ZOOM + 2);
        }
    }
    img
}

fn save(img: &RgbImage, fichier: &str) -> PathBuf {
    let path = out_dir().join(fichier);
    img.save(&path).unwrap_or_else(|e| panic!("{} : {e}", path.display()));
    println!("planche : {}", path.display());
    path
}

// ---------------------------------------------------------------------------------- mesures

/// Chiffres sur les cases bitmap : alpha le long de la rampe, uniformité et aplat du S,
/// accumulation des passages croisés.
fn mesures(cases: &[Raster; 4]) -> String {
    let mut s = String::new();
    // A : alpha max et largeur de la colonne, à 10 %, 30 %, 50 %, 70 %, 90 % de la rampe.
    let a = &cases[0];
    let _ = write!(s, "  rampe (appui → alpha max / largeur px) :");
    for f in [0.1, 0.3, 0.5, 0.7, 0.9] {
        let x = (16.0 + f * (CELL_W as f32 - 32.0)) as usize;
        let (mut max, mut n) = (0u8, 0);
        for y in 0..a.height {
            let v = a.get(x, y).a();
            if v > 0 {
                n += 1;
                max = max.max(v);
            }
        }
        let _ = write!(s, "  {:.0} % → {max}/{n}", 0.02 * 100.0 + 0.98 * f * 100.0);
    }
    s.push('\n');
    // C : part des pixels couverts qui sont pleins (alpha ≥ 250), nombre de niveaux distincts,
    // écart-type de l'alpha des pixels couverts à plus de moitié.
    let c = &cases[2];
    let mut levels = [false; 256];
    let (mut covered, mut full) = (0usize, 0usize);
    let mut vals = Vec::new();
    for y in 0..c.height {
        for x in 0..c.width {
            let v = c.get(x, y).a();
            if v > 0 {
                covered += 1;
                levels[v as usize] = true;
                if v >= 250 {
                    full += 1;
                }
                if v >= 128 {
                    vals.push(v as f32);
                }
            }
        }
    }
    let mean = vals.iter().sum::<f32>() / vals.len().max(1) as f32;
    let sd = (vals.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / vals.len().max(1) as f32).sqrt();
    let _ = writeln!(
        s,
        "  courbe S : {covered} px couverts, {:.0} % pleins (alpha ≥ 250), {} niveaux d'alpha, écart-type de l'intérieur {sd:.1}",
        100.0 * full as f32 / covered.max(1) as f32,
        levels.iter().filter(|&&b| b).count()
    );
    // D : alpha moyen dans la zone d'un seul passage et dans la zone croisée.
    let d = &cases[3];
    let zone = |x0: usize, x1: usize, y0: usize, y1: usize| {
        let mut sum = 0.0;
        for y in y0..y1 {
            for x in x0..x1 {
                sum += d.get(x, y).a() as f32;
            }
        }
        sum / ((x1 - x0) * (y1 - y0)) as f32
    };
    let (seul, croise) = (zone(40, 100, 45, 125), zone(120, 170, 50, 120));
    let _ = writeln!(s, "  passages croisés : alpha moyen un passage {seul:.0}, croisé {croise:.0} (rapport {:.2})", croise / seul.max(1.0));
    s
}

/// Écart entre le rendu bitmap et le rendu vectoriel des quatre cases : écart moyen d'alpha sur
/// les pixels touchés par l'un ou l'autre, et part de ces pixels dont l'alpha diffère de plus
/// de 16 (sur 255).
fn ecart(bitmap: &[Raster; 4], vecto: &[Raster; 4]) -> String {
    let mut s = String::from("  écart bitmap / vectoriel (|Δalpha| moyen ; pixels à plus de 16 d'écart) :");
    for (k, (b, v)) in bitmap.iter().zip(vecto).enumerate() {
        let (mut sum, mut n, mut far) = (0.0f32, 0usize, 0usize);
        for y in 0..b.height {
            for x in 0..b.width {
                let (pb, pv) = (b.get(x, y).a() as i32, v.get(x, y).a() as i32);
                if pb == 0 && pv == 0 {
                    continue;
                }
                n += 1;
                let d = (pb - pv).abs();
                sum += d as f32;
                if d > 16 {
                    far += 1;
                }
            }
        }
        let _ = write!(s, "  {} {:.1} ; {:.0} %", ['A', 'B', 'C', 'D'][k], sum / n.max(1) as f32, 100.0 * far as f32 / n.max(1) as f32);
    }
    s.push('\n');
    s
}

// ------------------------------------------------------------------------------- réglages

/// Un réglage à rendre : nom, fichier, réglage, dureté gardée par le trait vectoriel.
struct Cas {
    nom: String,
    fichier: String,
    settings: BrushSettings,
    hardness_vec: f32,
    source: String,
}

fn cas(nom: &str, fichier: &str, settings: BrushSettings, source: &str) -> Cas {
    Cas { nom: nom.into(), fichier: fichier.into(), settings, hardness_vec: settings.hardness, source: source.into() }
}

/// Ramène une brosse importée à une taille qui tient dans la case.
fn bornee(mut s: BrushSettings, notes: &mut String) -> BrushSettings {
    if s.size > TAILLE_MAX {
        let _ = writeln!(notes, "  (taille {:.0} px ramenée à {TAILLE_MAX:.0} px pour la planche)", s.size);
        s.size = TAILLE_MAX;
    }
    s
}

/// Rend un cas : bitmap et vectoriel, planche et mesures.
fn rendre_cas(c: &Cas, lib: &BrushLibrary, mesures_txt: &mut String) {
    let tex = lib.textures(&c.settings);
    let (bitmap, paths) = rangee_bitmap("bitmap", &c.settings, &tex, Toucher::default());
    let vecto = rangee_vector("vectoriel", &c.settings, lib, &paths, c.hardness_vec);
    let s = &c.settings;
    let sous = format!(
        "{} — taille {:.1}, opacité {:.2}, débit {:.2}{}, dureté {:.2}{}, pas {:.2}, grain {:.2}{}{}, taille mini {:.2}{}{}",
        c.source,
        s.size,
        s.opacity,
        s.power,
        if s.build_up { " (accumule)" } else { "" },
        s.hardness,
        if s.profile.is_some() { " + profil" } else { "" },
        s.spacing,
        s.grain,
        if s.paper.is_some() { " + papier" } else { "" },
        if s.tip.is_some() { ", pointe en image" } else { "" },
        s.min_size,
        if s.pressure_opacity || s.dynamics.opacity.active() { ", opacité ↔ appui" } else { "" },
        if s.mix > 0.0 || s.charge < 1.0 { &", humide" } else { &"" },
    );
    let _ = writeln!(mesures_txt, "## {} ({})", c.nom, c.fichier);
    let _ = writeln!(mesures_txt, "  réglage : {}", serde_json::to_string(s).unwrap_or_default());
    mesures_txt.push_str(&mesures(&bitmap.cases));
    mesures_txt.push_str(&ecart(&bitmap.cases, &vecto.cases));
    mesures_txt.push('\n');
    let img = planche(&c.nom, &sous, &[bitmap, vecto]);
    save(&img, &c.fichier);
}

/// Planche du bord : le pinceau rond à appui constant, en 1,5, 3 et 8 px, bitmap (bord doux du
/// toucher) contre vectoriel (`Mask::stamp`, l'ancienne couverture) — pour l'étude « vectoriel
/// comme bitmap ».
fn planche_bord(lib: &BrushLibrary) {
    let mut rangees = Vec::new();
    for size in [1.5f32, 3.0, 8.0] {
        let s = BrushSettings { size, pressure_size: false, smoothing: 0.0, ..BrushSettings::brush() };
        let tex = lib.textures(&s);
        let (b, paths) = rangee_bitmap(&format!("bitmap {size} px"), &s, &tex, Toucher::default());
        let v = rangee_vector(&format!("vectoriel {size} px"), &s, lib, &paths, s.hardness);
        rangees.push(b);
        rangees.push(v);
    }
    let img = planche("Bord du trait : bitmap contre vectoriel (pinceau rond, appui constant)", "le vectoriel garde l'ancienne couverture (rampe d'un pixel coupée par la dureté), le bitmap le bord doux du toucher", &rangees);
    save(&img, "12_vecto_bord.png");
}

/// Les réglages livrés avec Pelure, plus le crayon et le pinceau de la barre d'outils.
fn cas_pelure(lib: &BrushLibrary) -> Vec<Cas> {
    let mut v = vec![
        cas("Crayon (barre d'outils)", "01_pelure_crayon-barre.png", BrushSettings::pencil(), "BrushSettings::pencil()"),
        cas("Pinceau (barre d'outils) = Pinceau rond", "02_pelure_pinceau-rond.png", BrushSettings::brush(), "BrushSettings::brush()"),
    ];
    // Le trait vectoriel du crayon de la barre garde une dureté de 1 (`begin_vector_stroke`).
    v[0].hardness_vec = 1.0;
    for (k, p) in lib.presets.iter().filter(|p| p.source == "Pelure" && p.target == Target::Brush).enumerate() {
        if p.name == "Pinceau rond" {
            continue;
        }
        let slug = p.name.to_lowercase().replace(' ', "-").replace('à', "a").replace('é', "e");
        v.push(cas(&p.name, &format!("{:02}_pelure_{slug}.png", 3 + k), p.settings, "réglage livré « Pelure »"));
    }
    v
}

/// Essais : ce que les briques déjà là donnent avec d'autres valeurs (pas de nouveau code).
fn cas_essais(lib: &BrushLibrary) -> Vec<Cas> {
    let base = BrushSettings::brush();
    // Crayon graphite : pointe un peu aplatie qui suit le trait, papier gros grain agrandi, débit
    // qui s'accumule, appui qui joue sur l'opacité, un peu de hasard.
    let mut graphite = BrushSettings {
        size: 5.0,
        hardness: 0.7,
        spacing: 0.12,
        min_size: 0.3,
        pressure_opacity: true,
        min_opacity: 0.08,
        power: 0.55,
        build_up: true,
        grain: 0.9,
        paper: Some(PAPER_COARSE),
        paper_scale: 2.5,
        size_jitter: 0.15,
        scatter: 0.04,
        roundness: 0.8,
        smoothing: 0.2,
        ..base
    };
    graphite.dynamics.angle = Connection { source: Source::Direction, ..Default::default() };
    // Craie : grosse pointe douce, grain plein, deux empreintes dispersées par pas.
    let craie = BrushSettings {
        size: 14.0,
        hardness: 0.4,
        spacing: 0.1,
        min_size: 0.5,
        pressure_opacity: true,
        min_opacity: 0.2,
        power: 0.5,
        build_up: true,
        grain: 1.0,
        paper: Some(PAPER_COARSE),
        paper_scale: 2.0,
        count: 2,
        scatter: 0.15,
        size_jitter: 0.2,
        ..base
    };
    // Gouache : peinture humide qui se mélange, pointe plate qui suit le trait.
    let mut gouache = BrushSettings {
        size: 16.0,
        hardness: 0.6,
        spacing: 0.08,
        min_size: 0.6,
        power: 0.9,
        mix: 0.3,
        charge: 0.85,
        roundness: 0.5,
        ..base
    };
    gouache.dynamics.angle = Connection { source: Source::Direction, ..Default::default() };
    // Encre au pinceau : courbe de taille ferme, pas de jitter — l'encre est le trait plat.
    let mut encre = BrushSettings { size: 7.0, hardness: 1.0, spacing: 0.05, min_size: 0.04, smoothing: 0.35, ..base };
    encre.dynamics.size = Connection { source: Source::Pressure, curve: Curve::from_points(&[[0.0, 0.0], [0.4, 0.15], [0.8, 0.7], [1.0, 1.0]]), min: 0.04, ..Default::default() };
    let _ = lib;
    vec![
        cas("Essai graphite (briques existantes)", "20_essai_graphite.png", graphite, "essai de l'audit"),
        cas("Essai craie (briques existantes)", "21_essai_craie.png", craie, "essai de l'audit"),
        cas("Essai gouache humide (briques existantes)", "22_essai_gouache.png", gouache, "essai de l'audit"),
        cas("Essai encre au pinceau (briques existantes)", "23_essai_encre.png", encre, "essai de l'audit"),
    ]
}

/// Les outils TVPaint d'Anthony (`raccourcis outils.tvpx`, lecture seule) et une sélection des
/// panneaux livrés avec TVPaint 12, importés par Pelure.
fn cas_tvpaint(lib: &mut BrushLibrary, notes: &mut String) -> Vec<Cas> {
    let mut v = Vec::new();
    let res = crate::tvpbrush::tvpaint_resources();
    let _ = writeln!(notes, "## Imports TVPaint");
    let _ = writeln!(notes, "  Resources : {:?}", res);
    let mine = Path::new(r"C:\Users\ferre\Documents\Logiciels\Config TVPaint\raccourcis outils.tvpx");
    match std::fs::read(mine).map_err(|e| e.to_string()).and_then(|b| crate::tvpbrush::Panel::from_tvpx(&b)) {
        Ok(panel) => {
            let before = lib.presets.len();
            let r = crate::tvpbrush::import_panel(&panel, "Mon TVPX", lib, res.as_deref());
            let _ = writeln!(notes, "  « raccourcis outils.tvpx » : {} réglages, {} pointes, {} papiers ; notes : {}", r.presets, r.tips, r.papers, r.notes.join(" | "));
            for p in lib.presets[before..].to_vec() {
                let _ = writeln!(notes, "  {} : {}", p.name, serde_json::to_string(&p.settings).unwrap_or_default());
                if p.target == Target::Brush {
                    let s = bornee(p.settings, notes);
                    v.push(cas(&format!("TVPaint d'Anthony : {}", p.name), &format!("30_tvpx_anthony_{}.png", p.name.to_lowercase()), s, "raccourcis outils.tvpx (tv_pencil)"));
                }
            }
        }
        Err(e) => {
            let _ = writeln!(notes, "  « raccourcis outils.tvpx » illisible : {e}");
        }
    }
    let Some(res) = res else {
        let _ = writeln!(notes, "  TVPaint 12 introuvable : panneaux livrés non rendus.");
        return v;
    };
    let choix: [(&str, &[&str]); 4] = [
        ("Pencils_v1", &["2B  Pencil", "HB Pencil", "Pencil 1", "Pencil 2", "Sketch Pencil", "8B Pencil"]),
        ("Chalks_v1", &["Chalk 1", "Charcoal"]),
        ("Pens_v1", &["Pen 1", "Pen 2", "Chinese Brush", "Watercolor Pen"]),
        ("PaintBrushes_v1", &["Oil 1", "Oil 2", "Acrylic 1", "Watercolour 1", "Watercolour 5", "Watercolour 9"]),
    ];
    let mut k = 40;
    for (panel_name, names) in choix {
        let path = res.join("custompanels").join(format!("{panel_name}.bin"));
        let Ok(panel) = crate::tvpbrush::Panel::from_bin(&path) else {
            let _ = writeln!(notes, "  {panel_name} : illisible");
            continue;
        };
        let before = lib.presets.len();
        let r = crate::tvpbrush::import_panel(&panel, panel_name, lib, Some(&res));
        let _ = writeln!(notes, "  {panel_name} : {} réglages, {} pointes, {} papiers", r.presets, r.tips, r.papers);
        for n in &r.notes {
            let _ = writeln!(notes, "    {n}");
        }
        for p in lib.presets[before..].to_vec() {
            if !names.contains(&p.name.as_str()) {
                continue;
            }
            let _ = writeln!(notes, "  {} : {}", p.name, serde_json::to_string(&p.settings).unwrap_or_default());
            let s = bornee(p.settings, notes);
            let slug = p.name.trim().to_lowercase().replace(' ', "-").replace("--", "-");
            v.push(cas(&format!("TVPaint {panel_name} : {}", p.name.trim()), &format!("{k}_tvp_{slug}.png"), s, &format!("panneau {panel_name} de TVPaint 12, importé")));
            k += 1;
        }
    }
    v
}

/// Réglages Krita / MyPaint du poste (lecture seule), importés par Pelure.
fn cas_krita(lib: &mut BrushLibrary, notes: &mut String) -> Vec<Cas> {
    let mut v = Vec::new();
    let dir = Path::new(r"C:\Users\ferre\AppData\Roaming\krita\paintoppresets");
    let brushes = Path::new(r"C:\Users\ferre\AppData\Roaming\krita\brushes");
    let _ = writeln!(notes, "## Imports Krita / MyPaint");
    let mut k = 60;
    for (file, nom) in [("c)_Pencil_2b_(mypaint).myb", "Pencil 2B (MyPaint)"), ("d)_Ink_pen_(mypaint).myb", "Ink pen (MyPaint)"), ("e)_Marker_Medium_(mypaint).myb", "Marker Medium (MyPaint)")] {
        let Ok(text) = std::fs::read_to_string(dir.join(file)) else { continue };
        let before = lib.presets.len();
        match crate::kritabrush::import_myb(&text, nom, "Krita (MyPaint)", lib) {
            Ok(r) => {
                let _ = writeln!(notes, "  {nom} : {}", r.notes.join(" | "));
            }
            Err(e) => {
                let _ = writeln!(notes, "  {nom} : {e}");
                continue;
            }
        }
        for p in lib.presets[before..].to_vec() {
            let _ = writeln!(notes, "  {} : {}", p.name, serde_json::to_string(&p.settings).unwrap_or_default());
            let s = bornee(p.settings, notes);
            v.push(cas(&format!("Krita/MyPaint : {}", p.name), &format!("{k}_krita_{}.png", file.trim_end_matches(".myb").to_lowercase()), s, "réglage MyPaint du poste, importé"));
            k += 1;
        }
    }
    for file in ["b)_Basic-5_Size_default.kpp", "j)_WaterC_Basic_Round-Grain.kpp"] {
        let Ok(bytes) = std::fs::read(dir.join(file)) else { continue };
        let before = lib.presets.len();
        match crate::kritabrush::import_kpp(&bytes, "Krita", lib, Some(brushes)) {
            Ok(r) => {
                let _ = writeln!(notes, "  {file} : {}", r.notes.join(" | "));
            }
            Err(e) => {
                let _ = writeln!(notes, "  {file} : {e}");
                continue;
            }
        }
        for p in lib.presets[before..].to_vec() {
            let _ = writeln!(notes, "  {} : {}", p.name, serde_json::to_string(&p.settings).unwrap_or_default());
            let s = bornee(p.settings, notes);
            v.push(cas(&format!("Krita : {}", p.name), &format!("{k}_krita_{}.png", file.trim_end_matches(".kpp").to_lowercase()), s, "réglage Krita du poste, importé"));
            k += 1;
        }
    }
    v
}

// --------------------------------------------------------------------------- papiers, pointes

/// Pose une image en gris (0 à 1 = blanc à noir, ou l'inverse) en (ox, oy), agrandie `k` fois.
fn poser_gris(img: &mut RgbImage, g: &GrayImage, (fx, fy, fw, fh): (usize, usize, usize, usize), ox: usize, oy: usize, k: usize, noir_pour_1: bool) {
    for y in 0..fh {
        for x in 0..fw {
            let v = g.get((fx + x) % g.width, (fy + y) % g.height).clamp(0.0, 1.0);
            let lum = if noir_pour_1 { 255.0 * (1.0 - v) } else { 255.0 * v } as u8;
            for dy in 0..k {
                for dx in 0..k {
                    let (px, py) = ((ox + x * k + dx) as u32, (oy + y * k + dy) as u32);
                    if px < img.width() && py < img.height() {
                        img.put_pixel(px, py, Rgb([lum, lum, lum]));
                    }
                }
            }
        }
    }
}

/// Planche des papiers : ceux de Pelure et quelques papiers de TVPaint, à 1:1 et ×4, puis ce que
/// chacun laisse comme grain à trois appuis (`grain_factor`, grain 1).
fn planche_papiers(res: Option<&Path>, notes: &mut String) {
    let mut papiers: Vec<(String, GrayImage)> = vec![
        ("Pelure « Grain fin » (bruit 256, flou 0)".into(), procedural_paper(256, 0, 11)),
        ("Pelure « Grain gros » (bruit 256, flou 1)".into(), procedural_paper(256, 1, 29)),
    ];
    if let Some(res) = res {
        for name in ["Paper 33", "Paper 04", "cement_02", "noisy_009", "Noisy Hard"] {
            match crate::tvpbrush::find_paper(res, name) {
                Some(img) => {
                    let _ = writeln!(notes, "  papier TVPaint « {name} » : {}×{}", img.width, img.height);
                    // Avant l'audit : étiré de 0 à 1 ; depuis : réparti par rang (`add_paper`).
                    papiers.push((format!("TVPaint « {name} » ({}×{}), étiré (avant)", img.width, img.height), img.clone().normalized()));
                    papiers.push((format!("TVPaint « {name} », réparti par rang (après)"), img.equalized()));
                }
                None => {
                    let _ = writeln!(notes, "  papier TVPaint « {name} » introuvable");
                }
            }
        }
    }
    let (tw, th) = (128usize, 128usize);
    let cols = 6; // 1:1, ×4, grain à 20 %, 50 %, 90 %, (vide)
    let w = GAUCHE + 160 + cols * (tw + MARGE);
    let h = ENTETE + 24 + papiers.len() * (th + MARGE);
    let mut img = RgbImage::from_pixel(w as u32, h as u32, Rgb([255, 255, 255]));
    etiquette(&mut img, MARGE as f32, 4.0, "Papiers : relief à 1:1, gros plan ×4, puis le grain laissé à un appui de 20 %, 50 %, 90 % (grain 1)", 15.0);
    for (c, n) in ["relief 1:1", "relief ×4 (32 px)", "appui 20 %", "appui 50 %", "appui 90 %"].iter().enumerate() {
        etiquette(&mut img, (GAUCHE + 160 + c * (tw + MARGE)) as f32, (ENTETE + 2) as f32, n, 11.0);
    }
    for (i, (nom, p)) in papiers.iter().enumerate() {
        let y = ENTETE + 24 + i * (th + MARGE);
        etiquette(&mut img, MARGE as f32, (y + 2) as f32, nom, 10.0);
        let x0 = GAUCHE + 160;
        poser_gris(&mut img, p, (0, 0, tw, th), x0, y, 1, false);
        poser_gris(&mut img, p, (0, 0, 32, 32), x0 + (tw + MARGE), y, 4, false);
        for (k, pr) in [0.2f32, 0.5, 0.9].iter().enumerate() {
            let x = x0 + (2 + k) * (tw + MARGE);
            for yy in 0..th {
                for xx in 0..tw {
                    let hgt = p.get(xx % p.width, yy % p.height);
                    let f = grain_factor(hgt, *pr, 1.0);
                    let lum = (255.0 * (1.0 - f)) as u8;
                    img.put_pixel((x + xx) as u32, (y + yy) as u32, Rgb([lum, lum, lum]));
                }
            }
        }
    }
    save(&img, "10_papiers.png");
}

/// Planche des pointes : l'empreinte ronde de Pelure (dureté 0,85 et 1) et des pointes `.dip` de
/// TVPaint (première image, puis la bande des images d'une pointe animée).
fn planche_pointes(res: Option<&Path>, notes: &mut String) {
    let mut pointes: Vec<(String, GrayImage)> = Vec::new();
    for (h, nom) in [(0.85f32, "Pelure : empreinte ronde, dureté 0,85 (Pinceau rond)"), (1.0, "Pelure : empreinte ronde, dureté 1 (Crayon, Encre)")] {
        let n = 64usize;
        let r = 26.0f32;
        let inner = r * h;
        let values = (0..n * n)
            .map(|i| {
                let (x, y) = ((i % n) as f32 + 0.5 - n as f32 / 2.0, (i / n) as f32 + 0.5 - n as f32 / 2.0);
                crate::brush::coverage_soft((x * x + y * y).sqrt(), r, inner)
            })
            .collect();
        pointes.push((nom.into(), GrayImage::new(n, n, values)));
    }
    if let Some(res) = res {
        let choix = [
            ("Pencils_v1", "79ee5bfb-5d7c-4738-98bf-718ee774bb39", "TVPaint « 2B Pencil » (Pencils)"),
            ("Pencils_v1", "131b4b3a-068a-aa4e-8269-ce3d89ac1ca1", "TVPaint « HB Pencil » (Pencils, animée)"),
            ("Pencils_v1", "e19d19dd-42a0-4525-b88d-602e88753580", "TVPaint « Pencil 1 » (Pencils)"),
            ("Chalks_v1", "8ac64e30-40df-4f81-8085-b30ab3f9b84e", "TVPaint « Charcoal » (Chalks)"),
            ("Pens_v1", "fa882273-8006-6245-a3d6-421c913c769e", "TVPaint « Chinese Brush » (Pens)"),
            ("PaintBrushes_v1", "79244b59-0b3e-4d87-8640-8b1beaca4e7f", "TVPaint « Oil 1 » (PaintBrushes)"),
            ("PaintBrushes_v1", "02bd87e8-18cc-46a9-905f-7e8051c9a77f", "TVPaint « Watercolour 1 » (PaintBrushes)"),
        ];
        for (panel, uuid, nom) in choix {
            let path = res.join("custompanels").join(format!("{panel}.data")).join(format!("{uuid}.dip"));
            match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| crate::dip::read(&b)) {
                Ok(d) => {
                    let _ = writeln!(notes, "  pointe {nom} : {}×{}, {} image(s)", d.width, d.height, d.frames.len());
                    pointes.push((format!("{nom} — {}×{}, {} image(s)", d.width, d.height, d.frames.len()), crate::dip::tip_strip(&d)));
                }
                Err(e) => {
                    let _ = writeln!(notes, "  pointe {nom} : {e}");
                }
            }
        }
    }
    let row_h = 150usize;
    let w = GAUCHE + 300 + 900;
    let h = ENTETE + 10 + pointes.len() * (row_h + MARGE);
    let mut img = RgbImage::from_pixel(w as u32, h as u32, Rgb([255, 255, 255]));
    etiquette(&mut img, MARGE as f32, 4.0, "Pointes : ce qu'une empreinte dépose (noir = dépôt plein), à 1:1, bande des images jusqu'à 900 px", 15.0);
    for (i, (nom, p)) in pointes.iter().enumerate() {
        let y = ENTETE + 10 + i * (row_h + MARGE);
        etiquette(&mut img, MARGE as f32, (y + 2) as f32, nom, 10.0);
        let (fw, fh) = (p.width.min(900), p.height.min(row_h));
        poser_gris(&mut img, p, (0, 0, fw, fh), GAUCHE + 300, y, 1, true);
    }
    save(&img, "11_pointes.png");
}

// ------------------------------------------------------------------------------------ l'essai

/// Toutes les planches de l'audit.
#[test]
#[ignore]
fn planches_rendu_audit() {
    std::fs::create_dir_all(out_dir()).expect("dossier audit-rendu");
    let mut notes = String::new();
    let _ = writeln!(notes, "# Mesures de l'audit du rendu (rendu_audit_tests.rs)\n");
    let mut lib = BrushLibrary::builtin();
    lib.share();

    // 0. Toucher ancien contre nouveau : la preuve que le chantier « sensation » change peu l'image.
    for (nom, fichier, s) in [
        ("Crayon (barre) : toucher ancien / nouveau", "00_toucher_crayon.png", BrushSettings::pencil()),
        ("Pinceau rond : toucher ancien / nouveau", "00_toucher_pinceau-rond.png", BrushSettings::brush()),
    ] {
        let tex = lib.textures(&s);
        let (ancien, _) = rangee_bitmap("toucher ancien", &s, &tex, Toucher::ancien());
        let (nouveau, _) = rangee_bitmap("toucher nouveau", &s, &tex, Toucher::default());
        // Et l'ancien crayon crénelé, celui d'avant le chantier.
        let vieux = BrushSettings { size: 2.0, hardness: 1.0, smoothing: 0.0, aliased: true, pressure_opacity: false, min_opacity: 0.0, ..BrushSettings::brush() };
        let mut rangees = vec![ancien, nouveau];
        if fichier.contains("crayon") {
            let (r, _) = rangee_bitmap("crayon d'avant (crénelé)", &vieux, &lib.textures(&vieux), Toucher::ancien());
            rangees.push(r);
        }
        let img = planche(nom, "bitmap ; le chantier « sensation des brosses » n'a changé que la rangée du bas", &rangees);
        save(&img, fichier);
    }

    // 1. Les réglages livrés et les essais.
    let _ = writeln!(notes, "## Réglages livrés avec Pelure");
    for c in cas_pelure(&lib) {
        rendre_cas(&c, &lib, &mut notes);
    }
    let _ = writeln!(notes, "## Essais (briques existantes, autres valeurs)");
    for c in cas_essais(&lib) {
        rendre_cas(&c, &lib, &mut notes);
    }

    // 2. Papiers et pointes.
    let res = crate::tvpbrush::tvpaint_resources();
    let _ = writeln!(notes, "## Papiers et pointes");
    planche_papiers(res.as_deref(), &mut notes);
    planche_pointes(res.as_deref(), &mut notes);
    planche_bord(&lib);

    // 3. Les outils TVPaint d'Anthony et une sélection des panneaux livrés, puis Krita / MyPaint.
    let tvp = cas_tvpaint(&mut lib, &mut notes);
    let krita = cas_krita(&mut lib, &mut notes);
    lib.share();
    let _ = writeln!(notes, "## Rendus des imports");
    for c in tvp.iter().chain(&krita) {
        rendre_cas(c, &lib, &mut notes);
    }

    std::fs::write(out_dir().join("mesures.txt"), &notes).expect("mesures.txt");
    std::fs::write(
        out_dir().join("LISEZMOI.txt"),
        "Planches de l'audit du rendu des brosses (src/rendu_audit_tests.rs).\n\
         Chaque planche : quatre exercices en colonnes — A rampe de pression (appui de 2 % à 100 %, de gauche à droite),\n\
         B hachures (sept traits rapides, puis cinq croisés plus légers), C courbe en S (lente et légère aux bouts,\n\
         rapide et appuyée au milieu), D zone remplie par passages croisés (zigzag noir à 45 % d'appui, puis zigzag bleu\n\
         sur la moitié droite). Rangées : le rendu bitmap (calque bitmap) et le rendu vectoriel (calque vectoriel, au\n\
         lâcher) à 100 %, puis les mêmes en gros plan ×4 (fenêtre de 64×42 px : début léger de la rampe, cœur des\n\
         hachures, milieu du S, frontière des passages croisés). Papier blanc, encre presque noire.\n\
         00_ : toucher ancien / nouveau. 01–09 : réglages livrés. 10–12 : papiers, pointes, bord bitmap / vectoriel. 20– : essais.\n\
         30– : outils TVPaint d'Anthony importés. 40– : panneaux TVPaint 12 importés. 60– : Krita / MyPaint importés.\n\
         mesures.txt : réglages exacts et chiffres (alpha le long de la rampe, aplat du S, accumulation des croisés).\n",
    )
    .expect("LISEZMOI.txt");
    println!("{notes}");
}
