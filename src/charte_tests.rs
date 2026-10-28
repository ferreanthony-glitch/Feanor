//! La charte graphique de SHOT (planche d'Anthony, 2026-10-28) : le thème « SHOT » (défaut d'une installation
//! neuve, un poste qui a déjà choisi un thème le garde), ses couleurs et ses contrastes, ce que l'orange touche (et ce
//! qu'il ne touche pas), le logo et l'icône aux couleurs officielles, le petit logotype de la barre de menus, le bouton
//! principal de l'accueil et des fenêtres. Les captures d'écran (essais ignorés) vont dans `SHOT_CAPTURE_DIR`.

use eframe::egui::{self, epaint::Shape, vec2, Color32, Pos2, Rect};

use crate::document::Document;
use crate::logo::{self, Variante};
use crate::perf_tests::{banc_taille, Banc};
use crate::settings::Settings;
use crate::theme::{self, contraste, ChoixTheme, Couleurs, Palette, ReglageTheme};
use crate::workspace::Espace;

const NOIR: Color32 = Color32::from_rgb(0x0E, 0x0E, 0x0E);
const BLANC_CASSE: Color32 = Color32::from_rgb(0xF4, 0xF1, 0xEA);
const GRIS_SCENE: Color32 = Color32::from_rgb(0x8A, 0x8A, 0x8A);
const ORANGE: Color32 = Color32::from_rgb(0xE7, 0x6F, 0x3C);

fn rvb(c: Color32) -> [u8; 3] {
    [c.r(), c.g(), c.b()]
}

fn luminance_proche(a: Color32, b: Color32) -> bool {
    (theme::luminance(a) - theme::luminance(b)).abs() < 0.25
}

fn palette(c: Couleurs) -> Palette {
    Palette::depuis(&c)
}

// ---------------------------------------------------------------------------------------- choix du thème

#[test]
fn a_fresh_install_gets_shot_and_the_installed_palette_is_orange() {
    assert_eq!(Settings::default().theme.choix, ChoixTheme::Shot);
    assert_eq!(toml::from_str::<Settings>("").unwrap().theme.choix, ChoixTheme::Shot);
    assert_eq!(ChoixTheme::default().nom(), "SHOT");
    theme::installer(&ReglageTheme::default());
    assert_eq!(theme::accent(), ORANGE);
    assert_eq!(theme::fond(), NOIR);
    assert_eq!(theme::texte(), BLANC_CASSE);
    // La liste des Préférences : SHOT d'abord, puis sa variante claire, puis les gris, puis Personnalisé.
    assert_eq!(ChoixTheme::TOUS.map(ChoixTheme::nom), ["SHOT", "SHOT clair", "Gris sombre", "Gris clair", "Sombre", "Personnalisé"]);
}

#[test]
fn a_computer_that_already_chose_a_theme_keeps_it() {
    // Tout choix écrit dans le fichier de réglages est relu tel quel, y compris Gris sombre (l'ancien défaut).
    for choix in ChoixTheme::TOUS {
        let mut s = Settings::default();
        s.theme.choix = choix;
        let texte = toml::to_string(&s).unwrap();
        assert!(texte.contains("[theme]"), "{texte}");
        let relu: Settings = toml::from_str(&texte).unwrap();
        assert_eq!(relu.theme.choix, choix, "{choix:?}");
    }
    for (ecrit, attendu) in [("gris_sombre", ChoixTheme::GrisSombre), ("gris_clair", ChoixTheme::GrisClair), ("sombre", ChoixTheme::Sombre), ("shot", ChoixTheme::Shot), ("shot_clair", ChoixTheme::ShotClair)] {
        let s: Settings = toml::from_str(&format!("[theme]\nchoix = \"{ecrit}\"\n")).unwrap();
        assert_eq!(s.theme.choix, attendu);
    }
    // Le fichier d'un poste qui avait gardé Gris sombre (écrit avant la charte, avec ses neuf couleurs) : Gris sombre.
    let ancien = "[theme]\nchoix = \"gris_sombre\"\n[theme.perso]\npanneau = [64, 64, 64]\nentete = [54, 54, 54]\nfenetre = [56, 56, 56]\nwidgets = [90, 90, 90]\ntexte = [230, 230, 230]\naccent = [200, 200, 200]\nfond = [46, 46, 46]\ntimeline = [72, 72, 72]\nbordure = [80, 80, 80]\n";
    let s: Settings = toml::from_str(ancien).unwrap();
    assert_eq!(s.theme.choix, ChoixTheme::GrisSombre);
    assert_eq!(s.theme.couleurs(), Couleurs::gris_sombre());
    assert!(!Palette::depuis(&s.theme.couleurs()).identite, "pas d'orange dans un thème d'avant la charte");
    // Un thème personnalisé d'avant la charte garde ses neuf couleurs, sans l'identité ni le gris scène.
    let perso = ancien.replace("gris_sombre", "personnalise").replace("[200, 200, 200]", "[10, 20, 30]");
    let s: Settings = toml::from_str(&perso).unwrap();
    assert_eq!(s.theme.choix, ChoixTheme::Personnalise);
    let c = s.theme.couleurs();
    assert_eq!((c.accent, c.panneau, c.doux, c.identite), ([10, 20, 30], [64, 64, 64], None, false));
    // Un thème personnalisé qui part de SHOT garde, lui, l'identité (et la relit).
    let mut r = ReglageTheme::default();
    let mut c = r.couleurs();
    c.fond = [30, 10, 10];
    r.modifier(c);
    assert_eq!(r.choix, ChoixTheme::Personnalise);
    let mut s = Settings::default();
    s.theme = r;
    let relu: Settings = toml::from_str(&toml::to_string(&s).unwrap()).unwrap();
    assert_eq!(relu.theme, r);
    assert!(relu.theme.couleurs().identite && relu.theme.couleurs().doux == Some(rvb(GRIS_SCENE)));
}

// ---------------------------------------------------------------------------------------- couleurs

#[test]
fn the_shot_theme_is_built_on_the_four_colours_of_the_charter() {
    let c = Couleurs::shot();
    assert_eq!((c.fond, c.texte, c.accent, c.doux), (rvb(NOIR), rvb(BLANC_CASSE), rvb(ORANGE), Some(rvb(GRIS_SCENE))));
    // Les fonds sont des gris très foncés déclinés du noir cinéma : du plus sombre (autour du dessin) au plus clair
    // (les boutons), tous distincts les uns des autres de plusieurs crans.
    let p = palette(c);
    let niveaux = [p.fond, p.entete, p.panneau, p.controle, p.bord];
    for n in niveaux {
        assert!(n.r() == n.g() && n.g() == n.b() && n.r() <= 0x40, "{n:?} : un gris très foncé");
    }
    assert!(niveaux.windows(2).all(|w| w[1].r() >= w[0].r() + 5), "fonds distincts, du plus sombre au plus clair : {niveaux:?}");
    for (a, b, nom) in [(p.panneau, p.entete, "panneau et barre"), (p.panneau, p.rangee, "panneau et rangée"), (p.panneau, p.fenetre, "panneau et fenêtre"), (p.panneau, p.controle, "panneau et bouton"), (p.panneau, p.fond, "panneau et fond")] {
        assert!((a.r() as i32 - b.r() as i32).abs() >= 5, "{nom} se distinguent : {a:?} {b:?}");
    }
    // Le texte secondaire et les icônes au repos : le gris scène lui-même quand il se lit assez.
    assert_eq!(p.texte_doux, GRIS_SCENE);
    assert_eq!(p.texte, BLANC_CASSE);
    assert_eq!(p.accent, ORANGE);
    // La variante claire : fonds blanc cassé, texte noir cinéma, même accent.
    let l = Couleurs::shot_clair();
    assert_eq!((l.panneau, l.texte, l.accent), (rvb(BLANC_CASSE), rvb(NOIR), rvb(ORANGE)));
    let p = palette(l);
    assert!(p.claire);
    assert_eq!((p.texte, p.accent), (NOIR, ORANGE));
    assert!(contraste(p.texte_doux, p.panneau) >= 4.5 && p.texte_doux.r() <= GRIS_SCENE.r(), "le gris scène est foncé jusqu'à se lire sur le clair : {:?}", p.texte_doux);
}

#[test]
fn shot_texts_reach_4_5_to_1_on_every_background_in_both_variants() {
    for (nom, c) in [("SHOT", Couleurs::shot()), ("SHOT clair", Couleurs::shot_clair())] {
        let p = palette(c);
        for fond in [p.panneau, p.entete, p.fenetre, p.rangee, p.rangee_sombre, p.controle, p.controle_survol, p.eleve, p.champ, p.fond] {
            assert!(contraste(p.texte, fond) >= 4.5, "{nom} : texte sur {fond:?} : {:.2}", contraste(p.texte, fond));
        }
        // Texte secondaire et icônes au repos : sur les panneaux, les barres, les menus et les rangées.
        for fond in [p.panneau, p.entete, p.fenetre, p.rangee, p.rangee_sombre] {
            assert!(contraste(p.texte_doux, fond) >= 4.5, "{nom} : texte secondaire sur {fond:?} : {:.2}", contraste(p.texte_doux, fond));
        }
        assert!(contraste(p.texte_fort, p.controle_actif) >= 4.5, "{nom} : bouton enfoncé");
        // Ce qui s'écrit sur l'orange (élément choisi, bouton principal, repos, survol, appui).
        for fond in [p.accent, p.accent_survol, p.accent_actif] {
            assert!(contraste(p.sur_accent, fond) >= 4.5, "{nom} : texte sur {fond:?} : {:.2}", contraste(p.sur_accent, fond));
        }
        // L'orange des traits et des textes sur le panneau (liens, filets) ; le contour des éléments choisis sur les rangées.
        assert!(contraste(p.accent_texte, p.panneau) >= 4.5, "{nom} : orange sur le panneau");
        for fond in [p.panneau, p.entete, p.rangee, p.rangee_sombre] {
            assert!(contraste(p.contour, fond) >= 3.0, "{nom} : contour sur {fond:?} : {:.2}", contraste(p.contour, fond));
        }
    }
    // Sur le thème sombre, l'orange est l'orange de la charte tel quel, partout.
    let p = palette(Couleurs::shot());
    assert_eq!((p.accent, p.accent_texte, p.contour), (ORANGE, ORANGE, ORANGE));
    assert_eq!(p.sur_accent, NOIR, "noir cinéma sur l'orange (6,2:1 ; le blanc n'aurait que 3,1:1)");
}

// ---------------------------------------------------------------------------------------- ce que l'orange touche

#[test]
fn orange_marks_what_is_chosen_and_leaves_the_working_colours_alone() {
    let (shot, gris) = (palette(Couleurs::shot()), palette(Couleurs::gris_sombre()));
    // Ce que l'accent marque : la sélection d'egui, l'élément choisi (contour des clés, plans, séquences, calques), le filet
    // du survol d'un bouton, le bouton principal.
    let v = shot.visuels();
    assert_eq!((v.selection.bg_fill, v.selection.stroke.color), (ORANGE, shot.sur_accent));
    assert_eq!(v.widgets.hovered.bg_stroke.color, shot.accent_texte);
    assert_eq!(shot.contour, ORANGE);
    // Les autres thèmes ne changent pas : contour blanc, survol sans accent.
    assert_eq!(gris.contour, Color32::WHITE);
    assert_eq!(gris.visuels().widgets.hovered.bg_stroke.color, gris.controle_actif);
    // Ce que l'orange ne touche pas : les couleurs du travail sont celles d'avant (feuille et pelures d'oignon sont des
    // données du document ; calques, repères de caméra, rouge des alertes viennent de la palette).
    // (Les repères vifs de la timeline sont poussés vers le blanc jusqu'à 3,2:1 sur les rangées : sur les rangées plus
    // sombres de SHOT ils restent un peu plus saturés, de la même teinte.)
    let dominante = |c: Color32| [c.r(), c.g(), c.b()].iter().enumerate().max_by_key(|(_, v)| **v).map(|(i, _)| i).unwrap();
    for (nom, a, b) in [
        ("dessin vectoriel", shot.dessin_vectoriel, gris.dessin_vectoriel),
        ("dessin bitmap", shot.dessin_bitmap, gris.dessin_bitmap),
        ("calque de couleur", shot.dessin_couleur, gris.dessin_couleur),
        ("calque d'effet", shot.dessin_cfx, gris.dessin_cfx),
        ("annotation", shot.dessin_annotation, gris.dessin_annotation),
        ("calques de décor", shot.decor, gris.decor),
        ("image courante", shot.image_courante, gris.image_courante),
        ("clés de caméra", shot.camera, gris.camera),
        ("marques d'image", shot.marque, gris.marque),
        ("clés d'effets", shot.effet, gris.effet),
    ] {
        assert_eq!(dominante(a), dominante(b), "{nom} garde sa teinte avec la charte : {a:?} contre {b:?}");
        assert!(luminance_proche(a, b), "{nom} : {a:?} contre {b:?}");
    }
    for (nom, a, b) in [
        ("calque vectoriel (sourd)", shot.calque_vectoriel, gris.calque_vectoriel),
        ("calque bitmap (sourd)", shot.calque_bitmap, gris.calque_bitmap),
        ("calque de couleur (sourd)", shot.calque_couleur, gris.calque_couleur),
        ("son muet", shot.son_muet, gris.son_muet),
        ("erreur", shot.erreur, gris.erreur),
        ("avertissement", shot.avertissement, gris.avertissement),
        ("succès", shot.succes, gris.succes),
    ] {
        assert_eq!(a, b, "{nom} ne change pas avec la charte");
    }
    // Le rouge des alertes reste rouge, et n'est pas l'orange.
    assert!(shot.erreur.r() > 200 && shot.erreur.g() < 120 && shot.erreur != ORANGE);
}

/// Les formes à plat (`Shape::Vec` déroulés).
fn a_plat<'a>(formes: &'a [egui::epaint::ClippedShape]) -> Vec<&'a Shape> {
    fn va<'a>(s: &'a Shape, out: &mut Vec<&'a Shape>) {
        match s {
            Shape::Vec(v) => v.iter().for_each(|s| va(s, out)),
            autre => out.push(autre),
        }
    }
    let mut out = Vec::new();
    for c in formes {
        va(&c.shape, &mut out);
    }
    out
}

/// Surface totale des rectangles pleins de cette couleur exacte, en points carrés.
fn surface_de(formes: &[egui::epaint::ClippedShape], c: Color32) -> f32 {
    a_plat(formes).into_iter().filter_map(|s| if let Shape::Rect(r) = s { (r.fill == c).then(|| r.rect.width() * r.rect.height()) } else { None }).sum()
}

fn texte_a<'a>(formes: &'a [&'a Shape], texte: &str) -> Option<(&'a egui::epaint::TextShape, Rect)> {
    formes.iter().find_map(|s| match s {
        Shape::Text(t) if t.galley.job.text == texte => Some((t, Rect::from_min_size(t.pos, t.galley.size()))),
        _ => None,
    })
}

#[test]
fn orange_never_fills_a_large_surface_in_the_main_window() {
    let mut b = banc_taille(crate::vignettes_tests::quatre_plans(), false, 1600, 900);
    b.garder_formes = true;
    let ecran = b.taille.x * b.taille.y;
    for espace in [Espace::Vignettes, Espace::Storyboard, Espace::Animation, Espace::Montage] {
        b.app.set_espace(espace);
        b.chauffer(6);
        let orange = surface_de(&b.formes, ORANGE);
        assert!(orange < ecran * 0.02, "{} : l'orange remplit {:.1} % de la fenêtre", espace.nom(), orange / ecran * 100.0);
        // L'espace de travail actif est marqué d'orange : son nom est écrit en noir cinéma sur un fond orange.
        let formes = a_plat(&b.formes);
        let (texte, _) = texte_a(&formes, &format!(" {} ", espace.nom())).unwrap_or_else(|| panic!("onglet {}", espace.nom()));
        assert_eq!(texte.galley.job.sections[0].format.color, theme::sur_accent(), "{}", espace.nom());
        assert!(orange > 0.0, "{} : l'espace actif est orange", espace.nom());
    }
}

// ---------------------------------------------------------------------------------------- logo et icône

/// Pixel RVBA (non prémultiplié) d'une icône carrée de `t` pixels.
fn pixel(px: &[u8], t: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * t + x) * 4) as usize;
    [px[i], px[i + 1], px[i + 2], px[i + 3]]
}

#[test]
fn the_logo_uses_the_official_colours() {
    let (w, h, px) = logo::image(948, Variante::Sombre);
    let s = w as f32 / logo::VUE_L;
    let au = |px: &[u8], x: f32, y: f32| {
        let (ix, iy) = ((((x - logo::VUE_X) * s) as u32).min(w - 1), (((y - logo::VUE_Y) * s) as u32).min(h - 1));
        let i = ((iy * w + ix) * 4) as usize;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    };
    // Variante sombre : blanc cassé #F4F1EA (lettres, angles, barre de « STORY » et devise) pour poser sur du noir cinéma ;
    // « STORY » est en creux, du noir cinéma #0E0E0E.
    assert_eq!(au(&px, 407.0, 300.0), [0xF4, 0xF1, 0xEA, 255], "montant du H");
    assert_eq!(au(&px, 250.0, 215.0), [0xF4, 0xF1, 0xEA, 255], "angle haut gauche");
    assert_eq!(au(&px, 606.0, 442.0), [0xF4, 0xF1, 0xEA, 255], "barre de STORY");
    let creux = (437..449).flat_map(|y| (609..695).map(move |x| (x, y))).filter(|&(x, y)| au(&px, x as f32, y as f32) == [0x0E, 0x0E, 0x0E, 255]).count();
    assert!(creux > 100, "« STORY » en noir cinéma dans la barre : {creux} pixels");
    // Variante claire : l'inverse (noir cinéma sur blanc cassé).
    let (w2, h2, px2) = logo::image(948, Variante::Clair);
    assert_eq!((w, h), (w2, h2));
    assert_eq!(au(&px2, 407.0, 300.0), [0x0E, 0x0E, 0x0E, 255]);
    assert_eq!(au(&px2, 606.0, 442.0), [0x0E, 0x0E, 0x0E, 255]);
    let creux = (437..449).flat_map(|y| (609..695).map(move |x| (x, y))).filter(|&(x, y)| au(&px2, x as f32, y as f32) == [0xF4, 0xF1, 0xEA, 255]).count();
    assert!(creux > 100, "« STORY » en blanc cassé dans la barre sombre : {creux} pixels");
}

#[test]
fn the_application_icon_is_the_orange_square_with_a_black_s_and_two_corners() {
    for t in [32u32, 64, 256] {
        let px = logo::icone(t);
        assert_eq!(px, logo::icone(t), "déterministe");
        // Le haut du carré, au milieu : l'orange #E76F3C, plein.
        assert_eq!(pixel(&px, t, t / 2, 1), [0xE7, 0x6F, 0x3C, 255], "fond orange à {t} px");
        // Le coin du carré est arrondi (transparent), et l'arrondi est grand : à 12 % du côté du coin, rien encore.
        assert_eq!(pixel(&px, t, 0, 0)[3], 0, "{t}");
        if t == 256 {
            assert_eq!(pixel(&px, t, 12, 12)[3], 0, "rayon d'environ 22 % du côté");
            assert_eq!(pixel(&px, t, 24, 24)[3], 255);
        }
        // L'angle haut gauche : noir cinéma, au centre de sa barre.
        let (x, y) = ((t as f32 * 0.16) as u32, (t as f32 * 0.16) as u32);
        assert_eq!(&pixel(&px, t, x, y)[..3], &[0x0E, 0x0E, 0x0E], "angle à {t} px");
        // Au centre du carré (où passe le « S ») : de l'encre noir cinéma et du fond orange, et rien d'autre.
        let c = t / 2;
        let r = (t / 14).max(2);
        let mut vus = std::collections::BTreeSet::new();
        for yy in c - r..=c + r {
            for xx in c - r..=c + r {
                let p = pixel(&px, t, xx, yy);
                if p[3] == 255 {
                    vus.insert([p[0], p[1], p[2]]);
                }
            }
        }
        assert!(vus.contains(&[0x0E, 0x0E, 0x0E]) && vus.contains(&[0xE7, 0x6F, 0x3C]), "le centre mêle le S noir et le fond orange à {t} px : {vus:?}");
        // Orange et noir en proportions raisonnables.
        let n = px.chunks_exact(4).filter(|p| p[3] == 255).count() as f32;
        let noir = px.chunks_exact(4).filter(|p| p[3] == 255 && p[0] < 0x40).count() as f32;
        assert!((0.05..0.40).contains(&(noir / n)), "part d'encre à {t} px : {}", noir / n);
    }
    // La variante sombre existe, sans être utilisée : carré noir cinéma, S et angles blanc cassé.
    let px = logo::icone_sombre(256);
    assert_eq!(pixel(&px, 256, 128, 1), [0x0E, 0x0E, 0x0E, 255]);
    let (x, y) = (41, 41);
    assert_eq!(&pixel(&px, 256, x, y)[..3], &[0xF4, 0xF1, 0xEA]);
    assert_ne!(logo::icone_sombre(64), logo::icone(64));
    // L'icône de l'exécutable (`.ico`) est celle de l'application : orange.
    let ico = logo::ico(&[48]);
    let dib = &ico[6 + 16..];
    // Pixels BVRA de bas en haut : la ligne du haut est la dernière ; son milieu est orange.
    let (t, haut) = (48usize, 40 + 47 * 48 * 4 + 24 * 4);
    assert_eq!((dib[haut], dib[haut + 1], dib[haut + 2], dib[haut + 3]), (0x3C, 0x6F, 0xE7, 255), "{t}");
}

#[test]
fn the_small_logotype_is_the_word_and_two_corners_without_the_tagline() {
    let (w, h, px) = logo::logotype(250, Variante::Sombre, Some(rvb(ORANGE)));
    assert_eq!((w, h), (457, 250));
    let s = h as f32 / logo::LOGOTYPE_H;
    let au = |x: f32, y: f32| {
        let (ix, iy) = ((((x - logo::LOGOTYPE_X) * s) as u32).min(w - 1), (((y - logo::LOGOTYPE_Y) * s) as u32).min(h - 1));
        let i = ((iy * w + ix) * 4) as usize;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    };
    assert_eq!(au(407.0, 300.0), [0xF4, 0xF1, 0xEA, 255], "les lettres sont blanc cassé");
    assert_eq!(au(250.0, 215.0), [0xE7, 0x6F, 0x3C, 255], "angle haut gauche orange");
    assert_eq!(au(690.0, 445.0), [0xE7, 0x6F, 0x3C, 255], "angle bas droit orange");
    // Sans la devise : rien sous les lettres, à gauche (là où est « FRAME THE »), et la barre de STORY est vide de texte.
    assert_eq!(au(450.0, 443.0)[3], 0, "pas de FRAME THE");
    // Sans couleur d'angles : tout d'une couleur (thèmes sans identité).
    let (_, _, px) = logo::logotype(250, Variante::Sombre, None);
    assert!(px.chunks_exact(4).filter(|p| p[3] == 255).all(|p| [p[0], p[1], p[2]] == [0xF4, 0xF1, 0xEA]));
    // La variante claire : lettres noir cinéma.
    let (_, _, px) = logo::logotype(250, Variante::Clair, None);
    assert!(px.chunks_exact(4).filter(|p| p[3] == 255).all(|p| [p[0], p[1], p[2]] == [0x0E, 0x0E, 0x0E]));
    // Les proportions suivent le cadre, à toute taille.
    for hauteur in [20u32, 30, 60] {
        let (w, h, _) = logo::logotype(hauteur, Variante::Sombre, None);
        assert!((w as f32 / h as f32 - logo::LOGOTYPE_L / logo::LOGOTYPE_H).abs() < 0.06, "{w}×{h}");
    }
}

// ---------------------------------------------------------------------------------------- la fenêtre

/// L'image du logotype de la barre, à l'écran : le rectangle de l'image dont la texture est celle du logotype.
fn logotype_a_l_ecran(b: &Banc) -> Option<Rect> {
    let (texture, _, _): (egui::TextureHandle, u32, u32) = b.ctx.data(|d| d.get_temp::<((u32, bool, Option<[u8; 3]>), egui::TextureHandle, u32, u32)>(egui::Id::new("shot_logotype"))).map(|(_, t, w, h)| (t, w, h))?;
    a_plat(&b.formes).into_iter().find_map(|s| match s {
        Shape::Rect(r) if r.brush.as_ref().is_some_and(|br| br.fill_texture_id == texture.id()) => Some(r.rect),
        _ => None,
    })
}

#[test]
fn the_small_logotype_sits_first_in_the_menu_bar_without_crowding_it_at_150_percent() {
    // 1280 points de large : la fenêtre d'un écran de 1920 pixels à 150 %.
    let mut b = banc_taille(Document::with_frame(160, 90, 24), false, 1280, 720);
    b.garder_formes = true;
    b.chauffer(6);
    let logotype = logotype_a_l_ecran(&b).expect("le logotype est peint dans la barre de menus");
    let formes = a_plat(&b.formes);
    let (_, fichier) = texte_a(&formes, "Fichier").expect("menu Fichier");
    let (_, aide) = texte_a(&formes, "Aide").expect("menu Aide");
    let (_, vignettes) = texte_a(&formes, " Vignettes ").expect("onglet Vignettes");
    assert!(logotype.right() < fichier.left(), "le logotype est à gauche des menus : {logotype:?} {fichier:?}");
    assert!(logotype.left() < 12.0, "tout à gauche de la barre : {logotype:?}");
    assert!(logotype.height() <= 22.0 && logotype.top() >= 0.0 && logotype.bottom() <= 30.0, "à la hauteur de la barre : {logotype:?}");
    // Il ne pousse pas les menus plus qu'une cinquantaine de points, et les espaces de travail restent à droite des menus.
    assert!(fichier.left() < logotype.width() + 30.0, "Fichier à {} pt", fichier.left());
    assert!(aide.right() + 20.0 < vignettes.left(), "les menus et les espaces de travail ne se touchent pas : {aide:?} {vignettes:?}");
    // Rendu une fois et gardé : la même texture d'une image à l'autre.
    let avant: egui::TextureId = b.ctx.data(|d| d.get_temp::<((u32, bool, Option<[u8; 3]>), egui::TextureHandle, u32, u32)>(egui::Id::new("shot_logotype"))).unwrap().1.id();
    b.chauffer(5);
    let apres: egui::TextureId = b.ctx.data(|d| d.get_temp::<((u32, bool, Option<[u8; 3]>), egui::TextureHandle, u32, u32)>(egui::Id::new("shot_logotype"))).unwrap().1.id();
    assert_eq!(avant, apres);
}

fn accueil() -> Banc {
    let mut b = banc_taille(Document::with_frame(160, 90, 24), false, 1600, 900);
    b.garder_formes = true;
    b.app.welcome = true;
    b.chauffer(6);
    b
}

#[test]
fn the_welcome_card_follows_the_theme_and_new_project_is_the_orange_button() {
    let b = accueil();
    assert!(b.app.welcome_shown());
    let p = theme::palette();
    let formes = a_plat(&b.formes);
    // « Nouveau projet » : un grand bouton plein d'orange, son texte en noir cinéma. « Ouvrir un projet » : un bouton ordinaire.
    let (titre, titre_rect) = texte_a(&formes, "Nouveau projet").expect("Nouveau projet");
    assert_eq!(titre.fallback_color, p.sur_accent);
    let bouton = formes
        .iter()
        .find_map(|s| match s {
            Shape::Rect(r) if r.fill == p.accent && r.rect.width() > 300.0 && r.rect.contains(titre_rect.center()) => Some(r.rect),
            _ => None,
        })
        .expect("le bouton principal est rempli d'orange");
    assert!((bouton.height() - crate::accueil::ENTREE_H).abs() < 1.0);
    let (autre, autre_rect) = texte_a(&formes, "Ouvrir un projet").expect("Ouvrir un projet");
    assert_eq!(autre.fallback_color, p.texte_fort);
    assert!(!bouton.contains(autre_rect.center()));
    let ordinaire = formes.iter().any(|s| matches!(s, Shape::Rect(r) if r.fill == p.controle && r.rect.contains(autre_rect.center())));
    assert!(ordinaire, "l'autre entrée est au repos, de la couleur des boutons");
    // L'orange ne remplit que ce bouton (et de petits repères) : moins de 5 % de l'écran.
    let ecran = b.taille.x * b.taille.y;
    assert!(surface_de(&b.formes, ORANGE) < ecran * 0.05, "{:.1} %", surface_de(&b.formes, ORANGE) / ecran * 100.0);
    // La carte est du fond des fenêtres du thème.
    assert!(formes.iter().any(|s| matches!(s, Shape::Rect(r) if r.fill == p.fenetre && r.rect.width() >= 460.0)), "la carte prend le fond des fenêtres");
}

#[test]
fn the_welcome_card_follows_a_light_theme_too() {
    let mut b = accueil();
    b.app.settings.theme.choisir(ChoixTheme::ShotClair);
    b.chauffer(6);
    let p = theme::palette();
    assert!(p.claire && p.identite);
    let formes = a_plat(&b.formes);
    assert!(formes.iter().any(|s| matches!(s, Shape::Rect(r) if r.fill == p.fenetre && r.rect.width() >= 460.0)), "carte claire");
    let (titre, _) = texte_a(&formes, "Nouveau projet").unwrap();
    assert_eq!(titre.fallback_color, NOIR, "noir cinéma sur l'orange, aussi sur le thème clair");
    assert!(crate::accueil::logo_texture(&b.ctx, 240.0).size()[0] > 0 && !crate::accueil::logo_sombre(), "le logo est celui des fonds clairs");
    theme::installer(&ReglageTheme::default());
}

#[test]
fn the_main_button_of_a_window_is_orange_and_greys_out_when_disabled() {
    theme::installer(&ReglageTheme::default());
    let p = theme::palette();
    let ctx = egui::Context::default();
    ctx.set_visuals(theme::visuels());
    let mut cliques = [None, None];
    let out = ctx.run(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 200.0))), ..Default::default() }, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                cliques[0] = Some(theme::bouton_principal(ui, true, "Exporter").rect);
                cliques[1] = Some(theme::bouton_principal(ui, false, "Valider").rect);
            });
        });
    });
    let formes: Vec<&Shape> = out.shapes.iter().map(|c| &c.shape).collect();
    let plat: Vec<&Shape> = formes
        .iter()
        .flat_map(|s| match s {
            Shape::Vec(v) => v.iter().collect::<Vec<_>>(),
            s => vec![*s],
        })
        .collect();
    let [actif, inactif] = [cliques[0].unwrap(), cliques[1].unwrap()];
    let remplissage = |zone: Rect| plat.iter().find_map(|s| if let Shape::Rect(r) = s { (zone.contains(r.rect.center()) && r.fill != Color32::TRANSPARENT).then_some(r.fill) } else { None });
    assert_eq!(remplissage(actif), Some(p.accent), "actif : fond d'accent");
    assert_ne!(remplissage(inactif), Some(p.accent), "désactivé : grisé");
    let texte = plat.iter().find_map(|s| if let Shape::Text(t) = s { (t.galley.job.text == "Exporter").then_some(t.fallback_color) } else { None });
    assert_eq!(texte, Some(p.sur_accent), "texte noir cinéma sur l'orange");
}

// ---------------------------------------------------------------------------------------- captures (à regarder)

fn dossier_de_captures() -> std::path::PathBuf {
    std::env::var_os("SHOT_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

/// Les deux icônes de la planche en PNG 256 px (`icone-sombre-256.png`, `icone-orange-256.png`) :
/// `SHOT_CAPTURE_DIR=… cargo test --release --offline --target-dir target\msvc charte_icones -- --ignored --nocapture`
#[test]
#[ignore]
fn charte_icones() {
    let dir = dossier_de_captures();
    for (nom, px) in [("icone-sombre-256.png", logo::icone_sombre(256)), ("icone-orange-256.png", logo::icone(256))] {
        image::save_buffer(dir.join(nom), &px, 256, 256, image::ExtendedColorType::Rgba8).unwrap();
        println!("CHARTE {nom}");
    }
}

/// La fenêtre principale (Vignettes, Storyboard, Animation, Montage) et l'accueil, avec le thème SHOT, en PNG :
/// `SHOT_CAPTURE_DIR=… cargo test --release --offline --target-dir target\msvc charte_captures -- --ignored --nocapture`
#[test]
#[ignore]
fn charte_captures() {
    let dir = dossier_de_captures();
    let sauver = |b: &Banc, nom: &str| {
        let Some(rgba) = b.capturer() else { return println!("CHARTE pas de carte graphique : {nom} ignorée") };
        image::save_buffer(dir.join(nom), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
        println!("CHARTE {nom}");
    };
    // (Les thèmes clairs en dernier : la palette courante est propre au fil d'essai, et un banc qui vient de naître ne la remet pas.)
    theme::installer(&ReglageTheme::default());
    let mut a = crate::perf_tests::banc(Document::with_frame(160, 90, 24), true);
    a.app.welcome = true;
    a.chauffer(8);
    sauver(&a, "accueil.png");
    let mut b = crate::perf_tests::banc(crate::vignettes_tests::quatre_plans(), true);
    b.chauffer(6);
    for (espace, nom) in [(Espace::Vignettes, "fenetre-principale-vignettes.png"), (Espace::Storyboard, "fenetre-principale-storyboard.png"), (Espace::Animation, "fenetre-principale-animation.png"), (Espace::Montage, "fenetre-principale-montage.png")] {
        b.app.set_espace(espace);
        b.chauffer(8);
        sauver(&b, nom);
    }
    b.app.settings.theme.choisir(ChoixTheme::ShotClair);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(8);
    sauver(&b, "fenetre-principale-clair.png");
    a.app.settings.theme.choisir(ChoixTheme::ShotClair);
    a.chauffer(8);
    sauver(&a, "accueil-clair.png");
    theme::installer(&ReglageTheme::default());
}