//! Lecture des dessins `.tvg` d'un projet Storyboard Pro (import 71, 2026-10-29). Le format a été
//! relevé en lisant les fichiers d'Anthony (interopérabilité, aucun code ni outil de Toon Boom) ; ce
//! qui est déduit est marqué « à vérifier » dans `docs/IMPORT-SBP.md`. La signature (`CERT`, `SIGN`)
//! n'est ni lue ni vérifiée : on ne fait que lire.
//!
//! Conteneur : « OTVGfull », puis une table `TTOC` (nombre, puis par section : balise et décalage),
//! puis des sections `balise(4) mode(4: UNCO brut | ZLIB) taille(u64) données` ; en ZLIB, les données
//! sont `taille décompressée (u64)` puis un flux zlib. Sections : `tUAA` `tCAA` `tLAA` `tOAA` (arts
//! Underlay, Color, Line, Overlay), `TLAB` (calque bitmap), `TPAL` (palette du dessin).
//!
//! Dans une section, des blocs `balise(4) drapeau(1) longueur` : la longueur tient sur autant
//! d'octets que le drapeau a de bits à 1 (1, 3, 7 → 1, 2, 3 octets), petit-boutiste.
//! - art : `TGLY` (calque plan) > `TGVS` (zone) > `TGSD` (attributs ; `TGCO` : 49 octets avant
//!   l'identifiant de couleur, 64 bits) + `TGBP` (courbe : nombre de points u32, masque « point
//!   sur la courbe » en bits de poids faible d'abord, puis des (x, y) f32). Trois points hors
//!   courbe entre deux points sur la courbe : une Bézier cubique ; un chemin fermé.
//! - bitmap : `TBBM` : un bloc `TBBA` (x, y, largeur, hauteur i32 : la zone utile) et un PNG.
//! - palette : `TCSC` (RVBA) suivi de `TCID` (nombre de lettres u32, UTF-16, identifiant u64).
//! Unités : 576 unités de dessin par « champ » (un champ = 1/9 de la hauteur du cadre) ; y vers le
//! haut (vérifié en recalant des rendus sur les vignettes de Storyboard Pro).

use std::collections::HashMap;
use std::io::Read;

/// Une zone remplie : ses contours (polylignes, Béziers aplanies) dans les unités du dessin.
#[derive(Clone, Debug, PartialEq)]
pub struct TvgShape {
    pub ring: Vec<[f32; 2]>,
    pub color: u64,
    /// Rang du calque plan (`TGLY`) qui porte la zone, dans tout le dessin.
    pub layer: usize,
    /// Zone sans couleur et fermée : un trou de la zone colorée qui la précède (même calque plan).
    pub hole: bool,
}

/// Une image bitmap : rectangle `(x, y, largeur, hauteur)` en pixels de la toile, centrée sur 0.
#[derive(Clone, Debug)]
pub struct TvgBitmap {
    pub x: i32,
    pub y: i32,
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct TvgDrawing {
    pub shapes: Vec<TvgShape>,
    pub palette: HashMap<u64, [u8; 4]>,
    pub bitmap: Option<TvgBitmap>,
    /// Zones sans couleur (traits de construction ?), non reprises.
    pub skipped: usize,
    /// Les zones sans couleur : leur ligne (courbes aplanies) et les 13 octets de leurs attributs.
    pub open: Vec<TvgOpen>,
}

/// Une zone sans couleur (TGSD de 13 octets) : une ligne médiane.
#[derive(Clone, Debug, PartialEq)]
pub struct TvgOpen {
    pub line: Vec<[f32; 2]>,
    pub attrs: Vec<u8>,
    pub layer: usize,
}

fn le(b: &[u8]) -> u64 {
    b.iter().rev().fold(0u64, |a, &x| (a << 8) | x as u64)
}

fn block(b: &[u8], o: usize) -> Option<(&[u8], usize, usize)> {
    if o + 6 > b.len() || b[o] != b'T' || !b[o..o + 4].iter().all(u8::is_ascii_alphabetic) {
        return None;
    }
    let flag = b[o + 4];
    if !matches!(flag, 1 | 3 | 7 | 15) {
        return None;
    }
    let n = flag.count_ones() as usize;
    if o + 5 + n > b.len() {
        return None;
    }
    let len = le(&b[o + 5..o + 5 + n]) as usize;
    let start = o + 5 + n;
    (start + len <= b.len()).then(|| (&b[o..o + 4], start, start + len))
}

/// Les blocs de premier niveau trouvés dans `b` (les octets entre deux blocs sont sautés).
fn blocks(b: &[u8]) -> Vec<(&[u8], &[u8])> {
    let mut out = Vec::new();
    let mut o = 0;
    while o < b.len() {
        match block(b, o) {
            Some((tag, s, e)) => {
                out.push((tag, &b[s..e]));
                o = e;
            }
            None => o += 1,
        }
    }
    out
}

fn sections(b: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    if b.len() < 16 || &b[..4] != b"OTVG" {
        return Err("ce fichier n'est pas un dessin Storyboard Pro (.tvg)".into());
    }
    let ttoc = b.windows(4).position(|w| w == b"TTOC").ok_or("table des sections introuvable")?;
    let cnt = le(b.get(ttoc + 8..ttoc + 12).ok_or("table tronquée")?) as usize;
    let base = ttoc + 12 + cnt * 12 + 12;
    let mut out = Vec::new();
    for k in 0..cnt {
        let o = ttoc + 12 + k * 12;
        let entry = b.get(o..o + 12).ok_or("table tronquée")?;
        let name = String::from_utf8_lossy(&entry[..4]).into_owned();
        let at = base + le(&entry[4..12]) as usize;
        let head = b.get(at..at + 16).ok_or("section hors du fichier")?;
        let size = le(&head[8..16]) as usize;
        let data = b.get(at + 16..at + 16 + size).ok_or("section tronquée")?;
        let raw = if &head[4..8] == b"ZLIB" {
            let mut v = Vec::new();
            flate2::read::ZlibDecoder::new(data.get(8..).ok_or("section ZLIB vide")?).read_to_end(&mut v).map_err(|e| format!("section {name} : {e}"))?;
            v
        } else {
            data.to_vec()
        };
        out.push((name, raw));
    }
    Ok(out)
}

/// Lit un dessin `.tvg`.
pub fn read(bytes: &[u8]) -> Result<TvgDrawing, String> {
    let mut d = TvgDrawing::default();
    for (name, raw) in sections(bytes)? {
        match name.as_str() {
            "TPAL" => read_palette(&raw, &mut d.palette),
            "TLAB" => d.bitmap = read_bitmap(&raw),
            n if n.starts_with('t') => read_art(&raw, &mut d),
            _ => {}
        }
    }
    Ok(d)
}

fn read_palette(raw: &[u8], out: &mut HashMap<u64, [u8; 4]>) {
    let mut cur = [0, 0, 0, 255];
    fn walk(b: &[u8], cur: &mut [u8; 4], out: &mut HashMap<u64, [u8; 4]>) {
        for (tag, d) in blocks(b) {
            match tag {
                b"TCSC" if d.len() >= 4 => *cur = [d[0], d[1], d[2], d[3]],
                b"TCID" if d.len() >= 4 => {
                    let p = 4 + le(&d[..4]) as usize * 2;
                    if let Some(id) = d.get(p..p + 8) {
                        out.insert(le(id), *cur);
                    }
                }
                _ => {}
            }
        }
    }
    walk(raw, &mut cur, out);
}

fn read_art(raw: &[u8], d: &mut TvgDrawing) {
    fn walk(b: &[u8], d: &mut TvgDrawing, ly: &mut usize) {
        for (tag, body) in blocks(b) {
            if tag == b"TGVS" {
                shape(body, d, *ly);
            } else {
                if tag == b"TGLY" {
                    *ly += 1;
                }
                walk(body, d, ly);
            }
        }
    }
    let mut ly = d.shapes.last().map_or(0, |s| s.layer + 1);
    walk(raw, d, &mut ly);
}

fn shape(body: &[u8], d: &mut TvgDrawing, layer: usize) {
    let (mut color, mut ring) = (None, None);
    let mut attrs: Vec<u8> = Vec::new();
    for (tag, data) in blocks(body) {
        match tag {
            b"TGSD" => {
                attrs = data.to_vec();
                if let Some(co) = blocks(data).into_iter().find(|(t, _)| *t == b"TGCO").map(|x| x.1) {
                    if co.len() >= 57 {
                        color = Some(le(&co[49..57]));
                    }
                }
            }
            b"TGBP" => ring = bezier(data),
            _ => {}
        }
    }
    match (color, ring) {
        (Some(c), Some(r)) if r.len() >= 3 => d.shapes.push(TvgShape { ring: r, color: c, layer, hole: false }),
        (None, Some(r)) if r.len() >= 4 && (r[0][0] - r[r.len() - 1][0]).hypot(r[0][1] - r[r.len() - 1][1]) < 0.5 => {
            d.shapes.push(TvgShape { ring: r.clone(), color: 0, layer, hole: true });
            d.open.push(TvgOpen { line: r, attrs, layer });
        }
        (None, Some(r)) => {
            d.skipped += 1;
            d.open.push(TvgOpen { line: r, attrs, layer });
        }
        _ => {}
    }
}

fn bezier(d: &[u8]) -> Option<Vec<[f32; 2]>> {
    let n = le(d.get(..4)?) as usize;
    let mask = n.div_ceil(8);
    let f = |p: usize| d.get(p..p + 4).map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]));
    let mut pts = Vec::with_capacity(n);
    for i in 0..n {
        let p = 4 + mask + i * 8;
        pts.push((f(p)?, f(p + 4)?, d.get(4 + i / 8)? >> (i % 8) & 1 == 1));
    }
    let mut out = vec![[pts.first()?.0, pts.first()?.1]];
    let mut i = 1;
    while i < pts.len() {
        if pts[i].2 {
            out.push([pts[i].0, pts[i].1]);
            i += 1;
        } else if i + 2 < pts.len() {
            let p0 = *out.last()?;
            let (p1, p2, p3) = ([pts[i].0, pts[i].1], [pts[i + 1].0, pts[i + 1].1], [pts[i + 2].0, pts[i + 2].1]);
            for s in 1..=10 {
                let t = s as f32 / 10.0;
                let u = 1.0 - t;
                let k = |a: f32, b: f32, c: f32, e: f32| u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * e;
                out.push([k(p0[0], p1[0], p2[0], p3[0]), k(p0[1], p1[1], p2[1], p3[1])]);
            }
            i += 3;
        } else {
            break;
        }
    }
    Some(out)
}

fn read_bitmap(raw: &[u8]) -> Option<TvgBitmap> {
    fn tiles<'a>(b: &'a [u8], out: &mut Vec<&'a [u8]>) {
        for (tag, d) in blocks(b) {
            if tag == b"TBBM" {
                out.push(d);
            } else {
                tiles(d, out);
            }
        }
    }
    let mut list = Vec::new();
    tiles(raw, &mut list);
    let i32le = |b: &[u8]| i32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    let mut parts = Vec::new();
    for t in list {
        let area = blocks(t).into_iter().flat_map(|(_, x)| blocks(x)).find(|(tag, x)| *tag == b"TBBA" && x.len() >= 16).map(|x| x.1)?;
        let at = t.windows(8).position(|w| w == [0x89, 0x50, 0x4e, 0x47, 13, 10, 26, 10])?;
        let img = image::load_from_memory(&t[at..]).ok()?.to_rgba8();
        parts.push((i32le(&area[0..]), i32le(&area[4..]), img));
    }
    let x0 = parts.iter().map(|p| p.0).min()?;
    let y0 = parts.iter().map(|p| p.1).min()?;
    let x1 = parts.iter().map(|p| p.0 + p.2.width() as i32).max()?;
    let y1 = parts.iter().map(|p| p.1 + p.2.height() as i32).max()?;
    let (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
    let mut rgba = vec![0u8; w * h * 4];
    for (px, py, img) in &parts {
        for (x, y, p) in img.enumerate_pixels() {
            let o = (((py - y0) as usize + y as usize) * w + (px - x0) as usize + x as usize) * 4;
            rgba[o..o + 4].copy_from_slice(&p.0);
        }
    }
    Some(TvgBitmap { x: x0, y: y0, width: w, height: h, rgba })
}

#[cfg(test)]
pub(crate) mod fabrique {
    //! Fabrique de petits `.tvg` synthétiques pour les essais.
    use std::io::Write;

    pub fn blk(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = tag.to_vec();
        if body.len() < 256 {
            v.extend([1, body.len() as u8]);
        } else {
            v.extend([3]);
            v.extend((body.len() as u16).to_le_bytes());
        }
        v.extend(body);
        v
    }

    pub fn zone(color: u64, pts: &[(f32, f32, bool)]) -> Vec<u8> {
        let mut co = vec![1u8];
        co.extend([0u8; 48]);
        co.extend(color.to_le_bytes());
        co.extend([0u8; 1]);
        let mut sd = vec![0u8; 9];
        sd.extend(blk(b"TGCO", &co));
        let mut bp = (pts.len() as u32).to_le_bytes().to_vec();
        let mut mask = vec![0u8; pts.len().div_ceil(8)];
        for (i, p) in pts.iter().enumerate() {
            if p.2 {
                mask[i / 8] |= 1 << (i % 8);
            }
        }
        bp.extend(mask);
        for p in pts {
            bp.extend(p.0.to_le_bytes());
            bp.extend(p.1.to_le_bytes());
        }
        let mut vs = blk(b"TGSD", &sd);
        vs.push(1);
        vs.extend(blk(b"TGBP", &bp));
        blk(b"TGVS", &vs)
    }

    /// Une zone sans couleur (TGSD de 13 octets, sans TGCO).
    pub fn zone_sans_couleur(pts: &[(f32, f32, bool)]) -> Vec<u8> {
        let mut bp = (pts.len() as u32).to_le_bytes().to_vec();
        let mut mask = vec![0u8; pts.len().div_ceil(8)];
        for (i, p) in pts.iter().enumerate() {
            if p.2 {
                mask[i / 8] |= 1 << (i % 8);
            }
        }
        bp.extend(mask);
        for p in pts {
            bp.extend(p.0.to_le_bytes());
            bp.extend(p.1.to_le_bytes());
        }
        let mut vs = blk(b"TGSD", &[0u8; 13]);
        vs.push(1);
        vs.extend(blk(b"TGBP", &bp));
        blk(b"TGVS", &vs)
    }

    pub fn palette(id: u64, rgba: [u8; 4]) -> Vec<u8> {
        let mut v = blk(b"TCSC", &rgba);
        let mut c = 1u32.to_le_bytes().to_vec();
        c.extend([b'A', 0]);
        c.extend(id.to_le_bytes());
        c.extend([0; 4]);
        v.extend(blk(b"TCID", &c));
        v
    }

    /// Un `.tvg` : art « ligne » avec les zones `shapes`, palette `pal`, la deuxième section en ZLIB.
    pub fn tvg(shapes: &[Vec<u8>], pal: &[u8]) -> Vec<u8> {
        let mut art = vec![0u8; 10];
        let mut ly = vec![2u8, 0, 1, 0, 0, 0];
        for s in shapes {
            ly.extend(s);
        }
        art.extend(blk(b"TGLY", &ly));
        let mut z = (art.len() as u64).to_le_bytes().to_vec();
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(&art).unwrap();
        z.extend(enc.finish().unwrap());
        let secs: Vec<(&[u8; 4], &[u8; 4], Vec<u8>)> = vec![(b"tUAA", b"UNCO", vec![0; 4]), (b"tLAA", b"ZLIB", z), (b"TPAL", b"UNCO", pal.to_vec())];
        let mut region = Vec::new();
        let mut toc = Vec::new();
        for (name, mode, data) in &secs {
            toc.extend(*name);
            toc.extend((region.len() as u64).to_le_bytes());
            region.extend(*name);
            region.extend(*mode);
            region.extend((data.len() as u64).to_le_bytes());
            region.extend(data);
        }
        let mut f = b"OTVGfull".to_vec();
        f.extend([0xf1, 3, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0]);
        f.extend(b"TTOC");
        f.extend(0u32.to_le_bytes());
        f.extend((secs.len() as u32).to_le_bytes());
        f.extend(toc);
        f.extend(b"UNCO");
        f.extend((region.len() as u32).to_le_bytes());
        f.extend(0u32.to_le_bytes());
        f.extend(region);
        f
    }
}

#[cfg(test)]
mod tests {
    use super::fabrique::*;
    use super::*;

    #[test]
    fn reads_a_synthetic_drawing() {
        let sq = [(0.0, 0.0, true), (100.0, 0.0, true), (100.0, 100.0, true), (0.0, 100.0, true)];
        let bytes = tvg(&[zone(7, &sq)], &palette(7, [10, 20, 30, 255]));
        let d = read(&bytes).unwrap();
        assert_eq!(d.shapes.len(), 1);
        assert_eq!(d.shapes[0].color, 7);
        assert_eq!(d.shapes[0].ring.len(), 4);
        assert_eq!(d.palette[&7], [10, 20, 30, 255]);
    }

    #[test]
    fn cubic_segments_are_flattened() {
        let c = [(0.0, 0.0, true), (0.0, 50.0, false), (50.0, 100.0, false), (100.0, 100.0, true)];
        let d = read(&tvg(&[zone(1, &c)], &[])).unwrap();
        assert_eq!(d.shapes[0].ring.len(), 11);
        let c2 = [(0.0, 0.0, true), (0.0, 50.0, false), (50.0, 100.0, false), (100.0, 100.0, true), (100.0, 0.0, true)];
        let d = read(&tvg(&[zone(1, &c2)], &[])).unwrap();
        assert_eq!(d.shapes[0].ring.len(), 1 + 10 + 1);
    }

    #[test]
    fn refuses_other_files() {
        assert!(read(b"not a tvg at all, really").is_err());
    }

    #[test]
    fn closed_colorless_zone_is_a_hole_and_open_one_is_skipped() {
        use super::fabrique::{palette, tvg, zone, zone_sans_couleur};
        let sq = |h: f32| [(-h, -h, true), (h, -h, true), (h, h, true), (-h, h, true), (-h, -h, true)];
        let open = [(0.0, 0.0, true), (10.0, 0.0, true), (20.0, 5.0, true)];
        let d = super::read(&tvg(&[zone(7, &sq(100.0)), zone_sans_couleur(&sq(40.0)), zone_sans_couleur(&open)], &palette(7, [0, 0, 0, 255]))).unwrap();
        assert_eq!(d.shapes.len(), 2, "une zone et son trou");
        assert!(!d.shapes[0].hole && d.shapes[1].hole);
        assert_eq!(d.skipped, 1, "la ligne ouverte est comptée, pas reprise");
    }
    /// Sonde à la main : `TVG_SONDE=<fichier .tvg>` affiche les zones (couleur ou non) et leurs attributs.
    #[test]
    #[ignore]
    fn sonde() {
        let Some(path) = std::env::var_os("TVG_SONDE") else { return };
        let bytes = std::fs::read(path).unwrap();
        for (name, raw) in super::sections(&bytes).unwrap() {
            println!("section {name} : {} octets", raw.len());
            if !name.starts_with('t') {
                continue;
            }
            fn walk(b: &[u8], depth: usize, shown: &mut usize) {
                for (tag, body) in super::blocks(b) {
                    let tag = String::from_utf8_lossy(tag).into_owned();
                    if tag == "TGVS" {
                        if *shown < 400 && super::blocks(body).iter().any(|(t, d)| *t == b"TGSD" && !super::blocks(d).iter().any(|(t3, _)| *t3 == b"TGCO")) {
                            *shown += 1;
                            println!("{:indent$}TGVS {} octets", "", body.len(), indent = depth * 2);
                            for (t2, d2) in super::blocks(body) {
                                let t2 = String::from_utf8_lossy(t2).into_owned();
                                let hex: String = d2.iter().take(80).map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ");
                                println!("{:indent$}  {t2} ({}) {hex}", "", d2.len(), indent = depth * 2);
                                if t2 == "TGSD" {
                                    for (t3, d3) in super::blocks(d2) {
                                        let hex3: String = d3.iter().take(70).map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ");
                                        println!("{:indent$}    {} ({}) {hex3}", "", String::from_utf8_lossy(t3), d3.len(), indent = depth * 2);
                                    }
                                }
                            }
                        }
                    } else {
                        println!("{:indent$}{tag} ({})", "", body.len(), indent = depth * 2);
                        walk(body, depth + 1, shown);
                    }
                }
            }
            let mut shown = 0;
            walk(&raw, 1, &mut shown);
        }
    }
    /// Sonde à la main : `TVG_DOSSIER=<dossier elements>` compte, par section d'art, les zones avec et sans couleur.
    #[test]
    #[ignore]
    fn sonde_dossier() {
        let Some(dir) = std::env::var_os("TVG_DOSSIER") else { return };
        let mut count: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
        let mut exemple: Option<(std::path::PathBuf, String)> = None;
        fn walk_files(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk_files(&p, out);
                } else if p.extension().is_some_and(|x| x == "tvg") {
                    out.push(p);
                }
            }
        }
        let mut files = Vec::new();
        walk_files(std::path::Path::new(&dir), &mut files);
        for f in &files {
            let Ok(bytes) = std::fs::read(f) else { continue };
            let Ok(secs) = super::sections(&bytes) else { continue };
            for (name, raw) in secs {
                if !name.starts_with('t') {
                    continue;
                }
                fn walk(b: &[u8], e: &mut (usize, usize), first_none: &mut Option<usize>, depth: usize) {
                    for (tag, body) in super::blocks(b) {
                        if tag == b"TGVS" {
                            let sd = super::blocks(body).into_iter().find(|(t, _)| *t == b"TGSD").map(|x| x.1);
                            let has = sd.is_some_and(|d| super::blocks(d).iter().any(|(t, _)| *t == b"TGCO"));
                            if has {
                                e.0 += 1;
                            } else {
                                e.1 += 1;
                                first_none.get_or_insert(body.len());
                            }
                        } else {
                            walk(body, e, first_none, depth + 1);
                        }
                    }
                }
                let mut e = (0, 0);
                let mut first_none = None;
                walk(&raw, &mut e, &mut first_none, 0);
                let c = count.entry(name.clone()).or_default();
                c.0 += e.0;
                c.1 += e.1;
                if e.1 > 0 && exemple.as_ref().is_none_or(|x| x.1 != name) {
                    exemple = Some((f.clone(), name.clone()));
                }
            }
        }
        let (mut closed, mut opened, mut n) = (0usize, 0usize, 0usize);
        let mut tailles: std::collections::BTreeMap<usize, usize> = Default::default();
        for f in &files {
            let Ok(bytes) = std::fs::read(f) else { continue };
            let Ok(d) = super::read(&bytes) else { continue };
            for o in &d.open {
                n += 1;
                *tailles.entry(o.line.len().min(40)).or_default() += 1;
                let (a, b) = (o.line[0], *o.line.last().unwrap());
                if (a[0] - b[0]).hypot(a[1] - b[1]) < 0.5 {
                    closed += 1;
                } else {
                    opened += 1;
                }
            }
        }
        println!("sans couleur : {n}, fermées {closed}, ouvertes {opened}");
        println!("points : {tailles:?}");
        for (k, v) in &count {
            println!("section {k} : {} zones colorées, {} sans couleur", v.0, v.1);
        }
        if let Some(e) = exemple {
            println!("exemple : {} section {}", e.0.display(), e.1);
        }
    }
    /// Sonde à la main : `TVG_BITMAP=<fichier>` et `TVG_SORTIE=<png>` : écrit le bitmap décodé du dessin.
    #[test]
    #[ignore]
    fn sonde_bitmap() {
        let (Some(path), Some(out)) = (std::env::var_os("TVG_BITMAP"), std::env::var_os("TVG_SORTIE")) else { return };
        let bytes = std::fs::read(&path).unwrap();
        for (name, raw) in super::sections(&bytes).unwrap() {
            if name == "TLAB" {
                let sig = [0x89u8, 0x50, 0x4e, 0x47, 13, 10, 26, 10];
                let mut o = 0;
                while let Some(p) = raw[o..].windows(8).position(|w| w == sig) {
                    let h = &raw[o + p + 16..o + p + 29];
                    println!("PNG largeur {} hauteur {} profondeur {} type {} ", u32::from_be_bytes([h[0], h[1], h[2], h[3]]), u32::from_be_bytes([h[4], h[5], h[6], h[7]]), h[8], h[9]);
                    o += p + 8;
                }
            }
        }
        {
            fn tiles<'a>(b: &'a [u8], out: &mut Vec<&'a [u8]>) {
                for (tag, d) in super::blocks(b) {
                    if tag == b"TBBM" { out.push(d); } else { tiles(d, out); }
                }
            }
            for (name, raw) in super::sections(&bytes).unwrap() {
                if name != "TLAB" { continue; }
                let mut list = Vec::new();
                tiles(&raw, &mut list);
                for t in list.iter().take(6) {
                    println!("tuile {} octets ; blocs : {:?}", t.len(), super::blocks(t).iter().map(|(g, x)| (String::from_utf8_lossy(g).into_owned(), x.len())).collect::<Vec<_>>());
                    for (_, x) in super::blocks(t) {
                        for (g, y) in super::blocks(x) {
                            let v: Vec<i32> = y.chunks(4).take(6).map(|c| if c.len() == 4 { i32::from_le_bytes([c[0], c[1], c[2], c[3]]) } else { -1 }).collect();
                            println!("   sous-bloc {} {} octets : {:?}", String::from_utf8_lossy(g), y.len(), v);
                        }
                    }
                }
            }
        }
        let d = super::read(&bytes).unwrap();
        let Some(bm) = d.bitmap else { println!("pas de bitmap ; {} zones", d.shapes.len()); return };
        println!("bitmap {}x{} en ({}, {})", bm.width, bm.height, bm.x, bm.y);
        image::RgbaImage::from_raw(bm.width as u32, bm.height as u32, bm.rgba).unwrap().save(out).unwrap();
    }}
