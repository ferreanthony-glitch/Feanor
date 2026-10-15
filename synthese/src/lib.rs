//! Synthèse d'image par l'exemple (phase « propagation », étape a) : portage en Rust de la version
//! processeur d'ebsynth (Ondřej Jamriška, domaine public, https://github.com/jamriska/ebsynth).
//! Étape b : `flow` (flux optique) et `propagation` (une image clé propagée sur une vidéo).
//!
//! On donne une image de style (la peinture) et des paires de guides (source, cible) : la synthèse
//! fabrique une image de la taille des guides cibles en recopiant des morceaux carrés (patchs) du
//! style, choisis pour que les guides cible ressemblent aux guides source. C'est la brique de base
//! d'EbSynth : plus tard (étape b), les guides viendront de la vidéo (contours, position suivie par
//! le flux optique, image précédente) et le style sera une image clé peinte dans Pelure.
//!
//! L'algorithme est celui d'`ebsynth_cpu.cpp`, pas à pas : pyramide d'images, PatchMatch avec la
//! contrainte d'uniformité (chaque morceau du style est utilisé à peu près autant que les autres),
//! vote simple (moyenne des patchs qui recouvrent un pixel), passe finale 3×3 en option.
//! Différences voulues :
//! - le hasard vient d'une graine (`Params::seed`) ; avec un seul fil le résultat est reproductible ;
//! - chaque fil ne propage verticalement qu'à l'intérieur de sa bande de lignes (l'original lit les
//!   lignes des autres fils pendant qu'elles changent) ;
//! - le vote est réparti sur tous les fils (il est séquentiel dans l'original).
//!
//! Attention : PatchMatch est breveté par Adobe (brevet américain 8,861,869), comme le signale le
//! README d'ebsynth. À vérifier avant de distribuer ou de vendre Pelure.

pub mod flow;
mod patchmatch;
pub mod propagation;
#[cfg(test)]
mod flow_tests;
#[cfg(test)]
mod propagation_tests;
#[cfg(test)]
mod tests;

use std::fmt;

use patchmatch::{level_size, nnf_random, nnf_upscale, patchmatch, resample, vote, Plane, Rng, Search};

/// Image 8 bits, canaux entrelacés, lignes à la suite (le format des tampons d'ebsynth).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub channels: usize,
    pub data: Vec<u8>,
}

impl Image {
    pub fn new(width: usize, height: usize, channels: usize, data: Vec<u8>) -> Image {
        assert_eq!(data.len(), width * height * channels, "taille du tampon");
        Image { width, height, channels, data }
    }

    /// Nombre de canaux qu'ebsynth garde d'une image RGBA : 1 si elle est grise, 3 sinon, un de plus
    /// si un pixel n'est pas opaque (`evalNumChannels`).
    pub fn channels_needed(rgba: &[u8]) -> usize {
        let gray = rgba.chunks_exact(4).all(|p| p[0] == p[1] && p[1] == p[2]);
        let alpha = rgba.chunks_exact(4).any(|p| p[3] < 255);
        (if gray { 1 } else { 3 }) + alpha as usize
    }

    /// Range une image RGBA sur `channels` canaux comme ebsynth : 1 = R, 2 = R et A, 3 = RVB,
    /// 4 = RVBA.
    pub fn from_rgba(width: usize, height: usize, rgba: &[u8], channels: usize) -> Image {
        assert!((1..=4).contains(&channels), "1 à 4 canaux");
        assert_eq!(rgba.len(), width * height * 4, "taille du tampon RGBA");
        let mut data = Vec::with_capacity(width * height * channels);
        for p in rgba.chunks_exact(4) {
            match channels {
                1 => data.push(p[0]),
                2 => data.extend_from_slice(&[p[0], p[3]]),
                3 => data.extend_from_slice(&p[..3]),
                _ => data.extend_from_slice(p),
            }
        }
        Image { width, height, channels, data }
    }

    /// Retour en RGBA : le gris est recopié sur R, V et B ; sans canal alpha l'image est opaque.
    pub fn to_rgba(&self) -> Vec<u8> {
        assert!((1..=4).contains(&self.channels), "1 à 4 canaux");
        let mut out = Vec::with_capacity(self.width * self.height * 4);
        for p in self.data.chunks_exact(self.channels) {
            out.extend_from_slice(&match self.channels {
                1 => [p[0], p[0], p[0], 255],
                2 => [p[0], p[0], p[0], p[1]],
                3 => [p[0], p[1], p[2], 255],
                _ => [p[0], p[1], p[2], p[3]],
            });
        }
        out
    }
}

/// Un guide : la même information vue sur le style (`source`) et sur l'image à fabriquer
/// (`target`). `weight` est son poids total, partagé entre ses canaux.
#[derive(Clone, Copy, Debug)]
pub struct GuidePair<'a> {
    pub source: &'a Image,
    pub target: &'a Image,
    pub weight: f32,
}

/// Réglages, avec les valeurs par défaut de la ligne de commande d'ebsynth.
#[derive(Clone, Debug, PartialEq)]
pub struct Params {
    /// Poids de l'uniformité ; 500 à 15000 d'après ebsynth, 3500 par défaut.
    pub uniformity: f32,
    /// Côté des patchs, impair, au moins 3.
    pub patch_size: usize,
    /// Nombre de niveaux de la pyramide ; `None` = autant que la taille des images le permet.
    pub pyramid_levels: Option<usize>,
    /// Recherches suivies d'un vote, à chaque niveau.
    pub search_vote_iters: usize,
    /// Passes de PatchMatch par recherche.
    pub patchmatch_iters: usize,
    /// Passe de finition avec des patchs 3×3 et sans uniformité, au niveau le plus fin.
    pub extra_pass_3x3: bool,
    /// Nombre de fils de calcul ; 0 = tous les cœurs.
    pub threads: usize,
    /// Graine du hasard.
    pub seed: u64,
}

impl Default for Params {
    fn default() -> Params {
        Params {
            uniformity: 3500.0,
            patch_size: 5,
            pyramid_levels: None,
            search_vote_iters: 6,
            patchmatch_iters: 4,
            extra_pass_3x3: false,
            threads: 0,
            seed: 0,
        }
    }
}

/// Résultat : l'image fabriquée (canaux du style, taille des guides cibles) et, pour chacun de ses
/// pixels, le centre du patch du style qui lui a été attribué (`nnf[y * largeur + x] = [x, y]`).
#[derive(Clone, Debug)]
pub struct Output {
    pub image: Image,
    pub nnf: Vec<[i32; 2]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    NoGuide,
    BadPatchSize(usize),
    BadWeight,
    /// Un guide source n'a pas la taille du style, ou un guide cible celle du premier guide cible.
    SizeMismatch(usize),
    /// Les deux images d'un guide n'ont pas le même nombre de canaux.
    ChannelMismatch(usize),
    /// Images plus petites que 2 × patch + 1 pixels de côté.
    TooSmall,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::NoGuide => write!(f, "il faut au moins un guide"),
            Error::BadPatchSize(p) => write!(f, "taille de patch {p} : il faut un nombre impair, au moins 3"),
            Error::BadWeight => write!(f, "les poids doivent être positifs ou nuls"),
            Error::SizeMismatch(i) => write!(f, "le guide {} n'a pas la bonne taille", i + 1),
            Error::ChannelMismatch(i) => {
                write!(f, "les deux images du guide {} n'ont pas le même nombre de canaux", i + 1)
            }
            Error::TooSmall => write!(f, "images trop petites pour cette taille de patch"),
        }
    }
}

impl std::error::Error for Error {}

/// Nombre de niveaux de pyramide possibles : le plus petit côté du niveau le plus grossier doit
/// garder au moins 2 × patch + 1 pixels, pour la source comme pour la cible (comme ebsynth).
pub fn max_pyramid_levels(source: (usize, usize), target: (usize, usize), patch_size: usize) -> usize {
    let w = source.0.min(target.0);
    let h = source.1.min(target.1);
    (0..=32)
        .rev()
        .find(|&level| {
            let f = 2f32.powi(-level);
            ((w as f32 * f) as usize).min((h as f32 * f) as usize) > 2 * patch_size
        })
        .map_or(0, |level| level as usize + 1)
}

/// Fabrique l'image : le style recopié par morceaux pour que les guides cibles ressemblent aux
/// guides sources. `style_weight` est le poids total du style, partagé entre ses canaux.
pub fn synthesize(style: &Image, style_weight: f32, guides: &[GuidePair], params: &Params) -> Result<Output, Error> {
    if guides.is_empty() {
        return Err(Error::NoGuide);
    }
    if params.patch_size < 3 || params.patch_size % 2 == 0 {
        return Err(Error::BadPatchSize(params.patch_size));
    }
    if style_weight < 0.0 || guides.iter().any(|g| g.weight < 0.0) {
        return Err(Error::BadWeight);
    }
    let (sw, sh) = (style.width, style.height);
    let (tw, th) = (guides[0].target.width, guides[0].target.height);
    for (i, g) in guides.iter().enumerate() {
        if (g.source.width, g.source.height) != (sw, sh) || (g.target.width, g.target.height) != (tw, th) {
            return Err(Error::SizeMismatch(i));
        }
        if g.source.channels != g.target.channels {
            return Err(Error::ChannelMismatch(i));
        }
    }
    let max_levels = max_pyramid_levels((sw, sh), (tw, th), params.patch_size);
    if max_levels == 0 {
        return Err(Error::TooSmall);
    }
    let levels = params.pyramid_levels.unwrap_or(max_levels).clamp(1, max_levels);
    let threads = match params.threads {
        0 => std::thread::available_parallelism().map_or(1, |n| n.get()),
        n => n,
    };

    // Style et guides entrelacés dans un seul tampon par image (canaux du style d'abord) : le coût
    // d'un patch devient une somme pondérée sur des lignes d'octets contiguës.
    let ns = style.channels;
    let c = ns + guides.iter().map(|g| g.source.channels).sum::<usize>();
    let mut weights = vec![style_weight / ns as f32; ns];
    for g in guides {
        weights.extend(std::iter::repeat(g.weight / g.source.channels as f32).take(g.source.channels));
    }
    // La cible n'a pas encore de style : des zéros, que le premier vote remplace.
    let interleave = |w: usize, h: usize, style: Option<&Image>| {
        let mut data = Vec::with_capacity(w * h * c);
        for i in 0..w * h {
            match style {
                Some(s) => data.extend_from_slice(&s.data[i * ns..(i + 1) * ns]),
                None => data.extend(std::iter::repeat(0).take(ns)),
            }
            for g in guides {
                let im = if style.is_some() { g.source } else { g.target };
                data.extend_from_slice(&im.data[i * im.channels..(i + 1) * im.channels]);
            }
        }
        Plane { w, h, c, data }
    };
    let full_source = interleave(sw, sh, Some(style));
    let mut full_target = Some(interleave(tw, th, None));

    let mut rng = Rng::new(params.seed);
    let mut prev_nnf: Option<(Vec<[i32; 2]>, usize, usize)> = None;
    let mut result = None;
    for level in 0..levels {
        let last = level == levels - 1;
        let (lsw, lsh) = level_size((sw, sh), levels, level);
        let (ltw, lth) = level_size((tw, th), levels, level);
        // Les niveaux grossiers sont tous rééchantillonnés depuis les images entières, comme
        // ebsynth ; la partie style de la cible sera écrite par le vote.
        let small_source;
        let source = if last {
            &full_source
        } else {
            small_source = resample(&full_source, lsw, lsh);
            &small_source
        };
        let mut target = if last {
            full_target.take().expect("cible du dernier niveau")
        } else {
            resample(full_target.as_ref().expect("cible entière"), ltw, lth)
        };
        let mut nnf = match prev_nnf.take() {
            None => nnf_random((ltw, lth), (lsw, lsh), params.patch_size, &mut rng),
            Some((prev, pw, ph)) => nnf_upscale(&prev, (pw, ph), (ltw, lth), (lsw, lsh), params.patch_size),
        };
        let mut errors = vec![0f32; ltw * lth];
        let mut run = |patch_size: usize, uniformity: f32, target: &mut Plane, nnf: &mut Vec<[i32; 2]>| {
            vote(target, source, ns, nnf, patch_size, threads);
            for _ in 0..params.search_vote_iters {
                let search = Search {
                    target: &*target,
                    source,
                    weights: &weights,
                    patch_size,
                    uniformity,
                    iters: params.patchmatch_iters,
                    threads,
                };
                patchmatch(&search, nnf, &mut errors, &mut rng);
                vote(target, source, ns, nnf, patch_size, threads);
            }
        };
        run(params.patch_size, params.uniformity, &mut target, &mut nnf);
        if last && params.extra_pass_3x3 {
            run(3, 0.0, &mut target, &mut nnf);
        }
        if last {
            let data = target.data.chunks_exact(c).flat_map(|p| &p[..ns]).copied().collect();
            result = Some(Output { image: Image::new(tw, th, ns, data), nnf });
        } else {
            prev_nnf = Some((nnf, ltw, lth));
        }
    }
    Ok(result.expect("au moins un niveau"))
}
