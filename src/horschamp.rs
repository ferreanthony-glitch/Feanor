//! Dessiner partout sur la planche des Vignettes (Anthony, 2026-10-30) : « tout le canevas est dessinable, mais seul ce qui est dans le champ
//! d'une case (intérieur et limite extérieure) est défini comme appartenant à un plan ».
//!
//! # Ce que c'est
//!
//! - **Le champ d'une case** : son image plus le débord réglé dans Préférences > Vignettes (12 % de sa largeur d'origine) ; pour la case d'un
//!   mouvement de caméra, la tuile de son empreinte. Ce qui y tombe appartient au plan de la case, comme avant.
//! - **Un trait peut partir de n'importe où** (entre les cases, autour, sous une barre) : s'il part hors de tout champ, il se trace dans le
//!   dessin de la case la plus proche (la même mécanique que le trait qui part de la bande de note, `Vignettes::appui_bande`) ; l'aperçu du
//!   geste est celui d'avant (le trait déborde, pâle, `debord.rs`).
//! - **Au lâcher** (jamais pendant le geste : aucune latence ajoutée), le trait est **réparti** (`App::horschamp_partager`) :
//!   - la partie dans le champ de la case où il a été tracé reste dans son dessin ;
//!   - la partie dans le champ d'une autre case (deux cases voisines) va dans le dessin de cette case, sur le calque de même sorte (sinon
//!     la partie reste sur la planche, voir plus bas) ; quand deux champs se recouvrent (leurs marges), le point va à la case où le trait a
//!     commencé, sinon à la première dans l'ordre de lecture : jamais deux fois ;
//!   - **le reste est sur la planche, dans le calque « Hors cadres »** (`Annotations::calque_hors_cadres`, un calque d'annotation de la
//!     planche marqué `planche`) : visible, enregistré, relu, dans l'export de la planche, et dans aucun plan, ni l'animatique, ni le film, ni le
//!     storyboard en PDF. Un trait de ce calque est un trait vectoriel (couleur, opacité, épaisseur du trait d'origine, sans la texture du pinceau).
//!   - **Une seule étape d'annulation** pour tout le trait.
//! - Un trait qui reste dans le champ de sa case ne coûte rien de plus (un parcours de ses points).
//!
//! # Les outils
//!
//! - **Pinceau, crayon, ligne, rectangle, ellipse, courbe, polyligne** : partout. Un calque vectoriel est coupé en tronçons (chacun garde sa
//!   pression et ses instants) ; un calque bitmap est coupé au pixel (ce qui sort de la case est rendu au dessin tel qu'il était, passé à la case
//!   voisine ou à la planche). Les formes d'un calque bitmap ne sont pas réparties (elles restent dans la case, comme avant).
//! - **Gomme** : dans une case, elle agit sur le plan, dans son champ. Partie hors de toute case, elle efface les traits du calque « Hors cadres »
//!   qu'elle touche (le trait entier, comme la gomme d'annotation). **Pot de peinture** : seulement dans une case (une zone fermée de la feuille).
//!   **Sélection, lasso, texte** : inchangés (les notes de la planche et le lasso d'annotation).

use eframe::egui::{pos2, vec2, Pos2, Rect, Vec2};

use crate::app::App;
use crate::board::BoardLayout;
use crate::document::{CelId, LayerKind};
use crate::history::Edit;
use crate::raster::Tile;
use crate::vector::{Element, Trace as VTrace, VPoint};

/// Le passage du dessin du calque courant à la planche : `planche = (a·x + c·y + tx, b·x + d·y + ty)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Aff {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Aff {
    pub fn applique(&self, p: Pos2) -> Pos2 {
        pos2(self.a * p.x + self.c * p.y + self.tx, self.b * p.x + self.d * p.y + self.ty)
    }

    /// L'échelle si c'est une mise à l'échelle et un décalage seulement (sans rotation ni inclinaison).
    pub fn echelle(&self) -> Option<f32> {
        let s = self.a;
        (s.is_finite() && s > 1e-6 && (self.d - s).abs() < s * 1e-3 && self.b.abs() < s * 1e-3 && self.c.abs() < s * 1e-3).then_some(s)
    }
}

/// Le geste en cours dans la case vive : où il est, comment le dessin se pose sur la planche, d'où il part (`horschamp_suivre`).
#[derive(Clone, Debug)]
pub(crate) struct Suivi {
    pub carte: u64,
    pub aff: Aff,
    /// Le nombre d'étapes d'annulation avant le geste.
    pub base_len: usize,
    /// Le nombre d'éléments du dessin courant avant le geste (calque vectoriel).
    pub elements_avant: usize,
}

/// Ce que le geste a laissé, noté à son lâcher pour être réparti à la fin de l'image d'écran.
#[derive(Debug)]
pub(crate) enum Matiere {
    /// Un calque vectoriel : le dernier élément du dessin.
    Vecteur,
    /// Un calque bitmap : les tuiles d'avant le trait, et son tracé (ligne médiane, rayon), sa couleur, son opacité.
    Bitmap { avant: Vec<(usize, Option<Tile>)>, path: Vec<VPoint>, couleur: [u8; 3], alpha: f32 },
}

#[derive(Debug)]
pub(crate) struct Fini {
    pub suivi: Suivi,
    pub cel: CelId,
    pub matiere: Matiere,
}

/// Le champ de chaque case de la planche (même rang que `layout.cards`) : son image, ou la tuile de son empreinte, plus le débord.
pub(crate) fn champs(layout: &BoardLayout, debord: f32) -> Vec<Rect> {
    layout
        .cards
        .iter()
        .enumerate()
        .map(|(k, c)| {
            let base = layout.move_of(k).map_or(c.thumb, |m| m.tuile.rect);
            base.expand(debord * c.thumb.width())
        })
        .collect()
}

/// La case à qui appartient le point `p` de la planche : celle d'où le trait est parti (`origine`) si le point est dans son champ, sinon la
/// première, dans l'ordre de lecture, dont le champ le contient et qui peut recevoir le trait (`recoit`). Rien : hors de tout champ.
pub(crate) fn proprietaire(champs: &[Rect], recoit: &[bool], origine: usize, p: Pos2) -> Option<usize> {
    if champs.get(origine).is_some_and(|r| r.contains(p)) {
        return Some(origine);
    }
    champs.iter().enumerate().find(|(k, r)| *k != origine && recoit.get(*k).copied().unwrap_or(false) && r.contains(p)).map(|(k, _)| k)
}

/// Un point d'un tracé : son point de dessin, sa place sur la planche, son appui, son instant.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Pt {
    pub layer: VPoint,
    pub board: Pos2,
    pub pressure: f32,
    pub time: f64,
}

fn entre(a: &Pt, b: &Pt, t: f32) -> Pt {
    let l = |x: f32, y: f32| x + (y - x) * t;
    Pt {
        layer: VPoint { x: l(a.layer.x, b.layer.x), y: l(a.layer.y, b.layer.y), r: l(a.layer.r, b.layer.r) },
        board: pos2(l(a.board.x, b.board.x), l(a.board.y, b.board.y)),
        pressure: l(a.pressure, b.pressure),
        time: a.time + (b.time - a.time) * f64::from(t),
    }
}

/// Coupe le tracé `pts` en tronçons de même propriétaire : (le propriétaire, ses points). Un segment qui change de propriétaire est coupé
/// à la limite (à un millième de segment près). Les tronçons se suivent ; aucun point n'est perdu.
pub(crate) fn trancher(pts: &[Pt], owner: &dyn Fn(Pos2) -> Option<usize>) -> Vec<(Option<usize>, Vec<Pt>)> {
    let mut out: Vec<(Option<usize>, Vec<Pt>)> = Vec::new();
    let mut prev: Option<(Pt, Option<usize>)> = None;
    for p in pts {
        let o = owner(p.board);
        if let Some((a, oa)) = prev {
            if oa != o {
                // La limite : la dichotomie entre le dernier point du tronçon et celui-ci.
                let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
                for _ in 0..12 {
                    let mid = (lo + hi) * 0.5;
                    if owner(entre(&a, p, mid).board) == oa {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                if lo > 0.0 {
                    if let Some(l) = out.last_mut() {
                        l.1.push(entre(&a, p, lo));
                    }
                }
                out.push((o, vec![entre(&a, p, hi.min(1.0))]));
                if hi < 1.0 {
                    out.last_mut().expect("un tronçon").1.push(*p);
                }
            } else if let Some(l) = out.last_mut() {
                l.1.push(*p);
            }
        } else {
            out.push((o, vec![*p]));
        }
        prev = Some((*p, o));
    }
    out
}

impl App {
    /// Le dessin hors des cases, peint sous les cases : calque « Hors cadres » (s'il est montré), à l'opacité du réglage.
    pub(crate) fn hors_cadres_peindre(&self, painter: &eframe::egui::Painter, view: Rect, xf: crate::boardui::Xf, layout: &BoardLayout) {
        let a = &self.doc.annotations;
        if a.id_hors_cadres().is_none() {
            return;
        }
        let opacite = f32::from(self.settings.vignettes.opacite_hors_champ.min(100)) / 100.0;
        for i in a.ordre_de_peinture() {
            let t = &a.traces[i];
            if !a.calque(t.calque).is_some_and(|c| c.planche) {
                continue;
            }
            let Some(points) = a.points_planche(layout, i) else { continue };
            crate::boardui::paint_note_a(painter, view, xf, &points, t.color, (f32::from(t.alpha) * opacite).round() as u8);
        }
    }

    /// Le geste qui dure dans la case vive `carte` : comment son dessin se pose sur la planche. À appeler pendant que la vue du dessin est
    /// celle de la case (`card_canvas`).
    pub(crate) fn horschamp_suivre(&mut self, carte: u64) {
        if self.stroke.is_none() && self.shape_drag.is_none() && self.shape_edit.is_none() {
            return;
        }
        let xf = self.vignettes.xf;
        let (p0, p1, p2) = (xf.to_board(self.layer_to_screen(pos2(0.0, 0.0))), xf.to_board(self.layer_to_screen(pos2(1.0, 0.0))), xf.to_board(self.layer_to_screen(pos2(0.0, 1.0))));
        let aff = Aff { a: p1.x - p0.x, b: p1.y - p0.y, c: p2.x - p0.x, d: p2.y - p0.y, tx: p0.x, ty: p0.y };
        let deja = self.vignettes.suivi.as_ref().filter(|s| s.carte == carte);
        let (base_len, elements_avant) = match deja {
            Some(s) => (s.base_len, s.elements_avant),
            None => (self.history.undo_len(), self.cel_here().and_then(|c| self.drawing(c)).map_or(0, |d| d.elements.len())),
        };
        self.vignettes.suivi = Some(Suivi { carte, aff, base_len, elements_avant });
    }

    /// Le trait bitmap qui vient de finir (`end_stroke`) : ce qu'il faut pour le répartir à la fin de l'image d'écran.
    pub(crate) fn horschamp_noter_bitmap(&mut self, cel: CelId, edit: &Edit, path: Vec<VPoint>, paint: Option<([u8; 3], f32)>) {
        // (Le premier trait sur un calque vide crée son dessin : l'étape est alors un groupe, les tuiles d'avant y sont.)
        fn tuiles(e: &Edit) -> Option<&Vec<(usize, Option<Tile>)>> {
            match e {
                Edit::Tiles { tiles, .. } => Some(tiles),
                Edit::Group(v) => v.iter().find_map(tuiles),
                _ => None,
            }
        }
        let (Some(suivi), Some(tiles), Some((couleur, alpha))) = (self.vignettes.suivi.clone(), tuiles(edit), paint) else { return };
        if !self.board_shown() || self.tool == crate::app::Tool::Eraser || path.is_empty() {
            return;
        }
        self.vignettes.fini = Some(Fini { suivi, cel, matiere: Matiere::Bitmap { avant: tiles.clone(), path, couleur, alpha } });
    }

    /// Le trait ou la forme vectoriels qui viennent d'être posés dans le dessin `cel` : à répartir à la fin de l'image d'écran.
    pub(crate) fn horschamp_noter_vecteur(&mut self, cel: CelId) {
        let Some(suivi) = self.vignettes.suivi.clone() else { return };
        if !self.board_shown() {
            return;
        }
        self.vignettes.fini = Some(Fini { suivi, cel, matiere: Matiere::Vecteur });
    }

    /// Répartit le trait qui vient de finir : sa partie dans le champ de sa case reste, celle dans le champ d'une autre case va à son plan, le
    /// reste va sur la planche (« Hors cadres »). Une seule étape d'annulation. Vrai si quelque chose a bougé.
    pub(crate) fn horschamp_partager(&mut self, layout: &BoardLayout) -> bool {
        // (Le suivi dure autant que le geste : son départ ne change pas d'une image à l'autre.)
        if self.stroke.is_none() && self.shape_drag.is_none() && self.shape_edit.is_none() {
            self.vignettes.suivi = None;
        }
        let Some(fini) = self.vignettes.fini.take() else { return false };
        let Some(origine) = layout.cards.iter().position(|c| c.id == fini.suivi.carte) else { return false };
        let Some(a) = fini.suivi.aff.echelle() else { return false };
        let champs = champs(layout, self.settings.vignettes.debord_fraction());
        let fait = match &fini.matiere {
            Matiere::Vecteur => self.partager_vecteur(layout, &fini, origine, a, &champs),
            Matiere::Bitmap { .. } => self.partager_bitmap(layout, &fini, origine, a, &champs),
        };
        if fait {
            let n = self.history.undo_len().saturating_sub(fini.suivi.base_len);
            if n >= 2 {
                self.history.merge_last(n);
            }
            self.modified = true;
            self.renderer.invalidate_all();
            self.vignettes.debord_garde = None;
            self.ctx.request_repaint();
        } else {
            self.vignettes.debord_garde = None;
        }
        fait
    }

    /// Les cases qui peuvent recevoir une partie du trait tracé dans la case `origine` : même caméra à leur image, même largeur de case, un
    /// calque de même sorte que celui où l'on dessine, ni verrouillé ni masqué.
    fn cases_qui_recoivent(&self, layout: &BoardLayout, origine: usize, genre: LayerKind) -> Vec<Option<usize>> {
        let o = &layout.cards[origine];
        layout
            .cards
            .iter()
            .enumerate()
            .map(|(k, c)| {
                if k == origine {
                    return Some(self.cur_layer);
                }
                let meme_camera = self.doc.panel_camera(o.panel, o.frame) == self.doc.panel_camera(c.panel, c.frame);
                let meme_taille = (c.thumb.width() - o.thumb.width()).abs() < 0.01 && (c.thumb.height() - o.thumb.height()).abs() < 0.01;
                if !(meme_camera && meme_taille) || layout.move_of(k).is_some() {
                    return None;
                }
                let calques = &self.doc.panels.get(c.panel)?.layers;
                let voulu = self.layers().get(self.cur_layer)?;
                let rang = calques
                    .iter()
                    .position(|l| l.id == voulu.id)
                    .or_else(|| calques.iter().position(|l| l.name == voulu.name && l.kind == genre))
                    .or_else(|| calques.iter().position(|l| l.kind == genre && !l.folder && l.effet.is_none() && l.video.is_none() && !l.decor && !l.annotation && l.couleur.is_none() && l.scene_wide.is_none()))?;
                let l = &calques[rang];
                (l.kind == genre && !l.locked && !l.folder && l.effet.is_none() && l.video.is_none() && l.pose(c.frame).is_identity()).then_some(rang)
            })
            .collect()
    }

    /// Le dessin de la case `k` sur son calque `rang`, créé au besoin (comme `ensure_plain_cel`).
    fn cel_de_la_case(&mut self, layout: &BoardLayout, k: usize, rang: usize) -> Option<CelId> {
        let c = layout.cards[k];
        let layer = self.doc.panels.get(c.panel)?.layers.get(rang)?;
        if let Some(id) = layer.cel_at(c.frame) {
            return Some(id);
        }
        let kind = layer.kind;
        let id = self.doc.new_cel(kind);
        let limit = self.doc.panels[c.panel].frame_count();
        let (start, end) = crate::images::new_drawing_span(&self.doc.panels[c.panel].layers[rang].frames, c.frame, limit, true, self.default_exposure);
        let layer = &mut self.doc.panels[c.panel].layers[rang];
        for f in start..end {
            layer.set(f, Some(id));
        }
        Some(id)
    }

    fn partager_vecteur(&mut self, layout: &BoardLayout, fini: &Fini, origine: usize, a: f32, champs: &[Rect]) -> bool {
        let cel = fini.cel;
        let Some(d) = self.drawing(cel) else { return false };
        // (Un trait posé sous les autres, « Peint sur l'aplat », n'est pas le dernier élément : il reste où il est.)
        if d.elements.len() <= fini.suivi.elements_avant || self.paint_behind {
            return false;
        }
        let Some(Element::Stroke { points, color, opacity, hardness, brush, trace }) = d.elements.last().cloned() else { return false };
        if points.is_empty() {
            return false;
        }
        let aligne = trace.pressure.len() == points.len() && trace.time.len() == points.len();
        let aff = fini.suivi.aff;
        let pts: Vec<Pt> = points
            .iter()
            .enumerate()
            .map(|(i, p)| Pt { layer: *p, board: aff.applique(p.pos()), pressure: if aligne { trace.pressure[i] } else { 1.0 }, time: if aligne { trace.time[i] } else { 0.0 } })
            .collect();
        let cibles = self.cases_qui_recoivent(layout, origine, LayerKind::Vector);
        let recoit: Vec<bool> = cibles.iter().map(Option::is_some).collect();
        let owner = |p: Pos2| proprietaire(champs, &recoit, origine, p);
        // Tout dans le champ de la case, rayon compris : rien à répartir.
        if pts.iter().all(|p| champs[origine].shrink(p.layer.r * a).contains(p.board)) {
            return false;
        }
        let runs = trancher(&pts, &owner);
        if runs.iter().all(|(o, _)| *o == Some(origine)) {
            return false;
        }
        // Avant de toucher à quoi que ce soit : les états d'avant (la structure, pour les calques et la planche ; les dessins des autres cases).
        let snap = self.snapshot();
        let mut avant: Vec<(CelId, crate::vector::VectorDrawing)> = Vec::new();
        let element = |run: &[Pt], dec: Vec2| Element::Stroke {
            points: run.iter().map(|p| VPoint { x: p.layer.x + dec.x, y: p.layer.y + dec.y, r: p.layer.r }).collect(),
            color,
            opacity,
            hardness,
            brush: brush.clone(),
            trace: if aligne {
                VTrace { pressure: run.iter().map(|p| p.pressure).collect(), time: run.iter().map(|p| p.time).collect(), feel: trace.feel, secondary: trace.secondary, cache: Default::default() }
            } else {
                VTrace::default()
            },
        };
        let o = layout.cards[origine];
        let mut sur_la_planche: Vec<Vec<VPoint>> = Vec::new();
        let mut chez_les_autres: Vec<(usize, Vec<Element>)> = Vec::new();
        let mut chez_elle: Vec<Element> = Vec::new();
        for (qui, run) in &runs {
            match qui {
                Some(k) if *k == origine => chez_elle.push(element(run, Vec2::ZERO)),
                Some(k) => {
                    let s = layout.cards[*k].thumb.min - o.thumb.min;
                    let el = element(run, vec2(-s.x / a, -s.y / a));
                    match chez_les_autres.iter_mut().find(|(c, _)| c == k) {
                        Some((_, v)) => v.push(el),
                        None => chez_les_autres.push((*k, vec![el])),
                    }
                }
                None => sur_la_planche.push(run.iter().map(|p| VPoint { x: p.board.x, y: p.board.y, r: p.layer.r * a }).collect()),
            }
        }
        // Le dessin de la case d'origine : le trait entier est remplacé par ses tronçons du champ.
        let region = {
            let mut r = Element::Stroke { points: points.clone(), color, opacity, hardness, brush: None, trace: VTrace::default() }.bounds();
            for e in &chez_elle {
                r = r.union(e.bounds());
            }
            r
        };
        if let Some(d) = self.drawing_mut(cel) {
            d.elements.pop();
            d.elements.extend(chez_elle);
        }
        let mut a_rejouer = vec![cel];
        for (k, els) in chez_les_autres {
            let Some(rang) = cibles[k] else { continue };
            let Some(cible) = self.cel_de_la_case(layout, k, rang) else { continue };
            if let Some(d) = self.drawing(cible) {
                avant.push((cible, d.clone()));
            }
            let region_k = els.iter().fold(Rect::NOTHING, |r, e| r.union(e.bounds()));
            if let Some(d) = self.drawing_mut(cible) {
                d.elements.extend(els);
            }
            self.refresh_cel_region(cible, region_k.expand(2.0));
            a_rejouer.push(cible);
        }
        self.refresh_cel_region(cel, region.expand(2.0));
        if brush.is_some() {
            for c in a_rejouer {
                self.rejeu_cel(c);
            }
        }
        for (cel, drawing) in avant {
            self.history.push(Edit::Vector { cel, drawing });
        }
        let planche = self.poser_hors_cadres(layout, sur_la_planche, color, (opacity.clamp(0.0, 1.0) * 255.0).round() as u8);
        // La structure (calques créés, images de dessin, planche) : l'état d'avant, rangé dans l'étape du trait.
        self.history.push(snap);
        let _ = planche;
        true
    }

    /// Pose sur la planche, dans le calque « Hors cadres », les tracés `traces` (points de la planche, rayon compris).
    fn poser_hors_cadres(&mut self, layout: &BoardLayout, traces: Vec<Vec<VPoint>>, couleur: [u8; 3], alpha: u8) -> usize {
        let mut n = 0;
        for t in traces {
            if self.doc.annotations.ajouter_hors_cadres(layout, &t, couleur, alpha) {
                n += 1;
            }
        }
        n
    }

    fn partager_bitmap(&mut self, layout: &BoardLayout, fini: &Fini, origine: usize, a: f32, champs: &[Rect]) -> bool {
        let Matiere::Bitmap { avant, path, couleur, alpha } = &fini.matiere else { return false };
        let aff = fini.suivi.aff;
        let cibles = self.cases_qui_recoivent(layout, origine, LayerKind::Bitmap);
        let recoit: Vec<bool> = cibles.iter().map(Option::is_some).collect();
        let pts: Vec<Pt> = path.iter().map(|p| Pt { layer: *p, board: aff.applique(p.pos()), pressure: 1.0, time: 0.0 }).collect();
        // Tout le trait dans le champ, rayon compris : rien à répartir.
        if pts.iter().all(|p| champs[origine].shrink((p.layer.r + 1.5) * a).contains(p.board)) {
            return false;
        }
        let champ_o = champs[origine];
        let o = layout.cards[origine];
        // 1. Le dessin d'origine : ce qui sort du champ est rendu tel qu'il était ; ce qui tombe dans le champ d'une autre case est gardé pour elle.
        let cel = fini.cel;
        let mut passes: Vec<(usize, Vec<(i64, i64, eframe::egui::Color32)>)> = Vec::new();
        let mut change: Vec<usize> = Vec::new();
        {
            let Some(c) = self.doc.cels.get_mut(&cel) else { return false };
            for (id, avant_tuile) in avant {
                let Some(apres) = c.raster.slot(*id).cloned() else { continue };
                let (ox, oy) = c.raster.tile_origin(*id);
                let mut neuve = apres.clone();
                let mut touche = false;
                for j in 0..crate::raster::TILE {
                    for i in 0..crate::raster::TILE {
                        let (x, y) = (ox + i as i64, oy + j as i64);
                        let bord = aff.applique(pos2(x as f32 + 0.5, y as f32 + 0.5));
                        if champ_o.contains(bord) {
                            continue;
                        }
                        let k = j * crate::raster::TILE + i;
                        let apres_px = apres[k];
                        let avant_px = avant_tuile.as_ref().map_or(eframe::egui::Color32::TRANSPARENT, |t| t[k]);
                        if apres_px == avant_px {
                            continue;
                        }
                        // Un pixel neuf (rien dessous) peut passer à la case voisine ; un pixel qui recouvrait du dessin ne passe nulle part.
                        if avant_px.a() == 0 && apres_px.a() > 0 {
                            if let Some(q) = proprietaire(champs, &recoit, origine, bord).filter(|q| *q != origine) {
                                match passes.iter_mut().find(|(c, _)| *c == q) {
                                    Some((_, v)) => v.push((x, y, apres_px)),
                                    None => passes.push((q, vec![(x, y, apres_px)])),
                                }
                            }
                        }
                        neuve[k] = avant_px;
                        touche = true;
                    }
                }
                if touche {
                    c.raster.replace_slot(*id, Some(neuve));
                    change.push(*id);
                }
            }
            c.version += 1;
        }
        self.renderer.invalidate_tiles(&change);
        let snap = self.snapshot();
        // 2. Les autres cases : leurs pixels, posés par-dessus, décalés de la distance entre les deux cases (au pixel près).
        let mut etapes: Vec<Edit> = Vec::new();
        for (q, pixels) in passes {
            let Some(rang) = cibles[q] else { continue };
            let Some(cible) = self.cel_de_la_case(layout, q, rang) else { continue };
            let s = layout.cards[q].thumb.min - o.thumb.min;
            let (dx, dy) = ((-s.x / a).round() as i64, (-s.y / a).round() as i64);
            let Some(c) = self.doc.cels.get_mut(&cible) else { continue };
            let mut vieux: Vec<(usize, Option<Tile>)> = Vec::new();
            for (x, y, px) in pixels {
                let (x, y) = (x + dx, y + dy);
                let (tx, ty) = (x.div_euclid(crate::raster::TILE as i64) as i32, y.div_euclid(crate::raster::TILE as i64) as i32);
                let id = c.raster.id_of(tx, ty);
                if !vieux.iter().any(|(i, _)| *i == id) {
                    vieux.push((id, c.raster.slot(id).cloned()));
                }
                let t = c.raster.tile_mut(id);
                let k = y.rem_euclid(crate::raster::TILE as i64) as usize * crate::raster::TILE + x.rem_euclid(crate::raster::TILE as i64) as usize;
                t[k] = crate::render::over(t[k], px, 255, None);
            }
            c.version += 1;
            etapes.push(Edit::Tiles { cel: cible, tiles: vieux });
        }
        for e in etapes {
            self.history.push(e);
        }
        // 3. Ce qui est hors de tout champ : sur la planche (le tracé, rayon et couleur du trait).
        let runs = trancher(&pts, &|p| proprietaire(champs, &recoit, origine, p));
        let traces: Vec<Vec<VPoint>> = runs
            .into_iter()
            .filter(|(q, _)| q.is_none())
            .map(|(_, run)| run.iter().map(|p| VPoint { x: p.board.x, y: p.board.y, r: p.layer.r * a }).collect())
            .collect();
        self.poser_hors_cadres(layout, traces, *couleur, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8);
        self.history.push(snap);
        true
    }
}
