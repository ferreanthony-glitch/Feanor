//! Les bords des cases ne coupent jamais le dessin (partie 125, Anthony, 2026-11-08 : « quand je sélectionne des traits dans un cadre et que je les
//! déplace ou les transforme, seule la partie dans le champ du cadre est prise, le reste est coupé » ; « pour moi les caméras doivent être le moins
//! intrusives et destructives possibles dans le dessin »).
//!
//! # Ce que c'est
//!
//! - **La sélection de la planche** : dans les Vignettes, sur un calque vectoriel, **Ctrl + glisser** trace un lasso **n'importe où** : dans une case,
//!   sur le papier autour et entre les cases, ou à cheval. **Ctrl + clic** prend ce qui est sous le pointeur. Le lasso est sur la planche, pas dans le
//!   dessin d'une case : il ne s'arrête à aucun bord. (L'outil Caméra et T tenue gardent Ctrl pour la caméra.)
//! - **Le trait entier** : un trait tracé à travers les cases est réparti au lâcher (`horschamp.rs`) ; chaque part (dans le dessin d'une case, sur la
//!   planche) porte le même numéro (`vector::Trace::lien`, `annotations::Trace::lien`). Prendre une part prend toutes celles qui la prolongent : celles
//!   du même numéro dont un bout touche le sien (`entier_de`). Un projet d'avant n'a pas ce numéro : ses traits déjà coupés restent des morceaux. Une
//!   copie (dessin dupliqué, plan dupliqué, collage) prend des numéros neufs.
//! - **Les autres éléments d'une case** (formes remplies, textes, images) sont pris aussi, comme avant : ils restent dans leur case et ne sont pas
//!   répartis ; le geste les transforme sur place, dans le même pas que les traits.
//! - **Les gestes** : la boîte du choix est sur la planche (`selbox.rs`, les mêmes poignées). Ctrl + glisser dedans : déplacer ; un coin : l'échelle ;
//!   hors d'un coin : tourner ; un bord : incliner. Pendant le geste, rien ne change dans le projet : les traits sont peints à leur nouvelle place,
//!   par-dessus (des lignes de leur couleur ; règle « sans latence »). **Au lâcher**, tout est d'abord calculé sans rien écrire (`selp_preparer`) : si une
//!   part n'a nulle part où aller (planche pleine), le geste est refusé tel quel et la ligne d'état le dit. Sinon chaque trait est rassemblé, transformé,
//!   puis réparti comme un trait qu'on vient de tracer (`App::horschamp_repartir_element`) : il garde son rang dans le dessin d'où il vient ; dans une
//!   case d'arrivée il est posé dessus, ou dessous s'il était peint derrière. Une seule étape d'annulation, même historique plein.
//! - **Suppr, Ctrl+C, Ctrl+X, Ctrl+V, recolorer** agissent sur le choix entier, quand le dessin a la main (pas la timeline, le montage ni un panneau).
//! - **Les refus de la partie 122 restent** : une case dont le calque est verrouillé, masqué, transformé… ne reçoit rien (la part reste sur la planche,
//!   par-dessus elle, `ajouter_sur_la_case`) ; une part posée dans un tel calque n'est pas prise.
//! - **Ce qui ne change pas** : la vue Storyboard et la vue Animation (un seul plan), et les deux gestes de caméra qui déplacent le dessin. Le calque
//!   bitmap a sa propre sélection de la planche (partie 127, `vignetteselbmp.rs`).

use std::cell::RefCell;

use eframe::egui::{self, pos2, Color32, Modifiers, Pos2, Rect, Stroke};

use crate::app::{App, Tool};
use crate::board::BoardLayout;
use crate::boardui::Xf;
use crate::document::{CelId, LayerKind};
use crate::history::Edit;
use crate::horschamp::{Pose, Pt};
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

/// Où est une part.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PartRef {
    /// L'élément `rang` du dessin `cel`, montré par la case `case` (`CardPlace::id`).
    Dessin { cel: CelId, case: u64, rang: usize },
    /// Un trait du calque « Hors cadres » de la planche (`annotations::Trace::id`).
    Planche { id: u64 },
}

/// Un trait entier (ses parts dans l'ordre du tracé), ou un autre élément d'une case (`objet` : une forme, un texte, une image ; il reste dans sa case).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TraitEntier {
    pub parts: Vec<PartRef>,
    pub objet: bool,
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

/// Ce que le choix montre sur la planche, calculé une fois tant que rien ne change (`selp_vue`).
#[derive(Clone, Debug)]
pub(crate) struct Vue {
    cle: (u64, usize, usize, [u32; 2]),
    /// Chaque part d'un trait, sur la planche (rayon compris).
    parts: Vec<Vec<VPoint>>,
    /// Chaque trait entier, rassemblé, et sa couleur (l'aperçu du geste).
    entiers: Vec<(Vec<VPoint>, Color32)>,
    /// Les coins de chaque autre élément pris.
    objets: Vec<[Pos2; 4]>,
    bornes: Rect,
}

impl Default for Vue {
    fn default() -> Self {
        Vue { cle: (0, 0, 0, [0, 0]), parts: Vec::new(), entiers: Vec::new(), objets: Vec::new(), bornes: Rect::NOTHING }
    }
}

/// La sélection de la planche.
#[derive(Clone, Debug, Default)]
pub(crate) struct SelPlanche {
    pub traits: Vec<TraitEntier>,
    /// L'historique au moment du choix : s'il a changé sans nous, les rangs ne valent plus, le choix est levé.
    pub serial: u64,
    pub lasso: Option<Vec<Pos2>>,
    pub geste: Option<GesteSel>,
    pub pivot: Option<Pos2>,
    vue: RefCell<Option<Vue>>,
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

/// Où un trait déplacé sera reposé, calculé avant de rien changer.
enum Repose {
    /// Dans le dessin de la case `k` (calque `rang`), ses points dans ce dessin ; puis réparti.
    Case { k: usize, rang: usize, points: Vec<VPoint>, echelle: f32 },
    /// Hors de toute case qui puisse le recevoir : sur la planche, par tronçons (la case qu'un tronçon recouvre, s'il en recouvre une ; ses points).
    Planche { troncons: Vec<(Option<crate::board::CardPlace>, Ligne)> },
}

/// Un trait prêt à être reposé.
struct APoser {
    trait_: Rassemble,
    repose: Repose,
    lien: u64,
    /// La place qu'avaient ses parts dans les dessins d'où elles viennent (rang une fois les parts retirées) : il la garde.
    places: Vec<(CelId, usize)>,
    derriere: bool,
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
    if pts.len() == 1 {
        return (pts[0].pos() - p).length() - pts[0].r;
    }
    let mut best = f32::INFINITY;
    for w in pts.windows(2) {
        let (a, b) = (w[0].pos(), w[1].pos());
        let ab = b - a;
        let t = if ab.length_sq() > 1e-9 { ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
        let r = w[0].r + (w[1].r - w[0].r) * t;
        best = best.min(((a + ab * t) - p).length() - r);
    }
    best
}

/// L'affinité qui fait, dans un dessin posé par `de`, ce que `aff` fait sur la planche, puis le pose par `vers` (autour du point `o` du dessin : exacte
/// pour une caméra droite, l'approximation locale sinon).
fn affinite_locale(de: &Pose, aff: &Affine, vers: &Pose, o: Pos2) -> Option<Affine> {
    let u = 50.0;
    let f = |p: Pos2| vers.dessin_sur(aff.apply(de.planche(p)));
    let (oo, x, y) = (f(o)?, f(o + egui::vec2(u, 0.0))?, f(o + egui::vec2(0.0, u))?);
    let (a, c, b, d) = ((x.x - oo.x) / u, (x.y - oo.y) / u, (y.x - oo.x) / u, (y.y - oo.y) / u);
    let r = Affine { a, b, c, d, tx: oo.x - a * o.x - b * o.y, ty: oo.y - c * o.x - d * o.y };
    [r.a, r.b, r.c, r.d, r.tx, r.ty].iter().all(|v| v.is_finite()).then_some(r)
}

impl App {
    /// La sélection de la planche est-elle possible ici : la planche des Vignettes, un calque vectoriel.
    pub(crate) fn selp_possible(&self) -> bool {
        self.board_shown() && self.layers().get(self.cur_layer).is_some_and(|l| l.kind == LayerKind::Vector)
    }

    /// Des éléments sont-ils choisis sur la planche, et le choix vaut-il encore (même historique, calque vectoriel) ?
    pub(crate) fn selp_active(&self) -> bool {
        self.selp_possible() && self.vignettes.selp.as_ref().is_some_and(|s| !s.traits.is_empty() && s.serial == self.history.serial())
    }

    /// Les touches d'édition (Suppr, Ctrl+C, Ctrl+X) vont-elles au choix de la planche : il vaut, et le dessin a la main (pas les plans, les images de la
    /// timeline ni les clés).
    pub(crate) fn selp_a_la_main(&self) -> bool {
        self.selp_active() && !self.panels_have_focus() && !self.images_focus && !self.key_focus
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

    /// La part `r` (un trait) sur la planche, et son allure. Rien si elle n'existe plus ou n'est pas un trait.
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

    /// Les coins, sur la planche, de l'élément `r` d'une case (une forme, un texte, une image).
    fn objet_coins(&self, layout: &BoardLayout, r: &PartRef) -> Option<[Pos2; 4]> {
        let PartRef::Dessin { cel, case, rang } = *r else { return None };
        let k = layout.cards.iter().position(|c| c.id == case)?;
        let pose = crate::horschamp::pose_de_la_case(&self.doc, layout, k)?;
        let b = self.drawing(cel)?.elements.get(rang)?.bounds();
        b.is_finite().then(|| [b.left_top(), b.right_top(), b.right_bottom(), b.left_bottom()].map(|p| pose.planche(p)))
    }

    /// Toutes les parts du numéro `lien` qu'on peut prendre : dans le dessin des cases (calque où l'on choisit), et sur la planche.
    fn parts_du_lien(&self, layout: &BoardLayout, dessins: &[(usize, CelId)], lien: u64) -> Vec<PartRef> {
        let mut out = Vec::new();
        for &(k, cel) in dessins {
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

    /// Le trait entier dont `depart` est une part : les parts du même numéro qui le prolongent, bout à bout, dans l'ordre du tracé (`dessins` : les dessins
    /// des cases, `dessins_des_cases`).
    fn entier_de_avec(&self, layout: &BoardLayout, dessins: &[(usize, CelId)], depart: PartRef) -> TraitEntier {
        let seul = TraitEntier { parts: vec![depart], objet: false };
        let Some((l0, a0)) = self.part_planche(layout, &depart) else { return seul };
        let Some(lien) = a0.lien else { return seul };
        let mut restes: Vec<(PartRef, Pos2, Pos2)> = self
            .parts_du_lien(layout, dessins, lien)
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
        TraitEntier { parts: chaine.into_iter().collect(), objet: false }
    }

    /// Le trait entier `t`, rassemblé sur la planche : ses parts bout à bout, l'allure de sa première part posée dans un dessin (ou de la première).
    pub(crate) fn rassembler(&self, layout: &BoardLayout, t: &TraitEntier) -> Option<Rassemble> {
        if t.objet {
            return None;
        }
        let vus: Vec<(Ligne, Allure, bool)> = t.parts.iter().filter_map(|r| self.part_planche(layout, r).map(|(l, a)| (l, a, matches!(r, PartRef::Dessin { .. })))).collect();
        if vus.is_empty() {
            return None;
        }
        let allure = vus.iter().find(|v| v.2).unwrap_or(&vus[0]).1.clone();
        let lignes: Vec<Ligne> = vus.into_iter().map(|v| v.0).collect();
        Some(Rassemble { ligne: joindre(&lignes), allure })
    }

    /// Ce que prend le lasso `poly` (planche) : les traits des dessins des cases et de la planche qui ont un point dedans, chacun avec toutes ses parts ;
    /// et les autres éléments des cases qu'il touche (comme le lasso d'une case).
    pub(crate) fn selp_dans_le_lasso(&self, layout: &BoardLayout, poly: &[Pos2]) -> Vec<TraitEntier> {
        let boite = Rect::from_points(poly);
        let dessins = self.dessins_des_cases(layout);
        let champs = crate::horschamp::champs(layout, self.settings.vignettes.debord_fraction());
        let mut prises: Vec<(PartRef, bool)> = Vec::new();
        for &(k, cel) in &dessins {
            // (Seul ce que la case montre compte : un trait d'avant la partie 111 qui dépasse de son champ n'est pas pris par ce qu'on n'y voit pas.)
            let Some(champ) = champs.get(k).copied().filter(|c| c.intersects(boite)) else { continue };
            let Some(pose) = crate::horschamp::pose_de_la_case(&self.doc, layout, k) else { continue };
            let Some(d) = self.drawing(cel) else { continue };
            // (Le lasso dans le dessin de la case, pour les éléments qui ne sont pas des traits : s'il a une place dans ce dessin.)
            let poly_dessin: Option<Vec<Pos2>> = poly.iter().map(|p| pose.dessin_sur(*p)).collect();
            for (rang, e) in d.elements.iter().enumerate() {
                let r = PartRef::Dessin { cel, case: layout.cards[k].id, rang };
                match e {
                    Element::Stroke { points, .. } => {
                        if points.iter().any(|p| {
                            let b = pose.planche(p.pos());
                            champ.contains(b) && boite.contains(b) && crate::vector::point_in_polygon(b, poly)
                        }) {
                            prises.push((r, false));
                        }
                    }
                    _ => {
                        if poly_dessin.as_ref().is_some_and(|q| e.touches(q)) {
                            prises.push((r, true));
                        }
                    }
                }
            }
        }
        if let Some(calque) = self.doc.annotations.hors_cadres_montre() {
            prises.extend(self.doc.annotations.dans_le_lasso(layout, calque, poly).into_iter().map(|id| (PartRef::Planche { id }, false)));
        }
        self.en_traits_entiers(layout, &dessins, prises)
    }

    /// Ce qui est sous le point `p` (planche), à `tol` près : le trait le plus proche d'une case ou de la planche ; sinon l'élément d'une case sous le point.
    pub(crate) fn selp_sous_le_point(&self, layout: &BoardLayout, p: Pos2, tol: f32) -> Vec<TraitEntier> {
        let dessins = self.dessins_des_cases(layout);
        let mut meilleur: Option<(f32, PartRef)> = None;
        let mut objet: Option<PartRef> = None;
        for &(k, cel) in &dessins {
            let Some(d) = self.drawing(cel) else { continue };
            let Some(pose) = crate::horschamp::pose_de_la_case(&self.doc, layout, k) else { continue };
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
            if objet.is_none() && crate::horschamp::dans_image(layout, k, p) {
                if let Some(q) = pose.dessin_sur(p) {
                    let tol_d = tol / pose.echelle_en(q).max(1e-6);
                    if let Some(i) = d.hit_test(q, tol_d).filter(|i| !matches!(d.elements[*i], Element::Stroke { .. })) {
                        objet = Some(PartRef::Dessin { cel, case: layout.cards[k].id, rang: i });
                    }
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
        let prises = match (meilleur, objet) {
            (Some((_, r)), _) => vec![(r, false)],
            (None, Some(r)) => vec![(r, true)],
            _ => Vec::new(),
        };
        self.en_traits_entiers(layout, &dessins, prises)
    }

    /// Les parts `prises`, chacune complétée en son trait entier (un autre élément reste seul) ; une part déjà prise ne compte qu'une fois.
    fn en_traits_entiers(&self, layout: &BoardLayout, dessins: &[(usize, CelId)], prises: Vec<(PartRef, bool)>) -> Vec<TraitEntier> {
        let mut out: Vec<TraitEntier> = Vec::new();
        let mut vues: std::collections::HashSet<(u8, u64, usize)> = std::collections::HashSet::new();
        let cle = |r: &PartRef| match *r {
            PartRef::Dessin { cel, rang, .. } => (0u8, cel, rang),
            PartRef::Planche { id } => (1u8, id, 0),
        };
        for (r, objet) in prises {
            if vues.contains(&cle(&r)) {
                continue;
            }
            let t = if objet { TraitEntier { parts: vec![r], objet: true } } else { self.entier_de_avec(layout, dessins, r) };
            for p in &t.parts {
                vues.insert(cle(p));
            }
            out.push(t);
        }
        out
    }

    /// Ce que le choix montre sur la planche : calculé une fois tant que l'historique, la planche et le choix ne changent pas.
    fn selp_vue(&self, layout: &BoardLayout) -> Option<Vue> {
        let s = self.vignettes.selp.as_ref()?;
        let cle = (self.history.serial(), s.traits.len(), layout.cards.len(), [layout.size.x.to_bits(), layout.size.y.to_bits()]);
        if let Some(v) = s.vue.borrow().as_ref().filter(|v| v.cle == cle) {
            return Some(v.clone());
        }
        let mut v = Vue { cle, bornes: Rect::NOTHING, ..Vue::default() };
        for t in &s.traits {
            if t.objet {
                if let Some(q) = t.parts.first().and_then(|r| self.objet_coins(layout, r)) {
                    for p in q {
                        v.bornes = v.bornes.union(Rect::from_min_max(p, p));
                    }
                    v.objets.push(q);
                }
                continue;
            }
            for r in &t.parts {
                if let Some((l, _)) = self.part_planche(layout, r) {
                    for p in &l.points {
                        v.bornes = v.bornes.union(Rect::from_center_size(p.pos(), egui::Vec2::splat(p.r * 2.0)));
                    }
                    v.parts.push(l.points);
                }
            }
            if let Some(r) = self.rassembler(layout, t) {
                let a = r.allure;
                v.entiers.push((r.ligne.points, Color32::from_rgba_unmultiplied(a.color[0], a.color[1], a.color[2], (a.opacity.clamp(0.0, 1.0) * 255.0).round() as u8)));
            }
        }
        *s.vue.borrow_mut() = Some(v.clone());
        Some(v)
    }

    /// La boîte du choix (planche), droite, avec un peu d'air.
    pub(crate) fn selp_cadre(&self, layout: &BoardLayout, zoom: f32) -> Option<SelFrame> {
        let v = self.selp_vue(layout)?;
        let s = self.vignettes.selp.as_ref()?;
        (v.bornes.is_finite() && v.bornes.width() >= 0.0).then(|| SelFrame { rect0: v.bornes.expand(2.0 / zoom.max(0.05)), m: Affine::IDENTITY, pivot: s.pivot })
    }

    fn selp_cadre_vu(&self, layout: &BoardLayout, zoom: f32) -> Option<SelFrame> {
        match self.vignettes.selp.as_ref().and_then(|s| s.geste.as_ref()) {
            Some(g) if g.handle != Handle::Pivot => Some(g.frame.then(&g.aff)),
            _ => self.selp_cadre(layout, zoom),
        }
    }

    /// Un appui sur la planche (`pos`, écran) : Ctrl seul (calque vectoriel) prend l'appui pour la sélection de la planche. Sur la boîte du choix : son
    /// geste ; ailleurs : un lasso. Tout autre appui lève le choix. L'outil Caméra et T tenue gardent Ctrl pour la caméra. Vrai si l'appui est pris.
    pub(crate) fn selp_appui(&mut self, layout: &BoardLayout, xf: Xf, pos: Pos2, m: Modifiers) -> bool {
        let ctrl = (m.ctrl || m.command) && !m.shift && !m.alt;
        if !ctrl || !self.selp_possible() || self.tool == Tool::Camera || self.vignettes.camera_tenue {
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
        let s = self.vignettes.selp.get_or_insert_with(SelPlanche::default);
        s.traits.clear();
        s.lasso = Some(vec![board]);
        s.geste = None;
        // (La sélection d'une case n'a plus cours : la planche a le choix.)
        self.vsel = None;
        true
    }

    /// Le geste à cette image d'écran : le lasso grandit, ou la boîte suit le pointeur ; au lâcher, il est posé. Échap abandonne. Un appui hors de la
    /// planche (`dehors`), ou un calque qui n'est plus vectoriel, lève le choix.
    pub(crate) fn selp_geste(&mut self, layout: &BoardLayout, xf: Xf, latest: Option<Pos2>, down: bool, escape: bool, dehors: bool) {
        if self.vignettes.selp.is_some() && (dehors || !self.selp_possible()) {
            self.selp_lever();
            return;
        }
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
                *s.vue.borrow_mut() = None;
                let n = s.traits.iter().filter(|t| !t.objet).count();
                let o = s.traits.len() - n;
                let parts: usize = s.traits.iter().filter(|t| !t.objet).map(|t| t.parts.len()).sum();
                let objets = if o > 0 { format!(" et {o} élément{} de case", if o > 1 { "s" } else { "" }) } else { String::new() };
                self.status = match (n, o) {
                    (0, 0) => "Rien dans le lasso.".into(),
                    (1, _) if parts > 1 => format!("1 trait choisi, entier ({parts} parts){objets} : Ctrl + glisser dans la boîte le déplace, un coin l'agrandit ou le tourne ; Suppr l'efface."),
                    _ => format!("{n} trait{} choisi{}, entier{}{objets} : Ctrl + glisser dans la boîte pour déplacer, un coin pour agrandir ou tourner ; Suppr pour effacer.", if n > 1 { "s" } else { "" }, if n > 1 { "s" } else { "" }, if n > 1 { "s" } else { "" }),
                };
            }
        }
    }

    /// Retire du projet les parts de `traits` (aucune étape posée ici) : rend l'état d'avant de chaque dessin touché.
    fn selp_retirer(&mut self, traits: &[TraitEntier]) -> Vec<(CelId, VectorDrawing)> {
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
        if !planche.is_empty() {
            self.doc.annotations.supprimer_traces(&planche);
        }
        avant
    }

    /// Le dessin `cel` a changé : son image se refait en fond.
    fn selp_refaire(&mut self, cel: CelId) {
        if let Some(c) = self.doc.cels.get_mut(&cel) {
            c.version += 1;
        }
        self.rejeu_cel(cel);
    }

    /// Où reposer le trait rassemblé `t`, **sans rien changer** : dans le dessin de la première case qui peut le recevoir là où il passe (son image, puis sa
    /// marge), si ce dessin a une place pour tous ses points ; sinon sur la planche, par tronçons (par-dessus l'image des cases qu'il recouvre).
    fn selp_preparer(&self, layout: &BoardLayout, t: &Rassemble) -> Repose {
        let champs = crate::horschamp::champs(layout, self.settings.vignettes.debord_fraction());
        let mut essayees: Vec<usize> = Vec::new();
        for p in &t.ligne.points {
            let images = (0..layout.cards.len()).filter(|k| crate::horschamp::dans_image(layout, *k, p.pos()));
            let marges = champs.iter().enumerate().filter(|(_, c)| c.contains(p.pos())).map(|(k, _)| k);
            for k in images.chain(marges) {
                if essayees.contains(&k) {
                    continue;
                }
                essayees.push(k);
                let Ok(rang) = self.case_recoit(layout, k, LayerKind::Vector) else { continue };
                let Some(pose) = crate::horschamp::pose_de_la_case(&self.doc, layout, k) else { continue };
                let pts: Option<Vec<VPoint>> = t.ligne.points.iter().map(|p| pose.dessin_sur(p.pos()).map(|q| VPoint { x: q.x, y: q.y, r: (p.r / pose.echelle_en(q).max(1e-6)).clamp(0.05, 400.0) })).collect();
                if let Some(points) = pts.filter(|v| !v.is_empty()) {
                    let echelle = pose.echelle_en(points[0].pos()).max(1e-6);
                    return Repose::Case { k, rang, points, echelle };
                }
            }
        }
        // Aucune case ne peut le recevoir : sur la planche, par-dessus l'image des cases qu'il traverse (partie 122), sous elles ailleurs.
        let pts: Vec<Pt> = t.ligne.points.iter().enumerate().map(|(i, p)| Pt {
            layer: *p,
            board: p.pos(),
            pressure: t.ligne.appui.get(i).copied().unwrap_or(1.0),
            time: t.ligne.instants.get(i).copied().unwrap_or(0.0),
        }).collect();
        let n = layout.cards.len();
        let owner = |p: Pos2| (0..n).find(|k| crate::horschamp::dans_image(layout, *k, p)).or_else(|| layout.annulee_en(p).map(|i| n + i));
        let suit = t.ligne.aligne();
        let troncons = crate::horschamp::trancher(&pts, &owner)
            .into_iter()
            .map(|(q, run)| {
                let c = q.and_then(|q| layout.cards.get(q).or_else(|| layout.annulees.get(q - n))).copied();
                let l = Ligne { points: run.iter().map(|p| p.layer).collect(), appui: if suit { run.iter().map(|p| p.pressure).collect() } else { Vec::new() }, instants: if suit { run.iter().map(|p| p.time).collect() } else { Vec::new() } };
                (c, l)
            })
            .collect();
        Repose::Planche { troncons }
    }

    /// Pose le trait préparé `p` (rien ne peut plus échouer : la place a été vérifiée). Les dessins touchés sont notés dans `avant` **avant** d'être changés
    /// (leur état d'avant entre dans l'étape).
    fn selp_poser(&mut self, layout: &BoardLayout, p: &APoser, avant: &mut Vec<(CelId, VectorDrawing)>) {
        let t = &p.trait_;
        match &p.repose {
            Repose::Planche { troncons } => {
                let recette = Recette { reglage: t.allure.brush, opacite: t.allure.opacity, durete: t.allure.hardness, toucher: t.allure.feel, couleur_b: t.allure.secondary };
                let alpha = (t.allure.opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
                for (sur, l) in troncons {
                    let brosse = Some((recette, l.appui.clone(), l.instants.clone(), t.allure.echelle));
                    let n = self.doc.annotations.traces.len();
                    let pose = match sur {
                        Some(c) => {
                            let rang = layout.cards.iter().position(|k| k.id == c.id).unwrap_or(0);
                            self.doc.annotations.ajouter_sur_la_case(c, rang, &l.points, t.allure.color, alpha, brosse)
                        }
                        None => self.doc.annotations.ajouter_hors_cadres_brosse(layout, &l.points, t.allure.color, alpha, brosse),
                    };
                    debug_assert!(pose, "la place a été vérifiée avant de rien retirer");
                    if let Some(tr) = self.doc.annotations.traces.get_mut(n).filter(|_| pose) {
                        tr.lien = Some(p.lien);
                    }
                }
                if let Some(id) = self.doc.annotations.id_hors_cadres() {
                    self.doc.annotations.regler_visible(id, true);
                }
            }
            Repose::Case { k, rang, points, echelle } => {
                let (k, rang) = (*k, *rang);
                let Some(cel) = self.cel_de_la_case(layout, k, rang) else { return };
                let rapport = t.allure.echelle / *echelle;
                let suit = t.ligne.aligne();
                let el = Element::Stroke {
                    points: points.clone(),
                    color: t.allure.color,
                    opacity: t.allure.opacity,
                    hardness: t.allure.hardness,
                    brush: t.allure.brush.map(|b| Box::new(crate::brush::BrushSettings { size: b.size * rapport, ..b })),
                    trace: VTrace { pressure: if suit { t.ligne.appui.clone() } else { Vec::new() }, time: if suit { t.ligne.instants.clone() } else { Vec::new() }, feel: t.allure.feel, secondary: t.allure.secondary, lien: Some(p.lien), cache: Default::default() },
                };
                if !avant.iter().any(|(c, _)| *c == cel) {
                    let d = self.drawing(cel).cloned().unwrap_or_default();
                    self.history.push(Edit::Vector { cel, drawing: d.clone() });
                    avant.push((cel, d));
                }
                let Some(d) = self.drawing_mut(cel) else { return };
                // Le dessin d'où il vient : il y garde son rang. Une case d'arrivée : dessus, ou dessous s'il était peint derrière.
                let i = match p.places.iter().find(|(c, _)| *c == cel) {
                    Some(&(_, i)) => i.min(d.elements.len()),
                    None if p.derriere => 0,
                    None => d.elements.len(),
                };
                d.elements.insert(i, el);
                // (La répartition se fait depuis le plan de la case, sur son calque : on y passe le temps de répartir.)
                let (cp, cf, cl) = (self.cur_panel, self.cur_frame, self.cur_layer);
                let c = layout.cards[k];
                self.cur_panel = c.panel;
                self.cur_frame = c.frame;
                self.cur_layer = rang;
                let fait = self.horschamp_repartir_element(layout, k, cel, i, p.derriere, &p.places);
                self.cur_panel = cp.min(self.doc.panels.len().saturating_sub(1));
                self.cur_frame = cf;
                self.cur_layer = cl;
                if !fait {
                    self.selp_refaire(cel);
                }
            }
        }
    }

    /// Le choix reçoit `aff` (planche) : chaque trait est rassemblé, transformé, puis réparti à nouveau ; les autres éléments sont transformés sur place.
    /// **Tout ou rien** : tout est calculé avant de changer quoi que ce soit ; si une part n'a nulle part où aller, rien ne bouge et la ligne d'état le dit.
    /// Une seule étape d'annulation, nommée `nom`.
    pub(crate) fn selp_reporter(&mut self, layout: &BoardLayout, aff: &Affine, nom: &str) {
        let Some(traits) = self.vignettes.selp.as_ref().filter(|_| self.selp_active()).map(|s| s.traits.clone()) else { return };
        if self.storyboard_refuses() {
            return;
        }
        let dessins = self.dessins_des_cases(layout);
        // 1. Le plan, sans rien écrire.
        let mut retires: Vec<(CelId, usize)> = Vec::new();
        for r in traits.iter().filter(|t| !t.objet).flat_map(|t| t.parts.iter()) {
            if let PartRef::Dessin { cel, rang, .. } = *r {
                retires.push((cel, rang));
            }
        }
        let mut a_poser: Vec<APoser> = Vec::new();
        for t in traits.iter().filter(|t| !t.objet) {
            let Some(r) = self.rassembler(layout, t) else { continue };
            let r = Rassemble { ligne: transformee(&r.ligne, aff), allure: r.allure };
            // Son numéro : le même si le trait repris est complet ; sinon un neuf (d'autres parts du même numéro restent ailleurs).
            let complet = r.allure.lien.is_some_and(|l| self.parts_du_lien(layout, &dessins, l).iter().all(|q| t.parts.contains(q)));
            let lien = match r.allure.lien {
                Some(l) if complet => l,
                _ => crate::horschamp::nouveau_lien(),
            };
            let mut places: Vec<(CelId, usize)> = Vec::new();
            let mut derriere = false;
            for q in &t.parts {
                if let PartRef::Dessin { cel, rang, .. } = *q {
                    let avant = retires.iter().filter(|(c, i)| *c == cel && *i < rang).count();
                    let i = rang - avant;
                    match places.iter_mut().find(|(c, _)| *c == cel) {
                        Some((_, j)) => *j = (*j).min(i),
                        None => places.push((cel, i)),
                    }
                    derriere |= rang == 0 && self.drawing(cel).is_some_and(|d| d.elements.len() > 1);
                }
            }
            let repose = self.selp_preparer(layout, &r);
            a_poser.push(APoser { trait_: r, repose, lien, places, derriere });
        }
        // La place sur la planche : celle que libèrent les parts retirées, contre celle que demandent les tronçons hors des cases.
        let liberees = traits.iter().filter(|t| !t.objet).flat_map(|t| t.parts.iter()).filter(|r| matches!(r, PartRef::Planche { .. })).count();
        let demandees: usize = a_poser.iter().map(|p| if let Repose::Planche { troncons } = &p.repose { troncons.len() } else { 0 }).sum();
        if demandees > 0 && (self.doc.annotations.traces.len() - liberees.min(self.doc.annotations.traces.len()) + demandees > crate::annotations::MAX_TRACES || layout.cards.is_empty()) {
            self.status = format!("La planche a atteint sa limite de {} traits : ce geste mènerait des traits hors des cases, il est refusé, rien n'a bougé.", crate::annotations::MAX_TRACES);
            return;
        }
        // Les autres éléments : leur transformation dans leur dessin (une case à la caméra qui ne voit pas leur nouvelle place : le geste est refusé).
        let mut objets: Vec<(CelId, usize, Element)> = Vec::new();
        for t in traits.iter().filter(|t| t.objet) {
            let Some(PartRef::Dessin { cel, case, rang }) = t.parts.first().copied() else { continue };
            let Some(k) = layout.cards.iter().position(|c| c.id == case) else { continue };
            let Some(pose) = crate::horschamp::pose_de_la_case(&self.doc, layout, k) else { continue };
            let Some(e) = self.drawing(cel).and_then(|d| d.elements.get(rang)).cloned() else { continue };
            let Some(local) = affinite_locale(&pose, aff, &pose, e.bounds().center()) else {
                self.status = "Ce geste mène un élément là où la caméra de sa case ne le voit plus : il est refusé, rien n'a bougé.".into();
                return;
            };
            objets.push((cel, rang, e.transformed(&local)));
        }
        // 2. Maintenant seulement, on écrit.
        let tenu = self.selp_apercu_formes(layout, Some(aff));
        let marque = self.history.marque();
        let before = self.history.context.replace(nom.to_string());
        self.history.push(self.snapshot());
        // (L'état d'avant de chaque dessin touché, rangé d'abord : il est rendu en dernier à l'annulation.)
        let mut avant: Vec<(CelId, VectorDrawing)> = Vec::new();
        let mut touches: Vec<CelId> = Vec::new();
        for c in retires.iter().map(|(c, _)| *c).chain(objets.iter().map(|(c, _, _)| *c)) {
            if !avant.iter().any(|(a, _)| *a == c) {
                if let Some(d) = self.drawing(c).cloned() {
                    self.history.push(Edit::Vector { cel: c, drawing: d.clone() });
                    avant.push((c, d));
                    touches.push(c);
                }
            }
        }
        for (cel, rang, e) in &objets {
            if let Some(slot) = self.drawing_mut(*cel).and_then(|d| d.elements.get_mut(*rang)) {
                *slot = e.clone();
            }
        }
        self.selp_retirer(&traits.iter().filter(|t| !t.objet).cloned().collect::<Vec<_>>());
        // (Les places les plus hautes d'abord : un trait posé ne décale pas la place d'un autre plus bas dans le même dessin.)
        a_poser.sort_by_key(|p| std::cmp::Reverse(p.places.iter().map(|(_, i)| *i).max().unwrap_or(usize::MAX)));
        let mut liens = Vec::new();
        for p in &a_poser {
            self.selp_poser(layout, p, &mut avant);
            liens.push(p.lien);
        }
        for c in touches {
            self.selp_refaire(c);
        }
        self.history.regrouper_depuis(marque);
        self.history.context = before;
        self.modified = true;
        self.renderer.invalidate_all();
        // Le choix suit à sa nouvelle place.
        let layout = self.board_layout();
        let dessins = self.dessins_des_cases(&layout);
        let mut nouveaux: Vec<TraitEntier> = Vec::new();
        for lien in liens {
            if let Some(r) = self.parts_du_lien(&layout, &dessins, lien).into_iter().find(|r| !nouveaux.iter().any(|t| t.parts.contains(r))) {
                nouveaux.push(self.entier_de_avec(&layout, &dessins, r));
            }
        }
        for (cel, _, e) in &objets {
            if let Some(i) = self.drawing(*cel).and_then(|d| d.elements.iter().position(|x| x == e)) {
                if let Some(&(k, _)) = dessins.iter().find(|(_, c)| c == cel) {
                    nouveaux.push(TraitEntier { parts: vec![PartRef::Dessin { cel: *cel, case: layout.cards[k].id, rang: i }], objet: true });
                }
            }
        }
        let serial = self.history.serial();
        if let Some(s) = self.vignettes.selp.as_mut() {
            s.traits = nouveaux;
            s.serial = serial;
            s.pivot = None;
            *s.vue.borrow_mut() = None;
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

    /// Suppr : le choix part, toutes les parts des traits, en une étape.
    pub(crate) fn selp_supprimer(&mut self) {
        let Some(traits) = self.vignettes.selp.as_ref().filter(|_| self.selp_active()).map(|s| s.traits.clone()) else { return };
        if self.storyboard_refuses() {
            return;
        }
        let marque = self.history.marque();
        let before = self.history.context.replace("Effacer".to_string());
        let snap = self.snapshot();
        let avant = self.selp_retirer(&traits);
        self.history.push(snap);
        for (cel, d) in &avant {
            self.history.push(Edit::Vector { cel: *cel, drawing: d.clone() });
        }
        for (c, _) in &avant {
            self.selp_refaire(*c);
        }
        self.history.regrouper_depuis(marque);
        self.history.context = before;
        self.modified = true;
        self.renderer.invalidate_all();
        self.vignettes.selp = None;
        let n = traits.len();
        self.status = format!("{n} élément{} effacé{}, entier{}.", if n > 1 { "s" } else { "" }, if n > 1 { "s" } else { "" }, if n > 1 { "s" } else { "" });
    }

    /// Ctrl+C : le choix, entier, dans le presse-papiers des éléments, à sa place dans le dessin de la case courante. Vrai si copié.
    pub(crate) fn selp_copier(&mut self) -> bool {
        let Some(traits) = self.vignettes.selp.as_ref().filter(|_| self.selp_active()).map(|s| s.traits.clone()) else { return false };
        let layout = self.board_layout();
        let cur = self.cur_frame.min(self.frame_count().saturating_sub(1));
        let Some(kc) = layout.cards.iter().position(|c| c.panel == self.cur_panel && (c.frame..c.end).contains(&cur)) else { return false };
        let Some(pose) = crate::horschamp::pose_de_la_case(&self.doc, &layout, kc) else { return false };
        let mut items = Vec::new();
        for t in &traits {
            if t.objet {
                let Some(PartRef::Dessin { cel, case, rang }) = t.parts.first().copied() else { continue };
                let Some(k) = layout.cards.iter().position(|c| c.id == case) else { continue };
                let Some(pk) = crate::horschamp::pose_de_la_case(&self.doc, &layout, k) else { continue };
                let Some(e) = self.drawing(cel).and_then(|d| d.elements.get(rang)).cloned() else { continue };
                if let Some(local) = affinite_locale(&pk, &Affine::IDENTITY, &pose, e.bounds().center()) {
                    items.push(e.transformed(&local));
                }
                continue;
            }
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
        self.status = format!("{} élément(s) copié(s), entiers.", items.len());
        self.ctx.copy_text(format!("{}{} élément(s) vectoriel(s)", crate::pasteimage::MARK, items.len()));
        self.clipboard = items;
        self.collage.origines.elements = Some(crate::adaptation::Origine::de(&self.doc));
        self.multi_clip = None;
        true
    }

    /// Recolorer : toutes les parts (et les autres éléments) prennent la couleur courante, en une étape. Une part de la planche au pinceau refait son rendu
    /// (sa couleur est dans ses pixels).
    pub(crate) fn selp_recolorer(&mut self) {
        let Some(traits) = self.vignettes.selp.as_ref().filter(|_| self.selp_active()).map(|s| s.traits.clone()) else { return };
        let color = self.color;
        let marque = self.history.marque();
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
                        t.brosse = t.brosse.as_ref().map(|b| std::sync::Arc::new(b.refaite()));
                    }
                }
            }
        }
        for c in touches {
            self.selp_refaire(c);
        }
        self.history.regrouper_depuis(marque);
        self.history.context = before;
        self.modified = true;
        self.renderer.invalidate_all();
        let serial = self.history.serial();
        if let Some(s) = self.vignettes.selp.as_mut() {
            s.serial = serial;
            *s.vue.borrow_mut() = None;
        }
    }

    /// Ctrl+V dans les Vignettes : les éléments collés dans le dessin `cel` (rangs `rangs`) sont répartis comme des traits qu'on vient de tracer, et
    /// deviennent le choix de la planche. L'étape du collage (d'après `marque`) les rejoint.
    pub(crate) fn selp_apres_le_collage(&mut self, cel: CelId, rangs: std::ops::Range<usize>, marque: u64) {
        let layout = self.board_layout();
        let cur = self.cur_frame.min(self.frame_count().saturating_sub(1));
        let Some(k) = layout.cards.iter().position(|c| c.panel == self.cur_panel && (c.frame..c.end).contains(&cur)) else { return };
        let mut liens = Vec::new();
        let mut objets = Vec::new();
        for i in rangs.rev() {
            let lien = crate::horschamp::nouveau_lien();
            match self.drawing_mut(cel).and_then(|d| d.elements.get_mut(i)) {
                Some(Element::Stroke { trace, .. }) => trace.lien = Some(lien),
                Some(e) => {
                    objets.push(e.clone());
                    continue;
                }
                None => continue,
            }
            liens.push(lien);
            self.horschamp_repartir_element(&layout, k, cel, i, false, &[]);
        }
        self.history.regrouper_depuis(marque);
        self.vsel = None;
        let layout = self.board_layout();
        let dessins = self.dessins_des_cases(&layout);
        let mut traits: Vec<TraitEntier> = Vec::new();
        for lien in liens {
            if let Some(r) = self.parts_du_lien(&layout, &dessins, lien).into_iter().next() {
                traits.push(self.entier_de_avec(&layout, &dessins, r));
            }
        }
        if let Some(d) = self.drawing(cel) {
            for e in &objets {
                if let Some(i) = d.elements.iter().position(|x| x == e) {
                    traits.push(TraitEntier { parts: vec![PartRef::Dessin { cel, case: layout.cards[k].id, rang: i }], objet: true });
                }
            }
        }
        let serial = self.history.serial();
        self.vignettes.selp = Some(SelPlanche { traits, serial, ..SelPlanche::default() });
    }

    /// Les formes du choix à l'écran, à la place où le geste `aff` le mène (rien : à sa place) : des lignes de sa couleur, par-dessus tout.
    fn selp_apercu_formes(&self, layout: &BoardLayout, aff: Option<&Affine>) -> Vec<(Rect, egui::Shape)> {
        let Some(v) = self.selp_vue(layout) else { return Vec::new() };
        let xf = self.vignettes.xf;
        let mut out = Vec::new();
        for (pts, couleur) in &v.entiers {
            let l = Ligne { points: pts.clone(), ..Ligne::default() };
            let l = match aff {
                Some(a) => transformee(&l, a),
                None => l,
            };
            let ecran: Vec<Pos2> = l.points.iter().map(|p| xf.to_screen(p.pos())).collect();
            if ecran.len() < 2 {
                continue;
            }
            let w = (l.points.iter().map(|p| p.r).sum::<f32>() / l.points.len() as f32 * xf.zoom * 2.0).max(1.0);
            let b = Rect::from_points(&ecran).expand(w);
            out.push((b, egui::Shape::line(ecran.clone(), Stroke::new(w, *couleur))));
            for e in [ecran[0], ecran[ecran.len() - 1]] {
                out.push((Rect::from_center_size(e, egui::Vec2::splat(w)), egui::Shape::circle_filled(e, w * 0.5, *couleur)));
            }
        }
        out
    }

    /// Le choix de la planche à l'écran : chaque trait entier souligné, chaque autre élément encadré, le lasso en cours, la boîte et ses poignées ; pendant
    /// un geste, à leur nouvelle place. (Calculé une fois tant que rien ne change : `selp_vue`.)
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
        let Some(v) = self.selp_vue(layout) else { return };
        let geste = s.geste.as_ref().filter(|g| g.change && g.handle != Handle::Pivot).map(|g| g.aff);
        let vers = |p: Pos2| xf.to_screen(geste.map_or(p, |a| a.apply(p)));
        if let Some(aff) = geste {
            for (_, sh) in self.selp_apercu_formes(layout, Some(&aff)) {
                clip.add(sh);
            }
        }
        for pts in &v.parts {
            let e: Vec<Pos2> = pts.iter().map(|p| vers(p.pos())).collect();
            if e.len() >= 2 && Rect::from_points(&e).intersects(view) {
                clip.add(egui::Shape::line(e, Stroke::new(1.5_f32, ACCENT)));
            }
        }
        for q in &v.objets {
            clip.add(egui::Shape::closed_line(q.iter().map(|p| vers(*p)).collect(), Stroke::new(1.0_f32, ACCENT)));
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
        let _ = pos2;
    }
}
