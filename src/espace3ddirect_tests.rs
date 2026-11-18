//! Essais des parties 158 et 159 (`docs/INTERFACE.md`) : le maillage du sol reste visible et juste quand la vue est tout
//! près de la scène (158) ; **tous les plans sont dans l'espace** au format 56, sans qu'aucune image d'un projet d'avant ne
//! change d'un pixel, et la 3D ne se sent jamais hors de la vue de la scène (159, `espace3ddirect.rs`).

use std::sync::Arc;

use eframe::egui::{pos2, Color32, Pos2, Rect};

use crate::camera::{Camera, CameraKey};
use crate::document::{Document, Layer, LayerKind, LayerTransform};
use crate::espace3ddirect::dit_la_3d;
use crate::espace3dmath::{Orbite, View};
use crate::espace3dview::{grid_lines_pas, traits_a_l_ecran, GridLine};
use crate::exportrender::{self, Papier, Rendu};
use crate::perf_tests::{banc_taille, Banc};
use crate::poskeys::PosKey;
use crate::raster::TILE;
use crate::render;

const ECRAN: [f32; 2] = [1280.0, 720.0];

fn vue(distance: f32) -> View {
    Orbite { target: [0.0; 3], azimuth: 30.0, elevation: 22.0, distance, focal: 35.0 }.view(ECRAN[0])
}

fn traits(view: &View, lignes: &[GridLine]) -> Vec<(Pos2, Pos2, [f32; 4])> {
    let zone = Rect::from_min_max(pos2(0.0, 0.0), pos2(ECRAN[0], ECRAN[1]));
    traits_a_l_ecran(view, [ECRAN[0] / 2.0, ECRAN[1] / 2.0], pos2(0.0, 0.0), zone, &lignes)
}

/// Le point du sol (y = 0) sous ce point de l'écran.
fn au_sol(view: &View, p: Pos2) -> Option<[f32; 3]> {
    let r = view.ray([p.x, p.y], [ECRAN[0] / 2.0, ECRAN[1] / 2.0]);
    (r.d[1].abs() > 1e-6).then(|| -r.o[1] / r.d[1]).filter(|t| *t > 0.0).map(|t| [r.o[0] + r.d[0] * t, 0.0, r.o[2] + r.d[2] * t])
}

/// Le cas d'Anthony : la vue tout près de la scène, le maillage au pas fixe de 0,60 m. Le sol devant la vue (le bas de
/// l'image) doit être quadrillé, et chaque trait montré doit être sur le sol, sur la ligne de son trait du monde.
#[test]
fn le_maillage_couvre_le_sol_devant_une_vue_tout_pres() {
    for (distance, pas) in [(1.5, Some(0.6)), (0.6, Some(0.15)), (1.5, None), (0.3, None)] {
        let view = vue(distance);
        let lignes = grid_lines_pas([0.0; 3], distance, pas);
        let montres = traits(&view, &lignes);
        // Le bas de l'image (le sol juste devant la vue, entre elle et ce qu'elle regarde) : des traits le traversent.
        let dans = |haut: f32| {
            let r = Rect::from_min_max(pos2(0.0, ECRAN[1] * haut), pos2(ECRAN[0], ECRAN[1]));
            montres.iter().filter(|(a, b, _)| crate::guides::clip_line(*a, *b - *a, r, true).is_some_and(|(p, q)| (q - p).length() > 2.0)).count()
        };
        assert!(dans(0.75) >= 2 && dans(0.5) >= 4, "distance {distance}, pas {pas:?} : {} traits dans le quart du bas, {} dans la moitié du bas, sur {} montrés", dans(0.75), dans(0.5), montres.len());
        // Juste : chaque trait montré est sur le sol, le long d'une ligne du quadrillage (x ou z constant, au pas).
        let pas_reel = pas.unwrap_or_else(|| (lignes[2].a[2] - lignes[0].a[2]).abs());
        for (a, b, _) in &montres {
            for p in [*a, *b, *a + (*b - *a) * 0.5] {
                // (Près de l'horizon, retrouver le point du sol sous un pixel n'est plus précis : on juge le sol proche.)
                let Some(s) = au_sol(&view, p).filter(|s| (s[0] * s[0] + s[2] * s[2]).sqrt() < 12.0) else { continue };
                let sur = |v: f32| ((v / pas_reel) - (v / pas_reel).round()).abs() * pas_reel < 0.02 * pas_reel.max(0.05) + 1e-3 * distance;
                assert!(sur(s[0]) || sur(s[2]), "distance {distance} : le point {p:?} de l'écran tombe au sol en {s:?}, hors des lignes du pas {pas_reel}");
            }
        }
    }
}

/// Un trait tout entier devant la vue (vue de loin) est montré exactement comme avant : mêmes bouts à l'écran.
#[test]
fn de_loin_les_traits_sont_les_memes_qu_avant() {
    let view = vue(40.0);
    let lignes = grid_lines_pas([0.0; 3], 40.0, None);
    let montres = traits(&view, &lignes);
    let c = [ECRAN[0] / 2.0, ECRAN[1] / 2.0];
    for l in &lignes {
        if let (Some(a), Some(b)) = (view.project(l.a, c), view.project(l.b, c)) {
            let (a, b) = (pos2(a[0], a[1]), pos2(b[0], b[1]));
            let zone = Rect::from_min_max(pos2(0.0, 0.0), pos2(ECRAN[0], ECRAN[1]));
            if zone.contains(a) && zone.contains(b) {
                assert!(montres.iter().any(|(p, q, _)| p.distance(a) < 1e-3 && q.distance(b) < 1e-3), "le trait {a:?}–{b:?} est montré tel quel");
            }
        }
    }
}

// ------------------------------------------------------------------------------- partie 159 : tous les plans dans l'espace

const ROUGE: Color32 = Color32::from_rgb(220, 40, 40);
const VERT: Color32 = Color32::from_rgb(40, 200, 60);
const BLEU: Color32 = Color32::from_rgb(40, 80, 230);
const JAUNE: Color32 = Color32::from_rgb(230, 210, 40);

fn remplir(doc: &mut Document, cel: crate::document::CelId, r: [usize; 4], color: Color32) {
    let raster = &mut doc.cels.get_mut(&cel).expect("dessin").raster;
    for y in r[1]..r[3] {
        for x in r[0]..r[2] {
            let idx = raster.tile_index_of(x, y);
            raster.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = color;
        }
    }
}

/// Un calque bitmap peint d'un rectangle, tenu 12 images, à la profondeur `depth`.
fn calque(doc: &mut Document, nom: &str, color: Color32, r: [usize; 4], depth: f32) -> Layer {
    let mut layer = Layer::new(nom, LayerKind::Bitmap);
    let cel = doc.new_cel(LayerKind::Bitmap);
    remplir(doc, cel, r, color);
    layer.frames = vec![Some(cel); 12];
    layer.depth = depth;
    layer
}

/// Un plan ordinaire de storyboard : cadre 320 x 180 sur une feuille 640 x 360, trois calques à des profondeurs
/// différentes, dans l'ordre de la pile (le plus loin en bas), qui se recouvrent ; celui du dessus glisse (clés de position).
fn plan_ordinaire() -> Document {
    let mut doc = Document::with_frame(320, 180, 24);
    doc.panels[0].layers = Vec::new();
    for (nom, color, r, depth) in [("Fond", ROUGE, [180, 100, 420, 260], 1.5), ("Milieu", VERT, [240, 130, 400, 230], 0.5), ("Devant", BLEU, [300, 150, 360, 250], 0.0)] {
        let l = calque(&mut doc, nom, color, r, depth);
        doc.panels[0].layers.push(l);
    }
    doc.panels[0].layers[2].pos_keys = vec![PosKey::new(0, LayerTransform::default()), PosKey::new(11, LayerTransform { x: 40.0, y: -10.0, ..LayerTransform::default() })];
    doc
}

/// **Le projet d'un storyboarder d'avant le format 56**, tout ce qu'un plan ordinaire du format 55 savait tenir, dans trois
/// plans :
///
/// - plan 0 : une caméra animée dans les anciennes limites (glissé, zoom, rotation, avancée, bascule et panoramique), la
///   pile dans le désordre des profondeurs (le plus proche en bas), un calque penché, et une clé de profondeur au-delà de la
///   borne qu'on ne peut ramener à la borne sans changer l'image entre deux clés ;
/// - plan 1 : sans clé de caméra, un calque dont les deux clés sont au-delà de la borne
///   (ramenées sans rien changer) et un calque posé juste à la borne ;
/// - plan 2 : un mouvement de caméra des Vignettes (un cadre A au début, un cadre B à la fin, sur deux cases).
fn projet_d_avant() -> Document {
    let mut doc = plan_ordinaire();
    let d = doc.default_camera();
    // Plan 0.
    doc.panels[0].layers.swap(0, 2);
    doc.panels[0].layers[1].transform.tilt_x = 30.0;
    doc.panels[0].layers[0].pos_keys = vec![PosKey::new(0, LayerTransform::default()), PosKey::new(11, LayerTransform { x: -30.0, dz: -2.0, ..LayerTransform::default() })];
    doc.panels[0].camera = vec![
        CameraKey::new(0, Camera { x: d.x - 20.0, zoom: 1.1, ..d }),
        CameraKey::new(6, Camera { x: d.x + 25.0, y: d.y - 10.0, zoom: 1.3, angle: 6.0, dolly: 40.0, tilt: 9.0, pan: -7.0 }),
        CameraKey::new(11, Camera { x: d.x + 40.0, zoom: 1.0, angle: -4.0, ..d }),
    ];
    // Plan 1.
    let i1 = doc.add_scene_after(0);
    doc.panels[i1].layers = Vec::new();
    for (nom, color, r, depth) in [("Ciel peint", JAUNE, [100, 60, 540, 300], 2.0), ("Perso", VERT, [260, 120, 380, 260], 0.0), ("Herbe", ROUGE, [200, 220, 440, 300], -0.9)] {
        let l = calque(&mut doc, nom, color, r, depth);
        doc.panels[i1].layers.push(l);
    }
    doc.panels[i1].layers[1].pos_keys = vec![PosKey::new(0, LayerTransform { dz: -1.4, ..LayerTransform::default() }), PosKey::new(11, LayerTransform { x: 20.0, dz: -1.6, ..LayerTransform::default() })];
    // Plan 2 : deux cases (une marque à l'image 6) et le mouvement des Vignettes de la première.
    let i2 = doc.add_scene_after(i1);
    doc.panels[i2].layers = Vec::new();
    for (nom, color, r, depth) in [("Décor", BLEU, [120, 80, 520, 290], 0.8), ("Perso", ROUGE, [280, 140, 340, 240], 0.0)] {
        let l = calque(&mut doc, nom, color, r, depth);
        doc.panels[i2].layers.push(l);
    }
    doc.panels[i2].length = 12;
    doc.panels[i2].marks = vec![crate::document::ImageMark { frame: 6, ..Default::default() }];
    doc.panels[i2].camera = vec![CameraKey::new(0, Camera { x: d.x - 60.0, zoom: 1.4, ..d }), CameraKey::new(5, Camera { x: d.x + 50.0, y: d.y + 20.0, zoom: 1.2, ..d })];
    for p in &mut doc.panels {
        p.length = 12;
    }
    doc
}

/// Ce qu'une version du format 55 montrait de `doc` : un plan ordinaire y tenait toutes ses profondeurs à la borne et ne
/// passait jamais par le changement de repère. Le code d'aujourd'hui le refait exactement en tenant chaque calque des plans
/// ordinaires à la borne (`Layer::profondeur_bornee`) : c'est la seule chose que le format 56 lit autrement pour eux.
fn comme_au_format_55(mut d: Document) -> Document {
    for p in d.panels.iter_mut().filter(|p| !p.espace) {
        for l in &mut p.layers {
            l.profondeur_bornee = true;
        }
    }
    d
}

/// L'image filmée de chaque image de chaque plan, par le chemin de toutes les sorties.
fn filmees(doc: &Document) -> Vec<Vec<Vec<Color32>>> {
    (0..doc.panels.len())
        .map(|p| {
            (0..doc.panels[p].frame_count())
                .map(|f| {
                    let plan = render::panel_plan_at(doc, p, f, doc.render_paper());
                    render::camera_image(doc, &plan, &doc.panel_camera(p, f))
                })
                .collect()
        })
        .collect()
}

/// Les images de l'export (le même rendu que l'export d'images et de vidéos, `exportrender::Rendu`).
fn exportees(doc: &Document) -> Vec<Vec<Vec<Color32>>> {
    let rendu = Rendu::new(exportrender::snapshot_of(doc, &(0..doc.panels.len()).collect::<Vec<_>>()), Papier::Fond, false, Vec::new(), Arc::default());
    (0..doc.panels.len()).map(|p| (0..doc.panels[p].frame_count()).map(|f| rendu.image(p, f).expect("image").to_vec()).collect()).collect()
}

fn ecarts(a: &[Vec<Vec<Color32>>], b: &[Vec<Vec<Color32>>]) -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    for (p, (pa, pb)) in a.iter().zip(b).enumerate() {
        assert_eq!(pa.len(), pb.len(), "plan {p} : autant d'images");
        for (f, (ia, ib)) in pa.iter().zip(pb).enumerate() {
            let n = ia.iter().zip(ib).filter(|(x, y)| x != y).count();
            if n > 0 || ia.len() != ib.len() {
                out.push((p, f, n));
            }
        }
    }
    out
}

/// Un dossier d'essai neuf.
fn dossier(nom: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-159-{nom}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dossier");
    dir
}

/// Enregistre `doc` comme l'aurait fait une version du format 55 : le même fichier, avec son numéro (aucun des champs du
/// format 56 n'est écrit pour un plan ordinaire sans calque tenu à la borne, ni pour un plan venu du format 55).
fn enregistrer_au_format_55(doc: &Document, dir: &std::path::Path) -> std::path::PathBuf {
    let manifeste = dir.join("P.shot");
    crate::project::save(doc, &manifeste, &mut Default::default()).expect("enregistre");
    let texte = std::fs::read_to_string(&manifeste).expect("relu");
    for champ in ["camera_libre", "ordre_distance", "profondeur_bornee"] {
        assert!(!texte.contains(champ), "un projet d'avant n'a pas « {champ} »");
    }
    let ancien = texte.replace(&format!("\"version\": {}", crate::project::VERSION), "\"version\": 55");
    assert_ne!(ancien, texte, "le numéro du format est dans le manifeste");
    std::fs::write(&manifeste, ancien).expect("écrit");
    manifeste
}

#[test]
fn un_projet_du_format_55_s_ouvre_au_format_56_sans_question_et_avec_les_memes_images_au_pixel_pres() {
    assert_eq!(crate::project::VERSION, 59, "partie 159 : le format 56, puis 57 (partie 170), puis 58 (partie 171), puis 59 (partie 172)");
    let doc = projet_d_avant();
    let au_55 = comme_au_format_55(projet_d_avant());
    let avant = filmees(&au_55);
    let avant_export = exportees(&au_55);
    assert!(ecarts(&avant, &avant_export).is_empty(), "l'export compose les mêmes images que l'écran");
    // (Rouge sans la migration : lu tel quel au format 56, le calque dont la clé passe la borne irait à sa vraie place.)
    let sans_migration = ecarts(&avant, &filmees(&doc));
    assert!(!sans_migration.is_empty(), "sans la migration, les clés au-delà de la borne changeraient l'image");
    let dir = dossier("55-vers-56");
    let manifeste = enregistrer_au_format_55(&doc, &dir);
    let lu = crate::project::load(&manifeste).expect("un projet 55 s'ouvre");
    assert!(lu.warnings.is_empty(), "aucune question, aucun avertissement : {:?}", lu.warnings);
    // Les mêmes images, au pixel près, à toutes les images de tous les plans, à l'écran et à l'export.
    assert_eq!(ecarts(&avant, &filmees(&lu.doc)), Vec::<(usize, usize, usize)>::new(), "les images filmées après la migration");
    assert_eq!(ecarts(&avant, &exportees(&lu.doc)), Vec::<(usize, usize, usize)>::new(), "les images exportées après la migration");
    // Ce que la migration a fait : un calque tenu à la borne (plan 0), un calque ramené à la borne (plan 1), rien d'autre.
    let tenus: Vec<(usize, String)> = lu.doc.panels.iter().enumerate().flat_map(|(p, pl)| pl.layers.iter().filter(|l| l.profondeur_bornee).map(move |l| (p, l.name.clone()))).collect();
    assert_eq!(tenus, vec![(0, "Devant".to_string())], "seul le calque qu'on ne pouvait ramener est tenu à la borne");
    let ramene = &lu.doc.panels[1].layers[1];
    assert!(ramene.pos_keys.iter().all(|k| ramene.depth + k.t.dz == crate::document::Layer::MIN_DEPTH), "clés ramenées à la borne : {:?}", ramene.pos_keys);
    assert!(lu.doc.panels.iter().all(|p| !p.espace && !p.camera_libre && !p.ordre_distance), "ordre de la pile et caméra d'avant");
    // Enregistré : au format 56, et l'aller-retour garde tout.
    crate::project::save(&lu.doc, &manifeste, &mut Default::default()).expect("enregistre");
    let texte = std::fs::read_to_string(&manifeste).expect("relu");
    assert!(texte.contains("\"version\": 59") && texte.contains("\"profondeur_bornee\": true"), "le format 56 et le calque tenu");
    let relu = crate::project::load(&manifeste).expect("relu au format 56");
    assert_eq!(ecarts(&avant, &filmees(&relu.doc)), Vec::<(usize, usize, usize)>::new(), "aller-retour au format 56");
    assert!(relu.doc.panels[0].layers.iter().any(|l| l.profondeur_bornee));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn le_projet_d_essai_complet_du_depot_s_ouvre_au_format_56_avec_les_memes_images() {
    let (f, manifeste) = crate::fiabilite_fixture::projet_enregistre("159-complet");
    // (Les dessins vectoriels à texte dépendent des polices lues en arrière-plan : comparés sans eux, comme la fiabilité.)
    let sans_texte = |mut d: Document| {
        for p in &mut d.panels {
            for l in &mut p.layers {
                for c in l.frames.iter_mut() {
                    if c.is_some_and(|id| f.sans_pixels.contains(&id)) {
                        *c = None;
                    }
                }
            }
        }
        d
    };
    // (Le projet relu tel quel, puis montré comme le format 55 le montrait.)
    let avant = filmees(&comme_au_format_55(sans_texte(crate::project::load(&manifeste).expect("s'ouvre").doc)));
    let texte = std::fs::read_to_string(&manifeste).expect("relu");
    std::fs::write(&manifeste, texte.replace(&format!("\"version\": {}", crate::project::VERSION), "\"version\": 55")).expect("écrit");
    let lu = crate::project::load(&manifeste).expect("s'ouvre");
    assert!(lu.warnings.iter().all(|w| !w.contains("format")), "{:?}", lu.warnings);
    assert_eq!(ecarts(&avant, &filmees(&sans_texte(lu.doc))), Vec::<(usize, usize, usize)>::new());
    std::fs::remove_dir_all(crate::fiabilite_fixture::racine(&manifeste)).ok();
}

#[test]
fn un_plan_en_espace_3d_du_format_55_garde_son_image_et_son_ordre() {
    let mut doc = plan_ordinaire();
    doc.panels[0].layers.swap(0, 2);
    doc.panels[0].espace = true;
    let d = doc.default_camera();
    doc.panels[0].camera = vec![CameraKey::new(0, d), CameraKey::new(11, Camera { x: d.x + 30.0, zoom: 1.2, pan: 20.0, ..d })];
    let avant = filmees(&doc);
    let dir = dossier("espace-55");
    let manifeste = enregistrer_au_format_55(&doc, &dir);
    let lu = crate::project::load(&manifeste).expect("s'ouvre");
    assert!(lu.doc.panels[0].espace && lu.doc.ordre_par_la_distance(0));
    assert_eq!(ecarts(&avant, &filmees(&lu.doc)), Vec::<(usize, usize, usize)>::new());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn un_projet_neuf_du_format_56_garde_ses_choix_a_l_aller_retour() {
    let neuf = || {
        let mut doc = plan_ordinaire();
        doc.panels[0].ordre_distance = true;
        doc.panels[0].camera_libre = true;
        doc.panels[0].camera = vec![CameraKey::new(0, Camera { tilt: 90.0, pan: 200.0, ..doc.default_camera() })];
        doc.panels[0].layers[1].profondeur_bornee = true;
        doc.panels[0].layers[0].pos_keys = vec![PosKey::new(0, LayerTransform { dz: -3.0, ..LayerTransform::default() })];
        doc
    };
    let mut doc = plan_ordinaire();
    assert!(!doc.panels[0].espace && !doc.panels[0].camera_libre && !doc.panels[0].ordre_distance, "un plan neuf se dessine à plat");
    doc.panels[0].ordre_distance = true;
    doc.panels[0].camera_libre = true;
    doc.panels[0].camera = vec![CameraKey::new(0, Camera { tilt: 120.0, pan: 200.0, ..doc.default_camera() })];
    doc.panels[0].layers[1].profondeur_bornee = true;
    doc.panels[0].layers[0].pos_keys = vec![PosKey::new(0, LayerTransform { dz: -3.0, ..LayerTransform::default() })];
    let avant = filmees(&doc);
    let dir = dossier("56");
    let manifeste = dir.join("P.shot");
    crate::project::save(&doc, &manifeste, &mut Default::default()).expect("enregistre");
    let lu = crate::project::load(&manifeste).expect("relu");
    let p = &lu.doc.panels[0];
    assert!(p.ordre_distance && p.camera_libre && !p.espace && p.layers[1].profondeur_bornee);
    assert_eq!(p.camera[0].cam.tilt, 90.0, "la bascule d'une caméra libre va jusqu'à la verticale");
    assert_eq!(p.camera[0].cam.pan, 200.0, "le panoramique d'une caméra libre fait le tour");
    assert_eq!(p.layers[0].depth + p.layers[0].pos_keys[0].t.dz, 1.5 - 3.0, "une profondeur au-delà de la borne reste");
    // (La bascule relue à 90° au lieu de 120° : la même borne que le format 55 pour un plan en Espace 3D.)
    assert_eq!(ecarts(&filmees(&neuf()), &filmees(&lu.doc)), Vec::<(usize, usize, usize)>::new());
    let _ = avant;
    // Un projet d'un format plus récent est refusé, comme toujours.
    let texte = std::fs::read_to_string(&manifeste).expect("relu");
    std::fs::write(&manifeste, texte.replace("\"version\": 59", "\"version\": 60")).expect("écrit");
    assert!(crate::project::load(&manifeste).is_err_and(|e| e.contains("plus récente")));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn la_camera_libre_ne_passe_par_le_repere_qu_aux_images_ou_elle_sort_des_anciennes_limites() {
    let mut doc = plan_ordinaire();
    let d = doc.default_camera();
    doc.panels[0].camera = vec![CameraKey::new(0, Camera { x: d.x + 20.0, ..d }), CameraKey::new(5, Camera { x: d.x + 30.0, zoom: 1.2, ..d }), CameraKey::new(11, Camera { pan: 160.0, ..d })];
    let avant = filmees(&doc);
    // Sans `camera_libre` (un projet d'avant), jamais de repère, même pour une caméra que le format 55 aurait bridée.
    assert!((0..12).all(|f| doc.repere_du_plan(0, f).is_none()));
    doc.panels[0].camera_libre = true;
    let libres: Vec<usize> = (0..12).filter(|&f| doc.repere_du_plan(0, f).is_some()).collect();
    assert!(!libres.is_empty() && !libres.contains(&0) && !libres.contains(&5), "seules les images où la caméra est hors des limites : {libres:?}");
    let apres = filmees(&doc);
    for f in (0..12).filter(|f| !libres.contains(f)) {
        assert_eq!(avant[0][f], apres[0][f], "image {f} : le chemin de toujours, les mêmes pixels");
    }
}

fn banc(doc: Document) -> Banc {
    let mut b = banc_taille(doc, false, 1280, 720);
    b.chauffer(2);
    b.app.cur_layer = 0;
    b
}

fn ouvrir(b: &mut Banc) {
    b.app.run_action("espace3d.basculer");
    b.chauffer(3);
    assert!(b.app.espace3d.on && !b.app.camera_view, "la vue de la scène est ouverte");
}

fn fermer(b: &mut Banc) {
    b.app.run_action("espace3d.basculer");
    b.chauffer(3);
    assert!(!b.app.espace3d.on, "la vue de la scène est fermée");
}

/// Ce qui fait les dessins et leur place.
fn poses(doc: &Document) -> String {
    doc.panels[0].layers.iter().map(|l| format!("{} {:?} {:?} {:?} {:?}", l.depth, l.transform, l.pos_keys, l.frames, l.calque_3d)).collect::<Vec<_>>().join("\n")
}

#[test]
fn le_cube_ne_change_rien_d_un_plan_qu_on_regarde_et_sol_mur_ciel_se_posent_sur_tout_plan() {
    // Le plan d'avant le plus difficile : caméra animée, pile dans le désordre, calque penché.
    let mut doc = projet_d_avant();
    doc.panels.truncate(1);
    let avant = filmees(&doc);
    let dessins = poses(&doc);
    let mut b = banc(doc);
    let etapes = b.app.history.undo_len();
    ouvrir(&mut b);
    assert!(!b.app.status.contains("gardé ordinaire") && !b.app.status.contains("refus"), "{}", b.app.status);
    assert_eq!(b.app.history.undo_len(), etapes, "regarder ne pose aucune étape");
    assert_eq!(ecarts(&avant, &filmees(&b.app.doc)), Vec::<(usize, usize, usize)>::new(), "regarder ne change aucun pixel");
    fermer(&mut b);
    assert_eq!(ecarts(&avant, &filmees(&b.app.doc)), Vec::<(usize, usize, usize)>::new());
    assert_eq!(poses(&b.app.doc), dessins);
    // Sol, Mur, Ciel, une surface : posés, jamais refusés. La pile dans le désordre garde son ordre, et la barre d'état le dit.
    for (action, nom) in [("calque.nouveau_sol", "Sol"), ("calque.nouveau_mur", "Mur"), ("calque.nouveau_ciel", "Ciel"), ("espace3d.surface_face", "Surface face à la caméra")] {
        b.app.run_action(action);
        b.chauffer(1);
        assert_eq!(b.app.layer().name, nom, "{action} : {}", b.app.status);
        assert!(b.app.status.contains(crate::espace3ddirect::PILE_GARDEE), "{action} : {}", b.app.status);
    }
    assert!(b.app.espace3d.on, "un calque de la scène se pose dans la vue de la scène");
    assert!(!b.app.doc.ordre_par_la_distance(0));
    // Un plan dont la pile est dans l'ordre des distances : il passe à « devant cache derrière » dans l'étape du Sol, sans
    // changer l'image de ce qu'il avait ; Ctrl+Z rend tout.
    let doc = plan_ordinaire();
    let avant = filmees(&doc);
    let mut b = banc(doc);
    ouvrir(&mut b);
    let etapes = b.app.history.undo_len();
    b.app.run_action("calque.nouveau_sol");
    b.chauffer(1);
    assert!(b.app.doc.ordre_par_la_distance(0) && !b.app.status.contains(crate::espace3ddirect::PILE_GARDEE), "{}", b.app.status);
    assert_eq!(b.app.history.undo_len(), etapes + 1, "une seule étape");
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert!(!b.app.doc.ordre_par_la_distance(0));
    assert_eq!(ecarts(&avant, &filmees(&b.app.doc)), Vec::<(usize, usize, usize)>::new());
    // « Devant cache derrière » se règle à la main, en une étape.
    b.app.run_action("espace3d.ordre_distance");
    assert!(b.app.doc.ordre_par_la_distance(0));
    b.app.run_action("edition.annuler");
    assert!(!b.app.doc.ordre_par_la_distance(0));
}

#[test]
fn la_vue_de_la_scene_libere_la_camera_et_l_outil_camera_de_la_feuille_la_regle_encore() {
    let mut b = banc(plan_ordinaire());
    ouvrir(&mut b);
    // La vue de la scène pose la caméra telle quelle : retournée, elle sort des anciennes limites.
    let cam = Camera { pan: 150.0, tilt: 20.0, ..b.app.current_camera() };
    b.app.espace3d_step("Orienter la caméra", |s| s.set_current_camera_libre(cam));
    assert!(b.app.doc.panels[0].camera_libre, "le plan retient sa caméra libre");
    assert_eq!(b.app.current_camera().pan, 150.0, "rien ne la bride");
    fermer(&mut b);
    // Sur la feuille, l'image passe par le repère : le cadre est au centre, et l'outil Caméra le tire sans refuser.
    assert!(b.app.doc.repere_du_plan(0, 0).is_some());
    b.app.tool = crate::app::Tool::Camera;
    b.chauffer(1);
    let centre = b.app.doc.stage_center();
    let ecran = |b: &Banc, p: Pos2| b.app.view.to_screen(b.app.canvas_rect, b.app.canvas_size(), p);
    let s = ecran(&b, centre);
    b.app.camera_press(centre, s);
    b.app.camera_drag(centre + eframe::egui::vec2(30.0, 0.0), false);
    b.app.finish_camera_drag();
    assert!(!dit_la_3d(&b.app.status), "{}", b.app.status);
    let apres = b.app.current_camera();
    assert!(apres != cam && apres.pan == 150.0, "la caméra a glissé dans son propre plan : {apres:?}");
    // Sur un plan dessiné à plat, le même geste fait exactement ce qu'il a toujours fait : le cadre glisse de 30 pixels.
    let mut b = banc(plan_ordinaire());
    b.app.tool = crate::app::Tool::Camera;
    b.chauffer(1);
    let cam0 = b.app.current_camera();
    let s = ecran(&b, centre);
    b.app.camera_press(centre, s);
    b.app.camera_drag(centre + eframe::egui::vec2(30.0, -12.0), false);
    b.app.finish_camera_drag();
    assert_eq!(b.app.current_camera(), cam0.shifted(30.0, -12.0).limited(b.app.doc.frame));
    assert!(b.app.doc.repere_du_plan(0, 0).is_none() && !b.app.doc.panels[0].camera_libre);
}

#[test]
fn les_mouvements_de_camera_des_vignettes_marchent_sur_tout_plan_dont_la_camera_se_dit_sur_la_feuille() {
    let mut doc = plan_ordinaire();
    doc.panels[0].length = 12;
    assert!(doc.camera_en_cadres(0), "un plan neuf : des cadres sur la feuille, comme toujours");
    let mut b = banc(doc);
    b.app.cam_ajouter_cadre(0);
    assert!(b.app.doc.panels[0].camera.len() >= 2, "un cadre de plus : {}", b.app.status);
    assert!(!dit_la_3d(&b.app.status), "{}", b.app.status);
    // Une caméra retournée par la vue de la scène ne se dit pas en cadres : la barre d'état le dit, sans un mot de 3D.
    let d = b.app.doc.default_camera();
    b.app.doc.panels[0].camera = vec![CameraKey::new(0, Camera { pan: 170.0, ..d })];
    b.app.doc.panels[0].camera_libre = true;
    assert!(!b.app.doc.camera_en_cadres(0));
    b.app.cam_ajouter_cadre(0);
    assert_eq!(b.app.status, crate::vignettecamui::CAMERA_SANS_CADRES);
    assert!(!dit_la_3d(crate::vignettecamui::CAMERA_SANS_CADRES));
}

// --------------------------------------------------------------------- la 3D ne se sent jamais hors de la vue de la scène

#[test]
fn le_detecteur_reconnait_les_mots_de_la_3d_et_seulement_eux() {
    for t in ["Espace 3D", "Plan en Espace 3D : sa caméra…", "posée à 2,5 m en mètres", "une carte inclinée", "Choisis une carte ou la caméra", "Les cartes du plan", "Trajectoire dans l'espace", "la scène 3D de blocage"] {
        assert!(dit_la_3d(t), "« {t} » dit la 3D");
    }
    for t in ["Caméra : décalage +30 × +0 px, zoom 100 %", "le pilote de la carte graphique", "Cinq étapes, une petite carte à chaque fois.", "Le panneau est dans l'espace de travail", "Profondeur", "Vue de dessus (profondeur des calques)", "Il écarte le calque", "Scène 2"] {
        assert!(!dit_la_3d(t), "« {t} » ne dit pas la 3D");
    }
}

/// Tous les textes peints à l'image d'écran d'avant.
fn textes_peints(b: &Banc) -> Vec<String> {
    fn dans(shape: &eframe::egui::Shape, out: &mut Vec<String>) {
        match shape {
            eframe::egui::Shape::Text(t) => out.push(t.galley.job.text.clone()),
            eframe::egui::Shape::Vec(v) => v.iter().for_each(|s| dans(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in &b.formes {
        dans(&c.shape, &mut out);
    }
    out
}

fn sans_3d(b: &Banc, quand: &str) {
    assert!(!dit_la_3d(&b.app.status), "{quand} : la barre d'état dit la 3D : {}", b.app.status);
    assert!(!dit_la_3d(crate::messages::resume(&b.app.status).as_ref()), "{quand}");
    let peints: Vec<String> = textes_peints(b).into_iter().filter(|t| dit_la_3d(t)).collect();
    assert!(peints.is_empty(), "{quand} : l'écran montre {peints:?}");
}

#[test]
fn en_2d_aucun_texte_de_l_interface_ne_dit_la_3d() {
    let mut doc = projet_d_avant();
    doc.panels.truncate(2);
    let mut b = banc(doc);
    b.garder_formes = true;
    b.chauffer(1);
    sans_3d(&b, "à l'ouverture");
    // Les gestes de tous les jours, à plat : changer d'image, de calque, de plan ; l'outil Caméra ; la vue de dessus ; une clé
    // de caméra ; un calque neuf ; annuler, rétablir ; accrocher à la caméra un calque penché ; fusionner un calque penché.
    let centre = b.app.doc.stage_center();
    for (k, geste) in ["image.suivante", "image.precedente", "panneau.suivant", "panneau.precedent", "vue.dessus", "camera.cle_ici", "calque.nouveau_bitmap", "edition.annuler", "edition.retablir", "vue.dessus", "calque.accrocher_camera", "calque.fusionner_dessous"].into_iter().enumerate() {
        if geste == "calque.accrocher_camera" || geste == "calque.fusionner_dessous" {
            b.app.cur_layer = 2.min(b.app.layers().len() - 1);
            if let Some(i) = b.app.layers().iter().position(|l| l.transform.is_tilted()) {
                b.app.cur_layer = i;
            }
        }
        b.app.run_action(geste);
        b.chauffer(2);
        sans_3d(&b, &format!("geste {k} « {geste} »"));
    }
    b.app.tool = crate::app::Tool::Camera;
    b.chauffer(1);
    let s = b.app.view.to_screen(b.app.canvas_rect, b.app.canvas_size(), centre);
    b.app.camera_press(centre, s);
    b.app.camera_drag(centre + eframe::egui::vec2(12.0, 4.0), false);
    b.app.finish_camera_drag();
    b.chauffer(2);
    sans_3d(&b, "outil Caméra");
    b.app.tool = crate::app::Tool::Trajectoire;
    b.chauffer(3);
    sans_3d(&b, "outil Trajectoire sur la feuille");
    // La vue de la scène ouverte puis fermée : son panneau et son icône s'en vont avec elle.
    ouvrir(&mut b);
    assert!(b.app.pane_shown(crate::dock::Pane::Espace3d), "vue ouverte : son panneau est là");
    fermer(&mut b);
    b.chauffer(2);
    assert!(!b.app.pane_shown(crate::dock::Pane::Espace3d), "vue fermée : son panneau s'en va");
    sans_3d(&b, "après la vue de la scène");
    // Les menus : la palette (Ctrl+K) ne propose, de la 3D, que sa porte ; l'aide animée qui joue sur la feuille non plus.
    let palette: Vec<&str> = b.app.palette_commandes_ici().into_iter().map(|(_, n)| n).filter(|n| dit_la_3d(n)).collect();
    assert_eq!(palette, vec!["Espace 3D (afficher / quitter)"], "la palette en 2D");
    for a in crate::aideanim::toutes().filter(|a| a.contextes.iter().any(|c| !c.starts_with("espace3d") && *c != "trajectoire.espace")) {
        assert!(!dit_la_3d(a.titre) && !dit_la_3d(a.texte), "l'aide « {} » joue en 2D et dit la 3D", a.id);
    }
    // Les bulles des barres : celles des commandes et des panneaux montrés hors de la vue de la scène.
    for barre in crate::barres::Barre::TOUTES {
        for c in barre.commandes() {
            assert!(!dit_la_3d(c.nom) && !dit_la_3d(c.description), "barre {} : « {} »", barre.nom(), c.nom);
        }
    }
    for espace in crate::workspace::Espace::TOUS {
        for g in crate::barrepanneaux::icones_de(espace, true, &|_| false) {
            for e in g.iter().filter(|e| e.pane != crate::dock::Pane::Espace3d) {
                assert!(!dit_la_3d(e.nom) && !dit_la_3d(e.description), "barre des panneaux : « {} »", e.nom);
            }
        }
    }
    for t in crate::app::Tool::ALL {
        assert!(!dit_la_3d(t.label()) && !dit_la_3d(t.description()), "barre d'outils : « {} »", t.label());
    }
}

#[test]
fn un_projet_partage_d_avant_le_format_56_se_migre_aussi_plan_par_plan() {
    // (Trouvé à la relecture : un projet partagé lisait tous ses plans comme du format 56. Un fichier de plan écrit avant, en
    // `VERSION_PLAN` 1, se lit maintenant comme un plan d'un projet 55.)
    let au_55 = comme_au_format_55(projet_d_avant());
    let avant = filmees(&au_55);
    let racine = dossier("partage-55");
    let manifeste = crate::cooperation_tests::partager(&projet_d_avant(), &racine);
    let plans = racine.join("Film").join("plans");
    let mut vus = 0;
    for e in std::fs::read_dir(&plans).expect("plans").flatten() {
        let chemin = e.path();
        if chemin.extension().is_some_and(|x| x == "json") {
            let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&chemin).expect("lu")).expect("JSON");
            assert_eq!(v["version"], serde_json::json!(crate::project::partage_io::VERSION_PLAN));
            v["version"] = serde_json::json!(1);
            std::fs::write(&chemin, serde_json::to_string_pretty(&v).expect("écrit")).expect("écrit");
            vus += 1;
        }
    }
    assert_eq!(vus, 3, "un fichier par plan");
    let lu = crate::project::load(&manifeste).expect("le projet partagé s'ouvre");
    assert_eq!(ecarts(&avant, &filmees(&lu.doc)), Vec::<(usize, usize, usize)>::new(), "les mêmes images");
    assert!(lu.doc.panels[0].layers.iter().any(|l| l.profondeur_bornee), "migré comme un projet 55");
    // Relu tel qu'il s'écrit aujourd'hui (plans en 2) : rien n'est migré deux fois.
    let manifeste = crate::cooperation_tests::partager(&lu.doc, &dossier("partage-56"));
    let relu = crate::project::load(&manifeste).expect("relu");
    assert_eq!(ecarts(&avant, &filmees(&relu.doc)), Vec::<(usize, usize, usize)>::new());
    std::fs::remove_dir_all(&racine).ok();
}