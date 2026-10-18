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
//! exact part en arrière-plan (`rejeu.rs`), comme avant. (Pendant le geste, les éléments restent
//! où ils étaient : seule la feuille bouge.)
//!
//! Les deux dessins temporaires vivent dans `Document::cels` le temps du geste ; aucun calque ne
//! les référence, donc ils ne sont jamais enregistrés.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::document::{Cel, CelId, Document};
use crate::render::PlanItem;
use crate::vector::Affine;

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
}

impl Flottante {
    /// Saisie : sépare le dessin `src` en deux images (depuis les caches des traits) et les range
    /// dans `doc` sous `BASE_CEL` et `SHEET_CEL`.
    pub fn lift(doc: &mut Document, src: CelId, items: &[usize]) -> Option<Flottante> {
        let cel = doc.cels.get(&src)?;
        let d = cel.vector.as_ref()?;
        let (w, h) = (cel.raster.width, cel.raster.height);
        let (base, sheet) = d.render_split(Some(items), w, h);
        let version = VERSION.fetch_add(2, Ordering::Relaxed);
        for (id, raster, version) in [(BASE_CEL, base, version), (SHEET_CEL, sheet, version + 1)] {
            let mut c = Cel::bitmap(raster);
            c.version = version;
            doc.cels.insert(id, c);
        }
        Some(Flottante { src, aff: Affine::IDENTITY })
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
        let sheet = PlanItem { cel: SHEET_CEL, ctg: None, pre: Some(self.aff), ..line.clone() };
        [base, sheet]
    }
}
