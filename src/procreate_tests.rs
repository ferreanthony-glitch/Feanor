//! Brosses Procreate : liste de propriétés binaire et archive fabriquées ici (aucune vraie
//! brosse sur le poste).

use crate::brush::Source;
use crate::brushtex::BrushLibrary;
use crate::procreate::{import, keyed_root, read_bplist, Plist};

/// Un objet à écrire dans une liste de propriétés binaire (les conteneurs désignent d'autres
/// objets par leur rang).
enum W {
    Str(&'static str),
    Real(f64),
    Int(i64),
    Uid(u8),
    Arr(Vec<usize>),
    Dict(Vec<(usize, usize)>),
}

/// Une liste de propriétés binaire (« bplist00 ») faite de `nodes`, l'objet `top` en haut.
fn bplist(nodes: &[W], top: usize) -> Vec<u8> {
    let mut out = b"bplist00".to_vec();
    let mut offsets = Vec::new();
    for n in nodes {
        offsets.push(out.len() as u16);
        match n {
            W::Str(s) => {
                if s.len() < 15 {
                    out.push(0x50 | s.len() as u8);
                } else {
                    out.extend_from_slice(&[0x5F, 0x10, s.len() as u8]);
                }
                out.extend_from_slice(s.as_bytes());
            }
            W::Real(v) => {
                out.push(0x23);
                out.extend_from_slice(&v.to_be_bytes());
            }
            W::Int(v) => {
                out.push(0x13);
                out.extend_from_slice(&v.to_be_bytes());
            }
            W::Uid(u) => out.extend_from_slice(&[0x80, *u]),
            W::Arr(items) => {
                out.push(0xA0 | items.len() as u8);
                out.extend(items.iter().map(|&i| i as u8));
            }
            W::Dict(items) => {
                out.push(0xD0 | items.len() as u8);
                out.extend(items.iter().map(|(k, _)| *k as u8));
                out.extend(items.iter().map(|(_, v)| *v as u8));
            }
        }
    }
    let table = out.len() as u64;
    for o in &offsets {
        out.extend_from_slice(&o.to_be_bytes());
    }
    out.extend_from_slice(&[0, 0, 0, 0, 0, 0, 2, 1]);
    out.extend_from_slice(&(nodes.len() as u64).to_be_bytes());
    out.extend_from_slice(&(top as u64).to_be_bytes());
    out.extend_from_slice(&table.to_be_bytes());
    out
}

/// Le `Brush.archive` d'une brosse nommée `name` (NSKeyedArchiver).
fn archive(name: &'static str) -> Vec<u8> {
    use W::*;
    let nodes = vec![
        /* 0 */ Dict(vec![(1, 2), (3, 4), (5, 6)]),
        /* 1 */ Str("$objects"),
        /* 2 */ Arr(vec![7, 8, 9, 10, 11, 12]),
        /* 3 */ Str("$top"),
        /* 4 */ Dict(vec![(13, 14)]),
        /* 5 */ Str("$archiver"),
        /* 6 */ Str("NSKeyedArchiver"),
        // $objects : 0 "$null", 1 la racine, puis ses valeurs.
        /* 7 */ Str("$null"),
        /* 8 */ Dict(vec![(15, 16), (17, 18), (19, 20), (21, 22), (23, 24), (25, 26)]),
        /* 9 */ Str(name),
        /* 10 */ Real(0.25),
        /* 11 */ Real(0.6),
        /* 12 */ Int(3),
        /* 13 */ Str("root"),
        /* 14 */ Uid(1),
        /* 15 */ Str("name"),
        /* 16 */ Uid(2),
        /* 17 */ Str("paintSize"),
        /* 18 */ Uid(3),
        /* 19 */ Str("dynamicsPressureSize"),
        /* 20 */ Uid(4),
        /* 21 */ Str("shapeCount"),
        /* 22 */ Uid(5),
        /* 23 */ Str("bundledShapePath"),
        /* 24 */ Uid(0),
        /* 25 */ Str("plotSpacing"),
        /* 26 */ Real(0.15),
    ];
    bplist(&nodes, 0)
}

#[test]
fn a_binary_plist_and_its_keyed_archive_are_read() {
    let p = read_bplist(&archive("Gouache sèche")).unwrap();
    let keys = keyed_root(&p).unwrap();
    assert_eq!(keys.get("name"), Some(&Plist::Str("Gouache sèche".into())));
    assert_eq!(keys.get("paintSize").and_then(Plist::num), Some(0.25));
    assert_eq!(keys.get("shapeCount").and_then(Plist::num), Some(3.0));
    assert_eq!(keys.get("bundledShapePath"), Some(&Plist::Null), "$null : absent");
    assert_eq!(keys.get("plotSpacing").and_then(Plist::num), Some(0.15), "valeur rangée sans renvoi");
    assert!(read_bplist(b"pas une liste").is_err());
}

#[test]
fn a_brushset_becomes_brushes_with_shape_and_grain() {
    let mut png = Vec::new();
    image::DynamicImage::ImageLuma8(image::GrayImage::from_fn(8, 8, |x, _| image::Luma([if x < 4 { 255 } else { 0 }])))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let a = archive("Craie");
    let b = archive("Fusain");
    let set = crate::kritabrush_tests::zip(&[
        ("brushset.plist", b"x"),
        ("AAAA/Brush.archive", &a),
        ("AAAA/Shape.png", &png),
        ("AAAA/Grain.png", &png),
        ("AAAA/Reset/Brush.archive", &a),
        ("BBBB/Brush.archive", &b),
    ]);
    let mut lib = BrushLibrary::load(None);
    let (n, tips, papers, notes) = import(&set, "jeu.brushset", &mut lib).unwrap();
    assert_eq!((n, tips, papers), (2, 1, 1), "les réglages d'usine (Reset) ne comptent pas");
    // (Pelure a maintenant ses propres « Craie » et « Fusain » : on cherche ceux du jeu importé.)
    let craie = lib.presets.iter().find(|p| p.name == "Craie" && p.source == "jeu.brushset").unwrap().settings;
    assert!(craie.tip.is_some() && craie.paper.is_some());
    assert_eq!((craie.count, craie.dynamics.size.source), (3, Source::Pressure));
    assert!((craie.dynamics.size.min - 0.4).abs() < 1e-5 && (craie.spacing - 0.15).abs() < 1e-6);
    let fusain = lib.presets.iter().find(|p| p.name == "Fusain" && p.source == "jeu.brushset").unwrap().settings;
    assert!(fusain.tip.is_none() && fusain.grain == 0.0);
    assert!(notes[0].contains("devinées"));
    assert!(import(&crate::kritabrush_tests::zip(&[("x.txt", b"x")]), "x", &mut lib).is_err());
}
