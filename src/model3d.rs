//! Modèles 3D de repère (2d) : lecture glTF 2.0 (`.glb`, `.gltf`), maillage soudé, arêtes
//! utiles. Lecteur écrit pour Pelure (pas de bibliothèque à ajouter) : il prend les triangles
//! (positions et indices) des maillages de la scène, avec les transformations des nœuds, et
//! ignore le reste (matières, textures, animations). Non lus : accesseurs « clairsemés »,
//! compression Draco ou quantifiée (refusés avec un message).
//!
//! Dans le projet, un modèle est gardé dans son propre format (`modeles/<id>.pelure3d`) :
//! positions et triangles seulement, indépendant du fichier d'origine.

use std::path::Path;

use serde_json::Value;

/// Maillage en triangles.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh3d {
    pub positions: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
}

/// Un modèle importé : maillage soudé et arêtes à dessiner.
#[derive(Debug)]
pub struct Model3d {
    pub id: u64,
    /// Nom du fichier d'origine (affiché).
    pub name: String,
    pub mesh: Mesh3d,
    /// Arêtes utiles : bords, cassures (faces voisines à plus de `CREASE` degrés), arêtes
    /// partagées par plus de deux faces. Pas les diagonales des faces planes. Les plus longues
    /// d'abord.
    pub edges: Vec<[u32; 2]>,
    /// Boîte englobante (min, max).
    pub bounds: ([f32; 3], [f32; 3]),
}

/// Angle au-delà duquel une arête entre deux faces est dessinée.
pub const CREASE: f32 = 25.0;

impl Model3d {
    pub fn new(id: u64, name: String, mesh: Mesh3d) -> Model3d {
        let mesh = weld(&mesh);
        let mut edges = feature_edges(&mesh, CREASE);
        // Les plus longues d'abord : l'aperçu pendant qu'on tourne n'en prend que le début.
        let length = |e: &[u32; 2]| {
            let (a, b) = (mesh.positions[e[0] as usize], mesh.positions[e[1] as usize]);
            (0..3).map(|k| (a[k] - b[k]) * (a[k] - b[k])).sum::<f32>()
        };
        edges.sort_by(|a, b| length(b).total_cmp(&length(a)).then(a.cmp(b)));
        let bounds = bounds_of(&mesh.positions);
        Model3d { id, name, mesh, edges, bounds }
    }
}

pub fn bounds_of(points: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for p in points {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    if points.is_empty() {
        ([0.0; 3], [0.0; 3])
    } else {
        (lo, hi)
    }
}

// ------------------------------------------------------------------------------------- glTF

/// Lit un fichier glTF 2.0 (`.glb` binaire, ou `.gltf` texte avec ses tampons à côté ou intégrés).
pub fn load_gltf(path: &Path) -> Result<Mesh3d, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{} : {e}", path.display()))?;
    let dir = path.parent().unwrap_or(Path::new("."));
    parse_gltf(&bytes, Some(dir))
}

/// `bytes` : le fichier ; `dir` : son dossier, pour les tampons externes (aucun : refusés).
pub fn parse_gltf(bytes: &[u8], dir: Option<&Path>) -> Result<Mesh3d, String> {
    let (json, bin) = if bytes.starts_with(b"glTF") {
        parse_glb(bytes)?
    } else {
        let text = std::str::from_utf8(bytes).map_err(|_| "fichier glTF illisible (ni .glb ni texte)")?;
        let json: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("glTF illisible : {e}"))?;
        (json, None)
    };
    let version = json["asset"]["version"].as_str().unwrap_or("");
    if !version.starts_with('2') {
        return Err(format!("glTF version « {version} » : seule la version 2 est lue"));
    }
    for ext in json["extensionsRequired"].as_array().into_iter().flatten() {
        let name = ext.as_str().unwrap_or("?");
        return Err(format!("le fichier exige l'extension glTF « {name} » (compression ?), non lue par SHOT : réexporte sans compression"));
    }
    let buffers = load_buffers(&json, bin, dir)?;
    let mut mesh = Mesh3d::default();
    // Nœuds de la scène par défaut (ou de la première), avec leurs transformations.
    let scene = json["scene"].as_u64().unwrap_or(0) as usize;
    let roots: Vec<usize> = match json["scenes"].get(scene) {
        Some(s) => s["nodes"].as_array().into_iter().flatten().filter_map(|n| n.as_u64()).map(|n| n as usize).collect(),
        // Pas de scène : tous les nœuds qui ne sont l'enfant d'aucun autre.
        None => {
            let nodes = json["nodes"].as_array().map_or(0, |a| a.len());
            let children: Vec<usize> = json["nodes"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|n| n["children"].as_array().into_iter().flatten().filter_map(|c| c.as_u64()))
                .map(|c| c as usize)
                .collect();
            (0..nodes).filter(|i| !children.contains(i)).collect()
        }
    };
    let mut stack: Vec<(usize, [f32; 16], usize)> = roots.into_iter().map(|n| (n, IDENTITY, 0)).collect();
    while let Some((n, parent, depth)) = stack.pop() {
        if depth > 64 {
            return Err("hiérarchie de nœuds trop profonde (boucle ?)".into());
        }
        let node = &json["nodes"][n];
        if node.is_null() {
            continue;
        }
        let world = mul(&parent, &node_matrix(node));
        if let Some(m) = node["mesh"].as_u64() {
            add_mesh(&json, &buffers, m as usize, &world, &mut mesh)?;
        }
        for c in node["children"].as_array().into_iter().flatten().filter_map(|c| c.as_u64()) {
            stack.push((c as usize, world, depth + 1));
        }
    }
    if mesh.triangles.is_empty() {
        return Err("aucun triangle dans ce fichier".into());
    }
    Ok(mesh)
}

fn parse_glb(bytes: &[u8]) -> Result<(Value, Option<Vec<u8>>), String> {
    let u32_at = |i: usize| bytes.get(i..i + 4).map(|b| u32::from_le_bytes(b.try_into().expect("4 octets")) as usize);
    let total = u32_at(8).ok_or("fichier .glb tronqué")?.min(bytes.len());
    let mut i = 12;
    let (mut json, mut bin) = (None, None);
    while i + 8 <= total {
        let len = u32_at(i).ok_or("fichier .glb tronqué")?;
        let kind = &bytes[i + 4..i + 8];
        let data = bytes.get(i + 8..i + 8 + len).ok_or("fichier .glb tronqué")?;
        match kind {
            b"JSON" => json = Some(serde_json::from_slice::<Value>(data).map_err(|e| format!("glTF illisible : {e}"))?),
            b"BIN\0" => bin = Some(data.to_vec()),
            _ => {}
        }
        i += 8 + len;
    }
    Ok((json.ok_or("fichier .glb sans description")?, bin))
}

fn load_buffers(json: &Value, mut bin: Option<Vec<u8>>, dir: Option<&Path>) -> Result<Vec<Vec<u8>>, String> {
    let mut out = Vec::new();
    for (i, b) in json["buffers"].as_array().into_iter().flatten().enumerate() {
        let data = match b["uri"].as_str() {
            None if i == 0 => bin.take().ok_or("tampon binaire manquant dans le .glb")?,
            None => return Err("tampon sans données".into()),
            Some(uri) if uri.starts_with("data:") => {
                let b64 = uri.split_once(";base64,").map(|x| x.1).ok_or("tampon intégré sans base64")?;
                base64_decode(b64)?
            }
            Some(uri) => {
                let dir = dir.ok_or("tampon externe : fichier introuvable")?;
                let name = percent_decode(uri);
                // On reste dans le dossier du fichier glTF.
                if Path::new(&name).components().any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::RootDir | std::path::Component::Prefix(_))) {
                    return Err(format!("tampon « {name} » hors du dossier du fichier"));
                }
                std::fs::read(dir.join(&name)).map_err(|e| format!("tampon « {name} » : {e}"))?
            }
        };
        out.push(data);
    }
    Ok(out)
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    let hex = |c: u8| (c as char).to_digit(16);
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    };
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for &c in s.as_bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        acc = (acc << 6) | val(c).ok_or("base64 invalide")?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

/// Octets d'un accesseur : (données de la vue, décalage, pas entre éléments, nombre).
fn accessor<'a>(json: &Value, buffers: &'a [Vec<u8>], index: usize, elem: usize) -> Result<(&'a [u8], usize, usize, usize), String> {
    let acc = &json["accessors"][index];
    if acc.is_null() {
        return Err(format!("accesseur {index} manquant"));
    }
    if !acc["sparse"].is_null() {
        return Err("accesseur « clairsemé » non lu : réexporte sans cette option".into());
    }
    let count = acc["count"].as_u64().unwrap_or(0) as usize;
    let Some(view) = acc["bufferView"].as_u64() else {
        return Err("accesseur sans données".into());
    };
    let view = &json["bufferViews"][view as usize];
    let buffer = buffers.get(view["buffer"].as_u64().unwrap_or(0) as usize).ok_or("tampon manquant")?;
    let start = view["byteOffset"].as_u64().unwrap_or(0) as usize + acc["byteOffset"].as_u64().unwrap_or(0) as usize;
    let stride = view["byteStride"].as_u64().map_or(elem, |s| s as usize).max(elem);
    let end = start + if count == 0 { 0 } else { stride * (count - 1) + elem };
    if end > buffer.len() {
        return Err("accesseur hors de son tampon".into());
    }
    Ok((buffer, start, stride, count))
}

fn read_positions(json: &Value, buffers: &[Vec<u8>], index: usize) -> Result<Vec<[f32; 3]>, String> {
    let acc = &json["accessors"][index];
    if acc["componentType"].as_u64() != Some(5126) || acc["type"].as_str() != Some("VEC3") {
        return Err("positions non flottantes (fichier quantifié ?) : réexporte sans compression".into());
    }
    let (buf, start, stride, count) = accessor(json, buffers, index, 12)?;
    Ok((0..count)
        .map(|i| {
            let o = start + i * stride;
            let f = |k: usize| f32::from_le_bytes(buf[o + 4 * k..o + 4 * k + 4].try_into().expect("4 octets"));
            [f(0), f(1), f(2)]
        })
        .collect())
}

fn read_indices(json: &Value, buffers: &[Vec<u8>], index: usize) -> Result<Vec<u32>, String> {
    let size = match json["accessors"][index]["componentType"].as_u64() {
        Some(5121) => 1,
        Some(5123) => 2,
        Some(5125) => 4,
        _ => return Err("indices d'un type inconnu".into()),
    };
    let (buf, start, stride, count) = accessor(json, buffers, index, size)?;
    Ok((0..count)
        .map(|i| {
            let o = start + i * stride;
            match size {
                1 => buf[o] as u32,
                2 => u16::from_le_bytes([buf[o], buf[o + 1]]) as u32,
                _ => u32::from_le_bytes(buf[o..o + 4].try_into().expect("4 octets")),
            }
        })
        .collect())
}

fn add_mesh(json: &Value, buffers: &[Vec<u8>], m: usize, world: &[f32; 16], out: &mut Mesh3d) -> Result<(), String> {
    for prim in json["meshes"][m]["primitives"].as_array().into_iter().flatten() {
        let mode = prim["mode"].as_u64().unwrap_or(4);
        if !(4..=6).contains(&mode) {
            continue; // points et lignes : rien à dessiner en surface
        }
        let Some(pos) = prim["attributes"]["POSITION"].as_u64() else { continue };
        let positions = read_positions(json, buffers, pos as usize)?;
        let indices = match prim["indices"].as_u64() {
            Some(i) => read_indices(json, buffers, i as usize)?,
            None => (0..positions.len() as u32).collect(),
        };
        let base = out.positions.len() as u32;
        out.positions.extend(positions.iter().map(|p| transform(world, *p)));
        let n = positions.len() as u32;
        let ok = |t: [u32; 3]| t.iter().all(|&i| i < n) && t[0] != t[1] && t[1] != t[2] && t[0] != t[2];
        let tris: Vec<[u32; 3]> = match mode {
            4 => indices.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect(),
            5 => indices
                .windows(3)
                .enumerate()
                .map(|(k, c)| if k % 2 == 0 { [c[0], c[1], c[2]] } else { [c[1], c[0], c[2]] })
                .collect(),
            _ => indices.iter().skip(1).zip(indices.iter().skip(2)).map(|(&b, &c)| [indices[0], b, c]).collect(),
        };
        out.triangles.extend(tris.into_iter().filter(|&t| ok(t)).map(|t| t.map(|i| i + base)));
    }
    Ok(())
}

// --------------------------------------------------------------------------------- matrices

/// Matrice 4×4 en colonnes (comme glTF).
const IDENTITY: [f32; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

fn mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut m = [0.0; 16];
    for c in 0..4 {
        for r in 0..4 {
            m[c * 4 + r] = (0..4).map(|k| a[k * 4 + r] * b[c * 4 + k]).sum();
        }
    }
    m
}

fn transform(m: &[f32; 16], p: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|r| m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2] + m[12 + r])
}

fn node_matrix(node: &Value) -> [f32; 16] {
    let nums = |v: &Value, n: usize, default: &[f32]| -> Vec<f32> {
        match v.as_array() {
            Some(a) if a.len() == n => a.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect(),
            _ => default.to_vec(),
        }
    };
    if node["matrix"].is_array() {
        let m = nums(&node["matrix"], 16, &IDENTITY);
        return m.try_into().unwrap_or(IDENTITY);
    }
    let t = nums(&node["translation"], 3, &[0.0, 0.0, 0.0]);
    let q = nums(&node["rotation"], 4, &[0.0, 0.0, 0.0, 1.0]);
    let s = nums(&node["scale"], 3, &[1.0, 1.0, 1.0]);
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    // Rotation (quaternion) × échelle, puis translation.
    let r = [
        1.0 - 2.0 * (y * y + z * z),
        2.0 * (x * y + z * w),
        2.0 * (x * z - y * w),
        2.0 * (x * y - z * w),
        1.0 - 2.0 * (x * x + z * z),
        2.0 * (y * z + x * w),
        2.0 * (x * z + y * w),
        2.0 * (y * z - x * w),
        1.0 - 2.0 * (x * x + y * y),
    ];
    [
        r[0] * s[0], r[1] * s[0], r[2] * s[0], 0.0,
        r[3] * s[1], r[4] * s[1], r[5] * s[1], 0.0,
        r[6] * s[2], r[7] * s[2], r[8] * s[2], 0.0,
        t[0], t[1], t[2], 1.0,
    ]
}

// ------------------------------------------------------------------------- soudure, arêtes

/// Fusionne les sommets confondus (les exports dupliquent les sommets aux coutures de textures
/// ou de normales) : sans cela, chaque face aurait ses propres arêtes de bord.
/// Les sommets gardent l'ordre de leur première apparition : souder un maillage déjà soudé le
/// rend tel quel.
pub fn weld(mesh: &Mesh3d) -> Mesh3d {
    let (lo, hi) = bounds_of(&mesh.positions);
    let size = (0..3).map(|k| hi[k] - lo[k]).fold(0.0f32, f32::max).max(1e-9);
    let cell = size * 1e-6;
    let keys: Vec<[i64; 3]> = mesh.positions.iter().map(|p| p.map(|v| (v / cell).round() as i64)).collect();
    // Tri (stable) des sommets par case : les sommets confondus se suivent, le premier en tête.
    let mut order: Vec<u32> = (0..keys.len() as u32).collect();
    order.sort_by_key(|&i| keys[i as usize]);
    let mut first = vec![0u32; keys.len()];
    for group in order.chunk_by(|&a, &b| keys[a as usize] == keys[b as usize]) {
        for &i in group {
            first[i as usize] = group[0];
        }
    }
    let mut positions = Vec::new();
    let mut remap = vec![0u32; keys.len()];
    for i in 0..keys.len() {
        let f = first[i] as usize;
        remap[i] = if f == i {
            positions.push(mesh.positions[i]);
            (positions.len() - 1) as u32
        } else {
            remap[f]
        };
    }
    let triangles = mesh
        .triangles
        .iter()
        .map(|t| t.map(|i| remap[i as usize]))
        .filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2])
        .collect();
    Mesh3d { positions, triangles }
}

fn normal(m: &Mesh3d, t: &[u32; 3]) -> [f32; 3] {
    let [a, b, c] = t.map(|i| m.positions[i as usize]);
    let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
    let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-20);
    n.map(|x| x / len)
}

/// Arêtes à dessiner : bords (une seule face), cassures (faces à plus de `crease` degrés, et à
/// moins de 180 − `crease` : une lame très fine n'est pas dessinée) et arêtes partagées par plus
/// de deux faces.
pub fn feature_edges(mesh: &Mesh3d, crease: f32) -> Vec<[u32; 2]> {
    // Chaque côté de triangle, repéré par ses deux sommets (le plus petit d'abord) ; triés, les
    // côtés d'une même arête se suivent.
    let mut sides: Vec<(u64, u32)> = Vec::with_capacity(mesh.triangles.len() * 3);
    for (f, t) in mesh.triangles.iter().enumerate() {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            sides.push(((a.min(b) as u64) << 32 | a.max(b) as u64, f as u32));
        }
    }
    sides.sort_unstable();
    let normals: Vec<[f32; 3]> = mesh.triangles.iter().map(|t| normal(mesh, t)).collect();
    let cos = crease.to_radians().cos();
    sides
        .chunk_by(|a, b| a.0 == b.0)
        .filter(|faces| match faces {
            [_] => true,
            [(_, a), (_, b)] => {
                let (na, nb) = (normals[*a as usize], normals[*b as usize]);
                // Faces à moins de `crease` degrés : arête intérieure, cachée. Valeur absolue : une
                // face retournée (fréquent dans les décors) ne fait pas apparaître ses diagonales.
                (na[0] * nb[0] + na[1] * nb[1] + na[2] * nb[2]).abs() < cos
            }
            _ => true,
        })
        .map(|faces| [(faces[0].0 >> 32) as u32, faces[0].0 as u32])
        .collect()
}

// ------------------------------------------------------------------------ format de Pelure

const MAGIC: &[u8; 8] = b"PELURE3D";

/// Le maillage dans le format de Pelure : en-tête, nombre de sommets et de triangles, puis les
/// positions (3 flottants) et les triangles (3 entiers), en petit-boutiste.
pub fn to_bytes(mesh: &Mesh3d) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&(mesh.positions.len() as u32).to_le_bytes());
    out.extend_from_slice(&(mesh.triangles.len() as u32).to_le_bytes());
    for p in &mesh.positions {
        for v in p {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    for t in &mesh.triangles {
        for i in t {
            out.extend_from_slice(&i.to_le_bytes());
        }
    }
    out
}

pub fn from_bytes(bytes: &[u8]) -> Result<Mesh3d, String> {
    if !bytes.starts_with(MAGIC) || bytes.len() < 20 {
        return Err("modèle 3D illisible".into());
    }
    let u = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().expect("4 octets"));
    if u(8) != 1 {
        return Err("modèle 3D d'une version plus récente de SHOT".into());
    }
    let (nv, nt) = (u(12) as usize, u(16) as usize);
    if bytes.len() != 20 + nv * 12 + nt * 12 {
        return Err("modèle 3D tronqué".into());
    }
    let f = |i: usize| f32::from_le_bytes(bytes[i..i + 4].try_into().expect("4 octets"));
    let positions = (0..nv).map(|k| [0, 1, 2].map(|c| f(20 + k * 12 + c * 4))).collect();
    let base = 20 + nv * 12;
    let triangles: Vec<[u32; 3]> = (0..nt).map(|k| [0, 1, 2].map(|c| u(base + k * 12 + c * 4))).collect();
    if triangles.iter().flatten().any(|&i| i as usize >= nv) {
        return Err("modèle 3D abîmé".into());
    }
    Ok(Mesh3d { positions, triangles })
}
