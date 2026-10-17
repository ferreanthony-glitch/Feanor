//! Images `.dip` de TVPaint : un fichier fabriqué ici, et (essai ignoré) les pointes de
//! l'installation de TVPaint 12, lues sans rien y écrire.

use crate::dip::{chunks, read, runs, tip_strip};

/// Un bloc IFF.
fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut v = id.to_vec();
    v.extend_from_slice(&(data.len() as u32).to_be_bytes());
    v.extend_from_slice(data);
    if data.len() % 2 == 1 {
        v.push(0);
    }
    v
}

/// Un fichier DEEP de 3×2 pixels (bleu, vert, rouge, alpha « 17 »), deux images.
fn sample() -> Vec<u8> {
    let mut body = Vec::new();
    body.extend(chunk(b"DGBL", &[0, 3, 0, 2, 0, 1, 1, 1]));
    body.extend(chunk(b"DPEL", &[0, 0, 0, 4, 0, 3, 0, 8, 0, 2, 0, 8, 0, 1, 0, 8, 0, 17, 0, 8]));
    body.extend(chunk(b"DLOC", &[0, 3, 0, 2, 0, 0, 0, 0]));
    // Image 1 : un pixel rouge opaque, puis cinq pixels transparents (une plage).
    body.extend(chunk(b"DBOD", &[0x00, 0, 0, 255, 255, 0xFC, 0, 0, 0, 0]));
    // Image 2 : sauter 4 pixels, puis deux pixels bleus à moitié.
    body.extend(chunk(b"DBAN", &[0x80, 4, 0xFF, 128, 0, 0, 128, 0x80]));
    let mut f = b"FORM".to_vec();
    f.extend_from_slice(&((body.len() + 4) as u32).to_be_bytes());
    f.extend_from_slice(b"DEEP");
    f.extend(body);
    f
}

#[test]
fn a_deep_image_is_read_with_its_animation() {
    let d = read(&sample()).unwrap();
    assert_eq!((d.width, d.height, d.frames.len()), (3, 2, 2));
    assert_eq!(d.frames[0][0], [255, 0, 0, 255], "bleu, vert, rouge, alpha remis en RVBA");
    assert_eq!(d.frames[0][1], [0, 0, 0, 0]);
    assert_eq!(d.frames[1][0], [255, 0, 0, 255], "un pixel sauté garde l'image d'avant");
    assert_eq!(d.frames[1][4], [0, 0, 128, 128]);
    assert_eq!(d.frames[1][5], [0, 0, 128, 128]);
    // En pointe : les images côte à côte, l'alpha fait le dépôt.
    let g = tip_strip(&d);
    assert_eq!((g.width, g.height), (6, 2));
    assert_eq!((g.get(0, 0), g.get(3, 0), g.get(4, 1)), (1.0, 1.0, 128.0 / 255.0));
    assert!(read(b"FORM\0\0\0\x04ILBM").is_err());
}

/// Les pointes livrées avec TVPaint 12 (lecture seule) : chaque image se décode en exactement
/// largeur × hauteur pixels. `PELURE_TVPAINT` peut désigner le dossier `Resources`.
#[test]
#[ignore]
fn tvpaint_tips_decode_exactly() {
    let res = std::env::var_os("PELURE_TVPAINT").map(std::path::PathBuf::from).unwrap_or_else(|| {
        std::path::PathBuf::from(r"C:\Program Files\TVPaint Developpement\TVPaint Animation 12 Pro\Resources")
    });
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(res.join("Default Config").join("Brushes")).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "dip")).collect();
    for dir in ["Pencils_v1.data", "Chalks_v1.data", "Smudge_v1.data"] {
        if let Ok(rd) = std::fs::read_dir(res.join("custompanels").join(dir)) {
            files.extend(rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "dip") && !p.file_name().unwrap().to_string_lossy().starts_with('_')).take(4));
        }
    }
    for f in files {
        let bytes = std::fs::read(&f).unwrap();
        let d = read(&bytes).unwrap_or_else(|e| panic!("{} : {e}", f.display()));
        // Chaque bloc d'image couvre exactement l'image.
        let cs = chunks(&bytes).unwrap();
        let n = d.width * d.height;
        let mut buf = vec![0u8; n * 4];
        let mut counts = Vec::new();
        for (id, data) in &cs {
            if *id == "DBOD" || *id == "DBAN" {
                counts.push(runs(data, &mut buf, 4, *id == "DBAN"));
            }
        }
        let alpha: Vec<f32> = d.frames.iter().map(|fr| fr.iter().map(|p| p[3] as f32).sum::<f32>() / n as f32).collect();
        let jumps: Vec<f32> = d.frames.windows(2).map(|w| w[0].iter().zip(&w[1]).map(|(a, b)| (a[3] as f32 - b[3] as f32).abs()).sum::<f32>() / n as f32).collect();
        let max_jump = jumps.iter().cloned().fold(0.0, f32::max);
        println!(
            "{} : {}×{}, {} image(s), alpha moyen {:.1}…{:.1}, saut d'alpha max entre images {:.2}",
            f.file_name().unwrap().to_string_lossy(),
            d.width,
            d.height,
            d.frames.len(),
            alpha.iter().cloned().fold(f32::MAX, f32::min),
            alpha.iter().cloned().fold(0.0, f32::max),
            max_jump
        );
        assert!(counts.iter().all(|&c| c == n), "{} : pixels décodés {counts:?} pour {n}", f.display());
        // Pour voir : PELURE_DIP_PNG = dossier où écrire les 8 premières images en bande.
        if let Some(out) = std::env::var_os("PELURE_DIP_PNG") {
            let k = d.frames.len().min(8);
            let mut img = image::RgbaImage::new((d.width * k) as u32, d.height as u32);
            for (i, fr) in d.frames.iter().take(k).enumerate() {
                for (j, p) in fr.iter().enumerate() {
                    let (x, y) = (j % d.width, j / d.width);
                    // Sur fond blanc : le dépôt (alpha) en noir.
                    let v = 255 - p[3];
                    img.put_pixel((i * d.width + x) as u32, y as u32, image::Rgba([v, v, v, 255]));
                }
            }
            let name = f.file_stem().unwrap().to_string_lossy().into_owned();
            img.save(std::path::Path::new(&out).join(format!("{name}.png"))).unwrap();
        }
    }
}