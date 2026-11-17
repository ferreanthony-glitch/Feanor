//! Logo et icône de FEANOR : le joyau (formes de la charte du 17/11/2026, couleur et mono), l'icône de l'application, le fichier
//! `.ico` et l'animation d'ouverture (la fonction des images clés).

use crate::logo::{self, EtatAnim, Joyau};

/// Pixel (RVBA prémultiplié) du Joyau rasterisé sur `cote` pixels, pour un point de la grille de 64.
fn pixel_au(px: &[u8], cote: u32, x: f32, y: f32) -> [u8; 4] {
    let s = cote as f32 / 64.0;
    let (ix, iy) = (((x * s) as u32).min(cote - 1), ((y * s) as u32).min(cote - 1));
    let i = ((iy * cote + ix) * 4) as usize;
    [px[i], px[i + 1], px[i + 2], px[i + 3]]
}

/// Pixel du joyau rasterisé sur `cote` pixels, pour un point du dessin (sur 100).
fn pixel_d(px: &[u8], cote: u32, x: f32, y: f32) -> [u8; 4] {
    pixel_au(px, cote, x * logo::GRILLE / logo::DESSIN, y * logo::GRILLE / logo::DESSIN)
}

#[test]
fn the_svg_files_in_the_repository_have_the_shapes_and_colours_of_the_drawing() {
    let couleur = include_str!("../assets/feanor-logo.svg");
    assert!(couleur.contains(r#"viewBox="0 0 100 100""#));
    let etoile = logo::ETOILE.iter().map(|(x, y)| format!("{x} {y}")).collect::<Vec<_>>().join(" L");
    assert!(couleur.contains(&format!("M{etoile} Z")), "l'étoile du dessin : M{etoile} Z");
    for c in logo::DEGRADE.iter().map(|(_, c)| format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])) {
        assert!(couleur.contains(&c), "le dégradé : {c}");
    }
    for (o, _) in logo::QUARTS.iter().filter(|(o, _)| *o < 1.0) {
        assert!(couleur.contains(&format!("opacity=\"{o}\"")), "transparence d'un quart : {o}");
    }
    assert!(couleur.contains("mix-blend-mode:multiply") && couleur.contains("#6A6FD6") && couleur.contains("#2E9FBF"), "les ombres des facettes");
    assert_eq!(couleur.matches("fill=\"url(#ir)\"").count(), 8, "huit facettes");
    let encre = include_str!("../assets/feanor-logo-encre.svg");
    assert!(encre.contains("evenodd") && encre.contains("#121214") && !encre.contains("url(#ir)"), "le mono : un aplat, l'étoile découpée");
    let blanc = include_str!("../assets/feanor-logo-blanc.svg");
    assert!(blanc.contains("evenodd") && blanc.contains("#FFFFFF"));
}

#[test]
fn the_colour_joyau_has_a_white_star_a_vertical_gradient_eight_facets_and_a_transparent_background() {
    let px = logo::image(256, Joyau::Jade, None);
    assert_eq!(px.len(), 256 * 256 * 4);
    assert_eq!(pixel_d(&px, 256, 50.0, 50.0), [255, 255, 255, 255], "le cœur de l'étoile est blanc");
    assert_eq!(pixel_d(&px, 256, 50.0, 33.0), [255, 255, 255, 255], "la branche haute de l'étoile");
    assert_eq!(pixel_d(&px, 256, 3.0, 3.0)[3], 0, "le coin est transparent");
    assert_eq!(pixel_d(&px, 256, 50.0, 1.0)[3], 0, "la pointe du haut est arrondie : rien au-dessus de 4");
    // Le haut est jade (vert plus fort que rouge), le bas tire vers le bleu et le lilas (bleu plus fort que vert).
    let haut = pixel_d(&px, 256, 50.0, 14.0);
    assert!(haut[3] == 255 && haut[1] > haut[0] + 60, "haut jade : {haut:?}");
    let bas = pixel_d(&px, 256, 50.0, 86.0);
    assert!(bas[3] == 255 && bas[2] > bas[1], "bas bleu lilas : {bas:?}");
    // Les facettes : de part et d'autre d'une arête, la lumière change.
    assert_ne!(pixel_d(&px, 256, 40.0, 30.0), pixel_d(&px, 256, 60.0, 30.0), "haut gauche et haut droite");
    assert_ne!(pixel_d(&px, 256, 60.0, 25.0), pixel_d(&px, 256, 72.0, 42.0), "les deux moitiés du quart haut droit");
    // Rien ne déborde : la bordure de l'image reste vide.
    let cote = 256usize;
    for k in 0..cote {
        for (x, y) in [(k, 0), (k, cote - 1), (0, k), (cote - 1, k)] {
            assert!(px[(y * cote + x) * 4 + 3] < 255, "bord plein en ({x}, {y})");
        }
    }
    // Les autres variantes en couleur dessinent le même joyau.
    assert_eq!(logo::image(64, Joyau::JadePetit, None), logo::image(64, Joyau::Jade, None));
    assert_eq!(logo::image(64, Joyau::Iris, None), logo::image(64, Joyau::Jade, None));
}

#[test]
fn the_mono_joyau_is_one_flat_ink_with_the_star_cut_out() {
    for encre in [logo::ENCRE_SOMBRE, logo::ENCRE_CLAIRE] {
        let px = logo::image(256, Joyau::Mono(encre), None);
        let plein = [encre[0], encre[1], encre[2], 255];
        assert_eq!(pixel_d(&px, 256, 25.0, 50.0), plein, "la pierre");
        assert_eq!(pixel_d(&px, 256, 50.0, 85.0), plein, "la pierre, en bas : pas de facettes");
        assert_eq!(pixel_d(&px, 256, 50.0, 50.0)[3], 0, "l'étoile est découpée");
        assert_eq!(pixel_d(&px, 256, 5.0, 5.0)[3], 0, "hors de la pierre");
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
    // Au début, l'étoile blanche n'existe pas encore : sa place est vide (la pierre est taillée autour), la pierre est là.
    let debut = logo::image(128, Joyau::Jade, Some(&logo::etat_animation(0.0)));
    assert_eq!(pixel_d(&debut, 128, 50.0, 50.0)[3], 0, "pas d'étoile au début : le cœur est vide");
    assert_eq!(pixel_d(&debut, 128, 50.0, 14.0)[3], 255, "la pierre est déjà là");
    // Au pic, l'étoile dépasse de la grille ou presque (échelle 1,25) : elle est coupée, rien ne plante.
    let _ = logo::image(64, Joyau::Iris, Some(&logo::etat_animation(0.34 * logo::DUREE_ANIMATION)));
}

#[test]
fn the_application_icon_is_a_dark_rounded_square_with_a_hairline_and_the_joyau() {
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
        // Le centre est l'étoile blanche.
        if taille >= 32 {
            assert_eq!(&at(taille / 2, taille / 2)[..3], &logo::ETOILE_BLANCHE, "centre à {taille} px");
        } else {
            assert!(at(taille / 2, taille / 2)[0] > 150, "centre clair (pixels mêlés) à {taille} px");
        }
    }
    let t = 256u32;
    let px = logo::icone(t);
    let at = |x: u32, y: u32| px[((y * t + x) * 4) as usize..][..4].to_vec();
    // Le haut de la pierre est jade : à 6,4 + 0,8 × 14 × 0,64 de la grille, soit 13,6 / 64 → 54 px sur 256.
    let haut = at(t / 2, 54);
    assert!(haut[3] == 255 && haut[1] > haut[0] + 60, "haut de la pierre jade : {haut:?}");
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
        ("couleur-sombre", Joyau::Jade, logo::SOL),
        ("couleur-clair", Joyau::Jade, [0xF5, 0xF1, 0xE8]),
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
