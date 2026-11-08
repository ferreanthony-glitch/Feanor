//! Essais du dessin hors des cases avec la texture du pinceau (`horschamptex.rs`) et du passe-partout dans l'export de la planche
//! (Anthony, 2026-10-30) :
//! - le trait hors cadres garde la recette de sa brosse (réglage, papier, opacité, appuis) et la retrouve après enregistrer / rouvrir ;
//! - un fichier d'avant s'ouvre ; une recette abîmée laisse une ligne ;
//! - un trait à cheval sur une case et le hors-cadres a la même brosse des deux côtés ; Ctrl+Z l'enlève d'un coup ;
//! - le rendu est fait sur un fil de fond, gardé, et la planche ne le refait ni ne le renvoie à la carte graphique quand rien ne bouge ;
//! - l'export montre la texture ; le passe-partout de l'export a sa case, son opacité, son réglage retenu.

use std::time::{Duration, Instant};

use eframe::egui::{pos2, vec2, Pos2};
use image::{Rgb, RgbImage};

use crate::annotations::COULEUR_PAR_DEFAUT;
use crate::brush::BrushSettings;
use crate::brushtex::{BrushLibrary, GrayImage};
use crate::document::{Document, LayerKind};
use crate::horschamptex::Recette;
use crate::perf_tests::Banc;
use crate::planche::{self, Options};
use crate::vector::{Element, VPoint};
use crate::vignettes_tests::{glisser, ouvert, pas_d_annulation, quatre_plans, vague, vignette};

fn en_bitmap(mut doc: Document) -> Document {
    for p in &mut doc.panels {
        for l in &mut p.layers {
            l.kind = LayerKind::Bitmap;
        }
    }
    doc
}

/// Un papier à grain marqué (du bruit net) : un pinceau qui s'y appuie laisse un trait troué, reconnaissable.
fn papier() -> GrayImage {
    let mut x: u32 = 0x1234_5678;
    GrayImage::new(32, 32, (0..32 * 32).map(|_| {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        (x % 256) as f32 / 255.0
    }).collect())
}

/// Le pinceau de l'application, posé sur ce papier à grain fort.
fn texturer(b: &mut Banc) {
    let id = b.app.brushes.add_paper("Grain d'essai", papier());
    b.app.brush.paper = Some(id);
    b.app.brush.grain = 1.0;
    b.app.brush.size = 14.0;
}

fn hors_cadres(b: &Banc) -> Vec<usize> {
    let a = &b.app.doc.annotations;
    (0..a.traces.len()).filter(|&i| a.calque(a.traces[i].calque).is_some_and(|c| c.planche)).collect()
}

/// Un coin du papier, loin de toute case (le même choix que les essais d'avant).
fn coin(b: &Banc) -> (Pos2, Pos2) {
    let a = pos2(b.app.vignettes.view.left() + 6.0, b.app.vignettes.view.top() + 6.0);
    (a, a + vec2(80.0, 40.0))
}

/// Attend que le fil de fond ait fini le rendu du trait `i` de la planche (des images d'écran passent : c'est l'interface qui le demande).
fn attendre_le_rendu(b: &mut Banc, i: usize) {
    let debut = Instant::now();
    loop {
        b.chauffer(1);
        let t = &b.app.doc.annotations.traces[i];
        if t.brosse.as_ref().is_some_and(|br| br.etat() == "prêt") {
            return;
        }
        assert!(debut.elapsed() < Duration::from_secs(30), "le rendu n'est jamais arrivé ({:?})", t.brosse.as_ref().map(|br| br.etat()));
        std::thread::sleep(Duration::from_millis(4));
    }
}

fn dossier(nom: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-hc-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier d'essai");
    dir
}

// ------------------------------------------------------------------------------ la recette du pinceau

#[test]
fn an_outside_stroke_keeps_its_brush_and_a_texture_not_a_plain_line() {
    for bitmap in [false, true] {
        let doc = if bitmap { en_bitmap(quatre_plans()) } else { quatre_plans() };
        let mut b = ouvert(doc, 4);
        texturer(&mut b);
        b.app.color = [30, 60, 160];
        let (de, a) = coin(&b);
        let n = pas_d_annulation(&b);
        glisser(&mut b, &vague(de, a, 14));
        if bitmap {
            // Partie 127 : un trait bitmap garde hors des cases **ses pixels mêmes** (couche bitmap de la planche), pas une recette rejouée : le grain du
            // papier y est tel que le pinceau l'a posé.
            assert!(hors_cadres(&b).is_empty(), "plus de trait vectoriel pour un trait bitmap");
            let m = b.app.doc.annotations.bitmap.morceaux.first().expect("un morceau sur la planche");
            let mut tons = std::collections::BTreeSet::new();
            crate::planchebitmap::chaque_pixel(&m.pixels, |_, _, c| {
                tons.insert(c.a());
            });
            assert!(tons.len() >= 8, "le trait a la texture du papier ({} tons)", tons.len());
            assert_eq!(pas_d_annulation(&b), n + 1);
            b.app.run_action("edition.annuler");
            b.chauffer(2);
            assert!(b.app.doc.annotations.bitmap.morceaux.is_empty(), "Ctrl+Z enlève tout le trait d'un coup");
            b.app.run_action("edition.retablir");
            b.chauffer(2);
            assert_eq!(b.app.doc.annotations.bitmap.morceaux.len(), 1, "rétablir le rend");
            continue;
        }
        let tous = hors_cadres(&b);
        assert_eq!(tous.len(), 1, "bitmap {bitmap}");
        let t = &b.app.doc.annotations.traces[tous[0]];
        let br = t.brosse.as_ref().unwrap_or_else(|| panic!("bitmap {bitmap} : le trait hors cadres porte la recette de son pinceau"));
        let reglage = br.recette.reglage.expect("réglage complet");
        assert!(reglage.paper.is_some_and(|k| k & 0x8000_0000 != 0), "bitmap {bitmap} : le papier est désigné par sa clé de contenu : {:?}", reglage.paper);
        assert_eq!(reglage.grain, 1.0);
        assert_eq!((reglage.size, br.recette.opacite), (14.0, b.app.brush.opacity), "bitmap {bitmap} : même taille, même opacité");
        assert_eq!(t.color, [30, 60, 160]);
        assert_eq!(br.appui.len(), t.points.len(), "bitmap {bitmap} : un appui par point");
        assert_eq!(br.instants.len(), t.points.len());
        assert!(br.largeur_px > 10.0, "la largeur de la case en pixels du dessin : {}", br.largeur_px);
        // Le rendu : de l'encre, de la bonne couleur, et un grain (beaucoup de tons : pas un aplat).
        let py = br.pyramide_ou_calcul(&t.points, t.color).unwrap_or_else(|| panic!("bitmap {bitmap} : rendu"));
        assert!(py.alpha_total() > 0);
        let mut tons = std::collections::BTreeSet::new();
        for p in py.niveaux[0].blocs.iter().flat_map(|bl| &bl.pixels).filter(|p| p.a() > 0) {
            tons.insert(p.a());
        }
        assert!(tons.len() >= 8, "bitmap {bitmap} : le trait a la texture du papier ({} tons)", tons.len());
        // Une seule étape d'annulation, qui emporte le trait.
        assert_eq!(pas_d_annulation(&b), n + 1, "bitmap {bitmap}");
        b.app.run_action("edition.annuler");
        b.chauffer(2);
        assert!(hors_cadres(&b).is_empty(), "bitmap {bitmap} : Ctrl+Z enlève tout le trait d'un coup");
        assert_eq!(pas_d_annulation(&b), n);
        b.app.run_action("edition.retablir");
        b.chauffer(2);
        assert_eq!(hors_cadres(&b).len(), 1, "bitmap {bitmap} : rétablir le rend, avec sa recette");
        assert!(b.app.doc.annotations.traces[hors_cadres(&b)[0]].brosse.is_some());
    }
}

#[test]
fn a_stroke_across_a_case_edge_has_the_same_brush_on_both_sides() {
    for bitmap in [false, true] {
        let doc = if bitmap { en_bitmap(quatre_plans()) } else { quatre_plans() };
        let mut b = ouvert(doc, 4);
        texturer(&mut b);
        b.app.color = [200, 40, 40];
        let v = vignette(&b, 0);
        let loin = pos2(b.app.vignettes.view.left() + 4.0, v.center().y);
        glisser(&mut b, &vague(v.center(), loin, 18));
        if bitmap {
            // Partie 127 : des deux côtés, les pixels du même pinceau (dans le plan, et sur la couche bitmap de la planche).
            assert!(crate::vignettes_tests::encre(&b.app.doc, 0), "la part dans le champ reste dans le plan");
            assert!(!b.app.doc.annotations.bitmap.morceaux.is_empty(), "une part sur la planche, en pixels");
            continue;
        }
        let tous = hors_cadres(&b);
        assert!(!tous.is_empty(), "bitmap {bitmap} : une part sur la planche");
        let t = &b.app.doc.annotations.traces[tous[0]];
        let br = t.brosse.as_ref().expect("recette");
        if !bitmap {
            // Côté plan : le trait du dessin vectoriel porte le même réglage, la même opacité, le même toucher.
            let cel = b.app.doc.panels[0].layers.iter().flat_map(|l| l.frames.iter().flatten()).find_map(|id| b.app.doc.cels.get(id).filter(|c| c.vector.as_ref().is_some_and(|v| !v.elements.is_empty())))
                .expect("un dessin vectoriel avec le trait");
            let Some(Element::Stroke { brush, opacity, hardness, trace, color, .. }) = cel.vector.as_ref().and_then(|v| v.elements.last()).cloned() else { panic!("un trait") };
            assert_eq!(brush.as_deref().copied(), br.recette.reglage, "même brosse, même papier, même taille des deux côtés");
            assert_eq!((opacity, hardness, trace.feel, trace.secondary), (br.recette.opacite, br.recette.durete, br.recette.toucher, br.recette.couleur_b));
            assert_eq!(color, t.color);
            assert!(trace.pressure.len() > 3 && !br.appui.is_empty(), "les appuis sont gardés des deux côtés");
        } else {
            // Côté plan : les pixels du dessin ; côté planche : la recette du moteur, avec le papier choisi.
            assert!(br.recette.reglage.is_some_and(|r| r.paper.is_some() && r.grain == 1.0 && r.size == 14.0));
            assert!(crate::vignettes_tests::encre(&b.app.doc, 0), "la part dans le champ reste dans le plan");
        }
        // La taille de la ligne est continue d'un côté à l'autre : le rayon du premier point hors cadres (en pixels du dessin) est celui du pinceau.
        let r_px = t.points[0].r * br.largeur_px;
        assert!((r_px - 7.0).abs() < 0.8, "bitmap {bitmap} : rayon {r_px} (pinceau de 14)");
    }
}

#[test]
fn the_recipe_is_saved_and_reread_with_the_project_and_its_textures() {
    let mut b = ouvert(quatre_plans(), 4);
    texturer(&mut b);
    let (de, a) = coin(&b);
    glisser(&mut b, &vague(de, a, 14));
    let i = hors_cadres(&b)[0];
    let avant = b.app.doc.annotations.traces[i].clone();
    let dir = dossier("rouvrir");
    let manifeste = dir.join("P.shot");
    crate::project::save(&b.app.doc, &manifeste, &mut Default::default()).expect("enregistrement");
    let texte = std::fs::read_to_string(dir.join(crate::annotations::FILE)).expect("annotations.json");
    assert!(texte.contains("\"pinceau\"") && texte.contains("\"version\": 2"), "la recette est dans le fichier, la version du fichier ne change pas : {}", &texte[..texte.len().min(400)]);
    assert!(std::fs::read_dir(dir.join("textures")).expect("dossier des textures").flatten().any(|e| e.file_name().to_string_lossy().contains("papier") || e.file_name().to_string_lossy().contains("paper")), "le papier du pinceau est écrit avec le projet");
    let relu = crate::project::load(&manifeste).expect("ouverture");
    assert!(relu.warnings.iter().all(|w| !w.contains("annotations")), "{:?}", relu.warnings);
    let apres = relu.doc.annotations.traces.iter().find(|t| relu.doc.annotations.calque(t.calque).is_some_and(|c| c.planche)).expect("le trait hors cadres est revenu");
    let (x, y) = (avant.brosse.as_ref().expect("recette"), apres.brosse.as_ref().expect("la recette est revenue avec le trait"));
    assert_eq!(x.recette.reglage, y.recette.reglage, "même brosse, même papier");
    assert_eq!((x.recette.opacite, x.recette.durete, x.recette.toucher, x.recette.couleur_b), (y.recette.opacite, y.recette.durete, y.recette.toucher, y.recette.couleur_b));
    assert!((x.largeur_px - y.largeur_px).abs() < 1e-3 && (x.taille - y.taille).abs() < 1e-6);
    assert_eq!((x.appui.len(), x.instants.len()), (y.appui.len(), y.instants.len()));
    assert!(x.appui.iter().zip(&y.appui).all(|(p, q)| (p - q).abs() < 1e-3), "les appuis, à l'arrondi du fichier près");
    assert_eq!((apres.color, apres.alpha, apres.anchor), (avant.color, avant.alpha, avant.anchor));
    // Et rendu : de l'encre, au papier près.
    let py = y.pyramide_ou_calcul(&apres.points, apres.color).expect("rendu après réouverture");
    assert!(py.alpha_total() > 0);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_old_file_opens_a_broken_recipe_leaves_a_line_and_an_unknown_key_is_ignored() {
    // Un fichier d'avant : pas de clé « pinceau ».
    let vieux = r#"{"format":"shot-annotations","version":2,"calques":[{"id":1,"nom":"Hors cadres","visible":true,"planche":true,"traces":[{"ancre":{"vignette":1,"apres":0,"rang":0},"couleur":[10,20,30],"alpha":128,"points":[[0.1,0.1,0.01],[0.3,0.2,0.01]]}]}]}"#;
    let a = crate::annotations::parse(vieux).expect("un fichier d'avant s'ouvre");
    assert_eq!(a.traces.len(), 1);
    assert!(a.traces[0].brosse.is_none(), "une ligne vectorielle, comme avant");
    assert_eq!((a.traces[0].alpha, a.traces[0].color), (128, [10, 20, 30]));
    // Une recette absurde (largeur nulle) : le trait reste, en ligne.
    let casse = vieux.replace(r#""points":[[0.1,0.1,0.01],[0.3,0.2,0.01]]"#, r#""points":[[0.1,0.1,0.01],[0.3,0.2,0.01]],"pinceau":{"largeur":0.0,"opacite":1.0}"#);
    let b = crate::annotations::parse(&casse).expect("lisible");
    assert_eq!(b.traces.len(), 1);
    assert!(b.traces[0].brosse.is_none(), "recette absurde écartée, le trait reste");
    // Une recette bonne, avec une clé inconnue de plus.
    let bon = vieux.replace(r#""points":[[0.1,0.1,0.01],[0.3,0.2,0.01]]"#, r#""points":[[0.1,0.1,0.01],[0.3,0.2,0.01]],"pinceau":{"largeur":1920.0,"opacite":0.5,"durete":0.8,"inconnue":3},"autre":1"#);
    let c = crate::annotations::parse(&bon).expect("lisible");
    let br = c.traces[0].brosse.as_ref().expect("recette lue");
    assert_eq!((br.largeur_px, br.recette.opacite, br.recette.durete), (1920.0, 0.5, 0.8));
    assert!(br.recette.reglage.is_none(), "sans réglage : trait lisse");
    // Une trace dont l'appui ne suit pas les points : recette gardée, sans appuis (ils se retrouvent d'après les rayons).
    let decale = vieux.replace(r#""points":[[0.1,0.1,0.01],[0.3,0.2,0.01]]"#, r#""points":[[0.1,0.1,0.01],[0.3,0.2,0.01]],"pinceau":{"largeur":100.0,"trace":{"appui":[65535,65535,65535],"instants":[0,1,2]}}"#);
    let d = crate::annotations::parse(&decale).expect("lisible");
    assert!(d.traces[0].brosse.as_ref().is_some_and(|b| b.appui.is_empty()));
}

/// Un trait au pinceau posé hors des cases, attaché à la case `k` (le plus près du début de sa case), avec une recette à papier.
fn poser_pres_de(b: &mut Banc, k: usize) -> usize {
    let layout = b.app.board_layout();
    let case = layout.cards[k].thumb;
    let mut lib = BrushLibrary::builtin();
    let id = lib.add_paper("Grain", papier());
    let reglage = lib.for_vector(&BrushSettings { paper: Some(id), grain: 1.0, size: 10.0, ..BrushSettings::brush() });
    let recette = Recette { reglage, opacite: 1.0, durete: 1.0, toucher: None, couleur_b: None };
    let a = case.width() / 1920.0;
    let pts: Vec<VPoint> = (0..12).map(|j| VPoint { x: case.left() - 12.0 + j as f32 * 3.0, y: case.center().y + (j as f32).sin() * 3.0, r: 5.0 * a }).collect();
    assert!(b.app.doc.annotations.ajouter_hors_cadres_brosse(&layout, &pts, COULEUR_PAR_DEFAUT, 255, Some((recette, Vec::new(), Vec::new(), a))));
    b.app.doc.annotations.traces.len() - 1
}

/// Le coin haut gauche du rendu du trait `i`, relativement à sa case, sur la planche.
fn coin_du_rendu(b: &Banc, i: usize) -> Option<(f32, f32)> {
    let l = b.app.board_layout();
    let t = &b.app.doc.annotations.traces[i];
    let (_, case) = crate::annotations::case_de(&l, &t.anchor)?;
    let br = t.brosse.as_ref()?;
    let py = br.pyramide(&t.points)?;
    let r = py.boite()?;
    let k = case.width() / br.largeur_px;
    Some((r.min.x * k, r.min.y * k))
}

#[test]
fn a_textured_stroke_follows_its_case_when_sequences_are_reordered_and_waits_when_hidden_without_being_rendered_again() {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.add_scene_after(0);
    let b0 = doc.add_sequence_after(1);
    doc.add_scene_after(b0);
    doc.add_sequence_after(3);
    let mut b = ouvert(doc, 6);
    let seqs = b.app.doc.sequence_order();
    assert_eq!(seqs.len(), 3);
    let k = b.app.board_layout().cards.iter().position(|c| c.panel == 2).unwrap();
    let i = poser_pres_de(&mut b, k);
    attendre_le_rendu(&mut b, i);
    let avant = coin_du_rendu(&b, i).expect("rendu prêt");
    let replays = crate::vector::replay_count();
    // Les séquences réordonnées : le trait suit sa case, au même endroit dans sa case, sans être rendu de nouveau.
    b.app.affichage_deplacer(seqs[2], 0);
    b.chauffer(4);
    let apres = coin_du_rendu(&b, i).expect("toujours prêt : aucun rendu refait");
    assert!((avant.0 - apres.0).abs() < 0.01 && (avant.1 - apres.1).abs() < 0.01, "même place dans sa case : {avant:?} {apres:?}");
    assert_eq!(b.app.doc.annotations.traces[i].brosse.as_ref().unwrap().etat(), "prêt");
    // Sa séquence cachée : pas peint, ni perdu ni passé à une voisine ; remontrée, il est où il était.
    b.app.affichage_basculer(seqs[1]);
    b.chauffer(3);
    assert_eq!(b.app.doc.annotations.traces.len(), 1);
    assert_eq!(b.app.doc.annotations.traces[i].anchor.card, b.app.doc.panels[2].id);
    assert!(b.app.doc.annotations.traces[i].brosse.is_some());
    b.app.affichage_basculer(seqs[1]);
    b.chauffer(6);
    let revenu = coin_du_rendu(&b, i).expect("prêt");
    assert!((avant.0 - revenu.0).abs() < 0.01 && (avant.1 - revenu.1).abs() < 0.01);
    assert_eq!(crate::vector::replay_count(), replays, "rien n'a été rejoué sur le fil de l'interface");
}

#[test]
fn moving_or_scaling_a_textured_stroke_scales_its_brush_and_asks_for_a_new_render() {
    let mut b = ouvert(quatre_plans(), 4);
    let i = poser_pres_de(&mut b, 1);
    attendre_le_rendu(&mut b, i);
    let layout = b.app.board_layout();
    let id = b.app.doc.annotations.traces[i].id;
    let (avant, p0) = (b.app.doc.annotations.traces[i].brosse.clone().unwrap(), b.app.doc.annotations.traces[i].points[0]);
    let c = crate::annotations::case_de(&layout, &b.app.doc.annotations.traces[i].anchor).unwrap().1;
    let aff = crate::vector::Affine::scale_about(c.center(), 2.0, 2.0);
    assert_eq!(b.app.doc.annotations.transformer(&layout, &[id], &aff), 1);
    let t = &b.app.doc.annotations.traces[i];
    let apres = t.brosse.as_ref().expect("la recette suit le trait");
    assert_eq!(apres.recette, avant.recette, "même brosse");
    assert!((apres.taille - 2.0).abs() < 1e-4, "la taille du pinceau suit l'échelle : {}", apres.taille);
    assert_eq!(apres.etat(), "vide", "un rendu neuf, à la demande");
    assert_eq!(avant.etat(), "prêt", "l'ancien rendu reste à qui le garde (l'annulation)");
    assert!(t.points[0].r > p0.r * 1.9, "le trait est plus épais");
    // Le rendu neuf se fait, sur le fil de fond, avec la taille doublée.
    attendre_le_rendu(&mut b, i);
    let t = &b.app.doc.annotations.traces[i];
    let neuf = t.brosse.as_ref().unwrap().pyramide(&t.points).expect("rendu neuf");
    assert!(neuf.alpha_total() > 0);
}

// ------------------------------------------------------------------------------ sans latence

#[test]
fn the_texture_is_rendered_in_the_background_kept_and_not_sent_again_while_nothing_moves() {
    let mut b = ouvert(quatre_plans(), 4);
    texturer(&mut b);
    let (de, a) = coin(&b);
    let replays = crate::vector::replay_count();
    glisser(&mut b, &vague(de, a, 14));
    let i = hors_cadres(&b)[0];
    assert_eq!(crate::vector::replay_count(), replays, "le fil de l'interface n'a rejoué aucun trait : ni dans le plan, ni sur la planche");
    attendre_le_rendu(&mut b, i);
    assert_eq!(crate::vector::replay_count(), replays, "le rendu est venu du fil de fond");
    b.chauffer(4);
    let (envoye, nombre) = {
        let t = b.app.vignettes.hc_tex.borrow();
        (t.total_envoye, t.nombre())
    };
    assert!(envoye > 0 && nombre == 1, "la texture est à la carte graphique : {envoye} octets, {nombre} trait(s)");
    // Au repos : rien n'est renvoyé, rien n'est refait.
    b.chauffer(40);
    assert_eq!(b.app.vignettes.hc_tex.borrow().total_envoye, envoye, "rien n'est renvoyé à la carte graphique au repos");
    assert_eq!(crate::vector::replay_count(), replays);
    assert_eq!(b.app.doc.annotations.traces[i].brosse.as_ref().unwrap().etat(), "prêt");
    // Un trait dans une case ne touche pas à la texture déjà là.
    let v = vignette(&b, 1);
    glisser(&mut b, &vague(v.center() - vec2(20.0, 0.0), v.center() + vec2(20.0, 0.0), 8));
    b.chauffer(10);
    assert_eq!(b.app.vignettes.hc_tex.borrow().total_envoye, envoye);
    // La planche n'a plus rien à montrer : les textures sont rendues.
    b.app.run_action("edition.annuler");
    b.app.run_action("edition.annuler");
    b.chauffer(3);
    assert!(hors_cadres(&b).is_empty());
}

#[test]
fn many_textured_strokes_stay_fluid_to_scroll_and_zoom_and_each_is_rendered_once() {
    let mut doc = Document::with_frame(1920, 1080, 24);
    doc.add_panel_after(0);
    doc.add_panel_after(1);
    doc.add_scene_after(2);
    doc.into_plans();
    let mut b = ouvert(doc, 4);
    texturer(&mut b);
    b.app.brush.size = 40.0;
    // Trois cents traits au pinceau (déjà rendus par le moteur à leur taille réelle), semés autour des cases visibles.
    let layout = b.app.board_layout();
    let mut lib = BrushLibrary::builtin();
    let id = lib.add_paper("Grain", papier());
    let reglage = lib.for_vector(&BrushSettings { paper: Some(id), grain: 1.0, size: 40.0, ..BrushSettings::brush() });
    let recette = Recette { reglage, opacite: 1.0, durete: 1.0, toucher: None, couleur_b: None };
    let n = 300;
    for k in 0..n {
        let case = layout.cards[k % layout.cards.len()].thumb;
        let (x0, y0) = (case.left() - 30.0 + (k % 7) as f32 * 9.0, case.top() - 20.0 + (k / 7 % 9) as f32 * 11.0);
        let pts: Vec<VPoint> = (0..24).map(|j| VPoint { x: x0 + j as f32 * 2.5, y: y0 + (j as f32 * 0.5).sin() * 4.0, r: 20.0 * case.width() / 1920.0 }).collect();
        assert!(b.app.doc.annotations.ajouter_hors_cadres_brosse(&layout, &pts, COULEUR_PAR_DEFAUT, 255, Some((recette, Vec::new(), Vec::new(), case.width() / 1920.0))));
    }
    assert_eq!(b.app.doc.annotations.traces.len(), n);
    // Le temps d'une image avant que les rendus soient là (lignes vectorielles de secours).
    let mesurer = |b: &mut Banc, images: usize| -> f64 {
        let t: f64 = (0..images).map(|_| b.image(Vec::new(), 1.0 / 60.0).construction).sum();
        t / images as f64
    };
    b.chauffer(3);
    let _ = mesurer(&mut b, 3);
    // Tous les rendus finissent, sur le fil de fond.
    let debut = Instant::now();
    loop {
        b.chauffer(2);
        let prets = b.app.doc.annotations.traces.iter().filter(|t| t.brosse.as_ref().is_some_and(|br| br.etat() == "prêt")).count();
        if prets == n {
            break;
        }
        assert!(debut.elapsed() < Duration::from_secs(240), "{prets} rendus sur {n}");
        std::thread::sleep(Duration::from_millis(10));
    }
    let rendu_en = debut.elapsed();
    // Toutes les textures montent à la carte graphique, quelques-unes par image d'écran.
    b.chauffer(60);
    let (envoye, nombre, garde) = {
        let t = b.app.vignettes.hc_tex.borrow();
        (t.total_envoye, t.nombre(), t.octets())
    };
    assert!(nombre > 0, "des textures à l'écran");
    // Au repos : l'image la plus lente (sur 30), pour le défilement ensuite.
    let repos = mesurer(&mut b, 30);
    assert_eq!(b.app.vignettes.hc_tex.borrow().total_envoye, envoye, "rien de renvoyé au repos");
    // Défiler et zoomer sur place (même niveau de la pyramide : rien n'est renvoyé, rien n'est rejoué).
    let replays = crate::vector::replay_count();
    let mut pire: f64 = 0.0;
    for k in 0..40 {
        let dx = if k % 2 == 0 { 3.0 } else { -3.0 };
        b.app.board_pan += vec2(dx, 0.0);
        pire = pire.max(b.image(Vec::new(), 1.0 / 60.0).construction);
    }
    assert_eq!(crate::vector::replay_count(), replays, "défiler ne rejoue rien sur le fil de l'interface");
    eprintln!("MESURE hors-cadres : {n} traits au pinceau (papier, 40 px) : tous rendus en {:.1} s sur le fil de fond ; image au repos {repos:.2} ms, image la plus lente en défilant {pire:.2} ms ; {nombre} traits à l'écran, {} Ko envoyés à la carte graphique en tout, {} Ko gardés", rendu_en.as_secs_f32(), envoye / 1024, garde / 1024);
    assert!(repos < 40.0, "image au repos : {repos} ms");
    assert!(pire < 120.0, "défilement : {pire} ms");
}

/// Prépare une copie d'un vrai projet pour une capture à l'écran (`SHOT_HC_COPIE` : le fichier du projet copié ; `SHOT_HC_PNG` : où écrire la
/// planche exportée avec passe-partout) : des traits au pinceau de la bibliothèque, l'un qui sort d'une case, d'autres hors des cases.
/// Lancé à la main : `cargo test --release --offline --target-dir target\msvc prepare_une_copie -- --ignored --nocapture`.
#[test]
#[ignore]
fn prepare_une_copie_pour_une_capture() {
    let Ok(chemin) = std::env::var("SHOT_HC_COPIE") else { return };
    let manifest = std::path::PathBuf::from(chemin);
    let charge = crate::project::load(&manifest).expect("le projet s'ouvre");
    let (doc, mut written) = (charge.doc, charge.written);
    let mut b = ouvert(doc, 4);
    // Un vrai pinceau texturé de la bibliothèque (le premier à pointe ou papier), gros, en bleu.
    let lib = BrushLibrary::builtin();
    let p = lib.presets.iter().find(|p| p.target == crate::brushtex::Target::Brush && (p.settings.tip.is_some() || p.settings.paper.is_some()) && p.settings.grain > 0.0).or_else(|| lib.presets.iter().find(|p| p.settings.tip.is_some() || p.settings.paper.is_some())).expect("un pinceau texturé");
    println!("pinceau : {}", p.name);
    b.app.brush = p.settings;
    b.app.brush.size = 46.0;
    b.app.brush.opacity = 0.9;
    b.app.color = [24, 52, 140];
    let v = vignette(&b, 0);
    let vue = b.app.vignettes.view;
    // Un trait qui part de la case 1 et file vers la gauche, hors de la case ; un autre en l'air dans le coin ; un troisième entre deux cases.
    glisser(&mut b, &vague(v.center() + vec2(40.0, 0.0), pos2(vue.left() + 8.0, v.center().y + 30.0), 28));
    b.app.color = [150, 40, 40];
    glisser(&mut b, &vague(pos2(vue.left() + 10.0, vue.top() + 12.0), pos2(vue.left() + 190.0, vue.top() + 40.0), 22));
    let (v1, v2) = (vignette(&b, 1), vignette(&b, 2));
    b.app.color = [20, 110, 70];
    glisser(&mut b, &vague(v1.center() + vec2(20.0, 10.0), v2.center() - vec2(20.0, -10.0), 26));
    for i in hors_cadres(&b) {
        attendre_le_rendu(&mut b, i);
    }
    println!("traits hors cadres : {}", hors_cadres(&b).len());
    crate::project::save(&b.app.doc, &manifest, &mut written).expect("enregistrement");
    // La planche exportée, passe-partout coché à 70 % (celui de l'écran), pour la capture.
    if let Ok(sortie) = std::env::var("SHOT_HC_PNG") {
        b.app.settings.vignettes.passe_partout_cases_opacite = 70;
        let tous: Vec<usize> = (0..b.app.doc.panels.len()).collect();
        let opt = Options { marie: true, marie_couleur: [250, 244, 230], ..Options::default() };
        let job = b.app.planche_job(&tous, &opt, std::path::Path::new(&sortie)).expect("tâche");
        lancer(&mut b, job);
        let fin = b.app.export_job.as_ref().and_then(|j| j.outcome.clone()).expect("fini");
        println!("planche exportée : {fin:?}");
        // Le même export sans aucune annotation ni trait hors cadres (rien de cette branche ne peut y jouer) et sans passe-partout.
        if let Ok(sans) = std::env::var("SHOT_HC_PNG_SANS") {
            b.app.export_job = None;
            b.app.doc.annotations = Default::default();
            let job = b.app.planche_job(&tous, &Options::default(), std::path::Path::new(&sans)).expect("tâche");
            lancer(&mut b, job);
        }
    }
}

fn lancer(b: &mut Banc, job: crate::export::ExportJob) {
    b.app.start_job(job);
    let debut = Instant::now();
    while b.app.export_job.as_ref().is_some_and(|j| j.outcome.is_none()) && debut.elapsed() < Duration::from_secs(120) {
        b.image(Vec::new(), 1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(2));
    }
    b.chauffer(2);
}

#[test]
fn the_export_job_writes_the_textured_stroke_and_the_passe_partout_at_the_screen_opacity() {
    let dir = dossier("job-texture");
    let mut b = ouvert(quatre_plans(), 4);
    texturer(&mut b);
    b.app.brush.size = 22.0;
    b.app.color = [30, 60, 200];
    let (de, a) = coin(&b);
    glisser(&mut b, &vague(de, a, 14));
    let i = hors_cadres(&b)[0];
    // (Le rendu du trait vient du fil de fond ; l'export le prend prêt, ou le calcule sur son fil.)
    attendre_le_rendu(&mut b, i);
    let tous: Vec<usize> = (0..b.app.doc.panels.len()).collect();
    b.app.settings.vignettes.passe_partout_cases_opacite = 100;
    let sans = dir.join("sans.png");
    let job = b.app.planche_job(&tous, &Options::default(), &sans).expect("tâche");
    lancer(&mut b, job);
    b.app.export_job = None;
    // Case cochée, opacité de l'écran à 100 % : les marges au noir ; le trait (hors des cases) disparaît dessous.
    let plein = dir.join("plein.png");
    let job = b.app.planche_job(&tous, &Options { marie: true, ..Options::default() }, &plein).expect("tâche");
    lancer(&mut b, job);
    b.app.export_job = None;
    b.app.settings.vignettes.passe_partout_cases_opacite = 50;
    let demi = dir.join("demi.png");
    let job = b.app.planche_job(&tous, &Options { marie: true, ..Options::default() }, &demi).expect("tâche");
    lancer(&mut b, job);
    let (sans, plein, demi) = (image::open(&sans).expect("PNG").to_rgb8(), image::open(&plein).expect("PNG").to_rgb8(), image::open(&demi).expect("PNG").to_rgb8());
    let (encre, tons) = encre_de(&sans);
    assert!(encre > 100 && tons.len() >= 8, "le fichier exporté montre la texture : {encre} pixels, {} tons", tons.len());
    assert_eq!(plein.get_pixel(1, 1).0, [0, 0, 0], "passe-partout à 100 % : noir");
    assert_eq!(encre_de(&plein).0, 0, "le trait hors des cases est couvert");
    // (Un coin de page sans trait dessous : le blanc du papier sous un noir à moitié.)
    let m = demi.get_pixel(demi.width() - 2, demi.height() - 2).0;
    assert!(m[0] > 100 && m[0] < 160 && m[0] == m[1], "à 50 % : {m:?}");
    assert_eq!(plein.get_pixel(plein.width() - 2, plein.height() - 2).0, [0, 0, 0]);
    assert!(encre_de(&demi).0 > 0, "à 50 %, le trait se voit encore à travers le passe-partout");
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------ l'export

fn planche_de(b: &Banc) -> planche::Planche {
    let tous: Vec<usize> = (0..b.app.doc.panels.len()).collect();
    planche::construire(&b.app.doc, &tous, &b.app.layout.board_settings(), &Options::default(), &b.app.doc.annotations)
}

fn vignettes(p: &planche::Planche, s: f32) -> Vec<RgbImage> {
    (0..p.cartes.len())
        .map(|k| {
            let [w, h] = planche::taille_vignette(p, k, s);
            RgbImage::from_pixel(w, h, Rgb([255, 255, 255]))
        })
        .collect()
}

fn encre_de(img: &RgbImage) -> (usize, std::collections::BTreeSet<u8>) {
    let mut tons = std::collections::BTreeSet::new();
    let mut n = 0;
    for p in img.pixels().filter(|p| p.0 != [255, 255, 255] && p.0 != [235, 235, 235]) {
        // (Le filet gris des cases est écarté : seul le bleu du trait compte.)
        if p.0[2] > p.0[0] + 20 {
            n += 1;
            tons.insert(p.0[0]);
        }
    }
    (n, tons)
}

#[test]
fn the_exported_board_shows_the_brush_texture_not_a_plain_line() {
    let mut b = ouvert(quatre_plans(), 4);
    texturer(&mut b);
    b.app.brush.size = 22.0;
    b.app.color = [30, 60, 200];
    let (de, a) = coin(&b);
    glisser(&mut b, &vague(de, a, 14));
    assert_eq!(hors_cadres(&b).len(), 1);
    let p = planche_de(&b);
    assert_eq!(p.annotations.traces.len(), 1, "le dessin hors des cases sort même sans l'option des annotations");
    let s = 2.0;
    let opt = Options::default();
    let img = planche::composer(&p, &vignettes(&p, s), &opt, s).expect("image");
    let (encre, tons) = encre_de(&img);
    assert!(encre > 200, "le trait est dans l'image : {encre} pixels");
    assert!(tons.len() >= 8, "avec la texture du papier : {} tons", tons.len());
    // Le même trait sans sa recette (la ligne vectorielle d'avant) : un aplat de quelques tons seulement.
    let mut ligne = p.clone();
    for t in &mut ligne.annotations.traces {
        t.brosse = None;
    }
    let img2 = planche::composer(&ligne, &vignettes(&ligne, s), &opt, s).expect("image");
    let (_, tons2) = encre_de(&img2);
    assert!(tons.len() > tons2.len() + 4, "la texture se voit à l'export ({} tons contre {} pour la ligne)", tons.len(), tons2.len());
    // Une autre résolution : le trait suit (pas de pavé décalé).
    let (s3, opt3) = (1.0, Options::default());
    let img3 = planche::composer(&p, &vignettes(&p, s3), &opt3, s3).expect("image");
    let (n3, _) = encre_de(&img3);
    assert!(n3 > 50 && (n3 as f32) < encre as f32 * 0.6, "à demi-échelle : {n3} pixels contre {encre}");
    // Écrit en PNG, lisible.
    let png = planche::png(&img).expect("png");
    assert!(png.len() > 1000);
}

#[test]
fn the_export_has_its_own_passe_partout_box_with_the_screen_opacity_and_a_remembered_choice() {
    let mut b = ouvert(quatre_plans(), 4);
    // Par défaut : décochée ; l'interrupteur de l'écran n'y change rien.
    assert!(!Options::default().marie && !b.app.settings.vignettes.export_passe_partout);
    b.app.run_action("vignettes.marie_louise");
    assert!(b.app.vignettes.annot.marie && !b.app.settings.vignettes.export_passe_partout, "le bouton de l'écran ne coche pas la case de l'export");
    b.app.run_action("vignettes.marie_louise");
    // Le choix est retenu dans les réglages de l'application, pas dans le projet.
    b.app.settings.vignettes.export_passe_partout = true;
    let t = toml::to_string(&b.app.settings.vignettes).expect("réglages");
    assert!(t.contains("export_passe_partout = true"), "{t}");
    let relu: crate::vignettesreglages::ReglagesVignettes = toml::from_str(&t).expect("relus");
    assert!(relu.export_passe_partout);
    let ancien: crate::vignettesreglages::ReglagesVignettes = toml::from_str("debord_pour_cent = 10").expect("anciens réglages");
    assert!(!ancien.export_passe_partout, "un réglage d'avant : décoché");
    // L'export prend la case (pas le bouton de l'écran), et l'opacité de l'écran.
    b.app.settings.vignettes.passe_partout_cases_opacite = 40;
    let (opt, _) = b.app.planche_options_de_depart();
    assert!(opt.marie, "la case cochée dans la fenêtre");
    assert_eq!(b.app.planche_options(&opt).marie_opacite, 40, "l'opacité réglée à l'écran");
    b.app.settings.vignettes.export_passe_partout = false;
    b.app.vignettes.annot.marie = true;
    assert!(!b.app.planche_options_de_depart().0.marie, "le bouton de l'écran allumé ne suffit pas : la case de l'export décide");
    // Avec et sans : le passe-partout peint (à 100 % : noir plein ; à 40 % : gris), sans lui la table grise.
    let p = planche_de(&b);
    let s = 1.0;
    let v = vignettes(&p, s);
    let sans = planche::composer(&p, &v, &Options { marie: false, ..Options::default() }, s).expect("image");
    let plein = planche::composer(&p, &v, &Options { marie: true, marie_opacite: 100, ..Options::default() }, s).expect("image");
    let moitie = planche::composer(&p, &v, &Options { marie: true, marie_opacite: 40, ..Options::default() }, s).expect("image");
    let coin_hors = |img: &RgbImage| img.get_pixel(1, 1).0;
    assert_eq!(coin_hors(&sans), [255, 255, 255], "sans passe-partout : le papier de la page");
    assert_eq!(coin_hors(&plein), [0, 0, 0], "à 100 % : noir plein");
    let m = coin_hors(&moitie);
    assert!(m[0] > 100 && m[0] < 180 && m[0] == m[1], "à 40 % : le passe-partout laisse voir dessous ({m:?})");
    // Les vignettes restent intactes dedans.
    let k = &p.cartes[0].place.thumb;
    let (x, y) = (((k.center().x - p.cadre.left()) * s) as u32, ((k.center().y - p.cadre.top()) * s) as u32);
    assert_eq!(plein.get_pixel(x, y).0, sans.get_pixel(x, y).0);
}

#[test]
fn the_export_box_is_remembered_across_sessions_through_the_settings_file_and_the_window_writes_it() {
    // La fenêtre : cocher la case écrit les réglages de l'application (pas le projet).
    let mut b = ouvert(quatre_plans(), 4);
    b.app.open_planche();
    b.chauffer(2);
    b.app.settings_a_ecrire = false;
    assert!(!b.app.settings.vignettes.export_passe_partout);
    let modifie = b.app.modified;
    b.app.planche_regler_passe_partout(true);
    assert!(b.app.settings_a_ecrire, "les réglages de l'application sont à écrire");
    assert_eq!(b.app.modified, modifie, "le projet n'est pas touché");
    assert!(b.app.planche_options_de_depart().0.marie);
    let ecrit = toml::to_string(&b.app.settings.vignettes).unwrap();
    let autre: crate::vignettesreglages::ReglagesVignettes = toml::from_str(&ecrit).unwrap();
    assert!(autre.export_passe_partout, "à la séance suivante : coché");
    assert!(!autre.est_defaut());
}
