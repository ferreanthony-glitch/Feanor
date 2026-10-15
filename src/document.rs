use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use eframe::egui::{pos2, vec2, Pos2, Rect, Vec2};
use serde::{Deserialize, Serialize};

use crate::audio::Pcm;
use crate::blend::BlendMode;
use crate::camera::{self, Camera, CameraKey};
use crate::guides::Guide;
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

    /// Pose `element`, qui vient d'être ajouté en dernier au dessin vectoriel : les tuiles
    /// `before` (l'état d'avant un aperçu dessiné par-dessus) sont remises, puis l'élément est
    /// dessiné sur le reste. Même image qu'un `rerender` complet (l'élément est au-dessus de
    /// tout), pour le prix du seul élément. Renvoie les tuiles touchées.
    pub fn render_on_top(&mut self, before: Vec<(usize, Option<crate::raster::Tile>)>, element: &crate::vector::Element) -> Vec<usize> {
        let mut changed: Vec<usize> = before.iter().map(|t| t.0).collect();
        for (i, t) in before {
            self.raster.tiles[i] = t;
        }
        crate::vector::render_element(&mut self.raster, element);
        changed.extend(crate::vector::tiles_in(&self.raster, element.bounds().expand(1.5)));
        changed.sort_unstable();
        changed.dedup();
        self.version += 1;
        changed
    }

    /// Refait l'image d'un dessin vectoriel dans `region` seulement : ses tuiles sont vidées, puis
    /// les éléments qui les touchent y sont redessinés, dans l'ordre. Même image qu'un `rerender`
    /// complet, pour un coût qui ne dépend que de la zone et de ce qu'elle contient (et non plus
    /// du nombre de traits du dessin). Renvoie les tuiles refaites.
    pub fn rerender_region(&mut self, region: Rect) -> Vec<usize> {
        let Some(v) = &self.vector else {
            return Vec::new();
        };
        let tiles = crate::vector::tiles_in(&self.raster, region);
        if tiles.is_empty() {
            return tiles;
        }
        // Rectangle exact des tuiles : on y redessine tout, pixel pour pixel.
        let clip = tiles.iter().fold(Rect::NOTHING, |r, &i| {
            let (x, y, w, h) = self.raster.tile_bounds(i);
            r.union(Rect::from_min_size(pos2(x as f32, y as f32), vec2(w as f32, h as f32)))
        });
        for &i in &tiles {
            self.raster.tiles[i] = None;
        }
        for e in &v.elements {
            if e.bounds().expand(1.5).intersects(clip) {
                crate::vector::render_element_clipped(&mut self.raster, e, clip);
            }
        }
        self.version += 1;
        tiles
    }
}

static NEXT_LAYER_ID: AtomicU64 = AtomicU64::new(1);

/// Panneaux copiés (12e) : les panneaux tels qu'ils étaient, une copie de leurs dessins, et la
/// taille du projet d'où ils viennent (un dessin n'a de sens qu'à cette taille).
#[derive(Clone)]
pub struct PanelClip {
    pub panels: Vec<Panel>,
    pub cels: HashMap<CelId, Cel>,
    pub size: [usize; 2],
}

/// Identifiant de calque, unique pendant la session (il n'est pas enregistré).
pub fn fresh_layer_id() -> u64 {
    NEXT_LAYER_ID.fetch_add(1, Ordering::Relaxed)
}

/// Transformation d'un calque entier : décalage, échelle et rotation autour du centre de la
/// feuille. Elle s'applique à l'affichage de tous ses dessins ; les dessins eux-mêmes ne changent
/// pas (on peut l'annuler à tout moment).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayerTransform {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    /// En degrés (sens horaire à l'écran).
    pub angle: f32,
}

impl Default for LayerTransform {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            scale: 1.0,
            angle: 0.0,
        }
    }
}

impl LayerTransform {
    pub fn is_identity(&self) -> bool {
        *self == Self::default()
    }

    /// Du dessin du calque vers la feuille, la feuille ayant son centre en `pivot`.
    pub fn affine(&self, pivot: Pos2) -> crate::vector::Affine {
        let (s, c) = self.angle.to_radians().sin_cos();
        let k = self.scale;
        let (a, b, cc, d) = (k * c, -k * s, k * s, k * c);
        crate::vector::Affine {
            a,
            b,
            c: cc,
            d,
            tx: pivot.x + self.x - (a * pivot.x + b * pivot.y),
            ty: pivot.y + self.y - (cc * pivot.x + d * pivot.y),
        }
    }

    /// Même transformation suivie d'une similitude autour du point `q` de la feuille : échelle
    /// `factor` et rotation `turn` (degrés). On reste ainsi dans « décalage + échelle + rotation ».
    pub fn around(&self, pivot: Pos2, q: Pos2, factor: f32, turn: f32) -> LayerTransform {
        let (s, c) = turn.to_radians().sin_cos();
        let v = pos2(pivot.x + self.x, pivot.y + self.y) - q;
        let moved = q + vec2(v.x * c - v.y * s, v.x * s + v.y * c) * factor;
        LayerTransform {
            x: moved.x - pivot.x,
            y: moved.y - pivot.y,
            scale: (self.scale * factor).clamp(0.01, 100.0),
            angle: self.angle + turn,
        }
    }
}

/// Où est la caméra, pour le multiplan (`Layer::transform_seen`) : son décalage depuis le centre
/// de la feuille, son avancée, et la distance de référence (pixels de la feuille).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraSeen {
    pub offset: Vec2,
    pub dolly: f32,
    pub d0: f32,
}

/// Calque de toute la scène (6f), comme un décor : chaque panneau de la scène `scene` en a un
/// exemplaire, réglé pareil, qui montre le même dessin pendant tout le panneau. `key` relie les
/// exemplaires d'un panneau à l'autre (chaque calque garde son propre `id`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneWide {
    pub key: u64,
    pub scene: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub id: u64,
    pub name: String,
    pub kind: LayerKind,
    pub visible: bool,
    pub opacity: f32,
    /// Verrouillé : on ne peut plus dessiner dessus (la timeline reste modifiable).
    pub locked: bool,
    /// Transformation du calque entier (identité par défaut).
    pub transform: LayerTransform,
    /// Profondeur (multiplan) : 0 = plan de référence ; 1 = deux fois plus loin de la caméra
    /// (défile deux fois moins vite) ; négatif = premier plan (défile plus vite).
    pub depth: f32,
    /// Une entrée par image du plan : le dessin exposé à cette image, ou rien.
    /// Plusieurs images consécutives qui pointent vers le même dessin forment une exposition
    /// (un dessin « tenu » sur 2, 3… images).
    pub frames: Vec<Option<CelId>>,
    /// Calque de toute la scène (6f) ; aucun : calque du seul panneau.
    pub scene_wide: Option<SceneWide>,
    /// Mode de fusion avec ce qui est dessous (8).
    pub blend: crate::blend::BlendMode,
    /// Pelure d'oignon du calque (13b) : quand la pelure d'oignon est allumée, ses images
    /// voisines sont montrées même s'il n'est pas le calque courant.
    pub onion: bool,
    /// Préserver la transparence (13b) : le pinceau ne colore que ce qui est déjà dessiné (le
    /// vide reste vide) ; la gomme gomme normalement.
    pub keep_alpha: bool,
    /// Pochoir (13b) : le dessin du calque limite où l'on peint sur les autres calques ;
    /// positif = seulement dans sa forme, négatif (`stencil_negative`) = seulement hors d'elle.
    pub stencil: bool,
    pub stencil_negative: bool,
    /// Position verrouillée (13b) : ni l'outil Transformer ni le déplacement en 3D ne bougent
    /// le calque, et on ne peut plus dessiner dessus.
    pub position_locked: bool,
    /// Groupe de couleur (13b) : un des `groups::COUNT` groupes du projet, 0 = « Défaut ».
    pub group: u8,
    /// Ligne réduite dans la timeline (13b) ; pour un dossier : ses calques n'y ont plus de ligne.
    pub collapsed: bool,
    /// Dossier de calques (13b-5) : pas de dessin, il range les calques dont `parent` est son id
    /// (voir `folders.rs`).
    pub folder: bool,
    /// Dossier du calque (id du calque-dossier), s'il est rangé dans un dossier.
    pub parent: Option<u64>,
    /// Pré / post-comportement (13c) : ce que le calque montre avant sa première image dessinée
    /// et après sa dernière (voir `shown_at`).
    pub pre: Behaviour,
    pub post: Behaviour,
    /// Cassure automatique de l'instance (13c) : dessiner sur une image tenue (ailleurs qu'au
    /// début de son exposition) en fait un nouveau dessin, copie du tenu, jusqu'à la fin de
    /// l'exposition.
    pub auto_break: bool,
    /// Création automatique d'image (13c) : dessiner sur une image vide crée un dessin (sinon :
    /// refusé, il faut d'abord un « Nouveau dessin »).
    pub auto_create: bool,
    /// Noms d'instance (13c) : le nom donné à un dessin du calque (montré dans la timeline).
    pub names: std::collections::BTreeMap<CelId, String>,
}

/// Pré / post-comportement d'un calque (13c, sens confirmé par Anthony) : avant la première
/// image (après la dernière), il montre rien, la boucle de ses images, un va-et-vient, ou sa
/// première (dernière) image maintenue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Behaviour {
    #[default]
    #[serde(rename = "rien")]
    Nothing,
    #[serde(rename = "boucle")]
    Loop,
    #[serde(rename = "va_et_vient")]
    PingPong,
    #[serde(rename = "maintenir")]
    Hold,
}

impl Behaviour {
    pub const ALL: [Behaviour; 4] = [Behaviour::Nothing, Behaviour::Loop, Behaviour::PingPong, Behaviour::Hold];

    pub fn label(self) -> &'static str {
        match self {
            Behaviour::Nothing => "Rien",
            Behaviour::Loop => "Boucle",
            Behaviour::PingPong => "Va-et-vient",
            Behaviour::Hold => "Maintenir",
        }
    }
}

impl Layer {
    pub fn new(name: impl Into<String>, kind: LayerKind) -> Self {
        Self {
            id: fresh_layer_id(),
            name: name.into(),
            kind,
            visible: true,
            opacity: 1.0,
            locked: false,
            transform: LayerTransform::default(),
            depth: 0.0,
            frames: Vec::new(),
            scene_wide: None,
            blend: Default::default(),
            onion: false,
            keep_alpha: false,
            stencil: false,
            stencil_negative: false,
            position_locked: false,
            group: 0,
            collapsed: false,
            folder: false,
            parent: None,
            pre: Behaviour::Nothing,
            post: Behaviour::Nothing,
            auto_break: false,
            auto_create: true,
            names: Default::default(),
        }
    }

    /// Nouveau dossier de calques (13b-5), vide.
    pub fn new_folder(name: impl Into<String>) -> Self {
        Self { folder: true, ..Self::new(name, LayerKind::Bitmap) }
    }

    /// On peut dessiner sur le calque : ni verrouillé, ni de position verrouillée (13b), et ce
    /// n'est pas un dossier.
    pub fn drawable(&self) -> bool {
        !self.locked && !self.position_locked && !self.folder
    }

    /// Plus proche que ça, le calque serait derrière la caméra.
    pub const MIN_DEPTH: f32 = -0.9;

    /// Facteur de perspective : 1 au plan de référence, ½ deux fois plus loin, 2 deux fois plus près.
    pub fn perspective(&self) -> f32 {
        1.0 / (1.0 + self.depth.max(Self::MIN_DEPTH))
    }

    /// Transformation affichée quand la caméra est en `seen` : décalée par rapport au centre, un
    /// calque lointain suit en partie la caméra (il défile moins vite), un premier plan part en
    /// sens inverse (il défile plus vite) ; avancée vers la feuille, un calque grossit d'autant
    /// plus qu'il est proche. Aucune : le calque est derrière la caméra (on ne le voit pas).
    ///
    /// Le calque de profondeur d est à la distance D(1 + d) de la caméra de référence (D : distance
    /// de référence), agrandi de (1 + d) pour qu'elle le voie tel qu'il est dessiné. Vu de la
    /// caméra avancée de z et décalée de o, puis ramené sur le plan de référence : son dessin est
    /// agrandi de s(1 + d) autour du centre et décalé de (1 − s) o, avec s = (D − z) / (D(1 + d) − z).
    pub fn transform_seen(&self, seen: &CameraSeen) -> Option<LayerTransform> {
        if self.depth == 0.0 || (seen.offset == Vec2::ZERO && seen.dolly == 0.0) {
            return Some(self.transform);
        }
        let d = self.depth.max(Self::MIN_DEPTH);
        let dist = seen.d0 * (1.0 + d) - seen.dolly;
        if dist <= seen.d0 * 0.02 {
            return None;
        }
        let s = (seen.d0 - seen.dolly) / dist;
        let k = s * (1.0 + d);
        Some(LayerTransform {
            x: self.transform.x * k + seen.offset.x * (1.0 - s),
            y: self.transform.y * k + seen.offset.y * (1.0 - s),
            scale: self.transform.scale * k,
            ..self.transform
        })
    }

    pub fn cel_at(&self, frame: usize) -> Option<CelId> {
        self.frames.get(frame).copied().flatten()
    }

    /// Dessin que l'image `frame` montre (13c) : celui de la timeline entre la première et la
    /// dernière image dessinée du calque ; avant et après, selon le pré / post-comportement
    /// (rien, la boucle des images, un va-et-vient, la première / dernière image maintenue).
    /// Les outils de dessin, eux, travaillent sur `cel_at`.
    pub fn shown_at(&self, frame: usize) -> Option<CelId> {
        let first = self.frames.iter().position(Option::is_some)?;
        let last = self.frames.iter().rposition(Option::is_some)?;
        let how = if frame < first {
            self.pre
        } else if frame > last {
            self.post
        } else {
            return self.cel_at(frame);
        };
        let len = (last - first + 1) as i64;
        let k = frame as i64 - first as i64;
        let index = match how {
            Behaviour::Nothing => return None,
            Behaviour::Hold => if frame < first { first } else { last },
            Behaviour::Loop => first + k.rem_euclid(len) as usize,
            Behaviour::PingPong if len == 1 => first,
            Behaviour::PingPong => {
                let period = 2 * len - 2;
                let m = k.rem_euclid(period);
                let offset = if m < len { m } else { period - m };
                first + offset as usize
            }
        };
        self.cel_at(index)
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
    /// Images clés de la caméra, triées (aucune : caméra au centre de la feuille).
    pub camera: Vec<CameraKey>,
    /// Repère 3D (2d) : modèle importé vu par sa propre caméra, en fil de fer dans le cadre.
    pub ref3d: Option<Ref3d>,
    /// Marques d'image (13a), triées par image, toutes après la première image du plan.
    pub marks: Vec<ImageMark>,
}

/// Marque d'image d'un plan (13a) : une image de storyboard à l'image `frame` du plan (le début
/// d'un ancien panneau, ou posée à la main), avec ses propres textes. Le plan lui-même est sa
/// première image de storyboard : ses textes sont ceux du plan. Son `id` est unique dans le
/// document, comme celui d'un plan : une note de la planche peut s'y attacher.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ImageMark {
    pub id: u64,
    pub frame: usize,
    /// Couleur : rang dans la palette des marques.
    pub color: u8,
    pub action: String,
    pub dialogue: String,
    pub notes: String,
}

/// Repère 3D d'un panneau : un modèle de `Document::models`, vu par sa caméra, dessiné en fil
/// de fer dans le cadre comme un guide (ni à l'export ni dans les vignettes ; « Dessiner dans un
/// calque » en fait un dessin).
#[derive(Clone, Debug, PartialEq)]
pub struct Ref3d {
    pub model: u64,
    pub cam: crate::view3d::Cam3d,
    pub visible: bool,
    /// Masquer les arêtes cachées par des faces plus proches.
    pub hidden_lines: bool,
    pub color: [u8; 3],
    pub opacity: f32,
}

impl Ref3d {
    pub fn new(model: u64, cam: crate::view3d::Cam3d) -> Ref3d {
        Ref3d { model, cam, visible: true, hidden_lines: true, color: [40, 150, 230], opacity: 0.8 }
    }
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

/// Clés d'une partie de `len` images qui tient sa caméra jusqu'à ses bords (13a) : ses clés dans
/// la partie, plus une clé au début et une à la fin s'il n'y en a pas (`cams` : caméras montrées
/// au début et à la fin).
fn held_keys(keys: &[CameraKey], len: usize, cams: (Camera, Camera)) -> Vec<CameraKey> {
    let mut v: Vec<CameraKey> = keys.iter().filter(|k| k.frame < len).copied().collect();
    if v.first().is_none_or(|k| k.frame > 0) {
        v.insert(0, CameraKey::new(0, cams.0));
    }
    if len > 1 && v.last().is_some_and(|k| k.frame < len - 1) {
        v.push(CameraKey::new(len - 1, cams.1));
    }
    v
}

/// Images d'un panneau sans compter ses calques de toute la scène (qui suivent sa durée).
fn own_frame_count(p: &Panel) -> usize {
    p.layers
        .iter()
        .filter(|l| l.scene_wide.is_none())
        .map(|l| l.frames.len())
        .max()
        .unwrap_or(0)
        .max(p.length)
        .max(1)
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
///
/// C'est aussi tout l'état du document en dehors des dessins et des sons : noms, textes, réglages
/// des calques et des pistes, guides, couleur du papier, cadence. Une étape d'annulation qui le
/// conserve rétablit donc tout cela.
#[derive(Clone)]
pub struct Structure {
    pub sequences: Vec<Sequence>,
    pub scenes: Vec<Scene>,
    pub panels: Vec<Panel>,
    pub audio_tracks: Vec<AudioTrack>,
    pub guides: Vec<Guide>,
    pub paper: [u8; 3],
    pub fps: u32,
    /// Notes de la planche (vue Vignettes, phase 7).
    pub board: crate::board::Board,
    /// Groupes de couleur des calques (13b).
    pub groups: Vec<crate::groups::LayerGroup>,
}

pub struct Document {
    /// Taille de la feuille de dessin (de tous les dessins), en pixels. Elle déborde du cadre
    /// pour qu'on puisse dessiner hors champ (panoramiques, mouvements de caméra).
    pub width: usize,
    pub height: usize,
    /// Cadre de la caméra : taille de l'image finale (export), centré sur la feuille.
    pub frame: [usize; 2],
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
    /// Modèles 3D importés (repères, 2d). Comme les sons, on n'en retire pas pendant la session.
    pub models: HashMap<u64, Arc<crate::model3d::Model3d>>,
    /// Guides de perspective du projet.
    pub guides: Vec<Guide>,
    /// Notes de la planche (vue Vignettes, phase 7).
    pub board: crate::board::Board,
    /// Groupes de couleur des calques (13b), toujours `groups::COUNT`.
    pub groups: Vec<crate::groups::LayerGroup>,
    next_id: u64,
}

impl Document {
    /// Projet neuf d'un cadre `frame_w`×`frame_h`, sur une feuille deux fois plus grande.
    pub fn with_frame(frame_w: usize, frame_h: usize, fps: u32) -> Self {
        let mut doc = Self::new(frame_w * 2, frame_h * 2, fps);
        doc.frame = [frame_w, frame_h];
        doc
    }

    /// Projet neuf : une séquence, une scène, un panneau ; feuille et cadre de même taille.
    pub fn new(width: usize, height: usize, fps: u32) -> Self {
        let mut doc = Self {
            width,
            height,
            frame: [width, height],
            fps,
            paper: [255, 255, 255],
            sequences: Vec::new(),
            scenes: Vec::new(),
            panels: Vec::new(),
            cels: HashMap::new(),
            audio_tracks: Vec::new(),
            sounds: HashMap::new(),
            models: HashMap::new(),
            guides: Vec::new(),
            board: Default::default(),
            groups: crate::groups::defaults(),
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
        structure: Structure,
        cels: HashMap<CelId, Cel>,
        sounds: HashMap<u64, Sound>,
    ) -> Self {
        let max_id = structure
            .panels
            .iter()
            .map(|p| p.id)
            .chain(structure.panels.iter().flat_map(|p| p.marks.iter().map(|m| m.id)))
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
            frame: [width, height],
            fps: structure.fps.max(1),
            paper: structure.paper,
            sequences: structure.sequences,
            scenes: structure.scenes,
            panels: structure.panels,
            cels,
            audio_tracks: structure.audio_tracks,
            sounds,
            models: HashMap::new(),
            guides: structure.guides,
            board: structure.board,
            groups: crate::groups::completed(structure.groups),
            next_id: max_id + 1,
        };
        doc.normalize();
        doc
    }

    /// Cadre de la caméra sur la feuille (centré), en pixels de la feuille.
    pub fn frame_rect(&self) -> Rect {
        let [fw, fh] = self.frame;
        let x0 = (self.width.saturating_sub(fw) / 2) as f32;
        let y0 = (self.height.saturating_sub(fh) / 2) as f32;
        Rect::from_min_size(pos2(x0, y0), vec2(fw as f32, fh as f32))
    }

    /// Agrandit la feuille à `width`×`height` en gardant le dessin au centre : dessins bitmap et
    /// vectoriels, guides, tout est décalé ; le cadre ne change pas de taille et reste centré.
    pub fn enlarge_stage(&mut self, width: usize, height: usize) {
        if width < self.width || height < self.height {
            return;
        }
        let (dx, dy) = ((width - self.width) / 2, (height - self.height) / 2);
        let shift = crate::vector::Affine::translate(vec2(dx as f32, dy as f32));
        for cel in self.cels.values_mut() {
            match &mut cel.vector {
                Some(d) => {
                    for e in &mut d.elements {
                        *e = e.transformed(&shift);
                    }
                    cel.raster = Raster::new(width, height);
                    cel.rerender();
                }
                None => {
                    cel.raster = cel.raster.placed_in(width, height, dx, dy);
                    cel.version += 1;
                }
            }
        }
        for g in &mut self.guides {
            for p in &mut g.points {
                p[0] += dx as f32;
                p[1] += dy as f32;
            }
        }
        for panel in &mut self.panels {
            for k in &mut panel.camera {
                k.cam = k.cam.shifted(dx as f32, dy as f32);
            }
        }
        self.width = width;
        self.height = height;
    }

    /// Centre de la feuille (pivot des transformations de calque).
    pub fn stage_center(&self) -> Pos2 {
        pos2(self.width as f32 / 2.0, self.height as f32 / 2.0)
    }

    /// Caméra par défaut : au centre de la feuille, cadre de la taille de l'image finale.
    pub fn default_camera(&self) -> Camera {
        Camera::centered(self.width, self.height)
    }

    /// Caméra du panneau `index` à son image `frame`. Un panneau sans image clé garde la caméra
    /// où le panneau d'avant (dans la scène) l'a laissée, sinon celle où le panneau d'après la
    /// prend : la caméra ne revient pas au centre en passant d'un panneau à l'autre (9c).
    pub fn panel_camera(&self, index: usize, frame: usize) -> Camera {
        let default = self.default_camera();
        let Some(p) = self.panels.get(index) else { return default };
        if !p.camera.is_empty() {
            return camera::camera_at(&p.camera, frame, default);
        }
        let (start, end) = self.scene_run(index);
        if let Some(q) = self.panels[start..index].iter().rev().find(|q| !q.camera.is_empty()) {
            return camera::camera_at(&q.camera, q.frame_count().saturating_sub(1), default);
        }
        if let Some(q) = self.panels[index + 1..end].iter().find(|q| !q.camera.is_empty()) {
            return camera::camera_at(&q.camera, 0, default);
        }
        default
    }

    /// Où est la caméra du plan `index` à l'image `frame`, pour le multiplan : décalage depuis
    /// le centre de la feuille, avancée, distance de référence.
    pub fn camera_seen(&self, index: usize, frame: usize) -> CameraSeen {
        let c = self.panel_camera(index, frame);
        CameraSeen { offset: pos2(c.x, c.y) - self.stage_center(), dolly: c.dolly, d0: camera::ref_distance(self.frame) }
    }

    pub fn structure(&self) -> Structure {
        Structure {
            sequences: self.sequences.clone(),
            scenes: self.scenes.clone(),
            panels: self.panels.clone(),
            audio_tracks: self.audio_tracks.clone(),
            guides: self.guides.clone(),
            paper: self.paper,
            fps: self.fps,
            board: self.board.clone(),
            groups: self.groups.clone(),
        }
    }

    pub fn replace_structure(&mut self, s: Structure) -> Structure {
        Structure {
            sequences: std::mem::replace(&mut self.sequences, s.sequences),
            scenes: std::mem::replace(&mut self.scenes, s.scenes),
            panels: std::mem::replace(&mut self.panels, s.panels),
            audio_tracks: std::mem::replace(&mut self.audio_tracks, s.audio_tracks),
            guides: std::mem::replace(&mut self.guides, s.guides),
            paper: std::mem::replace(&mut self.paper, s.paper),
            fps: std::mem::replace(&mut self.fps, s.fps),
            board: std::mem::replace(&mut self.board, s.board),
            groups: std::mem::replace(&mut self.groups, s.groups),
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

    /// Déplace les panneaux `ids` (gardés dans l'ordre du film) dans la scène `scene`, devant le
    /// panneau `before` (un id ; aucun : à la fin de la scène). Un panneau visé qui fait partie
    /// du déplacement cède la place au suivant. Renvoie l'indice du premier panneau déplacé, ou
    /// rien si rien n'a changé.
    pub fn move_panels(&mut self, ids: &[u64], scene: u64, before: Option<u64>) -> Option<usize> {
        if self.scene(scene).is_none() {
            return None;
        }
        let moving = |p: &Panel| ids.contains(&p.id);
        // Panneau devant lequel poser le groupe (aucun : tout à la fin du film).
        let anchor = match before.and_then(|b| self.panels.iter().position(|p| p.id == b)) {
            Some(i) => self.panels[i..].iter().find(|p| !moving(p)).map(|p| p.id),
            None => {
                let last = self.panels.iter().rposition(|p| p.scene == scene)?;
                self.panels[last + 1..].iter().find(|p| !moving(p)).map(|p| p.id)
            }
        };
        let before_order: Vec<(u64, u64)> = self.panels.iter().map(|p| (p.id, p.scene)).collect();
        let mut moved: Vec<Panel> = Vec::new();
        let mut kept: Vec<Panel> = Vec::new();
        for p in std::mem::take(&mut self.panels) {
            if moving(&p) {
                moved.push(p);
            } else {
                kept.push(p);
            }
        }
        if moved.is_empty() {
            self.panels = kept;
            return None;
        }
        let first = moved[0].id;
        for p in &mut moved {
            p.scene = scene;
        }
        let at = anchor.and_then(|a| kept.iter().position(|p| p.id == a)).unwrap_or(kept.len());
        kept.splice(at..at, moved);
        self.panels = kept;
        let index = self.normalize_keeping(first);
        let after: Vec<(u64, u64)> = self.panels.iter().map(|p| (p.id, p.scene)).collect();
        (after != before_order).then_some(index)
    }

    /// Retire les panneaux `ids` ; il en reste toujours au moins un (le dernier n'est pas retiré).
    /// Renvoie l'indice du panneau à rendre courant, ou rien si rien n'a été retiré.
    pub fn remove_panels(&mut self, ids: &[u64]) -> Option<usize> {
        let first = self.panels.iter().position(|p| ids.contains(&p.id))?;
        // Tous visés : le premier panneau du film reste.
        let spared = self.panels.iter().all(|p| ids.contains(&p.id)).then(|| self.panels[0].id);
        let before = self.panels.len();
        self.panels.retain(|p| !ids.contains(&p.id) || Some(p.id) == spared);
        if self.panels.len() == before {
            return None;
        }
        let next = self.panels[first.min(self.panels.len() - 1)].id;
        Some(self.normalize_keeping(next))
    }

    /// Coupe le panneau `index` à son image `at` : il garde [0, at) ; un nouveau panneau de la
    /// même scène, juste après, reçoit [at, fin) avec ses propres copies des dessins (modifier
    /// l'un ne touche pas l'autre). Les textes restent au premier. Renvoie l'indice du nouveau.
    pub fn split_panel(&mut self, index: usize, at: usize) -> Option<usize> {
        let n = self.panels.get(index)?.frame_count();
        if at == 0 || at >= n {
            return None;
        }
        let source = self.panels[index].clone();
        let mut copies: HashMap<CelId, CelId> = HashMap::new();
        let mut layers = Vec::with_capacity(source.layers.len());
        for layer in &source.layers {
            let mut frames = Vec::with_capacity(layer.frames.len().saturating_sub(at));
            for f in layer.frames.iter().skip(at) {
                frames.push(match *f {
                    // Un calque de toute la scène garde le dessin de la scène.
                    Some(c) if layer.scene_wide.is_some() => Some(c),
                    Some(c) => Some(match copies.get(&c) {
                        Some(&d) => d,
                        None => {
                            let d = self.duplicate_cel(c);
                            copies.insert(c, d);
                            d
                        }
                    }),
                    None => None,
                });
            }
            let mut l = Layer {
                id: fresh_layer_id(),
                frames,
                ..layer.clone()
            };
            l.trim();
            layers.push(l);
        }
        let (keys_before, keys_after) = camera::split_keys(&source.camera, at, self.default_camera());
        let second = Panel {
            id: self.alloc(),
            scene: source.scene,
            length: n - at,
            layers,
            action: String::new(),
            dialogue: String::new(),
            notes: String::new(),
            camera: keys_after,
            ref3d: source.ref3d.clone(),
            marks: Vec::new(),
        };
        let first = &mut self.panels[index];
        for l in &mut first.layers {
            l.frames.truncate(at);
            l.trim();
        }
        first.length = at;
        first.camera = keys_before;
        Some(self.insert_and_normalize(index + 1, vec![second]))
    }

    /// Réunit le panneau `index` et le suivant, s'il est dans la même scène : les images du
    /// suivant viennent après, calque par calque (même rang et même sorte, sinon un calque de
    /// plus), et les textes se suivent. L'inverse de `split_panel`.
    pub fn merge_with_next(&mut self, index: usize) -> Option<usize> {
        if index + 1 >= self.panels.len() || self.panels[index].scene != self.panels[index + 1].scene {
            return None;
        }
        // Caméras qu'ils montraient (un panneau sans clé hérite de ses voisins) : gardées.
        let (cam_a, cam_b) = (self.panel_camera(index, 0), self.panel_camera(index + 1, 0));
        let next = self.panels.remove(index + 1);
        let next_len = next.frame_count();
        let a = &mut self.panels[index];
        let a_len = a.frame_count();
        a.camera = camera::join_keys(&a.camera, a_len, cam_a, &next.camera, next_len, cam_b);
        // Les calques de toute la scène sont déjà dans le premier : seuls les autres se suivent,
        // rang pour rang parmi les calques propres au panneau.
        let own: Vec<usize> = (0..a.layers.len()).filter(|&k| a.layers[k].scene_wide.is_none()).collect();
        for (i, lb) in next.layers.into_iter().filter(|l| l.scene_wide.is_none()).enumerate() {
            if let Some(&k) = own.get(i).filter(|&&k| a.layers[k].kind == lb.kind) {
                let la = &mut a.layers[k];
                la.frames.resize(a_len, None);
                la.frames.extend(lb.frames);
                la.trim();
            } else {
                let mut l = lb;
                l.frames.splice(0..0, std::iter::repeat_n(None, a_len));
                l.trim();
                a.layers.push(l);
            }
        }
        a.length = a_len + next_len;
        for (dst, src) in [
            (&mut a.action, next.action),
            (&mut a.dialogue, next.dialogue),
            (&mut a.notes, next.notes),
        ] {
            if !src.trim().is_empty() {
                if !dst.trim().is_empty() {
                    dst.push('\n');
                }
                dst.push_str(&src);
            }
        }
        let id = a.id;
        Some(self.normalize_keeping(id))
    }

    /// Coupe la scène du panneau `index` : ce panneau et les suivants de la scène forment une
    /// nouvelle scène, juste après. Refusé sur le premier panneau d'une scène.
    pub fn split_scene(&mut self, index: usize) -> Option<usize> {
        let (start, end) = self.scene_run(index);
        if index == start {
            return None;
        }
        let scene = self.new_scene(self.sequence_of(index));
        let mut moved: Vec<Panel> = self.panels.drain(index..end).collect();
        for p in &mut moved {
            p.scene = scene;
        }
        // La nouvelle scène garde le fond de l'ancienne, en copie à elle.
        self.adopt_scene_layers(&mut moved, scene);
        let id = moved[0].id;
        self.panels.splice(index..index, moved);
        Some(self.normalize_keeping(id))
    }

    /// Réunit la scène du panneau `index` à la scène qui la précède dans la même séquence.
    pub fn join_previous_scene(&mut self, index: usize) -> Option<usize> {
        let (start, end) = self.scene_run(index);
        let prev = start.checked_sub(1)?;
        if self.sequence_of(prev) != self.sequence_of(index) {
            return None;
        }
        let target = self.panels[prev].scene;
        for p in &mut self.panels[start..end] {
            p.scene = target;
        }
        let id = self.panels[index].id;
        Some(self.normalize_keeping(id))
    }

    /// Fusionne le calque `layer` du panneau `index` dans celui du dessous. Les dessins fusionnés
    /// sont de nouveaux dessins : ceux d'avant restent intacts (l'annulation y revient). Deux
    /// calques vectoriels donnent un calque vectoriel ; sinon le résultat est bitmap. Les
    /// opacités des deux calques sont appliquées aux dessins, le résultat est à 100 %.
    pub fn merge_layer_down(&mut self, index: usize, layer: usize) -> Result<(), String> {
        let panel = self.panels.get(index).ok_or("panneau introuvable")?;
        if layer == 0 || layer >= panel.layers.len() {
            return Err("Il n'y a pas de calque en dessous.".into());
        }
        let (above, below) = (panel.layers[layer].clone(), panel.layers[layer - 1].clone());
        if above.folder || below.folder {
            return Err("Un dossier ne se fusionne pas : fusionne ses calques.".into());
        }
        if !above.visible || !below.visible {
            return Err("Affiche les deux calques avant de les fusionner.".into());
        }
        if above.locked || below.locked {
            return Err("Un des deux calques est verrouillé (Alt+L).".into());
        }
        if !above.transform.is_identity() || !below.transform.is_identity() {
            return Err("Un des deux calques est transformé : annule la transformation (Ctrl+R) avant de fusionner.".into());
        }
        if above.depth != below.depth {
            return Err("Les deux calques ne sont pas à la même profondeur.".into());
        }
        if above.scene_wide.is_some() || below.scene_wide.is_some() {
            return Err("Un calque de toute la scène ne se fusionne pas avec un autre.".into());
        }
        // Un calque du dessus en mode de fusion se fond dans les pixels : le résultat est bitmap.
        let vector = above.kind == LayerKind::Vector && below.kind == LayerKind::Vector && above.blend == BlendMode::Normal;
        let n = above.frames.len().max(below.frames.len());
        let mut made: HashMap<(Option<CelId>, Option<CelId>), Option<CelId>> = HashMap::new();
        let mut frames = Vec::with_capacity(n);
        for f in 0..n {
            let key = (below.cel_at(f), above.cel_at(f));
            let cel = match made.get(&key) {
                Some(&c) => c,
                None => {
                    let c = self.merged_cel(key, below.opacity, above.opacity, vector, above.blend);
                    made.insert(key, c);
                    c
                }
            };
            frames.push(cel);
        }
        let mut merged = Layer {
            kind: if vector { LayerKind::Vector } else { LayerKind::Bitmap },
            opacity: 1.0,
            frames,
            ..below
        };
        merged.trim();
        let p = &mut self.panels[index];
        p.layers[layer - 1] = merged;
        p.layers.remove(layer);
        Ok(())
    }

    /// Nouveau dessin : `below` puis `above` par-dessus (selon `mode`), chacun avec son opacité.
    fn merged_cel(
        &mut self,
        (below, above): (Option<CelId>, Option<CelId>),
        ob: f32,
        oa: f32,
        vector: bool,
        mode: BlendMode,
    ) -> Option<CelId> {
        if below.is_none() && above.is_none() {
            return None;
        }
        let sources: Vec<(Cel, f32, BlendMode)> = [(below, ob, BlendMode::Normal), (above, oa, mode)]
            .into_iter()
            .filter_map(|(c, o, m)| c.and_then(|c| self.cels.get(&c)).map(|cel| (cel.clone(), o.clamp(0.0, 1.0), m)))
            .collect();
        let kind = if vector { LayerKind::Vector } else { LayerKind::Bitmap };
        let id = self.new_cel(kind);
        let cel = self.cels.get_mut(&id).expect("dessin créé juste avant");
        if vector {
            let drawing = cel.vector.get_or_insert_with(VectorDrawing::default);
            for (src, o, _) in sources {
                for mut e in src.vector.map(|d| d.elements).unwrap_or_default() {
                    match &mut e {
                        crate::vector::Element::Stroke { opacity, .. }
                        | crate::vector::Element::Fill { opacity, .. }
                        | crate::vector::Element::Text { opacity, .. } => *opacity *= o,
                    }
                    drawing.elements.push(e);
                }
            }
            cel.rerender();
        } else {
            let opacity = |o: f32| (o * 255.0).round() as u8;
            for (src, o, m) in sources {
                for (idx, tile) in src.raster.tiles.iter().enumerate() {
                    let Some(tile) = tile else { continue };
                    let (ox, oy, _, _) = cel.raster.tile_bounds(idx);
                    let dst = cel.raster.tile_mut(idx);
                    for (i, (d, &s)) in dst.iter_mut().zip(tile.iter()).enumerate() {
                        if s.a() != 0 {
                            let at = (ox + i % crate::raster::TILE, oy + i / crate::raster::TILE);
                            *d = crate::blend::blend_over(*d, s, opacity(o), None, m, at);
                        }
                    }
                }
            }
        }
        Some(id)
    }

    /// Ajoute un dessin existant (collage) sous un nouvel identifiant.
    pub fn add_cel(&mut self, mut cel: Cel) -> CelId {
        cel.version = 0;
        let id = self.alloc();
        self.cels.insert(id, cel);
        id
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
            camera: Vec::new(),
            ref3d: None,
            marks: Vec::new(),
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
        let mut copies: Vec<Panel> = (start..end)
            .map(|i| {
                let mut p = self.duplicate_panel(i);
                p.scene = scene;
                p
            })
            .collect();
        self.adopt_scene_layers(&mut copies, scene);
        copies
    }

    /// Copie complète d'un panneau : ses dessins sont dupliqués, modifier l'un ne touche pas l'autre
    /// (sauf ceux des calques de toute la scène : la copie est dans la même scène).
    pub fn duplicate_panel(&mut self, index: usize) -> Panel {
        let mut panel = self.panels[index].clone();
        panel.id = self.alloc();
        for m in &mut panel.marks {
            m.id = self.alloc();
        }
        let mut copies: HashMap<CelId, CelId> = HashMap::new();
        for layer in &mut panel.layers {
            layer.id = fresh_layer_id();
            if layer.scene_wide.is_some() {
                continue;
            }
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

    // ---------------------------------------------------------------- copier, coller des panneaux (12e)

    /// Copie des panneaux `ids`, dans l'ordre du film, avec une copie de leurs dessins (les
    /// modifier ensuite ne change pas ce qui sera collé). Rien si aucun n'existe.
    pub fn copy_panels(&self, ids: &[u64]) -> Option<PanelClip> {
        let panels: Vec<Panel> = self.panels.iter().filter(|p| ids.contains(&p.id)).cloned().collect();
        if panels.is_empty() {
            return None;
        }
        let cels = panels
            .iter()
            .flat_map(|p| &p.layers)
            .filter(|l| l.scene_wide.is_none())
            .flat_map(|l| l.frames.iter().flatten())
            .filter_map(|id| self.cels.get(id).map(|c| (*id, c.clone())))
            .collect();
        Some(PanelClip { panels, cels, size: [self.width, self.height] })
    }

    /// Colle les panneaux copiés juste après le panneau `index`, dans sa scène, chacun avec ses
    /// propres dessins. Un calque de toute la scène suit la règle d'un panneau déplacé : collé
    /// dans sa scène, il en garde le dessin ; ailleurs, il est retiré et le panneau prend le fond
    /// de sa nouvelle scène (`sync_scene_layers`). Renvoie l'indice du premier panneau collé, ou
    /// rien si la copie vient d'un projet d'une autre taille.
    pub fn paste_panels(&mut self, clip: &PanelClip, index: usize) -> Option<usize> {
        if clip.panels.is_empty() || clip.size != [self.width, self.height] {
            return None;
        }
        let scene = self.panels[index.min(self.panels.len() - 1)].scene;
        let mut copies: HashMap<CelId, CelId> = HashMap::new();
        let mut pasted = Vec::with_capacity(clip.panels.len());
        for source in &clip.panels {
            let mut panel = source.clone();
            panel.id = self.alloc();
            for m in &mut panel.marks {
                m.id = self.alloc();
            }
            panel.scene = scene;
            for layer in &mut panel.layers {
                layer.id = fresh_layer_id();
                if layer.scene_wide.is_some() {
                    continue;
                }
                for f in layer.frames.iter_mut() {
                    *f = f.and_then(|old| match copies.get(&old) {
                        Some(&c) => Some(c),
                        None => {
                            let c = self.add_cel(clip.cels.get(&old)?.clone());
                            copies.insert(old, c);
                            Some(c)
                        }
                    });
                }
            }
            pasted.push(panel);
        }
        Some(self.insert_and_normalize(index + 1, pasted))
    }

    // ---------------------------------------------------------------- plans (13a)

    /// Vrai si chaque scène a exactement un panneau : un plan (13a).
    pub fn is_plans(&self) -> bool {
        self.scenes.iter().all(|s| self.panels.iter().filter(|p| p.scene == s.id).count() == 1)
    }

    /// Plans (13a) : chaque scène devient un seul plan, ses panneaux mis bout à bout (le début
    /// de chacun devient une marque d'image qui garde ses textes) ; les calques de toute la scène
    /// deviennent des calques ordinaires, leur dessin tenu sur tout le plan. Renvoie ce qui n'a
    /// pas pu être gardé tel quel.
    pub fn into_plans(&mut self) -> Vec<String> {
        self.flatten_scene_layers();
        let mut warnings = Vec::new();
        let mut i = 0;
        while i < self.panels.len() {
            while i + 1 < self.panels.len() && self.panels[i + 1].scene == self.panels[i].scene {
                warnings.extend(self.append_next(i));
            }
            i += 1;
        }
        self.normalize();
        warnings
    }

    /// Les calques de toute la scène deviennent des calques ordinaires de chaque panneau, leur
    /// dessin tenu sur tout le panneau.
    fn flatten_scene_layers(&mut self) {
        for p in &mut self.panels {
            let n = own_frame_count(p);
            for l in &mut p.layers {
                if l.scene_wide.take().is_some() {
                    let cel = l.frames.iter().flatten().next().copied();
                    l.frames = cel.map_or(Vec::new(), |c| vec![Some(c); n]);
                }
            }
        }
    }

    /// Met le plan (ou panneau) `index + 1` au bout du plan `index` (voir `append_plan`).
    fn append_next(&mut self, index: usize) -> Option<String> {
        let last = |d: &Document, i: usize| d.panels[i].frame_count() - 1;
        let cams_a = (self.panel_camera(index, 0), self.panel_camera(index, last(self, index)));
        let cams_b = (self.panel_camera(index + 1, 0), self.panel_camera(index + 1, last(self, index + 1)));
        let b = self.panels.remove(index + 1);
        self.append_plan(index, b, cams_a, cams_b)
    }

    /// Met le plan `b` (déjà retiré du document) au bout du plan `index`.
    /// - Ses images viennent après, calque par calque : un calque de `b` va avec le calque de
    ///   même nom (sinon de même rang) s'ils sont réglés pareil (sorte, visibilité, opacité,
    ///   mode, transformation, profondeur), pour que rien ne change à l'écran ; sinon il
    ///   s'ajoute, juste au-dessus du calque où va celui qui est sous lui.
    /// - La caméra ne bouge pas aux jointures : des clés tiennent la caméra à la fin de l'un et
    ///   au début de l'autre (`cams_a`, `cams_b` : caméras montrées au début et à la fin).
    /// - Le début de `b` devient une marque d'image avec ses textes ; ses marques suivent.
    /// - Les notes de la planche attachées à `b` s'attachent à cette marque.
    ///
    /// Renvoie un avertissement si le repère 3D de `b` n'a pas pu être gardé.
    fn append_plan(&mut self, index: usize, b: Panel, cams_a: (Camera, Camera), cams_b: (Camera, Camera)) -> Option<String> {
        let mark_id = self.alloc();
        let label = self.scene_label(self.panels[index].scene);
        let a_len = self.panels[index].frame_count();
        let b_len = b.frame_count();
        let a = &mut self.panels[index];

        // Caméra : chaque partie tient sa caméra jusqu'à ses bords.
        if !(a.camera.is_empty() && b.camera.is_empty() && cams_a == cams_b) {
            let mut keys = held_keys(&a.camera, a_len, cams_a);
            keys.extend(held_keys(&b.camera, b_len, cams_b).into_iter().map(|k| CameraKey { frame: k.frame + a_len, ..k }));
            a.camera = keys;
        }

        // Calques : par nom, puis par rang, s'ils sont réglés pareil.
        let same = |x: &Layer, y: &Layer| {
            x.kind == y.kind
                && x.visible == y.visible
                && x.opacity == y.opacity
                && x.blend == y.blend
                && x.transform == y.transform
                && x.depth == y.depth
        };
        let mut used = vec![false; a.layers.len()];
        let mut target: Vec<Option<usize>> = Vec::with_capacity(b.layers.len());
        for lb in &b.layers {
            let k = (0..a.layers.len()).find(|&k| !used[k] && a.layers[k].name == lb.name && same(&a.layers[k], lb));
            if let Some(k) = k {
                used[k] = true;
            }
            target.push(k);
        }
        for (j, lb) in b.layers.iter().enumerate() {
            if target[j].is_none() && j < a.layers.len() && !used[j] && same(&a.layers[j], lb) {
                used[j] = true;
                target[j] = Some(j);
            }
        }
        for l in &mut a.layers {
            l.frames.resize(a_len, None);
        }
        // Calque d'arrivée de chaque calque de `b`, par son id (les ajouts décalent les rangs).
        let mut dest: Vec<u64> = Vec::with_capacity(b.layers.len());
        for (j, mut lb) in b.layers.into_iter().enumerate() {
            lb.frames.truncate(b_len);
            match target[j] {
                Some(k) => {
                    a.layers[k].frames.extend(lb.frames);
                    dest.push(a.layers[k].id);
                }
                None => {
                    let at = match j.checked_sub(1).and_then(|p| dest.get(p)) {
                        Some(below) => a.layers.iter().position(|l| l.id == *below).map_or(a.layers.len(), |k| k + 1),
                        None => 0,
                    };
                    lb.frames.splice(0..0, std::iter::repeat_n(None, a_len));
                    dest.push(lb.id);
                    a.layers.insert(at, lb);
                }
            }
        }
        for l in &mut a.layers {
            l.trim();
        }

        // Le début de `b` : une marque d'image avec ses textes ; puis ses propres marques.
        a.marks.push(ImageMark { id: mark_id, frame: a_len, color: 0, action: b.action, dialogue: b.dialogue, notes: b.notes });
        a.marks.extend(b.marks.into_iter().map(|m| ImageMark { frame: m.frame + a_len, ..m }));
        a.length = a_len + b_len;
        let warning = match (&a.ref3d, b.ref3d) {
            (None, r) => {
                a.ref3d = r;
                None
            }
            (Some(x), Some(y)) if *x != y => Some(format!(
                "Plan {label} : il garde son repère 3D ; celui de la partie qui commence à l'image {} n'est pas repris.",
                a_len + 1
            )),
            _ => None,
        };
        self.reanchor_notes(b.id, mark_id);
        warning
    }

    /// Les notes de la planche attachées à `from` (un plan ou une marque) s'attachent à `to`.
    fn reanchor_notes(&mut self, from: u64, to: u64) {
        for s in &mut self.board.strokes {
            if s.anchor == Some(from) {
                s.anchor = Some(to);
            }
        }
        for t in &mut self.board.texts {
            if t.anchor == Some(from) {
                t.anchor = Some(to);
            }
        }
    }

    /// Réunit le plan `index` et le suivant de la même séquence : les images du suivant viennent
    /// après (voir `append_plan`). Rien si le suivant est dans une autre séquence.
    pub fn join_next_plan(&mut self, index: usize) -> Option<usize> {
        if index + 1 >= self.panels.len() || self.sequence_of(index) != self.sequence_of(index + 1) {
            return None;
        }
        self.append_next(index);
        let id = self.panels[index].id;
        Some(self.normalize_keeping(id))
    }

    /// Coupe le plan `index` à son image `at` : il garde [0, at) ; un nouveau plan, juste après
    /// dans la même séquence, reçoit [at, fin) avec ses propres copies des dessins. Les marques
    /// d'après suivent ; une marque à l'image `at` devient le début du nouveau plan (ses textes
    /// deviennent ceux du plan). L'inverse de `join_next_plan`. Renvoie l'indice du nouveau.
    pub fn split_plan(&mut self, index: usize, at: usize) -> Option<usize> {
        let n = self.panels.get(index)?.frame_count();
        if at == 0 || at >= n {
            return None;
        }
        let first_id = self.panels[index].id;
        let marks = std::mem::take(&mut self.panels[index].marks);
        let second = self.split_panel(index, at)?;
        let scene = self.new_scene(self.sequence_of(second));
        let (before, after): (Vec<ImageMark>, Vec<ImageMark>) = marks.into_iter().partition(|m| m.frame < at);
        if let Some(first) = self.panels.iter_mut().find(|p| p.id == first_id) {
            first.marks = before;
        }
        let mut start = None;
        let p = &mut self.panels[second];
        p.scene = scene;
        for m in after {
            if m.frame == at {
                (p.action, p.dialogue, p.notes) = (m.action, m.dialogue, m.notes);
                start = Some(m.id);
            } else {
                p.marks.push(ImageMark { frame: m.frame - at, ..m });
            }
        }
        let id = p.id;
        if let Some(mark) = start {
            self.reanchor_notes(mark, id);
        }
        Some(self.normalize_keeping(id))
    }

    /// Déplace les plans `ids` (gardés dans l'ordre du film) devant le plan `before` (aucun : à la
    /// fin). Chaque plan reste un plan ; il garde sa séquence s'il retombe à côté d'elle, sinon il
    /// rejoint celle du plan devant lequel on le pose (ou du dernier, tout à la fin). Renvoie
    /// l'indice du premier plan déplacé, ou rien si rien n'a changé.
    pub fn move_plans(&mut self, ids: &[u64], before: Option<u64>) -> Option<usize> {
        let order = |d: &Document| -> Vec<(u64, u64)> { d.panels.iter().map(|p| (p.id, d.scene(p.scene).map_or(0, |s| s.sequence))).collect() };
        let was = order(self);
        // Le plan devant lequel poser : `before`, ou le premier qui ne bouge pas après lui.
        let anchor = before.and_then(|b| {
            let i = self.panels.iter().position(|p| p.id == b)?;
            self.panels[i..].iter().find(|p| !ids.contains(&p.id)).map(|p| p.id)
        });
        let (moved, mut kept): (Vec<Panel>, Vec<Panel>) = std::mem::take(&mut self.panels).into_iter().partition(|p| ids.contains(&p.id));
        if moved.is_empty() {
            self.panels = kept;
            return None;
        }
        let at = anchor.and_then(|b| kept.iter().position(|p| p.id == b)).unwrap_or(kept.len());
        let seq_of = |d: &Document, p: Option<&Panel>| p.and_then(|p| d.scene(p.scene)).map(|s| s.sequence);
        let left = seq_of(self, at.checked_sub(1).and_then(|i| kept.get(i)));
        let right = seq_of(self, kept.get(at));
        for p in &moved {
            let own = self.scene(p.scene).map(|s| s.sequence);
            let target = if own.is_some() && (own == left || own == right) { own } else { right.or(left) };
            if let (Some(seq), Some(s)) = (target, self.scene_mut(p.scene)) {
                s.sequence = seq;
            }
        }
        let first = moved[0].id;
        kept.splice(at..at, moved);
        self.panels = kept;
        let index = self.normalize_keeping(first);
        (order(self) != was).then_some(index)
    }

    /// Colle les plans copiés juste après le plan `index`, chacun dans sa propre scène de la
    /// même séquence (voir `paste_panels`). Renvoie l'indice du premier plan collé.
    pub fn paste_plans(&mut self, clip: &PanelClip, index: usize) -> Option<usize> {
        let first = self.paste_panels(clip, index)?;
        let seq = self.sequence_of(first);
        let ids: Vec<u64> = self.panels[first..first + clip.panels.len()].iter().map(|p| p.id).collect();
        for id in &ids {
            let scene = self.new_scene(seq);
            if let Some(p) = self.panels.iter_mut().find(|p| p.id == *id) {
                p.scene = scene;
            }
        }
        Some(self.normalize_keeping(ids[0]))
    }

    /// Pose une marque d'image (une image de storyboard) à l'image `frame` du plan `index`, ou la
    /// retire s'il y en a déjà une. Rien à la première image : c'est déjà le début du plan.
    /// Renvoie vrai si une marque a été posée, faux si elle a été retirée.
    pub fn toggle_mark(&mut self, index: usize, frame: usize) -> Option<bool> {
        if frame == 0 || index >= self.panels.len() {
            return None;
        }
        if let Some(k) = self.panels[index].marks.iter().position(|m| m.frame == frame) {
            self.panels[index].marks.remove(k);
            return Some(false);
        }
        let id = self.alloc();
        let marks = &mut self.panels[index].marks;
        let at = marks.iter().position(|m| m.frame > frame).unwrap_or(marks.len());
        marks.insert(at, ImageMark { id, frame, ..Default::default() });
        Some(true)
    }

    /// Image de storyboard à l'image `frame` du plan `index` : la marque d'image à cette image ou
    /// avant, sinon le plan lui-même (`None`).
    pub fn board_mark_at(&self, index: usize, frame: usize) -> Option<&ImageMark> {
        self.panels.get(index)?.marks.iter().rev().find(|m| m.frame <= frame)
    }

    // ---------------------------------------------------------------- calques de toute la scène (6f)

    /// Accorde les calques de toute la scène : chaque panneau de la scène a son exemplaire,
    /// réglé comme le modèle, qui montre le même dessin pendant tout le panneau. Le modèle est
    /// l'exemplaire du panneau `cur` s'il en a un (c'est lui qu'on modifie), sinon le premier
    /// trouvé ; son dessin est celui de l'image `frame` (un nouveau dessin remplace donc l'ancien
    /// partout). Un exemplaire égaré dans une autre scène (panneau déplacé, scènes réunies) est
    /// retiré : le panneau prend le fond de sa nouvelle scène. Vrai si quelque chose a changé.
    pub fn sync_scene_layers(&mut self, cur: usize, frame: usize) -> bool {
        let mut changed = false;
        for p in &mut self.panels {
            let scene = p.scene;
            let before = p.layers.len();
            p.layers.retain(|l| l.scene_wide.is_none_or(|w| w.scene == scene));
            if p.layers.len() != before {
                changed = true;
                if p.layers.is_empty() {
                    p.layers.push(Layer::new("Dessin", LayerKind::Vector));
                }
            }
        }
        // Modèles : l'exemplaire, son rang et son dessin.
        let mut models: Vec<(Layer, usize, Option<CelId>)> = Vec::new();
        for (pi, p) in self.panels.iter().enumerate() {
            for (k, l) in p.layers.iter().enumerate() {
                let Some(w) = l.scene_wide else { continue };
                let first = l.frames.iter().flatten().next().copied();
                let cel = if pi == cur { l.cel_at(frame).or(first) } else { first };
                match models.iter_mut().find(|m| m.0.scene_wide.is_some_and(|v| v.key == w.key)) {
                    Some(m) if pi == cur => *m = (l.clone(), k, cel),
                    Some(_) => {}
                    None => models.push((l.clone(), k, cel)),
                }
            }
        }
        for p in &mut self.panels {
            let n = own_frame_count(p);
            for (model, rank, cel) in &models {
                let Some(w) = model.scene_wide.filter(|w| w.scene == p.scene) else { continue };
                let frames = cel.map_or(Vec::new(), |c| vec![Some(c); n]);
                match p.layers.iter().position(|l| l.scene_wide == Some(w)) {
                    Some(k) => {
                        let want = Layer { id: p.layers[k].id, frames, ..model.clone() };
                        if p.layers[k] != want {
                            p.layers[k] = want;
                            changed = true;
                        }
                    }
                    None => {
                        let at = (*rank).min(p.layers.len());
                        p.layers.insert(at, Layer { id: fresh_layer_id(), frames, ..model.clone() });
                        changed = true;
                    }
                }
            }
        }
        changed
    }

    /// Fait du calque `layer` du panneau `index` un calque de toute sa scène (son dessin à l'image
    /// `frame` sera celui de toute la scène), ou le rend à son seul panneau : les autres panneaux
    /// gardent alors leur exemplaire, chacun avec sa propre copie du dessin.
    pub fn set_scene_wide(&mut self, index: usize, layer: usize, frame: usize, on: bool) {
        let Some(l) = self.panels.get(index).and_then(|p| p.layers.get(layer)) else { return };
        match (on, l.scene_wide) {
            (true, None) => {
                let w = SceneWide { key: fresh_layer_id(), scene: self.panels[index].scene };
                self.panels[index].layers[layer].scene_wide = Some(w);
                self.sync_scene_layers(index, frame);
            }
            (false, Some(w)) => {
                for pi in 0..self.panels.len() {
                    let Some(k) = self.panels[pi].layers.iter().position(|l| l.scene_wide == Some(w)) else { continue };
                    self.panels[pi].layers[k].scene_wide = None;
                    if pi == index {
                        continue;
                    }
                    let mut copies: HashMap<CelId, CelId> = HashMap::new();
                    let mut frames = std::mem::take(&mut self.panels[pi].layers[k].frames);
                    for f in frames.iter_mut().flatten() {
                        *f = match copies.get(f) {
                            Some(&c) => c,
                            None => {
                                let c = self.duplicate_cel(*f);
                                copies.insert(*f, c);
                                c
                            }
                        };
                    }
                    self.panels[pi].layers[k].frames = frames;
                }
            }
            _ => {}
        }
    }

    /// Retire de tous les panneaux le calque de toute la scène `w` (un panneau qui n'aurait plus
    /// de calque en reçoit un vide).
    pub fn remove_scene_layer(&mut self, w: SceneWide) {
        for p in &mut self.panels {
            p.layers.retain(|l| l.scene_wide != Some(w));
            if p.layers.is_empty() {
                p.layers.push(Layer::new("Dessin", LayerKind::Vector));
            }
        }
    }

    /// Les calques de toute la scène des panneaux `panels`, passés dans la scène `scene` (scène
    /// coupée ou copiée), deviennent les siens : nouvelle clé et leur propre copie du dessin
    /// (modifier le fond d'une scène ne touche pas l'autre).
    fn adopt_scene_layers(&mut self, panels: &mut [Panel], scene: u64) {
        let mut keys: HashMap<u64, u64> = HashMap::new();
        let mut cels: HashMap<CelId, CelId> = HashMap::new();
        for p in panels.iter_mut() {
            for l in &mut p.layers {
                let Some(w) = l.scene_wide.filter(|w| w.scene != scene) else { continue };
                let key = *keys.entry(w.key).or_insert_with(fresh_layer_id);
                l.scene_wide = Some(SceneWide { key, scene });
                for f in l.frames.iter_mut().flatten() {
                    *f = match cels.get(f) {
                        Some(&c) => c,
                        None => {
                            let c = self.duplicate_cel(*f);
                            cels.insert(*f, c);
                            c
                        }
                    };
                }
            }
        }
    }

    pub fn new_cel(&mut self, kind: LayerKind) -> CelId {
        let id = self.alloc();
        self.cels.insert(id, Cel::new(self.width, self.height, kind));
        id
    }

    /// Nouveau dessin : `source` retourné (13c) de gauche à droite (`h`) et / ou de haut en bas
    /// (`v`), autour du centre de la feuille.
    pub fn flipped_cel(&mut self, source: CelId, h: bool, v: bool) -> CelId {
        let id = self.duplicate_cel(source);
        let (w, hh) = (self.width, self.height);
        let cel = self.cels.get_mut(&id).expect("dessin copié juste avant");
        match cel.vector.as_mut() {
            Some(d) => {
                let flip = crate::vector::Affine {
                    a: if h { -1.0 } else { 1.0 },
                    d: if v { -1.0 } else { 1.0 },
                    tx: if h { w as f32 } else { 0.0 },
                    ty: if v { hh as f32 } else { 0.0 },
                    ..crate::vector::Affine::IDENTITY
                };
                for e in &mut d.elements {
                    *e = e.transformed(&flip);
                }
                cel.raster = Raster::new(w, hh);
                cel.rerender();
            }
            None => {
                let old = std::mem::replace(&mut cel.raster, Raster::new(w, hh));
                for (idx, tile) in old.tiles.iter().enumerate() {
                    let Some(tile) = tile else { continue };
                    let (ox, oy, _, _) = old.tile_bounds(idx);
                    for (i, &px) in tile.iter().enumerate() {
                        let (x, y) = (ox + i % crate::raster::TILE, oy + i / crate::raster::TILE);
                        if px.a() == 0 || x >= w || y >= hh {
                            continue;
                        }
                        let (nx, ny) = (if h { w - 1 - x } else { x }, if v { hh - 1 - y } else { y });
                        let t = cel.raster.tile_index_of(nx, ny);
                        cel.raster.tile_mut(t)[(ny % crate::raster::TILE) * crate::raster::TILE + nx % crate::raster::TILE] = px;
                    }
                }
                cel.version += 1;
            }
        }
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

    /// Ajoute un modèle 3D importé ; renvoie son identifiant.
    pub fn add_model(&mut self, name: String, mesh: crate::model3d::Mesh3d) -> u64 {
        let id = self.alloc();
        self.models.insert(id, Arc::new(crate::model3d::Model3d::new(id, name, mesh)));
        id
    }

    /// Remet un modèle relu d'un projet, avec son identifiant.
    pub fn insert_model(&mut self, model: crate::model3d::Model3d) {
        self.next_id = self.next_id.max(model.id + 1);
        self.models.insert(model.id, Arc::new(model));
    }

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
