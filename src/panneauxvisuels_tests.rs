//! Partie 163 : les panneaux visuels. Chaque dessin, manipulé dans la vraie fenêtre sans écran (`Banc`), change le projet par le
//! même chemin que le champ qu'il remplace ; chaque ancien réglage reste joignable (dessin, « Précis », clic droit).

use eframe::egui::{pos2, vec2, Modifiers, Pos2, Rect};

use crate::boardsens::{Champ, Sens};
use crate::bulles::vus;
use crate::camera::{Camera, CameraKey};
use crate::dock::Pane;
use crate::export::Format;
use crate::panneauxvisuels::{self as pv, Courbe, Famille, Mouvement};
use crate::perf_tests::{banc, Banc};
use crate::pistes_tests::{clic, etapes, glisser};
use crate::workspace::Espace;

fn banc_163(espace: Espace) -> Banc {
    let mut doc = crate::pistes_tests::doc_with(&[&["Dessin"], &["Seul"]]);
    doc.panels[0].set_length(24);
    doc.panels[1].set_length(24);
    let mut b = banc(doc, false);
    b.app.set_espace(espace);
    b.chauffer(6);
    b
}

/// Le rectangle de l'élément dont la bulle s'appelle `nom`, à la dernière image (le plus à droite : la colonne des panneaux).
fn ou(nom: &str) -> Rect {
    vus::ou(nom).into_iter().max_by(|a, b| a.center().x.total_cmp(&b.center().x)).unwrap_or_else(|| panic!("« {nom} » n'est pas à l'écran : {:?}", vus::noms()))
}

fn present(nom: &str) -> bool {
    !vus::ou(nom).is_empty()
}

fn derniere_etape(b: &Banc) -> String {
    b.app.history.names().0.last().map(|s| s.to_string()).unwrap_or_default()
}

// ------------------------------------------------------------------------------------ les calculs

#[test]
fn zoom_in_puts_b_inside_a_and_zoom_out_around_it() {
    let frame = [1920, 1080];
    let a = Camera::centered(1920, 1080);
    let dedans = |q: &[Pos2; 4], r: Rect| q.iter().all(|p| r.expand(0.5).contains(*p));
    let boite = |q: [Pos2; 4]| crate::vignettecam::boite(&q);
    let b = pv::cadre_b(Mouvement::ZoomAvant, frame, a);
    assert!(dedans(&b.corners(frame), boite(a.corners(frame))), "B plus petit, dans A");
    assert!((b.zoom - a.zoom * pv::ZOOM).abs() < 1e-4 && b.x == a.x && b.y == a.y, "centré dans A");
    let b = pv::cadre_b(Mouvement::ZoomArriere, frame, a);
    assert!(dedans(&a.corners(frame), boite(b.corners(frame))), "B plus grand, autour de A");
    assert_eq!(pv::mouvements_de(frame, &a, &pv::cadre_b(Mouvement::ZoomAvant, frame, a)), vec![Mouvement::ZoomAvant]);
    assert_eq!(pv::mouvements_de(frame, &a, &pv::cadre_b(Mouvement::ZoomArriere, frame, a)), vec![Mouvement::ZoomArriere]);
    assert_eq!(pv::mouvements_de(frame, &a, &pv::cadre_b(Mouvement::Travelling, frame, a)), vec![Mouvement::Travelling]);
    assert_eq!(pv::mouvements_de(frame, &a, &pv::cadre_b(Mouvement::Rotation, frame, a)), vec![Mouvement::Rotation]);
    assert_eq!(pv::mouvements_de(frame, &a, &pv::cadre_b(Mouvement::Pano, frame, a)), vec![Mouvement::Pano]);
    assert!(pv::mouvements_de(frame, &a, &a).is_empty(), "rien ne bouge : aucun allumé");
}

#[test]
fn each_curve_is_read_back_from_the_keys_it_writes() {
    let cam = Camera::centered(100, 100);
    let keys = vec![CameraKey::new(0, cam), CameraKey::new(10, cam), CameraKey::new(30, cam)];
    assert_eq!(pv::courbe_des(&keys, 0..3), Some(Courbe::Constant), "des clés posées par le geste : vitesse constante");
    for c in Courbe::TOUTES {
        let k = pv::avec_la_courbe(&keys, 0..3, c);
        assert_eq!(pv::courbe_des(&k, 0..3), Some(c), "{c:?}");
        // Les vraies valeurs, en images, sur chaque intervalle : celles que montrent les poignées d'adoucis.
        assert_eq!((k[0].ease_out, k[1].ease_in), c.adoucis(10.0));
        assert_eq!((k[1].ease_out, k[2].ease_in), c.adoucis(20.0));
        assert_eq!(k[0].ease_in, 0.0, "l'arrivée sur A (le mouvement d'avant) ne change pas");
    }
    let mut melange = pv::avec_la_courbe(&keys, 0..2, Courbe::Doux);
    melange = pv::avec_la_courbe(&melange, 1..3, Courbe::Brusque);
    assert_eq!(pv::courbe_des(&melange, 0..3), None, "deux courbes différentes : aucune allumée");
    // La courbe dessinée est la vraie : Doux part lentement, Brusque d'un coup.
    assert!(crate::camera::ease(0.1, 1.0, 0.5, 0.5) < 0.1 && crate::camera::ease(0.1, 1.0, 0.0, 1.0) > 0.1);
}

#[test]
fn the_duration_bar_scale_and_text() {
    assert_eq!(pv::texte_duree(44, 24), "1 s 20 i · 44 images");
    assert_eq!(pv::texte_duree(10, 24), "10 i · 10 images");
    let e = pv::etendue_barre(44, 24);
    assert!(e >= 66 && e % 24 == 0, "{e}");
    assert_eq!(pv::duree_a(0.0, 0.0, 100.0, 48), 1, "jamais moins d'une image");
    assert_eq!(pv::duree_a(50.0, 0.0, 100.0, 48), 24);
    assert_eq!(pv::cases_a(10.0 + 2.0 * 30.0, 10.0, 30.0, 6), 2);
    assert_eq!(pv::cases_a(1000.0, 10.0, 30.0, 6), 6);
    assert_eq!(pv::cases_a(0.0, 10.0, 30.0, 6), 1);
}

// ------------------------------------------------------------------------------------ 163.1 : la Fiche du plan

fn fiche(b: &mut Banc) {
    b.app.run_action("vue.fiche");
    b.chauffer(3);
    assert_eq!(b.app.etat_dans_la_barre(Pane::Sheet), crate::barrepanneaux::EtatIcone::Devant);
}

/// Où est la poignée de la barre de durée, et la durée que donne le point `x`.
fn barre(b: &Banc) -> (Rect, f32, impl Fn(f32) -> usize) {
    let r = ou(crate::handles::BULLE_DUREE_NOM);
    let (g, d) = (r.left() + 5.0, r.right() - 7.0);
    let etendue = pv::etendue_barre(b.app.frame_count(), b.app.doc.fps);
    let x = g + (d - g) * b.app.frame_count() as f32 / etendue as f32;
    (r, x, move |px: f32| pv::duree_a(px, g, d, etendue))
}

#[test]
fn pulling_the_duration_bar_changes_the_duration_in_one_step_and_ctrl_moves_the_keys() {
    let mut b = banc_163(Espace::Storyboard);
    let cam = Camera::centered(b.app.doc.frame[0], b.app.doc.frame[1]);
    b.app.doc.panels[0].camera = vec![CameraKey::new(0, cam), CameraKey::new(12, cam.shifted(40.0, 0.0))];
    fiche(&mut b);
    let avant = etapes(&b);
    let (r, x, duree) = barre(&b);
    let y = r.top() + 11.0;
    glisser(&mut b, pos2(x, y), pos2(x + 50.0, y));
    let attendu = duree(x + 50.0);
    assert!(attendu > 24);
    assert_eq!(b.app.doc.panels[0].frame_count(), attendu, "la barre a allongé le plan");
    assert_eq!(etapes(&b), avant + 1, "une seule étape pour tout le geste");
    assert_eq!(derniere_etape(&b), crate::rognage::noms::FIN_LONGUE);
    assert_eq!(b.app.doc.panels[0].camera[1].frame, 12, "sans Ctrl : les clés restent où elles sont");
    b.app.run_action("edition.annuler");
    b.chauffer(3);
    assert_eq!(b.app.doc.panels[0].frame_count(), 24, "Ctrl+Z rend la durée");
    // Ctrl tenue : toutes les clés suivent la poignée du même nombre d'images.
    let (r, x, duree) = barre(&b);
    let y = r.top() + 11.0;
    b.touches = Modifiers::CTRL;
    glisser(&mut b, pos2(x, y), pos2(x + 50.0, y));
    b.touches = Modifiers::NONE;
    let n = duree(x + 50.0);
    assert_eq!(b.app.doc.panels[0].frame_count(), n);
    assert_eq!(b.app.doc.panels[0].camera[1].frame, 12 + (n - 24), "Ctrl : la clé a suivi");
    assert_eq!(derniere_etape(&b), crate::rognage::noms::DUREE_CLES_LONGUE);
    // Raccourcir aussi, vers la gauche.
    let (r, x, duree) = barre(&b);
    glisser(&mut b, pos2(x, r.top() + 11.0), pos2(x - 80.0, r.top() + 11.0));
    assert_eq!(b.app.doc.panels[0].frame_count(), duree(x - 80.0));
    // Comme la poignée de la timeline, la barre ne se prend qu'à sa poignée : un appui loin d'elle ne change rien.
    let (n, e) = (b.app.doc.panels[0].frame_count(), etapes(&b));
    let (r, x, _) = barre(&b);
    let loin = if x - r.left() > 60.0 { r.left() + 10.0 } else { r.right() - 10.0 };
    glisser(&mut b, pos2(loin, r.top() + 11.0), pos2(loin + 30.0, r.top() + 11.0));
    assert_eq!((b.app.doc.panels[0].frame_count(), etapes(&b)), (n, e));
    assert!(b.app.handle_drag.is_none(), "aucun geste laissé ouvert");
}

#[test]
fn a_pictogram_at_the_end_of_what_the_camera_can_do_changes_nothing() {
    let mut b = vignettes();
    let cam = Camera { pan: 85.0, ..Camera::centered(b.app.doc.frame[0], b.app.doc.frame[1]) }.limited(b.app.doc.frame);
    b.app.doc.panels[0].camera = vec![CameraKey::new(0, cam)];
    b.chauffer(2);
    let (avant, plans) = (etapes(&b), b.app.doc.panels.len());
    clic(&mut b, ou("Mouvement : Pano").center(), Modifiers::NONE);
    assert_eq!((etapes(&b), b.app.doc.panels[0].camera.len(), b.app.doc.panels.len()), (avant, 1, plans), "ni étape, ni clé, ni coupe");
    assert!(b.app.status.contains("plus loin"), "{}", b.app.status);
}

#[test]
fn a_version_thumbnail_switches_version_and_the_old_buttons_stay_in_precis() {
    let mut b = banc_163(Espace::Storyboard);
    let plan = b.app.doc.panels[0].id;
    b.app.nouvelle_version_du_plan(0);
    fiche(&mut b);
    assert_eq!(b.app.doc.numero_de_version(plan), 2);
    assert!(present("Version V2 (active)") && present("Nouvelle version, en dupliquant"));
    let avant = etapes(&b);
    clic(&mut b, ou("Version V1").center(), Modifiers::NONE);
    assert_eq!(b.app.doc.numero_de_version(plan), 1, "le clic passe à la V1");
    assert_eq!(etapes(&b), avant + 1);
    // « + nouvelle » : une version de plus.
    clic(&mut b, ou("Nouvelle version, en dupliquant").center(), Modifiers::NONE);
    assert_eq!(b.app.doc.nombre_de_versions(plan), 3);
    // Sous « Précis » : la ligne de version d'avant (passer, comparer, restaurer, supprimer) et la durée en chiffres.
    assert!(!present("Passer à cette version"), "replié d'origine");
    crate::panneauxvisuels::ouvrir_precis(&b.ctx, "Fiche du plan", true);
    b.chauffer(3);
    for nom in ["Passer à cette version", "Comparer à la version d'aujourd'hui", "Restaurer cette version", "Supprimer cette version", "Nouvelle version, plan vide"] {
        assert!(present(nom), "« {nom} » sous Précis : {:?}", vus::noms());
    }
}

// ------------------------------------------------------------------------------------ 163.2 : le mouvement de la case

fn vignettes() -> Banc {
    let mut b = banc_163(Espace::Vignettes);
    b.app.show_pane(Pane::Vignettes);
    b.app.cur_panel = 0;
    b.app.cur_frame = 0;
    b.chauffer(4);
    b
}

#[test]
fn the_zoom_in_pictogram_puts_b_inside_a_and_the_curves_set_the_easing() {
    let mut b = vignettes();
    let frame = b.app.doc.frame;
    assert!(b.app.doc.panels[0].camera.len() < 2, "pas de mouvement");
    let avant = etapes(&b);
    clic(&mut b, ou("Mouvement : Zoom avant").center(), Modifiers::NONE);
    let keys = b.app.doc.panels[0].camera.clone();
    assert_eq!(keys.len(), 2, "A et B");
    assert_eq!((keys[0].frame, keys[1].frame), (0, 23), "sur la durée de la case");
    let a = crate::vignettecam::boite(&keys[0].cam.corners(frame));
    assert!(keys[1].cam.corners(frame).iter().all(|p| a.expand(0.5).contains(*p)), "B dans A");
    assert_eq!(pv::mouvements_de(frame, &keys[0].cam, &keys[1].cam), vec![Mouvement::ZoomAvant], "le pictogramme allumé");
    assert_eq!(etapes(&b), avant + 1);
    assert_eq!(derniere_etape(&b), "Mouvement de caméra : Zoom avant");
    // Un autre pictogramme : seul B change.
    clic(&mut b, ou("Mouvement : Travelling").center(), Modifiers::NONE);
    let k2 = b.app.doc.panels[0].camera.clone();
    assert_eq!(k2[0].cam, keys[0].cam, "A ne bouge pas");
    assert_eq!(pv::mouvements_de(frame, &k2[0].cam, &k2[1].cam), vec![Mouvement::Travelling]);
    // Les courbes : les adoucis des clés.
    for c in [Courbe::Doux, Courbe::Brusque, Courbe::Constant] {
        let n = etapes(&b);
        clic(&mut b, ou(&format!("Démarrage : {}", c.nom())).center(), Modifiers::NONE);
        let k = &b.app.doc.panels[0].camera;
        assert_eq!(pv::courbe_des(k, 0..2), Some(c), "{c:?}");
        assert_eq!((k[0].ease_out, k[1].ease_in), c.adoucis(23.0));
        assert_eq!(etapes(&b), n + 1, "{c:?} : une étape");
    }
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(pv::courbe_des(&b.app.doc.panels[0].camera, 0..2), Some(Courbe::Brusque), "Ctrl+Z rend la courbe d'avant");
    // « Précis » : les adoucis en images, « Ajouter un cadre ».
    crate::panneauxvisuels::ouvrir_precis(&b.ctx, "Mouvement de caméra", true);
    b.chauffer(3);
    for nom in ["Adouci de départ", "Adouci d'arrivée", "Ajouter un cadre"] {
        assert!(present(nom), "« {nom} » sous Précis");
    }
    clic(&mut b, ou("Ajouter un cadre").center(), Modifiers::NONE);
    assert_eq!(b.app.doc.panels[0].camera.len(), 3, "un cadre C");
}

// ------------------------------------------------------------------------------------ 163.3 : la planche

#[test]
fn the_board_drawings_set_reading_order_cases_per_row_and_texts() {
    let mut b = vignettes();
    // Le sens de lecture, par les deux pages.
    clic(&mut b, ou(Sens::Japonais.nom()).center(), Modifiers::NONE);
    assert_eq!(b.app.layout.board_sens, Sens::Japonais);
    clic(&mut b, ou(Sens::Lignes.nom()).center(), Modifiers::NONE);
    assert_eq!(b.app.layout.board_sens, Sens::Lignes);
    // La mini-planche : on clique, on tire son bord.
    let r = ou("Cases par ligne");
    let (debut, pas) = (r.left() + 8.0, (r.width() - 26.0) / crate::board::MAX_COLUMNS as f32);
    clic(&mut b, pos2(debut + 2.0 * pas, r.center().y), Modifiers::NONE);
    assert_eq!(b.app.layout.board_columns, 2);
    glisser(&mut b, pos2(debut + 2.0 * pas, r.center().y), pos2(debut + 4.0 * pas + 3.0, r.center().y));
    assert_eq!(b.app.layout.board_columns, 4, "le bord tiré");
    // À la japonaise, la mini-planche se tire vers le bas.
    clic(&mut b, ou(Sens::Japonais.nom()).center(), Modifiers::NONE);
    let r = ou("Cases par colonne");
    let (debut, pas) = (r.top() + 6.0, (r.height() - 26.0) / crate::board::MAX_COLUMNS as f32);
    clic(&mut b, pos2(r.center().x, debut + 3.0 * pas), Modifiers::NONE);
    assert_eq!(b.app.layout.board_columns, 3);
    clic(&mut b, ou(Sens::Lignes.nom()).center(), Modifiers::NONE);
    // Les textes sous les cases.
    for champ in Champ::TOUS {
        let avant = b.app.layout.board_textes.montre(champ);
        clic(&mut b, ou(champ.nom()).center(), Modifiers::NONE);
        assert_eq!(b.app.layout.board_textes.montre(champ), !avant, "{champ:?}");
    }
    // « Précis » : les séquences montrées, les numéros de plan, les cases en chiffres.
    assert!(!present("Toutes") && !present("Numéros de plan"), "repliés d'origine, le moins d'infos possible");
    crate::panneauxvisuels::ouvrir_precis(&b.ctx, "Planche", true);
    b.chauffer(3);
    for nom in ["Nouvelle séquence", "Toutes", "Seulement la courante", "Aucune autre", "Numéros de plan", "Cases par ligne (en chiffres)"] {
        assert!(present(nom), "« {nom} » sous Précis");
    }
    let n = b.app.settings.vignettes.numeros_visibles;
    clic(&mut b, ou("Numéros de plan").center(), Modifiers::NONE);
    assert_eq!(b.app.settings.vignettes.numeros_visibles, !n);
}

// ------------------------------------------------------------------------------------ 163.4 : les exports

#[test]
fn an_export_thumbnail_chooses_the_family_and_the_settings_stay_in_precis() {
    let mut b = banc_163(Espace::Storyboard);
    b.app.open_export(Format::Mp4);
    b.chauffer(3);
    for f in Famille::TOUTES {
        assert!(present(&format!("Export : {}", f.nom())), "{f:?}");
    }
    assert!(present("Exporter"), "le gros bouton");
    clic(&mut b, ou("Export : Storyboard PDF").center(), Modifiers::NONE);
    assert_eq!(b.app.export_dialog.as_ref().map(|s| s.format), Some(Format::Pdf));
    clic(&mut b, ou("Export : Images une à une").center(), Modifiers::NONE);
    assert_eq!(b.app.export_dialog.as_ref().map(|s| s.format), Some(Format::Png));
    clic(&mut b, ou("Export : Film vidéo").center(), Modifiers::NONE);
    assert_eq!(b.app.export_dialog.as_ref().map(|s| s.format), Some(Format::Mp4));
    // Les réglages d'avant, sous « Précis ».
    assert!(!present("Graver les informations"), "replié d'origine");
    crate::panneauxvisuels::ouvrir_precis(&b.ctx, "Exporter", true);
    b.chauffer(3);
    assert!(present("Graver les informations"), "les réglages du film sous Précis");
    // « Pour le montage » : sa fenêtre à elle, à la place de celle-ci.
    clic(&mut b, ou("Export : Pour le montage").center(), Modifiers::NONE);
    assert!(b.app.export_dialog.is_none() && b.app.editorial.is_some(), "une seule fenêtre à la fois");
    // Le son garde sa fenêtre telle quelle (pas de vignettes).
    b.app.editorial = None;
    b.app.open_export(Format::Wav);
    b.chauffer(3);
    assert!(!present("Export : Film vidéo"));
}

#[test]
fn the_painted_widgets_never_say_3d() {
    for t in Mouvement::TOUS.iter().flat_map(|m| [m.nom(), m.description()]).chain(Courbe::TOUTES.iter().flat_map(|c| [c.nom(), c.description()])).chain(Famille::TOUTES.iter().flat_map(|f| [f.nom(), f.description()])) {
        assert!(!crate::espace3ddirect::dit_la_3d(t), "« {t} »");
    }
    let _ = vec2(0.0, 0.0);
}
