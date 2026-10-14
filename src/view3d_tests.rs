//! Tests du repère 3D (2d) : lecture glTF, soudure et arêtes utiles, format de Pelure, caméra
//! en orbite, fil de fer et arêtes cachées, enregistrement dans le projet.

use eframe::egui::{pos2, Pos2};
use serde_json::{json, Value};

use crate::document::{Document, Ref3d};
use crate::model3d::{self, Mesh3d, Model3d};
use crate::project;
use crate::view3d::{wireframe, Cam3d, MAX_ELEVATION};

/// Cube [-1, 1]³ comme l'exporte un logiciel 3D : 4 sommets par face (24), faces vers l'extérieur.
fn cube_faces() -> (Vec<[f32; 3]>, Vec<u16>) {
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for a in 0..3 {
        for s in [1.0f32, -1.0] {
            let e = |k: usize| {
                let mut v = [0.0; 3];
                v[k % 3] = 1.0;
                v
            };
            let (n, u, v) = (e(a), e(a + 1), e(a + 2));
            let corner = |cu: f32, cv: f32| [0, 1, 2].map(|k| s * n[k] + cu * u[k] + cv * v[k]);
            let mut quad = vec![corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)];
            if s < 0.0 {
                quad.reverse();
            }
            let base = positions.len() as u16;
            positions.extend(quad);
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    (positions, indices)
}

fn cube_mesh() -> Mesh3d {
    let (positions, indices) = cube_faces();
    Mesh3d {
        positions,
        triangles: indices.chunks(3).map(|c| [c[0] as u32, c[1] as u32, c[2] as u32]).collect(),
    }
}

/// Description glTF d'un maillage (positions, indices 16 bits) et son tampon binaire.
fn gltf(nodes: Value, positions: &[[f32; 3]], indices: &[u16], mode: u64) -> (Value, Vec<u8>) {
    let mut bin: Vec<u8> = positions.iter().flatten().flat_map(|v| v.to_le_bytes()).collect();
    let pl = bin.len();
    bin.extend(indices.iter().flat_map(|i| i.to_le_bytes()));
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let desc = json!({
        "asset": {"version": "2.0", "generator": "test"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": nodes,
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "indices": 1, "mode": mode}]}],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": positions.len(), "type": "VEC3"},
            {"bufferView": 1, "componentType": 5123, "count": indices.len(), "type": "SCALAR"}
        ],
        "bufferViews": [
            {"buffer": 0, "byteOffset": 0, "byteLength": pl},
            {"buffer": 0, "byteOffset": pl, "byteLength": indices.len() * 2}
        ],
        "buffers": [{"byteLength": bin.len()}]
    });
    (desc, bin)
}

/// Fichier `.glb` : en-tête, morceau JSON (complété d'espaces), morceau binaire.
fn glb(desc: &Value, bin: &[u8]) -> Vec<u8> {
    let mut text = serde_json::to_vec(desc).unwrap();
    while text.len() % 4 != 0 {
        text.push(b' ');
    }
    let total = 12 + 8 + text.len() + 8 + bin.len();
    let mut out = b"glTF".to_vec();
    out.extend(2u32.to_le_bytes());
    out.extend((total as u32).to_le_bytes());
    out.extend((text.len() as u32).to_le_bytes());
    out.extend(b"JSON");
    out.extend(&text);
    out.extend((bin.len() as u32).to_le_bytes());
    out.extend(b"BIN\0");
    out.extend(bin);
    out
}

fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            if k <= c.len() {
                out.push(ABC[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn close(a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() < 1e-4)
}

#[test]
fn glb_cube_is_read_with_its_node_transforms() {
    let (positions, indices) = cube_faces();
    // Parent déplacé de 5 en y, enfant doublé et déplacé de 10 en x.
    let nodes = json!([
        {"translation": [0.0, 5.0, 0.0], "children": [1]},
        {"translation": [10.0, 0.0, 0.0], "scale": [2.0, 2.0, 2.0], "mesh": 0}
    ]);
    let (desc, bin) = gltf(nodes, &positions, &indices, 4);
    let mesh = model3d::parse_gltf(&glb(&desc, &bin), None).unwrap();
    assert_eq!((mesh.positions.len(), mesh.triangles.len()), (24, 12));
    let (lo, hi) = model3d::bounds_of(&mesh.positions);
    assert!(close(lo, [8.0, 3.0, -2.0]) && close(hi, [12.0, 7.0, 2.0]), "{lo:?} {hi:?}");
}

#[test]
fn rotation_and_matrix_nodes() {
    let tri = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 0.0]];
    // Quart de tour autour de y : x devient -z.
    let h = std::f32::consts::FRAC_1_SQRT_2;
    let (desc, bin) = gltf(json!([{"rotation": [0.0, h, 0.0, h], "mesh": 0}]), &tri, &[0, 1, 2], 4);
    let mesh = model3d::parse_gltf(&glb(&desc, &bin), None).unwrap();
    assert!(close(mesh.positions[0], [0.0, 0.0, -1.0]), "{:?}", mesh.positions[0]);
    // Matrice en colonnes : translation dans les 3 premières valeurs de la dernière colonne.
    let m = json!([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 3, 4, 5, 1]);
    let (desc, bin) = gltf(json!([{"matrix": m, "mesh": 0}]), &tri, &[0, 1, 2], 4);
    let mesh = model3d::parse_gltf(&glb(&desc, &bin), None).unwrap();
    assert!(close(mesh.positions[2], [3.0, 4.0, 5.0]));
}

#[test]
fn strips_and_fans_become_triangles() {
    let quad = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0]];
    let (desc, bin) = gltf(json!([{"mesh": 0}]), &quad, &[0, 1, 2, 3], 5);
    let strip = model3d::parse_gltf(&glb(&desc, &bin), None).unwrap();
    assert_eq!(strip.triangles, vec![[0, 1, 2], [2, 1, 3]]);
    let (desc, bin) = gltf(json!([{"mesh": 0}]), &quad, &[0, 1, 3, 2], 6);
    let fan = model3d::parse_gltf(&glb(&desc, &bin), None).unwrap();
    assert_eq!(fan.triangles, vec![[0, 1, 3], [0, 3, 2]]);
}

#[test]
fn text_gltf_with_embedded_buffer() {
    let (positions, indices) = cube_faces();
    let (mut desc, bin) = gltf(json!([{"mesh": 0}]), &positions, &indices, 4);
    desc["buffers"][0]["uri"] = json!(format!("data:application/octet-stream;base64,{}", base64(&bin)));
    let text = serde_json::to_vec_pretty(&desc).unwrap();
    let mesh = model3d::parse_gltf(&text, None).unwrap();
    assert_eq!(mesh.triangles.len(), 12);
    assert_eq!(model3d::base64_decode(&base64(b"Pelure!")).unwrap(), b"Pelure!");
}

#[test]
fn unreadable_files_are_refused_with_a_reason() {
    let (positions, indices) = cube_faces();
    let (desc, bin) = gltf(json!([{"mesh": 0}]), &positions, &indices, 4);
    let mut compressed = desc.clone();
    compressed["extensionsRequired"] = json!(["KHR_draco_mesh_compression"]);
    let e = model3d::parse_gltf(&glb(&compressed, &bin), None).unwrap_err();
    assert!(e.contains("KHR_draco_mesh_compression"), "{e}");
    let mut old = desc.clone();
    old["asset"]["version"] = json!("1.0");
    assert!(model3d::parse_gltf(&glb(&old, &bin), None).unwrap_err().contains("version"));
    assert!(model3d::parse_gltf(b"\x00\x01pas un modele", None).is_err());
    // Tampon externe hors du dossier du fichier : refusé sans être lu.
    let mut outside = desc.clone();
    outside["buffers"][0]["uri"] = json!("..%2Fsecret.bin");
    let text = serde_json::to_vec(&outside).unwrap();
    let e = model3d::parse_gltf(&text, Some(&std::env::temp_dir())).unwrap_err();
    assert!(e.contains("hors du dossier"), "{e}");
    // Tampon trop court pour l'accesseur.
    let e = model3d::parse_gltf(&glb(&desc, &bin[..40]), None).unwrap_err();
    assert!(e.contains("hors de son tampon"), "{e}");
    // Que des points : aucun triangle.
    let (points, _) = gltf(json!([{"mesh": 0}]), &positions, &indices, 0);
    assert!(model3d::parse_gltf(&glb(&points, &bin), None).unwrap_err().contains("aucun triangle"));
}

#[test]
fn welded_cube_has_twelve_edges_and_no_diagonal() {
    let model = Model3d::new(1, "cube".into(), cube_mesh());
    assert_eq!(model.mesh.positions.len(), 8);
    assert_eq!(model.mesh.triangles.len(), 12);
    assert_eq!(model.edges.len(), 12);
    for e in &model.edges {
        let (a, b) = (model.mesh.positions[e[0] as usize], model.mesh.positions[e[1] as usize]);
        let differing = (0..3).filter(|&k| a[k] != b[k]).count();
        assert_eq!(differing, 1, "diagonale {a:?} {b:?}");
    }
    // Un triangle retourné (normale à l'envers) ne fait pas apparaître la diagonale de sa face.
    let mut flipped = cube_mesh();
    flipped.triangles[0].swap(1, 2);
    assert_eq!(Model3d::new(2, "cube".into(), flipped).edges.len(), 12);
    // Un plan seul : ses 4 bords, pas la diagonale.
    let plane = Mesh3d {
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
    };
    assert_eq!(Model3d::new(3, "plan".into(), plane).edges.len(), 4);
}

#[test]
fn pelure_format_round_trip_and_damage() {
    let mesh = model3d::weld(&cube_mesh());
    let bytes = model3d::to_bytes(&mesh);
    assert_eq!(model3d::from_bytes(&bytes).unwrap(), mesh);
    assert!(model3d::from_bytes(&bytes[..bytes.len() - 1]).is_err(), "tronqué");
    assert!(model3d::from_bytes(b"PELURE3X").is_err());
    let mut newer = bytes.clone();
    newer[8] = 2;
    assert!(model3d::from_bytes(&newer).unwrap_err().contains("récente"));
    let mut broken = bytes.clone();
    let last = broken.len() - 4;
    broken[last..].copy_from_slice(&99u32.to_le_bytes());
    assert!(model3d::from_bytes(&broken).is_err(), "indice hors des sommets");
}

const FRAME: [f32; 2] = [640.0, 360.0];

fn cube_bounds() -> ([f32; 3], [f32; 3]) {
    ([-1.0; 3], [1.0; 3])
}

#[test]
fn projection_follows_the_camera() {
    let cam = Cam3d { target: [0.0; 3], azimuth: 0.0, elevation: 0.0, distance: 10.0, focal: 36.0 };
    // L'œil est en +z et regarde vers -z : le point visé au centre, +x à droite, +y en haut.
    assert_eq!(cam.project([0.0; 3], FRAME), Some(pos2(320.0, 180.0)));
    let right = cam.project([1.0, 0.0, 0.0], FRAME).unwrap();
    let up = cam.project([0.0, 1.0, 0.0], FRAME).unwrap();
    // Focale 36 mm pour 36 mm de large : 640 px par unité de x/z, ici à 10 unités de l'œil.
    assert!((right.x - (320.0 + 640.0 / 10.0)).abs() < 1e-3, "{right:?}");
    assert!(up.y < 180.0);
    assert_eq!(cam.project([0.0, 0.0, 11.0], FRAME), None, "derrière l'œil");
}

#[test]
fn fit_frames_the_whole_model() {
    let cam = Cam3d::fit(cube_bounds(), FRAME);
    for i in 0..8 {
        let p = [i & 1, i >> 1 & 1, i >> 2 & 1].map(|b| if b == 1 { 1.0 } else { -1.0 });
        let q = cam.project(p, FRAME).unwrap();
        assert!(q.x > 0.0 && q.x < FRAME[0] && q.y > 0.0 && q.y < FRAME[1], "{p:?} → {q:?}");
    }
}

#[test]
fn orbit_pan_and_dolly() {
    let mut cam = Cam3d::fit(cube_bounds(), FRAME);
    cam.orbit(350.0, 200.0);
    assert!((cam.azimuth - 25.0).abs() < 1e-3, "{}", cam.azimuth);
    assert_eq!(cam.elevation, MAX_ELEVATION);
    // Glisser : le point visé suit la souris.
    let mut cam = Cam3d::fit(cube_bounds(), FRAME);
    let before = cam.target;
    cam.pan(40.0, -25.0, FRAME[0]);
    let q = cam.project(before, FRAME).unwrap();
    assert!((q.x - 360.0).abs() < 0.01 && (q.y - 155.0).abs() < 0.01, "{q:?}");
    let d = cam.distance;
    cam.dolly(0.5);
    assert_eq!(cam.distance, d * 0.5);
    cam.dolly(0.0);
    assert!(cam.distance > 0.0);
}

fn distance_to_segment(p: Pos2, s: [Pos2; 2]) -> f32 {
    let d = s[1] - s[0];
    let t = ((p - s[0]).dot(d) / d.length_sq().max(1e-9)).clamp(0.0, 1.0);
    p.distance(s[0] + d * t)
}

/// Vu de +x, +y, +z : le sommet (-1, -1, -1) est caché, et les 3 arêtes qui y mènent ; les 9
/// autres sont entières dans `seen`.
fn check_back_edges(model: &Model3d, cam: &Cam3d, frame: [f32; 2], seen: &[[Pos2; 2]]) {
    let hidden_corner = [-1.0f32; 3];
    let mut hidden = 0;
    for e in &model.edges {
        let (a, b) = (model.mesh.positions[e[0] as usize], model.mesh.positions[e[1] as usize]);
        let back = a == hidden_corner || b == hidden_corner;
        let (pa, pb) = (cam.project(a, frame).unwrap(), cam.project(b, frame).unwrap());
        for k in 1..10 {
            let q = pa + (pb - pa) * (k as f32 / 10.0);
            let near = seen.iter().map(|s| distance_to_segment(q, *s)).fold(f32::MAX, f32::min);
            if back {
                assert!(near > 1.0, "arête arrière visible {a:?}-{b:?} en {q:?}");
            } else {
                assert!(near < 1.0, "arête avant coupée {a:?}-{b:?} en {q:?}");
            }
        }
        hidden += back as usize;
    }
    assert_eq!(hidden, 3);
}

#[test]
fn wireframe_hides_the_back_edges() {
    let model = Model3d::new(1, "cube".into(), cube_mesh());
    let cam = Cam3d::fit(cube_bounds(), FRAME);
    let all = wireframe(&model, &cam, FRAME, false);
    assert_eq!(all.len(), 12);
    check_back_edges(&model, &cam, FRAME, &wireframe(&model, &cam, FRAME, true));
}

#[test]
fn back_edges_stay_hidden_off_frame() {
    // Petit cadre, cube vu de près : il déborde du cadre, sur la feuille autour.
    let model = Model3d::new(1, "cube".into(), cube_mesh());
    let frame = [160.0, 90.0];
    let mut cam = Cam3d::fit(cube_bounds(), frame);
    cam.dolly(0.75);
    let outside = all_points_outside(&model, &cam, frame);
    assert!(outside > 0, "le cube doit déborder du cadre");
    let area = eframe::egui::Rect::from_min_size(pos2(-160.0, -90.0), eframe::egui::vec2(480.0, 270.0));
    let seen = crate::view3d::wireframe_in(&model, &cam, frame, area, true);
    check_back_edges(&model, &cam, frame, &seen);
}

fn all_points_outside(model: &Model3d, cam: &Cam3d, frame: [f32; 2]) -> usize {
    model
        .mesh
        .positions
        .iter()
        .filter_map(|&p| cam.project(p, frame))
        .filter(|q| q.x < 0.0 || q.y < 0.0 || q.x > frame[0] || q.y > frame[1])
        .count()
}

#[test]
fn camera_inside_the_model_does_not_break() {
    let model = Model3d::new(1, "cube".into(), cube_mesh());
    let cam = Cam3d { target: [0.0; 3], azimuth: 10.0, elevation: 5.0, distance: 0.5, focal: 24.0 };
    for hidden in [false, true] {
        let segs = wireframe(&model, &cam, FRAME, hidden);
        assert!(!segs.is_empty());
        assert!(segs.iter().flatten().all(|p| p.x.is_finite() && p.y.is_finite()));
    }
}

#[test]
fn models_and_references_are_saved_with_the_project() {
    let dir = std::env::temp_dir().join(format!("pelure-test-repere3d-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("R.pelure");
    let mut doc = Document::with_frame(32, 24, 24);
    let id = doc.add_model("decor.glb".into(), cube_mesh());
    let unused = doc.add_model("rien.glb".into(), cube_mesh());
    let mut r = Ref3d::new(id, Cam3d::fit(cube_bounds(), FRAME));
    r.cam.azimuth = 120.0;
    r.hidden_lines = false;
    r.color = [200, 10, 10];
    r.opacity = 0.5;
    doc.panels[0].ref3d = Some(r.clone());
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let file = dir.join("modeles").join(format!("{id}.pelure3d"));
    assert!(file.exists());
    assert!(!dir.join("modeles").join(format!("{unused}.pelure3d")).exists(), "modèle inutilisé non écrit");

    let mut back = project::load(&manifest).unwrap().doc;
    assert_eq!(back.panels[0].ref3d, Some(r));
    let model = &back.models[&id];
    assert_eq!(model.name, "decor.glb");
    assert_eq!(model.mesh, doc.models[&id].mesh);
    assert_eq!(model.edges.len(), 12);
    // Les identifiants suivants ne réutilisent pas celui du modèle.
    assert!(back.add_model("autre".into(), cube_mesh()) > id);

    // Plus aucun panneau ne s'en sert : le fichier est effacé au prochain enregistrement.
    back.panels[0].ref3d = None;
    project::save(&back, &manifest, &mut Default::default()).unwrap();
    assert!(!file.exists());
    assert_eq!(project::load(&manifest).unwrap().doc.panels[0].ref3d, None);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn longest_edges_come_first_for_the_preview() {
    let plane = Mesh3d {
        positions: vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
    };
    let model = Model3d::new(1, "plan".into(), plane);
    let length = |e: &[u32; 2]| {
        let (a, b) = (model.mesh.positions[e[0] as usize], model.mesh.positions[e[1] as usize]);
        (a[0] - b[0]).abs() + (a[1] - b[1]).abs()
    };
    let lengths: Vec<f32> = model.edges.iter().map(length).collect();
    assert_eq!(lengths, vec![10.0, 10.0, 1.0, 1.0]);
    let cube = Model3d::new(2, "cube".into(), cube_mesh());
    let cam = Cam3d::fit(cube_bounds(), FRAME);
    assert_eq!(crate::view3d::preview(&cube, &cam, FRAME, 5).len(), 5);
    assert_eq!(crate::view3d::preview(&cube, &cam, FRAME, 100).len(), 12);
}

#[test]
fn lines_are_traced_smoothly_in_a_mask() {
    use crate::view3d::rasterize;
    // Trait horizontal au milieu de la ligne 5.
    let m = rasterize(&[[pos2(2.0, 5.5), pos2(18.0, 5.5)]], [20, 10], 1.0, 1.3);
    assert_eq!(m[5 * 20 + 10], 255);
    assert!(m[4 * 20 + 10] > 0 && m[4 * 20 + 10] < 128, "bord adouci : {}", m[4 * 20 + 10]);
    assert_eq!(m[3 * 20 + 10], 0);
    assert_eq!(m[5 * 20], 0, "le trait s'arrête à son bout");
    // Même trait en pixels du cadre, image deux fois plus grande.
    let m2 = rasterize(&[[pos2(1.0, 2.75), pos2(9.0, 2.75)]], [20, 10], 2.0, 1.3);
    assert_eq!(m2[5 * 20 + 10], 255);
    // Trait vertical sur une grande hauteur (partagée entre plusieurs fils) : sans trou.
    let tall = rasterize(&[[pos2(10.5, -5.0), pos2(10.5, 400.0)]], [30, 300], 1.0, 1.3);
    assert!((0..300).all(|y| tall[y * 30 + 10] == 255));
    // Trait très long qui sort de l'image : coupé, la diagonale est tracée.
    let diag = rasterize(&[[pos2(-1000.0, -1000.0), pos2(1000.0, 1000.0)]], [100, 100], 1.0, 1.3);
    assert!((0..100).all(|i| diag[i * 100 + i] == 255));
    assert_eq!(diag[10 * 100 + 90], 0);
}

#[test]
fn reference_is_baked_into_a_bottom_layer() {
    let mut doc = Document::with_frame(64, 48, 24);
    assert_eq!(crate::ref3dtool::bake(&mut doc, 0, 1.5), None, "pas de repère, pas de calque");
    let id = doc.add_model("cube".into(), cube_mesh());
    let mut r = Ref3d::new(id, Cam3d::fit(cube_bounds(), [64.0, 48.0]));
    r.color = [200, 0, 0];
    doc.panels[0].ref3d = Some(r);
    let before = doc.panels[0].layers.len();
    assert_eq!(crate::ref3dtool::bake(&mut doc, 0, 1.5), Some(0));
    let layers = &doc.panels[0].layers;
    assert_eq!(layers.len(), before + 1);
    assert_eq!(layers[0].name, "Repère 3D");
    assert_eq!(layers[0].kind, crate::document::LayerKind::Bitmap);
    assert_eq!(layers[0].frames.len(), doc.panels[0].frame_count());
    // Cube cadré : tout dans le cadre, de la couleur du repère.
    let (inside, outside) = painted(&doc, layers[0].frames[0].unwrap());
    assert!(inside > 50 && outside == 0, "{inside} dedans, {outside} dehors");
    // Vu de plus près, il déborde : le dessin continue sur la feuille, hors du cadre.
    doc.panels[0].ref3d.as_mut().unwrap().cam.dolly(0.6);
    crate::ref3dtool::bake(&mut doc, 0, 1.5).unwrap();
    let (inside, outside) = painted(&doc, doc.panels[0].layers[0].frames[0].unwrap());
    assert!(inside > 50 && outside > 20, "{inside} dedans, {outside} dehors");
}

/// Pixels peints d'un dessin bitmap, dans le cadre et hors du cadre (tous en rouge).
fn painted(doc: &Document, cel: crate::document::CelId) -> (usize, usize) {
    let raster = &doc.cels[&cel].raster;
    let frame = doc.frame_rect();
    let (mut inside, mut outside) = (0, 0);
    for y in 0..raster.height {
        for x in 0..raster.width {
            let c = raster.get(x, y);
            if c.a() > 0 {
                assert!(c.r() > 0 && c.g() == 0 && c.b() == 0, "{c:?}");
                if frame.contains(pos2(x as f32 + 0.5, y as f32 + 0.5)) {
                    inside += 1;
                } else {
                    outside += 1;
                }
            }
        }
    }
    (inside, outside)
}

/// Mesure sur un vrai décor : `PELURE_GLB=chemin.glb cargo test --release real_decor -- --ignored --nocapture`.
#[test]
#[ignore]
fn real_decor_timings() {
    let Ok(path) = std::env::var("PELURE_GLB") else { return };
    let t = std::time::Instant::now();
    let mesh = model3d::load_gltf(std::path::Path::new(&path)).unwrap();
    let read = t.elapsed();
    let (nv, nt) = (mesh.positions.len(), mesh.triangles.len());
    let t = std::time::Instant::now();
    let model = Model3d::new(1, "decor".into(), mesh);
    let build = t.elapsed();
    println!(
        "lecture {read:?} ({nv} sommets, {nt} triangles) ; soudure + arêtes {build:?} ({} sommets, {} arêtes)",
        model.mesh.positions.len(),
        model.edges.len()
    );
    let frame = [1920.0, 1080.0];
    let cam = Cam3d::fit(model.bounds, frame);
    for hidden in [false, true] {
        let t = std::time::Instant::now();
        let segs = wireframe(&model, &cam, frame, hidden);
        let wire = t.elapsed();
        let t = std::time::Instant::now();
        let mask = crate::view3d::rasterize(&segs, [1920, 1080], 1.0, 1.3);
        println!(
            "fil de fer (cachées {hidden}) : {wire:?}, {} segments ; image {:?} ({} pixels tracés)",
            segs.len(),
            t.elapsed(),
            mask.iter().filter(|&&a| a > 0).count()
        );
    }
    let t = std::time::Instant::now();
    let segs = crate::view3d::preview(&model, &cam, frame, 6000);
    println!("aperçu : {:?}, {} segments", t.elapsed(), segs.len());
    println!("boîte : {:?}", model.bounds);
    // Répartition des sommets (1 %, 5 %, 50 %, 95 %, 99 %) sur chaque axe.
    for k in 0..3 {
        let mut v: Vec<f32> = model.mesh.positions.iter().map(|p| p[k]).collect();
        v.sort_by(f32::total_cmp);
        let q = |f: f32| v[((v.len() - 1) as f32 * f) as usize];
        println!("axe {k} : {:.1} {:.1} {:.1} {:.1} {:.1}", q(0.01), q(0.05), q(0.5), q(0.95), q(0.99));
    }
    if let Ok(png) = std::env::var("PELURE_GLB_PNG") {
        let segs = wireframe(&model, &cam, frame, true);
        let mask = crate::view3d::rasterize(&segs, [1920, 1080], 1.0, 1.3);
        let pixels: Vec<eframe::egui::Color32> =
            mask.iter().map(|&a| eframe::egui::Color32::from_gray(255 - a)).collect();
        crate::io::write_png(std::path::Path::new(&png), &pixels, 1920, 1080).unwrap();
    }
}

#[test]
fn missing_model_file_is_a_warning_not_a_failure() {
    let dir = std::env::temp_dir().join(format!("pelure-test-repere3d-manque-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("M.pelure");
    let mut doc = Document::with_frame(32, 24, 24);
    let id = doc.add_model("decor.glb".into(), cube_mesh());
    doc.panels[0].ref3d = Some(Ref3d::new(id, Cam3d::fit(cube_bounds(), FRAME)));
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    std::fs::remove_file(dir.join("modeles").join(format!("{id}.pelure3d"))).unwrap();
    let loaded = project::load(&manifest).unwrap();
    assert_eq!(loaded.doc.panels[0].ref3d, None);
    assert!(loaded.warnings.iter().any(|w| w.contains("decor.glb")), "{:?}", loaded.warnings);
    std::fs::remove_dir_all(&dir).ok();
}
