//! Tests des nuanciers personnels (passe couleur) : opérations, rangement, noms, réglages
//! retenus, formats GIMP (.gpl) et Photoshop (.aco) sur des exemples écrits ici.

use eframe::egui::{pos2, vec2, Rect};

use crate::couleur::{lab_to_rgb, ColorSettings};
use crate::nuancier::{self, aco_color, parse_aco, parse_gpl, to_aco, to_gpl, Palette, Swatch};

fn palette(name: &str, colors: &[([u8; 3], &str)]) -> Palette {
    Palette { name: name.into(), swatches: colors.iter().map(|(c, n)| Swatch::new(*c, n)).collect() }
}

#[test]
fn palettes_are_created_renamed_duplicated_and_deleted() {
    let mut list = vec![Palette::pelure()];
    assert_eq!(list[0].swatches.len(), 10, "l'ancien nuancier de SHOT");
    assert_eq!(list[0].name, "FEANOR");
    let i = nuancier::create(&mut list, 0);
    assert_eq!((i, list[1].name.as_str()), (1, "Nouveau nuancier"));
    let j = nuancier::create(&mut list, 0);
    assert_eq!((j, list[1].name.as_str()), (1, "Nouveau nuancier 2"), "juste après, sous un nom libre");
    assert!(nuancier::rename(&mut list, 1, "  Décors  ").is_ok());
    assert_eq!(list[1].name, "Décors", "sans les espaces autour");
    assert!(nuancier::rename(&mut list, 1, "FEANOR").is_err(), "nom déjà pris");
    assert!(nuancier::rename(&mut list, 1, "   ").is_err(), "nom vide");
    assert!(nuancier::rename(&mut list, 1, "Décors").is_ok(), "son propre nom");
    list[1].add([1, 2, 3], "Ciel").unwrap();
    let d = nuancier::duplicate(&mut list, 1).unwrap();
    assert_eq!((d, list[2].name.as_str()), (2, "Décors (copie)"));
    assert_eq!(list[2].swatches, list[1].swatches);
    assert_eq!(nuancier::duplicate(&mut list, 1), Some(2));
    assert_eq!(list[2].name, "Décors (copie) 2");
    assert_eq!(nuancier::duplicate(&mut list, 9), None);
    // Supprimer : on montre celui d'avant ; il en reste toujours un.
    let n = list.len();
    assert_eq!(nuancier::delete(&mut list, 2), 1);
    assert_eq!(list.len(), n - 1);
    while list.len() > 1 {
        nuancier::delete(&mut list, 0);
    }
    assert_eq!(nuancier::delete(&mut list, 0), 0);
    assert_eq!(list.len(), 1, "un nuancier vide reste");
    assert!(list[0].swatches.is_empty());
}

#[test]
fn swatches_are_added_once_and_moved() {
    let mut p = Palette::new("Essai");
    assert_eq!(p.add([10, 0, 0], ""), Ok(0));
    assert_eq!(p.add([20, 0, 0], "deux"), Ok(1));
    assert_eq!(p.add([10, 0, 0], "encore"), Err(0), "pas deux fois la même couleur");
    assert_eq!(p.position([20, 0, 0]), Some(1));
    let mut v = vec!['a', 'b', 'c', 'd'];
    // Fentes : 0 avant « a », 4 après « d ».
    assert_eq!(nuancier::move_item(&mut v, 0, 3), 2);
    assert_eq!(v, ['b', 'c', 'a', 'd']);
    assert_eq!(nuancier::move_item(&mut v, 3, 0), 0);
    assert_eq!(v, ['d', 'b', 'c', 'a']);
    assert_eq!(nuancier::move_item(&mut v, 1, 4), 3);
    assert_eq!(v, ['d', 'c', 'a', 'b']);
    assert_eq!(nuancier::move_item(&mut v, 2, 2), 2, "lâché à sa place");
    assert_eq!(nuancier::move_item(&mut v, 2, 3), 2, "juste après lui-même : à sa place aussi");
    assert_eq!(v, ['d', 'c', 'a', 'b']);
    assert_eq!(nuancier::move_item(&mut v, 9, 0), 9, "rang inconnu : rien");
}

#[test]
fn the_drop_slot_follows_the_pointer() {
    // Deux rangées de trois pastilles de 18 points, 2 points d'écart.
    let rects: Vec<Rect> = (0..6).map(|k| Rect::from_min_size(pos2((k % 3) as f32 * 20.0, (k / 3) as f32 * 20.0), vec2(18.0, 18.0))).collect();
    assert_eq!(nuancier::drop_slot(&rects, pos2(2.0, 9.0)), Some(0), "moitié gauche de la première");
    assert_eq!(nuancier::drop_slot(&rects, pos2(15.0, 9.0)), Some(1), "moitié droite de la première");
    assert_eq!(nuancier::drop_slot(&rects, pos2(55.0, 29.0)), Some(6), "après la dernière");
    assert_eq!(nuancier::drop_slot(&rects, pos2(-30.0, 25.0)), Some(3), "à gauche de la seconde rangée");
    assert_eq!(nuancier::drop_slot(&[], pos2(0.0, 0.0)), None);
}

#[test]
fn palettes_are_kept_in_the_settings_file() {
    let mut s = crate::settings::Settings::default();
    s.colors.palettes.push(palette("Décors", &[([1, 2, 3], "Ciel"), ([250, 240, 230], "")]));
    s.colors.shown = 1;
    let text = toml::to_string(&s).unwrap();
    for key in ["nuancier_affiche = 1", "[[couleurs.nuanciers]]", "nom = \"Décors\"", "[[couleurs.nuanciers.pastilles]]", "couleur = [1, 2, 3]", "nom = \"Ciel\""] {
        assert!(text.contains(key), "{key} manque :\n{text}");
    }
    let back: crate::settings::Settings = toml::from_str(&text).unwrap();
    assert_eq!(back.colors, s.colors);
    // Réparés à la lecture : un nuancier au moins, un rang montré qui existe.
    let broken = ColorSettings { palettes: Vec::new(), shown: 7, recent: vec![[0, 0, 0]; 100], ..Default::default() }.sanitized();
    assert_eq!(broken.palettes, vec![Palette::pelure()]);
    assert_eq!(broken.shown, 0);
    assert_eq!(broken.recent.len(), crate::couleur::RECENT_MAX);
}

#[test]
fn the_palette_kept_under_the_old_name_is_renamed_on_read() {
    // Réglages écrits sous les anciens noms du logiciel : le nuancier de départ s'appelait « SHOT », et « Pelure » avant.
    for ancien in ["SHOT", "Pelure"] {
        let mut old = Palette::pelure();
        old.name = ancien.into();
        let s = ColorSettings { palettes: vec![old, palette("Décors", &[([1, 2, 3], "Ciel")])], ..Default::default() }.sanitized();
        assert_eq!(s.palettes[0].name, "FEANOR");
        assert_eq!(s.palettes[1].name, "Décors");
    }
    // Un nuancier « FEANOR » déjà là : l'ancien garde son nom, pas de doublon.
    let mut old = Palette::pelure();
    old.name = "SHOT".into();
    let s = ColorSettings { palettes: vec![old, Palette::pelure()], ..Default::default() }.sanitized();
    assert_eq!(s.palettes.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["SHOT", "FEANOR"]);
    // Les deux anciens : seul le plus récent devient « FEANOR ».
    let (mut a, mut b) = (Palette::pelure(), Palette::pelure());
    a.name = "Pelure".into();
    b.name = "SHOT".into();
    let s = ColorSettings { palettes: vec![a, b], ..Default::default() }.sanitized();
    assert_eq!(s.palettes.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Pelure", "FEANOR"]);
}

// ------------------------------------------------------------ GIMP

/// Un nuancier tel que GIMP l'écrit (en-tête, nom, colonnes, commentaire), avec des
/// tabulations, des espaces en trop, une couleur sans nom, une ligne illisible et un
/// « Untitled ».
const GIMP: &str = "GIMP Palette\r\nName: Bus de nuit\r\nColumns: 4\r\n# un commentaire\r\n\
255   0   0\tRouge feu\r\n  0 128 255\tBleu nuit\tid-42\r\n 12  34  56\r\n300 0 0\tTrop\r\n\
1 2 3 Gris très foncé\r\n\r\n10 20 30\tUntitled\r\n";

#[test]
fn gimp_palettes_are_read() {
    let imp = parse_gpl(GIMP).unwrap();
    assert_eq!(imp.palette.name, "Bus de nuit");
    assert_eq!(
        imp.palette.swatches,
        vec![
            Swatch::new([255, 0, 0], "Rouge feu"),
            Swatch::new([0, 128, 255], "Bleu nuit"),
            Swatch::new([12, 34, 56], ""),
            Swatch::new([1, 2, 3], "Gris très foncé"),
            Swatch::new([10, 20, 30], ""),
        ]
    );
    assert_eq!(imp.skipped, 1, "300 n'est pas une valeur de 0 à 255");
    assert!(parse_gpl("Ceci n'est pas un nuancier\n1 2 3\n").is_err());
    assert!(parse_gpl("\u{feff}GIMP Palette\n1 2 3\n").is_ok(), "marque d'ordre des octets en tête");
}

#[test]
fn gimp_palettes_are_written_and_read_back() {
    let p = palette("Mon nuancier", &[([255, 0, 0], "Rouge"), ([0, 0, 0], ""), ([5, 6, 7], "avec\ttab")]);
    let text = to_gpl(&p);
    assert!(text.starts_with("GIMP Palette\nName: Mon nuancier\n"), "{text}");
    assert!(text.contains("255   0   0\tRouge\n"), "{text}");
    assert!(text.contains("  0   0   0\n"), "sans nom : rien après les chiffres\n{text}");
    let back = parse_gpl(&text).unwrap();
    assert_eq!(back.palette.name, p.name);
    assert_eq!(back.palette.swatches[..2], p.swatches[..2]);
    assert_eq!(back.palette.swatches[2].name, "avec tab", "une tabulation dans un nom devient une espace");
    assert_eq!(back.skipped, 0);
}

// ------------------------------------------------------------ Photoshop

/// Octets d'un .aco, écrits à la main : `words` en 16 bits, octet de poids fort d'abord.
fn be(words: &[u16]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_be_bytes()).collect()
}

#[test]
fn photoshop_version_1_is_read() {
    // Version 1, 2 couleurs : rouge en RVB, puis un vert en TSV (« HSB ») : teinte 120°.
    let data = be(&[1, 2, 0, 0xFFFF, 0, 0, 0, 1, 21845, 0xFFFF, 0xFFFF, 0]);
    let imp = parse_aco(&data).unwrap();
    assert_eq!(imp.palette.swatches, vec![Swatch::new([255, 0, 0], ""), Swatch::new([0, 255, 0], "")]);
    assert_eq!(imp.skipped, 0);
}

#[test]
fn photoshop_version_2_gives_the_names() {
    // Version 1 puis version 2 (la même couleur, nommée « Ré » : R, é, 0 final), comme Photoshop.
    let mut data = be(&[1, 1, 0, 0x8080, 0x4040, 0x2020, 0]);
    data.extend(be(&[2, 1, 0, 0x8080, 0x4040, 0x2020, 0]));
    data.extend(3u32.to_be_bytes());
    data.extend(be(&[b'R' as u16, 0xE9, 0]));
    let imp = parse_aco(&data).unwrap();
    assert_eq!(imp.palette.swatches, vec![Swatch::new([128, 64, 32], "Ré")]);
    // Une section de version 2 abîmée : on garde celle de la version 1.
    let cut = &data[..data.len() - 3];
    assert_eq!(parse_aco(cut).unwrap().palette.swatches, vec![Swatch::new([128, 64, 32], "")]);
    // Tronqué dès la première section, ou version inconnue : refusé.
    assert!(parse_aco(&data[..9]).is_err());
    assert!(parse_aco(&be(&[7, 0])).is_err());
    assert!(parse_aco(&[]).is_err());
}

#[test]
fn photoshop_color_spaces() {
    // CMJN : 0 = 100 % d'encre. Sans encre : blanc ; noir plein : noir ; cyan plein : (0, 255, 255).
    assert_eq!(aco_color(2, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF), Some([255, 255, 255]));
    assert_eq!(aco_color(2, 0xFFFF, 0xFFFF, 0xFFFF, 0), Some([0, 0, 0]));
    assert_eq!(aco_color(2, 0, 0xFFFF, 0xFFFF, 0xFFFF), Some([0, 255, 255]));
    // Gris : 0 = blanc, 10000 = noir (comme GIMP le lit).
    assert_eq!(aco_color(8, 0, 0, 0, 0), Some([255, 255, 255]));
    assert_eq!(aco_color(8, 10000, 0, 0, 0), Some([0, 0, 0]));
    assert_eq!(aco_color(8, 5000, 0, 0, 0), Some([128, 128, 128]));
    // Lab : L en centièmes, a et b signés en centièmes.
    assert_eq!(aco_color(7, 10000, 0, 0, 0), Some([255, 255, 255]));
    assert_eq!(aco_color(7, 0, 0, 0, 0), Some([0, 0, 0]));
    let red = aco_color(7, 5324, 8022u16, 6720u16, 0).unwrap();
    assert!(red[0] > 230 && red[1] < 40 && red[2] < 40, "rouge : {red:?}");
    // Nuanciers de fabricants (Pantone…) : laissés de côté.
    assert_eq!(aco_color(3, 1, 2, 3, 4), None);
    let data = be(&[1, 2, 3, 1, 2, 3, 4, 0, 0, 0, 0xFFFF, 0]);
    let imp = parse_aco(&data).unwrap();
    assert_eq!((imp.palette.swatches.len(), imp.skipped), (1, 1));
}

#[test]
fn lab_to_rgb_matches_reference_values() {
    assert_eq!(lab_to_rgb(100.0, 0.0, 0.0), [255, 255, 255]);
    assert_eq!(lab_to_rgb(0.0, 0.0, 0.0), [0, 0, 0]);
    // Gris moyen du sRGB (128) : L* ≈ 53,585.
    let g = lab_to_rgb(53.585, 0.0, 0.0);
    assert!(g.iter().all(|&v| (127..=129).contains(&v)), "{g:?}");
    // Rouge, vert, bleu du sRGB (valeurs D50 publiées par Bruce Lindbloom), à 2 près.
    for (lab, rgb) in [([54.29, 80.81, 69.89], [255, 0, 0]), ([87.82, -79.29, 80.99], [0, 255, 0]), ([29.57, 68.30, -112.03], [0, 0, 255])] {
        let got = lab_to_rgb(lab[0], lab[1], lab[2]);
        assert!(got.iter().zip(rgb).all(|(a, b)| (*a as i32 - b).abs() <= 2), "{lab:?} → {got:?}, attendu {rgb:?}");
    }
}

#[test]
fn photoshop_palettes_are_written_and_read_back() {
    let p = palette("Essai", &[([255, 0, 0], "Rouge"), ([1, 2, 3], ""), ([0, 128, 255], "Bleu œil")]);
    let data = to_aco(&p);
    // Version 1 : 2 + 2 + 3 × 10 octets ; puis la version 2.
    assert_eq!(&data[..4], &[0, 1, 0, 3]);
    assert_eq!(&data[4..14], &[0, 0, 0xFF, 0xFF, 0, 0, 0, 0, 0, 0], "rouge, espace RVB");
    assert_eq!(&data[34..38], &[0, 2, 0, 3]);
    let back = parse_aco(&data).unwrap();
    assert_eq!(back.palette.swatches, p.swatches);
    assert_eq!(back.skipped, 0);
}

#[test]
fn palette_files_by_extension() {
    let dir = std::env::temp_dir().join(format!("pelure-test-nuanciers-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = palette("Bus : nuit / jour", &[([255, 0, 0], "Rouge"), ([0, 0, 0], "Noir")]);
    assert_eq!(nuancier::file_name(&p), "Bus _ nuit _ jour", "sans les caractères refusés par Windows");
    // Sans extension connue : en .gpl.
    let gpl = nuancier::write_file(&dir.join("sans_extension"), &p).unwrap();
    assert_eq!(gpl.extension().unwrap(), "gpl");
    let aco = nuancier::write_file(&dir.join("essai.ACO"), &p).unwrap();
    for path in [&gpl, &aco] {
        let imp = nuancier::read_file(path).unwrap();
        assert_eq!(imp.palette.swatches, p.swatches, "{}", path.display());
    }
    assert_eq!(nuancier::read_file(&gpl).unwrap().palette.name, p.name, "GIMP garde le nom du nuancier");
    assert_eq!(nuancier::read_file(&aco).unwrap().palette.name, "essai", "Photoshop n'en a pas : celui du fichier");
    // Extension inconnue : d'après le contenu.
    std::fs::write(dir.join("gimp.txt"), GIMP).unwrap();
    assert_eq!(nuancier::read_file(&dir.join("gimp.txt")).unwrap().palette.name, "Bus de nuit");
    assert!(nuancier::read_file(&dir.join("absent.gpl")).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
