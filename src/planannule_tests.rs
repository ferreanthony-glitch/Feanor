//! Essais du plan annulé (partie 114) : par les vrais gestes de la planche (`Banc`), et un essai par famille d'export.
//!
//! La règle : un plan annulé est absent de tout, sauf de la planche où il reste barré ; rien n'est renuméroté, rien n'est effacé ; une étape
//! par geste ; aucun index périmé (toutes les images d'écran tournent sur le film changé, dans les trois espaces).

use std::path::PathBuf;

use eframe::egui::{Event, Modifiers, PointerButton, Pos2};

use crate::app::Tool;
use crate::document::{Cel, Document, Layer, LayerKind};
use crate::empreinte::Donnees;
use crate::perf_tests::Banc;
use crate::project;
use crate::vignettes_tests::{appui, clic_avec, dernier_nom, glisser, ouvert, pas_d_annulation, vignette};
use crate::workspace::Espace;

const LONGUEUR: usize = 12;

/// Quatre plans (une seule séquence), de 12 images, deux avec un texte à eux : chacun a un dessin qui a un pixel à lui, et un texte qui le nomme.
fn film() -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    for i in 0..3 {
        doc.add_scene_after(i);
    }
    for i in 0..4 {
        doc.panels[i].layers.clear();
        doc.panels[i].set_length(LONGUEUR);
        let mut l = Layer::new("Dessin", LayerKind::Bitmap);
        let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
        cel.raster.tile_mut(0)[0] = eframe::egui::Color32::from_rgba_premultiplied(10 + 40 * i as u8, 30, 200, 255);
        let c = doc.add_cel(cel);
        l.frames = vec![Some(c); LONGUEUR];
        doc.panels[i].layers.push(l);
        doc.panels[i].action = format!("ACTION{i}");
        doc.panels[i].dialogue = format!("DIALOGUE{i}");
    }
    doc
}

fn dans_les_vignettes(doc: Document) -> Banc {
    let mut b = ouvert(doc, 6);
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(4);
    b
}

/// Le numéro du plan `plan` à l'écran (celui de l'image d'avant) : son centre.
fn numero(b: &Banc, plan: u64) -> Pos2 {
    b.app.vignettes.numeros_zones.iter().find(|(_, p)| *p == plan).unwrap_or_else(|| panic!("le numéro du plan {plan} n'est pas à l'écran : {:?}", b.app.vignettes.numeros_zones)).0.center()
}

/// Une image de plus, pour que les zones des numéros soient à jour.
fn clic_numero(b: &mut Banc, plan: u64) {
    b.chauffer(2);
    let p = numero(b, plan);
    clic_avec(b, p, Modifiers::NONE);
    b.chauffer(2);
}

/// Tout ce que le plan `plan` porte, dessins compris, comme le fichier l'écrirait.
fn contenu(doc: &Document, panel: &crate::document::Panel) -> String {
    let entree = project::snapshot_value(panel, &|c| c).expect("entrée").to_string();
    let dessins: Vec<u64> = panel.layers.iter().flat_map(|l| l.frames.iter().flatten()).filter_map(|c| doc.cels.get(c)).map(|c| Donnees::de(c).empreinte()).collect();
    format!("{entree}|{dessins:?}")
}

/// Le plan, où qu'il soit : film ou annulé.
fn plan_de(doc: &Document, id: u64) -> crate::document::Panel {
    doc.panels.iter().find(|p| p.id == id).cloned().or_else(|| doc.annules.plans.iter().find(|g| g.panel.id == id).map(|g| g.panel.clone())).expect("le plan existe")
}

fn etiquette(doc: &Document, id: u64) -> String {
    doc.scene_label(plan_de(doc, id).scene)
}

fn dossier(nom: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("feanor-plan-annule-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier d'essai");
    dir
}

// ------------------------------------------------------------------------------ le geste

#[test]
fn a_click_on_the_number_cancels_the_plan_in_place_and_a_second_click_restores_it_all() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let mut b = dans_les_vignettes(film());
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    let cible = ids[1];
    let avant_contenu = contenu(&b.app.doc, &b.app.doc.panels[1]);
    let avant = b.app.board_layout();
    let (th0, th2, th3) = (avant.cards[0].thumb, avant.cards[2].thumb, avant.cards[3].thumb);
    let cible_th = avant.cards[1].thumb;
    let numeros: Vec<String> = ids.iter().map(|&i| etiquette(&b.app.doc, i)).collect();
    let duree = b.app.doc.total_frames();
    let etapes = pas_d_annulation(&b);

    clic_numero(&mut b, cible);

    assert_eq!(b.app.doc.panels.len(), 3, "le plan sort du film : {}", b.app.status);
    assert!(!b.app.doc.panels.iter().any(|p| p.id == cible));
    assert_eq!(b.app.doc.total_frames(), duree - LONGUEUR, "la durée du film");
    assert_eq!(pas_d_annulation(&b), etapes + 1, "une seule étape");
    assert_eq!(dernier_nom(&b), "Annuler le plan");
    // La case reste à sa place, les autres ne bougent pas.
    let apres = b.app.board_layout();
    assert_eq!(apres.cards.len(), 3);
    assert_eq!(apres.annulees.len(), 1);
    assert_eq!(apres.annulees[0].plan, cible);
    assert_eq!(apres.annulees[0].thumb, cible_th, "la case annulée n'a pas bougé");
    assert_eq!((apres.cards[0].thumb, apres.cards[1].thumb, apres.cards[2].thumb), (th0, th2, th3), "les autres non plus");
    // Rien n'est renuméroté.
    let apres_numeros: Vec<String> = ids.iter().map(|&i| etiquette(&b.app.doc, i)).collect();
    assert_eq!(numeros, apres_numeros, "les numéros ne bougent pas");
    // Les rangs des cases sont ceux du film d'aujourd'hui.
    assert!(apres.cards.iter().all(|c| b.app.doc.panels[c.panel].id == c.plan));

    clic_numero(&mut b, cible);

    assert_eq!(b.app.doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), ids, "à sa place");
    assert_eq!(contenu(&b.app.doc, &b.app.doc.panels[1]), avant_contenu, "tout y est");
    assert_eq!(pas_d_annulation(&b), etapes + 2);
    assert_eq!(dernier_nom(&b), "Rétablir le plan");
    assert!(b.app.doc.annules.is_empty());
    let revenue = b.app.board_layout();
    assert_eq!(revenue.cards[1].thumb, cible_th);
    assert!(revenue.annulees.is_empty());
}

#[test]
fn ctrl_z_undoes_each_gesture_in_one_step() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let mut b = dans_les_vignettes(film());
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    b.app.select_panel(1);
    b.chauffer(2);
    clic_numero(&mut b, ids[1]);
    assert_eq!(b.app.doc.panels.len(), 3);
    assert!(b.app.cur_panel < 3, "le plan courant annulé : un voisin devient courant");
    b.app.run_action("edition.annuler");
    b.chauffer(3);
    assert_eq!(b.app.doc.panels.len(), 4, "Ctrl+Z rend le plan");
    assert!(b.app.doc.annules.is_empty());
    assert_eq!(b.app.doc.panels[1].id, ids[1]);
    b.app.run_action("edition.retablir");
    b.chauffer(3);
    assert_eq!(b.app.doc.panels.len(), 3, "le rétablissement refait l'annulation");
    assert_eq!(b.app.doc.annules.plans.len(), 1);
}

#[test]
fn hidden_numbers_show_while_shift_is_held_and_shift_click_cancels_instead_of_choosing() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let mut b = dans_les_vignettes(film());
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    b.app.run_action("vignettes.numeros");
    b.chauffer(3);
    assert!(!b.app.settings.vignettes.numeros_visibles);
    assert!(b.app.vignettes.numeros_zones.is_empty(), "numéros cachés : rien à cliquer");
    // Maj tenue : ils se montrent.
    let centre = vignette(&b, 2).center();
    b.touches = Modifiers::SHIFT;
    b.image(vec![Event::PointerMoved(centre)], 1.0 / 60.0);
    b.chauffer(2);
    let p = numero(&b, ids[2]);
    let choix = b.app.vignettes.libre;
    let etapes = pas_d_annulation(&b);
    // Maj + clic sur le numéro : le plan s'annule (au lâcher), il n'est pas choisi.
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![appui(p, true, Modifiers::SHIFT)], 1.0 / 60.0);
    assert_eq!(b.app.doc.panels.len(), 4, "rien à l'appui");
    b.image(vec![appui(p, false, Modifiers::SHIFT)], 1.0 / 60.0);
    b.chauffer(2);
    assert_eq!(b.app.doc.panels.len(), 3, "{}", b.app.status);
    assert_eq!(b.app.vignettes.libre, choix, "le numéro prime sur « Maj + clic choisit le plan »");
    assert_eq!(pas_d_annulation(&b), etapes + 1);
    // Son numéro aussi se montre sous Maj : Maj + clic le rétablit.
    let p = numero(&b, ids[2]);
    b.image(vec![appui(p, true, Modifiers::SHIFT)], 1.0 / 60.0);
    b.image(vec![appui(p, false, Modifiers::SHIFT)], 1.0 / 60.0);
    b.chauffer(2);
    assert_eq!(b.app.doc.panels.len(), 4);
    // Maj relâchée : plus de numéros, et l'endroit n'est plus cliquable.
    b.touches = Modifiers::NONE;
    b.chauffer(3);
    assert!(b.app.vignettes.numeros_zones.is_empty());
    let n = b.app.doc.panels.len();
    let etapes = pas_d_annulation(&b);
    clic_avec(&mut b, p, Modifiers::NONE);
    b.chauffer(2);
    assert_eq!(b.app.doc.panels.len(), n);
    assert!(pas_d_annulation(&b) <= etapes + 1, "un clic sans numéro : un point sur le papier au plus");
    assert!(b.app.doc.annules.plans.is_empty());
    // Et Maj + clic ailleurs choisit toujours le plan.
    b.touches = Modifiers::SHIFT;
    let v = vignette(&b, 3).center();
    b.image(vec![Event::PointerMoved(v)], 1.0 / 60.0);
    b.image(vec![appui(v, true, Modifiers::SHIFT)], 1.0 / 60.0);
    b.image(vec![appui(v, false, Modifiers::SHIFT)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.chauffer(2);
    assert_eq!(b.app.cur_panel, 3);
    assert!(!b.app.vignettes.libre);
}

#[test]
fn shift_dragging_from_a_number_moves_the_plan_and_cancels_nothing() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let mut b = dans_les_vignettes(film());
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    b.touches = Modifiers::SHIFT;
    b.chauffer(3);
    let p = numero(&b, ids[0]);
    let vers = vignette(&b, 2).center();
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![appui(p, true, Modifiers::SHIFT)], 1.0 / 60.0);
    for k in 1..=12 {
        b.image(vec![Event::PointerMoved(p + (vers - p) * (k as f32 / 12.0))], 1.0 / 60.0);
    }
    b.image(vec![appui(vers, false, Modifiers::SHIFT)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.chauffer(3);
    assert!(b.app.doc.annules.plans.is_empty(), "un glissé n'annule rien : {}", b.app.status);
    assert_eq!(b.app.doc.panels.len(), 4);
    assert_ne!(b.app.doc.panels[0].id, ids[0], "le plan s'est déplacé");
}

#[test]
fn a_stroke_from_a_number_is_a_stroke_and_cuts_nothing() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    use crate::traits_continus_tests::{planche, relever, repos, tracer};
    let Some(mut b) = planche(film(), 4, 1.0) else { return };
    b.app.set_espace(Espace::Vignettes);
    b.app.settings.vignettes.numeros_visibles = true;
    b.chauffer(4);
    b.app.set_tool(Tool::Brush);
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    let etapes = pas_d_annulation(&b);
    let de = numero(&b, ids[1]);
    let a = vignette(&b, 1).center();
    let sans = crate::vignettes_tests::encre(&b.app.doc, 1);
    tracer(&mut b, de, a);
    repos(&mut b);
    assert_eq!(b.app.doc.panels.len(), 4, "un trait parti du numéro n'annule pas : {}", b.app.status);
    assert!(b.app.doc.annules.plans.is_empty());
    assert_eq!(pas_d_annulation(&b), etapes + 1, "une seule étape : le trait");
    assert!(sans, "(le plan avait déjà son pixel)");
    let rgba = b.capturer().expect("capture");
    let r = relever(&rgba, de + (a - de).normalized() * 14.0, a);
    assert!(r.trou <= 3, "le trait est continu depuis le numéro : {r:?}");
    // Un clic sans mouvement annule, sans point de trait (pas d'étape de plus).
    let etapes = pas_d_annulation(&b);
    b.chauffer(2);
    let de = numero(&b, ids[1]);
    clic_avec(&mut b, de, Modifiers::NONE);
    b.chauffer(2);
    assert_eq!(b.app.doc.panels.len(), 3, "{}", b.app.status);
    assert_eq!(pas_d_annulation(&b), etapes + 1, "le clic : une étape, pas de point de trait");
}

#[test]
fn the_right_click_menu_is_the_second_way() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let mut b = dans_les_vignettes(film());
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    b.garder_formes = true;
    let v = vignette(&b, 1).center();
    let droit = |pos: Pos2, down: bool| Event::PointerButton { pos, button: PointerButton::Secondary, pressed: down, modifiers: Modifiers::NONE };
    b.image(vec![Event::PointerMoved(v)], 1.0 / 60.0);
    b.image(vec![droit(v, true)], 1.0 / 60.0);
    b.image(vec![droit(v, false)], 1.0 / 60.0);
    b.chauffer(2);
    let entree = |b: &Banc, texte: &str| {
        b.formes.iter().find_map(|s| match &s.shape {
            eframe::egui::epaint::Shape::Text(t) if t.galley.job.text == texte => Some(t.pos + t.galley.size() / 2.0),
            _ => None,
        })
    };
    // (Dans « Plus » : un menu de dix entrées au plus, et c'est le chemin de secours.)
    let plus = entree(&b, "Plus").expect("« Plus » au clic droit de la case");
    clic_avec(&mut b, plus, Modifiers::NONE);
    b.chauffer(3);
    let p = entree(&b, "Annuler ce plan").expect("« Annuler ce plan » est au clic droit de la case");
    clic_avec(&mut b, p, Modifiers::NONE);
    b.chauffer(3);
    assert_eq!(b.app.doc.panels.len(), 3, "{}", b.app.status);
    assert_eq!(b.app.doc.annules.plans[0].panel.id, ids[1]);
    // Sur la case annulée : « Rétablir ce plan ».
    let c = b.app.board_layout().annulees[0].thumb;
    let v = b.app.vignettes.xf.rect(c).center();
    b.image(vec![Event::PointerMoved(v)], 1.0 / 60.0);
    b.image(vec![droit(v, true)], 1.0 / 60.0);
    b.image(vec![droit(v, false)], 1.0 / 60.0);
    b.chauffer(2);
    let entree = b.formes.iter().find_map(|s| match &s.shape {
        eframe::egui::epaint::Shape::Text(t) if t.galley.job.text == "Rétablir ce plan" => Some(t.pos + t.galley.size() / 2.0),
        _ => None,
    });
    clic_avec(&mut b, entree.expect("« Rétablir ce plan » sur la case annulée"), Modifiers::NONE);
    b.chauffer(3);
    assert_eq!(b.app.doc.panels.len(), 4);
}

#[test]
fn drawing_in_a_cancelled_case_is_refused_and_says_so() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let mut b = dans_les_vignettes(film());
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    clic_numero(&mut b, ids[1]);
    let avant = contenu(&b.app.doc, &plan_de(&b.app.doc, ids[1]));
    let dessins: Vec<String> = ids.iter().map(|&i| contenu(&b.app.doc, &plan_de(&b.app.doc, i))).collect();
    let etapes = pas_d_annulation(&b);
    let c = b.app.vignettes.xf.rect(b.app.board_layout().annulees[0].thumb);
    b.app.set_tool(Tool::Brush);
    glisser(&mut b, &[c.center(), c.center() + eframe::egui::vec2(12.0, 6.0), c.center() + eframe::egui::vec2(24.0, 0.0)]);
    b.chauffer(2);
    assert_eq!(pas_d_annulation(&b), etapes, "aucun trait, aucune étape");
    assert!(b.app.status.contains("annulé"), "{}", b.app.status);
    assert_eq!(contenu(&b.app.doc, &plan_de(&b.app.doc, ids[1])), avant);
    assert_eq!(ids.iter().map(|&i| contenu(&b.app.doc, &plan_de(&b.app.doc, i))).collect::<Vec<_>>(), dessins, "aucun autre plan n'a reçu le trait");
}

#[test]
fn a_stroke_crossing_a_cancelled_case_is_not_cut() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    use crate::traits_continus_tests::{planche, relever, repos, tracer};
    let Some(mut b) = planche(film(), 4, 1.0) else { return };
    b.app.set_espace(Espace::Vignettes);
    b.app.settings.vignettes.numeros_visibles = true;
    b.chauffer(4);
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    clic_numero(&mut b, ids[1]);
    b.app.set_tool(Tool::Brush);
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let (de, a) = (xf.rect(l.cards[0].thumb).center(), xf.rect(l.cards[1].thumb).center());
    let etapes = pas_d_annulation(&b);
    tracer(&mut b, de, a);
    repos(&mut b);
    assert_eq!(pas_d_annulation(&b), etapes + 1, "un seul trait, une étape");
    let rgba = b.capturer().expect("capture");
    let r = relever(&rgba, de, a);
    assert!(r.trou <= 3, "le trait traverse la case annulée sans se couper : {r:?}");
    // Rétablir : le plan n'a pas reçu le trait, et rien ne s'est perdu.
    b.chauffer(2);
    let plan = plan_de(&b.app.doc, ids[1]);
    let avant = contenu(&b.app.doc, &plan);
    clic_numero(&mut b, ids[1]);
    assert_eq!(contenu(&b.app.doc, &plan_de(&b.app.doc, ids[1])), avant);
}

// ------------------------------------------------------------------------------ le plan courant, tout le film, aucun index périmé

/// Toutes les vues tournent sur le film tel qu'il est : aucune panique, aucun rang périmé.
fn faire_le_tour(b: &mut Banc) {
    for e in [Espace::Storyboard, Espace::Montage, Espace::Vignettes] {
        b.app.set_espace(e);
        b.chauffer(4);
        assert!(b.app.cur_panel < b.app.doc.panels.len());
    }
    b.app.run_action("panneau.suivant");
    b.chauffer(2);
    b.app.run_action("panneau.precedent");
    b.chauffer(2);
}

#[test]
fn cancelling_the_current_plan_goes_to_a_neighbour_on_every_screen() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let mut b = dans_les_vignettes(film());
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    b.app.select_panel(3);
    b.chauffer(2);
    clic_numero(&mut b, ids[3]);
    assert_eq!(b.app.doc.panels[b.app.cur_panel].id, ids[2], "le dernier plan annulé : le voisin d'avant");
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(4);
    assert_eq!(b.app.doc.panels[b.app.cur_panel].id, ids[2], "le Storyboard est sur un plan actif");
    faire_le_tour(&mut b);
}

#[test]
fn when_everything_is_cancelled_the_storyboard_says_so_and_exports_refuse() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let mut b = dans_les_vignettes(film());
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    let contenus: Vec<String> = ids.iter().map(|&i| contenu(&b.app.doc, &plan_de(&b.app.doc, i))).collect();
    for &i in &ids {
        clic_numero(&mut b, i);
    }
    assert!(b.app.doc.film_vide(), "{} plans, {:?}", b.app.doc.panels.len(), b.app.status);
    assert_eq!(b.app.doc.panels.len(), 1, "le support tient le film");
    assert_eq!(b.app.doc.annules.plans.len(), 4);
    let l = b.app.board_layout();
    assert!(l.cards.is_empty(), "le support n'a pas de case");
    assert_eq!(l.annulees.len(), 4);
    assert!(!l.empty.is_empty(), "les cases pâles restent");
    // Le Storyboard : un état vide clair.
    b.garder_textes = true;
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(4);
    assert!(b.textes.iter().any(|t| t.contains(crate::planannuleui::TEXTE_FILM_VIDE)), "{:?}", b.textes.iter().take(20).collect::<Vec<_>>());
    // Tous les exports refusent, avec un message.
    let actions: Vec<&str> = crate::keymap::ACTIONS.iter().map(|a| a.id).filter(|a| a.starts_with("fichier.exporter") || *a == "fichier.kitsu_publier").collect();
    assert!(actions.len() >= 15, "{actions:?}");
    for a in &actions {
        b.app.status.clear();
        b.app.run_action(a);
        assert_eq!(b.app.status, crate::planannuleui::REFUS_EXPORT, "{a}");
    }
    faire_le_tour(&mut b);
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(3);
    // Rétablir un plan : le support part, ce plan est le film.
    clic_numero(&mut b, ids[2]);
    assert_eq!(b.app.doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), vec![ids[2]]);
    assert!(b.app.doc.annules.support.is_none());
    assert!(!b.app.doc.film_vide());
    assert_eq!(contenu(&b.app.doc, &plan_de(&b.app.doc, ids[2])), contenus[2]);
    // Tout rétablir : le film d'origine.
    for &i in [ids[0], ids[1], ids[3]].iter() {
        clic_numero(&mut b, i);
    }
    assert_eq!(b.app.doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), ids);
    assert_eq!(ids.iter().map(|&i| contenu(&b.app.doc, &plan_de(&b.app.doc, i))).collect::<Vec<_>>(), contenus);
    assert!(b.app.doc.annules.is_empty());
}

#[test]
fn a_film_of_one_plan_can_be_cancelled_undone_and_a_new_plan_replaces_the_placeholder() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let mut doc = Document::with_frame(160, 90, 24);
    doc.panels[0].action = "SEUL".into();
    let seul = doc.panels[0].id;
    let mut b = dans_les_vignettes(doc);
    clic_numero(&mut b, seul);
    assert!(b.app.doc.film_vide());
    assert_eq!(b.app.cur_panel, 0);
    // Ctrl+Z : le film d'avant.
    b.app.run_action("edition.annuler");
    b.chauffer(3);
    assert_eq!(b.app.doc.panels[0].id, seul);
    assert!(!b.app.doc.film_vide() && b.app.doc.annules.is_empty());
    b.app.run_action("edition.retablir");
    b.chauffer(3);
    assert!(b.app.doc.film_vide());
    // Un plan neuf : le support part, le plan annulé reste.
    let support = b.app.doc.panels[0].id;
    b.app.run_action("panneau.nouveau");
    b.chauffer(3);
    assert_eq!(b.app.doc.panels.len(), 1, "le plan neuf prend la place du support");
    assert_ne!(b.app.doc.panels[0].id, support);
    assert!(!b.app.doc.film_vide());
    assert_eq!(b.app.doc.annules.plans.len(), 1);
    b.app.run_action("edition.annuler");
    b.chauffer(3);
    assert!(b.app.doc.film_vide(), "annuler le plan neuf rend l'état vide");
    faire_le_tour(&mut b);
}

#[test]
fn drawing_in_a_pale_case_when_everything_is_cancelled_makes_a_real_plan() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let mut doc = Document::with_frame(160, 90, 24);
    doc.add_scene_after(0);
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    let mut b = dans_les_vignettes(doc);
    for &i in &ids {
        clic_numero(&mut b, i);
    }
    assert!(b.app.doc.film_vide());
    b.app.set_tool(Tool::Brush);
    let c = crate::vignettes_tests::case_vide(&b, 0);
    crate::vignettes_tests::glisser(&mut b, &crate::vignettes_tests::vague(c.center() - eframe::egui::vec2(20.0, 0.0), c.center() + eframe::egui::vec2(20.0, 0.0), 8));
    b.chauffer(3);
    assert!(!b.app.doc.film_vide(), "{}", b.app.status);
    assert_eq!(b.app.doc.panels.len(), 1);
    assert!(crate::vignettes_tests::encre(&b.app.doc, 0), "le trait est dans le plan");
    assert_eq!(b.app.doc.annules.plans.len(), 2);
    faire_le_tour(&mut b);
}

// ------------------------------------------------------------------------------ coupe, réunion, déplacement, duplication, séquences

/// L'ordre de la planche, en numéros de plan.
fn ordre(doc: &Document) -> Vec<u64> {
    doc.ordre_planche()
        .into_iter()
        .map(|r| match r {
            crate::planannule::Rang::Film(i) => doc.panels[i].id,
            crate::planannule::Rang::Annule(k) => doc.annules.plans[k].panel.id,
        })
        .collect()
}

#[test]
fn cutting_joining_moving_and_duplicating_neighbours_keep_the_cancelled_plan_in_place() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let mut doc = film();
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    doc.annuler_plan(ids[2]).unwrap();
    assert_eq!(ordre(&doc), ids);
    // Couper le plan d'avant : le plan annulé reste derrière la suite.
    let i = doc.panels.iter().position(|p| p.id == ids[1]).unwrap();
    let suite = doc.split_plan(i, 4).unwrap();
    let suite_id = doc.panels[suite].id;
    assert_eq!(ordre(&doc), vec![ids[0], ids[1], suite_id, ids[2], ids[3]]);
    // Réunir le plan d'avant et sa suite : le plan annulé reste derrière le plan réuni.
    let i = doc.panels.iter().position(|p| p.id == ids[1]).unwrap();
    doc.join_next_plan(i).unwrap();
    assert_eq!(ordre(&doc), ids);
    // Réunir à travers lui (au montage, les deux plans sont voisins) : rien ne se perd.
    let i = doc.panels.iter().position(|p| p.id == ids[1]).unwrap();
    doc.join_next_plan(i).unwrap();
    assert_eq!(ordre(&doc), vec![ids[0], ids[1], ids[2]], "il reste derrière le plan réuni");
    assert_eq!(doc.annules.plans.len(), 1);
    // Rétablir : il revient derrière le plan réuni.
    doc.retablir_plan(ids[2]).unwrap();
    assert_eq!(doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), vec![ids[0], ids[1], ids[2]]);

    // Déplacer : il suit le plan qui le précède ; dupliquer : la copie se pose derrière lui.
    let mut doc = film();
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    doc.annuler_plan(ids[1]).unwrap();
    doc.move_plans(&[ids[0]], None).unwrap();
    assert_eq!(ordre(&doc), vec![ids[2], ids[3], ids[0], ids[1]]);
    doc.retablir_plan(ids[1]).unwrap();
    assert_eq!(doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), vec![ids[2], ids[3], ids[0], ids[1]]);
    let mut doc = film();
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    doc.annuler_plan(ids[1]).unwrap();
    let copie = doc.duplicate_scene_after(0);
    let copie = doc.panels[copie].id;
    assert_eq!(ordre(&doc), vec![ids[0], ids[1], copie, ids[2], ids[3]]);
}

#[test]
fn the_plan_before_it_deleted_or_in_the_bin_leaves_it_on_the_board_and_nothing_is_lost() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let mut doc = film();
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    doc.annuler_plan(ids[1]).unwrap();
    assert!(doc.corbeille.is_empty(), "annuler n'envoie rien à la corbeille");
    let i = doc.panels.iter().position(|p| p.id == ids[0]).unwrap();
    doc.remove_panel(i).unwrap();
    assert_eq!(doc.corbeille.len(), 1, "le voisin supprimé est dans la corbeille");
    // Le plan d'avant n'est plus là : le plan annulé reste devant celui qui le suivait.
    assert_eq!(ordre(&doc), vec![ids[1], ids[2], ids[3]]);
    doc.retablir_plan(ids[1]).unwrap();
    assert_eq!(doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), vec![ids[1], ids[2], ids[3]]);
    // Plus aucun voisin connu : il passe à la fin, jamais perdu.
    let mut doc = film();
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    doc.annuler_plan(ids[1]).unwrap();
    let i = doc.panels.iter().position(|p| p.id == ids[2]).unwrap();
    doc.remove_panel(i).unwrap();
    let i = doc.panels.iter().position(|p| p.id == ids[0]).unwrap();
    doc.remove_panel(i).unwrap();
    assert_eq!(ordre(&doc), vec![ids[3], ids[1]]);
    // Supprimer un plan du film alors qu'un plan est annulé : la corbeille le rend comme avant.
    let id = doc.corbeille.entrees[0].id;
    let e = doc.corbeille.trouver(id).unwrap().clone();
    let (charge, _) = crate::corbeille::charger(&e, None, &|_| true).unwrap();
    doc.restaurer_entree(id, charge, 0).unwrap();
    assert_eq!(doc.annules.plans.len(), 1, "le plan annulé n'a pas bougé");
    assert!(doc.corbeille.len() == 1);
}

#[test]
fn a_cancelled_plan_keeps_its_transitions_comments_crop_versions_and_references() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let mut app = crate::montage_sans_casse_tests::appli("annule-tout");
    let ids: Vec<u64> = app.doc.panels.iter().map(|p| p.id).collect();
    app.rogner_plan(1, crate::rognage::Rogner::Tail(-3), "Rogner");
    app.doc.nouvelle_version(1, 1_000).expect("une version");
    app.doc.set_transition(1, crate::transitions::Kind::Dissolve).expect("une transition");
    app.doc.set_transition(2, crate::transitions::Kind::Dissolve).expect("une transition");
    let reference: crate::references::Reference = serde_json::from_str(&format!(r#"{{"id":9000,"taille":[4,4],"x":0.0,"y":0.0,"plans":[{}]}}"#, ids[1])).expect("une référence");
    app.doc.references.push(reference);
    let avant = app.doc.structure();
    let plan = contenu(&app.doc, &app.doc.panels[1]);
    let n_transitions = app.doc.transitions.len();
    let reserve = app.doc.reserves.get(&ids[1]).map(|r| r.cels().count());
    assert!(reserve.is_some_and(|n| n > 0));
    let versions = app.doc.versions.get(&ids[1]).map(|v| v.autres.len());

    app.doc.annuler_plan(ids[1]).unwrap();
    assert_eq!(app.doc.transitions.len(), 0, "les transitions du plan ne sont plus dans le film");
    assert!(!app.doc.reserves.contains_key(&ids[1]) && !app.doc.versions.contains_key(&ids[1]));
    assert!(app.doc.references[0].plans.is_empty());
    assert!(app.doc.used_cels().len() >= avant.panels[1].layers[0].frames.iter().flatten().count(), "ses dessins restent dans le projet");
    let retenus = app.doc.used_cels();
    assert!(avant.panels[1].layers.iter().flat_map(|l| l.frames.iter().flatten()).all(|c| retenus.contains(c)), "enregistrer ne les efface pas");

    app.doc.retablir_plan(ids[1]).unwrap();
    assert_eq!(contenu(&app.doc, &app.doc.panels[1]), plan);
    assert_eq!(app.doc.transitions.len(), n_transitions, "ses transitions reviennent");
    assert_eq!(app.doc.reserves.get(&ids[1]).map(|r| r.cels().count()), reserve);
    assert_eq!(app.doc.versions.get(&ids[1]).map(|v| v.autres.len()), versions);
    assert_eq!(app.doc.references[0].plans, vec![ids[1]]);
}

#[test]
fn a_cancellation_belongs_to_its_sequence_version_and_the_last_plan_of_a_versioned_sequence_is_refused() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    use crate::variantes::Nouvelle;
    let mut doc = film();
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    let q = doc.sequence_of(0);
    assert_eq!(doc.numero_de_variante(q), 1);
    let b_num = doc.nouvelle_variante(q, Nouvelle::Partager, 1_000).expect("une version B");
    assert_eq!(doc.numero_de_variante(q), b_num);
    let dessin = contenu(&doc, &doc.panels[1]);
    // Annulé dans B : A le montre encore (il est au garage, tel qu'il est).
    doc.annuler_plan(ids[1]).unwrap();
    assert_eq!(doc.annules_visibles().len(), 1);
    assert!(doc.changer_de_variante(q, 1), "retour à A");
    assert!(doc.panels.iter().any(|p| p.id == ids[1]), "A montre le plan");
    assert!(doc.annules_visibles().is_empty(), "et pas de croix dans A");
    assert_eq!(contenu(&doc, doc.panels.iter().find(|p| p.id == ids[1]).unwrap()), dessin);
    // Retouché dans A : B retrouve le plan retouché en le rétablissant.
    let i = doc.panels.iter().position(|p| p.id == ids[1]).unwrap();
    doc.panels[i].action = "RETOUCHE DANS A".into();
    assert!(doc.changer_de_variante(q, b_num), "retour à B");
    assert!(!doc.panels.iter().any(|p| p.id == ids[1]));
    assert_eq!(doc.annules_visibles().len(), 1);
    doc.retablir_plan(ids[1]).unwrap();
    assert_eq!(doc.panels.iter().find(|p| p.id == ids[1]).unwrap().action, "RETOUCHE DANS A");
    // Une séquence à versions garde au moins un plan : l'annuler les ferait disparaître.
    for &i in &ids[1..] {
        doc.annuler_plan(i).unwrap();
    }
    crate::variantes::prendre_refus();
    assert!(doc.annuler_plan(ids[0]).is_none());
    assert!(crate::variantes::prendre_refus().is_some_and(|m| m.contains("versions de séquence")));
    assert!(doc.panels.iter().any(|p| p.id == ids[0]));
    assert!(doc.variantes.contains_key(&q));
}

#[test]
fn a_sequence_whose_plans_are_all_cancelled_stays_on_the_board_with_its_name_and_comes_back() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let mut doc = crate::vignettes_tests::deux_sequences();
    let dernier = doc.panels.len() - 1;
    let id = doc.panels[dernier].id;
    let q = doc.sequence_of(dernier);
    doc.sequence_mut(q).unwrap().name = "La fuite".into();
    let numero_seq = doc.sequence_number(q);
    doc.annuler_plan(id).unwrap();
    assert!(!doc.sequences.iter().any(|s| s.id == q), "plus dans le film");
    assert_eq!(doc.sequence_order_planche().last(), Some(&q));
    assert_eq!(doc.sequence_number(q), numero_seq);
    assert_eq!(doc.sequence_label(q), "La fuite");
    let vue = crate::planchesequences::vue(&doc).expect("deux séquences sur la planche");
    assert_eq!(vue.sequences.len(), 2);
    doc.retablir_plan(id).unwrap();
    assert_eq!(doc.sequence_of(doc.panels.len() - 1), q);
    assert_eq!(doc.sequences.iter().find(|s| s.id == q).unwrap().name, "La fuite");
    // À l'écran : par séquences, puis à la japonaise, tout annulé compris.
    let mut b = dans_les_vignettes(doc);
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    b.chauffer(2);
    clic_numero(&mut b, id);
    let l = b.app.board_layout();
    assert_eq!(l.groupes.len(), 2);
    assert_eq!(l.annulees.len(), 1);
    b.app.layout.board_sens = crate::boardsens::Sens::Japonais;
    b.chauffer(4);
    let l = b.app.board_layout();
    assert_eq!(l.annulees.len(), 1);
    assert!(l.cards.iter().all(|c| b.app.doc.panels[c.panel].id == c.plan));
    for &i in ids.iter().filter(|i| **i != id) {
        clic_numero(&mut b, i);
    }
    assert!(b.app.doc.film_vide());
    faire_le_tour(&mut b);
}

// ------------------------------------------------------------------------------ le son

#[test]
fn the_sound_neither_moves_nor_disappears_and_the_status_line_says_so() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let mut doc = film();
    let pcm = crate::audio::Pcm::from_interleaved(&vec![0.2; 48_000 * 2], 2, crate::audio::RATE);
    let son = doc.add_sound("essai.wav".into(), "wav".into(), std::sync::Arc::new(Vec::new()), std::sync::Arc::new(pcm));
    let clip = doc.place_sound(son, LONGUEUR + 2, None).expect("un extrait posé dans le plan 2");
    let piste_avant = doc.audio_tracks.clone();
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    let mut b = dans_les_vignettes(doc);
    clic_numero(&mut b, ids[1]);
    assert_eq!(b.app.doc.audio_tracks, piste_avant, "ni détruit ni déplacé");
    assert!(b.app.doc.clip(clip).is_some());
    assert!(b.app.status.contains("le son reste à sa place"), "{}", b.app.status);
    assert!(!b.app.status.trim_end().ends_with('.') && !b.app.status.contains(". "), "une seule phrase : la barre d'état coupe au premier point");
    clic_numero(&mut b, ids[1]);
    assert_eq!(b.app.doc.audio_tracks, piste_avant, "rétablir retrouve l'accord d'avant");
}

// ------------------------------------------------------------------------------ le format

fn enregistrer(doc: &Document, nom: &str) -> (PathBuf, PathBuf) {
    let dir = dossier(nom);
    let manifest = dir.join(format!("P.{}", project::EXTENSION));
    project::save(doc, &manifest, &mut Default::default()).expect("enregistrement");
    (dir, manifest)
}

#[test]
fn everything_survives_saving_and_reopening_and_nothing_is_written_when_no_plan_is_cancelled() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let mut app = crate::montage_sans_casse_tests::appli("annule-fichier");
    let doc = &mut app.doc;
    assert_eq!(project::VERSION, 48);
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    // Un film sans plan annulé n'écrit aucun fichier.
    let (dir, _) = enregistrer(doc, "sans");
    assert!(!dir.join(crate::planannule::FILE).exists());
    std::fs::remove_dir_all(&dir).ok();

    app.rogner_plan(1, crate::rognage::Rogner::Tail(-3), "Rogner");
    let doc = &mut app.doc;
    doc.nouvelle_version(1, 1_000).expect("une version");
    let c = doc.default_camera();
    doc.panels[1].camera = vec![crate::camera::CameraKey::new(0, c), crate::camera::CameraKey::new(5, crate::camera::Camera { x: c.x + 30.0, ..c })];
    doc.panels[1].action = "ACTION DU PLAN".into();
    doc.set_transition(2, crate::transitions::Kind::Dissolve).expect("une transition");
    let nom = doc.panels[1].scene;
    doc.scene_mut(nom).unwrap().name = "Le toit".into();
    let plan = contenu(doc, &doc.panels[1]);
    let plan_cels: Vec<u64> = doc.panels[1].layers.iter().flat_map(|l| l.frames.iter().flatten().copied()).collect();
    let numeros: Vec<String> = ids.iter().map(|&i| etiquette(doc, i)).collect();
    let rogne = doc.reserves.get(&ids[1]).map(|r| r.cels().count());
    let versions = doc.versions.get(&ids[1]).map(|v| v.autres.len());
    let n_transitions = doc.transitions.len();
    doc.annuler_plan(ids[1]).unwrap();

    let (dir, manifest) = enregistrer(doc, "avec");
    assert!(dir.join(crate::planannule::FILE).exists());
    let texte = std::fs::read_to_string(&manifest).unwrap();
    assert!(texte.contains("\"version\": 48"), "le manifeste dit 48");

    let mut lu = project::load(&manifest).expect("réouverture");
    assert!(lu.warnings.is_empty(), "{:?}", lu.warnings);
    assert_eq!(lu.doc.panels.len(), 3, "toujours hors du film");
    assert_eq!(lu.doc.annules.plans.len(), 1);
    assert_eq!(ids.iter().map(|&i| etiquette(&lu.doc, i)).collect::<Vec<_>>(), numeros, "les numéros");
    assert_eq!(ordre(&lu.doc), ids, "sa place");
    assert!(plan_cels.iter().all(|c| lu.doc.cels.contains_key(c)), "ses dessins sont relus");
    // Enregistrer encore et encore n'efface rien.
    let mut written = lu.written.clone();
    project::save(&lu.doc, &manifest, &mut written).expect("enregistrement");
    project::save(&lu.doc, &manifest, &mut written).expect("enregistrement");
    let mut lu2 = project::load(&manifest).expect("réouverture");
    assert_eq!(lu2.doc.annules.plans.len(), 1);
    // Rétablir après réouverture : tout y est.
    lu2.doc.retablir_plan(ids[1]).unwrap();
    assert_eq!(lu2.doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), ids);
    let i = lu2.doc.panels.iter().position(|p| p.id == ids[1]).unwrap();
    assert_eq!(contenu(&lu2.doc, &lu2.doc.panels[i]), plan, "dessins, caméra, textes");
    assert_eq!(lu2.doc.scene_label(lu2.doc.panels[i].scene), "Le toit");
    assert_eq!(lu2.doc.reserves.get(&ids[1]).map(|r| r.cels().count()), rogne, "contenu rogné");
    assert_eq!(lu2.doc.versions.get(&ids[1]).map(|v| v.autres.len()), versions, "versions");
    assert_eq!(lu2.doc.transitions.len(), n_transitions, "transitions");
    // Rétabli et enregistré : le fichier part.
    let mut written = lu2.written.clone();
    project::save(&lu2.doc, &manifest, &mut written).expect("enregistrement");
    assert!(!dir.join(crate::planannule::FILE).exists());
    // (Le film en mémoire d'avant reste valide.)
    lu.doc.normalize();
    std::fs::remove_dir_all(&dir).ok();
}

/// Sur un vrai projet : `FEANOR_ESSAI_PROJET` est le manifeste d'une **copie** ; il est réécrit.
#[test]
#[ignore = "demande une copie d'un vrai projet : FEANOR_ESSAI_PROJET"]
fn on_a_copy_of_a_real_project() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let Some(m) = std::env::var_os("FEANOR_ESSAI_PROJET").map(PathBuf::from) else { return };
    let mut lu = project::load(&m).expect("le projet s'ouvre");
    let n = lu.doc.panels.len();
    assert!(n >= 2, "{n} plans");
    let id = lu.doc.panels[n / 2].id;
    let avant = contenu(&lu.doc, &lu.doc.panels[n / 2]);
    let numeros: Vec<String> = lu.doc.panels.iter().map(|p| lu.doc.scene_label(p.scene)).collect();
    let duree = lu.doc.total_frames();
    let longueur = lu.doc.panels[n / 2].frame_count();
    lu.doc.annuler_plan(id).expect("annulé");
    assert_eq!(lu.doc.total_frames(), duree - longueur);
    let mut written = lu.written.clone();
    project::save(&lu.doc, &m, &mut written).expect("enregistré");
    let mut relu = project::load(&m).expect("rouvert");
    assert_eq!(relu.doc.panels.len(), n - 1);
    relu.doc.retablir_plan(id).expect("rétabli");
    assert_eq!(relu.doc.panels.iter().map(|p| relu.doc.scene_label(p.scene)).collect::<Vec<_>>(), numeros);
    let k = relu.doc.panels.iter().position(|p| p.id == id).expect("à sa place");
    assert_eq!(k, n / 2);
    assert_eq!(contenu(&relu.doc, &relu.doc.panels[k]), avant, "tout y est après enregistrement et réouverture");
    let mut b = dans_les_vignettes(relu.doc);
    clic_numero(&mut b, id);
    faire_le_tour(&mut b);
    eprintln!("vrai projet : {n} plans, le plan {} annulé, enregistré, rouvert, rétabli : identique.", numeros[n / 2]);
}

#[test]
fn a_plan_cancelled_here_and_shown_by_another_sequence_version_reopens_without_being_renumbered() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    use crate::variantes::Nouvelle;
    let mut doc = film();
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    let q = doc.sequence_of(0);
    let b_num = doc.nouvelle_variante(q, Nouvelle::Partager, 1_000).expect("une version B");
    doc.panels[1].action = "RETOUCHE".into();
    doc.annuler_plan(ids[1]).unwrap();
    let (dir, manifest) = enregistrer(&doc, "garage");
    let mut lu = project::load(&manifest).expect("réouverture");
    assert!(lu.warnings.is_empty(), "aucun numéro en double : {:?}", lu.warnings);
    assert_eq!(lu.doc.annules.plans[0].panel.id, ids[1]);
    assert_eq!(lu.doc.numero_de_variante(q), b_num);
    assert_eq!(ordre(&lu.doc), ids);
    lu.doc.retablir_plan(ids[1]).unwrap();
    assert_eq!(lu.doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), ids);
    assert_eq!(lu.doc.panels[1].action, "RETOUCHE");
    assert!(lu.doc.changer_de_variante(q, 1), "la version A montre toujours le plan");
    assert!(lu.doc.panels.iter().any(|p| p.id == ids[1]));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn what_a_cancelled_plan_has_off_the_sheet_survives_a_reopen() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let mut doc = film();
    let id = doc.panels[1].id;
    let c = doc.panels[1].layers[0].frames[0].unwrap();
    let cel = doc.cels.get_mut(&c).unwrap();
    // Un dessin hors de la feuille (toile infinie).
    let rouge = eframe::egui::Color32::from_rgba_premultiplied(200, 0, 0, 255);
    let t = cel.raster.id_of(-1, -1);
    cel.raster.tile_mut(t)[0] = rouge;
    assert!(cel.raster.has_beyond());
    doc.annuler_plan(id).unwrap();
    let (dir, manifest) = enregistrer(&doc, "hors");
    let lu = project::load(&manifest).expect("réouverture");
    let relu = lu.doc.cels.get(&c).expect("le dessin relu");
    assert_eq!(relu.raster.get_i(-64, -64), rouge, "le dessin hors de la feuille est relu");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_bar_before_a_cancelled_case_joins_nothing() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let mut b = dans_les_vignettes(film());
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    clic_numero(&mut b, ids[1]);
    assert_eq!(b.app.board_layout().barres_mortes, vec![0]);
    let z = crate::vignettes_tests::barre(&b, 0).center();
    let etapes = pas_d_annulation(&b);
    b.app.set_tool(Tool::Eraser);
    clic_avec(&mut b, z, Modifiers::NONE);
    b.chauffer(2);
    assert_eq!(b.app.doc.panels.len(), 3);
    assert_eq!(pas_d_annulation(&b), etapes, "rien de réuni");
    assert!(b.app.status.contains("annulée"), "{}", b.app.status);
}

#[test]
fn all_cancelled_survives_a_reopen_too() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let mut doc = film();
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    for &i in &ids {
        doc.annuler_plan(i).unwrap();
    }
    assert!(doc.film_vide());
    let (dir, manifest) = enregistrer(&doc, "tout");
    let lu = project::load(&manifest).expect("réouverture");
    assert!(lu.doc.film_vide(), "{:?}", lu.warnings);
    assert_eq!(ordre(&lu.doc), ids);
    let mut b = dans_les_vignettes(lu.doc);
    let l = b.app.board_layout();
    assert_eq!((l.cards.len(), l.annulees.len()), (0, 4));
    clic_numero(&mut b, ids[0]);
    assert_eq!(b.app.doc.panels.len(), 1);
    assert!(!b.app.doc.film_vide());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_older_project_opens_as_is_and_an_unreadable_file_is_never_overwritten() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let doc = film();
    let (dir, manifest) = enregistrer(&doc, "ancien");
    // Un projet 47 : aucun fichier, le numéro d'avant.
    let texte = std::fs::read_to_string(&manifest).unwrap().replace("\"version\": 48", "\"version\": 47");
    std::fs::write(&manifest, texte).unwrap();
    let lu = project::load(&manifest).expect("un projet 47 s'ouvre");
    assert!(lu.doc.annules.is_empty());
    assert_eq!(lu.doc.panels.len(), 4);
    // Un projet plus récent est refusé, comme toujours.
    let texte = std::fs::read_to_string(&manifest).unwrap().replace("\"version\": 47", "\"version\": 49");
    std::fs::write(&manifest, texte).unwrap();
    assert!(project::load(&manifest).is_err(), "une version plus récente refuse");
    std::fs::remove_dir_all(&dir).ok();

    // Un fichier illisible : on le dit, le projet s'ouvre, le fichier reste tel quel et aucun dessin n'est nettoyé.
    let mut doc = film();
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    let garde: Vec<u64> = doc.panels[1].layers.iter().flat_map(|l| l.frames.iter().flatten().copied()).collect();
    doc.annuler_plan(ids[1]).unwrap();
    let (dir, manifest) = enregistrer(&doc, "illisible");
    let fichier = dir.join(crate::planannule::FILE);
    std::fs::write(&fichier, "{ pas du json").unwrap();
    let lu = project::load(&manifest).expect("le projet s'ouvre");
    assert!(lu.warnings.iter().any(|w| w.contains(crate::planannule::FILE)), "{:?}", lu.warnings);
    assert!(lu.doc.annules.is_empty());
    project::save(&lu.doc, &manifest, &mut lu.written.clone()).unwrap();
    assert_eq!(std::fs::read_to_string(&fichier).unwrap(), "{ pas du json");
    let dessins = dir.join("dessins");
    let noms: Vec<String> = std::fs::read_dir(&dessins).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert!(garde.iter().all(|c| noms.iter().any(|n| n.starts_with(&c.to_string()))), "le dessin du plan est toujours là : {noms:?}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_shared_film_refuses_the_gesture_and_a_film_with_cancelled_plans_refuses_to_be_shared() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let mut b = dans_les_vignettes(film());
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    clic_numero(&mut b, ids[1]);
    assert!(b.app.partage_refuse_annules(), "un film qui a des plans annulés ne se partage pas");
    assert!(b.app.status.contains("rétablis"), "{}", b.app.status);
    clic_numero(&mut b, ids[1]);
    assert!(!b.app.partage_refuse_annules());
}

#[test]
fn locked_numbers_are_kept_and_not_given_away() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let mut doc = film();
    doc.lock_numbering();
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    doc.annuler_plan(ids[3]).unwrap();
    // Un plan neuf à la fin ne reprend pas le 4 du plan annulé.
    let n = doc.add_scene_after(2);
    assert_ne!(doc.plan_code(doc.panels[n].scene), "4");
    assert_eq!(doc.scene_label(plan_de(&doc, ids[3]).scene), "4");
    doc.retablir_plan(ids[3]).unwrap();
    assert_eq!(doc.scene_label(plan_de(&doc, ids[3]).scene), "4", "il revient avec son numéro");
    // « Renuméroter » garde la place d'un plan annulé.
    let mut doc = film();
    doc.lock_numbering();
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    doc.annuler_plan(ids[1]).unwrap();
    doc.renumber();
    assert_eq!(ids.iter().map(|&i| etiquette(&doc, i)).collect::<Vec<_>>(), vec!["1", "2", "3", "4"]);
    // Verrouiller ne change rien à l'écran.
    let mut doc = film();
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    doc.annuler_plan(ids[0]).unwrap();
    let avant: Vec<String> = ids.iter().map(|&i| etiquette(&doc, i)).collect();
    doc.lock_numbering();
    assert_eq!(ids.iter().map(|&i| etiquette(&doc, i)).collect::<Vec<_>>(), avant);
    // `Numeros::of` dit la même chose que le document (une passe, l'écran).
    let num = crate::numerotation::Numeros::of(&doc);
    for &i in &ids {
        let s = plan_de(&doc, i).scene;
        assert_eq!(num.scene_label(&doc, s), doc.scene_label(s));
    }
}

// ------------------------------------------------------------------------------ une famille d'export à la fois

/// Le film avec son plan 2 annulé, et ce que le film d'origine dirait s'il ne l'avait jamais eu (pour la durée).
fn film_sans_le_plan_2() -> (Document, u64) {
    let mut doc = film();
    let id = doc.panels[1].id;
    doc.annuler_plan(id).unwrap();
    (doc, id)
}

fn tout(doc: &Document) -> Vec<usize> {
    (0..doc.panels.len()).collect()
}

fn courir(job: &mut crate::export::ExportJob, doc: &Document) {
    let t = std::time::Instant::now();
    while job.outcome.is_none() {
        job.step(doc, std::time::Duration::from_millis(20));
        std::thread::sleep(std::time::Duration::from_millis(2));
        assert!(t.elapsed() < std::time::Duration::from_secs(60), "export trop long");
    }
}

#[test]
fn export_video_and_image_sequence() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    use crate::export::{self, ExportJob, Range};
    let (doc, _) = film_sans_le_plan_2();
    let frames = export::frames_for(&doc, Range::Montage, 0);
    assert_eq!(frames.len(), 3 * LONGUEUR, "les images du film : ni celles du plan annulé");
    let dir = dossier("images");
    let mut job = ExportJob::images(&dir, "essai", frames, doc.frame, true);
    courir(&mut job, &doc);
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 3 * LONGUEUR);
    std::fs::remove_dir_all(&dir).ok();
    // La vidéo, si ffmpeg est là.
    if let Some(ffmpeg) = export::find_ffmpeg(None) {
        if let Ok(enc) = export::probe(&ffmpeg) {
            if enc.h264.is_some() {
                let dir = dossier("video");
                let out = dir.join("essai.mp4");
                let frames = export::frames_for(&doc, Range::Montage, 0);
                let n = frames.len();
                let args = export::video_args(export::Format::Mp4, &enc, doc.frame, doc.fps, false, None, &out).unwrap();
                let mut job = ExportJob::video(&ffmpeg, args, &out, frames, None).unwrap();
                courir(&mut job, &doc);
                assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
                assert!(n == 3 * LONGUEUR && std::fs::metadata(&out).unwrap().len() > 500);
                std::fs::remove_dir_all(&dir).ok();
            }
        }
    }
}

#[test]
fn export_images_by_plan_and_by_key() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let (doc, _) = film_sans_le_plan_2();
    let plans = tout(&doc);
    let par_plan = crate::exportstills::items(&doc, &plans, crate::exportstills::StillsPer::Plan);
    assert_eq!(par_plan.len(), 3, "une image par plan du film");
    let (cles, _) = crate::exportcles::compter(&doc, &plans, false);
    assert_eq!(cles, 3, "une clé par plan du film");
    assert_eq!(crate::exportcles::items(&doc, &plans, false).len(), 3);
    // Les noms gardent les numéros de plan du film : le 2 manque, le 3 reste le 3.
    let noms: Vec<String> = plans.iter().map(|&i| doc.plan_ref(doc.panels[i].scene)).collect();
    assert_eq!(noms, vec!["PL001", "PL003", "PL004"]);
}

#[test]
fn export_sound_by_plan() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let (mut doc, _) = film_sans_le_plan_2();
    let pcm = crate::audio::Pcm::from_interleaved(&vec![0.2; 48_000 * 2 * 3], 2, crate::audio::RATE);
    let son = doc.add_sound("essai.wav".into(), "wav".into(), std::sync::Arc::new(Vec::new()), std::sync::Arc::new(pcm));
    doc.place_sound(son, 0, None).unwrap();
    assert_eq!(crate::exportsonplan::compter(&doc, &tout(&doc)), 3);
    assert_eq!(crate::exportsonplan::plan(&doc, &tout(&doc), Default::default()).len(), 3);
}

#[test]
fn export_color_script_board_and_layers() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let (doc, id) = film_sans_le_plan_2();
    let plans = tout(&doc);
    let spec = crate::colorscript::construire(&doc, &plans, &crate::colorscript::Options::default());
    assert_eq!(spec.blocs.iter().map(|b| b.entrees.len()).sum::<usize>(), 3, "le color script : une image par plan du film");
    assert_eq!(crate::planchesequences::plans_affiches(&doc).len(), 3, "« tout » sur la planche d'export");
    let l = crate::planchesequences::layout_export(&doc, &crate::board::BoardSettings::default(), &crate::board::Options::default());
    assert_eq!(l.cards.len(), 3);
    assert!(l.annulees.is_empty() && !l.cards.iter().any(|c| c.plan == id), "la planche exportée ne le montre pas, barré ou non");
    assert_eq!(crate::exportlayers::plans_info(&doc, &plans, false).len(), 3);
    let camera = crate::exportcamera::build(&doc, &plans, "essai", &Default::default());
    let lu: serde_json::Value = serde_json::from_str(&camera.json).expect("le JSON de la caméra");
    assert_eq!(lu["plans"].as_array().map(Vec::len), Some(3), "les clés de caméra");
}

#[test]
fn export_for_editing_sheet_and_shot_list() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let (doc, _) = film_sans_le_plan_2();
    let plan = crate::editorial::build_plan(&doc, &crate::editorial::Options::default(), 0, "essai").expect("le montage");
    assert_eq!(plan.clips.len(), 3, "les clips du montage");
    let lignes = crate::listeplans::construire(&doc, &tout(&doc), 0, None, false);
    assert_eq!(lignes.len(), 3);
    let texte = crate::listeplans::json(&doc, &lignes, crate::listeplans::Profil::Generique, 0);
    assert!(!texte.contains("ACTION1") && !texte.contains("DIALOGUE1"), "le texte du plan annulé n'y est pas");
    let fiche = crate::fiche::build(&doc, &crate::fiche::Entree { plans: &tout(&doc), colonnes: &crate::fiche::Colonne::TOUTES, par_image: false, start_tc: 0, etats: None, envoi: None }).expect("la fiche");
    assert_eq!(fiche.lignes.len(), 3);
    // Le PDF lit les mêmes plans ; la durée du budget aussi.
    assert_eq!(doc.total_frames(), 3 * LONGUEUR);
}

#[test]
fn delivery_package_and_kitsu_leave_the_cancelled_plan_out_and_delete_nothing() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let (doc, id) = film_sans_le_plan_2();
    let cibles = crate::kitsu::cibles(&doc, &tout(&doc), 0, &Default::default());
    assert_eq!(cibles.len(), 3, "Kitsu : on n'envoie pas le plan");
    assert!(!cibles.iter().any(|c| c.panel == id));
    // Aucun chemin d'effacement vers le serveur : le pont ne connaît que créer et mettre à jour.
    let source = include_str!("kitsu.rs");
    assert!(!source.contains("\"DELETE\"") && !source.contains(".delete("), "le pont Kitsu n'efface jamais rien sur le serveur");
    // Le paquet de livraison passe par les mêmes rangs du film.
    let source = include_str!("paquet.rs");
    assert!(!source.contains("annules"), "le paquet ne lit que `panels`");
}

// ------------------------------------------------------------------------------ la capture

#[test]
#[ignore = "écrit une capture : FEANOR_CAPTURE_DIR"]
fn plan_annule_captures() {
    // (Ces essais calculent, exportent et enregistrent : ils tiennent le « monde » partagé des effets, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    use crate::traits_continus_tests::{planche, repos};
    let Some(dir) = std::env::var_os("FEANOR_CAPTURE_DIR").map(PathBuf::from) else {
        eprintln!("FEANOR_CAPTURE_DIR absent");
        return;
    };
    std::fs::create_dir_all(&dir).expect("dossier");
    let mut doc = film();
    // Des dessins qui se voient.
    for i in 0..4 {
        let c = doc.panels[i].layers[0].frames[0].unwrap();
        let cel = doc.cels.get_mut(&c).unwrap();
        for t in 0..cel.raster.tiles.len().min(400) {
            let px = cel.raster.tile_mut(t);
            for p in px.iter_mut().step_by(3) {
                *p = eframe::egui::Color32::from_rgba_premultiplied(30 + 50 * i as u8, 60, 170 - 30 * i as u8, 255);
            }
        }
        cel.version += 1;
    }
    let Some(mut b) = planche(doc, 4, 1.0) else { return };
    b.app.set_espace(Espace::Vignettes);
    b.app.settings.vignettes.numeros_visibles = true;
    b.chauffer(6);
    let sauver = |b: &Banc, nom: &str| {
        let rgba = b.capturer().expect("capture");
        image::save_buffer(dir.join(nom), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
    };
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    repos(&mut b);
    sauver(&b, "plan-annule-0-avant.png");
    clic_numero(&mut b, ids[1]);
    repos(&mut b);
    sauver(&b, "plan-annule-1-croix.png");
    b.app.settings.vignettes.numeros_visibles = false;
    b.chauffer(2);
    repos(&mut b);
    sauver(&b, "plan-annule-2-numeros-caches.png");
    b.touches = Modifiers::SHIFT;
    b.image(vec![Event::PointerMoved(vignette(&b, 0).center())], 1.0 / 60.0);
    b.chauffer(3);
    sauver(&b, "plan-annule-3-maj-tenue.png");
    b.touches = Modifiers::NONE;
    b.app.settings.vignettes.numeros_visibles = true;
    for &i in &[ids[0], ids[2], ids[3]] {
        clic_numero(&mut b, i);
    }
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(6);
    sauver(&b, "plan-annule-4-storyboard-vide.png");
}
