//! Coller une image du presse-papiers de Windows (Anthony, 2026-10-17 : « ajoute la possibilité
//! de coller une image copiée dans le presse-papiers de Windows, issue d'internet ou autre »).
//!
//! Ctrl+V (ou Édition > Coller) : si le presse-papiers de Windows tient une image qui ne vient pas
//! de SHOT (ses copies y écrivent « SHOT : … »), c'est elle qui est collée. Sur un calque
//! bitmap modifiable : dans son dessin ; sinon sur un nouveau calque bitmap « Image collée »,
//! au-dessus du calque courant. Centrée dans le cadre, réduite s'il le faut pour y tenir, et
//! sélectionnée (poignées, flèches) : Échap ou un autre outil la pose.
//!
//! egui garde Ctrl+V pour lui quand le presse-papiers n'a pas de texte (une image copiée d'un
//! navigateur) et n'envoie rien ; le relâché de V, lui, arrive (voir `handle_shortcuts`).

use crate::app::App;
use crate::document::LayerKind;
use crate::pixsel::Floating;
use crate::raster::premultiply;

/// Début du texte qu'écrivent les copies de SHOT dans le presse-papiers de Windows.
pub const MARK: &str = "SHOT : ";

/// Le même début chez l'ancien nom du logiciel (Pelure) : un presse-papiers rempli par l'ancienne
/// version est encore reconnu.
pub const ANCIEN_MARK: &str = "Pelure : ";

/// Ce texte du presse-papiers est-il une copie interne de SHOT (ou de Pelure) ?
pub fn est_copie_interne(texte: &str) -> bool {
    texte.starts_with(MARK) || texte.starts_with(ANCIEN_MARK)
}

/// Taille qui tient dans `max_w` × `max_h` sans déformer ; jamais agrandie.
pub fn fit_size(w: usize, h: usize, max_w: usize, max_h: usize) -> (usize, usize) {
    if (w <= max_w && h <= max_h) || w == 0 || h == 0 {
        return (w, h);
    }
    let k = (max_w as f64 / w as f64).min(max_h as f64 / h as f64);
    (((w as f64 * k).round() as usize).max(1), ((h as f64 * k).round() as usize).max(1))
}

/// Pixels RVBA (non prémultipliés) en pixels soulevés, coin haut-gauche en (`x0`, `y0`). Une
/// image dont aucun pixel n'a d'opacité (certains logiciels n'en écrivent pas) est opaque.
pub fn to_floating(rgba: &[u8], w: usize, h: usize, x0: usize, y0: usize) -> Floating {
    let px = &rgba[..(w * h * 4).min(rgba.len())];
    let no_alpha = px.chunks_exact(4).all(|p| p[3] == 0);
    let pixels = px
        .chunks_exact(4)
        .map(|p| premultiply(p[0], p[1], p[2], if no_alpha { 255 } else { p[3] }))
        .collect();
    Floating { x0, y0, w, h, pixels }
}

/// L'image du presse-papiers de Windows (largeur, hauteur, RVBA), si elle ne vient pas de SHOT.
pub fn clipboard_image() -> Option<(usize, usize, Vec<u8>)> {
    // Les essais ne lisent jamais le presse-papiers de la machine : une image qui s'y trouverait
    // se collerait dans l'essai (Ctrl+V lancé par `run_action`).
    if cfg!(test) {
        return None;
    }
    let mut clipboard = arboard::Clipboard::new().ok()?;
    if clipboard.get_text().is_ok_and(|t| est_copie_interne(&t)) {
        return None;
    }
    let image = clipboard.get_image().ok()?;
    (image.width > 0 && image.height > 0).then(|| (image.width, image.height, image.bytes.into_owned()))
}

impl App {
    /// Colle l'image (largeur, hauteur, RVBA) venue du presse-papiers de Windows.
    pub(crate) fn paste_clipboard_image(&mut self, (w, h, rgba): (usize, usize, Vec<u8>)) {
        self.end_stroke();
        self.drop_bitmap_selection();
        let l = self.layer();
        let here = l.kind == LayerKind::Bitmap && l.drawable() && !l.locked && !l.position_locked;
        if !here {
            self.add_layer(LayerKind::Bitmap);
            let li = self.cur_layer;
            if let Some(l) = self.layers_mut().get_mut(li) {
                l.name = "Image collée".into();
            }
        }
        if !self.layer_editable() {
            return;
        }
        let frame = self.doc.frame_rect();
        let (tw, th) = fit_size(w, h, frame.width() as usize, frame.height() as usize);
        let rgba = if (tw, th) == (w, h) {
            rgba
        } else {
            let Some(img) = image::RgbaImage::from_raw(w as u32, h as u32, rgba) else { return };
            image::imageops::resize(&img, tw as u32, th as u32, image::imageops::FilterType::Triangle).into_raw()
        };
        // Au centre du cadre, dans le dessin du calque (qui peut être transformé).
        let c = self.to_layer(frame.center());
        let (sw, sh) = (self.doc.width, self.doc.height);
        let x0 = (c.x - tw as f32 * 0.5).round().clamp(0.0, sw.saturating_sub(tw) as f32) as usize;
        let y0 = (c.y - th as f32 * 0.5).round().clamp(0.0, sh.saturating_sub(th) as f32) as usize;
        if self.paste_floating(to_floating(&rgba, tw, th, x0, y0)) {
            let reduced = if (tw, th) == (w, h) { String::new() } else { format!(", réduite à {tw}×{th} pour tenir dans le cadre") };
            self.status = format!("Image collée ({w}×{h}{reduced}) : déplace-la (flèches, poignées), puis Échap ou un autre outil pour la poser.");
        }
    }
}
