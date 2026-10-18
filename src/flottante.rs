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

use eframe::egui::vec2;

use crate::document::{Cel, CelId, Document};
use crate::render::{PlanItem, Pre};
use crate::vector::{Affine, Element};

/// Le dessin sans les éléments choisis.
pub const BASE_CEL: CelId = 1 << 62;
/// Les éléments choisis seuls.
pub const SHEET_CEL: CelId = (1 << 62) + 1;

/// Numéro de version des feuilles : neuf à chaque saisie (la carte graphique les renvoie).
static VERSION: AtomicU64 = AtomicU64::new(1 << 40);

/// Un geste en cours sur la sélection du dessin `src`.
pub struct Flottante {
    pub src: CelId,
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
        let cel = doc.cels.get(&src)?;
        let d = cel.vector.as_ref()?;
        let (w, h) = (cel.raster.width, cel.raster.height);
        let (base, sheet, margin) = d.render_split_extended(items, w, h);
        let dims = ((sheet.width, sheet.height) != (w, h)).then_some([sheet.width, sheet.height]);
        let version = VERSION.fetch_add(2, Ordering::Relaxed);
        for (id, raster, version) in [(BASE_CEL, base, version), (SHEET_CEL, sheet, version + 1)] {
            let mut c = Cel::bitmap(raster);
            c.version = version;
            doc.cels.insert(id, c);
        }
        Some(Flottante { src, aff: Affine::IDENTITY, pending: None, margin, dims })
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
        doc.cels.remove(&BASE_CEL);
        doc.cels.remove(&SHEET_CEL);
    }

    /// Ce que montre le plan à la place de `line`, l'élément du dessin `src` : le dessin sans la
    /// sélection, et sa feuille transformée par-dessus (mêmes opacité, mode et effets).
    pub fn items(&self, line: &PlanItem) -> [PlanItem; 2] {
        let base = PlanItem { cel: BASE_CEL, ..line.clone() };
        // (Du pixel de la feuille élargie au point du dessin, puis le geste.)
        let aff = match self.margin {
            (0, 0) => self.aff,
            (l, t) => Affine::translate(vec2(-(l as f32), -(t as f32))).then(&self.aff),
        };
        let sheet = PlanItem { cel: SHEET_CEL, ctg: None, pre: Some(Pre { aff, dims: self.dims }), ..line.clone() };
        [base, sheet]
    }
}
