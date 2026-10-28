//! L'aperçu du tremblement par défaut (`tremblementapercu.rs`, Préférences > Dessin) : peu coûteux (le moteur de brosses ne
//! dessine le trait d'exemple qu'une fois, les images se refont à chaque changement de réglage et jamais à chaque image) ;
//! immobile tant que « Animer à l'arrêt » est éteint, il cycle les variantes à la cadence du réglage quand il est allumé.

use std::collections::BTreeSet;
use std::sync::atomic::Ordering;

use eframe::egui::{self, epaint::Shape, RawInput, Rect, TextureId};

use crate::tremblement::Defaut;
use crate::tremblementapercu::{self as ap, apercu_ui, variante_montree};

/// Une image d'écran avec l'aperçu de `d` ; rend les images (hors police) que cette image a peintes.
fn image(ctx: &egui::Context, d: &Defaut, fps: u32, t: f64) -> BTreeSet<TextureId> {
    let input = RawInput { screen_rect: Some(Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(600.0, 400.0))), time: Some(t), ..Default::default() };
    let out = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| apercu_ui(ui, d, fps));
    });
    fn parcourir(s: &Shape, out: &mut BTreeSet<TextureId>) {
        match s {
            Shape::Mesh(m) if m.texture_id != TextureId::Managed(0) => {
                out.insert(m.texture_id);
            }
            Shape::Vec(v) => v.iter().for_each(|s| parcourir(s, out)),
            _ => {}
        }
    }
    let mut ids = BTreeSet::new();
    out.shapes.iter().for_each(|s| parcourir(&s.shape, &mut ids));
    ids
}

fn nerveux() -> Defaut {
    Defaut { actif: true, intensite: 8.0, frequence: 70.0, repos: true, ..Defaut::default() }
}

#[test]
fn the_brush_engine_draws_the_sample_once_and_the_images_are_made_once_per_setting_change() {
    let ctx = egui::Context::default();
    let d = nerveux();
    let mut t = 0.0;
    for _ in 0..60 {
        image(&ctx, &d, 12, t);
        t += 1.0 / 60.0;
    }
    assert_eq!(ap::calculs(&ctx), d.variantes, "une image par variante, calculées une fois pour soixante images d'écran");
    assert_eq!(ap::RENDUS_DE_BASE.load(Ordering::Relaxed), 1, "le trait d'exemple est dessiné une seule fois par la séance");
    // Un réglage change : les images se refont, une fois.
    let d2 = Defaut { intensite: 12.0, variantes: 5, ..d };
    for _ in 0..30 {
        image(&ctx, &d2, 12, t);
        t += 1.0 / 60.0;
    }
    assert_eq!(ap::calculs(&ctx), d.variantes + 5);
    // Allumer ou éteindre le tremblement par défaut lui-même ne change pas l'aperçu des réglages.
    image(&ctx, &Defaut { actif: false, ..d2 }, 12, t);
    assert_eq!(ap::calculs(&ctx), d.variantes + 5, "seuls les réglages comptent");
    assert_eq!(ap::RENDUS_DE_BASE.load(Ordering::Relaxed), 1);
}

#[test]
fn the_preview_cycles_its_variants_only_when_animating_at_rest_is_on() {
    let ctx = egui::Context::default();
    let mut vus_allume = BTreeSet::new();
    let mut vus_eteint = BTreeSet::new();
    let eteint = Defaut { repos: false, ..nerveux() };
    for k in 0..120 {
        let t = k as f64 / 60.0;
        vus_allume.extend(image(&ctx, &nerveux(), 12, t));
    }
    for k in 120..240 {
        let t = k as f64 / 60.0;
        vus_eteint.extend(image(&ctx, &eteint, 12, t));
    }
    assert_eq!(vus_allume.len(), 3, "trois variantes, toutes montrées en deux secondes : {vus_allume:?}");
    assert_eq!(vus_eteint.len(), 1, "éteint, l'aperçu reste immobile : {vus_eteint:?}");
}

#[test]
fn the_shown_variant_follows_the_project_rate_and_the_speed_setting() {
    // 12 images par seconde, 2 images par variante, 3 variantes : 6 images (une demi-seconde) pour un tour.
    let d = Defaut { repos: true, vitesse: 2, variantes: 3, ..Defaut::default() };
    let vus: Vec<usize> = (0..12).map(|k| variante_montree(&d, 12, k as f64 / 12.0 + 0.001)).collect();
    assert_eq!(vus, vec![0, 0, 1, 1, 2, 2, 0, 0, 1, 1, 2, 2]);
    // Plus vite : une variante par image ; plus de variantes : un tour plus long.
    let vite = Defaut { vitesse: 1, ..d };
    assert_eq!((0..4).map(|k| variante_montree(&vite, 12, k as f64 / 12.0 + 0.001)).collect::<Vec<_>>(), vec![0, 1, 2, 0]);
    let six = Defaut { variantes: 6, vitesse: 1, ..d };
    assert_eq!((0..7).map(|k| variante_montree(&six, 12, k as f64 / 12.0 + 0.001)).collect::<Vec<_>>(), vec![0, 1, 2, 3, 4, 5, 0]);
    // Immobile quand on n'anime pas à l'arrêt.
    assert!((0..20).all(|k| variante_montree(&Defaut { repos: false, ..d }, 12, k as f64 / 7.0) == 0));
}
