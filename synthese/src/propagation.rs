//! Propagation d'une image clé sur une vidéo (étape b), d'après « Stylizing Video by Example »
//! (Jamriška et al., SIGGRAPH 2019) et les réglages d'EbSynth. On part de l'image clé et on
//! avance (puis on recule) d'une image à la fois. Chaque image est synthétisée depuis la clé avec
//! trois guides :
//! - vidéo : l'image de la vidéo sous la clé, contre l'image de la vidéo à fabriquer ;
//! - position : pour chaque pixel, d'où il vient dans l'image clé, suivi par le flux optique
//!   d'image en image (rouge = x, vert = y) ;
//! - temporel : l'image fabriquée juste avant, déplacée par le flux, contre la clé elle-même
//!   (c'est ce qui limite le scintillement).
//!
//! Poids par défaut ceux d'EbSynth : clé 1, vidéo 4, position (« Mapping ») 10, temporel
//! (« De-flicker ») 1.

use std::fmt;
use std::ops::Range;

use crate::flow::{flow, Flow, FlowParams, Gray};
use crate::{synthesize, Error, GuidePair, Image, Params};

/// Poids des guides de la propagation.
#[derive(Clone, Debug, PartialEq)]
pub struct Weights {
    pub style: f32,
    pub video: f32,
    pub position: f32,
    pub temporal: f32,
}

impl Default for Weights {
    fn default() -> Weights {
        Weights { style: 1.0, video: 4.0, position: 10.0, temporal: 1.0 }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PropagateParams {
    pub weights: Weights,
    pub synth: Params,
    pub flow: FlowParams,
}

#[derive(Debug)]
pub enum PropagateError {
    /// L'image n'a pas pu être lue.
    Load(usize, String),
    /// L'image n'a pas la taille (ou le nombre de canaux) de l'image clé.
    Size(usize),
    /// La clé n'est pas dans la plage d'images à fabriquer.
    KeyOutsideRange,
    Synth(usize, Error),
    /// `each` a demandé d'arrêter.
    Cancelled,
}

impl fmt::Display for PropagateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PropagateError::Load(i, e) => write!(f, "image {} : {e}", i + 1),
            PropagateError::Size(i) => write!(f, "l'image {} n'a pas la taille de l'image clé", i + 1),
            PropagateError::KeyOutsideRange => write!(f, "l'image clé n'est pas dans la plage"),
            PropagateError::Synth(i, e) => write!(f, "image {} : {e}", i + 1),
            PropagateError::Cancelled => write!(f, "propagation arrêtée"),
        }
    }
}

impl std::error::Error for PropagateError {}

/// Image déplacée par un flux : le pixel x prend la valeur bilinéaire de `im` en x + flux(x).
pub fn warp(im: &Image, flow: &Flow) -> Image {
    assert_eq!((im.width, im.height), (flow.width, flow.height), "même taille");
    let (w, h, c) = (im.width as i32, im.height as i32, im.channels);
    let at = |x: i32, y: i32| (y.clamp(0, h - 1) * w + x.clamp(0, w - 1)) as usize * c;
    let mut data = Vec::with_capacity(im.data.len());
    for (i, u) in flow.data.iter().enumerate() {
        let (x, y) = ((i % im.width) as f32 + u[0], (i / im.width) as f32 + u[1]);
        let (fx, fy) = (x.floor(), y.floor());
        let (a, b) = (x - fx, y - fy);
        let (ix, iy) = (fx as i32, fy as i32);
        let (p00, p10, p01, p11) = (at(ix, iy), at(ix + 1, iy), at(ix, iy + 1), at(ix + 1, iy + 1));
        for k in 0..c {
            let v = (im.data[p00 + k] as f32 * (1.0 - a) + im.data[p10 + k] as f32 * a) * (1.0 - b)
                + (im.data[p01 + k] as f32 * (1.0 - a) + im.data[p11 + k] as f32 * a) * b;
            data.push((v + 0.5) as u8);
        }
    }
    Image::new(im.width, im.height, c, data)
}

/// Positions suivies : chaque pixel prend la position (dans l'image clé) de l'endroit d'où il
/// vient, lue de façon bilinéaire.
fn warp_positions(pos: &Flow, flow: &Flow) -> Flow {
    let data = flow
        .data
        .iter()
        .enumerate()
        .map(|(i, u)| pos.sample((i % pos.width) as f32 + u[0], (i / pos.width) as f32 + u[1]))
        .collect();
    Flow { width: pos.width, height: pos.height, data }
}

/// Guide de position (2 canaux) : x et y dans l'image clé ramenés à 0..255.
pub fn position_guide(pos: &Flow) -> Image {
    let (sx, sy) = (255.0 / (pos.width.max(2) - 1) as f32, 255.0 / (pos.height.max(2) - 1) as f32);
    let data = pos
        .data
        .iter()
        .flat_map(|p| [(p[0] * sx + 0.5).clamp(0.0, 255.0) as u8, (p[1] * sy + 0.5).clamp(0.0, 255.0) as u8])
        .collect();
    Image::new(pos.width, pos.height, 2, data)
}

fn identity(width: usize, height: usize) -> Flow {
    Flow { width, height, data: (0..width * height).map(|i| [(i % width) as f32, (i / width) as f32]).collect() }
}

/// Propage la clé `style`, peinte sur l'image `key` de la vidéo, sur les images `range` (qui
/// contient `key`). `load(i)` lit l'image `i` de la vidéo ; `each(i, image)` reçoit chaque image
/// fabriquée (la clé elle-même pour `key`, puis en avançant, puis en reculant depuis la clé) et
/// renvoie `false` pour arrêter.
pub fn propagate(
    mut load: impl FnMut(usize) -> Result<Image, String>,
    key: usize,
    style: &Image,
    range: Range<usize>,
    params: &PropagateParams,
    mut each: impl FnMut(usize, &Image) -> bool,
) -> Result<(), PropagateError> {
    if !range.contains(&key) {
        return Err(PropagateError::KeyOutsideRange);
    }
    let key_frame = load(key).map_err(|e| PropagateError::Load(key, e))?;
    let (w, h) = (key_frame.width, key_frame.height);
    if (style.width, style.height) != (w, h) {
        return Err(PropagateError::Size(key));
    }
    if !each(key, style) {
        return Err(PropagateError::Cancelled);
    }
    let key_gray = Gray::from_image(&key_frame);
    let key_position = position_guide(&identity(w, h));
    let wt = &params.weights;
    let directions: [Vec<usize>; 2] = [(key + 1..range.end).collect(), (range.start..key).rev().collect()];
    for steps in directions {
        let (mut prev_gray, mut prev_out, mut pos) = (key_gray.clone(), style.clone(), identity(w, h));
        for t in steps {
            let frame = load(t).map_err(|e| PropagateError::Load(t, e))?;
            if (frame.width, frame.height, frame.channels) != (w, h, key_frame.channels) {
                return Err(PropagateError::Size(t));
            }
            let gray = Gray::from_image(&frame);
            // Pour chaque pixel de l'image t, où il était dans l'image d'avant.
            let back = flow(&gray, &prev_gray, &params.flow);
            pos = warp_positions(&pos, &back);
            let position = position_guide(&pos);
            let temporal = warp(&prev_out, &back);
            let mut guides = Vec::new();
            if wt.video > 0.0 {
                guides.push(GuidePair { source: &key_frame, target: &frame, weight: wt.video });
            }
            if wt.position > 0.0 {
                guides.push(GuidePair { source: &key_position, target: &position, weight: wt.position });
            }
            if wt.temporal > 0.0 {
                guides.push(GuidePair { source: style, target: &temporal, weight: wt.temporal });
            }
            let out = synthesize(style, wt.style, &guides, &params.synth).map_err(|e| PropagateError::Synth(t, e))?;
            if !each(t, &out.image) {
                return Err(PropagateError::Cancelled);
            }
            (prev_gray, prev_out) = (gray, out.image);
        }
    }
    Ok(())
}
