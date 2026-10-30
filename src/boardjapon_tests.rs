//! Essais du panneau « Vignettes » (2026-10-24) : la planche lue à la japonaise (colonnes, texte à droite des cases), les textes montrés à
//! côté des cases, la mémoire de ces réglages, et l'étiquette de version sur les cases.

use std::collections::HashMap;

use eframe::egui::{epaint::Shape, pos2, vec2, Event, Modifiers, PointerButton, Pos2, Rect};

use crate::board::{self, BoardLayout, BoardSettings, Hit, Options, LABEL_H, NOTE_LINE_H, PAGE_PAD};
use crate::boardsens::{Champ, Sens, Textes};
use crate::dock::Pane;
use crate::document::Document;
use crate::perf_tests::{banc_taille, Banc};
use crate::vignettes_camera_tests::{deux_cadres, plan_court, sans_chevauchement, K};
use crate::vignettes_tests::{appui, clic_avec, encre, pas_d_annulation, quatre_plans};
use crate::workspace::{Espace, Layout};

/// Deux longueurs égales, aux arrondis des décalages près.
fn eg(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

fn reglages(colonnes: usize) -> BoardSettings {
    BoardSettings { card_w: 170.0, columns: colonnes, rows: 0 }
}

fn japonais() -> Options {
    Options { sens: Sens::Japonais, ..Default::default() }
}

fn disposer(doc: &Document, colonnes: usize) -> BoardLayout {
    board::layout_with(doc, &reglages(colonnes), &HashMap::new(), false, &japonais())
}

/// Un film de `n` plans d'une case chacun.
fn film(n: usize) -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    while doc.panels.len() < n {
        let last = doc.panels.len() - 1;
        doc.add_scene_after(last);
    }
    doc
}

pub(crate) fn banc_vignettes(doc: Document, sens: Sens, colonnes: usize) -> Banc {
    let mut b = banc_taille(doc, false, 1600, 940);
    b.garder_textes = true;
    b.garder_formes = true;
    b.app.set_espace(Espace::Vignettes);
    b.app.layout.board_columns = colonnes;
    b.app.layout.board_rows = 0;
    b.app.layout.card_size = 170.0;
    b.app.layout.board_sens = sens;
    b.app.board_zoom = 0.0;
    b.chauffer(6);
    b
}

pub(crate) fn ecran(b: &Banc, r: Rect) -> Rect {
    b.app.vignettes.xf.rect(r)
}

/// Le centre, à l'écran, du premier texte peint qui contient `morceau`.
fn texte_a_l_ecran(b: &Banc, morceau: &str) -> Pos2 {
    for clipped in &b.formes {
        if let Shape::Text(t) = &clipped.shape {
            if t.galley.job.text.contains(morceau) {
                return t.pos + t.galley.size() / 2.0;
            }
        }
    }
    panic!("le texte « {morceau} » n'est pas à l'écran : {:?}", b.textes);
}

/// Le centre, à l'écran, du premier texte peint qui est exactement `texte`.
fn texte_exact(b: &Banc, texte: &str) -> Pos2 {
    for clipped in &b.formes {
        if let Shape::Text(t) = &clipped.shape {
            if t.galley.job.text == texte {
                return t.pos + t.galley.size() / 2.0;
            }
        }
    }
    panic!("le texte « {texte} » n'est pas à l'écran : {:?}", b.textes);
}

pub(crate) fn clic(b: &mut Banc, p: Pos2) {
    clic_avec(b, p, Modifiers::NONE);
    b.chauffer(2);
}

// ------------------------------------------------------------------------------------------- la mise en page à la japonaise

#[test]
fn the_cases_flow_top_to_bottom_in_a_column_then_the_next_column_starts_to_the_right() {
    let doc = film(8);
    let l = disposer(&doc, 3);
    assert_eq!(l.sens, Sens::Japonais);
    assert_eq!(l.cards.len(), 8);
    let colonnes: Vec<Vec<usize>> = (0..3).map(|c| (0..8).filter(|&k| l.cards[k].row == c).collect()).collect();
    assert_eq!(colonnes, vec![vec![0, 1, 2], vec![3, 4, 5], vec![6, 7]], "trois cases par colonne, puis la suivante");
    for col in &colonnes {
        for w in col.windows(2) {
            let (a, b) = (l.cards[w[0]], l.cards[w[1]]);
            assert_eq!(a.thumb.left(), b.thumb.left(), "les cases d'une colonne sont alignées");
            // Chaque case garde au-dessus d'elle la ligne de son numéro, et un écart de coupe (un plan par case).
            assert!((b.thumb.top() - a.thumb.bottom() - (LABEL_H + board::gap_cut(170.0))).abs() < 1e-3, "{} {}", a.thumb.bottom(), b.thumb.top());
        }
    }
    // La colonne suivante : en haut, entièrement à droite du texte de la précédente, à un écart de coupe.
    for c in 1..3 {
        let (p, premiere) = (l.cards[colonnes[c - 1][0]], l.cards[colonnes[c][0]]);
        assert_eq!(premiere.thumb.top(), l.cards[0].thumb.top(), "toutes les colonnes commencent à la même hauteur");
        assert!((premiere.thumb.left() - (p.note.right() + board::gap_cut(170.0))).abs() < 1e-3, "la colonne {c} commence après le texte de la précédente");
    }
    // Le pas de colonne est le même partout (case, texte, écart).
    let pas = l.cards[3].thumb.left() - l.cards[0].thumb.left();
    assert!((l.cards[6].thumb.left() - l.cards[3].thumb.left() - pas).abs() < 1e-3);
    assert!((pas - (170.0 + board::note_gap(170.0) + 170.0 + board::gap_cut(170.0))).abs() < 1e-3);
    // L'ordre de lecture : de haut en bas, colonne par colonne.
    for k in 1..8 {
        let (a, b) = (l.cards[k - 1], l.cards[k]);
        assert!(b.row > a.row || (b.row == a.row && b.thumb.top() > a.thumb.top()), "ordre de lecture cassé en {k}");
    }
    sans_chevauchement(&l);
    // Une seule grande page, qui contient tout.
    assert_eq!(l.pages.len(), 1);
    assert!(l.cards.iter().all(|c| l.pages[0].contains_rect(c.card) && l.pages[0].contains_rect(c.note)));
}

#[test]
fn the_text_of_a_case_is_to_its_right_and_taller_text_pushes_the_next_case_down() {
    let doc = film(5);
    let l = disposer(&doc, 6);
    for c in &l.cards {
        assert!((c.note.left() - (c.thumb.right() + board::note_gap(170.0))).abs() < 1e-3, "à droite de la case, à 12 % et 8 points");
        assert!(eg(c.note.top(), c.thumb.top()), "en haut de l'image");
        assert!(eg(c.note.width(), 170.0), "à la largeur d'une case");
        assert!((c.note.height() - NOTE_LINE_H).abs() < 1e-3, "une bande vide garde une ligne");
    }
    // Trois champs cochés : trois lignes, la zone grandit, sans toucher à la colonne (elle tient à côté de l'image).
    let tout = Options { textes: Textes { action: true, dialogue: true, notes: true }, ..japonais() };
    let l3 = board::layout_with(&doc, &reglages(6), &HashMap::new(), false, &tout);
    assert!(eg(l3.cards[1].note.height(), 3.0 * NOTE_LINE_H));
    assert_eq!(l3.cards[2].thumb.top(), l.cards[2].thumb.top(), "trois lignes tiennent à côté de l'image : la colonne ne bouge pas");
    // Une ligne par champ coché, jamais plus, quelle que soit la longueur du texte ; des cases de 170 sont plus larges que haute de 96 :
    // trois lignes de 13 tiennent toujours à côté de l'image, la colonne ne descend donc pas plus.
    let mut long = film(5);
    long.panels[1].notes = "mot ".repeat(200);
    let ll = board::layout_with(&long, &reglages(6), &HashMap::new(), false, &tout);
    assert!(eg(ll.cards[1].note.height(), 3.0 * NOTE_LINE_H));
    assert_eq!(ll.cards[2].thumb.top(), l.cards[2].thumb.top());
}

#[test]
fn the_pale_cases_follow_the_same_flow_three_columns_of_six_by_default() {
    let doc = film(1);
    let l = board::layout_with(&doc, &BoardSettings { card_w: 170.0, columns: 6, rows: 0 }, &HashMap::new(), false, &Options { sens: Sens::Japonais, ..Options::pale(3) });
    // Trois colonnes de six cases, dont une est occupée.
    assert_eq!(l.empty.len(), 18 - 1);
    let mut xs: Vec<f32> = l.empty.iter().map(|e| e.left()).collect();
    xs.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
    assert_eq!(xs.len(), 3, "trois colonnes : {xs:?}");
    assert_eq!(xs[0], l.cards[0].thumb.left(), "la première case pâle est sous la dernière case, dans sa colonne");
    assert!(l.empty[0].top() > l.cards[0].thumb.bottom());
    let pas = xs[1] - xs[0];
    assert!((xs[2] - xs[1] - pas).abs() < 1e-3, "des colonnes à pas égal, texte compris");
    assert!((pas - (170.0 + board::note_gap(170.0) + 170.0 + board::gap_cut(170.0))).abs() < 1e-3);
    // Dans une colonne : de haut en bas, sans trou ni recouvrement.
    for w in l.empty.windows(2).filter(|w| (w[0].left() - w[1].left()).abs() < 1e-3) {
        assert!(w[1].top() > w[0].bottom());
    }
    sans_chevauchement(&l);
    // Toujours une colonne pâle après la dernière colonne occupée : trois colonnes de trois cases, une quatrième pâle.
    let doc = film(8);
    let l = board::layout_with(&doc, &reglages(3), &HashMap::new(), false, &Options { sens: Sens::Japonais, ..Options::pale(3) });
    let colonnes_pales: std::collections::BTreeSet<i32> = l.empty.iter().map(|e| (e.left() * 10.0) as i32).collect();
    assert_eq!(colonnes_pales.len(), 2, "le reste de la troisième colonne, puis une colonne de plus");
    assert_eq!(l.empty.len(), 1 + 3);
    // Sans cases pâles (les exports) : aucune.
    assert!(disposer(&film(8), 3).empty.is_empty());
}

#[test]
fn the_default_reading_is_unchanged_and_the_options_do_not_touch_the_rows() {
    let doc = film(8);
    let a = board::layout_with(&doc, &reglages(3), &HashMap::new(), false, &Options::default());
    let b = board::layout_with(&doc, &reglages(3), &HashMap::new(), false, &Options { sens: Sens::Lignes, textes: Textes::default(), ..Default::default() });
    assert_eq!(a, b);
    assert_eq!(a.sens, Sens::Lignes);
    // En lignes : de gauche à droite, le texte dessous.
    assert!(a.cards[1].thumb.left() > a.cards[0].thumb.right());
    assert_eq!(a.cards[1].thumb.top(), a.cards[0].thumb.top());
    assert!(a.cards[0].note.top() > a.cards[0].thumb.bottom());
}

// ------------------------------------------------------------------------------------------- un mouvement de caméra

#[test]
fn the_text_of_a_move_is_right_of_the_rightmost_frame_and_the_next_column_is_entirely_to_its_right() {
    use crate::vignettecam::{coins_de, se_recouvrent};
    for (dx, dy) in [(160.0, 55.0), (0.0, 90.0), (160.0, 0.0), (80.0, 120.0), (40.0, 70.0), (-160.0, 0.0), (0.0, -90.0), (-120.0, -60.0)] {
        let mut doc = plan_court();
        deux_cadres(&mut doc, 1, dx, dy);
        // Quatre cases par colonne : K est la dernière de sa colonne, la suivante est à droite.
        let l = disposer(&doc, 4);
        let (c, m) = (l.cards[K], l.move_of(K).expect("une empreinte"));
        assert_eq!(l.cards.len(), 6, "aucune vignette perdue");
        let b = m.cadres.iter().max_by(|a, b| a.rect.right().total_cmp(&b.rect.right())).unwrap().rect;
        assert!(c.note.left() >= b.right() + board::note_gap(170.0) - 1e-2, "({dx}, {dy}) : le texte est à droite du cadre le plus à droite : {:?} {b:?}", c.note);
        assert!((c.note.top() - b.top()).abs() < 1e-2, "({dx}, {dy}) : en haut de ce cadre");
        assert!(!se_recouvrent(&coins_de(c.note).to_vec(), &m.enveloppe, 0.5), "({dx}, {dy}) : le texte ne recouvre pas l'enveloppe");
        assert_eq!(board::hit(&l, c.note.center(), 0.0), Hit::Note(K), "({dx}, {dy}) : on peut le toucher");
        // Toute la colonne suivante est à droite du mouvement et de son texte, pas seulement ce qui est à côté.
        let droite = c.note.right().max(c.foot.right());
        for n in l.cards.iter().filter(|n| n.row > c.row) {
            assert!(n.card.left() >= droite + board::gap_cut(170.0) - 1e-2, "({dx}, {dy}) : la case {} de la colonne suivante commence à {} (le mouvement et son texte vont jusqu'à {droite})", n.id, n.card.left());
        }
        sans_chevauchement(&l);
    }
    // Six cases par colonne : les cases d'après sont dessous, plus bas que l'enveloppe et son texte.
    for (dx, dy) in [(0.0, 90.0), (160.0, 55.0), (80.0, 120.0)] {
        let mut doc = plan_court();
        deux_cadres(&mut doc, 1, dx, dy);
        let l = disposer(&doc, 6);
        let c = l.cards[K];
        let bas = c.note.bottom().max(c.foot.bottom());
        for n in l.cards.iter().skip(K + 1).filter(|n| n.row == c.row) {
            assert!(n.card.top() >= bas + board::gap_same(170.0).min(board::gap_cut(170.0)) - 1e-2, "({dx}, {dy}) : la case {} remonte jusqu'à {}, le mouvement va jusqu'à {bas}", n.id, n.card.top());
        }
        sans_chevauchement(&l);
    }
}

#[test]
fn a_stays_put_when_the_hull_grows_right_or_down_and_the_hold_keeps_it_during_a_gesture() {
    let ordinaire = disposer(&plan_court(), 4);
    for (dx, dy) in [(160.0, 0.0), (0.0, 55.0), (0.0, 90.0), (500.0, 0.0), (160.0, 55.0), (17.3, 23.9)] {
        let mut doc = plan_court();
        deux_cadres(&mut doc, 1, dx, dy);
        let l = disposer(&doc, 4);
        assert_eq!(l.cards[K].thumb, ordinaire.cards[K].thumb, "({dx}, {dy}) : A ne bouge pas");
        for j in 0..K {
            assert_eq!(l.cards[j].thumb, ordinaire.cards[j].thumb, "la case {j}, d'avant, reste");
        }
        assert_eq!(l.cards.len(), ordinaire.cards.len());
        sans_chevauchement(&l);
    }
    // Vers la gauche : le cadre le plus à gauche s'aligne sur le bord de la colonne (c'est A qui est en retrait à droite, rien ne glisse),
    // les cases d'avant ne bougent pas. Vers le haut : A passe sous la case d'avant, une fois.
    for (dx, dy) in [(-160.0, 0.0), (-400.0, 30.0), (0.0, -90.0), (-120.0, -60.0)] {
        let mut doc = plan_court();
        deux_cadres(&mut doc, 1, dx, dy);
        let l = disposer(&doc, 4);
        sans_chevauchement(&l);
        assert_eq!(l.decalage.x, 0.0, "({dx}, {dy}) : rien ne glisse à gauche, A est en retrait");
        for j in 0..K {
            assert!((l.cards[j].thumb.translate(-l.decalage).min - ordinaire.cards[j].thumb.min).length() < 1e-3, "la case {j}, d'avant, reste ({dx}, {dy})");
        }
        // Le cadre le plus à gauche est au bord de la colonne, comme les autres cases de la colonne.
        let m = l.move_of(K).unwrap();
        let gauche = m.cadres.iter().map(|c| c.rect.left()).fold(f32::MAX, f32::min).min(l.cards[K].thumb.left());
        assert!(eg(gauche, l.cards[K].ligne.left()), "({dx}, {dy}) : le plus à gauche à la ligne de la colonne : {gauche} {:?}", l.cards[K].ligne);
        assert!(l.cards.iter().filter(|n| n.row == l.cards[K].row && n.id != l.cards[K].id).all(|n| eg(l.cards[K].ligne.left(), n.thumb.left())), "({dx}, {dy}) : la même colonne que les autres");
        // A ne recule jamais : elle reste dans sa colonne (plus bas au plus, et plus à droite si un cadre est à sa gauche), ou passe en haut de
        // la colonne suivante quand elle n'y tient plus.
        let (avant, apres) = (ordinaire.cards[K].thumb, l.cards[K].thumb.translate(-l.decalage));
        let plus_tard = l.cards[K].row > ordinaire.cards[K].row || (apres.left() >= avant.left() - 1e-3 && apres.top() >= avant.top() - 1e-3);
        assert!(plus_tard, "({dx}, {dy}) : A ne recule jamais : {avant:?} {apres:?}");
    }
    // Une première case dont le cadre monte au-dessus de la planche : tout descend, la vue compense.
    let mut doc = plan_court();
    deux_cadres(&mut doc, 0, 0.0, -90.0);
    let l = disposer(&doc, 4);
    assert!(l.decalage.y > 0.0, "{:?}", l.decalage);
    assert!((l.cards[0].thumb.translate(-l.decalage).min - ordinaire.cards[0].thumb.min).length() < 1e-3);
    sans_chevauchement(&l);
    // Pendant un geste : la réserve tient A exactement, même quand l'enveloppe recouvre des cases d'avant.
    let pose = board::Reserve::depuis(&ordinaire, K).unwrap();
    assert!(pose.pose_y.is_some(), "à la japonaise, la réserve tient aussi la hauteur");
    for (dx, dy) in [(-160.0, 0.0), (-120.0, -60.0), (0.0, -90.0), (-400.0, 30.0), (200.0, 40.0)] {
        let mut doc = plan_court();
        deux_cadres(&mut doc, 1, dx, dy);
        let l = board::layout_with(&doc, &reglages(4), &HashMap::new(), false, &Options { hold: Some(pose), ..japonais() });
        let (a, b) = (l.cards[K].thumb.translate(-l.decalage), ordinaire.cards[K].thumb);
        assert!((a.min - b.min).length() < 1e-3, "({dx}, {dy}) : A reste pendant le geste : {a:?} {b:?}");
        assert_eq!(l.cards.len(), ordinaire.cards.len());
        assert!(l.pages.iter().any(|p| p.contains_rect(l.cards[K].foot)), "l'empreinte tient dans la page ({dx}, {dy})");
    }
    // L'enveloppe ne rétrécit pas tant que le geste dure : les cases d'après ne remontent pas.
    let mut doc = plan_court();
    deux_cadres(&mut doc, 1, 0.0, 100.0);
    let grand = disposer(&doc, 6);
    let reserve = board::Reserve::depuis(&grand, K).unwrap();
    deux_cadres(&mut doc, 1, 0.0, 40.0);
    let libre = disposer(&doc, 6);
    let retenu = board::layout_with(&doc, &reglages(6), &HashMap::new(), false, &Options { hold: Some(reserve), ..japonais() });
    assert!(libre.cards[K + 1].thumb.top() < grand.cards[K + 1].thumb.top() - 50.0, "sans réserve, elles remontent");
    assert!((retenu.cards[K + 1].thumb.min - grand.cards[K + 1].thumb.min).length() < 1e-2, "avec la réserve, les autres cases ne bougent pas");
    assert!((retenu.cards[K].thumb.min - grand.cards[K].thumb.min).length() < 1e-2, "et A non plus");
}

#[test]
fn the_layout_memo_follows_the_reading_and_the_fields() {
    let doc = film(8);
    let mut cache = board::CacheDisposition::default();
    let s = reglages(3);
    let lignes = HashMap::new();
    let a = board::layout_cache(&doc, &s, &lignes, false, &Options::default(), &mut cache);
    board::layout_cache(&doc, &s, &lignes, false, &Options::default(), &mut cache);
    assert_eq!(cache.calculs, 1, "même clé : une seule mise en page");
    let b = board::layout_cache(&doc, &s, &lignes, false, &japonais(), &mut cache);
    assert_eq!(cache.calculs, 2, "le sens de lecture fait partie de la clé");
    assert_ne!(a.cards[1].thumb, b.cards[1].thumb);
    assert_eq!(b.sens, Sens::Japonais);
    let tout = Options { textes: Textes { action: true, dialogue: false, notes: true }, ..japonais() };
    board::layout_cache(&doc, &s, &lignes, false, &tout, &mut cache);
    assert_eq!(cache.calculs, 3, "les champs montrés aussi");
    // Le memo rend ce que la mise en page directe rend.
    assert_eq!(board::layout_cache(&doc, &s, &lignes, false, &tout, &mut cache), board::layout_with(&doc, &s, &lignes, false, &tout));
    assert_eq!(cache.calculs, 3);
}

// ------------------------------------------------------------------------------------------- toucher, glisser, barres

#[test]
fn hit_testing_follows_the_columns_the_bars_are_horizontal_and_their_touch_zone_grows_vertically() {
    let doc = quatre_plans();
    let l = disposer(&doc, 4);
    for (k, c) in l.cards.iter().enumerate() {
        assert_eq!(board::hit(&l, c.thumb.center(), 0.0), Hit::Thumb(k));
        assert_eq!(board::hit(&l, c.note.center(), 0.0), Hit::Note(k), "le texte, à droite de la case");
    }
    // Entre deux cases d'une colonne : une barre horizontale ; la dernière d'une colonne a la sienne, sous elle.
    for b in &l.bars {
        assert!(b.bar.width() > b.bar.height(), "horizontale : {:?}", b.bar);
        assert!(b.zone.width() > b.zone.height());
        assert_eq!(board::hit(&l, b.zone.center(), 0.0), Hit::Bar(b.after));
    }
    let (c0, b0) = (l.cards[0], l.bars[0]);
    assert!((b0.zone.top() - c0.thumb.bottom()).abs() < 1e-3, "dans l'écart sous la case");
    assert!(!b0.cut, "deux cases d'un même plan : grise");
    // Une barre étroite se touche plus largement, en hauteur : un point au-dessus de sa zone, dans la case, la prend.
    let p = pos2(b0.zone.center().x, b0.zone.top() - 8.0);
    assert_eq!(board::hit(&l, p, 0.0), Hit::Thumb(0));
    assert_eq!(board::hit(&l, p, 30.0), Hit::Bar(0));
    // À une coupe : noire.
    assert!(l.bars[2].cut);
    // Le texte d'une case se touche aussi quand il est loin à droite d'une grande case.
    assert!(board::hit(&l, pos2(c0.note.right() + 5.0, c0.note.center().y), 0.0) != Hit::Note(0));
}

#[test]
fn the_drop_markers_are_horizontal_between_two_cases_of_a_column_and_above_the_first_of_a_column() {
    let doc = quatre_plans();
    let l = disposer(&doc, 4);
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    let slots = board::drop_slots(&l);
    assert_eq!(slots.len(), 5, "devant chaque plan, et après le dernier");
    for s in &slots {
        assert_eq!(s.a.y, s.b.y, "un trait horizontal : {s:?}");
        assert!(s.b.x > s.a.x);
    }
    // Quatre cases par colonne : le plan 4 (case 5) est le deuxième de sa colonne, juste sous la case 4 (plan 3).
    let devant4 = slots.iter().find(|s| s.before == Some(ids[3])).unwrap();
    let (c4, c5) = (l.cards[4], l.cards[5]);
    assert!(devant4.a.y > c4.thumb.bottom() && devant4.a.y < c5.card.top(), "dans l'écart entre les deux cases : {} entre {} et {}", devant4.a.y, c4.thumb.bottom(), c5.card.top());
    // Le plan 3 (case 4) est le premier de la colonne suivante : le repère est au-dessus de la case.
    let devant3 = slots.iter().find(|s| s.before == Some(ids[2])).unwrap();
    assert!(devant3.a.y < l.cards[4].card.top() && devant3.a.y > l.cards[4].card.top() - PAGE_PAD);
    // Le plus proche d'un point : celui de l'écart, même à côté du trait.
    let ordre = ids.clone();
    let p = pos2(devant4.x + 10.0, devant4.a.y + 3.0);
    let cible = board::nearest_slot(&slots, &ordre, &[ids[1]], p).expect("un repère");
    assert_eq!(cible.before, Some(ids[3]));
    // La copie le prend aussi, même devant l'original.
    assert!(board::nearest_slot_copie(&slots, pos2(slots[1].x, slots[1].a.y)).is_some());
    // En lignes, les repères restent verticaux (rien n'a changé pour eux).
    let en_lignes = board::layout_with(&doc, &reglages(4), &HashMap::new(), false, &Options::default());
    assert!(board::drop_slots(&en_lignes).iter().all(|s| s.a.x == s.b.x && s.b.y > s.a.y));
}

#[test]
fn on_screen_a_bar_cuts_a_plan_a_frame_drag_moves_it_and_a_stroke_draws_in_the_case() {
    let mut b = banc_vignettes(quatre_plans(), Sens::Japonais, 4);
    let l = b.app.board_layout();
    assert_eq!(l.sens, Sens::Japonais);
    assert!(b.app.board_zoom > 0.0);
    // Du dessin dans la case 3 (le plan 2) : il arrive dans le plan.
    let case = ecran(&b, l.cards[3].thumb);
    for (de, a) in [(case.left_top() + vec2(20.0, 20.0), case.right_bottom() - vec2(20.0, 20.0))] {
        b.image(vec![Event::PointerMoved(de)], 1.0 / 60.0);
        b.image(vec![appui(de, true, Modifiers::NONE)], 1.0 / 60.0);
        for k in 1..=6 {
            b.image(vec![Event::PointerMoved(de + (a - de) * (k as f32 / 6.0))], 1.0 / 60.0);
        }
        b.image(vec![appui(a, false, Modifiers::NONE)], 1.0 / 60.0);
        b.chauffer(2);
    }
    assert!(encre(&b.app.doc, 1), "le trait est dans le plan de la case");
    // Un clic sur la barre grise sous la case 0 coupe son plan (une seule étape d'annulation).
    let avant = b.app.doc.panels.len();
    let n = pas_d_annulation(&b);
    let barre = b.app.vignettes.xf.to_screen(l.bars[0].bar.center());
    clic(&mut b, barre);
    assert_eq!(b.app.doc.panels.len(), avant + 1, "le plan est coupé : {}", b.app.status);
    assert_eq!(pas_d_annulation(&b), n + 1);
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(b.app.doc.panels.len(), avant);
    // Maj + glisser le cadre d'une case : son plan se pose au repère le plus proche, entre deux plans.
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    let l = b.app.board_layout();
    let slot = board::drop_slots(&l).into_iter().find(|s| s.before == Some(ids[3])).expect("un repère devant le plan 4");
    let case = ecran(&b, l.cards[3].thumb);
    let cadre = pos2(case.left() + 2.0, case.center().y);
    let cible = b.app.vignettes.xf.to_screen(pos2(slot.x, slot.a.y));
    let maj = Modifiers::SHIFT;
    b.touches = maj;
    b.image(vec![Event::PointerMoved(cadre)], 1.0 / 60.0);
    b.image(vec![appui(cadre, true, maj)], 1.0 / 60.0);
    for k in 1..=8 {
        b.image(vec![Event::PointerMoved(cadre + (cible - cadre) * (k as f32 / 8.0))], 1.0 / 60.0);
    }
    assert!(b.app.vignettes.moving.as_ref().is_some_and(|m| m.active && m.target.is_some_and(|t| t.before == Some(ids[3]))), "le repère de pose est celui de l'écart : {:?}", b.app.vignettes.moving.as_ref().map(|m| m.target));
    b.image(vec![appui(cible, false, maj)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.chauffer(2);
    // Le plan 2 (index 1) est passé devant le plan 4 : après le plan 3 (index 2).
    assert_eq!(b.app.doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), vec![ids[0], ids[2], ids[1], ids[3]], "{}", b.app.status);
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(b.app.doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), ids);
}

#[test]
fn the_notes_icon_is_at_the_top_right_of_the_text_zone_and_opens_the_note() {
    let mut b = banc_vignettes(quatre_plans(), Sens::Japonais, 4);
    let l = b.app.board_layout();
    let c = l.cards[1];
    let icone = b.app.note_icone_rect(&c);
    let bande = ecran(&b, c.note);
    assert!((icone.right() - (bande.right() - 1.0)).abs() < 1e-3 && (icone.top() - bande.top()).abs() < 1e-3, "tout au bout du texte, à droite : {icone:?} {bande:?}");
    assert!(icone.left() > ecran(&b, c.thumb).right(), "à droite de la case");
    clic(&mut b, icone.center());
    assert_eq!(b.app.vignettes.note.as_ref().map(|e| e.id), Some(c.id), "la note de la case s'écrit");
}

#[test]
fn zoom_to_fit_follows_the_flow_width_in_rows_height_in_columns() {
    let mut b = banc_vignettes(film(14), Sens::Japonais, 6);
    b.app.board_fit();
    b.chauffer(3);
    let (l, vue) = (b.app.board_layout(), b.app.vignettes.view);
    let z = b.app.board_zoom;
    assert!((z * l.size.y - vue.height()).abs() < 1.5 || z == 0.2 || z == 1.5, "à la hauteur de la vue : zoom {z}, planche {:?}, vue {vue:?}", l.size);
    assert!(z * l.size.y <= vue.height() + 1.0);
    // Toutes les cases tiennent verticalement sans défiler.
    assert!(l.cards.iter().all(|c| ecran(&b, c.card).bottom() <= vue.bottom() + 1.0));
    // En lignes : à la largeur.
    b.app.layout.board_sens = Sens::Lignes;
    b.app.board_fit();
    b.chauffer(3);
    let (l, vue) = (b.app.board_layout(), b.app.vignettes.view);
    assert!(b.app.board_zoom * l.size.x <= vue.width() + 1.0);
    assert!((b.app.board_zoom * l.size.x - vue.width()).abs() < 1.5 || b.app.board_zoom == 0.2 || b.app.board_zoom == 1.5);
}

// ------------------------------------------------------------------------------------------- les textes à côté des cases

pub(crate) fn plan_avec_textes() -> Document {
    let mut doc = quatre_plans();
    doc.panels[1].action = "Il court".into();
    doc.panels[1].dialogue = "Où vas-tu ?".into();
    doc.panels[1].notes = "La clé".into();
    doc
}

#[test]
fn the_fields_toggles_change_what_is_laid_out_and_what_is_painted() {
    let mut b = banc_vignettes(plan_avec_textes(), Sens::Lignes, 4);
    assert_eq!(b.app.layout.board_textes, Textes::default(), "d'origine : les notes seules, comme avant");
    let peint = |b: &Banc, morceau: &str| b.textes.iter().any(|t| t.contains(morceau));
    assert!(peint(&b, "La clé") && !peint(&b, "Il court") && !peint(&b, "Où vas-tu"), "{:?}", b.textes);
    assert!(b.textes.iter().any(|t| t.contains("Note  ")), "l'intitulé est toujours là (2026-10-30) : {:?}", b.textes);
    let hauteur = |b: &Banc| b.app.board_layout().cards[3].note.height();
    assert!(eg(hauteur(&b), NOTE_LINE_H), "une ligne de notes");
    // Cocher l'action : deux champs, chacun son étiquette, deux lignes.
    b.app.run_action("vignettes.texte_action");
    b.chauffer(4);
    assert_eq!(b.app.layout.board_textes, Textes { action: true, dialogue: false, notes: true });
    assert!(peint(&b, "Action  Il court") && peint(&b, "Note  La clé"), "{:?}", b.textes);
    assert!(eg(hauteur(&b), 2.0 * NOTE_LINE_H));
    // Le dialogue aussi : trois lignes. (Les autres cases, sans texte, ont les mêmes trois lignes, vides : une ligne par champ coché.)
    b.app.run_action("vignettes.texte_dialogue");
    b.chauffer(4);
    assert!(peint(&b, "Dialogue  Où vas-tu ?"), "{:?}", b.textes);
    assert!(eg(hauteur(&b), 3.0 * NOTE_LINE_H));
    assert!(eg(b.app.board_layout().cards[0].note.height(), 3.0 * NOTE_LINE_H));
    // Décocher les notes puis l'action : le dialogue seul, sans étiquette.
    b.app.run_action("vignettes.texte_notes");
    b.app.run_action("vignettes.texte_action");
    b.chauffer(4);
    assert!(b.textes.iter().any(|t| t.ends_with("Où vas-tu ?")), "{:?}", b.textes);
    assert!(!peint(&b, "La clé") && !peint(&b, "Il court"));
    assert!(eg(hauteur(&b), NOTE_LINE_H));
    // Les champs d'une marque d'image sont ceux de la marque.
    let mut doc = plan_avec_textes();
    doc.panels[0].marks[0].action = "Plan large".into();
    let m = doc.panels[0].marks[0].id;
    let champs = crate::boardsens::champs_de_la_case(&doc, 0, m, doc.panels[0].marks[0].frame, Textes { action: true, dialogue: true, notes: true });
    assert_eq!(champs, vec![(Champ::Action, "Plan large")]);
    let champs = crate::boardsens::champs_de_la_case(&doc, 1, doc.panels[1].id, 0, Textes { action: true, dialogue: true, notes: true });
    assert_eq!(champs.iter().map(|c| c.0).collect::<Vec<_>>(), vec![Champ::Action, Champ::Dialogue, Champ::Notes], "dans l'ordre de la fiche");
}

#[test]
fn the_text_beside_a_case_is_measured_and_painted_to_the_right_in_the_japanese_reading() {
    let mut b = banc_vignettes(plan_avec_textes(), Sens::Japonais, 4);
    b.app.layout.board_textes = Textes { action: true, dialogue: true, notes: true };
    b.chauffer(4);
    let l = b.app.board_layout();
    let c = l.cards[3];
    assert!(eg(c.note.height(), 3.0 * NOTE_LINE_H));
    assert!(c.note.left() > c.thumb.right());
    // Les trois textes sont peints à droite de la case.
    for morceau in ["Action  Il court", "Dialogue  Où vas-tu", "Note  La cl"] {
        let p = texte_a_l_ecran(&b, morceau);
        assert!(p.x > ecran(&b, c.thumb).right(), "{morceau} est à droite de la case : {p:?}");
        assert!(p.y >= ecran(&b, c.note).top() - 1.0 && p.y <= ecran(&b, c.note).bottom() + 1.0, "{morceau} est dans la bande du texte");
    }
    // Un texte très long : une seule ligne par champ, coupée par « … » (trois lignes en tout pour trois champs).
    b.app.doc.panels[1].action = "mot ".repeat(200);
    b.chauffer(4);
    assert!(eg(b.app.board_layout().cards[3].note.height(), 3.0 * NOTE_LINE_H), "une ligne par champ, quel que soit le texte");
    let galley = b.formes.iter().filter_map(|s| if let Shape::Text(t) = &s.shape { Some(t.galley.clone()) } else { None }).find(|g| g.job.text.contains("mot mot mot")).expect("l'action est peinte");
    assert_eq!(galley.rows.len(), 1, "une seule ligne");
    assert!(galley.rows[0].text().ends_with('…'), "« … » au bout : {:?}", galley.rows[0].text());
}

// ------------------------------------------------------------------------------------------- l'export de la planche

#[test]
fn the_board_export_follows_the_reading_and_the_fields() {
    use crate::planche::{composer, construire, Options as Export};
    let doc = plan_avec_textes();
    let s = reglages(4);
    let tout: Vec<usize> = (0..doc.panels.len()).collect();
    let en_lignes = construire(&doc, &tout, &s, &Export::default(), &doc.annotations);
    let o = Export { sens: Sens::Japonais, textes: Textes { action: true, dialogue: true, notes: true }, ..Export::default() };
    let jap = construire(&doc, &tout, &s, &o, &doc.annotations);
    assert_eq!(en_lignes.layout.sens, Sens::Lignes);
    assert_eq!(jap.layout.sens, Sens::Japonais);
    assert_eq!(jap.cartes.len(), 6, "aucune case perdue");
    assert_eq!(jap.layout.cards[0].thumb.left(), jap.layout.cards[1].thumb.left(), "les cases en colonne");
    let c = jap.cartes.iter().find(|c| c.place.plan == doc.panels[1].id).unwrap();
    assert_eq!(c.champs.iter().map(|c| c.0).collect::<Vec<_>>(), vec![Champ::Action, Champ::Dialogue, Champ::Notes]);
    assert!(c.champs.iter().all(|(champ, _, lignes)| lignes[0].starts_with(champ.etiquette())), "plusieurs champs : chacun son étiquette : {:?}", c.champs);
    assert!(eg(c.place.note.height(), 3.0 * NOTE_LINE_H));
    // Les notes seules, comme avant : pas d'étiquette.
    let c = en_lignes.cartes.iter().find(|c| c.place.plan == doc.panels[1].id).unwrap();
    assert_eq!(c.champs.len(), 1);
    assert_eq!(c.champs[0].2, vec!["La clé".to_string()]);
    // L'image se compose, avec les barres horizontales et du texte à droite des cases.
    let s_px = 1.5;
    let vignettes: Vec<image::RgbImage> = jap.cartes.iter().map(|_| image::RgbImage::from_pixel(64, 36, image::Rgb([255, 255, 255]))).collect();
    let img = composer(&jap, &vignettes, &o, s_px).expect("l'image se compose");
    let r = c_note(&jap, doc.panels[1].id);
    let (x0, y0) = (((r.left() - jap.cadre.left()) * s_px) as u32, ((r.top() - jap.cadre.top()) * s_px) as u32);
    let sombre = (0..(r.width() * s_px) as u32).any(|dx| (0..(r.height() * s_px) as u32).any(|dy| img.get_pixel(x0 + dx, y0 + dy).0[0] < 160));
    assert!(sombre, "le texte est écrit à droite de la case");
    let b = &jap.layout.bars[0];
    let (bx, by) = ((((b.bar.center().x - jap.cadre.left()) * s_px) as u32), (((b.bar.center().y - jap.cadre.top()) * s_px) as u32));
    assert!((by - 1..=by + 1).any(|y| img.get_pixel(bx, y).0[0] < 250), "la barre horizontale est dessinée entre les deux cases");
}

fn c_note(p: &crate::planche::Planche, plan: u64) -> Rect {
    p.cartes.iter().find(|c| c.place.plan == plan).unwrap().place.note
}

// ------------------------------------------------------------------------------------------- la mémoire des réglages

#[test]
fn the_reading_and_the_fields_survive_save_and_reopen_old_files_get_the_defaults() {
    // Dans la disposition de la fenêtre (réglages de l'application) : écrite en toml et relue.
    let layout = Layout { board_sens: Sens::Japonais, board_textes: Textes { action: true, dialogue: true, notes: false }, board_columns: 5, ..Layout::default() };
    let texte = toml::to_string(&layout).expect("toml");
    assert!(texte.contains("sens_lecture") && texte.contains("japonais"), "{texte}");
    let relu: Layout = toml::from_str(&texte).expect("relu");
    assert_eq!((relu.board_sens, relu.board_textes, relu.board_columns), (Sens::Japonais, layout.board_textes, 5));
    // Dans les réglages complets, pour l'espace Vignettes.
    let mut reglages = crate::settings::Settings::default();
    reglages.set_layout_of(Espace::Vignettes, layout.clone());
    let relus: crate::settings::Settings = toml::from_str(&toml::to_string(&reglages).expect("toml")).expect("relus");
    let l = relus.layout_of(Espace::Vignettes).expect("la disposition retenue");
    assert_eq!((l.board_sens, l.board_textes), (Sens::Japonais, layout.board_textes));
    // Un fichier d'avant le panneau : les valeurs d'origine (en lignes, aucune ligne de texte depuis le 2026-10-30).
    let ancien: Layout = toml::from_str("colonnes_vignettes = 4\ntaille_vignettes = 200.0\n").expect("ancien fichier");
    assert_eq!((ancien.board_sens, ancien.board_textes, ancien.board_columns), (Sens::Lignes, Textes::aucun(), 4));
}

#[test]
fn the_choices_are_remembered_by_the_app_and_follow_the_user_from_one_space_to_the_next() {
    let mut b = banc_vignettes(film(3), Sens::Lignes, 6);
    assert_eq!(b.app.layout.board_sens, Sens::Lignes);
    b.app.run_action("vignettes.sens_japonais");
    b.app.run_action("vignettes.texte_dialogue");
    assert!(b.app.status.contains("dialogue"), "{}", b.app.status);
    assert!(b.app.settings_a_ecrire, "l'écriture des réglages est demandée à chaque changement");
    let courant = b.app.current_layout();
    assert_eq!(courant.board_sens, Sens::Japonais);
    assert!(courant.board_textes.dialogue && courant.board_textes.notes && !courant.board_textes.action);
    // D'un espace de travail à l'autre : le choix reste (comme la taille des cases).
    for espace in [Espace::Storyboard, Espace::Animation, Espace::Montage, Espace::Vignettes] {
        b.app.set_espace(espace);
        b.chauffer(2);
        assert_eq!(b.app.layout.board_sens, Sens::Japonais, "{espace:?}");
        assert!(b.app.layout.board_textes.dialogue, "{espace:?}");
    }
    // Rétablir la disposition de l'espace ne rend pas la planche : seuls les panneaux reviennent à l'origine.
    b.app.retablir_disposition();
    assert_eq!(b.app.layout.board_sens, Sens::Japonais);
    // Un clic sur le choix « En lignes » du panneau le rétablit, et la planche refait sa mise en page tout de suite.
    b.app.run_action("vignettes.sens_lignes");
    b.chauffer(3);
    assert_eq!(b.app.board_layout().sens, Sens::Lignes);
    assert!(b.app.board_layout().cards[1].thumb.left() > b.app.board_layout().cards[0].thumb.right());
}

// ------------------------------------------------------------------------------------------- le panneau

#[test]
fn the_panel_is_in_the_vignettes_layout_with_its_icon_its_command_and_its_name() {
    assert!(Espace::Vignettes.defaut().dock_layout().contains(Pane::Vignettes), "dans l'espace Vignettes");
    for e in [Espace::Storyboard, Espace::Animation, Espace::Montage] {
        assert!(!e.defaut().dock_layout().contains(Pane::Vignettes), "{e:?} ne l'a pas d'office");
    }
    assert_eq!(Pane::Vignettes.name(), "Planche");
    assert_eq!(serde_json::to_string(&Pane::Vignettes).unwrap(), "\"vignettes\"");
    assert!(Pane::ALL.contains(&Pane::Vignettes));
    assert_eq!(crate::barrepanneaux::pane_de_action("vue.vignettes_panneau"), Some(Pane::Vignettes));
    let icones: Vec<Pane> = crate::barrepanneaux::icones_de(Espace::Vignettes, false, &|_| false).into_iter().flatten().map(|e| e.pane).collect();
    assert!(icones.contains(&Pane::Vignettes), "{icones:?}");
    let ailleurs: Vec<Pane> = crate::barrepanneaux::icones_de(Espace::Storyboard, false, &|_| false).into_iter().flatten().map(|e| e.pane).collect();
    assert!(!ailleurs.contains(&Pane::Vignettes), "pas d'icône hors des Vignettes tant qu'il est fermé");
    for action in ["vue.vignettes_panneau", "vignettes.sens_lignes", "vignettes.sens_japonais", "vignettes.texte_action", "vignettes.texte_dialogue", "vignettes.texte_notes"] {
        assert!(crate::keymap::action_info(action).is_some_and(|a| a.implemented), "{action}");
    }
    // Il s'ouvre, au-dessus des réglages de l'outil, dans les autres espaces aussi.
    let mut d = Espace::Storyboard.defaut().dock_layout();
    d.open(Pane::Vignettes);
    assert!(d.contains(Pane::Vignettes));
    d.close(Pane::Vignettes);
    assert!(!d.contains(Pane::Vignettes));
}

#[test]
fn a_saved_vignettes_layout_from_before_the_panel_gets_it_once() {
    let mut s = crate::settings::Settings::default();
    // Une disposition retenue d'avant : celle d'origine d'hier (sans le panneau).
    let mut avant = Espace::Vignettes.defaut();
    let mut d = avant.dock_layout();
    d.close(Pane::Vignettes);
    avant.panneaux = Some(d);
    s.vignettes_layout = Some(avant);
    s.poser_le_panneau_vignettes();
    assert!(s.vignettes_layout.as_ref().unwrap().dock_layout().contains(Pane::Vignettes), "le panneau arrive avec l'onglet");
    assert!(s.panneau_vignettes_pose);
    // Fermé à la main ensuite : il reste fermé.
    let mut d = s.vignettes_layout.as_ref().unwrap().dock_layout();
    d.close(Pane::Vignettes);
    s.vignettes_layout.as_mut().unwrap().panneaux = Some(d);
    s.poser_le_panneau_vignettes();
    assert!(!s.vignettes_layout.as_ref().unwrap().dock_layout().contains(Pane::Vignettes));
    // Sans disposition retenue : rien à faire (l'origine du jour l'a).
    let mut vide = crate::settings::Settings::default();
    vide.poser_le_panneau_vignettes();
    assert!(vide.vignettes_layout.is_none());
}

#[test]
fn the_panel_draws_both_readings_and_the_three_fields_and_its_buttons_work() {
    let mut b = banc_vignettes(film(4), Sens::Lignes, 6);
    assert!(b.app.dock_layout().contains(Pane::Vignettes), "le panneau est à l'écran avec l'onglet");
    for morceau in ["En lignes", "À la japonaise"] {
        texte_a_l_ecran(&b, morceau);
    }
    for exact in ["Action", "Dialogue", "Notes"] {
        texte_exact(&b, exact);
    }
    // « À la japonaise » : la planche se range en colonnes et se remet à la taille de la vue.
    let p = texte_a_l_ecran(&b, "À la japonaise");
    clic(&mut b, p);
    assert_eq!(b.app.layout.board_sens, Sens::Japonais);
    let l = b.app.board_layout();
    assert_eq!(l.sens, Sens::Japonais);
    assert_eq!(l.cards[0].thumb.left(), l.cards[1].thumb.left());
    assert!(b.app.board_zoom > 0.0, "la planche est remise à la taille de la vue");
    // « Dialogue » se coche.
    let p = texte_exact(&b, "Dialogue");
    clic(&mut b, p);
    assert!(b.app.layout.board_textes.dialogue, "{:?}", b.app.layout.board_textes);
    let p = texte_exact(&b, "Dialogue");
    clic(&mut b, p);
    assert!(!b.app.layout.board_textes.dialogue);
    // « En lignes » revient.
    let p = texte_a_l_ecran(&b, "En lignes");
    clic(&mut b, p);
    assert_eq!(b.app.layout.board_sens, Sens::Lignes);
    // Le réglage « cases par colonne » est celui des cases par ligne.
    assert_eq!(b.app.layout.board_columns, 6);
}

// ------------------------------------------------------------------------------------------- les versions sur la planche

fn doc_avec_deux_versions() -> Document {
    let mut doc = quatre_plans();
    assert!(doc.nouvelle_version(0, 10).is_some());
    doc
}

fn etiquette_du_plan(b: &Banc, plan: u64) -> Option<Rect> {
    b.app.pile_versions.etiquettes.iter().find(|(_, p)| *p == plan).map(|(r, _)| *r)
}

#[test]
fn the_version_badge_is_at_the_top_right_of_the_case_only_with_two_or_more_versions() {
    for sens in [Sens::Lignes, Sens::Japonais] {
        let mut b = banc_vignettes(doc_avec_deux_versions(), sens, 4);
        let plan = b.app.doc.panels[0].id;
        let zone = etiquette_du_plan(&b, plan).unwrap_or_else(|| panic!("{sens:?} : l'étiquette est posée sur la case du plan"));
        let case = ecran(&b, b.app.board_layout().cards[0].thumb);
        // (Depuis le 2026-10-30 : hors du cadre, à côté du numéro du plan, au-dessus de l'image.)
        assert!(!zone.intersects(case), "{sens:?} : hors de la case : {zone:?} {case:?}");
        assert!(zone.bottom() <= case.top() + 1.0 && zone.left() >= case.left() - 1.0, "{sens:?} : au-dessus du cadre, près du numéro");
        assert!(b.textes.iter().any(|t| t == "V2"), "{sens:?} : « V2 » est écrit : {:?}", b.textes);
        // Les autres plans n'ont qu'une version : pas d'étiquette. La planche n'en pose qu'une.
        assert_eq!(b.app.pile_versions.etiquettes.len(), 1, "{sens:?}");
        // Une version de moins : plus d'étiquette.
        b.app.supprimer_la_version(0, 1);
        b.chauffer(3);
        assert!(etiquette_du_plan(&b, plan).is_none(), "{sens:?} : un plan à une seule version n'a pas d'étiquette");
        // Ctrl+Z la ramène.
        b.app.run_action("edition.annuler");
        b.chauffer(3);
        assert!(etiquette_du_plan(&b, plan).is_some(), "{sens:?}");
    }
    // Un film sans version : aucune étiquette, et la pile ne demande rien.
    let b = banc_vignettes(quatre_plans(), Sens::Lignes, 4);
    assert!(b.app.pile_versions.etiquettes.is_empty());
    assert!(b.app.vignettes.etiquettes.is_empty());
}

#[test]
fn hovering_the_badge_opens_the_list_picking_a_version_does_what_the_timeline_does_and_undo_works() {
    for sens in [Sens::Lignes, Sens::Japonais] {
        let mut b = banc_vignettes(doc_avec_deux_versions(), sens, 4);
        let mut temoin = banc_vignettes(doc_avec_deux_versions(), sens, 4);
        let plan = b.app.doc.panels[0].id;
        let zone = etiquette_du_plan(&b, plan).expect("l'étiquette");
        // Le survol ouvre la liste, au-dessus de l'étiquette.
        let ev = b.souris(zone.center().x, zone.center().y);
        b.image(ev, 1.0 / 60.0);
        b.image(Vec::new(), 1.0 / 60.0);
        assert_eq!(b.app.pile_versions.ouverte(), Some(plan), "{sens:?} : le survol ouvre la liste");
        let pile = b.app.pile_versions.zone.expect("la liste est posée");
        assert!(pile.top() < zone.top(), "{sens:?} : au-dessus de l'étiquette");
        // Un clic sur la carte V1 (la plus haute) passe le plan à V1.
        let carte = pile.min + vec2(crate::pilevers::CARTE_L / 2.0, crate::pilevers::BORD + crate::pilevers::CARTE_H / 2.0);
        let n0 = b.app.history.names().0.len();
        for p in [pos2(zone.center().x, (zone.top() + pile.bottom()) / 2.0), carte] {
            let ev = b.souris(p.x, p.y);
            b.image(ev, 1.0 / 60.0);
        }
        let clic = |pressed| Event::PointerButton { pos: carte, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        b.image(vec![clic(true)], 1.0 / 60.0);
        b.image(vec![clic(false)], 1.0 / 60.0);
        b.image(Vec::new(), 1.0 / 60.0);
        assert_eq!(b.app.doc.numero_de_version(plan), 1, "{sens:?} : le clic a passé le plan à V1");
        let (undo, _) = b.app.history.names();
        assert_eq!(&undo[n0..], ["Passer à la version V1"], "{sens:?} : la même étape d'annulation que la timeline : {undo:?}");
        // Exactement ce que fait le choix d'une version dans la timeline et dans le montage (le même chemin).
        temoin.app.passer_a_la_version(0, 1);
        temoin.chauffer(2);
        assert_eq!(temoin.app.doc.numero_de_version(plan), b.app.doc.numero_de_version(plan));
        assert_eq!(temoin.app.doc.panels[0].frame_count(), b.app.doc.panels[0].frame_count());
        assert_eq!(temoin.app.history.names().0.last(), b.app.history.names().0.last(), "{sens:?}");
        assert_eq!(temoin.app.history.names().0.len(), b.app.history.names().0.len(), "{sens:?} : le même nombre d'étapes");
        // La case montre cette version : l'étiquette dit « V1 ».
        b.chauffer(3);
        assert!(b.textes.iter().any(|t| t == "V1"), "{sens:?} : {:?}", b.textes);
        // Annuler rend V2.
        b.app.run_action("edition.annuler");
        b.chauffer(2);
        assert_eq!(b.app.doc.numero_de_version(plan), 2, "{sens:?} : Ctrl+Z");
    }
}

#[test]
fn pressing_the_badge_pins_the_list_and_draws_nothing_in_the_case() {
    let mut b = banc_vignettes(doc_avec_deux_versions(), Sens::Lignes, 4);
    let plan = b.app.doc.panels[0].id;
    let zone = etiquette_du_plan(&b, plan).expect("l'étiquette");
    let n = pas_d_annulation(&b);
    let p = zone.center();
    // (Un stylet qui ne survole pas : l'appui épingle la liste.)
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![appui(p, true, Modifiers::NONE)], 1.0 / 60.0);
    b.image(vec![Event::PointerMoved(p + vec2(3.0, 2.0))], 1.0 / 60.0);
    b.image(vec![appui(p + vec2(3.0, 2.0), false, Modifiers::NONE)], 1.0 / 60.0);
    b.chauffer(2);
    assert_eq!(b.app.pile_versions.ouverte(), Some(plan));
    assert!(b.app.pile_versions.epinglee, "épinglée jusqu'à un appui ailleurs");
    assert_eq!(pas_d_annulation(&b), n, "rien n'a été dessiné");
    assert!(!encre(&b.app.doc, 0));
}
