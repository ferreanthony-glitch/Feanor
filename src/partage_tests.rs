//! La planche-mère (`partage.rs`, `partageui.rs`, partie 126) : partager un calque entre des plans, l'accorder, détacher, réutiliser un
//! plan, couper, réunir, dupliquer, enregistrer et relire ; et les commandes depuis l'application.

use crate::document::{Document, Layer, LayerKind};
use crate::partage::Cibles;
use crate::perf_tests::banc_taille;

/// Un film de `n` plans (une scène chacun) sur une feuille 64 × 48, un calque vectoriel par plan avec un dessin à l'image 0 (chaque
/// copie de scène a ses propres dessins).
fn film(n: usize) -> Document {
    let mut doc = Document::with_frame(64, 48, 12);
    doc.panels[0].layers = vec![Layer::new("Dessin", LayerKind::Vector)];
    let c = doc.new_cel(LayerKind::Vector);
    doc.panels[0].layers[0].frames = vec![Some(c)];
    for _ in 1..n {
        let last = doc.panels.len() - 1;
        doc.duplicate_scene_after(last);
    }
    doc
}

fn cle(doc: &Document, pi: usize, li: usize) -> Option<u64> {
    doc.panels[pi].layers[li].partage
}

fn dessin(doc: &Document, pi: usize, li: usize, f: usize) -> Option<u64> {
    doc.panels[pi].layers[li].cel_at(f)
}

#[test]
fn a_layer_reused_in_another_plan_shows_the_same_drawing_follows_the_model_and_keeps_its_own_pose() {
    let mut doc = film(2);
    let c0 = dessin(&doc, 0, 0, 0).unwrap();
    assert_ne!(dessin(&doc, 1, 0, 0), Some(c0), "deux plans, deux dessins au départ");
    assert_eq!(doc.cibles(0, Cibles::Suivant), [1]);
    assert_eq!(doc.cibles(1, Cibles::Precedent), [0]);
    assert!(doc.cibles(0, Cibles::Precedent).is_empty() && doc.cibles(1, Cibles::Suivant).is_empty());
    assert_eq!(doc.cibles(0, Cibles::Sequence), [1], "les autres plans de la séquence");
    let k = doc.reutiliser_calque(0, 0, &[1]).expect("un calque de dessin se partage");
    assert_eq!(cle(&doc, 0, 0), Some(k));
    assert_eq!(doc.panels[1].layers.len(), 2, "un exemplaire de plus dans le plan 2");
    assert_eq!(cle(&doc, 1, 0), Some(k), "au même rang");
    assert_eq!(dessin(&doc, 1, 0, 0), Some(c0), "le même dessin, pas une copie");
    assert_eq!(doc.plans_du_partage(0, 0), [0, 1]);
    assert_eq!(doc.exemplaires(k), [(0, 0), (1, 0)]);
    assert!(doc.plan_partage(0) && doc.plan_partage(1));
    // Réutiliser encore vers le même plan : rien de plus.
    doc.reutiliser_calque(0, 0, &[1]);
    assert_eq!(doc.panels[1].layers.len(), 2);
    // L'accord : le dessin du modèle (le plan courant) est tenu sur toute la durée de chaque plan ; puis plus rien ne bouge.
    let n1 = doc.panels[1].frame_count();
    assert!(doc.sync_partages(0));
    assert_eq!(doc.panels[1].layers[0].frames.len(), n1);
    assert!(doc.panels[1].layers[0].frames.iter().all(|f| *f == Some(c0)));
    assert!(!doc.sync_partages(0), "stable");
    // Le modèle change de dessin et de nom : l'exemplaire suit.
    let c1 = doc.new_cel(LayerKind::Vector);
    doc.panels[0].layers[0].frames[0] = Some(c1);
    doc.panels[0].layers[0].name = "Planche".into();
    assert!(doc.sync_partages(0));
    assert_eq!(dessin(&doc, 1, 0, 0), Some(c1));
    assert_eq!(doc.panels[1].layers[0].name, "Planche");
    // La pose reste à chaque plan.
    doc.panels[1].layers[0].transform.x = 40.0;
    doc.sync_partages(0);
    assert_eq!(doc.panels[1].layers[0].transform.x, 40.0);
    assert_eq!(doc.panels[0].layers[0].transform.x, 0.0);
    // Le plan courant est le modèle : retoucher dans le plan 2 vaut pour le plan 1.
    let c2 = doc.new_cel(LayerKind::Vector);
    doc.panels[1].layers[0].frames[0] = Some(c2);
    doc.sync_partages(1);
    assert_eq!(dessin(&doc, 0, 0, 0), Some(c2));
    // Un exemplaire qui s'allonge est ramené à la durée de son plan : le calque partagé ne fixe jamais cette durée.
    let n0 = doc.panels[0].frame_count();
    doc.panels[0].layers[0].frames.resize(200, Some(c2));
    doc.sync_partages(0);
    assert_eq!(doc.panels[0].frame_count(), n0);
    // Un calque qui ne peut pas être partagé : refusé.
    doc.panels[0].layers.push(Layer::new_folder("Dossier"));
    let dossier = doc.panels[0].layers.len() - 1;
    assert!(!doc.panels[0].layers[dossier].partageable());
    assert_eq!(doc.reutiliser_calque(0, dossier, &[1]), None);
}

#[test]
fn detaching_keeps_a_copy_for_the_detached_one_and_the_last_one_loses_its_key() {
    let mut doc = film(3);
    let c0 = dessin(&doc, 0, 0, 0).unwrap();
    let k = doc.reutiliser_calque(0, 0, &[1, 2]).unwrap();
    doc.sync_partages(0);
    assert_eq!(doc.exemplaires(k).len(), 3);
    assert!(doc.detacher_partage(1, 0));
    assert_eq!(cle(&doc, 1, 0), None);
    let copie = dessin(&doc, 1, 0, 0).expect("un dessin");
    assert_ne!(copie, c0, "une copie à lui");
    assert!(doc.cels.contains_key(&copie));
    assert_eq!((cle(&doc, 0, 0), cle(&doc, 2, 0)), (Some(k), Some(k)), "les autres restent liés");
    assert_eq!(dessin(&doc, 2, 0, 0), Some(c0));
    // Détacher l'avant-dernier : le dernier perd la clé aussi, et garde le dessin d'origine.
    assert!(doc.detacher_partage(2, 0));
    assert_eq!((cle(&doc, 0, 0), cle(&doc, 2, 0)), (None, None));
    assert_eq!(dessin(&doc, 0, 0, 0), Some(c0));
    assert_ne!(dessin(&doc, 2, 0, 0), Some(c0));
    assert!(!doc.detacher_partage(0, 0), "plus rien à détacher");
    assert!(!doc.sync_partages(0));
    // Une clé laissée seule (plan supprimé) disparaît à l'accord.
    doc.reutiliser_calque(0, 0, &[1]);
    doc.panels.remove(1);
    assert!(doc.sync_partages(0));
    assert_eq!(cle(&doc, 0, 0), None);
}

#[test]
fn reusing_a_plan_links_all_its_drawing_layers_in_a_new_scene_and_detaching_the_plan_frees_them() {
    let mut doc = film(1);
    doc.panels[0].layers.push(Layer::new("Perso", LayerKind::Bitmap));
    let cb = doc.new_cel(LayerKind::Bitmap);
    doc.panels[0].layers[1].frames = vec![Some(cb)];
    doc.panels[0].action = "Il court.".into();
    let i = doc.reutiliser_plan_apres(0);
    assert_eq!((i, doc.panels.len()), (1, 2));
    assert_ne!(doc.panels[1].scene, doc.panels[0].scene, "une scène neuve : un plan au sens du découpage");
    assert_eq!(doc.sequence_of(1), doc.sequence_of(0), "dans la même séquence");
    for li in 0..2 {
        assert!(cle(&doc, 0, li).is_some(), "calque {li} : l'origine reçoit sa clé");
        assert_eq!(cle(&doc, 1, li), cle(&doc, 0, li));
        assert_eq!(dessin(&doc, 1, li, 0), dessin(&doc, 0, li, 0), "calque {li} : le même dessin");
    }
    assert_ne!(cle(&doc, 0, 0), cle(&doc, 0, 1), "une clé par calque");
    assert!(doc.panels[1].action.is_empty(), "les textes repartent vides");
    assert_eq!(doc.panels[1].frame_count(), doc.panels[0].frame_count());
    assert_eq!(doc.panels[1].camera, doc.panels[0].camera, "la caméra de l'origine, au départ");
    assert!(doc.plan_partage(1));
    // Détacher le plan : ses deux calques gardent des copies, et l'origine, seule, perd ses clés.
    assert_eq!(doc.detacher_plan(1), 2);
    assert!(doc.panels[1].layers.iter().all(|l| l.partage.is_none()));
    assert!(doc.panels[0].layers.iter().all(|l| l.partage.is_none()));
    assert_ne!(dessin(&doc, 1, 0, 0), dessin(&doc, 0, 0, 0));
    assert_ne!(dessin(&doc, 1, 1, 0), dessin(&doc, 0, 1, 0));
    assert_eq!(doc.detacher_plan(1), 0);
}

#[test]
fn cutting_keeps_the_link_merging_detaches_the_second_part_and_duplicating_drops_the_key() {
    let mut doc = film(2);
    let c0 = dessin(&doc, 0, 0, 0).unwrap();
    let k = doc.reutiliser_calque(0, 0, &[1]).unwrap();
    doc.sync_partages(0);
    assert!(doc.panels[0].frame_count() >= 4);
    let second = doc.split_panel(0, 2).expect("coupé");
    assert_eq!(second, 1);
    assert_eq!(cle(&doc, 1, 0), Some(k), "la seconde partie reste liée");
    assert_eq!(dessin(&doc, 1, 0, 0), Some(c0), "le même dessin, pas une copie");
    assert_eq!(doc.exemplaires(k).len(), 3);
    // Réunir : la seconde partie se détache d'abord (ses dessins copiés suivent), le lien reste entre le plan réuni et le plan 3.
    assert!(doc.merge_with_next(0).is_some());
    assert_eq!(doc.exemplaires(k).len(), 2);
    assert_eq!(cle(&doc, 0, 0), Some(k));
    assert_eq!(cle(&doc, 1, 0), Some(k));
    // Dupliquer : la copie n'a pas de clé et a ses propres dessins.
    let copie = doc.duplicate_panel(0);
    assert!(copie.layers.iter().all(|l| l.partage.is_none()));
    assert_ne!(copie.layers[0].cel_at(0), Some(c0));
    // Coller : pareil.
    let ids = vec![doc.panels[0].id];
    let clip = doc.copy_panels(&ids).expect("copié");
    let at = doc.paste_panels(&clip, 1).expect("collé");
    assert!(doc.panels[at].layers.iter().all(|l| l.partage.is_none()));
}

#[test]
fn the_link_is_saved_comes_back_and_a_newer_project_is_refused() {
    let mut doc = film(2);
    doc.reutiliser_calque(0, 0, &[1]).unwrap();
    doc.sync_partages(0);
    let dir = std::env::temp_dir().join(format!("feanor-partage-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("P.shot");
    crate::project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(&manifest).unwrap();
    assert!(texte.contains("\"partage\""), "la clé est écrite");
    assert_eq!(crate::project::VERSION, 51);
    assert!(texte.contains("\"version\": 51"));
    let back = crate::project::load(&manifest).unwrap().doc;
    let k = back.panels[0].layers[0].partage.expect("la clé revient");
    assert_eq!(back.panels[1].layers[0].partage, Some(k), "la même pour les deux exemplaires");
    assert_eq!(back.panels[1].layers[0].cel_at(0), back.panels[0].layers[0].cel_at(0), "le même dessin");
    // Un projet plus récent est refusé.
    std::fs::write(&manifest, texte.replace("\"version\": 51", "\"version\": 52")).unwrap();
    assert!(crate::project::load(&manifest).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

/// Vignettes, partie 125 face à la planche-mère : deux cases montrent le même dessin partagé (mêmes numéros `lien` de traits). La sélection du
/// trait entier prend la part une seule fois, la déplacer la déplace dans les deux cases (c'est le même dessin), et la repose ne la duplique pas.
#[test]
fn on_the_board_a_stroke_of_a_shared_drawing_is_taken_once_and_moving_it_moves_it_in_both_cases() {
    use crate::traits_continus_tests::{des_plans, planche, repos, tracer};
    use crate::vignettes_selection_tests::{choisies, glisser_droit, lasso, parts};
    use eframe::egui::vec2;
    let mut doc = des_plans(12, false);
    assert_eq!(doc.reutiliser_plan_apres(0), 1, "le plan 2 est le plan 1 réutilisé");
    let Some(mut b) = planche(doc, 6, 1.0) else { return };
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let (t0, t1) = (xf.rect(l.cards[0].thumb), xf.rect(l.cards[1].thumb));
    assert_eq!((l.cards[0].panel, l.cards[1].panel), (0, 1), "les deux premières cases sont les deux plans liés");
    // Un trait en biais au milieu de la case 1, bien dans son champ : il va dans le dessin partagé, que la case 2 montre aussi.
    let (de, a) = (t0.center() + vec2(-30.0, -20.0), t0.center() + vec2(30.0, 20.0));
    tracer(&mut b, de, a);
    repos(&mut b);
    let cel = b.app.doc.panels[0].layers[0].cel_at(0).expect("un dessin");
    assert_eq!(b.app.doc.panels[1].layers[0].cel_at(0), Some(cel), "le même dessin dans les deux plans");
    assert_eq!(parts(&b), (1, 0), "une part, dans le dessin, rien sur la planche");
    let avant = b.app.doc.cels[&cel].vector.as_ref().unwrap().elements[0].bounds();
    // Ctrl + lasso autour du trait dans la case 1 : une seule part choisie, pas deux (le dessin est montré par deux cases).
    lasso(&mut b, t0.center(), 45.0);
    assert_eq!(choisies(&b), 1, "{}", b.app.status);
    // Le glisser vers le bas, pris dans la boîte du choix (pas sur une poignée) : le trait bouge dans le dessin partagé, donc dans les deux
    // cases ; une part, toujours, nulle part ailleurs.
    let q = crate::vignettes_selection_tests::boite(&b);
    let prise = eframe::egui::pos2(q[0].x * 0.7 + q[2].x * 0.3, q[0].y * 0.7 + q[2].y * 0.3);
    glisser_droit(&mut b, prise, vec2(0.0, 20.0));
    repos(&mut b);
    assert_eq!(parts(&b), (1, 0), "{}", b.app.status);
    assert_eq!(b.app.doc.panels[0].layers[0].cel_at(0), Some(cel));
    assert_eq!(b.app.doc.panels[1].layers[0].cel_at(0), Some(cel), "le lien tient");
    let apres = b.app.doc.cels[&cel].vector.as_ref().unwrap().elements[0].bounds();
    assert!(apres.center().y > avant.center().y + 5.0, "le trait est descendu : {avant:?} → {apres:?}");
    assert!((apres.center().x - avant.center().x).abs() < 3.0);
    // La case 2 n'est pas touchée autrement : elle n'a toujours qu'un calque, le partagé.
    assert_eq!(b.app.doc.panels[1].layers.len(), b.app.doc.panels[0].layers.len());
    let _ = t1;
}

#[test]
fn from_the_app_the_commands_share_detach_reuse_a_plan_choose_plans_and_undo() {
    let doc = film(3);
    let mut b = banc_taille(doc, false, 1280, 720);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.chauffer(3);
    let c0 = b.app.doc.panels[0].layers[0].cel_at(0).unwrap();
    let etapes = b.app.history.names().0.len();
    b.app.run_action("calque.reutiliser_suivant");
    assert!(b.app.status.contains("partagé"), "{}", b.app.status);
    let k = b.app.doc.panels[0].layers[0].partage.expect("une clé");
    assert_eq!(b.app.doc.panels[1].layers[0].partage, Some(k));
    assert_eq!(b.app.history.names().0.len(), etapes + 1);
    assert_eq!(b.app.history.names().0.last().copied(), Some("Réutiliser le calque"));
    b.chauffer(2);
    assert!(b.app.doc.panels[1].layers[0].frames.iter().all(|f| *f == Some(c0)), "tenu sur tout le plan 2");
    // Un autre dessin dans le plan courant : le plan 2 le montre à l'image d'écran suivante (l'accord tourne à chaque image).
    let c1 = b.app.doc.new_cel(LayerKind::Vector);
    b.app.layers_mut()[0].frames[0] = Some(c1);
    b.chauffer(2);
    assert_eq!(b.app.doc.panels[1].layers[0].cel_at(0), Some(c1));
    // Ctrl+Z : plus de partage, le plan 2 n'a plus l'exemplaire ; rétablir le rend.
    b.app.undo();
    b.chauffer(1);
    assert!(b.app.doc.panels[0].layers[0].partage.is_none());
    assert_eq!(b.app.doc.panels[1].layers.len(), 1);
    b.app.redo();
    b.chauffer(1);
    assert_eq!(b.app.doc.panels[1].layers.len(), 2);
    assert_eq!(b.app.doc.panels[1].layers[0].partage, Some(k));
    // Toute la séquence : le plan 3 aussi.
    b.app.run_action("calque.reutiliser_sequence");
    assert_eq!(b.app.doc.plans_du_partage(0, 0), [0, 1, 2]);
    // Détacher le calque courant : les deux autres restent liés entre eux.
    b.app.run_action("calque.detacher_partage");
    assert!(b.app.status.contains("détaché"), "{}", b.app.status);
    assert!(b.app.doc.panels[0].layers[0].partage.is_none());
    assert_eq!(b.app.doc.panels[1].layers[0].partage, Some(k));
    assert_eq!(b.app.doc.panels[2].layers[0].partage, Some(k));
    b.app.run_action("calque.detacher_partage");
    assert!(b.app.status.contains("n'est pas partagé"), "{}", b.app.status);
    // Réutiliser le plan : un plan lié juste après, devenu courant.
    b.app.run_action("plan.reutiliser");
    assert_eq!(b.app.doc.panels.len(), 4);
    assert_eq!(b.app.cur_panel, 1, "le nouveau plan est courant");
    assert!(b.app.status.contains("réutilisé"), "{}", b.app.status);
    assert!(b.app.doc.plan_partage(1) && b.app.doc.plan_partage(0));
    assert_eq!(b.app.doc.panels[1].layers[0].cel_at(0), b.app.doc.panels[0].layers[0].cel_at(0));
    b.app.run_action("plan.detacher");
    assert!(!b.app.doc.plan_partage(1) && !b.app.doc.plan_partage(0), "{}", b.app.status);
    b.app.run_action("plan.detacher");
    assert!(b.app.status.contains("ne partage aucun"), "{}", b.app.status);
    // Les plans à cocher : la fenêtre s'ouvre, cocher le dernier plan et appliquer le lie.
    b.app.cur_panel = 0;
    b.app.run_action("calque.reutiliser_choisir");
    let choix = b.app.reutiliser.as_ref().expect("la fenêtre");
    assert_eq!(choix.plans.len(), 4);
    assert!(choix.plans[0].1 && !choix.plans[3].1);
    b.chauffer(2);
    assert!(b.app.reutiliser.is_some(), "elle reste ouverte");
    let mut choix = b.app.reutiliser.take().unwrap();
    choix.plans[3].1 = true;
    b.app.appliquer_choix_reutiliser(&choix);
    assert_eq!(b.app.doc.plans_du_partage(0, 0), [0, 3], "{}", b.app.status);
    // Décocher le même plan le détache.
    choix.plans[3].1 = false;
    b.app.appliquer_choix_reutiliser(&choix);
    assert!(b.app.doc.panels[0].layers[0].partage.is_none());
    assert!(b.app.status.contains("n'est plus partagé"), "{}", b.app.status);
    // Les commandes sont déclarées (palette Ctrl+K) avec un nom clair.
    for a in ["plan.reutiliser", "plan.detacher", "calque.reutiliser_suivant", "calque.reutiliser_precedent", "calque.reutiliser_sequence", "calque.reutiliser_choisir", "calque.detacher_partage"] {
        let info = crate::keymap::action_info(a).unwrap_or_else(|| panic!("{a} déclarée"));
        assert!(info.implemented && info.label.contains("planche-mère"), "{a} : {}", info.label);
    }
}
