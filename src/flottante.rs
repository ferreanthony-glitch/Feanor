//! Feuille flottante d'une sélection vectorielle (Anthony, 2026-10-18 : « le ralentissement est
//! évident » en déplaçant une quarantaine de traits ; règle : sans latence).
//!
//! Avant, chaque image d'un glissé refaisait les traits choisis dans l'image du dessin
//! (`transformed_preview` puis `rerender_region` sur toute la boîte de la sélection) et
//! invalidait des dizaines de tuiles, renvoyées à la carte graphique à chaque image. Maintenant,
//! à la première image du geste, on fait UNE fois deux images : le dessin SANS les éléments
//! choisis (`BASE_CEL`) et les éléments choisis seuls (`SHEET_CEL`), l'une et l'autre posées depuis
//! les pixels en cache. Le plan montre la première telle quelle et la seconde avec la
//! transformation du geste (`PlanItem::pre`) : la carte graphique n'a qu'un paramètre à changer
//! par image, aucun pixel n'est refait ni envoyé.
//!
//! Au lâcher (`App::finish_vdrag`), la géométrie des éléments reçoit la transformation et le rejeu
//! exact part en arrière-plan (`rejeu.rs`) ; la feuille reste affichée jusqu'à son arrivée, donc
//! le lâcher ne coûte rien non plus. Si l'utilisateur agit avant (clic, touche), l'image du
//! dessin est d'abord mise à jour à l'ancienne (`App::flush_flottante`). Pendant le geste, les
//! éléments restent où ils étaient : seule la feuille bouge.
//!
//! Les deux dessins temporaires vivent dans `Document::cels` le temps du geste ; aucun calque ne
//! les référence, donc ils ne sont jamais enregistrés.
//!
//! La feuille des éléments choisis est élargie de ce qui dépasse de la feuille du dessin (les
//! pixels en cache des traits le gardent, `vector::overhang`) : un trait qui en sort, déplacé vers
//! l'intérieur, se montre en entier au lieu d'être coupé en ligne droite à l'ancien bord. Sa
//! marge et sa taille sont comptées dans la transformation du plan (`Pre`).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use eframe::egui::{vec2, Pos2, Rect};

use crate::document::{Cel, CelId, Document};
use crate::raster::Raster;
use crate::render::{PlanItem, Pre};
use crate::vector::{Affine, Element, VectorDrawing};
use crate::warp::Warp;

/// Le dessin sans les éléments choisis.
pub const BASE_CEL: CelId = 1 << 62;
/// Les éléments choisis seuls.
pub const SHEET_CEL: CelId = (1 << 62) + 1;

/// Numéro de version des feuilles : neuf à chaque saisie (la carte graphique les renvoie).
static VERSION: AtomicU64 = AtomicU64::new(1 << 40);

/// Les numéros des deux dessins temporaires de la place `slot` : la place 0 est celle d'une
/// sélection sur un seul calque (`BASE_CEL`, `SHEET_CEL`) ; une sélection sur plusieurs calques
/// (`multisel.rs`) a une place par calque, de la 1 à la suivante.
pub const fn slot_ids(slot: usize) -> (CelId, CelId) {
    (BASE_CEL + 2 * slot as u64, SHEET_CEL + 2 * slot as u64)
}

/// Un geste en cours sur la sélection du dessin `src`.
pub struct Flottante {
    pub src: CelId,
    /// Sa place : où vivent ses deux dessins temporaires (`slot_ids`).
    pub slot: usize,
    /// La transformation du geste, en coordonnées du dessin.
    pub aff: Affine,
    /// Geste lâché : les éléments ont leur nouvelle géométrie, mais l'image du dessin (`raster`)
    /// est encore celle d'avant. La feuille reste affichée jusqu'à l'arrivée du rendu exact
    /// (`App::poll_rejeu`), ou jusqu'à la prochaine action de l'utilisateur
    /// (`App::flush_flottante`).
    pub pending: Option<Pending>,
    /// La feuille des éléments choisis garde ce qui dépasse de la feuille du dessin (un trait qui
    /// en sort, déplacé vers l'intérieur, se montre en entier) : son image est plus grande que la
    /// feuille, et son pixel (x, y) est le point (x − marge.0, y − marge.1) du dessin. Aucune
    /// marge : la feuille a la taille du dessin.
    pub margin: (usize, usize),
    /// Taille de l'image de la feuille si ce n'est pas celle du dessin.
    pub dims: Option<[usize; 2]>,
    /// Enveloppe ou perspective : la feuille n'est pas posée par `aff` mais par ce maillage
    /// (`warp.rs`), mis à jour à chaque image du geste ; la carte graphique le dessine.
    pub warp: Option<Arc<Warp>>,
    /// Cette feuille est celle d'une séance d'enveloppe ou de perspective (`App::warp_float`),
    /// faite du dessin d'origine de la séance : elle sert à chaque geste de la séance.
    pub for_warp: bool,
    /// Feuille d'une zone bitmap soulevée (`pixtools.rs`) : le dessin sans la zone, et la zone seule à sa place
    /// d'origine ; `aff` est la transformation de la zone (`Lifted::aff`). Aucun élément vectoriel.
    pub bitmap: bool,
}

impl crate::app::App {
    /// Un geste d'enveloppe ou de perspective sur la région `src` du dessin `cel` : la feuille
    /// flottante des éléments `items` (tous si `None`) du dessin d'origine `drawing` est posée par
    /// un maillage (`map` : où va chaque point de la région). La feuille est faite une fois (à la
    /// première image du geste, depuis les pixels en cache des traits) et gardée tant que la séance
    /// dure : un geste lâché dont le rendu exact n'est pas arrivé la garde aussi, le geste suivant
    /// la reprend. Ne rejoue jamais le moteur sur le fil de l'interface.
    pub(crate) fn warp_float(&mut self, cel: CelId, drawing: &VectorDrawing, items: Option<&[usize]>, src: Rect, min_cells: usize, map: impl Fn(Pos2) -> Pos2) {
        if !self.flottante.as_ref().is_some_and(|f| f.src == cel && f.for_warp) {
            // La feuille d'un autre geste (déplacer, tourner) se termine d'abord.
            self.flush_flottante();
            if self.flottante.take().is_some() {
                Flottante::release(&mut self.doc);
            }
            let all: Vec<usize>;
            let list = match items {
                Some(list) => list,
                None => {
                    all = (0..drawing.elements.len()).collect();
                    &all
                }
            };
            self.flottante = Flottante::lift_from(&mut self.doc, cel, drawing, list);
        }
        if let Some(f) = self.flottante.as_mut() {
            f.pending = None;
            f.set_warp(src, min_cells, map);
        }
    }

    /// Le geste est lâché : la feuille déformée attend le rendu exact (`App::poll_rejeu`).
    pub(crate) fn warp_settle(&mut self, cel: CelId) {
        if let Some(f) = self.flottante.as_mut().filter(|f| f.src == cel && f.for_warp) {
            f.pending = Some(Pending { items: Vec::new(), originals: Vec::new() });
        }
    }
}

/// Ce qu'il faut pour terminer le travail d'un geste lâché sans attendre le rejeu.
pub struct Pending {
    /// Les éléments transformés, et leurs états d'avant le geste.
    pub items: Vec<usize>,
    pub originals: Vec<Element>,
}

impl Flottante {
    /// Saisie : sépare le dessin `src` en deux images (depuis les caches des traits) et les range
    /// dans `doc` sous `BASE_CEL` et `SHEET_CEL`.
    pub fn lift(doc: &mut Document, src: CelId, items: &[usize]) -> Option<Flottante> {
        Self::lift_slot(doc, src, items, 0)
    }

    /// Comme `lift`, dans la place `slot` (plusieurs feuilles à la fois : `multisel.rs`).
    pub fn lift_slot(doc: &mut Document, src: CelId, items: &[usize], slot: usize) -> Option<Flottante> {
        let cel = doc.cels.get(&src)?;
        let d = cel.vector.as_ref()?;
        let split = d.render_split_extended(items, cel.raster.width, cel.raster.height);
        Some(Self::store(doc, src, split, slot))
    }

    /// Comme `lift`, mais depuis un autre état du dessin `src` que celui du document : l'enveloppe
    /// et la perspective déforment toujours le dessin d'origine de leur séance (pas de perte d'un
    /// geste à l'autre), alors que le dessin du document a déjà reçu la géométrie du geste d'avant.
    pub fn lift_from(doc: &mut Document, src: CelId, drawing: &VectorDrawing, items: &[usize]) -> Option<Flottante> {
        Self::lift_from_slot(doc, src, drawing, items, 0)
    }

    /// Comme `lift_from`, dans la place `slot` (une déformation sur plusieurs calques : une place par calque, `deformmulti.rs`).
    pub fn lift_from_slot(doc: &mut Document, src: CelId, drawing: &VectorDrawing, items: &[usize], slot: usize) -> Option<Flottante> {
        let cel = doc.cels.get(&src)?;
        let split = drawing.render_split_extended(items, cel.raster.width, cel.raster.height);
        Some(Flottante { for_warp: true, ..Self::store(doc, src, split, slot) })
    }

    fn store(doc: &mut Document, src: CelId, (base, sheet, margin): (Raster, Raster, (usize, usize)), slot: usize) -> Flottante {
        let (w, h) = (base.width, base.height);
        let dims = ((sheet.width, sheet.height) != (w, h)).then_some([sheet.width, sheet.height]);
        let version = VERSION.fetch_add(2, Ordering::Relaxed);
        let (base_id, sheet_id) = slot_ids(slot);
        for (id, raster, version) in [(base_id, base, version), (sheet_id, sheet, version + 1)] {
            let mut c = Cel::bitmap(raster);
            c.version = version;
            doc.cels.insert(id, c);
        }
        Flottante { src, slot, aff: Affine::IDENTITY, pending: None, margin, dims, warp: None, for_warp: false, bitmap: false }
    }

    /// Saisie d'une zone bitmap : `base`, le dessin sans la zone, et `sheet`, la zone seule à sa place d'origine (images de la
    /// taille du dessin, rien n'est copié : les tuiles se partagent). La carte graphique pose la feuille avec `aff` à chaque
    /// image du geste ; aucun pixel n'est refait ni envoyé (`pixtools.rs`).
    pub fn lift_bitmap(doc: &mut Document, src: CelId, base: &Raster, sheet: Raster) -> Flottante {
        let (w, h) = (base.width, base.height);
        let dims = ((sheet.width, sheet.height) != (w, h)).then_some([sheet.width, sheet.height]);
        let version = VERSION.fetch_add(2, Ordering::Relaxed);
        let (base_id, sheet_id) = slot_ids(0);
        for (id, raster, version) in [(base_id, base.clone(), version), (sheet_id, sheet, version + 1)] {
            let mut c = Cel::bitmap(raster);
            c.version = version;
            doc.cels.insert(id, c);
        }
        Flottante { src, slot: 0, aff: Affine::IDENTITY, pending: None, margin: (0, 0), dims, warp: None, for_warp: false, bitmap: true }
    }

    /// Le maillage du geste en cours : la région `src` du dessin (le cadre de l'enveloppe), chaque
    /// point allant où `map` le met. Faux si le maillage est impossible (région vide).
    pub fn set_warp(&mut self, src: Rect, min_cells: usize, map: impl Fn(Pos2) -> Pos2) -> bool {
        match Warp::new(src, self.margin, min_cells, map) {
            Some(w) => {
                self.warp = Some(Arc::new(w));
                true
            }
            None => false,
        }
    }

    /// Terminer tout de suite un geste d'enveloppe ou de perspective lâché, sans attendre le rendu
    /// exact : l'image du dessin devient le reste, avec la feuille posée par le maillage (calcul
    /// sur processeur, quelques dizaines de ms ; arrive seulement si l'utilisateur agit dans la
    /// seconde qui suit le lâcher). À appeler avant `release`. Rend les tuiles changées.
    pub fn flatten_warp(doc: &mut Document, src: CelId, warp: &Warp) -> Vec<usize> {
        Self::flatten_warp_slot(doc, src, 0, warp)
    }

    /// Comme `flatten_warp`, pour la feuille de la place `slot`.
    pub fn flatten_warp_slot(doc: &mut Document, src: CelId, slot: usize, warp: &Warp) -> Vec<usize> {
        let (base_id, sheet_id) = slot_ids(slot);
        let (Some(base), Some(sheet)) = (doc.cels.get(&base_id), doc.cels.get(&sheet_id)) else { return Vec::new() };
        let (w, h) = (base.raster.width, base.raster.height);
        let mut img = base.raster.clone();
        img.composite_over(&warp.rasterize(&sheet.raster, None, w, h));
        let Some(c) = doc.cels.get_mut(&src) else { return Vec::new() };
        let changed: Vec<usize> = (0..img.tiles.len()).filter(|&i| img.tiles[i].is_some() || c.raster.tiles.get(i).is_some_and(Option::is_some)).collect();
        c.raster = img;
        c.version += 1;
        changed
    }

    /// Lâcher, sans rien refaire : la nouvelle géométrie des éléments choisis (`items`, d'états
    /// d'avant `originals`), sans pixels en cache. Rapide.
    pub fn apply_geometry(doc: &mut Document, src: CelId, items: &[usize], originals: &[Element], aff: &Affine) {
        let Some(d) = doc.cels.get_mut(&src).and_then(|c| c.vector.as_mut()) else { return };
        for (k, &i) in items.iter().enumerate() {
            if let (Some(dst), Some(from)) = (d.elements.get_mut(i), originals.get(k)) {
                *dst = from.transformed(aff);
            }
        }
    }

    /// L'ancienne voie, pour terminer le travail tout de suite : les éléments reçoivent la
    /// transformation avec leurs pixels en cache suivis, et l'image du dessin est refaite là où
    /// quelque chose a changé. Rend les tuiles touchées. Coûteux (rotation, échelle : 30 à 70 ms).
    pub fn apply_preview(doc: &mut Document, src: CelId, items: &[usize], originals: &[Element], aff: &Affine) -> Vec<usize> {
        let Some(c) = doc.cels.get_mut(&src) else { return Vec::new() };
        let Some(d) = c.vector.as_mut() else { return Vec::new() };
        let mut region = eframe::egui::Rect::NOTHING;
        for (k, &i) in items.iter().enumerate() {
            if let (Some(dst), Some(from)) = (d.elements.get_mut(i), originals.get(k)) {
                // (L'image du dessin montre encore l'élément tel qu'il était avant le geste, quand
                // le lâcher n'a posé que la géométrie : c'est sa place d'avant qu'il faut aussi
                // refaire, sinon il y reste en double.)
                region = region.union(from.bounds().expand(1.5));
                region = region.union(dst.bounds().expand(1.5));
                *dst = from.transformed_preview(aff);
                region = region.union(dst.bounds().expand(1.5));
            }
        }
        c.rerender_region(region.expand(2.0))
    }

    /// Lâcher : les deux dessins temporaires s'en vont.
    pub fn release(doc: &mut Document) {
        Self::release_slot(doc, 0);
    }

    /// Lâcher la place `slot` : ses deux dessins temporaires s'en vont.
    pub fn release_slot(doc: &mut Document, slot: usize) {
        let (base, sheet) = slot_ids(slot);
        doc.cels.remove(&base);
        doc.cels.remove(&sheet);
    }

    /// Ce que montre le plan à la place de `line`, l'élément du dessin `src` : le dessin sans la
    /// sélection, et sa feuille transformée par-dessus (mêmes opacité, mode et effets).
    pub fn items(&self, line: &PlanItem) -> [PlanItem; 2] {
        let (base_id, sheet_id) = slot_ids(self.slot);
        let base = PlanItem { cel: base_id, ..line.clone() };
        // (Du pixel de la feuille élargie au point du dessin, puis le geste. Un maillage compte
        // déjà la marge.)
        let aff = match self.margin {
            (0, 0) => self.aff,
            _ if self.warp.is_some() => self.aff,
            (l, t) => Affine::translate(vec2(-(l as f32), -(t as f32))).then(&self.aff),
        };
        let sheet = PlanItem { cel: sheet_id, ctg: None, pre: Some(Pre { aff, dims: self.dims, warp: self.warp.clone(), card: false }), ..line.clone() };
        [base, sheet]
    }
}
