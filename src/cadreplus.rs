//! Le « + » de la peau de Maj (partie 116) : un cadre de caméra de plus en Vignettes, sans clic droit.
//!
//! Maj tenue (ou T tenue, ou l'outil Caméra en main), un petit « + » bleu, de la couleur des poignées, se pose juste à côté du **dernier cadre**
//! du plan survolé (ou, pointeur loin des cases, du plan choisi). Sans Maj ni caméra : rien, la planche reste du papier.
//! - un **clic** sur le « + » : un cadre de plus, posé à côté du dernier (`App::cam_ajouter_cadre`, la même chose que le clic droit) ;
//! - **glisser** depuis le « + » : le nouveau cadre naît et suit le pointeur, posé où l'on lâche (le geste de caméra ordinaire,
//!   `Prise::Nouveau`, avec son aimant doux) ; un glissé qui revient au point de départ reste un clic ;
//! - une seule étape d'annulation dans les deux cas, nommée comme avant (« Mouvement de caméra » pour B, « Cadre C »…).
//!
//! Le « + » ne pose rien dans le projet tant qu'on ne l'a pas touché. Il n'apparaît pas pendant un appui (un trait, un geste) et ne réagit
//! pas non plus : la place est calculée à chaque image sans appui, et l'appui lit **ce que l'œil a vu** à l'image d'avant.
//! Il se place à droite du cadre, sinon dessous, dessus, à gauche (jamais hors de la vue, jamais sur la vignette d'un autre plan),
//! sinon dans le cadre, contre son bord droit. Dans sa zone, il passe avant les autres gestes de Maj. Les refus (storyboard verrouillé,
//! plan pris par un autre) sont ceux de `cam_ajouter_cadre` : ils parlent dans la ligne d'état au clic.
//!
//! Plan à plusieurs cases : les cadres d'un mouvement vivent sur la **première** case du plan. Le « + » se pose à côté du dernier cadre
//! de cette case ; si elle n'est pas dans la vue, à côté de la case survolée. Un plan qui ne bouge pas encore et qu'on survole par une
//! de ses autres cases : le « + » est à côté de cette case (le futur B naît de la caméra du plan).
//!
//! **Partie 142** : une case de continuité d'un plan qui bouge déjà porte son propre mouvement, ou peut en commencer un quand la caméra y
//! est à l'arrêt (`App::mouvement_de_case`) : survolée, le « + » est pour elle, à côté de son dernier cadre (ou d'elle). Un clic ou un glissé y
//! pose B vers C, puis C vers D sur la case d'après.

use eframe::egui::{vec2, Color32, Painter, Pos2, Rect, Stroke};

use crate::app::App;
use crate::board::{self, BoardLayout, Hit};
use crate::boardui::{Press, Xf, MOVE_THRESHOLD};
use crate::vignettecam;

/// Le rayon du disque dessiné, et celui de la cible (points d'écran : le « + » ne grandit pas avec le zoom).
const RAYON_VU: f32 = 8.0;
pub(crate) const RAYON_PRISE: f32 = 12.0;
/// Du bord du cadre au centre du « + » : la cible touche le cadre sans le recouvrir.
const DECALE: f32 = RAYON_PRISE + 1.0;

/// Où est le « + » à l'écran, pour quel plan.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PlusPlace {
    pub plan: u64,
    /// La case survolée (ou la première du plan choisi) : le « + » reste au même plan tant que le pointeur est dessus.
    pub survol: usize,
    /// La case que le geste de caméra prend : la première du plan, ou la case survolée d'un plan qui ne bouge pas encore.
    pub ancre: usize,
    pub centre: Pos2,
}

/// Un appui qui a pris le « + », jusqu'à son lâcher.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PlusGeste {
    pub plan: u64,
    pub ancre: usize,
    /// L'image du plan où commence la case qui porte le mouvement (0 : la première ; partie 142 : une case de continuité porte le sien).
    pub image: usize,
    /// Le pointeur à l'appui (écran).
    pub depart: Pos2,
    /// Le geste de caméra est lancé (le pointeur a passé le seuil).
    pub lance: bool,
}

/// Le contraste d'une couleur sur le blanc (1 : aucun, 21 : le noir).
fn contraste_sur_blanc(c: Color32) -> f32 {
    let lin = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    let lum = 0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b());
    1.05 / (lum + 0.05)
}

#[derive(Default)]
pub(crate) struct Etat {
    pub place: Option<PlusPlace>,
    pub geste: Option<PlusGeste>,
    /// Le pointeur est sur le « + » (la ligne d'état le dit une fois à l'arrivée).
    pub survole: bool,
}

impl App {
    /// Le dernier cadre de la case `k` (sur la planche).
    fn plus_dernier_cadre(&self, layout: &BoardLayout, k: usize) -> Option<[Pos2; 4]> {
        self.cadres_sur_la_planche(layout, k).0.last().copied()
    }

    /// Le plan que le « + » sert et la case qui l'a désigné : le même tant que le pointeur est sur le « + », sinon le plan sous le pointeur,
    /// sinon le plan choisi.
    fn plus_plan(&self, layout: &BoardLayout, xf: Xf, over: Option<Pos2>) -> Option<(u64, usize)> {
        if let (Some(p), Some(pl)) = (over, self.vignettes.plus.place) {
            if pl.centre.distance(p) <= RAYON_PRISE + 4.0 && layout.cards.get(pl.survol).is_some_and(|c| c.plan == pl.plan) {
                return Some((pl.plan, pl.survol));
            }
        }
        if let Some(k) = over.and_then(|p| layout.card_near(xf.to_board(p), self.marge_camera())) {
            return Some((layout.cards[k].plan, k));
        }
        let plan = self.vignettes.camera_choisie.or_else(|| (!self.vignettes.libre && self.board_sel.len() == 1).then(|| self.board_sel[0]))?;
        let k = layout.cards.iter().position(|c| c.plan == plan && c.frame == 0)?;
        Some((plan, k))
    }

    /// La place du « + » à côté du cadre `r` (écran) : à droite, dessous, dessus, à gauche, puis dans le cadre ; la première où il tient dans la
    /// vue sans mordre sur la vignette d'un autre plan (ni sur une autre case de ce plan que `ancre`).
    fn plus_chercher(&self, layout: &BoardLayout, xf: Xf, view: Rect, r: Rect, plan: u64, ancre: usize) -> Option<Pos2> {
        let sur = view.shrink(RAYON_PRISE);
        let en_x = |x: f32| x.clamp(sur.left(), sur.right()).clamp(r.left().min(r.right() - 1.0), r.right());
        let en_y = |y: f32| y.clamp(sur.top(), sur.bottom()).clamp(r.top().min(r.bottom() - 1.0), r.bottom());
        let c = r.center();
        let candidats = [
            Pos2::new(r.right() + DECALE, en_y(c.y)),
            Pos2::new(en_x(c.x), r.bottom() + DECALE),
            Pos2::new(en_x(c.x), r.top() - DECALE),
            Pos2::new(r.left() - DECALE, en_y(c.y)),
            // (Dernier recours, dans le cadre contre son bord droit : un cadre qui remplit la vue, ou une planche serrée.)
            Pos2::new((r.right() - DECALE).max(r.left()), en_y(c.y)),
        ];
        // (Jamais sur une barre : Maj + clic sur une barre coupe ou réunit, et le « + » passe devant dans sa zone.)
        let tol = 14.0 / xf.zoom.max(1e-3);
        let sur_une_barre = |p: Pos2| {
            [vec2(0.0, 0.0), vec2(RAYON_VU, 0.0), vec2(-RAYON_VU, 0.0), vec2(0.0, RAYON_VU), vec2(0.0, -RAYON_VU)]
                .iter()
                .any(|d| matches!(board::hit(layout, xf.to_board(p + *d), tol), Hit::Bar(_)))
        };
        candidats.into_iter().find(|&p| {
            sur.contains(p)
                && !sur_une_barre(p)
                && !layout.cards.iter().enumerate().any(|(j, o)| {
                    let cible = if o.plan != plan { o.foot } else if j == ancre || o.frame == 0 { return false } else { o.thumb };
                    xf.rect(cible).distance_to_pos(p) < RAYON_VU + 1.0
                })
        })
    }

    /// Met à jour le « + » de cette image d'écran : sa place si `voulu` (Maj, T ou l'outil Caméra, sans appui en cours), sinon rien.
    pub(crate) fn plus_mettre_a_jour(&mut self, layout: &BoardLayout, xf: Xf, view: Rect, over: Option<Pos2>, voulu: bool) {
        let place = if voulu { self.plus_calculer(layout, xf, view, over) } else { None };
        let survole = place.is_some_and(|pl| over.is_some_and(|p| p.distance(pl.centre) <= RAYON_PRISE));
        if survole && !self.vignettes.plus.survole {
            self.status = "Un cadre de plus : un clic le pose à côté du dernier, ou glisse-le où tu veux.".into();
        }
        self.vignettes.plus.survole = survole;
        self.vignettes.plus.place = place;
    }

    fn plus_calculer(&self, layout: &BoardLayout, xf: Xf, view: Rect, over: Option<Pos2>) -> Option<PlusPlace> {
        let (plan, survol) = self.plus_plan(layout, xf, over)?;
        let k0 = layout.cards.iter().position(|c| c.plan == plan && c.frame == 0)?;
        // (Partie 142) Une case de continuité d'un plan qui bouge déjà, survolée : si elle porte son propre mouvement, ou peut en commencer
        // un (la caméra y est à l'arrêt), le « + » est pour elle, à côté de son dernier cadre (ou d'elle) : B vers C, puis C vers D.
        let s = layout.cards[survol];
        let pour_elle = s.plan == plan && s.frame > 0 && self.case_du_mouvement(s.panel, s.frame) > 0 && self.cam_possible(&s);
        let (cadres, simple) = self.cadres_sur_la_planche(layout, if pour_elle { survol } else { k0 });
        // Un plan qui ne bouge pas encore, survolé par une autre de ses cases : le « + » est à côté de cette case-là.
        let autre_case = !pour_elle && simple && s.plan == plan && s.frame > 0;
        let ancre = if autre_case || pour_elle { survol } else { k0 };
        if !self.cam_possible(&layout.cards[ancre]) {
            return None;
        }
        let dernier = if autre_case { vignettecam::coins_de(layout.cards[ancre].thumb) } else { *cadres.last()? };
        let boite = |q: &[Pos2; 4]| vignettecam::boite(&q.map(|p| xf.to_screen(p)));
        // À côté du dernier cadre ; s'il n'est pas dans la vue, à côté de la case survolée.
        let centre = self
            .plus_chercher(layout, xf, view, boite(&dernier), plan, ancre)
            .or_else(|| (survol != ancre).then(|| self.plus_chercher(layout, xf, view, xf.rect(layout.cards[survol].thumb), plan, ancre)).flatten())?;
        Some(PlusPlace { plan, survol, ancre, centre })
    }

    /// Le « + » : un disque blanc cerclé de la couleur des poignées, un plus de cette couleur ; sous le pointeur, il se remplit.
    pub(crate) fn plus_peindre(&self, painter: &Painter, over: Option<Pos2>) {
        let Some(pl) = self.vignettes.plus.place else { return };
        // La couleur des poignées ; si elle est trop claire pour se lire sur le papier blanc, le bleu des cadres de caméra (`cadre_couleur`).
        let accent = if contraste_sur_blanc(crate::theme::accent()) >= 3.0 { crate::theme::accent() } else { self.cadre_couleur() };
        let chaud = over.is_some_and(|p| p.distance(pl.centre) <= RAYON_PRISE);
        painter.circle_filled(pl.centre, RAYON_VU + 2.0, Color32::from_white_alpha(150));
        painter.circle_filled(pl.centre, RAYON_VU, if chaud { accent } else { Color32::WHITE });
        painter.circle_stroke(pl.centre, RAYON_VU, Stroke::new(1.3_f32, accent));
        let trait_ = Stroke::new(1.7_f32, if chaud { Color32::WHITE } else { accent });
        let b = RAYON_VU * 0.5;
        painter.line_segment([pl.centre - vec2(b, 0.0), pl.centre + vec2(b, 0.0)], trait_);
        painter.line_segment([pl.centre - vec2(0.0, b), pl.centre + vec2(0.0, b)], trait_);
    }

    /// Un appui du bouton principal en `pos` : s'il tombe sur le « + » (tel que l'œil l'a vu), il le prend avant tout autre geste.
    pub(crate) fn plus_appui(&mut self, layout: &BoardLayout, pos: Pos2) -> bool {
        let Some(pl) = self.vignettes.plus.place else { return false };
        if pl.centre.distance(pos) > RAYON_PRISE || layout.cards.get(pl.ancre).is_none_or(|c| c.plan != pl.plan) {
            return false;
        }
        self.commit_note_text(layout);
        self.commit_card_note();
        // (L'image où commence la case qui porte le mouvement que le « + » allonge : `case_du_mouvement`.)
        let image = layout.cards.get(pl.ancre).map_or(0, |c| self.case_du_mouvement(c.panel, c.frame));
        self.vignettes.plus.geste = Some(PlusGeste { plan: pl.plan, ancre: pl.ancre, image, depart: pos, lance: false });
        self.vignettes.press = Press::Dead;
        true
    }

    /// À chaque image, appui tenu : au-delà du seuil, le nouveau cadre naît et suit le pointeur (le geste de caméra ordinaire, un cadre de plus
    /// parti du centre du dernier : le cadre est alors centré sur le pointeur).
    pub(crate) fn plus_suivre(&mut self, layout: &BoardLayout, xf: Xf, latest: Option<Pos2>) {
        let Some(mut g) = self.vignettes.plus.geste else { return };
        if g.lance || !latest.is_some_and(|p| p.distance(g.depart) > MOVE_THRESHOLD) {
            return;
        }
        g.lance = true;
        let centre = layout.cards.get(g.ancre).filter(|c| c.plan == g.plan).and_then(|_| self.plus_dernier_cadre(layout, g.ancre)).map(|q| xf.to_screen(q[0].lerp(q[2], 0.5)));
        match centre {
            Some(c) if self.cam_press(layout, xf, g.ancre, c, true) => {
                self.vignettes.press = Press::Dead;
                self.vignettes.plus.geste = Some(g);
            }
            _ => self.vignettes.plus.geste = None,
        }
    }

    /// Le lâcher. Un glissé lancé qui s'est éloigné du départ est posé par `finish_cam_geste` (une étape, « Cadre C ») ; sans mouvement, ou
    /// revenu au point de départ, c'est un clic : les clés d'avant sont remises et `cam_ajouter_cadre` pose le cadre (la même étape).
    pub(crate) fn plus_lacher(&mut self, latest: Option<Pos2>, any_down: bool) {
        if any_down {
            return;
        }
        let Some(g) = self.vignettes.plus.geste.take() else { return };
        if latest.is_some_and(|p| p.distance(g.depart) > MOVE_THRESHOLD) {
            return;
        }
        let mut pose_ailleurs = false;
        if let Some(cg) = self.vignettes.cam.take_if(|cg| cg.plan == g.plan) {
            if cg.annule {
                return;
            }
            if cg.actif {
                if let Some(pi) = self.doc.panels.iter().position(|p| p.id == g.plan) {
                    self.doc.panels[pi].camera = cg.orig.clone();
                }
            }
        } else if g.lance {
            // (Une commande a posé le geste entre-temps, `poser_le_geste_de_camera` : rien de plus à faire.)
            pose_ailleurs = true;
        }
        if pose_ailleurs {
            return;
        }
        if let Some(pi) = self.doc.panels.iter().position(|p| p.id == g.plan) {
            self.cam_ajouter_cadre_case(pi, g.image);
        }
    }
}
