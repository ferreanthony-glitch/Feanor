//! Les bords des cases ne coupent jamais le dessin (partie 125, Anthony, 2026-11-08 : « quand je sélectionne des traits dans un cadre et que je les
//! déplace ou les transforme, seule la partie dans le champ du cadre est prise, le reste est coupé » ; « pour moi les caméras doivent être le moins
//! intrusives et destructives possibles dans le dessin »).
//!
//! # Ce que c'est
//!
//! - **La sélection de la planche** : dans les Vignettes, sur un calque vectoriel, **Ctrl + glisser** trace un lasso **n'importe où** : dans une case,
//!   sur le papier autour et entre les cases, ou à cheval. **Ctrl + clic** prend le trait sous le pointeur. Le lasso est sur la planche, pas dans le
//!   dessin d'une case : il ne s'arrête à aucun bord.
//! - **Le trait entier** : un trait tracé à travers les cases est réparti au lâcher (`horschamp.rs`) ; chaque part (dans le dessin d'une case, sur la
//!   planche) porte le même numéro (`vector::Trace::lien`, `annotations::Trace::lien`). Prendre une part prend toutes celles qui la prolongent : celles
//!   du même numéro dont un bout touche le sien (`entier_de`). Un projet d'avant n'a pas ce numéro : ses traits déjà coupés restent des morceaux.
//! - **Les gestes** : la boîte du choix est sur la planche (`selbox.rs`, les mêmes poignées). Ctrl + glisser dedans : déplacer ; un coin : l'échelle ;
//!   hors d'un coin : tourner ; un bord : incliner. Pendant le geste, rien ne change dans le projet : les traits sont peints à leur nouvelle place,
//!   par-dessus (des lignes de leur couleur, sans le grain du pinceau ; règle « sans latence »). **Au lâcher**, chaque trait est rassemblé, transformé,
//!   puis réparti à nouveau comme un trait qu'on vient de dessiner (`App::horschamp_repartir_element`) : chaque case reçoit sa part, le reste va sur
//!   la planche, une seule étape d'annulation.
//! - **Suppr, Ctrl+C, Ctrl+X, Ctrl+V, recolorer** agissent sur les traits entiers. Ctrl+V dans les Vignettes répartit ce qui est collé.
//! - **Les refus de la partie 122 restent** : une case dont le calque est verrouillé, masqué, transformé… ne reçoit rien (la part reste sur la planche,
//!   par-dessus elle) ; une part posée dans un tel calque n'est pas prise (on n'écrit pas dans un calque verrouillé).
//! - **Ce qui ne change pas** : la vue Storyboard et la vue Animation (un seul plan), le calque bitmap (sa sélection reste celle de la case, voir la
//!   partie 125), les formes remplies et le texte d'une case (ils ne se répartissent pas), et les deux gestes de caméra qui déplacent le dessin.

use eframe::egui::{self, Color32, Modifiers, Pos2, Rect, Stroke};

use crate::app::App;
use crate::board::BoardLayout;
use crate::boardui::Xf;
use crate::document::{CelId, LayerKind};
use crate::history::Edit;
use crate::horschamptex::Recette;
use crate::selbox::{self, Grabs, Handle, Mods, SelFrame};
use crate::vector::{Affine, Element, Trace as VTrace, VPoint, VectorDrawing};

/// Deux bouts de parts plus proches que cela (unités de la planche) se suivent : la répartition coupe un segment à un millième près.
const JOINT: f32 = 0.75;
/// Un lasso plus petit que cela (points d'écran) est un clic.
const CLIC_PX: f32 = 4.0;
/// La prise d'un clic autour du trait (points d'écran).
const PRISE_PX: f32 = 6.0;
const PIVOT_SNAP: f32 = 6.0;
const LASSO_MAX: usize = 4000;

/// Où est une part d'un trait.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PartRef {
    /// L'élément `rang` du dessin `cel`, montré par la case `case` (`CardPlace::id`).
    Dessin { cel: CelId, case: u64, rang: usize },
    /// Un trait du calque « Hors cadres » de la planche (`annotations::Trace::id`).
    Planche { id: u64 },
}

/// Un trait entier : ses parts dans l'ordre du tracé.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TraitEntier {
    pub parts: Vec<PartRef>,
}

/// Le geste de la boîte en cours.
#[derive(Clone, Debug)]
pub(crate) struct GesteSel {
    pub handle: Handle,
    pub start: Pos2,
    pub frame: SelFrame,
    pub aff: Affine,
    pub change: bool,
}

/// La sélection de la planche.
#[derive(Clone, Debug, Default)]
pub(crate) struct SelPlanche {
    pub traits: Vec<TraitEntier>,
    /// L'historique au moment du choix : s'il a changé sans nous, les rangs ne valent plus, le choix est levé.
    pub serial: u64,
    pub lasso: Option<Vec<Pos2>>,
    pub ajout: bool,
    pub geste: Option<GesteSel>,
    pub pivot: Option<Pos2>,
}

/// Une part sur la planche : ses points (rayon compris, en unités de la planche), l'appui et l'instant de chacun (vides s'ils ne suivent pas).
#[derive(Clone, Debug, Default)]
pub(crate) struct Ligne {
    pub points: Vec<VPoint>,
    pub appui: Vec<f32>,
    pub instants: Vec<f64>,
}

impl Ligne {
    fn aligne(&self) -> bool {
        self.appui.len() == self.points.len() && self.instants.len() == self.points.len()
    }
}

/// L'allure d'un trait : couleur, opacité, dureté, pinceau (taille en pixels de son dessin `echelle` : unités de la planche par pixel).
#[derive(Clone, Debug)]
pub(crate) struct Allure {
    pub color: [u8; 3],
    pub opacity: f32,
    pub hardness: f32,
    pub brush: Option<crate::brush::BrushSettings>,
    pub feel: Option<crate::brush::Toucher>,
    pub secondary: Option<[u8; 3]>,
    pub echelle: f32,
    pub lien: Option<u64>,
}

/// Un trait entier rassemblé sur la planche.
#[derive(Clone, Debug)]
pub(crate) struct Rassemble {
    pub ligne: Ligne,
    pub allure: Allure,
}

/// Les lignes `parts`, bout à bout : le point de jonction n'est gardé qu'une fois. L'appui suit si toutes le suivent.
pub(crate) fn joindre(parts: &[Ligne]) -> Ligne {
    let aligne = parts.iter().all(Ligne::aligne);
    let mut out = Ligne::default();
    for l in parts {
        let saute = match (out.points.last(), l.points.first()) {
            (Some(a), Some(b)) => (a.pos() - b.pos()).length() <= JOINT,
            _ => false,
        };
        let d = usize::from(saute);
        out.points.extend_from_slice(&l.points[d.min(l.points.len())..]);
        if aligne {
            out.appui.extend_from_slice(&l.appui[d.min(l.appui.len())..]);
            out.instants.extend_from_slice(&l.instants[d.min(l.instants.len())..]);
        }
    }
    out
}

/// La ligne `l` après `aff` (planche) : la position suit, le rayon suit l'échelle.
fn transformee(l: &Ligne, aff: &Affine) -> Ligne {
    let k = aff.width_factor();
    Ligne {
        points: l.points.iter().map(|p| {
            let q = aff.apply(p.pos());
            VPoint { x: q.x, y: q.y, r: p.r * k }
        }).collect(),
        appui: l.appui.clone(),
        instants: l.instants.clone(),
    }
}

/// La distance du point `p` à la ligne brisée `pts`, moins le rayon.
fn distance_au_trait(pts: &[VPoint], p: Pos2) -> f32 {
    let mut best = f32::INFINITY;
    for w in pts.windows(2) {
        let (a, b) = (w[0].pos(), w[1].pos());
        let ab = b - a;
        let t = if ab.length_sq() > 1e-9 { ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
        let r = w[0].r + (w[1].r - w[0].r) * t;
        best = best.min(((a + ab * t) - p).length() - r);
    }
    if pts.len() == 1 {
        best = (pts[0].pos() - p).length() - pts[0].r;
    }
    best
}

impl App {
    /// La sélection de la planche est-elle possible ici : la planche des Vignettes, un calque vectoriel.
    pub(crate) fn selp_possible(&self) -> bool {
        self.board_shown() && self.layers().get(self.cur_layer).is_some_and(|l| l.kind == LayerKind::Vector)
    }

    /// Des traits sont-ils choisis sur la planche (et le choix vaut-il encore) ?
    pub(crate) fn selp_active(&self) -> bool {
        self.vignettes.selp.as_ref().is_some_and(|s| !s.traits.is_empty() && s.serial == self.history.serial())
    }

    /// Lève le choix.
    pub(crate) fn selp_lever(&mut self) {
        self.vignettes.selp = None;
    }

    /// Le calque de la case `k` où l'on choisit et où l'on pose (celui de `case_recoit`, partie 122) et son dessin.
    fn dessin_de_la_case(&self, layout: &BoardLayout, k: usize) -> Option<(usize, CelId)> {
        let rang = self.case_recoit(layout, k, LayerKind::Vector).ok()?;
        let c = layout.cards.get(k)?;
        let cel = self.doc.panels.get(c.panel)?.layers.get(rang)?.cel_at(c.frame)?;
        Some((rang, cel))
    }

    /// Les dessins des cases (un dessin montré par deux cases ne compte qu'une fois, la première) : (case, dessin).
    fn dessins_des_cases(&self, layout: &BoardLayout) -> Vec<(usize, CelId)> {
        let mut out: Vec<(usize, CelId)> = Vec::new();
        for k in 0..layout.cards.len() {
            if let Some((_, cel)) = self.dessin_de_la_case(layout, k) {
                if !out.iter().any(|(_, c)| *c == cel) {
                    out.push((k, cel));
                }
            }
        }
        out
    }

    /// La part `r` sur la planche, et son allure. Rien si elle n'existe plus.
    pub(crate) fn part_planche(&self, layout: &BoardLayout, r: &PartRef) -> Option<(Ligne, Allure)> {
        match *r {
            PartRef::Dessin { cel, case, rang } => {
                let k = layout.cards.iter().position(|c| c.id == case)?;
                let pose = crate::horschamp::pose_de_la_case(&self.doc, layout, k)?;
                let Some(Element::Stroke { points, color, opacity, hardness, brush, trace }) = self.drawing(cel)?.elements.get(rang) else { return None };
                let p0 = points.first()?.pos();
                let echelle = pose.echelle_en(p0);
                let pts: Vec<VPoint> = points.iter().map(|p| {
                    let b = pose.planche(p.pos());
                    VPoint { x: b.x, y: b.y, r: p.r * pose.echelle_en(p.pos()) }
                }).collect();
                let suit = trace.pressure.len() == points.len() && trace.time.len() == points.len();
                let ligne = Ligne { points: pts, appui: if suit { trace.pressure.clone() } else { Vec::new() }, instants: if suit { trace.time.clone() } else { Vec::new() } };
                Some((ligne, Allure { color: *color, opacity: *opacity, hardness: *hardness, brush: brush.as_deref().copied(), feel: trace.feel, secondary: trace.secondary, echelle, lien: trace.lien }))
            }
            PartRef::Planche { id } => {
                let a = &self.doc.annotations;
                let i = a.traces.iter().position(|t| t.id == id)?;
                let t = &a.traces[i];
                let pts = a.points_planche(layout, i)?;
                let (_, case) = crate::annotations::case_de(layout, &t.anchor)?;
                let (ligne, allure) = match t.brosse.as_ref() {
                    Some(b) => {
                        let echelle = case.width() / b.largeur_px.max(1e-3);
                        let suit = b.appui.len() == pts.len() && b.instants.len() == pts.len();
                        let brush = b.recette.reglage.map(|mut s| {
                            s.size *= b.taille;
                            s
                        });
                        (
                            Ligne { appui: if suit { b.appui.clone() } else { Vec::new() }, instants: if suit { b.instants.clone() } else { Vec::new() }, points: pts },
                            Allure { color: t.color, opacity: b.recette.opacite, hardness: b.recette.durete, brush, feel: b.recette.toucher, secondary: b.recette.couleur_b, echelle, lien: t.lien },
                        )
                    }
                    None => (Ligne { points: pts, ..Ligne::default() }, Allure { color: t.color, opacity: f32::from(t.alpha) / 255.0, hardness: 1.0, brush: None, feel: None, secondary: None, echelle: 1.0, lien: t.lien }),
                };
                Some((ligne, allure))
            }
        }
    }

    /// Toutes les parts du numéro `lien` qu'on peut prendre : dans le dessin des cases (calque où l'on choisit), et sur la planche.
    fn parts_du_lien(&self, layout: &BoardLayout, lien: u64) -> Vec<PartRef> {
        let mut out = Vec::new();
        for (k, cel) in self.dessins_des_cases(layout) {
            let Some(d) = self.drawing(cel) else { continue };
            for (rang, e) in d.elements.iter().enumerate() {
                if let Element::Stroke { trace, .. } = e {
                    if trace.lien == Some(lien) {
                        out.push(PartRef::Dessin { cel, case: layout.cards[k].id, rang });
                    }
                }
            }
        }
        if let Some(calque) = self.doc.annotations.hors_cadres_montre() {
            out.extend(self.doc.annotations.traces.iter().filter(|t| t.calque == calque && t.lien == Some(lien)).map(|t| PartRef::Planche { id: t.id }));
        }
        out
    }

    /// Le trait entier dont `depart` est une part : les parts du même numéro qui le prolongent, bout à bout, dans l'ordre du tracé.
    pub(crate) fn entier_de(&self, layout: &BoardLayout, depart: PartRef) -> TraitEntier {
        let seul = TraitEntier { parts: vec![depart] };
        let Some((l0, a0)) = self.part_planche(layout, &depart) else { return seul };
        let Some(lien) = a0.lien else { return seul };
        let mut restes: Vec<(PartRef, Pos2, Pos2)> = self
            .parts_du_lien(layout, lien)
            .into_iter()
            .filter(|r| *r != depart)
            .filter_map(|r| {
                let (l, _) = self.part_planche(layout, &r)?;
                Some((r, l.points.first()?.pos(), l.points.last()?.pos()))
            })
            .collect();
        let (Some(debut), Some(fin)) = (l0.points.first().map(|p| p.pos()), l0.points.last().map(|p| p.pos())) else { return seul };
        let mut chaine = std::collections::VecDeque::from([depart]);
        let (mut debut, mut fin) = (debut, fin);
        loop {
            if let Some(i) = restes.iter().position(|(_, a, _)| (*a - fin).length() <= JOINT) {
                let (r, _, b) = restes.remove(i);
                chaine.push_back(r);
                fin = b;
                continue;
            }
            if let Some(i) = restes.iter().position(|(_, _, b)| (*b - debut).length() <= JOINT) {
                let (r, a, _) = restes.remove(i);
                chaine.push_front(r);
                debut = a;
                continue;
            }
            break;
        }
        TraitEntier { parts: chaine.into_iter().collect() }
    }

    /// Le trait entier `t`, rassemblé sur la planche : ses parts bout à bout, l'allure de sa première part posée dans un dessin (ou de la première).
    pub(crate) fn rassembler(&self, layout: &BoardLayout, t: &TraitEntier) -> Option<Rassemble> {
        let vus: Vec<(Ligne, Allure, bool)> = t.parts.iter().filter_map(|r| self.part_planche(layout, r).map(|(l, a)| (l, a, matches!(r, PartRef::Dessin { .. })))).collect();
        if vus.is_empty() {
            return None;
        }
        let allure = vus.iter().find(|v| v.2).unwrap_or(&vus[0]).1.clone();
        let lignes: Vec<Ligne> = vus.into_iter().map(|v| v.0).collect();
        Some(Rassemble { ligne: joindre(&lignes), allure })
    }

    /// Ce que prend le lasso `poly` (planche) : les traits des dessins des cases et de la planche qui ont un point dedans, chacun avec toutes ses parts.
    pub(crate) fn selp_dans_le_lasso(&self, layout: &BoardLayout, poly: &[Pos2]) -> Vec<TraitEntier> {
        let boite = Rect::from_points(poly);
        let champs = crate::horschamp::champs(layout, self.settings.vignettes.debord_fraction());
        let mut prises: Vec<PartRef> = Vec::new();
        for (k, cel) in self.dessins_des_cases(layout) {
            if !champs.get(k).is_some_and(|c| c.intersects(boite)) {
                continue;
            }
            let Some(pose) = crate::horschamp::pose_de_la_case(&self.doc, layout, k) else { continue };
            let Some(d) = self.drawing(cel) else { continue };
            for (rang, e) in d.elements.iter().enumerate() {
                if let Element::Stroke { points, .. } = e {
                    if points.iter().any(|p| {
                        let b = pose.planche(p.pos());
                        boite.contains(b) && crate::vector::point_in_polygon(b, poly)
                    }) {
                        prises.push(PartRef::Dessin { cel, case: layout.cards[k].id, rang });
                    }
                }
            }
        }
        if let Some(calque) = self.doc.annotations.hors_cadres_montre() {
            prises.extend(self.doc.annotations.dans_le_lasso(layout, calque, poly).into_iter().map(|id| PartRef::Planche { id }));
        }
        self.en_traits_entiers(layout, prises)
    }

    /// Le trait sous le point `p` (planche), à `tol` près : le plus haut d'une case, sinon de la planche.
    pub(crate) fn selp_sous_le_point(&self, layout: &BoardLayout, p: Pos2, tol: f32) -> Vec<TraitEntier> {
        let mut meilleur: Option<(f32, PartRef)> = None;
        for (k, cel) in self.dessins_des_cases(layout) {
            let Some(d) = self.drawing(cel) else { continue };
            for rang in 0..d.elements.len() {
                let r = PartRef::Dessin { cel, case: layout.cards[k].id, rang };
                if !matches!(d.elements[rang], Element::Stroke { .. }) {
                    continue;
                }
                let Some((l, _)) = self.part_planche(layout, &r) else { continue };
                let dist = distance_au_trait(&l.points, p);
                if dist <= tol && meilleur.is_none_or(|(m, _)| dist <= m) {
                    meilleur = Some((dist, r));
                }
            }
        }
        if let Some(calque) = self.doc.annotations.hors_cadres_montre() {
            for t in self.doc.annotations.traces.iter().filter(|t| t.calque == calque) {
                let r = PartRef::Planche { id: t.id };
                let Some((l, _)) = self.part_planche(layout, &r) else { continue };
                let dist = distance_au_trait(&l.points, p);
                if dist <= tol && meilleur.is_none_or(|(m, _)| dist < m) {
                    meilleur = Some((dist, r));
                }
            }
        }
        self.en_traits_entiers(layout, meilleur.map(|(_, r)| r).into_iter().collect())
    }

    /// Les parts `prises`, chacune complétée en son trait entier ; une part déjà prise par un autre trait ne compte qu'une fois.
    fn en_traits_entiers(&self, layout: &BoardLayout, prises: Vec<PartRef>) -> Vec<TraitEntier> {
        let mut out: Vec<TraitEntier> = Vec::new();
        for r in prises {
            if out.iter().any(|t| t.parts.contains(&r)) {
                continue;
            }
            out.push(self.entier_de(layout, r));
        }
        out
    }

    /// La boîte du choix (planche), droite, avec un peu d'air.
    pub(crate) fn selp_cadre(&self, layout: &BoardLayout, zoom: f32) -> Option<SelFrame> {
        let s = self.vignettes.selp.as_ref()?;
        let mut b = Rect::NOTHING;
        for t in &s.traits {
            for r in &t.parts {
                if let Some((l, _)) = self.part_planche(layout, r) {
                    for p in &l.points {
                        b = b.union(Rect::from_center_size(p.pos(), egui::Vec2::splat(p.r * 2.0)));
                    }
                }
            }
        }
        (b.is_finite() && b.width() >= 0.0).then(|| SelFrame { rect0: b.expand(2.0 / zoom.max(0.05)), m: Affine::IDENTITY, pivot: s.pivot })
    }

    fn selp_cadre_vu(&self, layout: &BoardLayout, zoom: f32) -> Option<SelFrame> {
        match self.vignettes.selp.as_ref().and_then(|s| s.geste.as_ref()) {
            Some(g) if g.handle != Handle::Pivot => Some(g.frame.then(&g.aff)),
            _ => self.selp_cadre(layout, zoom),
        }
    }

    /// Un appui sur la planche (`pos`, écran) : Ctrl seul (calque vectoriel) prend l'appui pour la sélection de la planche. Sur la boîte du choix : son geste ;
    /// ailleurs : un lasso. Tout autre appui lève le choix. Vrai si l'appui est pris.
    pub(crate) fn selp_appui(&mut self, layout: &BoardLayout, xf: Xf, pos: Pos2, m: Modifiers) -> bool {
        let ctrl = (m.ctrl || m.command) && !m.shift && !m.alt;
        if !ctrl || !self.selp_possible() {
            self.selp_lever();
            return false;
        }
        let board = xf.to_board(pos);
        if self.selp_active() {
            if let Some(frame) = self.selp_cadre(layout, xf.zoom) {
                let quad = frame.corners().map(|q| xf.to_screen(q));
                let pivot = xf.to_screen(frame.pivot_world());
                if let Some(h) = selbox::hit_test(quad, pivot, pos, &Grabs::default()) {
                    if let Some(s) = self.vignettes.selp.as_mut() {
                        s.geste = Some(GesteSel { handle: h, start: board, frame, aff: Affine::IDENTITY, change: false });
                    }
                    return true;
                }
            }
        }
        let garde = self.selp_active();
        let s = self.vignettes.selp.get_or_insert_with(SelPlanche::default);
        if !garde {
            s.traits.clear();
        }
        s.lasso = Some(vec![board]);
        s.ajout = false;
        s.geste = None;
        // (Un trait commencé ailleurs ne se mêle pas au lasso : `vsel`, la sélection d'une case, est levée.)
        self.vsel = None;
        true
    }

    /// Le geste à cette image d'écran : le lasso grandit, ou la boîte suit le pointeur ; au lâcher, il est posé. Échap abandonne.
    pub(crate) fn selp_geste(&mut self, layout: &BoardLayout, xf: Xf, latest: Option<Pos2>, down: bool, escape: bool) {
        let Some(s) = self.vignettes.selp.as_mut() else { return };
        if escape {
            if s.lasso.is_some() || s.geste.is_some() {
                s.lasso = None;
                s.geste = None;
                self.status = "Geste abandonné : rien n'a bougé.".into();
            } else {
                self.vignettes.selp = None;
            }
            return;
        }
        if let Some(g) = s.geste.as_mut() {
            if let Some(p) = latest {
                let board = xf.to_board(p);
                if g.handle == Handle::Pivot {
                    s.pivot = selbox::move_pivot(&g.frame, g.start, board, PIVOT_SNAP, xf.zoom);
                } else {
                    // (Ctrl est la touche du lasso dans les Vignettes : il ne change pas le point fixe ici.)
                    let m = Mods::with(false, false, self.keep_proportions, self.settings.point_fixe, self.settings.rotation_autour);
                    g.aff = selbox::gesture(g.handle, &g.frame, g.start, board, m);
                    g.change = g.change || g.aff != Affine::IDENTITY;
                    self.status = crate::seltools::gesture_text(g.handle, &g.frame.then(&g.aff).params());
                }
            }
            if !down {
                let g = s.geste.take().expect("le geste");
                if g.handle != Handle::Pivot && g.change && g.aff != Affine::IDENTITY {
                    self.selp_reporter(layout, &g.aff, g.handle.step_name());
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
                let trouves = if l.len() < 3 || etendue < CLIC_PX { self.selp_sous_le_point(layout, l[0], PRISE_PX / zoom) } else { self.selp_dans_le_lasso(layout, &l) };
                let serial = self.history.serial();
                let s = self.vignettes.selp.get_or_insert_with(SelPlanche::default);
                s.traits = trouves;
                s.serial = serial;
                s.pivot = None;
                let n = s.traits.len();
                let parts: usize = s.traits.iter().map(|t| t.parts.len()).sum();
                self.status = match n {
                    0 => "Rien dans le lasso.".into(),
                    1 if parts > 1 => format!("1 trait choisi, entier ({parts} parts) : Ctrl + glisser dans la boîte le déplace, un coin l'agrandit ou le tourne ; Suppr l'efface."),
                    1 => "1 trait choisi : Ctrl + glisser dans la boîte le déplace, un coin l'agrandit ou le tourne ; Suppr l'efface.".into(),
                    n => format!("{n} traits choisis, entiers : Ctrl + glisser dans la boîte les déplace, un coin les agrandit ou les tourne ; Suppr les efface."),
                };
            }
        }
    }

    /// Retire du projet les parts de `traits` (aucune étape posée ici) : rend l'état d'avant de chaque dessin touché, et s'il a fallu toucher la planche.
    fn selp_retirer(&mut self, traits: &[TraitEntier]) -> (Vec<(CelId, VectorDrawing)>, bool) {
        let mut par_dessin: Vec<(CelId, Vec<usize>)> = Vec::new();
        let mut planche: Vec<u64> = Vec::new();
        for r in traits.iter().flat_map(|t| t.parts.iter()) {
            match *r {
                PartRef::Dessin { cel, rang, .. } => match par_dessin.iter_mut().find(|(c, _)| *c == cel) {
                    Some((_, v)) => v.push(rang),
                    None => par_dessin.push((cel, vec![rang])),
                },
                PartRef::Planche { id } => planche.push(id),
            }
        }
        let mut avant = Vec::new();
        for (cel, mut rangs) in par_dessin {
            rangs.sort_unstable();
            rangs.dedup();
            let Some(d) = self.drawing_mut(cel) else { continue };
            avant.push((cel, d.clone()));
            for i in rangs.into_iter().rev() {
                if i < d.elements.len() {
                    d.elements.remove(i);
                }
            }
        }
        let touche = !planche.is_empty() && self.doc.annotations.supprimer_traces(&planche) > 0;
        (avant, touche)
    }

    /// Le dessin `cel` a changé : son image se refait en fond.
    fn selp_refaire(&mut self, cel: CelId) {
        if let Some(c) = self.doc.cels.get_mut(&cel) {
            c.version += 1;
        }
        self.rejeu_cel(cel);
    }

    /// Le trait rassemblé `t` posé de nouveau sur la planche, comme un trait qu'on vient de tracer : dans le dessin de la case où il commence (le premier de
    /// ses points dans une case qui peut le recevoir), puis réparti ; entièrement hors des cases : sur la planche. Les dessins touchés sont notés dans `avant`
    /// **avant** d'être changés. Rend son numéro.
    fn selp_poser(&mut self, layout: &BoardLayout, t: &Rassemble, avant: &mut Vec<(CelId, VectorDrawing)>) -> u64 {
        let lien = t.allure.lien.unwrap_or_else(crate::horschamp::nouveau_lien);
        let champs = crate::horschamp::champs(layout, self.settings.vignettes.debord_fraction());
        let images = |p: Pos2| (0..layout.cards.len()).find(|k| crate::horschamp::dans_image(layout, *k, p));
        let marges = |p: Pos2| champs.iter().position(|c| c.contains(p));
        // (La case où le trait commence : son image d'abord, sinon sa marge ; une case qui ne peut pas le recevoir n'en est pas une.)
        let mut origine: Option<(usize, usize, CelId)> = None;
        'pts: for p in &t.ligne.points {
            for k in [images(p.pos()), marges(p.pos())].into_iter().flatten() {
                if let Ok(rang) = self.case_recoit(layout, k, LayerKind::Vector) {
                    if let Some(cel) = self.cel_de_la_case(layout, k, rang) {
                        origine = Some((k, rang, cel));
                        break 'pts;
                    }
                }
            }
        }
        let Some((k, rang, cel)) = origine else {
            // Hors de toute case : le trait va sur la planche, avec la recette de son pinceau.
            let recette = Recette { reglage: t.allure.brush, opacite: t.allure.opacity, durete: t.allure.hardness, toucher: t.allure.feel, couleur_b: t.allure.secondary };
            let (appui, instants) = if t.ligne.aligne() { (t.ligne.appui.clone(), t.ligne.instants.clone()) } else { (Vec::new(), Vec::new()) };
            let alpha = (t.allure.opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
            let n = self.doc.annotations.traces.len();
            if self.doc.annotations.ajouter_hors_cadres_brosse(layout, &t.ligne.points, t.allure.color, alpha, Some((recette, appui, instants, t.allure.echelle))) {
                if let Some(tr) = self.doc.annotations.traces.get_mut(n) {
                    tr.lien = Some(lien);
                }
                if let Some(id) = self.doc.annotations.id_hors_cadres() {
                    self.doc.annotations.regler_visible(id, true);
                }
            }
            return lien;
        };
        let Some(pose) = crate::horschamp::pose_de_la_case(&self.doc, layout, k) else { return lien };
        let mut pts = Vec::with_capacity(t.ligne.points.len());
        for p in &t.ligne.points {
            let Some(q) = pose.dessin_sur(p.pos()) else { return lien };
            pts.push(VPoint { x: q.x, y: q.y, r: (p.r / pose.echelle_en(q).max(1e-6)).clamp(0.05, 400.0) });
        }
        let cible = pose.echelle_en(pts[0].pos()).max(1e-6);
        let rapport = t.allure.echelle / cible;
        let suit = t.ligne.aligne();
        let el = Element::Stroke {
            points: pts,
            color: t.allure.color,
            opacity: t.allure.opacity,
            hardness: t.allure.hardness,
            brush: t.allure.brush.map(|b| Box::new(crate::brush::BrushSettings { size: b.size * rapport, ..b })),
            trace: VTrace { pressure: if suit { t.ligne.appui.clone() } else { Vec::new() }, time: if suit { t.ligne.instants.clone() } else { Vec::new() }, feel: t.allure.feel, secondary: t.allure.secondary, lien: Some(lien), cache: Default::default() },
        };
        if !avant.iter().any(|(c, _)| *c == cel) {
            let d = self.drawing(cel).cloned().unwrap_or_default();
            self.history.push(Edit::Vector { cel, drawing: d.clone() });
            avant.push((cel, d));
        }
        let Some(d) = self.drawing_mut(cel) else { return lien };
        d.elements.push(el);
        let i = d.elements.len() - 1;
        // (La répartition se fait depuis le plan de la case d'origine, sur son calque : on y passe le temps de répartir.)
        let (cp, cf, cl) = (self.cur_panel, self.cur_frame, self.cur_layer);
        let c = layout.cards[k];
        self.cur_panel = c.panel;
        self.cur_frame = c.frame;
        self.cur_layer = rang;
        let fait = self.horschamp_repartir_element(layout, k, cel, i);
        self.cur_panel = cp.min(self.doc.panels.len().saturating_sub(1));
        self.cur_frame = cf;
        self.cur_layer = cl;
        if !fait {
            self.selp_refaire(cel);
        }
        lien
    }

    /// Les traits choisis reçoivent `aff` (planche) : chacun est rassemblé, transformé, puis réparti à nouveau. Une seule étape d'annulation, nommée `nom`.
    pub(crate) fn selp_reporter(&mut self, layout: &BoardLayout, aff: &Affine, nom: &str) {
        let Some(traits) = self.vignettes.selp.as_ref().filter(|_| self.selp_active()).map(|s| s.traits.clone()) else { return };
        if self.storyboard_refuses() {
            return;
        }
        let rassembles: Vec<Rassemble> = traits.iter().filter_map(|t| self.rassembler(layout, t)).map(|r| Rassemble { ligne: transformee(&r.ligne, aff), allure: r.allure }).collect();
        // Ce que le geste montrait, tenu à l'écran tant que les parts ne sont pas rendues.
        let tenu = self.selp_apercu_formes(layout, Some(aff));
        let base = self.history.undo_len();
        let before = self.history.context.replace(nom.to_string());
        let snap = self.snapshot();
        let (mut avant, _) = self.selp_retirer(&traits);
        // (L'état d'avant, rangé d'abord : il est rendu en dernier à l'annulation.)
        self.history.push(snap);
        for (cel, d) in &avant {
            self.history.push(Edit::Vector { cel: *cel, drawing: d.clone() });
        }
        let touches: Vec<CelId> = avant.iter().map(|(c, _)| *c).collect();
        let mut liens = Vec::new();
        for r in &rassembles {
            liens.push(self.selp_poser(layout, r, &mut avant));
        }
        for c in touches {
            self.selp_refaire(c);
        }
        let n = self.history.undo_len().saturating_sub(base);
        if n >= 2 {
            self.history.merge_last(n);
        }
        self.history.context = before;
        self.modified = true;
        self.renderer.invalidate_all();
        // Le choix suit les traits à leur nouvelle place.
        let layout = self.board_layout();
        let mut nouveaux: Vec<TraitEntier> = Vec::new();
        for lien in liens {
            if let Some(r) = self.parts_du_lien(&layout, lien).into_iter().find(|r| !nouveaux.iter().any(|t| t.parts.contains(r))) {
                nouveaux.push(self.entier_de(&layout, r));
            }
        }
        let serial = self.history.serial();
        if let Some(s) = self.vignettes.selp.as_mut() {
            s.traits = nouveaux;
            s.serial = serial;
            s.pivot = None;
        }
        if !tenu.is_empty() {
            self.vignettes.debord_tenu = Some(crate::debord::Tenu {
                debord: crate::debord::Debord { thumb: Rect::NOTHING, zone: Some(Vec::new()), shapes: tenu },
                xf: self.vignettes.xf,
                etapes: self.history.undo_len(),
                debut: std::time::Instant::now(),
            });
        }
        self.ctx.request_repaint();
    }

    /// Suppr : les traits choisis partent, toutes leurs parts, en une étape.
    pub(crate) fn selp_supprimer(&mut self) {
        let Some(traits) = self.vignettes.selp.as_ref().filter(|_| self.selp_active()).map(|s| s.traits.clone()) else { return };
        if self.storyboard_refuses() {
            return;
        }
        let base = self.history.undo_len();
        let before = self.history.context.replace("Effacer".to_string());
        let snap = self.snapshot();
        let (avant, _) = self.selp_retirer(&traits);
        self.history.push(snap);
        for (cel, d) in &avant {
            self.history.push(Edit::Vector { cel: *cel, drawing: d.clone() });
        }
        for (c, _) in &avant {
            self.selp_refaire(*c);
        }
        let n = self.history.undo_len().saturating_sub(base);
        if n >= 2 {
            self.history.merge_last(n);
        }
        self.history.context = before;
        self.modified = true;
        self.renderer.invalidate_all();
        self.vignettes.selp = None;
        self.status = format!("{} trait{} effacé{}, entier{}.", traits.len(), if traits.len() > 1 { "s" } else { "" }, if traits.len() > 1 { "s" } else { "" }, if traits.len() > 1 { "s" } else { "" });
    }

    /// Ctrl+C : les traits choisis, entiers, dans le presse-papiers des éléments, à leur place dans le dessin de la case courante. Vrai si copié.
    pub(crate) fn selp_copier(&mut self) -> bool {
        let Some(traits) = self.vignettes.selp.as_ref().filter(|_| self.selp_active()).map(|s| s.traits.clone()) else { return false };
        let layout = self.board_layout();
        let cur = self.cur_frame.min(self.frame_count().saturating_sub(1));
        let Some(k) = layout.cards.iter().position(|c| c.panel == self.cur_panel && (c.frame..c.end).contains(&cur)) else { return false };
        let Some(pose) = crate::horschamp::pose_de_la_case(&self.doc, &layout, k) else { return false };
        let mut items = Vec::new();
        for t in &traits {
            let Some(r) = self.rassembler(&layout, t) else { continue };
            let Some(pts) = r.ligne.points.iter().map(|p| pose.dessin_sur(p.pos()).map(|q| VPoint { x: q.x, y: q.y, r: p.r / pose.echelle_en(q).max(1e-6) })).collect::<Option<Vec<_>>>() else { continue };
            let Some(p0) = pts.first().map(|p| p.pos()) else { continue };
            let rapport = r.allure.echelle / pose.echelle_en(p0).max(1e-6);
            let suit = r.ligne.aligne();
            items.push(Element::Stroke {
                points: pts,
                color: r.allure.color,
                opacity: r.allure.opacity,
                hardness: r.allure.hardness,
                brush: r.allure.brush.map(|b| Box::new(crate::brush::BrushSettings { size: b.size * rapport, ..b })),
                // (Une copie n'est plus une part du trait d'origine : sans numéro.)
                trace: VTrace { pressure: if suit { r.ligne.appui } else { Vec::new() }, time: if suit { r.ligne.instants } else { Vec::new() }, feel: r.allure.feel, secondary: r.allure.secondary, lien: None, cache: Default::default() },
            });
        }
        if items.is_empty() {
            return false;
        }
        self.status = format!("{} trait(s) copié(s), entiers.", items.len());
        self.ctx.copy_text(format!("{}{} élément(s) vectoriel(s)", crate::pasteimage::MARK, items.len()));
        self.clipboard = items;
        self.collage.origines.elements = Some(crate::adaptation::Origine::de(&self.doc));
        self.multi_clip = None;
        true
    }

    /// Recolorer : toutes les parts prennent la couleur courante, en une étape.
    pub(crate) fn selp_recolorer(&mut self) {
        let Some(traits) = self.vignettes.selp.as_ref().filter(|_| self.selp_active()).map(|s| s.traits.clone()) else { return };
        let color = self.color;
        let base = self.history.undo_len();
        let before = self.history.context.replace("Recolorer".to_string());
        self.history.push(self.snapshot());
        let mut touches: Vec<CelId> = Vec::new();
        for r in traits.iter().flat_map(|t| t.parts.iter()) {
            match *r {
                PartRef::Dessin { cel, rang, .. } => {
                    if !touches.contains(&cel) {
                        if let Some(d) = self.drawing(cel).cloned() {
                            self.history.push(Edit::Vector { cel, drawing: d });
                        }
                        touches.push(cel);
                    }
                    if let Some(e) = self.drawing_mut(cel).and_then(|d| d.elements.get_mut(rang)) {
                        e.set_color(color);
                    }
                }
                PartRef::Planche { id } => {
                    if let Some(t) = self.doc.annotations.traces.iter_mut().find(|t| t.id == id) {
                        t.color = color;
                    }
                }
            }
        }
        for c in touches {
            self.selp_refaire(c);
        }
        let n = self.history.undo_len().saturating_sub(base);
        if n >= 2 {
            self.history.merge_last(n);
        }
        self.history.context = before;
        self.modified = true;
        self.renderer.invalidate_all();
        let serial = self.history.serial();
        if let Some(s) = self.vignettes.selp.as_mut() {
            s.serial = serial;
        }
    }

    /// Ctrl+V dans les Vignettes : les éléments collés dans le dessin `cel` (rangs `rangs`) sont répartis comme des traits qu'on vient de tracer, et
    /// deviennent le choix de la planche. L'étape du collage les rejoint.
    pub(crate) fn selp_apres_le_collage(&mut self, cel: CelId, rangs: std::ops::Range<usize>, base: usize) {
        let layout = self.board_layout();
        let cur = self.cur_frame.min(self.frame_count().saturating_sub(1));
        let Some(k) = layout.cards.iter().position(|c| c.panel == self.cur_panel && (c.frame..c.end).contains(&cur)) else { return };
        let mut liens = Vec::new();
        for i in rangs.rev() {
            let lien = crate::horschamp::nouveau_lien();
            match self.drawing_mut(cel).and_then(|d| d.elements.get_mut(i)) {
                Some(Element::Stroke { trace, .. }) => trace.lien = Some(lien),
                _ => continue,
            }
            liens.push(lien);
            self.horschamp_repartir_element(&layout, k, cel, i);
        }
        let n = self.history.undo_len().saturating_sub(base);
        if n >= 2 {
            self.history.merge_last(n);
        }
        self.vsel = None;
        let layout = self.board_layout();
        let mut traits: Vec<TraitEntier> = Vec::new();
        for lien in liens {
            if let Some(r) = self.parts_du_lien(&layout, lien).into_iter().next() {
                traits.push(self.entier_de(&layout, r));
            }
        }
        let serial = self.history.serial();
        self.vignettes.selp = Some(SelPlanche { traits, serial, ..SelPlanche::default() });
    }

    /// Les formes du choix à l'écran, à la place où le geste `aff` le mène (rien : à sa place) : des lignes de sa couleur, par-dessus tout.
    fn selp_apercu_formes(&self, layout: &BoardLayout, aff: Option<&Affine>) -> Vec<(Rect, egui::Shape)> {
        let Some(s) = self.vignettes.selp.as_ref() else { return Vec::new() };
        let xf = self.vignettes.xf;
        let mut out = Vec::new();
        for t in &s.traits {
            let Some(r) = self.rassembler(layout, t) else { continue };
            let l = match aff {
                Some(a) => transformee(&r.ligne, a),
                None => r.ligne,
            };
            let couleur = Color32::from_rgba_unmultiplied(r.allure.color[0], r.allure.color[1], r.allure.color[2], (r.allure.opacity.clamp(0.0, 1.0) * 255.0).round() as u8);
            let pts: Vec<Pos2> = l.points.iter().map(|p| xf.to_screen(p.pos())).collect();
            if pts.len() < 2 {
                continue;
            }
            let w = (l.points.iter().map(|p| p.r).sum::<f32>() / l.points.len() as f32 * xf.zoom * 2.0).max(1.0);
            let b = Rect::from_points(&pts).expand(w);
            out.push((b, egui::Shape::line(pts.clone(), Stroke::new(w, couleur))));
            for e in [pts[0], pts[pts.len() - 1]] {
                out.push((Rect::from_center_size(e, egui::Vec2::splat(w)), egui::Shape::circle_filled(e, w * 0.5, couleur)));
            }
        }
        out
    }

    /// Le choix de la planche à l'écran : chaque trait entier souligné, le lasso en cours, la boîte et ses poignées ; pendant un geste, les traits à leur
    /// nouvelle place.
    pub(crate) fn selp_peindre(&self, painter: &egui::Painter, view: Rect, layout: &BoardLayout, survol: Option<Pos2>) {
        use crate::theme::{SELECTION_BOITE as ACCENT, SELECTION_POIGNEE_FOND};
        let Some(s) = self.vignettes.selp.as_ref() else { return };
        let xf = self.vignettes.xf;
        let clip = painter.with_clip_rect(view);
        if let Some(l) = &s.lasso {
            let pts: Vec<Pos2> = l.iter().map(|p| xf.to_screen(*p)).collect();
            if let Some(shape) = crate::contours::lasso_shape(&pts) {
                clip.add(shape);
            }
        }
        if !self.selp_active() {
            return;
        }
        let geste = s.geste.as_ref().filter(|g| g.change && g.handle != Handle::Pivot).map(|g| g.aff);
        if let Some(aff) = geste {
            for (_, sh) in self.selp_apercu_formes(layout, Some(&aff)) {
                clip.add(sh);
            }
        }
        // Le trait entier souligné, toutes ses parts (celles des cases voisines et de la planche comprises).
        for t in &s.traits {
            for r in &t.parts {
                let Some((l, _)) = self.part_planche(layout, r) else { continue };
                let l = match geste {
                    Some(a) => transformee(&l, &a),
                    None => l,
                };
                let pts: Vec<Pos2> = l.points.iter().map(|p| xf.to_screen(p.pos())).collect();
                if pts.len() >= 2 {
                    clip.add(egui::Shape::line(pts, Stroke::new(1.5_f32, ACCENT)));
                }
            }
        }
        let Some(frame) = self.selp_cadre_vu(layout, xf.zoom) else { return };
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
