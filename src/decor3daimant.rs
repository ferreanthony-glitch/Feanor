//! « Aimanté au sol » (partie 181, Anthony : « Tout objet peut être aimanté dans la scène 3D pour se fixer directement au sol
//! sans pénétration ») : la hauteur d'un objet aimanté du décor 3D n'est pas tapée, elle est **calculée** — il repose sur ce qui
//! est sous lui, le sol (`Monde::sol_y`) ou le dessus des autres objets du décor (un banc sur une dalle de route, un arbre sur un
//! morceau de terrain), sans y entrer.
//!
//! **Comment.** Le dessous de l'objet est le plus bas de ses sommets (la rotation n'est qu'autour de la verticale et l'échelle
//! est la même sur les trois axes : le plus bas sommet placé est le bas de la boîte du modèle, `Modele3d::bornes`, à
//! l'échelle). On lance neuf verticales à travers son empreinte (les coins, les milieux des bords et le centre de sa boîte, un
//! peu rentrés pour ne pas toucher le voisin collé), et on relève où chacune traverse les **triangles** des autres objets
//! (dans leur propre repère, où la verticale reste verticale : un test de point dans un triangle, à plat). Une hauteur `c`
//! convient si, sur chaque verticale, aucune surface ne passe entre `c` et `c + hauteur de l'objet` (à 1 mm près) et si
//! l'objet n'est pas enfermé dans un autre (nombre pair de traversées au-dessus de son milieu). Parmi les hauteurs qui
//! conviennent (le sol, et chaque surface traversée), l'objet prend **la plus proche de là où il est** (à égalité, la plus
//! haute) : il monte sur la dalle qu'on glisse sous lui, mais un feu de camp glissé sous la ramure d'un arbre reste au sol.
//! Il y en a toujours une : au-dessus de tout.
//!
//! **Pas de boucle.** Les objets aimantés se posent un par un : d'abord les plus bas (au millimètre), à égalité la plus grande
//! empreinte (la dalle avant le banc posé dessus, le terrain avant l'arbre), puis le rang dans la liste ; chacun ne s'appuie que
//! sur les objets non aimantés et sur ceux déjà posés. Un objet ne repose donc jamais sur un objet qui repose sur lui, et deux
//! calculs de suite donnent la même chose.
//!
//! **Bon marché.** Calculé aux retouches seulement (ajout, déplacement, rotation, échelle, origine, retrait, un objet du dessous
//! qui bouge), jamais à chaque image ; une verticale qui ne passe pas dans la boîte d'un objet ne regarde pas ses triangles ; un
//! objet de plus de `TRIANGLES_EXACTS` triangles sert d'appui par sa boîte seulement (la barre d'état le dit), pour que le
//! glisser reste fluide.

use crate::decor3d::{Decor3d, Objet3d};
use crate::espace3dmath::{self as m, M4};

/// La tolérance du contact (mètres) : une surface à moins d'un millimètre du dessous de l'objet le touche, elle n'y entre pas.
pub const TOLERANCE: f32 = 0.001;
/// Au-delà, un objet sert d'appui par sa boîte (le contact exact coûterait trop à chaque retouche).
pub const TRIANGLES_EXACTS: usize = 150_000;

/// Ce qu'on garde d'un objet le temps du calcul.
struct Fiche {
    inverse: M4,
    /// Sa boîte dans le monde, en x et z.
    xz: [f32; 4],
    /// Le bas de la boîte de son modèle et sa hauteur dans le repère du modèle.
    lo_y: f32,
    hi_y: f32,
    echelle: f32,
    origine_y: f32,
    /// Les neuf points d'appui (x, z du monde).
    appuis: Vec<[f32; 2]>,
    aire: f32,
    boite_seule: bool,
}

fn fiche(o: &Objet3d) -> Fiche {
    let (lo, hi) = o.modele.bornes;
    let mx = o.matrice();
    let (blo, bhi) = o.bornes_monde();
    let e = o.place.echelle_sure();
    // (Un peu rentrés : un point d'appui sur le bord même toucherait l'objet collé à côté.)
    let rentre = |a: f32, b: f32| ((b - a) * 0.02).max(0.0);
    let (dx, dz) = (rentre(lo[0], hi[0]), rentre(lo[2], hi[2]));
    let xs = [lo[0] + dx, (lo[0] + hi[0]) / 2.0, hi[0] - dx];
    let zs = [lo[2] + dz, (lo[2] + hi[2]) / 2.0, hi[2] - dz];
    let appuis = xs.iter().flat_map(|&x| zs.iter().map(move |&z| (x, z))).map(|(x, z)| {
        let p = m::point(&mx, [x, lo[1], z]);
        [p[0], p[2]]
    });
    Fiche {
        inverse: m::inverse_affine(&mx),
        xz: [blo[0], bhi[0], blo[2], bhi[2]],
        lo_y: lo[1],
        hi_y: hi[1],
        echelle: e,
        origine_y: o.origine[1],
        appuis: appuis.collect(),
        aire: e * e * (hi[0] - lo[0]).max(0.0) * (hi[2] - lo[2]).max(0.0),
        boite_seule: o.modele.scene.triangles() > TRIANGLES_EXACTS,
    }
}

/// Où la verticale (x, z) du monde traverse l'objet `o` : hauteurs dans le repère de son modèle, triées, les doublons (une
/// verticale sur l'arête commune de deux triangles) fondus.
fn traversees(o: &Objet3d, f: &Fiche, x: f32, z: f32, out: &mut Vec<f32>) {
    out.clear();
    if x < f.xz[0] - TOLERANCE || x > f.xz[1] + TOLERANCE || z < f.xz[2] - TOLERANCE || z > f.xz[3] + TOLERANCE {
        return;
    }
    let p = m::point(&f.inverse, [x, 0.0, z]);
    let (px, pz) = (p[0], p[2]);
    let (lo, hi) = o.modele.bornes;
    if px < lo[0] || px > hi[0] || pz < lo[2] || pz > hi[2] {
        return;
    }
    if f.boite_seule {
        out.extend([lo[1], hi[1]]);
        return;
    }
    for prim in &o.modele.scene.primitives {
        for t in prim.indices.chunks_exact(3) {
            let (Some(a), Some(b), Some(c)) = (prim.positions.get(t[0] as usize), prim.positions.get(t[1] as usize), prim.positions.get(t[2] as usize)) else { continue };
            let d = (b[0] - a[0]) * (c[2] - a[2]) - (c[0] - a[0]) * (b[2] - a[2]);
            if d.abs() < 1e-12 {
                // (Une face verticale : une verticale ne la traverse pas.)
                continue;
            }
            let w1 = ((px - a[0]) * (c[2] - a[2]) - (c[0] - a[0]) * (pz - a[2])) / d;
            let w2 = ((b[0] - a[0]) * (pz - a[2]) - (px - a[0]) * (b[2] - a[2])) / d;
            let w0 = 1.0 - w1 - w2;
            const E: f32 = -1e-6;
            if w0 >= E && w1 >= E && w2 >= E {
                out.push(w0 * a[1] + w1 * b[1] + w2 * c[1]);
            }
        }
    }
    out.sort_by(f32::total_cmp);
    out.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
}

/// Pose les objets aimantés du décor `d` sur ce qui est sous eux (`sol_y` : le sol). Renvoie ce que la barre d'état doit dire
/// (un objet trop lourd pour le contact exact, qui sert d'appui par sa boîte).
pub fn aimanter(d: &mut Decor3d, sol_y: f32) -> Vec<String> {
    let n = d.objets.len();
    if !d.objets.iter().any(|o| o.aimante) {
        return Vec::new();
    }
    let mut fiches: Vec<Fiche> = d.objets.iter().map(fiche).collect();
    let mut pose: Vec<bool> = d.objets.iter().map(|o| !o.aimante).collect();
    let mut notes = Vec::new();
    let mut hits = Vec::new();
    loop {
        // Le prochain : le plus bas (au millimètre), puis la plus grande empreinte, puis le rang.
        let bas = |i: usize| {
            let o = &d.objets[i];
            let f = &fiches[i];
            ((o.place.position[1] + f.echelle * (f.lo_y - f.origine_y)) * 1000.0).round() as i64
        };
        let Some(i) = (0..n).filter(|&i| !pose[i]).min_by(|&a, &b| bas(a).cmp(&bas(b)).then(fiches[b].aire.total_cmp(&fiches[a].aire)).then(a.cmp(&b))) else { break };
        let f = &fiches[i];
        let hauteur = f.echelle * (f.hi_y - f.lo_y);
        let ici = d.objets[i].place.position[1] + f.echelle * (f.lo_y - f.origine_y);
        // Sur chaque verticale : les surfaces des objets déjà posés (monde), avec leur objet (pour savoir si on est dedans).
        let mut verticales: Vec<Vec<(usize, f32)>> = Vec::with_capacity(f.appuis.len());
        for &[x, z] in &f.appuis {
            let mut v = Vec::new();
            for j in (0..n).filter(|&j| j != i && pose[j]) {
                let (o, fj) = (&d.objets[j], &fiches[j]);
                traversees(o, fj, x, z, &mut hits);
                if fj.boite_seule && !hits.is_empty() && !notes.iter().any(|n: &String| n.contains(&o.nom)) {
                    notes.push(format!("« {} » a trop de triangles pour le contact exact : sa boîte sert d'appui aux objets aimantés.", o.nom));
                }
                let base = o.place.position[1] - fj.echelle * fj.origine_y;
                v.extend(hits.iter().map(|&y| (j, base + fj.echelle * y)));
            }
            verticales.push(v);
        }
        let convient = |c: f32| {
            let milieu = c + hauteur / 2.0;
            verticales.iter().all(|v| {
                if v.iter().any(|&(_, y)| y > c + TOLERANCE && y < c + hauteur - TOLERANCE) {
                    return false;
                }
                // (Dedans un objet fermé : un nombre impair de ses surfaces au-dessus.)
                let mut dessus: Vec<(usize, usize)> = Vec::new();
                for &(j, _) in v.iter().filter(|&&(_, y)| y > milieu) {
                    match dessus.iter_mut().find(|(k, _)| *k == j) {
                        Some(e) => e.1 += 1,
                        None => dessus.push((j, 1)),
                    }
                }
                dessus.iter().all(|&(_, k)| k % 2 == 0)
            })
        };
        let mut niveaux: Vec<f32> = std::iter::once(sol_y).chain(verticales.iter().flatten().map(|&(_, y)| y).filter(|&y| y >= sol_y - TOLERANCE).map(|y| y.max(sol_y))).collect();
        niveaux.sort_by(f32::total_cmp);
        niveaux.dedup_by(|a, b| (*a - *b).abs() < TOLERANCE / 10.0);
        let choisi = niveaux.iter().copied().filter(|&c| convient(c)).min_by(|a, b| (a - ici).abs().total_cmp(&(b - ici).abs()).then(b.total_cmp(a))).unwrap_or_else(|| niveaux.last().copied().unwrap_or(sol_y));
        let o = &mut d.objets[i];
        o.place.position[1] = choisi + f.echelle * (f.origine_y - f.lo_y);
        pose[i] = true;
        // (Sa fiche ne dépend pas de sa hauteur pour x et z ; l'inverse, si : refaite.)
        fiches[i] = fiche(&d.objets[i]);
    }
    notes
}
