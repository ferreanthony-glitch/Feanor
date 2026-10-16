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
//!
//! Plusieurs clés (étape c) : avant la première et après la dernière, une seule séquence ; entre
//! deux clés, une séquence depuis chacune, mélangées image par image (`blend`).

use std::fmt;
use std::ops::Range;

use crate::blend::{BlendParams, ErrorMap};
use crate::flow::{flow, Flow, FlowParams, Gray};
use crate::{Cpu, Error, GuidePair, Image, Output, Params, Synthesizer};

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
    pub blend: BlendParams,
}

/// Une image clé : la peinture `style` de l'image `frame` de la vidéo.
#[derive(Clone, Copy, Debug)]
pub struct Key<'a> {
    pub frame: usize,
    pub style: &'a Image,
}

#[derive(Debug)]
pub enum PropagateError {
    /// L'image n'a pas pu être lue.
    Load(usize, String),
    /// L'image n'a pas la taille (ou le nombre de canaux) des autres.
    Size(usize),
    NoKey,
    /// Cette clé n'est pas dans la plage d'images à fabriquer.
    KeyOutsideRange(usize),
    /// Deux clés sur la même image.
    DuplicateKey(usize),
    Synth(usize, Error),
    /// `each` a demandé d'arrêter.
    Cancelled,
}

impl fmt::Display for PropagateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PropagateError::Load(i, e) => write!(f, "image {} : {e}", i + 1),
            PropagateError::Size(i) => write!(f, "l'image {} n'a pas la taille des autres", i + 1),
            PropagateError::NoKey => write!(f, "il faut au moins une image clé"),
            PropagateError::KeyOutsideRange(i) => write!(f, "la clé de l'image {} n'est pas dans la plage", i + 1),
            PropagateError::DuplicateKey(i) => write!(f, "deux clés sur l'image {}", i + 1),
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

/// Taille des carrés de l'erreur réduite qui décide du mélange.
const ERROR_CELL: usize = 8;

/// Propage la clé `style`, peinte sur l'image `key` de la vidéo, sur les images `range` (qui
/// contient `key`) : `propagate_keys` avec une seule clé.
pub fn propagate(
    load: impl FnMut(usize) -> Result<Image, String>,
    key: usize,
    style: &Image,
    range: Range<usize>,
    params: &PropagateParams,
    each: impl FnMut(usize, &Image) -> bool,
) -> Result<(), PropagateError> {
    propagate_keys(load, &[Key { frame: key, style }], range, params, each)
}

/// Propage les clés `keys` sur les images `range` (qui les contient toutes). `load(i)` lit l'image
/// `i` de la vidéo ; `each(i, image)` reçoit chaque image fabriquée, une fois chacune, et renvoie
/// `false` pour arrêter. Ordre : les clés elles-mêmes, les images avant la première clé (en
/// reculant), celles entre deux clés (en reculant depuis la seconde, une fois la séquence depuis
/// la première faite), celles après la dernière clé (en avançant).
pub fn propagate_keys(
    load: impl FnMut(usize) -> Result<Image, String>,
    keys: &[Key],
    range: Range<usize>,
    params: &PropagateParams,
    each: impl FnMut(usize, &Image) -> bool,
) -> Result<(), PropagateError> {
    propagate_keys_on(&Cpu, load, keys, range, params, each)
}

/// `propagate_keys` avec une synthèse au choix : `Cpu` ou la carte graphique (`gpu::Gpu`).
pub fn propagate_keys_on(
    synth: &dyn Synthesizer,
    mut load: impl FnMut(usize) -> Result<Image, String>,
    keys: &[Key],
    range: Range<usize>,
    params: &PropagateParams,
    mut each: impl FnMut(usize, &Image) -> bool,
) -> Result<(), PropagateError> {
    let mut keys = keys.to_vec();
    keys.sort_by_key(|k| k.frame);
    let (Some(first), Some(last)) = (keys.first().copied(), keys.last().copied()) else {
        return Err(PropagateError::NoKey);
    };
    if let Some(k) = keys.iter().find(|k| !range.contains(&k.frame)) {
        return Err(PropagateError::KeyOutsideRange(k.frame));
    }
    if let Some(pair) = keys.windows(2).find(|p| p[0].frame == p[1].frame) {
        return Err(PropagateError::DuplicateKey(pair[0].frame));
    }
    let mut frames = Vec::with_capacity(keys.len());
    for k in &keys {
        let f = load(k.frame).map_err(|e| PropagateError::Load(k.frame, e))?;
        let size = |im: &Image| (im.width, im.height);
        if size(k.style) != size(&f) || frames.first().is_some_and(|f0: &Image| (size(f0), f0.channels) != (size(&f), f.channels)) {
            return Err(PropagateError::Size(k.frame));
        }
        frames.push(f);
    }
    let mut emit = |t: usize, im: &Image| if each(t, im) { Ok(()) } else { Err(PropagateError::Cancelled) };
    for k in &keys {
        emit(k.frame, k.style)?;
    }
    chain(synth, &mut load, first, &frames[0], (range.start..first.frame).rev(), params, |t, o| emit(t, &o.image))?;
    for (i, pair) in keys.windows(2).enumerate() {
        let (k1, k2) = (pair[0], pair[1]);
        let mut from_first: Vec<(Image, ErrorMap)> = Vec::with_capacity(k2.frame - k1.frame);
        chain(synth, &mut load, k1, &frames[i], k1.frame + 1..k2.frame, params, |_, o| {
            let e = ErrorMap::new(&o.error, o.image.width, o.image.height, ERROR_CELL);
            from_first.push((o.image, e));
            Ok(())
        })?;
        let span = (k2.frame - k1.frame) as f32;
        chain(synth, &mut load, k2, &frames[i + 1], (k1.frame + 1..k2.frame).rev(), params, |t, o| {
            let (a, ea) = &from_first[t - k1.frame - 1];
            let eb = ErrorMap::new(&o.error, o.image.width, o.image.height, ERROR_CELL);
            emit(t, &synth.blend(a, ea, &o.image, &eb, (t - k1.frame) as f32 / span, &params.blend))
        })?;
    }
    chain(synth, &mut load, last, &frames[keys.len() - 1], last.frame + 1..range.end, params, |t, o| emit(t, &o.image))
}

/// Une séquence depuis la clé `key` (sur l'image de la vidéo `key_frame`) : les images `steps`,
/// chacune voisine de la précédente (la première voisine de la clé). `out(t, résultat)` reçoit
/// chacune. Le flux optique de l'image suivante (qui ne dépend que de la vidéo) est calculé sur
/// un autre fil pendant la synthèse de l'image courante : avec la carte graphique, il ne coûte
/// presque plus rien.
fn chain(
    synth: &dyn Synthesizer,
    load: &mut impl FnMut(usize) -> Result<Image, String>,
    key: Key,
    key_frame: &Image,
    steps: impl Iterator<Item = usize>,
    params: &PropagateParams,
    mut out: impl FnMut(usize, Output) -> Result<(), PropagateError>,
) -> Result<(), PropagateError> {
    let (w, h, style) = (key_frame.width, key_frame.height, key.style);
    let key_position = position_guide(&identity(w, h));
    let wt = &params.weights;
    let mut load_checked = |t: usize| {
        let frame = load(t).map_err(|e| PropagateError::Load(t, e))?;
        if (frame.width, frame.height, frame.channels) != (w, h, key_frame.channels) {
            return Err(PropagateError::Size(t));
        }
        Ok(frame)
    };
    // L'image t, sa luminance et, pour chacun de ses pixels, où il était dans l'image d'avant.
    let prepare = |frame: Image, before: &Gray| {
        let gray = Gray::from_image(&frame);
        let back = flow(&gray, before, &params.flow);
        (frame, gray, back)
    };
    let steps: Vec<usize> = steps.collect();
    let (mut prev_gray, mut prev_out, mut pos) = (Gray::from_image(key_frame), style.clone(), identity(w, h));
    let mut ready = None;
    for (k, &t) in steps.iter().enumerate() {
        let (frame, gray, back) = match ready.take() {
            Some(r) => r,
            None => prepare(load_checked(t)?, &prev_gray),
        };
        let following = steps.get(k + 1).map(|&u| load_checked(u)).transpose()?;
        pos = warp_positions(&pos, &back);
        let position = position_guide(&pos);
        let temporal = warp(&prev_out, &back);
        let mut guides = Vec::new();
        if wt.video > 0.0 {
            guides.push(GuidePair { source: key_frame, target: &frame, weight: wt.video });
        }
        if wt.position > 0.0 {
            guides.push(GuidePair { source: &key_position, target: &position, weight: wt.position });
        }
        if wt.temporal > 0.0 {
            guides.push(GuidePair { source: style, target: &temporal, weight: wt.temporal });
        }
        let (result, next) = std::thread::scope(|scope| {
            let next = following.map(|f| {
                let (prepare, gray) = (&prepare, &gray);
                scope.spawn(move || prepare(f, gray))
            });
            let result = synth.synthesize(style, wt.style, &guides, &params.synth);
            (result, next.map(|n| n.join().expect("flux optique")))
        });
        let result = result.map_err(|e| PropagateError::Synth(t, e))?;
        ready = next;
        (prev_gray, prev_out) = (gray, result.image.clone());
        out(t, result)?;
    }
    Ok(())
}
