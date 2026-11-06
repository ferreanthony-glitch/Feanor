//! Essais de la passe « Vignettes plus simples » (partie 113) : le pont entre les Vignettes et le Storyboard ne piège plus (Maj+Tab, le
//! bouton de retour de l'animation, une disposition retenue sur le dessin), une commande de plan montre le plan qu'elle choisit et l'amène
//! dans la vue, et ce qui ne fait rien le dit.

use eframe::egui::{epaint::Shape, Modifiers, Pos2};

use crate::app::{BoardView, Tool};
use crate::boardui::BORDER_CURRENT;
use crate::perf_tests::Banc;
use crate::vignettes_tests::{clic_avec, ouvert, quatre_plans};
use crate::workspace::Espace;

/// Les Vignettes ouvertes sur quatre plans, comme par l'onglet.
fn dans_les_vignettes() -> Banc {
    let mut b = ouvert(quatre_plans(), 4);
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(4);
    b
}

/// Le centre du premier texte peint par la dernière image qui contient `texte`.
fn texte_a_l_ecran(b: &Banc, texte: &str) -> Option<Pos2> {
    b.formes.iter().find_map(|s| match &s.shape {
        Shape::Text(t) if t.galley.job.text.contains(texte) => Some(t.pos + t.galley.size() / 2.0),
        _ => None,
    })
}

/// Les cadres noirs de sélection peints à la dernière image.
fn cadres_noirs(b: &mut Banc) -> usize {
    b.garder_formes = true;
    b.chauffer(2);
    let vue = b.app.vignettes.view;
    b.formes.iter().filter(|s| matches!(&s.shape, Shape::Rect(r) if r.stroke.color == BORDER_CURRENT && vue.contains_rect(r.rect.expand(3.0)))).count()
}

// ------------------------------------------------------------------------------ le pont, sans piège

#[test]
fn shift_tab_from_the_vignettes_goes_to_the_storyboard_drawing_and_the_vignettes_always_reopen_on_the_board() {
    let mut b = dans_les_vignettes();
    assert!(b.app.board_shown());
    // Maj+Tab (« vue dessin / vue vignettes ») : depuis les Vignettes, c'est le dessin du Storyboard, jamais le dessin seul sous
    // l'onglet Vignettes (d'où rien ne ramenait la planche : l'onglet, déjà allumé, ne faisait rien).
    b.app.run_action("vue.planches");
    b.chauffer(3);
    assert!(b.app.espace == Espace::Storyboard && b.app.board_view == BoardView::Scene, "Maj+Tab depuis les Vignettes : le dessin du Storyboard ({:?})", b.app.espace);
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(3);
    assert!(b.app.board_shown(), "les Vignettes rouvrent sur la planche");
    // Une disposition retenue qui dit « pas la planche » (un ancien réglage) s'ouvre quand même sur la planche.
    let mut l = b.app.settings.layout_of(Espace::Vignettes).cloned().unwrap_or_else(|| Espace::Vignettes.defaut());
    l.vignettes = false;
    b.app.settings.set_layout_of(Espace::Vignettes, l);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(2);
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(3);
    assert!(b.app.board_shown(), "l'onglet Vignettes, c'est toujours la planche");
    // Et la disposition que l'on retient pour les Vignettes dit la planche (elle s'écrit une fois les panneaux rangés).
    b.chauffer(16);
    assert!(b.app.settings.layout_of(Espace::Vignettes).is_some_and(|l| l.vignettes), "la disposition retenue des Vignettes porte la planche");
}

#[test]
fn entered_from_the_vignettes_the_animation_returns_to_the_vignettes_by_its_button_as_by_tab() {
    let mut b = dans_les_vignettes();
    b.garder_formes = true;
    b.app.enter_animation(2);
    b.chauffer(4);
    assert_eq!(b.app.espace, Espace::Animation);
    let p = texte_a_l_ecran(&b, "Retour aux Vignettes").expect("venu des Vignettes, le bouton du haut dit « Retour aux Vignettes »");
    assert!(p.y < 40.0, "dans la barre du haut : {p:?}");
    clic_avec(&mut b, p, Modifiers::NONE);
    assert_eq!(b.app.espace, Espace::Vignettes, "le bouton ramène aux Vignettes");
    assert!(b.app.board_shown());
    assert_eq!(b.app.cur_panel, 2, "sur le même plan");
    // Tab fait la même chose.
    b.app.enter_animation(1);
    b.chauffer(2);
    b.app.run_action("mode.basculer");
    assert_eq!(b.app.espace, Espace::Vignettes, "Tab aussi");
    // Venu du Storyboard : le bouton dit « Retour au storyboard » et y revient, comme avant.
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(2);
    b.app.enter_animation(1);
    b.chauffer(4);
    let p = texte_a_l_ecran(&b, "Retour au storyboard").expect("venu du Storyboard : « Retour au storyboard »");
    clic_avec(&mut b, p, Modifiers::NONE);
    assert_eq!(b.app.espace, Espace::Storyboard);
}

// ------------------------------------------------------------------------------ le plan courant se voit

#[test]
fn a_plan_command_in_the_vignettes_shows_which_plan_is_current_and_brings_it_into_view() {
    let mut b = dans_les_vignettes();
    assert!(b.app.vignettes.libre);
    assert_eq!(cadres_noirs(&mut b), 0);
    b.app.run_action("panneau.suivant");
    b.chauffer(2);
    assert_eq!(b.app.cur_panel, 1);
    assert!(!b.app.vignettes.libre, "« Plan suivant » choisit le plan : son cadre se voit");
    assert_eq!(cadres_noirs(&mut b), 1);
    // Un nouveau plan : il est le plan courant, et on le voit.
    let n = b.app.doc.panels.len();
    b.app.run_action("panneau.nouveau");
    b.chauffer(2);
    assert_eq!(b.app.doc.panels.len(), n + 1);
    assert_eq!(b.app.cur_panel, 2);
    assert!(!b.app.vignettes.libre && cadres_noirs(&mut b) == 1, "le plan neuf est choisi, on sait où il est");
    // Dessiner ensuite libère le choix, comme toujours.
    let v = crate::vignettes_tests::vignette(&b, 0);
    let chemin = crate::vignettes_tests::vague(v.center() - eframe::egui::vec2(20.0, 0.0), v.center() + eframe::egui::vec2(20.0, 0.0), 8);
    crate::vignettes_tests::glisser(&mut b, &chemin);
    assert!(b.app.vignettes.libre);

    // Un long projet : « Plan suivant » loin de la vue fait défiler la planche jusqu'à la case courante.
    let mut doc = quatre_plans();
    for _ in 0..80 {
        let last = doc.panels.len() - 1;
        doc.add_scene_after(last);
    }
    let mut b = ouvert(doc, 6);
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(3);
    b.app.select_panel(0);
    b.chauffer(2);
    let loin = 60;
    let k = b.app.board_layout().cards.iter().position(|c| c.panel == loin).expect("la case du plan 60");
    assert!(!b.app.vignettes.view.intersects(crate::vignettes_tests::vignette(&b, k)), "au départ, la case du plan 60 est hors de la vue");
    for _ in 0..loin {
        b.app.run_action("panneau.suivant");
    }
    b.chauffer(3);
    assert_eq!(b.app.cur_panel, loin);
    let r = crate::vignettes_tests::vignette(&b, k);
    assert!(b.app.vignettes.view.contains_rect(r), "la planche a défilé jusqu'à la case courante : {r:?} dans {:?}", b.app.vignettes.view);
    assert_eq!(cadres_noirs(&mut b), 1, "et son cadre se voit");
}

// ------------------------------------------------------------------------------ décisions 1, 5, 8, 10, 11 ; propositions A, B, C

#[test]
fn shift_tab_is_the_bridge_both_ways_and_tab_enters_the_plan_from_the_vignettes() {
    let mut b = dans_les_vignettes();
    // Du Storyboard, Maj+Tab mène aux Vignettes (décision 1) ; l'ancienne « vue vignettes » du Storyboard n'existe plus.
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(2);
    b.app.run_action("vue.planches");
    b.chauffer(2);
    assert!(b.app.espace == Espace::Vignettes && b.app.board_shown(), "Maj+Tab depuis le Storyboard : les Vignettes");
    b.app.run_action("vue.planches");
    b.chauffer(2);
    assert!(b.app.espace == Espace::Storyboard && b.app.board_view == BoardView::Scene, "et retour au dessin du Storyboard");
    assert!(crate::keymap::action_info("espace.ensemble").is_none(), "plus de disposition « Vue d'ensemble »");
    // Tab, des Vignettes : le plan courant s'anime ; Tab encore : retour aux Vignettes.
    b.app.set_espace(Espace::Vignettes);
    b.app.select_panel(2);
    b.chauffer(2);
    b.app.run_action("mode.basculer");
    assert_eq!((b.app.espace, b.app.cur_panel), (Espace::Animation, 2), "Tab entre dans le plan");
    b.app.run_action("mode.basculer");
    assert!(b.app.espace == Espace::Vignettes && b.app.board_shown(), "Tab en revient");
}

#[test]
fn a_tail_of_stroke_only_makes_the_next_plan_never_farther_ones_and_the_status_counts_the_empty_plans() {
    let mut b = dans_les_vignettes();
    let n = b.app.doc.panels.len();
    // Un bout de trait dans la troisième case pâle (pas la première) : rien n'est créé, le bout reste sur la planche.
    let loin = crate::vignettes_tests::case_vide(&b, 2);
    let de = eframe::egui::pos2(loin.center().x, loin.bottom() + 40.0);
    let chemin = crate::vignettes_tests::vague(de, loin.center(), 20);
    crate::vignettes_tests::glisser(&mut b, &chemin);
    b.chauffer(6);
    assert_eq!(b.app.doc.panels.len(), n, "aucun plan vide né d'un bout de trait : {}", b.app.status);
    let a = &b.app.doc.annotations;
    assert!(a.traces.iter().any(|t| a.calque(t.calque).is_some_and(|c| c.planche)), "le bout est sur la planche, pas perdu");
    // Un bout dans la première case pâle : exactement un plan de plus.
    let suivante = crate::vignettes_tests::case_vide(&b, 0);
    let de = eframe::egui::pos2(suivante.center().x, suivante.bottom() + 40.0);
    let chemin = crate::vignettes_tests::vague(de, suivante.center(), 20);
    crate::vignettes_tests::glisser(&mut b, &chemin);
    b.chauffer(6);
    assert_eq!(b.app.doc.panels.len(), n + 1, "le plan suivant, et lui seul");
    // Dessiner dans une case pâle lointaine garde sa place, comme avant, et la ligne d'état compte les plans vides.
    let loin = crate::vignettes_tests::case_vide(&b, 2);
    let chemin = crate::vignettes_tests::vague(loin.center() - eframe::egui::vec2(20.0, 0.0), loin.center() + eframe::egui::vec2(20.0, 0.0), 8);
    crate::vignettes_tests::glisser(&mut b, &chemin);
    b.chauffer(4);
    assert_eq!(b.app.doc.panels.len(), n + 4);
    assert!(b.app.status.contains("2 plans vides"), "{}", b.app.status);
}

#[test]
fn shift_alone_moves_a_plan_from_anywhere_in_its_case_chooses_it_on_a_click_and_pulls_the_camera_at_a_corner() {
    use eframe::egui::{Event, Key};
    let mut b = dans_les_vignettes();
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    let n = crate::vignettes_tests::pas_d_annulation(&b);
    // Maj tenue, sans appui : les poignées se montrent, rien ne change, aucune étape ; la ligne d'état dit ce que Maj permet.
    b.garder_formes = true;
    b.touches = Modifiers::SHIFT;
    let v = crate::vignettes_tests::vignette(&b, 3);
    b.image(vec![Event::PointerMoved(v.center())], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    let ronds = b.formes.iter().filter(|s| matches!(s.shape, Shape::Circle(_))).count();
    assert!(ronds >= 4 * 6, "les poignées des coins de chaque case : {ronds}");
    assert!(b.app.status.contains("Maj tenue"), "{}", b.app.status);
    assert_eq!(crate::vignettes_tests::pas_d_annulation(&b), n);
    // Maj + clic au milieu d'une case, sans bouger : le plan est choisi.
    let shift = Modifiers::SHIFT;
    b.image(vec![crate::vignettes_tests::appui(v.center(), true, shift)], 1.0 / 60.0);
    b.image(vec![crate::vignettes_tests::appui(v.center(), false, shift)], 1.0 / 60.0);
    b.chauffer(2);
    assert!(!b.app.vignettes.libre && b.app.cur_panel == 1, "Maj + clic choisit le plan");
    assert_eq!(b.app.doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), ids, "rien n'a bougé");
    // Maj + glisser depuis le milieu de la case : le plan se déplace (devant le plan 4).
    let layout = b.app.board_layout();
    let slot = crate::board::drop_slots(&layout).into_iter().find(|s| s.before == Some(ids[3])).expect("un repère devant le plan 4");
    let cible = b.app.vignettes.xf.to_screen(eframe::egui::pos2(slot.x, (slot.top + slot.bottom) * 0.5));
    b.image(vec![crate::vignettes_tests::appui(v.center(), true, shift)], 1.0 / 60.0);
    for k in 1..=8 {
        b.image(vec![Event::PointerMoved(v.center() + (cible - v.center()) * (k as f32 / 8.0))], 1.0 / 60.0);
    }
    b.image(vec![crate::vignettes_tests::appui(cible, false, shift)], 1.0 / 60.0);
    b.chauffer(2);
    assert_eq!(b.app.doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), vec![ids[0], ids[2], ids[1], ids[3]], "Maj + glisser déplace le plan : {}", b.app.status);
    assert_eq!(crate::vignettes_tests::pas_d_annulation(&b), n + 1);
    // Maj + glisser un coin de la case : la caméra (un zoom), jamais le plan.
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    let v = crate::vignettes_tests::vignette(&b, 3);
    let coin = v.right_bottom() - eframe::egui::vec2(2.0, 2.0);
    b.image(vec![Event::PointerMoved(coin)], 1.0 / 60.0);
    b.image(vec![crate::vignettes_tests::appui(coin, true, shift)], 1.0 / 60.0);
    for k in 1..=8 {
        b.image(vec![Event::PointerMoved(coin + eframe::egui::vec2(30.0, 20.0) * (k as f32 / 8.0))], 1.0 / 60.0);
    }
    b.image(vec![crate::vignettes_tests::appui(coin + eframe::egui::vec2(30.0, 20.0), false, shift)], 1.0 / 60.0);
    b.chauffer(2);
    assert_eq!(b.app.doc.panels[1].camera.len(), 2, "un coin avec Maj : les clés A et B : {}", b.app.status);
    assert_eq!(b.app.doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), ids, "le plan n'a pas bougé");
    // Maj relâchée : plus de poignées, et Échap ne laisse rien.
    b.touches = Modifiers::NONE;
    b.image(vec![Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerMoved(v.center())], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    let apres = b.formes.iter().filter(|s| matches!(s.shape, Shape::Circle(_))).count();
    assert!(apres < ronds, "Maj relâchée : tout redevient du papier ({apres} ronds contre {ronds})");
}

#[test]
fn a_chosen_plan_wears_the_accent_border_and_a_joined_bar_is_a_visible_dot() {
    let mut b = dans_les_vignettes();
    b.garder_formes = true;
    b.chauffer(2);
    let accent = crate::theme::accent();
    let liseres = |b: &Banc| b.formes.iter().filter(|s| matches!(&s.shape, Shape::Rect(r) if r.stroke.color == accent && (r.stroke.width - 1.5).abs() < 0.01)).count();
    assert_eq!(liseres(&b), 0, "aucun plan choisi : aucun liseré");
    // Le point gris entre deux cases d'un même plan (proposition B), au centre de la barre.
    let barre = crate::vignettes_tests::barre(&b, 0).center();
    assert!(b.formes.iter().any(|s| matches!(&s.shape, Shape::Circle(c) if c.fill == crate::boardui::BAR_POINT && c.center.distance(barre) < 6.0)), "un point visible dans l'écart");
    let v = crate::vignettes_tests::vignette(&b, 3);
    clic_avec(&mut b, v.center(), Modifiers::SHIFT);
    b.chauffer(2);
    assert_eq!(liseres(&b), 1, "le plan choisi seul porte le liseré d'accent (décision 10)");
}

#[test]
fn action_and_dialogue_are_written_in_place_like_the_notes() {
    use eframe::egui::Event;
    let mut b = dans_les_vignettes();
    b.app.layout.board_textes = crate::boardsens::Textes { action: true, dialogue: true, notes: true };
    b.chauffer(3);
    let layout = b.app.board_layout();
    let place = layout.cards[3];
    let ligne = b.app.vignettes.xf.rect(b.app.ligne_du_champ(&place, crate::boardsens::Champ::Dialogue).expect("la ligne Dialogue"));
    let n = crate::vignettes_tests::pas_d_annulation(&b);
    clic_avec(&mut b, ligne.center(), Modifiers::NONE);
    assert!(b.app.vignettes.note.as_ref().is_some_and(|e| e.champ == crate::boardsens::Champ::Dialogue), "un clic sur la ligne Dialogue l'ouvre");
    b.image(vec![Event::Text("LÉA : Tu viens ?".into())], 1.0 / 60.0);
    b.image(vec![Event::Key { key: eframe::egui::Key::Enter, physical_key: Some(eframe::egui::Key::Enter), pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(2);
    assert_eq!(b.app.doc.panels[1].dialogue, "LÉA : Tu viens ?");
    assert_eq!(crate::vignettes_tests::pas_d_annulation(&b), n + 1, "une étape");
    assert_eq!(crate::vignettes_tests::dernier_nom(&b), "Dialogue de vignette");
    // La ligne Action de même ; le dialogue ne bouge pas.
    let ligne = b.app.vignettes.xf.rect(b.app.ligne_du_champ(&place, crate::boardsens::Champ::Action).expect("la ligne Action"));
    clic_avec(&mut b, ligne.center(), Modifiers::NONE);
    assert!(b.app.vignettes.note.as_ref().is_some_and(|e| e.champ == crate::boardsens::Champ::Action));
    b.image(vec![Event::Text("Léa sort.".into())], 1.0 / 60.0);
    b.image(vec![Event::Key { key: eframe::egui::Key::Enter, physical_key: Some(eframe::egui::Key::Enter), pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(2);
    assert_eq!(b.app.doc.panels[1].action, "Léa sort.");
    assert_eq!(b.app.doc.panels[1].dialogue, "LÉA : Tu viens ?", "le dialogue n'a pas bougé");
}

/// Décision 11, en pixels : un trait qui part sur la barre entre deux cases d'un même plan est un trait, continu, et ne coupe rien ; un clic
/// sur la barre, lui, coupe toujours, sans laisser de point de trait.
#[test]
fn a_stroke_starting_on_the_bar_draws_without_cutting_and_a_click_on_the_bar_still_cuts() {
    use crate::traits_continus_tests::{planche, relever, repos, tracer};
    let Some(mut b) = planche(quatre_plans(), 4, 1.0) else { return };
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(4);
    b.app.set_tool(Tool::Brush);
    b.app.color = [200, 30, 40];
    b.app.brush.size = 14.0;
    let plans = b.app.doc.panels.len();
    let n = crate::vignettes_tests::pas_d_annulation(&b);
    // La barre entre les cases 0 et 1 (même plan) : de son centre, un trait vers le milieu de la case 1.
    let de = crate::vignettes_tests::barre(&b, 0).center();
    let a = crate::vignettes_tests::vignette(&b, 1).center();
    tracer(&mut b, de, a);
    repos(&mut b);
    assert_eq!(b.app.doc.panels.len(), plans, "un trait parti de la barre ne coupe pas le plan : {}", b.app.status);
    assert_eq!(b.app.doc.panels[0].marks.len(), 2, "les deux marques d'image du plan 1 sont intactes");
    assert_eq!(crate::vignettes_tests::pas_d_annulation(&b), n + 1, "une seule étape : le trait");
    assert!(crate::vignettes_tests::encre(&b.app.doc, 0), "le trait est dans le plan 1");
    let rgba = b.capturer().expect("capture");
    let r = relever(&rgba, de + (a - de).normalized() * 12.0, a);
    assert!(r.trou <= 3, "le trait est continu depuis la barre : {r:?}");
    // Un clic sur la barre (sans bouger) : la coupe, comme toujours, et aucun point de trait en plus.
    let n = crate::vignettes_tests::pas_d_annulation(&b);
    clic_avec(&mut b, de, Modifiers::NONE);
    b.chauffer(3);
    assert_eq!(b.app.doc.panels.len(), plans + 1, "le clic coupe : {}", b.app.status);
    assert_eq!(crate::vignettes_tests::pas_d_annulation(&b), n + 1, "une seule étape : la coupe, pas de point de trait");
    assert_eq!(crate::vignettes_tests::dernier_nom(&b), "Couper le plan entre deux vignettes");
}

#[test]
fn frame_all_puts_the_whole_board_in_the_view_in_both_directions() {
    let mut doc = quatre_plans();
    for _ in 0..80 {
        let last = doc.panels.len() - 1;
        doc.add_scene_after(last);
    }
    let mut b = ouvert(doc, 6);
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(3);
    let derniere = b.app.board_layout().cards.len() - 1;
    assert!(!b.app.vignettes.view.contains_rect(crate::vignettes_tests::vignette(&b, derniere)), "à la largeur de la vue, la dernière case est hors de la vue");
    b.app.planche_cadrer_tout();
    b.chauffer(4);
    let (r0, r1) = (crate::vignettes_tests::vignette(&b, 0), crate::vignettes_tests::vignette(&b, derniere));
    let vue = b.app.vignettes.view;
    assert!(vue.contains_rect(r0) && vue.contains_rect(r1), "« Cadrer tout » : la première et la dernière case sont dans la vue ({r0:?} {r1:?} dans {vue:?})");
    assert!(b.app.board_zoom >= 0.2);
    // La page à la largeur de la vue, elle, est inchangée.
    b.app.board_fit();
    b.chauffer(3);
    assert!(!vue.contains_rect(crate::vignettes_tests::vignette(&b, derniere)));
}

// ------------------------------------------------------------------------------ captures (à regarder)

/// Les écrans du travail en Vignettes, en PNG, pour juger la lisibilité (hors de la suite) :
/// `FEANOR_CAPTURE_DIR=… cargo test --release --offline --target-dir target\msvc vignettes_simplicite_captures -- --ignored --nocapture`
#[test]
#[ignore]
fn vignettes_simplicite_captures() {
    use eframe::egui::vec2;
    let dir = std::env::var_os("FEANOR_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let sauver = |b: &Banc, nom: &str| {
        let Some(rgba) = b.capturer() else { return println!("CAPTURE pas de carte graphique : {nom} ignorée") };
        image::save_buffer(dir.join(nom), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
        println!("CAPTURE {nom}");
    };
    let ouvrir = |doc: crate::document::Document| -> Banc {
        let mut b = crate::perf_tests::banc(doc, true);
        b.app.layout.board_textes = crate::boardsens::Textes::aucun();
        b.app.set_espace(Espace::Vignettes);
        b.app.board_zoom = 0.0;
        b.app.vignettes.ouverture_a_faire = true;
        b.chauffer(16);
        b.attendre_vignettes();
        b
    };
    // 1. Première ouverture : quatre plans, cadrage 6 × 2.
    let mut b = ouvrir(quatre_plans());
    sauver(&b, "01-vignettes-ouverture.png");
    // 2. Un trait qui part du papier, traverse une case et entre dans une case vide.
    let v = crate::vignettes_tests::vignette(&b, 4);
    let de = v.left_bottom() + vec2(-60.0, 40.0);
    let fin = v.right_top() + vec2(v.width() * 1.4, -20.0);
    let chemin = crate::vignettes_tests::vague(de, fin, 40);
    crate::vignettes_tests::glisser(&mut b, &chemin);
    b.chauffer(10);
    b.attendre_vignettes();
    sauver(&b, "02-vignettes-trait-a-travers.png");
    // 3. Maj tenue : la peau de la planche ; puis Maj + clic : un plan choisi, et « Plan suivant » par sa commande.
    let centre = crate::vignettes_tests::vignette(&b, 3).center();
    b.touches = Modifiers::SHIFT;
    b.image(vec![eframe::egui::Event::PointerMoved(centre)], 1.0 / 60.0);
    b.chauffer(2);
    sauver(&b, "03a-vignettes-maj-tenue.png");
    b.touches = Modifiers::NONE;
    clic_avec(&mut b, centre, Modifiers::SHIFT);
    b.chauffer(3);
    sauver(&b, "03-vignettes-plan-choisi.png");
    b.app.run_action("panneau.suivant");
    b.chauffer(3);
    sauver(&b, "04-vignettes-plan-suivant.png");
    // 4. Un outil refusé : la ligne d'état.
    b.app.run_action("outil.remplissage");
    b.chauffer(3);
    sauver(&b, "05-vignettes-outil-refuse.png");
    b.app.run_action("outil.pinceau");
    // 5. L'animation entrée depuis les Vignettes, et son bouton de retour ; puis le Storyboard sur le même plan.
    b.app.enter_animation(2);
    b.chauffer(8);
    sauver(&b, "06-animation-depuis-vignettes.png");
    b.app.run_action("mode.basculer");
    b.chauffer(6);
    sauver(&b, "07-vignettes-retour.png");
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(8);
    b.attendre_vignettes();
    sauver(&b, "08-storyboard-meme-plan.png");
    // 6. Un plan à mouvement de caméra, et l'outil Caméra.
    let mut doc = crate::vignettes_camera_tests::plan_court();
    crate::vignettes_camera_tests::deux_cadres(&mut doc, 1, 160.0, 0.0);
    let mut c = ouvrir(doc);
    sauver(&c, "09-vignettes-mouvement-camera.png");
    c.app.set_tool(Tool::Camera);
    let v = crate::vignettes_tests::vignette(&c, crate::vignettes_camera_tests::K);
    c.image(vec![eframe::egui::Event::PointerMoved(v.center())], 1.0 / 60.0);
    c.chauffer(2);
    sauver(&c, "10-vignettes-outil-camera.png");
    // 7. Deux séquences, les textes cochés.
    let mut d = ouvrir(crate::vignettes_tests::deux_sequences());
    d.app.layout.board_textes = crate::boardsens::Textes { action: true, dialogue: true, notes: true };
    d.app.doc.panels[1].dialogue = "LÉA : Tu viens ?".into();
    d.app.doc.panels[1].action = "Léa sort de la cuisine, un sac à la main.".into();
    d.chauffer(6);
    sauver(&d, "11-vignettes-sequences-textes.png");
    // 8. Un projet long : 300 plans, à l'ouverture, puis « Cadrer tout ».
    let mut doc = quatre_plans();
    for _ in 0..296 {
        let last = doc.panels.len() - 1;
        doc.add_scene_after(last);
    }
    let mut e = ouvrir(doc);
    sauver(&e, "12-vignettes-300-plans-ouverture.png");
    e.app.planche_cadrer_tout();
    e.chauffer(8);
    e.attendre_vignettes();
    sauver(&e, "13-vignettes-300-plans-cadrer-tout.png");
    // 9. Préférences > Vignettes, repliées puis dépliées.
    let mut f = ouvrir(quatre_plans());
    f.app.settings.prefs_onglet = crate::prefsonglets::Onglet::Vignettes;
    f.app.run_action("fichier.preferences");
    f.chauffer(8);
    sauver(&f, "14-preferences-vignettes.png");
    f.app.prefs_plus_vignettes = true;
    f.chauffer(6);
    sauver(&f, "15-preferences-vignettes-plus.png");
    // 10. L'outil Caméra des Vignettes : ses Réglages courts.
    f.app.prefs_window = false;
    f.app.set_tool(Tool::Camera);
    f.chauffer(6);
    sauver(&f, "16-vignettes-camera-reglages.png");
}

// ------------------------------------------------------------------------------ ce qui ne fait rien le dit

#[test]
fn the_vignettes_refuse_the_other_tools_keep_the_ctrl_lasso_and_the_fill_tool_on_the_paper_says_where_it_acts() {
    let mut b = dans_les_vignettes();
    // Décision 2 : une touche d'un autre outil ne change rien, et la ligne d'état le dit ; les formes aussi sont refusées.
    for action in ["outil.remplissage", "outil.selection", "outil.texte", "outil.formes", "outil.rectangle", "outil.pipette", "outil.transformer", "deformer.enveloppe"] {
        b.app.status.clear();
        b.app.run_action(action);
        assert_eq!(b.app.tool, Tool::Brush, "{action} : l'outil reste le pinceau");
        assert!(b.app.status.contains("pinceau, crayon, gomme et caméra"), "{action} : {}", b.app.status);
    }
    // Les outils des Vignettes, eux, passent ; la main aussi.
    for (action, outil) in [("outil.crayon", Tool::Pencil), ("outil.gomme", Tool::Eraser), ("outil.camera", Tool::Camera), ("outil.main", Tool::Pan), ("outil.pinceau", Tool::Brush)] {
        b.app.run_action(action);
        assert_eq!(b.app.tool, outil, "{action}");
    }
    // Le lasso de Ctrl n'est pas un outil qu'on prend : il reste.
    b.app.prendre_le_lasso();
    assert_eq!(b.app.tool, Tool::Select, "Ctrl + glisser : le lasso, le temps du geste");
    b.app.set_tool(Tool::Brush);
    // Arriver dans les Vignettes avec un autre outil en main : le pinceau le remplace.
    b.app.set_espace(Espace::Storyboard);
    b.app.set_tool(Tool::Fill);
    b.app.set_espace(Espace::Vignettes);
    assert_eq!(b.app.tool, Tool::Brush, "{}", b.app.status);
    b.chauffer(2);
    // Le pot de peinture sur le papier, hors de toute case, ou dans une case pâle : rien ne se passe, et la ligne d'état dit où il agit.
    b.app.set_tool(Tool::Fill);
    let layout = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let papier = xf.to_screen(eframe::egui::pos2(layout.cards[0].thumb.right() + 2.0, layout.cards[0].thumb.top() - 2.0));
    assert!(matches!(crate::board::hit(&layout, xf.to_board(papier), 0.0), crate::board::Hit::Nothing), "le point est sur le papier");
    let (plans, n) = (b.app.doc.panels.len(), crate::vignettes_tests::pas_d_annulation(&b));
    b.app.status.clear();
    clic_avec(&mut b, papier, Modifiers::NONE);
    assert!(b.app.status.to_lowercase().contains("case"), "le pot sur le papier : {}", b.app.status);
    b.app.status.clear();
    let vide = crate::vignettes_tests::case_vide(&b, 0).center();
    clic_avec(&mut b, vide, Modifiers::NONE);
    assert!(b.app.status.to_lowercase().contains("dessine d'abord"), "le pot dans une case pâle : {}", b.app.status);
    assert_eq!((b.app.doc.panels.len(), crate::vignettes_tests::pas_d_annulation(&b)), (plans, n), "rien n'a changé");
}
