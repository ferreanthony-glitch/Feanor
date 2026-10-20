//! Les formats de la fiche technique (`fiche.rs` décrit, ceci écrit) : CSV, texte pour le
//! presse-papiers, classeur XLSX. Pur : des octets, jamais de fichier.
//!
//! # CSV
//!
//! UTF-8 **avec** BOM (c'est lui qui fait lire les accents à Excel), fins de ligne `\r\n`, séparateur
//! `;` (Excel en français) ou `,`. Un champ est entre guillemets quand il contient le séparateur, un
//! guillemet, un retour à la ligne ; le guillemet est doublé. Les retours à la ligne des textes sont
//! gardés dans le champ. Les nombres décimaux prennent la virgule avec `;` (Excel en français les lit
//! comme des nombres) et le point avec `,`. Aucun texte n'est raccourci.
//!
//! # Formules (injection CSV)
//!
//! Un tableur évalue une cellule de texte qui commence par `=`, `+`, `-` ou `@` (et par une
//! tabulation ou un retour chariot) : un dialogue « =HYPERLINK(…) » ferait ouvrir une page. **Choix :
//! une apostrophe `'` est ajoutée devant** ces textes, en CSV et dans le presse-papiers (elle se voit :
//! `'- Salut`, c'est voulu, la protection ne se cache pas). Les nombres écrits par SHOT (durées…) ne
//! sont pas du texte et ne sont pas touchés. Dans le classeur XLSX les textes sont des chaînes
//! (type `s`) que le tableur ne lit jamais comme une formule : rien n'y est ajouté.
//!
//! # XLSX
//!
//! Office Open XML écrit à la main puis zippé (`zipwrite.rs`) : une feuille « Fiche technique », des
//! chaînes partagées, un en-tête en gras sur fond gris et figé (la première ligne reste en haut
//! quand on défile), un filtre automatique, des largeurs de colonnes, du texte qui passe à la ligne
//! pour les colonnes de texte, les durées en nombres, les time codes en texte. Les vignettes (PNG) sont
//! posées dans les cellules de la colonne « Vignette » (dessin ancré à la cellule, hauteur de ligne
//! réglée) ; le chemin du fichier reste dans la cellule, dessous. Texte long : une cellule XLSX tient
//! 32 767 caractères au plus, au-delà le texte est coupé (avec « … ») et la fiche le dit ; le CSV garde
//! tout.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Write as _;

use crate::fiche::{self, Cellule, Colonne, Fiche, Sorte};
use crate::zipwrite::ZipWriter;

/// Le séparateur d'un CSV.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Separateur {
    /// Excel en français.
    #[default]
    #[serde(rename = "point_virgule")]
    PointVirgule,
    #[serde(rename = "virgule")]
    Virgule,
}

impl Separateur {
    pub fn caractere(self) -> char {
        match self {
            Separateur::PointVirgule => ';',
            Separateur::Virgule => ',',
        }
    }

    /// Le séparateur décimal qui va avec (Excel en français lit « 2,5 » avec `;`).
    pub fn decimale(self) -> char {
        match self {
            Separateur::PointVirgule => ',',
            Separateur::Virgule => '.',
        }
    }
}

/// Le format du fichier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Format {
    #[default]
    #[serde(rename = "xlsx")]
    Xlsx,
    #[serde(rename = "csv")]
    Csv,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Format::Xlsx => "xlsx",
            Format::Csv => "csv",
        }
    }
}

/// Une cellule de texte qui commence par un de ces caractères serait lue comme une formule.
fn est_formule(s: &str) -> bool {
    s.chars().next().is_some_and(|c| matches!(c, '=' | '+' | '-' | '@' | '\t' | '\r'))
}

/// Le texte d'une cellule pour un fichier de texte (CSV, presse-papiers) : une apostrophe devant s'il
/// pourrait être lu comme une formule.
pub fn neutralise(s: &str) -> Cow<'_, str> {
    if est_formule(s) {
        Cow::Owned(format!("'{s}"))
    } else {
        Cow::Borrowed(s)
    }
}

/// Retours à la ligne unifiés en `\n` (un `\r\n` ou un `\r` seul deviennent `\n`).
fn lignes_unifiees(s: &str) -> Cow<'_, str> {
    if s.contains('\r') {
        Cow::Owned(s.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        Cow::Borrowed(s)
    }
}

/// Un champ pour un fichier de texte séparé par `sep` : entre guillemets s'il le faut.
pub fn champ(s: &str, sep: char) -> String {
    if s.contains(sep) || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn valeur_texte(c: &Cellule, decimale: char) -> String {
    match c {
        Cellule::Vide => String::new(),
        Cellule::Texte(t) => neutralise(&lignes_unifiees(t)).into_owned(),
        Cellule::Entier(n) => n.to_string(),
        Cellule::Decimal(f) => {
            let s = fiche::nombre(*f, 3);
            if decimale == '.' {
                s.replace(',', ".")
            } else {
                s
            }
        }
    }
}

fn ligne_texte<'a>(cases: impl Iterator<Item = String> + 'a, sep: char) -> String {
    cases.map(|s| champ(&s, sep)).collect::<Vec<_>>().join(&sep.to_string())
}

/// Le CSV de la fiche : BOM, en-tête, une ligne par ligne, `\r\n`.
pub fn csv(f: &Fiche, sep: Separateur) -> Vec<u8> {
    let (s, d) = (sep.caractere(), sep.decimale());
    let mut out = String::new();
    out.push('\u{feff}');
    out.push_str(&ligne_texte(f.titres().into_iter().map(|t| neutralise(t).into_owned()), s));
    out.push_str("\r\n");
    for l in &f.lignes {
        out.push_str(&ligne_texte(l.cellules.iter().map(|c| valeur_texte(c, d)), s));
        out.push_str("\r\n");
    }
    out.into_bytes()
}

/// Le texte du presse-papiers : séparé par des tabulations, l'en-tête d'abord. Un champ qui contient une
/// tabulation, un guillemet ou un retour à la ligne est entre guillemets (comme Excel et Google Sheets
/// les collent). Décimales à la virgule.
pub fn presse_papiers(f: &Fiche) -> String {
    let mut out = String::new();
    out.push_str(&ligne_texte(f.titres().into_iter().map(|t| neutralise(t).into_owned()), '\t'));
    out.push_str("\r\n");
    for l in &f.lignes {
        out.push_str(&ligne_texte(l.cellules.iter().map(|c| valeur_texte(c, ',')), '\t'));
        out.push_str("\r\n");
    }
    out
}

// ----------------------------------------------------------------------------------- XLSX

/// Une cellule XLSX tient 32 767 caractères au plus.
pub const XLSX_TEXTE_MAX: usize = 32_767;

/// Le texte rendu valide pour XML 1.0 : les caractères de contrôle (sauf tabulation et retours à la
/// ligne) et les non-caractères sont retirés ; les retours à la ligne sont unifiés.
fn texte_xml(s: &str) -> String {
    let s = lignes_unifiees(s);
    s.chars().filter(|&c| matches!(c, '\t' | '\n' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..=char::MAX)).collect()
}

/// Échappe pour un texte ou un attribut XML.
pub fn echappe(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

/// « A », « B »… « Z », « AA » : le nom de la colonne `i` (depuis 0).
pub fn nom_de_colonne(i: usize) -> String {
    let mut n = i + 1;
    let mut s = Vec::new();
    while n > 0 {
        let r = (n - 1) % 26;
        s.push(b'A' + r as u8);
        n = (n - 1) / 26;
    }
    s.reverse();
    String::from_utf8(s).expect("ASCII")
}

/// La taille d'un PNG, lue dans son en-tête IHDR.
pub fn taille_png(png: &[u8]) -> Option<(u32, u32)> {
    if png.len() < 24 || !png.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) || &png[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(png[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(png[20..24].try_into().ok()?);
    (w > 0 && h > 0).then_some((w, h))
}

/// La taille d'affichage d'une vignette de `w`×`h` pixels dans le classeur : elle tient dans le cadre
/// `VIGNETTE_AFFICHEE`, proportions gardées.
pub fn taille_affichee(w: u32, h: u32) -> (u32, u32) {
    let [bw, bh] = fiche::VIGNETTE_AFFICHEE;
    let k = (bw as f64 / w.max(1) as f64).min(bh as f64 / h.max(1) as f64);
    (((w as f64 * k).round() as u32).max(1), ((h as f64 * k).round() as u32).max(1))
}

const EMU_PAR_PIXEL: u64 = 9525;

/// Le résultat : l'archive et ce qu'il a fallu couper.
pub struct Classeur {
    pub octets: Vec<u8>,
    pub avertissements: Vec<String>,
}

const NS_MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const NS_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_PKG_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const NOM_FEUILLE: &str = "Fiche technique";
const STYLE_DEFAUT: u32 = 0;
const STYLE_ENTETE: u32 = 1;
const STYLE_TEXTE: u32 = 2;
const STYLE_DECIMAL: u32 = 3;

/// Le classeur XLSX de la fiche. `images` : les PNG des vignettes, dans l'ordre des lignes qui en ont
/// (`Fiche::images_vignettes`) ; il peut y en avoir moins (les lignes sans image restent vides).
pub fn xlsx(f: &Fiche, images: &[Vec<u8>]) -> Result<Classeur, String> {
    let mut avertissements = Vec::new();
    let n_col = f.colonnes.len();
    if n_col == 0 {
        return Err("aucune colonne".into());
    }
    // --- les chaînes partagées
    let mut chaines: Vec<String> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut total_chaines = 0usize;
    let mut coupes = 0usize;
    let mut chaine = |t: &str| -> usize {
        total_chaines += 1;
        let mut t = texte_xml(t);
        if t.chars().count() > XLSX_TEXTE_MAX {
            coupes += 1;
            t = t.chars().take(XLSX_TEXTE_MAX - 1).collect::<String>() + "…";
        }
        if let Some(&i) = index.get(&t) {
            return i;
        }
        chaines.push(t.clone());
        index.insert(t, chaines.len() - 1);
        chaines.len() - 1
    };

    // --- la feuille
    let derniere_ligne = f.lignes.len() + 1;
    let derniere_col = nom_de_colonne(n_col - 1);
    let col_vignette = f.colonnes.iter().position(|c| *c == Colonne::Vignette);
    let mut feuille = String::new();
    feuille.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    let _ = write!(feuille, "<worksheet xmlns=\"{NS_MAIN}\" xmlns:r=\"{NS_REL}\">");
    feuille.push_str("<sheetPr><pageSetUpPr fitToPage=\"1\"/></sheetPr>");
    let _ = write!(feuille, "<dimension ref=\"A1:{derniere_col}{derniere_ligne}\"/>");
    feuille.push_str("<sheetViews><sheetView workbookViewId=\"0\"><pane ySplit=\"1\" topLeftCell=\"A2\" activePane=\"bottomLeft\" state=\"frozen\"/><selection pane=\"bottomLeft\" activeCell=\"A2\" sqref=\"A2\"/></sheetView></sheetViews>");
    feuille.push_str("<sheetFormatPr defaultRowHeight=\"15\"/>");
    feuille.push_str("<cols>");
    for (i, c) in f.colonnes.iter().enumerate() {
        let _ = write!(feuille, "<col min=\"{0}\" max=\"{0}\" width=\"{1}\" customWidth=\"1\"/>", i + 1, c.largeur());
    }
    feuille.push_str("</cols><sheetData>");
    // L'en-tête.
    feuille.push_str("<row r=\"1\" ht=\"30\" customHeight=\"1\">");
    for (i, c) in f.colonnes.iter().enumerate() {
        let _ = write!(feuille, "<c r=\"{}1\" s=\"{STYLE_ENTETE}\" t=\"s\"><v>{}</v></c>", nom_de_colonne(i), chaine(c.titre()));
    }
    feuille.push_str("</row>");
    // Les lignes ; les vignettes (colonne, ligne, rang de l'image) pour le dessin.
    let mut posees: Vec<(usize, usize, usize)> = Vec::new();
    let mut rang_image = 0usize;
    for (r, l) in f.lignes.iter().enumerate() {
        let numero = r + 2;
        let image = l.vignette.as_ref().and_then(|_| {
            let k = rang_image;
            rang_image += 1;
            images.get(k).and_then(|png| taille_png(png)).map(|(w, h)| (k, w, h))
        });
        let hauteur = match (image, col_vignette) {
            (Some((_, w, h)), Some(_)) => Some(taille_affichee(w, h).1 as f64 * 0.75 + 6.0),
            _ => None,
        };
        match hauteur {
            Some(ht) => {
                let _ = write!(feuille, "<row r=\"{numero}\" ht=\"{ht}\" customHeight=\"1\">");
            }
            None => {
                let _ = write!(feuille, "<row r=\"{numero}\">");
            }
        }
        for (i, (c, cel)) in f.colonnes.iter().zip(&l.cellules).enumerate() {
            let ref_ = format!("{}{numero}", nom_de_colonne(i));
            let style = if c.retour_a_la_ligne() { STYLE_TEXTE } else { STYLE_DEFAUT };
            match cel {
                Cellule::Vide => {}
                Cellule::Texte(t) => {
                    let _ = write!(feuille, "<c r=\"{ref_}\" s=\"{style}\" t=\"s\"><v>{}</v></c>", chaine(t));
                }
                Cellule::Entier(n) => {
                    let _ = write!(feuille, "<c r=\"{ref_}\"><v>{n}</v></c>");
                }
                Cellule::Decimal(x) if x.is_finite() => {
                    let style = if c.sorte() == Sorte::Decimal { STYLE_DECIMAL } else { STYLE_DEFAUT };
                    let _ = write!(feuille, "<c r=\"{ref_}\" s=\"{style}\"><v>{x}</v></c>");
                }
                Cellule::Decimal(_) => {}
            }
        }
        feuille.push_str("</row>");
        if let (Some((k, _, _)), Some(col)) = (image, col_vignette) {
            posees.push((col, r + 1, k));
        }
    }
    feuille.push_str("</sheetData>");
    let _ = write!(feuille, "<autoFilter ref=\"A1:{derniere_col}{derniere_ligne}\"/>");
    feuille.push_str("<pageMargins left=\"0.4\" right=\"0.4\" top=\"0.5\" bottom=\"0.5\" header=\"0.3\" footer=\"0.3\"/>");
    feuille.push_str("<pageSetup paperSize=\"9\" orientation=\"landscape\" fitToWidth=\"1\" fitToHeight=\"0\"/>");
    // Les nombres de plan comme « 12 » écrits en texte : pas de triangle vert « nombre stocké sous forme de texte ».
    let _ = write!(feuille, "<ignoredErrors><ignoredError sqref=\"A1:{derniere_col}{derniere_ligne}\" numberStoredAsText=\"1\"/></ignoredErrors>");
    if !posees.is_empty() {
        feuille.push_str("<drawing r:id=\"rId1\"/>");
    }
    feuille.push_str("</worksheet>");
    if coupes > 0 {
        avertissements.push(format!("{coupes} texte(s) dépassent les {XLSX_TEXTE_MAX} caractères d'une cellule XLSX : coupés dans le classeur (le CSV garde tout)."));
    }

    // --- les images : une seule copie d'une image répétée
    let mut medias: Vec<&Vec<u8>> = Vec::new();
    let mut media_de: Vec<usize> = Vec::new();
    for &(_, _, k) in &posees {
        let png = &images[k];
        let m = match medias.iter().position(|p| *p == png) {
            Some(m) => m,
            None => {
                medias.push(png);
                medias.len() - 1
            }
        };
        media_de.push(m);
    }

    // --- les autres parties
    let mut zip = ZipWriter::new();
    let mut types = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">");
    types.push_str("<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>");
    types.push_str("<Default Extension=\"xml\" ContentType=\"application/xml\"/>");
    if !medias.is_empty() {
        types.push_str("<Default Extension=\"png\" ContentType=\"image/png\"/>");
    }
    types.push_str("<Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>");
    types.push_str("<Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>");
    types.push_str("<Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>");
    types.push_str("<Override PartName=\"/xl/sharedStrings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml\"/>");
    if !posees.is_empty() {
        types.push_str("<Override PartName=\"/xl/drawings/drawing1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.drawing+xml\"/>");
    }
    types.push_str("</Types>");
    zip.deflate("[Content_Types].xml", types.as_bytes())?;

    let racine = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"{NS_PKG_REL}\"><Relationship Id=\"rId1\" Type=\"{NS_REL}/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>"
    );
    zip.deflate("_rels/.rels", racine.as_bytes())?;

    let classeur = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<workbook xmlns=\"{NS_MAIN}\" xmlns:r=\"{NS_REL}\"><bookViews><workbookView xWindow=\"0\" yWindow=\"0\" windowWidth=\"24000\" windowHeight=\"12000\"/></bookViews><sheets><sheet name=\"{NOM_FEUILLE}\" sheetId=\"1\" r:id=\"rId1\"/></sheets><definedNames><definedName name=\"_xlnm._FilterDatabase\" localSheetId=\"0\" hidden=\"1\">'{NOM_FEUILLE}'!$A$1:${derniere_col}${derniere_ligne}</definedName></definedNames></workbook>"
    );
    zip.deflate("xl/workbook.xml", classeur.as_bytes())?;

    let rels_classeur = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"{NS_PKG_REL}\"><Relationship Id=\"rId1\" Type=\"{NS_REL}/worksheet\" Target=\"worksheets/sheet1.xml\"/><Relationship Id=\"rId2\" Type=\"{NS_REL}/styles\" Target=\"styles.xml\"/><Relationship Id=\"rId3\" Type=\"{NS_REL}/sharedStrings\" Target=\"sharedStrings.xml\"/></Relationships>"
    );
    zip.deflate("xl/_rels/workbook.xml.rels", rels_classeur.as_bytes())?;

    zip.deflate("xl/styles.xml", styles().as_bytes())?;

    let mut partagees = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<sst xmlns=\"{NS_MAIN}\" count=\"{total_chaines}\" uniqueCount=\"{}\">", chaines.len());
    for t in &chaines {
        let _ = write!(partagees, "<si><t xml:space=\"preserve\">{}</t></si>", echappe(t));
    }
    partagees.push_str("</sst>");
    zip.deflate("xl/sharedStrings.xml", partagees.as_bytes())?;
    zip.deflate("xl/worksheets/sheet1.xml", feuille.as_bytes())?;

    if !posees.is_empty() {
        let rels_feuille = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"{NS_PKG_REL}\"><Relationship Id=\"rId1\" Type=\"{NS_REL}/drawing\" Target=\"../drawings/drawing1.xml\"/></Relationships>"
        );
        zip.deflate("xl/worksheets/_rels/sheet1.xml.rels", rels_feuille.as_bytes())?;

        let mut dessin = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<xdr:wsDr xmlns:xdr=\"http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"");
        dessin.push_str(NS_REL);
        dessin.push_str("\">");
        let mut rels_dessin = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"{NS_PKG_REL}\">");
        for (n, ((col, ligne, k), m)) in posees.iter().zip(&media_de).enumerate() {
            let (w, h) = taille_png(&images[*k]).unwrap_or((fiche::VIGNETTE_AFFICHEE[0], fiche::VIGNETTE_AFFICHEE[1]));
            let (dw, dh) = taille_affichee(w, h);
            let (cx, cy) = (dw as u64 * EMU_PAR_PIXEL, dh as u64 * EMU_PAR_PIXEL);
            let nom_fichier = f.lignes[*ligne - 1].vignette.as_ref().map_or(String::new(), |v| v.fichier.clone());
            let _ = write!(
                dessin,
                "<xdr:oneCellAnchor><xdr:from><xdr:col>{col}</xdr:col><xdr:colOff>{off}</xdr:colOff><xdr:row>{ligne}</xdr:row><xdr:rowOff>{off}</xdr:rowOff></xdr:from><xdr:ext cx=\"{cx}\" cy=\"{cy}\"/><xdr:pic><xdr:nvPicPr><xdr:cNvPr id=\"{id}\" name=\"Vignette {n1}\" descr=\"{descr}\"/><xdr:cNvPicPr><a:picLocks noChangeAspect=\"1\"/></xdr:cNvPicPr></xdr:nvPicPr><xdr:blipFill><a:blip r:embed=\"rId{rid}\"/><a:stretch><a:fillRect/></a:stretch></xdr:blipFill><xdr:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></xdr:spPr></xdr:pic><xdr:clientData/></xdr:oneCellAnchor>",
                off = 3 * EMU_PAR_PIXEL,
                id = n + 2,
                n1 = n + 1,
                descr = echappe(&nom_fichier),
                rid = n + 1
            );
            let _ = write!(rels_dessin, "<Relationship Id=\"rId{}\" Type=\"{NS_REL}/image\" Target=\"../media/image{}.png\"/>", n + 1, m + 1);
        }
        dessin.push_str("</xdr:wsDr>");
        rels_dessin.push_str("</Relationships>");
        zip.deflate("xl/drawings/drawing1.xml", dessin.as_bytes())?;
        zip.deflate("xl/drawings/_rels/drawing1.xml.rels", rels_dessin.as_bytes())?;
        for (m, png) in medias.iter().enumerate() {
            zip.store(&format!("xl/media/image{}.png", m + 1), png)?;
        }
    }
    Ok(Classeur { octets: zip.finish()?, avertissements })
}

/// Les styles : 0 normal, 1 en-tête (gras, fond gris, filet), 2 texte qui passe à la ligne, 3 décimal « 0,00 ».
fn styles() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<styleSheet xmlns=\"{NS_MAIN}\">\
<fonts count=\"2\"><font><sz val=\"11\"/><name val=\"Calibri\"/><family val=\"2\"/></font><font><b/><sz val=\"11\"/><name val=\"Calibri\"/><family val=\"2\"/></font></fonts>\
<fills count=\"3\"><fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill><fill><patternFill patternType=\"solid\"><fgColor rgb=\"FFD9D9D9\"/><bgColor indexed=\"64\"/></patternFill></fill></fills>\
<borders count=\"2\"><border><left/><right/><top/><bottom/><diagonal/></border><border><left/><right/><top/><bottom style=\"thin\"><color auto=\"1\"/></bottom><diagonal/></border></borders>\
<cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs>\
<cellXfs count=\"4\">\
<xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyAlignment=\"1\"><alignment vertical=\"top\"/></xf>\
<xf numFmtId=\"0\" fontId=\"1\" fillId=\"2\" borderId=\"1\" xfId=\"0\" applyFont=\"1\" applyFill=\"1\" applyBorder=\"1\" applyAlignment=\"1\"><alignment vertical=\"center\" wrapText=\"1\"/></xf>\
<xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyAlignment=\"1\"><alignment vertical=\"top\" wrapText=\"1\"/></xf>\
<xf numFmtId=\"2\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyNumberFormat=\"1\" applyAlignment=\"1\"><alignment vertical=\"top\"/></xf>\
</cellXfs>\
<cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles>\
</styleSheet>"
    )
}
