//! Le travail réparti sur les cœurs donne le même résultat qu'une seule boucle : `par_map`,
//! l'écriture et la lecture des dessins d'un projet, `Raster::to_rgba` / `from_rgba`, l'ouverture
//! sur un autre fil.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::document::{Cel, Document, Layer, LayerKind};
use crate::parallele::par_map;
use crate::project;
use crate::raster::{premultiply, unpremultiply, Raster, TILE};

/// Un hasard reproductible.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
}

fn dossier(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("shot-parallele-{}-{nom}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("dossier");
    d
}

/// Un dessin de taille quelconque (les tuiles du bord sont coupées) : des pixels visibles ici et
/// là, certains à peine visibles, d'autres transparents.
fn dessin_au_hasard(w: usize, h: usize, seed: u64) -> Raster {
    let mut rng = Lcg(seed);
    let mut r = Raster::new(w, h);
    for _ in 0..(w * h / 40).max(1) {
        let (x, y) = (rng.next() as usize % w, rng.next() as usize % h);
        let (red, green, blue) = (rng.next() as u8, rng.next() as u8, rng.next() as u8);
        let alpha = [0u8, 1, 17, 128, 254, 255][rng.next() as usize % 6];
        let i = r.tile_index_of(x, y);
        r.tile_mut(i)[(y % TILE) * TILE + x % TILE] = premultiply(red, green, blue, alpha);
    }
    r
}

/// L'ancien calcul de `to_rgba` (un tableau plein, puis un octet à la fois).
fn to_rgba_reference(r: &Raster) -> Vec<u8> {
    r.to_flat().into_iter().flat_map(unpremultiply).collect()
}

/// L'ancien calcul de `from_rgba` (pixel par pixel).
fn from_rgba_reference(width: usize, height: usize, data: &[u8]) -> Raster {
    let mut raster = Raster::new(width, height);
    for idx in 0..raster.tiles.len() {
        let (ox, oy, tw, th) = raster.tile_bounds(idx);
        let mut tile: Option<crate::raster::Tile> = None;
        for y in 0..th {
            for x in 0..tw {
                let i = ((oy + y) * width + ox + x) * 4;
                if data[i + 3] == 0 {
                    continue;
                }
                let t = tile.get_or_insert_with(crate::raster::empty_tile);
                t[y * TILE + x] = premultiply(data[i], data[i + 1], data[i + 2], data[i + 3]);
            }
        }
        raster.tiles[idx] = tile;
    }
    raster
}

fn memes_tuiles(a: &Raster, b: &Raster) -> bool {
    a.width == b.width && a.height == b.height && a.tiles.len() == b.tiles.len() && a.tiles.iter().zip(&b.tiles).all(|(x, y)| match (x, y) {
        (None, None) => true,
        (Some(x), Some(y)) => x.iter().zip(y.iter()).all(|(p, q)| p == q),
        _ => false,
    })
}

#[test]
fn par_map_keeps_the_order_and_runs_everything() {
    for n in [0usize, 1, 2, 7, 100] {
        let items: Vec<usize> = (0..n).collect();
        let out = par_map(&items, 8, |i, &x| {
            // Des durées inégales, pour que les fils finissent dans le désordre.
            std::thread::sleep(std::time::Duration::from_micros(((x * 37) % 5) as u64 * 200));
            (i, x * x)
        });
        assert_eq!(out, items.iter().map(|&x| (x, x * x)).collect::<Vec<_>>());
    }
    // Un seul fil : même résultat.
    let items: Vec<u32> = (0..20).collect();
    assert_eq!(par_map(&items, 1, |_, &x| x + 1), (1..21).collect::<Vec<_>>());
}

#[test]
fn rgba_conversions_match_the_old_pixel_loops() {
    // Des tailles qui ne tombent pas sur des tuiles entières.
    for (k, (w, h)) in [(64, 64), (200, 130), (65, 63), (1, 1), (333, 70)].into_iter().enumerate() {
        let r = dessin_au_hasard(w, h, k as u64 + 1);
        let fast = r.to_rgba();
        assert_eq!(fast, to_rgba_reference(&r), "to_rgba {w}x{h}");
        let back = Raster::from_rgba(w, h, &fast);
        assert!(memes_tuiles(&back, &from_rgba_reference(w, h, &fast)), "from_rgba {w}x{h}");
        // Un dessin vide n'a aucune tuile.
        let blank = Raster::new(w, h);
        assert_eq!(blank.to_rgba(), vec![0u8; w * h * 4]);
        assert!(Raster::from_rgba(w, h, &blank.to_rgba()).tiles.iter().all(Option::is_none));
    }
    // Des octets RGBA quelconques (alpha à 0 avec des couleurs, qui ne comptent pas).
    let mut rng = Lcg(99);
    let data: Vec<u8> = (0..130 * 70 * 4).map(|i| if i % 4 == 3 && rng.next() % 3 == 0 { 0 } else { rng.next() as u8 }).collect();
    assert!(memes_tuiles(&Raster::from_rgba(130, 70, &data), &from_rgba_reference(130, 70, &data)));
}

/// Un petit projet : trois plans, des dessins bitmap et un vectoriel.
fn petit_projet() -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    for p in 0..doc.panels.len() {
        let mut bit = Layer::new("Bitmap", LayerKind::Bitmap);
        for f in 0..6 {
            if f % 2 == 0 {
                let id = doc.add_cel(Cel::bitmap(dessin_au_hasard(doc.width, doc.height, (p * 10 + f) as u64 + 1)));
                bit.frames.push(Some(id));
                bit.frames.push(Some(id));
            }
        }
        let fond = {
            let id = doc.add_cel(Cel::bitmap(dessin_au_hasard(doc.width, doc.height, 500 + p as u64)));
            let mut l = Layer::new("Fond", LayerKind::Bitmap);
            l.frames = vec![Some(id); 12];
            l
        };
        doc.panels[p].layers = vec![fond, bit];
        doc.panels[p].length = 12;
    }
    doc
}

fn tous_les_pixels(doc: &Document) -> HashMap<u64, Vec<u8>> {
    doc.used_cels().into_iter().map(|id| (id, doc.cels[&id].raster.to_rgba())).collect()
}

#[test]
fn parallel_save_and_load_round_trip_a_project() {
    let doc = petit_projet();
    let dir = dossier("aller-retour");
    let manifeste = dir.join("Essai.pelure");
    let mut written = HashMap::new();
    let report = project::save(&doc, &manifeste, &mut written).expect("enregistrement");
    assert_eq!(report.written, doc.used_cels().len());
    // Les fichiers sont ceux que l'ancienne boucle aurait écrits : le PNG de `encode_png(to_rgba)`.
    for id in doc.used_cels() {
        let r = &doc.cels[&id].raster;
        let want = crate::io::encode_png(&to_rgba_reference(r), r.width, r.height).expect("png");
        let got = std::fs::read(dir.join("dessins").join(format!("{id}.png"))).expect("fichier");
        assert_eq!(got, want, "dessin {id}");
    }
    let l = project::load(&manifeste).expect("lecture");
    assert!(l.warnings.is_empty(), "{:?}", l.warnings);
    assert_eq!(l.doc.panels.len(), doc.panels.len());
    assert_eq!(tous_les_pixels(&l.doc), tous_les_pixels(&doc));
    // Rien n'a changé : le second enregistrement n'écrit rien.
    let again = project::save(&doc, &manifeste, &mut written).expect("deuxième enregistrement");
    assert_eq!(again.written, 0);
    // Un dessin effacé du dossier est réécrit (la liste du dossier est lue, pas supposée).
    let id = *doc.used_cels().iter().next().unwrap();
    std::fs::remove_file(dir.join("dessins").join(format!("{id}.png"))).unwrap();
    let third = project::save(&doc, &manifeste, &mut written).expect("troisième enregistrement");
    assert_eq!(third.written, 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_drawings_are_reported_in_identifier_order() {
    let doc = petit_projet();
    let dir = dossier("manquants");
    let manifeste = dir.join("Essai.pelure");
    project::save(&doc, &manifeste, &mut HashMap::new()).expect("enregistrement");
    let ids: Vec<u64> = doc.used_cels().into_iter().collect();
    // Trois dessins perdus, pris à des rangs éloignés.
    let perdus = [ids[ids.len() - 1], ids[1], ids[ids.len() / 2]];
    for id in perdus {
        std::fs::remove_file(dir.join("dessins").join(format!("{id}.png"))).unwrap();
    }
    let l = project::load(&manifeste).expect("lecture");
    assert_eq!(l.warnings.len(), 3, "{:?}", l.warnings);
    let mut sorted = perdus.to_vec();
    sorted.sort_unstable();
    for (w, id) in l.warnings.iter().zip(sorted) {
        assert!(w.contains(&format!("dessin {id} ")), "{w} ne parle pas du dessin {id}");
    }
    // Les images qui les montraient n'ont plus de dessin.
    for id in perdus {
        assert!(l.doc.panels.iter().flat_map(|p| &p.layers).all(|ly| !ly.frames.contains(&Some(id))));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn opening_a_project_does_not_block_the_interface() {
    let doc = petit_projet();
    let dir = dossier("ouverture");
    let manifeste = dir.join("Essai.pelure");
    project::save(&doc, &manifeste, &mut HashMap::new()).expect("enregistrement");
    let mut b = crate::perf_tests::banc(Document::with_frame(160, 90, 24), false);
    b.app.open_path(manifeste.clone());
    // Le fil est parti, le projet d'avant est encore là, et l'interface en tient compte.
    assert!(b.app.open_job.is_some(), "l'ouverture doit se faire sur un autre fil");
    assert_eq!(b.app.doc.panels.len(), 1);
    assert!(b.app.modal_open());
    for _ in crate::perf_tests::attente() {
        b.image(Vec::new(), 1.0 / 60.0);
        if b.app.open_job.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(b.app.open_job.is_none(), "le projet doit finir par être adopté");
    assert_eq!(b.app.doc.panels.len(), 3);
    assert_eq!(b.app.path.as_deref(), Some(manifeste.as_path()));
    assert!(!b.app.modal_open());
    assert_eq!(tous_les_pixels(&b.app.doc).len(), tous_les_pixels(&doc).len());
    // Un projet qui n'existe pas : un message, pas de plantage.
    b.app.open_path(dir.join("absent").join("Rien.pelure"));
    for _ in crate::perf_tests::attente() {
        b.image(Vec::new(), 1.0 / 60.0);
        if b.app.open_job.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(b.app.status.starts_with("Impossible d'ouvrir le projet"), "{}", b.app.status);
    assert_eq!(b.app.doc.panels.len(), 3, "le projet ouvert reste en place");
    let _ = std::fs::remove_dir_all(&dir);
}

/// L'enregistrement en parallèle écrit le fichier des commentaires, l'ouverture sur un autre fil le
/// relit, et les dessins dérivés des effets de l'ancien projet sont oubliés (`App::load_doc`).
#[test]
fn parallel_save_and_open_keep_the_comments_and_forget_the_effects() {
    let mut doc = petit_projet();
    let (premier, dernier) = (doc.panels[0].id, doc.panels[2].id);
    let a = doc.add_comment(premier, 3, "Anthony", 1_760_000_000);
    let b_id = doc.add_comment(dernier, 11, "Léa", 1_760_000_100);
    doc.comment_mut(a).unwrap().text = "Trop long.".into();
    doc.comment_mut(b_id).unwrap().done = true;
    let dir = dossier("commentaires");
    let manifeste = dir.join("Essai.pelure");
    project::save(&doc, &manifeste, &mut HashMap::new()).expect("enregistrement");
    assert!(dir.join(crate::commentaires::FILE).exists(), "commentaires.json est écrit à côté du manifeste");
    let mut b = crate::perf_tests::banc(Document::with_frame(160, 90, 24), false);
    let epoque = crate::fxcache::epoch();
    b.app.open_path(manifeste.clone());
    for _ in crate::perf_tests::attente() {
        b.image(Vec::new(), 1.0 / 60.0);
        if b.app.open_job.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(b.app.open_job.is_none(), "le projet doit finir par être adopté");
    assert_eq!(b.app.doc.comments, doc.comments);
    assert!(crate::fxcache::epoch() != epoque, "les effets de l'ancien projet sont oubliés");
    let _ = std::fs::remove_dir_all(&dir);
}
