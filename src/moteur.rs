//! Moteur de dessin de Pelure pour d'autres interfaces (11a) : la bibliothèque de pinceaux (ceux
//! de Pelure et ceux importés, gardés dans %APPDATA%\Pelure\pinceaux), des dessins en tuiles, et
//! des traits peints par le même moteur que Pelure (`brush::StrokeState` : lissage, pression,
//! pointes et papiers en image). Données simples, sans egui ; utilisé par Pelure Studio.

use eframe::egui::{pos2, Color32};
use serde::Serialize;

use crate::brush::{stroke_preview, BrushSettings, StrokeMode, StrokeState};
use crate::brushtex::{BrushLibrary, Target};
use crate::history::Edit;
use crate::raster::{Raster, Tile};

/// Côté d'une tuile, en pixels.
pub const TUILE: usize = crate::raster::TILE;

/// Un réglage de pinceau nommé, tel qu'une interface le montre.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Reglage {
    /// Rang dans la bibliothèque (à redonner pour peindre avec).
    pub numero: usize,
    pub nom: String,
    /// Réglage de gomme (sinon de pinceau).
    pub gomme: bool,
    /// « Pelure », ou le fichier d'où il a été importé.
    pub source: String,
    /// Diamètre (pixels) et opacité (0–1) de départ.
    pub taille: f32,
    pub opacite: f32,
    pub pression_taille: bool,
    /// Pointe ou papier en image.
    pub texture: bool,
}

/// La bibliothèque de pinceaux.
pub struct Pinceaux {
    lib: BrushLibrary,
}

impl Pinceaux {
    /// Ceux de Pelure et ceux importés dans Pelure (même dossier).
    pub fn charger() -> Pinceaux {
        Pinceaux { lib: BrushLibrary::load(BrushLibrary::user_dir().as_deref()) }
    }

    /// Ceux de Pelure seulement (sans rien lire sur le disque).
    pub fn de_pelure() -> Pinceaux {
        Pinceaux { lib: BrushLibrary::builtin() }
    }

    pub fn reglages(&self) -> Vec<Reglage> {
        self.lib
            .presets
            .iter()
            .enumerate()
            .map(|(numero, p)| Reglage {
                numero,
                nom: p.name.clone(),
                gomme: p.target == Target::Eraser,
                source: p.source.clone(),
                taille: p.settings.size,
                opacite: p.settings.opacity,
                pression_taille: p.settings.pressure_size,
                texture: p.settings.tip.is_some() || (p.settings.paper.is_some() && p.settings.grain > 0.0),
            })
            .collect()
    }

    /// Aperçu du réglage `numero` : un trait en vague tracé par le moteur, `largeur`×`hauteur`
    /// pixels, couverture (0–255) ligne par ligne.
    pub fn apercu(&self, numero: usize, largeur: usize, hauteur: usize) -> Option<Vec<u8>> {
        let p = self.lib.presets.get(numero)?;
        Some(stroke_preview(&p.settings, self.lib.textures(&p.settings), largeur.max(8), hauteur.max(8)))
    }

    /// Le même aperçu en PNG : le trait dans la couleur `couleur`, sur fond transparent.
    pub fn apercu_png(&self, numero: usize, largeur: usize, hauteur: usize, couleur: [u8; 3]) -> Option<Vec<u8>> {
        let (l, h) = (largeur.max(8), hauteur.max(8));
        let couverture = self.apercu(numero, l, h)?;
        let rgba: Vec<u8> = couverture.iter().flat_map(|&a| [couleur[0], couleur[1], couleur[2], a]).collect();
        crate::io::encode_png(&rgba, l, h).ok()
    }
}

/// Un dessin : une image RGBA prémultipliée en tuiles de 64×64, allouées où l'on dessine.
pub struct Dessin {
    raster: Raster,
}

impl Dessin {
    pub fn new(largeur: usize, hauteur: usize) -> Dessin {
        Dessin { raster: Raster::new(largeur.max(1), hauteur.max(1)) }
    }

    pub fn largeur(&self) -> usize {
        self.raster.width
    }

    pub fn hauteur(&self) -> usize {
        self.raster.height
    }

    /// Origine et taille utile de la tuile `idx` (x, y, l, h).
    pub fn bornes(&self, idx: usize) -> (usize, usize, usize, usize) {
        self.raster.tile_bounds(idx)
    }

    /// Pixels de la tuile `idx` (sa partie dans l'image), RGBA prémultipliés, ligne par ligne.
    pub fn tuile_rgba(&self, idx: usize) -> Vec<u8> {
        let (_, _, w, h) = self.bornes(idx);
        let mut out = Vec::with_capacity(w * h * 4);
        match self.raster.tiles.get(idx).and_then(|t| t.as_ref()) {
            Some(t) => {
                for y in 0..h {
                    for c in &t[y * TUILE..y * TUILE + w] {
                        out.extend_from_slice(&c.to_array());
                    }
                }
            }
            None => out.resize(w * h * 4, 0),
        }
        out
    }

    /// Pixel (RGBA prémultipliés).
    pub fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        if x >= self.raster.width || y >= self.raster.height {
            return [0; 4];
        }
        self.raster.get(x, y).to_array()
    }
}

/// Avec quoi peindre un trait : un réglage de la bibliothèque, éventuellement retouché.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outil {
    pub reglage: usize,
    pub couleur: [u8; 3],
    /// Remplacent ceux du réglage s'ils sont donnés.
    pub taille: Option<f32>,
    pub opacite: Option<f32>,
    pub pression_taille: Option<bool>,
    /// Gomme : retire au lieu de peindre (quel que soit le réglage).
    pub gomme: bool,
}

/// Bout du trait : du dernier point posé à la pointe du stylet, rayon, couleur et opacité ;
/// de quoi relier à l'écran le trait à la pointe (le lissage le fait traîner un peu derrière).
#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
pub struct Pointe {
    pub depuis: [f32; 2],
    pub jusqua: [f32; 2],
    pub rayon: f32,
    pub couleur: [u8; 3],
    pub opacite: f32,
}

/// Un trait en cours.
pub struct Trait {
    etat: StrokeState,
}

/// De quoi défaire un trait (ou refaire un trait défait) : les tuiles d'avant.
pub struct Annulation {
    tuiles: Vec<(usize, Option<Tile>)>,
}

impl Trait {
    /// Commence un trait avec `outil` ; `stylet` : la pression vient d'une tablette.
    pub fn commencer(pinceaux: &Pinceaux, outil: &Outil, stylet: bool) -> Trait {
        let preset = pinceaux.lib.presets.get(outil.reglage).or(pinceaux.lib.presets.first());
        let base = preset.map_or_else(BrushSettings::brush, |p| p.settings);
        let reglages = BrushSettings {
            size: outil.taille.unwrap_or(base.size).clamp(0.5, 2000.0),
            opacity: outil.opacite.unwrap_or(base.opacity).clamp(0.0, 1.0),
            pressure_size: outil.pression_taille.unwrap_or(base.pressure_size),
            ..base
        };
        let mode = if outil.gomme { StrokeMode::Erase } else { StrokeMode::Paint(outil.couleur) };
        let etat = StrokeState::new(0, mode, reglages, None, stylet).with_textures(pinceaux.lib.textures(&reglages));
        Trait { etat }
    }

    /// Ajoute un point du stylet ; les tuiles changées sont ajoutées à `changees`.
    pub fn point(&mut self, dessin: &mut Dessin, x: f32, y: f32, pression: f32, changees: &mut Vec<usize>) {
        self.etat.add_point(&mut dessin.raster, pos2(x, y), pression.clamp(0.0, 1.0), changees);
    }

    /// Comme `point`, avec l'instant (secondes) où le stylet y était : la vitesse des connexions
    /// et le lissage selon le temps en viennent (sans instant : par échantillon, comme avant).
    pub fn point_instant(&mut self, dessin: &mut Dessin, x: f32, y: f32, pression: f32, instant: f64, changees: &mut Vec<usize>) {
        self.etat.add_point_at(&mut dessin.raster, pos2(x, y), pression.clamp(0.0, 1.0), instant, changees);
    }

    pub fn pointe(&self) -> Option<Pointe> {
        let (depuis, jusqua, rayon) = self.etat.tip()?;
        let (couleur, opacite) = self.etat.tip_paint().unwrap_or(([0, 0, 0], 0.0));
        Some(Pointe { depuis: [depuis.x, depuis.y], jusqua: [jusqua.x, jusqua.y], rayon, couleur, opacite })
    }

    /// Termine le trait (le lissage rattrape la pointe) ; rend de quoi l'annuler.
    pub fn finir(mut self, dessin: &mut Dessin, changees: &mut Vec<usize>) -> Annulation {
        let tuiles = match self.etat.finish(&mut dessin.raster, changees) {
            Some(Edit::Tiles { tiles, .. }) => tiles,
            _ => Vec::new(),
        };
        Annulation { tuiles }
    }
}

impl Annulation {
    /// Rien à défaire (trait sans effet).
    pub fn est_vide(&self) -> bool {
        self.tuiles.is_empty()
    }

    /// Remet les tuiles d'avant dans `dessin` ; rend de quoi refaire, et les tuiles changées.
    pub fn appliquer(self, dessin: &mut Dessin) -> (Annulation, Vec<usize>) {
        let mut changees = Vec::with_capacity(self.tuiles.len());
        let mut tuiles = Vec::with_capacity(self.tuiles.len());
        let n = dessin.raster.tiles.len();
        for (idx, avant) in self.tuiles.into_iter().filter(|(idx, _)| *idx < n) {
            changees.push(idx);
            tuiles.push((idx, std::mem::replace(&mut dessin.raster.tiles[idx], avant)));
        }
        (Annulation { tuiles }, changees)
    }
}

/// Couleur prémultipliée (pour les tests et les interfaces qui en ont besoin).
pub fn premultiplie(r: u8, g: u8, b: u8, a: u8) -> [u8; 4] {
    Color32::from_rgba_unmultiplied(r, g, b, a).to_array()
}
