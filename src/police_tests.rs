//! Mesure du coût de la police de l'interface : temps d'une image d'une fenêtre pleine de texte (Préférences > Général),
//! à froid (les 20 premières images : polices, atlas de glyphes) et à chaud (la moyenne des 100 suivantes).
//! Essai ignoré : `cargo test --release police_mesure -- --ignored --nocapture`.

use crate::document::Document;
use crate::perf_tests::banc_taille;

#[test]
#[ignore]
fn police_mesure() {
    let mut b = banc_taille(Document::with_frame(160, 90, 24), false, 1600, 900);
    b.app.prefs_window = true;
    let froid = std::time::Instant::now();
    for _ in 0..20 {
        b.image(Vec::new(), 1.0 / 60.0);
    }
    let froid = froid.elapsed().as_secs_f64() * 1000.0 / 20.0;
    let mut mesures = Vec::new();
    for _ in 0..100 {
        let t = std::time::Instant::now();
        b.image(Vec::new(), 1.0 / 60.0);
        mesures.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    mesures.sort_by(|a, c| a.partial_cmp(c).unwrap());
    let moyenne = mesures.iter().sum::<f64>() / mesures.len() as f64;
    println!("POLICE froid {froid:.2} ms/image ; chaud moyenne {moyenne:.2} ms, médiane {:.2} ms, p90 {:.2} ms", mesures[50], mesures[90]);
}