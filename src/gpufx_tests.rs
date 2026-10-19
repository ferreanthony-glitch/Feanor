//! Les effets de calque sur la carte graphique (`gpufx.rs`) donnent ce que donne le calcul sur
//! processeur (`fx::apply`, la référence de l'export). Chaque effet est dessiné par la carte et
//! comparé pixel pour pixel : écart moyen en niveaux (0 à 255) sur les pixels qui portent quelque chose
//! d'un côté ou de l'autre, écart le plus grand, nombre de pixels à plus de 2 niveaux. Essais sans
//! fenêtre, sur le périphérique par défaut de la machine (passés s'il n'y en a aucun).

use eframe::egui::epaint::Color32;
use eframe::egui_wgpu::RenderState;

use crate::document::{CelId, Document, LayerKind};
use crate::fx::{EffectKind, FxStep};
use crate::gpu::GpuCanvas;
use crate::gpu_tests::{headless, render_with};
use crate::gpufx::{plan_chain, Geo};
use crate::raster::Raster;
use crate::render::{Plan, PlanItem};

const W: usize = 640;
const H: usize = 360;

fn step(kind: EffectKind, values: &[f32]) -> FxStep {
    FxStep { kind, values: values.to_vec() }
}

/// Le tremblement du trait avec ses réglages par défaut et la variante 1 : le seul effet sans passe.
fn boil() -> FxStep {
    let mut values: Vec<f32> = EffectKind::Boil.params().iter().map(|p| p.default).collect();
    values.push(1.0);
    FxStep { kind: EffectKind::Boil, values }
}

/// Un dessin qui montre de tout : un disque en dégradé aux bords doux, des aplats francs, un aplat
/// translucide, un trait fin en diagonale, un peu de couleur collée au bord droit et au bord bas de la
/// feuille, et des zones vides.
fn content(w: usize, h: usize) -> Raster {
    let mut rgba = vec![0u8; w * h * 4];
    let mut put = |x: usize, y: usize, c: [u8; 4]| {
        let i = (y * w + x) * 4;
        rgba[i..i + 4].copy_from_slice(&c);
    };
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32, y as f32);
            // Disque en dégradé, bord doux.
            let d = ((fx - 190.0).powi(2) + (fy - 170.0).powi(2)).sqrt();
            if d < 125.0 {
                let a = ((125.0 - d) / 20.0).clamp(0.0, 1.0);
                put(x, y, [(60.0 + d * 1.4) as u8, (220.0 - d * 1.2) as u8, (90.0 + fx * 0.3) as u8, (a * 255.0) as u8]);
            }
            // Aplat franc rouge.
            if (330..470).contains(&x) && (70..190).contains(&y) {
                put(x, y, [230, 40, 40, 255]);
            }
            // Aplat translucide bleu.
            if (400..600).contains(&x) && (150..300).contains(&y) {
                put(x, y, [40, 90, 220, 128]);
            }
            // Trait fin en diagonale.
            if ((x as i32 - y as i32 * 2 / 3 - 40).abs() < 2) && y > 200 {
                put(x, y, [20, 200, 60, 255]);
            }
            // Collé au bord droit et au bord bas.
            if x >= w - 30 && y >= h - 80 {
                put(x, y, [250, 220, 30, 255]);
            }
            if y >= h - 12 && x < 120 {
                put(x, y, [200, 30, 200, 200]);
            }
        }
    }
    Raster::from_rgba(w, h, &rgba)
}

struct Scene {
    doc: Document,
    id: CelId,
}

/// Version des dessins de ces essais. Le cache des effets du processeur de référence, commun à tous les essais
/// du processus, est indexé par numéro et version de dessin : des dessins d'autres essais, de même numéro et de
/// même version (0 ou 1), mais d'un autre contenu, s'y confondraient. Ceux-ci portent une version à eux.
const OWN_VERSION: u64 = 5_000_000;

fn scene(w: usize, h: usize) -> Scene {
    let mut doc = Document::new(w, h, 24);
    let id = doc.new_cel(LayerKind::Bitmap);
    let cel = doc.cels.get_mut(&id).unwrap();
    cel.raster = content(w, h);
    cel.version = OWN_VERSION;
    Scene { doc, id }
}

fn item(id: CelId, fx: Vec<FxStep>) -> PlanItem {
    PlanItem { cel: id, opacity: 255, tint: None, transform: None, blend: Default::default(), fx, ctg: None, fond: None, pre: None }
}

/// Écart entre deux images : moyenne (niveaux, sur les pixels qui portent quelque chose), maximum, nombre de
/// pixels à plus de 2 niveaux, nombre de pixels comptés.
#[derive(Debug)]
struct Gap {
    mean: f64,
    max: i32,
    over2: usize,
    counted: usize,
}

fn gap(a: &[Color32], b: &[Color32]) -> Gap {
    assert_eq!(a.len(), b.len());
    let (mut sum, mut max, mut over2, mut counted) = (0u64, 0i32, 0usize, 0usize);
    for (p, q) in a.iter().zip(b) {
        if p.a() == 0 && q.a() == 0 {
            continue;
        }
        counted += 1;
        let mut worst = 0;
        for (x, y) in p.to_array().iter().zip(q.to_array()) {
            let d = (*x as i32 - y as i32).abs();
            sum += d as u64;
            worst = worst.max(d);
        }
        max = max.max(worst);
        if worst > 2 {
            over2 += 1;
        }
    }
    Gap { mean: sum as f64 / (counted.max(1) * 4) as f64, max, over2, counted }
}

/// Le dessin passé par `steps` sur la carte graphique (image composée lue sans l'affichage).
fn on_gpu(gpu: &mut GpuCanvas, rs: &RenderState, s: &Scene, steps: &[FxStep]) -> Vec<Color32> {
    let plan = Plan { paper: None, items: vec![item(s.id, steps.to_vec())] };
    let _ = render_with(gpu, rs, &s.doc, &plan, true);
    gpu.read_frame()
}

/// Le même dessin par le calcul sur processeur (la référence de l'export).
fn on_cpu(s: &Scene, steps: &[FxStep]) -> Vec<Color32> {
    crate::fx::apply(&s.doc.cels[&s.id].raster, steps).to_flat()
}

/// Compare, affiche, et exige : écart moyen ≤ `mean` niveaux, écart maximal ≤ `max` niveaux.
fn check(name: &str, gpu: &mut GpuCanvas, rs: &RenderState, s: &Scene, steps: &[FxStep], mean: f64, max: i32) -> Gap {
    let (a, b) = (on_gpu(gpu, rs, s, steps), on_cpu(s, steps));
    let g = gap(&a, &b);
    println!("{name:<44} moyenne {:.3}  max {:>3}  > 2 niveaux : {:>6} sur {:>7} pixels", g.mean, g.max, g.over2, g.counted);
    assert!(g.counted > 500, "{name} : l'image est presque vide ({} pixels)", g.counted);
    assert!(g.mean <= mean, "{name} : écart moyen {:.3} > {mean}", g.mean);
    assert!(g.max <= max, "{name} : écart maximal {} > {max}", g.max);
    g
}

// ---------------------------------------------------------------------- la planification (sans carte)

#[test]
fn the_blur_is_three_boxes_in_rows_then_three_in_columns_like_the_cpu() {
    for radius in [0.5f32, 3.0, 8.0, 40.0, 130.0, 200.0] {
        let boxes = crate::gpufx::box_radii(radius);
        let reach: i32 = boxes.iter().sum();
        // La portée des trois boîtes couvre le rayon demandé, à un ou deux pixels près.
        assert!((reach as f32 - radius).abs() <= 3.0 || reach == 0, "rayon {radius} : portée {reach}");
        let chain = plan_chain(&[step(EffectKind::GaussianBlur, &[radius])], Geo { x: 256, y: 256, w: 128, h: 128 }, (3840, 2160));
        let passes = boxes.iter().filter(|&&r| r > 0).count() * 2;
        match chain {
            Some(c) => assert_eq!(c.passes.len(), passes, "rayon {radius}"),
            None => assert_eq!(passes, 0, "rayon {radius}"),
        }
    }
}

#[test]
fn every_effect_but_the_boil_has_passes() {
    let sheet = (1920u32, 1080u32);
    let input = Geo { x: 100, y: 100, w: 512, h: 384 };
    for kind in EffectKind::ALL {
        let values: Vec<f32> = kind.params().iter().map(|p| p.default).collect();
        let s = FxStep { kind, values };
        let chain = plan_chain(std::slice::from_ref(&s), input, sheet);
        // Les valeurs par défaut de certains effets ne changent rien (rayon, 0 %…) : pas de chaîne alors ;
        // un effet qui change quelque chose doit avoir sa chaîne, sauf le tremblement du trait.
        if crate::gpufx::supported(kind) && kind.changes(&s.values) {
            let chain = chain.unwrap_or_else(|| panic!("{kind:?} sans chaîne"));
            assert!(!chain.passes.is_empty());
            // La dernière passe écrit la texture du résultat.
            assert_eq!(chain.passes.last().unwrap().dst, crate::gpufx::Tex::Temp(chain.out_temp));
            // Le résultat tient dans la feuille.
            assert!(chain.out.x >= 0 && chain.out.y >= 0 && chain.out.right() <= sheet.0 as i32 && chain.out.bottom() <= sheet.1 as i32, "{kind:?} : {:?}", chain.out);
            // Chaque passe est un point d'entrée connu du shader.
            for p in &chain.passes {
                assert!(crate::gpufx::ENTRIES.contains(&p.entry), "{kind:?} : {}", p.entry);
            }
        } else if !crate::gpufx::supported(kind) {
            assert!(chain.is_none(), "{kind:?} n'a pas de passe");
        }
    }
}

#[test]
fn a_stack_with_one_cpu_effect_stays_on_the_cpu() {
    let ok = [step(EffectKind::GaussianBlur, &[10.0]), step(EffectKind::Noise, &[20.0])];
    assert!(crate::gpufx::steps_supported(&ok));
    let mixed = [step(EffectKind::GaussianBlur, &[10.0]), boil(), step(EffectKind::Noise, &[20.0])];
    assert!(!crate::gpufx::steps_supported(&mixed));
    assert!(plan_chain(&mixed, Geo { x: 0, y: 0, w: 64, h: 64 }, (640, 360)).is_none());
    assert!(!crate::gpufx::steps_supported(&[]));
}

#[test]
fn the_blur_enlarges_the_rectangle_by_its_reach_and_color_effects_do_not() {
    let sheet = (1920u32, 1080u32);
    let input = Geo { x: 512, y: 384, w: 256, h: 128 };
    let blur = plan_chain(&[step(EffectKind::GaussianBlur, &[40.0])], input, sheet).unwrap();
    assert_eq!(blur.passes.len(), 6, "trois boîtes en lignes, trois en colonnes");
    assert!(blur.out.x <= input.x - 40 && blur.out.right() >= input.right() + 40 && blur.out.y <= input.y - 40 && blur.out.bottom() >= input.bottom() + 40, "{:?}", blur.out);
    let color = plan_chain(&[step(EffectKind::HueSaturation, &[30.0, 20.0, 0.0])], input, sheet).unwrap();
    assert_eq!(color.out, input);
    // Les effets qui remplissent l'image écrivent la feuille entière.
    let flash = plan_chain(&[step(EffectKind::Flash, &[50.0, 255.0, 255.0, 255.0, 0.0])], input, sheet).unwrap();
    assert_eq!(flash.out, Geo { x: 0, y: 0, w: 1920, h: 1080 });
}

// ---------------------------------------------------------------------- la carte contre le processeur

macro_rules! gpu_or_return {
    () => {
        match headless() {
            Some(rs) => rs,
            None => {
                println!("pas de carte graphique : essai passé");
                return;
            }
        }
    };
}

#[test]
fn every_entry_point_of_the_shader_builds_a_pipeline() {
    let rs = gpu_or_return!();
    let fx = crate::gpufx::GpuFx::try_new(&rs.device, &rs.queue).expect("le shader est accepté par la carte");
    fx.make_all();
    // La liste des points d'entrée est celle du shader.
    let mut declared: Vec<&str> = include_str!("gpufx.wgsl").lines().filter_map(|l| l.strip_prefix("fn fs_")).filter_map(|l| l.split('(').next()).collect();
    declared.sort_unstable();
    let mut listed: Vec<&str> = crate::gpufx::ENTRIES.iter().map(|e| &e[3..]).collect();
    listed.sort_unstable();
    assert_eq!(declared, listed);
}

#[test]
fn color_adjustments_match_the_cpu() {
    let rs = gpu_or_return!();
    let mut gpu = GpuCanvas::new(&rs);
    let s = scene(W, H);
    check("luminosité / contraste (30, 40)", &mut gpu, &rs, &s, &[step(EffectKind::BrightnessContrast, &[30.0, 40.0])], 0.5, 4);
    check("luminosité / contraste (-50, -60)", &mut gpu, &rs, &s, &[step(EffectKind::BrightnessContrast, &[-50.0, -60.0])], 0.5, 4);
    check("niveaux (20, 230, 1,8)", &mut gpu, &rs, &s, &[step(EffectKind::Levels, &[20.0, 230.0, 1.8])], 0.5, 4);
    check("teinte / saturation (40, 30, 10)", &mut gpu, &rs, &s, &[step(EffectKind::HueSaturation, &[40.0, 30.0, 10.0])], 0.5, 4);
    check("teinte / saturation (-120, -80, -20)", &mut gpu, &rs, &s, &[step(EffectKind::HueSaturation, &[-120.0, -80.0, -20.0])], 0.5, 4);
    check("noir et blanc (80 %)", &mut gpu, &rs, &s, &[step(EffectKind::BlackWhite, &[80.0])], 0.5, 4);
    check("négatif (100 %)", &mut gpu, &rs, &s, &[step(EffectKind::Invert, &[100.0])], 0.5, 4);
    check("postérisation (5)", &mut gpu, &rs, &s, &[step(EffectKind::Posterize, &[5.0])], 0.5, 4);
    check("seuil (128)", &mut gpu, &rs, &s, &[step(EffectKind::Threshold, &[128.0])], 0.5, 4);
}

#[test]
fn noise_and_grain_use_the_same_hash_as_the_cpu() {
    let rs = gpu_or_return!();
    let mut gpu = GpuCanvas::new(&rs);
    let s = scene(W, H);
    check("bruit (45 %)", &mut gpu, &rs, &s, &[step(EffectKind::Noise, &[45.0])], 0.5, 4);
    check("bruit (100 %)", &mut gpu, &rs, &s, &[step(EffectKind::Noise, &[100.0])], 0.5, 4);
    check("grain (25 %, 1 px, graine 1, image 0)", &mut gpu, &rs, &s, &[step(EffectKind::Grain, &[25.0, 1.0, 1.0, 0.0])], 0.5, 4);
    check("grain (60 %, 3 px, graine 7, image 33)", &mut gpu, &rs, &s, &[step(EffectKind::Grain, &[60.0, 3.0, 7.0, 33.0])], 0.5, 4);
}

#[test]
fn full_frame_cinema_effects_match_the_cpu() {
    let rs = gpu_or_return!();
    let mut gpu = GpuCanvas::new(&rs);
    let s = scene(W, H);
    check("flash en fondu (60 %, jaune)", &mut gpu, &rs, &s, &[step(EffectKind::Flash, &[60.0, 255.0, 240.0, 120.0, 0.0])], 0.5, 4);
    check("flash en écran (70 %, orange)", &mut gpu, &rs, &s, &[step(EffectKind::Flash, &[70.0, 255.0, 140.0, 20.0, 1.0])], 0.5, 4);
    check("vignettage (70 %, 50, 60)", &mut gpu, &rs, &s, &[step(EffectKind::Vignette, &[70.0, 50.0, 60.0, 45.0, 55.0, 100.0, 100.0])], 0.5, 4);
    check("aberration chromatique (6 px)", &mut gpu, &rs, &s, &[step(EffectKind::Aberration, &[6.0, 50.0, 50.0])], 0.5, 4);
    check("secousse (12, -8, 2°, 105 %)", &mut gpu, &rs, &s, &[step(EffectKind::Shake, &[12.0, -8.0, 2.0, 105.0, 0.0])], 0.5, 4);
    check("secousse avec flou de bougé", &mut gpu, &rs, &s, &[step(EffectKind::Shake, &[40.0, 20.0, -1.0, 100.0, 60.0])], 0.5, 4);
}

#[test]
fn impact_frame_matches_the_cpu() {
    let rs = gpu_or_return!();
    let mut gpu = GpuCanvas::new(&rs);
    let s = scene(W, H);
    // Quantité, mode, seuil, teinte rgb, force, éclat, lignes, centre, graine.
    for (mode, burst, lines, name) in [(0.0, 0.0, 0.0, "négatif"), (1.0, 60.0, 0.0, "noir et blanc avec éclat"), (2.0, 0.0, 80.0, "inversé avec lignes"), (3.0, 100.0, 100.0, "postérisé, éclat et lignes")] {
        let v = [100.0, mode, 128.0, 230.0, 30.0, 30.0, 40.0, burst, lines, 50.0, 50.0, 3.0];
        check(&format!("image d'impact : {name}"), &mut gpu, &rs, &s, &[step(EffectKind::Impact, &v)], 0.5, 4);
    }
}

#[test]
fn gaussian_blur_matches_the_cpu_within_the_kernel_difference() {
    let rs = gpu_or_return!();
    let mut gpu = GpuCanvas::new(&rs);
    let s = scene(W, H);
    // (Un seul noyau au lieu de trois flous en boîte arrondis chacun à 8 bits : quelques niveaux au plus.)
    for r in [4.0f32, 8.0, 25.0, 60.0] {
        check(&format!("flou gaussien ({r} px)"), &mut gpu, &rs, &s, &[step(EffectKind::GaussianBlur, &[r])], 0.5, 4);
    }
}

#[test]
fn glow_sharpen_shadow_and_motion_match_the_cpu() {
    let rs = gpu_or_return!();
    let mut gpu = GpuCanvas::new(&rs);
    let s = scene(W, H);
    check("lueur (12 px, 100 %)", &mut gpu, &rs, &s, &[step(EffectKind::Glow, &[12.0, 100.0])], 0.5, 4);
    check("lueur (40 px, 160 %)", &mut gpu, &rs, &s, &[step(EffectKind::Glow, &[40.0, 160.0])], 0.5, 4);
    check("netteté (120 %, 2 px)", &mut gpu, &rs, &s, &[step(EffectKind::Sharpen, &[120.0, 2.0])], 0.5, 4);
    check("ombre portée (14, 10, 8 px, 70 %)", &mut gpu, &rs, &s, &[step(EffectKind::DropShadow, &[14.0, 10.0, 8.0, 70.0])], 0.5, 4);
    check("ombre portée sans flou", &mut gpu, &rs, &s, &[step(EffectKind::DropShadow, &[-12.0, 6.0, 0.0, 80.0])], 0.5, 4);
    check("flou de mouvement (30 px, 20°)", &mut gpu, &rs, &s, &[step(EffectKind::MotionBlur, &[30.0, 20.0])], 0.5, 4);
    check("flou de mouvement (120 px, -75°)", &mut gpu, &rs, &s, &[step(EffectKind::MotionBlur, &[120.0, -75.0])], 0.5, 4);
}

#[test]
fn bloom_matches_the_cpu() {
    let rs = gpu_or_return!();
    let mut gpu = GpuCanvas::new(&rs);
    let s = scene(W, H);
    for (r, i, t) in [(6.0, 100.0, 120.0), (20.0, 150.0, 140.0), (45.0, 100.0, 100.0), (110.0, 200.0, 140.0)] {
        check(&format!("lueur diffuse ({r} px, {i} %, seuil {t})"), &mut gpu, &rs, &s, &[step(EffectKind::Bloom, &[r, i, t])], 0.5, 4);
    }
}

#[test]
fn warps_render_and_mosaic_match_the_cpu() {
    let rs = gpu_or_return!();
    let mut gpu = GpuCanvas::new(&rs);
    let s = scene(W, H);
    check("onde (12 px, 90 px, 20°)", &mut gpu, &rs, &s, &[step(EffectKind::Wave, &[12.0, 90.0, 20.0])], 0.5, 4);
    check("ondulation (8 px, 40 px)", &mut gpu, &rs, &s, &[step(EffectKind::Ripple, &[8.0, 40.0, 40.0, 60.0])], 0.5, 4);
    check("tourbillon (160°, 60 %)", &mut gpu, &rs, &s, &[step(EffectKind::Twirl, &[160.0, 60.0, 30.0, 50.0])], 0.5, 4);
    check("pincement (60 %, 80 %)", &mut gpu, &rs, &s, &[step(EffectKind::Pinch, &[60.0, 80.0, 30.0, 50.0])], 0.5, 4);
    check("pincement bombé (-70 %)", &mut gpu, &rs, &s, &[step(EffectKind::Pinch, &[-70.0, 80.0, 30.0, 50.0])], 0.5, 4);
    check("nuages (80 px, 70 %, graine 3)", &mut gpu, &rs, &s, &[step(EffectKind::Clouds, &[80.0, 70.0, 3.0])], 0.5, 4);
    check("éclairage (35 %, 35 %, 100 %, 40 %)", &mut gpu, &rs, &s, &[step(EffectKind::Lighting, &[35.0, 35.0, 100.0, 40.0])], 0.5, 4);
    check("dégradé (120°, 60 %)", &mut gpu, &rs, &s, &[step(EffectKind::Gradient, &[120.0, 60.0])], 0.5, 4);
    check("mosaïque (10 px)", &mut gpu, &rs, &s, &[step(EffectKind::Mosaic, &[10.0])], 0.5, 4);
    check("mosaïque (37 px, hors de la grille de la feuille)", &mut gpu, &rs, &s, &[step(EffectKind::Mosaic, &[37.0])], 0.5, 4);
    check("mosaïque (2 px)", &mut gpu, &rs, &s, &[step(EffectKind::Mosaic, &[2.0])], 0.5, 4);
}

#[test]
fn stacks_of_several_effects_match_the_cpu() {
    let rs = gpu_or_return!();
    let mut gpu = GpuCanvas::new(&rs);
    let s = scene(W, H);
    // La scène d'Anthony : flou, bruit, teinte / saturation.
    check(
        "flou + bruit + teinte / saturation",
        &mut gpu,
        &rs,
        &s,
        &[step(EffectKind::GaussianBlur, &[50.0]), step(EffectKind::BrightnessContrast, &[80.0, -45.0]), step(EffectKind::Noise, &[30.0]), step(EffectKind::HueSaturation, &[10.0, -80.0, 0.0])],
        0.5,
        4,
    );
    // Impact, lueur, secousse.
    check(
        "impact + lueur + secousse",
        &mut gpu,
        &rs,
        &s,
        &[
            step(EffectKind::Impact, &[100.0, 0.0, 128.0, 230.0, 30.0, 30.0, 40.0, 60.0, 0.0, 50.0, 50.0, 3.0]),
            step(EffectKind::Glow, &[18.0, 120.0]),
            step(EffectKind::Shake, &[15.0, 6.0, 1.0, 102.0, 40.0]),
        ],
        0.5,
        4,
    );
    check("secousse + grain + vignettage", &mut gpu, &rs, &s, &[step(EffectKind::Shake, &[10.0, -5.0, 0.5, 100.0, 0.0]), step(EffectKind::Grain, &[30.0, 2.0, 5.0, 12.0]), step(EffectKind::Vignette, &[60.0, 55.0, 60.0, 50.0, 50.0, 100.0, 100.0])], 0.5, 4);
}

// ---------------------------------------------------------------------- la composition entière

/// Le plan entier par la carte (image composée) et par le calcul sur processeur (l'export).
fn whole_plan(gpu: &mut GpuCanvas, rs: &RenderState, s: &Scene, plan: &Plan) -> (Vec<Color32>, Vec<Color32>) {
    let a = {
        let _ = render_with(gpu, rs, &s.doc, plan, true);
        gpu.read_frame()
    };
    (a, crate::render::composite_full(&s.doc, plan))
}

/// Les deux scènes d'Anthony, en 3840 × 2160 : la composition entière sur la carte et sur le processeur (l'export).
#[test]
fn the_users_scenes_match_the_cpu_at_full_size() {
    crate::fxcache::monde::lire();
    let rs = gpu_or_return!();
    let mut gpu = GpuCanvas::new(&rs);
    // (Chaque scène a ses versions de dessins à elle : voir `OWN_VERSION`. Les deux ont les mêmes numéros.)
    let (mut scene_1, _) = crate::fxperf_tests::scene();
    let mut scene_2 = crate::effetsperf_tests::scene_with_effects();
    for (k, doc) in [&mut scene_1, &mut scene_2].into_iter().enumerate() {
        for cel in doc.cels.values_mut() {
            cel.version += OWN_VERSION + 1_000_000 * (k as u64 + 1);
        }
    }
    for (name, doc, frames) in [("flou + bruit + fusion", &scene_1, [1usize, 22, 40]), ("impact + lueur + secousse", &scene_2, [12, 25, 38])] {
        for f in frames {
            let plan = crate::render::panel_plan_at(doc, 0, f, doc.render_paper());
            let _ = render_with(&mut gpu, &rs, doc, &plan, true);
            let (a, b) = (gpu.read_frame(), crate::render::composite_full(doc, &plan));
            let g = gap(&a, &b);
            println!("{name}, image {f:>2} : moyenne {:.4}  max {:>3}  > 2 niveaux : {} sur {}", g.mean, g.max, g.over2, g.counted);
            assert!(g.mean <= 0.5 && (g.over2 as f64) < g.counted as f64 * 0.005, "{name}, image {f} : {g:?}");
        }
    }
}

#[test]
fn opacity_blend_mode_and_transform_apply_after_the_effects() {
    crate::fxcache::monde::lire();
    let rs = gpu_or_return!();
    let mut gpu = GpuCanvas::new(&rs);
    let s = scene(W, H);
    let steps = vec![step(EffectKind::GaussianBlur, &[14.0]), step(EffectKind::HueSaturation, &[30.0, 20.0, 0.0])];
    let mut it = item(s.id, steps);
    it.opacity = 170;
    it.blend = crate::blend::BlendMode::Multiply;
    it.transform = Some(crate::document::LayerTransform { x: 30.0, y: -20.0, scale: 0.9, angle: 6.0, ..Default::default() });
    let plan = Plan { paper: Some([245, 240, 230]), items: vec![item(s.id, vec![step(EffectKind::Invert, &[100.0])]), it] };
    let (a, b) = whole_plan(&mut gpu, &rs, &s, &plan);
    let g = gap(&a, &b);
    println!("opacité, fusion, transformation : moyenne {:.3}  max {}  > 2 niveaux : {} sur {}", g.mean, g.max, g.over2, g.counted);
    assert!(g.mean <= 1.0 && (g.over2 as f64) < g.counted as f64 * 0.01, "{g:?}");
}

/// Un repère de calque d'ajustement hors dossier.
fn adjustment(steps: Vec<FxStep>, opacity: u8, blend: crate::blend::BlendMode) -> PlanItem {
    marker(steps, opacity, blend, 0, true)
}

/// Un repère de calque d'ajustement : `from` est le rang, dans les éléments du plan, du premier élément qu'il
/// couvre (dans un dossier : `root` faux).
fn marker(steps: Vec<FxStep>, opacity: u8, blend: crate::blend::BlendMode, from: usize, root: bool) -> PlanItem {
    use crate::calqueffet::Reglage;
    use crate::render::FondPart;
    let r = Reglage { layer: 7, from, root, steps, opacity, blend };
    PlanItem { cel: (1 << 62) | 7, opacity: 255, tint: None, transform: None, blend: Default::default(), fx: Vec::new(), ctg: None, fond: Some(FondPart::Reglage(std::sync::Arc::new(r))), pre: None }
}

#[test]
fn an_adjustment_layer_processes_everything_below_it() {
    crate::fxcache::monde::lire();
    let rs = gpu_or_return!();
    let mut gpu = GpuCanvas::new(&rs);
    let s = scene(W, H);
    // Deux dessins dessous (un avec ses effets), le papier, puis le calque d'ajustement.
    let under = vec![item(s.id, vec![step(EffectKind::Noise, &[20.0])]), {
        let mut i = item(s.id, Vec::new());
        i.transform = Some(crate::document::LayerTransform { x: 90.0, y: 40.0, scale: 0.6, ..Default::default() });
        i.opacity = 200;
        i
    }];
    for (name, steps, opacity, blend) in [
        ("lueur + vignettage, normal à 100 %", vec![step(EffectKind::Glow, &[16.0, 100.0]), step(EffectKind::Vignette, &[60.0, 50.0, 60.0, 50.0, 50.0, 100.0, 100.0])], 255u8, crate::blend::BlendMode::Normal),
        ("flou + grain, normal à 60 %", vec![step(EffectKind::GaussianBlur, &[20.0]), step(EffectKind::Grain, &[30.0, 2.0, 4.0, 9.0])], 153, crate::blend::BlendMode::Normal),
        ("flash en écran, fusion Produit", vec![step(EffectKind::Flash, &[50.0, 255.0, 200.0, 90.0, 0.0])], 255, crate::blend::BlendMode::Multiply),
        ("secousse + aberration, normal", vec![step(EffectKind::Shake, &[10.0, 6.0, 1.0, 101.0, 30.0]), step(EffectKind::Aberration, &[5.0, 50.0, 50.0])], 255, crate::blend::BlendMode::Normal),
    ] {
        let mut items = under.clone();
        items.push(adjustment(steps, opacity, blend));
        let plan = Plan { paper: Some([255, 255, 255]), items };
        let (a, b) = whole_plan(&mut gpu, &rs, &s, &plan);
        let g = gap(&a, &b);
        println!("calque d'ajustement : {name:<40} moyenne {:.3}  max {:>3}  > 2 niveaux : {} sur {}", g.mean, g.max, g.over2, g.counted);
        assert!(g.mean <= 1.0 && (g.over2 as f64) < g.counted as f64 * 0.01, "{name} : {g:?}");
    }
}

#[test]
fn only_changed_effects_are_redrawn() {
    let rs = gpu_or_return!();
    let mut gpu = GpuCanvas::new(&rs);
    let s = scene(W, H);
    let plan = |r: f32| Plan { paper: None, items: vec![item(s.id, vec![step(EffectKind::GaussianBlur, &[r]), step(EffectKind::Noise, &[30.0])])] };
    let _ = render_with(&mut gpu, &rs, &s.doc, &plan(20.0), true);
    let first = gpu.effects_passes();
    assert!(first >= 7, "7 passes au moins pour un flou et un bruit : {first}");
    // La même image : rien n'est redessiné.
    let _ = render_with(&mut gpu, &rs, &s.doc, &plan(20.0), false);
    assert_eq!(gpu.effects_passes(), first, "la même image ne redessine rien");
    // Un réglage qui change : les passes repartent.
    let _ = render_with(&mut gpu, &rs, &s.doc, &plan(21.0), false);
    assert_eq!(gpu.effects_passes(), 2 * first, "un curseur redessine la pile entière, une fois");
    // Le dessin d'origine change (version et pixels) : la texture est refaite.
    let mut s2 = scene(W, H);
    s2.doc.cels.get_mut(&s2.id).unwrap().version += 1;
    let mut r = Raster::new(W, H);
    r.tile_mut(0)[0] = Color32::from_rgb(200, 10, 10);
    s2.doc.cels.get_mut(&s2.id).unwrap().raster = r;
    let before = gpu.effects_passes();
    let _ = render_with(&mut gpu, &rs, &s2.doc, &plan(21.0), true);
    assert!(gpu.effects_passes() > before);
    // La texture du résultat est recyclée d'une image à l'autre : pas de texture de plus à chaque réglage.
    let textures = gpu.stats().textures;
    for r in 22..30 {
        let _ = render_with(&mut gpu, &rs, &s.doc, &plan(r as f32), true);
    }
    assert!(gpu.stats().textures <= textures + 1, "{} textures, il y en avait {textures}", gpu.stats().textures);
}

/// L'image que montre la carte quand le calcul de fond sur processeur a répondu : on redemande tant que le
/// résultat n'est pas arrivé (une demande peut être abandonnée par l'écran d'un autre essai : l'état des effets en
/// attente est commun à tous les essais du processus, `fxcache::monde`).
fn background_frame(gpu: &mut GpuCanvas, rs: &RenderState, s: &Scene, plan: &Plan, steps: &[FxStep]) -> Vec<Color32> {
    let want = on_cpu(s, steps);
    let mut frame = Vec::new();
    for _ in 0..300 {
        let _ = render_with(gpu, rs, &s.doc, plan, true);
        crate::fxcache::settle();
        let _ = render_with(gpu, rs, &s.doc, plan, true);
        frame = gpu.read_frame();
        if gap(&frame, &want).max == 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    frame
}

#[test]
fn a_stack_with_a_warp_and_the_switch_off_use_the_cpu_path() {
    crate::fxcache::monde::lire();
    let rs = gpu_or_return!();
    let mut gpu = GpuCanvas::new(&rs);
    let s = scene(W, H);
    // Un effet sans passe au milieu : toute la pile reste sur le processeur.
    let mixed = vec![step(EffectKind::GaussianBlur, &[6.0]), boil(), step(EffectKind::Noise, &[20.0])];
    let plan = Plan { paper: None, items: vec![item(s.id, mixed.clone())] };
    let _ = render_with(&mut gpu, &rs, &s.doc, &plan, true);
    assert_eq!(gpu.effects_passes(), 0, "aucune passe : la pile n'est pas entièrement sur la carte");
    let g = gap(&background_frame(&mut gpu, &rs, &s, &plan, &mixed), &on_cpu(&s, &mixed));
    assert!(g.max == 0, "le calcul de fond donne exactement la référence : {g:?}");
    assert_eq!(gpu.effects_passes(), 0);
    // Le réglage éteint : même une pile entièrement sur la carte passe par le calcul de fond.
    let simple = vec![step(EffectKind::GaussianBlur, &[9.0])];
    let plan = Plan { paper: None, items: vec![item(s.id, simple.clone())] };
    gpu.set_effects_gpu(false);
    assert!(!gpu.effects_on());
    let g = gap(&background_frame(&mut gpu, &rs, &s, &plan, &simple), &on_cpu(&s, &simple));
    assert_eq!(g.max, 0, "le calcul de fond donne exactement la référence : {g:?}");
    assert_eq!(gpu.effects_passes(), 0);
    // Rallumé : la carte reprend.
    gpu.set_effects_gpu(true);
    assert!(gpu.effects_on());
    let _ = render_with(&mut gpu, &rs, &s.doc, &plan, true);
    assert!(gpu.effects_passes() >= 6);
    assert!(gap(&gpu.read_frame(), &on_cpu(&s, &simple)).max <= 1);
}

#[test]
fn an_empty_drawing_and_a_drawing_touching_the_sheet_edge_are_drawn_like_the_cpu() {
    let rs = gpu_or_return!();
    let mut gpu = GpuCanvas::new(&rs);
    // Un dessin vide : un flash le remplit, un flou ne montre rien.
    let mut s = scene(W, H);
    s.doc.cels.get_mut(&s.id).unwrap().raster = Raster::new(W, H);
    let flash = [step(EffectKind::Flash, &[40.0, 255.0, 255.0, 255.0, 0.0])];
    let (a, b) = (on_gpu(&mut gpu, &rs, &s, &flash), on_cpu(&s, &flash));
    assert_eq!(gap(&a, &b).max, 0);
    assert!(a.iter().all(|p| p.a() == 102), "le flash couvre toute l'image");
    let blur = [step(EffectKind::GaussianBlur, &[30.0])];
    assert!(on_gpu(&mut gpu, &rs, &s, &blur).iter().all(|p| p.a() == 0));
    // Un dessin collé aux quatre bords : la perte au bord de la feuille est celle du processeur.
    let mut rgba = vec![0u8; W * H * 4];
    for y in 0..H {
        for x in 0..W {
            if x < 20 || y < 20 || x >= W - 20 || y >= H - 20 {
                rgba[(y * W + x) * 4..(y * W + x) * 4 + 4].copy_from_slice(&[240, 120, 20, 255]);
            }
        }
    }
    s.doc.cels.get_mut(&s.id).unwrap().raster = Raster::from_rgba(W, H, &rgba);
    for (name, steps) in [("flou 25 px", vec![step(EffectKind::GaussianBlur, &[25.0])]), ("lueur 30 px", vec![step(EffectKind::Glow, &[30.0, 100.0])]), ("ombre portée", vec![step(EffectKind::DropShadow, &[20.0, 20.0, 10.0, 80.0])])] {
        let g = gap(&on_gpu(&mut gpu, &rs, &s, &steps), &on_cpu(&s, &steps));
        println!("dessin collé aux bords, {name} : moyenne {:.3} max {}", g.mean, g.max);
        assert!(g.mean <= 0.1 && g.max <= 2, "{name} : {g:?}");
    }
}
