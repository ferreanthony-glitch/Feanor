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
use crate::planchebitmap::{Masque, A64};
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

#[cfg(test)]
thread_local! {
    /// Essais : la mesure de la limite lève aussi la borne d'arrivée.
    pub(crate) static SANS_LIMITE_D_ARRIVEE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Le plus de tuiles qu'un geste peut poser à l'arrivée.
fn limite_arrivee() -> usize {
    #[cfg(test)]
    if SANS_LIMITE_D_ARRIVEE.with(|c| c.get()) {
        return usize::MAX;
    }
    2 * LIMITE_TUILES
}

/// Pourquoi une sélection ne suit pas le trait entier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Echec {
    /// Au-delà de `LIMITE_TUILES`.
    Limite,
    /// Une empreinte du trait ne dit plus où il est : son dessin a changé sous elle depuis qu'il a été réparti.
    Perime,
}

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
    /// La disposition de la planche au moment du choix (`cle_planche`) : si elle change, le choix est levé.
    pub planche: u64,
}

impl Default for SelBitmap {
    fn default() -> Self {
        SelBitmap { pieces: Vec::new(), liens: Vec::new(), serial: 0, lasso: None, geste: None, pivot: None, bornes: Rect::NOTHING, tenu: None, limite_atteinte: false, planche: 0 }
    }
}

/// La disposition de la planche : les cases (et les cases écartées), leurs places.
pub(crate) fn cle_planche(layout: &BoardLayout) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut manger = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x0100_0000_01b3);
    };
    for c in layout.cards.iter().chain(layout.annulees.iter()) {
        manger(c.id);
        for v in [c.thumb.min.x, c.thumb.min.y, c.thumb.max.x, c.thumb.max.y] {
            manger(u64::from(v.to_bits()));
        }
    }
    h
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
        let id = out.id_of(tx, ty);
        // (Lue sans garder ouverte une tuile rangée.)
        let (entiere, neuve, une) = t.avec_les_pixels(|px| {
            let entiere = px.iter().enumerate().all(|(k, c)| c.a() == 0 || bits[k / TILE] >> (k % TILE) & 1 == 1);
            let mut neuve = [Color32::TRANSPARENT; TILE_PIXELS];
            let mut une = false;
            if !entiere {
                for (k, c) in px.iter().enumerate() {
                    if c.a() != 0 && bits[k / TILE] >> (k % TILE) & 1 == 1 {
                        neuve[k] = *c;
                        une = true;
                    }
                }
            }
            (entiere, neuve, une)
        });
        if entiere {
            // (Toute l'encre de la tuile est prise : la même mémoire, rien de recopié.)
            out.replace_slot(id, Some(t.clone()));
            continue;
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
    fn selb_prendre(&self, layout: &BoardLayout, poly: &[Pos2], seule: Option<usize>, clic: Option<(Pos2, f32)>) -> Result<(Vec<Piece>, Vec<u64>), Echec> {
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
        self.vignettes.selb = Some(SelBitmap { pieces, serial, planche: cle_planche(layout), ..SelBitmap::default() });
        self.selb_reporter(layout, g, "Déplacer", true);
    }

    /// Essais : le lasso `poly` avec la limite (rend vrai s'il est refusé pour la limite).
    #[cfg(test)]
    pub(crate) fn selb_lasso_refuse_pour_essai(&self, layout: &BoardLayout, poly: &[Pos2]) -> bool {
        matches!(self.selb_prendre(layout, poly, None, None), Err(Echec::Limite))
    }
    /// Essais : le geste `lien` se prend-il entier (sous la limite) ?
    #[cfg(test)]
    pub(crate) fn selb_du_lien_pour_essai(&self, layout: &BoardLayout, lien: u64) -> bool {
        self.selb_prendre_liens(layout, &[lien]).is_ok()
    }

    fn selb_prendre_jusqu_a(&self, layout: &BoardLayout, poly: &[Pos2], seule: Option<usize>, clic: Option<(Pos2, f32)>, limite: usize) -> Result<(Vec<Piece>, Vec<u64>), Echec> {
        let champs = crate::horschamp::champs(layout, self.settings.vignettes.debord_fraction());
        // Le passe-partout montre le film : ce qui dépasse des cases est caché, et le papier passe sous leurs images. On ne prend que ce qu'on voit.
        let film = self.vignettes.annot.marie;
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
            // La règle d'avant la partie 131 (« seulement là où la case se voit ») vaut encore quand la planche ne montre pas le dessin loin de
            // sa case : avec le passe-partout, et pour un dessin que plusieurs cases montrent (`render::dessins_communs`).
            let pres_de_sa_case = film || self.dessin_commun(layout, k, cel);
            let Some(d) = self.doc.cels.get(&cel) else { continue };
            let Some(inv) = a.inverse() else { continue };
            let source = Source::Dessin { cel, case: layout.cards[k].id };
            if let Some((p, tol)) = clic {
                if pres_de_sa_case && !champ.contains(p) {
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
            if pres_de_sa_case && !champ.intersects(boite) {
                continue;
            }
            let poly_px: Vec<Pos2> = poly.iter().map(|p| inv.point(*p)).collect();
            // (Partie 131 : la case montre tout son dessin sur la planche, où qu'il aille, par-dessus les autres cases aussi : il se prend partout où il
            // a des pixels. Avant, seulement là où la case se voyait : son image, sa marge hors de l'image d'une autre. Seulement là où le dessin a des tuiles.)
            let Some((dx0, dy0, dx1, dy1)) = crate::planchebitmap::boite_px(&d.raster) else { continue };
            let mut b = Rect::from_points(&poly_px).intersect(Rect::from_min_max(pos2(dx0 as f32, dy0 as f32), pos2(dx1 as f32, dy1 as f32)));
            if pres_de_sa_case {
                // (Seulement là où la case se voit : son champ, en pixels de son dessin.)
                b = b.intersect(Rect::from_points(&[champ.left_top(), champ.right_top(), champ.right_bottom(), champ.left_bottom()].map(|p| inv.point(p))));
            }
            if !b.is_positive() || !boite.is_positive() {
                continue;
            }
            // Près de sa case : l'image de la case en pixels (là, tout ce qu'on voit est à elle), et les cases voisines dont l'image peut cacher sa marge.
            let mouvement = layout.move_of(k).is_some();
            let t = &layout.cards[k].thumb;
            let image_px = Rect::from_points(&[t.left_top(), t.right_bottom()].map(|p| inv.point(p)));
            let voisines: Vec<usize> = (0..layout.cards.len()).filter(|&j| j != k && (layout.cards[j].thumb.intersects(*champ) || layout.move_of(j).is_some())).collect();
            let vu = |x: i64, y: i64| -> bool {
                if !pres_de_sa_case {
                    return true;
                }
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
                            let ligne = (y.rem_euclid(tl) * tl) as usize;
                            let bits = tuile.avec_les_pixels(|px| {
                                let mut bits = 0u64;
                                for xx in x..fin {
                                    let k2 = (xx - tx * tl) as usize;
                                    if px[ligne + k2].a() != 0 && vu(xx, y) {
                                        bits |= 1u64 << k2;
                                    }
                                }
                                bits
                            });
                            if bits != 0 {
                                *pris[i].1.mot(tx as i32, y.div_euclid(tl) as i32, y.rem_euclid(tl) as usize) |= bits;
                            }
                        }
                        x = fin;
                    }
                }
            }
            // La limite, au fur et à mesure : une sélection trop grande s'arrête là, sans avoir tout parcouru.
            if pris.iter().map(|(_, m)| m.tuiles.len()).sum::<usize>() > limite {
                return Err(Echec::Limite);
            }
        }
        // 2. La couche bitmap de la planche (montrée) : partout (partie 131 : le papier se peint par-dessus les images des cases, rien ne le cache) ;
        // avec le passe-partout, comme avant : par-dessus une case, ou sur le papier hors des images.
        if montre && seule.is_none() {
            for m in &bm.morceaux {
                let Some(v) = m.vers_planche(layout) else { continue };
                let Some(bornes) = m.bornes(layout) else { continue };
                let Some(inv) = v.inverse() else { continue };
                let visible = |p: Pos2| !film || m.dessus || (layout.card_at(p).is_none() && layout.annulee_en(p).is_none());
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
                            if crate::planchebitmap::alpha_sans_garder(&m.pixels, x, y) != 0 && visible(v.point(pos2(x as f32 + 0.5, y as f32 + 0.5))) {
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
                            // (Mot par mot : deux masques se touchent-ils ?)
                            let touche = m.tuiles.iter().any(|(k, a)| e.masque.tuiles.get(k).is_some_and(|b| a.iter().zip(b.iter()).any(|(x, y)| x & y != 0)));
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
            // Une empreinte que le dessin a dépassée (des pixels déplacés au Storyboard, un trait peint par-dessus…) ne dit plus où est le trait : on ne la
            // suit pas, la sélection reste celle de la case (l'appelant le dit).
            if !self.selb_liens_fiables(&liens) {
                return Err(Echec::Perime);
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
            return Err(Echec::Limite);
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
        Ok((pieces, liens))
    }

    /// Les empreintes des gestes `liens` disent-elles encore où sont leurs pixels (chaque dessin inchangé sous son empreinte) ?
    fn selb_liens_fiables(&self, liens: &[u64]) -> bool {
        self.doc.annotations.bitmap.empreintes.iter().filter(|e| liens.contains(&e.lien)).all(|e| self.doc.cels.get(&e.cel).is_none_or(|d| e.fiable_a(&d.raster, d.version)))
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
        // (Une couche qu'on n'a pas pu lire ne pourrait pas s'enregistrer : on n'y mène rien. La sélection d'une case, d'avant, reste possible.)
        if self.doc.annotations.bitmap.illisible {
            self.vignettes.selb = None;
            self.status = "La couche bitmap de la planche n'a pas pu être lue à l'ouverture : la sélection de la planche est coupée (rien n'est perdu) ; Ctrl + lasso choisit dans la case.".into();
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
        // La planche a changé de disposition depuis le choix (fenêtre, séquences montrées, cases) : ses places ne valent plus, le choix est levé.
        let cle = cle_planche(layout);
        if self.vignettes.selb.as_ref().is_some_and(|s| !s.pieces.is_empty() && s.planche != cle) {
            self.vignettes.selb = None;
            self.status = "La planche a changé : le choix est levé.".into();
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
                let mut echec: Option<Echec> = None;
                let trouve = if clic {
                    // (Un clic : les traits répartis sous le pointeur, entiers ; rien si l'un d'eux ne peut plus l'être.)
                    match self.selb_prendre(layout, &[], None, Some((l[0], PRISE_PX / zoom))) {
                        Ok((_, liens)) if liens.is_empty() => Some((Vec::new(), liens)),
                        Ok((_, liens)) => match self.selb_du_lien(layout, &liens) {
                            Ok(t) => Some(t),
                            Err(e) => {
                                echec = Some(e);
                                None
                            }
                        },
                        Err(e) => {
                            echec = Some(e);
                            None
                        }
                    }
                } else {
                    match self.selb_prendre(layout, &l, None, None) {
                        Ok(t) => Some(t),
                        Err(e) => {
                            // Au-delà de la limite, ou un trait qu'on ne sait plus suivre : la sélection reste celle de la case où le lasso commence.
                            echec = Some(e);
                            let d = |r: Rect, p: Pos2| ((r.left() - p.x).max(p.x - r.right()).max(0.0)).hypot((r.top() - p.y).max(p.y - r.bottom()).max(0.0));
                            let k = layout.card_at(l[0]).or_else(|| (0..layout.cards.len()).min_by(|a, b| d(layout.cards[*a].thumb, l[0]).total_cmp(&d(layout.cards[*b].thumb, l[0]))));
                            k.and_then(|k| self.selb_prendre(layout, &l, Some(k), None).ok())
                        }
                    }
                };
                let limite = echec.is_some();
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
        s.planche = cle_planche(&layout);
                s.limite_atteinte = limite;
                let cases = s.pieces.iter().filter(|p| matches!(p.source, Source::Dessin { .. })).count();
                let planche = s.pieces.len() - cases;
                let entiers = s.liens.len();
                let pourquoi = match echec {
                    Some(Echec::Limite) => "Sélection trop grande pour suivre tout le trait",
                    _ => "Ce trait a changé depuis qu'il a été tracé (pixels déplacés ou repeints ailleurs) : on ne sait plus le suivre",
                };
                self.status = if s.pieces.is_empty() {
                    if limite { format!("{pourquoi}, et rien n'est pris.") } else { "Rien dans le lasso.".into() }
                } else if limite {
                    format!("{pourquoi} : seule la part de cette case est prise.")
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
    fn selb_du_lien(&self, layout: &BoardLayout, liens: &[u64]) -> Result<(Vec<Piece>, Vec<u64>), Echec> {
        self.selb_prendre_liens(layout, liens)
    }

    fn selb_prendre_liens(&self, layout: &BoardLayout, liens: &[u64]) -> Result<(Vec<Piece>, Vec<u64>), Echec> {
        if !self.selb_liens_fiables(liens) {
            return Err(Echec::Perime);
        }
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
        if tuiles > LIMITE_TUILES { Err(Echec::Limite) } else { Ok((fondues, liens.to_vec())) }
    }

    /// Où iront les pixels des pièces `pieces` menées par `g` (planche), **sans rien écrire**. **Chaque pièce va tout entière à un seul endroit** (partie 131 :
    /// le bord d'une case ne coupe rien) : le dessin de la case d'où elle vient tant qu'il lui reste de l'encre dans son champ, sinon la case ou le papier à qui
    /// revient le centre de sa boîte ; jamais deux endroits ni aucun. `liberes` : les morceaux que le geste vide entièrement. Une erreur (la phrase de la ligne
    /// d'état) : rien ne bougera.
    fn selb_preparer(&self, layout: &BoardLayout, pieces: &[Piece], g: &A64, liberes: usize) -> Result<Plan, String> {
        let champs = crate::horschamp::champs(layout, self.settings.vignettes.debord_fraction());
        let cases = self.selb_cases(layout);
        let recoit: Vec<bool> = cases.iter().map(Option::is_some).collect();
        let n = layout.cards.len();
        let monde = crate::raster::WORLD as f32 - TILE as f32;
        let mut dessins: Vec<(usize, usize, Raster)> = Vec::new();
        let mut a_poser: Vec<(Option<usize>, Raster, A64)> = Vec::new();
        let mut tuiles_arrivee = 0usize;
        for p in pieces {
            let origine = match p.source {
                Source::Dessin { case, .. } => layout.cards.iter().position(|c| c.id == case).unwrap_or(usize::MAX),
                Source::Morceau { .. } => usize::MAX,
            };
            let m = p.vers_planche.puis(g);
            let bornes = Self::bornes_des(std::slice::from_ref(&Piece { vers_planche: m, ..p.clone() }));
            if !bornes.is_finite() || !m.fini() {
                return Err("Ce geste mène des pixels hors de toute place : il est refusé, rien n'a bougé.".into());
            }
            let candidats: Vec<usize> = (0..n).filter(|&k| champs[k].intersects(bornes.expand(2.0)) || layout.cards[k].thumb.intersects(bornes.expand(2.0)) || layout.move_of(k).is_some()).collect();
            // À qui revient le point `b` de la planche : l'image d'une case d'abord, puis une marge (celle de la case d'où viennent les pixels d'abord).
            let chez = |b: Pos2| -> Option<usize> {
                if origine < n && crate::horschamp::dans_image(layout, origine, b) {
                    return Some(if recoit[origine] { origine } else { crate::horschamp::DESSUS + origine });
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
            // 1. Partie 131 : **la pièce va tout entière à un seul endroit**, plus aucun pixel n'est trié d'après la case qu'il recouvre. Des pixels pris
            // dans le dessin d'une case y restent tant qu'il leur reste de l'encre dans son champ (son image, sa marge) : on les a déplacés chez elle.
            // Sortis tout entiers de son champ, ou venus du papier : ils vont à qui revient le centre de leur boîte, une case ou le papier.
            let encre_dans_le_champ = |k: usize| -> bool {
                p.pixels.drawn_ids().into_iter().any(|id| {
                    let Some(tuile) = p.pixels.slot(id) else { return false };
                    let (ox, oy) = p.pixels.tile_origin(id);
                    // (La tuile entière d'abord : loin du champ, aucun pixel n'est lu.)
                    let tl = TILE as f32;
                    let coins = [(0.0, 0.0), (tl, 0.0), (tl, tl), (0.0, tl)].map(|(u, v)| m.point(pos2(ox as f32 + u, oy as f32 + v)));
                    if !Rect::from_points(&coins).intersects(champs[k]) && layout.move_of(k).is_none() {
                        return false;
                    }
                    tuile.avec_les_pixels(|px| {
                        px.iter().enumerate().any(|(i, c)| {
                            c.a() != 0 && {
                                let b = m.point(pos2((ox + (i % TILE) as i64) as f32 + 0.5, (oy + (i / TILE) as i64) as f32 + 0.5));
                                champs[k].contains(b) || crate::horschamp::dans_image(layout, k, b)
                            }
                        })
                    })
                })
            };
            let qui: Option<usize> = if origine < n && recoit[origine] && encre_dans_le_champ(origine) { Some(origine) } else { chez(bornes.center()) };
            // Un dessin que plusieurs cases montrent (Décor, planche-mère… `render::dessins_communs`) ne se voit pas loin de sa case : la pièce
            // n'y entre que si elle tient tout entière dans le champ de la case. Sinon elle va sur la planche, par-dessus la case, où elle se voit.
            let qui = match qui {
                Some(k) if k < n && cases[k].is_some_and(|(rang, _)| self.calque_commun(layout, k, rang)) && !champs[k].contains_rect(bornes) && layout.move_of(k).is_none() => Some(crate::horschamp::DESSUS + k),
                q => q,
            };
            let mut par_case: BTreeMap<usize, Raster> = BTreeMap::new();
            let mut parts: BTreeMap<Option<usize>, Raster> = BTreeMap::new();
            for id in p.pixels.drawn_ids() {
                let Some(tuile) = p.pixels.slot(id) else { continue };
                // (Lue sans garder ouverte une tuile rangée d'un morceau. Une tuile sans encre ne compte pas.)
                if !tuile.avec_les_pixels(|px| px.iter().any(|c| c.a() != 0)) {
                    continue;
                }
                let (tx, ty) = p.pixels.key_of(id);
                let dest: &mut Raster = match qui {
                    Some(k) if k < crate::horschamp::DESSUS => par_case.entry(k).or_insert_with(|| Raster::new(p.pixels.width, p.pixels.height)),
                    _ => parts.entry(qui.map(|q| q - crate::horschamp::DESSUS)).or_insert_with(|| Raster::new(p.pixels.width, p.pixels.height)),
                };
                // (La même mémoire : rien n'est recopié.)
                let did = dest.id_of(tx, ty);
                dest.replace_slot(did, Some(tuile.clone()));
            }
            // 2. Les cases qui reçoivent : leurs pixels dans leur dessin. Même échelle et sans rotation : recopiés tels quels, au pixel le plus proche
            // (comme d'une case à l'autre au lever d'un trait) ; sinon rééchantillonnés (`Raster::transformee`).
            for (k, sous) in par_case {
                let Some((rang, a)) = cases[k] else { continue };
                let Some(vers_dessin) = a.inverse() else { continue };
                let t = m.puis(&vers_dessin);
                // (Ce que l'arrivée couvrira, avant de la faire : dans la toile du dessin, et dans la limite.)
                let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
                for id in sous.drawn_ids() {
                    let (ox, oy) = sous.tile_origin(id);
                    let tl = TILE as f32;
                    let coins = [(0.0, 0.0), (tl, 0.0), (tl, tl), (0.0, tl)].map(|(u, v)| t.point(pos2(ox as f32 + u, oy as f32 + v)));
                    let r = Rect::from_points(&coins);
                    (x0, y0, x1, y1) = (x0.min(r.left()), y0.min(r.top()), x1.max(r.right()), y1.max(r.bottom()));
                    tuiles_arrivee += ((r.width() * r.height()) / (TILE * TILE) as f32).ceil().max(1.0) as usize;
                }
                if ![x0, y0, x1, y1].iter().all(|v| v.is_finite() && v.abs() < monde) {
                    return Err("Ce geste mène des pixels trop loin de la feuille de leur plan : il est refusé, rien n'a bougé.".into());
                }
                // (Deux fois la limite : une sélection permise, tournée ou un peu agrandie, se repose ; une mise à l'échelle démesurée, non.)
                if tuiles_arrivee > limite_arrivee() {
                    return Err("Ce geste agrandit trop la sélection pour la reposer : il est refusé, rien n'a bougé.".into());
                }
                let meme = (t.a - 1.0).abs() < 0.005 && (t.d - 1.0).abs() < 0.005 && t.b.abs() < 1e-4 && t.c.abs() < 1e-4;
                let ouverte = if matches!(p.source, Source::Morceau { .. }) { crate::planchebitmap::copie_ouverte(&sous) } else { sous };
                let arrivee: Raster = if meme { ouverte.decalee(t.tx.round() as i64, t.ty.round() as i64) } else { ouverte.transformee(&t.affine()) };
                if !arrivee.is_blank() {
                    dessins.push((k, rang, arrivee));
                }
            }
            // 3. Ce qu'aucune case ne reçoit : sur la planche, dans la grille de la pièce (aucun rééchantillonnage), par-dessus la case qui refuse s'il y en a une.
            for (code, r) in parts {
                a_poser.push((code, r, m));
            }
        }
        if !a_poser.is_empty() && layout.cards.is_empty() {
            return Err("Il n'y a aucune case où attacher ces pixels sur la planche : le geste est refusé, rien n'a bougé.".into());
        }
        let b = &self.doc.annotations.bitmap;
        if b.illisible {
            return Err("La couche bitmap de la planche n'a pas pu être lue à l'ouverture : ce geste y mènerait des pixels, il est refusé, rien n'a bougé.".into());
        }
        if !a_poser.is_empty() && !self.planche_bitmap_ecrivable() {
            return Err("Dans ce film partagé, le montage n'est pas à toi : la planche ne peut pas recevoir ces pixels, le geste est refusé, rien n'a bougé.".into());
        }
        // (Les morceaux sans image comptent, comme pour `ajouter` ; ne libère une place que le morceau que le geste vide entièrement.)
        if !a_poser.is_empty() && !b.peut_poser_apres(liberes, a_poser.len()) {
            return Err(format!("La planche a atteint sa limite de {} morceaux : ce geste mènerait des pixels hors des cases, il est refusé, rien n'a bougé.", crate::planchebitmap::MAX_MORCEAUX));
        }
        Ok(Plan { dessins, morceaux: a_poser })
    }
    /// Le choix reçoit `g` (planche) : tout est calculé, puis les pixels quittent leurs dessins et se reposent, chaque pièce entière à un seul endroit
    /// (partie 131, `selb_preparer`). `retirer` : faux pour un collage (les
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
        // (Ne libère une place que le morceau que le geste vide entièrement ; pris en partie, il reste, en un morceau neuf.)
        let compter = |r: &Raster| {
            let mut n = 0usize;
            crate::planchebitmap::chaque_pixel(r, |_, _, _| n += 1);
            n
        };
        let liberes = if retirer {
            pieces
                .iter()
                .filter(|p| match p.source {
                    Source::Morceau { id } => self.doc.annotations.bitmap.morceaux.iter().find(|m| m.id == id).is_some_and(|m| compter(&m.pixels) == compter(&p.pixels)),
                    _ => false,
                })
                .count()
        } else {
            0
        };
        // 1. Le plan, sans rien écrire.
        let plan = match self.selb_preparer(layout, &pieces, g, liberes) {
            Ok(p) => p,
            Err(phrase) => {
                self.status = phrase;
                return;
            }
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
                            let t: [Color32; TILE_PIXELS] = t.avec_les_pixels(|p| *p);
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
                            if crate::planchebitmap::alpha_sans_garder(&p.pixels, x, y) != 0 {
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
                let src: [Color32; TILE_PIXELS] = src.avec_les_pixels(|p| *p);
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
        let mut echec = false;
        for (code, r, m) in plan.morceaux {
            let sur = code.and_then(|q| crate::horschamp::case_du_code(layout, crate::horschamp::DESSUS + q).map(|c| (c, layout.cards.iter().position(|j| j.id == c.id).unwrap_or(0))));
            let vide = r.is_blank();
            let fait = self.doc.annotations.bitmap.ajouter(layout, r, m, sur, Some(lien), false).is_some();
            echec |= !fait && !vide;
            pose_un |= fait;
        }
        if echec {
            // (Tout a été vérifié avant ; si la planche refuse quand même, tout le geste est défait : rien n'a bougé, rien n'est perdu.)
            for (cel, tiles) in vieux {
                self.history.push(Edit::Tiles { cel, tiles });
            }
            self.history.regrouper_depuis(marque);
            self.history.context = before;
            self.undo();
            self.status = "La planche n'a pas pu recevoir ces pixels : le geste est défait, rien n'a bougé.".into();
            return;
        }
        if pose_un {
            self.doc.annotations.calque_hors_cadres();
            if let Some(id) = self.doc.annotations.id_hors_cadres() {
                self.doc.annotations.regler_visible(id, true);
            }
        }
        let mut sans_empreinte = false;
        {
            let doc = &mut self.doc;
            for (cel, m) in empreintes {
                let Some(d) = doc.cels.get(&cel) else { continue };
                let cels = &doc.cels;
                sans_empreinte |= !doc.annotations.bitmap.noter_empreinte(lien, cel, m, &d.raster, |e| cels.get(&e.cel).is_none_or(|x| e.fiable_a(&x.raster, x.version)));
            }
        }
        if sans_empreinte {
            self.status = "Trop de traits suivis sur la planche : ces pixels ne pourront plus être choisis entiers, seulement case par case.".into();
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
        s.planche = cle_planche(&layout);
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
                        let t: [Color32; TILE_PIXELS] = t.avec_les_pixels(|p| *p);
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
                        if crate::planchebitmap::alpha_sans_garder(&p.pixels, x, y) != 0 {
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
        // (Le marqueur de FEANOR dans le presse-papiers du système, comme les autres copies : une image restée là d'avant ne passe plus devant au Ctrl+V.)
        self.ctx.copy_text(format!("{}pixels de la planche", crate::pasteimage::MARK));
        self.status = "Pixels copiés : Ctrl+V les repose à leur place.".into();
        true
    }

    /// Ctrl+V dans les Vignettes, calque bitmap, après un Ctrl+C de la planche : les pixels reviennent à leur place. Vrai si collé.
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

// ---------------------------------------------------------------------------------------------- Ctrl+A (partie 134)

impl App {
    /// Ctrl+A dans les Vignettes, calque bitmap (partie 134) : comme un lasso autour de toute la planche, les pixels du calque où l'on choisit dans
    /// chaque case et ceux du papier, les traits pris entiers ; rien des autres calques. Au-delà de la limite de la sélection, rien n'est pris
    /// et la barre d'état le dit.
    pub(crate) fn selb_tout(&mut self) {
        if !self.selb_possible() {
            return;
        }
        self.end_stroke();
        let layout = self.board_layout();
        let poly = crate::vignettesel::lasso_de_toute_la_planche();
        let pris = self.selb_prendre(&layout, &poly, None, None);
        self.vsel = None;
        let (pieces, liens) = match pris {
            Ok(t) => t,
            Err(Echec::Limite) => {
                self.vignettes.selb = None;
                self.status = "Tout sélectionner : la planche entière dépasse ce que la sélection de pixels peut prendre ; rien n'est pris (un lasso plus petit).".into();
                return;
            }
            Err(_) => {
                self.vignettes.selb = None;
                self.status = "Tout sélectionner : un trait a changé depuis qu'il a été tracé, on ne sait plus le suivre ; rien n'est pris.".into();
                return;
            }
        };
        if pieces.is_empty() {
            self.vignettes.selb = None;
            self.status = "Tout sélectionner : rien sur ce calque ni sur le papier.".into();
            return;
        }
        let serial = self.history.serial();
        let bornes = Self::bornes_des(&pieces);
        let cases = pieces.iter().filter(|p| matches!(p.source, Source::Dessin { .. })).count();
        let planche = pieces.len() - cases;
        self.vignettes.selb = Some(SelBitmap { pieces, liens, serial, bornes, planche: cle_planche(&layout), ..SelBitmap::default() });
        let s = |k: usize| if k > 1 { "s" } else { "" };
        let note = self.tout_note_calques();
        self.status = format!("Tout sélectionné : les pixels de {cases} dessin{}{} : Ctrl + glisser dans la boîte pour déplacer.{note}", s(cases), if planche > 0 { " et du papier" } else { "" });
    }
}
