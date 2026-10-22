//! Essais des clés de case (`cases.rs`) et de la case vivante après un travelling (2026-10-22).

use eframe::egui::{vec2, Color32};

use crate::decor::Genre;
use crate::document::{CelId, Document, LayerKind};
use crate::perf_tests::se_reposer;
use crate::raster::TILE;
use crate::vignettes_tests::{case_vide, dernier_nom, encre, glisser, ouvert, pas_d_annulation, quatre_plans, vague, vignette};

fn pixel(doc: &mut Document, cel: CelId, x: usize, y: usize, c: Color32) {
    let k = doc.cels.get_mut(&cel).expect("le dessin");
    let i = k.raster.tile_index_of(x, y);
    k.raster.tile_mut(i)[(y % TILE) * TILE + x % TILE] = c;
    k.version += 1;
}

fn a_de_l_encre(doc: &Document, cel: CelId) -> bool {
    doc.cels[&cel].raster.tiles.iter().flatten().any(|t| t.iter().any(|p| p.a() != 0))
}

/// Un plan de douze images dont deux calques de dessin (« Dessin » : un dessin tenu ; « Calque 2 » : rien) et un calque Décor.
fn un_plan_a_dessins() -> (Document, CelId, CelId) {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.set_duration(0, 12);
    let x = doc.new_cel(LayerKind::Bitmap);
    pixel(&mut doc, x, 20, 20, Color32::from_rgb(200, 30, 30));
    for f in 0..12 {
        doc.panels[0].layers[0].set(f, Some(x));
    }
    doc.panels[0].layers.push(crate::document::Layer::new("Calque 2", LayerKind::Bitmap));
    doc.assurer_decor(0, 0).unwrap();
    let d = doc.panels[0].layers[doc.decor_de(0).unwrap()].cel_at(0).unwrap();
    pixel(&mut doc, d, 60, 50, Color32::from_rgb(0x8F, 0xA3, 0xB8));
    (doc, x, d)
}

#[test]
fn a_new_case_puts_a_copy_of_the_held_drawing_at_its_frame_on_each_drawing_layer_and_nothing_on_the_decor() {
    let (mut doc, x, d) = un_plan_a_dessins();
    doc.toggle_mark(0, 6).unwrap();
    assert_eq!(doc.cles_de_case(0, 6), 1, "une seule clé : « Dessin » ; « Calque 2 » n'a rien dessiné, le décor reste un seul dessin");
    let l = &doc.panels[0].layers[doc.panels[0].layers.iter().position(|l| l.name == "Dessin").unwrap()];
    let k = l.cel_at(6).expect("la clé de la case");
    assert_ne!(k, x, "un dessin à elle");
    assert_eq!(l.frames[..6], [Some(x); 6], "la case d'avant garde le sien");
    assert_eq!(l.frames[6..], [Some(k); 6], "jusqu'à la fin de l'exposition");
    // Une copie : rien ne saute à l'écran.
    assert_eq!(doc.cels[&k].raster.get(20, 20), doc.cels[&x].raster.get(20, 20));
    assert!(a_de_l_encre(&doc, k));
    // Le décor n'a pas de clé de plus.
    let at = doc.decor_de(0).unwrap();
    assert_eq!(doc.panels[0].layers[at].frames, vec![Some(d); 12]);
    // L'autre calque est resté vide.
    let autre = doc.panels[0].layers.iter().find(|l| l.name == "Calque 2").unwrap();
    assert!(autre.frames.iter().all(Option::is_none));
    // Retoucher la case 2 ne change pas la case 1.
    pixel(&mut doc, k, 90, 40, Color32::BLACK);
    assert_eq!(doc.cels[&x].raster.get(90, 40).a(), 0);
}

#[test]
fn a_layer_that_had_a_drawing_but_not_the_new_case_gets_a_blank_key() {
    let (mut doc, x, _) = un_plan_a_dessins();
    // Le dessin de « Dessin » ne va que jusqu'à l'image 6 : la case suivante n'a rien sur ce calque.
    let l = &mut doc.panels[0].layers[1];
    for f in 6..12 {
        l.set(f, None);
    }
    doc.toggle_mark(0, 6).unwrap();
    assert_eq!(doc.cles_de_case(0, 6), 1);
    let vide = doc.panels[0].layers[1].cel_at(6).expect("un dessin vide pour la case");
    assert_ne!(vide, x);
    assert!(!a_de_l_encre(&doc, vide));
    assert_eq!(doc.panels[0].layers[1].frames[..6], [Some(x); 6]);
    // Rien d'autre qu'une marque d'image de plus, et jamais à la première image ou après la dernière.
    assert_eq!(doc.cles_de_case(0, 0), 0);
    assert_eq!(doc.cles_de_case(0, 12), 0);
}

fn dessin_de(b: &crate::perf_tests::Banc, plan: usize, nom: &str) -> Vec<Option<CelId>> {
    b.app.doc.panels[plan].layers.iter().find(|l| l.name == nom).map(|l| l.frames.clone()).unwrap_or_default()
}

#[test]
fn drawing_in_the_pale_case_after_the_last_plan_without_a_cut_adds_a_key_and_one_undo_removes_the_case_and_its_keys() {
    let mut b = ouvert(quatre_plans(), 4);
    // Un dessin dans le dernier plan (carte 5), puis la barre grise devant la première case vide.
    let v = vignette(&b, 5);
    glisser(&mut b, &vague(v.center() - vec2(30.0, 0.0), v.center() + vec2(30.0, 0.0), 10));
    let dernier = b.app.doc.panels.len() - 1;
    assert!(encre(&b.app.doc, dernier));
    let avant = b.app.doc.panels[dernier].clone();
    let nom = b.app.doc.panels[dernier].layers[b.app.cur_layer].name.clone();
    let x = dessin_de(&b, dernier, &nom)[0].expect("le dessin tenu du dernier plan");
    let (n, plans) = (pas_d_annulation(&b), b.app.doc.panels.len());
    b.app.vignettes.empty_joins = true;
    let e = case_vide(&b, 0);
    glisser(&mut b, &vague(e.center() - vec2(20.0, 0.0), e.center() + vec2(20.0, 5.0), 10));
    assert_eq!(b.app.doc.panels.len(), plans, "une case de plus, pas un plan de plus");
    assert_eq!(pas_d_annulation(&b), n + 1, "la case, ses clés et le trait : une seule étape");
    assert_eq!(dernier_nom(&b), "Nouvelle image dessinée");
    let p = &b.app.doc.panels[dernier];
    let debut = p.marks.last().expect("la marque de la case").frame;
    assert!(debut > 0);
    // Une clé à l'image de la case, sur le calque de dessin : un dessin neuf, avec le trait de la case, sans toucher à la case d'avant.
    let frames = dessin_de(&b, dernier, &nom);
    let cle = frames[debut].expect("la clé de la case");
    assert_ne!(cle, x, "un dessin à elle");
    assert_eq!(frames[debut - 1], Some(x), "la case d'avant garde le sien");
    assert!(a_de_l_encre(&b.app.doc, cle) && a_de_l_encre(&b.app.doc, x));
    // Le trait fait dans la case 2 n'a pas changé la case 1 : son dessin est celui d'avant.
    let ancien = avant.layers.iter().find(|l| l.name == nom).unwrap().cel_at(0).unwrap();
    assert_eq!(ancien, x);
    // Une annulation défait la case, ses clés et le trait.
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(b.app.doc.panels[dernier].frame_count(), avant.frame_count());
    assert_eq!(dessin_de(&b, dernier, &nom), avant.layers.iter().find(|l| l.name == nom).unwrap().frames);
    assert!(b.app.doc.panels[dernier].marks.is_empty());
    assert_eq!(pas_d_annulation(&b), n);
}

#[test]
fn the_decor_gets_no_key_when_a_case_is_added_to_a_plan() {
    let mut b = ouvert(quatre_plans(), 4);
    crate::decor_tests::choisir(&mut b, Genre::Aplat);
    assert_eq!(b.app.perso.decor, Some(Genre::Aplat));
    let v = vignette(&b, 5);
    glisser(&mut b, &vague(v.center() - vec2(30.0, 0.0), v.center() + vec2(30.0, 0.0), 10));
    let dernier = b.app.doc.panels.len() - 1;
    b.app.vignettes.empty_joins = true;
    let e = case_vide(&b, 0);
    glisser(&mut b, &vague(e.center() - vec2(20.0, 0.0), e.center() + vec2(20.0, 5.0), 10));
    let at = b.app.doc.decor_de(dernier).expect("le décor");
    let frames = &b.app.doc.panels[dernier].layers[at].frames;
    assert_eq!(frames.iter().flatten().collect::<std::collections::BTreeSet<_>>().len(), 1, "un seul dessin de décor pour toute la continuité");
    assert!(b.app.doc.panels[dernier].marks.len() == 1, "la case de plus est bien faite");
}

#[test]
fn uniting_two_plans_on_the_board_puts_a_key_at_the_start_of_the_right_case_and_one_undo_takes_it_back() {
    let mut b = ouvert(quatre_plans(), 4);
    // Un dessin dans le plan 1 (carte 0) : le plan 2 (carte 3) n'a rien sur ce calque.
    let v = vignette(&b, 0);
    glisser(&mut b, &vague(v.center() - vec2(30.0, 0.0), v.center() + vec2(30.0, 0.0), 10));
    let nom = b.app.layer().name.clone();
    let x = dessin_de(&b, 0, &nom)[0].expect("le dessin du plan 1");
    let a_len = b.app.doc.panels[0].frame_count();
    let avant = b.app.doc.panels[0].layers.iter().find(|l| l.name == nom).unwrap().frames.clone();
    let n = pas_d_annulation(&b);
    let layout = b.app.board_layout();
    assert!(b.app.board_toggle_cut(&layout, 2), "la coupe entre les plans 1 et 2 se décoche");
    b.chauffer(2);
    assert_eq!(pas_d_annulation(&b), n + 1);
    let frames = dessin_de(&b, 0, &nom);
    let cle = frames[a_len].expect("une clé à l'image de la case de droite");
    assert_ne!(cle, x);
    assert!(!a_de_l_encre(&b.app.doc, cle), "vide : la case de droite était vide");
    assert_eq!(frames[a_len - 1], Some(x));
    // Dessiner dans la case de droite ne change pas celle de gauche.
    let gauche_avant = b.app.doc.cels[&x].raster.clone();
    let v = vignette(&b, 3);
    glisser(&mut b, &vague(v.center() - vec2(30.0, 0.0), v.center() + vec2(30.0, 0.0), 10));
    assert_eq!(b.app.doc.cels[&x].raster.tiles, gauche_avant.tiles, "la case de gauche n'a pas bougé");
    assert!(a_de_l_encre(&b.app.doc, b.app.doc.panels[0].layers.iter().find(|l| l.name == nom).unwrap().cel_at(a_len).unwrap()));
    // Une seule annulation du trait, puis de la réunion et de ses clés.
    b.app.run_action("edition.annuler");
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(b.app.doc.panels.len(), 4);
    assert_eq!(b.app.doc.panels[0].layers.iter().find(|l| l.name == nom).unwrap().frames, avant);
    assert_eq!(pas_d_annulation(&b), n);
}

#[test]
fn the_mark_command_also_puts_a_key_on_each_drawing_layer() {
    let (doc, x, d) = un_plan_a_dessins();
    let mut b = crate::perf_tests::banc(doc, false);
    b.app.cur_frame = 6;
    b.app.run_action("marque.basculer");
    b.chauffer(2);
    let l = &b.app.doc.panels[0].layers.iter().find(|l| l.name == "Dessin").unwrap().frames;
    assert_ne!(l[6], Some(x), "la marque pose une clé");
    assert_eq!(l[5], Some(x));
    let at = b.app.doc.decor_de(0).unwrap();
    assert_eq!(b.app.doc.panels[0].layers[at].frames, vec![Some(d); 12]);
    // Une seule étape pour la marque et ses clés : l'annuler les retire ensemble.
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert!(b.app.doc.panels[0].marks.is_empty());
    assert_eq!(b.app.doc.panels[0].layers.iter().find(|l| l.name == "Dessin").unwrap().frames, vec![Some(x); 12]);
}

// ------------------------------------------------------------------------------ la case vivante après un travelling

#[test]
fn the_live_case_after_a_travelling_shows_what_lies_beyond_the_sheet_where_camera_b_looks() {
    use crate::camera::set_key;
    let mut doc = Document::with_frame(160, 90, 24);
    doc.set_duration(0, 24);
    doc.toggle_mark(0, 12);
    let a = doc.default_camera();
    set_key(&mut doc.panels[0].camera, 0, a);
    // B regarde une largeur de cadre plus loin à droite : la moitié de sa vue est hors de la feuille (qui déborde d'une demi-largeur).
    set_key(&mut doc.panels[0].camera, 11, a.shifted(160.0, 0.0));
    let cel = doc.new_cel(LayerKind::Bitmap);
    for f in 0..24 {
        doc.panels[0].layers[0].set(f, Some(cel));
    }
    // Du trait hors de la feuille (320 × 180), à droite : la tuile hors-feuille (5, 0).
    let rouge = Color32::from_rgb(210, 40, 40);
    for p in doc.cels.get_mut(&cel).unwrap().raster.tile_mut(crate::raster::outer_id(5, 0)).iter_mut() {
        *p = rouge;
    }
    let mut b = ouvert(doc, 6);
    // La case qui suit le travelling devient la case vivante.
    b.app.select_board(0, 12);
    b.chauffer(4);
    for _ in 0..200 {
        b.image(Vec::new(), 1.0 / 60.0);
        if b.app.monde.pending() == 0 && b.app.monde.pixel(340, 55).is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let c = b.app.monde.pixel(340, 55).expect("le hors-feuille est composé pour la case vivante qui voit au-delà de la feuille");
    assert!(c.r() > 150 && c.g() < 100, "le trait d'au-delà de la feuille : {c:?}");
    se_reposer(&mut b, 400);
}

/// (Pas un essai : fabrique, pour la vérification à l'écran, un projet dont un plan a un travelling vers la droite qui sort de la feuille, et
/// une deuxième case qui le suit. `cargo test … fabrique_le_projet_du_travelling -- --ignored`, avec SHOT_ESSAI_SORTIE = le dossier.)
#[test]
#[ignore]
fn fabrique_le_projet_du_travelling() {
    use crate::camera::set_key;
    let Ok(sortie) = std::env::var("SHOT_ESSAI_SORTIE") else { return };
    let mut doc = Document::with_frame(160, 90, 24);
    doc.set_duration(0, 24);
    doc.toggle_mark(0, 12);
    let a = doc.default_camera();
    set_key(&mut doc.panels[0].camera, 0, a);
    set_key(&mut doc.panels[0].camera, 11, a.shifted(160.0, 0.0));
    let cel = doc.new_cel(LayerKind::Bitmap);
    for f in 0..24 {
        doc.panels[0].layers[0].set(f, Some(cel));
    }
    // Des bandes verticales sur la feuille (320 × 180), du rouge au-delà de son bord droit (tuiles hors-feuille 5 et 6).
    for x in 0..320usize {
        for y in 0..180usize {
            let c = if (x / 40) % 2 == 0 { Color32::from_rgb(120, 150, 200) } else { Color32::from_rgb(235, 235, 240) };
            pixel(&mut doc, cel, x, y, c);
        }
    }
    for tx in 5..=6 {
        for ty in 0..3 {
            let t = doc.cels.get_mut(&cel).unwrap().raster.tile_mut(crate::raster::outer_id(tx, ty));
            for (i, p) in t.iter_mut().enumerate() {
                *p = if (i % TILE / 8) % 2 == 0 { Color32::from_rgb(210, 40, 40) } else { Color32::from_rgb(240, 150, 60) };
            }
        }
    }
    let dossier = std::path::PathBuf::from(sortie);
    std::fs::create_dir_all(&dossier).unwrap();
    crate::project::save(&doc, &dossier.join("Travelling.pelure"), &mut Default::default()).expect("projet");
}

// ------------------------------------------------------------------------------ un mouvement créé sur une case qui a une continuité derrière elle

fn deux_cases_a_dessins() -> Document {
    let mut doc = crate::vignettes_camera_tests::plan_court();
    doc.set_duration(1, 24);
    doc.toggle_mark(1, 12);
    let (x, y) = (doc.new_cel(LayerKind::Bitmap), doc.new_cel(LayerKind::Bitmap));
    pixel(&mut doc, x, 30, 30, Color32::from_rgb(200, 30, 30));
    pixel(&mut doc, y, 60, 50, Color32::from_rgb(30, 160, 60));
    for f in 0..12 {
        doc.panels[1].layers[0].set(f, Some(x));
    }
    for f in 12..24 {
        doc.panels[1].layers[0].set(f, Some(y));
    }
    doc
}

#[test]
fn a_camera_move_created_on_a_case_with_continuity_cases_splits_them_off_untouched_in_one_undo_step() {
    use crate::vignettes_camera_tests::{ctrl_glisser, largeur_cadre_ecran, pas, K};
    let mut b = ouvert(deux_cases_a_dessins(), 6);
    let (case1, case2, w) = (vignette(&b, K), vignette(&b, K + 1), largeur_cadre_ecran(&b));
    se_reposer(&mut b, 400);
    let largeur = crate::thumbs::width_for(case2.width() * b.ctx.pixels_per_point());
    let id2 = b.app.board_layout().cards[K + 1].id;
    let avant = b.app.thumbs.case_image_of(id2, largeur).map(|(i, _)| i.pixels.clone()).expect("la vignette de la case 2");
    let (camera_avant, plans, n) = (b.app.doc.panel_camera(1, 12), b.app.doc.panels.len(), pas_d_annulation(&b));
    let cels_avant = b.app.doc.panels[1].layers[0].frames.clone();
    assert!(b.app.doc.panels[1].camera.is_empty() && b.app.doc.panels[1].marks.len() == 1);
    // Un mouvement sur la case 1.
    ctrl_glisser(&mut b, case1.center(), &pas(case1.center(), vec2(w, 0.0), 6), case1.center() + vec2(w, 0.0), true);
    b.chauffer(3);
    assert_eq!(b.app.doc.panels.len(), plans + 1, "deux plans : la case 1 avec son mouvement, la case 2 à part");
    assert_eq!(pas_d_annulation(&b), n + 1, "le mouvement et la coupe : une seule étape");
    assert_eq!(dernier_nom(&b), "Mouvement de caméra");
    let (premier, suite) = (&b.app.doc.panels[1], &b.app.doc.panels[2]);
    assert_eq!((premier.camera.len(), premier.marks.len(), premier.frame_count()), (2, 0, 12), "le mouvement ne montre que la case 1");
    assert_eq!((suite.camera.is_empty(), suite.marks.len(), suite.frame_count()), (true, 0, 12), "la case 2 est un plan sans mouvement");
    assert_eq!(b.app.doc.panel_camera(2, 0), camera_avant, "sa caméra est celle qu'elle montrait");
    assert_ne!(premier.id, suite.id);
    // Son dessin : celui d'avant (une copie), sans le dessin de la case 1.
    let y = suite.layers[0].cel_at(0).expect("son dessin");
    assert_ne!(Some(y), cels_avant[12], "une copie, au plan neuf");
    assert_eq!(b.app.doc.cels[&y].raster.get(60, 50), Color32::from_rgb(30, 160, 60));
    assert_eq!(b.app.doc.cels[&y].raster.get(30, 30).a(), 0);
    assert_eq!(premier.layers[0].cel_at(0), cels_avant[0], "la case 1 garde le sien");
    // Ses pixels sont ceux de l'ancienne vignette.
    se_reposer(&mut b, 400);
    let id_suite = b.app.board_layout().cards[K + 1].id;
    assert_eq!(id_suite, b.app.doc.panels[2].id);
    let apres = b.app.thumbs.case_image_of(id_suite, largeur).map(|(i, _)| i.pixels.clone()).expect("la vignette du plan neuf");
    assert_eq!(apres, avant, "la vignette de l'ancienne case 2 n'a pas changé d'un pixel");
    // Déplacer B ensuite ne touche à aucun dessin : le plan d'à côté est resté tel quel.
    let (suite_avant, tuiles_avant) = (b.app.doc.panels[2].clone(), b.app.doc.cels[&y].raster.tiles.clone());
    let c = crate::vignettes_camera_tests::centre(crate::vignettes_camera_tests::cadre_ecran(&b, 1));
    ctrl_glisser(&mut b, c, &pas(c, vec2(0.0, 0.3 * w), 6), c + vec2(0.0, 0.3 * w), true);
    b.chauffer(2);
    assert_eq!(b.app.doc.panels.len(), plans + 1, "déplacer B ne coupe rien de plus");
    let suite = &b.app.doc.panels[2];
    assert_eq!((suite.id, suite.layers[0].frames.clone(), suite.camera.clone()), (suite_avant.id, suite_avant.layers[0].frames.clone(), suite_avant.camera.clone()));
    assert_eq!(b.app.doc.cels[&y].raster.tiles, tuiles_avant);
    assert_eq!(b.app.doc.panels[1].layers[0].cel_at(0), cels_avant[0], "ni le dessin de la case 1");
    // Annuler le déplacement de B, puis le mouvement : un seul plan de deux cases, sans mouvement.
    b.app.run_action("edition.annuler");
    b.app.run_action("edition.annuler");
    b.chauffer(3);
    assert_eq!(b.app.doc.panels.len(), plans, "un seul plan");
    let p = &b.app.doc.panels[1];
    assert!(p.camera.is_empty(), "plus de mouvement");
    assert_eq!((p.marks.len(), p.frame_count(), p.layers[0].frames.clone()), (1, 24, cels_avant), "ses deux cases et leurs dessins");
    assert_eq!(pas_d_annulation(&b), n);
}

#[test]
fn a_continuity_case_made_after_the_move_still_shows_the_last_frame_and_a_move_on_a_plan_of_one_case_cuts_nothing() {
    use crate::vignettes_camera_tests::{ctrl_glisser, largeur_cadre_ecran, pas, K};
    // Une seule case : rien à couper, un seul plan de plus que les quatre d'origine n'apparaît pas.
    let mut doc = crate::vignettes_camera_tests::plan_court();
    doc.set_duration(1, 12);
    let mut b = ouvert(doc, 6);
    let (case1, w, plans) = (vignette(&b, K), largeur_cadre_ecran(&b), b.app.doc.panels.len());
    ctrl_glisser(&mut b, case1.center(), &pas(case1.center(), vec2(w, 0.0), 6), case1.center() + vec2(w, 0.0), true);
    assert_eq!((b.app.doc.panels.len(), b.app.doc.panels[1].camera.len()), (plans, 2));
    // Une case de continuité faite après le mouvement : elle voit à travers le dernier cadre, et le plan ne se coupe pas.
    b.app.doc.set_duration(1, 24);
    b.app.doc.toggle_mark(1, 12);
    let b_cam = b.app.doc.panels[1].camera.last().unwrap().cam;
    assert_eq!(b.app.doc.panel_camera(1, 20).x, b_cam.x, "la case suivante voit à travers B");
    assert_eq!(b.app.doc.panels.len(), plans);
}
