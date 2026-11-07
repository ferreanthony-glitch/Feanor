//! Le trait qui traverse une case qui ne pouvait pas le recevoir (partie 120), **vu en pixels** comme la partie 111 : un autre zoom, un roulis, une
//! bascule, un panoramique, des cadres A et B, un calque verrouillé, masqué ou d'une autre sorte, un plan écarté. Au vrai geste, en vectoriel et en
//! bitmap : pendant le geste, au repos, après réouverture, à l'export ; où sont les données ; une seule étape ; rien de ce qui était dessiné ne bouge.

use eframe::egui::{pos2, Color32, Event, Modifiers, Pos2, Rect};

use crate::document::LayerKind;
use crate::perf_tests::Banc;
use crate::traits_continus_tests::{des_plans, exporter, planche, relever, relever_a, repos, sauver, tracer};
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
    a.traces.iter().filter(|t| a.calque(t.calque).is_some_and(|c| c.planche)).count()
}

/// Les rectangles à l'écran de l'image du plan `id`.
fn case_ecran(b: &Banc, id: u64) -> Rect {
    let l = b.app.board_layout();
    let c = l.cards.iter().chain(l.annulees.iter()).find(|c| c.plan == id).expect("la case");
    b.app.vignettes.xf.rect(c.thumb)
}

/// Le seuil d'encre (rouge moins vert) : un trait d'un pixel à cheval sur deux rangées de la tuile d'un plan à cadres tournés est plus pâle, comme tout ce que montre
/// cette tuile, le trait d'avant compris (partie 120, ce qui reste).
fn seuil(v: Variante) -> i32 {
    if v == Variante::CadresRoulis {
        12
    } else {
        40
    }
}

/// Le trou le plus long sur le trait de `bd` à `ba` (planche) dans l'image exportée.
fn trou_a_l_export(b: &Banc, img: &image::RgbImage, opt: &crate::planche::Options, bd: Pos2, ba: Pos2, seuil: i32) -> (usize, f32) {
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
                v[0] as i32 - v[1] as i32 >= seuil
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
    let y = rd.top() + rd.height() * 0.35;
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
    let geste = relever_a(&geste_img, de, a, seuil(v)).trou;
    let statut_geste = b.app.status.clone();
    b.image(vec![appui(*chemin.last().unwrap(), false, Modifiers::NONE)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    repos(&mut b);
    let img = b.capturer()?;
    sauver(&img, &format!("{nom}-2-repos"));
    let repos_r = relever_a(&img, de, a, seuil(v));
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
        relever_a(&img, de, a, seuil(v)).trou
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
        Some(trou_a_l_export(&b, &img, &opt, bd, ba, seuil(v)).0)
    };
    // Annuler rend les données d'avant.
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    let annule_ok = encre_du_plan(&b.app.doc, ids[PLAN]) == avant_plan && traits_hors_cadres(&b.app.doc) == graine;
    Some(Constat { geste, repos: repos_r.trou, rouvert, export, dans_le_plan, hors_cadres, etapes, rejeux, statut, deja_intact, annule_ok })
}

/// Le tableau de la partie 120 : les mesures seules, sans jugement. `cargo test … mesure_les_variantes -- --ignored --nocapture --test-threads=1`
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

fn verifier_recu(v: Variante, bitmap: bool, depuis: bool) {
    let Some(c) = constater(v, bitmap, depuis) else { return println!("pas de carte graphique") };
    let nom = format!("{v:?} {} {}", sorte(bitmap), if depuis { "depuis la case" } else { "à travers" });
    println!("TRAIT {nom} : {c:?}");
    // (Les cadres tournés croisent le trait presque à plat : leur filet et son halo le recouvrent sur une vingtaine de pixels, ils y sont aussi sans lui.)
    let permis = if v == Variante::CadresRoulis { 26 } else { TROU_PERMIS };
    assert!(c.geste <= permis, "{nom} : pendant le geste : {c:?}");
    assert!(c.repos <= permis, "{nom} : au repos : {c:?}");
    assert!(c.rouvert <= permis, "{nom} : après réouverture : {c:?}");
    assert!(c.export.is_some_and(|t| t <= permis + 4), "{nom} : à l'export : {c:?}");
    assert!(c.dans_le_plan, "{nom} : la part est dans le plan : {c:?}");
    assert_eq!(c.hors_cadres, 0, "{nom} : rien ne reste sur la planche : {c:?}");
    assert_eq!(c.etapes, 1, "{nom} : une seule étape : {c:?}");
    assert_eq!(c.rejeux, 0, "{nom} : aucun rejeu sur le fil de l'interface : {c:?}");
    assert!(c.deja_intact, "{nom} : ce qui était dessiné ne bouge pas : {c:?}");
    assert!(c.annule_ok, "{nom} : annuler rend tout : {c:?}");
    assert!(c.statut.is_empty(), "{nom} : rien à dire : {c:?}");
}

#[test]
fn a_stroke_across_a_case_with_another_zoom_reaches_its_plan_and_stays_whole() {
    for v in [Variante::Zoom, Variante::ZoomFaible] {
        for bitmap in [false, true] {
            for depuis in [false, true] {
                verifier_recu(v, bitmap, depuis);
            }
        }
    }
}

#[test]
fn a_stroke_across_a_case_with_a_rolled_camera_reaches_its_plan_and_stays_whole() {
    for bitmap in [false, true] {
        for depuis in [false, true] {
            verifier_recu(Variante::Roulis, bitmap, depuis);
        }
    }
}

#[test]
fn a_stroke_across_a_case_with_a_tilted_or_panned_camera_reaches_its_plan_and_stays_whole() {
    for v in [Variante::Bascule, Variante::Panoramique] {
        for bitmap in [false, true] {
            for depuis in [false, true] {
                verifier_recu(v, bitmap, depuis);
            }
        }
    }
}

#[test]
fn a_stroke_across_a_plan_with_frames_a_and_b_reaches_its_plan_and_stays_whole() {
    for v in [Variante::Cadres, Variante::CadresZoom, Variante::CadresRoulis] {
        for bitmap in [false, true] {
            for depuis in [false, true] {
                verifier_recu(v, bitmap, depuis);
            }
        }
    }
}

/// Le plan a vraiment reçu sa part, au bon endroit : l'image du film, à travers la caméra de la case, montre le trait là où la case le montre.
#[test]
fn the_film_frame_of_the_plan_shows_the_part_where_the_case_shows_it() {
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
            tracer(&mut b, pos2(rd.center().x, y), pos2(ra.center().x, y));
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
            assert!(encre * 100 >= vus * 97, "{v:?} {} : le film montre le trait d'un bord à l'autre : {encre} sur {vus}", sorte(bitmap));
        }
    }
}

// ------------------------------------------------------------------------------ refusé, jamais caché

fn verifier_refus(v: Variante, bitmap: bool, mots: &str) {
    let Some(c) = constater(v, bitmap, false) else { return println!("pas de carte graphique") };
    let nom = format!("{v:?} {}", sorte(bitmap));
    println!("TRAIT {nom} : {c:?}");
    assert!(c.geste <= TROU_PERMIS, "{nom} : pendant le geste : {c:?}");
    assert!(c.repos <= TROU_PERMIS, "{nom} : au repos, rien n'a disparu : {c:?}");
    assert!(c.rouvert <= TROU_PERMIS, "{nom} : après réouverture : {c:?}");
    if let Some(t) = c.export {
        assert!(t <= 12, "{nom} : à l'export : {c:?}");
    }
    assert!(!c.dans_le_plan, "{nom} : rien n'est écrit dans ce plan : {c:?}");
    assert!(c.hors_cadres >= 1, "{nom} : la part reste sur la planche : {c:?}");
    assert_eq!(c.etapes, 1, "{nom} : une seule étape : {c:?}");
    assert_eq!(c.rejeux, 0, "{nom} : aucun rejeu sur le fil de l'interface : {c:?}");
    assert!(c.deja_intact, "{nom} : ce qui était dessiné ne bouge pas : {c:?}");
    assert!(c.annule_ok, "{nom} : annuler rend tout : {c:?}");
    assert!(c.statut.contains(mots), "{nom} : le message dit pourquoi ({mots}) : {:?}", c.statut);
    assert!(c.statut.contains(" / "), "{nom} : le message est là dès le geste, et encore au lever : {:?}", c.statut);
}

#[test]
fn a_stroke_across_a_locked_layer_writes_nothing_stays_visible_and_says_why() {
    for bitmap in [false, true] {
        verifier_refus(Variante::Verrou, bitmap, "verrouillé");
    }
}

#[test]
fn a_stroke_across_a_hidden_layer_writes_nothing_stays_visible_and_says_why() {
    for bitmap in [false, true] {
        verifier_refus(Variante::Masque, bitmap, "masqué");
    }
}

#[test]
fn a_stroke_across_a_plan_without_a_layer_of_its_kind_stays_visible_and_says_why() {
    verifier_refus(Variante::AutreSorte, false, "vectoriel");
    verifier_refus(Variante::AutreSorte, true, "bitmap");
}

#[test]
fn a_stroke_across_a_transformed_layer_stays_visible_and_says_why() {
    for bitmap in [false, true] {
        verifier_refus(Variante::CalqueTransforme, bitmap, "transformé");
    }
}

#[test]
fn a_stroke_across_a_cancelled_plan_stays_visible_and_says_why() {
    for bitmap in [false, true] {
        verifier_refus(Variante::Ecarte, bitmap, "écarté");
    }
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
        b.app.doc.panels[PLAN].layers[0].locked = false;
        b.chauffer(2);
        let y = rd.top() + rd.height() * 0.6;
        tracer(&mut b, pos2(rd.center().x, y), pos2(ra.center().x, y));
        repos(&mut b);
        assert_ne!(encre_du_plan(&b.app.doc, id), avant, "{} : déverrouillé, il reçoit", sorte(bitmap));
    }
}

/// La part posée par-dessus la case : la gomme l'efface d'un geste, sans clic droit et sans toucher au dessin du plan.
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
        tracer(&mut b, pos2(rd.center().x, y), pos2(ra.center().x, y));
        repos(&mut b);
        let dessus = |b: &Banc| b.app.doc.annotations.traces.iter().filter(|t| t.dessus).count();
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

/// Les petits débordements restent à la marge : une part dans la marge d'une case à l'autre caméra va dans son plan, et ne recouvre jamais l'image d'à côté.
#[test]
fn a_stroke_does_not_leave_a_refused_case_covered_by_a_margin_of_its_neighbour() {
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
    println!("TRAIT marge sur case refusée : {r:?}");
    assert!(r.trou <= TROU_PERMIS, "l'image refusée passe avant la marge de sa voisine : {r:?}");
    assert!(b.app.doc.annotations.traces.iter().any(|t| t.dessus), "la part est par-dessus");
}

/// À l'export, la part posée sur une case écartée part avec elle : le plan est absent, elle ne flotte pas sur la planche exportée.
#[test]
fn the_part_on_a_cancelled_plan_is_not_in_the_board_export() {
    let Some(mut b) = planche(des_plans(18, false), 6, 1.0) else { return println!("pas de carte graphique") };
    appliquer(&mut b, Variante::Ecarte, false);
    b.chauffer(4);
    let (rd, ra) = (case_ecran(&b, b.app.doc.panels[PLAN - 1].id), case_ecran(&b, b.app.doc.panels[PLAN].id));
    let y = rd.top() + rd.height() * 0.35;
    tracer(&mut b, pos2(rd.center().x, y), pos2(ra.center().x, y));
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

// ------------------------------------------------------------------------------ tous les plans écartés

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
        assert_eq!(b.app.status, crate::planannuleui::REFUS_DESSIN);
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
