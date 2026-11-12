//! Essais de la fusion de `main` (partie 144, les vues multiples de l'Espace 3D) dans la branche `trajectoire-3d`
//! (parties 146 à 150 : dessin dans la vue, profondeur libre, Trajectoire dans l'espace, panneau Espace 3D). Ils visent
//! les points où les deux côtés se rencontrent (`docs/INTERFACE.md`, partie 151) : le chemin d'affichage des vues
//! multiples n'envoie jamais un morceau sans triangle (partie 147) ; ses cartes se lisent avec la profondeur libre
//! (partie 148) ; le quadrillage est à la hauteur du sol ; l'outil Trajectoire trace aussi dans une des vues.

use eframe::egui::{self, epaint::Primitive, pos2, Event, Modifiers, PointerButton, Pos2};

use crate::app::Tool;
use crate::document::{Document, Layer, LayerKind};
use crate::espace3dmath::{Orbite, V3};
use crate::espace3dview::NEAR;
use crate::espace3dvues::{Disposition, PointDeVue};
use crate::perf_tests::{banc_taille, Banc};
use crate::trajectoire::Cles;

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn unit(a: V3) -> V3 {
    scale(a, 1.0 / dot(a, a).sqrt().max(1e-9))
}

/// Un projet d'essai : cadre 320 × 180, `n` calques de 24 images avec un peu d'encre.
fn doc_cartes(n: usize) -> Document {
    let mut doc = Document::with_frame(320, 180, 24);
    doc.panels[0].layers = Vec::new();
    doc.panels[0].length = 24;
    for k in 0..n {
        let mut layer = Layer::new(format!("Carte {k}"), LayerKind::Bitmap);
        let cel = doc.new_cel(LayerKind::Bitmap);
        let r = &mut doc.cels.get_mut(&cel).expect("dessin").raster;
        for y in 40..80 {
            for x in (30 + k * 7)..(120 + k * 7) {
                let idx = r.tile_index_of(x, y);
                r.tile_mut(idx)[(y % crate::raster::TILE) * crate::raster::TILE + x % crate::raster::TILE] = egui::Color32::from_rgb(200, 40, 40);
            }
        }
        layer.frames = vec![Some(cel); 24];
        doc.panels[0].layers.push(layer);
    }
    doc
}

/// Le banc, la vue Espace 3D ouverte en `disposition`, le calque 0 courant ; `espace` : le plan est un plan en Espace 3D.
fn ouvert(doc: Document, espace: bool, disposition: Disposition) -> Banc {
    let mut b = banc_taille(doc, false, 1280, 720);
    b.chauffer(2);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.app.run_action("espace3d.basculer");
    if espace {
        b.app.run_action("espace3d.plan_en_espace");
        assert!(b.app.doc.plan_en_espace(0), "{}", b.app.status);
    }
    b.app.run_action(disposition.action());
    b.chauffer(4);
    assert!(b.app.espace3d.on && !b.app.camera_view && b.app.e3_vues_multiples(), "les vues multiples sont montrées");
    b
}

/// Ce que la dernière image enverrait à la carte graphique et qui l'arrêterait : les morceaux qui ont des sommets et
/// aucun triangle.
fn morceaux_sans_triangle(b: &Banc) -> usize {
    b.ctx.tessellate(b.formes.clone(), 1.0).iter().filter(|p| matches!(&p.primitive, Primitive::Mesh(m) if m.indices.is_empty() && !m.vertices.is_empty())).count()
}

fn orbite_en(eye: V3, vers: V3) -> Orbite {
    let dir = scale(vers, -1.0);
    Orbite { target: add(eye, vers), azimuth: dir[0].atan2(dir[2]).to_degrees(), elevation: dir[1].clamp(-1.0, 1.0).asin().to_degrees(), distance: 1.0, focal: 35.0 }
}

#[test]
fn quatre_vues_qui_longent_une_carte_n_envoient_aucun_morceau_sans_triangle() {
    let mut b = ouvert(doc_cartes(1), false, Disposition::Quatre);
    b.garder_formes = true;
    let world = b.app.e3_world();
    let xf = world.carte(b.app.layer(), 0);
    let c = world.card_corners(&xf);
    let (u, v) = (sub(c[1], c[0]), sub(c[3], c[0]));
    let (largeur, vers) = (dot(u, u).sqrt(), unit(u));
    let normale = unit(cross(u, v));
    let centre = scale(add(add(c[0], c[1]), add(c[2], c[3])), 0.25);
    // Chacune des quatre vues longe la carte comme un mur, l'œil presque au bout (la situation du plantage de la partie
    // 147), des deux côtés et vers chaque bout.
    for (cote, sens) in [(0.02_f32, 1.0_f32), (-0.02, 1.0), (0.02, -1.0), (-0.02, -1.0)] {
        let vers = scale(vers, sens);
        let eye = add(add(centre, scale(vers, 0.47 * largeur - NEAR)), scale(normale, cote));
        for i in Disposition::Quatre.cadres() {
            let cadre = &mut b.app.espace3d.vues.cadres[*i];
            (cadre.point, cadre.orbite) = (PointDeVue::Perspective, orbite_en(eye, vers));
        }
        b.image(Vec::new(), 1.0 / 60.0);
        assert_eq!(morceaux_sans_triangle(&b), 0, "côté {cote}, sens {sens} : un morceau sans triangle part à la carte graphique");
    }
}

#[test]
fn les_vues_multiples_lisent_une_carte_derriere_la_camera_avec_sa_profondeur_libre_et_le_sol_a_sa_hauteur() {
    let mut b = ouvert(doc_cartes(1), true, Disposition::Quatre);
    // La carte passe derrière la caméra d'origine du plan (partie 148).
    b.app.layers_mut()[0].set_depth_selon(0, -3.0, true);
    b.chauffer(2);
    let world = b.app.e3_world();
    let xf = world.carte(&b.app.layers()[0], 0);
    assert!(b.app.layers()[0].depth_libre_at(0) < -2.9, "la carte est derrière la caméra");
    // Les bornes des vues (cadrer chaque vue) : la carte là où elle est, pas ramenée à la borne d'un plan ordinaire.
    let (lo, hi) = b.app.e3_bornes();
    for p in world.card_corners(&xf) {
        assert!((0..3).all(|k| p[k] >= lo[k] - 1e-3 && p[k] <= hi[k] + 1e-3), "le coin {p:?} de la carte est hors des bornes {lo:?} {hi:?}");
    }
    // Le quadrillage des vues (et de la vue seule) : à la hauteur du sol des surfaces.
    assert!((b.app.e3_hauteur_sol() - world.sol_y()).abs() < 1e-6, "{} contre {}", b.app.e3_hauteur_sol(), world.sol_y());
    assert!(world.sol_y().abs() > 1e-3, "le sol d'un plan en Espace 3D n'est pas à zéro : l'essai a un sens");
    let lignes = b.app.e3_quadrillage([0.0; 3], 8.0, None);
    assert!(!lignes.is_empty() && lignes.iter().all(|l| (l.a[1] - world.sol_y()).abs() < 1e-5));
}

#[test]
fn la_trajectoire_se_trace_au_sol_dans_une_des_deux_vues() {
    let mut b = ouvert(doc_cartes(1), true, Disposition::CoteACote);
    b.app.traj.cles_propose = Cles::Interpolation;
    b.app.run_action("outil.trajectoire");
    b.chauffer(3);
    assert!(b.app.tool == Tool::Trajectoire && b.app.e3_trace(), "l'outil a le pointeur dans les vues");
    // La vue de gauche (perspective) : un trait au sol, sous la carte.
    let (i, r) = *b.app.espace3d.vues.rects.first().expect("deux vues");
    assert_eq!(b.app.espace3d.vues.cadres[i].point, PointDeVue::Perspective);
    let view = b.app.espace3d.vues.cadres[i].vue(r.width()).expect("une perspective");
    let world = b.app.e3_world();
    let pied = world.carte(b.app.layer(), 0).pos;
    let au_sol = |dx: f32| -> Pos2 {
        let q = view.project([pied[0] + dx, world.sol_y(), pied[2] + 0.5], [r.width() / 2.0, r.height() / 2.0]).expect("le sol se voit");
        let p = pos2(r.left() + q[0], r.top() + q[1]);
        assert!(r.shrink(4.0).contains(p), "{p:?} hors de la vue {r:?}");
        p
    };
    let (a, z) = (au_sol(-0.6), au_sol(0.6));
    let m = Modifiers::NONE;
    b.image(vec![Event::PointerMoved(a)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: a, button: PointerButton::Primary, pressed: true, modifiers: m }], 1.0 / 60.0);
    for k in 1..=18 {
        b.image(vec![Event::PointerMoved(a + (z - a) * (k as f32 / 18.0))], 1.0 / 60.0);
    }
    b.image(vec![Event::PointerButton { pos: z, button: PointerButton::Primary, pressed: false, modifiers: m }], 1.0 / 60.0);
    b.chauffer(2);
    let t = b.app.layers()[0].trajectoire.clone().unwrap_or_else(|| panic!("la carte suit un chemin : {}", b.app.status));
    let s = t.support.expect("tracé dans l'espace");
    assert!(crate::espace3dmath::dot(s.normale(), [0.0, 1.0, 0.0]).abs() > 0.99, "posé sur le sol");
    assert!(!b.app.layers()[0].pos_keys.is_empty(), "la carte a reçu ses clés");
    // Le regard retenu pour le geste est bien celui de cette vue (pas celui de la dernière vue peinte).
    let regard = b.app.e3d.regard.expect("un regard");
    assert_eq!(regard.rect, r, "le regard du geste est celui de la vue où il a eu lieu");
}
