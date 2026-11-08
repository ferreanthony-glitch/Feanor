//! La sélection de la planche sur un calque **bitmap** (partie 127) : comme la partie 125 pour le vectoriel, les bords des cases ne coupent jamais
//! le dessin.
//!
//! # Ce que c'est
//!
//! - **Ctrl + glisser** trace un lasso sur la planche (dans une case, sur le papier, à cheval). **Ce que le lasso entoure est pris, au pixel**, dans le
//!   dessin de chaque case qu'il couvre (là où la case se voit : son image, ou sa marge hors de l'image d'une autre case) et sur la couche bitmap de
//!   la planche (`planchebitmap.rs`). **Un trait réparti d'un même geste est pris entier dès que le lasso en prend un pixel** : ses pixels dans les
//!   autres cases (ses empreintes) et ses morceaux de planche, même hors du lasso, même dans sa propre case hors du lasso (le bord d'une case ne le
//!   coupe jamais ; le lasso, lui, ne coupe que ce qui n'est pas un trait réparti). **Choix fait pour Anthony, à valider** (partie 127).
//! - **Ctrl + clic** : le trait réparti sous le pointeur, entier ; sinon rien.
//! - **Les gestes** : la boîte du choix (`selbox.rs`, les mêmes poignées que le vectoriel) : déplacer, mettre à l'échelle, tourner, incliner. Pendant le
//!   geste, rien ne change dans le projet : les pixels pris sont peints à leur nouvelle place, par blocs déjà faits (aucun rejeu, aucun calcul de pixels
//!   sur le fil de l'interface hors du budget d'envoi des textures). **Au lâcher**, tout est calculé avant de rien écrire ; puis les pixels quittent
//!   leurs dessins et se reposent, transformés (`Raster::transformee`, ou recopiés tels quels pour un déplacement d'un nombre entier de pixels), dans
//!   le dessin de la case qui les reçoit, et sur la planche hors de toute case qui puisse les recevoir (les refus de la partie 122 : par-dessus
//!   l'image de la case). Une seule étape d'annulation, même historique plein.
//! - **Suppr, Ctrl+C, Ctrl+X, Ctrl+V** : le choix entier ; Ctrl+V le repose à sa place, réparti comme un trait qu'on vient de tracer.
//! - **La limite** (`LIMITE_TUILES`) : au-delà, la sélection reste celle de la case où le lasso commence (sans suivre les traits hors d'elle), et la
//!   ligne d'état dit pourquoi. Rien n'est coupé en silence.

use std::collections::BTreeMap;
use std::sync::Arc;

use eframe::egui::{self, pos2, Color32, Modifiers, Pos2, Rect, Stroke};

use crate::app::{App, Tool};
use crate::board::BoardLayout;
use crate::boardui::Xf;
use crate::document::{CelId, LayerKind};
use crate::history::Edit;
use crate::planchebitmap::{pixels_vides, Masque, A64};
use crate::raster::{Raster, Tile, TILE, TILE_PIXELS};
use crate::selbox::{self, Grabs, Handle, Mods, SelFrame};
use crate::vector::Affine;
use crate::vignettesel::GesteSel;

/// La limite d'une sélection, en tuiles de 64 × 64 pixels prises (une tuile entamée compte entière) : 7 680 tuiles, **31,5 millions de pixels**, 126 Mo de
/// pixels pris (mesures et raison : partie 127, `vignettes_bitmap_tests::measure_the_memory_of_a_huge_selection`).
pub(crate) const LIMITE_TUILES: usize = 7_680;
const CLIC_PX: f32 = 4.0;
const PRISE_PX: f32 = 6.0;
const PIVOT_SNAP: f32 = 6.0;
const LASSO_MAX: usize = 4000;

/// D'où viennent des pixels pris.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Source {
    /// Le dessin `cel`, montré par la case `case` (`CardPlace::id`).
    Dessin { cel: CelId, case: u64 },
    /// Un morceau de la couche bitmap de la planche (`Morceau::id`).
    Morceau { id: u64 },
}

/// Des pixels pris, dans la grille de leur source, et comment ils se posent sur la planche.
#[derive(Clone)]
pub(crate) struct Piece {
    pub source: Source,
    pub pixels: Arc<Raster>,
    pub vers_planche: A64,
    /// Un numéro unique (les textures de l'aperçu).
    pub cle: u64,
}

impl std::fmt::Debug for Piece {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Piece").field("source", &self.source).field("tuiles", &crate::planchebitmap::tuiles_de(&self.pixels)).finish()
    }
}

/// La sélection bitmap de la planche.
#[derive(Debug)]
pub(crate) struct SelBitmap {
    pub pieces: Vec<Piece>,
    /// Les gestes pris entiers.
    pub liens: Vec<u64>,
    pub serial: u64,
    pub lasso: Option<Vec<Pos2>>,
    pub geste: Option<GesteSel>,
    pub pivot: Option<Pos2>,
    pub bornes: Rect,
    /// Après un lâcher : les pixels peints à leur nouvelle place tant que les vignettes ne sont pas refaites (étapes, début).
    pub tenu: Option<(Vec<Piece>, usize, std::time::Instant)>,
    pub limite_atteinte: bool,
}

impl Default for SelBitmap {
    fn default() -> Self {
        SelBitmap { pieces: Vec::new(), liens: Vec::new(), serial: 0, lasso: None, geste: None, pivot: None, bornes: Rect::NOTHING, tenu: None, limite_atteinte: false }
    }
}

/// Ce que prendra un lâcher, calculé sans rien écrire.
struct Plan {
    /// Par pièce et dessin d'arrivée : (case, calque, pixels à poser, dans la grille du dessin).
    dessins: Vec<(usize, usize, Raster)>,
    /// Les morceaux à poser : (case qu'ils recouvrent, pixels, pose).
    morceaux: Vec<(Option<usize>, Raster, A64)>,
}

fn la_piece(source: &Raster, pris: &Masque) -> Raster {
    // (La même feuille que la source : ses tuiles se retrouvent par leur rang, sans recherche, au rééchantillonnage.)
    let mut out = Raster::new(source.width, source.height);
    for (&(tx, ty), bits) in &pris.tuiles {
        let Some(t) = source.tile_at(tx, ty) else { continue };
        let px: &[Color32; TILE_PIXELS] = t;
        let entiere = px.iter().enumerate().all(|(k, c)| c.a() == 0 || bits[k / TILE] >> (k % TILE) & 1 == 1);
        let id = out.id_of(tx, ty);
        if entiere {
            // (Toute l'encre de la tuile est prise : la même mémoire, rien de recopié.)
            out.replace_slot(id, Some(t.clone()));
            continue;
        }
        let mut neuve = [Color32::TRANSPARENT; TILE_PIXELS];
        let mut une = false;
        for (k, c) in px.iter().enumerate() {
            if c.a() != 0 && bits[k / TILE] >> (k % TILE) & 1 == 1 {
                neuve[k] = *c;
                une = true;
            }
        }
        if une {
            out.replace_slot(id, Some(Tile::new(neuve)));
        }
    }
    out
}

/// Les pixels (centres) de la grille d'où `a` mène sur la planche, qui tombent dans le polygone `poly` (planche), rangée par rangée : (x0, x1) bornes exclues.
fn spans(poly_px: &[Pos2], y: i64) -> Vec<(i64, i64)> {
    let yc = y as f32 + 0.5;
    let mut xs: Vec<f32> = Vec::new();
    for i in 0..poly_px.len() {
        let (p, q) = (poly_px[i], poly_px[(i + 1) % poly_px.len()]);
        if (p.y > yc) != (q.y > yc) {
            xs.push(p.x + (yc - p.y) * (q.x - p.x) / (q.y - p.y));
        }
    }
    xs.sort_by(f32::total_cmp);
    xs.chunks_exact(2).map(|w| ((w[0] - 0.5).ceil() as i64, (w[1] - 0.5).ceil() as i64)).filter(|(a, b)| b > a).collect()
}

impl App {
    /// La sélection bitmap de la planche est-elle possible ici : la planche des Vignettes, un calque bitmap.
    pub(crate) fn selb_possible(&self) -> bool {
        self.board_shown() && self.layers().get(self.cur_layer).is_some_and(|l| l.kind == LayerKind::Bitmap)
    }

    pub(crate) fn selb_active(&self) -> bool {
        self.selb_possible() && self.vignettes.selb.as_ref().is_some_and(|s| !s.pieces.is_empty() && s.serial == self.history.serial())
    }

    pub(crate) fn selb_a_la_main(&self) -> bool {
        self.selb_active() && !self.panels_have_focus() && !self.images_focus && !self.key_focus
    }

    /// Les cases dont le dessin bitmap se choisit et reçoit : (case, calque, comment son dessin se pose sur la planche). Une case en perspective n'y est pas.
    fn selb_cases(&self, layout: &BoardLayout) -> Vec<Option<(usize, A64)>> {
        (0..layout.cards.len())
            .map(|k| {
                let rang = self.case_recoit(layout, k, LayerKind::Bitmap).ok()?;
                let a = crate::horschamp::pose_de_la_case(&self.doc, layout, k)?.affine64()?;
                Some((rang, a))
            })
            .collect()
    }

    fn cel_de(&self, layout: &BoardLayout, k: usize, rang: usize) -> Option<CelId> {
        let c = layout.cards.get(k)?;
        self.doc.panels.get(c.panel)?.layers.get(rang)?.cel_at(c.frame)
    }

    /// Ce que prend le lasso `poly` (planche) ; `seule` : seulement dans la case `seule`, sans suivre les traits (la limite). Rend les pièces et les gestes
    /// pris entiers, ou rien si la limite est dépassée.
    fn selb_prendre(&self, layout: &BoardLayout, poly: &[Pos2], seule: Option<usize>, clic: Option<(Pos2, f32)>) -> Option<(Vec<Piece>, Vec<u64>)> {
        self.selb_prendre_jusqu_a(layout, poly, seule, clic, LIMITE_TUILES)
    }

    /// Essais : ce que prendrait le lasso `poly` sans limite (la mesure de la limite).
    #[cfg(test)]
    pub(crate) fn selb_prendre_sans_limite(&self, layout: &BoardLayout, poly: &[Pos2]) -> Vec<Piece> {
        self.selb_prendre_jusqu_a(layout, poly, None, None, usize::MAX).map(|t| t.0).unwrap_or_default()
    }

    /// Essais : le lâcher d'un déplacement `g` des pièces `pieces`, comme au vrai geste.
    #[cfg(test)]
    pub(crate) fn selb_mesurer_un_lacher(&mut self, layout: &BoardLayout, pieces: Vec<Piece>, g: &A64) {
        let serial = self.history.serial();
        self.vignettes.selb = Some(SelBitmap { pieces, serial, ..SelBitmap::default() });
        self.selb_reporter(layout, g, "Déplacer", true);
    }

    /// Essais : le geste `lien` se prend-il entier (sous la limite) ?
    #[cfg(test)]
    pub(crate) fn selb_du_lien_pour_essai(&self, layout: &BoardLayout, lien: u64) -> bool {
        self.selb_prendre_liens(layout, &[lien]).is_some()
    }

    fn selb_prendre_jusqu_a(&self, layout: &BoardLayout, poly: &[Pos2], seule: Option<usize>, clic: Option<(Pos2, f32)>, limite: usize) -> Option<(Vec<Piece>, Vec<u64>)> {
        let champs = crate::horschamp::champs(layout, self.settings.vignettes.debord_fraction());
        let cases = self.selb_cases(layout);
        let boite = if poly.len() >= 3 { Rect::from_points(poly) } else { Rect::NOTHING };
        let mut pris: Vec<(Source, Masque)> = Vec::new();
        let mut liens: Vec<u64> = Vec::new();
        let bm = &self.doc.annotations.bitmap;
        let montre = self.doc.annotations.hors_cadres_montre().is_some();
        let ajouter = |pris: &mut Vec<(Source, Masque)>, s: Source, x: i64, y: i64| match pris.iter_mut().find(|(t, _)| *t == s) {
            Some((_, m)) => m.poser(x, y),
            None => {
                let mut m = Masque::default();
                m.poser(x, y);
                pris.push((s, m));
            }
        };
        let mut vus: Vec<CelId> = Vec::new();
        // 1. Les dessins des cases, là où la case se voit.
        for (k, c) in cases.iter().enumerate() {
            let Some((rang, a)) = *c else { continue };
            if seule.is_some_and(|s| s != k) {
                continue;
            }
            let Some(cel) = self.cel_de(layout, k, rang) else { continue };
            if vus.contains(&cel) {
                continue;
            }
            vus.push(cel);
            let Some(champ) = champs.get(k) else { continue };
            let Some(d) = self.doc.cels.get(&cel) else { continue };
            let Some(inv) = a.inverse() else { continue };
            let source = Source::Dessin { cel, case: layout.cards[k].id };
            if let Some((p, tol)) = clic {
                if !champ.contains(p) {
                    continue;
                }
                let q = inv.point(p);
                let r = (tol as f64 / a.echelle().max(1e-9)).ceil() as i64;
                for dy in -r..=r {
                    for dx in -r..=r {
                        let (x, y) = (q.x.floor() as i64 + dx, q.y.floor() as i64 + dy);
                        if d.raster.get_i(x, y).a() > 0 {
                            for e in bm.empreintes.iter().filter(|e| e.cel == cel && e.masque.contient(x, y)) {
                                if !liens.contains(&e.lien) {
                                    liens.push(e.lien);
                                }
                            }
                        }
                    }
                }
                continue;
            }
            if !champ.intersects(boite) {
                continue;
            }
            let poly_px: Vec<Pos2> = poly.iter().map(|p| inv.point(*p)).collect();
            // (Seulement là où la case se voit : son champ, en pixels de son dessin.)
            let champ_px = Rect::from_points(&[champ.left_top(), champ.right_top(), champ.right_bottom(), champ.left_bottom()].map(|p| inv.point(p)));
            let b = Rect::from_points(&poly_px).intersect(champ_px);
            if !b.is_positive() {
                continue;
            }
            // L'image de la case en pixels (là, tout ce qu'on voit est à elle), et les cases voisines dont l'image peut cacher sa marge.
            let mouvement = layout.move_of(k).is_some();
            let t = &layout.cards[k].thumb;
            let image_px = Rect::from_points(&[t.left_top(), t.right_bottom()].map(|p| inv.point(p)));
            let voisines: Vec<usize> = (0..layout.cards.len()).filter(|&j| j != k && (layout.cards[j].thumb.intersects(*champ) || layout.move_of(j).is_some())).collect();
            let vu = |x: i64, y: i64| -> bool {
                let c = pos2(x as f32 + 0.5, y as f32 + 0.5);
                if !mouvement && image_px.contains(c) {
                    return true;
                }
                let p = a.point(c);
                crate::horschamp::dans_image(layout, k, p) || (champ.contains(p) && !voisines.iter().any(|&j| crate::horschamp::dans_image(layout, j, p)) && layout.annulee_en(p).is_none())
            };
            let i = match pris.iter().position(|(s, _)| *s == source) {
                Some(i) => i,
                None => {
                    pris.push((source, Masque::default()));
                    pris.len() - 1
                }
            };
            let tl = TILE as i64;
            for y in b.top().floor() as i64..=b.bottom().ceil() as i64 {
                for (x0, x1) in spans(&poly_px, y) {
                    let (x0, x1) = (x0.max(b.left().floor() as i64), x1.min(b.right().ceil() as i64));
                    let mut x = x0;
                    while x < x1 {
                        // Tuile par tuile : une seule recherche de tuile et de mot du masque par morceau de rangée.
                        let tx = x.div_euclid(tl);
                        let fin = ((tx + 1) * tl).min(x1);
                        if let Some(tuile) = d.raster.tile_at(tx as i32, y.div_euclid(tl) as i32) {
                            let px: &[Color32; TILE_PIXELS] = tuile;
                            let ligne = (y.rem_euclid(tl) * tl) as usize;
                            let mut bits = 0u64;
                            for xx in x..fin {
                                let k2 = (xx - tx * tl) as usize;
                                if px[ligne + k2].a() != 0 && vu(xx, y) {
                                    bits |= 1u64 << k2;
                                }
                            }
                            if bits != 0 {
                                *pris[i].1.mot(tx as i32, y.div_euclid(tl) as i32, y.rem_euclid(tl) as usize) |= bits;
                            }
                        }
                        x = fin;
                    }
                }
            }
        }
        // 2. La couche bitmap de la planche (montrée), là où elle se voit : par-dessus une case, ou sur le papier hors des images.
        if montre && seule.is_none() {
            for m in &bm.morceaux {
                let Some(v) = m.vers_planche(layout) else { continue };
                let Some(bornes) = m.bornes(layout) else { continue };
                let Some(inv) = v.inverse() else { continue };
                let visible = |p: Pos2| m.dessus || (layout.card_at(p).is_none() && layout.annulee_en(p).is_none());
                if let Some((p, tol)) = clic {
                    if m.touche(layout, p, tol) && visible(p) {
                        if let Some(l) = m.lien.filter(|l| !liens.contains(l)) {
                            liens.push(l);
                        }
                    }
                    continue;
                }
                if !bornes.intersects(boite) {
                    continue;
                }
                let poly_px: Vec<Pos2> = poly.iter().map(|p| inv.point(*p)).collect();
                let b = Rect::from_points(&poly_px);
                for y in b.top().floor() as i64..=b.bottom().ceil() as i64 {
                    for (x0, x1) in spans(&poly_px, y) {
                        for x in x0..x1 {
                            if m.pixels.get_i(x, y).a() == 0 {
                                continue;
                            }
                            if visible(v.point(pos2(x as f32 + 0.5, y as f32 + 0.5))) {
                                ajouter(&mut pris, Source::Morceau { id: m.id }, x, y);
                            }
                        }
                    }
                }
            }
        }
        // 3. Les gestes touchés, pris entiers : leurs empreintes dans les dessins des cases, et leurs morceaux.
        if seule.is_none() {
            for (s, m) in &pris {
                match *s {
                    Source::Dessin { cel, .. } => {
                        for e in bm.empreintes.iter().filter(|e| e.cel == cel) {
                            if liens.contains(&e.lien) {
                                continue;
                            }
                            let mut touche = false;
                            m.chaque(|x, y| touche |= e.masque.contient(x, y));
                            if touche {
                                liens.push(e.lien);
                            }
                        }
                    }
                    Source::Morceau { id } => {
                        if let Some(l) = bm.morceaux.iter().find(|q| q.id == id).and_then(|q| q.lien).filter(|l| !liens.contains(l)) {
                            liens.push(l);
                        }
                    }
                }
            }
            for &lien in &liens {
                for e in bm.empreintes.iter().filter(|e| e.lien == lien) {
                    // (Seulement là où la case le montre et peut le reprendre : un dessin d'une autre image, un calque verrouillé gardent leur part.)
                    let Some(k) = (0..layout.cards.len()).find(|&k| cases[k].is_some_and(|(rang, _)| self.cel_de(layout, k, rang) == Some(e.cel))) else { continue };
                    let Some(d) = self.doc.cels.get(&e.cel) else { continue };
                    let source = Source::Dessin { cel: e.cel, case: layout.cards[k].id };
                    e.masque.chaque(|x, y| {
                        if d.raster.get_i(x, y).a() > 0 {
                            ajouter(&mut pris, source, x, y);
                        }
                    });
                }
                if montre {
                    for m in bm.morceaux.iter().filter(|m| m.lien == Some(lien)) {
                        crate::planchebitmap::chaque_pixel(&m.pixels, |x, y, _| ajouter(&mut pris, Source::Morceau { id: m.id }, x, y));
                    }
                }
            }
        }
        // La limite : avant de recopier le moindre pixel.
        let tuiles: usize = pris.iter().map(|(_, m)| m.tuiles.len()).sum();
        if tuiles > limite {
            return None;
        }
        let mut pieces = Vec::new();
        for (s, m) in pris {
            let (src, v): (&Raster, A64) = match s {
                Source::Dessin { cel, case } => {
                    let Some(k) = layout.cards.iter().position(|c| c.id == case) else { continue };
                    let Some((_, a)) = cases[k] else { continue };
                    let Some(d) = self.doc.cels.get(&cel) else { continue };
                    (&d.raster, a)
                }
                Source::Morceau { id } => {
                    let Some(q) = bm.morceaux.iter().find(|q| q.id == id) else { continue };
                    let Some(v) = q.vers_planche(layout) else { continue };
                    (&q.pixels, v)
                }
            };
            let px = la_piece(src, &m);
            if !px.is_blank() {
                pieces.push(Piece { source: s, pixels: Arc::new(px), vers_planche: v, cle: crate::horschamp::nouveau_lien() });
            }
        }
        Some((pieces, liens))
    }

    /// La boîte des pièces sur la planche.
    fn bornes_des(pieces: &[Piece]) -> Rect {
        let mut r = Rect::NOTHING;
        for p in pieces {
            let Some((x0, y0, x1, y1)) = crate::planchebitmap::boite_px(&p.pixels) else { continue };
            // (Au pixel près : la boîte de l'encre, pas celle des tuiles.)
            let mut b = (i64::MAX, i64::MAX, i64::MIN, i64::MIN);
            crate::planchebitmap::chaque_pixel(&p.pixels, |x, y, _| b = (b.0.min(x), b.1.min(y), b.2.max(x + 1), b.3.max(y + 1)));
            let (x0, y0, x1, y1) = if b.0 <= b.2 { b } else { (x0, y0, x1, y1) };
            for (x, y) in [(x0, y0), (x1, y0), (x1, y1), (x0, y1)] {
                let q = p.vers_planche.point(pos2(x as f32, y as f32));
                r = r.union(Rect::from_min_max(q, q));
            }
        }
        r
    }

    fn selb_cadre(&self, zoom: f32) -> Option<SelFrame> {
        let s = self.vignettes.selb.as_ref()?;
        (s.bornes.is_finite() && s.bornes.width() >= 0.0).then(|| SelFrame { rect0: s.bornes.expand(2.0 / zoom.max(0.05)), m: Affine::IDENTITY, pivot: s.pivot })
    }

    /// Un appui avec Ctrl sur un calque bitmap des Vignettes : le geste de la boîte, ou un lasso. Vrai si l'appui est pris.
    pub(crate) fn selb_appui(&mut self, xf: Xf, pos: Pos2, m: Modifiers) -> bool {
        let ctrl = (m.ctrl || m.command) && !m.shift && !m.alt;
        if !ctrl || !self.selb_possible() || self.tool == Tool::Camera || self.vignettes.camera_tenue {
            self.vignettes.selb = None;
            return false;
        }
        let board = xf.to_board(pos);
        if self.selb_active() {
            if let Some(frame) = self.selb_cadre(xf.zoom) {
                let quad = frame.corners().map(|q| xf.to_screen(q));
                let pivot = xf.to_screen(frame.pivot_world());
                if let Some(h) = selbox::hit_test(quad, pivot, pos, &Grabs::default()) {
                    if let Some(s) = self.vignettes.selb.as_mut() {
                        s.geste = Some(GesteSel { handle: h, start: board, frame, aff: Affine::IDENTITY, change: false });
                    }
                    return true;
                }
            }
        }
        let tenu = self.vignettes.selb.as_mut().and_then(|s| s.tenu.take());
        self.vignettes.selb = Some(SelBitmap { lasso: Some(vec![board]), tenu, ..SelBitmap::default() });
        self.vsel = None;
        true
    }

    /// Le geste à cette image d'écran (le lasso grandit, la boîte suit) ; au lâcher, il est posé. Échap abandonne.
    pub(crate) fn selb_geste(&mut self, layout: &BoardLayout, xf: Xf, latest: Option<Pos2>, down: bool, escape: bool, dehors: bool) {
        if self.vignettes.selb.is_some() && (dehors || !self.selb_possible()) {
            self.vignettes.selb = None;
            return;
        }
        let Some(s) = self.vignettes.selb.as_mut() else { return };
        if escape {
            if s.lasso.is_some() || s.geste.is_some() {
                s.lasso = None;
                s.geste = None;
                self.status = "Geste abandonné : rien n'a bougé.".into();
            } else {
                self.vignettes.selb = None;
            }
            return;
        }
        if let Some(g) = s.geste.as_mut() {
            if let Some(p) = latest {
                let board = xf.to_board(p);
                if g.handle == Handle::Pivot {
                    s.pivot = selbox::move_pivot(&g.frame, g.start, board, PIVOT_SNAP, xf.zoom);
                } else {
                    let m = Mods::with(false, false, self.keep_proportions, self.settings.point_fixe, self.settings.rotation_autour);
                    g.aff = selbox::gesture(g.handle, &g.frame, g.start, board, m);
                    g.change = g.change || g.aff != Affine::IDENTITY;
                    self.status = crate::seltools::gesture_text(g.handle, &g.frame.then(&g.aff).params());
                }
            }
            if !down {
                let g = s.geste.take().expect("le geste");
                if g.handle != Handle::Pivot && g.change && g.aff != Affine::IDENTITY {
                    self.selb_reporter(layout, &A64::de(&g.aff), g.handle.step_name(), true);
                }
            }
            return;
        }
        if let Some(l) = s.lasso.as_mut() {
            if let Some(p) = latest {
                let b = xf.to_board(p);
                let eps = 2.0 / xf.zoom.max(0.05);
                if l.last().is_none_or(|q| (*q - b).length() >= eps) && l.len() < LASSO_MAX {
                    l.push(b);
                }
            }
            if !down {
                let l = s.lasso.take().unwrap_or_default();
                let zoom = xf.zoom.max(0.05);
                let etendue = Rect::from_points(&l).size().max_elem() * zoom;
                let clic = l.len() < 3 || etendue < CLIC_PX;
                let mut limite = false;
                let trouve = if clic {
                    self.selb_prendre(layout, &[], None, Some((l[0], PRISE_PX / zoom))).and_then(|(_, liens)| {
                        // (Un clic : les traits répartis sous le pointeur, entiers.)
                        if liens.is_empty() {
                            return Some((Vec::new(), liens));
                        }
                        self.selb_du_lien(layout, &liens)
                    })
                } else {
                    match self.selb_prendre(layout, &l, None, None) {
                        Some(t) => Some(t),
                        None => {
                            // Au-delà de la limite : la sélection reste celle de la case où le lasso commence.
                            limite = true;
                            let d = |r: Rect, p: Pos2| ((r.left() - p.x).max(p.x - r.right()).max(0.0)).hypot((r.top() - p.y).max(p.y - r.bottom()).max(0.0));
                            let k = layout.card_at(l[0]).or_else(|| (0..layout.cards.len()).min_by(|a, b| d(layout.cards[*a].thumb, l[0]).total_cmp(&d(layout.cards[*b].thumb, l[0]))));
                            k.and_then(|k| self.selb_prendre(layout, &l, Some(k), None))
                        }
                    }
                };
                let (pieces, liens) = trouve.unwrap_or_default();
                let serial = self.history.serial();
                let bornes = Self::bornes_des(&pieces);
                let n: usize = pieces.iter().map(|p| crate::planchebitmap::tuiles_de(&p.pixels)).sum();
                let s = self.vignettes.selb.get_or_insert_with(SelBitmap::default);
                s.pieces = pieces;
                s.liens = liens;
                s.serial = serial;
                s.pivot = None;
                s.bornes = bornes;
                s.limite_atteinte = limite;
                let cases = s.pieces.iter().filter(|p| matches!(p.source, Source::Dessin { .. })).count();
                let planche = s.pieces.len() - cases;
                let entiers = s.liens.len();
                self.status = if s.pieces.is_empty() {
                    if limite { "Sélection trop grande pour suivre tout le trait, et rien dans cette case.".into() } else { "Rien dans le lasso.".into() }
                } else if limite {
                    "Sélection trop grande pour suivre tout le trait : seule la part de cette case est prise.".into()
                } else {
                    let ent = match entiers {
                        0 => String::new(),
                        1 => ", un trait entier".into(),
                        e => format!(", {e} traits entiers"),
                    };
                    let _ = n;
                    format!("Pixels choisis dans {cases} dessin{}{}{ent} : Ctrl + glisser dans la boîte les déplace, un coin agrandit ou tourne ; Suppr efface.", if cases > 1 { "s" } else { "" }, if planche > 0 { " et sur la planche" } else { "" })
                };
            }
        }
    }

    /// Les gestes `liens`, entiers (leurs empreintes et leurs morceaux). Rien au-delà de la limite.
    fn selb_du_lien(&self, layout: &BoardLayout, liens: &[u64]) -> Option<(Vec<Piece>, Vec<u64>)> {
        self.selb_prendre_liens(layout, liens)
    }

    fn selb_prendre_liens(&self, layout: &BoardLayout, liens: &[u64]) -> Option<(Vec<Piece>, Vec<u64>)> {
        let cases = self.selb_cases(layout);
        let bm = &self.doc.annotations.bitmap;
        let mut pieces = Vec::new();
        let mut tuiles = 0;
        for &lien in liens {
            for e in bm.empreintes.iter().filter(|e| e.lien == lien) {
                let Some(k) = (0..layout.cards.len()).find(|&k| cases[k].is_some_and(|(rang, _)| self.cel_de(layout, k, rang) == Some(e.cel))) else { continue };
                let Some((_, a)) = cases[k] else { continue };
                let Some(d) = self.doc.cels.get(&e.cel) else { continue };
                tuiles += e.masque.tuiles.len();
                let px = la_piece(&d.raster, &e.masque);
                if !px.is_blank() {
                    pieces.push(Piece { source: Source::Dessin { cel: e.cel, case: layout.cards[k].id }, pixels: Arc::new(px), vers_planche: a, cle: crate::horschamp::nouveau_lien() });
                }
            }
            if self.doc.annotations.hors_cadres_montre().is_some() {
                for m in bm.morceaux.iter().filter(|m| m.lien == Some(lien)) {
                    let Some(v) = m.vers_planche(layout) else { continue };
                    tuiles += crate::planchebitmap::tuiles_de(&m.pixels);
                    pieces.push(Piece { source: Source::Morceau { id: m.id }, pixels: m.pixels.clone(), vers_planche: v, cle: crate::horschamp::nouveau_lien() });
                }
            }
        }
        // (Deux empreintes d'un même dessin : une seule pièce par source.)
        let mut fondues: Vec<Piece> = Vec::new();
        for p in pieces {
            match fondues.iter_mut().find(|q| q.source == p.source) {
                Some(q) => {
                    let mut r = (*q.pixels).clone();
                    crate::planchebitmap::chaque_pixel(&p.pixels, |x, y, c| {
                        let t = TILE as i64;
                        let id = r.id_of(x.div_euclid(t) as i32, y.div_euclid(t) as i32);
                        r.tile_mut(id)[(y.rem_euclid(t) * t + x.rem_euclid(t)) as usize] = c;
                    });
                    q.pixels = Arc::new(r);
                }
                None => fondues.push(p),
            }
        }
        (tuiles <= LIMITE_TUILES).then_some((fondues, liens.to_vec()))
    }

    /// Où iront les pixels des pièces `pieces` menées par `g` (planche), **sans rien écrire**. Rien : le geste mène hors de toute place (planche pleine).
    fn selb_preparer(&self, layout: &BoardLayout, pieces: &[Piece], g: &A64, liberes: usize) -> Option<Plan> {
        let champs = crate::horschamp::champs(layout, self.settings.vignettes.debord_fraction());
        let cases = self.selb_cases(layout);
        let recoit: Vec<bool> = cases.iter().map(Option::is_some).collect();
        let n = layout.cards.len();
        let mut dessins: Vec<(usize, usize, Raster)> = Vec::new();
        let mut morceaux: BTreeMap<Option<usize>, Vec<(Raster, A64)>> = BTreeMap::new();
        for p in pieces {
            let origine = match p.source {
                Source::Dessin { case, .. } => layout.cards.iter().position(|c| c.id == case).unwrap_or(usize::MAX),
                Source::Morceau { .. } => usize::MAX,
            };
            let m = p.vers_planche.puis(g);
            let bornes = Self::bornes_des(std::slice::from_ref(&Piece { vers_planche: m, ..p.clone() }));
            if !bornes.is_finite() || !m.fini() {
                continue;
            }
            let candidats: Vec<usize> = (0..n).filter(|&k| champs[k].intersects(bornes.expand(2.0)) || layout.cards[k].thumb.intersects(bornes.expand(2.0)) || layout.move_of(k).is_some()).collect();
            let owner = |b: Pos2| -> Option<usize> {
                if origine < n && crate::horschamp::dans_image(layout, origine, b) {
                    return Some(origine);
                }
                if let Some(&k) = candidats.iter().find(|&&k| k != origine && crate::horschamp::dans_image(layout, k, b)) {
                    return Some(if recoit[k] { k } else { crate::horschamp::DESSUS + k });
                }
                if let Some(i) = layout.annulee_en(b) {
                    return Some(crate::horschamp::DESSUS + n + i);
                }
                if origine < n && recoit.get(origine).copied().unwrap_or(false) && champs[origine].contains(b) {
                    return Some(origine);
                }
                candidats.iter().copied().find(|&k| recoit[k] && champs[k].contains(b))
            };
            // Les cases qui reçoivent : les pixels transformés dans leur dessin, gardés là où elles en sont propriétaires.
            for &k in &candidats {
                let Some((rang, a)) = cases[k] else { continue };
                let Some(vers_dessin) = a.inverse() else { continue };
                let t = m.puis(&vers_dessin);
                // (Seules les tuiles de la pièce qui peuvent tomber dans le champ de cette case : les autres n'y ont rien à faire.)
                let zone = champs[k].union(layout.cards[k].thumb).union(layout.move_of(k).map_or(Rect::NOTHING, |mv| mv.tuile.rect)).expand(2.0);
                let mut sous = Raster::new(p.pixels.width, p.pixels.height);
                let tl = TILE as f32;
                for id in p.pixels.drawn_ids() {
                    let (ox, oy) = p.pixels.tile_origin(id);
                    let coins = [(0.0, 0.0), (tl, 0.0), (tl, tl), (0.0, tl)].map(|(u, v)| m.point(pos2(ox as f32 + u, oy as f32 + v)));
                    if Rect::from_points(&coins).intersects(zone) {
                        sous.replace_slot(id, p.pixels.slot(id).cloned());
                    }
                }
                if sous.is_blank() {
                    continue;
                }
                // Même échelle et sans rotation : les pixels recopiés tels quels, au pixel le plus proche (comme d'une case à l'autre au lever d'un trait) ;
                // sinon rééchantillonnés (`Raster::transformee`).
                let meme = (t.a - 1.0).abs() < 0.005 && (t.d - 1.0).abs() < 0.005 && t.b.abs() < 1e-4 && t.c.abs() < 1e-4;
                let mut arrivee: Raster = if meme { sous.decalee(t.tx.round() as i64, t.ty.round() as i64) } else { sous.transformee(&t.affine()) };
                drop(sous);
                // Gardés là où cette case en est propriétaire ; le reste est effacé de l'arrivée.
                let ids = arrivee.drawn_ids();
                for id in ids {
                    let (ox, oy) = arrivee.tile_origin(id);
                    let a_soi = {
                        let Some(tuile) = arrivee.slot(id) else { continue };
                        let px: &[Color32; TILE_PIXELS] = tuile;
                        let mut garde = [false; TILE_PIXELS];
                        let mut tout = true;
                        let mut rien = true;
                        for (i, c) in px.iter().enumerate() {
                            if c.a() == 0 {
                                continue;
                            }
                            let o = owner(a.point(pos2((ox + (i % TILE) as i64) as f32 + 0.5, (oy + (i / TILE) as i64) as f32 + 0.5))) == Some(k);
                            garde[i] = o;
                            tout &= o;
                            rien &= !o;
                        }
                        (garde, tout, rien)
                    };
                    match a_soi {
                        (_, _, true) => {
                            arrivee.replace_slot(id, None);
                        }
                        (_, true, _) => {}
                        (garde, _, _) => {
                            let d = arrivee.tile_mut(id);
                            for (i, g) in garde.iter().enumerate() {
                                if !g {
                                    d[i] = Color32::TRANSPARENT;
                                }
                            }
                        }
                    }
                }
                if arrivee.is_blank() {
                    continue;
                }
                dessins.push((k, rang, arrivee));
            }
            // Ce qu'aucune case ne reçoit : sur la planche, dans la grille de la pièce (aucun rééchantillonnage), par-dessus la case qui refuse s'il y en a une.
            let mut parts: BTreeMap<Option<usize>, Raster> = BTreeMap::new();
            crate::planchebitmap::chaque_pixel(&p.pixels, |x, y, c| {
                let o = owner(m.point(pos2(x as f32 + 0.5, y as f32 + 0.5)));
                if o.is_none_or(|q| q >= crate::horschamp::DESSUS) {
                    let code = o.map(|q| q - crate::horschamp::DESSUS);
                    crate::planchebitmap::poser_pixel(parts.entry(code).or_insert_with(pixels_vides), x, y, c, false);
                }
            });
            for (code, r) in parts {
                morceaux.entry(code).or_default().push((r, m));
            }
        }
        let a_poser: Vec<(Option<usize>, Raster, A64)> = morceaux.into_iter().flat_map(|(code, v)| v.into_iter().map(move |(r, m)| (code, r, m))).collect();
        if self.doc.annotations.bitmap.morceaux.len().saturating_sub(liberes) + a_poser.len() > crate::planchebitmap::MAX_MORCEAUX {
            return None;
        }
        Some(Plan { dessins, morceaux: a_poser })
    }

    /// Le choix reçoit `g` (planche) : tout est calculé, puis les pixels quittent leurs dessins et se reposent, répartis. `retirer` : faux pour un collage (les
    /// pièces viennent du presse-papiers). Une seule étape d'annulation, nommée `nom`.
    pub(crate) fn selb_reporter(&mut self, layout: &BoardLayout, g: &A64, nom: &str, retirer: bool) {
        let pieces: Vec<Piece> = if retirer {
            match self.vignettes.selb.as_ref().filter(|_| self.selb_active()) {
                Some(s) => s.pieces.clone(),
                None => return,
            }
        } else {
            match self.vignettes.selb_presse.clone() {
                Some(p) => p,
                None => return,
            }
        };
        if pieces.is_empty() || self.storyboard_refuses() {
            return;
        }
        let liens_pris: Vec<u64> = if retirer { self.vignettes.selb.as_ref().map(|s| s.liens.clone()).unwrap_or_default() } else { Vec::new() };
        let liberes = if retirer { pieces.iter().filter(|p| matches!(p.source, Source::Morceau { .. })).count() } else { 0 };
        // 1. Le plan, sans rien écrire.
        let Some(plan) = self.selb_preparer(layout, &pieces, g, liberes) else {
            self.status = format!("La planche a atteint sa limite de {} morceaux : ce geste mènerait des pixels hors des cases, il est refusé, rien n'a bougé.", crate::planchebitmap::MAX_MORCEAUX);
            return;
        };
        // 2. Maintenant seulement, on écrit.
        let marque = self.history.marque();
        let before = self.history.context.replace(nom.to_string());
        self.history.push(self.snapshot());
        let mut vieux: BTreeMap<CelId, Vec<(usize, Option<Tile>)>> = BTreeMap::new();
        let garder = |vieux: &mut BTreeMap<CelId, Vec<(usize, Option<Tile>)>>, cel: CelId, r: &Raster, id: usize| {
            let v = vieux.entry(cel).or_default();
            if !v.iter().any(|(k, _)| *k == id) {
                v.push((id, r.slot(id).cloned()));
            }
        };
        let lien = crate::horschamp::nouveau_lien();
        if retirer {
            for p in &pieces {
                match p.source {
                    Source::Dessin { cel, .. } => {
                        let Some(c) = self.doc.cels.get_mut(&cel) else { continue };
                        for id in p.pixels.drawn_ids() {
                            let (tx, ty) = p.pixels.key_of(id);
                            let Some(t) = p.pixels.slot(id) else { continue };
                            let t: &[Color32; TILE_PIXELS] = t;
                            let cid = c.raster.id_of(tx, ty);
                            if c.raster.slot(cid).is_none() {
                                continue;
                            }
                            garder(&mut vieux, cel, &c.raster, cid);
                            let d = c.raster.tile_mut(cid);
                            for (k, px) in t.iter().enumerate() {
                                if px.a() != 0 {
                                    d[k] = Color32::TRANSPARENT;
                                }
                            }
                        }
                        c.version += 1;
                    }
                    Source::Morceau { id } => {
                        let b = &mut self.doc.annotations.bitmap;
                        let Some(i) = b.morceaux.iter().position(|m| m.id == id) else { continue };
                        // (Pris en partie : le reste devient un morceau neuf, à la même place.)
                        let mut reste = (*b.morceaux[i].pixels).clone();
                        let mut pris_tout = true;
                        crate::planchebitmap::chaque_pixel(&b.morceaux[i].pixels, |x, y, _| {
                            if p.pixels.get_i(x, y).a() != 0 {
                                let t = TILE as i64;
                                let id = reste.id_of(x.div_euclid(t) as i32, y.div_euclid(t) as i32);
                                reste.tile_mut(id)[(y.rem_euclid(t) * t + x.rem_euclid(t)) as usize] = Color32::TRANSPARENT;
                            } else {
                                pris_tout = false;
                            }
                        });
                        if pris_tout {
                            b.morceaux.remove(i);
                        } else {
                            let ancien = b.morceaux[i].clone();
                            crate::planchebitmap::sans_tuiles_vides(&mut reste);
                            reste.geler();
                            b.morceaux[i] = crate::planchebitmap::Morceau { id: crate::horschamp::nouveau_lien(), pixels: Arc::new(reste), ..ancien };
                        }
                    }
                }
            }
            // Les gestes pris entiers ont quitté leurs places : leurs empreintes partent (le geste reposé a les siennes).
            self.doc.annotations.bitmap.empreintes.retain(|e| !liens_pris.contains(&e.lien));
        }
        // Les dessins d'arrivée.
        let mut empreintes: Vec<(CelId, Masque)> = Vec::new();
        for (k, rang, pixels) in plan.dessins {
            let Some(cel) = self.cel_de_la_case(layout, k, rang) else { continue };
            let Some(c) = self.doc.cels.get_mut(&cel) else { continue };
            let mut masque = match empreintes.iter().position(|(e, _)| *e == cel) {
                Some(i) => empreintes.swap_remove(i).1,
                None => Masque::default(),
            };
            for id in pixels.drawn_ids() {
                let (tx, ty) = pixels.key_of(id);
                let Some(src) = pixels.slot(id) else { continue };
                let src: &[Color32; TILE_PIXELS] = src;
                let cid = c.raster.id_of(tx, ty);
                garder(&mut vieux, cel, &c.raster, cid);
                let d = c.raster.tile_mut(cid);
                for (i, px) in src.iter().enumerate() {
                    if px.a() != 0 {
                        d[i] = crate::render::over(d[i], *px, 255, None);
                        *masque.mot(tx, ty, i / TILE) |= 1u64 << (i % TILE);
                    }
                }
            }
            c.version += 1;
            empreintes.push((cel, masque));
        }
        let mut pose_un = false;
        for (code, r, m) in plan.morceaux {
            let sur = code.and_then(|q| crate::horschamp::case_du_code(layout, crate::horschamp::DESSUS + q).map(|c| (c, layout.cards.iter().position(|j| j.id == c.id).unwrap_or(0))));
            pose_un |= self.doc.annotations.bitmap.ajouter(layout, r, m, sur, Some(lien), false).is_some();
        }
        if pose_un {
            self.doc.annotations.calque_hors_cadres();
            if let Some(id) = self.doc.annotations.id_hors_cadres() {
                self.doc.annotations.regler_visible(id, true);
            }
        }
        for (cel, m) in empreintes {
            self.doc.annotations.bitmap.noter_empreinte(lien, cel, m);
        }
        for (cel, tiles) in vieux {
            self.history.push(Edit::Tiles { cel, tiles });
        }
        self.history.regrouper_depuis(marque);
        self.history.context = before;
        self.modified = true;
        self.renderer.invalidate_all();
        // Le choix suit à sa nouvelle place ; ce qui a bougé reste peint jusqu'à ce que les vignettes soient refaites.
        let tenu: Vec<Piece> = pieces.iter().map(|p| Piece { vers_planche: p.vers_planche.puis(g), ..p.clone() }).collect();
        let layout = self.board_layout();
        let suivant = self.selb_prendre_liens(&layout, &[lien]).unwrap_or_default();
        let serial = self.history.serial();
        let bornes = Self::bornes_des(&suivant.0);
        let etapes = self.history.undo_len();
        let s = self.vignettes.selb.get_or_insert_with(SelBitmap::default);
        s.pieces = suivant.0;
        s.liens = vec![lien];
        s.serial = serial;
        s.pivot = None;
        s.bornes = bornes;
        s.tenu = Some((tenu, etapes, std::time::Instant::now()));
        self.ctx.request_repaint();
    }

    /// Suppr : les pixels choisis partent, partout, en une étape.
    pub(crate) fn selb_supprimer(&mut self) {
        let Some(pieces) = self.vignettes.selb.as_ref().filter(|_| self.selb_active()).map(|s| s.pieces.clone()) else { return };
        if self.storyboard_refuses() {
            return;
        }
        let liens = self.vignettes.selb.as_ref().map(|s| s.liens.clone()).unwrap_or_default();
        let marque = self.history.marque();
        let before = self.history.context.replace("Effacer".to_string());
        self.history.push(self.snapshot());
        let mut vieux: BTreeMap<CelId, Vec<(usize, Option<Tile>)>> = BTreeMap::new();
        for p in &pieces {
            match p.source {
                Source::Dessin { cel, .. } => {
                    let Some(c) = self.doc.cels.get_mut(&cel) else { continue };
                    for id in p.pixels.drawn_ids() {
                        let (tx, ty) = p.pixels.key_of(id);
                        let Some(t) = p.pixels.slot(id) else { continue };
                        let t: &[Color32; TILE_PIXELS] = t;
                        let cid = c.raster.id_of(tx, ty);
                        if c.raster.slot(cid).is_none() {
                            continue;
                        }
                        let v = vieux.entry(cel).or_default();
                        if !v.iter().any(|(k, _)| *k == cid) {
                            v.push((cid, c.raster.slot(cid).cloned()));
                        }
                        let d = c.raster.tile_mut(cid);
                        for (k, px) in t.iter().enumerate() {
                            if px.a() != 0 {
                                d[k] = Color32::TRANSPARENT;
                            }
                        }
                    }
                    c.version += 1;
                }
                Source::Morceau { id } => {
                    let b = &mut self.doc.annotations.bitmap;
                    let Some(i) = b.morceaux.iter().position(|m| m.id == id) else { continue };
                    let mut reste = (*b.morceaux[i].pixels).clone();
                    let mut pris_tout = true;
                    crate::planchebitmap::chaque_pixel(&b.morceaux[i].pixels, |x, y, _| {
                        if p.pixels.get_i(x, y).a() != 0 {
                            let t = TILE as i64;
                            let id = reste.id_of(x.div_euclid(t) as i32, y.div_euclid(t) as i32);
                            reste.tile_mut(id)[(y.rem_euclid(t) * t + x.rem_euclid(t)) as usize] = Color32::TRANSPARENT;
                        } else {
                            pris_tout = false;
                        }
                    });
                    if pris_tout {
                        b.morceaux.remove(i);
                    } else {
                        let ancien = b.morceaux[i].clone();
                        crate::planchebitmap::sans_tuiles_vides(&mut reste);
                        reste.geler();
                        b.morceaux[i] = crate::planchebitmap::Morceau { id: crate::horschamp::nouveau_lien(), pixels: Arc::new(reste), ..ancien };
                    }
                }
            }
        }
        self.doc.annotations.bitmap.empreintes.retain(|e| !liens.contains(&e.lien));
        for (cel, tiles) in vieux {
            self.history.push(Edit::Tiles { cel, tiles });
        }
        self.history.regrouper_depuis(marque);
        self.history.context = before;
        self.modified = true;
        self.renderer.invalidate_all();
        self.vignettes.selb = None;
        self.status = "Les pixels choisis sont effacés, partout où ils étaient.".into();
    }

    /// Ctrl+C : le choix, à sa place sur la planche (pas dans le presse-papiers du système). Vrai si copié.
    pub(crate) fn selb_copier(&mut self) -> bool {
        let Some(pieces) = self.vignettes.selb.as_ref().filter(|_| self.selb_active()).map(|s| s.pieces.clone()) else { return false };
        // (Une copie n'est plus une part du geste d'origine : ses pièces n'ont pas de source à retirer.)
        self.vignettes.selb_presse = Some(pieces.into_iter().map(|p| Piece { cle: crate::horschamp::nouveau_lien(), ..p }).collect());
        self.status = "Pixels copiés : Ctrl+V les repose à leur place, répartis.".into();
        true
    }

    /// Ctrl+V dans les Vignettes, calque bitmap, après un Ctrl+C de la planche : les pixels reviennent à leur place, répartis. Vrai si collé.
    pub(crate) fn selb_coller(&mut self) -> bool {
        if !self.selb_possible() || self.vignettes.selb_presse.is_none() {
            return false;
        }
        let layout = self.board_layout();
        self.selb_reporter(&layout, &A64::IDENTITE, "Coller", false);
        true
    }

    /// Le choix à l'écran : le lasso en cours, l'aperçu du geste (les pixels à leur nouvelle place), la boîte et ses poignées.
    pub(crate) fn selb_peindre(&self, painter: &egui::Painter, view: Rect, survol: Option<Pos2>) {
        use crate::theme::{SELECTION_BOITE as ACCENT, SELECTION_POIGNEE_FOND};
        self.vignettes.selb_tex.borrow_mut().debut();
        let Some(s) = self.vignettes.selb.as_ref() else { return };
        let xf = self.vignettes.xf;
        let clip = painter.with_clip_rect(view);
        // Ce qui vient de bouger, tant que les vignettes ne sont pas refaites.
        if let Some((tenu, etapes, debut)) = &s.tenu {
            let pret = self.thumbs.outstanding() == 0 && !self.thumbs.pending && !self.rejeu.has_pending();
            if !pret && *etapes == self.history.undo_len() && debut.elapsed() < crate::debord::TENU_MAX {
                let mut tex = self.vignettes.selb_tex.borrow_mut();
                for p in tenu {
                    tex.peindre_pixels(&clip, view, xf, p.cle, &p.pixels, p.vers_planche, Color32::WHITE);
                }
                painter.ctx().request_repaint_after(std::time::Duration::from_millis(50));
            }
        }
        if let Some(l) = &s.lasso {
            let pts: Vec<Pos2> = l.iter().map(|p| xf.to_screen(*p)).collect();
            if let Some(shape) = crate::contours::lasso_shape(&pts) {
                clip.add(shape);
            }
        }
        if !self.selb_active() {
            return;
        }
        let geste = s.geste.as_ref().filter(|g| g.change && g.handle != Handle::Pivot).map(|g| g.aff);
        if let Some(aff) = geste {
            let g = A64::de(&aff);
            let mut tex = self.vignettes.selb_tex.borrow_mut();
            let mut complet = true;
            for p in &s.pieces {
                complet &= tex.peindre_pixels(&clip, view, xf, p.cle, &p.pixels, p.vers_planche.puis(&g), Color32::WHITE);
            }
            if !complet {
                painter.ctx().request_repaint();
            }
        }
        let frame = match s.geste.as_ref() {
            Some(g) if g.handle != Handle::Pivot => Some(g.frame.then(&g.aff)),
            _ => self.selb_cadre(xf.zoom),
        };
        let Some(frame) = frame else { return };
        let quad = frame.corners().map(|q| xf.to_screen(q));
        let pivot = xf.to_screen(frame.pivot_world());
        if !quad.iter().chain([&pivot]).all(|p| p.is_finite()) {
            return;
        }
        let edge = Stroke::new(1.0_f32, ACCENT);
        for k in 0..4 {
            clip.line_segment([quad[k], quad[(k + 1) % 4]], edge);
        }
        let hover = s.geste.as_ref().map(|g| g.handle).or_else(|| survol.and_then(|p| selbox::hit_test(quad, pivot, p, &Grabs::default())));
        let ppp = painter.ctx().pixels_per_point();
        let poignee = |c: Pos2, on: bool| {
            let c = selbox::snap_handle_center(c, ppp);
            clip.rect(selbox::handle_rect(c, selbox::HANDLE_SIDE), 0.0, if on { ACCENT } else { SELECTION_POIGNEE_FOND }, Stroke::new(1.0_f32, ACCENT), egui::StrokeKind::Middle);
        };
        for (k, c) in quad.iter().enumerate() {
            poignee(*c, hover == Some(Handle::Corner(k)));
        }
        for k in 0..4 {
            poignee(quad[k] + (quad[(k + 1) % 4] - quad[k]) * 0.5, hover == Some(Handle::Edge(k)));
        }
        clip.circle_stroke(pivot, selbox::PIVOT_RADIUS, Stroke::new(1.0_f32, ACCENT));
    }
}
