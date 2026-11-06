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
//! - **Options** : numéros de plan, notes (une ligne par champ coché sous les images, « … » au bout), barres de coupe, cadres de caméra
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
use crate::boardsens::{Champ, Sens, Textes};
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
    /// Le passe-partout autour des cases (case « Passe-partout » de la fenêtre, décochée d'origine) : sa couleur, et son opacité (pour cent),
    /// celle de l'écran (posée par `App::planche_job`) ; 100 : opaque.
    pub marie: bool,
    pub marie_couleur: [u8; 3],
    pub marie_opacite: u8,
    /// La couleur des cadres de caméra : celle de l'écran (Préférences > Vignettes), posée par `App::planche_job`.
    pub couleur_cadre: [u8; 3],
    /// Le sens de lecture de la planche et les champs de texte montrés à côté des cases : ceux de l'écran (panneau « Vignettes »), posés
    /// par `App::planche_job`. D'origine : en lignes, les notes seules.
    pub sens: Sens,
    pub textes: Textes,
    /// Une légende sous le numéro de chaque case : le nom du plan selon le modèle de noms (`SQ01_PL012A`, `_C02` pour les cases d'après) et sa durée. Elle remplace le
    /// numéro seul. Décochée d'origine (la fenêtre la coche).
    pub legendes: bool,
    /// Le bloc de titre en haut de chaque page du PDF (série, épisode, séquence, version, date, page n / N) ; sans effet pour une image.
    pub bloc_titre: bool,
    /// Ce que dit le bloc de titre : posé par `App::planche_job` d'après le projet.
    pub entete: crate::entetepdf::Entete,
    /// La marge d'une case, en part de sa largeur (Préférences > Vignettes) : ce qui y dépasse du cadre sort sur le papier, comme à l'écran,
    /// sauf avec le passe-partout. Posée par `App::planche_options`.
    pub debord: f32,
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
            marie_opacite: 100,
            couleur_cadre: crate::vignettesreglages::CADRE_DEFAUT,
            sens: Sens::default(),
            textes: Textes::default(),
            legendes: false,
            bloc_titre: false,
            entete: Default::default(),
            debord: board::RING_FRACTION,
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
const ENCRE_LEGENDE: [u8; 3] = [30, 30, 30];

/// Le texte `t`, coupé par « … » pour tenir dans `largeur` à la taille `taille`.
pub(crate) fn tronquer(t: &str, taille: f32, largeur: f32) -> String {
    let mesure = |s: &str| crate::text::layout(s, crate::text::FALLBACK, taille.max(1.0)).bounds.width();
    if mesure(t) <= largeur {
        return t.to_string();
    }
    let mut s: String = t.to_string();
    while !s.is_empty() && mesure(&format!("{s}…")) > largeur {
        s.pop();
    }
    format!("{}…", s.trim_end())
}

/// Une vignette à exporter.
#[derive(Clone, Debug)]
pub struct Carte {
    pub place: CardPlace,
    /// L'indice du plan dans le vrai projet.
    pub plan_reel: usize,
    /// Le numéro du plan, s'il commence là.
    pub etiquette: Option<String>,
    /// Les champs montrés (action, dialogue, notes) qui ont un texte : leur taille et leurs lignes (trois au plus chacun, « … » au bout).
    /// Quand plusieurs champs sont montrés, la première ligne de chacun commence par son étiquette.
    pub champs: Vec<(Champ, f32, Vec<String>)>,
    /// L'empreinte du mouvement de caméra (cadres A, B, C…, à l'échelle des cases), si la caméra bouge et que les cadres sont demandés.
    pub mouvement: Option<board::MoveBoard>,
    /// La légende (`Options::legendes`) : le nom du plan ou de la case, et sa durée (`12s 04`).
    pub legende: Option<(String, String)>,
}

/// Tout ce qu'il faut pour dessiner la planche, sans pixel.
#[derive(Clone, Debug)]
pub struct Planche {
    pub layout: BoardLayout,
    pub cartes: Vec<Carte>,
    /// Les titres des séquences (leur place sur la planche, leur nom) : rien quand les plans exportés sont dans une seule séquence.
    pub titres: Vec<(Rect, String)>,
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
    // Les textes d'abord : une ligne par champ coché, coupée par « … » (sans l'option Notes : aucune ligne, la place se resserre).
    let largeur_note = (s.card_w.clamp(board::CARD_MIN, board::CARD_MAX) - 2.0).max(10.0);
    let textes = if opt.notes { opt.textes } else { Textes { action: false, dialogue: false, notes: false } };
    let avec_etiquettes = textes.etiquettes();
    let mut notes: std::collections::HashMap<u64, Vec<(Champ, f32, Vec<String>)>> = std::collections::HashMap::new();
    if opt.notes {
        for p in &sub.panels {
            let marks = p.marks.iter().map(|m| (m.id, [m.action.as_str(), m.dialogue.as_str(), m.notes.as_str()]));
            for (id, trois) in std::iter::once((p.id, [p.action.as_str(), p.dialogue.as_str(), p.notes.as_str()])).chain(marks) {
                let mut champs: Vec<(Champ, f32, Vec<String>)> = Vec::new();
                for (champ, texte) in Champ::TOUS.into_iter().zip(trois).filter(|(c, t)| textes.montre(*c) && !t.trim().is_empty()) {
                    let taille = board::note_font_size(texte.chars().count());
                    let texte = crate::boardsens::une_ligne(texte);
                    let brut = if avec_etiquettes { format!("{} {texte}", champ.etiquette()) } else { texte };
                    let l = lignes_de(&brut, taille, largeur_note, 1);
                    if !l.is_empty() {
                        champs.push((champ, taille, l));
                    }
                }
                if !champs.is_empty() {
                    notes.insert(id, champs);
                }
            }
        }
    }
    // (Même rangement qu'à l'écran, sans les cases pâles : elles ne sont pas exportées.)
    // (Plusieurs séquences : chacune a son titre, sa rangée de départ ; `planchesequences.rs`.)
    let layout = crate::planchesequences::layout_export(&sub, s, &board::Options { sens: opt.sens, textes, ..Default::default() });
    let titres: Vec<(Rect, String)> = layout.groupes.iter().filter_map(|g| g.titre.map(|t| (t, doc.sequence_label(g.sequence)))).collect();
    // Les légendes : le nom de chaque plan (ou case) selon le modèle de noms du projet, et sa durée. (Les noms de tous les plans d'un coup : pas le carré du nombre de plans.)
    let legendes: Option<std::collections::HashMap<u64, (String, String)>> = opt.legendes.then(|| {
        let noms = doc.noms_de_tous_les_plans();
        let mut m = std::collections::HashMap::new();
        for c in &layout.cards {
            let plan_reel = plans[c.panel];
            let spans = board::plan_boards(doc, plan_reel);
            let Some(rang) = spans.iter().position(|s| s.0 == c.id) else { continue };
            let (_, a, b) = spans[rang];
            let nom = if rang == 0 { noms[plan_reel].clone() } else { doc.nom_de_plan_avec(plan_reel, Some(rang + 1), None) };
            m.insert(c.id, (nom, crate::storyboard::duration_label(b - a, doc.fps)));
        }
        m
    });
    let cartes: Vec<Carte> = layout
        .cards
        .iter()
        .enumerate()
        .map(|(k, c)| {
            let plan_reel = plans[c.panel];
            let champs = notes.get(&c.id).cloned().unwrap_or_default();
            let etiquette = (c.starts_plan() && opt.numeros).then(|| doc.scene_label(c.scene));
            // (Avec le passe-partout, l'ouverture d'un plan à mouvement se sait même sans les cadres.)
            let mouvement = if opt.cadres || opt.marie { layout.move_of(k).cloned() } else { None };
            Carte { place: *c, plan_reel, etiquette, champs, mouvement, legende: legendes.as_ref().and_then(|l| l.get(&c.id).cloned()) }
        })
        .collect();
    let mut cadre = layout.pages.iter().fold(Rect::NOTHING, |r, p| r.union(*p));
    // Le dessin hors des cases (calque « Hors cadres ») sort toujours dans la planche ; les autres annotations, si on le demande.
    let annotations = if opt.annotations {
        annotations.clone()
    } else {
        let planche: Vec<_> = annotations.calques.iter().filter(|c| c.planche).cloned().collect();
        let traces = annotations.traces.iter().filter(|t| planche.iter().any(|c| c.id == t.calque)).cloned().collect();
        Annotations { calques: planche, traces, ..Default::default() }
    };
    if (opt.annotations || !annotations.traces.is_empty()) && !opt.marie {
        if let Some(e) = annotations.etendue(&layout) {
            // (Ce qui déborde des pages se voit, dans une limite raisonnable.)
            cadre = cadre.union(e.expand(4.0).intersect(cadre.expand(cadre.width().max(cadre.height()) * 0.5)));
        }
    }
    // Ce qui dépasse d'une case en bordure de page sort aussi (partie 111) : seulement là où le plan a du dessin, jamais plus loin que sa marge.
    if !opt.marie {
        for (k, c) in layout.cards.iter().enumerate() {
            if layout.move_of(k).is_some() {
                continue;
            }
            let plan_reel = plans[c.panel];
            let Some(aff) = crate::horschamp::aff_de_camera(&doc.panel_camera(plan_reel, c.frame), doc.frame, c.thumb) else { continue };
            let aire = crate::render::plan_bounds(doc, &crate::render::panel_plan_at(doc, plan_reel, c.frame, None));
            let encre = Rect::from_two_pos(aff.applique(eframe::egui::pos2(aire.x0 as f32, aire.y0 as f32)), aff.applique(eframe::egui::pos2(aire.x1 as f32, aire.y1 as f32)));
            let r = encre.intersect(c.thumb.expand(opt.debord * c.thumb.width()));
            if r.is_positive() {
                cadre = cadre.union(r);
            }
        }
    }
    Planche { layout, cartes, titres, annotations, cadre }
}

/// Le titre d'une séquence coupé par « … » pour tenir dans `largeur` pixels (tel quel s'il tient).
pub(crate) fn coupe_au_titre(nom: &str, taille: f32, largeur: f32) -> String {
    let mesure = |t: &str| crate::text::layout(t, crate::text::FALLBACK, taille).bounds.width();
    if mesure(nom) <= largeur {
        return nom.to_string();
    }
    let mut cars: Vec<char> = nom.chars().collect();
    while cars.len() > 1 {
        cars.pop();
        let essai = format!("{}…", cars.iter().collect::<String>().trim_end());
        if mesure(&essai) <= largeur {
            return essai;
        }
    }
    "…".to_string()
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

/// `remplir` à l'opacité `alpha` (0 à 255), par-dessus ce qui est déjà là.
pub(crate) fn remplir_a(pix: &mut Pixmap, r: [i32; 4], c: [u8; 3], alpha: u8) {
    if r[2] <= r[0] || r[3] <= r[1] || alpha == 0 {
        return;
    }
    if let Some(rect) = tiny_skia::Rect::from_xywh(r[0] as f32, r[1] as f32, (r[2] - r[0]) as f32, (r[3] - r[1]) as f32) {
        let mut paint = peinture(c, alpha);
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
    trait_rond_a(pix, pts, c, 255);
}

/// `trait_rond` à l'opacité `alpha` (le dessin hors des cases garde celle du pinceau).
pub(crate) fn trait_rond_a(pix: &mut Pixmap, pts: &[(f32, f32, f32)], c: [u8; 3], alpha: u8) {
    let paint = peinture(c, alpha);
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
#[cfg(test)]
pub fn composer(p: &Planche, vignettes: &[RgbImage], opt: &Options, s: f32) -> Result<RgbImage, String> {
    composer_avec_tuiles(p, vignettes, &[], opt, s)
}

/// Le dessin d'une case hors de son image : pour un plan à mouvement de caméra, sur toute son enveloppe (sa « tuile », comme à l'écran) ;
/// sinon, sa marge seule. Pixels RVBA prémultipliés, `taille` pixels, dont `marge` tout autour du rectangle de la tuile ou de la case.
#[derive(Clone, Debug)]
pub struct TuileImage {
    pub taille: [u32; 2],
    pub marge: u32,
    pub rvba: Vec<u8>,
}

/// Comme `composer`, avec `tuiles[k]`, le dessin de la vignette `k` hors de son image : posé sur le papier, sous **toutes** les images (rien :
/// la vignette seule).
pub fn composer_avec_tuiles(p: &Planche, vignettes: &[RgbImage], tuiles: &[Option<TuileImage>], opt: &Options, s: f32) -> Result<RgbImage, String> {
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
            // (Ni devant la première case vide, ni entre deux séquences : le titre les sépare.)
            if p.layout.vers_pale(b.after) {
                continue;
            }
            let r = rect_px(b.bar, p.cadre, s);
            let couleur = if b.cut { BARRE_NOIRE } else { BARRE_GRISE };
            if p.layout.sens.japonais() {
                // À la japonaise, la barre est horizontale (les cases d'une colonne se suivent de haut en bas).
                let epaisseur = (b.bar.height() * s).round().max(1.0) as i32;
                let cy = (r[1] + r[3]) / 2;
                remplir(&mut pix, [r[0], cy - epaisseur / 2, r[2], cy - epaisseur / 2 + epaisseur], couleur);
            } else {
                let largeur = (b.bar.width() * s).round().max(1.0) as i32;
                let cx = (r[0] + r[2]) / 2;
                remplir(&mut pix, [cx - largeur / 2, r[1], cx - largeur / 2 + largeur, r[3]], couleur);
            }
        }
    }
    // Les titres des séquences : le nom, gras, et un filet dessous.
    for (t, nom) in &p.titres {
        let r = rect_px(*t, p.cadre, s);
        let taille = (22.0 * s).max(8.0);
        // (Un titre plus long que la séquence est coupé net par « … », comme à l'écran, `docs/INTERFACE.md` partie 106.)
        let nom = &coupe_au_titre(nom, taille, (r[2] - r[0]) as f32);
        let lay = crate::text::layout(nom, crate::text::FALLBACK, taille);
        let y = r[3] as f32 - lay.bounds.height() - 2.0 * s;
        for dx in [0.0, (taille * 0.035).max(0.6)] {
            ecrire(&mut pix, nom, r[0] as f32 + dx, y, taille, [15, 15, 15]);
        }
        remplir(&mut pix, [r[0], r[3] + (2.0 * s) as i32, r[2], r[3] + (2.0 * s) as i32 + epais as i32], [200, 200, 200]);
    }
    // Le dessin hors des cases, sous les cases (comme sur l'écran).
    for i in p.annotations.ordre_de_peinture() {
        let t = &p.annotations.traces[i];
        if !p.annotations.calque(t.calque).is_some_and(|c| c.planche) {
            continue;
        }
        // Un trait au pinceau : ses pixels, rendus par le moteur comme dans une case (la texture du pinceau) ; sinon la ligne vectorielle.
        if let (Some(b), Some((_, case))) = (&t.brosse, crate::annotations::case_de(&p.layout, &t.anchor)) {
            if b.peindre_pixmap(&mut pix, &t.points, t.color, case, p.cadre, s) {
                continue;
            }
        }
        let Some(pts) = p.annotations.points_planche(&p.layout, i) else { continue };
        let px: Vec<(f32, f32, f32)> = pts.iter().map(|q| ((q.x - p.cadre.left()) * s, (q.y - p.cadre.top()) * s, q.r * s)).collect();
        trait_rond_a(&mut pix, &px, t.color, t.alpha);
    }
    // Les marges des cases, puis le dessin sur toute l'enveloppe des plans à mouvement : sur le papier, sous toutes les images, comme à l'écran.
    for mouvements in [false, true] {
        for (k, c) in p.cartes.iter().enumerate() {
            if c.mouvement.is_some() != mouvements {
                continue;
            }
            let Some(Some(t)) = tuiles.get(k) else { continue };
            let q = rect_px(c.mouvement.as_ref().map_or(c.place.thumb, |m| m.tuile.rect), p.cadre, s);
            if let Some(cell) = tiny_skia::IntSize::from_wh(t.taille[0], t.taille[1]).and_then(|sz| Pixmap::from_vec(t.rvba.clone(), sz)) {
                pix.draw_pixmap(q[0] - t.marge as i32, q[1] - t.marge as i32, cell.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
            }
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
                    pix.stroke_path(&path, &peinture(opt.couleur_cadre, 255), &Stroke { width: trait_, line_cap: LineCap::Round, ..Default::default() }, Transform::identity(), None);
                }
            };
            let distincts = crate::vignettecam::cadres_distincts(&mv.quads());
            for (cadre, _) in mv.cadres.iter().zip(&distincts).filter(|(_, d)| **d) {
                for j in 0..4 {
                    fin(&mut pix, cadre.coins[j], cadre.coins[(j + 1) % 4]);
                }
                let q = rect_px(cadre.rect, p.cadre, s);
                ecrire(&mut pix, &crate::vignettecam::lettre(cadre.rang).to_string(), q[0] as f32 + 3.0 * s.max(1.0), q[1] as f32 + 1.0 * s.max(1.0), (11.0 * s).max(8.0), opt.couleur_cadre);
            }
            for lien in crate::vignettecam::liens(&mv.quads(), c.place.thumb) {
                match lien {
                    crate::vignettecam::Lien::Bord(a, b) => fin(&mut pix, a, b),
                    crate::vignettecam::Lien::Fleche { de, vers } => {
                        // La même fine flèche qu'à l'écran : un petit chevron, moins opaque que les cadres.
                        let d = vers - de;
                        if d.length() > 1.0 {
                            let u = d.normalized();
                            let [h1, h2] = crate::vignettecam::chevron(vers, u, (d.length() * 0.25).clamp(3.0, 5.5));
                            for (a, b) in [(de, vers), (vers, h1), (vers, h2)] {
                                let mut pb = PathBuilder::new();
                                pb.move_to((a.x - p.cadre.left()) * s, (a.y - p.cadre.top()) * s);
                                pb.line_to((b.x - p.cadre.left()) * s, (b.y - p.cadre.top()) * s);
                                if let Some(path) = pb.finish() {
                                    pix.stroke_path(&path, &peinture(opt.couleur_cadre, crate::vignettecam::FLECHE_ALPHA), &Stroke { width: epais.max(1.0), line_cap: LineCap::Round, ..Default::default() }, Transform::identity(), None);
                                }
                            }
                        }
                    }
                }
            }
        }
        // La légende, au-dessus de la vignette : le nom du plan (ou de la case) à gauche, sa durée à droite ; elle tient lieu du numéro seul.
        if let Some((nom, duree)) = c.legende.as_ref().filter(|_| opt.legendes) {
            let l = c.place.label();
            let taille = (10.0 * s).max(6.0);
            let largeur = (l.width() - 2.0) * s;
            let mesure = |t: &str, tl: f32| crate::text::layout(t, crate::text::FALLBACK, tl.max(1.0)).bounds.width();
            let duree_w = mesure(duree, taille) + 6.0 * s;
            let nom = tronquer(nom, taille, (largeur - duree_w).max(10.0));
            let y = (l.bottom() - p.cadre.top()) * s - taille * 1.45;
            let x = (l.left() + 1.0 - p.cadre.left()) * s;
            ecrire(&mut pix, &nom, x, y, taille, ENCRE_LEGENDE);
            ecrire(&mut pix, duree, x + largeur - mesure(duree, taille), y, taille, ENCRE_NUMERO);
        } else if let Some(e) = c.etiquette.as_ref().filter(|_| opt.numeros) {
            let l = c.place.label();
            let taille = 12.0 * s;
            let lay = crate::text::layout(e, crate::text::FALLBACK, taille.max(2.0));
            let y = (l.center().y - p.cadre.top()) * s - lay.bounds.height() * 0.5;
            ecrire(&mut pix, e, (l.left() + 1.0 - p.cadre.left()) * s, y, taille, ENCRE_NUMERO);
        }
        // Les textes, sous l'image (ou à sa droite, à la japonaise) : chaque champ montré l'un sous l'autre ; avec plusieurs champs, la
        // première ligne de chacun commence par son étiquette, dans la couleur du champ.
        let haut = c.place.note.top();
        let x = (c.place.note.left() + 1.0 - p.cadre.left()) * s;
        for (champ, taille, lignes) in c.champs.iter().filter(|_| opt.notes) {
            // La ligne du champ : son rang parmi les champs cochés (un champ sans texte garde sa ligne, vide).
            let Some(rang) = opt.textes.ligne_du(*champ) else { continue };
            for ligne in lignes {
                let y = (haut - p.cadre.top() + rang as f32 * board::NOTE_LINE_H) * s;
                match ligne.strip_prefix(champ.etiquette()).filter(|_| opt.textes.etiquettes()) {
                    Some(reste) => {
                        ecrire(&mut pix, champ.etiquette(), x, y, taille * s, champ.encre());
                        let decalage = crate::text::layout(&format!("{} ", champ.etiquette()), crate::text::FALLBACK, (taille * s).max(1.0)).bounds.width();
                        ecrire(&mut pix, reste.trim_start(), x + decalage, y, taille * s, ENCRE_NOTE);
                    }
                    None => ecrire(&mut pix, ligne, x, y, taille * s, ENCRE_NOTE),
                }
            }
        }
    }
    // Les annotations, par-dessus tout : calque par calque, du plus bas au plus haut (un calque caché n'y est pas).
    if opt.annotations {
        for i in p.annotations.ordre_de_peinture() {
            if p.annotations.calque(p.annotations.traces[i].calque).is_some_and(|c| c.planche) {
                continue;
            }
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
        let trous: Vec<Rect> = p.cartes.iter().map(|c| en_pixels(c.place.thumb)).collect();
        // (L'opacité est celle du passe-partout de l'écran : avec moins de 100 %, ce qui déborde se voit à travers.)
        let alpha = (f32::from(opt.marie_opacite.min(100)) * 2.55).round() as u8;
        // Un plan à mouvement s'ouvre sur tout ce que sa caméra balaie, pas sur sa seule case A (comme à l'écran : `marie_peindre`).
        let mut ouverture = tiny_skia::Mask::new(w, h).filter(|_| p.cartes.iter().any(|c| c.mouvement.as_ref().is_some_and(|m| !m.balayage.is_empty())));
        if let Some(mask) = ouverture.as_mut() {
            for poly in p.cartes.iter().filter_map(|c| c.mouvement.as_ref()).flat_map(|m| m.balayage.iter()).filter(|q| q.len() >= 3) {
                let pts: Vec<(f32, f32)> = poly.iter().map(|q| ((q.x - p.cadre.left()) * s, (q.y - p.cadre.top()) * s)).collect();
                // (Tous dans le même sens : la règle de l'enroulement en fait l'union.)
                let aire: f32 = (0..pts.len()).map(|i| pts[i].0 * pts[(i + 1) % pts.len()].1 - pts[(i + 1) % pts.len()].0 * pts[i].1).sum();
                let mut pb = PathBuilder::new();
                let ordre: Vec<(f32, f32)> = if aire >= 0.0 { pts } else { pts.into_iter().rev().collect() };
                pb.move_to(ordre[0].0, ordre[0].1);
                for q in &ordre[1..] {
                    pb.line_to(q.0, q.1);
                }
                pb.close();
                if let Some(path) = pb.finish() {
                    mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
                }
            }
            mask.invert();
        }
        for r in crate::marie::hors_des_cases(Rect::from_min_size(Pos2::ZERO, vec2(w as f32, h as f32)), &trous) {
            let q = [r.left().round() as i32, r.top().round() as i32, r.right().round() as i32, r.bottom().round() as i32];
            match ouverture.as_ref() {
                None => remplir_a(&mut pix, q, opt.marie_couleur, alpha),
                Some(mask) => {
                    if q[2] > q[0] && q[3] > q[1] && alpha > 0 {
                        if let Some(rect) = tiny_skia::Rect::from_xywh(q[0] as f32, q[1] as f32, (q[2] - q[0]) as f32, (q[3] - q[1]) as f32) {
                            let mut paint = peinture(opt.marie_couleur, alpha);
                            paint.anti_alias = false;
                            pix.fill_rect(rect, &paint, Transform::identity(), Some(mask));
                        }
                    }
                }
            }
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
#[cfg_attr(not(test), allow(dead_code))]
pub fn encoder(p: &Planche, img: &RgbImage, sortie: Sortie, titre: &str, s: f32) -> Result<Vec<u8>, String> {
    encoder_avec(p, img, sortie, titre, s, None)
}

/// Comme `encoder` ; `bloc` : le bloc de titre à écrire en haut de chaque page d'un PDF (sans effet pour une image).
pub fn encoder_avec(p: &Planche, img: &RgbImage, sortie: Sortie, titre: &str, s: f32, bloc: Option<&crate::entetepdf::Entete>) -> Result<Vec<u8>, String> {
    // Les polices du bloc : d'après tout ce qu'il écrira.
    let fonts_pour = |b: &crate::entetepdf::Entete| crate::pdffont::Fonts::choose(&format!("{} {titre}", b.tout_le_texte()));
    match sortie {
        Sortie::Png => png(img),
        Sortie::Jpeg => jpeg(img, 92),
        Sortie::PdfUnePage => {
            // 1 unité de la planche = 1/96 de pouce = 0,75 point ; une page de PDF va jusqu'à 14 400 points.
            let k = (0.75_f32).min(14_400.0 / p.cadre.width().max(p.cadre.height()).max(1.0));
            let (pw, ph_image) = (p.cadre.width() * k, p.cadre.height() * k);
            let mut pdf = crate::pdf::PdfFile::new();
            let id = pdf.add_jpeg(img.width(), img.height(), &jpeg(img, 88)?);
            let Some(b) = bloc else {
                pdf.add_page_sized(pw, ph_image, crate::pdf::op_image(ph_image, id, 0.0, 0.0, pw, ph_image).as_bytes(), &[id]);
                return Ok(pdf.finish(titre));
            };
            // Le bloc de titre au-dessus de la planche, à une taille qui suit celle de la page.
            let kb = (pw / 842.0).clamp(1.0, 6.0);
            let haut = 40.0 * kb;
            let ph = ph_image + haut;
            let mut fonts = fonts_pour(b);
            fonts.attach(&mut pdf);
            let mut ops = crate::pdf::op_image(ph, id, 0.0, haut, pw, ph_image);
            ops += &bloc_de_titre(&mut fonts, ph, pw, (20.0 * kb, 4.0 * kb), kb, b, "Page 1 / 1");
            pdf.add_page_sized(pw, ph, ops.as_bytes(), &[id]);
            fonts.write(&mut pdf);
            Ok(pdf.finish(titre))
        }
        Sortie::PdfA4 | Sortie::PdfA3 => {
            let (pw, ph) = if sortie == Sortie::PdfA4 { (842.0_f32, 595.0_f32) } else { (1191.0, 842.0) };
            let (marge, pied) = (20.0_f32, 16.0_f32);
            // (Le bloc de titre prend la hauteur de ses deux lignes en haut de chaque page.)
            let kb = (pw / 842.0).clamp(1.0, 2.0);
            let haut = if bloc.is_some() { 40.0 * kb } else { 0.0 };
            let (zone_w, zone_h) = (pw - 2.0 * marge, ph - 2.0 * marge - pied - haut);
            // La planche à la largeur de la page (sans jamais être agrandie de plus d'un tiers).
            let k = (zone_w / p.cadre.width().max(1.0)).min(1.0);
            let feuille = zone_h / k;
            let morceaux = coupures(p.cadre.top(), p.cadre.bottom(), &p.layout.pages, feuille);
            let mut pdf = crate::pdf::PdfFile::new();
            let n = morceaux.len();
            let mut fonts = bloc.map(fonts_pour);
            if let Some(f) = &mut fonts {
                f.attach(&mut pdf);
            }
            for (i, (y0, y1)) in morceaux.iter().enumerate() {
                let (r0, r1) = ((((y0 - p.cadre.top()) * s).round() as u32).min(img.height().saturating_sub(1)), (((y1 - p.cadre.top()) * s).round() as u32).min(img.height()));
                let (r0, r1) = (r0, r1.max(r0 + 1));
                let tuile = image::imageops::crop_imm(img, 0, r0, img.width(), r1 - r0).to_image();
                let id = pdf.add_jpeg(tuile.width(), tuile.height(), &jpeg(&tuile, 88)?);
                let (w_pt, h_pt) = (p.cadre.width() * k, (y1 - y0) * k);
                let x = marge + (zone_w - w_pt) * 0.5;
                let mut ops = crate::pdf::op_image(ph, id, x, marge + haut, w_pt, h_pt);
                ops += &crate::pdf::op_text(ph, marge, ph - marge * 0.5, &format!("{titre} · page {} / {n}", i + 1), 8.0, false, 0.4);
                if let (Some(f), Some(b)) = (&mut fonts, bloc) {
                    ops += &bloc_de_titre(f, ph, pw, (marge, marge), kb, b, &format!("Page {} / {n}", i + 1));
                }
                pdf.add_page_sized(pw, ph, ops.as_bytes(), &[id]);
            }
            if let Some(f) = &fonts {
                f.write(&mut pdf);
            }
            Ok(pdf.finish(titre))
        }
    }
}

/// Le bloc de titre d'une page : la série en gras et « Page n / N » à droite, puis « Épisode · Séquence · Version · date », et un filet. marge_x : la marge
/// gauche et droite ; kb : l'échelle du texte (la page est plus grande qu'une feuille A4).
pub(crate) fn bloc_de_titre(fonts: &mut crate::pdffont::Fonts, page_h: f32, pw: f32, (marge_x, haut): (f32, f32), kb: f32, e: &crate::entetepdf::Entete, page: &str) -> String {
    use crate::pdfmise::{op_filet, op_texte};
    let droite = pw - marge_x;
    let mut ops = op_texte(fonts, page_h, marge_x, haut + 14.0 * kb, &e.ligne1(), true, 13.0 * kb, 0.1);
    let largeur = fonts.width(page, false, 9.0 * kb);
    ops += &op_texte(fonts, page_h, droite - largeur, haut + 14.0 * kb, page, false, 9.0 * kb, 0.35);
    ops += &op_texte(fonts, page_h, marge_x, haut + 27.0 * kb, &e.ligne2(), false, 9.0 * kb, 0.35);
    ops += &op_filet(page_h, marge_x, droite, haut + 33.0 * kb, 0.6, 0.75);
    ops
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
