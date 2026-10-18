//! Import des brosses TVPaint (moteur de dessin, étape 10) : archive, connexions, commandes,
//! papier, panneau fabriqué ici, et (essai ignoré) les panneaux livrés avec TVPaint 12.

use std::collections::HashMap;

use crate::brush::{Source, TipOrder};
use crate::brushtex::{BrushLibrary, MINE};
use crate::tvpbrush::{connection, curve_points, george_args, import_panel, paper_of, read_tar, Panel};

/// Une archive tar (ustar) faite de `files`.
fn tar(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, data) in files {
        let mut h = [0u8; 512];
        h[..name.len()].copy_from_slice(name.as_bytes());
        let size = format!("{:011o}\0", data.len());
        h[124..136].copy_from_slice(size.as_bytes());
        h[156] = b'0';
        h[257..263].copy_from_slice(b"ustar\0");
        out.extend_from_slice(&h);
        out.extend_from_slice(data);
        out.resize(out.len().div_ceil(512) * 512, 0);
    }
    out.extend_from_slice(&[0u8; 1024]);
    out
}

#[test]
fn a_tvpx_archive_is_read() {
    let bytes = tar(&[("config.txt", b"[001]\nkind=CustomPanel\n"), ("001/custompanel.bin", b"[toolbox]\n"), ("001/custompanel.data/a.dip", b"xyz")]);
    let files = read_tar(&bytes).unwrap();
    assert_eq!(files.get("001/custompanel.data/a.dip").map(Vec::as_slice), Some(&b"xyz"[..]));
    let p = Panel::from_tvpx(&bytes).unwrap();
    assert!(p.bin.contains("[toolbox]"));
    assert_eq!(p.data.get("a.dip").map(Vec::as_slice), Some(&b"xyz"[..]));
}

#[test]
fn connections_and_george_commands_are_read() {
    let mut skipped = Vec::new();
    let c = connection("10;4 1 0 0 0 0.281434 0.311568 0.483525 0.781272 1 1 ", &mut skipped).unwrap();
    assert_eq!((c.source, c.invert, c.curve.points().len()), (Source::Pressure, false, 4));
    assert!((c.curve.eval(0.483525) - 0.781272).abs() < 1e-4);
    let d = connection("-5;2 0 1 0 0 1 1", &mut skipped).unwrap();
    assert_eq!((d.source, d.invert), (Source::Direction, true));
    assert!(connection("0;2 0 1 0 0 1 1", &mut skipped).is_none());
    assert!(connection("11;2 0 1 0 0 1 1", &mut skipped).is_none());
    assert_eq!(skipped, vec!["inclinaison (altitude)".to_string()]);
    // Plus de points que Pelure n'en garde : pris à intervalles réguliers, bouts compris.
    let many = format!("12 0 1 {}", (0..12).map(|k| format!("{} {}", k as f32 / 11.0, k as f32 / 11.0)).collect::<Vec<_>>().join(" "));
    let pts = curve_points(&many).unwrap();
    assert_eq!(pts.len(), crate::brush::Curve::MAX);
    assert_eq!((pts[0], pts[pts.len() - 1]), ([0.0, 0.0], [1.0, 1.0]));
    let (name, a) = george_args(r#"tv_penbrush mode "color" size 10 power 50 aaliasing 0 csize "10;2 0 1 0 0 1 1""#);
    assert_eq!(name, "tv_penbrush");
    assert_eq!((a["size"].as_str(), a["power"].as_str(), a["csize"].as_str()), ("10", "50", "10;2 0 1 0 0 1 1"));
    let p = paper_of(r#"cmd: tv_paper Hardness 68 Invert 1 Size 0.600000 Angle 12 OffsetX 0 FlipX 0 Active 1 Name "/cement_02""#).unwrap();
    assert_eq!((p.name.as_str(), p.hardness, p.size, p.angle, p.invert), ("cement_02", 68.0, 0.6, 12.0, true));
    assert!(paper_of(r#"cmd: tv_paper Hardness 68 Active 0 Name "/x""#).is_none());
}

/// Une pointe `.dip` animée de deux images 2×2 (alpha seulement).
fn tiny_dip() -> Vec<u8> {
    let chunk = |id: &[u8; 4], data: &[u8]| {
        let mut v = id.to_vec();
        v.extend_from_slice(&(data.len() as u32).to_be_bytes());
        v.extend_from_slice(data);
        if data.len() % 2 == 1 {
            v.push(0);
        }
        v
    };
    let mut body = Vec::new();
    body.extend(chunk(b"DGBL", &[0, 2, 0, 2, 0, 1, 1, 1]));
    body.extend(chunk(b"DPEL", &[0, 0, 0, 4, 0, 3, 0, 8, 0, 2, 0, 8, 0, 1, 0, 8, 0, 17, 0, 8]));
    body.extend(chunk(b"DLOC", &[0, 2, 0, 2, 0, 0, 0, 0]));
    body.extend(chunk(b"DBOD", &[0xFD, 0, 0, 0, 255]));
    body.extend(chunk(b"DBAN", &[0x80, 2, 0xFF, 0, 0, 0, 0, 0x80]));
    let mut f = b"FORM".to_vec();
    f.extend_from_slice(&((body.len() + 4) as u32).to_be_bytes());
    f.extend_from_slice(b"DEEP");
    f.extend(body);
    f
}

#[test]
fn a_panel_becomes_brushes_with_their_tips_and_notes() {
    let ini = "\u{feff}[tvp tool]\nsize=200.000000\nopacity=80.000000\nstepon=1\nstepval=10.00\njitter=25\nanim=6\nc_size2=10;3 1 0 0 0 0.5 0.2 1 1\nc_opacity2=12;2 0 1 0 0 1 1\nc_angle2=8;2 0 1 0 0 1 1\nsmear=50\ndry=0\n";
    let bin = "\u{feff}[toolbox]\n0_cmd0=CBTool:abc\n0_cmd1=cmd: tv_paper Hardness 50 Invert 0 Size 2 Angle 0 Active 1 Name \"/Paper 33\"\n0_count=2\n0_text=Crayon gras\n\
               1_cmd0=Tool:tv_penbrush mode \"color\" size 12 power 40 opacity 100 aaliasing 0 csize \"10;2 0 1 0 0 1 1\"\n1_count=1\n1_text=Encre\n\
               2_cmd0=action:tv_undo\n2_count=1\n2_text=Annuler\n";
    let mut data = HashMap::new();
    data.insert("abc.ini".to_string(), ini.as_bytes().to_vec());
    data.insert("abc.dip".to_string(), tiny_dip());
    let mut lib = BrushLibrary::load(None);
    let before = lib.presets.len();
    let r = import_panel(&Panel { bin: bin.to_string(), data }, "essai.tvpx", &mut lib, None);
    assert_eq!((r.presets, r.tips, r.papers), (2, 1, 0));
    let mine: Vec<_> = lib.presets[before..].to_vec();
    let crayon = &mine[0];
    assert_eq!((crayon.name.as_str(), crayon.source.as_str()), ("Crayon gras", "essai.tvpx"));
    let s = crayon.settings;
    assert_eq!(s.size, 4.0, "200 % d'une pointe de 2 pixels");
    assert!((s.opacity - 0.8).abs() < 1e-6 && (s.spacing - 0.1).abs() < 1e-6 && (s.scatter - 0.5).abs() < 1e-6);
    assert_eq!((s.dynamics.size.source, s.dynamics.angle.source), (Source::Pressure, Source::Random));
    assert!(!s.dynamics.opacity.active(), "azimut : non lu");
    assert_eq!((s.tip_cells, s.tip_order), (2, TipOrder::Pressure));
    assert!(s.tip.is_some() && s.paper.is_none());
    let encre = mine[1].settings;
    assert_eq!((encre.size, encre.aliased, encre.build_up), (12.0, true, true));
    assert!((encre.power - 0.4).abs() < 1e-6 && encre.dynamics.size.source == Source::Pressure);
    let notes = r.notes.join(" | ");
    for want in ["azimut", "ne sèche pas", "papier « Paper 33 » introuvable", "pointe animée de 2 images", "1 bouton(s) sans outil"] {
        assert!(notes.contains(want), "« {want} » absent de : {notes}");
    }
    assert!(lib.presets.iter().all(|p| p.source != MINE));
}

/// Un panneau exporté par TVPaint 12 (`.tvpx`) écrit `[ToolBox]`, les panneaux livrés
/// `[toolbox]` : les sections sont lues sans tenir compte de la casse (l'export d'Anthony
/// n'importait rien). Le `step` d'un outil de base est pris en pour cent de la taille, et le
/// grain du crayon est noté comme non repris.
#[test]
fn an_exported_panel_with_capitalized_section_is_read() {
    let bin = "\u{feff}[ToolBox]\n0_cmd0=Tool:tv_pencil mode \"color\" size 9.500000 power 100 opacity 100 aspect 100 angle 35 step 12.500000 dry 1 gradient 0 grain 1 csize \"10;2 0 1 0 0 1 1 \" caspect \"0;2 0 1 0 0 1 1 \" cangle \"0;2 0 1 0 0 1 1 \" cpower \"10;2 0 1 0 0 1 1 \" profile \"3 1 1 0 0 0.0746112 0.737231 1 1 \" \n0_cmd1=cmd: tv_paper Active 0\n0_count=2\n0_text=Trait\n_name=Mon TVPX\n";
    let mut lib = BrushLibrary::load(None);
    let r = import_panel(&Panel { bin: bin.to_string(), data: HashMap::new() }, "mon.tvpx", &mut lib, None);
    assert_eq!(r.presets, 1, "{:?}", r.notes);
    let p = lib.presets.last().unwrap();
    let s = p.settings;
    assert_eq!((p.name.as_str(), s.size, s.opacity, s.aliased), ("Trait", 9.5, 1.0, false));
    assert_eq!((s.dynamics.size.source, s.dynamics.opacity.source), (Source::Pressure, Source::Pressure));
    assert!(s.profile.is_some());
    assert!((s.spacing - 0.125).abs() < 1e-6, "step 12.5 → 12,5 % de la taille, pas 12,5 px ({})", s.spacing);
    assert!(r.notes.iter().any(|n| n.contains("grain du crayon")), "{:?}", r.notes);
}

/// Les panneaux livrés avec TVPaint 12, lus sans rien y écrire (`PELURE_TVPAINT` peut désigner
/// le dossier `Resources`) : leurs brosses, pointes et papiers arrivent.
#[test]
#[ignore]
fn tvpaint_panels_import() {
    let res = crate::tvpbrush::tvpaint_resources().expect("TVPaint installé");
    for name in ["Pencils_v1", "Chalks_v1", "Pens_v1"] {
        let panel = Panel::from_bin(&res.join("custompanels").join(format!("{name}.bin"))).unwrap();
        let mut lib = BrushLibrary::load(None);
        let r = import_panel(&panel, name, &mut lib, Some(&res));
        println!("{name} : {} brosses, {} pointes, {} papiers", r.presets, r.tips, r.papers);
        for n in r.notes.iter().take(6) {
            println!("   {n}");
        }
        assert!(r.presets > 0, "{name}");
        // Pour voir : PELURE_DIP_PNG = dossier où écrire l'aperçu de chaque brosse importée.
        if let Some(out) = std::env::var_os("PELURE_DIP_PNG") {
            let (w, h) = (360usize, 70usize);
            let imported: Vec<_> = lib.presets.iter().filter(|p| p.source == name).take(8).cloned().collect();
            let mut img = image::RgbaImage::from_pixel(w as u32, (h * imported.len()) as u32, image::Rgba([255, 255, 255, 255]));
            for (k, p) in imported.iter().enumerate() {
                let mut s = p.settings;
                s.size = s.size.min(24.0);
                let a = crate::brush::stroke_preview(&s, lib.textures(&s), w, h);
                for (j, v) in a.iter().enumerate() {
                    let g = 255 - *v;
                    img.put_pixel((j % w) as u32, (k * h + j / w) as u32, image::Rgba([g, g, g, 255]));
                }
            }
            img.save(std::path::Path::new(&out).join(format!("{name}.png"))).unwrap();
        }
    }
}
