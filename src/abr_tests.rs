//! Brosses Photoshop (`.abr`) : fichiers fabriqués ici d'après la structure lue par GIMP et Krita
//! (aucune vraie brosse sur le poste).

use crate::abr::read;
use crate::brush::Source;

fn be16(v: u16) -> [u8; 2] {
    v.to_be_bytes()
}
fn be32(v: u32) -> [u8; 4] {
    v.to_be_bytes()
}

/// Une pointe 4×2 : bornes, profondeur 8, compression par plages (PackBits par ligne).
fn sampled_rle() -> Vec<u8> {
    let mut b = Vec::new();
    for v in [0u32, 0, 2, 4] {
        b.extend(be32(v));
    }
    b.extend(be16(8));
    b.push(1);
    // Ligne 1 : quatre fois 255 (répétition) ; ligne 2 : 0, 64, 128, 255 (tels quels).
    let rows: [&[u8]; 2] = [&[0xFD, 255], &[3, 0, 64, 128, 255]];
    for r in rows {
        b.extend(be16(r.len() as u16));
    }
    for r in rows {
        b.extend_from_slice(r);
    }
    b
}

#[test]
fn a_version_2_sampled_brush_is_read() {
    let mut body = Vec::new();
    body.extend(be32(0));
    body.extend(be16(25));
    let name: Vec<u16> = "Craie\0".encode_utf16().collect();
    body.extend(be32(name.len() as u32));
    for u in name {
        body.extend(be16(u));
    }
    body.push(1);
    body.extend([0u8; 8]);
    body.extend(sampled_rle());
    let mut f = Vec::new();
    f.extend(be16(2));
    f.extend(be16(2));
    // Une brosse calculée (sautée), puis l'échantillonnée.
    f.extend(be16(1));
    f.extend(be32(4));
    f.extend([0u8; 4]);
    f.extend(be16(2));
    f.extend(be32(body.len() as u32));
    f.extend(body);
    let mut notes = Vec::new();
    let brushes = read(&f, &mut notes).unwrap();
    assert_eq!(brushes.len(), 1);
    let b = &brushes[0];
    assert_eq!(b.name, "Craie");
    let t = b.tip.as_ref().unwrap();
    assert_eq!((t.width, t.height), (4, 2));
    assert_eq!((t.get(3, 0), t.get(0, 1), t.get(3, 1)), (1.0, 0.0, 1.0));
    assert!((t.get(1, 1) - 64.0 / 255.0).abs() < 1e-5);
    assert_eq!(b.settings.size, 4.0);
    assert!((b.settings.spacing - 0.25).abs() < 1e-6);
    assert!(notes[0].contains("calculée"));
}

/// Un élément de descripteur : clé (quatre lettres, sinon précédée de sa longueur), type, valeur.
fn item(key: &str, ty: &str, val: &[u8]) -> Vec<u8> {
    let mut v = if key.len() == 4 { be32(0).to_vec() } else { be32(key.len() as u32).to_vec() };
    v.extend_from_slice(key.as_bytes());
    v.extend_from_slice(ty.as_bytes());
    v.extend_from_slice(val);
    v
}

fn obj(items: &[Vec<u8>]) -> Vec<u8> {
    let mut v = be32(0).to_vec();
    v.extend(be32(0));
    v.extend_from_slice(b"null");
    v.extend(be32(items.len() as u32));
    for i in items {
        v.extend_from_slice(i);
    }
    v
}

fn text(s: &str) -> Vec<u8> {
    let u: Vec<u16> = format!("{s}\0").encode_utf16().collect();
    let mut v = be32(u.len() as u32).to_vec();
    for x in u {
        v.extend(be16(x));
    }
    v
}

fn unit(v: f64) -> Vec<u8> {
    let mut b = b"#Pxl".to_vec();
    b.extend_from_slice(&v.to_be_bytes());
    b
}

#[test]
fn a_version_6_brush_comes_with_its_preset() {
    let id = "$0123456789abcdef0123456789abcdef012";
    // Section samp : une pointe (identifiant en chaîne Pascal, préambule de 10 octets).
    let mut tip = vec![id.len() as u8];
    tip.extend_from_slice(id.as_bytes());
    tip.extend([0u8; 10]);
    tip.extend(sampled_rle());
    let mut samp = be32(tip.len() as u32).to_vec();
    samp.extend(&tip);
    while samp.len() % 4 != 0 {
        samp.push(0);
    }
    // Section desc : un préréglage nommé, sa pointe liée, l'appui sur la taille.
    let brsh = obj(&[item("Dmtr", "UntF", &unit(30.0)), item("Angl", "UntF", &unit(45.0)), item("Rndn", "UntF", &unit(50.0)), item("Spcn", "UntF", &unit(10.0)), item("sampledData", "TEXT", &text(id))]);
    let size_var = obj(&[item("bVTy", "long", &be32(2)), item("Mnm ", "UntF", &unit(20.0))]);
    let preset = obj(&[item("Nm  ", "TEXT", &text("Fusain")), item("Brsh", "Objc", &brsh), item("szVr", "Objc", &size_var), item("useScatter", "bool", &[1])]);
    let mut list = b"Objc".to_vec();
    list.extend(preset);
    let mut top = be32(1).to_vec();
    top.extend(list);
    let mut desc = be32(16).to_vec();
    desc.extend(obj(&[item("Brsh", "VlLs", &top)]));
    let mut f = Vec::new();
    f.extend(be16(6));
    f.extend(be16(1));
    for (key, data) in [("samp", &samp), ("desc", &desc)] {
        f.extend_from_slice(b"8BIM");
        f.extend_from_slice(key.as_bytes());
        f.extend(be32(data.len() as u32));
        f.extend_from_slice(data);
        if data.len() % 2 == 1 {
            f.push(0);
        }
    }
    let mut notes = Vec::new();
    let brushes = read(&f, &mut notes).unwrap();
    assert_eq!(brushes.len(), 1, "{notes:?}");
    let b = &brushes[0];
    assert_eq!(b.name, "Fusain");
    assert!(b.tip.is_some(), "la pointe liée par son identifiant");
    let s = b.settings;
    assert_eq!((s.size, s.angle, s.roundness), (30.0, 45.0, 0.5));
    assert!((s.spacing - 0.1).abs() < 1e-6);
    assert_eq!(s.dynamics.size.source, Source::Pressure);
    assert!((s.dynamics.size.min - 0.2).abs() < 1e-6);
    assert!(notes.iter().any(|n| n.contains("dispersion")));
    assert!(read(&[0, 3, 0, 0], &mut notes).is_err());
}