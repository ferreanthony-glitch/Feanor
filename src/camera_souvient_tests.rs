//! La caméra se souvient (partie 131, Anthony, 2026-11-09 : « une grande planche de dessin sur laquelle des caméras sont posées » ; son choix :
//! « Ok on essaye A »). **Un trait appartient tout entier à la case où le stylet s'est posé ; hors de toute case, il appartient au papier.** Au
//! vrai geste sur le `Banc`, en vectoriel et en bitmap, vu en données et en pixels : où est le trait, ce que montre la planche (la feuille),
//! ce que montre le passe-partout (le film), ce que montre le film du plan, la sélection qui le déplace, l'enregistrement, le coût.
//!
//! Les essais d'avant qui décrivaient la découpe du trait au bord des cases ont été réécrits pour dire cette règle, là où ils sont
//! (`horschamp_tests`, `canevas_libre_tests`, `traits_continus_tests`, `traits_caches_tests`, `caseprend_tests`, `vignettes_dessin_tests`…) ;
//! ceux qui gardent un trait **coupé comme avant** (`horschamp::COMME_AVANT_131`) disent ce que devient le trait d'un projet d'avant.

use eframe::egui::{pos2, vec2, Color32, Pos2, Rect};

use crate::document::LayerKind;
use crate::perf_tests::Banc;
use crate::traits_continus_tests::{continu, des_plans, planche, px, repos, sauver, tracer};
use crate::vector::{Element, VPoint};
use crate::vignettes_selection_tests::{boite, choisies, glisser_droit, lasso, lasso_par, parts, present, sur_le_trait, voir};
use crate::vignettes_tests::encre;

/// La case `k` à l'écran.
fn case(b: &Banc, k: usize) -> Rect {
    let l = b.app.board_layout();
    b.app.vignettes.xf.rect(l.cards[k].thumb)
}

/// Les pixels d'encre rouge dans le rectangle d'écran `r` (un peu rentré : ses filets n'en sont pas).
fn rouge_dans(rgba: &[u8], r: Rect) -> usize {
    let mut n = 0;
    for y in (r.top() as i32 + 3)..(r.bottom() as i32 - 3) {
        for x in (r.left() as i32 + 3)..(r.right() as i32 - 3) {
            let v = px(rgba, x, y);
            n += usize::from(v[0] - v[1] > 40);
        }
    }
    n
}

/// Ce qui est sur le papier : les traits du calque « Hors cadres » et les morceaux de la couche bitmap de la planche.
fn sur_le_papier(b: &Banc) -> usize {
    let a = &b.app.doc.annotations;
    a.traces.iter().filter(|t| a.calque(t.calque).is_some_and(|c| c.planche)).count() + a.bitmap.morceaux.len()
}

/// Les pixels d'encre des dessins du plan `i` **hors de la feuille** (ses tuiles hors de la fenêtre).
fn pixels_hors_feuille(doc: &crate::document::Document, i: usize) -> usize {
    let mut vus = Vec::new();
    let mut n = 0;
    for id in doc.panels[i].layers.iter().flat_map(|l| l.frames.iter().flatten()) {
        if vus.contains(id) {
            continue;
        }
        vus.push(*id);
        if let Some(c) = doc.cels.get(id) {
            n += c.raster.outer.values().map(|t| t.iter().filter(|p| p.a() != 0).count()).sum::<usize>();
        }
    }
    n
}

/// Les pixels d'encre des dessins bitmap du plan `i` : (dans le cadre de la caméra au repos, hors de ce cadre). La feuille est plus grande que
/// le cadre, centrée sur lui : « hors du cadre » compte la marge de la feuille et ce qui est au-delà d'elle.
fn pixels_du_plan(doc: &crate::document::Document, i: usize) -> (usize, usize) {
    let t = crate::raster::TILE as i64;
    let (mut dedans, mut dehors) = (0, 0);
    let mut vus = Vec::new();
    for id in doc.panels[i].layers.iter().flat_map(|l| l.frames.iter().flatten()) {
        if vus.contains(id) {
            continue;
        }
        vus.push(*id);
        let Some(c) = doc.cels.get(id) else { continue };
        let r = &c.raster;
        let (fw, fh) = (doc.frame[0] as i64, doc.frame[1] as i64);
        let (ox, oy) = ((r.width as i64 - fw) / 2, (r.height as i64 - fh) / 2);
        for tuile in r.drawn_ids() {
            let (x0, y0) = r.tile_origin(tuile);
            for y in y0..y0 + t {
                for x in x0..x0 + t {
                    if r.get_i(x, y).a() != 0 {
                        if (ox..ox + fw).contains(&x) && (oy..oy + fh).contains(&y) {
                            dedans += 1;
                        } else {
                            dehors += 1;
                        }
                    }
                }
            }
        }
    }
    (dedans, dehors)
}

/// Les pixels d'encre rouge de l'image du film du plan `i` (192 × 108, le cadre de sa caméra).
fn rouge_du_film(b: &Banc, i: usize) -> usize {
    let doc = &b.app.doc;
    let plan = crate::render::panel_plan_at(doc, i, 0, Some(doc.paper));
    let cam = doc.panel_camera(i, 0);
    crate::render::thumbnail(doc, &plan, &cam, 192, 108).pixels.iter().filter(|p| i32::from(p.r()) - i32::from(p.g()) > 40).count()
}

/// Un point du papier, loin de toute case : le coin haut gauche de la vue (vérifié).
fn coin_du_papier(b: &Banc) -> Pos2 {
    let view = b.app.vignettes.view;
    let a = pos2(view.left() + 6.0, view.top() + 6.0);
    let l = b.app.board_layout();
    let p = b.app.vignettes.xf.to_board(a);
    assert!(!crate::horschamp::champs(&l, b.app.settings.vignettes.debord_fraction()).iter().any(|c| c.contains(p)), "le coin est hors de tout champ");
    a
}

/// Prendre la boîte du choix vectoriel loin de son pivot et la glisser de `d`.
fn deplacer_le_choix(b: &mut Banc, d: eframe::egui::Vec2) {
    let q = boite(b);
    glisser_droit(b, pos2(q[0].x * 0.7 + q[2].x * 0.3, q[0].y * 0.7 + q[2].y * 0.3), d);
}

// ------------------------------------------------------------------------------ la règle

#[test]
fn a_stroke_belongs_whole_to_the_case_where_the_pen_went_down() {
    for bitmap in [false, true] {
        let Some(mut b) = planche(des_plans(18, bitmap), 6, 1.0) else { return println!("pas de carte graphique") };
        let sorte = if bitmap { "bitmap" } else { "vecteur" };
        let (v0, v3) = (case(&b, 0), case(&b, 3));
        let y = v0.top() + v0.height() * 0.2;
        // De la case 1 à la case 4, à travers les cases 2 et 3.
        let (de, a) = (pos2(v0.center().x, y), pos2(v3.center().x, y));
        let etapes = b.app.history.undo_len();
        tracer(&mut b, de, a);
        repos(&mut b);
        // Les données : tout le trait dans le plan où il commence, rien ailleurs, rien sur le papier, rien de coupé.
        assert_eq!(b.app.history.undo_len(), etapes + 1, "{sorte} : une seule étape");
        assert!(encre(&b.app.doc, 0), "{sorte} : le trait est dans le plan de la case où le stylet s'est posé");
        for i in 1..=3 {
            assert!(!encre(&b.app.doc, i), "{sorte} : le plan {} , que le trait traverse, n'a rien reçu", i + 1);
        }
        assert_eq!(sur_le_papier(&b), 0, "{sorte} : rien sur le papier");
        assert!(b.app.doc.annotations.bitmap.empreintes.is_empty(), "{sorte} : aucune empreinte, le trait n'est pas réparti");
        if bitmap {
            assert!(pixels_hors_feuille(&b.app.doc, 0) > 200, "bitmap : ses pixels hors du cadre restent dans le dessin du plan ({})", pixels_hors_feuille(&b.app.doc, 0));
        } else {
            assert_eq!(parts(&b), (1, 0), "vecteur : un seul élément, le trait entier");
            let cadre = b.app.doc.frame[0] as f32;
            let loin = b.app.doc.cels.values().filter_map(|c| c.vector.as_ref()).flat_map(|v| v.elements.iter()).any(|e| matches!(e, Element::Stroke { points, trace, .. } if trace.lien.is_none() && points.iter().any(|p| p.x > cadre * 3.0)));
            assert!(loin, "vecteur : il garde ses points jusqu'à trois cadres plus loin, sans numéro de part");
        }
        // La feuille : la planche le montre en entier, par-dessus les cases qu'il traverse.
        let rgba = b.capturer().unwrap();
        sauver(&rgba, &format!("souvient-feuille-{sorte}"));
        continu(&rgba, de, a, &format!("la feuille, {sorte}"));
        // Le film du plan : coupé au cadre de sa caméra ; les plans traversés n'ont rien.
        assert!(rouge_du_film(&b, 0) > 0, "{sorte} : le film du plan 1 montre le trait");
        for i in 1..=3 {
            assert_eq!(rouge_du_film(&b, i), 0, "{sorte} : le film du plan {} n'a rien du trait", i + 1);
        }
        // Le passe-partout : la planche montre le film, chaque case son plan.
        b.app.vignettes.annot.marie = true;
        b.chauffer(4);
        repos(&mut b);
        let rgba = b.capturer().unwrap();
        sauver(&rgba, &format!("souvient-film-{sorte}"));
        assert!(rouge_dans(&rgba, v0) > 0, "{sorte} : le film : la case 1 montre son trait");
        for k in 1..=3 {
            assert_eq!(rouge_dans(&rgba, case(&b, k)), 0, "{sorte} : le film : la case {} ne montre que son plan", k + 1);
        }
        b.app.vignettes.annot.marie = false;
        // Annuler : tout part d'un coup.
        b.app.undo();
        repos(&mut b);
        assert!(!encre(&b.app.doc, 0) && pixels_hors_feuille(&b.app.doc, 0) == 0, "{sorte} : annulé");
    }
}

#[test]
fn a_stroke_that_starts_on_the_paper_is_paper_whole_and_shows_over_the_cases() {
    for bitmap in [false, true] {
        let Some(mut b) = planche(des_plans(18, bitmap), 6, 1.0) else { return println!("pas de carte graphique") };
        let sorte = if bitmap { "bitmap" } else { "vecteur" };
        // Du coin du papier jusqu'au milieu de la case 2, à travers le haut de la case 1.
        let (de, a) = (coin_du_papier(&b), case(&b, 1).center());
        let etapes = b.app.history.undo_len();
        tracer(&mut b, de, a);
        repos(&mut b);
        assert_eq!(b.app.history.undo_len(), etapes + 1, "{sorte} : une seule étape");
        for i in 0..b.app.doc.panels.len() {
            assert!(!encre(&b.app.doc, i) && pixels_hors_feuille(&b.app.doc, i) == 0, "{sorte} : rien dans le plan {}", i + 1);
        }
        assert_eq!(sur_le_papier(&b), 1, "{sorte} : le trait est sur le papier, entier, en un seul morceau");
        let rgba = b.capturer().unwrap();
        sauver(&rgba, &format!("souvient-papier-{sorte}"));
        continu(&rgba, de, a, &format!("le papier par-dessus les cases, {sorte}"));
        // Le passe-partout : le papier n'est dans aucun plan, les cases ne le montrent pas.
        b.app.vignettes.annot.marie = true;
        b.chauffer(4);
        repos(&mut b);
        let rgba = b.capturer().unwrap();
        assert_eq!(rouge_dans(&rgba, case(&b, 1)) + rouge_dans(&rgba, case(&b, 0)), 0, "{sorte} : le film : aucune case ne montre le papier");
        b.app.vignettes.annot.marie = false;
        b.app.undo();
        repos(&mut b);
        assert_eq!(sur_le_papier(&b), 0, "{sorte} : annulé");
    }
}

// ------------------------------------------------------------------------------ enregistrer, rouvrir

#[test]
fn what_a_case_drew_beyond_its_frame_is_saved_with_its_plan_and_shows_again_after_reopening() {
    for bitmap in [false, true] {
        let Some(mut b) = planche(des_plans(6, bitmap), 6, 1.0) else { return println!("pas de carte graphique") };
        let sorte = if bitmap { "bitmap" } else { "vecteur" };
        let (t1, t4) = (case(&b, 1), case(&b, 4));
        let (de, a) = (t1.center() + vec2(0.0, -12.0), t4.center() + vec2(0.0, 14.0));
        tracer(&mut b, de, a);
        repos(&mut b);
        let hors = pixels_hors_feuille(&b.app.doc, 1);
        let dossier = std::env::temp_dir().join(format!("shot-test-camera-souvient-{sorte}-{}-{:?}", std::process::id(), std::thread::current().id()));
        std::fs::remove_dir_all(&dossier).ok();
        std::fs::create_dir_all(&dossier).unwrap();
        let manifeste = dossier.join("P.shot");
        crate::project::save(&b.app.doc, &manifeste, &mut Default::default()).expect("enregistrement");
        assert!(!dossier.join(crate::planchebitmap::DOSSIER).exists(), "{sorte} : rien sur la planche : aucun dossier `planche_bitmap/`");
        assert!(!dossier.join(crate::annotations::FILE).exists(), "{sorte} : rien sur la planche : aucun fichier d'annotations");
        let relu = crate::project::load(&manifeste).expect("réouverture");
        std::fs::remove_dir_all(&dossier).ok();
        assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
        assert!(encre(&relu.doc, 1) && !encre(&relu.doc, 2), "{sorte} : relu, le trait est dans son plan, entier");
        assert_eq!(pixels_hors_feuille(&relu.doc, 1), hors, "{sorte} : relu, les mêmes pixels hors du cadre");
        let (zoom, pan) = (b.app.board_zoom, b.app.board_pan);
        let Some(mut c) = planche(relu.doc, 6, 1.0) else { return };
        c.app.board_zoom = zoom;
        c.app.board_pan = pan;
        c.chauffer(4);
        c.attendre_vignettes();
        repos(&mut c);
        let rgba = c.capturer().unwrap();
        sauver(&rgba, &format!("souvient-rouvert-{sorte}"));
        continu(&rgba, de, a, &format!("rouvert, {sorte}"));
    }
}

// ------------------------------------------------------------------------------ la sélection de la planche

#[test]
fn a_stroke_moved_with_the_lasso_goes_whole_where_its_first_point_lands() {
    let Some(mut b) = planche(des_plans(12, false), 6, 1.0) else { return println!("pas de carte graphique") };
    let (t1, t2, t3) = (case(&b, 1), case(&b, 2), case(&b, 3));
    // De la case 2 à la case 4, à travers la case 3.
    let (de, a) = (t1.center() + vec2(-20.0, -10.0), t3.center() + vec2(0.0, 10.0));
    tracer(&mut b, de, a);
    repos(&mut b);
    assert_eq!(parts(&b), (1, 0));
    assert!(encre(&b.app.doc, 1) && !encre(&b.app.doc, 2) && !encre(&b.app.doc, 3));
    // Ctrl + lasso dans la case 3, où il ne fait que passer : le trait entier est pris, là où la planche le montre.
    lasso(&mut b, sur_le_trait(de, a, t2.center().x), 10.0);
    assert_eq!(choisies(&b), 1, "le trait se prend partout où il passe ({})", b.app.status);
    let etapes = b.app.history.undo_len();
    // Une case vers la droite : son premier point arrive dans la case 3, tout le trait passe à son plan.
    let d = vec2(t2.center().x - t1.center().x, 0.0);
    deplacer_le_choix(&mut b, d);
    // (Le dessin de la case quittée se refait en arrière-plan, sans latence : on attend le repos avant de lire ses pixels.)
    repos(&mut b);
    assert_eq!(parts(&b), (1, 0), "toujours un seul trait, entier ({})", b.app.status);
    assert!(encre(&b.app.doc, 2) && !encre(&b.app.doc, 1) && !encre(&b.app.doc, 3), "il est dans le plan de la case où il commence maintenant");
    assert_eq!(b.app.history.undo_len(), etapes + 1, "une seule étape");
    present(&voir(&mut b, "souvient-lasso-case", de + d, a + d), "déplacé, entier");
    assert_eq!(choisies(&b), 1, "le choix le suit");
    // Vers le haut, au-dessus de la première rangée : son premier point est sur le papier, tout le trait y va.
    let haut = vec2(0.0, -(t1.height() * 0.5 + t1.width() * 0.25));
    deplacer_le_choix(&mut b, haut);
    repos(&mut b);
    assert_eq!(parts(&b), (0, 1), "il commence sur le papier : entier sur la planche ({})", b.app.status);
    assert!((1..=3).all(|i| !encre(&b.app.doc, i)), "plus rien dans les plans");
    present(&voir(&mut b, "souvient-lasso-papier", de + d + haut, a + d + haut), "sur le papier, entier, par-dessus les cases");
    // Annuler deux fois : tout revient.
    b.app.undo();
    b.app.undo();
    repos(&mut b);
    assert_eq!(parts(&b), (1, 0));
    assert!(encre(&b.app.doc, 1) && !encre(&b.app.doc, 2));
}

#[test]
fn bitmap_pixels_moved_with_the_lasso_stay_in_their_case_until_they_leave_it_then_go_whole_to_the_case_under_them() {
    // 1. Un trait court dans la case 3, déplacé d'une case : il quitte son champ tout entier, il va au plan de la case où il arrive.
    let Some(mut b) = planche(des_plans(12, true), 6, 1.0) else { return println!("pas de carte graphique") };
    let (t2, t3) = (case(&b, 2), case(&b, 3));
    // (En biais : la boîte du choix a de la hauteur, la prendre ne tombe pas sur un bord.)
    let (de, a) = (t2.center() + vec2(-30.0, -18.0), t2.center() + vec2(30.0, 18.0));
    tracer(&mut b, de, a);
    repos(&mut b);
    lasso(&mut b, t2.center(), 50.0);
    assert!(b.app.selb_active(), "le lasso a pris les pixels du trait : {}", b.app.status);
    let etapes = b.app.history.undo_len();
    glisser_droit(&mut b, t2.center() + (a - de).normalized() * 12.0, vec2(t3.center().x - t2.center().x, 0.0));
    repos(&mut b);
    assert!(encre(&b.app.doc, 3) && !encre(&b.app.doc, 2), "les pixels sont passés au plan de la case d'arrivée ({})", b.app.status);
    assert_eq!(sur_le_papier(&b), 0, "rien sur la planche");
    assert_eq!(b.app.history.undo_len(), etapes + 1, "une seule étape");

    // 2. Un trait de la case 3 à la case 4, pris en entier et poussé de quelques points : il a toujours de l'encre dans sa case, il y reste, entier.
    let Some(mut b) = planche(des_plans(12, true), 6, 1.0) else { return };
    let (t2, t3) = (case(&b, 2), case(&b, 3));
    let (de, a) = (t2.center(), t3.center() + vec2(0.0, 10.0));
    tracer(&mut b, de, a);
    repos(&mut b);
    assert!(encre(&b.app.doc, 2) && !encre(&b.app.doc, 3));
    let (g, h) = (de + vec2(-20.0, -30.0), a + vec2(20.0, 30.0));
    lasso_par(&mut b, &[g, pos2(h.x, g.y), h, pos2(g.x, h.y), g]);
    assert!(b.app.selb_active(), "le lasso a pris le trait entier, dans sa case et au-delà : {}", b.app.status);
    let d = vec2(0.0, 8.0);
    glisser_droit(&mut b, de + (a - de) * 0.5 + (a - de).normalized() * 12.0, d);
    repos(&mut b);
    assert!(encre(&b.app.doc, 2) && !encre(&b.app.doc, 3), "poussé de quelques points, il reste entier dans son plan ({})", b.app.status);
    assert_eq!(sur_le_papier(&b), 0, "rien sur la planche");
    let choix = b.app.vignettes.selb.take();
    b.chauffer(2);
    let rgba = b.capturer().unwrap();
    b.app.vignettes.selb = choix;
    sauver(&rgba, "souvient-lasso-bitmap");
    continu(&rgba, de + d, a + d, "le trait bitmap poussé, entier");
}

// ------------------------------------------------------------------------------ la gomme de la planche

/// Ce qu'on voit sur le papier se gomme sur le papier (`gommedebord.rs`) : la gomme posée hors de toute case efface le trait d'une case qui dépasse là
/// (vectoriel : le trait entier ; bitmap : les pixels sous son cercle), sans toucher au dessin que la case a dans son cadre ; une seule étape.
#[test]
fn the_board_eraser_erases_what_a_case_drew_beyond_its_frame() {
    for bitmap in [false, true] {
        let Some(mut b) = planche(des_plans(12, bitmap), 6, 1.0) else { return println!("pas de carte graphique") };
        let sorte = if bitmap { "bitmap" } else { "vecteur" };
        let t1 = case(&b, 1);
        // Une gomme de 12 pixels d'écran de rayon (le rayon de la gomme de la planche : la moitié de celui de l'outil).
        b.app.eraser.size = 4.0 * 12.0 / b.app.vignettes.xf.zoom;
        // La gomme partie du papier, bien au-dessus de la case, puis menée le long de `chemin`.
        let gommer = |b: &mut Banc, chemin: &[Pos2]| {
            b.app.set_tool(crate::app::Tool::Eraser);
            b.chauffer(1);
            let mut pas = Vec::new();
            for w in chemin.windows(2) {
                let n = ((w[1] - w[0]).length() / 4.0).ceil().max(1.0) as usize;
                pas.extend((0..n).map(|i| w[0] + (w[1] - w[0]) * (i as f32 / n as f32)));
            }
            pas.push(chemin[chemin.len() - 1]);
            crate::vignettes_tests::glisser(b, &pas);
            b.app.set_tool(crate::app::Tool::Brush);
            repos(b);
        };
        // Deux traits tout entiers dans la case 2 : un au milieu, un tout contre son bord du haut.
        tracer(&mut b, t1.center() + vec2(-40.0, 25.0), t1.center() + vec2(40.0, 25.0));
        tracer(&mut b, pos2(t1.left() + 20.0, t1.top() + 7.0), pos2(t1.left() + 60.0, t1.top() + 7.0));
        repos(&mut b);
        let avant = (parts(&b), pixels_du_plan(&b.app.doc, 1), b.app.history.undo_len());
        assert!(if bitmap { avant.1 .0 > 0 && avant.1 .1 == 0 } else { avant.0 == (2, 0) }, "{sorte} : deux traits dans le cadre de la case 2 ({avant:?})");
        // 1. La gomme descend du papier jusqu'à trois pixels du bord de la case et le longe : son cercle mord dans le cadre, par-dessus le
        // trait du haut. **Dans son cadre, le dessin de la case est à elle** : rien n'est touché, aucune étape. (Relecture de la partie 131 :
        // un trait vectoriel resté tout entier dans le cadre partait, pris par le cercle.)
        let loin = t1.top() - t1.width() * 0.22;
        gommer(&mut b, &[pos2(t1.left() + 10.0, loin), pos2(t1.left() + 10.0, t1.top() - 3.0), pos2(t1.left() + 70.0, t1.top() - 3.0)]);
        assert_eq!((parts(&b), pixels_du_plan(&b.app.doc, 1), b.app.history.undo_len()), avant, "{sorte} : la gomme de la planche ne touche pas à ce qui est dans le cadre");
        // 2. Un troisième trait, qui sort de la case par le haut et monte au-dessus de la première rangée.
        let haut = pos2(t1.center().x + 30.0, t1.top() - t1.width() * 0.3);
        tracer(&mut b, t1.center() + vec2(0.0, -10.0), haut);
        repos(&mut b);
        assert_eq!(sur_le_papier(&b), 0);
        let (el, (dedans, hors)) = (parts(&b).0, pixels_du_plan(&b.app.doc, 1));
        assert!(if bitmap { dedans > 0 && hors > 0 } else { el == 3 }, "{sorte} : trois traits dans le plan 2, dont un qui sort de son cadre ({el}, {dedans}, {hors})");
        let etapes = b.app.history.undo_len();
        // La gomme, posée sur le papier au-dessus de la première rangée, à côté du trait, glissée à travers son bout.
        let y = loin;
        let x = t1.center().x + 30.0 * ((t1.center().y - 10.0 - y) / (t1.center().y - 10.0 - haut.y));
        let a_travers = [pos2(x - 40.0, y), pos2(x + 40.0, y)];
        // 3. Le storyboard verrouillé : les dessins des plans ne changent plus.
        b.app.doc.storyboard_locked = true;
        gommer(&mut b, &a_travers);
        b.app.doc.storyboard_locked = false;
        assert_eq!((parts(&b).0, pixels_du_plan(&b.app.doc, 1), b.app.history.undo_len()), (el, (dedans, hors), etapes), "{sorte} : storyboard verrouillé, la gomme ne touche à aucun plan");
        // 4. Le passe-partout : ce qui dépasse des cases est caché, on ne gomme pas ce qu'on ne voit pas.
        b.app.vignettes.annot.marie = true;
        b.chauffer(2);
        gommer(&mut b, &a_travers);
        b.app.vignettes.annot.marie = false;
        b.chauffer(2);
        repos(&mut b);
        assert_eq!((parts(&b).0, pixels_du_plan(&b.app.doc, 1), b.app.history.undo_len()), (el, (dedans, hors), etapes), "{sorte} : avec le passe-partout, la gomme ne touche pas à ce qui dépasse");
        // 5. Sur la feuille : le bout part.
        gommer(&mut b, &a_travers);
        assert_eq!(b.app.history.undo_len(), etapes + 1, "{sorte} : une seule étape pour le geste de gomme");
        if bitmap {
            let (dedans_apres, apres) = pixels_du_plan(&b.app.doc, 1);
            assert!(apres < hors, "bitmap : les pixels sous la gomme, hors du cadre, sont effacés ({hors} -> {apres})");
            assert!(apres > 0, "bitmap : le reste du bout, hors du cercle de la gomme, est toujours là");
            assert_eq!(dedans_apres, dedans, "bitmap : rien n'est effacé dans le cadre de la case");
        } else {
            assert_eq!(parts(&b), (2, 0), "vecteur : le trait qui dépassait est parti, entier ; les deux traits du cadre restent");
        }
        assert!(encre(&b.app.doc, 1), "{sorte} : la case garde ce qu'elle a dans son cadre");
        // À l'écran : plus rien sous la gomme.
        let rgba = b.capturer().unwrap();
        sauver(&rgba, &format!("souvient-gomme-{sorte}"));
        assert_eq!(rouge_dans(&rgba, Rect::from_center_size(pos2(x, y), vec2(14.0, 14.0))), 0, "{sorte} : le bout est gommé à l'écran");
        // Annuler : tout revient d'un coup.
        b.app.undo();
        repos(&mut b);
        assert_eq!((parts(&b).0, pixels_du_plan(&b.app.doc, 1)), (el, (dedans, hors)), "{sorte} : annulé");
    }
}

// ------------------------------------------------------------------------------ la relecture de la partie 131

/// Les pixels de `r` que `f` retient.
fn compter(r: &crate::raster::Raster, f: impl Fn(Color32) -> bool) -> usize {
    let t = crate::raster::TILE as i64;
    let mut n = 0;
    for id in r.drawn_ids() {
        let (x0, y0) = r.tile_origin(id);
        for y in y0..y0 + t {
            for x in x0..x0 + t {
                n += usize::from(f(r.get_i(x, y)));
            }
        }
    }
    n
}

/// Les pixels d'encre bleue dans le rectangle d'écran `r`.
fn bleu_dans(rgba: &[u8], r: Rect) -> usize {
    let mut n = 0;
    for y in (r.top() as i32 + 3)..(r.bottom() as i32 - 3) {
        for x in (r.left() as i32 + 3)..(r.right() as i32 - 3) {
            let v = px(rgba, x, y);
            n += usize::from(v[2] - v[0] > 40);
        }
    }
    n
}

const BLEU: [u8; 3] = [30, 30, 200];

/// Ce qui dépasse d'une case se peint par-dessus les autres **sans les cacher** : seule son encre couvre, le reste de sa vignette est transparent.
#[test]
fn what_overflows_a_case_does_not_hide_the_drawing_of_the_cases_it_crosses() {
    let Some(mut b) = planche(des_plans(12, false), 6, 1.0) else { return println!("pas de carte graphique") };
    let (t0, t1) = (case(&b, 0), case(&b, 1));
    // La case 2 a son dessin, en bleu ; puis un trait rouge part de la case 1 et la traverse.
    b.app.color = BLEU;
    tracer(&mut b, t1.center() + vec2(-20.0, -30.0), t1.center() + vec2(-20.0, 30.0));
    tracer(&mut b, t1.center() + vec2(-50.0, 30.0), t1.center() + vec2(50.0, 30.0));
    b.app.color = [200, 30, 40];
    let (de, a) = (t0.center(), t1.center() + vec2(40.0, 0.0));
    tracer(&mut b, de, a);
    repos(&mut b);
    assert!(encre(&b.app.doc, 0) && parts(&b) == (3, 0), "le trait rouge est au plan 1, entier ({:?})", parts(&b));
    let bleus = bleu_dans(&b.capturer().unwrap(), t1);
    let rgba = b.capturer().unwrap();
    sauver(&rgba, "souvient-par-dessus");
    continu(&rgba, de, a, "le trait rouge, par-dessus la case 2");
    assert!(bleus > 150, "le dessin de la case 2 se voit toujours sous ce qui la traverse ({bleus} pixels bleus)");
}

/// Un dessin que plusieurs cases montrent garde la règle d'avant (`render::dessins_communs`) : le Décor d'un plan est réparti case par case, et
/// aucune case ne s'étend pour lui. Sans cela, chaque case d'une continuité répéterait autour d'elle ce qui dépasse du décor, par-dessus ses voisines.
#[test]
fn a_drawing_that_several_cases_show_keeps_the_rule_of_before() {
    let mut doc = des_plans(12, false);
    for p in &mut doc.panels {
        p.layers[0].decor = true;
    }
    let Some(mut b) = planche(doc, 6, 1.0) else { return println!("pas de carte graphique") };
    let (t0, t2) = (case(&b, 0), case(&b, 2));
    let (de, a) = (t0.center(), t2.center());
    tracer(&mut b, de, a);
    repos(&mut b);
    assert!((0..=2).all(|i| encre(&b.app.doc, i)), "le trait du Décor est réparti entre les cases qu'il traverse, comme avant ({})", b.app.status);
    let l = b.app.board_layout();
    for k in 0..=2 {
        let c = l.cards[k];
        assert_eq!(b.app.thumbs.debord_de_case(&b.app.doc, c.panel, c.frame, c.id, None), [0.0; 4], "la case {} ne s'étend pas pour son Décor", k + 1);
    }
    // À l'écran : le trait est là, d'un bout à l'autre.
    let rgba = b.capturer().unwrap();
    sauver(&rgba, "souvient-decor");
    continu(&rgba, de, a, "le trait du Décor, réparti");

    // Un dessin tenu d'une marque d'image à l'autre est commun aux deux cases du plan ; chacune son dessin : il est à elle seule.
    let mut doc = des_plans(1, false);
    let (un, deux) = (doc.new_cel(LayerKind::Vector), doc.new_cel(LayerKind::Vector));
    for f in 0..4 {
        doc.panels[0].layers[0].set(f, Some(un));
    }
    doc.panels[0].marks.push(crate::document::ImageMark { id: 77, frame: 2, color: 0, action: String::new(), dialogue: String::new(), notes: String::new() });
    assert_eq!(crate::render::dessins_communs(&doc, 0, 0), vec![un]);
    assert_eq!(crate::render::dessins_communs(&doc, 0, 3), vec![un]);
    for f in 2..4 {
        doc.panels[0].layers[0].set(f, Some(deux));
    }
    assert!(crate::render::dessins_communs(&doc, 0, 0).is_empty() && crate::render::dessins_communs(&doc, 0, 2).is_empty());
}

/// Un trait bitmap parti du papier traverse le dessin d'une case : il va entier sur le papier, **seul** (`horschamp::part_du_trait`). Le dessin de
/// la case reste tel qu'il était, et le morceau du papier n'en emporte pas un double.
#[test]
fn a_bitmap_stroke_from_the_paper_does_not_take_the_drawing_it_crosses() {
    let Some(mut b) = planche(des_plans(12, true), 6, 1.0) else { return println!("pas de carte graphique") };
    let t1 = case(&b, 1);
    // Le dessin de la case 2 : un trait bleu, opaque, en travers.
    b.app.color = BLEU;
    tracer(&mut b, t1.center() + vec2(-50.0, 0.0), t1.center() + vec2(50.0, 0.0));
    repos(&mut b);
    let cel = b.app.doc.panels[1].layers[0].cel_at(0).unwrap();
    let bleus = compter(&b.app.doc.cels[&cel].raster, |c| c.a() != 0);
    assert!(bleus > 0);
    // Un trait rouge à moitié transparent, parti du papier au-dessus de la rangée, qui descend à travers le trait bleu.
    b.app.color = [200, 30, 40];
    b.app.brush.opacity = 0.5;
    tracer(&mut b, pos2(t1.center().x, t1.top() - t1.width() * 0.25), t1.center() + vec2(0.0, 30.0));
    repos(&mut b);
    assert_eq!(sur_le_papier(&b), 1, "le trait est sur le papier, entier ({})", b.app.status);
    let r = &b.app.doc.cels[&cel].raster;
    assert_eq!(compter(r, |c| c.a() != 0), bleus, "le dessin de la case a les mêmes pixels qu'avant");
    assert_eq!(compter(r, |c| i32::from(c.r()) - i32::from(c.b()) > 20), 0, "aucun pixel du trait rouge n'est resté dans le dessin de la case");
    let morceau = &b.app.doc.annotations.bitmap.morceaux[0];
    assert!(compter(&morceau.pixels, |c| c.a() != 0) > 100, "le morceau du papier porte le trait");
    assert_eq!(compter(&morceau.pixels, |c| c.a() != 0 && i32::from(c.b()) > i32::from(c.r()) + 8), 0, "le morceau du papier n'emporte pas le bleu de la case");

    // Le calcul seul : le trait retrouvé sous l'encre qu'il recouvrait, dessus et dessous.
    let pres = |x: Color32, y: Color32| x.to_array().iter().zip(y.to_array()).all(|(u, v)| (i32::from(*u) - i32::from(v)).abs() <= 3);
    let (encre_opaque, encre_claire) = (Color32::from_rgba_premultiplied(30, 30, 200, 255), Color32::from_rgba_premultiplied(0, 0, 100, 128));
    let (demi, plein) = (Color32::from_rgba_premultiplied(100, 15, 20, 128), Color32::from_rgba_premultiplied(200, 30, 40, 255));
    for avant in [encre_opaque, encre_claire] {
        for le_trait in [demi, plein] {
            let dessus = crate::horschamp::part_du_trait(avant, crate::render::over(avant, le_trait, 255, None), [200, 30, 40], false);
            assert!(pres(dessus, le_trait), "dessus : {le_trait:?} sur {avant:?} rend {dessus:?}");
        }
    }
    // (Dessous : il ne reste du trait que ce qui se voyait à travers l'encre ; sous une encre opaque, rien.)
    let dessous = crate::horschamp::part_du_trait(encre_claire, crate::render::over(plein, encre_claire, 255, None), [200, 30, 40], true);
    assert!(pres(dessous, plein), "dessous : {dessous:?}");
    assert_eq!(crate::horschamp::part_du_trait(encre_opaque, crate::render::over(plein, encre_opaque, 255, None), [200, 30, 40], true), Color32::TRANSPARENT);
}

/// La marge ordinaire d'une case ne rétrécit jamais, quelle que soit la taille de son image (une tuile d'export) : seul ce qui s'étend au-delà
/// d'elle se borne. (Relecture de la partie 131 : une image plus grande que la borne perdait toute sa marge.)
#[test]
fn the_ordinary_margin_of_a_case_never_shrinks() {
    let mut doc = des_plans(1, true);
    let cel = doc.new_cel(LayerKind::Bitmap);
    doc.panels[0].layers[0].set(0, Some(cel));
    // De l'encre loin du cadre, de tous les côtés.
    let (fw, fh) = (doc.frame[0] as i64, doc.frame[1] as i64);
    let t = crate::raster::TILE as i64;
    for (x, y) in [(-3 * fw, -3 * fh), (5 * fw, 5 * fh)] {
        let r = &mut doc.cels.get_mut(&cel).unwrap().raster;
        let id = r.id_of(x.div_euclid(t) as i32, y.div_euclid(t) as i32);
        r.tile_mut(id)[0] = Color32::BLACK;
    }
    let plan = crate::render::panel_plan_at(&doc, 0, 0, Some(doc.paper));
    let cam = doc.panel_camera(0, 0);
    for (tw, th, mini) in [(320usize, 180usize, 38usize), (3000, 1688, 360), (6000, 3375, 720)] {
        let m = crate::render::marges_de_l_encre(&doc, &plan, &cam, tw, th, mini, &[], crate::render::VIGNETTE_MAX);
        assert!(m.iter().all(|v| *v >= mini), "{tw} × {th} : la marge ordinaire ({mini}) reste de chaque côté : {m:?}");
        assert!(tw + m[0] + m[2] <= crate::render::VIGNETTE_MAX.max(tw + 2 * mini) && th + m[1] + m[3] <= crate::render::VIGNETTE_MAX.max(th + 2 * mini), "{tw} × {th} : l'image reste bornée : {m:?}");
        // Sans le dessin (il est commun à d'autres cases) : la marge ordinaire, rien de plus.
        assert_eq!(crate::render::marges_de_l_encre(&doc, &plan, &cam, tw, th, mini, &[cel], crate::render::VIGNETTE_MAX), [mini; 4]);
    }
    let m = crate::render::marges_de_l_encre(&doc, &plan, &cam, 320, 180, 38, &[], crate::render::VIGNETTE_MAX);
    assert!(m[0] > 38 && m[2] > 38 && m[0] <= 320 * crate::render::MARGE_MAX_CADRES, "une petite vignette s'étend à son encre, quatre cadres au plus : {m:?}");
}

// ------------------------------------------------------------------------------ le coût

/// Soixante cases, deux mille traits, et dans chaque case trois traits qui partent loin de son cadre (trois cadres à droite, deux en bas, un en
/// haut à gauche) : chaque vignette s'étend jusqu'à son encre. La planche se repose, une image d'écran reste légère, les vignettes tiennent
/// dans leur mémoire. (La mesure demandée à la relecture de la partie 127.)
#[test]
fn sixty_cases_and_two_thousand_strokes_with_long_overflows_rest_and_a_frame_stays_cheap() {
    let Some(mut b) = planche(des_plans(60, false), 6, 0.6) else { return println!("pas de carte graphique") };
    let l = b.app.board_layout();
    let (fw, fh) = (b.app.doc.frame[0] as f32, b.app.doc.frame[1] as f32);
    let mut n = 0;
    for k in 0..l.cards.len() {
        let c = l.cards[k];
        let pi = c.panel;
        let cel = match b.app.doc.panels[pi].layers[0].cel_at(c.frame) {
            Some(id) => id,
            None => {
                let id = b.app.doc.new_cel(LayerKind::Vector);
                let fc = b.app.doc.panels[pi].frame_count();
                for f in 0..fc {
                    b.app.doc.panels[pi].layers[0].set(f, Some(id));
                }
                id
            }
        };
        let d = b.app.drawing_mut(cel).unwrap();
        let trait_ = |points: Vec<VPoint>| Element::Stroke { points, color: [200, 30, 40], opacity: 1.0, hardness: 1.0, brush: None, trace: Default::default() };
        for j in 0..31 {
            let (x, y) = (100.0 + (j % 6) as f32 * 290.0, 100.0 + (j / 6) as f32 * 160.0);
            d.elements.push(trait_((0..20).map(|t| VPoint { x: x + t as f32 * 8.0, y: y + (t as f32 * 0.7).sin() * 30.0, r: 4.0 }).collect()));
        }
        for (vx, vy) in [(3.0 * fw, 0.1 * fh), (0.2 * fw, 2.0 * fh), (-fw, -fh)] {
            d.elements.push(trait_((0..=40).map(|t| VPoint { x: fw * 0.5 + vx * t as f32 / 40.0, y: fh * 0.5 + vy * t as f32 / 40.0, r: 5.0 }).collect()));
        }
        n += 34;
        b.app.refresh_cel(cel);
    }
    b.app.renderer.invalidate_all();
    let debut = std::time::Instant::now();
    b.chauffer(4);
    repos(&mut b);
    println!("COUT {n} traits sur {} cases, débords longs : repos après {:.1} s", l.cards.len(), debut.elapsed().as_secs_f64());
    // Au repos : plus aucune image demandée (sauf par `fx::draft_level`, commun au processus).
    for _ in 0..4 {
        b.image(Vec::new(), 0.5);
    }
    let reveil = b.raison_du_reveil();
    let derniers: Vec<&str> = reveil.lines().rev().take(3).collect();
    assert!(derniers.iter().all(|l| l.contains("demandée par rien") || l.contains("fx.rs")), "la planche ne redessine pas en continu : {reveil}");
    // Les vignettes : leur mémoire, et aucune recomposition au repos.
    let (octets, vignettes) = b.app.thumbs.octets();
    let rendues = b.app.thumbs.rendered();
    for _ in 0..20 {
        b.image(Vec::new(), 1.0 / 60.0);
    }
    assert_eq!(b.app.thumbs.rendered(), rendues, "au repos, aucune vignette n'est recomposée");
    println!("COUT vignettes : {vignettes} gardées, {:.1} Mo", octets as f64 / (1 << 20) as f64);
    assert!(octets < 512 << 20, "les vignettes étendues tiennent dans une mémoire raisonnable ({} Mo)", octets >> 20);
    // Le coût d'une image, la souris qui se promène sur la planche.
    let vue = b.app.vignettes.view;
    let mut mesures: Vec<f64> = (0..120)
        .map(|k| {
            let q = vue.center() + vec2((k as f32 * 0.21).sin() * vue.width() * 0.4, (k as f32 * 0.13).cos() * vue.height() * 0.4);
            b.image(vec![eframe::egui::Event::PointerMoved(q)], 1.0 / 60.0).construction
        })
        .collect();
    mesures.sort_by(f64::total_cmp);
    let mediane = mesures[mesures.len() / 2];
    println!("COUT image, {} cases à débords longs : construction {mediane:.2} ms (médiane), {:.2} ms (pire)", l.cards.len(), mesures[mesures.len() - 1]);
    assert!(mediane < 16.0, "construction d'une image : {mediane:.2} ms");
    // Un trait de plus dans une case : la planche revient au repos.
    let t = case(&b, 7);
    let debut = std::time::Instant::now();
    tracer(&mut b, t.center(), t.center() + vec2(t.width() * 1.5, 12.0));
    repos(&mut b);
    println!("COUT un trait de plus, puis le repos : {:.1} s", debut.elapsed().as_secs_f64());
}
