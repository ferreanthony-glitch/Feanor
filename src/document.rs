use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use eframe::egui::{pos2, vec2, Pos2, Rect, Vec2};
use serde::{Deserialize, Serialize};

use crate::audio::Pcm;
use crate::camera::{self, Camera, CameraKey};
use crate::fusionnoms::JoinOpts;
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
    /// Strate Fond (`fond.rs`) : ce qu'on a peint dans le fond de ce dessin, sous le trait, et où
    /// on l'a coupé ; rien tant qu'on n'y a pas touché.
    pub fond: Option<Box<Strate>>,
}

/// La strate Fond d'un dessin (`fond.rs`).
#[derive(Clone)]
pub struct Strate {
    /// Ce qu'on a peint dans le fond (un dessin bitmap ; sa version augmente aussi quand la coupe
    /// change).
    pub peinture: Cel,
    /// Où le fond est coupé, à la gomme dans la strate Fond (l'opacité dit combien).
    pub coupe: Raster,
}

impl Strate {
    pub fn new(width: usize, height: usize) -> Self {
        Strate { peinture: Cel::bitmap(Raster::new(width, height)), coupe: Raster::new(width, height) }
    }

    pub fn is_empty(&self) -> bool {
        self.peinture.is_empty() && self.coupe.tiles.iter().all(Option::is_none)
    }
}

impl Cel {
    pub fn new(width: usize, height: usize, kind: LayerKind) -> Self {
        Self {
            raster: Raster::new(width, height),
            vector: (kind == LayerKind::Vector).then(VectorDrawing::default),
            version: 0,
            fond: None,
        }
    }

    /// Un dessin bitmap fait de `raster`.
    pub fn bitmap(raster: Raster) -> Self {
        Self { raster, vector: None, version: 0, fond: None }
    }

    pub fn is_empty(&self) -> bool {
        match &self.vector {
            Some(v) => v.elements.is_empty(),
            None => self.raster.is_blank(),
        }
    }

    /// Les images du dessin : sa feuille, et la peinture et la coupe de sa strate Fond.
    fn rasters(&self) -> impl Iterator<Item = &Raster> {
        std::iter::once(&self.raster).chain(self.fond.iter().flat_map(|f| [&f.peinture.raster, &f.coupe]))
    }

    fn rasters_mut(&mut self) -> impl Iterator<Item = &mut Raster> {
        std::iter::once(&mut self.raster).chain(self.fond.iter_mut().flat_map(|f| [&mut f.peinture.raster, &mut f.coupe]))
    }

    /// Range les pixels du dessin et les lâche tout de suite (un dessin qu'on vient de lire : `Tile`, pixels rangés). Sans effet sur ce
    /// que le dessin montre : la première lecture rouvre les tuiles.
    pub fn geler(&mut self) -> usize {
        self.rasters_mut().map(Raster::geler).sum()
    }

    /// Fait les tampons des tuiles ouvertes du dessin (d'un fil de fond, sur une copie du dessin) ; rend le nombre de tuiles faites.
    pub fn ranger_les_tuiles(&self) -> usize {
        self.rasters().map(Raster::ranger_les_tuiles).sum()
    }

    /// Lâche les pixels des tuiles dont le tampon est fait et que le dessin est seul à tenir ; rend leur nombre.
    pub fn lacher_les_pixels(&mut self) -> usize {
        self.rasters_mut().map(Raster::lacher_les_pixels).sum()
    }

    /// Rouvre les pixels de toutes les tuiles du dessin (d'un fil de fond, sur une copie : il sera prêt quand on le regardera).
    pub fn rouvrir(&self) {
        self.rasters().for_each(Raster::rouvrir);
    }

    /// Reste-t-il des tuiles dont les pixels sont en mémoire ?
    pub fn a_des_tuiles_ouvertes(&self) -> bool {
        self.rasters().any(Raster::a_des_tuiles_ouvertes)
    }

    /// Reste-t-il des tuiles dont le tampon est à faire ?
    pub fn a_des_tuiles_a_ranger(&self) -> bool {
        self.rasters().any(Raster::a_des_tuiles_a_ranger)
    }

    /// Reste-t-il des tuiles dont les pixels pourraient partir tout de suite ?
    pub fn a_des_tuiles_lachables(&self) -> bool {
        self.rasters().any(Raster::a_des_tuiles_lachables)
    }

    /// Des copies des images du dessin (les tuiles se partagent : rien n'est recopié) pour qu'un fil de fond en fasse les tampons.
    pub fn copies_des_images(&self) -> Vec<Raster> {
        self.rasters().cloned().collect()
    }

    /// Octets que les tuiles du dessin tiennent en mémoire (pixels ouverts et tampons).
    pub fn octets_en_memoire(&self) -> usize {
        self.rasters().map(Raster::octets_en_memoire).sum()
    }

    /// (tuiles de la feuille dont les pixels sont en mémoire, tuiles de la feuille en tout) ; la strate Fond compte aussi.
    pub fn tuiles_ouvertes(&self) -> (usize, usize) {
        self.rasters().map(Raster::tuiles_ouvertes).fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1))
    }

    /// Refait l'image d'un dessin vectoriel. Renvoie les tuiles qui ont pu changer.
    pub fn rerender(&mut self) -> Vec<usize> {
        let Some(v) = &self.vector else {
            return Vec::new();
        };
        let fresh = v.render(self.raster.width, self.raster.height);
        // (Toile infinie : les tuiles hors de la feuille comptent aussi.)
        let changed = self.raster.ids_either(&fresh);
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
            self.raster.replace_slot(i, t);
        }
        crate::vector::render_element(&mut self.raster, element);
        changed.extend(crate::vector::tiles_in_world(&self.raster, element.bounds().expand(1.5)));
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
        let tiles = crate::vector::tiles_in_world(&self.raster, region);
        if tiles.is_empty() {
            return tiles;
        }
        // Rectangle exact des tuiles (entières, hors de la feuille comprises) : on y redessine tout,
        // pixel pour pixel.
        let clip = tiles.iter().fold(Rect::NOTHING, |r, &i| {
            let (x, y) = self.raster.tile_origin(i);
            r.union(Rect::from_min_size(pos2(x as f32, y as f32), vec2(crate::raster::TILE as f32, crate::raster::TILE as f32)))
        });
        for &i in &tiles {
            self.raster.replace_slot(i, None);
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

/// Panneaux copiés (12e) : les panneaux tels qu'ils étaient, une copie de leurs dessins, et d'où ils viennent (cadre,
/// feuille et cadence du projet : un dessin n'a de sens qu'à sa taille, `adaptation.rs` les adapte à celle du projet qui
/// les reçoit).
#[derive(Clone)]
pub struct PanelClip {
    pub panels: Vec<Panel>,
    pub cels: HashMap<CelId, Cel>,
    pub origine: crate::adaptation::Origine,
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
    /// Étirement en largeur et en hauteur, dans le repère du calque, en plus de `scale` (1 : aucun ;
    /// Anthony, 2026-10-18 : « l'étirement latéral »).
    pub stretch_x: f32,
    pub stretch_y: f32,
    /// Inclinaison de la carte dans l'Espace 3D (`espace3d.rs`), en degrés : rotation autour de l'axe
    /// horizontal (le haut vient vers la caméra quand elle est positive) puis de l'axe vertical (le bord
    /// droit s'éloigne quand elle est positive). 0 : la carte est à plat, parallèle à la feuille.
    pub tilt_x: f32,
    pub tilt_y: f32,
    /// Écart de profondeur, ajouté à `Layer::depth` : ce que porte la clé d'un calque animé (un calque fixe
    /// n'a jamais d'écart, il change `depth` : une seule source).
    pub dz: f32,
}

impl Default for LayerTransform {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            scale: 1.0,
            angle: 0.0,
            stretch_x: 1.0,
            stretch_y: 1.0,
            tilt_x: 0.0,
            tilt_y: 0.0,
            dz: 0.0,
        }
    }
}

/// Inclinaison la plus grande d'une carte (au-delà, elle est vue par la tranche).
pub const MAX_TILT: f32 = 85.0;

impl LayerTransform {
    pub fn is_identity(&self) -> bool {
        *self == Self::default()
    }

    /// La carte est inclinée dans l'espace (elle ne se pose plus par une affinité de la feuille).
    pub fn is_tilted(&self) -> bool {
        self.tilt_x != 0.0 || self.tilt_y != 0.0
    }

    /// Une inclinaison ramenée entre −`MAX_TILT` et `MAX_TILT` (une valeur abîmée : 0).
    pub fn clamp_tilt(v: f32) -> f32 {
        if v.is_finite() { v.clamp(-MAX_TILT, MAX_TILT) } else { 0.0 }
    }

    /// Un étirement ramené entre 1 % et 100 fois, son signe gardé (négatif : calque retourné).
    pub fn clamp_stretch(v: f32) -> f32 {
        if v < 0.0 { -(-v).clamp(0.01, 100.0) } else { v.clamp(0.01, 100.0) }
    }

    /// Du dessin du calque vers la feuille, la feuille ayant son centre en `pivot` : étirement,
    /// échelle, puis rotation.
    pub fn affine(&self, pivot: Pos2) -> crate::vector::Affine {
        let (s, c) = self.angle.to_radians().sin_cos();
        let (kx, ky) = (self.scale * self.stretch_x, self.scale * self.stretch_y);
        let (a, b, cc, d) = (kx * c, -ky * s, kx * s, ky * c);
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
            ..*self
        }
    }
}

/// Où est la caméra, pour le multiplan (`Layer::transform_seen`) : son décalage depuis le centre
/// de la feuille, son avancée, et la distance de référence (pixels de la feuille). Pour un calque accroché à la caméra
/// (`accroche.rs`, partie 141) : son grossissement (l'objectif et l'avancée) et sa rotation, en degrés.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraSeen {
    pub offset: Vec2,
    pub dolly: f32,
    pub d0: f32,
    pub grossissement: f32,
    pub angle: f32,
}

/// Calque de toute la scène (6f), comme un décor : chaque panneau de la scène `scene` en a un
/// exemplaire, réglé pareil, avec les mêmes images (ses clés, depuis le 2026-10-17 ; avant : un
/// seul dessin tenu sur tout le panneau). `key` relie les
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
    /// Transformation du calque entier (identité par défaut). Un calque animé (`pos_keys`) prend la
    /// pose de ses clés à chaque image : voir `Layer::pose`.
    pub transform: LayerTransform,
    /// Clés de position (`poskeys.rs`, Anthony 2026-10-20) : la pose du calque à certaines images du plan,
    /// triées par image ; entre deux clés elle va de l'une à l'autre. Vide : le calque a sa pose fixe.
    pub pos_keys: Vec<crate::poskeys::PosKey>,
    /// Trajectoire (`trajectoire.rs`, Anthony 2026-11-07) : le chemin dessiné et ses marques, qui posent les clés de position du
    /// calque ; rien : les clés sont à la main (ou il n'y en a pas). Elle ne se voit qu'avec l'outil Trajectoire.
    pub trajectoire: Option<crate::trajectoire::Trajectoire>,
    /// Clés d'opacité (`opacitecles.rs`, Anthony 2026-10-29) : l'opacité du calque à certaines images du plan, triées par image ; entre
    /// deux clés elle va de l'une à l'autre. Vide : le calque a son opacité fixe (`opacity`).
    pub opacity_keys: Vec<crate::opacitecles::OpKey>,
    /// Couleur unie du calque (`couleurunie.rs`, « Color card ») : un fond d'une seule couleur sur toute la feuille, rendu sous les
    /// dessins du calque, sans pixels stockés. Aucune : calque ordinaire.
    pub unie: Option<[u8; 3]>,
    /// Profondeur (multiplan) : 0 = plan de référence ; 1 = deux fois plus loin de la caméra
    /// (défile deux fois moins vite) ; négatif = premier plan (défile plus vite).
    pub depth: f32,
    /// Une entrée par image du plan : le dessin exposé à cette image, ou rien.
    /// Plusieurs images consécutives qui pointent vers le même dessin forment une exposition
    /// (un dessin « tenu » sur 2, 3… images).
    pub frames: Vec<Option<CelId>>,
    /// Calque de toute la scène (6f) ; aucun : calque du seul panneau.
    pub scene_wide: Option<SceneWide>,
    /// La planche-mère (`partage.rs`, Anthony 2026-11-08) : la clé que partagent les exemplaires d'un même calque dans plusieurs plans
    /// (mêmes dessins, chacun sous la caméra de son plan) ; aucune : calque du seul plan.
    pub partage: Option<u64>,
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
    /// Marques de couleur (13d, les « marques d'image » de TVPaint) : dessin → rang de sa couleur
    /// dans `marks::COLORS`.
    pub color_marks: std::collections::BTreeMap<CelId, u8>,
    /// Rôles des images (13g-2) : image du calque → rôle, même sur une image vide.
    pub roles: crate::roles::Roles,
    /// Pile d'effets (13i) : appliqués à l'affichage et à l'export, dans l'ordre.
    pub fx: Vec<crate::fx::Effect>,
    /// Interrupteur général de la pile d'effets (bouton « fx » de la ligne du calque, avec l'œil et le
    /// cadenas) : éteint, aucun effet du calque ne s'applique, mais la pile reste telle quelle (réglages,
    /// clés, effets cochés) et revient en rallumant. Allumé d'office.
    pub fx_enabled: bool,
    /// Calque vidéo (13k) : il montre les images de cette vidéo, pas de dessins.
    pub video: Option<crate::video::VideoClip>,
    /// Fond sous le trait (`fond.rs`) : chaque dessin du calque a un fond déduit de son trait,
    /// sous lui ; rien : pas de fond.
    pub fond: Option<crate::fond::FondReglages>,
    /// Calque d'effet (`calqueffet.rs`) : pas de dessin, une pile d'effets pour tout ce qui est
    /// dessous (ajustement) ou du texte sur l'image finale (incrustation) ; ses images sont ses
    /// expositions.
    pub effet: Option<crate::calqueffet::CalqueEffet>,
    /// Calque de couleur (`calquecouleur.rs`) : la couleur sous le trait d'un autre calque. Ses
    /// images suivent celles de ce calque de trait ; on y colorie sans abîmer le trait.
    pub couleur: Option<crate::calquecouleur::CalqueCouleur>,
    /// Le calque Décor du plan (`decor.rs`) : un par plan, fait par les brosses du décor des Vignettes, tout en bas du plan ; son
    /// dessin est tenu sur toutes les images du plan, donc partagé par ses cases.
    pub decor: bool,
    /// Le personnage de ce calque (`calquespastilles.rs`) : le numéro du personnage (`Personnage::id`) dont la pastille a fait le calque,
    /// pour le retrouver même si le personnage est renommé. Vectoriel, nommé comme lui, avec l'aplat qu'il a. Aucun : calque ordinaire.
    pub personnage: Option<u64>,
    /// Calque d'annotation (`calqueannot.rs`) : des notes de réalisateur sur le plan, vectorielles, peintes par-dessus tout à l'écran et
    /// jamais dans un export ni dans les vignettes (sauf si on le demande). Un calque vectoriel ordinaire pour le reste.
    pub annotation: bool,
    /// Accroché à la caméra (`accroche.rs`, Anthony 2026-11-11) : le calque garde sa place dans l'image filmée, quoi que fasse la caméra
    /// (déplacement, zoom, rotation, avancée). Sa pose se lit alors dans le cadre de la caméra, et le multiplan ne le fait plus défiler.
    pub accroche: bool,
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
            pos_keys: Vec::new(),
            trajectoire: None,
            opacity_keys: Vec::new(),
            unie: None,
            depth: 0.0,
            frames: Vec::new(),
            scene_wide: None,
            partage: None,
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
            color_marks: Default::default(),
            roles: Default::default(),
            fx: Vec::new(),
            fx_enabled: true,
            video: None,
            fond: None,
            effet: None,
            couleur: None,
            decor: false,
            personnage: None,
            annotation: false,
            accroche: false,
        }
    }

    /// Les images de ce calque suivent celles d'un autre (calque de couleur) : on n'y pose, déplace
    /// ni supprime de clé soi-même.
    pub fn keys_follow_line(&self) -> bool {
        self.couleur.as_ref().is_some_and(|c| c.est_lie())
    }

    /// Nouveau dossier de calques (13b-5), vide.
    pub fn new_folder(name: impl Into<String>) -> Self {
        Self { folder: true, ..Self::new(name, LayerKind::Bitmap) }
    }

    /// On peut dessiner sur le calque : ni verrouillé, ni de position verrouillée (13b), et ce
    /// n'est pas un dossier.
    pub fn drawable(&self) -> bool {
        !self.locked && !self.position_locked && !self.folder && self.effet.is_none()
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
        self.pose_seen(self.transform, seen)
    }

    /// Comme `transform_seen`, pour un calque qui peut être animé : la pose est celle de l'image `frame`
    /// (`pose`). Tout ce qui fabrique l'image d'un plan, d'une vignette ou d'un export passe par là.
    pub fn transform_seen_at(&self, seen: &CameraSeen, frame: usize) -> Option<LayerTransform> {
        self.pose_seen(self.pose(frame), seen)
    }

    /// Profondeur du calque à l'image `frame` : `depth` plus l'écart de sa pose (clés), au plus près
    /// `MIN_DEPTH`.
    pub fn depth_at(&self, frame: usize) -> f32 {
        (self.depth + self.pose(frame).dz).max(Self::MIN_DEPTH)
    }

    /// Règle la profondeur du calque à l'image `frame` : sa profondeur de base s'il est fixe, sinon l'écart
    /// de la clé de cette image (créée au besoin).
    pub fn set_depth_at(&mut self, frame: usize, depth: f32) {
        let depth = depth.max(Self::MIN_DEPTH);
        if self.pos_keys.is_empty() {
            self.depth = depth;
        } else {
            let mut t = self.pose(frame);
            t.dz = depth - self.depth;
            self.set_pose(frame, t);
        }
    }

    /// Un calque fixe n'a pas d'écart de profondeur : il passe dans `depth` (une seule source).
    pub fn fold_dz(&mut self) {
        self.depth = (self.depth + self.transform.dz).max(Self::MIN_DEPTH);
        self.transform.dz = 0.0;
    }

    /// La pose du calque à l'image `frame` du plan : celle que donnent ses clés de position, sinon sa
    /// transformation fixe (`transform`).
    pub fn pose(&self, frame: usize) -> LayerTransform {
        crate::poskeys::pose_at(&self.pos_keys, frame, self.transform)
    }

    /// Le calque a des clés de position : il est animé.
    pub fn is_animated(&self) -> bool {
        !self.pos_keys.is_empty()
    }

    /// Donne au calque la pose `t` à l'image `frame` : sa transformation fixe s'il n'a pas de clé, sinon
    /// la clé de cette image (créée au besoin, avec les adoucis de celle qu'elle remplace).
    pub fn set_pose(&mut self, frame: usize, t: LayerTransform) {
        if self.pos_keys.is_empty() {
            self.transform = t;
            self.fold_dz();
        } else {
            crate::poskeys::set_key(&mut self.pos_keys, frame, t);
        }
    }

    /// `t`, la pose du calque, vue par la caméra `seen` (voir `transform_seen`). Pour une carte inclinée
    /// (`LayerTransform::is_tilted`), c'est la pose à plat : l'inclinaison se pose par un maillage
    /// (`espace3d.rs`), pas par une affinité.
    pub(crate) fn pose_seen(&self, t: LayerTransform, seen: &CameraSeen) -> Option<LayerTransform> {
        // Accroché à la caméra (`accroche.rs`) : sa pose se lit dans le cadre de la caméra, où qu'elle soit ; pas de multiplan.
        if self.accroche {
            return Some(seen.accrochee(t));
        }
        let depth = self.depth + t.dz;
        if depth == 0.0 || (seen.offset == Vec2::ZERO && seen.dolly == 0.0) {
            return Some(t);
        }
        let d = depth.max(Self::MIN_DEPTH);
        let dist = seen.d0 * (1.0 + d) - seen.dolly;
        if dist <= seen.d0 * 0.02 {
            return None;
        }
        let s = (seen.d0 - seen.dolly) / dist;
        let k = s * (1.0 + d);
        Some(LayerTransform {
            x: t.x * k + seen.offset.x * (1.0 - s),
            y: t.y * k + seen.offset.y * (1.0 - s),
            scale: t.scale * k,
            ..t
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
        self.shift_frames(at, count, false);
    }

    /// `n` images insérées à l'image `at` (`removed` : retirées de `at` à `at + n`) : les rôles
    /// (13g-2), les clés d'effets (13i), les clés de position et d'opacité des images d'après suivent, et la
    /// trajectoire (partie 119 : son départ, sa fin et ses marques restent sur leurs images) aussi.
    pub fn shift_frames(&mut self, at: usize, n: usize, removed: bool) {
        crate::roles::shift(&mut self.roles, at, n, removed);
        crate::fx::shift_keys(&mut self.fx, at, n, removed);
        crate::poskeys::shift_keys(&mut self.pos_keys, at, n, removed);
        crate::opacitecles::shift_keys(&mut self.opacity_keys, at, n, removed);
        if let Some(t) = self.trajectoire.as_mut() {
            t.decaler_images(at, n, removed);
        }
    }

    /// L'opacité du calque à l'image `frame` du plan : celle que donnent ses clés d'opacité, sinon son opacité fixe.
    pub fn opacity_at(&self, frame: usize) -> f32 {
        crate::opacitecles::value_at(&self.opacity_keys, frame, self.opacity).clamp(0.0, 1.0)
    }

    /// Le calque a des clés d'opacité : son opacité est animée.
    pub fn is_opacity_animated(&self) -> bool {
        !self.opacity_keys.is_empty()
    }

    /// Donne au calque l'opacité `v` à l'image `frame` : son opacité fixe s'il n'a pas de clé, sinon la clé de cette image (créée au
    /// besoin, avec les adoucis de celle qu'elle remplace).
    pub fn set_opacity_at(&mut self, frame: usize, v: f32) {
        let v = crate::opacitecles::clamp(v);
        if self.opacity_keys.is_empty() {
            self.opacity = v;
        } else {
            crate::opacitecles::set_key(&mut self.opacity_keys, frame, v);
        }
    }

    /// Pose une clé d'opacité à l'image `frame` avec la valeur `v` (la première clé rend le calque animé).
    pub fn put_opacity_key(&mut self, frame: usize, v: f32) {
        crate::opacitecles::set_key(&mut self.opacity_keys, frame, v);
    }

    /// Remplace toutes les clés d'opacité (import) : `keys` est une liste de (image, opacité) ; une clé par image, triées.
    pub fn set_opacity_keys(&mut self, keys: &[(usize, f32)]) {
        self.opacity_keys.clear();
        for &(f, v) in keys {
            crate::opacitecles::set_key(&mut self.opacity_keys, f, v);
        }
    }

    /// Réunion de deux plans : les clés d'opacité du même calque de l'autre plan suivent les siennes, décalées de `a_len` images.
    pub fn join_opacity_keys(&mut self, a_len: usize, keys: &[crate::opacitecles::OpKey], base: f32, b_len: usize) {
        self.opacity_keys = crate::opacitecles::join_keys(&self.opacity_keys, a_len, self.opacity, keys, b_len, base);
    }

    /// Réunion de deux plans : les clés de position du même calque de l'autre plan (`keys`, `base` : sa
    /// pose fixe, `b_len` : sa durée) suivent les siennes, décalées de `a_len` images.
    pub fn join_pos_keys(&mut self, a_len: usize, keys: &[crate::poskeys::PosKey], base: LayerTransform, b_len: usize) {
        self.pos_keys = crate::poskeys::join_keys(&self.pos_keys, a_len, self.transform, keys, b_len, base);
    }

    /// Ajoute à la fin du calque, à partir de l'image `at`, les images `frames` d'un autre calque
    /// et ses rôles (réunion de plans).
    pub fn append_at(&mut self, at: usize, frames: Vec<Option<CelId>>, roles: &crate::roles::Roles) {
        self.frames.resize(at, None);
        self.frames.extend(frames);
        self.roles.retain(|f, _| *f < at);
        self.roles.extend(roles.iter().map(|(f, r)| (f + at, *r)));
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
    /// Numéro propre en numérotation verrouillée (`numerotation.rs`) ; vide = numéro du rang.
    pub number: String,
}

/// Une scène (un plan, au sens du découpage) : un groupe de panneaux qui se suivent.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub id: u64,
    /// Nom ou numéro choisi ; vide = numéro automatique.
    pub name: String,
    /// Numéro propre en numérotation verrouillée (`numerotation.rs`) ; vide = numéro du rang.
    pub number: String,
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
    /// Postérisation temporelle de la caméra (« saccade ») : rien dans les clés, tout à l'évaluation.
    pub saccade: camera::Saccade,
    /// Effet de caméra vivant (tremblement, explosion, zoom…), calculé à l'évaluation de la caméra, sans clé
    /// (`camfx::Vivant`, partie 129) ; « Convertir en clés » le cuit dans `camera`.
    pub effet_camera: Option<crate::camfx::Vivant>,
    /// Marques d'image (13a), triées par image, toutes après la première image du plan.
    pub marks: Vec<ImageMark>,
    /// Signets, points d'entrée et de sortie (13d).
    pub cues: Cues,
    /// Couleur de repère du plan : un rang dans `marks::COLORS` (rien : la couleur d'origine).
    pub color: Option<u8>,
    /// **Plan en espace** (`docs/ESPACE-LIBRE.md`, format 54) : la caméra du plan est libre dans l'Espace 3D et les cartes
    /// se cachent selon leur distance. Étape A1 : la case seulement, l'image n'en dépend pas encore. Faux : un plan ordinaire.
    pub espace: bool,
}

/// Après une copie de calques (de nouveaux id de calques, `old_ids` dans le même ordre, et des
/// copies de dessins `cels`) : les calques rangés suivent leur dossier copié (13b-5) et les
/// noms d'instance suivent leurs dessins copiés (13c).
pub fn relink_copies(layers: &mut [Layer], old_ids: &[u64], cels: &HashMap<CelId, CelId>) {
    let ids: HashMap<u64, u64> = old_ids.iter().zip(layers.iter()).map(|(o, l)| (*o, l.id)).collect();
    for l in layers.iter_mut() {
        l.parent = l.parent.map(|p| ids.get(&p).copied().unwrap_or(p));
        l.names = l.names.iter().map(|(c, n)| (cels.get(c).copied().unwrap_or(*c), n.clone())).collect();
        l.color_marks = l.color_marks.iter().map(|(c, k)| (cels.get(c).copied().unwrap_or(*c), *k)).collect();
        // Calque de couleur copié avec son trait : il colore la copie, avec les copies des dessins.
        if let Some(c) = &mut l.couleur {
            c.relink(&ids, cels);
        }
    }
}

/// Propriétés du projet (13e) : ce que montre la fenêtre « Propriétés du projet ». Les dates
/// sont en secondes depuis 1970 (0 : inconnue).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Properties {
    pub title: String,
    pub info: String,
    pub author: String,
    pub created: u64,
    pub modified: u64,
    pub notes: String,
    pub copyright: String,
    /// Couleur que prend un fond sous le trait qu'on allume (`fond.rs`) ; rien : blanc.
    pub fond: Option<[u8; 3]>,
    /// Le nom de la série ou du projet de production (`{projet}` du modèle de noms, colonnes de la liste des plans) ; vide : le titre.
    pub serie: String,
    /// Le numéro (ou le nom) de l'épisode (`{episode}`) ; vide : aucun.
    pub episode: String,
    /// Le modèle de noms des plans (`nommage.rs`) ; vide : le modèle d'origine (« SQ01_PL012A »).
    pub modele_noms: String,
    /// La durée cible du film (de l'épisode), en millisecondes ; 0 : aucune (`budget.rs`).
    pub budget_ms: u32,
    /// La durée cible de chaque séquence (numéro de la séquence → millisecondes) ; aucune entrée : pas de cible.
    pub budgets_sequences: std::collections::BTreeMap<u64, u32>,
}

/// Couleur B du damier d'un projet neuf.
pub const DEFAULT_PAPER_B: [u8; 3] = [204, 204, 204];

/// Arrière-plan du projet (13e), comme celui de TVPaint : aucun (transparent, montré noir),
/// une couleur (le papier, qui sort à l'export), ou un damier des couleurs A et B pour voir les
/// transparences (l'export est alors transparent).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BackgroundKind {
    #[serde(rename = "aucun")]
    None,
    #[default]
    #[serde(rename = "couleur")]
    Color,
    #[serde(rename = "damier")]
    Checker,
}

impl BackgroundKind {
    pub const ALL: [BackgroundKind; 3] = [BackgroundKind::None, BackgroundKind::Color, BackgroundKind::Checker];

    pub fn label(self) -> &'static str {
        match self {
            BackgroundKind::None => "Aucun (transparent)",
            BackgroundKind::Color => "Couleur",
            BackgroundKind::Checker => "Damier",
        }
    }
}

/// Repères d'un plan (13d) : signets (images repérées, triées) et points d'entrée / de sortie
/// (la lecture en boucle reste entre eux).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cues {
    pub bookmarks: Vec<usize>,
    pub in_point: Option<usize>,
    pub out_point: Option<usize>,
    /// Notes dessinées sur la timeline du plan (13e) : vues pendant la lecture, jamais
    /// exportées.
    pub notes: Vec<TimeNote>,
}

/// Une note dessinée sur la timeline (13e) : un trait dont chaque point est en images (x, la
/// position dans le temps) et en hauteur de la bande des notes (y, de 0 en haut à 1 en bas).
#[derive(Clone, Debug, PartialEq)]
pub struct TimeNote {
    /// 0 : bleu, 1 : rouge, 2 : noir.
    pub color: u8,
    /// Épaisseur, en points d'écran.
    pub width: f32,
    pub points: Vec<[f32; 2]>,
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

/// Deux calques sont réglés pareil : ils peuvent se réunir à la jonction de deux plans sans que rien ne change à l'écran.
pub(crate) fn layers_same(x: &Layer, y: &Layer) -> bool {
    x.kind == y.kind
        // (Un dossier ne se réunit qu'à un dossier.)
        && x.folder == y.folder
        && x.decor == y.decor
        && x.annotation == y.annotation
        && x.visible == y.visible
        && x.opacity == y.opacity
        && x.blend == y.blend
        && x.transform == y.transform
        // (Un calque animé ne se réunit pas à un autre : il reste un calque à part.)
        && x.pos_keys.is_empty()
        && y.pos_keys.is_empty()
        && x.opacity_keys.is_empty()
        && y.opacity_keys.is_empty()
        && x.unie == y.unie
        && x.depth == y.depth
        // (Un calque de couleur ne se réunit qu'à un calque de couleur, un calque d'effet, de vidéo ou de toute la scène qu'à
        // un calque de sa sorte : un calque bitmap ordinaire réuni à un calque de couleur voyait ses images refaites d'après le
        // trait — ses dessins n'étaient plus nulle part, et l'enregistrement suivant les effaçait. Revue du 2026-10-23.)
        && x.couleur.is_some() == y.couleur.is_some()
        && x.effet.is_none() == y.effet.is_none()
        && x.video.is_none() == y.video.is_none()
        && x.scene_wide == y.scene_wide
}

/// Le calque de `a` où va chaque calque de `b` à la jonction de deux plans (rien : il s'ajoute). Par nom, puis par rang, s'ils sont réglés
/// pareil (`layers_same`) ; `apart` : les calques de `b` (par numéro) qui ne se réunissent pas par rang (renommés ou gardés à part,
/// `fusionnoms.rs`).
pub(crate) fn layer_targets(a: &[Layer], b: &[Layer], apart: &std::collections::HashSet<u64>) -> Vec<Option<usize>> {
    let mut used = vec![false; a.len()];
    let mut target: Vec<Option<usize>> = Vec::with_capacity(b.len());
    for lb in b {
        let k = (0..a.len()).find(|&k| !used[k] && a[k].name == lb.name && layers_same(&a[k], lb));
        if let Some(k) = k {
            used[k] = true;
        }
        target.push(k);
    }
    for (j, lb) in b.iter().enumerate() {
        if target[j].is_none() && j < a.len() && !used[j] && !apart.contains(&lb.id) && layers_same(&a[j], lb) {
            used[j] = true;
            target[j] = Some(j);
        }
    }
    // Un calque de couleur de `b` ne se réunit à un calque de couleur de `a` que si son trait va lui aussi dans le trait de
    // celui-ci (ou s'ils sont libres tous les deux) : sinon la couleur réunie suivait le trait de `a`, vide sur la suite, et les
    // dessins de couleur de `b` n'étaient plus montrés par aucun calque — l'enregistrement les effaçait (revue du 2026-10-24).
    // Ajouté tel quel, il garde son trait (remis plus bas sur le calque où celui-ci est allé).
    for j in 0..b.len() {
        let (Some(k), Some(cb)) = (target[j], b[j].couleur.as_ref()) else { continue };
        let Some(ca) = a[k].couleur.as_ref() else { continue };
        let accordes = match (ca.trait_id, cb.trait_id) {
            (0, 0) => true,
            (0, _) | (_, 0) => false,
            (ta, tb) => b.iter().position(|l| l.id == tb).and_then(|jt| target[jt]).is_some_and(|kt| a[kt].id == ta),
        };
        if !accordes {
            target[j] = None;
        }
    }
    target
}

/// Images d'un panneau sans compter ses calques de toute la scène ni ses calques partagés (`partage.rs`), qui suivent sa durée.
pub(crate) fn own_frame_count(p: &Panel) -> usize {
    p.layers
        .iter()
        .filter(|l| l.scene_wide.is_none() && l.partage.is_none())
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
    /// Fondus, volume, clés de volume, égaliseur, écho (`audiofx.rs`).
    pub fx: crate::audiofx::ClipFx,
}

impl SoundClip {
    /// Un extrait sans effet.
    pub fn new(id: u64, sound: u64, start: usize, offset: usize, length: usize) -> SoundClip {
        SoundClip { id, sound, start, offset, length, fx: Default::default() }
    }

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
    /// Solo : quand une piste au moins l'est, seules les pistes en solo s'entendent (`audiofx::audible`).
    pub solo: bool,
    /// Gain linéaire (1 = inchangé).
    pub volume: f32,
    pub clips: Vec<SoundClip>,
    /// Verrouillée (13e) : ses extraits ne se déplacent, ne se coupent ni ne se suppriment plus.
    pub locked: bool,
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
    /// Numéro de la première image (13d, « image de départ ») : 1 par défaut.
    pub start_frame: usize,
    /// Arrière-plan (13e) et couleur B du damier (la couleur A est `paper`).
    pub background: BackgroundKind,
    pub paper_b: [u8; 3],
    /// Volume général du son (13e, la piste maîtresse).
    pub master_volume: f32,
    /// Storyboard verrouillé (13e) et propriétés du projet.
    pub storyboard_locked: bool,
    /// Numérotation des plans verrouillée (`numerotation.rs`) : chaque plan garde son numéro.
    pub numbering_locked: bool,
    pub properties: Properties,
    /// Commentaires sur la timeline (`commentaires.rs`) : une étape d'annulation les rétablit avec
    /// le reste.
    pub comments: Vec<crate::commentaires::Comment>,
    /// Ce que le rognage a mis de côté, par numéro de plan (`rognage.rs`) : l'annulation le
    /// rétablit avec le reste.
    pub reserves: std::collections::BTreeMap<u64, crate::rognage::Reserve>,
    /// Transitions du montage (`transitions.rs`) : l'annulation les rétablit avec le reste.
    pub transitions: Vec<crate::transitions::Transition>,
    /// Références visuelles (`references.rs`) : ajouter, retirer, déplacer s'annulent avec le reste.
    pub references: Vec<crate::references::Reference>,
    /// Personnages du projet (`personnages.rs`) : ajouter, modifier, ranger s'annulent avec le reste.
    pub personnages: Vec<crate::personnages::Personnage>,
    /// Versions des plans (`planvers.rs`), par numéro de plan : créer, passer, restaurer, supprimer une
    /// version s'annulent avec le reste.
    pub versions: std::collections::BTreeMap<u64, crate::planvers::VersionsDuPlan>,
    /// Les variantes rangées des séquences (`variantes.rs`) : changer de variante, en créer ou en supprimer s'annulent
    /// avec le reste. Partagées (`Arc`) : une étape d'annulation ne les recopie pas.
    pub variantes: crate::variantes::Variantes,
    /// La corbeille (corbeille.rs) : jeter et restaurer s'annulent avec le reste, sans double.
    pub corbeille: crate::corbeille::Corbeille,
    /// Les plans annulés (`planannule.rs`) : annuler et rétablir s'annulent avec le reste.
    pub annules: crate::planannule::Annules,
    /// Annotations de la vue Vignettes (`annotations.rs`) : tracer, effacer, tout effacer s'annulent avec le reste.
    pub annotations: crate::annotations::Annotations,
    /// Le scénario gardé dans le projet (`scenariogarde.rs`) : importer et créer les plans depuis lui s'annulent avec le reste.
    /// Partagé (`Arc`) : une étape d'annulation ne le recopie pas.
    pub scenario: Option<std::sync::Arc<crate::scenariogarde::Gardee>>,
    /// Les expositions des calques des plans, rangées en suites (`ranger_les_expositions`) : un instantané d'une vieille étape
    /// d'annulation les tient ainsi (une entrée par image et par calque pesait 16 octets : 4,5 Mo pour 450 plans, 830 Mo pour
    /// 183 étapes). Hors de cet état, `None` et les `frames` des calques sont en place.
    pub frames_rangees: Option<Box<FramesRangees>>,
}

/// Les `frames` de chaque calque de chaque plan d'une `Structure`, en suites (dessin ou rien, nombre d'images de suite).
#[derive(Clone)]
pub struct FramesRangees(Vec<Vec<Vec<(Option<CelId>, u32)>>>);

impl FramesRangees {
    /// Les dessins cités (un par suite, avec des répétitions d'un calque à l'autre).
    pub fn dessins(&self) -> impl Iterator<Item = CelId> + '_ {
        self.0.iter().flatten().flatten().filter_map(|(id, _)| *id)
    }
}

impl Structure {
    /// Range les expositions des calques en suites (rien ne se perd : `rouvrir_les_expositions` les rend telles quelles). Faux si elles
    /// l'étaient déjà.
    pub fn ranger_les_expositions(&mut self) -> bool {
        if self.frames_rangees.is_some() {
            return false;
        }
        let mut tout = Vec::with_capacity(self.panels.len());
        for p in &mut self.panels {
            let mut calques = Vec::with_capacity(p.layers.len());
            for l in &mut p.layers {
                let mut suites: Vec<(Option<CelId>, u32)> = Vec::new();
                for f in std::mem::take(&mut l.frames) {
                    match suites.last_mut() {
                        Some((id, n)) if *id == f && *n < u32::MAX => *n += 1,
                        _ => suites.push((f, 1)),
                    }
                }
                suites.shrink_to_fit();
                calques.push(suites);
            }
            tout.push(calques);
        }
        self.frames_rangees = Some(Box::new(FramesRangees(tout)));
        true
    }

    /// Rend leurs `frames` aux calques (sans effet si elles y sont).
    pub fn rouvrir_les_expositions(&mut self) {
        let Some(r) = self.frames_rangees.take() else { return };
        for (p, calques) in self.panels.iter_mut().zip(r.0) {
            for (l, suites) in p.layers.iter_mut().zip(calques) {
                l.frames = suites.into_iter().flat_map(|(id, n)| std::iter::repeat_n(id, n as usize)).collect();
            }
        }
    }

    /// Les dessins que montrent les plans de cet état (rangé ou non) (essais).
    #[cfg(test)]
    pub fn dessins_des_plans(&self) -> Vec<CelId> {
        match &self.frames_rangees {
            Some(r) => r.dessins().collect(),
            None => self.panels.iter().flat_map(|p| p.layers.iter()).flat_map(|l| l.frames.iter().flatten().copied()).collect(),
        }
    }
}

/// Plus grand côté de la feuille, en pixels : la texture minimale que toute carte graphique garantit (wgpu : 8 192).
pub const MAX_SHEET_SIDE: usize = 8192;

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
    /// Guides de perspective du projet.
    pub guides: Vec<Guide>,
    /// Notes de la planche (vue Vignettes, phase 7).
    pub board: crate::board::Board,
    /// Groupes de couleur des calques (13b), toujours `groups::COUNT`.
    pub groups: Vec<crate::groups::LayerGroup>,
    /// Numéro affiché de la première image de chaque plan (13d, « image de départ du projet »).
    pub start_frame: usize,
    /// Arrière-plan (13e) : aucun, la couleur `paper` (couleur A), ou un damier de `paper` et
    /// `paper_b`.
    pub background: BackgroundKind,
    pub paper_b: [u8; 3],
    /// Volume général du son (13e, la piste maîtresse de TVPaint) : multiplie celui des pistes.
    pub master_volume: f32,
    /// Storyboard verrouillé (13e) : l'ordre, le nombre et la durée des plans ne changent plus.
    pub storyboard_locked: bool,
    /// Numérotation des plans verrouillée (`numerotation.rs`) : chaque plan garde son numéro.
    pub numbering_locked: bool,
    /// Propriétés du projet (13e).
    pub properties: Properties,
    /// Commentaires sur la timeline, rangés par plan (`commentaires.rs`) ; enregistrés à côté du
    /// manifeste, dans `commentaires.json`.
    pub comments: Vec<crate::commentaires::Comment>,
    /// Contenu rogné hors des plans, rangé par numéro de plan (`rognage.rs`) ; enregistré à côté du
    /// manifeste, dans `rognages.json`.
    pub reserves: std::collections::BTreeMap<u64, crate::rognage::Reserve>,
    /// Transitions du montage, rangées par coupe (`transitions.rs`) ; enregistrées à côté du
    /// manifeste, dans `transitions.json`.
    pub transitions: Vec<crate::transitions::Transition>,
    /// Sons proposés par la lecture du storyboard (`indications.rs`) ; enregistrés à côté du
    /// manifeste, dans `sons_proposes.json`. Hors de l'annulation.
    pub sons_proposes: crate::indications::SonsProposes,
    /// Références visuelles du projet (`references.rs`) ; enregistrées à côté du manifeste, dans
    /// `references.json`, leurs fichiers dans `references/`.
    pub references: Vec<crate::references::Reference>,
    /// Personnages du projet (`personnages.rs`) ; enregistrés à côté du manifeste, dans
    /// `personnages.json`. Dans `Structure` : l'annulation les rétablit avec le reste.
    pub personnages: Vec<crate::personnages::Personnage>,
    /// Les envois (`envois.rs`) : l'état de chaque plan quand le film est parti ; enregistrés à côté du
    /// manifeste (`envois.json`, `versions/`). Hors de l'annulation : marquer un envoi est un
    /// fait, pas une retouche du film.
    pub envois: crate::envois::Envois,
    /// Les autres versions de chaque plan (`planvers.rs`), rangées par numéro de plan ; enregistrées à côté
    /// du manifeste, dans `plans_versions.json`, leurs dessins dans `dessins/`. Le plan lui-même
    /// (`panels`) est toujours la version active.
    pub versions: std::collections::BTreeMap<u64, crate::planvers::VersionsDuPlan>,
    /// Les variantes rangées des séquences (`variantes.rs`), par numéro de séquence ; enregistrées à côté du
    /// manifeste, dans `variantes.json`, leurs dessins dans `dessins/`. Le jeu de plans de la séquence
    /// (`scenes`, `panels`) est toujours la variante choisie.
    pub variantes: crate::variantes::Variantes,
    /// La corbeille du projet (corbeille.rs) : ce que les gestes du montage retirent du film, récupérable.
    pub corbeille: crate::corbeille::Corbeille,
    /// Les plans annulés (`planannule.rs`) : sortis du film, montrés barrés sur la planche, rétablis tels quels.
    pub annules: crate::planannule::Annules,
    /// Des dessins de variantes rangées manquaient à l'ouverture (projet partagé dont les fichiers n'étaient pas tous
    /// arrivés) : le fichier des variantes n'est pas réécrit, et on n'en change pas. Hors de l'annulation.
    pub variantes_partielles: bool,
    /// Les annotations de la vue Vignettes (`annotations.rs`) ; enregistrées à côté du manifeste, dans
    /// `annotations.json`. Dans `Structure` : l'annulation les rétablit avec le reste. Aucun export ne les lit.
    pub annotations: crate::annotations::Annotations,
    /// Le scénario gardé dans le projet (`scenariogarde.rs`), enregistré à côté du manifeste dans `scenario.json`. Dans `Structure`.
    pub scenario: Option<std::sync::Arc<crate::scenariogarde::Gardee>>,
    /// « Report du décor » (Préférences > Vignettes) : en réunissant deux plans, le décor du premier se tient sur la suite
    /// (`decor.rs`). Un réglage de l'application que `App::structure_op_porte` pose avant chaque opération : il n'est pas dans le projet.
    pub report_decor: bool,
    /// « Calque Décor : vectoriel / bitmap » (Préférences > Vignettes) : la sorte du calque Décor que le premier trait d'une brosse du décor
    /// crée dans un plan (`decor.rs`). Faux (d'origine) : vectoriel. Un calque Décor qui existe déjà garde sa sorte. Réglage de l'application,
    /// posé par `decor_avant_trait` : il n'est pas dans le projet.
    pub decor_bitmap: bool,
    /// « Durée d'un nouveau plan » (Préférences > Montage), en secondes : celle que reçoit un plan neuf (`make_panel`). Comme `report_decor`,
    /// un réglage de l'application que `App` pose avant ses commandes (`dureeplans.rs`) : il n'est pas dans le projet.
    pub duree_plan_neuf: f32,
    next_id: u64,
    /// Préfixe des numéros neufs d'un projet partagé (`préfixe << 32`, `cooperation.rs`) : deux équipiers ne
    /// donnent jamais le même numéro. 0 pour un projet classique, dont les numéros ne changent pas.
    id_prefix: u64,
    /// Ce que l'enregistrement sait déjà de chaque dessin, à sa version (`project::MemoDessin`) : qu'il a quelque chose
    /// hors de la feuille, quelles textures ses traits citent. Sans lui, chaque enregistrement relisait sur le fil de
    /// l'interface les pixels du bord de tous les dessins du film. Rempli à l'ouverture (sur le fil d'ouverture).
    pub(crate) memo_enregistrement: std::sync::Mutex<crate::project::MemoDessins>,
}

/// Les 32 bits du bas d'un numéro : le compteur de son équipier (`Document::id_prefix`).
pub(crate) const ID_LOW: u64 = 0xFFFF_FFFF;

impl Document {
    /// Projet neuf d'un cadre `frame_w`×`frame_h`, sur une feuille deux fois plus grande.
    pub fn with_frame(frame_w: usize, frame_h: usize, fps: u32) -> Self {
        let mut doc = Self::new(frame_w * 2, frame_h * 2, fps);
        doc.frame = [frame_w, frame_h];
        doc
    }

    /// Le même projet, avec maintenant pour date de création (13e, propriétés du projet).
    pub fn created_now(mut self) -> Self {
        self.properties.created = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        self
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
            guides: Vec::new(),
            board: Default::default(),
            groups: crate::groups::defaults(),
            start_frame: 1,
            background: BackgroundKind::Color,
            paper_b: DEFAULT_PAPER_B,
            master_volume: 1.0,
            storyboard_locked: false,
            numbering_locked: false,
            properties: Properties::default(),
            comments: Vec::new(),
            reserves: Default::default(),
            transitions: Vec::new(),
            sons_proposes: Default::default(),
            references: Vec::new(),
            personnages: Vec::new(),
            envois: Default::default(),
            versions: Default::default(),
            variantes: Default::default(),
            corbeille: Default::default(),
            annules: Default::default(),
            variantes_partielles: false,
            annotations: Default::default(),
            scenario: None,
            report_decor: true,
            decor_bitmap: false,
            duree_plan_neuf: crate::dureeplans::secondes_d_origine(),
            next_id: 1,
            id_prefix: 0,
            memo_enregistrement: Default::default(),
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
            .chain(structure.comments.iter().map(|c| c.id))
            .chain(structure.transitions.iter().map(|t| t.id))
            .chain(structure.references.iter().map(|r| r.id))
            .chain(structure.personnages.iter().map(|p| p.id))
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
            guides: structure.guides,
            board: structure.board,
            groups: crate::groups::completed(structure.groups),
            start_frame: structure.start_frame,
            background: structure.background,
            paper_b: structure.paper_b,
            master_volume: structure.master_volume,
            storyboard_locked: structure.storyboard_locked,
            numbering_locked: structure.numbering_locked,
            properties: structure.properties,
            comments: structure.comments,
            reserves: structure.reserves,
            transitions: structure.transitions,
            sons_proposes: Default::default(),
            references: structure.references,
            personnages: structure.personnages,
            envois: Default::default(),
            versions: structure.versions,
            variantes: structure.variantes,
            corbeille: structure.corbeille,
            annules: structure.annules,
            variantes_partielles: false,
            annotations: structure.annotations,
            scenario: structure.scenario,
            report_decor: true,
            decor_bitmap: false,
            duree_plan_neuf: crate::dureeplans::secondes_d_origine(),
            next_id: max_id + 1,
            id_prefix: 0,
            memo_enregistrement: Default::default(),
        };
        doc.normalize();
        doc
    }

    /// Le même document sans ses dessins (`cels` vide) : ce que l'enregistrement emporte sur son fil pour y écrire le
    /// manifeste et les fichiers annexes (`project::prepare_save`). Les sons et les instantanés d'envois sont partagés
    /// (`Arc`), le reste est copié. Chaque champ est nommé : un champ ajouté au document doit dire ici s'il part.
    pub(crate) fn copie_sans_dessins(&self) -> Document {
        Document {
            width: self.width,
            height: self.height,
            frame: self.frame,
            fps: self.fps,
            paper: self.paper,
            sequences: self.sequences.clone(),
            scenes: self.scenes.clone(),
            panels: self.panels.clone(),
            cels: HashMap::new(),
            audio_tracks: self.audio_tracks.clone(),
            sounds: self.sounds.clone(),
            guides: self.guides.clone(),
            board: self.board.clone(),
            groups: self.groups.clone(),
            start_frame: self.start_frame,
            background: self.background,
            paper_b: self.paper_b,
            master_volume: self.master_volume,
            storyboard_locked: self.storyboard_locked,
            numbering_locked: self.numbering_locked,
            properties: self.properties.clone(),
            comments: self.comments.clone(),
            reserves: self.reserves.clone(),
            transitions: self.transitions.clone(),
            sons_proposes: self.sons_proposes.clone(),
            references: self.references.clone(),
            personnages: self.personnages.clone(),
            envois: crate::envois::Envois { list: self.envois.list.clone(), pending: self.envois.pending.clone(), dossier: self.envois.dossier.clone() },
            versions: self.versions.clone(),
            variantes: self.variantes.clone(),
            corbeille: self.corbeille.clone(),
            annules: self.annules.clone(),
            variantes_partielles: self.variantes_partielles,
            annotations: self.annotations.clone(),
            scenario: self.scenario.clone(),
            report_decor: self.report_decor,
            decor_bitmap: self.decor_bitmap,
            duree_plan_neuf: self.duree_plan_neuf,
            next_id: self.next_id,
            id_prefix: self.id_prefix,
            memo_enregistrement: Default::default(),
        }
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
        // Jamais plus grande que la texture minimale garantie des cartes graphiques : au-delà, la vue serait blanche.
        let (width, height) = (width.min(MAX_SHEET_SIDE), height.min(MAX_SHEET_SIDE));
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
        self.map_annules(|p, r| {
            for k in &mut p.camera {
                k.cam = k.cam.shifted(dx as f32, dy as f32);
            }
            if let Some(r) = r {
                r.map_cameras(|c| c.shifted(dx as f32, dy as f32));
            }
        });
        self.map_reserve_cameras(|c| c.shifted(dx as f32, dy as f32));
        self.width = width;
        self.height = height;
    }

    /// Nouvelle taille d'image `w`×`h` (le cadre ; passe interface, Fichier > Réglages du projet).
    /// - `scale` : tout ce qui est en pixels est agrandi ou réduit (dessins, guides, caméra,
    ///   placement des calques) pour que le dessin remplisse le nouveau cadre sans être déformé ;
    ///   avec d'autres proportions, ce qui dépasse sort du cadre.
    /// - sinon : les dessins gardent leur taille en pixels, le cadre change autour d'eux (centré).
    ///
    /// La feuille reste au moins le double du cadre. Ne s'annule pas : l'appelant vide l'historique.
    pub fn resize_frame(&mut self, w: usize, h: usize, scale: bool) {
        let (w, h) = (w.clamp(16, 4096), h.clamp(16, 4096));
        let [fw, fh] = self.frame;
        if scale {
            let s = (w as f32 / fw.max(1) as f32).max(h as f32 / fh.max(1) as f32);
            if (s - 1.0).abs() > 1e-6 {
                self.scale_stage(s, [w, h]);
            }
        }
        self.frame = [w, h];
        let (sw, sh) = (self.width.max(w * 2), self.height.max(h * 2));
        // Agrandir garde le centre ; avec des parités différentes, un pixel de plus suffit.
        let (sw, sh) = (sw + (sw - self.width) % 2, sh + (sh - self.height) % 2);
        if sw > self.width || sh > self.height {
            self.enlarge_stage(sw, sh);
        }
    }

    /// Multiplie par `s` tout ce qui est en pixels de la feuille, feuille comprise ; `cadre` : le cadre d'après. C'est la
    /// règle du collage depuis un projet d'une autre taille (`adaptation.rs`) avec l'origine de la feuille pour pivot : les
    /// dessins (feuille et hors-feuille, fond sous le trait compris), la pose et les clés de position des calques, les
    /// paramètres d'effets en pixels et la caméra suivent d'un seul code.
    fn scale_stage(&mut self, s: f32, cadre: [usize; 2]) {
        use crate::adaptation::{Adaptation, Ajustement, Duree, Origine, Progression};
        let nw = ((self.width as f32 * s).round() as usize).max(1);
        let nh = ((self.height as f32 * s).round() as usize).max(1);
        let src = Origine::de(self);
        let dst = Origine { cadre, feuille: [nw, nh], cadence: src.cadence };
        let a = Adaptation { src, dst, ajustement: Ajustement::Remplir, duree: Duree::Images, s, t: Vec2::ZERO };
        self.cels = crate::adaptation::adapte_dessins(&self.cels, &a, &Progression::default());
        for g in &mut self.guides {
            for p in &mut g.points {
                p[0] *= s;
                p[1] *= s;
            }
        }
        for panel in &mut self.panels {
            for k in &mut panel.camera {
                *k = crate::adaptation::adapte_cle_camera(k, &a);
            }
            for l in &mut panel.layers {
                crate::adaptation::adapte_calque(l, &a);
            }
        }
        self.map_reserve_cameras(|c| crate::adaptation::adapte_camera(c, &a));
        self.map_annules(|p, r| {
            for k in &mut p.camera {
                *k = crate::adaptation::adapte_cle_camera(k, &a);
            }
            for l in &mut p.layers {
                crate::adaptation::adapte_calque(l, &a);
            }
            if let Some(r) = r {
                r.map_cameras(|c| crate::adaptation::adapte_camera(c, &a));
            }
        });
        self.width = nw;
        self.height = nh;
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
    ///
    /// C'est la caméra **montrée** : avec la postérisation temporelle du plan (`Panel::saccade`), elle ne change
    /// que toutes les N images ; tout ce qui montre ou exporte l'image passe par ici. Ce qui règle les clés
    /// (outil Caméra, pose d'une clé, réunion) lit la caméra douce : `panel_camera_douce`.
    pub fn panel_camera(&self, index: usize, frame: usize) -> Camera {
        self.panel_camera_selon(index, frame, true, true)
    }

    /// La caméra du plan `index` à l'image `frame`, interpolée en douceur entre les clés, sans la
    /// postérisation temporelle ni l'effet vivant : celle des clés, que l'édition règle.
    pub fn panel_camera_douce(&self, index: usize, frame: usize) -> Camera {
        self.panel_camera_selon(index, frame, false, false)
    }

    /// La caméra douce (sans les paliers), avec l'effet de caméra vivant du plan (partie 129) : ce que montre l'outil
    /// Caméra arrêté, pour que l'effet se voie pendant qu'on le règle.
    pub fn panel_camera_vivante(&self, index: usize, frame: usize) -> Camera {
        self.panel_camera_selon(index, frame, false, true)
    }

    /// Les clés de caméra du plan `index` telles qu'elles se montrent : les siennes, et son effet vivant cuit dedans
    /// (`camfx::cuire`), comme le ferait « Convertir en clés ». Sans effet : ses clés, sans copie.
    pub fn cles_montrees(&self, index: usize) -> std::borrow::Cow<'_, [camera::CameraKey]> {
        use std::borrow::Cow;
        let Some(p) = self.panels.get(index) else { return Cow::Borrowed(&[]) };
        let Some(v) = p.effet_camera else { return Cow::Borrowed(&p.camera) };
        let base = |f: usize| self.panel_camera_douce(index, f);
        match crate::camfx::cuire_vite(&v, self.fps, self.frame, p.frame_count(), &p.camera, &base) {
            Some(k) => Cow::Owned(k),
            None => Cow::Borrowed(&p.camera),
        }
    }

    /// Écrit l'effet vivant du plan `index` dans ses clés et le retire, **sans changer aucune image** : les clés écrites
    /// sont celles que l'effet montrait (`cles_montrees`). Les gestes qui coupent, réunissent, rognent ou décalent les
    /// images d'un plan le font d'abord (partie 129) : ils savent garder l'image avec des clés, pas avec un effet vivant.
    /// Vrai s'il y avait un effet.
    pub fn cuire_effet_vivant(&mut self, index: usize) -> bool {
        self.cuire_effets_vivants(&[index])
    }

    /// Comme `cuire_effet_vivant`, pour plusieurs plans **d'un coup** : toutes les clés sont lues avant d'en écrire une,
    /// pour que deux plans voisins sans clé, chacun avec son effet, gardent chacun son image (la base de l'un ne bouge pas
    /// quand l'autre devient des clés).
    pub fn cuire_effets_vivants(&mut self, indices: &[usize]) -> bool {
        let cuites: Vec<(usize, Vec<camera::CameraKey>)> = indices
            .iter()
            .copied()
            .filter(|&i| self.panels.get(i).is_some_and(|p| p.effet_camera.is_some()))
            .map(|i| (i, self.cles_montrees(i).into_owned()))
            .collect();
        for (i, keys) in &cuites {
            let p = &mut self.panels[*i];
            p.camera = keys.clone();
            p.effet_camera = None;
        }
        !cuites.is_empty()
    }

    fn panel_camera_selon(&self, index: usize, frame: usize, paliers: bool, effet: bool) -> Camera {
        let default = self.default_camera();
        let Some(p) = self.panels.get(index) else { return default };
        // La continuité (un plan sans clé prend la caméra de son voisin) se lit **avec l'effet du voisin**, partout : à
        // l'écran comme pour la caméra douce, base de l'effet d'un plan. Ainsi convertir l'effet du voisin en clés ne change
        // rien chez lui (partie 129). Une seule exception, pour ne pas tourner en rond : la base de l'effet d'un plan sans
        // clé ne lit pas l'effet d'un voisin lui aussi sans clé.
        let base_d_un_effet_sans_cle = !effet && p.camera.is_empty() && p.effet_camera.is_some();
        let cles = |i: usize| {
            let q = &self.panels[i];
            let lire_effet = if i == index { effet } else { !(base_d_un_effet_sans_cle && q.camera.is_empty()) };
            if lire_effet { self.cles_montrees(i) } else { std::borrow::Cow::Borrowed(q.camera.as_slice()) }
        };
        let au = |q: &Panel, keys: &[camera::CameraKey], f: usize| camera::camera_at(keys, if paliers { q.saccade.image_montree(keys, f) } else { f }, default);
        let mine = cles(index);
        if !mine.is_empty() {
            return au(p, &mine, frame);
        }
        let (start, end) = self.scene_run(index);
        for i in (start..index).rev() {
            let k = cles(i);
            if !k.is_empty() {
                let q = &self.panels[i];
                return au(q, &k, q.frame_count().saturating_sub(1));
            }
        }
        for i in index + 1..end {
            let k = cles(i);
            if !k.is_empty() {
                return au(&self.panels[i], &k, 0);
            }
        }
        default
    }

    /// Caméra d'un plan rangé (une version qui n'est pas dans `panels`) à son image `frame` : ses propres clés,
    /// sinon la caméra par défaut (avec la postérisation temporelle du plan, s'il en a une, et son effet vivant).
    pub fn camera_du_plan_range(&self, p: &Panel, frame: usize) -> Camera {
        let default = self.default_camera();
        let base = |f: usize| camera::camera_at(&p.camera, f, default);
        let vivantes = p.effet_camera.and_then(|v| crate::camfx::cuire_vite(&v, self.fps, self.frame, p.frame_count(), &p.camera, &base));
        let keys = vivantes.as_deref().unwrap_or(&p.camera);
        if keys.is_empty() {
            default
        } else {
            camera::camera_at(keys, p.saccade.image_montree(keys, frame), default)
        }
    }

    /// Comme `camera_seen`, pour un plan rangé.
    pub fn camera_seen_du_plan_range(&self, p: &Panel, frame: usize) -> CameraSeen {
        let c = self.camera_du_plan_range(p, frame);
        CameraSeen::de(&c, self.stage_center(), self.frame)
    }

    /// Où est la caméra du plan `index` à l'image `frame`, pour le multiplan : décalage depuis
    /// le centre de la feuille, avancée, distance de référence.
    pub fn camera_seen(&self, index: usize, frame: usize) -> CameraSeen {
        let c = self.panel_camera(index, frame);
        CameraSeen::de(&c, self.stage_center(), self.frame)
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
            start_frame: self.start_frame,
            background: self.background,
            paper_b: self.paper_b,
            master_volume: self.master_volume,
            storyboard_locked: self.storyboard_locked,
            numbering_locked: self.numbering_locked,
            properties: self.properties.clone(),
            comments: self.comments.clone(),
            reserves: self.reserves.clone(),
            transitions: self.transitions.clone(),
            references: self.references.clone(),
            personnages: self.personnages.clone(),
            versions: self.versions.clone(),
            variantes: self.variantes.clone(),
            corbeille: self.corbeille.clone(),
            annules: self.annules.clone(),
            annotations: self.annotations.clone(),
            scenario: self.scenario.clone(),
            frames_rangees: None,
        }
    }

    /// Papier sous les calques à l'export et à l'écran : la couleur A si l'arrière-plan est une
    /// couleur, sinon rien (transparent ; le damier n'est qu'un affichage).
    pub fn render_paper(&self) -> Option<[u8; 3]> {
        (self.background == BackgroundKind::Color).then_some(self.paper)
    }

    pub fn replace_structure(&mut self, mut s: Structure) -> Structure {
        // Les séquences que la planche montre sont un affichage, pas une étape du film : annuler ou rétablir ne les change pas
        // (`planchesequences.rs`).
        s.annotations.affichage = self.annotations.affichage.clone();
        // (Une vieille étape d'annulation a rangé les expositions de ses calques.)
        s.rouvrir_les_expositions();
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
            start_frame: std::mem::replace(&mut self.start_frame, s.start_frame),
            background: std::mem::replace(&mut self.background, s.background),
            paper_b: std::mem::replace(&mut self.paper_b, s.paper_b),
            master_volume: std::mem::replace(&mut self.master_volume, s.master_volume),
            storyboard_locked: std::mem::replace(&mut self.storyboard_locked, s.storyboard_locked),
            numbering_locked: std::mem::replace(&mut self.numbering_locked, s.numbering_locked),
            properties: std::mem::replace(&mut self.properties, s.properties),
            comments: std::mem::replace(&mut self.comments, s.comments),
            reserves: std::mem::replace(&mut self.reserves, s.reserves),
            transitions: std::mem::replace(&mut self.transitions, s.transitions),
            references: std::mem::replace(&mut self.references, s.references),
            personnages: std::mem::replace(&mut self.personnages, s.personnages),
            versions: std::mem::replace(&mut self.versions, s.versions),
            variantes: std::mem::replace(&mut self.variantes, s.variantes),
            corbeille: std::mem::replace(&mut self.corbeille, s.corbeille),
            annules: std::mem::replace(&mut self.annules, s.annules),
            annotations: std::mem::replace(&mut self.annotations, s.annotations),
            scenario: std::mem::replace(&mut self.scenario, s.scenario),
            frames_rangees: None,
        }
    }

    pub(crate) fn alloc(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id | self.id_prefix
    }

    /// Les numéros jusqu'à `id` sont pris (marques et commentaires d'une réserve relue). Dans un projet partagé,
    /// seuls comptent les numéros de mon préfixe : ceux des autres équipiers ont leur propre compteur.
    pub(crate) fn reserve_id(&mut self, id: u64) {
        if self.id_prefix == 0 {
            self.next_id = self.next_id.max(id + 1);
        } else if id & !ID_LOW == self.id_prefix {
            self.next_id = self.next_id.max((id & ID_LOW) + 1);
        }
    }

    /// Le projet est partagé avec d'autres équipiers (numérotation à préfixe).
    pub(crate) fn est_partage(&self) -> bool {
        self.id_prefix != 0
    }

    /// Le préfixe des numéros neufs (0 : projet classique).
    #[cfg(test)]
    pub(crate) fn id_prefix(&self) -> u64 {
        self.id_prefix
    }

    /// Passe le projet à la numérotation d'un équipier (`prefixe` sur 20 bits) : les numéros neufs sont
    /// `prefixe << 32 | compteur`, le compteur repartant au-dessus du plus grand numéro de ce préfixe déjà
    /// dans le projet. Les numéros d'avant ne changent pas.
    pub(crate) fn set_id_prefix(&mut self, prefixe: u64) {
        self.id_prefix = (prefixe & 0xF_FFFF) << 32;
        self.next_id = 1;
        let mine = self.id_prefix;
        let mut top = 0u64;
        let mut see = |id: u64| {
            if mine != 0 && id & !ID_LOW == mine {
                top = top.max(id & ID_LOW);
            }
        };
        for p in self.panels.iter().chain(self.plans_ranges()) {
            see(p.id);
            p.marks.iter().for_each(|m| see(m.id));
        }
        self.scenes.iter().for_each(|s| see(s.id));
        self.sequences.iter().for_each(|s| see(s.id));
        for t in &self.audio_tracks {
            see(t.id);
            t.clips.iter().for_each(|c| see(c.id));
        }
        self.comments.iter().for_each(|c| see(c.id));
        self.transitions.iter().for_each(|t| see(t.id));
        self.ids_des_variantes().into_iter().for_each(&mut see);
        self.references.iter().for_each(|r| see(r.id));
        self.personnages.iter().for_each(|p| see(p.id));
        self.cels.keys().for_each(|&c| see(c));
        self.sounds.keys().for_each(|&s| see(s));
        for r in self.reserves.values() {
            for piece in [&r.head, &r.tail] {
                piece.marks.iter().for_each(|m| see(m.id));
                piece.comments.iter().for_each(|c| see(c.id));
            }
        }
        self.next_id = top + 1;
    }

    pub fn new_sequence(&mut self) -> u64 {
        let id = self.alloc();
        self.sequences.push(Sequence { id, name: String::new(), number: String::new() });
        id
    }

    pub fn new_scene(&mut self, sequence: u64) -> u64 {
        let id = self.alloc();
        self.scenes.push(Scene {
            id,
            name: String::new(),
            number: String::new(),
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
        // (Dans l'ordre de première apparition. `contains` dans la boucle coûtait le carré du
        // nombre de plans : 11 ms par image d'écran à 600 plans, pour chaque titre de plan.)
        let mut seen = std::collections::HashSet::new();
        let mut order: Vec<u64> = Vec::new();
        for p in &self.panels {
            if seen.insert(p.scene) {
                order.push(p.scene);
            }
        }
        order
    }

    /// Séquences dans l'ordre du film.
    pub fn sequence_order(&self) -> Vec<u64> {
        let mut seen = std::collections::HashSet::new();
        let mut order: Vec<u64> = Vec::new();
        for s in self.sequence_ids() {
            if seen.insert(s) {
                order.push(s);
            }
        }
        order
    }

    /// La séquence de chaque panneau, dans l'ordre du film (une passe : `sequence_of` cherche la
    /// scène dans la liste à chaque appel).
    pub fn sequence_ids(&self) -> Vec<u64> {
        let of_scene: std::collections::HashMap<u64, u64> = self.scenes.iter().map(|s| (s.id, s.sequence)).collect();
        self.panels.iter().map(|p| of_scene.get(&p.scene).copied().unwrap_or(0)).collect()
    }

    /// Numéro automatique de la scène (1, 2, 3… dans l'ordre du film, toutes séquences confondues).
    pub fn scene_number(&self, scene: u64) -> usize {
        // (Les plans annulés gardent leur numéro : ils comptent, `planannule.rs`.)
        self.scene_order_planche().iter().position(|&s| s == scene).map_or(0, |i| i + 1)
    }

    pub fn sequence_number(&self, sequence: u64) -> usize {
        self.sequence_order_planche().iter().position(|&s| s == sequence).map_or(0, |i| i + 1)
    }

    /// Nom affiché de la scène : son nom s'il en a un, sinon son numéro.
    pub fn scene_label(&self, scene: u64) -> String {
        match self.scene_ou_annulee(scene) {
            Some(s) if !s.name.trim().is_empty() => s.name.trim().to_string(),
            _ => self.plan_code(scene),
        }
    }

    pub fn sequence_label(&self, sequence: u64) -> String {
        match self.sequence_ou_annulee(sequence) {
            Some(s) if !s.name.trim().is_empty() => s.name.trim().to_string(),
            _ => self.sequence_code(sequence),
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
        use std::collections::{HashMap, HashSet};
        // Tout le film était annulé et un plan vient d'entrer : le plan vide qui tenait la place part (`planannule.rs`).
        self.annules_menage();
        // (Des tables, pas des recherches dans les listes : l'ordre des plans se remettait en un temps de l'ordre du
        // nombre de plans à la puissance 4 — 100 s pour construire un film de 2 000 plans, une demi-seconde par
        // opération de montage à ce nombre. Passe de fiabilité du 2026-10-24.)
        let mut scenes_connues: HashSet<u64> = self.scenes.iter().map(|s| s.id).collect();
        // Un panneau sans scène connue rejoint la scène du panneau précédent (ou une scène neuve).
        for i in 0..self.panels.len() {
            if !scenes_connues.contains(&self.panels[i].scene) {
                let scene = match i.checked_sub(1) {
                    Some(prev) => self.panels[prev].scene,
                    None => {
                        let seq = self.sequences.first().map(|s| s.id).unwrap_or_else(|| self.new_sequence());
                        let neuve = self.new_scene(seq);
                        scenes_connues.insert(neuve);
                        neuve
                    }
                };
                self.panels[i].scene = scene;
            }
        }
        let mut sequences_connues: HashSet<u64> = self.sequences.iter().map(|q| q.id).collect();
        for s in 0..self.scenes.len() {
            if !sequences_connues.contains(&self.scenes[s].sequence) {
                let seq = self.sequences.first().map(|s| s.id).unwrap_or_else(|| self.new_sequence());
                sequences_connues.insert(seq);
                self.scenes[s].sequence = seq;
            }
        }
        // La séquence de chaque scène (la première trouvée, comme `find`).
        let mut seq_de_scene: HashMap<u64, u64> = HashMap::with_capacity(self.scenes.len());
        for s in &self.scenes {
            seq_de_scene.entry(s.id).or_insert(s.sequence);
        }
        let scene_order = self.scene_order();
        let mut seq_order: Vec<u64> = Vec::new();
        let mut rang_seq: HashMap<u64, usize> = HashMap::new();
        for &sc in &scene_order {
            if let Some(&q) = seq_de_scene.get(&sc) {
                if let std::collections::hash_map::Entry::Vacant(v) = rang_seq.entry(q) {
                    v.insert(seq_order.len());
                    seq_order.push(q);
                }
            }
        }
        // Les panneaux d'une même scène se suivent (dans leur ordre), les scènes d'une même séquence aussi (dans l'ordre de
        // première apparition), les séquences dans l'ordre de première apparition ; ceux d'une scène sans séquence à la fin.
        let rang_scene: HashMap<u64, usize> = scene_order.iter().enumerate().map(|(k, &sc)| (sc, k)).collect();
        self.panels.sort_by_cached_key(|p| match seq_de_scene.get(&p.scene).and_then(|q| rang_seq.get(q)) {
            Some(&rq) => (rq, rang_scene.get(&p.scene).copied().unwrap_or(0)),
            None => (usize::MAX, 0),
        });
        let scenes_gardees: HashSet<u64> = scene_order.iter().copied().collect();
        let sequences_gardees: HashSet<u64> = seq_order.iter().copied().collect();
        self.scenes.retain(|s| scenes_gardees.contains(&s.id));
        self.sequences.retain(|q| sequences_gardees.contains(&q.id));
        // Un plan retiré emporte ses commentaires (l'annulation les ramène avec lui).
        self.prune_comments();
        // Une transition dont un plan a disparu part avec lui (`transitions.rs`).
        self.prune_transitions();
        // Celles d'un plan écarté reviennent dès que leur coupe existe de nouveau (`planannule.rs`).
        self.annules_reposer_transitions();
        // Les références liées à un plan retiré ne le sont plus (elles restent sur le tableau).
        self.prune_references();
        // Les variantes rangées d'une séquence qui n'existe plus partent avec elle (`variantes.rs`).
        self.elaguer_variantes();
        // Numérotation verrouillée : un plan ou une séquence neuf reçoit son numéro entre ses voisins.
        self.assign_numbers();
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

    /// Nouvelle scène (un panneau) juste avant celle du panneau `index`, dans la même séquence
    /// (13e, « ajouter un plan avant le plan courant »). Renvoie l'indice du nouveau panneau.
    pub fn add_scene_before(&mut self, index: usize) -> usize {
        let (start, _) = self.scene_run(index);
        let scene = self.new_scene(self.sequence_of(index));
        let panel = self.make_panel(scene);
        self.insert_and_normalize(start, vec![panel])
    }

    /// Nouveau projet fait des plans `ids` (13e, « nouveau projet à partir des plans choisis »),
    /// dans l'ordre du film : même feuille, cadre, cadence, papier, arrière-plan, groupes,
    /// guides et image de départ ; chaque plan garde ses dessins, sa séquence et les notes de
    /// planche qui lui sont attachées. Le son n'est pas repris (il est en temps du montage, pas
    /// dans les plans). Des propriétés, seuls l'auteur et le copyright suivent. Rien si aucun des
    /// plans n'existe.
    pub fn from_plans(&self, ids: &[u64]) -> Option<Document> {
        use std::collections::HashSet;
        let mut s = self.structure();
        s.panels.retain(|p| ids.contains(&p.id));
        if s.panels.is_empty() {
            return None;
        }
        let scenes: HashSet<u64> = s.panels.iter().map(|p| p.scene).collect();
        s.scenes.retain(|x| scenes.contains(&x.id));
        let sequences: HashSet<u64> = s.scenes.iter().map(|x| x.sequence).collect();
        s.sequences.retain(|x| sequences.contains(&x.id));
        s.audio_tracks.clear();
        // (Les variantes rangées des autres séquences restent dans le film d'origine.)
        s.variantes = Default::default();
        // (Ni les plans annulés : ils restent dans le film d'origine.)
        s.annules = Default::default();
        let cards: HashSet<u64> = s.panels.iter().flat_map(|p| std::iter::once(p.id).chain(p.marks.iter().map(|m| m.id))).collect();
        let kept = |anchor: Option<u64>| anchor.is_some_and(|a| cards.contains(&a));
        s.board.strokes.retain(|n| kept(n.anchor));
        s.board.texts.retain(|n| kept(n.anchor));
        s.storyboard_locked = false;
        s.properties = Properties {
            author: self.properties.author.clone(),
            copyright: self.properties.copyright.clone(),
            fond: self.properties.fond,
            ..Default::default()
        };
        // Le contenu rogné des plans gardés les suit, avec ses dessins (`rognage.rs`).
        s.reserves.retain(|id, _| ids.contains(id));
        let rogne: Vec<CelId> = s.reserves.values().flat_map(|r| r.cels()).collect();
        let cels = s
            .panels
            .iter()
            .flat_map(|p| &p.layers)
            .flat_map(crate::calquecouleur::dessins_du_calque)
            .chain(rogne.iter().copied())
            .filter_map(|id| self.cels.get(&id).map(|c| (id, c.clone())))
            .collect();
        let mut doc = Document::from_parts(self.width, self.height, s, cels, HashMap::new());
        doc.frame = self.frame;
        Some(doc)
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
        // (Le dernier plan d'une séquence qui a d'autres versions de séquence ne se retire pas : elles disparaîtraient.)
        let retire = self.panels[index].id;
        if self.refuser_si_sequence_videe(|p| p == retire) {
            return None;
        }
        // (Un plan qu'une autre version de sa séquence montre est gardé : `variantes.rs`.)
        self.ranger_plan_partage(self.panels[index].id, false);
        // Ce qui sort du film reste récupérable : le plan va à la corbeille (`corbeille.rs`).
        self.jeter_plan(index, &[], "Plan supprimé");
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
        let retires: Vec<u64> = self.panels[start..end].iter().map(|p| p.id).collect();
        if self.refuser_si_sequence_videe(|p| retires.contains(&p)) {
            return None;
        }
        self.ranger_plans_partages(&retires, false);
        for i in start..end {
            self.jeter_plan(i, &retires, "Scène supprimée");
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
        // (Le dernier plan d'une séquence qui a d'autres versions de séquence ne la quitte pas : elles disparaîtraient.)
        if !self.variantes.is_empty() {
            let at = if slot > from { slot - 1 } else { slot };
            let autres: Vec<&Panel> = self.panels.iter().enumerate().filter(|(i, _)| *i != from).map(|(_, p)| p).collect();
            let (left, right) = (at.checked_sub(1).map(|i| autres[i].scene), autres.get(at).map(|p| p.scene));
            let own = self.panels[from].scene;
            let cible = if left != Some(own) && right != Some(own) { right.or(left).unwrap_or(own) } else { own };
            let sequence = |scene: u64| self.scene(scene).map(|s| s.sequence);
            let id = self.panels[from].id;
            if sequence(cible) != sequence(own) && self.refuser_si_sequence_videe(|p| p == id) {
                return None;
            }
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
        // Les plans qui quittent leur séquence : le dernier plan d'une séquence qui a d'autres versions de séquence ne la quitte pas.
        if !self.variantes.is_empty() {
            let cible = self.scene(scene).map(|s| s.sequence);
            let seqs = self.sequence_ids();
            let partent: std::collections::HashSet<u64> = self.panels.iter().zip(&seqs).filter(|(p, s)| moving(p) && Some(**s) != cible).map(|(p, _)| p.id).collect();
            if self.refuser_si_sequence_videe(|p| partent.contains(&p)) {
                return None;
            }
        }
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
        let retires: Vec<u64> = self.panels.iter().filter(|p| ids.contains(&p.id) && Some(p.id) != spared).map(|p| p.id).collect();
        if !self.variantes.is_empty() {
            let partent: std::collections::HashSet<u64> = retires.iter().copied().collect();
            if self.refuser_si_sequence_videe(|p| partent.contains(&p)) {
                return None;
            }
            self.ranger_plans_partages(&retires, false);
        }
        // Ce qui sort du film reste récupérable : chaque plan va à la corbeille (`corbeille.rs`), dans l'ordre du film.
        for i in 0..self.panels.len() {
            if retires.contains(&self.panels[i].id) {
                self.jeter_plan(i, &retires, "Plan supprimé");
            }
        }
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
        // Un effet de caméra vivant devient d'abord ses clés, sans changer l'image : la coupe des clés (`split_keys`) garde
        // alors chaque image des deux plans (partie 129).
        self.cuire_effet_vivant(index);
        let source = self.panels[index].clone();
        let mut copies: HashMap<CelId, CelId> = HashMap::new();
        let mut layers = Vec::with_capacity(source.layers.len());
        for layer in &source.layers {
            let mut frames = Vec::with_capacity(layer.frames.len().saturating_sub(at));
            for f in layer.frames.iter().skip(at) {
                frames.push(match *f {
                    // Un calque de toute la scène garde le dessin de la scène ; un calque partagé (`partage.rs`) garde le dessin
                    // partagé : les deux parties restent liées.
                    Some(c) if layer.scene_wide.is_some() || layer.partage.is_some() => Some(c),
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
                // Les rôles d'après la coupe passent au second (13g-2), comme les clés d'effets (13i).
                roles: layer.roles.range(at..).map(|(f, r)| (f - at, *r)).collect(),
                fx: crate::fx::keys_from(&layer.fx, at),
                // Les clés de position d'après la coupe passent au second (`poskeys::split_keys`).
                pos_keys: crate::poskeys::split_keys(&layer.pos_keys, at, layer.transform).1,
                // (Une trajectoire couvre un plan entier : chaque moitié garde ses clés, à la main.)
                trajectoire: None,
                opacity_keys: crate::opacitecles::split_keys(&layer.opacity_keys, at, layer.opacity).1,
                ..layer.clone()
            };
            l.trim();
            layers.push(l);
        }
        let old_ids: Vec<u64> = source.layers.iter().map(|l| l.id).collect();
        self.copier_griffonnages(&source.layers, &mut copies);
        relink_copies(&mut layers, &old_ids, &copies);
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
            saccade: source.saccade,
            // (L'effet vivant a été cuit en clés avant la coupe.)
            effet_camera: None,
            marks: Vec::new(),
            // Les signets d'après la coupe passent au nouveau plan ; les points d'entrée et de
            // sortie restent au premier.
            cues: Cues { bookmarks: source.cues.bookmarks.iter().filter(|&&b| b >= at).map(|b| b - at).collect(), ..Cues::default() },
            color: source.color,
            espace: source.espace,
        };
        // Le contenu rogné à la fin du plan est celui de la fin du second (qui finit où le plan finissait) : il lui passe, avec
        // les numéros de ses calques. Rien n'est oublié.
        let rename: HashMap<u64, u64> = old_ids.iter().copied().zip(second.layers.iter().map(|l| l.id)).collect();
        self.move_tail_reserve(source.id, second.id, &rename);
        let first = &mut self.panels[index];
        for l in &mut first.layers {
            l.frames.truncate(at);
            l.roles.retain(|f, _| *f < at);
            for e in &mut l.fx {
                e.keys.retain(|k| k.frame < at);
            }
            l.pos_keys = crate::poskeys::split_keys(&l.pos_keys, at, l.transform).0;
            l.trajectoire = None;
            l.opacity_keys = crate::opacitecles::split_keys(&l.opacity_keys, at, l.opacity).0;
            l.trim();
        }
        first.length = at;
        first.camera = keys_before;
        first.cues.bookmarks.retain(|&b| b < at);
        // Les commentaires d'après la coupe passent au nouveau plan.
        let (first_id, second_id) = (first.id, second.id);
        self.split_comments(first_id, second_id, at);
        self.annules_suivre_coupe(first_id, second_id);
        Some(self.insert_and_normalize(index + 1, vec![second]))
    }

    /// Réunit le panneau `index` et le suivant, s'il est dans la même scène : les images du
    /// suivant viennent après, calque par calque (même rang et même sorte, sinon un calque de
    /// plus), et les textes se suivent. L'inverse de `split_panel`.
    pub fn merge_with_next(&mut self, index: usize) -> Option<usize> {
        if index + 1 >= self.panels.len() || self.panels[index].scene != self.panels[index + 1].scene {
            return None;
        }
        // (Les versions rangées des deux plans se gardent : celles du suivant deviennent des versions du plan réuni.)
        if self.refuser_si_trop_de_versions(&[self.panels[index].id, self.panels[index + 1].id]) {
            return None;
        }
        // Deux effets de caméra vivants ne tiennent pas dans un plan : ils deviennent d'abord leurs clés, sans changer l'image
        // (partie 129). Puis les caméras qu'ils montraient (un panneau sans clé hérite de ses voisins) sont gardées ; les
        // caméras douces, celles des clés (comme `append_next_with`).
        self.cuire_effets_vivants(&[index, index + 1]);
        let (cam_a, cam_b) = (self.panel_camera_douce(index, 0), self.panel_camera_douce(index + 1, 0));
        // (Le plan réuni, qu'une autre version de sa séquence montre, en garde une copie : `variantes.rs`.)
        self.ranger_plan_partage(self.panels[index + 1].id, true);
        // Les calques partagés du second (`partage.rs`) se détachent d'abord : ses dessins, copiés, suivent dans le plan réuni sans
        // emporter le lien.
        self.detacher_plan(index + 1);
        let next = self.panels.remove(index + 1);
        let versions_suivant = self.versions.remove(&next.id);
        self.absorber_versions(self.panels[index].id, versions_suivant);
        self.forget_joint_reserves(self.panels[index].id, &next);
        let next_len = next.frame_count();
        let a_len = self.panels[index].frame_count();
        // Les commentaires du suivant suivent ses images.
        self.rehome_comments(next.id, self.panels[index].id, next_len, a_len);
        self.rehome_references(next.id, self.panels[index].id);
        self.annules_suivre_jonction(next.id, self.panels[index].id);
        let a = &mut self.panels[index];
        a.camera = camera::join_keys(&a.camera, a_len, cam_a, &next.camera, next_len, cam_b);
        // Les calques de toute la scène sont déjà dans le premier : seuls les autres se suivent,
        // rang pour rang parmi les calques propres au panneau.
        let own: Vec<usize> = (0..a.layers.len()).filter(|&k| a.layers[k].scene_wide.is_none()).collect();
        let mut renum: HashMap<u64, u64> = HashMap::new();
        for (i, lb) in next.layers.into_iter().filter(|l| l.scene_wide.is_none()).enumerate() {
            if let Some(&k) = own.get(i).filter(|&&k| a.layers[k].kind == lb.kind) {
                renum.insert(lb.id, a.layers[k].id);
                let la = &mut a.layers[k];
                // Deux calques de couleur qui se suivent : les couleurs du second gardent leur trait.
                if let (Some(ca), Some(cb)) = (la.couleur.as_mut(), lb.couleur.as_ref()) {
                    ca.dessins.extend(cb.dessins.iter().map(|(t, c)| (*t, *c)));
                    ca.griffonnages.extend(cb.griffonnages.iter().map(|(p, g)| (*p, *g)));
                    ca.remplissage |= cb.remplissage;
                }
                la.append_at(a_len, lb.frames, &lb.roles);
                la.join_pos_keys(a_len, &lb.pos_keys, lb.transform, next_len);
                // (Les clés réunies ne suivent plus un seul chemin : elles restent à la main.)
                la.trajectoire = None;
                la.join_opacity_keys(a_len, &lb.opacity_keys, lb.opacity, next_len);
                la.trim();
            } else {
                let mut l = lb;
                l.insert(0, None, a_len);
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
        // La fin rognée du second est celle du panneau réuni.
        self.move_tail_reserve(next.id, id, &renum);
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

    /// Fusionne le calque `layer` du panneau `index` dans celui du dessous (`fusioncalques.rs`) : le résultat est ce que
    /// l'écran montrait des deux calques (mode de fusion, opacité, effets, aplat, transformation), image par image. Les
    /// dessins fusionnés sont de nouveaux dessins : ceux d'avant restent intacts (l'annulation y revient). Deux calques
    /// vectoriels sans rien à cuire donnent un calque vectoriel ; sinon le résultat est bitmap, à 100 %.
    pub fn merge_layer_down(&mut self, index: usize, layer: usize) -> Result<(), String> {
        crate::fusioncalques::merge_down(self, index, layer)
    }

    /// Un petit document pour fusionner en tâche de fond (`fusiontache.rs`) : la feuille, et un plan qui n'a que les calques
    /// `lis` du plan `index` avec leurs dessins (partagés à la copie). Ses numéros neufs continuent ceux de ce document.
    pub(crate) fn mini_pour_fusion(&self, index: usize, lis: &[usize]) -> Document {
        let mut mini = Document::new(self.width, self.height, self.fps);
        mini.frame = self.frame;
        mini.paper = self.paper;
        mini.next_id = self.next_id;
        mini.id_prefix = self.id_prefix;
        let source = &self.panels[index];
        let layers: Vec<Layer> = lis.iter().map(|&l| source.layers[l].clone()).collect();
        for l in &layers {
            for id in l.frames.iter().flatten() {
                if let Some(c) = self.cels.get(id) {
                    mini.cels.insert(*id, c.clone());
                }
            }
        }
        mini.panels[0].length = source.length;
        mini.panels[0].layers = layers;
        mini
    }    /// Ajoute un dessin existant (collage) sous un nouvel identifiant.
    pub fn add_cel(&mut self, mut cel: Cel) -> CelId {
        cel.version = 0;
        let id = self.alloc();
        self.cels.insert(id, cel);
        id
    }

    pub(crate) fn insert_and_normalize(&mut self, at: usize, panels: Vec<Panel>) -> usize {
        let first = panels[0].id;
        let at = at.min(self.panels.len());
        self.panels.splice(at..at, panels);
        self.normalize_keeping(first)
    }

    /// Remet en ordre, et renvoie le nouvel indice du panneau `id`.
    pub(crate) fn normalize_keeping(&mut self, id: u64) -> usize {
        self.normalize();
        self.panels.iter().position(|p| p.id == id).unwrap_or(0)
    }

    /// Un plan peut-il devenir un plan en espace (`Panel::espace`) ? Il faut que le cadre tienne dans la feuille : la zone
    /// filmée d'un plan en espace est le cadre au centre de la feuille (`docs/ESPACE-LIBRE.md`, A0 point 2). Toujours vrai
    /// pour un projet créé ou redimensionné par FEANOR (la feuille fait au moins le double du cadre).
    pub fn accepte_plan_en_espace(&self) -> bool {
        self.frame[0] <= self.width && self.frame[1] <= self.height
    }

    /// Un panneau vide de deux secondes dans la scène `scene`, avec un calque vectoriel.
    pub fn make_panel(&mut self, scene: u64) -> Panel {
        Panel {
            id: self.alloc(),
            scene,
            length: crate::dureeplans::images(self.duree_plan_neuf, self.fps),
            layers: vec![Layer::new("Dessin", LayerKind::Vector)],
            action: String::new(),
            dialogue: String::new(),
            notes: String::new(),
            camera: Vec::new(),
            saccade: camera::Saccade::default(),
            effet_camera: None,
            marks: Vec::new(),
            cues: Cues::default(),
            color: None,
            espace: false,
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
        let old_ids: Vec<u64> = panel.layers.iter().map(|l| l.id).collect();
        let mut copies: HashMap<CelId, CelId> = HashMap::new();
        for layer in &mut panel.layers {
            layer.id = fresh_layer_id();
            // (Une copie ne reprend pas le lien d'une planche-mère : lier se demande, `partage.rs`.)
            layer.partage = None;
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
        self.copier_griffonnages(&self.panels[index].layers.clone(), &mut copies);
        relink_copies(&mut panel.layers, &old_ids, &copies);
        self.renumeroter_les_copies(copies.values().copied());
        panel
    }

    /// Les dessins copiés `cels` (un plan dupliqué ou collé) : leurs traits prennent des numéros neufs (partie 125). Une copie n'est pas une part du trait
    /// d'origine. (Pas dans `duplicate_cel` : la copie d'un dessin tenu, coupée à une image, est le même dessin à la même place, et ses caches suivent son
    /// contenu.)
    pub(crate) fn renumeroter_les_copies(&mut self, cels: impl Iterator<Item = CelId>) {
        for c in cels {
            if let Some(v) = self.cels.get_mut(&c).and_then(|c| c.vector.as_mut()) {
                crate::vector::renumeroter(&mut v.elements);
            }
        }
    }

    /// Avant `relink_copies` : les calques de couleur de `layers` (les calques d'origine) dont un dessin de peinture a été
    /// copié (`copies`) reçoivent aussi une copie de ses griffonnages — jamais un dessin partagé entre l'original et sa
    /// copie, qu'on retoucherait ensemble.
    pub(crate) fn copier_griffonnages(&mut self, layers: &[Layer], copies: &mut HashMap<CelId, CelId>) {
        // (Le remplissage de la copie, à l'écran en attendant le sien : celui de l'original, xcache::heriter.)
        self.copier_griffonnages_avec(layers, copies, |d, g| {
            let copie = d.duplicate_cel(g);
            crate::fxcache::heriter(g, copie);
            Some(copie)
        });
    }

    /// `copier_griffonnages`, la copie d'un dessin de griffonnages étant faite par `make` (rien : le dessin manque, la copie
    /// n'en a pas).
    pub(crate) fn copier_griffonnages_avec(
        &mut self,
        layers: &[Layer],
        copies: &mut HashMap<CelId, CelId>,
        mut make: impl FnMut(&mut Document, CelId) -> Option<CelId>,
    ) {
        let pairs: Vec<(CelId, CelId)> = crate::calquecouleur::griffonnages_de(layers).collect();
        for (peinture, g) in pairs {
            if copies.contains_key(&peinture) && !copies.contains_key(&g) {
                if let Some(d) = make(self, g) {
                    copies.insert(g, d);
                }
            }
        }
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
            .flat_map(crate::calquecouleur::dessins_du_calque)
            .filter_map(|id| self.cels.get(&id).map(|c| (id, c.clone())))
            .collect();
        Some(PanelClip { panels, cels, origine: crate::adaptation::Origine::de(self) })
    }

    /// Colle les panneaux copiés juste après le panneau `index`, dans sa scène, chacun avec ses
    /// propres dessins. Un calque de toute la scène suit la règle d'un panneau déplacé : collé
    /// dans sa scène, il en garde le dessin ; ailleurs, il est retiré et le panneau prend le fond
    /// de sa nouvelle scène (`sync_scene_layers`). Renvoie l'indice du premier panneau collé, ou
    /// rien si la copie vient d'un projet d'une autre taille. Les panneaux collés restent dans la
    /// scène : ce n'est pas un collage de plans (l'application passe par `paste_plans`), et à la
    /// relecture d'un projet enregistré ainsi, ils sont mis bout à bout avec le panneau voisin.
    pub fn paste_panels(&mut self, clip: &PanelClip, index: usize) -> Option<usize> {
        if clip.panels.is_empty() || clip.origine.feuille != [self.width, self.height] {
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
            let old_ids: Vec<u64> = panel.layers.iter().map(|l| l.id).collect();
            for layer in &mut panel.layers {
                layer.id = fresh_layer_id();
                // (Un plan collé ne reprend pas le lien d'une planche-mère, `partage.rs`.)
                layer.partage = None;
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
            self.copier_griffonnages_avec(&panel.layers.clone(), &mut copies, |d, g| clip.cels.get(&g).map(|c| d.add_cel(c.clone())));
            relink_copies(&mut panel.layers, &old_ids, &copies);
            self.renumeroter_les_copies(copies.values().copied());
            pasted.push(panel);
        }
        Some(self.insert_and_normalize(index + 1, pasted))
    }

    /// Donne la couleur de repère `color` (rang dans `marks::COLORS`, ou rien) aux plans `ids`.
    /// Renvoie vrai si au moins un plan change ; un rang hors palette est refusé.
    pub fn set_panels_color(&mut self, ids: &[u64], color: Option<u8>) -> bool {
        if color.is_some_and(|k| k as usize >= crate::marks::COLORS.len()) {
            return false;
        }
        let mut changed = false;
        for p in self.panels.iter_mut().filter(|p| ids.contains(&p.id)) {
            changed |= p.color != color;
            p.color = color;
        }
        changed
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
    pub(crate) fn append_next(&mut self, index: usize) -> Option<String> {
        self.append_next_with(index, &JoinOpts::default())
    }

    /// Comme `append_next`, avec ce qu'on a répondu à la question des calques de même nom (`JoinOpts`).
    pub(crate) fn append_next_with(&mut self, index: usize, opts: &JoinOpts) -> Option<String> {
        let last = |d: &Document, i: usize| d.panels[i].frame_count() - 1;
        // Les effets de caméra vivants des deux plans deviennent d'abord leurs clés, sans changer l'image (partie 129).
        self.cuire_effets_vivants(&[index, index + 1]);
        // (Les caméras douces, celles des clés : la postérisation temporelle n'est pas une pose.)
        let cams_a = (self.panel_camera_douce(index, 0), self.panel_camera_douce(index, last(self, index)));
        let cams_b = (self.panel_camera_douce(index + 1, 0), self.panel_camera_douce(index + 1, last(self, index + 1)));
        self.ranger_plan_partage(self.panels[index + 1].id, true);
        let b = self.panels.remove(index + 1);
        // Les versions rangées du plan absorbé restent au plan réuni, sous des numéros neufs (rien n'est perdu).
        let versions_b = self.versions.remove(&b.id);
        let hote = self.panels[index].id;
        let r = self.append_plan(index, b, cams_a, cams_b, opts);
        self.absorber_versions(hote, versions_b);
        r
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
    /// Renvoie un avertissement si quelque chose de `b` n'a pas pu être gardé (plus rien aujourd'hui : l'ancien repère 3D du plan a disparu).
    fn append_plan(&mut self, index: usize, mut b: Panel, cams_a: (Camera, Camera), cams_b: (Camera, Camera), opts: &JoinOpts) -> Option<String> {
        let mark_id = self.alloc();
        self.forget_joint_reserves(self.panels[index].id, &b);
        let a_len = self.panels[index].frame_count();
        let b_len = b.frame_count();
        // Les commentaires de `b` suivent ses images, au bout du plan.
        self.rehome_comments(b.id, self.panels[index].id, b_len, a_len);
        self.rehome_references(b.id, self.panels[index].id);
        self.annules_suivre_jonction(b.id, self.panels[index].id);
        let a = &mut self.panels[index];

        // Caméra : chaque partie tient sa caméra jusqu'à ses bords.
        if !(a.camera.is_empty() && b.camera.is_empty() && cams_a == cams_b) {
            let mut keys = held_keys(&a.camera, a_len, cams_a);
            keys.extend(held_keys(&b.camera, b_len, cams_b).into_iter().map(|k| CameraKey { frame: k.frame + a_len, ..k }));
            a.camera = keys;
        }

        // Calques : par nom, puis par rang, s'ils sont réglés pareil (`layer_targets`). Ce qu'on a répondu à la question des calques de même
        // nom (`fusionnoms.rs`) : renommés d'avance, ou gardés à part (le second renommé « Dessin_2 »…) ; ceux-là ne se réunissent pas par rang.
        let apart = opts.layers.prepare(&a.layers, &mut b.layers);
        let target = layer_targets(&a.layers, &b.layers, &apart);
        for l in &mut a.layers {
            l.frames.resize(a_len, None);
            l.roles.retain(|f, _| *f < a_len);
        }
        // Calque d'arrivée de chaque calque de `b`, par son id (les ajouts décalent les rangs).
        let mut dest: Vec<u64> = Vec::with_capacity(b.layers.len());
        let ids_b: Vec<u64> = b.layers.iter().map(|l| l.id).collect();
        // (Le calque d'arrivée par son id, pas par son rang : un calque de `b` ajouté plus bas décale les rangs de `a`, et les images
        // d'un calque bitmap partaient dans le calque vectoriel ajouté sous lui. Passe de débogage au hasard, 2026-10-23.)
        let cible: Vec<Option<u64>> = target.iter().map(|t| t.map(|k| a.layers[k].id)).collect();
        // Les sources des calques de couleur de `b` réunis à un calque de couleur de `a` : remises sur les calques d'arrivée plus bas.
        let mut sources_b: Vec<(u64, Vec<u64>)> = Vec::new();
        // Les dessins que montrent le trait de `a` et celui de `b` (par calque de couleur de `b`) : la couleur d'un dessin de trait
        // qui n'est plus montré garde son entrée dans la table (reste d'un trait effacé), et elle écrasait celle d'un dessin que
        // `a` montre encore (passe de débogage au hasard, 2026-10-24).
        let montres = |layers: &[Layer], trait_id: u64| -> BTreeSet<CelId> {
            layers.iter().find(|l| l.id == trait_id).map(|t| t.frames.iter().flatten().copied().collect()).unwrap_or_default()
        };
        let de_a_et_b: Vec<Option<(BTreeSet<CelId>, BTreeSet<CelId>)>> = (0..b.layers.len())
            .map(|j| {
                let (k, cb) = (target[j]?, b.layers[j].couleur.as_ref()?);
                let ca = a.layers[k].couleur.as_ref()?;
                Some((montres(&a.layers, ca.trait_id), montres(&b.layers, cb.trait_id)))
            })
            .collect();
        for (j, mut lb) in b.layers.into_iter().enumerate() {
            lb.frames.truncate(b_len);
            lb.roles.retain(|f, _| *f < b_len);
            match cible[j].and_then(|id| a.layers.iter().position(|l| l.id == id)) {
                Some(k) => {
                    // Deux calques de couleur réunis : la couleur de chaque dessin du trait de `b` et ses griffonnages suivent.
                    if let (Some(ca), Some(cb)) = (a.layers[k].couleur.as_mut(), lb.couleur.take()) {
                        let (de_a, de_b) = de_a_et_b[j].clone().unwrap_or_default();
                        for (trait_cel, couleur_cel) in cb.dessins {
                            // (Celle de `b` l'emporte seulement si `b` montre ce dessin de trait et pas `a`.)
                            if !ca.dessins.contains_key(&trait_cel) || (de_b.contains(&trait_cel) && !de_a.contains(&trait_cel)) {
                                ca.dessins.insert(trait_cel, couleur_cel);
                            }
                        }
                        for (trait_cel, g) in cb.griffonnages {
                            if !ca.griffonnages.contains_key(&trait_cel) || (de_b.contains(&trait_cel) && !de_a.contains(&trait_cel)) {
                                ca.griffonnages.insert(trait_cel, g);
                            }
                        }
                        sources_b.push((a.layers[k].id, cb.sources));
                    }
                    a.layers[k].append_at(a_len, lb.frames, &lb.roles);
                    dest.push(a.layers[k].id);
                }
                None => {
                    let at = match j.checked_sub(1).and_then(|p| dest.get(p)) {
                        Some(below) => a.layers.iter().position(|l| l.id == *below).map_or(a.layers.len(), |k| k + 1),
                        None => 0,
                    };
                    lb.insert(0, None, a_len);
                    dest.push(lb.id);
                    a.layers.insert(at, lb);
                }
            }
        }
        for l in &mut a.layers {
            l.trim();
        }
        // Un calque de `b` ajouté tel quel garde son dossier : celui où le dossier de `b` est allé, s'il s'est réuni à un dossier
        // de même nom (sinon le calque désignait un dossier disparu, passe de débogage au hasard, 2026-10-22). Puis chaque dossier
        // rassemble ses calques juste sous lui (`folders::normalize`).
        let ajoutes: std::collections::HashSet<u64> = ids_b.iter().zip(&dest).filter(|(b, d)| b == d).map(|(b, _)| *b).collect();
        let vers: HashMap<u64, u64> = ids_b.iter().copied().zip(dest.iter().copied()).collect();
        for l in a.layers.iter_mut().filter(|l| ajoutes.contains(&l.id)) {
            if let Some(p) = l.parent {
                l.parent = vers.get(&p).copied().or(Some(p));
            }
            // De même pour le trait d'un calque de couleur ajouté tel quel, et ses sources : le calque où le leur est allé.
            if let Some(c) = l.couleur.as_mut() {
                if c.trait_id != 0 {
                    c.trait_id = vers.get(&c.trait_id).copied().unwrap_or(c.trait_id);
                }
                for s in &mut c.sources {
                    *s = vers.get(s).copied().unwrap_or(*s);
                }
            }
        }
        for (id, sources) in sources_b {
            if let Some(c) = a.layers.iter_mut().find(|l| l.id == id).and_then(|l| l.couleur.as_mut()) {
                for s in sources.into_iter().map(|s| vers.get(&s).copied().unwrap_or(s)) {
                    if !c.sources.contains(&s) {
                        c.sources.push(s);
                    }
                }
            }
        }
        if a.layers.iter().any(|l| l.folder || l.parent.is_some()) {
            crate::folders::normalize(&mut a.layers);
        }

        // Le début de `b` : une marque d'image avec ses textes ; puis ses propres marques.
        a.marks.push(ImageMark { id: mark_id, frame: a_len, color: 0, action: b.action, dialogue: b.dialogue, notes: b.notes });
        a.marks.extend(b.marks.into_iter().map(|m| ImageMark { frame: m.frame + a_len, ..m }));
        a.length = a_len + b_len;
        let a_id = a.id;
        self.reanchor_notes(b.id, mark_id);
        // La fin rognée de `b` est celle du plan réuni (ses calques ont changé de numéro en se joignant à ceux de `a`).
        self.move_tail_reserve(b.id, a_id, &vers);
        None
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

    /// Réunit les plans `ids` en un seul, le premier (Anthony, 2026-10-17 : « quand deux plans
    /// ou plus sont sélectionnés, l'option de fusionner les plans doit être possible ») : comme
    /// `join_next_plan` répété, mais sans laisser de marque d'image aux jointures (« fusionner
    /// les deux plans laisse un repère sur la timeline, tu peux les enlever ? ») : les textes et
    /// les notes de la planche du plan fusionné passent à l'image de storyboard d'avant. Ses
    /// propres marques restent. Ils doivent être au moins deux, se suivre et être dans une même
    /// séquence ; sinon rien. Renvoie l'indice du plan réuni.
    pub fn join_plans(&mut self, ids: &[u64]) -> Option<usize> {
        self.join_plans_with(ids, &JoinOpts::default())
    }

    /// `join_plans`, avec la réponse à la question des calques de même nom (`JoinOpts`, `fusionnoms.rs`).
    pub fn join_plans_with(&mut self, ids: &[u64], opts: &JoinOpts) -> Option<usize> {
        let mut at: Vec<usize> = ids.iter().filter_map(|id| self.panels.iter().position(|p| p.id == *id)).collect();
        at.sort_unstable();
        at.dedup();
        if at.len() < 2 || at.windows(2).any(|w| w[1] != w[0] + 1) {
            return None;
        }
        let seq = self.sequence_of(at[0]);
        if at.iter().any(|&i| self.sequence_of(i) != seq) {
            return None;
        }
        let first = self.panels[at[0]].id;
        // (Avant de réunir le premier couple : un refus au milieu laisserait des plans déjà réunis, sans étape d'annulation. Les versions
        // rangées de tous ces plans se retrouvent au plan réuni : leur total ne dépasse pas le maximum.)
        let tous: Vec<u64> = at.iter().map(|&i| self.panels[i].id).collect();
        if self.refuser_si_trop_de_versions(&tous) {
            return None;
        }
        for _ in 1..at.len() {
            let i = self.panels.iter().position(|p| p.id == first)?;
            self.join_next_plan_without_mark_with(i, opts)?;
        }
        self.panels.iter().position(|p| p.id == first)
    }

    /// `join_next_plan` sans marque d'image à la jointure (Anthony, 2026-10-17, pour « Réunir le
    /// plan et le suivant » comme pour « Fusionner les plans choisis ») ; voir `drop_joint_mark`.
    pub fn join_next_plan_without_mark(&mut self, index: usize) -> Option<usize> {
        self.join_next_plan_without_mark_with(index, &JoinOpts::default())
    }

    /// `join_next_plan_without_mark`, avec la réponse à la question des calques de même nom (`JoinOpts`).
    pub fn join_next_plan_without_mark_with(&mut self, index: usize, opts: &JoinOpts) -> Option<usize> {
        let first = self.panels.get(index)?.id;
        let joint = self.panels[index].frame_count();
        self.join_next_plan_with(index, opts)?;
        let i = self.panels.iter().position(|p| p.id == first)?;
        self.drop_joint_mark(i, joint);
        Some(i)
    }

    /// Retire la marque d'image laissée par une jointure à l'image `joint` du plan `index` : ses
    /// textes s'ajoutent à ceux de l'image de storyboard d'avant (la marque précédente, sinon le
    /// plan), ses notes de la planche s'y attachent.
    pub(crate) fn drop_joint_mark(&mut self, index: usize, joint: usize) {
        let Some(k) = self.panels[index].marks.iter().position(|m| m.frame == joint) else { return };
        let m = self.panels[index].marks.remove(k);
        let append = |dst: &mut String, src: &str| {
            if !src.trim().is_empty() {
                if !dst.trim().is_empty() {
                    dst.push('\n');
                }
                dst.push_str(src);
            }
        };
        let p = &mut self.panels[index];
        let target = match p.marks.iter_mut().rev().find(|x| x.frame < joint) {
            Some(prev) => {
                append(&mut prev.action, &m.action);
                append(&mut prev.dialogue, &m.dialogue);
                append(&mut prev.notes, &m.notes);
                prev.id
            }
            None => {
                append(&mut p.action, &m.action);
                append(&mut p.dialogue, &m.dialogue);
                append(&mut p.notes, &m.notes);
                p.id
            }
        };
        self.reanchor_notes(m.id, target);
    }

    /// Réunit le plan `index` et le suivant de la même séquence : les images du suivant viennent
    /// après (voir `append_plan`). Rien si le suivant est dans une autre séquence.
    pub fn join_next_plan(&mut self, index: usize) -> Option<usize> {
        self.join_next_plan_with(index, &JoinOpts::default())
    }

    /// `join_next_plan`, avec la réponse à la question des calques de même nom (`JoinOpts`).
    pub fn join_next_plan_with(&mut self, index: usize, opts: &JoinOpts) -> Option<usize> {
        if index + 1 >= self.panels.len() || self.sequence_of(index) != self.sequence_of(index + 1) {
            return None;
        }
        // (Les versions rangées du plan absorbé deviennent des versions du plan réuni : `absorber_versions`.)
        if self.refuser_si_trop_de_versions(&[self.panels[index].id, self.panels[index + 1].id]) {
            return None;
        }
        // (Le décor du premier plan se tient sur la suite, ou la suite repart d'un décor vide : `decor.rs`.)
        self.join_with_decor_with(index, opts);
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
        // Les versions rangées du plan vont avec la première partie, ou avec la seconde si la coupe l'a demandé (`variantes::avec_cote`).
        let cote = crate::variantes::cote_choisi().unwrap_or(crate::variantes::Cote::Premiere);
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
        if cote == crate::variantes::Cote::Seconde {
            self.confier_versions_a_la_suite(first_id, id, scene);
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
        // Le dernier plan d'une séquence qui a d'autres versions de séquence ne la quitte pas (elles disparaîtraient).
        if !self.variantes.is_empty() {
            let kept: Vec<&Panel> = self.panels.iter().filter(|p| !ids.contains(&p.id)).collect();
            let at = anchor.and_then(|b| kept.iter().position(|p| p.id == b)).unwrap_or(kept.len());
            let sequence = |p: Option<&&Panel>| p.and_then(|p| self.scene(p.scene)).map(|s| s.sequence);
            let (left, right) = (sequence(at.checked_sub(1).and_then(|i| kept.get(i))), sequence(kept.get(at)));
            let partent: std::collections::HashSet<u64> = self
                .panels
                .iter()
                .filter(|p| ids.contains(&p.id))
                .filter(|p| {
                    let own = self.scene(p.scene).map(|s| s.sequence);
                    let cible = if own.is_some() && (own == left || own == right) { own } else { right.or(left) };
                    cible.is_some() && cible != own
                })
                .map(|p| p.id)
                .collect();
            if self.refuser_si_sequence_videe(|p| partent.contains(&p)) {
                return None;
            }
        }
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
    /// réglé comme le modèle, avec ses images (coupées à la durée du panneau ; plus court, il tient
    /// son dernier dessin). Le modèle est l'exemplaire du panneau `cur` s'il en a un (c'est lui
    /// qu'on modifie), sinon le premier trouvé. (`frame` ne sert plus depuis que le fond a ses
    /// clés.) Un exemplaire égaré dans une autre scène (panneau déplacé, scènes réunies) est
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
        // Modèles : l'exemplaire du panneau courant (sinon le premier) et son rang. Ses images
        // vont telles quelles dans chaque panneau de la scène (depuis le 2026-10-17 : ses clés
        // se posent et se déplacent dans le montage comme sur un autre calque), coupées à sa
        // durée ; plus court, il tient son dernier dessin.
        let _ = frame;
        let mut models: Vec<(Layer, usize)> = Vec::new();
        for (pi, p) in self.panels.iter().enumerate() {
            for (k, l) in p.layers.iter().enumerate() {
                let Some(w) = l.scene_wide else { continue };
                match models.iter_mut().find(|m| m.0.scene_wide.is_some_and(|v| v.key == w.key)) {
                    Some(m) if pi == cur => *m = (l.clone(), k),
                    Some(_) => {}
                    None => models.push((l.clone(), k)),
                }
            }
        }
        for p in &mut self.panels {
            let n = own_frame_count(p);
            for (model, rank) in &models {
                let Some(w) = model.scene_wide.filter(|w| w.scene == p.scene) else { continue };
                let mut frames = model.frames.clone();
                frames.truncate(n);
                if let Some(last) = frames.last().copied().flatten() {
                    frames.resize(n, Some(last));
                }
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
    pub(crate) fn adopt_scene_layers(&mut self, panels: &mut [Panel], scene: u64) {
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
        // (Un dessin tenu sur plusieurs images n'est compté qu'une fois par exposition : quatre fois moins de numéros à
        // trier pour un film animé à 2 et à 8, et c'est le fil de l'interface qui le fait à chaque enregistrement.)
        let mut vus: Vec<CelId> = Vec::new();
        for l in self.panels.iter().flat_map(|s| s.layers.iter()) {
            let mut avant = None;
            for id in l.frames.iter().flatten().copied() {
                if avant != Some(id) {
                    vus.push(id);
                    avant = Some(id);
                }
            }
        }
        let mut used: BTreeSet<CelId> = vus.into_iter().collect();
        // Le contenu rogné reste dans le projet (`rognage.rs`).
        for (_, r) in self.live_reserves() {
            used.extend(r.cels());
        }
        // Et celui des variantes rangées (`variantes.rs`).
        for r in self.reserves_de_variantes().chain(self.reserves_des_annules()) {
            used.extend(r.cels());
        }
        // Les autres versions des plans aussi (`planvers.rs`).
        for p in self.plans_ranges() {
            used.extend(p.layers.iter().flat_map(|l| l.frames.iter().flatten().copied()));
        }
        // Les griffonnages des calques de couleur (`griffonnage.rs`) : des dessins du projet, enregistrés avec le dessin
        // de peinture auquel ils répondent.
        let griffonnages: Vec<CelId> = self
            .panels
            .iter()
            .chain(self.plans_ranges())
            .flat_map(|p| crate::calquecouleur::griffonnages_de(&p.layers))
            .filter(|(peinture, g)| used.contains(peinture) && self.cels.contains_key(g))
            .map(|(_, g)| g)
            .collect();
        used.extend(griffonnages);
        used
    }

    /// Un plan sans calque en reçoit un vide : l'interface a toujours un calque courant (sans calque, la première image
    /// plantait, passe de débogage 2026-10-22). La lecture d'un projet en donne déjà un à chaque plan ; ceci garde la
    /// porte d'entrée de l'application (`App::load_doc`) pour un document venu d'ailleurs. Rend le nombre de plans réparés.
    pub fn un_calque_par_plan(&mut self) -> usize {
        let mut n = 0;
        for p in &mut self.panels {
            if p.layers.is_empty() {
                p.layers.push(Layer::new("Dessin", LayerKind::Vector));
                n += 1;
            }
        }
        n
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
            solo: false,
            volume: 1.0,
            clips: Vec::new(),
            locked: false,
        });
        id
    }

    /// Copie de la piste `track` (13e), avec ses extraits, juste en dessous d'elle. Renvoie son
    /// rang.
    pub fn duplicate_audio_track(&mut self, track: usize) -> Option<usize> {
        let mut copy = self.audio_tracks.get(track)?.clone();
        copy.id = self.alloc();
        copy.name = format!("{} copie", copy.name);
        copy.locked = false;
        for c in &mut copy.clips {
            c.id = self.alloc();
        }
        self.audio_tracks.insert(track + 1, copy);
        Some(track + 1)
    }

    /// L'extrait `id` est sur une piste verrouillée (13e).
    pub fn clip_locked(&self, id: u64) -> bool {
        self.find_clip(id).is_some_and(|(t, _)| self.audio_tracks[t].locked)
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
        clips.push(SoundClip::new(id, sound, start, 0, length));
        clips.sort_by_key(|c| c.start);
        Some(id)
    }

    /// Pose un extrait tel quel sur la piste `track` (14a, import) : `length` images du son
    /// `sound`, à partir de son image `offset`, à l'image `start` du montage. Rien si la piste
    /// n'existe pas.
    pub fn add_clip(&mut self, track: usize, sound: u64, start: usize, offset: usize, length: usize) -> Option<u64> {
        if track >= self.audio_tracks.len() {
            return None;
        }
        let id = self.alloc();
        let clips = &mut self.audio_tracks[track].clips;
        clips.push(SoundClip::new(id, sound, start, offset, length.max(1)));
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
        let mut right = SoundClip {
            id: right_id,
            sound: clip.sound,
            start: at,
            offset: clip.offset + cut,
            length: clip.length - cut,
            fx: clip.fx.clone(),
        };
        // Les effets suivent la coupe : volume, égaliseur et écho aux deux ; le fondu d'entrée reste
        // à gauche, celui de sortie et l'enchaîné avec le suivant passent à droite ; les clés de volume
        // se partagent (chaque morceau garde la valeur de la courbe à la coupe).
        let cut_t = cut as f32;
        let at_cut = clip.fx.key_gain(cut_t);
        let has_keys = !clip.fx.keys.is_empty();
        right.fx.fade_in = 0;
        let clips = &mut self.audio_tracks[t].clips;
        clips[c].length = cut;
        clips[c].fx.fade_out = 0;
        clips[c].fx.xfade = 0;
        if has_keys {
            clips[c].fx.keys.retain(|k| k.t < cut_t);
            clips[c].fx.set_key(cut_t, at_cut);
            right.fx.keys = clip.fx.keys.iter().filter(|k| k.t > cut_t).map(|k| crate::audiofx::VolumeKey { t: k.t - cut_t, v: k.v }).collect();
            right.fx.set_key(0.0, at_cut);
        }
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
        // Une poignée de bord découvre ou cache du son sans le bouger : les clés de volume restent
        // sous les mêmes images du son (leur temps est compté depuis le début de l'extrait).
        let shift = start as f32 - moved.start as f32;
        if !keep_offset && shift != 0.0 && !moved.fx.keys.is_empty() {
            let lead = moved.fx.key_gain(shift.max(0.0));
            for k in &mut moved.fx.keys {
                k.t -= shift;
            }
            if shift > 0.0 {
                // Les clés qui passent avant le nouveau début sortent ; la courbe garde alors la
                // valeur qu'elle avait là, par une clé au bord.
                let n = moved.fx.keys.len();
                moved.fx.keys.retain(|k| k.t > 0.0);
                if moved.fx.keys.len() < n && moved.fx.keys.first().is_none_or(|k| k.t > 0.0) {
                    moved.fx.set_key(0.0, lead);
                }
            }
        }
        moved.start = start;
        moved.length = end - start;
        moved.offset = offset as usize;
        let clips = &mut self.audio_tracks[track].clips;
        clips.push(moved);
        clips.sort_by_key(|c| c.start);
        true
    }
}
