//! Adapter ce qu'on colle à la feuille du projet qui le reçoit (Anthony, 2026-10-20 : « un plan collé dans un projet
//! d'une autre taille de feuille doit s'adapter »).
//!
//! Avant, un plan, un calque, des images ou des clés copiés dans un projet d'une autre taille de feuille étaient
//! refusés (un dessin n'a de sens qu'à sa taille). Maintenant la copie est **adaptée** : le cadre de la caméra du
//! projet d'origine est ajusté au cadre de la caméra du projet qui reçoit, de sorte que ce que la caméra voyait
//! reste ce que la caméra voit.
//!
//! # La règle
//!
//! Une similitude `p' = s·p + t` (même échelle en largeur et en hauteur : rien n'est déformé) :
//!
//! - `s` = cadre d'arrivée / cadre d'origine. Les deux cadres n'ont pas toujours les mêmes proportions :
//!   **Contenir** (d'origine) prend le plus petit des deux rapports (tout le cadre d'origine reste visible, des
//!   bandes apparaissent sur l'autre côté) ; **Remplir** prend le plus grand (le cadre d'arrivée est rempli, ce qui
//!   dépasse sort du cadre, mais reste sur la feuille : rien n'est perdu, la toile est infinie).
//! - `t` pose le centre de la feuille d'origine sur celui de la feuille d'arrivée : le cadre est centré sur la
//!   feuille (`Document::frame_rect`), donc centre de feuille = centre du cadre.
//! - Même cadre mais feuille d'une autre taille (feuille agrandie à la main) : `s = 1`, un simple décalage de
//!   `t` pixels (arrondi à l'entier : aucun rééchantillonnage, les pixels sont copiés tels quels).
//! - Même cadre, même feuille : l'adaptation est l'identité et **rien n'est touché** (comportement d'avant).
//!
//! # Ce qui est mis à l'échelle
//!
//! | Quoi | Comment |
//! |---|---|
//! | dessins bitmap (feuille et hors-feuille) | rééchantillonnés (`reechantillonne`) |
//! | dessins vectoriels | points, épaisseurs, tailles de pinceau et de texte (`Element::transformed`), puis rendu exact |
//! | strate Fond d'un dessin (peinture et coupe) | comme un bitmap |
//! | pose d'un calque, clés de position | décalage × `s` ; échelle, étirement et rotation inchangés (pivot : centre de la feuille) |
//! | clés de caméra | position par la similitude ; avancée × rapport des largeurs de cadre ; zoom et angles inchangés |
//! | effets : paramètres en pixels (`px`, `px/img`), clés comprises | × `s` (bornes de l'effet respectées) ; « trous pour 100 px de trait » ÷ `s` |
//! | fond sous le trait, calque de couleur (dont son remplissage au griffonnage) | débord et fermeture des trous (pixels) × `s` |
//! | zone bitmap, éléments vectoriels collés sur le dessin | même similitude : ils gardent leur place dans le cadre |
//!
//! Ne change pas : les paramètres en pour cent (relatifs à l'image), les incrustations (en pour cent de la hauteur
//! du cadre), les marques d'image, les textes, la scène 3D de blocage, les références liées (ce que `paste_plans`
//! ne copie pas non plus).
//!
//! # La cadence
//!
//! Par défaut les durées restent en **images** (comme avant : 24 images restent 24 images). Avec
//! `Duree::Secondes`, tout ce qui compte en images est recalculé à la cadence d'arrivée : `f' = round(f × to / from)`
//! (même arrondi que « Changer la cadence » : demi vers le haut), les longueurs au moins 1. Plus lent → des
//! dessins tenus plus longtemps (jamais d'intervalle inventé) ; plus rapide → des images sautées : un dessin
//! d'une seule image peut disparaître.
//!
//! # Sans latence
//!
//! Rééchantillonner un plan de 4 K se fait sur des fils de fond (voir `collage.rs`) : ce module est pur et ne
//! connaît ni l'application ni l'interface.

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};

use eframe::egui::{pos2, vec2, Color32, Pos2, Vec2};
use serde::{Deserialize, Serialize};

use crate::camera::{Camera, CameraKey};
use crate::document::{Cel, CelId, Document, Layer, Panel, Strate};
use crate::fx::{Effect, EffectKind, ParamStyle};
use crate::pixsel::Floating;
use crate::raster::{self, Raster, Tile, TILE, TILE_PIXELS};
use crate::vector::{Affine, Element};

// ------------------------------------------------------------------------------------------------ réglages

/// Que faire quand les proportions des deux cadres ne sont pas les mêmes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ajustement {
    /// Tout le cadre d'origine reste visible (des bandes au besoin).
    #[default]
    #[serde(rename = "contenir")]
    Contenir,
    /// Le cadre d'arrivée est rempli (ce qui dépasse sort du cadre).
    #[serde(rename = "remplir")]
    Remplir,
}

/// Que faire quand les cadences ne sont pas les mêmes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Duree {
    /// Les mêmes images : la durée en secondes change.
    #[default]
    #[serde(rename = "images")]
    Images,
    /// La même durée en secondes : les images sont recalculées.
    #[serde(rename = "secondes")]
    Secondes,
}

/// Les choix retenus dans les réglages (`Settings::collage`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReglageCollage {
    #[serde(default, rename = "proportions")]
    pub ajustement: Ajustement,
    #[serde(default, rename = "cadence")]
    pub duree: Duree,
    /// Poser la question (proportions ou cadence différentes) ; décoché : les choix ci-dessus s'appliquent seuls.
    #[serde(default = "oui", rename = "demander")]
    pub demander: bool,
}

fn oui() -> bool {
    true
}

impl Default for ReglageCollage {
    fn default() -> Self {
        ReglageCollage { ajustement: Ajustement::Contenir, duree: Duree::Images, demander: true }
    }
}

// ------------------------------------------------------------------------------------------------ l'origine

/// D'où vient une copie : le cadre de la caméra, la feuille et la cadence de son projet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Origine {
    pub cadre: [usize; 2],
    pub feuille: [usize; 2],
    pub cadence: u32,
}

impl Origine {
    pub fn de(doc: &Document) -> Origine {
        Origine { cadre: doc.frame, feuille: [doc.width, doc.height], cadence: doc.fps.max(1) }
    }

    /// Les proportions des deux cadres diffèrent.
    pub fn autres_proportions(&self, autre: &Origine) -> bool {
        self.cadre[0] * autre.cadre[1] != self.cadre[1] * autre.cadre[0]
    }
}

// ------------------------------------------------------------------------------------------------ la règle

/// La transformation d'une copie vers le projet qui la reçoit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Adaptation {
    pub src: Origine,
    pub dst: Origine,
    pub ajustement: Ajustement,
    pub duree: Duree,
    /// Échelle des positions et des tailles.
    pub s: f32,
    /// Décalage : `p' = s·p + t`.
    pub t: Vec2,
}

impl Adaptation {
    pub fn new(src: Origine, dst: Origine, ajustement: Ajustement, duree: Duree) -> Adaptation {
        let (sw, sh) = (src.cadre[0].max(1) as f32, src.cadre[1].max(1) as f32);
        let (rx, ry) = (dst.cadre[0] as f32 / sw, dst.cadre[1] as f32 / sh);
        let mut s = match ajustement {
            Ajustement::Contenir => rx.min(ry),
            Ajustement::Remplir => rx.max(ry),
        };
        if !(s.is_finite() && s > 0.0) || (s - 1.0).abs() < 1e-6 {
            s = if s.is_finite() && s > 0.0 { 1.0 } else { rx.max(1e-3) };
        }
        let cs = pos2(src.feuille[0] as f32 / 2.0, src.feuille[1] as f32 / 2.0);
        let cd = pos2(dst.feuille[0] as f32 / 2.0, dst.feuille[1] as f32 / 2.0);
        let mut t = vec2(cd.x - s * cs.x, cd.y - s * cs.y);
        if s == 1.0 {
            // (Sans changement d'échelle, un décalage entier : les pixels sont copiés, pas mélangés.)
            t = vec2(t.x.round(), t.y.round());
        }
        Adaptation { src, dst, ajustement, duree, s, t }
    }

    /// Un décalage ou un changement d'échelle des dessins.
    pub fn spatiale(&self) -> bool {
        self.s != 1.0 || self.t != Vec2::ZERO
    }

    /// Les images sont recalculées à la cadence d'arrivée.
    pub fn retemps(&self) -> bool {
        self.duree == Duree::Secondes && self.src.cadence != self.dst.cadence
    }

    /// Rien à changer : la copie se colle telle quelle.
    pub fn identite(&self) -> bool {
        !self.spatiale() && !self.retemps()
    }

    pub fn affine(&self) -> Affine {
        Affine { a: self.s, b: 0.0, c: 0.0, d: self.s, tx: self.t.x, ty: self.t.y }
    }

    pub fn point(&self, p: Pos2) -> Pos2 {
        pos2(self.s * p.x + self.t.x, self.s * p.y + self.t.y)
    }

    /// Rapport des largeurs de cadre : celui des distances de référence de la caméra (`camera::ref_distance`), donc
    /// de l'avancée.
    pub fn rapport_avancee(&self) -> f32 {
        self.dst.cadre[0] as f32 / self.src.cadre[0].max(1) as f32
    }

    /// Une image d'origine à la cadence d'arrivée (identique si les images sont gardées).
    pub fn image(&self, f: usize) -> usize {
        if self.retemps() {
            crate::tween::rescale_frame(f, self.src.cadence, self.dst.cadence)
        } else {
            f
        }
    }

    /// Une durée (au moins 1 image) à la cadence d'arrivée.
    pub fn duree_en_images(&self, n: usize) -> usize {
        self.image(n).max(1)
    }

    /// Ce que le message d'état dit : « Collé depuis un projet 3840×2160 : mis à l'échelle ×0,5. »
    pub fn resume(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.s != 1.0 {
            parts.push(format!("mis à l'échelle ×{}", nombre(self.s)));
        } else if self.t != Vec2::ZERO {
            parts.push("recentré sur la feuille de ce projet".into());
        }
        if self.src.autres_proportions(&self.dst) {
            parts.push(match self.ajustement {
                Ajustement::Contenir => "cadre contenu (des bandes de chaque côté)".into(),
                Ajustement::Remplir => "cadre rempli (ce qui dépasse sort du cadre)".into(),
            });
        }
        if self.src.cadence != self.dst.cadence {
            parts.push(if self.retemps() {
                format!("durées gardées en secondes ({} → {} i/s)", self.src.cadence, self.dst.cadence)
            } else {
                format!("images gardées ({} → {} i/s : la durée change)", self.src.cadence, self.dst.cadence)
            });
        }
        format!("Collé depuis un projet {}×{} : {}.", self.src.cadre[0], self.src.cadre[1], parts.join(", "))
    }
}

/// Un nombre au plus à trois décimales, à la française (0,5 ; 0,667).
fn nombre(x: f32) -> String {
    let t = format!("{x:.3}");
    t.trim_end_matches('0').trim_end_matches('.').replace('.', ",")
}

// ------------------------------------------------------------------------------------------------ progression

/// L'avancement d'un travail de fond, lu par l'interface.
#[derive(Default)]
pub struct Progression {
    pub fait: AtomicUsize,
    pub total: AtomicUsize,
}

impl Progression {
    fn avance(&self) {
        self.fait.fetch_add(1, Ordering::Relaxed);
    }
}

/// Les fils du calcul de fond : tous les cœurs sauf un (l'interface en garde un), de 1 à 8.
fn fils_disponibles() -> usize {
    std::thread::available_parallelism().map_or(2, |n| n.get()).saturating_sub(1).clamp(1, 8)
}

// ------------------------------------------------------------------------------------------------ rééchantillonnage

/// Racine d'un noyau cubique de Catmull-Rom (a = -0,5).
fn catmull_rom(x: f64) -> f64 {
    let x = x.abs();
    if x < 1.0 {
        1.5 * x * x * x - 2.5 * x * x + 1.0
    } else if x < 2.0 {
        -0.5 * x * x * x + 2.5 * x * x - 4.0 * x + 2.0
    } else {
        0.0
    }
}

/// Les pixels d'origine qui font le pixel d'arrivée `d` d'un axe, avec leurs poids (somme 1) : le premier pixel
/// d'origine et les poids. Réduction : tente (triangle) sur `1/s` pixels, comme `Raster::scaled` ; agrandissement
/// ou même échelle : Catmull-Rom. Les poids nuls aux extrémités sont retirés (un décalage entier : un seul pixel).
fn poids(d: i64, s: f64, t: f64) -> (i64, Vec<f32>) {
    let u = (d as f64 + 0.5 - t) / s;
    let (r, cubique) = if s < 1.0 { (1.0 / s, false) } else { (2.0, true) };
    let premier = (u - r - 0.5).ceil() as i64;
    let dernier = (u + r - 0.5).floor() as i64;
    let mut w: Vec<f64> = (premier..=dernier)
        .map(|i| {
            let x = i as f64 + 0.5 - u;
            if cubique {
                catmull_rom(x)
            } else {
                (1.0 - x.abs() / r).max(0.0)
            }
        })
        .collect();
    let mut debut = premier;
    while w.len() > 1 && w[0].abs() < 1e-9 {
        w.remove(0);
        debut += 1;
    }
    while w.len() > 1 && w[w.len() - 1].abs() < 1e-9 {
        w.pop();
    }
    let somme: f64 = w.iter().sum();
    if somme.abs() < 1e-12 {
        return (debut, vec![1.0]);
    }
    (debut, w.iter().map(|v| (v / somme) as f32).collect())
}

/// Le bloc `bw`×`bh` de l'origine dont le coin est (`x0`, `y0`), en pixels prémultipliés flottants ; rien si aucune
/// tuile n'y touche.
fn rassembler(src: &Raster, x0: i64, y0: i64, bw: usize, bh: usize) -> Option<Vec<[f32; 4]>> {
    let t = TILE as i64;
    let (tx0, tx1) = (x0.div_euclid(t), (x0 + bw as i64 - 1).div_euclid(t));
    let (ty0, ty1) = (y0.div_euclid(t), (y0 + bh as i64 - 1).div_euclid(t));
    let mut bloc: Option<Vec<[f32; 4]>> = None;
    for ty in ty0..=ty1 {
        for tx in tx0..=tx1 {
            let Some(tuile) = src.tile_at(tx as i32, ty as i32) else { continue };
            let b = bloc.get_or_insert_with(|| vec![[0.0; 4]; bw * bh]);
            let (ox, oy) = (tx * t, ty * t);
            let (ax0, ax1) = (x0.max(ox), (x0 + bw as i64).min(ox + t));
            let (ay0, ay1) = (y0.max(oy), (y0 + bh as i64).min(oy + t));
            for y in ay0..ay1 {
                for x in ax0..ax1 {
                    let c = tuile[(y - oy) as usize * TILE + (x - ox) as usize];
                    if c.a() == 0 {
                        continue;
                    }
                    b[(y - y0) as usize * bw + (x - x0) as usize] = [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32];
                }
            }
        }
    }
    bloc
}

/// La tuile d'arrivée (`tx`, `ty`) : les pixels d'origine, filtrés ; rien si elle reste vide.
fn tuile_d_arrivee(src: &Raster, s: f64, t: Vec2, tx: i32, ty: i32) -> Option<Tile> {
    let n = TILE as i64;
    let cols: Vec<(i64, Vec<f32>)> = (0..n).map(|k| poids(tx as i64 * n + k, s, t.x as f64)).collect();
    let lignes: Vec<(i64, Vec<f32>)> = (0..n).map(|k| poids(ty as i64 * n + k, s, t.y as f64)).collect();
    let x0 = cols.iter().map(|c| c.0).min()?;
    let x1 = cols.iter().map(|c| c.0 + c.1.len() as i64).max()?;
    let y0 = lignes.iter().map(|c| c.0).min()?;
    let y1 = lignes.iter().map(|c| c.0 + c.1.len() as i64).max()?;
    let (bw, bh) = ((x1 - x0) as usize, (y1 - y0) as usize);
    let bloc = rassembler(src, x0, y0, bw, bh)?;
    // Passe horizontale : bh lignes de 64 colonnes.
    let mut milieu = vec![[0.0f32; 4]; bh * TILE];
    for r in 0..bh {
        let ligne = &bloc[r * bw..(r + 1) * bw];
        for (k, (premier, w)) in cols.iter().enumerate() {
            let mut acc = [0.0f32; 4];
            let depart = (*premier - x0) as usize;
            for (j, wj) in w.iter().enumerate() {
                let p = &ligne[depart + j];
                acc[0] += p[0] * wj;
                acc[1] += p[1] * wj;
                acc[2] += p[2] * wj;
                acc[3] += p[3] * wj;
            }
            milieu[r * TILE + k] = acc;
        }
    }
    // Passe verticale.
    let mut pixels = [Color32::TRANSPARENT; TILE_PIXELS];
    let mut visible = false;
    for (j, (premier, w)) in lignes.iter().enumerate() {
        let depart = (*premier - y0) as usize;
        for k in 0..TILE {
            let mut acc = [0.0f32; 4];
            for (i, wi) in w.iter().enumerate() {
                let p = &milieu[(depart + i) * TILE + k];
                acc[0] += p[0] * wi;
                acc[1] += p[1] * wi;
                acc[2] += p[2] * wi;
                acc[3] += p[3] * wi;
            }
            let a = acc[3].round().clamp(0.0, 255.0);
            if a < 1.0 {
                continue;
            }
            let c = |v: f32| v.round().clamp(0.0, a) as u8;
            pixels[j * TILE + k] = Color32::from_rgba_premultiplied(c(acc[0]), c(acc[1]), c(acc[2]), a as u8);
            visible = true;
        }
    }
    visible.then(|| Tile::new(pixels))
}

/// Les tuiles d'arrivée que peut toucher le dessin d'origine : pour chaque tuile dessinée, son rectangle transformé,
/// élargi de la portée du filtre. Limité à la toile (`raster::WORLD`).
fn tuiles_d_arrivee(src: &Raster, s: f64, t: Vec2) -> BTreeSet<(i32, i32)> {
    let n = TILE as f64;
    let portee = if s < 1.0 { 1.5 } else { 2.0 * s + 1.0 };
    let limite = raster::WORLD / TILE as i64;
    let mut out = BTreeSet::new();
    let mut ajoute = |tx: i32, ty: i32| {
        let (x0, y0) = (tx as f64 * n, ty as f64 * n);
        let lo = (s * x0 + t.x as f64 - portee, s * y0 + t.y as f64 - portee);
        let hi = (s * (x0 + n) + t.x as f64 + portee, s * (y0 + n) + t.y as f64 + portee);
        let (a, b) = ((lo.0 / n).floor() as i64, (lo.1 / n).floor() as i64);
        let (c, d) = (((hi.0 - 1e-6) / n).floor() as i64, ((hi.1 - 1e-6) / n).floor() as i64);
        for ty in b.max(-limite)..=d.min(limite) {
            for tx in a.max(-limite)..=c.min(limite) {
                out.insert((tx as i32, ty as i32));
            }
        }
    };
    for (idx, tuile) in src.tiles.iter().enumerate() {
        if tuile.is_some() {
            ajoute((idx % src.tiles_x) as i32, (idx / src.tiles_x) as i32);
        }
    }
    for &(tx, ty) in src.outer.keys() {
        ajoute(tx, ty);
    }
    out
}

/// Le dessin `src` transformé par `p' = s·p + t`, dans une feuille `w`×`h`. Toute la toile est traitée : la feuille et
/// le hors-feuille, en entrée comme en sortie. `fils` : combien de fils au plus.
pub fn reechantillonne(src: &Raster, s: f32, t: Vec2, w: usize, h: usize, fils: usize) -> Raster {
    let mut out = Raster::new(w, h);
    let cibles: Vec<(i32, i32)> = tuiles_d_arrivee(src, s as f64, t).into_iter().collect();
    if cibles.is_empty() {
        return out;
    }
    let faire = |lot: &[(i32, i32)]| -> Vec<((i32, i32), Tile)> {
        lot.iter().filter_map(|&(tx, ty)| Some(((tx, ty), tuile_d_arrivee(src, s as f64, t, tx, ty)?))).collect()
    };
    let resultats: Vec<((i32, i32), Tile)> = if fils <= 1 || cibles.len() < 48 {
        faire(&cibles)
    } else {
        let part = cibles.len().div_ceil(fils.min(cibles.len()));
        std::thread::scope(|sc| {
            let taches: Vec<_> = cibles.chunks(part).map(|lot| sc.spawn(move || faire(lot))).collect();
            taches.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
        })
    };
    for ((tx, ty), tuile) in resultats {
        let id = out.id_of(tx, ty);
        out.replace_slot(id, Some(tuile));
    }
    out
}

// ------------------------------------------------------------------------------------------------ dessins

/// Un dessin adapté à la feuille d'arrivée `w`×`h`. C'est une nouvelle version du dessin : sa version suit celle du
/// dessin d'origine. Repartir de 0 laissait au même numéro la version d'un dessin relu (0) quand le document entier est
/// mis à l'échelle sur place (`Document::resize_frame`) : l'enregistrement le croyait déjà écrit et gardait l'ancien PNG,
/// que la réouverture refusait (taille différente de celle du projet).
pub fn adapte_dessin(cel: &Cel, ad: &Adaptation, w: usize, h: usize, fils: usize) -> Cel {
    let mut c = adapte_dessin_contenu(cel, ad, w, h, fils);
    c.version = cel.version + 1;
    c
}

fn adapte_dessin_contenu(cel: &Cel, ad: &Adaptation, w: usize, h: usize, fils: usize) -> Cel {
    let fond = cel.fond.as_ref().map(|f| {
        Box::new(Strate {
            peinture: adapte_dessin(&f.peinture, ad, w, h, fils),
            coupe: reechantillonne(&f.coupe, ad.s, ad.t, w, h, fils),
        })
    });
    match &cel.vector {
        Some(v) => {
            let m = ad.affine();
            let mut d = v.clone();
            for e in &mut d.elements {
                *e = e.transformed(&m);
            }
            let mut c = Cel { raster: Raster::new(w, h), vector: Some(d), version: 0, fond };
            c.rerender();
            c
        }
        None => Cel { raster: reechantillonne(&cel.raster, ad.s, ad.t, w, h, fils), vector: None, version: 0, fond },
    }
}

/// Tous les dessins d'une copie, sur des fils (un dessin à la fois par fil ; les fils restants servent à chaque
/// dessin).
pub fn adapte_dessins(cels: &HashMap<CelId, Cel>, ad: &Adaptation, progression: &Progression) -> HashMap<CelId, Cel> {
    let (w, h) = (ad.dst.feuille[0], ad.dst.feuille[1]);
    progression.total.fetch_add(cels.len(), Ordering::Relaxed);
    if !ad.spatiale() {
        progression.fait.fetch_add(cels.len(), Ordering::Relaxed);
        return cels.clone();
    }
    let liste: Vec<(&CelId, &Cel)> = cels.iter().collect();
    let dispo = fils_disponibles();
    let ouvriers = dispo.min(liste.len()).max(1);
    let par_dessin = (dispo / ouvriers).max(1);
    let prochain = AtomicUsize::new(0);
    let faire = || -> Vec<(CelId, Cel)> {
        let mut fait = Vec::new();
        loop {
            let i = prochain.fetch_add(1, Ordering::Relaxed);
            let Some((id, cel)) = liste.get(i) else { break };
            fait.push((**id, adapte_dessin(cel, ad, w, h, par_dessin)));
            progression.avance();
        }
        fait
    };
    if ouvriers <= 1 {
        return faire().into_iter().collect();
    }
    std::thread::scope(|sc| {
        let taches: Vec<_> = (0..ouvriers).map(|_| sc.spawn(faire)).collect();
        taches.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    })
}

// ------------------------------------------------------------------------------------------------ caméra, effets, calques

/// Une caméra adaptée : position par la similitude, avancée au rapport des cadres ; zoom et angles inchangés.
pub fn adapte_camera(c: &Camera, ad: &Adaptation) -> Camera {
    let p = ad.point(pos2(c.x, c.y));
    Camera { x: p.x, y: p.y, dolly: c.dolly * ad.rapport_avancee(), ..*c }
}

/// Une clé de caméra adaptée dans l'espace (son image, elle, se refait avec le temps : `retemps_reglages_plan`).
pub fn adapte_cle_camera(k: &CameraKey, ad: &Adaptation) -> CameraKey {
    CameraKey { cam: adapte_camera(&k.cam, ad), ..*k }
}

/// Le paramètre `k` de l'effet se compte en pixels de l'image (`" px"`, `" px/img"`).
fn en_pixels(kind: EffectKind, k: usize) -> bool {
    kind.params().get(k).is_some_and(|p| p.unit.trim_start().starts_with("px"))
}

/// Les paramètres d'un effet, mis à l'échelle (valeurs et clés) : ceux en pixels × `s`, la densité de trous du trait
/// sec (par 100 px de trait) ÷ `s`, dans les bornes de l'effet.
pub fn adapte_valeurs(kind: EffectKind, values: &mut [f32], s: f32) {
    for (k, v) in values.iter_mut().enumerate() {
        let Some(p) = kind.params().get(k) else { continue };
        let facteur = if en_pixels(kind, k) {
            s
        } else if kind == EffectKind::Dry && k == crate::fxmatiere::DRY_FREQ {
            1.0 / s
        } else {
            continue;
        };
        let mut x = *v * facteur;
        if kind.param_style(k) == ParamStyle::Integer {
            x = x.round();
        }
        *v = x.clamp(p.min, p.max);
    }
}

/// Un effet adapté : ses valeurs, celles de ses clés ; les images de ses clés si les durées sont gardées en secondes.
pub fn adapte_effet(e: &mut Effect, ad: &Adaptation) {
    if ad.s != 1.0 {
        adapte_valeurs(e.kind, &mut e.values, ad.s);
        for k in &mut e.keys {
            adapte_valeurs(e.kind, &mut k.values, ad.s);
        }
    }
}

/// Une valeur en pixels entiers (débord, fermeture des trous) × `s`, au moins 1 si elle n'était pas nulle.
fn pixels_u8(v: u8, s: f32) -> u8 {
    if v == 0 {
        return 0;
    }
    ((v as f32 * s).round() as i64).clamp(1, 255) as u8
}

/// Ce qui est en pixels sur un calque (hors dessins et hors images) : pose, clés de position, effets, fond sous le
/// trait, fermeture des trous des calques de couleur (peinture et remplissage au griffonnage).
pub fn adapte_calque(l: &mut Layer, ad: &Adaptation) {
    if !ad.spatiale() {
        return;
    }
    let s = ad.s;
    l.transform.x *= s;
    l.transform.y *= s;
    for k in &mut l.pos_keys {
        k.t.x *= s;
        k.t.y *= s;
    }
    // Le chemin de la trajectoire est en pixels de la feuille : il suit. (Un chemin tracé dans l'espace est en mètres, dans son
    // plan : il ne change pas, le cadre mesure toujours autant de mètres. Partie 149.)
    if let Some(t) = l.trajectoire.as_mut().filter(|t| t.sur_la_feuille()) {
        t.transformer(|p| [p[0] * s, p[1] * s]);
    }
    for e in &mut l.fx {
        adapte_effet(e, ad);
    }
    if let Some(f) = &mut l.fond {
        f.debord = pixels_u8(f.debord, s);
        f.trous = pixels_u8(f.trous, s);
    }
    if let Some(c) = &mut l.couleur {
        c.trous = pixels_u8(c.trous, s);
    }
}

/// Les réglages d'un calque qui comptent en images, à une autre cadence (`r` : une image d'avant → une image d'après) :
/// rôles, clés d'effets et de position, début de la vidéo. Les images du calque (`frames`) se refont à part. Aussi
/// utilisé par « Changer la cadence » (`tweenui.rs`) : une seule règle d'arrondi.
pub fn retemps_reglages_calque(l: &mut Layer, r: &dyn Fn(usize) -> usize) {
    l.roles = l.roles.iter().map(|(f, role)| (r(*f), *role)).collect();
    for e in &mut l.fx {
        for k in &mut e.keys {
            k.frame = r(k.frame);
        }
        e.keys.dedup_by_key(|k| k.frame);
    }
    for k in &mut l.pos_keys {
        k.frame = r(k.frame);
    }
    l.pos_keys.dedup_by_key(|k| k.frame);
    for k in &mut l.opacity_keys {
        k.frame = r(k.frame);
    }
    l.opacity_keys.dedup_by_key(|k| k.frame);
    if let Some(v) = &mut l.video {
        let n = |f: u64| r(f as usize) as i64;
        v.start = if v.start >= 0 { n(v.start as u64) } else { -n(v.start.unsigned_abs()) };
    }
}

/// Les réglages d'un plan qui comptent en images, à une autre cadence (`r`, et `k` = cadence d'arrivée / cadence
/// d'origine pour les notes dessinées sur la timeline) : durée, clés de caméra, marques d'image, repères.
pub fn retemps_reglages_plan(p: &mut Panel, r: &dyn Fn(usize) -> usize, k: f32) {
    p.length = r(p.length).max(1);
    for key in &mut p.camera {
        key.frame = r(key.frame);
    }
    p.camera.dedup_by_key(|key| key.frame);
    // (Une marque d'image reste après la première image du plan, et une seule par image.)
    for m in &mut p.marks {
        m.frame = r(m.frame).max(1);
    }
    p.marks.dedup_by_key(|m| m.frame);
    let c = &mut p.cues;
    c.bookmarks = c.bookmarks.iter().map(|f| r(*f)).collect();
    c.bookmarks.dedup();
    c.in_point = c.in_point.map(r);
    c.out_point = c.out_point.map(r);
    for n in &mut c.notes {
        for pt in &mut n.points {
            pt[0] *= k;
        }
    }
}

/// Un calque à une autre cadence : ses images (dessins tenus, jamais d'intervalle inventé) et ses réglages en images.
pub fn retemps_calque(l: &mut Layer, ad: &Adaptation) {
    if !ad.retemps() {
        return;
    }
    if !l.frames.is_empty() {
        l.frames = retemps_images(&l.frames, ad);
    }
    retemps_reglages_calque(l, &|f| ad.image(f));
}

/// Des images tenues sur une autre durée : l'image j tombe au temps j·n/m des images d'origine.
pub fn retemps_images(frames: &[Option<CelId>], ad: &Adaptation) -> Vec<Option<CelId>> {
    if !ad.retemps() || frames.is_empty() {
        return frames.to_vec();
    }
    crate::tween::resample(frames, ad.duree_en_images(frames.len()), false)
        .into_iter()
        .map(|slot| match slot {
            crate::tween::Slot::Keep(c) => c,
            // (Sans interpolation, `resample` ne rend que des dessins tenus.)
            crate::tween::Slot::Tween(a, _, _) => Some(a),
        })
        .collect()
}

/// Un plan adapté : caméra, calques ; à une autre cadence, aussi sa durée, ses marques et ses repères.
/// Les dessins sont adaptés à part (`adapte_dessins`).
pub fn adapte_plan(p: &mut Panel, ad: &Adaptation) {
    for l in &mut p.layers {
        adapte_calque(l, ad);
        retemps_calque(l, ad);
    }
    for k in &mut p.camera {
        *k = adapte_cle_camera(k, ad);
    }
    if ad.retemps() {
        retemps_reglages_plan(p, &|f| ad.image(f), ad.dst.cadence as f32 / ad.src.cadence.max(1) as f32);
    }
}

// ------------------------------------------------------------------------------------------------ les copies

use crate::app::LayerClip;
use crate::camkeys::{CamClip, CamClipKey};
use crate::document::PanelClip;
use crate::fxcopie::FxStackClip;
use crate::fxkeys::{FxClip, FxClipKey};
use crate::images::ImageClip;
use crate::montagekeys::KeyClip;
use crate::multisel::{ClipWhat, MultiClip};

/// Des plans copiés, adaptés. Le résultat est à la taille de la feuille d'arrivée.
pub fn adapte_plans(clip: &PanelClip, ad: &Adaptation, progression: &Progression) -> PanelClip {
    let mut panels = clip.panels.clone();
    for p in &mut panels {
        adapte_plan(p, ad);
    }
    PanelClip { panels, cels: adapte_dessins(&clip.cels, ad, progression), origine: ad.dst }
}

/// Un calque copié (avec son dossier et ses calques de couleur), adapté.
pub fn adapte_calque_copie(clip: &LayerClip, ad: &Adaptation, progression: &Progression) -> LayerClip {
    let mut out = clip.clone();
    for l in std::iter::once(&mut out.layer).chain(out.children.iter_mut()).chain(out.colours.iter_mut()) {
        adapte_calque(l, ad);
        retemps_calque(l, ad);
    }
    out.panel_len = ad.duree_en_images(out.panel_len);
    out.cels = adapte_dessins(&clip.cels, ad, progression);
    out.origine = ad.dst;
    out
}

/// Des images copiées (une rangée par calque), adaptées.
pub fn adapte_images_copiees(clip: &ImageClip, ad: &Adaptation, progression: &Progression) -> ImageClip {
    let mut out = clip.clone();
    if ad.retemps() {
        out.rows = out.rows.iter().map(|r| retemps_images(r, ad)).collect();
        for roles in &mut out.roles {
            for (f, _) in roles.iter_mut() {
                *f = ad.image(*f);
            }
        }
    }
    out.cels = adapte_dessins(&clip.cels, ad, progression);
    out.origine = ad.dst;
    out
}

/// Des clés copiées (dessins du montage), adaptées.
pub fn adapte_cles_copiees(clip: &KeyClip, ad: &Adaptation, progression: &Progression) -> KeyClip {
    let cels: HashMap<CelId, Cel> = clip.keys.iter().enumerate().map(|(i, k)| (i as CelId, k.cel.clone())).collect();
    let mut adaptes = adapte_dessins(&cels, ad, progression);
    let keys = clip
        .keys
        .iter()
        .enumerate()
        .map(|(i, k)| crate::montagekeys::ClipKey {
            offset: ad.image(k.offset),
            len: ad.duree_en_images(k.len),
            cel: adaptes.remove(&(i as CelId)).unwrap_or_else(|| k.cel.clone()),
            kind: k.kind,
            name: k.name.clone(),
            rank: k.rank,
            // (Les dessins de couleur ne passent pas d'une feuille à l'autre : voir `montagekeys.rs`, partie 103.)
            couleurs: Vec::new(),
        })
        .collect();
    KeyClip { keys, origine: ad.dst }
}

/// Une zone bitmap copiée, adaptée : elle garde sa place dans le cadre. Rien si elle sort de la feuille d'arrivée.
pub fn adapte_zone(f: &Floating, ad: &Adaptation) -> Option<Floating> {
    if !ad.spatiale() {
        return Some(f.clone());
    }
    // (La zone dans une toile de la taille de l'origine : seules ses tuiles existent.)
    let (sw, sh) = (ad.src.feuille[0].max(f.x0 + f.w), ad.src.feuille[1].max(f.y0 + f.h));
    let mut r = Raster::new(sw, sh);
    for y in 0..f.h {
        for x in 0..f.w {
            let c = f.pixels[y * f.w + x];
            if c.a() != 0 {
                let i = r.tile_index_of(f.x0 + x, f.y0 + y);
                r.tile_mut(i)[((f.y0 + y) % TILE) * TILE + (f.x0 + x) % TILE] = c;
            }
        }
    }
    let (w, h) = (ad.dst.feuille[0], ad.dst.feuille[1]);
    let out = reechantillonne(&r, ad.s, ad.t, w, h, 1);
    let (a, b) = (ad.point(pos2(f.x0 as f32, f.y0 as f32)), ad.point(pos2((f.x0 + f.w) as f32, (f.y0 + f.h) as f32)));
    let (x0, y0) = (a.x.floor().max(0.0) as usize, a.y.floor().max(0.0) as usize);
    let (x1, y1) = ((b.x.ceil().max(0.0) as usize + 1).min(w), (b.y.ceil().max(0.0) as usize + 1).min(h));
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let (nw, nh) = (x1 - x0, y1 - y0);
    let mut pixels = vec![Color32::TRANSPARENT; nw * nh];
    for y in 0..nh {
        for x in 0..nw {
            pixels[y * nw + x] = out.get(x0 + x, y0 + y);
        }
    }
    let zone = Floating { x0, y0, w: nw, h: nh, pixels };
    (!zone.is_empty()).then_some(zone)
}

/// Des éléments vectoriels copiés, adaptés.
pub fn adapte_elements(elements: &[Element], ad: &Adaptation) -> Vec<Element> {
    if !ad.spatiale() {
        return elements.to_vec();
    }
    let m = ad.affine();
    elements.iter().map(|e| e.transformed(&m)).collect()
}

/// Une sélection copiée sur plusieurs calques, adaptée.
pub fn adapte_multi(clip: &MultiClip, ad: &Adaptation) -> MultiClip {
    let items = clip
        .items
        .iter()
        .filter_map(|(layer, what)| {
            let what = match what {
                ClipWhat::Vector(e) => ClipWhat::Vector(adapte_elements(e, ad)),
                ClipWhat::Pixels(f) => ClipWhat::Pixels(adapte_zone(f, ad)?),
            };
            Some((*layer, what))
        })
        .collect();
    MultiClip { items }
}

/// Des clés de caméra copiées, adaptées.
pub fn adapte_cles_camera(clip: &CamClip, ad: &Adaptation) -> CamClip {
    CamClip { keys: clip.keys.iter().map(|k| CamClipKey { offset: ad.image(k.offset), key: adapte_cle_camera(&k.key, ad) }).collect() }
}

/// Des clés d'effet copiées, adaptées.
pub fn adapte_cles_effet(clip: &FxClip, ad: &Adaptation) -> FxClip {
    let keys = clip
        .keys
        .iter()
        .map(|k| {
            let mut values = k.values.clone();
            adapte_valeurs(clip.kind, &mut values, ad.s);
            FxClipKey { offset: ad.image(k.offset), values }
        })
        .collect();
    FxClip { kind: clip.kind, keys }
}

/// Des effets copiés d'un calque, adaptés.
pub fn adapte_pile_effets(clip: &FxStackClip, ad: &Adaptation) -> FxStackClip {
    let mut effects = clip.effects.clone();
    for e in &mut effects {
        adapte_effet(e, ad);
        if ad.retemps() {
            for k in &mut e.keys {
                k.frame = ad.image(k.frame);
            }
            e.keys.dedup_by_key(|k| k.frame);
        }
    }
    FxStackClip { effects }
}
