//! Import d'un projet Storyboard Pro sans Storyboard Pro (import 71, 2026-10-29) : les dessins sont
//! lus directement dans `elements/<dossier>/<élément>-<dessin>.tvg` (`tvg`), la caméra, les calques
//! et le son dans le `.sboard` (`sboard`). Rien n'est écrit dans le dossier du projet.
//!
//! - un plan Storyboard Pro devient un Plan SHOT ; ses panneaux deviennent des clés de dessin sur
//!   les calques du plan (un calque par nom), chacune avec sa marque d'image (la case des
//!   Vignettes) : `Document::into_plans` puis `cles_de_case` ;
//! - dessins vectoriels : zones colorées (`Element::Fill`), une par suite de zones de même couleur
//!   dans un calque plan ; calques bitmap : image posée au centre de la feuille (toile de 3840×2160
//!   de Storyboard Pro = feuille de SHOT, 1 pixel pour 1 pixel) ;
//! - placement du calque (décalage en champs, échelle, rotation) : cuit dans les points ;
//! - caméra : clés de position du `PEG` de la caméra (champs → pixels, z → zoom), à vérifier.
//! Tout ce qui n'est pas repris est dit dans le compte rendu (`warnings`).

use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use crate::camera::{Camera, CameraKey};
use crate::document::{Cel, Document, Layer, LayerKind};
use crate::raster::{premultiply, Raster, TILE};
use crate::sboard::{SbLayer, SbProject, SbShot};
use crate::tvg::{self, TvgDrawing};
use crate::vector::{Element, VectorDrawing};

/// Unités de dessin par champ (un champ : 1/9 de la hauteur du cadre).
pub const UNITS_PER_FIELD: f32 = 576.0;
/// Distance de la caméra au plan du dessin, en champs.
const CAMERA_Z: f64 = 12.0;

/// Unités du chemin de la caméra de Storyboard Pro, en champs (un champ : un neuvième de la hauteur du cadre) : z de la caméra,
/// x, y (calés sur les vignettes de Storyboard Pro, sbnative::tests::calage_descente : x 0,366 sur les deux projets ; y vaut les
/// trois quarts de x) et z des calques (profondeur de leur offset). La caméra est un sténopé à 12 champs du plan z = 0.
const CALAGE_DEFAUT: [f32; 4] = [0.45, 0.366, 0.2745, 1.1];

/// Les réglages en vigueur : ceux du calage ; les essais de calage (module de tests) les font varier.
fn calage() -> [f32; 4] {
    #[cfg(test)]
    {
        *CALAGE.lock().unwrap()
    }
    #[cfg(not(test))]
    {
        CALAGE_DEFAUT
    }
}

#[cfg(test)]
static CALAGE: std::sync::Mutex<[f32; 4]> = std::sync::Mutex::new(CALAGE_DEFAUT);

/// Pixels de flou de SHOT par unité de lurriness de Storyboard Pro (calé sur les vignettes).
const FLOU_PAR_UNITE: f32 = 10.0;

fn flou_par_unite() -> f32 {
    #[cfg(test)]
    {
        std::env::var("SBP_KB").ok().and_then(|v| v.parse().ok()).unwrap_or(FLOU_PAR_UNITE)
    }
    #[cfg(not(test))]
    {
        FLOU_PAR_UNITE
    }
}

/// Placement d'un calque sur la feuille.
#[derive(Clone, Copy)]
struct Place {
    /// Centre de la feuille, en pixels.
    center: [f32; 2],
    /// Pixels par champ.
    field: f32,
    offset: [f32; 2],
    scale: f32,
    angle: f32,
}

impl Place {
    fn key(&self) -> [u32; 4] {
        [self.offset[0].to_bits(), self.offset[1].to_bits(), self.scale.to_bits(), self.angle.to_bits()]
    }

    /// Point du dessin (unités) → pixel de la feuille.
    fn map(&self, p: [f32; 2]) -> [f32; 2] {
        let (s, c) = self.angle.to_radians().sin_cos();
        let (x, y) = (p[0] * self.scale, p[1] * self.scale);
        let (x, y) = (x * c - y * s + self.offset[0] * UNITS_PER_FIELD, x * s + y * c + self.offset[1] * UNITS_PER_FIELD);
        let k = self.field / UNITS_PER_FIELD;
        [self.center[0] + x * k, self.center[1] - y * k]
    }
}

/// Ce qui est compté en route pour le compte rendu.
#[derive(Default)]
struct Notes {
    missing: Vec<String>,
    skipped_zones: usize,
    unreadable: Vec<String>,
    effects: BTreeMap<String, usize>,
    animated: usize,
    bitmap_placed: usize,
    empty_vectors: usize,
}

/// Construit le projet SHOT. `dir` : dossier du `.sboard` (`elements`, `audio`).
pub fn build(sb: &SbProject, dir: &Path, progress: &dyn Fn(f32)) -> (Document, Vec<String>) {
    let mut warnings = sb.warnings.clone();
    let [w, h] = sb.size;
    let mut doc = Document::with_frame(w, h, sb.fps).created_now();
    // Les décors de Storyboard Pro débordent largement le cadre (plusieurs dizaines de champs) : on garde la feuille du projet
    // (deux fois le cadre, ce que la carte graphique sait montrer) ; ce qui la dépasse va sur la toile infinie, hors de la feuille.
    doc.sequences.clear();
    doc.scenes.clear();
    doc.panels.clear();
    doc.properties.title = sb.title.clone();
    let place0 = Place { center: [doc.width as f32 / 2.0, doc.height as f32 / 2.0], field: h as f32 / 9.0, offset: [0.0, 0.0], scale: 1.0, angle: 0.0 };

    let total: usize = sb.shots.iter().map(|s| s.panels.len()).sum();
    let mut notes = Notes::default();
    let mut cels: HashMap<u64, crate::document::CelId> = HashMap::new();
    let mut seq: Option<(String, u64)> = None;
    let mut done = 0;
    for shot in &sb.shots {
        let seq_id = match &seq {
            Some((name, id)) if *name == shot.sequence => *id,
            _ => {
                let id = doc.new_sequence();
                if let Some(s) = doc.sequence_mut(id) {
                    s.name = shot.sequence.clone();
                }
                seq = Some((shot.sequence.clone(), id));
                id
            }
        };
        let scene = doc.new_scene(seq_id);
        if let Some(s) = doc.scene_mut(scene) {
            s.name = shot.name.clone();
        }
        for p in &shot.panels {
            let mut panel = doc.make_panel(scene);
            panel.length = p.frames;
            panel.dialogue = p.dialogue.clone();
            panel.action = p.action.clone();
            panel.notes = p.notes.clone();
            let mut layers = Vec::new();
            // Du fond au premier plan : l'ordre de SHOT.
            for l in p.layers.iter().rev() {
                layers.push(layer(&mut doc, sb, dir, l, p.frames, place0, &mut cels, &mut notes, &shot.name));
            }
            // Groupes de Storyboard Pro : un dossier de calques SHOT (nom, visibilité), au-dessus de ses calques.
            let mut groups: Vec<(String, bool)> = Vec::new();
            for l in p.layers.iter().rev() {
                if !l.group.is_empty() && !groups.iter().any(|g| g.0 == l.group) {
                    groups.push((l.group.clone(), l.group_off));
                }
            }
            let mut folders: Vec<(usize, Layer)> = Vec::new();
            for (name, off) in groups {
                let mut folder = Layer::new_folder(name.clone());
                folder.visible = !off;
                let mut top = 0;
                for (k, l) in p.layers.iter().rev().enumerate() {
                    if l.group == name {
                        layers[k].parent = Some(folder.id);
                        top = k;
                    }
                }
                folders.push((top + 1, folder));
            }
            // (De haut en bas : les rangs d'insertion restent justes.)
            folders.sort_by_key(|f| std::cmp::Reverse(f.0));
            for (at, folder) in folders {
                layers.insert(at, folder);
            }            crate::folders::normalize(&mut layers);
            if !layers.is_empty() {
                panel.layers = layers;
            }
            doc.panels.push(panel);
            done += 1;
            progress(0.85 * done as f32 / total.max(1) as f32);
        }
    }
    report(&notes, &mut warnings);
    warnings.extend(doc.into_plans());
    // Une clé de dessin à chaque case, même quand le dessin ne change pas d'un panneau à l'autre.
    for i in 0..doc.panels.len() {
        let frames: Vec<usize> = doc.panels[i].marks.iter().map(|m| m.frame).collect();
        for f in frames {
            doc.cles_de_case(i, f);
        }
    }
    cameras(&mut doc, &sb.shots, place0.field, &mut warnings);
    crate::sbimport::add_sounds(&mut doc, sb, &dir.join("audio"), &mut warnings);
    progress(1.0);
    (doc, warnings)
}

fn report(n: &Notes, out: &mut Vec<String>) {
    if !n.missing.is_empty() {
        out.push(format!("Dessins introuvables dans le dossier « elements » : {} (calques laissés vides) ; par exemple {}.", n.missing.len(), n.missing.iter().take(3).cloned().collect::<Vec<_>>().join(", ")));
    }
    if !n.unreadable.is_empty() {
        out.push(format!("Dessins illisibles : {} ; par exemple {}.", n.unreadable.len(), n.unreadable.iter().take(3).cloned().collect::<Vec<_>>().join(", ")));
    }
    if n.skipped_zones > 0 {
        out.push(format!("{} ligne(s) médiane(s) sans couleur ni épaisseur (zones ouvertes) non reprises ; les contours fermés sans couleur sont repris comme trous.", n.skipped_zones));
    }
    for (e, c) in &n.effects {
        out.push(format!("Effet de calque non repris : {e} ({c} calque(s))."));
    }
    if n.animated > 0 {
        out.push(format!("{} calque(s) ont un réglage animé par une courbe (décalage, échelle, rotation, opacité) ou incliné : seule leur valeur fixe est reprise.", n.animated));
    }
}

#[allow(clippy::too_many_arguments)]
fn layer(
    doc: &mut Document,
    sb: &SbProject,
    dir: &Path,
    l: &SbLayer,
    frames: usize,
    base: Place,
    cels: &mut HashMap<u64, crate::document::CelId>,
    notes: &mut Notes,
    shot: &str,
) -> Layer {
    let kind = if l.vector { LayerKind::Vector } else { LayerKind::Bitmap };
    let mut layer = Layer::new(l.name.clone(), kind);
    layer.visible = l.visible;
    layer.locked = l.locked;
    layer.opacity = l.opacity;
    // Multiplan : le calque de profondeur z (en champs, plus grand = plus près) est à la distance 12 - z de la caméra de repos ;
    // dans SHOT, profondeur d = -z / 12, dessin rapetissé de 1 / (1 + d) (Storyboard Pro ne grossit pas un calque lointain).
    let depth = (-(l.z as f32) * calage()[3] / CAMERA_Z as f32).max(Layer::MIN_DEPTH);
    layer.depth = depth;
    layer.transform.scale = 1.0 / (1.0 + depth);
    for e in &l.effects {
        *notes.effects.entry(e.clone()).or_default() += 1;
    }
    if l.animated || l.blur_animated {
        notes.animated += 1;
    }
    // Flou gaussien de Storyboard Pro : l'effet « Flou gaussien » de SHOT (rayon en pixels de la feuille).
    if let Some(b) = l.blur.filter(|b| *b > 0.0) {
        let mut fx = crate::fx::Effect::new(crate::fx::EffectKind::GaussianBlur);
        fx.values[0] = (b * flou_par_unite()).clamp(0.0, 100.0);
        layer.fx.push(fx);
    }
    if l.drawing.is_empty() {
        return layer;
    }
    let (folder, name) = sb.elements.get(&l.element).cloned().unwrap_or_else(|| ("Draw".into(), "Draw".into()));
    let path: PathBuf = dir.join("elements").join(&folder).join(format!("{name}-{}.tvg", l.drawing));
    let place = Place { offset: l.offset, scale: if l.scale == 0.0 { 1.0 } else { l.scale }, angle: l.angle, ..base };
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (&path, place.key(), l.vector).hash(&mut h);
    let key = h.finish();
    if let Some(&c) = cels.get(&key) {
        layer.frames = vec![Some(c); frames];
        return layer;
    }
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(_) => {
            notes.missing.push(format!("{folder}/{name}-{} (plan {shot})", l.drawing));
            return layer;
        }
    };
    let drawing = match tvg::read(&bytes) {
        Ok(d) => d,
        Err(e) => {
            notes.unreadable.push(format!("{name}-{} ({e})", l.drawing));
            return layer;
        }
    };
    notes.skipped_zones += drawing.skipped;
    let cel = if let (Some(bm), false) = (&drawing.bitmap, l.vector && !drawing.shapes.is_empty()) {
        layer.kind = LayerKind::Bitmap;
        notes.bitmap_placed += 1;
        Cel::bitmap(bitmap_raster(doc, bm, &place))
    } else {
        layer.kind = LayerKind::Vector;
        let mut cel = Cel { raster: Raster::new(doc.width, doc.height), vector: Some(vector_drawing(&drawing, &place)), version: 0, fond: None };
        if cel.vector.as_ref().is_some_and(|v| v.elements.is_empty()) {
            notes.empty_vectors += 1;
        }
        cel.rerender();
        cel
    };
    let id = doc.add_cel(cel);
    cels.insert(key, id);
    layer.frames = vec![Some(id); frames];
    layer
}

/// Les zones d'un dessin : une zone remplie par suite de zones de même couleur dans un même calque
/// plan (pair-impair : un trou est un contour intérieur).
fn vector_drawing(d: &TvgDrawing, place: &Place) -> VectorDrawing {
    let mut elements: Vec<Element> = Vec::new();
    let mut last: Option<(usize, u64)> = None;
    for s in &d.shapes {
        let contour: Vec<[f32; 2]> = s.ring.iter().map(|&p| place.map(p)).collect();
        if s.hole {
            // Contour fermé sans couleur : un trou de la zone qui le précède dans le même calque plan.
            if last.is_some_and(|l| l.0 == s.layer) {
                if let Some(Element::Fill { contours, .. }) = elements.last_mut() {
                    contours.push(contour);
                }
            }
            continue;
        }
        if last == Some((s.layer, s.color)) {
            if let Some(Element::Fill { contours, .. }) = elements.last_mut() {
                contours.push(contour);
                continue;
            }
        }
        let c = d.palette.get(&s.color).copied().unwrap_or([0, 0, 0, 255]);
        if c[3] == 0 {
            last = None;
            continue;
        }
        elements.push(Element::Fill { contours: vec![contour], color: [c[0], c[1], c[2]], opacity: c[3] as f32 / 255.0, gradient: None });
        last = Some((s.layer, s.color));
    }
    VectorDrawing { elements }
}

fn bitmap_raster(doc: &Document, bm: &tvg::TvgBitmap, place: &Place) -> Raster {
    let mut out = Raster::new(doc.width, doc.height);
    let dx = (place.center[0] + place.offset[0] * place.field).round() as i64;
    let dy = (place.center[1] - place.offset[1] * place.field).round() as i64;
    for y in 0..bm.height {
        let sy = dy + bm.y as i64 + y as i64;
        if sy.abs() > crate::raster::WORLD {
            continue;
        }
        for x in 0..bm.width {
            let sx = dx + bm.x as i64 + x as i64;
            if sx.abs() > crate::raster::WORLD {
                continue;
            }
            let o = (y * bm.width + x) * 4;
            let p = &bm.rgba[o..o + 4];
            if p[3] == 0 {
                continue;
            }
            // Toile infinie : un pixel hors de la feuille va dans une tuile hors-feuille.
            let t = TILE as i64;
            let idx = out.id_of(sx.div_euclid(t) as i32, sy.div_euclid(t) as i32);
            out.tile_mut(idx)[(sy.rem_euclid(t) as usize) * TILE + sx.rem_euclid(t) as usize] = premultiply(p[0], p[1], p[2], p[3]);
        }
    }
    out
}

/// Caméra de chaque plan : le `PEG` de la caméra est un sténopé à la distance 12 champs du plan z = 0 (FOV vertical de 41,11°
/// = 9 champs de haut à cette distance). x, y en champs (x droite, y haut), z : la caméra recule de z champs. Dans SHOT :
/// zoom 1, avancée (`dolly`) négative de z × (distance de référence / 12), de sorte que le plan z = 0 est grossi de 12 / (12 + z)
/// et un calque de profondeur z_l de 12 / (12 + z - z_l) (`Layer::depth`, voir `layer`).
fn cameras(doc: &mut Document, shots: &[SbShot], field: f32, warnings: &mut Vec<String>) {
    let (cx, cy) = (doc.width as f32 / 2.0, doc.height as f32 / 2.0);
    let d0 = crate::camera::ref_distance(doc.frame);
    let [kz, kx, ky, _] = calage();
    let mut moved = 0;
    for (i, shot) in shots.iter().enumerate() {
        let Some(panel) = doc.panels.get_mut(i) else { break };
        if shot.camera.is_empty() {
            continue;
        }
        let n = panel.frame_count();
        let mut keys: Vec<CameraKey> = Vec::new();
        for k in &shot.camera {
            let cam = Camera {
                x: cx + k.pos[0] as f32 * field * kx,
                y: cy - k.pos[1] as f32 * field * ky,
                zoom: 1.0,
                // Rotation du PEG : x bascule (positif : la caméra regarde en haut), y panoramique (positif : à gauche dans Storyboard Pro,
                // donc signe changé), z roulis (anti-horaire dans Storyboard Pro, horaire dans SHOT).
                angle: -(k.rot[2] as f32),
                dolly: -(k.pos[2] as f32) * kz * d0 / CAMERA_Z as f32,
                tilt: k.rot[0] as f32,
                pan: -(k.rot[1] as f32),
            };
            let frame = k.frame.min(n.saturating_sub(1));
            if keys.last().is_none_or(|l| l.frame < frame) {
                keys.push(CameraKey { ease_in: k.ease_in, ease_out: k.ease_out, ..CameraKey::new(frame, cam) });
            }
        }
        if keys.len() > 1 {
            moved += 1;
        }
        panel.camera = keys;
    }
    let _ = (moved, warnings);
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tvg::fabrique::{palette, tvg, zone};

    const XML: &str = r#"<?xml version="1.0"?>
<project version="2700">
 <options><resolution size="192,108"/><framerate val="24"/></options>
 <elements><element id="1" elementName="Draw" elementFolder="Draw"/></elements>
 <scenes>
  <scene name="Top" id="top" nbframes="10"><columns><column type="0" name="T"><warpSeq exposures="1-10" id="s1"/></column></columns></scene>
  <scene name="shot" id="s1" nbframes="10">
   <columns>
    <column type="0" name="S"><warpSeq exposures="1-4" id="p1"/><warpSeq exposures="5-10" id="p2"/></column>
    <column type="2" name="PATH"><path3D><points><pt val="0,0,0" lockedInTime="1"/><pt val="2,1,0" lockedInTime="10"/></points></path3D></column>
   </columns>
   <rootgroup name="Top"><nodeslist>
    <module type="PEG" name="Camera-Peg"><attrs><position><attr3dpath col="PATH"/><x val="0"/><y val="0"/><z val="0"/></position></attrs></module>
   </nodeslist></rootgroup>
   <metas><meta type="sceneInfo"><sceneInfo name="1" sequenceName=""/></meta></metas>
  </scene>
  <scene name="panel" id="p1"><columns><column type="0" name="C1"><elementSeq exposures="1-4" val="1" id="1"/></column></columns>
   <rootgroup name="Top"><nodeslist>
    <module type="READ" name="A"><attrs><opacity val="100"/><lineArtDrawingMode val="VectorDrawingMode"/><drawing><element col="C1"/></drawing></attrs></module>
   </nodeslist></rootgroup></scene>
  <scene name="panel2" id="p2"><columns><column type="0" name="C2"><elementSeq exposures="1-6" val="1" id="1"/></column></columns>
   <rootgroup name="Top"><nodeslist>
    <module type="READ" name="A"><attrs><opacity val="100"/><lineArtDrawingMode val="VectorDrawingMode"/><drawing><element col="C2"/></drawing></attrs></module>
   </nodeslist></rootgroup></scene>
 </scenes>
 <timeline><scene id="top"/></timeline>
</project>"#;

    #[test]
    fn panels_become_keys_with_marks_drawing_and_camera() {
        let dir = std::env::temp_dir().join(format!("shot-test-sbnative-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join("elements/Draw")).unwrap();
        let sq = [(-288.0, -288.0, true), (288.0, -288.0, true), (288.0, 288.0, true), (-288.0, 288.0, true)];
        std::fs::write(dir.join("elements/Draw/Draw-1.tvg"), tvg(&[zone(7, &sq)], &palette(7, [200, 0, 0, 255]))).unwrap();
        let sb = crate::sboard::parse(XML, "essai").unwrap();
        assert_eq!(sb.shots[0].camera.len(), 2);
        let (doc, warnings) = build(&sb, &dir, &|_| {});
        std::fs::remove_dir_all(&dir).ok();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!((doc.width, doc.height), (384, 216), "la feuille garde la taille du projet (deux fois le cadre) : le décor qui dépasse va sur la toile infinie");
        let mut grand = Document::with_frame(192, 108, 24);
        grand.enlarge_stage(40_000, 40_000);
        assert!(grand.width <= crate::document::MAX_SHEET_SIDE && grand.height <= crate::document::MAX_SHEET_SIDE, "une feuille ne dépasse jamais la texture minimale de la carte graphique");
        assert_eq!(doc.panels.len(), 1, "un plan SBP = un Plan");
        let p = &doc.panels[0];
        assert_eq!(p.frame_count(), 10);
        assert_eq!(p.marks.iter().map(|m| m.frame).collect::<Vec<_>>(), vec![4], "une case par panneau");
        let l = &p.layers[0];
        let (a, b) = (l.cel_at(0).unwrap(), l.cel_at(4).unwrap());
        assert_ne!(a, b, "une clé de dessin propre à chaque panneau");
        assert!(!doc.cels[&a].is_empty());
        // Caméra : deux clés, x en pixels (2 unités de 0,366 champ de 12 pixels).
        assert_eq!(p.camera.len(), 2);
        assert!((p.camera[1].cam.x - p.camera[0].cam.x - 2.0 * 12.0 * CALAGE_DEFAUT[1]).abs() < 1e-2);
        assert_eq!(p.camera[1].frame, 9);
    }

    const XML2: &str = r#"<?xml version="1.0"?>
<project version="2700">
 <options><resolution size="192,108"/><framerate val="24"/></options>
 <elements><element id="1" elementName="Draw" elementFolder="Draw"/></elements>
 <scenes>
  <scene name="Top" id="top" nbframes="10"><columns><column type="0" name="T"><warpSeq exposures="1-10" id="s1"/></column></columns></scene>
  <scene name="shot" id="s1" nbframes="10">
   <columns>
    <column type="0" name="S"><warpSeq exposures="1-10" id="p1"/></column>
    <column type="2" name="PATH"><path3D><points><pt val="0,0,0" lockedInTime="1"/><pt val="2,1,12" lockedInTime="10"/></points></path3D>
     <velocity type="3"><points version="2">
      <pt x="1" y="0" rx="4" ry="0" lx="1" ly="0"/><pt x="10" y="1" rx="10" ry="1" lx="6" ly="1"/></points></velocity></column>
    <column type="8" name="ROT"><path3D><points><pt val="0,0,0" lockedInTime="1"/><pt val="10,20,30" lockedInTime="10"/></points></path3D></column>
   </columns>
   <rootgroup name="Top"><nodeslist>
    <module type="PEG" name="Camera-Peg"><attrs><position><attr3dpath col="PATH"/><x val="0"/><y val="0"/><z val="0"/></position><rotation><quaternionpath col="ROT"/></rotation></attrs></module>
   </nodeslist></rootgroup>
   <metas><meta type="sceneInfo"><sceneInfo name="1" sequenceName=""/></meta></metas>
  </scene>
  <scene name="panel" id="p1"><columns><column type="0" name="C1"><elementSeq exposures="1-10" val="1" id="1"/></column></columns>
   <rootgroup name="Top"><nodeslist>
    <module type="PEG" name="Groupe"><options><disabled val="true"/></options><attrs/></module>
    <module type="GAUSSIANBLUR-PLUGIN" name="Flou"><attrs><blurriness val="3"/></attrs></module>
    <module type="READ" name="A"><attrs><offset><x val="0"/><y val="0"/><z val="6"/></offset><opacity val="100"/><lineArtDrawingMode val="VectorDrawingMode"/><drawing><element col="C1"/></drawing></attrs></module>
   </nodeslist><linkedlist><link out="Groupe" in="A"/><link out="A" in="Flou"/></linkedlist></rootgroup></scene>
 </scenes>
 <timeline><scene id="top"/></timeline>
</project>"#;

    #[test]
    fn camera_rotation_eases_depth_group_and_blur() {
        let dir = std::env::temp_dir().join(format!("shot-test-sbnative2-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join("elements/Draw")).unwrap();
        let sq = |h: f32| [(-h, -h, true), (h, -h, true), (h, h, true), (-h, h, true), (-h, -h, true)];
        // Un carré noir avec un trou (contour fermé sans couleur) et une ligne ouverte sans couleur.
        let open = [(0.0, 0.0, true), (10.0, 0.0, true), (20.0, 5.0, true)];
        let bytes = tvg(&[zone(7, &sq(288.0)), crate::tvg::fabrique::zone_sans_couleur(&sq(100.0)), crate::tvg::fabrique::zone_sans_couleur(&open)], &palette(7, [0, 0, 0, 255]));
        std::fs::write(dir.join("elements/Draw/Draw-1.tvg"), bytes).unwrap();
        let sb = crate::sboard::parse(XML2, "essai").unwrap();
        let k = &sb.shots[0].camera;
        assert_eq!(k.len(), 2);
        assert_eq!((k[0].ease_out, k[1].ease_in), (3.0, 4.0), "poignées plates de la courbe de vitesse");
        assert_eq!(k[1].rot, [10.0, 20.0, 30.0]);
        let (doc, warnings) = build(&sb, &dir, &|_| {});
        std::fs::remove_dir_all(&dir).ok();
        let p = &doc.panels[0];
        let (c0, c1) = (p.camera[0], p.camera[1]);
        assert_eq!((c1.cam.tilt, c1.cam.pan, c1.cam.angle), (10.0, -20.0, -30.0), "bascule, panoramique et roulis");
        assert_eq!((c0.ease_out, c1.ease_in), (3.0, 4.0));
        let [kz, ..] = CALAGE_DEFAUT;
        assert!(c1.cam.dolly < 0.0 && (c1.cam.dolly + 12.0 * kz * crate::camera::ref_distance(doc.frame) / 12.0).abs() < 1e-2, "la caméra recule de z champs");
        // Calque à z = 6 : profondeur négative (plus près), dessin rapetissé de 1 / (1 + profondeur).
        let folder = p.layers.iter().find(|l| l.folder).expect("le groupe devient un dossier");
        assert_eq!(folder.name, "Groupe");
        assert!(!folder.visible, "groupe désactivé : dossier caché");
        let a = p.layers.iter().find(|l| l.name == "A").unwrap();
        assert_eq!(a.parent, Some(folder.id));
        assert!((a.depth + 6.0 * CALAGE_DEFAUT[3] / 12.0).abs() < 1e-6);
        assert!((a.transform.scale * (1.0 + a.depth) - 1.0).abs() < 1e-6);
        // Flou : 3 unités.
        assert_eq!(a.fx.len(), 1);
        assert!((a.fx[0].values[0] - 3.0 * FLOU_PAR_UNITE).abs() < 1e-4);
        // Le trou est repris (le carré est creux), la ligne ouverte est dite.
        let cel = &doc.cels[&a.cel_at(0).unwrap()];
        let centre = cel.raster.get(doc.width / 2, doc.height / 2);
        assert_eq!(centre.a(), 0, "trou au centre");
        assert!(warnings.iter().any(|w| w.contains("ligne(s) médiane(s)")), "{warnings:?}");
    }
    #[test]
    fn missing_drawings_are_reported_not_lost() {
        let dir = std::env::temp_dir().join(format!("shot-test-sbnative-vide-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let sb = crate::sboard::parse(XML, "essai").unwrap();
        let (doc, warnings) = build(&sb, &dir, &|_| {});
        std::fs::remove_dir_all(&dir).ok();
        assert!(warnings.iter().any(|w| w.contains("introuvables")), "{warnings:?}");
        assert_eq!(doc.panels[0].frame_count(), 10);
    }

    /// Essai à la main sur une COPIE d'un projet : `SBP_ESSAI=<dossier> cargo test --release --offline sbnative::tests::projet_reel -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn projet_reel() {
        let Some(dir) = std::env::var_os("SBP_ESSAI").map(PathBuf::from) else { return };
        let sboard = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x == "sboard")).unwrap();
        let sb = crate::sboard::read(&sboard).unwrap();
        let t0 = std::time::Instant::now();
        let (doc, warnings) = build(&sb, &dir, &|_| {});
        println!("import : {:.1} s", t0.elapsed().as_secs_f32());
        let panels: usize = sb.shots.iter().map(|s| s.panels.len()).sum();
        let sbp_frames: usize = sb.shots.iter().flat_map(|s| &s.panels).map(|p| p.frames).sum();
        let frames: usize = doc.panels.iter().map(|p| p.frame_count()).sum();
        let marks: usize = doc.panels.iter().map(|p| p.marks.len() + 1).sum();
        let (mut cases, mut inked) = (0, 0);
        for p in &doc.panels {
            let mut at: Vec<usize> = vec![0];
            at.extend(p.marks.iter().map(|m| m.frame));
            for f in at {
                cases += 1;
                if p.layers.iter().any(|l| l.visible && l.cel_at(f).and_then(|c| doc.cels.get(&c)).is_some_and(|c| !c.is_empty())) {
                    inked += 1;
                }
            }
        }
        let clips: usize = doc.audio_tracks.iter().map(|t| t.clips.len()).sum();
        let cams = doc.panels.iter().filter(|p| !p.camera.is_empty()).count();
        println!("plans SBP {} -> SHOT {} ; panneaux SBP {panels} -> cases {marks} ; images SBP {sbp_frames} -> SHOT {frames} (fps {})", sb.shots.len(), doc.panels.len(), doc.fps);
        println!("cases avec dessin visible : {inked} / {cases} ; plans avec caméra : {cams} ; pistes {} extraits {clips}", doc.audio_tracks.len());
        for w in &warnings {
            println!("remarque : {w}");
        }
        assert_eq!(frames, sbp_frames);
    }

    /// Calage de la caméra sur les vignettes de Storyboard Pro (à la main, sur une COPIE) : rend chaque image composée dont
    /// on a la vignette (`SBP_VIGNETTES` : dossier de `NNNN.png` et `thumbs.txt`) et écrit côte à côte vignette / rendu de
    /// SHOT dans `SBP_SORTIE`, avec l'écart moyen de luminance.
    #[test]
    #[ignore]
    fn calage_vignettes() {
        let (Some(dir), Some(vign), Some(out)) = (std::env::var_os("SBP_ESSAI").map(PathBuf::from), std::env::var_os("SBP_VIGNETTES").map(PathBuf::from), std::env::var_os("SBP_SORTIE").map(PathBuf::from)) else { return };
        std::fs::create_dir_all(&out).unwrap();
        let sboard = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x == "sboard")).unwrap();
        let sb = crate::sboard::read(&sboard).unwrap();
        let (doc, _) = build(&sb, &dir, &|_| {});
        let only: Option<Vec<String>> = std::env::var("SBP_PLANS").ok().map(|s| s.split(',').map(str::to_string).collect());
        let mut total = (0.0f64, 0);
        for line in std::fs::read_to_string(vign.join("thumbs.txt")).unwrap().lines() {
            let line = line.trim_start_matches('\u{feff}');
            let parts: Vec<&str> = line.splitn(2, ' ').collect();
            let f: Vec<&str> = parts[1].split('|').collect();
            if f[0] != "#0" || f.len() < 5 {
                continue;
            }
            let Some(i) = sb.shots.iter().position(|s| s.id == f[2]) else { continue };
            if only.as_ref().is_some_and(|o| !o.contains(&i.to_string())) {
                continue;
            }
            let frame = f[4].trim_start_matches('#').parse::<usize>().unwrap() - 1;
            let mini = crate::comparer::petit_projet(&doc, &doc.panels[i]);
            let ours = crate::comparer::rendre(&mini, frame, 300);
            let theirs = image::open(vign.join(format!("{}.png", parts[0].trim()))).unwrap_or_else(|e| panic!("{} : {e}", parts[0])).to_rgba8();
            let (w, h) = (theirs.width() as usize, theirs.height() as usize);
            let lum = |r: u8, g: u8, b: u8| (r as i32 * 299 + g as i32 * 587 + b as i32 * 114) / 1000;
            let (mut sum, mut n) = (0i64, 0i64);
            let mut side = image::RgbaImage::new((w * 2) as u32, h as u32);
            for y in 0..h {
                for x in 0..w {
                    let p = theirs.get_pixel(x as u32, y as u32);
                    side.put_pixel(x as u32, y as u32, *p);
                    let c = ours.pixels[(y * ours.size[1] / h).min(ours.size[1] - 1) * ours.size[0] + (x * ours.size[0] / w).min(ours.size[0] - 1)];
                    side.put_pixel((w + x) as u32, y as u32, image::Rgba([c.r(), c.g(), c.b(), 255]));
                    sum += (lum(p[0], p[1], p[2]) - lum(c.r(), c.g(), c.b())).abs() as i64;
                    n += 1;
                }
            }
            let mae = sum as f64 / n as f64;
            total = (total.0 + mae, total.1 + 1);
            side.save(out.join(format!("p{i:02}-f{:04}-{}.png", frame + 1, parts[0]))).unwrap();
            println!("plan {i} image {} vignette {} : écart {mae:.1}", frame + 1, parts[0]);
        }
        println!("écart moyen : {:.2} sur {} images", total.0 / total.1.max(1) as f64, total.1);
    }
    /// Écart moyen (masses d'encre, blocs de 6 pixels) entre les vignettes `cibles` et le rendu de SHOT avec les réglages `p`.
    fn ecart_calage(doc: &mut Document, sb: &SbProject, field: f32, cibles: &[(usize, usize, image::RgbaImage)], p: [f32; 4]) -> f64 {
        *CALAGE.lock().unwrap() = p;
        cameras(doc, &sb.shots, field, &mut Vec::new());
        for (pi, shot) in sb.shots.iter().enumerate() {
            for l in doc.panels[pi].layers.iter_mut() {
                if let Some(sl) = shot.panels.iter().flat_map(|p| &p.layers).find(|s| s.name == l.name) {
                    let d = (-(sl.z as f32) * p[3] / CAMERA_Z as f32).max(Layer::MIN_DEPTH);
                    l.depth = d;
                    l.transform.scale = 1.0 / (1.0 + d);
                }
            }
        }
        let mut sum = 0.0;
        for (i, frame, theirs) in cibles {
            let mini = crate::comparer::petit_projet(doc, &doc.panels[*i]);
            let ours = crate::comparer::rendre(&mini, *frame, 300);
            let (w, h) = (theirs.width() as usize, theirs.height() as usize);
            let lum = |r: u8, g: u8, b: u8| (r as f32 * 0.299 + g as f32 * 0.587 + b as f32 * 0.114) / 255.0;
            let bloc = 6;
            let mut s = 0.0f32;
            for by in 0..h / bloc {
                for bx in 0..w / bloc {
                    let (mut a, mut c2) = (0.0f32, 0.0f32);
                    for y in by * bloc..(by + 1) * bloc {
                        for x in bx * bloc..(bx + 1) * bloc {
                            let q = theirs.get_pixel(x as u32, y as u32);
                            a += lum(q[0], q[1], q[2]);
                            let c = ours.pixels[(y * ours.size[1] / h).min(ours.size[1] - 1) * ours.size[0] + (x * ours.size[0] / w).min(ours.size[0] - 1)];
                            c2 += lum(c.r(), c.g(), c.b());
                        }
                    }
                    s += (a - c2).abs() / (bloc * bloc) as f32;
                }
            }
            sum += s as f64 / ((w / bloc) * (h / bloc)) as f64 * 100.0;
        }
        sum / cibles.len().max(1) as f64
    }

    /// Descente par coordonnées des réglages du calage sur les vignettes des plans `SBP_PLANS` (tous si absent) ; départ `SBP_DEPART="kz,kx,ky,kl"`.
    #[test]
    #[ignore]
    fn calage_descente() {
        let (Some(dir), Some(vign)) = (std::env::var_os("SBP_ESSAI").map(PathBuf::from), std::env::var_os("SBP_VIGNETTES").map(PathBuf::from)) else { return };
        let sboard = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x == "sboard")).unwrap();
        let sb = crate::sboard::read(&sboard).unwrap();
        let (mut doc, _) = build(&sb, &dir, &|_| {});
        let field = doc.frame[1] as f32 / 9.0;
        let only: Option<Vec<String>> = std::env::var("SBP_PLANS").ok().map(|s| s.split(',').map(str::to_string).collect());
        let mut cibles = Vec::new();
        for line in std::fs::read_to_string(vign.join("thumbs.txt")).unwrap().lines() {
            let line = line.trim_start_matches('\u{feff}');
            let parts: Vec<&str> = line.splitn(2, ' ').collect();
            let f: Vec<&str> = parts[1].split('|').collect();
            if f[0] != "#0" || f.len() < 5 {
                continue;
            }
            let Some(i) = sb.shots.iter().position(|s| s.id == f[2]) else { continue };
            if only.as_ref().is_some_and(|o| !o.contains(&i.to_string())) {
                continue;
            }
            let frame = f[4].trim_start_matches('#').parse::<usize>().unwrap() - 1;
            cibles.push((i, frame, image::open(vign.join(format!("{}.png", parts[0].trim()))).unwrap().to_rgba8()));
        }
        // Au plus SBP_MAX vignettes par plan (réparties), pour que l'essai reste court.
        let max: usize = std::env::var("SBP_MAX").ok().and_then(|v| v.parse().ok()).unwrap_or(usize::MAX);
        let mut gardees = Vec::new();
        let mut plans: Vec<usize> = cibles.iter().map(|c| c.0).collect();
        plans.dedup();
        for pl in plans {
            let de: Vec<_> = cibles.iter().filter(|c| c.0 == pl).collect();
            let n = de.len().min(max);
            for k in 0..n {
                let c = de[k * de.len() / n];
                gardees.push((c.0, c.1, c.2.clone()));
            }
        }
        let cibles = gardees;
        let mut p: [f32; 4] = std::env::var("SBP_DEPART").map_or([0.5, 0.366, 0.274, 1.0], |s| {
            let v: Vec<f32> = s.split(',').filter_map(|x| x.parse().ok()).collect();
            [v[0], v[1], v[2], v[3]]
        });
        let mut best = ecart_calage(&mut doc, &sb, field, &cibles, p);
        println!("départ {p:?} : {best:.2} sur {} images", cibles.len());
        let mut step = 0.1f32;
        while step > 0.004 {
            let mut moved = false;
            for k in 0..4 {
                for sign in [1.0f32, -1.0] {
                    let mut q = p;
                    q[k] += sign * step * p[k].abs().max(0.2);
                    let e = ecart_calage(&mut doc, &sb, field, &cibles, q);
                    if e < best - 1e-4 {
                        best = e;
                        p = q;
                        moved = true;
                        println!("  {p:?} : {best:.3}");
                        break;
                    }
                }
            }
            if !moved {
                step /= 2.0;
            }
        }
        println!("RESULTAT {p:?} : écart {best:.3}");
    }
    #[test]
    #[ignore]
    fn liste_cameras() {
        let Some(dir) = std::env::var_os("SBP_ESSAI").map(PathBuf::from) else { return };
        let sboard = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x == "sboard")).unwrap();
        let sb = crate::sboard::read(&sboard).unwrap();
        for (i, s) in sb.shots.iter().enumerate() {
            let zs: Vec<f32> = s.panels.iter().flat_map(|p| p.layers.iter().map(|l| l.z)).collect();
            let (zmin, zmax) = zs.iter().fold((0.0f32, 0.0f32), |a, &z| (a.0.min(z), a.1.max(z)));
            let cam: Vec<String> = s.camera.iter().map(|k| format!("f{}=({:.1},{:.1},{:.1})", k.frame, k.pos[0], k.pos[1], k.pos[2])).collect();
            println!("plan {i} {} : {} panneaux, calques z {zmin:.1}..{zmax:.1}, caméra {}", s.id, s.panels.len(), cam.join(" "));
        }
    }
    #[test]
    #[ignore]
    fn liste_calques() {
        let Some(dir) = std::env::var_os("SBP_ESSAI").map(PathBuf::from) else { return };
        let plan: usize = std::env::var("SBP_PLAN").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        let sboard = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x == "sboard")).unwrap();
        let sb = crate::sboard::read(&sboard).unwrap();
        // SBP_CHERCHE=<dessin> : liste les plans (et panneaux) dont un calque utilise ce dessin (« 152 » ou « 152_3 »).
        if let Ok(c) = std::env::var("SBP_CHERCHE") {
            for (i, s) in sb.shots.iter().enumerate() {
                for (k, p) in s.panels.iter().enumerate() {
                    for l in &p.layers {
                        let (folder, name) = sb.elements.get(&l.element).cloned().unwrap_or_default();
                        let chemin = format!("{folder}/{name}-{}", l.drawing);
                        if c == "?" && i == 7 && k == 0 {
                            println!("exemple {chemin}");
                        }
                        if !(chemin.contains(&c) || (c == "bitmap" && !l.vector)) {
                            continue;
                        }
                        println!("plan {i} panneau {k} ({} images) calque {} vecteur {} dessin {chemin}", p.frames, l.name, l.vector);
                    }
                }
            }
            return;
        }
        for p in sb.shots[plan].panels.iter().take(2) {
            println!("panneau {} ({} images)", p.id, p.frames);
            for l in &p.layers {
                println!("  {} vecteur {} visible {} opacité {} z {} off {:?} échelle {} angle {} effets {:?} animé {} dessin {}/{}", l.name, l.vector, l.visible, l.opacity, l.z, l.offset, l.scale, l.angle, l.effects, l.animated, l.element, l.drawing);
            }
        }
    }}
