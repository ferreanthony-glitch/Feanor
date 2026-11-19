//! Le décor 3D d'une séquence (partie 180) : lecture, mesure et écriture des fichiers glTF 2.0 (`.glb` binaire, `.gltf` texte).
//!
//! Lecteur et écrivain faits main, sans bibliothèque (rien de neuf à télécharger) : c'est le lecteur de l'ancien repère 3D
//! (`model3d.rs`, retiré le 2026-10-21), repris et étendu à ce que le rendu du décor montre — positions, normales (des normales
//! **plates** quand le fichier n'en a pas, comme le veut la norme glTF), coordonnées de texture `TEXCOORD_0`, couleur de base de
//! la matière (`pbrMetallicRoughness.baseColorFactor`) et sa texture (`baseColorTexture`, image PNG ou JPEG rangée dans le
//! fichier ou intégrée en `data:`), mode d'alpha (`OPAQUE`, `MASK` et son seuil ; `BLEND` est lu comme opaque). Les nœuds de la
//! scène sont parcourus avec leurs transformations (TRS ou matrice), cuites dans les sommets.
//!
//! Refusés avec un message en français (« réexporte sans… ») : compression Draco ou quantifiée, accesseurs « clairsemés », et
//! toute extension que le fichier **exige** (`extensionsRequired`) : FEANOR ne saurait pas la lire et montrerait autre chose
//! que le modèle.
//!
//! **Le fichier gardé dans le projet est toujours un `.glb` autonome** (`decors/<id>.glb`) : un `.gltf` texte, avec ses tampons
//! et ses images à côté ou en `data:`, est **empaqueté** à l'import (`empaqueter`) — tout dans un seul fichier binaire, sans rien
//! décoder ni changer. Le projet ne dépend donc jamais du dossier d'où le modèle venait.
//!
//! **Mesurer avant d'accepter** (`Mesures`) : taille du fichier, triangles, sommets, textures et leur poids en mémoire une fois
//! décodées, plus grand côté, matières, et ce que le rendu de FEANOR n'utilise pas (cartes de normales, de métal-rugosité,
//! d'occlusion, d'émission, jeux de coordonnées en plus, tangentes, couleurs de sommets, animations, peaux, cibles de morphose).
//! Les budgets sont dans `decor3d::BUDGET`.
//!
//! **Alléger** (`ecrire_glb`) : un `.glb` neuf écrit depuis ce que FEANOR montre (`Scene3d`) — mêmes triangles, mêmes positions,
//! mêmes coordonnées de texture, mêmes couleurs de base ; les textures réduites à un côté maximal (JPEG pour une image opaque,
//! PNG quand l'alpha sert) ou gardées telles quelles ; sommets identiques soudés, indices sur 16 bits quand ils tiennent. Le
//! fichier d'origine, sur le disque, n'est jamais touché.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use serde_json::{json, Value};

/// Une texture de couleur de base, décodée (RGBA non prémultiplié, en sRGB comme dans le fichier).
#[derive(Clone)]
pub struct Texture3d {
    pub largeur: u32,
    pub hauteur: u32,
    pub rgba: Arc<Vec<u8>>,
    /// Un pixel au moins n'est pas opaque.
    pub alpha: bool,
    /// L'image telle qu'elle était dans le fichier (PNG ou JPEG), et sa sorte : ce que l'écriture recopie quand la texture
    /// n'est pas réduite (aucune perte d'un nouvel encodage).
    pub source: Arc<Vec<u8>>,
    pub mime: &'static str,
}

impl std::fmt::Debug for Texture3d {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Texture3d({}×{}, {})", self.largeur, self.hauteur, self.mime)
    }
}

/// Un morceau du décor d'une seule matière : ses triangles, dans le repère du décor (les transformations des nœuds y sont
/// cuites).
#[derive(Clone, Debug, Default)]
pub struct Primitive3d {
    pub positions: Vec<[f32; 3]>,
    /// Une normale par sommet, de longueur 1.
    pub normales: Vec<[f32; 3]>,
    /// Coordonnées de texture (`TEXCOORD_0`) ; vide : aucune.
    pub uvs: Vec<[f32; 2]>,
    /// Trois par triangle.
    pub indices: Vec<u32>,
    /// Couleur de base de la matière (linéaire, avec son alpha), `baseColorFactor`.
    pub couleur: [f32; 4],
    /// Texture de couleur de base (rang dans `Scene3d::textures`).
    pub texture: Option<usize>,
    /// Mode d'alpha `MASK` : un point dont l'alpha est sous ce seuil n'est pas dessiné. Rien : opaque.
    pub seuil_alpha: Option<f32>,
}

impl Primitive3d {
    pub fn triangles(&self) -> usize {
        self.indices.len() / 3
    }
}

/// Ce que FEANOR montre d'un fichier glTF.
#[derive(Clone, Debug, Default)]
pub struct Scene3d {
    pub primitives: Vec<Primitive3d>,
    pub textures: Vec<Texture3d>,
}

impl Scene3d {
    pub fn triangles(&self) -> usize {
        self.primitives.iter().map(Primitive3d::triangles).sum()
    }

    pub fn sommets(&self) -> usize {
        self.primitives.iter().map(|p| p.positions.len()).sum()
    }

    /// Boîte englobante (min, max), en mètres, dans le repère du décor.
    pub fn bornes(&self) -> ([f32; 3], [f32; 3]) {
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for p in self.primitives.iter().flat_map(|p| &p.positions) {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        if lo[0] > hi[0] {
            ([0.0; 3], [0.0; 3])
        } else {
            (lo, hi)
        }
    }

    /// Octets des textures décodées gardées en mémoire (largeur × hauteur × 4).
    pub fn octets_textures(&self) -> u64 {
        self.textures.iter().map(|t| t.largeur as u64 * t.hauteur as u64 * 4).sum()
    }
}

/// Ce qu'on mesure d'un fichier avant de l'accepter (`decor3d::BUDGET`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesures {
    /// Taille du fichier (ou des fichiers, pour un `.gltf` et ses annexes), en octets.
    pub octets_fichier: u64,
    pub triangles: usize,
    pub sommets: usize,
    pub matieres: usize,
    /// Les textures de couleur de base (celles que FEANOR montre) : nombre, poids décodées, plus grand côté.
    pub textures: usize,
    pub octets_textures: u64,
    pub plus_grand_cote: u32,
    /// Ce que le rendu de FEANOR n'utilise pas (le « compromis sur les shaders ») : images qui ne servent qu'aux cartes de
    /// normales, de métal-rugosité, d'occlusion ou d'émission, et leur poids dans le fichier.
    pub cartes_inutiles: usize,
    pub octets_cartes_inutiles: u64,
    /// Primitives qui portent des données que FEANOR ignore : jeux de coordonnées après le premier, tangentes, couleurs de
    /// sommets.
    pub uv_en_plus: usize,
    pub tangentes: usize,
    pub couleurs_sommets: usize,
    /// Animations, peaux (squelettes) et cibles de morphose : rien de cela ne sert au décor (étape 1) ; elles compteront pour les
    /// personnages (étape 2), et le fichier d'origine les garde.
    pub animations: usize,
    pub peaux: usize,
    pub morphoses: usize,
}

// ------------------------------------------------------------------------------------------------- lecture

/// Lit un fichier glTF : ce que FEANOR montre, et ses mesures. `dossier` : celui du fichier, pour les tampons et les images
/// à côté d'un `.gltf` (rien : refusés).
pub fn lire(octets: &[u8], dossier: Option<&Path>) -> Result<(Scene3d, Mesures), String> {
    let (json, bin) = decouper(octets)?;
    verifier(&json)?;
    let buffers = charger_tampons(&json, bin, dossier)?;
    let mut mesures = Mesures { octets_fichier: octets.len() as u64 + buffers.iter().skip(1).map(|b| b.len() as u64).sum::<u64>(), ..Mesures::default() };
    let images = |i: usize| octets_image(&json, &buffers, i, dossier);

    // Les textures de couleur de base, décodées une fois par image (taille lue dans l'en-tête et bornée **avant** de décoder).
    let mut octets_decodes: u64 = 0;
    let mut textures: Vec<Texture3d> = Vec::new();
    let mut par_image: HashMap<usize, Option<usize>> = HashMap::new();
    let mut texture_de = |tex: usize| -> Result<Option<usize>, String> {
        let Some(img) = json["textures"][tex]["source"].as_u64().map(|s| s as usize) else { return Ok(None) };
        if let Some(t) = par_image.get(&img) {
            return Ok(*t);
        }
        let t = match images(img)? {
            Some((source, mime)) => {
                let decodee = decoder_texture(&source, img, &mut octets_decodes)?;
                let (largeur, hauteur) = decodee.dimensions();
                let rgba = decodee.into_raw();
                let alpha = rgba.chunks_exact(4).any(|p| p[3] < 255);
                textures.push(Texture3d { largeur, hauteur, rgba: Arc::new(rgba), alpha, source: Arc::new(source), mime });
                Some(textures.len() - 1)
            }
            None => None,
        };
        par_image.insert(img, t);
        Ok(t)
    };

    let mut scene = Scene3d::default();
    // Nœuds de la scène par défaut (ou de la première), avec leurs transformations.
    let racines = racines(&json);
    let mut pile: Vec<(usize, [f32; 16], usize)> = racines.into_iter().map(|n| (n, IDENTITE, 0)).collect();
    while let Some((n, parent, profondeur)) = pile.pop() {
        if profondeur > 64 {
            return Err("hiérarchie de nœuds trop profonde (boucle ?)".into());
        }
        let node = &json["nodes"][n];
        if node.is_null() {
            continue;
        }
        let monde = mul(&parent, &matrice_du_noeud(node));
        if let Some(m) = node["mesh"].as_u64() {
            ajouter_maillage(&json, &buffers, m as usize, &monde, &mut scene, &mut texture_de, &mut mesures)?;
        }
        for c in node["children"].as_array().into_iter().flatten().filter_map(|c| c.as_u64()) {
            pile.push((c as usize, monde, profondeur + 1));
        }
    }
    if scene.triangles() == 0 {
        return Err("aucun triangle dans ce fichier".into());
    }
    scene.textures = textures;
    // Les mesures.
    mesures.triangles = scene.triangles();
    mesures.sommets = scene.sommets();
    mesures.matieres = json["materials"].as_array().map_or(0, |a| a.len());
    mesures.textures = scene.textures.len();
    mesures.octets_textures = scene.octets_textures();
    mesures.plus_grand_cote = scene.textures.iter().map(|t| t.largeur.max(t.hauteur)).max().unwrap_or(0);
    mesures.animations = json["animations"].as_array().map_or(0, |a| a.len());
    mesures.peaux = json["skins"].as_array().map_or(0, |a| a.len());
    // Les images qui ne servent qu'aux cartes que FEANOR n'utilise pas.
    let couleur: std::collections::BTreeSet<usize> = par_image.keys().copied().collect();
    let mut autres = std::collections::BTreeSet::new();
    for m in json["materials"].as_array().into_iter().flatten() {
        let refs = [&m["normalTexture"], &m["occlusionTexture"], &m["emissiveTexture"], &m["pbrMetallicRoughness"]["metallicRoughnessTexture"]];
        for r in refs {
            if let Some(img) = r["index"].as_u64().and_then(|t| json["textures"][t as usize]["source"].as_u64()) {
                if !couleur.contains(&(img as usize)) {
                    autres.insert(img as usize);
                }
            }
        }
    }
    mesures.cartes_inutiles = autres.len();
    mesures.octets_cartes_inutiles = autres.iter().map(|&i| poids_image(&json, &buffers, i)).sum();
    Ok((scene, mesures))
}

/// L'en-tête d'un `.glb` (sa description JSON et son tampon binaire), ou le texte d'un `.gltf`.
/// Décode une texture, après avoir lu sa taille dans l'en-tête : un côté au-delà de `PLAFONDS.cote_texture`, ou un total
/// décodé au-delà de `PLAFONDS.octets_textures`, est refusé sans rien décoder (une image de 100 000 × 100 000 pixels annoncée
/// par un fichier abîmé ne réserve pas 40 Go).
fn decoder_texture(source: &[u8], img: usize, total: &mut u64) -> Result<image::RgbaImage, String> {
    let lecteur = image::ImageReader::new(std::io::Cursor::new(source)).with_guessed_format().map_err(|e| format!("texture {img} illisible : {e}"))?;
    let (w, h) = lecteur.into_dimensions().map_err(|e| format!("texture {img} illisible : {e}"))?;
    let cap = crate::decor3d::PLAFONDS;
    if w.max(h) > cap.cote_texture || w == 0 || h == 0 {
        return Err(format!("texture {img} de {w} × {h} pixels : FEANOR n'accepte pas plus de {} pixels de côté", cap.cote_texture));
    }
    *total += w as u64 * h as u64 * 4;
    if *total > cap.octets_textures {
        return Err(format!("textures trop lourdes une fois décodées (plus de {} Mo) : réduis-les dans un outil 3D", cap.octets_textures >> 20));
    }
    let mut lecteur = image::ImageReader::new(std::io::Cursor::new(source)).with_guessed_format().map_err(|e| format!("texture {img} illisible : {e}"))?;
    let mut limites = image::Limits::default();
    limites.max_image_width = Some(cap.cote_texture);
    limites.max_image_height = Some(cap.cote_texture);
    limites.max_alloc = Some(w as u64 * h as u64 * 8 + (64 << 20));
    lecteur.limits(limites);
    Ok(lecteur.decode().map_err(|e| format!("texture {img} illisible : {e}"))?.to_rgba8())
}

fn decouper(octets: &[u8]) -> Result<(Value, Option<Vec<u8>>), String> {
    if octets.starts_with(b"glTF") {
        return lire_glb(octets);
    }
    let texte = std::str::from_utf8(octets).map_err(|_| "fichier glTF illisible (ni .glb ni texte)")?;
    let json: Value = serde_json::from_str(texte.trim_start_matches('\u{feff}')).map_err(|e| format!("glTF illisible : {e}"))?;
    Ok((json, None))
}

fn verifier(json: &Value) -> Result<(), String> {
    let version = json["asset"]["version"].as_str().unwrap_or("");
    if !version.starts_with('2') {
        return Err(format!("glTF version « {version} » : seule la version 2 est lue"));
    }
    // Les extensions exigées que FEANOR peut accepter sans montrer autre chose que le modèle : `KHR_materials_unlit` (une matière
    // « sans éclairage » : le décor l'éclaire quand même, c'est l'aspect choisi ; les modèles low poly en sont pleins) et
    // `KHR_materials_emissive_strength` (l'émission, que FEANOR ne montre pas).
    const ACCEPTEES: [&str; 2] = ["KHR_materials_unlit", "KHR_materials_emissive_strength"];
    if let Some(nom) = json["extensionsRequired"].as_array().into_iter().flatten().filter_map(|e| e.as_str()).find(|n| !ACCEPTEES.contains(n)) {
        let compression = nom.contains("draco") || nom.contains("meshopt") || nom.contains("quantization");
        return Err(if compression {
            format!("le fichier exige l'extension glTF « {nom} » (compression), que FEANOR ne lit pas : réexporte-le sans compression")
        } else {
            format!("le fichier exige l'extension glTF « {nom} », que FEANOR ne lit pas : réexporte-le sans elle")
        });
    }
    Ok(())
}

fn racines(json: &Value) -> Vec<usize> {
    let scene = json["scene"].as_u64().unwrap_or(0) as usize;
    match json["scenes"].get(scene) {
        Some(s) => s["nodes"].as_array().into_iter().flatten().filter_map(|n| n.as_u64()).map(|n| n as usize).collect(),
        // Pas de scène : tous les nœuds qui ne sont l'enfant d'aucun autre.
        None => {
            let nodes = json["nodes"].as_array().map_or(0, |a| a.len());
            let enfants: std::collections::BTreeSet<usize> =
                json["nodes"].as_array().into_iter().flatten().flat_map(|n| n["children"].as_array().into_iter().flatten().filter_map(|c| c.as_u64())).map(|c| c as usize).collect();
            (0..nodes).filter(|i| !enfants.contains(i)).collect()
        }
    }
}

fn lire_glb(octets: &[u8]) -> Result<(Value, Option<Vec<u8>>), String> {
    let u32_a = |i: usize| octets.get(i..i + 4).map(|b| u32::from_le_bytes(b.try_into().expect("4 octets")) as usize);
    let total = u32_a(8).ok_or("fichier .glb tronqué")?.min(octets.len());
    let mut i = 12;
    let (mut json, mut bin) = (None, None);
    while i + 8 <= total {
        let len = u32_a(i).ok_or("fichier .glb tronqué")?;
        let sorte = &octets[i + 4..i + 8];
        let data = octets.get(i + 8..i + 8 + len).ok_or("fichier .glb tronqué")?;
        match sorte {
            b"JSON" => json = Some(serde_json::from_slice::<Value>(data).map_err(|e| format!("glTF illisible : {e}"))?),
            b"BIN\0" if bin.is_none() => bin = Some(data.to_vec()),
            _ => {}
        }
        i += 8 + len.div_ceil(4) * 4;
    }
    Ok((json.ok_or("fichier .glb sans description")?, bin))
}

/// Un nom de fichier d'à côté, sans sortir du dossier du fichier glTF.
fn fichier_voisin(dossier: Option<&Path>, uri: &str) -> Result<Vec<u8>, String> {
    let dossier = dossier.ok_or("tampon externe : fichier introuvable")?;
    let nom = percent_decode(uri);
    if Path::new(&nom).components().any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::RootDir | std::path::Component::Prefix(_))) {
        return Err(format!("fichier « {nom} » hors du dossier du modèle"));
    }
    std::fs::read(dossier.join(&nom)).map_err(|e| format!("fichier « {nom} » : {e}"))
}

fn charger_tampons(json: &Value, mut bin: Option<Vec<u8>>, dossier: Option<&Path>) -> Result<Vec<Vec<u8>>, String> {
    let mut out = Vec::new();
    for (i, b) in json["buffers"].as_array().into_iter().flatten().enumerate() {
        let data = match b["uri"].as_str() {
            None if i == 0 => bin.take().ok_or("tampon binaire manquant dans le .glb")?,
            None => return Err("tampon sans données".into()),
            Some(uri) if uri.starts_with("data:") => {
                let b64 = uri.split_once(";base64,").map(|x| x.1).ok_or("tampon intégré sans base64")?;
                base64_decode(b64)?
            }
            Some(uri) => fichier_voisin(dossier, uri)?,
        };
        out.push(data);
    }
    Ok(out)
}

/// Les octets d'une vue de tampon.
fn vue<'a>(json: &Value, buffers: &'a [Vec<u8>], v: usize) -> Result<&'a [u8], String> {
    let view = &json["bufferViews"][v];
    let buffer = buffers.get(view["buffer"].as_u64().unwrap_or(0) as usize).ok_or("tampon manquant")?;
    let debut = view["byteOffset"].as_u64().unwrap_or(0) as usize;
    let len = view["byteLength"].as_u64().unwrap_or(0) as usize;
    buffer.get(debut..debut + len).ok_or_else(|| "vue hors de son tampon".to_string())
}

/// Les octets encodés de l'image `i` et sa sorte (PNG ou JPEG) ; rien : une sorte que FEANOR ne lit pas.
fn octets_image(json: &Value, buffers: &[Vec<u8>], i: usize, dossier: Option<&Path>) -> Result<Option<(Vec<u8>, &'static str)>, String> {
    let img = &json["images"][i];
    let octets = if let Some(v) = img["bufferView"].as_u64() {
        vue(json, buffers, v as usize)?.to_vec()
    } else if let Some(uri) = img["uri"].as_str() {
        if uri.starts_with("data:") {
            base64_decode(uri.split_once(";base64,").map(|x| x.1).ok_or("image intégrée sans base64")?)?
        } else {
            fichier_voisin(dossier, uri)?
        }
    } else {
        return Ok(None);
    };
    let mime = if octets.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if octets.starts_with(&[0xFF, 0xD8]) {
        "image/jpeg"
    } else {
        return Ok(None);
    };
    Ok(Some((octets, mime)))
}

/// Le poids d'une image dans le fichier (0 si elle n'est pas lisible).
fn poids_image(json: &Value, buffers: &[Vec<u8>], i: usize) -> u64 {
    let img = &json["images"][i];
    if let Some(v) = img["bufferView"].as_u64() {
        return json["bufferViews"][v as usize]["byteLength"].as_u64().unwrap_or(0);
    }
    let _ = buffers;
    img["uri"].as_str().map_or(0, |u| if u.starts_with("data:") { u.len() as u64 * 3 / 4 } else { 0 })
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

/// Les valeurs d'un accesseur, en flottants (les entiers normalisés ramenés entre 0 et 1, ou −1 et 1), `n` par élément.
fn accesseur(json: &Value, buffers: &[Vec<u8>], index: usize, n: usize, flottant_seul: Option<&str>) -> Result<Vec<f32>, String> {
    let acc = &json["accessors"][index];
    if acc.is_null() {
        return Err(format!("accesseur {index} manquant"));
    }
    if !acc["sparse"].is_null() {
        return Err("accesseur « clairsemé » non lu : réexporte le modèle sans cette option".into());
    }
    let composante = acc["componentType"].as_u64().unwrap_or(0);
    if let Some(quoi) = flottant_seul {
        if composante != 5126 {
            return Err(format!("{quoi} non flottantes (fichier quantifié ?) : réexporte le modèle sans compression"));
        }
    }
    let taille = match composante {
        5120 | 5121 => 1,
        5122 | 5123 => 2,
        5125 | 5126 => 4,
        _ => return Err("accesseur d'un type inconnu".into()),
    };
    let elem = taille * n;
    let count = acc["count"].as_u64().unwrap_or(0);
    // (Un fichier qui ment sur ses nombres : tout est vérifié contre la longueur réelle du tampon **avant** de réserver la
    // mémoire, en arithmétique vérifiée ; rien ne déborde, rien ne s'alloue au-delà de ce que le fichier contient.)
    if count > crate::decor3d::PLAFONDS.sommets as u64 {
        return Err(format!("{count} éléments dans un accesseur : au-delà de ce que FEANOR accepte ({})", crate::decor3d::PLAFONDS.sommets));
    }
    let count = count as usize;
    let Some(v) = acc["bufferView"].as_u64() else { return Err("accesseur sans données".into()) };
    let view = &json["bufferViews"][v as usize];
    let buffer = buffers.get(view["buffer"].as_u64().unwrap_or(0) as usize).ok_or("tampon manquant")?;
    let vue_debut = view["byteOffset"].as_u64().unwrap_or(0);
    let vue_len = view["byteLength"].as_u64().unwrap_or(u64::MAX);
    let debut = vue_debut.checked_add(acc["byteOffset"].as_u64().unwrap_or(0)).ok_or("accesseur hors de son tampon")?;
    let pas = view["byteStride"].as_u64().map_or(elem as u64, |s| s).max(elem as u64);
    let etendue = if count == 0 { Some(0) } else { pas.checked_mul(count as u64 - 1).and_then(|x| x.checked_add(elem as u64)) };
    let fin = etendue.and_then(|e| debut.checked_add(e)).ok_or("accesseur hors de son tampon")?;
    if fin > buffer.len() as u64 || fin > vue_debut.saturating_add(vue_len) || pas > 255 && view["byteStride"].is_u64() {
        return Err("accesseur hors de son tampon (fichier abîmé ?)".into());
    }
    let (debut, pas) = (debut as usize, pas as usize);
    let normalise = acc["normalized"].as_bool().unwrap_or(false);
    let mut out = Vec::with_capacity(count * n);
    for i in 0..count {
        for k in 0..n {
            let o = debut + i * pas + k * taille;
            let b = &buffer[o..o + taille];
            let v = match composante {
                5126 => f32::from_le_bytes(b.try_into().expect("4 octets")),
                5125 => u32::from_le_bytes(b.try_into().expect("4 octets")) as f32,
                5121 => b[0] as f32 / if normalise { 255.0 } else { 1.0 },
                5120 => (b[0] as i8 as f32 / if normalise { 127.0 } else { 1.0 }).max(-1.0),
                5123 => u16::from_le_bytes([b[0], b[1]]) as f32 / if normalise { 65535.0 } else { 1.0 },
                _ => (i16::from_le_bytes([b[0], b[1]]) as f32 / if normalise { 32767.0 } else { 1.0 }).max(-1.0),
            };
            out.push(v);
        }
    }
    Ok(out)
}

fn lire_indices(json: &Value, buffers: &[Vec<u8>], index: usize) -> Result<Vec<u32>, String> {
    let acc = &json["accessors"][index];
    let taille = match acc["componentType"].as_u64() {
        Some(5121) => 1,
        Some(5123) => 2,
        Some(5125) => 4,
        _ => return Err("indices d'un type inconnu".into()),
    };
    if !acc["sparse"].is_null() {
        return Err("accesseur « clairsemé » non lu : réexporte le modèle sans cette option".into());
    }
    let count = acc["count"].as_u64().unwrap_or(0);
    if count > crate::decor3d::PLAFONDS.indices as u64 {
        return Err(format!("{count} indices dans un accesseur : au-delà de ce que FEANOR accepte ({})", crate::decor3d::PLAFONDS.indices));
    }
    let count = count as usize;
    let Some(v) = acc["bufferView"].as_u64() else { return Err("indices sans données".into()) };
    let view = &json["bufferViews"][v as usize];
    let buffer = buffers.get(view["buffer"].as_u64().unwrap_or(0) as usize).ok_or("tampon manquant")?;
    let debut = view["byteOffset"].as_u64().unwrap_or(0).checked_add(acc["byteOffset"].as_u64().unwrap_or(0)).ok_or("indices hors de leur tampon")?;
    let fin = (count as u64).checked_mul(taille as u64).and_then(|x| x.checked_add(debut)).ok_or("indices hors de leur tampon")?;
    if fin > buffer.len() as u64 {
        return Err("indices hors de leur tampon (fichier abîmé ?)".into());
    }
    let debut = debut as usize;
    Ok((0..count)
        .map(|i| {
            let o = debut + i * taille;
            match taille {
                1 => buffer[o] as u32,
                2 => u16::from_le_bytes([buffer[o], buffer[o + 1]]) as u32,
                _ => u32::from_le_bytes(buffer[o..o + 4].try_into().expect("4 octets")),
            }
        })
        .collect())
}

#[allow(clippy::too_many_arguments)]
fn ajouter_maillage(
    json: &Value,
    buffers: &[Vec<u8>],
    m: usize,
    monde: &[f32; 16],
    scene: &mut Scene3d,
    texture_de: &mut dyn FnMut(usize) -> Result<Option<usize>, String>,
    mesures: &mut Mesures,
) -> Result<(), String> {
    let normales_monde = matrice_des_normales(monde);
    // Une matrice qui retourne (échelle négative) inverse le sens des triangles ; le rendu dessine les deux faces, mais les
    // normales plates calculées doivent rester dehors.
    let retourne = determinant3(monde) < 0.0;
    for prim in json["meshes"][m]["primitives"].as_array().into_iter().flatten() {
        let mode = prim["mode"].as_u64().unwrap_or(4);
        if !(4..=6).contains(&mode) {
            continue; // points et lignes : rien à dessiner en surface
        }
        let attrs = &prim["attributes"];
        let Some(pos) = attrs["POSITION"].as_u64() else { continue };
        let positions: Vec<[f32; 3]> = accesseur(json, buffers, pos as usize, 3, Some("positions"))?.chunks_exact(3).map(|c| transform(monde, [c[0], c[1], c[2]])).collect();
        let n = positions.len() as u32;
        let normales: Option<Vec<[f32; 3]>> = match attrs["NORMAL"].as_u64() {
            Some(i) => Some(accesseur(json, buffers, i as usize, 3, Some("normales"))?.chunks_exact(3).map(|c| normalize(transform_dir(&normales_monde, [c[0], c[1], c[2]]))).collect()),
            None => None,
        };
        let uvs: Vec<[f32; 2]> = match attrs["TEXCOORD_0"].as_u64() {
            Some(i) => accesseur(json, buffers, i as usize, 2, None)?.chunks_exact(2).map(|c| [c[0], c[1]]).collect(),
            None => Vec::new(),
        };
        if normales.as_ref().is_some_and(|v| v.len() != positions.len()) || (!uvs.is_empty() && uvs.len() != positions.len()) {
            return Err("attributs d'un maillage de longueurs différentes".into());
        }
        // Ce que FEANOR ignore, compté pour l'allègement.
        let noms: Vec<&str> = attrs.as_object().map(|o| o.keys().map(String::as_str).collect()).unwrap_or_default();
        mesures.uv_en_plus += usize::from(noms.iter().any(|k| k.starts_with("TEXCOORD_") && *k != "TEXCOORD_0"));
        mesures.tangentes += usize::from(noms.contains(&"TANGENT"));
        mesures.couleurs_sommets += usize::from(noms.iter().any(|k| k.starts_with("COLOR_")));
        mesures.morphoses += prim["targets"].as_array().map_or(0, |a| a.len());
        let indices = match prim["indices"].as_u64() {
            Some(i) => lire_indices(json, buffers, i as usize)?,
            None => (0..n).collect(),
        };
        let tris: Vec<[u32; 3]> = match mode {
            4 => indices.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect(),
            5 => indices.windows(3).enumerate().map(|(k, c)| if k % 2 == 0 { [c[0], c[1], c[2]] } else { [c[1], c[0], c[2]] }).collect(),
            _ => indices.iter().skip(1).zip(indices.iter().skip(2)).map(|(&b, &c)| [indices[0], b, c]).collect(),
        };
        let ok = |t: &[u32; 3]| t.iter().all(|&i| i < n) && t[0] != t[1] && t[1] != t[2] && t[0] != t[2];
        let tris: Vec<[u32; 3]> = tris.into_iter().filter(ok).map(|t| if retourne { [t[0], t[2], t[1]] } else { t }).collect();
        if tris.is_empty() {
            continue;
        }
        // La matière.
        let mat = prim["material"].as_u64().map(|i| &json["materials"][i as usize]);
        let pbr = mat.map(|m| &m["pbrMetallicRoughness"]);
        let couleur = pbr
            .and_then(|p| p["baseColorFactor"].as_array())
            .filter(|a| a.len() == 4)
            .map(|a| [0, 1, 2, 3].map(|k| a[k].as_f64().unwrap_or(1.0) as f32))
            .unwrap_or([1.0; 4]);
        let tex_ref = pbr.map(|p| &p["baseColorTexture"]);
        let texture = match tex_ref.and_then(|t| t["index"].as_u64()) {
            Some(t) if tex_ref.and_then(|r| r["texCoord"].as_u64()).unwrap_or(0) == 0 && !uvs.is_empty() => texture_de(t as usize)?,
            _ => None,
        };
        let seuil_alpha = match mat.and_then(|m| m["alphaMode"].as_str()) {
            Some("MASK") => Some(mat.and_then(|m| m["alphaCutoff"].as_f64()).unwrap_or(0.5) as f32),
            _ => None,
        };
        let p = match normales {
            Some(normales) => Primitive3d { positions, normales, uvs, indices: tris.into_iter().flatten().collect(), couleur, texture, seuil_alpha },
            // Sans normales : des normales plates (la norme glTF), donc des sommets propres à chaque triangle.
            None => {
                let mut q = Primitive3d { couleur, texture, seuil_alpha, ..Default::default() };
                for t in tris {
                    let [a, b, c] = t.map(|i| positions[i as usize]);
                    let nf = normale_face(a, b, c);
                    for &i in &t {
                        q.indices.push(q.positions.len() as u32);
                        q.positions.push(positions[i as usize]);
                        q.normales.push(nf);
                        if !uvs.is_empty() {
                            q.uvs.push(uvs[i as usize]);
                        }
                    }
                }
                q
            }
        };
        scene.primitives.push(p);
    }
    Ok(())
}

pub fn normale_face(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
    normalize([u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]])
}

fn normalize(n: [f32; 3]) -> [f32; 3] {
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if l < 1e-20 || !l.is_finite() {
        [0.0, 1.0, 0.0]
    } else {
        n.map(|x| x / l)
    }
}

// --------------------------------------------------------------------------------- matrices

/// Matrice 4×4 en colonnes (comme glTF).
const IDENTITE: [f32; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

fn mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    crate::espace3dmath::mul(a, b)
}

fn transform(m: &[f32; 16], p: [f32; 3]) -> [f32; 3] {
    crate::espace3dmath::point(m, p)
}

fn transform_dir(m: &[f32; 16], d: [f32; 3]) -> [f32; 3] {
    crate::espace3dmath::dir(m, d)
}

fn determinant3(m: &[f32; 16]) -> f32 {
    let (a, b, c) = ([m[0], m[1], m[2]], [m[4], m[5], m[6]], [m[8], m[9], m[10]]);
    crate::espace3dmath::dot(a, crate::espace3dmath::cross(b, c))
}

/// L'inverse transposée de la partie 3×3 (juste, même avec une échelle inégale).
fn matrice_des_normales(m: &[f32; 16]) -> [f32; 16] {
    let inv = crate::espace3dmath::inverse_affine(m);
    let mut n = IDENTITE;
    for r in 0..3 {
        for c in 0..3 {
            n[c * 4 + r] = inv[r * 4 + c];
        }
    }
    n
}

fn matrice_du_noeud(node: &Value) -> [f32; 16] {
    let nums = |v: &Value, n: usize, defaut: &[f32]| -> Vec<f32> {
        match v.as_array() {
            Some(a) if a.len() == n => a.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect(),
            _ => defaut.to_vec(),
        }
    };
    if node["matrix"].is_array() {
        let m = nums(&node["matrix"], 16, &IDENTITE);
        return m.try_into().unwrap_or(IDENTITE);
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
    #[rustfmt::skip]
    let m = [
        r[0] * s[0], r[1] * s[0], r[2] * s[0], 0.0,
        r[3] * s[1], r[4] * s[1], r[5] * s[1], 0.0,
        r[6] * s[2], r[7] * s[2], r[8] * s[2], 0.0,
        t[0], t[1], t[2], 1.0,
    ];
    m
}

// ------------------------------------------------------------------------------------------- empaqueter

/// Le fichier en un seul `.glb` autonome : un `.gltf` texte reçoit ses tampons et ses images (d'à côté ou en `data:`) dans un
/// tampon binaire unique ; un `.glb` qui ne cite rien d'extérieur est rendu tel quel. Rien n'est décodé : les octets des
/// tampons et des images sont recopiés, seuls leurs emplacements changent dans la description.
pub fn empaqueter(octets: &[u8], dossier: Option<&Path>) -> Result<Vec<u8>, String> {
    let (mut json, bin) = decouper(octets)?;
    let externe = |v: &Value| v["uri"].as_str().is_some();
    let a_citer = json["buffers"].as_array().is_some_and(|a| a.iter().any(externe)) || json["images"].as_array().is_some_and(|a| a.iter().any(externe));
    if octets.starts_with(b"glTF") && !a_citer {
        return Ok(octets.to_vec());
    }
    let buffers = charger_tampons(&json, bin, dossier)?;
    // Un seul tampon : les autres à la suite, chacun aligné sur 4 octets.
    let mut sortie: Vec<u8> = Vec::new();
    let mut debuts = Vec::with_capacity(buffers.len());
    for b in &buffers {
        aligner(&mut sortie);
        debuts.push(sortie.len());
        sortie.extend_from_slice(b);
    }
    for v in json["bufferViews"].as_array_mut().into_iter().flatten() {
        let b = v["buffer"].as_u64().unwrap_or(0) as usize;
        let o = v["byteOffset"].as_u64().unwrap_or(0) as usize + debuts.get(b).copied().unwrap_or(0);
        v["buffer"] = json!(0);
        v["byteOffset"] = json!(o);
    }
    // Les images citées par un nom ou en `data:` passent dans le tampon, chacune dans sa vue.
    let nb_images = json["images"].as_array().map_or(0, |a| a.len());
    for i in 0..nb_images {
        let Some(uri) = json["images"][i]["uri"].as_str().map(str::to_string) else { continue };
        let data = if uri.starts_with("data:") { base64_decode(uri.split_once(";base64,").map(|x| x.1).ok_or("image intégrée sans base64")?)? } else { fichier_voisin(dossier, &uri)? };
        let mime = if data.starts_with(&[0x89, b'P', b'N', b'G']) { "image/png" } else if data.starts_with(&[0xFF, 0xD8]) { "image/jpeg" } else { "application/octet-stream" };
        aligner(&mut sortie);
        let vue = json!({ "buffer": 0, "byteOffset": sortie.len(), "byteLength": data.len() });
        sortie.extend_from_slice(&data);
        let views = json["bufferViews"].as_array_mut();
        let n = match views {
            Some(a) => {
                a.push(vue);
                a.len() - 1
            }
            None => {
                json["bufferViews"] = json!([vue]);
                0
            }
        };
        let img = &mut json["images"][i];
        if let Some(o) = img.as_object_mut() {
            o.remove("uri");
        }
        img["bufferView"] = json!(n);
        img["mimeType"] = json!(mime);
    }
    json["buffers"] = json!([{ "byteLength": sortie.len() }]);
    ecrire_conteneur(&json, &sortie)
}

fn aligner(v: &mut Vec<u8>) {
    while v.len() % 4 != 0 {
        v.push(0);
    }
}

/// Le conteneur `.glb` : en-tête, morceau JSON (complété d'espaces), morceau binaire (complété de zéros).
fn ecrire_conteneur(json: &Value, bin: &[u8]) -> Result<Vec<u8>, String> {
    let mut texte = serde_json::to_vec(json).map_err(|e| e.to_string())?;
    while texte.len() % 4 != 0 {
        texte.push(b' ');
    }
    let mut bin = bin.to_vec();
    aligner(&mut bin);
    let total = 12 + 8 + texte.len() + if bin.is_empty() { 0 } else { 8 + bin.len() };
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(texte.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&texte);
    if !bin.is_empty() {
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------- écrire

/// Comment écrire le `.glb` allégé.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ecriture {
    /// Côté maximal des textures ; rien : chaque texture garde sa taille et ses octets d'origine.
    pub cote_max: Option<u32>,
}

/// Une texture réduite à `cote` au plus (proportions gardées) ; elle-même si elle tient.
pub fn reduire(t: &Texture3d, cote: u32) -> Texture3d {
    let grand = t.largeur.max(t.hauteur);
    if grand <= cote || cote == 0 {
        return t.clone();
    }
    let k = cote as f32 / grand as f32;
    let (w, h) = (((t.largeur as f32 * k).round() as u32).max(1), ((t.hauteur as f32 * k).round() as u32).max(1));
    let Some(img) = image::RgbaImage::from_raw(t.largeur, t.hauteur, t.rgba.to_vec()) else { return t.clone() };
    let petite = image::imageops::resize(&img, w, h, image::imageops::FilterType::Triangle);
    let mut source = Vec::new();
    let mime = if t.alpha {
        let _ = image::codecs::png::PngEncoder::new(&mut source).write_image(petite.as_raw(), w, h, image::ExtendedColorType::Rgba8);
        "image/png"
    } else {
        let rgb = image::DynamicImage::ImageRgba8(petite.clone()).to_rgb8();
        let _ = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut source, 90).encode(rgb.as_raw(), w, h, image::ExtendedColorType::Rgb8);
        "image/jpeg"
    };
    Texture3d { largeur: w, hauteur: h, rgba: Arc::new(petite.into_raw()), alpha: t.alpha, source: Arc::new(source), mime }
}

use image::ImageEncoder;

/// Soude les sommets identiques d'une primitive (mêmes position, normale et coordonnées, au bit près) : rien ne change à
/// l'image, le fichier maigrit.
pub fn souder(p: &Primitive3d) -> Primitive3d {
    let mut vus: HashMap<[u32; 8], u32> = HashMap::with_capacity(p.positions.len());
    let mut q = Primitive3d { couleur: p.couleur, texture: p.texture, seuil_alpha: p.seuil_alpha, ..Default::default() };
    let mut remap = Vec::with_capacity(p.positions.len());
    for i in 0..p.positions.len() {
        let uv = p.uvs.get(i).copied().unwrap_or([0.0; 2]);
        let (a, n) = (p.positions[i], p.normales[i]);
        let cle = [a[0], a[1], a[2], n[0], n[1], n[2], uv[0], uv[1]].map(f32::to_bits);
        let j = *vus.entry(cle).or_insert_with(|| {
            q.positions.push(a);
            q.normales.push(n);
            if !p.uvs.is_empty() {
                q.uvs.push(uv);
            }
            (q.positions.len() - 1) as u32
        });
        remap.push(j);
    }
    q.indices = p.indices.iter().map(|&i| remap[i as usize]).collect();
    q
}

/// Écrit un `.glb` neuf de ce que FEANOR montre : un nœud, un maillage d'une primitive par matière, les matières (couleur de
/// base, texture, mode d'alpha), les textures (réduites selon `e`), un échantillonneur qui répète. Les sommets identiques sont
/// soudés, les indices sur 16 bits quand ils tiennent. Ce que le rendu n'utilise pas n'y est pas.
pub fn ecrire_glb(scene: &Scene3d, e: Ecriture) -> Result<Vec<u8>, String> {
    let mut bin: Vec<u8> = Vec::new();
    let mut vues: Vec<Value> = Vec::new();
    let mut accs: Vec<Value> = Vec::new();
    let mut pousser = |bin: &mut Vec<u8>, data: &[u8], cible: Option<u32>| -> usize {
        aligner(bin);
        let mut v = json!({ "buffer": 0, "byteOffset": bin.len(), "byteLength": data.len() });
        if let Some(c) = cible {
            v["target"] = json!(c);
        }
        bin.extend_from_slice(data);
        vues.push(v);
        vues.len() - 1
    };
    // Les textures utilisées, dans l'ordre.
    let mut images = Vec::new();
    let mut rang_texture: HashMap<usize, usize> = HashMap::new();
    for p in &scene.primitives {
        if let Some(t) = p.texture {
            if !rang_texture.contains_key(&t) {
                let tex = scene.textures.get(t).ok_or("texture manquante")?;
                let tex = match e.cote_max {
                    Some(c) => reduire(tex, c),
                    None => tex.clone(),
                };
                let v = pousser(&mut bin, &tex.source, None);
                images.push(json!({ "bufferView": v, "mimeType": tex.mime }));
                rang_texture.insert(t, images.len() - 1);
            }
        }
    }
    let mut matieres = Vec::new();
    let mut prims = Vec::new();
    for p in &scene.primitives {
        let p = souder(p);
        let (lo, hi) = Scene3d { primitives: vec![p.clone()], textures: Vec::new() }.bornes();
        let f32s = |v: &[[f32; 3]]| v.iter().flatten().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
        let vp = pousser(&mut bin, &f32s(&p.positions), Some(34962));
        accs.push(json!({ "bufferView": vp, "componentType": 5126, "count": p.positions.len(), "type": "VEC3", "min": lo, "max": hi }));
        let ap = accs.len() - 1;
        let vn = pousser(&mut bin, &f32s(&p.normales), Some(34962));
        accs.push(json!({ "bufferView": vn, "componentType": 5126, "count": p.normales.len(), "type": "VEC3" }));
        let an = accs.len() - 1;
        let mut attributs = json!({ "POSITION": ap, "NORMAL": an });
        if !p.uvs.is_empty() {
            let data: Vec<u8> = p.uvs.iter().flatten().flat_map(|x| x.to_le_bytes()).collect();
            let vu = pousser(&mut bin, &data, Some(34962));
            accs.push(json!({ "bufferView": vu, "componentType": 5126, "count": p.uvs.len(), "type": "VEC2" }));
            attributs["TEXCOORD_0"] = json!(accs.len() - 1);
        }
        let court = p.positions.len() <= u16::MAX as usize;
        let data: Vec<u8> = if court { p.indices.iter().flat_map(|&i| (i as u16).to_le_bytes()).collect() } else { p.indices.iter().flat_map(|i| i.to_le_bytes()).collect() };
        let vi = pousser(&mut bin, &data, Some(34963));
        accs.push(json!({ "bufferView": vi, "componentType": if court { 5123 } else { 5125 }, "count": p.indices.len(), "type": "SCALAR" }));
        let ai = accs.len() - 1;
        let mut pbr = json!({ "baseColorFactor": p.couleur, "metallicFactor": 0.0, "roughnessFactor": 1.0 });
        if let Some(t) = p.texture.and_then(|t| rang_texture.get(&t)) {
            pbr["baseColorTexture"] = json!({ "index": t });
        }
        let mut mat = json!({ "pbrMetallicRoughness": pbr, "doubleSided": true });
        if let Some(s) = p.seuil_alpha {
            mat["alphaMode"] = json!("MASK");
            mat["alphaCutoff"] = json!(s);
        }
        matieres.push(mat);
        prims.push(json!({ "attributes": attributs, "indices": ai, "material": matieres.len() - 1, "mode": 4 }));
    }
    let mut j = json!({
        "asset": { "version": "2.0", "generator": "FEANOR (décor 3D allégé)" },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "mesh": 0, "name": "décor" }],
        "meshes": [{ "primitives": prims }],
        "materials": matieres,
        "accessors": accs,
        "bufferViews": vues,
        "buffers": [{ "byteLength": bin.len() }],
    });
    if !images.is_empty() {
        let n = images.len();
        j["images"] = json!(images);
        j["samplers"] = json!([{ "wrapS": 10497, "wrapT": 10497 }]);
        j["textures"] = json!((0..n).map(|i| json!({ "source": i, "sampler": 0 })).collect::<Vec<_>>());
    }
    ecrire_conteneur(&j, &bin)
}
