//! Zones fermées d'un trait (« coloriage par zones », `coloriage.rs`).
//!
//! `segment` découpe un dessin au trait en zones fermées : chaque zone a un numéro, une aire, un
//! centre, une boîte, un périmètre, ses voisines et un descripteur de forme (invariants de Hu
//! d'ordre 2). Les trous du trait jusqu'à `2 × trous` pixels sont fermés, comme pour le fond sous le
//! trait (`fond.rs`), sans que les petites zones disparaissent :
//!
//! 1. le **cœur** d'une zone est ce qui reste à plus de `trous` pixels du trait (les cœurs sont
//!    séparés par les trous fermés) ;
//! 2. chaque cœur reprend son bord jusqu'au trait (la bande de fermeture), les cœurs voisins se
//!    partageant un trou ;
//! 3. une zone plus petite que la fermeture (une pupille, une dent) n'a pas de cœur : elle reste une
//!    zone à elle si elle est bien fermée ;
//! 4. chaque zone passe ensuite de `debord` pixels **sous** le trait, pour qu'un aplat posé dessous
//!    ne laisse aucun jour. Le dehors (ce qui touche le bord de la boîte du trait) ne passe pas
//!    sous le trait.
//!
//! Le bord de l'image ferme les zones, comme pour le fond. Le résultat se garde par version du
//! dessin (`ZoneCache`).

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use crate::fond::{edt, LINE_ALPHA};
use crate::raster::{Raster, TILE};

/// Aire (en pixels) en dessous de laquelle une zone est du bruit de tracé : elle disparaît.
pub const MIN_AREA: u32 = 10;
/// Épaisseur de trait (en pixels) au-delà de laquelle deux zones ne sont plus voisines.
const FACING: usize = 14;
/// Aire (en pixels) en dessous de laquelle un cœur est un îlot sans importance.
const MIN_CORE: u32 = 8;
/// Au plus de zones par dessin (les identifiants tiennent sur 16 bits).
pub const MAX_ZONES: usize = 60_000;

/// Réglages de la découpe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Reglages {
    /// Les trous du trait jusqu'à `2 × trous` pixels sont fermés.
    pub trous: u8,
    /// Pixels de zone sous le trait.
    pub debord: u8,
}

impl Default for Reglages {
    fn default() -> Self {
        Reglages { trous: crate::fond::TROUS, debord: crate::fond::DEBORD }
    }
}

/// Une zone fermée d'un dessin.
#[derive(Clone, Debug)]
pub struct Zone {
    /// Numéro (à partir de 1) ; la zone est la `id - 1`-ième de `Segmentation::zones`.
    pub id: u16,
    /// Pixels de la zone, sous le trait compris.
    pub area: u32,
    /// Centre, en pixels de la feuille.
    pub centroid: (f32, f32),
    /// Boîte (x0, y0, x1, y1), bornes comprises, en pixels de la feuille.
    pub bbox: [u32; 4],
    /// Longueur du bord (côtés de pixels). Descripteur gardé pour le conseiller de couleur et les
    /// essais ; l'appariement lui préfère les invariants de Hu (le périmètre en côtés de pixels
    /// change avec la rotation).
    #[allow(dead_code)]
    pub perimeter: u32,
    /// Zones voisines et longueur du bord commun, la plus longue d'abord (le dehors y est).
    pub neighbours: Vec<(u16, u32)>,
    /// Invariants de Hu d'ordre 2 (identiques par déplacement, rotation et changement d'échelle).
    pub hu: [f64; 2],
    /// C'est le dehors : ce qui touche le bord de la boîte du trait. Jamais colorié.
    pub outside: bool,
}

impl Zone {
    /// Rayon du disque de même aire.
    pub fn radius(&self) -> f32 {
        (self.area as f32 / std::f32::consts::PI).sqrt()
    }
}

/// Le dessin découpé en zones.
pub struct Segmentation {
    /// Taille de la feuille.
    pub width: usize,
    pub height: usize,
    /// La boîte de travail (le trait, agrandi) : coin et taille.
    pub x0: usize,
    pub y0: usize,
    pub bw: usize,
    pub bh: usize,
    /// Numéro de zone de chaque pixel de la boîte (0 : aucune).
    pub lab: Vec<u16>,
    pub zones: Vec<Zone>,
}

impl Segmentation {
    /// Un dessin sans trait : pas de zone.
    fn empty(width: usize, height: usize) -> Segmentation {
        Segmentation { width, height, x0: 0, y0: 0, bw: 0, bh: 0, lab: Vec::new(), zones: Vec::new() }
    }

    /// Le numéro de la zone du pixel (`x`, `y`) de la feuille (0 : aucune, hors de la boîte aussi).
    pub fn label_at(&self, x: i64, y: i64) -> u16 {
        let (lx, ly) = (x - self.x0 as i64, y - self.y0 as i64);
        if lx < 0 || ly < 0 || lx >= self.bw as i64 || ly >= self.bh as i64 {
            return 0;
        }
        self.lab[ly as usize * self.bw + lx as usize]
    }

    pub fn zone(&self, id: u16) -> Option<&Zone> {
        (id as usize).checked_sub(1).and_then(|i| self.zones.get(i))
    }

    /// La zone du pixel (`x`, `y`) de la feuille.
    #[cfg(test)]
    pub fn zone_at(&self, x: i64, y: i64) -> Option<&Zone> {
        self.zone(self.label_at(x, y))
    }

    /// Les zones à colorier (le dehors exclu).
    pub fn inner(&self) -> impl Iterator<Item = &Zone> {
        self.zones.iter().filter(|z| !z.outside)
    }

    /// Octets pris en mémoire (pour le cache).
    pub fn bytes(&self) -> usize {
        self.lab.len() * 2 + self.zones.iter().map(|z| 96 + z.neighbours.len() * 8).sum::<usize>()
    }

    /// Le masque de la zone `id` dans sa boîte agrandie de `margin` pixels : (x0, y0, largeur,
    /// hauteur, masque).
    pub fn mask_of(&self, id: u16, margin: usize) -> Option<(usize, usize, usize, usize, Vec<bool>)> {
        let z = self.zone(id)?;
        let x0 = (z.bbox[0] as usize).saturating_sub(margin);
        let y0 = (z.bbox[1] as usize).saturating_sub(margin);
        let x1 = z.bbox[2] as usize + margin;
        let y1 = z.bbox[3] as usize + margin;
        let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
        let mut mask = vec![false; w * h];
        for y in 0..h {
            for x in 0..w {
                mask[y * w + x] = self.label_at((x0 + x) as i64, (y0 + y) as i64) == id;
            }
        }
        Some((x0, y0, w, h, mask))
    }
}

/// Composantes connexes (4 voisins) de `mask`, dans une grille `w × h` : les numéros de 1 à n par
/// pixel (0 hors du masque) et n.
fn components(mask: &[bool], w: usize, h: usize) -> (Vec<u32>, u32) {
    let mut lab = vec![0u32; w * h];
    let mut parent: Vec<u32> = vec![0];
    fn find(parent: &mut [u32], mut a: u32) -> u32 {
        while parent[a as usize] != a {
            parent[a as usize] = parent[parent[a as usize] as usize];
            a = parent[a as usize];
        }
        a
    }
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if !mask[i] {
                continue;
            }
            let left = if x > 0 { lab[i - 1] } else { 0 };
            let up = if y > 0 { lab[i - w] } else { 0 };
            lab[i] = match (left, up) {
                (0, 0) => {
                    let id = parent.len() as u32;
                    parent.push(id);
                    id
                }
                (a, 0) | (0, a) => a,
                (a, b) => {
                    let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
                    if ra != rb {
                        parent[ra.max(rb) as usize] = ra.min(rb);
                    }
                    ra.min(rb)
                }
            };
        }
    }
    // Numéros compacts.
    let mut compact = vec![0u32; parent.len()];
    let mut n = 0u32;
    for a in 1..parent.len() as u32 {
        let r = find(&mut parent, a);
        if compact[r as usize] == 0 {
            n += 1;
            compact[r as usize] = n;
        }
        compact[a as usize] = compact[r as usize];
    }
    for l in lab.iter_mut() {
        if *l != 0 {
            *l = compact[*l as usize];
        }
    }
    (lab, n)
}

/// Le rectangle des pixels du trait de `line` : (x0, y0, x1, y1).
fn line_bounds(line: &Raster) -> Option<(usize, usize, usize, usize)> {
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
    for (idx, t) in line.tiles.iter().enumerate() {
        let Some(t) = t else { continue };
        let (ox, oy, tw, th) = line.tile_bounds(idx);
        for y in 0..th {
            for x in 0..tw {
                if t[y * TILE + x].a() >= LINE_ALPHA {
                    x0 = x0.min(ox + x);
                    y0 = y0.min(oy + y);
                    x1 = x1.max(ox + x);
                    y1 = y1.max(oy + y);
                }
            }
        }
    }
    (x0 != usize::MAX).then_some((x0, y0, x1, y1))
}

/// Découpe le dessin au trait `line` en zones fermées.
pub fn segment(line: &Raster, r: &Reglages) -> Segmentation {
    let (w, h) = (line.width, line.height);
    let Some((x0, y0, x1, y1)) = line_bounds(line) else { return Segmentation::empty(w, h) };
    let trous = r.trous as usize;
    let pad = trous + 3;
    let (bx0, by0) = (x0.saturating_sub(pad), y0.saturating_sub(pad));
    let (bx1, by1) = ((x1 + pad).min(w - 1), (y1 + pad).min(h - 1));
    let (bw, bh) = (bx1 - bx0 + 1, by1 - by0 + 1);
    // Les côtés de la boîte qui ne sont pas le bord de l'image : par là, le dehors entoure le trait.
    let open = [x0 >= pad, y0 >= pad, x1 + pad < w, y1 + pad < h];
    // Le trait dans la boîte, tuile par tuile.
    let mut solid = vec![false; bw * bh];
    for (idx, t) in line.tiles.iter().enumerate() {
        let Some(t) = t else { continue };
        let (ox, oy, tw, th) = line.tile_bounds(idx);
        if ox > bx1 || oy > by1 || ox + tw <= bx0 || oy + th <= by0 {
            continue;
        }
        for y in 0..th {
            let gy = oy + y;
            if gy < by0 || gy > by1 {
                continue;
            }
            for x in 0..tw {
                let gx = ox + x;
                if gx >= bx0 && gx <= bx1 && t[y * TILE + x].a() >= LINE_ALPHA {
                    solid[(gy - by0) * bw + gx - bx0] = true;
                }
            }
        }
    }
    // Le cœur : à plus de `trous` pixels du trait.
    let dist = edt(&solid, bw, bh);
    let limit = (trous * trous) as f64;
    let core: Vec<bool> = solid.iter().zip(&dist).map(|(&s, &d)| !s && d > limit).collect();
    drop(dist);
    let (mut clab, n_core) = components(&core, bw, bh);
    drop(core);
    // Les îlots de quelques pixels (au plus étroit d'un trou fermé, deux pointes de cœur se
    // frôlent) ne sont pas des zones : leurs pixels sont repris par les cœurs voisins. Une vraie
    // petite zone fermée n'y perd rien (étape 3).
    let mut core_size = vec![0u32; n_core as usize + 1];
    clab.iter().for_each(|&l| core_size[l as usize] += 1);
    for l in clab.iter_mut() {
        if *l != 0 && core_size[*l as usize] < MIN_CORE {
            *l = 0;
        }
    }
    // Le dehors : les cœurs qui touchent un côté ouvert de la boîte.
    let mut outside_core = vec![false; n_core as usize + 1];
    let mut touch = |x: usize, y: usize| {
        let l = clab[y * bw + x];
        if l != 0 {
            outside_core[l as usize] = true;
        }
    };
    if open[0] {
        (0..bh).for_each(|y| touch(0, y));
    }
    if open[1] {
        (0..bw).for_each(|x| touch(x, 0));
    }
    if open[2] {
        (0..bh).for_each(|y| touch(bw - 1, y));
    }
    if open[3] {
        (0..bw).for_each(|x| touch(x, bh - 1));
    }
    // Étiquettes provisoires (u32) : les cœurs, le dehors réuni sous un seul numéro.
    let outside_label = (1..=n_core).find(|&l| outside_core[l as usize]);
    let mut lab: Vec<u32> = clab;
    if let Some(first) = outside_label {
        for l in lab.iter_mut() {
            if *l != 0 && outside_core[*l as usize] {
                *l = first;
            }
        }
    }
    let mut next_label = n_core + 1;
    // Étape 2 : chaque cœur reprend son bord, par les pixels qui ne sont pas du trait.
    grow(&mut lab, &solid, bw, bh, false, usize::MAX, None);
    // Étape 3 : les zones sans cœur (plus petites que la fermeture).
    let rest: Vec<bool> = lab.iter().zip(&solid).map(|(&l, &s)| l == 0 && !s).collect();
    if rest.iter().any(|&b| b) {
        let (rlab, n_rest) = components(&rest, bw, bh);
        let mut sizes = vec![0u32; n_rest as usize + 1];
        rlab.iter().for_each(|&l| sizes[l as usize] += 1);
        let mut map = vec![0u32; n_rest as usize + 1];
        for k in 1..=n_rest {
            if sizes[k as usize] >= MIN_AREA {
                map[k as usize] = next_label;
                next_label += 1;
            }
        }
        for (i, &l) in rlab.iter().enumerate() {
            if l != 0 && map[l as usize] != 0 {
                lab[i] = map[l as usize];
            }
        }
    }
    // Étape 4 : sous le trait, sauf le dehors.
    grow(&mut lab, &solid, bw, bh, true, r.debord as usize, outside_label);
    // Aires, puis suppression du bruit et numéros compacts (le dehors compte comme une zone).
    let mut area = vec![0u32; next_label as usize + 1];
    lab.iter().for_each(|&l| area[l as usize] += 1);
    let mut map = vec![0u16; next_label as usize + 1];
    let mut count = 0usize;
    for l in 1..next_label {
        let is_outside = Some(l) == outside_label;
        if (is_outside || area[l as usize] >= MIN_AREA) && count < MAX_ZONES {
            count += 1;
            map[l as usize] = count as u16;
        }
    }
    let outside_id = outside_label.map(|l| map[l as usize]).filter(|&i| i != 0);
    let lab16: Vec<u16> = lab.iter().map(|&l| map[l as usize]).collect();
    drop(lab);
    let mut seg = Segmentation { width: w, height: h, x0: bx0, y0: by0, bw, bh, lab: lab16, zones: Vec::new() };
    describe(&mut seg, count, outside_id);
    seg
}

/// Fait grandir les zones déjà numérotées dans les pixels sans numéro : par les pixels qui ne sont
/// pas du trait (`under` faux, sans limite), ou sous le trait (`under` vrai, `steps` pixels au plus,
/// sans partir de `skip`).
fn grow(lab: &mut [u32], solid: &[bool], bw: usize, bh: usize, under: bool, steps: usize, skip: Option<u32>) {
    let free = |lab: &[u32], q: usize| lab[q] == 0 && (solid[q] == under);
    let neighbours = |p: usize| {
        let (x, y) = (p % bw, p / bw);
        [(x > 0).then(|| p - 1), (x + 1 < bw).then(|| p + 1), (y > 0).then(|| p - bw), (y + 1 < bh).then(|| p + bw)]
    };
    let mut frontier: Vec<u32> = Vec::new();
    for p in 0..bw * bh {
        let l = lab[p];
        if l == 0 || Some(l) == skip {
            continue;
        }
        if neighbours(p).into_iter().flatten().any(|q| free(lab, q)) {
            frontier.push(p as u32);
        }
    }
    if under {
        for _ in 0..steps {
            let mut next = Vec::with_capacity(frontier.len());
            for &p in &frontier {
                let l = lab[p as usize];
                for q in neighbours(p as usize).into_iter().flatten() {
                    if free(lab, q) {
                        lab[q] = l;
                        next.push(q as u32);
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            frontier = next;
        }
    } else {
        // Une file : le plus proche d'abord, les cœurs voisins se partagent un trou à égale distance.
        let mut queue: VecDeque<u32> = frontier.into_iter().collect();
        while let Some(p) = queue.pop_front() {
            let l = lab[p as usize];
            for q in neighbours(p as usize).into_iter().flatten() {
                if free(lab, q) {
                    lab[q] = l;
                    queue.push_back(q as u32);
                }
            }
        }
    }
}

/// Aire, centre, boîte, périmètre, voisines et forme de chaque zone.
fn describe(seg: &mut Segmentation, count: usize, outside: Option<u16>) {
    #[derive(Clone, Default)]
    struct Acc {
        n: f64,
        sx: f64,
        sy: f64,
        sxx: f64,
        sxy: f64,
        syy: f64,
        bbox: [u32; 4],
        perimeter: u32,
    }
    let (bw, bh) = (seg.bw, seg.bh);
    let mut acc = vec![Acc { bbox: [u32::MAX, u32::MAX, 0, 0], ..Default::default() }; count + 1];
    let mut shared: HashMap<u32, u32> = HashMap::new();
    // Somme de 1..k de k² (pour les sommes sur une plage de pixels d'une ligne).
    let s2 = |k: f64| k * (k + 1.0) * (2.0 * k + 1.0) / 6.0;
    for y in 0..bh {
        let row = &seg.lab[y * bw..(y + 1) * bw];
        let mut x = 0;
        while x < bw {
            let l = row[x];
            let mut e = x;
            while e + 1 < bw && row[e + 1] == l {
                e += 1;
            }
            if l != 0 {
                let a = &mut acc[l as usize];
                let n = (e - x + 1) as f64;
                let sum_x = n * (x + e) as f64 / 2.0;
                let sum_xx = s2(e as f64) - if x > 0 { s2(x as f64 - 1.0) } else { 0.0 };
                a.n += n;
                a.sx += sum_x;
                a.sy += y as f64 * n;
                a.sxx += sum_xx;
                a.sxy += y as f64 * sum_x;
                a.syy += (y * y) as f64 * n;
                let (gx0, gx1, gy) = ((seg.x0 + x) as u32, (seg.x0 + e) as u32, (seg.y0 + y) as u32);
                a.bbox[0] = a.bbox[0].min(gx0);
                a.bbox[1] = a.bbox[1].min(gy);
                a.bbox[2] = a.bbox[2].max(gx1);
                a.bbox[3] = a.bbox[3].max(gy);
            }
            x = e + 1;
        }
        // Les bords : à droite et en dessous (et ceux de la boîte, côté boîte seulement).
        for x in 0..bw {
            let l = row[x];
            let right = if x + 1 < bw { row[x + 1] } else { 0 };
            let below = if y + 1 < bh { seg.lab[(y + 1) * bw + x] } else { 0 };
            for (m, edge) in [(right, x + 1 < bw), (below, y + 1 < bh)] {
                if m == l {
                    continue;
                }
                if l != 0 {
                    acc[l as usize].perimeter += 1;
                }
                if m != 0 && edge {
                    acc[m as usize].perimeter += 1;
                }
            }
        }
    }
    // Les voisines : deux zones se font face de part et d'autre d'un trait (ou d'un trou) d'au
    // plus `FACING` pixels, le long d'une ligne ou d'une colonne ; le trait qui les sépare compte
    // pour leur bord commun.
    let mut face = |a: u16, b: u16| {
        if a != b {
            *shared.entry((a.min(b) as u32) << 16 | a.max(b) as u32).or_insert(0) += 1;
        }
    };
    for y in 0..bh {
        let (mut prev, mut at) = (0u16, 0usize);
        for x in 0..bw {
            let l = seg.lab[y * bw + x];
            if l != 0 {
                if prev != 0 && x - at <= FACING + 1 {
                    face(prev, l);
                }
                (prev, at) = (l, x);
            }
        }
    }
    for x in 0..bw {
        let (mut prev, mut at) = (0u16, 0usize);
        for y in 0..bh {
            let l = seg.lab[y * bw + x];
            if l != 0 {
                if prev != 0 && y - at <= FACING + 1 {
                    face(prev, l);
                }
                (prev, at) = (l, y);
            }
        }
    }
    let mut neighbours: Vec<Vec<(u16, u32)>> = vec![Vec::new(); count + 1];
    for (key, len) in shared {
        let (a, b) = ((key >> 16) as u16, (key & 0xffff) as u16);
        neighbours[a as usize].push((b, len));
        neighbours[b as usize].push((a, len));
    }
    seg.zones = (1..=count)
        .map(|i| {
            let a = &acc[i];
            let n = a.n.max(1.0);
            let (cx, cy) = (a.sx / n, a.sy / n);
            let (mu20, mu02, mu11) = (a.sxx - n * cx * cx, a.syy - n * cy * cy, a.sxy - n * cx * cy);
            let (e20, e02, e11) = (mu20 / (n * n), mu02 / (n * n), mu11 / (n * n));
            let mut nb = std::mem::take(&mut neighbours[i]);
            nb.sort_by(|p, q| q.1.cmp(&p.1).then(p.0.cmp(&q.0)));
            Zone {
                id: i as u16,
                area: a.n as u32,
                centroid: ((seg.x0 as f64 + cx) as f32 + 0.5, (seg.y0 as f64 + cy) as f32 + 0.5),
                bbox: if a.n > 0.0 { a.bbox } else { [0; 4] },
                perimeter: a.perimeter,
                neighbours: nb,
                hu: [e20 + e02, (e20 - e02).powi(2) + 4.0 * e11 * e11],
                outside: Some(i as u16) == outside,
            }
        })
        .collect();
}

// ---------------------------------------------------------------------------------------- cache

type Key = (u64, u64, Reglages);

struct CacheInner {
    map: HashMap<Key, Arc<Segmentation>>,
    order: VecDeque<Key>,
    bytes: usize,
}

/// Les dessins déjà découpés, par identifiant et version du dessin et réglages : relancer le
/// coloriage après avoir retouché des couleurs ne redécoupe rien. Borné en mémoire.
pub struct ZoneCache {
    inner: Mutex<CacheInner>,
    budget: usize,
    computed: std::sync::atomic::AtomicUsize,
}

impl ZoneCache {
    pub fn new(budget_bytes: usize) -> ZoneCache {
        ZoneCache { inner: Mutex::new(CacheInner { map: HashMap::new(), order: VecDeque::new(), bytes: 0 }), budget: budget_bytes, computed: Default::default() }
    }

    /// La découpe du dessin `cel` (version `version`), refaite si le cache ne l'a pas.
    pub fn get(&self, cel: u64, version: u64, line: &Raster, r: &Reglages) -> Arc<Segmentation> {
        let key = (cel, version, *r);
        if let Some(s) = self.lock().map.get(&key) {
            return s.clone();
        }
        // (Calculé hors du verrou : deux fils peuvent le faire ensemble, le dernier garde la sienne.)
        let seg = Arc::new(segment(line, r));
        self.computed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut inner = self.lock();
        if inner.map.insert(key, seg.clone()).is_none() {
            inner.order.push_back(key);
            inner.bytes += seg.bytes();
        }
        while inner.bytes > self.budget && inner.order.len() > 1 {
            let Some(old) = inner.order.pop_front() else { break };
            if let Some(s) = inner.map.remove(&old) {
                inner.bytes = inner.bytes.saturating_sub(s.bytes());
            }
        }
        seg
    }

    /// Combien de découpes ont été calculées (et non retrouvées).
    #[cfg(test)]
    pub fn computed(&self) -> usize {
        self.computed.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, CacheInner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}
