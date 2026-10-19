use std::collections::HashMap;
use std::time::{Duration, Instant};

use eframe::egui::{self, TextureHandle, TextureId, TextureOptions};

use crate::document::Document;
use crate::render::{self, Plan};

/// Ce qui fait changer une vignette : sa composition, la version de chaque dessin et la caméra.
type Key = (Plan, Vec<u64>, [u32; 7]);

struct Entry {
    key: Key,
    texture: TextureHandle,
    /// La vignette a été faite avec des effets pas encore calculés (le fil de fond les calcule,
    /// `fxcache.rs`) : elle se refait quand un calcul de fond finit, pas avant. Contient le
    /// compteur de calculs finis au moment où elle a été faite.
    waiting: Option<u64>,
}

/// Temps accordé aux vignettes par image d'écran (au moins une est refaite à chaque fois).
pub const THUMB_TIME: Duration = Duration::from_millis(6);

/// Vignettes des plans et de leurs images de storyboard, refaites seulement quand leur contenu change,
/// et seulement quelques millisecondes par image d'écran pour ne pas saccader l'interface.
/// Une vignette par plan et par largeur demandée (la bande et la planche n'ont pas la même).
#[derive(Default)]
pub struct Thumbs {
    entries: HashMap<(u64, usize), Entry>,
    /// Des vignettes attendent leur tour : il faut redessiner l'écran.
    pub pending: bool,
}

/// Largeur des vignettes de la bande de montage.
pub const STRIP_WIDTH: usize = 192;

/// Résolution des vignettes, en pour cent de la largeur demandée (100, 50 ou 25 ; réglage
/// « vignettes » du diagnostic des performances). L'affichage agrandit la vignette à sa place :
/// seule la définition baisse.
static SCALE_PCT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(100);

pub fn set_scale_pct(pct: u32) {
    SCALE_PCT.store(pct.clamp(10, 100), std::sync::atomic::Ordering::Relaxed);
}

pub fn scale_pct() -> u32 {
    SCALE_PCT.load(std::sync::atomic::Ordering::Relaxed)
}

/// `width` à l'échelle `pct` pour cent (jamais sous `min`).
pub fn scaled(width: usize, pct: u32, min: usize) -> usize {
    (width * pct as usize / 100).max(min).min(width.max(min))
}

pub fn thumb_size(doc: &Document, width: usize) -> (usize, usize) {
    thumb_size_at(doc, width, scale_pct())
}

/// `thumb_size` à l'échelle `pct`.
pub fn thumb_size_at(doc: &Document, width: usize, pct: u32) -> (usize, usize) {
    let tw = scaled(width.clamp(32, 2048), pct, 32);
    let [fw, fh] = doc.frame;
    let th = ((tw as f32 * fh as f32 / fw.max(1) as f32).round() as usize).clamp(8, 2048);
    (tw, th)
}

/// Largeur de vignette pour `pixels` points physiques à l'écran : par paliers de 128 (on ne la
/// refait pas à chaque cran de zoom), entre 128 et 1024.
pub fn width_for(pixels: f32) -> usize {
    ((pixels / 128.0).ceil().max(1.0) as usize * 128).min(1024)
}

impl Thumbs {
    /// Vignette de la première image du plan `panel`.
    pub fn texture(
        &mut self,
        ctx: &egui::Context,
        doc: &Document,
        panel: usize,
        width: usize,
        deadline: Instant,
    ) -> Option<TextureId> {
        let id = doc.panels[panel].id;
        self.texture_at(ctx, doc, panel, 0, id, width, deadline)
    }

    /// Vignette de l'image `frame` du plan `panel`, rangée sous `id` (le plan, ou une marque
    /// d'image : une image de storyboard, 13a).
    #[allow(clippy::too_many_arguments)]
    pub fn texture_at(
        &mut self,
        ctx: &egui::Context,
        doc: &Document,
        panel: usize,
        frame: usize,
        id: u64,
        width: usize,
        deadline: Instant,
    ) -> Option<TextureId> {
        let slot = (id, width);
        let plan = render::panel_plan_at(doc, panel, frame, Some(doc.paper));
        // (Plus le nombre de fonds sous le trait arrivés de l'arrière-plan : la vignette montre le
        // fond d'avant jusqu'à l'arrivée du nouveau.)
        let versions = plan
            .items
            .iter()
            .map(|i| doc.cels.get(&i.cel).map_or(0, |c| c.version))
            .chain([crate::fx::fond_epoch()])
            .collect();
        let cam = doc.panel_camera(panel, frame);
        let key = (plan, versions, [cam.x, cam.y, cam.zoom, cam.angle, cam.dolly, cam.tilt, cam.pan].map(f32::to_bits));
        let generation = crate::fxcache::generation();
        let fresh = self.entries.get(&slot).is_some_and(|e| e.key == key && e.waiting.is_none_or(|g| g == generation));
        if fresh || Instant::now() >= deadline {
            if !fresh {
                self.pending = true;
            }
            // En attendant : celle-ci, même ancienne, ou une autre taille du même plan.
            return self
                .entries
                .get(&slot)
                .or_else(|| self.entries.iter().find(|(k, _)| k.0 == id).map(|(_, e)| e))
                .map(|e| e.texture.id());
        }
        let (tw, th) = thumb_size(doc, width);
        // Les effets non encore calculés ne bloquent pas l'écran : dernier résultat, ou dessin
        // sans effet, et la vignette se refait quand le fil de fond a fini ; le fond sous le trait
        // montre celui d'avant jusqu'à l'arrivée du nouveau (`fx::differe`).
        let (image, complete) = crate::fx::differe(|| crate::fxcache::soft_scope(|| render::thumbnail(doc, &key.0, &cam, tw, th)));
        let waiting = (!complete).then_some(generation);
        match self.entries.get_mut(&slot) {
            Some(e) => {
                e.texture.set(image, TextureOptions::LINEAR);
                e.key = key;
                e.waiting = waiting;
            }
            None => {
                let texture = ctx.load_texture(format!("vignette-{id}-{width}"), image, TextureOptions::LINEAR);
                self.entries.insert(slot, Entry { key, texture, waiting });
            }
        }
        self.entries.get(&slot).map(|e| e.texture.id())
    }

    pub fn prune(&mut self, doc: &Document) {
        if self.entries.is_empty() {
            return;
        }
        // (Les identifiants des plans et de leurs marques d'abord : chercher chaque vignette parmi
        // tous les plans, à chaque image d'écran, coûtait le carré du nombre de plans.)
        let ids: std::collections::HashSet<u64> = doc.panels.iter().flat_map(|p| std::iter::once(p.id).chain(p.marks.iter().map(|m| m.id))).collect();
        self.entries.retain(|(id, _), _| ids.contains(id));
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}
