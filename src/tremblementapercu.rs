//! L'aperçu du tremblement du trait par défaut (Préférences > Dessin > Tremblement des traits) : un petit trait vectoriel
//! d'exemple, montré avec les réglages du moment, qui bouge quand « Animer à l'arrêt » est allumé.
//!
//! Il coûte presque rien (règle « sans latence ») :
//! - le trait d'exemple est dessiné **une seule fois** par le moteur de brosses (240 × 84 pixels, blanc : l'aperçu le teinte
//!   de la couleur du texte du thème) et gardé pour toute la séance ;
//! - à chaque **changement de réglage** — pas à chaque image —, une petite image par variante est calculée par le même calcul
//!   que l'effet (`tremblement::boil`, quelques dizaines de microsecondes à la taille de l'aperçu) et envoyée à la carte
//!   graphique ; le moteur de brosses ne rejoue rien ;
//! - ensuite, l'aperçu ne fait que changer d'image à la cadence du réglage (`vitesse` images du projet par variante), en
//!   demandant une repeinture à cette cadence seulement, et seulement quand « Animer à l'arrêt » est allumé et que l'aperçu est
//!   à l'écran.
//! L'état (réglage vu, images, nombre de calculs) vit dans la mémoire d'egui : aucun champ d'application de plus.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use eframe::egui::{self, pos2, vec2, Color32, ColorImage, Rect, RichText, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions};

use crate::document::{Cel, LayerKind};
use crate::raster::Raster;
use crate::tremblement::{self, Defaut};
use crate::vector::{Element, VPoint};

pub(crate) const LARGEUR: usize = 240;
pub(crate) const HAUTEUR: usize = 84;

/// Le trait d'exemple, rendu une fois.
static BASE: OnceLock<Arc<Raster>> = OnceLock::new();
/// Combien de fois le moteur de brosses a dessiné le trait d'exemple (au plus une fois par séance).
pub(crate) static RENDUS_DE_BASE: AtomicUsize = AtomicUsize::new(0);

fn base() -> Arc<Raster> {
    BASE.get_or_init(|| {
        RENDUS_DE_BASE.fetch_add(1, Ordering::Relaxed);
        let mut cel = Cel::new(LARGEUR, HAUTEUR, LayerKind::Vector);
        // Une vague et une boucle : de quoi voir le déplacement le long du trait et à son bord.
        let points: Vec<VPoint> = (0..=56)
            .map(|k| {
                let t = k as f32 / 56.0;
                VPoint { x: 14.0 + t * (LARGEUR as f32 - 28.0), y: HAUTEUR as f32 / 2.0 + 20.0 * (t * std::f32::consts::TAU * 1.5).sin() * (1.0 - 0.3 * t), r: 2.6 }
            })
            .collect();
        if let Some(v) = cel.vector.as_mut() {
            v.elements = vec![Element::Stroke { points, color: [255, 255, 255], opacity: 1.0, hardness: 1.0, brush: None, trace: Default::default() }];
        }
        cel.rerender();
        Arc::new(cel.raster)
    })
    .clone()
}

/// Ce que l'écran garde de l'aperçu entre deux images.
#[derive(Clone)]
struct Etat {
    /// Le réglage pour lequel les images sont faites (allumé d'office : l'aperçu montre les réglages).
    reglage: Defaut,
    /// Une image par variante.
    images: Vec<TextureHandle>,
    /// Combien d'images ont été calculées depuis le début (les essais comptent : aucun calcul à chaque image).
    calculs: usize,
}

fn cle() -> egui::Id {
    egui::Id::new("tremblement_defaut_apercu")
}

/// Combien d'images de variante ont été calculées pour l'aperçu de ce contexte.
#[cfg(test)]
pub(crate) fn calculs(ctx: &egui::Context) -> usize {
    ctx.data(|d| d.get_temp::<Etat>(cle())).map_or(0, |e| e.calculs)
}

fn image_de(r: &Raster) -> ColorImage {
    let mut image = ColorImage::new([LARGEUR, HAUTEUR], Color32::TRANSPARENT);
    for y in 0..HAUTEUR {
        for x in 0..LARGEUR {
            image.pixels[y * LARGEUR + x] = r.get(x, y);
        }
    }
    image
}

/// Les images de l'aperçu pour ce réglage (une par variante).
fn calculer(ctx: &egui::Context, d: &Defaut) -> Vec<TextureHandle> {
    let base = base();
    (0..d.variantes)
        .map(|v| {
            let mut values = d.values();
            values.push(v as f32);
            let r = tremblement::boil(&base, &values, 0);
            ctx.load_texture(format!("tremblement-apercu-{v}"), image_de(&r), TextureOptions::LINEAR)
        })
        .collect()
}

/// L'image de l'aperçu à montrer au temps `temps` (secondes) : la première tant qu'on n'anime pas à l'arrêt ; sinon la variante
/// de l'image du projet (à `fps` images par seconde) que l'horloge a atteinte, comme le trait d'un calque.
pub(crate) fn variante_montree(d: &Defaut, fps: u32, temps: f64) -> usize {
    if !d.repos {
        return 0;
    }
    tremblement::variant_at(tremblement::rest_frame((temps * 1000.0) as u64, fps), d.vitesse, d.variantes)
}

/// Peint l'aperçu : le trait d'exemple avec le tremblement par défaut `reglage`, `fps` images par seconde (celles du projet).
pub(crate) fn apercu_ui(ui: &mut egui::Ui, reglage: &Defaut, fps: u32) {
    let ctx = ui.ctx().clone();
    let d = Defaut { actif: true, ..reglage.sanitized() };
    let courant = ctx.data(|c| c.get_temp::<Etat>(cle())).filter(|e| e.reglage == d);
    let etat = match courant {
        Some(e) => e,
        None => {
            // Un réglage a changé (ou c'est la première fois) : les petites images se refont, une fois.
            let avant = ctx.data(|c| c.get_temp::<Etat>(cle())).map_or(0, |e| e.calculs);
            let images = calculer(&ctx, &d);
            let e = Etat { reglage: d, calculs: avant + images.len(), images };
            ctx.data_mut(|c| c.insert_temp(cle(), e.clone()));
            e
        }
    };
    let (rect, reponse) = ui.allocate_exact_size(vec2(LARGEUR as f32, HAUTEUR as f32), Sense::hover());
    crate::bulles::bulle(
        reponse,
        "Aperçu du tremblement",
        "Un trait d'exemple avec les réglages d'à côté. Il bouge à la cadence du réglage quand « Animer à l'arrêt » est allumé ; sinon, il reste immobile (comme une image fixe).",
        "",
    );
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, crate::theme::apercu_trait());
    painter.rect_stroke(rect, 4.0, Stroke::new(1.0_f32, crate::theme::bord()), StrokeKind::Inside);
    let temps = ui.input(|i| i.time);
    let v = variante_montree(&d, fps, temps);
    if let Some(image) = etat.images.get(v) {
        painter.image(image.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), crate::theme::texte_fort());
    }
    if d.repos {
        // (L'écran ne se repeint qu'à la cadence du tremblement, pas à 60 Hz ; et seulement tant que l'aperçu est montré.)
        let periode = d.vitesse as f64 / fps.max(1) as f64;
        ui.ctx().request_repaint_after(Duration::from_secs_f64(periode.max(0.02)));
        ui.label(RichText::new(format!("{} dessins, un nouveau toutes les {} images.", d.variantes, d.vitesse)).small().weak());
    } else {
        ui.label(RichText::new("Immobile : « Animer à l'arrêt » le fait bouger.").small().weak());
    }
}
