//! Propagation d'une peinture sur un calque vidéo (étape e de la propagation façon EbSynth, caisse
//! `synthese`) : on peint des images clés sur un calque bitmap posé sur un calque vidéo ;
//! « Propager » fabrique les autres images de ce calque, sur toute la durée de la vidéo dans le
//! plan. Choix d'Anthony (2026-10-16) :
//! - les images fabriquées vont dans le calque peint lui-même, et ses clés y sont marquées « Clé »
//!   (rôles de TVPaint 12) : on peut retoucher une clé et repropager ;
//! - la clé est le calque peint seul, avec sa transparence : la vidéo reste visible dessous.
//!
//! Les clés sont les images marquées « Clé » qui ont un dessin, s'il y en a ; sinon chaque image
//! où un dessin commence. On travaille dans le rectangle où `video::read_frame` pose les images de
//! la vidéo sur la feuille, en RVBA prémultiplié (les moyennes des patchs restent justes au bord
//! de la peinture). La peinture hors de ce rectangle n'est pas propagée.

use std::ops::Range;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::Color32;
use synthese::propagation::{propagate_keys_on, Key, PropagateParams};
use synthese::{Image, Synthesizer};

use crate::document::{Cel, Document, LayerKind};
use crate::raster::{Raster, TILE};
use crate::roles::Role;
use crate::video;

/// Ce que la propagation fera, relevé sans rien lire de lourd (pour la fenêtre).
#[derive(Clone, Debug, PartialEq)]
pub struct Outline {
    /// Le calque vidéo.
    pub video: usize,
    /// Images du plan où la vidéo a une image.
    pub range: Range<usize>,
    /// Images clés du calque peint.
    pub keys: Vec<usize>,
}

impl Outline {
    /// Nombre d'images à fabriquer.
    pub fn outputs(&self) -> usize {
        self.range.len() - self.keys.len()
    }
}

/// Tout ce qu'il faut pour propager, relevé sur le document au départ : la tâche, sur un autre
/// fil, n'y touche plus.
pub struct Plan {
    pub outline: Outline,
    /// Image de la vidéo pour chaque image de `outline.range`.
    pub files: Vec<PathBuf>,
    /// La peinture de chaque clé, dans le rectangle de la vidéo.
    pub styles: Vec<Image>,
    /// Coin du rectangle de la vidéo sur la feuille, et sa taille.
    pub origin: (usize, usize),
    pub size: (usize, usize),
}

/// Le calque vidéo sous le calque `paint` (le plus proche), sinon le premier du plan.
pub fn video_layer(doc: &Document, panel: usize, paint: usize) -> Option<usize> {
    let layers = &doc.panels.get(panel)?.layers;
    (0..paint.min(layers.len())).rev().find(|&i| layers[i].video.is_some()).or_else(|| layers.iter().position(|l| l.video.is_some()))
}

/// La propagation du calque `paint` du plan `panel` est-elle possible, et que fera-t-elle ?
pub fn outline(doc: &Document, panel: usize, paint: usize) -> Result<Outline, String> {
    let p = doc.panels.get(panel).ok_or("Plan introuvable.")?;
    let layer = p.layers.get(paint).ok_or("Calque introuvable.")?;
    if layer.video.is_some() {
        return Err("C'est le calque vidéo : peins sur un calque au-dessus, puis propage ce calque-là.".into());
    }
    if layer.folder || layer.kind != LayerKind::Bitmap {
        return Err("La propagation se fait sur un calque bitmap.".into());
    }
    let video = video_layer(doc, panel, paint).ok_or("Pas de calque vidéo dans ce plan : importe d'abord une vidéo.")?;
    let clip = p.layers[video].video.as_ref().expect("calque vidéo");
    let shown: Vec<usize> = (0..p.frame_count()).filter(|&f| clip.index_at(f, doc.fps).is_some()).collect();
    let (Some(&first), Some(&last)) = (shown.first(), shown.last()) else {
        return Err("La vidéo ne couvre aucune image de ce plan.".into());
    };
    let range = first..last + 1;
    let marked: Vec<usize> = range.clone().filter(|&f| layer.roles.get(&f) == Some(&Role::Key) && layer.cel_at(f).is_some()).collect();
    let keys = if marked.is_empty() {
        range.clone().filter(|&f| layer.cel_at(f).is_some() && (f == range.start || layer.cel_at(f - 1) != layer.cel_at(f))).collect()
    } else {
        marked
    };
    if keys.is_empty() {
        return Err("Peins au moins une image clé sur ce calque, pendant la vidéo.".into());
    }
    let outline = Outline { video, range, keys };
    if outline.outputs() == 0 {
        return Err("Toutes les images sont des clés : rien à propager.".into());
    }
    Ok(outline)
}

/// Relève tout ce qu'il faut pour propager le calque `paint` du plan `panel`.
pub fn plan(doc: &Document, panel: usize, paint: usize) -> Result<Plan, String> {
    let outline = outline(doc, panel, paint)?;
    let p = &doc.panels[panel];
    let clip = p.layers[outline.video].video.as_ref().expect("calque vidéo");
    let dir = video::dir_of(clip.id).ok_or("Les images de la vidéo sont introuvables.")?;
    let files: Vec<PathBuf> = outline.range.clone().map(|f| video::frame_file(&dir, clip.index_at(f, doc.fps).unwrap_or(0), &clip.ext)).collect();
    let (w, h) = image::image_dimensions(&files[0]).map_err(|e| format!("{} : {e}", files[0].display()))?;
    let size = (w as usize, h as usize);
    let origin = plan_origin(doc, size);
    let styles: Vec<Image> = outline
        .keys
        .iter()
        .map(|&f| {
            let cel = p.layers[paint].cel_at(f).and_then(|c| doc.cels.get(&c));
            match cel {
                Some(cel) => crop(&cel.raster, origin, size),
                None => Image::new(size.0, size.1, 4, vec![0; size.0 * size.1 * 4]),
            }
        })
        .collect();
    if styles.iter().all(|s| s.data.chunks_exact(4).all(|px| px[3] == 0)) {
        return Err("Les clés sont vides dans le rectangle de la vidéo : rien à propager.".into());
    }
    Ok(Plan { outline, files, styles, origin, size })
}

/// Là où `video::read_frame` pose une image de la vidéo de taille `size` sur la feuille : au
/// centre du cadre.
pub fn plan_origin(doc: &Document, size: (usize, usize)) -> (usize, usize) {
    let frame = doc.frame_rect();
    (
        (frame.min.x as i64 + (frame.width() as i64 - size.0 as i64) / 2).max(0) as usize,
        (frame.min.y as i64 + (frame.height() as i64 - size.1 as i64) / 2).max(0) as usize,
    )
}

/// La partie (`origin`, `size`) d'un dessin, en RVBA prémultiplié.
pub fn crop(raster: &Raster, origin: (usize, usize), size: (usize, usize)) -> Image {
    let mut data = Vec::with_capacity(size.0 * size.1 * 4);
    for y in 0..size.1 {
        for x in 0..size.0 {
            let (sx, sy) = (origin.0 + x, origin.1 + y);
            let c = if sx < raster.width && sy < raster.height { raster.get(sx, sy) } else { Color32::TRANSPARENT };
            data.extend_from_slice(&c.to_array());
        }
    }
    Image::new(size.0, size.1, 4, data)
}

/// Une image fabriquée (RVBA prémultiplié) posée en `origin` sur une feuille `width` × `height` ;
/// les zones transparentes restent sans tuile.
pub fn to_raster(width: usize, height: usize, im: &Image, origin: (usize, usize)) -> Raster {
    let mut r = Raster::new(width, height);
    for y in 0..im.height {
        for x in 0..im.width {
            let p = &im.data[(y * im.width + x) * 4..][..4];
            let (sx, sy) = (origin.0 + x, origin.1 + y);
            if p[3] == 0 || sx >= width || sy >= height {
                continue;
            }
            // Le mélange peut dépasser de peu : une couleur prémultipliée reste sous son alpha.
            let a = p[3];
            let i = r.tile_index_of(sx, sy);
            r.tile_mut(i)[(sy % TILE) * TILE + sx % TILE] = Color32::from_rgba_premultiplied(p[0].min(a), p[1].min(a), p[2].min(a), a);
        }
    }
    r
}

/// Propage (sur le fil appelant) : `done(image du plan, image fabriquée)` reçoit chaque image hors
/// clés ; tout s'arrête dès que `cancel` passe à vrai.
pub fn run(
    plan: &Plan,
    synth: &dyn Synthesizer,
    params: &PropagateParams,
    cancel: &AtomicBool,
    mut done: impl FnMut(usize, Image),
) -> Result<(), String> {
    let o = &plan.outline;
    let keys: Vec<Key> = o.keys.iter().zip(&plan.styles).map(|(&frame, style)| Key { frame, style }).collect();
    let load = |f: usize| -> Result<Image, String> {
        let path = &plan.files[f - o.range.start];
        let im = image::open(path).map_err(|e| format!("{} : {e}", path.display()))?.to_rgb8();
        Ok(Image::new(im.width() as usize, im.height() as usize, 3, im.into_raw()))
    };
    propagate_keys_on(synth, load, &keys, o.range.clone(), params, |f, im| {
        if !o.keys.contains(&f) {
            done(f, im.clone());
        }
        !cancel.load(Ordering::Relaxed)
    })
    .map_err(|e| e.to_string())
}

/// Pose les images fabriquées `made` dans le calque `paint` du plan `panel` (des dessins neufs) et
/// marque ses clés « Clé ». (L'appelant prend l'instantané pour l'annulation.)
pub fn apply(doc: &mut Document, panel: usize, paint: usize, origin: (usize, usize), keys: &[usize], made: Vec<(usize, Image)>) {
    for (f, im) in made {
        let raster = to_raster(doc.width, doc.height, &im, origin);
        let cel = doc.add_cel(Cel { raster, vector: None, version: 0 });
        doc.panels[panel].layers[paint].set(f, Some(cel));
    }
    let layer = &mut doc.panels[panel].layers[paint];
    for &k in keys {
        layer.roles.insert(k, Role::Key);
    }
}
