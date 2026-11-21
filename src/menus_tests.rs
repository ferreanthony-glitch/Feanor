//! Les menus du clic droit restent courts (simplification des barres, 2026-10-21) : une dizaine d'entrées au premier
//! niveau au plus ; le rare est dans un sous-menu, rien n'est perdu. On peint chaque menu dans une zone d'egui et on
//! compte ses lignes (les textes posés à des hauteurs différentes ; un raccourci, un sous-menu ou une case sont sur la
//! ligne de leur nom).

use eframe::egui::{self, pos2, vec2, Rect, Shape};

use crate::app::App;

/// Combien d'entrées au plus au premier niveau d'un menu du clic droit.
pub(crate) const MAX_ENTREES: usize = 10;

fn headless(name: &str) -> (egui::Context, App) {
    let dir = std::env::temp_dir().join(format!("shot-menus-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    (ctx, app)
}

fn textes(shape: &Shape, out: &mut Vec<f32>) {
    match shape {
        Shape::Text(t) => out.push(t.pos.y + t.galley.rect.center().y),
        Shape::Vec(v) => v.iter().for_each(|s| textes(s, out)),
        _ => {}
    }
}

/// Les lignes qu'un menu peint : on peint `contenu` dans une zone, deux images, et on compte les hauteurs de texte
/// distinctes (à 6 points près).
pub(crate) fn lignes(ctx: &egui::Context, app: &mut App, contenu: &mut dyn FnMut(&mut App, &mut egui::Ui)) -> usize {
    let mut ys = Vec::new();
    for t in 0..2 {
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(700.0, 1600.0))), time: Some(t as f64 * 0.1), ..Default::default() };
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| contenu(app, ui));
        });
        ys.clear();
        for s in &out.shapes {
            textes(&s.shape, &mut ys);
        }
    }
    ys.sort_by(|a, b| a.total_cmp(b));
    let mut n = 0;
    let mut dernier = f32::NEG_INFINITY;
    for y in ys {
        if y - dernier > 6.0 {
            n += 1;
        }
        dernier = y;
    }
    n
}

#[test]
fn the_right_click_menus_have_about_ten_entries_at_most() {
    let (ctx, mut app) = headless("courts");
    // Un projet un peu garni : trois plans, une transition, un deuxième calque.
    app.doc.add_scene_after(0);
    app.doc.add_scene_after(1);
    let app_transition_id = app.doc.set_transition(1, crate::transitions::Kind::Dissolve);
    let mut trop = Vec::new();
    let mut vus = Vec::new();
    {
        let mut mesure = |nom: &str, f: &mut dyn FnMut(&mut App, &mut egui::Ui)| {
            let n = lignes(&ctx, &mut app, f);
            vus.push(format!("{nom} : {n}"));
            // Le menu d'un plan du montage a jusqu'à douze entrées : la couleur et les deux transitions sont au premier niveau,
            // sans sous-menu « Plus » (choix d'Anthony, 2026-10-28, copie d'écran annotée).
            // Le menu d'un calque a onze entrées au premier niveau, en groupes (§82 : « pas plus d'une douzaine »).
            let max = if nom == "plan (montage)" || nom.starts_with("calque") { MAX_ENTREES + 2 } else { MAX_ENTREES };
            if n > max {
                trop.push(format!("{nom} : {n}"));
            }
        };
        let transition = app_transition_id.expect("une transition");
        mesure("dessin", &mut |a, ui| a.canvas_menu(ui));
        mesure("transition", &mut |a, ui| a.transition_menu(ui, transition));
        mesure("calque, ligne de la timeline", &mut |a, ui| a.layer_head_menu(ui, 0));
        mesure("calque, montage", &mut |a, ui| a.montage_layer_menu(ui, 0));
        mesure("calque, en-tête de la timeline", &mut |a, ui| a.layer_head_menu(ui, 0));
        mesure("fond de la planche des vignettes", &mut |a, ui| a.board_background_menu(ui));
        mesure("images (case de la timeline)", &mut |a, ui| a.images_menu(ui));
        mesure("plan (montage)", &mut |a, ui| a.panel_menu(ui, 0));
        mesure("plans (barre du montage)", &mut |a, ui| a.plans_barre_menu(ui));
        mesure("vignette", &mut |a, ui| a.card_menu(ui, 0, 0, None));
        mesure("vignette (menu du dessin)", &mut |a, ui| a.card_context_menu(ui));
        mesure("coupe", &mut |a, ui| a.cut_menu(ui, 1));
        mesure("clé de caméra", &mut |a, ui| a.cam_key_menu_items(ui));
        mesure("clé d'effet", &mut |a, ui| a.fx_key_menu(ui));
        mesure("clé de position", &mut |a, ui| a.pos_key_menu(ui));
        mesure("règle de la timeline", &mut |a, ui| a.grid_menu_ui(ui, crate::handles::GridMenu::Ruler));
        mesure("ligne de la caméra", &mut |a, ui| a.grid_menu_ui(ui, crate::handles::GridMenu::Camera));
        mesure("ligne du son", &mut |a, ui| a.grid_menu_ui(ui, crate::handles::GridMenu::SoundHeader));
        mesure("rognage d'un plan", &mut |a, ui| a.trim_menu_items(ui, 0));
        mesure("repères", &mut |a, ui| a.cues_menu(ui));
        mesure("affichage de la timeline", &mut |a, ui| a.timeline_view_menu(ui));
        mesure("son", &mut |a, ui| a.sound_menu(ui));
    }
    eprintln!("MENUS\n{}", vus.join("\n"));
    assert!(trop.is_empty(), "menus de plus de {MAX_ENTREES} entrées : {trop:?}");
}

/// Partie 205 : dans un projet qui a de la 3D, le menu d'un plan du Storyboard garde la douzaine (§82) avec « Environnement du
/// plan » : les deux transitions se rangent sous « Transitions du plan » ; un projet sans 3D garde son menu. Le clic droit d'un plan
/// dans la timeline de la séquence (onglet 3D) montre les choix d'environnement directement, et reste court même avec beaucoup de
/// séquences (« Lié à une autre séquence ▸ »).
#[test]
fn the_plan_menu_keeps_a_dozen_entries_in_a_3d_project_and_is_unchanged_without_3d() {
    let textes_du_menu = |ctx: &egui::Context, app: &mut App, f: &mut dyn FnMut(&mut App, &mut egui::Ui)| -> Vec<String> {
        let mut out = Vec::new();
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(700.0, 1600.0))), ..Default::default() };
        let o = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| f(app, ui));
        });
        fn chercher(s: &Shape, out: &mut Vec<String>) {
            match s {
                Shape::Text(t) => out.push(t.galley.text().to_string()),
                Shape::Vec(v) => v.iter().for_each(|s| chercher(s, out)),
                _ => {}
            }
        }
        for s in &o.shapes {
            chercher(&s.shape, &mut out);
        }
        out
    };
    let mut compte = Vec::new();
    for (nom, doc) in [("sans 3D", None), ("3D", Some(crate::decor3d_tests::projet_a_deux_carres()))] {
        let (ctx, mut app) = headless(&format!("plan205-{}", nom.len()));
        if let Some(d) = doc {
            app.doc = d;
        }
        app.doc.add_scene_after(0);
        app.doc.add_scene_after(1);
        for multi in [false, true] {
            app.board_sel = if multi { app.doc.panels.iter().take(2).map(|p| p.id).collect() } else { Vec::new() };
            let n = lignes(&ctx, &mut app, &mut |a, ui| a.panel_menu(ui, 0));
            let t = textes_du_menu(&ctx, &mut app, &mut |a, ui| a.panel_menu(ui, 0));
            compte.push((nom, multi, n, t));
        }
    }
    eprintln!("MENU DU PLAN 205 : {:?}", compte.iter().map(|(n, m, k, _)| (n, m, k)).collect::<Vec<_>>());
    for (nom, multi, n, t) in &compte {
        if *nom == "3D" {
            assert!(*n <= MAX_ENTREES + 2, "3D, plusieurs plans {multi} : {n} entrées");
            assert!(t.iter().any(|x| x == "Environnement du plan") && t.iter().any(|x| x == "Transitions du plan"), "{t:?}");
            assert!(!t.iter().any(|x| x == "Transition au début du plan"), "{t:?}");
        } else {
            // Sans 3D : le menu d'avant, onze entrées (douze avec plusieurs plans choisis), les deux transitions au premier niveau.
            assert_eq!(*n, if *multi { 12 } else { 11 }, "sans 3D, plusieurs plans {multi}");
            assert!(t.iter().any(|x| x == "Transition au début du plan") && t.iter().any(|x| x == "Transition à la fin du plan"), "{t:?}");
            assert!(!t.iter().any(|x| x == "Environnement du plan" || x == "Transitions du plan"), "{t:?}");
        }
    }
    // Aucune commande perdue : les entrées de premier niveau sans 3D se retrouvent avec la 3D (les transitions, un niveau plus bas).
    let sans: Vec<&String> = compte[1].3.iter().filter(|x| !x.starts_with("Transition ")).collect();
    for x in sans {
        assert!(compte[3].3.contains(x), "« {x} » manque au menu d'un projet 3D");
    }
    // La timeline de la séquence : les choix à plat, sous un titre ; huit séquences : « Lié à une autre séquence ▸ ».
    let (ctx, mut app) = headless("plan205-timeline");
    app.doc = crate::decor3d_tests::projet_a_deux_carres();
    for i in 0..7 {
        app.doc.add_sequence_after(i);
    }
    let n = lignes(&ctx, &mut app, &mut |a, ui| a.menu_env_du_plan_a_plat(ui, 0));
    let t = textes_du_menu(&ctx, &mut app, &mut |a, ui| a.menu_env_du_plan_a_plat(ui, 0));
    assert!(n <= MAX_ENTREES, "{n} : {t:?}");
    assert!(t.iter().any(|x| x == "Propre : vide") && t.iter().any(|x| x == "Lié à une autre séquence") && t.iter().any(|x| x.starts_with("Environnement du plan")), "{t:?}");
    assert!(!t.iter().any(|x| x.starts_with("Lié à la séquence")), "{t:?}");
    // Trois séquences : les liens à plat.
    let (ctx, mut app) = headless("plan205-timeline3");
    app.doc = crate::decor3d_tests::projet_a_deux_carres();
    app.doc.add_sequence_after(0);
    app.doc.add_sequence_after(1);
    let t = textes_du_menu(&ctx, &mut app, &mut |a, ui| a.menu_env_du_plan_a_plat(ui, 0));
    assert_eq!(t.iter().filter(|x| x.starts_with("Lié à la séquence")).count(), 2, "{t:?}");
}
