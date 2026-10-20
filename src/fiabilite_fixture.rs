//! Le projet d'essai de la fiabilité (`docs/FIABILITE.md`) : un projet fabriqué par le code, qui
//! contient tout ce que SHOT sait enregistrer, pour que les essais d'aller-retour, de fumée et de
//! robustesse travaillent sur un projet « comme celui d'Anthony » et non sur un cas d'école.
//!
//! Il contient : cinq plans dans deux séquences (noms de séquences et de plans) ; des calques
//! bitmap et vectoriels ; des traits vectoriels texturés avec leur trace (appui, instants,
//! toucher) ; le « fond sous le trait » (réglages du calque, strate peinte et coupe d'un dessin) ;
//! des effets de calque avec clés (flou, bruit, teinte-saturation, tremblement du trait), dont un
//! éteint ; un mode de fusion, de l'opacité, une transformation, de la profondeur ; un dossier de
//! calques et un de ses calques ; un calque CTG ; des calques d'effets (un ajustement à clés et une
//! incrustation, format 27) ; un calque vidéo (dix images PNG minuscules
//! fabriquées sur disque) ; des rôles d'images, des noms d'instance, des marques de couleur ; une
//! caméra à clés avec adoucis, un effet de caméra posé (explosion) et un repère 3D (un cube) ;
//! des marques d'image, des signets, des points d'entrée et de sortie, des notes dessinées, la
//! couleur d'un plan ; des commentaires (dont un rangé hors du plan rogné) ; un plan rogné au début
//! et à la fin, avec sa réserve ; un son (WAV fabriqué) sur deux pistes ; les guides de
//! perspective, les notes de la planche, les groupes de couleur, l'arrière-plan en damier, les
//! propriétés du projet, le volume général, l'image de départ ; des transitions du montage ; les
//! effets des extraits de son et le solo d'une piste ; des sons proposés (suggestions sonores) ; des
//! références visuelles (une image, une vidéo de trois images) ; la planche de crayonnés (deux pages,
//! des traits lisses, une case peinte et cochée, une légende seule).
//!
//! Il ne contient pas : de texte vectoriel dont l'image soit comparée (son rendu dépend des
//! polices du poste, voir `Fixture::sans_pixels`) et de calques « toute la scène » (un projet d'avant
//! les plans, devenus des calques ordinaires à l'ouverture).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::{pos2, Color32};

use crate::blend::BlendMode;
use crate::brushtex::BrushLibrary;
use crate::camera::{Camera, CameraKey};
use crate::camfx::{self, Generator, Params, Preset};
use crate::document::{Behaviour, Cel, CelId, Cues, Document, Layer, LayerKind, LayerTransform, Ref3d, Strate, TimeNote};
use crate::fond::FondReglages;
use crate::fx::{Effect, EffectKind};
use crate::raster::{premultiply, Raster, TILE};
use crate::roles::Role;
use crate::vector::{Element, Trace, VPoint, VectorDrawing};
use crate::video::VideoClip;
use crate::view3d::Cam3d;

/// Taille de la feuille de dessin du projet d'essai (le cadre fait la moitié).
pub(crate) const FEUILLE: (usize, usize) = (480, 270);

/// Le projet d'essai et ce qui sert à le comparer.
pub(crate) struct Fixture {
    pub doc: Document,
    /// Dessins dont l'image n'est pas comparée : un dessin vectoriel à texte (son rendu dépend des
    /// polices installées, que SHOT lit en arrière-plan : l'image n'est pas la même avant et après).
    pub sans_pixels: BTreeSet<CelId>,
}

// ------------------------------------------------------------------ petits outils

/// Un dessin bitmap fait de rectangles de couleurs vives et de transparences, selon `seed`.
fn bitmap(seed: u32) -> Cel {
    let (w, h) = FEUILLE;
    let mut r = Raster::new(w, h);
    let mut poser = |x0: usize, y0: usize, x1: usize, y1: usize, c: [u8; 4]| {
        let color = premultiply(c[0], c[1], c[2], c[3]);
        for y in y0..y1.min(h) {
            for x in x0..x1.min(w) {
                let i = r.tile_index_of(x, y);
                r.tile_mut(i)[(y % TILE) * TILE + x % TILE] = color;
            }
        }
    };
    let s = seed as usize;
    poser(10 + s * 7 % 90, 12 + s * 5 % 60, 120 + s * 11 % 100, 90 + s * 3 % 50, [200, (40 + s * 20 % 200) as u8, 60, 255]);
    poser(150 + s * 13 % 120, 70 + s * 9 % 80, 300 + s * 17 % 90, 200, [30, 90, (120 + s * 40 % 130) as u8, 128]);
    poser(300, 20 + s * 4 % 40, 330, 230, [20, 20, 20, 30]);
    Cel::bitmap(r)
}

/// L'image d'un dessin vectoriel passée par le fichier : les appuis et les instants d'une trace sont
/// enregistrés en entiers (`vector.rs`, « invisible au rendu »). Le projet d'essai part déjà de ces
/// valeurs, pour qu'un aller-retour rende exactement ce qu'on lui a donné.
fn par_le_fichier(d: &VectorDrawing) -> VectorDrawing {
    serde_json::from_str(&serde_json::to_string(d).expect("écriture")).expect("relecture")
}

/// Un trait texturé (réglage de la bibliothèque `preset`) le long de `pts`, avec sa trace.
fn trait_texture(lib: &BrushLibrary, preset: &str, taille: f32, depart: (f32, f32), pente: (f32, f32)) -> Element {
    let mut s = lib.presets.iter().find(|p| p.name == preset).expect("brosse de SHOT").settings;
    s.size = taille;
    let pts: Vec<_> = (0..=40)
        .map(|i| {
            let t = i as f32 / 40.0;
            (pos2(depart.0 + pente.0 * t, depart.1 + pente.1 * t + (t * 7.0).sin() * 12.0), 0.3 + 0.7 * (t * std::f32::consts::PI).sin(), i as f64 / 100.0)
        })
        .collect();
    crate::affichage_tests::trait_rouge(&s, lib, FEUILLE.0, FEUILLE.1, &pts)
}

fn dessin_vectoriel(doc: &mut Document, elements: Vec<Element>) -> CelId {
    let id = doc.new_cel(LayerKind::Vector);
    let cel = doc.cels.get_mut(&id).expect("dessin neuf");
    cel.vector = Some(par_le_fichier(&VectorDrawing { elements }));
    cel.rerender();
    cel.version = 0;
    id
}

fn dessin_bitmap(doc: &mut Document, seed: u32) -> CelId {
    doc.add_cel(bitmap(seed))
}

/// Un son WAV de 16 bits, mono, à 48 kHz : `n` échantillons d'une onde en dents de scie.
fn wav(n: usize) -> Vec<u8> {
    let data_len = (n * 2) as u32;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&48_000u32.to_le_bytes());
    b.extend_from_slice(&96_000u32.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..n {
        b.extend_from_slice(&(((i % 200) as i16 - 100) * 90).to_le_bytes());
    }
    b
}

fn cam(x: f32, y: f32, zoom: f32, angle: f32) -> Camera {
    Camera { x, y, zoom, angle, dolly: 0.0, tilt: 0.0, pan: 0.0 }
}

// ------------------------------------------------------------------ le projet

/// Fabrique le projet d'essai. `travail` : un dossier (neuf) où poser les images de la vidéo.
pub(crate) fn projet_complet(travail: &Path) -> Fixture {
    let (w, h) = FEUILLE;
    let lib = BrushLibrary::builtin();
    lib.share();
    let mut doc = Document::with_frame(w / 2, h / 2, 24);
    let mut sans_pixels = BTreeSet::new();

    // Séquences et plans : deux séquences nommées, cinq plans (le premier existe déjà).
    let i1 = doc.add_scene_after(0);
    let i2 = doc.add_scene_after(i1);
    let i3 = doc.add_sequence_after(i2);
    let i4 = doc.add_scene_after(i3);
    assert_eq!((i1, i2, i3, i4), (1, 2, 3, 4));
    let (seq1, seq2) = (doc.sequence_of(0), doc.sequence_of(i3));
    doc.sequence_mut(seq1).expect("séquence").name = "Prologue".into();
    doc.sequence_mut(seq2).expect("séquence").name = "La course — « poursuite »".into();
    let scene1 = doc.panels[i1].scene;
    doc.scene_mut(scene1).expect("scène").name = "Arrivée".into();

    // ---------------------------------------------------------------- plan 1 : le plan complet
    let fond = dessin_bitmap(&mut doc, 1);
    let nuages = dessin_bitmap(&mut doc, 2);
    let aplats = dessin_bitmap(&mut doc, 3);
    let perso1 = dessin_vectoriel(
        &mut doc,
        vec![
            trait_texture(&lib, "Gouache", 18.0, (40.0, 60.0), (200.0, 40.0)),
            trait_texture(&lib, "Feutre", 10.0, (60.0, 180.0), (260.0, -60.0)),
            Element::Fill { contours: vec![vec![[300.0, 40.0], [380.0, 60.0], [350.0, 120.0]]], color: [220, 180, 40], opacity: 0.75 },
        ],
    );
    let perso2 = dessin_vectoriel(&mut doc, vec![trait_texture(&lib, "Fusain", 22.0, (80.0, 200.0), (180.0, -120.0))]);
    let perso3 = dessin_vectoriel(&mut doc, vec![trait_texture(&lib, "Gouache", 14.0, (120.0, 40.0), (150.0, 150.0))]);
    // Le fond sous le trait du premier dessin du personnage : de la peinture, et une coupe.
    {
        let mut strate = Strate::new(w, h);
        let mut peinture = Raster::new(w, h);
        let mut coupe = Raster::new(w, h);
        for y in 90..130 {
            for x in 100..170 {
                let i = peinture.tile_index_of(x, y);
                peinture.tile_mut(i)[(y % TILE) * TILE + x % TILE] = premultiply(240, 120, 90, 255);
            }
        }
        for y in 100..110 {
            for x in 120..140 {
                let i = coupe.tile_index_of(x, y);
                coupe.tile_mut(i)[(y % TILE) * TILE + x % TILE] = Color32::WHITE;
            }
        }
        strate.peinture.raster = peinture;
        strate.coupe = coupe;
        doc.cels.get_mut(&perso1).expect("dessin").fond = Some(Box::new(strate));
    }
    let mut calque_fond = Layer::new("Fond", LayerKind::Bitmap);
    calque_fond.frames = vec![Some(fond); 24];
    calque_fond.group = 2;
    calque_fond.names.insert(fond, "Décor de jour".into());
    calque_fond.color_marks.insert(fond, 3);

    // La vidéo : dix images PNG minuscules dans un dossier d'origine, enregistré dans le projet.
    let clip = VideoClip { id: crate::video::new_id(), name: "rushes-plan1.mp4".into(), frames: 10, fps: 12.0, start: -2, ext: "png".into() };
    let source = travail.join("videos-origine").join(clip.id.to_string());
    std::fs::create_dir_all(&source).expect("dossier des images de la vidéo");
    for k in 0..clip.frames {
        image::RgbaImage::from_pixel(24, 14, image::Rgba([(k * 25) as u8, 90, 200, 255])).save(crate::video::frame_file(&source, k, "png")).expect("image de la vidéo");
    }
    crate::video::register_dir(clip.id, source);
    let mut calque_video = Layer::new("Rushes", LayerKind::Bitmap);
    calque_video.video = Some(clip);
    calque_video.opacity = 0.5;
    calque_video.blend = BlendMode::Screen;

    let mut perso = Layer::new("Personnage", LayerKind::Vector);
    perso.frames = [vec![Some(perso1); 8], vec![Some(perso2); 8], vec![None; 4], vec![Some(perso3); 4]].concat();
    perso.fond = Some(FondReglages { couleur: [250, 240, 200], debord: 4, trous: 7, bord: false });
    perso.blend = BlendMode::Multiply;
    perso.opacity = 0.8;
    perso.transform = LayerTransform { x: 12.5, y: -4.0, scale: 1.1, angle: 3.0, stretch_x: 1.2, stretch_y: 0.9 };
    perso.depth = 0.5;
    perso.pre = Behaviour::Loop;
    perso.post = Behaviour::PingPong;
    perso.onion = true;
    perso.keep_alpha = true;
    perso.auto_break = true;
    perso.auto_create = false;
    perso.group = 7;
    perso.names.insert(perso2, "Il court".into());
    perso.color_marks.insert(perso3, 5);
    perso.roles.insert(0, Role::Key);
    perso.roles.insert(8, Role::Breakdown);
    perso.roles.insert(12, Role::Inbetween);
    perso.roles.insert(20, Role::SpecialDone);
    // Effets : tremblement du trait (clés), flou (clés), bruit (éteint), teinte-saturation (clés).
    let mut tremble = Effect::new(EffectKind::Boil);
    tremble.set_param(0, 4.5);
    tremble.set_key(0, tremble.values.clone());
    let mut fin = tremble.values.clone();
    fin[1] = 80.0;
    tremble.set_key(15, fin);
    let mut flou = Effect::new(EffectKind::GaussianBlur);
    flou.set_key(2, vec![0.0]);
    flou.set_key(10, vec![6.5]);
    let mut bruit = Effect::new(EffectKind::Noise);
    bruit.enabled = false;
    bruit.set_param(0, 35.0);
    let mut teinte = Effect::new(EffectKind::HueSaturation);
    teinte.set_key(0, vec![0.0, 0.0, 0.0]);
    teinte.set_key(23, vec![-90.0, 40.0, -10.0]);
    // Effets du trait (`fxtrait.rs`) : écriture (clés, effacement), crayonné (éteint).
    let mut ecriture = Effect::new(EffectKind::Write);
    ecriture.set_param(crate::fxtrait::WRITE_SOFT, 12.0);
    ecriture.set_key(0, ecriture.values.iter().enumerate().map(|(k, v)| if k == crate::fxtrait::WRITE_PROGRESS { 0.0 } else { *v }).collect());
    ecriture.set_key(14, ecriture.values.iter().enumerate().map(|(k, v)| if k == crate::fxtrait::WRITE_PROGRESS { 100.0 } else { *v }).collect());
    let mut crayonne = Effect::new(EffectKind::Sketch);
    crayonne.enabled = false;
    crayonne.set_param(crate::fxtrait::SKETCH_PASSES, 3.0);
    perso.fx = vec![tremble, flou, bruit, teinte, ecriture, crayonne];

    let mut dossier = Layer::new_folder("Décors");
    dossier.collapsed = true;
    dossier.opacity = 0.9;
    let mut nuage = Layer::new("Nuages", LayerKind::Bitmap);
    nuage.frames = [vec![Some(nuages); 12], vec![None; 12]].concat();
    nuage.parent = Some(dossier.id);
    nuage.stencil = true;
    nuage.stencil_negative = true;
    nuage.position_locked = true;
    nuage.fx = vec![{
        let mut e = Effect::new(EffectKind::Glow);
        e.set_key(0, vec![12.0, 100.0]);
        e.set_key(11, vec![40.0, 150.0]);
        e
    }];
    let mut ctg = Layer::new("Aplats", LayerKind::Bitmap);
    ctg.frames = vec![Some(aplats); 24];
    ctg.ctg = Some(crate::ctg::Ctg { sources: vec![perso.id], gap: 6 });
    ctg.locked = true;
    ctg.visible = false;
    // Calques d'effets (format 27, `calqueffet.rs`) : un ajustement à clés (image d'impact, secousse,
    // lueur diffuse ; opacité, mode de fusion, images 6 à 8 seulement) et une incrustation réglée à
    // fond (texte libre à mots remplacés, boîte, « seulement à l'export »).
    let pose = doc.new_cel(LayerKind::Bitmap);
    let mut ajustement = Layer::new_effect("Impact", crate::calqueffet::CalqueEffet::ajustement("Image d'impact (impact frame)"));
    ajustement.frames = [vec![None; 6], vec![Some(pose); 3]].concat();
    ajustement.opacity = 0.8;
    ajustement.blend = BlendMode::Screen;
    let mut impact = Effect::new(EffectKind::Impact);
    impact.set_key(6, vec![100.0, 1.0, 128.0, 230.0, 30.0, 30.0, 60.0, 70.0, 40.0, 50.0, 50.0, 7.0]);
    impact.set_key(8, vec![0.0, 1.0, 128.0, 230.0, 30.0, 30.0, 60.0, 70.0, 40.0, 50.0, 50.0, 7.0]);
    let mut secousse = Effect::new(EffectKind::Shake);
    secousse.set_key(6, vec![6.0, -4.0, 1.5, 102.0, 40.0]);
    secousse.set_key(7, vec![-3.0, 2.0, -0.5, 100.0, 40.0]);
    let mut lueur = Effect::new(EffectKind::Bloom);
    lueur.set_param(1, 120.0);
    ajustement.fx = vec![impact, secousse, lueur];
    let pose_tc = doc.new_cel(LayerKind::Bitmap);
    let mut timecode = Layer::new_effect(
        "Timecode",
        crate::calqueffet::CalqueEffet::incrustation(
            crate::incrustation::Incrustation {
                source: crate::incrustation::Source::Libre,
                texte: "{plan} · {timecode} · {image}".into(),
                portee: crate::incrustation::Portee::Plan,
                ancre: crate::incrustation::Ancre::HautDroite,
                taille: 4.5,
                couleur: [250, 240, 120, 230],
                fond: Some([10, 20, 30, 170]),
                marge: 2.5,
                quand: crate::incrustation::Quand::ExportSeulement,
                police: "Ubuntu Light".into(),
            },
            "Timecode et incrustations",
        ),
    );
    timecode.frames = vec![Some(pose_tc); 24];
    let mut calques = vec![calque_fond, calque_video, perso, nuage, dossier, ctg, ajustement, timecode];
    crate::folders::normalize(&mut calques);
    doc.panels[0].layers = calques;
    doc.panels[0].length = 24;
    doc.panels[0].action = "Léa traverse la cour,\nelle n'a pas vu la voiture.".into();
    doc.panels[0].dialogue = "LÉA : « On part. Maintenant. »".into();
    doc.panels[0].notes = "Plan d'ouverture — ½ ralenti, caméra épaule.".into();
    doc.panels[0].color = Some(2);

    // La caméra : clés avec adoucis, un effet « explosion » posé par-dessus.
    {
        let c = &mut doc.panels[0].camera;
        c.push(CameraKey { ease_in: 3.0, ease_out: 4.5, ..CameraKey::new(0, cam(240.0, 135.0, 1.0, 0.0)) });
        c.push(CameraKey { smooth: true, ..CameraKey::new(12, cam(200.0, 120.0, 1.5, 4.0)) });
        let mut t = cam(210.0, 130.0, 1.25, -2.0);
        t.dolly = 30.0;
        t.tilt = 5.0;
        t.pan = -7.5;
        c.push(CameraKey::new(23, t));
        let params = Params { duration: 5, ..Params::new(Preset::Explosion, 24) }.sanitized(24);
        let g = Generator::new(&params, 24);
        let extra: Vec<usize> = doc.panels[0].camera.iter().map(|k| k.frame).collect();
        let len = doc.panels[0].frame_count();
        let plan = camfx::plan(&g, doc.frame, 14, len, &extra, true, &|f| doc.panel_camera(0, f));
        assert!(plan.last_frame().is_some(), "l'effet de caméra pose des clés");
        camfx::write(&mut doc.panels[0].camera, &plan);
    }
    // Marques d'image (avec leurs textes), signets, entrée et sortie, notes dessinées.
    doc.toggle_mark(0, 8);
    doc.toggle_mark(0, 16);
    for (k, m) in doc.panels[0].marks.iter_mut().enumerate() {
        m.action = format!("Image {} : elle se retourne", k + 2);
        m.dialogue = "« Tu viens ? »".into();
        m.notes = "Plan serré".into();
        m.color = 4 + k as u8;
    }
    doc.panels[0].cues = Cues {
        bookmarks: vec![4, 10],
        in_point: Some(2),
        out_point: Some(21),
        notes: vec![TimeNote { color: 1, width: 2.5, points: vec![[1.0, 0.2], [5.0, 0.7], [9.0, 0.4]] }],
    };
    // Le repère 3D : un cube.
    {
        let sommets = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [0.0, 1.0, 1.0]];
        let faces = [[0, 2, 1], [0, 3, 2], [4, 5, 6], [4, 6, 7], [0, 1, 5], [0, 5, 4], [2, 3, 7], [2, 7, 6], [1, 2, 6], [1, 6, 5], [3, 0, 4], [3, 4, 7]];
        let id = doc.add_model("cube.glb".into(), crate::model3d::Mesh3d { positions: sommets.to_vec(), triangles: faces.to_vec() });
        let mut r = Ref3d::new(id, Cam3d { target: [0.5, 0.5, 0.5], azimuth: 35.0, elevation: 20.0, distance: 6.0, focal: 50.0 });
        r.hidden_lines = false;
        r.color = [200, 60, 90];
        r.opacity = 0.6;
        doc.panels[0].ref3d = Some(r);
        // Scène 3D de blocage (`scene3d.rs`, `scenes3d.json`) : toutes les formes, un mannequin à la pose
        // libre, un groupe, un modèle importé, un décor de séquence, la caméra du plateau et son rendu.
        use crate::scene3d::{joint, Light, Mannequin, Pose, RenderArea, RenderSettings, RenderStyle, SceneCam, Shape3d, Target};
        let (plan, seq) = (doc.panels[0].id, doc.sequence_of(0));
        let formes = [Shape3d::Cube, Shape3d::Plane, Shape3d::Cylinder, Shape3d::Sphere, Shape3d::Cone, Shape3d::Stairs { steps: 7 }, Shape3d::Model { id }];
        let mut numeros = Vec::new();
        for (k, forme) in formes.into_iter().enumerate() {
            numeros.extend(doc.scenes3d.add(plan, seq, Target::Plan, forme, [k as f32 * 1.5 - 4.0, 0.0, -1.0 * k as f32]));
        }
        let mut mannequin = Mannequin::new(Pose::Walking, 1.68);
        mannequin.set_joint(joint::ELBOW_R, [35.0, 4.0, 0.0]);
        let mannequin = doc.scenes3d.add(plan, seq, Target::Plan, Shape3d::Mannequin(Box::new(mannequin)), [0.5, 0.0, 1.0]).expect("mannequin");
        if let Some(o) = doc.scenes3d.find_mut(plan, seq, mannequin) {
            o.name = "Léa".into();
            o.color = [210, 120, 90];
            o.xf.rot = [0.0, 25.0, 0.0];
        }
        numeros.push(mannequin);
        doc.scenes3d.group(plan, seq, &numeros[..2]);
        doc.scenes3d.add(plan, seq, Target::Decor, Shape3d::Plane, [0.0; 3]);
        doc.scenes3d.add(plan, seq, Target::Decor, Shape3d::Cylinder, [-3.0, 0.0, -3.0]);
        let scene = doc.scenes3d.plan_mut(plan);
        scene.cam = SceneCam { pos: [0.3, 1.6, 6.5], pan: -8.0, tilt: -3.0, roll: 1.0, focal: 50.0 };
        scene.light = Light { azimuth: 70.0, elevation: 40.0, intensity: 0.9, ambient: 0.3 };
        scene.render = RenderSettings { style: RenderStyle::Outline, lines: true, line_px: 2.5, area: RenderArea::Frame };
        scene.follow = false;
    }

    // ---------------------------------------------------------------- plan 2 : le plan rogné
    {
        let anciens: Vec<CelId> = (0..6).map(|k| dessin_bitmap(&mut doc, 10 + k)).collect();
        let avants: Vec<CelId> = (0..3).map(|k| dessin_bitmap(&mut doc, 20 + k)).collect();
        let mut arriere = Layer::new("Arrière", LayerKind::Bitmap);
        arriere.frames = anciens.iter().flat_map(|c| vec![Some(*c); 6]).collect();
        arriere.roles.insert(0, Role::Key);
        arriere.roles.insert(18, Role::Key);
        arriere.roles.insert(30, Role::Breakdown);
        let mut bruit = Effect::new(EffectKind::Noise);
        bruit.set_key(2, vec![10.0]);
        bruit.set_key(30, vec![60.0]);
        arriere.fx = vec![bruit];
        let mut avant = Layer::new("Avant", LayerKind::Bitmap);
        avant.frames = avants.iter().flat_map(|c| vec![Some(*c); 12]).collect();
        doc.panels[i1].layers = vec![arriere, avant];
        doc.panels[i1].length = 36;
        for (f, c) in [(0usize, cam(240.0, 135.0, 1.0, 0.0)), (5, cam(230.0, 130.0, 1.2, 1.0)), (20, cam(250.0, 140.0, 1.6, -1.0)), (35, cam(240.0, 135.0, 2.0, 0.0))] {
            crate::camera::set_key(&mut doc.panels[i1].camera, f, c);
        }
        doc.toggle_mark(i1, 3);
        doc.toggle_mark(i1, 33);
        doc.panels[i1].cues.bookmarks = vec![1, 34];
        doc.panels[i1].cues.in_point = Some(2);
        let plan = doc.panels[i1].id;
        for (f, texte, fait) in [(2usize, "Trop long au début.", false), (18, "Reprendre l'ombre.", true), (34, "Couper ici ?", false)] {
            let id = doc.add_comment(plan, f, "Anthony", 1_760_000_000 + f as u64);
            doc.comment_mut(id).expect("commentaire").text = texte.into();
            doc.comment_mut(id).expect("commentaire").done = fait;
            doc.comment_mut(id).expect("commentaire").color = [220, 40 + f as u8, 90];
        }
        // Rogné : six images au début, huit à la fin (rien n'est détruit : la réserve).
        assert_eq!(doc.trim_head(i1, 6), 6);
        assert_eq!(doc.trim_tail(i1, -8), -8);
        assert_eq!(doc.hidden(i1), (6, 8));
    }

    // ---------------------------------------------------------------- plan 3 : des formes
    {
        let simple = Element::Stroke {
            points: (0..12).map(|k| VPoint { x: 50.0 + k as f32 * 20.0, y: 100.0 + (k as f32).sin() * 20.0, r: 3.0 + k as f32 * 0.25 }).collect(),
            color: [30, 30, 120],
            opacity: 0.9,
            hardness: 0.6,
            brush: None,
            trace: Trace::default(),
        };
        let cercle = Element::Fill { contours: vec![vec![[200.0, 150.0], [260.0, 150.0], [260.0, 210.0], [200.0, 210.0]], vec![[215.0, 165.0], [245.0, 165.0], [245.0, 195.0], [215.0, 195.0]]], color: [200, 30, 30], opacity: 1.0 };
        let formes = dessin_vectoriel(&mut doc, vec![simple.clone(), cercle.clone()]);
        // Un texte : son image dépend des polices du poste, elle n'est pas comparée.
        let texte = Element::text("Fin de la scène".into(), "Arial".into(), 24.0, [10, 10, 10], crate::vector::Affine::translate(eframe::egui::vec2(40.0, 30.0)));
        let avec_texte = dessin_vectoriel(&mut doc, vec![simple, cercle, texte]);
        sans_pixels.insert(avec_texte);
        let mut l = Layer::new("Formes", LayerKind::Vector);
        l.frames = [vec![Some(formes); 6], vec![Some(avec_texte); 6]].concat();
        doc.panels[i2].layers = vec![l];
        doc.panels[i2].length = 12;
        doc.panels[i2].notes = "Texte en bas à droite.".into();
    }

    // ---------------------------------------------------------------- plan 4 : animation bitmap
    {
        let dessins: Vec<CelId> = (0..4).map(|k| dessin_bitmap(&mut doc, 40 + k)).collect();
        let mut anim = Layer::new("Marche", LayerKind::Bitmap);
        anim.frames = dessins.iter().flat_map(|c| vec![Some(*c); 2]).collect();
        anim.pre = Behaviour::Hold;
        anim.post = Behaviour::Loop;
        anim.blend = BlendMode::Difference;
        anim.collapsed = true;
        let mut cache = Layer::new("Calque caché", LayerKind::Bitmap);
        cache.visible = false;
        cache.locked = true;
        doc.panels[i3].layers = vec![cache, anim];
        doc.panels[i3].length = 8;
        doc.panels[i3].color = Some(5);
    }

    // Le plan 5 reste tel qu'un plan neuf (calque vectoriel vide, deux secondes).
    doc.panels[i4].dialogue = "(silence)".into();

    // ---------------------------------------------------------------- le projet lui-même
    let wav_octets = wav(7_200);
    let pcm = crate::audio::decode(&wav_octets, "wav").expect("le WAV fabriqué se décode");
    let son = doc.add_sound("ambiance.wav".into(), "wav".into(), Arc::new(wav_octets), Arc::new(pcm));
    let clip_a = doc.place_sound(son, 3, None).expect("extrait");
    assert!(clip_a > 0);
    doc.audio_tracks[0].name = "Dialogue".into();
    doc.audio_tracks[0].volume = 0.7;
    let seconde = doc.add_audio_track();
    assert!(seconde > 0);
    doc.audio_tracks[1].muted = true;
    doc.audio_tracks[1].locked = true;
    doc.audio_tracks[1].volume = 1.25;
    doc.add_clip(1, son, 10, 2, 4).expect("extrait de la seconde piste");
    // Effets des sons (`effets_son.json`) : volume, fondus, enchaîné, clés de volume, égaliseur, écho, solo.
    {
        let fx = &mut doc.audio_tracks[0].clips[0].fx;
        fx.gain = 1.25;
        fx.fade_in = 2;
        fx.fade_out = 3;
        fx.xfade = 4;
        fx.keys = vec![crate::audiofx::VolumeKey { t: 1.0, v: 0.5 }, crate::audiofx::VolumeKey { t: 4.0, v: 1.5 }];
        fx.eq = [3.0, -2.0, 6.0];
        fx.echo = crate::audiofx::Echo { mix: 0.35, delay_ms: 180.0, feedback: 0.4 };
    }
    doc.audio_tracks[1].solo = true;
    doc.guides.push(crate::guides::Guide { name: "Fuite à deux points".into(), points: vec![[40.0, 100.0], [440.0, 100.0]] });
    doc.board.strokes.push(crate::board::NoteStroke { points: (0..5).map(|k| VPoint { x: k as f32 * 7.0, y: 3.0 + k as f32, r: 1.5 }).collect(), color: [200, 40, 40], anchor: Some(doc.panels[i1].id) });
    doc.board.texts.push(crate::board::NoteText { pos: pos2(12.0, 30.0), text: "À revoir avec Léa".into(), size: 18.0, color: [20, 80, 160], anchor: None });
    doc.groups[2].name = "Décors lointains".into();
    doc.groups[2].shown = false;
    doc.start_frame = 1001;
    doc.background = crate::document::BackgroundKind::Checker;
    doc.paper = [250, 245, 230];
    doc.paper_b = [190, 200, 210];
    doc.master_volume = 0.8;
    doc.storyboard_locked = true;
    doc.properties = crate::document::Properties {
        title: "Course-poursuite".into(),
        info: "Essai de fiabilité".into(),
        author: "Anthony".into(),
        created: 1_760_000_000,
        modified: 1_760_100_000,
        notes: "Notes — « importantes »".into(),
        copyright: "© 2026".into(),
        fond: Some([250, 250, 210]),
    };
    // Un commentaire sur le plan 1 (visible, dans le plan non rogné) et un sur le dernier plan.
    let id = doc.add_comment(doc.panels[0].id, 9, "Léa", 1_760_000_500);
    doc.comment_mut(id).expect("commentaire").text = "Le flou arrive trop tôt.\nVoir avec le son.".into();
    let id = doc.add_comment(doc.panels[i4].id, 0, "Anthony", 1_760_000_600);
    doc.comment_mut(id).expect("commentaire").color = [10, 200, 90];
    // Transitions du montage (`transitions.json`) : un fondu enchaîné centré, un volet décalé, un fondu
    // depuis le blanc au début du film et un fondu au noir à la fin.
    let enchaine = doc.set_transition(1, crate::transitions::Kind::Dissolve).expect("transition");
    doc.transitions.iter_mut().find(|t| t.id == enchaine).expect("transition").frames = 6;
    let volet = doc.set_transition(2, crate::transitions::Kind::Wipe).expect("volet");
    {
        let t = doc.transitions.iter_mut().find(|t| t.id == volet).expect("volet");
        t.frames = 4;
        t.align = crate::transitions::Align::After;
    }
    doc.set_transition(0, crate::transitions::Kind::White).expect("début du film");
    let n = doc.panels.len();
    doc.set_transition(n, crate::transitions::Kind::Black).expect("fin du film");
    // Sons proposés par la lecture du storyboard (`sons_proposes.json`) : un bruitage à générer, une
    // voix rejetée et modifiée à la main, une ambiance acceptée (son posé), et la confirmation d'envoi
    // en ligne du projet.
    {
        use crate::indications::{Genre, IndicationBrute, Statut};
        let clip = doc.audio_tracks.iter().flat_map(|t| t.clips.iter()).map(|c| c.id).next();
        let (p0, p1, p4) = (doc.panels[0].id, doc.panels[i1].id, doc.panels[i4].id);
        doc.sons_proposes.ajouter(p0, IndicationBrute { start: 3, frames: 36, kind: Some(Genre::Bruitage), description: "Porte qui claque".into(), prompt: "heavy wooden door slamming shut".into(), ..Default::default() });
        let voix = doc.sons_proposes.ajouter(p1, IndicationBrute { start: 0, frames: 30, kind: Some(Genre::Voix), description: "MARIE : « Viens ici. »".into(), text: "Viens ici.".into(), character: "MARIE".into(), voice: "ff_siwis".into(), ..Default::default() });
        let ambiance = doc.sons_proposes.ajouter(p4, IndicationBrute { start: 0, frames: 120, kind: Some(Genre::Ambiance), description: "Pluie".into(), prompt: "steady rain".into(), ..Default::default() });
        for (id, statut) in [(voix, Statut::Rejetee), (ambiance, Statut::Acceptee)] {
            let i = doc.sons_proposes.get_mut(id).expect("proposition");
            i.status = statut;
            i.edited = id == voix;
            i.clip = (id == ambiance).then_some(clip).flatten();
        }
        doc.sons_proposes.envoi_en_ligne_confirme = true;
    }
    // Références visuelles (`references.json` et `references/`) : une image étiquetée, tournée, en gris,
    // liée à deux plans ; une vidéo (trois images PNG) aux réglages de lecture changés, liée au plan 3.
    {
        use crate::references::{Clip, Reference};
        let origine = travail.join("references-origine");
        std::fs::create_dir_all(&origine).expect("dossier des références");
        image::RgbaImage::from_pixel(8, 6, image::Rgba([180, 120, 40, 255])).save(origine.join("rue.png")).expect("image de référence");
        let id = doc.new_reference_id();
        let mut rue = Reference::image(id, "Rue de nuit", "rue.png", [8, 6], pos2(-40.0, 25.0));
        rue.tags = vec!["décor".into(), "lumière".into()];
        rue.rotation = 12.5;
        rue.scale = 20.0;
        rue.flip_h = true;
        rue.grey = true;
        rue.plans = vec![doc.panels[0].id, doc.panels[i2].id];
        rue.home = Some(origine.clone());
        doc.add_reference(rue);
        let dossier_video = origine.join("course");
        std::fs::create_dir_all(&dossier_video).expect("dossier des images de la vidéo de référence");
        for k in 0..3usize {
            image::RgbaImage::from_pixel(8, 6, image::Rgba([(k * 60) as u8, 100, 160, 255])).save(crate::video::frame_file(&dossier_video, k, "png")).expect("image de la vidéo de référence");
        }
        let id = doc.new_reference_id();
        let mut course = Reference::image(id, "Course à pied", "course", [8, 6], pos2(210.0, -60.0));
        course.video = Some(Clip { in_point: 1, out_point: Some(2), looping: true, speed: 1.5, follow: true, offset: -3, ..Clip::new(3, 12.0, "png") });
        course.plans = vec![doc.panels[i3].id];
        course.home = Some(origine);
        doc.add_reference(course);
    }
    // La planche de crayonnés (`crayonnes.json`) : deux pages de la grille 4 × 3 ; une case au crayon (deux
    // traits lisses), une case peinte au pinceau, légendée et déjà servie (cochée), une case avec une
    // légende seule dans la deuxième page.
    {
        let planche = &mut doc.crayonnes;
        planche.ensure(doc.frame);
        planche.add_page();
        let [cw, ch] = planche.size;
        let trait_lisse = |x: f32, y: f32, couleur: [u8; 3]| Element::Stroke {
            points: (0..9).map(|k| VPoint { x: x + k as f32 * 30.0, y: y + (k as f32 * 0.8).sin() * 25.0, r: 2.0 + k as f32 * 0.3 }).collect(),
            color: couleur,
            opacity: 0.9,
            hardness: 0.8,
            brush: None,
            trace: Trace::default(),
        };
        {
            let c = &mut planche.cases[0];
            c.caption = "Léa sort de la cour".into();
            c.ink.vector = Some(VectorDrawing { elements: vec![trait_lisse(60.0, 120.0, [20, 20, 20]), trait_lisse(80.0, 250.0, [90, 90, 90])] });
        }
        {
            let mut peinture = Raster::new(cw, ch);
            for y in 40..140 {
                for x in 200..380 {
                    let i = peinture.tile_index_of(x, y);
                    peinture.tile_mut(i)[(y % TILE) * TILE + x % TILE] = premultiply(30, 30, 30, 255);
                }
            }
            let c = &mut planche.cases[1];
            c.caption = "Gros plan : la clé".into();
            c.used = true;
            c.paint = Cel::bitmap(peinture);
        }
        planche.cases[13].caption = "Plan large, la voiture arrive".into();
    }

    Fixture { doc, sans_pixels }
}

// ------------------------------------------------------------------ comparer deux projets

pub(crate) fn empreinte(octets: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in octets {
        h = (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Tout ce que le projet doit garder, en lignes de texte (une ligne par chose, pour qu'un écart se
/// lise). Ce qui n'est pas dit ici n'est pas persistant par construction : les numéros des calques
/// (neufs à chaque lecture : on les remplace par leur rang), les versions des dessins (elles
/// repartent de zéro), les pixels en cache des traits, et le compteur interne des numéros.
/// `sans_pixels` : dessins dont l'image n'est pas comparée.
pub(crate) fn instantane(doc: &Document, sans_pixels: &BTreeSet<CelId>) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "feuille {}x{} cadre {:?} cadence {} papier {:?} papier_b {:?}", doc.width, doc.height, doc.frame, doc.fps, doc.paper, doc.paper_b);
    let _ = writeln!(s, "fond {:?} image_de_depart {} volume {} verrou {}", doc.background, doc.start_frame, doc.master_volume, doc.storyboard_locked);
    let _ = writeln!(s, "proprietes {:?}", doc.properties);
    let _ = writeln!(s, "sequences {:?}", doc.sequences);
    let _ = writeln!(s, "scenes {:?}", doc.scenes);
    let _ = writeln!(s, "groupes {:?}", doc.groups);
    let _ = writeln!(s, "guides {:?}", doc.guides);
    let _ = writeln!(s, "planche {:?}", doc.board);
    // (Les scènes 3D de blocage : le contenu, sans la version ni le compteur, qui ne sont pas enregistrés.)
    let _ = writeln!(s, "scenes3d plans {:?} decors {:?}", doc.scenes3d.plans, doc.scenes3d.decors);
    // (Le fichier des commentaires les range dans l'ordre du film, `commentaires::to_bytes` : leur
    // ordre dans la liste n'est pas persistant, leurs numéros le sont.)
    let mut commentaires = doc.comments.clone();
    commentaires.sort_by_key(|c| c.id);
    let _ = writeln!(s, "commentaires {:?}", commentaires);
    let _ = writeln!(s, "pistes {:?}", doc.audio_tracks);
    // (Le fichier des transitions les range dans l'ordre du film : leurs numéros, eux, sont gardés.)
    let mut transitions = doc.transitions.clone();
    transitions.sort_by_key(|t| t.id);
    let _ = writeln!(s, "transitions {:?}", transitions);
    // (Le fichier des sons proposés les range dans l'ordre du film : leurs numéros sont gardés.)
    let mut proposes = doc.sons_proposes.clone();
    proposes.indications.sort_by_key(|i| i.id);
    let _ = writeln!(s, "sons_proposes {:?}", proposes);
    // (Le dossier où attendent les fichiers d'une référence n'est pas enregistré : `home` ne compte pas.)
    let mut references = doc.references.clone();
    for r in &mut references {
        r.home = None;
    }
    let _ = writeln!(s, "references {:?}", references);
    // La planche de crayonnés : sa grille, sa taille de case, puis chaque case (légende, coche, traits
    // dans leur forme enregistrée, empreinte de la peinture).
    let p = &doc.crayonnes;
    let _ = writeln!(s, "crayonnes grille {:?} taille {:?} cases {}", p.grid, p.size, p.cases.len());
    for c in &p.cases {
        let traits = c.ink.vector.as_ref().map(|v| serde_json::to_string(&v.elements).expect("écriture des traits"));
        let _ = writeln!(s, "  case {} {:?} utilisee {} traits {traits:?} peinture {:016x}", c.id, c.caption, c.used, empreinte(&c.paint.raster.to_rgba()));
    }
    let mut sons: Vec<_> = doc.sounds.values().collect();
    sons.sort_by_key(|k| k.id);
    for k in sons {
        let _ = writeln!(s, "son {} {:?} {} octets {:016x} pcm {:016x}", k.id, k.name, k.ext, empreinte(&k.bytes), empreinte(&k.pcm.data.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>()));
    }
    let mut modeles: Vec<_> = doc.models.values().collect();
    modeles.sort_by_key(|m| m.id);
    for m in modeles {
        let _ = writeln!(s, "modele {} {:?} {:?}", m.id, m.name, m.mesh);
    }
    for (i, p) in doc.panels.iter().enumerate() {
        let rang: BTreeMap<u64, usize> = p.layers.iter().enumerate().map(|(k, l)| (l.id, k)).collect();
        let _ = writeln!(s, "plan {i} id {} scene {} duree {} couleur {:?}", p.id, p.scene, p.length, p.color);
        let _ = writeln!(s, "  textes {:?} {:?} {:?}", p.action, p.dialogue, p.notes);
        let _ = writeln!(s, "  camera {:?}", p.camera);
        let _ = writeln!(s, "  repere3d {:?}", p.ref3d);
        let _ = writeln!(s, "  marques {:?}", p.marks);
        let _ = writeln!(s, "  reperes {:?}", p.cues);
        for (k, l) in p.layers.iter().enumerate() {
            let mut l = l.clone();
            l.id = k as u64;
            l.parent = l.parent.map(|id| rang.get(&id).copied().unwrap_or(usize::MAX) as u64);
            if let Some(c) = l.ctg.as_mut() {
                for src in &mut c.sources {
                    *src = rang.get(src).copied().unwrap_or(usize::MAX) as u64;
                }
            }
            let _ = writeln!(s, "  calque {k} {l:?}");
        }
        if let Some(r) = doc.reserves.get(&p.id) {
            let mut r = r.clone();
            for piece in [&mut r.head, &mut r.tail] {
                for lp in &mut piece.layers {
                    lp.layer = rang.get(&lp.layer).copied().unwrap_or(usize::MAX) as u64;
                }
            }
            let _ = writeln!(s, "  reserve {r:?}");
        }
    }
    for id in doc.used_cels() {
        let Some(c) = doc.cels.get(&id) else {
            let _ = writeln!(s, "dessin {id} ABSENT");
            continue;
        };
        let pixels = if sans_pixels.contains(&id) { "-".to_string() } else { format!("{:016x}", empreinte(&c.raster.to_rgba())) };
        // (Le vecteur est lu dans sa forme enregistrée : son `Debug` dirait aussi si les pixels de
        // ses traits sont en cache, ce que le fichier ne garde pas.)
        let vecteur = c.vector.as_ref().map(|v| serde_json::to_string(v).expect("écriture du dessin"));
        let _ = writeln!(s, "dessin {id} {}x{} pixels {pixels} vecteur {vecteur:?}", c.raster.width, c.raster.height);
        match &c.fond {
            Some(f) => {
                let _ = writeln!(s, "dessin {id} fond peinture {:016x} coupe {:016x}", empreinte(&f.peinture.raster.to_rgba()), empreinte(&f.coupe.to_rgba()));
            }
            None => {
                let _ = writeln!(s, "dessin {id} sans fond");
            }
        }
    }
    s
}

/// Rien ne diffère ? Sinon, la première ligne qui diffère, et ce qu'elle dit des deux côtés.
pub(crate) fn meme_projet(a: &str, b: &str) -> Result<(), String> {
    if a == b {
        return Ok(());
    }
    for (n, (x, y)) in a.lines().zip(b.lines()).enumerate() {
        if x != y {
            let court = |t: &str| if t.len() > 700 { format!("{}…", t.chars().take(700).collect::<String>()) } else { t.to_string() };
            return Err(format!("ligne {} du détail :\n  avant : {}\n  après : {}", n + 1, court(x), court(y)));
        }
    }
    Err(format!("les détails n'ont pas la même longueur ({} lignes contre {})", a.lines().count(), b.lines().count()))
}

// ------------------------------------------------------------------ outils des essais de fiabilité

/// Un hasard reproductible.
pub(crate) struct Lcg(pub u64);

impl Lcg {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    /// Un nombre de 0 à `n` (exclu).
    pub fn sous(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Un dossier de travail neuf (effacé s'il existe), propre à l'essai et au processus.
pub(crate) fn dossier(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("shot-fiabilite-{}-{nom}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("dossier d'essai");
    d
}

/// Tous les fichiers sous `dir` : chemin relatif → empreinte du contenu.
pub(crate) fn arbre(dir: &Path) -> BTreeMap<String, u64> {
    fn parcourir(base: &Path, dir: &Path, out: &mut BTreeMap<String, u64>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                parcourir(base, &p, out);
            } else {
                let rel = p.strip_prefix(base).expect("sous le dossier").to_string_lossy().replace('\\', "/");
                out.insert(rel, empreinte(&std::fs::read(&p).expect("lecture")));
            }
        }
    }
    let mut out = BTreeMap::new();
    parcourir(dir, dir, &mut out);
    out
}

/// Le projet d'essai enregistré dans un dossier neuf : (projet, manifeste).
pub(crate) fn projet_enregistre(nom: &str) -> (Fixture, PathBuf) {
    let travail = dossier(nom);
    let f = projet_complet(&travail);
    let manifeste = travail.join("Film").join("Film.pelure");
    crate::project::save(&f.doc, &manifeste, &mut Default::default()).expect("enregistrement");
    (f, manifeste)
}

/// Le dossier du projet dont `manifeste` est le manifeste.
pub(crate) fn racine(manifeste: &Path) -> &Path {
    manifeste.parent().expect("dossier du projet")
}

/// Lit le projet sur un fil à part (lecture rapide, celle de l'ouverture) : le résultat de la
/// lecture, ou ce qui l'a empêchée d'en avoir un — un plantage, ou plus de `limite` de calcul (un
/// blocage).
pub(crate) fn charger(manifeste: &Path, limite: std::time::Duration) -> Result<Result<crate::project::Loaded, String>, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let p = manifeste.to_path_buf();
    std::thread::spawn(move || {
        let r = std::panic::catch_unwind(|| crate::project::load_quick(&p)).map_err(|e| {
            e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_else(|| "plantage".into())
        });
        let _ = tx.send(r);
    });
    match rx.recv_timeout(limite) {
        Ok(Ok(r)) => Ok(r),
        Ok(Err(plantage)) => Err(format!("a planté : {plantage}")),
        Err(_) => Err(format!("bloqué plus de {limite:?}")),
    }
}

pub(crate) fn lire_json(chemin: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(chemin).expect("lecture")).expect("JSON")
}

pub(crate) fn ecrire_json(chemin: &Path, v: &serde_json::Value) {
    std::fs::write(chemin, serde_json::to_vec_pretty(v).expect("écriture")).expect("écriture du fichier");
}
