//! Dessiner partout sur la planche des Vignettes (Anthony, 2026-10-30 : « tout le canevas est dessinable »), et **la caméra se souvient**
//! (partie 131, Anthony, 2026-11-09 : « une grande planche de dessin sur laquelle des caméras sont posées » ; son choix parmi cinq approches :
//! « Ok on essaye A »).
//!
//! # La règle (partie 131)
//!
//! **Un trait appartient tout entier à la case où le stylet s'est posé ; hors de toute case, il appartient au papier. Le bord d'une case est une
//! étiquette, jamais des ciseaux.** Avant (parties 111 à 128), chaque point du trait cherchait sa case au lâcher et le trait était coupé en
//! parts (la case d'origine, les voisines, le papier), reliées par un numéro (`vector::Trace::lien`). Il reste entier, à un seul endroit.
//!
//! - **Le champ d'une case** : son image plus le débord réglé dans Préférences > Vignettes (12 % de sa largeur d'origine) ; pour la case d'un
//!   mouvement de caméra, la tuile de son empreinte. **L'intérieur du cadre d'une case a toujours priorité sur la marge d'une autre** (partie 111).
//! - **Le propriétaire d'un trait est celui de son premier point** (`proprietaire_cadre`) : la case dont l'image le contient, sinon celle dont
//!   la marge le contient (deux marges qui se recouvrent : la case où le trait se trace, sinon la première dans l'ordre de lecture), sinon une
//!   case pâle (son cadre ou sa marge : son plan naît à l'appui, `boardui::board_press`), sinon **le papier**.
//! - **Dans une case** : le trait reste dans le dessin de son plan, **où qu'il aille ensuite**. La planche le montre en entier (la vignette de
//!   la case s'étend jusqu'à l'encre du plan, `render::marges_de_l_encre`), sur le papier et **par-dessus l'image des autres cases**
//!   (`App::board_debords`) ; le film, l'animatique et les exports du plan restent coupés au cadre de la caméra. Rien n'est écrit au lâcher.
//! - **La feuille ou le film** : sur la planche rien n'est caché par une case (le papier aussi se peint par-dessus les images, sous les filets,
//!   les numéros et les notes) ; avec le **passe-partout**, chaque case ne montre que son plan et tout le reste passe dessous
//!   (`boardui::board_view`). L'export de la planche suit la même règle (`planche::composer_avec_tuiles`).
//! - **Hors de toute case** (entre les cases, autour, sous une barre, dans une bande de note) : le trait se trace dans le dessin de la case la
//!   plus proche le temps du geste (`Vignettes::appui_bande`, l'aperçu déborde, `debord.rs`), et **au lâcher** (jamais pendant le geste : aucune
//!   latence ajoutée) il passe **entier** sur la planche (`App::horschamp_partager`), dans le calque « Hors cadres »
//!   (`Annotations::calque_hors_cadres`, un calque d'annotation de la planche marqué `planche`) : visible, enregistré, relu, dans l'export de la
//!   planche, et dans aucun plan, ni l'animatique, ni le film, ni le storyboard en PDF. Il y garde la recette de son pinceau (`horschamptex.rs`) ;
//!   un trait **bitmap** y garde ses pixels mêmes, sur la couche bitmap de la planche (`planchebitmap.rs`, partie 127 ; parti d'une vue en
//!   perspective : un trait vectoriel au pinceau). **Une seule étape d'annulation.**
//! - **Le bout d'un trait ne fait plus naître la case pâle suivante** (c'était la partie 113, décision 8) : un trait appartient à la case où il
//!   commence ; une case pâle naît quand on pose le stylet dedans ou dans sa marge.
//! - **Les deux gestes qui déplacent le dessin** (Anthony : une caméra qu'on déplace avec son dessin ; un mouvement de caméra qui fait de la
//!   place) n'ont rien de plus à faire : le dessin est rangé dans le plan, il suit sa case ; le papier suit la case à laquelle il est attaché.
//!
//! # Les outils
//!
//! - **Pinceau, crayon, ligne, rectangle, ellipse, courbe, polyligne** : partout, avec la règle ci-dessus.
//! - **Gomme** : dans une case, elle agit sur le dessin du plan, où qu'il soit. Partie hors de toute case, elle efface les traits du calque
//!   « Hors cadres » qu'elle touche (le trait entier, comme la gomme d'annotation). **Pot de peinture** : seulement dans une case (une zone
//!   fermée de la feuille). **Sélection de la planche** (Ctrl) : `vignettesel.rs`.
//!
//! # La case qui ne peut pas recevoir (partie 122)
//!
//! - **Toute caméra reçoit** : un autre zoom, un roulis, une bascule, un panoramique, un plan à cadres (`Pose`).
//! - **Refusé, et jamais caché** : un calque verrouillé, masqué, transformé ou d'une autre sorte, un plan tenu par un autre poste, un plan
//!   écarté. On n'écrit pas dans son dessin. Un trait qui devrait y aller (son premier point est dans son image : un trait qu'un déplacement
//!   de la sélection y mène, un trait tracé depuis une case écartée) reste **entier** sur la planche, **par-dessus l'image de la case**
//!   (`Trace::dessus`), attaché à elle, et la ligne d'état dit pourquoi.
//!
//! # Les projets d'avant
//!
//! Ouverts tels quels : les parts d'un même numéro restent des parts, peintes comme avant, que la sélection prend toujours ensemble ; un
//! trait coupé redevient entier dès qu'on le déplace (`vignettesel.rs`). Le format du projet ne change pas.

use eframe::egui::{pos2, vec2, Color32, Pos2, Rect, Vec2};

use crate::app::App;
use crate::board::{BoardLayout, CardPlace};
use crate::document::{CelId, LayerKind};
use crate::history::Edit;
use crate::horschamptex::Recette;
use crate::persp::{self, Homography};
use crate::raster::{Tile, TILE};
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

/// Au-delà, un point n'a plus de place raisonnable dans un dessin (pixels).
const DESSIN_MAX: f32 = 200_000.0;
/// Le travail d'un rééchantillonnage, en lectures de pixels, sur le fil de l'interface au lever : au-delà, la part reste sur la planche plutôt que de geler l'écran
/// (`mesure_le_cout_du_lever_a_fort_zoom`).
const LECTURES_MAX: u64 = 1_500_000;
/// La taille d'un trait lisse redessiné dans une case, en pixels de sa boîte.
const TRAIT_LISSE_MAX: f32 = 16_000_000.0;

/// Comment un dessin se pose sur la planche, **quelle que soit la caméra** de sa case : une homographie et son inverse (une caméra droite : une simple
/// échelle ; un roulis : une rotation ; une bascule ou un panoramique : une perspective).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Pose {
    vers_planche: Homography,
    vers_dessin: Homography,
}

impl Pose {
    pub fn de_aff(a: &Aff) -> Option<Pose> {
        let h = [a.a, a.c, a.tx, a.b, a.d, a.ty, 0.0, 0.0, 1.0];
        Some(Pose { vers_planche: h, vers_dessin: persp::inverse(&h)? })
    }

    /// Le point du dessin → la planche.
    pub fn planche(&self, p: Pos2) -> Pos2 {
        persp::apply(&self.vers_planche, p)
    }

    /// Le point de la planche → le dessin.
    pub fn dessin(&self, p: Pos2) -> Pos2 {
        persp::apply(&self.vers_dessin, p)
    }

    /// Unités de la planche par pixel du dessin, autour du point `p` du dessin.
    pub fn echelle_en(&self, p: Pos2) -> f32 {
        let c = self.planche(p);
        let (dx, dy) = (self.planche(p + vec2(1.0, 0.0)) - c, self.planche(p + vec2(0.0, 1.0)) - c);
        (dx.x * dy.y - dx.y * dy.x).abs().sqrt()
    }

    /// Le point de la planche → le dessin, s'il y a un sens : devant la caméra, à une distance raisonnable (tout près de l'horizon d'une caméra tournée, non).
    pub fn dessin_sur(&self, p: Pos2) -> Option<Pos2> {
        let h = &self.vers_dessin;
        if h[6] * p.x + h[7] * p.y + h[8] <= 0.0 {
            return None;
        }
        let q = self.dessin(p);
        (q.x.is_finite() && q.y.is_finite() && q.x.abs() < DESSIN_MAX && q.y.abs() < DESSIN_MAX).then_some(q)
    }

    /// Une vraie perspective (bascule, panoramique), pas une simple affinité.
    pub fn en_perspective(&self) -> bool {
        let h = &self.vers_planche;
        // (Un panoramique de 11 degrés : de l'ordre de 1e-4 ; une simple affinité : zéro, au bruit de calcul près.)
        h[6].abs() + h[7].abs() > h[8].abs() * 1e-6
    }

    /// Du dessin à la planche, si c'est une affinité (pas une perspective), en double précision (`planchebitmap::A64`).
    pub fn affine64(&self) -> Option<crate::planchebitmap::A64> {
        if self.en_perspective() {
            return None;
        }
        let h = self.vers_planche.map(f64::from);
        let w = h[8];
        let a = crate::planchebitmap::A64 { a: h[0] / w, b: h[1] / w, tx: h[2] / w, c: h[3] / w, d: h[4] / w, ty: h[5] / w };
        (a.fini() && a.inverse().is_some()).then_some(a)
    }
}

/// Le geste en cours dans la case vive : où il est, comment le dessin se pose sur la planche, d'où il part (`horschamp_suivre`).
#[derive(Clone, Debug)]
pub(crate) struct Suivi {
    pub carte: u64,
    pub aff: Aff,
    pub pose: Pose,
    /// L'épaisseur du trait sur la planche pendant le geste : unités de la planche par pixel du dessin. Le trait qui sort de la case a cette épaisseur
    /// partout (`debord.rs`), même dans une vue en perspective où la feuille, elle, s'étire : au lever, il la garde.
    pub a: f32,
    /// Le nombre d'étapes d'annulation avant le geste.
    pub base_len: usize,
    /// La marque de l'historique avant le geste (`History::marque`) : les étapes du trait se regroupent depuis elle, même historique plein.
    pub marque: u64,
    /// Le nombre d'éléments du dessin courant avant le geste (calque vectoriel).
    pub elements_avant: usize,
}

/// Ce que le geste a laissé, noté à son lâcher pour être réparti à la fin de l'image d'écran.
#[derive(Debug)]
pub(crate) enum Matiere {
    /// Un calque vectoriel : le dernier élément du dessin ; `derriere` : le premier, un trait peint derrière (« Peindre derrière ») est posé sous les autres
    /// éléments (`commit_vector_stroke`) : s'il passe au papier, il y reste derrière aussi.
    Vecteur { derriere: bool },
    /// Un calque bitmap : les tuiles d'avant le trait, et son tracé (ligne médiane, rayon), sa couleur, son opacité ; la recette du pinceau
    /// (son réglage, et l'appui et l'instant de chaque point du tracé, vides s'ils ne le suivent pas) pour que la part qui sort de la case
    /// garde la texture du pinceau sur la planche (`horschamptex.rs`).
    /// `derriere` : peint derrière (« Peindre derrière ») : sa part dans une autre case passe sous son dessin, son morceau de planche sous les autres.
    Bitmap { avant: Vec<(usize, Option<Tile>)>, path: Vec<VPoint>, couleur: [u8; 3], alpha: f32, recette: Option<Recette>, appui: Vec<f32>, instants: Vec<f64>, derriere: bool },
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
    /// Unités de la planche par pixel du dessin d'où vient le tronçon.
    pub a: f32,
    /// La case qu'il recouvre, s'il tombe dans l'image d'une case qui ne peut pas le recevoir.
    pub sur: Option<CardPlace>,
}

impl Course {
    /// Le tronçon `run`, `a` : unités de la planche par pixel du dessin (`Suivi::a`) ; `suit` : les appuis et les instants sont ceux du tracé.
    pub fn de(run: &[Pt], a: f32, suit: bool) -> Course {
        Course {
            points: run.iter().map(|p| VPoint { x: p.board.x, y: p.board.y, r: p.layer.r * a }).collect(),
            appui: if suit { run.iter().map(|p| p.pressure).collect() } else { Vec::new() },
            instants: if suit { run.iter().map(|p| p.time).collect() } else { Vec::new() },
            a,
            sur: None,
        }
    }
}

/// Ce qui empêche une case de recevoir la part d'un trait : la ligne d'état le dit.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Refus {
    Verrouille(String),
    Masque(String),
    Transforme(String),
    AutreSorte(LayerKind),
    SansDessin(String),
    TenuAilleurs,
    Camera,
    /// La caméra de la case cadre si large que ce bout du trait y serait trop grand pour son dessin (bitmap, ou tout près de l'horizon d'une caméra tournée).
    TropGrand,
}

/// Le code d'une part qui tombe dans l'image d'une case qui ne peut pas la recevoir : `DESSUS + k` pour la case `k`, `DESSUS + nombre de cases + i`
/// pour la case écartée `i` (`proprietaire_cadre`).
pub(crate) const DESSUS: usize = 1 << 30;

#[cfg(test)]
thread_local! {
    /// Essais : vrai, un trait est réparti au lâcher **comme avant la partie 131** (chaque point à la case qu'il recouvre, le trait coupé en parts
    /// de même numéro). De quoi fabriquer, dans un essai, les traits coupés d'un projet d'avant : ils s'ouvrent tels quels, la sélection les prend
    /// entiers et les recolle quand on les déplace.
    pub(crate) static COMME_AVANT_131: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Répartir comme avant la partie 131 ? Jamais hors des essais (`COMME_AVANT_131`).
fn comme_avant_131() -> bool {
    #[cfg(test)]
    {
        COMME_AVANT_131.with(std::cell::Cell::get)
    }
    #[cfg(not(test))]
    {
        false
    }
}

/// Un numéro neuf pour les parts d'un trait (partie 125) : unique dans le projet en pratique (l'heure en nanosecondes mêlée à un compteur), enregistré.
pub(crate) fn nouveau_lien() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COMPTEUR: AtomicU64 = AtomicU64::new(1);
    let n = COMPTEUR.fetch_add(1, Ordering::Relaxed);
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
    (t ^ n.wrapping_mul(0x9E37_79B9_7F4A_7C15)).max(1)
}

/// La case du code `q` (`DESSUS`…).
pub(crate) fn case_du_code(layout: &BoardLayout, q: usize) -> Option<&CardPlace> {
    let i = q.checked_sub(DESSUS)?;
    layout.cards.get(i).or_else(|| layout.annulees.get(i - layout.cards.len()))
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
/// Les règles d'ordre de `proprietaire` ne jouent qu'entre deux marges. Dans l'image d'une case qui ne peut pas recevoir la part, ou d'une case écartée :
/// le code `DESSUS` de cette case, la part reste sur la planche, par-dessus elle (partie 122).
pub(crate) fn proprietaire_cadre(layout: &BoardLayout, champs: &[Rect], recoit: &[bool], origine: usize, p: Pos2) -> Option<usize> {
    if dans_image(layout, origine, p) {
        return Some(origine);
    }
    if let Some(k) = (0..layout.cards.len()).find(|k| *k != origine && dans_image(layout, *k, p)) {
        return Some(if recoit.get(k).copied().unwrap_or(false) { k } else { DESSUS + k });
    }
    if let Some(i) = layout.annulee_en(p) {
        return Some(DESSUS + layout.cards.len() + i);
    }
    proprietaire(champs, recoit, origine, p)
}

/// Comment le dessin de la case `k` se pose sur la planche, quelle que soit sa caméra (un calque sans transformation).
pub(crate) fn pose_de_la_case(doc: &crate::document::Document, layout: &BoardLayout, k: usize) -> Option<Pose> {
    if let Some(a) = aff_de_la_case(doc, layout, k) {
        return Pose::de_aff(&a);
    }
    let c = layout.cards.get(k)?;
    let cam = doc.panel_camera(c.panel, c.frame);
    let e = c.thumb.width() / doc.frame[0].max(1) as f32;
    if !e.is_finite() || e <= 1e-6 {
        return None;
    }
    let image_vers_planche: Homography = [e, 0.0, c.thumb.min.x, 0.0, e, c.thumb.min.y, 0.0, 0.0, 1.0];
    let planche_vers_image: Homography = [1.0 / e, 0.0, -c.thumb.min.x / e, 0.0, 1.0 / e, -c.thumb.min.y / e, 0.0, 0.0, 1.0];
    Some(Pose { vers_planche: persp::mul(&image_vers_planche, &cam.homography_inverse(doc.frame)), vers_dessin: persp::mul(&cam.homography(doc.frame), &planche_vers_image) })
}

/// Comment le dessin de la case `k` se pose sur la planche, si c'est une échelle et un décalage : rien si sa caméra n'est pas droite (bascule, panoramique, roulis ; voir `pose_de_la_case`).
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

/// Les pixels `pixels` du dessin d'origine, posé par `origine`, tels que les voit le dessin posé par `cible` : le même trait à la même place de la
/// planche, à une autre échelle ou sous une autre caméra. Échantillonnage inverse (bilinéaire, moyenné quand le dessin d'arrivée est plus grossier). **Rien
/// si ce travail n'a pas sa place au lever** (`LECTURES_MAX`), ou si un pixel n'a pas de place dans le dessin d'arrivée : l'appelant n'a encore rien changé, la
/// part reste sur la planche. Jamais un résultat partiel : les pixels sont retirés du dessin d'origine.
fn reechantillonner(pixels: &[(i64, i64, Color32)], origine: &Pose, cible: &Pose) -> Option<Vec<(i64, i64, Color32)>> {
    use std::collections::HashMap;
    const NB: usize = TILE * TILE;
    let t = TILE as i64;
    // Chaque tuile d'origine, et la boîte des pixels qu'elle porte : on ne lit que là où il y a du trait.
    let mut source: HashMap<(i64, i64), (Box<[Color32; NB]>, [i64; 4])> = HashMap::new();
    for &(x, y, c) in pixels {
        let (tuile, boite) = source.entry((x.div_euclid(t), y.div_euclid(t))).or_insert_with(|| (Box::new([Color32::TRANSPARENT; NB]), [x, y, x, y]));
        tuile[(y.rem_euclid(t) * t + x.rem_euclid(t)) as usize] = c;
        *boite = [boite[0].min(x), boite[1].min(y), boite[2].max(x), boite[3].max(y)];
    }
    let vers_source: Homography = persp::mul(&origine.vers_dessin, &cible.vers_planche);
    // Le premier passage : le travail à faire, avant d'en faire la moindre part.
    let mut plan: Vec<(Rect, i64)> = Vec::with_capacity(source.len());
    let mut lectures = 0_u64;
    for (_, boite) in source.values() {
        let coin = |x: i64, y: i64| cible.dessin_sur(origine.planche(pos2(x as f32, y as f32)));
        let coins = [coin(boite[0] - 1, boite[1] - 1)?, coin(boite[2] + 2, boite[1] - 1)?, coin(boite[2] + 2, boite[3] + 2)?, coin(boite[0] - 1, boite[3] + 2)?];
        let arrivee = Rect::from_points(&coins);
        // Combien de pixels d'origine par pixel d'arrivée : quatre lectures au plus par côté.
        let par_px = origine.echelle_en(pos2(boite[0] as f32, boite[1] as f32)) / cible.echelle_en(coins[0]).max(1e-6);
        let n = (1.0 / par_px.max(1e-6)).ceil().clamp(1.0, 4.0) as i64;
        lectures += (f64::from(arrivee.width().max(1.0)) * f64::from(arrivee.height().max(1.0))) as u64 * (n * n) as u64;
        if lectures > LECTURES_MAX {
            return None;
        }
        plan.push((arrivee, n));
    }
    // (La dernière tuile lue reste à portée : presque toutes les lectures y tombent, une recherche par lecture se sentait.)
    let mut derniere: ((i64, i64), Option<&[Color32; NB]>) = ((i64::MAX, i64::MAX), None);
    let mut lire = |x: i64, y: i64| {
        let k = (x.div_euclid(t), y.div_euclid(t));
        if derniere.0 != k {
            derniere = (k, source.get(&k).map(|s| &*s.0));
        }
        derniere.1.map_or([0.0_f32; 4], |tuile| {
            let c = tuile[(y.rem_euclid(t) * t + x.rem_euclid(t)) as usize];
            [f32::from(c.r()), f32::from(c.g()), f32::from(c.b()), f32::from(c.a())]
        })
    };
    let mut arrivee: HashMap<(i64, i64), Box<[Color32; NB]>> = HashMap::new();
    for (boite, n) in plan {
        let n = n.max(1);
        for y in boite.top().floor() as i64..boite.bottom().ceil() as i64 {
            for x in boite.left().floor() as i64..boite.right().ceil() as i64 {
                let mut somme = [0.0_f32; 4];
                for j in 0..n {
                    for i in 0..n {
                        let s = persp::apply(&vers_source, pos2(x as f32 + (i as f32 + 0.5) / n as f32, y as f32 + (j as f32 + 0.5) / n as f32)) - vec2(0.5, 0.5);
                        let (sx, sy) = (s.x.floor(), s.y.floor());
                        let (fx, fy) = (s.x - sx, s.y - sy);
                        let (sx, sy) = (sx as i64, sy as i64);
                        let (a, b, c, d) = (lire(sx, sy), lire(sx + 1, sy), lire(sx, sy + 1), lire(sx + 1, sy + 1));
                        for k in 0..4 {
                            let haut = a[k] + (b[k] - a[k]) * fx;
                            let bas = c[k] + (d[k] - c[k]) * fx;
                            somme[k] += haut + (bas - haut) * fy;
                        }
                    }
                }
                let v = somme.map(|s| (s / (n * n) as f32).round().clamp(0.0, 255.0) as u8);
                if v[3] > 0 {
                    let tuile = arrivee.entry((x.div_euclid(t), y.div_euclid(t))).or_insert_with(|| Box::new([Color32::TRANSPARENT; NB]));
                    tuile[(y.rem_euclid(t) * t + x.rem_euclid(t)) as usize] = Color32::from_rgba_premultiplied(v[0], v[1], v[2], v[3]);
                }
            }
        }
    }
    let mut out = Vec::new();
    for ((tx, ty), tuile) in &arrivee {
        for (k, c) in tuile.iter().enumerate() {
            if c.a() > 0 {
                out.push((tx * t + (k % TILE) as i64, ty * t + (k / TILE) as i64, *c));
            }
        }
    }
    Some(out)
}

/// Le tronçon `run` d'un trait bitmap, redessiné dans le dessin posé par `cible` : un trait lisse (sans le moteur des pinceaux, aucun rejeu), de l'épaisseur
/// `a` sur la planche : ses pixels. Rien s'il n'a pas de place dans ce dessin (l'appelant n'a encore rien changé).
fn trait_lisse(run: &[Pt], a: f32, cible: &Pose, couleur: [u8; 3], alpha: f32, durete: f32) -> Option<Vec<(i64, i64, Color32)>> {
    let mut points: Vec<VPoint> = Vec::with_capacity(run.len());
    for p in run {
        let q = cible.dessin_sur(p.board)?;
        points.push(VPoint { x: q.x, y: q.y, r: (p.layer.r * a / cible.echelle_en(q).max(1e-6)).clamp(0.2, 200.0) });
    }
    let boite = points.iter().fold(Rect::NOTHING, |b, p| b.union(Rect::from_center_size(p.pos(), Vec2::splat(p.r * 2.0 + 2.0))));
    if boite.width() * boite.height() > TRAIT_LISSE_MAX {
        return None;
    }
    let (raster, (ox, oy)) = crate::vector::render_isole(&points, couleur, alpha, durete, None, &VTrace::default())?;
    let mut out = Vec::new();
    for (k, t) in raster.tiles.iter().enumerate() {
        let Some(t) = t else { continue };
        let (x0, y0, tw, th) = raster.tile_bounds(k);
        for y in 0..th {
            for x in 0..tw {
                let c = t[y * TILE + x];
                if c.a() > 0 {
                    out.push(((x0 + x) as i64 + ox, (y0 + y) as i64 + oy, c));
                }
            }
        }
    }
    Some(out)
}

impl App {
    /// Le dessin hors des cases : calque « Hors cadres » (s'il est montré), à l'opacité du réglage ; la planche le peint par-dessus les images
    /// des cases (la feuille, partie 131), ou dessous avec le passe-partout (le film). `dessus` : la seconde passe, toujours après les images, pour
    /// les traits posés par-dessus l'image d'une case qui ne pouvait pas les recevoir (partie 122), pleins : ils sont dans une case, pas hors champ.
    pub(crate) fn hors_cadres_peindre(&self, painter: &eframe::egui::Painter, view: Rect, xf: crate::boardui::Xf, layout: &BoardLayout, dessus: bool) {
        let a = &self.doc.annotations;
        let mut textures = self.vignettes.hc_tex.borrow_mut();
        if a.id_hors_cadres().is_none() {
            if textures.nombre() > 0 {
                *textures = Default::default();
            }
            let mut pb = self.vignettes.pb_tex.borrow_mut();
            if pb.nombre() > 0 {
                *pb = Default::default();
            }
            return;
        }
        let opacite = if dessus { 1.0 } else { f32::from(self.settings.vignettes.opacite_hors_champ.min(100)) / 100.0 };
        let teinte = Color32::from_white_alpha((opacite * 255.0).round() as u8);
        if !dessus {
            textures.debut();
        }
        // La couche bitmap de la planche (partie 127) : ses pixels, sous les traits vectoriels hors des cases.
        {
            let mut pb = self.vignettes.pb_tex.borrow_mut();
            if !dessus {
                pb.debut();
            }
            if a.hors_cadres_montre().is_some() && !a.bitmap.morceaux.is_empty() && !pb.peindre(painter, view, xf, layout, &a.bitmap, dessus, opacite) {
                painter.ctx().request_repaint();
            }
        }
        for i in a.ordre_de_peinture() {
            let t = &a.traces[i];
            if t.dessus != dessus || !a.calque(t.calque).is_some_and(|c| c.planche) {
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
        let (base_len, marque, elements_avant, pose_deja) = match deja {
            Some(s) => (s.base_len, s.marque, s.elements_avant, Some(s.pose)),
            None => (self.history.undo_len(), self.history.marque(), self.cel_here().and_then(|c| self.drawing(c)).map_or(0, |d| d.elements.len()), None),
        };
        if deja.is_none() {
            self.vignettes.refus_dits.clear();
            self.vignettes.refus_phrase = None;
        }
        // (Une vue en perspective ou une carte inclinée ne se résument pas à trois points : la pose exacte.)
        let pose = if self.view.persp.is_some() || self.layer().pose(self.cur_frame).is_tilted() { self.pose_de_la_vue().or(pose_deja) } else { Pose::de_aff(&aff).or(pose_deja) };
        let Some(pose) = pose else {
            self.vignettes.suivi = None;
            return;
        };
        let a = if self.view.persp.is_some() { self.layer_zoom() / xf.zoom.max(1e-6) } else { pose.echelle_en(Pos2::ZERO) };
        self.vignettes.suivi = Some(Suivi { carte, aff, pose, a, base_len, marque, elements_avant });
    }

    /// La pose du dessin du calque courant, d'après les quatre coins de la feuille vus par la vue courante (une homographie les porte tous).
    fn pose_de_la_vue(&self) -> Option<Pose> {
        let xf = self.vignettes.xf;
        let r = Rect::from_min_size(Pos2::ZERO, vec2(self.doc.width.max(1) as f32, self.doc.height.max(1) as f32));
        let coin = |p: Pos2| xf.to_board(self.layer_to_screen(p));
        let h = persp::rect_to_quad(r, [coin(r.left_top()), coin(r.right_top()), coin(r.right_bottom()), coin(r.left_bottom())])?;
        Some(Pose { vers_planche: h, vers_dessin: persp::inverse(&h)? })
    }

    /// Le trait **part** de l'image d'une case qui ne peut pas le recevoir (une case écartée, ou une autre case que celle où il se trace) : la ligne
    /// d'état le dit dès cet instant (une fois par case), le trait continue, il restera entier sur la planche, par-dessus elle. Depuis la partie 131
    /// seul le premier point compte (un trait qui traverse une case n'y laisse rien : il reste à la case où il a commencé). Un test de plus par image
    /// d'écran, seulement pendant un trait.
    pub(crate) fn horschamp_prevenir(&mut self, layout: &BoardLayout) {
        let (Some(stroke), Some(suivi)) = (&self.stroke, &self.vignettes.suivi) else { return };
        let Some(bout) = stroke.path.first() else { return };
        let p = suivi.pose.planche(bout.pos());
        let carte = suivi.carte;
        let Some(origine) = layout.cards.iter().position(|c| c.id == carte) else { return };
        if dans_image(layout, origine, p) {
            return;
        }
        let genre = self.layer().kind;
        let (case, refus) = match (0..layout.cards.len()).find(|k| *k != origine && dans_image(layout, *k, p)) {
            Some(k) => match self.case_recoit(layout, k, genre) {
                Ok(_) => return,
                Err(r) => (layout.cards[k].id, Some(r)),
            },
            None => match layout.annulee_en(p) {
                Some(i) => (layout.annulees[i].id, None),
                None => return,
            },
        };
        if !self.vignettes.refus_dits.contains(&case) {
            self.vignettes.refus_dits.push(case);
            let phrase = self.phrase_du_refus(layout, case, refus.as_ref());
            self.vignettes.refus_phrase = Some(phrase.clone());
            self.status = phrase;
        }
    }

    /// Le geste s'arrête sans rien poser par-dessus une case : ce que la ligne d'état annonçait pendant le geste n'est plus vrai.
    fn refus_oublier(&mut self) {
        self.vignettes.refus_dits.clear();
        if let Some(p) = self.vignettes.refus_phrase.take() {
            if self.status == p {
                self.status.clear();
            }
        }
    }

    /// Ce que dit la ligne d'état quand une part de trait reste par-dessus la case `case` : pourquoi (`refus` ; rien : un plan écarté), avec les mots du logiciel.
    fn phrase_du_refus(&self, layout: &BoardLayout, case: u64, refus: Option<&Refus>) -> String {
        let plan = layout.cards.iter().find(|c| c.id == case).map_or_else(|| "Ce plan".to_string(), |c| {
            let t = crate::numerotation::Numeros::pour_l_ecran(&self.doc).scene_label(&self.doc, c.scene);
            format!("Plan {t}")
        });
        // (La planche pleine : le geste ne promet pas ce que le lever ne pourra pas tenir.)
        let pleine = if self.layers().get(self.cur_layer).is_some_and(|l| l.kind == LayerKind::Bitmap) {
            !self.doc.annotations.bitmap.peut_poser(2) || !self.planche_bitmap_ecrivable()
        } else {
            self.doc.annotations.traces.len() >= crate::annotations::MAX_TRACES
        };
        let suite = if pleine { "et la planche est à sa limite : le trait restera dans le dessin où il se trace" } else { "le trait reste sur la planche, par-dessus la case, hors du plan" };
        match refus {
            None => format!("Ce plan est écarté : {suite}"),
            Some(Refus::Verrouille(n)) => format!("{plan} : le calque « {n} » est verrouillé, {suite}"),
            Some(Refus::Masque(n)) => format!("{plan} : le calque « {n} » est masqué, {suite}"),
            Some(Refus::Transforme(n)) => format!("{plan} : le calque « {n} » est transformé, {suite}"),
            Some(Refus::AutreSorte(LayerKind::Bitmap)) => format!("{plan} : pas de calque bitmap où poser ce trait, {suite}"),
            Some(Refus::AutreSorte(LayerKind::Vector)) => format!("{plan} : pas de calque vectoriel où poser ce trait, {suite}"),
            Some(Refus::SansDessin(n)) => format!("{plan} : le calque « {n} » n'a pas de dessin à cette image et n'en crée pas seul, {suite}"),
            Some(Refus::TenuAilleurs) => format!("{plan} n'est pas à toi dans ce film partagé : {suite}"),
            Some(Refus::Camera) => format!("{plan} : sa caméra ne voit pas la feuille ici, {suite}"),
            Some(Refus::TropGrand) => format!("{plan} : sa caméra cadre trop large pour écrire ce trait dans son dessin, {suite}"),
        }
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
        let derriere = self.paint_behind;
        self.vignettes.fini = Some(Fini { suivi, cel, matiere: Matiere::Bitmap { avant: tiles.clone(), path, couleur, alpha, recette, appui, instants, derriere } });
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

    /// Le trait ou la forme vectoriels qui viennent d'être posés dans le dessin `cel` : à répartir à la fin de l'image d'écran. `derriere` : posé sous les
    /// autres éléments, en premier (« Peindre derrière »), pas en dernier.
    pub(crate) fn horschamp_noter_vecteur(&mut self, cel: CelId, derriere: bool) {
        let Some(suivi) = self.vignettes.suivi.clone() else { return };
        if !self.board_shown() {
            return;
        }
        self.vignettes.fini = Some(Fini { suivi, cel, matiere: Matiere::Vecteur { derriere } });
    }

    /// Le trait qui vient de finir va à son propriétaire, **entier** (partie 131) : parti de sa case, il reste dans son dessin et rien n'est
    /// écrit ; parti hors de toute case, il passe sur la planche (« Hors cadres »). Une seule étape d'annulation. Vrai si quelque chose a bougé.
    pub(crate) fn horschamp_partager(&mut self, layout: &BoardLayout) -> bool {
        // (Le suivi dure autant que le geste : son départ ne change pas d'une image à l'autre.)
        let geste_fini = self.stroke.is_none() && self.shape_drag.is_none() && self.shape_edit.is_none();
        if geste_fini {
            self.vignettes.suivi = None;
        }
        let Some(fini) = self.vignettes.fini.take() else {
            if geste_fini {
                self.refus_oublier();
            }
            return false;
        };
        let Some(origine) = layout.cards.iter().position(|c| c.id == fini.suivi.carte) else {
            self.refus_oublier();
            return false;
        };
        let champs = champs(layout, self.settings.vignettes.debord_fraction());
        let fait = match &fini.matiere {
            Matiere::Vecteur { .. } => self.partager_vecteur(layout, &fini, origine, &champs),
            Matiere::Bitmap { .. } => self.partager_bitmap(layout, &fini, origine, &champs),
        };
        // (Rien n'a bougé : ce que le geste annonçait n'a pas eu lieu. Sinon : `dire_les_refus` a dit la vérité.)
        if fait {
            self.vignettes.refus_dits.clear();
            self.vignettes.refus_phrase = None;
            // (Regroupées depuis la marque : un historique plein, qui oublie sa plus vieille étape à chaque ajout, ne fausse pas le compte.)
            self.history.regrouper_depuis(fini.suivi.marque);
            self.modified = true;
            self.renderer.invalidate_all();
        } else {
            self.refus_oublier();
        }
        // Le trait tel qu'il a été tracé reste peint hors de sa case jusqu'à ce que ce qui le montre soit prêt, en fond : rien ne change au
        // lâcher (`boardui.rs`, `Vignettes::debord_tenu`). Passé au papier : son rendu au pinceau. Resté dans le dessin de sa case : la vignette
        // de la case, qui s'étend jusqu'à lui (partie 131 ; avant, un trait resté dans sa case ne débordait que de sa marge).
        self.vignettes.debord_tenu = self.vignettes.debord_garde.take().map(|d| crate::debord::Tenu { debord: d, xf: self.vignettes.xf, etapes: self.history.undo_len(), debut: std::time::Instant::now() });
        if fait || self.vignettes.debord_tenu.is_some() {
            self.ctx.request_repaint();
        }
        fait
    }

    /// Le rang, dans son dessin, du trait vectoriel qui vient de finir : le dernier élément, ou le premier s'il a été peint derrière. Rien s'il n'y a
    /// rien à poser ailleurs (dessin disparu, rien de neuf, pas un trait).
    fn rang_du_trait(&self, fini: &Fini) -> Option<usize> {
        let Matiere::Vecteur { derriere } = fini.matiere else { return None };
        let d = self.drawing(fini.cel)?;
        if d.elements.len() <= fini.suivi.elements_avant {
            return None;
        }
        let i = if derriere { 0 } else { d.elements.len() - 1 };
        matches!(d.elements.get(i), Some(Element::Stroke { .. })).then_some(i)
    }

    /// La case `k` peut-elle recevoir une part du trait, et sur quel calque : le calque de même sorte que celui où l'on dessine, ni verrouillé, ni
    /// masqué, ni transformé, quelle que soit la caméra de la case (partie 122). Sinon, ce qui l'en empêche.
    pub(crate) fn case_recoit(&self, layout: &BoardLayout, k: usize, genre: LayerKind) -> Result<usize, Refus> {
        let c = layout.cards.get(k).ok_or(Refus::Camera)?;
        if pose_de_la_case(&self.doc, layout, k).is_none() {
            return Err(Refus::Camera);
        }
        // (Dans un film partagé, on n'écrit jamais dans un plan qu'on ne tient pas.)
        if self.partage.as_ref().is_some_and(|s| s.peut_modifier_plan(c.plan).is_err()) {
            return Err(Refus::TenuAilleurs);
        }
        let calques = &self.doc.panels.get(c.panel).ok_or(Refus::Camera)?.layers;
        let voulu = self.layers().get(self.cur_layer).ok_or(Refus::AutreSorte(genre))?;
        let rang = calques
            .iter()
            .position(|l| l.id == voulu.id)
            .or_else(|| calques.iter().position(|l| l.name == voulu.name && l.kind == genre))
            .or_else(|| calques.iter().position(|l| l.kind == genre && !l.folder && l.effet.is_none() && l.video.is_none() && !l.decor && !l.annotation && l.couleur.is_none() && l.scene_wide.is_none()))
            .ok_or(Refus::AutreSorte(genre))?;
        let l = &calques[rang];
        if l.kind != genre || l.folder || l.effet.is_some() || l.video.is_some() {
            return Err(Refus::AutreSorte(genre));
        }
        if !l.drawable() || crate::folders::locked_by_folder(calques, rang) {
            return Err(Refus::Verrouille(l.name.clone()));
        }
        if crate::folders::effect(calques, rang).is_none() {
            return Err(Refus::Masque(l.name.clone()));
        }
        // (Ce que la case montre du calque : sa pose à cette image, et sa profondeur vue de cette caméra.)
        let vue = l.transform_seen_at(&self.doc.camera_seen(c.panel, c.frame), c.frame);
        if !l.pose(c.frame).is_identity() || vue.is_none_or(|t| !t.is_identity()) {
            return Err(Refus::Transforme(l.name.clone()));
        }
        if !l.auto_create && l.cel_at(c.frame).is_none() {
            return Err(Refus::SansDessin(l.name.clone()));
        }
        Ok(rang)
    }

    /// Ce que chaque case de la planche ferait d'une part du trait tracé dans la case `origine`.
    fn cases_qui_recoivent(&self, layout: &BoardLayout, origine: usize, genre: LayerKind) -> Vec<Result<usize, Refus>> {
        (0..layout.cards.len()).map(|k| if k == origine { Ok(self.cur_layer) } else { self.case_recoit(layout, k, genre) }).collect()
    }

    /// Le lever du trait : les parts restées par-dessus des cases, dites une fois (ce que le geste a déjà montré).
    fn dire_les_refus(&mut self, layout: &BoardLayout, cibles: &[Result<usize, Refus>], codes: &[usize], remontre: bool) {
        const REMONTRE: &str = "Le calque « Hors cadres » était caché : il est de nouveau montré";
        let Some(c) = codes.first().and_then(|q| case_du_code(layout, *q)) else {
            self.refus_oublier();
            if remontre {
                self.status = REMONTRE.into();
            }
            return;
        };
        let q = codes[0];
        let refus = cibles.get(q - DESSUS).and_then(|r| r.as_ref().err()).cloned();
        let mut phrase = self.phrase_du_refus(layout, c.id, refus.as_ref());
        let autres = {
            let mut v = codes.to_vec();
            v.sort_unstable();
            v.dedup();
            v.len() - 1
        };
        if autres > 0 {
            phrase.push_str(&format!(" (et {autres} autre{} case{})", if autres > 1 { "s" } else { "" }, if autres > 1 { "s" } else { "" }));
        }
        if remontre {
            phrase.push_str(". ");
            phrase.push_str(REMONTRE);
        }
        self.status = phrase;
    }

    /// La planche ne peut pas recevoir ces parts : elles restent dans le dessin d'origine, et la ligne d'état le dit.
    /// La couche bitmap de la planche peut-elle recevoir des pixels : elle a été lue, et, dans un film partagé, je tiens le jeton du montage (elle ne
    /// s'enregistre qu'avec lui : sans lui, ces pixels ne seraient écrits nulle part).
    pub(crate) fn planche_bitmap_ecrivable(&self) -> bool {
        !self.doc.annotations.bitmap.illisible && self.partage.as_ref().is_none_or(|s| s.tiens(crate::verrous::Cible::Montage))
    }

    fn dire_planche_pleine(&mut self) {
        let bitmap = self.layers().get(self.cur_layer).is_some_and(|l| l.kind == LayerKind::Bitmap);
        if bitmap && !self.planche_bitmap_ecrivable() && !self.doc.annotations.bitmap.illisible {
            self.status = "Dans ce film partagé, le montage n'est pas à toi : la planche ne reçoit pas ce trait, il reste dans le dessin de la case la plus proche, hors de son cadre".into();
            return;
        }
        if bitmap && self.doc.annotations.bitmap.illisible {
            self.status = "La couche bitmap de la planche n'a pas pu être lue à l'ouverture : rien n'y est posé, ce trait reste dans le dessin de la case la plus proche, hors de son cadre".into();
            return;
        }
        let limite = if bitmap { format!("{} morceaux bitmap", crate::planchebitmap::MAX_MORCEAUX) } else { format!("{} traits", crate::annotations::MAX_TRACES) };
        self.status = format!("La planche a atteint sa limite de {limite} : ce trait n'y est pas posé, il reste dans le dessin de la case la plus proche, hors de son cadre");
    }

    /// Le dessin de la case `k` sur son calque `rang`, créé au besoin (comme `ensure_plain_cel`).
    pub(crate) fn cel_de_la_case(&mut self, layout: &BoardLayout, k: usize, rang: usize) -> Option<CelId> {
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

    fn partager_vecteur(&mut self, layout: &BoardLayout, fini: &Fini, origine: usize, champs: &[Rect]) -> bool {
        let cel = fini.cel;
        // (Un trait peint derrière est le premier élément de son dessin, pas le dernier : il se répartit de même, et ses parts restent derrière.)
        let derriere = matches!(fini.matiere, Matiere::Vecteur { derriere: true, .. });
        let Some(rang) = self.rang_du_trait(fini) else { return false };
        let Some(Element::Stroke { points, color, opacity, hardness, brush, trace }) = self.drawing(cel).and_then(|d| d.elements.get(rang)).cloned() else { return false };
        if points.is_empty() {
            return false;
        }
        // Toutes les parts portent le même numéro (partie 125) : la sélection reprend le trait entier. Un trait déjà réparti garde le sien.
        let lien = trace.lien.unwrap_or_else(nouveau_lien);
        let aligne = trace.pressure.len() == points.len() && trace.time.len() == points.len();
        let pose = fini.suivi.pose;
        let pts: Vec<Pt> = points
            .iter()
            .enumerate()
            .map(|(i, p)| Pt { layer: *p, board: pose.planche(p.pos()), pressure: if aligne { trace.pressure[i] } else { 1.0 }, time: if aligne { trace.time[i] } else { 0.0 } })
            .collect();
        let cibles = self.cases_qui_recoivent(layout, origine, LayerKind::Vector);
        let recoit: Vec<bool> = cibles.iter().map(Result::is_ok).collect();
        // Partie 131 : une seule question, pour le premier point du trait. Il appartient tout entier à la case où il a commencé (son image, sa
        // marge), sinon au papier ; plus aucune découpe au bord des cases (un seul tronçon : le trait).
        let qui = proprietaire_cadre(layout, champs, &recoit, origine, pts[0].board);
        // **Sauf dans un dessin que plusieurs cases montrent** (le Décor d'une continuité, une planche-mère, un calque de toute la scène, un
        // dessin tenu d'une marque à l'autre : `render::dessins_communs`) : la règle d'avant y reste, chaque point à la case qu'il recouvre.
        // Entier, il déborderait autour de chacune des cases qui le montrent, et ces cases ne montrent pas ces dessins loin de leur cadre.
        // (Essais : `comme_avant_131` répartit de même, pour fabriquer les traits coupés d'un projet d'avant.)
        let ancien = comme_avant_131() || self.dessin_commun(layout, origine, cel);
        let owner = |p: Pos2| if ancien { proprietaire_cadre(layout, champs, &recoit, origine, p) } else { qui };
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
                VTrace { pressure: run.iter().map(|p| p.pressure).collect(), time: run.iter().map(|p| p.time).collect(), feel: trace.feel, secondary: trace.secondary, lien: Some(lien), cache: Default::default() }
            } else {
                VTrace { lien: Some(lien), ..VTrace::default() }
            },
        };
        // La part d'un dessin posé autrement sur la planche (un autre zoom, une autre caméra) : chaque point à la même place de la planche, l'épaisseur à
        // l'échelle du dessin qui la reçoit. Le pinceau suit, comme quand on transforme un trait (`Element::scale_width`). En perspective, l'échelle change
        // le long du trait : le rejeu reprend l'appui d'après le rayon de chaque point.
        let a_trait = fini.suivi.a;
        // (Rien si un point n'a pas de place dans ce dessin : tout près de l'horizon d'une caméra tournée.)
        let element_pose = |run: &[Pt], cible: &Pose| -> Option<Element> {
            let mut vus: Vec<(VPoint, f32)> = Vec::with_capacity(run.len());
            for p in run {
                let q = cible.dessin_sur(p.board)?;
                let k = a_trait / cible.echelle_en(q).max(1e-6);
                vus.push((VPoint { x: q.x, y: q.y, r: (p.layer.r * k).clamp(0.2, 200.0) }, k));
            }
            let moyenne = vus.iter().map(|(_, k)| *k).sum::<f32>() / vus.len().max(1) as f32;
            let constante = vus.iter().all(|(_, k)| (*k - moyenne).abs() <= moyenne * 0.005);
            let suit = aligne && constante;
            Some(Element::Stroke {
                points: vus.iter().map(|(p, _)| *p).collect(),
                color,
                opacity,
                hardness,
                brush: brush.as_ref().map(|b| Box::new(crate::brush::BrushSettings { size: b.size * moyenne, ..**b })),
                trace: VTrace { pressure: if suit { run.iter().map(|p| p.pressure).collect() } else { Vec::new() }, time: if aligne { run.iter().map(|p| p.time).collect() } else { Vec::new() }, feel: trace.feel, secondary: trace.secondary, lien: Some(lien), cache: Default::default() },
            })
        };
        let plat = fini.suivi.aff.echelle();
        // (Avec le tronçon d'où chacune vient : si la planche ne peut pas les recevoir, ils restent dans le dessin d'origine.)
        let mut sur_la_planche: Vec<(Course, &[Pt])> = Vec::new();
        let mut chez_les_autres: Vec<(usize, Vec<Element>)> = Vec::new();
        let mut chez_elle: Vec<Element> = Vec::new();
        let mut refus_vus: Vec<usize> = Vec::new();
        let mut dits = cibles.clone();
        for (qui, run) in &runs {
            match qui {
                Some(k) if *k == origine => chez_elle.push(element(run, Vec2::ZERO)),
                Some(q) if *q >= DESSUS => {
                    refus_vus.push(*q);
                    sur_la_planche.push((Course { sur: case_du_code(layout, *q).copied(), ..Course::de(run, a_trait, aligne) }, run.as_slice()));
                }
                Some(k) => {
                    // (Le dessin de la case voisine se pose ailleurs sur la planche : le même point de la planche, dans son dessin à elle.)
                    let (a, b) = (plat, aff_de_la_case(&self.doc, layout, *k));
                    let el = match (a, b) {
                        // Même échelle, caméras droites : un simple décalage, le trait reste identique.
                        (Some(a), Some(cible)) if cible.echelle().is_some_and(|e| (e - a).abs() <= a * 0.005) => Some(element(run, vec2((fini.suivi.aff.tx - cible.tx) / a, (fini.suivi.aff.ty - cible.ty) / a))),
                        _ => pose_de_la_case(&self.doc, layout, *k).and_then(|cible| element_pose(run, &cible)),
                    };
                    match el {
                        Some(el) => match chez_les_autres.iter_mut().find(|(c, _)| c == k) {
                            Some((_, v)) => v.push(el),
                            None => chez_les_autres.push((*k, vec![el])),
                        },
                        // Pas de place dans ce dessin : la part reste sur la planche, visible.
                        None => {
                            dits[*k] = Err(Refus::TropGrand);
                            refus_vus.push(DESSUS + *k);
                            sur_la_planche.push((Course { sur: Some(layout.cards[*k]), ..Course::de(run, a_trait, aligne) }, run.as_slice()));
                        }
                    }
                }
                None => sur_la_planche.push((Course::de(run, a_trait, aligne), run.as_slice())),
            }
        }
        // La planche ne peut pas tout recevoir : rien de ces parts n'y va, elles restent où elles étaient, dans le dessin d'origine.
        let pleine = sur_la_planche.len() > crate::annotations::MAX_TRACES.saturating_sub(self.doc.annotations.traces.len());
        if pleine {
            chez_elle.extend(sur_la_planche.iter().map(|(_, run)| element(run, Vec2::ZERO)));
        }
        let sur_la_planche: Vec<Course> = if pleine { Vec::new() } else { sur_la_planche.into_iter().map(|(c, _)| c).collect() };
        // Le dessin de la case d'origine : le trait entier est remplacé par ses tronçons du champ.
        let region = {
            let mut r = Element::Stroke { points: points.clone(), color, opacity, hardness, brush: None, trace: VTrace::default() }.bounds();
            for e in &chez_elle {
                r = r.union(e.bounds());
            }
            r
        };
        if let Some(d) = self.drawing_mut(cel) {
            d.elements.remove(rang);
            d.elements.splice(rang..rang, chez_elle);
        }
        let mut regions: Vec<(CelId, Rect)> = Vec::new();
        for (k, els) in chez_les_autres {
            let Ok(rang) = cibles[k] else { continue };
            let Some(cible) = self.cel_de_la_case(layout, k, rang) else { continue };
            if let Some(d) = self.drawing(cible) {
                avant.push((cible, d.clone()));
            }
            let region_k = els.iter().fold(Rect::NOTHING, |r, e| r.union(e.bounds()));
            if let Some(d) = self.drawing_mut(cible) {
                // (Peint derrière : derrière le dessin de cette case aussi.)
                if derriere {
                    d.elements.splice(0..0, els);
                } else {
                    d.elements.extend(els);
                }
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
        let remontre = self.poser_hors_cadres(layout, sur_la_planche, color, (opacity.clamp(0.0, 1.0) * 255.0).round() as u8, Some(recette), Some(lien));
        // La structure (calques créés, images de dessin, planche) : l'état d'avant, rangé dans l'étape du trait.
        self.history.push(snap);
        if pleine {
            self.dire_planche_pleine();
        } else {
            self.dire_les_refus(layout, &dits, &refus_vus, remontre);
        }
        true
    }

    /// Pose sur la planche, dans le calque « Hors cadres », les tracés `traces` (points de la planche, rayon compris), par-dessus l'image de la case qu'ils
    /// recouvrent s'ils en ont une. `recette` : le pinceau du trait, qui donne sa texture à la part posée sur la planche. **L'appelant a vérifié la place
    /// libre avant de rien changer.** Le calque caché se remontre : une part neuve ne disparaît pas au lever (annuler le recache). Vrai s'il était caché.
    fn poser_hors_cadres(&mut self, layout: &BoardLayout, traces: Vec<Course>, couleur: [u8; 3], alpha: u8, recette: Option<Recette>, lien: Option<u64>) -> bool {
        if traces.is_empty() {
            return false;
        }
        for t in traces {
            let avant = self.doc.annotations.traces.len();
            let brosse = recette.map(|r| (r, t.appui, t.instants, t.a));
            let pose = match &t.sur {
                Some(c) => {
                    let rang = layout.cards.iter().position(|k| k.id == c.id).or_else(|| crate::annotations::case_proche(layout, c.thumb.center()).map(|(r, _)| r)).unwrap_or(0);
                    self.doc.annotations.ajouter_sur_la_case(c, rang, &t.points, couleur, alpha, brosse)
                }
                None => self.doc.annotations.ajouter_hors_cadres_brosse(layout, &t.points, couleur, alpha, brosse),
            };
            debug_assert!(pose, "la place a été vérifiée avant de retirer le trait de son dessin");
            if let Some(t) = self.doc.annotations.traces.get_mut(avant).filter(|_| pose) {
                t.lien = lien;
            }
        }
        self.doc.annotations.id_hors_cadres().is_some_and(|id| self.doc.annotations.regler_visible(id, true))
    }

    fn partager_bitmap(&mut self, layout: &BoardLayout, fini: &Fini, origine: usize, champs: &[Rect]) -> bool {
        let Matiere::Bitmap { avant, path, couleur, alpha, recette, appui, instants, derriere } = &fini.matiere else { return false };
        let derriere = *derriere;
        let pose = fini.suivi.pose;
        // Partie 127 : hors de toute case qui puisse le recevoir, le trait garde ses pixels, sur la couche bitmap de la planche (`planchebitmap.rs`). Seul
        // le trait parti d'une vue en perspective garde l'ancienne règle (un trait vectoriel au pinceau sur la planche) : ses pixels hors de la case sont
        // étirés comme la feuille.
        let pixels_planche = pose.affine64();
        let lien = nouveau_lien();
        let mut cibles = self.cases_qui_recoivent(layout, origine, LayerKind::Bitmap);
        let suit = appui.len() == path.len() && instants.len() == path.len();
        let pts: Vec<Pt> = path
            .iter()
            .enumerate()
            .map(|(i, p)| Pt { layer: *p, board: pose.planche(p.pos()), pressure: if suit { appui[i] } else { 1.0 }, time: if suit { instants[i] } else { 0.0 } })
            .collect();
        let a_trait = fini.suivi.a;
        let cel = fini.cel;
        let plat = fini.suivi.aff.echelle();
        let aff = fini.suivi.aff;
        let durete = recette.as_ref().map_or(1.0, |r| r.durete);
        let place = crate::annotations::MAX_TRACES.saturating_sub(self.doc.annotations.traces.len());
        // (Couche bitmap : la place se compte d'après les pixels, une fois classés ; s'il n'y en a pas assez, ou si la couche n'a pas pu être lue à
        // l'ouverture — on ne pourrait pas l'enregistrer —, on recommence avec la planche « pleine » : tout reste dans le dessin d'origine.)
        let mut forcer_pleine = pixels_planche.is_some() && (!self.doc.annotations.bitmap.peut_poser(0) || !self.planche_bitmap_ecrivable());
        // (Un dessin que plusieurs cases montrent garde la règle d'avant : voir `partager_vecteur`.)
        let commun = self.dessin_commun(layout, origine, cel);
        // Le plan, sans rien changer : ce qui va où. **Le dessin d'origine ne perd ses pixels qu'une fois sûr qu'ils ont une place ailleurs** : une case qui ne peut
        // pas prendre sa part (trop grossie) passe aux refus, et on recommence.
        type Pixels = Vec<(i64, i64, Color32)>;
        #[allow(clippy::type_complexity)]
        let (neuves, poses, runs, pleine, planche, reste): (Vec<(usize, Tile)>, Vec<(usize, usize, Pixels)>, Vec<(Option<usize>, Vec<Pt>)>, bool, Vec<(Option<usize>, Pixels)>, crate::planchebitmap::Masque) = loop {
            let recoit: Vec<bool> = cibles.iter().map(Result::is_ok).collect();
            // Partie 131 : une seule question, pour le premier point du tracé. Le trait appartient tout entier à la case où il a commencé (son
            // image, sa marge), sinon au papier ; ses pixels ne sont plus triés un à un d'après la case qu'ils recouvrent.
            let qui = pts.first().and_then(|p0| proprietaire_cadre(layout, champs, &recoit, origine, p0.board));
            let ancien = comme_avant_131() || commun;
            let brut = |p: Pos2| if ancien { proprietaire_cadre(layout, champs, &recoit, origine, p) } else { qui };
            // La planche ne peut pas tout recevoir : rien n'y va, ces parts restent où elles étaient, dans le dessin d'origine. (Couche bitmap : un morceau
            // pour le papier, un par case qui refuse ; vectoriel : un trait par tronçon.)
            let pleine = if pixels_planche.is_some() {
                forcer_pleine
            } else {
                trancher(&pts, &brut).iter().filter(|(q, _)| q.is_none_or(|q| q >= DESSUS)).count() > place
            };
            let owner = |p: Pos2| match brut(p) {
                Some(q) if q < DESSUS => Some(q),
                _ if pleine => Some(origine),
                o => o,
            };
            // Tout le trait à la case, rayon compris (son cadre, ou sa marge là où aucun autre cadre n'est) : rien à répartir.
            let sur = |p: &Pt| [vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(-1.0, 1.0), vec2(1.0, 1.0)].iter().all(|d| owner(p.board + *d * ((p.layer.r + 1.5) * a_trait)) == Some(origine));
            if pts.iter().all(sur) {
                // (Seulement si le trait sortait vraiment de sa case : un trait resté dedans n'a rien à dire.)
                if pleine && pts.iter().any(|p| brut(p.board).is_none_or(|q| q >= DESSUS)) {
                    self.dire_planche_pleine();
                }
                return false;
            }
            // 1. Le dessin d'origine : ce qui sort du champ est rendu tel qu'il était ; ce qui tombe dans le champ d'une autre case est gardé pour elle.
            let mut passes: Vec<(usize, Pixels)> = Vec::new();
            let mut neuves: Vec<(usize, Tile)> = Vec::new();
            // (Partie 127 : les pixels qui vont sur la planche, par case qu'ils recouvrent ; et où le trait reste dans le dessin d'origine.)
            let mut planche: Vec<(Option<usize>, Pixels)> = Vec::new();
            let mut reste = crate::planchebitmap::Masque::default();
            let Some(c) = self.doc.cels.get(&cel) else { return false };
            for (id, avant_tuile) in avant {
                let Some(apres) = c.raster.slot(*id).cloned() else { continue };
                let (ox, oy) = c.raster.tile_origin(*id);
                let mut neuve = apres.clone();
                let mut touche = false;
                for j in 0..TILE {
                    for i in 0..TILE {
                        let (x, y) = (ox + i as i64, oy + j as i64);
                        let k = j * TILE + i;
                        let apres_px = apres[k];
                        let avant_px = avant_tuile.as_ref().map_or(Color32::TRANSPARENT, |t| t[k]);
                        // (Seuls les pixels que le trait a changés coûtent le calcul de leur propriétaire.)
                        if apres_px == avant_px {
                            continue;
                        }
                        let bord = pose.planche(pos2(x as f32 + 0.5, y as f32 + 0.5));
                        let qui = owner(bord);
                        if qui == Some(origine) {
                            reste.poser(x, y);
                            continue;
                        }
                        if pixels_planche.is_some() {
                            // Partie 127 : le pixel tel que le trait l'a laissé, rien de perdu. Dans le champ d'une autre case : à elle ; hors de toute case qui
                            // puisse le recevoir : sur la planche.
                            if apres_px.a() > 0 {
                                match qui.filter(|q| *q < DESSUS) {
                                    Some(q) => match passes.iter_mut().find(|(c, _)| *c == q) {
                                        Some((_, v)) => v.push((x, y, apres_px)),
                                        None => passes.push((q, vec![(x, y, apres_px)])),
                                    },
                                    None => {
                                        // Partie 131 : un trait parti du papier y va entier, **par-dessus le dessin de la case qu'il traverse**.
                                        // Là où il recouvre de l'encre, c'est le trait seul qui part : le pixel d'après est le trait mêlé à cette
                                        // encre, et l'emporter en laisserait un double sur le papier (visible dès qu'on gomme ou déplace l'un des deux).
                                        let px = if avant_px.a() == 0 { apres_px } else { part_du_trait(avant_px, apres_px, *couleur, derriere) };
                                        if px.a() > 0 {
                                            match planche.iter_mut().find(|(c, _)| *c == qui) {
                                                Some((_, v)) => v.push((x, y, px)),
                                                None => planche.push((qui, vec![(x, y, px)])),
                                            }
                                        }
                                    }
                                }
                            }
                        } else if avant_px.a() == 0 && apres_px.a() > 0 {
                            // (Parti d'une vue en perspective, la règle d'avant : un pixel neuf passe à la case voisine, un pixel qui recouvrait du dessin
                            // ne passe nulle part ; la part de la planche est un trait vectoriel au pinceau.)
                            if let Some(q) = qui.filter(|q| *q < DESSUS) {
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
                    neuves.push((*id, neuve));
                }
            }
            // Parti d'une vue en perspective : les pixels hors de la case sont étirés comme la feuille, plus minces que le trait du geste. La part de la case
            // voisine se pose alors d'après le tracé, un trait lisse de l'épaisseur du geste, sans rejouer le moteur (le grain du pinceau n'y est pas).
            let runs = trancher(&pts, &owner);
            let en_persp = pose.en_perspective();
            if en_persp {
                for (q, _) in &runs {
                    if let Some(q) = q.filter(|q| *q != origine && *q < DESSUS) {
                        if !passes.iter().any(|(c, _)| *c == q) {
                            passes.push((q, Vec::new()));
                        }
                    }
                }
            }
            // 2. Les autres cases : leurs pixels. Même échelle, caméras droites : les mêmes, décalés au pixel près. Sinon, rééchantillonnés dans le dessin de la case
            // (jamais son dessin à elle : seuls les pixels du trait sont lus).
            let mut poses: Vec<(usize, usize, Pixels)> = Vec::new();
            let mut trop = None;
            for (q, pixels) in passes {
                let Ok(rang) = cibles[q] else { continue };
                let pose_q = pose_de_la_case(&self.doc, layout, q);
                let prets: Option<Pixels> = match (plat, aff_de_la_case(&self.doc, layout, q)) {
                    _ if en_persp => pose_q.and_then(|pose_q| {
                        let mut tous = Vec::new();
                        for (_, run) in runs.iter().filter(|(k, _)| *k == Some(q)) {
                            tous.extend(trait_lisse(run, a_trait, &pose_q, *couleur, *alpha, durete)?);
                        }
                        Some(tous)
                    }),
                    (Some(a), Some(aff_q)) if aff_q.echelle().is_some_and(|e| (e - a).abs() <= a * 0.005) => {
                        let (dx, dy) = (((aff.tx - aff_q.tx) / a).round() as i64, ((aff.ty - aff_q.ty) / a).round() as i64);
                        Some(pixels.into_iter().map(|(x, y, px)| (x + dx, y + dy, px)).collect())
                    }
                    _ => pose_q.and_then(|pose_q| reechantillonner(&pixels, &pose, &pose_q)),
                };
                match prets {
                    Some(v) => poses.push((q, rang, v)),
                    None => {
                        trop = Some(q);
                        break;
                    }
                }
            }
            if let Some(q) = trop {
                cibles[q] = Err(Refus::TropGrand);
                continue;
            }
            // (Autant de morceaux neufs que de groupes de pixels pour la planche : pas la place, ou une couche qu'on ne pourrait pas enregistrer : tout reste.)
            if pixels_planche.is_some() && !pleine && !planche.is_empty() && (!self.doc.annotations.bitmap.peut_poser(planche.len()) || !self.planche_bitmap_ecrivable() || layout.cards.is_empty()) {
                forcer_pleine = true;
                continue;
            }
            break (neuves, poses, runs, pleine, planche, reste);
        };
        // Tout a sa place : maintenant seulement, on écrit.
        let mut change: Vec<usize> = Vec::new();
        {
            let Some(c) = self.doc.cels.get_mut(&cel) else { return false };
            for (id, neuve) in neuves {
                c.raster.replace_slot(id, Some(neuve));
                change.push(id);
            }
            c.version += 1;
        }
        self.renderer.invalidate_tiles(&change);
        let snap = self.snapshot();
        let mut etapes: Vec<Edit> = Vec::new();
        let mut empreintes: Vec<(CelId, crate::planchebitmap::Masque)> = vec![(cel, reste)];
        for (q, rang, pixels) in poses {
            let Some(cible) = self.cel_de_la_case(layout, q, rang) else { continue };
            let Some(c) = self.doc.cels.get_mut(&cible) else { continue };
            let mut masque = crate::planchebitmap::Masque::default();
            for &(x, y, _) in &pixels {
                masque.poser(x, y);
            }
            empreintes.push((cible, masque));
            let mut vieux: Vec<(usize, Option<Tile>)> = Vec::new();
            // (Par suites de pixels d'une même tuile : une recherche par pixel se sentait dès un trait un peu long.)
            let t_i = TILE as i64;
            let mut i = 0;
            while i < pixels.len() {
                let (tx, ty) = (pixels[i].0.div_euclid(t_i), pixels[i].1.div_euclid(t_i));
                let id = c.raster.id_of(tx as i32, ty as i32);
                if !vieux.iter().any(|(k, _)| *k == id) {
                    vieux.push((id, c.raster.slot(id).cloned()));
                }
                let t = c.raster.tile_mut(id);
                while let Some(&(x, y, px)) = pixels.get(i).filter(|p| p.0.div_euclid(t_i) == tx && p.1.div_euclid(t_i) == ty) {
                    let k = (y - ty * t_i) as usize * TILE + (x - tx * t_i) as usize;
                    // (Peint derrière : sous le dessin de cette case aussi.)
                    t[k] = if derriere { crate::render::over(px, t[k], 255, None) } else { crate::render::over(t[k], px, 255, None) };
                    i += 1;
                }
            }
            c.version += 1;
            etapes.push(Edit::Tiles { cel: cible, tiles: vieux });
        }
        for e in etapes {
            self.history.push(e);
        }
        // 3. Ce qui est hors de tout champ : sur la planche. Partie 127 : ses pixels, sur la couche bitmap (un morceau pour le papier, un par case qui refuse,
        // par-dessus elle) ; parti d'une vue en perspective : le tracé, rayon et couleur du trait (un trait vectoriel au pinceau).
        let mut refus_vus: Vec<usize> = runs.iter().filter_map(|(q, _)| q.filter(|q| *q >= DESSUS)).collect();
        let mut rendus_planche = false;
        let remontre = match pixels_planche {
            Some(vers_planche) => {
                let mut pose_un = false;
                let mut rendus = false;
                for (code, pixels) in planche {
                    let mut r = crate::planchebitmap::pixels_vides();
                    for &(x, y, px) in &pixels {
                        crate::planchebitmap::poser_pixel(&mut r, x, y, px, false);
                    }
                    let sur = code.and_then(|q| case_du_code(layout, q).map(|c| (c, layout.cards.iter().position(|k| k.id == c.id).unwrap_or(0))));
                    if let Some(q) = code {
                        refus_vus.push(q);
                    }
                    let fait = self.doc.annotations.bitmap.ajouter(layout, r, vers_planche, sur, Some(lien), derriere).is_some();
                    if !fait {
                        // (La place a été vérifiée avant ; si la planche refuse quand même, les pixels reviennent dans le dessin d'où ils partaient :
                        // jamais nulle part. Le trait seul, reposé sur ce que le dessin avait là, ou dessous s'il était peint derrière. Leurs
                        // tuiles sont dans l'étape du trait.)
                        if let Some(c) = self.doc.cels.get_mut(&cel) {
                            let t = TILE as i64;
                            for (x, y, px) in pixels {
                                let id = c.raster.id_of(x.div_euclid(t) as i32, y.div_euclid(t) as i32);
                                let k = (y.rem_euclid(t) * t + x.rem_euclid(t)) as usize;
                                let tuile = c.raster.tile_mut(id);
                                tuile[k] = if derriere { crate::render::over(px, tuile[k], 255, None) } else { crate::render::over(tuile[k], px, 255, None) };
                            }
                            c.version += 1;
                        }
                        rendus = true;
                    }
                    pose_un |= fait;
                }
                if rendus {
                    self.renderer.invalidate_all();
                    rendus_planche = true;
                }
                if pose_un {
                    self.doc.annotations.calque_hors_cadres();
                }
                pose_un && self.doc.annotations.id_hors_cadres().is_some_and(|id| self.doc.annotations.regler_visible(id, true))
            }
            None => {
                let traces: Vec<Course> = runs.into_iter().filter(|(q, _)| q.is_none_or(|q| q >= DESSUS)).map(|(q, run)| Course { sur: q.and_then(|q| case_du_code(layout, q)).copied(), ..Course::de(&run, a_trait, suit) }).collect();
                self.poser_hors_cadres(layout, traces, *couleur, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8, *recette, None)
            }
        };
        // Les empreintes du geste : où ses pixels sont dans chaque dessin (la sélection retrouve ainsi le trait entier, `vignetteselbmp.rs`).
        let mut sans_empreinte = false;
        if pixels_planche.is_some() {
            let doc = &mut self.doc;
            for (c, m) in empreintes {
                // (Signée sur ses pixels de maintenant : si le dessin change sous l'empreinte, par n'importe quel outil, elle ne vaut plus. Deux cases qui
                // montrent le même dessin : une seule empreinte, les deux masques réunis.)
                let Some(d) = doc.cels.get(&c) else { continue };
                let cels = &doc.cels;
                // (Une seule règle : un dessin absent, hors mémoire ou à la corbeille, garde son empreinte ; un dessin changé sous elle la perd.)
                sans_empreinte |= !doc.annotations.bitmap.noter_empreinte(lien, c, m, &d.raster, |e| cels.get(&e.cel).is_none_or(|x| e.fiable_a(&x.raster, x.version)));
            }
        }
        self.history.push(snap);
        if pleine {
            self.dire_planche_pleine();
        } else {
            self.dire_les_refus(layout, &cibles, &refus_vus, remontre);
        }
        if rendus_planche {
            self.status = "La planche n'a pas pu recevoir ce trait : il reste dans le dessin du plan où il a commencé, hors de sa case.".into();
        }
        if sans_empreinte {
            self.status = format!("{} (Trop de traits suivis sur la planche : celui-ci ne pourra pas être choisi entier, seulement case par case.)", self.status).trim().to_string();
        }
        true
    }

    /// Le dessin `cel`, que montre la case `origine`, est-il montré par d'autres cases aussi (`render::dessins_communs` : Décor, planche-mère,
    /// calque de toute la scène, dessin tenu d'une marque d'image à l'autre) ? La règle d'avant la partie 131 vaut alors pour lui.
    pub(crate) fn dessin_commun(&self, layout: &BoardLayout, origine: usize, cel: CelId) -> bool {
        layout.cards.get(origine).is_some_and(|c| crate::render::dessins_communs(&self.doc, c.panel, c.frame).contains(&cel))
    }

    /// Le calque `rang` de la case `k` porte-t-il (ou porterait-il, s'il n'a pas encore de dessin là) un dessin que plusieurs cases montrent ?
    pub(crate) fn calque_commun(&self, layout: &BoardLayout, k: usize, rang: usize) -> bool {
        let Some(c) = layout.cards.get(k) else { return false };
        let Some(l) = self.doc.panels.get(c.panel).and_then(|p| p.layers.get(rang)) else { return false };
        l.decor || l.partage.is_some() || l.scene_wide.is_some() || l.cel_at(c.frame).is_some_and(|cel| crate::render::dessins_communs(&self.doc, c.panel, c.frame).contains(&cel))
    }
}

/// Ce que le trait a posé sur un pixel **qui avait déjà de l'encre** : `apres` est le trait mêlé à `avant` ; rend le trait seul (prémultiplié),
/// pour le poser ailleurs sans emporter le dessin qu'il recouvrait. `couleur` : celle du trait ; `derriere` : il était peint sous le dessin.
///
/// Dessus : `apres = trait + avant × (1 − s)`, `s` l'opacité du trait sur ce pixel ; elle se lit dans l'opacité et dans les trois couleurs (au
/// plus juste des quatre : sur de l'encre opaque, l'opacité seule ne dit rien). Dessous : `apres = avant + trait × (1 − opacité d'avant)`, il
/// ne reste du trait que ce qui se voyait.
pub(crate) fn part_du_trait(avant: Color32, apres: Color32, couleur: [u8; 3], derriere: bool) -> Color32 {
    let (a, r) = (avant.to_array().map(f32::from), apres.to_array().map(f32::from));
    let octet = |v: f32, max: f32| v.round().clamp(0.0, max) as u8;
    if derriere {
        let reste = 255.0 - a[3];
        if reste <= 0.0 {
            return Color32::TRANSPARENT;
        }
        let k = 255.0 / reste;
        let sa = ((r[3] - a[3]) * k).round().clamp(0.0, 255.0);
        return Color32::from_rgba_premultiplied(octet((r[0] - a[0]) * k, sa), octet((r[1] - a[1]) * k, sa), octet((r[2] - a[2]) * k, sa), sa as u8);
    }
    // Quatre mesures de `s` : l'opacité, et chaque couleur (un trait de couleur `couleur`, prémultiplié : `s × couleur`).
    let e = [255.0 - a[3], f32::from(couleur[0]) - a[0], f32::from(couleur[1]) - a[1], f32::from(couleur[2]) - a[2]];
    let d = [r[3] - a[3], r[0] - a[0], r[1] - a[1], r[2] - a[2]];
    let (haut, bas) = e.iter().zip(d).fold((0.0, 0.0), |(h, b), (e, d)| (h + d * e, b + e * e));
    // (De l'encre opaque de la couleur du trait : rien ne dit ce qu'il a posé ; c'est la même couleur, il est pris plein.)
    let s = if bas < 1.0 { 1.0 } else { (haut / bas).clamp(0.0, 1.0) };
    let sa = (255.0 * s).round();
    let garde = 1.0 - s;
    Color32::from_rgba_premultiplied(octet(r[0] - a[0] * garde, sa), octet(r[1] - a[1] * garde, sa), octet(r[2] - a[2] * garde, sa), sa as u8)
}
