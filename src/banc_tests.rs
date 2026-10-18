//! Banc de traits (moteur de dessin, étape 1) : enregistrer, relire, rejouer.

use crate::banc::{append, read, replay, Recorded};
use crate::brush::BrushSettings;

#[test]
fn a_recorded_stroke_is_kept_and_replayed() {
    let dir = std::env::temp_dir().join(format!("pelure-banc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let s = BrushSettings { size: 10.0, smoothing: 0.0, ..BrushSettings::brush() };
    // (Des valeurs exactes en binaire : le texte JSON les relit sans arrondi.)
    let points: Vec<[f64; 4]> = (0..=20).map(|k| [100.0 + 5.0 * k as f64, 50.0, 0.25 + k as f64 / 32.0, k as f64 / 64.0]).collect();
    let r = Recorded { settings: s, color: [10, 20, 30], eraser: false, points, feel: Default::default() };
    let path = append(&dir, &r).unwrap();
    append(&dir, &Recorded { points: vec![[0.0, 0.0, 1.0, -1.0], [3.0, 4.0, 1.0, -1.0]], ..r.clone() }).unwrap();
    let back = read(&path).unwrap();
    assert_eq!(back.len(), 2);
    assert_eq!(back[0], r);
    // Rejoué : le trait est là, à sa place dans la feuille (décalée de la marge).
    let (raster, off) = replay(&back[0], Default::default());
    let (x, y) = ((150.0 + off[0]) as usize, (50.0 + off[1]) as usize);
    assert!(raster.get(x, y).a() > 200, "encre au milieu du trait");
    assert!(raster.get(x, y + 12).a() == 0, "rien loin du trait");
    // Le trait rejoué garde ses instants : la vitesse est connue.
    let mut st = crate::brush::StrokeState::new(0, crate::brush::StrokeMode::Paint([0, 0, 0]), s, None, true);
    let mut rr = crate::raster::Raster::new(64, 64);
    let mut dirty = Vec::new();
    st.add_point_at(&mut rr, eframe::egui::pos2(10.0, 10.0), 0.5, 1.5, &mut dirty);
    st.add_point(&mut rr, eframe::egui::pos2(20.0, 10.0), 0.6, &mut dirty);
    assert_eq!(st.raw_log, vec![[10.0, 10.0, 0.5, 1.5], [20.0, 10.0, 0.6000000238418579, -1.0]]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Les traits enregistrés par Anthony (`PELURE_TRAITS` : le fichier `traits.jsonl`), rejoués et
/// écrits en images à côté (pour comparer avec TVPaint).
#[test]
#[ignore]
fn recorded_strokes_replay() {
    let path = std::path::PathBuf::from(std::env::var_os("PELURE_TRAITS").expect("PELURE_TRAITS"));
    let lib = crate::brushtex::BrushLibrary::load(crate::brushtex::BrushLibrary::user_dir().as_deref());
    for (k, r) in read(&path).unwrap().iter().enumerate() {
        let (raster, _) = replay(r, lib.textures(&r.settings));
        let (w, h) = (raster.width, raster.height);
        let img = image::RgbaImage::from_fn(w as u32, h as u32, |x, y| {
            let a = raster.get(x as usize, y as usize).a();
            image::Rgba([255 - a, 255 - a, 255 - a, 255])
        });
        img.save(path.with_file_name(format!("trait_{k:03}.png"))).unwrap();
        println!("trait {k} : {} points, {w}×{h}", r.points.len());
    }
}