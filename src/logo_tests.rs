//! Logo et icône de SHOT : le dessin rasterisé n'est pas vide, a les bonnes proportions, et reste
//! lisible dans les deux variantes.

use crate::logo::{self, Variante, VUE_H, VUE_L, VUE_X, VUE_Y};

/// Pixel (RVBA prémultiplié) du logo rasterisé, pour un point en unités du SVG.
fn pixel_au(v: Variante, largeur: u32, x: f32, y: f32) -> [u8; 4] {
    let (w, h, px) = logo::image(largeur, v);
    let s = w as f32 / VUE_L;
    let (ix, iy) = ((((x - VUE_X) * s) as u32).min(w - 1), (((y - VUE_Y) * s) as u32).min(h - 1));
    let i = ((iy * w + ix) * 4) as usize;
    [px[i], px[i + 1], px[i + 2], px[i + 3]]
}

#[test]
fn le_fichier_svg_du_depot_est_celui_dont_viennent_les_formes() {
    let svg = include_str!("../assets/shot-logo.svg");
    assert!(svg.contains(r#"viewBox="0 0 960 720""#));
    assert!(svg.contains(logo::S_GLYPHE), "le S du SVG est celui du dessin");
}

#[test]
fn le_logo_n_est_pas_vide_et_a_les_bonnes_proportions() {
    let (w, h, px) = logo::image(948, Variante::Sombre);
    assert_eq!((w, h), (948, 532));
    assert_eq!(px.len(), (w * h * 4) as usize);
    let opaques = px.chunks_exact(4).filter(|p| p[3] == 255).count();
    let part = opaques as f32 / (w * h) as f32;
    assert!((0.08..0.40).contains(&part), "part de pixels pleins : {part}");
    let (l, a) = (w as f32, h as f32);
    assert!((l / a - VUE_L / VUE_H).abs() < 0.01, "proportions du cadre : {} contre {}", l / a, VUE_L / VUE_H);
    // Quelle que soit la taille demandée, les proportions restent celles du cadre.
    for largeur in [120, 400, 801] {
        let (w, h, _) = logo::image(largeur, Variante::Sombre);
        assert!((w as f32 / h as f32 - VUE_L / VUE_H).abs() < 0.03, "{w}×{h}");
    }
}

#[test]
fn le_fond_est_transparent_et_les_formes_ont_les_couleurs_du_logo() {
    // Coin du cadre : rien.
    assert_eq!(pixel_au(Variante::Sombre, 948, VUE_X + 1.0, VUE_Y + 1.0)[3], 0);
    // Le montant du « H » : #ECE8DC plein.
    assert_eq!(pixel_au(Variante::Sombre, 948, 407.0, 300.0), [0xEC, 0xE8, 0xDC, 255]);
    // Le trou du « O » reste vide (règle pair-impair).
    assert_eq!(pixel_au(Variante::Sombre, 948, 547.0, 330.0)[3], 0);
    // La barre en bas à droite est claire, et « STORY » y est écrit en creux, couleur du fond.
    assert_eq!(pixel_au(Variante::Sombre, 948, 606.0, 442.0), [0xEC, 0xE8, 0xDC, 255]);
    let (w, h, px) = logo::image(948, Variante::Sombre);
    let s = w as f32 / VUE_L;
    let mut creux = 0;
    for y in 437..449 {
        for x in 609..695 {
            let i = (((((y as f32 - VUE_Y) * s) as u32).min(h - 1) * w + (((x as f32 - VUE_X) * s) as u32).min(w - 1)) * 4) as usize;
            if px[i + 3] == 255 && px[i] < 0x30 {
                creux += 1;
            }
        }
    }
    assert!(creux > 100, "« STORY » doit se voir en sombre dans la barre claire : {creux} pixels");
}

#[test]
fn la_variante_claire_inverse_le_logo_pour_un_fond_clair() {
    assert_eq!(pixel_au(Variante::Clair, 948, 407.0, 300.0), [0x1E, 0x1E, 0x22, 255]);
    // Le creux de « STORY » prend le papier : lisible dans la barre sombre.
    assert_eq!(pixel_au(Variante::Clair, 948, 606.0, 442.0), [0x1E, 0x1E, 0x22, 255]);
}

#[test]
fn les_chemins_du_svg_sont_lus_en_entier() {
    let b = logo::chemin(logo::S_GLYPHE).bounds();
    assert!((b.left() - 277.6).abs() < 0.05 && (b.right() - 366.0).abs() < 0.05, "{b:?}");
    assert!((b.top() - 240.0).abs() < 0.05 && (b.bottom() - 420.0).abs() < 0.05, "{b:?}");
    // L'anneau du « O » (arcs) : de 505 à 590 en x, de 241 à 418 en y.
    let o = logo::chemin("M541 241H554A36 30 0 0 1 590 271V388A36 30 0 0 1 554 418H541A36 30 0 0 1 505 388V271A36 30 0 0 1 541 241Z");
    let b = o.bounds();
    assert!((b.left() - 505.0).abs() < 0.5 && (b.right() - 590.0).abs() < 0.5, "{b:?}");
    assert!((b.top() - 241.0).abs() < 0.5 && (b.bottom() - 418.0).abs() < 0.5, "{b:?}");
}

#[test]
fn l_icone_est_un_carre_sombre_avec_le_s_et_deux_angles() {
    for taille in [16u32, 32, 64, 256] {
        let px = logo::icone(taille);
        assert_eq!(px.len(), (taille * taille * 4) as usize);
        let at = |x: u32, y: u32| &px[((y * taille + x) * 4) as usize..][..4];
        // Coins arrondis : le tout premier pixel est transparent, le centre du bord haut ne l'est pas.
        assert!(at(0, 0)[3] < 100, "{taille}");
        assert_eq!(at(taille / 2, 0)[3], 255, "{taille}");
        // Angle haut gauche du logo : clair.
        let (x, y) = ((taille as f32 * 0.14) as u32, (taille as f32 * 0.14) as u32);
        assert!(at(x, y)[0] > 0xC0, "angle haut gauche à {taille} px : {:?}", at(x, y));
        // Du clair (le S et les angles) et du sombre (le fond) en proportions raisonnables.
        let clairs = px.chunks_exact(4).filter(|p| p[3] == 255 && p[0] > 0xC0).count();
        let part = clairs as f32 / (taille * taille) as f32;
        assert!((0.10..0.45).contains(&part), "part de clair à {taille} px : {part}");
    }
}

#[test]
fn le_fichier_ico_a_une_entree_par_taille_et_des_decalages_justes() {
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
        // BITMAPINFOHEADER : 40, largeur, 2 × hauteur.
        assert_eq!(u32::from_le_bytes(ico[decalage..][..4].try_into().unwrap()), 40);
        assert_eq!(u32::from_le_bytes(ico[decalage + 8..][..4].try_into().unwrap()), t * 2);
        let masque = (*t as usize).div_ceil(32) * 4 * *t as usize;
        assert_eq!(octets, 40 + (*t * *t * 4) as usize + masque);
        fin += octets;
    }
    assert_eq!(ico.len(), fin);
}

/// Planches pour regarder le logo et l'icône (essai ignoré : `cargo test logo_planches -- --ignored`,
/// écrit dans le dossier donné par `SHOT_PLANCHES`).
#[test]
#[ignore]
fn logo_planches() {
    let Some(dir) = std::env::var_os("SHOT_PLANCHES").map(std::path::PathBuf::from) else { return };
    std::fs::create_dir_all(&dir).unwrap();
    for (nom, v, fond) in [("logo-sombre", Variante::Sombre, [0x12u8, 0x12, 0x14]), ("logo-clair", Variante::Clair, [0xF4, 0xF2, 0xEC])] {
        for largeur in [948u32, 400] {
            let (w, h, px) = logo::image(largeur, v);
            let mut rgb = Vec::new();
            for p in px.chunks_exact(4) {
                for c in 0..3 {
                    // Prémultiplié sur le fond.
                    rgb.push((p[c] as u32 + fond[c] as u32 * (255 - p[3] as u32) / 255).min(255) as u8);
                }
            }
            image::save_buffer(dir.join(format!("{nom}-{largeur}.png")), &rgb, w, h, image::ExtendedColorType::Rgb8).unwrap();
        }
    }
    for t in [16u32, 32, 64, 256] {
        image::save_buffer(dir.join(format!("icone-{t}.png")), &logo::icone(t), t, t, image::ExtendedColorType::Rgba8).unwrap();
    }
}
