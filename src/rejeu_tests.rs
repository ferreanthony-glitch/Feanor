//! Défauts du rejeu en arrière-plan trouvés par la contre-vérification de 55b35a3 (N1 à N9) :
//! chacun a ici son essai.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use crate::document::{Cel, Document, LayerKind};
use crate::envelope::Committed;
use crate::project;
use crate::raster::{Raster, TILE};
use crate::rejeu::{self, Rejeu};
use crate::vector::{Element, VectorDrawing};

fn fill() -> Element {
    Element::Fill { contours: vec![vec![[2.0, 2.0], [40.0, 2.0], [40.0, 30.0], [2.0, 30.0]]], color: [10, 20, 30], opacity: 1.0, gradient: None }
}

/// Un dessin de `n` éléments (le nombre d'éléments dit de quel dessin il s'agit, pour les essais).
fn drawing(n: usize) -> VectorDrawing {
    VectorDrawing { elements: (0..n).map(|_| fill()).collect() }
}

// ------------------------------------------------------------------ N1 : Ctrl+Z n'annule pas tout

#[test]
fn history_only_outdates_the_drawings_it_changed() {
    let mut r = Rejeu::new();
    for cel in 1..=3 {
        r.submit_with(cel, &drawing(1), 64, 64, 10, false);
    }
    // L'historique a refait le dessin 2 (sa version a changé) ; le 3 a disparu ; le 1 n'a pas bougé.
    let versions: HashMap<u64, u64> = [(1, 10), (2, 11)].into();
    let mut stale = rejeu::outdated(&r, |c| versions.get(&c).copied());
    stale.sort_unstable();
    assert_eq!(stale, vec![2, 3], "seuls les dessins dépassés");
    for c in stale {
        r.cancel(c);
    }
    assert!(r.is_pending(1), "le rejeu du dessin 1 continue");
    let done = r.wait(Duration::from_secs(20));
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].cel, 1);
}

// ------------------------------------------------------------------ N2 / N3 : adoption

#[test]
fn a_result_is_adopted_only_if_the_drawing_is_the_one_rendered() {
    let mut r = Rejeu::new();
    let mut cel = Cel::new(64, 64, LayerKind::Vector);
    cel.vector = Some(drawing(2));
    cel.version = 5;
    r.submit_with(1, cel.vector.as_ref().unwrap(), 64, 64, cel.version, true);
    let done = r.wait(Duration::from_secs(20)).pop().expect("rendu");
    assert_eq!(done.version, 5);
    assert!(rejeu::adoptable(&cel, &done), "le dessin est celui qui a été rendu");
    // Un autre état du dessin : pixels changés sans que les éléments le soient (un trait partiel,
    // la strate Fond…) — la version a avancé, le rendu ne vaut plus.
    let mut moved = Cel { raster: Raster::new(64, 64), ..cel.clone() };
    moved.version = 6;
    assert!(!rejeu::adoptable(&moved, &done));
    // Ou les éléments ont changé.
    let mut more = cel.clone();
    more.vector.as_mut().unwrap().elements.push(fill());
    assert!(!rejeu::adoptable(&more, &done));
    // Pas un dessin vectoriel : rien à adopter.
    let mut bitmap = cel.clone();
    bitmap.vector = None;
    assert!(!rejeu::adoptable(&bitmap, &done));
}

// ------------------------------------------------------------------ N2 : priorité

static ORDER: Mutex<Vec<usize>> = Mutex::new(Vec::new());

fn slow_render(d: &VectorDrawing, w: usize, h: usize) -> Raster {
    ORDER.lock().unwrap().push(d.elements.len());
    std::thread::sleep(Duration::from_millis(60));
    Raster::new(w, h)
}

#[test]
fn an_interactive_request_goes_before_the_loading_queue() {
    ORDER.lock().unwrap().clear();
    // (Un seul fil pour le chargement : la file attend vraiment.)
    let mut r = Rejeu::with_renderer_and_threads(slow_render, 1);
    // Le chargement d'un projet : cinq dessins en file (le nombre d'éléments tient lieu de nom).
    for n in 1..=5 {
        r.submit_with(n as u64, &drawing(n), 64, 64, 0, false);
    }
    // L'utilisateur lâche un geste : sa demande passe devant ce qui n'a pas commencé.
    r.submit_with(9, &drawing(9), 64, 64, 0, true);
    let mut got = Vec::new();
    while r.has_pending() {
        got.extend(r.wait(Duration::from_secs(20)).into_iter().map(|d| d.cel));
    }
    let order = ORDER.lock().unwrap().clone();
    let pos = |n: usize| order.iter().position(|&x| x == n).unwrap();
    assert!(pos(9) <= 1, "le geste passe devant la file : {order:?}");
    assert!(pos(9) < pos(3) && pos(9) < pos(4) && pos(9) < pos(5), "{order:?}");
    got.sort_unstable();
    assert_eq!(got, vec![1, 2, 3, 4, 5, 9], "tout finit par arriver");
}

// ------------------------------------------------------------------ gros projets : fils du chargement

static EN_MEME_TEMPS: Mutex<(usize, usize)> = Mutex::new((0, 0));

fn counted_render(d: &VectorDrawing, w: usize, h: usize) -> Raster {
    {
        let mut c = EN_MEME_TEMPS.lock().unwrap();
        c.0 += 1;
        c.1 = c.1.max(c.0);
    }
    // (Le dessin de 9 éléments est le geste : il ne dure pas.)
    std::thread::sleep(Duration::from_millis(if d.elements.len() == 9 { 1 } else { 400 }));
    EN_MEME_TEMPS.lock().unwrap().0 -= 1;
    Raster::new(w, h)
}

/// La file du chargement a plusieurs fils ; un geste lâché pendant qu'ils sont tous occupés n'attend pas qu'un se
/// libère (le fil des gestes est à lui), et chaque demande ne se rend qu'une fois.
#[test]
fn the_loading_queue_runs_on_several_threads_and_a_gesture_never_waits_for_it() {
    *EN_MEME_TEMPS.lock().unwrap() = (0, 0);
    let mut r = Rejeu::with_renderer_and_threads(counted_render, 3);
    let demandes: Vec<rejeu::Demande> = (1..=9u64).map(|n| rejeu::Demande::new(n, &drawing(1), 7)).collect();
    r.submit_batch(demandes, 64, 64, false);
    std::thread::sleep(Duration::from_millis(40));
    let t = std::time::Instant::now();
    r.submit_with(100, &drawing(9), 64, 64, 1, true);
    let mut got = Vec::new();
    while !got.contains(&100) {
        got.extend(r.wait(Duration::from_secs(20)).into_iter().map(|d| d.cel));
    }
    // (Derrière la file, il attendrait au moins 350 ms.)
    assert!(t.elapsed() < Duration::from_millis(300), "le geste est rendu tout de suite : {:?}", t.elapsed());
    while r.has_pending() {
        got.extend(r.wait(Duration::from_secs(20)).into_iter().map(|d| d.cel));
    }
    got.sort_unstable();
    assert_eq!(got, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 100], "chaque demande une fois");
    let max = EN_MEME_TEMPS.lock().unwrap().1;
    assert!((3..=4).contains(&max), "trois fils du chargement (plus celui des gestes) : {max}");
}

/// Le nom du fil qui a rendu chaque dessin (par son nombre d'éléments), et combien de temps durent les dessins de plus de 20
/// éléments (les autres : 1 ms).
static FILS_DES_RENDUS: Mutex<Vec<(usize, String)>> = Mutex::new(Vec::new());

fn named_render(d: &VectorDrawing, w: usize, h: usize) -> Raster {
    let n = d.elements.len();
    FILS_DES_RENDUS.lock().unwrap().push((n, std::thread::current().name().unwrap_or("").to_string()));
    std::thread::sleep(Duration::from_millis(if n > 20 { 300 } else { 1 }));
    Raster::new(w, h)
}

/// Relecture indépendante (2026-10-23) : un fil du chargement libre (sous la priorité de l'interface) prenait une demande
/// interactive alors que le fil des gestes était libre — sous charge, le geste tournait à basse priorité. Les fils du
/// chargement ne prennent une demande interactive que si le fil des gestes est occupé ; sinon c'est lui qui la rend.
#[test]
fn an_idle_gesture_thread_renders_the_gestures_and_a_busy_one_hands_the_next_to_the_loading_threads() {
    FILS_DES_RENDUS.lock().unwrap().clear();
    let mut r = Rejeu::with_renderer_and_threads(named_render, 2);
    // Les fils du chargement naissent, rendent la file et attendent, libres.
    r.submit_batch((1..=4u64).map(|n| rejeu::Demande::new(n, &drawing(n as usize), 1)).collect(), 64, 64, false);
    while r.has_pending() {
        r.wait(Duration::from_secs(20));
    }
    // Des gestes, l'un après l'autre : tous rendus par le fil des gestes.
    for k in 0..12u64 {
        r.submit_with(100 + k, &drawing(5 + (k as usize % 3)), 64, 64, 1, true);
        while r.has_pending() {
            r.wait(Duration::from_secs(20));
        }
    }
    let fils = FILS_DES_RENDUS.lock().unwrap().clone();
    let gestes: Vec<&(usize, String)> = fils.iter().filter(|(n, _)| (5..=7).contains(n)).collect();
    assert_eq!(gestes.len(), 12);
    assert!(gestes.iter().all(|(_, f)| f == "rejeu"), "un geste, fil des gestes libre, est rendu par lui : {gestes:?}");
    // Le fil des gestes est pris par un long geste : le suivant va à un fil du chargement libre, sans attendre.
    FILS_DES_RENDUS.lock().unwrap().clear();
    r.submit_with(200, &drawing(30), 64, 64, 1, true);
    std::thread::sleep(Duration::from_millis(40));
    let t = std::time::Instant::now();
    r.submit_with(201, &drawing(8), 64, 64, 1, true);
    let mut got = Vec::new();
    while !got.contains(&201) {
        got.extend(r.wait(Duration::from_secs(20)).into_iter().map(|d| d.cel));
    }
    assert!(t.elapsed() < Duration::from_millis(200), "le second geste n'attend pas le premier : {:?}", t.elapsed());
    while r.has_pending() {
        r.wait(Duration::from_secs(20));
    }
    let fils = FILS_DES_RENDUS.lock().unwrap().clone();
    assert!(fils.iter().any(|(n, f)| *n == 30 && f == "rejeu"), "{fils:?}");
    assert!(fils.iter().any(|(n, f)| *n == 8 && f == "rejeu-chargement"), "{fils:?}");
}

/// Une demande remplacée (même dessin, plus récente) ou retirée ne se rend pas : la file indexée par dessin saute ses
/// anciennes entrées.
static ORDER_2: Mutex<Vec<usize>> = Mutex::new(Vec::new());

fn slow_render_2(d: &VectorDrawing, w: usize, h: usize) -> Raster {
    ORDER_2.lock().unwrap().push(d.elements.len());
    std::thread::sleep(Duration::from_millis(60));
    Raster::new(w, h)
}

#[test]
fn a_replaced_or_cancelled_request_is_skipped() {
    let mut r = Rejeu::with_renderer_and_threads(slow_render_2, 1);
    // Le fil du chargement prend le 1 ; 2 à 5 attendent.
    for n in 1..=5 {
        r.submit_with(n as u64, &drawing(n), 64, 64, 0, false);
    }
    std::thread::sleep(Duration::from_millis(20));
    // Le 3 est redemandé (un dessin de 13 éléments), le 4 retiré.
    r.submit_with(3, &drawing(13), 64, 64, 1, false);
    r.cancel(4);
    let mut got = Vec::new();
    while r.has_pending() {
        got.extend(r.wait(Duration::from_secs(20)).into_iter().map(|d| (d.cel, d.drawing.elements.len())));
    }
    got.sort_unstable();
    assert_eq!(got, vec![(1, 1), (2, 2), (3, 13), (5, 5)]);
    let order = ORDER_2.lock().unwrap().clone();
    assert!(!order.contains(&3) && !order.contains(&4), "ni l'ancien 3 ni le 4 ne sont rendus : {order:?}");
}

// ------------------------------------------------------------------ N9 : panic du rendu

fn render_unless_empty(d: &VectorDrawing, w: usize, h: usize) -> Raster {
    assert!(!d.elements.is_empty(), "rendu qui échoue");
    d.render(w, h)
}

#[test]
fn a_panicking_render_is_reported_and_the_worker_keeps_going() {
    let mut r = Rejeu::with_renderer(render_unless_empty);
    r.submit_with(1, &drawing(0), 64, 64, 0, true);
    assert!(r.wait(Duration::from_secs(20)).is_empty());
    assert!(!r.has_pending(), "plus rien à attendre pour ce dessin");
    assert_eq!(r.take_failed(), vec![1], "l'échec est signalé");
    assert!(r.take_failed().is_empty());
    // Le fil vit toujours : la demande suivante aboutit.
    r.submit_with(2, &drawing(1), 64, 64, 0, true);
    let done = r.wait(Duration::from_secs(20));
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].cel, 2);
}

// ------------------------------------------------------------------ N7 : premier enregistrement

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pelure-test-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn adopting_the_exact_render_does_not_make_the_first_save_rewrite_everything() {
    let dir = temp_dir("adoption");
    let manifest = dir.join("Film").join("Film.pelure");
    std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    let mut doc = Document::new(64, 64, 24);
    let a = doc.new_cel(LayerKind::Vector);
    doc.cels.get_mut(&a).unwrap().vector = Some(drawing(2));
    doc.panels[0].layers[0].frames = vec![Some(a); 48];
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let loaded = project::load(&manifest).unwrap();
    let (mut doc, mut written) = (loaded.doc, loaded.written);
    // Sans le correctif : l'adoption fait monter la version, le dessin paraît modifié.
    let before = doc.cels[&a].version;
    doc.cels.get_mut(&a).unwrap().version += 1;
    assert_eq!(project::save(&doc, &manifest, &mut written.clone()).unwrap().written, 1, "version avancée : réécrit");
    // Avec : le fichier reste à jour.
    rejeu::keep_saved(&mut written, a, before, doc.cels[&a].version);
    assert_eq!(project::save(&doc, &manifest, &mut written).unwrap().written, 0);
    // Un dessin modifié depuis l'enregistrement, lui, reste à réécrire.
    let mut stale: HashMap<u64, u64> = HashMap::new();
    stale.insert(a, before + 7);
    rejeu::keep_saved(&mut stale, a, before, before + 1);
    assert_eq!(stale[&a], before + 7);
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------ N4 : annulation des déformations

fn raster_with(px: &[(usize, usize)]) -> Raster {
    let mut r = Raster::new(2 * TILE, TILE);
    for &(x, y) in px {
        let idx = r.tile_index_of(x, y);
        r.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = eframe::egui::Color32::BLACK;
    }
    r
}

#[test]
fn undoing_the_second_deformation_goes_back_to_the_first_not_to_the_original() {
    use crate::history::{Edit, History, Nav};
    let mut doc = Document::new(2 * TILE, TILE, 24);
    let id = doc.new_cel(LayerKind::Bitmap);
    let original = raster_with(&[(3, 3)]);
    doc.cels.get_mut(&id).unwrap().raster = original.clone();
    let mut history = History::default();
    let mut committed = Committed::default();
    // Premier geste : la tuile 0 devient le résultat A.
    let first = raster_with(&[(5, 5)]);
    let shown = doc.cels[&id].raster.clone();
    let (before, changed) = committed.step(&original, &shown, &first).expect("changé");
    assert!(!changed.is_empty());
    doc.cels.get_mut(&id).unwrap().raster = first.clone();
    history.push(Edit::Tiles { cel: id, tiles: before });
    // Second geste, dans la même séance : l'image affichée est son aperçu, le résultat exact B.
    let preview = raster_with(&[(9, 9)]);
    doc.cels.get_mut(&id).unwrap().raster = preview;
    let second = raster_with(&[(7, 7)]);
    let shown = doc.cels[&id].raster.clone();
    let (before, _) = committed.step(&original, &shown, &second).expect("changé");
    doc.cels.get_mut(&id).unwrap().raster = second;
    history.push(Edit::Tiles { cel: id, tiles: before });
    // Ctrl+Z du second geste : on retrouve le résultat du premier (avant : l'original).
    let mut nav = Nav { panel: 0, layer: 0 };
    assert!(history.undo(&mut doc, &mut nav));
    assert_eq!(doc.cels[&id].raster.to_rgba(), first.to_rgba(), "retour au premier geste");
    // Puis Ctrl+Z du premier : l'original.
    assert!(history.undo(&mut doc, &mut nav));
    assert_eq!(doc.cels[&id].raster.to_rgba(), original.to_rgba());
}
