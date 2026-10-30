//! La mise en page du storyboard PDF « à la carte » (exports V1, 2026-10-30) : format de page (A4, A3, Lettre), portrait ou paysage, 1,
//! 2, 3, 4, 6, 8 ou 9 cases par page (la grille est calculée), les textes qu'on veut voir (action, dialogue, notes, durée, numéro), le
//! titre et la date en en-tête, les numéros de page. Les textes sont écrits avec une police embarquée (`pdffont.rs`) : accents et
//! tous les caractères de la police, aucun « ? ».
//!
//! L'ancienne mise en page (`pdf::storyboard_with` : A4 paysage, 6 cases, Helvetica) reste pour la version à imprimer de la relecture.

use serde::{Deserialize, Serialize};

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
}

impl Default for PdfLayout {
    fn default() -> Self {
        PdfLayout { paper: Paper::A4, landscape: true, per_page: 6, action: true, dialogue: true, notes: true, duration: true, number: true, header: true, page_numbers: true }
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

/// Le storyboard : les polices sont choisies d'après le texte (`Fonts::choose`).
pub fn storyboard_layout(title: &str, date: &str, boards: &[Board], images: &[BoardImage], aspect: f32, layout: &PdfLayout) -> Vec<u8> {
    let fonts = Fonts::choose(&tout_le_texte(title, date, boards));
    storyboard_layout_with(title, date, boards, images, aspect, layout, fonts)
}

/// Le storyboard écrit avec les polices `fonts`. `aspect` : largeur / hauteur du cadre.
pub fn storyboard_layout_with(title: &str, date: &str, boards: &[Board], images: &[BoardImage], aspect: f32, layout: &PdfLayout, mut fonts: Fonts) -> Vec<u8> {
    let mut pdf = PdfFile::new();
    fonts.attach(&mut pdf);
    let (pw, ph) = layout.page_size();
    let per = layout.per_page_ok();
    let (cols, rows) = grid(per, layout.landscape);
    let pages = pdf::paginate_n(boards, per);
    let total = pages.len();
    // Les marges et les écarts suivent la taille de la page ; le texte, la largeur des cases.
    let k = (pw.min(ph) / 595.0).clamp(1.0, 1.4);
    let (margin, gap_x, gap_y) = (28.0 * k, 16.0 * k, 14.0 * k);
    let header_h = if layout.header { 26.0 * k } else { 0.0 };
    let footer_h = if layout.page_numbers { 18.0 * k } else { 0.0 };
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
        if layout.header {
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
        if layout.page_numbers {
            c.gray(0.35);
            let label = format!("Page {} / {total}", n + 1);
            let w = fonts.width(&label, false, 8.5 * k);
            c.text(&mut fonts, (pw - w) / 2.0, ph - margin + 4.0 * k, &label, false, 8.5 * k);
        }
        pdf.add_page_sized(pw, ph, c.ops.as_bytes(), &used);
    }
    fonts.write(&mut pdf);
    pdf.finish(title)
}
