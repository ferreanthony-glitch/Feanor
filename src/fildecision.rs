//! Fil de décision par plan (audit 2026-10-30, idée ★1) : le client ou le réalisateur renvoie beaucoup de notes,
//! parfois contradictoires ; il faut savoir laquelle compte et ce qui a été traité.
//!
//! Une note est un commentaire de la timeline (`commentaires.rs`) à qui on ajoute un **statut** (à faire, fait,
//! refusé avec sa raison, en discussion), son origine (SHOT, relecture, plus tard Kitsu), l'envoi et la version
//! du plan qu'elle concerne, des réponses, des conflits avec d'autres notes et un éventuel « retake ».
//!
//! # Format (aucun changement de `project::VERSION`, ni de la version de `commentaires.json`)
//!
//! Tout est **additif** et se lit à l'identique pour un projet d'avant :
//! - `resolu` (d'avant) reste le fait « Fait » : une note résolue devient « fait », les autres « à faire » ;
//! - un objet `suivi` (absent s'il est vide) porte le reste : `statut` (seulement « refuse » ou « discussion »),
//!   `raison`, `statut_le`, `source` (« relecture », « kitsu » ; absent : SHOT), `externe` (l'identifiant de la note
//!   dans l'outil d'où elle vient), `envoi` (numéro de l'envoi), `version` (numéro de version du plan), `reponses`,
//!   `conflits` (numéros de notes), `retake` (le motif).
//!
//! Un SHOT d'avant lit le même fichier sans broncher : il ignore `suivi`, une note refusée ou en discussion y
//! apparaît « pas résolue ». Il n'y a donc **qu'un seul endroit** où vit « fait » (`done`) : `statut()` en déduit
//! le statut, `appliquer_statut` garde les deux d'accord.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::commentaires::{Comment, TEXT_MAX};
use crate::document::Document;

/// Une raison de refus, un motif de retake plus longs que ça sont coupés à la lecture.
pub(crate) const REASON_MAX: usize = 2_000;
/// Réponses gardées par note, au plus (un fichier abîmé ne gèle pas l'interface).
pub(crate) const REPLIES_MAX: usize = 500;
/// Notes liées à une note par « en conflit avec… », au plus.
pub(crate) const CONFLICTS_MAX: usize = 50;

/// Où en est une note.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Status {
    #[default]
    #[serde(rename = "a_faire")]
    Todo,
    #[serde(rename = "fait")]
    Done,
    #[serde(rename = "refuse")]
    Refused,
    #[serde(rename = "discussion")]
    Discussion,
}

impl Status {
    /// Dans l'ordre où l'écran les montre (compteurs, filtres, menus).
    pub const ALL: [Status; 4] = [Status::Todo, Status::Done, Status::Refused, Status::Discussion];

    pub fn label(self) -> &'static str {
        match self {
            Status::Todo => "À faire",
            Status::Done => "Fait",
            Status::Refused => "Refusé",
            Status::Discussion => "En discussion",
        }
    }

    pub fn index(self) -> usize {
        Status::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    /// Reste à traiter : à faire ou en discussion.
    pub fn is_open(self) -> bool {
        matches!(self, Status::Todo | Status::Discussion)
    }
}

/// D'où vient une note.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    #[default]
    #[serde(rename = "shot")]
    Shot,
    #[serde(rename = "relecture")]
    Relecture,
    /// Les notes d'un logiciel de suivi de production (le pont viendra : voir `docs/INTERFACE.md` §88).
    #[serde(rename = "kitsu")]
    Kitsu,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Shot => "FEANOR",
            Source::Relecture => "Relecture",
            Source::Kitsu => "Kitsu",
        }
    }
}

/// Une réponse dans le fil d'une note.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Reply {
    #[serde(rename = "auteur", default)]
    pub author: String,
    #[serde(rename = "texte", default)]
    pub text: String,
    /// Secondes depuis 1970 (0 : inconnue).
    #[serde(rename = "cree", default)]
    pub created: u64,
    #[serde(rename = "source", default, skip_serializing_if = "is_default_source")]
    pub source: Source,
    /// L'identifiant de la réponse dans l'outil d'où elle vient (Kitsu).
    #[serde(rename = "externe", default, skip_serializing_if = "Option::is_none")]
    pub external: Option<String>,
}

fn is_default_source(s: &Source) -> bool {
    *s == Source::Shot
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

fn is_zero64(n: &u64) -> bool {
    *n == 0
}

/// Tout ce que le fil ajoute à un commentaire (voir le format plus haut).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Follow {
    /// Seulement « refusé » ou « en discussion » : « fait » est `Comment::done`, « à faire » est l'absence de tout.
    #[serde(rename = "statut", default, skip_serializing_if = "Option::is_none")]
    pub state: Option<Status>,
    /// Pourquoi la note est refusée (obligatoire pour refuser).
    #[serde(rename = "raison", default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
    /// Quand le statut a changé pour la dernière fois (secondes depuis 1970).
    #[serde(rename = "statut_le", default, skip_serializing_if = "is_zero64")]
    pub status_at: u64,
    #[serde(rename = "source", default, skip_serializing_if = "is_default_source")]
    pub source: Source,
    /// L'identifiant de la note dans l'outil d'où elle vient (Kitsu).
    #[serde(rename = "externe", default, skip_serializing_if = "Option::is_none")]
    pub external: Option<String>,
    /// L'envoi (`envois.rs`) que la note concerne.
    #[serde(rename = "envoi", default, skip_serializing_if = "Option::is_none")]
    pub send: Option<u64>,
    /// Le numéro de version du plan (V1, V2…) quand la note a été écrite ; 0 : inconnu.
    #[serde(rename = "version", default, skip_serializing_if = "is_zero")]
    pub plan_version: u32,
    #[serde(rename = "reponses", default, skip_serializing_if = "Vec::is_empty")]
    pub replies: Vec<Reply>,
    /// Les notes avec lesquelles celle-ci est en conflit (posé à la main, des deux côtés).
    #[serde(rename = "conflits", default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<u64>,
    /// La note demande de refaire le plan : le motif (peut être vide).
    #[serde(rename = "retake", default, skip_serializing_if = "Option::is_none")]
    pub retake: Option<String>,
}

impl Follow {
    pub(crate) fn is_empty(&self) -> bool {
        *self == Follow::default()
    }
}

impl Comment {
    /// Le statut : « fait » si le commentaire est résolu, sinon celui du fil, sinon « à faire ».
    pub fn status(&self) -> Status {
        if self.done {
            Status::Done
        } else {
            self.follow.state.unwrap_or(Status::Todo)
        }
    }

    /// Reste à traiter (à faire ou en discussion).
    pub fn is_open(&self) -> bool {
        self.status().is_open()
    }

    /// L'origine : celle posée, ou la relecture pour une note importée d'avant le fil.
    pub fn source(&self) -> Source {
        match self.follow.source {
            Source::Shot if self.note.is_some() => Source::Relecture,
            s => s,
        }
    }

    /// Change le statut en gardant `done` et le fil d'accord. La raison ne se garde que pour « refusé ».
    pub(crate) fn appliquer_statut(&mut self, status: Status, reason: &str, now: u64) {
        self.done = status == Status::Done;
        self.follow.state = matches!(status, Status::Refused | Status::Discussion).then_some(status);
        self.follow.reason = if status == Status::Refused { clean(reason, REASON_MAX) } else { String::new() };
        self.follow.status_at = now;
    }

    /// Remet d'accord ce qu'un fichier a écrit (texte borné, rien d'incohérent).
    pub(crate) fn normalize_follow(&mut self) {
        let f = &mut self.follow;
        // « fait » gagne : une note résolue n'est ni refusée ni en discussion.
        f.state = match f.state {
            Some(Status::Refused) | Some(Status::Discussion) if !self.done => f.state,
            Some(Status::Done) => {
                self.done = true;
                None
            }
            _ => None,
        };
        f.reason = if f.state == Some(Status::Refused) { clean(&f.reason, REASON_MAX) } else { String::new() };
        f.external = f.external.take().filter(|e| !e.is_empty() && e.len() <= 120);
        f.replies.truncate(REPLIES_MAX);
        for r in &mut f.replies {
            r.text = clean(&r.text, TEXT_MAX);
            r.external = r.external.take().filter(|e| !e.is_empty() && e.len() <= 120);
        }
        f.retake = f.retake.take().map(|m| clean(&m, REASON_MAX));
        let own = self.id;
        let mut seen = HashSet::new();
        f.conflicts.retain(|&c| c != own && c != 0 && seen.insert(c));
        f.conflicts.truncate(CONFLICTS_MAX);
    }
}

/// Texte coupé à `max` caractères, sans espaces autour.
fn clean(s: &str, max: usize) -> String {
    s.trim().chars().take(max).collect()
}

/// Les statuts montrés (liste, repères des règles) et l'envoi auquel on se limite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    /// Par `Status::index`.
    pub shown: [bool; 4],
    /// Seulement les notes de cet envoi ; `None` : tous.
    pub send: Option<u64>,
}

impl Filter {
    pub fn all() -> Filter {
        Filter { shown: [true; 4], send: None }
    }

    /// Seulement ce qui reste à traiter.
    pub fn open_only() -> Filter {
        let mut shown = [false; 4];
        shown[Status::Todo.index()] = true;
        shown[Status::Discussion.index()] = true;
        Filter { shown, send: None }
    }

    pub fn matches(&self, c: &Comment) -> bool {
        self.shown[c.status().index()] && self.send.is_none_or(|s| c.follow.send == Some(s))
    }

    pub fn is_all(&self) -> bool {
        *self == Filter::all()
    }
}

impl Default for Filter {
    fn default() -> Self {
        Filter::all()
    }
}

/// Combien de notes dans chaque statut.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts(pub [usize; 4]);

impl Counts {
    pub fn get(&self, s: Status) -> usize {
        self.0[s.index()]
    }

    pub fn total(&self) -> usize {
        self.0.iter().sum()
    }

    /// À faire et en discussion.
    pub fn open(&self) -> usize {
        self.get(Status::Todo) + self.get(Status::Discussion)
    }
}

/// Ce que la vignette d'un plan dit de ses notes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlanMark {
    pub counts: Counts,
    /// Une note à traiter est en conflit avec une autre.
    pub conflict: bool,
    /// Une note à traiter demande un retake.
    pub retake: bool,
}

/// Une ligne de la liste des retakes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetakeRow {
    pub note: u64,
    pub plan_rank: usize,
    pub sequence: String,
    pub number: String,
    pub version: String,
    pub timecode: String,
    pub image: usize,
    pub motive: String,
    pub text: String,
    pub author: String,
    pub source: &'static str,
    pub send: String,
    pub status: Status,
    pub reason: String,
    pub date: String,
}

pub(crate) const RETAKE_COLUMNS: [&str; 14] = [
    "Séquence",
    "N° de plan",
    "Version",
    "TC entrée",
    "Image",
    "Motif du retake",
    "Note",
    "Auteur",
    "Source",
    "Envoi",
    "Statut",
    "Raison du refus",
    "Date de la note",
    "Réponses",
];

impl Document {
    /// Le changement de statut demandé est-il permis, et change-t-il quelque chose ? (Sert à ne pas prendre une étape
    /// d'annulation pour rien.)
    pub(crate) fn note_status_changes(&self, id: u64, status: Status, reason: &str) -> Result<bool, String> {
        let Some(c) = self.comment(id) else { return Err("Cette note n'existe plus.".into()) };
        if status == Status::Refused && reason.trim().is_empty() {
            return Err("Refuser une note demande une raison.".into());
        }
        let same = c.status() == status && (status != Status::Refused || c.follow.reason == clean(reason, REASON_MAX));
        Ok(!same)
    }

    /// Change le statut de la note `id`. `Ok(false)` : déjà ainsi. « Refusé » demande une raison.
    pub(crate) fn set_note_status(&mut self, id: u64, status: Status, reason: &str, now: u64) -> Result<bool, String> {
        if !self.note_status_changes(id, status, reason)? {
            return Ok(false);
        }
        if let Some(c) = self.comment_mut(id) {
            c.appliquer_statut(status, reason, now);
        }
        Ok(true)
    }

    /// Les compteurs, pour tout le film ou pour un envoi.
    pub(crate) fn note_counts(&self, send: Option<u64>) -> Counts {
        let mut n = [0usize; 4];
        for c in self.comments.iter().filter(|c| send.is_none_or(|s| c.follow.send == Some(s))) {
            n[c.status().index()] += 1;
        }
        Counts(n)
    }

    /// Par plan (son id) : ce que sa vignette montre. Seuls les plans qui ont une note y sont.
    pub(crate) fn plan_marks(&self) -> HashMap<u64, PlanMark> {
        let mut out: HashMap<u64, PlanMark> = HashMap::new();
        if self.comments.is_empty() {
            return out;
        }
        let open: HashSet<u64> = self.comments.iter().filter(|c| c.is_open()).map(|c| c.id).collect();
        for c in &self.comments {
            let m = out.entry(c.panel).or_default();
            m.counts.0[c.status().index()] += 1;
            if c.is_open() {
                m.conflict |= c.follow.conflicts.iter().any(|o| open.contains(o));
                m.retake |= c.follow.retake.is_some();
            }
        }
        out
    }

    /// Ce que la vignette du plan `panel` dit de ses notes (un seul plan : un passage sur les commentaires).
    pub(crate) fn plan_mark(&self, panel: u64) -> Option<PlanMark> {
        let mut m = PlanMark::default();
        let mut any = false;
        for c in self.comments.iter().filter(|c| c.panel == panel) {
            any = true;
            m.counts.0[c.status().index()] += 1;
            if c.is_open() {
                m.conflict |= c.follow.conflicts.iter().any(|o| self.comment(*o).is_some_and(Comment::is_open));
                m.retake |= c.follow.retake.is_some();
            }
        }
        any.then_some(m)
    }

    /// Les notes du plan `panel`, dans l'ordre où elles sont venues : par date, puis par numéro. Toutes les
    /// versions du plan, tous les envois.
    pub(crate) fn thread_of(&self, panel: u64) -> Vec<u64> {
        let mut v: Vec<(u64, u64)> = self.comments.iter().filter(|c| c.panel == panel).map(|c| (c.created, c.id)).collect();
        v.sort_unstable();
        v.into_iter().map(|(_, id)| id).collect()
    }

    /// La note à traiter qui suit : celle d'après `selected` si elle est à traiter, sinon la première après l'image
    /// `after` du film ; on revient au début en fin de film. `send` : seulement un envoi.
    pub(crate) fn next_to_treat(&self, after: usize, selected: Option<u64>, send: Option<u64>) -> Option<u64> {
        let filter = Filter { shown: Filter::open_only().shown, send };
        let list = self.comments_in_order(&filter);
        if list.is_empty() {
            return None;
        }
        if let Some(i) = selected.and_then(|s| list.iter().position(|(_, id)| *id == s)) {
            return Some(list[(i + 1) % list.len()].1);
        }
        list.iter().find(|(p, _)| *p > after).or_else(|| list.first()).map(|(_, id)| *id)
    }

    /// Les notes en conflit avec `id` (celles qui existent encore).
    pub(crate) fn conflicts_of(&self, id: u64) -> Vec<u64> {
        self.comment(id).map_or_else(Vec::new, |c| c.follow.conflicts.iter().copied().filter(|o| self.comment(*o).is_some()).collect())
    }

    /// Marque `a` et `b` en conflit, des deux côtés. Faux : rien n'a changé.
    pub(crate) fn link_conflict(&mut self, a: u64, b: u64) -> bool {
        if a == b || self.comment(a).is_none() || self.comment(b).is_none() {
            return false;
        }
        let mut changed = false;
        for (x, y) in [(a, b), (b, a)] {
            if let Some(c) = self.comment_mut(x) {
                if !c.follow.conflicts.contains(&y) && c.follow.conflicts.len() < CONFLICTS_MAX {
                    c.follow.conflicts.push(y);
                    changed = true;
                }
            }
        }
        changed
    }

    pub(crate) fn unlink_conflict(&mut self, a: u64, b: u64) -> bool {
        let mut changed = false;
        for (x, y) in [(a, b), (b, a)] {
            if let Some(c) = self.comment_mut(x) {
                let n = c.follow.conflicts.len();
                c.follow.conflicts.retain(|o| *o != y);
                changed |= c.follow.conflicts.len() != n;
            }
        }
        changed
    }

    /// Les conflits d'une note qui disparaît sont défaits chez les autres.
    pub(crate) fn forget_conflicts_of(&mut self, id: u64) {
        for c in self.comments.iter_mut() {
            c.follow.conflicts.retain(|o| *o != id);
        }
    }

    /// Ajoute une réponse (texte non vide) au fil de la note `id`.
    pub(crate) fn add_reply(&mut self, id: u64, author: &str, text: &str, created: u64) -> bool {
        let text = clean(text, TEXT_MAX);
        if text.is_empty() {
            return false;
        }
        let Some(c) = self.comment_mut(id) else { return false };
        if c.follow.replies.len() >= REPLIES_MAX {
            return false;
        }
        c.follow.replies.push(Reply { author: author.trim().to_string(), text, created, ..Default::default() });
        true
    }

    /// Pose ou retire le « retake » d'une note (`Some(motif)`, le motif peut être vide).
    pub(crate) fn set_retake(&mut self, id: u64, motive: Option<&str>) -> bool {
        let Some(c) = self.comment_mut(id) else { return false };
        let new = motive.map(|m| clean(m, REASON_MAX));
        let changed = c.follow.retake != new;
        c.follow.retake = new;
        changed
    }

    /// L'envoi et la version du plan que la note concerne quand elle est posée : le dernier envoi, la version affichée.
    pub(crate) fn stamp_follow(&self, panel: u64) -> (Option<u64>, u32) {
        (self.envois.dernier().map(|e| e.id), if self.panels.iter().any(|p| p.id == panel) { self.numero_de_version(panel) } else { 0 })
    }

    /// Les notes de retake, dans l'ordre du film (puis de la note).
    pub(crate) fn retake_rows(&self) -> Vec<RetakeRow> {
        let spans = self.plan_spans();
        let mut rows: Vec<(usize, usize, u64, RetakeRow)> = Vec::new();
        for c in self.comments.iter().filter(|c| c.follow.retake.is_some()) {
            let Some(rank) = self.panels.iter().position(|p| p.id == c.panel) else { continue };
            let p = &self.panels[rank];
            let (start, len) = spans.get(&p.id).copied().unwrap_or((0, 0));
            let image = c.frame.min(len.saturating_sub(1));
            let version = if self.nombre_de_versions(p.id) >= 2 || c.follow.plan_version >= 2 { format!("V{}", if c.follow.plan_version > 0 { c.follow.plan_version } else { self.numero_de_version(p.id) }) } else { String::new() };
            rows.push((
                rank,
                c.frame,
                c.id,
                RetakeRow {
                    note: c.id,
                    plan_rank: rank,
                    sequence: self.sequence_label(self.sequence_of(rank)),
                    number: self.scene_label(p.scene),
                    version,
                    timecode: crate::storyboard::timecode(start, self.fps),
                    image: image + 1,
                    motive: c.follow.retake.clone().unwrap_or_default(),
                    text: c.text.trim().to_string(),
                    author: c.author.clone(),
                    source: c.source().label(),
                    send: c.follow.send.and_then(|s| self.envois.trouve(s)).map(|e| e.libelle()).unwrap_or_default(),
                    status: c.status(),
                    reason: c.follow.reason.clone(),
                    date: if c.created == 0 { String::new() } else { crate::envois::date_courte(c.created) },
                },
            ));
        }
        rows.sort_by_key(|(rank, frame, id, _)| (*rank, *frame, *id));
        rows.into_iter().map(|r| r.3).collect()
    }

    /// Le CSV des retakes (Excel en français : point-virgule, BOM, `\r\n`), une ligne par note de retake.
    pub(crate) fn retakes_csv(&self) -> Vec<u8> {
        use crate::ficheformats::{champ, neutralise};
        let sep = ';';
        let mut out = String::from('\u{feff}');
        let line = |cells: Vec<String>| cells.iter().map(|c| champ(&neutralise(c), sep)).collect::<Vec<_>>().join(&sep.to_string());
        out.push_str(&line(RETAKE_COLUMNS.iter().map(|s| s.to_string()).collect()));
        out.push_str("\r\n");
        for r in self.retake_rows() {
            let replies = self.comment(r.note).map_or_else(String::new, |c| c.follow.replies.iter().map(|x| if x.author.is_empty() { x.text.clone() } else { format!("{} : {}", x.author, x.text) }).collect::<Vec<_>>().join("\n"));
            out.push_str(&line(vec![
                r.sequence,
                r.number,
                r.version,
                r.timecode,
                r.image.to_string(),
                r.motive,
                r.text,
                r.author,
                r.source.to_string(),
                r.send,
                r.status.label().to_string(),
                r.reason,
                r.date,
                replies,
            ]));
            out.push_str("\r\n");
        }
        out.into_bytes()
    }
}
