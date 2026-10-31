//! La mise en page du storyboard PDF « à la carte » (exports V1, 2026-10-30) : format de page (A4, A3, Lettre), portrait ou paysage, 1,
//! 2, 3, 4, 6, 8 ou 9 cases par page (la grille est calculée), les textes qu'on veut voir (action, dialogue, notes, durée, numéro), le
//! titre et la date en en-tête, les numéros de page. Les textes sont écrits avec une police embarquée (`pdffont.rs`) : accents et
//! tous les caractères de la police, aucun « ? ».
//!
//! L'ancienne mise en page (`pdf::storyboard_with` : A4 paysage, 6 cases, Helvetica) reste pour la version à imprimer de la relecture.

use serde::{Deserialize, Serialize};

use crate::entetepdf::{Entete, Revision, Studio};
use crate::pdf::{self, Board, BoardImage, PdfFile};
use crate::pdffont::Fonts;

/// Le format de la page.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Paper {
    #[default]
    A4,
    A3,
    Letter,
}

impl Paper {
    pub const ALL: [Paper; 3] = [Paper::A4, Paper::A3, Paper::Letter];

    pub fn label(self) -> &'static str {
        match self {
            Paper::A4 => "A4",
            Paper::A3 => "A3",
            Paper::Letter => "Lettre (US)",
        }
    }

    /// Largeur et hauteur en portrait, en points (1/72 de pouce).
    pub fn portrait(self) -> (f32, f32) {
        match self {
            Paper::A4 => (595.0, 842.0),
            Paper::A3 => (842.0, 1191.0),
            Paper::Letter => (612.0, 792.0),
        }
    }
}

/// Nombres de cases par page proposés.
pub const PER_PAGE_CHOICES: [usize; 7] = [1, 2, 3, 4, 6, 8, 9];

/// Les choix de la fenêtre du storyboard PDF.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PdfLayout {
    pub paper: Paper,
    pub landscape: bool,
    pub per_page: usize,
    pub action: bool,
    pub dialogue: bool,
    pub notes: bool,
    pub duration: bool,
    /// Le titre du plan (« Séquence 1 — plan 12A ») sous l'image.
    pub number: bool,
    /// Le titre du film et la date en haut de chaque page.
    pub header: bool,
    pub page_numbers: bool,
    /// L'en-tête de studio (`entetepdf.rs`) : la série, l'épisode, la séquence, la version (le nom du dernier envoi), la date, et « Page n / N » ; il remplace le
    /// titre et la date du haut de page. Décoché d'origine : la page garde l'en-tête d'avant.
    pub studio: bool,
    /// Marquer les plans modifiés ou ajoutés depuis le dernier envoi (un cadre et « MODIFIÉ » sur l'image). Sans envoi, rien n'est marqué.
    pub marquer_modifies: bool,
    /// Une page « Notes de révision » à la fin : les plans modifiés et les notes traitées depuis le dernier envoi. Décochée d'origine.
    pub notes_revision: bool,
}

impl Default for PdfLayout {
    fn default() -> Self {
        PdfLayout {
            paper: Paper::A4,
            landscape: true,
            per_page: 6,
            action: true,
            dialogue: true,
            notes: true,
            duration: true,
            number: true,
            header: true,
            page_numbers: true,
            studio: false,
            marquer_modifies: true,
            notes_revision: false,
        }
    }
}

impl PdfLayout {
    /// Le nombre de cases par page ramené à un choix permis.
    pub fn per_page_ok(&self) -> usize {
        if PER_PAGE_CHOICES.contains(&self.per_page) {
            self.per_page
        } else {
            *PER_PAGE_CHOICES.iter().min_by_key(|n| n.abs_diff(self.per_page)).unwrap_or(&6)
        }
    }

    /// Largeur et hauteur de la page, en points.
    pub fn page_size(&self) -> (f32, f32) {
        let (w, h) = self.paper.portrait();
        if self.landscape {
            (h, w)
        } else {
            (w, h)
        }
    }

    /// La largeur des images du PDF, en pixels : environ 216 points par pouce dans une case (au moins 600, au plus 2000).
    pub fn image_width(&self) -> u32 {
        let (pw, ph) = self.page_size();
        let (cols, _) = grid(self.per_page_ok(), self.landscape);
        let k = (pw.min(ph) / 595.0).clamp(1.0, 1.4);
        let cell_w = (pw - 56.0 * k - (cols as f32 - 1.0) * 16.0 * k) / cols as f32;
        ((cell_w * 3.0).round() as u32).clamp(600, 2000)
    }

    /// Une description courte (« A4 paysage, 6 cases par page »).
    pub fn label(&self) -> String {
        format!("{} {}, {} case{} par page", self.paper.label(), if self.landscape { "paysage" } else { "portrait" }, self.per_page_ok(), if self.per_page_ok() > 1 { "s" } else { "" })
    }
}

/// La grille (colonnes, rangées) de `per_page` cases sur une page paysage ou portrait.
pub fn grid(per_page: usize, landscape: bool) -> (usize, usize) {
    match (per_page, landscape) {
        (1, _) => (1, 1),
        (2, true) => (2, 1),
        (2, false) => (1, 2),
        (3, true) => (3, 1),
        (3, false) => (1, 3),
        (4, _) => (2, 2),
        (6, true) => (3, 2),
        (6, false) => (2, 3),
        (8, true) => (4, 2),
        (8, false) => (2, 4),
        (9, _) => (3, 3),
        (n, true) => (n.div_ceil(2), 2),
        (n, false) => (2, n.div_ceil(2)),
    }
}

/// Les commandes d'une page : origine en haut à gauche, comme à l'écran.
struct Canvas {
    ops: String,
    h: f32,
}

impl Canvas {
    fn text(&mut self, fonts: &mut Fonts, x: f32, top: f32, s: &str, bold: bool, size: f32) {
        let (font, operand) = (fonts.resource(bold), fonts.show(s, bold));
        self.ops += &format!("BT /{font} {size:.2} Tf {x:.2} {:.2} Td {operand} Tj ET\n", self.h - top);
    }

    fn text_right(&mut self, fonts: &mut Fonts, right: f32, top: f32, s: &str, bold: bool, size: f32) {
        let x = right - fonts.width(s, bold, size);
        self.text(fonts, x, top, s, bold, size);
    }

    fn gray(&mut self, g: f32) {
        self.ops += &format!("{g:.2} g {g:.2} G\n");
    }

    fn rect(&mut self, x: f32, top: f32, w: f32, h: f32, line: f32) {
        self.ops += &format!("{line} w {x:.2} {:.2} {w:.2} {h:.2} re S\n", self.h - top - h);
    }

    fn line(&mut self, x0: f32, x1: f32, top: f32, width: f32) {
        let y = self.h - top;
        self.ops += &format!("{width} w {x0:.2} {y:.2} m {x1:.2} {y:.2} l S\n");
    }

    fn image(&mut self, id: usize, x: f32, top: f32, w: f32, h: f32) {
        self.ops += &pdf::op_image(self.h, id, x, top, w, h);
    }

    /// Une couleur pour le trait et le remplissage (0 à 1) ; `gray` y revient.
    fn rgb(&mut self, r: f32, g: f32, b: f32) {
        self.ops += &format!("{r:.2} {g:.2} {b:.2} rg {r:.2} {g:.2} {b:.2} RG\n");
    }

    /// Un rectangle plein de la couleur courante.
    fn fill(&mut self, x: f32, top: f32, w: f32, h: f32) {
        self.ops += &format!("{x:.2} {:.2} {w:.2} {h:.2} re f\n", self.h - top - h);
    }
}

/// Une ligne de texte d'une case : un libellé en gras au début (« Action : ») ou non, et la place qu'il prend.
struct Ligne {
    label: Option<String>,
    text: String,
    indent: f32,
}

/// Les lignes de texte d'une case, coupées à `width` points et à `max_lines` lignes (« … » si le texte ne tient pas).
fn lignes(b: &Board, layout: &PdfLayout, fonts: &Fonts, size: f32, width: f32, max_lines: usize) -> Vec<Ligne> {
    let mut out: Vec<Ligne> = Vec::new();
    let mut cut = false;
    let mut fields: Vec<(String, &str)> = Vec::new();
    if layout.action {
        fields.push(("Action".into(), &b.action));
    }
    if layout.dialogue {
        fields.push(("Dialogue".into(), &b.dialogue));
    }
    if layout.notes {
        fields.push(("Notes".into(), &b.notes));
    }
    for (author, text) in &b.comments {
        let who = author.trim();
        fields.push((if who.is_empty() { "Commentaire".into() } else { format!("Commentaire ({who})") }, text));
    }
    let measure = |s: &str, bold: bool, sz: f32| fonts.width(s, bold, sz);
    'fields: for (label, text) in fields {
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        let label = format!("{label} : ");
        let indent = fonts.width(&label, true, size);
        for (k, line) in pdf::wrap_with(&measure, text, false, size, width, indent).into_iter().enumerate() {
            if out.len() >= max_lines {
                cut = true;
                break 'fields;
            }
            out.push(Ligne { label: (k == 0).then(|| label.clone()), text: line, indent: if k == 0 { indent } else { 0.0 } });
        }
    }
    if cut {
        if let Some(last) = out.last_mut() {
            let room = width - last.indent - fonts.width("…", false, size);
            while !last.text.is_empty() && fonts.width(&last.text, false, size) > room {
                last.text.pop();
            }
            last.text = format!("{}…", last.text.trim_end());
        }
    }
    out
}

/// Tout le texte que la mise en page écrira (pour choisir une police qui a toutes les lettres).
pub(crate) fn tout_le_texte(title: &str, date: &str, boards: &[Board]) -> String {
    let mut s = format!("{title} {date} Séquence · Page page / 0123456789 Action Dialogue Notes Commentaire () : — … ");
    for b in boards {
        for t in [&b.title, &b.duration, &b.sequence, &b.action, &b.dialogue, &b.notes] {
            s.push_str(t);
            s.push(' ');
        }
        for (a, t) in &b.comments {
            s.push_str(a);
            s.push_str(t);
        }
    }
    s
}

/// Le storyboard : les polices sont choisies d'après le texte (`Fonts::choose`) ; avec l'en-tête de studio, les plans modifiés et la page de révision de `studio` (`entetepdf.rs`) selon les choix de `layout`.
pub fn storyboard_layout_studio(title: &str, date: &str, boards: &[Board], images: &[BoardImage], aspect: f32, layout: &PdfLayout, studio: Option<&Studio>) -> Vec<u8> {
    let texte = format!("{} {}", tout_le_texte(title, date, boards), studio.map(Studio::tout_le_texte).unwrap_or_default());
    let fonts = Fonts::choose(&texte);
    storyboard_layout_studio_with(title, date, boards, images, aspect, layout, fonts, studio)
}

/// Le storyboard écrit avec les polices `fonts`. `aspect` : largeur / hauteur du cadre.
#[cfg_attr(not(test), allow(dead_code))]
pub fn storyboard_layout_with(title: &str, date: &str, boards: &[Board], images: &[BoardImage], aspect: f32, layout: &PdfLayout, fonts: Fonts) -> Vec<u8> {
    storyboard_layout_studio_with(title, date, boards, images, aspect, layout, fonts, None)
}

/// Les lignes de la page « Notes de révision » coupées à `largeur` : (gras ?, retrait, texte), une ligne vide pour un blanc.
fn lignes_de_revision(rev: &Revision, fonts: &Fonts, size: f32, largeur: f32) -> Vec<(bool, f32, String)> {
    let mesure = |s: &str, bold: bool, sz: f32| fonts.width(s, bold, sz);
    let mut out = Vec::new();
    for (bold, retrait, texte) in rev.lignes() {
        if texte.trim().is_empty() {
            out.push((false, 0.0, String::new()));
            continue;
        }
        for l in pdf::wrap_with(&mesure, &texte, bold, size, (largeur - retrait).max(40.0), 0.0) {
            out.push((bold, retrait, l));
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
pub fn storyboard_layout_studio_with(title: &str, date: &str, boards: &[Board], images: &[BoardImage], aspect: f32, layout: &PdfLayout, mut fonts: Fonts, studio: Option<&Studio>) -> Vec<u8> {
    let mut pdf = PdfFile::new();
    fonts.attach(&mut pdf);
    let (pw, ph) = layout.page_size();
    let per = layout.per_page_ok();
    let (cols, rows) = grid(per, layout.landscape);
    let pages = pdf::paginate_n(boards, per);
    // L'en-tête de studio : seulement si l'on a de quoi l'écrire.
    let studio_on = layout.studio && studio.is_some();
    let revision = studio.filter(|_| layout.notes_revision).and_then(|s| s.revision.as_ref());
    // Les marges et les écarts suivent la taille de la page ; le texte, la largeur des cases.
    let k = (pw.min(ph) / 595.0).clamp(1.0, 1.4);
    let (margin, gap_x, gap_y) = (28.0 * k, 16.0 * k, 14.0 * k);
    let header_h = if studio_on { 38.0 * k } else if layout.header { 26.0 * k } else { 0.0 };
    // (« Page n / N » est dans l'en-tête de studio : pas de second numéro en bas.)
    let footer_h = if layout.page_numbers && !studio_on { 18.0 * k } else { 0.0 };
    // La page de révision : ses lignes, et le nombre de pages qu'elle prend (leur nombre fait partie du « N » de chaque page).
    let (rev_size, rev_leading) = (9.5 * k, 13.0 * k);
    let rev_lignes = revision.map(|r| lignes_de_revision(r, &fonts, rev_size, pw - 2.0 * margin)).unwrap_or_default();
    let rev_top = margin + header_h + 22.0 * k;
    let rev_par_page = (((ph - margin - rev_top) / rev_leading).floor() as usize).max(1);
    let rev_pages = if revision.is_some() { rev_lignes.len().div_ceil(rev_par_page).max(1) } else { 0 };
    let total = pages.len() + rev_pages;
    let grid_top = margin + header_h;
    let cell_w = (pw - 2.0 * margin - (cols as f32 - 1.0) * gap_x) / cols as f32;
    let row_h = (ph - grid_top - margin - footer_h - gap_y * (rows as f32 - 1.0)) / rows as f32;
    let scale = (cell_w / 240.0).clamp(0.85, 1.7);
    let (text_size, leading, title_size) = (8.0 * scale, 10.0 * scale, 8.5 * scale);
    let any_text = layout.action || layout.dialogue || layout.notes;
    let info_line = layout.number || layout.duration;
    // L'image prend toute la largeur de la case, sans manger la place du texte.
    let reserve = if info_line { leading + 4.0 } else { 0.0 } + if any_text { 3.0 * leading } else { 0.0 };
    let mut max_h = (row_h - reserve - 4.0).max(row_h * 0.3);
    if any_text {
        max_h = max_h.min(row_h * 0.62);
    }
    let img_h = (cell_w / aspect.max(0.1)).min(max_h);
    let img_w = img_h * aspect.max(0.1);
    for (n, page) in pages.iter().enumerate() {
        let mut c = Canvas { ops: String::new(), h: ph };
        let mut used = Vec::new();
        if let (true, Some(s)) = (studio_on, studio) {
            en_tete_de_studio(&mut c, &mut fonts, &s.entete.pour_sequence(&boards[page[0]].sequence), (pw, margin, k), &format!("Page {} / {total}", n + 1));
        } else if layout.header {
            c.gray(0.1);
            c.text(&mut fonts, margin, margin + 12.0 * k, title, true, 13.0 * k);
            c.gray(0.35);
            let seq = &boards[page[0]].sequence;
            c.text_right(&mut fonts, pw - margin, margin + 12.0 * k, &format!("Séquence {seq} · {date}"), false, 9.0 * k);
            c.gray(0.75);
            c.line(margin, pw - margin, margin + 19.0 * k, 0.6);
        }
        for (slot, &i) in page.iter().enumerate() {
            let (col, row) = (slot % cols, slot / cols);
            let x = margin + col as f32 * (cell_w + gap_x);
            let top = grid_top + row as f32 * (row_h + gap_y);
            let ix = x + (cell_w - img_w) / 2.0;
            if let Some(img) = images.get(i) {
                let id = pdf.add_jpeg(img.width, img.height, &img.jpeg);
                c.image(id, ix, top, img_w, img_h);
                used.push(id);
            }
            c.gray(0.55);
            c.rect(ix, top, img_w, img_h, 0.5);
            // Un plan modifié ou ajouté depuis le dernier envoi : un cadre rouge et une étiquette « MODIFIÉ » dans le coin.
            if layout.marquer_modifies && studio.is_some_and(|s| s.modifies.get(i).copied().unwrap_or(false)) {
                c.rgb(0.85, 0.12, 0.12);
                c.rect(ix, top, img_w, img_h, 2.0);
                let (tag_size, tag) = (7.5 * scale, "MODIFIÉ");
                let tag_w = fonts.width(tag, true, tag_size) + 6.0 * scale;
                c.fill(ix, top, tag_w, tag_size + 4.0 * scale);
                c.gray(1.0);
                c.text(&mut fonts, ix + 3.0 * scale, top + tag_size + 1.0 * scale, tag, true, tag_size);
            }
            let b = &boards[i];
            let mut y = top + img_h + if info_line { 11.0 * scale } else { 4.0 * scale };
            if info_line {
                if layout.number {
                    c.gray(0.1);
                    c.text(&mut fonts, x, y, &b.title, true, title_size);
                }
                if layout.duration {
                    c.gray(0.4);
                    c.text_right(&mut fonts, x + cell_w, y, &b.duration, false, title_size);
                }
                y += leading + 1.0;
            }
            let max_lines = ((top + row_h - y) / leading).floor().max(0.0) as usize + 1;
            c.gray(0.15);
            for l in lignes(b, layout, &fonts, text_size, cell_w, max_lines) {
                if let Some(label) = &l.label {
                    c.text(&mut fonts, x, y, label, true, text_size);
                }
                c.text(&mut fonts, x + l.indent, y, &l.text, false, text_size);
                y += leading;
            }
        }
        if layout.page_numbers && !studio_on {
            c.gray(0.35);
            let label = format!("Page {} / {total}", n + 1);
            let w = fonts.width(&label, false, 8.5 * k);
            c.text(&mut fonts, (pw - w) / 2.0, ph - margin + 4.0 * k, &label, false, 8.5 * k);
        }
        pdf.add_page_sized(pw, ph, c.ops.as_bytes(), &used);
    }
    // La page finale « Notes de révision » (une ou plusieurs pages), avec le même en-tête.
    if let (Some(_), Some(s)) = (revision, studio) {
        let paquets: Vec<&[(bool, f32, String)]> = if rev_lignes.is_empty() { vec![&rev_lignes[..]] } else { rev_lignes.chunks(rev_par_page).collect() };
        for (r, lignes) in paquets.into_iter().enumerate() {
            let mut c = Canvas { ops: String::new(), h: ph };
            let numero = pages.len() + r + 1;
            if studio_on {
                en_tete_de_studio(&mut c, &mut fonts, &s.entete, (pw, margin, k), &format!("Page {numero} / {total}"));
            } else {
                c.gray(0.1);
                c.text(&mut fonts, margin, margin + 12.0 * k, title, true, 13.0 * k);
                c.gray(0.35);
                c.text_right(&mut fonts, pw - margin, margin + 12.0 * k, &format!("Page {numero} / {total}"), false, 9.0 * k);
                c.gray(0.75);
                c.line(margin, pw - margin, margin + 19.0 * k, 0.6);
            }
            c.gray(0.1);
            c.text(&mut fonts, margin, margin + header_h + 12.0 * k, if r == 0 { "Notes de révision" } else { "Notes de révision (suite)" }, true, 12.0 * k);
            let mut y = rev_top + rev_leading;
            for (bold, retrait, texte) in lignes {
                if !texte.is_empty() {
                    c.gray(if *bold { 0.1 } else { 0.2 });
                    c.text(&mut fonts, margin + retrait, y, texte, *bold, rev_size);
                }
                y += rev_leading;
            }
            pdf.add_page_sized(pw, ph, c.ops.as_bytes(), &[]);
        }
    }
    fonts.write(&mut pdf);
    pdf.finish(title)
}

/// Le texte `s` à (`x`, `top`) d'une page de hauteur `page_h` (origine en haut à gauche), en `gray` (0 noir, 1 blanc) : les commandes d'une page de PDF.
#[allow(clippy::too_many_arguments)]
pub(crate) fn op_texte(fonts: &mut Fonts, page_h: f32, x: f32, top: f32, s: &str, bold: bool, size: f32, gray: f32) -> String {
    let (font, operand) = (fonts.resource(bold), fonts.show(s, bold));
    format!("{gray:.2} g BT /{font} {size:.2} Tf {x:.2} {:.2} Td {operand} Tj ET\n", page_h - top)
}

/// Un filet horizontal de `x0` à `x1` à la hauteur `top`.
pub(crate) fn op_filet(page_h: f32, x0: f32, x1: f32, top: f32, width: f32, gray: f32) -> String {
    let y = page_h - top;
    format!("{gray:.2} G {width} w {x0:.2} {y:.2} m {x1:.2} {y:.2} l S\n")
}

/// L'en-tête de studio : le titre et « Page n / N » sur une ligne, la ligne « Épisode · Séquence · Version · date » dessous, un filet. `(largeur de la page, marge, k)`.
fn en_tete_de_studio(c: &mut Canvas, fonts: &mut Fonts, e: &Entete, (pw, margin, k): (f32, f32, f32), page: &str) {
    c.gray(0.1);
    c.text(fonts, margin, margin + 12.0 * k, &e.ligne1(), true, 13.0 * k);
    c.gray(0.35);
    c.text_right(fonts, pw - margin, margin + 12.0 * k, page, false, 9.0 * k);
    c.text(fonts, margin, margin + 25.0 * k, &e.ligne2(), false, 9.0 * k);
    c.gray(0.75);
    c.line(margin, pw - margin, margin + 31.0 * k, 0.6);
}
