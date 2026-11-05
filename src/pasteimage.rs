//! Coller une image du presse-papiers de Windows (Anthony, 2026-10-17 : « ajoute la possibilité
//! de coller une image copiée dans le presse-papiers de Windows, issue d'internet ou autre »).
//!
//! Ctrl+V (ou Édition > Coller) : si le presse-papiers de Windows tient une image qui ne vient pas
//! de FEANOR (ses copies y écrivent « FEANOR : … »), c'est elle qui est collée.
//!
//! - Sur un calque **vectoriel** (2026-10-29) : l'image devient un **objet vectoriel** du dessin de la clé (`Element::Image`,
//!   `imagevec.rs`), centré dans le cadre, réduit s'il le faut pour y tenir, et sélectionné (outil Sélection : on le déplace,
//!   on le transforme, on le supprime, on le réordonne comme les autres objets). Aucun calque n'est créé. Une grande image se
//!   prépare sur un fil de fond (`TacheImage`, reprise par `tick_collage`).
//! - Sur un calque **bitmap** modifiable : dans son dessin, en zone flottante (poignées, flèches ; Échap ou un autre outil la pose).
//! - Sinon (dossier, calque vidéo, calque bitmap verrouillé…) : sur un nouveau calque bitmap « Image collée », au-dessus du
//!   calque courant, comme avant.
//!
//! egui garde Ctrl+V pour lui quand le presse-papiers n'a pas de texte (une image copiée d'un
//! navigateur) et n'envoie rien ; le relâché de V, lui, arrive (voir `handle_shortcuts`).

use std::sync::mpsc::{channel, Receiver};
use std::time::Instant;

use crate::app::App;
use crate::collage::Ancre;
use crate::document::LayerKind;
use crate::pixsel::Floating;
use crate::raster::premultiply;

/// Début du texte qu'écrivent les copies de FEANOR dans le presse-papiers de Windows.
pub const MARK: &str = "FEANOR : ";

/// Le même début sous les anciens noms du logiciel : un presse-papiers rempli par une ancienne
/// version est encore reconnu.
pub const ANCIENNES_MARKS: [&str; 2] = ["SHOT : ", "Pelure : "];

/// Ce texte du presse-papiers est-il une copie interne de FEANOR (ou d'une ancienne version) ?
pub fn est_copie_interne(texte: &str) -> bool {
    texte.starts_with(MARK) || ANCIENNES_MARKS.iter().any(|m| texte.starts_with(m))
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

/// Une grande image qu'un fil de fond prépare (réduction, empreinte) avant de la poser en objet vectoriel : où elle ira.
pub(crate) struct TacheImage {
    rx: Receiver<Option<(u64, usize, usize)>>,
    ancre: Ancre,
    debut: Instant,
}

impl App {
    /// Colle l'image (largeur, hauteur, RVBA) venue du presse-papiers de Windows.
    pub(crate) fn paste_clipboard_image(&mut self, (w, h, rgba): (usize, usize, Vec<u8>)) {
        self.end_stroke();
        self.drop_bitmap_selection();
        // Un calque vectoriel : l'image devient un objet de son dessin, sans calque neuf.
        if self.layer().kind == LayerKind::Vector && !self.layer().folder {
            self.paste_image_as_object((w, h, rgba));
            return;
        }
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

    /// L'image (largeur, hauteur, RVBA) devient un objet vectoriel du dessin de l'image courante du calque vectoriel courant.
    /// Une grande image est préparée sur un fil de fond : l'interface continue de vivre, l'objet se pose à la fin (`tick_image`).
    pub(crate) fn paste_image_as_object(&mut self, (w, h, rgba): (usize, usize, Vec<u8>)) {
        if self.collage.images.is_some() {
            self.status = "Une image est déjà en cours de préparation.".into();
            return;
        }
        if !self.layer_editable() {
            return;
        }
        if w * h <= crate::imagevec::GRANDE {
            match crate::imagevec::preparer(w, h, rgba) {
                Some((key, tw, th)) => self.place_image_object(key, tw, th, (w, h)),
                None => self.status = "Image vide ou illisible : rien n'est collé.".into(),
            }
            return;
        }
        let (tx, rx) = channel();
        let ok = std::thread::Builder::new().name("image-collee".into()).spawn(move || {
            let _ = tx.send(crate::imagevec::preparer(w, h, rgba));
        });
        if ok.is_err() {
            self.status = "Collage impossible : la préparation de l'image n'a pas pu démarrer.".into();
            return;
        }
        let ancre = Ancre { panel: self.panel().id, layer: self.layer().id, frame: self.cur_frame };
        self.collage.images = Some(TacheImage { rx, ancre, debut: Instant::now() });
        self.status = format!("Image de {w}×{h} en préparation…");
    }

    /// Pose l'image d'empreinte `key` (déjà dans le magasin, `tw` × `th` pixels ; `origine` : sa taille avant réduction, pour le
    /// message) comme un objet du dessin courant : centrée dans le cadre, réduite pour y tenir, sélectionnée. Une étape d'annulation.
    pub(crate) fn place_image_object(&mut self, key: u64, tw: usize, th: usize, origine: (usize, usize)) {
        if !self.layer_editable() {
            return;
        }
        let frame = self.doc.frame_rect();
        let (fw, fh) = fit_size(tw, th, frame.width() as usize, frame.height() as usize);
        let s = fw as f32 / tw.max(1) as f32;
        // Au centre du cadre, dans le dessin du calque (qui peut être transformé).
        let c = self.to_layer(frame.center());
        let place = crate::vector::Affine { a: s, b: 0.0, c: 0.0, d: s, tx: c.x - tw as f32 * s * 0.5, ty: c.y - th as f32 * s * 0.5 };
        let element = crate::vector::Element::image(key, [tw as u32, th as u32], place);
        let (cel, prefix) = self.ensure_cel();
        let Some(d) = self.doc.cels.get_mut(&cel).and_then(|c| c.vector.as_mut()) else { return };
        let before = d.clone();
        let at = d.elements.len();
        d.elements.push(element);
        self.refresh_cel(cel);
        self.push_vector(cel, before, prefix);
        self.vsel = Some(crate::vtools::VSelection { cel, items: vec![at] });
        self.tool = crate::app::Tool::Select;
        let reduite = if (fw, fh) == (tw, th) { String::new() } else { format!(", réduite à {fw}×{fh} pour tenir dans le cadre") };
        self.status = format!("Image collée en objet vectoriel ({}×{}{reduite}) : déplace-la, transforme-la ou supprime-la comme les autres objets.", origine.0, origine.1);
    }

    /// À chaque image d'écran : la grande image est-elle prête ? Elle se pose là où le collage a été demandé.
    pub(crate) fn tick_image(&mut self) {
        let Some(t) = &self.collage.images else { return };
        match t.rx.try_recv() {
            Ok(resultat) => {
                let Some(t) = self.collage.images.take() else { return };
                let Some((key, tw, th)) = resultat else {
                    self.status = "Image vide ou illisible : rien n'est collé.".into();
                    return;
                };
                if let Some(pi) = self.doc.panels.iter().position(|p| p.id == t.ancre.panel) {
                    self.cur_panel = pi;
                    if let Some(li) = self.layers().iter().position(|l| l.id == t.ancre.layer) {
                        self.cur_layer = li;
                    }
                    self.cur_frame = t.ancre.frame.min(self.frame_count().saturating_sub(1));
                }
                let _ = t.debut;
                self.place_image_object(key, tw, th, (tw, th));
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => self.ctx.request_repaint_after(std::time::Duration::from_millis(30)),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.collage.images = None;
                self.status = "Le collage de l'image a échoué : la préparation s'est arrêtée.".into();
            }
        }
    }
}
