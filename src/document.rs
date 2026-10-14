use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

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

/// Un plan du montage : ses calques, sa durée et ses indications de storyboard.
#[derive(Clone)]
pub struct Shot {
    pub id: u64,
    pub name: String,
    /// Durée minimale voulue ; le plan s'allonge tout seul si l'animation dépasse.
    pub length: usize,
    pub layers: Vec<Layer>,
    pub action: String,
    pub dialogue: String,
    pub notes: String,
}

impl Shot {
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

pub struct Document {
    pub width: usize,
    pub height: usize,
    pub fps: u32,
    pub paper: [u8; 3],
    pub shots: Vec<Shot>,
    /// Tous les dessins du projet. On n'en retire jamais pendant la session (l'historique
    /// d'annulation peut y faire référence) ; seuls ceux encore utilisés sont enregistrés.
    pub cels: HashMap<CelId, Cel>,
    next_id: u64,
}

impl Document {
    pub fn new(width: usize, height: usize, fps: u32) -> Self {
        let mut doc = Self {
            width,
            height,
            fps,
            paper: [255, 255, 255],
            shots: Vec::new(),
            cels: HashMap::new(),
            next_id: 1,
        };
        let shot = doc.make_shot();
        doc.shots.push(shot);
        doc
    }

    pub fn from_parts(
        width: usize,
        height: usize,
        fps: u32,
        paper: [u8; 3],
        shots: Vec<Shot>,
        cels: HashMap<CelId, Cel>,
    ) -> Self {
        let max_cel = cels.keys().max().copied().unwrap_or(0);
        let max_shot = shots.iter().map(|s| s.id).max().unwrap_or(0);
        Self {
            width,
            height,
            fps,
            paper,
            shots,
            cels,
            next_id: max_cel.max(max_shot) + 1,
        }
    }

    fn alloc(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Un plan vide de deux secondes, avec un calque vectoriel.
    pub fn make_shot(&mut self) -> Shot {
        Shot {
            id: self.alloc(),
            name: String::new(),
            length: self.fps as usize * 2,
            layers: vec![Layer::new("Dessin", LayerKind::Vector)],
            action: String::new(),
            dialogue: String::new(),
            notes: String::new(),
        }
    }

    /// Copie complète d'un plan : ses dessins sont dupliqués, modifier l'un ne touche pas l'autre.
    pub fn duplicate_shot(&mut self, index: usize) -> Shot {
        let mut shot = self.shots[index].clone();
        shot.id = self.alloc();
        let mut copies: HashMap<CelId, CelId> = HashMap::new();
        for layer in &mut shot.layers {
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
        shot
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
        self.shots
            .iter()
            .flat_map(|s| s.layers.iter())
            .flat_map(|l| l.frames.iter().flatten().copied())
            .collect()
    }

    /// Première image du plan `index` dans le montage.
    pub fn shot_start(&self, index: usize) -> usize {
        self.shots[..index].iter().map(Shot::frame_count).sum()
    }

    pub fn total_frames(&self) -> usize {
        self.shots.iter().map(Shot::frame_count).sum::<usize>().max(1)
    }

    /// Plan et image locale correspondant à une image du montage.
    pub fn locate(&self, global: usize) -> (usize, usize) {
        let mut start = 0;
        for (i, shot) in self.shots.iter().enumerate() {
            let len = shot.frame_count();
            if global < start + len {
                return (i, global - start);
            }
            start += len;
        }
        let last = self.shots.len() - 1;
        (last, self.shots[last].frame_count() - 1)
    }
}
