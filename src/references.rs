//! Références visuelles (Anthony, 2026-10-19 : une fenêtre « Références » pour regarder des images et
//! des vidéos pendant qu'on dessine, sur un deuxième écran si besoin).
//!
//! - **Ce que c'est** : un tableau libre (moodboard) d'images et de vidéos, rangé par projet. Chaque
//!   référence a un nom, des étiquettes, une place sur le tableau (centre, échelle, rotation,
//!   miroirs, niveaux de gris) et, si on le veut, des plans auxquels elle est liée. Une vidéo garde
//!   en plus ses réglages de lecture (entrée, sortie, boucle, vitesse, « suivre la tête de lecture »
//!   avec un décalage).
//! - **Rangement** : `Document::references` et `Structure::references` : une étape d'annulation
//!   (`Edit::Structure`) les rétablit avec le reste, avec un nom en français. Seules les
//!   informations sont dans la structure ; les fichiers sont à côté.
//! - **Fichiers** : `references.json` à côté du manifeste, sans changer `VERSION` du projet, et le
//!   dossier `references/` : une image y est copiée telle quelle (`<id>.png` ou `<id>.jpg`), une vidéo
//!   y est un dossier d'images extraites par ffmpeg (`<numéro>/000001.jpg`…, le chemin de l'import
//!   des calques vidéo, `video.rs`). Un projet sans ce fichier n'a pas de références, et une version
//!   de SHOT qui ne le connaît pas le laisse là (elle n'efface que le contenu de `dessins/`, `sons/`,
//!   `modeles/`, `textures/`).
//! - **Fiabilité** (comme `commentaires.json`) : `references.json` s'écrit avec le reste de
//!   l'enregistrement (`SaveJob`, atomique), jamais sur un fichier que cette version ne sait pas lire
//!   (elle le met de côté sous `.illisible`, `project::write_sidecar`), et aucun fichier de
//!   `references/` n'est effacé quand ce fichier n'a pas pu être lu. Un fichier qui n'est plus cité
//!   n'est effacé qu'à l'enregistrement d'une séance suivante : annuler une suppression après
//!   avoir enregistré retrouve l'image (`touch`).
//! - **Avant le premier enregistrement** : le projet n'a pas de dossier. Les fichiers attendent dans
//!   un dossier temporaire (`Reference::home`) et sont recopiés à l'enregistrement. Un projet déjà
//!   enregistré reçoit ses nouvelles images tout de suite dans `references/`.
//! - L'écran est dans `referencesui.rs`.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use eframe::egui::{pos2, vec2, Pos2, Rect, Vec2};
use serde::{Deserialize, Serialize};

use crate::document::Document;

/// Nom du fichier, dans le dossier du projet.
pub(crate) const FILE: &str = "references.json";
/// Le dossier des fichiers, dans le dossier du projet.
pub(crate) const DIR: &str = "references";
const FORMAT: &str = "shot-references";
/// Le format de `references.json` quand rien de nouveau n'y est ; la planche (post-it, textes, titres, cadres,
/// recadrage, opacité) l'écrit en `VERSION_PLANCHE` : une ancienne version de SHOT, qui perdrait ces éléments en
/// réécrivant le fichier, le met de côté sans le toucher (`parse` refuse un format plus récent).
const VERSION: u32 = 1;
const VERSION_PLANCHE: u32 = 2;
/// Formats d'image acceptés (ceux que le programme sait lire).
pub(crate) const IMAGE_EXTS: [&str; 3] = ["png", "jpg", "jpeg"];
/// Côté le plus long d'une référence neuve sur le tableau, en unités du tableau.
pub(crate) const BOARD_SIDE: f32 = 360.0;
pub(crate) const SCALE_MIN: f32 = 0.005;
pub(crate) const SCALE_MAX: f32 = 50.0;
/// Les images des vidéos de références ont au plus cette taille (largeur, hauteur) : une référence se
/// regarde, elle ne s'exporte pas.
pub(crate) const VIDEO_FRAME: [usize; 2] = [1280, 720];
const COORD_MAX: f32 = 1.0e6;
const NAME_MAX: usize = 200;
const TAG_MAX: usize = 40;
const TAGS_PER_REF: usize = 32;
pub(crate) const SPEED_MIN: f32 = 0.1;
pub(crate) const SPEED_MAX: f32 = 4.0;

// ---------------------------------------------------------------------- la référence

fn one() -> f32 {
    1.0
}

fn is_zero_u8(n: &u8) -> bool {
    *n == 0
}

fn is_zero_u64(n: &u64) -> bool {
    *n == 0
}

fn is_one(v: &f32) -> bool {
    *v >= 1.0
}

/// Ce qu'un élément du tableau est.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// Une image ou une vidéo (un fichier dans `references/`).
    #[default]
    #[serde(rename = "image")]
    Image,
    #[serde(rename = "note")]
    Note,
    #[serde(rename = "texte")]
    Text,
    #[serde(rename = "titre")]
    Title,
    #[serde(rename = "cadre")]
    Frame,
}

impl Kind {
    pub fn is_image(&self) -> bool {
        *self == Kind::Image
    }

    /// Un élément fait de texte : post-it, texte, titre.
    pub fn has_text(&self) -> bool {
        matches!(self, Kind::Note | Kind::Text | Kind::Title | Kind::Frame)
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Une image ou une vidéo du tableau.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    pub id: u64,
    #[serde(rename = "nom", default)]
    pub name: String,
    /// Nom du fichier (image) ou du dossier d'images (vidéo) dans `references/`.
    #[serde(rename = "fichier", default)]
    pub file: String,
    /// Taille de l'image en pixels (celle de la première image pour une vidéo).
    #[serde(rename = "taille")]
    pub size: [u32; 2],
    #[serde(rename = "etiquettes", default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Centre sur le tableau.
    pub x: f32,
    pub y: f32,
    /// Unités du tableau par pixel de l'image.
    #[serde(rename = "echelle", default = "one")]
    pub scale: f32,
    /// En degrés, dans le sens des aiguilles d'une montre.
    #[serde(rename = "rotation", default)]
    pub rotation: f32,
    #[serde(rename = "miroir_h", default, skip_serializing_if = "is_false")]
    pub flip_h: bool,
    #[serde(rename = "miroir_v", default, skip_serializing_if = "is_false")]
    pub flip_v: bool,
    #[serde(rename = "gris", default, skip_serializing_if = "is_false")]
    pub grey: bool,
    /// Les plans (leur numéro) auxquels la référence est liée.
    #[serde(rename = "plans", default, skip_serializing_if = "Vec::is_empty")]
    pub plans: Vec<u64>,
    #[serde(rename = "video", default, skip_serializing_if = "Option::is_none")]
    pub video: Option<Clip>,
    /// Ce que c'est : une image (ou une vidéo), un post-it, un texte, un titre ou un cadre (`planchref.rs`).
    /// Les éléments qui ne sont pas des images n'ont pas de fichier : `size` est leur taille sur le tableau
    /// (à l'échelle 1) et `scale` les agrandit avec leur texte.
    #[serde(rename = "type", default, skip_serializing_if = "Kind::is_image")]
    pub kind: Kind,
    /// Le texte d'un post-it, d'un texte, d'un titre, ou le nom affiché d'un cadre (le nom de la référence
    /// sert aux images).
    #[serde(rename = "texte", default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// La couleur (numéro dans `planchref::NOTE_COLORS`) d'un post-it, d'un texte ou d'un cadre.
    #[serde(rename = "couleur", default, skip_serializing_if = "is_zero_u8")]
    pub color: u8,
    /// Opacité de l'élément, de 0 à 1 (1 : opaque).
    #[serde(rename = "opacite", default = "one", skip_serializing_if = "is_one")]
    pub opacity: f32,
    /// Recadrage non destructif d'une image : le rectangle visible, de 0 à 1 dans l'image entière
    /// (gauche, haut, droite, bas, avant miroirs). Rien : l'image entière.
    #[serde(rename = "recadrage", default, skip_serializing_if = "Option::is_none")]
    pub crop: Option<[f32; 4]>,
    /// Identifiant unique au monde de l'élément (travail en équipe, plus tard : deux planches se fusionnent
    /// par ces identifiants, jamais par `id`, qui n'est unique que dans un projet).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub uid: String,
    /// Qui l'a posé, et quand (secondes depuis 1970) ; `modified` : la dernière modification.
    #[serde(rename = "auteur", default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    #[serde(rename = "cree", default, skip_serializing_if = "is_zero_u64")]
    pub created: u64,
    #[serde(rename = "modifie", default, skip_serializing_if = "is_zero_u64")]
    pub modified: u64,
    /// Le dossier où est ``file`` en ce moment : celui du projet (`references/`), ou un dossier
    /// temporaire avant le premier enregistrement. Pas dans le fichier.
    #[serde(skip)]
    pub home: Option<PathBuf>,
}

/// Une vidéo et ses réglages de lecture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    #[serde(rename = "images")]
    pub frames: usize,
    #[serde(rename = "ips")]
    pub fps: f64,
    /// Extension des images (`jpg`).
    pub ext: String,
    /// Première image jouée (depuis 0).
    #[serde(rename = "entree", default)]
    pub in_point: usize,
    /// Dernière image jouée, comprise ; rien : la dernière de la vidéo.
    #[serde(rename = "sortie", default, skip_serializing_if = "Option::is_none")]
    pub out_point: Option<usize>,
    /// Revient à l'entrée après la sortie (sinon la vidéo s'arrête sur sa dernière image jouée).
    #[serde(rename = "boucle", default, skip_serializing_if = "is_false")]
    pub looping: bool,
    #[serde(rename = "vitesse", default = "one")]
    pub speed: f32,
    /// L'image montrée suit la tête de lecture de SHOT.
    #[serde(rename = "suivre", default, skip_serializing_if = "is_false")]
    pub follow: bool,
    /// Décalage, en images du projet, entre la tête de lecture et le début de la vidéo.
    #[serde(rename = "decalage", default)]
    pub offset: i64,
}

impl Clip {
    /// Une vidéo de `frames` images à `fps` images par seconde, lue en entier.
    pub fn new(frames: usize, fps: f64, ext: &str) -> Clip {
        Clip { frames: frames.max(1), fps: if fps.is_finite() && fps > 0.0 { fps } else { 24.0 }, ext: ext.into(), in_point: 0, out_point: None, looping: false, speed: 1.0, follow: false, offset: 0 }
    }

    /// Première et dernière image jouées (comprises), toujours dans la vidéo, la première avant la
    /// dernière.
    pub fn range(&self) -> (usize, usize) {
        let last = self.frames.saturating_sub(1);
        let lo = self.in_point.min(last);
        let hi = self.out_point.unwrap_or(last).min(last).max(lo);
        (lo, hi)
    }

    /// Nombre d'images jouées entre l'entrée et la sortie.
    pub fn span(&self) -> usize {
        let (lo, hi) = self.range();
        hi - lo + 1
    }

    /// Ramène une position (en images, avec décimales) dans ce qui est joué : avec la boucle, elle
    /// tourne entre l'entrée et la sortie ; sans, elle reste entre les deux.
    pub fn wrap(&self, pos: f64) -> f64 {
        let (lo, hi) = self.range();
        let (lo, end) = (lo as f64, hi as f64 + 1.0);
        if !pos.is_finite() || pos < lo {
            return lo;
        }
        if pos < end {
            return pos;
        }
        if self.looping {
            lo + (pos - lo).rem_euclid(end - lo)
        } else {
            // Juste avant la fin : la dernière image jouée reste à l'écran.
            end - 1e-4
        }
    }

    /// L'image (depuis 0) d'une position en images, toujours entre l'entrée et la sortie.
    pub fn index_of(&self, pos: f64) -> usize {
        let (lo, hi) = self.range();
        ((self.wrap(pos) + 1e-6).floor().max(0.0) as usize).clamp(lo, hi)
    }

    /// L'image à montrer quand la tête de lecture du projet est à l'image `playhead` (cadence du
    /// projet `project_fps`) : la vidéo commence à l'image `-offset`, à sa vitesse, et tourne entre
    /// l'entrée et la sortie si la boucle est allumée.
    pub fn follow_index(&self, playhead: i64, project_fps: u32) -> usize {
        let seconds = (playhead as f64 + self.offset as f64) / project_fps.max(1) as f64;
        let (lo, _) = self.range();
        self.index_of(lo as f64 + seconds * self.fps * self.speed as f64)
    }

    /// La position après `dt` secondes de lecture, et si elle est arrivée au bout (sans boucle).
    pub fn advance(&self, pos: f64, dt: f64) -> (f64, bool) {
        let next = pos + dt * self.fps * self.speed as f64;
        let (_, hi) = self.range();
        if !self.looping && next >= hi as f64 + 1.0 {
            return (hi as f64 + 1.0 - 1e-4, true);
        }
        (self.wrap(next), false)
    }
}

impl Reference {
    /// Une image : sa place est choisie par l'appelant (`free_spot`).
    pub fn image(id: u64, name: &str, file: &str, size: [u32; 2], at: Pos2) -> Reference {
        Reference {
            id,
            name: name.into(),
            file: file.into(),
            size,
            tags: Vec::new(),
            x: at.x,
            y: at.y,
            scale: fit_scale(size),
            rotation: 0.0,
            flip_h: false,
            flip_v: false,
            grey: false,
            plans: Vec::new(),
            video: None,
            kind: Kind::Image,
            text: String::new(),
            color: 0,
            opacity: 1.0,
            crop: None,
            uid: String::new(),
            author: String::new(),
            created: 0,
            modified: 0,
            home: None,
        }
    }

    /// Quelque chose que seule la planche (format 2 du fichier) sait écrire.
    pub(crate) fn uses_board_features(&self) -> bool {
        self.kind != Kind::Image || self.crop.is_some() || self.opacity < 1.0
    }

    pub fn is_video(&self) -> bool {
        self.video.is_some()
    }

    /// Le rectangle visible de l'image, de 0 à 1 (gauche, haut, droite, bas) : l'image entière sans recadrage.
    pub fn crop_rect(&self) -> [f32; 4] {
        self.crop.unwrap_or([0.0, 0.0, 1.0, 1.0])
    }

    pub fn center(&self) -> Pos2 {
        pos2(self.x, self.y)
    }

    /// Moitié de la taille sur le tableau (avant rotation).
    pub fn half(&self) -> Vec2 {
        let c = self.crop_rect();
        vec2(self.size[0].max(1) as f32 * (c[2] - c[0]), self.size[1].max(1) as f32 * (c[3] - c[1])) * self.scale * 0.5
    }

    fn angle(&self) -> f32 {
        self.rotation.to_radians()
    }

    /// Un point du tableau, dans le repère de la référence (centre en 0, sans rotation, sans miroir).
    pub fn to_local(&self, p: Pos2) -> Vec2 {
        let d = p - self.center();
        let (s, c) = (-self.angle()).sin_cos();
        vec2(d.x * c - d.y * s, d.x * s + d.y * c)
    }

    /// L'inverse de `to_local`.
    pub fn from_local(&self, v: Vec2) -> Pos2 {
        let (s, c) = self.angle().sin_cos();
        self.center() + vec2(v.x * c - v.y * s, v.x * s + v.y * c)
    }

    /// Le point du tableau est-il sur la référence ?
    pub fn contains(&self, p: Pos2) -> bool {
        let (l, h) = (self.to_local(p), self.half());
        l.x.abs() <= h.x && l.y.abs() <= h.y
    }

    /// Les quatre coins sur le tableau : haut-gauche, haut-droite, bas-droite, bas-gauche.
    pub fn corners(&self) -> [Pos2; 4] {
        let h = self.half();
        [vec2(-h.x, -h.y), vec2(h.x, -h.y), vec2(h.x, h.y), vec2(-h.x, h.y)].map(|v| self.from_local(v))
    }

    /// Le plus petit rectangle (du tableau) qui contient la référence tournée.
    pub fn bounds(&self) -> Rect {
        let c = self.corners();
        let mut r = Rect::from_min_max(c[0], c[0]);
        for p in &c[1..] {
            r.extend_with(*p);
        }
        r
    }

    /// Où est un point du tableau dans l'image, de 0 à 1 (miroirs compris) ; rien s'il est dehors.
    pub fn unit_at(&self, p: Pos2) -> Option<(f32, f32)> {
        let (l, h) = (self.to_local(p), self.half());
        if h.x <= 0.0 || h.y <= 0.0 || l.x.abs() > h.x || l.y.abs() > h.y {
            return None;
        }
        let (mut u, mut v) = ((l.x / h.x + 1.0) * 0.5, (l.y / h.y + 1.0) * 0.5);
        if self.flip_h {
            u = 1.0 - u;
        }
        if self.flip_v {
            v = 1.0 - v;
        }
        // Le recadrage : le point est dans la partie visible, on le replace dans l'image entière.
        let c = self.crop_rect();
        let (u, v) = (c[0] + u.clamp(0.0, 1.0) * (c[2] - c[0]), c[1] + v.clamp(0.0, 1.0) * (c[3] - c[1]));
        Some((u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)))
    }

    /// Le point du tableau est-il sur ce qu'on peut attraper de l'élément ? Une image, un post-it, un texte :
    /// toute sa surface. Un cadre : son bandeau de titre et son bord seulement (le reste laisse passer le clic,
    /// pour se déplacer ou choisir ce qu'il contient).
    pub fn hit(&self, p: Pos2) -> bool {
        if self.kind != Kind::Frame {
            return self.contains(p);
        }
        let (l, h) = (self.to_local(p), self.half());
        if l.x.abs() > h.x || l.y.abs() > h.y {
            return false;
        }
        let strip = crate::planchref::FRAME_STRIP * self.scale;
        let edge = 10.0 * self.scale.max(0.2);
        l.y < -h.y + strip || l.x.abs() > h.x - edge || l.y > h.y - edge
    }

    /// Le rectangle de l'image entière, dans le repère de la référence (centre du visible en 0) : sert à
    /// montrer ce que le recadrage cache.
    pub fn full_local(&self) -> Rect {
        let c = self.crop_rect();
        let (w, h) = (self.size[0].max(1) as f32 * self.scale, self.size[1].max(1) as f32 * self.scale);
        let (cx, cy) = ((c[0] + c[2]) * 0.5, (c[1] + c[3]) * 0.5);
        let sx = if self.flip_h { -1.0 } else { 1.0 };
        let sy = if self.flip_v { -1.0 } else { 1.0 };
        let center = vec2(sx * (0.5 - cx) * w, sy * (0.5 - cy) * h);
        Rect::from_center_size(center.to_pos2(), vec2(w, h))
    }

    /// Change le recadrage sans que ce qu'on voit ne bouge : le centre suit.
    pub fn set_crop(&mut self, crop: Option<[f32; 4]>) {
        let (old, new) = (self.crop_rect(), crop.unwrap_or([0.0, 0.0, 1.0, 1.0]));
        let (w, h) = (self.size[0].max(1) as f32 * self.scale, self.size[1].max(1) as f32 * self.scale);
        let sx = if self.flip_h { -1.0 } else { 1.0 };
        let sy = if self.flip_v { -1.0 } else { 1.0 };
        let d = vec2(sx * ((new[0] + new[2]) - (old[0] + old[2])) * 0.5 * w, sy * ((new[1] + new[3]) - (old[1] + old[3])) * 0.5 * h);
        let c = self.from_local(d);
        (self.x, self.y) = (c.x, c.y);
        self.crop = crop.filter(|c| *c != [0.0, 0.0, 1.0, 1.0]);
    }

    /// Le pixel de l'image sous un point du tableau (`size` : la taille de l'image décodée, qui peut
    /// être plus petite que l'originale).
    pub fn pixel_at(&self, p: Pos2, size: [usize; 2]) -> Option<[usize; 2]> {
        let (u, v) = self.unit_at(p)?;
        let at = |t: f32, n: usize| ((t * n as f32) as usize).min(n.saturating_sub(1));
        (size[0] > 0 && size[1] > 0).then(|| [at(u, size[0]), at(v, size[1])])
    }

    /// La couleur vue sous un point du tableau, lue dans `rgba` (image décodée de taille `size`) : ce
    /// que l'œil voit, donc en gris si la référence est en niveaux de gris. Rien hors de l'image ou
    /// sur un pixel transparent.
    pub fn sample(&self, rgba: &[u8], size: [usize; 2], p: Pos2) -> Option<[u8; 3]> {
        let [x, y] = self.pixel_at(p, size)?;
        let i = (y * size[0] + x) * 4;
        let px = rgba.get(i..i + 4)?;
        if px[3] == 0 {
            return None;
        }
        let rgb = [px[0], px[1], px[2]];
        Some(if self.grey { [luma(rgb); 3] } else { rgb })
    }

    /// Le fichier (image) ou le dossier d'images (vidéo) de la référence.
    pub fn path(&self) -> Option<PathBuf> {
        (!self.file.is_empty()).then(|| self.home.as_ref().map(|h| h.join(&self.file))).flatten()
    }

    /// Le fichier de ce qu'on voit : l'image, ou l'image `k` de la vidéo.
    pub fn shown_path(&self, k: usize) -> Option<PathBuf> {
        let p = self.path()?;
        Some(match &self.video {
            Some(c) => crate::video::frame_file(&p, k.min(c.frames.saturating_sub(1)), &c.ext),
            None => p,
        })
    }

    /// Le texte où chercher : le nom et les étiquettes.
    fn haystack(&self) -> String {
        let mut s = fold(&self.name);
        if !self.text.is_empty() {
            s.push(' ');
            s.push_str(&fold(&self.text));
        }
        for t in &self.tags {
            s.push(' ');
            s.push_str(&fold(t));
        }
        s
    }
}

/// L'échelle d'une référence neuve : son plus long côté fait `BOARD_SIDE`.
pub(crate) fn fit_scale(size: [u32; 2]) -> f32 {
    let side = size[0].max(size[1]).max(1) as f32;
    (BOARD_SIDE / side).clamp(SCALE_MIN, SCALE_MAX)
}

/// La valeur d'un gris (Rec. 709 sur les valeurs sRGB, comme le « désaturer » des logiciels de
/// retouche) : un gris reste lui-même, sans arrondi.
pub(crate) fn luma([r, g, b]: [u8; 3]) -> u8 {
    ((2126 * r as u32 + 7152 * g as u32 + 722 * b as u32 + 5000) / 10_000) as u8
}

/// Un endroit libre près de `want` : on décale en diagonale tant qu'une référence a son centre là.
pub(crate) fn free_spot(existing: &[Reference], want: Pos2) -> Pos2 {
    let mut p = want;
    for _ in 0..400 {
        if existing.iter().any(|r| (r.center() - p).length() < 24.0) {
            p += vec2(36.0, 30.0);
        } else {
            break;
        }
    }
    p
}

// ---------------------------------------------------------------------- la vue du tableau

pub(crate) const ZOOM_MIN: f32 = 0.02;
pub(crate) const ZOOM_MAX: f32 = 32.0;

/// Le point du tableau montré au centre du panneau, et le zoom (points d'écran par unité).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub center: Pos2,
    pub zoom: f32,
}

impl Default for View {
    fn default() -> Self {
        View { center: Pos2::ZERO, zoom: 1.0 }
    }
}

impl View {
    pub fn to_screen(&self, rect: Rect, p: Pos2) -> Pos2 {
        rect.center() + (p - self.center) * self.zoom
    }

    pub fn to_board(&self, rect: Rect, s: Pos2) -> Pos2 {
        self.center + (s - rect.center()) / self.zoom
    }

    /// Zoom de `factor` autour du point d'écran `at` : le point du tableau dessous ne bouge pas.
    pub fn zoom_about(&mut self, rect: Rect, at: Pos2, factor: f32) {
        let before = self.to_board(rect, at);
        self.zoom = (self.zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
        let after = self.to_board(rect, at);
        self.center += before - after;
    }

    /// Fait glisser le tableau de `delta` points d'écran.
    pub fn pan(&mut self, delta: Vec2) {
        self.center -= delta / self.zoom;
    }

    /// La vue qui montre `bounds` en entier dans `rect`, avec `margin` points autour.
    pub fn fit(bounds: Rect, rect: Rect, margin: f32) -> View {
        let (bw, bh) = (bounds.width().max(1.0), bounds.height().max(1.0));
        let (aw, ah) = ((rect.width() - 2.0 * margin).max(8.0), (rect.height() - 2.0 * margin).max(8.0));
        View { center: bounds.center(), zoom: (aw / bw).min(ah / bh).clamp(ZOOM_MIN, ZOOM_MAX) }
    }

    /// Le zoom où un pixel de l'image fait un pixel de l'écran (`ppp` : pixels par point).
    pub fn actual_zoom(r: &Reference, ppp: f32) -> f32 {
        (1.0 / (r.scale.max(SCALE_MIN) * ppp.max(0.25))).clamp(ZOOM_MIN, ZOOM_MAX)
    }
}

/// Le centre d'une référence tirée de `screen_delta` points d'écran depuis sa place `start`.
pub(crate) fn dragged_center(start: Pos2, screen_delta: Vec2, zoom: f32) -> Pos2 {
    let d = screen_delta / zoom.max(ZOOM_MIN);
    pos2((start.x + d.x).clamp(-COORD_MAX, COORD_MAX), (start.y + d.y).clamp(-COORD_MAX, COORD_MAX))
}

/// L'échelle d'une référence dont on tire un coin : elle suit la distance de la souris à son centre
/// (`grab_dist` : celle de l'appui).
pub(crate) fn dragged_scale(start: f32, center: Pos2, grab_dist: f32, at: Pos2) -> f32 {
    (start * (at - center).length() / grab_dist.max(1e-3)).clamp(SCALE_MIN, SCALE_MAX)
}

/// L'angle d'une référence dont on tire la poignée ronde : l'angle de départ plus celui dont la souris
/// a tourné autour du centre (`grab_angle` : l'angle de l'appui, en radians) ; `snap` : par 15°.
pub(crate) fn dragged_rotation(start_deg: f32, center: Pos2, grab_angle: f32, at: Pos2, snap: bool) -> f32 {
    let mut deg = start_deg + ((at - center).angle() - grab_angle).to_degrees();
    if snap {
        deg = (deg / 15.0).round() * 15.0;
    }
    normalize_degrees(deg)
}

/// Le rectangle qui contient toutes les références (rien s'il n'y en a pas).
pub(crate) fn bounds_of<'a>(refs: impl IntoIterator<Item = &'a Reference>) -> Option<Rect> {
    let mut out: Option<Rect> = None;
    for r in refs {
        let b = r.bounds();
        out = Some(out.map_or(b, |o| o.union(b)));
    }
    out
}

/// L'élément le plus haut sous le point du tableau : ceux qui ne sont pas des cadres d'abord (le dernier de
/// la liste est le plus haut), puis les cadres (leur bandeau et leur bord).
pub(crate) fn hit_test<'a>(refs: impl DoubleEndedIterator<Item = &'a Reference> + Clone, p: Pos2) -> Option<u64> {
    let found = refs.clone().rev().find(|r| r.kind != Kind::Frame && r.hit(p));
    found.or_else(|| refs.rev().find(|r| r.kind == Kind::Frame && r.hit(p))).map(|r| r.id)
}

/// Une grille de cellules carrées de `cell` points dans `width` points : le rectangle de la cellule
/// `i` (depuis le coin haut-gauche), et la hauteur totale pour `n` cellules.
pub(crate) fn grid_cell(i: usize, n: usize, width: f32, cell: f32, gap: f32) -> (Rect, f32) {
    let per_row = (((width + gap) / (cell + gap)).floor() as usize).max(1);
    let (row, col) = (i / per_row, i % per_row);
    let min = pos2(col as f32 * (cell + gap), row as f32 * (cell + gap));
    let rows = n.div_ceil(per_row);
    (Rect::from_min_size(min, vec2(cell, cell)), rows as f32 * (cell + gap))
}

// ---------------------------------------------------------------------- recherche et plans

/// Minuscules sans accents, pour chercher « etude » dans « Étude ».
pub(crate) fn fold(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' => 'i',
            'ô' | 'ö' | 'ó' | 'õ' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ç' => 'c',
            'ÿ' => 'y',
            'ñ' => 'n',
            c => c,
        })
        .collect()
}

/// Ce que le panneau montre : la recherche (tous les mots, dans le nom ou les étiquettes), une
/// étiquette, et seulement les références du plan courant.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Filter {
    pub text: String,
    pub tag: Option<String>,
    pub current_plan: bool,
}

impl Filter {
    /// La référence passe-t-elle ? `plan` : le plan courant.
    pub fn matches(&self, r: &Reference, plan: Option<u64>) -> bool {
        if self.current_plan && !plan.is_some_and(|p| r.plans.contains(&p)) {
            return false;
        }
        if let Some(t) = &self.tag {
            let t = fold(t);
            if !r.tags.iter().any(|x| fold(x) == t) {
                return false;
            }
        }
        let text = fold(&self.text);
        if text.trim().is_empty() {
            return true;
        }
        let hay = r.haystack();
        text.split_whitespace().all(|w| hay.contains(w))
    }
}

/// Toutes les étiquettes du tableau, sans doublon (accents et casse près), triées.
pub(crate) fn all_tags(refs: &[Reference]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out: Vec<String> = refs.iter().flat_map(|r| r.tags.iter()).filter(|t| seen.insert(fold(t))).cloned().collect();
    out.sort_by_key(|t| fold(t));
    out
}

/// Les étiquettes d'un texte « décor, lumière ; rue » : séparées par des virgules ou des
/// points-virgules, sans blancs autour, sans doublon, bornées.
pub(crate) fn parse_tags(input: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for t in input.split([',', ';']) {
        let t: String = t.trim().chars().take(TAG_MAX).collect();
        if !t.is_empty() && seen.insert(fold(&t)) {
            out.push(t);
        }
        if out.len() >= TAGS_PER_REF {
            break;
        }
    }
    out
}

// ---------------------------------------------------------------------- le document

impl Document {
    pub(crate) fn reference(&self, id: u64) -> Option<&Reference> {
        self.references.iter().find(|r| r.id == id)
    }

    pub(crate) fn reference_mut(&mut self, id: u64) -> Option<&mut Reference> {
        self.references.iter_mut().find(|r| r.id == id)
    }

    /// Un numéro neuf pour une référence (le même compteur que les plans et les commentaires).
    pub(crate) fn new_reference_id(&mut self) -> u64 {
        self.alloc()
    }

    /// Pose la référence en haut de la pile.
    pub(crate) fn add_reference(&mut self, r: Reference) {
        touch(&r.file);
        // Un cadre se pose au fond : il reste derrière ce qu'on y range.
        if r.kind == Kind::Frame {
            let at = self.references.iter().take_while(|x| x.kind == Kind::Frame).count();
            self.references.insert(at, r);
        } else {
            self.references.push(r);
        }
    }

    pub(crate) fn remove_reference(&mut self, id: u64) -> bool {
        let Some(i) = self.references.iter().position(|r| r.id == id) else { return false };
        let r = self.references.remove(i);
        touch(&r.file);
        true
    }

    /// Met la référence derrière les autres (un cadre est déjà au fond).
    pub(crate) fn lower_reference(&mut self, id: u64) -> bool {
        match self.references.iter().position(|r| r.id == id) {
            Some(i) if i > 0 && self.references[i].kind != Kind::Frame => {
                let r = self.references.remove(i);
                // (Après les cadres, qui restent au fond.)
                let at = self.references.iter().take_while(|x| x.kind == Kind::Frame).count();
                self.references.insert(at, r);
                true
            }
            _ => false,
        }
    }

    /// Met la référence devant les autres (un cadre reste derrière ce qu'il contient : il ne monte pas).
    pub(crate) fn raise_reference(&mut self, id: u64) -> bool {
        match self.references.iter().position(|r| r.id == id) {
            Some(i) if i + 1 < self.references.len() && self.references[i].kind != Kind::Frame => {
                let r = self.references.remove(i);
                self.references.push(r);
                true
            }
            _ => false,
        }
    }

    /// Lie la référence au plan `panel` (`on`) ou l'en délie. Vrai si quelque chose a changé.
    pub(crate) fn link_reference(&mut self, id: u64, panel: u64, on: bool) -> bool {
        let Some(r) = self.reference_mut(id) else { return false };
        let linked = r.plans.contains(&panel);
        match (on, linked) {
            (true, false) => r.plans.push(panel),
            (false, true) => r.plans.retain(|p| *p != panel),
            _ => return false,
        }
        true
    }

    /// Retire des références les plans qui n'existent plus.
    pub(crate) fn prune_references(&mut self) {
        if self.references.iter().all(|r| r.plans.is_empty()) {
            return;
        }
        let ids: HashSet<u64> = self.panels.iter().map(|p| p.id).collect();
        for r in &mut self.references {
            r.plans.retain(|p| ids.contains(p));
        }
    }

    /// Le plan `from` est réuni au plan `to` : ses références le sont aussi à `to`.
    pub(crate) fn rehome_references(&mut self, from: u64, to: u64) {
        for r in &mut self.references {
            if r.plans.contains(&from) {
                r.plans.retain(|p| *p != from);
                if !r.plans.contains(&to) {
                    r.plans.push(to);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------- le fichier

#[derive(Serialize, Deserialize)]
struct Sidecar {
    format: String,
    version: u32,
    #[serde(default)]
    references: Vec<Reference>,
}

/// Le contenu de `references.json` pour `doc` ; rien s'il n'y a aucune référence (le fichier est
/// alors retiré).
pub(crate) fn to_bytes(doc: &Document) -> Result<Option<Vec<u8>>, String> {
    if doc.references.is_empty() {
        return Ok(None);
    }
    let version = if doc.references.iter().any(Reference::uses_board_features) { VERSION_PLANCHE } else { VERSION };
    let sidecar = Sidecar { format: FORMAT.into(), version, references: doc.references.clone() };
    serde_json::to_vec_pretty(&sidecar).map(Some).map_err(|e| e.to_string())
}

/// Les références lues dans le texte de `references.json`. Numéros uniques et non nuls (un doublon
/// en reçoit un neuf), valeurs absurdes ramenées à des valeurs sûres, noms de fichier réduits à un
/// nom (jamais un chemin).
pub(crate) fn parse(text: &str) -> Result<Vec<Reference>, String> {
    let sidecar: Sidecar = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("illisible ({e})"))?;
    if sidecar.format != FORMAT {
        return Err("ce n'est pas un fichier de références".into());
    }
    if sidecar.version > VERSION_PLANCHE {
        return Err(format!("écrit par une version plus récente (format {})", sidecar.version));
    }
    let mut next = sidecar.references.iter().map(|r| r.id).max().unwrap_or(0) + 1;
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(sidecar.references.len());
    for mut r in sidecar.references {
        if r.id == 0 || !seen.insert(r.id) {
            r.id = next;
            next += 1;
            seen.insert(r.id);
        }
        sanitize(&mut r);
        out.push(r);
    }
    Ok(out)
}

/// Ramène une référence lue à des valeurs sûres (un fichier édité à la main ne casse rien).
fn sanitize(r: &mut Reference) {
    let coord = |v: f32| if v.is_finite() { v.clamp(-COORD_MAX, COORD_MAX) } else { 0.0 };
    r.x = coord(r.x);
    r.y = coord(r.y);
    r.scale = if r.scale.is_finite() && r.scale > 0.0 { r.scale.clamp(SCALE_MIN, SCALE_MAX) } else { 1.0 };
    r.rotation = if r.rotation.is_finite() { normalize_degrees(r.rotation) } else { 0.0 };
    r.size = [r.size[0].clamp(1, 1 << 20), r.size[1].clamp(1, 1 << 20)];
    if r.name.chars().count() > NAME_MAX {
        r.name = r.name.chars().take(NAME_MAX).collect();
    }
    // Un nom de fichier, jamais un chemin (« .. », « C:\… »).
    r.file = Path::new(&r.file.replace('\\', "/")).file_name().map(|n| n.to_string_lossy().into_owned()).filter(|n| n != ".." && n != ".").unwrap_or_default();
    r.tags = parse_tags(&r.tags.join(","));
    r.opacity = if r.opacity.is_finite() { r.opacity.clamp(0.05, 1.0) } else { 1.0 };
    r.crop = r.crop.and_then(|c| {
        let c = c.map(|v| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 });
        (c[2] - c[0] >= 0.01 && c[3] - c[1] >= 0.01 && c != [0.0, 0.0, 1.0, 1.0]).then_some(c)
    });
    r.color = r.color.min(crate::planchref::NOTE_COLORS.len() as u8 - 1);
    if r.text.chars().count() > crate::planchref::TEXT_MAX {
        r.text = r.text.chars().take(crate::planchref::TEXT_MAX).collect();
    }
    if r.kind != Kind::Image {
        // Pas de fichier, pas de vidéo, pas de recadrage : la taille est celle du bloc sur le tableau.
        r.file.clear();
        r.video = None;
        r.crop = None;
        r.flip_h = false;
        r.flip_v = false;
        r.grey = false;
        r.size = [r.size[0].clamp(8, 1 << 16), r.size[1].clamp(8, 1 << 16)];
    }
    r.author = r.author.chars().take(NAME_MAX).collect();
    let mut seen = HashSet::new();
    r.plans.retain(|p| seen.insert(*p));
    if let Some(c) = &mut r.video {
        c.frames = c.frames.clamp(1, 10_000_000);
        if !(c.fps.is_finite() && c.fps > 0.0 && c.fps < 10_000.0) {
            c.fps = 24.0;
        }
        if !matches!(c.ext.as_str(), "jpg" | "png") {
            c.ext = "jpg".into();
        }
        c.speed = if c.speed.is_finite() { c.speed.clamp(SPEED_MIN, SPEED_MAX) } else { 1.0 };
        c.offset = c.offset.clamp(-10_000_000, 10_000_000);
        c.in_point = c.in_point.min(c.frames - 1);
        c.out_point = c.out_point.map(|o| o.min(c.frames - 1).max(c.in_point));
    }
}

/// Un angle en degrés ramené entre -180 (exclu) et 180.
pub(crate) fn normalize_degrees(a: f32) -> f32 {
    let r = a.rem_euclid(360.0);
    if r > 180.0 {
        r - 360.0
    } else {
        r
    }
}

/// Lit `references.json` et pose, pour chaque référence, le dossier où sont ses fichiers
/// (`references/`). Absent : aucune référence. Illisible (abîmé, d'une version plus récente) : le
/// projet s'ouvre sans références, avec un avertissement, et le fichier reste tel quel.
pub(crate) fn load(dir: &Path, warnings: &mut Vec<String>) -> Vec<Reference> {
    let path = dir.join(FILE);
    let text = match std::fs::read(&path) {
        Ok(bytes) => String::from_utf8(bytes).map_err(|_| "le fichier n'est pas du texte (UTF-8)".to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => Err(e.to_string()),
    };
    let mut list = match text.and_then(|t| parse(&t)) {
        Ok(list) => list,
        Err(e) => {
            warnings.push(format!("{FILE} : {e} ; les références ne sont pas chargées."));
            return Vec::new();
        }
    };
    let home = dir.join(DIR);
    for r in &mut list {
        r.home = Some(home.clone());
        if !r.file.is_empty() && !home.join(&r.file).exists() {
            warnings.push(format!("référence « {} » : fichier introuvable ({DIR}/{}).", r.name, r.file));
        }
    }
    list
}

// ---------------------------------------------------------------------- les fichiers

/// Les fichiers touchés pendant la séance (ajoutés ou retirés d'une référence) : l'enregistrement
/// ne les efface pas, une annulation ou un rétablissement peut les reprendre.
static TOUCHED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

pub(crate) fn touch(file: &str) {
    if !file.is_empty() {
        TOUCHED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashSet::new).insert(file.to_string());
    }
}

fn touched(file: &str) -> bool {
    TOUCHED.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|s| s.contains(file))
}

/// Un dossier temporaire neuf, où attendent les fichiers d'un projet pas encore enregistré.
pub(crate) fn staging_dir() -> PathBuf {
    std::env::temp_dir().join("pelure-references").join(crate::video::new_id().to_string())
}

/// Copie `from` en `to` sans jamais laisser un fichier à moitié copié (à côté, puis renommé).
fn copy_atomic(from: &Path, to: &Path) -> Result<(), String> {
    let mut tmp = to.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::copy(from, &tmp).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{} : {e}", from.display())
    })?;
    std::fs::rename(&tmp, to).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{} : {e}", to.display())
    })
}

/// Recopie un fichier, ou les images d'un dossier, là où elles manquent. Renvoie le nombre de
/// fichiers écrits.
pub(crate) fn copy_entry(from: &Path, to: &Path) -> Result<usize, String> {
    if from.is_dir() {
        std::fs::create_dir_all(to).map_err(|e| format!("{} : {e}", to.display()))?;
        let mut n = 0;
        for e in std::fs::read_dir(from).map_err(|e| format!("{} : {e}", from.display()))?.flatten() {
            let p = e.path();
            let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
            // (Le son extrait avec les images d'une vidéo n'a rien à faire ici.)
            if p.is_file() && matches!(ext.as_str(), "jpg" | "png") {
                let dst = to.join(e.file_name());
                if !dst.exists() {
                    copy_atomic(&p, &dst)?;
                    n += 1;
                }
            }
        }
        return Ok(n);
    }
    if to.exists() {
        return Ok(0);
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{} : {e}", parent.display()))?;
    }
    copy_atomic(from, to)?;
    Ok(1)
}

/// Ce que l'enregistrement fait des références (préparé sur le fil de l'interface, joué par
/// `SaveJob::run_with`) : les fichiers à recopier, `references.json`, et de quoi nettoyer.
#[derive(Default)]
pub(crate) struct SavePlan {
    copies: Vec<(PathBuf, PathBuf)>,
    json: Option<Vec<u8>>,
    used: BTreeSet<String>,
    /// Le `references.json` du dossier n'a pas pu être lu : on n'efface rien de `references/`.
    protect: bool,
}

/// Prépare l'enregistrement des références de `doc` dans le dossier de projet `dir`.
pub(crate) fn plan_save(doc: &Document, dir: &Path) -> Result<SavePlan, String> {
    let target = dir.join(DIR);
    let mut plan = SavePlan { json: to_bytes(doc)?, protect: existing_is_unreadable(&dir.join(FILE)), ..SavePlan::default() };
    for r in &doc.references {
        if r.file.is_empty() {
            continue;
        }
        plan.used.insert(r.file.clone());
        let Some(home) = &r.home else { continue };
        let (from, to) = (home.join(&r.file), target.join(&r.file));
        if from != to && from.exists() {
            plan.copies.push((from, to));
        }
    }
    Ok(plan)
}

/// Le fichier existe et cette version ne sait pas le lire.
fn existing_is_unreadable(path: &Path) -> bool {
    match std::fs::read(path) {
        Ok(b) => !String::from_utf8(b).ok().is_some_and(|t| parse(&t).is_ok()),
        Err(e) => e.kind() != std::io::ErrorKind::NotFound,
    }
}

impl SavePlan {
    /// Recopie les fichiers qui manquent dans `references/`, puis écrit `references.json` (ou le
    /// retire s'il n'y a plus de référence), sans jamais détruire un fichier illisible.
    pub(crate) fn write(&self, dir: &Path) -> Result<usize, String> {
        let mut written = 0;
        for (from, to) in &self.copies {
            written += copy_entry(from, to)?;
        }
        crate::project::write_sidecar(&dir.join(FILE), self.json.as_deref(), |t| parse(t).is_ok())?;
        Ok(written)
    }

    /// Efface de `references/` ce que le projet ne cite plus et que cette séance n'a pas touché : les
    /// restes de séances d'avant. Rien si `references.json` était illisible. Ne touche que les noms que
    /// SHOT donne (`<nombre>.png`, `<nombre>.jpg`, dossier `<nombre>`).
    pub(crate) fn cleanup(&self, dir: &Path) -> usize {
        if self.protect {
            return 0;
        }
        let Ok(entries) = std::fs::read_dir(dir.join(DIR)) else { return 0 };
        let mut removed = 0;
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = e.path();
            if self.used.contains(&name) || touched(&name) || !is_our_name(&name, path.is_dir()) {
                continue;
            }
            let done = if path.is_dir() { std::fs::remove_dir_all(&path).is_ok() } else { std::fs::remove_file(&path).is_ok() };
            removed += usize::from(done);
        }
        removed
    }
}

/// Un nom que SHOT donne à ses fichiers de références.
fn is_our_name(name: &str, is_dir: bool) -> bool {
    let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    if is_dir {
        return digits(name);
    }
    match name.rsplit_once('.') {
        Some((stem, ext)) => digits(stem) && IMAGE_EXTS.contains(&ext),
        None => false,
    }
}

// ---------------------------------------------------------------------- l'import

/// Une image importée : son nom, le fichier écrit et sa taille.
#[derive(Debug)]
pub(crate) struct Imported {
    pub name: String,
    pub file: String,
    pub size: [u32; 2],
}

fn display_name(src: &Path) -> String {
    src.file_stem().map(|s| s.to_string_lossy().into_owned()).filter(|s| !s.is_empty()).unwrap_or_else(|| "Image".into())
}

/// Copie l'image `src` (PNG ou JPEG) dans `dest_dir` sous le nom `<id>.<ext>`. Rien n'est décodé en
/// entier : la taille se lit dans l'en-tête.
pub(crate) fn import_image(src: &Path, dest_dir: &Path, id: u64) -> Result<Imported, String> {
    let name = display_name(src);
    let ext = src.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
    let ext = match ext.as_str() {
        "png" => "png",
        "jpg" | "jpeg" => "jpg",
        _ => return Err(format!("« {name} » : format d'image non pris en charge (PNG ou JPEG).")),
    };
    let (w, h) = image::image_dimensions(src).map_err(|e| format!("« {name} » : image illisible ({e})."))?;
    if w == 0 || h == 0 {
        return Err(format!("« {name} » : image vide."));
    }
    std::fs::create_dir_all(dest_dir).map_err(|e| format!("{} : {e}", dest_dir.display()))?;
    let file = format!("{id}.{ext}");
    copy_atomic(src, &dest_dir.join(&file))?;
    Ok(Imported { name, file, size: [w, h] })
}

/// Écrit une image en mémoire (RVBA, par exemple collée du presse-papiers) en PNG dans `dest_dir`.
pub(crate) fn import_rgba(name: &str, size: [usize; 2], rgba: &[u8], dest_dir: &Path, id: u64) -> Result<Imported, String> {
    if size[0] == 0 || size[1] == 0 || rgba.len() < size[0] * size[1] * 4 {
        return Err("image vide".into());
    }
    let bytes = crate::io::encode_png(&rgba[..size[0] * size[1] * 4], size[0], size[1])?;
    std::fs::create_dir_all(dest_dir).map_err(|e| format!("{} : {e}", dest_dir.display()))?;
    let file = format!("{id}.png");
    crate::project::write_atomic(&dest_dir.join(&file), &bytes)?;
    Ok(Imported { name: name.into(), file, size: [size[0] as u32, size[1] as u32] })
}

// ---------------------------------------------------------------------- décoder

/// Les définitions des vignettes gardées en mémoire graphique (côté le plus long, en pixels).
pub(crate) const LEVELS: [u32; 6] = [128, 256, 512, 1024, 2048, 4096];

/// La définition qu'il faut pour montrer une image de `shown` pixels d'écran de côté : le premier
/// niveau qui suffit, réduit par le réglage de résolution des vignettes (`pct`, en pour cent) ; 0 : la
/// taille de l'image, quand elle est plus petite que le niveau voulu (`native` : son plus long côté).
pub(crate) fn level_for(shown: f32, native: u32, pct: u32) -> u32 {
    let want = (shown.max(1.0) * pct.clamp(10, 100) as f32 / 100.0).ceil() as u32;
    let level = LEVELS.iter().copied().find(|l| *l >= want).unwrap_or(*LEVELS.last().unwrap_or(&4096));
    if native <= level {
        0
    } else {
        level
    }
}

/// Lit `path` et le ramène à `level` pixels de côté au plus (0 : tel quel), en niveaux de gris si
/// `grey`. Renvoie largeur, hauteur et pixels RVBA. Sur un autre fil que celui de l'interface.
pub(crate) fn decode(path: &Path, level: u32, grey: bool) -> Option<(usize, usize, Vec<u8>)> {
    let img = image::open(path).ok()?;
    let img = if level > 0 && img.width().max(img.height()) > level {
        let k = level as f32 / img.width().max(img.height()) as f32;
        let (w, h) = (((img.width() as f32 * k).round() as u32).max(1), ((img.height() as f32 * k).round() as u32).max(1));
        img.resize_exact(w, h, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let mut rgba = img.to_rgba8();
    if grey {
        for p in rgba.pixels_mut() {
            let g = luma([p[0], p[1], p[2]]);
            p[0] = g;
            p[1] = g;
            p[2] = g;
        }
    }
    let (w, h) = (rgba.width() as usize, rgba.height() as usize);
    Some((w, h, rgba.into_raw()))
}

/// Un cache des plus récemment utilisés, borné en octets. Une entrée utilisée pendant l'image en cours
/// (`clock`) n'est jamais évincée.
pub(crate) struct Lru<K, V> {
    entries: std::collections::HashMap<K, (V, usize, u64)>,
    total: usize,
    clock: u64,
}

impl<K, V> Default for Lru<K, V> {
    fn default() -> Self {
        Lru { entries: std::collections::HashMap::new(), total: 0, clock: 0 }
    }
}

impl<K: std::hash::Hash + Eq + Clone, V> Lru<K, V> {
    /// Une nouvelle image d'écran : les entrées qui s'y servent portent ce nombre.
    pub(crate) fn tick(&mut self) {
        self.clock += 1;
    }

    pub(crate) fn get(&mut self, k: &K) -> Option<&V> {
        let clock = self.clock;
        self.entries.get_mut(k).map(|e| {
            e.2 = clock;
            &e.0
        })
    }

    pub(crate) fn insert(&mut self, k: K, v: V, bytes: usize) {
        if let Some(old) = self.entries.insert(k, (v, bytes, self.clock)) {
            self.total -= old.1;
        }
        self.total += bytes;
    }

    pub(crate) fn keys(&self) -> impl Iterator<Item = &K> {
        self.entries.keys()
    }

    /// Octets gardés en tout.
    pub(crate) fn total(&self) -> usize {
        self.total
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(crate) fn contains(&self, k: &K) -> bool {
        self.entries.contains_key(k)
    }

    /// Évince les moins récemment utilisées tant que le total dépasse `budget`, sauf celles de
    /// l'image en cours. Renvoie le nombre d'entrées parties.
    pub(crate) fn evict(&mut self, budget: usize) -> usize {
        let mut gone = 0;
        while self.total > budget {
            let oldest = self.entries.iter().filter(|(_, e)| e.2 < self.clock).min_by_key(|(_, e)| e.2).map(|(k, _)| k.clone());
            let Some(k) = oldest else { break };
            if let Some(e) = self.entries.remove(&k) {
                self.total -= e.1;
                gone += 1;
            }
        }
        gone
    }
}
