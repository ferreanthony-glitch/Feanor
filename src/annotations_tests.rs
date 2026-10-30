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

/// La planche comme à l'écran : avec les cases pâles (où une annotation se pose aussi).
fn pale(doc: &Document, s: &BoardSettings) -> BoardLayout {
    board::layout_with(doc, s, &Default::default(), false, &board::Options::pale(3))
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
    let layout = pale(&doc, &reglages(200.0, 3, 0));
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
    let layout = pale(&doc, &reglages(200.0, 3, 0));
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
    let l3 = pale(&doc, &reglages(200.0, 3, 0));
    let mut a = Annotations::default();
    let case = l3.cards[4].thumb;
    let c = case.left_top() + vec2(30.0, 20.0);
    assert!(a.ajouter(&l3, &trait_en(c, 2.0), COULEUR_PAR_DEFAUT));
    let rel_avant = (centre_du_trait(&a, &l3, 0) - case.left_top()) / case.width();
    for s in [reglages(200.0, 2, 0), reglages(200.0, 5, 0), reglages(400.0, 3, 0), reglages(120.0, 6, 2)] {
        let l = pale(&doc, &s);
        let c4 = l.cards[4].thumb;
        let rel = (centre_du_trait(&a, &l, 0) - c4.left_top()) / c4.width();
        assert!((rel - rel_avant).length() < 1e-3, "réglages {s:?} : le trait a bougé dans sa case ({rel:?} contre {rel_avant:?})");
        let pts = a.points_planche(&l, 0).unwrap();
        assert!((pts[1].r / c4.width() - 2.0 / case.width()).abs() < 1e-4);
    }
    let big = pale(&doc, &reglages(400.0, 3, 0));
    assert!((a.points_planche(&big, 0).unwrap()[1].r - 4.0).abs() < 1e-3);
}

#[test]
fn a_stroke_in_an_empty_case_stays_in_the_same_case_when_it_fills() {
    let mut doc = quatre_plans();
    let s = reglages(200.0, 4, 0);
    let layout = pale(&doc, &s);
    let vide = layout.empty[0];
    let mut a = Annotations::default();
    assert!(a.ajouter(&layout, &trait_en(vide.center(), 2.0), COULEUR_PAR_DEFAUT));
    let t = &a.traces[0];
    assert_eq!(t.anchor.card, layout.cards.last().unwrap().id, "attachée à la dernière vignette");
    assert_eq!(t.anchor.after, 1, "la case vide qui suit la dernière vignette");
    assert!(layout.empty[0].contains(centre_du_trait(&a, &layout, 0)));
    let last = doc.panels.len() - 1;
    doc.add_scene_after(last);
    let l2 = pale(&doc, &s);
    assert_eq!(l2.cards.len(), layout.cards.len() + 1);
    let after = centre_du_trait(&a, &l2, 0);
    let visee = cases(&l2)[layout.cards.len()];
    assert!(visee.contains(after), "le trait est resté dans la même case");
}

#[test]
fn eraser_only_removes_the_strokes_it_touches_and_nothing_else() {
    let mut doc = quatre_plans();
    doc.board.strokes.push(board::NoteStroke { points: trait_en(pos2(50.0, 50.0), 2.0), color: [1, 2, 3], anchor: None });
    let layout = pale(&doc, &reglages(200.0, 3, 0));
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
    let avant = pale(&doc, &s);
    let mut a = Annotations::default();
    let c = avant.cards[3].thumb.center() + vec2(60.0, 10.0);
    a.ajouter(&avant, &trait_en(c, 2.0), COULEUR_PAR_DEFAUT);
    assert_eq!(a.traces[0].anchor.card, avant.cards[3].id);
    let absolu_avant = a.points_planche(&avant, 0).unwrap();
    let id = avant.cards[3].plan;
    doc.remove_panels(&[id]).expect("plan retiré");
    let apres = pale(&doc, &s);
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
    let layout = pale(&doc, &reglages(200.0, 3, 0));
    let mut a = Annotations::default();
    let calque = a.calque_ou_premier(None);
    a.traces.push(Trace::new(calque, Ancre { card: 987_654, after: 0, rank: 2 }, COULEUR_PAR_DEFAUT, vec![VPoint { x: 0.1, y: 0.1, r: 0.01 }]));
    a.traces.push(Trace::new(calque, Ancre { card: 987_655, after: 0, rank: 99 }, COULEUR_PAR_DEFAUT, vec![VPoint { x: 0.1, y: 0.1, r: 0.01 }]));
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
    let layout = pale(&doc, &s);
    let mut a = Annotations::default();
    a.ajouter(&layout, &trait_en(layout.cards[4].thumb.center(), 2.0), COULEUR_PAR_DEFAUT);
    assert_eq!(a.traces[0].anchor.rank, 4);
    doc.add_panel_after(0);
    let l2 = pale(&doc, &s);
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
    let layout = pale(&doc, &reglages(200.0, 3, 0));
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
        let layout = pale(&doc, &reglages(200.0, 3, 0));
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

// ------------------------------------------------------------------------------------ les calques

/// Un fichier `annotations.json` tel que la version d'avant les calques l'écrivait (version 1, traits à plat), avec trois traits sur
/// les cases d'un film de quatre plans : les mêmes structures, le même `to_vec_pretty`.
fn fichier_ancien(doc: &Document) -> String {
    #[derive(Serialize)]
    struct Vieux {
        ancre: Ancre,
        couleur: [u8; 3],
        points: Vec<[f32; 3]>,
    }
    #[derive(Serialize)]
    struct VieuxFichier {
        format: String,
        version: u32,
        traces: Vec<Vieux>,
    }
    let carte = |k: usize| doc.panels[k].id;
    let f = VieuxFichier {
        format: "shot-annotations".into(),
        version: 1,
        traces: vec![
            // (Des valeurs que l'ancienne version écrivait telles quelles : quatre décimales au plus.)
            Vieux { ancre: Ancre { card: carte(0), after: 0, rank: 0 }, couleur: [220, 38, 38], points: vec![[0.25, 0.125, 0.0625], [0.5, 0.25, 0.0625], [0.75, 0.3125, 0.03]] },
            Vieux { ancre: Ancre { card: carte(3), after: 0, rank: 4 }, couleur: [30, 90, 200], points: vec![[0.5, 0.5, 0.0625]] },
            Vieux { ancre: Ancre { card: carte(3), after: 3, rank: 7 }, couleur: [10, 160, 60], points: vec![[0.125, 0.25, 0.0156], [0.875, 0.25, 0.0156]] },
        ],
    };
    String::from_utf8(serde_json::to_vec_pretty(&f).unwrap()).unwrap()
}

#[test]
fn an_old_annotations_file_opens_as_one_layer_with_the_same_strokes_in_the_same_places() {
    let doc = quatre_plans();
    let texte = fichier_ancien(&doc);
    let a = parse(&texte).expect("l'ancien fichier se lit");
    assert_eq!(a.calques.len(), 1, "un seul calque");
    let c = &a.calques[0];
    assert_eq!((c.nom.as_str(), c.visible), (NOM_PAR_DEFAUT, true));
    assert_eq!(a.traces.len(), 3, "les trois traits, dans l'ordre");
    assert!(a.traces.iter().all(|t| t.calque == c.id), "tous dans ce calque");
    let attendu: [(u64, u32, u32, [u8; 3], &[[f32; 3]]); 3] = [
        (doc.panels[0].id, 0, 0, [220, 38, 38], &[[0.25, 0.125, 0.0625], [0.5, 0.25, 0.0625], [0.75, 0.3125, 0.03]]),
        (doc.panels[3].id, 0, 4, [30, 90, 200], &[[0.5, 0.5, 0.0625]]),
        (doc.panels[3].id, 3, 7, [10, 160, 60], &[[0.125, 0.25, 0.0156], [0.875, 0.25, 0.0156]]),
    ];
    for (t, (carte, apres, rang, couleur, pts)) in a.traces.iter().zip(attendu) {
        assert_eq!((t.anchor, t.color), (Ancre { card: carte, after: apres, rank: rang }, couleur));
        assert_eq!(t.points.iter().map(|p| [p.x, p.y, p.r]).collect::<Vec<_>>(), pts.to_vec(), "les points, au bit près");
    }
    // Sur la planche : exactement où l'ancienne version les posait (la case, puis le point relatif à elle).
    for reglage in [reglages(200.0, 3, 0), reglages(120.0, 6, 0)] {
        let layout = pale(&doc, &reglage);
        for (i, t) in a.traces.iter().enumerate() {
            let (_, r) = case_de(&layout, &t.anchor).expect("sa case");
            let vus = a.points_planche(&layout, i).expect("placé");
            let voulus: Vec<VPoint> = t.points.iter().map(|p| VPoint { x: r.left() + p.x * r.width(), y: r.top() + p.y * r.width(), r: p.r * r.width() }).collect();
            assert_eq!(vus, voulus);
        }
    }
    // Le dessin : un seul calque visible, ordre de peinture = ordre du fichier.
    assert_eq!(a.ordre_de_peinture(), vec![0, 1, 2]);
    // Réécrit, le fichier est de la version 2, et se relit pareil.
    let mut doc2 = quatre_plans();
    doc2.annotations = a.clone();
    let ecrit = String::from_utf8(to_bytes(&doc2).unwrap().expect("à écrire")).unwrap();
    assert!(ecrit.contains("\"version\": 2") && ecrit.contains("\"calques\"") && !ecrit.contains("\"traces\": [\n    {"), "{ecrit}");
    assert_eq!(parse(&ecrit).unwrap(), a);
}

#[test]
fn a_project_with_an_old_annotations_file_opens_saves_in_the_new_format_and_reopens_the_same() {
    for nom_manifeste in ["P.shot", "P.pelure"] {
        let dir = dossier(&format!("ancien-{}", nom_manifeste.replace('.', "-")));
        let doc = quatre_plans();
        let texte = fichier_ancien(&doc);
        project::save(&doc, &dir.join("P.shot"), &mut Default::default()).expect("enregistrement");
        if nom_manifeste == "P.pelure" {
            // Un projet d'avant le `.shot` : le même manifeste sous l'ancien nom.
            std::fs::copy(dir.join("P.shot"), dir.join("P.pelure")).unwrap();
            std::fs::remove_file(dir.join("P.shot")).unwrap();
        }
        let manifeste = dir.join(nom_manifeste);
        std::fs::write(dir.join(FILE), &texte).unwrap();
        let loaded = project::load(&manifeste).expect("ouverture");
        assert!(loaded.warnings.iter().all(|w| !w.contains(FILE)), "{nom_manifeste} : {:?}", loaded.warnings);
        let a = &loaded.doc.annotations;
        assert_eq!((a.calques.len(), a.traces.len()), (1, 3), "{nom_manifeste} : un calque, trois traits");
        assert_eq!(a.calques[0].nom, NOM_PAR_DEFAUT);
        assert_eq!(a, &parse(&texte).unwrap(), "{nom_manifeste} : ce que le fichier dit");
        // L'enregistrement écrit le nouveau format, qui se relit identique.
        let mut doc2 = loaded.doc;
        doc2.annotations.regler_visible(doc2.annotations.calques[0].id, true);
        project::save(&doc2, &manifeste, &mut loaded.written.clone()).expect("réenregistrement");
        let neuf = std::fs::read_to_string(dir.join(FILE)).unwrap();
        assert!(neuf.contains("\"version\": 2") && neuf.contains("\"calques\""), "{nom_manifeste} : {neuf}");
        let relu = project::load(&manifeste).expect("ouverture");
        assert_eq!(relu.doc.annotations, doc2.annotations, "{nom_manifeste} : aller-retour");
        std::fs::remove_dir_all(&dir).ok();
    }
}

#[test]
fn layers_can_be_added_renamed_hidden_reordered_and_deleted_with_their_strokes() {
    let doc = quatre_plans();
    let layout = pale(&doc, &reglages(200.0, 3, 0));
    let mut a = Annotations::default();
    assert!(a.rien_a_ecrire() && a.calques.is_empty());
    // Le premier trait crée « Annotations » ; un calque de plus s'appelle « Annotations 2 ».
    assert!(a.ajouter(&layout, &trait_en(layout.cards[0].thumb.center(), 2.0), COULEUR_PAR_DEFAUT));
    let un = a.calques[0].id;
    assert_eq!((a.calques.len(), a.calques[0].nom.as_str()), (1, NOM_PAR_DEFAUT));
    assert!(!a.rien_a_ecrire());
    let deux = a.ajouter_calque().expect("un calque de plus");
    let trois = a.ajouter_calque().expect("et un autre");
    assert_eq!(a.calques.iter().map(|c| c.nom.as_str()).collect::<Vec<_>>(), ["Annotations", "Annotations 2", "Annotations 3"]);
    assert!(a.calques.iter().map(|c| c.id).collect::<std::collections::BTreeSet<_>>().len() == 3, "des identifiants tous différents");
    // Des traits dans chacun.
    assert!(a.ajouter_dans(&layout, Some(deux), &trait_en(layout.cards[1].thumb.center(), 2.0), [1, 2, 3]));
    assert!(a.ajouter_dans(&layout, Some(trois), &trait_en(layout.cards[2].thumb.center(), 2.0), [4, 5, 6]));
    assert!(a.ajouter_dans(&layout, Some(deux), &trait_en(layout.cards[4].thumb.center(), 2.0), [7, 8, 9]));
    assert_eq!((a.traits_du_calque(un), a.traits_du_calque(deux), a.traits_du_calque(trois)), (1, 2, 1));
    // Renommer : un nom vide ne change rien, le nom est taillé aux bords.
    assert!(!a.renommer_calque(deux, "   "));
    assert!(a.renommer_calque(deux, "  Reprises du réalisateur  "));
    assert_eq!(a.calque(deux).unwrap().nom, "Reprises du réalisateur");
    assert!(!a.renommer_calque(deux, "Reprises du réalisateur"), "même nom : rien n'a changé");
    // Monter, descendre : au bord, rien.
    assert!(a.deplacer_calque(un, true));
    assert_eq!(a.calques.iter().map(|c| c.id).collect::<Vec<_>>(), [deux, un, trois]);
    assert!(!a.deplacer_calque(trois, true), "déjà tout en haut");
    assert!(!a.deplacer_calque(deux, false), "déjà tout en bas");
    assert!(a.deplacer_calque(trois, false));
    assert_eq!(a.calques.iter().map(|c| c.id).collect::<Vec<_>>(), [deux, trois, un]);
    // L'ordre de peinture suit les calques : les traits de `deux` d'abord, puis `trois`, puis `un`.
    let ordre: Vec<u64> = a.ordre_de_peinture().iter().map(|&i| a.traces[i].calque).collect();
    assert_eq!(ordre, [deux, deux, trois, un]);
    // Cacher un calque : ses traits ne sont plus peints, ni cadrés, mais ils sont toujours là.
    assert!(a.regler_visible(trois, false));
    assert_eq!(a.ordre_de_peinture().iter().map(|&i| a.traces[i].calque).collect::<Vec<_>>(), [deux, deux, un]);
    assert_eq!(a.traits_du_calque(trois), 1);
    assert!(a.etendue(&layout).is_some());
    // Supprimer : le calque et ses traits partent, ceux des autres restent.
    assert_eq!(a.supprimer_calque(deux), Some(2));
    assert_eq!((a.calques.len(), a.traces.len()), (2, 2));
    assert!(a.traces.iter().all(|t| t.calque != deux));
    assert_eq!(a.supprimer_calque(deux), None, "déjà parti");
    // Un calque caché, ou renommé, est à écrire (rien ne se perd), même sans trait.
    let mut b = Annotations::default();
    let seul = b.calque_ou_premier(None);
    assert!(b.rien_a_ecrire(), "le calque par défaut, vide : pas de fichier");
    b.regler_visible(seul, false);
    assert!(!b.rien_a_ecrire());
    b.regler_visible(seul, true);
    b.renommer_calque(seul, "Mes notes");
    assert!(!b.rien_a_ecrire());
    // Le fichier garde les calques sans trait, leur nom, leur ordre et leur œil.
    let mut d = quatre_plans();
    d.annotations = a.clone();
    d.annotations.ajouter_calque().unwrap();
    let ecrit = String::from_utf8(to_bytes(&d).unwrap().unwrap()).unwrap();
    let relu = parse(&ecrit).unwrap();
    // (Le fichier arrondit les points à quatre décimales, et range les traits calque par calque : les calques sont identiques, les traits proches
    // et peints dans le même ordre.)
    assert_eq!(relu.calques, d.annotations.calques);
    let peint = |a: &Annotations| Annotations { calques: a.calques.clone(), traces: a.calques.iter().flat_map(|c| a.traces.iter().filter(move |t| t.calque == c.id).cloned()).collect(), ..Default::default() };
    assert!(proche(&peint(&relu), &peint(&d.annotations)) && peint(&relu).traces.iter().zip(&peint(&d.annotations).traces).all(|(x, y)| x.calque == y.calque));
    assert_eq!(relu.calques.iter().map(|c| (c.nom.clone(), c.visible)).collect::<Vec<_>>(), d.annotations.calques.iter().map(|c| (c.nom.clone(), c.visible)).collect::<Vec<_>>());
}

#[test]
fn a_hostile_file_cannot_lose_strokes_or_break_the_layers() {
    // Des calques de même identifiant, un trait dans un calque qui n'existe pas : rien ne se perd, tout est dans un calque.
    let trait_ = r#"{"ancre":{"vignette":5},"couleur":[1,2,3],"points":[[0.1,0.2,0.01]]}"#;
    let texte = format!(
        r#"{{"format":"shot-annotations","version":2,"calques":[
            {{"id":4,"nom":"A","visible":true,"traces":[{trait_}]}},
            {{"id":4,"nom":"B","visible":false,"traces":[{trait_},{trait_}]}},
            {{"id":0,"nom":"  ","traces":[{trait_}]}}
        ]}}"#
    );
    let a = parse(&texte).expect("lisible");
    assert_eq!(a.traces.len(), 4, "aucun trait perdu");
    let ids: std::collections::BTreeSet<u64> = a.calques.iter().map(|c| c.id).collect();
    assert_eq!(ids.len(), 3, "des identifiants tous différents : {:?}", a.calques);
    assert!(a.traces.iter().all(|t| a.calque(t.calque).is_some()), "chaque trait a son calque");
    assert_eq!(a.calques[2].nom, NOM_PAR_DEFAUT, "un nom vide devient le nom d'origine");
    assert!(!a.calques[1].visible);
    assert_eq!(a.calques.iter().map(|c| a.traits_du_calque(c.id)).collect::<Vec<_>>(), [1, 2, 1], "chaque calque garde ses traits");
    // Trop de calques : les traits des calques en trop passent au dernier gardé.
    let beaucoup: Vec<String> = (1..=MAX_CALQUES + 5).map(|i| format!(r#"{{"id":{i},"nom":"c{i}","traces":[{trait_}]}}"#)).collect();
    let gros = parse(&format!(r#"{{"format":"shot-annotations","version":2,"calques":[{}]}}"#, beaucoup.join(","))).unwrap();
    assert_eq!(gros.calques.len(), MAX_CALQUES);
    assert_eq!(gros.traces.len(), MAX_CALQUES + 5, "les traits des calques en trop sont gardés");
    assert!(gros.traces.iter().all(|t| gros.calque(t.calque).is_some()));
    // La version 3 d'un futur est refusée, jamais lue à moitié.
    assert!(parse(r#"{"format":"shot-annotations","version":3,"calques":[]}"#).unwrap_err().contains("plus récente"));
}

#[test]
fn transforming_moves_scales_and_rotates_strokes_and_reattaches_them_to_the_nearest_case() {
    let doc = quatre_plans();
    let layout = pale(&doc, &reglages(200.0, 3, 0));
    let mut a = Annotations::default();
    let c1 = layout.cards[1].thumb.center();
    assert!(a.ajouter(&layout, &trait_en(c1, 2.0), COULEUR_PAR_DEFAUT));
    assert!(a.ajouter(&layout, &trait_en(layout.cards[5].thumb.center(), 2.0), COULEUR_PAR_DEFAUT));
    let (premier, autre) = (a.traces[0].id, a.traces[1].id);
    let avant = a.points_planche_de(&layout, premier).unwrap();
    let autre_avant = a.points_planche_de(&layout, autre).unwrap();
    // Déplacer : tous les points suivent, le rayon ne change pas ; l'autre trait ne bouge pas.
    assert_eq!(a.transformer(&layout, &[premier], &Affine::translate(vec2(15.0, -7.0))), 1);
    let apres = a.points_planche_de(&layout, premier).unwrap();
    for (p, q) in avant.iter().zip(&apres) {
        assert!((q.x - p.x - 15.0).abs() < 1e-2 && (q.y - p.y + 7.0).abs() < 1e-2 && (q.r - p.r).abs() < 1e-3);
    }
    assert_eq!(a.points_planche_de(&layout, autre).unwrap(), autre_avant);
    // Mettre à l'échelle (×2 autour du centre du trait) : les distances et le rayon doublent.
    let centre = board::points_bounds(&apres).center();
    assert_eq!(a.transformer(&layout, &[premier], &Affine::scale_about(centre, 2.0, 2.0)), 1);
    let grand = a.points_planche_de(&layout, premier).unwrap();
    let d = |pts: &[VPoint]| (pts[0].pos() - pts[2].pos()).length();
    assert!((d(&grand) / d(&apres) - 2.0).abs() < 1e-3, "{} contre {}", d(&grand), d(&apres));
    assert!((grand[1].r / apres[1].r - 2.0).abs() < 1e-3, "l'épaisseur suit l'échelle");
    // Tourner de 90° : les distances ne changent pas, la direction du trait si.
    let centre = board::points_bounds(&grand).center();
    assert_eq!(a.transformer(&layout, &[premier], &Affine::rotate_about(centre, std::f32::consts::FRAC_PI_2)), 1);
    let tourne = a.points_planche_de(&layout, premier).unwrap();
    assert!((d(&tourne) - d(&grand)).abs() < 1e-2, "rotation : les distances restent");
    let (u, v) = (grand[2].pos() - grand[0].pos(), tourne[2].pos() - tourne[0].pos());
    assert!(u.dot(v).abs() < 1e-2 * u.length() * v.length(), "tourné à angle droit");
    assert!((tourne[1].r - grand[1].r).abs() < 1e-3, "la rotation ne change pas l'épaisseur");
    // Un trait emporté dans une autre case s'y rattache : il suit cette case quand la planche change.
    let hors = layout.cards[4].thumb.center() - board::points_bounds(&tourne).center();
    assert_eq!(a.transformer(&layout, &[premier], &Affine::translate(hors)), 1);
    let t = a.traces.iter().find(|t| t.id == premier).unwrap();
    assert_eq!(t.anchor.card, layout.cards[4].id, "attaché à la case où il est maintenant");
    let large = pale(&doc, &reglages(400.0, 3, 0));
    let sur_la_case = |l: &BoardLayout| (centre_du_trait(&a, l, 0) - l.cards[4].thumb.left_top()) / l.cards[4].thumb.width();
    assert!((sur_la_case(&layout) - sur_la_case(&large)).length() < 1e-3, "il suit sa nouvelle case");
    // Un trait inconnu, ou une transformation vide : rien.
    assert_eq!(a.transformer(&layout, &[987_654_321], &Affine::translate(vec2(1.0, 1.0))), 0);
}

#[test]
fn the_lasso_picks_the_strokes_of_one_layer_that_have_a_point_inside_and_a_click_picks_the_top_one() {
    let doc = quatre_plans();
    let layout = pale(&doc, &reglages(200.0, 3, 0));
    let mut a = Annotations::default();
    let c = layout.cards[2].thumb.center();
    a.ajouter(&layout, &trait_en(c, 2.0), COULEUR_PAR_DEFAUT);
    let bas = a.calques[0].id;
    let haut = a.ajouter_calque().unwrap();
    a.ajouter_dans(&layout, Some(haut), &trait_en(c, 2.0), COULEUR_PAR_DEFAUT);
    a.ajouter_dans(&layout, Some(haut), &trait_en(layout.cards[5].thumb.center(), 2.0), COULEUR_PAR_DEFAUT);
    let carre = |c: Pos2, r: f32| vec![c + vec2(-r, -r), c + vec2(r, -r), c + vec2(r, r), c + vec2(-r, r)];
    // Un lasso autour de la case 2 : un trait par calque, jamais ceux de l'autre.
    let (t_bas, t_haut) = (a.traces[0].id, a.traces[1].id);
    assert_eq!(a.dans_le_lasso(&layout, bas, &carre(c, 30.0)), vec![t_bas]);
    assert_eq!(a.dans_le_lasso(&layout, haut, &carre(c, 30.0)), vec![t_haut]);
    // Un lasso qui n'a aucun point du trait dedans (il entoure son milieu seulement, mais un point y est) / loin : rien.
    assert!(a.dans_le_lasso(&layout, haut, &carre(c + vec2(0.0, 90.0), 5.0)).is_empty());
    // Un seul point ne fait pas un lasso.
    assert!(a.dans_le_lasso(&layout, bas, &[c, c]).is_empty());
    // Un clic : le trait sous le point, le plus haut s'ils se superposent (le dernier de la liste), dans le calque visé seulement.
    assert_eq!(a.sous_le_point(&layout, haut, c, 3.0), Some(t_haut));
    assert_eq!(a.sous_le_point(&layout, bas, c, 3.0), Some(t_bas));
    assert_eq!(a.sous_le_point(&layout, bas, c + vec2(0.0, 90.0), 3.0), None);
    // La gomme d'un calque ne touche pas aux autres.
    assert_eq!(a.effacer_en_calque(&layout, bas, c, 3.0), 1);
    assert_eq!(a.traces.len(), 2);
    assert!(a.traces.iter().all(|t| t.calque == haut));
    // Les bornes de ce qui est choisi.
    assert!(a.bornes(&layout, &[t_haut]).unwrap().contains(c));
    assert!(a.bornes(&layout, &[]).is_none());
}
