//! Essais de la partie 158 (`docs/INTERFACE.md`) : le maillage du sol reste visible et juste quand la vue est tout près
//! de la scène, et la vue Espace 3D met d'elle-même le plan en Espace 3D quand cela ne change rien à son image
//! (`espace3ddirect.rs`).

use eframe::egui::{pos2, Color32, Pos2, Rect};

use crate::camera::{Camera, CameraKey};
use crate::document::{Document, Layer, LayerKind, LayerTransform};
use crate::espace3ddirect::{Garde, PASSE};
use crate::espace3dmath::{Orbite, View};
use crate::espace3dview::{grid_lines_pas, traits_a_l_ecran, GridLine};
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

// ------------------------------------------------------------------------------------ le plan passe tout seul en Espace 3D

const ROUGE: Color32 = Color32::from_rgb(220, 40, 40);
const VERT: Color32 = Color32::from_rgb(40, 200, 60);
const BLEU: Color32 = Color32::from_rgb(40, 80, 230);

fn remplir(doc: &mut Document, cel: crate::document::CelId, r: [usize; 4], color: Color32) {
    let raster = &mut doc.cels.get_mut(&cel).expect("dessin").raster;
    for y in r[1]..r[3] {
        for x in r[0]..r[2] {
            let idx = raster.tile_index_of(x, y);
            raster.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = color;
        }
    }
}

/// Un plan ordinaire de storyboard : cadre 320 x 180 sur une feuille 640 x 360, trois cartes posees a des profondeurs
/// differentes, dans l'ordre de la pile (la plus loin en bas), qui se recouvrent ; celle du dessus glisse (cles de position).
fn plan_ordinaire() -> Document {
    let mut doc = Document::with_frame(320, 180, 24);
    doc.panels[0].layers = Vec::new();
    for (k, (color, r, depth)) in [(ROUGE, [180, 100, 420, 260], 1.5), (VERT, [240, 130, 400, 230], 0.5), (BLEU, [300, 150, 360, 250], 0.0)].into_iter().enumerate() {
        let mut layer = Layer::new(format!("Carte {k}"), LayerKind::Bitmap);
        let cel = doc.new_cel(LayerKind::Bitmap);
        remplir(&mut doc, cel, r, color);
        layer.frames = vec![Some(cel); 12];
        layer.depth = depth;
        doc.panels[0].layers.push(layer);
    }
    doc.panels[0].layers[2].pos_keys = vec![PosKey::new(0, LayerTransform::default()), PosKey::new(11, LayerTransform { x: 40.0, y: -10.0, ..LayerTransform::default() })];
    doc
}

/// L'image filmee du plan 0 a chaque image, par le chemin de toutes les sorties.
fn filmees(doc: &Document) -> Vec<Vec<Color32>> {
    (0..doc.panels[0].frame_count())
        .map(|f| {
            let plan = render::panel_plan_at(doc, 0, f, None);
            render::camera_image(doc, &plan, &doc.panel_camera(0, f))
        })
        .collect()
}

/// Ce qui fait les dessins et leur place : rien de cela ne doit bouger.
fn poses(doc: &Document) -> String {
    doc.panels[0].layers.iter().map(|l| format!("{} {:?} {:?} {:?} {:?}", l.depth, l.transform, l.pos_keys, l.frames, l.calque_3d)).collect::<Vec<_>>().join("\n")
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
    assert!(b.app.espace3d.on && !b.app.camera_view, "la vue Espace 3D est ouverte");
}

fn fermer(b: &mut Banc) {
    b.app.run_action("espace3d.basculer");
    b.chauffer(3);
    assert!(!b.app.espace3d.on, "la vue Espace 3D est fermee");
}

#[test]
fn ouvrir_la_vue_met_le_plan_en_espace_3d_sans_changer_un_pixel_de_l_image_filmee() {
    let doc = plan_ordinaire();
    assert_eq!(doc.garde_de_l_espace(0), None, "rien ne s'y oppose");
    let avant = filmees(&doc);
    let dessins = poses(&doc);
    assert!(avant[0].iter().any(|c| *c == BLEU) && avant[0].iter().any(|c| *c == VERT), "les cartes se voient");
    let mut b = banc(doc);
    let etapes = b.app.history.undo_len();
    assert!(!b.app.doc.plan_en_espace(0));
    ouvrir(&mut b);
    // Le petit cube suffit : le plan est en Espace 3D, la barre d'etat le dit d'abord.
    assert!(b.app.doc.plan_en_espace(0), "le plan est passe en Espace 3D : {}", b.app.status);
    assert_eq!(b.app.status, PASSE);
    assert!(crate::messages::resume(&b.app.status).contains("Plan passé en Espace 3D tout seul"), "{}", crate::messages::resume(&b.app.status));
    assert_eq!(b.app.history.undo_len(), etapes, "pas d'etape d'annulation : rien n'a change");
    // Pas un pixel de l'image filmee, a aucune image, ni un dessin, ni une pose.
    let pendant = filmees(&b.app.doc);
    for (f, (a, p)) in avant.iter().zip(&pendant).enumerate() {
        assert_eq!(a.iter().zip(p).filter(|(x, y)| x != y).count(), 0, "image {f} : la meme, pixel pour pixel");
    }
    assert_eq!(poses(&b.app.doc), dessins);
    // Ce qui n'existait que pour les plans en Espace 3D est la : « Sol » pose son calque sans refuser.
    b.app.run_action("calque.nouveau_sol");
    assert_eq!(b.app.layer().name, "Sol", "{}", b.app.status);
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(b.app.doc.panels[0].layers.len(), 3);
    assert!(b.app.doc.plan_en_espace(0), "l'annulation du Sol laisse le plan en Espace 3D tant que la vue le montre");
    // La vue fermee sans rien y avoir fait : le plan redevient ordinaire, l'image est toujours la meme.
    fermer(&mut b);
    assert!(!b.app.doc.plan_en_espace(0), "rien n'avait besoin de l'Espace 3D : le plan est de nouveau ordinaire");
    assert_eq!(filmees(&b.app.doc), avant);
    assert_eq!(poses(&b.app.doc), dessins);
}

#[test]
fn un_plan_dont_le_passage_changerait_l_image_reste_ordinaire_et_la_barre_d_etat_dit_pourquoi() {
    // La camera a bouge.
    let mut doc = plan_ordinaire();
    let cam = Camera { x: 30.0, zoom: 1.2, ..doc.default_camera() };
    doc.panels[0].camera = vec![CameraKey::new(0, cam)];
    assert_eq!(doc.garde_de_l_espace(0), Some(Garde::Camera));
    let avant = filmees(&doc);
    let mut b = banc(doc);
    ouvrir(&mut b);
    assert!(!b.app.doc.plan_en_espace(0));
    assert!(crate::messages::resume(&b.app.status).starts_with("Plan gardé ordinaire : sa caméra a bougé"), "{}", b.app.status);
    assert_eq!(filmees(&b.app.doc), avant);
    // Sol, Mur, Ciel et Nouvelle surface refusent, en disant pourquoi ; rien ne bouge.
    let (n, etapes) = (b.app.layers().len(), b.app.history.undo_len());
    b.app.run_action("calque.nouveau_sol");
    assert!(b.app.status.starts_with("Sol refusé : sa caméra a bougé"), "{}", b.app.status);
    b.app.run_action("espace3d.surface_face");
    assert!(b.app.status.starts_with("Surface face à la caméra refusée : sa caméra a bougé"), "{}", b.app.status);
    assert_eq!((b.app.layers().len(), b.app.history.undo_len()), (n, etapes));
    assert!(!b.app.doc.plan_en_espace(0));
    // Le bouton l'y met quand meme : un choix, une etape ; la vue fermee ne le rend pas ordinaire.
    b.app.run_action("espace3d.plan_en_espace");
    assert!(b.app.doc.plan_en_espace(0));
    fermer(&mut b);
    assert!(b.app.doc.plan_en_espace(0), "un choix a la main reste");

    // La pile dans le desordre des profondeurs, une carte penchee, une carte contre la camera.
    let mut doc = plan_ordinaire();
    doc.panels[0].layers.swap(0, 2);
    assert_eq!(doc.garde_de_l_espace(0), Some(Garde::Ordre));
    let mut doc = plan_ordinaire();
    doc.panels[0].layers[1].transform.tilt_x = 30.0;
    assert_eq!(doc.garde_de_l_espace(0), Some(Garde::Penchee));
    let mut doc = plan_ordinaire();
    doc.panels[0].layers[2].pos_keys[1].t.dz = -1.5;
    assert_eq!(doc.garde_de_l_espace(0), Some(Garde::Borne), "a la derniere image, la carte passe la borne");
    // Les phrases tiennent dans la barre d'etat.
    for g in [Garde::Cadre, Garde::Camera, Garde::Borne, Garde::Penchee, Garde::Ordre, Garde::TroisD] {
        let r = crate::messages::resume(g.phrase());
        assert!(r.starts_with("Plan gardé ordinaire : ") && r.chars().count() > 30, "{g:?} : {r}");
    }
}

#[test]
fn un_calque_3d_met_un_plan_ordinaire_en_espace_3d_dans_sa_propre_etape() {
    // Sans ouvrir la vue : le menu « Nouveau calque > Calques 3D ».
    let doc = plan_ordinaire();
    let avant = filmees(&doc);
    let mut b = banc(doc);
    let etapes = b.app.history.undo_len();
    b.app.run_action("calque.nouveau_mur");
    b.chauffer(2);
    assert_eq!(b.app.layer().name, "Mur", "{}", b.app.status);
    assert!(b.app.doc.plan_en_espace(0));
    assert_eq!(b.app.history.undo_len(), etapes + 1, "une seule etape");
    // Ctrl+Z : le calque et le passage partent ensemble, l'image est celle d'avant.
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(b.app.doc.panels[0].layers.len(), 3);
    assert!(!b.app.doc.plan_en_espace(0), "le plan est ordinaire comme avant");
    assert_eq!(filmees(&b.app.doc), avant);
}

#[test]
fn quitter_la_vue_garde_l_espace_3d_d_un_plan_qui_s_en_sert_et_decocher_est_respecte() {
    let mut b = banc(plan_ordinaire());
    ouvrir(&mut b);
    assert!(b.app.doc.plan_en_espace(0));
    assert!(b.app.doc.panels[0].espace_tout_seul, "marqué « mis tout seul »");
    // Une etape quelconque vue ouverte (son cliche garde le plan en Espace 3D), la vue fermee, puis Ctrl+Z : le plan
    // revient en Espace 3D par l'annulation, et redevient ordinaire aussitot (rien ne s'en sert).
    // (Un calque vide tout en haut de la pile, a la profondeur de la carte du dessus : l'ordre reste celui de la pile.)
    b.app.cur_layer = 2;
    b.app.run_action("calque.nouveau_bitmap");
    b.chauffer(1);
    assert_eq!(b.app.doc.panels[0].layers.len(), 4);
    assert!(b.app.doc.panels[0].espace_tout_seul, "rien ne s'en sert encore");
    fermer(&mut b);
    assert!(!b.app.doc.plan_en_espace(0));
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert!(!b.app.doc.plan_en_espace(0), "l'annulation ne le laisse pas en Espace 3D");
    b.app.run_action("edition.retablir");
    b.chauffer(2);
    assert!(!b.app.doc.plan_en_espace(0), "le retablissement non plus");
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(b.app.doc.panels[0].layers.len(), 3);
    // Un plan mis tout seul s'enregistre ordinaire.
    ouvrir(&mut b);
    assert!(b.app.doc.plan_en_espace(0));
    let dir = std::env::temp_dir().join(format!("shot-test-direct-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dossier");
    let manifeste = dir.join("P.shot");
    crate::project::save(&b.app.doc, &manifeste, &mut Default::default()).expect("enregistre");
    assert!(!crate::project::load(&manifeste).expect("relu").doc.panels[0].espace, "le fichier garde le plan ordinaire");
    std::fs::remove_dir_all(&dir).ok();
    // Un Sol (couche au sol) : le plan s'en sert, il reste en Espace 3D quand on ferme la vue.
    b.app.run_action("calque.nouveau_sol");
    fermer(&mut b);
    assert!(b.app.doc.plan_en_espace(0), "le Sol a besoin de l'Espace 3D");
    // Ctrl+Z hors de la vue : le Sol part, plus rien n'a besoin de l'Espace 3D, le plan redevient ordinaire.
    b.app.run_action("edition.annuler");
    b.chauffer(3);
    assert_eq!(b.app.doc.panels[0].layers.len(), 3);
    assert!(!b.app.doc.plan_en_espace(0), "redevenu ordinaire");
    // Decoche a la main dans la vue : la vue ne le remet pas.
    ouvrir(&mut b);
    assert!(b.app.doc.plan_en_espace(0));
    b.app.run_action("espace3d.plan_en_espace");
    b.chauffer(3);
    assert!(!b.app.doc.plan_en_espace(0), "decoche : il reste ordinaire");
    fermer(&mut b);
    ouvrir(&mut b);
    assert!(!b.app.doc.plan_en_espace(0), "rouvrir la vue ne revient pas sur ce choix");
}