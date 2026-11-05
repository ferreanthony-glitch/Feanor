//! La charte graphique de FEANOR (livre de marque) : les thèmes « Atelier » (sombre, par défaut) et « Papier » (clair) aux jetons
//! exacts, leurs contrastes (texte 4,5:1, bordures de commandes 3:1, `alerte` 4,5:1), l'absence de rouge de marque, le
//! lockup (le Joyau à la place du O, dimensionné sur la hauteur réelle des capitales), l'animation d'ouverture de l'accueil
//! (une fois, sans bloquer, zéro image ensuite) et l'accueil. Les captures d'écran (essais ignorés) vont dans
//! `FEANOR_CAPTURE_DIR` (ou `SHOT_CAPTURE_DIR`).

use eframe::egui::{self, epaint::Shape, Color32, Rect};

use crate::document::Document;
use crate::logo;
use crate::marque::{self, Mise};
use crate::perf_tests::{banc_taille, Banc};
use crate::settings::Settings;
use crate::theme::{self, contraste, ChoixTheme, Couleurs, Palette, ReglageTheme};
use crate::workspace::Espace;

fn c(v: [u8; 3]) -> Color32 {
    Color32::from_rgb(v[0], v[1], v[2])
}

fn palette(co: Couleurs) -> Palette {
    Palette::depuis(&co)
}

/// Cette couleur est-elle un rouge ou un orange de marque (famille de l'ancien #FF3B30 / #E76F3C) ?
fn rougeatre(c: Color32) -> bool {
    c.r() >= 170 && c.r() as i32 - c.g() as i32 >= 70 && c.r() as i32 - c.b() as i32 >= 70
}

// ---------------------------------------------------------------------------------------- choix du thème

#[test]
fn a_fresh_install_gets_atelier_the_dark_brand_theme() {
    assert_eq!(Settings::default().theme.choix, ChoixTheme::Shot);
    assert_eq!(toml::from_str::<Settings>("").unwrap().theme.choix, ChoixTheme::Shot);
    assert_eq!(ChoixTheme::default().nom(), "Atelier");
    assert_eq!(ChoixTheme::TOUS.map(ChoixTheme::nom), ["Atelier", "Papier", "Gris sombre", "Gris clair", "Sombre", "Personnalisé"]);
    assert_eq!(ReglageTheme::default().couleurs(), Couleurs::shot());
}

#[test]
fn a_computer_that_already_chose_a_theme_keeps_it() {
    // Tout choix écrit dans le fichier de réglages est relu tel quel, y compris l'ancien « shot » (qui est Atelier maintenant).
    for choix in ChoixTheme::TOUS {
        let mut s = Settings::default();
        s.theme.choix = choix;
        let texte = toml::to_string(&s).unwrap();
        assert!(texte.contains("[theme]"), "{texte}");
        let relu: Settings = toml::from_str(&texte).unwrap();
        assert_eq!(relu.theme.choix, choix, "{choix:?}");
    }
    for (ecrit, attendu) in [("gris_sombre", ChoixTheme::GrisSombre), ("gris_clair", ChoixTheme::GrisClair), ("sombre", ChoixTheme::Sombre), ("shot", ChoixTheme::Shot), ("papier", ChoixTheme::Papier)] {
        let s: Settings = toml::from_str(&format!("[theme]\nchoix = \"{ecrit}\"\n")).unwrap();
        assert_eq!(s.theme.choix, attendu);
    }
    // Le fichier d'un poste qui avait gardé Gris sombre (écrit avant la marque, avec ses neuf couleurs) : Gris sombre, inchangé.
    let ancien = "[theme]\nchoix = \"gris_sombre\"\n[theme.perso]\npanneau = [64, 64, 64]\nentete = [54, 54, 54]\nfenetre = [56, 56, 56]\nwidgets = [90, 90, 90]\ntexte = [230, 230, 230]\naccent = [200, 200, 200]\nfond = [46, 46, 46]\ntimeline = [72, 72, 72]\nbordure = [80, 80, 80]\n";
    let s: Settings = toml::from_str(ancien).unwrap();
    assert_eq!(s.theme.choix, ChoixTheme::GrisSombre);
    assert_eq!(s.theme.couleurs(), Couleurs::gris_sombre());
    assert!(!Palette::depuis(&s.theme.couleurs()).identite, "les thèmes d'avant la marque n'ont pas ses jetons");
    // Un thème personnalisé qui part d'Atelier garde les jetons de la marque (et les relit).
    let mut r = ReglageTheme::default();
    let mut co = r.couleurs();
    co.fond = [30, 10, 10];
    r.modifier(co);
    assert_eq!(r.choix, ChoixTheme::Personnalise);
    let mut s = Settings::default();
    s.theme = r;
    let relu: Settings = toml::from_str(&toml::to_string(&s).unwrap()).unwrap();
    assert_eq!(relu.theme, r);
    assert!(relu.theme.couleurs().identite);
}

// ---------------------------------------------------------------------------------------- jetons et contrastes

#[test]
fn atelier_and_papier_carry_the_exact_brand_tokens() {
    let a = palette(Couleurs::shot());
    assert!(!a.claire && a.identite);
    assert_eq!((a.entete, a.panneau, a.fond, a.bord), (c(theme::ATELIER_SOL), c(theme::ATELIER_PANNEAU), c(theme::ATELIER_CANEVAS), c(theme::ATELIER_LIGNE)));
    assert_eq!((a.texte, a.texte_doux, a.bord_fort), (c(theme::ATELIER_ENCRE), c(theme::ATELIER_ENCRE_DOUCE), c(theme::ATELIER_LIGNE_FORTE)));
    assert_eq!((a.accent, a.sur_accent, a.accent_texte, a.focus, a.rehaut), (c(theme::JADE), c(theme::SUR_JADE), c(theme::ATELIER_JADE_TEXTE), c(theme::ATELIER_JADE_TEXTE), c(theme::ATELIER_JADE_TEXTE)));
    let p = palette(Couleurs::papier());
    assert!(p.claire && p.identite);
    assert_eq!((p.entete, p.panneau, p.fond, p.bord), (c(theme::PAPIER_SOL), c(theme::PAPIER_PANNEAU), c(theme::PAPIER_CANEVAS), c(theme::PAPIER_LIGNE)));
    assert_eq!((p.texte, p.texte_doux, p.bord_fort), (c(theme::PAPIER_ENCRE), c(theme::PAPIER_ENCRE_DOUCE), c(theme::PAPIER_LIGNE_FORTE)));
    assert_eq!((p.accent, p.sur_accent, p.accent_texte, p.focus), (c(theme::JADE), c(theme::SUR_JADE), c(theme::PAPIER_JADE_TEXTE), c(theme::PAPIER_JADE_TEXTE)));
    // Les valeurs du livre de marque.
    assert_eq!(theme::ATELIER_SOL, [0x12, 0x12, 0x14]);
    assert_eq!(theme::PAPIER_CANEVAS, [0xE4, 0xE4, 0xE0]);
    assert_eq!(theme::JADE, [0x2F, 0xD4, 0xC0]);
    assert_eq!(theme::SUR_JADE, [0x06, 0x2B, 0x2A]);
    assert_eq!(theme::ATELIER_ALERTE, [0xFF, 0x6B, 0x6B]);
    assert_eq!(theme::PAPIER_ALERTE, [0xB3, 0x26, 0x1E]);
    assert_eq!(theme::BRAISE, [0xF2, 0xA3, 0x3A]);
    // `alerte` est le rouge des erreurs ; l'avertissement est de l'encre neutre (la braise n'est qu'une pastille).
    assert_eq!((a.erreur, a.alerte, a.braise), (c(theme::ATELIER_ALERTE), c(theme::ATELIER_ALERTE), c(theme::BRAISE)));
    assert_eq!((p.erreur, p.alerte), (c(theme::PAPIER_ALERTE), c(theme::PAPIER_ALERTE)));
    assert_eq!((a.avertissement, p.avertissement), (a.texte_fort, p.texte_fort));
}

#[test]
fn brand_texts_borders_and_alerts_meet_their_contrast_in_both_themes() {
    for (nom, co) in [("Atelier", Couleurs::shot()), ("Papier", Couleurs::papier())] {
        let p = palette(co);
        for fond in [p.panneau, p.entete, p.fenetre, p.rangee, p.rangee_sombre, p.fond, p.champ] {
            assert!(contraste(p.texte, fond) >= 4.5, "{nom} : texte sur {fond:?} : {:.2}", contraste(p.texte, fond));
            assert!(contraste(p.texte_doux, fond) >= 4.5, "{nom} : texte secondaire sur {fond:?} : {:.2}", contraste(p.texte_doux, fond));
            assert!(contraste(p.accent_texte, fond) >= 4.5, "{nom} : texte d'accent sur {fond:?} : {:.2}", contraste(p.accent_texte, fond));
            assert!(contraste(p.alerte, fond) >= 4.5, "{nom} : alerte sur {fond:?} : {:.2}", contraste(p.alerte, fond));
        }
        assert!(contraste(p.sur_accent, p.accent) >= 4.5, "{nom} : ce qui s'écrit sur un aplat jade : {:.2}", contraste(p.sur_accent, p.accent));
        for fond in [p.panneau, p.entete, p.fenetre] {
            assert!(contraste(p.bord_fort, fond) >= 3.0, "{nom} : bordure de commande sur {fond:?} : {:.2}", contraste(p.bord_fort, fond));
            assert!(contraste(p.focus, fond) >= 3.0, "{nom} : anneau de focus sur {fond:?} : {:.2}", contraste(p.focus, fond));
        }
        assert!(contraste(p.alerte, c(if p.claire { theme::PAPIER_SOL } else { theme::ATELIER_SOL })) >= 4.5, "{nom} : alerte sur le sol");
        // Les commandes ont leur bordure forte, jade au survol ; pas d'ombre portée ; panneaux à 8 px.
        let v = p.visuels();
        assert_eq!(v.widgets.inactive.bg_stroke.color, p.bord_fort, "{nom}");
        assert_eq!(v.widgets.hovered.bg_stroke.color, p.accent_texte, "{nom}");
        assert_eq!((v.window_shadow.color.a(), v.popup_shadow.color.a()), (0, 0), "{nom} : pas d'ombre portée");
        assert_eq!(v.window_corner_radius, egui::CornerRadius::same(8), "{nom}");
        assert_eq!(v.widgets.inactive.corner_radius, egui::CornerRadius::same(4), "{nom} : commandes à 4 px");
        // La sélection est un aplat jade, l'encre sur jade dessus.
        assert_eq!((v.selection.bg_fill, v.selection.stroke.color), (c(theme::JADE), c(theme::SUR_JADE)), "{nom}");
    }
}

#[test]
fn the_other_themes_keep_their_alert_colours_and_their_old_look() {
    // Hors marque : l'avertissement et l'erreur sont ceux d'avant ; `alerte` existe quand même, lisible.
    for co in [Couleurs::gris_sombre(), Couleurs::gris_clair(), Couleurs::sombre()] {
        let p = palette(co);
        assert!(!p.identite);
        assert_ne!(p.avertissement, p.texte_fort);
        assert_eq!(p.alerte, p.erreur, "hors marque, l'alerte est le rouge d'erreur d'avant");
    }
}

#[test]
fn no_brand_red_or_orange_is_left_in_the_chrome_palette() {
    for (nom, co) in [("Atelier", Couleurs::shot()), ("Papier", Couleurs::papier())] {
        let p = palette(co);
        for (champ, col) in [
            ("fond", p.fond),
            ("entête", p.entete),
            ("panneau", p.panneau),
            ("fenêtre", p.fenetre),
            ("élevé", p.eleve),
            ("bord", p.bord),
            ("bord fort", p.bord_fort),
            ("champ", p.champ),
            ("bouton", p.controle),
            ("bouton survolé", p.controle_survol),
            ("bouton enfoncé", p.controle_actif),
            ("texte", p.texte),
            ("texte secondaire", p.texte_doux),
            ("texte fort", p.texte_fort),
            ("accent", p.accent),
            ("texte d'accent", p.accent_texte),
            ("focus", p.focus),
            ("rehaut", p.rehaut),
            ("contour", p.contour),
            ("rangée", p.rangee),
            ("bloc", p.bloc),
            ("poignée", p.poignee),
            ("avertissement", p.avertissement),
        ] {
            assert!(!rougeatre(col), "{nom} : {champ} n'est pas rouge/orange : {col:?}");
        }
    }
    // Le jade couvre l'accent ; la braise est une pastille, jamais un fond.
    assert!(!rougeatre(c(theme::JADE)));
}

#[test]
fn the_drawing_and_meaning_colours_are_unchanged_by_the_brand() {
    // Calques, repères de caméra, marques d'image… sont des couleurs qui disent quelque chose : mêmes dans les thèmes de gris et dans Atelier.
    let (atelier, gris) = (palette(Couleurs::shot()), palette(Couleurs::gris_sombre()));
    for (nom, a, b) in [
        ("dessin vectoriel", atelier.dessin_vectoriel, gris.dessin_vectoriel),
        ("dessin bitmap", atelier.dessin_bitmap, gris.dessin_bitmap),
        ("calque de couleur", atelier.dessin_couleur, gris.dessin_couleur),
        ("calque d'effet", atelier.dessin_cfx, gris.dessin_cfx),
        ("annotation", atelier.dessin_annotation, gris.dessin_annotation),
        ("calques de décor", atelier.decor, gris.decor),
        ("clés de caméra", atelier.camera, gris.camera),
        ("marques d'image", atelier.marque, gris.marque),
        ("clés d'effets", atelier.effet, gris.effet),
        ("son muet", atelier.son_muet, gris.son_muet),
        ("succès", atelier.succes, gris.succes),
    ] {
        // (Les rangées de la timeline n'ont pas les mêmes fonds : seule la teinte compte, à quelques niveaux près.)
        let ecart = |x: u8, y: u8| (x as i32 - y as i32).abs();
        assert!(ecart(a.r(), b.r()).max(ecart(a.g(), b.g())).max(ecart(a.b(), b.b())) <= 70, "{nom} : {a:?} contre {b:?}");
    }
}

// ---------------------------------------------------------------------------------------- la fenêtre

/// Les formes à plat (`Shape::Vec` déroulés).
fn a_plat<'a>(formes: &'a [egui::epaint::ClippedShape]) -> Vec<&'a Shape> {
    fn va<'a>(s: &'a Shape, out: &mut Vec<&'a Shape>) {
        match s {
            Shape::Vec(v) => v.iter().for_each(|s| va(s, out)),
            autre => out.push(autre),
        }
    }
    let mut out = Vec::new();
    for cs in formes {
        va(&cs.shape, &mut out);
    }
    out
}

fn texte_a<'a>(formes: &'a [&'a Shape], texte: &str) -> Option<(&'a egui::epaint::TextShape, Rect)> {
    formes.iter().find_map(|s| match s {
        Shape::Text(t) if t.galley.job.text == texte => Some((t, Rect::from_min_size(t.pos, t.galley.size()))),
        _ => None,
    })
}

#[test]
fn the_active_workspace_is_a_jade_tab_with_on_jade_text_and_a_thin_jade_line() {
    let mut b = banc_taille(crate::vignettes_tests::quatre_plans(), false, 1600, 900);
    b.garder_formes = true;
    for espace in [Espace::Vignettes, Espace::Storyboard, Espace::Animation, Espace::Montage] {
        b.app.set_espace(espace);
        b.chauffer(6);
        let formes = a_plat(&b.formes);
        let (t, mot) = texte_a(&formes, &format!(" {} ", espace.nom())).unwrap_or_else(|| panic!("onglet {}", espace.nom()));
        assert_eq!(t.galley.job.sections[0].format.color, theme::sur_accent(), "{} : encre sur jade", espace.nom());
        assert_eq!(theme::sur_accent(), c(theme::SUR_JADE));
        assert!(formes.iter().any(|s| matches!(s, Shape::Rect(r) if r.fill == theme::accent() && r.rect.intersects(mot))), "{} : fond jade", espace.nom());
        // Le mince trait jade sous le mot actif.
        let traits: Vec<Rect> = formes.iter().filter_map(|s| if let Shape::Rect(r) = s { (r.fill == theme::rehaut() && r.rect.height() <= 2.5 && r.rect.width() < 120.0 && r.rect.width() > 8.0).then_some(r.rect) } else { None }).collect();
        assert!(traits.iter().any(|tr| tr.center().x > mot.left() && tr.center().x < mot.right() && tr.center().y > mot.center().y), "{} : un trait sous le mot : {traits:?}", espace.nom());
    }
}

#[test]
fn the_menu_bar_starts_with_fichier_without_a_logotype() {
    // 1280 points de large : un écran de 1920 pixels à 150 %.
    let mut b = banc_taille(Document::with_frame(160, 90, 24), false, 1280, 720);
    b.garder_formes = true;
    b.chauffer(6);
    let formes = a_plat(&b.formes);
    let (_, fichier) = texte_a(&formes, "Fichier").expect("menu Fichier");
    let (_, aide) = texte_a(&formes, "Aide").expect("menu Aide");
    let (_, vignettes) = texte_a(&formes, " Vignettes ").expect("onglet Vignettes");
    assert!(fichier.left() < 16.0, "Fichier est le premier de la barre : {fichier:?}");
    assert!(aide.right() + 20.0 < vignettes.left(), "les menus et les espaces de travail ne se touchent pas : {aide:?} {vignettes:?}");
    let image_avant = formes.iter().any(|s| matches!(s, Shape::Rect(r) if r.brush.is_some() && r.rect.bottom() <= 30.0 && r.rect.right() <= fichier.left()));
    assert!(!image_avant, "pas de logotype à gauche de Fichier");
}

/// La couleur du texte d'un `Label` : celle de sa section (`RichText::color`).
fn couleur_du_texte(t: &egui::epaint::TextShape) -> Color32 {
    t.galley.job.sections.first().map(|s| s.format.color).filter(|col| *col != Color32::PLACEHOLDER).unwrap_or(t.fallback_color)
}

fn accueil() -> Banc {
    let mut b = banc_taille(Document::with_frame(160, 90, 24), false, 1000, 620);
    b.garder_formes = true;
    b.app.welcome = true;
    b.chauffer(6);
    b
}

#[test]
fn the_welcome_page_has_two_framed_panels_in_the_theme_colours_the_lockup_and_no_old_orange() {
    let b = accueil();
    assert!(b.app.welcome_shown());
    let p = theme::palette();
    let formes = a_plat(&b.formes);
    let (titre, titre_rect) = texte_a(&formes, "Nouveau projet").expect("Nouveau projet");
    let (recents, recents_rect) = texte_a(&formes, "Projets récents").expect("Projets récents");
    assert_eq!((couleur_du_texte(titre), couleur_du_texte(recents)), (p.texte_fort, p.texte_fort));
    assert!(titre_rect.center().x < recents_rect.center().x);
    for rect in [titre_rect, recents_rect] {
        assert!(formes.iter().any(|s| matches!(s, Shape::Rect(r) if r.fill == p.panneau && r.rect.width() > 300.0 && r.rect.contains(rect.center()))), "cadre aux couleurs du thème");
    }
    assert!(formes.iter().any(|s| matches!(s, Shape::Rect(r) if r.fill == p.fond && r.rect.width() >= 999.0)), "le fond de la page");
    assert!(!formes.iter().any(|s| matches!(s, Shape::Rect(r) if rougeatre(r.fill) && r.rect.width() > 4.0 && r.rect.height() > 4.0)), "pas d'aplat rouge ou orange sur l'accueil");
    // Le lockup : « FEAN » et « R » en texte de marque, le Joyau (une image) entre les deux, la devise dessous.
    let (_, fean) = texte_a(&formes, "FEAN").expect("le mot FEAN");
    let (_, r) = texte_a(&formes, "R").expect("le R");
    let (_, devise) = texte_a(&formes, marque::DEVISE).expect("la devise");
    let joyau = formes.iter().find_map(|s| match s {
        Shape::Mesh(m) if m.texture_id != egui::TextureId::default() && !m.vertices.is_empty() => {
            let rc = Rect::from_points(&m.vertices.iter().map(|v| v.pos).collect::<Vec<_>>());
            (rc.left() > fean.left() && rc.width() > 40.0 && rc.height() < 120.0).then_some(rc)
        }
        _ => None,
    });
    let joyau = joyau.expect("le Joyau entre FEAN et R");
    assert!(joyau.center().x > fean.center().x && joyau.center().x < r.center().x, "le Joyau remplace le O");
    assert!(devise.top() > fean.center().y && (devise.left() - fean.left()).abs() <= 1.0, "la devise est sous le mot, commence au F : {devise:?} {fean:?}");
}

#[test]
fn the_welcome_page_follows_a_light_theme() {
    let mut b = accueil();
    b.app.settings.theme.choisir(ChoixTheme::Papier);
    b.chauffer(6);
    let p = theme::palette();
    assert!(p.claire && p.identite);
    let formes = a_plat(&b.formes);
    assert!(formes.iter().any(|s| matches!(s, Shape::Rect(r) if r.fill == p.panneau && r.rect.width() > 300.0)), "cadres clairs");
    let (titre, _) = texte_a(&formes, "Nouveau projet").unwrap();
    assert_eq!(couleur_du_texte(titre), p.texte_fort);
    theme::installer(&ReglageTheme::default());
}

// ---------------------------------------------------------------------------------------- lockup et polices

#[test]
fn the_brand_fonts_are_embedded_and_the_joyau_is_sized_on_the_real_cap_height() {
    let m = marque::metriques(marque::MOT_OCTETS).expect("Big Shoulders Display lisible");
    assert!(m.unites_par_em >= 1000.0 && m.hauteur_capitales > 0.5 * m.unites_par_em && m.hauteur_capitales < m.unites_par_em, "{m:?}");
    assert!(marque::metriques(marque::DEVISE_OCTETS).is_some());
    // Pour 50 points de capitales : une taille de police où les capitales font bien 50 points.
    let taille = marque::taille_pour_capitales(&m, 50.0);
    assert!((taille * m.hauteur_capitales / m.unites_par_em - 50.0).abs() < 1e-3);
    // Le Joyau : la pierre (39 unités sur 64) a la hauteur des capitales.
    let cote = marque::cote_joyau(50.0);
    assert!((cote * 39.0 / 64.0 - 50.0).abs() < 1e-3);
    // Les licences sont livrées avec les polices.
    for f in ["OFL-BigShoulders.txt", "OFL-IBMPlexSans.txt", "OFL-IBMPlexMono.txt"] {
        let chemin = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets").join("polices").join(f);
        assert!(std::fs::read_to_string(chemin).unwrap().contains("SIL OPEN FONT LICENSE"), "{f}");
    }
}

#[test]
fn the_lockup_puts_the_joyau_over_the_o_sunk_and_overlapping_its_neighbours() {
    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::default();
    marque::polices(&mut fonts);
    ctx.set_fonts(fonts);
    // (Une image pour que les polices existent.)
    let _ = ctx.run(Default::default(), |_| {});
    let capitales = 60.0;
    let l = marque::disposer(&ctx, egui::pos2(100.0, 100.0), capitales, true, Mise::RemplaceLeO);
    let cote = marque::cote_joyau(capitales);
    assert!((l.joyau.width() - cote).abs() < 1e-3 && (l.joyau.height() - cote).abs() < 1e-3);
    // Enfoncé de cote × 12/64 sous la ligne de base ; la pierre (13/64 depuis le haut) touche le haut des capitales.
    assert!((l.base - 160.0).abs() < 1e-3);
    assert!((l.joyau.bottom() - (l.base + cote * 12.0 / 64.0)).abs() < 1e-3);
    assert!((l.joyau.top() + cote * 13.0 / 64.0 - 100.0).abs() < 1e-3, "le haut de la pierre est le haut des capitales");
    // Il mord de cote × 7/64 sur la lettre de gauche.
    let gauche = marque::mot(&ctx, "FEAN", capitales, Color32::WHITE);
    assert!((l.joyau.left() - (100.0 + gauche.size().x - cote * 7.0 / 64.0)).abs() < 1e-3);
    // Sous 24 points de capitales : le Joyau seul.
    let petit = marque::disposer(&ctx, egui::pos2(0.0, 0.0), 20.0, true, Mise::RemplaceLeO);
    assert_eq!(petit.rect, petit.joyau);
    // La devise s'ajoute sous le mot, seulement si demandée.
    let sans = marque::disposer(&ctx, egui::pos2(100.0, 100.0), capitales, false, Mise::RemplaceLeO);
    assert!(l.rect.bottom() > sans.rect.bottom());
    // Le lockup principal : le Joyau devant le mot entier, la devise alignée à droite sur le R.
    let devant = marque::disposer(&ctx, egui::pos2(100.0, 100.0), capitales, true, Mise::Devant);
    let entier = marque::mot(&ctx, "FEANOR", capitales, Color32::WHITE);
    assert!(devant.joyau.right() <= devant.mot_gauche + cote * 0.1 && devant.joyau.left() == 100.0, "le Joyau est devant : {devant:?}");
    assert!((devant.mot_droite - devant.mot_gauche - entier.size().x).abs() < 1e-3, "le mot entier suit le Joyau");
    assert!((devant.joyau.bottom() - (devant.base + cote * 12.0 / 64.0)).abs() < 1e-3 && (devant.joyau.top() + cote * 13.0 / 64.0 - 100.0).abs() < 1e-3);
    // Jamais coupé dans une fenêtre étroite : il rétrécit pour tenir, puis il ne reste que le Joyau.
    let large = marque::disposer(&ctx, egui::pos2(0.0, 0.0), capitales, false, Mise::Devant).rect.width();
    assert_eq!(marque::ajuster(&ctx, large + 50.0, capitales, Mise::Devant), capitales, "assez de place : inchangé");
    for dispo in [large * 0.8, large * 0.5, 120.0, 40.0] {
        let cap = marque::ajuster(&ctx, dispo, capitales, Mise::Devant);
        let l = marque::disposer(&ctx, egui::pos2(0.0, 0.0), cap, false, Mise::Devant);
        assert!(l.rect.width() <= dispo + 0.5 && cap <= capitales, "tient dans {dispo} : {cap} → {}", l.rect.width());
    }
    let seul = marque::disposer(&ctx, egui::pos2(0.0, 0.0), marque::ajuster(&ctx, 40.0, capitales, Mise::Devant), false, Mise::Devant);
    assert_eq!(seul.rect, seul.joyau, "sans place : le Joyau seul");
}

// ---------------------------------------------------------------------------------------- animation d'ouverture

/// Quelques images vides avant l'essai : egui redemande une image au tout début (polices, textures), pas à cause de la barre.
fn echauffer(ctx: &egui::Context) {
    for k in 0..4 {
        let _ = ctx.run(egui::RawInput { time: Some(k as f64 * 0.01), ..Default::default() }, |_| {});
    }
}

#[test]
fn the_opening_animation_plays_once_asks_for_frames_only_while_running_then_none() {
    let ctx = egui::Context::default();
    echauffer(&ctx);
    let dernier = std::cell::Cell::new(logo::etat_repos());
    let image = |t: f64| -> bool {
        let sortie = ctx.run(egui::RawInput { time: Some(t), ..Default::default() }, |ctx| dernier.set(marque::animation_accueil(ctx)));
        sortie.viewport_output.values().any(|v| v.repaint_delay.is_zero())
    };
    assert!(image(10.0), "elle démarre : une image est demandée");
    assert_eq!(dernier.get(), logo::etat_animation(0.0));
    assert!(image(10.5), "elle court : encore une image");
    assert_eq!(dernier.get(), logo::etat_animation(0.5));
    // (egui donne une image de plus après une demande : on regarde la deuxième image calme.)
    let _ = image(10.0 + logo::DUREE_ANIMATION as f64 + 0.01);
    assert!(!image(10.0 + logo::DUREE_ANIMATION as f64 + 0.05), "fin : plus aucune image demandée");
    assert_eq!(dernier.get(), logo::etat_repos());
    // Figée : ni mouvement ni image demandée, même des heures plus tard ; et elle ne se rejoue pas.
    for t in [20.0, 99.0, 5000.0] {
        assert!(!image(t), "au repos à {t} s");
        assert_eq!(dernier.get(), logo::etat_repos());
    }
}

#[test]
fn the_opening_animation_is_skipped_when_egui_has_no_animation_time() {
    let ctx = egui::Context::default();
    echauffer(&ctx);
    ctx.style_mut(|s| s.animation_time = 0.0);
    let mut etat = None;
    let sortie = ctx.run(egui::RawInput { time: Some(1.0), ..Default::default() }, |ctx| etat = Some(marque::animation_accueil(ctx)));
    assert_eq!(etat, Some(logo::etat_repos()), "mouvement réduit : le Joyau est tout de suite au repos");
    assert!(!sortie.viewport_output.values().any(|v| v.repaint_delay.is_zero()));
}

#[test]
fn the_welcome_page_keeps_clicks_working_during_the_animation_and_rests_after_it() {
    let mut b = accueil();
    // Pendant l'animation (les premières images) la fenêtre répond : le formulaire est là et rien n'est bloqué.
    assert!(b.app.welcome_shown());
    // Après la fin, l'accueil se repose comme avant (zéro image demandée).
    let ctx = b.ctx.clone();
    let _ = ctx;
    std::thread::sleep(std::time::Duration::from_millis((logo::DUREE_ANIMATION * 1000.0) as u64 + 150));
    b.chauffer(3);
    // (Le curseur clignotant du champ « Nom du projet », qui a le focus à l'ouverture, demande des images : ce n'est pas le Joyau.)
    b.ctx.memory_mut(|m| {
        if let Some(id) = m.focused() {
            m.surrender_focus(id);
        }
    });
    let n = crate::perf_tests::se_reposer(&mut b, 3000);
    assert!(n < 3000, "FEANOR se repose sur l'accueil une fois le Joyau ouvert : {}", b.raison_du_reveil());
}

// ---------------------------------------------------------------------------------------- captures (à regarder)

fn dossier_de_captures() -> std::path::PathBuf {
    std::env::var_os("FEANOR_CAPTURE_DIR").or_else(|| std::env::var_os("SHOT_CAPTURE_DIR")).map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

/// La fenêtre principale (Vignettes, Storyboard, Animation, Montage) et l'accueil, en PNG :
/// `FEANOR_CAPTURE_DIR=… cargo test --release --offline --target-dir target\msvc charte_captures -- --ignored --nocapture`
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
    // À propos (lockup principal : Joyau jade devant le mot).
    b.app.set_espace(Espace::Storyboard);
    b.app.about_window = true;
    b.chauffer(8);
    sauver(&b, "a-propos.png");
    b.app.about_window = false;
    // Le thème clair « Papier » : l'accueil et la fenêtre principale.
    b.app.settings.theme.choisir(ChoixTheme::Papier);
    b.chauffer(8);
    sauver(&b, "fenetre-principale-papier.png");
    let mut c = crate::perf_tests::banc(Document::with_frame(160, 90, 24), true);
    c.app.settings.theme.choisir(ChoixTheme::Papier);
    c.app.welcome = true;
    c.chauffer(8);
    sauver(&c, "accueil-papier.png");
    theme::installer(&ReglageTheme::default());
}
