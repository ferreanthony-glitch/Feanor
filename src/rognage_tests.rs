//! Tests du rognage des plans : début et fin rognés sans rien perdre (réserve), ripple et roll,
//! glisser le contenu, aimantation, durée tapée, annulation, ce qui est attaché au plan (couches,
//! caméra, effets, marques, signets, commentaires), verrou du storyboard, fichier.

use eframe::egui::Color32;

use crate::camera::{camera_at, Camera, CameraKey};
use crate::commentaires::Comment;
use crate::document::{CelId, Document, ImageMark, LayerKind, Panel};
use crate::fx::{Effect, EffectKind};
use crate::history::{Edit, History, Nav};
use crate::project;
use crate::rognage::{noms, parse_duration, snap_points, snap_to, Rogner};
use crate::roles::Role;

fn cam(x: f32) -> Camera {
    Camera { x, y: 50.0, zoom: 1.0, angle: 0.0, dolly: 0.0, tilt: 0.0, pan: 0.0 }
}

fn comment(id: u64, panel: u64, frame: usize) -> Comment {
    Comment { id, panel, frame, text: format!("c{id}"), author: "A".into(), color: [1, 2, 3], created: 0, done: false, note: None, annotation: None }
}

/// Un plan de 24 images dont le calque montre le dessin k à l'image k, avec des rôles, un effet
/// à clés, une caméra qui bouge, deux marques, deux signets, une entrée et une sortie, et trois
/// commentaires.
fn rich() -> (Document, Vec<CelId>) {
    let mut doc = Document::new(64, 48, 24);
    let cels: Vec<CelId> = (0..24).map(|_| doc.new_cel(LayerKind::Bitmap)).collect();
    let id = doc.panels[0].id;
    {
        let p = &mut doc.panels[0];
        p.set_length(24);
        p.layers[0].frames = cels.iter().map(|c| Some(*c)).collect();
        p.layers[0].roles.insert(3, Role::Key);
        p.layers[0].roles.insert(15, Role::Breakdown);
        let mut e = Effect::new(EffectKind::GaussianBlur);
        e.set_key(2, vec![0.0]);
        e.set_key(10, vec![10.0]);
        e.set_key(20, vec![4.0]);
        p.layers[0].fx.push(e);
        p.camera = vec![CameraKey::new(0, cam(0.0)), CameraKey::new(12, cam(120.0)), CameraKey::new(23, cam(230.0))];
        p.marks = vec![ImageMark { id: 900, frame: 8, ..Default::default() }, ImageMark { id: 901, frame: 18, ..Default::default() }];
        p.cues.bookmarks = vec![4, 15];
        p.cues.in_point = Some(1);
        p.cues.out_point = Some(20);
    }
    doc.comments = vec![comment(800, id, 2), comment(810, id, 10), comment(820, id, 22)];
    (doc, cels)
}

/// Tout ce qui se rattache aux images d'un plan, en texte (pour comparer deux états).
fn sig(p: &Panel) -> String {
    format!(
        "{:?}",
        (p.length, p.layers.iter().map(|l| (&l.frames, &l.roles, &l.fx)).collect::<Vec<_>>(), &p.camera, &p.marks, &p.cues)
    )
}

fn sorted(doc: &Document) -> Vec<Comment> {
    let mut v = doc.comments.clone();
    v.sort_by_key(|c| c.id);
    v
}

fn nav() -> Nav {
    Nav { panel: 0, layer: 0 }
}

/// Trois plans de 24, 30 et 20 images.
fn film() -> Document {
    let mut doc = Document::new(64, 48, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    for (p, n) in doc.panels.iter_mut().zip([24, 30, 20]) {
        p.set_length(n);
    }
    doc
}

fn total(doc: &Document) -> usize {
    doc.panels.iter().map(Panel::frame_count).sum()
}

#[test]
fn trimming_the_head_hides_frames_and_every_attachment_follows() {
    let (mut doc, cels) = rich();
    let def = doc.default_camera();
    let (before, comments0) = (sig(&doc.panels[0]), sorted(&doc));
    let (cam0, fx0) = (doc.panels[0].camera.clone(), doc.panels[0].layers[0].fx[0].clone());
    assert_eq!(doc.rogner(0, Rogner::Head(6)), Ok(6));
    let p = &doc.panels[0];
    assert_eq!(p.frame_count(), 18);
    assert_eq!(doc.hidden(0), (6, 0));
    // Les couches : la première image est celle qui était la septième.
    assert_eq!(p.layers[0].frames[0], Some(cels[6]));
    assert_eq!(p.layers[0].frames.len(), 18);
    assert_eq!(p.layers[0].roles.keys().copied().collect::<Vec<_>>(), vec![9], "le rôle de l'image 3 est caché, celui de 15 suit");
    // La caméra continue son mouvement : une clé au bord garde la position qu'elle avait.
    assert_eq!(p.camera.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 6, 17]);
    assert!((p.camera[0].cam.x - 60.0).abs() < 1e-3, "x à l'image 6 de l'original : {}", p.camera[0].cam.x);
    for f in 0..18 {
        let (a, b) = (camera_at(&p.camera, f, def), camera_at(&cam0, f + 6, def));
        assert!((a.x - b.x).abs() < 1e-3, "image {f} : {} et {}", a.x, b.x);
    }
    // Les clés d'effet aussi.
    let fx = &p.layers[0].fx[0];
    assert_eq!(fx.keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 4, 14]);
    for f in 0..18 {
        assert!((fx.values_at(f)[0] - fx0.values_at(f + 6)[0]).abs() < 1e-4, "effet à l'image {f}");
    }
    // Marques, signets, entrée et sortie, commentaires.
    assert_eq!(p.marks.iter().map(|m| (m.id, m.frame)).collect::<Vec<_>>(), vec![(900, 2), (901, 12)]);
    assert_eq!(p.cues.bookmarks, vec![9], "le signet de l'image 4 est caché");
    assert_eq!((p.cues.in_point, p.cues.out_point), (None, Some(14)));
    let live: Vec<(u64, usize)> = sorted(&doc).iter().map(|c| (c.id, c.frame)).collect();
    assert_eq!(live, vec![(810, 4), (820, 16)], "le commentaire de l'image 2 est dans la réserve");
    // Tout revient.
    assert_eq!(doc.rogner(0, Rogner::Head(-6)), Ok(-6));
    assert_eq!(sig(&doc.panels[0]), before, "rien n'est perdu ni ajouté");
    assert_eq!(sorted(&doc), comments0);
    assert_eq!(doc.hidden(0), (0, 0));
    assert!(doc.reserves.is_empty());
}

#[test]
fn trimming_the_tail_hides_frames_and_every_attachment_follows() {
    let (mut doc, cels) = rich();
    let def = doc.default_camera();
    let (before, comments0) = (sig(&doc.panels[0]), sorted(&doc));
    let (cam0, fx0) = (doc.panels[0].camera.clone(), doc.panels[0].layers[0].fx[0].clone());
    assert_eq!(doc.rogner(0, Rogner::Tail(-6)), Ok(-6));
    let p = &doc.panels[0];
    assert_eq!((p.frame_count(), doc.hidden(0)), (18, (0, 6)));
    assert_eq!(p.layers[0].frames.len(), 18);
    assert_eq!(p.layers[0].frames[17], Some(cels[17]));
    assert_eq!(p.layers[0].roles.keys().copied().collect::<Vec<_>>(), vec![3, 15]);
    assert_eq!(p.camera.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 12, 17], "une clé tient le mouvement au dernier plan");
    for f in 0..18 {
        let (a, b) = (camera_at(&p.camera, f, def), camera_at(&cam0, f, def));
        assert!((a.x - b.x).abs() < 1e-3, "image {f}");
    }
    let fx = &p.layers[0].fx[0];
    for f in 0..18 {
        assert!((fx.values_at(f)[0] - fx0.values_at(f)[0]).abs() < 1e-4, "effet à l'image {f}");
    }
    assert_eq!(p.marks.iter().map(|m| m.id).collect::<Vec<_>>(), vec![900], "la marque de l'image 18 sort");
    assert_eq!(p.cues.bookmarks, vec![4, 15]);
    assert_eq!((p.cues.in_point, p.cues.out_point), (Some(1), None), "la sortie était dans la partie cachée");
    assert_eq!(sorted(&doc).iter().map(|c| c.id).collect::<Vec<_>>(), vec![800, 810]);
    assert_eq!(doc.rogner(0, Rogner::Tail(6)), Ok(6));
    assert_eq!(sig(&doc.panels[0]), before);
    assert_eq!(sorted(&doc), comments0);
    assert!(doc.reserves.is_empty());
}

#[test]
fn trims_stack_and_come_back_in_steps_without_changing_what_is_shown() {
    let (mut doc, cels) = rich();
    let def = doc.default_camera();
    let before = sig(&doc.panels[0]);
    let cam0 = doc.panels[0].camera.clone();
    doc.rogner(0, Rogner::Head(6)).unwrap();
    doc.rogner(0, Rogner::Head(4)).unwrap();
    doc.rogner(0, Rogner::Tail(-5)).unwrap();
    assert_eq!((doc.panels[0].frame_count(), doc.hidden(0)), (9, (10, 5)));
    assert_eq!(doc.panels[0].layers[0].frames[0], Some(cels[10]));
    // Rendre trois images au début : la fenêtre commence à l'image 7 de l'original.
    assert_eq!(doc.rogner(0, Rogner::Head(-3)), Ok(-3));
    assert_eq!((doc.panels[0].frame_count(), doc.hidden(0)), (12, (7, 5)));
    assert_eq!(doc.panels[0].layers[0].frames[0], Some(cels[7]));
    for f in 0..12 {
        let (a, b) = (camera_at(&doc.panels[0].camera, f, def), camera_at(&cam0, f + 7, def));
        assert!((a.x - b.x).abs() < 1e-3, "caméra à l'image {f}");
    }
    doc.rogner(0, Rogner::Head(-7)).unwrap();
    doc.rogner(0, Rogner::Tail(5)).unwrap();
    assert_eq!(sig(&doc.panels[0]), before);
    assert!(doc.reserves.is_empty());
}

#[test]
fn going_past_the_hidden_content_holds_the_first_or_last_drawing() {
    let (mut doc, cels) = rich();
    // Au début : le premier dessin est tenu, et tout ce qui tient aux images suit le contenu.
    assert_eq!(doc.rogner(0, Rogner::Head(-5)), Ok(-5));
    let p = &doc.panels[0];
    assert_eq!(p.frame_count(), 29);
    assert!(p.layers[0].frames[..5].iter().all(|f| *f == Some(cels[0])));
    assert_eq!(p.layers[0].frames[5], Some(cels[0]));
    assert_eq!(p.layers[0].frames[6], Some(cels[1]));
    assert_eq!(p.camera.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![5, 17, 28]);
    assert_eq!(p.marks[0].frame, 13);
    assert_eq!(p.cues.bookmarks, vec![9, 20]);
    assert_eq!(sorted(&doc).iter().map(|c| c.frame).collect::<Vec<_>>(), vec![7, 15, 27]);
    assert_eq!(p.layers[0].fx[0].keys[0].frame, 7);
    assert_eq!(doc.hidden(0), (0, 0));
    // À la fin : le dernier dessin est tenu (comme l'ancienne poignée de durée).
    let (mut doc, cels) = rich();
    assert_eq!(doc.rogner(0, Rogner::Tail(4)), Ok(4));
    assert_eq!(doc.panels[0].frame_count(), 28);
    assert!(doc.panels[0].layers[0].frames[23..].iter().all(|f| *f == Some(cels[23])));
}

#[test]
fn a_plan_keeps_at_least_one_frame() {
    let (mut doc, _) = rich();
    assert_eq!(doc.rogner(0, Rogner::Head(100)), Ok(23));
    assert_eq!(doc.panels[0].frame_count(), 1);
    assert_eq!(doc.rogner(0, Rogner::Tail(-100)), Ok(0));
    assert_eq!(doc.rogner(0, Rogner::Head(-23)), Ok(-23));
    assert_eq!(doc.panels[0].frame_count(), 24);
    assert_eq!(doc.rogner(0, Rogner::Duration(0)), Ok(-23), "une durée de 0 devient 1");
    assert_eq!(doc.panels[0].frame_count(), 1);
}

#[test]
fn a_trim_ripples_the_following_plans_and_a_roll_keeps_the_total() {
    let mut doc = film();
    let t = total(&doc);
    assert_eq!(doc.panel_start(2), 54);
    // Ripple : le plan 2 perd 5 images au début ; le suivant glisse, le film raccourcit.
    assert_eq!(doc.rogner(1, Rogner::Head(5)), Ok(5));
    assert_eq!((doc.panels[1].frame_count(), doc.panel_start(2), total(&doc)), (25, 49, t - 5));
    // Fin allongée de 3 : tout ce qui suit recule.
    assert_eq!(doc.rogner(1, Rogner::Tail(3)), Ok(3));
    assert_eq!((doc.panels[1].frame_count(), doc.panel_start(2), total(&doc)), (28, 52, t - 2));
    // Roll : la coupe entre les plans 1 et 2 glisse de 4 images vers la droite.
    let t = total(&doc);
    assert_eq!(doc.rogner(0, Rogner::Roll(4)), Ok(4));
    assert_eq!((doc.panels[0].frame_count(), doc.panels[1].frame_count(), total(&doc)), (28, 24, t));
    assert_eq!(doc.panel_start(2), 52, "le plan d'après ne bouge pas");
    // Vers la gauche, au plus jusqu'à une image au premier plan.
    assert_eq!(doc.rogner(0, Rogner::Roll(-100)), Ok(-27));
    assert_eq!((doc.panels[0].frame_count(), total(&doc)), (1, t));
    // Le début du second plan est revenu de la réserve, puis tenu.
    assert_eq!(doc.hidden(1).0, 0);
    // Un roll au bout du film n'a rien à déplacer.
    assert_eq!(doc.rogner(2, Rogner::Roll(3)), Ok(0));
}

#[test]
fn a_roll_uses_the_hidden_frames_of_both_sides() {
    let (mut doc, cels) = rich();
    doc.add_scene_after(0);
    doc.panels[1].set_length(10);
    // Le plan 1 perd sa fin (la réserve la garde) ; le plan 2 est court.
    doc.rogner(0, Rogner::Tail(-6)).unwrap();
    let t = total(&doc);
    assert_eq!(doc.rogner(0, Rogner::Roll(4)), Ok(4));
    assert_eq!(doc.hidden(0), (0, 2), "quatre images revenues de la réserve");
    assert_eq!(doc.panels[0].layers[0].frames[21], Some(cels[21]));
    assert_eq!((doc.panels[1].frame_count(), doc.hidden(1).0, total(&doc)), (6, 4, t));
    assert_eq!(doc.rogner(0, Rogner::Roll(-4)), Ok(-4));
    assert_eq!((doc.hidden(0), doc.hidden(1), total(&doc)), ((0, 6), (0, 0), t));
}

#[test]
fn slip_moves_the_content_under_the_window_without_changing_duration_or_place() {
    let (mut doc, cels) = rich();
    doc.rogner(0, Rogner::Head(4)).unwrap();
    doc.rogner(0, Rogner::Tail(-3)).unwrap();
    assert_eq!((doc.panels[0].frame_count(), doc.hidden(0)), (17, (4, 3)));
    // Tirer le contenu de 3 images vers la droite : on voit des images plus anciennes.
    assert_eq!(doc.rogner(0, Rogner::Slip(3)), Ok(3));
    assert_eq!((doc.panels[0].frame_count(), doc.hidden(0)), (17, (1, 6)));
    assert_eq!(doc.panels[0].layers[0].frames[0], Some(cels[1]));
    assert_eq!(doc.panels[0].layers[0].frames[16], Some(cels[17]));
    // Au bout de la réserve, il s'arrête.
    assert_eq!(doc.rogner(0, Rogner::Slip(10)), Ok(1));
    assert_eq!((doc.panels[0].frame_count(), doc.hidden(0)), (17, (0, 7)));
    assert_eq!(doc.rogner(0, Rogner::Slip(1)), Ok(0));
    // Dans l'autre sens.
    assert_eq!(doc.rogner(0, Rogner::Slip(-20)), Ok(-7));
    assert_eq!((doc.panels[0].frame_count(), doc.hidden(0)), (17, (7, 0)));
    assert_eq!(doc.panels[0].layers[0].frames[0], Some(cels[7]));
    // Sans réserve, il n'y a rien à faire glisser.
    let (mut fresh, _) = rich();
    assert_eq!(fresh.rogner(0, Rogner::Slip(2)), Ok(0));
    // La durée du film n'a pas bougé.
    assert_eq!(doc.total_frames(), 17);
}

#[test]
fn undo_and_redo_restore_the_plan_its_reserve_and_its_comments() {
    let (mut doc, cels) = rich();
    let original = sig(&doc.panels[0]);
    let comments0 = sorted(&doc);
    let mut nav = nav();
    let mut history = History::default();
    let step = |doc: &mut Document, history: &mut History, name: &str, op: Rogner| {
        let snap = Edit::Structure { structure: doc.structure(), nav: self::nav() };
        doc.rogner(0, op).unwrap();
        let before = history.context.replace(name.to_string());
        history.push(snap);
        history.context = before;
    };
    step(&mut doc, &mut history, noms::DEBUT_COURT, Rogner::Head(6));
    step(&mut doc, &mut history, noms::FIN_COURTE, Rogner::Tail(-4));
    step(&mut doc, &mut history, noms::GLISSER, Rogner::Slip(2));
    assert_eq!(history.names().0, vec!["Raccourcir le début", "Raccourcir la fin", "Glisser le contenu"]);
    let done = sig(&doc.panels[0]);
    let done_hidden = doc.hidden(0);
    assert_eq!(done_hidden, (4, 6));
    assert!(history.undo(&mut doc, &mut nav));
    assert_eq!(doc.hidden(0), (6, 4));
    assert!(history.undo(&mut doc, &mut nav));
    assert_eq!(doc.hidden(0), (6, 0));
    assert_eq!(doc.panels[0].frame_count(), 18);
    assert!(history.undo(&mut doc, &mut nav));
    assert_eq!(sig(&doc.panels[0]), original, "annulé trois fois : le plan d'origine");
    assert_eq!(sorted(&doc), comments0, "les commentaires sont revenus");
    assert!(doc.reserves.is_empty());
    for _ in 0..3 {
        assert!(history.redo(&mut doc, &mut nav));
    }
    assert_eq!(sig(&doc.panels[0]), done);
    assert_eq!(doc.hidden(0), done_hidden);
    assert_eq!(doc.panels[0].layers[0].frames[0], Some(cels[4]));
    // Une étape ensuite : l'historique repart.
    assert!(history.undo(&mut doc, &mut nav));
    step(&mut doc, &mut history, noms::DUREE, Rogner::Duration(12));
    assert_eq!(history.names().1.len(), 0);
}

#[test]
fn a_video_layer_keeps_its_sync_when_the_head_is_trimmed() {
    use crate::document::Layer;
    use crate::video::{self, VideoClip};
    let mut doc = Document::new(16, 16, 24);
    doc.panels[0].set_length(20);
    let clip = VideoClip { id: video::new_id(), name: "v.mp4".into(), frames: 20, fps: 24.0, start: 2, ext: "png".into() };
    let mut layer = Layer::new("Vidéo", LayerKind::Bitmap);
    layer.video = Some(clip);
    doc.panels[0].layers.push(layer);
    let shown = |d: &Document, f: usize| d.panels[0].layers.last().unwrap().video.as_ref().unwrap().index_at(f, 24);
    assert_eq!((shown(&doc, 1), shown(&doc, 2), shown(&doc, 7)), (None, Some(0), Some(5)));
    // Le plan commence à l'image 5 de l'ancien : la vidéo, elle, a commencé avant (début négatif).
    doc.rogner(0, Rogner::Head(5)).unwrap();
    assert_eq!(doc.panels[0].layers.last().unwrap().video.as_ref().unwrap().start, -3);
    assert_eq!((shown(&doc, 0), shown(&doc, 2)), (Some(3), Some(5)), "la même image de la vidéo sous la même image du contenu");
    // Plus loin que la réserve : la vidéo suit le contenu décalé.
    doc.rogner(0, Rogner::Head(-8)).unwrap();
    assert_eq!(doc.panels[0].layers.last().unwrap().video.as_ref().unwrap().start, 5);
    assert_eq!((shown(&doc, 4), shown(&doc, 5), shown(&doc, 7)), (None, Some(0), Some(2)));
    // Revenu à zéro rogné : la vidéo est comme au début.
    let mut again = Document::new(16, 16, 24);
    again.panels[0].set_length(20);
    let c = VideoClip { id: video::new_id(), name: "v.mp4".into(), frames: 20, fps: 24.0, start: 2, ext: "png".into() };
    let mut l = Layer::new("Vidéo", LayerKind::Bitmap);
    l.video = Some(c);
    again.panels[0].layers.push(l);
    again.rogner(0, Rogner::Head(5)).unwrap();
    again.rogner(0, Rogner::Head(-5)).unwrap();
    assert_eq!(again.panels[0].layers.last().unwrap().video.as_ref().unwrap().start, 2);
}

#[test]
fn a_drag_replays_from_the_plan_as_it_was_whatever_the_path() {
    use crate::rognageui::PlanBackup;
    let (mut doc, _) = rich();
    let before = sig(&doc.panels[0]);
    let comments0 = sorted(&doc);
    let backup = PlanBackup::capture(&doc, 0);
    // Comme le geste : chaque position repart du plan d'avant, dans tous les sens.
    for delta in [3, 9, 4, -2, -6, 0, 12, 1, -1] {
        backup.restore(&mut doc);
        doc.rogner(0, Rogner::Head(delta)).unwrap();
    }
    backup.restore(&mut doc);
    assert_eq!(sig(&doc.panels[0]), before, "relâcher sans bouger : rien n'a changé");
    assert_eq!(sorted(&doc), comments0);
    assert!(doc.reserves.is_empty());
    for delta in [-4, -9, 3] {
        backup.restore(&mut doc);
        doc.rogner(0, Rogner::Tail(delta)).unwrap();
    }
    backup.restore(&mut doc);
    assert_eq!(sig(&doc.panels[0]), before);
    assert_eq!(sorted(&doc), comments0);
    assert!(doc.reserves.is_empty());
}

#[test]
fn the_storyboard_lock_refuses_every_trim() {
    let (mut doc, _) = rich();
    let before = sig(&doc.panels[0]);
    doc.storyboard_locked = true;
    for op in [Rogner::Head(3), Rogner::Tail(-3), Rogner::Tail(2), Rogner::Roll(1), Rogner::Slip(1), Rogner::Duration(5), Rogner::RestoreAll, Rogner::Forget] {
        assert_eq!(doc.rogner(0, op), Err(crate::projetui::STORYBOARD_LOCKED), "{op:?}");
    }
    assert_eq!(sig(&doc.panels[0]), before);
    assert!(doc.reserves.is_empty());
    doc.storyboard_locked = false;
    assert_eq!(doc.rogner(0, Rogner::Head(3)), Ok(3));
}

#[test]
fn restoring_and_forgetting_the_hidden_content() {
    let (mut doc, _) = rich();
    let before = sig(&doc.panels[0]);
    doc.rogner(0, Rogner::Head(5)).unwrap();
    doc.rogner(0, Rogner::Tail(-3)).unwrap();
    assert_eq!(doc.rogner(0, Rogner::RestoreAll), Ok(8));
    assert_eq!(sig(&doc.panels[0]), before);
    doc.rogner(0, Rogner::Head(5)).unwrap();
    assert_eq!(doc.rogner(0, Rogner::Forget), Ok(5));
    assert_eq!(doc.hidden(0), (0, 0));
    assert_eq!(doc.panels[0].frame_count(), 19, "le plan reste tel qu'il est : le rognage devient définitif");
    assert_eq!(doc.rogner(0, Rogner::Head(-5)), Ok(-5), "il n'y a plus de réserve : le premier dessin est tenu");
}

#[test]
fn the_hidden_content_stays_in_the_project_file() {
    let mut doc = Document::new(16, 16, 24);
    let cels: Vec<CelId> = (0..12)
        .map(|k| {
            let c = doc.new_cel(LayerKind::Bitmap);
            doc.cels.get_mut(&c).unwrap().raster.tile_mut(0)[0] = Color32::from_rgb(10 * k as u8 + 5, 20, 30);
            c
        })
        .collect();
    doc.panels[0].set_length(12);
    doc.panels[0].layers[0].frames = cels.iter().map(|c| Some(*c)).collect();
    doc.panels[0].camera = vec![CameraKey::new(0, cam(0.0)), CameraKey::new(11, cam(110.0))];
    let id = doc.panels[0].id;
    doc.comments.push(comment(700, id, 1));
    let mut fx = Effect::new(EffectKind::GaussianBlur);
    fx.set_key(1, vec![2.0]);
    fx.set_key(9, vec![8.0]);
    doc.panels[0].layers[0].fx.push(fx);
    doc.rogner(0, Rogner::Head(3)).unwrap();
    doc.rogner(0, Rogner::Tail(-2)).unwrap();
    assert_eq!(doc.hidden(0), (3, 2));
    assert!(doc.used_cels().is_superset(&cels.iter().copied().collect()), "les dessins cachés sont gardés");
    let dir = std::env::temp_dir().join(format!("pelure-test-rognage-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("R.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(dir.join(crate::rognage::FILE).exists(), "rognages.json est écrit à côté du manifeste");
    let loaded = project::load(&manifest).unwrap();
    assert!(!loaded.warnings.iter().any(|w| w.contains("rognages")), "{:?}", loaded.warnings);
    let mut back = loaded.doc;
    assert_eq!(back.hidden(0), (3, 2));
    assert_eq!(back.panels[0].frame_count(), 7);
    assert_eq!(back.reserves.values().next().map(|r| r.cels().count()), Some(5), "les dessins cachés ont leur fichier");
    assert_eq!(back.rogner(0, Rogner::RestoreAll), Ok(5));
    assert_eq!(back.panels[0].frame_count(), 12);
    // Les dessins sont revenus dans leur ordre, avec leur contenu (le premier pixel de chacun).
    let want: Vec<Option<CelId>> = cels.iter().map(|c| Some(*c)).collect();
    assert_eq!(back.panels[0].layers[0].frames, want);
    for (k, c) in cels.iter().enumerate() {
        let px = back.cels[c].raster.tiles[0].as_ref().map(|t| t[0]);
        assert_eq!(px, Some(Color32::from_rgb(10 * k as u8 + 5, 20, 30)), "dessin {k}");
    }
    assert_eq!(back.panels[0].camera.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 11]);
    assert_eq!(back.panels[0].layers[0].fx[0].keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![1, 9]);
    assert_eq!(back.comments.len(), 1);
    assert_eq!((back.comments[0].panel, back.comments[0].frame), (back.panels[0].id, 1));
    // Ne rien rogner : pas de fichier.
    let (plain, _) = rich();
    assert!(crate::rognage::to_bytes(&plain).unwrap().is_none());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn snapping_pulls_a_cut_to_the_playhead_the_other_cuts_and_the_keys() {
    let mut doc = film();
    doc.panels[1].camera = vec![CameraKey::new(0, cam(0.0)), CameraKey::new(10, cam(10.0))];
    let mut fx = Effect::new(EffectKind::GaussianBlur);
    fx.set_key(4, vec![3.0]);
    doc.panels[2].layers[0].fx.push(fx);
    let pts = snap_points(&doc, 40);
    // Les bords des plans (0, 24, 54, 74), la tête de lecture, les clés (début du plan 2 à 24).
    for want in [0, 24, 54, 74, 40, 34, 35, 58, 59] {
        assert!(pts.contains(&want), "{want} manque dans {pts:?}");
    }
    assert!(pts.windows(2).all(|w| w[0] < w[1]), "triés, sans doublon");
    // À trois pixels près, avec 8 pixels par image : 0,4 image.
    assert_eq!(snap_to(&pts, 40.3, 6.0 / 8.0), Some(40), "la tête de lecture");
    assert_eq!(snap_to(&pts, 23.6, 0.75), Some(24), "une coupe");
    assert_eq!(snap_to(&pts, 34.4, 0.75), Some(34), "une clé de caméra");
    assert_eq!(snap_to(&pts, 58.7, 0.75), Some(59), "une clé d'effet");
    // Trop loin ou rien à proximité : pas d'aimantation.
    assert_eq!(snap_to(&pts, 29.0, 0.75), None);
    assert_eq!(snap_to(&[], 3.0, 5.0), None);
    // Le plus proche gagne.
    assert_eq!(snap_to(&[10, 11], 10.6, 1.0), Some(11));
}

#[test]
fn the_edge_under_the_mouse_is_the_nearest_cut_and_a_narrow_plan_keeps_its_body() {
    use crate::rognageui::{edge_hit, Side};
    use eframe::egui::{pos2, Rect};
    let b = |x0: f32, x1: f32| Rect::from_min_max(pos2(x0, 0.0), pos2(x1, 40.0));
    let blocks = [b(1.0, 99.0), b(101.0, 199.0), b(201.0, 230.0), b(300.0, 306.0)];
    assert_eq!(edge_hit(&blocks, 3.0), Some((0, Side::Head)));
    assert_eq!(edge_hit(&blocks, 50.0), None, "le milieu d'un plan se prend pour le déplacer");
    // Entre deux plans, la coupe la plus proche gagne ; à égalité, la fin du plan de gauche.
    assert_eq!(edge_hit(&blocks, 99.6), Some((0, Side::Tail)));
    assert_eq!(edge_hit(&blocks, 100.4), Some((1, Side::Head)));
    assert_eq!(edge_hit(&blocks, 100.0), Some((0, Side::Tail)));
    assert_eq!(edge_hit(&blocks, 232.0), Some((2, Side::Tail)));
    // Un plan de six points garde son milieu.
    assert_eq!(edge_hit(&blocks, 303.0), None);
    assert_eq!(edge_hit(&blocks, 300.5), Some((3, Side::Head)));
}

#[test]
fn a_dragged_edge_moves_by_whole_frames_and_sticks_to_the_snap_points() {
    use crate::rognageui::{drag_delta, EdgeKind};
    let pts = [0, 24, 40, 54];
    // Sans point proche : l'image la plus proche de la souris.
    assert_eq!(drag_delta(EdgeKind::Head, 24, 4.4, &pts, 0.75), 4);
    assert_eq!(drag_delta(EdgeKind::Head, 24, 4.6, &pts, 0.75), 5);
    assert_eq!(drag_delta(EdgeKind::Tail, 54, -3.5, &pts, 0.75), -3, "au plus proche, arrondi loin de zéro");
    // La tête de lecture (40) attire le bord à trois quarts d'image près.
    assert_eq!(drag_delta(EdgeKind::Tail, 24, 15.4, &pts, 0.75), 16);
    assert_eq!(drag_delta(EdgeKind::Tail, 24, 14.2, &pts, 0.75), 14);
    // Aimantation éteinte (aucun point) : à l'image près.
    assert_eq!(drag_delta(EdgeKind::Tail, 24, 15.4, &[], 0.75), 15);
    // Revenir sur sa place d'origine : zéro.
    assert_eq!(drag_delta(EdgeKind::Head, 24, 0.3, &pts, 0.75), 0);
    // Le contenu glissé ne s'aimante pas.
    assert_eq!(drag_delta(EdgeKind::Slip, 0, 16.4, &pts, 5.0), 16);
    assert_eq!(drag_delta(EdgeKind::Slip, 0, -2.6, &pts, 5.0), -3);
}

#[test]
fn a_typed_duration_is_frames_or_a_time_code() {
    let fps = 24;
    assert_eq!(parse_duration("36", fps, 10), Some(36));
    assert_eq!(parse_duration(" 36 ", fps, 10), Some(36));
    assert_eq!(parse_duration("1:12", fps, 10), Some(36), "secondes:images");
    assert_eq!(parse_duration("1s12", fps, 10), Some(36));
    assert_eq!(parse_duration("2s", fps, 10), Some(48));
    assert_eq!(parse_duration("0:00:01:12", fps, 10), Some(36));
    assert_eq!(parse_duration("+5", fps, 10), Some(15), "relative");
    assert_eq!(parse_duration("-4", fps, 10), Some(6));
    assert_eq!(parse_duration("-10", fps, 10), None, "pas de plan vide");
    assert_eq!(parse_duration("0", fps, 10), None);
    assert_eq!(parse_duration("", fps, 10), None);
    assert_eq!(parse_duration("abc", fps, 10), None);
    assert_eq!(parse_duration("1:xx", fps, 10), None);
    assert_eq!(parse_duration("1000000", fps, 10), Some(crate::rognage::MAX_PLAN), "bornée");
}

#[test]
fn splitting_and_joining_do_not_leave_a_reserve_that_no_longer_fits() {
    let (mut doc, _) = rich();
    doc.rogner(0, Rogner::Head(4)).unwrap();
    doc.rogner(0, Rogner::Tail(-4)).unwrap();
    assert_eq!(doc.hidden(0), (4, 4));
    // Couper : le début caché reste au premier plan, la fin cachée est oubliée.
    let second = doc.split_plan(0, 8).unwrap();
    assert_eq!(doc.hidden(0), (4, 0));
    assert_eq!(doc.hidden(second), (0, 0));
    // Réunir : seul le début caché du premier reste.
    doc.join_next_plan_without_mark(0).unwrap();
    assert_eq!(doc.hidden(0), (4, 0));
    assert_eq!(doc.rogner(0, Rogner::Head(-4)), Ok(-4));
    assert!(doc.reserves.is_empty());
}
