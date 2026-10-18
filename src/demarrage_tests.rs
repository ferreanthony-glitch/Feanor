//! Le démarrage avec beaucoup de pinceaux importés (Anthony : des centaines) : la bibliothèque se
//! lit sur plusieurs cœurs, et la répartition d'un papier se compte sur un histogramme quand
//! l'image est en 256 niveaux. Les mêmes valeurs qu'avant, au bit près.

use crate::brush::BrushSettings;
use crate::brushtex::{content_key, shared_key, BrushLibrary, GrayImage, Preset, Target, TextureKind};

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
}

/// L'ancien calcul de `GrayImage::equalized` : un tri des indices.
fn equalized_reference(img: &GrayImage) -> Vec<f32> {
    let n = img.values.len();
    if n < 2 {
        return img.values.clone();
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| img.values[a].total_cmp(&img.values[b]));
    let d = (n - 1) as f32;
    let mut ranked = vec![0.0f32; n];
    let mut i = 0;
    while i < n {
        let mut j = i;
        while j + 1 < n && img.values[order[j + 1]] == img.values[order[i]] {
            j += 1;
        }
        let r = (i + j) as f32 / 2.0 / d;
        for &k in &order[i..=j] {
            ranked[k] = r;
        }
        i = j + 1;
    }
    ranked
}

fn bytes_image(w: usize, h: usize, seed: u64, levels: u32) -> GrayImage {
    let mut rng = Lcg(seed);
    // Des niveaux peu nombreux (beaucoup d'égalités) ou tous, comme un fichier de 8 bits.
    GrayImage::new(w, h, (0..w * h).map(|_| (rng.next() % levels) as u8 as f32 * (255 / levels.max(1)) as f32 / 255.0).collect())
}

#[test]
fn the_histogram_equalization_gives_the_values_of_the_sort() {
    for (k, (w, h, levels)) in [(64, 64, 256u32), (100, 37, 40), (33, 1, 3), (257, 129, 256), (8, 8, 1), (2, 1, 256)].into_iter().enumerate() {
        let img = bytes_image(w, h, k as u64 + 1, levels);
        let want = equalized_reference(&img);
        let got = img.clone().equalized();
        assert_eq!(got.values, want, "image {w}x{h} à {levels} niveaux");
        assert_eq!((got.width, got.height), (w, h));
    }
    // Un octet quelconque (k / 255 pour tout k).
    let all = GrayImage::new(256, 4, (0..1024).map(|i| (i % 256) as f32 / 255.0).collect());
    assert_eq!(all.clone().equalized().values, equalized_reference(&all));
    // Des valeurs qui ne sont pas des niveaux d'octet : le tri, comme avant.
    let mut rng = Lcg(7);
    let floats = GrayImage::new(50, 50, (0..2500).map(|_| rng.next() as f32 / u32::MAX as f32).collect());
    assert_eq!(floats.clone().equalized().values, equalized_reference(&floats));
    // Une valeur hors de 0..1, une valeur non finie : le tri aussi.
    let odd = GrayImage::new(4, 1, vec![0.0, 2.0, 0.5, f32::NAN]);
    let got = odd.clone().equalized().values;
    let want = equalized_reference(&odd);
    assert!(got.iter().zip(&want).all(|(a, b)| a.to_bits() == b.to_bits()));
    // Une seule valeur : tout au même rang.
    let flat = GrayImage::new(5, 5, vec![0.25; 25]);
    assert_eq!(flat.clone().equalized().values, equalized_reference(&flat));
}

#[test]
fn a_saved_brush_library_loads_the_same_on_several_cores() {
    let mut lib = BrushLibrary::builtin();
    let mut rng = Lcg(5);
    let mut tips = Vec::new();
    let mut papers = Vec::new();
    // Plus de huit textures : le chemin parallèle.
    for k in 0..14 {
        let n = 24 + k * 3;
        tips.push(lib.add_tip(&format!("Pointe {k}"), GrayImage::new(n, n, (0..n * n).map(|_| (rng.next() % 256) as f32 / 255.0).collect())));
    }
    for k in 0..6 {
        let n = 40 + k * 5;
        papers.push(lib.add_paper(&format!("Papier {k}"), GrayImage::new(n, n, (0..n * n).map(|_| rng.next() as f32 / u32::MAX as f32).collect())));
    }
    for k in 0..30 {
        lib.presets.push(Preset { name: format!("Importé {k}"), target: Target::Brush, settings: BrushSettings::brush(), source: "Essai.abr".into() });
    }
    let dir = std::env::temp_dir().join(format!("shot-demarrage-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    lib.save(&dir).expect("enregistrement");
    let loaded = BrushLibrary::load(Some(&dir));
    assert_eq!(loaded.presets.len(), lib.presets.len());
    for &id in &tips {
        let file = image::open(dir.join(format!("texture_{id}.png"))).expect("fichier").to_luma8();
        let want = GrayImage::new(file.width() as usize, file.height() as usize, file.as_raw().iter().map(|&v| v as f32 / 255.0).collect());
        let got = loaded.textures(&BrushSettings { tip: Some(id), ..BrushSettings::brush() }).tip.expect("pointe");
        assert_eq!(got.full(), &want, "pointe {id}");
    }
    for &id in &papers {
        let file = image::open(dir.join(format!("texture_{id}.png"))).expect("fichier").to_luma8();
        let raw = GrayImage::new(file.width() as usize, file.height() as usize, file.as_raw().iter().map(|&v| v as f32 / 255.0).collect());
        let got = loaded.textures(&BrushSettings { paper: Some(id), ..BrushSettings::brush() }).paper.expect("papier");
        assert_eq!(got.values, equalized_reference(&raw), "papier {id}");
    }
    // Les clés de contenu partagées sont celles du calcul d'une texture à la fois.
    for &id in &tips {
        let t = loaded.textures(&BrushSettings { tip: Some(id), ..BrushSettings::brush() }).tip.unwrap();
        assert_eq!(shared_key(TextureKind::Tip, id), Some(content_key(TextureKind::Tip, t.full())));
    }
    for &id in &papers {
        let p = loaded.textures(&BrushSettings { paper: Some(id), ..BrushSettings::brush() }).paper.unwrap();
        assert_eq!(shared_key(TextureKind::Paper, id), Some(content_key(TextureKind::Paper, &p)));
    }
    let _ = std::fs::remove_dir_all(&dir);
}
