//! « Propager » dans l'interface (propagation façon EbSynth, étape e). Trois temps :
//! - la fenêtre dit ce qui va se passer (guide, images, clés), propose le guide (calque vidéo,
//!   ce qu'on voit dessous, un calque choisi), la plage à refaire (toute celle du guide, à la
//!   main, ou l'entrée et la sortie de la timeline) et donne les réglages ;
//! - la tâche tourne sur un autre fil : progression, aperçu de la dernière image fabriquée,
//!   « Arrêter ». Les images d'un guide fait de calques sont composées ici, sur le fil de
//!   l'interface, à la demande de la tâche (le document ne quitte pas ce fil) ;
//! - à la fin, les dessins sont posés en une seule étape d'annulation.
//!
//! Le calcul se fait sur la carte graphique de l'affichage (le périphérique wgpu d'eframe,
//! partagé), sinon sur le processeur. Voir `propager.rs`.

use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use eframe::egui::{self, vec2, Align2};
use eframe::egui_wgpu::{wgpu, RenderState};
use synthese::propagation::PropagateParams;
use synthese::{Cpu, Image, Synthesizer};

use crate::app::App;
use crate::document::LayerKind;
use crate::propager::{self, Cadence, Frames, Guide};
use crate::raster::Raster;

/// Canaux de la synthèse : le style (RVBA), les guides (image 3, position 2, temporel 4).
const STYLE_CHANNELS: usize = 4;
const GUIDE_CHANNELS: usize = 9;

/// La propagation : réglages, fenêtre et tâche en cours.
pub(crate) struct Propagation {
    /// Le périphérique wgpu de l'affichage, s'il y en a un, et le nom de la carte.
    device: Option<(wgpu::Device, wgpu::Queue, String)>,
    window: bool,
    on_gpu: bool,
    /// Le guide choisi dans la fenêtre (aucun : celui proposé d'abord).
    guide: Option<Guide>,
    /// Les images à refaire choisies dans la fenêtre, et pour quel plan, calque et guide
    /// (identifiants) ; aucune : toute la plage du guide.
    range: Option<((u64, u64, Guide), Range<usize>)>,
    params: PropagateParams,
    /// Quelles images fabriquer : toutes, en 2, en 3…, ou seulement aux clés du guide.
    cadence: Cadence,
    job: Option<Job>,
}

/// Une propagation en cours.
struct Job {
    /// Plan et calque (identifiants : on les retrouve même si l'ordre a changé).
    panel: u64,
    layer: u64,
    guide: Guide,
    /// Les clés à marquer : celles de la plage et celles hors plage.
    keys: Vec<usize>,
    /// Les images qui tiendront l'image d'avant (cadence plus lente que l'image par image).
    held: Vec<usize>,
    total: usize,
    /// Images calculées (toutes celles hors clés, gardées ou non) : la progression.
    computed: Arc<AtomicUsize>,
    cancel: Arc<AtomicBool>,
    /// Les images fabriquées, déjà en dessins (seules les zones peintes prennent de la place), au
    /// fur et à mesure ; et la vignette de la dernière, pour l'aperçu.
    made: Arc<Mutex<Vec<(usize, Raster)>>>,
    latest: Arc<Mutex<Option<egui::ColorImage>>>,
    /// Images du guide demandées par la tâche (calques composés), et la réponse.
    requests: Receiver<usize>,
    answers: Sender<Result<Image, String>>,
    /// Le nom du calcul (carte ou processeur), ou l'erreur.
    handle: JoinHandle<Result<String, String>>,
    started: Instant,
    preview: Option<egui::TextureHandle>,
}

impl Propagation {
    pub(crate) fn new(rs: Option<&RenderState>) -> Propagation {
        let device = rs.map(|rs| {
            let info = rs.adapter.get_info();
            (rs.device.clone(), rs.queue.clone(), format!("{} ({:?})", info.name, info.backend))
        });
        Propagation { on_gpu: device.is_some(), device, window: false, guide: None, range: None, params: PropagateParams::default(), cadence: Cadence::default(), job: None }
    }

    /// Une propagation tourne : le reste de l'interface attend.
    pub(crate) fn running(&self) -> bool {
        self.job.is_some()
    }
}

/// Aperçu d'une image fabriquée (RVBA prémultiplié), réduite à `max` pixels de côté au plus (une
/// image très haute dépassait la taille de texture permise, très large donnait 0 de haut).
fn preview(im: &Image, max: usize) -> egui::ColorImage {
    let step = im.width.max(im.height).div_ceil(max).max(1);
    let (w, h) = ((im.width / step).max(1).min(im.width), (im.height / step).max(1).min(im.height));
    let mut data = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            data.extend_from_slice(&im.data[((y * step) * im.width + x * step) * 4..][..4]);
        }
    }
    egui::ColorImage::from_rgba_premultiplied([w, h], &data)
}

/// Un champ de numéro d'image dans `span`, numéroté comme la timeline : on glisse, ou on tape un
/// numéro (ou un time code).
fn frame_field<'a>(app: &'a App, value: &'a mut usize, span: &Range<usize>) -> egui::DragValue<'a> {
    egui::DragValue::new(value)
        .range(span.start..=span.end - 1)
        .speed(0.2)
        .custom_formatter(move |v, _| app.frame_label(v.max(0.0) as usize))
        .custom_parser(move |s| crate::cues::parse_frame(s, app.doc.start_frame, app.doc.fps, app.time_display).map(|f| f as f64))
}

impl App {
    /// Le guide de la propagation du calque courant : celui choisi s'il est encore possible, sinon
    /// celui proposé d'abord.
    fn propagation_guide(&self) -> Guide {
        let choices = propager::guides(&self.doc, self.cur_panel, self.cur_layer);
        match self.propagation.guide {
            Some(g) if choices.iter().any(|(c, _)| *c == g) => g,
            _ => propager::default_guide(&self.doc, self.cur_panel, self.cur_layer),
        }
    }

    /// La plage du guide `guide` pour le calque courant, et les images à refaire : celles choisies
    /// dans la fenêtre pour ce plan, ce calque et ce guide, ramenées dans la plage du guide ;
    /// sinon toute cette plage.
    fn propagation_ranges(&self, guide: Guide) -> Result<(Range<usize>, Range<usize>), String> {
        let span = propager::span(&self.doc, self.cur_panel, self.cur_layer, guide)?;
        let chosen = match &self.propagation.range {
            Some((at, r)) if *at == (self.panel().id, self.layer().id, guide) => r.start.max(span.start)..r.end.min(span.end),
            _ => span.clone(),
        };
        Ok(if chosen.is_empty() { (span.clone(), span) } else { (span, chosen) })
    }

    /// Action « calque.propager » : la fenêtre, si le calque courant peut être propagé.
    pub(crate) fn open_propagation(&mut self) {
        let layer = self.layer();
        if layer.video.is_some() || layer.folder || layer.kind != LayerKind::Bitmap {
            self.status = "La propagation se fait sur un calque bitmap (peint au-dessus d'une vidéo ou d'une animation).".into();
            return;
        }
        if propager::guides(&self.doc, self.cur_panel, self.cur_layer).is_empty() {
            self.status = "Rien à suivre : il faut un autre calque dans le plan (vidéo, animation…).".into();
            return;
        }
        self.propagation.window = true;
    }

    /// Dans le menu d'une ligne de calque : « Propager… », pour un calque bitmap d'un plan qui a
    /// d'autres calques à suivre.
    pub(crate) fn propagation_menu(&mut self, ui: &mut egui::Ui, li: usize) {
        let paintable = self.layers().get(li).is_some_and(|l| l.video.is_none() && !l.folder && l.kind == LayerKind::Bitmap);
        if paintable && !propager::guides(&self.doc, self.cur_panel, li).is_empty() {
            ui.separator();
            self.action_item(ui, "Propager la peinture…", "calque.propager");
        }
    }

    /// À chaque image d'affichage : la fenêtre, ou la tâche en cours.
    pub(crate) fn propagation_ui(&mut self, ctx: &egui::Context) {
        if self.propagation.job.is_some() {
            self.propagation_progress(ctx);
        } else if self.propagation.window {
            self.propagation_window(ctx);
        }
    }

    fn propagation_window(&mut self, ctx: &egui::Context) {
        let guide = self.propagation_guide();
        let choices = propager::guides(&self.doc, self.cur_panel, self.cur_layer);
        let ranges = self.propagation_ranges(guide);
        let outline = ranges.clone().and_then(|(_, r)| propager::outline(&self.doc, self.cur_panel, self.cur_layer, guide, Some(r)));
        let labels = |frames: &[usize]| frames.iter().map(|&k| self.frame_label(k)).collect::<Vec<_>>().join(", ");
        // Les clés du guide (aucune pour une vidéo) et la cadence choisie : images fabriquées, tenues.
        let guide_keys = propager::guide_keys(&self.doc, self.cur_panel, self.cur_layer, guide);
        let mut cadence = self.propagation.cadence;
        let summary = outline.as_ref().map_err(|e| e.clone()).and_then(|o| {
            let made = propager::made_frames(o.range.clone(), &o.keys, guide_keys.as_deref(), cadence);
            if made.is_empty() {
                return Err("Aucune image à fabriquer avec ce choix (les clés du guide sont toutes des clés peintes).".to_string());
            }
            let held = o.outputs() - made.len();
            let mut text = format!(
                "Images {} à {} : {} {} ({}), {} images à fabriquer{}.",
                self.frame_label(o.range.start),
                self.frame_label(o.range.end - 1),
                o.keys.len(),
                if o.keys.len() > 1 { "clés" } else { "clé" },
                labels(&o.keys),
                made.len(),
                if held > 0 { format!(", {held} qui tiennent l'image d'avant") } else { String::new() }
            );
            if !o.ignored.is_empty() {
                text += &format!(" Hors de la plage, ignorées : {} {}.", if o.ignored.len() > 1 { "clés" } else { "clé" }, labels(&o.ignored));
            }
            Ok(text)
        });
        // Sans clé dans la plage choisie : où sont celles du calque.
        let summary = summary.map_err(|e| match propager::outline(&self.doc, self.cur_panel, self.cur_layer, guide, None) {
            Ok(all) if ranges.as_ref().is_ok_and(|(span, r)| span != r) => format!("{e} Clés du calque : {}.", labels(&all.keys)),
            _ => e.clone(),
        });
        // L'entrée et la sortie de la timeline, ramenées dans la plage du guide.
        let cues = &self.panel().cues;
        let marked_io = cues.in_point.is_some() || cues.out_point.is_some();
        let in_out = ranges.as_ref().ok().and_then(|(span, _)| {
            let (a, b) = self.plan_play_range();
            let r = a.max(span.start)..b.min(span.end);
            (marked_io && !r.is_empty()).then_some(r)
        });
        let layer_name = self.layer().name.clone();
        let device = self.propagation.device.as_ref().map(|d| d.2.clone());
        let (mut params, mut on_gpu, mut chosen) = (self.propagation.params.clone(), self.propagation.on_gpu, guide);
        let (mut open, mut go) = (true, false);
        let mut picked: Option<Range<usize>> = None;
        let app: &App = self;
        egui::Window::new("Propager la peinture")
            .collapsible(false)
            .resizable(false)
            .default_pos(ctx.screen_rect().center() - vec2(220.0, 160.0))
            .pivot(Align2::LEFT_TOP)
            .show(ctx, |ui| {
                ui.set_width(460.0);
                ui.label(format!("Calque peint : « {layer_name} »."));
                ui.horizontal(|ui| {
                    ui.label("Suivre");
                    let current = choices.iter().find(|(g, _)| *g == chosen).map_or("", |(_, n)| n.as_str());
                    egui::ComboBox::from_id_salt("guide_propagation").width(360.0).selected_text(current).show_ui(ui, |ui| {
                        for (g, name) in &choices {
                            ui.selectable_value(&mut chosen, *g, name);
                        }
                    });
                });
                // La plage à refaire, numérotée comme la timeline (image, time code ou trame).
                if let Ok((span, range)) = &ranges {
                    let (mut first, mut last) = (range.start, range.end - 1);
                    ui.horizontal(|ui| {
                        ui.label("De l'image");
                        let from = ui.add(frame_field(app, &mut first, span)).changed();
                        ui.label("à l'image");
                        let to = ui.add(frame_field(app, &mut last, span)).changed();
                        if from || to {
                            // Les deux bornes restent dans l'ordre : celle qu'on bouge pousse l'autre.
                            if first > last {
                                (first, last) = if from { (first, first) } else { (last, last) };
                            }
                            picked = Some(first..last + 1);
                        }
                        if ui.add_enabled(*range != *span, egui::Button::new("Tout")).on_hover_text("Toute la plage du guide.").clicked() {
                            picked = Some(span.clone());
                        }
                        let io = ui
                            .add_enabled(in_out.is_some(), egui::Button::new("Entrée → sortie"))
                            .on_hover_text("Reprendre les points d'entrée et de sortie de la timeline.")
                            .on_disabled_hover_text(if marked_io {
                                "Les points d'entrée et de sortie sont hors des images que le guide couvre."
                            } else {
                                "Pas de point d'entrée ni de sortie dans la timeline."
                            });
                        if io.clicked() {
                            picked = in_out.clone();
                        }
                    });
                }
                // Quelles images fabriquer (Anthony, 2026-10-17).
                ui.horizontal(|ui| {
                    ui.label("Fabriquer");
                    ui.radio_value(&mut cadence.keys_only, false, "une image toutes les");
                    ui.add_enabled(
                        !cadence.keys_only,
                        egui::DragValue::new(&mut cadence.step).range(1..=8).speed(0.05).custom_formatter(|v, _| format!("{v:.0}")),
                    )
                    .on_hover_text("1 : chaque image ; 2 : en 2 ; 3 : en 3… Les autres images tiennent l'image d'avant.");
                    ui.label("images");
                });
                ui.horizontal(|ui| {
                    ui.add_space(62.0);
                    let keys_note = "Une vidéo change à chaque image : elle n'a pas de clés.";
                    ui.add_enabled_ui(guide_keys.is_some(), |ui| {
                        ui.radio_value(&mut cadence.keys_only, true, "seulement aux clés du guide")
                            .on_hover_text("Une image là où le dessin du guide change ; elle tient jusqu'à la clé suivante.")
                            .on_disabled_hover_text(keys_note);
                    });
                });
                ui.horizontal(|ui| {
                    ui.add_space(62.0);
                    let on = !cadence.keys_only && cadence.step > 1 && guide_keys.is_some();
                    ui.add_enabled(on, egui::Checkbox::new(&mut cadence.follow_keys, "suivre les clés du guide"))
                        .on_hover_text("Une image aussi à chaque clé du guide, et la cadence repart de là ; sinon la cadence va tout droit.")
                        .on_disabled_hover_text("Avec une cadence plus lente que chaque image, et un guide qui a des clés.");
                });
                match &summary {
                    Err(e) => {
                        ui.label(e.as_str());
                    }
                    Ok(frames) => {
                        ui.label(frames);
                        ui.label(
                            egui::RichText::new(
                                "Les autres images de la plage sont remplacées par des dessins fabriqués ; celles hors plage ne changent \
                                 pas. Les clés sont marquées « Clé » (retouche une clé puis repropage). Une seule étape d'annulation.",
                            )
                            .weak()
                            .small(),
                        );
                    }
                }
                ui.separator();
                match &device {
                    Some(name) => {
                        ui.radio_value(&mut on_gpu, true, format!("Carte graphique : {name}"));
                        ui.radio_value(&mut on_gpu, false, "Processeur (une vingtaine de fois plus lent)");
                    }
                    None => {
                        ui.label("Calcul sur le processeur (l'affichage n'utilise pas la carte graphique).");
                    }
                }
                ui.collapsing("Réglages", |ui| {
                    let w = &mut params.weights;
                    ui.add(egui::Slider::new(&mut w.video, 0.0..=20.0).text("Fidélité au guide"));
                    ui.add(egui::Slider::new(&mut w.position, 0.0..=40.0).text("Suivi du mouvement"));
                    ui.add(egui::Slider::new(&mut w.temporal, 0.0..=5.0).text("Anti-scintillement"));
                    ui.add(egui::Slider::new(&mut params.synth.uniformity, 500.0..=15000.0).text("Diversité"));
                    ui.add(egui::Slider::new(&mut params.blend.softness, 0.0..=1.0).text("Douceur du passage d'une clé à l'autre"));
                    if ui.button("Réglages par défaut").clicked() {
                        params = PropagateParams::default();
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    go = ui.add_enabled(summary.is_ok(), egui::Button::new("Propager")).clicked();
                    if ui.button("Fermer").clicked() {
                        open = false;
                    }
                });
            });
        (self.propagation.params, self.propagation.on_gpu, self.propagation.window) = (params, on_gpu, open);
        self.propagation.cadence = cadence;
        self.propagation.guide = Some(chosen);
        if let Some(r) = picked {
            self.propagation.range = Some(((self.panel().id, self.layer().id, guide), r));
        }
        if go {
            self.start_propagation(ctx);
        }
    }

    fn start_propagation(&mut self, ctx: &egui::Context) {
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        self.playing = false;
        let guide = self.propagation_guide();
        let chosen = self.propagation_ranges(guide).ok().map(|(_, r)| r);
        let plan = match propager::plan(&self.doc, self.cur_panel, self.cur_layer, guide, chosen) {
            Ok(p) => p,
            Err(e) => {
                self.status = e;
                return;
            }
        };
        let device = if self.propagation.on_gpu { self.propagation.device.clone() } else { None };
        let params = self.propagation.params.clone();
        // Toutes les clés seront marquées, celles hors plage aussi (voir `propager::apply`).
        let (keys, total) = ([&plan.outline.keys[..], &plan.outline.ignored[..]].concat(), plan.outline.outputs());
        // La cadence : toutes les images sont calculées (le suivi du mouvement va d'une image à la
        // suivante), seules celles choisies sont gardées.
        let guide_keys = propager::guide_keys(&self.doc, self.cur_panel, self.cur_layer, guide);
        let range = plan.outline.range.clone();
        let keep = propager::made_frames(range.clone(), &plan.outline.keys, guide_keys.as_deref(), self.propagation.cadence);
        let held = propager::held_frames(range, &plan.outline.keys, &keep);
        let cancel = Arc::new(AtomicBool::new(false));
        let made = Arc::new(Mutex::new(Vec::new()));
        let latest = Arc::new(Mutex::new(None));
        let computed = Arc::new(AtomicUsize::new(0));
        let (stop, sink, wake, count, last) = (cancel.clone(), made.clone(), ctx.clone(), computed.clone(), latest.clone());
        let sheet = (self.doc.width, self.doc.height);
        let (ask, requests) = channel::<usize>();
        let (answers, replies) = channel::<Result<Image, String>>();
        let handle = std::thread::spawn(move || -> Result<String, String> {
            let (w, h) = plan.size;
            // Un périphérique de la carte à elle, ouvert avec toutes ses limites (la 4K y tient) :
            // une erreur de la carte (manque de mémoire…) arrête alors la propagation sur ce fil
            // sans toucher à l'affichage, que wgpu aurait fait planter avec tout Pelure (Anthony,
            // 2026-10-17 : « le logiciel crashe très vite lors de l'utilisation de l'outil de
            // propagation »). Celui de l'affichage s'il ne s'ouvre pas, sinon le processeur.
            let (synth, name): (Box<dyn Synthesizer>, String) = match device {
                Some((device, queue, name)) => match synthese::gpu::Gpu::new() {
                    Ok(g) if g.fits(w, h, STYLE_CHANNELS, GUIDE_CHANNELS) => {
                        let n = format!("carte graphique, {}", g.name());
                        (Box::new(g), n)
                    }
                    _ => {
                        let gpu = synthese::gpu::Gpu::with_device(device, queue, name.clone());
                        if gpu.fits(w, h, STYLE_CHANNELS, GUIDE_CHANNELS) {
                            (Box::new(gpu), format!("carte graphique, {name}"))
                        } else {
                            (Box::new(Cpu), "processeur".into())
                        }
                    }
                },
                None => (Box::new(Cpu), "processeur".into()),
            };
            // Les images du guide : les fichiers de la vidéo, ou des calques composés demandés au
            // fil de l'interface (réveillé pour répondre tout de suite).
            let load = |f: usize| -> Result<Image, String> {
                match plan.frames {
                    Frames::Files(_) => plan.load_file(f),
                    Frames::Rendered => {
                        ask.send(f).map_err(|_| "l'interface ne répond plus".to_string())?;
                        wake.request_repaint();
                        replies.recv().map_err(|_| "l'interface ne répond plus".to_string())?
                    }
                }
            };
            propager::run(&plan, synth.as_ref(), &params, &stop, load, |f, im| {
                count.fetch_add(1, Ordering::Relaxed);
                *last.lock().unwrap_or_else(|e| e.into_inner()) = Some(preview(&im, 480));
                if keep.binary_search(&f).is_ok() {
                    // Tout de suite en dessin : l'image entière (8 Mo en 1080p) ne reste pas.
                    let raster = propager::to_raster(sheet.0, sheet.1, &im, plan.origin);
                    sink.lock().unwrap_or_else(|e| e.into_inner()).push((f, raster));
                }
            })?;
            Ok(name)
        });
        self.propagation.window = false;
        self.propagation.job = Some(Job {
            panel: self.panel().id,
            layer: self.layer().id,
            guide,
            keys,
            held,
            total,
            computed,
            cancel,
            made,
            latest,
            requests,
            answers,
            handle,
            started: Instant::now(),
            preview: None,
        });
    }

    fn propagation_progress(&mut self, ctx: &egui::Context) {
        // Les images de guide demandées par la tâche : composées ici, seul fil qui touche au
        // document.
        if let Some(job) = self.propagation.job.as_ref() {
            let place = self.doc.panels.iter().position(|p| p.id == job.panel).and_then(|pi| {
                let li = self.doc.panels[pi].layers.iter().position(|l| l.id == job.layer)?;
                Some((pi, li))
            });
            while let Ok(f) = job.requests.try_recv() {
                let image = place.map(|(pi, li)| propager::render_guide(&self.doc, pi, li, job.guide, f)).ok_or_else(|| "le calque a disparu".to_string());
                let _ = job.answers.send(image);
            }
        }
        let Some(job) = self.propagation.job.as_mut() else { return };
        if let Some(image) = job.latest.lock().unwrap_or_else(|e| e.into_inner()).take() {
            job.preview = Some(ctx.load_texture("propagation", image, egui::TextureOptions::LINEAR));
        }
        if !job.handle.is_finished() {
            let (done, total) = (job.computed.load(Ordering::Relaxed), job.total.max(1));
            let stopping = job.cancel.load(Ordering::Relaxed);
            let mut stop = false;
            egui::Modal::new(egui::Id::new("progression_propagation")).show(ctx, |ui| {
                ui.set_width(480.0);
                ui.heading("Propagation de la peinture");
                ui.add_space(6.0);
                ui.add(egui::ProgressBar::new(done as f32 / total as f32).show_percentage());
                let mut text = format!("{done} images sur {total}");
                if done > 0 {
                    let rest = job.started.elapsed().as_secs_f32() / done as f32 * (total - done.min(total)) as f32;
                    text += &format!(" · reste environ {}:{:02}", rest as u64 / 60, rest as u64 % 60);
                }
                ui.label(text);
                if let Some(t) = &job.preview {
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(t.id(), t.size_vec2())));
                }
                ui.add_space(6.0);
                stop = ui.add_enabled(!stopping, egui::Button::new(if stopping { "Arrêt…" } else { "Arrêter" })).clicked();
            });
            if stop {
                job.cancel.store(true, Ordering::Relaxed);
            }
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        let job = self.propagation.job.take().expect("propagation en cours");
        let stopped = job.cancel.load(Ordering::Relaxed);
        match job.handle.join() {
            Err(_) => self.status = "La propagation s'est arrêtée sur une erreur ; rien n'a changé.".into(),
            Ok(Err(_)) if stopped => self.status = "Propagation arrêtée : rien n'a changé.".into(),
            Ok(Err(e)) => self.status = format!("Propagation impossible : {e}"),
            Ok(Ok(name)) => {
                let made = std::mem::take(&mut *job.made.lock().unwrap_or_else(|e| e.into_inner()));
                let seconds = job.started.elapsed().as_secs_f32();
                let Some(pi) = self.doc.panels.iter().position(|p| p.id == job.panel) else {
                    self.status = "Propagation : le plan a disparu, rien n'a changé.".into();
                    return;
                };
                let Some(li) = self.doc.panels[pi].layers.iter().position(|l| l.id == job.layer) else {
                    self.status = "Propagation : le calque a disparu, rien n'a changé.".into();
                    return;
                };
                if pi != self.cur_panel {
                    self.set_panel(pi);
                }
                let n = made.len();
                let snap = self.snapshot();
                propager::apply_rasters(&mut self.doc, pi, li, &job.keys, made);
                propager::hold(&mut self.doc, pi, li, &job.held);
                self.renderer.invalidate_all();
                self.commit_structure(snap);
                let held = if job.held.is_empty() { String::new() } else { format!(", {} qui tiennent l'image d'avant", job.held.len()) };
                self.status = format!("Propagation : {n} images fabriquées{held} en {seconds:.0} s ({name}).");
            }
        }
    }
}
