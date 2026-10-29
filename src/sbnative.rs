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
/// Taille de la feuille, en cadres.
const SHEET_FACTOR: usize = 6;
/// Unités du chemin de la caméra, en champs (x, y, z) : calées sur les vignettes de Storyboard Pro
/// (plan 1 de BRAWLIDAYS_CLEAN_PART_1, deux poses) ; à confirmer sur d'autres projets.
const CAM_UNIT: [f64; 3] = [0.366, 0.274, 0.52];

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
    // Les décors de Storyboard Pro débordent largement le cadre (plusieurs dizaines de champs) : feuille 6 fois plus grande.
    doc.enlarge_stage(w * SHEET_FACTOR, h * SHEET_FACTOR);
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
        out.push(format!("{} zone(s) sans couleur (traits de construction ou de crayon sans épaisseur lue) non reprises.", n.skipped_zones));
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
    for e in &l.effects {
        *notes.effects.entry(e.clone()).or_default() += 1;
    }
    if l.animated {
        notes.animated += 1;
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
        if sy < 0 || sy >= doc.height as i64 {
            continue;
        }
        for x in 0..bm.width {
            let sx = dx + bm.x as i64 + x as i64;
            if sx < 0 || sx >= doc.width as i64 {
                continue;
            }
            let o = (y * bm.width + x) * 4;
            let p = &bm.rgba[o..o + 4];
            if p[3] == 0 {
                continue;
            }
            let (sx, sy) = (sx as usize, sy as usize);
            let idx = out.tile_index_of(sx, sy);
            out.tile_mut(idx)[(sy % TILE) * TILE + sx % TILE] = premultiply(p[0], p[1], p[2], p[3]);
        }
    }
    out
}

/// Caméra de chaque plan : le `PEG` de la caméra en champs (x droite, y haut), z en distance de la
/// caméra au plan du dessin (12 champs + z : le zoom est 12 / (12 + z)). À vérifier.
fn cameras(doc: &mut Document, shots: &[SbShot], field: f32, warnings: &mut Vec<String>) {
    let (cx, cy) = (doc.width as f32 / 2.0, doc.height as f32 / 2.0);
    let mut moved = 0;
    for (i, shot) in shots.iter().enumerate() {
        let Some(panel) = doc.panels.get_mut(i) else { break };
        if shot.camera.is_empty() {
            continue;
        }
        let n = panel.frame_count();
        let mut keys: Vec<CameraKey> = Vec::new();
        for k in &shot.camera {
            let z = (CAMERA_Z / (CAMERA_Z - k.pos[2] * CAM_UNIT[2]).max(1.0)) as f32;
            let cam = Camera { x: cx + (k.pos[0] * CAM_UNIT[0]) as f32 * field, y: cy - (k.pos[1] * CAM_UNIT[1]) as f32 * field, zoom: z, ..Camera::centered(doc_w(cx), doc_w(cy)) };
            let frame = k.frame.min(n.saturating_sub(1));
            if keys.last().is_none_or(|l| l.frame < frame) {
                keys.push(CameraKey::new(frame, cam));
            }
        }
        if keys.len() > 1 {
            moved += 1;
        }
        panel.camera = keys;
    }
    let _ = (moved, warnings);
}

fn doc_w(half: f32) -> usize {
    (half * 2.0) as usize
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
        assert!((p.camera[1].cam.x - p.camera[0].cam.x - 2.0 * 0.366 * 12.0).abs() < 1e-2);
        assert_eq!(p.camera[1].frame, 9);
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
}
