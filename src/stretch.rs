//! Vitesse d'un son (14a, sons importés) : changer la vitesse et la hauteur
//! ensemble (rééchantillonnage), ou la vitesse seule (WSOLA : des morceaux du son, recollés en
//! fondu là où ils se ressemblent le plus, gardent leur hauteur).

use crate::audio::{Pcm, RATE};

/// Échantillons stéréo de `pcm`, de `from` à `to` (exclu, bornés au son).
pub fn slice(pcm: &Pcm, from: usize, to: usize) -> Vec<[f32; 2]> {
    let to = to.min(pcm.frames());
    (from.min(to)..to).map(|i| {
        let (l, r) = pcm.frame(i);
        [l, r]
    })
    .collect()
}

/// Lu `speed` fois plus vite, hauteur comprise (interpolation linéaire).
pub fn resample(src: &[[f32; 2]], speed: f64) -> Vec<[f32; 2]> {
    if src.is_empty() || speed <= 0.0 {
        return Vec::new();
    }
    let n = (src.len() as f64 / speed).floor() as usize;
    let last = src.len() - 1;
    (0..n)
        .map(|i| {
            let t = i as f64 * speed;
            let i0 = (t as usize).min(last);
            let f = (t - i0 as f64) as f32;
            let (a, b) = (src[i0], src[(i0 + 1).min(last)]);
            [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]
        })
        .collect()
}

/// Taille d'un morceau (43 ms à 48 kHz), pas de recollage, et marge de recherche.
const WIN: usize = 2048;
const HOP: usize = WIN / 2;
const TOL: usize = 512;

/// Lu `tempo` fois plus vite sans changer la hauteur (WSOLA). Un son trop court pour être
/// découpé est simplement rééchantillonné.
pub fn wsola(src: &[[f32; 2]], tempo: f64) -> Vec<[f32; 2]> {
    if (tempo - 1.0).abs() < 1e-6 {
        return src.to_vec();
    }
    if src.len() < WIN * 2 || tempo <= 0.0 {
        return resample(src, tempo);
    }
    let out_len = (src.len() as f64 / tempo) as usize;
    let win: Vec<f32> = (0..WIN).map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / WIN as f32).cos()).collect();
    let mono: Vec<f32> = src.iter().map(|s| 0.5 * (s[0] + s[1])).collect();
    let max_pos = src.len() - WIN;
    let mut out = vec![[0f32; 2]; out_len + WIN];
    let mut weight = vec![0f32; out_len + WIN];
    let mut prev = 0usize;
    let mut m = 0usize;
    while m * HOP < out_len {
        let at = m * HOP;
        let nominal = ((at as f64 * tempo) as usize).min(max_pos);
        let pos = if m == 0 {
            0
        } else {
            // Suite naturelle du morceau précédent, et le morceau proche de `nominal` qui lui
            // ressemble le plus (comparaison sur un échantillon sur 4, assez pour la parole).
            let natural = (prev + HOP).min(max_pos);
            let lo = nominal.saturating_sub(TOL);
            let hi = (nominal + TOL).min(max_pos);
            let mut best = (f32::MIN, nominal);
            let mut c = lo;
            while c <= hi {
                let mut s = 0f32;
                let mut k = 0;
                while k < WIN {
                    s += mono[c + k] * mono[natural + k];
                    k += 4;
                }
                if s > best.0 {
                    best = (s, c);
                }
                c += 2;
            }
            best.1
        };
        for i in 0..WIN {
            let w = win[i];
            let s = src[pos + i];
            out[at + i][0] += s[0] * w;
            out[at + i][1] += s[1] * w;
            weight[at + i] += w;
        }
        prev = pos;
        m += 1;
    }
    out.truncate(out_len);
    for (o, w) in out.iter_mut().zip(&weight) {
        if *w > 1e-3 {
            o[0] /= w;
            o[1] /= w;
        }
    }
    out
}

/// Multiplie le volume.
pub fn gain(samples: &mut [[f32; 2]], g: f32) {
    if (g - 1.0).abs() > 1e-6 {
        for s in samples {
            s[0] *= g;
            s[1] *= g;
        }
    }
}

/// Le son prêt pour Pelure (stéréo à `RATE`) et le fichier WAV 16 bits qui l'enregistre.
pub fn to_sound(samples: &[[f32; 2]]) -> (Pcm, Vec<u8>) {
    let flat: Vec<f32> = samples.iter().flat_map(|s| [s[0], s[1]]).collect();
    let pcm = Pcm::from_interleaved(&flat, 2, RATE);
    let mut bytes = Vec::with_capacity(44 + pcm.data.len() * 2);
    bytes.extend_from_slice(&crate::export::wav_header(pcm.frames() as u32));
    for v in &pcm.data {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    (pcm, bytes)
}
