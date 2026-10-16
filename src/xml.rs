//! Lecture d'un fichier XML en arbre (14a, projets Storyboard Pro) : chaque élément garde son
//! nom, ses attributs (déjà décodés : `&lt;` devient `<`) et ses enfants. Le texte entre les
//! balises est gardé aussi, mais les fichiers lus s'en servent peu.

use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion};

/// Un élément XML et tout ce qu'il contient.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Element>,
    pub text: String,
}

impl Element {
    /// Valeur de l'attribut `key`.
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    /// Premier enfant nommé `name`.
    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.name == name)
    }

    /// Enfants nommés `name`, dans l'ordre du fichier.
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// Le descendant au bout du chemin `a/b/c` (premier enfant de ce nom à chaque pas).
    pub fn path(&self, path: &str) -> Option<&Element> {
        path.split('/').try_fold(self, |e, step| e.child(step))
    }

    /// Tous les descendants nommés `name`, dans l'ordre du fichier (sans entrer dans ceux trouvés).
    pub fn find_all<'a>(&'a self, name: &str, out: &mut Vec<&'a Element>) {
        for c in &self.children {
            if c.name == name {
                out.push(c);
            } else {
                c.find_all(name, out);
            }
        }
    }
}

/// Lit tout un document ; renvoie son élément racine.
pub fn parse(src: &str) -> Result<Element, String> {
    let mut reader = Reader::from_str(src);
    // Pile des éléments ouverts ; le fond est une racine fictive.
    let mut stack: Vec<Element> = vec![Element::default()];
    // Texte : les blancs de mise en page (avant tout autre texte) ne sont pas gardés ; une
    // entité (`&amp;`) arrive à part, les blancs qui l'entourent restent.
    let add_text = |stack: &mut Vec<Element>, s: &str| {
        let e = stack.last_mut().expect("racine");
        if !e.text.is_empty() || !s.trim().is_empty() {
            e.text.push_str(s);
        }
    };
    loop {
        let event = reader.read_event().map_err(|e| format!("XML illisible à l'octet {} : {e}", reader.buffer_position()))?;
        match event {
            Event::Start(s) => stack.push(open(&s)?),
            Event::Empty(s) => {
                let e = open(&s)?;
                stack.last_mut().expect("racine").children.push(e);
            }
            Event::End(_) => {
                if stack.len() < 2 {
                    return Err("XML mal formé : balise fermante en trop".into());
                }
                let mut e = stack.pop().expect("élément ouvert");
                let end = e.text.trim_end().len();
                e.text.truncate(end);
                stack.last_mut().expect("racine").children.push(e);
            }
            Event::Text(t) => {
                let s = t.decode().map_err(|e| e.to_string())?;
                add_text(&mut stack, &s);
            }
            Event::CData(t) => {
                let s = t.decode().map_err(|e| e.to_string())?;
                add_text(&mut stack, &s);
            }
            Event::GeneralRef(r) => {
                let c = match r.resolve_char_ref().map_err(|e| e.to_string())? {
                    Some(c) => c.to_string(),
                    None => predefined(&r.decode().map_err(|e| e.to_string())?).to_string(),
                };
                add_text(&mut stack, &c);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if stack.len() != 1 {
        return Err("XML incomplet : des balises ne sont pas fermées".into());
    }
    let root = stack.pop().expect("racine");
    root.children.into_iter().next().ok_or_else(|| "XML vide".to_string())
}

fn open(s: &quick_xml::events::BytesStart) -> Result<Element, String> {
    let name = String::from_utf8_lossy(s.name().as_ref()).into_owned();
    let mut attrs = Vec::new();
    for a in s.attributes().with_checks(false) {
        let a = a.map_err(|e| e.to_string())?;
        let key = String::from_utf8_lossy(a.key.as_ref()).into_owned();
        let value = a.normalized_value(XmlVersion::Implicit1_0).map_err(|e| e.to_string())?.into_owned();
        attrs.push((key, value));
    }
    Ok(Element { name, attrs, children: Vec::new(), text: String::new() })
}

/// Les cinq entités prédéfinies du XML (les autres restent telles quelles).
fn predefined(name: &str) -> &str {
    match name {
        "lt" => "<",
        "gt" => ">",
        "amp" => "&",
        "apos" => "'",
        "quot" => "\"",
        other => other,
    }
}
