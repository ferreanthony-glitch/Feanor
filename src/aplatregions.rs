//! Les régions de l'aplat et leur correspondance d'une clé à l'autre : le cœur de l'**aplat intelligent** (Anthony, 2026-10-28 :
//! « un aplat intelligent qui suit le trait et comprend la peinture à l'intérieur — un œil, une ombre… — réglable en redessinant,
//! sans IA »). Pas de modèle, pas d'apprentissage : de la géométrie, déterministe et rapide.
//!
//! ## Régions
//! Pour un trait, les **régions** sont les zones fermées que l'aplat sous le trait remplit : exactement le « dedans » de
//! `fond::silhouette` (mêmes murs, même fermeture des trous, même règle du bord de l'image), découpé en composantes connexes. Un
//! œil fermé dans un visage fermé donne deux régions (la peau, qui entoure l'œil, et l'œil) ; le dehors n'est pas une région. Les
//! poussières (moins de `AIRE_MIN` pixels) n'en sont pas. Chaque région connaît son aire, son centre, sa boîte, et sa **région
//! englobante** (celle qui l'entoure, si elle n'a qu'une seule voisine à travers le trait : un œil dans un visage).
//!
//! Le **territoire** d'une région est ce qu'elle colorie : ses pixels, plus la bande du trait qui l'entoure et le débord (les
//! pixels du trait sont à la région la plus proche). L'aplat affiché est la couleur du calque plus la peinture de la strate
//! (`Strate`), dans la forme : « la couleur d'une région » est donc **ce qui est peint dans son territoire** (sa couleur la plus
//! fréquente, si plus de la moitié de la région est peinte ; sinon la couleur du calque). Rien n'est ajouté au format du projet :
//! l'aplat intelligent ne fait qu'écrire de la peinture dans la strate, comme le pot de peinture.
//!
//! ## Correspondance (déterministe)
//! Pour retrouver, dans la clé « source » (une autre clé, ou la même avant une retouche), la région qui correspond à une région
//! de la clé « cible » :
//! 1. **Recalage global**, parmi quatre candidats : l'identité ; la boîte englobante de toutes les régions (translation et
//!    échelle, chaque axe) ; le centre de masse (translation) ; le centre de masse avec l'échelle `√(aire cible / aire source)`.
//!    On garde celui qui recouvre le plus de surface (les suivants doivent gagner d'au moins 1 % de la surface cible pour
//!    détrôner le précédent : en cas de quasi-égalité, l'identité gagne). Une retouche d'une clé n'essaie que l'identité (le
//!    dessin n'a pas bougé).
//! 2. **Recouvrement** : sous ce recalage, rapport (surface commune) / (surface réunie) de chaque couple (région source, région
//!    cible) ; couples pris du meilleur au moins bon, une région une seule fois, à partir de `SEUIL_RECOUVREMENT` (0,2).
//! 3. **Centre, aire, inclusion** pour les restantes : centre source recalé à moins de 0,75 × `√(plus grande aire)` du centre
//!    cible, aires dans un rapport de 1 à 3, et *inclusion* compatible (toutes deux sans région englobante, ou toutes deux avec, et
//!    leurs englobantes appariées entre elles quand elles le sont) ; du plus proche au plus loin.
//!
//! Les égalités se départagent par le rang des régions : le résultat ne dépend de rien d'autre que des deux dessins.
//!
//! ## Couleurs posées
//! Pour chaque région cible appariée à une région source, dans son territoire (là où l'aplat est visible, `silhouette` > 0) :
//! le pixel de peinture (et de coupe, gomme dans l'aplat) de la source à l'endroit recalé **est repris** s'il est dans le
//! territoire de la région source (les détails peints à la main, une ombre, un reflet, suivent donc) ; sinon (le pixel est
//! *gagné* par la région : le trait a bougé) il prend la **couleur de la région source**. Une région cible **sans
//! correspondance** : en suivant une autre clé, elle prend la couleur de sa région englobante *si celle-ci est appariée* (un œil
//! neuf dans un visage colorié, c'est sûr) ; sinon elle reste comme elle est (vide, donc à la couleur du calque). Une
//! région source qui disparaît (fondue dans une autre, ou ouverte vers le dehors) ne laisse rien : la région qui l'absorbe garde
//! sa couleur à elle.

use std::collections::{BTreeSet, HashMap, VecDeque};

use eframe::egui::Color32;

use crate::fond::{self, FondReglages};
use crate::raster::Raster;

/// Une composante de moins de pixels n'est pas une région (poussière du trait).
pub(crate) const AIRE_MIN: u32 = 6;
/// Rapport (surface commune) / (surface réunie) à partir duquel deux régions se correspondent.
pub(crate) const SEUIL_RECOUVREMENT: f32 = 0.2;
/// Une région est « peinte » quand cette part de ses pixels l'est.
const PART_PEINTE: f32 = 0.5;
/// Dehors, dans la propagation des voisinages.
const DEHORS: u32 = u32::MAX;

/// Une zone fermée par le trait.
#[derive(Clone, Debug)]
pub(crate) struct Region {
    /// Pixels de la région (sans le trait qui l'entoure).
    pub aire: u32,
    /// Centre de masse, en coordonnées du dessin.
    pub cx: f32,
    pub cy: f32,
    /// Boîte (bornes comprises), en coordonnées du dessin.
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
    /// La région qui l'entoure, s'il n'y en a qu'une et que la région n'ouvre pas sur le dehors (rang dans `Regions::regs`).
    pub parent: Option<usize>,
}

/// Les régions d'un dessin au trait.
#[derive(Clone, Debug)]
pub(crate) struct Regions {
    /// Rectangle de travail, en coordonnées du dessin.
    pub bx0: usize,
    pub by0: usize,
    pub bw: usize,
    pub bh: usize,
    /// Dans le rectangle : 0 hors région, sinon rang de la région + 1.
    pub comp: Vec<u32>,
    /// Le territoire (voir plus haut) : 0 hors territoire, sinon rang de la région + 1.
    pub terr: Vec<u32>,
    pub regs: Vec<Region>,
}

/// Les quatre voisins du pixel `p` d'une grille `bw` × `bh`.
fn voisins(p: usize, bw: usize, bh: usize) -> [Option<usize>; 4] {
    let (x, y) = (p % bw, p / bw);
    [(x > 0).then(|| p - 1), (x + 1 < bw).then(|| p + 1), (y > 0).then(|| p - bw), (y + 1 < bh).then(|| p + bw)]
}

/// Les régions du trait `line` pour les réglages `r` ; rien s'il n'y en a aucune (pas de trait, rien de fermé).
pub(crate) fn calculer(line: &Raster, r: &FondReglages) -> Option<Regions> {
    let g = fond::geometrie(line, r)?;
    let (bw, bh) = (g.bw, g.bh);
    let n = bw * bh;
    let libre = |p: usize| !g.wall[p] && !g.outside[p];
    // 1. Les composantes connexes de l'intérieur.
    let mut comp = vec![0u32; n];
    let mut vu = vec![false; n];
    let mut regs: Vec<Region> = Vec::new();
    let mut pile: Vec<usize> = Vec::new();
    let mut pixels: Vec<usize> = Vec::new();
    for depart in 0..n {
        if vu[depart] || !libre(depart) {
            continue;
        }
        pixels.clear();
        vu[depart] = true;
        pile.push(depart);
        while let Some(p) = pile.pop() {
            pixels.push(p);
            for q in voisins(p, bw, bh).into_iter().flatten() {
                if !vu[q] && libre(q) {
                    vu[q] = true;
                    pile.push(q);
                }
            }
        }
        if pixels.len() < AIRE_MIN as usize {
            continue;
        }
        let id = regs.len() as u32 + 1;
        let (mut sx, mut sy) = (0f64, 0f64);
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for &p in &pixels {
            comp[p] = id;
            let (x, y) = ((g.bx0 + p % bw) as i32, (g.by0 + p / bw) as i32);
            sx += x as f64 + 0.5;
            sy += y as f64 + 0.5;
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
        let a = pixels.len() as f64;
        regs.push(Region { aire: pixels.len() as u32, cx: (sx / a) as f32, cy: (sy / a) as f32, x0, y0, x1, y1, parent: None });
    }
    if regs.is_empty() {
        return None;
    }
    // 2. Les voisines à travers le trait : les murs sont partagés entre les composantes et le dehors, par le plus proche.
    let mut lab: Vec<u32> = comp.clone();
    let mut file: VecDeque<usize> = VecDeque::new();
    for p in 0..n {
        if g.outside[p] {
            lab[p] = DEHORS;
        }
        if lab[p] != 0 {
            file.push_back(p);
        }
    }
    while let Some(p) = file.pop_front() {
        let l = lab[p];
        for q in voisins(p, bw, bh).into_iter().flatten() {
            if lab[q] == 0 {
                lab[q] = l;
                file.push_back(q);
            }
        }
    }
    let mut voisines: Vec<BTreeSet<u32>> = vec![BTreeSet::new(); regs.len()];
    for p in 0..n {
        let (x, y) = (p % bw, p / bw);
        for q in [(x + 1 < bw).then(|| p + 1), (y + 1 < bh).then(|| p + bw)].into_iter().flatten() {
            let (a, b) = (lab[p], lab[q]);
            if a != b && a != 0 && b != 0 {
                if a != DEHORS {
                    voisines[a as usize - 1].insert(b);
                }
                if b != DEHORS {
                    voisines[b as usize - 1].insert(a);
                }
            }
        }
    }
    for (i, v) in voisines.iter().enumerate() {
        if v.len() == 1 {
            let l = *v.iter().next().unwrap_or(&DEHORS);
            if l != DEHORS {
                regs[i].parent = Some(l as usize - 1);
            }
        }
    }
    // 3. Le territoire : chaque région gagne, pixel par pixel, ce qui l'entoure (mur, trait, débord) jusqu'à `prof`.
    let prof = (r.trous as usize + r.debord as usize + 14).min(64);
    let mut terr = comp.clone();
    let mut front: Vec<usize> = (0..n).filter(|&p| terr[p] != 0).collect();
    for _ in 0..prof {
        let mut suite = Vec::new();
        for &p in &front {
            let l = terr[p];
            for q in voisins(p, bw, bh).into_iter().flatten() {
                if terr[q] == 0 {
                    terr[q] = l;
                    suite.push(q);
                }
            }
        }
        if suite.is_empty() {
            break;
        }
        front = suite;
    }
    Some(Regions { bx0: g.bx0, by0: g.by0, bw, bh, comp, terr, regs })
}

impl Regions {
    /// Rang, dans le rectangle de travail, du pixel (`x`, `y`) du dessin.
    fn rang(&self, x: i64, y: i64) -> Option<usize> {
        let (dx, dy) = (x - self.bx0 as i64, y - self.by0 as i64);
        (dx >= 0 && dy >= 0 && (dx as usize) < self.bw && (dy as usize) < self.bh).then(|| dy as usize * self.bw + dx as usize)
    }

    /// La région dont le pixel (`x`, `y`) du dessin fait partie (rang + 1), 0 sinon.
    pub(crate) fn region_en(&self, x: i64, y: i64) -> u32 {
        self.rang(x, y).map_or(0, |i| self.comp[i])
    }

    /// Le territoire où tombe le pixel (`x`, `y`) du dessin (rang + 1), 0 sinon.
    pub(crate) fn territoire_en(&self, x: i64, y: i64) -> u32 {
        self.rang(x, y).map_or(0, |i| self.terr[i])
    }

    fn aire_totale(&self) -> f32 {
        self.regs.iter().map(|r| r.aire as f32).sum()
    }

    /// Centre de masse de toutes les régions.
    fn centre_de_masse(&self) -> (f32, f32) {
        let a = self.aire_totale().max(1.0);
        (self.regs.iter().map(|r| r.cx * r.aire as f32).sum::<f32>() / a, self.regs.iter().map(|r| r.cy * r.aire as f32).sum::<f32>() / a)
    }

    /// Boîte de toutes les régions : (gauche, haut, droite, bas), bornes comprises.
    fn boite(&self) -> (f32, f32, f32, f32) {
        let x0 = self.regs.iter().map(|r| r.x0).min().unwrap_or(0) as f32;
        let y0 = self.regs.iter().map(|r| r.y0).min().unwrap_or(0) as f32;
        let x1 = self.regs.iter().map(|r| r.x1).max().unwrap_or(0) as f32 + 1.0;
        let y1 = self.regs.iter().map(|r| r.y1).max().unwrap_or(0) as f32 + 1.0;
        (x0, y0, x1, y1)
    }

    /// La couleur de chaque région d'après la peinture `peinture` (la strate de l'aplat) : sa couleur la plus fréquente si au
    /// moins la moitié de ses pixels sont peints (opaques à 200 sur 255 au moins), sinon rien (la couleur du calque).
    pub(crate) fn couleurs(&self, peinture: &Raster) -> Vec<Option<Color32>> {
        let n = self.regs.len();
        let mut histo: Vec<HashMap<u32, u32>> = vec![HashMap::new(); n];
        let mut peints = vec![0u32; n];
        let mut dernier: (usize, u32, u32) = (usize::MAX, 0, 0);
        for (p, &l) in self.comp.iter().enumerate() {
            if l == 0 {
                continue;
            }
            let c = peinture.get(self.bx0 + p % self.bw, self.by0 + p / self.bw);
            if c.a() < 200 {
                continue;
            }
            let (i, k) = (l as usize - 1, u32::from_le_bytes(c.to_array()));
            peints[i] += 1;
            if (i, k) == (dernier.0, dernier.1) {
                dernier.2 += 1;
            } else {
                if dernier.2 > 0 {
                    *histo[dernier.0].entry(dernier.1).or_default() += dernier.2;
                }
                dernier = (i, k, 1);
            }
        }
        if dernier.2 > 0 {
            *histo[dernier.0].entry(dernier.1).or_default() += dernier.2;
        }
        (0..n)
            .map(|i| {
                if (peints[i] as f32) < PART_PEINTE * self.regs[i].aire as f32 {
                    return None;
                }
                // Le plus fréquent ; à égalité, la plus petite valeur (déterministe).
                let (k, _) = histo[i].iter().map(|(&k, &c)| (k, c)).max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))?;
                let [r, g, b, a] = k.to_le_bytes();
                Some(Color32::from_rgba_premultiplied(r, g, b, a))
            })
            .collect()
    }
}

/// Le recalage global de la source vers la cible : `x' = x × sx + tx`, `y' = y × sy + ty`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Recalage {
    pub sx: f32,
    pub sy: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Recalage {
    pub(crate) const IDENTITE: Recalage = Recalage { sx: 1.0, sy: 1.0, tx: 0.0, ty: 0.0 };

    fn vers_cible(&self, x: f32, y: f32) -> (f32, f32) {
        (x * self.sx + self.tx, y * self.sy + self.ty)
    }

    fn vers_source(&self, x: f32, y: f32) -> (f32, f32) {
        ((x - self.tx) / self.sx, (y - self.ty) / self.sy)
    }

    fn surface(&self) -> f32 {
        (self.sx * self.sy).abs()
    }
}

/// Les quatre recalages à essayer d'une source vers une cible, dans l'ordre (l'identité d'abord).
fn candidats(src: &Regions, dst: &Regions) -> Vec<Recalage> {
    let mut out = vec![Recalage::IDENTITE];
    let (s0, s1, s2, s3) = src.boite();
    let (d0, d1, d2, d3) = dst.boite();
    let (sw, sh, dw, dh) = ((s2 - s0).max(1.0), (s3 - s1).max(1.0), (d2 - d0).max(1.0), (d3 - d1).max(1.0));
    let (sx, sy) = ((dw / sw).clamp(0.5, 2.0), (dh / sh).clamp(0.5, 2.0));
    let ((scx, scy), (dcx, dcy)) = (((s0 + s2) / 2.0, (s1 + s3) / 2.0), ((d0 + d2) / 2.0, (d1 + d3) / 2.0));
    out.push(Recalage { sx, sy, tx: dcx - scx * sx, ty: dcy - scy * sy });
    let ((mx, my), (nx, ny)) = (src.centre_de_masse(), dst.centre_de_masse());
    out.push(Recalage { sx: 1.0, sy: 1.0, tx: nx - mx, ty: ny - my });
    let s = (dst.aire_totale() / src.aire_totale().max(1.0)).sqrt().clamp(0.5, 2.0);
    out.push(Recalage { sx: s, sy: s, tx: nx - mx * s, ty: ny - my * s });
    out
}

/// Surface commune (en pixels de la cible) de chaque couple (région source + 1, région cible + 1) sous le recalage `t`.
fn recouvrements(src: &Regions, dst: &Regions, t: Recalage) -> HashMap<(u32, u32), u32> {
    let mut out: HashMap<(u32, u32), u32> = HashMap::new();
    let mut dernier = ((0u32, 0u32), 0u32);
    for (p, &b) in dst.comp.iter().enumerate() {
        if b == 0 {
            continue;
        }
        let (gx, gy) = ((dst.bx0 + p % dst.bw) as f32 + 0.5, (dst.by0 + p / dst.bw) as f32 + 0.5);
        let (fx, fy) = t.vers_source(gx, gy);
        let a = src.region_en(fx.floor() as i64, fy.floor() as i64);
        if a == 0 {
            continue;
        }
        if dernier.0 == (a, b) {
            dernier.1 += 1;
        } else {
            if dernier.1 > 0 {
                *out.entry(dernier.0).or_default() += dernier.1;
            }
            dernier = ((a, b), 1);
        }
    }
    if dernier.1 > 0 {
        *out.entry(dernier.0).or_default() += dernier.1;
    }
    out
}

/// Ce que la correspondance a trouvé : pour chaque région cible, sa région source.
#[derive(Clone, Debug)]
pub(crate) struct Appariement {
    pub recalage: Recalage,
    /// Région cible → région source (rangs).
    pub de_cible: Vec<Option<usize>>,
    /// Région source → région cible (rangs).
    pub de_source: Vec<Option<usize>>,
    /// Surface cible couverte par les couples du recouvrement (pour choisir le recalage).
    score: u64,
}

/// Premier temps : le recouvrement, du meilleur couple au moins bon.
fn apparier_par_recouvrement(src: &Regions, dst: &Regions, t: Recalage) -> Appariement {
    let comptes = recouvrements(src, dst, t);
    let surface = t.surface();
    let mut couples: Vec<(u32, u32, u32, u32)> = Vec::new();
    for (&(a, b), &commun) in &comptes {
        let at = src.regs[a as usize - 1].aire as f32 * surface;
        let bt = dst.regs[b as usize - 1].aire as f32;
        let union = at + bt - commun as f32;
        if union <= 0.0 {
            continue;
        }
        let iou = commun as f32 / union;
        if iou >= SEUIL_RECOUVREMENT {
            couples.push(((iou * 1_000_000.0) as u32, a, b, commun));
        }
    }
    // (Les égalités : le rang des régions.)
    couples.sort_by(|x, y| y.0.cmp(&x.0).then(x.1.cmp(&y.1)).then(x.2.cmp(&y.2)));
    let mut ap = Appariement { recalage: t, de_cible: vec![None; dst.regs.len()], de_source: vec![None; src.regs.len()], score: 0 };
    for (_, a, b, commun) in couples {
        let (ia, ib) = (a as usize - 1, b as usize - 1);
        if ap.de_source[ia].is_none() && ap.de_cible[ib].is_none() {
            ap.de_source[ia] = Some(ib);
            ap.de_cible[ib] = Some(ia);
            ap.score += commun as u64;
        }
    }
    ap
}

/// Deuxième temps : les régions restantes se retrouvent par leur centre, leur aire et leur inclusion.
fn apparier_par_centre(src: &Regions, dst: &Regions, ap: &mut Appariement) {
    let t = ap.recalage;
    let surface = t.surface();
    let mut couples: Vec<(u32, usize, usize)> = Vec::new();
    for (ia, a) in src.regs.iter().enumerate() {
        if ap.de_source[ia].is_some() {
            continue;
        }
        let (ax, ay) = t.vers_cible(a.cx, a.cy);
        let at = a.aire as f32 * surface;
        for (ib, b) in dst.regs.iter().enumerate() {
            if ap.de_cible[ib].is_some() {
                continue;
            }
            let bt = b.aire as f32;
            let rapport = at / bt.max(1.0);
            if !(1.0 / 3.0..=3.0).contains(&rapport) {
                continue;
            }
            let d = ((ax - b.cx).powi(2) + (ay - b.cy).powi(2)).sqrt();
            if d <= 0.75 * at.max(bt).sqrt() {
                couples.push(((d * 1000.0) as u32, ia, ib));
            }
        }
    }
    couples.sort();
    for (_, ia, ib) in couples {
        if ap.de_source[ia].is_some() || ap.de_cible[ib].is_some() {
            continue;
        }
        // L'inclusion : toutes deux dans une région, ou toutes deux libres ; leurs englobantes, appariées entre elles.
        let (pa, pb) = (src.regs[ia].parent, dst.regs[ib].parent);
        let compatible = match (pa, pb) {
            (None, None) => true,
            (Some(pa), Some(pb)) => ap.de_source[pa].is_none_or(|m| m == pb) && ap.de_cible[pb].is_none_or(|m| m == pa),
            _ => false,
        };
        if compatible {
            ap.de_source[ia] = Some(ib);
            ap.de_cible[ib] = Some(ia);
        }
    }
}

/// La correspondance des régions de `dst` avec celles de `src`. `retouche` : la même clé avant et après un trait, le dessin n'a pas
/// bougé, seul l'identité est essayée.
pub(crate) fn apparier(src: &Regions, dst: &Regions, retouche: bool) -> Appariement {
    let tous = if retouche { vec![Recalage::IDENTITE] } else { candidats(src, dst) };
    let marge = (dst.aire_totale() * 0.01) as u64;
    let mut meilleur: Option<Appariement> = None;
    for t in tous {
        let ap = apparier_par_recouvrement(src, dst, t);
        if meilleur.as_ref().is_none_or(|m| ap.score > m.score + marge) {
            meilleur = Some(ap);
        }
    }
    let mut ap = meilleur.unwrap_or_else(|| apparier_par_recouvrement(src, dst, Recalage::IDENTITE));
    apparier_par_centre(src, dst, &mut ap);
    ap
}

/// Un dessin et son aplat : le trait, la peinture de la strate, sa coupe.
pub(crate) struct Entree<'a> {
    pub trait_: &'a Raster,
    pub peinture: &'a Raster,
    pub coupe: &'a Raster,
}

/// La source préparée : ses régions et leurs couleurs.
pub(crate) struct Preparee {
    pub regions: Regions,
    pub couleurs: Vec<Option<Color32>>,
}

impl Preparee {
    /// Au moins une région a sa couleur propre (peinte).
    pub(crate) fn coloriee(&self) -> bool {
        self.couleurs.iter().any(Option::is_some)
    }
}

/// Les régions de `src` et leurs couleurs ; rien si son trait ne ferme aucune zone.
pub(crate) fn preparer(src: &Entree, r: &FondReglages) -> Option<Preparee> {
    let regions = calculer(src.trait_, r)?;
    let couleurs = regions.couleurs(src.peinture);
    Some(Preparee { regions, couleurs })
}

/// Ce qu'une mise en couleur a fait (pour la barre d'état et les essais).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Bilan {
    pub regions_cible: usize,
    /// Régions cibles appariées à une région source.
    pub appariees: usize,
    /// Régions cibles sans correspondance qui ont pris la couleur de leur région englobante.
    pub englobees: usize,
    /// Régions cibles sans correspondance laissées comme elles sont.
    pub laissees: usize,
}

/// Pose sur la cible les couleurs de la source, région par région (voir le début du fichier). Rend la peinture et la coupe de la
/// cible refaites, et le bilan ; rien si la cible n'a aucune région. `retouche` : la source est la même clé avant un trait.
pub(crate) fn colorier(src: &Entree, prep: &Preparee, dst: &Entree, r: &FondReglages, retouche: bool) -> Option<(Raster, Raster, Bilan)> {
    let rd = calculer(dst.trait_, r)?;
    let rs = &prep.regions;
    let ap = apparier(rs, &rd, retouche);
    // Pour chaque région cible : (région source dont elle prend la couleur, à plat ?).
    let mut bilan = Bilan { regions_cible: rd.regs.len(), ..Default::default() };
    let eff: Vec<Option<(usize, bool)>> = (0..rd.regs.len())
        .map(|ib| match ap.de_cible[ib] {
            Some(ia) => {
                bilan.appariees += 1;
                Some((ia, false))
            }
            None => {
                // Une région neuve dans une région appariée : sa couleur, sans risque. En retouche, elle garde ce qu'elle a.
                let englobee = (!retouche).then(|| rd.regs[ib].parent.and_then(|p| ap.de_cible[p])).flatten();
                match englobee {
                    Some(ia) => {
                        bilan.englobees += 1;
                        Some((ia, true))
                    }
                    None => {
                        bilan.laissees += 1;
                        None
                    }
                }
            }
        })
        .collect();
    let cover = fond::silhouette(dst.trait_, r);
    let w = dst.trait_.width;
    let mut peinture = dst.peinture.clone();
    let mut coupe = dst.coupe.clone();
    let poser = |raster: &mut Raster, x: usize, y: usize, v: Color32| {
        if raster.get(x, y) != v {
            let i = raster.tile_index_of(x, y);
            raster.tile_mut(i)[(y % crate::raster::TILE) * crate::raster::TILE + x % crate::raster::TILE] = v;
        }
    };
    let t = ap.recalage;
    for (p, &b) in rd.terr.iter().enumerate() {
        if b == 0 {
            continue;
        }
        let (gx, gy) = (rd.bx0 + p % rd.bw, rd.by0 + p / rd.bw);
        if cover[gy * w + gx] == 0 {
            continue;
        }
        let Some((ia, plat)) = eff[b as usize - 1] else { continue };
        let couleur = prep.couleurs[ia].unwrap_or(Color32::TRANSPARENT);
        let (np, nc) = if plat {
            (couleur, Color32::TRANSPARENT)
        } else {
            let (fx, fy) = t.vers_source((gx as f32) + 0.5, (gy as f32) + 0.5);
            let (sx, sy) = (fx.floor() as i64, fy.floor() as i64);
            // Repris : un pixel de l'intérieur de la région source (peint ou non : un détail, un trou), ou de son territoire quand il y est
            // peint (la bande du trait, sous le trait). Sinon, le pixel est gagné par la région : la couleur de la région.
            let ici = (rs.region_en(sx, sy), rs.territoire_en(sx, sy));
            let (pp, pc) = if ici.0 > 0 || ici.1 > 0 { (src.peinture.get(sx as usize, sy as usize), src.coupe.get(sx as usize, sy as usize)) } else { (Color32::TRANSPARENT, Color32::TRANSPARENT) };
            if ici.0 == ia as u32 + 1 || (ici.1 == ia as u32 + 1 && pp.a() > 0) {
                (pp, pc)
            } else {
                (couleur, Color32::TRANSPARENT)
            }
        };
        poser(&mut peinture, gx, gy, np);
        poser(&mut coupe, gx, gy, nc);
    }
    Some((peinture, coupe, bilan))
}

/// `preparer` puis `colorier` : pour les essais et les appels simples. Rien si la source ne ferme aucune zone ou la cible non plus.
#[allow(dead_code)]
pub(crate) fn colorier_comme(src: &Entree, dst: &Entree, r: &FondReglages, retouche: bool) -> Option<(Raster, Raster, Bilan)> {
    let prep = preparer(src, r)?;
    colorier(src, &prep, dst, r, retouche)
}
