//! Calque de couleur : la couleur **sous le trait** (choix d'Anthony, 2026-10-20 : « le trait / la
//! couleur », à la façon de Toon Boom — le comportement exact de Storyboard Pro ou de Harmony est
//! à vérifier, SHOT n'en copie rien).
//!
//! Un calque de couleur est lié à un calque de trait, dont il est le sous-sol : posé juste dessous
//! dans la pile, il suit ses images, et l'on y colorie sans jamais toucher au trait.
//!
//! - **Modèle** : un calque bitmap ordinaire (`Layer::couleur` en plus). Rien dans la composition,
//!   les vignettes, l'export, le PDF, le montage ou l'export Storyboard Pro n'a changé : pour eux, c'est
//!   un calque bitmap de plus. Le trait peut être vectoriel : la couleur, elle, est en pixels (un
//!   aplat n'a pas besoin d'être modifiable point par point, et le pot de peinture lit les pixels
//!   rendus du trait, `Cel::raster`, que le calque soit vectoriel ou bitmap).
//! - **Images** : celles du calque de couleur sont celles du calque de trait, pour chaque image
//!   (`refleter`). Un dessin du trait a son dessin de couleur (`CalqueCouleur::dessins`) : déplacer,
//!   allonger, supprimer une clé du trait emmène sa couleur ; une nouvelle clé du trait reçoit une
//!   couleur vide. Le miroir est refait à chaque image d'écran et à chaque étape d'annulation
//!   (`App::sync_couleurs`) : jamais une étape de plus pour l'utilisateur.
//! - **Pile** : `reparer` remet le calque de couleur juste sous son trait (dans son dossier), et
//!   oublie le lien d'un calque dont le trait a disparu (il devient un calque bitmap ordinaire).
//! - **Peindre** : le pinceau et le crayon peignent comme sur un calque bitmap ; par défaut
//!   (`zone`), un trait de pinceau reste dans la zone fermée par le trait où il commence (et passe
//!   un peu sous lui). Le pot de peinture remplit la zone fermée par le trait où l'on clique : les
//!   trous du trait jusqu'à `2 × trous` pixels sont fermés (`Murs`, même idée que le fond sous le
//!   trait, `fond::zone`, et que le remplissage au griffonnage, `griffonnage::fill`).
//! - **Griffonner** (`griffonnage.rs`, « On fusionne », Anthony, 2026-10-21 : ce qu'était le calque CTG) : en mode
//!   « Griffonner » (`griffonner`), le pinceau, le crayon et la gomme travaillent sur un dessin de griffonnages
//!   (`griffonnages` : dessin de peinture → dessin de griffonnage, à part du dessin de peinture) ; chaque zone fermée par
//!   le trait (et les `sources` supplémentaires) que touche un griffonnage est remplie de sa couleur, sous le trait,
//!   et le remplissage se refait tout seul, en arrière-plan, quand le trait ou un griffonnage change. Il se pose **sous**
//!   la peinture libre (qui reste un dessin ordinaire, sans latence). Une même couche mêle donc peinture libre et
//!   zones griffonnées.
//! - **Calque libre** (`trait_id == 0`) : un calque de couleur sans calque de trait propre, aux images à lui, dont les zones
//!   sont fermées par ses `sources`. C'est ce que devient un ancien calque CTG que son trait ne suit pas (`migrer_ctg`).
//! - **Fond sous le trait** (`fond.rs`) : l'autre moyen d'avoir de la couleur sous un trait, dans le
//!   même calque (une strate sous le trait, déduite de lui). Les deux se complètent et se cumulent
//!   (la strate du calque de trait est sous le calque de couleur, qui est sous le trait).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

use eframe::egui::Color32;

use crate::document::{CelId, Document, Layer, LayerKind};
use crate::fond::{edt, LINE_ALPHA, UNDER};
use crate::raster::{Raster, Tile, TILE};

/// Fermeture des trous du pot de peinture d'un calque de couleur neuf (pixels de chaque côté du trou).
pub const TROUS: u8 = 4;

/// Le lien d'un calque de couleur avec son calque de trait, et ses réglages.
#[derive(Clone, Debug, PartialEq)]
pub struct CalqueCouleur {
    /// Numéro (de séance) du calque de trait que ce calque colore ; 0 : calque libre, sans calque de trait propre (ses
    /// images sont à lui).
    pub trait_id: u64,
    /// À chaque dessin du trait, le dessin de couleur qui lui répond.
    pub dessins: BTreeMap<CelId, CelId>,
    /// Les trous du trait jusqu'à `2 × trous` pixels sont fermés (pot de peinture, pinceau dans la zone, remplissage
    /// au griffonnage).
    pub trous: u8,
    /// Un trait de pinceau ou de crayon reste dans la zone fermée où il commence.
    pub zone: bool,
    /// Mode « Griffonner » : le pinceau, le crayon, la gomme et le pot (un tampon) posent des griffonnages, qui
    /// remplissent les zones fermées par le trait (`griffonnage.rs`).
    pub griffonner: bool,
    /// Le calque a des griffonnages : son remplissage est calculé et montré (posé la première fois qu'on griffonne).
    pub remplissage: bool,
    /// À chaque dessin de peinture du calque, son dessin de griffonnages (un dessin du projet, à part).
    pub griffonnages: BTreeMap<CelId, CelId>,
    /// Sources supplémentaires : d'autres calques de dessin dont les traits ferment aussi les zones du remplissage.
    pub sources: Vec<u64>,
}

impl CalqueCouleur {
    pub fn new(trait_id: u64) -> Self {
        CalqueCouleur {
            trait_id,
            dessins: BTreeMap::new(),
            trous: TROUS,
            zone: true,
            griffonner: false,
            remplissage: false,
            griffonnages: BTreeMap::new(),
            sources: Vec::new(),
        }
    }

    /// Après une copie du calque (et de son trait) : le trait copié, les dessins copiés (de peinture et de griffonnages),
    /// les sources copiées avec lui.
    pub fn relink(&mut self, ids: &HashMap<u64, u64>, cels: &HashMap<CelId, CelId>) {
        self.trait_id = ids.get(&self.trait_id).copied().unwrap_or(self.trait_id);
        let map = |c: &CelId| cels.get(c).copied().unwrap_or(*c);
        self.dessins = self.dessins.iter().map(|(t, c)| (map(t), map(c))).collect();
        self.griffonnages = self.griffonnages.iter().map(|(p, g)| (map(p), map(g))).collect();
        for s in &mut self.sources {
            *s = ids.get(s).copied().unwrap_or(*s);
        }
    }

    /// Le calque suit les images d'un calque de trait (sinon il est libre).
    pub fn est_lie(&self) -> bool {
        self.trait_id != 0
    }

    /// Les dessins de griffonnages de ce calque, pour les enregistrer, les copier, les emporter.
    pub fn dessins_griffonnages(&self) -> impl Iterator<Item = CelId> + '_ {
        self.griffonnages.values().copied()
    }
}

/// Le calque peut recevoir un calque de couleur : un calque de dessin ordinaire (ni dossier, ni
/// calque d'effet, ni vidéo, ni lui-même un calque de couleur, ni calque de toute la scène).
pub fn peut_porter(l: &Layer) -> bool {
    !l.folder && l.effet.is_none() && l.video.is_none() && l.couleur.is_none() && l.scene_wide.is_none()
}

fn peut_etre_couleur(l: &Layer) -> bool {
    !l.folder && l.effet.is_none() && l.video.is_none() && l.scene_wide.is_none()
}

/// Tous les dessins d'un calque : ceux de ses images, et les griffonnages (s'il est un calque de couleur). Ce qu'il faut
/// emporter quand on copie le calque ou son plan.
pub fn dessins_du_calque(l: &Layer) -> impl Iterator<Item = CelId> + '_ {
    l.frames.iter().flatten().copied().chain(l.couleur.iter().flat_map(|c| c.griffonnages.values().copied()))
}

/// Les dessins de griffonnages que portent les calques de couleur de `layers` (à enregistrer et à emporter avec les
/// dessins de peinture qui leur répondent).
pub fn griffonnages_de<'a>(layers: &'a [Layer]) -> impl Iterator<Item = (CelId, CelId)> + 'a {
    layers.iter().filter_map(|l| l.couleur.as_ref()).flat_map(|c| c.griffonnages.iter().map(|(p, g)| (*p, *g)))
}

/// Rangs des calques de couleur du calque de trait `li`, du plus bas au plus haut.
pub fn couleurs_de(layers: &[Layer], li: usize) -> Vec<usize> {
    let Some(id) = layers.get(li).map(|l| l.id) else { return Vec::new() };
    (0..layers.len()).filter(|&k| layers[k].couleur.as_ref().is_some_and(|c| c.trait_id == id)).collect()
}

/// Rang du calque de trait du calque de couleur `ci` (rien pour un calque libre).
pub fn trait_de(layers: &[Layer], ci: usize) -> Option<usize> {
    let id = layers.get(ci)?.couleur.as_ref()?.trait_id;
    if id == 0 {
        return None;
    }
    layers.iter().position(|l| l.id == id)
}

/// `lis` et, pour chaque calque de trait qui y est, ses calques de couleur (rangs triés, sans doublon) :
/// ce qui part ensemble à la suppression.
pub fn avec_couleurs(layers: &[Layer], lis: &[usize]) -> Vec<usize> {
    let mut out: Vec<usize> = lis.iter().copied().filter(|&l| l < layers.len()).collect();
    for &li in lis {
        out.extend(couleurs_de(layers, li));
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Un calque de couleur neuf pour le calque de trait `line` (ses images viennent de `refleter`).
pub fn nouveau(line: &Layer, name: String) -> Layer {
    let mut l = Layer::new(name, LayerKind::Bitmap);
    l.couleur = Some(CalqueCouleur::new(line.id));
    l.parent = line.parent;
    l.transform = line.transform;
    l.pos_keys = line.pos_keys.clone();
    l.depth = line.depth;
    l.pre = line.pre;
    l.post = line.post;
    l
}

/// Remet de l'ordre dans la pile : un lien dont le trait n'existe plus (ou n'est plus un calque de
/// dessin ordinaire) est oublié — le calque devient un calque bitmap ordinaire, ou, s'il a des griffonnages, un calque de
/// couleur libre, qui les garde —, et chaque calque de couleur lié est posé juste sous son trait, dans le même
/// dossier. Vrai si quelque chose a changé.
pub fn reparer(layers: &mut Vec<Layer>) -> bool {
    if !layers.iter().any(|l| l.couleur.is_some()) {
        return false;
    }
    let mut changed = false;
    let lines: HashSet<u64> = layers.iter().filter(|l| peut_porter(l)).map(|l| l.id).collect();
    for l in layers.iter_mut() {
        if l.couleur.is_none() {
            continue;
        }
        let (id, possible) = (l.id, peut_etre_couleur(l));
        let Some(c) = &mut l.couleur else { continue };
        let garde = c.remplissage || !c.griffonnages.is_empty();
        if !possible {
            l.couleur = None;
            changed = true;
        } else if c.trait_id != 0 && (c.trait_id == id || !lines.contains(&c.trait_id)) {
            if garde {
                // Le trait a disparu : le calque garde ses griffonnages et leurs sources, ses images deviennent les siennes.
                c.trait_id = 0;
                c.dessins.clear();
            } else {
                l.couleur = None;
            }
            changed = true;
        } else if l.kind != LayerKind::Bitmap {
            l.kind = LayerKind::Bitmap;
            changed = true;
        }
    }
    let before: Vec<(u64, Option<u64>)> = layers.iter().map(|l| (l.id, l.parent)).collect();
    let mut by_line: HashMap<u64, Vec<Layer>> = HashMap::new();
    let mut rest: Vec<Layer> = Vec::with_capacity(layers.len());
    for l in layers.drain(..) {
        // (Un calque libre garde sa place.)
        match l.couleur.as_ref().map(|c| c.trait_id).filter(|&t| t != 0) {
            Some(t) => by_line.entry(t).or_default().push(l),
            None => rest.push(l),
        }
    }
    for l in rest {
        if let Some(mut colours) = by_line.remove(&l.id) {
            for c in &mut colours {
                c.parent = l.parent;
            }
            layers.extend(colours);
        }
        layers.push(l);
    }
    changed || before != layers.iter().map(|l| (l.id, l.parent)).collect::<Vec<_>>()
}

/// Accorde les images des calques de couleur du plan `pi` à celles de leur trait : à chaque image où
/// le trait montre un dessin, le calque de couleur montre le dessin de couleur qui lui répond (un
/// dessin vide neuf s'il n'en a pas encore) ; là où le trait n'a rien, la couleur non plus. Le pré
/// et le post-comportement, la transformation et la profondeur du trait sont ceux de la couleur.
/// Vrai si quelque chose a changé.
pub fn refleter(doc: &mut Document, pi: usize) -> bool {
    let Some(panel) = doc.panels.get(pi) else { return false };
    let jobs: Vec<(usize, usize)> = (0..panel.layers.len())
        .filter_map(|ci| {
            let c = panel.layers[ci].couleur.as_ref()?;
            let li = panel.layers.iter().position(|l| l.id == c.trait_id)?;
            Some((ci, li))
        })
        .collect();
    let mut changed = false;
    for (ci, li) in jobs {
        let (frames_of_line, pre, post, transform, depth, pos_keys) = {
            let l = &doc.panels[pi].layers[li];
            (l.frames.clone(), l.pre, l.post, l.transform, l.depth, l.pos_keys.clone())
        };
        let mut map = doc.panels[pi].layers[ci].couleur.as_ref().map(|c| c.dessins.clone()).unwrap_or_default();
        let old_frames = doc.panels[pi].layers[ci].frames.clone();
        let mut map_changed = false;
        // Les dessins de couleur déjà pris par un dessin du trait qui est là.
        let present: HashSet<CelId> = frames_of_line.iter().flatten().copied().collect();
        let mut claimed: HashSet<CelId> =
            map.iter().filter(|(t, c)| present.contains(*t) && doc.cels.contains_key(*c)).map(|(_, c)| *c).collect();
        let mut frames: Vec<Option<CelId>> = Vec::with_capacity(frames_of_line.len());
        for (f, trait_frame) in frames_of_line.iter().enumerate() {
            frames.push(match trait_frame {
                None => None,
                Some(trait_cel) => Some(match map.get(trait_cel) {
                    Some(&c) if doc.cels.contains_key(&c) => c,
                    _ => {
                        // Un dessin du trait qui n'a pas encore sa couleur. Si une commande a travaillé
                        // sur les deux calques à la fois (dupliquer, coller des images de plusieurs
                        // calques…), la couleur déjà posée à cette image, que personne ne réclame, est la
                        // sienne ; sinon une couleur vide.
                        let adopted = old_frames.get(f).copied().flatten().filter(|c| !claimed.contains(c) && doc.cels.contains_key(c));
                        let c = adopted.unwrap_or_else(|| doc.new_cel(LayerKind::Bitmap));
                        claimed.insert(c);
                        map.insert(*trait_cel, c);
                        map_changed = true;
                        c
                    }
                }),
            });
        }
        let layer = &mut doc.panels[pi].layers[ci];
        if layer.frames != frames {
            layer.frames = frames;
            changed = true;
        }
        if map_changed {
            if let Some(c) = layer.couleur.as_mut() {
                c.dessins = map;
            }
            changed = true;
        }
        if (layer.pre, layer.post, layer.transform, layer.depth) != (pre, post, transform, depth) || layer.pos_keys != pos_keys {
            (layer.pre, layer.post, layer.transform, layer.depth) = (pre, post, transform, depth);
            layer.pos_keys = pos_keys;
            changed = true;
        }
    }
    changed
}

/// Un dessin de trait copié (« nouveau dessin = copie », cassure automatique…) : ses calques de
/// couleur copient aussi la couleur du dessin d'origine, pour que la copie garde sa couleur.
pub fn noter_copie(doc: &mut Document, pi: usize, li: usize, source: CelId, copie: CelId) {
    let Some(panel) = doc.panels.get(pi) else { return };
    // Le calque est lui-même un calque de couleur (libre, aux dessins à lui) : la copie de son dessin garde ses griffonnages.
    let own = panel.layers.get(li).and_then(|l| l.couleur.as_ref()).and_then(|c| c.griffonnages.get(&source).copied());
    if let Some(g) = own {
        let d = doc.duplicate_cel(g);
        // (Son remplissage à l'écran, en attendant le sien : celui du dessin dont il est la copie.)
        crate::fxcache::heriter(g, d);
        if let Some(c) = doc.panels[pi].layers[li].couleur.as_mut() {
            c.griffonnages.insert(copie, d);
        }
    }
    let colours = couleurs_de(&doc.panels[pi].layers, li);
    for ci in colours {
        let Some(&old) = doc.panels[pi].layers[ci].couleur.as_ref().and_then(|c| c.dessins.get(&source)) else { continue };
        let fresh = doc.duplicate_cel(old);
        // Ses griffonnages aussi : la copie du dessin garde son remplissage.
        let source_scribbles = doc.panels[pi].layers[ci].couleur.as_ref().and_then(|c| c.griffonnages.get(&old).copied());
        let scribbles = source_scribbles.map(|g| doc.duplicate_cel(g));
        // Le remplissage de la clé neuve est, en attendant le sien, celui de la clé dont elle est la copie : l'aplat ne disparaît pas
        // le temps qu'il se calcule (`fxcache::heriter`).
        if let (Some(g), Some(d)) = (source_scribbles, scribbles) {
            crate::fxcache::heriter(g, d);
        }
        if let Some(c) = doc.panels[pi].layers[ci].couleur.as_mut() {
            c.dessins.insert(copie, fresh);
            if let Some(g) = scribbles {
                c.griffonnages.insert(fresh, g);
            }
        }
    }
}

// ------------------------------------------------------------------------------------ zones

/// Le trait d'un dessin, prêt à servir de frontière : ses pixels pleins, et ses murs (le trait épaissi
/// de `trous` pixels, qui ferme les trous). Calculé une fois par version du dessin (`Cache`).
pub struct Murs {
    /// Taille du dessin du trait.
    pub largeur: usize,
    pub hauteur: usize,
    /// La boîte calculée (le rectangle du trait, agrandi de la fermeture) : le dehors continue au-delà.
    bx: usize,
    by: usize,
    bw: usize,
    bh: usize,
    mur: Vec<bool>,
    plein: Vec<bool>,
    trous: usize,
}

/// Une zone fermée par le trait : les pixels de la boîte qui en font partie, et si elle fuit vers le
/// dehors (dans ce cas elle comprend aussi tout le dessin hors de la boîte).
pub struct Zone {
    pub bx: usize,
    pub by: usize,
    pub bw: usize,
    pub bh: usize,
    pub cases: Vec<bool>,
    pub dehors: bool,
}

impl Murs {
    /// Les murs du trait `trait_`, ses trous jusqu'à `2 × trous` pixels fermés. Rien s'il n'y a pas de
    /// trait.
    pub fn new(trait_: &Raster, trous: u8) -> Option<Murs> {
        let (w, h) = (trait_.width, trait_.height);
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
        for (idx, t) in trait_.tiles.iter().enumerate() {
            let Some(t) = t else { continue };
            let (ox, oy, tw, th) = trait_.tile_bounds(idx);
            for ty in 0..th {
                for tx in 0..tw {
                    if t[ty * TILE + tx].a() >= LINE_ALPHA {
                        x0 = x0.min(ox + tx);
                        y0 = y0.min(oy + ty);
                        x1 = x1.max(ox + tx);
                        y1 = y1.max(oy + ty);
                    }
                }
            }
        }
        if x0 == usize::MAX || w == 0 || h == 0 {
            return None;
        }
        let pad = trous as usize + UNDER + 3;
        let (bx, by) = (x0.saturating_sub(pad), y0.saturating_sub(pad));
        let (bx1, by1) = ((x1 + pad).min(w - 1), (y1 + pad).min(h - 1));
        let (bw, bh) = (bx1 - bx + 1, by1 - by + 1);
        let mut plein = vec![false; bw * bh];
        for y in 0..bh {
            for x in 0..bw {
                plein[y * bw + x] = trait_.get(bx + x, by + y).a() >= LINE_ALPHA;
            }
        }
        let mur = if trous == 0 {
            plein.clone()
        } else {
            let t2 = (trous as f64) * (trous as f64);
            edt(&plein, bw, bh).iter().map(|&d| d <= t2).collect()
        };
        Some(Murs { largeur: w, hauteur: h, bx, by, bw, bh, mur, plein, trous: trous as usize })
    }

    /// La zone fermée où tombe le point (`x`, `y`) du dessin : les pixels qu'on atteint sans traverser
    /// de mur, jusqu'au trait, plus `UNDER` pixels sous lui. Un clic tout près du trait, ou sur un trait
    /// fin, prend le pixel libre le plus proche (à `trous + UNDER + 1` pixels au plus). Rien si aucun n'est
    /// assez près (le cœur d'un aplat noir), ou si le point est hors du dessin.
    pub fn zone(&self, x: usize, y: usize) -> Option<Zone> {
        if x >= self.largeur || y >= self.hauteur {
            return None;
        }
        let (bw, bh) = (self.bw, self.bh);
        let inside = |gx: usize, gy: usize| gx >= self.bx && gy >= self.by && gx < self.bx + bw && gy < self.by + bh;
        let mut cases = vec![false; bw * bh];
        let mut stack: Vec<usize> = Vec::new();
        if inside(x, y) {
            let (cx, cy) = (x - self.bx, y - self.by);
            let r = self.trous + UNDER + 1;
            let start = (cy.saturating_sub(r)..=(cy + r).min(bh - 1))
                .flat_map(|py| (cx.saturating_sub(r)..=(cx + r).min(bw - 1)).map(move |px| (px, py)))
                .filter(|&(px, py)| !self.mur[py * bw + px])
                .min_by_key(|&(px, py)| px.abs_diff(cx).pow(2) + py.abs_diff(cy).pow(2))?;
            stack.push(start.1 * bw + start.0);
        } else {
            // Le dehors : on part du bord de la boîte.
            for bx in 0..bw {
                stack.extend([bx, (bh - 1) * bw + bx]);
            }
            for by in 0..bh {
                stack.extend([by * bw, by * bw + bw - 1]);
            }
            stack.retain(|&p| !self.mur[p]);
        }
        for &p in &stack {
            cases[p] = true;
        }
        while let Some(p) = stack.pop() {
            let (px, py) = (p % bw, p / bw);
            let near = [(px > 0).then(|| p - 1), (px + 1 < bw).then(|| p + 1), (py > 0).then(|| p - bw), (py + 1 < bh).then(|| p + bw)];
            for q in near.into_iter().flatten() {
                if !self.mur[q] && !cases[q] {
                    cases[q] = true;
                    stack.push(q);
                }
            }
        }
        // La zone fuit : elle atteint le bord de la boîte, ou on a cliqué hors d'elle.
        let on_border = |p: usize| {
            let (px, py) = (p % bw, p / bw);
            px == 0 || py == 0 || px + 1 == bw || py + 1 == bh
        };
        let dehors = !inside(x, y) || (0..bw * bh).any(|p| cases[p] && on_border(p));
        // Elle regagne la bande de fermeture jusqu'au trait, puis passe un peu sous lui.
        grow(&mut cases, bw, bh, self.trous, |p| !self.plein[p]);
        grow(&mut cases, bw, bh, UNDER, |p| self.plein[p]);
        Some(Zone { bx: self.bx, by: self.by, bw, bh, cases, dehors })
    }
}

/// `times` fois : chaque pixel où `can` le permet, voisin d'un pixel de la zone, y entre.
fn grow(cases: &mut [bool], w: usize, h: usize, times: usize, can: impl Fn(usize) -> bool) {
    for _ in 0..times {
        let before = cases.to_vec();
        let mut changed = false;
        for p in 0..w * h {
            if before[p] || !can(p) {
                continue;
            }
            let (px, py) = (p % w, p / w);
            let near = [(px > 0).then(|| p - 1), (px + 1 < w).then(|| p + 1), (py > 0).then(|| p - w), (py + 1 < h).then(|| p + w)];
            if near.into_iter().flatten().any(|q| before[q]) {
                cases[p] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

impl Zone {
    /// Le pixel (`x`, `y`) du dessin est dans la zone.
    pub fn contient(&self, x: usize, y: usize) -> bool {
        if x >= self.bx && y >= self.by && x < self.bx + self.bw && y < self.by + self.bh {
            self.cases[(y - self.by) * self.bw + (x - self.bx)]
        } else {
            self.dehors
        }
    }

    /// Le rectangle (`ox`, `oy`, `tw`, `th`) ne touche pas la boîte : il est tout dans la zone si elle
    /// fuit, tout dehors sinon.
    fn hors_boite(&self, (ox, oy, tw, th): (usize, usize, usize, usize)) -> Option<bool> {
        let apart = ox >= self.bx + self.bw || oy >= self.by + self.bh || ox + tw <= self.bx || oy + th <= self.by;
        apart.then_some(self.dehors)
    }

    /// La zone en dessin blanc plein (pour un pochoir) de `largeur × hauteur` pixels.
    pub fn pochoir(&self, largeur: usize, hauteur: usize) -> Raster {
        let mut out = Raster::new(largeur, hauteur);
        for idx in 0..out.tiles.len() {
            let bounds = out.tile_bounds(idx);
            let (ox, oy, tw, th) = bounds;
            out.tiles[idx] = match self.hors_boite(bounds) {
                Some(false) => None,
                Some(true) => Some(Tile::new([Color32::WHITE; crate::raster::TILE_PIXELS])),
                None => {
                    let mut tile: Option<Tile> = None;
                    for ty in 0..th {
                        for tx in 0..tw {
                            if self.contient(ox + tx, oy + ty) {
                                tile.get_or_insert_with(crate::raster::empty_tile)[ty * TILE + tx] = Color32::WHITE;
                            }
                        }
                    }
                    tile
                }
            };
        }
        out
    }

    /// Pose `couleur` (ou efface, si rien) sur les pixels de la zone ; renvoie les tuiles d'avant
    /// (annulation).
    pub fn peindre(&self, r: &mut Raster, couleur: Option<Color32>) -> Vec<(usize, Option<Tile>)> {
        let mut old = Vec::new();
        let ink = couleur.unwrap_or(Color32::TRANSPARENT);
        for idx in 0..r.tiles.len() {
            if couleur.is_none() && r.tiles[idx].is_none() {
                continue;
            }
            let bounds = r.tile_bounds(idx);
            let (ox, oy, tw, th) = bounds;
            match self.hors_boite(bounds) {
                Some(false) => continue,
                Some(true) => {
                    old.push((idx, r.tiles[idx].clone()));
                    let tile = r.tile_mut(idx);
                    for ty in 0..th {
                        tile[ty * TILE..ty * TILE + tw].fill(ink);
                    }
                }
                None => {
                    let touched = (0..th).any(|ty| (0..tw).any(|tx| self.contient(ox + tx, oy + ty)));
                    if !touched {
                        continue;
                    }
                    old.push((idx, r.tiles[idx].clone()));
                    let tile = r.tile_mut(idx);
                    for ty in 0..th {
                        for tx in 0..tw {
                            if self.contient(ox + tx, oy + ty) {
                                tile[ty * TILE + tx] = ink;
                            }
                        }
                    }
                }
            }
        }
        old
    }
}

// ------------------------------------------------------------------------------------ cache

/// Ce qui identifie les murs d'un trait : son dessin, sa version, la fermeture des trous.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cle {
    pub cel: CelId,
    pub version: u64,
    pub trous: u8,
}

/// Les murs du dernier trait servi, gardés tant que le dessin du trait ne change pas (le
/// premier calcul coûte, les suivants — pot de peinture, début d'un trait dans la zone — sont
/// immédiats). `prechauffer` les calcule sur un autre fil dès qu'on arrive sur le calque de couleur.
#[derive(Default)]
pub struct Cache {
    pret: Option<(Cle, Option<Arc<Murs>>)>,
    en_cours: Option<(Cle, Receiver<Option<Murs>>)>,
}

impl Cache {
    /// Un résultat du fil de fond est arrivé.
    pub fn poll(&mut self) -> bool {
        let Some((cle, rx)) = self.en_cours.take() else { return false };
        match rx.try_recv() {
            Ok(m) => {
                self.pret = Some((cle, m.map(Arc::new)));
                true
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                self.en_cours = Some((cle, rx));
                false
            }
            Err(_) => false,
        }
    }

    /// Les murs du trait `trait_` (rien : pas de trait). Calculés tout de suite s'ils ne sont pas prêts.
    pub fn obtenir(&mut self, cle: Cle, trait_: &Raster) -> Option<Arc<Murs>> {
        self.poll();
        if let Some((k, m)) = &self.pret {
            if *k == cle {
                return m.clone();
            }
        }
        let m = Murs::new(trait_, cle.trous).map(Arc::new);
        self.pret = Some((cle, m.clone()));
        self.en_cours = None;
        m
    }

    /// Lance le calcul des murs sur un autre fil s'ils ne sont ni prêts ni en route.
    pub fn prechauffer(&mut self, cle: Cle, trait_: &Raster) {
        self.poll();
        if self.pret.as_ref().is_some_and(|(k, _)| *k == cle) || self.en_cours.as_ref().is_some_and(|(k, _)| *k == cle) {
            return;
        }
        let (tx, rx) = channel();
        let trait_ = trait_.clone();
        let trous = cle.trous;
        let spawned = std::thread::Builder::new().name("murs-couleur".into()).spawn(move || {
            let _ = tx.send(Murs::new(&trait_, trous));
        });
        if spawned.is_ok() {
            self.en_cours = Some((cle, rx));
        }
    }

    /// Les murs sont prêts pour `cle` (essais).
    pub fn est_pret(&self, cle: Cle) -> bool {
        self.pret.as_ref().is_some_and(|(k, _)| *k == cle)
    }

    /// Un calcul est en route (essais).
    pub fn calcule(&self) -> bool {
        self.en_cours.is_some()
    }
}
