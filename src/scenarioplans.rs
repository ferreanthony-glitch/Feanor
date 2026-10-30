//! Importer un scénario : du scénario lu (`scenario`) aux séquences et aux plans de SHOT.
//!
//! **La correspondance** (le modèle de SHOT : séquence > plan, un plan étant une « scène » du
//! document avec un seul panneau, voir `Document::is_plans`) :
//! - chaque **intitulé de scène** (`INT. CUISINE – NUIT`) devient une **séquence** du même nom ;
//!   ce qui précède le premier intitulé va dans la séquence « Ouverture » ;
//! - les **plans** de la séquence viennent de l'action et des répliques, selon le découpage choisi
//!   (`Split`) ;
//! - les textes d'un plan vont dans ses champs de la fiche : **action** (le texte de l'action),
//!   **dialogue** (une ligne par réplique, `MARIE (V.O.) (bas) : Viens ici.`, comme le lit le lecteur
//!   de sons par règles), **notes** (notes `[[ ]]`, transitions, synopsis, sections, et le décor
//!   `Décor : INT. CUISINE – NUIT` sur le premier plan de la séquence) ;
//! - en découpage « un plan par scène », les blocs d'une scène sont des **images de storyboard** du
//!   plan (marques d'image, chacune avec ses textes) ;
//! - la **durée** est estimée du texte (`Rhythm`).
//!
//! Les personnages qui parlent sont écrits dans les notes du projet (Réglages du projet > Notes),
//! sous « Personnages du scénario : » : une ligne `- NOM (n répliques)` par personnage. Un autre
//! import ajoute à cette liste (les comptes s'additionnent). C'est la liste que les voix
//! provisoires liront.

use crate::document::{Document, ImageMark};
use crate::scenario::{self, Character, Kind, Script, OPENING};

/// Comment les scènes sont découpées en plans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Split {
    /// Un plan par scène : le plan a une image de storyboard par paragraphe d'action et par échange.
    Scene,
    /// Un plan par paragraphe d'action et un plan par échange (les répliques qui se suivent).
    Paragraph,
    /// Un plan par paragraphe d'action et un plan par réplique.
    Line,
}

impl Split {
    pub const ALL: [Split; 3] = [Split::Scene, Split::Paragraph, Split::Line];

    pub fn label(self) -> &'static str {
        match self {
            Split::Scene => "Un plan par scène",
            Split::Paragraph => "Un plan par paragraphe",
            Split::Line => "Un plan par réplique",
        }
    }

    pub fn help(self) -> &'static str {
        match self {
            Split::Scene => "Chaque scène est un seul plan ; ses paragraphes et ses répliques en sont les images de storyboard.",
            Split::Paragraph => "Un plan par paragraphe d'action, et un plan par échange (les répliques qui se suivent sans action entre elles).",
            Split::Line => "Un plan par paragraphe d'action, et un plan pour chaque réplique.",
        }
    }
}

/// L'estimation de la durée d'un texte (réglable dans la fenêtre).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rhythm {
    /// Mots dits par seconde dans une réplique.
    pub words_per_second: f32,
    /// Secondes par ligne d'action (une ligne : `CHARS_PER_LINE` signes).
    pub seconds_per_action_line: f32,
    /// Durée d'un plan (ou d'une image de storyboard) au plus court, en secondes.
    pub min_seconds: f32,
    /// Durée au plus long.
    pub max_seconds: f32,
}

/// Signes d'une ligne d'action, pour compter ses lignes.
pub const CHARS_PER_LINE: usize = 60;

impl Default for Rhythm {
    fn default() -> Rhythm {
        Rhythm { words_per_second: 2.5, seconds_per_action_line: 1.0, min_seconds: 1.0, max_seconds: 30.0 }
    }
}

impl Rhythm {
    /// Des valeurs sensées (une case vidée ne donne pas une division par zéro).
    pub fn sane(self) -> Rhythm {
        let d = Rhythm::default();
        let ok = |v: f32, lo: f32, hi: f32, dv: f32| if v.is_finite() { v.clamp(lo, hi) } else { dv };
        let min_seconds = ok(self.min_seconds, 0.1, 60.0, d.min_seconds);
        Rhythm {
            words_per_second: ok(self.words_per_second, 0.2, 20.0, d.words_per_second),
            seconds_per_action_line: ok(self.seconds_per_action_line, 0.1, 30.0, d.seconds_per_action_line),
            min_seconds,
            max_seconds: ok(self.max_seconds, min_seconds, 600.0, d.max_seconds).max(min_seconds),
        }
    }

    /// Une action : `seconds_per_action_line` par ligne (les lignes trop longues en comptent
    /// plusieurs, une par `CHARS_PER_LINE` signes).
    pub fn action_seconds(&self, text: &str) -> f32 {
        let lines: usize = text.split('\n').map(|l| l.trim().chars().count()).filter(|&n| n > 0).map(|n| n.div_ceil(CHARS_PER_LINE)).sum();
        lines as f32 * self.seconds_per_action_line
    }

    /// Des répliques de `words` mots en tout.
    pub fn speech_seconds(&self, words: usize) -> f32 {
        words as f32 / self.words_per_second
    }

    /// La durée gardée : ni sous le minimum, ni au-delà du maximum.
    pub fn clamp(&self, seconds: f32) -> f32 {
        seconds.clamp(self.min_seconds, self.max_seconds.max(self.min_seconds))
    }
}

/// Les choix de l'import.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub split: Split,
    pub rhythm: Rhythm,
    /// Écrire « Décor : INT. CUISINE – NUIT » dans les notes du premier plan de chaque séquence.
    pub scene_in_notes: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options { split: Split::Paragraph, rhythm: Rhythm::default(), scene_in_notes: true }
    }
}

/// Où va le scénario.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dest {
    NewProject,
    End,
    AfterCurrent,
}

impl Dest {
    pub fn label(self) -> &'static str {
        match self {
            Dest::NewProject => "Dans un nouveau projet",
            Dest::End => "À la fin du projet ouvert",
            Dest::AfterCurrent => "Après la séquence du plan courant",
        }
    }
}

/// Une image de storyboard à venir : ses textes et sa durée.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Beat {
    pub action: String,
    pub dialogue: String,
    pub notes: Vec<String>,
    /// Durée estimée, en secondes (déjà entre le minimum et le maximum).
    pub seconds: f32,
    raw: f32,
}

/// Un plan à venir : sa première image de storyboard est le plan lui-même, les suivantes en sont
/// des marques d'image.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanDraft {
    pub beats: Vec<Beat>,
    /// La tranche d'éléments du scénario dont le plan vient (de, à, compris) : le lien du panneau Scénario.
    pub span: (usize, usize),
}

impl PlanDraft {
    pub fn seconds(&self) -> f32 {
        self.beats.iter().map(|b| b.seconds).sum()
    }
}

/// Une séquence à venir.
#[derive(Clone, Debug, PartialEq)]
pub struct SequenceDraft {
    pub name: String,
    pub plans: Vec<PlanDraft>,
}

impl SequenceDraft {
    pub fn seconds(&self) -> f32 {
        self.plans.iter().map(PlanDraft::seconds).sum()
    }
}

/// Tout ce que l'import va créer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outline {
    pub sequences: Vec<SequenceDraft>,
    pub characters: Vec<Character>,
}

impl Outline {
    pub fn plan_count(&self) -> usize {
        self.sequences.iter().map(|s| s.plans.len()).sum()
    }

    pub fn seconds(&self) -> f32 {
        self.sequences.iter().map(SequenceDraft::seconds).sum()
    }
}

// ------------------------------------------------------------------------------------ découpage

enum Block {
    /// `first` : l'indice, dans le scénario, du premier élément du bloc.
    Action { text: String, notes: Vec<String>, first: usize },
    Speech { lines: Vec<String>, words: usize, notes: Vec<String>, first: usize },
}

impl Block {
    fn first(&self) -> usize {
        match self {
            Block::Action { first, .. } | Block::Speech { first, .. } => *first,
        }
    }
}

impl Block {
    fn notes_mut(&mut self) -> &mut Vec<String> {
        match self {
            Block::Action { notes, .. } | Block::Speech { notes, .. } => notes,
        }
    }
}

struct SceneAcc {
    name: String,
    /// L'intitulé, à noter dans le premier plan (rien pour « Ouverture »).
    slug: Option<String>,
    blocks: Vec<Block>,
    /// Notes du premier plan (sections qui précèdent, synopsis, notes avant tout bloc).
    first_notes: Vec<String>,
    /// Premier et dernier éléments du scénario de la scène (l'intitulé compris).
    start: usize,
    end: usize,
}

impl SceneAcc {
    fn new(name: String, slug: Option<String>, carry: &mut Vec<String>, start: usize) -> SceneAcc {
        SceneAcc { name, slug, blocks: Vec::new(), first_notes: std::mem::take(carry), start, end: start }
    }

    /// Une note qui se rapporte à ce qui précède : le dernier bloc, ou le premier plan.
    fn note_last(&mut self, note: String) {
        match self.blocks.last_mut() {
            Some(b) => b.notes_mut().push(note),
            None => self.first_notes.push(note),
        }
    }
}

fn labelled(label: &str, text: &str) -> String {
    format!("{label} : {text}")
}

fn inline_notes(notes: &[String]) -> Vec<String> {
    notes.iter().filter(|n| !n.is_empty()).map(|n| labelled("Note", n)).collect()
}

fn beat_of(block: &Block, r: &Rhythm) -> Beat {
    match block {
        Block::Action { text, notes, .. } => Beat { action: text.clone(), dialogue: String::new(), notes: notes.clone(), seconds: 0.0, raw: r.action_seconds(text) },
        Block::Speech { lines, words, notes, .. } => {
            Beat { action: String::new(), dialogue: lines.join("\n"), notes: notes.clone(), seconds: 0.0, raw: r.speech_seconds(*words) }
        }
    }
}

fn merge_into(a: &mut Beat, b: Beat) {
    if !b.action.is_empty() {
        if !a.action.is_empty() {
            a.action.push('\n');
        }
        a.action.push_str(&b.action);
    }
    if !b.dialogue.is_empty() {
        if !a.dialogue.is_empty() {
            a.dialogue.push('\n');
        }
        a.dialogue.push_str(&b.dialogue);
    }
    a.notes.extend(b.notes);
    a.raw += b.raw;
}

fn finish(beats: &mut [Beat], r: &Rhythm) {
    for b in beats {
        b.seconds = r.clamp(b.raw);
    }
}

fn plans_of(scene: &SceneAcc, opt: &Options) -> Vec<PlanDraft> {
    let r = &opt.rhythm;
    let mut plans: Vec<PlanDraft> = Vec::new();
    // Où chaque plan commence dans le scénario (le premier, au début de la scène).
    let mut starts: Vec<usize> = Vec::new();
    match opt.split {
        Split::Scene => {
            let beats: Vec<Beat> = scene.blocks.iter().map(|b| beat_of(b, r)).collect();
            plans.push(PlanDraft { beats, span: (0, 0) });
            starts.push(scene.start);
        }
        Split::Paragraph | Split::Line => {
            let mut last_was_speech = false;
            for b in &scene.blocks {
                let beat = beat_of(b, r);
                let speech = matches!(b, Block::Speech { .. });
                match plans.last_mut() {
                    Some(p) if speech && last_was_speech && opt.split == Split::Paragraph => merge_into(&mut p.beats[0], beat),
                    _ => {
                        plans.push(PlanDraft { beats: vec![beat], span: (0, 0) });
                        starts.push(if starts.is_empty() { scene.start } else { b.first() });
                    }
                }
                last_was_speech = speech;
            }
        }
    }
    if plans.iter().all(|p| p.beats.is_empty()) {
        plans = vec![PlanDraft { beats: vec![Beat::default()], span: (0, 0) }];
        starts = vec![scene.start];
    }
    for k in 0..plans.len() {
        let de = starts[k];
        let a = if k + 1 < starts.len() { starts[k + 1].saturating_sub(1).max(de) } else { scene.end.max(de) };
        plans[k].span = (de, a);
    }
    // Les notes de tête (décor, sections, synopsis) vont au premier plan, avant les siennes.
    let mut head: Vec<String> = Vec::new();
    if opt.scene_in_notes {
        if let Some(slug) = &scene.slug {
            head.push(labelled("Décor", slug));
        }
    }
    head.extend(scene.first_notes.iter().cloned());
    if let Some(first) = plans.first_mut().and_then(|p| p.beats.first_mut()) {
        head.append(&mut first.notes);
        first.notes = head;
    }
    for p in &mut plans {
        finish(&mut p.beats, r);
    }
    plans
}

/// Les séquences et les plans qu'un scénario donne avec ces choix.
pub fn build(script: &Script, opt: &Options) -> Outline {
    let opt = Options { rhythm: opt.rhythm.sane(), ..opt.clone() };
    let mut scenes: Vec<SceneAcc> = Vec::new();
    let mut carry: Vec<String> = Vec::new();
    for (ei, e) in script.elements.iter().enumerate() {
        if let Kind::Heading(h) = &e.kind {
            let mut s = SceneAcc::new(h.clone(), Some(h.clone()), &mut carry, ei);
            s.first_notes.extend(inline_notes(&e.notes));
            scenes.push(s);
            continue;
        }
        if let Kind::Section(t) = &e.kind {
            carry.push(labelled("Section", t));
            carry.extend(inline_notes(&e.notes));
            continue;
        }
        if scenes.is_empty() {
            // Avant le premier intitulé, un synopsis ou une note décrit la scène qui vient ; seuls
            // de l'action, une réplique ou une transition font une séquence « Ouverture ».
            match &e.kind {
                Kind::Synopsis(t) => {
                    carry.push(labelled("Synopsis", t));
                    carry.extend(inline_notes(&e.notes));
                    continue;
                }
                Kind::Note(t) => {
                    carry.push(labelled("Note", t));
                    continue;
                }
                _ => scenes.push(SceneAcc::new(OPENING.to_string(), None, &mut carry, ei)),
            }
        }
        let Some(scene) = scenes.last_mut() else { continue };
        match &e.kind {
            Kind::Action(text) => scene.blocks.push(Block::Action { text: text.clone(), notes: inline_notes(&e.notes), first: ei }),
            Kind::Dialogue(sp) => {
                let line = sp.line();
                let words = sp.words();
                match scene.blocks.last_mut() {
                    // Deux personnages qui parlent ensemble : les deux lignes d'un même bloc.
                    Some(Block::Speech { lines, words: w, notes, .. }) if sp.dual => {
                        lines.push(line);
                        *w += words;
                        notes.extend(inline_notes(&e.notes));
                    }
                    _ => scene.blocks.push(Block::Speech { lines: vec![line], words, notes: inline_notes(&e.notes), first: ei }),
                }
            }
            Kind::Transition(t) => {
                scene.note_last(labelled("Transition", t));
                for n in inline_notes(&e.notes) {
                    scene.note_last(n);
                }
            }
            Kind::Synopsis(t) => {
                scene.note_last(labelled("Synopsis", t));
                for n in inline_notes(&e.notes) {
                    scene.note_last(n);
                }
            }
            Kind::Note(t) => scene.note_last(labelled("Note", t)),
            Kind::Heading(_) | Kind::Section(_) => {}
        }
    }
    // Des sections restées sans scène après elles : notes du dernier plan.
    if !carry.is_empty() {
        if let Some(s) = scenes.last_mut() {
            for n in carry.drain(..) {
                s.note_last(n);
            }
        }
    }
    // Chaque scène court jusqu'à l'intitulé de la suivante ; la dernière, jusqu'à la fin du scénario.
    let last = script.elements.len().saturating_sub(1);
    let starts: Vec<usize> = scenes.iter().map(|s| s.start).collect();
    for (k, s) in scenes.iter_mut().enumerate() {
        s.end = starts.get(k + 1).map_or(last, |n| n.saturating_sub(1)).max(s.start);
    }
    let sequences = scenes.iter().map(|s| SequenceDraft { name: s.name.clone(), plans: plans_of(s, &opt) }).collect();
    Outline { sequences, characters: scenario::characters(&script.elements) }
}

// ------------------------------------------------------------------------------------ dans le projet

/// Ce qui a été créé.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applied {
    /// Indice du premier plan créé.
    pub first: usize,
    pub plans: usize,
    pub sequences: usize,
    /// Un lien par plan créé : son id et la tranche du scénario dont il vient (`scenariogarde`).
    pub liens: Vec<crate::scenariogarde::Lien>,
}

/// Images d'une durée en secondes (une au moins).
fn frames_of(seconds: f32, fps: u32) -> usize {
    ((seconds * fps.max(1) as f32).round() as usize).max(1)
}

/// Vrai si le projet est celui qu'un « Nouveau projet » vient de créer : un seul plan, sans texte
/// ni dessin. L'import le remplace alors au lieu de le laisser en tête.
pub fn is_blank(doc: &Document) -> bool {
    let [p] = doc.panels.as_slice() else { return false };
    p.action.trim().is_empty()
        && p.dialogue.trim().is_empty()
        && p.notes.trim().is_empty()
        && p.marks.is_empty()
        && p.camera.is_empty()
        && p.layers.iter().all(|l| l.frames.iter().all(Option::is_none))
        && doc.audio_tracks.iter().all(|t| t.clips.is_empty())
}

/// Où insérer dans le projet ouvert : `current` est l'indice du plan courant. Une séquence ne se
/// coupe pas : « après le plan courant » est donc à la fin de sa séquence.
pub fn insert_point(doc: &Document, dest: Dest, current: usize) -> usize {
    match dest {
        Dest::AfterCurrent if !doc.panels.is_empty() => doc.sequence_run(current.min(doc.panels.len() - 1)).1,
        _ => doc.panels.len(),
    }
}

/// Crée les séquences et les plans de `outline` dans `doc`, à l'indice de plan `at`. Renvoie rien
/// s'il n'y a aucun plan à créer.
pub fn insert(doc: &mut Document, outline: &Outline, at: usize) -> Option<Applied> {
    if outline.plan_count() == 0 {
        return None;
    }
    let fps = doc.fps.max(1);
    let mut panels = Vec::with_capacity(outline.plan_count());
    let mut liens = Vec::with_capacity(outline.plan_count());
    for seq in outline.sequences.iter().filter(|s| !s.plans.is_empty()) {
        let sid = doc.new_sequence();
        if let Some(s) = doc.sequence_mut(sid) {
            s.name = seq.name.clone();
        }
        for plan in &seq.plans {
            let scene = doc.new_scene(sid);
            let mut panel = doc.make_panel(scene);
            let mut cursor = 0;
            for (k, beat) in plan.beats.iter().enumerate() {
                let notes = beat.notes.join("\n");
                if k == 0 {
                    panel.action = beat.action.clone();
                    panel.dialogue = beat.dialogue.clone();
                    panel.notes = notes;
                } else {
                    let id = doc.alloc();
                    panel.marks.push(ImageMark { id, frame: cursor, color: 0, action: beat.action.clone(), dialogue: beat.dialogue.clone(), notes });
                }
                cursor += frames_of(beat.seconds, fps);
            }
            panel.length = cursor.max(1);
            liens.push(crate::scenariogarde::Lien { plan: panel.id, de: plan.span.0, a: plan.span.1 });
            panels.push(panel);
        }
    }
    let first_id = panels.first()?.id;
    let count = panels.len();
    let at = at.min(doc.panels.len());
    doc.panels.splice(at..at, panels);
    doc.normalize();
    let first = doc.panels.iter().position(|p| p.id == first_id).unwrap_or(at);
    Some(Applied { first, plans: count, sequences: outline.sequences.iter().filter(|s| !s.plans.is_empty()).count(), liens })
}

/// Un projet qui a déjà le format voulu (créé par « Nouveau projet ») reçoit le scénario : son plan
/// vide est remplacé, le titre et l'auteur du scénario deviennent ceux du projet.
#[cfg(test)]
pub fn fill_new_project(doc: &mut Document, outline: &Outline, title: &str, author: &str) -> Option<Applied> {
    doc.sequences.clear();
    doc.scenes.clear();
    doc.panels.clear();
    if !title.trim().is_empty() {
        doc.properties.title = title.trim().to_string();
    }
    if !author.trim().is_empty() {
        doc.properties.author = author.trim().to_string();
    }
    let applied = insert(doc, outline, 0);
    if applied.is_none() {
        // Rien à créer : un projet ne reste pas sans plan.
        let seq = doc.new_sequence();
        let scene = doc.new_scene(seq);
        let panel = doc.make_panel(scene);
        doc.panels.push(panel);
    }
    note_characters(doc, &outline.characters);
    applied
}

/// Ajoute le scénario au projet ouvert, au point de `dest` (jamais `NewProject`). Un projet
/// encore vierge est remplacé.
pub fn append_to_project(doc: &mut Document, outline: &Outline, dest: Dest, current: usize) -> Option<Applied> {
    if outline.plan_count() == 0 {
        return None;
    }
    let at = if is_blank(doc) {
        doc.sequences.clear();
        doc.scenes.clear();
        doc.panels.clear();
        0
    } else {
        insert_point(doc, dest, current)
    };
    let applied = insert(doc, outline, at);
    note_characters(doc, &outline.characters);
    applied
}

// ------------------------------------------------------------------------------------ personnages

/// L'intitulé du bloc de personnages dans les notes du projet.
pub const CHARACTERS_HEADER: &str = "Personnages du scénario :";

fn character_line(name: &str, lines: usize) -> String {
    format!("- {name} ({lines} {})", if lines > 1 { "répliques" } else { "réplique" })
}

/// Les personnages notés dans `notes` (nom, répliques), dans l'ordre du bloc.
pub fn read_characters(notes: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in notes.lines() {
        let t = line.trim();
        if t == CHARACTERS_HEADER {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        let Some(item) = t.strip_prefix("- ") else { break };
        let (name, count) = match item.rfind(" (") {
            Some(i) => (item[..i].trim(), item[i + 2..].split_whitespace().next().and_then(|n| n.parse::<usize>().ok()).unwrap_or(0)),
            None => (item.trim(), 0),
        };
        if !name.is_empty() {
            out.push((name.to_string(), count));
        }
    }
    out
}

/// Les notes du projet avec le bloc de personnages remplacé (ou ajouté à la fin) par `chars`.
pub fn write_characters(notes: &str, chars: &[(String, usize)]) -> String {
    let mut sorted = chars.to_vec();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut block = vec![CHARACTERS_HEADER.to_string()];
    block.extend(sorted.iter().map(|(n, c)| character_line(n, *c)));
    let lines: Vec<&str> = notes.lines().collect();
    match lines.iter().position(|l| l.trim() == CHARACTERS_HEADER) {
        Some(start) => {
            let mut end = start + 1;
            while end < lines.len() && lines[end].trim().starts_with("- ") {
                end += 1;
            }
            let mut out: Vec<String> = lines[..start].iter().map(|s| s.to_string()).collect();
            out.extend(block);
            out.extend(lines[end..].iter().map(|s| s.to_string()));
            out.join("\n")
        }
        None => {
            let base = notes.trim_end();
            if base.is_empty() {
                block.join("\n")
            } else {
                format!("{base}\n\n{}", block.join("\n"))
            }
        }
    }
}

/// Note les personnages trouvés dans les notes du projet ; ceux déjà notés gardent leur place et
/// leur compte s'additionne.
pub fn note_characters(doc: &mut Document, found: &[Character]) {
    if found.is_empty() {
        return;
    }
    let mut all = read_characters(&doc.properties.notes);
    for c in found {
        match all.iter_mut().find(|(n, _)| n.eq_ignore_ascii_case(&c.name)) {
            Some(entry) => entry.1 += c.lines,
            None => all.push((c.name.clone(), c.lines)),
        }
    }
    doc.properties.notes = write_characters(&doc.properties.notes, &all);
}
