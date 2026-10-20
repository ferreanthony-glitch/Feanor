//! Coller ce qui vient d'un projet d'une autre taille de feuille ou d'une autre cadence (`collage.rs`,
//! `adaptation.rs`) : plans, calque, images, clés, caméra, effets, éléments, zones, sélection sur plusieurs calques.
//!
//! Ce que ces essais gardent : la copie est adaptée (pixels, traits, caméra, poses, effets) dans les deux sens ; la
//! question se pose quand les proportions ou la cadence diffèrent, et seulement alors ; ce qu'on choisit est retenu ;
//! une feuille de même taille se colle exactement comme avant, sans question ni calcul de fond ; une seule étape
//! d'annulation, nommée ; le calcul n'occupe jamais le fil de l'interface ; le presse-papiers n'est pas touché.

use std::time::{Duration, Instant};

use eframe::egui::Color32;

use crate::adaptation::{Ajustement, Duree, Origine};
use crate::camera::{Camera, CameraKey};
use crate::document::{Cel, Document, Layer, LayerKind, LayerTransform};
use crate::fx::{Effect, EffectKind};
use crate::perf_tests::{banc, Banc};
use crate::poskeys::PosKey;
use crate::vector::{Element, VPoint};

// ---------------------------------------------------------------------------------------------- les projets

const ROUGE: Color32 = Color32::from_rgba_premultiplied(200, 0, 0, 255);

fn rectangle(cel: &mut Cel, x0: usize, y0: usize, x1: usize, y1: usize, c: Color32) {
    for y in y0..y1 {
        for x in x0..x1 {
            let i = cel.raster.tile_index_of(x, y);
            cel.raster.tile_mut(i)[(y % 64) * 64 + x % 64] = c;
        }
    }
    cel.version += 1;
}

/// Un plan de `n` images, avec un calque bitmap (un rectangle rouge de ±w/16 autour du centre de la feuille, décalé
/// d'une image à l'autre), un calque vectoriel (un trait), un décalage de calque, des clés de position, un flou et deux
/// clés de caméra. Tout se compte en fonction de la largeur du cadre : les valeurs attendues s'en déduisent.
fn plan_riche(cadre: (usize, usize), fps: u32, n: usize) -> Document {
    let (w, h) = cadre;
    let mut doc = Document::with_frame(w, h, fps);
    let (cx, cy) = (doc.width / 2, doc.height / 2);
    let mut fond = Layer::new("Fond", LayerKind::Bitmap);
    let mut trait_ = Layer::new("Trait", LayerKind::Vector);
    for k in 0..n {
        let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
        let dx = k * (w / 96);
        rectangle(&mut cel, cx - w / 16 + dx, cy - h / 16, cx + w / 16 + dx, cy + h / 16, ROUGE);
        fond.frames.push(Some(doc.add_cel(cel)));
        let mut v = Cel::new(doc.width, doc.height, LayerKind::Vector);
        v.vector.as_mut().expect("vectoriel").elements.push(Element::Stroke {
            points: vec![VPoint { x: (cx - w / 8) as f32, y: (cy + h / 4) as f32, r: w as f32 / 240.0 }, VPoint { x: (cx + w / 8) as f32, y: (cy + h / 4) as f32, r: w as f32 / 240.0 }],
            color: [0, 0, 200],
            opacity: 1.0,
            hardness: 1.0,
            brush: None,
            trace: Default::default(),
        });
        v.rerender();
        trait_.frames.push(Some(doc.add_cel(v)));
    }
    fond.transform = LayerTransform { x: w as f32 / 10.0, y: 0.0, ..Default::default() };
    fond.pos_keys = vec![PosKey::new(0, fond.transform), PosKey::new(8, LayerTransform { x: w as f32 / 5.0, ..fond.transform })];
    let mut flou = Effect::new(EffectKind::GaussianBlur);
    flou.values[0] = w as f32 / 96.0;
    fond.fx = vec![flou];
    let centre = Camera::centered(doc.width, doc.height);
    let p = &mut doc.panels[0];
    p.layers = vec![fond, trait_];
    p.length = n;
    p.camera = vec![CameraKey::new(0, centre), CameraKey::new(n / 2, Camera { x: centre.x + w as f32 / 4.0, dolly: w as f32 / 20.0, ..centre })];
    doc
}

fn nouvel_onglet(b: &mut Banc, doc: Document) {
    assert!(b.app.nouvel_onglet(), "{}", b.app.status);
    assert!(b.app.charger_projet(doc));
    b.app.welcome = false;
    b.chauffer(2);
}

/// Copie le premier plan du projet affiché.
fn copier_plan(b: &mut Banc) {
    b.app.cur_panel = 0;
    b.app.board_sel = vec![b.app.doc.panels[0].id];
    b.app.panel_focus = true;
    b.app.copy_panels();
    assert!(b.app.panel_clipboard.is_some());
}

/// Copie le plan courant (celui qui vient d'être collé).
fn copier_plan_courant(b: &mut Banc) {
    b.app.board_sel = vec![b.app.doc.panels[b.app.cur_panel].id];
    b.app.panel_focus = true;
    b.app.copy_panels();
    assert!(b.app.panel_clipboard.is_some());
}

/// Des images d'écran jusqu'à la fin du calcul de fond ; rend la plus longue durée d'une image d'écran.
fn attendre(b: &mut Banc) -> Duration {
    let mut pire = Duration::ZERO;
    let debut = Instant::now();
    while b.app.collage.tache.is_some() {
        assert!(debut.elapsed() < Duration::from_secs(300), "le collage ne finit pas : {}", b.app.status);
        let t = Instant::now();
        b.image(Vec::new(), 0.016);
        pire = pire.max(t.elapsed());
        std::thread::sleep(Duration::from_millis(3));
    }
    b.image(Vec::new(), 0.016);
    pire
}

fn cels_du_plan(doc: &Document, plan: usize, calque: usize) -> Vec<&Cel> {
    doc.panels[plan].layers[calque].frames.iter().flatten().map(|id| &doc.cels[id]).collect()
}

fn dernier_nom(b: &Banc) -> String {
    b.app.history.names().0.last().map(|s| s.to_string()).unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------- les plans

#[test]
fn a_4k_plan_pasted_into_a_1080_project_is_scaled_and_scaled_back() {
    let mut b = banc(plan_riche((3840, 2160), 24, 4), false);
    copier_plan(&mut b);
    nouvel_onglet(&mut b, plan_riche((1920, 1080), 24, 1));
    let avant = b.app.doc.panels.len();
    let serial = b.app.history.serial();
    b.app.panel_focus = true;
    b.app.paste_panels();
    assert!(b.app.collage.dialogue.is_none(), "mêmes proportions, même cadence : rien à choisir");
    attendre(&mut b);
    assert_eq!(b.app.doc.panels.len(), avant + 1, "{}", b.app.status);
    assert!(b.app.status.contains("Collé depuis un projet 3840×2160 : mis à l'échelle ×0,5."), "{}", b.app.status);
    // Une seule étape d'annulation, nommée.
    assert_eq!(b.app.history.serial(), serial + 1);
    assert_eq!(dernier_nom(&b), "Coller des plans (adapté de 3840×2160)");

    let p = b.app.cur_panel;
    let doc = &b.app.doc;
    assert_eq!((doc.width, doc.height), (3840, 2160));
    // Les pixels : le rectangle de ±240 pixels est devenu ±120, au centre de la feuille d'arrivée.
    let fond = cels_du_plan(doc, p, 0);
    assert_eq!(fond.len(), 4);
    assert_eq!((fond[0].raster.width, fond[0].raster.height), (3840, 2160));
    assert_eq!(fond[0].raster.get(1920 + 110, 1080).a(), 255);
    assert_eq!(fond[0].raster.get(1920 + 130, 1080).a(), 0);
    assert_eq!(fond[0].raster.get(1920 - 119, 1080 - 59).a(), 255);
    assert_eq!(fond[3].raster.get(1920 - 119 + 3 * 20 + 2, 1080).a(), 255, "le décalage d'une image à l'autre est réduit aussi");
    // Les traits : points et épaisseur.
    let trait_ = cels_du_plan(doc, p, 1);
    let Element::Stroke { points, .. } = &trait_[0].vector.as_ref().expect("vectoriel").elements[0] else { panic!("un trait") };
    assert_eq!((points[0].x, points[0].y, points[0].r), (1920.0 - 240.0, 1080.0 + 270.0, 8.0));
    assert!(trait_[0].raster.get(1920, 1080 + 270).a() > 200, "le rendu suit");
    // Le calque : décalage et clés de position ; l'effet : le rayon du flou.
    let calque = &doc.panels[p].layers[0];
    assert_eq!(calque.transform.x, 192.0);
    assert_eq!(calque.pos_keys[1].t.x, 384.0);
    assert_eq!(calque.fx[0].values[0], 20.0);
    // Les clés de caméra : position et avancée.
    let cam = &doc.panels[p].camera;
    assert_eq!((cam[0].cam.x, cam[0].cam.y), (1920.0, 1080.0));
    assert_eq!((cam[1].cam.x, cam[1].cam.dolly), (1920.0 + 480.0, 96.0));

    // Annuler d'un coup : le plan disparaît. Rétablir : il revient, adapté.
    b.app.undo();
    assert_eq!(b.app.doc.panels.len(), avant);
    b.app.redo();
    assert_eq!(b.app.doc.panels.len(), avant + 1);

    // Et en sens inverse : le plan réduit, recopié dans le 4K, reprend sa taille (à un pixel près).
    copier_plan_courant(&mut b);
    assert!(b.app.activer_onglet(0));
    let avant = b.app.doc.panels.len();
    b.app.paste_panels();
    attendre(&mut b);
    assert_eq!(b.app.doc.panels.len(), avant + 1, "{}", b.app.status);
    assert!(b.app.status.contains("×2"), "{}", b.app.status);
    let p = b.app.cur_panel;
    let retour = cels_du_plan(&b.app.doc, p, 0);
    assert_eq!((retour[0].raster.width, retour[0].raster.height), (7680, 4320));
    let (cx, cy) = (3840, 2160);
    assert_eq!(retour[0].raster.get(cx + 236, cy).a(), 255);
    assert_eq!(retour[0].raster.get(cx + 246, cy).a(), 0);
    let cam = &b.app.doc.panels[p].camera;
    assert_eq!((cam[1].cam.x, cam[1].cam.dolly), (3840.0 + 960.0, 192.0));
}

#[test]
fn the_original_stays_in_the_clipboard_and_pastes_at_its_own_size_elsewhere() {
    let mut b = banc(plan_riche((320, 180), 24, 2), false);
    copier_plan(&mut b);
    let origine = b.app.panel_clipboard.as_ref().expect("copie").origine;
    nouvel_onglet(&mut b, plan_riche((640, 360), 24, 1));
    b.app.paste_panels();
    attendre(&mut b);
    let colle = b.app.panel_clipboard.as_ref().expect("la copie reste");
    assert_eq!(colle.origine, origine, "le presse-papiers n'a pas été remplacé par la copie adaptée");
    assert_eq!(colle.origine.cadre, [320, 180]);
    // Collé une seconde fois : même résultat.
    let n = b.app.doc.panels.len();
    b.app.paste_panels();
    attendre(&mut b);
    assert_eq!(b.app.doc.panels.len(), n + 1);
    // Collé dans un projet de la taille d'origine : intact, sans calcul de fond.
    nouvel_onglet(&mut b, plan_riche((320, 180), 24, 1));
    let n = b.app.doc.panels.len();
    b.app.paste_panels();
    assert_eq!(b.app.doc.panels.len(), n + 1, "tout de suite");
    assert!(b.app.collage.tache.is_none());
}

#[test]
fn a_project_of_the_same_size_pastes_exactly_as_before() {
    let mut b = banc(plan_riche((320, 180), 24, 3), false);
    copier_plan(&mut b);
    let source: Vec<Vec<u8>> = cels_du_plan(&b.app.doc, 0, 0).iter().map(|c| c.raster.to_rgba()).collect();
    nouvel_onglet(&mut b, plan_riche((320, 180), 24, 1));
    let (n, serial) = (b.app.doc.panels.len(), b.app.history.serial());
    b.app.paste_panels();
    // Tout de suite : ni question, ni calcul de fond.
    assert!(b.app.collage.dialogue.is_none() && b.app.collage.tache.is_none() && !b.app.collage_occupe());
    assert_eq!(b.app.doc.panels.len(), n + 1, "{}", b.app.status);
    assert_eq!(b.app.history.serial(), serial + 1);
    assert!(!b.app.status.contains("Collé depuis"), "rien n'a été adapté : {}", b.app.status);
    assert!(!dernier_nom(&b).contains("adapté"), "{}", dernier_nom(&b));
    let colle: Vec<Vec<u8>> = cels_du_plan(&b.app.doc, b.app.cur_panel, 0).iter().map(|c| c.raster.to_rgba()).collect();
    assert_eq!(colle, source, "les pixels sont ceux de la copie");
    let calque = &b.app.doc.panels[b.app.cur_panel].layers[0];
    assert_eq!(calque.transform.x, 32.0);
    assert_eq!(calque.fx[0].values[0], 320.0 / 96.0);
}

#[test]
fn different_proportions_ask_and_the_choice_is_applied_and_remembered() {
    let mut b = banc(plan_riche((1920, 1080), 24, 2), false);
    copier_plan(&mut b);
    nouvel_onglet(&mut b, plan_riche((1080, 1080), 24, 1));
    let n = b.app.doc.panels.len();
    b.app.paste_panels();
    assert!(b.app.collage.dialogue.is_some(), "les proportions diffèrent : on demande");
    assert_eq!(b.app.doc.panels.len(), n, "rien n'est collé avant la réponse");
    assert!(b.app.modal_open(), "la fenêtre fige les raccourcis");
    assert!(!b.app.activer_onglet(0), "on ne change pas d'onglet pendant la question");
    b.image(Vec::new(), 0.016);
    b.image(Vec::new(), 0.016);
    // Annuler ne colle rien et laisse la copie.
    b.app.collage_annuler();
    assert!(b.app.collage.dialogue.is_none() && b.app.panel_clipboard.is_some());
    assert_eq!(b.app.doc.panels.len(), n);

    // « Remplir », sans redemander la prochaine fois.
    b.app.paste_panels();
    {
        let d = b.app.collage.dialogue.as_mut().expect("question");
        assert_eq!((d.ajustement, d.duree), (Ajustement::Contenir, Duree::Images), "les réglages d'origine");
        d.ajustement = Ajustement::Remplir;
        d.ne_plus_demander = true;
    }
    b.app.collage_valider();
    attendre(&mut b);
    assert_eq!(b.app.doc.panels.len(), n + 1, "{}", b.app.status);
    assert!(b.app.status.contains("cadre rempli"), "{}", b.app.status);
    assert_eq!(b.app.settings.collage.ajustement, Ajustement::Remplir);
    assert!(!b.app.settings.collage.demander);
    // Remplir : l'échelle est 1 (1080 / 1080), le calque garde sa pose ; la feuille est recentrée d'un décalage entier.
    let p = b.app.cur_panel;
    let fond = cels_du_plan(&b.app.doc, p, 0);
    assert_eq!(fond[0].raster.get(1080 + 110, 1080).a(), 255, "le rectangle de ±120 reste de ±120, centré");
    assert_eq!(fond[0].raster.get(1080 + 130, 1080).a(), 0);
    assert_eq!(b.app.doc.panels[p].layers[0].transform.x, 192.0);
    // La fois suivante : plus de question, et le même choix.
    let n = b.app.doc.panels.len();
    b.app.paste_panels();
    assert!(b.app.collage.dialogue.is_none());
    attendre(&mut b);
    assert_eq!(b.app.doc.panels.len(), n + 1);
    assert!(b.app.status.contains("cadre rempli"), "{}", b.app.status);
}

#[test]
fn contain_keeps_the_whole_source_frame_visible() {
    let mut b = banc(plan_riche((1920, 1080), 24, 1), false);
    copier_plan(&mut b);
    nouvel_onglet(&mut b, plan_riche((1080, 1080), 24, 1));
    b.app.paste_panels();
    assert!(b.app.collage.dialogue.is_some());
    b.app.collage_valider();
    attendre(&mut b);
    assert!(b.app.status.contains("×0,56") && b.app.status.contains("cadre contenu"), "{}", b.app.status);
    let p = b.app.cur_panel;
    let calque = &b.app.doc.panels[p].layers[0];
    assert!((calque.transform.x - 192.0 * 0.5625).abs() < 1e-3);
    let Element::Stroke { points, .. } = &cels_du_plan(&b.app.doc, p, 1)[0].vector.as_ref().expect("vectoriel").elements[0] else { panic!("un trait") };
    assert!((points[0].x - (1080.0 - 240.0 * 0.5625)).abs() < 1e-2, "{}", points[0].x);
}

#[test]
fn a_different_rate_asks_and_keeps_either_the_images_or_the_seconds() {
    let mut b = banc(plan_riche((320, 180), 24, 24), false);
    copier_plan(&mut b);
    let n_source = b.app.doc.panels[0].frame_count();
    nouvel_onglet(&mut b, plan_riche((320, 180), 25, 1));
    // Garder les images (le réglage d'origine) : rien d'adapté dans l'espace, la durée change et le dit.
    let n = b.app.doc.panels.len();
    b.app.paste_panels();
    assert!(b.app.collage.dialogue.is_some(), "la cadence diffère : on demande");
    b.app.collage_valider();
    attendre(&mut b);
    assert_eq!(b.app.doc.panels.len(), n + 1, "{}", b.app.status);
    assert_eq!(b.app.doc.panels[b.app.cur_panel].frame_count(), n_source, "les mêmes images");
    assert!(b.app.status.contains("24 i/s") && b.app.status.contains("25 i/s"), "{}", b.app.status);
    // Garder la durée en secondes : 24 images à 24 i/s deviennent 25 images à 25 i/s.
    b.app.paste_panels();
    b.app.collage.dialogue.as_mut().expect("question").duree = Duree::Secondes;
    b.app.collage_valider();
    attendre(&mut b);
    let p = b.app.cur_panel;
    assert_eq!(b.app.doc.panels[p].frame_count(), 25, "{}", b.app.status);
    assert_eq!(b.app.doc.panels[p].layers[0].frames.len(), 25);
    assert_eq!(b.app.doc.panels[p].camera[1].frame, 13, "12 → 12,5 arrondi vers le haut");
    assert_eq!(b.app.doc.panels[p].layers[0].pos_keys[1].frame, 8, "8 × 25 / 24 = 8,33");
    assert!(b.app.status.contains("durées gardées en secondes (24 → 25 i/s)"), "{}", b.app.status);
    assert_eq!(dernier_nom(&b), "Coller des plans (adapté de 320×180)");
}

#[test]
fn a_copy_of_unknown_origin_pastes_as_is() {
    // Une copie dont l'origine n'est pas connue (rien n'a été copié dans cette séance) se colle telle quelle.
    let mut b = banc(plan_riche((320, 180), 24, 1), false);
    b.app.cur_frame = 0;
    b.app.pixel_clipboard = Some(crate::pixsel::Floating { x0: 4, y0: 4, w: 2, h: 2, pixels: vec![ROUGE; 4] });
    assert!(b.app.collage.origines.zone.is_none());
    b.app.bitmap_paste();
    let id = b.app.doc.panels[0].layers[0].frames[0].expect("dessin");
    assert_eq!(b.app.doc.cels[&id].raster.get(4, 4), ROUGE);
}

// ---------------------------------------------------------------------------------------------- les autres copies

/// Un premier projet de 6 images (320 × 180), prêt à copier.
fn petit_projet() -> Banc {
    banc(plan_riche((320, 180), 24, 6), false)
}

#[test]
fn a_layer_pasted_from_a_smaller_project_is_scaled() {
    let mut b = petit_projet();
    b.app.cur_layer = 0;
    b.app.copy_layer();
    nouvel_onglet(&mut b, plan_riche((640, 360), 24, 6));
    let (n, serial) = (b.app.layers().len(), b.app.history.serial());
    b.app.paste_layer();
    attendre(&mut b);
    assert_eq!(b.app.layers().len(), n + 1, "{}", b.app.status);
    assert_eq!(b.app.history.serial(), serial + 1);
    assert!(b.app.status.contains("mis à l'échelle ×2"), "{}", b.app.status);
    let colle = &b.app.layers()[b.app.cur_layer];
    assert_eq!(colle.transform.x, 64.0, "32 × 2");
    assert!((colle.fx[0].values[0] - 320.0 / 96.0 * 2.0).abs() < 1e-4, "le flou de 3,33 devient 6,67 : {}", colle.fx[0].values[0]);
    let id = colle.frames[0].expect("image");
    let cel = &b.app.doc.cels[&id];
    assert_eq!((cel.raster.width, cel.raster.height), (1280, 720));
    assert_eq!(cel.raster.get(640 + 38, 360).a(), 255, "±20 devient ±40");
    assert_eq!(cel.raster.get(640 + 42, 360).a(), 0);
    assert_eq!(dernier_nom(&b), "Coller un calque (adapté de 320×180)");
    b.app.undo();
    assert_eq!(b.app.layers().len(), n);
}

#[test]
fn images_and_montage_keys_pasted_from_another_size_are_scaled() {
    let mut b = petit_projet();
    b.app.frame_sel = Some((1, 3));
    b.app.images_focus = true;
    b.app.copy_images();
    b.app.frame_sel = None;
    b.app.cur_frame = 0;
    b.app.copy_keys();
    assert!(b.app.key_clipboard.is_some(), "{}", b.app.status);
    nouvel_onglet(&mut b, plan_riche((640, 360), 24, 6));
    b.app.images_focus = true;
    b.app.cur_layer = 0;
    b.app.cur_frame = 4;
    b.app.paste_images(false);
    attendre(&mut b);
    let id = b.app.layer().frames[4].expect("image collée");
    let cel = &b.app.doc.cels[&id];
    assert_eq!((cel.raster.width, cel.raster.height), (1280, 720));
    // L'image 1 de la source avait son rectangle de x = 303 à 343 : 606 à 686 dans la feuille d'arrivée.
    assert_eq!(cel.raster.get(650, 360).a(), 255, "{}", b.app.status);
    assert_eq!(cel.raster.get(700, 360).a(), 0);
    assert!(b.app.status.contains("mis à l'échelle ×2"), "{}", b.app.status);
    // Une clé du montage.
    b.app.cur_frame = 0;
    b.app.paste_keys_at_head();
    attendre(&mut b);
    let id = b.app.layer().frames[0].expect("clé collée");
    assert_eq!(b.app.doc.cels[&id].raster.width, 1280, "{}", b.app.status);
    assert!(b.app.status.contains("Collé depuis un projet 320×180"), "{}", b.app.status);
}

#[test]
fn camera_keys_effects_elements_and_zones_pasted_from_another_size_are_scaled() {
    let mut b = petit_projet();
    // Clés de caméra.
    let id = b.app.doc.panels[0].id;
    b.app.cam_key_sel = vec![crate::camkeys::CamKeyRef { panel: id, frame: 0 }, crate::camkeys::CamKeyRef { panel: id, frame: 3 }];
    b.app.copy_cam_keys();
    assert!(b.app.cam_key_clipboard.is_some(), "{}", b.app.status);
    // Effets.
    b.app.fx_copy_all(0);
    assert!(b.app.fx_stack_clipboard.is_some(), "{}", b.app.status);
    // Éléments vectoriels.
    b.app.cur_layer = 1;
    let cel = b.app.doc.panels[0].layers[1].frames[0].expect("dessin");
    b.app.vsel = Some(crate::vtools::VSelection { cel, items: vec![0] });
    assert!(b.app.copy_selection());
    // Une zone de pixels, à droite du centre.
    b.app.pixel_clipboard = Some(crate::pixsel::Floating { x0: 420, y0: 180, w: 20, h: 10, pixels: vec![ROUGE; 200] });
    b.app.collage.origines.zone = Some(Origine::de(&b.app.doc));
    nouvel_onglet(&mut b, plan_riche((640, 360), 24, 6));
    b.app.cur_panel = 0;
    // Caméra : la tête de lecture à l'image 2, les clés arrivent avec leur écart.
    b.app.cur_frame = 2;
    b.app.paste_cam_keys_at_head();
    let cles = &b.app.doc.panels[0].camera;
    let colle = cles.iter().find(|k| k.frame == 5).expect("la clé de l'image 3 arrive à l'image 5");
    assert_eq!(colle.cam.x, 640.0 + 80.0 * 2.0, "l'écart horizontal est doublé : {}", b.app.status);
    assert_eq!(colle.cam.dolly, 16.0 * 2.0);
    assert!(b.app.status.contains("mis à l'échelle ×2"), "{}", b.app.status);
    // Effets : le flou de 3,33 px devient 6,67.
    b.app.cur_layer = 0;
    b.app.layers_mut()[0].fx.clear();
    b.app.fx_paste(vec![0], false);
    assert!((b.app.layers()[0].fx[0].values[0] - 320.0 / 96.0 * 2.0).abs() < 1e-4, "{}", b.app.status);
    // Éléments : le trait, doublé.
    b.app.cur_layer = 1;
    b.app.cur_frame = 0;
    b.app.paste();
    let cel = b.app.doc.panels[0].layers[1].frames[0].expect("dessin");
    let elements = &b.app.doc.cels[&cel].vector.as_ref().expect("vectoriel").elements;
    assert_eq!(elements.len(), 2, "{}", b.app.status);
    let Element::Stroke { points, .. } = &elements[1] else { panic!("un trait") };
    assert_eq!((points[0].x, points[0].y, points[0].r), (640.0 - 80.0, 360.0 + 90.0, 2.0 * 320.0 / 240.0));
    // Zone : plus grande, au même endroit dans le cadre (de x = 420 à 440 devient de 840 à 880).
    b.app.cur_layer = 0;
    b.app.bitmap_paste();
    let sel = b.app.bsel.as_ref().expect("zone soulevée");
    let r = &b.app.doc.cels[&sel.cel].raster;
    assert_eq!(r.get(850, 365).a(), 255, "{}", b.app.status);
    assert_eq!(r.get(810, 365).a(), 0);
    assert_eq!(r.get(900, 365).a(), 0);
}

#[test]
fn a_selection_on_several_layers_pastes_onto_the_chosen_layers_of_another_project() {
    use crate::multisel::{ClipWhat, MultiClip};
    let mut b = petit_projet();
    let source = b.app.doc.panels[0].layers.iter().map(|l| l.id).collect::<Vec<_>>();
    let trait_ = Element::Stroke {
        points: vec![VPoint { x: 300.0, y: 200.0, r: 3.0 }, VPoint { x: 340.0, y: 200.0, r: 3.0 }],
        color: [9, 9, 9],
        opacity: 1.0,
        hardness: 1.0,
        brush: None,
        trace: Default::default(),
    };
    b.app.multi_clip = Some(MultiClip {
        items: vec![
            (source[1], ClipWhat::Vector(vec![trait_])),
            (source[0], ClipWhat::Pixels(crate::pixsel::Floating { x0: 420, y0: 180, w: 10, h: 10, pixels: vec![ROUGE; 100] })),
        ],
    });
    b.app.collage.origines.multi = Some(Origine::de(&b.app.doc));
    nouvel_onglet(&mut b, plan_riche((640, 360), 24, 6));
    // Les deux calques de ce projet sont choisis : le fond (bitmap) et le trait (vectoriel).
    b.app.cur_layer = 0;
    let trait_id = b.app.layers()[1].id;
    b.app.layer_sel = vec![trait_id];
    b.app.cur_frame = 0;
    b.app.multi_paste();
    assert!(b.app.multi.sel.is_some(), "{}", b.app.status);
    let cel = b.app.doc.panels[0].layers[1].frames[0].expect("dessin");
    let elements = &b.app.doc.cels[&cel].vector.as_ref().expect("vectoriel").elements;
    let Element::Stroke { points, .. } = elements.last().expect("trait collé") else { panic!("un trait") };
    assert_eq!((points[0].x, points[0].y, points[0].r), (640.0 + (300.0 - 320.0) * 2.0, 360.0 + (200.0 - 180.0) * 2.0, 6.0));
    let id = b.app.doc.panels[0].layers[0].frames[0].expect("dessin");
    assert_eq!(b.app.doc.cels[&id].raster.get(845, 365).a(), 255, "la zone de pixels est sur le calque bitmap, doublée");
}

// ---------------------------------------------------------------------------------------------- sans latence

#[test]
fn a_big_paste_never_blocks_the_interface_thread() {
    // Un plan 4K de 3 calques × 24 dessins (un fond bitmap, un trait vectoriel, une peinture bitmap qui couvre le
    // quart d'une zone de 3 840 × 1 920), collé dans un projet 1080.
    let mut doc = plan_riche((3840, 2160), 24, 24);
    let (cx, cy) = (doc.width / 2, doc.height / 2);
    let mut peinture = Layer::new("Peinture", LayerKind::Bitmap);
    for k in 0..24 {
        let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
        for ty in 0..30usize {
            for tx in 0..60usize {
                if (tx + ty + k) % 4 == 0 {
                    let i = cel.raster.tile_index_of(cx - 1920 + tx * 64, cy - 960 + ty * 64);
                    let tuile = cel.raster.tile_mut(i);
                    for (j, p) in tuile.iter_mut().enumerate() {
                        *p = Color32::from_rgba_premultiplied((j % 251) as u8 / 2, (tx * 4) as u8 / 2, (ty * 8) as u8 / 2, 128);
                    }
                }
            }
        }
        peinture.frames.push(Some(doc.add_cel(cel)));
    }
    doc.panels[0].layers.push(peinture);
    let mut b = banc(doc, false);
    copier_plan(&mut b);
    nouvel_onglet(&mut b, plan_riche((1920, 1080), 24, 1));
    let n = b.app.doc.panels.len();
    let t = Instant::now();
    b.app.paste_panels();
    let lancement = t.elapsed();
    assert!(b.app.collage.tache.is_some(), "le calcul part sur un fil de fond : {}", b.app.status);
    assert_eq!(b.app.doc.panels.len(), n, "rien n'est collé avant la fin");
    assert!(b.app.modal_open() && b.app.status.contains("Collage en cours"), "{}", b.app.status);
    assert!(!b.app.activer_onglet(0), "pas de changement d'onglet pendant le calcul : {}", b.app.status);
    let debut = Instant::now();
    let pire = attendre(&mut b);
    let total = debut.elapsed();
    assert_eq!(b.app.doc.panels.len(), n + 1, "{}", b.app.status);
    eprintln!("collage 4K -> 1080 (72 dessins) : lancement {lancement:?}, calcul {total:?}, pire image d'écran {pire:?}");
    // Le fil de l'interface ne fait que lancer, puis coller : jamais le rééchantillonnage.
    assert!(lancement < Duration::from_millis(60), "lancer le collage prend {lancement:?}");
    assert!(pire < Duration::from_millis(400), "une image d'écran a pris {pire:?} pendant le collage");
}

#[test]
fn the_dialog_is_a_modal_window_that_draws_without_trouble() {
    let mut b = banc(plan_riche((1920, 1080), 24, 1), false);
    copier_plan(&mut b);
    nouvel_onglet(&mut b, plan_riche((1080, 1080), 25, 1));
    b.app.paste_panels();
    assert!(b.app.collage.dialogue.is_some());
    for _ in 0..3 {
        b.image(Vec::new(), 0.016);
    }
    assert!(b.app.collage.dialogue.is_some(), "la fenêtre attend la réponse");
    assert!(b.app.modal_open());
    b.app.collage_annuler();
    assert!(!b.app.collage_occupe());
}

/// Mesures pour `docs/PERFORMANCE.md` (à lancer à la main : `cargo test --release perf_collage -- --ignored --nocapture`).
#[test]
#[ignore]
fn perf_collage() {
    use crate::adaptation::{reechantillonne, Adaptation};
    let rempli = |w: usize, h: usize| {
        let mut r = crate::raster::Raster::new(w, h);
        for i in 0..r.tiles.len() {
            let t = r.tile_mut(i);
            for (j, p) in t.iter_mut().enumerate() {
                *p = Color32::from_rgba_premultiplied((j % 251) as u8 / 2, (i % 200) as u8 / 2, 20, 128);
            }
        }
        r
    };
    let orig = |w: usize, h: usize| Origine { cadre: [w, h], feuille: [w * 2, h * 2], cadence: 24 };
    let fils = std::thread::available_parallelism().map_or(2, |n| n.get());
    for (nom, de, vers) in [("4K -> 1080", (3840, 2160), (1920, 1080)), ("1080 -> 4K", (1920, 1080), (3840, 2160)), ("4K -> 720", (3840, 2160), (1280, 720))] {
        let src = rempli(de.0 * 2, de.1 * 2);
        let a = Adaptation::new(orig(de.0, de.1), orig(vers.0, vers.1), Ajustement::Contenir, Duree::Images);
        for n in [1, fils.saturating_sub(1).max(1)] {
            let t = Instant::now();
            let out = reechantillonne(&src, a.s, a.t, vers.0 * 2, vers.1 * 2, n);
            eprintln!("{nom} : un dessin plein sur toute la feuille, {n} fil(s) : {:?} ({} tuiles)", t.elapsed(), out.tiles.iter().flatten().count());
        }
    }
}