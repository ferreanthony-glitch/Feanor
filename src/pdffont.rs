//! Polices TrueType embarquées dans le PDF du storyboard (exports V1, 2026-10-30) : les accents et tous les caractères que la police
//! connaît sont écrits tels quels, sans le « ? » de la table WinAnsi des polices standard.
//!
//! - **Quelle police** : une police déjà présente dans Windows (`C:\Windows\Fonts`, lue seulement, rien n'est copié dans le dépôt) :
//!   Segoe UI, sinon Arial, Calibri, Tahoma, Verdana. On lit la licence d'incorporation déclarée dans la table OS/2 (`fsType`) : une
//!   police « Restreinte » (bit 1) ou dont on ne peut incorporer que les bitmaps (bit 9) n'est jamais prise. Parmi celles qui
//!   l'autorisent, on garde la première qui possède toutes les lettres du document.
//! - **Comment** : la police entière (compressée), en `CIDFontType2` / `Identity-H` : le texte est écrit en numéros de glyphes (2 octets),
//!   avec les largeurs (`/W`) et une table `ToUnicode` pour que le texte du PDF reste sélectionnable et cherchable. Pas de
//!   sous-ensemble : la licence « modifiable » ou « installable » l'autorise, mais un sous-ensemble demande de réécrire `glyf` et
//!   `loca`, pour un gain de place qui ne compte pas ici (quelques centaines de Ko).
//! - **Repli** : aucune police permise ou trouvée (ou pas de Windows) : les polices standard Helvetica et leur WinAnsi d'avant
//!   (`pdf::winansi`), telles quelles.

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::PathBuf;

use crate::pdf::{self, PdfFile};

/// Les paires (normale, grasse) essayées, dans l'ordre : le nom des fichiers de `C:\Windows\Fonts`.
const PAIRES: [(&str, &str); 5] = [("segoeui.ttf", "segoeuib.ttf"), ("arial.ttf", "arialbd.ttf"), ("calibri.ttf", "calibrib.ttf"), ("tahoma.ttf", "tahomabd.ttf"), ("verdana.ttf", "verdanab.ttf")];

/// Une police TrueType embarquée : ses octets, les lettres que le texte utilise (numéro de glyphe, largeur en millièmes d'em) et ce
/// qu'il en a effectivement écrit.
pub struct EmbFont {
    data: Vec<u8>,
    name: String,
    bold: bool,
    /// Lettre → (numéro de glyphe, largeur en 1/1000 d'em). Les lettres que la police n'a pas ne sont pas là.
    glyphs: HashMap<char, (u16, u16)>,
    /// Largeur du glyphe absent, en 1/1000 d'em.
    notdef: u16,
    /// Les glyphes écrits : numéro → lettre.
    used: BTreeMap<u16, char>,
    bbox: [i32; 4],
    ascent: i32,
    descent: i32,
    cap: i32,
    italic: f32,
}

/// La licence d'incorporation de la police : vrai si le fichier peut être embarqué dans un PDF (lu dans OS/2, `fsType`).
pub fn embeddable(face: &ttf_parser::Face) -> bool {
    let Some(os2) = face.tables().os2 else { return false };
    let permitted = matches!(
        os2.permissions(),
        Some(ttf_parser::os2::Permissions::Installable) | Some(ttf_parser::os2::Permissions::Editable) | Some(ttf_parser::os2::Permissions::PreviewAndPrint)
    );
    permitted && os2.is_outline_embedding_allowed()
}

impl EmbFont {
    /// Lit la police `data` pour les lettres de `chars` ; `None` si elle est illisible ou si sa licence refuse l'incorporation.
    fn open(data: Vec<u8>, chars: &[char], bold: bool) -> Option<EmbFont> {
        let (glyphs, notdef, name, bbox, ascent, descent, cap, italic) = {
            let face = ttf_parser::Face::parse(&data, 0).ok()?;
            if !embeddable(&face) {
                return None;
            }
            let upem = face.units_per_em().max(1) as f32;
            let milli = |v: f32| (v * 1000.0 / upem).round() as i32;
            let adv = |g: ttf_parser::GlyphId| milli(face.glyph_hor_advance(g).unwrap_or(0) as f32).clamp(0, 5000) as u16;
            let mut glyphs = HashMap::new();
            for &c in chars {
                if let Some(g) = face.glyph_index(c) {
                    glyphs.insert(c, (g.0, adv(g)));
                }
            }
            let bb = face.global_bounding_box();
            let name: String = face
                .names()
                .into_iter()
                .find(|n| n.name_id == ttf_parser::name_id::POST_SCRIPT_NAME && n.is_unicode())
                .and_then(|n| n.to_string())
                .unwrap_or_else(|| "SHOTFont".to_string())
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
                .collect();
            (
                glyphs,
                adv(ttf_parser::GlyphId(0)),
                name,
                [milli(bb.x_min as f32), milli(bb.y_min as f32), milli(bb.x_max as f32), milli(bb.y_max as f32)],
                milli(face.ascender() as f32),
                milli(face.descender() as f32),
                face.capital_height().map_or(700, |c| milli(c as f32)),
                face.italic_angle(),
            )
        };
        Some(EmbFont { data, name: if name.is_empty() { "SHOTFont".into() } else { name }, bold, glyphs, notdef, used: BTreeMap::new(), bbox, ascent, descent, cap, italic })
    }

    fn width(&self, c: char) -> u16 {
        match c {
            '\t' | '\u{a0}' => self.glyphs.get(&' ').map_or(self.notdef, |g| g.1),
            _ => self.glyphs.get(&c).map_or(self.notdef, |g| g.1),
        }
    }

    /// L'opérande du texte `s` pour l'opérateur `Tj` : les numéros de glyphes, en hexadécimal.
    fn show(&mut self, s: &str) -> String {
        let mut out = String::from("<");
        for c in s.chars() {
            let c = if c == '\t' || c == '\u{a0}' { ' ' } else { c };
            let gid = match self.glyphs.get(&c) {
                Some(&(g, _)) => {
                    self.used.insert(g, c);
                    g
                }
                None => 0,
            };
            out.push_str(&format!("{gid:04X}"));
        }
        out.push('>');
        out
    }

    /// Les cinq objets de la police : `Type0`, `CIDFontType2`, le descripteur, le fichier de la police et la table `ToUnicode`
    /// (écrits aux numéros `first` à `first + 4`).
    fn write(&self, pdf: &mut PdfFile, first: usize) {
        let (type0, cid, desc, file, tounicode) = (first, first + 1, first + 2, first + 3, first + 4);
        let tag = if self.bold { "SHOTBD" } else { "SHOTRG" };
        let base = format!("{tag}+{}", self.name);
        pdf.set_object(type0, format!("<< /Type /Font /Subtype /Type0 /BaseFont /{base} /Encoding /Identity-H /DescendantFonts [{cid} 0 R] /ToUnicode {tounicode} 0 R >>").into_bytes());
        let mut w = String::new();
        for (&g, &c) in &self.used {
            w.push_str(&format!("{g} [{}] ", self.width(c)));
        }
        pdf.set_object(
            cid,
            format!(
                "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{base} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
                 /FontDescriptor {desc} 0 R /DW {} /W [ {w}] /CIDToGIDMap /Identity >>",
                self.notdef
            )
            .into_bytes(),
        );
        let [x0, y0, x1, y1] = self.bbox;
        pdf.set_object(
            desc,
            format!(
                "<< /Type /FontDescriptor /FontName /{base} /Flags 32 /FontBBox [{x0} {y0} {x1} {y1}] /ItalicAngle {} /Ascent {} /Descent {} /CapHeight {} /StemV {} /FontFile2 {file} 0 R >>",
                self.italic, self.ascent, self.descent, self.cap, if self.bold { 140 } else { 80 }
            )
            .into_bytes(),
        );
        // La police entière, compressée.
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        let packed = z.write_all(&self.data).and_then(|()| z.finish()).unwrap_or_default();
        pdf.set_object(file, PdfFile::stream_body(&format!("/Filter /FlateDecode /Length1 {}", self.data.len()), &packed));
        pdf.set_object(tounicode, PdfFile::stream_body("", self.to_unicode().as_bytes()));
    }

    /// La table `ToUnicode` des glyphes écrits (par blocs de 100, comme l'exige la syntaxe).
    fn to_unicode(&self) -> String {
        let mut s = String::from(
            "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
             /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
        );
        let all: Vec<(&u16, &char)> = self.used.iter().collect();
        for chunk in all.chunks(100) {
            s.push_str(&format!("{} beginbfchar\n", chunk.len()));
            for (g, c) in chunk {
                let mut utf16 = String::new();
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf).iter() {
                    utf16.push_str(&format!("{u:04X}"));
                }
                s.push_str(&format!("<{g:04X}> <{utf16}>\n"));
            }
            s.push_str("endbfchar\n");
        }
        s.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
        s
    }
}

/// Les polices d'un PDF : celles de Windows embarquées, ou les polices standard d'avant.
pub enum Fonts {
    Std,
    Emb { regular: Box<EmbFont>, bold: Box<EmbFont>, first_id: usize },
}

fn fonts_dir() -> Option<PathBuf> {
    let root = std::env::var_os("WINDIR").or_else(|| std::env::var_os("SystemRoot"))?;
    Some(PathBuf::from(root).join("Fonts"))
}

/// Les lettres d'un texte (sans espaces ni saut de ligne), une fois chacune.
pub fn chars_of(text: &str) -> Vec<char> {
    let mut v: Vec<char> = text.chars().filter(|c| !c.is_control()).collect();
    v.push(' ');
    v.sort_unstable();
    v.dedup();
    v
}

impl Fonts {
    /// Les polices du texte `text` (toutes ses lettres) : la première paire permise de Windows qui les possède toutes, sinon celle
    /// qui en possède le plus, sinon les polices standard.
    pub fn choose(text: &str) -> Fonts {
        let Some(dir) = fonts_dir() else { return Fonts::Std };
        let chars = chars_of(text);
        let mut best: Option<(usize, EmbFont, EmbFont)> = None;
        for (reg, bold) in PAIRES {
            let Ok(rd) = std::fs::read(dir.join(reg)) else { continue };
            let Ok(bd) = std::fs::read(dir.join(bold)) else { continue };
            let (Some(r), Some(b)) = (EmbFont::open(rd, &chars, false), EmbFont::open(bd, &chars, true)) else { continue };
            let have = chars.iter().filter(|c| r.glyphs.contains_key(c) && b.glyphs.contains_key(c)).count();
            if have == chars.len() {
                return Fonts::Emb { regular: Box::new(r), bold: Box::new(b), first_id: 0 };
            }
            if best.as_ref().is_none_or(|(n, ..)| have > *n) {
                best = Some((have, r, b));
            }
        }
        match best {
            Some((_, r, b)) => Fonts::Emb { regular: Box::new(r), bold: Box::new(b), first_id: 0 },
            None => Fonts::Std,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_embedded(&self) -> bool {
        matches!(self, Fonts::Emb { .. })
    }

    /// Le nom de la police embarquée (essais, rapport).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn name(&self) -> String {
        match self {
            Fonts::Std => "Helvetica".into(),
            Fonts::Emb { regular, .. } => regular.name.clone(),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    /// Les lettres du texte que les polices ne peuvent pas écrire (elles sortent comme un glyphe absent, ou « ? » en standard).
    pub fn missing(&self, text: &str) -> Vec<char> {
        let mut out: Vec<char> = chars_of(text)
            .into_iter()
            .filter(|c| match self {
                Fonts::Std => pdf::winansi(&c.to_string()) == b"?" && *c != '?',
                Fonts::Emb { regular, .. } => !regular.glyphs.contains_key(c),
            })
            .collect();
        out.sort_unstable();
        out
    }

    /// Largeur de `s` en points à la taille `size`.
    pub fn width(&self, s: &str, bold: bool, size: f32) -> f32 {
        match self {
            Fonts::Std => pdf::text_width(s, bold, size),
            Fonts::Emb { regular, bold: b, .. } => {
                let f = if bold { b } else { regular };
                s.chars().map(|c| f.width(c) as f32).sum::<f32>() * size / 1000.0
            }
        }
    }

    /// Le nom de la police dans les ressources d'une page.
    pub fn resource(&self, bold: bool) -> &'static str {
        match (self, bold) {
            (Fonts::Std, false) => "F1",
            (Fonts::Std, true) => "F2",
            (Fonts::Emb { .. }, false) => "F3",
            (Fonts::Emb { .. }, true) => "F4",
        }
    }

    /// L'opérande de `Tj` pour `s`.
    pub fn show(&mut self, s: &str, bold: bool) -> String {
        match self {
            Fonts::Std => pdf::pdf_string(&pdf::winansi(s)),
            Fonts::Emb { regular, bold: b, .. } => (if bold { b } else { regular }).show(s),
        }
    }

    /// Réserve dans `pdf` la place des objets des polices embarquées (à faire avant la première page).
    pub fn attach(&mut self, pdf: &mut PdfFile) {
        if let Fonts::Emb { first_id, .. } = self {
            *first_id = pdf.reserve(10);
            pdf.set_extra_fonts(format!("/F3 {} 0 R /F4 {} 0 R ", *first_id, *first_id + 5));
        }
    }

    /// Écrit les polices embarquées (après la dernière page : on sait alors quelles lettres ont servi).
    pub fn write(&self, pdf: &mut PdfFile) {
        if let Fonts::Emb { regular, bold, first_id } = self {
            regular.write(pdf, *first_id);
            bold.write(pdf, *first_id + 5);
        }
    }
}
