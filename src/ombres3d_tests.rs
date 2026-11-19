//! Essais de la partie 189 : **les ombres des cartes et du décor 3D dans la vue Espace 3D** (`ombres3d.rs`). L'application entière
//! est peinte par la carte graphique (le banc des essais de mesure, `perf_tests::Banc` : les textures d'egui, le décor de la vue, les
//! cartes, leurs ombres), la vue Espace 3D ouverte en plein écran. Sous le soleil des essais de la partie 185 (derrière le décor, à
//! 45°) : l'ombre d'une carte tombe sur la dalle devant elle, vue de côté comme de face, au même endroit du monde de quelque côté
//! qu'on tourne ; une boîte haute derrière la carte l'assombrit ; une carte en ombre une autre ; les mêmes couleurs que l'image du
//! plan ; les cases décochées, les ombres du décor éteintes ou un calque accroché à la caméra : la vue d'avant, au pixel près ; la
//! carte d'ombres des cartes n'est refaite que si une carte bouge, une fois pour toutes les vues.

use eframe::egui::{Color32, Rect};

use crate::decor3d::{Decor3d, Objet3d};
use crate::decor3d_tests::boite;
use crate::decor3dbudget::Qualite;
use crate::decor3drendu::Aspect;
use crate::document::{Document, Layer, LayerKind};
use crate::espace3d::Monde;
use crate::espace3dmath::{Orbite, V3};
use crate::perf_tests::{banc_taille, Banc};

const BLEU: Color32 = Color32::from_rgb(0, 0, 255);

// ---------------------------------------------------------------------------------------------- la scène (celle de la partie 185)

fn projet() -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.panels[0].length = 24;
    doc.panels[0].layers.clear();
    doc
}

fn objet_boite(id: u64, a: [f32; 3], b: [f32; 3], pos: [f32; 3]) -> Objet3d {
    let mut o = crate::decor3d_tests::objet(id, boite(a, b), pos[0], pos[2]);
    o.place.position = pos;
    o.origine = [0.0; 3];
    o.aimante = false;
    o
}

/// La dalle : le sol à y = −1, de z = −6,5 à 0,5, sur 8 m de large.
fn dalle() -> Objet3d {
    objet_boite(950, [-4.0, -0.1, -6.5], [4.0, 0.0, 0.5], [0.0, -1.0, 0.0])
}

/// Le soleil de derrière le décor, à 45° : l'ombre d'une carte tombe devant elle, vers la caméra du plan.
fn aspect() -> Aspect {
    Aspect { ciel: false, brume: false, ombres: true, soleil_azimut: 180.0, soleil_hauteur: 45.0, ..Aspect::default() }
}

fn poser_decor(doc: &mut Document, objets: Vec<Objet3d>) {
    let seq = doc.sequence_of(0);
    let mut d = Decor3d::new(aspect());
    d.objets = objets;
    doc.decors3d.par_sequence.insert(seq, d);
}

/// Un calque dont la carte est à `z` mètres, avec de l'encre bleue sur les rectangles du monde `rects` (x0, x1, y0, y1 en mètres,
/// sur le plan de la carte). Son rang.
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

/// La dalle et un personnage à 1,5 m derrière la feuille, debout sur elle (ses pieds sous le sol).
fn scene() -> Document {
    let mut doc = projet();
    poser_decor(&mut doc, vec![dalle()]);
    calque(&mut doc, "Perso", -1.5, &[[-0.6, 0.6, -1.3, 0.5]]);
    doc
}

/// La dalle, une boîte haute derrière le personnage (entre le soleil et lui), le personnage.
fn scene_boite() -> Document {
    let mut doc = projet();
    poser_decor(&mut doc, vec![dalle(), objet_boite(951, [-0.3, 0.0, -0.2], [0.3, 3.5, 0.2], [0.0, -1.0, -2.8])]);
    calque(&mut doc, "Perso", -1.5, &[[-1.0, 1.0, -1.3, 0.8]]);
    doc
}

// ---------------------------------------------------------------------------------------------- le banc

/// Le banc, la carte graphique comprise (rien sans elle), la vue Espace 3D en plein écran, les ombres du décor à la qualité `q`, le
/// quadrillage et la sélection retirés (rien ne se peint par-dessus les points regardés).
fn banc_vue(doc: Document, q: Qualite) -> Option<Banc> {
    crate::decor3d::essai_poser(Some(q), None);
    let mut b = banc_taille(doc, true, 960, 640);
    b.rs.as_ref()?;
    b.chauffer(2);
    b.app.cur_layer = 0;
    b.app.run_action("espace3d.basculer");
    b.app.espace3d.show_grid = false;
    b.chauffer(3);
    assert!(b.app.espace3d.on, "la vue Espace 3D est ouverte");
    b.app.espace3d.sel.clear();
    Some(b)
}

/// Regarde depuis l'orbite (`azimut`, `hauteur`, en degrés) autour du point (0, −1, −1), à 8 m, et peint quelques images.
fn regarder(b: &mut Banc, azimut: f32, hauteur: f32) {
    b.app.espace3d.cam = Some(Orbite { target: [0.0, -1.0, -1.0], azimuth: azimut, elevation: hauteur, distance: 8.0, focal: 35.0 });
    b.app.espace3d.sel.clear();
    b.chauffer(3);
}

/// Ce que l'écran montre, et où la vue met chaque point du monde.
struct Ecran {
    rgba: Vec<u8>,
    largeur: usize,
    draw: Rect,
    view: crate::espace3dmath::View,
}

fn ecran(b: &Banc) -> Ecran {
    let draw = b.app.espace3d.rect;
    Ecran { rgba: b.capturer().expect("capture"), largeur: b.taille.x as usize, draw, view: b.app.e3_view(draw) }
}

impl Ecran {
    fn en(&self, p: V3) -> (usize, usize) {
        let q = self.view.project(p, [self.draw.width() / 2.0, self.draw.height() / 2.0]).expect("devant l'œil");
        let (x, y) = (self.draw.left() + q[0], self.draw.top() + q[1]);
        assert!(self.draw.contains(eframe::egui::pos2(x, y)), "{p:?} est dans la vue");
        (x.floor() as usize, y.floor() as usize)
    }

    fn px(&self, p: V3) -> [i32; 3] {
        let (x, y) = self.en(p);
        let i = (y * self.largeur + x) * 4;
        [self.rgba[i] as i32, self.rgba[i + 1] as i32, self.rgba[i + 2] as i32]
    }

    /// Les pixels de la vue (seulement elle : la barre d'état et les panneaux peuvent changer d'une image à l'autre).
    fn vue(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for y in self.draw.top().ceil() as usize..self.draw.bottom().floor() as usize {
            let a = (y * self.largeur + self.draw.left().ceil() as usize) * 4;
            let b = (y * self.largeur + self.draw.right().floor() as usize) * 4;
            out.extend_from_slice(&self.rgba[a..b]);
        }
        out
    }
}

fn somme(c: [i32; 3]) -> i32 {
    c.iter().sum()
}

/// La même vue, sans rien de la partie 189 (aucune ombre de carte préparée : le chemin d'avant).
fn avant_189(b: &mut Banc) -> Ecran {
    crate::ombres3d::AVANT_189.with(|a| a.set(true));
    b.chauffer(3);
    let e = ecran(b);
    crate::ombres3d::AVANT_189.with(|a| a.set(false));
    b.chauffer(3);
    e
}

/// Ce que la vue prépare des ombres des cartes à cette image (rien : la vue d'avant).
fn ombres(b: &mut Banc) -> Option<crate::ombres3d::OmbresDeLaVue> {
    let ctx = b.ctx.clone();
    b.app.e3_ombres_de_la_vue(&ctx)
}

fn cartes_d_ombres_faites(b: &Banc) -> u64 {
    let rs = b.rs.as_ref().expect("carte graphique");
    let r = rs.renderer.read();
    r.callback_resources.get::<crate::decor3dgpu::Moteur>().map_or(0, |m| m.cartes_d_ombres_faites)
}

fn fin() {
    crate::decor3d::essai_poser(None, None);
}

// ---------------------------------------------------------------------------------------------- les ombres sur le décor

/// Les points de la dalle : sous l'ombre de la carte (devant elle), et au soleil (au-delà de sa hauteur, à côté, derrière).
const A_L_OMBRE: [V3; 3] = [[0.0, -1.0, -1.2], [0.0, -1.0, -0.9], [0.0, -1.0, -0.5]];
const AU_SOLEIL: [V3; 3] = [[0.0, -1.0, 0.35], [2.0, -1.0, -0.9], [-2.2, -1.0, -0.5]];

#[test]
fn vue_de_cote_l_ombre_d_une_carte_tombe_sur_la_dalle() {
    let Some(mut b) = banc_vue(scene(), Qualite::Hautes) else { return fin() };
    for q in [Qualite::Hautes, Qualite::Basses] {
        crate::decor3d::essai_poser(Some(q), None);
        regarder(&mut b, 90.0, 30.0);
        assert!(ombres(&mut b).is_some_and(|o| o.cartes.porteuses.len() == 1 && o.recoivent == vec![0]), "la carte porte une ombre");
        let avec = ecran(&b);
        b.app.doc.panels[0].layers[0].porte_ombre = false;
        b.chauffer(3);
        let sans = ecran(&b);
        b.app.doc.panels[0].layers[0].porte_ombre = true;
        for p in A_L_OMBRE {
            assert!(somme(avec.px(p)) + 30 < somme(sans.px(p)), "{q:?} : à l'ombre en {p:?} : {:?} contre {:?}", avec.px(p), sans.px(p));
        }
        for p in AU_SOLEIL {
            assert!((somme(avec.px(p)) - somme(sans.px(p))).abs() <= 3, "{q:?} : au soleil en {p:?} : {:?} contre {:?}", avec.px(p), sans.px(p));
        }
    }
    fin();
}

#[test]
fn l_ombre_reste_au_meme_endroit_du_monde_quand_la_vue_tourne() {
    let Some(mut b) = banc_vue(scene(), Qualite::Hautes) else { return fin() };
    let mut ombres = Vec::new();
    for (azimut, hauteur) in [(0.0, 25.0), (40.0, 35.0), (90.0, 30.0), (-60.0, 45.0), (20.0, 70.0)] {
        regarder(&mut b, azimut, hauteur);
        let avec = ecran(&b);
        b.app.doc.panels[0].layers[0].porte_ombre = false;
        b.chauffer(3);
        let sans = ecran(&b);
        b.app.doc.panels[0].layers[0].porte_ombre = true;
        for p in A_L_OMBRE {
            assert!(somme(avec.px(p)) + 30 < somme(sans.px(p)), "vue ({azimut}°, {hauteur}°) : à l'ombre en {p:?} : {:?} contre {:?}", avec.px(p), sans.px(p));
        }
        for p in AU_SOLEIL {
            assert!((somme(avec.px(p)) - somme(sans.px(p))).abs() <= 3, "vue ({azimut}°, {hauteur}°) : au soleil en {p:?}");
        }
        ombres.push(avec.px(A_L_OMBRE[1]));
        // Vue de devant, la carte elle-même (qui reçoit les ombres) n'est pas dans sa propre ombre.
        if azimut.abs() < 45.0 && hauteur < 45.0 {
            let c = avec.px([0.0, 0.2, -1.5]);
            assert!(c[2] >= 250 && c[0] < 10, "vue ({azimut}°, {hauteur}°) : la carte au soleil : {c:?}");
        }
    }
    // La dalle n'a pas de reflets : le même point à l'ombre a la même couleur de partout.
    for c in &ombres {
        assert!((somme(*c) - somme(ombres[0])).abs() <= 9, "la même ombre de chaque côté : {ombres:?}");
    }
    fin();
}

// ---------------------------------------------------------------------------------------------- les ombres sur les cartes

#[test]
fn une_boite_du_decor_ombre_la_carte_dans_la_vue() {
    let Some(mut b) = banc_vue(scene_boite(), Qualite::Hautes) else { return fin() };
    for (azimut, hauteur) in [(15.0, 10.0), (-35.0, 20.0)] {
        regarder(&mut b, azimut, hauteur);
        let e = ecran(&b);
        let (dans, hors) = (e.px([0.0, 0.0, -1.5]), e.px([0.75, 0.0, -1.5]));
        assert!(hors[2] >= 250 && hors[0] < 10, "vue ({azimut}°) : hors de l'ombre de la boîte, la carte telle qu'elle est : {hors:?}");
        assert!(dans[2] < 215 && dans[2] > 120 && dans[0] < 10, "vue ({azimut}°) : à l'ombre de la boîte, la carte s'assombrit : {dans:?}");
    }
    // « Reçoit les ombres » décoché : elle reste telle qu'elle est dessinée.
    b.app.doc.panels[0].layers[0].recoit_ombres = false;
    b.chauffer(3);
    let e = ecran(&b);
    assert!(e.px([0.0, 0.0, -1.5])[2] >= 250, "{:?}", e.px([0.0, 0.0, -1.5]));
    fin();
}

#[test]
fn une_carte_ombre_une_autre_carte_dans_la_vue() {
    let mut doc = projet();
    poser_decor(&mut doc, vec![dalle()]);
    calque(&mut doc, "Panneau", -2.6, &[[-0.3, 0.3, 0.5, 3.0]]);
    calque(&mut doc, "Mur", -1.5, &[[-1.0, 1.0, -1.3, 0.8]]);
    let Some(mut b) = banc_vue(doc, Qualite::Hautes) else { return fin() };
    regarder(&mut b, 10.0, 5.0);
    let e = ecran(&b);
    let (dans, hors) = ([0.0, -0.2, -1.5], [0.75, -0.2, -1.5]);
    assert!(e.px(hors)[2] >= 250, "{:?}", e.px(hors));
    assert!(e.px(dans)[2] < 215, "le panneau ombre le mur : {:?}", e.px(dans));
    b.app.doc.panels[0].layers[0].porte_ombre = false;
    b.chauffer(3);
    assert!(ecran(&b).px(dans)[2] >= 250);
    fin();
}

#[test]
fn la_vue_assombrit_comme_l_image_du_plan() {
    let Some(mut b) = banc_vue(scene_boite(), Qualite::Hautes) else { return fin() };
    regarder(&mut b, 15.0, 10.0);
    let e = ecran(&b);
    // L'image du plan, par la carte graphique (partie 185), à la même qualité.
    let rs = b.rs.clone().expect("carte graphique");
    let doc = &b.app.doc;
    let plan = crate::render::panel_plan_at(doc, 0, 0, None);
    let mut gpu = crate::gpu::GpuCanvas::new(&rs);
    let _ = crate::gpu_tests::render_with(&mut gpu, &rs, doc, &plan, true);
    let img = gpu.read_frame();
    let monde = Monde::of(doc);
    let feuille = |p: V3| {
        let s = monde.to_sheet(monde.eye_reference(), p).unwrap();
        let c = img[s.y.floor() as usize * 320 + s.x.floor() as usize];
        [c.r() as i32, c.g() as i32, c.b() as i32]
    };
    // La carte à l'ombre de la boîte : le même bleu assombri.
    let (vue, plan_px) = (e.px([0.0, 0.0, -1.5]), feuille([0.0, 0.0, -1.5]));
    assert!(plan_px[2] < 215, "l'image du plan l'assombrit : {plan_px:?}");
    assert!((vue[2] - plan_px[2]).abs() <= 6 && vue[0] < 10, "la vue comme l'image du plan : {vue:?} contre {plan_px:?}");
    // La dalle à l'ombre de la carte, et au soleil : la même couleur.
    let (ombre, soleil) = ([0.6, -1.0, -0.6], [2.0, -1.0, -0.9]);
    for p in [ombre, soleil] {
        let (v, f) = (e.px(p), feuille(p));
        assert!((0..3).all(|k| (v[k] - f[k]).abs() <= 6), "la dalle en {p:?} : {v:?} contre {f:?}");
    }
    assert!(somme(feuille(ombre)) + 30 < somme(feuille(soleil)), "le premier point est bien à l'ombre de la carte : {:?} contre {:?}", feuille(ombre), feuille(soleil));
    fin();
}

// ---------------------------------------------------------------------------------------------- rien ne change sans ombres

#[test]
fn sans_ombres_la_vue_est_celle_d_avant_au_pixel_pres() {
    // Les deux cases décochées : rien n'est préparé, la vue d'avant.
    let mut doc = scene_boite();
    doc.panels[0].layers[0].porte_ombre = false;
    doc.panels[0].layers[0].recoit_ombres = false;
    let Some(mut b) = banc_vue(doc, Qualite::Hautes) else { return fin() };
    regarder(&mut b, 15.0, 10.0);
    assert!(ombres(&mut b).is_none());
    let a = ecran(&b).vue();
    assert!(a == avant_189(&mut b).vue(), "les cases décochées");
    // Accroché à la caméra : jamais d'ombre.
    b.app.doc.panels[0].layers[0].porte_ombre = true;
    b.app.doc.panels[0].layers[0].recoit_ombres = true;
    b.app.doc.panels[0].layers[0].accroche = true;
    b.chauffer(3);
    assert!(ombres(&mut b).is_none(), "un calque accroché ne porte ni ne reçoit");
    let a = ecran(&b).vue();
    assert!(a == avant_189(&mut b).vue(), "accroché à la caméra");
    // L'aspect « Ombres » décoché : ni ombres du décor ni ombres des cartes, la vue d'avant.
    b.app.doc.panels[0].layers[0].accroche = false;
    let seq = b.app.doc.sequence_of(0);
    b.app.doc.decors3d.par_sequence.get_mut(&seq).unwrap().aspect.ombres = false;
    b.chauffer(3);
    assert!(ombres(&mut b).is_none());
    let a = ecran(&b).vue();
    assert!(a == avant_189(&mut b).vue(), "sans l'aspect Ombres");
    // Un ordinateur « sans ombres » : de même.
    b.app.doc.decors3d.par_sequence.get_mut(&seq).unwrap().aspect.ombres = true;
    crate::decor3d::essai_poser(Some(Qualite::Sans), None);
    b.chauffer(3);
    assert!(ombres(&mut b).is_none());
    let a = ecran(&b).vue();
    assert!(a == avant_189(&mut b).vue(), "sans ombres sur cet ordinateur");
    // Et avec les ombres : la vue change bien (l'essai compare quelque chose).
    crate::decor3d::essai_poser(Some(Qualite::Hautes), None);
    b.chauffer(3);
    assert!(ombres(&mut b).is_some());
    let a = ecran(&b).vue();
    assert!(a != avant_189(&mut b).vue(), "avec les ombres, la vue change");
    fin();
}

// ---------------------------------------------------------------------------------------------- ce que ça coûte

#[test]
fn la_carte_d_ombres_des_cartes_n_est_refaite_que_si_une_carte_bouge() {
    let Some(mut b) = banc_vue(scene(), Qualite::Hautes) else { return fin() };
    regarder(&mut b, 30.0, 30.0);
    let n = cartes_d_ombres_faites(&b);
    assert!(n >= 1);
    // La vue tourne : le soleil ne bouge pas, la carte d'ombres reste.
    for k in 0..4 {
        regarder(&mut b, 30.0 + 10.0 * k as f32, 30.0);
    }
    assert_eq!(cartes_d_ombres_faites(&b), n, "la vue tourne, rien n'est refait");
    // La carte avance : refaite, une fois.
    b.app.doc.panels[0].layers[0].depth += 0.05;
    b.chauffer(3);
    assert_eq!(cartes_d_ombres_faites(&b), n + 1, "la carte a bougé");
    // Quatre vues : une seule carte d'ombres pour toutes, et l'ombre se voit dans plus d'une.
    b.app.e3_vues_poser(crate::espace3dvues::Disposition::Quatre);
    b.chauffer(4);
    let avant = cartes_d_ombres_faites(&b);
    b.app.doc.panels[0].layers[0].depth -= 0.05;
    b.chauffer(3);
    assert_eq!(cartes_d_ombres_faites(&b), avant + 1, "une fois pour les quatre vues");
    let avec = b.capturer().unwrap();
    b.app.doc.panels[0].layers[0].porte_ombre = false;
    b.chauffer(3);
    let sans = b.capturer().unwrap();
    let w = b.taille.x as usize;
    let rects = b.app.e3_vues_rects(b.app.espace3d.rect);
    let changees = rects
        .iter()
        .filter(|(_, r)| {
            let mut n = 0;
            for y in r.top() as usize + 2..r.bottom() as usize - 2 {
                for x in r.left() as usize + 2..r.right() as usize - 2 {
                    let i = (y * w + x) * 4;
                    let (a, s) = (avec[i] as i32 + avec[i + 1] as i32 + avec[i + 2] as i32, sans[i] as i32 + sans[i + 1] as i32 + sans[i + 2] as i32);
                    n += (a + 30 < s) as usize;
                }
            }
            n > 20
        })
        .count();
    assert!(changees >= 2, "l'ombre de la carte se voit dans {changees} vues");
    fin();
}

#[test]
fn les_nombres_des_shaders_de_la_vue_sont_ceux_du_programme() {
    let src = include_str!("decor3dgpu.rs");
    assert!(src.contains(&format!("const SEUIL_ALPHA: f32 = {};", crate::ombres::SEUIL_ALPHA)));
    // Le calcul des ombres est celui de l'image du plan (le même texte, `gpu::OMBRES_COMMUN`).
    assert!(crate::gpu::OMBRES_COMMUN.contains("fn soleil_vu_par_la_carte") && crate::gpu::OMBRES_COMMUN.contains("fn soleil_des_cartes_sur_le_decor"));
    assert!(src.contains("soleil_vu_par_la_carte(lum_vers(o.monde)") && src.contains("return soleil_des_cartes_sur_le_decor(l);"));
    // La carte grossière regarde assez loin autour du point : le filtre du décor lit au plus 2 × PAS texels plus un.
    assert!(src.contains("const CASE_GROSSIERE: u32 = 8;") && src.contains("(t - 4.0) / 8.0") && 2.0 * crate::ombres::PAS + 1.0 <= 4.0);
}

#[test]
fn la_carte_grossiere_ne_change_rien_a_l_image() {
    // Le décor sous l'ombre d'une carte, ses bords flous compris : le même au pixel près, que la carte grossière épargne le filtre
    // ou non (une carte inclinée, deux barres, aux deux qualités).
    let mut doc = projet();
    poser_decor(&mut doc, vec![dalle()]);
    calque(&mut doc, "Barres", -1.5, &[[-0.7, -0.35, -1.3, 0.5], [0.35, 0.7, -1.3, 0.5]]);
    calque(&mut doc, "Fine", -0.8, &[[1.0, 1.03, -1.3, 1.0]]);
    let Some(mut b) = banc_vue(doc, Qualite::Hautes) else { return fin() };
    b.app.doc.panels[0].layers[0].transform.tilt_y = 25.0;
    for q in [Qualite::Hautes, Qualite::Basses] {
        crate::decor3d::essai_poser(Some(q), None);
        for (azimut, hauteur) in [(20.0, 35.0), (70.0, 60.0)] {
            regarder(&mut b, azimut, hauteur);
            let avec = ecran(&b).vue();
            crate::decor3dgpu::ESSAI_SANS_GROSSIERE.with(|e| e.set(true));
            b.chauffer(3);
            let sans = ecran(&b).vue();
            crate::decor3dgpu::ESSAI_SANS_GROSSIERE.with(|e| e.set(false));
            let differents = avec.chunks(4).zip(sans.chunks(4)).filter(|(a, s)| a != s).count();
            assert_eq!(differents, 0, "{q:?}, vue ({azimut}°, {hauteur}°)");
        }
    }
    fin();
}

/// Le temps d'une image d'écran sur la carte graphique (millisecondes) : ce que les images `formes` (l'une après l'autre, en boucle)
/// envoient, `n` images enchaînées dans la file comme à l'écran (la carte ne s'arrête pas entre deux), la meilleure de cinq séries.
fn temps_de_peinture(b: &Banc, formes: &[Vec<eframe::egui::epaint::ClippedShape>], n: usize) -> f64 {
    use eframe::egui_wgpu::{wgpu, ScreenDescriptor};
    let rs = b.rs.as_ref().expect("carte graphique");
    let (w, h) = (b.taille.x as u32, b.taille.y as u32);
    let cible = rs.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("écran"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: rs.target_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let vue = cible.create_view(&Default::default());
    let prims: Vec<_> = formes.iter().map(|f| b.ctx.tessellate(f.clone(), 1.0)).collect();
    let screen = ScreenDescriptor { size_in_pixels: [w, h], pixels_per_point: 1.0 };
    let mut meilleur = f64::MAX;
    for serie in 0..6 {
        let debut = std::time::Instant::now();
        for i in 0..n {
            let p = &prims[i % prims.len()];
            let mut enc = rs.device.create_command_encoder(&Default::default());
            let extra = rs.renderer.write().update_buffers(&rs.device, &rs.queue, &mut enc, p, &screen);
            {
                let mut pass = enc
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("écran"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &vue, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    })
                    .forget_lifetime();
                rs.renderer.read().render(&mut pass, p, &screen);
            }
            rs.queue.submit(extra.into_iter().chain([enc.finish()]));
        }
        let _ = rs.device.poll(wgpu::Maintain::Wait);
        // (La première série chauffe la carte.)
        if serie > 0 {
            meilleur = meilleur.min(debut.elapsed().as_secs_f64() * 1e3 / n as f64);
        }
    }
    meilleur
}

/// Les formes de l'image d'écran que la vue fait de l'orbite `azimut` (degrés), les calques posés par `f`.
fn formes_de(b: &mut Banc, azimut: f32, f: impl Fn(&mut Layer)) -> Vec<eframe::egui::epaint::ClippedShape> {
    for l in &mut b.app.doc.panels[0].layers {
        f(l);
    }
    b.app.espace3d.cam = Some(Orbite { target: [0.0, -1.0, -3.0], azimuth: azimut, elevation: 25.0, distance: 9.0, focal: 35.0 });
    b.garder_formes = true;
    b.chauffer(3);
    b.garder_formes = false;
    b.formes.clone()
}

/// Les mesures de la partie 189 (`docs/INTERFACE.md`, 189.3) : l'image d'écran de l'application, la vue Espace 3D en plein écran
/// (écran de 1920 × 1080), huit cartes de personnage debout sur la grille d'essai en sol (180 000 triangles, ciel, brume), ombres du
/// décor hautes et basses : sans les ombres des cartes, avec (les cartes immobiles : la carte d'ombres des cartes gardée), et avec
/// des cartes qui bougent à chaque image (la carte d'ombres refaite). `cargo test --lib ombres3d_perf -- --ignored --nocapture`
#[test]
#[ignore]
fn ombres3d_perf() {
    let mut doc = Document::with_frame(960, 540, 24);
    doc.panels[0].length = 24;
    doc.panels[0].layers.clear();
    for k in 0..8 {
        let x = -3.5 + k as f32;
        calque(&mut doc, &format!("Perso {k}"), -1.0 - 0.6 * k as f32, &[[x - 0.3, x + 0.3, -1.3, 0.5]]);
    }
    let mut o = crate::decor3d_tests::objet(953, crate::decor3dbudget::scene_d_essai(300, 512), 0.0, 0.0);
    o.origine = [0.0; 3];
    o.aimante = false;
    o.place.position = [0.0, -1.0, -3.0];
    let triangles = o.modele.scene.triangles();
    let seq = doc.sequence_of(0);
    let mut d = Decor3d::new(Aspect { ciel: true, brume: true, ..aspect() });
    d.objets = vec![o];
    doc.decors3d.par_sequence.insert(seq, d);
    crate::decor3d::essai_poser(Some(Qualite::Hautes), None);
    let mut b = banc_taille(doc, true, 1920, 1080);
    if b.rs.is_none() {
        return fin();
    }
    b.chauffer(2);
    b.app.run_action("espace3d.basculer");
    b.app.espace3d.sel.clear();
    b.chauffer(4);
    let rect = b.app.espace3d.rect;
    println!("vue Espace 3D {:.0} × {:.0} pixels, décor de {triangles} triangles, 8 cartes", rect.width(), rect.height());
    for q in [Qualite::Hautes, Qualite::Basses] {
        crate::decor3d::essai_poser(Some(q), None);
        let sans = formes_de(&mut b, 20.0, |l| {
            l.porte_ombre = false;
            l.recoit_ombres = false;
        });
        let avec = formes_de(&mut b, 20.0, |l| {
            l.porte_ombre = true;
            l.recoit_ombres = true;
        });
        let portees = formes_de(&mut b, 20.0, |l| l.recoit_ombres = false);
        let recues = formes_de(&mut b, 20.0, |l| {
            l.recoit_ombres = true;
            l.porte_ombre = false;
        });
        let bouge_a = formes_de(&mut b, 20.0, |l| {
            l.porte_ombre = true;
            l.depth += 0.002;
        });
        let bouge_b = formes_de(&mut b, 20.0, |l| l.depth -= 0.002);
        for passage in 0..3 {
            let t_sans = temps_de_peinture(&b, &[sans.clone()], 40);
            let t_avec = temps_de_peinture(&b, &[avec.clone()], 40);
            let t_portees = temps_de_peinture(&b, &[portees.clone()], 40);
            let t_recues = temps_de_peinture(&b, &[recues.clone()], 40);
            let t_bouge = temps_de_peinture(&b, &[bouge_a.clone(), bouge_b.clone()], 40);
            // (Sans la carte grossière : le filtre du décor partout dans la boîte des cartes, ce qu'elle épargne.)
            crate::decor3dgpu::ESSAI_SANS_GROSSIERE.with(|e| e.set(true));
            let t_sans_g = temps_de_peinture(&b, &[avec.clone()], 40);
            crate::decor3dgpu::ESSAI_SANS_GROSSIERE.with(|e| e.set(false));
            println!("  sans la carte grossière : {t_sans_g:.2} ms (+{:.2})", t_sans_g - t_sans);
            println!(
                "{} (passage {passage}) : image d'écran sans ombres de cartes {t_sans:.2} ms ; avec {t_avec:.2} ms (+{:.2}) ; seulement portées {t_portees:.2} ms (+{:.2}) ; seulement reçues {t_recues:.2} ms (+{:.2}) ; cartes qui bougent à chaque image {t_bouge:.2} ms (+{:.2})",
                q.nom(),
                t_avec - t_sans,
                t_portees - t_sans,
                t_recues - t_sans,
                t_bouge - t_sans
            );
        }
    }
    fin();
}
