//! Essais de la partie 185 : **les ombres des cartes et du décor 3D** (`ombres.rs`). Sous un soleil qui vient de derrière le décor
//! (45° de haut), l'ombre d'une carte debout sur une dalle tombe devant elle, sur la dalle ; c'est l'ombre de son encre, pas de
//! son rectangle ; une boîte entre le soleil et une carte l'assombrit ; une carte en ombre une autre ; une carte n'est pas dans sa
//! propre ombre ; les cases décochées, les ombres du décor éteintes, l'absence de décor ne changent rien ; un calque accroché à la
//! caméra, d'annotation ou d'effet n'a pas d'ombre ; les cases sont des étapes d'annulation, des commandes, s'enregistrent ; la
//! carte graphique, le dessin et l'export disent la même chose.

use std::sync::Arc;

use eframe::egui::epaint::{ClippedPrimitive, Primitive};
use eframe::egui::{self, pos2, vec2, Color32, Rect};
use eframe::egui_wgpu::{wgpu, RenderState, ScreenDescriptor};

use crate::app::App;
use crate::decor3d::{Decor3d, Objet3d};
use crate::decor3d_tests::boite;
use crate::decor3dbudget::Qualite;
use crate::decor3drendu::Aspect;
use crate::document::{Document, Layer, LayerKind};
use crate::espace3d::Monde;
use crate::espace3dmath as m;
use crate::render::{FondPart, Plan};

const BLEU: Color32 = Color32::from_rgb(0, 0, 255);

/// Un projet de 160 × 90 (feuille 320 × 180), sans calque.
fn projet() -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.panels[0].length = 24;
    doc.panels[0].layers.clear();
    doc
}

/// Une boîte grise du décor : `a`..`b` autour de son origine, posée en `pos` (mètres), sans aimant.
fn objet_boite(id: u64, a: [f32; 3], b: [f32; 3], pos: [f32; 3]) -> Objet3d {
    let mut o = crate::decor3d_tests::objet(id, boite(a, b), pos[0], pos[2]);
    o.place.position = pos;
    // (L'origine au point (0, 0, 0) du modèle : la boîte est `pos + a` .. `pos + b`.)
    o.origine = [0.0; 3];
    o.aimante = false;
    o
}

/// La dalle : le sol à y = −1, de z = −6,5 à 0,5, sur 8 m de large.
fn dalle() -> Objet3d {
    objet_boite(950, [-4.0, -0.1, -6.5], [4.0, 0.0, 0.5], [0.0, -1.0, 0.0])
}

/// Le soleil des essais : derrière le décor (loin de la caméra), à 45° : l'ombre d'une carte tombe devant elle, vers la caméra.
fn aspect() -> Aspect {
    Aspect { ciel: false, brume: false, ombres: true, soleil_azimut: 180.0, soleil_hauteur: 45.0, ..Aspect::default() }
}

fn poser_decor(doc: &mut Document, objets: Vec<Objet3d>, aspect: Aspect) {
    let seq = doc.sequence_of(0);
    let mut d = Decor3d::new(aspect);
    d.objets = objets;
    doc.decors3d.par_sequence.insert(seq, d);
}

/// Le pixel de la feuille où l'on voit le point `p` du monde (de l'œil de référence : la caméra du plan ne bouge pas).
fn pixel(doc: &Document, p: [f32; 3]) -> (i32, i32) {
    let w = Monde::of(doc);
    let s = w.to_sheet(w.eye_reference(), p).unwrap();
    (s.x.floor() as i32, s.y.floor() as i32)
}

/// Un calque bitmap dont la carte est à `z` mètres, avec de l'encre bleue sur chacun des rectangles du monde `rects` (x0, x1, y0,
/// y1 en mètres, sur le plan de la carte). Son rang.
fn calque(doc: &mut Document, nom: &str, z: f32, rects: &[[f32; 4]]) -> usize {
    let c = doc.new_cel(LayerKind::Bitmap);
    let monde = Monde::of(doc);
    let eye = monde.eye_reference();
    let px: Vec<[usize; 4]> = rects
        .iter()
        .map(|r| {
            let a = monde.to_sheet(eye, [r[0], r[3], z]).unwrap();
            let b = monde.to_sheet(eye, [r[1], r[2], z]).unwrap();
            [a.x.round().max(0.0) as usize, b.x.round() as usize, a.y.round().max(0.0) as usize, b.y.round() as usize]
        })
        .collect();
    let r = &mut doc.cels.get_mut(&c).unwrap().raster;
    for [x0, x1, y0, y1] in px {
        for y in y0..y1.min(r.height) {
            for x in x0..x1.min(r.width) {
                let idx = r.tile_index_of(x, y);
                let (ox, oy) = r.tile_origin(idx);
                r.tile_mut(idx)[(y as i64 - oy) as usize * crate::raster::TILE + (x as i64 - ox) as usize] = BLEU;
            }
        }
    }
    let mut l = Layer::new(nom, LayerKind::Bitmap);
    l.frames = vec![Some(c); 24];
    l.depth = -z / monde.d0;
    doc.panels[0].layers.push(l);
    doc.panels[0].layers.len() - 1
}

/// L'image composée de `plan` (la feuille, 320 × 180) par la carte graphique, le décor dessiné par elle, les ombres du décor à la
/// qualité `q`.
fn composee_plan(rs: &RenderState, doc: &Document, plan: &Plan, q: Qualite) -> Vec<Color32> {
    crate::decor3d::essai_poser(Some(q), None);
    let mut gpu = crate::gpu::GpuCanvas::new(rs);
    let _ = crate::gpu_tests::render_with(&mut gpu, rs, doc, plan, true);
    let img = gpu.read_frame();
    crate::decor3d::essai_poser(None, None);
    img
}

fn composee(rs: &RenderState, doc: &Document, q: Qualite) -> Vec<Color32> {
    composee_plan(rs, doc, &crate::render::panel_plan_at(doc, 0, 0, None), q)
}

/// L'image du plan 0 exportée (le cadre, 160 × 90), le décor rendu par la carte graphique prêtée à l'export, à la qualité `q`.
fn exportee(rs: &RenderState, doc: &Document, q: Qualite) -> Vec<Color32> {
    use crate::exportrender::{snapshot_of, Papier, Rendu};
    crate::decor3d::essai_poser(Some(q), Some(rs));
    // (Le décor est rendu sur ce fil, où la carte est prêtée ; l'export le trouve rendu.)
    let plan = crate::render::panel_plan_at(doc, 0, 0, None);
    if let Some(FondPart::Decor(key)) = &plan.items[0].fond {
        let _ = crate::decor3d::montre(doc, key);
    }
    let rendu = Rendu::new(snapshot_of(doc, &[0]), Papier::Aucun, false, Vec::new(), Arc::new(std::sync::atomic::AtomicBool::new(false)));
    let img = (*rendu.image(0, 0).expect("image")).clone();
    crate::decor3d::essai_poser(None, None);
    img
}

/// Le pixel de la feuille (`x`, `y`).
fn px(img: &[Color32], (x, y): (i32, i32)) -> Color32 {
    img[(y * 320 + x) as usize]
}

/// Le pixel du cadre exporté qui montre le pixel (`x`, `y`) de la feuille (le cadre est au milieu de la feuille).
fn px_cadre(img: &[Color32], (x, y): (i32, i32)) -> Color32 {
    img[((y - 45) * 160 + x - 80) as usize]
}

fn somme(c: Color32) -> i32 {
    c.r() as i32 + c.g() as i32 + c.b() as i32
}

/// Le plan sans les ombres de ses cartes (la composition d'avant).
fn sans_ombres(plan: &Plan) -> Plan {
    let mut p = plan.clone();
    for i in &mut p.items {
        i.ombre = None;
    }
    p
}

// ---------------------------------------------------------------------------------------------- le calcul

#[test]
fn seules_les_cartes_du_monde_ont_des_ombres_et_seulement_sur_un_decor_qui_en_a() {
    let mut doc = projet();
    let r = [[-0.6, 0.6, -1.2, 0.5]];
    let a = calque(&mut doc, "Perso", -1.5, &r);
    let b = calque(&mut doc, "Accroché", -1.5, &r);
    doc.panels[0].layers[b].accroche = true;
    let c = calque(&mut doc, "Devant", -1.5, &r);
    doc.panels[0].layers[c].devant_decor = true;
    let d = calque(&mut doc, "Notes", -1.5, &r);
    doc.panels[0].layers[d].annotation = true;
    let e = calque(&mut doc, "Ni l'un ni l'autre", -1.5, &r);
    doc.panels[0].layers[e].porte_ombre = false;
    doc.panels[0].layers[e].recoit_ombres = false;
    let f = calque(&mut doc, "Reçoit seulement", -1.5, &r);
    doc.panels[0].layers[f].porte_ombre = false;
    assert!(crate::render::panel_plan_at_avec(&doc, 0, 0, None, true).items.iter().all(|i| i.ombre.is_none()), "sans décor : aucune ombre");
    poser_decor(&mut doc, vec![dalle()], Aspect { ombres: false, ..aspect() });
    assert!(crate::render::panel_plan_at_avec(&doc, 0, 0, None, true).items.iter().all(|i| i.ombre.is_none()), "un décor sans ses ombres : aucune");
    poser_decor(&mut doc, vec![dalle()], aspect());
    let plan = crate::render::panel_plan_at_avec(&doc, 0, 0, None, true);
    let de = |li: usize| {
        let cel = doc.panels[0].layers[li].frames[0].unwrap();
        plan.items.iter().find(|i| i.cel == cel).unwrap().clone()
    };
    let o = de(a).ombre.expect("une carte ordinaire porte et reçoit");
    assert!(o.porte && o.recoit);
    assert_eq!(Some(o.plan), de(a).profondeur, "le même plan que la profondeur partagée");
    assert!(de(b).ombre.is_none(), "accrochée à la caméra : dans le cadre, pas au soleil");
    assert!(de(c).ombre.is_some_and(|o| o.porte && o.recoit), "« Toujours devant le décor 3D » ne touche pas au soleil");
    assert!(de(d).ombre.is_none(), "un calque d'annotation");
    assert!(de(e).ombre.is_none(), "les deux cases décochées");
    assert!(de(f).ombre.is_some_and(|o| !o.porte && o.recoit));
    // Un dossier, un calque d'effet : pas de carte, pas d'ombre.
    let mut dossier = Layer::new("Dossier", LayerKind::Bitmap);
    dossier.folder = true;
    assert!(!crate::ombres::concerne(&dossier));
    // Le dessin (le plan de travail) a les mêmes ombres que les sorties.
    let dir = std::env::temp_dir().join(format!("feanor-ombres-plan-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    assert!(app.build_plan_avec(0, false, false, false) == crate::render::panel_plan_at(&app.doc, 0, 0, None), "le dessin et les sorties : le même plan, ombres comprises");
}

#[test]
fn une_carte_n_est_jamais_dans_sa_propre_ombre_et_le_calcul_est_celui_de_la_carte_graphique() {
    let doc = projet();
    let monde = Monde::of(&doc);
    let eye = monde.eye_reference();
    let prise = crate::decor3drendu::Prise::de_la_feuille(&monde, eye).unwrap();
    let modele = Arc::new(crate::decor3d::Modele3d::new(Arc::new(boite([-4.0, -1.1, -6.0], [4.0, -1.0, 0.0])), Arc::new(Vec::new())));
    let d = crate::decor3d::dessins_gpu(&[(modele, m::IDENT)], aspect());
    let ec = crate::decor3dgpu::eclairage(&d, eye, &aspect(), Qualite::Hautes);
    let lum = crate::ombres::Lumiere::de(&ec, &aspect(), &prise).expect("ombres");
    assert!(crate::ombres::Lumiere::de(&crate::decor3dgpu::eclairage(&d, eye, &aspect(), Qualite::Sans), &aspect(), &prise).is_none(), "sans ombres : rien");
    // La pente d'une carte inclinée sous le soleil : sa profondeur dans la carte d'ombres suit son plan, exactement.
    let xf = crate::espace3d::Xf { pos: [0.2, -0.3, -2.0], rot: [25.0, 40.0, 0.0], size: [2.0, 1.5, 1.0] };
    let mx = xf.matrix();
    let plan = crate::profondeur::plan_de(&monde, &mx, eye).unwrap();
    let n = m::normalize(m::cross(m::dir(&mx, [1.0, 0.0, 0.0]), m::dir(&mx, [0.0, 1.0, 0.0])));
    let o = crate::ombres::OmbreCarte { plan, point: m::point(&mx, [0.0; 3]), normale: n, porte: true, recoit: true };
    let g = lum.gradient(&o);
    let centre = lum.vers_lumiere(m::point(&mx, [0.0; 3]));
    for (u, v) in [(0.3, 0.1), (-0.2, 0.35), (0.4, -0.4)] {
        let l = lum.vers_lumiere(m::point(&mx, [u, v, 0.0]));
        let attendu = centre[2] + g[0] * (l[0] - centre[0]) + g[1] * (l[1] - centre[1]);
        assert!((attendu - l[2]).abs() < 1e-4, "la pente suit le plan : {attendu} contre {}", l[2]);
    }
    // Le point du monde d'un pixel, par l'inverse de sa distance.
    let q = m::point(&mx, [0.25, -0.1, 0.0]);
    let s = monde.to_sheet(eye, q).unwrap();
    let p = lum.monde(s.x, s.y, crate::profondeur::inverse_distance(&plan, s.x, s.y));
    assert!(m::length(m::sub(p, q)) < 1e-3, "{p:?} contre {q:?}");
    // Le filtre : au soleil hors de la carte d'ombres ; la carte d'ombres de la carte elle-même ne l'ombre pas.
    let cote = lum.cote as usize;
    assert_eq!(lum.pcf(&vec![0.0; cote * cote], [1.5, 0.5, 0.5], [0.0; 2], 1), 1.0);
    let mut carte = vec![1.0f32; cote * cote];
    for j in 0..cote {
        for i in 0..cote {
            let (uu, vv) = ((i as f32 + 0.5) / cote as f32, (j as f32 + 0.5) / cote as f32);
            carte[j * cote + i] = centre[2] + g[0] * (uu - centre[0]) + g[1] * (vv - centre[1]);
        }
    }
    assert!(lum.pcf(&carte, centre, g, lum.rayon_cartes()) > 0.999, "pas dans sa propre ombre");
    assert!(lum.pcf(&carte, [centre[0], centre[1], centre[2] + 0.05], g, 1) < 0.001, "derrière elle : à l'ombre");
    // Les nombres du shader sont ceux du programme.
    let src = include_str!("gpu.rs");
    assert!(src.contains(&format!("const BIAIS: f32 = {};", crate::ombres::BIAIS)));
    assert!(src.contains(&format!("const PAS: f32 = {};", crate::ombres::PAS)));
    assert!(src.contains(&format!("const SEUIL_ALPHA: f32 = {};", crate::ombres::SEUIL_ALPHA)));
}

// ---------------------------------------------------------------------------------------------- par la carte graphique

/// La scène des essais : la dalle, et un personnage à 1,5 m derrière la feuille, debout sur elle (ses pieds sous le sol), fait de
/// ces rectangles d'encre.
fn scene(rects: &[[f32; 4]]) -> Document {
    let mut doc = projet();
    poser_decor(&mut doc, vec![dalle()], aspect());
    calque(&mut doc, "Perso", -1.5, rects);
    doc
}

#[test]
fn l_ombre_d_une_carte_tombe_sur_le_sol_devant_elle() {
    let Some(rs) = crate::gpu_tests::headless() else { return };
    let doc = scene(&[[-0.6, 0.6, -1.3, 0.5]]);
    let plan = crate::render::panel_plan_at(&doc, 0, 0, None);
    for q in [Qualite::Basses, Qualite::Hautes] {
        let avec = composee_plan(&rs, &doc, &plan, q);
        let sans = composee_plan(&rs, &doc, &sans_ombres(&plan), q);
        // Devant la carte, sur la dalle, sous son encre : à l'ombre.
        for z in [-1.2, -0.9, -0.5] {
            let p = pixel(&doc, [0.0, -1.0, z]);
            assert!(somme(px(&avec, p)) + 30 < somme(px(&sans, p)), "{q:?} : sous la carte, à z = {z} : {:?} contre {:?}", px(&avec, p), px(&sans, p));
        }
        // Au-delà de sa hauteur (1,5 m au-dessus du sol : l'ombre s'arrête à z = 0), à côté d'elle, derrière elle : rien ne change.
        for p in [[0.0, -1.0, 0.3], [2.0, -1.0, -0.9], [-2.2, -1.0, -0.5], [2.0, -1.0, -3.0]] {
            let q2 = pixel(&doc, p);
            let (a, b) = (px(&avec, q2), px(&sans, q2));
            assert!((somme(a) - somme(b)).abs() <= 3, "{q:?} : au soleil en {p:?} : {a:?} contre {b:?}");
        }
        // La carte elle-même, au soleil, n'est pas dans sa propre ombre.
        let haut = pixel(&doc, [0.0, 0.2, -1.5]);
        assert!(px(&avec, haut).b() >= 250, "{q:?} : {:?}", px(&avec, haut));
        // Le décor ailleurs : identique au pixel près.
        let loin: usize = avec.iter().zip(&sans).take(320 * 40).filter(|(a, b)| a != b).count();
        assert_eq!(loin, 0, "{q:?} : le haut de l'image ne change pas");
    }
}

#[test]
fn l_ombre_suit_l_encre_pas_le_rectangle() {
    let Some(rs) = crate::gpu_tests::headless() else { return };
    // Deux barres, un vide entre elles : le vide ne fait pas d'ombre.
    let doc = scene(&[[-0.7, -0.35, -1.3, 0.5], [0.35, 0.7, -1.3, 0.5]]);
    let plan = crate::render::panel_plan_at(&doc, 0, 0, None);
    let avec = composee_plan(&rs, &doc, &plan, Qualite::Basses);
    let sans = composee_plan(&rs, &doc, &sans_ombres(&plan), Qualite::Basses);
    for x in [-0.55, 0.55] {
        let p = pixel(&doc, [x, -1.0, -0.9]);
        assert!(somme(px(&avec, p)) + 30 < somme(px(&sans, p)), "sous la barre en x = {x}");
    }
    let p = pixel(&doc, [0.0, -1.0, -0.9]);
    assert!((somme(px(&avec, p)) - somme(px(&sans, p))).abs() <= 3, "entre les barres : au soleil, {:?} contre {:?}", px(&avec, p), px(&sans, p));
}

/// Une boîte haute derrière le personnage, entre le soleil et lui.
fn scene_boite() -> Document {
    let mut doc = projet();
    poser_decor(&mut doc, vec![dalle(), objet_boite(951, [-0.3, 0.0, -0.2], [0.3, 3.5, 0.2], [0.0, -1.0, -2.8])], aspect());
    calque(&mut doc, "Perso", -1.5, &[[-1.0, 1.0, -1.3, 0.8]]);
    doc
}

#[test]
fn une_boite_du_decor_ombre_la_carte_qui_est_devant_elle() {
    let Some(rs) = crate::gpu_tests::headless() else { return };
    let mut doc = scene_boite();
    let img = composee(&rs, &doc, Qualite::Hautes);
    let (dans, hors) = (pixel(&doc, [0.0, 0.0, -1.5]), pixel(&doc, [0.75, 0.0, -1.5]));
    assert!(px(&img, hors).b() >= 250 && px(&img, hors).r() < 10, "hors de l'ombre de la boîte : {:?}", px(&img, hors));
    let c = px(&img, dans);
    assert!(c.b() < 215 && c.b() > 120 && c.r() < 10 && c.a() == 255, "à l'ombre de la boîte, la carte s'assombrit : {c:?}");
    // « Reçoit les ombres » décoché : elle reste telle qu'elle est dessinée.
    doc.panels[0].layers[0].recoit_ombres = false;
    let img = composee(&rs, &doc, Qualite::Hautes);
    assert!(px(&img, dans).b() >= 250, "{:?}", px(&img, dans));
}

#[test]
fn une_carte_ombre_une_autre_carte() {
    let Some(rs) = crate::gpu_tests::headless() else { return };
    let mut doc = projet();
    poser_decor(&mut doc, vec![dalle()], aspect());
    // Un panneau haut à 2,6 m (entre le soleil et le mur), puis un mur peint à 1,5 m, au-dessus dans la pile.
    calque(&mut doc, "Panneau", -2.6, &[[-0.3, 0.3, 0.5, 3.0]]);
    calque(&mut doc, "Mur", -1.5, &[[-1.0, 1.0, -1.3, 0.8]]);
    let dans = pixel(&doc, [0.0, -0.2, -1.5]);
    let hors = pixel(&doc, [0.75, -0.2, -1.5]);
    let img = composee(&rs, &doc, Qualite::Hautes);
    assert!(px(&img, hors).b() >= 250, "{:?}", px(&img, hors));
    assert!(px(&img, dans).b() < 215, "le panneau ombre le mur : {:?}", px(&img, dans));
    // Le panneau ne porte pas d'ombre : le mur est au soleil.
    doc.panels[0].layers[0].porte_ombre = false;
    let img = composee(&rs, &doc, Qualite::Hautes);
    assert!(px(&img, dans).b() >= 250, "{:?}", px(&img, dans));
}

#[test]
fn sans_ombres_rien_ne_change() {
    let Some(rs) = crate::gpu_tests::headless() else { return };
    // Les cases décochées : la même image que sans les ombres des cartes.
    let mut doc = scene_boite();
    doc.panels[0].layers[0].porte_ombre = false;
    doc.panels[0].layers[0].recoit_ombres = false;
    let plan = crate::render::panel_plan_at(&doc, 0, 0, None);
    assert!(plan.items.iter().all(|i| i.ombre.is_none()));
    // Les ombres du décor sur cet ordinateur « sans ombres » (la qualité) : la même image que sans les ombres des cartes.
    let doc = scene_boite();
    let plan = crate::render::panel_plan_at(&doc, 0, 0, None);
    assert!(plan.items.iter().any(|i| i.ombre.is_some()));
    assert!(composee_plan(&rs, &doc, &plan, Qualite::Sans) == composee_plan(&rs, &doc, &sans_ombres(&plan), Qualite::Sans));
    // Une carte qui ne porte pas d'ombre et n'en reçoit pas, à la qualité haute : la même image que sans.
    let mut doc = scene_boite();
    doc.panels[0].layers[0].porte_ombre = false;
    let plan = crate::render::panel_plan_at(&doc, 0, 0, None);
    let mut seule = plan.clone();
    for i in &mut seule.items {
        if let Some(o) = i.ombre.as_mut() {
            o.recoit = false;
        }
    }
    assert!(composee_plan(&rs, &doc, &seule, Qualite::Hautes) == composee_plan(&rs, &doc, &sans_ombres(&plan), Qualite::Hautes));
    // L'aspect « Ombres » décoché : aucune ombre.
    let mut doc = scene_boite();
    let seq = doc.sequence_of(0);
    doc.decors3d.par_sequence.get_mut(&seq).unwrap().aspect.ombres = false;
    assert!(crate::render::panel_plan_at(&doc, 0, 0, None).items.iter().all(|i| i.ombre.is_none()));
    // Accroché à la caméra : pas d'ombre.
    let mut doc = scene_boite();
    doc.panels[0].layers[0].accroche = true;
    assert!(crate::render::panel_plan_at(&doc, 0, 0, None).items.iter().all(|i| i.ombre.is_none()));
}

#[test]
fn sans_decor_l_image_est_la_meme_au_bit_pres() {
    let mut doc = projet();
    calque(&mut doc, "A", -1.5, &[[-0.6, 0.6, -1.3, 0.5]]);
    calque(&mut doc, "B", -0.5, &[[0.2, 0.9, -0.3, 0.4]]);
    let plan = crate::render::panel_plan_at(&doc, 0, 0, None);
    assert!(plan.items.iter().all(|i| i.ombre.is_none() && i.profondeur.is_none()));
    // Des éléments qui porteraient des ombres, sans décor dessous : ignorées, par le processeur comme par la carte graphique.
    let mut faux = plan.clone();
    for i in &mut faux.items {
        i.ombre = Some(crate::ombres::OmbreCarte { plan: [0.0, 0.0, 0.2], point: [0.0, 0.0, -1.0], normale: [0.0, 0.0, 1.0], porte: true, recoit: true });
    }
    assert!(crate::render::composite_full(&doc, &plan) == crate::render::composite_full(&doc, &faux));
    assert!(crate::ombres::contexte(&doc, &faux).is_none(), "rien n'est préparé");
    if let Some(rs) = crate::gpu_tests::headless() {
        let a = composee_plan(&rs, &doc, &plan, Qualite::Hautes);
        assert!(!a.is_empty() && a == composee_plan(&rs, &doc, &faux, Qualite::Hautes), "sur la carte graphique aussi");
    }
}

// ---------------------------------------------------------------------------------------------- l'export

#[test]
fn l_export_a_les_memes_ombres_que_la_carte_graphique() {
    let Some(rs) = crate::gpu_tests::headless() else { return };
    // L'ombre de la carte sur la dalle.
    let doc = scene(&[[-0.6, 0.6, -1.3, 0.5]]);
    let ecran = composee(&rs, &doc, Qualite::Basses);
    let export = exportee(&rs, &doc, Qualite::Basses);
    let mut sans_doc = scene(&[[-0.6, 0.6, -1.3, 0.5]]);
    sans_doc.panels[0].layers[0].porte_ombre = false;
    let export_sans = exportee(&rs, &sans_doc, Qualite::Basses);
    let ecran_sans = composee(&rs, &sans_doc, Qualite::Basses);
    for z in [-1.2, -0.9, -0.5] {
        let p = pixel(&doc, [0.0, -1.0, z]);
        assert!(somme(px_cadre(&export, p)) + 30 < somme(px_cadre(&export_sans, p)), "à l'export aussi, l'ombre sur la dalle à z = {z}");
        assert!((somme(px_cadre(&export, p)) - somme(px(&ecran, p))).abs() <= 12, "l'export et l'écran : {:?} contre {:?}", px_cadre(&export, p), px(&ecran, p));
    }
    // Les pixels assombris, à l'écran et à l'export : presque les mêmes (les bords de l'ombre diffèrent d'un pixel ou deux).
    let (mut les_deux, mut un_seul) = (0, 0);
    for y in 46..134 {
        for x in 81..239 {
            let e = somme(px(&ecran, (x, y))) + 20 < somme(px(&ecran_sans, (x, y)));
            let x2 = somme(px_cadre(&export, (x, y))) + 20 < somme(px_cadre(&export_sans, (x, y)));
            match (e, x2) {
                (true, true) => les_deux += 1,
                (false, false) => {}
                _ => un_seul += 1,
            }
        }
    }
    assert!(les_deux > 100 && un_seul * 5 < les_deux, "{les_deux} assombris des deux côtés, {un_seul} d'un seul");
    // La boîte qui ombre la carte : à l'export aussi.
    let doc = scene_boite();
    let export = exportee(&rs, &doc, Qualite::Hautes);
    let ecran = composee(&rs, &doc, Qualite::Hautes);
    let (dans, hors) = (pixel(&doc, [0.0, 0.0, -1.5]), pixel(&doc, [0.75, 0.0, -1.5]));
    assert!(px_cadre(&export, hors).b() >= 250, "{:?}", px_cadre(&export, hors));
    assert!((px_cadre(&export, dans).b() as i32 - px(&ecran, dans).b() as i32).abs() <= 6, "{:?} contre {:?}", px_cadre(&export, dans), px(&ecran, dans));
    assert!(px_cadre(&export, dans).b() < 215);
}

#[test]
fn le_rendu_de_secours_n_a_pas_d_ombres_de_cartes() {
    // Sans carte graphique prêtée, le décor est rendu par le processeur, sans ombres : les cartes n'en ont pas non plus.
    use crate::exportrender::{snapshot_of, Papier, Rendu};
    let doc = scene(&[[-0.6, 0.6, -1.3, 0.5]]);
    let mut sans = scene(&[[-0.6, 0.6, -1.3, 0.5]]);
    sans.panels[0].layers[0].porte_ombre = false;
    sans.panels[0].layers[0].recoit_ombres = false;
    let image = |d: &Document| {
        let rendu = Rendu::new(snapshot_of(d, &[0]), Papier::Aucun, false, Vec::new(), Arc::new(std::sync::atomic::AtomicBool::new(false)));
        (*rendu.image(0, 0).expect("image")).clone()
    };
    assert!(image(&doc) == image(&sans));
}

// ---------------------------------------------------------------------------------------------- les cases, le fichier

#[test]
fn les_cases_sont_des_etapes_d_annulation_et_des_commandes() {
    let dir = std::env::temp_dir().join(format!("feanor-ombres-cases-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = scene(&[[-0.6, 0.6, -1.3, 0.5]]);
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    assert!(app.layers()[0].porte_ombre && app.layers()[0].recoit_ombres, "cochées d'office");
    let avant = app.history.undo_len();
    app.basculer_porte_ombre(0);
    assert!(!app.layers()[0].porte_ombre);
    assert_eq!(app.history.undo_len(), avant + 1, "une étape");
    app.undo();
    assert!(app.layers()[0].porte_ombre, "Ctrl+Z la défait");
    app.basculer_recoit_ombres(0);
    assert!(!app.layers()[0].recoit_ombres);
    app.undo();
    assert!(app.layers()[0].recoit_ombres);
    app.run_action(crate::ombres::ACTION_PORTE);
    assert!(!app.layers()[0].porte_ombre, "la commande de la palette");
    app.run_action(crate::ombres::ACTION_RECOIT);
    assert!(!app.layers()[0].recoit_ombres);
    let toml = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("keymap.toml")).unwrap();
    for a in [crate::ombres::ACTION_PORTE, crate::ombres::ACTION_RECOIT] {
        assert!(crate::keymap::ACTIONS.iter().any(|x| x.id == a), "{a}");
        assert!(toml.contains(&format!("action = \"{a}\"")), "son bloc dans le fichier des raccourcis : {a}");
    }
}

#[test]
fn les_cases_s_enregistrent_et_un_fichier_d_avant_s_ouvre_cochees() {
    let dir = std::env::temp_dir().join(format!("feanor-ombres-fichier-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("P.shot");
    let mut doc = projet();
    let r = [[-0.6, 0.6, -1.3, 0.5]];
    calque(&mut doc, "Sans ombre", -1.5, &r);
    calque(&mut doc, "Ne reçoit pas", -1.5, &r);
    calque(&mut doc, "Ordinaire", -1.0, &r);
    doc.panels[0].layers[0].porte_ombre = false;
    doc.panels[0].layers[1].recoit_ombres = false;
    crate::project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(&manifest).unwrap();
    assert_eq!(texte.matches("porte_ombre").count(), 1, "écrit pour le seul calque décoché");
    assert_eq!(texte.matches("recoit_ombres").count(), 1);
    assert!(texte.contains(&format!("\"version\": {}", crate::project::VERSION)) && crate::project::VERSION == 61, "le format ne change pas");
    let relu = crate::project::load(&manifest).unwrap().doc;
    let l = &relu.panels[0].layers;
    assert!(!l[0].porte_ombre && l[0].recoit_ombres);
    assert!(l[1].porte_ombre && !l[1].recoit_ombres);
    assert!(l[2].porte_ombre && l[2].recoit_ombres);
    // Un fichier d'avant (sans les champs, format 60) : il s'ouvre, les cases cochées.
    let ancien = texte.replace(&format!("\"version\": {}", crate::project::VERSION), "\"version\": 60");
    let ancien: String = ancien.lines().filter(|l| !l.contains("porte_ombre") && !l.contains("recoit_ombres")).collect::<Vec<_>>().join("\n");
    std::fs::write(&manifest, ancien).unwrap();
    let relu = crate::project::load(&manifest).expect("un projet d'avant s'ouvre").doc;
    assert!(relu.panels[0].layers.iter().all(|l| l.porte_ombre && l.recoit_ombres));
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

/// Mesures (à lancer à la main : `cargo test --lib ombres_perf -- --ignored --nocapture`) : huit cartes sur une feuille de 3840 ×
/// 2160 posées sur la grille d'essai du décor, composées avec les ombres des cartes et sans (les mêmes cartes, sans ombres), aux
/// deux qualités d'ombres, et le même plan sans décor ; des cartes de personnage (un huitième de la feuille) et des cartes pleines
/// (le pire cas). Et le rendu du décor avec sa troisième cible (la part du soleil) et sans.
#[test]
#[ignore]
fn ombres_perf() {
    let Some(rs) = crate::gpu_tests::headless() else { return };
    for (sorte, part) in [("d'un huitième de la feuille", 8usize), ("pleines", 1)] {
        let mut doc = Document::with_frame(1920, 1080, 24);
        doc.panels[0].length = 24;
        doc.panels[0].layers.clear();
        let (w, h) = (doc.width, doc.height);
        let (cw, ch) = if part == 1 { (w, h) } else { (w / 4, h / 2) };
        for k in 0..8 {
            let x0 = (k * w / 8).min(w - cw);
            let c = doc.new_cel(LayerKind::Bitmap);
            let r = &mut doc.cels.get_mut(&c).unwrap().raster;
            for y in (h - ch) / 2..(h + ch) / 2 {
                for x in x0..x0 + cw {
                    let idx = r.tile_index_of(x, y);
                    let (ox, oy) = r.tile_origin(idx);
                    r.tile_mut(idx)[(y as i64 - oy) as usize * crate::raster::TILE + (x as i64 - ox) as usize] = BLEU;
                }
            }
            let mut l = Layer::new(&format!("Carte {k}"), LayerKind::Bitmap);
            l.frames = vec![Some(c); 24];
            l.depth = (0.5 + k as f32) / Monde::of(&doc).d0;
            doc.panels[0].layers.push(l);
        }
        let sans_decor = crate::render::panel_plan_at(&doc, 0, 0, None);
        let t_sans_decor = temps_de_composition(&rs, &doc, &sans_decor, 20);
        // (La grille d'essai en sol, sous l'œil : elle couvre le bas de la feuille, où tombent les ombres des cartes.)
        let mut o = crate::decor3d_tests::objet(953, crate::decor3dbudget::scene_d_essai(300, 512), 0.0, 0.0);
        o.origine = [0.0; 3];
        o.aimante = false;
        poser_decor(&mut doc, vec![o], Aspect { ciel: true, brume: true, ..aspect() });
        let avec = crate::render::panel_plan_at(&doc, 0, 0, None);
        assert_eq!(avec.items.iter().filter(|i| i.ombre.is_some()).count(), 8);
        let sans = sans_ombres(&avec);
        for q in [Qualite::Basses, Qualite::Hautes] {
            crate::decor3d::essai_poser(Some(q), None);
            let t_sans = temps_de_composition(&rs, &doc, &sans, 20);
            let t_avec = temps_de_composition(&rs, &doc, &avec, 20);
            crate::decor3d::essai_poser(None, None);
            let (a, b) = (composee_plan(&rs, &doc, &avec, q), composee_plan(&rs, &doc, &sans, q));
            let changes = a.iter().zip(&b).filter(|(x, y)| x != y).count();
            assert!(changes > 1000, "les ombres sont bien là : {changes} pixels");
            // L'export par le processeur, le décor rendu par la carte graphique prêtée : une image (la carte d'ombres des cartes refaite).
            let export = |d: &Document, plan: &Plan| {
                crate::decor3d::essai_poser(Some(q), Some(&rs));
                if let Some(FondPart::Decor(key)) = &plan.items[0].fond {
                    let _ = crate::decor3d::montre(d, key);
                }
                let mut meilleur = f32::MAX;
                for _ in 0..3 {
                    crate::ombres::oublier();
                    let debut = std::time::Instant::now();
                    let img = crate::render::camera_image(d, plan, &d.panel_camera(0, 0));
                    meilleur = meilleur.min(debut.elapsed().as_secs_f32() * 1000.0);
                    assert!(!img.is_empty());
                }
                crate::decor3d::essai_poser(None, None);
                meilleur
            };
            let (e_sans, e_avec) = (export(&doc, &sans), export(&doc, &avec));
            println!("  export par le processeur (cadre 1920 × 1080) : cartes sans ombres {e_sans:.0} ms, avec {e_avec:.0} ms");
            println!("8 cartes {sorte}, feuille {w} × {h}, {} : sans décor {t_sans_decor:.2} ms ; décor, cartes sans ombres {t_sans:.2} ms ; ombres des cartes {t_avec:.2} ms (+{:.2} ms), {changes} pixels assombris", q.nom(), t_avec - t_sans);
        }
    }
    // Le décor seul : sa troisième cible (la part du soleil) coûte-t-elle ?
    let doc = Document::with_frame(1920, 1080, 24);
    let monde = Monde::of(&doc);
    let prise = crate::decor3drendu::Prise::de_la_feuille(&monde, monde.eye_reference()).unwrap();
    for cote in [100usize, 300, 700] {
        let modele = Arc::new(crate::decor3d::Modele3d::new(Arc::new(crate::decor3dbudget::scene_d_essai(cote, 512)), Arc::new(Vec::new())));
        let d = crate::decor3d::dessins_gpu(&[(modele, m::translation([0.0, -1.0, -3.0]))], Aspect { ciel: true, brume: true, ..aspect() });
        for q in [Qualite::Basses, Qualite::Hautes] {
            let mut mt = crate::decor3dgpu::Moteur::new(&rs.device, &rs.queue);
            let taille = [monde.stage[0] as u32, monde.stage[1] as u32];
            let t2 = crate::decor3dgpu::rendre_hors_ecran_tout(&rs.device, &rs.queue, &mut mt, &d, &prise, taille, true, q, 8, false, false).map(|r| r.1).unwrap_or(0.0);
            let t3 = crate::decor3dgpu::rendre_hors_ecran_tout(&rs.device, &rs.queue, &mut mt, &d, &prise, taille, true, q, 8, false, true).map(|r| r.1).unwrap_or(0.0);
            println!("décor de {} triangles, {} : deux cibles {t2:.2} ms, avec la part du soleil {t3:.2} ms", d[0].scene.triangles(), q.nom());
        }
    }
}
