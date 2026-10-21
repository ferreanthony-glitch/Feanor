//! Essais des annotations de la vue Vignettes (modèle et fichier : `annotations.rs`).

use eframe::egui::{pos2, vec2, Pos2};

use super::*;
use crate::board::{self, BoardSettings};
use crate::document::Document;
use crate::project;
use crate::vignettes_tests::quatre_plans;

fn reglages(card_w: f32, colonnes: usize, lignes: usize) -> BoardSettings {
    BoardSettings { card_w, columns: colonnes, rows: lignes }
}

fn trait_en(c: Pos2, r: f32) -> Vec<VPoint> {
    vec![VPoint { x: c.x - 10.0, y: c.y - 3.0, r }, VPoint { x: c.x, y: c.y, r }, VPoint { x: c.x + 10.0, y: c.y + 4.0, r }]
}

fn dossier(nom: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-annotations-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier d'essai");
    dir
}

/// Le centre de la boîte d'un trait, sur la planche.
fn centre_du_trait(a: &Annotations, layout: &BoardLayout, i: usize) -> Pos2 {
    board::points_bounds(&a.points_planche(layout, i).expect("trait placé")).center()
}

#[test]
fn a_stroke_is_attached_to_the_nearest_case_and_kept_relative_to_it() {
    let doc = quatre_plans();
    let layout = board::layout(&doc, &reglages(200.0, 3, 0));
    let mut a = Annotations::default();
    let cible = layout.cards[4].thumb;
    assert!(a.ajouter(&layout, &trait_en(cible.center(), 2.0), COULEUR_PAR_DEFAUT));
    assert_eq!(a.traces.len(), 1);
    assert_eq!(a.traces[0].anchor.card, layout.cards[4].id);
    assert_eq!(a.traces[0].anchor.after, 0);
    let rel = &a.traces[0].points[1];
    assert!((rel.x - 0.5).abs() < 0.01, "x relatif {}", rel.x);
    assert!((rel.y - 0.5 * cible.height() / cible.width()).abs() < 0.01, "y relatif {}", rel.y);
    let back = a.points_planche(&layout, 0).unwrap();
    for (p, q) in back.iter().zip(trait_en(cible.center(), 2.0)) {
        assert!((p.x - q.x).abs() < 0.01 && (p.y - q.y).abs() < 0.01 && (p.r - q.r).abs() < 0.01);
    }
}

#[test]
fn the_nearest_case_wins_even_far_from_every_case() {
    let doc = quatre_plans();
    let layout = board::layout(&doc, &reglages(200.0, 3, 0));
    let mut a = Annotations::default();
    assert!(a.ajouter(&layout, &trait_en(layout.cards[0].thumb.left_top() - vec2(15.0, 24.0), 1.0), COULEUR_PAR_DEFAUT));
    assert_eq!(a.traces[0].anchor.card, layout.cards[0].id);
    let loin = pos2(layout.size.x + 400.0, layout.cards[3].thumb.center().y);
    assert!(a.ajouter(&layout, &trait_en(loin, 1.0), COULEUR_PAR_DEFAUT));
    let (rang, _) = case_proche(&layout, loin).unwrap();
    assert_eq!(a.traces[1].anchor.rank as usize, rang);
}

#[test]
fn a_stroke_follows_its_case_when_the_board_reflows_and_when_cases_grow() {
    let doc = quatre_plans();
    let l3 = board::layout(&doc, &reglages(200.0, 3, 0));
    let mut a = Annotations::default();
    let case = l3.cards[4].thumb;
    let c = case.left_top() + vec2(30.0, 20.0);
    assert!(a.ajouter(&l3, &trait_en(c, 2.0), COULEUR_PAR_DEFAUT));
    let rel_avant = (centre_du_trait(&a, &l3, 0) - case.left_top()) / case.width();
    for s in [reglages(200.0, 2, 0), reglages(200.0, 5, 0), reglages(400.0, 3, 0), reglages(120.0, 6, 2)] {
        let l = board::layout(&doc, &s);
        let c4 = l.cards[4].thumb;
        let rel = (centre_du_trait(&a, &l, 0) - c4.left_top()) / c4.width();
        assert!((rel - rel_avant).length() < 1e-3, "réglages {s:?} : le trait a bougé dans sa case ({rel:?} contre {rel_avant:?})");
        let pts = a.points_planche(&l, 0).unwrap();
        assert!((pts[1].r / c4.width() - 2.0 / case.width()).abs() < 1e-4);
    }
    let big = board::layout(&doc, &reglages(400.0, 3, 0));
    assert!((a.points_planche(&big, 0).unwrap()[1].r - 4.0).abs() < 1e-3);
}

#[test]
fn a_stroke_in_an_empty_case_stays_in_the_same_case_when_it_fills() {
    let mut doc = quatre_plans();
    let s = reglages(200.0, 4, 0);
    let layout = board::layout(&doc, &s);
    let vide = layout.empty[1];
    let mut a = Annotations::default();
    assert!(a.ajouter(&layout, &trait_en(vide.center(), 2.0), COULEUR_PAR_DEFAUT));
    let t = &a.traces[0];
    assert_eq!(t.anchor.card, layout.cards.last().unwrap().id, "attachée à la dernière vignette");
    assert_eq!(t.anchor.after, 2, "la deuxième case vide après elle");
    assert!(layout.empty[1].contains(centre_du_trait(&a, &layout, 0)));
    let last = doc.panels.len() - 1;
    doc.add_scene_after(last);
    let l2 = board::layout(&doc, &s);
    assert_eq!(l2.cards.len(), layout.cards.len() + 1);
    let after = centre_du_trait(&a, &l2, 0);
    let visee = cases(&l2)[layout.cards.len() + 1];
    assert!(visee.contains(after), "le trait est resté dans la même case");
}

#[test]
fn eraser_only_removes_the_strokes_it_touches_and_nothing_else() {
    let mut doc = quatre_plans();
    doc.board.strokes.push(board::NoteStroke { points: trait_en(pos2(50.0, 50.0), 2.0), color: [1, 2, 3], anchor: None });
    let layout = board::layout(&doc, &reglages(200.0, 3, 0));
    let mut a = Annotations::default();
    let (c0, c5) = (layout.cards[0].thumb.center(), layout.cards[5].thumb.center());
    a.ajouter(&layout, &trait_en(c0, 2.0), COULEUR_PAR_DEFAUT);
    a.ajouter(&layout, &trait_en(c5, 2.0), COULEUR_PAR_DEFAUT);
    assert!(a.touche(&layout, c0, 3.0) && !a.touche(&layout, c0 + vec2(0.0, 80.0), 3.0));
    assert_eq!(a.effacer_en(&layout, c0, 3.0), 1);
    assert_eq!(a.traces.len(), 1);
    assert_eq!(a.traces[0].anchor.card, layout.cards[5].id);
    assert_eq!(a.effacer_en(&layout, c0, 3.0), 0);
    assert_eq!(doc.board.strokes.len(), 1);
    assert_eq!(doc.panels.len(), 4);
}

#[test]
fn when_a_case_disappears_its_strokes_go_to_the_nearest_neighbour_at_the_same_place() {
    let mut doc = quatre_plans();
    let s = reglages(200.0, 3, 0);
    let avant = board::layout(&doc, &s);
    let mut a = Annotations::default();
    let c = avant.cards[3].thumb.center() + vec2(60.0, 10.0);
    a.ajouter(&avant, &trait_en(c, 2.0), COULEUR_PAR_DEFAUT);
    assert_eq!(a.traces[0].anchor.card, avant.cards[3].id);
    let absolu_avant = a.points_planche(&avant, 0).unwrap();
    let id = avant.cards[3].plan;
    doc.remove_panels(&[id]).expect("plan retiré");
    let apres = board::layout(&doc, &s);
    assert!(apres.card_of(avant.cards[3].id).is_none());
    assert_eq!(a.rattacher_manquantes(Some(&avant), &apres), 1);
    let t = &a.traces[0];
    assert!(apres.card_of(t.anchor.card).is_some(), "attaché à une vignette qui reste");
    let (_, r) = case_de(&avant, &t.anchor).expect("sa nouvelle case existe aussi sur l'ancienne planche");
    for (p, q) in t.points.iter().zip(&absolu_avant) {
        let abs = absolu(p, r);
        assert!((abs.x - q.x).abs() < 0.01 && (abs.y - q.y).abs() < 0.01 && (abs.r - q.r).abs() < 0.01);
    }
    assert_eq!(a.rattacher_manquantes(Some(&avant), &apres), 0);
    assert!(a.points_planche(&apres, 0).is_some());
}

#[test]
fn without_the_old_board_a_lost_stroke_falls_back_to_the_case_of_the_same_rank() {
    let doc = quatre_plans();
    let layout = board::layout(&doc, &reglages(200.0, 3, 0));
    let mut a = Annotations::default();
    a.traces.push(Trace { anchor: Ancre { card: 987_654, after: 0, rank: 2 }, color: COULEUR_PAR_DEFAUT, points: vec![VPoint { x: 0.1, y: 0.1, r: 0.01 }] });
    a.traces.push(Trace { anchor: Ancre { card: 987_655, after: 0, rank: 99 }, color: COULEUR_PAR_DEFAUT, points: vec![VPoint { x: 0.1, y: 0.1, r: 0.01 }] });
    assert!(a.points_planche(&layout, 0).is_none(), "orpheline : rien n'est montré tant qu'on ne l'a pas rattachée");
    assert_eq!(a.rattacher_manquantes(None, &layout), 2);
    assert_eq!(a.traces[0].anchor.card, layout.cards[2].id);
    assert_eq!(a.traces[1].anchor.card, layout.cards.last().unwrap().id, "un rang trop grand tombe sur la dernière");
    assert!(a.points_planche(&layout, 0).is_some() && a.points_planche(&layout, 1).is_some());
}

#[test]
fn ranks_follow_the_board() {
    let mut doc = quatre_plans();
    let s = reglages(200.0, 3, 0);
    let layout = board::layout(&doc, &s);
    let mut a = Annotations::default();
    a.ajouter(&layout, &trait_en(layout.cards[4].thumb.center(), 2.0), COULEUR_PAR_DEFAUT);
    assert_eq!(a.traces[0].anchor.rank, 4);
    doc.add_panel_after(0);
    let l2 = board::layout(&doc, &s);
    assert!(a.noter_rangs(&l2));
    let k = l2.cards.iter().position(|c| c.id == a.traces[0].anchor.card).unwrap();
    assert_eq!(a.traces[0].anchor.rank as usize, k);
    assert!(!a.noter_rangs(&l2), "rien à changer au second passage");
}

#[test]
fn simplifying_keeps_the_ends_and_caps_the_length() {
    let pts: Vec<VPoint> = (0..1000).map(|i| VPoint { x: i as f32 * 0.1, y: 0.0, r: 1.0 }).collect();
    let e = epurer(&pts, 0.5);
    assert!(e.len() < 300 && e.first() == pts.first() && e.last() == pts.last());
    let r = reduire(&pts, 50);
    assert_eq!(r.len(), 50);
    assert_eq!((r[0], r[49]), (pts[0], pts[999]));
    assert_eq!(reduire(&pts[..10], 50).len(), 10);
}

// ------------------------------------------------------------------------------------ le fichier

fn doc_avec_traits() -> Document {
    let mut doc = quatre_plans();
    let layout = board::layout(&doc, &reglages(200.0, 3, 0));
    doc.annotations.ajouter(&layout, &trait_en(layout.cards[1].thumb.center(), 2.5), [10, 20, 30]);
    doc.annotations.ajouter(&layout, &trait_en(layout.empty[0].center(), 3.5), COULEUR_PAR_DEFAUT);
    doc
}

fn proche(a: &Annotations, b: &Annotations) -> bool {
    a.traces.len() == b.traces.len()
        && a.traces.iter().zip(&b.traces).all(|(x, y)| {
            x.anchor.card == y.anchor.card
                && x.anchor.after == y.anchor.after
                && x.color == y.color
                && x.points.len() == y.points.len()
                && x.points.iter().zip(&y.points).all(|(p, q)| (p.x - q.x).abs() < 1e-3 && (p.y - q.y).abs() < 1e-3 && (p.r - q.r).abs() < 1e-3)
        })
}

#[test]
fn the_sidecar_round_trips_and_is_absent_without_annotations() {
    let doc = doc_avec_traits();
    assert!(to_bytes(&quatre_plans()).unwrap().is_none(), "aucune annotation : aucun fichier");
    let bytes = to_bytes(&doc).unwrap().expect("des annotations");
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("shot-annotations") && text.contains("\"vignette\"") && text.contains("\"apres\""));
    let back = parse(&text).expect("relu");
    assert!(proche(&doc.annotations, &back));
    assert!(proche(&parse(&format!("\u{feff}{text}")).expect("avec un BOM"), &back));
}

#[test]
fn a_damaged_sidecar_is_refused_and_a_hostile_one_is_cleaned() {
    assert!(parse("pas du json").is_err());
    assert!(parse("{}").is_err());
    assert!(parse(r#"{"format":"autre","version":1,"traces":[]}"#).is_err());
    let err = parse(r#"{"format":"shot-annotations","version":99,"traces":[]}"#).unwrap_err();
    assert!(err.contains("plus récente"), "{err}");
    let text = r#"{"format":"shot-annotations","version":1,"traces":[
        {"ancre":{"vignette":5},"couleur":[1,2,3],"points":[]},
        {"ancre":{"vignette":5,"apres":2,"rang":7},"couleur":[1,2,3],"points":[[0.1,0.2,0.01],[0.3,0.4,9999.0]]}
    ]}"#;
    let a = parse(text).expect("lisible");
    assert_eq!(a.traces.len(), 1, "le trait vide est écarté");
    assert_eq!(a.traces[0].anchor, Ancre { card: 5, after: 2, rank: 7 });
    assert!(a.traces[0].points[1].r <= 50.0, "rayon borné");
    // Un point qui n'est pas un nombre : le fichier est refusé en entier (jamais à moitié lu).
    assert!(parse(r#"{"format":"shot-annotations","version":1,"traces":[{"ancre":{"vignette":6},"couleur":[4,5,6],"points":[[0.1,null,0.01]]}]}"#).is_err());
    let beaucoup: Vec<String> = (0..MAX_TRACES + 50).map(|i| format!(r#"{{"ancre":{{"vignette":{i}}},"couleur":[0,0,0],"points":[[0.1,0.1,0.01]]}}"#)).collect();
    let big = format!(r#"{{"format":"shot-annotations","version":1,"traces":[{}]}}"#, beaucoup.join(","));
    assert_eq!(parse(&big).unwrap().traces.len(), MAX_TRACES);
}

#[test]
fn a_project_saves_and_reopens_its_annotations_and_drops_the_file_when_there_are_none() {
    let dir = dossier("aller-retour");
    let manifest = dir.join("P.pelure");
    let doc = doc_avec_traits();
    let mut written = Default::default();
    project::save(&doc, &manifest, &mut written).expect("enregistrement");
    let fichier = dir.join(FILE);
    assert!(fichier.exists(), "annotations.json est à côté du manifeste");
    let loaded = project::load(&manifest).expect("ouverture");
    assert!(loaded.warnings.iter().all(|w| !w.contains(FILE)), "{:?}", loaded.warnings);
    assert!(proche(&doc.annotations, &loaded.doc.annotations));
    assert!(!std::fs::read_to_string(&manifest).unwrap().contains("annotation"));
    let mut vide = loaded.doc;
    vide.annotations = Annotations::default();
    project::save(&vide, &manifest, &mut loaded.written.clone()).expect("enregistrement");
    assert!(!fichier.exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_unreadable_sidecar_never_stops_opening_and_is_never_destroyed() {
    for (nom, contenu) in [("abime", "{ pas du json".to_string()), ("futur", r#"{"format":"shot-annotations","version":9,"traces":[]}"#.to_string())] {
        let dir = dossier(nom);
        let manifest = dir.join("P.pelure");
        project::save(&Document::with_frame(160, 90, 24), &manifest, &mut Default::default()).unwrap();
        std::fs::write(dir.join(FILE), &contenu).unwrap();
        let loaded = project::load(&manifest).expect("le projet s'ouvre quand même");
        let avis: Vec<&String> = loaded.warnings.iter().filter(|w| w.contains(FILE)).collect();
        assert_eq!(avis.len(), 1, "{nom} : un avertissement ({:?})", loaded.warnings);
        assert!(loaded.doc.annotations.is_empty());
        project::save(&loaded.doc, &manifest, &mut loaded.written.clone()).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join(FILE)).unwrap(), contenu, "{nom} : fichier intact");
        let mut doc = loaded.doc;
        let layout = board::layout(&doc, &reglages(200.0, 3, 0));
        doc.annotations.ajouter(&layout, &trait_en(layout.cards[0].thumb.center(), 2.0), COULEUR_PAR_DEFAUT);
        project::save(&doc, &manifest, &mut loaded.written.clone()).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join(format!("{FILE}.illisible"))).unwrap(), contenu, "{nom} : l'ancien est gardé");
        assert!(parse(&std::fs::read_to_string(dir.join(FILE)).unwrap()).is_ok());
        std::fs::remove_dir_all(&dir).ok();
    }
}

#[test]
fn annotations_come_back_with_undo_and_are_not_in_the_exports_snapshot() {
    use crate::history::{Edit, History, Nav};
    let mut doc = doc_avec_traits();
    let mut history = History::default();
    let nav = Nav { panel: 0, layer: 0 };
    history.push(Edit::Structure { structure: doc.structure(), nav });
    doc.annotations = Annotations::default();
    let mut nav2 = nav;
    assert!(history.undo(&mut doc, &mut nav2));
    assert_eq!(doc.annotations.traces.len(), 2, "annuler rétablit les annotations");
    let s = crate::exportrender::snapshot_of(&doc, &[0, 1]);
    assert!(s.annotations.is_empty(), "le cliché d'un export ne copie pas les annotations");
}
