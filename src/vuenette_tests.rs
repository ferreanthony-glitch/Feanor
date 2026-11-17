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
        let p = n.update(ctx, doc, plan, c, false, false, false, &[]);
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
    // Première image : rien à peindre encore (l'image de toujours, lissée en attendant), le calcul part après un court repos, sur
    // un autre fil.
    assert!(n.update(&ctx, &doc, &plan, Some(c), false, false, true, &[]).is_none());
    assert!(n.lisse(), "une image nette est attendue : l'image de toujours se pose lissée");
    let p = attendre(&mut n, &ctx, &doc, &plan, Some(c), |n| n.composees == 1).expect("l'image nette est arrivée");
    assert_eq!(p.cadrage, c);
    // Rien ne change : elle reste, et rien ne se recalcule.
    std::thread::sleep(Duration::from_millis(120));
    assert!(n.update(&ctx, &doc, &plan, Some(c), false, false, false, &[]).is_some());
    assert!(!n.en_calcul() && n.composees == 1);
    // Un trait en cours touche des tuiles : l'image reste (percée là où ça change), et **rien ne se calcule pendant le geste**.
    for _ in 0..30 {
        assert!(n.update(&ctx, &doc, &plan, Some(c), true, true, false, &[17, 18]).is_some());
        assert!(!n.en_calcul(), "aucun calcul pendant un trait");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(n.composees, 1);
    // Le geste fini : l'image se refait.
    attendre(&mut n, &ctx, &doc, &plan, Some(c), |n| n.composees == 2).expect("l'image nette est revenue");
    // Un bouton tenu pendant que la vue ne bouge pas (une sélection qu'on tire) : un geste de dessin lui aussi, rien ne se calcule.
    std::thread::sleep(Duration::from_millis(300));
    for _ in 0..20 {
        assert!(n.update(&ctx, &doc, &plan, Some(c), false, true, false, &[17]).is_some());
        assert!(!n.en_calcul(), "aucun calcul pendant un geste, la vue immobile");
        std::thread::sleep(Duration::from_millis(5));
    }
    attendre(&mut n, &ctx, &doc, &plan, Some(c), |n| n.composees == 3).expect("l'image nette est revenue");
    // La vue bouge : l'ancienne image reste montrée (collée au dessin) jusqu'à la nouvelle.
    let c2 = cadrage(&doc, &View { pan: vec2(30.0, -12.0), ..vue(3.0) });
    let p = n.update(&ctx, &doc, &plan, Some(c2), false, false, false, &[]).expect("l'image d'avant reste");
    assert_eq!(p.cadrage, c);
    let p = attendre(&mut n, &ctx, &doc, &plan, Some(c2), |n| n.composees == 4).expect("la nouvelle image");
    assert_eq!(p.cadrage, c2);
    // **On déplace la vue, bouton tenu, sans s'arrêter** (une image d'écran toutes les 5 ms, la vue décalée à chacune) : l'image
    // nette se refait pendant le mouvement, sans attendre le repos.
    let avant = n.composees;
    for k in 1..=80 {
        let ck = cadrage(&doc, &View { pan: vec2(30.0 + k as f32, -12.0), ..vue(3.0) });
        assert!(n.update(&ctx, &doc, &plan, Some(ck), false, true, false, &[]).is_some(), "une image nette reste montrée pendant le déplacement");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(n.composees > avant, "l'image nette s'est refaite pendant le déplacement ({} puis {})", avant, n.composees);
    let c3 = cadrage(&doc, &View { pan: vec2(110.0, -12.0), ..vue(3.0) });
    let p = attendre(&mut n, &ctx, &doc, &plan, Some(c3), |n| !n.en_calcul() && n.composees > avant).expect("la dernière image");
    let p = if p.cadrage == c3 { p } else { attendre(&mut n, &ctx, &doc, &plan, Some(c3), |n| !n.en_calcul()).expect("la dernière image") };
    let _ = p;
    let fini = n.composees;
    // Tout a pu changer (autre image, réglage) : l'image nette est retirée tout de suite, puis revient.
    assert!(n.update(&ctx, &doc, &plan, Some(c3), false, false, true, &[]).is_none());
    attendre(&mut n, &ctx, &doc, &plan, Some(c3), |n| n.composees > fini && !n.en_calcul()).expect("l'image nette est revenue");
    // Un trait qui commence pendant un calcul l'arrête.
    assert!(n.update(&ctx, &doc, &plan, Some(c3), false, false, true, &[]).is_none());
    let (debut, la) = (Instant::now(), n.composees);
    while !n.en_calcul() && n.composees == la {
        n.update(&ctx, &doc, &plan, Some(c3), false, false, false, &[]);
        assert!(debut.elapsed() < Duration::from_secs(20), "le calcul ne part pas");
        std::thread::sleep(Duration::from_millis(2));
    }
    if n.composees == la {
        n.update(&ctx, &doc, &plan, Some(c3), true, true, false, &[]);
        assert!(!n.en_calcul(), "le trait arrête le calcul en cours");
    }
    // À 100 % (plus de cadrage) : plus rien à peindre, plus rien à lisser.
    assert!(n.update(&ctx, &doc, &plan, None, false, false, false, &[]).is_none());
    assert!(!n.lisse());
}

#[test]
fn un_plan_sans_vectoriel_garde_ses_pixels_nets_et_une_zone_sans_trait_aussi() {
    let ctx = egui::Context::default();
    let mut doc = Document::new(400, 300, 24);
    let fond = bitmap(&mut doc, &[8, 9, 10, 15, 16, 17], [40, 90, 160, 255]);
    let plan = Plan { paper: Some(PAPIER), items: vec![item(fond)] };
    let c = cadrage(&doc, &vue(3.0));
    let mut n = VueNette::new();
    // Tout en bitmap : jamais d'image nette, jamais de lissage, en mouvement comme à l'arrêt.
    for k in 0..20 {
        let ck = cadrage(&doc, &View { pan: vec2(k as f32, 0.0), ..vue(3.0) });
        assert!(n.update(&ctx, &doc, &plan, Some(ck), false, k % 2 == 0, k == 0, &[]).is_none());
        assert!(!n.lisse() && !n.en_calcul(), "un plan sans dessin vectoriel garde ses pixels nets");
        std::thread::sleep(Duration::from_millis(5));
    }
    // Un plan qui a du vectoriel, mais pas dans la zone regardée : au repos, constat fait, la zone garde ses pixels nets ; et un
    // trait qu'on y trace sur le bitmap (des tuiles changent) ne la lisse pas le temps du geste.
    let loin = vectoriel(&mut doc, vec![trait_droit(pos2(5.0, 5.0), pos2(20.0, 8.0), 2.0, false)]);
    let plan = Plan { paper: Some(PAPIER), items: vec![item(fond), item(loin)] };
    let debut = Instant::now();
    loop {
        assert!(n.update(&ctx, &doc, &plan, Some(c), false, false, false, &[]).is_none());
        if !n.lisse() {
            break;
        }
        assert!(debut.elapsed() < Duration::from_secs(20), "le constat « rien de vectoriel ici » n'arrive pas");
        std::thread::sleep(Duration::from_millis(5));
    }
    for _ in 0..10 {
        assert!(n.update(&ctx, &doc, &plan, Some(c), true, true, false, &[17]).is_none());
        assert!(!n.lisse() && !n.en_calcul(), "la zone sans trait vectoriel reste en pixels nets pendant le geste");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn l_image_nette_reste_sur_les_pixels_de_l_ecran_quand_la_vue_se_deplace() {
    let doc = Document::new(400, 300, 24);
    for ppp in [1.0_f32, 1.5] {
        let size = vec2(400.0, 300.0);
        let image = Cadrage::de(&vue(3.0), zone(), size, ppp).expect("la vue grossit");
        // La même vue : à sa place exacte.
        let coins = crate::vuenette::coins_a_l_ecran(&image, Some(&image), ppp).expect("même vue");
        assert_eq!(coins[0], pos2(image.origine[0] as f32 / ppp, image.origine[1] as f32 / ppp));
        // La vue déplacée de (30, −12) points : l'image suit, d'un nombre entier de pixels d'écran.
        let deplacee = Cadrage::de(&View { pan: vec2(30.0, -12.0), ..vue(3.0) }, zone(), size, ppp).expect("la vue grossit");
        let coins = crate::vuenette::coins_a_l_ecran(&image, Some(&deplacee), ppp).expect("la vue n'a fait que se déplacer");
        let attendu = pos2(image.origine[0] as f32 / ppp + 30.0, image.origine[1] as f32 / ppp - 12.0);
        assert!(coins[0].distance(attendu) <= 0.75 / ppp, "le coin de l'image : {:?}, attendu près de {attendu:?}", coins[0]);
        for c in coins {
            let (x, y) = (c.x * ppp, c.y * ppp);
            assert!((x - x.round()).abs() < 1e-3 && (y - y.round()).abs() < 1e-3, "le coin {c:?} tombe sur un pixel entier de l'écran");
        }
        assert!((coins[2] - coins[0] - vec2(image.taille[0] as f32 / ppp, image.taille[1] as f32 / ppp)).length() < 1e-3);
        // La vue a zoomé, ou tourné : pas de recalage (l'image suit ses coins sur la feuille).
        assert!(crate::vuenette::coins_a_l_ecran(&image, Cadrage::de(&vue(3.3), zone(), size, ppp).as_ref(), ppp).is_none());
        assert!(crate::vuenette::coins_a_l_ecran(&image, Cadrage::de(&View { rotation: 0.2, ..vue(3.0) }, zone(), size, ppp).as_ref(), ppp).is_none());
        assert!(crate::vuenette::coins_a_l_ecran(&image, None, ppp).is_none());
    }
    let _ = doc;
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

// ---------------------------------------------------------------------------------------------- sur la carte graphique

/// Dans la zone `zone` de la capture d'écran `px` (1920 × 1080, RGBA) : le nombre de paires de pixels voisins sur une ligne dont le
/// rouge diffère d'au moins 2, et le nombre de pixels ni encre ni papier (rouge entre 40 et 215).
fn mesurer(px: &[u8], zone: Rect) -> (usize, usize) {
    let (x0, x1) = (zone.min.x.ceil().max(0.0) as usize, (zone.max.x.floor() as usize).min(1919));
    let (y0, y1) = (zone.min.y.ceil().max(0.0) as usize, (zone.max.y.floor() as usize).min(1079));
    let rouge = |x: usize, y: usize| px[(y * 1920 + x) * 4] as i32;
    let (mut changements, mut gris) = (0, 0);
    for y in y0..y1 {
        for x in x0..x1 {
            if (rouge(x, y) - rouge(x + 1, y)).abs() >= 2 {
                changements += 1;
            }
            if (40..=215).contains(&rouge(x, y)) {
                gris += 1;
            }
        }
    }
    (changements, gris)
}

/// Sur la carte graphique, la vue à 400 % : l'image de toujours seule est en marches ; une image nette attendue, elle se pose
/// lissée ; l'image nette arrivée, le bord du trait tient en un pixel d'écran.
#[test]
fn sur_la_carte_graphique_l_image_attendue_est_lissee_puis_nette() {
    let mut doc = Document::new(400, 300, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].length = 4;
    let (a, z) = (pos2(150.0, 120.0), pos2(250.0, 180.0));
    let dessin = vectoriel(&mut doc, vec![trait_droit(a, z, 6.0, true)]);
    let mut calque = crate::document::Layer::new("Trait", LayerKind::Vector);
    calque.set(0, Some(dessin));
    doc.panels[0].layers.push(calque);
    let mut b = crate::perf_tests::banc(doc, true);
    if b.capturer().is_none() {
        println!("     pas de carte graphique pour les essais : essai sauté");
        return;
    }
    b.app.set_mode(crate::app::Mode::Animation);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.chauffer(10);
    b.app.zoom_by(4.0 / b.app.view.zoom);
    b.chauffer(4);
    // La zone du trait à l'écran, avec un peu de papier autour.
    let zone = Rect::from_two_pos(ecran(&b, a), ecran(&b, z)).expand(40.0).intersect(b.app.canvas_rect.shrink(4.0));
    assert!(zone.width() > 200.0 && zone.height() > 120.0, "le trait est à l'écran : {zone:?}");
    // 1. La vue nette éteinte (comme dans tous les autres essais) : l'image de toujours, grossie sans lissage.
    let (marches, gris_marches) = mesurer(&b.capturer().expect("carte graphique"), zone);
    // 2. La vue nette allumée, à sa première image : rien de net encore (le calcul n'est pas parti), mais une image nette est
    //    attendue : l'image de toujours se pose lissée.
    crate::vuenette::activer_pour_essai();
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(b.app.nette.lisse() && b.app.nette.composees == 0, "une image nette est attendue, aucune n'est encore arrivée");
    let (lisse, gris_lisse) = mesurer(&b.capturer().expect("carte graphique"), zone);
    // 3. L'image nette arrivée.
    images_jusqu_a(&mut b, "l'image nette n'arrive pas à 400 %", |a| a.nette.composees >= 1 && !a.nette.en_calcul());
    b.chauffer(2);
    let (net, gris_net) = mesurer(&b.capturer().expect("carte graphique"), zone);
    // En marches : la couleur ne change, sur une ligne, qu'aux bords des carrés de 4 pixels. Lissée : elle change de pixel en pixel
    // dans tout le dégradé du bord.
    assert!(lisse as f32 > 1.5 * marches as f32, "l'image attendue est lissée : {lisse} changements de pixel à pixel, contre {marches} en marches");
    // Nette : le bord tient en un pixel d'écran ; bien moins de pixels intermédiaires que l'image en marches ou lissée.
    assert!((gris_net as f32) < 0.5 * gris_marches as f32 && (gris_net as f32) < 0.5 * gris_lisse as f32, "l'image nette a un bord fin : {gris_net} pixels intermédiaires, contre {gris_marches} en marches et {gris_lisse} lissée");
    assert!(net > 0 && gris_net > 0, "le trait est bien dans la zone mesurée");
}