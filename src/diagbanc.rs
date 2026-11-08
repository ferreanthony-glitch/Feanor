//! Le banc du diagnostic des performances (`diagnostic.rs`) : une quinzaine de secondes de
//! mesures sur un autre fil, avec un avancement que l'interface lit et un drapeau pour
//! annuler. L'interface ne s'arrête jamais : elle ne fait que lire `Avancement`.
//!
//! Les étapes reprennent le code de l'application, sur un plan synthétique (douze calques de
//! 3840 × 2160, pas de projet de l'utilisateur) :
//! 1. disque : écrire puis relire 64 Mo dans le dossier temporaire (`materiel::mesurer_disque`) ;
//! 2. composition de douze calques sur le processeur (`render::composite_full`) ;
//! 3. rejeu d'un trait vectoriel texturé large (le moteur de pinceau, `vector::render_element`) ;
//! 4. un flou gaussien de la feuille entière, à pleine résolution ;
//! 5. le même flou en aperçu réduit de moitié (`fxproxy::downscale`, `fx::apply_scaled`) ;
//! 6. une lecture à 24 images par seconde : l'image filmée de chaque image du plan
//!    (`render::camera_image`), temps moyen et pire ;
//! 7. l'envoi de dessins à la carte graphique (écriture d'une texture 1920 × 1080 et attente de
//!    la carte), si l'affichage passe par wgpu.
//!
//! Le fil laisse un cœur à l'interface (`render::set_thread_cap`). Aucun fichier de l'utilisateur
//! n'est ouvert ; le fichier d'essai du disque est effacé quoi qu'il arrive.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use eframe::egui::{pos2, Color32};
use eframe::egui_wgpu::wgpu;

use crate::diagnostic::Mesures;
use crate::document::{Cel, CelId, Document, Layer, LayerKind};
use crate::materiel::{self, Disque};
use crate::raster::{empty_tile, Raster, Tile, TILE};

/// Les étapes : nom affiché, durée visée en secondes (elle fixe la part de la barre).
pub const ETAPES: [(&str, f32); 7] = [
    ("Disque : écriture et lecture de 64 Mo", 2.0),
    ("Composition de douze calques", 2.5),
    ("Rejeu d'un trait texturé", 2.5),
    ("Flou à pleine résolution", 2.0),
    ("Flou en aperçu réduit", 1.5),
    ("Lecture d'un plan chargé", 4.0),
    ("Envoi à la carte graphique", 1.5),
];

/// L'avancement, lu par l'interface pendant que le fil travaille.
#[derive(Default)]
pub struct Avancement {
    /// Millièmes du banc faits.
    fraction: AtomicU32,
    etape: Mutex<String>,
}

impl Avancement {
    /// De 0 à 1.
    pub fn fraction(&self) -> f32 {
        self.fraction.load(Ordering::Relaxed) as f32 / 1000.0
    }

    pub fn etape(&self) -> String {
        self.etape.lock().map(|e| e.clone()).unwrap_or_default()
    }

    fn pose(&self, fraction: f32) {
        self.fraction.store((fraction.clamp(0.0, 1.0) * 1000.0) as u32, Ordering::Relaxed);
    }
}

/// Ce que le banc rend.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Resultat {
    pub mesures: Mesures,
    pub disque: Option<Disque>,
    /// L'utilisateur a annulé : les mesures sont incomplètes.
    pub annule: bool,
}

/// Un banc en route.
pub struct Banc {
    pub avancement: Arc<Avancement>,
    annuler: Arc<AtomicBool>,
    handle: JoinHandle<Resultat>,
}

impl Banc {
    /// Démarre le banc. `carte` : le périphérique de l'affichage (pour l'envoi de textures).
    /// `reveil` est appelé à chaque pas, pour que l'interface redessine la barre.
    pub fn lancer(carte: Option<(wgpu::Device, wgpu::Queue)>, reveil: impl Fn() + Send + 'static) -> Banc {
        let avancement = Arc::new(Avancement::default());
        let annuler = Arc::new(AtomicBool::new(false));
        let (av, an) = (avancement.clone(), annuler.clone());
        let handle = std::thread::Builder::new()
            .name("diagnostic".into())
            .spawn(move || executer(&av, &an, carte, &reveil))
            .expect("fil du diagnostic");
        Banc { avancement, annuler, handle }
    }

    pub fn annuler(&self) {
        self.annuler.store(true, Ordering::Relaxed);
    }

    pub fn fini(&self) -> bool {
        self.handle.is_finished()
    }

    /// Le résultat (à n'appeler qu'une fois `fini`). Un banc qui a planté rend un résultat annulé.
    pub fn joindre(self) -> Resultat {
        self.handle.join().unwrap_or(Resultat { annule: true, ..Default::default() })
    }
}

/// Un travail chronométré : `f` tourne jusqu'à `duree` écoulée (au moins `minimum` fois, au plus
/// 400), la première fois pour chauffer, sans compter. Rend les temps de chaque tour en
/// millisecondes, `None` si on a annulé.
struct Chrono<'a> {
    avancement: &'a Avancement,
    annuler: &'a AtomicBool,
    reveil: &'a dyn Fn(),
    /// Part de la barre déjà faite avant l'étape, et part de l'étape.
    debut: f32,
    part: f32,
}

impl Chrono<'_> {
    fn tours(&self, duree: Duration, minimum: usize, mut f: impl FnMut()) -> Option<Vec<f64>> {
        f();
        let depart = Instant::now();
        let mut temps = Vec::new();
        while temps.len() < 400 && (depart.elapsed() < duree || temps.len() < minimum) {
            if self.annuler.load(Ordering::Relaxed) {
                return None;
            }
            let t = Instant::now();
            f();
            temps.push(t.elapsed().as_secs_f64() * 1e3);
            let avance = (depart.elapsed().as_secs_f32() / duree.as_secs_f32()).min(1.0);
            self.avancement.pose(self.debut + self.part * avance);
            (self.reveil)();
        }
        Some(temps)
    }
}

/// La médiane (les autres tours, perturbés par le reste de la machine, comptent moins).
pub fn mediane(temps: &[f64]) -> f64 {
    let mut v = temps.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    if v.is_empty() {
        0.0
    } else {
        v[v.len() / 2]
    }
}

fn executer(av: &Avancement, annuler: &AtomicBool, carte: Option<(wgpu::Device, wgpu::Queue)>, reveil: &dyn Fn()) -> Resultat {
    let debut_total = Instant::now();
    // Un cœur reste à l'interface.
    crate::render::set_thread_cap(crate::parallele::coeurs().saturating_sub(1).max(1));
    let total: f32 = ETAPES.iter().map(|e| e.1).sum();
    let mut fait = 0.0f32;
    let mut res = Resultat::default();
    let etape = |k: usize, fait: &mut f32| {
        if let Ok(mut e) = av.etape.lock() {
            *e = ETAPES[k].0.to_string();
        }
        let chrono = Chrono { avancement: av, annuler, reveil, debut: *fait / total, part: ETAPES[k].1 / total };
        *fait += ETAPES[k].1;
        reveil();
        chrono
    };
    macro_rules! fini {
        () => {{
            res.annule = true;
            res.mesures.duree_s = debut_total.elapsed().as_secs_f64();
            return res;
        }};
    }
    // 1. Disque.
    let c = etape(0, &mut fait);
    res.disque = materiel::mesurer_disque(&std::env::temp_dir(), 64, annuler);
    av.pose(c.debut + c.part);
    if annuler.load(Ordering::Relaxed) {
        fini!();
    }
    // Le plan synthétique, fait une fois.
    let doc = plan_lourd(12, 12, 60);
    let plan = crate::render::panel_plan_at(&doc, 0, 0, Some(doc.paper));
    // 2. Composition.
    let c = etape(1, &mut fait);
    match c.tours(Duration::from_millis(2300), 3, || {
        std::hint::black_box(crate::render::composite_full(&doc, &plan));
    }) {
        Some(t) => res.mesures.composition_ms = Some(mediane(&t)),
        None => fini!(),
    }
    // 3. Un trait texturé.
    let c = etape(2, &mut fait);
    let trait_ = trait_texture(1920, 1080);
    match c.tours(Duration::from_millis(2300), 3, || {
        let mut e = trait_.clone();
        e.invalidate_cache();
        let mut r = Raster::new(1920, 1080);
        crate::vector::render_element(&mut r, &e);
        std::hint::black_box(r);
    }) {
        Some(t) => res.mesures.trait_ms = Some(mediane(&t)),
        None => fini!(),
    }
    // 4 et 5. Le flou, plein puis réduit.
    let feuille = feuille_peinte(3840, 2160, 120);
    let flou = [crate::fx::FxStep { kind: crate::fx::EffectKind::GaussianBlur, values: vec![20.0] }];
    let c = etape(3, &mut fait);
    match c.tours(Duration::from_millis(1800), 3, || {
        std::hint::black_box(crate::fx::apply_scaled(&feuille, &flou, 0));
    }) {
        Some(t) => res.mesures.flou_plein_ms = Some(mediane(&t)),
        None => fini!(),
    }
    let c = etape(4, &mut fait);
    match c.tours(Duration::from_millis(1300), 3, || {
        let petit = crate::fxproxy::downscale(&feuille, 1);
        std::hint::black_box(crate::fx::apply_scaled(&petit, &flou, 1));
    }) {
        Some(t) => res.mesures.flou_apercu_ms = Some(mediane(&t)),
        None => fini!(),
    }
    drop(feuille);
    // 6. Une lecture : l'image filmée de chaque image du plan, en boucle.
    let c = etape(5, &mut fait);
    let mut image = 0usize;
    match c.tours(Duration::from_millis(3800), 12, || {
        let f = image % 12;
        image += 1;
        let p = crate::render::panel_plan_at(&doc, 0, f, Some(doc.paper));
        let cam = doc.panel_camera(0, f);
        std::hint::black_box(crate::render::camera_image(&doc, &p, &cam));
    }) {
        Some(t) => {
            res.mesures.lecture_ms = Some(t.iter().sum::<f64>() / t.len().max(1) as f64);
            res.mesures.lecture_pire_ms = t.iter().copied().reduce(f64::max);
        }
        None => fini!(),
    }
    drop(doc);
    // 7. La carte graphique.
    let c = etape(6, &mut fait);
    if let Some((device, queue)) = &carte {
        match envoi_carte(&c, device, queue) {
            Ok(v) => res.mesures.envoi_carte_mo_s = v,
            Err(()) => fini!(),
        }
    }
    av.pose(1.0);
    res.mesures.duree_s = debut_total.elapsed().as_secs_f64();
    res
}

/// Débit d'envoi d'une texture de 1920 × 1080 à la carte, en Mo par seconde (`Err` : annulé).
fn envoi_carte(c: &Chrono, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Option<f64>, ()> {
    let (w, h) = (1920u32, 1080u32);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("diagnostic"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let data = vec![0x5Au8; (w * h * 4) as usize];
    let temps = c
        .tours(Duration::from_millis(1300), 3, || {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                &data,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
                wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            );
            queue.submit(std::iter::empty());
            device.poll(wgpu::Maintain::Wait);
        })
        .ok_or(())?;
    let mo = (w * h * 4) as f64 / (1u64 << 20) as f64;
    let t = mediane(&temps) / 1e3;
    Ok((t > 0.0).then_some(mo / t))
}

// ---------------------------------------------------------------------- le plan synthétique

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

/// Un dessin de la taille de la feuille avec `tiles` tuiles peintes, réparties selon `seed`.
fn dessin(doc: &mut Document, tiles: usize, seed: usize) -> CelId {
    let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
    let n = cel.raster.tiles.len().max(1);
    for k in 0..tiles {
        let idx = (seed * 31 + k * 17) % n;
        cel.raster.tiles[idx] = Some(tuile((seed + k) as u32));
    }
    doc.add_cel(cel)
}

/// Un plan de `layers` calques et `frames` images (un nouveau dessin toutes les deux images),
/// `tiles` tuiles peintes par dessin.
pub fn plan_lourd(layers: usize, frames: usize, tiles: usize) -> Document {
    let mut doc = Document::with_frame(1920, 1080, 24);
    let mut ls = Vec::new();
    for l in 0..layers {
        let mut layer = Layer::new(format!("Calque {}", l + 1), LayerKind::Bitmap);
        let mut last = None;
        for f in 0..frames {
            if f % 2 == 0 {
                last = Some(dessin(&mut doc, tiles, l * 1000 + f));
            }
            layer.frames.push(last);
        }
        ls.push(layer);
    }
    doc.panels[0].layers = ls;
    doc.panels[0].length = frames;
    doc
}

/// Une feuille `w` × `h` avec `tiles` tuiles peintes.
fn feuille_peinte(w: usize, h: usize, tiles: usize) -> Raster {
    let mut r = Raster::new(w, h);
    let n = r.tiles.len().max(1);
    for k in 0..tiles {
        r.tiles[(k * 37) % n] = Some(tuile(k as u32));
    }
    r
}

/// Un trait de gouache large, tracé par le moteur de pinceau, tel qu'un lâcher le garde.
pub fn trait_texture(w: usize, h: usize) -> crate::vector::Element {
    use crate::brush::{StrokeMode, StrokeState, Toucher};
    use crate::vector::{Element, Trace};
    let lib = crate::brushtex::BrushLibrary::builtin();
    lib.share();
    let mut s = lib.presets.iter().find(|p| p.name == "Gouache").map(|p| p.settings).unwrap_or_else(crate::brush::BrushSettings::brush);
    s.size = 110.0;
    let steps = 120;
    let pts: Vec<_> = (0..=steps)
        .map(|i| {
            let t = i as f32 / steps as f32;
            (pos2(300.0 + 900.0 * t, 400.0 + (t * 9.0).sin() * 60.0), 0.3 + 0.7 * (t * std::f32::consts::PI).sin(), i as f64 / 200.0)
        })
        .collect();
    let mut raster = Raster::new(w, h);
    let mut st = StrokeState::new(0, StrokeMode::Paint([200, 30, 30]), s, None, true).with_textures(lib.textures(&s)).with_feel(Toucher::default());
    let mut dirty = Vec::new();
    for &(p, pr, t) in &pts {
        st.add_point_at(&mut raster, p, pr, t, &mut dirty);
    }
    st.finish(&mut raster, &mut dirty);
    let trace = Trace { pressure: st.path_pressure.clone(), time: st.path_time.clone(), feel: Some(Toucher::default()), secondary: None, lien: None, cache: Default::default() };
    Element::Stroke { points: st.path.clone(), color: [200, 30, 30], opacity: s.opacity, hardness: s.hardness, brush: lib.for_vector(&s).map(Box::new), trace }
}
