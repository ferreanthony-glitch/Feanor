//! Essais du décor 3D d'une séquence (parties 180 et 181) : lecture glTF (et refus des fichiers abîmés), enregistrement à côté
//! du manifeste, décor par séquence, rendu dans l'image du plan et à l'export, aspect (ciel, brume, ombres), budget de la
//! machine, allègement et simplification, import par l'interface ; puis (partie 181) un décor de plusieurs objets : ajout,
//! annulation, fichiers partagés, ancien fichier, budget de la somme, rendu de tous les objets, origine et aimant.

use std::sync::Arc;

use serde_json::{json, Value};

use crate::decor3d::{self, Decor3d, Modele3d, Objet3d, Place3d};
use crate::decor3dbudget::{budget_depuis, BudgetMachine, Chronos, Cout, Qualite};
use crate::decor3dgltf::{self as gltf, Primitive3d, Scene3d};
use crate::decor3drendu::{Aspect, Prise};
use crate::document::{Document, Layer, LayerKind};
use crate::espace3d::Monde;

// ------------------------------------------------------------------------------------ fichiers d'essai

/// Un conteneur `.glb` fait à la main (sans l'écrivain de FEANOR) : en-tête, JSON, binaire.
fn conteneur(j: &Value, bin: &[u8]) -> Vec<u8> {
    let mut t = serde_json::to_vec(j).unwrap();
    while t.len() % 4 != 0 {
        t.push(b' ');
    }
    let mut b = bin.to_vec();
    while b.len() % 4 != 0 {
        b.push(0);
    }
    let total = 12 + 8 + t.len() + 8 + b.len();
    let mut out = b"glTF".to_vec();
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(t.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&t);
    out.extend_from_slice(&(b.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&b);
    out
}

fn png(w: u32, h: u32, px: &[[u8; 4]]) -> Vec<u8> {
    use image::ImageEncoder;
    let raw: Vec<u8> = px.iter().flatten().copied().collect();
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out).write_image(&raw, w, h, image::ExtendedColorType::Rgba8).unwrap();
    out
}

fn b64(d: &[u8]) -> String {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in d.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            s.push(if k <= c.len() { A[(n >> (18 - 6 * k) & 63) as usize] as char } else { '=' });
        }
    }
    s
}

/// Un carré de 2 × 2 m face à +Z, centré, d'une matière rouge ; `avec_normales`, `texture` (une image PNG), `extra` : des
/// champs posés dans la description (extensions exigées, carte de normales…).
pub(crate) fn carre(avec_normales: bool, texture: Option<Vec<u8>>, extra: impl Fn(&mut Value)) -> Vec<u8> {
    let pos: [[f32; 3]; 4] = [[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [1.0, 1.0, 0.0], [-1.0, 1.0, 0.0]];
    let nrm: [[f32; 3]; 4] = [[0.0, 0.0, 1.0]; 4];
    let uv: [[f32; 2]; 4] = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    let idx: [u16; 6] = [0, 1, 2, 0, 2, 3];
    let mut bin = Vec::new();
    let mut vues = Vec::new();
    let mut ajouter = |bin: &mut Vec<u8>, d: &[u8]| {
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        vues.push(json!({ "buffer": 0, "byteOffset": bin.len(), "byteLength": d.len() }));
        bin.extend_from_slice(d);
        vues.len() - 1
    };
    let f = |v: &[[f32; 3]]| v.iter().flatten().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    let vp = ajouter(&mut bin, &f(&pos));
    let vn = ajouter(&mut bin, &f(&nrm));
    let vu = ajouter(&mut bin, &uv.iter().flatten().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>());
    let vi = ajouter(&mut bin, &idx.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>());
    let vimg = texture.as_ref().map(|t| ajouter(&mut bin, t));
    let mut attributs = json!({ "POSITION": 0, "TEXCOORD_0": 2 });
    if avec_normales {
        attributs["NORMAL"] = json!(1);
    }
    let mut pbr = json!({ "baseColorFactor": [0.8, 0.2, 0.1, 1.0] });
    if vimg.is_some() {
        pbr["baseColorTexture"] = json!({ "index": 0 });
    }
    let mut j = json!({
        "asset": { "version": "2.0" },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "mesh": 0 }],
        "meshes": [{ "primitives": [{ "attributes": attributs, "indices": 3, "material": 0 }] }],
        "materials": [{ "pbrMetallicRoughness": pbr }],
        "accessors": [
            { "bufferView": vp, "componentType": 5126, "count": 4, "type": "VEC3", "min": [-1, -1, 0], "max": [1, 1, 0] },
            { "bufferView": vn, "componentType": 5126, "count": 4, "type": "VEC3" },
            { "bufferView": vu, "componentType": 5126, "count": 4, "type": "VEC2" },
            { "bufferView": vi, "componentType": 5123, "count": 6, "type": "SCALAR" },
        ],
        "bufferViews": vues,
        "buffers": [{ "byteLength": bin.len() }],
    });
    if let Some(v) = vimg {
        j["images"] = json!([{ "bufferView": v, "mimeType": "image/png" }]);
        j["textures"] = json!([{ "source": 0 }]);
    }
    extra(&mut j);
    conteneur(&j, &bin)
}

fn quatre_pixels() -> Vec<u8> {
    png(2, 2, &[[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255, 255, 255, 255]])
}

pub(crate) fn dossier(nom: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("feanor-decor3d-{nom}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Un décor du carré, posé à `place` (l'origine au point (0, 0, 0) du modèle, comme à la partie 180), sans ciel ni brume (le
/// transparent autour se voit).
fn decor_du_carre(doc: &mut Document, seq: u64, place: Place3d) {
    let octets = carre(true, None, |_| {});
    let (scene, _) = gltf::lire(&octets, None).unwrap();
    let id = doc.alloc();
    let aspect = Aspect { ciel: false, brume: false, ombres: false, ..Aspect::default() };
    let modele = Arc::new(Modele3d::new(Arc::new(scene), Arc::new(octets)));
    let mut d = Decor3d::new(aspect);
    d.objets.push(Objet3d { id, nom: "Carré".into(), source: "carre.glb".into(), allege: Vec::new(), place, origine: [0.0; 3], aimante: false, modele, couleur: None, cache: false, verrou: false, cles: Vec::new(), caches_dans: Vec::new() });
    doc.decors3d.par_sequence.insert(seq, d);
}

/// Un projet de 160 × 90 (feuille 320 × 180) dont la séquence a le carré pour décor, debout derrière la feuille, au centre.
fn projet_avec_decor() -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    let seq = doc.sequence_of(0);
    decor_du_carre(&mut doc, seq, Place3d { position: [0.0, 0.0, -1.0], rotation_y: 0.0, echelle: 1.0 });
    doc
}

// ------------------------------------------------------------------------------------------------ lecture

#[test]
fn un_carre_colore_se_lit_positions_normales_coordonnees_couleur() {
    let (s, m) = gltf::lire(&carre(true, None, |_| {}), None).unwrap();
    assert_eq!(s.triangles(), 2);
    assert_eq!(m.triangles, 2);
    let p = &s.primitives[0];
    assert_eq!(p.positions.len(), 4);
    assert_eq!(p.positions[2], [1.0, 1.0, 0.0]);
    assert!(p.normales.iter().all(|n| (n[2] - 1.0).abs() < 1e-6));
    assert_eq!(p.uvs[0], [0.0, 1.0]);
    assert_eq!(p.couleur, [0.8, 0.2, 0.1, 1.0]);
    assert!(p.texture.is_none());
    assert_eq!(s.bornes(), ([-1.0, -1.0, 0.0], [1.0, 1.0, 0.0]));
}

#[test]
fn un_carre_texture_garde_son_image() {
    let (s, m) = gltf::lire(&carre(true, Some(quatre_pixels()), |_| {}), None).unwrap();
    assert_eq!(s.textures.len(), 1);
    let t = &s.textures[0];
    assert_eq!((t.largeur, t.hauteur), (2, 2));
    assert_eq!(&t.rgba[0..4], &[255, 0, 0, 255]);
    assert_eq!(&t.rgba[4..8], &[0, 255, 0, 255]);
    assert_eq!(s.primitives[0].texture, Some(0));
    assert_eq!((m.textures, m.octets_textures, m.plus_grand_cote), (1, 16, 2));
}

#[test]
fn sans_normales_le_lecteur_les_fait_plates() {
    let (s, _) = gltf::lire(&carre(false, None, |_| {}), None).unwrap();
    let p = &s.primitives[0];
    assert_eq!(p.positions.len(), 6, "des sommets propres à chaque triangle");
    assert!(p.normales.iter().all(|n| (n[2] - 1.0).abs() < 1e-5), "{:?}", p.normales);
}

#[test]
fn compression_et_extensions_exigees_sont_refusees_avec_un_message() {
    let draco = carre(true, None, |j| j["extensionsRequired"] = json!(["KHR_draco_mesh_compression"]));
    let e = gltf::lire(&draco, None).unwrap_err();
    assert!(e.contains("KHR_draco_mesh_compression") && e.contains("compression"), "{e}");
    let quantifie = carre(true, None, |j| j["accessors"][0]["componentType"] = json!(5123));
    assert!(gltf::lire(&quantifie, None).unwrap_err().contains("quantifié"));
    let clairseme = carre(true, None, |j| j["accessors"][0]["sparse"] = json!({ "count": 1 }));
    assert!(gltf::lire(&clairseme, None).unwrap_err().contains("clairsemé"));
}

fn crc32(d: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in d {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

#[test]
fn un_fichier_abime_est_refuse_proprement() {
    // Un accesseur qui ment sur son nombre d'éléments, ou sur sa place.
    let menteur = carre(true, None, |j| j["accessors"][0]["count"] = json!(10_000_000));
    assert!(gltf::lire(&menteur, None).is_err());
    let enorme = carre(true, None, |j| j["accessors"][3]["count"] = json!(u64::MAX / 2));
    assert!(gltf::lire(&enorme, None).is_err());
    let decale = carre(true, None, |j| j["accessors"][1]["byteOffset"] = json!(u64::MAX - 3));
    assert!(gltf::lire(&decale, None).is_err());
    // Un .glb tronqué.
    let entier = carre(true, None, |_| {});
    for n in [3, 12, 20, entier.len() / 2, entier.len() - 5] {
        assert!(decor3d::lire_sans_paniquer(&entier[..n]).is_err(), "tronqué à {n}");
    }
    // Une image qui annonce 100 000 × 100 000 pixels : refusée sans être décodée.
    let mut faux = png(1, 1, &[[0, 0, 0, 255]]);
    faux[16..20].copy_from_slice(&100_000u32.to_be_bytes());
    faux[20..24].copy_from_slice(&100_000u32.to_be_bytes());
    let crc = crc32(&faux[12..29]);
    faux[29..33].copy_from_slice(&crc.to_be_bytes());
    let absurde = carre(true, Some(faux), |_| {});
    let e = gltf::lire(&absurde, None).unwrap_err();
    assert!(e.contains("100000") || e.contains("illisible"), "{e}");
    // Du n'importe quoi.
    assert!(decor3d::lire_sans_paniquer(b"glTF\x02\0\0\0\xff\xff\xff\xff\x10\0\0\0JSON{}").is_err());
    assert!(decor3d::lire_sans_paniquer(&[0xFF; 64]).is_err());
}

#[test]
fn un_gltf_texte_est_empaquete_en_glb_autonome() {
    let glb = carre(true, Some(quatre_pixels()), |_| {});
    // Le même en texte, son tampon en `data:`.
    let len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    let mut j: Value = serde_json::from_slice(&glb[20..20 + len]).unwrap();
    let bin = glb[20 + len + 8..].to_vec();
    j["buffers"][0]["uri"] = json!(format!("data:application/octet-stream;base64,{}", b64(&bin)));
    let texte = serde_json::to_vec(&j).unwrap();
    let paquet = gltf::empaqueter(&texte, None).unwrap();
    assert!(paquet.starts_with(b"glTF"));
    let (a, _) = gltf::lire(&paquet, None).unwrap();
    let (b, _) = gltf::lire(&glb, None).unwrap();
    assert_eq!(a.primitives[0].positions, b.primitives[0].positions);
    assert_eq!(a.textures[0].rgba, b.textures[0].rgba);
    // Un .glb qui ne cite rien d'extérieur est gardé tel quel.
    assert_eq!(gltf::empaqueter(&glb, None).unwrap(), glb);
}

// ----------------------------------------------------------------------------------------------- le projet

#[test]
fn le_decor_s_enregistre_a_cote_du_manifeste_et_se_relit() {
    let dir = dossier("fichier");
    let manifeste = dir.join("Film.shot");
    let doc = projet_avec_decor();
    crate::project::save(&doc, &manifeste, &mut Default::default()).unwrap();
    let d = &doc.decor_du_plan(0).unwrap().objets[0];
    let fichier = dir.join(decor3d::DIR).join(format!("{}.glb", d.modele.empreinte));
    assert_eq!(std::fs::read(&fichier).unwrap(), *d.modele.octets, "les octets du modèle, copiés tels quels");
    let texte = std::fs::read_to_string(dir.join(decor3d::FILE)).unwrap();
    assert!(texte.contains("\"fichier_d_origine\": \"carre.glb\""), "{texte}");
    assert!(!std::fs::read_to_string(&manifeste).unwrap().contains("decor3d"), "le manifeste ne change pas (pas de VERSION neuve)");
    let relu = crate::project::load(&manifeste).unwrap();
    assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
    let rd = relu.doc.decor_du_plan(0).expect("décor relu");
    assert_eq!(rd.aspect, doc.decor_du_plan(0).unwrap().aspect);
    let r = &rd.objets[0];
    assert_eq!((r.id, &r.nom, &r.source, r.place, r.origine, r.aimante), (d.id, &d.nom, &d.source, d.place, d.origine, d.aimante));
    assert_eq!(r.modele.scene.triangles(), 2);
    // Enlevé, le décor part du disque à l'enregistrement suivant.
    let mut sans = relu.doc;
    sans.decors3d.par_sequence.clear();
    crate::project::save(&sans, &manifeste, &mut Default::default()).unwrap();
    assert!(!dir.join(decor3d::FILE).exists() && !fichier.exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn un_projet_sans_fichier_de_decors_s_ouvre_sans_decor() {
    let dir = dossier("sans");
    let manifeste = dir.join("Film.shot");
    crate::project::save(&Document::with_frame(160, 90, 24), &manifeste, &mut Default::default()).unwrap();
    let relu = crate::project::load(&manifeste).unwrap();
    assert!(relu.doc.decors3d.is_empty() && relu.warnings.is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn un_modele_manquant_n_empeche_pas_d_ouvrir_et_n_est_pas_efface() {
    let dir = dossier("manquant");
    let manifeste = dir.join("Film.shot");
    let doc = projet_avec_decor();
    crate::project::save(&doc, &manifeste, &mut Default::default()).unwrap();
    let o = &doc.decor_du_plan(0).unwrap().objets[0];
    let id = o.id;
    std::fs::remove_file(dir.join(decor3d::DIR).join(format!("{}.glb", o.modele.empreinte))).unwrap();
    let relu = crate::project::load(&manifeste).unwrap();
    assert!(relu.doc.decor_du_plan(0).is_none());
    assert!(relu.warnings.iter().any(|w| w.contains("décor 3D")), "{:?}", relu.warnings);
    crate::project::save(&relu.doc, &manifeste, &mut Default::default()).unwrap();
    assert!(std::fs::read_to_string(dir.join(decor3d::FILE)).unwrap().contains(&id.to_string()), "l'entrée est gardée telle quelle");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn chaque_plan_de_la_sequence_voit_son_decor_et_pas_ceux_d_une_autre() {
    let mut doc = projet_avec_decor();
    let seq = doc.sequence_of(0);
    // Un second plan dans la même séquence, puis une autre séquence.
    let scene = doc.panels[0].scene;
    let p = doc.make_panel(scene);
    doc.panels.push(p);
    let s2 = doc.new_sequence();
    let sc2 = doc.new_scene(s2);
    let p2 = doc.make_panel(sc2);
    doc.panels.push(p2);
    assert_eq!(doc.sequence_of(1), seq);
    assert_ne!(doc.sequence_of(2), seq);
    let a_un_decor = |doc: &Document, i: usize| crate::render::panel_plan_at(doc, i, 0, None).items.iter().any(|it| matches!(it.fond, Some(crate::render::FondPart::Decor(_))));
    assert!(a_un_decor(&doc, 0) && a_un_decor(&doc, 1));
    assert!(!a_un_decor(&doc, 2));
    // Le décor est l'élément le plus bas du plan.
    assert!(matches!(crate::render::panel_plan_at(&doc, 0, 0, None).items[0].fond, Some(crate::render::FondPart::Decor(_))));
}

/// L'image du plan 0 à l'image 0, exportée (le chemin d'export, exact, par le processeur).
pub(crate) fn image_exportee(doc: &Document) -> Vec<eframe::egui::Color32> {
    use crate::exportrender::{snapshot_of, Papier, Rendu};
    let rendu = Rendu::new(snapshot_of(doc, &[0]), Papier::Aucun, false, Vec::new(), Arc::new(std::sync::atomic::AtomicBool::new(false)));
    (*rendu.image(0, 0).expect("image")).clone()
}

#[test]
fn le_decor_se_voit_dans_l_image_exportee_et_les_calques_passent_dessus() {
    let mut doc = projet_avec_decor();
    let img = image_exportee(&doc);
    let [w, h] = doc.frame;
    let centre = img[(h / 2) * w + w / 2];
    assert_eq!(centre.a(), 255, "le carré au centre de l'image");
    assert!(centre.r() > centre.b(), "sa couleur rouge, éclairée : {centre:?}");
    assert_eq!(img[2 * w + 2].a(), 0, "transparent hors du carré (sans ciel)");
    // Un calque dessiné au centre passe par-dessus le décor.
    let c = doc.new_cel(LayerKind::Bitmap);
    let (cx, cy) = (doc.width / 2, doc.height / 2);
    let r = &mut doc.cels.get_mut(&c).unwrap().raster;
    for y in cy - 3..cy + 3 {
        for x in cx - 3..cx + 3 {
            let idx = r.tile_index_of(x, y);
            let (ox, oy, _, _) = r.tile_bounds(idx);
            r.tile_mut(idx)[(y - oy) * crate::raster::TILE + (x - ox)] = eframe::egui::Color32::from_rgb(0, 0, 255);
        }
    }
    let mut l = Layer::new("Perso", LayerKind::Bitmap);
    l.frames = vec![Some(c); doc.panels[0].length.max(1)];
    doc.panels[0].layers.push(l);
    let img = image_exportee(&doc);
    let centre = img[(h / 2) * w + w / 2];
    assert_eq!((centre.r(), centre.g(), centre.b()), (0, 0, 255), "le calque bleu au-dessus du décor");
    assert_eq!(img[(h / 2) * w + w / 2 + 20].a(), 255, "le décor autour");
}

#[test]
fn l_ecran_sans_carte_n_attend_jamais_le_rendu_du_decor() {
    // À l'écran (`fx::differe`) : rien n'est calculé ici, le rendu part sur un autre fil.
    let doc = projet_avec_decor();
    let plan = crate::render::panel_plan_at(&doc, 0, 0, None);
    let Some(crate::render::FondPart::Decor(key)) = plan.items[0].fond.clone() else { panic!("décor attendu") };
    let debut = std::time::Instant::now();
    let _ = crate::fx::differe(|| crate::decor3d::montre(&doc, &key).is_some());
    assert!(debut.elapsed() < std::time::Duration::from_millis(50));
    decor3d::wait_idle();
    assert!(crate::fx::differe(|| crate::decor3d::montre(&doc, &key)).is_some(), "le rendu arrivé est montré");
}

// --------------------------------------------------------------------------------------------- l'aspect

/// Un sol de 40 m de profondeur et une boîte posée dessus, vus de l'œil de référence.
fn sol_et_boite() -> (Scene3d, Prise) {
    let mut sol = Primitive3d { couleur: [0.6, 0.7, 0.5, 1.0], ..Default::default() };
    for p in [[-20.0, -1.5, 0.0], [20.0, -1.5, 0.0], [20.0, -1.5, -40.0], [-20.0, -1.5, -40.0]] {
        sol.positions.push(p);
        sol.normales.push([0.0, 1.0, 0.0]);
    }
    sol.indices = vec![0, 1, 2, 0, 2, 3];
    let mut boite = Primitive3d { couleur: [0.8, 0.6, 0.4, 1.0], ..Default::default() };
    let (a, b) = ([-0.6, -1.5, -4.0], [0.6, -0.3, -2.8]);
    for f in 0..6 {
        let (axe, cote) = (f / 2, f % 2);
        let base = boite.positions.len() as u32;
        let mut n = [0.0; 3];
        n[axe] = if cote == 0 { -1.0 } else { 1.0 };
        let (u, v) = ((axe + 1) % 3, (axe + 2) % 3);
        for (du, dv) in [(0, 0), (1, 0), (1, 1), (0, 1)] {
            let mut p = [0.0; 3];
            p[axe] = if cote == 0 { a[axe] } else { b[axe] };
            p[u] = if du == 0 { a[u] } else { b[u] };
            p[v] = if dv == 0 { a[v] } else { b[v] };
            boite.positions.push(p);
            boite.normales.push(n);
        }
        boite.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    let monde = Monde::new([320, 180], [160, 90]);
    let prise = Prise::de_la_feuille(&monde, monde.eye_reference()).unwrap();
    (Scene3d { primitives: vec![sol, boite], textures: Vec::new() }, prise)
}

fn ecart_a_l_horizon(px: &[u8]) -> f32 {
    let hz = crate::decor3drendu::HORIZON.map(|c| crate::decor3drendu::vers_srgb(c) * 255.0);
    (0..3).map(|k| (px[k] as f32 - hz[k]).abs()).sum()
}

#[test]
fn le_rendu_par_le_processeur_a_le_ciel_en_haut_et_la_brume_au_loin() {
    let (scene, prise) = sol_et_boite();
    let aspect = Aspect::default();
    let img = crate::decor3drendu::rendre(&scene, &crate::espace3dmath::IDENT, &prise, [320, 180], 1, 2, &aspect);
    let px = |x: usize, y: usize| img[(y * 320 + x) * 4..(y * 320 + x) * 4 + 4].to_vec();
    // Le ciel : opaque, plus bleu que rouge en haut.
    assert_eq!(px(10, 2)[3], 255);
    assert!(px(10, 2)[2] > px(10, 2)[0], "{:?}", px(10, 2));
    // Le sol : près (en bas) plus loin de la couleur de la brume que plus haut sur l'image (plus loin).
    // (Le sol va de la feuille, z = 0, à la ligne 130 environ, jusqu'à 40 m derrière, vers la ligne 95.)
    let pres = px(10, 127);
    let loin = px(10, 97);
    assert!(ecart_a_l_horizon(&loin) < ecart_a_l_horizon(&pres), "brume : loin {loin:?}, près {pres:?}");
    // Sans ciel ni brume : transparent en haut.
    let nu = Aspect { ciel: false, brume: false, ..aspect };
    let img = crate::decor3drendu::rendre(&scene, &crate::espace3dmath::IDENT, &prise, [320, 180], 1, 2, &nu);
    assert_eq!(img[(2 * 320 + 10) * 4 + 3], 0);
}

#[test]
fn facettes_ou_lisse_changent_l_ombrage() {
    let (mut scene, prise) = sol_et_boite();
    // Le sol aux normales penchées : lissé, il prend leur lumière ; en facettes, celle de sa face.
    for n in &mut scene.primitives[0].normales {
        *n = crate::espace3dmath::normalize([0.6, 0.6, 0.0]);
    }
    let a = Aspect { ciel: false, brume: false, ..Aspect::default() };
    let f = crate::decor3drendu::rendre(&scene, &crate::espace3dmath::IDENT, &prise, [320, 180], 1, 2, &a);
    let l = crate::decor3drendu::rendre(&scene, &crate::espace3dmath::IDENT, &prise, [320, 180], 1, 2, &Aspect { facettes: false, ..a });
    let i = (120 * 320 + 20) * 4;
    assert_ne!(f[i..i + 3], l[i..i + 3]);
}

#[test]
fn ombres_et_ciel_sur_la_carte_graphique() {
    let Some(rs) = crate::gpu_tests::headless() else { return };
    let (scene, prise) = sol_et_boite();
    let scene = Arc::new(scene);
    let mut mt = crate::decor3dgpu::Moteur::new(&rs.device, &rs.queue);
    let mut rendu = |q: Qualite| {
        let aspect = Aspect { soleil_azimut: 0.0, soleil_hauteur: 45.0, ..Aspect::default() };
        let d = [crate::decor3dgpu::Dessin { id: 7, scene: scene.clone(), matrice: crate::espace3dmath::IDENT, aspect, teinte: Default::default() }];
        crate::decor3dgpu::rendre_hors_ecran(&rs.device, &rs.queue, &mut mt, &d, &prise, [320, 180], true, q, 1).unwrap().0
    };
    let avec = rendu(Qualite::Hautes);
    let sans = rendu(Qualite::Sans);
    assert_eq!(avec[(2 * 320 + 10) * 4 + 3], 255, "le ciel couvre le haut");
    assert!(avec[(2 * 320 + 10) * 4 + 2] > avec[(2 * 320 + 10) * 4], "bleu en haut");
    let somme = |v: &[u8]| v.chunks(4).skip(320 * 100).map(|p| p[0] as u64 + p[1] as u64 + p[2] as u64).sum::<u64>();
    assert!(somme(&avec) < somme(&sans), "l'ombre de la boîte assombrit le sol : {} contre {}", somme(&avec), somme(&sans));
}

// ----------------------------------------------------------------------------------------- budget machine

fn chronos(gpu_par_million: f32) -> Chronos {
    let c = |extra: f32| Cout { fixe: 0.3, par_million: gpu_par_million * extra };
    Chronos { gpu: Some([c(1.0), c(1.6), c(2.4)]), prof4: Some(Cout { fixe: 0.05, par_million: 0.5 }), cpu: Cout { fixe: 5.0, par_million: 900.0 }, vram: Some(8 << 30), ram: Some(32 << 30) }
}

#[test]
fn le_budget_se_deduit_des_mesures_et_suit_leur_echelle() {
    let rapide = budget_depuis(&chronos(0.5), "A".into(), 1);
    let lent = budget_depuis(&chronos(1.0), "A".into(), 1);
    // Une carte deux fois plus lente : deux fois moins de triangles (le coût fixe est le même).
    let ratio = rapide.triangles as f64 / lent.triangles as f64;
    assert!((ratio - 2.0).abs() < 0.05, "{ratio}");
    let ms = BudgetMachine::ms_par_passe();
    assert!(Cout { fixe: 0.3, par_million: 1.0 }.ms(lent.triangles) <= ms + 1e-3);
    assert_eq!(rapide.textures_mo, 2048, "un quart de 8 Go de mémoire vidéo");
    // Sans carte : le processeur, plus lent, et pas d'ombres.
    let cpu = budget_depuis(&Chronos { gpu: None, ..chronos(1.0) }, "A".into(), 1);
    assert!(!cpu.carte && cpu.triangles < lent.triangles);
    assert_eq!(cpu.qualite_pour(1000), Qualite::Sans);
}

/// Partie 204 (l'essai de la carte graphique des cyclos échouait une fois sur trois en suite complète) : un étalonnage fait sur un
/// ordinateur très occupé mesure un coût fixe plus long qu'une image ; le budget n'en tombe pas à 0 triangle, il garde son plancher,
/// et un carré n'est pas masqué.
#[test]
fn un_etalonnage_fausse_par_un_ordinateur_occupe_garde_un_plancher() {
    let occupe = |fixe: f32| Cout { fixe, par_million: 50.0 };
    let ch = Chronos { gpu: Some([occupe(40.0), occupe(45.0), occupe(50.0)]), prof4: Some(Cout { fixe: 5.0, par_million: 1.0 }), cpu: Cout { fixe: 100_000.0, par_million: 9000.0 }, vram: None, ram: None };
    assert_eq!(occupe(40.0).triangles_sous(BudgetMachine::ms_par_passe()), 0, "la mesure faussée seule donnerait 0");
    let b = budget_depuis(&ch, "A".into(), 1);
    assert_eq!((b.triangles, b.triangles_bords), (crate::decor3dbudget::PLANCHER_TRIANGLES, crate::decor3dbudget::PLANCHER_TRIANGLES));
    let cpu = budget_depuis(&Chronos { gpu: None, ..ch }, "A".into(), 1);
    assert_eq!(cpu.triangles, crate::decor3dbudget::PLANCHER_TRIANGLES);
    assert!(b.triangles >= 2, "un carré tient");
}

#[test]
fn la_qualite_des_ombres_suit_le_budget_mesure() {
    let b = budget_depuis(&chronos(1.0), "A".into(), 1);
    // Peu de triangles : les ombres hautes tiennent ; beaucoup : basses, puis aucune.
    assert_eq!(b.qualite_pour(10_000), Qualite::Hautes);
    let ms = BudgetMachine::ms_par_passe();
    let basses_max = Cout { fixe: 0.3, par_million: 1.6 }.triangles_sous(ms);
    let hautes_max = Cout { fixe: 0.3, par_million: 2.4 }.triangles_sous(ms);
    assert_eq!(b.qualite_pour((hautes_max + basses_max) / 2), Qualite::Basses);
    assert_eq!(b.qualite_pour(basses_max + 10_000), Qualite::Sans);
}

#[test]
fn le_budget_garde_vaut_pour_sa_machine_seulement() {
    let b = budget_depuis(&chronos(1.0), "Carte A | Metal | 1.0 | Processeur".into(), 1);
    assert!(crate::decor3dbudget::vaut_pour(Some(&b), "Carte A | Metal | 1.0 | Processeur"));
    assert!(!crate::decor3dbudget::vaut_pour(Some(&b), "Carte B | Metal | 1.0 | Processeur"), "une autre carte : remesurer");
    assert!(!crate::decor3dbudget::vaut_pour(None, "x"));
    // Gardé dans les réglages et relu tel quel.
    let s = crate::settings::Settings { decor3d: Some(b.clone()), ..Default::default() };
    let texte = toml::to_string(&s).unwrap();
    assert!(texte.contains("budget_decor_3d"));
    let relu: crate::settings::Settings = toml::from_str(&texte).unwrap();
    assert_eq!(relu.decor3d, Some(b));
}

pub(crate) fn attendre(b: &mut crate::perf_tests::Banc) {
    let debut = std::time::Instant::now();
    while b.app.decor3d.en_attente() || crate::decor3dbudget::en_cours() {
        b.chauffer(1);
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(debut.elapsed() < std::time::Duration::from_secs(120), "l'import ne finit pas");
    }
    b.chauffer(1);
}

#[test]
fn une_cle_de_machine_differente_fait_remesurer_a_l_import() {
    let mut b = crate::perf_tests::banc_taille(Document::with_frame(160, 90, 24), false, 1200, 800);
    b.chauffer(2);
    b.app.settings.decor3d = Some(budget_depuis(&chronos(1.0), "une autre machine".into(), 1));
    assert!(b.app.decor3d_budget().is_none(), "la clé ne correspond pas");
    let dir = dossier("cle");
    let f = dir.join("carre.glb");
    std::fs::write(&f, carre(true, None, |_| {})).unwrap();
    b.app.decor3d_importer_chemin(f);
    attendre(&mut b);
    assert!(b.app.decor3d_budget().is_some(), "remesuré pour cette machine");
    assert!(b.app.doc.decor_du_plan(0).is_some(), "puis importé : un carré tient dans tout budget");
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------------- allègement, simplification

pub(crate) fn budget_large() -> BudgetMachine {
    BudgetMachine { cle: crate::decor3dbudget::cle_machine(None), date: 1, triangles: 1_000_000, triangles_bords: 1_000_000, textures_mo: 512, carte: false, gpu: None, cpu: Cout { fixe: 1.0, par_million: 100.0 }, prof4: None }
}

#[test]
fn le_verdict_compare_les_mesures_au_budget() {
    let (_, m) = gltf::lire(&carre(true, None, |_| {}), None).unwrap();
    assert_eq!(decor3d::verdict(&m, &budget_large(), None), decor3d::Verdict::Direct);
    let serre = BudgetMachine { triangles: 1, ..budget_large() };
    match decor3d::verdict(&m, &serre, None) {
        decor3d::Verdict::Lourd { tel_quel, simplifier, raisons } => {
            assert!(!tel_quel && simplifier, "trop de triangles : ni tel quel, la simplification proposée");
            assert!(raisons[0].contains("triangles"));
        }
        v => panic!("{v:?}"),
    }
    let peu_de_memoire = BudgetMachine { textures_mo: 0, ..budget_large() };
    let (_, mt) = gltf::lire(&carre(true, Some(quatre_pixels()), |_| {}), None).unwrap();
    assert!(matches!(decor3d::verdict(&mt, &peu_de_memoire, None), decor3d::Verdict::Lourd { tel_quel: false, simplifier: false, .. }));
    // Trop lent, mesuré sur la carte : lourd aussi. (Partie 198.5 : pour un modèle qui n'est pas loin sous le budget ; loin sous lui,
    // le temps mesuré n'est que le coût fixe d'une passe, déjà dans le budget : il ne le juge pas.)
    let juste = BudgetMachine { triangles: m.triangles as u64 * 4, ..budget_large() };
    assert!(matches!(decor3d::verdict(&m, &juste, Some(50.0)), decor3d::Verdict::Lourd { simplifier: true, .. }));
    assert_eq!(decor3d::verdict(&m, &budget_large(), Some(50.0)), decor3d::Verdict::Direct);
}

#[test]
fn alleger_reduit_les_textures_retire_les_cartes_et_garde_la_forme() {
    // Une texture de 64 × 64, une carte de normales que FEANOR ne montre pas, une animation.
    let grande = png(64, 64, &vec![[200, 100, 50, 255]; 64 * 64]);
    let normales = png(8, 8, &vec![[128, 128, 255, 255]; 64]);
    let octets = carre(true, Some(grande), |j| {
        j["images"].as_array_mut().unwrap().push(json!({ "uri": format!("data:image/png;base64,{}", b64(&normales)) }));
        j["textures"].as_array_mut().unwrap().push(json!({ "source": 1 }));
        j["materials"][0]["normalTexture"] = json!({ "index": 1 });
        j["animations"] = json!([{ "channels": [], "samplers": [] }]);
    });
    let dir = dossier("alleger");
    let f = dir.join("rue.glb");
    std::fs::write(&f, &octets).unwrap();
    let a = decor3d::analyser(&f).unwrap();
    assert_eq!((a.mesures.cartes_inutiles, a.mesures.animations, a.mesures.plus_grand_cote), (1, 1, 64));
    let r = decor3d::alleger(&a, decor3d::Options { cote_max: Some(16), triangles: None }, &Default::default()).unwrap();
    assert_eq!(std::fs::read(&f).unwrap(), octets, "le fichier d'origine n'est pas touché");
    let (avant, apres) = (&a.scene.primitives[0], &r.scene.primitives[0]);
    assert_eq!(r.scene.triangles(), a.scene.triangles());
    assert_eq!(apres.positions, avant.positions);
    assert_eq!(apres.uvs, avant.uvs);
    assert_eq!(apres.couleur, avant.couleur);
    assert_eq!((r.scene.textures[0].largeur, r.scene.textures[0].hauteur), (16, 16));
    assert_eq!((r.mesures.cartes_inutiles, r.mesures.animations), (0, 0), "carte de normales et animation retirées");
    assert!(r.mesures.octets_textures < a.mesures.octets_textures);
    assert!(r.notes.iter().any(|n| n.contains("64 → 16")) && r.notes.iter().any(|n| n.contains("normales")), "{:?}", r.notes);
    assert!(gltf::lire(&r.glb, None).is_ok(), "relu par le lecteur ordinaire : un .glb correct");
    std::fs::remove_dir_all(&dir).ok();
}

/// Une grille ondulée de `n` × `n` carrés, coordonnées de texture de 0 à 1, en deux matières (deux moitiés).
fn grille_dense(n: usize) -> Scene3d {
    let moitie = |x0: usize, x1: usize, couleur: [f32; 4]| {
        let mut p = Primitive3d { couleur, ..Default::default() };
        let cols = x1 - x0 + 1;
        for j in 0..=n {
            for i in x0..=x1 {
                let (u, v) = (i as f32 / n as f32, j as f32 / n as f32);
                p.positions.push([u * 4.0 - 2.0, (u * 9.0).sin() * (v * 7.0).cos() * 0.2, v * 4.0 - 2.0]);
                p.normales.push([0.0, 1.0, 0.0]);
                p.uvs.push([u, v]);
            }
        }
        for j in 0..n {
            for i in 0..cols - 1 {
                let a = (j * cols + i) as u32;
                let c = cols as u32;
                p.indices.extend([a, a + c, a + 1, a + 1, a + c, a + c + 1]);
            }
        }
        p
    };
    Scene3d { primitives: vec![moitie(0, n / 2, [0.8, 0.3, 0.2, 1.0]), moitie(n / 2, n, [0.2, 0.5, 0.8, 1.0])], textures: Vec::new() }
}

#[test]
fn simplifier_reduit_vers_la_cible_et_garde_boite_matieres_et_coordonnees() {
    let s = grille_dense(120);
    let total = s.triangles();
    let cible = total / 4;
    let r = crate::decor3dsimplifier::simplifier(&s, cible, &Default::default()).unwrap();
    let t = r.triangles();
    assert!(t < total / 2 && (t as f64) < cible as f64 * 1.4, "{total} → {t} (cible {cible})");
    let ((a0, a1), (b0, b1)) = (s.bornes(), r.bornes());
    for k in 0..3 {
        assert!((a0[k] - b0[k]).abs() < 0.05 && (a1[k] - b1[k]).abs() < 0.05, "boîte : {:?} → {:?}", s.bornes(), r.bornes());
    }
    assert_eq!(r.primitives.len(), 2, "deux matières, deux primitives");
    assert_eq!(r.primitives[1].couleur, s.primitives[1].couleur);
    assert!(r.primitives.iter().flat_map(|p| &p.uvs).all(|uv| (0.0..=1.0).contains(&uv[0]) && (0.0..=1.0).contains(&uv[1])));
    // Écrit et relu : un .glb correct, les mêmes triangles.
    let glb = gltf::ecrire_glb(&r, gltf::Ecriture { cote_max: None }).unwrap();
    let (relu, _) = gltf::lire(&glb, None).unwrap();
    assert_eq!(relu.triangles(), t);
    // Et par l'allègement : la note le dit.
    let a = decor3d::analyser_octets(&gltf::ecrire_glb(&s, gltf::Ecriture { cote_max: None }).unwrap(), None, "Grille".into(), "grille.glb".into()).unwrap();
    let al = decor3d::alleger(&a, decor3d::Options { cote_max: None, triangles: Some(cible) }, &Default::default()).unwrap();
    assert!(al.notes.iter().any(|n| n.starts_with("simplifié")), "{:?}", al.notes);
}

#[test]
fn la_simplification_s_annule() {
    let s = grille_dense(80);
    let suivi = crate::decor3dsimplifier::Suivi::default();
    suivi.annule.store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(crate::decor3dsimplifier::simplifier(&s, 10, &suivi).is_err());
}

// ---------------------------------------------------------------------------------------------- l'interface

pub(crate) fn banc_avec_budget(budget: BudgetMachine) -> crate::perf_tests::Banc {
    let mut b = crate::perf_tests::banc_taille(Document::with_frame(160, 90, 24), false, 1200, 800);
    b.chauffer(2);
    b.app.settings.decor3d = Some(budget);
    b
}

#[test]
fn sous_le_budget_le_decor_s_importe_sans_fenetre_en_une_etape_d_annulation() {
    let mut b = banc_avec_budget(budget_large());
    let dir = dossier("import");
    let f = dir.join("carre.glb");
    let octets = carre(true, Some(quatre_pixels()), |_| {});
    std::fs::write(&f, &octets).unwrap();
    let etapes = b.app.history.undo_len();
    b.app.decor3d_importer_chemin(f.clone());
    attendre(&mut b);
    assert!(!b.app.decor3d.fenetre_ouverte(), "pas de fenêtre sous le budget");
    let d = &b.app.doc.decor_du_plan(0).expect("décor posé").objets[0];
    assert_eq!(d.source, "carre.glb");
    assert_eq!(*d.modele.octets, octets, "le .glb gardé tel quel");
    assert!(b.app.status.contains("posé dans le décor de la séquence"), "{}", b.app.status);
    // Posé sur le sol (son origine, le centre du dessous, sur le sol ; aimanté), derrière la feuille.
    let monde = Monde::of(&b.app.doc);
    assert!(d.aimante && d.origine == [0.0, -1.0, 0.0], "{d:?}");
    assert!((d.place.position[1] - monde.sol_y()).abs() < 1e-4 && d.place.position[2] <= 0.0, "{:?}", d.place);
    assert!((d.bornes_monde().0[1] - monde.sol_y()).abs() < 1e-4, "le dessous sur le sol");
    assert_eq!(b.app.history.undo_len(), etapes + 1);
    assert_eq!(std::fs::read(&f).unwrap(), octets);
    b.app.undo();
    assert!(b.app.doc.decor_du_plan(0).is_none(), "annulé d'un coup");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn au_dela_du_budget_la_fenetre_propose_d_alleger() {
    let mut b = banc_avec_budget(BudgetMachine { textures_mo: 0, ..budget_large() });
    let dir = dossier("lourd");
    let f = dir.join("carre.glb");
    std::fs::write(&f, carre(true, Some(quatre_pixels()), |_| {})).unwrap();
    b.app.decor3d_importer_chemin(f);
    attendre(&mut b);
    assert!(b.app.decor3d.fenetre_ouverte(), "« Ce décor est lourd »");
    assert!(b.app.doc.decor_du_plan(0).is_none(), "rien n'est posé avant le choix");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn un_fichier_refuse_le_dit_dans_la_barre_d_etat() {
    let mut b = banc_avec_budget(budget_large());
    let dir = dossier("refus");
    let f = dir.join("draco.glb");
    std::fs::write(&f, carre(true, None, |j| j["extensionsRequired"] = json!(["KHR_draco_mesh_compression"]))).unwrap();
    b.app.decor3d_importer_chemin(f);
    attendre(&mut b);
    assert!(b.app.status.starts_with("Décor 3D refusé") && b.app.status.contains("draco"), "{}", b.app.status);
    assert!(b.app.doc.decor_du_plan(0).is_none());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn les_commandes_du_decor_ont_leur_bloc_dans_le_fichier_de_raccourcis() {
    let map = crate::keymap_tests::default_map();
    for id in ["fichier.importer_decor3d", "decor3d.enlever", "decor3d.remesurer"] {
        assert!(map.bindings.iter().any(|b| b.action == id), "{id}");
        assert!(crate::keymap::ACTIONS.iter().any(|a| a.id == id), "{id}");
    }
}

// ------------------------------------------------------------------------------------------------ mesures

/// Les temps du décor sur cette machine, pour des tailles données (à lancer à la main, sur le PC d'Anthony aussi) :
/// `cargo test --lib decor3d_perf -- --ignored --nocapture`.
#[test]
#[ignore]
fn decor3d_perf() {
    let rs = crate::gpu_tests::headless();
    let ch = crate::decor3dbudget::etalonner(rs.as_ref(), [3840, 2160], [1920, 1080]);
    println!("étalonnage (feuille 3840 × 2160, cadre 1920 × 1080) : {ch:#?}");
    let b = budget_depuis(&ch, crate::decor3dbudget::cle_machine(rs.as_ref().map(|r| r.adapter.get_info()).as_ref()), 0);
    println!("machine : {}", b.cle);
    println!("budget : {} ; ombres pour 100 000 triangles : {:?} ; bords doux (partie 193) jusqu'à {} triangles sans ombres", b.texte(), b.qualite_pour(100_000), b.triangles_bords);
    for cote in [100usize, 300, 700, 1000] {
        let s = crate::decor3dbudget::scene_d_essai(cote, 2048);
        let monde = Monde::new([1920, 1080], [1920, 1080]);
        let prise = Prise::de_la_feuille(&monde, monde.eye_reference()).unwrap();
        let debut = std::time::Instant::now();
        let _ = crate::decor3drendu::rendre(&s, &crate::espace3dmath::IDENT, &prise, [1920, 1080], 1, 0, &Aspect::default());
        let cpu = debut.elapsed().as_secs_f32() * 1000.0;
        let gpu = rs.as_ref().and_then(|rs| {
            let mut mt = crate::decor3dgpu::Moteur::new(&rs.device, &rs.queue);
            let d = [crate::decor3dgpu::Dessin { id: 1, scene: Arc::new(s.clone()), matrice: crate::espace3dmath::IDENT, aspect: Aspect::default(), teinte: Default::default() }];
            let max = rs.device.limits().max_texture_dimension_2d.min(3840);
            let feuille = [max, max * 9 / 16];
            let prise_f = Prise::de_la_feuille(&Monde::new([feuille[0] as usize, feuille[1] as usize], [1920, 1080]), monde.eye_reference()).unwrap();
            // (Partie 193 : sans carte qui partage la profondeur, la passe d'avant ; avec, les bords doux en plus.)
            Some([false, true].map(|quatre| Qualite::TOUTES.map(|q| crate::decor3dgpu::rendre_hors_ecran_tout(&rs.device, &rs.queue, &mut mt, &d, &prise_f, feuille, true, q, 5, false, true, quatre).map(|r| r.1)).into_iter().collect::<Option<Vec<_>>>()))
        });
        println!("{} triangles, texture 2048² : processeur (1920 × 1080) {cpu:.0} ms ; carte (feuille, sans / basses / hautes ; sans carte qui partage la profondeur, puis avec les bords doux) {gpu:?} ms", s.triangles());
    }
}

/// Les modèles de la banque `assets/banque3d/` (s'il y en a une dans ce dossier) : chacun se lit, se mesure et se rend sans
/// erreur. Sauté sans la banque.
#[test]
fn les_modeles_de_la_banque_se_lisent_et_se_rendent() {
    let racine = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets").join("banque3d");
    let Ok(dossiers) = std::fs::read_dir(&racine) else { return };
    let mut lus = 0;
    for d in dossiers.flatten().filter(|d| d.path().is_dir()) {
        for f in std::fs::read_dir(d.path()).into_iter().flatten().flatten() {
            let p = f.path();
            if p.extension().and_then(|e| e.to_str()) != Some("glb") {
                continue;
            }
            let a = decor3d::analyser(&p).unwrap_or_else(|e| panic!("{} : {e}", p.display()));
            assert!(a.mesures.triangles > 0, "{}", p.display());
            let monde = Monde::new([320, 180], [160, 90]);
            let place = Place3d::d_origine(&a.scene, &monde);
            let prise = Prise::de_la_feuille(&monde, monde.eye_reference()).unwrap();
            let img = crate::decor3drendu::rendre(&a.scene, &place.matrice(), &prise, [320, 180], 1, 2, &Aspect { ciel: false, ..Aspect::default() });
            assert_eq!(img.len(), 320 * 180 * 4);
            lus += 1;
        }
    }
    assert!(lus > 0 || !racine.exists());
}

// ================================================================================ partie 181 : plusieurs objets

/// Un objet du carré, son origine au point (0, 0, 0) du modèle, à `place`.
fn objet_du_carre(doc: &mut Document, place: Place3d, modele: Arc<Modele3d>) -> Objet3d {
    Objet3d { id: doc.alloc(), nom: "Carré".into(), source: "carre.glb".into(), allege: Vec::new(), place, origine: [0.0; 3], aimante: false, modele, couleur: None, cache: false, verrou: false, cles: Vec::new(), caches_dans: Vec::new() }
}

fn modele_du_carre() -> Arc<Modele3d> {
    let octets = carre(true, None, |_| {});
    let (scene, _) = gltf::lire(&octets, None).unwrap();
    Arc::new(Modele3d::new(Arc::new(scene), Arc::new(octets)))
}

#[test]
fn ajouter_deux_objets_les_garde_tous_deux_et_annuler_retire_le_dernier() {
    let mut b = banc_avec_budget(budget_large());
    let dir = dossier("deux");
    let f = dir.join("carre.glb");
    std::fs::write(&f, carre(true, None, |_| {})).unwrap();
    let etapes = b.app.history.undo_len();
    b.app.decor3d_importer_chemin(f.clone());
    attendre(&mut b);
    b.app.decor3d_importer_chemin(f.clone());
    attendre(&mut b);
    let d = b.app.doc.decor_du_plan(0).expect("décor");
    assert_eq!(d.objets.len(), 2, "le second fichier s'ajoute, il ne remplace pas");
    assert!(Arc::ptr_eq(&d.objets[0].modele, &d.objets[1].modele), "le même contenu : le même modèle (une lecture, un envoi à la carte)");
    assert_ne!(d.objets[0].id, d.objets[1].id);
    assert!(b.app.status.contains("2 objets"), "{}", b.app.status);
    assert_eq!(b.app.history.undo_len(), etapes + 2, "une étape par ajout");
    b.app.undo();
    assert_eq!(b.app.doc.decor_du_plan(0).unwrap().objets.len(), 1, "annuler retire le dernier");
    b.app.undo();
    assert!(b.app.doc.decor_du_plan(0).is_none());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn dupliquer_et_enlever_un_objet_sont_une_etape_chacun() {
    let mut b = banc_avec_budget(budget_large());
    let seq = b.app.doc.sequence_of(0);
    let mdl = modele_du_carre();
    let mut d = Decor3d::new(Aspect::default());
    let o = objet_du_carre(&mut b.app.doc, Place3d::default(), mdl);
    let id = o.id;
    d.objets.push(o);
    b.app.doc.decors3d.par_sequence.insert(seq, d);
    let etapes = b.app.history.undo_len();
    b.app.decor3d_dupliquer_objet(id);
    let d = b.app.doc.decor_du_plan(0).unwrap();
    assert_eq!(d.objets.len(), 2);
    assert!(d.objets[1].place.position[0] > d.objets[0].place.position[0] + 1.9, "à côté, pas dessus : {:?}", d.objets[1].place);
    assert!(Arc::ptr_eq(&d.objets[0].modele, &d.objets[1].modele));
    assert_eq!(b.app.decor3d.choisi, Some(d.objets[1].id), "la copie est choisie");
    b.app.decor3d_enlever_objet(id);
    assert_eq!(b.app.doc.decor_du_plan(0).unwrap().objets.len(), 1);
    assert_eq!(b.app.history.undo_len(), etapes + 2);
    b.app.undo();
    b.app.undo();
    assert_eq!(b.app.doc.decor_du_plan(0).unwrap().objets.len(), 1);
    assert_eq!(b.app.doc.decor_du_plan(0).unwrap().objets[0].id, id);
}

#[test]
fn des_fichiers_identiques_ne_s_enregistrent_qu_une_fois_et_se_relisent_pareil() {
    let dir = dossier("partage");
    let manifeste = dir.join("Film.shot");
    let mut doc = Document::with_frame(160, 90, 24);
    let seq = doc.sequence_of(0);
    let mut d = Decor3d::new(Aspect::default());
    // Dix « arbres » : dix modèles chargés à part (dix imports), un seul contenu.
    for k in 0..10 {
        let o = objet_du_carre(&mut doc, Place3d { position: [k as f32 * 0.5, 0.0, -2.0], rotation_y: k as f32 * 10.0, echelle: 1.0 }, modele_du_carre());
        d.objets.push(o);
    }
    doc.decors3d.par_sequence.insert(seq, d);
    crate::project::save(&doc, &manifeste, &mut Default::default()).unwrap();
    let fichiers: Vec<_> = std::fs::read_dir(dir.join(decor3d::DIR)).unwrap().flatten().collect();
    assert_eq!(fichiers.len(), 1, "un seul fichier pour dix objets du même contenu");
    let relu = crate::project::load(&manifeste).unwrap();
    assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
    let (a, r) = (doc.decor_du_plan(0).unwrap(), relu.doc.decor_du_plan(0).unwrap());
    assert_eq!(r.objets.len(), 10);
    for (x, y) in a.objets.iter().zip(&r.objets) {
        assert_eq!((x.id, &x.nom, x.place, x.origine), (y.id, &y.nom, y.place, y.origine));
        assert!(Arc::ptr_eq(&y.modele, &r.objets[0].modele), "relus : un seul modèle partagé");
    }
    // Réenregistré : rien de plus sur le disque.
    crate::project::save(&relu.doc, &manifeste, &mut Default::default()).unwrap();
    assert_eq!(std::fs::read_dir(dir.join(decor3d::DIR)).unwrap().count(), 1);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn un_fichier_de_la_partie_180_se_lit_comme_un_objet_a_la_meme_place() {
    let dir = dossier("v1");
    let manifeste = dir.join("Film.shot");
    let doc = Document::with_frame(160, 90, 24);
    crate::project::save(&doc, &manifeste, &mut Default::default()).unwrap();
    let seq = doc.sequence_of(0);
    let octets = carre(true, None, |_| {});
    std::fs::create_dir_all(dir.join(decor3d::DIR)).unwrap();
    std::fs::write(dir.join(decor3d::DIR).join("9000.glb"), &octets).unwrap();
    let ancien = json!({ "version": 1, "decors": [{ "sequence": seq, "id": 9000, "nom": "Rue", "origine": "rue.glb", "fichier": "decors/9000.glb",
        "position": [0.3, 0.2, -1.5], "rotation_y": 30.0, "echelle": 1.5, "aspect": { "ciel": false } }] });
    std::fs::write(dir.join(decor3d::FILE), serde_json::to_vec_pretty(&ancien).unwrap()).unwrap();
    let place_180 = Place3d { position: [0.3, 0.2, -1.5], rotation_y: 30.0, echelle: 1.5 };
    let relu = crate::project::load(&manifeste).unwrap();
    assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
    let d = relu.doc.decor_du_plan(0).expect("un décor");
    assert_eq!(d.objets.len(), 1, "une liste d'un objet");
    assert!(!d.aspect.ciel, "son aspect gardé");
    let o = &d.objets[0];
    assert_eq!((o.id, o.nom.as_str(), o.source.as_str(), o.aimante), (9000, "Rue", "rue.glb", false), "pas aimanté : Anthony l'avait placé à la main");
    assert_eq!(o.origine, decor3d::origine_au_sol(o.modele.bornes), "l'origine d'office");
    let (a, b) = (o.matrice(), place_180.matrice());
    assert!(a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-5), "le décor n'a pas bougé : {a:?} contre {b:?}");
    // Réenregistré : version 2, le modèle sous son empreinte, l'ancien fichier parti ; relu, toujours à la même place.
    crate::project::save(&relu.doc, &manifeste, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(dir.join(decor3d::FILE)).unwrap();
    assert!(texte.contains("\"version\": 2") && texte.contains("\"objets\""), "{texte}");
    assert!(dir.join(decor3d::DIR).join(format!("{}.glb", o.modele.empreinte)).exists());
    assert!(!dir.join(decor3d::DIR).join("9000.glb").exists(), "l'ancien nom, plus cité, est effacé");
    let encore = crate::project::load(&manifeste).unwrap();
    let c = encore.doc.decor_du_plan(0).unwrap().objets[0].matrice();
    assert!(c.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-5));
    assert!(!std::fs::read_to_string(&manifeste).unwrap().contains("decor3d"), "le manifeste ne change pas (pas de VERSION neuve)");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn le_budget_vaut_pour_la_somme_des_objets() {
    let (_, m) = gltf::lire(&carre(true, Some(quatre_pixels()), |_| {}), None).unwrap();
    let b = BudgetMachine { triangles: 5, ..budget_large() };
    assert_eq!(decor3d::verdict_sequence(&m, decor3d::Poids::default(), false, &b, None), decor3d::Verdict::Direct, "seul, il tient");
    let deja = decor3d::Poids { triangles: 4, octets_textures: 0 };
    match decor3d::verdict_sequence(&m, deja, false, &b, None) {
        decor3d::Verdict::Lourd { simplifier, raisons, .. } => {
            assert!(simplifier && raisons[0].contains("déjà posés") && raisons[0].contains('6'), "{raisons:?}");
        }
        v => panic!("avec les autres, il dépasse : {v:?}"),
    }
    // Les textures d'un modèle déjà là ne comptent pas deux fois.
    let serre = BudgetMachine { textures_mo: 0, ..budget_large() };
    let deja = decor3d::Poids { triangles: 2, octets_textures: 0 };
    assert!(matches!(decor3d::verdict_sequence(&m, deja, false, &serre, None), decor3d::Verdict::Lourd { .. }));
    assert_eq!(decor3d::verdict_sequence(&m, deja, true, &serre, None), decor3d::Verdict::Direct, "le même modèle : rien de plus en mémoire");
    // Une séquence au bout du budget : refusée d'emblée.
    assert!(decor3d::sequence_pleine(decor3d::Poids { triangles: 5, octets_textures: 0 }, &b).is_some());
    assert!(decor3d::sequence_pleine(decor3d::Poids { triangles: 4, octets_textures: 0 }, &b).is_none());
    // Le poids d'un décor : chaque objet compte ses triangles, un modèle partagé ses textures une fois.
    let mut doc = Document::with_frame(160, 90, 24);
    let mdl = Arc::new(Modele3d::new(Arc::new(gltf::lire(&carre(true, Some(quatre_pixels()), |_| {}), None).unwrap().0), Arc::new(Vec::new())));
    let mut d = Decor3d::new(Aspect::default());
    for _ in 0..3 {
        let o = objet_du_carre(&mut doc, Place3d::default(), mdl.clone());
        d.objets.push(o);
    }
    assert_eq!(d.poids(), decor3d::Poids { triangles: 6, octets_textures: 16 });
}

#[test]
fn un_objet_qui_ferait_depasser_le_decor_ouvre_la_fenetre_ou_est_refuse() {
    let dir = dossier("somme");
    let f = dir.join("carre.glb");
    std::fs::write(&f, carre(true, None, |_| {})).unwrap();
    // Trois triangles au plus : un carré (deux) tient, le second ferait quatre.
    let mut b = banc_avec_budget(BudgetMachine { triangles: 3, ..budget_large() });
    b.app.decor3d_importer_chemin(f.clone());
    attendre(&mut b);
    assert_eq!(b.app.doc.decor_du_plan(0).unwrap().objets.len(), 1);
    b.app.decor3d_importer_chemin(f.clone());
    attendre(&mut b);
    assert!(b.app.decor3d.fenetre_ouverte(), "« Ce décor est lourd » pour l'objet qu'on ajoute");
    assert_eq!(b.app.doc.decor_du_plan(0).unwrap().objets.len(), 1, "rien n'est posé avant le choix");
    // Deux au plus : la séquence est pleine, le second est refusé sans fenêtre.
    let mut b = banc_avec_budget(BudgetMachine { triangles: 2, ..budget_large() });
    b.app.decor3d_importer_chemin(f.clone());
    attendre(&mut b);
    b.app.decor3d_importer_chemin(f.clone());
    attendre(&mut b);
    assert!(!b.app.decor3d.fenetre_ouverte());
    assert!(b.app.status.contains("refusé") && b.app.status.contains("enlève"), "{}", b.app.status);
    assert_eq!(b.app.doc.decor_du_plan(0).unwrap().objets.len(), 1);
    std::fs::remove_dir_all(&dir).ok();
}

/// Un projet de 160 × 90 dont le décor a deux carrés, l'un à gauche, l'autre à droite du cadre.
pub(crate) fn projet_a_deux_carres() -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    let seq = doc.sequence_of(0);
    let monde = Monde::of(&doc);
    // (La largeur du cadre en mètres : le sol est à une demi-hauteur de cadre sous le centre.)
    let k = -2.0 * monde.sol_y() / doc.frame[1] as f32;
    let largeur = doc.frame[0] as f32 * k;
    let mdl = modele_du_carre();
    let mut d = Decor3d::new(Aspect { ciel: false, brume: false, ombres: false, ..Aspect::default() });
    for x in [-largeur / 4.0, largeur / 4.0] {
        let o = objet_du_carre(&mut doc, Place3d { position: [x, 0.0, -0.001], rotation_y: 0.0, echelle: largeur / 10.0 }, mdl.clone());
        d.objets.push(o);
    }
    doc.decors3d.par_sequence.insert(seq, d);
    doc
}

#[test]
fn l_image_d_un_plan_montre_tous_les_objets_du_decor() {
    let doc = projet_a_deux_carres();
    let img = image_exportee(&doc);
    let [w, h] = doc.frame;
    assert_eq!(img[(h / 2) * w + w / 4].a(), 255, "le carré de gauche");
    assert_eq!(img[(h / 2) * w + 3 * w / 4].a(), 255, "le carré de droite");
    assert_eq!(img[(h / 2) * w + w / 2].a(), 0, "rien entre eux");
    // Un seul élément de décor dans le plan, pour les deux.
    let plan = crate::render::panel_plan_at(&doc, 0, 0, None);
    assert_eq!(plan.items.iter().filter(|it| matches!(it.fond, Some(crate::render::FondPart::Decor(_)))).count(), 1);
    // Un objet qui bouge fait une autre clé (le rendu gardé ne sert plus) ; la même texture de la carte.
    let Some(crate::render::FondPart::Decor(k1)) = plan.items[0].fond.clone() else { panic!() };
    let mut doc2 = doc;
    let seq = doc2.sequence_of(0);
    doc2.decors3d.par_sequence.get_mut(&seq).unwrap().objets[0].place.position[0] += 0.1;
    let Some(crate::render::FondPart::Decor(k2)) = crate::render::panel_plan_at(&doc2, 0, 0, None).items[0].fond.clone() else { panic!() };
    assert_ne!(decor3d::id_of(&k1), decor3d::id_of(&k2));
    assert_eq!(k1.decor, k2.decor);
    assert!(decor3d::objets_de(&doc2, &k1).is_none(), "une clé d'avant ne vaut plus");
    assert_eq!(decor3d::objets_de(&doc2, &k2).unwrap().len(), 2);
}

#[test]
fn deux_objets_d_un_meme_modele_sur_la_carte_graphique() {
    let Some(rs) = crate::gpu_tests::headless() else { return };
    let doc = projet_a_deux_carres();
    let plan = crate::render::panel_plan_at(&doc, 0, 0, None);
    let Some(crate::render::FondPart::Decor(key)) = plan.items[0].fond.clone() else { panic!() };
    let objets = decor3d::objets_de(&doc, &key).unwrap();
    let d = decor3d::dessins_gpu(&objets, key.aspect);
    assert_eq!(d[0].id, d[1].id, "un modèle partagé : un seul envoi à la carte");
    let mut mt = crate::decor3dgpu::Moteur::new(&rs.device, &rs.queue);
    let [w, h] = key.feuille;
    let (rgba, _) = crate::decor3dgpu::rendre_hors_ecran(&rs.device, &rs.queue, &mut mt, &d, &key.prise().unwrap(), [w as u32, h as u32], false, Qualite::Sans, 1).unwrap();
    let a = |x: usize, y: usize| rgba[(y * w + x) * 4 + 3];
    // (Le cadre est au centre de la feuille, deux fois plus petite.)
    let (cx, cy, fw) = (w / 2, h / 2, doc.frame[0]);
    assert!(a(cx - fw / 4, cy) > 200 && a(cx + fw / 4, cy) > 200, "les deux carrés");
    assert_eq!(a(cx, cy), 0);
    // Dans la carte : le poids d'un seul modèle (envoyé une fois pour les deux objets).
    let mut seul = crate::decor3dgpu::Moteur::new(&rs.device, &rs.queue);
    crate::decor3dgpu::rendre_hors_ecran(&rs.device, &rs.queue, &mut seul, &d[..1], &key.prise().unwrap(), [w as u32, h as u32], false, Qualite::Sans, 1).unwrap();
    assert_eq!(mt.octets(), seul.octets());
}

// ------------------------------------------------------------------------------------------------ l'origine

#[test]
fn rotation_et_echelle_tournent_autour_de_l_origine() {
    let mdl = modele_du_carre();
    let mut o = Objet3d::neuf(1, "Carré".into(), String::new(), Vec::new(), mdl);
    o.origine = [0.5, -1.0, 0.0];
    for (rot, e) in [(0.0, 1.0), (37.0, 2.0), (90.0, 0.5), (-120.0, 3.0)] {
        o.place = Place3d { position: [1.0, 2.0, -3.0], rotation_y: rot, echelle: e };
        let p = o.au_monde(o.origine);
        assert!(crate::espace3dmath::length(crate::espace3dmath::sub(p, o.place.position)) < 1e-5, "l'origine reste à sa position ({rot}°, ×{e}) : {p:?}");
        let q = o.au_monde(crate::espace3dmath::add(o.origine, [1.0, 0.0, 0.0]));
        assert!((crate::espace3dmath::length(crate::espace3dmath::sub(q, p)) - e).abs() < 1e-4, "un mètre du modèle, à l'échelle, depuis l'origine");
    }
}

#[test]
fn changer_l_origine_ne_bouge_pas_l_objet() {
    let mdl = modele_du_carre();
    let mut o = Objet3d::neuf(1, "Carré".into(), String::new(), Vec::new(), mdl);
    o.place = Place3d { position: [0.4, -0.2, -2.0], rotation_y: 33.0, echelle: 1.7 };
    let coins = |o: &Objet3d| o.modele.scene.primitives[0].positions.iter().map(|p| o.au_monde(*p)).collect::<Vec<_>>();
    let avant = coins(&o);
    for nouvelle in [[1.0, 1.0, 0.0], [0.0, 0.0, 0.0], [-1.0, -1.0, 0.0], [0.3, 7.0, -2.0]] {
        o.changer_origine(nouvelle);
        assert_eq!(o.origine, nouvelle);
        for (a, b) in avant.iter().zip(coins(&o)) {
            assert!(crate::espace3dmath::length(crate::espace3dmath::sub(*a, b)) < 1e-4, "{a:?} → {b:?}");
        }
    }
    // Et la rotation se fait maintenant autour du nouveau point.
    o.changer_origine([1.0, 1.0, 0.0]);
    let pivot = o.au_monde([1.0, 1.0, 0.0]);
    o.place.rotation_y += 90.0;
    assert!(crate::espace3dmath::length(crate::espace3dmath::sub(o.au_monde([1.0, 1.0, 0.0]), pivot)) < 1e-4);
}

// --------------------------------------------------------------------------------------------- l'aimant

/// Une boîte pleine de `a` à `b` (six faces).
pub(crate) fn boite(a: [f32; 3], b: [f32; 3]) -> Scene3d {
    let mut p = Primitive3d { couleur: [0.7, 0.7, 0.7, 1.0], ..Default::default() };
    for f in 0..6 {
        let (axe, cote) = (f / 2, f % 2);
        let base = p.positions.len() as u32;
        let mut n = [0.0; 3];
        n[axe] = if cote == 0 { -1.0 } else { 1.0 };
        let (u, v) = ((axe + 1) % 3, (axe + 2) % 3);
        for (du, dv) in [(0, 0), (1, 0), (1, 1), (0, 1)] {
            let mut q = [0.0; 3];
            q[axe] = if cote == 0 { a[axe] } else { b[axe] };
            q[u] = if du == 0 { a[u] } else { b[u] };
            q[v] = if dv == 0 { a[v] } else { b[v] };
            p.positions.push(q);
            p.normales.push(n);
        }
        p.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    Scene3d { primitives: vec![p], textures: Vec::new() }
}

pub(crate) fn objet(id: u64, scene: Scene3d, x: f32, z: f32) -> Objet3d {
    // (Un vrai .glb : l'objet s'enregistre et se relit.)
    let glb = gltf::ecrire_glb(&scene, gltf::Ecriture { cote_max: None }).unwrap();
    let mut o = Objet3d::neuf(id, format!("objet {id}"), String::new(), Vec::new(), Arc::new(Modele3d::new(Arc::new(scene), Arc::new(glb))));
    o.place.position = [x, 0.0, z];
    o
}

fn bas(o: &Objet3d) -> f32 {
    o.modele.scene.primitives.iter().flat_map(|p| &p.positions).map(|p| o.au_monde(*p)[1]).fold(f32::MAX, f32::min)
}

const SOL: f32 = -1.25;

#[test]
fn aimante_son_plus_bas_sommet_se_pose_sur_le_sol() {
    let mut d = Decor3d::new(Aspect::default());
    let mut o = objet(1, boite([-0.5, 2.0, -0.5], [0.5, 3.0, 0.5]), 0.0, -2.0);
    // L'origine loin au-dessus du modèle, l'objet en l'air.
    o.origine = [0.0, 5.0, 0.0];
    o.place.position[1] = 4.0;
    d.objets.push(o);
    crate::decor3daimant::aimanter(&mut d, SOL);
    assert!((bas(&d.objets[0]) - SOL).abs() < 1e-3, "{}", bas(&d.objets[0]));
}

#[test]
fn un_objet_tourne_et_penche_n_entre_jamais_dans_le_sol() {
    // Une boîte penchée dans son propre modèle (20° autour de x), puis tournée de 30° autour de la verticale et agrandie.
    let mut s = boite([-0.6, 0.0, -0.3], [0.6, 0.8, 0.3]);
    let r = crate::espace3dmath::rot_x(20.0);
    for p in &mut s.primitives[0].positions {
        *p = crate::espace3dmath::point(&r, *p);
    }
    let mut d = Decor3d::new(Aspect::default());
    let mut o = objet(1, s, 0.3, -2.0);
    o.place.rotation_y = 30.0;
    o.place.echelle = 1.7;
    o.place.position[1] = -3.0;
    d.objets.push(o);
    crate::decor3daimant::aimanter(&mut d, SOL);
    let b = bas(&d.objets[0]);
    assert!(b >= SOL - 1e-3 && b <= SOL + 1e-3, "il touche le sol sans y entrer : {b}");
}

#[test]
fn un_objet_pose_sur_un_autre_repose_sur_son_dessus() {
    let mut d = Decor3d::new(Aspect::default());
    d.objets.push(objet(2, boite([-0.25, 0.0, -0.25], [0.25, 0.5, 0.25]), 0.1, -2.0));
    d.objets.push(objet(1, boite([-1.0, 0.0, -1.0], [1.0, 1.0, 1.0]), 0.0, -2.0));
    crate::decor3daimant::aimanter(&mut d, SOL);
    let (petit, grand) = (&d.objets[0], &d.objets[1]);
    assert!((bas(grand) - SOL).abs() < 1e-3, "le grand au sol");
    assert!((bas(petit) - (SOL + 1.0)).abs() < 1e-3, "le petit sur le grand : {}", bas(petit));
    // Un banc qui dépasse du bord d'une dalle : toujours dessus.
    let mut d2 = Decor3d::new(Aspect::default());
    d2.objets.push(objet(1, boite([-1.0, 0.0, -1.0], [1.0, 0.1, 1.0]), 0.0, -2.0));
    d2.objets.push(objet(2, boite([-0.6, 0.0, -0.2], [0.6, 0.5, 0.2]), 1.2, -2.0));
    crate::decor3daimant::aimanter(&mut d2, SOL);
    assert!((bas(&d2.objets[1]) - (SOL + 0.1)).abs() < 1e-3, "{}", bas(&d2.objets[1]));
}

#[test]
fn l_objet_du_dessous_qui_bouge_fait_suivre_celui_du_dessus() {
    let mut d = Decor3d::new(Aspect::default());
    d.objets.push(objet(1, boite([-1.0, 0.0, -1.0], [1.0, 1.0, 1.0]), 0.0, -2.0));
    d.objets.push(objet(2, boite([-0.25, 0.0, -0.25], [0.25, 0.5, 0.25]), 0.0, -2.0));
    crate::decor3daimant::aimanter(&mut d, SOL);
    assert!((bas(&d.objets[1]) - (SOL + 1.0)).abs() < 1e-3);
    // Le dessous, plus haut (il n'est plus aimanté : posé sur une marche à 0,5 m) : le dessus monte avec lui.
    d.objets[0].aimante = false;
    d.objets[0].place.position[1] = SOL + 0.5;
    crate::decor3daimant::aimanter(&mut d, SOL);
    assert!((bas(&d.objets[1]) - (SOL + 1.5)).abs() < 1e-3, "{}", bas(&d.objets[1]));
    // Le dessous, ailleurs : le dessus redescend au sol.
    d.objets[0].place.position[0] = 6.0;
    crate::decor3daimant::aimanter(&mut d, SOL);
    assert!((bas(&d.objets[1]) - SOL).abs() < 1e-3, "{}", bas(&d.objets[1]));
}

#[test]
fn decoche_l_objet_garde_sa_hauteur() {
    let mut d = Decor3d::new(Aspect::default());
    d.objets.push(objet(1, boite([-1.0, 0.0, -1.0], [1.0, 1.0, 1.0]), 0.0, -2.0));
    d.objets.push(objet(2, boite([-0.25, 0.0, -0.25], [0.25, 0.5, 0.25]), 0.0, -2.0));
    crate::decor3daimant::aimanter(&mut d, SOL);
    let haut = d.objets[1].place.position;
    d.objets[1].aimante = false;
    d.objets[0].place.position[0] = 6.0;
    crate::decor3daimant::aimanter(&mut d, SOL);
    assert_eq!(d.objets[1].place.position, haut, "décoché : il reste où il était");
}

#[test]
fn deux_objets_au_meme_endroit_ne_font_pas_de_boucle() {
    let mut d = Decor3d::new(Aspect::default());
    d.objets.push(objet(1, boite([-0.5, 0.0, -0.5], [0.5, 1.0, 0.5]), 0.0, -2.0));
    d.objets.push(objet(2, boite([-0.5, 0.0, -0.5], [0.5, 1.0, 0.5]), 0.0, -2.0));
    crate::decor3daimant::aimanter(&mut d, SOL);
    let mut bas_ = [bas(&d.objets[0]), bas(&d.objets[1])];
    bas_.sort_by(f32::total_cmp);
    assert!((bas_[0] - SOL).abs() < 1e-3 && (bas_[1] - (SOL + 1.0)).abs() < 1e-3, "l'un sur l'autre : {bas_:?}");
    assert!((bas(&d.objets[0]) - SOL).abs() < 1e-3, "le premier de la liste en dessous (ordre fixe)");
    // Recalculé : la même chose (pas d'aller-retour).
    let avant: Vec<_> = d.objets.iter().map(|o| o.place.position).collect();
    for _ in 0..3 {
        crate::decor3daimant::aimanter(&mut d, SOL);
        assert_eq!(d.objets.iter().map(|o| o.place.position).collect::<Vec<_>>(), avant);
    }
}

#[test]
fn un_objet_garde_sa_place_sous_une_ramure() {
    // Un « arbre » : un tronc fin et une ramure de 1,5 à 3 m ; un feu de camp glissé dessous reste au sol.
    let mut arbre = boite([-0.1, 0.0, -0.1], [0.1, 1.5, 0.1]);
    arbre.primitives.extend(boite([-1.0, 1.5, -1.0], [1.0, 3.0, 1.0]).primitives);
    let mut d = Decor3d::new(Aspect::default());
    d.objets.push(objet(1, arbre, 0.0, -2.0));
    d.objets.push(objet(2, boite([-0.2, 0.0, -0.2], [0.2, 0.3, 0.2]), 0.6, -2.0));
    crate::decor3daimant::aimanter(&mut d, SOL);
    assert!((bas(&d.objets[1]) - SOL).abs() < 1e-3, "sous la ramure, au sol : {}", bas(&d.objets[1]));
}

#[test]
fn un_tres_gros_objet_sert_d_appui_par_sa_boite_et_le_dit() {
    let mut gros = grille_dense(300);
    for p in gros.primitives.iter_mut().flat_map(|p| p.positions.iter_mut()) {
        p[1] = p[1].abs();
    }
    assert!(gros.triangles() > crate::decor3daimant::TRIANGLES_EXACTS);
    let mut d = Decor3d::new(Aspect::default());
    let mut g = objet(1, gros, 0.0, -2.0);
    g.aimante = false;
    g.place.position[1] = SOL;
    d.objets.push(g);
    d.objets.push(objet(2, boite([-0.2, 0.0, -0.2], [0.2, 0.3, 0.2]), 0.0, -2.0));
    let debut = std::time::Instant::now();
    let notes = crate::decor3daimant::aimanter(&mut d, SOL);
    assert!(debut.elapsed() < std::time::Duration::from_millis(100), "{:?}", debut.elapsed());
    assert!(notes.iter().any(|n| n.contains("boîte")), "{notes:?}");
    let (_, hi) = d.objets[0].bornes_monde();
    assert!((bas(&d.objets[1]) - hi[1]).abs() < 1e-3, "sur le dessus de sa boîte");
}

#[test]
fn decocher_aimante_laisse_l_objet_ou_l_aimant_l_avait_mis_meme_apres_reouverture() {
    let mut b = banc_avec_budget(budget_large());
    let seq = b.app.doc.sequence_of(0);
    let sol = Monde::of(&b.app.doc).sol_y();
    let id = b.app.doc.alloc();
    let mut o = objet(id, boite([-0.5, 0.0, -0.5], [0.5, 1.0, 0.5]), 0.0, -2.0);
    o.aimante = false;
    o.place.position[1] = sol + 3.0;
    let mut d = Decor3d::new(Aspect::default());
    d.objets.push(o);
    b.app.doc.decors3d.par_sequence.insert(seq, d);
    let etapes = b.app.history.undo_len();
    // Coché (une retouche du panneau) : il repose sur le sol ; une étape d'annulation.
    let mut d = b.app.doc.decor_du_plan(0).unwrap().clone();
    d.objets[0].aimante = true;
    b.app.decor3d_retoucher(seq, d);
    b.app.decor3d_retouche_finir();
    let pose = b.app.doc.decor_du_plan(0).unwrap().objets[0].place;
    assert!((bas(&b.app.doc.decor_du_plan(0).unwrap().objets[0]) - sol).abs() < 1e-3, "sur le sol");
    assert!((pose.position[1] - sol).abs() < 1e-4, "la hauteur calculée est celle gardée (le champ Y la montre)");
    assert_eq!(b.app.history.undo_len(), etapes + 1);
    // Décoché : il ne bouge pas, ni vers la hauteur d'avant l'aimant ; une étape d'annulation.
    let mut d = b.app.doc.decor_du_plan(0).unwrap().clone();
    d.objets[0].aimante = false;
    b.app.decor3d_retoucher(seq, d);
    b.app.decor3d_retouche_finir();
    let o = b.app.doc.decor_du_plan(0).unwrap().objets[0].clone();
    assert!(!o.aimante);
    assert_eq!(o.place, pose, "décoché : exactement où l'aimant l'avait mis");
    assert_eq!(b.app.history.undo_len(), etapes + 2, "décocher : une seule étape");
    // Enregistré et relu : toujours là.
    let dir = dossier("decoche");
    let manifeste = dir.join("Film.shot");
    crate::project::save(&b.app.doc, &manifeste, &mut Default::default()).unwrap();
    let relu = crate::project::load(&manifeste).unwrap();
    let r = &relu.doc.decor_du_plan(0).unwrap().objets[0];
    assert!(!r.aimante);
    assert_eq!(r.place, pose, "relu : la même place");
    // Annuler le décochage le rend aimanté, à la même place.
    b.app.undo();
    let o = &b.app.doc.decor_du_plan(0).unwrap().objets[0];
    assert!(o.aimante && o.place == pose);
    std::fs::remove_dir_all(&dir).ok();
}
