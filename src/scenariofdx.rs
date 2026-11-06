//! Importer un scénario : les formats à paragraphes typés, Final Draft (`.fdx`, du XML) et Word
//! (`.docx`, un zip dont `word/document.xml` est lu avec `zipread`).
//!
//! Le XML est lu en flux (`quick-xml`, déjà dans le projet) plutôt qu'avec `xml.rs` : ce dernier
//! retire les espaces en fin de texte, et un paragraphe de Final Draft ou de Word est fait de
//! plusieurs morceaux (« Bonjour » en gras, « tout le monde » à côté) dont l'espace de fin compte.
//!
//! - **Final Draft** : le type de chaque paragraphe (`Scene Heading`, `Action`, `Character`,
//!   `Dialogue`, `Parenthetical`, `Transition`…) dit ce qu'il est ; un paragraphe `Character` et
//!   les `Parenthetical` / `Dialogue` qui le suivent font une réplique ; `DualDialogue` fait des
//!   répliques simultanées ; les notes de script (`ScriptNote`) deviennent des notes.
//! - **Word** : si les paragraphes portent les styles d'un logiciel de scénario (au moins trois,
//!   « Scene Heading », « Character », « Dialogue »…), ils sont lus comme Final Draft ; sinon le
//!   texte (un paragraphe = une ligne) passe par la lecture du texte brut.

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use crate::scenario::{self, en_tete_depuis, parse_cue, Element, Format, Kind, Script, Speech, TextOptions};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Flavor {
    Fdx,
    Docx,
}

/// Un paragraphe lu : son style (ou type), son texte, ses notes.
#[derive(Clone, Default)]
struct RawPara {
    style: String,
    text: String,
    notes: Vec<String>,
    /// Deuxième personnage d'un `DualDialogue`.
    dual: bool,
    /// Dans la page de titre (Final Draft).
    title_page: bool,
    /// Le numéro de scène que Final Draft écrit dans l'attribut `Number` d'un intitulé (vide : aucun).
    number: String,
}

/// Le nom d'une balise sans son préfixe d'espace de noms (`w:p` donne `p`).
fn local(name: &[u8]) -> String {
    String::from_utf8_lossy(name).rsplit(':').next().unwrap_or_default().to_string()
}

fn attr(e: &BytesStart, key: &str) -> Option<String> {
    for a in e.attributes().with_checks(false).flatten() {
        let k = String::from_utf8_lossy(a.key.as_ref()).into_owned();
        if k == key || k.rsplit(':').next() == Some(key) {
            return a.normalized_value(XmlVersion::Implicit1_0).ok().map(|v| v.into_owned());
        }
    }
    None
}

fn predefined(name: &str) -> String {
    match name {
        "lt" => "<".into(),
        "gt" => ">".into(),
        "amp" => "&".into(),
        "apos" => "'".into(),
        "quot" => "\"".into(),
        other => format!("&{other};"),
    }
}

struct Walk {
    flavor: Flavor,
    paras: Vec<RawPara>,
    cur: Option<RawPara>,
    in_text: bool,
    note_depth: usize,
    note_buf: String,
    in_dual: bool,
    dual_chars: usize,
    in_title: bool,
    root: Option<String>,
    depth: usize,
}

impl Walk {
    /// Du texte lu dans une balise de texte (`Text`, `w:t`).
    fn push_text(&mut self, s: &str) {
        if self.in_text {
            self.push_raw(s);
        }
    }

    /// Du texte de la mise en forme (tabulation, saut de ligne), hors balise de texte.
    fn push_raw(&mut self, s: &str) {
        if self.note_depth > 0 {
            self.note_buf.push_str(s);
        } else if let Some(c) = self.cur.as_mut() {
            c.text.push_str(s);
        }
    }

    fn end_para(&mut self) {
        if let Some(mut p) = self.cur.take() {
            if self.flavor == Flavor::Fdx && self.in_dual && p.style.eq_ignore_ascii_case("Character") {
                self.dual_chars += 1;
                p.dual = self.dual_chars >= 2;
            }
            self.paras.push(p);
        }
    }

    fn start(&mut self, e: &BytesStart, empty: bool) {
        let name = local(e.name().as_ref());
        if self.root.is_none() {
            self.root = Some(name.clone());
        }
        if !empty {
            self.depth += 1;
        }
        match (self.flavor, name.as_str()) {
            (Flavor::Fdx, "TitlePage") => self.in_title = !empty,
            (Flavor::Fdx, "DualDialogue") => {
                self.in_dual = !empty;
                self.dual_chars = 0;
            }
            (Flavor::Fdx, "ScriptNote") if !empty => {
                if self.note_depth == 0 {
                    self.note_buf.clear();
                }
                self.note_depth += 1;
            }
            (Flavor::Fdx, "Paragraph") if self.note_depth == 0 => {
                self.cur = Some(RawPara { style: attr(e, "Type").unwrap_or_default(), title_page: self.in_title, number: attr(e, "Number").unwrap_or_default(), ..RawPara::default() });
                if empty {
                    self.end_para();
                }
            }
            (Flavor::Fdx, "Text") => self.in_text = !empty,
            (Flavor::Docx, "p") => {
                self.cur = Some(RawPara::default());
                if empty {
                    self.end_para();
                }
            }
            (Flavor::Docx, "pStyle") => {
                if let (Some(c), Some(v)) = (self.cur.as_mut(), attr(e, "val")) {
                    c.style = v;
                }
            }
            (Flavor::Docx, "t") => self.in_text = !empty,
            (Flavor::Docx, "tab") => self.push_raw(" "),
            (Flavor::Docx, "br" | "cr") => self.push_raw("\n"),
            _ => {}
        }
    }

    fn end(&mut self, name: &str) {
        self.depth = self.depth.saturating_sub(1);
        match (self.flavor, name) {
            (Flavor::Fdx, "TitlePage") => self.in_title = false,
            (Flavor::Fdx, "DualDialogue") => self.in_dual = false,
            (Flavor::Fdx, "ScriptNote") => {
                if self.note_depth > 0 {
                    self.note_depth -= 1;
                    if self.note_depth == 0 {
                        let n = self.note_buf.split_whitespace().collect::<Vec<_>>().join(" ");
                        if let (false, Some(c)) = (n.is_empty(), self.cur.as_mut()) {
                            c.notes.push(n);
                        }
                        self.note_buf.clear();
                    }
                }
            }
            (Flavor::Fdx, "Paragraph") if self.note_depth == 0 => self.end_para(),
            (Flavor::Fdx, "Text") | (Flavor::Docx, "t") => self.in_text = false,
            (Flavor::Docx, "p") => self.end_para(),
            _ => {}
        }
    }
}

/// Lit les paragraphes d'un document XML ; renvoie aussi le nom de l'élément racine.
fn read_paragraphs(src: &str, flavor: Flavor, progress: &dyn Fn(f32)) -> Result<(Vec<RawPara>, String), String> {
    let mut reader = Reader::from_str(src);
    let mut w = Walk {
        flavor,
        paras: Vec::new(),
        cur: None,
        in_text: false,
        note_depth: 0,
        note_buf: String::new(),
        in_dual: false,
        dual_chars: 0,
        in_title: false,
        root: None,
        depth: 0,
    };
    let total = src.len().max(1);
    let mut count = 0usize;
    loop {
        let event = reader.read_event().map_err(|e| format!("XML mal formé à l'octet {} : {e}.", reader.buffer_position()))?;
        count += 1;
        if count % 20_000 == 0 {
            progress(0.1 + 0.8 * (reader.buffer_position() as f32 / total as f32).min(1.0));
        }
        match event {
            Event::Start(e) => w.start(&e, false),
            Event::Empty(e) => w.start(&e, true),
            Event::End(e) => {
                let name = local(e.name().as_ref());
                w.end(&name);
            }
            Event::Text(t) => {
                let s = t.decode().map_err(|e| format!("Texte illisible : {e}."))?;
                w.push_text(&s);
            }
            Event::CData(t) => {
                let s = t.decode().map_err(|e| format!("Texte illisible : {e}."))?;
                w.push_text(&s);
            }
            Event::GeneralRef(r) => {
                let c = match r.resolve_char_ref().map_err(|e| format!("Entité illisible : {e}."))? {
                    Some(c) => c.to_string(),
                    None => predefined(&r.decode().map_err(|e| format!("Entité illisible : {e}."))?),
                };
                w.push_text(&c);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if w.depth > 0 {
        return Err("Le fichier est tronqué ou incomplet (des balises ne sont pas fermées).".into());
    }
    let root = w.root.take().ok_or("Le fichier XML est vide.")?;
    Ok((w.paras, root))
}

// ------------------------------------------------------------------------------------ types

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ParaKind {
    Heading,
    Action,
    Character,
    Dialogue,
    Parenthetical,
    Transition,
    Section,
    Skip,
}

/// Ce que dit le style ou le type d'un paragraphe (aucun : pas un style de scénario).
fn kind_of(style: &str) -> Option<ParaKind> {
    let k: String = style.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
    Some(match k.as_str() {
        "sceneheading" | "scenetitle" | "slugline" | "montageheader" | "titredescene" | "titredescène" => ParaKind::Heading,
        "action" | "general" | "shot" => ParaKind::Action,
        "character" | "personnage" => ParaKind::Character,
        "dialogue" | "dialog" | "replique" | "réplique" => ParaKind::Dialogue,
        "parenthetical" | "parenthese" | "parenthèse" => ParaKind::Parenthetical,
        "transition" => ParaKind::Transition,
        "newact" | "endofact" => ParaKind::Section,
        "castlist" => ParaKind::Skip,
        _ => return None,
    })
}

fn lines_of(text: &str) -> String {
    text.split('\n').map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

/// Les éléments d'une suite de paragraphes typés ; un paragraphe d'un type inconnu est de l'action.
fn build_typed(paras: &[RawPara]) -> Vec<Element> {
    let mut out: Vec<Element> = Vec::new();
    let mut speech: Option<Element> = None;
    let flush = |speech: &mut Option<Element>, out: &mut Vec<Element>| {
        if let Some(s) = speech.take() {
            out.push(s);
        }
    };
    for p in paras {
        let text = lines_of(&p.text);
        let kind = kind_of(&p.style).unwrap_or(ParaKind::Action);
        if text.is_empty() {
            // Un paragraphe vide ne dit rien ; ses notes restent.
            out.extend(p.notes.iter().map(|n| Element::new(Kind::Note(n.clone()))));
            continue;
        }
        match kind {
            ParaKind::Character => {
                flush(&mut speech, &mut out);
                let one_line = text.replace('\n', " ");
                let sp = match parse_cue(&one_line, true) {
                    Some(c) => Speech { name: c.name, tags: c.tags, dual: p.dual || c.dual, ..Speech::default() },
                    None => Speech { name: one_line, dual: p.dual, ..Speech::default() },
                };
                speech = Some(Element { notes: p.notes.clone(), ..Element::new(Kind::Dialogue(sp)) });
            }
            ParaKind::Parenthetical | ParaKind::Dialogue => {
                let one_line = text.replace('\n', " ");
                if speech.is_none() {
                    if kind == ParaKind::Parenthetical {
                        // Une parenthèse hors réplique : de l'action.
                        out.push(Element { notes: p.notes.clone(), ..Element::new(Kind::Action(one_line)) });
                        continue;
                    }
                    speech = Some(Element::new(Kind::Dialogue(Speech::default())));
                }
                if let Some(Element { kind: Kind::Dialogue(s), notes, .. }) = speech.as_mut() {
                    if kind == ParaKind::Parenthetical {
                        let wrapped = if one_line.starts_with('(') { one_line } else { format!("({one_line})") };
                        s.asides.push(wrapped);
                    } else {
                        if !s.text.is_empty() {
                            s.text.push(' ');
                        }
                        s.text.push_str(&one_line);
                    }
                    notes.extend(p.notes.iter().cloned());
                }
            }
            ParaKind::Heading => {
                flush(&mut speech, &mut out);
                out.push(en_tete_depuis(&text.replace('\n', " "), p.number.clone()).element(p.notes.clone()));
            }
            ParaKind::Transition => {
                flush(&mut speech, &mut out);
                out.push(Element { notes: p.notes.clone(), ..Element::new(Kind::Transition(text.replace('\n', " "))) });
            }
            ParaKind::Section => {
                flush(&mut speech, &mut out);
                out.push(Element { notes: p.notes.clone(), ..Element::new(Kind::Section(text.replace('\n', " "))) });
            }
            ParaKind::Skip => {}
            ParaKind::Action => {
                flush(&mut speech, &mut out);
                out.push(Element { notes: p.notes.clone(), ..Element::new(Kind::Action(text)) });
            }
        }
    }
    flush(&mut speech, &mut out);
    out
}

// ------------------------------------------------------------------------------------ Final Draft

/// Lit un fichier Final Draft (`.fdx`).
pub fn parse_fdx(src: &str, progress: &dyn Fn(f32)) -> Result<Script, String> {
    let (paras, root) = read_paragraphs(src, Flavor::Fdx, progress).map_err(|e| format!("Fichier .fdx illisible : {e}"))?;
    if root != "FinalDraft" {
        return Err(format!("Ce fichier XML n'est pas un scénario .fdx (il commence par « {root} » au lieu de « FinalDraft »)."));
    }
    let mut script = Script::empty(Format::Fdx);
    let title_texts: Vec<String> = paras.iter().filter(|p| p.title_page).map(|p| lines_of(&p.text).replace('\n', " ")).filter(|t| !t.is_empty()).collect();
    script.title = title_texts.first().cloned().unwrap_or_default();
    if let Some(i) = title_texts.iter().position(|t| matches!(t.to_lowercase().trim_end_matches(':'), "written by" | "by" | "écrit par" | "par" | "scénario de" | "un film de")) {
        script.author = title_texts.get(i + 1).cloned().unwrap_or_default();
    }
    let body: Vec<RawPara> = paras.into_iter().filter(|p| !p.title_page).collect();
    if body.is_empty() {
        return Err("Ce fichier .fdx ne contient aucun paragraphe de scénario.".into());
    }
    script.elements = build_typed(&body);
    Ok(script)
}

// ------------------------------------------------------------------------------------ Word

/// Lit un document Word (`.docx`).
pub fn parse_docx(bytes: &[u8], progress: &dyn Fn(f32)) -> Result<Script, String> {
    let zip = crate::zipread::Zip::open(bytes).map_err(|e| format!("Ce n'est pas un document .docx lisible : {e}."))?;
    let data = zip.read("word/document.xml").ok_or("Ce document .docx ne contient pas de texte (word/document.xml est absent ou illisible).")?;
    let src = scenario::decode_text(&data);
    let (paras, _) = read_paragraphs(&src, Flavor::Docx, progress).map_err(|e| format!("Document .docx illisible : {e}"))?;
    let typed = paras
        .iter()
        .filter(|p| matches!(kind_of(&p.style), Some(ParaKind::Heading | ParaKind::Character | ParaKind::Dialogue | ParaKind::Parenthetical | ParaKind::Transition)))
        .count();
    if typed >= 3 {
        let mut script = Script::empty(Format::Docx);
        script.elements = build_typed(&paras);
        return Ok(script);
    }
    // Sans styles de scénario : le texte, un paragraphe par ligne.
    let text = paras.iter().map(|p| p.text.as_str()).collect::<Vec<_>>().join("\n");
    let mut script = scenario::parse_text(&text, TextOptions { line_is_paragraph: true }, progress);
    script.format = Format::Docx;
    Ok(script)
}
