//! Essais du rangement des tuiles (`tuilepack.rs`) : les mêmes pixels reviennent, au bit près.

use eframe::egui::Color32;

use crate::raster::TILE_PIXELS;
use crate::tuilepack::{pack, unpack, MAX_OCTETS};

fn graine(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

fn tuile_de(f: impl FnMut(usize) -> Color32) -> Box<[Color32; TILE_PIXELS]> {
    let v: Vec<Color32> = (0..TILE_PIXELS).map(f).collect();
    v.into_boxed_slice().try_into().unwrap()
}

#[test]
fn a_tile_comes_back_bit_for_bit() {
    let mut s = 0x9E37_79B9_7F4A_7C15u64;
    let gris = |i: usize| Color32::from_rgba_premultiplied((i % 251) as u8, (i / 251) as u8, 7, 255);
    let cas: Vec<(&str, bool, Box<[Color32; TILE_PIXELS]>)> = vec![
        ("transparente", true, tuile_de(|_| Color32::TRANSPARENT)),
        ("aplat", true, tuile_de(|_| Color32::from_rgba_premultiplied(200, 10, 30, 255))),
        ("deux séries", true, tuile_de(|i| if i < 1000 { Color32::BLACK } else { Color32::WHITE })),
        ("trait fin", true, tuile_de(|i| if i % 64 == 31 || i % 64 == 32 { Color32::from_rgba_premultiplied(9, 9, 9, 200) } else { Color32::TRANSPARENT })),
        ("séries de 2 entre des pixels seuls (ne se range pas)", false,tuile_de(|i| if i % 5 < 2 { Color32::BLACK } else if i % 5 == 2 { Color32::RED } else { Color32::TRANSPARENT })),
        ("première et dernière différentes", true, tuile_de(|i| if i == 0 || i == TILE_PIXELS - 1 { Color32::RED } else { Color32::TRANSPARENT })),
        ("série de 3 exactement", true, tuile_de(|i| if (10..13).contains(&i) { Color32::GREEN } else if i < 300 { gris(i) } else { Color32::TRANSPARENT })),
        ("série qui finit la tuile", true, tuile_de(|i| if i < 50 { gris(i) } else { Color32::BLUE })),
        ("tout différent (ne se range pas)", false, tuile_de(gris)),
        ("bruit (ne se range pas)", false, tuile_de(|_| Color32::from_rgba_premultiplied(graine(&mut s) as u8, graine(&mut s) as u8, graine(&mut s) as u8, 255))),
    ];
    for (nom, doit_se_ranger, t) in cas {
        match pack(&t) {
            Some(octets) => {
                assert!(doit_se_ranger, "{nom} ne devait pas se ranger");
                assert!(octets.len() <= MAX_OCTETS, "{nom}");
                let r = unpack(&octets);
                assert!(r[..] == t[..], "{nom} : pixels différents");
            }
            None => assert!(!doit_se_ranger, "{nom} devait se ranger"),
        }
    }
}

#[test]
fn random_tiles_round_trip() {
    let mut s = 12345u64;
    let mut rangees = 0;
    for k in 0..400 {
        // Un mélange de séries, de transparent et de pixels quelconques, plus ou moins dense.
        let densite = (k % 20) as u64 * 5;
        let t = tuile_de(|i| {
            let r = graine(&mut s);
            if (r >> 8) % 100 < densite {
                Color32::from_rgba_premultiplied(r as u8, (r >> 16) as u8, (r >> 24) as u8, (r >> 32) as u8)
            } else if i % 97 < 40 {
                Color32::from_rgba_premultiplied(5, 6, 7, 255)
            } else {
                Color32::TRANSPARENT
            }
        });
        if let Some(octets) = pack(&t) {
            rangees += 1;
            assert!(unpack(&octets)[..] == t[..], "tuile {k}");
        }
    }
    assert!(rangees > 100, "{rangees} tuiles rangées");
}

#[test]
fn a_line_art_tile_is_much_smaller() {
    let t = tuile_de(|i| if i % 64 == 31 { Color32::from_rgba_premultiplied(9, 9, 9, 200) } else { Color32::TRANSPARENT });
    let octets = pack(&t).unwrap();
    assert!(octets.len() < 2048, "{} octets", octets.len());
}
