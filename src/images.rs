//! Images de la timeline (13c-2) : images choisies (Maj + clic / glisser dans la grille, sur
//! les calques choisis), supprimer avec un mode de trou, insérer, couper / copier / coller en
//! insérant ou en remplaçant, dupliquer, inverser, tout choisir. Les commandes « du plan »
//! (I, Maj+I, Maj+F5 : tes touches TVPaint) touchent tous les calques, la durée du plan, ses
//! clés de caméra et ses marques.
//!
//! Choix d'Anthony (2026-10-15) pour le trou laissé par des images supprimées : laisser vide,
//! combler depuis la gauche (le dessin d'avant est tenu), combler depuis la droite (la suite
//! recule).

use std::collections::{BTreeMap, HashMap};

use eframe::egui;

use crate::app::{App, Mode};
use crate::document::{Cel, CelId, Panel};

/// Ce que devient la place d'images supprimées.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Hole {
    /// La suite du calque recule (comme « Supprimer le dessin » jusqu'ici).
    #[default]
    FromRight,
    /// Les images restent, vides.
    Empty,
    /// Le dessin d'avant le trou y est tenu.
    FromLeft,
}

impl Hole {
    pub(crate) const ALL: [Hole; 3] = [Hole::Empty, Hole::FromLeft, Hole::FromRight];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Hole::Empty => "Laisser vide",
            Hole::FromLeft => "Combler depuis la gauche (le dessin d'avant est tenu)",
            Hole::FromRight => "Combler depuis la droite (la suite recule)",
        }
    }
}

type Frames = Vec<Option<CelId>>;

fn trim(frames: &mut Frames) {
    while frames.last() == Some(&None) {
        frames.pop();
    }
}

/// Les images [a, b) d'un calque (vides au-delà de sa fin).
pub(crate) fn frames_in(frames: &Frames, a: usize, b: usize) -> Frames {
    (a..b).map(|f| frames.get(f).copied().flatten()).collect()
}

/// Supprime les images [a, b) ; le trou selon `hole`.
pub(crate) fn delete_frames(frames: &mut Frames, a: usize, b: usize, hole: Hole) {
    let b = b.min(frames.len());
    if a >= b {
        return;
    }
    match hole {
        Hole::FromRight => {
            frames.drain(a..b);
        }
        Hole::Empty => frames[a..b].fill(None),
        Hole::FromLeft => {
            let before = a.checked_sub(1).and_then(|i| frames[i]);
            frames[a..b].fill(before);
        }
    }
    trim(frames);
}

/// Insère `row` à l'image `at` ; la suite du calque avance.
pub(crate) fn insert_frames(frames: &mut Frames, at: usize, row: &[Option<CelId>]) {
    if at > frames.len() {
        if row.iter().all(Option::is_none) {
            return;
        }
        frames.resize(at, None);
    }
    frames.splice(at..at, row.iter().copied());
    trim(frames);
}

/// Inverse l'ordre des images [a, b).
pub(crate) fn reverse_frames(frames: &mut Frames, a: usize, b: usize) {
    if b > frames.len() {
        frames.resize(b, None);
    }
    frames[a..b].reverse();
    trim(frames);
}

/// Décale les clés de caméra et les marques d'image du plan quand `n` images sont insérées à
/// `at` (ou, `removed`, quand les images [at, at + n) sont supprimées : celles qui y étaient
/// partent).
pub(crate) fn shift_panel(panel: &mut Panel, at: usize, n: usize, removed: bool) {
    if removed {
        panel.camera.retain(|k| k.frame < at || k.frame >= at + n);
        panel.marks.retain(|m| m.frame < at || m.frame >= at + n);
        for k in panel.camera.iter_mut().filter(|k| k.frame >= at + n) {
            k.frame -= n;
        }
        for m in panel.marks.iter_mut().filter(|m| m.frame >= at + n) {
            m.frame -= n;
        }
    } else {
        for k in panel.camera.iter_mut().filter(|k| k.frame >= at) {
            k.frame += n;
        }
        for m in panel.marks.iter_mut().filter(|m| m.frame >= at) {
            m.frame += n;
        }
    }
    // Une marque ne peut pas être sur la première image (c'est le début du plan).
    panel.marks.retain(|m| m.frame > 0);
    // Signets, points d'entrée et de sortie (13d) : ils suivent leurs images.
    let moved = |f: usize| -> Option<usize> {
        match (removed, f >= at) {
            (_, false) => Some(f),
            (false, true) => Some(f + n),
            (true, true) if f < at + n => None,
            (true, true) => Some(f - n),
        }
    };
    let cues = &mut panel.cues;
    cues.bookmarks = cues.bookmarks.iter().filter_map(|&b| moved(b)).collect();
    // Un point dont l'image disparaît se pose sur le bord du trou.
    cues.in_point = cues.in_point.map(|f| moved(f).unwrap_or(at));
    cues.out_point = cues.out_point.map(|f| moved(f).unwrap_or(at.saturating_sub(1)));
    // Notes dessinées (13e) : elles suivent le temps ; dans un trou, elles se serrent au bord.
    let (a, nf) = (at as f32, n as f32);
    for note in &mut cues.notes {
        for p in &mut note.points {
            if removed {
                if p[0] >= a + nf {
                    p[0] -= nf;
                } else if p[0] > a {
                    p[0] = a;
                }
            } else if p[0] >= a {
                p[0] += nf;
            }
        }
    }
}

/// Bornes [début, fin) de l'instance (un dessin et ses expositions) qui contient `f`.
fn run_at(frames: &Frames, f: usize) -> (usize, usize) {
    let v = frames.get(f).copied().flatten();
    let mut s = f;
    while s > 0 && frames.get(s - 1).copied().flatten() == v {
        s -= 1;
    }
    let mut e = f + 1;
    while e < frames.len() && frames[e] == v {
        e += 1;
    }
    (s, e)
}

/// Début de l'instance dessinée suivante (après celle qui contient `f`).
pub(crate) fn next_instance(frames: &Frames, f: usize) -> Option<usize> {
    let (_, mut e) = run_at(frames, f);
    while e < frames.len() {
        if frames[e].is_some() {
            return Some(e);
        }
        e = run_at(frames, e).1;
    }
    None
}

/// Début de l'instance dessinée précédente : celle qui contient `f` si l'on n'est pas à son
/// début, sinon celle d'avant.
pub(crate) fn prev_instance(frames: &Frames, f: usize) -> Option<usize> {
    let (s, _) = run_at(frames, f);
    if s < f && frames.get(f).copied().flatten().is_some() {
        return Some(s);
    }
    let mut g = s;
    while g > 0 {
        let (s2, _) = run_at(frames, g - 1);
        if frames[s2].is_some() {
            return Some(s2);
        }
        g = s2;
    }
    None
}

/// Casse l'instance à l'image `f` : de `f` à la fin de l'instance, le dessin `copy` (une copie)
/// remplace l'ancien. Faux si `f` est le début de l'instance ou une image vide.
pub(crate) fn break_instance(frames: &mut Frames, f: usize, copy: CelId) -> bool {
    if frames.get(f).copied().flatten().is_none() {
        return false;
    }
    let (s, e) = run_at(frames, f);
    if s == f {
        return false;
    }
    frames[f..e].fill(Some(copy));
    true
}

/// Allonge (`more`) ou raccourcit d'une image chaque instance dessinée qui commence dans
/// [a, b) (ou qui contient `a`) ; une instance d'une seule image ne raccourcit pas. Renvoie le
/// nombre d'instances changées.
pub(crate) fn change_exposures(frames: &mut Frames, a: usize, b: usize, more: bool) -> usize {
    let mut runs = Vec::new();
    let mut f = run_at(frames, a.min(frames.len().saturating_sub(1))).0;
    while f < b.min(frames.len()) {
        let (s, e) = run_at(frames, f);
        if frames[s].is_some() {
            runs.push((s, e));
        }
        f = e;
    }
    let mut n = 0;
    for &(s, e) in runs.iter().rev() {
        if more {
            let v = frames[s];
            frames.insert(e, v);
            n += 1;
        } else if e - s > 1 {
            frames.remove(e - 1);
            n += 1;
        }
    }
    trim(frames);
    n
}

/// Comment le faux-fixe enchaîne les instances.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cycle {
    Loop,
    PingPong,
    Random,
}

/// Instances (dessin, nombre d'images) des images [a, b).
fn runs_in(frames: &Frames, a: usize, b: usize) -> Vec<(Option<CelId>, usize)> {
    let mut out: Vec<(Option<CelId>, usize)> = Vec::new();
    for f in a..b {
        let v = frames.get(f).copied().flatten();
        match out.last_mut() {
            Some((w, n)) if *w == v => *n += 1,
            _ => out.push((v, 1)),
        }
    }
    out
}

/// Faux-fixe (choix d'Anthony) : les instances des images [a, b) sont rejouées `times` fois en
/// tout, à la suite (insérées juste après), en boucle, en va-et-vient ou au hasard (`seed`) ;
/// chaque instance garde son exposition et son dessin. Renvoie le nombre d'images ajoutées.
pub(crate) fn fake_hold(frames: &mut Frames, a: usize, b: usize, cycle: Cycle, times: usize, seed: u32) -> usize {
    let runs = runs_in(frames, a, b);
    let r = runs.len();
    if r == 0 || times < 2 {
        return 0;
    }
    let mut rng = seed.max(1);
    let mut last = r - 1;
    let mut row: Frames = Vec::new();
    for i in 0..(times - 1) * r {
        let idx = match cycle {
            Cycle::Loop => i % r,
            Cycle::PingPong if r == 1 => 0,
            Cycle::PingPong => {
                let period = 2 * r - 2;
                let m = (r + i) % period;
                if m < r { m } else { period - m }
            }
            Cycle::Random => {
                rng ^= rng << 13;
                rng ^= rng >> 17;
                rng ^= rng << 5;
                let mut k = rng as usize % r;
                // Pas deux fois de suite la même instance, s'il y en a d'autres.
                if r > 1 && k == last {
                    k = (k + 1) % r;
                }
                k
            }
        };
        last = idx;
        row.extend(std::iter::repeat_n(runs[idx].0, runs[idx].1));
    }
    let n = row.len();
    insert_frames(frames, b, &row);
    n
}

/// Comment étirer un calque jusqu'à une longueur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stretch {
    HoldLast,
    Loop,
    PingPong,
}

/// Étire un calque jusqu'à `len` images après sa dernière image dessinée : en tenant celle-ci,
/// ou en rejouant ses images en boucle ou en va-et-vient (comme le post-comportement, mais posé
/// dans la timeline, où l'on peut ensuite le retoucher).
pub(crate) fn stretch_frames(frames: &Frames, len: usize, how: Stretch) -> Frames {
    use crate::document::{Behaviour, Layer, LayerKind};
    let mut tmp = Layer::new("", LayerKind::Bitmap);
    tmp.frames = frames.clone();
    tmp.post = match how {
        Stretch::HoldLast => Behaviour::Hold,
        Stretch::Loop => Behaviour::Loop,
        Stretch::PingPong => Behaviour::PingPong,
    };
    let Some(last) = frames.iter().rposition(Option::is_some) else { return frames.clone() };
    let mut out = frames.clone();
    out.truncate(last + 1);
    for f in last + 1..len {
        out.push(tmp.shown_at(f));
    }
    out
}

/// Où va un bloc d'images glissé.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DropMode {
    /// Sans touche : déplacé (sa place se vide, il recouvre ce qu'il y a là où on le pose).
    Move,
    /// Ctrl : copié (il recouvre ce qu'il y a là où on le pose).
    Copy,
    /// Maj : inséré (il quitte sa place, la suite recule, puis il s'insère là où on le pose).
    Insert,
}

/// Pose les images [a, b) à partir de l'image `to`, selon `mode` ; `row` : les images posées
/// (les mêmes, ou leurs copies).
pub(crate) fn drop_frames(frames: &mut Frames, a: usize, b: usize, to: usize, mode: DropMode, row: &[Option<CelId>]) {
    match mode {
        DropMode::Insert => {
            delete_frames(frames, a, b, Hole::FromRight);
            insert_frames(frames, to, row);
        }
        DropMode::Move | DropMode::Copy => {
            if mode == DropMode::Move {
                let end = b.min(frames.len());
                if a < end {
                    frames[a..end].fill(None);
                }
            }
            if frames.len() < to + row.len() {
                frames.resize(to + row.len(), None);
            }
            frames[to..to + row.len()].copy_from_slice(row);
            trim(frames);
        }
    }
}

/// Dessins d'une timeline dans l'ordre où ils apparaissent, chacun une fois : le premier est
/// l'instance 1.
pub(crate) fn drawings_in_order(frames: &Frames) -> Vec<CelId> {
    let mut out: Vec<CelId> = Vec::new();
    for id in frames.iter().flatten() {
        if !out.contains(id) {
            out.push(*id);
        }
    }
    out
}

/// Noms d'instance : « Tout renommer » numérote les dessins dans l'ordre (1, 2, 3…).
pub(crate) fn renumber(names: &mut BTreeMap<CelId, String>, order: &[CelId]) {
    for (k, id) in order.iter().enumerate() {
        names.insert(*id, (k + 1).to_string());
    }
}

/// Renommage intelligent (TVPaint) : un préfixe et un suffixe ajoutés aux noms des instances
/// des sortes choisies — sans nom (leur numéro sert de base), nommées par un nombre, nommées
/// par un texte (au moins une lettre).
#[derive(Clone, Debug, Default)]
pub(crate) struct SmartRename {
    pub prefix: String,
    pub suffix: String,
    pub empty: bool,
    pub numbers: bool,
    pub texts: bool,
}

/// Applique le renommage intelligent ; renvoie le nombre de noms changés.
pub(crate) fn smart_rename(names: &mut BTreeMap<CelId, String>, order: &[CelId], opts: &SmartRename) -> usize {
    let mut n = 0;
    for (k, id) in order.iter().enumerate() {
        let name = names.get(id).cloned().unwrap_or_default();
        let wanted = if name.trim().is_empty() {
            opts.empty
        } else if name.chars().any(char::is_alphabetic) {
            opts.texts
        } else {
            opts.numbers
        };
        if !wanted {
            continue;
        }
        let base = if name.trim().is_empty() { (k + 1).to_string() } else { name };
        names.insert(*id, format!("{}{}{}", opts.prefix, base, opts.suffix));
        n += 1;
    }
    n
}

/// Renommage d'une instance en cours dans la timeline : calque, dessin, nom tapé, champ déjà
/// activé.
pub(crate) struct InstanceRename {
    pub layer: u64,
    pub cel: CelId,
    pub name: String,
    pub focused: bool,
}

/// Images copiées : une rangée par calque (du plus bas au plus haut) et leurs dessins.
#[derive(Clone)]
pub(crate) struct ImageClip {
    pub rows: Vec<Frames>,
    pub cels: HashMap<CelId, Cel>,
}

impl App {
    /// Images visées : les images choisies, sinon l'image courante.
    pub(crate) fn image_range(&self) -> (usize, usize) {
        self.frame_sel.filter(|(a, b)| a < b).unwrap_or((self.cur_frame, self.cur_frame + 1))
    }

    /// Calques visés : les calques choisis, sans les dossiers.
    fn image_layers(&self) -> Vec<usize> {
        self.chosen_layers().into_iter().filter(|&li| !self.layers()[li].folder).collect()
    }

    /// Tous les calques du plan, sans les dossiers.
    fn all_image_layers(&self) -> Vec<usize> {
        (0..self.layers().len()).filter(|&li| !self.layers()[li].folder).collect()
    }

    /// Les images choisies visent la timeline : Ctrl+C / X / V et Suppr agissent sur elles
    /// après un clic dans la grille (comme pour les plans dans le montage, 12e).
    pub(crate) fn images_have_focus(&self) -> bool {
        self.images_focus && self.mode == Mode::Animation && self.frame_sel.is_some()
    }

    fn done(&mut self, snap: crate::history::Edit, status: String) {
        self.vsel = None;
        self.commit_structure(snap);
        self.status = status;
    }

    /// Supprime les images visées des calques choisis (`all` : de tous les calques du plan),
    /// selon le mode de trou. Sur tout le plan et en reculant la suite, le plan raccourcit et
    /// ses clés de caméra et marques suivent.
    pub(crate) fn delete_images(&mut self, all: bool) {
        // Storyboard verrouillé (13e) : ni les images ni la durée du plan ne changent.
        if self.content_refuses() || (!all && self.refuse_on_folder()) {
            return;
        }
        self.end_stroke();
        self.drop_bitmap_selection();
        let (a, b) = self.image_range();
        let lis = if all { self.all_image_layers() } else { self.image_layers() };
        let hole = self.hole;
        let snap = self.snapshot();
        for li in lis {
            delete_frames(&mut self.layers_mut()[li].frames, a, b, hole);
        }
        let n = b - a;
        if all && hole == Hole::FromRight {
            let len = self.panel().length;
            let panel = self.panel_mut();
            shift_panel(panel, a, n, true);
            panel.length = len.saturating_sub(n).max(1);
            self.cur_frame = a.min(self.panel().length - 1);
        }
        self.frame_sel = None;
        self.done(snap, format!("{n} image{} supprimée{}.", s(n), s(n)));
    }

    /// Insère `n` images vides avant (ou après) les images visées : sur les calques choisis, ou
    /// (`all`) sur tout le plan, qui s'allonge d'autant.
    pub(crate) fn insert_images(&mut self, before: bool, n: usize, all: bool) {
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        let (a, b) = self.image_range();
        let at = if before { a } else { b };
        let lis = if all { self.all_image_layers() } else { self.image_layers() };
        let snap = self.snapshot();
        let row = vec![None; n];
        for li in lis {
            insert_frames(&mut self.layers_mut()[li].frames, at, &row);
        }
        if all {
            let panel = self.panel_mut();
            shift_panel(panel, at, n, false);
            panel.length += n;
        }
        self.cur_frame = at;
        self.follow_frame = true;
        self.frame_sel = None;
        self.done(snap, format!("{n} image{} insérée{}.", s(n), s(n)));
    }

    /// Copie les images visées des calques choisis.
    pub(crate) fn copy_images(&mut self) {
        let (a, b) = self.image_range();
        let rows: Vec<Frames> = self.image_layers().into_iter().map(|li| frames_in(&self.layers()[li].frames, a, b)).collect();
        let cels = rows
            .iter()
            .flatten()
            .flatten()
            .filter_map(|id| self.doc.cels.get(id).map(|c| (*id, c.clone())))
            .collect();
        let (n, k) = (b - a, rows.len());
        self.image_clipboard = Some(ImageClip { rows, cels });
        self.status = format!("{n} image{} de {k} calque{} copiée{}.", s(n), s(k), s(n));
    }

    /// Coupe les images visées : copiées, puis supprimées (selon le mode de trou).
    pub(crate) fn cut_images(&mut self) {
        self.copy_images();
        self.delete_images(false);
    }

    /// Colle les images copiées : avant l'image courante (en insérant), ou (`replace`) à la
    /// place des images choisies. La première rangée va sur le calque courant, les suivantes
    /// sur les calques au-dessus ; chaque collage a ses propres copies des dessins.
    pub(crate) fn paste_images(&mut self, replace: bool) {
        let Some(clip) = self.image_clipboard.clone() else {
            self.status = "Aucune image copiée.".into();
            return;
        };
        if self.refuse_on_folder() {
            return;
        }
        self.end_stroke();
        self.drop_bitmap_selection();
        let targets: Vec<usize> = (self.cur_layer..self.layers().len()).filter(|&li| !self.layers()[li].folder).take(clip.rows.len()).collect();
        let (a, b) = self.image_range();
        let at = if replace { a } else { self.cur_frame };
        let snap = self.snapshot();
        let mut copies: HashMap<CelId, CelId> = HashMap::new();
        for (row, &li) in clip.rows.iter().zip(&targets) {
            let pasted: Frames = row
                .iter()
                .map(|f| {
                    f.and_then(|old| {
                        if let Some(&c) = copies.get(&old) {
                            return Some(c);
                        }
                        let c = self.doc.add_cel(clip.cels.get(&old)?.clone());
                        copies.insert(old, c);
                        Some(c)
                    })
                })
                .collect();
            let frames = &mut self.layers_mut()[li].frames;
            if replace {
                delete_frames(frames, a, b, Hole::FromRight);
            }
            insert_frames(frames, at, &pasted);
        }
        let n = clip.rows.first().map_or(0, Vec::len);
        self.frame_sel = Some((at, at + n));
        let lost = clip.rows.len().saturating_sub(targets.len());
        let mut status = format!("{n} image{} collée{}.", s(n), s(n));
        if lost > 0 {
            status += &format!(" {lost} rangée{} sans calque au-dessus, laissée{} de côté.", s(lost), s(lost));
        }
        self.done(snap, status);
    }

    /// Duplique les images visées juste après elles.
    pub(crate) fn duplicate_images(&mut self) {
        if self.refuse_on_folder() {
            return;
        }
        self.end_stroke();
        let (a, b) = self.image_range();
        let snap = self.snapshot();
        let mut copies: HashMap<CelId, CelId> = HashMap::new();
        for li in self.image_layers() {
            let row = frames_in(&self.layers()[li].frames, a, b);
            let dup: Frames = row
                .iter()
                .map(|f| {
                    f.map(|old| match copies.get(&old) {
                        Some(&c) => c,
                        None => {
                            let c = self.doc.duplicate_cel(old);
                            copies.insert(old, c);
                            c
                        }
                    })
                })
                .collect();
            insert_frames(&mut self.layers_mut()[li].frames, b, &dup);
        }
        self.frame_sel = Some((b, b + (b - a)));
        self.done(snap, "Images dupliquées après elles-mêmes.".into());
    }

    /// Inverse l'ordre des images visées.
    pub(crate) fn reverse_images(&mut self) {
        let (a, b) = self.image_range();
        if b - a < 2 {
            self.status = "Choisis plusieurs images à inverser (Maj + clic dans la timeline).".into();
            return;
        }
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        let snap = self.snapshot();
        for li in self.image_layers() {
            reverse_frames(&mut self.layers_mut()[li].frames, a, b);
        }
        self.done(snap, "Ordre des images inversé.".into());
    }

    /// Choisit toutes les images du plan (sur les calques choisis).
    pub(crate) fn select_all_images(&mut self) {
        self.frame_sel = Some((0, self.frame_count()));
        self.images_focus = true;
        self.status = "Toutes les images du plan sont choisies.".into();
    }

    /// Va au début de l'instance précédente (`next` : suivante) du calque courant.
    pub(crate) fn goto_instance(&mut self, next: bool) {
        let frames = &self.layer().frames;
        let to = if next { next_instance(frames, self.cur_frame) } else { prev_instance(frames, self.cur_frame) };
        match to {
            Some(f) => self.goto_frame(f),
            None => self.status = if next { "Pas d'instance après." } else { "Pas d'instance avant." }.into(),
        }
    }

    /// Va à la tête (première image) ou à la queue (dernière image) de l'instance courante.
    pub(crate) fn goto_instance_end(&mut self, tail: bool) {
        let (s, e) = run_at(&self.layer().frames, self.cur_frame);
        self.goto_frame(if tail { e - 1 } else { s });
    }

    /// Casse l'instance à l'image courante : la suite de l'exposition devient un nouveau
    /// dessin, copie de celui-ci.
    pub(crate) fn break_instance_here(&mut self) {
        if self.refuse_on_folder() {
            return;
        }
        self.end_stroke();
        let Some(id) = self.cel_here() else {
            self.status = "Pas de dessin à cette image.".into();
            return;
        };
        let (s, _) = run_at(&self.layer().frames, self.cur_frame);
        if s == self.cur_frame {
            self.status = "C'est déjà le début de l'instance.".into();
            return;
        }
        let snap = self.snapshot();
        let copy = self.doc.duplicate_cel(id);
        let (f, li) = (self.cur_frame, self.cur_layer);
        break_instance(&mut self.layers_mut()[li].frames, f, copy);
        self.done(snap, "Instance cassée : la suite est un nouveau dessin.".into());
    }

    /// Une exposition de plus (`more`) ou de moins à chaque instance des images visées.
    pub(crate) fn change_chosen_exposures(&mut self, more: bool) {
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        let (a, b) = self.image_range();
        let snap = self.snapshot();
        let mut n = 0;
        for li in self.image_layers() {
            n += change_exposures(&mut self.layers_mut()[li].frames, a, b, more);
        }
        if n == 0 {
            self.status = "Aucune instance à changer.".into();
            return;
        }
        let what = if more { "allongée" } else { "raccourcie" };
        self.done(snap, format!("{n} instance{} {what}{} d'une image.", s(n), s(n)));
    }

    /// Nouveau dessin vide avant l'instance courante (exposé selon « en N »).
    pub(crate) fn add_instance_before(&mut self) {
        if self.refuse_on_folder() {
            return;
        }
        self.end_stroke();
        let (start, _) = run_at(&self.layer().frames, self.cur_frame);
        let at = if self.cel_here().is_some() { start } else { self.cur_frame };
        let snap = self.snapshot();
        let kind = self.layer().kind;
        let id = self.doc.new_cel(kind);
        let n = self.default_exposure.max(1);
        let li = self.cur_layer;
        insert_frames(&mut self.layers_mut()[li].frames, at, &vec![Some(id); n]);
        self.cur_frame = at;
        self.follow_frame = true;
        self.done(snap, "Nouveau dessin avant l'instance.".into());
    }

    /// Faux-fixe des images visées sur les calques choisis : rejouées `fake_hold_times` fois en
    /// tout, à la suite.
    pub(crate) fn fake_hold_chosen(&mut self, cycle: Cycle) {
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        let (a, b) = self.image_range();
        let times = self.fake_hold_times.max(2);
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.subsec_nanos());
        let snap = self.snapshot();
        let mut added = 0;
        for li in self.image_layers() {
            added = added.max(fake_hold(&mut self.layers_mut()[li].frames, a, b, cycle, times, seed));
        }
        if added == 0 {
            self.status = "Rien à rejouer : choisis des images dessinées (Maj + clic).".into();
            return;
        }
        self.frame_sel = Some((b, b + added));
        let how = match cycle {
            Cycle::Loop => "en boucle",
            Cycle::PingPong => "en va-et-vient",
            Cycle::Random => "au hasard",
        };
        self.done(snap, format!("Faux-fixe {how} : {added} images ajoutées."));
    }

    /// Étire les calques choisis jusqu'à la fin du plan.
    pub(crate) fn stretch_chosen(&mut self, how: Stretch) {
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        let len = self.frame_count();
        let snap = self.snapshot();
        for li in self.image_layers() {
            let frames = stretch_frames(&self.layers()[li].frames, len, how);
            self.layers_mut()[li].frames = frames;
        }
        self.done(snap, "Calque étiré jusqu'à la fin du plan.".into());
    }

    /// Coupe les calques choisis après l'image courante.
    pub(crate) fn cut_layers_after_current(&mut self) {
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        let keep = self.cur_frame + 1;
        let snap = self.snapshot();
        for li in self.image_layers() {
            let frames = &mut self.layers_mut()[li].frames;
            frames.truncate(keep);
            trim(frames);
        }
        self.done(snap, "Calque coupé après l'image courante.".into());
    }

    /// Fin d'un glisser d'images choisies : posées à partir de leur nouvelle place, selon
    /// `mode` (une copie a ses propres dessins).
    pub(crate) fn drop_images(&mut self, delta: isize, mode: DropMode) {
        let (a, b) = self.image_range();
        let to = (a as isize + delta).max(0) as usize;
        if to == a && mode != DropMode::Copy {
            return;
        }
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        let snap = self.snapshot();
        let mut copies: HashMap<CelId, CelId> = HashMap::new();
        for li in self.image_layers() {
            let mut row = frames_in(&self.layers()[li].frames, a, b);
            if mode == DropMode::Copy {
                for f in row.iter_mut() {
                    *f = f.map(|old| *copies.entry(old).or_insert_with(|| self.doc.duplicate_cel(old)));
                }
            }
            drop_frames(&mut self.layers_mut()[li].frames, a, b, to, mode, &row);
        }
        self.frame_sel = Some((to, to + (b - a)));
        self.cur_frame = to;
        let what = match mode {
            DropMode::Move => "déplacées",
            DropMode::Copy => "copiées",
            DropMode::Insert => "insérées",
        };
        self.done(snap, format!("Images {what}."));
    }

    /// Renommer l'instance du dessin `cel` du calque `li` (double-clic dans la timeline).
    pub(crate) fn start_instance_rename(&mut self, li: usize, cel: CelId) {
        let Some(l) = self.layers().get(li) else { return };
        let name = l.names.get(&cel).cloned().unwrap_or_default();
        self.instance_rename = Some(InstanceRename { layer: l.id, cel, name, focused: false });
    }

    /// Pose le nom tapé (vide : le nom est retiré), une étape d'annulation.
    pub(crate) fn commit_instance_rename(&mut self) {
        let Some(r) = self.instance_rename.take() else { return };
        let Some(li) = self.layers().iter().position(|l| l.id == r.layer) else { return };
        let name = r.name.trim().to_string();
        if self.layers()[li].names.get(&r.cel).map_or("", String::as_str) == name {
            return;
        }
        self.record_step();
        let names = &mut self.layers_mut()[li].names;
        if name.is_empty() {
            names.remove(&r.cel);
        } else {
            names.insert(r.cel, name);
        }
    }

    /// Change les noms d'instance des calques choisis (une étape d'annulation).
    fn change_names(&mut self, change: impl Fn(&mut BTreeMap<CelId, String>, &[CelId]) -> usize, status: &str) {
        self.record_step();
        let mut n = 0;
        for li in self.image_layers() {
            let order = drawings_in_order(&self.layers()[li].frames);
            n += change(&mut self.layers_mut()[li].names, &order);
        }
        self.status = format!("{status} ({n}).");
    }

    /// Efface les noms d'instance des calques choisis.
    pub(crate) fn clear_instance_names(&mut self) {
        self.change_names(
            |names, _| {
                let n = names.len();
                names.clear();
                n
            },
            "Noms d'instance effacés",
        );
    }

    /// Numérote les instances des calques choisis dans l'ordre (1, 2, 3…).
    pub(crate) fn renumber_instances(&mut self) {
        self.change_names(
            |names, order| {
                renumber(names, order);
                order.len()
            },
            "Instances renommées 1, 2, 3…",
        );
    }

    /// Fenêtre du renommage intelligent, si elle est ouverte.
    pub(crate) fn smart_rename_window(&mut self, ctx: &egui::Context) {
        let Some(mut opts) = self.smart_rename.take() else { return };
        let mut keep = true;
        let mut apply = false;
        egui::Window::new("Renommer intelligemment les instances")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                egui::Grid::new("grille_renommage").num_columns(2).show(ui, |ui| {
                    ui.label("Préfixe");
                    ui.text_edit_singleline(&mut opts.prefix);
                    ui.end_row();
                    ui.label("Suffixe");
                    ui.text_edit_singleline(&mut opts.suffix);
                    ui.end_row();
                });
                ui.checkbox(&mut opts.empty, "Les instances sans nom (leur numéro sert de base)");
                ui.checkbox(&mut opts.numbers, "Les instances nommées par un nombre");
                ui.checkbox(&mut opts.texts, "Les instances nommées par un texte (au moins une lettre)");
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Renommer").clicked() {
                        apply = true;
                    }
                    if ui.button("Annuler").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        keep = false;
                    }
                });
            });
        if apply {
            let o = opts.clone();
            self.change_names(move |names, order| smart_rename(names, order, &o), "Instances renommées");
        } else if keep {
            self.smart_rename = Some(opts);
        }
    }

    /// Retourne les images visées : chaque dessin touché est remplacé, sur ces images, par une
    /// copie retournée (ailleurs, l'original reste).
    pub(crate) fn flip_images(&mut self, h: bool, v: bool) {
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        self.drop_bitmap_selection();
        let (a, b) = self.image_range();
        let snap = self.snapshot();
        let mut flipped: HashMap<CelId, CelId> = HashMap::new();
        for li in self.image_layers() {
            let end = b.min(self.layers()[li].frames.len());
            for f in a..end {
                let Some(old) = self.layers()[li].frames[f] else { continue };
                let new = *flipped.entry(old).or_insert_with(|| self.doc.flipped_cel(old, h, v));
                self.layers_mut()[li].frames[f] = Some(new);
            }
        }
        if flipped.is_empty() {
            self.status = "Aucun dessin à retourner sur ces images.".into();
            return;
        }
        self.done(snap, format!("{} dessin{} retourné{}.", flipped.len(), s(flipped.len()), s(flipped.len())));
    }

    /// Menu « Images » (barre de la timeline, menu Animation).
    pub(crate) fn images_menu(&mut self, ui: &mut egui::Ui) {
        let (a, b) = self.image_range();
        let what = if b - a > 1 { format!("les {} images choisies", b - a) } else { "l'image courante".into() };
        ui.label(egui::RichText::new(format!("Sur {what}, calques choisis :")).weak());
        self.action_item(ui, "Supprimer", "images.supprimer");
        self.action_item(ui, "Couper", "images.couper");
        self.action_item(ui, "Copier", "images.copier");
        self.action_item(ui, "Coller (insérer avant l'image courante)", "images.coller");
        self.action_item(ui, "Coller (remplacer les images choisies)", "images.coller_remplacer");
        self.action_item(ui, "Dupliquer", "images.dupliquer");
        self.action_item(ui, "Inverser l'ordre", "images.inverser");
        self.action_item(ui, "Insérer une image avant", "images.inserer_avant_calques");
        self.action_item(ui, "Insérer une image après", "images.inserer_apres_calques");
        ui.separator();
        ui.label(egui::RichText::new("Sur tout le plan (sa durée change) :").weak());
        self.action_item(ui, "Ajouter une image après l'image courante", "images.ajouter");
        self.action_item(ui, "Insérer une image avant l'image courante", "images.inserer");
        self.action_item(ui, "Supprimer l'image (ou les images choisies)", "images.supprimer_plan");
        ui.menu_button("Instances (dessin et ses expositions)", |ui| {
            self.action_item(ui, "Instance précédente", "instance.precedente");
            self.action_item(ui, "Instance suivante", "instance.suivante");
            self.action_item(ui, "Tête de l'instance", "instance.tete");
            self.action_item(ui, "Queue de l'instance", "instance.queue");
            ui.separator();
            self.action_item(ui, "Casser l'instance à l'image courante", "instance.casser");
            self.action_item(ui, "Nouveau dessin avant l'instance", "instance.ajouter_avant");
            self.action_item(ui, "Nouveau dessin après l'instance", "dessin.nouveau");
            ui.separator();
            self.action_item(ui, "Une exposition de plus à chaque instance choisie", "instance.exposition_plus");
            self.action_item(ui, "Une exposition de moins à chaque instance choisie", "instance.exposition_moins");
        });
        ui.menu_button("Faux-fixe (rejouer les images choisies)", |ui| {
            ui.horizontal(|ui| {
                ui.label("Jouées en tout");
                ui.add(egui::DragValue::new(&mut self.fake_hold_times).range(2..=50).suffix(" fois"));
            });
            self.action_item(ui, "À la suite, en boucle", "images.faux_fixe_boucle");
            self.action_item(ui, "En va-et-vient", "images.faux_fixe_va_et_vient");
            self.action_item(ui, "Au hasard", "images.faux_fixe_hasard");
        });
        ui.menu_button("Étirer le calque jusqu'à la fin du plan", |ui| {
            self.action_item(ui, "En tenant sa dernière image", "calque.etirer_tenir");
            self.action_item(ui, "En rejouant ses images en boucle", "calque.etirer_boucle");
            self.action_item(ui, "En rejouant ses images en va-et-vient", "calque.etirer_va_et_vient");
            ui.separator();
            self.action_item(ui, "Couper le calque après l'image courante", "calque.couper_apres");
        });
        ui.menu_button("Retourner", |ui| {
            self.action_item(ui, "Horizontalement", "images.retourner_h");
            self.action_item(ui, "Verticalement", "images.retourner_v");
            self.action_item(ui, "Horizontalement et verticalement", "images.retourner_hv");
        });
        ui.menu_button("Noms d'instance", |ui| {
            self.action_item(ui, "Renommer l'instance courante", "instance.renommer");
            self.action_item(ui, "Effacer les noms", "instance.effacer_noms");
            self.action_item(ui, "Tout renommer (1, 2, 3…)", "instance.tout_renommer");
            self.action_item(ui, "Renommer intelligemment…", "instance.renommer_intelligemment");
            ui.label(egui::RichText::new("Double-clic sur un dessin de la timeline : le renommer.").weak().small());
        });
        ui.label(egui::RichText::new("Glisser les images choisies : déplacer ; avec Ctrl : copier ; avec Maj : insérer.").weak().small());
        ui.separator();
        self.action_item(ui, "Choisir toutes les images", "images.tout_choisir");
        ui.menu_button("Trou laissé par une suppression", |ui| {
            for h in Hole::ALL {
                if ui.radio(self.hole == h, h.label()).clicked() {
                    self.hole = h;
                    ui.close_menu();
                }
            }
        });
    }
}

fn s(n: usize) -> &'static str {
    if n > 1 {
        "s"
    } else {
        ""
    }
}
