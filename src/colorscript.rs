//! Exporter un « color script » (Anthony, 2026-10-21) : une petite image par plan, rangées par séquence en rangées serrées sur un
//! fond uni, pour voir d'un coup d'œil comment la couleur et la lumière évoluent dans le film. Fichier > Exporter > « Color script ».
//! La fenêtre et la tâche de fond : `colorscriptui.rs` ; le calcul, ici, sans interface.
//!
//! - **Une image par plan** d'office (la première, ou celle du milieu du plan) ; l'option « une image par case » en prend une par
//!   case de la planche (le plan et chacune de ses marques d'image).
//! - **Rangement** : chaque séquence a son titre, puis ses images en rangées de `par_ligne` au plus, séparées de `espace` pixels, sur
//!   un fond (noir d'office, ou la couleur choisie) ; la largeur de l'image est celle de la plus longue rangée. Les numéros de plan
//!   sont facultatifs (petit texte en haut à gauche de chaque image de début de plan).
//! - **Bande de couleurs dominantes** facultative (éteinte d'office) : sous chaque image, une bande de cinq couleurs au plus, chacune
//!   large comme la part de l'image qu'elle représente (`dominantes`).
//! - **Sortie** : PNG, ou PDF en une seule grande page (images en JPEG). Une image trop grande est réduite aux bornes de
//!   `planche.rs`, et le compte rendu le dit.

use image::{Rgb, RgbImage};
use tiny_skia::{Color, Pixmap};

use crate::document::Document;
use crate::planche::{MAX_COTE, MAX_PIXELS};

/// Quelle image représente un plan (ou une case).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prise {
    Premiere,
    Milieu,
}

/// Le fichier écrit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sortie {
    Png,
    Pdf,
}

impl Sortie {
    pub fn extension(self) -> &'static str {
        match self {
            Sortie::Png => "png",
            Sortie::Pdf => "pdf",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub sortie: Sortie,
    /// Une image par case (le plan et chacune de ses marques d'image) au lieu d'une par plan.
    pub par_case: bool,
    pub prise: Prise,
    /// Largeur d'une image, en pixels, et écart entre deux images.
    pub largeur: u32,
    pub espace: u32,
    /// Images par rangée, au plus.
    pub par_ligne: u32,
    pub fond: [u8; 3],
    pub titres: bool,
    pub numeros: bool,
    /// La bande des couleurs dominantes sous chaque image : éteinte d'office.
    pub bande: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { sortie: Sortie::Png, par_case: false, prise: Prise::Premiere, largeur: 160, espace: 2, par_ligne: 12, fond: [0, 0, 0], titres: true, numeros: false, bande: false }
    }
}

/// Une image du color script.
#[derive(Clone, Debug, PartialEq)]
pub struct Entree {
    /// Le plan, dans le projet, et l'image du plan.
    pub plan: usize,
    pub frame: usize,
    pub etiquette: Option<String>,
    /// Où elle est posée (coin haut gauche) et la taille de l'image elle-même.
    pub x: i32,
    pub y: i32,
}

/// Une séquence et ses images.
#[derive(Clone, Debug, PartialEq)]
pub struct Bloc {
    pub titre: String,
    pub titre_y: i32,
    pub entrees: Vec<Entree>,
}

/// Le color script rangé, sans pixel.
#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    pub blocs: Vec<Bloc>,
    /// La taille d'une image, la hauteur de sa bande, et celle de l'image entière.
    pub cellule: [u32; 2],
    pub bande_h: u32,
    pub marge: i32,
    pub titre_taille: f32,
    pub taille: [u32; 2],
    pub options: Options,
    pub reduit: Option<String>,
}

impl Spec {
    /// Les images à calculer : (plan, image du plan), dans l'ordre du rangement.
    pub fn images(&self) -> Vec<(usize, usize)> {
        self.blocs.iter().flat_map(|b| b.entrees.iter().map(|e| (e.plan, e.frame))).collect()
    }

    pub fn nombre(&self) -> usize {
        self.blocs.iter().map(|b| b.entrees.len()).sum()
    }
}

/// Ce que montre le color script pour les plans `plans` (indices du projet, dans l'ordre du film).
pub fn construire(doc: &Document, plans: &[usize], opt: &Options) -> Spec {
    let mut opt = opt.clone();
    opt.largeur = opt.largeur.clamp(24, 2000);
    opt.par_ligne = opt.par_ligne.clamp(1, 200);
    opt.espace = opt.espace.min(200);
    // Les images, séquence par séquence (dans l'ordre du film).
    let mut groupes: Vec<(u64, Vec<(usize, usize, Option<String>)>)> = Vec::new();
    for &p in plans.iter().filter(|&&p| p < doc.panels.len()) {
        let seq = doc.sequence_of(p);
        let n = doc.panels[p].frame_count();
        let prises: Vec<(usize, usize, bool)> = if opt.par_case {
            crate::board::plan_boards(doc, p).into_iter().map(|(_, a, b)| (a, b, a == 0)).collect()
        } else {
            vec![(0, n, true)]
        };
        for (a, b, debut) in prises {
            let frame = match opt.prise {
                Prise::Premiere => a,
                Prise::Milieu => (a + b.saturating_sub(1)) / 2,
            };
            let etiquette = (opt.numeros && debut).then(|| doc.scene_label(doc.panels[p].scene));
            match groupes.last_mut() {
                Some((s, v)) if *s == seq => v.push((p, frame.min(n.saturating_sub(1)), etiquette)),
                _ => groupes.push((seq, vec![(p, frame.min(n.saturating_sub(1)), etiquette)])),
            }
        }
    }
    let colonnes = groupes.iter().map(|g| g.1.len()).max().unwrap_or(1).min(opt.par_ligne as usize).max(1);
    // Les bornes d'une image : la largeur d'une image est réduite jusqu'à y entrer.
    let ratio = doc.frame[1].max(1) as f32 / doc.frame[0].max(1) as f32;
    let mut reduit = None;
    let mut largeur = opt.largeur;
    let mesure = |largeur: u32| -> (u32, [u32; 2]) {
        let h = (largeur as f32 * ratio).round().max(1.0) as u32;
        let bande = if opt.bande { (h as f32 * 0.14).round().max(3.0) as u32 } else { 0 };
        let marge = (opt.espace * 4).max(8);
        let titre_h = if opt.titres { ((largeur as f32 * 0.14).clamp(12.0, 40.0) * 1.5).ceil() as u32 + opt.espace * 2 } else { 0 };
        let cell_h = h + bande;
        let mut y = marge;
        for (_, v) in &groupes {
            y += titre_h;
            let rangees = v.len().div_ceil(colonnes) as u32;
            y += rangees * (cell_h + opt.espace) - opt.espace + (opt.espace * 3).max(10);
        }
        let w = marge * 2 + colonnes as u32 * largeur + (colonnes as u32 - 1) * opt.espace;
        (bande, [w, y - (opt.espace * 3).max(10) + marge])
    };
    let (mut bande_h, mut taille) = mesure(largeur);
    let voulue = taille;
    while (taille[0] as f32 > MAX_COTE || taille[1] as f32 > MAX_COTE || taille[0] as f32 * taille[1] as f32 > MAX_PIXELS) && largeur > 24 {
        largeur = (largeur as f32 * 0.9) as u32;
        (bande_h, taille) = mesure(largeur);
    }
    if largeur != opt.largeur {
        reduit = Some(format!("Images réduites à {largeur} pixels de large (l'image aurait fait {} × {} pixels).", voulue[0], voulue[1]));
        opt.largeur = largeur;
    }
    let h = (largeur as f32 * ratio).round().max(1.0) as u32;
    let marge = (opt.espace * 4).max(8) as i32;
    let titre_taille = (largeur as f32 * 0.14).clamp(12.0, 40.0);
    let titre_h = if opt.titres { (titre_taille * 1.5).ceil() as i32 + opt.espace as i32 * 2 } else { 0 };
    let cell_h = (h + bande_h) as i32;
    let esp = opt.espace as i32;
    let mut y = marge;
    let mut blocs = Vec::new();
    for (seq, v) in &groupes {
        let titre = doc.sequence_label(*seq);
        let titre_y = y;
        y += titre_h;
        let entrees = v
            .iter()
            .enumerate()
            .map(|(i, (plan, frame, etiquette))| {
                let (r, c) = ((i / colonnes) as i32, (i % colonnes) as i32);
                Entree { plan: *plan, frame: *frame, etiquette: etiquette.clone(), x: marge + c * (largeur as i32 + esp), y: y + r * (cell_h + esp) }
            })
            .collect();
        let rangees = v.len().div_ceil(colonnes) as i32;
        y += rangees * (cell_h + esp) - esp + (esp * 3).max(10);
        blocs.push(Bloc { titre, titre_y, entrees });
    }
    Spec { blocs, cellule: [largeur, h], bande_h, marge, titre_taille, taille, options: opt, reduit }
}

/// Les couleurs dominantes d'une image : au plus `n`, avec la part de l'image que chacune représente (la somme fait 1), de la plus
/// présente à la moins présente. Deux couleurs trop proches n'en font qu'une.
pub fn dominantes(img: &RgbImage, n: usize) -> Vec<([u8; 3], f32)> {
    if img.width() == 0 || img.height() == 0 || n == 0 {
        return Vec::new();
    }
    let petit = if img.width() > 64 {
        let h = ((img.height() as f32 * 64.0 / img.width() as f32).round() as u32).max(1);
        image::imageops::resize(img, 64, h, image::imageops::FilterType::Triangle)
    } else {
        img.clone()
    };
    // Un histogramme à quatre bits par canal.
    let mut cases: std::collections::HashMap<u16, (u32, [u32; 3])> = std::collections::HashMap::new();
    for p in petit.pixels() {
        let k = ((p[0] as u16 >> 4) << 8) | ((p[1] as u16 >> 4) << 4) | (p[2] as u16 >> 4);
        let e = cases.entry(k).or_insert((0, [0; 3]));
        e.0 += 1;
        for c in 0..3 {
            e.1[c] += p[c] as u32;
        }
    }
    let mut triees: Vec<(u16, u32, [f32; 3])> = cases.into_iter().map(|(k, (nb, s))| (k, nb, [s[0] as f32 / nb as f32, s[1] as f32 / nb as f32, s[2] as f32 / nb as f32])).collect();
    triees.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let dist = |a: [f32; 3], b: [f32; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
    let mut centres: Vec<[f32; 3]> = Vec::new();
    for t in &triees {
        if centres.len() >= n {
            break;
        }
        if centres.iter().all(|c| dist(*c, t.2) >= 40.0) {
            centres.push(t.2);
        }
    }
    let mut parts = vec![(0u32, [0f32; 3]); centres.len()];
    for t in &triees {
        let k = (0..centres.len()).min_by(|&a, &b| dist(centres[a], t.2).total_cmp(&dist(centres[b], t.2))).unwrap_or(0);
        parts[k].0 += t.1;
        for c in 0..3 {
            parts[k].1[c] += t.2[c] * t.1 as f32;
        }
    }
    let total: u32 = parts.iter().map(|p| p.0).sum::<u32>().max(1);
    let mut out: Vec<([u8; 3], f32)> = parts
        .into_iter()
        .filter(|p| p.0 > 0)
        .map(|(nb, s)| ([(s[0] / nb as f32).round() as u8, (s[1] / nb as f32).round() as u8, (s[2] / nb as f32).round() as u8], nb as f32 / total as f32))
        .collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    out
}

/// L'image d'une entrée du color script, avec sa palette si la bande est demandée.
pub type Cellule = (RgbImage, Vec<([u8; 3], f32)>);

/// La couleur lisible sur le fond `fond`.
fn encre_sur(fond: [u8; 3]) -> [u8; 3] {
    let l = 0.299 * fond[0] as f32 + 0.587 * fond[1] as f32 + 0.114 * fond[2] as f32;
    if l < 128.0 {
        [235, 235, 235]
    } else {
        [30, 30, 30]
    }
}

/// Le color script dessiné : `cellules[k]` est l'image de la `k`-ième entrée (dans l'ordre de `Spec::images`).
pub fn composer(spec: &Spec, cellules: &[Cellule]) -> Result<RgbImage, String> {
    let [w, h] = spec.taille;
    let mut pix = Pixmap::new(w, h).ok_or("image trop grande pour la mémoire")?;
    let f = spec.options.fond;
    pix.fill(Color::from_rgba8(f[0], f[1], f[2], 255));
    let encre = encre_sur(f);
    let [cw, ch] = spec.cellule;
    let mut k = 0;
    for bloc in &spec.blocs {
        if spec.options.titres && !bloc.titre.trim().is_empty() {
            crate::planche::ecrire(&mut pix, &bloc.titre, spec.marge as f32, bloc.titre_y as f32, spec.titre_taille, encre);
        }
        for e in &bloc.entrees {
            let Some((img, palette)) = cellules.get(k) else {
                k += 1;
                continue;
            };
            k += 1;
            let img = if img.dimensions() != (cw, ch) { image::imageops::resize(img, cw, ch, image::imageops::FilterType::Triangle) } else { img.clone() };
            let mut rgba = Vec::with_capacity((cw * ch * 4) as usize);
            for px in img.pixels() {
                rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
            if let Some(cell) = tiny_skia::IntSize::from_wh(cw, ch).and_then(|sz| Pixmap::from_vec(rgba, sz)) {
                pix.draw_pixmap(e.x, e.y, cell.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
            }
            // La bande des couleurs dominantes, sous l'image.
            if spec.bande_h > 0 && !palette.is_empty() {
                let (y0, y1) = (e.y + ch as i32, e.y + (ch + spec.bande_h) as i32);
                let mut x = e.x as f32;
                for (i, (c, part)) in palette.iter().enumerate() {
                    let fin = if i + 1 == palette.len() { (e.x + cw as i32) as f32 } else { x + part * cw as f32 };
                    crate::planche::remplir(&mut pix, [x.round() as i32, y0, fin.round() as i32, y1], *c);
                    x = fin;
                }
            }
            if let Some(num) = &e.etiquette {
                let t = (cw as f32 * 0.09).clamp(8.0, 18.0);
                crate::planche::remplir(&mut pix, [e.x, e.y, e.x + (t * (num.chars().count() as f32 * 0.62 + 0.8)) as i32, e.y + (t * 1.3) as i32], [0, 0, 0]);
                crate::planche::ecrire(&mut pix, num, e.x as f32 + t * 0.3, e.y as f32 + t * 0.05, t, [255, 255, 255]);
            }
        }
    }
    let mut out = RgbImage::new(w, h);
    for (dst, src) in out.pixels_mut().zip(pix.data().chunks_exact(4)) {
        *dst = Rgb([src[0], src[1], src[2]]);
    }
    Ok(out)
}

/// Les octets du fichier.
pub fn encoder(img: &RgbImage, sortie: Sortie, titre: &str) -> Result<Vec<u8>, String> {
    match sortie {
        Sortie::Png => crate::planche::png(img),
        Sortie::Pdf => {
            // 150 points par pouce, une seule grande page (jusqu'à 14 400 points).
            let k = (72.0_f32 / 150.0).min(14_400.0 / img.width().max(img.height()).max(1) as f32);
            let (pw, ph) = (img.width() as f32 * k, img.height() as f32 * k);
            let mut pdf = crate::pdf::PdfFile::new();
            let id = pdf.add_jpeg(img.width(), img.height(), &crate::planche::jpeg(img, 90)?);
            pdf.add_page_sized(pw, ph, crate::pdf::op_image(ph, id, 0.0, 0.0, pw, ph).as_bytes(), &[id]);
            Ok(pdf.finish(titre))
        }
    }
}

#[cfg(test)]
#[path = "colorscript_tests.rs"]
mod tests;
