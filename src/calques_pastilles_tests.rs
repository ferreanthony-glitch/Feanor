//! Les pastilles créent leurs calques (`calquespastilles.rs`, `decor.rs`, `decorui.rs`) : un calque vectoriel par personnage, un seul
//! calque Décor par plan (vectoriel d'origine, bitmap si les Préférences le demandent), la pastille « classique » qui ne crée rien.

use eframe::egui::{vec2, Color32, Event, Modifiers, PointerButton, Pos2};

use crate::app::Tool;
use crate::decor::Genre;
use crate::document::{CelId, Document, Layer, LayerKind};
use crate::perf_tests::{banc_taille, Banc};
use crate::personnagesui::{Case, Choix};
use crate::render;
use crate::vignettes_tests::{case_vide, glisser, ouvert, pas_d_annulation, quatre_plans, vague, vignette};
use crate::workspace::Espace;

const ROUGE: [u8; 3] = [200, 30, 30];
const ROSE: [u8; 3] = [250, 200, 200];
const BLEU: [u8; 3] = [30, 40, 200];
const CIEL: [u8; 3] = [190, 210, 250];

// ------------------------------------------------------------------------------------------ outils d'essai

/// Le dessin du Storyboard, sur une feuille de 400 par 300, l'outil Pinceau.
fn banc() -> Banc {
    crate::fxcache::monde::ecrire();
    let mut b = banc_taille(Document::new(400, 300, 24), false, 1600, 900);
    b.app.tool = Tool::Brush;
    b.chauffer(6);
    assert_eq!(b.app.espace, Espace::Storyboard);
    assert!(!b.app.board_shown(), "le dessin, pas la planche");
    b
}

fn appui(p: Pos2, enfonce: bool) -> Event {
    Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: enfonce, modifiers: Modifiers::NONE }
}

fn clic(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![appui(p, true)], 1.0 / 60.0);
    b.image(vec![appui(p, false)], 1.0 / 60.0);
    b.image(vec![Event::PointerGone], 1.0 / 60.0);
    b.chauffer(3);
}

fn case(b: &Banc, c: Case) -> Pos2 {
    b.app.perso.cases.iter().find(|(_, k)| *k == c).map(|(r, _)| r.center()).unwrap_or_else(|| panic!("la case {c:?} n'est pas dans la rangée : {:?}", b.app.perso.cases))
}

fn clic_case(b: &mut Banc, c: Case) {
    let p = case(b, c);
    clic(b, p);
}

/// Un trait au milieu du dessin, de gauche à droite, avec un décalage vertical (`dy`) pour que deux traits ne se confondent pas.
fn trait_au_milieu(b: &mut Banc, dy: f32) {
    let c = b.app.canvas_rect.center() + vec2(0.0, dy);
    let (a, z) = (c - vec2(60.0, 0.0), c + vec2(60.0, 10.0));
    b.image(vec![Event::PointerMoved(a)], 1.0 / 60.0);
    b.image(vec![appui(a, true)], 1.0 / 60.0);
    for k in 1..=10 {
        b.image(vec![Event::PointerMoved(a + (z - a) * (k as f32 / 10.0))], 1.0 / 60.0);
    }
    b.image(vec![appui(z, false)], 1.0 / 60.0);
    b.image(vec![Event::PointerGone], 1.0 / 60.0);
    b.chauffer(3);
}

/// Un personnage de plus, choisi : son nom, sa couleur de trait et sa couleur d'aplat.
fn personnage(b: &mut Banc, nom: &str, ligne: [u8; 3], fond: Option<[u8; 3]>) -> u64 {
    let id = b.app.perso_ajouter().expect("un personnage de plus");
    b.app.perso_ecrire(Choix::Personnage(id), nom.into(), ligne, fond);
    b.app.perso_choisir(Choix::Personnage(id));
    id
}

/// Le calque `li` du plan courant a de l'encre : un pixel peint, ou un trait vectoriel.
fn encre(b: &Banc, li: usize) -> bool {
    let doc = &b.app.doc;
    doc.panels[b.app.cur_panel].layers[li].frames.iter().flatten().any(|c| a_de_l_encre(doc, *c))
}

fn a_de_l_encre(doc: &Document, cel: CelId) -> bool {
    doc.cels.get(&cel).is_some_and(|c| c.raster.tiles.iter().flatten().any(|t| t.iter().any(|p| p.a() != 0)) || c.vector.as_ref().is_some_and(|v| !v.elements.is_empty()))
}

/// Les calques du plan `i` qui sont celui du personnage `id`.
fn calques_de(doc: &Document, i: usize, id: u64) -> Vec<usize> {
    doc.panels[i].layers.iter().enumerate().filter(|(_, l)| l.personnage == Some(id)).map(|(k, _)| k).collect()
}

fn dossier(nom: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-pastilles-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier d'essai");
    dir
}

/// Un plan de six images en deux cases (une marque à l'image 3), un dessin par case.
fn une_continuite() -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.set_duration(0, 6);
    doc.toggle_mark(0, 3);
    for (de, a) in [(0, 3), (3, 6)] {
        let cel = doc.new_cel(LayerKind::Bitmap);
        for f in de..a {
            doc.panels[0].layers[0].set(f, Some(cel));
        }
    }
    doc
}

// ------------------------------------------------------------------------------------------ le calque d'un personnage

#[test]
fn the_first_stroke_with_a_character_pastille_makes_a_vector_layer_with_its_name_and_its_fill() {
    let mut b = banc();
    let lou = personnage(&mut b, "Lou", ROUGE, Some(ROSE));
    let (couches, ordinaire) = (b.app.layers().len(), b.app.layers()[b.app.cur_layer].id);
    let avant = pas_d_annulation(&b);
    assert!(calques_de(&b.app.doc, 0, lou).is_empty(), "choisir ne crée rien");
    assert_eq!(pas_d_annulation(&b), avant);
    // L'appui : le calque est là dans la même image d'écran, le trait est commencé (rien n'attend un calcul).
    let c = b.app.canvas_rect.center();
    b.image(vec![Event::PointerMoved(c - vec2(60.0, 0.0))], 1.0 / 60.0);
    b.image(vec![appui(c - vec2(60.0, 0.0), true)], 1.0 / 60.0);
    assert_eq!(b.app.layers().len(), couches + 1, "le calque naît à l'appui");
    assert!(b.app.stroke.is_some(), "le trait est commencé sur lui");
    for k in 1..=10 {
        b.image(vec![Event::PointerMoved(c - vec2(60.0, 0.0) + vec2(12.0 * k as f32, 0.0))], 1.0 / 60.0);
    }
    b.image(vec![appui(c + vec2(60.0, 0.0), false)], 1.0 / 60.0);
    b.image(vec![Event::PointerGone], 1.0 / 60.0);
    b.chauffer(3);
    b.app.perso_attendre();
    let rang = calques_de(&b.app.doc, 0, lou);
    assert_eq!(rang.len(), 1, "un seul calque pour Lou");
    let l = &b.app.layers()[rang[0]];
    assert_eq!((l.name.as_str(), l.kind), ("Lou", LayerKind::Vector), "vectoriel, au nom du personnage");
    assert_eq!(l.fond.map(|f| f.couleur), Some(ROSE), "son aplat est allumé, à la couleur d'aplat du personnage");
    assert_eq!(b.app.cur_layer, rang[0], "c'est le calque courant");
    assert_eq!(rang[0], 1, "juste au-dessus du calque qui était courant");
    assert_eq!(b.app.layers()[0].id, ordinaire, "l'autre calque n'a pas bougé");
    assert!(encre(&b, rang[0]), "le trait est dans le calque de Lou");
    assert!(!encre(&b, 0), "pas dans l'ancien calque courant");
    assert_eq!(pas_d_annulation(&b), avant + 1, "le calque et le trait ne font qu'une étape d'annulation");
    assert!(b.app.history.names().0.last().is_some_and(|n| n.contains("Lou")), "{:?}", b.app.history.names().0.last());
}

#[test]
fn the_second_stroke_goes_into_the_same_layer_and_a_pastille_without_aplat_makes_a_layer_without() {
    let mut b = banc();
    let lou = personnage(&mut b, "Lou", ROUGE, Some(ROSE));
    trait_au_milieu(&mut b, -40.0);
    let avant = pas_d_annulation(&b);
    let couches = b.app.layers().len();
    trait_au_milieu(&mut b, 40.0);
    assert_eq!(b.app.layers().len(), couches, "pas de calque de plus");
    assert_eq!(calques_de(&b.app.doc, 0, lou).len(), 1);
    assert_eq!(pas_d_annulation(&b), avant + 1, "un trait de plus, une étape de plus");
    // Un personnage sans aplat : son calque n'en a pas.
    let fantome = personnage(&mut b, "Fantôme", BLEU, None);
    trait_au_milieu(&mut b, 0.0);
    let rang = calques_de(&b.app.doc, 0, fantome);
    assert_eq!(rang.len(), 1);
    assert!(b.app.layers()[rang[0]].fond.is_none(), "sans couleur d'aplat, pas d'aplat");
    assert_eq!(b.app.layers().len(), couches + 1);
    // Deux personnages, deux calques : chacun ses traits.
    assert!(encre(&b, rang[0]) && encre(&b, calques_de(&b.app.doc, 0, lou)[0]), "chacun a ses traits");
}

#[test]
fn the_layer_and_its_first_stroke_are_one_undo_step_and_redo_brings_both_back() {
    let mut b = banc();
    let lou = personnage(&mut b, "Lou", ROUGE, Some(ROSE));
    let (couches, avant) = (b.app.layers().len(), pas_d_annulation(&b));
    trait_au_milieu(&mut b, 0.0);
    assert_eq!(pas_d_annulation(&b), avant + 1);
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(pas_d_annulation(&b), avant, "une seule annulation défait le calque et le trait");
    assert_eq!(b.app.layers().len(), couches, "le calque est parti avec son premier trait");
    assert!(calques_de(&b.app.doc, 0, lou).is_empty());
    b.app.run_action("edition.retablir");
    b.chauffer(2);
    let rang = calques_de(&b.app.doc, 0, lou);
    assert_eq!(rang.len(), 1, "rétablir rend le calque");
    assert!(encre(&b, rang[0]), "avec son trait");
}

#[test]
fn a_click_that_draws_nothing_leaves_no_layer_and_the_eraser_and_neutral_colours_create_none() {
    let mut b = banc();
    let lou = personnage(&mut b, "Lou", ROUGE, Some(ROSE));
    let (couches, avant) = (b.app.layers().len(), pas_d_annulation(&b));
    // La gomme n'est pas un trait de personnage : elle agit sur le calque courant.
    b.app.set_tool(Tool::Eraser);
    trait_au_milieu(&mut b, 0.0);
    assert_eq!(b.app.layers().len(), couches, "la gomme ne crée rien");
    b.app.set_tool(Tool::Brush);
    // Une couleur neutre n'est pas un personnage : son trait va dans le calque courant.
    b.app.perso_choisir(Choix::Neutre(0));
    trait_au_milieu(&mut b, 0.0);
    assert_eq!(b.app.layers().len(), couches, "une couleur neutre ne crée pas de calque");
    assert!(calques_de(&b.app.doc, 0, lou).is_empty());
    assert_eq!(pas_d_annulation(&b), avant + 1, "son trait est une étape, rien d'autre");
}

#[test]
fn choosing_a_character_that_has_its_layer_makes_it_current_and_each_plan_gets_its_own() {
    let mut b = banc();
    let lou = personnage(&mut b, "Lou", ROUGE, Some(ROSE));
    trait_au_milieu(&mut b, 0.0);
    let rang = calques_de(&b.app.doc, 0, lou)[0];
    // Un autre calque devient courant, puis la pastille de Lou le rend à Lou.
    b.app.select_layer(0);
    assert_eq!(b.app.cur_layer, 0);
    b.app.perso_choisir(Choix::Personnage(lou));
    assert_eq!(b.app.cur_layer, rang, "la pastille rend son calque courant");
    let couches = b.app.layers().len();
    clic_case(&mut b, Case::Classique);
    b.app.select_layer(0);
    clic_case(&mut b, Case::Choix(Choix::Personnage(lou)));
    assert_eq!(b.app.cur_layer, rang, "par un clic sur la pastille aussi");
    assert_eq!(b.app.layers().len(), couches, "et sans rien créer");
    // Un trait qui part avec un autre calque courant va quand même dans celui de Lou, tant que sa pastille est choisie.
    b.app.select_layer(0);
    let n = b.app.layers().len();
    trait_au_milieu(&mut b, 30.0);
    assert_eq!((b.app.layers().len(), b.app.cur_layer), (n, rang));
    // Un autre plan : Lou n'y a pas de calque, le premier trait le crée.
    b.app.run_action("panneau.nouveau");
    b.chauffer(3);
    assert!(b.app.doc.panels.len() > 1, "un plan de plus");
    let p = b.app.cur_panel;
    assert!(b.app.doc.calque_de_personnage(p, lou).is_none());
    b.app.perso_choisir(Choix::Personnage(lou));
    trait_au_milieu(&mut b, 0.0);
    assert_eq!(calques_de(&b.app.doc, p, lou).len(), 1, "le calque de Lou dans le plan où le trait a commencé");
    assert_eq!(calques_de(&b.app.doc, 0, lou).len(), 1, "et le premier plan garde le sien");
}

#[test]
fn renaming_a_character_renames_its_layers_but_not_one_that_was_renamed_by_hand() {
    let mut b = banc();
    let lou = personnage(&mut b, "Lou", ROUGE, Some(ROSE));
    trait_au_milieu(&mut b, 0.0);
    let max = personnage(&mut b, "Max", BLEU, Some(CIEL));
    trait_au_milieu(&mut b, 40.0);
    let (a, c) = (calques_de(&b.app.doc, 0, lou)[0], calques_de(&b.app.doc, 0, max)[0]);
    assert_eq!((b.app.layers()[a].name.as_str(), b.app.layers()[c].name.as_str()), ("Lou", "Max"));
    // Le panneau du personnage : le nom change, ses calques suivent (le numéro, lui, ne change pas).
    b.app.perso_ecrire(Choix::Personnage(lou), "Léa".into(), ROUGE, Some(ROSE));
    assert_eq!(b.app.layers()[a].name, "Léa", "le calque suit le nom du personnage");
    assert_eq!(b.app.layers()[c].name, "Max", "l'autre n'a pas bougé");
    assert_eq!(b.app.layers()[a].personnage, Some(lou), "retrouvé par son numéro");
    assert_eq!(b.app.doc.calque_de_personnage(0, lou), Some(a));
    // Un calque qu'on a soi-même rebaptisé garde son nom.
    b.app.layers_mut()[c].name = "Max de dos".into();
    b.app.perso_ecrire(Choix::Personnage(max), "Maxime".into(), BLEU, Some(CIEL));
    assert_eq!(b.app.layers()[c].name, "Max de dos");
    // La pastille retrouve le calque renommé, même si son nom ne dit plus rien.
    b.app.select_layer(0);
    b.app.perso_choisir(Choix::Personnage(max));
    assert_eq!(b.app.cur_layer, c);
}

#[test]
fn renaming_reaches_the_layers_of_every_plan_and_one_undo_step_gives_the_old_names_back() {
    let mut b = banc();
    let lou = personnage(&mut b, "Lou", ROUGE, Some(ROSE));
    trait_au_milieu(&mut b, 0.0);
    b.app.run_action("panneau.nouveau");
    b.chauffer(3);
    assert!(b.app.doc.panels.len() > 1);
    b.app.perso_choisir(Choix::Personnage(lou));
    trait_au_milieu(&mut b, 0.0);
    let noms = |b: &Banc| -> Vec<String> { (0..b.app.doc.panels.len()).flat_map(|i| b.app.doc.panels[i].layers.iter().filter(|l| l.personnage == Some(lou)).map(|l| l.name.clone()).collect::<Vec<_>>()).collect() };
    assert_eq!(noms(&b), ["Lou", "Lou"]);
    // Comme le champ du panneau : une étape d'annulation, puis le changement.
    b.app.record_step();
    b.app.perso_ecrire(Choix::Personnage(lou), "Léa".into(), ROUGE, Some(ROSE));
    assert_eq!(noms(&b), ["Léa", "Léa"]);
    b.app.undo();
    assert_eq!(noms(&b), ["Lou", "Lou"], "l'annulation rend le nom du personnage et ceux de ses calques");
    assert_eq!(b.app.doc.personnages[0].name, "Lou");
}

#[test]
fn a_duplicated_layer_is_not_the_character_layer_and_the_character_survives_the_file() {
    let mut b = banc();
    let lou = personnage(&mut b, "Lou", ROUGE, Some(ROSE));
    trait_au_milieu(&mut b, 0.0);
    let rang = calques_de(&b.app.doc, 0, lou)[0];
    b.app.select_layer(rang);
    b.app.duplicate_layer();
    assert_eq!(calques_de(&b.app.doc, 0, lou), vec![rang], "la copie n'est plus « le » calque du personnage");
    // Le fichier : le numéro du personnage revient avec le calque ; un projet sans personnage n'écrit rien de plus.
    let dir = dossier("fichier");
    let manifeste = dir.join("P.pelure");
    crate::project::save(&b.app.doc, &manifeste, &mut Default::default()).expect("enregistrement");
    let texte = std::fs::read_to_string(&manifeste).unwrap();
    assert!(texte.contains("\"personnage\""), "le calque du personnage est marqué dans le manifeste");
    let relu = crate::project::load(&manifeste).expect("réouverture");
    assert_eq!(calques_de(&relu.doc, 0, lou), vec![rang]);
    assert_eq!(relu.doc.panels[0].layers[rang].name, "Lou");
    assert_eq!(relu.doc.panels[0].layers[rang].fond.map(|f| f.couleur), Some(ROSE));
    assert_eq!(relu.doc.panels[0].layers[rang].kind, LayerKind::Vector);
    let sans = dir.join("Sans.pelure");
    crate::project::save(&Document::new(64, 48, 24), &sans, &mut Default::default()).unwrap();
    assert!(!std::fs::read_to_string(&sans).unwrap().contains("personnage"), "aucun champ de plus pour un projet sans personnage");
    let ancien = crate::project::load(&sans).expect("un projet d'avant s'ouvre");
    assert!(ancien.doc.panels.iter().all(|p| p.layers.iter().all(|l| l.personnage.is_none())));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_classic_pastille_draws_in_the_current_layer_and_creates_nothing() {
    let mut b = banc();
    let lou = personnage(&mut b, "Lou", ROUGE, Some(ROSE));
    trait_au_milieu(&mut b, 0.0);
    let rang = calques_de(&b.app.doc, 0, lou)[0];
    b.app.select_layer(0);
    let (couches, avant) = (b.app.layers().len(), pas_d_annulation(&b));
    clic_case(&mut b, Case::Classique);
    assert_eq!(b.app.perso.actif, None);
    assert_eq!(b.app.cur_layer, 0, "classique ne change pas de calque");
    trait_au_milieu(&mut b, 0.0);
    assert_eq!(b.app.layers().len(), couches, "aucun calque créé");
    assert_eq!(b.app.cur_layer, 0);
    assert!(encre(&b, 0), "le trait est dans le calque courant");
    assert_eq!(pas_d_annulation(&b), avant + 1);
    assert!(calques_de(&b.app.doc, 0, lou) == vec![rang]);
    assert!(b.app.doc.panels[0].layers.iter().all(|l| !l.decor), "pas de décor non plus");
}

#[test]
fn the_character_layer_is_made_from_the_vignettes_board() {
    // La planche : un trait dans la case d'un plan, avec la pastille d'un personnage.
    let mut b = ouvert(quatre_plans(), 4);
    let id = b.app.perso_ajouter().expect("personnage");
    b.app.perso_ecrire(Choix::Personnage(id), "Lou".into(), ROUGE, Some(ROSE));
    b.app.perso_choisir(Choix::Personnage(id));
    let (couches, n) = (b.app.doc.panels[1].layers.len(), pas_d_annulation(&b));
    let v = vignette(&b, 3);
    glisser(&mut b, &vague(v.center() - vec2(30.0, 0.0), v.center() + vec2(30.0, 0.0), 10));
    assert_eq!(b.app.cur_panel, 1);
    let rang = calques_de(&b.app.doc, 1, id);
    assert_eq!(rang.len(), 1, "le calque de Lou est dans le plan de la case");
    assert_eq!(b.app.doc.panels[1].layers.len(), couches + 1);
    let l = &b.app.doc.panels[1].layers[rang[0]];
    assert_eq!((l.name.as_str(), l.kind, l.fond.map(|f| f.couleur)), ("Lou", LayerKind::Vector, Some(ROSE)));
    assert!(encre(&b, rang[0]));
    assert_eq!(pas_d_annulation(&b), n + 1, "une seule étape pour le calque et le trait");
    // Une case vide : le plan neuf est fait, avec le calque de Lou et le trait, en une seule étape.
    let (plans, n) = (b.app.doc.panels.len(), pas_d_annulation(&b));
    let e = case_vide(&b, 0);
    glisser(&mut b, &vague(e.center() - vec2(20.0, 0.0), e.center() + vec2(20.0, 5.0), 10));
    assert_eq!(b.app.doc.panels.len(), plans + 1, "le plan est fait à l'appui");
    assert_eq!(calques_de(&b.app.doc, plans, id).len(), 1, "avec le calque de Lou");
    assert_eq!(pas_d_annulation(&b), n + 1, "le plan, le calque et le trait ne font qu'une étape");
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(b.app.doc.panels.len(), plans);
}

#[test]
fn a_chosen_character_draws_in_its_own_layer_in_the_animation_too() {
    // L'animation : la barre des pastilles y est cachée d'office, mais un personnage choisi y dessine aussi dans son calque.
    let mut a = banc();
    a.app.set_espace(Espace::Animation);
    a.chauffer(4);
    let lou = personnage(&mut a, "Lou", ROUGE, Some(ROSE));
    let (couches, avant) = (a.app.layers().len(), pas_d_annulation(&a));
    trait_au_milieu(&mut a, 0.0);
    assert_eq!(calques_de(&a.app.doc, 0, lou).len(), 1);
    assert_eq!(a.app.layers().len(), couches + 1);
    assert_eq!(pas_d_annulation(&a), avant + 1);
}

// ------------------------------------------------------------------------------------------ le calque Décor

#[test]
fn the_decor_layer_is_vector_by_default_and_bitmap_when_the_setting_asks() {
    for bitmap in [false, true] {
        let mut doc = une_continuite();
        doc.decor_bitmap = bitmap;
        let (at, change) = doc.assurer_decor(0, 4).expect("le plan existe");
        assert!(change);
        let l = &doc.panels[0].layers[at];
        assert_eq!(l.kind, if bitmap { LayerKind::Bitmap } else { LayerKind::Vector });
        assert!(l.decor && l.name == "Décor" && at == 0, "un calque Décor, tout en bas");
        let cel = l.cel_at(0).expect("un dessin");
        assert_eq!(l.frames, vec![Some(cel); 6], "une clé tenue sur tout le plan");
        assert_eq!(doc.cels[&cel].vector.is_some(), !bitmap, "le dessin est de la sorte du calque");
    }
    // D'origine : vectoriel (le modèle, les réglages).
    assert!(!Document::new(10, 10, 24).decor_bitmap);
    assert!(!crate::vignettesreglages::ReglagesVignettes::default().decor_bitmap);
}

#[test]
fn an_old_bitmap_decor_stays_bitmap_and_gets_its_new_drawings_in_bitmap_whatever_the_setting() {
    let mut doc = une_continuite();
    doc.report_decor = false;
    doc.decor_bitmap = true;
    let (at, _) = doc.assurer_decor(0, 4).unwrap();
    // Le réglage change ensuite : le décor qui existe garde sa sorte, et un dessin de plus (la case d'avant) est bitmap aussi.
    doc.decor_bitmap = false;
    let (at2, change) = doc.assurer_decor(0, 1).unwrap();
    assert_eq!(at2, at);
    assert!(change, "la première case n'avait pas de décor : un dessin neuf");
    let l = &doc.panels[0].layers[at];
    assert_eq!(l.kind, LayerKind::Bitmap, "le décor reste bitmap");
    assert!(l.frames.iter().flatten().all(|c| doc.cels[c].vector.is_none()), "ses dessins neufs aussi");
    assert_eq!(doc.panels[0].layers.iter().filter(|l| l.decor).count(), 1, "toujours un seul calque Décor");
    // La jonction avec un plan dont le décor est vectoriel ne mêle pas les sortes et ne perd rien.
    let mut doc = Document::with_frame(160, 90, 24);
    doc.add_scene_after(0);
    doc.into_plans();
    for i in 0..2 {
        doc.set_duration(i, 6);
    }
    doc.decor_bitmap = true;
    doc.assurer_decor(0, 0).unwrap();
    doc.decor_bitmap = false;
    doc.assurer_decor(1, 0).unwrap();
    let vectoriel = doc.panels[1].layers[doc.decor_de(1).unwrap()].cel_at(0).unwrap();
    doc.report_decor = false;
    doc.join_next_plan(0).expect("réunis");
    let p = &doc.panels[0];
    assert_eq!(p.layers.iter().filter(|l| l.decor).count(), 1, "un seul calque Décor");
    let d = &p.layers[doc.decor_de(0).unwrap()];
    assert_eq!(d.kind, LayerKind::Bitmap);
    assert!(d.frames.iter().flatten().all(|c| doc.cels[c].vector.is_none()), "pas de dessin vectoriel dans le calque bitmap");
    assert!(p.layers.iter().any(|l| !l.decor && l.kind == LayerKind::Vector && l.shown_at(8) == Some(vectoriel)), "le décor du second plan est gardé, à part");
}

#[test]
fn the_vector_decor_is_shared_by_the_cases_of_a_continuity_like_the_bitmap_one() {
    let mut doc = une_continuite();
    let (at, _) = doc.assurer_decor(0, 4).unwrap();
    let cel = doc.panels[0].layers[at].cel_at(0).unwrap();
    assert!(doc.cels[&cel].vector.is_some());
    // Un trait de décor dans la seconde case : un élément dans l'unique dessin, que toutes les images du plan montrent.
    doc.cels.get_mut(&cel).and_then(|c| c.vector.as_mut()).expect("vectoriel").elements.push(crate::vector::Element::Stroke {
        points: vec![crate::vector::VPoint { x: 20.0, y: 40.0, r: 2.0 }, crate::vector::VPoint { x: 120.0, y: 42.0, r: 2.0 }],
        color: [0x8F, 0xA3, 0xB8],
        opacity: 1.0,
        hardness: 1.0,
        brush: None,
        trace: Default::default(),
    });
    for f in 0..6 {
        let plan = render::panel_plan_at(&doc, 0, f, None);
        assert!(plan.items.iter().any(|i| i.cel == cel), "le décor est dans l'image {f}");
    }
    // Une case de plus (clés de case) : le décor ne reçoit pas de copie, il reste un seul dessin.
    doc.toggle_mark(0, 5);
    doc.cles_de_case(0, 5);
    assert_eq!(doc.panels[0].layers[at].frames, vec![Some(cel); 6]);
    // Une coupe cochée donne au plan suivant sa propre copie, vectorielle aussi.
    let suite = doc.split_plan(0, 3).expect("le plan se coupe");
    let copie = doc.panels[suite].layers[doc.decor_de(suite).expect("son décor")].cel_at(0).unwrap();
    assert_ne!(copie, cel, "une copie indépendante");
    assert!(doc.cels[&copie].vector.as_ref().is_some_and(|v| v.elements.len() == 1), "qui garde le trait");
}

#[test]
fn the_vector_decor_is_held_through_the_last_frame_of_a_camera_move_and_follows_a_longer_plan() {
    use crate::camera::CameraKey;
    let mut doc = une_continuite();
    let a = doc.default_camera();
    doc.panels[0].camera = vec![CameraKey::new(0, a), CameraKey::new(2, a.shifted(40.0, 0.0))];
    let (at, _) = doc.assurer_decor(0, 4).unwrap();
    let cel = doc.panels[0].layers[at].cel_at(0).unwrap();
    assert!(doc.cels[&cel].vector.is_some());
    // La deuxième case (images 3 à 5) voit à travers le dernier cadre de la caméra, et y voit le décor.
    let b = doc.panel_camera(0, 2);
    for f in 3..6 {
        assert_eq!(doc.panel_camera(0, f).x, b.x, "l'image {f} voit à travers B");
        assert!(render::panel_plan_at(&doc, 0, f, None).items.iter().any(|i| i.cel == cel));
    }
    // Un plan plus long : le décor suit (« Maintenir »).
    doc.set_duration(0, 10);
    for f in 0..10 {
        assert_eq!(doc.panels[0].layers[at].shown_at(f), Some(cel), "image {f}");
    }
}

#[test]
fn decor_brushes_make_one_vector_decor_layer_per_plan_shared_by_all_of_them_in_one_undo_step() {
    let mut b = banc();
    let (couches, avant) = (b.app.layers().len(), pas_d_annulation(&b));
    clic_case(&mut b, Case::Decor(Genre::Feutre));
    trait_au_milieu(&mut b, -40.0);
    let at = b.app.doc.decor_de(0).expect("le calque Décor est né au premier trait");
    let l = &b.app.layers()[at];
    assert_eq!((at, l.kind, l.name.as_str()), (0, LayerKind::Vector, "Décor"), "vectoriel, tout en bas du plan");
    assert_eq!(b.app.layers().len(), couches + 1);
    assert!(encre(&b, at));
    assert_eq!(pas_d_annulation(&b), avant + 1, "le calque et le premier trait ne font qu'une étape");
    // Une autre brosse du décor : le même calque.
    clic_case(&mut b, Case::Decor(Genre::Crayon));
    trait_au_milieu(&mut b, 0.0);
    clic_case(&mut b, Case::Decor(Genre::Aplat));
    trait_au_milieu(&mut b, 40.0);
    assert_eq!(b.app.doc.panels[0].layers.iter().filter(|l| l.decor).count(), 1, "un seul calque Décor pour toutes les brosses");
    assert_eq!(b.app.layers().len(), couches + 1);
    assert_eq!(pas_d_annulation(&b), avant + 3, "un trait, une étape");
    let cel = b.app.layers()[at].cel_at(0).unwrap();
    assert_eq!(b.app.layers()[at].frames, vec![Some(cel); b.app.doc.panels[0].frame_count()], "une clé tenue sur tout le plan");
    // Annuler les trois traits : le calque part avec le premier.
    for _ in 0..3 {
        b.app.run_action("edition.annuler");
    }
    b.chauffer(2);
    assert_eq!(b.app.doc.decor_de(0), None);
    assert_eq!(pas_d_annulation(&b), avant);
}

#[test]
fn with_the_setting_the_decor_layer_is_made_bitmap_and_changing_it_later_leaves_the_existing_one() {
    let mut b = banc();
    b.app.settings.vignettes.decor_bitmap = true;
    clic_case(&mut b, Case::Decor(Genre::Feutre));
    trait_au_milieu(&mut b, 0.0);
    let at = b.app.doc.decor_de(0).expect("le calque Décor");
    assert_eq!(b.app.layers()[at].kind, LayerKind::Bitmap, "bitmap, comme le demandent les Préférences");
    assert!(encre(&b, at));
    // Le réglage repasse à vectoriel : le calque qui existe reste bitmap et reçoit toujours les traits.
    b.app.settings.vignettes.decor_bitmap = false;
    let avant = pas_d_annulation(&b);
    trait_au_milieu(&mut b, 40.0);
    assert_eq!(b.app.doc.panels[0].layers.iter().filter(|l| l.decor).count(), 1);
    assert_eq!(b.app.layers()[at].kind, LayerKind::Bitmap);
    assert_eq!(pas_d_annulation(&b), avant + 1);
    let cel = b.app.layers()[at].cel_at(0).unwrap();
    assert!(b.app.doc.cels[&cel].vector.is_none() && a_de_l_encre(&b.app.doc, cel));
}

#[test]
fn an_old_project_with_a_bitmap_decor_opens_with_it_and_its_decor_brushes_keep_drawing_in_it() {
    let dir = dossier("ancien-decor");
    let mut doc = une_continuite();
    doc.decor_bitmap = true;
    let (at, _) = doc.assurer_decor(0, 0).unwrap();
    let cel = doc.panels[0].layers[at].cel_at(0).unwrap();
    let c = doc.cels.get_mut(&cel).unwrap();
    let i = c.raster.tile_index_of(60, 50);
    c.raster.tile_mut(i)[(50 % crate::raster::TILE) * crate::raster::TILE + 60 % crate::raster::TILE] = Color32::from_rgb(0x8F, 0xA3, 0xB8);
    c.version += 1;
    let manifeste = dir.join("Ancien.pelure");
    crate::project::save(&doc, &manifeste, &mut Default::default()).expect("enregistrement");
    let relu = crate::project::load(&manifeste).expect("un projet d'avant s'ouvre").doc;
    let at = relu.decor_de(0).expect("le décor est revenu");
    assert_eq!(relu.panels[0].layers[at].kind, LayerKind::Bitmap, "il reste bitmap");
    let cel = relu.panels[0].layers[at].cel_at(0).unwrap();
    assert!(a_de_l_encre(&relu, cel), "sans rien perdre");
    // Dans SHOT (réglage vectoriel, d'origine) : les pinceaux de décor dessinent dans ce calque bitmap.
    crate::fxcache::monde::ecrire();
    let mut b = banc_taille(relu, false, 1600, 900);
    b.chauffer(6);
    assert!(!b.app.settings.vignettes.decor_bitmap);
    let (couches, avant) = (b.app.layers().len(), pas_d_annulation(&b));
    clic_case(&mut b, Case::Decor(Genre::Crayon));
    trait_au_milieu(&mut b, 0.0);
    assert_eq!(b.app.layers().len(), couches, "aucun calque de plus");
    assert_eq!(b.app.doc.panels[0].layers.iter().filter(|l| l.decor).count(), 1);
    assert_eq!(b.app.layers()[b.app.cur_layer].kind, LayerKind::Bitmap);
    assert_eq!(pas_d_annulation(&b), avant + 1);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn on_the_board_a_vector_decor_stroke_in_one_case_shows_in_every_case_of_the_continuity() {
    let mut b = ouvert(quatre_plans(), 4);
    let cartes = b.app.board_layout().cards.clone();
    assert!(cartes[0].plan == cartes[1].plan && cartes[1].plan == cartes[2].plan, "le plan 1 a trois cases");
    crate::decor_tests::choisir(&mut b, Genre::Aplat);
    let (n, v) = (pas_d_annulation(&b), vignette(&b, 1));
    glisser(&mut b, &vague(v.center() - vec2(20.0, 0.0), v.center() + vec2(20.0, 0.0), 10));
    let at = b.app.doc.decor_de(0).expect("le calque Décor du plan 1");
    let l = &b.app.doc.panels[0].layers[at];
    assert_eq!(l.kind, LayerKind::Vector, "vectoriel, d'origine");
    let cel = l.cel_at(0).unwrap();
    assert_eq!(l.frames, vec![Some(cel); b.app.doc.panels[0].frame_count()], "un seul dessin pour toute la continuité");
    for c in &cartes[..3] {
        let plan = render::panel_plan_at(&b.app.doc, 0, c.frame, None);
        assert!(plan.items.iter().any(|i| i.cel == cel), "la case de l'image {} montre le décor", c.frame);
    }
    assert!(a_de_l_encre(&b.app.doc, cel), "le trait est dans le décor");
    assert_eq!(pas_d_annulation(&b), n + 1, "le calque et le trait ne font qu'une étape");
    assert!(b.app.doc.decor_de(1).is_none(), "le plan voisin n'a rien");
}

#[test]
fn the_decor_setting_is_saved_in_the_application_settings_and_old_settings_read_as_vector() {
    use crate::settings::Settings;
    let defaut = Settings::default();
    assert!(!defaut.vignettes.decor_bitmap);
    assert!(!toml::to_string(&defaut).unwrap().contains("calque_decor_bitmap"), "des réglages d'origine n'écrivent rien de plus");
    let mut s = Settings::default();
    s.vignettes.decor_bitmap = true;
    let texte = toml::to_string(&s).unwrap();
    assert!(texte.contains("calque_decor_bitmap = true"), "{texte}");
    assert!(toml::from_str::<Settings>(&texte).unwrap().vignettes.decor_bitmap);
    let ancien: Settings = toml::from_str("[vignettes]\nreport_decor = false").unwrap();
    assert!(!ancien.vignettes.decor_bitmap, "un fichier d'avant : vectoriel");
}

#[test]
fn a_layer_made_by_hand_and_the_model_helpers_are_unaffected() {
    // Un calque ordinaire n'a pas de personnage ; le nom d'un personnage sans nom a un repli.
    let l = Layer::new("Calque 2", LayerKind::Bitmap);
    assert!(l.personnage.is_none() && !l.decor);
    assert_eq!(crate::calquespastilles::nom_du_calque("  "), "Personnage");
    assert_eq!(crate::calquespastilles::nom_du_calque("Lou"), "Lou");
    let mut doc = une_continuite();
    let mut lou = Layer::new("Lou", LayerKind::Vector);
    lou.personnage = Some(7);
    doc.panels[0].layers.push(lou);
    assert_eq!(doc.calque_de_personnage(0, 7), Some(1));
    assert_eq!(doc.calque_de_personnage(0, 8), None);
    assert_eq!(doc.renommer_calques_de_personnage(7, "Lou", "Lou"), 0);
    assert_eq!(doc.renommer_calques_de_personnage(7, "Lou", "Léa"), 1);
    assert_eq!(doc.panels[0].layers[1].name, "Léa");
    assert_eq!(doc.renommer_calques_de_personnage(7, "Léa", ""), 1, "un nom vide : « Personnage »");
    assert_eq!(doc.panels[0].layers[1].name, "Personnage");
}
