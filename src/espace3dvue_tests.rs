//! Essais de la tenue de la vue Espace 3D quand on s'y promène (`docs/INTERFACE.md`, partie 147). Anthony, 2026-11-12 :
//! « il y a beaucoup de crash du logiciel quand on se balade dans l'espace 3D ».
//!
//! Le plantage (rapport du 12 novembre, 0 h 27 : « Buffer slices can not be empty », dans `egui_wgpu::Renderer::render`) :
//! une carte que l'œil coupe au point qu'il n'en reste devant lui pas un seul carreau entier donnait un maillage avec des
//! sommets et **aucun triangle**. egui ne jette que les maillages sans sommet ni triangle ; celui-là partait à la carte
//! graphique, et wgpu refuse une tranche vide. Ces essais regardent ce que la vue envoie : jamais un tel morceau.

use eframe::egui::epaint::Primitive;
use eframe::egui::{pos2, vec2, Event, Modifiers, PointerButton, Pos2, Vec2};

use crate::document::{Document, Layer, LayerKind};
use crate::espace3dgizmo::{Mode as GizmoMode, CAMERA_ID};
use crate::espace3dmath::{Orbite, V3};
use crate::espace3dview::NEAR;
use crate::perf_tests::{banc_taille, se_reposer, Banc, Mesure};

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

/// Un projet d'essai : cadre `w` × `h` (la feuille fait le double), `n` calques de 12 images avec un peu d'encre.
fn doc_cartes(n: usize, w: usize, h: usize) -> Document {
    let mut doc = Document::with_frame(w, h, 24);
    doc.panels[0].layers = Vec::new();
    for k in 0..n {
        let mut layer = Layer::new(format!("Carte {k}"), LayerKind::Bitmap);
        let cel = doc.new_cel(LayerKind::Bitmap);
        let r = &mut doc.cels.get_mut(&cel).expect("dessin").raster;
        for y in 40..80 {
            for x in (30 + k * 7)..(120 + k * 7) {
                let idx = r.tile_index_of(x, y);
                r.tile_mut(idx)[(y % crate::raster::TILE) * crate::raster::TILE + x % crate::raster::TILE] = eframe::egui::Color32::from_rgb(200, 40, 40);
            }
        }
        layer.frames = vec![Some(cel); 12];
        doc.panels[0].layers.push(layer);
    }
    doc
}

/// Le banc, la vue Espace 3D ouverte, le calque 0 courant, les formes de chaque image gardées.
fn ouvert(doc: Document) -> Banc {
    let mut b = banc_taille(doc, false, 1280, 720);
    b.chauffer(2);
    b.app.cur_layer = 0;
    b.app.run_action("espace3d.basculer");
    b.chauffer(4);
    assert!(b.app.espace3d.on && !b.app.camera_view, "la vue Espace 3D est ouverte");
    b.garder_formes = true;
    b
}

/// Ce que la dernière image enverrait à la carte graphique et qui l'arrêterait : les morceaux qui ont des sommets et
/// aucun triangle (egui ne retire que ceux qui n'ont ni l'un ni l'autre).
fn morceaux_sans_triangle(b: &Banc) -> usize {
    b.ctx.tessellate(b.formes.clone(), 1.0).iter().filter(|p| matches!(&p.primitive, Primitive::Mesh(m) if m.indices.is_empty() && !m.vertices.is_empty())).count()
}

/// La vue d'orbite dont l'œil est en `eye` et qui regarde vers `vers` (unitaire).
fn orbite_en(eye: V3, vers: V3) -> Orbite {
    let dir = scale(vers, -1.0);
    Orbite { target: add(eye, vers), azimuth: dir[0].atan2(dir[2]).to_degrees(), elevation: dir[1].clamp(-1.0, 1.0).asin().to_degrees(), distance: 1.0, focal: 35.0 }
}

#[test]
fn a_card_the_eye_cuts_down_to_less_than_one_square_sends_no_piece_without_triangles() {
    let mut b = ouvert(doc_cartes(1, 320, 180));
    let world = b.app.e3_world();
    let xf = world.card_xf(&b.app.layer().pose(0), b.app.layer().depth_at(0));
    let c = world.card_corners(&xf);
    let (u, v) = (sub(c[1], c[0]), sub(c[3], c[0]));
    let (largeur, vers) = (dot(u, u).sqrt(), unit(u));
    let normale = unit(cross(u, v));
    let centre = scale(add(add(c[0], c[1]), add(c[2], c[3])), 0.25);
    let mut mordu = 0;
    // On longe la carte comme un mur, l'œil presque au bout : seule la dernière rangée de sommets reste devant lui. Des
    // deux côtés du mur, et vers chaque bout.
    for (cote, sens) in [(0.02_f32, 1.0_f32), (-0.02, 1.0), (0.02, -1.0), (-0.02, -1.0)] {
        let vers = scale(vers, sens);
        let eye = add(add(centre, scale(vers, 0.47 * largeur - NEAR)), scale(normale, cote));
        let orbite = orbite_en(eye, vers);
        let view = orbite.view(b.app.canvas_rect.width());
        assert!(dot(view.forward, vers) > 0.999 && dot(sub(view.eye, eye), sub(view.eye, eye)) < 1e-6, "la vue d'essai regarde où il faut : {view:?}");
        // La situation est bien celle du plantage : des sommets devant l'œil, pas un carreau entier.
        let devant = |i: usize, j: usize| dot(sub(add(add(c[0], scale(u, i as f32 / 16.0)), scale(v, j as f32 / 16.0)), eye), view.forward) >= NEAR;
        let sommets = (0..=16).flat_map(|j| (0..=16).map(move |i| (i, j))).filter(|&(i, j)| devant(i, j)).count();
        let carreaux = (0..16).flat_map(|j| (0..16).map(move |i| (i, j))).filter(|&(i, j)| devant(i, j) && devant(i + 1, j) && devant(i, j + 1) && devant(i + 1, j + 1)).count();
        assert!(sommets > 0 && carreaux == 0, "{sommets} sommets devant l'œil, {carreaux} carreaux entiers");
        mordu += 1;
        b.app.espace3d.cam = Some(orbite);
        b.image(Vec::new(), 1.0 / 60.0);
        assert_eq!(morceaux_sans_triangle(&b), 0, "côté {cote}, sens {sens} : un morceau sans triangle part à la carte graphique, elle s'arrête dessus");
    }
    assert_eq!(mordu, 4);
}

#[test]
fn walking_through_the_surfaces_of_a_plan_en_espace_never_sends_a_piece_without_triangles_and_stays_fast() {
    let mut b = ouvert(doc_cartes(8, 1920, 1080));
    b.app.run_action("espace3d.plan_en_espace");
    for a in ["espace3d.surface_sol", "espace3d.surface_mur", "espace3d.surface_face"] {
        b.app.run_action(a);
    }
    b.chauffer(4);
    let world = b.app.e3_world();
    let sol = b.app.layers().iter().position(|l| l.name == "Surface au sol").expect("la surface au sol");
    let xf = world.card_xf(&b.app.layers()[sol].pose(0), b.app.layers()[sol].depth_at(0));
    let c = world.card_corners(&xf);
    let centre = scale(add(add(c[0], c[1]), add(c[2], c[3])), 0.25);
    let (demi_u, demi_v) = (scale(sub(c[1], c[0]), 0.5), scale(sub(c[3], c[0]), 0.5));
    // On traverse le sol d'un coin à l'autre et au-delà, à hauteur d'homme, en tournant la tête : le sol passe sans
    // cesse en partie derrière l'œil, les murs aussi.
    const N: usize = 360;
    let mut pire: Option<(usize, usize)> = None;
    let mut temps = Vec::with_capacity(N);
    for k in 0..N {
        let t = k as f32 / (N - 1) as f32;
        let s = -1.25 + 2.5 * t;
        let pied = add(centre, add(scale(demi_u, s), scale(demi_v, s * 0.8)));
        let eye = [pied[0], world.sol_y() + 0.35 + 0.25 * (t * 9.0).sin(), pied[2]];
        let (az, el) = ((40.0 + 700.0 * t).to_radians(), (12.0 * (t * 23.0).sin()).to_radians());
        let vers = [el.cos() * az.sin(), -el.sin(), el.cos() * az.cos()];
        b.app.espace3d.cam = Some(orbite_en(eye, vers));
        let m = b.image(Vec::new(), 1.0 / 60.0);
        temps.push(m.construction + m.decoupage);
        let vides = morceaux_sans_triangle(&b);
        if vides > 0 && pire.is_none() {
            pire = Some((k, vides));
        }
    }
    assert!(pire.is_none(), "image {:?} de la promenade : des morceaux sans triangle partent à la carte graphique, elle s'arrête dessus", pire);
    temps.sort_by(|a, b| a.total_cmp(b));
    let (mediane, max) = (temps[N / 2], temps[N - 1]);
    println!("PERF vue Espace 3D : se promener dans un plan en Espace 3D (11 cartes, feuille 3840 × 2160, écran 1280 × 720, sans carte graphique) : médiane {mediane:.2} ms, max {max:.2} ms par image");
    assert!(mediane < 16.0, "{mediane:.2} ms par image : la promenade doit tenir 60 images par seconde");
}

/// Médiane et maximum (ms) de ce que des images ont coûté, et la part de chaque étape dans la médiane.
fn resume(titre: &str, mesures: &[Mesure], b: &Banc) -> f64 {
    let mut t: Vec<f64> = mesures.iter().map(|m| m.total()).collect();
    t.sort_by(|a, b| a.total_cmp(b));
    let n = t.len().max(1);
    let moy = |f: &dyn Fn(&Mesure) -> f64| mesures.iter().map(f).sum::<f64>() / n as f64;
    println!(
        "PERF vue Espace 3D, {titre} (carte graphique {}) : médiane {:.2} ms, max {:.2} ms par image ; en moyenne interface {:.2}, découpage {:.2}, carte graphique {:.2} ; textures refaites {} ({} Ko)",
        b.app.gpu.is_some(),
        t[n / 2],
        t[n - 1],
        moy(&|m| m.construction),
        moy(&|m| m.decoupage),
        moy(&|m| m.carte),
        mesures.iter().map(|m| m.textures).sum::<usize>(),
        mesures.iter().map(|m| m.octets).sum::<usize>() / 1024,
    );
    t[n / 2]
}

/// Un glisser du bouton `bouton` dans la vue : on appuie en `de`, puis `n` images où le pointeur avance de `pas` (aller
/// et retour, pour rester dans la vue). Ce que chaque image du geste a coûté.
fn glisser(b: &mut Banc, bouton: PointerButton, de: Pos2, pas: Vec2, n: usize) -> Vec<Mesure> {
    let m = Modifiers::default();
    let dt = 1.0 / 60.0;
    b.image(vec![Event::PointerMoved(de)], dt);
    b.image(vec![Event::PointerButton { pos: de, button: bouton, pressed: true, modifiers: m }], dt);
    let mut p = de;
    let mut out = Vec::with_capacity(n);
    for k in 0..n {
        p += if (k / 40) % 2 == 0 { pas } else { -pas };
        out.push(b.image(vec![Event::PointerMoved(p)], dt));
    }
    b.image(vec![Event::PointerButton { pos: p, button: bouton, pressed: false, modifiers: m }], dt);
    out
}

/// Les gestes d'une promenade, un par un, dans un plan en Espace 3D à la taille d'un vrai projet (feuille 3840 × 2160,
/// huit calques dessinés et trois surfaces) : tourner et glisser la vue, avancer, puis la même chose en regardant par
/// la caméra (c'est alors la caméra du plan qui bouge : l'image du plan change à chaque image d'écran), et tirer la
/// caméra par sa poignée. Avec la carte graphique quand il y en a une.
#[test]
fn every_gesture_of_a_walk_in_a_plan_en_espace_stays_within_the_frame_budget() {
    let mut b = banc_taille(doc_cartes(8, 1920, 1080), true, 1280, 720);
    b.chauffer(2);
    b.app.cur_layer = 0;
    b.app.run_action("espace3d.basculer");
    b.app.run_action("espace3d.plan_en_espace");
    for a in ["espace3d.surface_sol", "espace3d.surface_mur", "espace3d.surface_face"] {
        b.app.run_action(a);
    }
    b.app.run_action("espace3d.cadrer");
    b.chauffer(8);
    let r = b.app.canvas_rect;
    let c = r.center();
    let dt = 1.0 / 60.0;
    let mut pire: f64 = 0.0;

    let repos: Vec<Mesure> = (0..60).map(|k| b.image(vec![Event::PointerMoved(c + vec2(k as f32, 0.0))], dt)).collect();
    pire = pire.max(resume("le pointeur passe sur la vue", &repos, &b));
    let m = glisser(&mut b, PointerButton::Secondary, c, vec2(3.0, 1.0), 120);
    pire = pire.max(resume("tourner la vue (clic droit)", &m, &b));
    let m = glisser(&mut b, PointerButton::Middle, c, vec2(2.0, 1.0), 120);
    pire = pire.max(resume("glisser la vue (bouton du milieu)", &m, &b));
    let m: Vec<Mesure> = (0..120)
        .map(|k| {
            let ev = b.molette(vec2(0.0, if (k / 30) % 2 == 0 { 30.0 } else { -30.0 }), Modifiers::default());
            b.image(ev, dt)
        })
        .collect();
    pire = pire.max(resume("avancer (molette)", &m, &b));

    // Tirer la caméra du plan par sa poignée : le plan change à chaque image.
    b.app.espace3d.sel = vec![CAMERA_ID];
    b.app.espace3d.mode = GizmoMode::Move;
    b.chauffer(2);
    let view = b.app.espace3d.cam.expect("la vue d'orbite").view(r.width());
    let centre = [r.width() / 2.0, r.height() / 2.0];
    let gizmo = b.app.e3_gizmo(&view).expect("le manipulateur de la caméra");
    let bout = view.project(gizmo.axis_tip(0), centre).expect("la poignée est devant l'œil");
    let avant = b.app.current_camera();
    let m = glisser(&mut b, PointerButton::Primary, pos2(r.left() + bout[0], r.top() + bout[1]), vec2(2.0, 0.0), 120);
    assert_ne!(b.app.current_camera(), avant, "la poignée a bien déplacé la caméra du plan : {}", b.app.status);
    pire = pire.max(resume("tirer la caméra du plan par sa poignée", &m, &b));

    // Regarder par la caméra : la promenade déplace la caméra du plan elle-même.
    b.app.run_action("espace3d.vue_camera");
    b.chauffer(4);
    assert!(b.app.espace3d.through_camera, "on regarde par la caméra");
    let avant = b.app.current_camera();
    let m = glisser(&mut b, PointerButton::Secondary, c, vec2(2.0, 1.0), 120);
    assert_ne!(b.app.current_camera(), avant, "le clic droit a bien tourné la caméra du plan");
    pire = pire.max(resume("par la caméra : tourner la caméra (clic droit)", &m, &b));
    let m = glisser(&mut b, PointerButton::Middle, c, vec2(2.0, 1.0), 120);
    pire = pire.max(resume("par la caméra : déplacer la caméra (bouton du milieu)", &m, &b));
    let m: Vec<Mesure> = (0..120)
        .map(|k| {
            let ev = b.molette(vec2(0.0, if (k / 30) % 2 == 0 { 30.0 } else { -30.0 }), Modifiers::default());
            b.image(ev, dt)
        })
        .collect();
    pire = pire.max(resume("par la caméra : avancer (molette)", &m, &b));

    let images = se_reposer(&mut b, 600);
    println!("PERF vue Espace 3D : repos retrouvé {images} images après la promenade");
    assert!(pire < 16.0, "{pire:.2} ms de médiane pour le geste le plus lent : la promenade doit tenir 60 images par seconde");
}
