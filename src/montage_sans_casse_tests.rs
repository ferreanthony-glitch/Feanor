//! La règle du réalisateur pour la V1 : « les outils de montage ne doivent pas permettre de casser le travail fait en storyboard et
//! vignettes ». Un outil de montage change le temps et l'ordre, pas les dessins ; ce qu'il retire du film reste récupérable ; chaque
//! geste s'annule en une seule étape (partie 104 de `docs/INTERFACE.md`).
//!
//! Chaque essai prend un geste du montage et compare l'**empreinte du contenu** des dessins (`empreinte::Donnees`, celle des envois)
//! avant, après, et après l'annulation : l'inventaire est l'ensemble des empreintes des dessins que montre le film ou que garde une
//! réserve de rognage. Un dessin qui change d'identifiant mais pas de contenu (une copie, comme après « Couper le plan ») compte pour
//! le même.

use std::collections::BTreeSet;

use eframe::egui::{self, Color32};

use crate::app::App;
use crate::document::{Cel, Document, Layer, LayerKind};
use crate::empreinte::Donnees;
use crate::rognage::{Reserve, Rogner};
use crate::workspace::Espace;

const LONGUEUR: usize = 12;

fn couleur(k: usize) -> Color32 {
    Color32::from_rgba_premultiplied(10 + k as u8 * 9, 30, 200, 255)
}

/// Quatre plans de 12 images dans une seule séquence ; chacun a deux calques (« Dessin » : un dessin pour trois images, « Fond » :
/// un seul dessin) ; chaque dessin a un contenu à lui (une couleur de pixel qui dépend de son rang).
fn film() -> Document {
    let mut doc = Document::with_frame(64, 48, 24);
    for i in 0..3 {
        doc.add_scene_after(i);
    }
    let mut k = 0;
    for i in 0..4 {
        doc.panels[i].layers.clear();
        doc.panels[i].set_length(LONGUEUR);
        let mut dessin = Layer::new("Dessin", LayerKind::Bitmap);
        for d in 0..4 {
            let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
            cel.raster.tile_mut(0)[0] = couleur(k);
            k += 1;
            let c = doc.add_cel(cel);
            for f in d * 3..d * 3 + 3 {
                dessin.set(f, Some(c));
            }
        }
        let mut fond = Layer::new("Fond", LayerKind::Bitmap);
        let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
        cel.raster.tile_mut(1)[0] = couleur(k);
        k += 1;
        let c = doc.add_cel(cel);
        fond.frames = vec![Some(c); LONGUEUR];
        doc.panels[i].layers.push(dessin);
        doc.panels[i].layers.push(fond);
    }
    doc
}

fn appli(nom: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-montage-sans-casse-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = film();
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app.set_espace(Espace::Montage);
    app
}

/// L'empreinte de chaque dessin que montre le film ou que garde une réserve de rognage.
fn inventaire(doc: &Document) -> BTreeSet<u64> {
    let mut ids = doc.used_cels();
    for r in doc.reserves.values() {
        ids.extend(Reserve::cels(r));
    }
    ids.into_iter().filter_map(|id| doc.cels.get(&id)).map(|c| Donnees::de(c).empreinte()).collect()
}

/// Ce que montre chaque image de chaque calque de chaque plan, par empreinte (dans l'ordre du film).
fn film_en_empreintes(doc: &Document) -> Vec<Vec<Vec<Option<u64>>>> {
    let h = |id: &u64| doc.cels.get(id).map(|c| Donnees::de(c).empreinte());
    doc.panels.iter().map(|p| p.layers.iter().map(|l| l.frames.iter().map(|f| f.as_ref().and_then(h)).collect()).collect()).collect()
}

fn annuler(app: &mut App) {
    app.run_action("edition.annuler");
}

const NOM: &str = "Essai";

#[test]
fn trimming_a_plan_keeps_every_drawing_and_lengthening_gives_the_frames_back() {
    for (nom, op, inverse) in [
        ("début", Rogner::Head(5), Rogner::Head(-5)),
        ("fin", Rogner::Tail(-5), Rogner::Tail(5)),
        ("durée", Rogner::Duration(3), Rogner::Tail(9)),
    ] {
        let mut app = appli(nom);
        let (avant, inventaire0) = (film_en_empreintes(&app.doc), inventaire(&app.doc));
        let n = app.history.undo_len();
        assert!(app.rogner_plan(1, op, NOM).is_some_and(|d| d != 0), "{nom}");
        assert_eq!(app.history.undo_len(), n + 1, "{nom} : une seule étape");
        assert_eq!(inventaire(&app.doc), inventaire0, "{nom} : aucun dessin perdu (ils sont dans le film ou la réserve)");
        assert_ne!(film_en_empreintes(&app.doc), avant, "{nom} : le plan a bien changé");
        // Récupérable sans Ctrl+Z : rallonger rend exactement les images coupées.
        assert!(app.rogner_plan(1, inverse, NOM).is_some());
        assert_eq!(film_en_empreintes(&app.doc), avant, "{nom} : rallonger rend les images coupées, les mêmes dessins");
        assert!(app.doc.reserves.is_empty(), "{nom} : la réserve est vide, rien ne reste caché");
        // Et l'annulation, un seul pas pour chaque geste.
        annuler(&mut app);
        annuler(&mut app);
        assert_eq!(film_en_empreintes(&app.doc), avant, "{nom} : annulé");
        assert_eq!(inventaire(&app.doc), inventaire0);
    }
}

#[test]
fn ripple_roll_and_slip_keep_every_drawing_and_each_is_one_undo_step() {
    for (nom, pi, op) in [("ripple", 0, Rogner::Tail(-4)), ("roll", 1, Rogner::Roll(4)), ("slip", 1, Rogner::Slip(0))] {
        let mut app = appli(nom);
        // Le glisser a besoin d'une réserve : on rogne d'abord les deux bords du plan 1.
        app.rogner_plan(1, Rogner::Head(3), NOM);
        app.rogner_plan(1, Rogner::Tail(-3), NOM);
        let op = if let Rogner::Slip(_) = op { Rogner::Slip(2) } else { op };
        let (avant, inv0) = (film_en_empreintes(&app.doc), inventaire(&app.doc));
        let n = app.history.undo_len();
        assert!(app.rogner_plan(pi, op, NOM).is_some_and(|d| d != 0), "{nom}");
        assert_eq!(app.history.undo_len(), n + 1, "{nom} : une seule étape");
        assert_eq!(inventaire(&app.doc), inv0, "{nom} : aucun dessin perdu");
        annuler(&mut app);
        assert_eq!(film_en_empreintes(&app.doc), avant, "{nom} : annulé");
    }
}

#[test]
fn moving_plans_only_changes_their_order() {
    let mut app = appli("deplacer");
    let (avant, inv0) = (film_en_empreintes(&app.doc), inventaire(&app.doc));
    let ids: Vec<u64> = app.doc.panels.iter().map(|p| p.id).collect();
    let n = app.history.undo_len();
    assert!(app.move_plans_before(&[ids[0]], Some(ids[3])));
    assert_eq!(app.history.undo_len(), n + 1);
    assert_eq!(inventaire(&app.doc), inv0);
    let apres = film_en_empreintes(&app.doc);
    let mut tries = apres.clone();
    let mut orig = avant.clone();
    tries.sort();
    orig.sort();
    assert_eq!(tries, orig, "les mêmes plans, dans un autre ordre");
    annuler(&mut app);
    assert_eq!(film_en_empreintes(&app.doc), avant);
}

#[test]
fn cutting_a_plan_keeps_every_drawing_and_the_hidden_end_goes_with_the_second_half() {
    let mut app = appli("couper");
    // La fin du plan 0 est rognée de 4 images : elle est en réserve.
    app.rogner_plan(0, Rogner::Tail(-4), NOM);
    let (avant, inv0) = (film_en_empreintes(&app.doc), inventaire(&app.doc));
    assert_eq!(app.doc.hidden(0), (0, 4));
    app.cur_panel = 0;
    app.cur_frame = 4;
    let n = app.history.undo_len();
    app.split_current_panel();
    assert_eq!(app.doc.panels.len(), 5, "coupé en deux");
    assert_eq!(app.history.undo_len(), n + 1, "une seule étape");
    assert_eq!(inventaire(&app.doc), inv0, "aucun dessin perdu, le contenu rogné compris");
    // Le contenu rogné à la fin du plan est la fin de sa seconde moitié : on peut le rallonger.
    assert_eq!(app.doc.hidden(1), (0, 4), "la réserve de la fin suit la seconde moitié");
    assert_eq!(app.doc.hidden(0), (0, 0));
    app.rogner_plan(1, Rogner::Tail(4), NOM);
    assert_eq!(app.doc.panels[1].frame_count(), 4 + 4, "les images rognées sont revenues");
    app.rogner_plan(1, Rogner::Tail(-4), NOM);
    annuler(&mut app);
    annuler(&mut app);
    annuler(&mut app);
    assert_eq!(film_en_empreintes(&app.doc), avant, "annulé");
    assert_eq!(app.doc.hidden(0), (0, 4), "la réserve est revenue au premier plan");
}

#[test]
fn joining_two_plans_keeps_every_drawing_and_what_was_hidden_at_the_ends() {
    let mut app = appli("reunir");
    // Début caché du premier plan, fin cachée du second : ce sont des bords du plan réuni, ils se gardent.
    app.rogner_plan(0, Rogner::Head(2), NOM);
    app.rogner_plan(1, Rogner::Tail(-3), NOM);
    let (avant, inv0) = (film_en_empreintes(&app.doc), inventaire(&app.doc));
    let n = app.history.undo_len();
    let (a, b) = (app.doc.panels[0].id, app.doc.panels[1].id);
    let un = app.doc.panels.len();
    app.cur_panel = 0;
    assert!(app.structure_op_dessins(|d, _| d.join_plans(&[a, b])));
    assert_eq!(app.doc.panels.len(), un - 1);
    assert_eq!(app.history.undo_len(), n + 1, "une seule étape");
    assert_eq!(inventaire(&app.doc), inv0, "aucun dessin perdu : le contenu rogné aux bords du plan réuni reste");
    annuler(&mut app);
    assert_eq!(film_en_empreintes(&app.doc), avant, "annulé");
}

#[test]
fn joining_plans_whose_trimmed_frames_would_end_up_inside_says_what_it_forgets() {
    use crate::fusionnoms::{JoinOpts, Jonction};
    // Fin cachée du premier plan et début caché du second : après la jonction ils seraient au milieu du plan réuni, où la réserve
    // (un début, une fin) ne peut pas les tenir. Ils sont oubliés, la barre d'état dit combien d'images, et une étape d'annulation
    // les rend. (Les rendre au film allongerait le plan réuni de ce que l'on avait choisi de couper.)
    let mut app = appli("reunir-milieu");
    app.rogner_plan(0, Rogner::Tail(-3), NOM);
    app.rogner_plan(1, Rogner::Head(2), NOM);
    let (avant, inv0) = (film_en_empreintes(&app.doc), inventaire(&app.doc));
    let a = app.doc.panels[0].id;
    let n = app.history.undo_len();
    app.cur_panel = 0;
    assert!(app.jonction_faire(Jonction::Suivant { a, precedent: false }, &JoinOpts::default()));
    assert_eq!(app.history.undo_len(), n + 1, "une seule étape");
    assert_eq!(app.doc.panels[0].frame_count(), 9 + 10, "la durée des deux plans, comme l'écran la montrait");
    assert!(app.status.contains("5 images rognées") && app.status.contains("Ctrl+Z"), "{}", app.status);
    assert!(inventaire(&app.doc).len() < inv0.len(), "le dessin des images cachées au milieu est sorti");
    annuler(&mut app);
    assert_eq!(film_en_empreintes(&app.doc), avant, "annulé : les deux plans, leurs réserves");
    assert_eq!(inventaire(&app.doc), inv0);
    assert_eq!(app.doc.hidden(0), (0, 3));
    assert_eq!(app.doc.hidden(1), (2, 0));
}
#[test]
fn merging_by_dragging_keeps_every_drawing_and_one_undo_gives_the_two_plans_back() {
    let mut app = appli("glisser");
    let (avant, inv0) = (film_en_empreintes(&app.doc), inventaire(&app.doc));
    let (x, y) = (app.doc.panels[0].id, app.doc.panels[2].id);
    let n = app.history.undo_len();
    app.cur_panel = 0;
    assert!(app.structure_op_dessins(|d, _| d.merge_plan_into(x, y)));
    assert_eq!(app.doc.panels.len(), 3);
    assert_eq!(app.history.undo_len(), n + 1);
    assert_eq!(inventaire(&app.doc), inv0);
    annuler(&mut app);
    assert_eq!(film_en_empreintes(&app.doc), avant);
}

#[test]
fn transitions_never_touch_a_drawing() {
    let mut app = appli("transitions");
    let (avant, inv0) = (film_en_empreintes(&app.doc), inventaire(&app.doc));
    let id = app.doc.set_transition(1, crate::transitions::Kind::Dissolve).expect("une transition");
    assert_eq!(film_en_empreintes(&app.doc), avant);
    assert!(app.doc.remove_transition(id));
    assert_eq!(film_en_empreintes(&app.doc), avant);
    assert_eq!(inventaire(&app.doc), inv0);
}

#[test]
fn deleting_plans_tells_what_goes_and_one_undo_gives_it_back() {
    let mut app = appli("supprimer-plan");
    let (avant, inv0) = (film_en_empreintes(&app.doc), inventaire(&app.doc));
    let ids = [app.doc.panels[1].id, app.doc.panels[2].id];
    let n = app.history.undo_len();
    app.delete_panels(&ids);
    assert_eq!(app.doc.panels.len(), 2);
    assert_eq!(app.history.undo_len(), n + 1, "une seule étape");
    assert!(inventaire(&app.doc).len() < inv0.len(), "des dessins quittent le film");
    // La perte se voit : la barre d'état dit combien de dessins partent et comment les reprendre.
    assert!(app.status.contains("10 dessins") && app.status.contains("Ctrl+Z"), "{}", app.status);
    annuler(&mut app);
    assert_eq!(film_en_empreintes(&app.doc), avant, "annulé : exactement les mêmes plans");
    assert_eq!(inventaire(&app.doc), inv0);
}

#[test]
fn deleting_a_layer_tells_what_goes_and_one_undo_gives_it_back() {
    let mut app = appli("supprimer-calque");
    let (avant, inv0) = (film_en_empreintes(&app.doc), inventaire(&app.doc));
    let n = app.history.undo_len();
    app.cur_layer = 0;
    app.delete_chosen_layers();
    assert_eq!(app.doc.panels[0].layers.len(), 1);
    assert_eq!(app.history.undo_len(), n + 1);
    assert!(inventaire(&app.doc).len() < inv0.len());
    assert!(app.status.contains("4 dessins") && app.status.contains("Ctrl+Z"), "{}", app.status);
    annuler(&mut app);
    assert_eq!(film_en_empreintes(&app.doc), avant);
}

#[test]
fn an_alt_drag_that_lands_on_a_drawing_says_so_and_one_undo_gives_it_back() {
    use crate::handles::KeyMove;
    use crate::montagekeys::KeyRef;
    let mut app = appli("alt");
    let (avant, inv0) = (film_en_empreintes(&app.doc), inventaire(&app.doc));
    let (panel, layer) = (app.doc.panels[0].id, app.doc.panels[0].layers[0].id);
    let n = app.history.undo_len();
    // La clé de l'image 0 (3 images) dupliquée 3 images plus loin : elle se pose sur le dessin de l'image 3.
    app.key_sel = vec![KeyRef { panel, layer, start: 0 }];
    assert!(app.begin_keys_drag(0));
    app.drag_keys_mode(3, KeyMove::Copy);
    app.end_keys_drag();
    assert_eq!(app.history.undo_len(), n + 1, "une seule étape");
    assert!(inventaire(&app.doc).len() < inv0.len(), "le dessin d'en dessous est sorti du calque");
    assert!(app.status.contains("Attention") && app.status.contains("1 dessin") && app.status.contains("Ctrl+Z"), "{}", app.status);
    annuler(&mut app);
    assert_eq!(film_en_empreintes(&app.doc), avant);
}