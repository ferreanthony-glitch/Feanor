//! Fusionner des calques (« Fusionner vers le bas », « Fusionner les calques choisis »…).
//!
//! La fusion **refait ce que l'écran montre** : pour chaque image du plan, les deux calques sont composés avec la
//! même composition que l'écran et l'export (`render::composite_area`) : mode de fusion du calque du dessus, opacité
//! du jour (clés d'opacité comprises), effets cuits, aplat sous le trait, transformation du calque (même animée),
//! dessins hors de la feuille. Le résultat est un dessin neuf par combinaison d'images ; les dessins d'avant restent
//! intacts (l'annulation y revient, en une étape).
//!
//! - Deux calques vectoriels **sans rien à cuire** (ni effet, ni aplat, ni transformation, ni mode de fusion, ni
//!   opacité animée) restent un calque vectoriel : les éléments se rangent l'un après l'autre.
//! - Tout autre cas donne un calque bitmap, et la bulle du menu le dit avant de cliquer.
//! - Image par image : l'union des expositions. Deux images qui montrent les mêmes dessins, réglés pareil, gardent le
//!   même dessin fusionné (la tenue est gardée).
//! - Ce qui ne peut pas se fondre dans un autre calque est refusé **avec la raison** (`raison_*`), qu'une entrée grisée
//!   du menu affiche en bulle : vidéo, calque d'effet, couleur unie, Décor, calque de toute la scène, calque de couleur
//!   qui a des griffonnages, calque caché ou verrouillé, profondeurs ou dossiers différents, carte inclinée en 3D.
//!
//! Limite assumée : le mode de fusion du calque du dessus agit sur le calque du dessous, pas sur ce qui est plus bas
//! (le papier, les autres calques) : l'écran peut différer un peu si le dessus est en « Produit » ou « Écran » sur un
//! fond qui n'est pas le dessous.

use std::collections::{BTreeMap, HashMap};

use eframe::egui::Color32;

use crate::blend::BlendMode;
use crate::document::{Behaviour, Cel, CelId, Document, Layer, LayerKind, LayerTransform, Panel};
use crate::raster::{outer_id, Raster, Tile, TILE, TILE_PIXELS, WORLD};
use crate::render::{self, Area, Plan, PlanItem};
use crate::vector::VectorDrawing;

/// Pourquoi ce calque ne peut se fondre dans aucun autre (rien : il le peut, sous réserve de son voisin).
pub(crate) fn raison_calque(doc: &Document, l: &Layer) -> Option<String> {
    let nom = &l.name;
    if l.folder {
        return Some("Un dossier ne se fusionne pas : fusionne ses calques.".into());
    }
    if l.video.is_some() {
        return Some(format!(
            "« {nom} » est un calque vidéo : ses images viennent du fichier. Dessine sur un calque au-dessus, ou transforme-le en calque d'animation (Plus > Calque vidéo)."
        ));
    }
    if l.effet.is_some() {
        return Some(format!("« {nom} » est un calque d'effet : il agit sur tout ce qui est dessous, il ne se fond pas dans un autre calque."));
    }
    if l.unie.is_some() {
        return Some(format!("« {nom} » est un calque Couleur unie : son fond couvre toute la feuille et reste un calque à part."));
    }
    if l.decor {
        return Some(format!("« {nom} » est le calque Décor : il est partagé par toutes les cases du plan et ne se fusionne pas."));
    }
    if l.scene_wide.is_some() {
        return Some(format!("« {nom} » est un calque de toute la scène : il est partagé entre plusieurs plans et ne se fusionne pas."));
    }
    // Le remplissage au griffonnage d'un calque de couleur se calcule d'après les traits (`griffonnage.rs`) : il ne se fige pas
    // dans un dessin de la fusion.
    if l.couleur.as_ref().is_some_and(|c| c.dessins_griffonnages().any(|g| doc.cels.get(&g).is_some_and(|c| !c.is_empty()))) {
        return Some(format!(
            "« {nom} » est un calque de couleur qui a des griffonnages : son aplat suit les traits. Gomme ses griffonnages d'abord, ou garde le calque."
        ));
    }
    if !l.visible {
        return Some(format!("« {nom} » est caché : affiche-le avant de le fusionner (ce qu'on ne voit pas ne se fond pas)."));
    }
    if l.locked {
        return Some(format!("« {nom} » est verrouillé : déverrouille-le (Alt+L) avant de le fusionner."));
    }
    None
}

/// Les images à fusionner : celles des deux calques, et tout le plan si l'un montre ses images avant / après (boucle,
/// va-et-vient, maintien : elles sont cuites image par image).
fn plage(panel: &Panel, a: &Layer, b: &Layer) -> usize {
    let n = a.frames.len().max(b.frames.len());
    if [a, b].iter().any(|l| l.pre != Behaviour::Nothing || l.post != Behaviour::Nothing) {
        n.max(panel.frame_count())
    } else {
        n
    }
}

/// Pourquoi ces deux calques voisins (`below` sous `above`) ne peuvent pas se fondre ensemble.
fn raison_paire(panel: &Panel, below: &Layer, above: &Layer) -> Option<String> {
    if below.depth != above.depth {
        return Some(format!("« {} » et « {} » ne sont pas à la même profondeur (multiplan).", below.name, above.name));
    }
    if below.parent != above.parent {
        return Some(format!("« {} » et « {} » ne sont pas dans le même dossier.", below.name, above.name));
    }
    // (Les notes ne se fondent pas dans le dessin du film : un calque d'annotation ne se fusionne qu'avec un autre.)
    if below.annotation != above.annotation {
        return Some("Un calque d'annotation ne se fusionne qu'avec un autre calque d'annotation : ses notes n'entrent pas dans le dessin.".into());
    }
    for f in 0..plage(panel, below, above) {
        for l in [below, above] {
            let t = l.pose(f);
            if t.is_tilted() || t.dz != 0.0 {
                return Some(format!("« {} » est incliné ou avancé en 3D : une carte de l'Espace 3D ne se fusionne pas.", l.name));
            }
        }
    }
    None
}

/// Pourquoi le calque `li` du plan `index` ne peut pas se fusionner avec celui du dessous (rien : il le peut).
pub(crate) fn blocker_down(doc: &Document, index: usize, li: usize) -> Option<String> {
    let Some(panel) = doc.panels.get(index) else { return Some("Plan introuvable.".into()) };
    if li == 0 || li >= panel.layers.len() {
        return Some("Il n'y a pas de calque en dessous.".into());
    }
    let (above, below) = (&panel.layers[li], &panel.layers[li - 1]);
    raison_calque(doc, above).or_else(|| raison_calque(doc, below)).or_else(|| raison_paire(panel, below, above))
}

/// Les rangs à fusionner pour la liste `lis` : un dossier choisi compte pour ses calques, triés, sans doublon.
pub(crate) fn rangs_a_fusionner(layers: &[Layer], lis: &[usize]) -> Vec<usize> {
    let mut v: Vec<usize> = lis
        .iter()
        .copied()
        .filter(|&l| l < layers.len())
        .flat_map(|l| if layers[l].folder { crate::folders::children_of(layers, l) } else { vec![l] })
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Pourquoi les calques `lis` du plan `index` ne peuvent pas se fusionner en un seul (rien : ils le peuvent).
pub(crate) fn blocker_list(doc: &Document, index: usize, lis: &[usize]) -> Option<String> {
    let Some(panel) = doc.panels.get(index) else { return Some("Plan introuvable.".into()) };
    let lis = rangs_a_fusionner(&panel.layers, lis);
    if lis.len() < 2 {
        return Some("Il faut au moins deux calques à fusionner : Ctrl + clic ou Maj + clic sur leurs lignes pour en choisir.".into());
    }
    for &l in &lis {
        if let Some(r) = raison_calque(doc, &panel.layers[l]) {
            return Some(r);
        }
    }
    for w in lis.windows(2) {
        if let Some(r) = raison_paire(panel, &panel.layers[w[0]], &panel.layers[w[1]]) {
            return Some(r);
        }
    }
    None
}

/// Deux calques vectoriels sans rien à cuire : la fusion garde les éléments vectoriels.
fn reste_vectoriel(below: &Layer, above: &Layer) -> bool {
    let simple = |l: &Layer| {
        l.kind == LayerKind::Vector
            && l.fx.is_empty()
            && l.fond.is_none()
            && l.transform.is_identity()
            && l.pos_keys.is_empty()
            && l.opacity_keys.is_empty()
    };
    simple(below) && simple(above) && above.blend == BlendMode::Normal
}

/// Ce que donnera la fusion des calques `lis`, en une phrase (la bulle de l'entrée du menu).
pub(crate) fn resultat_de(doc: &Document, index: usize, lis: &[usize]) -> &'static str {
    let Some(panel) = doc.panels.get(index) else { return "" };
    let lis = rangs_a_fusionner(&panel.layers, lis);
    let layers: Vec<&Layer> = lis.iter().map(|&l| &panel.layers[l]).collect();
    if layers.len() >= 2 && layers.windows(2).all(|w| reste_vectoriel(w[0], w[1])) {
        "Le résultat reste un calque vectoriel : les traits restent modifiables."
    } else if layers.iter().all(|l| l.kind == LayerKind::Bitmap) {
        "Le résultat est un calque bitmap, tel qu'il s'affiche : opacité, mode de fusion et effets sont cuits."
    } else {
        "Le résultat est un calque bitmap : les traits vectoriels ne seront plus modifiables (Ctrl+Z les rend)."
    }
}

/// Fusionne le calque `layer` du plan `index` dans celui du dessous. Les dessins fusionnés sont de nouveaux dessins : ceux
/// d'avant restent intacts. Rien ne change si la fusion est refusée (`blocker_down`).
pub(crate) fn merge_down(doc: &mut Document, index: usize, layer: usize) -> Result<(), String> {
    merge_down_ctl(doc, index, layer, None)
}

/// Ce que la fusion en tâche de fond partage avec l'interface (`fusiontache.rs`) : le bouton « Arrêter » et la progression.
#[derive(Default)]
pub(crate) struct Controle {
    pub arret: std::sync::atomic::AtomicBool,
    pub fait: std::sync::atomic::AtomicUsize,
    pub total: std::sync::atomic::AtomicUsize,
}

/// Le message d'une fusion arrêtée.
pub(crate) const ARRETEE: &str = "Fusion arrêtée.";

/// `merge_down`, qui regarde `ctl` à chaque image : arrêtée, elle s'interrompt (`ARRETEE`) et fait avancer la progression.
pub(crate) fn merge_down_ctl(doc: &mut Document, index: usize, layer: usize, ctl: Option<&Controle>) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    if let Some(raison) = blocker_down(doc, index, layer) {
        return Err(raison);
    }
    let panel = &doc.panels[index];
    let (above, below) = (panel.layers[layer].clone(), panel.layers[layer - 1].clone());
    let n = plage(panel, &below, &above);
    let vectoriel = reste_vectoriel(&below, &above);
    let mut frames: Vec<Option<CelId>> = Vec::with_capacity(n);
    // Pour les noms d'instance et les marques : d'où vient chaque dessin fusionné.
    let mut origines: Vec<(CelId, Option<CelId>, Option<CelId>)> = Vec::new();
    let mut vus_vecto: HashMap<(Option<CelId>, Option<CelId>), Option<CelId>> = HashMap::new();
    let mut vus_plans: Vec<(Plan, Option<CelId>)> = Vec::new();
    for f in 0..n {
        if let Some(c) = ctl {
            if c.arret.load(Ordering::Relaxed) {
                return Err(ARRETEE.into());
            }
            c.fait.fetch_add(1, Ordering::Relaxed);
        }
        let (cb, ca) = (below.shown_at(f), above.shown_at(f));
        let cel = if cb.is_none() && ca.is_none() {
            None
        } else if vectoriel {
            match vus_vecto.get(&(cb, ca)) {
                Some(&c) => c,
                None => {
                    let c = merged_vector(doc, (cb, ca), below.opacity, above.opacity);
                    vus_vecto.insert((cb, ca), c);
                    if let Some(c) = c {
                        origines.push((c, cb, ca));
                    }
                    c
                }
            }
        } else {
            let plan = Plan { paper: None, items: items_de(&below, cb, f, true).into_iter().chain(items_de(&above, ca, f, false)).collect() };
            match vus_plans.iter().find(|(p, _)| *p == plan) {
                Some((_, c)) => *c,
                None => {
                    let cel = rendre(doc, &plan);
                    let c = doc.add_cel(cel);
                    vus_plans.push((plan, Some(c)));
                    origines.push((c, cb, ca));
                    Some(c)
                }
            }
        };
        frames.push(cel);
    }
    // Les noms d'instance et les marques de couleur : ceux du dessin du dessous, sinon du dessus.
    let mut names: BTreeMap<CelId, String> = BTreeMap::new();
    let mut color_marks: BTreeMap<CelId, u8> = BTreeMap::new();
    for (m, cb, ca) in origines {
        if let Some(nom) = cb.and_then(|c| below.names.get(&c)).or_else(|| ca.and_then(|c| above.names.get(&c))) {
            names.insert(m, nom.clone());
        }
        if let Some(k) = cb.and_then(|c| below.color_marks.get(&c)).or_else(|| ca.and_then(|c| above.color_marks.get(&c))) {
            color_marks.insert(m, *k);
        }
    }
    let mut merged = Layer {
        kind: if vectoriel { LayerKind::Vector } else { LayerKind::Bitmap },
        opacity: 1.0,
        opacity_keys: Vec::new(),
        transform: LayerTransform::default(),
        pos_keys: Vec::new(),
        frames,
        // Les effets, l'aplat et les images avant / après sont cuits dans les dessins.
        fx: Vec::new(),
        fx_enabled: true,
        fond: None,
        pre: Behaviour::Nothing,
        post: Behaviour::Nothing,
        // Un calque de couleur fondu avec un autre n'a plus de trait à suivre.
        couleur: None,
        names,
        color_marks,
        ..below
    };
    merged.trim();
    let p = &mut doc.panels[index];
    p.layers[layer - 1] = merged;
    p.layers.remove(layer);
    Ok(())
}

/// Les éléments du plan d'un calque à l'image `f` (son aplat sous son dessin, puis son dessin), tels que l'écran les
/// pose pour une caméra au centre. `bas` : le calque du dessous, composé en mode normal (son propre mode de fusion
/// reste celui du calque obtenu).
fn items_de(l: &Layer, cel: Option<CelId>, f: usize, bas: bool) -> Vec<PlanItem> {
    let Some(cel) = cel else { return Vec::new() };
    let t = l.pose(f);
    let line = PlanItem {
        cel,
        opacity: (l.opacity_at(f) * 255.0).round() as u8,
        tint: None,
        transform: (!t.is_identity()).then_some(t),
        blend: if bas { BlendMode::Normal } else { l.blend },
        fx: crate::fx::layer_steps(l, f, None),
        ctg: None,
        fond: None,
        pre: None,
    };
    render::fond_item(l, &line).into_iter().chain([line]).collect()
}

/// Deux dessins vectoriels l'un après l'autre (le dessus par-dessus), chacun avec l'opacité de son calque.
fn merged_vector(doc: &mut Document, (below, above): (Option<CelId>, Option<CelId>), ob: f32, oa: f32) -> Option<CelId> {
    let sources: Vec<(Cel, f32)> = [(below, ob), (above, oa)]
        .into_iter()
        .filter_map(|(c, o)| c.and_then(|c| doc.cels.get(&c)).map(|cel| (cel.clone(), o.clamp(0.0, 1.0))))
        .collect();
    let id = doc.new_cel(LayerKind::Vector);
    let cel = doc.cels.get_mut(&id).expect("dessin créé juste avant");
    let drawing = cel.vector.get_or_insert_with(VectorDrawing::default);
    for (src, o) in sources {
        for mut e in src.vector.map(|d| d.elements).unwrap_or_default() {
            match &mut e {
                crate::vector::Element::Stroke { opacity, .. }
                | crate::vector::Element::Fill { opacity, .. }
                | crate::vector::Element::Text { opacity, .. }
                | crate::vector::Element::Image { opacity, .. } => *opacity *= o,
            }
            drawing.elements.push(e);
        }
    }
    cel.rerender();
    Some(id)
}

/// Un dessin bitmap : ce que `plan` montre sur un papier transparent, la feuille et ce qui dépasse autour d'elle.
pub(crate) fn rendre(doc: &Document, plan: &Plan) -> Cel {
    let mut raster = Raster::new(doc.width, doc.height);
    let t = TILE as i64;
    // Une composition simple (ni effet, ni aplat, ni transformation) ne touche que les tuiles où un dessin a quelque chose :
    // tuile par tuile, proportionnel à ce qui est dessiné (une fusion de dessins légers prend quelques millisecondes).
    let simple = plan.items.iter().all(|i| i.fx.is_empty() && i.fond.is_none() && i.transform.is_none() && i.pre.is_none() && i.ctg.is_none());
    if simple {
        let mut tuiles: std::collections::BTreeSet<(i32, i32)> = std::collections::BTreeSet::new();
        for item in &plan.items {
            if let Some(cel) = doc.cels.get(&item.cel) {
                tuiles.extend(cel.raster.drawn_ids().into_iter().map(|id| cel.raster.key_of(id)));
            }
        }
        for (tx, ty) in tuiles {
            let mut buf = vec![Color32::TRANSPARENT; TILE_PIXELS];
            render::composite_world_tile(doc, plan, tx, ty, &mut buf, TILE, (tx as i64 * t, ty as i64 * t));
            if buf.iter().any(|c| c.a() != 0) {
                let mut px = [Color32::TRANSPARENT; TILE_PIXELS];
                px.copy_from_slice(&buf);
                raster.replace_slot(raster.id_of(tx, ty), Some(Tile::new(px)));
            }
        }
        raster.share_solid_tiles();
        return Cel::bitmap(raster);
    }
    let sheet = Area::sheet(doc).tile_aligned();
    let aw = sheet.width();
    let pixels = render::composite_area(doc, plan, sheet);
    if !pixels.is_empty() {
        for ty in (sheet.y0 / t)..(sheet.y1 / t) {
            for tx in (sheet.x0 / t)..(sheet.x1 / t) {
                let mut px = [Color32::TRANSPARENT; TILE_PIXELS];
                let mut any = false;
                for y in 0..TILE {
                    let s = ((ty * t - sheet.y0) as usize + y) * aw + (tx * t - sheet.x0) as usize;
                    let row = &pixels[s..s + TILE];
                    px[y * TILE..(y + 1) * TILE].copy_from_slice(row);
                    any |= row.iter().any(|c| c.a() != 0);
                }
                if any {
                    let id = raster.id_of(tx as i32, ty as i32);
                    raster.replace_slot(id, Some(Tile::new(px)));
                }
            }
        }
    }
    // Ce qui dépasse de la feuille (toile infinie).
    for (tx, ty) in render::plan_world_tiles(doc, plan, Area { x0: -WORLD, y0: -WORLD, x1: WORLD, y1: WORLD }) {
        if raster.in_window(tx, ty) {
            continue;
        }
        let mut buf = vec![Color32::TRANSPARENT; TILE_PIXELS];
        render::composite_world_tile(doc, plan, tx, ty, &mut buf, TILE, (tx as i64 * t, ty as i64 * t));
        if buf.iter().any(|c| c.a() != 0) {
            let mut px = [Color32::TRANSPARENT; TILE_PIXELS];
            px.copy_from_slice(&buf);
            raster.replace_slot(outer_id(tx, ty), Some(Tile::new(px)));
        }
    }
    raster.share_solid_tiles();
    Cel::bitmap(raster)
}

/// Ce que coûterait la fusion de `lis` : `(poids, lourd)`. Le poids compte les tuiles à composer (une composition simple coûte ce
/// qu'il y a de dessiné, pas la feuille), `lourd` dit qu'un calque a des effets, un aplat ou une transformation (clés de
/// position comprises) : la composition touche alors toute la feuille.
pub(crate) fn travail_de(doc: &Document, index: usize, lis: &[usize]) -> (usize, bool) {
    let Some(panel) = doc.panels.get(index) else { return (0, false) };
    let lis = rangs_a_fusionner(&panel.layers, lis);
    let layers: Vec<&Layer> = lis.iter().map(|&l| &panel.layers[l]).collect();
    let mut n = layers.iter().map(|l| l.frames.len()).max().unwrap_or(0);
    if layers.iter().any(|l| l.pre != Behaviour::Nothing || l.post != Behaviour::Nothing) {
        n = n.max(panel.frame_count());
    }
    let lourd = layers.iter().any(|l| !l.fx.is_empty() || l.fond.is_some() || !l.transform.is_identity() || !l.pos_keys.is_empty());
    let anime = layers.iter().any(|l| !l.opacity_keys.is_empty());
    let tuiles = |l: &Layer, f: usize| l.shown_at(f).and_then(|c| doc.cels.get(&c)).map_or(0, |c| c.raster.drawn_ids().len().max(1));
    let poids = if anime {
        (0..n).map(|f| layers.iter().map(|l| tuiles(l, f)).sum::<usize>()).sum()
    } else {
        let mut vus: std::collections::HashSet<Vec<Option<CelId>>> = std::collections::HashSet::new();
        (0..n).filter(|&f| vus.insert(layers.iter().map(|l| l.shown_at(f)).collect())).map(|f| layers.iter().map(|l| tuiles(l, f)).sum::<usize>()).sum()
    };
    (poids, lourd)
}

/// La fusion se fait tout de suite, sur le fil de l'interface : moins d'une image d'écran (~16 ms, mesuré dans
/// `fusiontache_tests.rs` : une fusion de dessins légers prend moins d'une milliseconde, 100 tuiles pleines une dizaine), c'est-à-dire pas d'effet, d'aplat ni de
/// transformation, et peu de tuiles à composer. Sinon elle part en tâche de fond.
pub(crate) fn est_rapide(doc: &Document, index: usize, lis: &[usize]) -> bool {
    let (poids, lourd) = travail_de(doc, index, lis);
    !lourd && poids <= RAPIDE_MAX
}

/// Au plus ce nombre de tuiles (de 64 × 64 pixels) à composer pour une fusion immédiate.
pub(crate) const RAPIDE_MAX: usize = 100;
/// Le nombre d'images du plan que la fusion de `lis` parcourt (la progression de la tâche de fond).
pub(crate) fn images_a_calculer(doc: &Document, index: usize, lis: &[usize]) -> usize {
    let Some(panel) = doc.panels.get(index) else { return 1 };
    let lis = rangs_a_fusionner(&panel.layers, lis);
    let layers: Vec<&Layer> = lis.iter().map(|&l| &panel.layers[l]).collect();
    let mut n = layers.iter().map(|l| l.frames.len()).max().unwrap_or(1);
    if layers.iter().any(|l| l.pre != Behaviour::Nothing || l.post != Behaviour::Nothing) {
        n = n.max(panel.frame_count());
    }
    n.max(1)
}