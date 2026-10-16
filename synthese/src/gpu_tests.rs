use crate::gpu::Gpu;
use crate::tests::{interior_diff, noise, position, waves};
use crate::{synthesize, GuidePair, Params, Synthesizer};

/// La carte graphique, s'il y en a une (sinon l'essai ne fait rien).
fn gpu() -> Option<Gpu> {
    match Gpu::new() {
        Ok(g) => Some(g),
        Err(e) => {
            eprintln!("pas de carte graphique : {e}");
            None
        }
    }
}

#[test]
fn same_guides_give_back_the_style() {
    let Some(gpu) = gpu() else { return };
    let style = noise(48, 40, 3, 11);
    let guide = noise(48, 40, 1, 12);
    let out = gpu.synthesize(&style, 1.0, &[GuidePair { source: &guide, target: &guide, weight: 1.0 }], &Params::default()).unwrap();
    assert_eq!((out.image.width, out.image.height, out.image.channels), (48, 40, 3));
    let (mean, same) = interior_diff(&out.image, &style, 0, 0, 4);
    assert!(mean < 1.0 && same > 0.97, "écart {mean}, identiques {same}");
}

#[test]
fn position_guide_moves_the_style() {
    let Some(gpu) = gpu() else { return };
    let style = waves(64, 64);
    let (src, tgt) = (position(64, 64, 0, 0), position(64, 64, 5, 3));
    let guides = [GuidePair { source: &src, target: &tgt, weight: 10.0 }];
    for stop_threshold in [5, 0] {
        let out = gpu.synthesize(&style, 1.0, &guides, &Params { stop_threshold, ..Params::default() }).unwrap();
        let (shifted, _) = interior_diff(&out.image, &style, 5, 3, 10);
        let (unshifted, _) = interior_diff(&out.image, &style, 0, 0, 10);
        assert!(shifted < 14.0 && 3.0 * shifted < unshifted, "seuil {stop_threshold} : {shifted} / {unshifted}");
    }
}

#[test]
fn final_cost_close_to_the_processor() {
    // Le coût moyen des patchs retenus (style + guides) : la carte doit faire à peu près aussi bien
    // que le processeur.
    let Some(gpu) = gpu() else { return };
    let style = waves(120, 90);
    let (src, tgt) = (position(120, 90, 0, 0), position(120, 90, -7, 4));
    let guide = noise(120, 90, 1, 5);
    let guides = [GuidePair { source: &src, target: &tgt, weight: 4.0 }, GuidePair { source: &guide, target: &guide, weight: 0.5 }];
    let mean = |e: &[f32]| e.iter().sum::<f32>() / e.len() as f32;
    let cpu = synthesize(&style, 1.0, &guides, &Params::default()).unwrap();
    let on_gpu = gpu.synthesize(&style, 1.0, &guides, &Params::default()).unwrap();
    let (c, g) = (mean(&cpu.error), mean(&on_gpu.error));
    assert!(g < 1.5 * c, "carte {g}, processeur {c}");
}

#[test]
fn blend_like_the_processor() {
    // Le mélange résolu sur la carte (SOR) donne la même image que le gradient conjugué.
    let Some(gpu) = gpu() else { return };
    use crate::blend::{blend, BlendParams, ErrorMap};
    let (w, h) = (96, 64);
    let a = noise(w, h, 3, 1);
    let b = waves(w, h);
    let ea = ErrorMap::new(&(0..w * h).map(|i| if i % w < w / 2 { 1.0 } else { 6.0 }).collect::<Vec<_>>(), w, h, 8);
    let eb = ErrorMap::new(&vec![3.0; w * h], w, h, 8);
    for alpha in [0.2, 0.5, 0.8] {
        let cpu = blend(&a, &ea, &b, &eb, alpha, &BlendParams::default());
        let on_gpu = gpu.blend(&a, &ea, &b, &eb, alpha, &BlendParams::default());
        let max = cpu.data.iter().zip(&on_gpu.data).map(|(x, y)| x.abs_diff(*y)).max().unwrap();
        assert!(max <= 2, "α {alpha} : écart {max}");
    }
}

#[test]
fn extra_pass_and_different_sizes() {
    let Some(gpu) = gpu() else { return };
    let style = noise(30, 26, 4, 41);
    let (src, tgt) = (position(30, 26, 0, 0), position(50, 34, 0, 0));
    let params = Params { extra_pass_3x3: true, ..Params::default() };
    let out = gpu.synthesize(&style, 1.0, &[GuidePair { source: &src, target: &tgt, weight: 1.0 }], &params).unwrap();
    assert_eq!((out.image.width, out.image.height, out.image.channels), (50, 34, 4));
    assert_eq!((out.nnf.len(), out.error.len()), (50 * 34, 50 * 34));
    assert!(out.nnf.iter().all(|n| (1..29).contains(&n[0]) && (1..25).contains(&n[1])));
}
