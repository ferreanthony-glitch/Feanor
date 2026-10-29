//! Brosses perso (§70, Anthony 2026-10-29) : une seule sorte de chose, de trois types (Personnage, Classique, Décor), dans la barre, dans le
//! panneau « Brosses perso », dans les réglages et dans le projet. Les essais d'avant les concernent aussi (`personnages_tests`,
//! `calques_pastilles_tests`, `decor_tests`, `pastilles_storyboard_tests`, `aplatperso_tests`, `aplattrous_tests`) : ils ont été adaptés
//! avec un commentaire qui dit pourquoi.

use crate::calques_pastilles_tests::{banc, calques_de, case, clic_case, encre, personnage, trait_au_milieu};
use crate::decor::Genre;
use crate::document::{Document, LayerKind};
use crate::perf_tests::banc_taille;
use crate::personnages::{self, Type};
use crate::personnagesui::{Case, Choix};
use crate::settings::Settings;

// ------------------------------------------------------------------------------------------ la barre d'un projet neuf

#[test]
fn a_new_project_has_three_of_each_type_in_the_right_order_and_colours_and_no_classic_pastille() {
    crate::fxcache::monde::ecrire();
    let doc = crate::nouveau::build_document(&crate::nouveau::Values::default(), "Neuf");
    let persos: Vec<u64> = doc.personnages.iter().map(|p| p.id).collect();
    assert_eq!(persos.len(), 3);
    let mut b = banc_taille(doc, false, 1600, 900);
    b.chauffer(6);
    let attendu: Vec<Case> = persos
        .iter()
        .map(|id| Case::Choix(Choix::Personnage(*id)))
        .chain((0..3).map(|i| Case::Choix(Choix::Classique(i))))
        .chain((0..3).map(|i| Case::Choix(Choix::Decor(i))))
        .chain([Case::Plus, Case::Panneau])
        .collect();
    let cases: Vec<Case> = b.app.perso.cases.iter().map(|(_, c)| *c).collect();
    assert_eq!(cases, attendu, "Personnages, Classiques, Décors, puis « + » et le panneau : rien d'autre en tête");
    assert_eq!((b.app.perso_du_type(Type::Personnage).len(), b.app.perso_du_type(Type::Classique).len(), b.app.perso_du_type(Type::Decor).len()), (3, 3, 3));
    // Les couleurs d'origine.
    let c = |ch: Choix| b.app.perso_couleurs(ch).expect("une brosse");
    assert_eq!(c(Choix::Personnage(persos[0])), ([39, 2, 2], Some([150, 97, 97])));
    assert_eq!(c(Choix::Classique(0)), ([0x1E; 3], Some([0x8C; 3])));
    assert_eq!(c(Choix::Classique(2)), ([0x7A; 3], Some([0xDA; 3])));
    assert_eq!(c(Choix::Decor(0)), ([0x6E; 3], Some([0xC8; 3])));
    assert_eq!(c(Choix::Decor(1)), ([0x8E; 3], Some([0xD8; 3])));
    assert_eq!(c(Choix::Decor(2)), ([0xAB; 3], Some([0xE6; 3])));
    for d in personnages::decors_par_defaut() {
        assert!(d.line.iter().zip(d.fill.expect("un aplat")).all(|(t, a)| *t < a), "le trait d'une brosse Décor est plus foncé que son aplat");
    }
    // Les formes : des ronds normaux, des petits ronds, des carrés arrondis.
    let r = |ch: Choix| b.app.perso.cases.iter().find(|(_, k)| *k == Case::Choix(ch)).map(|(r, _)| *r).expect("la case");
    assert!(r(Choix::Classique(0)).width() + 6.0 <= r(Choix::Personnage(persos[0])).width(), "les classiques sont plus petites que les personnages");
    assert!((r(Choix::Decor(0)).width() - r(Choix::Personnage(persos[0])).width()).abs() < 0.5, "les décors ont la case d'un personnage");
    assert_eq!(b.app.settings.brosses_decor, personnages::decors_par_defaut());
}

// ------------------------------------------------------------------------------------------ ce que fait chaque type

#[test]
fn a_decor_brush_makes_the_vector_decor_layer_with_its_own_aplat_in_one_step_and_a_decor_bitmap_follows_the_setting() {
    let mut b = banc();
    let (couches, avant) = (b.app.layers().len(), b.app.history.undo_len());
    clic_case(&mut b, Case::Choix(Choix::Decor(1)));
    assert_eq!(b.app.color, [0x8E; 3], "la couleur de trait de Décor 2");
    assert_eq!(b.app.doc.decor_de(b.app.cur_panel), None, "choisir ne crée rien");
    trait_au_milieu(&mut b, 0.0);
    let at = b.app.doc.decor_de(b.app.cur_panel).expect("le calque Décor est né au premier trait");
    let l = &b.app.layers()[at];
    assert_eq!(l.kind, LayerKind::Vector, "vectoriel d'origine");
    assert_eq!(l.fond.map(|f| f.couleur), Some([0xD8; 3]), "avec l'aplat de la brosse");
    assert_eq!(b.app.layers().len(), couches + 1);
    assert!(encre(&b, at));
    assert_eq!(b.app.history.undo_len(), avant + 1, "le calque, son aplat et le premier trait ne font qu'une étape");
    // Une autre brosse Décor : le même calque, un trait de plus.
    clic_case(&mut b, Case::Choix(Choix::Decor(2)));
    trait_au_milieu(&mut b, 30.0);
    assert_eq!(b.app.layers().iter().filter(|l| l.decor).count(), 1, "un seul calque Décor par plan");
    // Bitmap selon Préférences > Vignettes, dans un projet neuf.
    let mut b = banc();
    b.app.settings.vignettes.decor_bitmap = true;
    clic_case(&mut b, Case::Choix(Choix::Decor(0)));
    trait_au_milieu(&mut b, 0.0);
    let at = b.app.doc.decor_de(b.app.cur_panel).expect("le calque Décor");
    assert_eq!(b.app.layers()[at].kind, LayerKind::Bitmap, "bitmap quand le réglage le demande");
}

#[test]
fn a_decor_brush_follows_its_pinceau_and_size_and_is_let_go_when_deleted() {
    let mut b = banc();
    let (outil0, pinceau0) = (b.app.tool, b.app.brush);
    clic_case(&mut b, Case::Choix(Choix::Decor(0)));
    assert_eq!((b.app.tool, b.app.pencil.size), (crate::app::Tool::Pencil, Genre::Crayon.taille_defaut()), "Décor 1 : un crayon de 5 px");
    // Le panneau change son pinceau et sa taille : la brosse choisie les prend à l'image suivante.
    b.app.perso_ecrire_decor(0, Genre::Aplat, 60.0);
    b.chauffer(3);
    assert_eq!(b.app.tool, crate::app::Tool::Brush);
    assert_eq!(b.app.brush.size, 60.0);
    assert!((b.app.brush.hardness - 1.0).abs() < 1e-6, "un pinceau plein");
    // Une autre couleur de trait : elle aussi.
    b.app.perso_ecrire(Choix::Decor(0), "Décor 1".into(), [10, 20, 30], Some([200, 210, 220]));
    b.chauffer(3);
    assert_eq!(b.app.color, [10, 20, 30]);
    // La supprimer la lâche : l'outil et le pinceau d'avant reviennent.
    b.app.perso_supprimer(Choix::Decor(0));
    assert_eq!(b.app.perso.actif, None);
    assert_eq!((b.app.tool, b.app.brush), (outil0, pinceau0));
    assert_eq!(b.app.settings.brosses_decor.len(), 2);
    // Les rangs des suivantes bougent avec la liste : la brosse choisie les suit.
    clic_case(&mut b, Case::Choix(Choix::Decor(1)));
    b.app.perso_deplacer(Choix::Decor(1), -1);
    assert_eq!(b.app.perso.actif, Some(Choix::Decor(0)), "le choix suit sa brosse");
    b.app.perso_dupliquer(Choix::Decor(0));
    assert_eq!(b.app.settings.brosses_decor.len(), 3);
    assert_eq!(b.app.perso.actif, Some(Choix::Decor(0)));
    assert!(b.app.settings_a_ecrire);
}

#[test]
fn clicking_the_chosen_brush_lets_go_of_each_type_and_the_colour_is_free() {
    let mut b = banc();
    let id = personnage(&mut b, "Lou", [200, 30, 30], Some([250, 200, 200]));
    b.chauffer(3);
    for ch in [Choix::Personnage(id), Choix::Classique(1), Choix::Decor(2)] {
        clic_case(&mut b, Case::Choix(ch));
        if let Choix::Personnage(_) = ch {
            // (Le personnage était déjà choisi à sa création : le premier clic le lâche, le second le reprend.)
            assert_eq!(b.app.perso.actif, None);
            clic_case(&mut b, Case::Choix(ch));
        }
        assert_eq!(b.app.perso.actif, Some(ch), "{ch:?} choisie");
        clic_case(&mut b, Case::Choix(ch));
        assert_eq!(b.app.perso.actif, None, "{ch:?} lâchée par un second clic : couleur libre");
        assert_eq!(b.app.decor_actif(), None);
    }
}

// ------------------------------------------------------------------------------------------ le « + »

#[test]
fn the_plus_opens_a_menu_of_three_types_and_each_adds_its_brush_without_a_window() {
    let mut b = banc();
    clic_case(&mut b, Case::Plus);
    for t in Type::ALL {
        assert!(b.app.perso.cases.iter().any(|(_, c)| *c == Case::Ajouter(t)), "{t:?} dans le menu : {:?}", b.app.perso.cases);
    }
    let ajoute = |b: &mut crate::perf_tests::Banc, t: Type| {
        let p = case(b, Case::Ajouter(t));
        crate::calques_pastilles_tests::clic(b, p);
    };
    ajoute(&mut b, Type::Classique);
    assert_eq!(b.app.settings.brosses_classiques.len(), 4);
    assert_eq!(b.app.perso.actif, Some(Choix::Classique(3)), "choisie tout de suite");
    assert_eq!(b.app.settings.brosses_classiques[3].name, "Classique 4");
    assert_eq!(b.app.perso.edition, None, "sans fenêtre Modifier");
    clic_case(&mut b, Case::Plus);
    ajoute(&mut b, Type::Decor);
    assert_eq!(b.app.settings.brosses_decor.len(), 4);
    assert_eq!(b.app.perso.actif, Some(Choix::Decor(3)));
    assert_eq!(b.app.decor_actif(), Some(3));
    assert_eq!(b.app.settings.brosses_decor[3].name, "Décor 4");
    clic_case(&mut b, Case::Plus);
    ajoute(&mut b, Type::Personnage);
    assert_eq!(b.app.doc.personnages.len(), 1);
    assert_eq!(b.app.perso.actif, Some(Choix::Personnage(b.app.doc.personnages[0].id)));
    // Ajouter un personnage s'annule ; une classique ou un décor sont un réglage de SHOT, sans annulation.
    b.app.undo();
    assert!(b.app.doc.personnages.is_empty());
}

// ------------------------------------------------------------------------------------------ le panneau et les ↺

#[test]
fn the_panel_has_three_sections_and_each_reset_restores_the_original_values_of_its_type() {
    // (Une fenêtre haute : le panneau montre ses trois sections sans défiler.)
    crate::fxcache::monde::ecrire();
    let mut b = banc_taille(Document::new(400, 300, 24), false, 1600, 2400);
    b.app.tool = crate::app::Tool::Brush;
    b.garder_textes = true;
    b.app.show_pane(crate::dock::Pane::Personnages);
    b.chauffer(4);
    for t in ["Personnages", "Classiques", "Décors"] {
        assert!(b.textes.iter().any(|x| x.trim() == t), "la section « {t} » : {:?}", b.textes.iter().take(60).collect::<Vec<_>>());
    }
    // Classique : changée, puis remise ; les Décors ne bougent pas.
    b.app.settings.brosses_classiques[0].line = [1, 2, 3];
    b.app.settings.brosses_decor[1].name = "À moi".into();
    b.app.perso_ajouter_poste(Type::Classique);
    assert_eq!(b.app.settings.brosses_classiques.len(), 4);
    b.app.perso_d_origine(Type::Classique);
    assert_eq!(b.app.settings.brosses_classiques, personnages::classiques_par_defaut());
    assert_eq!(b.app.settings.brosses_decor[1].name, "À moi", "les décors ne sont pas touchés par le ↺ des classiques");
    assert_eq!(b.app.perso.actif, None, "la brosse choisie qui disparaît est lâchée");
    b.app.perso_d_origine(Type::Decor);
    assert_eq!(b.app.settings.brosses_decor, personnages::decors_par_defaut());
}

#[test]
fn the_personnage_reset_restores_perso_1_to_3_in_one_undo_step_and_leaves_the_others_alone() {
    crate::fxcache::monde::ecrire();
    let doc = crate::nouveau::build_document(&crate::nouveau::Values::default(), "Neuf");
    let mut b = banc_taille(doc, false, 1600, 900);
    b.chauffer(4);
    let ids: Vec<u64> = b.app.doc.personnages.iter().map(|p| p.id).collect();
    let lea = b.app.perso_ajouter().expect("un personnage de plus");
    b.app.perso_ecrire(Choix::Personnage(lea), "Léa".into(), [9, 9, 9], None);
    b.app.record_step();
    b.app.perso_ecrire(Choix::Personnage(ids[1]), "Perso 2".into(), [1, 1, 1], Some([2, 2, 2]));
    b.app.perso_supprimer(Choix::Personnage(ids[2]));
    let avant = b.app.history.undo_len();
    b.app.perso_d_origine(Type::Personnage);
    assert_eq!(b.app.history.undo_len(), avant + 1, "une seule étape d'annulation");
    let p = &b.app.doc.personnages;
    assert_eq!(p.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), ["Perso 1", "Perso 2", "Léa", "Perso 3"], "Perso 3 revient au bout de la liste");
    assert_eq!((p[1].line, p[1].fill), ([0, 28, 46], Some([78, 110, 145])), "Perso 2 retrouve ses couleurs");
    assert_eq!((p[2].line, p[2].fill), ([9, 9, 9], None), "Léa n'est pas touchée");
    b.app.undo();
    let p = &b.app.doc.personnages;
    assert_eq!(p.len(), 3);
    assert_eq!(p[1].line, [1, 1, 1], "l'annulation rend d'un coup l'état d'avant");
    // Rien à remettre : pas d'étape de plus.
    b.app.perso_d_origine(Type::Personnage);
    b.app.perso_d_origine(Type::Personnage);
    let n = b.app.history.undo_len();
    b.app.perso_d_origine(Type::Personnage);
    assert_eq!(b.app.history.undo_len(), n, "déjà à l'origine : rien à défaire");
}

#[test]
fn the_bubbles_say_the_type_and_what_the_brush_does() {
    let d = crate::personnagesui::description(Type::Decor, [0x6E; 3], Some([0xC8; 3]), false, Some((Genre::Crayon, 5.0)));
    assert!(d.contains("Brosse décor") && d.contains("calque Décor") && d.contains("crayon") && d.contains("5 px"), "{d}");
    let c = crate::personnagesui::description(Type::Classique, [0x1E; 3], None, true, None);
    assert!(c.contains("Brosse classique") && c.contains("calque courant") && c.contains("sans aplat") && c.contains("lâcher"), "{c}");
    let p = crate::personnagesui::description(Type::Personnage, [1, 2, 3], Some([4, 5, 6]), false, None);
    assert!(p.contains("Brosse personnage") && p.contains("calque à son nom"), "{p}");
}

// ------------------------------------------------------------------------------------------ anciens réglages et anciens projets

#[test]
fn old_settings_with_neutral_colours_and_decor_sizes_are_read_without_loss() {
    let ancien = "couleurs_neutres = [{ nom = \"Sépia\", trait = [100, 60, 30] }, { nom = \"Gris X\", trait = [10, 10, 10], fond = [200, 200, 200] }]\n\
                  [vignettes]\ncouleur_decor = [10, 20, 30]\ntailles_decor = [3.0, 30.0, 120.0]\n";
    let s = Settings::apres_lecture(toml::from_str(ancien).expect("d'anciens réglages"));
    let noms: Vec<&str> = s.brosses_classiques.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(noms, ["Sépia", "Gris X"], "les anciennes couleurs neutres sont des brosses Classique");
    assert_eq!(s.brosses_classiques[0].fill, None);
    assert_eq!(s.brosses_classiques[1].fill, Some([200, 200, 200]));
    assert!(s.brosses_classiques.iter().all(|b| b.pinceau.is_none()));
    // Les trois Décors d'origine, avec les anciennes tailles ; l'ancienne clé n'est plus écrite.
    let tailles: Vec<f32> = s.brosses_decor.iter().map(|b| b.taille_decor()).collect();
    assert_eq!(tailles, [3.0, 30.0, 120.0]);
    assert_eq!(s.brosses_decor.iter().map(|b| b.line).collect::<Vec<_>>(), personnages::decors_par_defaut().iter().map(|b| b.line).collect::<Vec<_>>());
    let texte = toml::to_string(&s).expect("écriture");
    assert!(!texte.contains("tailles_decor") && !texte.contains("couleur_decor"), "{texte}");
    assert!(texte.contains("brosses_decor") && texte.contains("couleurs_neutres"));
    // Relu : rien ne change, et les anciennes tailles ne reviennent pas écraser un réglage fait depuis.
    let mut relu = Settings::apres_lecture(toml::from_str(&texte).expect("relecture"));
    assert_eq!(relu.brosses_decor, s.brosses_decor);
    relu.brosses_decor[0].taille = 7.0;
    let relu = Settings::apres_lecture(toml::from_str(&toml::to_string(&relu).expect("écriture")).expect("relecture"));
    assert_eq!(relu.brosses_decor[0].taille_decor(), 7.0);
    // Sans rien : les valeurs d'origine.
    let neuf = Settings::apres_lecture(Settings::default());
    assert_eq!(neuf.brosses_classiques, personnages::classiques_par_defaut());
    assert_eq!(neuf.brosses_decor, personnages::decors_par_defaut());
}

#[test]
fn an_old_project_with_characters_reads_as_personnage_brushes_without_loss() {
    // Le format du projet ne change pas : `personnages.json` (version 1) et le champ `personnage` des calques.
    let texte = r#"{"format":"shot-personnages","version":1,"personnages":[{"id":7,"nom":"Lou","trait":[200,30,30],"fond":[250,200,200]},{"id":9,"nom":"Sans aplat","trait":[0,0,0]}]}"#;
    let lus = personnages::parse(texte).expect("un ancien fichier");
    assert_eq!(lus.len(), 2);
    assert_eq!((lus[0].id, lus[0].name.as_str(), lus[0].fill), (7, "Lou", Some([250, 200, 200])));
    assert_eq!(lus[1].fill, None);
    let mut doc = Document::new(64, 48, 24);
    doc.personnages = lus;
    assert_eq!(personnages::to_bytes(&doc).expect("écriture").map(|b| b.len() > 10), Some(true));
    assert!(crate::project::VERSION >= 42, "les personnages n'ont pas changé le format (42 ; 43 vient des clés d'opacité)");
}

// ------------------------------------------------------------------------------------------ le calque d'un personnage

#[test]
fn choosing_a_character_brush_stays_in_a_copy_then_goes_to_the_last_used_layer_then_to_the_top() {
    let mut b = banc();
    let lou = personnage(&mut b, "Lou", [200, 30, 30], Some([250, 200, 200]));
    trait_au_milieu(&mut b, 0.0);
    let original = calques_de(&b.app.doc, 0, lou)[0];
    b.app.select_layer(original);
    b.app.duplicate_layer();
    let tous = calques_de(&b.app.doc, 0, lou);
    assert_eq!(tous.len(), 2, "la copie reste liée au personnage");
    let copie = tous.iter().copied().find(|i| *i != original).expect("la copie");
    // Le calque courant est la copie : choisir la brosse y reste.
    b.app.select_layer(copie);
    b.app.perso_lacher();
    b.app.perso_choisir(Choix::Personnage(lou));
    assert_eq!(b.app.cur_layer, copie, "on reste dans la copie");
    // Le calque courant est ordinaire : le dernier utilisé (la copie, choisie en dernier), pas l'original.
    b.app.select_layer(0);
    b.app.perso_choisir(Choix::Personnage(lou));
    assert_eq!(b.app.cur_layer, copie, "le dernier calque du personnage utilisé dans le plan");
    // Un trait avec la brosse choisie et un autre calque courant va dans ce dernier calque, sans en créer.
    b.app.select_layer(0);
    let couches = b.app.layers().len();
    trait_au_milieu(&mut b, 20.0);
    assert_eq!(b.app.layers().len(), couches, "aucun calque de plus");
    assert!(encre(&b, copie), "le trait est dans la copie");
    // L'original, choisi à la main, devient le dernier utilisé.
    b.app.select_layer(original);
    b.app.select_layer(0);
    b.app.perso_choisir(Choix::Personnage(lou));
    assert_eq!(b.app.cur_layer, original, "le plus récemment choisi");
    // Sans mémoire (rouvrir le projet) : le plus haut dans la pile.
    b.app.perso.dernier_calque.clear();
    b.app.select_layer(0);
    b.app.perso_choisir(Choix::Personnage(lou));
    assert_eq!(b.app.cur_layer, copie.max(original), "à défaut, le plus haut dans la pile");
}
