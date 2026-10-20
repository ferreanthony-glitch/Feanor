//! Rendu de la scène 3D par le processeur, pour l'envoi vers un calque de fond : un rasteriseur à
//! tampon de profondeur, ombré par la lumière de la scène, avec un sur-échantillonnage (bords
//! lisses) et des traits de contour (silhouettes, cassures) tracés dans l'image.
//!
//! Il tourne sur un autre fil (jamais sur celui de l'interface), par bandes de lignes réparties sur
//! les cœurs du processeur : chaque bande a son propre tampon, de la taille d'une bande, pas de
//! l'image. L'image rendue est celle que la caméra de la scène filme, à la taille du calque ; la
//! vue d'édition (carte graphique, `scene3dgpu.rs`) utilise la même `View` et les mêmes formules
//! d'ombrage, donc ce qu'on voit en éditant est ce qui sera dessiné.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::model3d::Model3d;
use crate::scene3d::{Light, RenderArea, RenderStyle, Scenes3d};
use crate::scene3dmath::{self as m, M4, V3, View};
use crate::scene3dmesh::{MeshLib, SolidMesh};

/// Un objet à rendre.
#[derive(Clone)]
pub struct Item {
    pub mesh: Arc<SolidMesh>,
    pub matrix: M4,
    /// Couleur de l'objet, de 0 à 1.
    pub color: [f32; 3],
    pub gray: bool,
    /// Numéro (non nul) : les traits de contour passent là où il change.
    pub id: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Opts {
    pub view: View,
    /// Taille de l'image, en pixels.
    pub size: [usize; 2],
    /// Pixel de l'image où la caméra regarde (le centre du cadre).
    pub center: [f32; 2],
    pub light: Light,
    pub style: RenderStyle,
    /// Tracer les traits de contour.
    pub lines: bool,
    /// Épaisseur des traits, en pixels de l'image.
    pub line_px: f32,
    /// Sur-échantillonnage par côté (1 à 4).
    pub ss: usize,
    /// Fond : rien (transparent) ou une couleur.
    pub paper: Option<[u8; 3]>,
    /// Fils de calcul (0 : tous les cœurs).
    pub threads: usize,
}

/// Couleur des traits.
const INK: [f32; 3] = [0.05, 0.05, 0.06];
/// Plan proche, en mètres.
pub const NEAR: f32 = 0.05;
/// Lignes de l'image par bande de travail.
const BAND: usize = 16;
/// Écart de profondeur relatif entre deux échantillons voisins à partir duquel on trace un trait.
const DEPTH_JUMP: f32 = 0.012;
/// Cosinus de l'angle de cassure à partir duquel on trace un trait entre deux faces voisines.
const CREASE_COS: f32 = 0.77;

/// Un triangle prêt à rasteriser : sommets (x et y en échantillons, 1/z), normales du monde
/// multipliées par 1/z (interpolation juste en perspective).
struct Tri {
    p: [[f32; 3]; 3],
    n: [V3; 3],
    color: [f32; 3],
    gray: bool,
    id: u32,
    y_range: (f32, f32),
}

pub fn threads() -> usize {
    std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 16)
}

/// La lumière d'un point de normale `n` (vers l'œil : normale déjà retournée) : ambiante plus
/// directionnelle. Les mêmes chiffres que dans le shader de la vue d'édition.
pub fn lighting(n: V3, light: &Light) -> f32 {
    light.ambient + light.intensity * m::dot(n, light.direction()).max(0.0)
}

/// Couleur d'un point selon le style.
pub fn shade(base: [f32; 3], gray: bool, style: RenderStyle, lit: f32) -> [f32; 3] {
    match style {
        // Aplats clairs, à peine ombrés : le dessin passe par-dessus.
        RenderStyle::Outline => {
            let v = (0.9 + 0.07 * (lit - 0.35).clamp(0.0, 1.0)).min(1.0);
            [v, v, v]
        }
        RenderStyle::Gray => {
            let y = base[0] * 0.299 + base[1] * 0.587 + base[2] * 0.114;
            [0, 1, 2].map(|_| (y * lit).clamp(0.0, 1.0))
        }
        RenderStyle::Color if gray => {
            let y = base[0] * 0.299 + base[1] * 0.587 + base[2] * 0.114;
            [0, 1, 2].map(|_| (y * lit).clamp(0.0, 1.0))
        }
        RenderStyle::Color => base.map(|c| (c * lit).clamp(0.0, 1.0)),
    }
}

/// Coupe le polygone (position dans le repère de l'œil, normale) au plan proche.
fn clip_near(poly: &[(V3, V3)], near: f32) -> Vec<(V3, V3)> {
    let mut out = Vec::with_capacity(poly.len() + 1);
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (ina, inb) = (a.0[2] >= near, b.0[2] >= near);
        if ina {
            out.push(a);
        }
        if ina != inb {
            let t = (near - a.0[2]) / (b.0[2] - a.0[2]);
            out.push((m::lerp(a.0, b.0, t), m::lerp(a.1, b.1, t)));
        }
    }
    out
}

fn build_triangles(items: &[Item], o: &Opts) -> Vec<Tri> {
    let s = o.view.px_per_unit();
    let ss = o.ss as f32;
    let mut tris = Vec::new();
    for item in items {
        let nm = m::normal_matrix(&item.matrix);
        let mesh = &item.mesh;
        // Sommets en repère de l'œil, normales en repère du monde.
        let cam: Vec<V3> = mesh.positions.iter().map(|p| o.view.to_view(m::point(&item.matrix, *p))).collect();
        let nrm: Vec<V3> = mesh.normals.iter().map(|n| m::normalize(m::dir(&nm, *n))).collect();
        let world_view = |i: usize| m::point(&item.matrix, mesh.positions[i]);
        for t in mesh.indices.chunks_exact(3) {
            let idx = [t[0] as usize, t[1] as usize, t[2] as usize];
            if idx.iter().all(|&i| cam[i][2] < NEAR) {
                continue;
            }
            // Retourne les normales de la face si elle est vue par son dos (ombrage des deux côtés).
            let center = m::scale(m::add(m::add(world_view(idx[0]), world_view(idx[1])), world_view(idx[2])), 1.0 / 3.0);
            let avg = m::add(m::add(nrm[idx[0]], nrm[idx[1]]), nrm[idx[2]]);
            let flip = if m::dot(avg, m::sub(o.view.eye, center)) < 0.0 { -1.0 } else { 1.0 };
            let poly: Vec<(V3, V3)> = idx.iter().map(|&i| (cam[i], m::scale(nrm[i], flip))).collect();
            let poly = if poly.iter().all(|v| v.0[2] >= NEAR) { poly } else { clip_near(&poly, NEAR) };
            if poly.len() < 3 {
                continue;
            }
            let v: Vec<([f32; 3], V3)> = poly
                .iter()
                .map(|(c, n)| {
                    let inv = 1.0 / c[2];
                    ([(o.center[0] + c[0] * inv * s) * ss, (o.center[1] - c[1] * inv * s) * ss, inv], m::scale(*n, inv))
                })
                .collect();
            if v.iter().any(|p| !(p.0[0].is_finite() && p.0[1].is_finite())) {
                continue;
            }
            for k in 1..v.len() - 1 {
                let p = [v[0].0, v[k].0, v[k + 1].0];
                let y_range = (p[0][1].min(p[1][1]).min(p[2][1]), p[0][1].max(p[1][1]).max(p[2][1]));
                tris.push(Tri { p, n: [v[0].1, v[k].1, v[k + 1].1], color: item.color, gray: item.gray, id: item.id, y_range });
            }
        }
    }
    tris
}

/// Tampons d'une bande, à la résolution des échantillons.
struct Band {
    w: usize,
    /// Première ligne d'échantillons de la bande dans l'image.
    y0: usize,
    rows: usize,
    invz: Vec<f32>,
    id: Vec<u32>,
    color: Vec<[f32; 3]>,
    normal: Vec<V3>,
}

impl Band {
    fn new(w: usize, y0: usize, rows: usize) -> Band {
        let n = w * rows;
        Band { w, y0, rows, invz: vec![0.0; n], id: vec![0; n], color: vec![[0.0; 3]; n], normal: vec![[0.0; 3]; n] }
    }
}

fn raster_triangle(t: &Tri, band: &mut Band, o: &Opts) {
    let p = &t.p;
    let (x0, x1) = (p[0][0].min(p[1][0]).min(p[2][0]), p[0][0].max(p[1][0]).max(p[2][0]));
    let (ya, yb) = t.y_range;
    let (by0, by1) = (band.y0 as f32, (band.y0 + band.rows) as f32);
    if x1 < 0.0 || x0 >= band.w as f32 || yb < by0 || ya >= by1 {
        return;
    }
    let area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
    if area.abs() < 1e-9 {
        return;
    }
    let (xs, xe) = ((x0.max(0.0).floor() as usize), (x1.ceil() as usize).min(band.w - 1));
    let (ys, ye) = ((ya.max(by0).floor() as usize).max(band.y0), (yb.ceil() as usize).min(band.y0 + band.rows - 1));
    let (gray, base) = (t.gray, t.color);
    for y in ys..=ye {
        let py = y as f32 + 0.5;
        let row = (y - band.y0) * band.w;
        for x in xs..=xe {
            let px = x as f32 + 0.5;
            let w0 = ((p[1][0] - px) * (p[2][1] - py) - (p[2][0] - px) * (p[1][1] - py)) / area;
            let w1 = ((p[2][0] - px) * (p[0][1] - py) - (p[0][0] - px) * (p[2][1] - py)) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let invz = w0 * p[0][2] + w1 * p[1][2] + w2 * p[2][2];
            let at = row + x;
            if invz <= band.invz[at] {
                continue;
            }
            let n = m::normalize([0, 1, 2].map(|k| (w0 * t.n[0][k] + w1 * t.n[1][k] + w2 * t.n[2][k]) / invz));
            band.invz[at] = invz;
            band.id[at] = t.id;
            band.normal[at] = n;
            band.color[at] = shade(base, gray, o.style, lighting(n, &o.light));
        }
    }
}

/// Marque les échantillons où un trait doit passer : l'objet change, la profondeur saute, ou la
/// surface casse. Puis épaissit.
fn line_mask(band: &Band, o: &Opts, core: std::ops::Range<usize>) -> Vec<bool> {
    let (w, rows) = (band.w, band.rows);
    let mut edge = vec![false; w * rows];
    for ry in 0..rows {
        for x in 0..w {
            let at = ry * w + x;
            let (ia, da, na) = (band.id[at], band.invz[at], band.normal[at]);
            let mut mark = false;
            for (dx, dy) in [(1isize, 0isize), (0, 1)] {
                let (nx, ny) = (x as isize + dx, ry as isize + dy);
                if nx >= w as isize || ny >= rows as isize {
                    continue;
                }
                let bt = ny as usize * w + nx as usize;
                let (ib, db) = (band.id[bt], band.invz[bt]);
                mark |= if ia != ib {
                    true
                } else if ia == 0 {
                    false
                } else {
                    (da - db).abs() > DEPTH_JUMP * da.max(db) || m::dot(na, band.normal[bt]) < CREASE_COS
                };
                if mark {
                    break;
                }
            }
            edge[at] = mark;
        }
    }
    // Épaisseur : on dilate d'autant d'échantillons que le trait demande.
    let reach = (((o.line_px * o.ss as f32) - 1.0) / 2.0).ceil().max(0.0) as isize;
    if reach == 0 {
        return edge;
    }
    let mut wide = vec![false; w * rows];
    for ry in 0..rows {
        for x in 0..w {
            let lo = (x as isize - reach).max(0) as usize;
            let hi = ((x as isize + reach) as usize).min(w - 1);
            wide[ry * w + x] = edge[ry * w + lo..=ry * w + hi].iter().any(|e| *e);
        }
    }
    let mut out = vec![false; w * rows];
    for ry in core {
        let lo = (ry as isize - reach).max(0) as usize;
        let hi = ((ry as isize + reach) as usize).min(rows - 1);
        for x in 0..w {
            out[ry * w + x] = (lo..=hi).any(|r| wide[r * w + x]);
        }
    }
    out
}

/// Rend `items` : octets RGBA (non prémultipliés) de l'image `o.size`.
pub fn render(items: &[Item], o: &Opts) -> Vec<u8> {
    let [w, h] = o.size;
    let mut out = vec![0u8; w * h * 4];
    if w == 0 || h == 0 {
        return out;
    }
    let ss = o.ss.clamp(1, 4);
    let o = &Opts { ss, ..*o };
    let tris = build_triangles(items, o);
    let reach = if o.lines { (((o.line_px * ss as f32) - 1.0) / 2.0).ceil().max(0.0) as usize + 2 } else { 0 };
    // Les triangles de chaque bande de lignes (marges de contour comprises).
    let bands = h.div_ceil(BAND);
    let mut bins: Vec<Vec<u32>> = vec![Vec::new(); bands];
    for (i, t) in tris.iter().enumerate() {
        let (ya, yb) = (t.y_range.0 - reach as f32 - 1.0, t.y_range.1 + reach as f32 + 1.0);
        if yb < 0.0 || ya >= (h * ss) as f32 {
            continue;
        }
        let first = (ya.max(0.0) as usize / ss / BAND).min(bands - 1);
        let last = ((yb.min((h * ss) as f32 - 1.0)) as usize / ss / BAND).min(bands - 1);
        for bin in &mut bins[first..=last] {
            bin.push(i as u32);
        }
    }
    let work = Mutex::new(out.chunks_mut(BAND * w * 4).enumerate());
    std::thread::scope(|sc| {
        for _ in 0..(if o.threads == 0 { threads() } else { o.threads }).min(bands) {
            sc.spawn(|| loop {
                let next = work.lock().ok().and_then(|mut it| it.next());
                let Some((band_index, dst)) = next else { break };
                let (py0, rows_px) = (band_index * BAND, dst.len() / (w * 4));
                // Lignes d'échantillons de la bande, avec la marge des traits en haut et en bas.
                let core_lo = py0 * ss;
                let core_hi = (py0 + rows_px) * ss;
                let ext_lo = core_lo.saturating_sub(reach);
                let ext_hi = (core_hi + reach).min(h * ss);
                let mut band = Band::new(w * ss, ext_lo, ext_hi - ext_lo);
                for &ti in &bins[band_index] {
                    raster_triangle(&tris[ti as usize], &mut band, o);
                }
                let mask = o.lines.then(|| line_mask(&band, o, (core_lo - ext_lo)..(core_hi - ext_lo)));
                for ry in 0..rows_px {
                    for x in 0..w {
                        let (mut sum, mut cover) = ([0.0f32; 3], 0.0f32);
                        for sy in 0..ss {
                            for sx in 0..ss {
                                let at = (ry * ss + sy + core_lo - ext_lo) * band.w + x * ss + sx;
                                if mask.as_ref().is_some_and(|mk| mk[at]) {
                                    sum = [0, 1, 2].map(|k| sum[k] + INK[k]);
                                    cover += 1.0;
                                } else if band.id[at] != 0 {
                                    sum = [0, 1, 2].map(|k| sum[k] + band.color[at][k]);
                                    cover += 1.0;
                                }
                            }
                        }
                        let n = (ss * ss) as f32;
                        let a = cover / n;
                        let rgb = if cover > 0.0 { sum.map(|c| c / cover) } else { [0.0; 3] };
                        let px = &mut dst[(ry * w + x) * 4..(ry * w + x) * 4 + 4];
                        let to8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                        match o.paper {
                            Some(p) => {
                                for k in 0..3 {
                                    px[k] = to8(rgb[k] * a + p[k] as f32 / 255.0 * (1.0 - a));
                                }
                                px[3] = 255;
                            }
                            None => {
                                px[..3].copy_from_slice(&rgb.map(to8));
                                px[3] = to8(a);
                            }
                        }
                    }
                }
            });
        }
    });
    drop(work);
    out
}

// ------------------------------------------------------------------- envoi vers un calque

/// Les objets du plan `panel` (de la séquence `seq`), décor compris, prêts à rendre.
pub fn items_of(scenes: &Scenes3d, panel: u64, seq: u64, models: &HashMap<u64, Arc<Model3d>>, lib: &mut MeshLib) -> Vec<Item> {
    scenes
        .visible_objects(panel, seq)
        .into_iter()
        .filter(|(o, _)| !o.hidden)
        .enumerate()
        .map(|(i, (o, _))| {
            let (_, mesh) = lib.get(&o.shape, models);
            Item { mesh, matrix: o.xf.matrix(), color: o.color.map(|c| c as f32 / 255.0), gray: o.gray, id: i as u32 + 1 }
        })
        .collect()
}

/// Une image rendue et sa place dans la feuille.
pub struct Plate {
    pub rgba: Vec<u8>,
    pub size: [usize; 2],
    /// Coin haut-gauche de l'image dans la feuille.
    pub origin: [usize; 2],
}

/// Coin haut-gauche du cadre dans la feuille (`Document::frame_rect`).
pub fn frame_origin(stage: [usize; 2], frame: [usize; 2]) -> [usize; 2] {
    [stage[0].saturating_sub(frame[0]) / 2, stage[1].saturating_sub(frame[1]) / 2]
}

/// Rend la scène du plan par sa caméra du plateau. `stage` : la feuille ; `frame` : le cadre (la
/// taille de l'image finale). Selon la zone choisie, l'image est le cadre seul ou la feuille entière ;
/// dans les deux cas un pixel de l'image est le pixel de la feuille à la même place, et les 36 mm de
/// la pellicule correspondent à la largeur du cadre.
pub fn render_plate(
    scenes: &Scenes3d,
    panel: u64,
    seq: u64,
    models: &HashMap<u64, Arc<Model3d>>,
    stage: [usize; 2],
    frame: [usize; 2],
    ss: usize,
    threads: usize,
) -> Plate {
    let plan = scenes.plan(panel).cloned().unwrap_or_default();
    // Figé par l'Espace 3D : la feuille vue de l'œil de la caméra du plan, alignée sur la feuille.
    if plan.sheet_aligned {
        return render_plate_eye(scenes, panel, seq, models, stage, frame, plan.cam.pos, ss, threads);
    }
    render_plate_with(scenes, panel, seq, models, stage, frame, plan.cam, plan.render.area, None, ss, threads)
}

/// Les objets du plan vus de l'œil `eye` (mètres) de la caméra du plan : regard droit devant, sans zoom ni rotation,
/// toute la feuille. C'est la feuille telle que l'œil la voit (le multiplan), sur laquelle la caméra du plan ajoute
/// ensuite son zoom, sa rotation, sa bascule et son panoramique, comme pour tout calque (`espace3dobjets.rs`).
pub fn render_plate_eye(
    scenes: &Scenes3d,
    panel: u64,
    seq: u64,
    models: &HashMap<u64, Arc<Model3d>>,
    stage: [usize; 2],
    frame: [usize; 2],
    eye: V3,
    ss: usize,
    threads: usize,
) -> Plate {
    let cam = crate::scene3d::SceneCam { pos: eye, pan: 0.0, tilt: 0.0, roll: 0.0, focal: crate::camera::REF_FOCAL };
    // L'image est celle de la feuille : le point qu'on voit droit devant l'œil est celui de la feuille sous l'œil
    // (x, y de la caméra), pas le centre de la feuille.
    let k = crate::scene3d::SceneCam::meters_per_px(frame[0]);
    let center = [stage[0] as f32 / 2.0 + eye[0] / k, stage[1] as f32 / 2.0 - eye[1] / k];
    render_plate_with(scenes, panel, seq, models, stage, frame, cam, RenderArea::Sheet, Some(center), ss, threads)
}

/// Le rendu des objets du plan par la caméra `cam`, sur `area` (lumière et style : ceux de la scène du plan).
pub fn render_plate_with(
    scenes: &Scenes3d,
    panel: u64,
    seq: u64,
    models: &HashMap<u64, Arc<Model3d>>,
    stage: [usize; 2],
    frame: [usize; 2],
    cam: crate::scene3d::SceneCam,
    area: RenderArea,
    center: Option<[f32; 2]>,
    ss: usize,
    threads: usize,
) -> Plate {
    let plan = scenes.plan(panel).cloned().unwrap_or_default();
    let mut lib = MeshLib::default();
    let items = items_of(scenes, panel, seq, models, &mut lib);
    let at = frame_origin(stage, frame);
    let (size, origin) = match area {
        RenderArea::Frame => (frame, at),
        RenderArea::Sheet => (stage, [0, 0]),
    };
    // Où la caméra regarde, dans l'image : le centre du cadre (ou le point demandé).
    let center = center.unwrap_or([(at[0] - origin[0]) as f32 + frame[0] as f32 / 2.0, (at[1] - origin[1]) as f32 + frame[1] as f32 / 2.0]);
    let opts = Opts {
        view: cam.view(frame[0].max(1) as f32),
        size,
        center,
        light: plan.light,
        style: plan.render.style,
        lines: plan.render.has_lines(),
        line_px: plan.render.line_px,
        ss,
        paper: None,
        threads,
    };
    Plate { rgba: render(&items, &opts), size, origin }
}

/// Le dessin de la feuille (`stage`) qui contient l'image : transparent partout ailleurs.
pub fn plate_raster(plate: &Plate, stage: [usize; 2]) -> crate::raster::Raster {
    if plate.size == stage && plate.origin == [0, 0] {
        return crate::raster::Raster::from_rgba(stage[0], stage[1], &plate.rgba);
    }
    let mut full = vec![0u8; stage[0] * stage[1] * 4];
    for y in 0..plate.size[1] {
        let dy = y + plate.origin[1];
        if dy >= stage[1] {
            break;
        }
        let w = plate.size[0].min(stage[0].saturating_sub(plate.origin[0]));
        let (src, dst) = ((y * plate.size[0]) * 4, (dy * stage[0] + plate.origin[0]) * 4);
        full[dst..dst + w * 4].copy_from_slice(&plate.rgba[src..src + w * 4]);
    }
    crate::raster::Raster::from_rgba(stage[0], stage[1], &full)
}
