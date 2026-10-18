//! Import des brosses Krita et MyPaint (moteur de dessin, étape 10) : réglage fabriqué ici,
//! capteurs, pointes `.gbr`, base64, MyPaint ; essai ignoré sur les réglages de Krita du poste.

use std::io::Write;

use crate::brush::Source;
use crate::brushtex::{BrushLibrary, Target};
use crate::kritabrush::{base64, import_kpp, import_myb, kpp_text, read_gbr, sensor};

/// Un `.kpp` : une image PNG minimale portant `xml` dans un bloc `zTXt` « preset ».
fn kpp(xml: &str) -> Vec<u8> {
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(xml.as_bytes()).unwrap();
    let mut data = b"preset\0\0".to_vec();
    data.extend(z.finish().unwrap());
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    for (kind, body) in [(&b"IHDR"[..], &[0u8, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0][..]), (&b"zTXt"[..], &data[..]), (&b"IEND"[..], &[][..])] {
        png.extend_from_slice(&(body.len() as u32).to_be_bytes());
        png.extend_from_slice(kind);
        png.extend_from_slice(body);
        png.extend_from_slice(&[0, 0, 0, 0]);
    }
    png
}

const PRESET: &str = r#"<Preset name="Feutre rond" paintopid="paintbrush">
 <param name="brush_definition" type="string"><![CDATA[<Brush type="auto_brush" spacing="0.15" angle="0"><MaskGenerator diameter="24" ratio="0.5" hfade="0.8" vfade="0.8" type="circle"/></Brush>]]></param>
 <param name="PressureSize" type="string"><![CDATA[true]]></param>
 <param name="SizeSensor" type="string"><![CDATA[<!DOCTYPE params> <params id="pressure"> <curve>0,0;0.35,0.1;1,1;</curve> </params>]]></param>
 <param name="PressureRotation" type="string"><![CDATA[true]]></param>
 <param name="RotationSensor" type="string"><![CDATA[<params id="drawingangle"><curve>0,0;1,1;</curve></params>]]></param>
 <param name="PressureOpacity" type="string"><![CDATA[true]]></param>
 <param name="OpacitySensor" type="string"><![CDATA[<params id="xtilt"><curve>0,0;1,1;</curve></params>]]></param>
 <param name="OpacityValue" type="string"><![CDATA[0.7]]></param>
 <param name="FlowValue" type="string"><![CDATA[0.5]]></param>
 <param name="EraserMode" type="string"><![CDATA[false]]></param>
</Preset>"#;

#[test]
fn a_krita_preset_becomes_a_brush() {
    let bytes = kpp(PRESET);
    assert!(kpp_text(&bytes).unwrap().contains("Feutre rond"));
    let mut lib = BrushLibrary::load(None);
    let r = import_kpp(&bytes, "feutre.kpp", &mut lib, None).unwrap();
    assert_eq!((r.presets, r.tips), (1, 0));
    let p = lib.presets.last().unwrap();
    assert_eq!((p.name.as_str(), p.target, p.source.as_str()), ("Feutre rond", Target::Brush, "feutre.kpp"));
    let s = p.settings;
    assert_eq!((s.size, s.roundness, s.hardness), (24.0, 0.5, 0.8));
    assert!((s.spacing - 0.15).abs() < 1e-6 && (s.opacity - 0.7).abs() < 1e-6 && (s.power - 0.5).abs() < 1e-6);
    assert!(s.build_up, "le Flow de Krita s'accumule dans le trait");
    assert_eq!((s.dynamics.size.source, s.dynamics.angle.source), (Source::Pressure, Source::Direction));
    assert!((s.dynamics.size.curve.eval(0.35) - 0.1).abs() < 1e-5, "la courbe de Krita");
    assert!(!s.dynamics.opacity.active());
    assert!(r.notes.join(" ").contains("xtilt"), "{:?}", r.notes);
    // Un autre moteur que « paintbrush » : rien d'importé, dit.
    let other = kpp(r#"<Preset name="Fumée" paintopid="particle"></Preset>"#);
    let r = import_kpp(&other, "fumee.kpp", &mut lib, None).unwrap();
    assert_eq!(r.presets, 0);
    assert!(r.notes[0].contains("particle"));
    assert!(kpp_text(b"pas un png").is_err());
}

#[test]
fn sensors_gbr_tips_and_base64_are_read() {
    let mut skipped = Vec::new();
    let c = sensor(r#"<params id="sensorslist"><ChildSensor id="speed"><curve>0,1;1,0;</curve></ChildSensor></params>"#, &mut skipped).unwrap();
    assert_eq!(c.source, Source::Speed);
    assert!((c.curve.eval(0.0) - 1.0).abs() < 1e-5 && c.curve.eval(1.0).abs() < 1e-5);
    assert!(sensor(r#"<params id="rotation"/>"#, &mut skipped).is_none());
    assert_eq!(skipped, vec!["rotation".to_string()]);
    // Une pointe .gbr 3×3 en gris, un point noir sur fond blanc : retournée (1 où elle dépose).
    let mut gbr = Vec::new();
    for v in [28u32 + 5, 2, 3, 3, 1] {
        gbr.extend_from_slice(&v.to_be_bytes());
    }
    gbr.extend_from_slice(b"GIMP");
    gbr.extend_from_slice(&25u32.to_be_bytes());
    gbr.extend_from_slice(b"rond\0");
    gbr.extend_from_slice(&[255, 255, 255, 255, 0, 255, 255, 255, 255]);
    let g = read_gbr(&gbr).unwrap();
    assert_eq!((g.width, g.height), (3, 3));
    assert!((g.get(1, 1) - 1.0).abs() < 1e-5 && g.get(0, 0).abs() < 1e-5);
    assert_eq!(base64("UGVsdXJl").unwrap(), b"Pelure");
    assert_eq!(base64("UGVsdQ==").unwrap(), b"Pelu");
}

#[test]
fn a_mypaint_brush_becomes_a_brush() {
    let json = r#"{"settings": {
        "radius_logarithmic": {"base_value": 1.0, "inputs": {"pressure": [[0.0, -1.0], [1.0, 0.0]]}},
        "opaque": {"base_value": 0.9, "inputs": {}},
        "opaque_multiply": {"base_value": 0.0, "inputs": {"pressure": [[0.0, 0.0], [1.0, 1.0]]}},
        "hardness": {"base_value": 0.6, "inputs": {"pressure": [[0.0, 0.0], [1.0, 0.3]]}},
        "dabs_per_actual_radius": {"base_value": 4.0, "inputs": {}},
        "elliptical_dab_ratio": {"base_value": 2.0, "inputs": {}},
        "elliptical_dab_angle": {"base_value": 120.0, "inputs": {}},
        "offset_by_random": {"base_value": 0.5, "inputs": {}},
        "eraser": {"base_value": 0.0, "inputs": {}}
    }}"#;
    let mut lib = BrushLibrary::load(None);
    let r = import_myb(json, "Crayon 2B", "crayon.myb", &mut lib).unwrap();
    let s = lib.presets.last().unwrap().settings;
    // Rayon e¹ au plus fort appui : diamètre 2e.
    assert!((s.size - 2.0 * std::f32::consts::E).abs() < 0.05, "{}", s.size);
    // À l'appui nul, le rayon est e⁰ : un e-ième de la taille.
    assert_eq!(s.dynamics.size.source, Source::Pressure);
    assert!((s.dynamics.size.curve.eval(0.0) - (-1.0f32).exp()).abs() < 0.02);
    // `opaque` 0,9 : un dépôt par empreinte qui s'accumule (rendu, étape 2), sous une opacité pleine.
    assert!((s.power - 0.9).abs() < 1e-6 && s.build_up && s.opacity == 1.0 && s.dynamics.opacity.source == Source::Pressure);
    assert!((s.hardness - 0.6).abs() < 1e-6 && (s.spacing - 0.125).abs() < 1e-6);
    assert!((s.roundness - 0.5).abs() < 1e-6 && (s.angle - 30.0).abs() < 1e-4 && (s.scatter - 0.25).abs() < 1e-6);
    assert!(r.notes.join(" ").contains("dureté selon l'appui"));
    assert!(import_myb("{", "x", "x.myb", &mut lib).is_err());
}

/// Les réglages Krita du poste (`%APPDATA%\krita\paintoppresets`), lus sans rien y écrire.
#[test]
#[ignore]
fn krita_presets_of_this_computer_import() {
    let dir = std::path::PathBuf::from(std::env::var_os("APPDATA").unwrap()).join("krita").join("paintoppresets");
    let brushes = dir.parent().unwrap().join("brushes");
    let mut lib = BrushLibrary::load(None);
    let bundles: Vec<Vec<u8>> = std::fs::read_dir(dir.parent().unwrap()).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "bundle")).filter_map(|p| std::fs::read(p).ok()).collect();
    let zips: Vec<crate::zipread::Zip> = bundles.iter().filter_map(|b| crate::zipread::Zip::open(b).ok()).collect();
    for (b, z) in bundles.iter().zip(&zips) {
        let r = crate::kritabrush::import_bundle(b, "paquet", &mut BrushLibrary::load(None)).unwrap();
        println!("paquet de {} fichiers : {} réglages, {} pointes", z.names().count(), r.presets, r.tips);
    }
    let (mut ok, mut skipped) = (0, 0);
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let p = e.path();
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let r = match p.extension().and_then(|e| e.to_str()) {
            Some("kpp") => {
                let find = |f: &str| std::fs::read(brushes.join(f)).ok().or_else(|| zips.iter().find_map(|z| z.find(f)));
                crate::kritabrush::import_kpp_with(&std::fs::read(&p).unwrap(), &name, &mut lib, &find)
            }
            Some("myb") => import_myb(&std::fs::read_to_string(&p).unwrap(), &name, &name, &mut lib),
            _ => continue,
        };
        match r {
            Ok(r) => {
                ok += r.presets;
                skipped += 1 - r.presets.min(1);
                println!("{name} : {} réglage(s) {:?}", r.presets, r.notes);
            }
            Err(e) => println!("{name} : ERREUR {e}"),
        }
    }
    println!("{ok} importés, {skipped} sautés");
    assert!(ok > 0);
}

/// Une archive zip : chaque fichier compressé (« deflate ») sauf le premier, rangé tel quel.
pub(crate) fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let (mut out, mut central) = (Vec::new(), Vec::new());
    for (k, (name, data)) in files.iter().enumerate() {
        let (method, body): (u16, Vec<u8>) = if k == 0 {
            (0, data.to_vec())
        } else {
            let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            e.write_all(data).unwrap();
            (8, e.finish().unwrap())
        };
        let offset = out.len() as u32;
        let header = |sig: u32, central: bool| {
            let mut h = sig.to_le_bytes().to_vec();
            if central {
                h.extend_from_slice(&20u16.to_le_bytes());
            }
            h.extend_from_slice(&20u16.to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes());
            h.extend_from_slice(&method.to_le_bytes());
            h.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]);
            h.extend_from_slice(&(body.len() as u32).to_le_bytes());
            h.extend_from_slice(&(data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(name.len() as u16).to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes());
            if central {
                h.extend_from_slice(&[0u8; 10]);
                h.extend_from_slice(&offset.to_le_bytes());
            }
            h.extend_from_slice(name.as_bytes());
            h
        };
        out.extend(header(0x0403_4b50, false));
        out.extend_from_slice(&body);
        central.extend(header(0x0201_4b50, true));
    }
    let start = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&start.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

#[test]
fn a_zip_archive_is_read() {
    let bytes = zip(&[("a.txt", "rangé tel quel".as_bytes()), ("dossier/b.txt", "compressé compressé compressé".as_bytes())]);
    let z = crate::zipread::Zip::open(&bytes).unwrap();
    assert_eq!(z.names().collect::<Vec<_>>(), vec!["a.txt", "dossier/b.txt"]);
    assert_eq!(z.read("a.txt").unwrap(), "rangé tel quel".as_bytes());
    assert_eq!(z.find("b.txt").unwrap(), "compressé compressé compressé".as_bytes());
    assert!(z.read("absent").is_none());
    assert!(crate::zipread::Zip::open(b"pas un zip").is_err());
}

#[test]
fn a_krita_bundle_brings_its_presets_and_tips() {
    // Un réglage dont la pointe est une image du paquet.
    let preset = PRESET.replace(
        r#"<Brush type="auto_brush" spacing="0.15" angle="0"><MaskGenerator diameter="24" ratio="0.5" hfade="0.8" vfade="0.8" type="circle"/></Brush>"#,
        r#"<Brush type="png_brush" filename="tache.png" spacing="0.2" scale="0.5" angle="0"/>"#,
    );
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_fn(40, 20, |x, _| image::Rgba([0, 0, 0, if x < 20 { 255 } else { 0 }])))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let bundle = zip(&[("mimetype", b"application/x-krita-resourcebundle"), ("paintoppresets/tache.kpp", &kpp(&preset)), ("brushes/tache.png", &png)]);
    let mut lib = BrushLibrary::load(None);
    let r = crate::kritabrush::import_bundle(&bundle, "paquet.bundle", &mut lib).unwrap();
    assert_eq!((r.presets, r.tips), (1, 1), "{:?}", r.notes);
    let s = lib.presets.last().unwrap().settings;
    assert!(s.tip.is_some());
    assert_eq!(s.size, 20.0, "40 pixels à l'échelle 0,5");
}