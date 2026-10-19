//! Les textures de dessin n'ont que la place du contenu (`gpu.rs`, `texture_rect`) : la carte
//! montre les mêmes pixels qu'avec une texture de la feuille entière, y compris quand un dessin
//! grandit, quand une texture est recyclée pour un autre dessin, pour un aperçu réduit d'effet, une
//! feuille flottante déformée ou un calque transformé ; et le cache garde bien plus de dessins.

use eframe::egui::{vec2, Color32};

use crate::affichage_tests::line_item;
use crate::blend::BlendMode;
use crate::document::{Cel, CelId, Document, LayerKind, LayerTransform};
use crate::envelope::Envelope;
use crate::flottante::Flottante;
use crate::gpu::{content_pixels, content_tiles, covers, texture_rect, GpuCanvas};
use crate::gpu_tests::{headless, render_with, render_with_tiles, scene};
use crate::raster::TILE;
use crate::render::{self, Plan};

/// Un dessin dont les tuiles `[tx0, tx0 + nx) x [ty0, ty0 + ny)` sont pleines, de couleurs qui
/// changent d'une tuile à l'autre (et un peu transparentes : le lissage a de quoi se voir).
fn bloc(doc: &mut Document, tx0: usize, ty0: usize, nx: usize, ny: usize, seed: u8) -> CelId {
    let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
    remplir(&mut cel, tx0, ty0, nx, ny, seed);
    doc.add_cel(cel)
}

fn remplir(cel: &mut Cel, tx0: usize, ty0: usize, nx: usize, ny: usize, seed: u8) {
    let across = cel.raster.tiles_x;
    for j in 0..ny {
        for i in 0..nx {
            let tile = cel.raster.tile_mut((ty0 + j) * across + tx0 + i);
            for (k, p) in tile.iter_mut().enumerate() {
                let v = (k % 64) as u8 * 2 + (i + j) as u8 * 20 + seed;
                *p = Color32::from_rgba_premultiplied(v / 3, 90u8.wrapping_add(seed), 200u8.wrapping_sub(v / 4), 220);
            }
        }
    }
    cel.version += 1;
}

fn plan(items: Vec<crate::render::PlanItem>) -> Plan {
    Plan { paper: Some([255, 255, 255]), items }
}

/// Pixels qui diffèrent de plus de `tol` niveaux.
fn ecarts(a: &[Color32], b: &[Color32], tol: i32) -> usize {
    a.iter().zip(b).filter(|(x, y)| x.to_array().iter().zip(y.to_array()).any(|(p, q)| (*p as i32 - q as i32).abs() > tol)).count()
}

#[test]
fn a_texture_covers_its_content_with_a_transparent_margin_and_stays_in_the_sheet() {
    let dims = (640u32, 360u32);
    // Un dessin vide : un pixel.
    assert_eq!(texture_rect(dims, None, 0), [0, 0, 1, 1]);
    assert!(covers(texture_rect(dims, None, 0), dims, None));
    // Une tuile au milieu : un pixel de marge de chaque côté, taille arrondie à 128.
    let r = texture_rect(dims, Some([3, 2, 3, 2]), 0);
    assert_eq!(r, [191, 127, 128, 128]);
    // Toutes les positions et tailles de blocs : la texture reste dans la feuille, couvre le
    // contenu, et laisse un pixel de transparent aux bords qui ne sont pas ceux de la feuille.
    let across = 10;
    let down = 6;
    for tx0 in 0..across {
        for ty0 in 0..down {
            for (nx, ny) in [(1, 1), (2, 3), (4, 2), (10, 6), (3, 6), (10, 1)] {
                let (tx1, ty1) = ((tx0 + nx - 1).min(across - 1), (ty0 + ny - 1).min(down - 1));
                let tiles = [tx0, ty0, tx1, ty1];
                for slack in [0, 2 * TILE] {
                    let r = texture_rect(dims, Some(tiles), slack);
                    assert!(r[0] + r[2] <= dims.0 && r[1] + r[3] <= dims.1, "{tiles:?} +{slack} : {r:?} sort de la feuille");
                    assert!(r[2] >= 1 && r[3] >= 1);
                    assert!(covers(r, dims, Some(content_pixels(dims, tiles))), "{tiles:?} +{slack} : {r:?} ne couvre pas son contenu");
                    // Un contenu qui touche le bord de la texture sans toucher celui de la feuille ne serait pas couvert.
                    if r[0] > 0 {
                        assert!(!covers([r[0] + 1, r[1], r[2] - 1, r[3]], dims, Some(content_pixels(dims, tiles))) || content_pixels(dims, tiles)[0] > (r[0] + 1) as usize);
                    }
                }
            }
        }
    }
    // Un contenu qui sort de la texture n'est plus couvert.
    let r = texture_rect(dims, Some([3, 2, 3, 2]), 0);
    assert!(!covers(r, dims, Some(content_pixels(dims, [3, 2, 4, 2]))));
    assert!(!covers(r, dims, Some(content_pixels(dims, [2, 2, 3, 2]))));
}

#[test]
fn content_sized_textures_render_exactly_like_whole_sheet_textures() {
    let Some(rs) = headless() else {
        println!("pas de carte graphique : essai passé");
        return;
    };
    let (w, h) = (640, 360);
    let mut doc = Document::new(w, h, 24);
    let middle = bloc(&mut doc, 3, 2, 3, 2, 10);
    let corner = bloc(&mut doc, 0, 0, 2, 2, 60);
    let far = bloc(&mut doc, 8, 4, 2, 2, 110);
    let full = bloc(&mut doc, 0, 0, 10, 6, 150);
    // Un aperçu réduit (comme ceux des effets) : une image moitié moins grande, montrée en entier.
    let mut small = Cel::new(w / 2, h / 2, LayerKind::Bitmap);
    remplir(&mut small, 1, 1, 2, 1, 200);
    let small = doc.add_cel(small);
    let mut gpu = GpuCanvas::new(&rs);
    let mut check = |nom: &str, plan: Plan, gain: bool| {
        gpu.set_whole_textures(true);
        let before = render_with(&mut gpu, &rs, &doc, &plan, true);
        let whole = gpu.stats();
        gpu.set_whole_textures(false);
        let after = render_with(&mut gpu, &rs, &doc, &plan, true);
        let sized = gpu.stats();
        let far = ecarts(&after, &before, 1);
        println!("{nom} : {far} pixels diffèrent, textures {} Ko contre {} Ko", sized.bytes / 1024, whole.bytes / 1024);
        assert_eq!(far, 0, "{nom} : {far} pixels diffèrent de la texture de la feuille entière");
        assert!(after.iter().filter(|c| **c != Color32::WHITE).count() > 1_000, "{nom} : l'image est vide");
        if gain {
            assert!(sized.bytes < whole.bytes / 2, "{nom} : {} contre {} octets", sized.bytes, whole.bytes);
        }
    };
    check("un bloc au milieu", plan(vec![line_item(middle)]), true);
    check("des blocs au coin, loin, au milieu", plan(vec![line_item(corner), line_item(middle), line_item(far)]), true);
    check("un dessin qui remplit la feuille", plan(vec![line_item(full)]), false);
    let moved = |id, t: LayerTransform| {
        let mut it = line_item(id);
        it.transform = Some(t);
        it
    };
    // Calques transformés : le lissage lit les voisins du bord de la texture (transparents).
    let t = LayerTransform { x: 40.5, y: -25.25, scale: 0.8, angle: 17.0, ..Default::default() };
    check("transformés", plan(vec![moved(middle, t), moved(corner, t), moved(far, LayerTransform { scale: 1.6, angle: -30.0, ..Default::default() })]), true);
    check("à moitié pixel", plan(vec![moved(middle, LayerTransform { x: 0.5, y: 0.5, ..Default::default() })]), true);
    check("rempli, agrandi, tourné", plan(vec![moved(full, LayerTransform { scale: 1.2, angle: 5.0, ..Default::default() })]), false);
    // Opacité et modes de fusion.
    let mut blended = line_item(middle);
    blended.blend = BlendMode::Multiply;
    blended.opacity = 180;
    let mut screen = line_item(corner);
    screen.blend = BlendMode::Screen;
    check("modes de fusion", plan(vec![line_item(full), blended, screen]), false);
    // Aperçu réduit, montré sur toute la feuille, puis transformé.
    check("aperçu réduit", plan(vec![line_item(small)]), false);
    check("aperçu réduit transformé", plan(vec![moved(small, LayerTransform { x: 12.0, scale: 0.9, angle: 8.0, ..Default::default() })]), false);
}

#[test]
fn content_sized_textures_render_a_warped_floating_sheet_like_whole_ones() {
    let Some(rs) = headless() else {
        println!("pas de carte graphique : essai passé");
        return;
    };
    let (w, h) = (960, 540);
    let (mut doc, id, elements, src) = scene(w, h);
    let items: Vec<usize> = (0..elements.len()).collect();
    let mut f = Flottante::lift(&mut doc, id, &items).expect("feuille");
    let mut gpu = GpuCanvas::new(&rs);
    let mut check = |nom: &str, f: &Flottante, doc: &Document| {
        let plan = plan(f.items(&line_item(id)).to_vec());
        gpu.set_whole_textures(true);
        let before = render_with(&mut gpu, &rs, doc, &plan, true);
        gpu.set_whole_textures(false);
        let after = render_with(&mut gpu, &rs, doc, &plan, true);
        let cpu = render::composite_full(doc, &plan);
        let far = ecarts(&after, &before, 1);
        println!("{nom} : {far} pixels diffèrent de la texture entière, {} du calcul de secours", ecarts(&after, &cpu, 4));
        // Même plan, même maillage : l'image composée gardée est la bonne.
        let composes = gpu.stats().composes;
        let kept = render_with_tiles(&mut gpu, &rs, doc, &plan, false, &[]);
        assert_eq!(gpu.stats().composes, composes, "{nom} : recomposé sans raison");
        assert_eq!(kept, after, "{nom} : l'image gardée diffère");
        assert_eq!(far, 0, "{nom} : {far} pixels diffèrent de la texture de la feuille entière");
        assert!(after.iter().filter(|c| c.g() < 200).count() > 3_000, "{nom} : vide");
    };
    f.aff = crate::vector::Affine::translate(vec2(30.0, 12.0));
    check("feuille flottante déplacée", &f, &doc);
    f.aff = crate::vector::Affine::IDENTITY;
    let mut env = Envelope::identity(src);
    env.move_corner(2, vec2(-70.0, 40.0));
    env.move_corner(0, vec2(30.0, 20.0));
    env.handles[1] += vec2(0.0, 50.0);
    assert!(f.set_warp(src, 8, |p| env.map(src, p)));
    check("feuille flottante en enveloppe", &f, &doc);
}

#[test]
fn a_drawing_that_grows_beyond_its_texture_is_uploaded_again_without_clipping() {
    let Some(rs) = headless() else {
        println!("pas de carte graphique : essai passé");
        return;
    };
    let (w, h) = (640, 360);
    let mut doc = Document::new(w, h, 24);
    let id = bloc(&mut doc, 4, 3, 1, 1, 30);
    let plan = plan(vec![line_item(id)]);
    let mut gpu = GpuCanvas::new(&rs);
    let first = render_with(&mut gpu, &rs, &doc, &plan, false);
    assert_eq!(ecarts(&first, &render::composite_full(&doc, &plan), 1), 0);
    let small = gpu.stats().bytes;
    assert!(small <= 128 * 128 * 4, "un dessin d'une tuile : {small} octets");
    // 1) Le dessin change de version et sort de sa texture (autre version : le chemin du dessin entier).
    let across = doc.cels[&id].raster.tiles_x;
    {
        let cel = doc.cels.get_mut(&id).unwrap();
        remplir(cel, 2, 1, 1, 1, 90);
    }
    let image = render_with(&mut gpu, &rs, &doc, &plan, false);
    assert_eq!(ecarts(&image, &render::composite_full(&doc, &plan), 1), 0, "après la sortie par une nouvelle version");
    let grown = gpu.stats().bytes;
    assert!(grown > small, "la texture a grandi : {small} -> {grown}");
    // 2) Un trait qui s'allonge tuile par tuile, par les tuiles touchées (la version ne change pas,
    // comme pendant qu'on peint) : chaque image est exacte. Les premières tuiles sont dans la marge
    // gardée, les suivantes la dépassent.
    let mut regrown = 0;
    let mut last = gpu.stats().bytes;
    for step in 0..9 {
        let idx = 3 * across + step;
        {
            let cel = doc.cels.get_mut(&id).unwrap();
            let version = cel.version;
            let tile = cel.raster.tile_mut(idx);
            for (k, p) in tile.iter_mut().enumerate() {
                *p = Color32::from_rgba_premultiplied(200, (k % 200) as u8 / 2, 20, 255);
            }
            cel.version = version;
        }
        let image = render_with_tiles(&mut gpu, &rs, &doc, &plan, false, &[idx]);
        let far = ecarts(&image, &render::composite_full(&doc, &plan), 1);
        assert_eq!(far, 0, "tuile {step} posée en cours de trait : {far} pixels faux");
        let now = gpu.stats().bytes;
        if now != last {
            regrown += 1;
        }
        last = now;
    }
    assert!(regrown >= 1 && regrown <= 4, "la texture ne se refait pas à chaque tuile ({regrown} fois sur 9)");
    // 3) Un trait effacé : les tuiles vidées disparaissent aussi de l'écran.
    {
        let cel = doc.cels.get_mut(&id).unwrap();
        let version = cel.version;
        for step in 0..9 {
            cel.raster.tiles[3 * across + step] = None;
        }
        cel.version = version;
    }
    let tiles: Vec<usize> = (0..9).map(|s| 3 * across + s).collect();
    let image = render_with_tiles(&mut gpu, &rs, &doc, &plan, false, &tiles);
    assert_eq!(ecarts(&image, &render::composite_full(&doc, &plan), 1), 0, "après l'effacement du trait");
    // 4) Plus rien dans le dessin : la carte non plus.
    {
        let cel = doc.cels.get_mut(&id).unwrap();
        for t in cel.raster.tiles.iter_mut() {
            *t = None;
        }
        cel.version += 1;
    }
    let image = render_with(&mut gpu, &rs, &doc, &plan, false);
    assert!(image.iter().all(|c| *c == Color32::WHITE), "dessin vidé : du papier seulement");
    // 5) Et il se remplit à nouveau, ailleurs.
    {
        let cel = doc.cels.get_mut(&id).unwrap();
        remplir(cel, 6, 1, 2, 2, 5);
    }
    let image = render_with(&mut gpu, &rs, &doc, &plan, false);
    assert_eq!(ecarts(&image, &render::composite_full(&doc, &plan), 1), 0, "rempli de nouveau");
}

#[test]
fn a_drawing_already_on_the_card_is_not_sent_again() {
    let Some(rs) = headless() else {
        println!("pas de carte graphique : essai passé");
        return;
    };
    let (w, h) = (640, 360);
    let mut doc = Document::new(w, h, 24);
    let a = bloc(&mut doc, 1, 1, 3, 2, 10);
    let b = bloc(&mut doc, 5, 2, 2, 2, 90);
    let c = bloc(&mut doc, 0, 0, 10, 6, 150);
    let mut gpu = GpuCanvas::new(&rs);
    let p = plan(vec![line_item(a), line_item(b), line_item(c)]);
    render_with(&mut gpu, &rs, &doc, &p, false);
    let first = gpu.stats().copies;
    assert!(first >= 6 + 4 + 60, "premier envoi : {first} tuiles");
    for _ in 0..3 {
        render_with(&mut gpu, &rs, &doc, &p, false);
    }
    assert_eq!(gpu.stats().copies, first, "rien n'a changé : rien n'est renvoyé");
    // Un autre plan, avec un dessin déjà là et un neuf.
    let d = bloc(&mut doc, 2, 2, 1, 1, 33);
    render_with(&mut gpu, &rs, &doc, &plan(vec![line_item(a), line_item(d)]), false);
    assert_eq!(gpu.stats().copies, first + 1, "seul le dessin neuf est envoyé");
}

#[test]
fn the_composed_image_is_kept_while_nothing_changes_and_redone_when_anything_does() {
    let Some(rs) = headless() else {
        println!("pas de carte graphique : essai passé");
        return;
    };
    let (w, h) = (640, 360);
    let mut doc = Document::new(w, h, 24);
    let a = bloc(&mut doc, 1, 1, 3, 2, 10);
    let b = bloc(&mut doc, 5, 2, 2, 2, 90);
    let mut gpu = GpuCanvas::new(&rs);
    let mut p = plan(vec![line_item(a), line_item(b)]);
    let exact = |gpu: &mut GpuCanvas, doc: &Document, p: &Plan, tiles: &[usize], what: &str| {
        let image = render_with_tiles(gpu, &rs, doc, p, false, tiles);
        assert_eq!(ecarts(&image, &render::composite_full(doc, p), 1), 0, "{what} : l'image n'est pas celle du calcul de secours");
        image
    };
    let first = exact(&mut gpu, &doc, &p, &[], "premier affichage");
    assert_eq!(gpu.stats().composes, 1);
    // La souris bouge, les panneaux s'animent : plan et dessins sont les mêmes, rien n'est recomposé.
    for _ in 0..4 {
        let again = exact(&mut gpu, &doc, &p, &[], "même plan");
        assert_eq!(again, first);
    }
    assert_eq!(gpu.stats().composes, 1, "la pile de calques n'est pas recomposée à chaque image");
    // Chaque changement refait l'image.
    p.items[1].opacity = 120;
    exact(&mut gpu, &doc, &p, &[], "opacité");
    assert_eq!(gpu.stats().composes, 2);
    p.items[0].transform = Some(LayerTransform { x: 10.5, scale: 0.9, ..Default::default() });
    exact(&mut gpu, &doc, &p, &[], "transformation");
    assert_eq!(gpu.stats().composes, 3);
    p.items[1].blend = BlendMode::Multiply;
    exact(&mut gpu, &doc, &p, &[], "mode de fusion");
    assert_eq!(gpu.stats().composes, 4);
    // Un trait : tuile signalée, même version.
    let across = doc.cels[&a].raster.tiles_x;
    let idx = 2 * across + 2;
    {
        let cel = doc.cels.get_mut(&a).unwrap();
        let version = cel.version;
        for (k, px) in cel.raster.tile_mut(idx).iter_mut().enumerate() {
            *px = Color32::from_rgba_premultiplied(250, (k % 90) as u8, 0, 255);
        }
        cel.version = version;
    }
    exact(&mut gpu, &doc, &p, &[idx], "tuile posée en cours de trait");
    assert_eq!(gpu.stats().composes, 5);
    // Une nouvelle version du dessin.
    remplir(doc.cels.get_mut(&b).unwrap(), 6, 4, 1, 1, 200);
    exact(&mut gpu, &doc, &p, &[], "nouvelle version");
    assert_eq!(gpu.stats().composes, 6);
    // Un dessin de plus.
    let c = bloc(&mut doc, 0, 4, 1, 1, 40);
    p.items.push(line_item(c));
    exact(&mut gpu, &doc, &p, &[], "dessin de plus");
    assert_eq!(gpu.stats().composes, 7);
    // Et de nouveau stable.
    exact(&mut gpu, &doc, &p, &[], "stable");
    exact(&mut gpu, &doc, &p, &[], "stable");
    assert_eq!(gpu.stats().composes, 7);
    // Une taille de feuille différente refait l'image composée (nouvelle texture).
    let mut small = Document::new(320, 180, 24);
    let s = bloc(&mut small, 1, 1, 2, 1, 5);
    let sp = plan(vec![line_item(s)]);
    let image = render_with(&mut gpu, &rs, &small, &sp, false);
    assert_eq!(ecarts(&image, &render::composite_full(&small, &sp), 1), 0);
    assert_eq!(gpu.stats().composes, 8);
}

#[test]
fn recycled_content_sized_textures_show_only_their_own_drawing() {
    let Some(rs) = headless() else {
        println!("pas de carte graphique : essai passé");
        return;
    };
    let (w, h) = (640, 360);
    let mut doc = Document::new(w, h, 24);
    // Des blocs de même taille à des places différentes (donc des textures de même taille,
    // d'origines différentes : recyclables), et quelques-uns d'autres tailles.
    let cels: Vec<CelId> = (0..14usize)
        .map(|k| {
            let (nx, ny) = if k % 5 == 4 { (3, 2) } else { (2, 2) };
            bloc(&mut doc, k % (10 - nx + 1), (k * 2) % (6 - ny + 1), nx, ny, (k * 17) as u8)
        })
        .collect();
    let mut gpu = GpuCanvas::new(&rs);
    // Le budget tient trois textures : à partir de la quatrième, une ancienne est recyclée.
    gpu.set_cache_bytes(3 * 128 * 192 * 4);
    let mut checked = 0;
    let mut seen = std::collections::BTreeSet::new();
    for round in 0..3 {
        for step in 0..cels.len() {
            let k = if round == 1 { cels.len() - 1 - step } else { (step * 3) % cels.len() };
            let p = plan(vec![line_item(cels[k])]);
            let on_gpu = render_with(&mut gpu, &rs, &doc, &p, false);
            let far = ecarts(&on_gpu, &render::composite_full(&doc, &p), 1);
            assert_eq!(far, 0, "tour {round}, dessin {k} : {far} pixels diffèrent de la composition sur processeur");
            seen.insert(gpu.stats().textures);
            checked += 1;
        }
    }
    assert_eq!(checked, 42);
    let stats = gpu.stats();
    assert!(stats.textures <= 4, "le cache tient dans son budget : {} textures", stats.textures);
}

#[test]
fn the_cache_keeps_many_more_content_sized_drawings() {
    let Some(rs) = headless() else {
        println!("pas de carte graphique : essai passé");
        return;
    };
    let (w, h) = (1920, 1080);
    let mut doc = Document::new(w, h, 24);
    // Quarante dessins au trait de 8 x 5 tuiles, un peu partout sur la feuille.
    let cels: Vec<CelId> = (0..40usize).map(|k| bloc(&mut doc, (k * 3) % 21, (k * 2) % 12, 8, 5, k as u8)).collect();
    let sheet = (w * h * 4) as u64;
    let mut gpu = GpuCanvas::new(&rs);
    // Un budget de quatre feuilles entières.
    gpu.set_cache_bytes(4 * sheet);
    for &id in &cels {
        render_with(&mut gpu, &rs, &doc, &plan(vec![line_item(id)]), false);
    }
    let kept = gpu.stats();
    let per = kept.bytes / kept.textures.max(1) as u64;
    println!("{} dessins gardés, {} Ko chacun, au lieu de {} Ko (feuille entière) : {} fois plus de dessins", kept.textures, per / 1024, sheet / 1024, sheet / per.max(1));
    assert!(kept.textures >= 16, "le cache garde {} dessins (quatre avec la feuille entière)", kept.textures);
    assert!(kept.bytes <= 4 * sheet + sheet / 2);
    assert!(per * 5 < sheet, "chaque texture pèse {per} octets pour une feuille de {sheet}");
    // Le contenu est le même que pour le calcul de secours.
    let p = plan(vec![line_item(cels[7]), line_item(cels[19])]);
    let image = render_with(&mut gpu, &rs, &doc, &p, false);
    assert_eq!(ecarts(&image, &render::composite_full(&doc, &p), 1), 0);
    // (Le contenu de chaque dessin tient dans sa texture.)
    for &id in &cels {
        let raster = &doc.cels[&id].raster;
        let tiles = content_tiles(raster).expect("contenu");
        assert!(content_pixels((w as u32, h as u32), tiles)[2] > 0);
    }
}
