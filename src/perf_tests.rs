//! Mesures de la performance de tout le logiciel, pas seulement du dessin (Anthony, 2026-10-18 :
//! « J'ai vraiment besoin qu'on avance sur l'optimisation du soft »). L'application tourne sans
//! fenêtre (`App::sans_fenetre`) : `App::frame` construit toute l'interface comme à l'écran, egui
//! la découpe en triangles, et, avec un périphérique wgpu sans fenêtre, la carte graphique reçoit
//! les textures et peint l'image (le dessin est composé par `gpu.rs`, comme à l'écran).
//!
//! Les essais `#[ignore]` s'affichent avec :
//! `cargo test --release --offline --target-dir target\msvc perf_ -- --ignored --nocapture --test-threads=1`.
//! Les chiffres de `docs/PERFORMANCE.md` viennent de là. L'essai d'inactivité, lui, tourne
//! toujours : un défaut de repeinture au repos est un défaut.

use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::epaint::ClippedPrimitive;
use eframe::egui::mutex::RwLock;
use eframe::egui::{self, pos2, vec2, Color32, Event, Modifiers, PointerButton, RawInput, Rect, ViewportId};
use eframe::egui_wgpu::{wgpu, RenderState, Renderer, ScreenDescriptor};

use crate::app::{App, Mode};
use crate::document::{Cel, CelId, Document, Layer, LayerKind};
use crate::dock::Pane;
use crate::raster::{empty_tile, Tile, TILE};

fn block_on<F: Future>(f: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    let mut f = std::pin::pin!(f);
    loop {
        if let std::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::yield_now();
    }
}

/// Un périphérique wgpu sans fenêtre, ou `None` (aucune carte graphique).
pub(crate) fn headless() -> Option<RenderState> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))?;
    let (device, queue) = block_on(adapter.request_device(
        &wgpu::DeviceDescriptor { label: Some("essai"), required_features: wgpu::Features::empty(), required_limits: adapter.limits(), memory_hints: wgpu::MemoryHints::Performance },
        None,
    ))
    .ok()?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let renderer = Renderer::new(&device, format, None, 1, false);
    Some(RenderState { adapter, available_adapters: Vec::new(), device, queue, target_format: format, renderer: Arc::new(RwLock::new(renderer)) })
}

// ------------------------------------------------------------------ documents d'essai

/// Une tuile peinte d'un motif (un peu de transparence, pour que la composition travaille).
fn tuile(seed: u32) -> Tile {
    let mut t = empty_tile();
    for (i, p) in t.iter_mut().enumerate() {
        let (x, y) = ((i % TILE) as u32, (i / TILE) as u32);
        if (x + y + seed) % 5 < 2 {
            *p = Color32::from_rgba_premultiplied(30, 20 + (seed % 90) as u8, 40, 200);
        }
    }
    t
}

/// Un dessin bitmap de la taille de la feuille avec `tiles` tuiles peintes, réparties selon `seed`.
fn dessin(doc: &mut Document, tiles: usize, seed: usize) -> CelId {
    let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
    let n = cel.raster.tiles.len();
    for k in 0..tiles {
        let idx = (seed * 31 + k * 17) % n;
        cel.raster.tiles[idx] = Some(tuile((seed + k) as u32));
    }
    doc.add_cel(cel)
}

/// Un calque bitmap de `frames` images : un nouveau dessin toutes les `every` images.
fn calque(doc: &mut Document, name: &str, frames: usize, every: usize, tiles: usize, seed: usize) -> Layer {
    let mut layer = Layer::new(name, LayerKind::Bitmap);
    let mut last = None;
    for f in 0..frames {
        if f % every == 0 {
            last = Some(dessin(doc, tiles, seed * 1000 + f));
        }
        layer.frames.push(last);
    }
    layer
}

/// Un long projet : `plans` plans en séquences de dix, `layers` calques de `frames` images
/// (animé à l'image 2 sur le premier, plus lent sur les suivants, un fond tenu sur le dernier).
pub(crate) fn projet_long(plans: usize, layers: usize, frames: usize, tiles: usize) -> Document {
    let mut doc = Document::with_frame(1920, 1080, 24);
    let mut at = 0;
    for i in 1..plans {
        at = if i % 10 == 0 { doc.add_sequence_after(at) } else { doc.add_scene_after(at) };
    }
    for p in 0..doc.panels.len() {
        let mut ls = Vec::new();
        for l in 0..layers {
            let every = if l + 1 == layers { frames } else { 2 + 6 * l };
            ls.push(calque(&mut doc, &format!("Calque {}", l + 1), frames, every, tiles, p * 10 + l));
        }
        doc.panels[p].layers = ls;
        doc.panels[p].length = frames;
    }
    doc
}

/// Un seul plan, très chargé : `layers` calques de `frames` images.
pub(crate) fn plan_charge(layers: usize, frames: usize, tiles: usize) -> Document {
    let mut doc = Document::with_frame(1920, 1080, 24);
    let mut ls = Vec::new();
    for l in 0..layers {
        ls.push(calque(&mut doc, &format!("Calque {}", l + 1), frames, 2, tiles, l));
    }
    doc.panels[0].layers = ls;
    doc.panels[0].length = frames;
    doc
}

// ------------------------------------------------------------------ le banc

/// Ce qu'une image d'écran a coûté, en millisecondes.
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct Mesure {
    /// `App::frame` : construction de toute l'interface.
    pub construction: f64,
    /// Découpage en triangles.
    pub decoupage: f64,
    /// Textures envoyées + peinture par la carte graphique, attendue jusqu'au bout.
    pub carte: f64,
    pub textures: usize,
    pub octets: usize,
    pub sommets: usize,
    /// Formes qu'egui a reçues de l'interface (l'interface a bien été construite).
    pub formes: usize,
    /// Dans combien de temps l'application demande à être repeinte.
    pub reveil: Duration,
    /// La carte graphique en détail : textures d'egui, préparation, passe, attente.
    pub detail: [f64; 4],
}

impl Mesure {
    pub fn total(&self) -> f64 {
        self.construction + self.decoupage + self.carte
    }
}

impl App {
    /// Des vignettes (des plans, des dessins) se font sur les fils de fond ou attendent d'être envoyées ; ou des
    /// vignettes de plans attendent d'être demandées (le temps de l'image d'écran, `THUMB_TIME`, était passé :
    /// sur une machine chargée, rien n'est encore en file, elles partent à l'image suivante).
    pub(crate) fn vignettes_en_cours(&self) -> bool {
        self.thumbs.busy() || self.thumbs.pending || self.layer_thumbs.busy()
    }
}

pub(crate) struct Banc {
    pub ctx: egui::Context,
    pub app: App,
    rs: Option<RenderState>,
    pub taille: egui::Vec2,
    t: f64,
    cible: Option<wgpu::Texture>,
    /// Les textes peints par la dernière image (si `garder_textes`).
    pub textes: Vec<String>,
    pub garder_textes: bool,
    /// Les formes peintes par la dernière image (si `garder_formes`) : de quoi retrouver où un losange est
    /// tombé à l'écran.
    pub formes: Vec<egui::epaint::ClippedShape>,
    pub garder_formes: bool,
    /// Le curseur demandé par la dernière image.
    pub curseur: egui::CursorIcon,
    /// Passe les événements par `raw_input_hook` d'eframe, comme la vraie fenêtre (Tab retiré à egui, traité
    /// comme raccourci) ; faux par défaut, comme avant.
    pub crochet: bool,
    /// Les touches tenues pendant les images (Maj, Ctrl, Alt) : `RawInput::modifiers`.
    pub touches: Modifiers,
    /// Les dernières images : le réveil qu'elles ont laissé, et ce qui les avait demandées (`raison_du_reveil`).
    reveils_recents: std::collections::VecDeque<String>,
    /// La prochaine image porte une demande de fermeture de la fenêtre (la croix de Windows) : `fermer_la_fenetre`.
    demander_fermeture: bool,
    /// L'application a répondu `CancelClose` à la dernière demande de fermeture : la fenêtre reste ouverte.
    pub fermeture_retenue: bool,
    /// Les commandes de fenêtre (`ViewportCommand`) envoyées par l'application depuis le début, dans l'ordre : de quoi
    /// vérifier la taille de la fenêtre d'accueil (`accueilfenetre.rs`).
    pub commandes: Vec<egui::ViewportCommand>,
}

/// Efface les dossiers `shot-perf-bac-*` du dossier temporaire plus vieux que `age` (date de modification).
/// Rend le nombre de dossiers effacés.
pub(crate) fn menage_des_bacs(age: std::time::Duration) -> usize {
    let Ok(entrees) = std::fs::read_dir(std::env::temp_dir()) else { return 0 };
    let mut n = 0;
    for e in entrees.flatten() {
        let vieux = e.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).is_some_and(|d| d > age);
        if vieux && e.file_name().to_string_lossy().starts_with("shot-perf-bac-") && std::fs::remove_dir_all(e.path()).is_ok() {
            n += 1;
        }
    }
    n
}

pub(crate) fn banc(doc: Document, avec_gpu: bool) -> Banc {
    banc_taille(doc, avec_gpu, 1920, 1080)
}

/// Un banc dont l'écran fait `largeur` × `hauteur` pixels (les essais de fiabilité, qui peignent
/// vraiment sur la carte graphique, se contentent d'un écran plus petit).
pub(crate) fn banc_taille(doc: Document, avec_gpu: bool, largeur: u32, hauteur: u32) -> Banc {
    // Un dossier par fil d'essai : `donnees::bac_a_sable` est propre à chaque fil.
    let bac = std::env::temp_dir().join(format!("shot-perf-bac-{}-{:?}", std::process::id(), std::thread::current().id()));
    // Ménage, une fois par processus d'essai : les dossiers des passages d'avant (plus de deux heures) partent.
    // (Ils s'accumulaient : 34 000 dossiers en une journée, Anthony, 2026-10-20. Ceux d'un passage en cours sont
    // récents et restent ; `scripts/verifier.ps1` efface aussi ceux de son passage à la fin.)
    static MENAGE: std::sync::Once = std::sync::Once::new();
    MENAGE.call_once(|| {
        menage_des_bacs(std::time::Duration::from_secs(2 * 3600));
    });
    // (Vidé au premier banc du fil : Windows réutilise les numéros de processus, et les fils sont numérotés dans
    // l'ordre ; un dossier de ce nom qui existe déjà vient d'un essai d'autrefois, qui y a laissé des copies de
    // secours ou des réglages. Pas aux suivants : un essai qui simule un plantage ouvre ensuite, dans un second
    // banc du même fil, ce que le premier a laissé.)
    thread_local! {
        static BAC_PRET: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    if !BAC_PRET.with(|p| p.replace(true)) {
        let _ = std::fs::remove_dir_all(&bac);
    }
    let _ = std::fs::create_dir_all(&bac);
    crate::donnees::bac_a_sable(&bac);
    let rs = if avec_gpu { headless() } else { None };
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, rs.as_ref());
    // (Les essais d'avant le 2026-10-30 comptent sur les notes montrées d'origine ; l'application n'en montre plus d'office.)
    app.layout.board_textes = crate::boardsens::Textes { action: false, dialogue: false, notes: true };
    // (Et la page à la largeur de la vue : le zoom d'ouverture a ses essais, `horschamp_tests.rs`.)
    app.vignettes.ouverture_a_faire = false;
    app.doc = doc;
    app.welcome = false;
    app.maximized_once = 4;
    let taille = vec2(largeur as f32, hauteur as f32);
    let cible = rs.as_ref().map(|rs| {
        rs.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("écran"),
            size: wgpu::Extent3d { width: largeur, height: hauteur, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: rs.target_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    });
    let mut b = Banc { ctx, app, rs, taille, t: 0.0, cible, textes: Vec::new(), garder_textes: false, formes: Vec::new(), garder_formes: false, curseur: egui::CursorIcon::Default, crochet: false, touches: Modifiers::NONE, reveils_recents: Default::default(), demander_fermeture: false, fermeture_retenue: false, commandes: Vec::new() };
    b.attendre_pinceaux();
    b.chauffer(8);
    b.attendre_vignettes();
    b
}

impl Banc {
    /// La bibliothèque de pinceaux importés se lit sur un autre fil, qui réveille l'interface (une
    /// seule fois) à sa fin : c'est voulu, et une application au repos n'en demande pas d'autre.
    /// Mais ce réveil tombe à un moment qui dépend de la machine : le banc l'attend, comme le
    /// ferait une application déjà ouverte depuis un instant, pour que « au repos » veuille dire
    /// la même chose à chaque essai.
    fn attendre_pinceaux(&mut self) {
        let debut = Instant::now();
        while self.app.brushes_job.is_some() && debut.elapsed() < Duration::from_secs(60) {
            std::thread::sleep(Duration::from_millis(2));
            self.image(Vec::new(), 1.0 / 60.0);
        }
    }

    /// Les vignettes se font sur des fils de fond (`thumbs.rs`) et réveillent l'écran à leur fin : le banc
    /// attend les premières, comme une application déjà ouverte depuis un instant, pour que « au repos »
    /// veuille dire la même chose à chaque essai.
    pub fn attendre_vignettes(&mut self) {
        let debut = Instant::now();
        // (Jusqu'à trois images de suite où l'écran n'a rien demandé et où rien ne se fait : les vignettes ne
        // sont demandées qu'en dessinant l'écran. Juste après un changement d'onglet ou de projet, rien n'est
        // encore en file : l'attente s'arrêtait avant la première demande, et sur une machine chargée les
        // images qui suivaient ne laissaient pas aux fils le temps de finir.)
        let mut calmes = 0;
        while calmes < 3 && debut.elapsed() < Duration::from_secs(60) {
            self.image(Vec::new(), 1.0 / 60.0);
            if self.app.vignettes_en_cours() {
                calmes = 0;
                std::thread::sleep(Duration::from_millis(1));
            } else {
                calmes += 1;
            }
        }
        // (Chaque réveil laisse derrière lui deux images, `egui::Context::request_repaint`.)
        self.chauffer(3);
    }

    /// Pourquoi l'application a demandé à être repeinte, pour le message d'un essai de repos qui échoue : les dernières
    /// images avec leur réveil et le fichier:ligne de ce qui les a demandées (un fil de fond, un compteur, l'interface).
    /// Fait une image de plus (la demande faite pendant l'image qui a échoué n'apparaît qu'à la suivante).
    pub fn raison_du_reveil(&mut self) -> String {
        self.image(Vec::new(), 0.5);
        format!("\n  {}", self.reveils_recents.iter().cloned().collect::<Vec<_>>().join("\n  "))
    }

    /// La croix de la fenêtre Windows : une image porte la demande de fermeture (`close_requested`). Rend vrai si
    /// l'application la retient (`CancelClose`) ; faux si la fenêtre se serait fermée.
    pub fn fermer_la_fenetre(&mut self) -> bool {
        self.demander_fermeture = true;
        self.image(Vec::new(), 1.0 / 60.0);
        self.fermeture_retenue
    }

    /// Quelques images pour que la mise en page se stabilise (egui mesure en deux passes).
    pub fn chauffer(&mut self, n: usize) {
        for _ in 0..n {
            self.image(Vec::new(), 1.0 / 60.0);
        }
    }

    /// Une image d'écran, `dt` secondes après la précédente, avec ces événements.
    pub fn image(&mut self, events: Vec<Event>, dt: f64) -> Mesure {
        self.t += dt;
        let mut input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), self.taille)), time: Some(self.t), events, modifiers: self.touches, ..RawInput::default() };
        input.viewports.entry(ViewportId::ROOT).or_default().native_pixels_per_point = Some(1.0);
        let fermeture = std::mem::take(&mut self.demander_fermeture);
        if fermeture {
            input.viewports.entry(ViewportId::ROOT).or_default().events.push(egui::ViewportEvent::Close);
        }
        let t0 = Instant::now();
        let app = &mut self.app;
        if self.crochet {
            eframe::App::raw_input_hook(app, &self.ctx, &mut input);
        }
        let out = self.ctx.run(input, |ctx| app.frame(ctx));
        if let Some(v) = out.viewport_output.get(&ViewportId::ROOT) {
            self.commandes.extend(v.commands.iter().cloned());
        }
        if fermeture {
            self.fermeture_retenue = out.viewport_output.get(&ViewportId::ROOT).is_some_and(|v| v.commands.iter().any(|c| matches!(c, egui::ViewportCommand::CancelClose)));
        }
        let construction = t0.elapsed().as_secs_f64() * 1e3;
        let t1 = Instant::now();
        let ppp = out.pixels_per_point;
        let reveil = out.viewport_output.get(&ViewportId::ROOT).map_or(Duration::MAX, |v| v.repaint_delay);
        // Ce qui a demandé cette image (fichier et ligne de chaque `request_repaint`, demandé pendant l'image d'avant ou entre
        // les deux) : gardé pour les essais de repos, qui disent d'où vient un réveil qu'ils n'attendaient pas.
        let causes: Vec<String> = self.ctx.repaint_causes().iter().map(|c| format!("{}:{} {}", c.file.rsplit(['/', '\\']).next().unwrap_or(c.file), c.line, c.reason).trim().to_string()).collect();
        if self.reveils_recents.len() >= 16 {
            self.reveils_recents.pop_front();
        }
        self.reveils_recents.push_back(format!("t={:.1}s réveil dans {reveil:?}, demandée par {}", self.t, if causes.is_empty() { "rien".to_string() } else { causes.join(", ") }));
        let (textures, octets) = (out.textures_delta.set.len(), out.textures_delta.set.iter().map(|(_, d)| d.image.width() * d.image.height() * 4).sum());
        let formes = out.shapes.len();
        if self.garder_textes {
            self.textes = out
                .shapes
                .iter()
                .filter_map(|s| if let egui::epaint::Shape::Text(t) = &s.shape { Some(t.galley.job.text.clone()) } else { None })
                .collect();
        }
        self.curseur = out.platform_output.cursor_icon;
        if self.garder_formes {
            self.formes = out.shapes.clone();
        }
        let prims = self.ctx.tessellate(out.shapes, ppp);
        let sommets = prims.iter().map(|p| if let egui::epaint::Primitive::Mesh(m) = &p.primitive { m.vertices.len() } else { 0 }).sum();
        let decoupage = t1.elapsed().as_secs_f64() * 1e3;
        let t2 = Instant::now();
        let mut detail = [0.0; 4];
        if let (Some(rs), Some(cible)) = (&self.rs, &self.cible) {
            detail = peindre(rs, cible, &out.textures_delta, &prims, [self.taille.x as u32, self.taille.y as u32]);
        }
        let carte = t2.elapsed().as_secs_f64() * 1e3;
        Mesure { construction, decoupage, carte, textures, octets, sommets, formes, reveil, detail }
    }

    /// L'image peinte par la dernière `image` (1920 × 1080, RGBA), pour voir une mise en page ;
    /// rien sans carte graphique.
    pub(crate) fn capturer(&self) -> Option<Vec<u8>> {
        let (rs, cible) = (self.rs.as_ref()?, self.cible.as_ref()?);
        let (w, h) = (1920u32, 1080u32);
        let row = (w * 4).div_ceil(256) * 256;
        let lecture = rs.device.create_buffer(&wgpu::BufferDescriptor { label: Some("lecture"), size: (row * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
        let mut encoder = rs.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            cible.as_image_copy(),
            wgpu::TexelCopyBufferInfo { buffer: &lecture, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        rs.queue.submit([encoder.finish()]);
        let tranche = lecture.slice(..);
        tranche.map_async(wgpu::MapMode::Read, |r| r.expect("lecture"));
        rs.device.poll(wgpu::Maintain::Wait);
        let data = tranche.get_mapped_range();
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h as usize {
            out.extend_from_slice(&data[y * row as usize..y * row as usize + (w * 4) as usize]);
        }
        Some(out)
    }

    pub fn souris(&mut self, x: f32, y: f32) -> Vec<Event> {
        vec![Event::PointerMoved(pos2(x, y))]
    }

    /// Molette sur la zone où se trouve la souris.
    pub fn molette(&mut self, delta: egui::Vec2, modifiers: Modifiers) -> Vec<Event> {
        vec![Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta, modifiers }]
    }
}

/// Envoie les textures et peint l'image sur la cible, en attendant la fin du travail de la carte.
/// Renvoie les temps (ms) : textures d'egui, préparation (`prepare` du dessin : composition des
/// calques enregistrée), enregistrement de la passe, attente de la carte.
fn peindre(rs: &RenderState, cible: &wgpu::Texture, delta: &egui::TexturesDelta, prims: &[ClippedPrimitive], taille: [u32; 2]) -> [f64; 4] {
    let t = Instant::now();
    for (id, d) in &delta.set {
        rs.renderer.write().update_texture(&rs.device, &rs.queue, *id, d);
    }
    let t_tex = t.elapsed().as_secs_f64() * 1e3;
    let t = Instant::now();
    let screen = ScreenDescriptor { size_in_pixels: taille, pixels_per_point: 1.0 };
    let view = cible.create_view(&Default::default());
    let mut encoder = rs.device.create_command_encoder(&Default::default());
    let extra = rs.renderer.write().update_buffers(&rs.device, &rs.queue, &mut encoder, prims, &screen);
    let t_prep = t.elapsed().as_secs_f64() * 1e3;
    let t = Instant::now();
    {
        let mut pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("écran"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            })
            .forget_lifetime();
        rs.renderer.read().render(&mut pass, prims, &screen);
    }
    rs.queue.submit(extra.into_iter().chain([encoder.finish()]));
    let t_pass = t.elapsed().as_secs_f64() * 1e3;
    let t = Instant::now();
    rs.device.poll(wgpu::Maintain::Wait);
    let t_wait = t.elapsed().as_secs_f64() * 1e3;
    for id in &delta.free {
        rs.renderer.write().free_texture(id);
    }
    [t_tex, t_prep, t_pass, t_wait]
}

// ------------------------------------------------------------------ chiffres

fn tri(mut v: Vec<f64>) -> (f64, f64, f64) {
    if v.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    v.sort_by(f64::total_cmp);
    let moy = v.iter().sum::<f64>() / v.len() as f64;
    (moy, v[v.len() / 2], v[(v.len() * 95 / 100).min(v.len() - 1)])
}

/// Affiche une ligne de mesures : moyenne, médiane, 95e centile du total, puis par étage.
fn rapporter(titre: &str, mesures: &[Mesure]) {
    let (moy, med, p95) = tri(mesures.iter().map(Mesure::total).collect());
    let (c, _, cmax) = tri(mesures.iter().map(|m| m.construction).collect());
    let d = tri(mesures.iter().map(|m| m.decoupage).collect()).0;
    let g = tri(mesures.iter().map(|m| m.carte).collect()).0;
    let pire = mesures.iter().map(Mesure::total).fold(0.0, f64::max);
    let n = mesures.len().max(1) as f64;
    let det: Vec<String> = (0..4).map(|k| format!("{:.2}", mesures.iter().map(|m| m.detail[k]).sum::<f64>() / n)).collect();
    println!("     carte en détail (ms) : textures egui {} | préparation {} | passe {} | attente {}", det[0], det[1], det[2], det[3]);
    let octets = mesures.iter().map(|m| m.octets).sum::<usize>() as f64 / n / 1e6;
    let textures = mesures.iter().map(|m| m.textures).sum::<usize>() as f64 / n;
    let sommets = mesures.iter().map(|m| m.sommets).sum::<usize>() as f64 / n;
    println!(
        "PERF {titre:<46} total moy {moy:>7.2} méd {med:>7.2} p95 {p95:>7.2} max {pire:>7.2} ms | interface {c:>6.2} (max {cmax:>6.2}) découpage {d:>5.2} carte {g:>6.2} | envoi {octets:>5.2} Mo et {textures:.1} textures/img, {sommets:.0} sommets ({} images)",
        mesures.len()
    );
}

/// `n` images, `dt` secondes l'une après l'autre ; les événements de chacune sont donnés par
/// `avant(numéro)` (qui peut aussi agir sur l'application). Les sections du profileur sont
/// affichées (les plus coûteuses).
pub(crate) fn scenario(titre: &str, b: &mut Banc, n: usize, dt: f64, mut avant: impl FnMut(&mut Banc, usize) -> Vec<Event>) -> Vec<Mesure> {
    crate::perf::activer(true);
    let mesures: Vec<Mesure> = (0..n)
        .map(|k| {
            let ev = avant(b, k);
            b.image(ev, dt)
        })
        .collect();
    rapporter(titre, &mesures);
    let r = crate::perf::resume();
    let tete: Vec<String> = r.sections.iter().take(9).map(|(nom, moy, max)| format!("{nom} {moy:.2}/{max:.2}")).collect();
    println!("     sections (ms moy/max) : {}", tete.join(" | "));
    crate::perf::activer(false);
    mesures
}

/// Images jusqu'à ce que plus rien ne demande de repeindre (vignettes, rejeux…), ou `max` images.
/// Renvoie le nombre d'images et le temps total (ms).
pub(crate) fn jusqu_au_repos(b: &mut Banc, max: usize) -> (usize, f64) {
    let t = Instant::now();
    for k in 0..max {
        let m = b.image(Vec::new(), 1.0 / 60.0);
        // (Le rafraîchissement de la superposition, 500 ms, ne compte pas. Les vignettes qui se font sur
        // les fils de fond réveillent l'écran à leur fin : on les attend.)
        if m.reveil >= Duration::from_millis(400) && !b.app.vignettes_en_cours() {
            return (k + 1, t.elapsed().as_secs_f64() * 1e3);
        }
        if b.app.vignettes_en_cours() {
            std::thread::sleep(Duration::from_micros(500));
        }
    }
    (max, t.elapsed().as_secs_f64() * 1e3)
}

/// Quatre déplacements de souris par image dans la zone `r` (de quoi garder l'interface éveillée,
/// comme une main sur la souris).
pub(crate) fn bouge(k: usize, r: Rect) -> Vec<Event> {
    (0..4)
        .map(|j| {
            let u = ((k * 4 + j) % 97) as f32 / 97.0;
            Event::PointerMoved(pos2(r.left() + r.width() * (0.2 + 0.6 * u), r.center().y))
        })
        .collect()
}

/// Un trait de pinceau à la souris : appui, déplacements (`par_image` par image), puis lâcher.
pub(crate) fn trait_souris(k: usize, n: usize, par_image: usize, r: Rect) -> Vec<Event> {
    let mut ev = Vec::new();
    let point = |i: usize| {
        let t = i as f32 / (n * par_image) as f32;
        pos2(r.left() + r.width() * (0.15 + 0.7 * t), r.center().y + (t * 25.0).sin() * r.height() * 0.2)
    };
    if k == 0 {
        ev.push(Event::PointerMoved(point(0)));
        ev.push(Event::PointerButton { pos: point(0), button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    }
    for j in 0..par_image {
        ev.push(Event::PointerMoved(point(k * par_image + j + 1)));
    }
    if k + 1 == n {
        ev.push(Event::PointerButton { pos: point(n * par_image), button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    }
    ev
}

// ------------------------------------------------------------------ essais

/// Des images (une demi-seconde d'écart) jusqu'à ce que cinq de suite n'aient plus rien à demander
/// avant une seconde ; leur nombre, ou `max` si le repos n'arrive pas. Un petit sommeil quand
/// quelque chose travaille encore en arrière-plan (vignettes, rejeux) : on lui laisse le processeur.
pub(crate) fn se_reposer(b: &mut Banc, max: usize) -> usize {
    let mut calmes = 0;
    for k in 0..max {
        let m = b.image(Vec::new(), 0.5);
        if m.reveil >= Duration::from_secs(1) && !b.app.vignettes_en_cours() {
            calmes += 1;
            if calmes >= 5 {
                return k + 1;
            }
        } else {
            calmes = 0;
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    max
}

/// Le ménage des dossiers d'essai n'efface que les vieux : un dossier tout neuf (celui d'un passage en
/// cours) reste.
#[test]
fn the_cleanup_of_test_folders_keeps_the_recent_ones() {
    let neuf = std::env::temp_dir().join(format!("shot-perf-bac-menage-{}", std::process::id()));
    std::fs::create_dir_all(&neuf).unwrap();
    menage_des_bacs(Duration::from_secs(3600));
    assert!(neuf.exists(), "un dossier de moins d'une heure reste");
    std::fs::remove_dir_all(&neuf).ok();
}

/// Au repos, SHOT ne demande aucune image de plus : processeur à zéro (règle « sans latence »,
/// et pas de chauffage inutile). Tourne toujours.
#[test]
fn an_idle_application_does_not_repaint() {
    // (Le moindre calcul d'effet d'un autre essai réveille cette application : état commun pour elle seule.)
    crate::fxcache::monde::ecrire();
    let doc = projet_long(12, 3, 24, 4);
    let mut b = banc(doc, false);
    // (Les vignettes se font par petites tranches de temps par image d'écran : combien d'images il en
    // faut dépend de la vitesse de la machine. On attend le repos, on ne compte pas les images.)
    assert!(se_reposer(&mut b, 3000) < 3000, "SHOT finit par se reposer");
    let mesures: Vec<Mesure> = (0..10).map(|_| b.image(Vec::new(), 0.5)).collect();
    let pire = mesures.iter().map(|m| m.reveil).min().unwrap_or(Duration::MAX);
    assert!(pire >= Duration::from_secs(1), "au repos, SHOT demande à être repeint dans {pire:?}");
    // Pendant la lecture, une image après l'autre ; une fois arrêtée, plus rien.
    b.app.set_mode(Mode::Animation);
    assert!(se_reposer(&mut b, 3000) < 3000, "SHOT se repose en animation");
    b.app.toggle_play_scope(false);
    let lecture: Vec<Mesure> = (0..6).map(|_| b.image(Vec::new(), 1.0 / 24.0)).collect();
    assert!(lecture.iter().all(|m| m.reveil < Duration::from_millis(100)), "la lecture doit demander l'image suivante");
    b.app.toggle_play_scope(false);
    assert!(!b.app.playing);
    assert!(se_reposer(&mut b, 3000) < 3000, "SHOT se repose après la lecture");
    let repos: Vec<Mesure> = (0..6).map(|_| b.image(Vec::new(), 0.5)).collect();
    let pire = repos.iter().map(|m| m.reveil).min().unwrap_or(Duration::MAX);
    assert!(pire >= Duration::from_secs(1), "lecture arrêtée, SHOT demande à être repeint dans {pire:?}");
    // Un trait de pinceau, puis plus rien (le rejeu et les fonds se terminent).
    let r = b.app.canvas_rect;
    let zone = r.shrink2(vec2(r.width() * 0.2, r.height() * 0.3));
    for k in 0..20 {
        b.image(trait_souris(k, 20, 6, zone), 1.0 / 60.0);
    }
    assert!(se_reposer(&mut b, 3000) < 3000, "SHOT se repose après un trait");
    let repos: Vec<Mesure> = (0..6).map(|_| b.image(Vec::new(), 0.5)).collect();
    let pire = repos.iter().map(|m| m.reveil).min().unwrap_or(Duration::MAX);
    assert!(pire >= Duration::from_secs(1), "après un trait, SHOT demande à être repeint dans {pire:?}");
}

/// L'action « vue.performance » allume la mesure et la superposition (qui montre les panneaux et
/// leur coût), et la rééteint ; éteinte, la mesure ne garde rien.
#[test]
fn the_performance_overlay_toggles_and_reports_the_panes() {
    let mut b = banc(projet_long(8, 3, 12, 2), false);
    b.garder_textes = true;
    assert!(crate::keymap::action_info("vue.performance").is_some_and(|a| a.implemented), "l'action doit être dans la palette");
    // Éteinte : rien n'est gardé, rien n'est montré.
    b.chauffer(3);
    assert!(!crate::perf::actif());
    assert_eq!(crate::perf::resume().images, 0);
    assert!(!b.textes.iter().any(|t| t.contains("Performance (vue.performance")));
    // Allumée par l'action.
    b.app.run_action("vue.performance");
    assert!(crate::perf::actif());
    for _ in 0..4 {
        b.image(Vec::new(), 1.0 / 60.0);
    }
    let r = crate::perf::resume();
    assert!(r.images >= 3 && r.construction_moy > 0.0, "{r:?}");
    let noms: Vec<&str> = r.sections.iter().map(|s| s.0).collect();
    for want in ["Dessin", "Montage", "barre de menus", "panneaux (dock, total)"] {
        assert!(noms.contains(&want), "la section « {want} » manque : {noms:?}");
    }
    assert!(b.textes.iter().any(|t| t.contains("Performance (vue.performance")), "la superposition doit se peindre");
    let joint = b.textes.join("\n");
    assert!(joint.contains("Images :") && joint.contains("Construction de l'interface") && joint.contains("Montage"), "{joint}");
    // Le rapport nomme les réveils : le rafraîchissement de la superposition, et l'entrée.
    let rapport = crate::perf::rapport(&r);
    assert!(rapport.contains("Réveils"), "{rapport}");
    // Éteinte de nouveau : l'historique est vidé et la superposition disparaît.
    b.app.run_action("vue.performance");
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(!crate::perf::actif());
    assert_eq!(crate::perf::resume().images, 0);
    assert!(!b.textes.iter().any(|t| t.contains("Performance (vue.performance")));
}

/// L'ordre des scènes et des séquences, trouvé en une passe (`HashSet`), est celui de l'ancienne
/// boucle (`contains` dans la boucle : le carré du nombre de plans).
#[test]
fn the_scene_and_sequence_order_match_the_naive_versions() {
    let mut doc = projet_long(45, 1, 6, 1);
    // Des séquences qui se recoupent et une scène sans séquence, pour les cas tordus.
    let seqs: Vec<u64> = doc.sequences.iter().map(|s| s.id).collect();
    for (k, s) in doc.scenes.iter_mut().enumerate() {
        if k % 7 == 3 {
            s.sequence = seqs[(k * 5) % seqs.len()];
        }
        if k == 11 {
            s.sequence = 987_654;
        }
    }
    let naive_scenes = {
        let mut order: Vec<u64> = Vec::new();
        for p in &doc.panels {
            if order.last() != Some(&p.scene) && !order.contains(&p.scene) {
                order.push(p.scene);
            }
        }
        order
    };
    let naive_sequences = {
        let mut order: Vec<u64> = Vec::new();
        for i in 0..doc.panels.len() {
            let s = doc.sequence_of(i);
            if !order.contains(&s) {
                order.push(s);
            }
        }
        order
    };
    assert_eq!(doc.scene_order(), naive_scenes);
    assert_eq!(doc.sequence_order(), naive_sequences);
    assert_eq!(doc.sequence_ids(), (0..doc.panels.len()).map(|i| doc.sequence_of(i)).collect::<Vec<_>>());
    assert!(doc.sequence_order().len() > 3 && doc.scene_order().len() == doc.panels.len());
}

/// Les colonnes de la timeline qu'on peint : celles de la zone visible, avec une de marge.
#[test]
fn the_timeline_columns_to_paint_cover_the_visible_area() {
    use crate::timeline::visible_columns;
    // Grille qui commence à x = 100, 16 par colonne, 2 000 colonnes.
    assert_eq!(visible_columns(100.0, 420.0, 100.0, 16.0, 2000), (0, 21));
    // Défilée : la zone visible va de x = 1 700 à 2 100, soit les colonnes 100 à 125 ; une de marge.
    let (a, b) = visible_columns(1700.0, 2100.0, 100.0, 16.0, 2000);
    assert!(a <= 100 && a >= 98 && b >= 125 && b <= 127, "{a}..{b}");
    // Tout au bout, la grille s'arrête à sa dernière colonne.
    assert_eq!(visible_columns(100.0 + 1990.0 * 16.0, 100.0 + 2200.0 * 16.0, 100.0, 16.0, 2000).1, 2000);
    // Zone entièrement après la grille : rien d'autre que la dernière colonne.
    let (a, b) = visible_columns(100_000.0, 101_000.0, 100.0, 16.0, 2000);
    assert!(a <= b && b == 2000);
    // Zone à gauche de la grille (rien n'a défilé de ce côté) : depuis la colonne 0.
    assert_eq!(visible_columns(-50.0, 40.0, 100.0, 16.0, 2000).0, 0);
}

/// La vignette (dessins cherchés une fois, puis chaque échantillon composé) donne les pixels de la
/// composition entière échantillonnée aux mêmes points : opacité, teinte, mode de fusion,
/// transformation du calque comprises.
#[test]
fn the_thumbnail_samples_the_full_composition() {
    let mut doc = projet_long(1, 4, 6, 30);
    doc.panels[0].layers[1].blend = crate::blend::BlendMode::Multiply;
    doc.panels[0].layers[1].opacity = 0.6;
    doc.panels[0].layers[2].transform = crate::document::LayerTransform { x: 40.0, y: -25.0, scale: 0.8, ..Default::default() };
    let cam = doc.panel_camera(0, 0);
    let mut plan = crate::render::panel_plan_at(&doc, 0, 2, Some(doc.paper));
    plan.items[3].tint = Some([200, 40, 40]);
    let (tw, th) = (96, 54);
    let thumb = crate::render::thumbnail(&doc, &plan, &cam, tw, th);
    let full = crate::render::composite_full(&doc, &plan);
    let [fw, fh] = doc.frame;
    let (sx, sy) = (fw as f32 / tw as f32, fh as f32 / th as f32);
    let map = cam.frame_map(doc.frame);
    let mut painted = 0;
    for y in 0..th {
        for x in 0..tw {
            let mut acc = [0u32; 4];
            for (ox, oy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                let c = match map.at((x as f32 + ox) * sx, (y as f32 + oy) * sy) {
                    Some(p) if p.x >= 0.0 && p.y >= 0.0 && p.x < doc.width as f32 && p.y < doc.height as f32 => full[p.y as usize * doc.width + p.x as usize],
                    _ => Color32::TRANSPARENT,
                };
                for (a, v) in acc.iter_mut().zip(c.to_array()) {
                    *a += v as u32;
                }
            }
            let want = Color32::from_rgba_premultiplied((acc[0] / 4) as u8, (acc[1] / 4) as u8, (acc[2] / 4) as u8, (acc[3] / 4) as u8);
            let got = thumb.pixels[y * tw + x];
            painted += usize::from(want != Color32::from_rgb(255, 255, 255));
            assert_eq!(got, want, "pixel ({x}, {y})");
        }
    }
    assert!(painted > 200, "la vignette doit montrer des dessins ({painted} pixels autres que le papier)");
}

/// La timeline zoomée montre une miniature par bloc visible, une seule fois : avant, une miniature
/// par bloc du plan entier (des centaines), et un cache de 512 vidé d'un coup refaisait tout à chaque
/// image (130 ms par image, 1 500 textures envoyées).
#[test]
fn a_zoomed_timeline_keeps_its_thumbnails_instead_of_remaking_them() {
    crate::fxcache::monde::ecrire();
    let mut b = banc(plan_charge(10, 160, 2), false);
    b.app.set_mode(Mode::Animation);
    b.app.timeline_cell = 48.0;
    b.chauffer(10);
    b.attendre_vignettes();
    let made = b.app.layer_thumbs.len();
    let cels = b.app.doc.cels.len();
    assert!(made > 20, "la timeline zoomée doit montrer des miniatures ({made})");
    assert!(made < cels / 2, "{made} miniatures pour {cels} dessins : seulement les blocs visibles");
    let zone = Rect::from_min_size(pos2(500.0, 880.0), vec2(900.0, 120.0));
    let steady: Vec<Mesure> = (0..12).map(|k| b.image(bouge(k, zone), 1.0 / 60.0)).collect();
    assert!(steady.iter().all(|m| m.textures == 0), "des textures renvoyées à chaque image : {:?}", steady.iter().map(|m| m.textures).collect::<Vec<_>>());
    assert_eq!(b.app.layer_thumbs.len(), made, "les miniatures gardées ne changent pas");
}

/// Le long projet : 100 plans x 3 calques x 48 images, en storyboard (le montage est sous les yeux).
#[test]
#[ignore]
fn perf_long_project() {
    let t = Instant::now();
    let doc = projet_long(100, 3, 48, 6);
    println!("PERF construction du projet d'essai : {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
    let mut b = banc(doc, true);
    // Première ouverture : combien d'images (et de temps) avant que les vignettes soient toutes là.
    crate::perf::activer(true);
    let (images, ms) = jusqu_au_repos(&mut b, 2000);
    println!("PERF ouverture : {images} images et {ms:.0} ms avant le repos (vignettes du montage)");
    println!("     sections : {}", crate::perf::resume().sections.iter().take(5).map(|(n, m, x)| format!("{n} {m:.2}/{x:.2}")).collect::<Vec<_>>().join(" | "));
    crate::perf::activer(false);
    let repos = b.image(Vec::new(), 0.5);
    println!("PERF au repos : prochaine image demandée dans {:?}", repos.reveil);
    let zone = Rect::from_min_size(pos2(500.0, 880.0), vec2(900.0, 120.0));
    scenario("souris qui bouge (images à 60/s)", &mut b, 120, 1.0 / 60.0, |_, k| bouge(k, zone));
    let total = b.app.doc.total_frames();
    scenario("défilement : tête de lecture qui saute", &mut b, 120, 1.0 / 60.0, |b, k| {
        b.app.goto_global((k * 977) % total);
        bouge(k, zone)
    });
    scenario("zoom du montage (cran par image)", &mut b, 90, 1.0 / 60.0, |b, k| {
        b.app.montage_zoom = 1.0 + (k % 45) as f32 * 0.2;
        b.app.follow_frame = true;
        Vec::new()
    });
    b.app.montage_zoom = 4.0;
    scenario("molette horizontale sur le montage", &mut b, 90, 1.0 / 60.0, |b, k| {
        let mut ev = b.souris(900.0, 940.0);
        ev.extend(b.molette(vec2(-60.0 - (k % 5) as f32 * 10.0, 0.0), Modifiers::NONE));
        ev
    });
    b.app.goto_global(0);
    b.app.toggle_play_scope(true);
    scenario("lecture du montage (24 images/s)", &mut b, 96, 1.0 / 24.0, |_, _| Vec::new());
    b.app.playing = false;
    // Dessiner sur le plan courant pendant que le montage est sous les yeux : sa vignette change.
    b.app.goto_global(0);
    b.chauffer(3);
    let r = b.app.canvas_rect;
    let zone = r.shrink2(vec2(r.width() * 0.2, r.height() * 0.3));
    let cel = b.app.cel_here();
    let avant = cel.map(|c| b.app.doc.cels[&c].version);
    scenario("dessin au pinceau, montage visible (8 pts/image)", &mut b, 60, 1.0 / 60.0, |_, k| trait_souris(k, 60, 8, zone));
    let apres = cel.map(|c| b.app.doc.cels[&c].version);
    println!("PERF le trait a bien été posé : version du dessin {avant:?} -> {apres:?}, projet modifié : {}", b.app.modified);
    // La planche : une vignette par image de storyboard, ici cent plans.
    b.app.board_view = crate::app::BoardView::Panels;
    b.app.board_zoom = 0.0;
    crate::perf::activer(true);
    let (images, ms) = jusqu_au_repos(&mut b, 2000);
    println!("PERF planche : {images} images et {ms:.0} ms avant le repos");
    crate::perf::activer(false);
    let zone = b.app.canvas_rect.shrink(100.0);
    scenario("planche (100 vignettes), souris qui bouge", &mut b, 90, 1.0 / 60.0, |_, k| bouge(k, zone));
    scenario("planche, molette (défilement)", &mut b, 90, 1.0 / 60.0, |b, k| {
        let mut ev = b.souris(zone.center().x, zone.center().y);
        ev.extend(b.molette(vec2(0.0, -80.0 + (k % 7) as f32 * 4.0), Modifiers::NONE));
        ev
    });
}

/// Un plan très chargé : 15 calques x 200 images, en animation (la timeline est sous les yeux).
#[test]
#[ignore]
fn perf_heavy_plan() {
    // (40 tuiles par dessin : un dessin au trait qui occupe le quart de la feuille.)
    let mut b = banc(plan_charge(15, 200, 40), true);
    b.app.set_mode(Mode::Animation);
    b.chauffer(10);
    let (images, ms) = jusqu_au_repos(&mut b, 600);
    println!("PERF ouverture du plan chargé : {images} images, {ms:.0} ms avant le repos");
    let repos = b.image(Vec::new(), 0.5);
    println!("PERF au repos : prochaine image demandée dans {:?}", repos.reveil);
    let zone = Rect::from_min_size(pos2(500.0, 880.0), vec2(900.0, 120.0));
    scenario("souris qui bouge (timeline)", &mut b, 120, 1.0 / 60.0, |_, k| bouge(k, zone));
    scenario("défilement : tête de lecture à chaque image", &mut b, 120, 1.0 / 60.0, |b, k| {
        b.app.cur_frame = (k * 3) % 200;
        bouge(k, zone)
    });
    // Timeline zoomée : les blocs sont assez larges pour porter une miniature du dessin.
    b.app.timeline_cell = 48.0;
    b.app.cur_frame = 0;
    let (images, ms) = {
        crate::perf::activer(true);
        let r = jusqu_au_repos(&mut b, 600);
        let s = crate::perf::resume();
        println!("     sections à l'ouverture : {}", s.sections.iter().take(4).map(|(n, m, x)| format!("{n} {m:.1}/{x:.1}")).collect::<Vec<_>>().join(" | "));
        crate::perf::activer(false);
        r
    };
    println!("PERF timeline zoomée (miniatures) : {images} images, {ms:.0} ms avant le repos");
    scenario("timeline zoomée, souris qui bouge", &mut b, 90, 1.0 / 60.0, |_, k| bouge(k, zone));
    scenario("timeline zoomée, tête de lecture à chaque image", &mut b, 90, 1.0 / 60.0, |b, k| {
        b.app.cur_frame = (k * 3) % 200;
        bouge(k, zone)
    });
    b.app.timeline_cell = crate::timeline::CELL_W;
    b.app.cur_frame = 0;
    b.app.toggle_play_scope(false);
    let m = scenario("lecture du plan (24 images/s, 15 calques)", &mut b, 144, 1.0 / 24.0, |_, _| Vec::new());
    let pires = m.iter().filter(|x| x.total() > 41.7).count();
    println!("PERF lecture : {pires} images sur {} dépassent le budget de 41,7 ms (24 images/s)", m.len());
    b.app.playing = false;
    // Dessiner sur un calque du milieu : les calques dessus et dessous restent à la carte graphique.
    b.app.cur_layer = 7;
    b.app.cur_frame = 10;
    b.chauffer(3);
    let r = b.app.canvas_rect;
    let zone = r.shrink2(vec2(r.width() * 0.2, r.height() * 0.3));
    let cel = b.app.cel_here();
    let avant = cel.map(|c| b.app.doc.cels[&c].version);
    scenario("dessin au pinceau sur le calque 8 (8 points/image)", &mut b, 60, 1.0 / 60.0, |_, k| trait_souris(k, 60, 8, zone));
    let apres = cel.map(|c| b.app.doc.cels[&c].version);
    println!("PERF le trait a bien été posé : version du dessin {avant:?} -> {apres:?}, projet modifié : {}", b.app.modified);
}

/// Le démarrage avec beaucoup de pinceaux importés : 300 pointes de 512 x 512 et 100 papiers de
/// 1024 x 1024, 400 réglages, lus dans le dossier de pinceaux de l'utilisateur (ici un dossier d'essai).
#[test]
#[ignore]
fn perf_brush_library_startup() {
    use crate::brushtex::{BrushLibrary, GrayImage, Preset, Target};
    let mut lib = BrushLibrary::builtin();
    let mut rng = 12345u64;
    let mut noise = |n: usize| -> Vec<f32> {
        (0..n)
            .map(|_| {
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                ((rng >> 40) as f32) / (1u64 << 24) as f32
            })
            .collect()
    };
    for k in 0..300 {
        lib.add_tip(&format!("Pointe {k}"), GrayImage::new(512, 512, noise(512 * 512)));
    }
    for k in 0..100 {
        lib.add_paper(&format!("Papier {k}"), GrayImage::new(1024, 1024, noise(1024 * 1024)));
    }
    for k in 0..400 {
        lib.presets.push(Preset { name: format!("Importé {k}"), target: Target::Brush, settings: crate::brush::BrushSettings::brush(), source: "Essai.abr".into() });
    }
    let dir = std::env::temp_dir().join(format!("shot-perf-pinceaux-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    lib.save(&dir).expect("enregistrement");
    let t = Instant::now();
    let loaded = BrushLibrary::load(Some(&dir));
    println!("PERF démarrage : lecture de la bibliothèque de pinceaux (300 pointes, 100 papiers) : {:.0} ms, {} réglages", t.elapsed().as_secs_f64() * 1e3, loaded.presets.len());
    let _ = std::fs::remove_dir_all(&dir);
    // Le reste de la construction de l'application (polices, thème, raccourcis).
    let t = Instant::now();
    let ctx = egui::Context::default();
    let _app = App::sans_fenetre(&ctx, None);
    println!("PERF démarrage : construction de l'application : {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
}

/// Ce qui grandit avec le projet : une timeline de 2 000 images sur 15 calques, un montage de 600 plans.
#[test]
#[ignore]
fn perf_scale() {
    let zone = Rect::from_min_size(pos2(500.0, 880.0), vec2(900.0, 120.0));
    let mut b = banc(plan_charge(15, 2000, 1), true);
    b.app.set_mode(Mode::Animation);
    jusqu_au_repos(&mut b, 400);
    scenario("timeline de 2 000 images x 15 calques, souris", &mut b, 90, 1.0 / 60.0, |_, k| bouge(k, zone));
    scenario("timeline de 2 000 images, tête de lecture", &mut b, 90, 1.0 / 60.0, |b, k| {
        b.app.cur_frame = (k * 37) % 2000;
        bouge(k, zone)
    });
    drop(b);
    let mut b = banc(projet_long(600, 3, 24, 1), true);
    jusqu_au_repos(&mut b, 3000);
    scenario("montage de 600 plans x 3 calques, souris", &mut b, 90, 1.0 / 60.0, |_, k| bouge(k, zone));
    let total = b.app.doc.total_frames();
    scenario("montage de 600 plans, tête de lecture", &mut b, 90, 1.0 / 60.0, |b, k| {
        b.app.goto_global((k * 977) % total);
        bouge(k, zone)
    });
    // Une étape d'annulation de structure copie toute la structure (pas les dessins).
    let t = Instant::now();
    for _ in 0..20 {
        std::hint::black_box(b.app.doc.structure());
    }
    println!("PERF étape d'annulation de structure, 600 plans x 3 calques : {:.2} ms", t.elapsed().as_secs_f64() * 1e3 / 20.0);
}

/// Dessiner à la souris sur un calque vectoriel qui porte déjà trente traits larges et texturés
/// (le cas d'Anthony), avec la gouache, le feutre et le fusain : coût de chaque image d'écran.
#[test]
#[ignore]
fn perf_vector_drawing() {
    let (w, h) = (1920, 1080);
    let lib = crate::brushtex::BrushLibrary::builtin();
    lib.share();
    for name in ["Gouache", "Feutre", "Fusain"] {
        let (mut doc, id) = crate::affichage_tests::doc_with(w, h, crate::affichage_tests::gros_traits(30, w, h, 7));
        doc.panels[0].layers[0].frames = vec![Some(id); 48];
        doc.panels[0].length = 48;
        let mut b = banc(doc, true);
        b.app.brush = lib.presets.iter().find(|p| p.name == name).expect("brosse").settings;
        b.app.tool = crate::app::Tool::Brush;
        b.app.set_mode(Mode::Animation);
        b.chauffer(5);
        let r = b.app.canvas_rect;
        let zone = r.shrink2(vec2(r.width() * 0.2, r.height() * 0.3));
        let avant = b.app.doc.cels[&id].vector.as_ref().map(|v| v.elements.len());
        scenario(&format!("dessin vectoriel {name} (8 points/image)"), &mut b, 60, 1.0 / 60.0, |_, k| trait_souris(k, 60, 8, zone));
        // Laisser le rejeu exact finir, puis relire.
        let apres = b.app.doc.cels[&id].vector.as_ref().map(|v| v.elements.len());
        println!("PERF traits du dessin : {avant:?} -> {apres:?}");
    }
}

/// Les panneaux : Calques (60 calques), Couleur, Réglages (avec la liste de 400 pinceaux dépliée).
#[test]
#[ignore]
fn perf_panels() {
    let mut doc = plan_charge(60, 24, 2);
    doc.panels[0].length = 24;
    // Trois cents commentaires sur la timeline (panneau Commentaires, fanions des règles).
    let plan = doc.panels[0].id;
    for k in 0..300 {
        let id = doc.add_comment(plan, k % 24, "Anthony", 1_760_000_000 + k as u64);
        doc.comment_mut(id).unwrap().text = format!("Remarque {k} : revoir cette image, le trait tremble trop ici.");
    }
    let mut b = banc(doc, true);
    b.app.set_mode(Mode::Animation);
    // Beaucoup de pinceaux importés.
    let modele = b.app.brushes.presets.clone();
    for k in 0..400 {
        let mut p = modele[k % modele.len()].clone();
        p.name = format!("Importé {k:03} — {}", p.name);
        b.app.brushes.presets.push(p);
    }
    println!("PERF pinceaux : {} réglages dans la bibliothèque", b.app.brushes.presets.len());
    let zone = Rect::from_min_size(pos2(1500.0, 300.0), vec2(300.0, 400.0));
    // La liste des brosses est dans les réglages de l'outil (le pinceau) : dépliée, avec ses 480 réglages.
    b.app.set_tool(crate::app::Tool::Brush);
    b.app.pinceaux.liste_ouverte = true;
    for (pane, nom) in [(Pane::Layers, "panneau Calques (60 calques)"), (Pane::Color, "panneau Couleur"), (Pane::ToolOptions, "panneau Réglages (liste de 480 pinceaux dépliée)"), (Pane::Effects, "panneau Effets"), (Pane::History, "panneau Historique"), (Pane::Comments, "panneau Commentaires (300 commentaires)")] {
        b.app.show_pane(pane);
        let (images, ms) = jusqu_au_repos(&mut b, 200);
        println!("PERF {nom} : {images} images, {ms:.0} ms avant le repos");
        scenario(nom, &mut b, 90, 1.0 / 60.0, |_, k| bouge(k, zone));
    }
}

/// Quelques travaux de processeur, pour comparer des réglages de compilation (LTO, codegen-units,
/// target-cpu) : le meilleur de cinq passages, en millisecondes, une ligne « BENCH » chacun.
/// Lancé avec `--ignored --nocapture --test-threads=1`, sans autre travail sur la machine.
#[test]
#[ignore]
fn perf_cpu_bench() {
    fn best(name: &str, mut f: impl FnMut()) {
        f();
        let mut best = f64::MAX;
        for _ in 0..5 {
            let t = Instant::now();
            f();
            best = best.min(t.elapsed().as_secs_f64() * 1e3);
        }
        println!("BENCH {name:<44} {best:>9.2} ms");
    }
    // Composition sur processeur d'une feuille entière (vignettes, export, secours).
    let doc = projet_long(1, 4, 6, 120);
    let plan = crate::render::panel_plan_at(&doc, 0, 2, Some(doc.paper));
    best("composition d'une feuille 3840x2160, 4 calques", || {
        std::hint::black_box(crate::render::composite_full(&doc, &plan));
    });
    let cam = doc.panel_camera(0, 0);
    best("image filmée (export) 1920x1080, 4 calques", || {
        std::hint::black_box(crate::render::camera_image(&doc, &plan, &cam));
    });
    best("vignette 192x108, 4 calques", || {
        std::hint::black_box(crate::render::thumbnail(&doc, &plan, &cam, 192, 108));
    });
    // Le moteur de pinceau : trente traits larges, texturés, qui se chevauchent.
    best("trente traits texturés, rendu vectoriel 1920x1080", || {
        let (w, h) = (1920, 1080);
        let (d, _) = crate::affichage_tests::doc_with(w, h, crate::affichage_tests::gros_traits(30, w, h, 7));
        std::hint::black_box(d);
    });
    // PNG : coder et décoder une feuille.
    let r = &doc.cels[&doc.panels[0].layers[0].frames[0].unwrap()].raster;
    let rgba = r.to_rgba();
    let png = crate::io::encode_png(&rgba, r.width, r.height).unwrap();
    best("PNG : coder une feuille 3840x2160", || {
        std::hint::black_box(crate::io::encode_png(&rgba, r.width, r.height).unwrap());
    });
    best("PNG : décoder une feuille 3840x2160", || {
        std::hint::black_box(image::load_from_memory(&png).unwrap().into_rgba8());
    });
    best("feuille -> octets RGBA puis tuiles", || {
        let bytes = r.to_rgba();
        std::hint::black_box(crate::raster::Raster::from_rgba(r.width, r.height, &bytes));
    });
    // L'interface : cent images de l'application (mise en page, découpage), 100 plans.
    let mut b = banc(projet_long(100, 3, 48, 2), false);
    let zone = Rect::from_min_size(pos2(500.0, 880.0), vec2(900.0, 120.0));
    best("interface : 100 images, 100 plans, souris", || {
        for k in 0..100 {
            b.image(bouge(k, zone), 1.0 / 60.0);
        }
    });
}

/// Une vignette du montage (192 x 108) : plan de 3 calques, puis de 15.
#[test]
#[ignore]
fn perf_thumbnail_cost() {
    for (layers, tiles) in [(3usize, 40usize), (15, 40)] {
        let doc = projet_long(1, layers, 12, tiles);
        let cam = doc.panel_camera(0, 0);
        let plan = crate::render::panel_plan_at(&doc, 0, 0, Some(doc.paper));
        let (tw, th) = crate::thumbs::thumb_size(&doc, crate::thumbs::STRIP_WIDTH);
        let t = Instant::now();
        let n = 10;
        for _ in 0..n {
            std::hint::black_box(crate::render::thumbnail(&doc, &plan, &cam, tw, th));
        }
        println!("PERF vignette {tw}x{th}, {layers} calques : {:.2} ms", t.elapsed().as_secs_f64() * 1e3 / n as f64);
    }
}

/// Le fil de l'interface pendant que les vignettes du montage se font (levier 4 de
/// `docs/PERFORMANCE.md`) : un montage de 600 plans x 3 calques, vignettes toutes à refaire, défilé
/// à la molette et par sauts de la tête de lecture, puis un trait de pinceau pendant que la vignette
/// du plan courant change à chaque image. Chiffre qui compte : « interface » (moyenne, pire).
#[test]
#[ignore]
fn perf_montage_thumbnails_ui_thread() {
    let mut b = banc(projet_long(600, 3, 24, 6), true);
    let (images, ms) = jusqu_au_repos(&mut b, 6000);
    println!("PERF vignettes : ouverture du montage de 600 plans : {images} images, {ms:.0} ms avant le repos");
    b.app.montage_zoom = 4.0;
    b.app.follow_frame = false;
    let zone = Rect::from_min_size(pos2(500.0, 880.0), vec2(900.0, 120.0));
    b.app.thumbs.clear();
    scenario("vignettes à froid : molette sur le montage", &mut b, 120, 1.0 / 60.0, |b, k| {
        let mut ev = b.souris(900.0, 940.0);
        ev.extend(b.molette(vec2(-120.0 - (k % 5) as f32 * 20.0, 0.0), Modifiers::NONE));
        ev
    });
    let (images, ms) = jusqu_au_repos(&mut b, 6000);
    println!("PERF vignettes : fin du défilement : {images} images, {ms:.0} ms avant le repos");
    b.app.thumbs.clear();
    let total = b.app.doc.total_frames();
    b.app.follow_frame = true;
    scenario("vignettes à froid : tête de lecture qui saute", &mut b, 120, 1.0 / 60.0, |b, k| {
        b.app.goto_global((k * 977) % total);
        bouge(k, zone)
    });
    let (images, ms) = jusqu_au_repos(&mut b, 6000);
    println!("PERF vignettes : fin des sauts : {images} images, {ms:.0} ms avant le repos");
    // Dessiner pendant que le montage est sous les yeux : la vignette du plan courant change.
    b.app.goto_global(0);
    b.chauffer(3);
    let r = b.app.canvas_rect;
    let dessin = r.shrink2(vec2(r.width() * 0.2, r.height() * 0.3));
    scenario("vignettes : dessin au pinceau (8 pts/image), montage visible", &mut b, 60, 1.0 / 60.0, |_, k| trait_souris(k, 60, 8, dessin));
    jusqu_au_repos(&mut b, 6000);
}

/// Chaque étape de l'écriture et de la lecture d'un dessin bitmap (3840 x 2160, 40 tuiles peintes).
#[test]
#[ignore]
fn perf_cel_io_steps() {
    let mut doc = Document::with_frame(1920, 1080, 24);
    let id = dessin(&mut doc, 40, 3);
    let r = &doc.cels[&id].raster;
    let temps = |nom: &str, f: &mut dyn FnMut()| {
        let t = Instant::now();
        let n = 5;
        for _ in 0..n {
            f();
        }
        println!("PERF dessin {nom:<34} {:>8.2} ms", t.elapsed().as_secs_f64() * 1e3 / n as f64);
    };
    let mut rgba = Vec::new();
    temps("to_rgba", &mut || rgba = r.to_rgba());
    let mut png = Vec::new();
    temps("encode_png (Fast, Adaptive)", &mut || png = crate::io::encode_png(&rgba, r.width, r.height).unwrap());
    println!("PERF dessin PNG : {} octets", png.len());
    let mut img = None;
    temps("décodage PNG (image::load_from_memory)", &mut || img = Some(image::load_from_memory(&png).unwrap().to_rgba8()));
    let img = img.unwrap();
    temps("Raster::from_rgba", &mut || {
        let _ = crate::raster::Raster::from_rgba(r.width, r.height, img.as_raw());
    });
    let dir = std::env::temp_dir().join(format!("shot-perf-io-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("x.png");
    temps("écriture (tmp + rename)", &mut || {
        let mut tmp = p.as_os_str().to_owned();
        tmp.push(".tmp");
        std::fs::write(&tmp, &png).unwrap();
        std::fs::rename(&tmp, &p).unwrap();
    });
    temps("existence du fichier (stat)", &mut || {
        let _ = p.exists();
    });
    let _ = std::fs::remove_dir_all(&dir);
}

/// Enregistrer et rouvrir un grand projet (sur le disque temporaire).
#[test]
#[ignore]
fn perf_project_open_save() {
    let doc = projet_long(100, 3, 48, 6);
    let dir = std::env::temp_dir().join(format!("shot-perf-projet-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dossier");
    let manifeste = dir.join("Essai.pelure");
    let mut written = std::collections::HashMap::new();
    let t = Instant::now();
    let job = crate::project::prepare_save(&doc, &manifeste, &mut written, None).expect("préparation");
    let prepare = t.elapsed().as_secs_f64() * 1e3;
    let t = Instant::now();
    let rapport = job.run().expect("écriture");
    println!("PERF enregistrement : préparation (fil de l'interface) {prepare:.0} ms, écriture {:.0} ms, {} dessins", t.elapsed().as_secs_f64() * 1e3, rapport.written);
    // Deuxième enregistrement, rien n'a changé.
    let t = Instant::now();
    let job = crate::project::prepare_save(&doc, &manifeste, &mut written, None).expect("préparation");
    let prepare = t.elapsed().as_secs_f64() * 1e3;
    let t = Instant::now();
    let rapport = job.run().expect("écriture");
    println!("PERF enregistrement, rien de modifié : préparation {prepare:.0} ms, écriture {:.0} ms, {} dessins", t.elapsed().as_secs_f64() * 1e3, rapport.written);
    let t = Instant::now();
    let l = crate::project::load(&manifeste).expect("lecture");
    println!("PERF ouverture : {:.0} ms ({} plans, {} dessins)", t.elapsed().as_secs_f64() * 1e3, l.doc.panels.len(), l.doc.cels.len());
    let t = Instant::now();
    let l = crate::project::load_quick_progress(&manifeste, &crate::project::Progress::default()).expect("lecture");
    println!("PERF ouverture rapide : {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
    drop(l);
    let _ = std::fs::remove_dir_all(&dir);
}

// ------------------------------------------------------------------ enregistrer, textures de la carte

/// Un dessin bitmap dont les `bw` × `bh` tuiles forment un bloc d'un seul tenant (un personnage, un
/// décor : comme un vrai dessin au trait), placé selon `seed`. (`dessin`, lui, éparpille ses tuiles
/// sur toute la feuille : la boîte du contenu y est toute la feuille.)
fn dessin_groupe(doc: &mut Document, bw: usize, bh: usize, seed: usize) -> CelId {
    let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
    let (tx, ty) = (cel.raster.tiles_x, cel.raster.tiles.len() / cel.raster.tiles_x);
    let (x0, y0) = ((seed * 7) % (tx - bw), (seed * 5) % (ty - bh));
    for j in 0..bh {
        for i in 0..bw {
            cel.raster.tiles[(y0 + j) * tx + x0 + i] = Some(tuile((seed + i + j * bw) as u32));
        }
    }
    doc.add_cel(cel)
}

/// Comme `plan_charge`, avec des dessins d'un seul bloc de 8 × 5 tuiles.
fn plan_charge_groupe(layers: usize, frames: usize) -> Document {
    let mut doc = Document::with_frame(1920, 1080, 24);
    let mut ls = Vec::new();
    for l in 0..layers {
        let mut layer = Layer::new(&format!("Calque {}", l + 1), LayerKind::Bitmap);
        let mut last = None;
        for f in 0..frames {
            if f % 2 == 0 {
                last = Some(dessin_groupe(&mut doc, 8, 5, l * 1000 + f));
            }
            layer.frames.push(last);
        }
        ls.push(layer);
    }
    doc.panels[0].layers = ls;
    doc.panels[0].length = frames;
    doc
}

/// Ce que contient le cache de la carte, en une ligne.
fn carte_cache(b: &Banc, titre: &str) {
    if let Some(g) = &b.app.gpu {
        let s = g.stats();
        println!("PERF cache de la carte ({titre}) : {} textures, {:.0} Mo, {:.2} Mo par texture, {} tuiles envoyées depuis le début", s.textures, s.bytes as f64 / 1e6, s.bytes as f64 / 1e6 / s.textures.max(1) as f64, s.copies);
    }
}

/// Dessins éparpillés (la boîte du contenu est toute la feuille) puis dessins d'un bloc : mémoire de
/// la carte par dessin, nombre de dessins gardés, coût du défilement et de la lecture.
#[test]
#[ignore]
fn perf_gpu_textures() {
    // (« Avant » : textures de la feuille entière et tampon d'envoi neuf à chaque image, deux
    // réglages d'essai qui rendent le comportement d'avant ; le recyclage, lui, est celui d'après.)
    for (nom, doc, entieres, neuf) in [
        ("dessins éparpillés (40 tuiles sur toute la feuille) : AVANT (feuille entière, tampon neuf)", plan_charge(15, 200, 40), true, true),
        ("dessins éparpillés : tampon gardé", plan_charge(15, 200, 40), true, false),
        ("dessins éparpillés : APRÈS (textures au contenu, tampon gardé)", plan_charge(15, 200, 40), false, false),
        ("dessins d'un bloc (8 x 5 tuiles) : AVANT (feuille entière, tampon neuf)", plan_charge_groupe(15, 200), true, true),
        ("dessins d'un bloc : tampon gardé", plan_charge_groupe(15, 200), true, false),
        ("dessins d'un bloc : APRÈS (textures au contenu, tampon gardé)", plan_charge_groupe(15, 200), false, false),
    ] {
        println!("PERF --- {nom}");
        let mut b = banc(doc, true);
        if let Some(g) = &b.app.gpu {
            g.set_whole_textures(entieres);
            g.set_fresh_staging(neuf);
        }
        b.app.set_mode(Mode::Animation);
        b.chauffer(10);
        jusqu_au_repos(&mut b, 600);
        let zone = Rect::from_min_size(pos2(500.0, 880.0), vec2(900.0, 120.0));
        // (30 images : 15 dessins neufs à chaque image, tous différents ; puis le même parcours.
        // Plus long, il dépasserait le cache même au contenu et chaque passage chasserait le suivant.)
        scenario("défilement, premier passage : 15 dessins neufs par image", &mut b, 30, 1.0 / 60.0, |b, k| {
            b.app.cur_frame = (k * 3) % 200;
            bouge(k, zone)
        });
        carte_cache(&b, "après le défilement");
        scenario("défilement, deuxième passage (mêmes dessins)", &mut b, 30, 1.0 / 60.0, |b, k| {
            b.app.cur_frame = (k * 3) % 200;
            bouge(k, zone)
        });
        carte_cache(&b, "après le deuxième passage");
        b.app.cur_frame = 0;
        b.app.toggle_play_scope(false);
        let m = scenario("lecture du plan (24 images/s, 15 calques)", &mut b, 144, 1.0 / 24.0, |_, _| Vec::new());
        println!("PERF lecture : {} images sur {} dépassent 41,7 ms", m.iter().filter(|x| x.total() > 41.7).count(), m.len());
        b.app.playing = false;
        carte_cache(&b, "après la lecture");
        // Tête de lecture sur place, rien ne change : la composition de 15 calques par la carte.
        b.app.cur_frame = 40;
        b.chauffer(6);
        scenario("repos de contenu (15 calques composés à chaque image)", &mut b, 90, 1.0 / 60.0, |_, k| bouge(k, zone));
    }
}

/// `Ctrl+S` : ce que coûte le fil de l'interface (l'appel) et le temps jusqu'à la fin de l'écriture,
/// 50 dessins modifiés sur 500 (40 tuiles chacun), puis rien de modifié.
#[test]
#[ignore]
fn perf_save_ui_thread() {
    let mut b = banc(plan_charge_groupe(10, 100), false);
    let dir = std::env::temp_dir().join(format!("shot-perf-ctrls-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dossier");
    b.app.path = Some(dir.join("Essai.pelure"));
    let mesurer = |b: &mut Banc, titre: &str| {
        let t = Instant::now();
        b.app.save();
        let appel = t.elapsed().as_secs_f64() * 1e3;
        // (Tant que l'écriture dure, l'interface continue de tourner.)
        let mut images = 0;
        let mut pire = 0.0f64;
        while b.app.save_en_cours() {
            let m = b.image(Vec::new(), 0.016);
            pire = pire.max(m.construction);
            images += 1;
            std::thread::sleep(Duration::from_millis(2));
        }
        println!("PERF {titre} : appel sur le fil de l'interface {appel:.1} ms, fin au bout de {:.0} ms ({images} images d'écran pendant l'écriture, la plus lente {pire:.1} ms), modifié : {}", t.elapsed().as_secs_f64() * 1e3, b.app.modified);
    };
    mesurer(&mut b, "premier enregistrement (500 dessins)");
    for essai in 0..3 {
        let ids: Vec<CelId> = b.app.doc.cels.keys().copied().take(50).collect();
        for id in ids {
            let cel = b.app.doc.cels.get_mut(&id).expect("dessin");
            if let Some(t) = cel.raster.tiles.iter_mut().flatten().next() {
                t[0] = Color32::from_rgba_premultiplied(9, 9, 9, 255);
            }
            cel.version += 1;
        }
        b.app.modified = true;
        mesurer(&mut b, &format!("50 dessins modifiés (essai {essai})"));
    }
    mesurer(&mut b, "rien de modifié");
    let _ = std::fs::remove_dir_all(&dir);
}

// ------------------------------------------------------------------ passe « optim-rendu » (2026-10-23)

/// Tuiles envoyées à la carte graphique depuis le début.
fn tuiles_envoyees(b: &Banc) -> u64 {
    b.app.gpu.as_ref().map_or(0, |g| g.stats().copies)
}

/// Tirer l'opacité d'un calque (ou une clé de position) dans un plan de 15 calques de 40 tuiles : avant, chaque image du geste
/// appelait `invalidate_all` (toutes les tuiles de tous les dessins du plan renvoyées à la carte graphique, le hors-feuille
/// refait) ; depuis, `invalidate_params`. Les deux, l'un après l'autre, dans le même processus (`optim_rendu_tests.rs` vérifie
/// le geste réel).
#[test]
#[ignore]
fn perf_parameter_drag() {
    let mut b = banc(plan_charge(15, 200, 40), true);
    b.app.set_mode(Mode::Animation);
    b.chauffer(10);
    jusqu_au_repos(&mut b, 600);
    let zone = Rect::from_min_size(pos2(500.0, 880.0), vec2(900.0, 120.0));
    scenario("repère : souris qui bouge", &mut b, 90, 1.0 / 60.0, |_, k| bouge(k, zone));
    for passe in 0..2 {
        for (titre, ancien) in [("opacité tirée, invalidate_all (avant)", true), ("opacité tirée, invalidate_params (après)", false)] {
            let t0 = tuiles_envoyees(&b);
            scenario(&format!("{titre}, série {}", passe + 1), &mut b, 90, 1.0 / 60.0, |b, k| {
                b.app.doc.panels[0].layers[3].opacity = 0.3 + (k % 50) as f32 * 0.01;
                if ancien {
                    b.app.renderer.invalidate_all();
                } else {
                    b.app.renderer.invalidate_params();
                }
                bouge(k, zone)
            });
            println!("     tuiles envoyées à la carte : {} en 90 images", tuiles_envoyees(&b) - t0);
        }
    }
}

/// Le montage d'un film de 600 plans, beaucoup de blocs visibles : avant (`perf::ancien` : numéros refaits par titre et par
/// séquence, règle et barre des séquences peintes sur tout le film) puis après, deux fois, dans le même processus.
#[test]
#[ignore]
fn perf_montage_numbering() {
    let mut b = banc(projet_long(600, 3, 24, 2), true);
    jusqu_au_repos(&mut b, 3000);
    let zone = Rect::from_min_size(pos2(500.0, 880.0), vec2(900.0, 120.0));
    let total = b.app.doc.total_frames();
    for zoom in [0.6f32, 3.0] {
        b.app.montage_zoom = zoom;
        b.app.goto_global(total / 2);
        b.app.follow_frame = true;
        b.chauffer(4);
        jusqu_au_repos(&mut b, 3000);
        for passe in 0..2 {
            for (titre, ancien) in [("avant", true), ("après", false)] {
                crate::perf::set_ancien(ancien);
                b.chauffer(2);
                scenario(&format!("montage 600 plans, zoom {zoom}, souris, {titre} ({})", passe + 1), &mut b, 90, 1.0 / 60.0, |_, k| bouge(k, zone));
            }
        }
        crate::perf::set_ancien(false);
    }
    // Les numéros seuls, comme une image d'écran du montage : 160 titres et une étiquette par séquence.
    let doc = &b.app.doc;
    for (titre, ancien) in [("avant", true), ("après", false)] {
        crate::perf::set_ancien(ancien);
        let t = Instant::now();
        let mut n = 0;
        for _ in 0..20 {
            let numeros = crate::numerotation::Numeros::pour_l_ecran(doc);
            for i in 0..160 {
                n += crate::planversui::titre_avec_version_n(doc, &numeros, i * 3).len();
            }
            for s in &doc.sequences {
                n += numeros.sequence_label(doc, s.id).len();
            }
        }
        println!("PERF numéros d'une image du montage (600 plans, 160 titres, 60 séquences), {titre} : {:.3} ms ({n})", t.elapsed().as_secs_f64() * 1e3 / 20.0);
    }
    crate::perf::set_ancien(false);
}

/// Des notes sous toutes les cases, et (`cams`) un mouvement de caméra sur chaque plan : un manifeste comme celui d'un
/// vrai film.
pub(crate) fn avec_notes_et_cameras(doc: &mut Document, cams: bool) {
    for i in 0..doc.panels.len() {
        doc.panels[i].notes = format!("Plan {i} : le personnage entre par la gauche, regarde la fenêtre, puis sort lentement du cadre.");
        if cams {
            let c0 = doc.panel_camera(i, 0);
            let mut c1 = c0;
            c1.x += 300.0;
            c1.zoom *= 1.3;
            let n = doc.panels[i].frame_count();
            doc.panels[i].camera = vec![crate::camera::CameraKey::new(0, c0), crate::camera::CameraKey::new(n.saturating_sub(1), c1)];
        }
    }
}

/// Enregistrer un grand film où rien n'a changé (Ctrl+S, copie de secours) : ce que coûte le fil de l'interface
/// (`prepare_save`, le cliché) et l'écriture (autre fil), 100 et 600 plans.
#[test]
#[ignore]
fn perf_prepare_save_big_film() {
    for plans in [100usize, 600] {
        let mut doc = projet_long(plans, 3, 24, 1);
        avec_notes_et_cameras(&mut doc, true);
        let dir = std::env::temp_dir().join(format!("shot-perf-prep-{}-{plans}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("projet.pelure");
        let mut written = std::collections::HashMap::new();
        let t = Instant::now();
        crate::project::save(&doc, &path, &mut written).expect("enregistrement");
        println!("PERF premier enregistrement {plans} plans ({} dessins) : {:.0} ms", doc.cels.len(), t.elapsed().as_secs_f64() * 1e3);
        // Avant et après en alternance, sept fois chacun : le meilleur de chaque (la machine est partagée).
        let (mut avant, mut apres, mut fil) = (f64::MAX, f64::MAX, f64::MAX);
        for _ in 0..7 {
            let t = Instant::now();
            std::hint::black_box(crate::project::prepare_save_avant(&doc, &path, &mut written).expect("préparation d'avant"));
            avant = avant.min(t.elapsed().as_secs_f64() * 1e3);
            let t = Instant::now();
            let mut job = crate::project::prepare_save(&doc, &path, &mut written, None).expect("cliché");
            apres = apres.min(t.elapsed().as_secs_f64() * 1e3);
            let t = Instant::now();
            job.resoudre_ici().expect("cliché résolu");
            fil = fil.min(t.elapsed().as_secs_f64() * 1e3);
        }
        println!("PERF fil de l'interface, rien de modifié, {plans} plans : avant {avant:.2} ms, après {apres:.2} ms (et {fil:.2} ms de plus sur le fil d'écriture)");
        // L'écriture : tout réécrire comme avant, ou laisser les fichiers identiques, en alternance.
        let (mut toujours, mut identiques) = (f64::MAX, f64::MAX);
        for k in 0..10 {
            let job = crate::project::prepare_save(&doc, &path, &mut written, None).expect("préparation");
            crate::project::ecrire_toujours(k % 2 == 0);
            let t = Instant::now();
            job.run().expect("écriture");
            let ms = t.elapsed().as_secs_f64() * 1e3;
            crate::project::ecrire_toujours(false);
            if k % 2 == 0 {
                toujours = toujours.min(ms);
            } else {
                identiques = identiques.min(ms);
            }
        }
        println!("PERF écriture « rien de modifié » (autre fil, cliché compris), {plans} plans : tout réécrit {toujours:.1} ms, fichiers identiques laissés {identiques:.1} ms");
        let t = Instant::now();
        let loaded = crate::project::load(&path).expect("ouverture");
        println!("PERF ouverture {plans} plans : {:.0} ms ({} dessins, mémo de l'enregistrement compris)", t.elapsed().as_secs_f64() * 1e3, loaded.doc.cels.len());
        let t = Instant::now();
        loaded.doc.memo_enregistrement.lock().unwrap().clear();
        loaded.doc.memoriser_dessins();
        println!("PERF   dont le mémo de l'enregistrement : {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// La planche (vue Vignettes) de 100 à 600 plans, avec ou sans mouvements de caméra : `board_layout()` seule, puis l'image
/// d'écran, avant (`perf::ancien` : refaite à chaque appel, notes remises en page à chaque image) et après.
#[test]
#[ignore]
fn perf_board_layout() {
    for (plans, cams) in [(100, false), (300, false), (300, true), (600, true)] {
        let mut doc = projet_long(plans, 3, 24, 2);
        avec_notes_et_cameras(&mut doc, cams);
        let mut b = banc(doc, true);
        b.app.board_view = crate::app::BoardView::Panels;
        b.app.board_zoom = 0.0;
        b.chauffer(5);
        jusqu_au_repos(&mut b, 3000);
        let zone = b.app.canvas_rect.shrink(100.0);
        for passe in 0..2 {
            for (titre, ancien) in [("avant", true), ("après", false)] {
                crate::perf::set_ancien(ancien);
                let t = Instant::now();
                let mut cards = 0;
                for _ in 0..50 {
                    cards = std::hint::black_box(b.app.board_layout()).cards.len();
                }
                println!("PERF board_layout(), {plans} plans, caméras {cams}, {titre} ({}) : {:.3} ms par appel ({cards} cases)", passe + 1, t.elapsed().as_secs_f64() * 1e3 / 50.0);
                scenario(&format!("planche {plans} plans, caméras {cams}, souris, {titre} ({})", passe + 1), &mut b, 60, 1.0 / 60.0, |_, k| bouge(k, zone));
            }
        }
        crate::perf::set_ancien(false);
    }
}

/// Octets des textures d'egui (vignettes comprises).
fn textures_egui(ctx: &egui::Context) -> (usize, f64) {
    let tm = ctx.tex_manager();
    let tm = tm.read();
    let bytes: usize = tm.allocated().map(|(_, m)| m.bytes_used()).sum();
    (tm.num_allocated(), bytes as f64 / 1e6)
}

/// Zoomer et défiler toute une planche de 100 plans à huit zooms : la mémoire des vignettes, avant (`perf::ancien` : rien ne
/// partait que les plans supprimés) puis après (bornée, `thumbs::LIMITE_OCTETS`).
#[test]
#[ignore]
fn perf_board_thumbnail_memory() {
    for (titre, ancien) in [("avant", true), ("après", false)] {
        crate::perf::set_ancien(ancien);
        let mut b = banc(projet_long(100, 3, 24, 2), true);
        b.app.board_view = crate::app::BoardView::Panels;
        b.app.board_zoom = 0.0;
        b.chauffer(5);
        jusqu_au_repos(&mut b, 3000);
        let (n0, mb0) = textures_egui(&b.ctx);
        println!("PERF {titre} : planche ajustée, {n0} textures, {mb0:.1} Mo");
        let mut pire = 0.0f64;
        for z in [0.5f32, 0.8, 1.2, 1.6, 2.0, 2.5, 3.0, 4.0] {
            b.app.board_zoom = z;
            for step in 0..40 {
                b.app.board_pan = vec2(0.0, -(step as f32) * 400.0 * z);
                b.chauffer(2);
                b.attendre_vignettes();
                pire = pire.max(b.app.thumbs.octets().0 as f64 / 1e6);
            }
            let (n, mb) = textures_egui(&b.ctx);
            let (octets, vignettes) = b.app.thumbs.octets();
            println!("PERF {titre} : après le zoom {z} et le défilement, {n} textures, {mb:.1} Mo (vignettes des plans : {vignettes}, {:.1} Mo)", octets as f64 / 1e6);
        }
        println!("PERF {titre} : vignettes des plans, au plus {pire:.1} Mo");
    }
    crate::perf::set_ancien(false);
}
