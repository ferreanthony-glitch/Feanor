//! L'outil Dégradé, côté interface (`degrade.rs` : le noyau). Anthony, 2026-10-24 : « un outil dégradé, vecto et bitmap ».
//!
//! * **Le geste** : appui, glisser, lâcher. Le glisser trace l'axe, du point d'appui (couleur d'avant-plan) au pointeur
//!   (couleur d'arrière-plan, ou la même devenue transparente : Réglages). Maj : l'axe par pas de 15°.
//! * **Où il s'applique**, dans cet ordre : la sélection s'il y en a une (lasso, rectangle, couleur : bitmap ; zones
//!   remplies choisies : vectoriel) ; sinon la zone fermée du trait où l'on appuie, trouvée comme le fait le pot de
//!   peinture (`fill.rs`) ; Alt tenu (ou « Tout le calque » dans Réglages) : toute la feuille. Sur l'aplat visé et sur un
//!   calque de couleur, la zone est celle que le pot de peinture y remplit.
//! * **Bitmap** : les pixels s'écrivent au lâcher (`degrade::peindre`, par tuiles, sur les cœurs), bords anticrénelés comme
//!   avec le pot de peinture ou la sélection. **Vectoriel** : la zone garde un dégradé (`Element::Fill::gradient`), jamais
//!   des pixels ; il se rend avec elle, suit sa transformation, s'enregistre (format 38).
//! * **Sans latence** : pendant le glisser, rien du moteur de pinceaux ne tourne. L'aperçu est un maillage de sommets
//!   colorés (le dégradé, calculé aux sommets d'une grille) posé sur une petite image de la zone (sa couverture, faite une
//!   fois à l'appui) : quelques milliers de sommets par image d'écran, quelle que soit la taille de la zone.
//! * Une seule étape d'annulation par dégradé.

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect, Shape, Stroke};

use crate::app::App;
use crate::degrade::{self, Degrade, Fin, Genre, Portee, Reglages};
use crate::document::{CelId, LayerKind};
use crate::pixsel::PixelMask;
use crate::raster::Raster;
use crate::vector::{self, Element};

/// Où le dégradé s'écrit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Dest {
    /// Le dessin du calque.
    Calque,
    /// La peinture de l'aplat du calque.
    Aplat,
    /// Le dessin d'un calque de couleur.
    Couleur,
}

/// Ce que le dégradé couvre, en pixels (calques bitmap, aplat, calque de couleur).
pub(crate) enum PorteeGeste {
    Tout,
    Zone { region: Vec<bool>, grown: Vec<bool> },
    Masque(PixelMask),
}

impl PorteeGeste {
    fn vue(&self) -> Portee<'_> {
        match self {
            PorteeGeste::Tout => Portee::Tout,
            PorteeGeste::Zone { region, grown } => Portee::Zone { region, grown },
            PorteeGeste::Masque(m) => Portee::Masque(m),
        }
    }
}

/// Ce que le geste va changer.
pub(crate) enum Cible {
    /// Des pixels.
    Pixels { dest: Dest, portee: PorteeGeste },
    /// Des zones remplies déjà là (vectoriel) : elles reçoivent le dégradé.
    Elements { cel: CelId, items: Vec<usize> },
    /// Une zone neuve (vectoriel), tracée au lâcher d'après la zone trouvée.
    Zone { grown: Vec<bool> },
    /// Toute la feuille (vectoriel) : une zone neuve qui la couvre.
    Feuille,
}

/// L'image de la couverture de la zone, pour l'aperçu : blanche, d'opacité la couverture, sur le rectangle `rect` (pixels
/// du dessin).
pub(crate) struct Apercu {
    pub rect: Rect,
    pub tex: Option<egui::TextureHandle>,
}

/// Le geste en cours.
pub(crate) struct Geste {
    pub depart: Pos2,
    pub fin: Pos2,
    pub reglages: Reglages,
    pub avant: [u8; 3],
    pub arriere: [u8; 3],
    pub cible: Cible,
    pub apercu: Apercu,
}

impl Geste {
    /// Le dégradé tel que l'axe tiré le donne maintenant.
    pub(crate) fn degrade(&self) -> Degrade {
        self.reglages.degrade(self.avant, self.arriere, self.depart, self.fin)
    }
}

/// Au plus ce nombre de pixels dans l'image de couverture de l'aperçu : au-delà, elle est réduite (le bord de la zone y
/// est un peu plus doux, le dégradé n'en dépend pas).
const APERCU_PIXELS: usize = 4_000_000;
/// Cases de la grille de l'aperçu, par côté.
const GRILLE: usize = 56;
/// Longueur d'axe (pixels de l'écran) sous laquelle un lâcher n'est pas un dégradé (un clic).
const AXE_MIN: f32 = 3.0;

/// L'image de couverture de `cover` sur le rectangle (`x0`, `y0`, `w`, `h`) du dessin.
fn apercu_de(ctx: &egui::Context, (x0, y0, w, h): (usize, usize, usize, usize), cover: &dyn Fn(usize, usize) -> f32) -> Apercu {
    let rect = Rect::from_min_size(pos2(x0 as f32, y0 as f32), vec2(w as f32, h as f32));
    if w == 0 || h == 0 {
        return Apercu { rect, tex: None };
    }
    let step = ((w * h) as f32 / APERCU_PIXELS as f32).sqrt().ceil().max(1.0) as usize;
    let (tw, th) = (w.div_ceil(step), h.div_ceil(step));
    let mut bytes = Vec::with_capacity(tw * th * 4);
    for j in 0..th {
        for i in 0..tw {
            let c = (cover((x0 + i * step).min(x0 + w - 1), (y0 + j * step).min(y0 + h - 1)).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            bytes.extend_from_slice(&[c, c, c, c]);
        }
    }
    let image = egui::ColorImage::from_rgba_premultiplied([tw, th], &bytes);
    Apercu { rect, tex: Some(ctx.load_texture("degrade-apercu", image, egui::TextureOptions::LINEAR)) }
}

/// Le rectangle (`x0`, `y0`, `w`, `h`) des pixels vrais de `mask` (`w` × `h` : la feuille), ou le rectangle vide.
fn boite_de(mask: &[bool], w: usize, h: usize) -> (usize, usize, usize, usize) {
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
    for y in 0..h {
        let row = &mask[y * w..(y + 1) * w];
        if let Some(a) = row.iter().position(|&b| b) {
            let b = row.iter().rposition(|&b| b).unwrap_or(a);
            x0 = x0.min(a);
            x1 = x1.max(b + 1);
            y0 = y0.min(y);
            y1 = y1.max(y + 1);
        }
    }
    if x0 >= x1 {
        (0, 0, 0, 0)
    } else {
        (x0, y0, x1 - x0, y1 - y0)
    }
}

impl App {
    /// Appui avec l'outil Dégradé, au point `p` du dessin ; `alt` : tout le calque.
    pub(crate) fn degrade_press(&mut self, p: Pos2, alt: bool) {
        self.end_stroke();
        self.playing = false;
        self.degrade_geste = None;
        if self.layer().couleur.as_ref().is_some_and(|c| c.griffonner) {
            self.status = "Le dégradé ne se pose pas en mode « Griffonner » : éteins-le (menu du calque de couleur).".into();
            return;
        }
        if !self.layer_editable() {
            return;
        }
        let tout = alt || self.degrade.tout_le_calque;
        let (w, h) = (self.doc.width, self.doc.height);
        let dedans = p.x >= 0.0 && p.y >= 0.0 && p.x < w as f32 && p.y < h as f32;
        let hors = "Le dégradé remplit dans la feuille : appuie sur une zone fermée de la feuille (Alt : tout le calque).";
        let aplat = self.aplat_en_cible();
        let couleur = self.layer().couleur.is_some();
        let cible_apercu: Option<(Cible, Apercu)> = if aplat {
            self.degrade_cible_aplat(p, tout, dedans, hors)
        } else if couleur {
            self.degrade_cible_couleur(p, tout, dedans, hors)
        } else if self.layer().kind == LayerKind::Vector {
            self.degrade_cible_vecteur(p, tout, dedans, hors)
        } else {
            self.degrade_cible_bitmap(p, tout, dedans, hors)
        };
        let Some((cible, apercu)) = cible_apercu else { return };
        self.degrade_geste = Some(Geste { depart: p, fin: p, reglages: self.degrade, avant: self.color, arriere: self.couleurs.prefs.back, cible, apercu });
    }

    fn degrade_apercu_pixels(&self, portee: &PorteeGeste) -> Apercu {
        let (w, h) = (self.doc.width, self.doc.height);
        match portee {
            PorteeGeste::Tout => Apercu { rect: Rect::from_min_size(Pos2::ZERO, vec2(w as f32, h as f32)), tex: None },
            PorteeGeste::Zone { grown, .. } => apercu_de(&self.ctx, boite_de(grown, w, h), &|x, y| if grown[y * w + x] { 1.0 } else { 0.0 }),
            PorteeGeste::Masque(m) => apercu_de(&self.ctx, (m.x0, m.y0, m.w, m.h), &|x, y| m.at(x, y)),
        }
    }

    /// Une zone trouvée comme le fait le pot de peinture sur le dessin du calque (ou sur tous les calques, d'après ses
    /// réglages) : la zone et la zone élargie du débord.
    fn degrade_zone_du_pot(&self, p: Pos2) -> (Vec<bool>, Vec<bool>) {
        let (w, h) = (self.doc.width, self.doc.height);
        let source = self.fill_source();
        let tolerance = (self.fill.tolerance * 255.0).round() as u8;
        let region = crate::fill::flood(&source, w, h, p.x as usize, p.y as usize, tolerance);
        let grown = crate::fill::dilate(&region, w, h, self.fill.expand);
        (region, grown)
    }

    fn degrade_cible_bitmap(&mut self, p: Pos2, tout: bool, dedans: bool, hors: &str) -> Option<(Cible, Apercu)> {
        // Une sélection bitmap : le dégradé n'en remplit que la zone.
        if let Some(sel) = self.bsel.as_ref().filter(|s| Some(s.cel) == self.cel_here()) {
            if sel.lifted.is_some() {
                self.status = "La zone est soulevée : pose-la d'abord (Échap ou autre outil), puis fais le dégradé.".into();
                return None;
            }
            let portee = PorteeGeste::Masque(sel.mask.clone());
            let apercu = self.degrade_apercu_pixels(&portee);
            return Some((Cible::Pixels { dest: Dest::Calque, portee }, apercu));
        }
        let portee = if tout {
            PorteeGeste::Tout
        } else if !dedans {
            self.status = hors.into();
            return None;
        } else {
            let (region, grown) = self.degrade_zone_du_pot(p);
            PorteeGeste::Zone { region, grown }
        };
        let apercu = self.degrade_apercu_pixels(&portee);
        Some((Cible::Pixels { dest: Dest::Calque, portee }, apercu))
    }

    fn degrade_cible_aplat(&mut self, p: Pos2, tout: bool, dedans: bool, hors: &str) -> Option<(Cible, Apercu)> {
        let Some(r) = self.layer().fond else { return None };
        let Some(cel) = self.cel_here() else {
            self.status = "Aplat : pas de dessin à cette image.".into();
            return None;
        };
        let portee = if tout {
            PorteeGeste::Tout
        } else if !dedans {
            self.status = hors.into();
            return None;
        } else {
            // La zone fermée par le trait, celle que le pot de peinture remplit dans l'aplat (`fondui.rs`).
            let Some(mask) = self.doc.cels.get(&cel).and_then(|c| crate::fond::zone(&c.raster, p.x as usize, p.y as usize, r.trous)) else {
                self.status = "Aplat : clique dans une zone fermée par le trait (pas sur le trait).".into();
                return None;
            };
            PorteeGeste::Zone { grown: mask.clone(), region: mask }
        };
        let apercu = self.degrade_apercu_pixels(&portee);
        Some((Cible::Pixels { dest: Dest::Aplat, portee }, apercu))
    }

    fn degrade_cible_couleur(&mut self, p: Pos2, tout: bool, dedans: bool, hors: &str) -> Option<(Cible, Apercu)> {
        let (w, h) = (self.doc.width, self.doc.height);
        let portee = if tout {
            PorteeGeste::Tout
        } else if !dedans {
            self.status = hors.into();
            return None;
        } else {
            let zone = match self.zone_couleur(p) {
                Ok(z) => z,
                Err(e) => {
                    self.status = e;
                    return None;
                }
            };
            let mut mask = vec![false; w * h];
            for y in 0..h {
                for x in 0..w {
                    mask[y * w + x] = zone.contient(x, y);
                }
            }
            PorteeGeste::Zone { grown: mask.clone(), region: mask }
        };
        let apercu = self.degrade_apercu_pixels(&portee);
        Some((Cible::Pixels { dest: Dest::Couleur, portee }, apercu))
    }

    fn degrade_cible_vecteur(&mut self, p: Pos2, tout: bool, dedans: bool, hors: &str) -> Option<(Cible, Apercu)> {
        let (w, h) = (self.doc.width, self.doc.height);
        self.flush_flottante();
        // Une sélection de zones remplies : elles reçoivent le dégradé.
        if let Some(sel) = self.vsel.as_ref().filter(|s| Some(s.cel) == self.cel_here()) {
            let cel = sel.cel;
            let items: Vec<usize> = sel
                .items
                .iter()
                .copied()
                .filter(|&i| self.drawing(cel).and_then(|d| d.elements.get(i)).is_some_and(|e| matches!(e, Element::Fill { .. })))
                .collect();
            if items.is_empty() {
                self.status = "Le dégradé remplit des zones : choisis une zone remplie, ou ôte la sélection pour remplir une zone fermée du trait.".into();
                return None;
            }
            let apercu = self.degrade_apercu_elements(cel, &items);
            return Some((Cible::Elements { cel, items }, apercu));
        }
        if tout {
            return Some((Cible::Feuille, Apercu { rect: Rect::from_min_size(Pos2::ZERO, vec2(w as f32, h as f32)), tex: None }));
        }
        if !dedans {
            self.status = hors.into();
            return None;
        }
        // Une zone déjà remplie sous le pointeur : elle reçoit le dégradé (le pot de peinture en change la couleur).
        if let Some(cel) = self.cel_here() {
            let tol = 2.0 / self.layer_zoom();
            if let Some(i) = self.drawing(cel).and_then(|d| d.hit_test(p, tol)) {
                if matches!(self.drawing(cel).map(|d| &d.elements[i]), Some(Element::Fill { .. })) {
                    let apercu = self.degrade_apercu_elements(cel, &[i]);
                    return Some((Cible::Elements { cel, items: vec![i] }, apercu));
                }
                self.status = "Le dégradé remplit des zones : appuie dans une zone fermée du trait, pas sur le trait lui-même.".into();
                return None;
            }
        }
        let (_, grown) = self.degrade_zone_du_pot(p);
        let apercu = apercu_de(&self.ctx, boite_de(&grown, w, h), &|x, y| if grown[y * w + x] { 1.0 } else { 0.0 });
        Some((Cible::Zone { grown }, apercu))
    }

    /// L'image de couverture des zones `items` du dessin vectoriel `cel` (les seules zones remplies sont rendues).
    fn degrade_apercu_elements(&self, cel: CelId, items: &[usize]) -> Apercu {
        let (w, h) = (self.doc.width, self.doc.height);
        let Some(d) = self.drawing(cel) else { return Apercu { rect: Rect::NOTHING, tex: None } };
        let mut r = Raster::new(w, h);
        let mut boite = Rect::NOTHING;
        for &i in items {
            if let Some(e) = d.elements.get(i) {
                vector::render_element(&mut r, e);
                boite = boite.union(e.bounds());
            }
        }
        if !boite.is_finite() {
            return Apercu { rect: Rect::NOTHING, tex: None };
        }
        let x0 = (boite.min.x.floor().max(0.0) as usize).min(w);
        let y0 = (boite.min.y.floor().max(0.0) as usize).min(h);
        let x1 = (boite.max.x.ceil().max(0.0) as usize).min(w);
        let y1 = (boite.max.y.ceil().max(0.0) as usize).min(h);
        if x0 >= x1 || y0 >= y1 {
            return Apercu { rect: Rect::NOTHING, tex: None };
        }
        apercu_de(&self.ctx, (x0, y0, x1 - x0, y1 - y0), &|x, y| r.get(x, y).a() as f32 / 255.0)
    }

    /// Le pointeur est en `p` : l'axe suit (Maj : par pas de 15°). Vrai si un geste de dégradé est en cours.
    pub(crate) fn degrade_drag(&mut self, p: Pos2, shift: bool) -> bool {
        let Some(g) = self.degrade_geste.as_mut() else { return false };
        g.fin = if shift {
            let d = p - g.depart;
            let angle = (d.y.atan2(d.x) / (std::f32::consts::PI / 12.0)).round() * (std::f32::consts::PI / 12.0);
            let len = d.length();
            g.depart + vec2(angle.cos(), angle.sin()) * len
        } else {
            p
        };
        true
    }

    /// Lâcher : le dégradé s'écrit (une étape d'annulation). Un simple clic ne fait rien.
    pub(crate) fn degrade_release(&mut self) {
        let Some(g) = self.degrade_geste.take() else { return };
        if g.depart.distance(g.fin) * self.layer_zoom() < AXE_MIN {
            self.status = "Glisse pour tracer l'axe du dégradé : le point d'appui a la couleur d'avant-plan, le pointeur celle d'arrière-plan.".into();
            return;
        }
        self.degrade_appliquer(g);
    }

    /// Écrit le dégradé du geste `g`.
    pub(crate) fn degrade_appliquer(&mut self, g: Geste) {
        let degrade = g.degrade();
        self.color_used(g.avant);
        if g.reglages.fin == Fin::VersFond {
            self.color_used(g.arriere);
        }
        let (w, h) = (self.doc.width, self.doc.height);
        match g.cible {
            Cible::Pixels { dest: Dest::Calque, portee } => {
                if !self.layer_editable() {
                    return;
                }
                let (cel, prefix) = self.ensure_cel();
                let keep = self.layer().keep_alpha;
                let stencil = if matches!(portee, PorteeGeste::Zone { .. }) { None } else { self.stencil_for(cel) };
                let Some(c) = self.doc.cels.get_mut(&cel) else { return };
                let tiles = degrade::peindre(&mut c.raster, &portee.vue(), &degrade, keep, stencil.as_ref());
                if tiles.is_empty() && prefix.is_none() {
                    self.status = "Rien n'a changé : le dégradé tombe hors de la zone, ou sur ce qui avait déjà ces pixels.".into();
                    return;
                }
                self.named_step("Dégradé", |app| app.push_pixels(cel, prefix, tiles));
                self.status = "Dégradé posé.".into();
            }
            Cible::Pixels { dest: Dest::Couleur, portee } => {
                if !self.layer_editable() {
                    return;
                }
                let (cel, prefix) = self.ensure_colour_cel();
                let Some(c) = self.doc.cels.get_mut(&cel) else { return };
                let tiles = degrade::peindre(&mut c.raster, &portee.vue(), &degrade, false, None);
                self.named_step("Dégradé", |app| app.push_pixels(cel, prefix, tiles));
                self.status = "Dégradé posé sur le calque de couleur, sous le trait.".into();
            }
            Cible::Pixels { dest: Dest::Aplat, portee } => {
                if !self.layer_editable() {
                    return;
                }
                let Some(cel) = self.cel_here() else { return };
                let Some(c) = self.doc.cels.get_mut(&cel) else { return };
                let s = c.fond.get_or_insert_with(|| Box::new(crate::document::Strate::new(w, h)));
                let tiles = degrade::peindre(&mut s.peinture.raster, &portee.vue(), &degrade, false, None);
                let mut edits = vec![crate::history::Edit::FondTiles { cel, tiles, coupe: false }];
                // La zone recoud l'aplat s'il était coupé là (comme le pot de peinture).
                if let PorteeGeste::Zone { region, .. } = &portee {
                    let old = crate::fond::paint_mask(&mut s.coupe, region, None);
                    if !old.is_empty() {
                        edits.push(crate::history::Edit::FondTiles { cel, tiles: old, coupe: true });
                    }
                }
                s.peinture.version += 1;
                c.version += 1;
                let avant = self.history.context.replace("Aplat : dégradé".to_string());
                self.history.push(crate::history::Edit::Group(edits));
                self.history.context = avant;
                self.modified = true;
                self.renderer.invalidate_all();
                self.status = "Aplat : dégradé posé, sous le trait.".into();
            }
            Cible::Elements { cel, items } => {
                let Some(d) = self.drawing_mut(cel) else { return };
                let before = d.clone();
                let mut region = Rect::NOTHING;
                for &i in &items {
                    if let Some(Element::Fill { color, opacity, gradient, .. }) = d.elements.get_mut(i) {
                        *gradient = Some(degrade);
                        *color = degrade.c0;
                        *opacity = 1.0;
                    }
                    if let Some(e) = d.elements.get(i) {
                        region = region.union(e.bounds());
                    }
                }
                self.refresh_cel_region(cel, region);
                self.named_step("Dégradé", |app| app.push_vector(cel, before, None));
                self.status = "Dégradé posé sur la zone (il reste modifiable : l'outil Sélection la déplace avec son axe).".into();
            }
            Cible::Zone { grown } => {
                let contours = vector::trace_contours(&grown, w, h);
                self.degrade_nouvelle_zone(contours, degrade);
            }
            Cible::Feuille => {
                let (fw, fh) = (w as f32, h as f32);
                self.degrade_nouvelle_zone(vec![vec![[0.0, 0.0], [fw, 0.0], [fw, fh], [0.0, fh]]], degrade);
            }
        }
    }

    /// Une zone vectorielle neuve en dégradé, sous les traits (comme le pot de peinture).
    fn degrade_nouvelle_zone(&mut self, contours: Vec<Vec<[f32; 2]>>, degrade: Degrade) {
        if contours.is_empty() || !self.layer_editable() {
            return;
        }
        let (cel, prefix) = self.ensure_cel();
        let Some(d) = self.drawing_mut(cel) else { return };
        let before = d.clone();
        let element = Element::Fill { contours, color: degrade.c0, opacity: 1.0, gradient: Some(degrade) };
        let region = element.bounds();
        let at = d.fill_insert_index();
        d.elements.insert(at, element);
        // L'insertion décale les éléments : une sélection garderait de mauvais indices.
        self.vsel = None;
        self.refresh_cel_region(cel, region);
        self.named_step("Dégradé", |app| app.push_vector(cel, before, prefix));
        self.status = "Dégradé posé : une zone vectorielle, sous les traits.".into();
    }

    // ------------------------------------------------------------------ l'aperçu

    /// L'aperçu du geste en cours, par-dessus le dessin : le dégradé sur la zone, et l'axe.
    pub(crate) fn paint_degrade_overlay(&self, painter: &egui::Painter) {
        let Some(g) = &self.degrade_geste else { return };
        let degrade = g.degrade();
        let r = g.apercu.rect;
        if r.is_finite() && r.width() > 0.0 && r.height() > 0.0 {
            let (nx, ny) = (GRILLE, GRILLE);
            let couleurs = degrade::sommets(&degrade, r.min, r.max, nx, ny, 1.0);
            let mut mesh = match &g.apercu.tex {
                Some(t) => egui::Mesh::with_texture(t.id()),
                None => egui::Mesh::default(),
            };
            for j in 0..=ny {
                for i in 0..=nx {
                    let (u, v) = (i as f32 / nx as f32, j as f32 / ny as f32);
                    let p = pos2(r.min.x + r.width() * u, r.min.y + r.height() * v);
                    let uv = if g.apercu.tex.is_some() { pos2(u, v) } else { egui::epaint::WHITE_UV };
                    mesh.vertices.push(egui::epaint::Vertex { pos: self.layer_to_screen(p), uv, color: couleurs[j * (nx + 1) + i] });
                }
            }
            for j in 0..ny {
                for i in 0..nx {
                    let a = (j * (nx + 1) + i) as u32;
                    let (b, c, d) = (a + 1, a + nx as u32 + 1, a + nx as u32 + 2);
                    mesh.add_triangle(a, b, d);
                    mesh.add_triangle(a, d, c);
                }
            }
            painter.add(Shape::mesh(mesh));
        }
        // L'axe : un trait avec un rond à chaque bout (le rond de départ plein, celui d'arrivée vide) ; en radial, le cercle d'arrivée.
        let (a, b) = (self.layer_to_screen(g.depart), self.layer_to_screen(g.fin));
        let ombre = Stroke::new(3.0_f32, Color32::from_black_alpha(150));
        let trait_ = Stroke::new(1.5_f32, Color32::WHITE);
        if g.reglages.genre == Genre::Radial {
            let rayon = a.distance(b);
            painter.circle_stroke(a, rayon, ombre);
            painter.circle_stroke(a, rayon, Stroke::new(1.0_f32, Color32::from_white_alpha(200)));
        }
        painter.line_segment([a, b], ombre);
        painter.line_segment([a, b], trait_);
        painter.circle_filled(a, 4.5, Color32::WHITE);
        painter.circle_stroke(a, 4.5, Stroke::new(1.0_f32, Color32::BLACK));
        painter.circle_stroke(b, 4.5, Stroke::new(1.5_f32, Color32::BLACK));
        painter.circle_stroke(b, 3.5, Stroke::new(1.5_f32, Color32::WHITE));
    }

    // ------------------------------------------------------------------ Réglages

    /// Les réglages de l'outil : la forme, la fin, l'opacité, inverser, tout le calque, et une bande qui montre le dégradé.
    pub(crate) fn degrade_options_ui(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::{bulle, choix};
        use egui_phosphor::regular as ph;
        ui.horizontal(|ui| {
            ui.label("Type");
            choix(ui, &mut self.degrade.genre, Genre::Lineaire, ph::GRADIENT, "Linéaire", "La couleur change le long de l'axe tracé.", "");
            choix(ui, &mut self.degrade.genre, Genre::Radial, ph::TARGET, "Radial", "Le point d'appui est le centre, le pointeur est sur le cercle d'arrivée.", "");
        });
        ui.vertical(|ui| {
            let resp = ui.radio_value(&mut self.degrade.fin, Fin::VersFond, "Vers la couleur de fond");
            bulle(resp, "Vers la couleur de fond", "Du carré d'avant-plan au carré d'arrière-plan du panneau Couleur.", "");
            let resp = ui.radio_value(&mut self.degrade.fin, Fin::VersTransparent, "Vers le transparent");
            bulle(resp, "Vers le transparent", "La couleur d'avant-plan, de plus en plus transparente.", "");
        });
        let mut pct = (self.degrade.opacite * 100.0).round();
        if ui.add(egui::Slider::new(&mut pct, 0.0..=100.0).text("Opacité").suffix(" %").fixed_decimals(0)).changed() {
            self.degrade.opacite = (pct / 100.0).clamp(0.0, 1.0);
        }
        ui.horizontal(|ui| {
            let resp = ui.checkbox(&mut self.degrade.inverse, "Inverser");
            bulle(resp, "Inverser", "Échanger les deux bouts : le point d'appui prend la couleur d'arrivée.", "");
            let resp = ui.checkbox(&mut self.degrade.tout_le_calque, "Tout le calque");
            bulle(resp, "Tout le calque", "Remplir toute la feuille au lieu de la sélection ou de la zone fermée (Alt tenu : la même chose, le temps d'un geste).", "");
        });
        // La bande : le dégradé tel que ces réglages le donnent, sur un fond gris (le transparent se voit).
        let largeur = ui.available_width().clamp(60.0, 240.0);
        let (rect, _) = ui.allocate_exact_size(vec2(largeur, 14.0), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 2.0, Color32::from_gray(110));
        let apercu = self.degrade.degrade(self.color, self.couleurs.prefs.back, pos2(0.0, 0.0), pos2(1.0, 0.0));
        let n = 24;
        let couleurs = degrade::sommets(&apercu, pos2(0.0, 0.0), pos2(1.0, 0.0), n, 1, 1.0);
        let mut mesh = egui::Mesh::default();
        for j in 0..2usize {
            for i in 0..=n {
                let pos = pos2(rect.left() + rect.width() * i as f32 / n as f32, if j == 0 { rect.top() } else { rect.bottom() });
                mesh.vertices.push(egui::epaint::Vertex { pos, uv: egui::epaint::WHITE_UV, color: couleurs[i] });
            }
        }
        for i in 0..n {
            let a = i as u32;
            let (b, c, d) = (a + 1, a + n as u32 + 1, a + n as u32 + 2);
            mesh.add_triangle(a, b, d);
            mesh.add_triangle(a, d, c);
        }
        painter.add(Shape::mesh(mesh));
        painter.rect_stroke(rect, 2.0, Stroke::new(1.0_f32, crate::theme::bord()), egui::StrokeKind::Inside);
        ui.label(egui::RichText::new("Glisser : tracer l'axe (Maj : par 15°). Dans la sélection, sinon dans la zone fermée où l'on appuie ; Alt : tout le calque.").small().weak());
    }
}
