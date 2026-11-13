//! Essais du lasso dans la vue Espace 3D (`espace3dlasso.rs`, `docs/INTERFACE.md` partie 152) : de vrais gestes de souris
//! dans la vue, sur la carte du calque courant. Sur un calque bitmap : tracer un lasso choisit, glisser ce qui est choisi
//! le déplace (et il se voit sur la carte pendant le geste), Suppr, couper, copier, coller, tout désélectionner, Échap et
//! l'annulation ; sur un calque vectoriel : le lasso prend des traits, la boîte les déplace, les agrandit et les tourne ;
//! Ctrl tenu avec un outil de dessin : le lasso, et Ctrl+clic prend toujours une carte ; tourner la vue reste sur
//! Alt+glisser ; un outil de la vue pose ce qui est choisi.

use eframe::egui::{pos2, vec2, Color32, Event, Key, Modifiers, PointerButton, Pos2, Rect};

use crate::app::Tool;
use crate::document::{CelId, Document, Layer, LayerKind, LayerTransform};
use crate::espace3d;
use crate::espace3ddessin::{CarteVue, Regard3d};
use crate::perf_tests::{attente, banc_taille, Banc};
use crate::pixtools::BDrag;
use crate::raster::TILE;
use crate::selbox::Handle;
use crate::vtools::VDrag;

/// Un projet d'essai : cadre 320 × 180 sur une feuille 640 × 360, `n` calques de 12 images, « Carte 0 » en bas.
fn doc_cartes(n: usize, kind: LayerKind) -> Document {
    let mut doc = Document::with_frame(320, 180, 24);
    doc.panels[0].layers = Vec::new();
    for k in 0..n {
        let mut layer = Layer::new(format!("Carte {k}"), kind);
        let cel = doc.new_cel(kind);
        layer.frames = vec![Some(cel); 12];
        doc.panels[0].layers.push(layer);
    }
    doc
}

/// Un carré noir de `n` pixels de côté, au point (`x0`, `y0`) du dessin `cel`.
fn peindre(doc: &mut Document, cel: CelId, x0: usize, y0: usize, n: usize) {
    let r = &mut doc.cels.get_mut(&cel).expect("le dessin").raster;
    for y in y0..y0 + n {
        for x in x0..x0 + n {
            let idx = r.tile_index_of(x, y);
            r.tile_mut(idx)[(y % TILE) * TILE + (x % TILE)] = Color32::BLACK;
        }
    }
}

/// Deux cartes bitmap ; sur la carte 0, deux carrés de 20 pixels : A en (200, 140) et B en (420, 220).
fn doc_deux_carres() -> Document {
    let mut doc = doc_cartes(2, LayerKind::Bitmap);
    let cel = doc.panels[0].layers[0].cel_at(0).expect("un dessin");
    peindre(&mut doc, cel, 200, 140, 20);
    peindre(&mut doc, cel, 420, 220, 20);
    doc
}

/// Le banc, la vue Espace 3D ouverte, le calque 0 courant.
fn ouvert(doc: Document) -> Banc {
    let mut b = banc_taille(doc, false, 1280, 720);
    b.chauffer(2);
    b.app.cur_layer = 0;
    b.app.run_action("espace3d.basculer");
    b.chauffer(4);
    assert!(b.app.espace3d.on && !b.app.camera_view, "la vue Espace 3D est ouverte");
    b
}

/// Prend l'outil Sélection dans la vue : le pointeur sélectionne.
fn selectionner(b: &mut Banc) {
    b.app.run_action("outil.selection");
    b.chauffer(2);
    assert!(b.app.e3_selectionne() && !b.app.e3_dessine() && !b.app.e3_trace(), "l'outil Sélection pris dans la vue a le pointeur ({})", b.app.status);
}

/// La carte du calque courant, vue par la vue.
fn carte(b: &Banc) -> CarteVue {
    b.app.e3_carte_vue().expect("un outil a le pointeur dans la vue, sur une carte")
}

/// Le point de l'écran où la vue montre le point (`x`, `y`) du dessin de la carte du calque courant.
fn ecran(b: &Banc, x: f32, y: f32) -> Pos2 {
    let q = carte(b).vers_ecran(pos2(x, y)).unwrap_or_else(|| panic!("le point ({x}, {y}) de la carte se voit"));
    assert!(b.app.canvas_rect.shrink(4.0).contains(q), "le point ({x}, {y}) de la carte tombe en {q:?}, hors de la vue {:?}", b.app.canvas_rect);
    q
}

/// Le tour du rectangle `r` du dessin de la carte, en points de l'écran : un lasso qui l'entoure.
fn tour(b: &Banc, r: Rect) -> Vec<Pos2> {
    [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top() + vec2(0.0, 3.0)].iter().map(|p| ecran(b, p.x, p.y)).collect()
}

/// Le début d'un geste : appui au premier point, puis le pointeur passe par les suivants, bouton tenu.
fn appuyer(b: &mut Banc, pts: &[Pos2], mods: Modifiers) {
    b.touches = mods;
    b.image(vec![Event::PointerMoved(pts[0])], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: pts[0], button: PointerButton::Primary, pressed: true, modifiers: mods }], 1.0 / 60.0);
    for w in pts.windows(2) {
        for k in 0..6 {
            let ev = (0..3).map(|j| Event::PointerMoved(w[0] + (w[1] - w[0]) * ((k * 3 + j + 1) as f32 / 18.0))).collect();
            b.image(ev, 1.0 / 60.0);
        }
    }
}

fn lacher(b: &mut Banc, ou: Pos2, mods: Modifiers) {
    b.image(vec![Event::PointerButton { pos: ou, button: PointerButton::Primary, pressed: false, modifiers: mods }], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.chauffer(2);
}

/// Un geste de souris qui passe par ces points de l'écran, touches `mods` tenues : appui au premier, relâché au dernier.
fn geste(b: &mut Banc, pts: &[Pos2], mods: Modifiers) {
    appuyer(b, pts, mods);
    lacher(b, *pts.last().expect("un point au moins"), mods);
}

/// Le dessin du calque `li` à l'image courante a de l'encre au point (`x`, `y`).
fn encre(b: &Banc, li: usize, x: i64, y: i64) -> bool {
    let Some(cel) = b.app.layers()[li].shown_at(b.app.cur_frame) else { return false };
    b.app.doc.cels[&cel].raster.get_i(x, y).a() > 0
}

/// Le nombre de pixels encrés du dessin du calque `li` à l'image courante.
fn pixels(b: &Banc, li: usize) -> usize {
    let Some(cel) = b.app.layers()[li].shown_at(b.app.cur_frame) else { return 0 };
    let raster = &b.app.doc.cels[&cel].raster;
    let (w, h) = (b.app.doc.width as i64, b.app.doc.height as i64);
    (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| raster.get_i(x, y).a() > 0).count()
}

/// Attend que la zone bitmap déplacée ait ses pixels posés à leur place (le rendu exact arrive d'un fil de travail).
fn attendre_la_pose(b: &mut Banc) {
    for _ in attente() {
        if !b.app.bsel.as_ref().and_then(|s| s.lifted.as_ref()).is_some_and(|l| l.stale) {
            break;
        }
        b.chauffer(1);
    }
    b.chauffer(2);
}

/// Un point du dessin, dans la boîte de ce qui est choisi, où un appui prend l'intérieur (ni une poignée, ni un bord, ni
/// le pivot, qui est au milieu).
fn dans_la_boite(b: &Banc) -> Pos2 {
    let c = b.app.sel_frame().expect("la boîte de ce qui est choisi").corners();
    for (u, v) in [(0.28, 0.3), (0.72, 0.3), (0.72, 0.7), (0.28, 0.7), (0.5, 0.25), (0.25, 0.5)] {
        let (haut, bas) = (c[0] + (c[1] - c[0]) * u, c[3] + (c[2] - c[3]) * u);
        let p = haut + (bas - haut) * v;
        if b.app.box_hit(b.app.layer_to_screen(p)) == Some(Handle::Move) {
            return p;
        }
    }
    panic!("aucun point de la boîte ne prend l'intérieur : {c:?}");
}

fn touche(b: &mut Banc, key: Key) {
    for pressed in [true, false] {
        b.image(vec![Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    }
    b.chauffer(1);
}

const A: Rect = Rect { min: pos2(185.0, 125.0), max: pos2(240.0, 180.0) };
const B: Rect = Rect { min: pos2(405.0, 205.0), max: pos2(455.0, 255.0) };

#[test]
fn a_lasso_drawn_in_the_3d_view_selects_on_the_card_and_dragging_the_selection_moves_it() {
    let mut b = ouvert(doc_deux_carres());
    let cel = b.app.layers()[0].cel_at(0).expect("un dessin");
    selectionner(&mut b);
    assert_eq!(b.app.espace3d.sel, vec![espace3d::card_selection(b.app.layers()[0].id)], "la carte du calque courant est choisie");
    assert!(b.app.status.contains("Sélection sur la carte « Carte 0 »"), "{}", b.app.status);
    assert!(b.app.e3_selection_legende().contains("Sélection sur la carte « Carte 0 »"));
    // Ce que l'outil peint par-dessus le dessin est posé sur la carte : un point du dessin se voit là où la vue le montre.
    let vue = carte(&b);
    for p in [pos2(210.0, 150.0), pos2(430.0, 230.0), pos2(10.0, 350.0)] {
        assert!((b.app.layer_to_screen(p) - vue.vers_ecran(p).expect("devant l'œil")).length() < 0.01, "{p:?}");
    }
    let (etapes, encres) = (b.app.history.undo_len(), pixels(&b, 0));
    assert_eq!(encres, 800);

    // Un lasso autour du carré A. Pendant qu'on le trace, c'est le lasso de la feuille ; au relâché, la zone est choisie.
    let lasso = tour(&b, A);
    appuyer(&mut b, &lasso[..3], Modifiers::NONE);
    assert!(matches!(&b.app.bdrag, Some(BDrag::Lasso { points, .. }) if points.len() > 4), "le lasso se trace sur la carte ({})", b.app.status);
    lacher(&mut b, lasso[2], Modifiers::NONE);
    b.app.run_action("edition.tout_deselectionner");
    geste(&mut b, &lasso, Modifiers::NONE);
    let zone = b.app.bitmap_selection_bounds().unwrap_or_else(|| panic!("une zone est choisie ({})", b.app.status));
    assert!((zone.min - A.min).length() < 3.0 && (zone.max - A.max).length() < 3.0, "la zone entoure le carré A : {zone:?}");
    assert_eq!((b.app.history.undo_len(), pixels(&b, 0)), (etapes, encres), "choisir ne change rien au dessin, et n'est pas une étape");
    // Sa boîte est posée sur la carte : elle se prend là où la vue montre la zone (son milieu : le pivot ; un coin : la
    // poignée du coin ; l'intérieur : déplacer).
    assert_eq!(b.app.box_hit(ecran(&b, 212.5, 152.5)), Some(Handle::Pivot));
    assert_eq!(b.app.box_hit(ecran(&b, zone.max.x, zone.max.y)), Some(Handle::Corner(2)));
    assert_eq!(b.app.box_hit(ecran(&b, 330.0, 300.0)), None, "loin de la zone : rien");
    let pris = dans_la_boite(&b);
    let dedans = ecran(&b, pris.x, pris.y);

    // Glisser ce qui est choisi le déplace : 100 pixels du dessin vers la droite, 40 vers le bas.
    let vers = ecran(&b, pris.x + 100.0, pris.y + 40.0);
    appuyer(&mut b, &[dedans, vers], Modifiers::NONE);
    assert!(matches!(&b.app.bdrag, Some(BDrag::Transform { handle: Handle::Move, .. })));
    // Pendant le geste, la zone qu'on déplace se voit sur la carte : la feuille flottante y est posée, 100 × 40 plus loin.
    let f = b.app.e3_feuille_flottante(cel).expect("la zone qu'on déplace est posée sur la carte");
    let ou = f.aff.apply(pos2(210.0, 150.0));
    assert!((ou - pos2(310.0, 190.0)).length() < 2.0, "le carré se voit en {ou:?}");
    assert_eq!(f.taille, [640.0, 360.0]);
    lacher(&mut b, vers, Modifiers::NONE);
    attendre_la_pose(&mut b);
    assert!(b.app.e3_feuille_flottante(cel).is_none(), "le geste fini, le dessin a ses pixels : plus de feuille flottante");
    assert!(encre(&b, 0, 310, 190) && !encre(&b, 0, 210, 150), "le carré A a bougé ({})", b.app.status);
    assert!(encre(&b, 0, 430, 230), "le carré B n'a pas bougé");
    assert_eq!(pixels(&b, 0), encres);
    // « Tout désélectionner » pose la zone : une étape d'annulation ; l'annuler remet le carré où il était.
    b.app.run_action("edition.tout_deselectionner");
    b.chauffer(2);
    assert!(b.app.bsel.is_none());
    assert_eq!(b.app.history.undo_len(), etapes + 1, "le déplacement est une étape, comme à plat");
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert!(encre(&b, 0, 210, 150) && !encre(&b, 0, 310, 190), "l'annulation remet le carré A");
    assert_eq!(b.app.history.undo_len(), etapes);
}

#[test]
fn delete_cut_copy_paste_deselect_and_escape_act_on_what_the_lasso_chose_in_the_view() {
    let mut b = ouvert(doc_deux_carres());
    selectionner(&mut b);
    let (etapes, encres) = (b.app.history.undo_len(), pixels(&b, 0));
    // Suppr efface ce qui est choisi : le carré B.
    let lasso = tour(&b, B);
    geste(&mut b, &lasso, Modifiers::NONE);
    assert!(b.app.bsel.is_some(), "{}", b.app.status);
    b.app.run_action("edition.supprimer");
    b.chauffer(2);
    assert!(!encre(&b, 0, 430, 230) && encre(&b, 0, 210, 150), "Suppr efface le carré B, pas le A");
    assert_eq!((pixels(&b, 0), b.app.history.undo_len()), (encres - 400, etapes + 1));
    assert!(b.app.bsel.is_none());
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(pixels(&b, 0), encres, "l'annulation rend le carré B");
    // Copier, puis couper : le carré A part ; coller le remet, choisi, prêt à être déplacé dans la vue.
    let lasso = tour(&b, A);
    geste(&mut b, &lasso, Modifiers::NONE);
    b.app.run_action("edition.copier");
    assert!(b.app.pixel_clipboard.is_some(), "{}", b.app.status);
    assert_eq!(pixels(&b, 0), encres, "copier ne change rien");
    b.app.run_action("edition.couper");
    b.chauffer(2);
    assert!(!encre(&b, 0, 210, 150) && encre(&b, 0, 430, 230), "couper enlève le carré A");
    b.app.run_action("edition.coller");
    b.chauffer(2);
    assert!(b.app.bsel.as_ref().is_some_and(|s| s.lifted.is_some()), "la zone collée est choisie ({})", b.app.status);
    assert!(encre(&b, 0, 210, 150) && b.app.e3_selectionne() && b.app.tool == Tool::Select);
    // On la déplace dans la vue, de 100 pixels du dessin vers la droite.
    let pris = dans_la_boite(&b);
    let (de, a) = (ecran(&b, pris.x, pris.y), ecran(&b, pris.x + 100.0, pris.y));
    geste(&mut b, &[de, a], Modifiers::NONE);
    attendre_la_pose(&mut b);
    assert!(encre(&b, 0, 310, 150) && !encre(&b, 0, 210, 150), "{}", b.app.status);
    // Échap pose la zone, comme sur la feuille : plus rien n'est choisi, le carré reste où on l'a mis.
    touche(&mut b, Key::Escape);
    assert!(b.app.bsel.is_none(), "Échap pose la zone ({})", b.app.status);
    assert!(encre(&b, 0, 310, 150) && pixels(&b, 0) == encres);
    // Maj ajoute au choix : les deux carrés, d'un lasso chacun. (Le second part du coin bas-droite du carré B : son coin
    // haut-gauche est, à l'écran, assez près du coin de la boîte du premier pour la faire tourner, comme sur la feuille.)
    let (lasso_a, mut lasso_b) = (tour(&b, A.translate(vec2(100.0, 0.0))), tour(&b, B));
    lasso_b.rotate_left(2);
    geste(&mut b, &lasso_a, Modifiers::NONE);
    appuyer(&mut b, &lasso_b, Modifiers::SHIFT);
    assert!(matches!(&b.app.bdrag, Some(BDrag::Lasso { add: true, .. })), "Maj : un lasso qui ajoute ({})", b.app.status);
    lacher(&mut b, *lasso_b.last().expect("un point"), Modifiers::SHIFT);
    let zone = b.app.bitmap_selection_bounds().expect("deux zones choisies ensemble");
    assert!(zone.min.x < 300.0 && zone.max.x > 440.0 && zone.max.y > 240.0, "{zone:?}");
    assert!(b.app.bsel.as_ref().is_some_and(|s| s.outline.len() == 2 && s.lifted.is_none()));
    // Un outil de la vue reprend le pointeur : ce qui était choisi est lâché (rien ne reste sous Suppr sans qu'on le voie).
    b.app.run_action("espace3d.outil_deplacer");
    b.chauffer(2);
    assert!(!b.app.e3_selectionne() && b.app.bsel.is_none() && b.app.vsel.is_none());
    assert_eq!(pixels(&b, 0), encres);
}

#[test]
fn on_a_vector_card_the_lasso_takes_strokes_and_the_box_moves_scales_and_turns_them() {
    let mut b = ouvert(doc_cartes(1, LayerKind::Vector));
    // Deux traits de pinceau, tracés dans la vue.
    b.app.run_action("outil.pinceau");
    b.chauffer(2);
    assert!(b.app.e3_dessine());
    for (de, a) in [((200.0, 140.0), (260.0, 160.0)), ((400.0, 220.0), (460.0, 240.0))] {
        let pts = [ecran(&b, de.0, de.1), ecran(&b, a.0, a.1)];
        geste(&mut b, &pts, Modifiers::NONE);
    }
    let cel = b.app.layers()[0].cel_at(0).expect("un dessin");
    assert_eq!(b.app.drawing(cel).expect("vectoriel").elements.len(), 2, "{}", b.app.status);
    let bornes = |b: &Banc, i: usize| b.app.drawing(cel).expect("vectoriel").elements[i].bounds();
    let (trait_0, trait_1) = (bornes(&b, 0), bornes(&b, 1));
    selectionner(&mut b);
    let etapes = b.app.history.undo_len();

    // Un lasso autour du premier trait : lui seul est choisi.
    let lasso = tour(&b, trait_0.expand(14.0));
    appuyer(&mut b, &lasso[..3], Modifiers::NONE);
    assert!(matches!(&b.app.vdrag, Some(VDrag::Lasso(points)) if points.len() > 4), "{}", b.app.status);
    lacher(&mut b, lasso[2], Modifiers::NONE);
    b.app.run_action("edition.tout_deselectionner");
    geste(&mut b, &lasso, Modifiers::NONE);
    assert_eq!(b.app.vsel.as_ref().map(|s| s.items.clone()), Some(vec![0]), "{}", b.app.status);
    assert_eq!(b.app.history.undo_len(), etapes);

    // Glisser l'intérieur de la boîte : le trait se déplace de 60 × 30 pixels du dessin. Pendant le geste, il se voit
    // sur la carte (la feuille flottante).
    let (c, pris) = (trait_0.center(), dans_la_boite(&b));
    let (de, a) = (ecran(&b, pris.x, pris.y), ecran(&b, pris.x + 60.0, pris.y + 30.0));
    appuyer(&mut b, &[de, a], Modifiers::NONE);
    assert!(b.app.e3_feuille_flottante(cel).is_some(), "le trait qu'on déplace est posé sur la carte");
    let ou = b.app.flottante.as_ref().expect("la feuille flottante du geste").aff.apply(c);
    assert!((ou - (c + vec2(60.0, 30.0))).length() < 3.0, "il suit le geste : {ou:?}");
    lacher(&mut b, a, Modifiers::NONE);
    let deplace = bornes(&b, 0);
    assert!((deplace.center() - (c + vec2(60.0, 30.0))).length() < 3.0, "{:?} au lieu de {:?}", deplace.center(), c + vec2(60.0, 30.0));
    assert!((deplace.size() - trait_0.size()).length() < 0.5, "sa taille n'a pas changé");
    assert_eq!(bornes(&b, 1), trait_1, "l'autre trait ne bouge pas");
    assert_eq!(b.app.history.undo_len(), etapes + 1, "une étape");

    // Un coin de la boîte l'agrandit.
    b.chauffer(3);
    let coins = b.app.sel_frame().expect("la boîte").corners();
    let coin = b.app.layer_to_screen(coins[2]);
    assert_eq!(b.app.box_hit(coin), Some(Handle::Corner(2)), "le coin bas-droite de la boîte, sur la carte");
    let plus_loin = b.app.layer_to_screen(coins[2] + (coins[2] - coins[0]) * 0.6);
    geste(&mut b, &[coin, plus_loin], Modifiers::NONE);
    let agrandi = bornes(&b, 0);
    assert!(agrandi.width() > deplace.width() * 1.3 && agrandi.height() > deplace.height() * 1.3, "{:?} → {:?}", deplace.size(), agrandi.size());
    assert_eq!(b.app.history.undo_len(), etapes + 2);

    // L'extérieur d'un coin le fait tourner.
    b.chauffer(3);
    let coins = b.app.sel_frame().expect("la boîte").corners();
    let (c0, c2) = (b.app.layer_to_screen(coins[0]), b.app.layer_to_screen(coins[2]));
    let dehors = c2 + (c2 - c0).normalized() * 15.0;
    assert!(matches!(b.app.box_hit(dehors), Some(Handle::Rotate(_))), "{:?}", b.app.box_hit(dehors));
    let tourne_vers = dehors + (c2 - c0).normalized().rot90() * 80.0;
    geste(&mut b, &[dehors, tourne_vers], Modifiers::NONE);
    let tourne = bornes(&b, 0);
    assert!((tourne.width() / tourne.height() - agrandi.width() / agrandi.height()).abs() > 0.05, "le trait a tourné : {:?} → {:?}", agrandi.size(), tourne.size());
    assert_eq!(b.app.history.undo_len(), etapes + 3);
    assert_eq!(bornes(&b, 1), trait_1);

    // Trois annulations : le trait revient où il était.
    for _ in 0..3 {
        b.app.run_action("edition.annuler");
        b.chauffer(2);
    }
    let revenu = bornes(&b, 0);
    assert!((revenu.min - trait_0.min).length() < 0.5 && (revenu.max - trait_0.max).length() < 0.5, "{revenu:?} au lieu de {trait_0:?}");

    // Suppr retire les traits choisis.
    b.app.run_action("edition.tout_deselectionner");
    let lasso = tour(&b, trait_1.expand(14.0));
    geste(&mut b, &lasso, Modifiers::NONE);
    assert_eq!(b.app.vsel.as_ref().map(|s| s.items.clone()), Some(vec![1]), "{}", b.app.status);
    b.app.run_action("edition.supprimer");
    b.chauffer(2);
    assert_eq!(b.app.drawing(cel).expect("vectoriel").elements.len(), 1);
}

#[test]
fn ctrl_held_with_a_drawing_tool_draws_a_lasso_and_ctrl_click_still_takes_a_card_and_alt_drag_turns_the_view() {
    let mut doc = doc_deux_carres();
    // La carte 1 est plus loin et décalée à droite : sa moitié droite dépasse de la carte 0.
    doc.panels[0].layers[1].transform = LayerTransform { x: 380.0, ..LayerTransform::default() };
    doc.panels[0].layers[1].depth = 0.5;
    let mut b = ouvert(doc);
    b.app.run_action("espace3d.cadrer");
    b.chauffer(2);
    b.app.run_action("outil.pinceau");
    b.chauffer(2);
    assert!(b.app.e3_dessine());
    let (etapes, encres) = (b.app.history.undo_len(), pixels(&b, 0));
    // Ctrl+glisser autour du carré A, le pinceau en main : un lasso, pas un trait.
    let lasso = tour(&b, A);
    geste(&mut b, &lasso, Modifiers::CTRL);
    let zone = b.app.bitmap_selection_bounds().unwrap_or_else(|| panic!("Ctrl tenu : le lasso a choisi une zone ({})", b.app.status));
    assert!((zone.min - A.min).length() < 3.0 && (zone.max - A.max).length() < 3.0, "{zone:?}");
    assert_eq!((pixels(&b, 0), b.app.history.undo_len()), (encres, etapes), "aucun trait n'a été tracé");
    // (Une zone bitmap n'existe qu'avec l'outil Sélection : il reste, comme sur la feuille, et garde le pointeur.)
    assert!(b.app.tool == Tool::Select && b.app.e3_selectionne(), "{:?}", b.app.tool);

    // Alt+glisser tourne la vue, avec l'outil Sélection aussi : rien n'est choisi ni déplacé.
    let (a, c) = (ecran(&b, 250.0, 160.0), ecran(&b, 390.0, 200.0));
    let avant = b.app.espace3d.cam.expect("la vue");
    geste(&mut b, &[a, c], Modifiers::ALT);
    let apres = b.app.espace3d.cam.expect("la vue");
    assert!((apres.azimuth - avant.azimuth).abs() > 1.0, "Alt+glisser tourne la vue : {avant:?} → {apres:?}");
    assert!(b.app.espace3d.drag.is_none() && b.app.bdrag.is_none() && b.app.e3_selectionne());
    let encore = b.app.bitmap_selection_bounds().expect("la zone est toujours choisie");
    assert!((encore.min - zone.min).length() < 0.01 && (encore.max - zone.max).length() < 0.01);
    b.app.espace3d.cam = Some(avant);
    b.chauffer(2);

    // Ctrl+clic sur la carte 1 : elle devient le calque courant ; la zone de la carte 0 est lâchée, rien n'a changé.
    let regard: Regard3d = b.app.e3d.regard.expect("le regard de la vue");
    let world = b.app.e3_world();
    let l = &b.app.layers()[1];
    let autre = CarteVue::new(regard, world.card_xf(&l.pose(0), l.depth_at(0)).matrix(), world.stage);
    let ici = autre.vers_ecran(pos2(600.0, 180.0)).expect("la droite de la carte 1 se voit");
    assert!(b.app.canvas_rect.contains(ici));
    geste(&mut b, &[ici, ici], Modifiers::CTRL);
    assert_eq!(b.app.cur_layer, 1, "Ctrl+clic prend la carte sous le pointeur : {}", b.app.status);
    b.chauffer(2);
    assert!(b.app.bsel.is_none(), "la zone choisie sur l'autre carte est lâchée");
    assert_eq!((pixels(&b, 0), pixels(&b, 1)), (encres, 0));
    assert!(b.app.e3_selection_legende().contains("« Carte 1 »"));
}
