//! La vue nette : au-delà de 100 %, les dessins vectoriels se montrent à la taille de l'écran (partie 166, Anthony, 2026-11-17 : « je
//! trouve le trait très pixélisé alors que c'est du vectoriel »).
//!
//! # Le défaut
//!
//! Un dessin vectoriel garde ses chemins, mais tout ce qui le montre passe par son image (`Cel::raster`), faite à la taille de la
//! feuille ; la vue l'agrandit sans lisser (`render::texture_options`, `gpu.rs` : « pixels nets en grossissant »). À 225 %, un pixel du
//! dessin vaut 2,25 pixels d'écran : le trait vectoriel se voit en marches, comme un bitmap.
//!
//! # Ce que fait ce module
//!
//! Quand la vue grossit au-delà de 100 % (un pixel de la feuille vaut plus d'un pixel d'écran), il compose **la zone que la vue
//! regarde, à la taille de l'écran** : un pixel de l'image = un pixel de l'écran. Dans cette image :
//!
//! - un **dessin vectoriel** est redessiné d'après ses éléments, à cette échelle (`vector::rendre_net` : un trait est rejoué par le
//!   moteur des pinceaux, agrandi, pas ses pixels) ;
//! - **tout le reste** (dessin bitmap, vidéo, aplat sous le trait, couleur unie, remplissage au griffonnage, feuille flottante d'un
//!   geste) garde les pixels de la feuille, grossis sans lissage, exactement comme la vue les montre déjà ;
//! - l'ordre des calques, leur opacité, leur teinte (pelure d'oignon), leur mode de fusion et leur transformation sont ceux du plan
//!   (`render::Plan`), posés comme `render::pixel_of`.
//!
//! L'image est posée **par-dessus** celle de toujours, qui continue d'être composée dessous : elle sert tant que la vue nette n'est
//! pas prête, et là où elle ne l'est plus.
//!
//! # Règles (« sans latence »)
//!
//! - **Rien ne se calcule sur le fil de l'interface** : le plan est figé (`Instantane` : les éléments visibles des dessins vectoriels,
//!   les tuiles partagées des autres), puis composé sur un autre fil (`composer`).
//! - **Rien ne se calcule pendant un geste** : un calcul ne part qu'au repos (ni trait en cours, ni bouton tenu), `ATTENTE` après le
//!   dernier changement ; un geste qui commence arrête celui qui tourne.
//! - **Ce qui change se voit tout de suite** : les tuiles qu'un trait touche sont percées dans l'image nette (on y voit l'image de
//!   toujours, avec le trait en cours) ; quand tout a pu changer (autre image, autre calque, réglage), l'image nette est retirée. Elle
//!   revient au repos.
//! - **La vue qui bouge garde l'image** : elle reste collée au dessin (ses coins sont gardés en points de la feuille) pendant qu'on
//!   déplace, tourne ou zoome ; la suivante la remplace quand elle arrive.
//!
//! # Ce qui n'est pas net (la vue de toujours reste seule)
//!
//! Un plan sans papier (arrière-plan « aucun » ou « damier »), un plan dont un calque a une pile d'effets ou est couvert par un calque
//! d'effet que la carte graphique compose, un calque posé par un maillage (carte inclinée de l'Espace 3D, enveloppe, perspective), la
//! vue en perspective, la lecture, une transition du montage, les cases des Vignettes. Et à 100 % ou moins : rien à faire.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::{self, pos2, Color32, ColorImage, Pos2, Rect, TextureFilter, TextureHandle, TextureId, TextureOptions, Vec2};

use crate::blend::{blend_over, BlendMode};
use crate::document::Document;
use crate::raster::{is_outer_id, outer_key, tiles_across, Raster, TILE};
use crate::render::{FondPart, Plan, PlanItem};
use crate::vector::{Affine, Element};
use crate::view::View;

/// Grossissement (pixels d'écran par pixel de la feuille) à partir duquel la vue nette sert.
pub(crate) const SEUIL: f32 = 1.02;
/// Plus grand côté de l'image nette, en pixels d'écran.
const COTE_MAX: usize = 8192;
/// Repos demandé après le dernier changement (vue ou dessin) avant de lancer un calcul.
const ATTENTE: Duration = Duration::from_millis(70);
/// Lignes composées par morceau (`render::par_chunks`).
const LIGNES: usize = 16;

/// Où l'image nette se pose : ses pixels sont ceux de l'écran.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Cadrage {
    /// Coin haut-gauche de l'image, en pixels de l'écran.
    pub origine: [i64; 2],
    /// Largeur et hauteur de l'image, en pixels.
    pub taille: [usize; 2],
    /// Des pixels de l'image vers la feuille.
    pub vers_feuille: Affine,
}

impl Cadrage {
    /// Le cadrage de la zone `rect` de l'écran (en points) pour la vue `view` d'une feuille de taille `size`, à `ppp` pixels par
    /// point. Rien : la vue ne grossit pas (ou à peine), elle est en perspective, ou la zone est vide ou démesurée.
    pub(crate) fn de(view: &View, rect: Rect, size: Vec2, ppp: f32) -> Option<Cadrage> {
        let grossi = view.zoom * ppp;
        if view.persp.is_some() || !grossi.is_finite() || grossi < SEUIL || !rect.is_finite() {
            return None;
        }
        let (x0, y0) = ((rect.min.x * ppp).floor() as i64, (rect.min.y * ppp).floor() as i64);
        let (x1, y1) = ((rect.max.x * ppp).ceil() as i64, (rect.max.y * ppp).ceil() as i64);
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
        if w > COTE_MAX || h > COTE_MAX {
            return None;
        }
        // (`View::to_canvas`, écrit en clair : la différence de deux points voisins perdrait la précision des flottants.)
        let o = view.to_canvas(rect, size, pos2(x0 as f32 / ppp, y0 as f32 / ppp));
        let k = 1.0 / grossi;
        let (s, c) = view.rotation.sin_cos();
        let f = if view.flip { -1.0 } else { 1.0 };
        let vers_feuille = Affine { a: k * c * f, b: k * s * f, c: -k * s, d: k * c, tx: o.x, ty: o.y };
        [o.x, o.y].iter().all(|v| v.is_finite()).then_some(Cadrage { origine: [x0, y0], taille: [w, h], vers_feuille })
    }

    /// Les quatre coins de l'image (haut-gauche, haut-droit, bas-droit, bas-gauche), en points de la feuille.
    pub(crate) fn coins_sur_la_feuille(&self) -> [Pos2; 4] {
        let [w, h] = [self.taille[0] as f32, self.taille[1] as f32];
        [pos2(0.0, 0.0), pos2(w, 0.0), pos2(w, h), pos2(0.0, h)].map(|p| self.vers_feuille.apply(p))
    }
}

/// Ce qu'il faut peindre : la texture, et où elle se pose.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Peinte {
    pub id: TextureId,
    pub cadrage: Cadrage,
}

/// Le plan peut-il se montrer en vue nette ? (Ce qui ne le peut pas : voir l'en-tête.)
pub(crate) fn admis(plan: &Plan) -> bool {
    plan.paper.is_some() && plan.items.iter().all(|i| i.fx.is_empty() && !matches!(i.fond, Some(FondPart::Reglage(_))) && i.pre.as_ref().map_or(true, |p| p.warp.is_none()))
}

/// L'élément montre-t-il un dessin vectoriel tel quel (ni effet, ni fond, ni remplissage, ni geste en cours) ? Alors son dessin.
fn dessin_net<'a>(doc: &'a Document, item: &PlanItem) -> Option<&'a crate::vector::VectorDrawing> {
    if !item.fx.is_empty() || item.ctg.is_some() || item.fond.is_some() || item.pre.is_some() {
        return None;
    }
    doc.cels.get(&item.cel)?.vector.as_ref()
}

/// Un élément du plan figé pour le fil qui compose.
enum Couche {
    /// Un dessin vectoriel : ses éléments visibles (dans l'ordre), et comment ils vont du dessin à l'image.
    Nette { elements: Vec<Element>, vers_image: Affine, opacity: u8, tint: Option<[u8; 3]>, blend: BlendMode },
    /// Tout le reste : les pixels montrés (tuiles partagées), lus comme `render::pixel_of`.
    Pixels { raster: Raster, inv: Option<Affine>, kx: f32, ky: f32, meme: bool, opacity: u8, tint: Option<[u8; 3]>, blend: BlendMode },
}

/// Le plan figé : ce qu'il faut au fil pour composer l'image nette.
struct Instantane {
    cadrage: Cadrage,
    papier: Color32,
    couches: Vec<Couche>,
}

impl Instantane {
    /// Figé sur le fil de l'interface. **Rien ne se calcule ici** (`fxcache::soft_scope`) : un aplat qui n'est pas prêt est demandé au
    /// fil de fond et manque en attendant, comme à l'écran. Rien : aucun dessin vectoriel n'a d'élément dans la zone (la vue de
    /// toujours montre déjà tout ce qu'il y a à voir).
    fn de(doc: &Document, plan: &Plan, cadrage: Cadrage) -> Option<Instantane> {
        let feuille_vers_image = cadrage.vers_feuille.inverse()?;
        let pivot = doc.stage_center();
        let image = Rect::from_min_max(pos2(0.0, 0.0), pos2(cadrage.taille[0] as f32, cadrage.taille[1] as f32)).expand(2.0);
        let mut net = false;
        let (couches, _) = crate::fxcache::soft_scope(|| {
            let mut couches = Vec::new();
            for item in &plan.items {
                if let Some(v) = dessin_net(doc, item) {
                    let vers_image = match item.affine(pivot) {
                        Some(a) => a.then(&feuille_vers_image),
                        None => feuille_vers_image,
                    };
                    let elements: Vec<Element> = v
                        .elements
                        .iter()
                        .filter(|e| {
                            let b = e.bounds().expand(1.5);
                            b.is_finite() && vers_image.apply_rect(b).intersects(image)
                        })
                        .cloned()
                        .collect();
                    if !elements.is_empty() {
                        net = true;
                        couches.push(Couche::Nette { elements, vers_image, opacity: item.opacity, tint: item.tint, blend: item.blend });
                    }
                    continue;
                }
                let Some(cel) = crate::fx::shown(doc, item) else { continue };
                let r = &cel.raster;
                if r.is_blank() {
                    continue;
                }
                let (kx, ky) = (r.width as f32 / doc.width.max(1) as f32, r.height as f32 / doc.height.max(1) as f32);
                let meme = r.width == doc.width && r.height == doc.height;
                couches.push(Couche::Pixels { raster: r.clone(), inv: crate::render::inverse_of(doc, item), kx, ky, meme, opacity: item.opacity, tint: item.tint, blend: item.blend });
            }
            couches
        });
        net.then(|| Instantane { cadrage, papier: crate::render::paper_of(plan.paper), couches })
    }
}

/// L'image nette de `snap`, ligne par ligne (`taille[0]` pixels par ligne). Rien : arrêté en route (`cancel`).
fn composer(snap: &Instantane, cancel: &AtomicBool) -> Option<Vec<Color32>> {
    let [w, h] = snap.cadrage.taille;
    // 1. Chaque dessin vectoriel sur une image de la taille de l'écran : ses éléments dans l'ordre, un dessin par fil.
    let nets: Vec<usize> = snap.couches.iter().enumerate().filter(|(_, c)| matches!(c, Couche::Nette { .. })).map(|(i, _)| i).collect();
    let rendus: Mutex<Vec<Option<Raster>>> = Mutex::new((0..snap.couches.len()).map(|_| None).collect());
    let suivant = AtomicUsize::new(0);
    let fils = crate::parallele::coeurs().saturating_sub(1).clamp(1, 8).min(nets.len().max(1));
    std::thread::scope(|s| {
        for _ in 0..fils {
            s.spawn(|| loop {
                let Some(&i) = nets.get(suivant.fetch_add(1, Ordering::Relaxed)) else { return };
                let Couche::Nette { elements, vers_image, .. } = &snap.couches[i] else { continue };
                let mut r = Raster::new(w, h);
                for e in elements {
                    if cancel.load(Ordering::Relaxed) {
                        return;
                    }
                    crate::vector::rendre_net(&mut r, e, vers_image);
                }
                if let Ok(mut rendus) = rendus.lock() {
                    rendus[i] = Some(r);
                }
            });
        }
    });
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    let rendus = rendus.into_inner().ok()?;
    // 2. La composition : le papier, puis chaque couche, pixel d'écran par pixel d'écran.
    let vf = snap.cadrage.vers_feuille;
    let mut out = vec![snap.papier; w * h];
    crate::render::par_chunks(&mut out, w * LIGNES, |morceau, lignes| {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        for (k, px) in lignes.iter_mut().enumerate() {
            let (u, v) = (k % w, morceau * LIGNES + k / w);
            let p = vf.apply(pos2(u as f32 + 0.5, v as f32 + 0.5));
            // Le pixel de la feuille (ou de la toile autour d'elle) sous ce pixel d'écran.
            let (x, y) = (p.x.floor() as i64, p.y.floor() as i64);
            let ou = (crate::render::world_dither(x), crate::render::world_dither(y));
            let mut c = *px;
            for (i, couche) in snap.couches.iter().enumerate() {
                let (src, opacity, tint, blend) = match couche {
                    Couche::Nette { opacity, tint, blend, .. } => (rendus[i].as_ref().map_or(Color32::TRANSPARENT, |r| r.get(u, v)), *opacity, *tint, *blend),
                    Couche::Pixels { raster, inv, kx, ky, meme, opacity, tint, blend } => {
                        let src = match inv {
                            Some(inv) => {
                                let q = inv.apply(pos2(x as f32 + 0.5, y as f32 + 0.5));
                                crate::render::sample(raster, q.x * kx, q.y * ky)
                            }
                            None if *meme => raster.get_i(x, y),
                            None if x < 0 || y < 0 => Color32::TRANSPARENT,
                            None => raster.get(((x as f32 + 0.5) * kx) as usize, ((y as f32 + 0.5) * ky) as usize),
                        };
                        (src, *opacity, *tint, *blend)
                    }
                };
                if src.a() != 0 {
                    c = blend_over(c, src, opacity, tint, blend, ou);
                }
            }
            *px = c;
        }
    });
    (!cancel.load(Ordering::Relaxed)).then_some(out)
}

/// (Essais.) L'image nette du plan `plan` pour ce cadrage, composée tout de suite sur le fil appelant ; rien : aucun dessin
/// vectoriel dans la zone.
#[cfg(test)]
pub(crate) fn image_pour_essai(doc: &Document, plan: &Plan, cadrage: Cadrage) -> Option<Vec<Color32>> {
    Instantane::de(doc, plan, cadrage).and_then(|s| composer(&s, &AtomicBool::new(false)))
}

/// Ce que le fil qui compose rend.
enum Rendu {
    Image(Vec<Color32>),
    /// Arrêté en route (un geste a commencé, le plan a changé).
    Arrete,
    /// Le calcul a échoué (une panique rattrapée) : on ne le relance pas pour ce plan et ce cadrage.
    Echec,
}

/// Une image nette qui se compose sur un autre fil.
struct Job {
    plan: Plan,
    cadrage: Cadrage,
    rx: Receiver<Rendu>,
    cancel: Arc<AtomicBool>,
}

impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Ce que la texture montre.
struct Montre {
    plan: Plan,
    cadrage: Cadrage,
    /// Des tuiles y ont été percées depuis qu'elle est arrivée : elle est à refaire.
    troue: bool,
}

/// L'image nette à l'écran, et celle qui se prépare.
pub(crate) struct VueNette {
    texture: Option<TextureHandle>,
    taille: [usize; 2],
    montre: Option<Montre>,
    job: Option<Job>,
    /// Ce qui a changé depuis le départ du calcul en cours : tout, ou ces tuiles.
    depuis_tout: bool,
    depuis: Vec<usize>,
    /// Le dernier cadrage demandé, et l'instant du dernier changement (vue ou dessin).
    demande: Option<Cadrage>,
    change: Instant,
    /// Rien de net à montrer pour ce plan et ce cadrage (aucun dessin vectoriel dans la zone) : on ne refige pas le plan à chaque image.
    rien: Option<(Plan, Cadrage)>,
    /// Images nettes composées depuis le début (mesures, essais).
    pub(crate) composees: u64,
}

fn options() -> TextureOptions {
    TextureOptions { magnification: TextureFilter::Nearest, minification: TextureFilter::Linear, ..TextureOptions::LINEAR }
}

/// Les coordonnées de tuile d'un numéro de tuile d'un dessin d'une feuille de `width` pixels de large (dans la feuille, ou hors d'elle).
fn tuile_de(width: usize, id: usize) -> (i64, i64) {
    if is_outer_id(id) {
        let (tx, ty) = outer_key(id);
        (tx as i64, ty as i64)
    } else {
        let across = tiles_across(width).max(1);
        ((id % across) as i64, (id / across) as i64)
    }
}

impl Default for VueNette {
    fn default() -> Self {
        Self::new()
    }
}

impl VueNette {
    pub(crate) fn new() -> Self {
        Self { texture: None, taille: [0, 0], montre: None, job: None, depuis_tout: false, depuis: Vec::new(), demande: None, change: Instant::now(), rien: None, composees: 0 }
    }

    /// Un calcul tourne-t-il ? (Essais.)
    #[cfg(test)]
    pub(crate) fn en_calcul(&self) -> bool {
        self.job.is_some()
    }

    /// Perce dans l'image montrée les tuiles `tuiles` du dessin (leur rectangle sur la feuille, ramené dans l'image, à un pixel
    /// près) : on y voit l'image de toujours, qui a déjà ce qui vient de changer.
    fn percer(&mut self, largeur_de_la_feuille: usize, tuiles: &[usize]) {
        let (Some(m), Some(tex)) = (&mut self.montre, &mut self.texture) else { return };
        let Some(vers_image) = m.cadrage.vers_feuille.inverse() else { return };
        let [w, h] = m.cadrage.taille;
        let t = TILE as f32;
        for &id in tuiles {
            let (tx, ty) = tuile_de(largeur_de_la_feuille, id);
            let sur_la_feuille = Rect::from_min_size(pos2(tx as f32 * t, ty as f32 * t), egui::vec2(t, t));
            let r = vers_image.apply_rect(sur_la_feuille).expand(1.0);
            if !r.is_finite() {
                continue;
            }
            let (x0, y0) = (r.min.x.floor().clamp(0.0, w as f32) as usize, r.min.y.floor().clamp(0.0, h as f32) as usize);
            let (x1, y1) = (r.max.x.ceil().clamp(0.0, w as f32) as usize, r.max.y.ceil().clamp(0.0, h as f32) as usize);
            if x1 <= x0 || y1 <= y0 {
                continue;
            }
            let (bw, bh) = (x1 - x0, y1 - y0);
            tex.set_partial([x0, y0], ColorImage { size: [bw, bh], pixels: bytemuck::zeroed_vec(bw * bh) }, options());
            m.troue = true;
        }
    }

    /// Met la vue nette à jour pour cette image d'écran et rend ce qu'il faut peindre par-dessus l'image de toujours (rien : elle
    /// seule). `cadrage` : celui que la vue demande (rien : la vue nette ne sert pas, `Cadrage::de`) ; `repos` : aucun geste en
    /// cours ; `tout`, `tuiles` : ce qui a changé dans les dessins depuis la dernière fois (`Renderer::take_net_dirty`).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn update(&mut self, ctx: &egui::Context, doc: &Document, plan: &Plan, cadrage: Option<Cadrage>, repos: bool, tout: bool, tuiles: &[usize]) -> Option<Peinte> {
        let maintenant = Instant::now();
        // 1. Ce qui a changé depuis la dernière image.
        if tout || !tuiles.is_empty() {
            self.change = maintenant;
            self.rien = None;
        }
        if tout {
            self.montre = None;
        } else {
            self.percer(doc.width, tuiles);
        }
        if self.job.is_some() {
            self.depuis_tout |= tout;
            self.depuis.extend_from_slice(tuiles);
        }
        if self.montre.as_ref().is_some_and(|m| m.plan != *plan) {
            self.montre = None;
            self.change = maintenant;
        }
        // Un geste qui commence, un autre plan : le calcul en cours ne servira pas, et ne doit rien prendre au geste.
        if self.job.as_ref().is_some_and(|j| !repos || j.plan != *plan) {
            self.arreter_le_calcul();
        }
        if cadrage != self.demande {
            self.demande = cadrage;
            self.change = maintenant;
        }
        // 2. L'image qui arrive.
        let recu = self.job.as_ref().and_then(|j| match j.rx.try_recv() {
            Ok(r) => Some((r, j.plan.clone(), j.cadrage)),
            // (Le fil est parti sans rien rendre : comme un échec.)
            Err(TryRecvError::Disconnected) => Some((Rendu::Echec, j.plan.clone(), j.cadrage)),
            Err(TryRecvError::Empty) => None,
        });
        match recu {
            Some((Rendu::Image(pixels), plan_du_calcul, c)) => {
                let (perime, depuis) = (self.depuis_tout, std::mem::take(&mut self.depuis));
                self.arreter_le_calcul();
                if !perime {
                    let image = ColorImage { size: c.taille, pixels };
                    match &mut self.texture {
                        Some(t) if self.taille == c.taille => t.set(image, options()),
                        _ => self.texture = Some(ctx.load_texture("vue nette", image, options())),
                    }
                    self.taille = c.taille;
                    self.montre = Some(Montre { plan: plan_du_calcul, cadrage: c, troue: false });
                    self.composees += 1;
                    // (Ce qui a changé pendant le calcul n'y est pas.)
                    self.percer(doc.width, &depuis);
                }
            }
            Some((Rendu::Arrete, _, _)) => self.arreter_le_calcul(),
            Some((Rendu::Echec, plan_du_calcul, c)) => {
                // Pas de vue nette pour ce plan et ce cadrage (l'image de toujours reste) : elle sera retentée quand quelque chose changera.
                self.arreter_le_calcul();
                self.montre = None;
                self.rien = Some((plan_du_calcul, c));
            }
            None => {}
        }
        // 3. Faut-il une image, et peut-on la lancer ?
        let Some(c) = cadrage.filter(|_| admis(plan)) else {
            self.arreter_le_calcul();
            self.montre = None;
            return None;
        };
        let a_jour = self.montre.as_ref().is_some_and(|m| m.cadrage == c && !m.troue);
        let en_route = self.job.as_ref().is_some_and(|j| j.cadrage == c && !self.depuis_tout && self.depuis.is_empty());
        let rien = self.rien.as_ref().is_some_and(|(p, k)| *k == c && p == plan);
        if !a_jour && !en_route && !rien {
            let attendu = maintenant.duration_since(self.change);
            if repos && attendu >= ATTENTE {
                self.arreter_le_calcul();
                match Instantane::de(doc, plan, c) {
                    Some(snap) => {
                        let (tx, rx) = channel();
                        let cancel = Arc::new(AtomicBool::new(false));
                        let (arret, reveil) = (cancel.clone(), ctx.clone());
                        let lance = std::thread::Builder::new().name("vue nette".into()).spawn(move || {
                            // (Une panique du rejeu d'un trait ne doit ni fermer FEANOR ni relancer le même calcul sans fin.)
                            let rendu = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| composer(&snap, &arret))) {
                                Ok(Some(pixels)) => Rendu::Image(pixels),
                                Ok(None) => Rendu::Arrete,
                                Err(_) => Rendu::Echec,
                            };
                            let _ = tx.send(rendu);
                            reveil.request_repaint();
                        });
                        if lance.is_ok() {
                            self.job = Some(Job { plan: plan.clone(), cadrage: c, rx, cancel });
                        }
                    }
                    None => {
                        // Aucun dessin vectoriel dans la zone : l'image de toujours suffit.
                        self.montre = None;
                        self.rien = Some((plan.clone(), c));
                    }
                }
            } else if repos {
                // (Au repos, on revient voir quand l'attente sera passée ; pendant un geste, rien : sa fin réveille l'écran.)
                ctx.request_repaint_after(ATTENTE.saturating_sub(attendu).max(Duration::from_millis(10)));
            }
        }
        let id = self.texture.as_ref()?.id();
        self.montre.as_ref().map(|m| Peinte { id, cadrage: m.cadrage })
    }

    fn arreter_le_calcul(&mut self) {
        self.job = None;
        self.depuis_tout = false;
        self.depuis.clear();
    }
}

#[cfg(test)]
thread_local! {
    /// (Essais.) La vue nette de l'application sert-elle sur ce fil d'essai ? Non d'origine : les milliers d'essais du banc
    /// (`perf_tests::Banc`) tournent sur de petites feuilles, souvent au-delà de 100 %, avec une horloge simulée, et beaucoup
    /// contrôlent qu'au repos rien ne demande à être repeint ; un fil de fond qui réveille l'écran à sa fin (comme les vignettes)
    /// les rendrait instables. Les essais de la vue nette l'allument (`activer_pour_essai`).
    static ACTIVE_POUR_ESSAI: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// (Essais.) Allume la vue nette de l'application pour les bancs de ce fil d'essai.
#[cfg(test)]
pub(crate) fn activer_pour_essai() {
    ACTIVE_POUR_ESSAI.with(|a| a.set(true));
}

impl crate::app::App {
    /// La vue nette du dessin pour cette image d'écran (`VueNette::update`) : ce qu'il faut peindre par-dessus l'image composée, ou
    /// rien. `rect` : la zone du dessin à l'écran ; `size` : la taille de la feuille ; `plan` : ce que l'écran compose.
    pub(crate) fn vue_nette(&mut self, ctx: &egui::Context, rect: Rect, size: Vec2, plan: &Plan) -> Option<Peinte> {
        #[cfg(test)]
        if !ACTIVE_POUR_ESSAI.with(|a| a.get()) {
            return None;
        }
        let (tout, tuiles) = self.renderer.take_net_dirty();
        let cadrage = if self.playing { None } else { Cadrage::de(&self.view, rect, size, ctx.pixels_per_point()) };
        let repos = self.stroke.is_none() && !ctx.input(|i| i.pointer.any_down());
        self.nette.update(ctx, &self.doc, plan, cadrage, repos, tout, &tuiles)
    }

    /// Peint l'image nette `n` : à sa place exacte, pixel pour pixel, quand la vue est celle où elle a été composée ; sinon collée au
    /// dessin (ses coins, gardés sur la feuille, suivent la vue qui bouge).
    pub(crate) fn peindre_vue_nette(&self, painter: &egui::Painter, rect: Rect, size: Vec2, n: Peinte) {
        let ppp = painter.ctx().pixels_per_point();
        let [w, h] = [n.cadrage.taille[0] as f32, n.cadrage.taille[1] as f32];
        let coins = if Cadrage::de(&self.view, rect, size, ppp) == Some(n.cadrage) {
            let o = pos2(n.cadrage.origine[0] as f32 / ppp, n.cadrage.origine[1] as f32 / ppp);
            [o, o + egui::vec2(w / ppp, 0.0), o + egui::vec2(w / ppp, h / ppp), o + egui::vec2(0.0, h / ppp)]
        } else {
            n.cadrage.coins_sur_la_feuille().map(|p| self.view.to_screen(rect, size, p))
        };
        let mut mesh = egui::Mesh::with_texture(n.id);
        for (pos, uv) in coins.into_iter().zip([pos2(0.0, 0.0), pos2(1.0, 0.0), pos2(1.0, 1.0), pos2(0.0, 1.0)]) {
            mesh.vertices.push(egui::epaint::Vertex { pos, uv, color: Color32::WHITE });
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        painter.add(egui::Shape::mesh(mesh));
    }
}
