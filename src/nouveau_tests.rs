//! Fenêtre « Nouveau projet » : préréglages, papier en pixels, unités, « Personnalisé »,
//! préréglages de l'utilisateur, réglages par défaut et projet créé.

use crate::document::BackgroundKind;
use crate::nouveau::*;
use crate::settings::Settings;

fn preset(name: &str) -> usize {
    PRESETS.iter().position(|p| p.name == name).unwrap_or_else(|| panic!("préréglage {name} absent"))
}

#[test]
fn the_builtin_presets_cover_video_social_and_paper() {
    let by = |c: Category| PRESETS.iter().filter(|p| p.category == c).map(|p| p.name).collect::<Vec<_>>();
    assert_eq!(by(Category::Video), ["HD 1080", "HD 720", "UHD 4K", "2K DCI", "Scope 2,39", "SD PAL 4:3"]);
    assert_eq!(by(Category::Social), ["Vertical 9:16", "Carré 1:1", "Portrait 4:5"]);
    assert_eq!(by(Category::Paper), ["A4", "A3", "A5", "US Letter"]);
    let px = |name: &str| match PRESETS[preset(name)].size {
        Size::Px(w, h) => (w, h),
        Size::Mm(..) => panic!("{name} est du papier"),
    };
    assert_eq!(px("HD 1080"), (1920, 1080));
    assert_eq!(px("HD 720"), (1280, 720));
    assert_eq!(px("UHD 4K"), (3840, 2160));
    assert_eq!(px("2K DCI"), (2048, 1080));
    assert_eq!(px("Scope 2,39"), (2048, 858));
    assert_eq!(px("Vertical 9:16"), (1080, 1920));
    assert_eq!(px("Carré 1:1"), (1080, 1080));
    assert_eq!(px("Portrait 4:5"), (1080, 1350));
    // Chaque préréglage garde la taille permise et porte une cadence.
    for p in PRESETS {
        let a = p.apply(300);
        assert!((MIN_PX..=MAX_PX).contains(&a.width) && (MIN_PX..=MAX_PX).contains(&a.height), "{}", p.name);
        assert!(p.fps >= 1);
    }
}

#[test]
fn presets_carry_their_frame_rate() {
    for p in PRESETS {
        let expected = if p.name == "SD PAL 4:3" { 25 } else { 24 };
        assert_eq!(p.fps, expected, "{}", p.name);
    }
}

#[test]
fn paper_sizes_follow_the_resolution() {
    let a = PRESETS[preset("A4")].apply(300);
    assert_eq!((a.width, a.height, a.dpi, a.capped), (3508, 2480, 300, false), "A4 paysage à 300 dpi");
    let a = PRESETS[preset("A5")].apply(300);
    assert_eq!((a.width, a.height), (2480, 1748));
    let a = PRESETS[preset("US Letter")].apply(300);
    assert_eq!((a.width, a.height), (3300, 2550));
    let a = PRESETS[preset("A4")].apply(150);
    assert_eq!((a.width, a.height), (1754, 1240));
    // Paysage par défaut : le grand côté est la largeur.
    for p in PRESETS.iter().filter(|p| p.is_paper()) {
        let a = p.apply(200);
        assert!(a.width > a.height, "{}", p.name);
    }
}

#[test]
fn big_paper_lowers_the_resolution_to_fit() {
    // A3 à 300 dpi ferait 4961 px : plus que le cadre permis.
    let a = PRESETS[preset("A3")].apply(300);
    assert!(a.capped);
    assert!(a.width <= MAX_PX && a.dpi < 300);
    assert_eq!(a.dpi, 247);
    assert_eq!((a.width, a.height), (4084, 2888));
    // Et le papier ainsi posé se reconnaît encore.
    let mut v = Values::default();
    apply_builtin(&mut v, preset("A3"));
    assert_eq!(v.dpi, 247);
    assert_eq!(detect(&v, &[]), Choice::Builtin(preset("A3")));
    // À 200 dpi, il tient.
    let a = PRESETS[preset("A3")].apply(200);
    assert!(!a.capped);
    assert_eq!((a.width, a.height), (3307, 2339));
}

#[test]
fn units_convert_with_the_resolution() {
    assert_eq!(Unit::Px.from_px(1920, 300), 1920.0);
    assert_eq!(Unit::In.from_px(300, 300), 1.0);
    assert!((Unit::Mm.from_px(3508, 300) - 297.0).abs() < 0.05);
    assert!((Unit::Cm.from_px(2480, 300) - 21.0).abs() < 0.01);
    assert_eq!(Unit::In.to_px(8.5, 300), 2550);
    assert_eq!(Unit::Mm.to_px(210.0, 300), 2480);
    assert_eq!(Unit::Cm.to_px(29.7, 300), 3508);
    assert_eq!(Unit::Mm.to_px(25.4, 96), 96);
    // Gardé dans les limites.
    assert_eq!(Unit::Px.to_px(1.0, 300), MIN_PX);
    assert_eq!(Unit::Px.to_px(100_000.0, 300), MAX_PX);
    assert_eq!(Unit::Px.to_px(f64::NAN, 300), MIN_PX);
}

#[test]
fn units_round_trip_exactly() {
    for unit in Unit::ALL {
        for dpi in [MIN_DPI, 72, 96, 144, 150, 247, 300, 600, MAX_DPI] {
            for px in (MIN_PX..=MAX_PX).step_by(7).chain([MIN_PX, 1080, 1920, 3508, MAX_PX]) {
                let back = unit.to_px(unit.from_px(px, dpi), dpi);
                assert_eq!(back, px, "{} à {dpi} dpi, {px} px", unit.label());
            }
        }
    }
}

#[test]
fn proportions_are_recognised_when_close() {
    assert_eq!(ratio_name(1920, 1080), "16:9");
    assert_eq!(ratio_name(1280, 720), "16:9");
    assert_eq!(ratio_name(1080, 1920), "9:16");
    assert_eq!(ratio_name(1080, 1080), "1:1");
    assert_eq!(ratio_name(1080, 1350), "4:5");
    assert_eq!(ratio_name(1350, 1080), "5:4");
    assert_eq!(ratio_name(768, 576), "4:3");
    assert_eq!(ratio_name(2048, 858), "2,39:1");
    assert_eq!(ratio_name(858, 2048), "1:2,39");
    assert_eq!(ratio_name(1998, 1080), "1,85:1");
    assert_eq!(ratio_name(3508, 2480), "1,41:1 (papier A)");
    // Loin de tout format connu : la fraction, ou le rapport.
    assert_eq!(ratio_name(1000, 700), "10:7");
    assert_eq!(ratio_name(1999, 701), "2,85:1");
}

#[test]
fn an_edited_value_becomes_custom() {
    let mut v = Values::default();
    assert_eq!(detect(&v, &[]), Choice::Builtin(preset("HD 1080")));
    v.width = 1921;
    assert_eq!(detect(&v, &[]), Choice::Custom);
    assert_eq!(Choice::CUSTOM_LABEL, "Personnalisé");
    // La cadence fait partie du préréglage.
    let mut v = Values::default();
    v.fps = 25;
    assert_eq!(detect(&v, &[]), Choice::Custom);
    // Chaque préréglage d'origine se reconnaît une fois posé.
    for i in 0..PRESETS.len() {
        let mut v = Values::default();
        apply_builtin(&mut v, i);
        assert_eq!(detect(&v, &[]), Choice::Builtin(i), "{}", PRESETS[i].name);
        // Le papier garde son nom dans l'autre orientation ; la vidéo, non (9:16 est un autre préréglage).
        if PRESETS[i].is_paper() {
            v.swap();
            assert_eq!(detect(&v, &[]), Choice::Builtin(i), "{} en portrait", PRESETS[i].name);
        }
    }
    // Le papier suit la résolution : A4 à 150 dpi.
    let mut v = Values::default();
    v.dpi = 150;
    apply_builtin(&mut v, preset("A4"));
    assert_eq!((v.width, v.height), (1754, 1240));
    assert_eq!(detect(&v, &[]), Choice::Builtin(preset("A4")));
    v.dpi = 300;
    assert_eq!(detect(&v, &[]), Choice::Custom, "la même taille n'est plus un A4 à 300 dpi");
}

#[test]
fn a_paper_preset_keeps_the_orientation() {
    let mut v = Values::default();
    v.swap();
    assert_eq!((v.width, v.height), (1080, 1920));
    apply_builtin(&mut v, preset("A4"));
    assert_eq!((v.width, v.height), (2480, 3508), "portrait gardé");
    apply_builtin(&mut v, preset("HD 1080"));
    assert_eq!((v.width, v.height, v.fps), (1920, 1080, 24), "la vidéo se pose telle quelle");
}

#[test]
fn user_presets_are_saved_replaced_and_deleted() {
    let mut list: Vec<UserPreset> = Vec::new();
    let mine = |name: &str, w: usize| UserPreset { name: name.into(), width: w, height: 1080, fps: 12, dpi: 96 };
    assert!(save_user_preset(&mut list, mine("Mon HD", 1920)));
    assert!(save_user_preset(&mut list, mine("Autre", 1000)));
    assert_eq!(list.len(), 2);
    // Même nom (casse ignorée) : remplacé, pas doublé.
    assert!(save_user_preset(&mut list, mine("mon hd", 1600)));
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].width, 1600);
    // Nom vide refusé ; espaces rognés.
    assert!(!save_user_preset(&mut list, mine("   ", 800)));
    assert!(save_user_preset(&mut list, mine("  Net  ", 800)));
    assert_eq!(list[2].name, "Net");
    // Un préréglage de l'utilisateur se reconnaît et se pose.
    let mut v = Values::default();
    apply_user(&mut v, &list[1]);
    assert_eq!((v.width, v.height, v.fps, v.dpi), (1000, 1080, 12, 96));
    assert_eq!(detect(&v, &list), Choice::User(1));
    // Suppression : le sien seulement.
    assert!(delete_user_preset(&mut list, "Autre"));
    assert!(!delete_user_preset(&mut list, "Autre"));
    assert!(!delete_user_preset(&mut list, "HD 1080"), "ceux d'origine ne sont pas dans la liste");
    assert_eq!(list.len(), 2);
    assert_eq!(PRESETS.len(), 13, "les préréglages d'origine ne changent pas");
}

#[test]
fn settings_default_to_hd_and_remember_values() {
    let s = Settings::default();
    let n = &s.new_project;
    assert_eq!((n.values.width, n.values.height, n.values.fps, n.values.dpi), (1920, 1080, 24, 300));
    assert_eq!(n.values.unit, Unit::Px);
    assert!(n.values.paper_on);
    assert_eq!(n.values.paper, [255, 255, 255]);
    assert_eq!(n.values.first_seconds, 2.0);
    assert!(n.presets.is_empty() && n.folder.is_none());
    // Un ancien fichier de réglages, sans cette table : les défauts.
    let old: Settings = toml::from_str("copie_secours_minutes = 3\n").unwrap();
    assert_eq!(old.new_project, NewProjectSettings::default());
    // Un fichier partiel complète le reste.
    let part: Settings = toml::from_str("[nouveau_projet.dernieres_valeurs]\nlargeur = 1080\nhauteur = 1920\n").unwrap();
    assert_eq!((part.new_project.values.width, part.new_project.values.height, part.new_project.values.fps), (1080, 1920, 24));
    // Ce qu'on retient revient tel quel, préréglages et dossier compris.
    let mut s = Settings::default();
    s.new_project.values = Values { width: 1080, height: 1350, fps: 12, dpi: 150, unit: Unit::Mm, paper_on: false, paper: [250, 240, 220], fond: [200, 200, 255], first_seconds: 3.5 };
    s.new_project.folder = Some(std::path::PathBuf::from(r"C:\Films"));
    save_user_preset(&mut s.new_project.presets, UserPreset { name: "Pub".into(), width: 1080, height: 1350, fps: 12, dpi: 150 });
    let text = toml::to_string(&s).unwrap();
    assert!(text.contains("nouveau_projet"));
    let back: Settings = toml::from_str(&text).unwrap();
    assert_eq!(back.new_project, s.new_project);
}

#[test]
fn file_names_are_safe() {
    assert_eq!(file_stem("Mon film"), "Mon film");
    assert_eq!(file_stem("  a/b:c*d?  "), "a_b_c_d_");
    assert_eq!(file_stem("fin..."), "fin");
    assert_eq!(file_stem("   "), "");
}

#[test]
fn the_new_project_gets_the_chosen_values() {
    let v = Values { width: 1080, height: 1920, fps: 25, paper: [250, 240, 220], fond: [200, 200, 255], first_seconds: 3.0, ..Values::default() };
    let doc = build_document(&v, "  Vertical  ");
    assert_eq!(doc.frame, [1080, 1920]);
    assert_eq!((doc.width, doc.height), (2160, 3840), "la feuille fait le double du cadre");
    assert_eq!(doc.fps, 25);
    assert_eq!(doc.paper, [250, 240, 220]);
    assert_eq!(doc.background, BackgroundKind::Color);
    assert_eq!(doc.properties.fond, Some([200, 200, 255]));
    assert_eq!(doc.properties.title, "Vertical");
    assert_eq!(doc.panels[0].length, 75, "3 s à 25 i/s");
    assert!(doc.properties.created > 0);
    // Papier décoché : transparent. Fond blanc : rien de posé.
    let doc = build_document(&Values { paper_on: false, ..Values::default() }, "");
    assert_eq!(doc.background, BackgroundKind::None);
    assert_eq!(doc.properties.fond, None);
    assert_eq!(doc.panels[0].length, 48);
    // Des valeurs hors limites sont rentrées.
    let doc = build_document(&Values { width: 99_999, height: 1, fps: 0, ..Values::default() }, "");
    assert_eq!(doc.frame, [MAX_PX, MIN_PX]);
    assert_eq!(doc.fps, 1);
}
