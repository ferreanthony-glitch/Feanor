//! Tests du thème : les textes restent lisibles sur leurs fonds dans chacun des quatre thèmes
//! livrés (SHOT et les trois gris), l'accent marque la sélection, le thème « Personnalisé »
//! naît d'une couleur changée, les réglages se relisent (une installation neuve donne SHOT, un poste
//! qui a déjà choisi un thème le garde), la police de Windows passe devant celles d'egui sans les
//! remplacer. La charte (couleurs, contrastes, ce que l'accent touche) est dans `charte_tests.rs`.

use eframe::egui::{Color32, FontDefinitions, FontFamily};

use crate::settings::Settings;
use crate::theme::{self, contraste, ChoixTheme, Couleurs, Palette, ReglageTheme};

/// Les quatre thèmes livrés, avec leur palette.
fn livres() -> Vec<(ChoixTheme, Palette)> {
    [ChoixTheme::Shot, ChoixTheme::GrisSombre, ChoixTheme::GrisClair, ChoixTheme::Sombre]
        .into_iter()
        .map(|c| {
            let mut r = ReglageTheme::default();
            r.choisir(c);
            (c, Palette::depuis(&r.couleurs()))
        })
        .collect()
}

#[test]
fn contrast_is_computed_like_wcag() {
    assert!((contraste(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 0.01);
    let p = Palette::depuis(&Couleurs::sombre());
    assert!((contraste(p.panneau, p.panneau) - 1.0).abs() < 1e-4);
}

#[test]
fn the_default_theme_is_shot_and_the_grey_themes_stay_neutral() {
    assert_eq!(ReglageTheme::default().choix, ChoixTheme::Shot);
    assert_eq!(ReglageTheme::default().couleurs(), Couleurs::shot());
    // Des gris neutres, pas du bleu-noir : rouge, vert et bleu égaux aux fonds.
    for (choix, p) in livres() {
        for fond in [p.panneau, p.entete, p.fenetre, p.fond, p.rangee, p.controle, p.bord] {
            assert!(fond.r() == fond.g() && fond.g() == fond.b(), "{choix:?} : {fond:?} n'est pas un gris neutre");
        }
    }
    let p = Palette::depuis(&Couleurs::gris_sombre());
    assert!((0x3a..=0x44).contains(&p.panneau.r()), "fond des panneaux autour de #3a–#44 : {:?}", p.panneau);
    assert!(p.fenetre.r() < p.panneau.r(), "fenêtres et menus un peu plus sombres");
    assert!(p.controle.r() > p.panneau.r(), "widgets plus clairs");
    assert_eq!(p.texte, Color32::from_gray(0xe6));
    let c = Palette::depuis(&Couleurs::gris_clair());
    assert!((0xb8..=0xc8).contains(&c.panneau.r()), "gris clair : {:?}", c.panneau);
    assert!(c.claire && !p.claire);
}

#[test]
fn texts_stay_readable_on_their_backgrounds_in_every_theme() {
    for (choix, p) in livres() {
        // Texte courant : au moins 7:1 (niveau AAA) sur les panneaux, les barres et les menus.
        for fond in [p.panneau, p.entete, p.fenetre] {
            assert!(contraste(p.texte, fond) >= 7.0, "{choix:?} : texte sur {fond:?} : {}", contraste(p.texte, fond));
        }
        // Et au moins 4,5:1 (AA) partout ailleurs : champs, boutons, rangées, autour du dessin.
        for fond in [p.champ, p.eleve, p.controle, p.fond, p.rangee, p.rangee_sombre] {
            assert!(contraste(p.texte, fond) >= 4.5, "{choix:?} : texte sur {fond:?} : {}", contraste(p.texte, fond));
        }
        // Texte secondaire : au moins 4,5:1 sur les panneaux, les barres et les rangées.
        for fond in [p.panneau, p.entete, p.fenetre, p.rangee, p.rangee_sombre] {
            assert!(contraste(p.texte_doux, fond) >= 4.5, "{choix:?} : texte doux sur {fond:?} : {}", contraste(p.texte_doux, fond));
        }
        assert!(contraste(p.texte_fort, p.controle_actif) >= 4.5, "{choix:?} : bouton enfoncé");
        // L'accent : le texte posé dessus (éléments choisis) à 4,5:1, et sa version pour les traits
        // et les liens à 4,5:1 sur le panneau (au moins 3:1 est exigé d'un élément graphique).
        assert!(contraste(p.sur_accent, p.accent) >= 4.5, "{choix:?} : texte sur l'accent : {}", contraste(p.sur_accent, p.accent));
        assert!(contraste(p.accent_texte, p.panneau) >= 4.5, "{choix:?} : accent sur le panneau : {}", contraste(p.accent_texte, p.panneau));
        assert!(contraste(p.accent_texte, p.entete) >= 3.0, "{choix:?} : accent sur les barres");
    }
}

#[test]
fn selections_stay_visible_in_every_theme() {
    for (choix, p) in livres() {
        // Le contour des éléments choisis (clés, séquences, calque courant) se voit sur les rangées
        // comme sur les blocs sourds, et l'accent se distingue des rangées.
        for fond in [p.rangee, p.rangee_sombre, p.panneau] {
            assert!(contraste(p.contour, fond) >= 3.0, "{choix:?} : contour sur {fond:?} : {}", contraste(p.contour, fond));
        }
        for fond in [p.calque_vectoriel, p.calque_bitmap, p.calque_couleur] {
            assert!(contraste(p.contour, fond) >= 3.0, "{choix:?} : contour sur {fond:?} : {}", contraste(p.contour, fond));
        }
        assert!(contraste(p.accent, p.rangee) >= 1.4 || contraste(p.accent_texte, p.rangee) >= 3.0, "{choix:?} : accent sur la timeline");
        // La sélection d'egui est l'accent, son texte est celui qui lit sur lui.
        let v = p.visuels();
        assert_eq!(v.selection.bg_fill, p.accent);
        assert_eq!(v.selection.stroke.color, p.sur_accent);
    }
}

#[test]
fn timeline_and_montage_stay_readable_in_every_theme() {
    for (choix, p) in livres() {
        // Noms écrits sur les extraits son et les calques sous les panneaux : 4,5:1.
        for fond in [p.son_extrait, p.son_choisi, p.calque_vectoriel, p.calque_bitmap, p.calque_couleur] {
            assert!(contraste(p.sur_calque, fond) >= 4.5, "{choix:?} : nom sur {fond:?} : {}", contraste(p.sur_calque, fond));
        }
        // Piste muette : le « M » blanc.
        assert!(contraste(Color32::WHITE, p.son_muet) >= 4.5, "{choix:?} : M sur la piste muette");
        for sequence in [true, false] {
            for impaire in [false, true] {
                let barre = p.barres[usize::from(!sequence) * 2 + usize::from(impaire)];
                assert!(contraste(p.texte, barre) >= 4.5, "{choix:?} : barre {barre:?}");
            }
        }
        // Repères dessinés sur les rangées (dessins exposés, image courante, clés de caméra, clés
        // d'effets, marques) : au moins 3:1, le minimum pour un élément graphique.
        for fond in [p.rangee, p.rangee_sombre] {
            for marque in [p.dessin_vectoriel, p.dessin_bitmap, p.dessin_couleur, p.dessin_vide, p.image_courante, p.camera, p.marque, p.effet] {
                assert!(contraste(marque, fond) >= 3.0, "{choix:?} : {marque:?} sur {fond:?} : {}", contraste(marque, fond));
            }
        }
        // Les noms écrits sur les dessins vifs et le chiffre sur l'image courante.
        for fond in [p.dessin_vectoriel, p.dessin_bitmap, p.dessin_couleur, p.dessin_vide, p.image_courante] {
            assert!(contraste(p.encre, fond) >= 3.0, "{choix:?} : encre sur {fond:?} : {}", contraste(p.encre, fond));
        }
        // Forme d'onde sur l'extrait son.
        assert!(contraste(p.son_onde, p.son_extrait) >= 3.0, "{choix:?} : forme d'onde");
        // Les graduations se voient sur les rangées sans les écraser.
        assert!(contraste(p.graduation_seconde, p.rangee) >= 1.2, "{choix:?} : graduation d'une seconde");
        assert!(p.fond != p.panneau, "{choix:?} : le fond autour du dessin se distingue du panneau");
    }
}

#[test]
fn each_kind_of_layer_has_its_colour() {
    use crate::document::LayerKind;
    for (choix, p) in livres() {
        theme::installer(&ReglageTheme { choix, perso: Couleurs::default() });
        // Anthony (2026-10-28) : vectoriel orange (celui de SHOT), bitmap bleu (l'ancien bleu du vectoriel), calque de couleur
        // rose (ex-CTG) ; trois couleurs distinctes.
        let (b, v, c) = (theme::couleur_calque(LayerKind::Bitmap, false), theme::couleur_calque(LayerKind::Vector, false), theme::couleur_calque(LayerKind::Bitmap, true));
        assert!(b.b() > b.r() && b.b() > b.g(), "{choix:?} : bitmap bleu");
        assert!(v.r() > v.g() && v.g() > v.b(), "{choix:?} : vectoriel orange");
        assert!(c.r() > c.g() && c.b() > c.g(), "{choix:?} : calque de couleur rose");
        assert_eq!(theme::couleur_calque(LayerKind::Vector, true), c, "un calque de couleur reste rose, quelle que soit sa sorte");
        assert_eq!(theme::fond_calque(LayerKind::Vector, false), p.calque_vectoriel);
    }
    theme::installer(&ReglageTheme::default());
}

#[test]
fn the_scene_wide_layers_keep_a_readable_tint_in_every_theme() {
    // Le nom d'un calque de toute la scène s'écrit dans la teinte « décor » sur la ligne du calque ;
    // ses dessins la prennent en fond sous les plans (avant, une couleur fixe, pâle sur les thèmes clairs).
    for (choix, p) in livres() {
        assert!(contraste(p.decor, p.etiquette) >= 3.0, "{choix:?} : le nom d'un calque de décor se lit ({:.1})", contraste(p.decor, p.etiquette));
        assert!(contraste(p.decor, p.rangee) >= 3.0, "{choix:?} : sur les rangées aussi");
        // (Le texte posé sur un dessin de décor est à 1,9:1 dans les thèmes sombres, comme avant :
        // noté dans docs/INTERFACE.md, pas changé ici pour ne pas déplacer l'aspect du montage.)
        if p.claire {
            assert!(contraste(p.sur_calque, p.calque_decor) >= 3.0, "{choix:?} : le texte posé sur un dessin de décor se lit ({:.1})", contraste(p.sur_calque, p.calque_decor));
        }
        assert!(p.decor.r() > p.decor.b(), "{choix:?} : la teinte reste chaude (ocre)");
    }
    // Le thème par défaut garde l'ocre d'avant.
    let sombre = Palette::depuis(&Couleurs::gris_sombre());
    assert_eq!(sombre.decor, Color32::from_rgb(214, 170, 92));
    assert_eq!(sombre.calque_decor, Color32::from_rgb(214, 170, 92));
}

#[test]
fn the_overlay_colours_are_written_once_in_the_theme() {
    // Poignées de sélection, de déformation, de transformation et cadres de composition : avant,
    // le même bleu était recopié dans six fichiers et la caméra jaune dans trois.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let wanted = ["from_rgb(70, 165, 255)", "from_rgb(255, 150, 60)", "from_rgb(120, 220, 140)", "from_rgb(0, 165, 195)", "from_rgb(230, 60, 160)", "from_rgb(255, 196, 60)", "from_rgb(214, 170, 92)"];
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).expect("dossier src") {
        let path = entry.expect("fichier").path();
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if !name.ends_with(".rs") || name.ends_with("_tests.rs") || name == "theme.rs" {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("source lisible");
        for w in wanted {
            if text.contains(w) {
                found.push(format!("{name} : {w}"));
            }
        }
    }
    assert!(found.is_empty(), "couleurs recopiées hors de theme.rs : {found:?}");
}

#[test]
fn bands_alternate_so_limits_show() {
    for (choix, p) in livres() {
        theme::installer(&ReglageTheme { choix, perso: Couleurs::default() });
        for sequence in [true, false] {
            assert_ne!(theme::barre(sequence, false), theme::barre(sequence, true), "{choix:?}");
        }
        assert_eq!(theme::barre(true, false), p.barres[0]);
    }
    theme::installer(&ReglageTheme::default());
}

#[test]
fn the_visuals_use_the_palette() {
    for (choix, p) in livres() {
        theme::installer(&ReglageTheme { choix, perso: Couleurs::default() });
        let v = theme::visuels();
        assert_eq!(v.dark_mode, !p.claire, "{choix:?}");
        assert_eq!(v.panel_fill, p.panneau);
        assert_eq!(v.window_fill, p.fenetre);
        assert_eq!(v.extreme_bg_color, p.champ);
        assert_eq!(v.widgets.noninteractive.fg_stroke.color, p.texte);
        assert_eq!(v.widgets.inactive.weak_bg_fill, p.controle);
        let d = theme::dock(egui_dock::Style::default());
        assert_eq!((d.tab_bar.bg_fill, d.tab.active.bg_fill), (p.entete, p.panneau));
        // Le bord entre deux panneaux se voit sans le survol (poignée du montage) : plus clair que le fond, autre que l'accent.
        assert_ne!(d.separator.color_idle, p.fond, "{choix:?}");
        assert_ne!(d.separator.color_idle, d.separator.color_hovered, "{choix:?}");
        // Les fonctions de lecture donnent la palette installée.
        assert_eq!((theme::panneau(), theme::fond(), theme::texte(), theme::accent()), (p.panneau, p.fond, p.texte, p.accent));
    }
    theme::installer(&ReglageTheme::default());
}

#[test]
fn editing_a_colour_switches_to_custom_starting_from_the_current_theme() {
    let mut r = ReglageTheme::default();
    r.choisir(ChoixTheme::GrisClair);
    // Rien n'a changé : le thème reste celui qu'on a choisi.
    let c = r.couleurs();
    r.modifier(c);
    assert_eq!(r.choix, ChoixTheme::GrisClair);
    // Une couleur change : Personnalisé, et les autres sont celles du thème de départ.
    let mut c = r.couleurs();
    c.accent = [200, 40, 40];
    r.modifier(c);
    assert_eq!(r.choix, ChoixTheme::Personnalise);
    assert_eq!(r.couleurs().accent, [200, 40, 40]);
    assert_eq!(r.couleurs().panneau, Couleurs::gris_clair().panneau);
    assert_eq!(r.couleurs().texte, Couleurs::gris_clair().texte);
    // Retourner sur un thème livré garde le personnalisé de côté ; Personnalisé le retrouve.
    r.choisir(ChoixTheme::Sombre);
    assert_eq!(r.couleurs(), Couleurs::sombre());
    assert_eq!(r.perso.accent, [200, 40, 40]);
    // Passer à Personnalisé depuis un thème livré part de ce thème.
    let mut s = ReglageTheme::default();
    s.choisir(ChoixTheme::Sombre);
    s.choisir(ChoixTheme::Personnalise);
    assert_eq!(s.couleurs(), Couleurs::sombre());
    // Les neuf champs de la fenêtre sont bien les neuf couleurs.
    let mut c = Couleurs::default();
    assert_eq!(c.champs().len(), 9);
    for (i, (_, _, rvb)) in c.champs().into_iter().enumerate() {
        *rvb = [i as u8, 7, 7];
    }
    assert_eq!([c.panneau[0], c.entete[0], c.fenetre[0], c.widgets[0], c.texte[0], c.accent[0], c.fond[0], c.timeline[0], c.bordure[0]], [0, 1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn restore_goes_back_to_shot() {
    let mut r = ReglageTheme::default();
    r.choisir(ChoixTheme::Personnalise);
    let mut c = r.couleurs();
    c.fond = [250, 0, 0];
    r.modifier(c);
    r.retablir();
    assert_eq!(r, ReglageTheme::default());
    assert_eq!(r.choix, ChoixTheme::Shot);
}

#[test]
fn a_custom_theme_derives_a_readable_palette() {
    // Quelles que soient les neuf couleurs, la palette déduite choisit un texte doux et un accent
    // de traits lisibles quand c'est possible, et le logo suit le fond.
    let mut c = Couleurs::gris_sombre();
    c.panneau = [120, 40, 40];
    c.texte = [250, 250, 250];
    let p = Palette::depuis(&c);
    assert!(!p.claire);
    assert!(contraste(p.texte_doux, p.panneau) >= 4.5);
    let mut c = Couleurs::gris_clair();
    c.panneau = [240, 240, 220];
    let p = Palette::depuis(&c);
    assert!(p.claire);
    assert!(contraste(p.texte_doux, p.panneau) >= 4.5);
    assert!(contraste(p.accent_texte, p.panneau) >= 4.5);
}

#[test]
fn settings_round_trip_and_old_files_get_the_default_theme() {
    // Un vieux fichier, sans section [theme] (une installation neuve) : SHOT.
    let old: Settings = toml::from_str("copie_secours_minutes = 3\nrouvrir_dernier_projet = false\n").unwrap();
    assert_eq!(old.theme, ReglageTheme::default());
    assert_eq!(old.theme.choix, ChoixTheme::Shot);
    assert_eq!(old.autosave_minutes, 3);
    // Un fichier vide aussi.
    assert_eq!(toml::from_str::<Settings>("").unwrap().theme.choix, ChoixTheme::Shot);
    // Une section incomplète complète ses trous.
    let partial: Settings = toml::from_str("[theme]\nchoix = \"gris_clair\"\n").unwrap();
    assert_eq!(partial.theme.choix, ChoixTheme::GrisClair);
    assert_eq!(partial.theme.couleurs(), Couleurs::gris_clair());
    let partial: Settings = toml::from_str("[theme]\nchoix = \"personnalise\"\n[theme.perso]\naccent = [10, 20, 30]\n").unwrap();
    assert_eq!(partial.theme.couleurs().accent, [10, 20, 30]);
    assert_eq!(partial.theme.couleurs().panneau, Couleurs::gris_sombre().panneau, "les couleurs absentes gardent le défaut");
    // Aller-retour d'un thème personnalisé.
    let mut s = Settings::default();
    let mut c = Couleurs::gris_clair();
    c.fond = [12, 34, 56];
    c.accent = [1, 2, 3];
    s.theme.choisir(ChoixTheme::GrisClair);
    s.theme.modifier(c);
    let text = toml::to_string(&s).unwrap();
    assert!(text.contains("[theme]") && text.contains("personnalise"), "{text}");
    let back: Settings = toml::from_str(&text).unwrap();
    assert_eq!(back.theme, s.theme);
    assert_eq!(back.theme.couleurs().fond, [12, 34, 56]);
    // Chaque choix survit à l'écriture.
    for choix in ChoixTheme::TOUS {
        let mut s = Settings::default();
        s.theme.choix = choix;
        let back: Settings = toml::from_str(&toml::to_string(&s).unwrap()).unwrap();
        assert_eq!(back.theme.choix, choix);
    }
}

#[test]
fn windows_font_goes_first_and_keeps_the_others() {
    let mut f = FontDefinitions::default();
    let avant = f.families[&FontFamily::Proportional].clone();
    if theme::polices(&mut f) {
        let apres = &f.families[&FontFamily::Proportional];
        assert_eq!(apres[0], "segoe-ui");
        assert_eq!(&apres[1..], &avant[..], "les polices d'egui restent derrière");
    } else {
        assert_eq!(f.families[&FontFamily::Proportional], avant);
    }
}
