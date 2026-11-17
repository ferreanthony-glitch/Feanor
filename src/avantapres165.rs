//! Partie 165 : l'avant | après du panneau Effets montre **le vrai calque** (Anthony : à gauche de la barre, le calque sans l'effet ;
//! à droite, le calque avec l'effet, à la force courante). La partie 164 n'en montrait qu'un schéma (un trait sur un papier,
//! `visuels164::dessin_effet`), qui reste le repli : calque vide, calque sans dessin (vidéo, calque d'effet), effet qui ne se calcule
//! pas en petit (une déformation, un effet du trait), ou aperçu pas encore arrivé.
//!
//! Règle « sans latence » : rien n'est calculé sur le fil de l'interface. Les deux moitiés sont des dessins dérivés comme les autres,
//! demandés au fil de fond du cache des effets (`fxcache::request`), **en petit** (le dessin réduit `1 << niveau` fois, à peu près la
//! largeur du panneau, `fxproxy::downscale`, les réglages en pixels mis à l'échelle) ; le dessin d'origine est la copie que le cache
//! garde déjà pour l'écran (`fxcache::source`, des tuiles partagées : rien n'est recopié). Tant que le calcul n'est pas arrivé,
//! l'image d'avant reste (pendant qu'on tire la barre : la dernière prête), puis l'écran se redessine quand il arrive. La demande passe
//! après l'image affichée (priorité `PRIO`), et aucune n'est faite pendant la lecture ni pendant un trait : rien ne ralentit le
//! dessin ni la lecture. Une fois arrivée, l'image est gardée ici en texture (quelques centaines de pixels de large), recadrée sur ce
//! qui est dessiné.

use std::hash::{Hash, Hasher};

use eframe::egui::{self, pos2, vec2, Color32, ColorImage, Rect, TextureHandle};

use crate::document::{Cel, CelId, Document, Layer, LayerKind};
use crate::fx::{Effect, FxStep};
use crate::raster::{Raster, TILE};

/// La priorité des demandes de l'aperçu : après l'image affichée (0) et les quelques suivantes de la lecture, avant les vignettes (100).
pub(crate) const PRIO: u32 = 50;

/// Le niveau de réduction (le dessin réduit `1 << niveau` fois) pour une feuille de `largeur` pixels montrée sur `cible` pixels d'écran :
/// le plus réduit qui reste au moins aussi large que la cible (au plus 5, un trente-deuxième).
pub(crate) fn niveau(largeur: usize, cible: f32) -> u8 {
    let mut l = 0u8;
    while l < 5 && ((largeur >> (l + 1)) as f32) >= cible.max(1.0) {
        l += 1;
    }
    l
}

/// Les deux piles d'effets à montrer pour l'effet `i` du calque à l'image `frame` : l'avant (la pile sans lui) et l'après (la pile avec
/// lui, allumé, à ses réglages de cette image). Les effets du trait (qui lisent les traits vectoriels en pleine résolution) sont laissés.
/// Rien si l'effet n'existe pas, s'il est lui-même un effet du trait ou de la matière (l'avant et l'après seraient pareils), ou si
/// l'après ne se calcule pas en petit (une déformation, un liseré : `fxproxy::proxyable`).
pub(crate) fn piles(layer: &Layer, i: usize, frame: usize) -> Option<(Vec<FxStep>, Vec<FxStep>)> {
    if layer.fx.get(i)?.kind.is_trait() {
        return None;
    }
    let vector = layer.kind == LayerKind::Vector;
    let mut sans: Vec<Effect> = layer.fx.clone();
    sans.get_mut(i)?.enabled = false;
    let mut avec = layer.fx.clone();
    avec[i].enabled = true;
    let pile = |fx: &[Effect]| -> Vec<FxStep> { crate::fx::steps_at_with(fx, frame, None, vector).into_iter().filter(|s| !s.kind.vector_stage()).collect() };
    let (a, b) = (pile(&sans), pile(&avec));
    if !crate::fxproxy::proxyable(&b) {
        return None;
    }
    Some((a, b))
}

/// Le morceau de la feuille (en pixels de la feuille) que montre l'aperçu : ce qui est dessiné (`encre`, tuiles comprises), avec une
/// marge pour ce que l'effet étend autour (une lueur, une ombre), au moins un bon tiers de la feuille, et de la forme de l'aperçu
/// (`aspect` = largeur / hauteur) tant que la feuille le permet. Rien si rien n'est dessiné dans la feuille.
pub(crate) fn cadrage(encre: Option<(i32, i32, i32, i32)>, feuille: (usize, usize), aspect: f32) -> Option<Rect> {
    let (fw, fh) = (feuille.0 as f32, feuille.1 as f32);
    let (tx0, ty0, tx1, ty1) = encre?;
    let t = TILE as f32;
    let dessin = Rect::from_min_max(pos2(tx0 as f32 * t, ty0 as f32 * t), pos2((tx1 + 1) as f32 * t, (ty1 + 1) as f32 * t));
    let dessin = dessin.intersect(Rect::from_min_size(pos2(0.0, 0.0), vec2(fw, fh)));
    if !dessin.is_positive() {
        return None;
    }
    let marge = fw.max(fh) * 0.06;
    let mut w = (dessin.width() + 2.0 * marge).max(fw * 0.35);
    let mut h = dessin.height() + 2.0 * marge;
    let aspect = aspect.max(0.1);
    if w / h < aspect {
        w = h * aspect;
    } else {
        h = w / aspect;
    }
    // (Plus grand que la feuille : réduit à elle, la forme gardée tant qu'un côté au moins tient.)
    if w > fw {
        w = fw;
        h = (w / aspect).min(fh);
    }
    if h > fh {
        h = fh;
        w = (h * aspect).min(fw);
    }
    let c = dessin.center();
    let x0 = (c.x - w * 0.5).clamp(0.0, fw - w);
    let y0 = (c.y - h * 0.5).clamp(0.0, fh - h);
    Some(Rect::from_min_size(pos2(x0, y0), vec2(w, h)))
}

/// Les pixels de `cadre` (en pixels de la feuille) pris dans `image`, le dessin dérivé réduit `1 << niveau` fois (prémultipliés, comme
/// ceux d'egui).
pub(crate) fn image_du_cadre(image: &Raster, cadre: Rect, niveau: u8) -> ColorImage {
    let s = 1.0 / (1u32 << niveau) as f32;
    let x0 = (cadre.left() * s).floor().max(0.0) as usize;
    let y0 = (cadre.top() * s).floor().max(0.0) as usize;
    let x1 = ((cadre.right() * s).ceil() as usize).min(image.width).max(x0 + 1);
    let y1 = ((cadre.bottom() * s).ceil() as usize).min(image.height).max(y0 + 1);
    let (w, h) = (x1 - x0, y1 - y0);
    let mut pixels = Vec::with_capacity(w * h);
    for y in y0..y1 {
        for x in x0..x1 {
            pixels.push(if x < image.width && y < image.height { image.get(x, y) } else { Color32::TRANSPARENT });
        }
    }
    ColorImage { size: [w, h], pixels }
}

/// Le numéro d'une moitié de l'aperçu : le dessin `cel` passé par `steps`, réduit au niveau `niveau` (bit de poids fort : jamais un vrai
/// dessin, ni un dessin dérivé de l'écran).
fn numero(cel: CelId, steps: &[FxStep], niveau: u8) -> CelId {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    crate::fx::fx_id(cel, steps).hash(&mut h);
    "avant | après, partie 165".hash(&mut h);
    niveau.hash(&mut h);
    h.finish() | (1 << 63)
}

/// Le dessin d'origine sous lequel le cache range le dernier résultat de l'aperçu : un numéro à lui, pour que l'image de l'écran, qui
/// montre en attendant le dernier résultat de son dessin (`fxcache::fallback`), ne prenne jamais cette petite image.
fn origine(cel: CelId) -> CelId {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    cel.hash(&mut h);
    "origine de l'avant | après".hash(&mut h);
    h.finish() | (1 << 63)
}

/// Une moitié arrivée : de quel calcul elle vient, sa texture (et, pour les essais, son image).
struct Moitie {
    cle: (CelId, u64, [i32; 4]),
    tex: TextureHandle,
    #[cfg(test)]
    image: ColorImage,
}

/// Ce que l'avant | après montre : pour quel calque, quel effet et quel dessin, et ses deux moitiés (la séance seulement, par onglet).
#[derive(Default)]
pub(crate) struct Apercu {
    pour: Option<(u64, usize, Option<CelId>)>,
    avant: Option<Moitie>,
    apres: Option<Moitie>,
    /// La dernière demande faite au fil de fond pour chaque moitié (numéro, version).
    demande_avant: Option<(CelId, u64)>,
    demande_apres: Option<(CelId, u64)>,
    /// Les deux moitiés montrées sont celles des réglages d'aujourd'hui (pas une image d'avant le dernier geste).
    a_jour: bool,
}

/// L'aperçu à peindre : les deux textures, et le morceau de l'aperçu qu'elles couvrent (relatif, 0 à 1).
pub(crate) struct Vue {
    pub avant: egui::TextureId,
    pub apres: egui::TextureId,
    pub place: Rect,
}

/// Ce qu'il faut pour préparer l'aperçu (tout est lu, rien n'est écrit dans le projet).
pub(crate) struct Demande<'a> {
    pub doc: &'a Document,
    pub layer: &'a Layer,
    pub effet: usize,
    pub frame: usize,
    /// La taille de l'aperçu en pixels d'écran.
    pub taille_px: egui::Vec2,
    /// Une demande au fil de fond est permise (ni la lecture, ni un trait en cours).
    pub permis: bool,
}

impl Apercu {
    /// Prépare l'aperçu de l'effet `d.effet` du calque `d.layer`, sans rien calculer ici : prend les moitiés arrivées au cache, demande
    /// les autres au fil de fond. Rend ce qu'il faut peindre, ou rien (le schéma) tant qu'aucune image de ce calque n'est là.
    pub(crate) fn preparer(&mut self, ctx: &egui::Context, d: &Demande) -> Option<Vue> {
        #[cfg(test)]
        let avant = crate::fx::calculs_sur_ce_fil();
        let vue = self.preparer_sans_compter(ctx, d);
        #[cfg(test)]
        CALCULS_DE_L_APERCU.with(|c| c.set(c.get() + crate::fx::calculs_sur_ce_fil() - avant));
        vue
    }

    fn preparer_sans_compter(&mut self, ctx: &egui::Context, d: &Demande) -> Option<Vue> {
        let aspect = d.taille_px.x / d.taille_px.y.max(1.0);
        // (Pendant la lecture ou un trait : l'aperçu montré reste tel quel, rien n'est lu au cache, converti ni envoyé.)
        if !d.permis && self.pour.is_some_and(|p| p.0 == d.layer.id && p.1 == d.effet) && self.avant.is_some() && self.apres.is_some() {
            return self.vue(aspect);
        }
        let cel_id = d.layer.shown_at(d.frame);
        // (Un autre calque, un autre effet ou un autre dessin : les images d'avant ne le montrent pas, on repart du schéma.)
        let pour = (d.layer.id, d.effet, cel_id);
        if self.pour != Some(pour) {
            *self = Apercu { pour: Some(pour), ..Default::default() };
        }
        let rien = |s: &mut Apercu| {
            s.avant = None;
            s.apres = None;
            s.a_jour = false;
            None
        };
        let Some(cel_id) = cel_id else { return rien(self) };
        let Some(cel) = d.doc.cels.get(&cel_id) else { return rien(self) };
        let raster = &cel.raster;
        let Some(cadre) = cadrage(raster.drawn_tile_range(), (raster.width, raster.height), aspect) else { return rien(self) };
        let Some((pile_avant, pile_apres)) = piles(d.layer, d.effet, d.frame) else { return rien(self) };
        // (Le cadre, en pixels de la feuille, à peu près à la largeur de l'aperçu ; une grande feuille toujours réduite au moins de moitié :
        // le fil de fond, qui sert aussi la vue de dessin, ne calcule jamais l'aperçu en pleine résolution.)
        let cible = 0.75 * d.taille_px.x * raster.width as f32 / cadre.width().max(1.0);
        let niv = niveau(raster.width, cible).max(if raster.width >= 1024 { 1 } else { 0 });
        let cle_cadre = [cadre.left() as i32, cadre.top() as i32, cadre.right() as i32, cadre.bottom() as i32];
        let mut a_jour = true;
        for (avant, pile) in [(true, &pile_avant), (false, &pile_apres)] {
            let id = numero(cel_id, pile, niv);
            let cle = (id, cel.version, cle_cadre);
            let (moitie, demandee) = if avant { (&mut self.avant, &mut self.demande_avant) } else { (&mut self.apres, &mut self.demande_apres) };
            if moitie.as_ref().is_some_and(|m| m.cle == cle) {
                continue;
            }
            let nom = if avant { "avant | après : avant" } else { "avant | après : après" };
            match crate::fxcache::get(id, cel.version) {
                Some(fait) => poser(moitie, ctx, nom, cle, &fait.raster, cadre, niv),
                None => {
                    a_jour = false;
                    // (Pendant le geste : la force demandée il y a quelques images est arrivée, elle se montre en attendant la dernière.)
                    if let Some((i, v)) = *demandee {
                        let deja = moitie.as_ref().is_some_and(|m| m.cle.0 == i && m.cle.1 == v && m.cle.2 == cle_cadre);
                        if i != id && v == cel.version && !deja {
                            if let Some(fait) = crate::fxcache::get(i, v) {
                                poser(moitie, ctx, nom, (i, v, cle_cadre), &fait.raster, cadre, niv);
                            }
                        }
                    }
                    // Une seule demande en cours par moitié : pendant qu'on tire la barre, la force d'il y a quelques images se finit, puis la
                    // dernière est demandée (pas une file de forces périmées). Une demande encore en file est renouvelée, sans rien recopier.
                    let en_cours = demandee.is_some_and(|(i, v)| crate::fxcache::renew(i, v, PRIO));
                    if d.permis && !en_cours {
                        crate::fxcache::request(job(cel_id, cel, id, pile.clone(), niv));
                        *demandee = Some((id, cel.version));
                    }
                }
            }
        }
        self.a_jour = a_jour;
        self.vue(aspect)
    }

    /// Ce qu'il faut peindre avec les deux moitiés gardées, dans un aperçu de forme `aspect` (rien s'il en manque une).
    fn vue(&self, aspect: f32) -> Option<Vue> {
        let (avant, apres) = (self.avant.as_ref()?, self.apres.as_ref()?);
        // (La place de l'image dans l'aperçu : toute, sauf quand la feuille est trop étroite ou trop basse pour sa forme.)
        let [w, h] = apres.tex.size();
        let forme = w as f32 / h.max(1) as f32;
        let place = if forme > aspect {
            let hh = aspect / forme;
            Rect::from_min_max(pos2(0.0, 0.5 - hh * 0.5), pos2(1.0, 0.5 + hh * 0.5))
        } else {
            let ww = forme / aspect;
            Rect::from_min_max(pos2(0.5 - ww * 0.5, 0.0), pos2(0.5 + ww * 0.5, 1.0))
        };
        Some(Vue { avant: avant.tex.id(), apres: apres.tex.id(), place })
    }

    /// Les images montrées (avant, après), pour les essais.
    #[cfg(test)]
    pub(crate) fn images(&self) -> Option<(&ColorImage, &ColorImage)> {
        Some((&self.avant.as_ref()?.image, &self.apres.as_ref()?.image))
    }

    /// Les deux moitiés sont celles des réglages d'aujourd'hui.
    #[cfg(test)]
    pub(crate) fn a_jour(&self) -> bool {
        self.a_jour
    }
}

/// Pose dans `moitie` l'image `cadre` du dessin dérivé `fait` (réduit au niveau `niv`) : la texture est remplacée, ou créée.
#[allow(clippy::too_many_arguments)]
fn poser(moitie: &mut Option<Moitie>, ctx: &egui::Context, nom: &str, cle: (CelId, u64, [i32; 4]), fait: &Raster, cadre: Rect, niv: u8) {
    let image = image_du_cadre(fait, cadre, niv);
    match moitie {
        Some(m) => {
            #[cfg(test)]
            {
                m.image = image.clone();
            }
            m.tex.set(image, egui::TextureOptions::LINEAR);
            m.cle = cle;
        }
        None => {
            #[cfg(test)]
            let copie = image.clone();
            let tex = ctx.load_texture(nom, image, egui::TextureOptions::LINEAR);
            *moitie = Some(Moitie {
                cle,
                tex,
                #[cfg(test)]
                image: copie,
            });
        }
    }
}

#[cfg(test)]
thread_local! {
    /// Essais : les effets et réductions calculés sur ce fil pendant la préparation de l'aperçu (toujours zéro : tout part au fil de fond).
    static CALCULS_DE_L_APERCU: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Essais : les calculs faits par l'aperçu sur ce fil jusqu'ici.
#[cfg(test)]
pub(crate) fn calculs_de_l_apercu() -> u64 {
    CALCULS_DE_L_APERCU.with(std::cell::Cell::get)
}

/// La demande au fil de fond pour une moitié.
fn job(cel_id: CelId, cel: &Cel, id: CelId, steps: Vec<FxStep>, niveau: u8) -> crate::fxcache::Job {
    crate::fxcache::Job {
        id,
        version: cel.version,
        src: crate::fxcache::source(cel_id, cel),
        vector: None,
        steps,
        level: niveau,
        prio: PRIO,
        epoch: crate::fxcache::epoch(),
        origin: origine(cel_id),
        seq: 0,
        world: crate::fxcache::world(),
        base: None,
        griffon: None,
    }
}
