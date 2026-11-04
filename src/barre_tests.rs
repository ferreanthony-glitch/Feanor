//! La barre de chargement iridescente (`barre.rs`) : le dégradé (arrêts à 0, 0,38, 0,66 et 1), le décalage qui reste dans les
//! bornes, le maillage (ancré sur toute la largeur, sans bandes, bouts arrondis), et le repos (aucune image demandée quand rien
//! ne charge ou quand le mouvement est réduit).

use eframe::egui::{self, pos2, vec2, Color32, Rect};

use crate::barre::{self, Barre};
use crate::logo::{IRIS_LILAS, IRIS_PALE, IRIS_ROSE, JADE};

fn c(v: [u8; 3]) -> Color32 {
    Color32::from_rgb(v[0], v[1], v[2])
}

#[test]
fn the_gradient_has_its_four_stops_and_blends_between_them() {
    assert_eq!(barre::couleur_degrade(0.0), c(JADE));
    assert_eq!(barre::couleur_degrade(0.38), c(IRIS_LILAS));
    assert_eq!(barre::couleur_degrade(0.66), c(IRIS_ROSE));
    assert_eq!(barre::couleur_degrade(1.0), c(IRIS_PALE));
    // Entre deux arrêts : un mélange (le milieu de jade et lilas n'est ni l'un ni l'autre).
    let m = barre::couleur_degrade(0.19);
    assert_ne!(m, c(JADE));
    assert_ne!(m, c(IRIS_LILAS));
    assert!(m.r() > JADE[0] && m.r() < IRIS_LILAS[0] && m.b() > JADE[2].min(IRIS_LILAS[2]) - 1);
    // Hors bornes ou non fini : les bouts.
    assert_eq!(barre::couleur_degrade(-3.0), c(JADE));
    assert_eq!(barre::couleur_degrade(9.0), c(IRIS_PALE));
    assert_eq!(barre::couleur_degrade(f32::NAN), c(JADE));
}

#[test]
fn the_drift_goes_back_and_forth_within_its_range_with_a_5_point_2_second_period() {
    assert_eq!(barre::PERIODE, 5.2);
    let mut vu_haut = 0.0_f32;
    for k in 0..=1040 {
        let t = k as f64 * 0.01;
        let d = barre::derive(t);
        assert!((0.0..=barre::DERIVE_MAX + 1e-6).contains(&d), "décalage {d} à {t} s");
        vu_haut = vu_haut.max(d);
        // Quelle que soit la dérive, la couleur lue reste dans le dégradé (réfléchi, jamais hors des bouts).
        for x in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let u = barre::reflechir(x + d);
            assert!((0.0..=1.0).contains(&u));
        }
    }
    assert!(vu_haut > barre::DERIVE_MAX * 0.99, "il atteint le bout du va-et-vient");
    assert!(barre::derive(0.0) < 1e-6 && (barre::derive(5.2) - barre::derive(0.0)).abs() < 1e-5, "revient au départ après une période");
    assert!((barre::derive(2.6) - barre::DERIVE_MAX).abs() < 1e-4, "au bout d'une demi-période");
    assert!((barre::reflechir(1.2) - 0.8).abs() < 1e-6 && (barre::reflechir(-0.25) - 0.25).abs() < 1e-6);
}

#[test]
fn the_fill_mesh_is_anchored_on_the_whole_track_and_has_many_columns() {
    let piste = Rect::from_min_size(pos2(10.0, 20.0), vec2(300.0, 6.0));
    let plein = barre::maille(piste, 1.0, 0.0);
    let moitie = barre::maille(piste, 0.5, 0.0);
    assert!(plein.vertices.len() >= 2 * 100, "assez de colonnes pour ne pas voir de bandes : {}", plein.vertices.len());
    // Le premier sommet est jade, le dernier jaune pâle.
    assert_eq!(plein.vertices[0].color, c(JADE));
    assert_eq!(plein.vertices[plein.vertices.len() - 1].color, c(IRIS_PALE));
    // La moitié remplie s'arrête au milieu de la piste, avec la couleur qu'a le plein à cet endroit (ancrage).
    let fin = moitie.vertices.iter().map(|v| v.pos.x).fold(f32::MIN, f32::max);
    assert!((fin - 160.0).abs() < 0.01, "{fin}");
    let derniere = moitie.vertices[moitie.vertices.len() - 1].color;
    assert_eq!(derniere, barre::couleur_a(0.5, 0.0));
    // Les bouts sont arrondis : la première et la dernière colonne sont fines, le milieu est plein.
    let haut = |m: &egui::Mesh, k: usize| m.vertices[2 * k + 1].pos.y - m.vertices[2 * k].pos.y;
    let n = plein.vertices.len() / 2;
    assert!(haut(&plein, 0) < 0.01 && haut(&plein, n - 1) < 0.01, "pointes des bouts");
    assert!((haut(&plein, n / 2) - 6.0).abs() < 1e-3, "pleine hauteur au milieu");
    // Tous les sommets restent dans la piste, et deux colonnes voisines n'ont presque pas de saut de couleur (pas de bandes).
    for v in &plein.vertices {
        assert!(v.pos.x >= piste.left() - 1e-3 && v.pos.x <= piste.right() + 1e-3 && v.pos.y >= piste.top() - 1e-3 && v.pos.y <= piste.bottom() + 1e-3);
    }
    for k in 1..n {
        let (a, b) = (plein.vertices[2 * k - 2].color, plein.vertices[2 * k].color);
        let saut = (a.r() as i32 - b.r() as i32).abs().max((a.g() as i32 - b.g() as i32).abs()).max((a.b() as i32 - b.b() as i32).abs());
        assert!(saut <= 4, "saut de {saut} niveaux entre deux colonnes ({k})");
    }
    // Rien à remplir : pas de maillage. Une valeur absurde ne plante pas.
    assert!(barre::maille(piste, 0.0, 0.0).vertices.is_empty());
    assert!(barre::maille(piste, f32::NAN, 0.0).vertices.is_empty());
    assert!(!barre::maille(piste, 7.0, 0.0).vertices.is_empty());
    // Un remplissage plus court que la hauteur est une calotte, pas une bande carrée.
    let petit = barre::maille(piste, 0.01, 0.0);
    assert!(!petit.vertices.is_empty() && petit.vertices.iter().all(|v| v.pos.x <= piste.left() + 3.01));
    // Le décalage change les couleurs, pas la géométrie.
    let decale = barre::maille(piste, 1.0, 0.2);
    assert_eq!(decale.vertices.len(), plein.vertices.len());
    assert_ne!(decale.vertices[0].color, plein.vertices[0].color);
}

/// Une image de la barre ; rend le délai avant la prochaine image demandée (`None` : aucune).
fn image(ctx: &egui::Context, t: f64, barre: impl Fn() -> Barre) -> Option<std::time::Duration> {
    let sortie = ctx.run(egui::RawInput { time: Some(t), screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 200.0))), ..Default::default() }, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add(barre());
        });
    });
    sortie.viewport_output.values().map(|v| v.repaint_delay).min().filter(|d| *d < std::time::Duration::from_secs(3600))
}

#[test]
fn the_bar_repaints_only_while_something_loads_and_never_faster_than_needed() {
    let ctx = egui::Context::default();
    for k in 0..4 {
        let _ = image(&ctx, k as f64 * 0.01, || Barre::new(1.0));
    }
    // En cours : une image demandée, pas plus vite que toutes les 50 ms.
    let d = image(&ctx, 1.0, || Barre::new(0.4).text("42 / 100")).expect("elle demande une image tant que ça charge");
    // (egui retranche le temps déjà passé dans l'image : un peu moins que 50 ms, jamais 0.)
    assert!(d >= std::time::Duration::from_millis(15) && d <= barre::IMAGE_TOUTES_LES, "{d:?}");
    // Avancement inconnu : de même.
    assert!(image(&ctx, 2.0, || Barre::new(0.0).animate(true)).is_some());
    // Fini (100 %) : plus aucune image.
    let _ = image(&ctx, 3.0, || Barre::new(1.0).show_percentage());
    assert_eq!(image(&ctx, 3.1, || Barre::new(1.0).show_percentage()), None, "terminé : zéro image");
    // Mouvement réduit : le dégradé reste immobile, aucune image demandée même en cours.
    let calme = egui::Context::default();
    calme.style_mut(|s| s.animation_time = 0.0);
    for k in 0..4 {
        let _ = image(&calme, k as f64 * 0.01, || Barre::new(1.0));
    }
    assert_eq!(image(&calme, 1.0, || Barre::new(0.4)), None, "mouvement réduit : immobile");
}

/// Planche pour regarder une barre (essai ignoré) : `FEANOR_CAPTURE_DIR=… cargo test … barre_planche -- --ignored`.
#[test]
#[ignore]
fn barre_planche() {
    let Some(dir) = std::env::var_os("FEANOR_CAPTURE_DIR").map(std::path::PathBuf::from) else { return };
    // Rastérisation à la main des maillages (pas de carte graphique ici) : fond canevas, trois barres, pas de texte.
    let (l, h) = (620usize, 130usize);
    let mut px = vec![0u8; l * h * 3];
    let fond = crate::theme::ATELIER_PANNEAU;
    for p in px.chunks_exact_mut(3) {
        p.copy_from_slice(&fond);
    }
    let piste = crate::theme::ATELIER_CANEVAS;
    for (k, (frac, derive)) in [(1.0_f32, 0.0_f32), (0.62, 0.0), (0.35, 0.18)].into_iter().enumerate() {
        let rect = Rect::from_min_size(pos2(20.0, 20.0 + k as f32 * 36.0), vec2(580.0, 8.0));
        for y in rect.top() as usize..rect.bottom() as usize {
            for x in rect.left() as usize..rect.right() as usize {
                let i = (y * l + x) * 3;
                px[i..i + 3].copy_from_slice(&piste);
            }
        }
        let m = barre::maille(rect.shrink(1.0), frac, derive);
        for t in m.indices.chunks_exact(3) {
            let (a, b, c) = (m.vertices[t[0] as usize], m.vertices[t[1] as usize], m.vertices[t[2] as usize]);
            let (x0, x1) = (a.pos.x.min(b.pos.x).min(c.pos.x), a.pos.x.max(b.pos.x).max(c.pos.x));
            let (y0, y1) = (a.pos.y.min(b.pos.y).min(c.pos.y), a.pos.y.max(b.pos.y).max(c.pos.y));
            for y in y0.floor() as usize..=(y1.ceil() as usize).min(h - 1) {
                for x in x0.floor() as usize..=(x1.ceil() as usize).min(l - 1) {
                    // Point (x+.5, y+.5) dans le triangle ?
                    let (px_, py_) = (x as f32 + 0.5, y as f32 + 0.5);
                    let s = |p: egui::Pos2, q: egui::Pos2| (px_ - q.x) * (p.y - q.y) - (p.x - q.x) * (py_ - q.y);
                    let (d1, d2, d3) = (s(a.pos, b.pos), s(b.pos, c.pos), s(c.pos, a.pos));
                    if !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0)) {
                        let i = (y * l + x) * 3;
                        px[i..i + 3].copy_from_slice(&[a.color.r(), a.color.g(), a.color.b()]);
                    }
                }
            }
        }
    }
    image::save_buffer(dir.join("barre-iridescente.png"), &px, l as u32, h as u32, image::ExtendedColorType::Rgb8).unwrap();
}
