//! PDF du storyboard (3c), écrit sans bibliothèque : A4 paysage, 6 panneaux par page (2 rangées
//! de 3), le texte sous chaque image. Police standard des PDF (Helvetica, présente dans tous les
//! lecteurs, rien à embarquer), en WinAnsi : les accents français passent, les caractères hors de
//! cette table deviennent « ? ». Images en JPEG, telles quelles dans le fichier.

/// A4 paysage, en points (1/72 de pouce).
pub const PAGE_W: f32 = 842.0;
pub const PAGE_H: f32 = 595.0;
const MARGIN: f32 = 28.0;
/// Bandeau du haut (titre, séquence, page).
const HEADER: f32 = 26.0;
const GAP_X: f32 = 16.0;
const GAP_Y: f32 = 14.0;
pub const PER_PAGE: usize = 6;
const COLUMNS: usize = 3;
/// Part de la hauteur d'une case laissée à l'image, au plus.
const IMAGE_SHARE: f32 = 0.58;
const TEXT_SIZE: f32 = 8.0;
const LEADING: f32 = 10.0;

// ------------------------------------------------------------------------------ texte et polices

/// Texte en WinAnsi (cp1252), l'encodage des polices standard des PDF.
pub fn winansi(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| match c as u32 {
            0x20..=0x7E | 0xA0..=0xFF => c as u32 as u8,
            _ => match c {
                '€' => 0x80,
                '‚' => 0x82,
                '„' => 0x84,
                '…' => 0x85,
                '‰' => 0x89,
                'Š' => 0x8A,
                '‹' => 0x8B,
                'Œ' => 0x8C,
                'Ž' => 0x8E,
                '‘' => 0x91,
                '’' => 0x92,
                '“' => 0x93,
                '”' => 0x94,
                '•' => 0x95,
                '–' => 0x96,
                '—' => 0x97,
                '™' => 0x99,
                'š' => 0x9A,
                '›' => 0x9B,
                'œ' => 0x9C,
                'ž' => 0x9E,
                'Ÿ' => 0x9F,
                '\t' => b' ',
                _ => b'?',
            },
        })
        .collect()
}

/// Largeurs (en millièmes de la taille) des caractères 32 à 126 d'Helvetica et d'Helvetica-Bold,
/// d'après leurs métriques publiées.
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, // espace à /
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, // chiffres
    278, 278, 584, 584, 584, 556, 1015, // : à @
    667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, // A à Z
    278, 278, 278, 469, 556, 333, // [ à `
    556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500,
    500, 500, // a à z
    334, 260, 334, 584, // { à ~
];
const HELVETICA_BOLD: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, // espace à /
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, // chiffres
    333, 333, 584, 584, 584, 611, 975, // : à @
    722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, // A à Z
    333, 278, 333, 584, 556, 333, // [ à `
    556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556,
    556, 500, // a à z
    389, 280, 389, 584, // { à ~
];

/// Largeur d'un octet WinAnsi : lettres accentuées comme leur lettre de base.
fn char_width(b: u8, bold: bool) -> u16 {
    let table = if bold { &HELVETICA_BOLD } else { &HELVETICA };
    let base = |c: u8| table[(c - 32) as usize];
    match b {
        32..=126 => base(b),
        0xC0..=0xC5 => base(b'A'),
        0xC7 => base(b'C'),
        0xC8..=0xCB => base(b'E'),
        0xCC..=0xCF | 0xEC..=0xEF => 278,
        0xD1 => base(b'N'),
        0xD2..=0xD6 | 0xD8 => base(b'O'),
        0xD9..=0xDC => base(b'U'),
        0xDD | 0x9F => base(b'Y'),
        0xE0..=0xE5 => base(b'a'),
        0xE7 => base(b'c'),
        0xE8..=0xEB => base(b'e'),
        0xF1 => base(b'n'),
        0xF2..=0xF6 | 0xF8 => base(b'o'),
        0xF9..=0xFC => base(b'u'),
        0xFD | 0xFF => base(b'y'),
        0xC6 | 0x8C | 0x85 | 0x97 => 1000,
        0xE6 => 889,
        0x9C => 944,
        0xA0 => 278,
        0xB0 => 400,
        0x91 | 0x92 => if bold { 278 } else { 222 },
        0x93 | 0x94 => if bold { 500 } else { 333 },
        0x95 => 350,
        _ => 556,
    }
}

/// Largeur de `s`, en points, à la taille `size`.
pub fn text_width(s: &str, bold: bool, size: f32) -> f32 {
    winansi(s).iter().map(|&b| char_width(b, bold) as f32).sum::<f32>() * size / 1000.0
}

/// Coupe `text` en lignes de `width` points au plus ; la première commence après `indent` points.
/// Les retours à la ligne du texte sont gardés ; un mot trop long est coupé.
pub fn wrap(text: &str, bold: bool, size: f32, width: f32, indent: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let room = |lines: &Vec<String>| if lines.is_empty() { width - indent } else { width };
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if text_width(&candidate, bold, size) <= room(&lines) {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            // Mot plus long qu'une ligne : coupé lettre par lettre.
            for c in word.chars() {
                let mut next = line.clone();
                next.push(c);
                if !line.is_empty() && text_width(&next, bold, size) > room(&lines) {
                    lines.push(std::mem::take(&mut line));
                    next = c.to_string();
                }
                line = next;
            }
        }
        lines.push(line);
    }
    while lines.len() > 1 && lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines
}

/// Chaîne PDF littérale : parenthèses et barres échappées, octets hors ASCII en octal.
fn pdf_string(bytes: &[u8]) -> String {
    let mut s = String::from("(");
    for &b in bytes {
        match b {
            b'(' | b')' | b'\\' => {
                s.push('\\');
                s.push(b as char);
            }
            32..=126 => s.push(b as char),
            _ => s.push_str(&format!("\\{b:03o}")),
        }
    }
    s.push(')');
    s
}

/// Chaîne de texte des métadonnées : UTF-16 (toutes les lettres passent).
fn pdf_text_utf16(s: &str) -> String {
    let mut out = String::from("<FEFF");
    for u in s.encode_utf16() {
        out.push_str(&format!("{u:04X}"));
    }
    out.push('>');
    out
}

// ------------------------------------------------------------------------------- fichier PDF

/// Un fichier PDF en construction : objets numérotés, pages.
pub struct PdfFile {
    /// Objet n° i + 1. Les 4 premiers (catalogue, pages, deux polices) sont écrits à la fin.
    objects: Vec<Vec<u8>>,
    pages: Vec<usize>,
}

impl Default for PdfFile {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfFile {
    pub fn new() -> PdfFile {
        PdfFile {
            objects: vec![Vec::new(); 4],
            pages: Vec::new(),
        }
    }

    fn add(&mut self, body: Vec<u8>) -> usize {
        self.objects.push(body);
        self.objects.len()
    }

    fn add_stream(&mut self, dict: &str, data: &[u8]) -> usize {
        let mut body = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
        body.extend_from_slice(data);
        body.extend_from_slice(b"\nendstream");
        self.add(body)
    }

    /// Image JPEG (RVB) ; renvoie son numéro, à citer dans `add_page`.
    pub fn add_jpeg(&mut self, width: u32, height: u32, jpeg: &[u8]) -> usize {
        let dict = format!(
            "/Type /XObject /Subtype /Image /Width {width} /Height {height} /ColorSpace /DeviceRGB \
             /BitsPerComponent 8 /Filter /DCTDecode"
        );
        self.add_stream(&dict, jpeg)
    }

    /// Page A4 paysage dessinée par `content` (polices /F1 et /F2, images /Im<n>).
    pub fn add_page(&mut self, content: &[u8], images: &[usize]) {
        let contents = self.add_stream("", content);
        let xobjects: String = images.iter().map(|id| format!("/Im{id} {id} 0 R ")).collect();
        let page = format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_W} {PAGE_H}] /Resources << /Font << /F1 3 0 R \
             /F2 4 0 R >> /XObject << {xobjects}>> >> /Contents {contents} 0 R >>"
        );
        let id = self.add(page.into_bytes());
        self.pages.push(id);
    }

    /// Le fichier complet.
    pub fn finish(mut self, title: &str) -> Vec<u8> {
        self.objects[0] = b"<< /Type /Catalog /Pages 2 0 R >>".to_vec();
        let kids: String = self.pages.iter().map(|id| format!("{id} 0 R ")).collect();
        self.objects[1] = format!("<< /Type /Pages /Kids [ {kids}] /Count {} >>", self.pages.len()).into_bytes();
        for (i, font) in ["Helvetica", "Helvetica-Bold"].into_iter().enumerate() {
            self.objects[2 + i] =
                format!("<< /Type /Font /Subtype /Type1 /BaseFont /{font} /Encoding /WinAnsiEncoding >>").into_bytes();
        }
        let info = self.add(format!("<< /Title {} /Producer (Pelure) >>", pdf_text_utf16(title)).into_bytes());
        let mut out = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
        let mut offsets = Vec::with_capacity(self.objects.len());
        for (i, body) in self.objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            out.extend_from_slice(body);
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", self.objects.len() + 1).as_bytes());
        for o in offsets {
            out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R /Info {info} 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                self.objects.len() + 1
            )
            .as_bytes(),
        );
        out
    }
}

/// Commandes de dessin d'une page (origine en haut à gauche, comme à l'écran).
#[derive(Default)]
struct Canvas {
    ops: String,
}

impl Canvas {
    fn text(&mut self, x: f32, top: f32, s: &str, bold: bool, size: f32) {
        let font = if bold { "F2" } else { "F1" };
        self.ops += &format!(
            "BT /{font} {size} Tf {x:.2} {:.2} Td {} Tj ET\n",
            PAGE_H - top,
            pdf_string(&winansi(s))
        );
    }

    fn text_right(&mut self, right: f32, top: f32, s: &str, bold: bool, size: f32) {
        let x = right - text_width(s, bold, size);
        self.text(x, top, s, bold, size);
    }

    fn gray(&mut self, g: f32) {
        self.ops += &format!("{g:.2} g {g:.2} G\n");
    }

    fn rect(&mut self, x: f32, top: f32, w: f32, h: f32, line: f32) {
        self.ops += &format!("{line} w {x:.2} {:.2} {w:.2} {h:.2} re S\n", PAGE_H - top - h);
    }

    fn line(&mut self, x0: f32, x1: f32, top: f32, width: f32) {
        let y = PAGE_H - top;
        self.ops += &format!("{width} w {x0:.2} {y:.2} m {x1:.2} {y:.2} l S\n");
    }

    fn image(&mut self, id: usize, x: f32, top: f32, w: f32, h: f32) {
        self.ops += &format!("q {w:.2} 0 0 {h:.2} {x:.2} {:.2} cm /Im{id} Do Q\n", PAGE_H - top - h);
    }
}

// ------------------------------------------------------------------------------ mise en page

/// Ce qu'on écrit d'un panneau.
#[derive(Clone, Debug)]
pub struct Board {
    /// « Scène Arrivée — panneau 1/2 ».
    pub title: String,
    pub duration: String,
    /// Une nouvelle séquence commence une nouvelle page.
    pub sequence: String,
    pub action: String,
    pub dialogue: String,
    pub notes: String,
}

/// Panneaux de chaque page : 6 au plus, nouvelle page à chaque séquence.
pub fn paginate(boards: &[Board]) -> Vec<Vec<usize>> {
    let mut pages: Vec<Vec<usize>> = Vec::new();
    for (i, b) in boards.iter().enumerate() {
        let new_page = match pages.last() {
            None => true,
            Some(page) => page.len() >= PER_PAGE || boards[page[0]].sequence != b.sequence,
        };
        if new_page {
            pages.push(Vec::new());
        }
        pages.last_mut().expect("page créée").push(i);
    }
    pages
}

/// Une ligne de texte d'une case : un libellé en gras au début (« Action : ») ou non.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardLine {
    pub label: Option<String>,
    pub text: String,
}

impl BoardLine {
    /// Retrait du texte : la largeur du libellé.
    pub fn indent(&self) -> f32 {
        self.label.as_ref().map_or(0.0, |l| text_width(l, true, TEXT_SIZE))
    }
}

/// Lignes de texte d'une case, coupées à la largeur et à `max_lines` lignes (« … » à la fin si
/// le texte ne tient pas).
pub fn board_lines(b: &Board, width: f32, max_lines: usize) -> Vec<BoardLine> {
    let mut out: Vec<BoardLine> = Vec::new();
    let mut cut = false;
    'fields: for (label, text) in [("Action", &b.action), ("Dialogue", &b.dialogue), ("Notes", &b.notes)] {
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        let label = format!("{label} : ");
        let indent = text_width(&label, true, TEXT_SIZE);
        for (k, line) in wrap(text, false, TEXT_SIZE, width, indent).into_iter().enumerate() {
            if out.len() >= max_lines {
                cut = true;
                break 'fields;
            }
            out.push(BoardLine {
                label: (k == 0).then(|| label.clone()),
                text: line,
            });
        }
    }
    if cut {
        if let Some(last) = out.last_mut() {
            let room = width - last.indent() - text_width("…", false, TEXT_SIZE);
            while !last.text.is_empty() && text_width(&last.text, false, TEXT_SIZE) > room {
                last.text.pop();
            }
            last.text = format!("{}…", last.text.trim_end());
        }
    }
    out
}

/// Image d'un panneau, prête pour le PDF : JPEG et taille en pixels.
pub struct BoardImage {
    pub width: u32,
    pub height: u32,
    pub jpeg: Vec<u8>,
}

/// Le storyboard complet. `aspect` = largeur / hauteur du cadre.
pub fn storyboard(title: &str, boards: &[Board], images: &[BoardImage], aspect: f32) -> Vec<u8> {
    let mut pdf = PdfFile::new();
    let pages = paginate(boards);
    let cell_w = (PAGE_W - 2.0 * MARGIN - (COLUMNS as f32 - 1.0) * GAP_X) / COLUMNS as f32;
    let grid_top = MARGIN + HEADER;
    let row_h = (PAGE_H - grid_top - MARGIN - GAP_Y) / 2.0;
    // Image : toute la largeur de la case, sauf si elle est trop haute (cadre carré ou vertical).
    let (img_w, img_h) = {
        let h = cell_w / aspect.max(0.1);
        let max_h = row_h * IMAGE_SHARE;
        if h <= max_h { (cell_w, h) } else { (max_h * aspect, max_h) }
    };
    let total = pages.len();
    for (n, page) in pages.iter().enumerate() {
        let mut c = Canvas::default();
        let mut used = Vec::new();
        // Bandeau : titre du projet, séquence, numéro de page.
        c.gray(0.1);
        c.text(MARGIN, MARGIN + 12.0, title, true, 13.0);
        c.gray(0.35);
        let seq = &boards[page[0]].sequence;
        c.text_right(PAGE_W - MARGIN, MARGIN + 12.0, &format!("Séquence {seq} · page {} / {total}", n + 1), false, 9.0);
        c.gray(0.75);
        c.line(MARGIN, PAGE_W - MARGIN, MARGIN + 19.0, 0.6);
        for (slot, &i) in page.iter().enumerate() {
            let (col, row) = (slot % COLUMNS, slot / COLUMNS);
            let x = MARGIN + col as f32 * (cell_w + GAP_X);
            let top = grid_top + row as f32 * (row_h + GAP_Y);
            let ix = x + (cell_w - img_w) / 2.0;
            if let Some(img) = images.get(i) {
                let id = pdf.add_jpeg(img.width, img.height, &img.jpeg);
                c.image(id, ix, top, img_w, img_h);
                used.push(id);
            }
            c.gray(0.55);
            c.rect(ix, top, img_w, img_h, 0.5);
            // Titre et durée, puis action, dialogue, notes.
            let b = &boards[i];
            let mut y = top + img_h + 11.0;
            c.gray(0.1);
            c.text(x, y, &b.title, true, 8.5);
            c.gray(0.4);
            c.text_right(x + cell_w, y, &b.duration, false, 8.5);
            y += LEADING + 1.0;
            let max_lines = ((top + row_h - y) / LEADING).floor().max(0.0) as usize + 1;
            c.gray(0.15);
            for line in board_lines(b, cell_w, max_lines) {
                if let Some(label) = &line.label {
                    c.text(x, y, label, true, TEXT_SIZE);
                }
                c.text(x + line.indent(), y, &line.text, false, TEXT_SIZE);
                y += LEADING;
            }
        }
        pdf.add_page(c.ops.as_bytes(), &used);
    }
    pdf.finish(title)
}
