//! Choisir une couleur recolore la sélection (Anthony, 2026-10-24).
//!
//! Avec une sélection active (lasso, rectangle, sélection par couleur : l'outil Sélection et ses cousins), choisir une autre couleur
//! d'avant-plan dans le panneau Couleur — clic sur une pastille, roue ou curseurs, code hexadécimal, intervertir, couleurs par défaut —
//! donne cette couleur à l'objet sélectionné :
//!
//! - **vectoriel** : les traits, les zones et les textes choisis prennent la couleur (la couleur de l'élément, rien d'autre) ;
//! - **bitmap** : les pixels de la zone sont recolorés en **gardant leur alpha** (la couverture et l'anticrénelage restent ceux du
//!   dessin : on remplace la couleur, pas la forme), au prorata de la couverture de la zone sur son bord.
//!
//! Sans sélection, rien ne change : la couleur choisie ne sert qu'aux outils, comme avant. La pipette, le texte et les autres chemins qui
//! changent la couleur d'avant-plan sans passer par le panneau ne recolorent rien. Une sélection sur plusieurs calques (`multisel.rs`)
//! n'est pas recolorée.
//!
//! **Une étape d'annulation par choix de couleur** : glisser un curseur ou la roue, ou taper un code dans le champ, en fait une seule,
//! pour tout le geste (`App::gesture_change` : la même clé tant que le geste dure et qu'aucune autre étape ne s'est glissée). Le
//! rendu suit sans latence : les traits sans moteur (zones, textes, traits simples) sont redessinés dans la zone touchée seulement ; ceux
//! du moteur de brosses sont rejoués par le fil de travail (`rejeu.rs`), jamais sur le fil de l'interface, et l'ancienne image reste
//! affichée jusqu'à l'arrivée de la nouvelle.

use eframe::egui::Color32;

use crate::app::App;
use crate::history::Edit;
use crate::pixsel::{Floating, PixelMask};
use crate::raster::{Tile, TILE};
use crate::vector::Element;

/// La clé du geste « choisir une couleur » dans `History::Gesture`.
pub(crate) const CLE_GESTE: u64 = 0x5EC0_1012;
/// Le nom de l'étape d'annulation (panneau Historique).
const NOM_ETAPE: &str = "Couleur de la sélection";

/// Le pixel prémultiplié `p`, recoloré de `couleur` au prorata `m` (0 à 1), son alpha gardé.
pub(crate) fn recolorer_pixel(p: Color32, couleur: [u8; 3], m: f32) -> Color32 {
    let a = p.a();
    if a == 0 || m <= 0.0 {
        return p;
    }
    let af = f32::from(a) / 255.0;
    // La couleur d'avant (non prémultipliée), mêlée à la nouvelle selon la couverture de la zone, puis prémultipliée de nouveau.
    let canal = |avant: u8, apres: u8| {
        let avant = (f32::from(avant) / af).min(255.0);
        let mixte = avant + (f32::from(apres) - avant) * m.min(1.0);
        ((mixte * af).round().clamp(0.0, f32::from(a))) as u8
    };
    Color32::from_rgba_premultiplied(canal(p.r(), couleur[0]), canal(p.g(), couleur[1]), canal(p.b(), couleur[2]), a)
}

/// Les pixels soulevés `f` recolorés (leur couverture est déjà dans leur alpha) ; vrai si un pixel a changé.
pub(crate) fn recolorer_soulevee(f: &mut Floating, couleur: [u8; 3]) -> bool {
    let mut change = false;
    for p in &mut f.pixels {
        let nouveau = recolorer_pixel(*p, couleur, 1.0);
        if nouveau != *p {
            *p = nouveau;
            change = true;
        }
    }
    change
}

/// Les tuiles de `raster` que touche la zone `mask`, avec leur tuile recolorée si elle change : (numéro, tuile d'avant, tuile d'après).
fn tuiles_recolorees(raster: &crate::raster::Raster, mask: &PixelMask, couleur: [u8; 3]) -> (Vec<(usize, Option<Tile>)>, Vec<(usize, Tile)>) {
    let (mut candidates, mut nouvelles) = (Vec::new(), Vec::new());
    if mask.w == 0 || mask.h == 0 {
        return (candidates, nouvelles);
    }
    let (tx0, ty0) = (mask.x0 / TILE, mask.y0 / TILE);
    let (tx1, ty1) = (((mask.x0 + mask.w - 1) / TILE).min(raster.tiles_x.saturating_sub(1)), ((mask.y0 + mask.h - 1) / TILE).min(raster.height.div_ceil(TILE).saturating_sub(1)));
    for ty in ty0..=ty1 {
        for tx in tx0..=tx1 {
            let idx = ty * raster.tiles_x + tx;
            let Some(Some(tile)) = raster.tiles.get(idx) else { continue };
            candidates.push((idx, Some(tile.clone())));
            let (ox, oy, tw, th) = raster.tile_bounds(idx);
            let mut copie: Option<Tile> = None;
            for y in oy.max(mask.y0)..(oy + th).min(mask.y0 + mask.h) {
                for x in ox.max(mask.x0)..(ox + tw).min(mask.x0 + mask.w) {
                    let m = mask.data[(y - mask.y0) * mask.w + (x - mask.x0)];
                    if m <= 0.0 {
                        continue;
                    }
                    let (pos, p) = ((y % TILE) * TILE + x % TILE, tile[(y % TILE) * TILE + x % TILE]);
                    let nouveau = recolorer_pixel(p, couleur, m);
                    if nouveau != p {
                        // (La copie n'est faite qu'à la première différence : une tuile qui ne change pas ne coûte rien.)
                        copie.get_or_insert_with(|| tile.clone())[pos] = nouveau;
                    }
                }
            }
            if let Some(t) = copie {
                nouvelles.push((idx, t));
            }
        }
    }
    (candidates, nouvelles)
}

impl App {
    /// Une couleur vient d'être choisie dans le panneau Couleur : si une sélection est active, elle la prend. Sans sélection, rien
    /// ne se passe (la couleur ne sert qu'aux outils).
    pub(crate) fn recolorier_la_selection(&mut self, couleur: [u8; 3]) {
        // Seulement quand la sélection est celle qu'on voit : un outil qui la garde, un calque qu'on peut modifier, aucun geste de
        // la boîte en cours, et pas une sélection sur plusieurs calques.
        if !self.tool.keeps_zone() || self.multi.sel.is_some() || self.vdrag.is_some() || self.bdrag.is_some() {
            return;
        }
        let calque = self.layer();
        if !calque.drawable() || calque.video.is_some() || self.doc.storyboard_locked {
            return;
        }
        if self.bsel.is_some() {
            self.recolorier_bitmap(couleur);
        } else if self.vsel.is_some() {
            self.recolorier_vecteur(couleur);
        }
    }

    /// La sélection vectorielle prend `couleur`.
    fn recolorier_vecteur(&mut self, couleur: [u8; 3]) {
        let Some(sel) = self.vsel.as_ref().filter(|s| !s.items.is_empty() && Some(s.cel) == self.cel_here()) else { return };
        let (cel, items) = (sel.cel, sel.items.clone());
        // Un geste lâché dont la feuille flottante attend encore son rendu : on le pose d'abord.
        self.flush_flottante();
        let Some(d) = self.drawing(cel) else { return };
        if items.iter().all(|&i| d.elements.get(i).is_none_or(|e| e.color() == couleur)) {
            return;
        }
        // Une étape d'annulation par geste : le dessin d'avant, pris au premier changement seulement.
        self.gesture_change(CLE_GESTE, |app| {
            if let Some(avant) = app.drawing(cel).cloned() {
                app.named_step(NOM_ETAPE, |a| a.push_vector(cel, avant, None));
            }
        });
        let mut moteur = false;
        if let Some(d) = self.drawing_mut(cel) {
            for &i in &items {
                if let Some(e) = d.elements.get_mut(i) {
                    moteur |= matches!(e, Element::Stroke { brush: Some(_), .. });
                    e.set_color(couleur);
                }
            }
        }
        if let Some(c) = self.doc.cels.get_mut(&cel) {
            c.version += 1;
        }
        if moteur {
            // Les traits du moteur de brosses : rejoués par le fil de travail (jamais sur le fil de l'interface) ; l'ancienne image
            // reste affichée jusqu'à l'arrivée de la nouvelle.
            self.rejeu_cel(cel);
        } else if let Some(zone) = self.selection_bounds() {
            // Les autres éléments se redessinent tout de suite, dans la zone touchée seulement.
            self.refresh_cel_region(cel, zone);
        }
    }

    /// La sélection bitmap est recolorée : les pixels de la zone prennent `couleur`, leur alpha gardé.
    fn recolorier_bitmap(&mut self, couleur: [u8; 3]) {
        let Some(sel) = self.bsel.as_ref().filter(|s| Some(s.cel) == self.cel_here()) else { return };
        let cel = sel.cel;
        // Des pixels soulevés (la zone a été déplacée, tournée…) : on les recolore, ils se reposent avec le reste du geste de
        // transformation (une étape d'annulation, à la pose).
        if sel.lifted.is_some() {
            let Some(sel) = self.bsel.as_mut() else { return };
            let Some(lifted) = sel.lifted.as_mut() else { return };
            if !recolorer_soulevee(std::sync::Arc::make_mut(&mut lifted.floating), couleur) {
                return;
            }
            let aff = lifted.aff;
            if lifted.steps.last() != Some(&NOM_ETAPE) {
                lifted.steps.push(NOM_ETAPE);
            }
            self.place_floating(aff);
            return;
        }
        let Some(c) = self.doc.cels.get(&cel) else { return };
        let (candidates, nouvelles) = tuiles_recolorees(&c.raster, &sel.mask, couleur);
        if nouvelles.is_empty() {
            return;
        }
        // Une étape d'annulation par geste : les tuiles de la zone telles qu'elles étaient avant le premier changement (toutes celles
        // que la zone touche, pas seulement celles qui ont changé la première fois : la couleur peut en changer d'autres ensuite).
        let avant = self.gesture.needs_step(CLE_GESTE, self.history.serial()).then_some(candidates);
        self.gesture_change(CLE_GESTE, move |app| {
            if let Some(tiles) = avant {
                app.named_step(NOM_ETAPE, |a| a.history.push(Edit::Tiles { cel, tiles }));
            }
        });
        let Some(c) = self.doc.cels.get_mut(&cel) else { return };
        let ids: Vec<usize> = nouvelles.iter().map(|(i, _)| *i).collect();
        for (idx, tuile) in nouvelles {
            c.raster.replace_slot(idx, Some(tuile));
        }
        c.version += 1;
        self.renderer.invalidate_tiles(&ids);
        self.modified = true;
    }

    /// À chaque image : tant qu'un curseur, la roue ou le champ du code tient le geste, l'étape d'annulation de la couleur
    /// continue (une seule pour tout le geste).
    pub(crate) fn recolor_geste_tick(&mut self, ctx: &eframe::egui::Context) {
        if ctx.input(|i| i.pointer.any_down()) || ctx.wants_keyboard_input() {
            self.keep_gesture(CLE_GESTE);
        }
    }
}
