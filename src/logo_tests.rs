//! Logo et icône de FEANOR : le Joyau (formes du livre de marque, quatre variantes), l'icône de l'application, le fichier
//! `.ico` et l'animation d'ouverture (la fonction des images clés).

use crate::logo::{self, EtatAnim, Joyau};

/// Pixel (RVBA prémultiplié) du Joyau rasterisé sur `cote` pixels, pour un point de la grille de 64.
fn pixel_au(px: &[u8], cote: u32, x: f32, y: f32) -> [u8; 4] {
    let s = cote as f32 / 64.0;
    let (ix, iy) = (((x * s) as u32).min(cote - 1), ((y * s) as u32).min(cote - 1));
    let i = ((iy * cote + ix) * 4) as usize;
    [px[i], px[i + 1], px[i + 2], px[i + 3]]
}

fn points(liste: &[(f32, f32)]) -> String {
    liste.iter().map(|(x, y)| format!("{},{}", x, y)).collect::<Vec<_>>().join(" ")
}

#[test]
fn the_svg_files_in_the_repository_have_the_shapes_of_the_drawing() {
    let (pierre, facette, etoile) = (points(&logo::PIERRE), points(&logo::FACETTE), points(&logo::ETOILE));
    for (nom, svg) in [
        ("jade", include_str!("../assets/feanor-viseur-jade.svg")),
        ("iris", include_str!("../assets/feanor-viseur-irise.svg")),
        ("icone", include_str!("../assets/feanor-viseur-jade-icone-app.svg")),
    ] {
        assert!(svg.contains(r#"viewBox="0 0 64 64""#), "{nom}");
        for forme in [&pierre, &facette, &etoile] {
            assert!(svg.contains(&format!("points=\"{forme}\"")), "{nom} : {forme}");
        }
    }
    let iris = include_str!("../assets/feanor-viseur-irise.svg");
    assert!(iris.contains("spreadMethod=\"reflect\"") && iris.contains("#C7B5FF") && iris.contains("#FFB3D1") && iris.contains("#FFF1A8"));
    assert!(iris.contains("opacity=\".42\""));
    for (nom, svg, encre) in [("clair", include_str!("../assets/feanor-viseur-mono-clair.svg"), "#EDEDEF"), ("fonce", include_str!("../assets/feanor-viseur-mono-fonce.svg"), "#121214")] {
        assert!(svg.contains("mask") && svg.contains(encre), "mono {nom} (pierre et étoile masquées l'une par l'autre)");
        assert!(!svg.contains(&facette), "le mono n'a pas de facette basse");
    }
    let icone = include_str!("../assets/feanor-viseur-jade-icone-app.svg");
    assert!(icone.contains("rx=\"14\"") && icone.contains("translate(6.4 6.4) scale(.8)") && icone.contains("#2C2C2E"));
}

#[test]
fn the_jade_joyau_has_the_three_brand_colours_and_a_transparent_background() {
    let px = logo::image(256, Joyau::Jade, None);
    assert_eq!(px.len(), 256 * 256 * 4);
    let jade = |c: [u8; 3]| [c[0], c[1], c[2], 255];
    assert_eq!(pixel_au(&px, 256, 22.0, 22.0), jade(logo::JADE), "la pierre");
    assert_eq!(pixel_au(&px, 256, 42.0, 42.0), jade(logo::JADE_PROFOND), "la facette basse");
    assert_eq!(pixel_au(&px, 256, 32.0, 8.0), jade(logo::ETOILE_BLANCHE), "la pointe de l'étoile, hors de la pierre");
    assert_eq!(pixel_au(&px, 256, 2.0, 2.0)[3], 0, "le coin est transparent");
    // L'étoile ne déborde pas de la grille : la bordure de l'image reste vide.
    let cote = 256usize;
    for k in 0..cote {
        for (x, y) in [(k, 0), (k, cote - 1), (0, k), (cote - 1, k)] {
            let a = px[(y * cote + x) * 4 + 3];
            assert!(a < 255, "bord plein en ({x}, {y})");
        }
    }
}

#[test]
fn the_small_joyau_has_thicker_arms_and_is_used_under_24_pixels_and_for_the_16_pixel_icon() {
    // Mêmes pierre et facette, étoile différente.
    assert_eq!(logo::ETOILE[6], (4.0, 32.0), "les bras gauche et droit sont égaux (4 et 60 sur l'axe)");
    assert_eq!(logo::ETOILE_PETITE, [(32.0, 2.0), (36.0, 28.0), (60.0, 32.0), (36.0, 36.0), (32.0, 62.0), (28.0, 36.0), (4.0, 32.0), (28.0, 28.0)]);
    let petit = include_str!("../assets/feanor-viseur-jade-petit.svg");
    assert!(petit.contains(&format!("points=\"{}\"", points(&logo::ETOILE_PETITE))) && petit.contains(&format!("points=\"{}\"", points(&logo::PIERRE))));
    // 16 px : le petit Joyau ; 32 px : le grand.
    assert_eq!(logo::image(16, Joyau::Jade, None), logo::image(16, Joyau::JadePetit, None));
    assert_ne!(logo::image(32, Joyau::Jade, None), logo::image(32, Joyau::JadePetit, None));
    // L'icône de 16 px utilise le petit Joyau, celle de 32 px le grand.
    assert_ne!(logo::icone(16), logo::icone(32)[..16 * 16 * 4].to_vec());
    // Un point que seul le gros bras épaissi couvre : (34.4, 20) du petit Joyau (la pointe du grand est plus fine).
    let px = logo::image(256, Joyau::JadePetit, None);
    let grand = logo::image(256, Joyau::Jade, None);
    assert_eq!(pixel_au(&px, 256, 34.4, 20.0), [0xF6, 0xFF, 0xFD, 255]);
    assert_ne!(pixel_au(&grand, 256, 34.4, 20.0), [0xF6, 0xFF, 0xFD, 255]);
}

#[test]
fn the_iridescent_joyau_has_a_gradient_stone_and_a_light_lower_facet() {
    let px = logo::image(256, Joyau::Iris, None);
    let jade = logo::image(256, Joyau::Jade, None);
    let (a, b) = (pixel_au(&px, 256, 20.0, 20.0), pixel_au(&px, 256, 47.0, 22.0));
    assert_eq!((a[3], b[3]), (255, 255));
    assert_ne!(a, b, "la pierre change de couleur d'un bout à l'autre");
    assert_ne!(pixel_au(&px, 256, 20.0, 20.0), pixel_au(&jade, 256, 20.0, 20.0), "ce n'est pas le jade");
    assert_ne!(pixel_au(&px, 256, 42.0, 42.0), pixel_au(&jade, 256, 42.0, 42.0), "la facette basse est à 42 %");
    assert_eq!(pixel_au(&px, 256, 32.0, 8.0), [0xF6, 0xFF, 0xFD, 255], "l'étoile reste blanche");
    // Le début du dégradé est le jade (coin haut gauche de la pierre).
    let debut = pixel_au(&px, 256, 19.0, 18.0);
    assert!(debut[0] < 0x80 && debut[1] > 0xA0, "début jade : {debut:?}");
}

#[test]
fn the_mono_joyau_swaps_full_and_empty_where_stone_and_star_overlap() {
    for encre in [logo::ENCRE_SOMBRE, logo::ENCRE_CLAIRE] {
        let px = logo::image(256, Joyau::Mono(encre), None);
        let plein = [encre[0], encre[1], encre[2], 255];
        assert_eq!(pixel_au(&px, 256, 22.0, 22.0), plein, "la pierre seule");
        assert_eq!(pixel_au(&px, 256, 32.0, 8.0), plein, "l'étoile seule");
        assert_eq!(pixel_au(&px, 256, 32.0, 25.0)[3], 0, "là où elles se recouvrent, c'est vide");
        assert_eq!(pixel_au(&px, 256, 42.0, 42.0), plein, "pas de facette basse : la pierre est pleine");
    }
}

#[test]
fn the_opening_animation_keyframes_start_hidden_peak_at_34_percent_and_end_at_rest() {
    let d = logo::DUREE_ANIMATION;
    assert!((d - 1.15).abs() < 1e-6);
    // Début : l'étoile est tournée de -200° et n'a pas de taille ; la seconde étoile aussi, à 0,9 d'opacité.
    let debut = logo::etat_animation(0.0);
    assert_eq!((debut.etoile_angle, debut.etoile_echelle), (-200.0, 0.0));
    assert_eq!((debut.seconde_angle, debut.seconde_echelle, debut.seconde_opacite), (-155.0, 0.0, 0.9));
    // À 34 % (0,39 s) : -180° et 1,25 ; seconde étoile -135°, 1,15.
    let pic = logo::etat_animation(0.34 * d);
    assert!((pic.etoile_angle + 180.0).abs() < 1e-3 && (pic.etoile_echelle - 1.25).abs() < 1e-3, "{pic:?}");
    assert!((pic.seconde_angle + 135.0).abs() < 1e-3 && (pic.seconde_echelle - 1.15).abs() < 1e-3 && (pic.seconde_opacite - 0.9).abs() < 1e-3, "{pic:?}");
    // À 72 % : 0,96.
    let creux = logo::etat_animation(0.72 * d);
    assert!((creux.etoile_echelle - 0.96).abs() < 1e-3, "{creux:?}");
    // Entre le début et le pic, l'étoile grandit sans reculer.
    let mut avant = 0.0;
    for k in 0..=34 {
        let e = logo::etat_animation(k as f32 / 100.0 * d).etoile_echelle;
        assert!(e + 1e-4 >= avant, "l'échelle recule à {k} %");
        avant = e;
    }
    // Fin : la pose de repos (-180°, échelle 1) et la seconde étoile invisible, à 80 % comme à 100 %.
    let fin = logo::etat_animation(d);
    assert_eq!((fin.etoile_angle, fin.etoile_echelle), (-180.0, 1.0));
    assert_eq!(fin.seconde_opacite, 0.0);
    assert!((fin.seconde_echelle - 0.45).abs() < 1e-6 && fin.seconde_angle == -135.0);
    assert_eq!(logo::etat_animation(0.8 * d).seconde_opacite, 0.0);
    assert_eq!(logo::etat_animation(d * 10.0), fin, "figée après la fin");
    assert_eq!(logo::etat_repos(), fin);
    // La première courbe (0,15 ; 0,6 ; 0,3 ; 1) démarre plus vite que le linéaire.
    assert!(logo::bezier(0.15, 0.6, 0.3, 1.0, 0.2) > 0.3);
    assert_eq!((logo::bezier(0.45, 0.0, 0.25, 1.0, 0.0), logo::bezier(0.45, 0.0, 0.25, 1.0, 1.0)), (0.0, 1.0));
    assert!((logo::bezier(0.25, 0.25, 0.75, 0.75, 0.3) - 0.3).abs() < 1e-3, "la courbe linéaire");
}

#[test]
fn the_rest_pose_of_the_animation_draws_the_joyau_at_rest_and_the_end_has_no_second_star() {
    let repos: EtatAnim = logo::etat_repos();
    let fixe = logo::image(128, Joyau::Jade, None);
    let fin = logo::image(128, Joyau::Jade, Some(&repos));
    let diff = fixe.iter().zip(&fin).filter(|(a, b)| a.abs_diff(**b) > 3).count();
    assert!(diff < 40, "l'image finale de l'animation est le repos : {diff} octets diffèrent");
    // Au début, l'étoile n'existe pas encore : le centre de la pierre est la couleur de la pierre, pas du blanc.
    let debut = logo::image(128, Joyau::Jade, Some(&logo::etat_animation(0.0)));
    assert_ne!(pixel_au(&debut, 128, 32.0, 8.0)[3], 255, "pas d'étoile au début");
    // Au pic, l'étoile dépasse de la grille ou presque (échelle 1,25) : elle est coupée, rien ne plante.
    let _ = logo::image(64, Joyau::Iris, Some(&logo::etat_animation(0.34 * logo::DUREE_ANIMATION)));
}

#[test]
fn the_application_icon_is_a_dark_rounded_square_with_a_hairline_and_the_jade_joyau() {
    for taille in [16u32, 32, 64, 256] {
        let px = logo::icone(taille);
        assert_eq!(px.len(), (taille * taille * 4) as usize);
        let at = |x: u32, y: u32| [px[((y * taille + x) * 4) as usize], px[((y * taille + x) * 4) as usize + 1], px[((y * taille + x) * 4) as usize + 2], px[((y * taille + x) * 4) as usize + 3]];
        assert!(at(0, 0)[3] < 100, "coins arrondis à {taille} px");
        assert_eq!(at(taille / 2, 0)[3], 255, "{taille}");
        // Le fond près du bord, dans l'angle bas (hors Joyau et hors filet) : le sol de la marque.
        if taille >= 64 {
            let (x, y) = (taille * 12 / 100, taille / 2);
            assert_eq!(&at(x, y)[..3], &logo::SOL, "fond à {taille} px");
        }
        // Le centre est l'étoile blanche, plus loin vers le haut la pierre jade.
        if taille >= 24 {
            assert_eq!(&at(taille / 2, taille / 2)[..3], &logo::ETOILE_BLANCHE, "centre à {taille} px");
        } else {
            assert!(at(taille / 2, taille / 2)[0] > 150, "centre clair (petit Joyau, pixels mêlés) à {taille} px");
        }
    }
    let t = 256u32;
    let px = logo::icone(t);
    let at = |x: u32, y: u32| px[((y * t + x) * 4) as usize..][..4].to_vec();
    // La pierre jade, à (22, 22) de la grille du Joyau réduit : 9,6 + 22 × 0,7 = 25 → 100 px sur 256.
    assert_eq!(&at(100, 100)[..3], &logo::JADE);
    assert_eq!(at(12, 12)[3], 0, "rayon d'environ 22 % du côté");
}

#[test]
fn the_ico_file_has_one_entry_per_size_and_right_offsets() {
    let tailles = [16u32, 32, 48, 256];
    let ico = logo::ico(&tailles);
    assert_eq!(&ico[..4], &[0, 0, 1, 0], "en-tête ICO");
    assert_eq!(u16::from_le_bytes([ico[4], ico[5]]) as usize, tailles.len());
    let mut fin = 6 + 16 * tailles.len();
    for (k, t) in tailles.iter().enumerate() {
        let e = &ico[6 + 16 * k..][..16];
        assert_eq!(e[0] as u32, if *t == 256 { 0 } else { *t });
        let octets = u32::from_le_bytes(e[8..12].try_into().unwrap()) as usize;
        let decalage = u32::from_le_bytes(e[12..16].try_into().unwrap()) as usize;
        assert_eq!(decalage, fin, "décalage de l'image {k}");
        assert_eq!(u32::from_le_bytes(ico[decalage..][..4].try_into().unwrap()), 40);
        assert_eq!(u32::from_le_bytes(ico[decalage + 8..][..4].try_into().unwrap()), t * 2);
        let masque = (*t as usize).div_ceil(32) * 4 * *t as usize;
        assert_eq!(octets, 40 + (*t * *t * 4) as usize + masque);
        fin += octets;
    }
    assert_eq!(ico.len(), fin);
}

/// Planches pour regarder le logo et l'icône (essai ignoré : `cargo test logo_planches -- --ignored`,
/// écrit dans le dossier donné par `FEANOR_PLANCHES`).
#[test]
#[ignore]
fn logo_planches() {
    let Some(dir) = std::env::var_os("FEANOR_PLANCHES").map(std::path::PathBuf::from) else { return };
    std::fs::create_dir_all(&dir).unwrap();
    let sur = |px: &[u8], fond: [u8; 3]| -> Vec<u8> {
        let mut rgb = Vec::new();
        for p in px.chunks_exact(4) {
            for c in 0..3 {
                rgb.push((p[c] as u32 + fond[c] as u32 * (255 - p[3] as u32) / 255).min(255) as u8);
            }
        }
        rgb
    };
    for (nom, v, fond) in [
        ("jade", Joyau::Jade, logo::SOL),
        ("iris", Joyau::Iris, logo::SOL),
        ("mono-fonce", Joyau::Mono(logo::ENCRE_SOMBRE), logo::SOL),
        ("mono-clair", Joyau::Mono(logo::ENCRE_CLAIRE), [0xF5, 0xF1, 0xE8]),
    ] {
        for cote in [512u32, 32, 16] {
            image::save_buffer(dir.join(format!("joyau-{nom}-{cote}.png")), &sur(&logo::image(cote, v, None), fond), cote, cote, image::ExtendedColorType::Rgb8).unwrap();
        }
    }
    for t in [16u32, 32, 256] {
        image::save_buffer(dir.join(format!("icone-{t}.png")), &logo::icone(t), t, t, image::ExtendedColorType::Rgba8).unwrap();
    }
    for (k, pct) in [0u32, 15, 34, 55, 72, 100].into_iter().enumerate() {
        let etat = logo::etat_animation(pct as f32 / 100.0 * logo::DUREE_ANIMATION);
        let px = logo::image(256, Joyau::Iris, Some(&etat));
        image::save_buffer(dir.join(format!("anim-{k}-{pct}.png")), &sur(&px, logo::SOL), 256, 256, image::ExtendedColorType::Rgb8).unwrap();
    }
}
