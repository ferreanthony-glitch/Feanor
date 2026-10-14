use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::audio::Pcm;
use crate::raster::Raster;
use crate::vector::VectorDrawing;

pub type CelId = u64;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum LayerKind {
    #[default]
    Bitmap,
    Vector,
}

impl LayerKind {
    pub fn badge(self) -> &'static str {
        match self {
            LayerKind::Bitmap => "B",
            LayerKind::Vector => "V",
        }
    }
}

/// Un dessin. Pour un dessin vectoriel, `raster` est son rendu, tenu à jour à chaque modification :
/// la composition, la pelure d'oignon et l'export ne voient que des pixels.
#[derive(Clone)]
pub struct Cel {
    pub raster: Raster,
    pub vector: Option<VectorDrawing>,
    /// Augmente à chaque modification (sert à savoir quand refaire les vignettes).
    pub version: u64,
}

impl Cel {
    pub fn new(width: usize, height: usize, kind: LayerKind) -> Self {
        Self {
            raster: Raster::new(width, height),
            vector: (kind == LayerKind::Vector).then(VectorDrawing::default),
            version: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        match &self.vector {
            Some(v) => v.elements.is_empty(),
            None => self.raster.tiles.iter().all(Option::is_none),
        }
    }

    /// Refait l'image d'un dessin vectoriel. Renvoie les tuiles qui ont pu changer.
    pub fn rerender(&mut self) -> Vec<usize> {
        let Some(v) = &self.vector else {
            return Vec::new();
        };
        let fresh = v.render(self.raster.width, self.raster.height);
        let changed = (0..fresh.tiles.len())
            .filter(|&i| self.raster.tiles[i].is_some() || fresh.tiles[i].is_some())
            .collect();
        self.raster = fresh;
        self.version += 1;
        changed
    }
}

static NEXT_LAYER_ID: AtomicU64 = AtomicU64::new(1);

/// Identifiant de calque, unique pendant la session (il n'est pas enregistré).
pub fn fresh_layer_id() -> u64 {
    NEXT_LAYER_ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone)]
pub struct Layer {
    pub id: u64,
    pub name: String,
    pub kind: LayerKind,
    pub visible: bool,
    pub opacity: f32,
    /// Une entrée par image du plan : le dessin exposé à cette image, ou rien.
    /// Plusieurs images consécutives qui pointent vers le même dessin forment une exposition
    /// (un dessin « tenu » sur 2, 3… images).
    pub frames: Vec<Option<CelId>>,
}

impl Layer {
    pub fn new(name: impl Into<String>, kind: LayerKind) -> Self {
        Self {
            id: fresh_layer_id(),
            name: name.into(),
            kind,
            visible: true,
            opacity: 1.0,
            frames: Vec::new(),
        }
    }

    pub fn cel_at(&self, frame: usize) -> Option<CelId> {
        self.frames.get(frame).copied().flatten()
    }

    /// Bornes [début, fin) de l'exposition qui contient `frame`.
    pub fn run_bounds(&self, frame: usize) -> (usize, usize) {
        if frame >= self.frames.len() {
            return (frame, frame + 1);
        }
        let value = self.frames[frame];
        let mut start = frame;
        while start > 0 && self.frames[start - 1] == value {
            start -= 1;
        }
        let mut end = frame + 1;
        while end < self.frames.len() && self.frames[end] == value {
            end += 1;
        }
        (start, end)
    }

    /// Fin (exclue) de la plage d'images vides qui commence à `frame`, sans dépasser `limit`.
    pub fn empty_run_end(&self, frame: usize, limit: usize) -> usize {
        let mut f = frame;
        while f < limit && self.cel_at(f).is_none() {
            f += 1;
        }
        f.max(frame + 1)
    }

    pub fn set(&mut self, frame: usize, cel: Option<CelId>) {
        if frame >= self.frames.len() {
            self.frames.resize(frame + 1, None);
        }
        self.frames[frame] = cel;
    }

    /// Insère `count` images à la position `at` en décalant la suite du calque.
    pub fn insert(&mut self, at: usize, cel: Option<CelId>, count: usize) {
        if at > self.frames.len() {
            self.frames.resize(at, None);
        }
        self.frames.splice(at..at, std::iter::repeat_n(cel, count));
    }

    /// Retire les images vides en fin de calque.
    pub fn trim(&mut self) {
        while self.frames.last() == Some(&None) {
            self.frames.pop();
        }
    }
}

/// Une séquence : un groupe de scènes qui se suivent.
#[derive(Clone, Debug, PartialEq)]
pub struct Sequence {
    pub id: u64,
    /// Nom choisi ; vide = numéro automatique.
    pub name: String,
}

/// Une scène (un plan, au sens du découpage) : un groupe de panneaux qui se suivent.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub id: u64,
    /// Nom ou numéro choisi ; vide = numéro automatique.
    pub name: String,
    pub sequence: u64,
}

/// Un panneau du storyboard : ses calques (et leur animation image par image), sa durée
/// et ses indications.
#[derive(Clone)]
pub struct Panel {
    pub id: u64,
    /// Scène à laquelle appartient le panneau.
    pub scene: u64,
    /// Durée minimale voulue ; le panneau s'allonge tout seul si l'animation dépasse.
    pub length: usize,
    pub layers: Vec<Layer>,
    pub action: String,
    pub dialogue: String,
    pub notes: String,
}

impl Panel {
    pub fn frame_count(&self) -> usize {
        self.layers
            .iter()
            .map(|l| l.frames.len())
            .max()
            .unwrap_or(0)
            .max(self.length)
            .max(1)
    }

    /// Change la durée : ce qui dépasse est coupé ; en allongeant, les dessins tenus jusqu'à la fin
    /// du plan restent tenus jusqu'à la nouvelle fin.
    pub fn set_length(&mut self, n: usize) {
        let n = n.max(1);
        let old = self.frame_count();
        for layer in &mut self.layers {
            if layer.frames.len() > n {
                layer.frames.truncate(n);
            } else if n > old && layer.frames.len() == old {
                if let Some(&Some(last)) = layer.frames.last() {
                    layer.frames.resize(n, Some(last));
                }
            }
            layer.trim();
        }
        self.length = n;
    }
}

/// Un fichier son importé, gardé tel quel (pour l'enregistrement) et décodé (pour la lecture).
#[derive(Clone)]
pub struct Sound {
    pub id: u64,
    /// Nom du fichier d'origine, sans dossier.
    pub name: String,
    /// Extension en minuscules (wav, mp3, flac, ogg).
    pub ext: String,
    pub bytes: Arc<Vec<u8>>,
    pub pcm: Arc<Pcm>,
}

impl Sound {
    /// Durée en images (arrondie au-dessus).
    pub fn frames(&self, fps: u32) -> usize {
        (self.pcm.seconds() * fps.max(1) as f64).ceil().max(1.0) as usize
    }
}

/// Un extrait de son posé sur une piste, en images du montage.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundClip {
    pub id: u64,
    pub sound: u64,
    /// Première image du montage où il commence.
    pub start: usize,
    /// Images sautées au début du son (extrait raccourci par la gauche).
    pub offset: usize,
    pub length: usize,
}

impl SoundClip {
    pub fn end(&self) -> usize {
        self.start + self.length
    }
}

/// Une piste son : des extraits qui ne se chevauchent pas.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioTrack {
    pub id: u64,
    pub name: String,
    pub muted: bool,
    /// Gain linéaire (1 = inchangé).
    pub volume: f32,
    pub clips: Vec<SoundClip>,
}

/// Séquences, scènes, panneaux et pistes son : tout ce qu'une opération de montage peut changer.
/// Les panneaux sont dans l'ordre du film ; ceux d'une même scène se suivent, comme les
/// scènes d'une même séquence.
#[derive(Clone)]
pub struct Structure {
    pub sequences: Vec<Sequence>,
    pub scenes: Vec<Scene>,
    pub panels: Vec<Panel>,
    pub audio_tracks: Vec<AudioTrack>,
}

pub struct Document {
    pub width: usize,
    pub height: usize,
    pub fps: u32,
    pub paper: [u8; 3],
    pub sequences: Vec<Sequence>,
    pub scenes: Vec<Scene>,
    pub panels: Vec<Panel>,
    /// Tous les dessins du projet. On n'en retire jamais pendant la session (l'historique
    /// d'annulation peut y faire référence) ; seuls ceux encore utilisés sont enregistrés.
    pub cels: HashMap<CelId, Cel>,
    /// Pistes son, en temps du montage (elles ne bougent pas avec les panneaux).
    pub audio_tracks: Vec<AudioTrack>,
    /// Sons importés. Comme les dessins, on n'en retire pas pendant la session.
    pub sounds: HashMap<u64, Sound>,
    next_id: u64,
}

impl Document {
    /// Projet neuf : une séquence, une scène, un panneau.
    pub fn new(width: usize, height: usize, fps: u32) -> Self {
        let mut doc = Self {
            width,
            height,
            fps,
            paper: [255, 255, 255],
            sequences: Vec::new(),
            scenes: Vec::new(),
            panels: Vec::new(),
            cels: HashMap::new(),
            audio_tracks: Vec::new(),
            sounds: HashMap::new(),
            next_id: 1,
        };
        let seq = doc.new_sequence();
        let scene = doc.new_scene(seq);
        let panel = doc.make_panel(scene);
        doc.panels.push(panel);
        doc
    }

    pub fn from_parts(
        width: usize,
        height: usize,
        fps: u32,
        paper: [u8; 3],
        structure: Structure,
        cels: HashMap<CelId, Cel>,
        sounds: HashMap<u64, Sound>,
    ) -> Self {
        let max_id = structure
            .panels
            .iter()
            .map(|p| p.id)
            .chain(structure.scenes.iter().map(|s| s.id))
            .chain(structure.sequences.iter().map(|s| s.id))
            .chain(structure.audio_tracks.iter().map(|t| t.id))
            .chain(structure.audio_tracks.iter().flat_map(|t| t.clips.iter().map(|c| c.id)))
            .chain(cels.keys().copied())
            .chain(sounds.keys().copied())
            .max()
            .unwrap_or(0);
        let mut doc = Self {
            width,
            height,
            fps,
            paper,
            sequences: structure.sequences,
            scenes: structure.scenes,
            panels: structure.panels,
            cels,
            audio_tracks: structure.audio_tracks,
            sounds,
            next_id: max_id + 1,
        };
        doc.normalize();
        doc
    }

    pub fn structure(&self) -> Structure {
        Structure {
            sequences: self.sequences.clone(),
            scenes: self.scenes.clone(),
            panels: self.panels.clone(),
            audio_tracks: self.audio_tracks.clone(),
        }
    }

    pub fn replace_structure(&mut self, s: Structure) -> Structure {
        Structure {
            sequences: std::mem::replace(&mut self.sequences, s.sequences),
            scenes: std::mem::replace(&mut self.scenes, s.scenes),
            panels: std::mem::replace(&mut self.panels, s.panels),
            audio_tracks: std::mem::replace(&mut self.audio_tracks, s.audio_tracks),
        }
    }

    fn alloc(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn new_sequence(&mut self) -> u64 {
        let id = self.alloc();
        self.sequences.push(Sequence { id, name: String::new() });
        id
    }

    pub fn new_scene(&mut self, sequence: u64) -> u64 {
        let id = self.alloc();
        self.scenes.push(Scene {
            id,
            name: String::new(),
            sequence,
        });
        id
    }

    pub fn scene(&self, id: u64) -> Option<&Scene> {
        self.scenes.iter().find(|s| s.id == id)
    }

    pub fn scene_mut(&mut self, id: u64) -> Option<&mut Scene> {
        self.scenes.iter_mut().find(|s| s.id == id)
    }

    pub fn sequence_mut(&mut self, id: u64) -> Option<&mut Sequence> {
        self.sequences.iter_mut().find(|s| s.id == id)
    }

    /// Séquence du panneau `index`.
    pub fn sequence_of(&self, index: usize) -> u64 {
        self.scene(self.panels[index].scene).map_or(0, |s| s.sequence)
    }

    /// Bornes [début, fin) des panneaux de la scène du panneau `index`.
    pub fn scene_run(&self, index: usize) -> (usize, usize) {
        let scene = self.panels[index].scene;
        self.run(index, |d, i| d.panels[i].scene == scene)
    }

    /// Bornes [début, fin) des panneaux de la séquence du panneau `index`.
    pub fn sequence_run(&self, index: usize) -> (usize, usize) {
        let seq = self.sequence_of(index);
        self.run(index, |d, i| d.sequence_of(i) == seq)
    }

    fn run(&self, index: usize, same: impl Fn(&Self, usize) -> bool) -> (usize, usize) {
        let mut start = index;
        while start > 0 && same(self, start - 1) {
            start -= 1;
        }
        let mut end = index + 1;
        while end < self.panels.len() && same(self, end) {
            end += 1;
        }
        (start, end)
    }

    /// Scènes dans l'ordre du film.
    pub fn scene_order(&self) -> Vec<u64> {
        let mut order: Vec<u64> = Vec::new();
        for p in &self.panels {
            if order.last() != Some(&p.scene) && !order.contains(&p.scene) {
                order.push(p.scene);
            }
        }
        order
    }

    /// Séquences dans l'ordre du film.
    pub fn sequence_order(&self) -> Vec<u64> {
        let mut order: Vec<u64> = Vec::new();
        for i in 0..self.panels.len() {
            let s = self.sequence_of(i);
            if !order.contains(&s) {
                order.push(s);
            }
        }
        order
    }

    /// Numéro automatique de la scène (1, 2, 3… dans l'ordre du film, toutes séquences confondues).
    pub fn scene_number(&self, scene: u64) -> usize {
        self.scene_order().iter().position(|&s| s == scene).map_or(0, |i| i + 1)
    }

    pub fn sequence_number(&self, sequence: u64) -> usize {
        self.sequence_order().iter().position(|&s| s == sequence).map_or(0, |i| i + 1)
    }

    /// Nom affiché de la scène : son nom s'il en a un, sinon son numéro.
    pub fn scene_label(&self, scene: u64) -> String {
        match self.scene(scene) {
            Some(s) if !s.name.trim().is_empty() => s.name.trim().to_string(),
            _ => self.scene_number(scene).to_string(),
        }
    }

    pub fn sequence_label(&self, sequence: u64) -> String {
        match self.sequences.iter().find(|s| s.id == sequence) {
            Some(s) if !s.name.trim().is_empty() => s.name.trim().to_string(),
            _ => self.sequence_number(sequence).to_string(),
        }
    }

    /// Numéro du panneau dans sa scène, et nombre de panneaux de la scène.
    pub fn panel_number(&self, index: usize) -> (usize, usize) {
        let (start, end) = self.scene_run(index);
        (index - start + 1, end - start)
    }

    /// Remet de l'ordre après une opération : les panneaux d'une même scène se suivent, les scènes
    /// d'une même séquence aussi (dans l'ordre de première apparition), et les scènes ou séquences
    /// vides disparaissent.
    pub fn normalize(&mut self) {
        let seq_of_scene = |scenes: &[Scene], id: u64| scenes.iter().find(|s| s.id == id).map(|s| s.sequence);
        // Un panneau sans scène connue rejoint la scène du panneau précédent (ou une scène neuve).
        for i in 0..self.panels.len() {
            if self.scene(self.panels[i].scene).is_none() {
                let scene = match i.checked_sub(1) {
                    Some(prev) => self.panels[prev].scene,
                    None => {
                        let seq = self.sequences.first().map(|s| s.id).unwrap_or_else(|| self.new_sequence());
                        self.new_scene(seq)
                    }
                };
                self.panels[i].scene = scene;
            }
        }
        for s in 0..self.scenes.len() {
            if !self.sequences.iter().any(|q| q.id == self.scenes[s].sequence) {
                let seq = self.sequences.first().map(|s| s.id).unwrap_or_else(|| self.new_sequence());
                self.scenes[s].sequence = seq;
            }
        }
        let scene_order = self.scene_order();
        let mut seq_order: Vec<u64> = Vec::new();
        for &sc in &scene_order {
            if let Some(q) = seq_of_scene(&self.scenes, sc) {
                if !seq_order.contains(&q) {
                    seq_order.push(q);
                }
            }
        }
        let mut panels = std::mem::take(&mut self.panels);
        let mut ordered = Vec::with_capacity(panels.len());
        for &q in &seq_order {
            for &sc in scene_order.iter().filter(|&&sc| seq_of_scene(&self.scenes, sc) == Some(q)) {
                let mut i = 0;
                while i < panels.len() {
                    if panels[i].scene == sc {
                        ordered.push(panels.remove(i));
                    } else {
                        i += 1;
                    }
                }
            }
        }
        ordered.extend(panels);
        self.panels = ordered;
        self.scenes.retain(|s| scene_order.contains(&s.id));
        self.sequences.retain(|q| seq_order.contains(&q.id));
    }

    // ---------------------------------------------------------------- opérations de montage
    // Chacune renvoie l'indice du panneau à rendre courant.

    /// Nouveau panneau juste après le panneau `index`, dans la même scène.
    pub fn add_panel_after(&mut self, index: usize) -> usize {
        let panel = self.make_panel(self.panels[index].scene);
        self.insert_and_normalize(index + 1, vec![panel])
    }

    /// Nouvelle scène (avec un panneau) après la scène du panneau `index`, dans la même séquence.
    pub fn add_scene_after(&mut self, index: usize) -> usize {
        let (_, end) = self.scene_run(index);
        let scene = self.new_scene(self.sequence_of(index));
        let panel = self.make_panel(scene);
        self.insert_and_normalize(end, vec![panel])
    }

    /// Nouvelle séquence (une scène, un panneau) après la séquence du panneau `index`.
    pub fn add_sequence_after(&mut self, index: usize) -> usize {
        let (_, end) = self.sequence_run(index);
        let seq = self.new_sequence();
        let scene = self.new_scene(seq);
        let panel = self.make_panel(scene);
        self.insert_and_normalize(end, vec![panel])
    }

    /// Copie du panneau `index`, juste après lui.
    pub fn duplicate_panel_after(&mut self, index: usize) -> usize {
        let copy = self.duplicate_panel(index);
        self.insert_and_normalize(index + 1, vec![copy])
    }

    /// Copie de la scène du panneau `index`, juste après elle.
    pub fn duplicate_scene_after(&mut self, index: usize) -> usize {
        let (_, end) = self.scene_run(index);
        let copies = self.duplicate_scene(index);
        self.insert_and_normalize(end, copies)
    }

    /// Retire le panneau `index` (et sa scène ou sa séquence si elles deviennent vides).
    /// Refusé s'il n'en reste qu'un.
    pub fn remove_panel(&mut self, index: usize) -> Option<usize> {
        if self.panels.len() <= 1 {
            return None;
        }
        self.panels.remove(index);
        let next = self.panels[index.min(self.panels.len() - 1)].id;
        Some(self.normalize_keeping(next))
    }

    /// Retire toute la scène du panneau `index`. Refusé si c'est la seule.
    pub fn remove_scene(&mut self, index: usize) -> Option<usize> {
        let (start, end) = self.scene_run(index);
        if end - start >= self.panels.len() {
            return None;
        }
        self.panels.drain(start..end);
        let next = self.panels[start.min(self.panels.len() - 1)].id;
        Some(self.normalize_keeping(next))
    }

    /// Déplace le panneau `from` à la place `slot` (0 = avant le premier, `len` = après le dernier).
    /// Il reste dans sa scène s'il retombe à côté d'un panneau de sa scène ; sinon il rejoint la
    /// scène du panneau devant lequel on le pose (ou du dernier, si on le pose tout à la fin).
    pub fn move_panel(&mut self, from: usize, slot: usize) -> Option<usize> {
        let n = self.panels.len();
        if from >= n || slot > n || slot == from || slot == from + 1 {
            return None;
        }
        let mut moved = self.panels.remove(from);
        let at = if slot > from { slot - 1 } else { slot };
        let left = at.checked_sub(1).map(|i| self.panels[i].scene);
        let right = self.panels.get(at).map(|p| p.scene);
        if left != Some(moved.scene) && right != Some(moved.scene) {
            if let Some(scene) = right.or(left) {
                moved.scene = scene;
            }
        }
        let id = moved.id;
        self.panels.insert(at, moved);
        Some(self.normalize_keeping(id))
    }

    fn insert_and_normalize(&mut self, at: usize, panels: Vec<Panel>) -> usize {
        let first = panels[0].id;
        let at = at.min(self.panels.len());
        self.panels.splice(at..at, panels);
        self.normalize_keeping(first)
    }

    /// Remet en ordre, et renvoie le nouvel indice du panneau `id`.
    fn normalize_keeping(&mut self, id: u64) -> usize {
        self.normalize();
        self.panels.iter().position(|p| p.id == id).unwrap_or(0)
    }

    /// Un panneau vide de deux secondes dans la scène `scene`, avec un calque vectoriel.
    pub fn make_panel(&mut self, scene: u64) -> Panel {
        Panel {
            id: self.alloc(),
            scene,
            length: self.fps as usize * 2,
            layers: vec![Layer::new("Dessin", LayerKind::Vector)],
            action: String::new(),
            dialogue: String::new(),
            notes: String::new(),
        }
    }

    /// Copie complète d'une scène et de ses panneaux, dans la même séquence.
    pub fn duplicate_scene(&mut self, index: usize) -> Vec<Panel> {
        let (start, end) = self.scene_run(index);
        let old = self.panels[index].scene;
        let (name, sequence) = self.scene(old).map_or((String::new(), 0), |s| (s.name.clone(), s.sequence));
        let scene = self.new_scene(sequence);
        if !name.trim().is_empty() {
            if let Some(s) = self.scene_mut(scene) {
                s.name = format!("{} copie", name.trim());
            }
        }
        (start..end)
            .map(|i| {
                let mut p = self.duplicate_panel(i);
                p.scene = scene;
                p
            })
            .collect()
    }

    /// Copie complète d'un panneau : ses dessins sont dupliqués, modifier l'un ne touche pas l'autre.
    pub fn duplicate_panel(&mut self, index: usize) -> Panel {
        let mut panel = self.panels[index].clone();
        panel.id = self.alloc();
        let mut copies: HashMap<CelId, CelId> = HashMap::new();
        for layer in &mut panel.layers {
            layer.id = fresh_layer_id();
            for f in layer.frames.iter_mut().flatten() {
                *f = match copies.get(f) {
                    Some(&c) => c,
                    None => {
                        let c = self.duplicate_cel(*f);
                        copies.insert(*f, c);
                        c
                    }
                };
            }
        }
        panel
    }

    pub fn new_cel(&mut self, kind: LayerKind) -> CelId {
        let id = self.alloc();
        self.cels.insert(id, Cel::new(self.width, self.height, kind));
        id
    }

    pub fn duplicate_cel(&mut self, source: CelId) -> CelId {
        let mut copy = self
            .cels
            .get(&source)
            .cloned()
            .unwrap_or_else(|| Cel::new(self.width, self.height, LayerKind::Bitmap));
        copy.version = 0;
        let id = self.alloc();
        self.cels.insert(id, copy);
        id
    }

    pub fn used_cels(&self) -> BTreeSet<CelId> {
        self.panels
            .iter()
            .flat_map(|s| s.layers.iter())
            .flat_map(|l| l.frames.iter().flatten().copied())
            .collect()
    }

    /// Première image du plan `index` dans le montage.
    pub fn panel_start(&self, index: usize) -> usize {
        self.panels[..index].iter().map(Panel::frame_count).sum()
    }

    pub fn total_frames(&self) -> usize {
        self.panels.iter().map(Panel::frame_count).sum::<usize>().max(1)
    }

    /// Plan et image locale correspondant à une image du montage.
    pub fn locate(&self, global: usize) -> (usize, usize) {
        let mut start = 0;
        for (i, panel) in self.panels.iter().enumerate() {
            let len = panel.frame_count();
            if global < start + len {
                return (i, global - start);
            }
            start += len;
        }
        let last = self.panels.len() - 1;
        (last, self.panels[last].frame_count() - 1)
    }

    // ---------------------------------------------------------------------------- son

    pub fn add_sound(&mut self, name: String, ext: String, bytes: Arc<Vec<u8>>, pcm: Arc<Pcm>) -> u64 {
        let id = self.alloc();
        self.sounds.insert(id, Sound { id, name, ext, bytes, pcm });
        id
    }

    /// Nouvelle piste son vide, en dernière position. Renvoie son identifiant.
    pub fn add_audio_track(&mut self) -> u64 {
        let id = self.alloc();
        let n = self.audio_tracks.len() + 1;
        self.audio_tracks.push(AudioTrack {
            id,
            name: format!("Son {n}"),
            muted: false,
            volume: 1.0,
            clips: Vec::new(),
        });
        id
    }

    /// Fin (exclue) du dernier extrait son, en images du montage.
    pub fn audio_end(&self) -> usize {
        self.audio_tracks
            .iter()
            .flat_map(|t| t.clips.iter().map(SoundClip::end))
            .max()
            .unwrap_or(0)
    }

    /// Piste et place de l'extrait `id`.
    pub fn find_clip(&self, id: u64) -> Option<(usize, usize)> {
        self.audio_tracks
            .iter()
            .enumerate()
            .find_map(|(t, track)| track.clips.iter().position(|c| c.id == id).map(|c| (t, c)))
    }

    pub fn clip(&self, id: u64) -> Option<&SoundClip> {
        self.find_clip(id).map(|(t, c)| &self.audio_tracks[t].clips[c])
    }

    /// Vrai si la plage [start, end) de la piste `track` est libre (en ignorant l'extrait `except`).
    pub fn track_free(&self, track: usize, start: usize, end: usize, except: u64) -> bool {
        self.audio_tracks.get(track).is_some_and(|t| {
            t.clips
                .iter()
                .all(|c| c.id == except || c.end() <= start || c.start >= end)
        })
    }

    /// Pose tout le son `sound` à l'image `start` : sur la piste `track` si la place est libre,
    /// sinon sur la première piste libre, sinon sur une nouvelle piste.
    /// Renvoie l'identifiant du nouvel extrait.
    pub fn place_sound(&mut self, sound: u64, start: usize, track: Option<usize>) -> Option<u64> {
        let length = self.sounds.get(&sound)?.frames(self.fps);
        let end = start + length;
        let t = track
            .filter(|&t| self.track_free(t, start, end, 0))
            .or_else(|| (0..self.audio_tracks.len()).find(|&t| self.track_free(t, start, end, 0)))
            .unwrap_or_else(|| {
                self.add_audio_track();
                self.audio_tracks.len() - 1
            });
        let id = self.alloc();
        let clips = &mut self.audio_tracks[t].clips;
        clips.push(SoundClip {
            id,
            sound,
            start,
            offset: 0,
            length,
        });
        clips.sort_by_key(|c| c.start);
        Some(id)
    }

    pub fn remove_clip(&mut self, id: u64) -> bool {
        match self.find_clip(id) {
            Some((t, c)) => {
                self.audio_tracks[t].clips.remove(c);
                true
            }
            None => false,
        }
    }

    /// Coupe l'extrait `id` à l'image `at` du montage. Renvoie l'identifiant de la partie droite.
    pub fn split_clip(&mut self, id: u64, at: usize) -> Option<u64> {
        let (t, c) = self.find_clip(id)?;
        let clip = self.audio_tracks[t].clips[c].clone();
        if at <= clip.start || at >= clip.end() {
            return None;
        }
        let right_id = self.alloc();
        let cut = at - clip.start;
        let right = SoundClip {
            id: right_id,
            sound: clip.sound,
            start: at,
            offset: clip.offset + cut,
            length: clip.length - cut,
        };
        let clips = &mut self.audio_tracks[t].clips;
        clips[c].length = cut;
        clips.insert(c + 1, right);
        Some(right_id)
    }

    /// Place l'extrait `id` sur [start, end) de la piste `track`, en gardant le même son sous les
    /// mêmes images (une poignée de bord ne fait que montrer ou cacher une partie du son).
    /// Refusé si la place est prise ou si on dépasse le son. `keep_offset` : déplacement simple,
    /// l'extrait garde son contenu.
    pub fn set_clip_span(&mut self, id: u64, track: usize, start: usize, end: usize, keep_offset: bool) -> bool {
        let Some((t, c)) = self.find_clip(id) else {
            return false;
        };
        let clip = self.audio_tracks[t].clips[c].clone();
        let Some(total) = self.sounds.get(&clip.sound).map(|s| s.frames(self.fps)) else {
            return false;
        };
        if end <= start || !self.track_free(track, start, end, id) {
            return false;
        }
        let offset = if keep_offset {
            clip.offset as i64
        } else {
            clip.offset as i64 + start as i64 - clip.start as i64
        };
        if offset < 0 || offset as usize + (end - start) > total {
            return false;
        }
        let mut moved = self.audio_tracks[t].clips.remove(c);
        moved.start = start;
        moved.length = end - start;
        moved.offset = offset as usize;
        let clips = &mut self.audio_tracks[track].clips;
        clips.push(moved);
        clips.sort_by_key(|c| c.start);
        true
    }
}
