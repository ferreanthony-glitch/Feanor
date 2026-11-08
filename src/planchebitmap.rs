//! La couche bitmap de la planche (partie 127, Anthony, 2026-11-08 : « les caméras doivent être le moins intrusives et destructives possibles dans
//! le dessin »). Ce qu'un trait **bitmap** tracé à travers les cases laisse hors de toute case qui puisse le recevoir reste ici, **en pixels**,
//! exactement ceux que le pinceau a posés : rien n'est rejoué, rien n'est rééchantillonné.
//!
//! # Ce que c'est
//!
//! - **Un morceau** (`Morceau`) : des pixels prémultipliés en tuiles creuses de 64 pixels (`Raster` sans feuille : seules les tuiles touchées
//!   existent, et elles se partagent à la copie), dans la grille de pixels du dessin d'où ils viennent, et la **pose** qui les place sur la planche
//!   **relativement à une case** (`Ancre`, comme les traits du calque « Hors cadres ») : quand les cases bougent ou grossissent, le morceau les suit.
//!   Un morceau ne change jamais : le déplacer, l'effacer ou le couper en fait un autre (numéro neuf). Les annuler ne coûte que des pointeurs.
//! - **Sous les cases**, comme le vectoriel hors des cases, à l'opacité « hors champ » du réglage ; **par-dessus** l'image d'une case qui ne
//!   pouvait pas le recevoir (`dessus`, partie 122), pleins. Montrés et cachés avec le calque « Hors cadres ».
//! - **Les empreintes** (`Empreinte`) : pour chaque trait réparti d'un même geste, où ses pixels sont tombés dans le dessin de chaque case (un bit par
//!   pixel, par tuile). Avec le numéro du geste (`lien`, le même que celui de ses morceaux), la sélection des Vignettes retrouve le trait entier
//!   (`vignetteselbmp.rs`).
//!
//! # Fichier
//!
//! Le dossier `planche_bitmap/` à côté du manifeste : `index.json` (les morceaux, leurs ancres, leurs poses, leurs tuiles ; les empreintes) et une
//! image par morceau (`m<numéro>.png`) et par empreinte (`e<geste>-<dessin>.png`), en atlas de tuiles comme ce qu'un dessin a hors de sa feuille.
//! **Le format du projet ne change pas** : un projet d'avant n'a pas ce dossier ; une version d'avant ne le lit pas, ne l'écrit pas, ne l'efface pas
//! (elle montre les traits sans leur part hors des cases, et la retrouve à la réouverture ici). Un index illisible ou d'une version plus récente :
//! rien n'est chargé, rien n'est réécrit ni effacé.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;

use eframe::egui::{self, pos2, Color32, ColorImage, Pos2, Rect, TextureHandle, TextureOptions};
use serde::{Deserialize, Serialize};
use tiny_skia::{FilterQuality, IntSize, Pixmap, PixmapPaint, Transform};

use crate::annotations::{self, Ancre};
use crate::board::BoardLayout;
use crate::document::CelId;
use crate::raster::{Raster, Tile, TILE, TILE_PIXELS};

pub const DOSSIER: &str = "planche_bitmap";
const INDEX: &str = "index.json";
const FORMAT: &str = "shot-planche-bitmap";
const VERSION: u32 = 1;
/// Bornes d'un index abîmé, et de la planche : au-delà, un geste ne pose plus de morceau (la ligne d'état le dit).
pub const MAX_MORCEAUX: usize = 5000;
pub const MAX_EMPREINTES: usize = 20_000;
const MAX_TUILES_LUES: usize = 60_000;
/// Côté d'un bloc de texture, en pixels de son niveau.
const BLOC: i64 = 256;
const NIVEAUX: usize = 6;

// ------------------------------------------------------------------------------------ une affinité en f64

/// p' = (a·x + b·y + tx, c·x + d·y + ty), en double précision : un pixel posé, déplacé, reposé ne doit pas glisser d'un rien.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct A64 {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub tx: f64,
    pub ty: f64,
}

impl A64 {
    pub const IDENTITE: A64 = A64 { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 0.0, ty: 0.0 };

    pub fn de(a: &crate::vector::Affine) -> A64 {
        A64 { a: a.a as f64, b: a.b as f64, c: a.c as f64, d: a.d as f64, tx: a.tx as f64, ty: a.ty as f64 }
    }

    pub fn echelle_decalage(s: f64, tx: f64, ty: f64) -> A64 {
        A64 { a: s, b: 0.0, c: 0.0, d: s, tx, ty }
    }

    pub fn applique(&self, x: f64, y: f64) -> (f64, f64) {
        (self.a * x + self.b * y + self.tx, self.c * x + self.d * y + self.ty)
    }

    pub fn point(&self, p: Pos2) -> Pos2 {
        let (x, y) = self.applique(p.x as f64, p.y as f64);
        pos2(x as f32, y as f32)
    }

    /// `self` puis `n`.
    pub fn puis(&self, n: &A64) -> A64 {
        A64 {
            a: n.a * self.a + n.b * self.c,
            b: n.a * self.b + n.b * self.d,
            c: n.c * self.a + n.d * self.c,
            d: n.c * self.b + n.d * self.d,
            tx: n.a * self.tx + n.b * self.ty + n.tx,
            ty: n.c * self.tx + n.d * self.ty + n.ty,
        }
    }

    pub fn inverse(&self) -> Option<A64> {
        let det = self.a * self.d - self.b * self.c;
        if !(det.abs() > 1e-18) || !det.is_finite() {
            return None;
        }
        let (a, b, c, d) = (self.d / det, -self.b / det, -self.c / det, self.a / det);
        Some(A64 { a, b, c, d, tx: -(a * self.tx + b * self.ty), ty: -(c * self.tx + d * self.ty) })
    }

    /// Unités d'arrivée par unité de départ (moyenne géométrique).
    pub fn echelle(&self) -> f64 {
        (self.a * self.d - self.b * self.c).abs().sqrt()
    }

    pub fn fini(&self) -> bool {
        [self.a, self.b, self.c, self.d, self.tx, self.ty].iter().all(|v| v.is_finite())
    }

    pub fn affine(&self) -> crate::vector::Affine {
        crate::vector::Affine { a: self.a as f32, b: self.b as f32, c: self.c as f32, d: self.d as f32, tx: self.tx as f32, ty: self.ty as f32 }
    }

    /// Une translation d'un nombre entier de pixels (à un millième près) : les pixels se recopient tels quels, sans rééchantillonner.
    pub fn translation_entiere(&self) -> Option<(i64, i64)> {
        let e = 1e-6;
        if (self.a - 1.0).abs() > e || (self.d - 1.0).abs() > e || self.b.abs() > e || self.c.abs() > e {
            return None;
        }
        let (rx, ry) = (self.tx.round(), self.ty.round());
        ((self.tx - rx).abs() < 1e-3 && (self.ty - ry).abs() < 1e-3).then_some((rx as i64, ry as i64))
    }

    fn tableau(&self) -> [f64; 6] {
        [self.a, self.b, self.c, self.d, self.tx, self.ty]
    }

    fn du_tableau(t: [f64; 6]) -> A64 {
        A64 { a: t[0], b: t[1], c: t[2], d: t[3], tx: t[4], ty: t[5] }
    }
}

/// De la planche aux coordonnées relatives à la case `r` (en largeurs de case).
fn vers_relatif(r: Rect) -> A64 {
    let k = (r.width() as f64).max(1e-3);
    A64::echelle_decalage(1.0 / k, -r.left() as f64 / k, -r.top() as f64 / k)
}

fn depuis_relatif(r: Rect) -> A64 {
    let k = r.width() as f64;
    A64::echelle_decalage(k, r.left() as f64, r.top() as f64)
}

// ------------------------------------------------------------------------------------ les pixels

/// Un dessin sans feuille : rien que des tuiles creuses (`Raster::outer`).
pub fn pixels_vides() -> Raster {
    Raster::new(0, 0)
}

/// Pose le pixel `c` (prémultiplié, opaque ou non) en (`x`, `y`) de `r`, par-dessus (`dessous` : sous ce qui y est).
pub fn poser_pixel(r: &mut Raster, x: i64, y: i64, c: Color32, dessous: bool) {
    let t = TILE as i64;
    let (tx, ty) = (x.div_euclid(t), y.div_euclid(t));
    if tx.abs() > crate::raster::WORLD / t || ty.abs() > crate::raster::WORLD / t {
        return;
    }
    let id = r.id_of(tx as i32, ty as i32);
    let tuile = r.tile_mut(id);
    let k = (y.rem_euclid(t) * t + x.rem_euclid(t)) as usize;
    tuile[k] = if dessous { crate::render::over(c, tuile[k], 255, None) } else { crate::render::over(tuile[k], c, 255, None) };
}

/// Chaque pixel dessiné de `r` : (x, y, couleur).
pub fn chaque_pixel(r: &Raster, mut f: impl FnMut(i64, i64, Color32)) {
    for id in r.drawn_ids() {
        let Some(tuile) = r.slot(id) else { continue };
        let (ox, oy) = r.tile_origin(id);
        // (Sans garder ouverte une tuile rangée.)
        tuile.avec_les_pixels(|px| {
            for (k, c) in px.iter().enumerate() {
                if c.a() != 0 {
                    f(ox + (k % TILE) as i64, oy + (k / TILE) as i64, *c);
                }
            }
        });
    }
}

/// Retire de `r` les tuiles devenues transparentes (des pixels qu'on a pris).
pub fn sans_tuiles_vides(r: &mut Raster) {
    for id in r.drawn_ids() {
        if r.slot(id).is_some_and(|t| t.avec_les_pixels(|p| p.iter().all(|c| c.a() == 0))) {
            r.replace_slot(id, None);
        }
    }
}

/// L'opacité du pixel (`x`, `y`) de `r`, sans garder ouverte une tuile rangée.
pub fn alpha_sans_garder(r: &Raster, x: i64, y: i64) -> u8 {
    let t = TILE as i64;
    match r.tile_at(x.div_euclid(t) as i32, y.div_euclid(t) as i32) {
        Some(tuile) => tuile.avec_les_pixels(|p| p[(y.rem_euclid(t) * t + x.rem_euclid(t)) as usize].a()),
        None => 0,
    }
}

/// Le nombre de tuiles d'un dessin (fenêtre et hors-feuille) : la mesure de ce qu'il pèse.
pub fn tuiles_de(r: &Raster) -> usize {
    r.tiles.iter().filter(|t| t.is_some()).count() + r.outer.len()
}

/// La boîte des tuiles dessinées, en pixels : (x0, y0, x1, y1), bornes exclues à droite.
pub fn boite_px(r: &Raster) -> Option<(i64, i64, i64, i64)> {
    let (a, b, c, d) = r.drawn_tile_range()?;
    let t = TILE as i64;
    Some((a as i64 * t, b as i64 * t, (c as i64 + 1) * t, (d as i64 + 1) * t))
}

// ------------------------------------------------------------------------------------ les empreintes

/// Un bit par pixel, par tuile de 64 : où un trait est tombé dans un dessin.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Masque {
    pub tuiles: BTreeMap<(i32, i32), Box<[u64; TILE]>>,
}

impl Masque {
    pub fn poser(&mut self, x: i64, y: i64) {
        let t = TILE as i64;
        let k = ((x.div_euclid(t)) as i32, (y.div_euclid(t)) as i32);
        let m = self.tuiles.entry(k).or_insert_with(|| Box::new([0u64; TILE]));
        m[y.rem_euclid(t) as usize] |= 1u64 << x.rem_euclid(t);
    }

    /// Le mot de la rangée `j` de la tuile (`tx`, `ty`), créé au besoin.
    pub fn mot(&mut self, tx: i32, ty: i32, j: usize) -> &mut u64 {
        &mut self.tuiles.entry((tx, ty)).or_insert_with(|| Box::new([0u64; TILE]))[j]
    }

    pub fn contient(&self, x: i64, y: i64) -> bool {
        let t = TILE as i64;
        self.tuiles.get(&((x.div_euclid(t)) as i32, (y.div_euclid(t)) as i32)).is_some_and(|m| m[y.rem_euclid(t) as usize] >> x.rem_euclid(t) & 1 == 1)
    }

    pub fn est_vide(&self) -> bool {
        self.tuiles.is_empty()
    }

    pub fn octets(&self) -> usize {
        self.tuiles.len() * (std::mem::size_of::<[u64; TILE]>() + 48)
    }

    /// Chaque pixel du masque.
    pub fn chaque(&self, mut f: impl FnMut(i64, i64)) {
        let t = TILE as i64;
        for (&(tx, ty), m) in &self.tuiles {
            for (j, ligne) in m.iter().enumerate() {
                let mut l = *ligne;
                while l != 0 {
                    let i = l.trailing_zeros() as i64;
                    l &= l - 1;
                    f(tx as i64 * t + i, ty as i64 * t + j as i64);
                }
            }
        }
    }
}

/// Où les pixels du geste `lien` sont tombés dans le dessin `cel`, et ce qu'ils étaient alors (`signature`) : si le dessin a changé là depuis (des pixels
/// déplacés au Storyboard, un trait peint par-dessus, la gomme), l'empreinte ne dit plus où est le trait, et la sélection ne la suit pas.
#[derive(Clone, Debug, PartialEq)]
pub struct Empreinte {
    pub lien: u64,
    pub cel: CelId,
    pub masque: Arc<Masque>,
    /// `signature(dessin, masque)` au moment où le geste a été posé ; 0 : inconnue (jamais fiable).
    pub signature: u64,
}

/// La signature des pixels de `r` sous le masque `m` (leurs places et leurs couleurs, dans l'ordre du masque). Jamais 0.
pub fn signature(r: &Raster, m: &Masque) -> u64 {
    let t = TILE as i64;
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut manger = |v: u64| {
        for b in v.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    for (&(tx, ty), bits) in &m.tuiles {
        manger(((tx as u32 as u64) << 32) | ty as u32 as u64);
        let tuile = r.tile_at(tx, ty);
        let mut lire = |px: Option<&[Color32; TILE_PIXELS]>| {
            for (j, l) in bits.iter().enumerate() {
                manger(*l);
                let mut l = *l;
                while l != 0 {
                    let i = l.trailing_zeros() as usize;
                    l &= l - 1;
                    manger(u64::from(u32::from_le_bytes(px.map_or([0; 4], |p| p[j * TILE + i].to_array()))));
                }
            }
        };
        match tuile {
            Some(tl) => tl.avec_les_pixels(|p| lire(Some(p))),
            None => lire(None),
        }
    }
    let _ = t;
    h.max(1)
}

impl Empreinte {
    /// L'empreinte dit-elle encore où est le trait (le dessin n'a pas changé sous elle) ?
    pub fn fiable(&self, r: &Raster) -> bool {
        self.signature != 0 && signature(r, &self.masque) == self.signature
    }
}

// ------------------------------------------------------------------------------------ les morceaux

/// Des pixels de la planche, hors des cases (voir le début du fichier).
#[derive(Clone)]
pub struct Morceau {
    /// Unique, enregistré : le nom de son image.
    pub id: u64,
    pub anchor: Ancre,
    /// Par-dessus l'image de sa case (partie 122) ; sinon sous les cases.
    pub dessus: bool,
    /// Le geste dont il est une part (le même numéro que ses empreintes).
    pub lien: Option<u64>,
    /// D'un pixel de `pixels` aux coordonnées relatives à sa case (en largeurs de case).
    pub pose: A64,
    pub pixels: Arc<Raster>,
}

impl PartialEq for Morceau {
    fn eq(&self, o: &Morceau) -> bool {
        self.id == o.id && self.anchor == o.anchor && self.dessus == o.dessus && self.lien == o.lien && self.pose == o.pose && Arc::ptr_eq(&self.pixels, &o.pixels)
    }
}

impl std::fmt::Debug for Morceau {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Morceau").field("id", &self.id).field("anchor", &self.anchor).field("dessus", &self.dessus).field("lien", &self.lien).field("tuiles", &tuiles_de(&self.pixels)).finish()
    }
}

impl Morceau {
    /// D'un pixel à la planche, sur la planche `layout`.
    pub fn vers_planche(&self, layout: &BoardLayout) -> Option<A64> {
        let (_, r) = annotations::case_de(layout, &self.anchor)?;
        Some(self.pose.puis(&depuis_relatif(r)))
    }

    /// Ce qu'il couvre sur la planche (la boîte de ses tuiles).
    pub fn bornes(&self, layout: &BoardLayout) -> Option<Rect> {
        let v = self.vers_planche(layout)?;
        let (x0, y0, x1, y1) = boite_px(&self.pixels)?;
        let coins = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)].map(|(x, y)| v.point(pos2(x as f32, y as f32)));
        Some(Rect::from_points(&coins))
    }

    /// Un pixel d'encre sous le cercle (`p`, `rayon`, planche) ?
    pub fn touche(&self, layout: &BoardLayout, p: Pos2, rayon: f32) -> bool {
        let Some(b) = self.bornes(layout) else { return false };
        if !b.expand(rayon).contains(p) {
            return false;
        }
        let Some(v) = self.vers_planche(layout) else { return false };
        let Some(inv) = v.inverse() else { return false };
        let q = inv.point(p);
        let r = (rayon as f64 / v.echelle().max(1e-9)).clamp(0.5, 400.0);
        let pas = (r / 24.0).max(1.0);
        let mut y = -r;
        while y <= r {
            let mut x = -r;
            while x <= r {
                if x * x + y * y <= r * r && alpha_sans_garder(&self.pixels, (q.x as f64 + x).floor() as i64, (q.y as f64 + y).floor() as i64) > 0 {
                    return true;
                }
                x += pas;
            }
            y += pas;
        }
        false
    }

    pub fn octets(&self) -> usize {
        self.pixels.octets_en_memoire()
    }
}

/// La couche bitmap de la planche : ses morceaux (du plus bas au plus haut) et les empreintes des gestes.
#[derive(Clone, Default)]
pub struct PlancheBitmap {
    pub morceaux: Vec<Morceau>,
    pub empreintes: Vec<Empreinte>,
    /// L'index du projet n'a pas pu être lu (abîmé, d'une version plus récente, démesuré) : rien n'y est réécrit ni effacé, rien n'est posé sur la
    /// couche (un trait reste dans sa case), et « Enregistrer sous » recopie le dossier tel quel.
    pub illisible: bool,
    /// Les morceaux de l'index dont l'image manque ou ne se lit pas (un dossier synchronisé où l'index arrive avant les images) : gardés tels quels dans
    /// l'index, leur fichier n'est jamais effacé, et recopié par « Enregistrer sous ». Ils reviennent à la réouverture si l'image est là.
    fantomes: Vec<MorceauFichier>,
    /// Le dossier du projet d'où la couche a été lue (« Enregistrer sous » y reprend ce qu'elle n'a pas en mémoire). Hors de la comparaison.
    pub dossier_lu: Option<std::path::PathBuf>,
}

impl std::fmt::Debug for PlancheBitmap {
    /// (Sans le dossier d'où la couche a été lue : ce n'est pas le projet, la photo d'un projet relu doit être la même.)
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlancheBitmap").field("morceaux", &self.morceaux).field("empreintes", &self.empreintes).field("illisible", &self.illisible).field("fantomes", &self.fantomes.iter().map(|m| m.id).collect::<Vec<_>>()).finish()
    }
}
impl PartialEq for PlancheBitmap {
    fn eq(&self, o: &PlancheBitmap) -> bool {
        self.morceaux == o.morceaux && self.empreintes == o.empreintes && self.illisible == o.illisible && self.fantomes.len() == o.fantomes.len() && self.fantomes.iter().zip(&o.fantomes).all(|(a, b)| a.id == b.id)
    }
}

impl PlancheBitmap {
    pub fn est_vide(&self) -> bool {
        self.morceaux.is_empty() && self.empreintes.is_empty() && self.fantomes.is_empty()
    }

    /// Des morceaux peuvent-ils être posés : la couche a été lue (sinon ils ne pourraient pas être enregistrés), et il reste `n` places.
    pub fn peut_poser(&self, n: usize) -> bool {
        !self.illisible && self.morceaux.len() + self.fantomes.len() + n <= MAX_MORCEAUX
    }

    /// Pose les pixels `pixels` (posés sur la planche par `vers_planche`) : attachés à `sur` (une case qui ne pouvait pas les recevoir : par-dessus
    /// elle), ou à la case la plus proche de leur centre. `derriere` : sous les autres morceaux. Rend le numéro du morceau.
    pub fn ajouter(&mut self, layout: &BoardLayout, pixels: Raster, vers_planche: A64, sur: Option<(&crate::board::CardPlace, usize)>, lien: Option<u64>, derriere: bool) -> Option<u64> {
        // (L'appelant a vérifié la place, `peut_poser`, avant de rien retirer d'un dessin ; ce refus n'est qu'un garde-fou.)
        if pixels.is_blank() || !vers_planche.fini() || !self.peut_poser(1) {
            return None;
        }
        let (x0, y0, x1, y1) = boite_px(&pixels)?;
        let (anchor, r, dessus) = match sur {
            Some((c, rang)) => (Ancre { card: c.id, after: 0, rank: rang.min(u32::MAX as usize) as u32 }, c.thumb, true),
            None => {
                let centre = vers_planche.point(pos2(((x0 + x1) / 2) as f32, ((y0 + y1) / 2) as f32));
                let (rang, r) = annotations::case_proche(layout, centre)?;
                (annotations::ancre_du_rang(layout, rang)?, r, false)
            }
        };
        let id = crate::horschamp::nouveau_lien();
        // (Rangées tout de suite : un dixième de leur taille pour du trait ; l'écran, l'export et l'enregistrement les relisent sans les garder.)
        let mut pixels = pixels;
        pixels.geler();
        let m = Morceau { id, anchor, dessus, lien, pose: vers_planche.puis(&vers_relatif(r)), pixels: Arc::new(pixels) };
        if derriere {
            self.morceaux.insert(0, m);
        } else {
            self.morceaux.push(m);
        }
        Some(id)
    }

    /// Note l'empreinte du geste `lien` dans le dessin `cel` (`raster` : ses pixels maintenant, d'où sa signature). Deux parts du même geste dans le même
    /// dessin (deux cases qui le montrent) font une seule empreinte. Pleines, les empreintes mortes partent d'abord (leur dessin n'existe plus ou a changé sous
    /// elles : `vivante`). Faux si elle n'a pas pu être notée : le geste ne se suivra pas entier, rien d'autre ne change.
    pub fn noter_empreinte(&mut self, lien: u64, cel: CelId, masque: Masque, raster: &Raster, vivante: impl Fn(&Empreinte) -> bool) -> bool {
        if masque.est_vide() {
            return true;
        }
        if let Some(e) = self.empreintes.iter_mut().find(|e| e.lien == lien && e.cel == cel) {
            let mut m = (*e.masque).clone();
            for (k, bits) in masque.tuiles {
                let w = m.tuiles.entry(k).or_insert_with(|| Box::new([0u64; TILE]));
                for (a, b) in w.iter_mut().zip(bits.iter()) {
                    *a |= *b;
                }
            }
            e.signature = signature(raster, &m);
            e.masque = Arc::new(m);
            return true;
        }
        if self.empreintes.len() >= MAX_EMPREINTES {
            self.empreintes.retain(|e| vivante(e));
        }
        if self.empreintes.len() >= MAX_EMPREINTES {
            return false;
        }
        let signature = signature(raster, &masque);
        self.empreintes.push(Empreinte { lien, cel, masque: Arc::new(masque), signature });
        true
    }

    /// Les morceaux sous le cercle (`p`, `rayon`) : seulement ceux par-dessus une case (`dessus`) ou seulement ceux du papier.
    pub fn touches(&self, layout: &BoardLayout, p: Pos2, rayon: f32, dessus: bool) -> Vec<usize> {
        (0..self.morceaux.len()).filter(|&i| self.morceaux[i].dessus == dessus && self.morceaux[i].touche(layout, p, rayon)).collect()
    }

    /// Retire les morceaux `rangs` (la gomme : chacun entier). Rend combien.
    pub fn retirer(&mut self, rangs: &[usize]) -> usize {
        let mut v = rangs.to_vec();
        v.sort_unstable();
        v.dedup();
        for &i in v.iter().rev() {
            if i < self.morceaux.len() {
                self.morceaux.remove(i);
            }
        }
        v.len()
    }

    /// Ce que couvrent les morceaux sur la planche.
    pub fn etendue(&self, layout: &BoardLayout) -> Option<Rect> {
        self.morceaux.iter().filter_map(|m| m.bornes(layout)).reduce(|a, b| a.union(b))
    }

    /// Les morceaux dont la case n'est plus là passent à la voisine la plus proche, à la même place de la planche (`Annotations::rattacher_manquantes`).
    pub fn rattacher_manquantes(&mut self, avant: Option<&BoardLayout>, apres: &BoardLayout) -> usize {
        if apres.cards.is_empty() {
            return 0;
        }
        let mut n = 0;
        for m in &mut self.morceaux {
            if apres.cards.iter().any(|c| c.id == m.anchor.card) || apres.masquees.contains(&m.anchor.card) {
                continue;
            }
            let nouveau = avant.and_then(|b| {
                let v = m.vers_planche(b)?;
                let centre = m.bornes(b)?.center();
                let near = b.cards.iter().filter(|k| apres.cards.iter().any(|a| a.id == k.id)).min_by(|x, y| distance(x.thumb, centre).total_cmp(&distance(y.thumb, centre)))?;
                let rang = apres.cards.iter().position(|a| a.id == near.id)?;
                Some((Ancre { card: near.id, after: 0, rank: rang as u32 }, v.puis(&vers_relatif(near.thumb))))
            });
            match nouveau {
                Some((a, pose)) => {
                    m.anchor = a;
                    m.pose = pose;
                }
                None => {
                    let rang = (m.anchor.rank as usize).min(apres.cards.len() - 1);
                    m.anchor = Ancre { card: apres.cards[rang].id, after: 0, rank: rang as u32 };
                }
            }
            n += 1;
        }
        n
    }

    pub fn noter_rangs(&mut self, layout: &BoardLayout) -> bool {
        let mut change = false;
        for m in &mut self.morceaux {
            if let Some((rang, _)) = annotations::case_de(layout, &m.anchor).filter(|(r, _)| *r != usize::MAX) {
                if m.anchor.rank as usize != rang {
                    m.anchor.rank = rang as u32;
                    change = true;
                }
            }
        }
        change
    }

    /// Ce que pèse la couche en mémoire : pixels (les tuiles rangées comptent leur tampon), masques.
    pub fn octets(&self) -> usize {
        self.morceaux.iter().map(Morceau::octets).sum::<usize>() + self.empreintes.iter().map(|e| e.masque.octets()).sum::<usize>()
    }

}

/// Une copie à soi des pixels de `r`, ouverts (des tuiles neuves) : la lire ne rouvre aucune tuile rangée de `r`, que d'autres tiennent (un morceau, l'annulation).
pub fn copie_ouverte(r: &Raster) -> Raster {
    let mut out = Raster::new(r.width, r.height);
    for id in r.drawn_ids() {
        if let Some(t) = r.slot(id) {
            let neuve = t.avec_les_pixels(|p| Tile::new(*p));
            out.replace_slot(id, Some(neuve));
        }
    }
    out
}

fn distance(r: Rect, p: Pos2) -> f32 {
    let dx = (r.left() - p.x).max(p.x - r.right()).max(0.0);
    let dy = (r.top() - p.y).max(p.y - r.bottom()).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

// ------------------------------------------------------------------------------------ les blocs (écran et export)

/// Le niveau qui convient quand un pixel couvre `par_pixel` pixels de la destination (1, 1/2, 1/4…).
fn niveau_pour(par_pixel: f64) -> usize {
    if !(par_pixel > 0.0) || par_pixel >= 1.0 {
        return 0;
    }
    ((1.0 / par_pixel).log2().floor().max(0.0) as usize).min(NIVEAUX - 1)
}

/// Les blocs (au niveau `f` : 1, 2, 4…) où le morceau a des tuiles.
fn blocs_de(r: &Raster, f: i64) -> Vec<(i64, i64)> {
    let t = TILE as i64;
    let mut v: Vec<(i64, i64)> = Vec::new();
    for id in r.drawn_ids() {
        let (ox, oy) = r.tile_origin(id);
        let (x0, x1) = (ox.div_euclid(BLOC * f), (ox + t - 1).div_euclid(BLOC * f));
        let (y0, y1) = (oy.div_euclid(BLOC * f), (oy + t - 1).div_euclid(BLOC * f));
        for by in y0..=y1 {
            for bx in x0..=x1 {
                v.push((bx, by));
            }
        }
    }
    v.sort_unstable();
    v.dedup();
    v
}

/// Les pixels du bloc (`bx`, `by`) au niveau `f` : chacun la moyenne prémultipliée de `f` × `f` pixels. Rien s'il est vide.
fn pixels_du_bloc(r: &Raster, f: i64, bx: i64, by: i64) -> Option<Bloc> {
    let (x0, y0) = (bx * BLOC * f, by * BLOC * f);
    let t = TILE as i64;
    // (Seulement le rectangle que couvrent les tuiles dessinées du bloc : un trait fin n'alloue pas un bloc entier de sommes.)
    let mut tuiles: Vec<(i64, i64)> = Vec::new();
    for ty in y0.div_euclid(t)..=(y0 + BLOC * f - 1).div_euclid(t) {
        for tx in x0.div_euclid(t)..=(x0 + BLOC * f - 1).div_euclid(t) {
            if r.tile_at(tx as i32, ty as i32).is_some() {
                tuiles.push((tx, ty));
            }
        }
    }
    if tuiles.is_empty() {
        return None;
    }
    let niv = |v: i64, o: i64| ((v - o).div_euclid(f)).clamp(0, BLOC);
    let lx0 = tuiles.iter().map(|&(tx, _)| niv(tx * t, x0)).min().unwrap_or(0);
    let ly0 = tuiles.iter().map(|&(_, ty)| niv(ty * t, y0)).min().unwrap_or(0);
    let lx1 = tuiles.iter().map(|&(tx, _)| niv((tx + 1) * t + f - 1, x0)).max().unwrap_or(0);
    let ly1 = tuiles.iter().map(|&(_, ty)| niv((ty + 1) * t + f - 1, y0)).max().unwrap_or(0);
    let (sw, sh) = ((lx1 - lx0).max(1) as usize, (ly1 - ly0).max(1) as usize);
    let mut somme = vec![[0u32; 4]; sw * sh];
    let mut vu = false;
    for &(tx, ty) in &tuiles {
        {
            let Some(tuile) = r.tile_at(tx as i32, ty as i32) else { continue };
            tuile.avec_les_pixels(|tuile| {
                for (k, c) in tuile.iter().enumerate() {
                    if c.a() == 0 {
                        continue;
                    }
                    let (x, y) = (tx * t + (k % TILE) as i64 - x0, ty * t + (k / TILE) as i64 - y0);
                    if x < 0 || y < 0 || x >= BLOC * f || y >= BLOC * f {
                        continue;
                    }
                    let (sx, sy) = (x / f - lx0, y / f - ly0);
                    if sx < 0 || sy < 0 || sx as usize >= sw || sy as usize >= sh {
                        continue;
                    }
                    let s = &mut somme[sy as usize * sw + sx as usize];
                    s[0] += u32::from(c.r());
                    s[1] += u32::from(c.g());
                    s[2] += u32::from(c.b());
                    s[3] += u32::from(c.a());
                    vu = true;
                }
            });
        }
    }
    if !vu {
        return None;
    }
    // Rogné à l'encre, avec un pixel transparent autour (l'échantillonnage lisse ne mord pas sur le bord) : un trait fin ne coûte que son encre. (Les
    // coordonnées sont celles du bloc entier.)
    let n = BLOC as usize;
    let (mut bx0, mut by0, mut bx1, mut by1) = (n, n, 0, 0);
    for j in 0..sh {
        for i in 0..sw {
            if somme[j * sw + i][3] != 0 {
                let (gi, gj) = (i + lx0 as usize, j + ly0 as usize);
                (bx0, by0, bx1, by1) = (bx0.min(gi), by0.min(gj), bx1.max(gi + 1), by1.max(gj + 1));
            }
        }
    }
    let (bx0, by0, bx1, by1) = (bx0.saturating_sub(1), by0.saturating_sub(1), (bx1 + 1).min(n), (by1 + 1).min(n));
    let q = (f * f) as u32;
    let mut px = Vec::with_capacity((bx1 - bx0) * (by1 - by0));
    for j in by0..by1 {
        for i in bx0..bx1 {
            let (si, sj) = (i as i64 - lx0, j as i64 - ly0);
            let s = if si >= 0 && sj >= 0 && (si as usize) < sw && (sj as usize) < sh { somme[sj as usize * sw + si as usize] } else { [0; 4] };
            px.push(Color32::from_rgba_premultiplied(((s[0] + q / 2) / q) as u8, ((s[1] + q / 2) / q) as u8, ((s[2] + q / 2) / q) as u8, ((s[3] + q / 2) / q) as u8));
        }
    }
    Some(Bloc { px, ox: bx0, oy: by0, w: bx1 - bx0, h: by1 - by0 })
}

/// Les pixels rognés d'un bloc : le rectangle `ox`, `oy`, `w` × `h` (pixels de son niveau, depuis le coin du bloc).
struct Bloc {
    px: Vec<Color32>,
    ox: usize,
    oy: usize,
    w: usize,
    h: usize,
}

impl Bloc {
    fn coins(&self) -> [Pos2; 4] {
        let (x0, y0, x1, y1) = (self.ox as f32, self.oy as f32, (self.ox + self.w) as f32, (self.oy + self.h) as f32);
        [pos2(x0, y0), pos2(x1, y0), pos2(x1, y1), pos2(x0, y1)]
    }
}

/// Du pixel d'un bloc (`bx`, `by`, niveau `f`) au pixel du morceau.
fn bloc_vers_pixels(f: i64, bx: i64, by: i64) -> A64 {
    A64::echelle_decalage(f as f64, (bx * BLOC * f) as f64, (by * BLOC * f) as f64)
}

const BUDGET_IMAGE: usize = 4 << 20;
const BUDGET_TOTAL: usize = 192 << 20;
const OUBLI: u64 = 240;

/// Les textures des morceaux à la carte graphique : seuls les blocs visibles, au niveau qui convient au zoom, dans une limite d'octets par image
/// d'écran ; ce qu'on ne voit plus est rendu. Rien n'est gardé en double sur le processeur : les pixels d'un bloc sont faits, envoyés, oubliés.
#[derive(Default)]
pub struct Textures {
    /// (pixels, niveau, bloc) → la texture, l'image d'écran où elle a servi, ses octets, ses coins (pixels du niveau, depuis le coin du bloc).
    #[allow(clippy::type_complexity)]
    blocs: HashMap<(u64, usize, i64, i64), (TextureHandle, u64, usize, [Pos2; 4])>,
    /// Les blocs de chaque pixels, par niveau.
    listes: HashMap<(u64, usize), Arc<Vec<(i64, i64)>>>,
    image: u64,
    envoyes: usize,
    pub total_envoye: usize,
}

impl Textures {
    pub fn debut(&mut self) {
        self.image += 1;
        self.envoyes = 0;
        let image = self.image;
        self.blocs.retain(|_, e| e.1 + OUBLI >= image);
        let mut total: usize = self.blocs.values().map(|e| e.2).sum();
        if total > BUDGET_TOTAL {
            let mut ages: Vec<(u64, (u64, usize, i64, i64))> = self.blocs.iter().map(|(k, e)| (e.1, *k)).collect();
            ages.sort_unstable();
            for (_, k) in ages {
                if total <= BUDGET_TOTAL / 10 * 9 {
                    break;
                }
                if let Some(e) = self.blocs.remove(&k) {
                    total -= e.2;
                }
            }
        }
    }

    pub fn nombre(&self) -> usize {
        self.blocs.len()
    }

    /// Peint les morceaux `dessus` (ou ceux du papier) de `b`, à l'opacité `opacite`. Faux si un bloc n'a pas pu être envoyé à cette image (budget) :
    /// l'appelant redemande une image.
    pub fn peindre(&mut self, painter: &egui::Painter, view: Rect, xf: crate::boardui::Xf, layout: &BoardLayout, b: &PlancheBitmap, dessus: bool, opacite: f32) -> bool {
        let teinte = Color32::from_white_alpha((opacite.clamp(0.0, 1.0) * 255.0).round() as u8);
        let mut complet = true;
        // (Les cases d'un coup : une recherche par morceau, sans refaire la liste des cases pour chacun.)
        let index: HashMap<u64, Rect> = layout.cards.iter().chain(layout.annulees.iter()).map(|c| (c.id, c.thumb)).collect();
        for m in b.morceaux.iter().filter(|m| m.dessus == dessus) {
            let r = if m.anchor.after == 0 { index.get(&m.anchor.card).copied() } else { None }.or_else(|| annotations::case_de(layout, &m.anchor).map(|(_, r)| r));
            let Some(r) = r else { continue };
            let v = m.pose.puis(&depuis_relatif(r));
            let Some((x0, y0, x1, y1)) = boite_px(&m.pixels) else { continue };
            let bornes = Rect::from_points(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)].map(|(x, y)| v.point(pos2(x as f32, y as f32))));
            let ecran = xf.rect(bornes);
            // (Hors de la vue, ou plus petit qu'un demi-point d'écran : rien à peindre.)
            if !ecran.expand(2.0).intersects(view) || (ecran.width() < 0.5 && ecran.height() < 0.5) {
                continue;
            }
            complet &= self.peindre_pixels(painter, view, xf, m.id, &m.pixels, v, teinte);
        }
        complet
    }

    /// Peint les pixels `pixels`, posés sur la planche par `v`, à la teinte `teinte` (blanc prémultiplié : l'opacité). Faux si un bloc attend le budget.
    /// `cle_px` : un numéro unique pour ces pixels (ils ne changent jamais sous ce numéro).
    #[allow(clippy::too_many_arguments)]
    pub fn peindre_pixels(&mut self, painter: &egui::Painter, view: Rect, xf: crate::boardui::Xf, cle_px: u64, pixels: &Raster, v: A64, teinte: Color32) -> bool {
        let ecran = A64::echelle_decalage(xf.zoom as f64, xf.origin.x as f64, xf.origin.y as f64);
        let vers_ecran = v.puis(&ecran);
        let niveau = niveau_pour(vers_ecran.echelle());
        let f = 1i64 << niveau;
        let mut complet = true;
        // (La liste des blocs d'un morceau ne change jamais sous son numéro : faite une fois par niveau.)
        if self.listes.len() > 50_000 {
            self.listes.clear();
        }
        let liste = self.listes.entry((cle_px, niveau)).or_insert_with(|| Arc::new(blocs_de(pixels, f))).clone();
        for &(bx, by) in liste.iter() {
            let a = bloc_vers_pixels(f, bx, by).puis(&vers_ecran);
            let n = BLOC as f32;
            let coins = [pos2(0.0, 0.0), pos2(n, 0.0), pos2(n, n), pos2(0.0, n)].map(|p| a.point(p));
            if !Rect::from_points(&coins).intersects(view) {
                continue;
            }
            let cle = (cle_px, niveau, bx, by);
            if !self.blocs.contains_key(&cle) {
                if self.envoyes >= BUDGET_IMAGE {
                    complet = false;
                    continue;
                }
                let Some(bloc) = pixels_du_bloc(pixels, f, bx, by) else { continue };
                let octets = bloc.w * bloc.h * 4;
                let rogne = bloc.coins();
                let image = ColorImage { size: [bloc.w, bloc.h], pixels: bloc.px };
                let tex = painter.ctx().load_texture(format!("planche-bitmap {cle_px:x} {niveau} {bx} {by}"), image, TextureOptions::LINEAR);
                self.envoyes += octets;
                self.total_envoye += octets;
                self.blocs.insert(cle, (tex, self.image, octets, rogne));
            }
            let Some((tex, vu, _, rogne)) = self.blocs.get_mut(&cle) else { continue };
            *vu = self.image;
            let coins = rogne.map(|p| a.point(p));
            let mut mesh = egui::Mesh::with_texture(tex.id());
            let uv = [pos2(0.0, 0.0), pos2(1.0, 0.0), pos2(1.0, 1.0), pos2(0.0, 1.0)];
            for (p, u) in coins.iter().zip(uv) {
                mesh.vertices.push(egui::epaint::Vertex { pos: *p, uv: u, color: teinte });
            }
            mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
            painter.add(egui::Shape::mesh(mesh));
        }
        complet
    }
}

/// Peint les morceaux `dessus` (ou ceux du papier) dans `pix` (l'export de la planche : `cadre` est ce que l'image montre, `s` pixels par unité).
pub fn peindre_pixmap(pix: &mut Pixmap, layout: &BoardLayout, b: &PlancheBitmap, dessus: bool, cadre: Rect, s: f32) {
    let vers_image = A64::echelle_decalage(s as f64, -(cadre.left() as f64) * s as f64, -(cadre.top() as f64) * s as f64);
    let image = Rect::from_min_size(Pos2::ZERO, egui::vec2(pix.width() as f32, pix.height() as f32));
    for m in b.morceaux.iter().filter(|m| m.dessus == dessus) {
        // (Seulement ce que l'image exportée montre.)
        if !m.bornes(layout).is_some_and(|r| r.intersects(cadre)) {
            continue;
        }
        let Some(v) = m.vers_planche(layout) else { continue };
        let vi = v.puis(&vers_image);
        let niveau = niveau_pour(vi.echelle());
        let f = 1i64 << niveau;
        let mut paint = PixmapPaint::default();
        paint.quality = FilterQuality::Bilinear;
        for (bx, by) in blocs_de(&m.pixels, f) {
            let n = BLOC as f32;
            let coins = [pos2(0.0, 0.0), pos2(n, 0.0), pos2(n, n), pos2(0.0, n)].map(|p| bloc_vers_pixels(f, bx, by).puis(&vi).point(p));
            if !Rect::from_points(&coins).intersects(image) {
                continue;
            }
            let Some(b) = pixels_du_bloc(&m.pixels, f, bx, by) else { continue };
            let mut octets = Vec::with_capacity(b.px.len() * 4);
            for c in &b.px {
                octets.extend_from_slice(&[c.r(), c.g(), c.b(), c.a()]);
            }
            let Some(bloc) = IntSize::from_wh(b.w as u32, b.h as u32).and_then(|sz| Pixmap::from_vec(octets, sz)) else { continue };
            let a = A64::echelle_decalage(1.0, b.ox as f64, b.oy as f64).puis(&bloc_vers_pixels(f, bx, by)).puis(&vi);
            if !a.fini() {
                continue;
            }
            pix.draw_pixmap(0, 0, bloc.as_ref(), &paint, Transform::from_row(a.a as f32, a.c as f32, a.b as f32, a.d as f32, a.tx as f32, a.ty as f32), None);
        }
    }
}

// ------------------------------------------------------------------------------------ le fichier

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MorceauFichier {
    id: u64,
    ancre: Ancre,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    dessus: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lien: Option<u64>,
    pose: [f64; 6],
    colonnes: usize,
    tuiles: Vec<[i32; 2]>,
}

#[derive(Serialize, Deserialize)]
struct EmpreinteFichier {
    lien: u64,
    dessin: CelId,
    colonnes: usize,
    tuiles: Vec<[i32; 2]>,
    /// La signature des pixels sous le masque (absente : 0, jamais fiable). En texte : un `u64` ne tient pas dans un nombre JSON lu en `f64`.
    #[serde(default)]
    signature: String,
}

#[derive(Serialize, Deserialize)]
struct Fichier {
    format: String,
    version: u32,
    #[serde(default)]
    morceaux: Vec<MorceauFichier>,
    #[serde(default)]
    empreintes: Vec<EmpreinteFichier>,
}

fn nom_morceau(id: u64) -> String {
    format!("m{id}.png")
}

fn nom_empreinte(lien: u64, cel: CelId) -> String {
    format!("e{lien}-{cel}.png")
}

/// L'image PNG de l'atlas des tuiles (`colonnes` de large, dans l'ordre de `tuiles`), lues sans garder ouvertes celles qui sont rangées.
fn atlas_png(tuiles: &[((i32, i32), Tile)]) -> Result<(Vec<u8>, usize), String> {
    let cols = (tuiles.len() as f64).sqrt().ceil().max(1.0) as usize;
    let rows = tuiles.len().div_ceil(cols).max(1);
    let (w, h) = (cols * TILE, rows * TILE);
    let mut rgba = vec![0u8; w * h * 4];
    for (i, (_, t)) in tuiles.iter().enumerate() {
        let (ox, oy) = ((i % cols) * TILE, (i / cols) * TILE);
        t.avec_les_pixels(|px| {
            for (k, c) in px.iter().enumerate() {
                if c.a() != 0 {
                    let o = ((oy + k / TILE) * w + ox + k % TILE) * 4;
                    rgba[o..o + 4].copy_from_slice(&crate::raster::unpremultiply(*c));
                }
            }
        });
    }
    Ok((crate::io::encode_png(&rgba, w, h)?, cols))
}

fn tuiles_du_masque(m: &Masque) -> Vec<((i32, i32), Tile)> {
    let blanc = Color32::WHITE;
    m.tuiles
        .iter()
        .map(|(k, bits)| {
            let mut px = [Color32::TRANSPARENT; TILE_PIXELS];
            for (j, l) in bits.iter().enumerate() {
                for i in 0..TILE {
                    if l >> i & 1 == 1 {
                        px[j * TILE + i] = blanc;
                    }
                }
            }
            (*k, Tile::new(px))
        })
        .collect()
}

/// Lit l'atlas `chemin` : ses tuiles, dans l'ordre de `tuiles`.
fn lire_atlas(chemin: &Path, colonnes: usize, tuiles: &[[i32; 2]]) -> Result<Vec<((i32, i32), Tile)>, String> {
    if tuiles.len() > MAX_TUILES_LUES || colonnes == 0 || colonnes > 4096 {
        return Err("index démesuré".into());
    }
    let octets = std::fs::read(chemin).map_err(|e| e.to_string())?;
    let img = image::load_from_memory(&octets).map_err(|e| e.to_string())?.into_rgba8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    if w % TILE != 0 || h % TILE != 0 || w / TILE < colonnes || (h / TILE) * colonnes < tuiles.len() {
        return Err("l'image ne correspond pas à son index".into());
    }
    let a = Raster::from_rgba(w, h, img.as_raw());
    Ok(tuiles.iter().enumerate().filter_map(|(i, [x, y])| a.tiles.get((i / colonnes) * a.tiles_x + i % colonnes).cloned().flatten().map(|t| ((*x, *y), t))).collect())
}

/// Au-delà, un index est tenu pour abîmé : il n'est pas chargé, et rien n'est réécrit ni effacé (`illisible`).
const MAX_LUS: usize = 100_000;

/// La couche bitmap de la planche du projet `dir` (absente : vide). Un index illisible, d'une version plus récente ou démesuré : vide, marquée `illisible`
/// (rien n'y sera posé, réécrit ni effacé), et un avertissement. Un morceau dont l'image manque ou ne se lit pas est gardé tel quel dans l'index
/// (`fantomes`) : il n'est pas montré, son fichier n'est jamais effacé, il revient à la réouverture si l'image arrive (un dossier synchronisé).
pub fn lire(dir: &Path, avertissements: &mut Vec<String>) -> PlancheBitmap {
    let d = dir.join(DOSSIER);
    let illisible = PlancheBitmap { illisible: true, dossier_lu: Some(dir.to_path_buf()), ..Default::default() };
    let texte = match std::fs::read_to_string(d.join(INDEX)) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // (Pas d'index, mais des images : un dossier en cours de synchronisation ; on n'y touche pas.)
            let images = std::fs::read_dir(&d).map(|r| r.flatten().any(|e| e.file_name().to_string_lossy().ends_with(".png"))).unwrap_or(false);
            if images {
                avertissements.push(format!("{DOSSIER} : des images sans leur index ; le dessin bitmap hors des cases n'est pas chargé (le dossier reste en place)."));
                return illisible;
            }
            return PlancheBitmap { dossier_lu: Some(dir.to_path_buf()), ..Default::default() };
        }
        Err(e) => {
            avertissements.push(format!("{DOSSIER}/{INDEX} : {e} ; le dessin bitmap hors des cases n'est pas chargé (le dossier reste en place)."));
            return illisible;
        }
    };
    let f: Fichier = match serde_json::from_str(texte.trim_start_matches('\u{feff}')) {
        Ok(f) => f,
        Err(e) => {
            avertissements.push(format!("{DOSSIER}/{INDEX} : illisible ({e}) ; le dessin bitmap hors des cases n'est pas chargé (le dossier reste en place)."));
            return illisible;
        }
    };
    if f.format != FORMAT || f.version > VERSION {
        avertissements.push(format!("{DOSSIER}/{INDEX} : écrit par une version plus récente ; le dessin bitmap hors des cases n'est pas chargé (le dossier reste en place)."));
        return illisible;
    }
    if f.morceaux.len() > MAX_LUS || f.empreintes.len() > MAX_LUS {
        avertissements.push(format!("{DOSSIER}/{INDEX} : démesuré ; le dessin bitmap hors des cases n'est pas chargé (le dossier reste en place)."));
        return illisible;
    }
    let mut out = PlancheBitmap { dossier_lu: Some(dir.to_path_buf()), ..Default::default() };
    for m in f.morceaux {
        let pose = A64::du_tableau(m.pose);
        match lire_atlas(&d.join(nom_morceau(m.id)), m.colonnes, &m.tuiles) {
            Ok(tuiles) if pose.fini() && pose.inverse().is_some() => {
                let mut r = pixels_vides();
                for ((x, y), t) in tuiles {
                    r.absorb_beyond(x, y, t);
                }
                r.geler();
                out.morceaux.push(Morceau { id: m.id, anchor: m.ancre, dessus: m.dessus, lien: m.lien, pose, pixels: Arc::new(r) });
            }
            _ => out.fantomes.push(m),
        }
    }
    for e in f.empreintes {
        let Ok(tuiles) = lire_atlas(&d.join(nom_empreinte(e.lien, e.dessin)), e.colonnes, &e.tuiles) else { continue };
        let mut masque = Masque::default();
        for ((tx, ty), t) in tuiles {
            let mut bits = [0u64; TILE];
            for (k, c) in t.iter().enumerate() {
                if c.a() > 127 {
                    bits[k / TILE] |= 1u64 << (k % TILE);
                }
            }
            if bits.iter().any(|b| *b != 0) {
                masque.tuiles.insert((tx, ty), Box::new(bits));
            }
        }
        if !masque.est_vide() && !out.empreintes.iter().any(|x| x.lien == e.lien && x.cel == e.dessin) {
            out.empreintes.push(Empreinte { lien: e.lien, cel: e.dessin, masque: Arc::new(masque), signature: u64::from_str_radix(&e.signature, 16).unwrap_or(0) });
        }
    }
    if !out.fantomes.is_empty() {
        avertissements.push(format!(
            "{DOSSIER} : {} morceau(x) de dessin hors des cases sans image lisible, pas montré(s) ; gardé(s) dans l'index, leurs fichiers ne sont jamais effacés.",
            out.fantomes.len()
        ));
    }
    out
}

/// Recopie de `de` vers `vers` les fichiers qui n'y sont pas (rien n'est écrasé). Rend leur nombre.
fn recopier_manquants(de: &Path, vers: &Path, seulement: Option<&[String]>) -> Result<usize, String> {
    let Ok(entrees) = std::fs::read_dir(de) else { return Ok(0) };
    std::fs::create_dir_all(vers).map_err(|e| format!("{} : {e}", vers.display()))?;
    let mut n = 0;
    for e in entrees.flatten() {
        let nom = e.file_name().to_string_lossy().to_string();
        if seulement.is_some_and(|s| !s.contains(&nom)) || !e.path().is_file() || vers.join(&nom).exists() {
            continue;
        }
        std::fs::copy(e.path(), vers.join(&nom)).map_err(|x| format!("{} : {x}", e.path().display()))?;
        n += 1;
    }
    Ok(n)
}

/// Écrit la couche dans `dir/planche_bitmap/` : les images qui manquent (un morceau ne change jamais : son image écrite reste juste), l'index, puis
/// retire les images que rien ne cite. Rien à écrire : le dossier se vide. Une couche lue `illisible` : rien n'est touché dans son dossier ; dans un autre
/// dossier (« Enregistrer sous »), le sien y est recopié tel quel. `nettoyer_aussi` : faux au premier passage (avant les dessins : une coupure ne perd rien,
/// au pire un pixel est à deux endroits), vrai au dernier (après le manifeste). Rend le nombre de fichiers écrits, recopiés ou retirés, plus un si l'index a changé.
pub fn ecrire(dir: &Path, b: &PlancheBitmap, nettoyer_aussi: bool) -> Result<usize, String> {
    let d = dir.join(DOSSIER);
    // (Le dossier d'où la couche a été lue, s'il n'est pas celui-ci : « Enregistrer sous ».)
    let ailleurs = b.dossier_lu.as_ref().filter(|s| s.as_path() != dir).map(|s| s.join(DOSSIER)).filter(|s| s.is_dir());
    if b.illisible {
        return match ailleurs {
            Some(src) => recopier_manquants(&src, &d, None),
            None => Ok(0),
        };
    }
    if b.est_vide() {
        if nettoyer_aussi && d.join(INDEX).exists() {
            crate::project::write_sidecar(&d.join(INDEX), None, |t| serde_json::from_str::<Fichier>(t).is_ok_and(|f| f.format == FORMAT && f.version <= VERSION))?;
            let n = nettoyer(&d, &[]);
            let _ = std::fs::remove_dir(&d);
            return Ok(n + 1);
        }
        return Ok(0);
    }
    std::fs::create_dir_all(&d).map_err(|e| format!("{} : {e}", d.display()))?;
    let mut n = 0;
    let mut fichier = Fichier { format: FORMAT.into(), version: VERSION, morceaux: Vec::new(), empreintes: Vec::new() };
    let mut gardes: Vec<String> = Vec::new();
    for m in &b.morceaux {
        let tuiles: Vec<((i32, i32), Tile)> = m.pixels.drawn_ids().into_iter().filter_map(|id| Some((m.pixels.key_of(id), m.pixels.slot(id)?.clone()))).collect();
        let colonnes = (tuiles.len() as f64).sqrt().ceil().max(1.0) as usize;
        let nom = nom_morceau(m.id);
        if !d.join(&nom).exists() {
            crate::project::write_atomic(&d.join(&nom), &atlas_png(&tuiles)?.0)?;
            n += 1;
        }
        gardes.push(nom);
        fichier.morceaux.push(MorceauFichier { id: m.id, ancre: m.anchor, dessus: m.dessus, lien: m.lien, pose: m.pose.tableau(), colonnes, tuiles: tuiles.iter().map(|((x, y), _)| [*x, *y]).collect() });
    }
    // Les morceaux sans image lisible : tels quels dans l'index, leur fichier gardé (et recopié d'où la couche a été lue, s'il y est).
    let noms_fantomes: Vec<String> = b.fantomes.iter().map(|m| nom_morceau(m.id)).collect();
    if let Some(src) = &ailleurs {
        n += recopier_manquants(src, &d, Some(&noms_fantomes))?;
    }
    gardes.extend(noms_fantomes);
    fichier.morceaux.extend(b.fantomes.iter().cloned());
    for e in &b.empreintes {
        // (Les clés des tuiles du masque, sans fabriquer ses pixels : l'image ne se fait que si elle manque.)
        let cles: Vec<[i32; 2]> = e.masque.tuiles.keys().map(|&(x, y)| [x, y]).collect();
        let colonnes = (cles.len() as f64).sqrt().ceil().max(1.0) as usize;
        let nom = nom_empreinte(e.lien, e.cel);
        if !d.join(&nom).exists() {
            crate::project::write_atomic(&d.join(&nom), &atlas_png(&tuiles_du_masque(&e.masque))?.0)?;
            n += 1;
        }
        gardes.push(nom);
        fichier.empreintes.push(EmpreinteFichier { lien: e.lien, dessin: e.cel, colonnes, tuiles: cles, signature: format!("{:x}", e.signature) });
    }
    let octets = serde_json::to_vec_pretty(&fichier).map_err(|e| e.to_string())?;
    // (Un index qui change compte : une suppression seule doit faire avancer la révision d'un film partagé.)
    if std::fs::read(d.join(INDEX)).ok().as_deref() != Some(octets.as_slice()) {
        n += 1;
    }
    crate::project::write_sidecar(&d.join(INDEX), Some(&octets), |t| serde_json::from_str::<Fichier>(t).is_ok_and(|f| f.format == FORMAT && f.version <= VERSION))?;
    if nettoyer_aussi {
        n += nettoyer(&d, &gardes);
    }
    Ok(n)
}
/// Retire de `d` les images de morceaux et d'empreintes que l'index ne cite plus (rien d'autre).
fn nettoyer(d: &Path, gardes: &[String]) -> usize {
    let Ok(entrees) = std::fs::read_dir(d) else { return 0 };
    let mut n = 0;
    for e in entrees.flatten() {
        let nom = e.file_name().to_string_lossy().to_string();
        let a_nous = (nom.starts_with('m') || nom.starts_with('e')) && nom.ends_with(".png") && nom[1..nom.len() - 4].chars().all(|c| c.is_ascii_digit() || c == '-');
        if a_nous && !gardes.contains(&nom) && std::fs::remove_file(e.path()).is_ok() {
            n += 1;
        }
    }
    n
}

/// Essais : ce que pèserait le masque `m` autrement (la compression des empreintes, partie 127) : (en mémoire, 1 bit par pixel ; par plages, deux octets
/// par plage et un par rangée ; son image PNG d'aujourd'hui ; une PNG en niveaux de gris).
#[cfg(test)]
pub(crate) fn tailles_d_un_masque(m: &Masque) -> (usize, usize, usize, usize) {
    let brut = m.tuiles.len() * TILE * 8;
    let mut plages = 0;
    for bits in m.tuiles.values() {
        for l in bits.iter() {
            plages += 1;
            let mut prec = false;
            for i in 0..TILE {
                let b = l >> i & 1 == 1;
                if b && !prec {
                    plages += 2;
                }
                prec = b;
            }
        }
    }
    let png = atlas_png(&tuiles_du_masque(m)).map_or(0, |p| p.0.len());
    let cles: Vec<_> = m.tuiles.keys().collect();
    let cols = (cles.len() as f64).sqrt().ceil().max(1.0) as usize;
    let rows = cles.len().div_ceil(cols).max(1);
    let (w, h) = (cols * TILE, rows * TILE);
    let mut gris = vec![0u8; w * h];
    for (i, bits) in m.tuiles.values().enumerate() {
        let (ox, oy) = ((i % cols) * TILE, (i / cols) * TILE);
        for (j, l) in bits.iter().enumerate() {
            for k in 0..TILE {
                if l >> k & 1 == 1 {
                    gris[(oy + j) * w + ox + k] = 255;
                }
            }
        }
    }
    use image::ImageEncoder;
    let mut sortie = Vec::new();
    let _ = image::codecs::png::PngEncoder::new(&mut sortie).write_image(&gris, w as u32, h as u32, image::ExtendedColorType::L8);
    (brut, plages, png, sortie.len())
}
#[cfg(test)]
#[path = "planchebitmap_tests.rs"]
mod tests;
