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
    // 3. Maj + clic : un plan choisi, et « Plan suivant » par sa commande.
    let centre = crate::vignettes_tests::vignette(&b, 3).center();
    clic_avec(&mut b, centre, Modifiers::SHIFT);
    b.chauffer(3);
    sauver(&b, "03-vignettes-plan-choisi.png");
    b.app.run_action("panneau.suivant");
    b.chauffer(3);
    sauver(&b, "04-vignettes-plan-suivant.png");
    // 4. Un outil hors de la colonne : la ligne d'état.
    b.app.run_action("outil.remplissage");
    b.chauffer(3);
    sauver(&b, "05-vignettes-outil-hors-colonne.png");
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
    // 9. Préférences > Vignettes.
    let mut f = ouvrir(quatre_plans());
    f.app.settings.prefs_onglet = crate::prefsonglets::Onglet::Vignettes;
    f.app.run_action("fichier.preferences");
    f.chauffer(8);
    sauver(&f, "14-preferences-vignettes.png");
}

// ------------------------------------------------------------------------------ ce qui ne fait rien le dit

#[test]
fn a_tool_outside_the_vignettes_column_says_so_and_the_fill_tool_on_the_paper_says_where_it_acts() {
    let mut b = dans_les_vignettes();
    b.app.run_action("outil.remplissage");
    assert_eq!(b.app.tool, Tool::Fill, "l'outil est pris quand même (ses raccourcis restent)");
    assert!(b.app.status.contains(Tool::Fill.label()) && b.app.status.contains("colonne des Vignettes"), "la ligne d'état dit que l'outil n'est pas dans la colonne : {}", b.app.status);
    // (Ce que la barre d'état en montre, avant la bulle, se suffit.)
    let court = crate::messages::resume(&b.app.status);
    assert!(court.contains("agit dans les cases"), "{court}");
    // Les quatre outils de la colonne, eux, ne provoquent aucune remarque.
    b.app.status.clear();
    b.app.run_action("outil.pinceau");
    assert!(!b.app.status.contains("colonne"), "{}", b.app.status);
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
