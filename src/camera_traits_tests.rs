//! Les traits d'un plan à mouvement de caméra (partie 118), **vus en pixels** : plan 6 d'une planche, cadre B posé par le geste des Vignettes,
//! de longs traits à travers A et B ; ce que montrent la case au repos et le canevas du Storyboard. La cause : les effets du calque (le tremblement par
//! défaut, sur tout calque vectoriel) ne rendaient que la feuille, ce qui la dépassait disparaissait de l'image dérivée (`fx::effets_hors_de_la_feuille`).

use eframe::egui::{pos2, vec2, Color32, Event, Modifiers, Pos2, Rect};

use crate::document::Document;
use crate::perf_tests::Banc;
use crate::traits_continus_tests::{des_plans, planche, relever, repos, sauver, tracer, Releve};
use crate::vignettes_tests::appui;
use crate::vignettes_camera_tests::{ctrl_glisser, pas};

/// Le plan 6.
const PLAN: usize = 5;
/// Le filet bleu d'un cadre, peint par-dessus le trait, le croise de biais sur quelques pixels (sans effet : 7). La coupure en faisait 59 à 300.
const TROU_PERMIS: usize = 8;

fn planche_de_12(bitmap: bool, tremble: bool) -> Option<Banc> {
    let mut doc: Document = des_plans(12, bitmap);
    for p in 0..doc.panels.len() {
        doc.set_duration(p, 24);
    }
    // (Les dessins dérivés se gardent par numéro de dessin, pour tout le processus : à l'application, ouvrir un projet les oublie (`fx::forget_all`) ;
    // ici, chaque banc a ses propres numéros, sinon il montrerait le dérivé du banc d'avant.)
    static BANCS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(40);
    for _ in 0..BANCS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) {
        doc.new_cel(crate::document::LayerKind::Vector);
    }
    let mut b = planche(doc, 5, 1.0)?;
    if tremble {
        // Le réglage du réalisateur (Préférences > Dessin), sans « Animer à l'arrêt » : le repos reste un repos.
        b.app.settings.tremblement_defaut = crate::tremblement::Defaut { actif: true, intensite: 0.5, vitesse: 3, ..Default::default() };
        b.chauffer(2);
    }
    Some(b)
}

fn case_du_plan(b: &Banc, plan: usize) -> usize {
    b.app.board_layout().cards.iter().position(|c| c.panel == plan).expect("la case du plan")
}

/// Le cadre B, posé par Ctrl + Maj + glisser depuis le milieu de la case, décalé de (`dx`, `dy`) largeurs de case.
fn poser_b(b: &mut Banc, plan: usize, dx: f32, dy: f32) {
    let k = case_du_plan(b, plan);
    let r = b.app.vignettes.xf.rect(b.app.board_layout().cards[k].thumb);
    let d = vec2(dx, dy) * r.width();
    ctrl_glisser(b, r.center(), &pas(r.center(), d, 8), r.center() + d, true);
    assert_eq!(b.app.doc.panels[plan].camera.len(), 2, "le cadre B est posé");
    b.chauffer(4);
    b.attendre_vignettes();
}

/// Les cadres du plan à l'écran.
fn cadres_ecran(b: &Banc, plan: usize) -> Vec<Rect> {
    let l = b.app.board_layout();
    let (quads, _) = b.app.cadres_sur_la_planche(&l, case_du_plan(b, plan));
    let xf = b.app.vignettes.xf;
    quads.iter().map(|q| Rect::from_points(&q.iter().map(|p| xf.to_screen(*p)).collect::<Vec<_>>())).collect()
}

/// Un point de l'écran des Vignettes, sur la feuille du plan.
fn sur_la_feuille(b: &Banc, plan: usize, p: Pos2) -> Pos2 {
    let l = b.app.board_layout();
    let aff = crate::horschamp::aff_de_la_case(&b.app.doc, &l, case_du_plan(b, plan)).expect("caméra droite");
    let q = b.app.vignettes.xf.to_board(p);
    pos2((q.x - aff.tx) / aff.a, (q.y - aff.ty) / aff.a)
}

/// Attend que le trait de `de` à `a` soit entier à l'écran (les effets se calculent en fond) ; la dernière image lue et son relevé.
fn image_du_trait(b: &mut Banc, de: Pos2, a: Pos2) -> (Vec<u8>, Releve) {
    let debut = std::time::Instant::now();
    loop {
        b.image(Vec::new(), 1.0 / 60.0);
        let rgba = b.capturer().expect("carte graphique");
        let r = relever(&rgba, de, a);
        if r.trou <= TROU_PERMIS || debut.elapsed() > std::time::Duration::from_secs(6) {
            return (rgba, r);
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
}

/// Montre le plan au Storyboard, à sa dernière image, assez loin pour voir tout le trait.
fn au_storyboard(b: &mut Banc, plan: usize) {
    b.app.set_espace(crate::workspace::Espace::Storyboard);
    b.app.board_view = crate::app::BoardView::Scene;
    b.app.settings.passe_partout_actif = false;
    b.app.select_panel(plan);
    b.app.cur_frame = b.app.frame_count() - 1;
    b.chauffer(4);
    let r = b.app.canvas_rect;
    let [w, h] = [b.app.doc.width as f32, b.app.doc.height as f32];
    b.app.view.fit(r, vec2(w, h) * 3.0);
    b.chauffer(4);
}

fn ecran_du_storyboard(b: &Banc, p: Pos2) -> Pos2 {
    b.app.view.to_screen(b.app.canvas_rect, b.app.canvas_size(), p)
}

/// Le trait vu à l'écran : un filet de cadre peut passer dessus (quelques pixels) ; rien de plus.
fn entier(b: &mut Banc, de: Pos2, a: Pos2, quoi: &str) -> Vec<u8> {
    let (rgba, r) = image_du_trait(b, de, a);
    sauver(&rgba, &quoi.replace(' ', "-"));
    println!("TRAIT {quoi} : {r:?}");
    assert!(r.trou <= TROU_PERMIS, "{quoi} : un trou de {} pixels à {:?} (trait de {} pixels)", r.trou, r.ou, r.longueur);
    rgba
}

/// Plan 6, cadre B décalé de (`dx`, `dy`) largeurs de case, un long trait de l'intérieur de A à la partie la plus lointaine de B.
struct Scene {
    b: Banc,
    de: Pos2,
    a: Pos2,
    feuille: (Pos2, Pos2),
}

fn scene(bitmap: bool, tremble: bool, dx: f32, dy: f32) -> Option<Scene> {
    let mut b = planche_de_12(bitmap, tremble)?;
    poser_b(&mut b, PLAN, dx, dy);
    // (Le cadre B vu en entier : le stylet ne dessine pas hors du panneau.)
    let (vue, tout) = (b.app.vignettes.view, cadres_ecran(&b, PLAN).iter().fold(Rect::NOTHING, |r, c| r.union(*c)));
    b.app.board_pan += vec2((vue.left() + 12.0 - tout.left()).max(0.0), (vue.top() + 12.0 - tout.top()).max(0.0));
    b.chauffer(4);
    let tout = cadres_ecran(&b, PLAN).iter().fold(Rect::NOTHING, |r, c| r.union(*c));
    assert!(vue.contains_rect(tout), "les cadres sont dans la vue : {tout:?} {vue:?}");
    let (de, a) = trait_a_travers(&b, PLAN, dx, dy);
    let feuille =(sur_la_feuille(&b, PLAN, de), sur_la_feuille(&b, PLAN, a));
    tracer(&mut b, de, a);
    repos(&mut b);
    Some(Scene { b, de, a, feuille })
}

fn trait_a_travers(b: &Banc, plan: usize, dx: f32, dy: f32) -> (Pos2, Pos2) {
    let c = cadres_ecran(b, plan);
    let (ra, rb) = (c[0], *c.last().unwrap());
    // Du côté de A qui s'éloigne du mouvement, au bout de B dans le sens du mouvement. (À côté de la flèche A vers B, qui se peint par-dessus le dessin.)
    let part = |d: f32, loin: f32| if d >= 0.0 { loin } else { 1.0 - loin };
    let de = pos2(ra.left() + ra.width() * part(dx, 0.2), ra.top() + ra.height() * part(dy, 0.6));
    let a = pos2(rb.left() + rb.width() * part(dx, 0.85), rb.top() + rb.height() * part(dy, 0.85));
    (de, a)
}

/// Rien n'a bougé dans la donnée : le trait va sur la feuille jusqu'où il a été tracé.
fn la_donnee_va_jusqu_au_bout(b: &Banc, plan: usize, fin: Pos2, bitmap: bool) {
    let d = &b.app.doc;
    let id = d.panels[plan].layers[0].frames.iter().flatten().next().copied().expect("un dessin");
    let c = d.cels.get(&id).expect("le dessin");
    if !bitmap {
        let r = c.vector.as_ref().expect("vectoriel").elements.iter().fold(Rect::NOTHING, |r, e| r.union(e.bounds()));
        assert!(r.expand(30.0).contains(fin), "les points du trait vont jusqu'à {fin:?} : {r:?}");
    }
    let (x, y) = (fin.x as i64, fin.y as i64);
    let encre = (-30..=30).any(|j| (-30..=30).any(|i| c.raster.get_i(x + i, y + j).a() > 0));
    assert!(encre, "les pixels du dessin vont jusqu'à {fin:?}");
}

/// Ce que le réalisateur voit : des traits coupés au repos, un nouveau trait dans la zone les fait réapparaître, le lever du stylet les recoupe.
#[test]
fn a_stroke_stays_whole_at_rest_while_another_is_drawn_and_after_it_is_released() {
    let Some(Scene { mut b, de, a, feuille }) = scene(false, true, 1.17, 0.5) else { return println!("pas de carte graphique") };
    la_donnee_va_jusqu_au_bout(&b, PLAN, feuille.1, false);
    entier(&mut b, de, a, "sequence repos");
    let n_avant = b.app.history.undo_len();
    // Un nouveau trait dans la zone, le stylet baissé.
    let c = cadres_ecran(&b, PLAN);
    let (p0, p1) = (pos2(c[0].right() - 8.0, c[0].bottom() - 6.0), pos2(c[1].right() - 8.0, c[1].bottom() - 8.0));
    let rejeux = crate::vector::replay_count();
    let chemin: Vec<Pos2> = (0..=20).map(|k| p0 + (p1 - p0) * (k as f32 / 20.0)).collect();
    b.image(vec![Event::PointerMoved(chemin[0])], 1.0 / 60.0);
    b.image(vec![appui(chemin[0], true, Modifiers::NONE)], 1.0 / 60.0);
    for q in &chemin[1..] {
        b.image(vec![Event::PointerMoved(*q)], 1.0 / 120.0);
    }
    assert_eq!(crate::vector::replay_count(), rejeux, "aucun rejeu du moteur sur le fil de l'interface pendant le geste");
    let geste = b.capturer().unwrap();
    sauver(&geste, "sequence-geste");
    let r = relever(&geste, de, a);
    assert!(r.trou <= TROU_PERMIS, "le stylet baissé : {r:?}");
    // Le stylet levé.
    b.image(vec![appui(*chemin.last().unwrap(), false, Modifiers::NONE)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    let lacher = b.capturer().unwrap();
    sauver(&lacher, "sequence-lacher");
    let r = relever(&lacher, de, a);
    assert!(r.trou <= TROU_PERMIS, "juste après le lâcher : {r:?}");
    repos(&mut b);
    // Ni l'ancien ni le nouveau ne se coupent, et le trait est une seule étape.
    entier(&mut b, de, a, "sequence apres");
    entier(&mut b, p0, p1, "sequence nouveau");
    assert_eq!(b.app.history.undo_len(), n_avant + 1);
}

#[test]
fn in_every_direction_the_board_and_the_storyboard_show_the_same_whole_stroke() {
    for (dx, dy) in [(1.17, 0.5), (2.2, 0.1), (-1.17, 0.3), (0.3, -1.1), (0.2, 1.2)] {
        for bitmap in [false, true] {
            for tremble in [false, true] {
                let nom = format!("{dx} {dy} {} {}", if bitmap { "bitmap" } else { "vecteur" }, if tremble { "tremble" } else { "net" });
                let Some(Scene { mut b, de, a, feuille }) = scene(bitmap, tremble, dx, dy) else { return println!("pas de carte graphique") };
                la_donnee_va_jusqu_au_bout(&b, PLAN, feuille.1, bitmap);
                entier(&mut b, de, a, &format!("vignettes {nom}"));
                // Le plan n'est plus sélectionné : sa case n'est plus vive, elle est la tuile.
                b.app.select_panel(0);
                b.chauffer(4);
                repos(&mut b);
                entier(&mut b, de, a, &format!("autre plan {nom}"));
                au_storyboard(&mut b, PLAN);
                let (e0, e1) = (ecran_du_storyboard(&b, feuille.0), ecran_du_storyboard(&b, feuille.1));
                assert!(b.app.canvas_rect.contains(e0) && b.app.canvas_rect.contains(e1), "{nom} : le trait est dans la vue : {e0:?} {e1:?}");
                entier(&mut b, e0, e1, &format!("storyboard {nom}"));
            }
        }
    }
}

/// Son réglage tel quel, « Animer à l'arrêt » allumé : le trait reste entier à chaque variante qui passe.
#[test]
fn with_the_tremble_animated_at_rest_every_image_shows_the_whole_stroke() {
    let Some(Scene { mut b, de, a, feuille }) = scene(false, true, 1.17, 0.5) else { return println!("pas de carte graphique") };
    b.app.settings.tremblement_defaut.repos = true;
    b.chauffer(2);
    entier(&mut b, de, a, "repos anime vignettes");
    au_storyboard(&mut b, PLAN);
    let (e0, e1) = (ecran_du_storyboard(&b, feuille.0), ecran_du_storyboard(&b, feuille.1));
    entier(&mut b, e0, e1, "repos anime storyboard");
    let mut pire = 0;
    for _ in 0..30 {
        b.image(Vec::new(), 1.0 / 24.0);
        std::thread::sleep(std::time::Duration::from_millis(40));
        pire = pire.max(relever(&b.capturer().unwrap(), e0, e1).trou);
    }
    assert!(pire <= TROU_PERMIS, "un trou de {pire} pixels pendant que le trait tremble");
}

#[test]
fn a_third_frame_far_away_keeps_the_stroke_too() {
    let Some(mut b) = planche_de_12(false, true) else { return println!("pas de carte graphique") };
    poser_b(&mut b, PLAN, 0.9, 0.2);
    let doc = &mut b.app.doc;
    let (ka, kb) = (doc.panels[PLAN].camera[0], doc.panels[PLAN].camera[1]);
    crate::camera::set_key(&mut doc.panels[PLAN].camera, 12, kb.cam);
    crate::camera::set_key(&mut doc.panels[PLAN].camera, kb.frame, ka.cam.shifted(1920.0 * 1.8, 1080.0 * 0.6));
    assert_eq!(doc.panels[PLAN].camera.len(), 3);
    b.chauffer(6);
    b.attendre_vignettes();
    let (de, a) = trait_a_travers(&b, PLAN, 1.0, 1.0);
    tracer(&mut b, de, a);
    repos(&mut b);
    entier(&mut b, de, a, "cadre C");
}

#[test]
fn saving_and_reopening_give_the_same_whole_stroke() {
    for bitmap in [false, true] {
        let Some(Scene { mut b, de, a, feuille }) = scene(bitmap, true, 1.17, 0.5) else { return println!("pas de carte graphique") };
        entier(&mut b, de, a, "avant enregistrement");
        let dossier = std::env::temp_dir().join(format!("shot-test-camera-traits-{bitmap}-{}-{:?}", std::process::id(), std::thread::current().id()));
        std::fs::remove_dir_all(&dossier).ok();
        std::fs::create_dir_all(&dossier).unwrap();
        let manifeste = dossier.join("P.shot");
        crate::project::save(&b.app.doc, &manifeste, &mut Default::default()).expect("enregistrement");
        let relu = crate::project::load(&manifeste).expect("réouverture");
        let (zoom, pan) = (b.app.board_zoom, b.app.board_pan);
        let Some(mut c) = planche(relu.doc, 5, 1.0) else { return };
        c.app.settings.tremblement_defaut = b.app.settings.tremblement_defaut;
        c.app.board_zoom = zoom;
        c.app.board_pan = pan;
        c.chauffer(4);
        repos(&mut c);
        la_donnee_va_jusqu_au_bout(&c, PLAN, feuille.1, bitmap);
        entier(&mut c, de, a, "apres reouverture");
        std::fs::remove_dir_all(&dossier).ok();
    }
}

/// Le film et les images montrent ce que le cadre voit : en B, le trait jusqu'à sa fin, en coordonnées de l'image.
#[test]
fn the_film_frame_shows_what_frame_b_sees() {
    for bitmap in [false, true] {
        let Some(Scene { b, feuille, .. }) = scene(bitmap, true, 1.17, 0.5) else { return println!("pas de carte graphique") };
        let doc = &b.app.doc;
        let f = doc.panels[PLAN].frame_count() - 1;
        let cam = doc.panel_camera(PLAN, f);
        let c0 = cam.corners(doc.frame)[0];
        let snap = crate::exportrender::snapshot_of(doc, &[PLAN]);
        let rendu = crate::exportrender::Rendu::new(snap, crate::exportrender::Papier::Feuille, false, Vec::new(), Default::default());
        let img = rendu.image(PLAN, f).expect("l'image");
        let [w, h] = doc.frame;
        if let Some(d) = std::env::var_os("FEANOR_CAPTURE_DIR") {
            let rgba: Vec<u8> = img.iter().flat_map(|c| c.to_array()).collect();
            let _ = image::save_buffer(std::path::PathBuf::from(d).join(format!("film-{bitmap}.png")), &rgba, w as u32, h as u32, image::ColorType::Rgba8);
        }
        // Le long du trait, dans le cadre B : de l'encre partout (le trou le plus long, en pixels du film).
        let (mut trou, mut courant, mut vus) = (0, 0, 0);
        let n = (feuille.1 - feuille.0).length() as i32;
        let d = (feuille.1 - feuille.0).normalized();
        for k in 40..n - 40 {
            let p = feuille.0 + d * k as f32 - c0.to_vec2();
            if p.x < 12.0 || p.y < 12.0 || p.x > w as f32 - 12.0 || p.y > h as f32 - 12.0 {
                continue;
            }
            vus += 1;
            let encre = (-14..=14).any(|j| (-14..=14).any(|i| {
                let c: Color32 = img[(p.y as i32 + j) as usize * w + (p.x as i32 + i) as usize];
                c.r() as i32 - c.g() as i32 > 40
            }));
            courant = if encre { 0 } else { courant + 1 };
            trou = trou.max(courant);
        }
        println!("TRAIT film bitmap {bitmap} : {vus} pixels suivis, trou {trou}");
        assert!(vus > 500, "le trait passe dans le cadre B : {vus}");
        assert!(trou <= 4, "bitmap {bitmap} : le film a un trou de {trou} pixels sur le trait");
    }
}

/// Le trait d'une scène, avec ou sans un flou posé sur le calque.
fn scene_floue(flou: bool) -> Option<Scene> {
    let mut s = scene(false, false, 1.17, 0.5)?;
    if flou {
        let mut e = crate::fx::Effect::new(crate::fx::EffectKind::GaussianBlur);
        e.values = vec![14.0];
        for l in s.b.app.doc.panels[PLAN].layers.iter_mut() {
            l.fx.push(e.clone());
        }
    }
    Some(s)
}

/// À `decale` pixels du milieu du trait, dans le cadre B et au-delà de la feuille : combien de points portent de l'encre, sur combien de points lus.
fn halo_du_film(s: &Scene, decale: f32) -> (usize, usize) {
    let doc = &s.b.app.doc;
    let f = doc.panels[PLAN].frame_count() - 1;
    let c0 = doc.panel_camera(PLAN, f).corners(doc.frame)[0];
    let snap = crate::exportrender::snapshot_of(doc, &[PLAN]);
    // (Le film ne connaît pas l'écran : aucun résultat du cache commun, aucun repli.)
    let rendu = crate::exportrender::Rendu::new(snap, crate::exportrender::Papier::Feuille, false, Vec::new(), Default::default());
    let img = rendu.image(PLAN, f).expect("l'image");
    let [w, h] = doc.frame;
    let d = (s.feuille.1 - s.feuille.0).normalized();
    let n = vec2(-d.y, d.x);
    let (mut vus, mut encre) = (0, 0);
    for k in 0..(s.feuille.1 - s.feuille.0).length() as i32 {
        let m = s.feuille.0 + d * k as f32;
        if m.x <= doc.width as f32 + 40.0 || m.x >= s.feuille.1.x - 40.0 {
            continue;
        }
        let p = m + n * decale - c0.to_vec2();
        if p.x < 4.0 || p.y < 4.0 || p.x > w as f32 - 4.0 || p.y > h as f32 - 4.0 {
            continue;
        }
        vus += 1;
        let c = img[p.y as usize * w + p.x as usize];
        encre += usize::from(c.r() as i32 - c.g() as i32 > 6);
    }
    (encre, vus)
}

/// Le film montre l'effet complet d'un dessin qui passe le bord, au-delà de la feuille : pas le repli sans effet de l'écran.
#[test]
fn the_film_shows_the_full_effect_beyond_the_sheet() {
    let Some(net) = scene_floue(false) else { return println!("pas de carte graphique") };
    let Some(flou) = scene_floue(true) else { return };
    let (a, va) = halo_du_film(&net, 14.0);
    let (b, vb) = halo_du_film(&flou, 14.0);
    println!("FILM à quatorze pixels du trait, au-delà de la feuille : net {a} sur {va}, flou {b} sur {vb}");
    assert!(vb > 200 && b * 100 >= vb * 90, "le film montre le flou : {b} sur {vb}");
    assert!(a * 100 <= va * 5, "sans flou, rien à cette distance : {a} sur {va}");
}

/// La planche exportée, de même : le flou au-delà de la feuille.
#[test]
fn the_board_export_shows_the_full_effect_beyond_the_sheet() {
    let Some(Scene { mut b, de, a, feuille }) = scene_floue(true) else { return println!("pas de carte graphique") };
    let xf = b.app.vignettes.xf;
    let (bd, ba) = (xf.to_board(de), xf.to_board(a));
    // Points de la planche par pixel de la feuille.
    let case = (ba - bd).length() / (feuille.1 - feuille.0).length();
    let opt = crate::planche::Options::default();
    let img = crate::traits_continus_tests::exporter(&mut b, &opt, "effet-debord");
    let opt = b.app.planche_options(&opt);
    let plans: Vec<usize> = (0..b.app.doc.panels.len()).collect();
    let p = crate::planche::construire(&b.app.doc, &plans, &b.app.layout.board_settings(), &opt, &b.app.doc.annotations);
    let (s, ..) = crate::planche::dimensions(p.cadre, opt.resolution);
    let vers = |q: Pos2| pos2((q.x - p.cadre.left()) * s, (q.y - p.cadre.top()) * s);
    let (i0, i1) = (vers(bd), vers(ba));
    let d = (i1 - i0).normalized();
    let n = vec2(-d.y, d.x);
    let long = (i1 - i0).length() as i32;
    // Dans la dernière moitié du trait (au-delà de la feuille, dans le cadre B), à quatorze pixels de la feuille : seul le flou y arrive.
    let decale = 14.0 * case * s;
    let (mut vus, mut encre) = (0, 0);
    for k in long / 2..long - (8.0 * s) as i32 {
        let c = i0 + d * k as f32 + n * decale;
        if c.x < 0.0 || c.y < 0.0 || c.x as u32 >= img.width() || c.y as u32 >= img.height() {
            continue;
        }
        vus += 1;
        let v = img.get_pixel(c.x as u32, c.y as u32);
        encre += usize::from(v[0] as i32 - v[1] as i32 > 2);
    }
    println!("PLANCHE à côté du trait, dernière moitié : {encre} sur {vus}, {case} points de la planche par pixel de la feuille, échelle {s}");
    assert!(vus > 20 && encre * 100 >= vus * 50, "la planche montre le flou : {encre} sur {vus}");
}

/// Le défaut trouvé le 2026-11-07 (`docs/FIABILITE.md`, section 20) : les tuiles de l'export de la planche se rendaient en mode doux, comme les vignettes de l'écran. Un effet que la réserve
/// commune n'avait pas encore (jamais montré, lâché, le fil de fond en retard) partait **sans effet dans le fichier**. Ici, rien entre le geste et l'export : ni image d'écran,
/// ni fil de fond qui aurait eu le temps ; l'ancien code rendait la tuile du flou identique à celle du trait net, à chaque fois.
#[test]
fn the_tiles_of_the_board_export_are_exact_whatever_the_shared_cache_holds() {
    let tuile = |s: &Scene| {
        let opt = s.b.app.planche_options(&crate::planche::Options::default());
        let plans: Vec<usize> = (0..s.b.app.doc.panels.len()).collect();
        let p = crate::planche::construire(&s.b.app.doc, &plans, &s.b.app.layout.board_settings(), &opt, &s.b.app.doc.annotations);
        let (e, ..) = crate::planche::dimensions(p.cadre, opt.resolution);
        let k = p.cartes.iter().position(|c| c.mouvement.is_some() && c.plan_reel == PLAN).expect("la case à cadres");
        s.b.app.planche_tuiles_pour_essai(&p, e, &opt).swap_remove(k).expect("sa tuile")
    };
    // (Les pixels où l'encre du trait, rouge, se voit, si peu que ce soit.)
    let encre = |t: &crate::planche::TuileImage| t.rvba.chunks_exact(4).filter(|c| c[3] != 0 && c[0] as i32 - c[1] as i32 > 2).count();
    let Some(net) = scene_floue(false) else { return println!("pas de carte graphique") };
    let Some(flou) = scene_floue(true) else { return };
    crate::fx::forget_all();
    let (a, b) = (tuile(&net), tuile(&flou));
    println!("TUILE encre : net {}, flou {}", encre(&a), encre(&b));
    // (Mesuré : 1 448 pixels nets, 2 539 avec le flou ; l'ancien code rendait les mêmes pixels dans les deux cas.)
    assert!(encre(&b) * 2 > encre(&a) * 3, "la tuile de la planche montre le flou, pas le trait net : net {}, flou {}", encre(&a), encre(&b));
    // L'écran l'a calculé ou non : le fichier ne change pas d'un octet.
    let mut flou = flou;
    let c = tuile(&flou);
    flou.b.chauffer(4);
    repos(&mut flou.b);
    let d = tuile(&flou);
    assert!(b.rvba == c.rvba && b.rvba == d.rvba, "le même fichier, quoi que la réserve commune tienne");
}

/// La planche exportée : au-delà de la verticale où la feuille finit, le trait est là.
#[test]
fn the_board_export_keeps_the_stroke_beyond_the_sheet() {
    let Some(Scene { mut b, de, a, .. }) = scene(false, true, 1.17, 0.5) else { return println!("pas de carte graphique") };
    let xf = b.app.vignettes.xf;
    let (bd, ba) = (xf.to_board(de), xf.to_board(a));
    let opt = crate::planche::Options::default();
    let img = crate::traits_continus_tests::exporter(&mut b, &opt, "camera-traits");
    if let Some(d) = std::env::var_os("FEANOR_CAPTURE_DIR") {
        let _ = img.save(std::path::PathBuf::from(d).join("planche-camera-traits.png"));
    }
    let opt = b.app.planche_options(&opt);
    let plans: Vec<usize> = (0..b.app.doc.panels.len()).collect();
    let p = crate::planche::construire(&b.app.doc, &plans, &b.app.layout.board_settings(), &opt, &b.app.doc.annotations);
    let (s, ..) = crate::planche::dimensions(p.cadre, opt.resolution);
    let vers = |q: Pos2| pos2((q.x - p.cadre.left()) * s, (q.y - p.cadre.top()) * s);
    let (i0, i1) = (vers(bd), vers(ba));
    let long = (i1 - i0).length() as i32;
    let (mut trou, mut courant) = (0, 0);
    for k in (8.0 * s) as i32..long - (8.0 * s) as i32 {
        let c = i0 + (i1 - i0) * (k as f32 / long as f32);
        let encre = (-8..=8).any(|w| {
            let (x, y) = (c.x as i32, c.y as i32 + (w as f32 * s.max(1.0)) as i32);
            x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() && {
                let v = img.get_pixel(x as u32, y as u32);
                v[0] as i32 - v[1] as i32 >= 40
            }
        });
        courant = if encre { 0 } else { courant + 1 };
        trou = trou.max(courant);
    }
    println!("TRAIT planche : trou {trou}, échelle {s}");
    assert!(trou as f32 <= 8.0 * s.max(1.0), "un trou de {trou} pixels à l'export de la planche (échelle {s})");
}

// ------------------------------------------------------------------------------ les effets et ce qui dépasse de la feuille

/// Une bande rouge de la feuille `w` × `h` qui la traverse à droite, jusqu'à 200 pixels au-delà.
fn bande(w: usize, h: usize) -> crate::raster::Raster {
    let mut r = crate::raster::Raster::new(w, h);
    let rouge = Color32::from_rgb(200, 30, 40);
    let tile = crate::raster::TILE as i32;
    for x in (w as i32 - 300)..(w as i32 + 200) {
        for y in 100..112i32 {
            let (tx, ty) = (x.div_euclid(tile), y.div_euclid(tile));
            let id = r.id_of(tx, ty);
            r.tile_mut(id)[(y.rem_euclid(tile) * tile + x.rem_euclid(tile)) as usize] = rouge;
        }
    }
    r
}

/// Chaque effet garde ce qui dépasse. Loin de ce qui dépasse, la feuille ne bouge pas ; près de lui, elle voit des deux côtés (partie 121).
#[test]
fn every_effect_keeps_what_is_beyond_the_sheet_and_leaves_the_sheet_as_it_was() {
    let (w, h) = (640, 320);
    let r = bande(w, h);
    let mut feuille_seule = r.clone();
    feuille_seule.outer.clear();
    for kind in crate::fx::EffectKind::ALL {
        if kind.vector_only() {
            continue;
        }
        let e = crate::fx::Effect::new(kind);
        let steps = [crate::fx::FxStep { kind, values: e.values.clone() }];
        let out = crate::fx::apply(&r, &steps);
        let seule = crate::fx::apply(&feuille_seule, &steps);
        assert!(out.has_beyond(), "{kind:?} : rien ne reste hors de la feuille");
        // (Un effet qui lit ses voisins : la plus grande portée des réglages d'origine est d'environ trois tuiles, au-delà la feuille n'a rien à lire de ce qui dépasse.
        // Un effet pixel à pixel n'a rien à lire : la feuille entière est la même.)
        let loin = if kind.lit_ses_voisins() { w - 3 * crate::raster::TILE - 16 } else { w };
        for (k, (a, b)) in out.tiles.iter().zip(&seule.tiles).enumerate() {
            let (ox, oy, tw, th) = out.tile_bounds(k);
            for y in 0..th {
                for x in 0..tw {
                    if ox + x >= loin {
                        continue;
                    }
                    let (pa, pb) = (a.as_ref().map_or(Color32::TRANSPARENT, |t| t[y * crate::raster::TILE + x]), b.as_ref().map_or(Color32::TRANSPARENT, |t| t[y * crate::raster::TILE + x]));
                    assert_eq!(pa, pb, "{kind:?} : la feuille change en ({}, {})", ox + x, oy + y);
                }
            }
        }
    }
}

/// Le trait ne saute pas au bord de la feuille : hors d'elle, le tremblement est celui qu'aurait une feuille plus grande.
#[test]
fn the_tremble_continues_beyond_the_sheet_with_the_same_field() {
    let (w, h) = (640, 320);
    let r = bande(w, h);
    let values = crate::tremblement::Defaut { actif: true, intensite: 3.0, ..Default::default() }.values();
    let mut valeurs = values;
    valeurs.push(0.0);
    let steps = [crate::fx::FxStep { kind: crate::fx::EffectKind::Boil, values: valeurs.clone() }];
    let out = crate::fx::apply(&r, &steps);
    // La même encre sur une feuille assez grande pour tout tenir.
    let mut grande = crate::raster::Raster::new(1280, 320);
    for x in 340..840usize {
        for y in 100..112usize {
            let t = grande.tile_index_of(x, y);
            grande.tile_mut(t)[(y % crate::raster::TILE) * crate::raster::TILE + x % crate::raster::TILE] = Color32::from_rgb(200, 30, 40);
        }
    }
    let attendu = crate::tremblement::boil(&grande, &valeurs, 0);
    let mut comptes = 0;
    for x in (w as i64 + 8)..(w as i64 + 190) {
        for y in 90..122 {
            assert_eq!(out.get_i(x, y), attendu.get_i(x, y), "({x}, {y})");
            comptes += usize::from(out.get_i(x, y).a() > 0);
        }
    }
    assert!(comptes > 1000, "de l'encre tremblée hors de la feuille : {comptes}");
}

/// Un effet du trait qui transforme chaque trait pose son image sur la feuille : hors d'elle, le trait reste tel quel, il ne disparaît pas.
#[test]
fn a_stroke_effect_on_a_drawing_beyond_the_sheet_does_not_cut_it() {
    use crate::vector::{Element, Trace, VPoint, VectorDrawing};
    let (w, h) = (640, 320);
    let points: Vec<VPoint> = (0..120).map(|k| VPoint { x: 300.0 + k as f32 * 6.0, y: 150.0, r: 3.0 }).collect();
    let v = VectorDrawing { elements: vec![Element::Stroke { points, color: [200, 30, 40], opacity: 1.0, hardness: 1.0, brush: None, trace: Trace::default() }] };
    let raster = v.render(w, h);
    assert!(raster.has_beyond());
    let e = crate::fx::Effect::new(crate::fx::EffectKind::StrokeWave);
    let out = crate::fx::apply_with(&raster, Some(&v), &[crate::fx::FxStep { kind: crate::fx::EffectKind::StrokeWave, values: e.values.clone() }], 0);
    let encre = (140..160).any(|y| out.get_i(900, y).a() > 0);
    assert!(encre, "le trait au-delà de la feuille reste");
}

/// Un effet de couleur traite aussi ce qui dépasse : pas de tuile brute (le rouge du dessin) hors de la feuille.
#[test]
fn a_colour_effect_treats_what_is_beyond_the_sheet_too() {
    let r = bande(640, 320);
    let e = crate::fx::Effect::new(crate::fx::EffectKind::BlackWhite);
    let out = crate::fx::apply(&r, &[crate::fx::FxStep { kind: crate::fx::EffectKind::BlackWhite, values: e.values.clone() }]);
    let c = out.get_i(760, 105);
    assert!(c.a() > 0 && (c.r() as i32 - c.g() as i32).abs() < 8, "gris, et non le rouge d'origine : {c:?}");
}

// ------------------------------------------------------------------------------ l'aperçu réduit

/// Défiler au Storyboard, vu de loin : l'aperçu réduit (`fx::draft_level`) ne recoupe le trait à aucune image, effets sur la carte graphique ou non.
#[test]
fn scrubbing_far_away_never_cuts_the_stroke_again() {
    for sur_la_carte in [true, false] {
        let Some(Scene { mut b, feuille, .. }) = scene(false, true, 1.17, 0.5) else { return println!("pas de carte graphique") };
        au_storyboard(&mut b, PLAN);
        if let Some(g) = &b.app.gpu {
            g.set_effects_gpu(sur_la_carte);
        }
        let (e0, e1) = (ecran_du_storyboard(&b, feuille.0), ecran_du_storyboard(&b, feuille.1));
        assert!(crate::fxproxy::level_for_zoom(b.app.view.zoom) > 0, "vu de loin : l'aperçu est réduit");
        entier(&mut b, e0, e1, "avant de defiler");
        let mut pire = 0;
        for k in 0..60 {
            b.app.cur_frame = k % b.app.frame_count();
            b.image(Vec::new(), 1.0 / 60.0);
            let r = relever(&b.capturer().unwrap(), e0, e1);
            if r.trou > pire {
                pire = r.trou;
                sauver(&b.capturer().unwrap(), &format!("defile-{sur_la_carte}"));
            }
        }
        println!("TRAIT défilement, effets sur la carte {sur_la_carte} : pire trou {pire}");
        assert!(pire <= TROU_PERMIS, "effets sur la carte {sur_la_carte} : un trou de {pire} pixels en défilant");
    }
}

/// La tuile qui se refait pendant que l'aperçu réduit du même dessin existe : elle ne le prend pas pour remplaçant, il n'a rien hors de la feuille.
#[test]
fn a_thumbnail_waiting_for_its_effect_does_not_use_a_reduced_preview_without_the_overflow() {
    let Some(Scene { b, feuille, .. }) = scene(false, true, 1.17, 0.5) else { return println!("pas de carte graphique") };
    let doc = &b.app.doc;
    let f = doc.panels[PLAN].frame_count() - 1;
    // Une autre variante pour le dérivé réduit et pour la vignette : seul le remplaçant est en jeu.
    let plan = crate::render::panel_plan_at(doc, PLAN, f, Some(doc.paper));
    let item = plan.items.iter().find(|i| !i.fx.is_empty()).expect("le tremblement");
    let debut = std::time::Instant::now();
    while !crate::fx::display(doc, item, 1).is_some_and(|d| d.ready && d.cel.raster.width < doc.width) {
        assert!(debut.elapsed() < std::time::Duration::from_secs(20), "l'aperçu réduit arrive");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let autre = crate::render::panel_plan_at(doc, PLAN, 3, Some(doc.paper));
    let cam = doc.panel_camera(PLAN, f);
    let (img, _) = crate::fxcache::soft_scope(|| crate::render::thumbnail(doc, &autre, &cam, 960, 540));
    let c0 = cam.corners(doc.frame)[0];
    // Le bout du trait, dans le cadre B, au-delà du bord de la feuille.
    let d = (feuille.1 - feuille.0).normalized();
    let (mut vus, mut encre) = (0, 0);
    for k in 0..((feuille.1 - feuille.0).length() as i32) {
        let p = feuille.0 + d * k as f32;
        if p.x <= doc.width as f32 + 30.0 {
            continue;
        }
        let q = (p - c0.to_vec2()) * 0.5;
        if q.x < 8.0 || q.y < 8.0 || q.x > 950.0 || q.y > 530.0 {
            continue;
        }
        vus += 1;
        encre += usize::from((-8..=8).any(|j| {
            let c = img.pixels[(q.y as i32 + j) as usize * 960 + q.x as usize];
            c.r() as i32 - c.g() as i32 > 30
        }));
    }
    println!("TRAIT vignette en attente : {encre} sur {vus}");
    assert!(vus > 200 && encre * 100 >= vus * 98, "le trait au-delà de la feuille : {encre} pixels d'encre sur {vus}");
}

// ------------------------------------------------------------------------------ l'encoche au bord de la feuille

/// (colonnes de la feuille, le long du bord, où la bande perd de l'encre ; la plus grande perte de couverture, sur 255).
fn encoche(out: &crate::raster::Raster, w: i64, attendu: &crate::raster::Raster) -> (usize, i32) {
    let (mut colonnes, mut pire) = (0, 0);
    for x in (w - 8)..w {
        let somme = |r: &crate::raster::Raster| (70..142).map(|y| r.get_i(x, y).a() as i32).sum::<i32>();
        let perte = somme(attendu) - somme(out);
        colonnes += usize::from(perte > 255);
        pire = pire.max(perte);
    }
    (colonnes, pire)
}

fn grande_bande(w: usize) -> crate::raster::Raster {
    let mut grande = crate::raster::Raster::new(w * 2, 320);
    for x in (w - 300)..(w + 200) {
        for y in 100..112usize {
            let t = grande.tile_index_of(x, y);
            grande.tile_mut(t)[(y % crate::raster::TILE) * crate::raster::TILE + x % crate::raster::TILE] = Color32::from_rgb(200, 30, 40);
        }
    }
    grande
}

/// Le tremblement seul : côté feuille non plus, le trait n'a pas d'encoche à son bord, et un dessin qui reste dans la feuille ne change pas d'un pixel.
#[test]
fn the_tremble_leaves_no_notch_on_the_sheet_side() {
    let (w, h) = (640, 320);
    for intensite in [0.5f32, 3.0, crate::tremblement::MAX_INTENSITY] {
        // (Plusieurs graines : le champ ne pousse pas vers le bord partout.)
        let (mut colonnes, mut pire, mut avant) = (0, 0, 0);
        for graine in 1..=24 {
            let mut valeurs = crate::tremblement::Defaut { actif: true, intensite, graine, ..Default::default() }.values();
            valeurs.push(0.0);
            let steps = [crate::fx::FxStep { kind: crate::fx::EffectKind::Boil, values: valeurs.clone() }];
            let out = crate::fx::apply(&bande(w, h), &steps);
            let attendu = crate::tremblement::boil(&grande_bande(w), &valeurs, 0);
            let (c, p) = encoche(&out, w as i64, &attendu);
            colonnes = colonnes.max(c);
            pire = pire.max(p);
            avant = avant.max(encoche(&crate::tremblement::boil(&bande(w, h), &valeurs, 0), w as i64, &attendu).1);
            // Sans débord : la feuille seule, comme toujours.
            let mut seule = bande(w, h);
            seule.outer.clear();
            let (avec, ancien) = (crate::fx::apply(&seule, &steps), crate::tremblement::boil(&seule, &valeurs, 0));
            assert!(avec.tiles.iter().zip(&ancien.tiles).all(|(a, b)| match (a, b) {
                (Some(a), Some(b)) => a[..] == b[..],
                (None, None) => true,
                _ => false,
            }));
        }
        println!("ENCOCHE intensité {intensite} : perte de couverture sans la bande du bord {avant}, avec {pire} (une colonne pleine : 12 x 255 = 3060), {colonnes} colonnes");
        // (Quelques unités : l'arrondi du champ calculé depuis une autre origine.)
        assert!(colonnes == 0 && pire <= 8, "intensité {intensite} : {colonnes} colonnes, perte {pire}");
        assert!(avant > 20 * pire.max(1), "l'essai voit bien l'encoche du calcul de la feuille seule : {avant}");
    }
}

/// À l'écran, effets sur la carte graphique ou non : ce qui reste d'encre, colonne par colonne, autour du bord de la feuille, vu de près. (Mesure.)
#[test]
#[ignore]
fn mesure_encoche_a_l_ecran() {
    for intensite in [0.0f32, 0.5, 3.0] {
        for sur_la_carte in [true, false] {
            let Some(Scene { mut b, feuille, .. }) = scene(false, intensite > 0.0, 2.2, 0.1) else { return };
            b.app.settings.tremblement_defaut.intensite = intensite;
            au_storyboard(&mut b, PLAN);
            if let Some(g) = &b.app.gpu {
                g.set_effects_gpu(sur_la_carte);
                println!("ENCOCHE effets possibles sur la carte : {}", g.effects_supported());
            }
            // Le point où le trait passe le bord de la feuille, au milieu du canevas, grossi 4 fois.
            let w = b.app.doc.width as f32;
            let t = (w - feuille.0.x) / (feuille.1.x - feuille.0.x);
            let bord = feuille.0 + (feuille.1 - feuille.0) * t;
            b.app.view.zoom = 4.0;
            b.app.view.pan = vec2(0.0, 0.0);
            let s = ecran_du_storyboard(&b, bord);
            b.app.view.pan = b.app.canvas_rect.center() - s;
            b.chauffer(4);
            for _ in 0..120 {
                b.image(Vec::new(), 1.0 / 60.0);
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            let img = b.capturer().unwrap();
            sauver(&img, &format!("encoche-{intensite}-{sur_la_carte}"));
            let c = b.app.canvas_rect.center();
            let colonne = |x: i32| (-30..=30).map(|y| crate::traits_continus_tests::px(&img, x, c.y as i32 + y)).map(|v| (v[0] - v[1]).max(0)).sum::<i32>();
            let valeurs: Vec<i32> = (-24..=24).step_by(2).map(|d| colonne(c.x as i32 + d)).collect();
            println!("ENCOCHE ÉCRAN intensité {intensite} carte {sur_la_carte} : {valeurs:?}");
        }
    }
}

/// Sur la carte graphique, le trait qui passe le bord de la feuille n'a plus de colonne plus claire : le shader du tremblement lit la bordure d'encre (`gpufx::Bordure`).
/// Et la carte montre les mêmes pixels que le calcul de fond, à l'arrondi près. (La colonne faisait 4,5 % de moins à l'intensité 0,5, 24 % à l'intensité 3.)
#[test]
fn the_tremble_on_the_graphics_card_has_no_lighter_column_at_the_edge() {
    for intensite in [0.5f32, 3.0] {
        let mut par_voie = Vec::new();
        for sur_la_carte in [true, false] {
            let Some(Scene { mut b, feuille, .. }) = scene(false, true, 2.2, 0.1) else { return println!("pas de carte graphique") };
            b.app.settings.tremblement_defaut.intensite = intensite;
            au_storyboard(&mut b, PLAN);
            let Some(g) = &b.app.gpu else { return };
            if !g.effects_supported() {
                return println!("pas d'effets sur la carte");
            }
            g.set_effects_gpu(sur_la_carte);
            let w = b.app.doc.width as f32;
            let t = (w - feuille.0.x) / (feuille.1.x - feuille.0.x);
            let bord = feuille.0 + (feuille.1 - feuille.0) * t;
            b.app.view.zoom = 4.0;
            b.app.view.pan = vec2(0.0, 0.0);
            let s = ecran_du_storyboard(&b, bord);
            b.app.view.pan = b.app.canvas_rect.center() - s;
            b.chauffer(4);
            for _ in 0..120 {
                b.image(Vec::new(), 1.0 / 60.0);
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            let img = b.capturer().unwrap();
            sauver(&img, &format!("bord-{intensite}-{sur_la_carte}"));
            let c = b.app.canvas_rect.center();
            let colonne = |x: i32| (-30..=30).map(|y| crate::traits_continus_tests::px(&img, x, c.y as i32 + y)).map(|v| (v[0] - v[1]).max(0)).sum::<i32>();
            let valeurs: Vec<i32> = (-24..=24).step_by(2).map(|d| colonne(c.x as i32 + d)).collect();
            println!("COLONNE intensité {intensite} carte {sur_la_carte} : {valeurs:?}");
            par_voie.push(valeurs);
        }
        let ecart = par_voie[0].iter().zip(&par_voie[1]).map(|(a, b)| (a - b).abs() as f32 / (*b).max(1) as f32).fold(0.0, f32::max);
        println!("COLONNE intensité {intensite} : écart carte / calcul de fond {:.2} %", ecart * 100.0);
        assert!(ecart < 0.02, "intensité {intensite} : la carte s'écarte de {:.1} % du calcul de fond", ecart * 100.0);
    }
}

// ------------------------------------------------------------------------------ le coût

/// Un trait perdu très loin : temps et mémoire de chaque effet. (Mesure.)
#[test]
#[ignore]
fn mesure_le_cout_d_un_trait_perdu_tres_loin() {
    let (w, h) = (3840, 2160);
    let sur_la_feuille = {
        let mut s = bande(w, h);
        s.outer.clear();
        s
    };
    let tile = crate::raster::TILE as i32;
    // Un point perdu : le rectangle du hors-feuille va de la feuille jusqu'à lui.
    for loin in [8_000, 10_800, 30_000] {
        let mut r = bande(w, h);
        r.tile_mut(crate::raster::outer_id(loin / tile, loin / tile))[0] = Color32::from_rgb(200, 30, 40);
        for kind in [crate::fx::EffectKind::Boil, crate::fx::EffectKind::GaussianBlur, crate::fx::EffectKind::Glow, crate::fx::EffectKind::BlackWhite] {
            let e = crate::fx::Effect::new(kind);
            let mut values = e.values.clone();
            if kind == crate::fx::EffectKind::Boil {
                values[0] = 0.5;
                values.push(0.0);
            }
            let steps = [crate::fx::FxStep { kind, values }];
            let t = std::time::Instant::now();
            let _ = crate::fx::apply(&sur_la_feuille, &steps);
            let feuille_ms = t.elapsed().as_millis();
            let avant = crate::lourd_tests::memoire_pic();
            let t = std::time::Instant::now();
            let out = crate::fx::apply(&r, &steps);
            println!("COUT point à {loin} {kind:?} : feuille seule {feuille_ms} ms ; avec le point {} ms ; pic du processus {avant:.0} puis {:.0} Mo ; tuiles hors feuille {}", t.elapsed().as_millis(), crate::lourd_tests::memoire_pic(), out.outer.len());
        }
    }
}

// ------------------------------------------------------------------------------ à regarder (FEANOR_CAPTURE_DIR)

/// La suite de ce que voit le réalisateur, image par image : les mesures seules, sans jugement.
#[test]
#[ignore]
fn captures_la_sequence_du_stylet() {
    let Some(Scene { mut b, de, a, .. }) = scene(false, true, 1.17, 0.5) else { return };
    let (img, r) = image_du_trait(&mut b, de, a);
    sauver(&img, "seq-1-repos");
    println!("SEQ repos : trou {}", r.trou);
    let c = cadres_ecran(&b, PLAN);
    let (p0, p1) = (pos2(c[0].right() - 8.0, c[0].top() + 10.0), pos2(c[1].right() - 8.0, c[1].top() + 10.0));
    let chemin: Vec<Pos2> = (0..=20).map(|k| p0 + (p1 - p0) * (k as f32 / 20.0)).collect();
    b.image(vec![Event::PointerMoved(chemin[0])], 1.0 / 60.0);
    b.image(vec![appui(chemin[0], true, Modifiers::NONE)], 1.0 / 60.0);
    for q in &chemin[1..] {
        b.image(vec![Event::PointerMoved(*q)], 1.0 / 120.0);
    }
    let img = b.capturer().unwrap();
    sauver(&img, "seq-2-stylet-baisse");
    println!("SEQ stylet baissé : trou {}", relever(&img, de, a).trou);
    b.image(vec![appui(*chemin.last().unwrap(), false, Modifiers::NONE)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    let img = b.capturer().unwrap();
    println!("SEQ juste levé : trou {}", relever(&img, de, a).trou);
    repos(&mut b);
    for _ in 0..40 {
        b.image(Vec::new(), 1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    let img = b.capturer().unwrap();
    sauver(&img, "seq-3-repos-apres");
    println!("SEQ repos après : trou {}", relever(&img, de, a).trou);
}

/// À l'écran, l'onde (la pile va alors au calcul de fond) : le trait ne saute pas à la verticale où la feuille finit, une fois l'effet arrivé du fil de fond.
#[test]
fn on_screen_a_wave_does_not_break_the_stroke_at_the_edge_of_the_sheet() {
    use crate::fx::{Effect, EffectKind as K};
    let Some(Scene { mut b, feuille, .. }) = scene(false, false, 1.17, 0.5) else { return println!("pas de carte graphique") };
    let mut e = Effect::new(K::Wave);
    e.values = vec![10.0, 120.0, 30.0];
    for l in b.app.doc.panels[PLAN].layers.iter_mut() {
        l.fx.push(e.clone());
    }
    au_storyboard(&mut b, PLAN);
    let w = b.app.doc.width as f32;
    let t = (w - feuille.0.x) / (feuille.1.x - feuille.0.x);
    let bord = feuille.0 + (feuille.1 - feuille.0) * t;
    b.app.view.zoom = 1.0;
    b.app.view.pan = vec2(0.0, 0.0);
    let s = ecran_du_storyboard(&b, bord);
    b.app.view.pan = b.app.canvas_rect.center() - s;
    b.chauffer(4);
    let centre = |img: &[u8], x: i32, cy: i32| {
        let (mut somme, mut pond) = (0.0f32, 0.0f32);
        for y in cy - 90..=cy + 90 {
            let v = crate::traits_continus_tests::px(img, x, y);
            let a = (v[0] - v[1]).max(0) as f32;
            somme += a;
            pond += a * y as f32;
        }
        (pond / somme.max(1.0), somme)
    };
    let debut = std::time::Instant::now();
    let c = ecran_du_storyboard(&b, bord);
    let (x, cy) = (c.x.round() as i32, c.y.round() as i32);
    // (Le saut d'une colonne à l'autre de chaque côté du bord, le trait étant en pente : au plus quelques pixels, d'ici que l'effet arrive du fil de fond.)
    let frame = b.app.cur_frame;
    let mut plan = crate::render::panel_plan_at(&b.app.doc, PLAN, frame, Some(b.app.doc.paper));
    plan.items.retain(|i| !i.fx.is_empty());
    assert!(!plan.items.is_empty(), "le calque porte l'onde");
    // Le fil de fond répond, le hors-feuille se refait, l'écran montre le résultat.
    while !(crate::fx::effets_prets(&b.app.doc, &plan.items[0]) && !b.app.monde.busy()) {
        assert!(debut.elapsed() < std::time::Duration::from_secs(30), "l'effet arrive du fil de fond");
        b.image(Vec::new(), 1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    for _ in 0..30 {
        b.image(Vec::new(), 1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    // (Un autre essai peut avoir vidé la réserve commune entre-temps : la règle ne se juge qu'avec le résultat là.)
    let pret = crate::fx::resultat_pret(&b.app.doc, &plan.items[0]).is_some();
    assert!(!pret || !crate::gpu::gpu_fx_item(&b.app.doc, &plan.items[0]), "un calque qui déborde, avec une onde, le résultat prêt : le calcul de fond");
    let img = b.capturer().unwrap();
    sauver(&img, "ecran-onde");
    let ((a, ea), (q, eq)) = (centre(&img, x - 2, cy), centre(&img, x + 2, cy));
    assert!(ea > 0.0 && eq > 0.0, "le trait est visible de chaque côté du bord");
    let saut = (a - q).abs();
    // Au-delà de la feuille, ce qu'on voit est bien l'onde : le trait à l'écran suit le dessin dérivé, pas le dessin sans effet.
    let derive = crate::fx::shown(&b.app.doc, &plan.items[0]).expect("le dessin dérivé");
    let cel = b.app.doc.cels.get(&plan.items[0].cel).expect("le dessin");
    let centre_de = |r: &crate::raster::Raster, wx: i64, wy: i64| {
        let (mut somme, mut pond) = (0.0f32, 0.0f32);
        for y in wy - 90..=wy + 90 {
            let c = r.get_i(wx, y);
            somme += c.a() as f32;
            pond += c.a() as f32 * y as f32;
        }
        pond / somme.max(1.0)
    };
    let (mut ecart_au_derive, mut ecart_au_dessin) = (0.0f32, 0.0f32);
    for d in [20i32, 60, 100, 140] {
        let (wx, wy) = (bord.x as i64 + d as i64, bord.y as i64);
        let vu = centre(&img, x + d, cy) .0 - cy as f32 + wy as f32;
        ecart_au_derive = ecart_au_derive.max((vu - centre_de(&derive.raster, wx, wy)).abs());
        ecart_au_dessin = ecart_au_dessin.max((centre_de(&cel.raster, wx, wy) - centre_de(&derive.raster, wx, wy)).abs());
    }
    println!("ONDE À L'ÉCRAN : saut de {saut:.2} pixels au bord de la feuille ; au-delà, {ecart_au_derive:.2} pixels du dessin dérivé, qui s'écarte de {ecart_au_dessin:.2} du dessin sans effet");
    assert!(saut <= 3.0, "le trait saute de {saut} pixels au bord de la feuille");
    assert!(ecart_au_dessin > 3.0, "l'onde se voit au-delà de la feuille");
    assert!(ecart_au_derive <= 2.0, "à l'écran, le trait au-delà de la feuille est celui de l'onde ({ecart_au_derive} pixels d'écart)");
}

/// Le trait à travers le bord de la feuille, vu de près, avec chaque effet qui s'y cassait (partie 121).
#[test]
#[ignore]
fn captures_effets_au_bord() {
    use crate::fx::{Effect, EffectKind as K};
    let effets: [(&str, K, Option<Vec<f32>>); 12] = [
        ("onde", K::Wave, Some(vec![10.0, 120.0, 30.0])),
        ("ondulation", K::Ripple, Some(vec![8.0, 60.0, 100.0, 0.0])),
        ("tourbillon", K::Twirl, Some(vec![90.0, 150.0, 100.0, 50.0])),
        ("secousse", K::Shake, Some(vec![30.0, 14.0, 2.0, 110.0, 0.0])),
        ("vignettage", K::Vignette, None),
        ("aberration", K::Aberration, Some(vec![30.0, 100.0, 0.0])),
        ("flou", K::GaussianBlur, None),
        ("mosaique", K::Mosaic, Some(vec![24.0])),
        ("pointilles", K::Dashes, None),
        ("ecriture", K::Write, Some(vec![70.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0])),
        ("poussiere", K::Dust, None),
        ("onde-du-trait", K::StrokeWave, None),
    ];
    for (nom, kind, valeurs) in effets {
        let Some(Scene { mut b, feuille, .. }) = scene(false, false, 1.17, 0.5) else { return };
        let mut e = Effect::new(kind);
        if let Some(v) = valeurs {
            e.values = v;
        }
        for l in b.app.doc.panels[PLAN].layers.iter_mut() {
            l.fx.push(e.clone());
        }
        au_storyboard(&mut b, PLAN);
        let w = b.app.doc.width as f32;
        let t = (w - feuille.0.x) / (feuille.1.x - feuille.0.x);
        let bord = feuille.0 + (feuille.1 - feuille.0) * t;
        b.app.view.zoom = 1.0;
        b.app.view.pan = vec2(0.0, 0.0);
        let s = ecran_du_storyboard(&b, bord);
        b.app.view.pan = b.app.canvas_rect.center() - s;
        b.chauffer(4);
        for _ in 0..150 {
            b.image(Vec::new(), 1.0 / 60.0);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        sauver(&b.capturer().unwrap(), &format!("effet-{nom}"));
    }
}

#[test]
#[ignore]
fn captures_camera_traits() {
    for (nom, dx, dy) in [("droite", 1.17, 0.5), ("loin", 2.2, 0.1), ("gauche", -1.17, 0.3), ("haut", 0.3, -1.1)] {
        for bitmap in [false, true] {
            let Some(Scene { mut b, de, a, feuille }) = scene(bitmap, true, dx, dy) else { return };
            let suffixe = format!("{nom}-{}", if bitmap { "bitmap" } else { "vecteur" });
            let (img, r) = image_du_trait(&mut b, de, a);
            sauver(&img, &format!("cap-vignettes-{suffixe}"));
            println!("{suffixe} vignettes {r:?}");
            au_storyboard(&mut b, PLAN);
            let (e0, e1) = (ecran_du_storyboard(&b, feuille.0), ecran_du_storyboard(&b, feuille.1));
            let (img, r) = image_du_trait(&mut b, e0, e1);
            sauver(&img, &format!("cap-storyboard-{suffixe}"));
            println!("{suffixe} storyboard {r:?}");
        }
    }
}