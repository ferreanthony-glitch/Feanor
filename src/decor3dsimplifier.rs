//! Simplifier la géométrie d'un décor 3D trop lourd (partie 180, « il peut proposer une simplification aussi », Anthony).
//!
//! **Cela change la forme** : c'est dit dans la fenêtre « Ce décor est lourd », et seulement proposé quand le décor dépasse le
//! budget de cet ordinateur (`decor3dbudget.rs`). Le fichier d'origine n'est jamais touché : seule la copie rangée dans le projet
//! (`decors/<id>.glb`) est simplifiée, et `decors3d.json` le note (« simplifié : 6,2 M → 1,9 M triangles »).
//!
//! **La méthode** : écrite ici, sans bibliothèque — la réduction par **contraction d'arêtes guidée par les quadriques d'erreur**
//! (Garland et Heckbert, 1997), dans sa forme « demi-arête » : une arête se contracte sur l'un de ses deux sommets (jamais
//! sur un point nouveau). C'est un peu moins fin que la position optimale, mais chaque sommet gardé est un sommet du modèle, avec
//! ses vraies coordonnées de texture : rien ne s'invente sur la texture.
//!
//! - Chaque sommet porte la somme des quadriques des plans de ses faces (pondérées par leur aire) ; contracter `a` sur `b` coûte
//!   l'erreur de `Qa + Qb` en `b`. Les contractions les moins chères passent d'abord (un tas, mis à jour paresseusement).
//! - **Contraintes dures** : on travaille primitive par primitive (une primitive, une matière : les frontières de matière ne se
//!   traversent jamais) ; les sommets sont soudés par position **et** coordonnées de texture, donc une couture de texture est un
//!   bord ; **tout sommet de bord** (bord du maillage, couture de texture) est **gardé**, et ne bouge pas.
//! - Une contraction est refusée si elle retourne une face (ou en fait une lame), ou si elle rendrait le maillage non variété
//!   (les deux sommets ont d'autres voisins communs que ceux de l'arête).
//! - Les normales sont **refaites** ensuite : lissées entre faces voisines à moins de 50°, cassées au-delà (un mur reste net).
//!
//! Limites : la cible est approchée, pas toujours atteinte (un maillage fait surtout de bords, comme une feuille de triangles
//! détachés, se simplifie peu) ; les primitives sont réduites dans la même proportion. Elle tourne sur un autre fil, avec son
//! avancement, et s'annule (`Suivi`).

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering as Mem};

use crate::decor3dgltf::{normale_face, souder, Primitive3d, Scene3d};

/// L'avancement d'une simplification, lu par la fenêtre ; `annule` l'arrête.
#[derive(Default)]
pub struct Suivi {
    pub fait: AtomicU64,
    pub total: AtomicU64,
    pub annule: AtomicBool,
}

impl Suivi {
    pub fn fraction(&self) -> f32 {
        let t = self.total.load(Mem::Relaxed).max(1);
        (self.fait.load(Mem::Relaxed) as f32 / t as f32).clamp(0.0, 1.0)
    }
}

/// Angle au-delà duquel deux faces voisines gardent une arête vive (normales cassées).
pub const ANGLE_VIF: f32 = 50.0;

/// La scène simplifiée à environ `cible` triangles en tout (chaque primitive dans la même proportion). `Err` : annulée.
pub fn simplifier(scene: &Scene3d, cible: usize, suivi: &Suivi) -> Result<Scene3d, String> {
    let total = scene.triangles().max(1);
    let ratio = (cible as f64 / total as f64).clamp(0.0, 1.0);
    suivi.total.store(total as u64, Mem::Relaxed);
    suivi.fait.store(0, Mem::Relaxed);
    let mut out = Scene3d { primitives: Vec::with_capacity(scene.primitives.len()), textures: scene.textures.clone() };
    for p in &scene.primitives {
        let voulu = ((p.triangles() as f64 * ratio).round() as usize).max(1);
        let q = simplifier_primitive(p, voulu, suivi)?;
        out.primitives.push(q);
    }
    Ok(out)
}

/// Une quadrique (matrice 4×4 symétrique, ses 10 coefficients).
#[derive(Clone, Copy, Default)]
struct Quadrique([f64; 10]);

impl Quadrique {
    fn du_plan(n: [f64; 3], d: f64, w: f64) -> Quadrique {
        let [a, b, c] = n;
        Quadrique([a * a * w, a * b * w, a * c * w, a * d * w, b * b * w, b * c * w, b * d * w, c * c * w, c * d * w, d * d * w])
    }

    fn ajouter(&mut self, o: &Quadrique) {
        for k in 0..10 {
            self.0[k] += o.0[k];
        }
    }

    fn erreur(&self, p: [f32; 3]) -> f64 {
        let q = &self.0;
        let (x, y, z) = (p[0] as f64, p[1] as f64, p[2] as f64);
        q[0] * x * x + 2.0 * q[1] * x * y + 2.0 * q[2] * x * z + 2.0 * q[3] * x + q[4] * y * y + 2.0 * q[5] * y * z + 2.0 * q[6] * y + q[7] * z * z + 2.0 * q[8] * z + q[9]
    }
}

/// Une contraction candidate : `de` sur `vers`, valable tant que les deux sommets n'ont pas changé (`tampons`).
#[derive(Clone, Copy)]
struct Candidat {
    cout: f64,
    de: u32,
    vers: u32,
    t_de: u32,
    t_vers: u32,
}

impl PartialEq for Candidat {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for Candidat {}
impl PartialOrd for Candidat {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Candidat {
    // (Le tas de Rust donne le plus grand : on inverse, la moins chère d'abord.)
    fn cmp(&self, o: &Self) -> Ordering {
        o.cout.total_cmp(&self.cout).then(o.de.cmp(&self.de)).then(o.vers.cmp(&self.vers))
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Une primitive réduite à environ `cible` triangles.
pub fn simplifier_primitive(p: &Primitive3d, cible: usize, suivi: &Suivi) -> Result<Primitive3d, String> {
    // 1. Sommets soudés par position et coordonnées de texture (les normales seront refaites).
    let mut cles: HashMap<[u32; 5], u32> = HashMap::with_capacity(p.positions.len());
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut uv: Vec<[f32; 2]> = Vec::new();
    let mut remap = Vec::with_capacity(p.positions.len());
    for i in 0..p.positions.len() {
        let t = p.uvs.get(i).copied().unwrap_or([0.0; 2]);
        let a = p.positions[i];
        let cle = [a[0], a[1], a[2], t[0], t[1]].map(f32::to_bits);
        let j = *cles.entry(cle).or_insert_with(|| {
            pos.push(a);
            uv.push(t);
            (pos.len() - 1) as u32
        });
        remap.push(j);
    }
    drop(cles);
    let mut faces: Vec<[u32; 3]> =
        p.indices.chunks_exact(3).map(|c| [remap[c[0] as usize], remap[c[1] as usize], remap[c[2] as usize]]).filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2]).collect();
    let nv = pos.len();
    let mut vivante = vec![true; faces.len()];
    let mut vivantes = faces.len();

    // 2. Les bords : une arête qui n'a pas exactement deux faces. Leurs sommets sont gardés.
    let mut aretes: HashMap<u64, u32> = HashMap::with_capacity(faces.len() * 2);
    for t in &faces {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            *aretes.entry((a.min(b) as u64) << 32 | a.max(b) as u64).or_insert(0) += 1;
        }
    }
    let mut garde = vec![false; nv];
    for (&k, &n) in &aretes {
        if n != 2 {
            garde[(k >> 32) as usize] = true;
            garde[(k & 0xFFFF_FFFF) as usize] = true;
        }
    }
    drop(aretes);

    // 3. Quadriques et faces de chaque sommet.
    let mut q = vec![Quadrique::default(); nv];
    let mut adj: Vec<Vec<u32>> = vec![Vec::new(); nv];
    for (f, t) in faces.iter().enumerate() {
        let [a, b, c] = t.map(|i| pos[i as usize]);
        let n = cross(sub(b, a), sub(c, a));
        let l = dot(n, n).sqrt();
        if l > 1e-20 {
            let nn = [n[0] as f64 / l as f64, n[1] as f64 / l as f64, n[2] as f64 / l as f64];
            let d = -(nn[0] * a[0] as f64 + nn[1] * a[1] as f64 + nn[2] * a[2] as f64);
            let k = Quadrique::du_plan(nn, d, l as f64 * 0.5);
            for &i in t {
                q[i as usize].ajouter(&k);
            }
        }
        for &i in t {
            adj[i as usize].push(f as u32);
        }
    }

    // 4. Les candidates.
    let mut tampon = vec![0u32; nv];
    let mut tas: BinaryHeap<Candidat> = BinaryHeap::new();
    let cout = |q: &[Quadrique], garde: &[bool], de: u32, vers: u32| -> Option<f64> {
        if garde[de as usize] {
            return None;
        }
        let mut s = q[de as usize];
        s.ajouter(&q[vers as usize]);
        Some(s.erreur(pos[vers as usize]).max(0.0))
    };
    let pousser = |tas: &mut BinaryHeap<Candidat>, tampon: &[u32], q: &[Quadrique], garde: &[bool], a: u32, b: u32| {
        for (de, vers) in [(a, b), (b, a)] {
            if let Some(c) = cout(q, garde, de, vers) {
                tas.push(Candidat { cout: c, de, vers, t_de: tampon[de as usize], t_vers: tampon[vers as usize] });
            }
        }
    };
    for t in &faces {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            if a < b {
                pousser(&mut tas, &tampon, &q, &garde, a, b);
            }
        }
    }

    let voisins = |adj: &[Vec<u32>], faces: &[[u32; 3]], vivante: &[bool], v: u32| -> Vec<u32> {
        let mut out: Vec<u32> = adj[v as usize].iter().filter(|&&f| vivante[f as usize]).flat_map(|&f| faces[f as usize]).filter(|&x| x != v).collect();
        out.sort_unstable();
        out.dedup();
        out
    };
    let mut mort = vec![false; nv];
    let mut compte = 0u64;
    while vivantes > cible {
        let Some(c) = tas.pop() else { break };
        let (de, vers) = (c.de as usize, c.vers as usize);
        if mort[de] || mort[vers] || tampon[de] != c.t_de || tampon[vers] != c.t_vers {
            continue;
        }
        compte += 1;
        if compte % 4096 == 0 && suivi.annule.load(Mem::Relaxed) {
            return Err("simplification annulée".into());
        }
        // Les faces de l'arête, et la condition de lien (variété) : les voisins communs sont les sommets opposés de l'arête.
        let communes: Vec<u32> = adj[de].iter().copied().filter(|&f| vivante[f as usize] && faces[f as usize].contains(&c.vers)).collect();
        if communes.is_empty() {
            continue;
        }
        let (va, vb) = (voisins(&adj, &faces, &vivante, c.de), voisins(&adj, &faces, &vivante, c.vers));
        let partages = va.iter().filter(|x| vb.binary_search(x).is_ok()).count();
        if partages != communes.len() {
            continue;
        }
        // Aucune face retournée ni écrasée.
        let mut ok = true;
        for &f in &adj[de] {
            if !vivante[f as usize] || communes.contains(&f) {
                continue;
            }
            let t = faces[f as usize];
            let avant = t.map(|i| pos[i as usize]);
            let apres = t.map(|i| if i == c.de { pos[vers] } else { pos[i as usize] });
            let n0 = cross(sub(avant[1], avant[0]), sub(avant[2], avant[0]));
            let n1 = cross(sub(apres[1], apres[0]), sub(apres[2], apres[0]));
            let (l0, l1) = (dot(n0, n0).sqrt(), dot(n1, n1).sqrt());
            if l1 < 1e-12 * l0.max(1e-30) || dot(n0, n1) < 0.2 * l0 * l1 {
                ok = false;
                break;
            }
        }
        if !ok {
            continue;
        }
        // La contraction.
        for &f in &communes {
            vivante[f as usize] = false;
            vivantes -= 1;
        }
        let faces_de = std::mem::take(&mut adj[de]);
        for f in faces_de {
            if !vivante[f as usize] {
                continue;
            }
            for i in faces[f as usize].iter_mut() {
                if *i == c.de {
                    *i = c.vers;
                }
            }
            adj[vers].push(f);
        }
        adj[vers].retain(|&f| vivante[f as usize]);
        let qd = q[de];
        q[vers].ajouter(&qd);
        mort[de] = true;
        tampon[vers] += 1;
        for n in voisins(&adj, &faces, &vivante, c.vers) {
            tampon[n as usize] += 1;
        }
        for n in voisins(&adj, &faces, &vivante, c.vers) {
            pousser(&mut tas, &tampon, &q, &garde, n, c.vers);
        }
        suivi.fait.fetch_add(communes.len() as u64, Mem::Relaxed);
    }

    // 5. Le maillage restant, avec des normales refaites (lissées sous `ANGLE_VIF`, cassées au-delà).
    let gardees: Vec<[u32; 3]> = faces.iter().zip(&vivante).filter(|(_, v)| **v).map(|(t, _)| *t).collect();
    let normales: Vec<[f32; 3]> = gardees.iter().map(|t| normale_face(pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize])).collect();
    let aires: Vec<f32> = gardees
        .iter()
        .map(|t| {
            let n = cross(sub(pos[t[1] as usize], pos[t[0] as usize]), sub(pos[t[2] as usize], pos[t[0] as usize]));
            dot(n, n).sqrt()
        })
        .collect();
    let mut autour: Vec<Vec<u32>> = vec![Vec::new(); nv];
    for (f, t) in gardees.iter().enumerate() {
        for &i in t {
            autour[i as usize].push(f as u32);
        }
    }
    let seuil = ANGLE_VIF.to_radians().cos();
    let mut out = Primitive3d { couleur: p.couleur, texture: p.texture, seuil_alpha: p.seuil_alpha, ..Default::default() };
    for (f, t) in gardees.iter().enumerate() {
        let nf = normales[f];
        for &i in t {
            let mut s = [0.0f32; 3];
            for &g in &autour[i as usize] {
                let ng = normales[g as usize];
                if dot(nf, ng) >= seuil {
                    let w = aires[g as usize];
                    s = [s[0] + ng[0] * w, s[1] + ng[1] * w, s[2] + ng[2] * w];
                }
            }
            let l = dot(s, s).sqrt();
            out.indices.push(out.positions.len() as u32);
            out.positions.push(pos[i as usize]);
            out.normales.push(if l > 1e-20 { s.map(|x| x / l) } else { nf });
            if !p.uvs.is_empty() {
                out.uvs.push(uv[i as usize]);
            }
        }
    }
    Ok(souder(&out))
}
