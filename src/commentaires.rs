//! Commentaires sur la timeline (Anthony, 2026-10-18) : « un petit curseur apparaît là où un
//! commentaire est laissé ; une boîte de dialogue apparaît quand on survole le curseur ; une
//! fenêtre dans les panneaux permet au commentateur d'écrire le commentaire, de choisir la couleur
//! du curseur ; le curseur sur la timeline est déplaçable ».
//!
//! - Où : un commentaire tient à un plan (son id) et à une image de ce plan (`frame`, comptée
//!   depuis 0). Il montre la même image dans le montage (l'image du film = début du plan +
//!   `frame`) et dans la timeline du plan : il suit son plan quand le montage change (plans
//!   avancés, reculés, supprimés avec leurs commentaires), `split_panel` et l'assemblage de plans
//!   les emmènent avec leurs images. Si le plan devient plus court que `frame`, il se montre sur
//!   sa dernière image (`frame` reste tel quel : rallonger le plan le remet où il était).
//! - Rangement : `Document::comments` et `Structure::comments` : une étape d'annulation (`Edit::Structure`)
//!   les rétablit avec le reste, sans autre mécanisme.
//! - Fichier : `commentaires.json` à côté du manifeste (`project.rs`), sans changer `VERSION` :
//!   un projet sans ce fichier n'a pas de commentaires, et une version de Pelure qui ne le connaît
//!   pas le laisse là (elle n'efface que le contenu de `dessins/`, `sons/`, `modeles/`, `textures/`).
//! - L'écran : repères dans la règle du montage et de la timeline (`commentairesui.rs`), panneau
//!   Commentaires.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::document::Document;
use crate::history::Edit;

/// Nom du fichier, dans le dossier du projet.
pub(crate) const FILE: &str = "commentaires.json";
const FORMAT: &str = "shot-commentaires";
const VERSION: u32 = 1;
/// Couleur d'un commentaire neuf (cyan : ni la marque d'image, ni le signet, ni la tête de lecture).
pub(crate) const DEFAULT_COLOR: [u8; 3] = [60, 200, 215];
/// Un texte plus long qu'ici est coupé à la lecture (un fichier abîmé ne gèle pas l'interface).
pub(crate) const TEXT_MAX: usize = 20_000;
/// Un dessin de relecteur plus lourd qu'ici (PNG en base64) n'est pas gardé à la lecture du fichier.
pub(crate) const ANNOTATION_B64_MAX: usize = 1_600_000;
/// Largeur d'un repère à gauche et à droite de son mât, et hauteur de sa tête, en points : la zone
/// où l'on prend le repère (`marker_hits`).
pub(crate) const FLAG_LEFT: f32 = 3.0;
pub(crate) const FLAG_RIGHT: f32 = 10.0;
pub(crate) const FLAG_HEAD: f32 = 12.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Comment {
    pub id: u64,
    /// Le plan (son id).
    #[serde(rename = "plan")]
    pub panel: u64,
    /// L'image dans le plan, depuis 0.
    #[serde(rename = "image")]
    pub frame: usize,
    #[serde(rename = "texte", default)]
    pub text: String,
    #[serde(rename = "auteur", default)]
    pub author: String,
    #[serde(rename = "couleur", default = "default_color")]
    pub color: [u8; 3],
    /// Secondes depuis 1970 (0 : inconnue).
    #[serde(rename = "cree", default)]
    pub created: u64,
    /// Résolu : le repère est estompé.
    #[serde(rename = "resolu", default, skip_serializing_if = "is_false")]
    pub done: bool,
    /// Venu d'une note de relecture (`relecture.rs`) : l'identifiant de la note, pour ne pas la
    /// poser deux fois si le même fichier est importé de nouveau.
    #[serde(rename = "note", default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Le dessin du relecteur par-dessus l'image du plan.
    #[serde(rename = "dessin", default, skip_serializing_if = "Option::is_none", with = "annotation_serde")]
    pub annotation: Option<Annotation>,
}

/// Le dessin qu'un relecteur a fait par-dessus l'image d'un plan : un PNG transparent de la taille du
/// cadre (réduit à l'import), partagé entre les copies du commentaire (annulation, plans rangés).
#[derive(Clone, Debug)]
pub struct Annotation(pub Arc<Vec<u8>>);

impl PartialEq for Annotation {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.0 == other.0
    }
}

impl Annotation {
    /// Octets d'un PNG, si l'en-tête est celui d'un PNG et que le fichier n'est pas trop lourd.
    pub(crate) fn from_png(bytes: Vec<u8>) -> Option<Annotation> {
        let sig = bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        (sig && bytes.len() <= ANNOTATION_B64_MAX / 4 * 3).then(|| Annotation(Arc::new(bytes)))
    }
}

/// `commentaires.json` garde le dessin en base64 ; un dessin illisible (trop lourd, pas du base64, pas un
/// PNG) est laissé de côté sans empêcher de lire le commentaire.
mod annotation_serde {
    use super::{Annotation, ANNOTATION_B64_MAX};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &Option<Annotation>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(a) => s.serialize_str(&crate::base64::encode(&a.0)),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Annotation>, D::Error> {
        let text = Option::<String>::deserialize(d)?;
        Ok(text.filter(|t| t.len() <= ANNOTATION_B64_MAX).and_then(|t| crate::base64::decode(&t)).and_then(Annotation::from_png))
    }
}

fn default_color() -> [u8; 3] {
    DEFAULT_COLOR
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Serialize, Deserialize)]
struct Sidecar {
    format: String,
    version: u32,
    #[serde(default)]
    commentaires: Vec<Comment>,
}

/// Le contenu de `commentaires.json` pour `doc` ; rien s'il n'y a aucun commentaire (le fichier
/// est alors retiré). Les commentaires d'un plan qui n'existe plus ne sont pas écrits.
pub(crate) fn to_bytes(doc: &Document) -> Result<Option<Vec<u8>>, String> {
    let order: HashMap<u64, usize> = doc.panels.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
    let mut list: Vec<Comment> = doc.comments.iter().filter(|c| order.contains_key(&c.panel)).cloned().collect();
    if list.is_empty() {
        return Ok(None);
    }
    // Dans l'ordre du film : le fichier se lit, et deux enregistrements se ressemblent.
    list.sort_by_key(|c| (order[&c.panel], c.frame, c.id));
    let sidecar = Sidecar { format: FORMAT.into(), version: VERSION, commentaires: list };
    serde_json::to_vec_pretty(&sidecar).map(Some).map_err(|e| e.to_string())
}

/// Les commentaires lus dans le texte de `commentaires.json`. Identifiants uniques et non nuls
/// (un doublon reçoit un identifiant neuf), textes bornés.
pub(crate) fn parse(text: &str) -> Result<Vec<Comment>, String> {
    let sidecar: Sidecar = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("illisible ({e})"))?;
    if sidecar.format != FORMAT {
        return Err("ce n'est pas un fichier de commentaires".into());
    }
    if sidecar.version > VERSION {
        return Err(format!("écrit par une version plus récente (format {})", sidecar.version));
    }
    let mut next = sidecar.commentaires.iter().map(|c| c.id).max().unwrap_or(0) + 1;
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(sidecar.commentaires.len());
    for mut c in sidecar.commentaires {
        if c.id == 0 || !seen.insert(c.id) {
            c.id = next;
            next += 1;
            seen.insert(c.id);
        }
        if c.text.chars().count() > TEXT_MAX {
            c.text = c.text.chars().take(TEXT_MAX).collect();
        }
        // (Une clé de note trop longue n'est pas une clé que SHOT a écrite.)
        c.note = c.note.filter(|n| !n.is_empty() && n.len() <= 120);
        out.push(c);
    }
    Ok(out)
}

/// Le nom du commentateur : celui des préférences s'il y en a un, sinon l'utilisateur Windows.
pub(crate) fn author_name(preferred: &str) -> String {
    let own = preferred.trim();
    if !own.is_empty() {
        return own.to_string();
    }
    std::env::var("USERNAME").ok().map(|u| u.trim().to_string()).filter(|u| !u.is_empty()).unwrap_or_else(|| "Anonyme".into())
}

/// Les premières lignes d'un texte, pour une bulle ou une liste : au plus `lines` lignes non vides
/// de `width` caractères, « … » si on a coupé.
pub(crate) fn preview(text: &str, lines: usize, width: usize) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut cut = false;
    for (n, line) in text.lines().map(str::trim).filter(|l| !l.is_empty()).enumerate() {
        if n >= lines {
            cut = true;
            break;
        }
        if line.chars().count() > width {
            out.push(line.chars().take(width).collect::<String>() + "…");
        } else {
            out.push(line.to_string());
        }
    }
    let mut s = out.join("\n");
    if cut {
        s.push_str("\n…");
    }
    s
}

/// La première ligne non vide d'un texte, coupée à `width` caractères ; « … » si le texte continue.
pub(crate) fn first_line(text: &str, width: usize) -> String {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    let Some(first) = lines.next() else { return String::new() };
    let more = lines.next().is_some();
    let mut s = if first.chars().count() > width { first.chars().take(width).collect::<String>() + "…" } else { first.to_string() };
    if more && !s.ends_with('…') {
        s.push_str(" …");
    }
    s
}

/// Un repère de la règle : le commentaire et où il se montre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Placed {
    pub id: u64,
    /// Image du film (montage) ou du plan (timeline).
    pub pos: usize,
    pub color: [u8; 3],
    pub done: bool,
}

/// Ce que montre une règle : tout le film, ou les commentaires d'un plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scope {
    Film,
    Plan(u64),
}

/// Les repères sous l'abscisse `x`, du plus proche au plus loin ; `markers` : (commentaire,
/// abscisse du mât) dans l'ordre où ils sont dessinés (le dernier est dessus). À distance égale,
/// celui qui est dessus passe d'abord. Le repère est un fanion à droite de son mât.
pub(crate) fn marker_hits(markers: &[(u64, f32)], x: f32) -> Vec<u64> {
    let mut hits: Vec<(f32, usize, u64)> = markers
        .iter()
        .enumerate()
        .filter(|(_, (_, pole))| x >= pole - FLAG_LEFT && x <= pole + FLAG_RIGHT)
        .map(|(i, (id, pole))| ((x - (pole + 3.0)).abs(), i, *id))
        .collect();
    hits.sort_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)));
    hits.into_iter().map(|h| h.2).collect()
}

/// Le commentaire que Suppr supprime : le choisi, seulement s'il existe et si le dernier clic l'a
/// visé (`focus`). Sinon rien : Suppr sans sélection ne fait rien.
pub(crate) fn delete_target(focus: bool, selected: Option<u64>, doc: &Document) -> Option<u64> {
    selected.filter(|&id| focus && doc.comment(id).is_some())
}

/// Le commentaire d'après (`forward`) ou d'avant la position `at` dans `list` (position, id),
/// triée.
pub(crate) fn step_comment(list: &[(usize, u64)], at: usize, forward: bool) -> Option<u64> {
    if forward {
        list.iter().find(|(p, _)| *p > at).map(|(_, id)| *id)
    } else {
        list.iter().rev().find(|(p, _)| *p < at).map(|(_, id)| *id)
    }
}

impl Document {
    pub(crate) fn comment(&self, id: u64) -> Option<&Comment> {
        self.comments.iter().find(|c| c.id == id)
    }

    pub(crate) fn comment_mut(&mut self, id: u64) -> Option<&mut Comment> {
        self.comments.iter_mut().find(|c| c.id == id)
    }

    /// Début et durée de chaque plan, par id.
    fn plan_spans(&self) -> HashMap<u64, (usize, usize)> {
        let mut start = 0;
        self.panels
            .iter()
            .map(|p| {
                let len = p.frame_count();
                let span = (p.id, (start, len));
                start += len;
                span
            })
            .collect()
    }

    /// Image du film où se montre le commentaire (rien si son plan n'existe plus).
    pub(crate) fn comment_global(&self, c: &Comment) -> Option<usize> {
        let (start, len) = *self.plan_spans().get(&c.panel)?;
        Some(start + c.frame.min(len.saturating_sub(1)))
    }

    /// Rang du plan du commentaire et image où il se montre dans ce plan.
    pub(crate) fn comment_spot(&self, c: &Comment) -> Option<(usize, usize)> {
        let i = self.panels.iter().position(|p| p.id == c.panel)?;
        Some((i, c.frame.min(self.panels[i].frame_count().saturating_sub(1))))
    }

    /// Les commentaires dans l'ordre du film : (image du film, id).
    pub(crate) fn comments_in_order(&self, show_done: bool) -> Vec<(usize, u64)> {
        if self.comments.is_empty() {
            return Vec::new();
        }
        let spans = self.plan_spans();
        let mut list: Vec<(usize, u64)> = self
            .comments
            .iter()
            .filter(|c| show_done || !c.done)
            .filter_map(|c| spans.get(&c.panel).map(|&(start, len)| (start + c.frame.min(len.saturating_sub(1)), c.id)))
            .collect();
        list.sort_unstable();
        list
    }

    /// Les repères d'une règle, dans l'ordre où les dessiner : par image, puis le commentaire
    /// choisi en dernier (dessus). `show_done` : montrer aussi les résolus.
    pub(crate) fn comment_positions(&self, scope: Scope, show_done: bool, selected: Option<u64>) -> Vec<Placed> {
        // Sans commentaire (le cas courant), rien à calculer à chaque image.
        if self.comments.is_empty() {
            return Vec::new();
        }
        let spans = self.plan_spans();
        let mut list: Vec<Placed> = self
            .comments
            .iter()
            .filter(|c| show_done || !c.done)
            .filter_map(|c| {
                let &(start, len) = spans.get(&c.panel)?;
                let local = c.frame.min(len.saturating_sub(1));
                let pos = match scope {
                    Scope::Film => start + local,
                    Scope::Plan(id) if id == c.panel => local,
                    Scope::Plan(_) => return None,
                };
                Some(Placed { id: c.id, pos, color: c.color, done: c.done })
            })
            .collect();
        list.sort_by_key(|p| (Some(p.id) == selected, p.pos, p.id));
        list
    }

    /// Nouveau commentaire à l'image `frame` du plan `panel`.
    pub(crate) fn add_comment(&mut self, panel: u64, frame: usize, author: &str, created: u64) -> u64 {
        let id = self.alloc();
        self.comments.push(Comment { id, panel, frame, text: String::new(), author: author.to_string(), color: DEFAULT_COLOR, created, done: false, note: None, annotation: None });
        id
    }

    /// Amène le commentaire à l'image `global` du film (il change de plan si l'image est dans un
    /// autre). Vrai si quelque chose a bougé.
    pub(crate) fn move_comment_global(&mut self, id: u64, global: usize) -> bool {
        let (i, frame) = self.locate(global);
        let panel = self.panels[i].id;
        self.set_comment_spot(id, panel, frame)
    }

    /// Amène le commentaire à l'image `frame` de son plan (dernière image au plus loin).
    pub(crate) fn move_comment_local(&mut self, id: u64, frame: usize) -> bool {
        let Some(panel) = self.comment(id).map(|c| c.panel) else { return false };
        let Some(len) = self.panels.iter().find(|p| p.id == panel).map(|p| p.frame_count()) else { return false };
        self.set_comment_spot(id, panel, frame.min(len.saturating_sub(1)))
    }

    fn set_comment_spot(&mut self, id: u64, panel: u64, frame: usize) -> bool {
        let Some(c) = self.comment_mut(id) else { return false };
        let changed = c.panel != panel || c.frame != frame;
        (c.panel, c.frame) = (panel, frame);
        changed
    }

    pub(crate) fn remove_comment(&mut self, id: u64) -> bool {
        let before = self.comments.len();
        self.comments.retain(|c| c.id != id);
        self.comments.len() != before
    }

    /// Retire les commentaires dont le plan n'existe plus.
    pub(crate) fn prune_comments(&mut self) {
        if self.comments.is_empty() {
            return;
        }
        let ids: HashSet<u64> = self.panels.iter().map(|p| p.id).collect();
        self.comments.retain(|c| ids.contains(&c.panel));
    }

    /// Les commentaires du plan `from` (de `len` images) passent au plan `to`, `offset` images plus
    /// loin : le plan `from` est réuni au bout de `to`.
    pub(crate) fn rehome_comments(&mut self, from: u64, to: u64, from_len: usize, offset: usize) {
        for c in self.comments.iter_mut().filter(|c| c.panel == from) {
            c.panel = to;
            c.frame = c.frame.min(from_len.saturating_sub(1)) + offset;
        }
    }

    /// Le plan `first` est coupé à l'image `at` et le plan `second` reçoit la suite : les
    /// commentaires d'après la coupe le suivent.
    pub(crate) fn split_comments(&mut self, first: u64, second: u64, at: usize) {
        for c in self.comments.iter_mut().filter(|c| c.panel == first && c.frame >= at) {
            c.panel = second;
            c.frame -= at;
        }
    }
}

/// L'état de l'interface des commentaires (`commentairesui.rs`).
pub(crate) struct CommentUi {
    /// Le commentaire choisi : celui de l'éditeur du panneau, éclairé dans les règles.
    pub sel: Option<u64>,
    /// Le dernier clic a visé un commentaire : Suppr le supprime (sinon Suppr ne le touche pas).
    pub focus: bool,
    /// Repère pris à la souris.
    pub drag: Option<CommentDrag>,
    /// Montrer aussi les commentaires résolus (liste et règles).
    pub show_done: bool,
    /// Le champ de texte de l'éditeur prend la main à sa prochaine image (commentaire neuf,
    /// « Modifier »).
    pub focus_text: bool,
    /// Faire défiler la liste jusqu'à ce commentaire.
    pub scroll_to: Option<u64>,
    /// Hauteur de l'éditeur à la dernière image : la liste prend la place qui reste au-dessus.
    pub editor_h: f32,
}

impl Default for CommentUi {
    fn default() -> Self {
        CommentUi { sel: None, focus: false, drag: None, show_done: true, focus_text: false, scroll_to: None, editor_h: 0.0 }
    }
}

/// Un repère pris à la souris : l'étape d'annulation de tout le geste (prise avant que rien ne
/// bouge), les commentaires d'avant (pour Échap et pour savoir si quelque chose a bougé), où la
/// souris tenait le repère par rapport à son mât, et le point du clic.
pub(crate) struct CommentDrag {
    pub id: u64,
    pub snap: Edit,
    pub before: Vec<Comment>,
    pub grab_dx: f32,
    pub press: eframe::egui::Pos2,
    pub moved: bool,
}
