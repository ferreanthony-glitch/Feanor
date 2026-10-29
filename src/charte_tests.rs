//! La charte graphique de SHOT (planche d'Anthony, 2026-10-28, puis sa demande de la rendre plus discrète) : le thème
//! « SHOT » est le gris sombre d'avant avec seulement quelques rehauts orange (défaut d'une installation neuve ; un poste
//! qui a déjà choisi un thème le garde). Ces essais vérifient ses couleurs et ses contrastes, ce que l'orange touche (très
//! peu) et ce qu'il ne touche pas (la sélection, les éléments choisis, les boutons), le logo et l'icône aux couleurs
//! officielles, le petit logotype de la barre de menus, le trait sous l'espace actif et l'accueil. Les captures d'écran
//! (essais ignorés) vont dans `SHOT_CAPTURE_DIR`.

use eframe::egui::{self, epaint::Shape, Color32, Rect};

use crate::document::Document;
use crate::logo::{self, Variante};
use crate::perf_tests::{banc_taille, Banc};
use crate::settings::Settings;
use crate::theme::{self, contraste, ChoixTheme, Couleurs, Palette, ReglageTheme};
use crate::workspace::Espace;

const BLANC_CASSE: Color32 = Color32::from_rgb(0xF4, 0xF1, 0xEA);
const ORANGE: Color32 = Color32::from_rgb(0xE7, 0x6F, 0x3C);

fn rvb(c: Color32) -> [u8; 3] {
    [c.r(), c.g(), c.b()]
}

fn palette(c: Couleurs) -> Palette {
    Palette::depuis(&c)
}

/// Cette couleur est-elle un orange (de la charte ou de la famille) ?
fn orangee(c: Color32) -> bool {
    c.r() >= 180 && (80..=170).contains(&c.g()) && c.b() <= 110 && c.r() as i32 - c.b() as i32 >= 100
}

// ---------------------------------------------------------------------------------------- choix du thème

#[test]
fn a_fresh_install_gets_shot_which_is_dark_grey_with_orange_touches() {
    assert_eq!(Settings::default().theme.choix, ChoixTheme::Shot);
    assert_eq!(toml::from_str::<Settings>("").unwrap().theme.choix, ChoixTheme::Shot);
    assert_eq!(ChoixTheme::default().nom(), "SHOT");
    theme::installer(&ReglageTheme::default());
    let (shot, gris) = (theme::palette(), palette(Couleurs::gris_sombre()));
    assert_eq!((shot.panneau, shot.fond, shot.accent), (gris.panneau, gris.fond, gris.accent));
    assert_eq!(theme::rehaut(), ORANGE);
    // La liste des Préférences : SHOT d'abord, puis les trois gris, puis Personnalisé (« SHOT clair » n'existe plus : un clair
    // de la charte n'aurait été que « Gris clair » avec les mêmes rehauts).
    assert_eq!(ChoixTheme::TOUS.map(ChoixTheme::nom), ["SHOT", "Gris sombre", "Gris clair", "Sombre", "Personnalisé"]);
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
    for (ecrit, attendu) in [("gris_sombre", ChoixTheme::GrisSombre), ("gris_clair", ChoixTheme::GrisClair), ("sombre", ChoixTheme::Sombre), ("shot", ChoixTheme::Shot)] {
        let s: Settings = toml::from_str(&format!("[theme]\nchoix = \"{ecrit}\"\n")).unwrap();
        assert_eq!(s.theme.choix, attendu);
    }
    // Le fichier d'un poste qui avait gardé Gris sombre (écrit avant la charte, avec ses neuf couleurs) : Gris sombre.
    let ancien = "[theme]\nchoix = \"gris_sombre\"\n[theme.perso]\npanneau = [64, 64, 64]\nentete = [54, 54, 54]\nfenetre = [56, 56, 56]\nwidgets = [90, 90, 90]\ntexte = [230, 230, 230]\naccent = [200, 200, 200]\nfond = [46, 46, 46]\ntimeline = [72, 72, 72]\nbordure = [80, 80, 80]\n";
    let s: Settings = toml::from_str(ancien).unwrap();
    assert_eq!(s.theme.choix, ChoixTheme::GrisSombre);
    assert_eq!(s.theme.couleurs(), Couleurs::gris_sombre());
    assert!(!Palette::depuis(&s.theme.couleurs()).identite, "pas d'orange dans un thème d'avant la charte");
    // Un thème personnalisé d'avant la charte garde ses neuf couleurs, sans rehauts orange.
    let perso = ancien.replace("gris_sombre", "personnalise").replace("[200, 200, 200]", "[10, 20, 30]");
    let s: Settings = toml::from_str(&perso).unwrap();
    assert_eq!(s.theme.choix, ChoixTheme::Personnalise);
    let c = s.theme.couleurs();
    assert_eq!((c.accent, c.panneau, c.identite), ([10, 20, 30], [64, 64, 64], false));
    // Un thème personnalisé qui part de SHOT garde, lui, les rehauts (et les relit).
    let mut r = ReglageTheme::default();
    let mut c = r.couleurs();
    c.fond = [30, 10, 10];
    r.modifier(c);
    assert_eq!(r.choix, ChoixTheme::Personnalise);
    let mut s = Settings::default();
    s.theme = r;
    let relu: Settings = toml::from_str(&toml::to_string(&s).unwrap()).unwrap();
    assert_eq!(relu.theme, r);
    assert!(relu.theme.couleurs().identite);
}

// ---------------------------------------------------------------------------------------- couleurs

#[test]
fn the_shot_theme_is_the_dark_grey_of_before_with_a_slightly_warmer_text() {
    let (c, g) = (Couleurs::shot(), Couleurs::gris_sombre());
    // Les fonds, les boutons, les bordures et la sélection sont ceux de « Gris sombre » (valeurs identiques).
    assert_eq!((c.panneau, c.entete, c.fenetre, c.widgets, c.fond, c.timeline, c.bordure, c.accent), (g.panneau, g.entete, g.fenetre, g.widgets, g.fond, g.timeline, g.bordure, g.accent));
    // Plus de noir lourd : le fond le plus sombre est un gris moyen-foncé, et le panneau reste autour de #40.
    let p = palette(c);
    for fond in [p.fond, p.entete, p.panneau, p.fenetre, p.rangee, p.controle] {
        assert!(fond.r() >= 0x2A, "{fond:?} : pas de noir cinéma pour l'interface");
    }
    assert!((0x3a..=0x44).contains(&p.panneau.r()));
    // Le texte est le blanc cassé de la charte, à peine différent du gris très clair d'avant.
    assert_eq!(c.texte, rvb(BLANC_CASSE));
    let avant = palette(g).texte;
    assert!(contraste(p.texte, avant) < 1.2, "le texte change à peine : {:?} contre {avant:?}", p.texte);
    // Les rehauts : l'orange de la charte, et lui seul.
    assert!(c.identite && p.identite && !g.identite);
    assert_eq!(p.rehaut, ORANGE);
    assert_eq!(palette(g).rehaut, palette(g).accent_texte, "sans identité, le rehaut est l'accent ordinaire");
}

#[test]
fn shot_texts_stay_readable_and_the_orange_touches_stay_visible() {
    let p = palette(Couleurs::shot());
    for fond in [p.panneau, p.entete, p.fenetre, p.rangee, p.rangee_sombre, p.controle, p.controle_survol, p.eleve, p.champ, p.fond] {
        assert!(contraste(p.texte, fond) >= 4.5, "texte sur {fond:?} : {:.2}", contraste(p.texte, fond));
    }
    for fond in [p.panneau, p.entete, p.fenetre, p.rangee, p.rangee_sombre] {
        assert!(contraste(p.texte_doux, fond) >= 4.5, "texte secondaire sur {fond:?} : {:.2}", contraste(p.texte_doux, fond));
    }
    assert!(contraste(p.sur_accent, p.accent) >= 4.5, "ce qui s'écrit sur la sélection");
    // Le trait orange sous l'espace actif, l'icône de « Nouveau projet » et les angles du logotype : 3:1 au moins sur les
    // panneaux et les barres (le minimum d'un élément graphique).
    for fond in [p.panneau, p.entete, p.fenetre] {
        assert!(contraste(p.rehaut, fond) >= 3.0, "rehaut sur {fond:?} : {:.2}", contraste(p.rehaut, fond));
    }
}

// ---------------------------------------------------------------------------------------- ce que l'orange touche

#[test]
fn orange_is_only_a_touch_selection_and_buttons_keep_their_greys() {
    let (shot, gris) = (palette(Couleurs::shot()), palette(Couleurs::gris_sombre()));
    // La sélection, les éléments choisis, les boutons : exactement ceux de Gris sombre (sélection grise, contour blanc).
    assert_eq!((shot.accent, shot.accent_texte, shot.sur_accent, shot.contour), (gris.accent, gris.accent_texte, gris.sur_accent, gris.contour));
    assert_eq!(shot.contour, Color32::WHITE);
    let v = shot.visuels();
    assert_eq!((v.selection.bg_fill, v.selection.stroke.color), (gris.accent, gris.sur_accent));
    assert_eq!(v.widgets.hovered.bg_stroke.color, shot.controle_actif, "pas de filet orange au survol");
    assert_eq!((v.widgets.inactive.weak_bg_fill, v.widgets.hovered.weak_bg_fill, v.widgets.active.weak_bg_fill), (shot.controle, shot.controle_survol, shot.controle_actif));
    // Aucune couleur de la palette, hors le rehaut, n'est orange : fonds, boutons, texte, sélection, contour, bords.
    for (nom, c) in [
        ("fond", shot.fond),
        ("entête", shot.entete),
        ("panneau", shot.panneau),
        ("fenêtre", shot.fenetre),
        ("élevé", shot.eleve),
        ("bord", shot.bord),
        ("champ", shot.champ),
        ("bouton", shot.controle),
        ("bouton survolé", shot.controle_survol),
        ("bouton enfoncé", shot.controle_actif),
        ("texte", shot.texte),
        ("texte secondaire", shot.texte_doux),
        ("texte fort", shot.texte_fort),
        ("accent", shot.accent),
        ("texte d'accent", shot.accent_texte),
        ("contour", shot.contour),
        ("rangée", shot.rangee),
        ("bloc", shot.bloc),
        ("poignée", shot.poignee),
    ] {
        assert!(!orangee(c), "{nom} n'est pas orange : {c:?}");
    }
    // Les couleurs du travail sont celles d'avant (feuille et pelures d'oignon sont des données du document ; calques,
    // repères de caméra, rouge des alertes viennent de la palette, identiques puisque les fonds le sont).
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
fn orange_is_one_thin_line_in_the_main_window_under_the_active_workspace() {
    let mut b = banc_taille(crate::vignettes_tests::quatre_plans(), false, 1600, 900);
    b.garder_formes = true;
    let ecran = b.taille.x * b.taille.y;
    for espace in [Espace::Vignettes, Espace::Storyboard, Espace::Animation, Espace::Montage] {
        b.app.set_espace(espace);
        b.chauffer(6);
        let formes = a_plat(&b.formes);
        // Le seul rectangle plein orange de la fenêtre : le trait, mince, sous le mot de l'espace actif.
        let traits: Vec<Rect> = formes.iter().filter_map(|s| if let Shape::Rect(r) = s { (r.fill == ORANGE).then_some(r.rect) } else { None }).collect();
        assert_eq!(traits.len(), 1, "{} : un seul trait orange : {traits:?}", espace.nom());
        let trait_ = traits[0];
        assert!(trait_.height() <= 2.5 && trait_.width() < 120.0, "{} : un mince trait : {trait_:?}", espace.nom());
        let (_, mot) = texte_a(&formes, &format!(" {} ", espace.nom())).unwrap_or_else(|| panic!("onglet {}", espace.nom()));
        assert!(trait_.center().x > mot.left() && trait_.center().x < mot.right(), "{} : sous le mot actif : {trait_:?} contre {mot:?}", espace.nom());
        assert!(trait_.center().y > mot.center().y && trait_.center().y < mot.bottom() + 6.0, "{} : au bas du mot : {trait_:?} contre {mot:?}", espace.nom());
        // Les autres espaces n'ont pas de trait.
        for autre in [Espace::Vignettes, Espace::Storyboard, Espace::Animation, Espace::Montage] {
            if autre != espace {
                let (_, m) = texte_a(&formes, &format!(" {} ", autre.nom())).unwrap();
                assert!(!(trait_.center().x > m.left() && trait_.center().x < m.right()), "{} actif : {} n'a pas de trait", espace.nom(), autre.nom());
            }
        }
        // Très peu d'orange plein : le trait seul, bien moins de 0,5 % de la fenêtre.
        let orange = surface_de(&b.formes, ORANGE);
        assert!(orange < ecran * 0.005, "{} : l'orange remplit {:.3} % de la fenêtre", espace.nom(), orange / ecran * 100.0);
        // Aucun contour ni trait orange : la sélection et les éléments choisis ne sont pas orange (ni crème changé).
        let trait_orange = formes.iter().any(|s| match s {
            Shape::Rect(r) => r.stroke.color == ORANGE,
            Shape::LineSegment { stroke, .. } => stroke.color == ORANGE,
            Shape::Path(p) => matches!(p.stroke.color, egui::epaint::ColorMode::Solid(c) if c == ORANGE),
            _ => false,
        });
        assert!(!trait_orange, "{} : aucun contour orange", espace.nom());
        // Le mot actif garde son fond de sélection gris, texte foncé dessus, comme avant la charte.
        let (t, _) = texte_a(&formes, &format!(" {} ", espace.nom())).unwrap();
        assert_eq!(t.galley.job.sections[0].format.color, theme::sur_accent(), "{}", espace.nom());
        assert!(formes.iter().any(|s| matches!(s, Shape::Rect(r) if r.fill == theme::accent() && r.rect.intersects(mot))), "{} : fond de sélection gris", espace.nom());
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


/// La couleur du texte d'un `Label` : celle de sa section (`RichText::color`).
fn couleur_du_texte(t: &egui::epaint::TextShape) -> Color32 {
    t.galley.job.sections.first().map(|s| s.format.color).filter(|c| *c != Color32::PLACEHOLDER).unwrap_or(t.fallback_color)
}

fn accueil() -> Banc {
    let mut b = banc_taille(Document::with_frame(160, 90, 24), false, 1000, 620);
    b.garder_formes = true;
    b.app.welcome = true;
    b.chauffer(6);
    b
}

#[test]
fn the_welcome_page_has_two_framed_panels_in_the_theme_colours_and_no_orange_fill() {
    let b = accueil();
    assert!(b.app.welcome_shown());
    let p = theme::palette();
    let formes = a_plat(&b.formes);
    let (titre, titre_rect) = texte_a(&formes, "Nouveau projet").expect("Nouveau projet");
    let (recents, recents_rect) = texte_a(&formes, "Projets récents").expect("Projets récents");
    // Les titres des deux cadres sont en texte fort, le cadre de gauche est à gauche de celui de droite.
    assert_eq!((couleur_du_texte(titre), couleur_du_texte(recents)), (p.texte_fort, p.texte_fort));
    assert!(titre_rect.center().x < recents_rect.center().x);
    // Chaque cadre est un rectangle de la couleur des panneaux du thème, autour de son titre.
    for rect in [titre_rect, recents_rect] {
        assert!(formes.iter().any(|s| matches!(s, Shape::Rect(r) if r.fill == p.panneau && r.rect.width() > 300.0 && r.rect.contains(rect.center()))), "cadre aux couleurs du thème");
    }
    // Le fond de la page est celui du thème ; aucun remplissage orange (le rehaut orange n'est que dans l'icône et le logotype).
    assert!(formes.iter().any(|s| matches!(s, Shape::Rect(r) if r.fill == p.fond && r.rect.width() >= 999.0)), "le fond de la page");
    assert!(!formes.iter().any(|s| matches!(s, Shape::Rect(r) if r.fill == ORANGE && r.rect.width() > 4.0 && r.rect.height() > 4.0)), "pas d'aplat orange sur l'accueil");
}

#[test]
fn the_welcome_page_follows_a_light_theme_without_any_orange() {
    let mut b = accueil();
    b.app.settings.theme.choisir(ChoixTheme::GrisClair);
    b.chauffer(6);
    let p = theme::palette();
    assert!(p.claire && !p.identite);
    let formes = a_plat(&b.formes);
    assert!(formes.iter().any(|s| matches!(s, Shape::Rect(r) if r.fill == p.panneau && r.rect.width() > 300.0)), "cadres clairs");
    let (titre, _) = texte_a(&formes, "Nouveau projet").unwrap();
    assert_eq!(couleur_du_texte(titre), p.texte_fort);
    assert!(!formes.iter().any(|s| matches!(s, Shape::Rect(r) if r.fill == ORANGE)));
    assert!(crate::accueil::logo_texture(&b.ctx, 240.0).size()[0] > 0 && !crate::accueil::logo_sombre(), "le logo est celui des fonds clairs");
    theme::installer(&ReglageTheme::default());
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
}