use std::collections::HashMap;

use eframe::egui::{self, TextureHandle, TextureId, TextureOptions};

use crate::document::Document;
use crate::render::{self, Plan};

/// Ce qui fait changer une vignette : sa composition, la version de chaque dessin et la caméra.
type Key = (Plan, Vec<u64>, [u32; 4]);

struct Entry {
    key: Key,
    texture: TextureHandle,
}

/// Vignettes des plans (première image), refaites seulement quand leur contenu change,
/// et pas plus de quelques-unes par image d'écran pour ne pas figer l'interface.
#[derive(Default)]
pub struct Thumbs {
    entries: HashMap<u64, Entry>,
    /// Des vignettes attendent leur tour : il faut redessiner l'écran.
    pub pending: bool,
}

pub fn thumb_size(doc: &Document) -> (usize, usize) {
    let tw = 192;
    let [fw, fh] = doc.frame;
    let th = ((tw as f32 * fh as f32 / fw.max(1) as f32).round() as usize).clamp(8, 320);
    (tw, th)
}

impl Thumbs {
    pub fn texture(
        &mut self,
        ctx: &egui::Context,
        doc: &Document,
        panel: usize,
        budget: &mut usize,
    ) -> Option<TextureId> {
        let s = &doc.panels[panel];
        let plan = render::panel_plan(s, 0, Some(doc.paper));
        let versions = plan
            .items
            .iter()
            .map(|i| doc.cels.get(&i.cel).map_or(0, |c| c.version))
            .collect();
        let cam = doc.panel_camera(panel, 0);
        let key = (plan, versions, [cam.x, cam.y, cam.zoom, cam.angle].map(f32::to_bits));
        let fresh = self.entries.get(&s.id).is_some_and(|e| e.key == key);
        if fresh || *budget == 0 {
            if !fresh {
                self.pending = true;
            }
            return self.entries.get(&s.id).map(|e| e.texture.id());
        }
        *budget -= 1;
        let (tw, th) = thumb_size(doc);
        let image = render::thumbnail(doc, &key.0, &cam, tw, th);
        match self.entries.get_mut(&s.id) {
            Some(e) => {
                e.texture.set(image, TextureOptions::LINEAR);
                e.key = key;
            }
            None => {
                let texture = ctx.load_texture(format!("vignette-{}", s.id), image, TextureOptions::LINEAR);
                self.entries.insert(s.id, Entry { key, texture });
            }
        }
        self.entries.get(&s.id).map(|e| e.texture.id())
    }

    pub fn prune(&mut self, doc: &Document) {
        self.entries.retain(|id, _| doc.panels.iter().any(|s| s.id == *id));
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}
