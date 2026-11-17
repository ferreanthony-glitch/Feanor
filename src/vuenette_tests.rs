//! La vue nette (`vuenette.rs`, partie 166) : au-delà de 100 %, un dessin vectoriel est redessiné à la taille de l'écran, le reste
//! garde les pixels de la feuille, et l'image suit ce qui change sans rien calculer pendant un geste.

use std::time::{Duration, Instant};

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect};

use crate::blend::BlendMode;
use crate::brush::BrushSettings;
use crate::document::{Cel, CelId, Document, LayerKind, LayerTransform};
use crate::raster::{Tile, TILE_PIXELS};
use crate::render::{composite_pixel_i, Plan, PlanItem};
use crate::vector::{Element, Trace, VPoint};
use crate::view::View;
use crate::vuenette::{image_pour_essai, Cadrage, VueNette};

const PAPIER: [u8; 3] = [255, 255, 255];

fn item(cel: CelId) -> PlanItem {
    PlanItem { cel, opacity: 255, tint: None, transform: None, blend: BlendMode::Normal, fx: Vec::new(), ctg: None, fond: None, pre: None }
}

/// Un dessin bitmap à motif : ces tuiles de la feuille, de cette couleur (prémultipliée).
fn bitmap(doc: &mut Document, tuiles: &[usize], c: [u8; 4]) -> CelId {
    let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
    for (k, &i) in tuiles.iter().enumerate() {
        let mut px = [Color32::TRANSPARENT; TILE_PIXELS];
        for (n, p) in px.iter_mut().enumerate() {
            if (n / 7 + n % 5 + k) % 3 != 0 {
                *p = Color32::from_rgba_premultiplied(c[0], c[1], c[2], c[3]);
            }
        }
        cel.raster.replace_slot(i, Some(Tile::new(px)));
    }
    doc.add_cel(cel)
}

/// Un trait droit de `a` à `b`, de rayon `r`, noir : lisse (`pinceau` rien) ou au réglage complet d'un pinceau dur.
fn trait_droit(a: Pos2, b: Pos2, r: f32, pinceau: bool) -> Element {
    let n = 24;
    let points = (0..=n).map(|k| a + (b - a) * (k as f32 / n as f32)).map(|p| VPoint { x: p.x, y: p.y, r }).collect();
    let brush = pinceau.then(|| Box::new(BrushSettings { size: 2.0 * r, hardness: 1.0, pressure_size: false, pressure_opacity: false, ..Default::default() }));
    Element::Stroke { points, color: [0, 0, 0], opacity: 1.0, hardness: 1.0, brush, trace: Trace::default() }
}

/// Un dessin vectoriel fait de ces éléments, son image à jour.
fn vectoriel(doc: &mut Document, elements: Vec<Element>) -> CelId {
    let mut cel = Cel::new(doc.width, doc.height, LayerKind::Vector);
    cel.vector.as_mut().expect("dessin vectoriel").elements = elements;
    cel.rerender();
    doc.add_cel(cel)
}

fn vue(zoom: f32) -> View {
    View { zoom, ..Default::default() }
}

/// La zone du dessin à l'écran dans ces essais, et la taille de la feuille.
fn zone() -> Rect {
    Rect::from_min_size(pos2(10.0, 20.0), vec2(320.0, 240.0))
}

fn cadrage(doc: &Document, view: &View) -> Cadrage {
    Cadrage::de(view, zone(), vec2(doc.width as f32, doc.height as f32), 1.0).expect("la vue grossit")
}

/// Distance du point `p` au segment `a`–`b`.
fn distance_au_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

#[test]
fn le_cadrage_suit_la_vue_tournee_et_retournee() {
    let size = vec2(400.0, 300.0);
    for (view, ppp) in [
        (vue(2.0), 1.0),
        (View { zoom: 3.3, rotation: 0.4, flip: true, pan: vec2(17.0, -31.0), persp: None }, 1.5),
        (View { zoom: 1.7, rotation: -1.1, flip: false, pan: vec2(-80.0, 12.0), persp: None }, 1.25),
    ] {
        let c = Cadrage::de(&view, zone(), size, ppp).expect("la vue grossit");
        for (u, v) in [(0.5, 0.5), (100.5, 7.5), (c.taille[0] as f32 - 0.5, c.taille[1] as f32 - 0.5)] {
            let attendu = view.to_canvas(zone(), size, pos2((c.origine[0] as f32 + u) / ppp, (c.origine[1] as f32 + v) / ppp));
            let lu = c.vers_feuille.apply(pos2(u, v));
            assert!(lu.distance(attendu) < 0.01, "le pixel ({u}, {v}) de l'image tombe en {lu:?} sur la feuille, la vue dit {attendu:?}");
        }
        // L'image couvre toute la zone, au pixel près.
        assert!(c.taille[0] as f32 >= zone().width() * ppp && c.taille[1] as f32 >= zone().height() * ppp);
    }
    // À 100 % ou moins, et en perspective : pas de vue nette.
    assert!(Cadrage::de(&vue(1.0), zone(), size, 1.0).is_none());
    assert!(Cadrage::de(&vue(0.35), zone(), size, 1.0).is_none());
    assert!(Cadrage::de(&vue(0.8), zone(), size, 1.5).is_some(), "0,8 × 1,5 pixel par point : la vue grossit");
}

#[test]
fn le_bitmap_garde_les_pixels_de_la_feuille() {
    let mut doc = Document::new(400, 300, 24);
    let fond = bitmap(&mut doc, &[8, 9, 10, 15, 16, 17], [40, 90, 160, 200]);
    let pose = bitmap(&mut doc, &[9, 16], [150, 30, 20, 255]);
    // (Un trait vectoriel loin du centre : sans lui, la vue nette n'a rien à montrer.)
    let trait_ = vectoriel(&mut doc, vec![trait_droit(pos2(150.0, 110.0), pos2(190.0, 112.0), 3.0, false)]);
    let deplace = PlanItem { transform: Some(LayerTransform { x: 13.0, y: -7.0, ..Default::default() }), blend: BlendMode::Multiply, ..item(pose) };
    let plan = Plan { paper: Some(PAPIER), items: vec![item(fond), deplace, item(trait_)] };
    let c = cadrage(&doc, &vue(2.0));
    let image = image_pour_essai(&doc, &plan, c).expect("un trait vectoriel est dans la zone");
    let encre = &doc.cels[&trait_].raster;
    let mut compares = 0;
    for v in 0..c.taille[1] {
        for u in 0..c.taille[0] {
            let p = c.vers_feuille.apply(pos2(u as f32 + 0.5, v as f32 + 0.5));
            let (x, y) = (p.x.floor() as i64, p.y.floor() as i64);
            // Loin du trait vectoriel (il n'a rien à trois pixels à la ronde) : exactement le pixel de la feuille.
            if (-3..=3).any(|dy| (-3..=3).any(|dx| encre.get_i(x + dx, y + dy).a() != 0)) {
                continue;
            }
            assert_eq!(image[v * c.taille[0] + u], composite_pixel_i(&doc, &plan, x, y), "pixel d'écran ({u}, {v}), pixel ({x}, {y}) de la feuille");
            compares += 1;
        }
    }
    assert!(compares > c.taille[0] * c.taille[1] / 2, "{compares} pixels comparés");
}

/// Le bord d'un trait net est à moins de `marge` pixel de la feuille de son contour exact ; rend la part des pixels d'écran dont la
/// couleur n'est ni l'encre ni le papier.
fn verifier_le_bord(image: &[Color32], c: &Cadrage, a: Pos2, b: Pos2, r: f32, marge: f32) {
    let (mut dedans, mut dehors) = (0, 0);
    for v in 0..c.taille[1] {
        for u in 0..c.taille[0] {
            let p = c.vers_feuille.apply(pos2(u as f32 + 0.5, v as f32 + 0.5));
            let d = distance_au_segment(p, a, b);
            let px = image[v * c.taille[0] + u];
            if d < r - marge {
                assert!(px.r() < 40, "en ({u}, {v}), à {d:.2} du milieu d'un trait de rayon {r} : {px:?} au lieu de l'encre");
                dedans += 1;
            } else if d > r + marge {
                assert!(px.r() > 215, "en ({u}, {v}), à {d:.2} du milieu d'un trait de rayon {r} : {px:?} au lieu du papier");
                dehors += 1;
            }
        }
    }
    assert!(dedans > 500 && dehors > 500, "{dedans} pixels dans le trait, {dehors} hors du trait");
}

#[test]
fn un_trait_lisse_est_redessine_a_la_taille_de_l_ecran() {
    let mut doc = Document::new(400, 300, 24);
    let (a, b, r) = (pos2(150.0, 120.0), pos2(250.0, 180.0), 6.0);
    let dessin = vectoriel(&mut doc, vec![trait_droit(a, b, r, false)]);
    let plan = Plan { paper: Some(PAPIER), items: vec![item(dessin)] };
    for view in [vue(4.0), View { zoom: 4.0, rotation: 0.6, flip: true, pan: vec2(9.0, 4.0), persp: None }] {
        let c = cadrage(&doc, &view);
        let image = image_pour_essai(&doc, &plan, c).expect("le trait est dans la zone");
        // À 400 %, un pixel d'écran vaut un quart de pixel de la feuille : le bord est tenu à 0,3 pixel de la feuille près. L'image de
        // la feuille grossie ne le tient pas (son bord est une marche d'un pixel entier).
        verifier_le_bord(&image, &c, a, b, r, 0.3);
    }
    // L'image de toujours, grossie : elle échoue au même contrôle (c'est le défaut).
    let c = cadrage(&doc, &vue(4.0));
    let grossie: Vec<Color32> = (0..c.taille[0] * c.taille[1])
        .map(|k| {
            let p = c.vers_feuille.apply(pos2((k % c.taille[0]) as f32 + 0.5, (k / c.taille[0]) as f32 + 0.5));
            composite_pixel_i(&doc, &plan, p.x.floor() as i64, p.y.floor() as i64)
        })
        .collect();
    let tient = std::panic::catch_unwind(|| verifier_le_bord(&grossie, &c, a, b, r, 0.3)).is_ok();
    assert!(!tient, "l'image de la feuille grossie ne devrait pas tenir le bord à 0,3 pixel près");
}

#[test]
fn un_trait_au_pinceau_est_rejoue_a_la_taille_de_l_ecran() {
    let mut doc = Document::new(400, 300, 24);
    let (a, b, r) = (pos2(150.0, 120.0), pos2(250.0, 180.0), 6.0);
    let dessin = vectoriel(&mut doc, vec![trait_droit(a, b, r, true)]);
    let plan = Plan { paper: Some(PAPIER), items: vec![item(dessin)] };
    let c = cadrage(&doc, &vue(4.0));
    let avant = crate::vector::replay_count();
    let image = image_pour_essai(&doc, &plan, c).expect("le trait est dans la zone");
    assert_eq!(crate::vector::replay_count(), avant, "le rejeu de la vue nette ne compte pas comme un rendu du dessin");
    // Le moteur des pinceaux pose des empreintes rondes : le bord est tenu à un demi-pixel de la feuille près (deux pixels d'écran).
    verifier_le_bord(&image, &c, a, b, r, 0.5);
    // Et l'image a du détail plus fin que le pixel de la feuille : des carrés de 4 × 4 pixels d'écran (un pixel de la feuille) qui ne
    // sont pas d'une seule couleur.
    let [w, h] = c.taille;
    let melanges = (0..h / 4)
        .flat_map(|by| (0..w / 4).map(move |bx| (bx, by)))
        .filter(|&(bx, by)| {
            let premier = image[by * 4 * w + bx * 4];
            (0..16).any(|k| image[(by * 4 + k / 4) * w + bx * 4 + k % 4] != premier)
        })
        .count();
    assert!(melanges > 100, "{melanges} carrés d'un pixel de la feuille montrent du détail");
    // Le dessin lui-même n'a pas changé : son image est celle de la feuille, son cache aussi.
    let cel = &doc.cels[&dessin];
    assert_eq!(cel.raster.width, 400);
    assert!(cel.vector.as_ref().expect("vectoriel").elements[0].cache_ready(), "le cache du trait d'origine est intact");
}

#[test]
fn un_calque_deplace_montre_son_trait_a_sa_place() {
    let mut doc = Document::new(400, 300, 24);
    let (a, b, r) = (pos2(150.0, 140.0), pos2(190.0, 140.0), 5.0);
    let dessin = vectoriel(&mut doc, vec![trait_droit(a, b, r, false)]);
    let d = vec2(40.0, 15.0);
    let plan = Plan { paper: Some(PAPIER), items: vec![PlanItem { transform: Some(LayerTransform { x: d.x, y: d.y, ..Default::default() }), ..item(dessin) }] };
    let c = cadrage(&doc, &vue(3.0));
    let image = image_pour_essai(&doc, &plan, c).expect("le trait est dans la zone");
    verifier_le_bord(&image, &c, a + d, b + d, r, 0.35);
}

#[test]
fn rien_de_net_sans_dessin_vectoriel_dans_la_zone() {
    let mut doc = Document::new(400, 300, 24);
    let fond = bitmap(&mut doc, &[8, 9], [40, 90, 160, 255]);
    // Un trait vectoriel dans le coin de la feuille : à 400 %, la vue (au centre) ne le regarde pas.
    let loin = vectoriel(&mut doc, vec![trait_droit(pos2(5.0, 5.0), pos2(20.0, 8.0), 2.0, false)]);
    let plan = Plan { paper: Some(PAPIER), items: vec![item(fond), item(loin)] };
    assert!(image_pour_essai(&doc, &plan, cadrage(&doc, &vue(4.0))).is_none());
    // Sans papier (arrière-plan « aucun » ou « damier ») : le plan n'est pas admis.
    assert!(!crate::vuenette::admis(&Plan { paper: None, items: plan.items.clone() }));
    assert!(crate::vuenette::admis(&plan));
}

/// Fait tourner `VueNette::update` (au repos, rien de changé) jusqu'à ce que `fini` soit vrai ; rend ce que la dernière image peint.
fn attendre(n: &mut VueNette, ctx: &egui::Context, doc: &Document, plan: &Plan, c: Option<Cadrage>, fini: impl Fn(&VueNette) -> bool) -> Option<crate::vuenette::Peinte> {
    let debut = Instant::now();
    loop {
        let p = n.update(ctx, doc, plan, c, true, false, &[]);
        if fini(n) {
            return p;
        }
        assert!(debut.elapsed() < Duration::from_secs(20), "la vue nette n'arrive pas");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn la_vue_nette_suit_ce_qui_change_et_ne_calcule_pas_pendant_un_geste() {
    let ctx = egui::Context::default();
    let mut doc = Document::new(400, 300, 24);
    let dessin = vectoriel(&mut doc, vec![trait_droit(pos2(150.0, 120.0), pos2(250.0, 180.0), 6.0, true)]);
    let plan = Plan { paper: Some(PAPIER), items: vec![item(dessin)] };
    let c = cadrage(&doc, &vue(3.0));
    let mut n = VueNette::new();
    // Première image : rien à peindre encore (l'image de toujours), le calcul part après un court repos, sur un autre fil.
    assert!(n.update(&ctx, &doc, &plan, Some(c), true, true, &[]).is_none());
    let p = attendre(&mut n, &ctx, &doc, &plan, Some(c), |n| n.composees == 1).expect("l'image nette est arrivée");
    assert_eq!(p.cadrage, c);
    // Rien ne change : elle reste, et rien ne se recalcule.
    std::thread::sleep(Duration::from_millis(120));
    assert!(n.update(&ctx, &doc, &plan, Some(c), true, false, &[]).is_some());
    assert!(!n.en_calcul() && n.composees == 1);
    // Un trait en cours touche des tuiles : l'image reste (percée là où ça change), et **rien ne se calcule pendant le geste**.
    for _ in 0..30 {
        assert!(n.update(&ctx, &doc, &plan, Some(c), false, false, &[17, 18]).is_some());
        assert!(!n.en_calcul(), "aucun calcul pendant un geste");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(n.composees, 1);
    // Le geste fini : l'image se refait.
    attendre(&mut n, &ctx, &doc, &plan, Some(c), |n| n.composees == 2).expect("l'image nette est revenue");
    // La vue bouge : l'ancienne image reste montrée (collée au dessin) jusqu'à la nouvelle.
    let c2 = cadrage(&doc, &View { pan: vec2(30.0, -12.0), ..vue(3.0) });
    let p = n.update(&ctx, &doc, &plan, Some(c2), true, false, &[]).expect("l'image d'avant reste");
    assert_eq!(p.cadrage, c);
    let p = attendre(&mut n, &ctx, &doc, &plan, Some(c2), |n| n.composees == 3).expect("la nouvelle image");
    assert_eq!(p.cadrage, c2);
    // Tout a pu changer (autre image, réglage) : l'image nette est retirée tout de suite, puis revient.
    assert!(n.update(&ctx, &doc, &plan, Some(c2), true, true, &[]).is_none());
    attendre(&mut n, &ctx, &doc, &plan, Some(c2), |n| n.composees == 4).expect("l'image nette est revenue");
    // Un geste qui commence pendant un calcul l'arrête.
    assert!(n.update(&ctx, &doc, &plan, Some(c2), true, true, &[]).is_none());
    let debut = Instant::now();
    while !n.en_calcul() {
        n.update(&ctx, &doc, &plan, Some(c2), true, false, &[]);
        assert!(debut.elapsed() < Duration::from_secs(20) && n.composees == 4 || n.en_calcul() || n.composees > 4, "le calcul ne part pas");
        if n.composees > 4 {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    if n.composees == 4 {
        n.update(&ctx, &doc, &plan, Some(c2), false, false, &[]);
        assert!(!n.en_calcul(), "le geste arrête le calcul en cours");
    }
    // À 100 % (plus de cadrage) : plus rien à peindre.
    assert!(n.update(&ctx, &doc, &plan, None, true, false, &[]).is_none());
}

// ---------------------------------------------------------------------------------------------- dans la vraie fenêtre

fn ecran(b: &crate::perf_tests::Banc, p: Pos2) -> Pos2 {
    b.app.view.to_screen(b.app.canvas_rect, b.app.canvas_size(), p)
}

/// Des images d'écran, au repos, jusqu'à ce que `fini` soit vrai.
fn images_jusqu_a(b: &mut crate::perf_tests::Banc, quoi: &str, fini: impl Fn(&crate::app::App) -> bool) {
    let debut = Instant::now();
    while !fini(&b.app) {
        assert!(debut.elapsed() < Duration::from_secs(30), "{quoi}");
        std::thread::sleep(Duration::from_millis(5));
        b.image(Vec::new(), 1.0 / 60.0);
    }
}

/// Sur le banc (la vraie fenêtre, sans écran) : la vue à 300 %, un trait de pinceau tracé au pointeur sur un calque vectoriel.
#[test]
fn dans_la_vraie_fenetre_un_trait_trace_en_gros_plan_revient_net() {
    use eframe::egui::{Event, Modifiers, PointerButton};
    // (La vue nette de l'application est éteinte d'origine dans les essais : `vuenette::activer_pour_essai`.)
    crate::vuenette::activer_pour_essai();
    let mut b = crate::multicalques_tests::banc(&[LayerKind::Vector], crate::app::Tool::Brush);
    b.app.zoom_by(3.0 / b.app.view.zoom);
    // (Le trait du calque est à la hauteur 60 de la feuille : la vue se pose dessus, sinon elle n'a rien de vectoriel à montrer.)
    let (rect, size) = (b.app.canvas_rect, b.app.canvas_size());
    let ou = b.app.view.to_screen(rect, size, pos2(200.0, 75.0));
    b.app.view.pan += rect.center() - ou;
    images_jusqu_a(&mut b, "l'image nette n'arrive pas à 300 %", |a| a.nette.composees >= 1 && !a.nette.en_calcul());
    let dessin = b.app.doc.panels[0].layers[0].cel_at(0).expect("le calque a un dessin");
    let traits = |a: &crate::app::App| a.doc.cels[&dessin].vector.as_ref().expect("vectoriel").elements.len();
    let (traits_avant, composees) = (traits(&b.app), b.app.nette.composees);
    // Le trait : appui, glissé, lâcher. Tant que le bouton est tenu, rien ne se calcule (« sans latence »).
    let pts = crate::multicalques_tests::ligne(pos2(150.0, 80.0), pos2(230.0, 100.0), 12);
    let bouton = |pos: Pos2, pressed: bool| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    let p0 = ecran(&b, pts[0]);
    b.image(vec![Event::PointerMoved(p0)], 1.0 / 60.0);
    b.image(vec![bouton(p0, true)], 1.0 / 60.0);
    for &p in &pts[1..] {
        let q = ecran(&b, p);
        b.image(vec![Event::PointerMoved(q)], 1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(8));
        assert!(!b.app.nette.en_calcul(), "aucun calcul de la vue nette pendant le trait");
    }
    assert_eq!(b.app.nette.composees, composees, "aucune image nette refaite pendant le trait");
    let fin = ecran(&b, *pts.last().expect("des points"));
    b.image(vec![bouton(fin, false)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(traits(&b.app), traits_avant + 1, "le trait est dans le dessin");
    // Le trait lâché : l'image nette se refait, avec lui, puis plus rien ne se calcule.
    images_jusqu_a(&mut b, "l'image nette ne revient pas après le trait", |a| a.nette.composees > composees && !a.nette.en_calcul());
    let apres = b.app.nette.composees;
    for _ in 0..6 {
        std::thread::sleep(Duration::from_millis(40));
        b.image(Vec::new(), 1.0 / 60.0);
    }
    assert!(!b.app.nette.en_calcul() && b.app.nette.composees == apres, "au repos, la vue nette ne se refait pas ({} puis {})", apres, b.app.nette.composees);
    // Et l'application se repose : l'image nette à jour, plus rien ne demande à être repeint.
    b.attendre_vignettes();
    let pire = (0..6).map(|_| b.image(Vec::new(), 0.5).reveil).min().unwrap_or(Duration::MAX);
    assert!(pire >= Duration::from_secs(1), "au repos, l'image nette à jour, FEANOR demande à être repeint dans {pire:?}{}", b.raison_du_reveil());
    assert_eq!(b.app.nette.composees, apres);
    // Annuler : le dessin perd le trait, l'image nette suit.
    b.app.run_action("edition.annuler");
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(traits(&b.app), traits_avant);
    images_jusqu_a(&mut b, "l'image nette ne revient pas après l'annulation", |a| a.nette.composees > apres && !a.nette.en_calcul());
    // À 100 % : plus d'image nette.
    b.app.zoom_by(1.0 / b.app.view.zoom);
    b.image(Vec::new(), 1.0 / 60.0);
    let au_cent = b.app.nette.composees;
    for _ in 0..5 {
        std::thread::sleep(Duration::from_millis(40));
        b.image(Vec::new(), 1.0 / 60.0);
    }
    assert!(!b.app.nette.en_calcul() && b.app.nette.composees == au_cent, "à 100 %, rien à composer");
}
