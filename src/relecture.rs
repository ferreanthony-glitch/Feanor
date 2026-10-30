//! Relecture par le réalisateur (Anthony, 2026-10-20) : le réalisateur ou le client n'a pas SHOT. SHOT lui
//! fabrique **une page HTML qui s'ouvre hors ligne** (le storyboard plan par plan, un champ de notes et un
//! dessin par-dessus chaque image) ; il renvoie un petit fichier `notes-<titre>-<date>.json`, que SHOT relit :
//! chaque note devient un commentaire de la timeline (`commentaires.rs`) sur le bon plan, avec son dessin.
//!
//! Ce module n'a pas d'interface ni de fil : le modèle de ce qui part (`build`), la page (`page`), le fichier
//! de notes qui revient (`parse_notes`, **traité comme venant d'un inconnu**), et leur pose dans le projet
//! (`place`, `apply`). La tâche d'export est dans `relecturejob.rs`, les fenêtres dans `relectureui.rs`.
//!
//! # La page
//!
//! Un seul fichier, images en base64 dedans, ni lien ni script ni police du dehors (une politique de contenu
//! `default-src 'none'` le fait respecter par le navigateur). **Tout texte du projet est échappé**
//! (`esc`) ; le script (`assets/relecture/page.js`) est le même pour tous les projets : il ne contient aucun
//! texte du projet, il lit la page (`data-*`).
//!
//! # Le fichier de notes (`shot-relecture`, version 1)
//!
//! ```json
//! { "format": "shot-relecture", "version": 1, "paquet": "a1b2…", "projet": "Mon film",
//!   "relecteur": "Claire", "cree": 1760000000, "date": "2026-10-20T10:00:00Z", "general": "Très bien.",
//!   "notes": [ { "id": "a1b2…-12-12", "plan": "12", "image": "12", "numero": "3A", "titre": "Plan 3A",
//!                "texte": "Plus large ?", "dessin": "data:image/png;base64,…" } ] }
//! ```
//! `plan` est l'identifiant du plan (`Panel::id`, stable aux déplacements et renumérotations), `image` celui de
//! l'image de storyboard (le plan lui-même pour sa première image, sinon la marque d'image).

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasher, Hasher};
use std::io::Read;
use std::path::Path;

use serde::Deserialize;

use crate::commentaires::{Annotation, Comment, ANNOTATION_B64_MAX, TEXT_MAX};
use crate::document::Document;
use crate::fildecision::Status;

// ------------------------------------------------------------------------------------- limites

/// Un fichier de notes plus gros n'est pas lu.
pub(crate) const NOTES_MAX_BYTES: u64 = 40 * 1024 * 1024;
/// Notes lues au plus dans un fichier (les suivantes sont comptées « ignorées »).
pub(crate) const NOTES_MAX: usize = 3000;
/// Longueur d'un nom de relecteur, d'un identifiant, d'un numéro de plan, d'un titre.
const NAME_MAX: usize = 120;
const ID_MAX: usize = 96;
/// Un dessin plus grand que ça (par côté, ou en pixels) n'est pas décodé.
const OVERLAY_MAX_SIDE: u32 = 4096;
const OVERLAY_MAX_PIXELS: u64 = 8_000_000;
/// Un dessin plus large que ça est réduit à l'import (il couvre le cadre : 1280 points suffisent).
const OVERLAY_STORE_WIDTH: u32 = 1280;
/// Mémoire que le décodeur PNG a le droit de prendre pour un dessin.
const OVERLAY_ALLOC: u64 = 64 * 1024 * 1024;
/// Couleur des commentaires venus d'une relecture (le jaune de la palette des commentaires).
pub(crate) const REVIEW_COLOR: [u8; 3] = [250, 204, 21];

/// Largeur des images de la page (pixels) et qualité JPEG ; la lecture animée est plus petite.
pub(crate) const STILL_WIDTH: u32 = 960;
pub(crate) const STILL_QUALITY: u8 = 80;
pub(crate) const PLAY_WIDTH: u32 = 480;
pub(crate) const PLAY_QUALITY: u8 = 62;
/// Images échantillonnées par plan pour la lecture animée, au plus.
const PLAY_PER_PLAN_MAX: usize = 16;
/// Total d'images de lecture visé pour tout le film (au-delà, moins par plan).
const PLAY_TOTAL: usize = 1500;

// ------------------------------------------------------------------------------------- petits outils

/// Texte du projet prêt à être écrit dans une page HTML (texte ou attribut entre guillemets) : `& < > " ' \``
/// remplacés, caractères de contrôle retirés (sauf retour à la ligne et tabulation).
pub(crate) fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            '`' => out.push_str("&#96;"),
            '\n' | '\t' => out.push(c),
            '\r' => {}
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// « 1 note », « 12 notes ».
pub(crate) fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n > 1 { many } else { one })
}

/// Un identifiant au hasard (hexadécimal) : le numéro d'un paquet, le nonce de la page.
pub(crate) fn random_hex(digits: usize) -> String {
    let mut out = String::new();
    while out.len() < digits {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
        h.write_u32(std::process::id());
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out.truncate(digits);
    out
}

/// Texte venu d'ailleurs, nettoyé : fins de ligne en `\n`, caractères de contrôle et de sens d'écriture
/// retirés, coupé à `max` caractères.
pub(crate) fn clean_text(s: &str, max: usize) -> String {
    let mut out = String::with_capacity(s.len().min(max * 2));
    let mut n = 0;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        let c = match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    continue;
                }
                '\n'
            }
            '\n' | '\t' => c,
            c if c.is_control() => continue,
            // Inversions de sens d'écriture (U+202A à U+202E, U+2066 à U+2069) : elles trompent la lecture.
            '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' => continue,
            c => c,
        };
        out.push(c);
        n += 1;
        if n >= max {
            break;
        }
    }
    out.trim().to_string()
}

/// Un identifiant de note : lettres, chiffres, `-`, `_` et `.` seulement, 96 caractères au plus.
pub(crate) fn clean_id(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty() && s.len() <= ID_MAX && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))).then(|| s.to_string())
}

// ------------------------------------------------------------------------------------- ce qui part

/// Ce que la page montre en plus.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// Lecture animée de chaque plan (images réduites, jouées dans la page).
    pub playback: bool,
    /// Les commentaires déjà posés sur la timeline, en lecture seule.
    pub comments: bool,
    /// Un PDF à imprimer, avec des cadres de notes à remplir à la main, à côté de la page.
    pub pdf: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { playback: false, comments: true, pdf: false }
    }
}

/// Ce que SHOT sait du paquet qu'il fabrique.
#[derive(Clone, Debug, Default)]
pub struct Meta {
    /// Le titre de la page.
    pub title: String,
    /// Le nom du projet, tel que SHOT le reconnaîtra à l'import.
    pub project: String,
    /// Identifiant du paquet (les notes qui reviennent le portent).
    pub paquet: String,
    /// L'envoi dont la page est la relecture (« v2 au réal »).
    pub envoi: Option<String>,
    /// Secondes depuis 1970.
    pub now: u64,
    /// Nonce de la politique de contenu de la page.
    pub nonce: String,
    /// Début du nom des fichiers de notes que la page fait télécharger.
    pub file_stem: String,
    /// Le numéro de l'envoi dont c'est la relecture : le fichier de notes le porte, et les notes qui reviennent
    /// sont posées sur cet envoi (`fildecision.rs`).
    pub send_id: Option<u64>,
    /// L'envoi dont la page rend compte des notes (celui d'avant `send_id`, sinon le dernier) : pour chaque plan, ses notes
    /// avec leur statut (fait, refusé et pourquoi) ; rien : pas de rappel.
    pub prior_send: Option<u64>,
    /// Les plans (identifiants) qui ont changé depuis l'envoi : la page les met en avant.
    pub changed: HashSet<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommentPage {
    pub author: String,
    pub text: String,
    pub date: String,
    /// « image 3 ».
    pub place: String,
}

/// Une note de l'envoi d'avant, avec ce qu'on en a fait : le relecteur la retrouve sur la page suivante.
#[derive(Clone, Debug, PartialEq)]
pub struct PriorPage {
    /// Le numéro de la note dans SHOT : la page le garde pour que la réponse du relecteur retrouve sa note.
    pub id: u64,
    pub author: String,
    pub text: String,
    pub date: String,
    /// « image 3 ».
    pub place: String,
    pub status: Status,
    /// La raison d'un refus.
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoardPage {
    /// Identifiant de l'image de storyboard (celui du plan pour la première).
    pub id: u64,
    /// « Image 2 sur 3 · 12 i » ; vide pour un plan d'une seule image.
    pub caption: String,
    pub start: usize,
    pub end: usize,
    pub action: String,
    pub dialogue: String,
    pub notes: String,
    /// Rang de son image dans la liste des images à calculer (`build`).
    pub image: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanPage {
    pub id: u64,
    pub number: String,
    /// « Plan 12A ».
    pub title: String,
    /// « V2 » seulement si le plan a plusieurs versions.
    pub version: Option<u32>,
    /// La séquence (son nom), pour les intertitres.
    pub sequence: String,
    pub duration: String,
    pub frames: usize,
    pub boards: Vec<BoardPage>,
    pub comments: Vec<CommentPage>,
    /// Les notes de l'envoi d'avant (relecteur, pas les notes internes), avec leur statut.
    pub prior: Vec<PriorPage>,
    /// Le plan a changé depuis l'envoi de référence.
    pub changed: bool,
    /// La lecture animée : (rang de l'image dans la liste des images, images tenues).
    pub play: Vec<(usize, u32)>,
}

#[derive(Clone, Debug)]
pub struct Package {
    pub meta: Meta,
    pub fps: u32,
    /// Taille du cadre du film.
    pub size: [usize; 2],
    pub plans: Vec<PlanPage>,
    /// Combien d'images de storyboard (les premières images de la liste) ; les suivantes sont de la lecture animée.
    pub stills: usize,
}

/// Les `max` positions les plus régulières de `0..len` (la première image toujours).
pub(crate) fn sample_positions(len: usize, max: usize) -> Vec<usize> {
    let len = len.max(1);
    let max = max.max(1);
    if len <= max {
        return (0..len).collect();
    }
    (0..max).map(|k| k * len / max).collect()
}

/// Ce qui part pour les plans `panels` (rangs, dans l'ordre du film) : le paquet et les images à calculer
/// (plan, image du plan) : d'abord les images de storyboard, puis les images de la lecture animée.
pub fn build(doc: &Document, panels: &[usize], opts: &Options, meta: Meta) -> (Package, Vec<(usize, usize)>) {
    let mut frames: Vec<(usize, usize)> = Vec::new();
    let mut plans: Vec<PlanPage> = Vec::new();
    let date_of = |secs: u64| if secs == 0 { String::new() } else { crate::envois::date_courte(secs) };
    for &i in panels.iter().filter(|&&i| i < doc.panels.len()) {
        let p = &doc.panels[i];
        let len = p.frame_count();
        let spans = crate::board::plan_boards(doc, i);
        let count = spans.len();
        let mut boards = Vec::with_capacity(count);
        for (k, (id, start, end)) in spans.into_iter().enumerate() {
            let (action, dialogue, notes) = match p.marks.iter().find(|m| m.id == id) {
                Some(m) => (m.action.clone(), m.dialogue.clone(), m.notes.clone()),
                None => (p.action.clone(), p.dialogue.clone(), p.notes.clone()),
            };
            let span = crate::storyboard::duration_label(end - start, doc.fps);
            let caption = if count > 1 { format!("Image {} sur {count} · {span}", k + 1) } else { String::new() };
            boards.push(BoardPage { id, caption, start, end, action, dialogue, notes, image: frames.len() });
            frames.push((i, start));
        }
        // Les notes de l'envoi d'avant, avec leur statut (celles du relecteur : les notes internes ne sortent pas).
        let in_prior = |c: &Comment| meta.prior_send.is_some() && c.follow.send == meta.prior_send && c.source() != crate::fildecision::Source::Shot;
        let mut prior: Vec<&Comment> = doc.comments.iter().filter(|c| c.panel == p.id && in_prior(c)).collect();
        prior.sort_by_key(|c| (c.frame, c.id));
        let prior: Vec<PriorPage> = prior
            .into_iter()
            .map(|c| PriorPage {
                id: c.id,
                author: c.author.clone(),
                text: c.text.clone(),
                date: date_of(c.created),
                place: format!("image {}", c.frame.min(len.saturating_sub(1)) + 1),
                status: c.status(),
                reason: c.follow.reason.clone(),
            })
            .collect();
        let comments = if opts.comments {
            let mut here: Vec<&Comment> = doc.comments.iter().filter(|c| c.panel == p.id && c.is_open() && !in_prior(c)).collect();
            here.sort_by_key(|c| (c.frame, c.id));
            here.into_iter()
                .map(|c| CommentPage {
                    author: c.author.clone(),
                    text: c.text.clone(),
                    date: date_of(c.created),
                    place: format!("image {}", c.frame.min(len.saturating_sub(1)) + 1),
                })
                .collect()
        } else {
            Vec::new()
        };
        let version = (doc.nombre_de_versions(p.id) >= 2).then(|| doc.numero_de_version(p.id));
        plans.push(PlanPage {
            id: p.id,
            number: doc.scene_label(p.scene),
            title: crate::storyboard::panel_title(doc, i),
            version,
            sequence: doc.sequence_label(doc.sequence_of(i)),
            duration: crate::storyboard::duration_label(len, doc.fps),
            frames: len,
            boards,
            comments,
            prior,
            changed: meta.changed.contains(&p.id),
            play: Vec::new(),
        });
    }
    let stills = frames.len();
    if opts.playback && !plans.is_empty() {
        let per_plan = (PLAY_TOTAL / plans.len()).clamp(3, PLAY_PER_PLAN_MAX);
        for (plan, &i) in plans.iter_mut().zip(panels.iter().filter(|&&i| i < doc.panels.len())) {
            if plan.frames < 2 {
                continue;
            }
            let at = sample_positions(plan.frames, per_plan);
            for (k, &pos) in at.iter().enumerate() {
                let next = at.get(k + 1).copied().unwrap_or(plan.frames);
                plan.play.push((frames.len(), (next - pos).max(1) as u32));
                frames.push((i, pos));
            }
        }
    }
    let package = Package { meta, fps: doc.fps, size: doc.frame, plans, stills };
    (package, frames)
}

/// La lecture animée avec ses images identiques à la suite fondues en une seule (`images` : les JPEG, par
/// rang) : un plan tenu ne pèse qu'une image.
pub(crate) fn merge_identical(play: &[(usize, u32)], images: &[Vec<u8>]) -> Vec<(usize, u32)> {
    let mut out: Vec<(usize, u32)> = Vec::with_capacity(play.len());
    for &(image, hold) in play {
        match out.last_mut() {
            Some((last, held)) if images.get(*last) == images.get(image) => *held += hold,
            _ => out.push((image, hold)),
        }
    }
    out
}

// ------------------------------------------------------------------------------------- la page

fn data_uri(jpeg: &[u8]) -> String {
    format!("data:image/jpeg;base64,{}", crate::base64::encode(jpeg))
}

const PAGE_CSS: &str = include_str!("../assets/relecture/page.css");
const PAGE_JS: &str = include_str!("../assets/relecture/page.js");

fn text_lines(b: &BoardPage) -> String {
    let mut out = String::new();
    for (class, label, text) in [("action", "Action", &b.action), ("dialogue", "Dialogue", &b.dialogue), ("notes", "Notes", &b.notes)] {
        let text = text.trim();
        if !text.is_empty() {
            out.push_str(&format!("<p class=\"{class}\"><b>{label}\u{a0}:</b> {}</p>", esc(text)));
        }
    }
    out
}

/// La page de relecture : `images` sont les JPEG, dans l'ordre de la liste de `build` (les premiers sont les
/// images de storyboard).
pub fn page(pkg: &Package, images: &[Vec<u8>]) -> String {
    let m = &pkg.meta;
    let [fw, fh] = pkg.size;
    // Taille du dessin, dans le repère de la page : 960 de large, la même proportion que le cadre.
    let (cw, ch) = (STILL_WIDTH, ((STILL_WIDTH as f64 * fh.max(1) as f64 / fw.max(1) as f64).round() as u32).max(1));
    let mut h = String::with_capacity(64 * 1024);
    h.push_str("<!doctype html>\n<html lang=\"fr\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    // Rien ne sort de la page : ni requête, ni formulaire, ni script autre que celui-ci.
    h.push_str(&format!(
        "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data: blob:; style-src 'unsafe-inline'; script-src 'nonce-{}'; form-action 'none'; base-uri 'none'\">\n",
        esc(&m.nonce)
    ));
    h.push_str(&format!("<title>{}</title>\n<style>\n{PAGE_CSS}</style>\n</head>\n", esc(&format!("Relecture · {}", m.title))));
    h.push_str(&format!(
        "<body data-paquet=\"{}\" data-projet=\"{}\" data-fichier=\"{}\"{}>\n",
        esc(&m.paquet),
        esc(&m.project),
        esc(&m.file_stem),
        m.send_id.map_or(String::new(), |e| format!(" data-envoi=\"{e}\""))
    ));
    // En-tête collant : le titre, le nom du relecteur, le nombre de notes, le bouton.
    h.push_str("<header class=\"haut\"><div class=\"haut-in\">");
    h.push_str(&format!("<h1>{}</h1>", esc(&m.title)));
    h.push_str("<label class=\"nom\"><span>Votre nom</span><input id=\"relecteur\" type=\"text\" autocomplete=\"name\" maxlength=\"120\" placeholder=\"Prénom Nom\"></label>");
    h.push_str("<span class=\"compte\" id=\"compte\" aria-live=\"polite\">Aucune note</span>");
    h.push_str("<button class=\"bouton\" id=\"enregistrer\" type=\"button\">Enregistrer mes notes</button>");
    h.push_str("</div></header>\n<main>\n");
    let mut meta_line = vec![format!("Relecture du {}", crate::envois::date_courte(m.now))];
    if let Some(e) = &m.envoi {
        meta_line.push(format!("envoi «\u{a0}{}\u{a0}»", e));
    }
    meta_line.push(plural(pkg.plans.len(), "plan", "plans"));
    meta_line.push(format!("{} i/s", pkg.fps));
    h.push_str(&format!("<p class=\"meta\">{}</p>\n", esc(&meta_line.join(" · "))));
    h.push_str(
        "<div class=\"aide\"><b>Comment relire</b><ol>\
         <li>Écrivez votre nom en haut.</li>\
         <li>Pour chaque image, écrivez votre note dans le cadre jaune, et si vous voulez dessinez dessus (bouton «\u{a0}Dessiner\u{a0}»).</li>\
         <li>Cliquez sur «\u{a0}Enregistrer mes notes\u{a0}», puis envoyez le fichier obtenu par e-mail.</li>\
         </ol><p>Cette page fonctionne sans internet : rien n\u{2019}est envoyé de votre appareil tant que vous n\u{2019}envoyez pas le fichier vous-même.</p></div>\n",
    );
    h.push_str("<p class=\"message\" id=\"message\" role=\"status\"></p>\n");
    // Sommaire : un lien par plan.
    h.push_str("<nav class=\"sommaire\" aria-label=\"Plans\">");
    for p in &pkg.plans {
        h.push_str(&format!("<a href=\"#p{}\">{}</a>", p.id, esc(&p.number)));
    }
    h.push_str("</nav>\n");
    let mut current: Option<&str> = None;
    for p in &pkg.plans {
        if current != Some(p.sequence.as_str()) {
            if current.is_some() {
                h.push_str("</section>\n");
            }
            h.push_str(&format!("<section class=\"sequence\"><h2>Séquence {}</h2>\n", esc(&p.sequence)));
            current = Some(&p.sequence);
        }
        h.push_str(&format!("<article class=\"plan\" id=\"p{}\"><div class=\"plan-tete\"><h3>{}", p.id, esc(&p.title)));
        if let Some(v) = p.version {
            h.push_str(&format!(" <span class=\"version\">V{v}</span>"));
        }
        h.push_str(&format!("</h3><span class=\"duree\">{}</span>", esc(&p.duration)));
        if p.changed {
            h.push_str("<span class=\"modifie\">Modifié depuis l\u{2019}envoi précédent</span>");
        }
        h.push_str("</div>\n");
        // La lecture animée.
        if p.play.len() > 1 {
            h.push_str(&format!("<div class=\"lecteur\" data-fps=\"{}\"><div class=\"pile\">", pkg.fps.max(1)));
            for (k, &(image, hold)) in p.play.iter().enumerate() {
                if let Some(jpeg) = images.get(image) {
                    h.push_str(&format!("<img data-h=\"{hold}\" alt=\"{}, image {}\" src=\"{}\">", esc(&p.title), k + 1, data_uri(jpeg)));
                }
            }
            h.push_str("</div><div class=\"barre\"><button class=\"bouton calme petit\" type=\"button\">Lire le plan</button><input type=\"range\" min=\"0\" max=\"0\" value=\"0\" aria-label=\"Position dans le plan\"><span class=\"tc duree\"></span></div></div>\n");
        }
        h.push_str("<div class=\"cartes\">\n");
        for b in &p.boards {
            let Some(jpeg) = images.get(b.image) else { continue };
            h.push_str(&format!(
                "<section class=\"carte\" data-plan=\"{}\" data-image=\"{}\" data-numero=\"{}\" data-titre=\"{}\">",
                p.id,
                b.id,
                esc(&p.number),
                esc(&p.title)
            ));
            h.push_str(&format!("<div class=\"cadre\" data-w=\"{cw}\" data-h=\"{ch}\"><img alt=\"{}\" src=\"{}\"></div>", esc(&p.title), data_uri(jpeg)));
            if !b.caption.is_empty() {
                h.push_str(&format!("<p class=\"legende\">{}</p>", esc(&b.caption)));
            }
            let texts = text_lines(b);
            if !texts.is_empty() {
                h.push_str(&format!("<div class=\"textes\">{texts}</div>"));
            }
            // Les commentaires déjà posés : sur la première carte du plan ou sur celle où ils tombent.
            let here: Vec<&CommentPage> = p.comments.iter().filter(|c| comment_on_board(c, b, p)).collect();
            if !here.is_empty() {
                h.push_str("<div class=\"existants\"><h4>Déjà noté dans SHOT</h4>");
                for c in here {
                    let who = if c.author.trim().is_empty() { "Sans nom".to_string() } else { c.author.clone() };
                    let when = if c.date.is_empty() { String::new() } else { format!(" · {}", c.date) };
                    h.push_str(&format!("<p><b>{}</b> ({}{}) : {}</p>", esc(&who), esc(&c.place), esc(&when), esc(&c.text)));
                }
                h.push_str("</div>");
            }
            // Les notes de l'envoi d'avant et ce qu'on en a fait : la page fait voir ce qui a été traité.
            let before: Vec<&PriorPage> = p.prior.iter().filter(|c| place_on_board(&c.place, b, p)).collect();
            if !before.is_empty() {
                h.push_str("<div class=\"precedentes\"><h4>Vos notes de l\u{2019}envoi précédent</h4>");
                for c in before {
                    let (class, label) = match c.status {
                        Status::Done => ("fait", "Fait"),
                        Status::Refused => ("refuse", "Refusé"),
                        Status::Discussion => ("discussion", "En discussion"),
                        Status::Todo => ("afaire", "Pas encore traitée"),
                    };
                    let who = if c.author.trim().is_empty() { "Vous".to_string() } else { c.author.clone() };
                    let when = if c.date.is_empty() { String::new() } else { format!(" · {}", c.date) };
                    h.push_str(&format!("<div class=\"prec {class}\" data-note=\"{}\"><p><span class=\"badge\">{label}</span> <b>{}</b> ({}{}) : {}</p>", c.id, esc(&who), esc(&c.place), esc(&when), esc(&c.text)));
                    if c.status == Status::Refused {
                        let why = if c.reason.trim().is_empty() { "aucune raison donnée".to_string() } else { c.reason.clone() };
                        h.push_str(&format!("<p class=\"raison\">Raison du refus\u{a0}: {}</p>", esc(&why)));
                    }
                    h.push_str(&format!("<input class=\"suite\" type=\"text\" maxlength=\"2000\" data-note=\"{}\" placeholder=\"Votre réponse (facultatif)\" aria-label=\"Votre réponse à cette note\"></div>", c.id));
                }
                h.push_str("</div>");
            }
            h.push_str("<div class=\"outils\"></div>");
            h.push_str(&format!("<textarea class=\"note\" rows=\"3\" maxlength=\"{TEXT_MAX}\" placeholder=\"Votre note sur cette image…\" aria-label=\"Votre note sur {}\"></textarea>", esc(&p.title)));
            h.push_str("</section>\n");
        }
        h.push_str("</div></article>\n");
    }
    if current.is_some() {
        h.push_str("</section>\n");
    }
    h.push_str(
        "<section class=\"general\"><h2>Note générale</h2><textarea class=\"note\" id=\"general\" rows=\"4\" maxlength=\"20000\" \
         placeholder=\"Une remarque sur l\u{2019}ensemble…\" aria-label=\"Note générale\"></textarea></section>\n",
    );
    h.push_str("<details class=\"secours\" id=\"secours\" hidden><summary>Le fichier ne s\u{2019}est pas téléchargé\u{a0}?</summary><p>Copiez ce texte dans un fichier de nom «\u{a0}notes.json\u{a0}» et envoyez-le.</p><textarea readonly></textarea></details>\n");
    h.push_str("<p class=\"pied\">Page fabriquée par SHOT. Elle ne contient aucune connexion : vos notes restent sur votre appareil jusqu\u{2019}à l\u{2019}envoi du fichier.</p>\n");
    h.push_str("</main>\n");
    h.push_str(&format!("<script nonce=\"{}\">\n{PAGE_JS}</script>\n</body>\n</html>\n", esc(&m.nonce)));
    h
}

/// Le commentaire `c` (« image 3 ») tombe-t-il sur l'image de storyboard `b` du plan `p` ?
fn comment_on_board(c: &CommentPage, b: &BoardPage, p: &PlanPage) -> bool {
    place_on_board(&c.place, b, p)
}

/// Un repère « image 3 » tombe-t-il sur l'image de storyboard `b` du plan `p` ?
fn place_on_board(place: &str, b: &BoardPage, p: &PlanPage) -> bool {
    let n: usize = place.trim_start_matches("image ").parse().unwrap_or(1);
    let at = n.saturating_sub(1);
    let last = p.boards.last().map(|l| l.id);
    at >= b.start && (at < b.end || Some(b.id) == last)
}

// ------------------------------------------------------------------------------------- les notes qui reviennent

/// Une note lue (nettoyée, validée).
#[derive(Clone, Debug)]
pub struct Note {
    pub id: String,
    pub plan: Option<u64>,
    pub image: Option<u64>,
    pub number: String,
    pub title: String,
    pub text: String,
    pub drawing: Option<Annotation>,
    /// Secondes depuis 1970 (0 : inconnue).
    pub created: u64,
    /// La note générale du fichier (sans plan).
    pub general: bool,
}

/// Une réponse du relecteur à une note de l'envoi d'avant (le numéro de la note dans SHOT : la page le garde).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suite {
    pub note: u64,
    pub text: String,
}

/// Un fichier de notes lu.
#[derive(Clone, Debug)]
pub struct Notes {
    /// Le numéro du paquet (la page dont elles viennent).
    pub paquet: String,
    /// L'envoi dont la page était la relecture ; rien : la page n'en disait pas.
    pub send: Option<u64>,
    /// Les réponses aux notes de l'envoi d'avant.
    pub suites: Vec<Suite>,
    pub project: String,
    pub reviewer: String,
    pub created: u64,
    pub notes: Vec<Note>,
    /// Notes du fichier qu'on n'a pas pu lire (illisibles, en trop, dessin refusé) : dites à l'utilisateur.
    pub ignored: usize,
    /// Dessins refusés (trop gros, pas un PNG…) dont la note a quand même été gardée.
    pub drawings_refused: usize,
}

/// Un identifiant venu du fichier : un nombre ou son écriture en texte.
#[derive(Deserialize)]
#[serde(untagged)]
enum RawId {
    Number(u64),
    Text(String),
}

impl RawId {
    fn value(&self) -> Option<u64> {
        match self {
            RawId::Number(n) => Some(*n),
            RawId::Text(t) => t.trim().parse().ok(),
        }
    }
}

#[derive(Deserialize)]
struct RawNote {
    #[serde(default)]
    id: Option<RawId>,
    #[serde(default)]
    plan: Option<RawId>,
    #[serde(default)]
    image: Option<RawId>,
    #[serde(default)]
    numero: Option<String>,
    #[serde(default)]
    titre: Option<String>,
    #[serde(default)]
    texte: Option<String>,
    #[serde(default)]
    dessin: Option<String>,
    #[serde(default)]
    cree: Option<u64>,
}

#[derive(Deserialize)]
struct RawSuite {
    #[serde(default)]
    note: Option<RawId>,
    #[serde(default)]
    texte: Option<String>,
}

#[derive(Deserialize)]
struct RawFile {
    format: String,
    version: u32,
    #[serde(default)]
    envoi: Option<RawId>,
    #[serde(default)]
    suites: Vec<RawSuite>,
    #[serde(default)]
    paquet: Option<String>,
    #[serde(default)]
    projet: Option<String>,
    #[serde(default)]
    relecteur: Option<String>,
    #[serde(default)]
    cree: Option<u64>,
    #[serde(default)]
    general: Option<String>,
    #[serde(default)]
    notes: Vec<RawNote>,
}

pub(crate) const FORMAT_NOTES: &str = "shot-relecture";
pub(crate) const VERSION_NOTES: u32 = 1;

/// Le dessin d'un relecteur (`data:image/png;base64,…`), vérifié : un vrai PNG d'une taille raisonnable,
/// décodé sous limites, réduit s'il est grand, réencodé proprement (rien du fichier d'origine n'est gardé que les
/// pixels). `Ok(None)` : un dessin entièrement transparent (rien à montrer).
pub(crate) fn decode_overlay(data_url: &str) -> Result<Option<Annotation>, String> {
    let b64 = data_url.strip_prefix("data:image/png;base64,").ok_or("ce n'est pas un PNG")?;
    if b64.len() > ANNOTATION_B64_MAX {
        return Err("dessin trop lourd".into());
    }
    let bytes = crate::base64::decode(b64).ok_or("dessin illisible")?;
    if !bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Err("ce n'est pas un PNG".into());
    }
    let reader = |bytes: &[u8]| image::ImageReader::with_format(std::io::Cursor::new(bytes.to_vec()), image::ImageFormat::Png);
    let (w, h) = reader(&bytes).into_dimensions().map_err(|_| "dessin illisible".to_string())?;
    if w == 0 || h == 0 || w > OVERLAY_MAX_SIDE || h > OVERLAY_MAX_SIDE || u64::from(w) * u64::from(h) > OVERLAY_MAX_PIXELS {
        return Err("dessin trop grand".into());
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(OVERLAY_MAX_SIDE);
    limits.max_image_height = Some(OVERLAY_MAX_SIDE);
    limits.max_alloc = Some(OVERLAY_ALLOC);
    let mut r = reader(&bytes);
    r.limits(limits);
    let rgba = r.decode().map_err(|_| "dessin illisible".to_string())?.to_rgba8();
    if rgba.pixels().all(|p| p.0[3] == 0) {
        return Ok(None);
    }
    let rgba = if rgba.width() > OVERLAY_STORE_WIDTH {
        let nh = ((u64::from(rgba.height()) * u64::from(OVERLAY_STORE_WIDTH)) / u64::from(rgba.width())).max(1) as u32;
        image::imageops::resize(&rgba, OVERLAY_STORE_WIDTH, nh, image::imageops::FilterType::Triangle)
    } else {
        rgba
    };
    let png = crate::io::encode_png(rgba.as_raw(), rgba.width() as usize, rgba.height() as usize)?;
    Annotation::from_png(png).map(Some).ok_or_else(|| "dessin trop lourd".to_string())
}

/// Le contenu d'un fichier de notes. Le fichier vient de quelqu'un d'autre : taille bornée (`read_notes_file`),
/// format et version vérifiés, textes nettoyés et coupés, identifiants restreints, dessins décodés sous limites,
/// au plus `NOTES_MAX` notes, aucune note ne fait échouer les autres.
pub fn parse_notes(bytes: &[u8]) -> Result<Notes, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "ce n'est pas un fichier de notes de relecture (texte illisible)".to_string())?;
    let raw: RawFile = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|_| "ce n'est pas un fichier de notes de relecture".to_string())?;
    if raw.format != FORMAT_NOTES {
        return Err("ce n'est pas un fichier de notes de relecture".into());
    }
    if raw.version > VERSION_NOTES {
        return Err(format!("notes écrites par une version plus récente de la page (format {})", raw.version));
    }
    let paquet = raw.paquet.as_deref().and_then(clean_id).unwrap_or_default();
    let reviewer = clean_text(raw.relecteur.as_deref().unwrap_or(""), NAME_MAX);
    let file_created = raw.cree.unwrap_or(0);
    let mut notes: Vec<Note> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let (mut ignored, mut refused) = (0usize, 0usize);
    let general = clean_text(raw.general.as_deref().unwrap_or(""), TEXT_MAX);
    if !general.is_empty() {
        let id = format!("{}-general", if paquet.is_empty() { "sans-paquet" } else { &paquet });
        seen.insert(id.clone());
        notes.push(Note { id, plan: None, image: None, number: String::new(), title: String::new(), text: general, drawing: None, created: file_created, general: true });
    }
    for (k, n) in raw.notes.into_iter().enumerate() {
        if k >= NOTES_MAX {
            ignored += 1;
            continue;
        }
        let (plan, image) = (n.plan.as_ref().and_then(RawId::value), n.image.as_ref().and_then(RawId::value));
        let text = clean_text(n.texte.as_deref().unwrap_or(""), TEXT_MAX);
        let drawing = match n.dessin.as_deref().filter(|d| !d.is_empty()) {
            None => None,
            Some(d) => match decode_overlay(d) {
                Ok(a) => a,
                Err(_) => {
                    refused += 1;
                    None
                }
            },
        };
        if text.is_empty() && drawing.is_none() {
            ignored += 1;
            continue;
        }
        // Sans identifiant lisible, la clé est refaite du paquet, du plan et de l'image (stable d'un envoi à l'autre).
        let given = match &n.id {
            Some(RawId::Text(t)) => clean_id(t),
            Some(RawId::Number(v)) => Some(v.to_string()),
            None => None,
        };
        let id = match given.or_else(|| plan.map(|p| format!("{}-{p}-{}", if paquet.is_empty() { "sans-paquet" } else { &paquet }, image.unwrap_or(0)))) {
            Some(id) => id,
            None => {
                ignored += 1;
                continue;
            }
        };
        if !seen.insert(id.clone()) {
            ignored += 1;
            continue;
        }
        notes.push(Note {
            id,
            plan,
            image,
            number: clean_text(n.numero.as_deref().unwrap_or(""), NAME_MAX),
            title: clean_text(n.titre.as_deref().unwrap_or(""), NAME_MAX),
            text,
            drawing,
            created: n.cree.unwrap_or(0),
            general: false,
        });
    }
    let suites: Vec<Suite> = raw
        .suites
        .iter()
        .take(NOTES_MAX)
        .filter_map(|s| {
            let note = s.note.as_ref().and_then(RawId::value)?;
            let text = clean_text(s.texte.as_deref().unwrap_or(""), TEXT_MAX);
            (!text.is_empty()).then_some(Suite { note, text })
        })
        .collect();
    Ok(Notes {
        paquet,
        send: raw.envoi.as_ref().and_then(RawId::value),
        suites,
        project: clean_text(raw.projet.as_deref().unwrap_or(""), NAME_MAX),
        reviewer: if reviewer.is_empty() { "Relecteur".into() } else { reviewer },
        created: file_created,
        notes,
        ignored,
        drawings_refused: refused,
    })
}

/// Lit un fichier de notes sur le disque, sans le croire : refusé s'il est plus gros que `NOTES_MAX_BYTES`
/// (même si sa taille annoncée ment : la lecture s'arrête à la limite).
pub fn read_notes_file(path: &Path) -> Result<Notes, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{} : {e}", path.display()))?;
    if file.metadata().map_or(0, |m| m.len()) > NOTES_MAX_BYTES {
        return Err(format!("fichier trop gros (plus de {} Mo)", NOTES_MAX_BYTES / 1024 / 1024));
    }
    let mut bytes = Vec::new();
    file.take(NOTES_MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|e| format!("{} : {e}", path.display()))?;
    if bytes.len() as u64 > NOTES_MAX_BYTES {
        return Err(format!("fichier trop gros (plus de {} Mo)", NOTES_MAX_BYTES / 1024 / 1024));
    }
    parse_notes(&bytes)
}

// ------------------------------------------------------------------------------------- la pose dans le projet

/// Comment une note a trouvé son plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Via {
    /// Par l'identifiant du plan.
    Id,
    /// Par son numéro (l'identifiant n'existe plus ou n'a jamais existé dans ce projet).
    Number,
    /// La note générale, posée sur le premier plan concerné.
    General,
}

/// Où va une note.
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    /// Rang de la note dans `Notes::notes`.
    pub note: usize,
    /// Le plan (rang) et comment on l'a trouvé ; `None` : aucun plan.
    pub plan: Option<(usize, Via)>,
    /// L'image du plan où poser le commentaire.
    pub frame: usize,
    /// L'image de storyboard visée a disparu : le commentaire est posé au début du plan.
    pub image_lost: bool,
    /// Un commentaire de ce projet porte déjà cette note (importée avant).
    pub already: bool,
}

/// Où iraient les notes de `notes` dans `doc`. Les notes sans plan ont `plan: None`.
pub fn place(doc: &Document, notes: &Notes) -> Vec<Placement> {
    let imported: HashSet<&str> = doc.comments.iter().filter_map(|c| c.note.as_deref()).collect();
    let by_id: HashMap<u64, usize> = doc.panels.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
    let mut by_number: HashMap<String, usize> = HashMap::new();
    for (i, p) in doc.panels.iter().enumerate() {
        by_number.entry(doc.scene_label(p.scene)).or_insert(i);
    }
    let mut out: Vec<Placement> = notes
        .notes
        .iter()
        .enumerate()
        .map(|(k, n)| {
            let already = imported.contains(n.id.as_str());
            let plan = n
                .plan
                .and_then(|id| by_id.get(&id).map(|&i| (i, Via::Id)))
                .or_else(|| (!n.number.is_empty()).then(|| by_number.get(&n.number).map(|&i| (i, Via::Number))).flatten());
            let (frame, image_lost) = match (plan, n.image) {
                (Some((i, _)), Some(image)) => match crate::board::plan_boards(doc, i).into_iter().find(|b| b.0 == image) {
                    Some((_, start, _)) => (start, false),
                    None => (0, true),
                },
                _ => (0, false),
            };
            Placement { note: k, plan: if n.general { None } else { plan }, frame, image_lost, already }
        })
        .collect();
    // La note générale : sur le premier plan (dans l'ordre du film) qui reçoit une autre note, sinon le premier plan.
    let first = out.iter().filter(|p| !notes.notes[p.note].general).filter_map(|p| p.plan.map(|(i, _)| i)).min().unwrap_or(0);
    for p in out.iter_mut().filter(|p| notes.notes[p.note].general) {
        if !doc.panels.is_empty() {
            p.plan = Some((first.min(doc.panels.len() - 1), Via::General));
        }
    }
    out
}

/// Ce qu'une importation a fait (ou ferait).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub imported: usize,
    /// Plans qui reçoivent au moins une note.
    pub plans: usize,
    pub already: usize,
    /// Rangs (dans `Notes::notes`) des notes sans plan.
    pub unmatched: Vec<usize>,
    pub by_number: usize,
    pub with_drawing: usize,
    pub ignored: usize,
    pub drawings_refused: usize,
    /// Réponses aux notes de l'envoi d'avant qui rejoignent le fil de leur note.
    pub replies: usize,
}

impl Summary {
    /// Ce que l'importation apporterait de neuf : des notes, ou des réponses.
    pub fn new_things(&self) -> usize {
        self.imported + self.replies
    }

    /// « 12 notes importées sur 9 plans, 1 note sans plan (plan supprimé) ».
    pub fn message(&self) -> String {
        let mut parts = vec![format!("{} importée{} sur {}", plural(self.imported, "note", "notes"), if self.imported > 1 { "s" } else { "" }, plural(self.plans, "plan", "plans"))];
        if self.already > 0 {
            parts.push(format!("{} déjà importée{}", plural(self.already, "note", "notes"), if self.already > 1 { "s" } else { "" }));
        }
        if self.replies > 0 {
            parts.push(format!("{} au fil des notes d'avant", plural(self.replies, "réponse", "réponses")));
        }
        if !self.unmatched.is_empty() {
            parts.push(format!("{} sans plan (plan supprimé)", plural(self.unmatched.len(), "note", "notes")));
        }
        if self.ignored > 0 {
            parts.push(format!("{} ignorée{} (illisible{})", plural(self.ignored, "note", "notes"), if self.ignored > 1 { "s" } else { "" }, if self.ignored > 1 { "s" } else { "" }));
        }
        if self.drawings_refused > 0 {
            parts.push(format!("{} refusé{}", plural(self.drawings_refused, "dessin", "dessins"), if self.drawings_refused > 1 { "s" } else { "" }));
        }
        let mut s = parts.join(", ");
        s.push('.');
        s
    }
}

/// Ce que `apply` ferait, sans rien poser.
pub fn preview(doc: &Document, notes: &Notes, places: &[Placement], unmatched_to: Option<usize>) -> Summary {
    let mut sum = Summary { ignored: notes.ignored, drawings_refused: notes.drawings_refused, ..Default::default() };
    let mut plans: HashSet<usize> = HashSet::new();
    for p in places {
        if p.already {
            sum.already += 1;
            continue;
        }
        let target = p.plan.map(|(i, _)| i).or(unmatched_to.filter(|&i| i < doc.panels.len()));
        match target {
            Some(i) => {
                sum.imported += 1;
                plans.insert(i);
                sum.with_drawing += usize::from(notes.notes[p.note].drawing.is_some());
                sum.by_number += usize::from(matches!(p.plan, Some((_, Via::Number))));
            }
            None => sum.unmatched.push(p.note),
        }
    }
    sum.plans = plans.len();
    sum.replies = notes.suites.iter().filter(|s| suite_target(doc, notes, s).is_some()).count();
    sum
}

/// La clé d'une réponse du relecteur (la même pour un même fichier : un second import ne la double pas).
fn suite_key(notes: &Notes, s: &Suite) -> String {
    format!("{}-{}", if notes.paquet.is_empty() { "sans-paquet" } else { &notes.paquet }, s.note)
}

/// La note de SHOT à qui la réponse s'adresse, si elle existe encore et si cette réponse n'y est pas déjà.
fn suite_target(doc: &Document, notes: &Notes, s: &Suite) -> Option<u64> {
    let c = doc.comment(s.note)?;
    let key = suite_key(notes, s);
    (!c.follow.replies.iter().any(|r| r.external.as_deref() == Some(key.as_str()))).then_some(c.id)
}

/// Pose les notes en commentaires de la timeline : auteur = le relecteur, date de la note, texte, dessin ; la clé
/// de la note est gardée pour qu'un second import du même fichier ne les double pas. `unmatched_to` : le plan où poser
/// aussi les notes sans plan (avec leur provenance dans le texte). Rien n'est annulable ici : l'appelant a pris
/// l'étape d'annulation avant.
pub fn apply(doc: &mut Document, notes: &Notes, places: &[Placement], unmatched_to: Option<usize>, now: u64) -> Summary {
    let sum = preview(doc, notes, places, unmatched_to);
    for p in places {
        if p.already {
            continue;
        }
        let n = &notes.notes[p.note];
        let (target, frame, loose) = match p.plan {
            Some((i, _)) => (Some(i), p.frame, false),
            None => (unmatched_to.filter(|&i| i < doc.panels.len()), 0, true),
        };
        let Some(i) = target else { continue };
        let panel = doc.panels[i].id;
        let mut text = n.text.clone();
        if n.general {
            text = format!("Note générale : {text}");
        } else if loose {
            let which = if !n.title.is_empty() { n.title.clone() } else if !n.number.is_empty() { format!("Plan {}", n.number) } else { "plan inconnu".into() };
            text = format!("[Note pour {which}, introuvable dans ce projet] {text}");
        } else if p.image_lost {
            text = format!("[Image de storyboard disparue] {text}");
        }
        let created = [n.created, notes.created].into_iter().find(|&c| c > 0 && c <= now + 86_400).unwrap_or(now);
        let id = doc.alloc();
        // L'envoi que la page relisait (sinon le dernier) et la version affichée du plan.
        let (last, plan_version) = doc.stamp_follow(panel);
        let send = notes.send.filter(|s| doc.envois.trouve(*s).is_some()).or(last);
        let follow = crate::fildecision::Follow { source: crate::fildecision::Source::Relecture, send, plan_version, ..Default::default() };
        doc.comments.push(Comment {
            id,
            panel,
            frame,
            text: clean_text(&text, TEXT_MAX),
            author: notes.reviewer.clone(),
            color: REVIEW_COLOR,
            created,
            done: false,
            note: Some(n.id.clone()),
            annotation: n.drawing.clone(),
            follow,
        });
    }
    // Les réponses aux notes de l'envoi d'avant rejoignent le fil de leur note.
    for s in &notes.suites {
        if let Some(id) = suite_target(doc, notes, s) {
            let created = if notes.created > 0 && notes.created <= now + 86_400 { notes.created } else { now };
            if let Some(c) = doc.comment_mut(id) {
                if c.follow.replies.len() < crate::fildecision::REPLIES_MAX {
                    c.follow.replies.push(crate::fildecision::Reply { author: notes.reviewer.clone(), text: s.text.clone(), created, source: crate::fildecision::Source::Relecture, external: Some(suite_key(notes, s)) });
                }
            }
        }
    }
    sum
}
