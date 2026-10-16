//! « Propager » dans l'interface (propagation façon EbSynth, étape e). Trois temps :
//! - la fenêtre dit ce qui va se passer (calque vidéo, images, clés) et donne les réglages ;
//! - la tâche tourne sur un autre fil : progression, aperçu de la dernière image fabriquée,
//!   « Arrêter » ;
//! - à la fin, les dessins sont posés en une seule étape d'annulation.
//!
//! Le calcul se fait sur la carte graphique de l'affichage (le périphérique wgpu d'eframe,
//! partagé), sinon sur le processeur. Voir `propager.rs`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use eframe::egui::{self, vec2, Align2};
use eframe::egui_wgpu::{wgpu, RenderState};
use synthese::propagation::PropagateParams;
use synthese::{Cpu, Image, Synthesizer};

use crate::app::App;
use crate::document::LayerKind;
use crate::propager;

/// Canaux de la synthèse : le style (RVBA), les guides (vidéo 3, position 2, temporel 4).
const STYLE_CHANNELS: usize = 4;
const GUIDE_CHANNELS: usize = 9;

/// La propagation : réglages, fenêtre et tâche en cours.
pub(crate) struct Propagation {
    /// Le périphérique wgpu de l'affichage, s'il y en a un, et le nom de la carte.
    device: Option<(wgpu::Device, wgpu::Queue, String)>,
    window: bool,
    on_gpu: bool,
    params: PropagateParams,
    job: Option<Job>,
}

/// Une propagation en cours.
struct Job {
    /// Plan et calque (identifiants : on les retrouve même si l'ordre a changé).
    panel: u64,
    layer: u64,
    origin: (usize, usize),
    keys: Vec<usize>,
    total: usize,
    cancel: Arc<AtomicBool>,
    /// Les images fabriquées, au fur et à mesure.
    made: Arc<Mutex<Vec<(usize, Image)>>>,
    /// Le nom du calcul (carte ou processeur), ou l'erreur.
    handle: JoinHandle<Result<String, String>>,
    started: Instant,
    preview: Option<egui::TextureHandle>,
    previewed: usize,
}

impl Propagation {
    pub(crate) fn new(rs: Option<&RenderState>) -> Propagation {
        let device = rs.map(|rs| {
            let info = rs.adapter.get_info();
            (rs.device.clone(), rs.queue.clone(), format!("{} ({:?})", info.name, info.backend))
        });
        Propagation { on_gpu: device.is_some(), device, window: false, params: PropagateParams::default(), job: None }
    }

    /// Une propagation tourne : le reste de l'interface attend.
    pub(crate) fn running(&self) -> bool {
        self.job.is_some()
    }
}

/// Aperçu d'une image fabriquée (RVBA prémultiplié), réduite à `max` pixels de large au plus.
fn preview(im: &Image, max: usize) -> egui::ColorImage {
    let step = im.width.div_ceil(max).max(1);
    let (w, h) = (im.width / step, im.height / step);
    let mut data = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            data.extend_from_slice(&im.data[((y * step) * im.width + x * step) * 4..][..4]);
        }
    }
    egui::ColorImage::from_rgba_premultiplied([w, h], &data)
}

impl App {
    /// Action « calque.propager » : la fenêtre, si la propagation du calque courant est possible.
    pub(crate) fn open_propagation(&mut self) {
        match propager::outline(&self.doc, self.cur_panel, self.cur_layer) {
            Ok(_) => self.propagation.window = true,
            Err(e) => self.status = e,
        }
    }

    /// Dans le menu d'une ligne de calque : « Propager… », pour un calque bitmap d'un plan qui a un
    /// calque vidéo.
    pub(crate) fn propagation_menu(&mut self, ui: &mut egui::Ui, li: usize) {
        let paintable = self.layers().get(li).is_some_and(|l| l.video.is_none() && !l.folder && l.kind == LayerKind::Bitmap);
        if paintable && propager::video_layer(&self.doc, self.cur_panel, li).is_some() {
            ui.separator();
            self.action_item(ui, "Propager la peinture sur la vidéo…", "calque.propager");
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
        let outline = propager::outline(&self.doc, self.cur_panel, self.cur_layer);
        let summary = outline.as_ref().map(|o| {
            let video = self.layers()[o.video].name.clone();
            let keys: Vec<String> = o.keys.iter().map(|&k| self.frame_label(k)).collect();
            (
                format!("Calque « {} » sur la vidéo « {video} ».", self.layer().name),
                format!(
                    "Images {} à {} : {} {} ({}), {} images à fabriquer.",
                    self.frame_label(o.range.start),
                    self.frame_label(o.range.end - 1),
                    o.keys.len(),
                    if o.keys.len() > 1 { "clés" } else { "clé" },
                    keys.join(", "),
                    o.outputs()
                ),
            )
        });
        let device = self.propagation.device.as_ref().map(|d| d.2.clone());
        let (mut params, mut on_gpu) = (self.propagation.params.clone(), self.propagation.on_gpu);
        let (mut open, mut go) = (true, false);
        egui::Window::new("Propager la peinture")
            .collapsible(false)
            .resizable(false)
            .default_pos(ctx.screen_rect().center() - vec2(220.0, 140.0))
            .pivot(Align2::LEFT_TOP)
            .show(ctx, |ui| {
                ui.set_width(440.0);
                match &summary {
                    Err(e) => {
                        ui.label(e.as_str());
                    }
                    Ok((what, frames)) => {
                        ui.label(what);
                        ui.label(frames);
                        ui.label(
                            egui::RichText::new(
                                "Les autres images du calque, sur la durée de la vidéo, sont remplacées par des dessins fabriqués ; \
                                 les clés sont marquées « Clé » (retouche une clé puis repropage). Une seule étape d'annulation.",
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
                    ui.add(egui::Slider::new(&mut w.video, 0.0..=20.0).text("Fidélité à la vidéo"));
                    ui.add(egui::Slider::new(&mut w.position, 0.0..=40.0).text("Suivi du mouvement"));
                    ui.add(egui::Slider::new(&mut w.temporal, 0.0..=5.0).text("Anti-scintillement"));
                    ui.add(egui::Slider::new(&mut params.synth.uniformity, 500.0..=15000.0).text("Diversité"));
                    ui.add(egui::Slider::new(&mut params.blend.softness, 0.0..=1.0).text("Douceur du passage d'une clé à l'autre"));
                    if ui.button("Réglages d'EbSynth").clicked() {
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
        if go {
            self.start_propagation();
        }
    }

    fn start_propagation(&mut self) {
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        self.playing = false;
        let plan = match propager::plan(&self.doc, self.cur_panel, self.cur_layer) {
            Ok(p) => p,
            Err(e) => {
                self.status = e;
                return;
            }
        };
        let device = if self.propagation.on_gpu { self.propagation.device.clone() } else { None };
        let params = self.propagation.params.clone();
        let (origin, keys, total) = (plan.origin, plan.outline.keys.clone(), plan.outline.outputs());
        let cancel = Arc::new(AtomicBool::new(false));
        let made = Arc::new(Mutex::new(Vec::new()));
        let (stop, sink) = (cancel.clone(), made.clone());
        let handle = std::thread::spawn(move || -> Result<String, String> {
            let (w, h) = plan.size;
            // La carte de l'affichage ; une image trop grande pour ses limites (la 4K) passe sur un
            // périphérique ouvert avec toutes les limites de la carte, sinon sur le processeur.
            let (synth, name): (Box<dyn Synthesizer>, String) = match device {
                Some((device, queue, name)) => {
                    let gpu = synthese::gpu::Gpu::with_device(device, queue, name.clone());
                    if gpu.fits(w, h, STYLE_CHANNELS, GUIDE_CHANNELS) {
                        (Box::new(gpu), format!("carte graphique, {name}"))
                    } else {
                        match synthese::gpu::Gpu::new() {
                            Ok(g) if g.fits(w, h, STYLE_CHANNELS, GUIDE_CHANNELS) => {
                                let n = format!("carte graphique, {}", g.name());
                                (Box::new(g), n)
                            }
                            _ => (Box::new(Cpu), "processeur".into()),
                        }
                    }
                }
                None => (Box::new(Cpu), "processeur".into()),
            };
            propager::run(&plan, synth.as_ref(), &params, &stop, |f, im| {
                sink.lock().unwrap_or_else(|e| e.into_inner()).push((f, im));
            })?;
            Ok(name)
        });
        self.propagation.window = false;
        self.propagation.job = Some(Job {
            panel: self.panel().id,
            layer: self.layer().id,
            origin,
            keys,
            total,
            cancel,
            made,
            handle,
            started: Instant::now(),
            preview: None,
            previewed: 0,
        });
    }

    fn propagation_progress(&mut self, ctx: &egui::Context) {
        let Some(job) = self.propagation.job.as_mut() else { return };
        {
            let made = job.made.lock().unwrap_or_else(|e| e.into_inner());
            if made.len() != job.previewed {
                job.previewed = made.len();
                if let Some((_, im)) = made.last() {
                    job.preview = Some(ctx.load_texture("propagation", preview(im, 480), egui::TextureOptions::LINEAR));
                }
            }
        }
        if !job.handle.is_finished() {
            let (done, total) = (job.previewed, job.total.max(1));
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
                propager::apply(&mut self.doc, pi, li, job.origin, &job.keys, made);
                self.renderer.invalidate_all();
                self.commit_structure(snap);
                self.status = format!("Propagation : {n} images fabriquées en {seconds:.0} s ({name}).");
            }
        }
    }
}
