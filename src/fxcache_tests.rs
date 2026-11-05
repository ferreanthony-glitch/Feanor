//! Cache des dessins dérivés et calcul de fond des effets (`fxcache.rs`, `fx::display`) : la
//! réserve en octets, l'ordre des demandes, le calcul unique, le dernier résultat montré en
//! attendant le nouveau, l'invalidation par version, les vignettes douces, l'aperçu réduit.
//!
//! Le cache et le fil de fond sont communs à tous les tests (ils tournent ensemble) : chaque test
//! prend ses propres numéros de dessin et ses propres valeurs de paramètres.

use super::*;

use crate::document::{Document, LayerKind};
use crate::fx::{self, EffectKind, FxStep, Shown};
use crate::fxexact_tests::{differing, random_raster};
use crate::render::{Plan, PlanItem};
use eframe::egui::Color32;

fn cel_of_tiles(n: usize) -> Arc<Cel> {
    let mut c = Cel::new(64 * 8, 64, LayerKind::Bitmap);
    for i in 0..n {
        c.raster.tile_mut(i)[0] = Color32::BLACK;
    }
    Arc::new(c)
}

const TILE_BYTES: usize = TILE_PIXELS * 4;

fn job(id: CelId, version: u64, prio: u32, epoch: u64) -> Job {
    Job { id, version, src: Arc::new(Raster::new(64, 64)), vector: None, steps: Vec::new(), level: 0, prio, epoch, origin: id, seq: 0, world: 0, base: None, griffon: None }
}

#[test]
fn the_reserve_is_limited_in_bytes_and_lets_the_oldest_go() {
    let mut st = State::new();
    let limit = 3 * TILE_BYTES;
    for id in 1..=3 {
        insert_within(&mut st, id, 0, cel_of_tiles(1), None, limit);
    }
    assert_eq!((st.cache.len(), st.bytes), (3, 3 * TILE_BYTES), "trois dessins d'une tuile tiennent");
    // Un dessin de deux tuiles : le plus ancien (1) part, puis le suivant (2).
    insert_within(&mut st, 4, 0, cel_of_tiles(2), None, limit);
    assert!(!st.cache.contains_key(&1) && !st.cache.contains_key(&2) && st.cache.contains_key(&3) && st.cache.contains_key(&4));
    assert_eq!(st.bytes, 3 * TILE_BYTES, "les octets suivent");
    // Un dessin plus gros que toute la réserve reste quand même (il est affiché).
    let mut big = State::new();
    insert_within(&mut big, 9, 0, cel_of_tiles(5), None, TILE_BYTES);
    assert!(big.cache.contains_key(&9));
}

#[test]
fn a_drawing_used_again_is_kept_over_an_older_one() {
    let mut st = State::new();
    let limit = 2 * TILE_BYTES;
    insert_within(&mut st, 1, 0, cel_of_tiles(1), None, limit);
    insert_within(&mut st, 2, 0, cel_of_tiles(1), None, limit);
    st.cache.get_mut(&1).unwrap().used = tick();
    insert_within(&mut st, 3, 0, cel_of_tiles(1), None, limit);
    assert!(st.cache.contains_key(&1) && !st.cache.contains_key(&2) && st.cache.contains_key(&3));
}

#[test]
fn replacing_a_drawing_keeps_the_byte_count_right() {
    let mut st = State::new();
    insert_within(&mut st, 1, 0, cel_of_tiles(3), None, usize::MAX);
    insert_within(&mut st, 1, 1, cel_of_tiles(1), None, usize::MAX);
    assert_eq!((st.cache.len(), st.bytes), (1, TILE_BYTES));
}

#[test]
fn the_last_result_of_a_drawing_is_kept_for_the_wait() {
    let mut st = State::new();
    insert_within(&mut st, 10, 0, cel_of_tiles(1), Some(77), usize::MAX);
    insert_within(&mut st, 11, 0, cel_of_tiles(1), Some(77), usize::MAX);
    assert_eq!(st.last[&77].id, 11, "le dernier calculé");
    assert_eq!(st.last.len(), 1);
}

#[test]
fn requests_go_by_priority_then_arrival_and_the_aged_ones_are_dropped() {
    let mut st = State::new();
    assert!(enqueue(&mut st, job(1, 0, 5, 10)));
    assert!(enqueue(&mut st, job(2, 0, 1, 10)));
    assert!(enqueue(&mut st, job(3, 0, 1, 10)));
    assert!(enqueue(&mut st, job(4, 0, 0, 2)), "ancienne : l'interface ne l'a pas renouvelée");
    let order: Vec<CelId> = std::iter::from_fn(|| take_next(&mut st, 10)).map(|j| j.id).collect();
    assert_eq!(order, vec![2, 3, 1], "priorité, puis ordre d'arrivée ; la 4 est abandonnée");
    assert!(st.queue.is_empty());
    assert_eq!(st.inflight.len(), 3, "les calculs pris sont notés en cours");
}

/// Une demande abandonnée parce qu'elle a vieilli est signalée (`take_next` réveille alors les vignettes qui
/// l'attendaient, `generation`) ; rien n'est signalé quand rien n'a vieilli.
#[test]
fn dropping_an_aged_request_is_reported() {
    let mut st = State::new();
    assert!(enqueue(&mut st, job(1, 0, 100, 10)));
    assert!(!drop_aged(&mut st, 10 + MAX_AGE), "encore attendue");
    assert_eq!(st.queue.len(), 1);
    assert!(drop_aged(&mut st, 11 + MAX_AGE), "vieillie : abandonnée");
    assert!(st.queue.is_empty());
    assert!(!drop_aged(&mut st, 100), "plus rien à abandonner");
}

#[test]
fn asking_again_raises_the_priority_and_a_new_version_replaces_the_request() {
    let mut st = State::new();
    assert!(enqueue(&mut st, job(1, 0, 9, 10)));
    assert!(!enqueue(&mut st, job(1, 0, 2, 11)), "même demande : pas de doublon");
    assert_eq!((st.queue.len(), st.queue[0].prio, st.queue[0].epoch), (1, 2, 11));
    assert!(!enqueue(&mut st, job(1, 5, 7, 12)), "dessin retouché : la demande est remplacée");
    assert_eq!((st.queue.len(), st.queue[0].version, st.queue[0].prio), (1, 5, 7));
}

#[test]
fn what_is_kept_or_being_computed_is_not_requested_again() {
    let mut st = State::new();
    insert_within(&mut st, 1, 4, cel_of_tiles(1), None, usize::MAX);
    assert!(!enqueue(&mut st, job(1, 4, 0, 0)), "déjà gardé à cette version");
    assert!(enqueue(&mut st, job(1, 5, 0, 0)), "la version a changé");
    st.inflight.insert(2, 0);
    assert!(!enqueue(&mut st, job(2, 0, 0, 0)), "déjà en cours");
    // Gardé entre la demande et le calcul : on le saute.
    st.queue.clear();
    assert!(enqueue(&mut st, job(3, 0, 0, 0)));
    insert_within(&mut st, 3, 0, cel_of_tiles(1), None, usize::MAX);
    assert!(take_next(&mut st, 0).is_none());
}

#[test]
fn several_threads_asking_for_the_same_drawing_compute_it_once() {
    monde::ecrire();
    let id = (1 << 63) | 0x5151_0001;
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let make = || {
        calls.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(40));
        Cel::bitmap(Raster::new(64, 64))
    };
    let got: Vec<Arc<Cel>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..6).map(|_| s.spawn(|| get_or_compute(id, 3, None, make))).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert_eq!(calls.load(Ordering::SeqCst), 1, "calculé une seule fois");
    assert!(got.iter().all(|c| Arc::ptr_eq(c, &got[0])), "le même dessin pour tous");
    // Une autre version est un autre calcul.
    get_or_compute(id, 4, None, make);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    get_or_compute(id, 4, None, make);
    assert_eq!(calls.load(Ordering::SeqCst), 2, "gardé");
}

#[test]
fn a_calculation_that_fails_does_not_block_the_next_one() {
    monde::ecrire();
    let id = (1 << 63) | 0x5151_0002;
    let r = std::panic::catch_unwind(|| get_or_compute(id, 0, None, || panic!("effet qui plante")));
    assert!(r.is_err());
    let again = get_or_compute(id, 0, None, || Cel::bitmap(Raster::new(64, 64)));
    assert_eq!(again.raster.width, 64, "l'attente n'est pas restée accrochée");
}

// ------------------------------------------------------------------ l'affichage

/// Un document avec un dessin bitmap de numéro `id` (des tuiles de pixels au hasard) et un élément
/// du plan qui le passe par `steps`.
fn doc_and_item(id: CelId, steps: Vec<FxStep>) -> (Document, PlanItem) {
    let mut doc = Document::new(320, 256, 24);
    let mut cel = Cel::bitmap(random_raster(320, 256, id as u32));
    cel.version = 1;
    doc.cels.insert(id, cel);
    let item = PlanItem {
        cel: id,
        opacity: 255,
        tint: None,
        transform: None,
        blend: Default::default(),
        fx: steps,
        ctg: None,
        fond: None,
        pre: None,
    };
    (doc, item)
}

fn blur(r: f32) -> FxStep {
    FxStep { kind: EffectKind::GaussianBlur, values: vec![r] }
}

#[test]
fn display_never_waits_it_shows_the_drawing_then_the_computed_effect() {
    monde::ecrire();
    let (doc, item) = doc_and_item(900_000_001, vec![blur(6.17)]);
    let d = fx::display(&doc, &item, 0).unwrap();
    assert!(!d.ready, "calculé en arrière-plan");
    assert!(matches!(d.cel, Shown::Plain(_)) && d.id == item.cel, "en attendant : le dessin sans effet");
    settle();
    let d = fx::display(&doc, &item, 0).unwrap();
    assert!(d.ready && d.id == fx::shown_id(&item));
    let exact = fx::apply(&doc.cels[&item.cel].raster, &item.fx);
    assert_eq!(differing(&d.cel.raster, &exact), 0, "le même dessin que le calcul d'avant");
    // Le calcul bloquant (export) rend le même dessin gardé.
    let Some(Shown::Fx(blocking)) = fx::shown(&doc, &item) else { panic!("dessin dérivé") };
    assert!(matches!(d.cel, Shown::Fx(ref a) if Arc::ptr_eq(a, &blocking)));
}

#[test]
fn while_a_setting_changes_the_previous_result_stays_on_screen() {
    monde::ecrire();
    let (doc, mut item) = doc_and_item(900_000_002, vec![blur(5.31)]);
    fx::display(&doc, &item, 0);
    settle();
    let first = fx::display(&doc, &item, 0).unwrap();
    assert!(first.ready);
    let (first_id, Shown::Fx(first_cel)) = (first.id, first.cel) else { panic!("dessin dérivé") };
    // Le curseur bouge : le nouveau réglage n'est pas prêt, l'écran garde l'ancien résultat.
    item.fx = vec![blur(7.43)];
    let d = fx::display(&doc, &item, 0).unwrap();
    assert!(!d.ready);
    assert_eq!(d.id, first_id, "texture de l'ancien résultat : pas de nouvel envoi à la carte graphique");
    assert!(matches!(d.cel, Shown::Fx(ref a) if Arc::ptr_eq(a, &first_cel)));
    settle();
    let d = fx::display(&doc, &item, 0).unwrap();
    assert!(d.ready && d.id == fx::shown_id(&item) && d.id != first_id);
}

#[test]
fn a_retouched_drawing_is_computed_again_and_the_others_are_not() {
    monde::ecrire();
    let (mut doc, item) = doc_and_item(900_000_003, vec![blur(4.77)]);
    let other = {
        let (d2, i2) = doc_and_item(900_000_004, vec![blur(4.77)]);
        doc.cels.insert(i2.cel, d2.cels[&i2.cel].clone());
        i2
    };
    fx::display(&doc, &item, 0);
    fx::display(&doc, &other, 0);
    settle();
    let a = fx::display(&doc, &item, 0).unwrap();
    let Shown::Fx(a_cel) = a.cel else { panic!() };
    let b = fx::display(&doc, &other, 0).unwrap();
    let Shown::Fx(b_cel) = b.cel else { panic!() };
    // Retouche du premier seulement.
    let c = doc.cels.get_mut(&item.cel).unwrap();
    c.raster.tile_mut(0)[5] = Color32::BLACK;
    c.version += 1;
    let d = fx::display(&doc, &item, 0).unwrap();
    assert!(!d.ready, "version changée : à refaire");
    settle();
    let d = fx::display(&doc, &item, 0).unwrap();
    assert!(d.ready);
    let Shown::Fx(new_cel) = d.cel else { panic!() };
    assert!(!Arc::ptr_eq(&a_cel, &new_cel));
    assert_eq!(differing(&new_cel.raster, &fx::apply(&doc.cels[&item.cel].raster, &item.fx)), 0, "avec la retouche");
    let b2 = fx::display(&doc, &other, 0).unwrap();
    assert!(b2.ready && matches!(b2.cel, Shown::Fx(ref x) if Arc::ptr_eq(x, &b_cel)), "l'autre dessin n'est pas recalculé");
}

#[test]
fn frames_with_equal_settings_share_one_derived_drawing() {
    // Un flou animé qui ne bouge plus après sa dernière clé : un seul calcul pour toutes ces images.
    let mut e = fx::Effect::new(EffectKind::GaussianBlur);
    e.set_key(1, vec![3.19]);
    e.set_key(7, vec![9.83]);
    let ids: Vec<CelId> = [7, 8, 20, 40]
        .iter()
        .map(|&f| fx::fx_id(900_000_005, &fx::steps_at(std::slice::from_ref(&e), f)))
        .collect();
    assert!(ids[0] == ids[1] && ids[1] == ids[2] && ids[2] == ids[3], "après la dernière clé : le même dessin");
    let before = fx::fx_id(900_000_005, &fx::steps_at(std::slice::from_ref(&e), 4));
    assert_ne!(before, ids[0], "entre deux clés : un autre");
}

#[test]
fn a_soft_thumbnail_does_not_wait_and_is_complete_once_the_effects_are_there() {
    monde::ecrire();
    let _one_at_a_time = DRAFT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (doc, item) = doc_and_item(900_000_006, vec![blur(5.93)]);
    let plan = Plan { paper: None, items: vec![item.clone()] };
    // Un point dessiné : une tuile pleine existe sûrement au milieu.
    let (x, y) = (100, 100);
    let (soft, complete) = soft_scope(|| crate::render::composite_pixel(&doc, &plan, x, y));
    assert!(!complete, "l'effet n'est pas encore calculé");
    assert_eq!(soft, doc.cels[&item.cel].raster.get(x, y), "en attendant : le dessin sans effet");
    settle();
    let (soft, complete) = soft_scope(|| crate::render::composite_pixel(&doc, &plan, x, y));
    assert!(complete);
    assert_eq!(soft, crate::render::composite_pixel(&doc, &plan, x, y), "comme le calcul exact");
    assert!(!soft_active(), "le mode doux ne déborde pas du bloc");
}

#[test]
fn prefetch_computes_the_next_frames_before_they_are_shown() {
    monde::ecrire();
    let (doc, item) = doc_and_item(900_000_007, vec![blur(3.61)]);
    assert!(fx::ready(&doc, &item, 0).is_none());
    fx::prefetch(&doc, std::slice::from_ref(&item), 3, 0);
    settle();
    let (id, cel) = fx::ready(&doc, &item, 0).expect("calculé d'avance");
    assert_eq!(id, fx::shown_id(&item));
    assert_eq!(differing(&cel.raster, &fx::apply(&doc.cels[&item.cel].raster, &item.fx)), 0);
    // Des éléments sans effet ne demandent rien.
    let plain = PlanItem { fx: Vec::new(), ..item };
    fx::prefetch(&doc, &[plain.clone()], 1, 0);
    assert!(fx::ready(&doc, &plain, 0).is_none());
}

// ------------------------------------------------------------------ l'aperçu réduit

#[test]
fn the_preview_is_a_smaller_drawing_with_its_own_number_and_full_size_comes_back_after() {
    monde::ecrire();
    let (doc, item) = doc_and_item(900_000_008, vec![blur(40.0), FxStep { kind: EffectKind::HueSaturation, values: vec![20.0, -30.0, 0.0] }]);
    let d = fx::display(&doc, &item, 1).unwrap();
    assert!(!d.ready);
    settle();
    let draft = fx::display(&doc, &item, 1).unwrap();
    assert!(draft.ready && draft.id != fx::shown_id(&item), "un numéro à part");
    assert_eq!((draft.cel.raster.width, draft.cel.raster.height), (160, 128), "réduit de moitié");
    // Au repos : la pleine résolution, demandée puis prête.
    let full = fx::display(&doc, &item, 0).unwrap();
    assert!(!full.ready);
    settle();
    let full = fx::display(&doc, &item, 0).unwrap();
    assert!(full.ready && (full.cel.raster.width, full.cel.raster.height) == (320, 256));
    // Pendant un défilement, l'aperçu gardé est montré tel quel (et, à défaut, la pleine résolution
    // déjà prête).
    let again = fx::display(&doc, &item, 1).unwrap();
    assert!(again.ready && again.id == draft.id);
}

#[test]
fn the_preview_is_not_used_for_layers_that_move_or_for_effects_that_do_not_scale() {
    monde::ecrire();
    let (doc, mut item) = doc_and_item(900_000_009, vec![blur(8.0)]);
    item.transform = Some(crate::document::LayerTransform { x: 5.0, ..Default::default() });
    fx::display(&doc, &item, 1);
    settle();
    let d = fx::display(&doc, &item, 1).unwrap();
    assert_eq!(d.id, fx::shown_id(&item), "calque transformé : pleine résolution");
    let (doc2, mut mosaic) = doc_and_item(900_000_010, vec![FxStep { kind: EffectKind::Mosaic, values: vec![12.0] }]);
    mosaic.transform = None;
    fx::display(&doc2, &mosaic, 2);
    settle();
    let d = fx::display(&doc2, &mosaic, 2).unwrap();
    assert_eq!(d.id, fx::shown_id(&mosaic), "mosaïque : pleine résolution");
}

#[test]
fn the_preview_level_follows_the_zoom() {
    use crate::fxproxy::level_for_zoom;
    assert_eq!([2.0, 1.0, 0.81, 0.8, 0.5, 0.36, 0.35, 0.1].map(level_for_zoom), [0, 0, 0, 1, 1, 1, 2, 2]);
}

/// L'état de l'aperçu (`fx::draft_level`) est commun à tout le programme : les tests qui s'en servent
/// passent l'un après l'autre.
static DRAFT_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn the_preview_lasts_a_quarter_second_after_the_last_movement() {
    monde::ecrire();
    let _one_at_a_time = DRAFT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let ctx = eframe::egui::Context::default();
    // (L'horloge de l'aperçu avance d'elle-même : aucun sommeil, donc aucune dépendance à la machine.)
    let wait = fx::advance_clock;
    // Une première image affichée, et chaque changement d'image : aperçu.
    assert_eq!(fx::draft_level(&ctx, 91, 0, false, 0.5), 1);
    assert_eq!(fx::draft_level(&ctx, 91, 1, false, 0.5), 1);
    assert_eq!(fx::draft_level(&ctx, 91, 1, false, 0.2), 2, "plus petit à l'écran : plus réduit");
    assert_eq!(fx::draft_level(&ctx, 91, 1, false, 1.5), 0, "zoomé : pleine résolution même en mouvement");
    wait(330);
    assert_eq!(fx::draft_level(&ctx, 91, 1, false, 0.5), 0, "au repos : pleine résolution");
    assert_eq!(fx::draft_level(&ctx, 91, 1, true, 0.5), 1, "en lecture : toujours l'aperçu");
    wait(330);
    fx::note_interaction(&ctx);
    assert_eq!(fx::draft_level(&ctx, 91, 1, false, 0.5), 1, "un curseur d'effet bouge");

    // Pendant l'aperçu, une vignette ne demande rien au fil de fond (il sert l'image affichée) ; à
    // la fin elle est prévenue (le compteur des calculs finis avance) et se refait.
    fx::begin_frame(&ctx);
    let (doc, item) = doc_and_item(900_000_011, vec![blur(3.07)]);
    let plan = Plan { paper: None, items: vec![item.clone()] };
    let (_, complete) = soft_scope(|| crate::render::composite_pixel(&doc, &plan, 100, 100));
    assert!(!complete);
    wait(150);
    assert!(fx::ready(&doc, &item, 0).is_none(), "rien n'a été demandé pendant l'aperçu");
    assert_eq!(pending(), 0, "ni mis en file");
    let before = generation();
    wait(330);
    fx::begin_frame(&ctx);
    assert!(generation() > before, "fin de l'aperçu : les vignettes qui attendent sont prévenues");
    let (_, complete) = soft_scope(|| crate::render::composite_pixel(&doc, &plan, 100, 100));
    assert!(!complete, "au repos, la vignette demande son effet");
    settle();
    assert!(fx::ready(&doc, &item, 0).is_some());
}

#[test]
fn downscaling_averages_blocks_and_keeps_empty_tiles_empty() {
    let mut r = Raster::new(130, 70);
    // Un bloc 2×2 : (200, 100, 0, 255) ×2 et transparent ×2 → moitié.
    r.tile_mut(0)[0] = Color32::from_rgba_premultiplied(200, 100, 0, 255);
    r.tile_mut(0)[1] = Color32::from_rgba_premultiplied(200, 100, 0, 255);
    let half = crate::fxproxy::downscale(&r, 1);
    assert_eq!((half.width, half.height), (65, 35));
    assert_eq!(half.get(0, 0), Color32::from_rgba_premultiplied(100, 50, 0, 128), "(2 × 200 + 2) / 4 ; alpha (510 + 2) / 4");
    assert_eq!(half.get(1, 0), Color32::TRANSPARENT);
    assert!(half.tiles.iter().skip(1).all(Option::is_none), "les tuiles vides restent vides");
    let quarter = crate::fxproxy::downscale(&r, 2);
    assert_eq!((quarter.width, quarter.height), (33, 18), "les bords impairs sont arrondis vers le haut");
    assert_eq!(quarter.get(0, 0).a(), 32, "(2 × 255 + 8) / 16");
    assert_eq!(differing(&crate::fxproxy::downscale(&r, 0), &r), 0, "niveau 0 : le dessin même");
}

/// Disque dégradé à bord net sur fond vide.
fn disc(w: usize, h: usize) -> Raster {
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = (x as f32 - w as f32 / 2.0, y as f32 - h as f32 / 2.0);
            if dx * dx + dy * dy < (w as f32 * 0.3).powi(2) {
                let i = (y * w + x) * 4;
                rgba[i..i + 4].copy_from_slice(&[(x * 255 / w) as u8, (y * 255 / h) as u8, 90, 255]);
            }
        }
    }
    Raster::from_rgba(w, h, &rgba)
}

/// Écart moyen et pire écart (sur 255), par composante, entre deux dessins de même taille.
fn gap(a: &Raster, b: &Raster) -> (f32, u8) {
    let (mut sum, mut worst, mut n) = (0u64, 0u8, 0u64);
    for y in 0..a.height {
        for x in 0..a.width {
            let (p, q) = (a.get(x, y), b.get(x, y));
            for (u, v) in p.to_array().into_iter().zip(q.to_array()) {
                let d = u.abs_diff(v);
                sum += d as u64;
                worst = worst.max(d);
                n += 1;
            }
        }
    }
    (sum as f32 / n as f32, worst)
}

#[test]
fn the_preview_looks_like_the_full_size_result_seen_at_half_size() {
    let r = disc(512, 384);
    let steps = vec![
        blur(30.0),
        FxStep { kind: EffectKind::BrightnessContrast, values: vec![30.0, 20.0] },
        FxStep { kind: EffectKind::HueSaturation, values: vec![25.0, -40.0, 0.0] },
        FxStep { kind: EffectKind::Glow, values: vec![16.0, 80.0] },
    ];
    let full = fx::apply(&r, &steps);
    let want = crate::fxproxy::downscale(&full, 1);
    let got = fx::apply_scaled(&crate::fxproxy::downscale(&r, 1), &steps, 1);
    let (mean, worst) = gap(&got, &want);
    println!("aperçu / plein, réduit de moitié : écart moyen {mean:.2}/255, pire {worst}");
    assert!(mean < 2.0, "écart moyen {mean}");
    assert!(worst < 40, "pire écart {worst}");
    // Un quart.
    let want = crate::fxproxy::downscale(&full, 2);
    let got = fx::apply_scaled(&crate::fxproxy::downscale(&r, 2), &steps, 2);
    let (mean, worst) = gap(&got, &want);
    println!("aperçu / plein, réduit au quart : écart moyen {mean:.2}/255, pire {worst}");
    assert!(mean < 3.0, "écart moyen {mean}");
}

#[test]
fn the_preview_noise_has_the_grain_that_the_reduced_full_size_noise_would_show() {
    // Un aplat gris : le bruit se lit directement dans l'écart avec le gris.
    let (w, h) = (256, 256);
    let flat = Raster::from_rgba(w, h, &[128u8, 128, 128, 255].repeat(w * h));
    let spread = |r: &Raster| {
        let v: Vec<f32> = (0..r.height).flat_map(|y| (0..r.width).map(move |x| (x, y))).map(|(x, y)| r.get(x, y).r() as f32 - 128.0).collect();
        let mean = v.iter().sum::<f32>() / v.len() as f32;
        (v.iter().map(|d| (d - mean).powi(2)).sum::<f32>() / v.len() as f32).sqrt()
    };
    let steps = [FxStep { kind: EffectKind::Noise, values: vec![60.0] }];
    let full = spread(&fx::apply(&flat, &steps));
    // Le bruit pleine taille, regardé à moitié de taille (moyenne de 2 × 2) : le grain de l'écran.
    let seen_at_half = spread(&crate::fxproxy::downscale(&fx::apply(&flat, &steps), 1));
    let draft = spread(&fx::apply_scaled(&crate::fxproxy::downscale(&flat, 1), &steps, 1));
    println!("grain : plein {full:.2}, plein vu à moitié {seen_at_half:.2}, aperçu {draft:.2}");
    assert!((draft - seen_at_half).abs() / seen_at_half < 0.08, "l'aperçu a le grain de l'affichage réduit : {draft} / {seen_at_half}");
    assert!(draft < full * 0.65, "et moins que le bruit pleine taille");
}

#[test]
fn level_zero_noise_is_the_old_noise() {
    // La formule d'avant ce chantier, pixel par pixel.
    let r = random_raster(200, 130, 5);
    let old = crate::fxfilters::map_colors(&r, |c, x, y| {
        let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77);
        h ^= h >> 15;
        h = h.wrapping_mul(0x2C1B_3C6D);
        h ^= h >> 12;
        let n = (h & 0xFFFF) as f32 / 32767.5 - 1.0;
        c.map(|v| v + n * (33.0_f32 / 100.0 * 0.5))
    });
    assert_eq!(differing(&crate::fxfilters::noise_scaled(&r, 33.0, 0), &old), 0);
}

#[test]
fn an_idle_derived_drawing_is_ranged_and_reads_back_exact_and_a_busy_one_is_left_alone() {
    monde::ecrire();
    clear();
    let mut c = Cel::new(64 * 8, 64, LayerKind::Bitmap);
    for i in 0..5 {
        let t = c.raster.tile_mut(i);
        for k in 0..TILE_PIXELS {
            t[k] = if k % 7 == 0 { Color32::from_rgb(10 + i as u8, 20, 30) } else { Color32::TRANSPARENT };
        }
    }
    let attendu = c.raster.to_flat();
    // Un dessin dérivé avec son « dernier résultat », un autre que quelqu'un lit encore.
    get_or_compute(9101, 3, Some(9100), move || c);
    let lu = get_or_compute(9102, 1, None, || (*cel_of_tiles(2)).clone());
    // Un premier passage ne fait que noter les derniers usages : rien ne bouge.
    assert_eq!(ranger_les_inactifs(600, 8).0, 0);
    for _ in 0..700 {
        tick();
    }
    // Un dessin touché entre les deux passages n'est pas au repos.
    let touche = get(9102, 1).expect("gardé");
    drop(touche);
    let (traites, tuiles) = ranger_les_inactifs(600, 8);
    assert_eq!(traites, 1, "le dessin lu par ailleurs est laissé tel quel");
    assert_eq!(tuiles, 5);
    let revenu = get(9101, 3).expect("toujours dans le cache");
    assert!(!revenu.a_des_tuiles_ouvertes(), "les pixels sont partis");
    assert!(fallback(9100).is_some(), "son dernier résultat est remis aussi");
    assert_eq!(lock().bytes, 7 * TILE_BYTES, "le compte des octets du cache ne change pas");
    assert_eq!(revenu.raster.to_flat(), attendu, "relu exactement");
    drop(lu);
    drop(revenu);
    assert_eq!(ranger_les_inactifs(0, 8).0, 1, "une fois lâché et laissé tranquille, l'autre se range à son tour");
    // Plus rien à faire : le rangement n'a plus de raison de réveiller l'interface.
    ranger_les_inactifs(0, 8);
    ranger_les_inactifs(0, 8);
    assert!(!a_examiner(), "tout est examiné et calme");
    // Un dessin touché de nouveau est à examiner de nouveau.
    let _ = get(9101, 3);
    assert!(a_examiner());
    clear();
}

/// Un dessin dérivé compressible de quelques tuiles (pixels clairsemés).
fn cel_clairseme(n: usize, v: u8) -> Cel {
    let mut c = Cel::new(64 * 8, 64, LayerKind::Bitmap);
    for i in 0..n {
        let t = c.raster.tile_mut(i);
        for k in 0..TILE_PIXELS {
            t[k] = if k % 11 == 0 { Color32::from_rgb(v, 20, 30) } else { Color32::TRANSPARENT };
        }
    }
    c
}

/// Deux passages de `sortir` : le premier note, le second sort les dessins au repos.
fn sortir_au_repos() -> Sortis {
    assert_eq!(sortir(0, 8).dessins.len(), 0, "le premier passage ne fait que noter");
    sortir(0, 8)
}

#[test]
fn a_ranging_cut_by_a_change_of_project_puts_nothing_back_and_the_byte_count_stays_right() {
    monde::ecrire();
    for ecart in ["mettre de côté", "tout oublier"] {
        clear();
        get_or_compute(9201, 1, Some(9200), || cel_clairseme(4, 10));
        let mut s = sortir_au_repos();
        assert_eq!(s.dessins.len(), 1);
        assert_eq!(lock().bytes, 0, "ses octets ont quitté le compte du cache le temps du rangement");
        // L'utilisateur change d'onglet (ou de projet) pendant le rangement.
        let reserve = if ecart == "mettre de côté" { Some(mettre_de_cote()) } else { None };
        if reserve.is_none() {
            forget_all();
        }
        // (Le nouveau projet reprend les mêmes numéros de dessin : un résultat de l'ancien lui servirait à tort.)
        ranger(&mut s);
        remettre(s);
        let st = lock();
        assert_eq!(st.bytes, 0, "{ecart} : le compte des octets ne bouge pas (il aurait dépassé le contenu du cache)");
        assert!(st.cache.is_empty(), "{ecart} : rien de l'ancien projet n'est remis");
        assert!(st.cache.values().map(|e| e.bytes).sum::<usize>() == st.bytes);
        assert!(st.last.is_empty(), "{ecart} : ni son dernier résultat");
        drop(st);
        if let Some(r) = reserve {
            rendre(r);
            let st = lock();
            assert_eq!(st.cache.values().map(|e| e.bytes).sum::<usize>(), st.bytes, "après le retour de l'onglet");
        }
    }
    clear();
}

#[test]
fn a_drawing_patched_or_computed_during_its_ranging_is_not_put_back_as_it_was() {
    monde::ecrire();
    clear();
    // Retouché (`update`) pendant le rangement : l'ancien n'est pas remis à sa place.
    get_or_compute(9211, 1, None, || cel_clairseme(3, 10));
    let mut s = sortir_au_repos();
    assert_eq!(s.dessins.len(), 1);
    update(9211, |c| c.clone());
    ranger(&mut s);
    remettre(s);
    assert!(get(9211, 1).is_none(), "l'ancien dessin n'est pas remis tel qu'il était : il se recalcule");
    assert_eq!(lock().bytes, 0);
    // Remplacé (`put`) pendant le rangement : c'est le neuf qui reste.
    clear();
    get_or_compute(9212, 1, None, || cel_clairseme(3, 10));
    let mut s = sortir_au_repos();
    let neuf = Arc::new(cel_clairseme(3, 77));
    put(9212, 1, neuf.clone());
    ranger(&mut s);
    remettre(s);
    let la = get(9212, 1).expect("le neuf est là");
    assert!(Arc::ptr_eq(&la, &neuf), "le dessin neuf n'est pas écrasé par l'ancien");
    assert_eq!(lock().bytes, 3 * TILE_BYTES);
    // Sans rien de tel : il est remis, rangé.
    clear();
    get_or_compute(9213, 1, None, || cel_clairseme(3, 10));
    let mut s = sortir_au_repos();
    ranger(&mut s);
    remettre(s);
    assert!(get(9213, 1).is_some_and(|c| !c.a_des_tuiles_ouvertes()));
    assert_eq!(lock().bytes, 3 * TILE_BYTES);
    clear();
}