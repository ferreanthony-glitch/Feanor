//! La gomme sur plusieurs calques (Anthony, 2026-11-10 : « Les outils sélection, gomme, déformer et transformer s'appliquent à
//! plusieurs calques quand plusieurs calques sont sélectionnés. »).
//!
//! Avec plusieurs calques choisis (Ctrl ou Maj + clic dans le panneau Calques, la timeline ou le montage), un passage de la gomme
//! efface sur chacun ce qu'elle touche **là où on le voit** : le calque courant comme toujours (`begin_paint`), et chaque autre
//! calque choisi reçoit les mêmes points, ramenés dans le repère de son dessin (sa transformation, ses clés de position et le
//! défilement multiplan à l'image courante, `multisel::layer_to_stage`).
//!
//! - **Chacun selon son genre** : un calque bitmap efface ses pixels avec la brosse de la gomme (agrandie ou réduite comme le
//!   calque), un calque vectoriel coupe ses traits (`vector::erase_circle`, comme la gomme vectorielle du calque courant, avec le
//!   réglage « traits entiers »).
//! - **Une seule étape d'annulation** pour tout le geste : la marque de l'historique est prise avant le trait du calque courant,
//!   et tout ce qui a été posé depuis est regroupé au lâcher (`History::regrouper_depuis`, juste même avec un historique plein).
//! - **Calques laissés de côté** : cachés, verrouillés (eux ou leur dossier, ou leur position), vidéo, sans dessin à cette
//!   image : la gomme les laisse tels quels et la barre d'état les nomme (`Targets::skipped_text`). Le calque courant refusé (par
//!   exemple verrouillé) ne bloque pas les autres.
//! - **Sans latence** : rien n'est rejoué ; un calque vectoriel refait seulement la zone des traits coupés, comme la gomme
//!   vectorielle d'un seul calque ; un calque bitmap pose les empreintes de la brosse, comme un trait.
//! - Ni dans une case des Vignettes (la planche a ses propres règles, `gommedebord.rs`), ni quand l'aplat est la cible
//!   (`aplatmulti.rs` s'en charge, sur les clés).

use eframe::egui::{Pos2, Rect};

use crate::app::{App, Tool};
use crate::brush::{StrokeMode, StrokeState};
use crate::document::CelId;
use crate::history::Edit;
use crate::vector::{self, Affine, VectorDrawing};

enum Sorte {
    /// Un dessin bitmap : la brosse de la gomme, comme un trait.
    Pixels(Box<StrokeState>),
    /// Un dessin vectoriel : son état d'avant le geste, le dernier point, la zone des traits coupés.
    Traits { avant: Box<VectorDrawing>, dernier: Option<Pos2>, region: Rect, change: bool },
}

/// Un autre calque choisi que la gomme traverse.
struct Autre {
    cel: CelId,
    /// Du dessin du calque courant vers le dessin de ce calque (à l'image courante).
    vers: Affine,
    /// L'échelle de ce passage : la gomme garde sa taille à l'écran.
    k: f32,
    /// Ce dessin est celui qu'on voit (sinon : celui d'une autre clé choisie, qu'on ne voit pas).
    live: bool,
    /// Un dessin qu'on ne voit pas reçoit ses points au lâcher (rien ne coûte pendant le geste) : point (dans son repère), appui,
    /// instant.
    attente: Vec<(Pos2, f32, f64)>,
    sorte: Sorte,
}

/// Les réglages de la gomme pour un point.
#[derive(Clone, Copy)]
struct Reglage {
    size: f32,
    pressure_size: bool,
    whole: bool,
}

/// La gomme passe en `q` (repère du dessin) sur ce dessin ; `voir` : l'image du dessin est refaite tout de suite (sinon au
/// lâcher). Rend les tuiles changées.
fn gommer(doc: &mut crate::document::Document, a: &mut Autre, q: Pos2, pressure: f32, time: f64, r: Reglage, voir: bool) -> Vec<usize> {
    let mut dirty = Vec::new();
    let Some(c) = doc.cels.get_mut(&a.cel) else { return dirty };
    match &mut a.sorte {
        Sorte::Pixels(stroke) => stroke.add_point_at(&mut c.raster, q, pressure, time, &mut dirty),
        Sorte::Traits { dernier, region, change, .. } => {
            let factor = if r.pressure_size { pressure.clamp(0.05, 1.0) } else { 1.0 };
            let radius = (r.size * 0.5 * factor * a.k).max(0.5);
            let Some(d) = c.vector.as_mut() else { return dirty };
            // Le long du segment parcouru (comme `App::erase_to`), et seule la zone des traits touchés est refaite.
            let from = dernier.unwrap_or(q);
            let steps = ((from.distance(q) / (radius * 0.5).max(0.5)).ceil() as usize).max(1);
            let swept = Rect::from_two_pos(from, q).expand(radius + 1.0);
            let zone = d.elements.iter().map(|e| e.bounds().expand(1.5)).filter(|b| b.intersects(swept)).fold(Rect::NOTHING, |r, b| r.union(b));
            let mut coupe = false;
            for k in 1..=steps {
                let p = from + (q - from) * (k as f32 / steps as f32);
                coupe |= vector::erase_circle(d, p, radius, r.whole);
            }
            *dernier = Some(q);
            if coupe {
                *change = true;
                *region = region.union(zone);
                if voir {
                    dirty = c.rerender_region(zone.expand(2.0));
                }
            }
        }
    }
    dirty
}

/// Le geste de gomme en cours sur les autres calques choisis.
#[derive(Default)]
pub(crate) struct GommeMulti {
    autres: Vec<Autre>,
    /// La marque de l'historique avant le geste (`History::marque`).
    marque: Option<u64>,
    /// Le geste est en cours (même si le calque courant n'a rien à gommer).
    actif: bool,
    pen: bool,
}

impl GommeMulti {
    /// Un geste de gomme est en cours sur plusieurs calques.
    pub(crate) fn actif(&self) -> bool {
        self.actif
    }

    pub(crate) fn pen(&self) -> bool {
        self.pen
    }

    /// La gomme passe en ce moment sur le dessin `cel` (son image n'est pas encore celle de sa géométrie : un rendu exact qui
    /// arriverait maintenant attend la fin du geste).
    pub(crate) fn busy_on(&self, cel: CelId) -> bool {
        self.actif && self.autres.iter().any(|a| a.cel == cel)
    }
}

impl App {
    /// La gomme agit-elle sur plusieurs calques ? L'outil Gomme, deux calques choisis au moins, hors des Vignettes et hors de
    /// l'aplat.
    pub(crate) fn gomme_multi_voulue(&self) -> bool {
        self.tool == Tool::Eraser && !self.in_card && !self.aplat_en_cible() && self.plusieurs_dessins_choisis()
    }

    /// Juste avant le trait du calque courant (`begin_paint`) : le geste d'avant est fini, et la marque de l'historique est prise
    /// (toutes les étapes du geste, sur tous les calques, n'en feront qu'une). Rien sans plusieurs calques choisis.
    pub(crate) fn gomme_multi_avant(&mut self) -> Option<u64> {
        if !self.gomme_multi_voulue() {
            return None;
        }
        self.end_stroke();
        Some(self.history.marque())
    }

    /// Juste après le trait du calque courant : les autres calques choisis reçoivent le même départ `p` (point du dessin du
    /// calque courant).
    pub(crate) fn gomme_multi_debut(&mut self, marque: u64, p: Pos2, pressure: f32, pen: bool) {
        self.gomme_multi = GommeMulti::default();
        if self.content_refuses() {
            return;
        }
        let Some(courant) = crate::multisel::layer_to_stage(&self.doc, self.cur_panel, self.cur_frame, self.cur_layer) else { return };
        // Le calque courant gomme déjà (son trait a commencé) : il n'est pas repris ici.
        let deja = (self.stroke.is_some() || self.verase.is_some()).then(|| self.cel_here()).flatten();
        let (targets, par_cles) = self.dessins_choisis();
        let Some(settings) = self.current_brush() else { return };
        let tol = self.close_tolerance();
        let mut autres = Vec::new();
        for t in &targets.list {
            if Some(t.cel) == deja || autres.iter().any(|a: &Autre| a.cel == t.cel) {
                continue;
            }
            let Some(inv) = t.to_stage.inverse() else { continue };
            let vers = courant.then(&inv);
            let k = (vers.a * vers.d - vers.b * vers.c).abs().sqrt().max(1e-3);
            let sorte = if t.vector {
                let Some(d) = self.doc.cels.get(&t.cel).and_then(|c| c.vector.as_ref()) else { continue };
                Sorte::Traits { avant: Box::new(d.clone()), dernier: None, region: Rect::NOTHING, change: false }
            } else {
                let layer = self.layers().iter().find(|l| l.id == t.id);
                let keep_alpha = layer.is_some_and(|l| l.keep_alpha);
                // (Comme `begin_stroke` : la toile infinie, sauf pour les griffonnages d'un calque de couleur.)
                let zone = layer.and_then(|l| l.couleur.as_ref()).is_some_and(|c| c.griffonner);
                let mut s = settings;
                s.size *= k;
                // (Le zoom du dessin de ce calque à l'écran : celui de la vue fois son échelle sur la feuille.)
                let echelle = (t.to_stage.a * t.to_stage.d - t.to_stage.b * t.to_stage.c).abs().sqrt();
                let mut stroke = StrokeState::new(t.cel, StrokeMode::Erase, s, None, pen)
                    .with_textures(self.brushes.textures(&s))
                    .with_feel(self.toucher)
                    .with_zoom(self.view.zoom * echelle)
                    .with_secondary(self.couleurs.prefs.back)
                    .on_layer(keep_alpha, None)
                    .with_options(false, false, tol);
                if !zone {
                    stroke = stroke.on_infinite_canvas();
                }
                Sorte::Pixels(Box::new(stroke))
            };
            autres.push(Autre { cel: t.cel, vers, k, live: t.live, attente: Vec::new(), sorte });
        }
        self.gomme_multi = GommeMulti { autres, marque: Some(marque), actif: true, pen };
        self.gomme_multi_point(p, pressure);
        // (Le dessin qu'on voit gomme toujours, choisi ou non.)
        let n = targets.list.len() + usize::from(deja.is_some_and(|c| !targets.list.iter().any(|t| t.cel == c)));
        let quoi = if par_cles { Self::dessins_texte(&targets, true) } else { format!("{n} calque{}", if n > 1 { "s" } else { "" }) };
        self.status = match targets.skipped_text() {
            Some(l) => format!("Gomme sur {quoi}. {l}"),
            None => format!("Gomme sur {quoi}."),
        };
    }

    /// La gomme passe en `p` (point du dessin du calque courant) : chaque autre calque choisi efface là, dans son repère.
    pub(crate) fn gomme_multi_point(&mut self, p: Pos2, pressure: f32) {
        if !self.gomme_multi.actif || self.gomme_multi.autres.is_empty() {
            return;
        }
        let time = self.input_time;
        let r = Reglage { size: self.eraser.size, pressure_size: self.eraser.pressure_size, whole: self.vector_erase_whole };
        let mut autres = std::mem::take(&mut self.gomme_multi.autres);
        for a in &mut autres {
            let q = a.vers.apply(p);
            if a.live {
                let tiles = gommer(&mut self.doc, a, q, pressure, time, r, true);
                self.renderer.invalidate_tiles(&tiles);
            } else {
                a.attente.push((q, pressure, time));
            }
        }
        self.gomme_multi.autres = autres;
    }

    /// Le geste est fini (`end_stroke`, après la gomme du calque courant) : chaque autre calque pose son étape, et tout ce qui a
    /// été posé depuis la marque devient une seule étape d'annulation.
    pub(crate) fn gomme_multi_fin(&mut self) {
        if !self.gomme_multi.actif {
            return;
        }
        let GommeMulti { autres, marque, .. } = std::mem::take(&mut self.gomme_multi);
        let mut edits: Vec<Edit> = Vec::new();
        let mut dirty = Vec::new();
        let r = Reglage { size: self.eraser.size, pressure_size: self.eraser.pressure_size, whole: self.vector_erase_whole };
        for mut a in autres {
            // Les dessins des autres clés reçoivent maintenant le geste entier.
            for (q, pressure, time) in std::mem::take(&mut a.attente) {
                gommer(&mut self.doc, &mut a, q, pressure, time, r, false);
            }
            match a.sorte {
                Sorte::Pixels(mut stroke) => {
                    let Some(c) = self.doc.cels.get_mut(&a.cel) else { continue };
                    let edit = stroke.finish(&mut c.raster, &mut dirty);
                    c.version += 1;
                    edits.extend(edit);
                }
                Sorte::Traits { avant, region, change, .. } => {
                    if !change {
                        continue;
                    }
                    if let Some(c) = self.doc.cels.get_mut(&a.cel) {
                        c.version += 1;
                    }
                    self.refresh_cel_region(a.cel, region);
                    edits.push(Edit::Vector { cel: a.cel, drawing: *avant });
                }
            }
        }
        self.renderer.invalidate_tiles(&dirty);
        if !edits.is_empty() {
            let avant = self.history.context.replace("Gomme".to_string());
            self.history.push(Edit::Group(edits));
            self.history.context = avant;
            self.modified = true;
        }
        if let Some(m) = marque {
            self.history.regrouper_depuis(m);
        }
    }
}
