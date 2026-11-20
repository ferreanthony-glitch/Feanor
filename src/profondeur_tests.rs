//! Essais de la partie 183 : **la profondeur partagée entre le décor 3D et les cartes** (`profondeur.rs`). Une carte derrière
//! une boîte du décor est cachée là où la boîte la couvre et se voit ailleurs ; devant, elle la couvre (par la carte graphique
//! et par le processeur) ; une carte inclinée qui traverse la boîte est cachée derrière, visible devant ; une carte aimantée sur
//! une dalle n'a aucun pixel caché à ses pieds ; un calque accroché à la caméra et un calque « Toujours devant le décor 3D » ne
//! sont jamais cachés ; sans décor, rien ne change ; la case s'enregistre ; la vue Espace 3D cache aussi les cartes.

use std::sync::Arc;

use eframe::egui::epaint::{ClippedPrimitive, Primitive};
use eframe::egui::{self, pos2, vec2, Color32, Rect};
use eframe::egui_wgpu::{wgpu, RenderState, ScreenDescriptor};

use crate::app::App;
use crate::decor3d::{Decor3d, Objet3d};
use crate::decor3d_tests::boite;
use crate::decor3drendu::Aspect;
use crate::document::{Document, Layer, LayerKind, LayerTransform};
use crate::espace3d::Monde;
use crate::render::{FondPart, Plan};

pub(crate) const BLEU: Color32 = Color32::from_rgb(0, 0, 255);

/// Un projet de 160 × 90 (feuille 320 × 180), sans calque.
pub(crate) fn projet() -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.panels[0].length = 24;
    doc.panels[0].layers.clear();
    doc
}

/// Une boîte grise du décor, posée par le bas de son centre en `pos` (mètres), sans aimant.
pub(crate) fn objet_boite(id: u64, a: [f32; 3], b: [f32; 3], pos: [f32; 3]) -> Objet3d {
    let mut o = crate::decor3d_tests::objet(id, boite(a, b), pos[0], pos[2]);
    o.place.position = pos;
    o.aimante = false;
    o
}

/// Le décor de la séquence du plan 0 : ces objets, sans ciel ni brume ni ombres (le transparent autour du décor se voit).
pub(crate) fn poser_decor(doc: &mut Document, objets: Vec<Objet3d>) {
    let seq = doc.sequence_of(0);
    let mut d = Decor3d::new(Aspect { ciel: false, brume: false, ombres: false, ..Aspect::default() });
    d.objets = objets;
    doc.decors3d.par_sequence.insert(seq, d);
}

/// La boîte de la plupart des essais : 1 m de large, 2 m de haut, de z = −1,2 à −0,8 (son milieu à 1 m derrière la feuille).
fn la_boite() -> Objet3d {
    objet_boite(900, [-0.5, 0.0, -0.2], [0.5, 2.0, 0.2], [0.0, -1.0, -1.0])
}

/// Un calque bitmap avec un rectangle bleu (`x0..x1` × `y0..y1`, pixels de la feuille), à la profondeur où sa carte est à `z`
/// mètres. Son rang.
pub(crate) fn calque_bleu(doc: &mut Document, nom: &str, [x0, x1, y0, y1]: [usize; 4], z: f32) -> usize {
    let c = doc.new_cel(LayerKind::Bitmap);
    let r = &mut doc.cels.get_mut(&c).unwrap().raster;
    for y in y0..y1 {
        for x in x0..x1 {
            let idx = r.tile_index_of(x, y);
            let (ox, oy) = r.tile_origin(idx);
            r.tile_mut(idx)[(y as i64 - oy) as usize * crate::raster::TILE + (x as i64 - ox) as usize] = BLEU;
        }
    }
    let mut l = Layer::new(nom, LayerKind::Bitmap);
    l.frames = vec![Some(c); 24];
    l.depth = -z / Monde::of(doc).d0;
    doc.panels[0].layers.push(l);
    doc.panels[0].layers.len() - 1
}

/// Le rectangle bleu du centre de la feuille (± 30 × ± 20 pixels).
const CENTRE: [usize; 4] = [130, 190, 70, 110];

/// L'image du plan 0 à l'image 0, exportée (le chemin d'export ; le décor par le processeur, sans carte graphique prêtée) : le
/// cadre, 160 × 90, son centre au centre de la feuille.
fn exportee(doc: &Document) -> Vec<Color32> {
    use crate::exportrender::{snapshot_of, Papier, Rendu};
    let rendu = Rendu::new(snapshot_of(doc, &[0]), Papier::Aucun, false, Vec::new(), Arc::new(std::sync::atomic::AtomicBool::new(false)));
    (*rendu.image(0, 0).expect("image")).clone()
}

/// Le pixel du cadre décalé de (dx, dy) depuis son centre.
fn au(img: &[Color32], dx: i32, dy: i32) -> Color32 {
    let (w, h) = (160i32, 90i32);
    img[((h / 2 + dy) * w + w / 2 + dx) as usize]
}

fn bleu(c: Color32) -> bool {
    c.b() > 200 && c.r() < 60 && c.g() < 60
}

/// Le gris éclairé de la boîte (ni bleu ni transparent).
fn gris(c: Color32) -> bool {
    c.a() == 255 && !bleu(c) && (c.r() as i32 - c.b() as i32).abs() < 60
}

// ---------------------------------------------------------------------------------------------- le calcul du plan

#[test]
fn le_plan_d_une_carte_donne_l_inverse_de_sa_distance_a_chaque_pixel() {
    let doc = projet();
    let world = Monde::of(&doc);
    let eye = world.eye_reference();
    // Une carte à plat à 2 m derrière la feuille : 1 / (d0 + 2) partout.
    let xf = crate::espace3d::Xf { pos: [0.3, -0.2, -2.0], rot: [0.0; 3], size: [3.0, 2.0, 1.0] };
    let p = crate::profondeur::plan_de(&world, &xf.matrix(), eye).unwrap();
    for (x, y) in [(0.5, 0.5), (160.5, 90.5), (319.5, 10.5)] {
        assert!((crate::profondeur::inverse_distance(&p, x, y) - 1.0 / (world.d0 + 2.0)).abs() < 1e-6);
    }
    // Inclinée : en chaque point de la carte, la distance est celle de ce point (le long de l'axe de l'œil).
    let xf = crate::espace3d::Xf { pos: [0.0, 0.0, -1.0], rot: [20.0, 55.0, 10.0], size: [3.0, 2.0, 1.0] };
    let p = crate::profondeur::plan_de(&world, &xf.matrix(), eye).unwrap();
    let mx = xf.matrix();
    for (u, v) in [(-0.4, 0.3), (0.0, 0.0), (0.45, -0.45)] {
        let q = crate::espace3dmath::point(&mx, [u, v, 0.0]);
        let s = world.to_sheet(eye, q).unwrap();
        let attendu = 1.0 / (eye[2] - q[2]);
        let lu = crate::profondeur::inverse_distance(&p, s.x, s.y);
        assert!((lu - attendu).abs() < 1e-5 * attendu.max(1.0), "{lu} contre {attendu}");
    }
    // Vue par la tranche (son plan passe par l'œil) : aucun plan.
    let xf = crate::espace3d::Xf { pos: [0.0, 0.0, -1.0], rot: [0.0, 90.0, 0.0], size: [3.0, 2.0, 1.0] };
    assert!(crate::profondeur::plan_de(&world, &xf.matrix(), eye).is_none());
    // La tolérance : un décor à peine plus près (moins de 0,4 % + 1 cm) ne cache rien ; nettement plus près, si.
    let p = [0.0, 0.0, 1.0 / 8.0];
    assert!(!crate::profondeur::cache(&p, 1.0 / 7.98, 1.0, 1.0));
    assert!(crate::profondeur::cache(&p, 1.0 / 7.9, 1.0, 1.0));
    assert!(!crate::profondeur::cache(&p, 0.0, 1.0, 1.0), "le ciel, le vide : jamais");
}

#[test]
fn les_tolerances_du_shader_sont_celles_du_programme() {
    // (Les composeurs de la carte graphique portent les mêmes nombres que `profondeur.rs`, écrits dans leur code.)
    let src = format!("{}\n{}", include_str!("gpu.rs"), include_str!("decor3dgpu.rs"));
    let rel = format!("const TOLERANCE_RELATIVE: f32 = {};", crate::profondeur::TOLERANCE_RELATIVE);
    let m = format!("const TOLERANCE_METRES: f32 = {};", crate::profondeur::TOLERANCE_METRES);
    assert_eq!(src.matches(&rel).count(), 2, "{rel}");
    assert_eq!(src.matches(&m).count(), 2, "{m}");
}

#[test]
fn seules_les_cartes_du_monde_partagent_la_profondeur_et_seulement_avec_un_decor() {
    let mut doc = projet();
    let a = calque_bleu(&mut doc, "Perso", CENTRE, -2.0);
    let b = calque_bleu(&mut doc, "Accroché", CENTRE, -2.0);
    doc.panels[0].layers[b].accroche = true;
    let c = calque_bleu(&mut doc, "Devant", CENTRE, -2.0);
    doc.panels[0].layers[c].devant_decor = true;
    let d = calque_bleu(&mut doc, "Notes", CENTRE, -2.0);
    doc.panels[0].layers[d].annotation = true;
    let plan_sans = crate::render::panel_plan_at_avec(&doc, 0, 0, None, true);
    assert!(plan_sans.items.iter().all(|i| i.profondeur.is_none()), "sans décor : aucun plan, la composition d'avant");
    poser_decor(&mut doc, vec![la_boite()]);
    let plan = crate::render::panel_plan_at_avec(&doc, 0, 0, None, true);
    assert!(matches!(plan.items[0].fond, Some(FondPart::Decor(_))), "le décor reste l'élément le plus bas");
    let du = |li: usize| {
        let cel = doc.panels[0].layers[li].frames[0].unwrap();
        plan.items.iter().find(|i| i.cel == cel).unwrap().profondeur
    };
    assert!(du(a).is_some());
    assert!(du(b).is_none(), "accroché à la caméra : jamais caché");
    assert!(du(c).is_none(), "Toujours devant le décor 3D");
    assert!(du(d).is_none(), "un calque d'annotation");
    // À l'écran (le plan de travail), la même chose : le décor dessous (il y manquait, partie 180), les mêmes plans.
    let dir = std::env::temp_dir().join(format!("feanor-profondeur-ecran-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    assert!(app.build_plan_avec(0, false, false, false) == crate::render::panel_plan_at(&app.doc, 0, 0, None));
    assert!(app.build_plan(0, true, true).items.iter().any(|i| matches!(i.fond, Some(FondPart::Decor(_)))), "le dessin montre le décor");
}

// ---------------------------------------------------------------------------------------------- par le processeur

#[test]
fn par_le_processeur_une_carte_derriere_la_boite_est_cachee_la_ou_elle_la_couvre() {
    let mut doc = projet();
    poser_decor(&mut doc, vec![la_boite()]);
    calque_bleu(&mut doc, "Perso", CENTRE, -2.0);
    let img = exportee(&doc);
    assert!(gris(au(&img, 0, 0)), "la boîte devant la carte : {:?}", au(&img, 0, 0));
    assert!(gris(au(&img, 8, 10)) && gris(au(&img, -8, -12)));
    assert!(bleu(au(&img, 20, 0)) && bleu(au(&img, -25, 15)), "hors de la boîte, la carte se voit : {:?}", au(&img, 20, 0));
    // Devant la boîte, elle la couvre.
    let mut doc2 = projet();
    poser_decor(&mut doc2, vec![la_boite()]);
    calque_bleu(&mut doc2, "Perso", CENTRE, -0.3);
    let img = exportee(&doc2);
    assert!(bleu(au(&img, 0, 0)) && bleu(au(&img, 8, 10)) && bleu(au(&img, 20, 0)));
    // La boîte se voit encore au-dessus de la carte (elle est plus haute).
    assert!(gris(au(&img, 0, -22)), "{:?}", au(&img, 0, -22));
}

#[test]
fn une_carte_inclinee_qui_traverse_la_boite_est_cachee_derriere_et_visible_devant() {
    let mut doc = projet();
    poser_decor(&mut doc, vec![la_boite()]);
    let li = calque_bleu(&mut doc, "Perso", CENTRE, -1.0);
    // Le bord droit s'éloigne (inclinaison Y positive) : à gauche du milieu, la carte sort devant la boîte ; à droite, elle passe
    // derrière.
    doc.panels[0].layers[li].transform = LayerTransform { tilt_y: 60.0, ..Default::default() };
    let plan = crate::render::panel_plan_at(&doc, 0, 0, None);
    let item = plan.items.iter().find(|i| i.pre.as_ref().is_some_and(|p| p.card)).expect("posée par un maillage");
    assert!(item.profondeur.is_some_and(|p| p[0].abs() > 0.0), "le plan d'une carte inclinée varie sur l'image");
    let img = exportee(&doc);
    assert!(bleu(au(&img, -8, 0)), "devant la boîte : {:?}", au(&img, -8, 0));
    assert!(gris(au(&img, 8, 0)), "derrière la boîte : {:?}", au(&img, 8, 0));
    assert!(gris(au(&img, 0, 0)), "dans la boîte, sa face avant la cache");
}

#[test]
fn une_carte_aimantee_sur_une_dalle_n_a_aucun_pixel_cache_a_ses_pieds() {
    let mut doc = projet();
    let world = Monde::du_plan(&doc, 0);
    let sol = world.sol_y();
    // Une dalle de 4 × 4 m, de 10 cm d'épaisseur, sur le sol, qui passe sous la carte (devant et derrière elle).
    poser_decor(&mut doc, vec![objet_boite(901, [-2.0, 0.0, -2.0], [2.0, 0.1, 2.0], [0.0, sol, -1.0])]);
    // Le personnage : le rectangle bleu (40 lignes au-dessus de son origine) et ses « pieds », 6 lignes de plus sous son origine,
    // qui rentrent dans la dalle.
    let li = calque_bleu(&mut doc, "Perso", [130, 190, 70, 116], -1.0);
    {
        let decor = doc.decor_du_plan(0).cloned();
        let l = &mut doc.panels[0].layers[li];
        l.origine = Some([160.0, 110.0]);
        l.aimante_au_sol = true;
        assert!(crate::origine::aimanter_au_sol(l, &world, decor.as_ref()));
    }
    // Où est l'origine dans le cadre (la ligne du contact avec la dalle) ?
    let contact = {
        let l = &doc.panels[0].layers[li];
        let xf = world.carte(l, 0);
        world.to_sheet(world.eye_reference(), world.origine_monde(&xf, world.ecart(l))).unwrap().y - 45.0
    };
    let avec = exportee(&doc);
    doc.panels[0].layers[li].devant_decor = true;
    let devant = exportee(&doc);
    // Ligne par ligne, les pixels bleus.
    let lignes = |img: &[Color32]| -> Vec<usize> { (0..90).map(|y| (0..160).filter(|&x| bleu(img[y * 160 + x])).count()).collect() };
    let (l_avec, l_devant) = (lignes(&avec), lignes(&devant));
    let (mut dessus, mut dessous) = (0, 0);
    for y in 0..90 {
        let c = y as f32 + 0.5;
        // Au-dessus de l'origine, jusqu'au ras de la dalle : aucun pixel caché.
        if c < contact - 0.5 {
            assert_eq!(l_avec[y], l_devant[y], "ligne {y} (contact à {contact}) : rien ne doit être caché au-dessus de l'origine");
            dessus += l_devant[y];
        }
        // Plus bas, rentrées dans la dalle : cachées par son dessus, plus près.
        if c > contact + 1.5 && l_devant[y] > 0 {
            assert_eq!(l_avec[y], 0, "ligne {y}, rentrée dans la dalle : cachée");
            dessous += 1;
        }
    }
    assert!(dessus > 30 * 60 && dessous >= 3, "{dessus} pixels au-dessus, {dessous} lignes dessous");
}

#[test]
fn un_calque_accroche_et_toujours_devant_le_decor_ne_sont_jamais_caches() {
    for sorte in ["accroché", "devant"] {
        let mut doc = projet();
        poser_decor(&mut doc, vec![la_boite()]);
        let li = calque_bleu(&mut doc, "Perso", CENTRE, -2.0);
        match sorte {
            "accroché" => doc.panels[0].layers[li].accroche = true,
            _ => doc.panels[0].layers[li].devant_decor = true,
        }
        let img = exportee(&doc);
        assert!(bleu(au(&img, 0, 0)) && bleu(au(&img, 8, 10)), "{sorte} : {:?}", au(&img, 0, 0));
    }
}

#[test]
fn sans_decor_l_image_est_celle_d_avant() {
    // Des cartes à plat, inclinée, en mode de fusion, à des profondeurs variées : sans décor, aucune ne porte de plan…
    let mut doc = projet();
    calque_bleu(&mut doc, "A", CENTRE, -2.0);
    let b = calque_bleu(&mut doc, "B", [100, 150, 40, 120], -0.5);
    doc.panels[0].layers[b].transform = LayerTransform { tilt_y: 35.0, tilt_x: -10.0, ..Default::default() };
    let c = calque_bleu(&mut doc, "C", [150, 220, 60, 90], -3.0);
    doc.panels[0].layers[c].blend = crate::blend::BlendMode::Multiply;
    let plan = crate::render::panel_plan_at(&doc, 0, 0, None);
    assert!(plan.items.iter().all(|i| i.profondeur.is_none() && i.fond.is_none()));
    // … et la composition ne lit aucune profondeur : la même image qu'un plan dont les éléments porteraient un plan (sans décor
    // dessous, il est ignoré), par le processeur et par la carte graphique.
    let mut faux = plan.clone();
    for i in &mut faux.items {
        i.profondeur = Some([0.0, 0.0, 10.0]);
    }
    assert!(crate::render::composite_full(&doc, &plan) == crate::render::composite_full(&doc, &faux));
    let export = exportee(&doc);
    doc.panels[0].layers[c].devant_decor = true;
    assert!(export == exportee(&doc), "sans décor, la case ne change rien");
    if let Some(rs) = crate::gpu_tests::headless() {
        let mut gpu = crate::gpu::GpuCanvas::new(&rs);
        let _ = crate::gpu_tests::render_with(&mut gpu, &rs, &doc, &plan, true);
        let a = gpu.read_frame();
        let mut gpu2 = crate::gpu::GpuCanvas::new(&rs);
        let _ = crate::gpu_tests::render_with(&mut gpu2, &rs, &doc, &faux, true);
        assert!(!a.is_empty() && a == gpu2.read_frame(), "sur la carte graphique aussi");
    }
    // Avec un décor, une carte toute devant lui : la même image que si elle était « Toujours devant le décor 3D ».
    let mut doc = projet();
    poser_decor(&mut doc, vec![la_boite()]);
    let li = calque_bleu(&mut doc, "Perso", CENTRE, -0.3);
    let a = exportee(&doc);
    doc.panels[0].layers[li].devant_decor = true;
    assert!(a == exportee(&doc));
}

// ---------------------------------------------------------------------------------------------- par la carte graphique

/// L'image composée du plan 0 (la feuille, 320 × 180), par la carte graphique, le décor compris (dessiné par elle).
pub(crate) fn composee(rs: &RenderState, doc: &Document) -> Vec<Color32> {
    let plan = crate::render::panel_plan_at(doc, 0, 0, None);
    let mut gpu = crate::gpu::GpuCanvas::new(rs);
    let _ = crate::gpu_tests::render_with(&mut gpu, rs, doc, &plan, true);
    gpu.read_frame()
}

fn sur_la_feuille(img: &[Color32], dx: i32, dy: i32) -> Color32 {
    img[((90 + dy) * 320 + 160 + dx) as usize]
}

#[test]
fn par_la_carte_graphique_la_boite_cache_la_carte_qui_est_derriere() {
    let Some(rs) = crate::gpu_tests::headless() else { return };
    let mut doc = projet();
    poser_decor(&mut doc, vec![la_boite()]);
    let li = calque_bleu(&mut doc, "Perso", CENTRE, -2.0);
    let img = composee(&rs, &doc);
    let px = |dx, dy| sur_la_feuille(&img, dx, dy);
    assert!(gris(px(0, 0)) && gris(px(8, 10)) && gris(px(-8, -12)), "la boîte devant : {:?}", px(0, 0));
    assert!(bleu(px(20, 0)) && bleu(px(-25, 15)), "ailleurs, la carte : {:?}", px(20, 0));
    // Devant la boîte : elle la couvre.
    doc.panels[0].layers[li].depth = 0.3 / Monde::of(&doc).d0;
    let img = composee(&rs, &doc);
    assert!(bleu(sur_la_feuille(&img, 0, 0)) && bleu(sur_la_feuille(&img, 8, 10)));
    // Inclinée, elle traverse la boîte : cachée derrière, visible devant (posée par un maillage).
    doc.panels[0].layers[li].depth = 1.0 / Monde::of(&doc).d0;
    doc.panels[0].layers[li].transform = LayerTransform { tilt_y: 60.0, ..Default::default() };
    let img = composee(&rs, &doc);
    assert!(bleu(sur_la_feuille(&img, -8, 0)), "{:?}", sur_la_feuille(&img, -8, 0));
    assert!(gris(sur_la_feuille(&img, 8, 0)), "{:?}", sur_la_feuille(&img, 8, 0));
    // En mode de fusion (le composeur qui lit le fond) : cachée de même.
    doc.panels[0].layers[li].transform = LayerTransform::default();
    doc.panels[0].layers[li].depth = 2.0 / Monde::of(&doc).d0;
    doc.panels[0].layers[li].blend = crate::blend::BlendMode::Darken;
    let img = composee(&rs, &doc);
    assert!(gris(sur_la_feuille(&img, 0, 0)), "{:?}", sur_la_feuille(&img, 0, 0));
    assert!(!gris(sur_la_feuille(&img, 20, 0)));
    // Toujours devant le décor 3D : jamais cachée.
    doc.panels[0].layers[li].blend = crate::blend::BlendMode::Normal;
    doc.panels[0].layers[li].devant_decor = true;
    let img = composee(&rs, &doc);
    assert!(bleu(sur_la_feuille(&img, 0, 0)));
}

#[test]
fn la_carte_graphique_et_le_processeur_donnent_la_meme_profondeur_du_decor() {
    let Some(rs) = crate::gpu_tests::headless() else { return };
    let mut doc = projet();
    poser_decor(&mut doc, vec![la_boite(), objet_boite(902, [-0.3, 0.0, -0.3], [0.3, 0.6, 0.3], [1.0, -1.2, -3.0])]);
    let plan = crate::render::panel_plan_at(&doc, 0, 0, None);
    let Some(FondPart::Decor(key)) = plan.items[0].fond.clone() else { panic!() };
    let objets = crate::decor3d::objets_de(&doc, &key).unwrap();
    let [w, h] = key.feuille;
    let scenes: Vec<(&crate::decor3dgltf::Scene3d, crate::espace3dmath::M4)> = objets.iter().map(|(mdl, mx)| (&*mdl.scene, *mx)).collect();
    let (_, prof_cpu) = crate::decor3drendu::rendre_objets_prof(&scenes, &key.prise().unwrap(), [w, h], 1, 2, &key.aspect);
    let mut mt = crate::decor3dgpu::Moteur::new(&rs.device, &rs.queue);
    let d = crate::decor3d::dessins_gpu(&objets, key.aspect);
    let (_, _, prof_gpu) = crate::decor3dgpu::rendre_hors_ecran_prof(&rs.device, &rs.queue, &mut mt, &d, &key.prise().unwrap(), [w as u32, h as u32], false, crate::decor3dbudget::Qualite::Sans, 1, true).unwrap();
    let prof_gpu = prof_gpu.unwrap();
    // Loin des bords des objets (où deux rasterisations peuvent prendre un pixel ou non), la même distance à 0,1 % près.
    let (mut comparés, mut vides) = (0, 0);
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let voisins = [(0, 0), (1, 0), (0, 1), (-1, 0), (0, -1)].map(|(dx, dy): (i32, i32)| prof_cpu.un((y as i32 + dy) as usize * w + (x as i32 + dx) as usize));
            if voisins.iter().all(|v| *v == 0.0) {
                vides += 1;
                assert_eq!(prof_gpu.echantillons(y * w + x), [0.0; 4], "rien à ({x}, {y})");
                continue;
            }
            if voisins.iter().any(|v| *v == 0.0 || (*v - voisins[0]).abs() > voisins[0] * 0.01) {
                continue;
            }
            comparés += 1;
            // (Partie 193 : la carte garde quatre échantillons par pixel ; loin des bords, ils sont tous à la distance du processeur.)
            let c = prof_cpu.un(y * w + x);
            let g = prof_gpu.un(y * w + x);
            assert!(prof_gpu.echantillons(y * w + x).iter().all(|e| (c - e).abs() <= c * 2e-3), "({x}, {y}) : processeur {c}, échantillons {:?}", prof_gpu.echantillons(y * w + x));
            assert!((c - g).abs() <= c * 1e-3, "({x}, {y}) : processeur {} m, carte {} m", 1.0 / c, 1.0 / g);
        }
    }
    assert!(comparés > 1000 && vides > 1000, "{comparés} comparés, {vides} vides");
}

// ---------------------------------------------------------------------------------------------- la vue Espace 3D

/// Ce que montrent des commandes de dessin à la suite, sur un écran de `w` × `h` pixels.
fn lire(rs: &RenderState, w: u32, h: u32, jobs: Vec<ClippedPrimitive>) -> Vec<Color32> {
    let screen = ScreenDescriptor { size_in_pixels: [w, h], pixels_per_point: 1.0 };
    let target = rs.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("écran"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: rs.target_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let mut encoder = rs.device.create_command_encoder(&Default::default());
    let extra = rs.renderer.write().update_buffers(&rs.device, &rs.queue, &mut encoder, &jobs, &screen);
    {
        let mut pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("écran"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            })
            .forget_lifetime();
        rs.renderer.read().render(&mut pass, &jobs, &screen);
    }
    let row = (w * 4).div_ceil(256) * 256;
    let readback = rs.device.create_buffer(&wgpu::BufferDescriptor { label: Some("lecture"), size: (row * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo { buffer: &readback, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    rs.queue.submit(extra.into_iter().chain([encoder.finish()]));
    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("lecture"));
    rs.device.poll(wgpu::Maintain::Wait);
    let data = slice.get_mapped_range();
    let mut out = Vec::with_capacity((w * h) as usize);
    for y in 0..h as usize {
        for x in 0..w as usize {
            let i = y * row as usize + x * 4;
            out.push(Color32::from_rgba_premultiplied(data[i], data[i + 1], data[i + 2], data[i + 3]));
        }
    }
    out
}

/// Le voile d'une carte de la vue Espace 3D, seul : le décor est dessiné par la passe de la vue, mais pas posé (sa zone de
/// découpe est vide) ; reste ce que le voile de la carte placée en `xf` remet du décor devant elle.
pub(crate) fn voile_seul(rs: &RenderState, doc: &Document, xf: &crate::espace3d::Xf) -> Vec<Color32> {
    let plan = crate::render::panel_plan_at(doc, 0, 0, None);
    let Some(FondPart::Decor(key)) = plan.items[0].fond.clone() else { panic!() };
    let objets = crate::decor3d::objets_de(doc, &key).unwrap();
    let [w, h] = key.feuille;
    let zone = Rect::from_min_size(pos2(0.0, 0.0), vec2(w as f32, h as f32));
    let decor = crate::decor3dgpu::VueDecor {
        cle: 77,
        dessins: crate::decor3d::dessins_gpu(&objets, key.aspect),
        vue: crate::decor3dgpu::Vue::Prise(key.prise().unwrap()),
        taille_pts: [w as f32, h as f32],
        cible_egui: rs.target_format,
        qualite: crate::decor3dbudget::Qualite::Sans,
        cartes: None,
    };
    let coins = Monde::of(doc).card_corners(xf);
    let voile = crate::decor3dgpu::VoileCarte { cle: 77, rang: 0, coins, cible_egui: rs.target_format };
    let jobs = vec![
        ClippedPrimitive { clip_rect: Rect::from_min_size(pos2(0.0, 0.0), vec2(0.0, 0.0)), primitive: Primitive::Callback(eframe::egui_wgpu::Callback::new_paint_callback(zone, decor)) },
        ClippedPrimitive { clip_rect: zone, primitive: Primitive::Callback(eframe::egui_wgpu::Callback::new_paint_callback(zone, voile)) },
    ];
    lire(rs, w as u32, h as u32, jobs)
}

#[test]
fn dans_la_vue_espace_3d_le_decor_qui_est_devant_une_carte_la_recouvre() {
    let Some(rs) = crate::gpu_tests::headless() else { return };
    let mut doc = projet();
    poser_decor(&mut doc, vec![la_boite()]);
    let px = |img: &[Color32], dx: i32, dy: i32| img[((90 + dy) * 320 + 160 + dx) as usize];
    // Une carte de 2 × 1 m à 2 m derrière la feuille : la boîte la recouvre là où elle est devant elle.
    let derriere = crate::espace3d::Xf { pos: [0.0, 0.0, -2.0], rot: [0.0; 3], size: [2.0, 1.0, 1.0] };
    let img = voile_seul(&rs, &doc, &derriere);
    assert!(gris(px(&img, 0, 0)) && gris(px(&img, 5, 5)), "le décor recouvre la carte : {:?}", px(&img, 0, 0));
    assert_eq!(px(&img, 20, 0).a(), 0, "hors de la boîte : la carte reste visible");
    assert_eq!(px(&img, 0, 40).a(), 0, "hors de la carte : rien");
    // Devant la boîte : rien ne la recouvre.
    let devant = crate::espace3d::Xf { pos: [0.0, 0.0, -0.3], ..derriere };
    let img = voile_seul(&rs, &doc, &devant);
    assert!(img.iter().all(|c| c.a() == 0), "devant le décor, aucun voile");
}

// ---------------------------------------------------------------------------------------------- la case, le fichier

#[test]
fn la_case_est_une_etape_d_annulation_et_une_commande() {
    let dir = std::env::temp_dir().join(format!("feanor-profondeur-case-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    let mut doc = projet();
    poser_decor(&mut doc, vec![la_boite()]);
    calque_bleu(&mut doc, "Perso", CENTRE, -2.0);
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    let avant = app.history.undo_len();
    app.basculer_devant_decor(0);
    assert!(app.layers()[0].devant_decor);
    assert_eq!(app.history.undo_len(), avant + 1, "une étape");
    app.undo();
    assert!(!app.layers()[0].devant_decor, "Ctrl+Z la défait");
    app.run_action(crate::profondeur::ACTION);
    assert!(app.layers()[0].devant_decor, "la commande de la palette");
    assert!(crate::keymap::ACTIONS.iter().any(|a| a.id == crate::profondeur::ACTION));
    // Le pot de peinture ne voit pas le décor (ce n'est pas un calque) : ses contours sont ceux d'avant.
    app.fill.sample_all = true;
    let avec = app.fill_source();
    let seq = app.doc.sequence_of(0);
    app.doc.decors3d.par_sequence.remove(&seq);
    assert!(avec == app.fill_source(), "les contours du pot de peinture ne changent pas avec le décor");
    let toml = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("keymap.toml")).unwrap();
    assert!(toml.contains(&format!("action = \"{}\"", crate::profondeur::ACTION)), "son bloc dans le fichier des raccourcis");
}

#[test]
fn la_case_s_enregistre_et_un_fichier_d_avant_s_ouvre_decochee() {
    let dir = std::env::temp_dir().join(format!("feanor-profondeur-fichier-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("P.shot");
    let mut doc = projet();
    calque_bleu(&mut doc, "Devant", CENTRE, -2.0);
    calque_bleu(&mut doc, "Ordinaire", CENTRE, -1.0);
    doc.panels[0].layers[0].devant_decor = true;
    crate::project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(&manifest).unwrap();
    assert_eq!(texte.matches("devant_decor_3d").count(), 1, "écrit pour le seul calque coché");
    assert!(texte.contains(&format!("\"version\": {}", crate::project::VERSION)) && crate::project::VERSION == 61, "le format ne change pas");
    let relu = crate::project::load(&manifest).unwrap().doc;
    assert!(relu.panels[0].layers[0].devant_decor);
    assert!(!relu.panels[0].layers[1].devant_decor);
    // Un fichier d'avant (sans le champ, format 60) : il s'ouvre, la case décochée.
    let ancien = texte.replace(&format!("\"version\": {}", crate::project::VERSION), "\"version\": 60");
    let ancien: String = ancien.lines().filter(|l| !l.contains("devant_decor_3d")).collect::<Vec<_>>().join("\n");
    assert!(!ancien.contains("devant_decor_3d"));
    std::fs::write(&manifest, ancien).unwrap();
    let relu = crate::project::load(&manifest).expect("un projet d'avant s'ouvre").doc;
    assert!(relu.panels[0].layers.iter().all(|l| !l.devant_decor));
    std::fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------------------------------- les mesures

/// Compose `plan` `n` fois sur la carte (affichage compris, sans relecture), l'image composée oubliée à chaque fois : la
/// meilleure de trois séries, en millisecondes par image.
fn temps_de_composition(rs: &RenderState, doc: &Document, plan: &Plan, n: usize) -> f32 {
    let mut gpu = crate::gpu::GpuCanvas::new(rs);
    let (w, h) = (doc.width as u32, doc.height as u32);
    let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(w as f32, h as f32));
    let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()];
    let screen = ScreenDescriptor { size_in_pixels: [w, h], pixels_per_point: 1.0 };
    let target = rs.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("écran"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: rs.target_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let une = |gpu: &mut crate::gpu::GpuCanvas| {
        let resolved = gpu.sync(doc, plan, false, &[], &[], 0);
        gpu.oublier_composition();
        let jobs = [ClippedPrimitive { clip_rect: rect, primitive: Primitive::Callback(gpu.paint(plan, &resolved, corners, None, rect, [doc.width, doc.height])) }];
        let mut encoder = rs.device.create_command_encoder(&Default::default());
        let extra = rs.renderer.write().update_buffers(&rs.device, &rs.queue, &mut encoder, &jobs, &screen);
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("écran"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store } })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();
            rs.renderer.read().render(&mut pass, &jobs, &screen);
        }
        rs.queue.submit(extra.into_iter().chain([encoder.finish()]));
        let _ = rs.device.poll(wgpu::Maintain::Wait);
    };
    // (Les premières : les textures partent à la carte, le décor se dessine.)
    for _ in 0..3 {
        une(&mut gpu);
    }
    let mut meilleur = f32::MAX;
    for _ in 0..3 {
        let debut = std::time::Instant::now();
        for _ in 0..n {
            une(&mut gpu);
        }
        meilleur = meilleur.min(debut.elapsed().as_secs_f32() * 1000.0 / n as f32);
    }
    meilleur
}

/// Mesures (à lancer à la main : `cargo test --lib profondeur_perf -- --ignored --nocapture`) : la composition de huit cartes
/// sur une feuille de 3840 × 2160, sur un décor, avec la profondeur partagée et sans (les mêmes cartes, sans plan), et le même plan
/// sans décor ; des cartes pleines (le pire cas : de l'encre sur toute la feuille) et des cartes de personnage (un rectangle d'un
/// huitième de la feuille, le reste transparent).
#[test]
#[ignore]
fn profondeur_perf() {
    let Some(rs) = crate::gpu_tests::headless() else { return };
    // (Partie 193 : avec les bords doux, le cas le plus cher ; sans eux, c'est le chemin d'avant.)
    crate::decor3d::essai_bords(Some(true));
    for (sorte, part) in [("pleines", 1usize), ("d'un huitième de la feuille", 8)] {
        let mut doc = Document::with_frame(1920, 1080, 24);
        doc.panels[0].length = 24;
        doc.panels[0].layers.clear();
        let (w, h) = (doc.width, doc.height);
        let (cw, ch) = if part == 1 { (w, h) } else { (w / 4, h / 2) };
        for k in 0..8 {
            let x0 = (k * w / 8).min(w - cw);
            calque_bleu(&mut doc, &format!("Carte {k}"), [x0, x0 + cw, (h - ch) / 2, (h + ch) / 2], -0.5 - k as f32);
        }
        let sans_decor = crate::render::panel_plan_at(&doc, 0, 0, None);
        let t_sans_decor = temps_de_composition(&rs, &doc, &sans_decor, 20);
        let mut o = crate::decor3d_tests::objet(903, crate::decor3dbudget::scene_d_essai(300, 512), 0.0, -3.0);
        o.aimante = false;
        poser_decor(&mut doc, vec![o]);
        let avec = crate::render::panel_plan_at(&doc, 0, 0, None);
        assert_eq!(avec.items.iter().filter(|i| i.profondeur.is_some()).count(), 8);
        let t_avec = temps_de_composition(&rs, &doc, &avec, 20);
        let mut devant = avec.clone();
        for i in &mut devant.items {
            i.profondeur = None;
        }
        let t_devant = temps_de_composition(&rs, &doc, &devant, 20);
        println!("composition de 8 cartes {sorte}, feuille {w} × {h} (une image, affichage compris) : sans décor {t_sans_decor:.2} ms ; décor, cartes sans plan {t_devant:.2} ms ; décor, profondeur partagée {t_avec:.2} ms (+{:.2} ms)", t_avec - t_devant);
    }
}
