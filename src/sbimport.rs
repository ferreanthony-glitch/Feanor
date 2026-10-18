//! Import d'un projet Storyboard Pro (14a) : le projet lu (`sboard`) devient un projet Pelure.
//! - chaque plan devient un plan, dans une séquence du même nom que la sienne ;
//! - ses panneaux sont mis bout à bout (la conversion en plans de 13a) : le début de chacun est
//!   une image de storyboard qui garde ses légendes (Dialog → dialogue, Action Notes → action,
//!   les autres → notes) ;
//! - ses calques gardent leur nom ; leurs dessins viennent des images exportées par le script
//!   (`sbscript`), une par calque et par panneau ; deux images identiques ne font qu'un dessin ;
//! - l'opacité des calques est dans ces images (Storyboard Pro les exporte avec) : les calques
//!   Pelure restent à 100 % ;
//! - les pistes son gardent leurs extraits ; un extrait au volume, au tempo ou à la vitesse
//!   changés devient son propre son, calculé une fois (`stretch`).

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Arc;

use crate::audio::{self, Pcm, RATE};
use crate::document::{Cel, CelId, Document, Layer, LayerKind};
use crate::raster::{premultiply, Raster, TILE};
use crate::sboard::{SbClip, SbPanel, SbProject};
use crate::sbscript::{DPanel, Drawings};
use crate::stretch;

/// Construit le projet. `project_dir` : dossier du `.sboard` (ses sons sont dans `audio`).
/// `progress` reçoit l'avancement (0 à 1). Renvoie aussi ce qui n'a pas pu être repris.
pub fn build(sb: &SbProject, project_dir: &Path, drawings: Option<&Drawings>, progress: &dyn Fn(f32)) -> (Document, Vec<String>) {
    let mut warnings = sb.warnings.clone();
    let [w, h] = sb.size;
    let mut doc = Document::with_frame(w, h, sb.fps).created_now();
    doc.sequences.clear();
    doc.scenes.clear();
    doc.panels.clear();
    doc.properties.title = sb.title.clone();

    let total: usize = sb.shots.iter().map(|s| s.panels.len()).sum();
    let matched = drawings.map(|d| match_panels(sb, d, &mut warnings));
    let mut cels: HashMap<u64, CelId> = HashMap::new();
    let mut stats = VectorStats::default();
    let no_palette = HashMap::new();
    let palette = drawings.map_or(&no_palette, |d| &d.palette);
    let mut seq: Option<(String, u64)> = None;
    let mut g = 0;
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
            let dp = matched.as_ref().and_then(|m| m[g]);
            let mut panel = doc.make_panel(scene);
            panel.length = p.frames;
            panel.dialogue = p.dialogue.clone();
            panel.action = p.action.clone();
            panel.notes = p.notes.clone();
            let layers = panel_layers(&mut doc, p, dp, palette, &mut cels, &mut warnings, &mut stats, &shot.name);
            if !layers.is_empty() {
                panel.layers = layers;
            }
            doc.panels.push(panel);
            g += 1;
            progress(0.8 * g as f32 / total.max(1) as f32);
        }
    }
    if stats.tried > stats.kept {
        let mut w = format!(
            "Dessins vectoriels : {} calque(s) sur {} gardé(s) en vecteurs ; les autres sont en images (traits illisibles, ou qui ne recouvrent pas l'image exportée).",
            stats.kept, stats.tried
        );
        if !stats.notes.is_empty() {
            w += &format!(" Script : {}", stats.notes.join(" ; "));
        }
        warnings.push(w);
    }
    warnings.extend(measure_note(&stats.measures));
    warnings.extend(doc.into_plans());
    add_sounds(&mut doc, sb, &project_dir.join("audio"), &mut warnings);
    progress(1.0);
    (doc, warnings)
}

/// Le panneau du script de chaque panneau du projet, dans l'ordre du film : par identifiant,
/// sinon (aucun identifiant commun) par rang.
fn match_panels<'a>(sb: &SbProject, d: &'a Drawings, warnings: &mut Vec<String>) -> Vec<Option<&'a DPanel>> {
    let panels: Vec<&SbPanel> = sb.shots.iter().flat_map(|s| &s.panels).collect();
    let by_id: HashMap<&str, &DPanel> = d.panels.iter().map(|p| (p.id.as_str(), p)).collect();
    let by_id_count = panels.iter().filter(|p| by_id.contains_key(p.id.as_str())).count();
    let out: Vec<Option<&DPanel>> = if by_id_count > 0 {
        panels.iter().map(|p| by_id.get(p.id.as_str()).copied()).collect()
    } else {
        if d.panels.len() != panels.len() {
            warnings.push(format!(
                "Dessins : le script a exporté {} panneaux, le projet en a {} ; ils sont associés dans l'ordre du film.",
                d.panels.len(),
                panels.len()
            ));
        }
        (0..panels.len()).map(|i| d.panels.get(i)).collect()
    };
    let mut different = 0;
    for (p, dp) in panels.iter().zip(&out) {
        if dp.is_some_and(|dp| dp.frames != 0 && dp.frames != p.frames) {
            different += 1;
        }
    }
    let missing = out.iter().filter(|o| o.is_none()).count();
    if missing > 0 {
        warnings.push(format!("Dessins : {missing} panneau(x) sans images exportées (projet modifié depuis l'export ?)."));
    }
    if different > 0 {
        warnings.push(format!("Dessins : {different} panneau(x) d'une autre durée que dans l'export (projet modifié depuis ?)."));
    }
    out
}

/// Calques d'un panneau, du fond au premier plan (l'ordre de Pelure). Avec les images du
/// script : ses calques, rangés comme dans le projet ; sans : les calques du projet, vides.
fn panel_layers(
    doc: &mut Document,
    p: &SbPanel,
    dp: Option<&DPanel>,
    palette: &HashMap<String, [f32; 4]>,
    cels: &mut HashMap<u64, CelId>,
    warnings: &mut Vec<String>,
    stats: &mut VectorStats,
    shot: &str,
) -> Vec<Layer> {
    let mut layers = Vec::new();
    match dp {
        Some(dp) => {
            // Rang de chaque calque dans le projet (du premier plan au fond) ; ceux qu'il ne
            // connaît pas passent devant, dans l'ordre du script.
            let rank = |name: &str| p.layers.iter().position(|l| l.name == name);
            let mut list: Vec<_> = dp.layers.iter().enumerate().collect();
            list.sort_by_key(|(i, l)| (rank(&l.name).map_or(0, |r| r + 1), *i));
            for (_, dl) in list.into_iter().rev() {
                // Un calque vectoriel de Storyboard Pro reste vectoriel (14a-2) : ses traits s'ils
                // ont pu être lus et calés sur son image, sinon il garde l'image (calque bitmap).
                let kind = if dl.vector { LayerKind::Vector } else { LayerKind::Bitmap };
                let mut layer = Layer::new(dl.name.clone(), kind);
                layer.visible = dl.visible;
                layer.locked = dl.locked;
                if let Some(file) = &dl.file {
                    match cel_from_export(doc, file, dl.traits.as_deref(), palette, cels, &mut stats.measures) {
                        Ok((cel, vector)) => {
                            layer.frames = vec![Some(cel); p.frames];
                            layer.kind = if vector { LayerKind::Vector } else { LayerKind::Bitmap };
                            if dl.vector {
                                stats.tried += 1;
                                if vector {
                                    stats.kept += 1;
                                } else if stats.notes.len() < 3 && !dl.note.is_empty() && !stats.notes.contains(&dl.note) {
                                    stats.notes.push(dl.note.clone());
                                }
                            }
                        }
                        Err(e) => warnings.push(format!("Plan {shot}, calque « {} » : {e}", dl.name)),
                    }
                }
                layers.push(layer);
            }
        }
        None => {
            for l in p.layers.iter().rev() {
                let kind = if l.vector { LayerKind::Vector } else { LayerKind::Bitmap };
                layers.push(Layer::new(l.name.clone(), kind));
            }
        }
    }
    layers
}

/// Bilan des dessins vectoriels : calques essayés, gardés en vecteurs, et quelques notes du
/// script sur ceux qui n'ont pas pu l'être.
#[derive(Default)]
struct VectorStats {
    tried: usize,
    kept: usize,
    notes: Vec<String>,
    /// Calage de chaque dessin gardé en vecteurs.
    measures: Vec<Measure>,
}

/// Calage d'un dessin vectoriel sur son image (`sbvector::Fit`) : pixels du cadre par unité
/// de Storyboard Pro, sens de l'axe vertical, origine (0, 0) par rapport au centre du cadre.
struct Measure {
    scale: f32,
    y_up: bool,
    origin: [f32; 2],
}

/// La mesure retenue (médiane des calages) en une phrase, pour l'export des vecteurs (14b-2).
fn measure_note(m: &[Measure]) -> Option<String> {
    if m.is_empty() {
        return None;
    }
    let median = |mut v: Vec<f32>| {
        v.sort_by(f32::total_cmp);
        v[v.len() / 2]
    };
    let s = median(m.iter().map(|x| x.scale).collect());
    let ox = median(m.iter().map(|x| x.origin[0]).collect());
    let oy = median(m.iter().map(|x| x.origin[1]).collect());
    let up = m.iter().filter(|x| x.y_up).count() * 2 >= m.len();
    Some(format!(
        "Mesure des dessins vectoriels ({} calage(s)) : 1 unité de Storyboard Pro = {s:.4} pixel, axe vertical vers le {}, origine à ({ox:.1}, {oy:.1}) pixels du centre du cadre.",
        m.len(),
        if up { "haut" } else { "bas" }
    ))
}

/// Le dessin d'un calque exporté (le même pour deux exports identiques) : vectoriel si ses
/// traits ont pu être lus et calés sur l'image (`sbvector`), sinon l'image. Vrai : vectoriel.
fn cel_from_export(
    doc: &mut Document,
    png: &Path,
    traits: Option<&Path>,
    palette: &HashMap<String, [f32; 4]>,
    cels: &mut HashMap<u64, CelId>,
    measures: &mut Vec<Measure>,
) -> Result<(CelId, bool), String> {
    let bytes = std::fs::read(png).map_err(|e| format!("image illisible ({e})"))?;
    let traits_text = traits.and_then(|t| std::fs::read(t).ok());
    let mut h = DefaultHasher::new();
    bytes.hash(&mut h);
    traits_text.hash(&mut h);
    let key = h.finish();
    if let Some(&c) = cels.get(&key) {
        let vector = doc.cels.get(&c).is_some_and(|c| c.vector.is_some());
        return Ok((c, vector));
    }
    let img = image::load_from_memory(&bytes).map_err(|e| format!("image illisible ({e})"))?.to_rgba8();
    let raster = place_image(doc, img);
    let fitted = traits_text
        .and_then(|t| serde_json::from_slice::<serde_json::Value>(&t).ok())
        .map(|json| crate::sbvector::raw_elements(&json, palette))
        .filter(|raw| !raw.is_empty())
        .and_then(|raw| crate::sbvector::fit_to_image(&raw, &raster));
    let (cel, vector) = match fitted {
        Some(fit) => {
            let c = doc.frame_rect().center();
            measures.push(Measure { scale: fit.scale, y_up: fit.y_up, origin: [fit.origin[0] - c.x, fit.origin[1] - c.y] });
            let mut cel = Cel { raster: Raster::new(doc.width, doc.height), vector: Some(crate::vector::VectorDrawing { elements: fit.elements }), version: 0, fond: None };
            cel.rerender();
            (cel, true)
        }
        None => (Cel::bitmap(raster), false),
    };
    let id = doc.add_cel(cel);
    cels.insert(key, id);
    Ok((id, vector))
}

/// Une image au centre du cadre ; à la taille du cadre si elle a ses proportions (un export à
/// une autre résolution).
pub fn place_image(doc: &Document, img: image::RgbaImage) -> Raster {
    let [fw, fh] = doc.frame;
    let (iw, ih) = (img.width() as usize, img.height() as usize);
    let same_ratio = iw > 0 && ih > 0 && ((iw * fh) as f64 / (ih * fw) as f64 - 1.0).abs() < 0.01;
    let img = if (iw, ih) != (fw, fh) && same_ratio {
        image::imageops::resize(&img, fw as u32, fh as u32, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let (iw, ih) = (img.width() as i64, img.height() as i64);
    let frame = doc.frame_rect();
    let x0 = frame.min.x as i64 + (frame.width() as i64 - iw) / 2;
    let y0 = frame.min.y as i64 + (frame.height() as i64 - ih) / 2;
    let mut out = Raster::new(doc.width, doc.height);
    for (y, row) in img.rows().enumerate() {
        let sy = y0 + y as i64;
        if sy < 0 || sy >= doc.height as i64 {
            continue;
        }
        for (x, p) in row.enumerate() {
            let sx = x0 + x as i64;
            if sx < 0 || sx >= doc.width as i64 || p[3] == 0 {
                continue;
            }
            let (sx, sy) = (sx as usize, sy as usize);
            let idx = out.tile_index_of(sx, sy);
            out.tile_mut(idx)[(sy % TILE) * TILE + sx % TILE] = premultiply(p[0], p[1], p[2], p[3]);
        }
    }
    out
}

/// Pistes et extraits de son.
fn add_sounds(doc: &mut Document, sb: &SbProject, audio_dir: &Path, warnings: &mut Vec<String>) {
    // Chaque fichier n'est lu qu'une fois : (son de Pelure, son décodé) ou rien s'il manque.
    let mut sources: HashMap<String, Option<(u64, Arc<Pcm>)>> = HashMap::new();
    let fps = doc.fps.max(1) as f64;
    for track in &sb.tracks {
        let t = doc.audio_tracks.len();
        doc.add_audio_track();
        doc.audio_tracks[t].name = track.name.clone();
        doc.audio_tracks[t].muted = track.muted;
        for clip in &track.clips {
            let label = format!("Piste {}, « {} » (image {})", track.name, clip.file, clip.start);
            if clip.to <= clip.from {
                warnings.push(format!("{label} : l'extrait commence après la fin du son, il est sauté."));
                continue;
            }
            let source = sources.entry(clip.file.clone()).or_insert_with(|| load_sound(doc, audio_dir, &clip.file, warnings)).clone();
            let Some((sound, pcm)) = source else { continue };
            if clip.envelope {
                warnings.push(format!("{label} : son enveloppe de volume n'est pas reprise (volume de départ gardé)."));
            }
            if !clip.other_effects.is_empty() {
                warnings.push(format!("{label} : effets non repris ({}).", clip.other_effects.join(", ")));
            }
            let start = clip.start.saturating_sub(1);
            let plain = (clip.tempo - 1.0).abs() < 1e-6 && (clip.speed - 1.0).abs() < 1e-6 && (clip.volume - 1.0).abs() < 1e-4;
            if plain {
                let offset = (clip.from * fps).round() as usize;
                let seconds = clip.to.min(pcm.seconds()) - clip.from;
                let available = (seconds * fps).floor().max(1.0) as usize;
                doc.add_clip(t, sound, start, offset, clip.frames().min(available));
            } else {
                let samples = processed_clip(&pcm, clip, fps);
                if samples.is_empty() {
                    warnings.push(format!("{label} : l'extrait est vide, il est sauté."));
                    continue;
                }
                let length = ((samples.len() as f64 / RATE as f64) * fps).round().max(1.0) as usize;
                let (pcm2, bytes) = stretch::to_sound(&samples);
                let stem = Path::new(&clip.file).file_stem().map_or_else(|| clip.file.clone(), |s| s.to_string_lossy().into_owned());
                let id = doc.add_sound(format!("{stem} (extrait Storyboard Pro).wav"), "wav".into(), Arc::new(bytes), Arc::new(pcm2));
                doc.add_clip(t, id, start, 0, length.min(clip.frames()));
            }
        }
    }
}

/// Le son entendu pendant l'extrait : la partie du fichier qu'il joue, à sa vitesse et à son
/// volume.
pub fn processed_clip(pcm: &Pcm, clip: &SbClip, fps: f64) -> Vec<[f32; 2]> {
    let seconds = (clip.frames() as f64 / fps).min(clip.to - clip.from);
    if seconds <= 0.0 {
        return Vec::new();
    }
    let factor = clip.tempo * clip.speed;
    let rate = RATE as f64;
    let a = (clip.from * factor * rate) as usize;
    // Un peu plus que nécessaire : le découpage du WSOLA rogne la fin.
    let b = ((clip.from + seconds) * factor * rate).ceil() as usize + 4096;
    let mut s = stretch::slice(pcm, a, b);
    s = stretch::resample(&s, clip.speed);
    s = stretch::wsola(&s, clip.tempo);
    s.truncate((seconds * rate).round() as usize);
    stretch::gain(&mut s, clip.volume);
    s
}

fn load_sound(doc: &mut Document, audio_dir: &Path, file: &str, warnings: &mut Vec<String>) -> Option<(u64, Arc<Pcm>)> {
    let path = audio_dir.join(file);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(_) => {
            warnings.push(format!("Son « {file} » introuvable dans le dossier audio du projet : ses extraits sont sautés."));
            return None;
        }
    };
    let ext = path.extension().map_or_else(String::new, |e| e.to_string_lossy().to_ascii_lowercase());
    match audio::decode(&bytes, &ext) {
        Ok(pcm) => {
            let pcm = Arc::new(pcm);
            let id = doc.add_sound(file.to_string(), ext, Arc::new(bytes), pcm.clone());
            Some((id, pcm))
        }
        Err(e) => {
            warnings.push(format!("Son « {file} » illisible ({e}) : ses extraits sont sautés."));
            None
        }
    }
}
