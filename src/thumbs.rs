//! Vignettes des plans (bande du montage, planche) et des dessins (miniatures de la timeline, panneau
//! Calques) : composées sur des fils de fond, jamais sur le fil de l'interface (levier 4 de
//! `docs/PERFORMANCE.md`, règle « sans latence »).
//!
//! Le fil de l'interface ne fait que trois choses, toutes légères :
//! 1. il calcule la clé de la vignette (composition du plan, version de chaque dessin, caméra) et la
//!    compare à celle de la vignette qu'il garde ;
//! 2. si elle a changé, il prend un cliché du plan (`snapshot` : un projet minuscule qui ne porte que
//!    les dessins du plan, partagés par `Arc` : copier un `Raster` ne copie aucune tuile) et le met en
//!    file ;
//! 3. il envoie à la carte graphique les images finies (`Thumbs::begin_frame`), au plus une
//!    milliseconde par image d'écran.
//!
//! En attendant, l'ancienne vignette (ou celle d'une autre taille du même plan) reste à l'écran.
//! Les fils composent avec `render::thumbnail_until`, le code d'avant, sur le cliché : les pixels sont
//! ceux d'avant, y compris les effets de calque, les calques d'effets, les fonds sous le trait, les
//! calques vidéo et remplissages au griffonnage, qui passent par le même mode « doux » (`fxcache::soft_scope`).
//!
//! File d'attente (`Pool`, aussi celle des miniatures de dessins, `layersui.rs`) : une demande par
//! vignette ; une nouvelle demande pour la même vignette remplace la précédente (et arrête son calcul
//! s'il a commencé) ; seules les vignettes demandées à l'une des dernières images d'écran sont
//! calculées (celles qu'on ne voit plus sont abandonnées), les dernières demandées d'abord.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, ColorImage, TextureHandle, TextureId, TextureOptions};

use crate::camera::Camera;
use crate::document::{Cel, Document};
use crate::raster::Raster;
use crate::render::{self, Plan};

/// Ce qui fait changer une vignette : pour un plan, sa composition, la version de chaque dessin et la
/// caméra ; pour un dessin, sa version et la taille demandée.
#[derive(PartialEq)]
pub(crate) enum Key {
    Plan(Plan, Vec<u64>, [u32; 7]),
    Cel(u64, usize, usize),
}

/// Une vignette : le plan (ou la marque d'image, ou le dessin) et la largeur demandée.
pub(crate) type Slot = (u64, usize);

struct Entry {
    key: Arc<Key>,
    texture: TextureHandle,
    /// La vignette a été faite avec des effets pas encore calculés (le fil de fond les calcule,
    /// `fxcache.rs`) : elle se refait quand un calcul de fond finit, pas avant. Contient le
    /// compteur de calculs finis au moment où elle a été demandée.
    waiting: Option<u64>,
    /// Numéro de la demande dont elle vient (une réponse plus ancienne ne la remplace pas).
    seq: u64,
    /// Pixels de marge autour du cadre dans l'image (la planche : le trait qui déborde un peu, `texture_case`) ; zéro ailleurs.
    marge: usize,
    /// L'image envoyée à la carte graphique (essais : la comparer à l'ancien calcul).
    #[cfg(test)]
    image: ColorImage,
}

/// La vignette d'une case de la planche : sa texture, et la marge (en pixels de la texture) qu'elle porte tout autour du
/// cadre, où seul le trait qui déborde est peint (le reste est transparent).
#[derive(Clone, Copy, Debug)]
pub struct CaseTex {
    pub id: TextureId,
    pub marge: usize,
    /// La taille de la texture entière, marge comprise.
    pub taille: [usize; 2],
}

impl CaseTex {
    /// Le rectangle de la texture entière à l'écran, la case (le cadre) étant `thumb`.
    pub fn rect(&self, thumb: egui::Rect) -> egui::Rect {
        let (tw, th) = (self.taille[0].saturating_sub(2 * self.marge).max(1) as f32, self.taille[1].saturating_sub(2 * self.marge).max(1) as f32);
        thumb.expand2(egui::vec2(self.marge as f32 * thumb.width() / tw, self.marge as f32 * thumb.height() / th))
    }
}

/// La marge, en pixels, d'une vignette de `tw` pixels de large : `RING_FRACTION` de sa largeur, un pixel au moins.
pub fn marge_px(tw: usize) -> usize {
    ((tw as f32 * crate::board::RING_FRACTION).round() as usize).max(1)
}

/// Temps accordé par image d'écran à la préparation des demandes de vignettes (cliché et mise en
/// file : quelques dizaines de microsecondes chacune).
pub const THUMB_TIME: Duration = Duration::from_millis(2);

/// Temps accordé par image d'écran à l'envoi des images finies à la carte graphique (au moins une).
pub(crate) const UPLOAD_TIME: Duration = Duration::from_millis(1);

/// Une demande que l'interface ne renouvelle pas depuis ce nombre d'images d'écran n'est plus
/// attendue par personne (la vignette n'est plus à l'écran) : elle est abandonnée.
const STALE_FRAMES: u64 = 3;

/// Vignettes des plans et de leurs images de storyboard, refaites seulement quand leur contenu change,
/// sur des fils de fond (voir plus haut). Une vignette par plan et par largeur demandée (la bande et
/// la planche n'ont pas la même).
#[derive(Default)]
pub struct Thumbs {
    entries: HashMap<Slot, Entry>,
    /// Des vignettes attendent leur tour de demande ou d'envoi : il faut redessiner l'écran. (Une
    /// vignette qui se fait sur un fil de fond ne le demande pas : le fil réveille l'écran à sa fin.)
    pub pending: bool,
    pool: Option<Pool>,
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
    thumb_size_of(doc.frame, width, pct)
}

/// La taille d'une vignette de `width` points de large pour un cadre de `frame` pixels.
pub fn thumb_size_of(frame: [usize; 2], width: usize, pct: u32) -> (usize, usize) {
    let tw = scaled(width.clamp(32, 2048), pct, 32);
    let [fw, fh] = frame;
    let th = ((tw as f32 * fh as f32 / fw.max(1) as f32).round() as usize).clamp(8, 2048);
    (tw, th)
}

/// Les vignettes d'une zone de la feuille (la grande case d'un mouvement de caméra, `vignettecam.rs`) se rangent sous
/// l'identifiant du plan marqué de ce bit : elles ne se mêlent pas à la vignette ordinaire du même plan.
const ZONE_BIT: u64 = 1 << 62;

/// Celles d'une case de la planche, avec leur marge autour du cadre (`texture_case`), se rangent sous l'identifiant marqué de ce bit.
const CASE_BIT: u64 = 1 << 61;

/// Les tuiles de l'empreinte d'un mouvement de caméra (`vignettecam::Empreinte`) se rangent sous le numéro de leur rangée, ici (5 bits).
const TUILE_SHIFT: u32 = 55;
const TUILE_MASK: u64 = 0b11111 << TUILE_SHIFT;

/// Largeur de vignette pour `pixels` points physiques à l'écran : par paliers de 128 (on ne la
/// refait pas à chaque cran de zoom), entre 128 et 1024.
pub fn width_for(pixels: f32) -> usize {
    ((pixels / 128.0).ceil().max(1.0) as usize * 128).min(1024)
}

impl Thumbs {
    /// Début d'une image d'écran : le compteur d'images avance (les demandes qu'on ne renouvelle
    /// pas vieillissent) et les vignettes finies arrivent à la carte graphique (au plus
    /// `UPLOAD_TIME` ; le reste attend l'image suivante).
    pub fn begin_frame(&mut self, ctx: &egui::Context) {
        self.pending = false;
        let Some(pool) = &self.pool else { return };
        let shared = pool.shared.clone();
        shared.new_frame();
        let start = Instant::now();
        while let Some(d) = shared.next_done() {
            self.apply(ctx, d);
            if start.elapsed() >= UPLOAD_TIME {
                break;
            }
        }
        self.pending = shared.has_done();
    }

    /// Une image finie : la texture de sa vignette (créée si c'est la première).
    fn apply(&mut self, ctx: &egui::Context, d: Done) {
        let slot = d.slot;
        match self.entries.get_mut(&slot) {
            Some(e) => {
                // (Une réponse plus ancienne que celle qu'on montre déjà ne la remplace pas : deux
                // fils peuvent finir dans le désordre.)
                if d.seq < e.seq {
                    return;
                }
                #[cfg(test)]
                {
                    e.image = d.image.clone();
                }
                e.texture.set(d.image, TextureOptions::LINEAR);
                e.key = d.key;
                e.waiting = d.waiting;
                e.seq = d.seq;
                e.marge = d.marge;
            }
            None => {
                #[cfg(test)]
                let image = d.image.clone();
                let texture = ctx.load_texture(format!("vignette-{}-{}", slot.0, slot.1), d.image, TextureOptions::LINEAR);
                self.entries.insert(
                    slot,
                    Entry {
                        key: d.key,
                        texture,
                        waiting: d.waiting,
                        seq: d.seq,
                        marge: d.marge,
                        #[cfg(test)]
                        image,
                    },
                );
            }
        }
    }

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
    /// d'image : une image de storyboard, 13a). Rend tout de suite la vignette gardée, même
    /// ancienne, ou celle d'une autre taille du même plan ; si elle n'est plus à jour, une demande
    /// part aux fils de fond (si `deadline` n'est pas passée : sinon à l'image suivante).
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
        let plan = render::panel_plan_at(doc, panel, frame, Some(doc.paper));
        let cam = doc.panel_camera(panel, frame);
        self.texture_of(ctx, doc, id, plan, cam, width, doc.frame, deadline)
    }

    /// Vignette de la première image d'un plan **rangé** (une autre version du plan, `planvers.rs` ; la pile des
    /// versions du montage), rangée sous le numéro du plan rangé. Même chemin que les autres : l'interface ne
    /// fait que la clé et le cliché, la composition est sur un fil de fond ; rien tant qu'elle n'est pas prête.
    pub fn texture_rangee(&mut self, ctx: &egui::Context, doc: &Document, panel: &crate::document::Panel, width: usize, deadline: Instant) -> Option<TextureId> {
        let plan = render::stored_panel_plan(doc, panel, 0, Some(doc.paper));
        let cam = doc.camera_du_plan_range(panel, 0);
        self.texture_of(ctx, doc, panel.id, plan, cam, width, doc.frame, deadline)
    }

    /// Vignette d'une **case de la planche** (`boardui.rs`) : l'image `frame` du plan `panel` (ou la zone d'une grande case
    /// de mouvement de caméra), avec une petite marge tout autour du cadre où seul le trait qui déborde est peint (le reste est
    /// transparent) : la case montre un peu du dessin au-delà de son cadre, comme la maquette. Rangée à part de la vignette
    /// ordinaire du même plan. Même chemin que les autres : clé et cliché sur le fil de l'interface, composition sur un fil de
    /// fond. Une **zone** de la feuille (la grande case d'un mouvement de caméra, `vignettecam.rs`) se voit par une caméra droite, sans
    /// zoom, centrée sur la zone, dont le cadre en a la taille : la feuille et ce qu'on a dessiné autour d'elle (toile infinie) s'y
    /// voient (les calques en profondeur gardent leur place vue de la caméra A, au centre de la feuille).
    #[allow(clippy::too_many_arguments)]
    pub fn texture_case(
        &mut self,
        ctx: &egui::Context,
        doc: &Document,
        panel: usize,
        frame: usize,
        id: u64,
        width: usize,
        zone: Option<(&crate::vignettecam::Zone, &[eframe::egui::Pos2])>,
        deadline: Instant,
    ) -> Option<CaseTex> {
        let plan = render::panel_plan_at(doc, panel, frame, Some(doc.paper));
        match zone {
            Some((zone, enveloppe)) => {
                let cadre = [zone.taille.x.round().clamp(1.0, 16384.0) as usize, zone.taille.y.round().clamp(1.0, 16384.0) as usize];
                let cam = Camera { x: zone.centre.x, y: zone.centre.y, zoom: 1.0, angle: 0.0, dolly: 0.0, tilt: 0.0, pan: 0.0 };
                // L'enveloppe, en points du cadre filmé : la tuile ne montre le papier que dedans.
                let moitie = eframe::egui::vec2(cadre[0] as f32, cadre[1] as f32) * 0.5;
                let masque: Vec<eframe::egui::Pos2> = enveloppe.iter().map(|p| ((*p - zone.centre) + moitie).to_pos2()).collect();
                self.texture_of_marge(ctx, doc, id | ZONE_BIT | CASE_BIT | (1 << TUILE_SHIFT), plan, cam, width, cadre, true, Some(masque), deadline)
            }
            None => {
                let cam = doc.panel_camera(panel, frame);
                self.texture_of_marge(ctx, doc, id | CASE_BIT, plan, cam, width, doc.frame, true, None, deadline)
            }
        }
    }

    /// `frame` : la taille du cadre filmé (celle du projet, ou celle d'une zone, `texture_case`).
    #[allow(clippy::too_many_arguments)]
    fn texture_of(&mut self, ctx: &egui::Context, doc: &Document, id: u64, plan: Plan, cam: Camera, width: usize, frame: [usize; 2], deadline: Instant) -> Option<TextureId> {
        self.texture_of_marge(ctx, doc, id, plan, cam, width, frame, false, None, deadline).map(|c| c.id)
    }

    /// Comme `texture_of` ; `avec_marge` : l'image porte une marge autour du cadre (`texture_case`).
    #[allow(clippy::too_many_arguments)]
    fn texture_of_marge(&mut self, ctx: &egui::Context, doc: &Document, id: u64, plan: Plan, cam: Camera, width: usize, frame: [usize; 2], avec_marge: bool, masque: Option<Vec<eframe::egui::Pos2>>, deadline: Instant) -> Option<CaseTex> {
        let _t = crate::perf::section("vignettes : clés et demandes");
        let slot = (id, width);
        // (Plus le nombre de fonds sous le trait arrivés de l'arrière-plan, si le plan montre un fond : la
        // vignette montre le fond d'avant jusqu'à l'arrivée du nouveau. Un plan sans fond ne se refait pas
        // quand le fond d'un autre arrive. Et la taille du cadre, qui donne celle de la vignette.)
        let fonds = plan.items.iter().any(|i| matches!(i.fond, Some(render::FondPart::Forme(_))));
        let versions = plan
            .items
            .iter()
            .map(|i| doc.cels.get(&i.cel).map_or(0, |c| c.version))
            .chain([if fonds { crate::fx::fond_epoch() } else { 0 }, frame[0] as u64, frame[1] as u64])
            // (Et l'enveloppe de la tuile : elle change la forme du papier.)
            .chain(masque.iter().flatten().flat_map(|p| [p.x.to_bits() as u64, p.y.to_bits() as u64]))
            .collect();
        let key = Key::Plan(plan, versions, [cam.x, cam.y, cam.zoom, cam.angle, cam.dolly, cam.tilt, cam.pan].map(f32::to_bits));
        let generation = crate::fxcache::generation();
        let fresh = self.entries.get(&slot).is_some_and(|e| *e.key == key && e.waiting.is_none_or(|g| g == generation));
        if !fresh {
            if Instant::now() < deadline {
                let pool = self.pool.get_or_insert_with(|| Pool::start(ctx, 4));
                pool.request(slot, key, |key| {
                    let Key::Plan(plan, _, _) = key else { unreachable!("une clé de plan") };
                    let mut mini = snapshot(doc, plan);
                    mini.frame = frame;
                    let size = thumb_size_of(frame, width, scale_pct());
                    Work::Plan { doc: mini, cam, size, marge: if avec_marge { marge_px(size.0) } else { 0 }, masque: masque.clone(), generation }
                });
            } else {
                self.pending = true;
            }
        }
        // En attendant : celle-ci, même ancienne, ou une autre taille du même plan.
        self.entries
            .get(&slot)
            .or_else(|| self.entries.iter().find(|(k, _)| k.0 == id).map(|(_, e)| e))
            .map(|e| CaseTex { id: e.texture.id(), marge: e.marge, taille: e.texture.size() })
    }

    pub fn prune(&mut self, doc: &Document) {
        if self.entries.is_empty() {
            return;
        }
        // (Les identifiants des plans et de leurs marques d'abord : chercher chaque vignette parmi
        // tous les plans, à chaque image d'écran, coûtait le carré du nombre de plans.)
        // (Et les plans rangés des versions : les cartes de la pile les demandent.)
        let ids: std::collections::HashSet<u64> =
            doc.panels.iter().flat_map(|p| std::iter::once(p.id).chain(p.marks.iter().map(|m| m.id))).chain(doc.plans_ranges().map(|p| p.id)).collect();
        self.entries.retain(|(id, _), _| ids.contains(&(id & !(ZONE_BIT | CASE_BIT | TUILE_MASK))));
    }

    /// Oublie tout (autre projet, autre résolution) : les vignettes gardées, les demandes en file,
    /// les calculs en cours et les images déjà finies qui n'ont pas été envoyées.
    pub fn clear(&mut self) {
        self.entries.clear();
        if let Some(pool) = &self.pool {
            pool.shared.reset();
        }
    }

    /// Le projet n'est plus affiché (onglet inactif) : les vignettes finies restent, mais les demandes en
    /// file, les calculs en cours et les images pas encore envoyées sont abandonnés — l'onglet inactif
    /// n'occupe plus les fils de fond, et ses demandes ne répondent plus à personne.
    pub fn abandonner(&mut self) {
        self.pending = false;
        if let Some(pool) = &self.pool {
            pool.shared.reset();
        }
    }

    /// Demandes en file, calculs en cours et images finies qui n'ont pas encore été envoyées à la
    /// carte graphique (profileur : « vignettes »).
    pub fn outstanding(&self) -> usize {
        self.pool.as_ref().map_or(0, |p| p.shared.outstanding())
    }

    /// Les fils de fond font-ils encore quelque chose pour les vignettes ? (Essais : attendre.)
    #[cfg(test)]
    pub(crate) fn busy(&self) -> bool {
        self.outstanding() > 0
    }

    /// L'image que la vignette de la case `id` de la planche montre, et sa marge en pixels (essais).
    #[cfg(test)]
    pub(crate) fn case_image_of(&self, id: u64, width: usize) -> Option<(&ColorImage, usize)> {
        self.entries.get(&(id | CASE_BIT, width)).map(|e| (&e.image, e.marge))
    }

    /// L'image que la vignette `slot` montre (essais).
    #[cfg(test)]
    pub(crate) fn image_of(&self, id: u64, width: usize) -> Option<&ColorImage> {
        self.entries.get(&(id, width)).map(|e| &e.image)
    }

    /// La vignette `slot` est là et rien n'y manquait quand elle a été faite (essais).
    #[cfg(test)]
    pub(crate) fn is_complete(&self, id: u64, width: usize) -> bool {
        self.entries.get(&(id, width)).is_some_and(|e| e.waiting.is_none())
    }

    /// Combien de vignettes les fils de fond ont composées (essais).
    #[cfg(test)]
    pub(crate) fn rendered(&self) -> usize {
        self.pool.as_ref().map_or(0, |p| p.shared.rendered.load(Ordering::Relaxed))
    }
}

impl Drop for Thumbs {
    fn drop(&mut self) {
        if let Some(pool) = self.pool.take() {
            pool.stop();
        }
    }
}

// ---------------------------------------------------------------------- le cliché

/// Un projet minuscule qui ne porte que ce dont `render::thumbnail` a besoin pour `plan` : la taille
/// de la feuille et du cadre, et les dessins du plan (bitmap, strate Fond, griffonnages et traits d'un remplissage).
/// Les tuiles sont partagées (`Arc`) : prendre le cliché ne copie aucun pixel, et si l'utilisateur
/// dessine pendant que la vignette se compose, seule la tuile touchée est copiée (copie à la
/// première écriture). Le dessin vectoriel n'est pas copié : la composition ne lit que ses pixels.
pub(crate) fn snapshot(doc: &Document, plan: &Plan) -> Document {
    let mut mini = Document::new(doc.width, doc.height, doc.fps);
    mini.frame = doc.frame;
    mini.paper = doc.paper;
    for item in &plan.items {
        for id in std::iter::once(item.cel).chain(item.ctg.iter().flat_map(|c| c.sources.iter().copied())) {
            if mini.cels.contains_key(&id) {
                continue;
            }
            // (Pas un dessin du projet : un dessin dérivé d'un calque d'effet, ou une image de calque
            // vidéo, que la composition trouve toute seule.)
            if let Some(c) = doc.cels.get(&id) {
                mini.cels.insert(id, Cel { raster: c.raster.clone(), vector: None, version: c.version, fond: c.fond.clone() });
            }
        }
    }
    mini
}

#[cfg(test)]
thread_local! {
    /// Miniatures de dessins composées par ce fil (essais : l'interface n'en compose aucune).
    pub(crate) static CEL_THUMBNAILS_HERE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Miniature d'un dessin : la partie filmée (le cadre `fr` de la feuille), `w` × `h` points, quatre
/// échantillons par point, sur fond blanc (timeline, panneau Calques).
pub(crate) fn cel_thumbnail(raster: &Raster, fr: egui::Rect, w: usize, h: usize) -> ColorImage {
    #[cfg(test)]
    CEL_THUMBNAILS_HERE.with(|n| n.set(n.get() + 1));
    let mut pixels = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
            for (dx, dy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                let sx = fr.min.x + (x as f32 + dx) / w as f32 * fr.width();
                let sy = fr.min.y + (y as f32 + dy) / h as f32 * fr.height();
                let (sx, sy) = (sx as usize, sy as usize);
                let p = if sx < raster.width && sy < raster.height { raster.get(sx, sy) } else { Color32::TRANSPARENT };
                let q = render::over(Color32::WHITE, p, 255, None);
                r += q.r() as u32;
                g += q.g() as u32;
                b += q.b() as u32;
            }
            pixels.push(Color32::from_rgb((r / 4) as u8, (g / 4) as u8, (b / 4) as u8));
        }
    }
    ColorImage { size: [w, h], pixels }
}

// ---------------------------------------------------------------------- les fils de fond

/// Ce qu'un fil compose.
pub(crate) enum Work {
    /// La vignette d'un plan sur son cliché (la clé porte le plan).
    /// `marge` : pixels autour de l'image où seul le dessin (pas le papier) est composé ; l'image entière fait donc
    /// `size` + deux marges.
    Plan { doc: Document, cam: Camera, size: (usize, usize), marge: usize, masque: Option<Vec<eframe::egui::Pos2>>, generation: u64 },
    /// La miniature d'un dessin.
    Cel { raster: Raster, frame: egui::Rect, size: (usize, usize) },
}

/// Une vignette à composer.
struct Job {
    slot: Slot,
    key: Arc<Key>,
    work: Work,
    /// Numéro de la demande.
    seq: u64,
    /// Les demandes d'avant `Shared::reset` (autre projet) n'arrivent à personne.
    era: u64,
    /// Dernière image d'écran où l'interface l'a demandée (ou renouvelée).
    wanted: u64,
    /// Rang de cette demande à cette image (l'ordre dans lequel l'écran les a demandées).
    order: u64,
    cancel: Arc<AtomicBool>,
    /// Le « monde » des effets (`fxcache::world`) au moment de la demande, sur le fil de l'interface : si un
    /// autre onglet est affiché quand le fil compose, la vignette ne lit ni ne demande rien aux effets de
    /// celui-ci (ses numéros de dessin ne sont pas ceux de l'autre projet). Rien : le monde du moment.
    world: Option<u64>,
}

struct Running {
    slot: Slot,
    key: Arc<Key>,
    seq: u64,
    cancel: Arc<AtomicBool>,
}

/// Une vignette finie, à envoyer à la carte graphique.
pub(crate) struct Done {
    pub(crate) slot: Slot,
    pub(crate) key: Arc<Key>,
    pub(crate) seq: u64,
    pub(crate) image: ColorImage,
    /// Quelque chose manquait quand elle a été faite : le compteur de calculs de fond à partir duquel
    /// elle se refait (`Entry::waiting`).
    pub(crate) waiting: Option<u64>,
    /// La marge de l'image autour du cadre (`Work::Plan::marge`).
    pub(crate) marge: usize,
}

#[derive(Default)]
struct Queue {
    waiting: Vec<Job>,
    running: Vec<Running>,
    done: std::collections::VecDeque<Done>,
    /// Image d'écran en cours (`Shared::new_frame`).
    frame: u64,
    order: u64,
    seq: u64,
    era: u64,
    /// Essais : les fils ne prennent rien tant que c'est vrai.
    #[cfg(test)]
    paused: bool,
}

impl Queue {
    fn outstanding(&self) -> usize {
        self.waiting.len() + self.running.len() + self.done.len()
    }

    /// Abandonne les demandes que l'interface n'a pas renouvelées depuis `STALE_FRAMES` images.
    fn drop_stale(&mut self) {
        let frame = self.frame;
        self.waiting.retain(|j| j.wanted + STALE_FRAMES >= frame);
    }

    /// La demande à calculer maintenant : la plus récemment demandée, et dans cette image, la
    /// première demandée par l'écran.
    fn pick(&mut self) -> Option<Job> {
        #[cfg(test)]
        if self.paused {
            return None;
        }
        self.drop_stale();
        let best = self.waiting.iter().enumerate().max_by_key(|(_, j)| (j.wanted, std::cmp::Reverse(j.order))).map(|(i, _)| i)?;
        let job = self.waiting.swap_remove(best);
        self.running.push(Running { slot: job.slot, key: job.key.clone(), seq: job.seq, cancel: job.cancel.clone() });
        Some(job)
    }
}

pub(crate) struct Shared {
    queue: Mutex<Queue>,
    wake: Condvar,
    stop: AtomicBool,
    ctx: egui::Context,
    /// Vignettes composées par les fils depuis le début (essais).
    #[cfg(test)]
    rendered: std::sync::atomic::AtomicUsize,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Queue> {
        self.queue.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Début d'une image d'écran : les demandes qu'on ne renouvelle pas vieillissent.
    pub(crate) fn new_frame(&self) {
        let mut q = self.lock();
        q.frame += 1;
        q.drop_stale();
    }

    /// L'image finie suivante, s'il y en a une.
    pub(crate) fn next_done(&self) -> Option<Done> {
        self.lock().done.pop_front()
    }

    pub(crate) fn has_done(&self) -> bool {
        !self.lock().done.is_empty()
    }

    pub(crate) fn outstanding(&self) -> usize {
        self.lock().outstanding()
    }

    /// La demande pour `slot` de contenu `key` est-elle déjà en file, en calcul ou finie (pas encore
    /// envoyée) ? Alors elle est renouvelée (l'écran la veut encore) et rien d'autre à faire.
    fn touch(&self, slot: Slot, key: &Key) -> bool {
        let mut q = self.lock();
        q.order += 1;
        let (frame, order) = (q.frame, q.order);
        if let Some(j) = q.waiting.iter_mut().find(|j| j.slot == slot && *j.key == *key) {
            j.wanted = frame;
            j.order = order;
            return true;
        }
        // (Un calcul qui vient d'être arrêté, parce qu'une demande plus récente l'a remplacé, n'est pas une
        // réponse : l'écran est revenu au contenu d'avant, il faut le redemander.)
        if q.running.iter().any(|r| r.slot == slot && *r.key == *key && !r.cancel.load(Ordering::Relaxed)) {
            return true;
        }
        // (Une vignette finie après le début de cette image d'écran, pas encore envoyée à la carte
        // graphique, est la réponse : `Thumbs::begin_frame` la pose à l'image suivante. La redemander la
        // ferait composer une seconde fois. Sauf si une demande plus récente pour la même vignette attend,
        // se calcule ou est finie : elle la remplacerait, il faut redemander ce contenu-ci.)
        let newer = |seq: u64| {
            q.waiting.iter().any(|j| j.slot == slot && j.seq > seq)
                || q.running.iter().any(|r| r.slot == slot && r.seq > seq && !r.cancel.load(Ordering::Relaxed))
                || q.done.iter().any(|d| d.slot == slot && d.seq > seq)
        };
        q.done.iter().any(|d| d.slot == slot && *d.key == *key && !newer(d.seq))
    }

    /// Met `job` en file. Celle d'avant pour la même vignette ne vaut plus rien : retirée si elle
    /// attend, arrêtée si elle est en calcul.
    fn push(&self, mut job: Job) {
        let mut q = self.lock();
        q.waiting.retain(|j| j.slot != job.slot);
        for r in q.running.iter().filter(|r| r.slot == job.slot) {
            r.cancel.store(true, Ordering::Relaxed);
        }
        q.seq += 1;
        q.order += 1;
        job.seq = q.seq;
        job.era = q.era;
        job.wanted = q.frame;
        job.order = q.order;
        q.waiting.push(job);
        drop(q);
        self.wake.notify_one();
    }

    /// Autre projet : tout ce qui est demandé, en calcul ou fini ne vaut plus rien.
    pub(crate) fn reset(&self) {
        let mut q = self.lock();
        q.era += 1;
        q.waiting.clear();
        q.done.clear();
        for r in &q.running {
            r.cancel.store(true, Ordering::Relaxed);
        }
    }
}

/// Les fils de fond d'une sorte de vignettes.
pub(crate) struct Pool {
    pub(crate) shared: Arc<Shared>,
    #[cfg(test)]
    threads: Vec<std::thread::JoinHandle<()>>,
}

impl Pool {
    /// `threads` fils (au plus ce que la machine laisse, deux cœurs restent à l'interface et à la carte
    /// graphique ; au moins un).
    pub(crate) fn start(ctx: &egui::Context, threads: usize) -> Pool {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::default()),
            wake: Condvar::new(),
            stop: AtomicBool::new(false),
            ctx: ctx.clone(),
            #[cfg(test)]
            rendered: std::sync::atomic::AtomicUsize::new(0),
        });
        let n = crate::parallele::coeurs().saturating_sub(2).clamp(1, threads.max(1));
        let handles: Vec<std::thread::JoinHandle<()>> = (0..n)
            .filter_map(|k| {
                let shared = shared.clone();
                std::thread::Builder::new().name(format!("vignettes-{k}")).spawn(move || worker(&shared)).ok()
            })
            .collect();
        #[cfg(not(test))]
        drop(handles);
        Pool {
            shared,
            #[cfg(test)]
            threads: handles,
        }
    }

    /// Demande la vignette `slot` de contenu `key`, sauf si la même est déjà attendue (alors la demande est
    /// seulement renouvelée). `work` fait le cliché, seulement s'il faut le faire.
    pub(crate) fn request(&self, slot: Slot, key: Key, work: impl FnOnce(&Key) -> Work) {
        if self.shared.touch(slot, &key) {
            return;
        }
        let work = work(&key);
        self.shared.push(Job { slot, key: Arc::new(key), work, seq: 0, era: 0, wanted: 0, order: 0, cancel: Arc::new(AtomicBool::new(false)), world: Some(crate::fxcache::world()) });
    }

    /// Arrête les fils : les calculs en cours s'interrompent, les autres fils s'éteignent. (Les essais
    /// attendent leur fin : un fil qui survit à son essai toucherait à l'état commun du suivant.)
    pub(crate) fn stop(self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        {
            let q = self.shared.lock();
            for r in &q.running {
                r.cancel.store(true, Ordering::Relaxed);
            }
        }
        self.shared.wake.notify_all();
        #[cfg(test)]
        for t in self.threads {
            let _ = t.join();
        }
    }
}

fn worker(shared: &Shared) {
    loop {
        let job = {
            let mut q = shared.lock();
            loop {
                if shared.stop.load(Ordering::Relaxed) {
                    return;
                }
                if let Some(job) = q.pick() {
                    break job;
                }
                // (Pas de réveil périodique : `push`, `stop` et les essais préviennent, et la file ne change
                // que sous ce verrou.)
                q = shared.wake.wait(q).unwrap_or_else(|e| e.into_inner());
            }
        };
        // Un dessin qui fait planter la composition ne ferme pas SHOT et n'est pas redemandé sans fin :
        // la vignette est alors vide.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| render_job(&job)));
        let finished = match result {
            Ok(r) => r,
            Err(_) => {
                let (w, h) = match &job.work {
                    Work::Plan { size, marge, .. } => (size.0 + 2 * marge, size.1 + 2 * marge),
                    Work::Cel { size, .. } => *size,
                };
                Some((ColorImage { size: [w, h], pixels: vec![Color32::TRANSPARENT; w * h] }, None))
            }
        };
        #[cfg(test)]
        shared.rendered.fetch_add(1, Ordering::Relaxed);
        {
            let mut q = shared.lock();
            q.running.retain(|r| r.seq != job.seq);
            if let Some((image, waiting)) = finished {
                if job.era == q.era && !job.cancel.load(Ordering::Relaxed) {
                    let marge = match &job.work {
                        Work::Plan { marge, .. } => *marge,
                        Work::Cel { .. } => 0,
                    };
                    q.done.push_back(Done { slot: job.slot, key: job.key.clone(), seq: job.seq, image, waiting, marge });
                }
            }
        }
        shared.ctx.request_repaint();
    }
}

/// Compose la vignette de `job`. Un plan comme l'écran l'a toujours fait : en mode doux (les effets pas
/// encore calculés sont demandés au fil de fond des effets et remplacés par leur dernier résultat ou le
/// dessin sans effet ; le fond sous le trait montre celui d'avant jusqu'à l'arrivée du nouveau). Rend
/// l'image, et (si quelque chose manquait) le compteur de calculs à partir duquel elle se refait.
/// `None` : arrêtée, personne ne l'attend plus.
fn render_job(job: &Job) -> Option<(ColorImage, Option<u64>)> {
    match (&job.work, &*job.key) {
        (Work::Plan { doc, cam, size, marge, masque, generation }, Key::Plan(plan, _, _)) => {
            // (Le fil de fond laisse deux cœurs à l'interface, comme celui des effets.)
            render::set_thread_cap(crate::parallele::coeurs().saturating_sub(2).max(1));
            let cancelled = || job.cancel.load(Ordering::Relaxed);
            let compose = || crate::fx::differe(|| crate::fxcache::soft_scope(|| render::thumbnail_marge_until(doc, plan, cam, size.0, size.1, *marge, masque.as_deref(), &cancelled)));
            let (image, complete) = match job.world {
                Some(w) => crate::fxcache::avec_monde(w, compose),
                None => compose(),
            };
            Some((image?, (!complete).then_some(*generation)))
        }
        (Work::Cel { raster, frame, size }, _) => Some((cel_thumbnail(raster, *frame, size.0, size.1), None)),
        _ => None,
    }
}

#[cfg(test)]
#[path = "thumbs_tests.rs"]
mod tests;
