//! Remplissage au griffonnage d'un calque de couleur (`calquecouleur.rs`). C'est ce qu'était le calque CTG de
//! TVPaint 12 (textes 011085106–204, 111020100–107 ; principe confirmé par Anthony le 2026-10-16), fondu dans le
//! calque de couleur le 2026-10-21 (« On fusionne », Anthony) : il n'y a plus qu'un calque de couleur, qui se
//! peint à la main (comme avant) **et** qui remplit des zones d'un coup de griffonnage.
//!
//! - **Griffonner** : sur un calque de couleur en mode « Griffonner » (`CalqueCouleur::griffonner`), le
//!   pinceau, le crayon et la gomme travaillent sur un dessin à part, celui des griffonnages
//!   (`CalqueCouleur::griffonnages` : dessin de peinture → dessin de griffonnage ; un vrai dessin du projet, enregistré
//!   comme les autres). On y pose des couleurs (les « graffitis ») ; chaque zone fermée par les traits des sources,
//!   touchée par un graffiti, est remplie de sa couleur, sous les traits.
//! - **Sources** : le calque de trait auquel le calque de couleur est lié, et les « sources supplémentaires »
//!   (`CalqueCouleur::sources`, d'autres calques dont les traits ferment aussi les zones). Un calque de couleur
//!   « libre » (sans calque de trait, issu d'un ancien calque CTG) n'a que ces sources.
//! - **Un élément du plan de plus** : le remplissage est un dessin dérivé (`fx::shown`), calculé à partir des
//!   griffonnages et des traits, et posé **sous** le dessin de peinture du calque (comme le fond sous le trait, `fond.rs`).
//!   La peinture libre reste donc un dessin ordinaire : zéro latence quand on peint dessus, même sur un calque qui
//!   remplit. Ordre, de bas en haut : remplissage, peinture libre, [calque de trait].
//! - **Vivant** : retoucher le trait ou un griffonnage change la version d'un des dessins que lit le remplissage ; il
//!   se recalcule alors **en arrière-plan** (`fxcache.rs`) et l'écran garde le remplissage d'avant en attendant le
//!   nouveau. L'export et les essais le calculent tout de suite (exact).
//!
//! Remplissage : un trait est un pixel des sources d'opacité au moins `LINE_ALPHA` ; chaque zone
//! (pixels voisins hors trait) touchée par des graffitis prend la couleur la plus présente parmi
//! eux ; puis la couleur glisse sous les traits voisins (`UNDER` pixels), pour ne pas laisser de
//! liseré entre le trait et la couleur.
//!
//! Ce fichier garde aussi la lecture des anciens projets : un calque CTG (format 35 et moins) devient un calque de
//! couleur au chargement (`migrer_ctg`), avec la même image.

use std::collections::{BTreeMap, HashMap, HashSet};

use eframe::egui::Color32;

use crate::calquecouleur::CalqueCouleur;
use crate::document::{Cel, CelId, Document, Layer, LayerKind, Panel};
use crate::raster::{Raster, TILE};

/// Ce que lit le remplissage d'un calque de couleur à une image, en plus des griffonnages : les dessins de ses
/// sources et sa fermeture des trous.
#[derive(Clone, Debug, PartialEq, Hash)]
pub struct GriffonInput {
    pub sources: Vec<CelId>,
    pub gap: u8,
}

/// Opacité à partir de laquelle un pixel des sources est un trait (il ferme les zones).
pub const LINE_ALPHA: u8 = 128;
/// Pixels de trait que la couleur gagne sous le trait, de chaque côté.
pub const UNDER: usize = 2;

/// Les dessins des sources du calque de couleur `l` (dans `layers`) à l'image `frame` : son calque de trait, puis ses
/// sources supplémentaires. Triés, sans doublon.
pub fn source_cels(layers: &[Layer], l: &Layer, c: &CalqueCouleur, frame: usize) -> Vec<CelId> {
    let mut cels: Vec<CelId> = Vec::new();
    let ids = std::iter::once(c.trait_id).filter(|&t| t != 0).chain(c.sources.iter().copied());
    for id in ids {
        if id == l.id {
            continue;
        }
        if let Some(s) = layers.iter().find(|s| s.id == id && !s.folder) {
            cels.extend(s.shown_at(frame));
        }
    }
    cels.sort_unstable();
    cels.dedup();
    cels
}

/// Le dessin de griffonnages du calque de couleur `li` à l'image `frame`, et ce que lit son remplissage. Rien pour un
/// calque qui n'a pas (encore) de griffonnages, ou à une image sans dessin.
pub fn entree(layers: &[Layer], li: usize, frame: usize) -> Option<(CelId, GriffonInput)> {
    let l = layers.get(li)?;
    let c = l.couleur.as_ref().filter(|c| c.remplissage)?;
    let peinture = l.shown_at(frame)?;
    let griffonnage = *c.griffonnages.get(&peinture)?;
    Some((griffonnage, GriffonInput { sources: source_cels(layers, l, c, frame), gap: c.trous }))
}

/// Ancienne entrée du remplissage d'un calque CTG, que `propager.rs` (qu'on ne touche pas ici) appelle encore pour
/// ses images guides, sur un élément dont le dessin est celui du calque. Un calque de couleur a son remplissage dans
/// un élément à part (`entree`) : rien ici. Les images guides de la propagation ne montrent donc pas les remplissages
/// au griffonnage (le trait et la peinture libre, si).
pub fn input(_layers: &[Layer], _li: usize, _frame: usize) -> Option<GriffonInput> {
    None
}

/// Le calque de couleur `l` a un remplissage au griffonnage.
pub fn remplit(l: &Layer) -> bool {
    l.couleur.as_ref().is_some_and(|c| c.remplissage)
}

/// Rayon (pixels) du tampon que pose le pot de peinture en mode « Griffonner ».
pub const RAYON_TAMPON: f32 = 5.0;

/// Pose un disque de `rayon` pixels de `couleur` en (`cx`, `cy`) ; renvoie les tuiles d'avant (annulation). Hors de la
/// feuille, rien.
pub fn tampon(r: &mut Raster, cx: f32, cy: f32, rayon: f32, couleur: Color32) -> Vec<(usize, Option<crate::raster::Tile>)> {
    let mut old: Vec<(usize, Option<crate::raster::Tile>)> = Vec::new();
    if r.width == 0 || r.height == 0 || !cx.is_finite() || !cy.is_finite() {
        return old;
    }
    let mut vues: HashSet<usize> = HashSet::new();
    let (w, h) = (r.width, r.height);
    let (x0, x1) = ((cx - rayon).floor().max(0.0) as usize, (cx + rayon).ceil().max(0.0) as usize);
    let (y0, y1) = ((cy - rayon).floor().max(0.0) as usize, (cy + rayon).ceil().max(0.0) as usize);
    for y in y0..=y1.min(h - 1) {
        for x in x0..=x1.min(w - 1) {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            if dx * dx + dy * dy > rayon * rayon {
                continue;
            }
            let idx = r.tile_index_of(x, y);
            if vues.insert(idx) {
                old.push((idx, r.tiles[idx].clone()));
            }
            r.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = couleur;
        }
    }
    old
}

/// Les traits épaissis de `r` pixels (carré) : les pixels à moins de `r` d'un trait, en lignes
/// puis en colonnes (deux balayages par sens).
fn thicken(line: &[bool], w: usize, h: usize, r: usize) -> Vec<bool> {
    let pass = |src: &[bool], n: usize, len: usize, at: &dyn Fn(usize, usize) -> usize| -> Vec<bool> {
        let mut out = vec![false; src.len()];
        for k in 0..n {
            // Distance au dernier trait vu, de gauche à droite puis de droite à gauche.
            let mut d = usize::MAX;
            for i in 0..len {
                d = if src[at(k, i)] { 0 } else { d.saturating_add(1) };
                if d <= r {
                    out[at(k, i)] = true;
                }
            }
            d = usize::MAX;
            for i in (0..len).rev() {
                d = if src[at(k, i)] { 0 } else { d.saturating_add(1) };
                if d <= r {
                    out[at(k, i)] = true;
                }
            }
        }
        out
    };
    let rows = pass(line, h, w, &|y, x| y * w + x);
    pass(&rows, w, h, &|x, y| y * w + x)
}

/// Le remplissage : les zones fermées par les traits de `sources`, touchées par `graffiti`,
/// remplies de leur couleur. `gap` : les trous des traits jusqu'à `2 × gap` pixels sont fermés.
pub fn fill(sources: &[&Raster], graffiti: &Raster, gap: usize) -> Raster {
    let (w, h) = (graffiti.width, graffiti.height);
    let mut out = Raster::new(w, h);
    if w == 0 || h == 0 {
        return out;
    }
    // Traits : l'opacité la plus forte des sources.
    let mut line = vec![0u8; w * h];
    for s in sources {
        for (idx, t) in s.tiles.iter().enumerate() {
            let Some(t) = t else { continue };
            let (ox, oy, tw, th) = s.tile_bounds(idx);
            for y in 0..th.min(h.saturating_sub(oy)) {
                for x in 0..tw.min(w.saturating_sub(ox)) {
                    let a = t[y * TILE + x].a();
                    let p = &mut line[(oy + y) * w + ox + x];
                    *p = (*p).max(a);
                }
            }
        }
    }
    // Murs des zones : les traits, épaissis de `gap` pixels pour fermer les trous.
    let solid: Vec<bool> = line.iter().map(|&a| a >= LINE_ALPHA).collect();
    let wall = if gap > 0 { thicken(&solid, w, h, gap) } else { solid.clone() };
    // Couleur de chaque pixel : 0 = rien, sinon rang + 1 dans `colors`.
    let mut colors: Vec<Color32> = Vec::new();
    let mut color_of = vec![0u32; w * h];
    let mut seen = vec![false; w * h];
    let mut stack: Vec<usize> = Vec::new();
    let mut region: Vec<usize> = Vec::new();
    for (idx, t) in graffiti.tiles.iter().enumerate() {
        let Some(t) = t else { continue };
        let (ox, oy, tw, th) = graffiti.tile_bounds(idx);
        for y in 0..th {
            for x in 0..tw {
                if t[y * TILE + x].a() == 0 {
                    continue;
                }
                let start = (oy + y) * w + ox + x;
                if seen[start] || wall[start] {
                    continue;
                }
                // La zone de ce graffiti, et les couleurs des graffitis qui la touchent.
                region.clear();
                stack.push(start);
                seen[start] = true;
                let mut votes: HashMap<[u8; 3], u32> = HashMap::new();
                while let Some(p) = stack.pop() {
                    region.push(p);
                    let g = graffiti.get(p % w, p / w);
                    if g.a() != 0 {
                        let [r, gg, b, _] = crate::raster::unpremultiply(g);
                        *votes.entry([r, gg, b]).or_insert(0) += 1;
                    }
                    let (px, py) = (p % w, p / w);
                    let mut visit = |q: usize| {
                        if !seen[q] && !wall[q] {
                            seen[q] = true;
                            stack.push(q);
                        }
                    };
                    if px > 0 {
                        visit(p - 1);
                    }
                    if px + 1 < w {
                        visit(p + 1);
                    }
                    if py > 0 {
                        visit(p - w);
                    }
                    if py + 1 < h {
                        visit(p + w);
                    }
                }
                let Some((rgb, _)) = votes.into_iter().max_by_key(|(c, n)| (*n, *c)) else { continue };
                colors.push(Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
                let k = colors.len() as u32;
                for &p in &region {
                    color_of[p] = k;
                }
            }
        }
    }
    if colors.is_empty() {
        return out;
    }
    // La couleur regagne la bande d'épaississement (hors des vrais traits), puis passe sous les
    // traits voisins (`UNDER` pixels).
    grow(&mut color_of, w, h, gap, |p| !solid[p]);
    grow(&mut color_of, w, h, UNDER, |p| line[p] != 0);
    for (p, &k) in color_of.iter().enumerate() {
        if k != 0 {
            let (x, y) = (p % w, p / w);
            let idx = out.tile_index_of(x, y);
            out.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = colors[k as usize - 1];
        }
    }
    out
}

/// `times` fois : chaque pixel sans couleur où `can` permet d'aller prend la couleur d'un voisin.
fn grow(color_of: &mut [u32], w: usize, h: usize, times: usize, can: impl Fn(usize) -> bool) {
    for _ in 0..times {
        let before = color_of.to_vec();
        let mut changed = false;
        for y in 0..h {
            for x in 0..w {
                let p = y * w + x;
                if before[p] != 0 || !can(p) {
                    continue;
                }
                let near = [(x > 0).then(|| p - 1), (x + 1 < w).then(|| p + 1), (y > 0).then(|| p - w), (y + 1 < h).then(|| p + w)];
                if let Some(k) = near.into_iter().flatten().map(|q| before[q]).find(|&k| k != 0) {
                    color_of[p] = k;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
}

// ------------------------------------------------------------------------------------ anciens calques CTG

/// Un calque CTG d'un projet d'avant (format 35 et moins), tel que son manifeste le décrit.
#[derive(Clone, Debug, PartialEq)]
pub struct AncienCtg {
    /// Numéro (de séance) du calque.
    pub calque: u64,
    /// Numéros des calques sources choisis (aucun : tous les autres calques de dessin du plan).
    pub sources: Vec<u64>,
    /// Fermeture des trous (pixels de chaque côté du trou).
    pub trou: u8,
}

/// Le numéro du dessin de peinture (vide) que reçoit le dessin `base` quand un ancien calque CTG devient un calque de
/// couleur. Dérivé du numéro de `base` plutôt que tiré du compteur du projet : deux ouvertures du même vieux projet
/// (ou deux équipiers qui relisent le même plan partagé) donnent les mêmes numéros, et rien ne peut heurter un dessin
/// d'un autre plan. Si un dessin porte déjà ce numéro (jamais en pratique), on cherche le suivant.
fn numero_peinture(cels: &HashMap<CelId, Cel>, base: CelId) -> CelId {
    let mut id = base | (1 << 62);
    while cels.contains_key(&id) {
        id = id.wrapping_add(1) | (1 << 62);
    }
    id
}

/// Les dessins du calque CTG `ctg` peuvent-ils devenir ceux d'un calque de couleur lié au calque de trait `t` ? Il
/// faut que l'image reste la même : mêmes pré / post-comportement, transformation, profondeur et clés de position que
/// le trait (le calque de couleur les reprend de lui), et des images qui se recouvrent exactement — à chaque image où le
/// trait montre un dessin, le calque CTG montre un même dessin de graffitis (ou aucun) pour ce dessin du trait, jamais
/// le même graffiti pour deux dessins du trait ; là où le trait n'a rien, le calque CTG non plus. Rend, pour chaque dessin
/// du trait, son dessin de graffitis.
fn recouvre(ctg: &Layer, t: &Layer) -> Option<BTreeMap<CelId, Option<CelId>>> {
    if ctg.pre != t.pre || ctg.post != t.post || ctg.transform != t.transform || ctg.depth != t.depth || ctg.pos_keys != t.pos_keys {
        return None;
    }
    let mut map: BTreeMap<CelId, Option<CelId>> = BTreeMap::new();
    for f in 0..ctg.frames.len().max(t.frames.len()) {
        let trait_cel = t.frames.get(f).copied().flatten();
        let graffiti = ctg.frames.get(f).copied().flatten();
        match (trait_cel, graffiti) {
            (None, None) => {}
            (None, Some(_)) => return None,
            (Some(tc), g) => match map.get(&tc) {
                Some(prev) if *prev != g => return None,
                Some(_) => {}
                None => {
                    map.insert(tc, g);
                }
            },
        }
    }
    let mut seen = HashSet::new();
    map.values().flatten().all(|g| seen.insert(*g)).then_some(map)
}

/// Fait des anciens calques CTG `anciens` de `panel` des calques de couleur qui montrent la même image et gardent leurs
/// graffitis (modifiables, et qui suivent toujours les traits). Rend le nombre de calques convertis.
///
/// - Le calque CTG reste à sa place dans la pile, avec tout ce qu'il avait (opacité, mode de fusion, effets…). Il porte
///   désormais un lien de calque de couleur, et ses dessins de graffitis deviennent les dessins de griffonnages : ce sont
///   les mêmes fichiers, rien n'est réécrit ni effacé.
/// - **Lié** à son premier calque source quand celui-ci est juste au-dessus de lui, dans le même dossier, et que leurs images
///   se recouvrent (`recouvre`) : le calque de couleur suit alors les images de ce calque de trait. Sinon **libre** (sans
///   calque de trait propre, images à lui) ; ses sources deviennent des « sources supplémentaires ». Dans les deux cas, les
///   sources sont les mêmes que celles du calque CTG (aucune choisie : tous les autres calques de dessin).
/// - Les dessins de peinture (vides) sont neufs (`numero_peinture`). Le mode « Griffonner » est allumé : sur ce calque,
///   dessiner continue de griffonner, comme avant.
pub fn migrer_ctg(panel: &mut Panel, cels: &mut HashMap<CelId, Cel>, (w, h): (usize, usize), anciens: &[AncienCtg]) -> usize {
    let ctg_ids: HashSet<u64> = anciens.iter().map(|a| a.calque).collect();
    let mut convertis = 0;
    for a in anciens {
        let Some(i) = panel.layers.iter().position(|l| l.id == a.calque) else { continue };
        if panel.layers[i].couleur.is_some() || panel.layers[i].folder {
            continue;
        }
        // Les sources, comme l'ancien calque les lisait : les choisies, sinon tous les autres calques de dessin.
        let lisibles = |l: &Layer| !l.folder && l.effet.is_none() && l.video.is_none() && !ctg_ids.contains(&l.id) && l.id != a.calque;
        let sources: Vec<u64> = panel.layers.iter().filter(|l| lisibles(l)).filter(|l| a.sources.is_empty() || a.sources.contains(&l.id)).map(|l| l.id).collect();
        // Lié à la source juste au-dessus, si l'image reste la même.
        let lie = panel
            .layers
            .get(i + 1)
            .filter(|t| sources.contains(&t.id) && crate::calquecouleur::peut_porter(t) && t.parent == panel.layers[i].parent)
            .and_then(|t| recouvre(&panel.layers[i], t).map(|m| (t.id, m)));
        let gap = a.trou.min(50);
        let mut link = CalqueCouleur::new(lie.as_ref().map_or(0, |(t, _)| *t));
        link.trous = gap;
        link.remplissage = true;
        link.griffonner = true;
        let mut nouvelles: HashMap<CelId, CelId> = HashMap::new();
        let mut frames: Vec<Option<CelId>> = Vec::new();
        let mut peinture_de = |cels: &mut HashMap<CelId, Cel>, graffiti: Option<CelId>, base: CelId, link: &mut CalqueCouleur| -> CelId {
            let id = numero_peinture(cels, base);
            cels.insert(id, Cel::new(w, h, LayerKind::Bitmap));
            if let Some(g) = graffiti {
                link.griffonnages.insert(id, g);
                nouvelles.insert(g, id);
            }
            id
        };
        match &lie {
            Some((_, map)) => {
                for (tc, g) in map {
                    let p = peinture_de(cels, *g, g.unwrap_or(*tc), &mut link);
                    link.dessins.insert(*tc, p);
                }
                let t = &panel.layers[i + 1];
                frames = t.frames.iter().map(|f| f.and_then(|tc| link.dessins.get(&tc).copied())).collect();
            }
            None => {
                let mut par_graffiti: HashMap<CelId, CelId> = HashMap::new();
                for f in &panel.layers[i].frames {
                    frames.push(f.map(|g| match par_graffiti.get(&g) {
                        Some(p) => *p,
                        None => {
                            let p = peinture_de(cels, Some(g), g, &mut link);
                            par_graffiti.insert(g, p);
                            p
                        }
                    }));
                }
                link.sources = sources.clone();
            }
        }
        if lie.is_some() {
            link.sources = sources.iter().copied().filter(|s| Some(*s) != lie.as_ref().map(|(t, _)| *t)).collect();
        }
        // Un calque vectoriel de graffitis : son image (les pixels) est ce qui compte pour le remplissage.
        if panel.layers[i].kind == LayerKind::Vector {
            for g in link.griffonnages.values() {
                if let Some(c) = cels.get_mut(g) {
                    c.vector = None;
                }
            }
        }
        let l = &mut panel.layers[i];
        l.frames = frames;
        l.trim();
        l.names = l.names.iter().map(|(c, n)| (nouvelles.get(c).copied().unwrap_or(*c), n.clone())).collect();
        l.color_marks = l.color_marks.iter().map(|(c, k)| (nouvelles.get(c).copied().unwrap_or(*c), *k)).collect();
        l.kind = LayerKind::Bitmap;
        // (Un fond sous le trait n'a jamais été montré sur un calque CTG : il ne doit pas apparaître maintenant.)
        l.fond = None;
        l.couleur = Some(link);
        convertis += 1;
    }
    convertis
}

/// Convertit les anciens calques CTG de tout le document (`anciens`, lus dans le manifeste) : les plans, et les autres versions
/// des plans. Rend le nombre de calques convertis.
pub fn migrer_ctg_document(doc: &mut Document, anciens: &[AncienCtg]) -> usize {
    if anciens.is_empty() {
        return 0;
    }
    let size = (doc.width, doc.height);
    let mut cels = std::mem::take(&mut doc.cels);
    let mut n = 0;
    for p in &mut doc.panels {
        n += migrer_ctg(p, &mut cels, size, anciens);
    }
    for v in doc.versions.values_mut() {
        for r in &mut v.autres {
            n += migrer_ctg(&mut r.panel, &mut cels, size, anciens);
        }
    }
    doc.cels = cels;
    n
}
