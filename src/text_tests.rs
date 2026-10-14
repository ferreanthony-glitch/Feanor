//! Tests du texte (5d) : mise en page, police de secours, élément vectoriel (rendu, trous des
//! lettres, transformation, sélection, gomme, enregistrement).

use eframe::egui::{pos2, vec2};

use crate::raster::Raster;
use crate::text::{self, FALLBACK};
use crate::vector::{self, Affine, Element, VectorDrawing};

#[test]
fn layout_in_the_built_in_font() {
    let a = text::layout("Pelure", FALLBACK, 40.0);
    assert_eq!(a.font, FALLBACK);
    assert_eq!(a.glyphs.len(), 6);
    assert!(a.bounds.width() > 80.0 && a.bounds.height() >= 40.0, "{:?}", a.bounds);
    // Deux fois plus grand : deux fois plus large.
    let b = text::layout("Pelure", FALLBACK, 80.0);
    assert!((b.bounds.width() / a.bounds.width() - 2.0).abs() < 0.01);
    // Espaces : pas de lettre, mais de la place.
    let spaced = text::layout("P e", FALLBACK, 40.0);
    assert_eq!(spaced.glyphs.len(), 2);
    // Deux lignes : deux fois plus haut à peu près, aussi large que la plus longue.
    let two = text::layout("Pe\nPelure", FALLBACK, 40.0);
    assert!(two.bounds.height() > a.bounds.height() * 1.8);
    assert!((two.bounds.width() - a.bounds.width()).abs() < 0.01);
    // Les lettres sont sous la ligne du haut et dans la boîte (à peu près).
    for p in a.glyphs.iter().flatten().flatten() {
        assert!(p[1] >= -1.0 && p[1] <= a.bounds.max.y + 1.0, "{p:?}");
    }
}

/// Polices du poste : `cargo test --release system_fonts -- --ignored --nocapture`.
#[test]
#[ignore]
fn system_fonts() {
    let t = std::time::Instant::now();
    let names = text::font_names();
    println!("{} polices lues en {:?} ; Arial : {}", names.len(), t.elapsed(), names.iter().any(|n| n == "Arial"));
    let t = std::time::Instant::now();
    let l = text::layout("Bonjour Pelure", "Arial", 64.0);
    println!("mise en page en {:?}, police {}", t.elapsed(), l.font);
}

#[test]
fn a_missing_font_falls_back() {
    let l = text::layout("Abc", "Police qui n'existe pas", 30.0);
    assert_eq!(l.font, FALLBACK);
    assert_eq!(l.glyphs.len(), 3);
    assert!(text::font_names().iter().any(|n| n == FALLBACK));
}

fn painted(r: &Raster, x: usize, y: usize) -> bool {
    r.get(x, y).a() > 128
}

#[test]
fn letters_keep_their_holes() {
    let e = Element::text("O".into(), FALLBACK.into(), 100.0, [200, 0, 0], Affine::translate(vec2(10.0, 10.0)));
    let b = e.bounds();
    let mut r = Raster::new(200, 200);
    vector::render_element(&mut r, &e);
    let c = b.center();
    assert!(!painted(&r, c.x as usize, c.y as usize), "le trou du O reste vide");
    // Le trait du O, sur le côté gauche, au milieu de la hauteur.
    let left = (b.min.x..b.center().x).step_by_f32(0.5).into_iter().any(|x| painted(&r, x as usize, c.y as usize));
    assert!(left, "le O est dessiné");
    // Haut du O (contours de la lettre, pas la boîte de la ligne).
    let (_, glyphs) = e.text_shape().unwrap();
    let top = glyphs.iter().flatten().flatten().map(|p| p[1]).fold(f32::MAX, f32::min);
    let px = r.get(c.x as usize, (top + 2.0) as usize);
    assert!(px.r() > 0 && px.g() == 0, "couleur du texte : {px:?}");
}

trait StepF32 {
    fn step_by_f32(self, step: f32) -> Vec<f32>;
}

impl StepF32 for std::ops::Range<f32> {
    fn step_by_f32(self, step: f32) -> Vec<f32> {
        let mut v = Vec::new();
        let mut x = self.start;
        while x < self.end {
            v.push(x);
            x += step;
        }
        v
    }
}

#[test]
fn text_moves_hits_and_is_saved() {
    let e = Element::text("Hi".into(), FALLBACK.into(), 50.0, [0, 0, 0], Affine::translate(vec2(100.0, 50.0)));
    let b = e.bounds();
    assert!(b.min.x >= 99.0 && b.min.y >= 49.0, "{b:?}");
    assert!(e.hit(b.center(), 0.0));
    assert!(!e.hit(b.center() + vec2(500.0, 0.0), 0.0));
    // Déplacé et agrandi par la sélection : la boîte suit.
    let moved = e.transformed(&Affine::translate(vec2(30.0, -10.0)));
    assert!((moved.bounds().min.x - b.min.x - 30.0).abs() < 0.01);
    let bigger = e.transformed(&Affine::scale_about(b.min, 2.0, 2.0));
    assert!((bigger.bounds().width() / b.width() - 2.0).abs() < 0.05);
    // Le lasso le prend.
    let lasso = [pos2(90.0, 40.0), pos2(400.0, 40.0), pos2(400.0, 200.0), pos2(90.0, 200.0)];
    assert!(e.touches(&lasso));
    // Enregistré et relu tel quel.
    let d = VectorDrawing { elements: vec![e.clone()] };
    let json = serde_json::to_string(&d).unwrap();
    assert!(json.contains("\"police\"") && json.contains("\"placement\""), "{json}");
    let back: VectorDrawing = serde_json::from_str(&json).unwrap();
    assert_eq!(back, d);
    // La gomme ne le coupe pas ; en mode « entier » elle le retire.
    let mut d2 = d.clone();
    assert!(!vector::erase_circle(&mut d2, b.center(), 5.0, false));
    assert!(vector::erase_circle(&mut d2, b.center(), 5.0, true));
    assert!(d2.elements.is_empty());
}
