//! Brosses dédiées (§70, Anthony 2026-10-29) : une seule sorte de chose, de trois types (Personnage, Classique, Décor), dans la barre, dans le
//! panneau « Brosses dédiées », dans les réglages et dans le projet. Les essais d'avant les concernent aussi (`personnages_tests`,
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
fn a_decor_brush_follows_the_pinceau_the_user_set_and_is_let_go_when_deleted() {
    // (Adapté au §107 : une brosse Décor d'origine ne prend plus ni outil ni taille, elle suit le pinceau de l'utilisateur.)
    let mut b = banc();
    b.app.set_tool(crate::app::Tool::Brush);
    let (outil0, pinceau0, crayon0) = (b.app.tool, b.app.brush, b.app.pencil);
    clic_case(&mut b, Case::Choix(Choix::Decor(0)));
    assert_eq!((b.app.tool, b.app.brush, b.app.pencil), (outil0, pinceau0, crayon0), "Décor 1 suit le pinceau : rien ne change au choix");
    // L'utilisateur change de taille : la brosse choisie dessine avec la nouvelle (c'est son pinceau).
    b.app.brush.size = 33.0;
    b.chauffer(3);
    assert_eq!(b.app.brush.size, 33.0, "la taille de l'utilisateur n'est pas reprise par la brosse");
    // Une autre couleur de trait : elle suit tout de suite.
    b.app.perso_ecrire(Choix::Decor(0), "Décor 1".into(), [10, 20, 30], Some([200, 210, 220]));
    b.chauffer(3);
    assert_eq!(b.app.color, [10, 20, 30]);
    // La supprimer la lâche : le pinceau de l'utilisateur est celui qu'il a réglé.
    b.app.perso_supprimer(Choix::Decor(0));
    assert_eq!(b.app.perso.actif, None);
    assert_eq!((b.app.tool, b.app.brush.size), (outil0, 33.0));
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
    let d = crate::personnagesui::description(Type::Decor, [0x6E; 3], Some([0xC8; 3]), false, "Crayon, 5 px, crayon");
    assert!(d.contains("Brosse décor") && d.contains("calque Décor") && d.contains("Dessine avec : Crayon, 5 px, crayon"), "{d}");
    let c = crate::personnagesui::description(Type::Classique, [0x1E; 3], None, true, "Suit le pinceau");
    assert!(c.contains("Suit le pinceau"), "{c}");
    assert!(c.contains("Brosse classique") && c.contains("calque courant") && c.contains("sans aplat") && c.contains("lâcher"), "{c}");
    let p = crate::personnagesui::description(Type::Personnage, [1, 2, 3], Some([4, 5, 6]), false, "Suit le pinceau");
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
    // Les trois Décors d'origine, avec les anciennes tailles : elles n'étaient pas celles d'origine, ce sont donc trois brosses spécifiques (§107) ;
    // l'ancienne clé n'est plus écrite.
    let tailles: Vec<f32> = s.brosses_decor.iter().map(|b| b.specifique.as_ref().expect("une brosse spécifique").settings.size).collect();
    assert_eq!(tailles, [3.0, 30.0, 120.0]);
    assert_eq!(s.brosses_decor.iter().map(|b| b.specifique.as_ref().map(|x| x.crayon)).collect::<Vec<_>>(), [Some(true), Some(false), Some(false)], "le crayon reste un crayon");
    assert_eq!(s.brosses_decor.iter().map(|b| b.line).collect::<Vec<_>>(), personnages::decors_par_defaut().iter().map(|b| b.line).collect::<Vec<_>>());
    let texte = toml::to_string(&s).expect("écriture");
    assert!(!texte.contains("tailles_decor") && !texte.contains("couleur_decor"), "{texte}");
    assert!(texte.contains("brosses_decor") && texte.contains("couleurs_neutres"));
    // Relu : rien ne change, et les anciennes tailles ne reviennent pas écraser un réglage fait depuis.
    let mut relu = Settings::apres_lecture(toml::from_str(&texte).expect("relecture"));
    assert_eq!(relu.brosses_decor, s.brosses_decor);
    relu.brosses_decor[0].pinceau = Some(Genre::Crayon);
    relu.brosses_decor[0].taille = 7.0;
    relu.brosses_decor[0].specifique = None;
    let relu = Settings::apres_lecture(toml::from_str(&toml::to_string(&relu).expect("écriture")).expect("relecture"));
    assert_eq!(relu.brosses_decor[0].specifique.as_ref().map(|x| x.settings.size), Some(7.0), "un ancien crayon de 7 px devient une brosse spécifique de 7 px");
    assert!(relu.brosses_decor[0].pinceau.is_none() && relu.brosses_decor[0].taille == 0.0, "les anciens champs sont vidés");
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

// ------------------------------------------------------------------------------------------ §107 : avec quoi elles dessinent

use crate::app::Tool;
use crate::brush::BrushSettings;
use crate::personnages::Specifique;

/// Un pinceau d'utilisateur reconnaissable (ni celui d'origine, ni un réglage de brosse dédiée).
fn pinceau_de_l_utilisateur(b: &mut crate::perf_tests::Banc) -> BrushSettings {
    b.app.set_tool(Tool::Brush);
    b.app.brush = BrushSettings { size: 17.0, hardness: 0.37, opacity: 0.8, grain: 0.4, ..b.app.brush };
    b.app.pencil = BrushSettings { size: 3.0, ..b.app.pencil };
    b.chauffer(3);
    b.app.brush
}

fn une_de_chaque(b: &mut crate::perf_tests::Banc) -> [Choix; 3] {
    let lou = personnage(b, "Lou", [200, 30, 30], Some([250, 200, 200]));
    b.app.perso_lacher();
    [Choix::Personnage(lou), Choix::Classique(0), Choix::Decor(0)]
}

fn feutre(taille: f32) -> Specifique {
    Specifique { crayon: false, settings: BrushSettings { size: taille, hardness: 0.1, ..BrushSettings::felt() }, nom: None }
}

#[test]
fn a_brush_of_every_type_follows_the_users_brush_and_never_touches_it() {
    let mut b = banc();
    let [lou, gris, decor] = une_de_chaque(&mut b);
    let (outil, pinceau, crayon) = (b.app.tool, pinceau_de_l_utilisateur(&mut b), b.app.pencil);
    for c in [lou, gris, decor] {
        assert_eq!(b.app.dediee_specifique(c), None, "{c:?} : « Suit le pinceau » d'origine");
        b.app.perso_choisir(c);
        b.chauffer(2);
        assert_eq!((b.app.tool, b.app.brush, b.app.pencil), (outil, pinceau, crayon), "{c:?} : choisie, le pinceau n'a pas bougé");
        // L'utilisateur change de taille : elle suit, c'est son pinceau.
        b.app.brush.size = 41.0;
        b.chauffer(2);
        assert_eq!(b.app.brush.size, 41.0, "{c:?} suit la taille de l'utilisateur");
        assert_eq!(b.app.dediee_specifique(c), None, "rien n'est gardé pour elle");
        b.app.brush.size = pinceau.size;
        b.app.perso_lacher();
        assert_eq!(b.app.brush, pinceau, "{c:?} lâchée : le pinceau est celui de l'utilisateur");
        b.chauffer(2);
    }
    // Un pinceau tout autre (le feutre d'origine) : elles le suivent aussi.
    b.app.brush = BrushSettings::felt();
    for c in [lou, gris, decor] {
        b.app.perso_choisir(c);
        b.chauffer(2);
        assert_eq!(b.app.brush, BrushSettings::felt(), "{c:?} suit le changement de pinceau");
    }
}

#[test]
fn a_specific_brush_takes_its_own_brush_and_gives_back_the_users_exactly_for_each_type_and_tool() {
    let mut b = banc();
    let [lou, gris, decor] = une_de_chaque(&mut b);
    let pinceau = pinceau_de_l_utilisateur(&mut b);
    let (outil, crayon) = (b.app.tool, b.app.pencil);
    for (c, avec_crayon) in [(lou, false), (gris, true), (decor, false)] {
        let sp = if avec_crayon { Specifique { crayon: true, settings: BrushSettings { size: 6.0, ..BrushSettings::graphite() }, nom: None } } else { feutre(40.0) };
        b.app.dediee_ecrire(c, Some(sp.clone()));
        b.app.perso_choisir(c);
        b.chauffer(2);
        if avec_crayon {
            assert_eq!((b.app.tool, b.app.pencil), (Tool::Pencil, sp.settings), "{c:?} : le crayon à elle");
            assert_eq!(b.app.brush, pinceau, "{c:?} : le pinceau de l'utilisateur n'a pas bougé");
        } else {
            assert_eq!((b.app.tool, b.app.brush), (Tool::Brush, sp.settings), "{c:?} : le pinceau à elle");
            assert_eq!(b.app.pencil, crayon);
        }
        b.app.perso_lacher();
        assert_eq!((b.app.tool, b.app.brush, b.app.pencil), (outil, pinceau, crayon), "{c:?} lâchée : tout est rendu tel quel");
        assert!(b.app.perso.pris.is_none());
        b.app.dediee_ecrire(c, None);
    }
}

#[test]
fn going_back_and_forth_between_brushes_never_changes_the_users_brush() {
    let mut b = banc();
    let [lou, gris, decor] = une_de_chaque(&mut b);
    let pinceau = pinceau_de_l_utilisateur(&mut b);
    b.app.dediee_ecrire(gris, Some(feutre(55.0)));
    // Qui suit -> spécifique -> qui suit -> spécifique -> spécifique -> qui suit, plusieurs fois.
    for _ in 0..3 {
        for c in [lou, gris, decor, gris, gris, lou, decor] {
            b.app.perso_choisir(c);
            b.chauffer(1);
            if b.app.dediee_specifique(c).is_none() {
                assert_eq!(b.app.brush, pinceau, "{c:?} suit le pinceau de l'utilisateur, jamais celui d'une autre brosse");
            } else {
                assert_eq!(b.app.brush.size, 55.0);
            }
        }
    }
    b.app.perso_lacher();
    assert_eq!(b.app.brush, pinceau);
    assert_eq!(b.app.dediee_specifique(gris).map(|s| s.settings.size), Some(55.0), "la brosse spécifique garde son réglage");
}

#[test]
fn what_the_user_sets_while_a_specific_brush_is_chosen_stays_with_it_and_not_with_the_users_brush() {
    let mut b = banc();
    let [_, gris, _] = une_de_chaque(&mut b);
    let pinceau = pinceau_de_l_utilisateur(&mut b);
    b.app.dediee_ecrire(gris, Some(feutre(40.0)));
    b.app.perso_choisir(gris);
    b.chauffer(2);
    // Taille aux crochets, panneau Réglages de l'outil : c'est le réglage de la brosse spécifique.
    b.app.brush.size = 77.0;
    b.chauffer(2);
    assert_eq!(b.app.dediee_specifique(gris).map(|s| s.settings.size), Some(77.0), "gardé pour la brosse spécifique");
    // Pas de fuite vers le pinceau retenu par sorte de calque.
    assert_ne!(b.app.pinceaux.pinceau_vecto.map(|p| p.size), Some(77.0));
    assert_ne!(b.app.pinceaux.pinceau_bitmap.map(|p| p.size), Some(77.0));
    b.app.perso_lacher();
    b.chauffer(2);
    assert_eq!(b.app.brush, pinceau, "le pinceau de l'utilisateur est revenu, sans le 77");
    // Et dans l'autre sens : un changement fait au pinceau de l'utilisateur ne passe pas dans la brosse spécifique.
    b.app.brush.size = 9.0;
    b.chauffer(2);
    assert_eq!(b.app.dediee_specifique(gris).map(|s| s.settings.size), Some(77.0));
    b.app.perso_choisir(gris);
    b.chauffer(2);
    assert_eq!(b.app.brush.size, 77.0, "la brosse spécifique retrouve son réglage à elle");
    b.app.perso_lacher();
    assert_eq!(b.app.brush.size, 9.0, "et l'utilisateur son pinceau, tel qu'il l'avait changé entre-temps");
}

#[test]
fn editing_the_specific_brush_while_it_is_chosen_changes_the_drawing_and_leaves_the_users_brush_aside() {
    let mut b = banc();
    let [_, _, decor] = une_de_chaque(&mut b);
    let pinceau = pinceau_de_l_utilisateur(&mut b);
    b.app.dediee_ecrire(decor, Some(feutre(40.0)));
    b.app.perso_choisir(decor);
    b.chauffer(2);
    // L'éditeur passe la brosse du feutre au crayon : le pinceau qu'elle tenait est rendu, le crayon est pris.
    b.app.dediee_ecrire(decor, Some(Specifique { crayon: true, settings: BrushSettings { size: 5.0, ..BrushSettings::graphite() }, nom: Some("Crayon".into()) }));
    b.chauffer(2);
    assert_eq!(b.app.tool, Tool::Pencil);
    assert_eq!(b.app.brush, pinceau, "le pinceau de l'utilisateur est rendu dès que la brosse change d'outil");
    assert_eq!(b.app.pencil.size, 5.0);
    // Revenir à « Suit le pinceau » : tout est rendu.
    b.app.dediee_ecrire(decor, None);
    b.chauffer(2);
    assert_eq!((b.app.brush, b.app.pencil.size), (pinceau, 3.0));
    assert!(b.app.perso.pris.is_none());
    assert_eq!(b.app.tool, Tool::Brush, "l'outil d'avant");
}

#[test]
fn a_character_specific_brush_is_saved_with_the_project_and_undone_and_an_old_file_reads_as_following() {
    let mut b = banc();
    let lou = personnage(&mut b, "Lou", [200, 30, 30], Some([250, 200, 200]));
    b.app.perso_lacher();
    let avant = b.app.history.undo_len();
    b.app.perso_etape("Modifier la brosse du personnage");
    b.app.dediee_ecrire(Choix::Personnage(lou), Some(feutre(33.0)));
    assert_eq!(b.app.history.undo_len(), avant + 1);
    // Enregistré avec les personnages, relu à l'identique (le format ne change pas).
    let octets = personnages::to_bytes(&b.app.doc).expect("écriture").expect("des personnages");
    let relus = personnages::parse(std::str::from_utf8(&octets).expect("du texte")).expect("relecture");
    assert_eq!(relus.iter().find(|p| p.id == lou).and_then(|p| p.brosse.clone()), Some(feutre(33.0)));
    // Annulé : le personnage suit de nouveau le pinceau.
    b.app.undo();
    assert_eq!(b.app.dediee_specifique(Choix::Personnage(lou)), None);
    // Un fichier d'avant : pas de champ « brosse » : il suit le pinceau.
    let ancien = r#"{"format":"shot-personnages","version":1,"personnages":[{"id":7,"nom":"Lou","trait":[200,30,30],"fond":[250,200,200]}]}"#;
    assert_eq!(personnages::parse(ancien).expect("ancien fichier")[0].brosse, None);
    assert!(crate::project::VERSION >= 42, "le format du projet ne change pas pour cela");
    // Un personnage qui suit le pinceau n'écrit rien de plus dans son fichier.
    assert!(!String::from_utf8(personnages::to_bytes(&b.app.doc).expect("écriture").expect("des personnages")).expect("du texte").contains("brosse"));
}

#[test]
fn old_decor_settings_follow_the_pinceau_when_untouched_and_become_specific_when_set_by_hand() {
    // Trois Décors d'avant le §107 : Décor 1 resté au crayon de 5 px, Décor 2 réglé à la main (feutre de 40 px), « Mon décor » : pinceau plein de 90 px.
    let ancien = "brosses_decor = [\
        { nom = \"Décor 1\", trait = [110, 110, 110], fond = [200, 200, 200], pinceau = \"Crayon\", taille = 5.0 },\
        { nom = \"Décor 2\", trait = [142, 142, 142], fond = [216, 216, 216], pinceau = \"Feutre\", taille = 40.0 },\
        { nom = \"Mon décor\", trait = [1, 2, 3], pinceau = \"Aplat\", taille = 90.0 }]\n";
    let s = Settings::apres_lecture(toml::from_str(ancien).expect("d'anciens réglages"));
    let d = &s.brosses_decor;
    assert_eq!(d.len(), 3, "rien ne se perd");
    assert!(d[0].specifique.is_none(), "resté à celui d'origine : il suit le pinceau");
    assert!(d[2].specifique.is_none(), "le pinceau plein de 90 px est celui d'origine : il suit le pinceau");
    let sp = d[1].specifique.as_ref().expect("réglé à la main : brosse spécifique");
    assert_eq!((sp.crayon, sp.settings.size), (false, 40.0), "il garde son pinceau et sa taille");
    assert!(d.iter().all(|b| b.pinceau.is_none() && b.taille == 0.0), "les anciens champs sont vidés");
    assert_eq!(d[2].name, "Mon décor");
    // Écrit puis relu : la même chose, et la forme nouvelle (le champ « brosse »).
    let texte = toml::to_string(&s).expect("écriture");
    assert!(texte.contains("brosse") && !texte.contains("pinceau = \""), "{texte}");
    let relu = Settings::apres_lecture(toml::from_str(&texte).expect("relecture"));
    assert_eq!(relu.brosses_decor, s.brosses_decor);
    // Les classiques portent aussi une brosse spécifique, écrite et relue.
    let mut s = s;
    s.brosses_classiques[0].specifique = Some(feutre(12.0));
    let relu = Settings::apres_lecture(toml::from_str(&toml::to_string(&s).expect("écriture")).expect("relecture"));
    assert_eq!(relu.brosses_classiques[0].specifique, Some(feutre(12.0)));
    assert_eq!(relu.brosses_classiques[1].specifique, None);
}

#[test]
fn the_bubble_and_the_panel_say_what_draws() {
    let mut b = banc_taille(Document::new(400, 300, 24), false, 1600, 2400);
    b.app.set_tool(Tool::Brush);
    b.garder_textes = true;
    b.app.show_pane(crate::dock::Pane::Personnages);
    b.chauffer(4);
    assert_eq!(b.app.dediee_etiquette(Choix::Classique(0)), "Suit le pinceau");
    assert!(b.textes.iter().any(|x| x.contains("Suit le pinceau")), "le panneau le dit : {:?}", b.textes.iter().take(60).collect::<Vec<_>>());
    assert!(b.textes.iter().any(|x| x.contains("Brosses dédiées")), "le panneau a son nouveau nom");
    b.app.dediee_ecrire(Choix::Classique(0), Some(Specifique { nom: Some("Feutre noir".into()), ..feutre(20.0) }));
    assert_eq!(b.app.dediee_etiquette(Choix::Classique(0)), "Feutre noir, 20 px, pinceau");
    assert_eq!(b.app.dediee_outil(Choix::Classique(0)), Some(false));
    assert_eq!(b.app.dediee_outil(Choix::Classique(1)), None);
    b.chauffer(3);
    assert!(b.textes.iter().any(|x| x.contains("Brosse spécifique")), "le panneau le dit : {:?}", b.textes.iter().take(60).collect::<Vec<_>>());
    assert_eq!(crate::dock::Pane::Personnages.name(), "Brosses dédiées");
}
