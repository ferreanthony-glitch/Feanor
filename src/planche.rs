//! Exporter la planche des Vignettes (Anthony, 2026-10-21) : toute la planche en une grande image PNG ou JPEG, ou en PDF (une
//! seule grande page, ou des pages A4 / A3 paysage). Fichier > Exporter > « Planche des vignettes ». La fenêtre et la tâche de
//! fond : `plancheui.rs` ; le calcul, ici, sans interface.
//!
//! # Ce qui est exporté
//!
//! - **La planche telle qu'à l'écran** : les mêmes pages blanches, la même disposition (taille des vignettes, colonnes, lignes par
//!   page : les réglages de la vue Vignettes), le même filet autour de chaque vignette ; sans la table grise autour des pages,
//!   sans les cases vides ni leur filet. Les images viennent du rendu de l'export (le cadre de la caméra, en pleine
//!   définition), calculées sur d'autres fils comme pour tout export (`ExportJob::collect_with`) : l'interface ne calcule
//!   rien.
//! - **Étendue** : tout le film, la séquence courante ou les plans choisis. Les vignettes se rangent alors comme sur la planche
//!   qui n'aurait que ces plans (mêmes colonnes).
//! - **Options** : numéros de plan, notes (bandes sous les images, trois lignes au plus), barres de coupe, cadres de caméra
//!   (là où la caméra bouge : l'empreinte de la case, A, B, C… en bleu fin avec leurs lettres, reliés comme à l'écran),
//!   annotations (décochées d'office), marie-louise (le passe-partout couvre tout ce qui est hors de l'intérieur des vignettes,
//!   traits qui débordent compris).
//! - **Résolution** : 150 ou 300 points par pouce (une unité de la planche = 1/96 de pouce), ou une largeur en pixels. Une image
//!   trop grande (plus de 16 000 pixels de côté ou 60 millions de pixels) est réduite, et le compte rendu le dit.
//! - **PDF** : « une page » (la planche entière sur une page à sa taille, jusqu'à 200 pouces) ou « A4 / A3 paysage » : la planche
//!   est mise à la largeur de la page puis coupée en hauteur, de préférence entre deux pages de la planche ; un petit pied de page
//!   numérote. Les images sont en JPEG dans le PDF.

use eframe::egui::{pos2, vec2, Pos2, Rect};
use image::{Rgb, RgbImage};
use tiny_skia::{Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform};

use crate::annotations::Annotations;
use crate::board::{self, BoardLayout, BoardSettings, CardPlace};
use crate::document::Document;

/// Ce que l'export écrit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sortie {
    Png,
    Jpeg,
    /// La planche entière sur une seule page.
    PdfUnePage,
    PdfA4,
    PdfA3,
}

impl Sortie {
    pub const TOUTES: [Sortie; 5] = [Sortie::Png, Sortie::Jpeg, Sortie::PdfUnePage, Sortie::PdfA4, Sortie::PdfA3];

    pub fn extension(self) -> &'static str {
        match self {
            Sortie::Png => "png",
            Sortie::Jpeg => "jpg",
            _ => "pdf",
        }
    }

    pub fn nom(self) -> &'static str {
        match self {
            Sortie::Png => "Image PNG",
            Sortie::Jpeg => "Image JPEG",
            Sortie::PdfUnePage => "PDF, une grande page",
            Sortie::PdfA4 => "PDF, pages A4 paysage",
            Sortie::PdfA3 => "PDF, pages A3 paysage",
        }
    }

}

/// La taille de l'image.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Resolution {
    /// Points par pouce : une unité de la planche = 1/96 de pouce.
    Dpi(u32),
    /// Largeur de l'image, en pixels.
    Largeur(u32),
}

/// Les options de l'export.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub sortie: Sortie,
    pub resolution: Resolution,
    pub numeros: bool,
    pub notes: bool,
    pub barres: bool,
    pub cadres: bool,
    /// Les annotations de la planche : décochées d'office.
    pub annotations: bool,
    pub marie: bool,
    pub marie_couleur: [u8; 3],
}

impl Default for Options {
    fn default() -> Self {
        Options {
            sortie: Sortie::Png,
            resolution: Resolution::Dpi(150),
            numeros: true,
            notes: true,
            barres: true,
            cadres: true,
            annotations: false,
            marie: false,
            marie_couleur: crate::marie::COULEUR_PAR_DEFAUT,
        }
    }
}

/// Bornes d'une image exportée.
pub const MAX_COTE: f32 = 16_000.0;
pub const MAX_PIXELS: f32 = 60_000_000.0;

const TABLE: [u8; 3] = [235, 235, 235];
const FILET: [u8; 3] = [178, 178, 178];
const BARRE_GRISE: [u8; 3] = [188, 188, 188];
const BARRE_NOIRE: [u8; 3] = [22, 22, 22];
const ENCRE_NUMERO: [u8; 3] = [96, 96, 96];
const ENCRE_NOTE: [u8; 3] = [70, 70, 70];
/// Le bleu fin des cadres de caméra (le même qu'à l'écran, `vignettecamui::BLEU`).
const CAMERA: [u8; 3] = [0x37, 0x8A, 0xDD];

/// Une vignette à exporter.
#[derive(Clone, Debug)]
pub struct Carte {
    pub place: CardPlace,
    /// L'indice du plan dans le vrai projet.
    pub plan_reel: usize,
    /// Le numéro du plan, s'il commence là.
    pub etiquette: Option<String>,
    /// Les lignes de la note (trois au plus, « … » au bout).
    pub note: Vec<String>,
    pub note_taille: f32,
    /// L'empreinte du mouvement de caméra (cadres A, B, C…, à l'échelle des cases), si la caméra bouge et que les cadres sont demandés.
    pub mouvement: Option<board::MoveBoard>,
}

/// Tout ce qu'il faut pour dessiner la planche, sans pixel.
#[derive(Clone, Debug)]
pub struct Planche {
    pub layout: BoardLayout,
    pub cartes: Vec<Carte>,
    pub annotations: Annotations,
    /// Ce qu'on exporte, en unités de la planche.
    pub cadre: Rect,
}

impl Planche {
    /// Les images à calculer : (plan du projet, image du plan), une par vignette.
    pub fn images(&self) -> Vec<(usize, usize)> {
        self.cartes.iter().map(|c| (c.plan_reel, c.place.frame)).collect()
    }
}

/// Le texte `text` mis en lignes de `width` unités au plus à la taille `size` : au plus `max` lignes, « … » au bout du texte coupé.
pub fn lignes_de(text: &str, size: f32, width: f32, max: usize) -> Vec<String> {
    let mesure = |s: &str| crate::text::layout(s, crate::text::FALLBACK, size.max(1.0)).bounds.width();
    let mut lignes: Vec<String> = Vec::new();
    let mut coupe = false;
    let paras: Vec<&str> = text.trim().split('\n').collect();
    'paragraphes: for (pi, para) in paras.iter().enumerate() {
        let mut ligne = String::new();
        for mot in para.split_whitespace() {
            let essai = if ligne.is_empty() { mot.to_string() } else { format!("{ligne} {mot}") };
            if ligne.is_empty() || mesure(&essai) <= width {
                ligne = essai;
                continue;
            }
            lignes.push(std::mem::take(&mut ligne));
            if lignes.len() >= max {
                coupe = true;
                break 'paragraphes;
            }
            ligne = mot.to_string();
        }
        lignes.push(ligne);
        if lignes.len() >= max {
            coupe = pi + 1 < paras.len();
            break;
        }
    }
    if coupe {
        if let Some(l) = lignes.last_mut() {
            while !l.is_empty() && mesure(&format!("{l}…")) > width {
                l.pop();
            }
            *l = format!("{}…", l.trim_end());
        }
    }
    lignes.retain(|l| !l.is_empty());
    lignes
}
/// La planche des plans `plans` (indices du projet) avec les réglages de la vue Vignettes.
pub fn construire(doc: &Document, plans: &[usize], s: &BoardSettings, opt: &Options, annotations: &Annotations) -> Planche {
    // Un projet qui n'a que ces plans : la planche se range comme sur l'écran, avec les mêmes colonnes.
    let mut sub = Document::new(1, 1, doc.fps);
    sub.frame = doc.frame;
    sub.sequences = doc.sequences.clone();
    sub.scenes = doc.scenes.clone();
    sub.panels = plans.iter().filter(|&&i| i < doc.panels.len()).map(|&i| crate::exportrender::plan_leger(&doc.panels[i])).collect();
    let plans: Vec<usize> = plans.iter().copied().filter(|&i| i < doc.panels.len()).collect();
    // Les notes d'abord (leur nombre de lignes règle la hauteur des rangées).
    let largeur_note = (s.card_w.clamp(board::CARD_MIN, board::CARD_MAX) - 2.0).max(10.0);
    let mut notes: std::collections::HashMap<u64, (Vec<String>, f32)> = std::collections::HashMap::new();
    let mut lignes = std::collections::HashMap::new();
    if opt.notes {
        for p in &sub.panels {
            let marks = p.marks.iter().map(|m| (m.id, m.notes.as_str()));
            for (id, texte) in std::iter::once((p.id, p.notes.as_str())).chain(marks) {
                if texte.trim().is_empty() {
                    continue;
                }
                let taille = board::note_font_size(texte.chars().count());
                let l = lignes_de(texte, taille, largeur_note, board::NOTE_MAX_LINES);
                if !l.is_empty() {
                    lignes.insert(id, l.len().clamp(1, board::NOTE_MAX_LINES));
                    notes.insert(id, (l, taille));
                }
            }
        }
    }
    // (Même rangement qu'à l'écran, sans les cases pâles : elles ne sont pas exportées.)
    let layout = board::layout_with(&sub, s, &lignes, false, &board::Options::default());
    let cartes: Vec<Carte> = layout
        .cards
        .iter()
        .enumerate()
        .map(|(k, c)| {
            let plan_reel = plans[c.panel];
            let (note, note_taille) = notes.get(&c.id).cloned().unwrap_or_default();
            let etiquette = (c.starts_plan() && opt.numeros).then(|| doc.scene_label(c.scene));
            let mouvement = if opt.cadres { layout.move_of(k).cloned() } else { None };
            Carte { place: *c, plan_reel, etiquette, note, note_taille, mouvement }
        })
        .collect();
    let mut cadre = layout.pages.iter().fold(Rect::NOTHING, |r, p| r.union(*p));
    let annotations = if opt.annotations { annotations.clone() } else { Annotations::default() };
    if opt.annotations && !opt.marie {
        if let Some(e) = annotations.etendue(&layout) {
            // (Ce qui déborde des pages se voit, dans une limite raisonnable.)
            cadre = cadre.union(e.expand(4.0).intersect(cadre.expand(cadre.width().max(cadre.height()) * 0.5)));
        }
    }
    Planche { layout, cartes, annotations, cadre }
}

/// L'échelle (pixels par unité de la planche), la taille de l'image, et ce qui a été réduit pour rester dans les bornes.
pub fn dimensions(cadre: Rect, res: Resolution) -> (f32, [u32; 2], Option<String>) {
    let (w, h) = (cadre.width().max(1.0), cadre.height().max(1.0));
    let voulue = match res {
        Resolution::Dpi(d) => d.clamp(30, 1200) as f32 / 96.0,
        Resolution::Largeur(px) => px.clamp(200, 40_000) as f32 / w,
    };
    let mut s = voulue;
    s = s.min(MAX_COTE / w).min(MAX_COTE / h).min((MAX_PIXELS / (w * h)).sqrt());
    let reduit = (s < voulue * 0.999).then(|| format!("Image réduite aux bornes ({} × {} pixels au lieu de {} × {}).", (w * s).ceil(), (h * s).ceil(), (w * voulue).ceil(), (h * voulue).ceil()));
    (s, [(w * s).ceil().max(1.0) as u32, (h * s).ceil().max(1.0) as u32], reduit)
}

/// La taille en pixels de l'image de la vignette `k`, à l'échelle `s`.
pub fn taille_vignette(p: &Planche, k: usize, s: f32) -> [u32; 2] {
    let r = rect_px(p.cartes[k].place.thumb, p.cadre, s);
    [(r[2] - r[0]).max(1) as u32, (r[3] - r[1]).max(1) as u32]
}

/// Un rectangle de la planche en pixels : x0, y0, x1, y1.
fn rect_px(r: Rect, cadre: Rect, s: f32) -> [i32; 4] {
    let f = |v: f32, o: f32| ((v - o) * s).round() as i32;
    [f(r.left(), cadre.left()), f(r.top(), cadre.top()), f(r.right(), cadre.left()), f(r.bottom(), cadre.top())]
}

// ------------------------------------------------------------------------------------ le dessin

pub(crate) fn peinture(c: [u8; 3], alpha: u8) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c[0], c[1], c[2], alpha);
    p.anti_alias = true;
    p
}

pub(crate) fn remplir(pix: &mut Pixmap, r: [i32; 4], c: [u8; 3]) {
    if r[2] <= r[0] || r[3] <= r[1] {
        return;
    }
    if let Some(rect) = tiny_skia::Rect::from_xywh(r[0] as f32, r[1] as f32, (r[2] - r[0]) as f32, (r[3] - r[1]) as f32) {
        let mut paint = peinture(c, 255);
        paint.anti_alias = false;
        pix.fill_rect(rect, &paint, Transform::identity(), None);
    }
}

pub(crate) fn contour(pix: &mut Pixmap, r: [i32; 4], c: [u8; 3], epaisseur: f32) {
    let mut pb = PathBuilder::new();
    let (x0, y0, x1, y1) = (r[0] as f32, r[1] as f32, r[2] as f32, r[3] as f32);
    pb.move_to(x0, y0);
    pb.line_to(x1, y0);
    pb.line_to(x1, y1);
    pb.line_to(x0, y1);
    pb.close();
    if let Some(path) = pb.finish() {
        let mut paint = peinture(c, 255);
        paint.anti_alias = false;
        pix.stroke_path(&path, &paint, &Stroke { width: epaisseur, ..Default::default() }, Transform::identity(), None);
    }
}

/// Écrit `texte` à `x`, `y` (haut gauche) à `taille` pixels.
pub(crate) fn ecrire(pix: &mut Pixmap, texte: &str, x: f32, y: f32, taille: f32, c: [u8; 3]) {
    if taille < 2.0 || texte.trim().is_empty() {
        return;
    }
    let lay = crate::text::layout(texte, crate::text::FALLBACK, taille);
    let mut pb = PathBuilder::new();
    for glyphe in &lay.glyphs {
        for contour in glyphe {
            let mut it = contour.iter();
            if let Some(p) = it.next() {
                pb.move_to(x + p[0], y + p[1]);
                for q in it {
                    pb.line_to(x + q[0], y + q[1]);
                }
                pb.close();
            }
        }
    }
    if let Some(path) = pb.finish() {
        pix.fill_path(&path, &peinture(c, 255), FillRule::Winding, Transform::identity(), None);
    }
}

/// Un trait d'annotation (points déjà en pixels, rayons compris) : des segments à bouts ronds.
pub(crate) fn trait_rond(pix: &mut Pixmap, pts: &[(f32, f32, f32)], c: [u8; 3]) {
    let paint = peinture(c, 255);
    match pts {
        [] => {}
        [(x, y, r)] => {
            if let Some(path) = PathBuilder::from_circle(*x, *y, r.max(0.5)) {
                pix.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
            }
        }
        _ => {
            for w in pts.windows(2) {
                let mut pb = PathBuilder::new();
                pb.move_to(w[0].0, w[0].1);
                pb.line_to(w[1].0, w[1].1);
                if let Some(path) = pb.finish() {
                    let stroke = Stroke { width: ((w[0].2 + w[1].2)).max(1.0), line_cap: LineCap::Round, line_join: LineJoin::Round, ..Default::default() };
                    pix.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                }
            }
        }
    }
}

/// La planche dessinée : `vignettes[k]` est l'image de la vignette `k` (n'importe quelle taille : elle est ramenée à celle de sa case).
/// Rend l'image RVB, à l'échelle `s`.
pub fn composer(p: &Planche, vignettes: &[RgbImage], opt: &Options, s: f32) -> Result<RgbImage, String> {
    let (w, h) = ((p.cadre.width() * s).ceil().max(1.0) as u32, (p.cadre.height() * s).ceil().max(1.0) as u32);
    let mut pix = Pixmap::new(w, h).ok_or("image trop grande pour la mémoire")?;
    pix.fill(Color::from_rgba8(TABLE[0], TABLE[1], TABLE[2], 255));
    for page in &p.layout.pages {
        remplir(&mut pix, rect_px(*page, p.cadre, s), [255, 255, 255]);
    }
    let epais = s.max(1.0).round();
    // Les barres de coupe, entre deux vignettes qui se suivent (pas devant la première case vide).
    if opt.barres {
        for b in &p.layout.bars {
            if b.after + 1 >= p.layout.cards.len() {
                continue;
            }
            let r = rect_px(b.bar, p.cadre, s);
            let largeur = (b.bar.width() * s).round().max(1.0) as i32;
            let cx = (r[0] + r[2]) / 2;
            remplir(&mut pix, [cx - largeur / 2, r[1], cx - largeur / 2 + largeur, r[3]], if b.cut { BARRE_NOIRE } else { BARRE_GRISE });
        }
    }
    for (k, c) in p.cartes.iter().enumerate() {
        let r = rect_px(c.place.thumb, p.cadre, s);
        let (tw, th) = ((r[2] - r[0]).max(1) as u32, (r[3] - r[1]).max(1) as u32);
        if let Some(img) = vignettes.get(k) {
            let img = if img.dimensions() != (tw, th) { image::imageops::resize(img, tw, th, image::imageops::FilterType::Triangle) } else { img.clone() };
            let mut rgba = Vec::with_capacity((tw * th * 4) as usize);
            for px in img.pixels() {
                rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
            if let Some(cell) = tiny_skia::IntSize::from_wh(tw, th).and_then(|sz| Pixmap::from_vec(rgba, sz)) {
                pix.draw_pixmap(r[0], r[1], cell.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
            }
        } else {
            remplir(&mut pix, r, [255, 255, 255]);
        }
        contour(&mut pix, [r[0] - 1, r[1] - 1, r[2] + 1, r[3] + 1], FILET, 1.0);
        // Les cadres de caméra, comme à l'écran : fins, bleus, avec leur lettre, reliés par la flèche ou les coins. (L'image de la case
        // est celle de A ; le reste de l'empreinte est le papier : seuls les cadres s'y dessinent.)
        if let Some(mv) = c.mouvement.as_ref().filter(|_| opt.cadres) {
            let trait_ = (epais * 1.2).max(1.0);
            let fin = |pix: &mut Pixmap, a: Pos2, b: Pos2| {
                let mut pb = PathBuilder::new();
                pb.move_to((a.x - p.cadre.left()) * s, (a.y - p.cadre.top()) * s);
                pb.line_to((b.x - p.cadre.left()) * s, (b.y - p.cadre.top()) * s);
                if let Some(path) = pb.finish() {
                    pix.stroke_path(&path, &peinture(CAMERA, 255), &Stroke { width: trait_, line_cap: LineCap::Round, ..Default::default() }, Transform::identity(), None);
                }
            };
            for t in &mv.tuiles {
                if t.rect != c.place.thumb {
                    contour(&mut pix, rect_px(t.rect, p.cadre, s), [205, 205, 205], 1.0);
                }
            }
            for cadre in &mv.cadres {
                let q = rect_px(cadre.rect, p.cadre, s);
                contour(&mut pix, q, CAMERA, trait_);
                ecrire(&mut pix, &crate::vignettecam::lettre(cadre.rang).to_string(), q[0] as f32 + 3.0 * s.max(1.0), q[1] as f32 + 1.0 * s.max(1.0), (11.0 * s).max(8.0), CAMERA);
            }
            let rects: Vec<(Rect, i32)> = mv.cadres.iter().map(|c| (c.rect, c.bande)).collect();
            for lien in crate::vignettecam::liens(&rects, c.place.thumb.width()) {
                match lien {
                    crate::vignettecam::Lien::Coins(coins) => coins.iter().for_each(|&(a, b)| fin(&mut pix, a, b)),
                    crate::vignettecam::Lien::FlecheDessus { de, vers } | crate::vignettecam::Lien::FlecheVerticale { de, vers } => {
                        fin(&mut pix, de, vers);
                        let d = vers - de;
                        if d.length() > 1.0 {
                            let u = d.normalized();
                            let perp = vec2(-u.y, u.x);
                            fin(&mut pix, vers, vers - u * 7.0 + perp * 3.5);
                            fin(&mut pix, vers, vers - u * 7.0 - perp * 3.5);
                        }
                    }
                }
            }
        }
        // Le numéro du plan, au-dessus de sa première vignette.
        if let Some(e) = c.etiquette.as_ref().filter(|_| opt.numeros) {
            let l = c.place.label();
            let taille = 12.0 * s;
            let lay = crate::text::layout(e, crate::text::FALLBACK, taille.max(2.0));
            let y = (l.center().y - p.cadre.top()) * s - lay.bounds.height() * 0.5;
            ecrire(&mut pix, e, (l.left() + 1.0 - p.cadre.left()) * s, y, taille, ENCRE_NUMERO);
        }
        // La note, sous l'image.
        for (i, ligne) in c.note.iter().enumerate().filter(|_| opt.notes) {
            let y = (c.place.note.top() - p.cadre.top() + i as f32 * board::NOTE_LINE_H) * s;
            ecrire(&mut pix, ligne, (c.place.note.left() + 1.0 - p.cadre.left()) * s, y, c.note_taille * s, ENCRE_NOTE);
        }
    }
    // Les annotations, par-dessus tout.
    if opt.annotations {
        for i in 0..p.annotations.traces.len() {
            let Some(pts) = p.annotations.points_planche(&p.layout, i) else { continue };
            let px: Vec<(f32, f32, f32)> = pts.iter().map(|q| ((q.x - p.cadre.left()) * s, (q.y - p.cadre.top()) * s, q.r * s)).collect();
            trait_rond(&mut pix, &px, p.annotations.traces[i].color);
        }
    }
    // La marie-louise : tout ce qui est hors de l'intérieur des vignettes, traits qui débordent compris.
    if opt.marie {
        // (L'image de chaque case, et les tuiles de l'empreinte d'un mouvement de caméra : on y voit les cadres.)
        let en_pixels = |r: Rect| {
            let q = rect_px(r, p.cadre, s);
            Rect::from_min_max(pos2(q[0] as f32, q[1] as f32), pos2(q[2] as f32, q[3] as f32))
        };
        let trous: Vec<Rect> = p.cartes.iter().flat_map(|c| std::iter::once(en_pixels(c.place.thumb)).chain(c.mouvement.iter().flat_map(|m| m.tuiles.iter().map(|t| en_pixels(t.rect))).collect::<Vec<_>>())).collect();
        for r in crate::marie::hors_des_cases(Rect::from_min_size(Pos2::ZERO, vec2(w as f32, h as f32)), &trous) {
            remplir(&mut pix, [r.left().round() as i32, r.top().round() as i32, r.right().round() as i32, r.bottom().round() as i32], opt.marie_couleur);
        }
        // (Le filet de chaque vignette reste : il est dans la vignette.)
        for c in &p.cartes {
            let r = rect_px(c.place.thumb, p.cadre, s);
            contour(&mut pix, [r[0] - 1, r[1] - 1, r[2] + 1, r[3] + 1], FILET, 1.0);
        }
    }
    let mut out = RgbImage::new(w, h);
    for (dst, src) in out.pixels_mut().zip(pix.data().chunks_exact(4)) {
        *dst = Rgb([src[0], src[1], src[2]]);
    }
    Ok(out)
}

// ------------------------------------------------------------------------------------ les fichiers

pub(crate) fn jpeg(img: &RgbImage, qualite: u8) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, qualite).encode(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgb8).map_err(|e| e.to_string())?;
    Ok(out)
}

pub(crate) fn png(img: &RgbImage) -> Result<Vec<u8>, String> {
    use image::ImageEncoder;
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new_with_quality(&mut out, image::codecs::png::CompressionType::Fast, image::codecs::png::FilterType::Sub)
        .write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgb8)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

/// Les coupures d'une planche de hauteur `[haut, bas]` en feuilles de `hauteur` unités : de préférence dans l'écart entre deux pages de la
/// planche (`pages`), sinon à la hauteur de la feuille.
pub fn coupures(haut: f32, bas: f32, pages: &[Rect], hauteur: f32) -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    let mut debut = haut;
    let ecarts: Vec<f32> = pages.windows(2).map(|w| (w[0].bottom() + w[1].top()) * 0.5).collect();
    while bas - debut > 1e-3 {
        let limite = debut + hauteur;
        if limite >= bas - 1e-3 {
            out.push((debut, bas));
            break;
        }
        let fin = ecarts.iter().copied().filter(|&e| e > debut + 1.0 && e <= limite).fold(f32::NAN, f32::max);
        let fin = if fin.is_nan() { limite } else { fin };
        out.push((debut, fin));
        debut = fin;
    }
    out
}

/// Les octets du fichier de sortie pour l'image `img` (échelle `s`) de la planche `p`.
pub fn encoder(p: &Planche, img: &RgbImage, sortie: Sortie, titre: &str, s: f32) -> Result<Vec<u8>, String> {
    match sortie {
        Sortie::Png => png(img),
        Sortie::Jpeg => jpeg(img, 92),
        Sortie::PdfUnePage => {
            // 1 unité de la planche = 1/96 de pouce = 0,75 point ; une page de PDF va jusqu'à 14 400 points.
            let k = (0.75_f32).min(14_400.0 / p.cadre.width().max(p.cadre.height()).max(1.0));
            let (pw, ph) = (p.cadre.width() * k, p.cadre.height() * k);
            let mut pdf = crate::pdf::PdfFile::new();
            let id = pdf.add_jpeg(img.width(), img.height(), &jpeg(img, 88)?);
            pdf.add_page_sized(pw, ph, crate::pdf::op_image(ph, id, 0.0, 0.0, pw, ph).as_bytes(), &[id]);
            Ok(pdf.finish(titre))
        }
        Sortie::PdfA4 | Sortie::PdfA3 => {
            let (pw, ph) = if sortie == Sortie::PdfA4 { (842.0_f32, 595.0_f32) } else { (1191.0, 842.0) };
            let (marge, pied) = (20.0_f32, 16.0_f32);
            let (zone_w, zone_h) = (pw - 2.0 * marge, ph - 2.0 * marge - pied);
            // La planche à la largeur de la page (sans jamais être agrandie de plus d'un tiers).
            let k = (zone_w / p.cadre.width().max(1.0)).min(1.0);
            let feuille = zone_h / k;
            let morceaux = coupures(p.cadre.top(), p.cadre.bottom(), &p.layout.pages, feuille);
            let mut pdf = crate::pdf::PdfFile::new();
            let n = morceaux.len();
            for (i, (y0, y1)) in morceaux.iter().enumerate() {
                let (r0, r1) = ((((y0 - p.cadre.top()) * s).round() as u32).min(img.height().saturating_sub(1)), (((y1 - p.cadre.top()) * s).round() as u32).min(img.height()));
                let (r0, r1) = (r0, r1.max(r0 + 1));
                let tuile = image::imageops::crop_imm(img, 0, r0, img.width(), r1 - r0).to_image();
                let id = pdf.add_jpeg(tuile.width(), tuile.height(), &jpeg(&tuile, 88)?);
                let (w_pt, h_pt) = (p.cadre.width() * k, (y1 - y0) * k);
                let x = marge + (zone_w - w_pt) * 0.5;
                let mut ops = crate::pdf::op_image(ph, id, x, marge, w_pt, h_pt);
                ops += &crate::pdf::op_text(ph, marge, ph - marge * 0.5, &format!("{titre} · page {} / {n}", i + 1), 8.0, false, 0.4);
                pdf.add_page_sized(pw, ph, ops.as_bytes(), &[id]);
            }
            Ok(pdf.finish(titre))
        }
    }
}

/// Le compte rendu d'un export réussi.
pub fn message_de_fin(sortie: Sortie, cartes: usize, dims: [u32; 2], reduit: Option<&str>, chemin: &std::path::Path) -> String {
    let mut m = format!("Planche exportée ({}, {cartes} vignette{}, {} × {} pixels) : {}", sortie.nom(), if cartes > 1 { "s" } else { "" }, dims[0], dims[1], chemin.display());
    if let Some(r) = reduit {
        m += &format!(" {r}");
    }
    m
}

#[cfg(test)]
#[path = "planche_tests.rs"]
mod tests;
