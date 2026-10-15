use std::collections::HashMap;
use std::time::{Duration, Instant};

use eframe::egui::{self, TextureHandle, TextureId, TextureOptions};

use crate::document::Document;
use crate::render::{self, Plan};

/// Ce qui fait changer une vignette : sa composition, la version de chaque dessin et la caméra.
type Key = (Plan, Vec<u64>, [u32; 5]);

struct Entry {
    key: Key,
    texture: TextureHandle,
}

/// Temps accordé aux vignettes par image d'écran (au moins une est refaite à chaque fois).
pub const THUMB_TIME: Duration = Duration::from_millis(6);

/// Vignettes des plans (première image), refaites seulement quand leur contenu change,
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

pub fn thumb_size(doc: &Document, width: usize) -> (usize, usize) {
    let tw = width.clamp(32, 2048);
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
    pub fn texture(
        &mut self,
        ctx: &egui::Context,
        doc: &Document,
        panel: usize,
        width: usize,
        deadline: Instant,
    ) -> Option<TextureId> {
        let s = &doc.panels[panel];
        let slot = (s.id, width);
        let plan = render::panel_plan_at(doc, panel, 0, Some(doc.paper));
        let versions = plan
            .items
            .iter()
            .map(|i| doc.cels.get(&i.cel).map_or(0, |c| c.version))
            .collect();
        let cam = doc.panel_camera(panel, 0);
        let key = (plan, versions, [cam.x, cam.y, cam.zoom, cam.angle, cam.dolly].map(f32::to_bits));
        let fresh = self.entries.get(&slot).is_some_and(|e| e.key == key);
        if fresh || Instant::now() >= deadline {
            if !fresh {
                self.pending = true;
            }
            // En attendant : celle-ci, même ancienne, ou une autre taille du même plan.
            return self
                .entries
                .get(&slot)
                .or_else(|| self.entries.iter().find(|(k, _)| k.0 == s.id).map(|(_, e)| e))
                .map(|e| e.texture.id());
        }
        let (tw, th) = thumb_size(doc, width);
        let image = render::thumbnail(doc, &key.0, &cam, tw, th);
        match self.entries.get_mut(&slot) {
            Some(e) => {
                e.texture.set(image, TextureOptions::LINEAR);
                e.key = key;
            }
            None => {
                let texture = ctx.load_texture(format!("vignette-{}-{width}", s.id), image, TextureOptions::LINEAR);
                self.entries.insert(slot, Entry { key, texture });
            }
        }
        self.entries.get(&slot).map(|e| e.texture.id())
    }

    pub fn prune(&mut self, doc: &Document) {
        self.entries.retain(|(id, _), _| doc.panels.iter().any(|s| s.id == *id));
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}
