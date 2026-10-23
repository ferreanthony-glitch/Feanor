//! Outil Sélection sur les calques bitmap : lasso ou rectangle, puis déplacer, mettre à l'échelle,
//! tourner, retourner, effacer dedans ou autour, copier, couper, coller.
//!
//! Les pixels ne sont soulevés qu'au premier déplacement. Tant que la sélection est active, ils
//! restent soulevés : chaque transformation repart de l'image d'origine. Tout est reposé (et
//! enregistré pour l'annulation, en une étape) quand on quitte la sélection.

use eframe::egui::{Pos2, Rect};

use crate::app::{App, Tool};
use crate::document::{CelId, LayerKind};
use crate::history::Edit;
use crate::pixsel::{self, Floating, PixelMask};
use crate::raster::{Raster, Tile};
use crate::selbox::{self, Handle, SelFrame};
use crate::vector::{self, Affine};

pub(crate) struct BSelection {
    pub cel: CelId,
    /// La zone, à sa place d'origine.
    pub mask: PixelMask,
    /// Contours de la zone (affichage), à leur place d'origine.
    pub outline: Vec<Vec<Pos2>>,
    pub lifted: Option<Lifted>,
}

pub(crate) struct Lifted {
    pub floating: Floating,
    /// Le dessin sans les pixels soulevés.
    pub base: Raster,
    /// Le dessin avant la sélection, pour l'annulation.
    pub before: Raster,
    /// Création du dessin (collage sur une image vide), annulée avec le reste.
    pub prefix: Option<Edit>,
    pub aff: Affine,
    /// Zone où les pixels transformés sont posés en ce moment.
    pub placed: Option<Rect>,
    /// Les gestes faits depuis le soulèvement, pour nommer l'étape d'annulation de la pose.
    pub steps: Vec<&'static str>,
}

pub(crate) enum BDrag {
    Lasso { points: Vec<Pos2>, add: bool },
    Rect { from: Pos2, to: Pos2, add: bool },
    /// Un geste de la boîte (`selbox.rs`) ; `frame` : la boîte au début du geste ; `field` : le
    /// geste vient d'un champ chiffré du panneau Réglages.
    Transform { handle: Handle, start: Pos2, frame: SelFrame, field: bool },
    /// Le pivot tiré.
    Pivot { start: Pos2, frame: SelFrame },
}

impl App {
    /// Rectangle englobant de la sélection bitmap, là où elle est affichée.
    pub(crate) fn bitmap_selection_bounds(&self) -> Option<Rect> {
        let sel = self.bsel.as_ref()?;
        Some(match &sel.lifted {
            Some(l) => l.aff.apply_rect(sel.mask.bounds()),
            None => sel.mask.bounds(),
        })
    }

    /// Contours de la sélection bitmap tels qu'affichés.
    pub(crate) fn bitmap_selection_outline(&self) -> Vec<Vec<Pos2>> {
        let Some(sel) = &self.bsel else { return Vec::new() };
        let aff = sel.lifted.as_ref().map_or(Affine::IDENTITY, |l| l.aff);
        sel.outline
            .iter()
            .map(|poly| poly.iter().map(|p| aff.apply(*p)).collect())
            .collect()
    }

    /// La sélection ne vaut que pour le dessin bitmap affiché et l'outil Sélection ; sinon on
    /// repose les pixels et on l'oublie.
    pub(crate) fn validate_bitmap_selection(&mut self) {
        let Some(sel) = &self.bsel else { return };
        let valid = Some(sel.cel) == self.cel_here()
            && self.layer().kind == LayerKind::Bitmap
            && self.tool.keeps_zone();
        if !valid && self.bdrag.is_none() {
            self.drop_bitmap_selection();
        }
    }

    /// Repose les pixels soulevés (une étape d'annulation) et oublie la sélection.
    pub(crate) fn drop_bitmap_selection(&mut self) {
        self.bdrag = None;
        let Some(sel) = self.bsel.take() else { return };
        let Some(lifted) = sel.lifted else { return };
        let Some(c) = self.doc.cels.get(&sel.cel) else { return };
        let tiles: Vec<(usize, Option<Tile>)> = (0..c.raster.tiles.len())
            .filter(|&i| lifted.before.tiles.get(i) != c.raster.tiles.get(i))
            .map(|i| (i, lifted.before.tiles[i].clone()))
            .collect();
        if tiles.is_empty() && lifted.prefix.is_none() {
            return;
        }
        // L'étape porte le nom du geste (« Tourner »…), ou « Transformer » s'il y en a eu plusieurs.
        let mut steps = lifted.steps;
        steps.dedup();
        let name = if let [one] = steps.as_slice() { *one } else { "Transformer la sélection" };
        self.named_step(name, |app| app.push_pixels(sel.cel, lifted.prefix, tiles));
    }

    /// Avant une opération sur la structure des plans (réunir, couper, dupliquer, une version, une variante, un rognage…) : les
    /// sélections en cours se posent, chacune avec son étape — une zone bitmap soulevée (déplacée, tournée), la feuille flottante
    /// d'une sélection vectorielle lâchée, la sélection sur plusieurs calques. Sinon l'opération copiait ou rangeait le dessin à mi-geste,
    /// et la zone soulevée restait sur un dessin qui n'était plus celui du plan : une nouvelle version faite pendant un lasso
    /// gardait les pixels déplacés sans étape pour les défaire (passe de débogage au hasard, 2026-10-23, graine 4667).
    pub(crate) fn poser_les_selections(&mut self) {
        self.drop_bitmap_selection();
        self.flush_flottante();
        self.multi_before_history();
    }

    /// Un geste de plus sur les pixels soulevés (le nom de l'étape d'annulation de la pose).
    pub(crate) fn note_bitmap_step(&mut self, handle: Handle) {
        self.note_bitmap_name(handle.step_name());
    }

    pub(crate) fn note_bitmap_name(&mut self, name: &'static str) {
        if let Some(l) = self.bsel.as_mut().and_then(|s| s.lifted.as_mut()) {
            l.steps.push(name);
        }
    }

    /// Clic avec l'outil Sélection sur un calque bitmap.
    pub(crate) fn bitmap_select_press(&mut self, p: Pos2, screen: Pos2, shift: bool) {
        self.playing = false;
        self.finish_field_edit();
        // La boîte de la zone : poignées, bords, extérieur d'un coin, pivot, intérieur.
        if let Some((handle, frame)) = self.box_press(screen, shift) {
            if handle == Handle::Pivot {
                self.bdrag = Some(BDrag::Pivot { start: p, frame });
            } else {
                self.begin_bitmap_transform(handle, p, frame);
            }
            return;
        }
        if !shift || self.bsel.as_ref().is_some_and(|s| s.lifted.is_some()) {
            self.drop_bitmap_selection();
        }
        self.bdrag = Some(if self.select_rect {
            BDrag::Rect {
                from: p,
                to: p,
                add: shift,
            }
        } else {
            BDrag::Lasso {
                points: vec![p],
                add: shift,
            }
        });
    }

    /// Soulève les pixels de la zone s'ils ne le sont pas encore.
    pub(crate) fn lift_selection(&mut self) -> bool {
        let Some(sel) = self.bsel.as_mut() else { return false };
        if sel.lifted.is_some() {
            return true;
        }
        let Some(c) = self.doc.cels.get_mut(&sel.cel) else { return false };
        let before = c.raster.clone();
        let floating = pixsel::lift(&mut c.raster, &sel.mask);
        let base = c.raster.clone();
        let placed = pixsel::stamp(&mut c.raster, &floating, &Affine::IDENTITY);
        sel.lifted = Some(Lifted {
            floating,
            base,
            before,
            prefix: None,
            aff: Affine::IDENTITY,
            placed,
            steps: Vec::new(),
        });
        true
    }

    fn begin_bitmap_transform(&mut self, handle: Handle, p: Pos2, frame: SelFrame) {
        if !self.layer_editable() || !self.lift_selection() {
            return;
        }
        self.note_bitmap_step(handle);
        self.bdrag = Some(BDrag::Transform { handle, start: p, frame, field: false });
    }

    /// Pose les pixels soulevés avec la transformation `aff` (à la place de la précédente).
    pub(crate) fn place_floating(&mut self, aff: Affine) {
        let Some(sel) = self.bsel.as_mut() else { return };
        let Some(lifted) = sel.lifted.as_mut() else { return };
        let Some(c) = self.doc.cels.get_mut(&sel.cel) else { return };
        let mut dirty = Vec::new();
        if let Some(r) = lifted.placed {
            for idx in vector::tiles_in(&c.raster, r) {
                c.raster.tiles[idx] = lifted.base.tiles[idx].clone();
                dirty.push(idx);
            }
        }
        lifted.placed = pixsel::stamp(&mut c.raster, &lifted.floating, &aff);
        if let Some(r) = lifted.placed {
            dirty.extend(vector::tiles_in(&c.raster, r));
        }
        lifted.aff = aff;
        c.version += 1;
        self.renderer.invalidate_tiles(&dirty);
        self.modified = true;
    }

    pub(crate) fn bitmap_drag(&mut self, p: Pos2, shift: bool, ctrl: bool) {
        let zoom = self.layer_zoom();
        let mods = self.box_mods(shift, ctrl);
        match &mut self.bdrag {
            Some(BDrag::Lasso { points, .. }) => {
                if points.last().is_none_or(|q| q.distance(p) * zoom > 1.5) {
                    points.push(p);
                }
            }
            Some(BDrag::Rect { to, .. }) => *to = p,
            Some(BDrag::Transform { handle, start, frame, field: false }) => {
                let (handle, frame) = (*handle, *frame);
                let mut t = selbox::gesture(handle, &frame, *start, p, mods);
                if handle == Handle::Move {
                    // Déplacement au pixel près : l'image reste nette.
                    t.tx = t.tx.round();
                    t.ty = t.ty.round();
                }
                let after = frame.then(&t);
                self.place_floating(after.m);
                self.status = crate::seltools::gesture_text(handle, &after.params());
            }
            Some(BDrag::Pivot { start, frame }) => {
                let (start, frame) = (*start, *frame);
                self.drag_pivot(&frame, start, p);
            }
            _ => {}
        }
    }

    pub(crate) fn finish_bdrag(&mut self) {
        let Some(drag) = self.bdrag.take() else { return };
        let (polygon, add, click) = match drag {
            BDrag::Transform { .. } | BDrag::Pivot { .. } => return,
            BDrag::Lasso { points, add } => {
                let click = points.first().copied();
                let screen: Vec<Pos2> = points.iter().map(|q| self.layer_to_screen(*q)).collect();
                let extent = Rect::from_points(&screen);
                if points.len() < 3 || extent.width().max(extent.height()) < 4.0 {
                    (None, add, click)
                } else {
                    (Some(points), add, click)
                }
            }
            BDrag::Rect { from, to, add } => {
                let r = Rect::from_two_pos(from, to);
                if r.width().min(r.height()) * self.layer_zoom() < 3.0 {
                    (None, add, Some(from))
                } else {
                    (Some(pixsel::rect_polygon(r)), add, Some(from))
                }
            }
        };
        // Un simple clic sur un dessin choisit l'objet sous la souris (`selobjet.rs`) ; dans le vide,
        // il désélectionne.
        let Some(polygon) = polygon else {
            if click.is_some_and(|p| self.bitmap_pick_object(p, add)) {
                return;
            }
            if !add {
                self.drop_bitmap_selection();
            }
            return;
        };
        let Some(cel) = self.cel_here() else {
            self.status = "Rien à sélectionner : cette image est vide.".into();
            return;
        };
        let (w, h) = (self.doc.width, self.doc.height);
        let Some(mask) = PixelMask::from_polygons(std::slice::from_ref(&polygon), w, h) else { return };
        match self.bsel.as_mut().filter(|s| add && s.cel == cel && s.lifted.is_none()) {
            Some(sel) => {
                sel.mask = sel.mask.union(&mask);
                sel.outline.push(polygon);
            }
            None => {
                self.bsel = Some(BSelection {
                    cel,
                    mask,
                    outline: vec![polygon],
                    lifted: None,
                });
            }
        }
    }

    /// Toute l'image.
    pub(crate) fn bitmap_select_all(&mut self) {
        self.drop_bitmap_selection();
        let Some(cel) = self.cel_here() else { return };
        let r = Rect::from_min_size(Pos2::ZERO, self.canvas_size());
        let polygon = pixsel::rect_polygon(r);
        let Some(mask) = PixelMask::from_polygons(std::slice::from_ref(&polygon), self.doc.width, self.doc.height) else {
            return;
        };
        self.bsel = Some(BSelection {
            cel,
            mask,
            outline: vec![polygon],
            lifted: None,
        });
        self.tool = Tool::Select;
    }

    /// Retourne la sélection (horizontalement ou verticalement) autour de son centre.
    pub(crate) fn bitmap_flip(&mut self, horizontal: bool) {
        let Some(b) = self.bitmap_selection_bounds() else { return };
        if !self.layer_editable() || !self.lift_selection() {
            return;
        }
        let Some(aff) = self.bsel.as_ref().and_then(|s| s.lifted.as_ref()).map(|l| l.aff) else { return };
        self.note_bitmap_name("Retourner");
        let flip = if horizontal {
            Affine::scale_about(b.center(), -1.0, 1.0)
        } else {
            Affine::scale_about(b.center(), 1.0, -1.0)
        };
        self.place_floating(aff.then(&flip));
    }

    /// Efface ce qui est dans la sélection (`inside`) ou autour.
    pub(crate) fn bitmap_erase(&mut self, inside: bool) {
        if !self.layer_editable() {
            return;
        }
        let Some(sel) = self.bsel.as_mut() else { return };
        let Some(c) = self.doc.cels.get_mut(&sel.cel) else { return };
        if let Some(lifted) = &sel.lifted {
            if inside {
                c.raster = lifted.base.clone();
            } else {
                let mut only = Raster::new(c.raster.width, c.raster.height);
                pixsel::stamp(&mut only, &lifted.floating, &lifted.aff);
                c.raster = only;
            }
            // Une seule étape d'annulation, comparée au dessin d'avant la sélection.
            self.drop_bitmap_selection();
        } else {
            let before = c.raster.clone();
            if inside {
                pixsel::lift(&mut c.raster, &sel.mask);
            } else {
                pixsel::keep_only(&mut c.raster, &sel.mask);
            }
            let tiles: Vec<(usize, Option<Tile>)> = (0..c.raster.tiles.len())
                .filter(|&i| before.tiles[i] != c.raster.tiles[i])
                .map(|i| (i, before.tiles[i].clone()))
                .collect();
            let cel = sel.cel;
            // Effacer autour garde la sélection : ce qui reste est dedans.
            if inside {
                self.bsel = None;
            }
            if !tiles.is_empty() {
                self.push_pixels(cel, None, tiles);
            }
        }
        self.renderer.invalidate_all();
    }

    /// Pixels de la sélection, tels qu'affichés (transformés s'ils ont été déplacés).
    fn selected_pixels(&self) -> Option<Floating> {
        let sel = self.bsel.as_ref()?;
        let c = self.doc.cels.get(&sel.cel)?;
        let f = match &sel.lifted {
            None => pixsel::copy(&c.raster, &sel.mask),
            Some(l) => {
                let mut r = Raster::new(c.raster.width, c.raster.height);
                let placed = pixsel::stamp(&mut r, &l.floating, &l.aff)?;
                let mask = PixelMask::from_polygons(&[pixsel::rect_polygon(placed)], r.width, r.height)?;
                pixsel::lift(&mut r, &mask)
            }
        };
        (!f.is_empty()).then_some(f)
    }

    pub(crate) fn bitmap_copy(&mut self) -> bool {
        let Some(f) = self.selected_pixels() else {
            self.status = "Rien à copier dans la sélection.".into();
            return false;
        };
        self.status = format!("Zone de {}×{} pixels copiée.", f.w, f.h);
        // Un texte dans le presse-papiers du système : sans lui, Ctrl+V n'arrive pas jusqu'à nous.
        self.ctx.copy_text(format!("SHOT : zone bitmap {}×{}", f.w, f.h));
        self.pixel_clipboard = Some(f);
        self.collage.origines.zone = Some(crate::adaptation::Origine::de(&self.doc));
        self.multi_clip = None;
        true
    }

    pub(crate) fn bitmap_cut(&mut self) {
        if self.bitmap_copy() {
            self.bitmap_erase(true);
        }
    }

    /// Colle la zone copiée à sa place d'origine, sélectionnée et prête à être déplacée.
    pub(crate) fn bitmap_paste(&mut self) {
        let Some(f) = self.pixel_clipboard.clone() else { return };
        // D'un projet d'une autre feuille : la zone est adaptée et garde sa place dans le cadre (adaptation.rs).
        let f = match self.collage_preparer(crate::collage::Colle::Zone(f), crate::collage::Reprise::Zone) {
            crate::collage::Preparation::Pret(crate::collage::Colle::Zone(f)) => f,
            _ => return,
        };
        if self.paste_floating(f) {
            self.status = "Zone collée : déplace-la, puis Échap ou change d'outil pour la poser.".into();
        }
    }

    /// Pose les pixels `f` sur le dessin courant, soulevés et sélectionnés (outil Sélection),
    /// prêts à être déplacés ; faux si le calque ne peut pas être modifié.
    pub(crate) fn paste_floating(&mut self, f: Floating) -> bool {
        self.end_stroke();
        self.drop_bitmap_selection();
        if !self.layer_editable() {
            return false;
        }
        let (cel, prefix) = self.ensure_cel();
        let Some(c) = self.doc.cels.get_mut(&cel) else { return false };
        let before = c.raster.clone();
        let base = c.raster.clone();
        let placed = pixsel::stamp(&mut c.raster, &f, &Affine::IDENTITY);
        c.version += 1;
        let polygon = pixsel::rect_polygon(f.bounds());
        let Some(mask) = PixelMask::from_polygons(std::slice::from_ref(&polygon), self.doc.width, self.doc.height) else {
            return false;
        };
        self.bsel = Some(BSelection {
            cel,
            mask,
            outline: vec![polygon],
            lifted: Some(Lifted {
                floating: f,
                base,
                before,
                prefix,
                aff: Affine::IDENTITY,
                placed,
                steps: Vec::new(),
            }),
        });
        self.tool = Tool::Select;
        self.modified = true;
        self.renderer.invalidate_all();
        true
    }
}
