//! Dessiner partout sur la planche des Vignettes (Anthony, 2026-10-30) : « tout le canevas est dessinable, mais seul ce qui est dans le champ
//! d'une case (intérieur et limite extérieure) est défini comme appartenant à un plan ».
//!
//! # Ce que c'est
//!
//! - **Le champ d'une case** : son image plus le débord réglé dans Préférences > Vignettes (12 % de sa largeur d'origine) ; pour la case d'un
//!   mouvement de caméra, la tuile de son empreinte. Ce qui y tombe appartient au plan de la case, comme avant. **L'intérieur du cadre d'une
//!   case a toujours priorité sur la marge d'une autre** (partie 111) ; la marge d'une case se voit sur le papier de la planche, sous les
//!   images des autres cases (`App::board_marges`).
//! - **Un trait peut partir de n'importe où** (entre les cases, autour, sous une barre) : s'il part hors de tout champ, il se trace dans le
//!   dessin de la case la plus proche (la même mécanique que le trait qui part de la bande de note, `Vignettes::appui_bande`) ; l'aperçu du
//!   geste est celui d'avant (le trait déborde, `debord.rs`).
//! - **Au lâcher** (jamais pendant le geste : aucune latence ajoutée), le trait est **réparti** (`App::horschamp_partager`) :
//!   - la partie dans le champ de la case où il a été tracé reste dans son dessin ;
//!   - la partie dans le champ d'une autre case (deux cases voisines) va dans le dessin de cette case, sur le calque de même sorte (sinon
//!     la partie reste sur la planche, voir plus bas) ; quand deux **marges** se recouvrent, le point va à la case où le trait a commencé, sinon
//!     à la première dans l'ordre de lecture : jamais deux fois ;
//!   - **le reste est sur la planche, dans le calque « Hors cadres »** (`Annotations::calque_hors_cadres`, un calque d'annotation de la
//!     planche marqué `planche`) : visible, enregistré, relu, dans l'export de la planche, et dans aucun plan, ni l'animatique, ni le film, ni le
//!     storyboard en PDF. Un trait de ce calque garde la recette de son pinceau (`horschamptex.rs`) : il a la texture du pinceau, comme dans
//!     une case (la ligne vectorielle ne sert que le temps que le rendu arrive, et aux fichiers d'avant).
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

use eframe::egui::{pos2, vec2, Color32, Pos2, Rect, Vec2};

use crate::app::App;
use crate::board::BoardLayout;
use crate::document::{CelId, LayerKind};
use crate::history::Edit;
use crate::horschamptex::Recette;
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
    /// Un calque bitmap : les tuiles d'avant le trait, et son tracé (ligne médiane, rayon), sa couleur, son opacité ; la recette du pinceau
    /// (son réglage, et l'appui et l'instant de chaque point du tracé, vides s'ils ne le suivent pas) pour que la part qui sort de la case
    /// garde la texture du pinceau sur la planche (`horschamptex.rs`).
    Bitmap { avant: Vec<(usize, Option<Tile>)>, path: Vec<VPoint>, couleur: [u8; 3], alpha: f32, recette: Option<Recette>, appui: Vec<f32>, instants: Vec<f64> },
}

#[derive(Debug)]
pub(crate) struct Fini {
    pub suivi: Suivi,
    pub cel: CelId,
    pub matiere: Matiere,
}

/// Un tronçon du trait qui reste sur la planche : ses points (planche, rayon compris), et l'appui et l'instant de chacun (vides s'ils ne
/// suivent pas les points).
#[derive(Clone, Debug)]
pub(crate) struct Course {
    pub points: Vec<VPoint>,
    pub appui: Vec<f32>,
    pub instants: Vec<f64>,
}

impl Course {
    /// Le tronçon `run`, `a` : unités de la planche par pixel du dessin ; `suit` : les appuis et les instants sont ceux du tracé.
    pub fn de(run: &[Pt], a: f32, suit: bool) -> Course {
        Course {
            points: run.iter().map(|p| VPoint { x: p.board.x, y: p.board.y, r: p.layer.r * a }).collect(),
            appui: if suit { run.iter().map(|p| p.pressure).collect() } else { Vec::new() },
            instants: if suit { run.iter().map(|p| p.time).collect() } else { Vec::new() },
        }
    }
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

/// Le point `p` de la planche est-il dans l'image de la case `k` (son cadre ; l'enveloppe de ses cadres pour un mouvement de caméra) ?
pub(crate) fn dans_image(layout: &BoardLayout, k: usize, p: Pos2) -> bool {
    let Some(c) = layout.cards.get(k) else { return false };
    c.thumb.contains(p) || layout.move_of(k).is_some_and(|m| crate::vignettecam::dans_polygone(&m.enveloppe, p))
}

/// La case à qui revient le point `p` : **l'intérieur du cadre d'une case a toujours priorité sur la marge d'une autre** (Anthony, 2026-11-06 :
/// un débord de 12 % dépasse l'écart entre deux rangées, le haut d'une case tombait dans la marge de celle du dessus et ne se voyait plus).
/// Les règles d'ordre de `proprietaire` ne jouent qu'entre deux marges.
pub(crate) fn proprietaire_cadre(layout: &BoardLayout, champs: &[Rect], recoit: &[bool], origine: usize, p: Pos2) -> Option<usize> {
    if dans_image(layout, origine, p) {
        return Some(origine);
    }
    if let Some(k) = (0..layout.cards.len()).find(|k| *k != origine && recoit.get(*k).copied().unwrap_or(false) && dans_image(layout, *k, p)) {
        return Some(k);
    }
    proprietaire(champs, recoit, origine, p)
}

/// Comment le dessin de la case `k` se pose sur la planche : rien si sa caméra n'est pas droite (bascule, panoramique, roulis).
pub(crate) fn aff_de_la_case(doc: &crate::document::Document, layout: &BoardLayout, k: usize) -> Option<Aff> {
    let c = layout.cards.get(k)?;
    if let Some(m) = layout.move_of(k) {
        let (z, r) = (&m.tuile.zone, m.tuile.rect);
        return Some(Aff { a: z.echelle, b: 0.0, c: 0.0, d: z.echelle, tx: r.center().x - z.centre.x * z.echelle, ty: r.center().y - z.centre.y * z.echelle });
    }
    aff_de_camera(&doc.panel_camera(c.panel, c.frame), doc.frame, c.thumb)
}

/// Comment la feuille se pose sur la planche quand la caméra `cam`, de cadre `frame`, remplit le rectangle `thumb` : rien si elle n'est pas droite.
pub(crate) fn aff_de_camera(cam: &crate::camera::Camera, frame: [usize; 2], thumb: Rect) -> Option<Aff> {
    if !cam.is_level() || cam.angle != 0.0 {
        return None;
    }
    let (s0, s1) = (cam.to_stage(frame, 0.0, 0.0), cam.to_stage(frame, frame[0] as f32, 0.0));
    let par_px = (s1.x - s0.x) / frame[0].max(1) as f32;
    if !par_px.is_finite() || par_px <= 1e-6 {
        return None;
    }
    let e = thumb.width() / frame[0].max(1) as f32 / par_px;
    Some(Aff { a: e, b: 0.0, c: 0.0, d: e, tx: thumb.min.x - s0.x * e, ty: thumb.min.y - s0.y * e })
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
        let mut textures = self.vignettes.hc_tex.borrow_mut();
        if a.id_hors_cadres().is_none() {
            if textures.nombre() > 0 {
                *textures = Default::default();
            }
            return;
        }
        let opacite = f32::from(self.settings.vignettes.opacite_hors_champ.min(100)) / 100.0;
        let teinte = Color32::from_white_alpha((opacite * 255.0).round() as u8);
        textures.debut();
        for i in a.ordre_de_peinture() {
            let t = &a.traces[i];
            if !a.calque(t.calque).is_some_and(|c| c.planche) {
                continue;
            }
            // Un trait au pinceau : ses pixels rendus par le moteur (sur un fil de fond), peints par blocs ; tant qu'ils ne sont pas prêts (ou
            // si le rendu est perdu), la ligne vectorielle d'avant. Un trait qu'on ne voit pas ne coûte qu'un rectangle.
            if let Some(b) = t.brosse.as_ref().filter(|b| !b.perdu()) {
                let Some((_, case)) = crate::annotations::case_de(layout, &t.anchor) else { continue };
                let k = case.width();
                let rel = t.points.iter().fold(Rect::NOTHING, |r, p| r.union(Rect::from_center_size(pos2(p.x, p.y), Vec2::splat(p.r * 2.0))));
                let zone = Rect::from_min_max(case.min + rel.min.to_vec2() * k, case.min + rel.max.to_vec2() * k);
                if !xf.rect(zone).expand(8.0).intersects(view) {
                    continue;
                }
                if textures.peindre(painter, view, xf, b, &t.points, case, teinte) {
                    continue;
                }
                b.demander(&t.points, t.color, painter.ctx());
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
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn horschamp_noter_bitmap(&mut self, cel: CelId, edit: &Edit, path: Vec<VPoint>, paint: Option<([u8; 3], f32)>, recette: Option<Recette>, appui: Vec<f32>, instants: Vec<f64>) {
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
        // (Les appuis et les instants ne valent que s'ils suivent le tracé point par point.)
        let suit = appui.len() == path.len() && instants.len() == path.len();
        let (appui, instants) = if suit { (appui, instants) } else { (Vec::new(), Vec::new()) };
        self.vignettes.fini = Some(Fini { suivi, cel, matiere: Matiere::Bitmap { avant: tiles.clone(), path, couleur, alpha, recette, appui, instants } });
    }

    /// La recette du pinceau d'un trait bitmap qui finit, avec l'appui et l'instant de chaque point de son tracé : rien hors de la planche
    /// des Vignettes (un trait ordinaire ne coûte rien de plus). Le trait vectoriel d'un plan se rejoue ainsi : sans mélange de couleurs (il
    /// lit des pixels), sans pixels crénelés.
    pub(crate) fn horschamp_recette(&self, stroke: &crate::brush::StrokeState) -> (Option<Recette>, Vec<f32>, Vec<f64>) {
        if !self.board_shown() || self.vignettes.suivi.is_none() {
            return (None, Vec::new(), Vec::new());
        }
        let mut s = stroke.settings();
        s.mix_off = true;
        s.aliased = false;
        let recette = Recette { reglage: self.brushes.for_vector(&s), opacite: s.opacity, durete: s.hardness, toucher: Some(stroke.feel()), couleur_b: stroke.secondary_color() };
        (Some(recette), stroke.path_pressure.clone(), stroke.path_time.clone())
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
        let Some(mut fini) = self.vignettes.fini.take() else { return false };
        let Some(origine) = layout.cards.iter().position(|c| c.id == fini.suivi.carte) else { return false };
        let Some(a) = fini.suivi.aff.echelle() else { return false };
        let champs = champs(layout, self.settings.vignettes.debord_fraction());
        // Une part dans le champ d'une case vide : son plan se crée d'abord (comme dessiner dedans), puis la répartition se fait sur la planche
        // d'après, où cette case est une vraie case.
        let nouvelle = self.horschamp_creer_cases(layout, &mut fini, origine, &champs);
        let cree = nouvelle.is_some();
        let (layout, origine) = match &nouvelle {
            Some(l) => match l.cards.iter().position(|c| c.id == fini.suivi.carte) {
                Some(o) => (l, o),
                None => return true,
            },
            None => (layout, origine),
        };
        let champs = if cree { self::champs(layout, self.settings.vignettes.debord_fraction()) } else { champs };
        let fait = match &fini.matiere {
            Matiere::Vecteur => self.partager_vecteur(layout, &fini, origine, a, &champs),
            Matiere::Bitmap { .. } => self.partager_bitmap(layout, &fini, origine, a, &champs),
        };
        let fait = fait || cree;
        if fait {
            let n = self.history.undo_len().saturating_sub(fini.suivi.base_len);
            if n >= 2 {
                self.history.merge_last(n);
            }
            self.modified = true;
            self.renderer.invalidate_all();
            // Le trait tel qu'il a été tracé reste peint jusqu'à ce que les dessins et les vignettes qui le reçoivent soient prêts, en fond : rien
            // ne change au lâcher (`boardui.rs`, `Vignettes::debord_tenu`).
            self.vignettes.debord_tenu = self.vignettes.debord_garde.take().map(|d| crate::debord::Tenu { debord: d, xf: self.vignettes.xf, etapes: self.history.undo_len(), debut: std::time::Instant::now() });
            self.ctx.request_repaint();
        } else {
            self.vignettes.debord_garde = None;
        }
        fait
    }

    /// Les points du trait fini sur la planche (rien s'il n'y a rien à répartir : trait « sous les autres », dessin disparu).
    fn points_du_trait(&self, fini: &Fini) -> Vec<Pos2> {
        let aff = fini.suivi.aff;
        match &fini.matiere {
            Matiere::Vecteur => {
                let Some(d) = self.drawing(fini.cel) else { return Vec::new() };
                if d.elements.len() <= fini.suivi.elements_avant || self.paint_behind {
                    return Vec::new();
                }
                match d.elements.last() {
                    Some(Element::Stroke { points, .. }) => points.iter().map(|p| aff.applique(p.pos())).collect(),
                    _ => Vec::new(),
                }
            }
            Matiere::Bitmap { path, .. } => path.iter().map(|p| aff.applique(p.pos())).collect(),
        }
    }

    /// Crée les plans des cases vides dont le champ reçoit une part du trait (Anthony, 2026-11-05 : « une partie qui tombe dans le champ
    /// d'une case vide crée le plan de cette case »), comme le fait un trait tracé dans une case vide. Les cases vides d'avant celle-ci
    /// deviennent des plans vides, comme toujours. Le plan courant, le calque et l'image courants sont rendus tels qu'ils étaient ; la
    /// création rejoint l'étape d'annulation du trait. Rend la planche d'après si des plans sont nés.
    fn horschamp_creer_cases(&mut self, layout: &BoardLayout, fini: &mut Fini, origine: usize, champs: &[Rect]) -> Option<BoardLayout> {
        if layout.empty.is_empty() || !self.tool_draws_in_empty() || self.storyboard_refuses() {
            return None;
        }
        let o = layout.cards[origine];
        // (Un plan neuf n'a qu'un calque vectoriel : un trait bitmap n'y entrerait pas, il ne crée pas de plan vide.)
        if !matches!(&fini.matiere, Matiere::Vecteur) {
            return None;
        }
        // (Un plan neuf a la caméra d'origine : une autre caméra, la part resterait sur la planche.)
        if self.doc.panel_camera(o.panel, o.frame) != self.doc.default_camera() || layout.move_of(origine).is_some() {
            return None;
        }
        let debord = self.settings.vignettes.debord_fraction();
        let recoit = vec![true; champs.len()];
        let pts = self.points_du_trait(fini);
        // Un bout de trait ne crée que **la case suivante** (la première case pâle de sa séquence), jamais une case plus loin : un bout
        // tombé dans la marge d'une case pâle lointaine faisait naître plusieurs plans vides devant elle (partie 113, décision 8). Un bout
        // plus loin reste sur la planche, visible, dans « Hors cadres ». Dessiner **dans** une case pâle lointaine garde sa place, comme avant.
        let mut premieres: Vec<usize> = layout.groupes.iter().filter(|g| g.vides.start < g.vides.end).map(|g| g.vides.start).collect();
        if premieres.is_empty() {
            premieres.push(0);
        }
        let mut touchee: Option<usize> = None;
        for p in &pts {
            if proprietaire(champs, &recoit, origine, *p).is_some() {
                continue;
            }
            if let Some(e) = layout.empty.iter().position(|r| r.expand(debord * r.width()).contains(*p)) {
                if premieres.contains(&e) {
                    touchee = Some(e);
                }
            }
        }
        let e = touchee?;
        let (cur_id, cur_frame, cur_layer) = (self.doc.panels[self.cur_panel].id, self.cur_frame, self.cur_layer);
        let (created, joins) = (self.vignettes.created, std::mem::take(&mut self.vignettes.empty_joins));
        let ok = self.board_create_from_empty(e);
        self.vignettes.created = created;
        self.vignettes.empty_joins = joins;
        if !ok {
            return None;
        }
        if let Some(i) = self.doc.panels.iter().position(|p| p.id == cur_id) {
            self.cur_panel = i;
            self.cur_frame = cur_frame.min(self.frame_count().saturating_sub(1));
            self.cur_layer = cur_layer.min(self.layers().len().saturating_sub(1));
            self.board_sel = vec![cur_id];
        }
        let neuve = self.board_layout();
        // Les cases d'après ont pu glisser : le dessin suit la case où il a été tracé.
        let apres = neuve.cards.iter().find(|c| c.id == fini.suivi.carte)?;
        let d = apres.thumb.min - o.thumb.min;
        fini.suivi.aff.tx += d.x;
        fini.suivi.aff.ty += d.y;
        Some(neuve)
    }

    /// Les cases qui peuvent recevoir une partie du trait tracé dans la case `origine`, un plan à mouvement de caméra compris : un dessin posé
    /// sur la planche à la même échelle (`a`, une caméra droite), un calque de même sorte que celui où l'on dessine, ni verrouillé ni masqué.
    fn cases_qui_recoivent(&self, layout: &BoardLayout, origine: usize, genre: LayerKind, a: f32) -> Vec<Option<usize>> {
        layout
            .cards
            .iter()
            .enumerate()
            .map(|(k, c)| {
                if k == origine {
                    return Some(self.cur_layer);
                }
                let e = aff_de_la_case(&self.doc, layout, k)?.echelle()?;
                if (e - a).abs() > a * 0.005 {
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
        let cibles = self.cases_qui_recoivent(layout, origine, LayerKind::Vector, a);
        let recoit: Vec<bool> = cibles.iter().map(Option::is_some).collect();
        let owner = |p: Pos2| proprietaire_cadre(layout, champs, &recoit, origine, p);
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
        let mut sur_la_planche: Vec<Course> = Vec::new();
        let mut chez_les_autres: Vec<(usize, Vec<Element>)> = Vec::new();
        let mut chez_elle: Vec<Element> = Vec::new();
        for (qui, run) in &runs {
            match qui {
                Some(k) if *k == origine => chez_elle.push(element(run, Vec2::ZERO)),
                Some(k) => {
                    // (Le dessin de la case voisine se pose ailleurs sur la planche : le même point de la planche, dans son dessin à elle.)
                    let Some(cible) = aff_de_la_case(&self.doc, layout, *k) else { continue };
                    let el = element(run, vec2((aff.tx - cible.tx) / a, (aff.ty - cible.ty) / a));
                    match chez_les_autres.iter_mut().find(|(c, _)| c == k) {
                        Some((_, v)) => v.push(el),
                        None => chez_les_autres.push((*k, vec![el])),
                    }
                }
                None => sur_la_planche.push(Course::de(run, a, aligne)),
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
        let mut regions: Vec<(CelId, Rect)> = Vec::new();
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
            regions.push((cible, region_k));
        }
        regions.push((cel, region));
        // Aucun rejeu du moteur sur le fil de l'interface : un trait au pinceau se refait en arrière-plan (`rejeu.rs`). En attendant, le dessin
        // d'origine garde ses pixels et la planche garde le trait tel qu'il a été tracé (`Vignettes::debord_tenu`). (Avant : chaque tronçon,
        // sans cache, était rejoué ici, au lâcher.)
        for (c, r) in regions {
            if brush.is_some() {
                if let Some(d) = self.doc.cels.get_mut(&c) {
                    d.version += 1;
                }
                self.rejeu_cel(c);
            } else {
                self.refresh_cel_region(c, r.expand(2.0));
            }
        }
        for (cel, drawing) in avant {
            self.history.push(Edit::Vector { cel, drawing });
        }
        // (La part sur la planche garde la recette du pinceau : même réglage, même opacité, mêmes appuis que dans la case.)
        let recette = Recette { reglage: brush.as_deref().copied(), opacite: opacity, durete: hardness, toucher: trace.feel, couleur_b: trace.secondary };
        let planche = self.poser_hors_cadres(layout, sur_la_planche, color, (opacity.clamp(0.0, 1.0) * 255.0).round() as u8, Some(recette), a);
        // La structure (calques créés, images de dessin, planche) : l'état d'avant, rangé dans l'étape du trait.
        self.history.push(snap);
        let _ = planche;
        true
    }

    /// Pose sur la planche, dans le calque « Hors cadres », les tracés `traces` (points de la planche, rayon compris).
    /// `recette` : le pinceau du trait, qui donne sa texture à la part posée sur la planche ; `a` : unités de la planche par pixel du dessin.
    fn poser_hors_cadres(&mut self, layout: &BoardLayout, traces: Vec<Course>, couleur: [u8; 3], alpha: u8, recette: Option<Recette>, a: f32) -> usize {
        let mut n = 0;
        for t in traces {
            let brosse = recette.map(|r| (r, t.appui, t.instants, a));
            if self.doc.annotations.ajouter_hors_cadres_brosse(layout, &t.points, couleur, alpha, brosse) {
                n += 1;
            }
        }
        n
    }

    fn partager_bitmap(&mut self, layout: &BoardLayout, fini: &Fini, origine: usize, a: f32, champs: &[Rect]) -> bool {
        let Matiere::Bitmap { avant, path, couleur, alpha, recette, appui, instants } = &fini.matiere else { return false };
        let aff = fini.suivi.aff;
        let cibles = self.cases_qui_recoivent(layout, origine, LayerKind::Bitmap, a);
        let recoit: Vec<bool> = cibles.iter().map(Option::is_some).collect();
        let suit = appui.len() == path.len() && instants.len() == path.len();
        let pts: Vec<Pt> = path
            .iter()
            .enumerate()
            .map(|(i, p)| Pt { layer: *p, board: aff.applique(p.pos()), pressure: if suit { appui[i] } else { 1.0 }, time: if suit { instants[i] } else { 0.0 } })
            .collect();
        let owner = |p: Pos2| proprietaire_cadre(layout, champs, &recoit, origine, p);
        // Tout le trait à la case, rayon compris (son cadre, ou sa marge là où aucun autre cadre n'est) : rien à répartir.
        let sur = |p: &Pt| [vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(-1.0, 1.0), vec2(1.0, 1.0)].iter().all(|d| owner(p.board + *d * ((p.layer.r + 1.5) * a)) == Some(origine));
        if pts.iter().all(sur) {
            return false;
        }
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
                        let k = j * crate::raster::TILE + i;
                        let apres_px = apres[k];
                        let avant_px = avant_tuile.as_ref().map_or(eframe::egui::Color32::TRANSPARENT, |t| t[k]);
                        // (Seuls les pixels que le trait a changés coûtent le calcul de leur propriétaire.)
                        if apres_px == avant_px {
                            continue;
                        }
                        let bord = aff.applique(pos2(x as f32 + 0.5, y as f32 + 0.5));
                        let qui = owner(bord);
                        if qui == Some(origine) {
                            continue;
                        }
                        // Un pixel neuf (rien dessous) peut passer à la case voisine ; un pixel qui recouvrait du dessin ne passe nulle part.
                        if avant_px.a() == 0 && apres_px.a() > 0 {
                            if let Some(q) = qui {
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
            let Some(aff_q) = aff_de_la_case(&self.doc, layout, q) else { continue };
            let (dx, dy) = (((aff.tx - aff_q.tx) / a).round() as i64, ((aff.ty - aff_q.ty) / a).round() as i64);
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
        let runs = trancher(&pts, &owner);
        let traces: Vec<Course> = runs.into_iter().filter(|(q, _)| q.is_none()).map(|(_, run)| Course::de(&run, a, suit)).collect();
        self.poser_hors_cadres(layout, traces, *couleur, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8, *recette, a);
        self.history.push(snap);
        true
    }
}
