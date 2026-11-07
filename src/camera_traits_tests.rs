//! Les traits d'un plan à mouvement de caméra (partie 117), **vus en pixels** : plan 6 d'une planche, cadre B posé par le geste des Vignettes,
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
        for (k, (a, b)) in out.tiles.iter().zip(&seule.tiles).enumerate() {
            let (ox, oy, tw, th) = out.tile_bounds(k);
            for y in 0..th {
                for x in 0..tw {
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