//! Le trait qui traverse une case qui ne pouvait pas le recevoir (partie 122), **vu en pixels** comme la partie 111 : un autre zoom, un roulis, une
//! bascule, un panoramique, des cadres A et B, un calque verrouillé, masqué ou d'une autre sorte, un plan écarté. Au vrai geste, en vectoriel et en
//! bitmap : pendant le geste, au repos, après réouverture, à l'export ; où sont les données ; une seule étape ; rien de ce qui était dessiné ne bouge.

use eframe::egui::{pos2, Color32, Event, Modifiers, Pos2, Rect};

use crate::document::LayerKind;
use crate::perf_tests::Banc;
use crate::traits_continus_tests::{des_plans, exporter, planche, relever, repos, sauver, tracer};
use crate::vignettes_tests::appui;

const PLAN: usize = 1;
/// Un filet de cadre croise le trait de biais sur quelques pixels (partie 118).
const TROU_PERMIS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Variante {
    Temoin,
    Zoom,
    ZoomFaible,
    Roulis,
    Bascule,
    Panoramique,
    Cadres,
    CadresZoom,
    CadresRoulis,
    Verrou,
    Masque,
    AutreSorte,
    CalqueTransforme,
    Ecarte,
}

const TOUTES: [Variante; 14] = [
    Variante::Temoin,
    Variante::Zoom,
    Variante::ZoomFaible,
    Variante::Roulis,
    Variante::Bascule,
    Variante::Panoramique,
    Variante::Cadres,
    Variante::CadresZoom,
    Variante::CadresRoulis,
    Variante::Verrou,
    Variante::Masque,
    Variante::AutreSorte,
    Variante::CalqueTransforme,
    Variante::Ecarte,
];

/// Ce que le plan `PLAN` devient, sous sa case déjà dessinée.
fn appliquer(b: &mut Banc, v: Variante, bitmap: bool) {
    let plan = b.app.doc.panels[PLAN].id;
    let doc = &mut b.app.doc;
    let base = doc.default_camera();
    let n = doc.panels[PLAN].frame_count();
    let cle = |doc: &mut crate::document::Document, cam| crate::camera::set_key(&mut doc.panels[PLAN].camera, 0, cam);
    match v {
        Variante::Temoin => {}
        Variante::Zoom => cle(doc, crate::camera::Camera { zoom: 1.6, ..base }),
        Variante::ZoomFaible => cle(doc, crate::camera::Camera { zoom: 0.7, ..base }),
        Variante::Roulis => cle(doc, crate::camera::Camera { angle: 17.0, ..base }),
        Variante::Bascule => cle(doc, crate::camera::Camera { tilt: 11.0, ..base }),
        Variante::Panoramique => cle(doc, crate::camera::Camera { pan: 11.0, ..base }),
        Variante::Cadres => crate::vignettes_camera_tests::deux_cadres(doc, PLAN, 160.0, 0.0),
        Variante::CadresZoom => {
            cle(doc, crate::camera::Camera { zoom: 1.5, ..base });
            crate::camera::set_key(&mut doc.panels[PLAN].camera, n - 1, crate::camera::Camera { zoom: 1.5, ..base }.shifted(160.0, 0.0));
        }
        Variante::CadresRoulis => {
            cle(doc, crate::camera::Camera { angle: 12.0, ..base });
            crate::camera::set_key(&mut doc.panels[PLAN].camera, n - 1, crate::camera::Camera { angle: 12.0, ..base }.shifted(160.0, 0.0));
        }
        Variante::Verrou => doc.panels[PLAN].layers[0].locked = true,
        Variante::Masque => doc.panels[PLAN].layers[0].visible = false,
        Variante::AutreSorte => {
            // Un plan dont le seul calque est de l'autre sorte, vide.
            let mut l = crate::document::Layer::new("Autre", if bitmap { LayerKind::Vector } else { LayerKind::Bitmap });
            l.frames = vec![None; n];
            doc.panels[PLAN].layers.clear();
            doc.panels[PLAN].layers.push(l);
        }
        Variante::CalqueTransforme => doc.panels[PLAN].layers[0].transform.scale = 1.3,
        Variante::Ecarte => {
            assert!(b.app.plan_ecarter_ou_reprendre(plan, true));
        }
    }
    b.app.renderer.invalidate_all();
}

/// Tout ce que la case porte : (nombre d'éléments, pixels d'encre, feuille et hors-feuille compris).
fn encre_du_plan(doc: &crate::document::Document, id: u64) -> (usize, usize) {
    let Some(p) = doc.panels.iter().find(|p| p.id == id).or_else(|| doc.annules.plans.iter().find(|g| g.panel.id == id).map(|g| &g.panel)) else { return (0, 0) };
    let mut vus: Vec<crate::document::CelId> = Vec::new();
    let (mut el, mut px) = (0, 0);
    for c in p.layers.iter().flat_map(|l| l.frames.iter().flatten()) {
        if vus.contains(c) {
            continue;
        }
        vus.push(*c);
        let Some(cel) = doc.cels.get(c) else { continue };
        el += cel.vector.as_ref().map_or(0, |v| v.elements.len());
        px += cel.raster.tiles.iter().flatten().chain(cel.raster.outer.values()).map(|t| t.iter().filter(|p| p.a() != 0).count()).sum::<usize>();
    }
    (el, px)
}

fn traits_hors_cadres(doc: &crate::document::Document) -> usize {
    let a = &doc.annotations;
    // (Partie 127 : un trait bitmap laisse ses pixels sur la couche bitmap de la planche, un morceau par case qui refuse et un pour le papier.)
    a.traces.iter().filter(|t| a.calque(t.calque).is_some_and(|c| c.planche)).count() + a.bitmap.morceaux.len()
}

/// Les rectangles à l'écran de l'image du plan `id`.
fn case_ecran(b: &Banc, id: u64) -> Rect {
    let l = b.app.board_layout();
    let c = l.cards.iter().chain(l.annulees.iter()).find(|c| c.plan == id).expect("la case");
    b.app.vignettes.xf.rect(c.thumb)
}

/// Le trou le plus long sur le trait de `bd` à `ba` (planche) dans l'image exportée.
fn trou_a_l_export(b: &Banc, img: &image::RgbImage, opt: &crate::planche::Options, bd: Pos2, ba: Pos2) -> (usize, f32) {
    let opt = b.app.planche_options(opt);
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
    (trou, s)
}

#[derive(Debug)]
pub(crate) struct Constat {
    /// Le plus long trou pendant le geste, au repos, après enregistrement et réouverture, à l'export (pixels ; rien : sans objet).
    geste: usize,
    repos: usize,
    rouvert: usize,
    export: Option<usize>,
    /// La case a reçu du neuf dans son plan.
    dans_le_plan: bool,
    hors_cadres: usize,
    etapes: usize,
    rejeux: usize,
    statut: String,
    /// Ce qui était déjà dessiné dans la case n'a pas bougé.
    deja_intact: bool,
    /// Annuler rend les données d'avant.
    annule_ok: bool,
}

/// Le déroulé : plan `PLAN` déjà dessiné, la variante posée, un trait à travers sa case (`depuis_la_case` : parti d'elle), et les mesures.
fn constater(v: Variante, bitmap: bool, depuis_la_case: bool) -> Option<Constat> {
    let mut doc = des_plans(18, bitmap);
    for p in 0..doc.panels.len() {
        doc.set_duration(p, 24);
    }
    static BANCS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(60);
    for _ in 0..BANCS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) {
        doc.new_cel(LayerKind::Vector);
    }
    let mut b = planche(doc, 6, 1.0)?;
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    // Ce qui est déjà dessiné dans la case, en bas : le trait d'après passe au-dessus, sans le toucher.
    let r = case_ecran(&b, ids[PLAN]);
    tracer(&mut b, pos2(r.left() + r.width() * 0.3, r.bottom() - r.height() * 0.15), pos2(r.left() + r.width() * 0.7, r.bottom() - r.height() * 0.15));
    repos(&mut b);
    let deja = b.app.doc.panels[PLAN].layers[0].cel_at(0);
    let deja_cel = deja.and_then(|c| b.app.doc.cels.get(&c)).cloned();
    let graine = traits_hors_cadres(&b.app.doc);
    let avant_plan = if v == Variante::AutreSorte { (0, 0) } else { encre_du_plan(&b.app.doc, ids[PLAN]) };
    appliquer(&mut b, v, bitmap);
    b.chauffer(4);
    b.attendre_vignettes();
    repos(&mut b);
    let (depart, arrivee) = if depuis_la_case { (ids[PLAN], ids[PLAN + 2]) } else { (ids[PLAN - 1], ids[PLAN + 1]) };
    let (rd, ra) = (case_ecran(&b, depart), case_ecran(&b, arrivee));
    // (Cadres tournés : un trait de plus de deux pixels et une hauteur où leurs filets le croisent de face. Un trait d'un pixel, dans leur tuile, est pâle : « Ce qui reste ».)
    let hauteur = if v == Variante::CadresRoulis {
        b.app.brush.size = 28.0;
        0.62
    } else {
        0.35
    };
    let y = rd.top() + rd.height() * hauteur;
    let (de, a) = (pos2(rd.center().x, y), pos2(ra.center().x, y));
    let (bd, ba) = (b.app.vignettes.xf.to_board(de), b.app.vignettes.xf.to_board(a));
    let etapes_avant = b.app.history.undo_len();
    let rejeux = crate::vector::replay_count();
    b.app.status.clear();
    // Le geste : la main en bas, l'image qu'on voit.
    let n = ((a - de).length() / 6.0).ceil().max(2.0) as usize;
    let chemin: Vec<Pos2> = (0..=n).map(|k| de + (a - de) * (k as f32 / n as f32)).collect();
    b.image(vec![Event::PointerMoved(chemin[0])], 1.0 / 60.0);
    b.image(vec![appui(chemin[0], true, Modifiers::NONE)], 1.0 / 60.0);
    for q in &chemin[1..] {
        b.image(vec![Event::PointerMoved(*q)], 1.0 / 120.0);
    }
    let geste_img = b.capturer()?;
    let nom = format!("{v:?}-{}-{}", if bitmap { "bitmap" } else { "vecteur" }, if depuis_la_case { "depuis" } else { "travers" });
    sauver(&geste_img, &format!("{nom}-1-geste"));
    let geste = relever(&geste_img, de, a).trou;
    let statut_geste = b.app.status.clone();
    b.image(vec![appui(*chemin.last().unwrap(), false, Modifiers::NONE)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    repos(&mut b);
    let img = b.capturer()?;
    sauver(&img, &format!("{nom}-2-repos"));
    let repos_r = relever(&img, de, a);
    let etapes = b.app.history.undo_len() - etapes_avant;
    let rejeux = crate::vector::replay_count() - rejeux;
    let apres_plan = encre_du_plan(&b.app.doc, ids[PLAN]);
    let dans_le_plan = apres_plan != avant_plan && !(v == Variante::Ecarte);
    let hors_cadres = traits_hors_cadres(&b.app.doc) - graine;
    let statut = if statut_geste.is_empty() { b.app.status.clone() } else { format!("{statut_geste} / {}", b.app.status) };
    // Ce qui était dessiné n'a pas bougé : mêmes pixels aux mêmes places (le trait neuf passe plus haut).
    let cel_apres = deja.and_then(|c| b.app.doc.cels.get(&c));
    let deja_intact = match (&deja_cel, cel_apres) {
        (Some(c0), Some(c1)) if v != Variante::AutreSorte => {
            let (mut total, mut pareils) = (0, 0);
            for (k, t) in c0.raster.tiles.iter().enumerate() {
                let Some(t) = t else { continue };
                let (ox, oy, tw, th) = c0.raster.tile_bounds(k);
                for y in 0..th {
                    for x in 0..tw {
                        let p = t[y * crate::raster::TILE + x];
                        if p.a() > 200 {
                            total += 1;
                            pareils += usize::from(c1.raster.get_i((ox + x) as i64, (oy + y) as i64) == p);
                        }
                    }
                }
            }
            total > 0 && pareils == total
        }
        _ => true,
    };
    // Enregistrer, rouvrir.
    let dossier = std::env::temp_dir().join(format!("shot-test-traits-caches-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dossier).ok();
    std::fs::create_dir_all(&dossier).unwrap();
    let manifeste = dossier.join("P.shot");
    crate::project::save(&b.app.doc, &manifeste, &mut Default::default()).expect("enregistrement");
    let relu = crate::project::load(&manifeste).expect("réouverture");
    let (zoom, pan) = (b.app.board_zoom, b.app.board_pan);
    let rouvert = {
        let mut c = planche(relu.doc, 6, 1.0)?;
        c.app.board_zoom = zoom;
        c.app.board_pan = pan;
        c.chauffer(4);
        c.attendre_vignettes();
        repos(&mut c);
        let img = c.capturer()?;
        sauver(&img, &format!("{nom}-3-rouvert"));
        relever(&img, de, a).trou
    };
    std::fs::remove_dir_all(&dossier).ok();
    // L'export de la planche (un plan écarté n'y est pas : le trait n'y a plus de sens de bout en bout).
    let export = if v == Variante::Ecarte {
        None
    } else {
        let opt = crate::planche::Options::default();
        let img = exporter(&mut b, &opt, &nom);
        if let Some(d) = std::env::var_os("FEANOR_CAPTURE_DIR") {
            let _ = img.save(std::path::PathBuf::from(d).join(format!("{nom}-4-export.png")));
        }
        Some(trou_a_l_export(&b, &img, &opt, bd, ba).0)
    };
    // Annuler rend les données d'avant.
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    let annule_ok = encre_du_plan(&b.app.doc, ids[PLAN]) == avant_plan && traits_hors_cadres(&b.app.doc) == graine;
    Some(Constat { geste, repos: repos_r.trou, rouvert, export, dans_le_plan, hors_cadres, etapes, rejeux, statut, deja_intact, annule_ok })
}

/// Le tableau de la partie 122 : les mesures seules, sans jugement. `cargo test … mesure_les_variantes -- --ignored --nocapture --test-threads=1`
#[test]
#[ignore]
fn mesure_les_variantes() {
    println!("| variante | sorte | part | geste | repos | rouvert | export | dans le plan | hors cadres | étapes | rejeux | déjà intact | annuler | message |");
    for v in TOUTES {
        for bitmap in [false, true] {
            for depuis in [false, true] {
                if depuis && matches!(v, Variante::Verrou | Variante::Masque | Variante::Ecarte) {
                    continue;
                }
                let Some(c) = constater(v, bitmap, depuis) else { return println!("pas de carte graphique") };
                println!("| {v:?} | {} | {} | {} | {} | {} | {:?} | {} | {} | {} | {} | {} | {} | {} |", if bitmap { "bitmap" } else { "vecteur" }, if depuis { "depuis" } else { "travers" }, c.geste, c.repos, c.rouvert, c.export, c.dans_le_plan, c.hors_cadres, c.etapes, c.rejeux, c.deja_intact, c.annule_ok, c.statut);
            }
        }
    }
}

// ------------------------------------------------------------------------------ toute caméra reçoit

fn sorte(bitmap: bool) -> &'static str {
    if bitmap {
        "bitmap"
    } else {
        "vecteur"
    }
}

/// Partie 131 (avant : la case traversée recevait sa part du trait dans son plan, ou la gardait par-dessus elle si elle ne pouvait pas la recevoir).
/// Un trait parti de la case voisine et qui **traverse** la case du plan `PLAN`, quelle qu'elle soit (une autre caméra, des cadres, un calque
/// verrouillé, masqué, transformé, d'une autre sorte, un plan écarté) : il reste entier dans le plan où il commence, se voit en entier sur la
/// planche, par-dessus la case traversée, qui ne reçoit rien ; rien ne va sur le papier, et il n'y a rien à dire.
fn verifier_traverse(v: Variante, bitmap: bool) {
    let Some(c) = constater(v, bitmap, false) else { return println!("pas de carte graphique") };
    let nom = format!("{v:?} {} à travers", sorte(bitmap));
    println!("TRAIT {nom} : {c:?}");
    assert!(c.geste <= TROU_PERMIS, "{nom} : pendant le geste : {c:?}");
    assert!(c.repos <= TROU_PERMIS, "{nom} : au repos, rien n'a disparu : {c:?}");
    assert!(c.rouvert <= TROU_PERMIS, "{nom} : après réouverture : {c:?}");
    if let Some(t) = c.export {
        assert!(t <= TROU_PERMIS, "{nom} : à l'export : {c:?}");
    }
    assert!(!c.dans_le_plan, "{nom} : rien n'entre dans le plan de la case traversée : {c:?}");
    assert_eq!(c.hors_cadres, 0, "{nom} : rien sur la planche, le trait est entier dans son plan : {c:?}");
    assert_eq!(c.etapes, 1, "{nom} : une seule étape : {c:?}");
    assert_eq!(c.rejeux, 0, "{nom} : aucun rejeu sur le fil de l'interface : {c:?}");
    assert!(c.deja_intact, "{nom} : ce qui était dessiné ne bouge pas : {c:?}");
    assert!(c.annule_ok, "{nom} : annuler rend tout : {c:?}");
    assert!(c.statut.is_empty(), "{nom} : rien à dire : {c:?}");
}

/// Partie 131 : un trait **parti** de la case du plan `PLAN`, quelle que soit sa caméra, lui appartient tout entier : il est dans son plan, se
/// voit en entier sur la planche jusque deux cases plus loin, rien ne va sur le papier.
///
/// **Ce qui reste** (noté dans la partie 131) : une caméra en panoramique ou basculée montre sa feuille en perspective, et le trait est dessiné
/// sur cette feuille. Loin du cadre, du côté où la feuille fuit, il s'amincit jusqu'à moins d'un pixel d'écran : il n'est pas perdu (il est entier
/// dans le dessin, et l'export, plus fin, le montre), mais la planche ne le montre plus d'un trait continu. Avant, sa part hors de la case était
/// redessinée à plat dans les cases voisines ou sur la planche.
fn verifier_depuis(v: Variante, bitmap: bool) {
    let Some(c) = constater(v, bitmap, true) else { return println!("pas de carte graphique") };
    let nom = format!("{v:?} {} depuis la case", sorte(bitmap));
    println!("TRAIT {nom} : {c:?}");
    let en_perspective = matches!(v, Variante::Bascule | Variante::Panoramique);
    assert!(c.geste <= TROU_PERMIS, "{nom} : pendant le geste : {c:?}");
    if !en_perspective {
        assert!(c.repos <= TROU_PERMIS, "{nom} : au repos : {c:?}");
        assert!(c.rouvert <= TROU_PERMIS, "{nom} : après réouverture : {c:?}");
    }
    assert!(c.export.is_some_and(|t| t <= TROU_PERMIS), "{nom} : à l'export : {c:?}");
    assert!(c.dans_le_plan, "{nom} : le trait est dans le plan de sa case : {c:?}");
    assert_eq!(c.hors_cadres, 0, "{nom} : rien sur la planche : {c:?}");
    assert_eq!(c.etapes, 1, "{nom} : une seule étape : {c:?}");
    assert_eq!(c.rejeux, 0, "{nom} : aucun rejeu sur le fil de l'interface : {c:?}");
    assert!(c.deja_intact, "{nom} : ce qui était dessiné ne bouge pas : {c:?}");
    assert!(c.annule_ok, "{nom} : annuler rend tout : {c:?}");
    assert!(c.statut.is_empty(), "{nom} : rien à dire : {c:?}");
}

#[test]
fn a_stroke_across_or_from_a_case_with_another_zoom_stays_whole_in_the_plan_where_it_starts() {
    for v in [Variante::Zoom, Variante::ZoomFaible] {
        for bitmap in [false, true] {
            verifier_traverse(v, bitmap);
            verifier_depuis(v, bitmap);
        }
    }
}

#[test]
fn a_stroke_across_or_from_a_case_with_a_rolled_camera_stays_whole_in_the_plan_where_it_starts() {
    for bitmap in [false, true] {
        verifier_traverse(Variante::Roulis, bitmap);
        verifier_depuis(Variante::Roulis, bitmap);
    }
}

#[test]
fn a_stroke_across_or_from_a_case_with_a_tilted_or_panned_camera_stays_whole_in_the_plan_where_it_starts() {
    for v in [Variante::Bascule, Variante::Panoramique] {
        for bitmap in [false, true] {
            verifier_traverse(v, bitmap);
            verifier_depuis(v, bitmap);
        }
    }
}

#[test]
fn a_stroke_across_or_from_a_plan_with_frames_a_and_b_stays_whole_in_the_plan_where_it_starts() {
    for v in [Variante::Cadres, Variante::CadresZoom, Variante::CadresRoulis] {
        for bitmap in [false, true] {
            verifier_traverse(v, bitmap);
            verifier_depuis(v, bitmap);
        }
    }
}

/// Le plan a vraiment le trait, au bon endroit : l'image du film, à travers la caméra de la case, montre le trait là où la case le montre.
/// (Partie 131 : le trait part du bord gauche de la case, dedans, et sort à droite : il est à elle, tout entier.)
#[test]
fn the_film_frame_of_the_plan_shows_the_stroke_where_the_case_shows_it() {
    for v in [Variante::Zoom, Variante::ZoomFaible, Variante::Roulis, Variante::Bascule, Variante::Panoramique] {
        for bitmap in [false, true] {
            let mut doc = des_plans(18, bitmap);
            for p in 0..doc.panels.len() {
                doc.set_duration(p, 24);
            }
            let Some(mut b) = planche(doc, 6, 1.0) else { return println!("pas de carte graphique") };
            appliquer(&mut b, v, bitmap);
            b.chauffer(4);
            let id = b.app.doc.panels[PLAN].id;
            let case = case_ecran(&b, id);
            let (rd, ra) = (case_ecran(&b, b.app.doc.panels[PLAN - 1].id), case_ecran(&b, b.app.doc.panels[PLAN + 1].id));
            let y = rd.top() + rd.height() * 0.35;
            tracer(&mut b, pos2(case.left() + 1.0, y), pos2(ra.center().x, y));
            repos(&mut b);
            let doc = &b.app.doc;
            let snap = crate::exportrender::snapshot_of(doc, &[PLAN]);
            let rendu = crate::exportrender::Rendu::new(snap, crate::exportrender::Papier::Feuille, false, Vec::new(), Default::default());
            let img = rendu.image(PLAN, 0).expect("l'image");
            let [w, h] = doc.frame;
            if let Some(d) = std::env::var_os("FEANOR_CAPTURE_DIR") {
                let rgba: Vec<u8> = img.iter().flat_map(|c| c.to_array()).collect();
                let _ = image::save_buffer(std::path::PathBuf::from(d).join(format!("film-{v:?}-{}.png", sorte(bitmap))), &rgba, w as u32, h as u32, image::ColorType::Rgba8);
            }
            // La hauteur du trait dans l'image de la case, en pixels du film : d'un bord à l'autre de l'image.
            let e = case.width() / w as f32;
            let yf = (y - case.top()) / e;
            let (mut vus, mut encre) = (0, 0);
            for x in (16..w as i32 - 16).step_by(4) {
                vus += 1;
                encre += usize::from((-10..=10).any(|j| {
                    let c: Color32 = img[(yf as i32 + j).clamp(0, h as i32 - 1) as usize * w + x as usize];
                    c.r() as i32 - c.g() as i32 > 40
                }));
            }
            println!("TRAIT film {v:?} {} : encre {encre} sur {vus}", sorte(bitmap));
            assert_eq!(encre, vus, "{v:?} {} : le film montre le trait d'un bord à l'autre", sorte(bitmap));
        }
    }
}

// ------------------------------------------------------------------------------ la case traversée ne reçoit rien, jamais caché

#[test]
fn a_stroke_across_a_locked_layer_writes_nothing_and_stays_whole_and_visible() {
    for bitmap in [false, true] {
        verifier_traverse(Variante::Verrou, bitmap);
    }
}

#[test]
fn a_stroke_across_a_hidden_layer_writes_nothing_and_stays_whole_and_visible() {
    for bitmap in [false, true] {
        verifier_traverse(Variante::Masque, bitmap);
    }
}

#[test]
fn a_stroke_across_a_plan_without_a_layer_of_its_kind_stays_whole_and_visible() {
    verifier_traverse(Variante::AutreSorte, false);
    verifier_traverse(Variante::AutreSorte, true);
}

#[test]
fn a_stroke_across_a_transformed_layer_stays_whole_and_visible() {
    for bitmap in [false, true] {
        verifier_traverse(Variante::CalqueTransforme, bitmap);
    }
}

#[test]
fn a_stroke_across_a_cancelled_plan_stays_whole_and_visible() {
    for bitmap in [false, true] {
        verifier_traverse(Variante::Ecarte, bitmap);
    }
}

/// Un point du papier, loin de toute case : le coin haut gauche de la vue (vérifié).
fn coin_du_papier(b: &Banc) -> Pos2 {
    let view = b.app.vignettes.view;
    let a = pos2(view.left() + 6.0, view.top() + 6.0);
    let l = b.app.board_layout();
    let champs = crate::horschamp::champs(&l, b.app.settings.vignettes.debord_fraction());
    let (p, q) = (b.app.vignettes.xf.to_board(a), b.app.vignettes.xf.to_board(a + eframe::egui::vec2(80.0, 40.0)));
    assert!(champs.iter().all(|c| !c.contains(p) && !c.contains(q)), "le coin est hors de tout champ");
    a
}

/// Un trait droit **coupé au lâcher comme avant la partie 131** (le trait d'un projet d'avant : ses parts sur les cases qui ne pouvaient pas les
/// recevoir sont par-dessus elles), puis le repos.
fn tracer_coupe(b: &mut Banc, de: Pos2, a: Pos2) {
    crate::vignettes_selection_tests::tracer_comme_avant(b, de, a);
}

/// À chaque image d'écran depuis le lâcher, le trait reste entier : il n'apparaît pas pour disparaître au lever.
#[test]
fn at_every_screen_image_after_the_release_the_stroke_stays_whole() {
    for v in [Variante::Verrou, Variante::Zoom, Variante::Roulis, Variante::Bascule] {
        for bitmap in [false, true] {
            let mut doc = des_plans(18, bitmap);
            for p in 0..doc.panels.len() {
                doc.set_duration(p, 24);
            }
            let Some(mut b) = planche(doc, 6, 1.0) else { return println!("pas de carte graphique") };
            appliquer(&mut b, v, bitmap);
            b.chauffer(4);
            b.attendre_vignettes();
            repos(&mut b);
            let (rd, ra) = (case_ecran(&b, b.app.doc.panels[PLAN - 1].id), case_ecran(&b, b.app.doc.panels[PLAN + 1].id));
            let y = rd.top() + rd.height() * 0.35;
            let (de, a) = (pos2(rd.center().x, y), pos2(ra.center().x, y));
            let n = ((a - de).length() / 6.0).ceil() as usize;
            let chemin: Vec<Pos2> = (0..=n).map(|k| de + (a - de) * (k as f32 / n as f32)).collect();
            b.image(vec![Event::PointerMoved(chemin[0])], 1.0 / 60.0);
            b.image(vec![appui(chemin[0], true, Modifiers::NONE)], 1.0 / 60.0);
            for q in &chemin[1..] {
                b.image(vec![Event::PointerMoved(*q)], 1.0 / 120.0);
            }
            b.image(vec![appui(*chemin.last().unwrap(), false, Modifiers::NONE)], 1.0 / 60.0);
            let mut pire = 0;
            for k in 0..240 {
                b.image(Vec::new(), 1.0 / 60.0);
                let r = relever(&b.capturer().unwrap(), de, a);
                pire = pire.max(r.trou);
                assert!(r.trou <= TROU_PERMIS, "{v:?} {} : image {k} après le lâcher : {r:?}", sorte(bitmap));
                std::thread::sleep(std::time::Duration::from_millis(2));
                if k > 30 && !b.app.vignettes_en_cours() && !b.app.rejeu.has_pending() {
                    break;
                }
            }
            println!("TRAIT après le lâcher {v:?} {} : pire trou {pire}", sorte(bitmap));
        }
    }
}

/// Le calque verrouillé n'a pas bougé d'un pixel ; une fois déverrouillé, un nouveau trait y entre normalement.
#[test]
fn a_locked_layer_keeps_every_pixel_and_receives_again_once_unlocked() {
    for bitmap in [false, true] {
        let mut doc = des_plans(18, bitmap);
        for p in 0..doc.panels.len() {
            doc.set_duration(p, 24);
        }
        let Some(mut b) = planche(doc, 6, 1.0) else { return println!("pas de carte graphique") };
        let id = b.app.doc.panels[PLAN].id;
        let r = case_ecran(&b, id);
        tracer(&mut b, pos2(r.left() + r.width() * 0.3, r.bottom() - r.height() * 0.15), pos2(r.left() + r.width() * 0.7, r.bottom() - r.height() * 0.15));
        repos(&mut b);
        let avant = encre_du_plan(&b.app.doc, id);
        let empreinte = |b: &Banc| {
            let c = b.app.doc.panels[PLAN].layers[0].cel_at(0).expect("le dessin");
            crate::empreinte::Donnees::de(b.app.doc.cels.get(&c).expect("le dessin")).empreinte()
        };
        let e0 = empreinte(&b);
        appliquer(&mut b, Variante::Verrou, bitmap);
        let (rd, ra) = (case_ecran(&b, b.app.doc.panels[PLAN - 1].id), case_ecran(&b, b.app.doc.panels[PLAN + 1].id));
        let y = rd.top() + rd.height() * 0.35;
        tracer(&mut b, pos2(rd.center().x, y), pos2(ra.center().x, y));
        repos(&mut b);
        assert_eq!(encre_du_plan(&b.app.doc, id), avant, "{} : rien n'est écrit dans le calque verrouillé", sorte(bitmap));
        assert_eq!(empreinte(&b), e0, "{}", sorte(bitmap));
        // Poser le stylet dans la case verrouillée : rien non plus (on n'y dessine pas), et rien ne va sur la planche à sa place.
        let y = r.top() + r.height() * 0.6;
        tracer(&mut b, pos2(r.center().x - 20.0, y), pos2(r.center().x + 20.0, y));
        repos(&mut b);
        assert_eq!(encre_du_plan(&b.app.doc, id), avant, "{} : on ne dessine pas dans le calque verrouillé", sorte(bitmap));
        assert_eq!(traits_hors_cadres(&b.app.doc), 0, "{} : rien sur la planche", sorte(bitmap));
        // Déverrouillé : un trait posé dans sa case y entre normalement. (Partie 131 : un trait qui la traverse, parti d'ailleurs, n'y entre plus.)
        b.app.doc.panels[PLAN].layers[0].locked = false;
        b.chauffer(2);
        tracer(&mut b, pos2(r.center().x - 20.0, y), pos2(r.center().x + 20.0, y));
        repos(&mut b);
        assert_ne!(encre_du_plan(&b.app.doc, id), avant, "{} : déverrouillé, il reçoit", sorte(bitmap));
    }
}

/// La part posée par-dessus la case (un trait d'avant la partie 131, coupé ; depuis, un trait qu'un déplacement mène sur une case qui ne peut pas
/// le recevoir) : la gomme l'efface d'un geste, sans clic droit et sans toucher au dessin du plan.
#[test]
fn the_eraser_removes_a_part_left_on_top_of_a_case_and_nothing_else() {
    for bitmap in [false, true] {
        let mut doc = des_plans(18, bitmap);
        for p in 0..doc.panels.len() {
            doc.set_duration(p, 24);
        }
        let Some(mut b) = planche(doc, 6, 1.0) else { return println!("pas de carte graphique") };
        appliquer(&mut b, Variante::Verrou, bitmap);
        let id = b.app.doc.panels[PLAN].id;
        let case = case_ecran(&b, id);
        let (rd, ra) = (case_ecran(&b, b.app.doc.panels[PLAN - 1].id), case_ecran(&b, b.app.doc.panels[PLAN + 1].id));
        let y = rd.top() + rd.height() * 0.35;
        tracer_coupe(&mut b, pos2(rd.center().x, y), pos2(ra.center().x, y));
        repos(&mut b);
        assert_eq!(dessus(&b), 1, "{}", sorte(bitmap));
        let plan = encre_du_plan(&b.app.doc, id);
        let etapes = b.app.history.undo_len();
        b.app.set_tool(crate::app::Tool::Eraser);
        b.chauffer(1);
        let x = case.center().x;
        tracer(&mut b, pos2(x, y), pos2(x + 12.0, y));
        b.chauffer(2);
        assert_eq!(dessus(&b), 0, "{} : la gomme l'a effacée", sorte(bitmap));
        assert_eq!(encre_du_plan(&b.app.doc, id), plan);
        assert_eq!(b.app.history.undo_len(), etapes + 1, "une étape");
        b.app.run_action("edition.annuler");
        b.chauffer(2);
        assert_eq!(dessus(&b), 1, "{} : annuler la rend", sorte(bitmap));
    }
}

/// Reprendre le plan écarté : la part reste visible, par-dessus sa case, et le plan n'a rien reçu.
#[test]
fn a_stroke_across_a_cancelled_plan_stays_visible_after_the_plan_comes_back() {
    let Some(mut b) = planche(des_plans(18, false), 6, 1.0) else { return println!("pas de carte graphique") };
    let id = b.app.doc.panels[PLAN].id;
    appliquer(&mut b, Variante::Ecarte, false);
    b.chauffer(4);
    repos(&mut b);
    let (rd, ra) = (case_ecran(&b, b.app.doc.panels[PLAN - 1].id), case_ecran(&b, b.app.doc.panels[PLAN].id));
    let y = rd.top() + rd.height() * 0.35;
    let (de, a) = (pos2(rd.center().x, y), pos2(ra.center().x, y));
    tracer(&mut b, de, a);
    repos(&mut b);
    assert!(b.app.plan_ecarter_ou_reprendre(id, false));
    b.chauffer(4);
    b.attendre_vignettes();
    repos(&mut b);
    let img = b.capturer().unwrap();
    sauver(&img, "ecarte-repris");
    let r = relever(&img, de, a);
    println!("TRAIT plan repris : {r:?}");
    assert!(r.trou <= TROU_PERMIS, "le plan repris ne cache pas la part : {r:?}");
    assert_eq!(encre_du_plan(&b.app.doc, id), (0, 0), "le plan n'a rien reçu");
}

/// Un grand débord (40 %) et une case verrouillée à côté : le trait parti de la case voisine reste entier dans son plan et se voit jusque dans
/// la case verrouillée, par-dessus son image ; rien n'est posé sur la planche. (Partie 131 ; avant : sa part sur la case verrouillée était par-dessus.)
#[test]
fn a_stroke_over_a_locked_case_with_wide_margins_stays_whole_and_visible() {
    let Some(mut b) = planche(des_plans(18, false), 6, 1.0) else { return println!("pas de carte graphique") };
    appliquer(&mut b, Variante::Verrou, false);
    b.app.settings.vignettes.debord = 40;
    b.chauffer(4);
    let (rd, ra) = (case_ecran(&b, b.app.doc.panels[PLAN - 1].id), case_ecran(&b, b.app.doc.panels[PLAN].id));
    let y = rd.top() + rd.height() * 0.35;
    let (de, a) = (pos2(rd.center().x, y), pos2(ra.center().x, y));
    tracer(&mut b, de, a);
    repos(&mut b);
    let img = b.capturer().unwrap();
    let r = relever(&img, de, a);
    println!("TRAIT sur case verrouillée, grand débord : {r:?}");
    assert!(r.trou <= TROU_PERMIS, "le trait se voit en entier, par-dessus l'image de la case verrouillée : {r:?}");
    assert_eq!(traits_hors_cadres(&b.app.doc), 0, "rien sur la planche");
    assert_eq!(encre_du_plan(&b.app.doc, b.app.doc.panels[PLAN - 1].id).0, 1, "le trait est entier dans le plan où il commence");
    assert_eq!(encre_du_plan(&b.app.doc, b.app.doc.panels[PLAN].id), (0, 0), "la case verrouillée n'a rien reçu");
}

/// À l'export, la part posée sur une case écartée part avec elle : le plan est absent, elle ne flotte pas sur la planche exportée.
#[test]
fn the_part_on_a_cancelled_plan_is_not_in_the_board_export() {
    let Some(mut b) = planche(des_plans(18, false), 6, 1.0) else { return println!("pas de carte graphique") };
    appliquer(&mut b, Variante::Ecarte, false);
    b.chauffer(4);
    let (rd, ra) = (case_ecran(&b, b.app.doc.panels[PLAN - 1].id), case_ecran(&b, b.app.doc.panels[PLAN].id));
    let y = rd.top() + rd.height() * 0.35;
    // (Un trait d'avant la partie 131, coupé : sa part sur la case écartée est par-dessus elle.)
    tracer_coupe(&mut b, pos2(rd.center().x, y), pos2(ra.center().x, y));
    repos(&mut b);
    let i = b.app.doc.annotations.traces.iter().position(|t| t.dessus).expect("la part sur la case écartée");
    let opt = b.app.planche_options(&crate::planche::Options::default());
    let plans: Vec<usize> = (0..b.app.doc.panels.len()).collect();
    let p = crate::planche::construire(&b.app.doc, &plans, &b.app.layout.board_settings(), &opt, &b.app.doc.annotations);
    assert!(p.annotations.points_planche(&p.layout, i).is_none(), "elle n'a pas de place sur la planche exportée");
    // Exporter ne plante pas, et l'écran la garde.
    let _ = exporter(&mut b, &crate::planche::Options::default(), "ecarte-export");
    assert!(b.app.doc.annotations.points_planche(&b.app.board_layout(), i).is_some());
}

/// Un vrai projet (une copie), `FEANOR_PROJET_ESSAI` : le trait à travers une case à roulis et une case au calque verrouillé.
#[test]
#[ignore]
fn sur_une_copie_d_un_vrai_projet() {
    let Some(chemin) = std::env::var_os("FEANOR_PROJET_ESSAI") else { return };
    let relu = crate::project::load(std::path::Path::new(&chemin)).expect("le projet s'ouvre");
    let Some(mut b) = planche(relu.doc, 6, 1.0) else { return };
    assert!(b.app.doc.panels.len() >= 3, "{} plans", b.app.doc.panels.len());
    let cam = b.app.doc.default_camera();
    crate::camera::set_key(&mut b.app.doc.panels[1].camera, 0, crate::camera::Camera { angle: 15.0, ..cam });
    b.chauffer(4);
    b.attendre_vignettes();
    repos(&mut b);
    let (rd, ra) = (case_ecran(&b, b.app.doc.panels[0].id), case_ecran(&b, b.app.doc.panels[2].id));
    let y = rd.top() + rd.height() * 0.3;
    let (de, a) = (pos2(rd.center().x, y), pos2(ra.center().x, y));
    let n = b.app.history.undo_len();
    tracer(&mut b, de, a);
    repos(&mut b);
    let img = b.capturer().unwrap();
    sauver(&img, "vrai-projet");
    println!("VRAI PROJET : {} plans, relevé {:?}, étapes {}, message {:?}", b.app.doc.panels.len(), relever(&img, de, a), b.app.history.undo_len() - n, b.app.status);
}

// ------------------------------------------------------------------------------ relecture : aucun dessin perdu, aucun message faux

/// Une planche de 18 plans, plan `PLAN` dans la variante `v`, et le trait (écran) de la case d'avant à celle d'après.
fn scene_refus(v: Variante, bitmap: bool) -> Option<(Banc, Pos2, Pos2)> {
    let mut doc = des_plans(18, bitmap);
    for p in 0..doc.panels.len() {
        doc.set_duration(p, 24);
    }
    let mut b = planche(doc, 6, 1.0)?;
    appliquer(&mut b, v, bitmap);
    b.chauffer(4);
    b.attendre_vignettes();
    repos(&mut b);
    let (rd, ra) = (case_ecran(&b, b.app.doc.panels[PLAN - 1].id), case_ecran(&b, b.app.doc.panels[PLAN + 1].id));
    let y = rd.top() + rd.height() * 0.35;
    Some((b, pos2(rd.center().x, y), pos2(ra.center().x, y)))
}

fn main_baissee(b: &mut Banc, de: Pos2, a: Pos2) {
    let n = ((a - de).length() / 6.0).ceil().max(2.0) as usize;
    b.image(vec![Event::PointerMoved(de)], 1.0 / 60.0);
    b.image(vec![appui(de, true, Modifiers::NONE)], 1.0 / 60.0);
    for k in 1..=n {
        b.image(vec![Event::PointerMoved(de + (a - de) * (k as f32 / n as f32))], 1.0 / 120.0);
    }
}

fn dessus(b: &Banc) -> usize {
    b.app.doc.annotations.traces.iter().filter(|t| t.dessus).count() + b.app.doc.annotations.bitmap.morceaux.iter().filter(|m| m.dessus).count()
}

/// Un très faible zoom dans la case d'arrivée (sa feuille y est 66 fois plus petite qu'à l'origine) : la tuile d'arrivée dépasserait 2048 pixels. Aucun pixel n'est perdu, le trait reste entier au repos, et il est quelque part.
#[test]
fn a_part_that_would_need_a_huge_tile_is_never_lost() {
    let Some(mut b) = planche(des_plans(18, true), 6, 1.0) else { return println!("pas de carte graphique") };
    let cam = b.app.doc.default_camera();
    crate::camera::set_key(&mut b.app.doc.panels[PLAN].camera, 0, crate::camera::Camera { zoom: 0.015, ..cam });
    b.chauffer(4);
    b.attendre_vignettes();
    let (rd, ra) = (case_ecran(&b, b.app.doc.panels[PLAN - 1].id), case_ecran(&b, b.app.doc.panels[PLAN + 1].id));
    let y = rd.top() + rd.height() * 0.35;
    let (de, a) = (pos2(rd.center().x, y), pos2(ra.center().x, y));
    let n = b.app.history.undo_len();
    tracer(&mut b, de, a);
    repos(&mut b);
    let img = b.capturer().unwrap();
    sauver(&img, "tuile-enorme");
    let r = relever(&img, de, a);
    println!("TRAIT tuile énorme : {r:?}, part sur la case {}, encre du plan {:?}, message {:?}", dessus(&b), encre_du_plan(&b.app.doc, b.app.doc.panels[PLAN].id), b.app.status);
    assert!(r.trou <= TROU_PERMIS, "rien n'est perdu : {r:?}");
    assert_eq!(b.app.history.undo_len(), n + 1);
}

/// Le coût du lever sur le fil de l'interface, à fort rapport de zoom. (Mesure.)
#[test]
#[ignore]
fn mesure_le_cout_du_lever_a_fort_zoom() {
    for taille in [14.0, 60.0] {
        for zoom in [0.6_f32, 0.25, 0.125, 0.0625, 0.04, 0.03, 0.015] {
            let Some(mut b) = planche(des_plans(18, true), 6, 1.0) else { return };
            b.app.brush.size = taille;
            let cam = b.app.doc.default_camera();
            crate::camera::set_key(&mut b.app.doc.panels[PLAN].camera, 0, crate::camera::Camera { zoom, ..cam });
            b.chauffer(4);
            let (rd, ra) = (case_ecran(&b, b.app.doc.panels[PLAN - 1].id), case_ecran(&b, b.app.doc.panels[PLAN + 1].id));
            let (de, a) = (rd.center() - eframe::egui::vec2(0.0, 12.0), ra.center() + eframe::egui::vec2(0.0, 12.0));
            main_baissee(&mut b, de, a);
            let mut pire = 0.0_f64;
            let m = b.image(vec![appui(a, false, Modifiers::NONE)], 1.0 / 60.0);
            pire = pire.max(m.construction);
            for _ in 0..3 {
                pire = pire.max(b.image(Vec::new(), 1.0 / 60.0).construction);
            }
            println!("COUT lever pinceau {taille} rapport {:.0} : pire image {pire:.1} ms, part sur la case {}, encre du plan {:?}", 1.0 / zoom, dessus(&b), encre_du_plan(&b.app.doc, b.app.doc.panels[PLAN].id));
        }
    }
}

/// La planche a atteint `MAX_TRACES` : un trait parti du papier ne peut pas y être posé, il reste où il a été tracé (le dessin de la case la plus
/// proche, hors de son cadre), et le message dit la vérité. (Partie 131 : seul un trait parti du papier va sur la planche.)
#[test]
fn when_the_board_is_full_a_paper_stroke_stays_in_the_drawing_where_it_was_traced_and_the_message_says_so() {
    for bitmap in [false, true] {
        let Some((mut b, _, _)) = scene_refus(Variante::Verrou, bitmap) else { return println!("pas de carte graphique") };
        let de = coin_du_papier(&b);
        let a = de + eframe::egui::vec2(80.0, 40.0);
        let l = b.app.board_layout();
        let carte = l.cards[0].id;
        let calque = b.app.doc.annotations.calque_hors_cadres();
        if bitmap {
            // (Partie 127 : la planche d'un trait bitmap est sa couche bitmap ; pleine, elle a `MAX_MORCEAUX` morceaux.)
            let mut r = crate::planchebitmap::pixels_vides();
            crate::planchebitmap::poser_pixel(&mut r, 0, 0, eframe::egui::Color32::BLACK, false);
            let r = std::sync::Arc::new(r);
            for i in 0..crate::planchebitmap::MAX_MORCEAUX {
                b.app.doc.annotations.bitmap.morceaux.push(crate::planchebitmap::Morceau {
                    id: i as u64 + 1,
                    anchor: crate::annotations::Ancre { card: carte, after: 0, rank: 0 },
                    dessus: false,
                    lien: None,
                    // (Dans la vue, sous la première rangée : 5 000 morceaux à peindre à chaque image ; les vignettes doivent se refaire quand même.)
                    pose: crate::planchebitmap::A64::echelle_decalage(0.0001, 0.0, 3.0),
                    pixels: r.clone(),
                });
            }
        } else {
            for _ in 0..crate::annotations::MAX_TRACES {
                b.app.doc.annotations.traces.push(crate::annotations::Trace::new(calque, crate::annotations::Ancre { card: carte, after: 0, rank: 0 }, [0, 0, 0], vec![crate::vector::VPoint { x: 0.0, y: 3.0, r: 0.001 }]));
            }
        }
        let milieu = b.app.vignettes.xf.to_board(de + (a - de) * 0.5);
        let aff = crate::horschamp::aff_de_la_case(&b.app.doc, &l, 0).expect("caméra droite");
        let sur_la_feuille = pos2((milieu.x - aff.tx) / aff.a, (milieu.y - aff.ty) / aff.a);
        tracer(&mut b, de, a);
        repos(&mut b);
        if bitmap {
            assert_eq!(b.app.doc.annotations.bitmap.morceaux.len(), crate::planchebitmap::MAX_MORCEAUX, "{}", sorte(bitmap));
        } else {
            assert_eq!(b.app.doc.annotations.traces.len(), crate::annotations::MAX_TRACES, "{}", sorte(bitmap));
        }
        println!("MESSAGE planche pleine : {}", b.app.status);
        assert!(!b.app.status.contains("reste sur la planche"), "{} : le message ment : {}", sorte(bitmap), b.app.status);
        assert!(b.app.status.contains("limite"), "{} : {}", sorte(bitmap), b.app.status);
        let cel = b.app.doc.panels[0].layers[0].cel_at(0).expect("le dessin d'origine");
        let c = b.app.doc.cels.get(&cel).expect("le dessin");
        let garde = if bitmap {
            (-20..=20).any(|j| c.raster.get_i(sur_la_feuille.x as i64, sur_la_feuille.y as i64 + j).a() > 0)
        } else {
            c.vector.as_ref().is_some_and(|v| v.elements.iter().any(|e| e.bounds().expand(20.0).contains(sur_la_feuille)))
        };
        assert!(garde, "{} : le trait est resté dans le dessin où il a été tracé", sorte(bitmap));
    }
}

/// Le calque « Hors cadres » caché : un trait du papier le remontre (sinon il disparaîtrait au lever), le message le dit, annuler le recache.
#[test]
fn a_hidden_off_frame_layer_comes_back_when_a_paper_stroke_lands_on_it() {
    for bitmap in [false, true] {
        let Some((mut b, _, _)) = scene_refus(Variante::Verrou, bitmap) else { return println!("pas de carte graphique") };
        let de = coin_du_papier(&b);
        let a = de + eframe::egui::vec2(80.0, 40.0);
        let id = b.app.doc.annotations.calque_hors_cadres();
        b.app.doc.annotations.regler_visible(id, false);
        tracer(&mut b, de, a);
        repos(&mut b);
        assert_eq!(traits_hors_cadres(&b.app.doc), 1, "{} : le trait est sur la planche, entier", sorte(bitmap));
        let img = b.capturer().unwrap();
        let r = relever(&img, de, a);
        assert!(r.trou <= TROU_PERMIS, "{} : le trait ne disparaît pas : {r:?}", sorte(bitmap));
        assert!(b.app.status.contains("Hors cadres"), "{} : {}", sorte(bitmap), b.app.status);
        b.app.run_action("edition.annuler");
        b.chauffer(2);
        assert!(!b.app.doc.annotations.calque(id).expect("le calque").visible, "annuler le recache");
    }
}

/// La gomme ne trouve pas ce qu'on ne voit pas : une part d'un calque « Hors cadres » caché ne s'efface pas, ni sur une case, ni sur le papier.
#[test]
fn the_eraser_does_not_remove_what_a_hidden_off_frame_layer_hides() {
    let Some((mut b, de, a)) = scene_refus(Variante::Verrou, false) else { return println!("pas de carte graphique") };
    // (Un trait d'avant la partie 131, coupé : des parts sur le papier et une par-dessus la case verrouillée.)
    tracer_coupe(&mut b, de, a);
    repos(&mut b);
    let avant = traits_hors_cadres(&b.app.doc);
    assert!(avant >= 2);
    let id = b.app.doc.annotations.id_hors_cadres().expect("le calque");
    b.app.doc.annotations.regler_visible(id, false);
    b.app.set_tool(crate::app::Tool::Eraser);
    b.chauffer(1);
    let case = case_ecran(&b, b.app.doc.panels[PLAN].id);
    tracer(&mut b, pos2(case.center().x, de.y), pos2(case.center().x + 12.0, de.y));
    let ecart = pos2(case.left() - 8.0, de.y);
    tracer(&mut b, ecart, ecart + eframe::egui::vec2(3.0, 1.0));
    b.chauffer(2);
    assert_eq!(traits_hors_cadres(&b.app.doc), avant, "ce qui est caché reste");
}

/// La gomme posée sur une part refusée n'efface que les parts refusées qu'elle touche : ni le reste du trait sur le papier, ni le dessin. Annuler et rétablir.
#[test]
fn the_eraser_on_a_refused_part_erases_only_refused_parts_and_undo_redo_follow() {
    for bitmap in [false, true] {
        let Some((mut b, de, a)) = scene_refus(Variante::Verrou, bitmap) else { return println!("pas de carte graphique") };
        // (Un trait d'avant la partie 131, coupé au lâcher.)
        tracer_coupe(&mut b, de, a);
        repos(&mut b);
        let avant = traits_hors_cadres(&b.app.doc);
        // (Vectoriel : trois tronçons, le papier de chaque côté et la case ; bitmap, partie 127 : un morceau pour le papier, un par-dessus la case.)
        assert_eq!((dessus(&b), avant), (1, if bitmap { 2 } else { 3 }), "{}", sorte(bitmap));
        let etapes = b.app.history.undo_len();
        b.app.set_tool(crate::app::Tool::Eraser);
        b.chauffer(1);
        // De l'intérieur de la case jusqu'au papier, à côté : le geste balaie aussi la part sur le papier.
        let case = case_ecran(&b, b.app.doc.panels[PLAN].id);
        tracer(&mut b, pos2(case.center().x, de.y), pos2(case.left() - 8.0, de.y));
        b.chauffer(2);
        assert_eq!(dessus(&b), 0, "{} : la part sur la case est partie", sorte(bitmap));
        assert_eq!(traits_hors_cadres(&b.app.doc), avant - 1, "{} : le reste du trait est là", sorte(bitmap));
        assert_eq!(b.app.history.undo_len(), etapes + 1, "{} : une étape", sorte(bitmap));
        b.app.run_action("edition.annuler");
        b.chauffer(2);
        assert_eq!((dessus(&b), traits_hors_cadres(&b.app.doc)), (1, avant), "{}", sorte(bitmap));
        b.app.run_action("edition.retablir");
        b.chauffer(2);
        assert_eq!((dessus(&b), traits_hors_cadres(&b.app.doc)), (0, avant - 1), "{} : rétablir", sorte(bitmap));
    }
}

/// Annuler puis rétablir le trait : la part refusée revient par-dessus la case, le plan n'a toujours rien reçu.
#[test]
fn undo_then_redo_of_a_stroke_brings_the_refused_part_back() {
    for bitmap in [false, true] {
        let Some((mut b, de, a)) = scene_refus(Variante::Verrou, bitmap) else { return println!("pas de carte graphique") };
        let id = b.app.doc.panels[PLAN].id;
        let plan = encre_du_plan(&b.app.doc, id);
        // (Un trait d'avant la partie 131, coupé au lâcher : sa part sur la case verrouillée est par-dessus elle.)
        tracer_coupe(&mut b, de, a);
        repos(&mut b);
        b.app.run_action("edition.annuler");
        repos(&mut b);
        assert_eq!((dessus(&b), traits_hors_cadres(&b.app.doc)), (0, 0), "{}", sorte(bitmap));
        b.app.run_action("edition.retablir");
        repos(&mut b);
        assert_eq!(dessus(&b), 1, "{}", sorte(bitmap));
        assert_eq!(encre_du_plan(&b.app.doc, id), plan, "{}", sorte(bitmap));
        let r = relever(&b.capturer().unwrap(), de, a);
        assert!(r.trou <= TROU_PERMIS, "{} : {r:?}", sorte(bitmap));
    }
}

/// Partie 131 (avant : chaque case qui ne pouvait pas recevoir sa part était annoncée une fois pendant le geste). Un trait qui traverse des cases
/// verrouillées ne leur donne rien et ne perd rien : il n'y a rien à annoncer, la ligne d'état reste muette.
#[test]
fn crossing_locked_cases_announces_nothing_during_a_gesture() {
    let Some(mut b) = planche(des_plans(18, false), 6, 1.0) else { return println!("pas de carte graphique") };
    b.app.doc.panels[1].layers[0].locked = true;
    b.app.doc.panels[2].layers[0].locked = true;
    b.chauffer(2);
    let cases: Vec<Rect> = (0..3).map(|k| case_ecran(&b, b.app.doc.panels[k].id)).collect();
    let y = cases[0].top() + cases[0].height() * 0.35;
    let etapes = [cases[0].center().x, cases[1].center().x, cases[2].center().x, cases[1].center().x];
    b.image(vec![Event::PointerMoved(pos2(etapes[0], y))], 1.0 / 60.0);
    b.image(vec![appui(pos2(etapes[0], y), true, Modifiers::NONE)], 1.0 / 60.0);
    b.app.status.clear();
    let mut dits: Vec<String> = Vec::new();
    for s in etapes.windows(2) {
        for k in 1..=30 {
            b.image(vec![Event::PointerMoved(pos2(s[0] + (s[1] - s[0]) * (k as f32 / 30.0), y))], 1.0 / 120.0);
            if !b.app.status.is_empty() && dits.last() != Some(&b.app.status) {
                dits.push(b.app.status.clone());
            }
        }
    }
    println!("MESSAGES {dits:?}");
    assert!(dits.is_empty(), "rien à annoncer : {dits:?}");
}

/// Annuler en plein geste : le trait part, rien n'est posé, et la ligne d'état ne dit pas qu'il reste sur la planche.
#[test]
fn undo_during_the_gesture_leaves_nothing_and_no_stale_message() {
    for bitmap in [false, true] {
        let Some((mut b, de, a)) = scene_refus(Variante::Verrou, bitmap) else { return println!("pas de carte graphique") };
        let id = b.app.doc.panels[PLAN].id;
        let plan = encre_du_plan(&b.app.doc, id);
        main_baissee(&mut b, de, a);
        assert!(!b.app.status.contains("reste sur la planche"), "rien n'est annoncé pendant le geste : {}", b.app.status);
        b.app.run_action("edition.annuler");
        b.chauffer(1);
        b.image(vec![appui(a, false, Modifiers::NONE)], 1.0 / 60.0);
        b.chauffer(3);
        println!("MESSAGE après annulation : {:?}", b.app.status);
        assert!(!b.app.status.contains("reste sur la planche"), "{} : message périmé : {}", sorte(bitmap), b.app.status);
        assert_eq!(traits_hors_cadres(&b.app.doc), 0, "{} : rien n'est posé", sorte(bitmap));
        assert_eq!(encre_du_plan(&b.app.doc, id), plan);
    }
}

/// Échap ne défait pas un trait du pinceau : il est posé, entier, dans le plan où il a commencé, en une étape ; rien par-dessus la case verrouillée
/// qu'il traverse, rien à dire. (Partie 131.)
#[test]
fn escape_during_the_gesture_keeps_the_stroke_whole_in_its_plan() {
    let Some((mut b, de, a)) = scene_refus(Variante::Verrou, false) else { return println!("pas de carte graphique") };
    let n = b.app.history.undo_len();
    main_baissee(&mut b, de, a);
    b.image(vec![Event::Key { key: eframe::egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    // (Échap pose le trait tel qu'il est, comme tout changement d'outil : il ne l'abandonne pas. Le reste du geste ne dessine plus.)
    println!("ÉCHAP : trait en cours {}", b.app.stroke.is_some());
    b.image(vec![appui(a, false, Modifiers::NONE)], 1.0 / 60.0);
    repos(&mut b);
    assert_eq!(b.app.history.undo_len(), n + 1);
    assert_eq!((dessus(&b), traits_hors_cadres(&b.app.doc)), (0, 0), "rien sur la planche");
    assert_eq!(encre_du_plan(&b.app.doc, b.app.doc.panels[PLAN - 1].id).0, 1, "le trait est entier dans le plan où il a commencé");
    assert!(!b.app.status.contains("reste sur la planche"), "{}", b.app.status);
}

/// Changer de plan en plein geste : pas de plantage, le calque verrouillé reste intact, et tout s'annule.
#[test]
fn changing_the_plan_during_the_gesture_loses_nothing_and_writes_nothing_in_a_locked_layer() {
    for bitmap in [false, true] {
        let Some((mut b, de, a)) = scene_refus(Variante::Verrou, bitmap) else { return println!("pas de carte graphique") };
        let id = b.app.doc.panels[PLAN].id;
        let plan = encre_du_plan(&b.app.doc, id);
        let etapes = b.app.history.undo_len();
        main_baissee(&mut b, de, a);
        b.app.run_action("panneau.suivant");
        b.chauffer(2);
        b.image(vec![appui(a, false, Modifiers::NONE)], 1.0 / 60.0);
        repos(&mut b);
        assert_eq!(encre_du_plan(&b.app.doc, id), plan, "{}", sorte(bitmap));
        let encre_ailleurs = encre_du_plan(&b.app.doc, b.app.doc.panels[0].id) != (0, 0);
        assert!(encre_ailleurs, "{} : le trait est dans son dessin d'origine", sorte(bitmap));
        while b.app.history.undo_len() > etapes {
            b.app.run_action("edition.annuler");
            b.chauffer(1);
        }
        assert_eq!(traits_hors_cadres(&b.app.doc), 0, "{}", sorte(bitmap));
        assert_eq!(encre_du_plan(&b.app.doc, b.app.doc.panels[0].id), (0, 0), "{} : tout s'annule", sorte(bitmap));
    }
}

/// Ouvrir un autre onglet en plein geste : la répartition d'un trait du projet d'avant n'écrit rien dans le projet ouvert.
#[test]
fn opening_another_tab_during_the_gesture_writes_nothing_in_it() {
    for bitmap in [false, true] {
        let Some((mut b, de, a)) = scene_refus(Variante::Verrou, bitmap) else { return println!("pas de carte graphique") };
        main_baissee(&mut b, de, a);
        assert!(b.app.nouvel_onglet(), "{}", b.app.status);
        assert!(b.app.charger_projet(des_plans(18, bitmap)));
        b.app.welcome = false;
        b.chauffer(3);
        b.image(vec![appui(a, false, Modifiers::NONE)], 1.0 / 60.0);
        b.chauffer(3);
        assert!(!b.app.status.contains("reste sur la planche"), "{} : le message du projet d'avant : {}", sorte(bitmap), b.app.status);
        assert_eq!(b.app.doc.annotations.traces.len(), 0, "{} : rien sur la planche de l'autre projet", sorte(bitmap));
        for p in &b.app.doc.panels {
            assert_eq!(encre_du_plan(&b.app.doc, p.id), (0, 0), "{} : rien dans ses plans", sorte(bitmap));
        }
    }
}

/// Film partagé : on n'écrit jamais dans le plan d'un autre. Un trait parti de mon plan qui entre dans la case d'un plan tenu par un autre reste
/// entier dans le mien (partie 131) : rien dans le sien, rien sur la planche (elle ne s'enregistre qu'avec le jeton du montage : rien à y perdre).
#[test]
fn in_a_shared_film_a_stroke_never_writes_in_a_plan_held_by_someone_else() {
    use crate::cooperation_tests::confirmer;
    let (mut b, mut lea, h, _) = crate::cooperation_ui_tests::app_partagee("traits-caches", 4);
    lea.prendre_plan(1).expect("libre");
    confirmer(&h);
    lea.tour();
    b.app.partage.as_ref().expect("session").pomper();
    b.image(Vec::new(), 0.05);
    b.app.cur_panel = 0;
    b.app.run_action("equipe.prendre_plan");
    assert!(!b.app.content_refuses(), "le plan 0 est à moi : {}", b.app.status);
    b.app.set_espace(crate::workspace::Espace::Vignettes);
    b.app.board_view = crate::app::BoardView::Panels;
    b.app.layout.board_columns = 4;
    b.app.layout.board_rows = 0;
    b.app.board_zoom = 0.0;
    b.app.vignettes.ouverture_a_faire = false;
    b.chauffer(4);
    b.app.set_tool(crate::app::Tool::Brush);
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    let tenu = encre_du_plan(&b.app.doc, ids[1]);
    let (rd, ra) = (case_ecran(&b, ids[0]), case_ecran(&b, ids[1]));
    let editable = b.app.layer_editable();
    println!("PARTAGE cases {rd:?} {ra:?}, vue {:?}, éditable {editable}", b.app.vignettes.view);
    // Du plan 0, le mien, jusqu'au milieu du plan 1, celui de Léa.
    tracer(&mut b, rd.center(), ra.center());
    b.chauffer(3);
    println!("PARTAGE : encre du plan 0 {:?}, plan courant {}, traits sur la planche {}, sur une case {}, message {:?}", encre_du_plan(&b.app.doc, ids[0]), b.app.cur_panel, traits_hors_cadres(&b.app.doc), dessus(&b), b.app.status);
    assert_eq!(encre_du_plan(&b.app.doc, ids[1]), tenu, "rien dans le plan de Léa");
    assert_ne!(encre_du_plan(&b.app.doc, ids[0]), (0, 0), "le trait est dans mon plan, entier");
    assert_eq!((traits_hors_cadres(&b.app.doc), dessus(&b)), (0, 0), "rien sur la planche");
}

/// La part sur une case garde le rang de sa case (le secours de `rattacher_manquantes` reste juste). (Un trait d'avant la partie 131, coupé au lâcher.)
#[test]
fn a_part_on_a_case_remembers_the_rank_of_its_case() {
    let Some((mut b, de, a)) = scene_refus(Variante::Verrou, false) else { return println!("pas de carte graphique") };
    let etapes = b.app.history.undo_len();
    crate::horschamp::COMME_AVANT_131.with(|c| c.set(true));
    main_baissee(&mut b, de, a);
    b.image(vec![appui(a, false, Modifiers::NONE)], 1.0 / 60.0);
    crate::horschamp::COMME_AVANT_131.with(|c| c.set(false));
    assert_eq!(b.app.history.undo_len(), etapes + 1, "réparti dès l'image du lâcher");
    let t = b.app.doc.annotations.traces.iter().find(|t| t.dessus).expect("la part");
    assert_eq!(t.anchor.rank as usize, PLAN);
}

/// L'encre de l'écran et celle de l'export, case par case : la part de l'une qu'on retrouve dans l'autre (à 3 pixels d'export près), dans les deux sens.
fn accord_ecran_export(b: &mut Banc, nom: &str) -> (f32, f32) {
    let img = b.capturer().unwrap();
    sauver(&img, &format!("accord-{nom}-ecran"));
    let opt = crate::planche::Options::default();
    let export = exporter(b, &opt, &format!("accord-{nom}"));
    if let Some(d) = std::env::var_os("FEANOR_CAPTURE_DIR") {
        let _ = export.save(std::path::PathBuf::from(d).join(format!("accord-{nom}-export.png")));
    }
    let opt = b.app.planche_options(&opt);
    let plans: Vec<usize> = (0..b.app.doc.panels.len()).collect();
    let p = crate::planche::construire(&b.app.doc, &plans, &b.app.layout.board_settings(), &opt, &b.app.doc.annotations);
    let (s, ..) = crate::planche::dimensions(p.cadre, opt.resolution);
    let xf = b.app.vignettes.xf;
    let vue = b.app.vignettes.view;
    let encre_e = |x: i32, y: i32| {
        let v = crate::traits_continus_tests::px(&img, x, y);
        v[0] - v[1] >= 40 && v[0] - v[2] >= 40
    };
    let encre_x = |q: Pos2| {
        (-3..=3).any(|j| (-3..=3).any(|i| {
            let (x, y) = (q.x as i32 + i, q.y as i32 + j);
            x >= 0 && y >= 0 && (x as u32) < export.width() && (y as u32) < export.height() && {
                let v = export.get_pixel(x as u32, y as u32);
                v[0] as i32 - v[1] as i32 >= 40 && v[0] as i32 - v[2] as i32 >= 40
            }
        }))
    };
    let vers_export = |q: Pos2| pos2((q.x - p.cadre.left()) * s, (q.y - p.cadre.top()) * s);
    let (mut vus_e, mut retrouves_e) = (0, 0);
    let l = b.app.board_layout();
    for c in &l.cards {
        let r = xf.rect(c.thumb).shrink(3.0).intersect(vue);
        for y in r.top() as i32..r.bottom() as i32 {
            for x in r.left() as i32..r.right() as i32 {
                if encre_e(x, y) {
                    vus_e += 1;
                    retrouves_e += usize::from(encre_x(vers_export(xf.to_board(pos2(x as f32, y as f32)))));
                }
            }
        }
    }
    // Dans l'autre sens : l'encre de l'export dans ces cases, à sa place à l'écran.
    let (mut vus_x, mut retrouves_x) = (0, 0);
    for c in &l.cards {
        let r = xf.rect(c.thumb).shrink(3.0).intersect(vue);
        let (a, z) = (vers_export(xf.to_board(r.min)), vers_export(xf.to_board(r.max)));
        for y in a.y as i32..z.y as i32 {
            for x in a.x as i32..z.x as i32 {
                if x < 0 || y < 0 || x as u32 >= export.width() || y as u32 >= export.height() {
                    continue;
                }
                let v = export.get_pixel(x as u32, y as u32);
                if v[0] as i32 - v[1] as i32 >= 40 && v[0] as i32 - v[2] as i32 >= 40 {
                    vus_x += 1;
                    let e = xf.to_screen(pos2(x as f32 / s + p.cadre.left(), y as f32 / s + p.cadre.top()));
                    retrouves_x += usize::from((-3..=3).any(|j| (-3..=3).any(|i| encre_e(e.x as i32 + i, e.y as i32 + j))));
                }
            }
        }
    }
    println!("ACCORD {nom} : écran {retrouves_e} sur {vus_e}, export {retrouves_x} sur {vus_x}");
    (retrouves_e as f32 / vus_e.max(1) as f32, retrouves_x as f32 / vus_x.max(1) as f32)
}

/// L'export d'un plan à cadres montre ce que montre l'écran : cadres droits (rien n'y change) et cadres tournés (la case montre la feuille).
#[test]
fn the_export_of_a_plan_with_frames_shows_what_the_screen_shows() {
    for v in [Variante::Temoin, Variante::Cadres, Variante::CadresZoom, Variante::CadresRoulis] {
        let mut doc = des_plans(18, false);
        for p in 0..doc.panels.len() {
            doc.set_duration(p, 24);
        }
        let Some(mut b) = planche(doc, 6, 1.0) else { return println!("pas de carte graphique") };
        b.app.brush.size = 28.0;
        // Un dessin qui reste dans la case, sans trait qui traverse.
        let r = case_ecran(&b, b.app.doc.panels[PLAN].id);
        tracer(&mut b, r.left_top() + r.size() * 0.3, r.left_top() + r.size() * 0.7);
        appliquer(&mut b, v, false);
        b.chauffer(4);
        b.attendre_vignettes();
        b.app.select_panel(0);
        repos(&mut b);
        let (e, x) = accord_ecran_export(&mut b, &format!("{v:?}"));
        assert!(e >= 0.98 && x >= 0.98, "{v:?} : écran {e}, export {x}");
    }
}

/// Une copie d'un vrai projet (`FEANOR_PROJET_ESSAI`) : l'export et l'écran s'accordent, et quels plans passent par la feuille. (Mesure.)
#[test]
#[ignore]
fn mesure_l_accord_ecran_export_sur_une_copie_d_un_vrai_projet() {
    let Some(chemin) = std::env::var_os("FEANOR_PROJET_ESSAI") else { return };
    let relu = crate::project::load(std::path::Path::new(&chemin)).expect("le projet s'ouvre");
    let Some(mut b) = planche(relu.doc, 6, 1.0) else { return };
    b.attendre_vignettes();
    repos(&mut b);
    let l = b.app.board_layout();
    for (k, c) in l.cards.iter().enumerate() {
        println!("CASE {k} plan {} : mouvement {}", c.panel, l.move_of(k).is_some());
    }
    let (e, x) = accord_ecran_export(&mut b, "vrai-projet");
    println!("VRAI PROJET accord : écran {e}, export {x}");
}

// ------------------------------------------------------------------------------ tous les plans écartés

/// Tout écarté : ce qui n'écrit pas dans le plan support reste possible.
#[test]
fn when_everything_is_cancelled_the_project_resolution_and_the_characters_still_work() {
    let Some(mut b) = tout_ecarte(false) else { return println!("pas de carte graphique") };
    let e = crate::projetui::ResolutionEdit { w: 1280, h: 720, fps: b.app.doc.fps, linked: true, scale: false, keep_seconds: true, interpolate: false };
    b.app.apply_resolution(e);
    assert_eq!(b.app.doc.frame, [1280, 720], "la résolution du projet change : {}", b.app.status);
    assert!(b.app.perso_ajouter().is_some(), "un personnage de plus : {}", b.app.status);
    assert!(b.app.doc.film_vide());
    // Ce qui écrirait dans le support est refusé, et le message le dit sans parler de dessin.
    let n = b.app.doc.panels[0].layers.len();
    b.app.status.clear();
    b.app.add_layer(crate::document::LayerKind::Bitmap);
    assert_eq!(b.app.doc.panels[0].layers.len(), n);
    println!("MESSAGE calque : {}", b.app.status);
    assert!(b.app.status.contains("Tous les plans sont écartés") && !b.app.status.contains("rien ne se dessine"), "{}", b.app.status);
}

/// La Fiche ne montre ni n'écrit le plan support.
#[test]
fn when_everything_is_cancelled_the_sheet_says_so_too() {
    let Some(mut b) = tout_ecarte(false) else { return println!("pas de carte graphique") };
    let ctx = eframe::egui::Context::default();
    let mut vu: Vec<String> = Vec::new();
    for _ in 0..2 {
        let out = ctx.run(eframe::egui::RawInput::default(), |c| {
            eframe::egui::CentralPanel::default().show(c, |ui| b.app.panel_inspector(ui));
        });
        vu = out.shapes.iter().filter_map(|s| if let eframe::egui::epaint::Shape::Text(t) = &s.shape { Some(t.galley.job.text.clone()) } else { None }).collect();
    }
    assert!(vu.iter().any(|t| t.contains(crate::planannuleui::TEXTE_FILM_VIDE)), "{vu:?}");
    assert!(!vu.iter().any(|t| t == "Dialogue"), "{vu:?}");
}

fn tout_ecarte(bitmap: bool) -> Option<Banc> {
    let mut b = planche(des_plans(3, bitmap), 3, 1.0)?;
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    for i in ids {
        assert!(b.app.plan_ecarter_ou_reprendre(i, true));
    }
    assert!(b.app.doc.film_vide());
    b.chauffer(4);
    Some(b)
}

fn support_vide(b: &Banc) -> bool {
    let p = &b.app.doc.panels[0];
    p.layers.iter().all(|l| l.frames.iter().all(Option::is_none))
}

/// Tous les plans écartés : le panneau Calques et les Réglages le disent comme la timeline, sans montrer le calque du plan support.
#[test]
fn when_everything_is_cancelled_the_layers_and_settings_panes_say_so() {
    let Some(mut b) = tout_ecarte(false) else { return println!("pas de carte graphique") };
    let nom_du_support = {
        b.app.doc.panels[0].layers[0].name = "CALQUE-DU-SUPPORT".into();
        b.app.doc.panels[0].layers[0].name.clone()
    };
    b.garder_textes = true;
    for espace in [crate::workspace::Espace::Storyboard, crate::workspace::Espace::Vignettes] {
        b.app.set_espace(espace);
        b.chauffer(4);
        let dits = b.textes.iter().filter(|t| t.contains(crate::planannuleui::TEXTE_FILM_VIDE)).count();
        println!("TEXTES {espace:?} : {dits} fois le texte du film vide");
        // (Au Storyboard : le dessin, le montage, les Calques et les Réglages ; dans les Vignettes, ce que son agencement montre, le montage au moins.)
        let attendus = if espace == crate::workspace::Espace::Storyboard { 4 } else { 1 };
        assert!(dits >= attendus, "{espace:?} : les panneaux le disent : {dits} fois");
        assert!(!b.textes.iter().any(|t| t.contains(&nom_du_support)), "{espace:?} : le calque du support est montré");
        // Le réglage de l'outil non plus : pas son titre « calque vectoriel ».
        assert!(!b.textes.iter().any(|t| t == "calque vectoriel" || t == "calque bitmap"), "{espace:?} : les Réglages parlent du calque du support");
    }
}

/// Aucun geste ne dessine dans le plan support : ni sur la planche, ni au Storyboard, ni par une commande.
#[test]
fn when_everything_is_cancelled_no_gesture_draws_in_the_placeholder() {
    for bitmap in [false, true] {
        let Some(mut b) = tout_ecarte(bitmap) else { return println!("pas de carte graphique") };
        let l = b.app.board_layout();
        let xf = b.app.vignettes.xf;
        // Sur une case écartée, et sur le papier autour.
        let c = xf.rect(l.annulees[0].thumb);
        tracer(&mut b, c.center(), c.center() + eframe::egui::vec2(60.0, 20.0));
        tracer(&mut b, c.left_top() - eframe::egui::vec2(8.0, 8.0), c.left_top() - eframe::egui::vec2(8.0, 40.0));
        b.chauffer(2);
        assert!(support_vide(&b), "{} : sur la planche", sorte(bitmap));
        // Les commandes de dessin.
        for a in ["dessin.nouveau", "dessin.effacer", "calque.nouveau", "image.dupliquer"] {
            b.app.run_action(a);
            b.chauffer(1);
        }
        assert!(support_vide(&b), "{} : par une commande", sorte(bitmap));
        assert!(b.app.doc.film_vide());
        // Une commande qui dessine dit pourquoi elle ne fait rien.
        b.app.status.clear();
        assert!(!b.app.layer_editable());
        assert_eq!(b.app.status, crate::planannuleui::REFUS_SUPPORT);
        // Au Storyboard, et en animation.
        for mode in [crate::app::Mode::Storyboard, crate::app::Mode::Animation] {
            b.app.set_espace(crate::workspace::Espace::Storyboard);
            b.app.set_mode(mode);
            b.chauffer(4);
            let r = b.app.canvas_rect;
            tracer(&mut b, r.center(), r.center() + eframe::egui::vec2(120.0, 40.0));
            b.chauffer(2);
            assert!(support_vide(&b), "{} : au Storyboard, {mode:?}", sorte(bitmap));
            assert!(b.app.stroke.is_none());
        }
    }
}
