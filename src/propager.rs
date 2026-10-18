//! Propagation d'une peinture (étape e de la propagation façon EbSynth, caisse `synthese`) : on
//! peint des images clés sur un calque bitmap ; « Propager » fabrique les autres images de ce
//! calque en suivant le mouvement d'un guide. Choix d'Anthony (2026-10-16) :
//! - les images fabriquées vont dans le calque peint lui-même, et ses clés y sont marquées « Clé »
//!   (rôles de TVPaint 12) : on peut retoucher une clé et repropager ;
//! - la clé est le calque peint seul, avec sa transparence : ce qui est dessous reste visible ;
//! - trois guides au choix (`Guide`) : un calque vidéo, ce qu'on voit sous le calque peint (tous
//!   les calques visibles dessous, composés), ou un calque choisi (la ligne d'une animation faite
//!   dans Pelure, sans le décor).
//!
//! Les clés sont les images marquées « Clé » qui ont un dessin, s'il y en a ; sinon chaque image
//! où un dessin commence. On refait toute la plage du guide, ou la plage choisie à la main (dans
//! celle du guide) : les images hors plage restent telles quelles, et les clés hors plage ne
//! servent pas. On travaille dans un rectangle de la feuille, en RVBA prémultiplié (les
//! moyennes des patchs restent justes au bord de la peinture) : celui où `video::read_frame` pose
//! les images de la vidéo, ou le cadre pour les autres guides. La peinture hors de ce rectangle
//! n'est pas propagée.

use std::ops::Range;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::Color32;
use synthese::propagation::{propagate_keys_on, Key, PropagateParams};
use synthese::{Image, Synthesizer};

use crate::document::{Cel, Document, LayerKind};
use crate::raster::{Raster, TILE};
use crate::render::{self, PlanItem};
use crate::roles::Role;
use crate::video;

/// Ce que la peinture suit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Guide {
    /// Un calque vidéo (identifiant du calque) : ses images, telles quelles.
    Video(u64),
    /// Tous les calques visibles sous le calque peint, composés : ce qu'on voit sous la peinture.
    Below,
    /// Un seul calque (identifiant), même caché : par exemple la ligne de l'animation, sans le
    /// décor.
    Layer(u64),
}

/// Ce que la propagation fera, relevé sans rien lire de lourd (pour la fenêtre).
#[derive(Clone, Debug, PartialEq)]
pub struct Outline {
    pub guide: Guide,
    /// Images du plan refaites : la plage du guide (`span`), ou la partie choisie à la main.
    pub range: Range<usize>,
    /// Images clés du calque peint dans la plage.
    pub keys: Vec<usize>,
    /// Les clés hors de la plage : elles ne servent pas.
    pub ignored: Vec<usize>,
}

impl Outline {
    /// Nombre d'images à fabriquer.
    pub fn outputs(&self) -> usize {
        self.range.len() - self.keys.len()
    }
}

/// Quelles images de la plage fabriquer (Anthony, 2026-10-17 : « l'option de créer des images
/// uniquement sur les clés existantes ou de créer des images à différentes cadences d'images à
/// choisir, avec l'option de suivre les clés existantes ou non »). Les autres images de la plage
/// (hors clés peintes) tiennent l'image d'avant, comme une exposition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cadence {
    /// Seulement aux clés du guide : là où le dessin de ses calques change.
    pub keys_only: bool,
    /// Sinon une image toutes les `step` images (1 : toutes, 2 : en 2…).
    pub step: usize,
    /// Avec la cadence : une image aussi à chaque clé du guide, et la cadence repart de là.
    pub follow_keys: bool,
}

impl Default for Cadence {
    fn default() -> Self {
        Self { keys_only: false, step: 1, follow_keys: true }
    }
}

/// Les images à fabriquer dans `range`, dans l'ordre. `keys` : les clés peintes (jamais
/// fabriquées ; la cadence repart de chacune) ; `guide_keys` : les clés du guide (aucune pour une
/// vidéo, qui change à chaque image : « seulement aux clés » fabrique alors chaque image). Le
/// début de la plage est toujours une image (ou une clé), pour que les suivantes aient quoi tenir.
pub fn made_frames(range: Range<usize>, keys: &[usize], guide_keys: Option<&[usize]>, c: Cadence) -> Vec<usize> {
    let mut out = Vec::new();
    match guide_keys {
        Some(gk) if c.keys_only => {
            for f in range.clone() {
                if (f == range.start || gk.contains(&f)) && !keys.contains(&f) {
                    out.push(f);
                }
            }
        }
        _ => {
            let step = if c.keys_only { 1 } else { c.step.max(1) };
            let gk = guide_keys.unwrap_or(&[]);
            let mut anchor = range.start;
            for f in range.clone() {
                if f == range.start || keys.contains(&f) || (c.follow_keys && gk.contains(&f)) {
                    anchor = f;
                }
                if (f - anchor) % step == 0 && !keys.contains(&f) {
                    out.push(f);
                }
            }
        }
    }
    out
}

/// Les images de `range` qui ne sont ni des clés peintes ni fabriquées : elles tiendront l'image
/// d'avant.
pub fn held_frames(range: Range<usize>, keys: &[usize], made: &[usize]) -> Vec<usize> {
    range.filter(|f| !keys.contains(f) && made.binary_search(f).is_err()).collect()
}

/// Les clés du guide : les images où le dessin d'un de ses calques commence ou change. Aucune
/// pour un guide vidéo (chaque image change).
pub fn guide_keys(doc: &Document, panel: usize, paint: usize, guide: Guide) -> Option<Vec<usize>> {
    if matches!(guide, Guide::Video(_)) || panel >= doc.panels.len() {
        return None;
    }
    let p = &doc.panels[panel];
    let mut keys: Vec<usize> = Vec::new();
    for li in guide_layers(doc, panel, paint, guide) {
        let frames = &p.layers[li].frames;
        keys.extend((0..frames.len()).filter(|&f| frames[f].is_some() && (f == 0 || frames[f - 1] != frames[f])));
    }
    keys.sort_unstable();
    keys.dedup();
    Some(keys)
}

/// D'où viennent les images du guide pendant la propagation.
pub enum Frames {
    /// Les images de la vidéo, une par image de la plage.
    Files(Vec<PathBuf>),
    /// Des calques composés : à demander au fil de l'interface (`render_guide`), qui seul touche
    /// au document.
    Rendered,
}

/// Tout ce qu'il faut pour propager, relevé sur le document au départ.
pub struct Plan {
    pub outline: Outline,
    pub frames: Frames,
    /// La peinture de chaque clé, dans le rectangle de travail.
    pub styles: Vec<Image>,
    /// Coin du rectangle de travail sur la feuille, et sa taille.
    pub origin: (usize, usize),
    pub size: (usize, usize),
}

impl Plan {
    /// L'image de la vidéo à l'image `f` du plan (guide vidéo).
    pub fn load_file(&self, f: usize) -> Result<Image, String> {
        let Frames::Files(files) = &self.frames else {
            return Err("ce guide n'est pas une vidéo".into());
        };
        let path = &files[f - self.outline.range.start];
        let im = image::open(path).map_err(|e| format!("{} : {e}", path.display()))?.to_rgb8();
        Ok(Image::new(im.width() as usize, im.height() as usize, 3, im.into_raw()))
    }
}

/// Le calque vidéo sous le calque `paint` (le plus proche), sinon le premier du plan.
pub fn video_layer(doc: &Document, panel: usize, paint: usize) -> Option<usize> {
    let layers = &doc.panels.get(panel)?.layers;
    (0..paint.min(layers.len())).rev().find(|&i| layers[i].video.is_some()).or_else(|| layers.iter().position(|l| l.video.is_some()))
}

/// Le guide proposé d'abord : le calque vidéo sous le calque peint s'il y en a un, sinon ce qu'on
/// voit dessous.
pub fn default_guide(doc: &Document, panel: usize, paint: usize) -> Guide {
    match video_layer(doc, panel, paint) {
        Some(vi) => Guide::Video(doc.panels[panel].layers[vi].id),
        None => Guide::Below,
    }
}

/// Les guides possibles pour le calque `paint`, avec leur nom : les calques vidéo, ce qu'on voit
/// dessous (s'il y a des calques dessous), chacun des autres calques.
pub fn guides(doc: &Document, panel: usize, paint: usize) -> Vec<(Guide, String)> {
    let Some(p) = doc.panels.get(panel) else { return Vec::new() };
    let mut out: Vec<(Guide, String)> = p.layers.iter().filter(|l| l.video.is_some()).map(|l| (Guide::Video(l.id), format!("La vidéo « {} »", l.name))).collect();
    if paint > 0 {
        out.push((Guide::Below, "Ce qu'on voit sous le calque (calques visibles dessous)".into()));
    }
    for (i, l) in p.layers.iter().enumerate() {
        if i != paint && l.video.is_none() && !l.folder {
            out.push((Guide::Layer(l.id), format!("Le calque « {} » seul", l.name)));
        }
    }
    out
}

/// Le calque d'identifiant `id` dans le plan `panel`.
fn layer_index(doc: &Document, panel: usize, id: u64) -> Option<usize> {
    doc.panels.get(panel)?.layers.iter().position(|l| l.id == id)
}

/// Les images que le guide `guide` couvre pour le calque peint `paint` du plan `panel` : celles où
/// la vidéo a une image, ou du début du plan à la dernière image des calques du guide. Erreur si
/// ce calque ne peut pas être propagé avec ce guide.
pub fn span(doc: &Document, panel: usize, paint: usize, guide: Guide) -> Result<Range<usize>, String> {
    let p = doc.panels.get(panel).ok_or("Plan introuvable.")?;
    let layer = p.layers.get(paint).ok_or("Calque introuvable.")?;
    if layer.video.is_some() {
        return Err("C'est un calque vidéo : peins sur un calque au-dessus, puis propage ce calque-là.".into());
    }
    if layer.folder || layer.kind != LayerKind::Bitmap {
        return Err("La propagation se fait sur un calque bitmap.".into());
    }
    match guide {
        Guide::Video(id) => {
            let clip = layer_index(doc, panel, id).and_then(|i| p.layers[i].video.as_ref()).ok_or("Ce calque vidéo n'est plus dans le plan.")?;
            let shown: Vec<usize> = (0..p.frame_count()).filter(|&f| clip.index_at(f, doc.fps).is_some()).collect();
            let (Some(&first), Some(&last)) = (shown.first(), shown.last()) else {
                return Err("La vidéo ne couvre aucune image de ce plan.".into());
            };
            Ok(first..last + 1)
        }
        Guide::Below if paint == 0 => return Err("Rien sous ce calque : choisis un autre guide.".into()),
        Guide::Layer(id) if layer_index(doc, panel, id).is_none() => return Err("Ce calque n'est plus dans le plan.".into()),
        Guide::Layer(id) if layer_index(doc, panel, id) == Some(paint) => {
            return Err("Le calque peint ne peut pas se guider lui-même.".into())
        }
        // Les calques du guide, jusqu'à leur dernière image (au-delà, le guide ne bouge plus).
        Guide::Below | Guide::Layer(_) => {
            let end = guide_layers(doc, panel, paint, guide)
                .iter()
                .map(|&li| {
                    let l = &p.layers[li];
                    l.video.as_ref().map_or(l.frames.len(), |v| v.end(doc.fps))
                })
                .max()
                .unwrap_or(0)
                .min(p.frame_count());
            if end == 0 {
                return Err("Le guide est vide : rien à suivre.".into());
            }
            Ok(0..end)
        }
    }
}

/// La propagation du calque `paint` du plan `panel` en suivant `guide`, sur les images `chosen`
/// (dans la plage du guide ; aucune : toute cette plage), est-elle possible, et que fera-t-elle ?
pub fn outline(doc: &Document, panel: usize, paint: usize, guide: Guide, chosen: Option<Range<usize>>) -> Result<Outline, String> {
    let span = span(doc, panel, paint, guide)?;
    let range = match chosen {
        Some(c) => c.start.max(span.start)..c.end.min(span.end),
        None => span.clone(),
    };
    if range.is_empty() {
        return Err("La plage choisie est hors des images que le guide couvre.".into());
    }
    // Les clés du calque sur toute la plage du guide (une exposition commencée avant la plage
    // choisie n'est pas une clé), puis celles de la plage choisie.
    let layer = &doc.panels[panel].layers[paint];
    let marked: Vec<usize> = span.clone().filter(|&f| layer.roles.get(&f) == Some(&Role::Key) && layer.cel_at(f).is_some()).collect();
    let all: Vec<usize> = if marked.is_empty() {
        span.clone().filter(|&f| layer.cel_at(f).is_some() && (f == span.start || layer.cel_at(f - 1) != layer.cel_at(f))).collect()
    } else {
        marked
    };
    if all.is_empty() {
        return Err("Peins au moins une image clé sur ce calque.".into());
    }
    let (keys, ignored): (Vec<usize>, Vec<usize>) = all.into_iter().partition(|f| range.contains(f));
    if keys.is_empty() {
        return Err("Aucune clé dans la plage choisie : élargis-la jusqu'à une clé, ou peins une clé dedans.".into());
    }
    let outline = Outline { guide, range, keys, ignored };
    if outline.outputs() == 0 {
        return Err("Toutes les images sont des clés : rien à propager.".into());
    }
    Ok(outline)
}

/// Relève tout ce qu'il faut pour propager le calque `paint` du plan `panel` en suivant `guide`,
/// sur les images `chosen` (aucune : toute la plage du guide).
pub fn plan(doc: &Document, panel: usize, paint: usize, guide: Guide, chosen: Option<Range<usize>>) -> Result<Plan, String> {
    let outline = outline(doc, panel, paint, guide, chosen)?;
    let p = &doc.panels[panel];
    let (frames, origin, size) = match guide {
        Guide::Video(id) => {
            let clip = layer_index(doc, panel, id).and_then(|i| p.layers[i].video.as_ref()).expect("calque vidéo");
            let dir = video::dir_of(clip.id).ok_or("Les images de la vidéo sont introuvables.")?;
            let files: Vec<PathBuf> = outline.range.clone().map(|f| video::frame_file(&dir, clip.index_at(f, doc.fps).unwrap_or(0), &clip.ext)).collect();
            let (w, h) = image::image_dimensions(&files[0]).map_err(|e| format!("{} : {e}", files[0].display()))?;
            let size = (w as usize, h as usize);
            (Frames::Files(files), plan_origin(doc, size), size)
        }
        // Les calques composés : le cadre (la caméra au repos filme la feuille pixel pour pixel).
        Guide::Below | Guide::Layer(_) => {
            let frame = doc.frame_rect();
            (Frames::Rendered, (frame.min.x as usize, frame.min.y as usize), (doc.frame[0], doc.frame[1]))
        }
    };
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
        return Err("Les clés sont vides dans le rectangle de travail : rien à propager.".into());
    }
    Ok(Plan { outline, frames, styles, origin, size })
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

/// Les calques composés pour le guide : ceux sous le calque peint qui se voient (« dessous »), ou
/// le calque choisi, même caché.
fn guide_layers(doc: &Document, panel: usize, paint: usize, guide: Guide) -> Vec<usize> {
    let p = &doc.panels[panel];
    match guide {
        Guide::Below => (0..paint.min(p.layers.len())).filter(|&li| crate::folders::effect(&p.layers, li).is_some()).collect(),
        Guide::Video(id) | Guide::Layer(id) => layer_index(doc, panel, id).into_iter().collect(),
    }
}

/// Les dessins à composer pour le guide `guide` à l'image `frame`, comme les compose l'affichage
/// (`render::panel_plan_at` : transformation et défilement multiplan, effets, CTG, vidéo), mais
/// seulement pour les calques du guide.
fn guide_items(doc: &Document, panel: usize, paint: usize, guide: Guide, frame: usize) -> Vec<PlanItem> {
    let p = &doc.panels[panel];
    let camera = doc.camera_seen(panel, frame);
    let item = |li: usize, opacity: f32| {
        let l = &p.layers[li];
        let t = l.transform_seen(&camera)?;
        video::cel_at(l, frame, doc.fps).map(|cel| PlanItem {
            cel,
            opacity: (opacity * 255.0).round() as u8,
            tint: None,
            transform: (!t.is_identity()).then_some(t),
            blend: l.blend,
            fx: crate::fx::steps_at(&l.fx, frame),
            ctg: crate::ctg::input(&p.layers, li, frame),
        })
    };
    match guide {
        // Les calques sous le calque peint, cachés exceptés (l'opacité des dossiers comptée).
        Guide::Below => (0..paint.min(p.layers.len())).filter_map(|li| item(li, crate::folders::effect(&p.layers, li)?)).collect(),
        Guide::Video(id) | Guide::Layer(id) => layer_index(doc, panel, id).and_then(|li| item(li, 1.0)).into_iter().collect(),
    }
}

/// L'image du guide `guide` à l'image `frame` du plan (guides « dessous » et « calque ») : les
/// calques composés sur le papier, dans le cadre, en RVB.
pub fn render_guide(doc: &Document, panel: usize, paint: usize, guide: Guide, frame: usize) -> Image {
    let plan = render::Plan { paper: Some(doc.paper), items: guide_items(doc, panel, paint, guide, frame) };
    let pixels = render::camera_image(doc, &plan, &doc.default_camera());
    let data = pixels.iter().flat_map(|c| [c.r(), c.g(), c.b()]).collect();
    Image::new(doc.frame[0], doc.frame[1], 3, data)
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

/// Opacité en dessous de laquelle un pixel fabriqué compte pour vide (moins de 1 % : le bruit du
/// mélange ne remplit pas de tuiles pour rien).
const NOISE: u8 = 3;

/// Une image fabriquée (RVBA prémultiplié) posée en `origin` sur une feuille `width` × `height` ;
/// les zones transparentes (ou presque, `NOISE`) restent sans tuile.
pub fn to_raster(width: usize, height: usize, im: &Image, origin: (usize, usize)) -> Raster {
    let mut r = Raster::new(width, height);
    for y in 0..im.height {
        for x in 0..im.width {
            let p = &im.data[(y * im.width + x) * 4..][..4];
            let (sx, sy) = (origin.0 + x, origin.1 + y);
            if p[3] < NOISE || sx >= width || sy >= height {
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

/// Propage (sur le fil appelant) : `load(f)` donne l'image du guide à l'image `f` du plan ;
/// `done(f, image fabriquée)` reçoit chaque image hors clés ; tout s'arrête dès que `cancel`
/// passe à vrai.
pub fn run(
    plan: &Plan,
    synth: &dyn Synthesizer,
    params: &PropagateParams,
    cancel: &AtomicBool,
    load: impl FnMut(usize) -> Result<Image, String>,
    mut done: impl FnMut(usize, Image),
) -> Result<(), String> {
    let o = &plan.outline;
    let keys: Vec<Key> = o.keys.iter().zip(&plan.styles).map(|(&frame, style)| Key { frame, style }).collect();
    propagate_keys_on(synth, load, &keys, o.range.clone(), params, |f, im| {
        if !o.keys.contains(&f) {
            done(f, im.clone());
        }
        !cancel.load(Ordering::Relaxed)
    })
    .map_err(|e| e.to_string())
}

/// Pose les images fabriquées `made` dans le calque `paint` du plan `panel` (des dessins neufs) et
/// marque ses clés « Clé » : celles de la plage, et aussi celles hors plage (`Outline::ignored`),
/// sinon une prochaine propagation, ne voyant plus que les clés marquées, les perdrait.
/// (L'appelant prend l'instantané pour l'annulation.) Pelure passe par `apply_rasters`.
#[cfg(test)]
pub fn apply(doc: &mut Document, panel: usize, paint: usize, origin: (usize, usize), keys: &[usize], made: Vec<(usize, Image)>) {
    let (w, h) = (doc.width, doc.height);
    let rasters = made.into_iter().map(|(f, im)| (f, to_raster(w, h, &im, origin))).collect();
    apply_rasters(doc, panel, paint, keys, rasters);
}

/// `apply` avec des images déjà converties en dessins (`to_raster`, sur le fil de la tâche, dès
/// qu'elles arrivent : seules les zones peintes prennent de la place en mémoire).
pub fn apply_rasters(doc: &mut Document, panel: usize, paint: usize, keys: &[usize], made: Vec<(usize, Raster)>) {
    for (f, raster) in made {
        let cel = doc.add_cel(Cel::bitmap(raster));
        doc.panels[panel].layers[paint].set(f, Some(cel));
    }
    let layer = &mut doc.panels[panel].layers[paint];
    for &k in keys {
        layer.roles.insert(k, Role::Key);
    }
}

/// Les images `held` (dans l'ordre) du calque `paint` tiennent le dessin de l'image d'avant (une
/// cadence plus lente que l'image par image, `Cadence`) ; à poser après `apply`.
pub fn hold(doc: &mut Document, panel: usize, paint: usize, held: &[usize]) {
    let layer = &mut doc.panels[panel].layers[paint];
    for &f in held {
        if f > 0 {
            let before = layer.frames.get(f - 1).copied().flatten();
            layer.set(f, before);
        }
    }
}
