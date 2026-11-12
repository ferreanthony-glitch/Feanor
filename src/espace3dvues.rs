//! Plusieurs vues dans l'Espace 3D (`docs/INTERFACE.md`, partie 144) : la zone du dessin se partage en une, deux (côte à
//! côte ou l'une sur l'autre) ou quatre vues. Chaque vue a son cadre et son point de vue, choisi par le petit menu de son coin
//! haut gauche : perspective libre, six vues orthographiques (dessus, dessous, face, arrière, gauche, droite), ou la caméra du
//! plan (ce que le plan filme).
//!
//! - **Une vue** : l'Espace 3D d'avant, tel quel (`espace3dview.rs`, sa vue `Espace3dState::cam`, « Regarder par la caméra »).
//! - **Plusieurs vues** : chaque cadre garde sa propre navigation (`Cadre`) ; les gestes vont à la vue sous le pointeur
//!   (celle où le glisser a commencé tant qu'il dure) et passent par le même code que la vue seule
//!   (`App::e3_input_objets`) : choisir, tirer une carte, les poignées. Ce qui est choisi et ce qui bouge est celui du plan :
//!   toutes les vues le montrent.
//! - **Orthographique** : une vraie projection parallèle (`espace3dmath::View::ortho`) : des rayons parallèles, partis du
//!   plan de l'image. Un plan de glisse ou un anneau vu de profil se manipule dans le plan de l'écran (`espace3dview.rs`,
//!   `e3_drag`). Elle se glisse et se zoome, ne tourne pas : un geste de rotation (clic droit, Alt + glisser) la fait passer
//!   en perspective, et la barre d'état le dit.
//! - **Mémoire** : la disposition, les points de vue et le partage des bordures vont dans le projet, dans un champ facultatif
//!   du manifeste (`vues_3d`, `Retenue`), sans changer le numéro de format : une version d'avant l'ignore (elle ouvre le projet
//!   en une vue et ne perd que cette disposition). Absent : une vue, comme avant.

use eframe::egui::{self, pos2, vec2, Align2, Color32, CursorIcon, FontId, PointerButton, Pos2, Rect, RichText, Sense, Stroke};
use egui_phosphor::regular as ph;
use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::espace3d;
use crate::espace3dmath::{self as m, Orbite, View, V3, MAX_ELEVATION};
use crate::espace3dview::{letterbox, nice_step, GridLine};

/// Où est le plan de l'image d'une vue orthographique, en arrière du point visé (mètres) : les rayons en partent, parallèles.
/// Assez loin pour que tout l'espace soit devant ; la projection, elle, ne dépend pas de cette distance.
pub const RECUL_ORTHO: f32 = 10_000.0;
/// Le partage des bordures reste entre ces fractions de la zone.
pub const PARTAGE_MIN: f32 = 0.15;
pub const PARTAGE_MAX: f32 = 0.85;
/// Nombre de cadres retenus : 0 à 3 pour les quatre vues, 4 et 5 pour les deux vues.
pub const CADRES: usize = 6;
/// Une vue plus petite que cela (pixels), au premier cadrage, attend d'avoir sa taille.
pub const TAILLE_MIN: f32 = 16.0;

const ACCENT: Color32 = crate::theme::POIGNEE_SELECTION;
const AXES: [Color32; 3] = [Color32::from_rgb(232, 80, 80), Color32::from_rgb(100, 205, 100), Color32::from_rgb(86, 138, 245)];
const ORBITE_D_ORIGINE: Orbite = Orbite { target: [0.0; 3], azimuth: 30.0, elevation: 22.0, distance: 8.0, focal: 35.0 };

/// Cette prise de vue est celle d'une vue orthographique.
pub fn est_ortho(view: &View) -> bool {
    view.is_ortho()
}

/// La largeur (mètres) que montre une vue orthographique sur toute sa largeur.
pub fn largeur_ortho(view: &View) -> Option<f32> {
    view.ortho
}

/// Comment la zone du dessin se partage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Disposition {
    #[default]
    Une,
    CoteACote,
    LUneSurLAutre,
    Quatre,
}

impl Disposition {
    pub const ALL: [Disposition; 4] = [Disposition::Une, Disposition::CoteACote, Disposition::LUneSurLAutre, Disposition::Quatre];

    pub fn nom(self) -> &'static str {
        match self {
            Disposition::Une => "1 vue",
            Disposition::CoteACote => "2 vues côte à côte",
            Disposition::LUneSurLAutre => "2 vues l'une sur l'autre",
            Disposition::Quatre => "4 vues",
        }
    }

    pub fn icone(self) -> &'static str {
        match self {
            Disposition::Une => ph::SQUARE,
            Disposition::CoteACote => ph::COLUMNS,
            Disposition::LUneSurLAutre => ph::ROWS,
            Disposition::Quatre => ph::SQUARES_FOUR,
        }
    }

    pub fn action(self) -> &'static str {
        match self {
            Disposition::Une => "espace3d.vues_une",
            Disposition::CoteACote => "espace3d.vues_cote",
            Disposition::LUneSurLAutre => "espace3d.vues_superposees",
            Disposition::Quatre => "espace3d.vues_quatre",
        }
    }

    fn cle(self) -> &'static str {
        match self {
            Disposition::Une => "une",
            Disposition::CoteACote => "cote_a_cote",
            Disposition::LUneSurLAutre => "l_une_sur_l_autre",
            Disposition::Quatre => "quatre",
        }
    }

    fn depuis_cle(s: &str) -> Option<Disposition> {
        Disposition::ALL.into_iter().find(|d| d.cle() == s)
    }

    /// Les cadres montrés, dans l'ordre de lecture (rien pour une vue : c'est la vue d'avant).
    pub fn cadres(self) -> &'static [usize] {
        match self {
            Disposition::Une => &[],
            Disposition::CoteACote | Disposition::LUneSurLAutre => &[4, 5],
            Disposition::Quatre => &[0, 1, 2, 3],
        }
    }

    pub fn suivante(self) -> Disposition {
        let i = Disposition::ALL.iter().position(|d| *d == self).unwrap_or(0);
        Disposition::ALL[(i + 1) % Disposition::ALL.len()]
    }
}

/// Le point de vue d'un cadre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointDeVue {
    Perspective,
    Dessus,
    Dessous,
    Face,
    Arriere,
    Gauche,
    Droite,
    Camera,
}

impl PointDeVue {
    pub const ALL: [PointDeVue; 8] = [
        PointDeVue::Perspective,
        PointDeVue::Dessus,
        PointDeVue::Dessous,
        PointDeVue::Face,
        PointDeVue::Arriere,
        PointDeVue::Gauche,
        PointDeVue::Droite,
        PointDeVue::Camera,
    ];

    pub fn nom(self) -> &'static str {
        match self {
            PointDeVue::Perspective => "Perspective",
            PointDeVue::Dessus => "Dessus",
            PointDeVue::Dessous => "Dessous",
            PointDeVue::Face => "Face",
            PointDeVue::Arriere => "Arrière",
            PointDeVue::Gauche => "Gauche",
            PointDeVue::Droite => "Droite",
            PointDeVue::Camera => "Caméra du plan",
        }
    }

    fn cle(self) -> &'static str {
        match self {
            PointDeVue::Perspective => "perspective",
            PointDeVue::Dessus => "dessus",
            PointDeVue::Dessous => "dessous",
            PointDeVue::Face => "face",
            PointDeVue::Arriere => "arriere",
            PointDeVue::Gauche => "gauche",
            PointDeVue::Droite => "droite",
            PointDeVue::Camera => "camera",
        }
    }

    fn depuis_cle(s: &str) -> Option<PointDeVue> {
        PointDeVue::ALL.into_iter().find(|p| p.cle() == s)
    }

    pub fn est_ortho(self) -> bool {
        self.axes().is_some()
    }

    /// Le repère d'une vue orthographique : vers où elle regarde, sa droite et son haut (axes de l'espace : X à droite, Y en
    /// haut, Z vers la caméra du plan). « Face » regarde l'espace du côté de la caméra du plan.
    pub fn axes(self) -> Option<(V3, V3, V3)> {
        Some(match self {
            PointDeVue::Dessus => ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
            PointDeVue::Dessous => ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            PointDeVue::Face => ([0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            PointDeVue::Arriere => ([0.0, 0.0, 1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            PointDeVue::Droite => ([-1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
            PointDeVue::Gauche => ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
            PointDeVue::Perspective | PointDeVue::Camera => return None,
        })
    }
}

/// Les points de vue d'origine des six cadres : Dessus, Face, Droite, Perspective pour les quatre vues ; Perspective et
/// Caméra du plan pour les deux vues.
pub const POINTS_D_ORIGINE: [PointDeVue; CADRES] = [PointDeVue::Dessus, PointDeVue::Face, PointDeVue::Droite, PointDeVue::Perspective, PointDeVue::Perspective, PointDeVue::Camera];

/// Ce que regarde une vue orthographique : le point au centre et la largeur montrée (mètres).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ortho {
    pub target: V3,
    pub span: f32,
}

/// La prise de vue orthographique `point` de `o`, pour une zone de `width` pixels.
pub fn vue_ortho(point: PointDeVue, o: &Ortho, width: f32) -> Option<View> {
    let (forward, right, up) = point.axes()?;
    let span = o.span.max(1e-3);
    Some(View { eye: m::sub(o.target, m::scale(forward, RECUL_ORTHO)), right, up, forward, focal: 35.0, ref_width: width, ortho: Some(span) })
}

/// La vue en perspective qui part d'une vue orthographique : même direction (à la verticale près), même point visé, à la
/// distance où elle montre à peu près la même largeur.
pub fn en_perspective(point: PointDeVue, o: &Ortho) -> Orbite {
    let Some((forward, _, _)) = point.axes() else { return ORBITE_D_ORIGINE };
    let d = m::scale(forward, -1.0);
    Orbite {
        target: o.target,
        azimuth: d[0].atan2(d[2]).to_degrees().rem_euclid(360.0),
        elevation: d[1].clamp(-1.0, 1.0).asin().to_degrees().clamp(-MAX_ELEVATION, MAX_ELEVATION),
        distance: (o.span * 35.0 / 36.0).max(0.5),
        focal: 35.0,
    }
}

/// Un cadre : son point de vue et sa navigation (en perspective, en orthographique).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cadre {
    pub point: PointDeVue,
    pub orbite: Orbite,
    pub ortho: Ortho,
}

impl Cadre {
    fn neuf(point: PointDeVue) -> Cadre {
        Cadre { point, orbite: ORBITE_D_ORIGINE, ortho: Ortho { target: [0.0; 3], span: 10.0 } }
    }

    /// Sa prise de vue (rien pour la caméra du plan, que l'application connaît).
    pub fn vue(&self, width: f32) -> Option<View> {
        match self.point {
            PointDeVue::Camera => None,
            PointDeVue::Perspective => Some(self.orbite.view(width)),
            p => vue_ortho(p, &self.ortho, width),
        }
    }

    /// La molette : avancer (perspective) ou zoomer (orthographique).
    pub fn zoomer(&mut self, scroll: f32) {
        let k = (-scroll * 0.002).exp();
        if self.point.est_ortho() {
            self.ortho.span = (self.ortho.span * k).clamp(0.01, 1e4);
        } else {
            self.orbite.dolly(k);
        }
    }

    /// Glisser la vue de (`dx`, `dy`) pixels : le point visé suit la souris.
    pub fn glisser(&mut self, dx: f32, dy: f32, width: f32) {
        if let Some((_, r, u)) = self.point.axes() {
            let k = self.ortho.span / width.max(1.0);
            self.ortho.target = m::add(self.ortho.target, m::add(m::scale(r, -dx * k), m::scale(u, dy * k)));
        } else {
            self.orbite.pan(dx, dy, width);
        }
    }

    /// Tourner la vue (pixels de souris). Une vue orthographique passe d'abord en perspective : vrai si c'est le cas.
    pub fn tourner(&mut self, dx: f32, dy: f32) -> bool {
        let passe = self.point.est_ortho();
        if passe {
            self.orbite = en_perspective(self.point, &self.ortho);
            self.point = PointDeVue::Perspective;
        }
        self.orbite.orbit(-dx * 0.35, dy * 0.35);
        passe
    }
}

/// Les rectangles des cadres montrés : ceux de la disposition, ou le seul cadre agrandi.
pub fn rects(disposition: Disposition, partage: [f32; 2], rect: Rect, agrandie: Option<usize>) -> Vec<(usize, Rect)> {
    let cadres = disposition.cadres();
    if let Some(i) = agrandie.filter(|i| cadres.contains(i)) {
        return vec![(i, rect)];
    }
    let x = rect.left() + rect.width() * partage[0].clamp(PARTAGE_MIN, PARTAGE_MAX);
    let y = rect.top() + rect.height() * partage[1].clamp(PARTAGE_MIN, PARTAGE_MAX);
    const G: f32 = 1.0;
    let (l, r) = (Rect::from_min_max(rect.min, pos2(x - G, rect.bottom())), Rect::from_min_max(pos2(x + G, rect.top()), rect.max));
    let (t, b) = (Rect::from_min_max(rect.min, pos2(rect.right(), y - G)), Rect::from_min_max(pos2(rect.left(), y + G), rect.max));
    match disposition {
        Disposition::Une => vec![(0, rect)],
        Disposition::CoteACote => vec![(cadres[0], l), (cadres[1], r)],
        Disposition::LUneSurLAutre => vec![(cadres[0], t), (cadres[1], b)],
        Disposition::Quatre => vec![
            (cadres[0], Rect::from_min_max(rect.min, pos2(x - G, y - G))),
            (cadres[1], Rect::from_min_max(pos2(x + G, rect.top()), pos2(rect.right(), y - G))),
            (cadres[2], Rect::from_min_max(pos2(rect.left(), y + G), pos2(x - G, rect.bottom()))),
            (cadres[3], Rect::from_min_max(pos2(x + G, y + G), rect.max)),
        ],
    }
}

/// Ce que le projet retient des vues (champ facultatif `vues_3d` du manifeste). Des noms plutôt que des numéros : un nom
/// inconnu (écrit par une version plus récente) redevient celui d'origine, le projet s'ouvre quand même.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Retenue {
    #[serde(default)]
    pub disposition: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<String>,
    #[serde(default = "milieux")]
    pub partage: [f32; 2],
}

impl Retenue {
    /// Lu en souple depuis le manifeste : une valeur qui n'a pas la bonne forme (un nombre, une liste trop courte, `null`…)
    /// ne donne rien, et le projet s'ouvre en une vue.
    pub fn lire(v: serde_json::Value) -> Option<Retenue> {
        serde_json::from_value::<Retenue>(v).ok()
    }
}

fn milieux() -> [f32; 2] {
    [0.5, 0.5]
}

fn partage_sain(p: [f32; 2]) -> [f32; 2] {
    p.map(|v| if v.is_finite() { v.clamp(PARTAGE_MIN, PARTAGE_MAX) } else { 0.5 })
}

/// L'état des vues dans l'application.
#[derive(Clone, Debug)]
pub struct Vues {
    pub disposition: Disposition,
    pub cadres: [Cadre; CADRES],
    /// Où sont les bordures : la verticale (fraction de la largeur) et l'horizontale (fraction de la hauteur).
    pub partage: [f32; 2],
    /// La vue active (la dernière touchée) : son liseré, et celle qu'agrandit le raccourci quand le pointeur est ailleurs.
    pub active: usize,
    /// La vue agrandie en plein cadre, s'il y en a une.
    pub agrandie: Option<usize>,
    /// La vue sous le pointeur à la dernière image.
    pub sous_pointeur: Option<usize>,
    /// Les rectangles des vues à la dernière image (de quoi cadrer chaque vue à sa taille).
    pub rects: Vec<(usize, Rect)>,
    /// Les cadres ont été cadrés sur l'espace (sinon ils le sont à leur première image).
    pub prets: bool,
}

impl Default for Vues {
    fn default() -> Vues {
        Vues {
            disposition: Disposition::Une,
            cadres: POINTS_D_ORIGINE.map(Cadre::neuf),
            partage: milieux(),
            active: 3,
            agrandie: None,
            sous_pointeur: None,
            rects: Vec::new(),
            prets: false,
        }
    }
}

impl Vues {
    /// L'état retenu par un projet (rien : une vue, comme avant).
    pub fn depuis(r: Option<&Retenue>) -> Vues {
        let mut v = Vues::default();
        let Some(r) = r else { return v };
        v.disposition = Disposition::depuis_cle(&r.disposition).unwrap_or_default();
        for (c, nom) in v.cadres.iter_mut().zip(&r.points) {
            if let Some(p) = PointDeVue::depuis_cle(nom) {
                c.point = p;
            }
        }
        v.partage = partage_sain(r.partage);
        v.active = v.disposition.cadres().last().copied().unwrap_or(3);
        v
    }

    pub fn retenue(&self) -> Retenue {
        Retenue { disposition: self.disposition.cle().to_string(), points: self.cadres.iter().map(|c| c.point.cle().to_string()).collect(), partage: partage_sain(self.partage) }
    }

    /// La vue active, si elle est montrée.
    pub fn active_montree(&self) -> Option<usize> {
        self.disposition.cadres().contains(&self.active).then_some(self.active)
    }
}

/// Les traits de la grille d'une vue orthographique : dans le plan de l'espace face à la vue (le sol vu du dessus ou du
/// dessous, le plan de la feuille vu de face ou de derrière, le plan de côté vu de gauche ou de droite), un trait plus marqué
/// tous les cinq, les axes à leur couleur. `aspect` : largeur / hauteur de la vue.
pub(crate) fn grille_ortho(point: PointDeVue, o: &Ortho, aspect: f32, sol: f32) -> Vec<GridLine> {
    let Some((forward, r, u)) = point.axes() else { return Vec::new() };
    let axe = |v: V3| (0..3).max_by(|&a, &b| v[a].abs().total_cmp(&v[b].abs())).unwrap_or(0);
    let (kr, ku, kf) = (axe(r), axe(u), axe(forward));
    let span = o.span.max(1e-3);
    let step = nice_step(span * 1.2);
    let (half_w, half_h) = (span * 0.55 + step, span * 0.55 / aspect.max(0.05) + step);
    let couleur = |k: usize| match k {
        0 => [0.9, 0.32, 0.32, 0.75],
        1 => [0.4, 0.8, 0.4, 0.75],
        _ => [0.35, 0.5, 0.95, 0.75],
    };
    let mut out = Vec::new();
    // Les traits à coordonnée constante le long de l'axe `k`, tirés le long de l'axe `long` sur `half_long`.
    let mut traits = |k: usize, long: usize, half: f32, half_long: f32| {
        let (c, cl) = (o.target[k], o.target[long]);
        let n = ((half / step).ceil() as i32).min(200);
        for i in -n..=n {
            let a = (c / step).round() * step + i as f32 * step;
            let major = ((a / step).round() as i64) % 5 == 0;
            let color = if a.abs() < step * 0.01 { couleur(long) } else { [0.7, 0.7, 0.72, if major { 0.32 } else { 0.14 }] };
            let mut p = [0.0f32; 3];
            p[k] = a;
            // (Le sol vu de dessus ou de dessous est à sa hauteur, `App::e3_hauteur_sol` ; les autres plans passent par
            // l'origine.)
            p[kf] = if kf == 1 { sol } else { 0.0 };
            let (mut pa, mut pb) = (p, p);
            pa[long] = cl - half_long;
            pb[long] = cl + half_long;
            out.push(GridLine { a: pa, b: pb, color });
        }
    };
    traits(kr, ku, half_w, half_h);
    traits(ku, kr, half_h, half_w);
    out
}

/// Distance du point `p` au segment [a, b] (pixels).
fn seg_dist(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let (d, w) = (b - a, p - a);
    let l2 = d.length_sq();
    let t = if l2 > 0.0 { (w.dot(d) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (w - d * t).length()
}

impl App {
    /// La zone 3D est partagée en plusieurs vues.
    pub(crate) fn e3_vues_multiples(&self) -> bool {
        self.espace3d.vues.disposition != Disposition::Une
    }

    /// Les vues montrées dans la zone `rect`.
    pub(crate) fn e3_vues_rects(&self, rect: Rect) -> Vec<(usize, Rect)> {
        let v = &self.espace3d.vues;
        rects(v.disposition, v.partage, rect, v.agrandie)
    }

    /// Le projet retient la disposition et les points de vue (hors de l'annulation : c'est une façon de regarder, pas une
    /// retouche du film).
    fn e3_vues_retenir(&mut self) {
        let r = Some(self.espace3d.vues.retenue());
        if self.doc.vues_3d != r {
            self.doc.vues_3d = r;
            self.modified = true;
        }
    }

    /// Les bornes de ce que l'espace montre : les cartes et la caméra du plan.
    pub(crate) fn e3_bornes(&self) -> (V3, V3) {
        let world = self.e3_world();
        let mut points: Vec<V3> = Vec::new();
        if self.espace3d.show_cards {
            for li in self.e3_visible_cards() {
                let layer = &self.layers()[li];
                points.extend(world.card_corners(&world.card_xf(&layer.pose(self.cur_frame), layer.depth_at(self.cur_frame))));
            }
        }
        points.push(world.eye(&self.current_camera()));
        m::bounds_of(points).filter(|b| m::length(m::sub(b.1, b.0)) > 0.1).unwrap_or(([-3.0, 0.0, -3.0], [3.0, 2.0, 3.0]))
    }

    /// La taille de la vue du cadre `i` à la dernière image (sinon une estimation d'après la zone du dessin).
    fn e3_vues_taille(&self, i: usize) -> [f32; 2] {
        let v = &self.espace3d.vues;
        if let Some((_, r)) = v.rects.iter().find(|(k, _)| *k == i) {
            return [r.width().max(0.0), r.height().max(0.0)];
        }
        let n = v.disposition.cadres().len().max(1) as f32;
        let r = self.canvas_rect;
        let (w, h) = match v.disposition {
            Disposition::CoteACote => (r.width() / 2.0, r.height()),
            Disposition::LUneSurLAutre => (r.width(), r.height() / 2.0),
            Disposition::Quatre => (r.width() / 2.0, r.height() / 2.0),
            Disposition::Une => (r.width() / n, r.height()),
        };
        [w.max(0.0), h.max(0.0)]
    }

    /// Cadre chaque vue sur tout l'espace ; `ortho_seules` : seulement les vues orthographiques (les vues en perspective
    /// gardent leur angle et leur distance).
    pub(crate) fn e3_vues_cadrer(&mut self, ortho_seules: bool) {
        let (lo, hi) = self.e3_bornes();
        let center = m::scale(m::add(lo, hi), 0.5);
        let montres = self.espace3d.vues.disposition.cadres();
        // (Une vue de moins de `TAILLE_MIN` pixels, au tout début, ne se cadre pas : elle le sera à une image où elle a sa
        // taille.)
        let mut assez = true;
        for i in 0..CADRES {
            let [w, h] = self.e3_vues_taille(i);
            if w < TAILLE_MIN || h < TAILLE_MIN {
                assez &= !montres.contains(&i);
                continue;
            }
            let c = &mut self.espace3d.vues.cadres[i];
            if !ortho_seules {
                let mut o = Orbite::fit_at((lo, hi), [w, h], c.orbite.azimuth, c.orbite.elevation);
                o.distance = o.distance.max(2.0);
                c.orbite = o;
            }
            // L'orthographique : tous les coins dans la largeur, et dans la hauteur à la proportion de la vue.
            let (_, r, u) = c.point.axes().unwrap_or(([0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]));
            let mut span: f32 = 0.5;
            for k in 0..8 {
                let corner = [0, 1, 2].map(|a| if k >> a & 1 == 0 { lo[a] } else { hi[a] });
                let d = m::sub(corner, center);
                span = span.max(2.0 * m::dot(d, r).abs()).max(2.0 * m::dot(d, u).abs() * w / h);
            }
            c.ortho = Ortho { target: center, span: span * 1.15 };
        }
        self.espace3d.vues.prets = assez;
    }

    /// Change la disposition des vues.
    pub(crate) fn e3_vues_poser(&mut self, d: Disposition) {
        if !self.espace3d.on {
            self.set_espace3d(true);
        }
        let avant = self.espace3d.vues.disposition;
        self.espace3d.vues.agrandie = None;
        if avant == Disposition::Une && d != Disposition::Une {
            // La vue d'avant se retrouve dans les vues en perspective ; les vues orthographiques se cadrent sur l'espace.
            let cam = self.espace3d.cam.unwrap_or(ORBITE_D_ORIGINE);
            if !self.espace3d.vues.prets {
                for c in self.espace3d.vues.cadres.iter_mut() {
                    c.orbite = cam;
                }
                self.espace3d.vues.disposition = d;
                self.e3_vues_cadrer(true);
            }
        }
        if d == Disposition::Une && avant != Disposition::Une {
            // Revenir à une vue : elle montre la vue active, si c'est une perspective ou la caméra du plan.
            if let Some(c) = self.espace3d.vues.active_montree().map(|i| self.espace3d.vues.cadres[i]) {
                match c.point {
                    PointDeVue::Perspective => {
                        self.espace3d.cam = Some(c.orbite);
                        self.espace3d.through_camera = false;
                    }
                    PointDeVue::Camera => self.espace3d.through_camera = true,
                    _ => {}
                }
            }
        }
        self.espace3d.vues.disposition = d;
        if self.espace3d.vues.active_montree().is_none() {
            self.espace3d.vues.active = d.cadres().last().copied().unwrap_or(3);
        }
        self.e3_vues_retenir();
        self.status = match d {
            Disposition::Une => "Espace 3D : une vue.".into(),
            _ => format!(
                "Espace 3D : {}. Le menu en haut à gauche de chaque vue choisit son point de vue ; double-clic dessus : l'agrandir. Les bordures se glissent."
                , d.nom()
            ),
        };
    }

    /// Le point de vue du cadre `i`.
    pub(crate) fn e3_vues_point(&mut self, i: usize, p: PointDeVue) {
        let Some(c) = self.espace3d.vues.cadres.get_mut(i) else { return };
        if c.point == p {
            return;
        }
        // Une perspective qui naît d'une vue orthographique en garde le point visé.
        if p == PointDeVue::Perspective && c.point.est_ortho() {
            c.orbite = en_perspective(c.point, &c.ortho);
        }
        // Une vue orthographique qui naît d'une perspective regarde le même point, à une largeur proche.
        if p.est_ortho() && c.point == PointDeVue::Perspective {
            c.ortho = Ortho { target: c.orbite.target, span: (c.orbite.distance * 36.0 / 35.0).max(0.5) };
        }
        c.point = p;
        self.espace3d.vues.active = i;
        self.e3_vues_retenir();
    }

    /// Agrandit la vue sous le pointeur (sinon la vue active) en plein cadre, ou revient à la disposition.
    pub(crate) fn e3_vues_agrandir(&mut self) {
        if !self.e3_vues_multiples() {
            self.status = "Une seule vue : rien à agrandir (barre de la vue 3D, « Vues »).".into();
            return;
        }
        let v = &mut self.espace3d.vues;
        if v.agrandie.take().is_some() {
            self.status = format!("Espace 3D : retour aux {}.", v.disposition.nom());
            return;
        }
        let i = v.sous_pointeur.or(v.active_montree()).unwrap_or(v.disposition.cadres()[0]);
        v.agrandie = Some(i);
        v.active = i;
        self.status = format!("Vue « {} » agrandie : le même raccourci, ou un double-clic sur son menu, ramène les {}.", v.cadres[i].point.nom(), v.disposition.nom());
    }

    /// « Regarder par la caméra » avec plusieurs vues : la vue active passe à la caméra du plan, ou en revient (perspective).
    pub(crate) fn e3_vues_par_la_camera(&mut self) {
        let Some(i) = self.espace3d.vues.active_montree() else { return };
        let p = if self.espace3d.vues.cadres[i].point == PointDeVue::Camera { PointDeVue::Perspective } else { PointDeVue::Camera };
        self.e3_vues_point(i, p);
    }

    /// « Regarder par la caméra » (barre de la vue 3D et panneau Réglages) : la vue seule, ou la vue active.
    pub(crate) fn e3_basculer_par_la_camera(&mut self) {
        if self.e3_vues_multiples() {
            self.e3_vues_par_la_camera();
        } else {
            self.espace3d.through_camera = !self.espace3d.through_camera;
        }
    }

    /// La vue active montre la caméra du plan (le bouton « Regarder par la caméra » est enfoncé).
    pub(crate) fn e3_par_la_camera(&self) -> bool {
        if self.e3_vues_multiples() {
            self.espace3d.vues.active_montree().is_some_and(|i| self.espace3d.vues.cadres[i].point == PointDeVue::Camera)
        } else {
            self.espace3d.through_camera
        }
    }

    /// Les commandes des vues. Vrai si l'action est la leur.
    pub(crate) fn e3_vues_action(&mut self, action: &str) -> bool {
        match action {
            "espace3d.vues" => {
                let d = self.espace3d.vues.disposition.suivante();
                self.e3_vues_poser(d);
            }
            "espace3d.vue_agrandir" => {
                if !self.espace3d.on {
                    self.set_espace3d(true);
                }
                self.e3_vues_agrandir();
            }
            "espace3d.vue_camera" if self.espace3d.on && self.e3_vues_multiples() => self.e3_vues_par_la_camera(),
            _ => match Disposition::ALL.into_iter().find(|d| d.action() == action) {
                Some(d) => self.e3_vues_poser(d),
                None => return false,
            },
        }
        true
    }

    /// La carte vue par la tranche sous le pixel `px` (une vue orthographique la montre en trait) : la plus proche de l'œil
    /// parmi celles dont le contour passe à moins de 6 pixels. Rien dans une vue en perspective.
    pub(crate) fn e3_pick_tranche(&self, view: &View, center: [f32; 2], px: [f32; 2]) -> Option<u64> {
        if !est_ortho(view) || !self.espace3d.show_cards {
            return None;
        }
        let world = self.e3_world();
        let frame = self.cur_frame;
        let p = pos2(px[0], px[1]);
        let mut best: Option<(f32, f32, u64)> = None;
        for li in self.e3_visible_cards() {
            let layer = &self.layers()[li];
            let xf = world.card_xf(&layer.pose(frame), layer.depth_at(frame));
            let c = world.card_corners(&xf);
            let q: Vec<Pos2> = c.iter().filter_map(|&w| view.project(w, center).map(|s| pos2(s[0], s[1]))).collect();
            if q.len() < 4 {
                continue;
            }
            let d = (0..4).map(|k| seg_dist(p, q[k], q[(k + 1) % 4])).fold(f32::MAX, f32::min);
            let depth = view.to_view(xf.pos)[2];
            if d < 6.0 && best.is_none_or(|(bd, bz, _)| d < bd - 0.5 || (d < bd + 0.5 && depth < bz)) {
                best = Some((d, depth, espace3d::card_selection(layer.id)));
            }
        }
        best.map(|b| b.2)
    }

    /// La prise de vue du cadre `c` dans la zone `draw`.
    fn e3_vues_vue(&self, c: &Cadre, draw: Rect) -> View {
        c.vue(draw.width()).unwrap_or_else(|| self.e3_world().scene_cam(&self.current_camera()).view(draw.width()))
    }

    /// La zone où le cadre `c` dessine, dans son rectangle `r` (la caméra du plan : au format du cadre).
    fn e3_vues_draw(&self, c: &Cadre, r: Rect) -> Rect {
        if c.point == PointDeVue::Camera {
            letterbox(r, self.doc.frame)
        } else {
            r
        }
    }

    /// La zone du dessin en plusieurs vues : chaque vue reçoit les gestes faits sous le pointeur, puis toutes se peignent.
    pub(crate) fn e3_vues_panel(&mut self, ui: &mut egui::Ui, rect: Rect) {
        if !self.espace3d.vues.prets {
            self.espace3d.vues.rects = self.e3_vues_rects(rect);
            self.e3_vues_cadrer(false);
        }
        let panes = self.e3_vues_rects(rect);
        self.espace3d.vues.rects = panes.clone();
        let ctx = ui.ctx().clone();
        let through_avant = self.espace3d.through_camera;
        let resps: Vec<(usize, Rect, egui::Response)> = panes.iter().map(|&(i, r)| (i, r, ui.interact(r, ui.id().with(("e3_vue", i)), Sense::click_and_drag()))).collect();
        self.espace3d.vues.sous_pointeur = resps.iter().find(|x| x.2.contains_pointer()).map(|x| x.0);
        // Le geste va à la vue où il a commencé, sinon à celle sous le pointeur.
        let cible = resps.iter().find(|x| x.2.dragged() || x.2.drag_stopped()).or_else(|| resps.iter().find(|x| x.2.hovered()));
        match cible {
            Some((i, r, resp)) => {
                let (i, r, resp) = (*i, *r, resp.clone());
                let scroll = ctx.input(|inp| inp.smooth_scroll_delta.y);
                if resp.is_pointer_button_down_on() || scroll != 0.0 {
                    self.espace3d.vues.active = i;
                }
                self.e3_vues_input(&ctx, i, r, &resp);
            }
            None => {
                self.espace3d.hover = None;
                if !ctx.input(|inp| inp.pointer.primary_down()) {
                    self.e3_finish_drag();
                }
            }
        }
        for &(i, r) in &panes {
            self.e3_vues_peindre(ui, i, r, panes.len() > 1);
        }
        self.espace3d.through_camera = through_avant;
        for &(i, r) in &panes {
            self.e3_vues_menu(ui, i, r);
        }
        self.e3_vues_bordures(ui, rect);
        if resps.iter().any(|x| x.2.hovered() || x.2.dragged()) {
            let icon = match (&self.espace3d.drag, self.espace3d.hover) {
                (Some(_), _) => CursorIcon::Grabbing,
                (None, Some(_)) => CursorIcon::Grab,
                _ => CursorIcon::Default,
            };
            ctx.set_cursor_icon(icon);
        }
    }

    /// Les gestes dans la vue `i` : sa navigation, puis ce que fait la vue seule (choisir, tirer, les poignées).
    fn e3_vues_input(&mut self, ctx: &egui::Context, i: usize, r: Rect, resp: &egui::Response) {
        let mut c = self.espace3d.vues.cadres[i];
        let camera = c.point == PointDeVue::Camera;
        self.espace3d.through_camera = camera;
        let draw = self.e3_vues_draw(&c, r);
        let (scroll, alt) = ctx.input(|inp| (inp.smooth_scroll_delta.y, inp.modifiers.alt));
        let delta = resp.drag_delta();
        let mut tournee = false;
        if camera {
            self.e3_nav_camera(draw, resp, scroll, delta);
        } else {
            if resp.hovered() && scroll != 0.0 {
                c.zoomer(scroll);
            }
            if resp.dragged_by(PointerButton::Middle) {
                c.glisser(delta.x, delta.y, draw.width());
            }
            if resp.dragged_by(PointerButton::Secondary) {
                tournee |= c.tourner(delta.x, delta.y);
            }
        }
        let view = self.e3_vues_vue(&c, draw);
        let orbiting = self.e3_input_objets(ctx, &view, draw, resp);
        if orbiting && !camera {
            // (Glisser dans le vide : une vue orthographique glisse, une perspective tourne ; Alt + glisser tourne toujours.)
            if c.point.est_ortho() && !alt {
                c.glisser(delta.x, delta.y, draw.width());
            } else {
                tournee |= c.tourner(delta.x, delta.y);
            }
        }
        if tournee {
            self.status = "Vue tournée : elle passe en perspective (son menu la remet de face, de dessus ou de côté).".into();
        }
        let avant = self.espace3d.vues.cadres[i].point;
        self.espace3d.vues.cadres[i] = c;
        if avant != c.point {
            self.e3_vues_retenir();
        }
    }

    /// Peint la vue `i` dans son rectangle `r` : fond, grille, cartes, caméra du plan, manipulateur, repère des axes, liseré
    /// de la vue active.
    fn e3_vues_peindre(&mut self, ui: &mut egui::Ui, i: usize, r: Rect, plusieurs: bool) {
        let c = self.espace3d.vues.cadres[i];
        let camera = c.point == PointDeVue::Camera;
        self.espace3d.through_camera = camera;
        let draw = self.e3_vues_draw(&c, r);
        let view = self.e3_vues_vue(&c, draw);
        let center = [draw.width() / 2.0, draw.height() / 2.0];
        let painter = ui.painter_at(r);
        let to_screen = |q: [f32; 2]| pos2(draw.left() + q[0], draw.top() + q[1]);
        let proj = |p: V3| view.project(p, center).map(to_screen);
        painter.rect_filled(r, 0.0, Color32::from_gray(26));
        if self.espace3d.show_grid {
            // (Le même quadrillage que la vue seule, `App::e3_quadrillage`.)
            let lignes = match c.point {
                PointDeVue::Camera => self.e3_quadrillage([0.0; 3], 8.0, None),
                PointDeVue::Perspective => self.e3_quadrillage(c.orbite.target, c.orbite.distance, None),
                p => self.e3_quadrillage(c.ortho.target, c.ortho.span, Some((p, c.ortho, draw.width() / draw.height().max(1.0)))),
            };
            Self::e3_peindre_quadrillage(&painter, &proj, lignes);
        }
        self.e3_paint_cards(ui.ctx(), &painter, &view, center, &to_screen);
        if !camera {
            let cam_now = self.e3_world().scene_cam(&self.current_camera());
            self.e3_paint_camera(&painter, &proj, &view, &cam_now);
        }
        if let Some(g) = self.e3_gizmo(&view) {
            self.e3_paint_gizmo(&painter, &proj, &g);
        }
        if camera {
            painter.rect_stroke(draw, 0.0, Stroke::new(1.0_f32, crate::theme::camera()), egui::StrokeKind::Outside);
        }
        peindre_axes(&painter, r, &view);
        if plusieurs && self.espace3d.vues.active == i {
            painter.rect_stroke(r.shrink(1.0), 0.0, Stroke::new(1.0_f32, ACCENT.gamma_multiply(0.55)), egui::StrokeKind::Inside);
        }
    }

    /// Le menu du coin haut gauche de la vue `i` : son point de vue ; double-clic : l'agrandir ou revenir.
    fn e3_vues_menu(&mut self, ui: &mut egui::Ui, i: usize, r: Rect) {
        let point = self.espace3d.vues.cadres[i].point;
        let zone = Rect::from_min_size(r.min + vec2(6.0, 5.0), vec2((r.width() - 12.0).max(10.0), 22.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(zone).id_salt(("e3_vue_menu", i)).layout(egui::Layout::left_to_right(egui::Align::Min)));
        let texte = RichText::new(format!("{}  {}", point.nom(), ph::CARET_DOWN)).size(12.0).color(if point == PointDeVue::Camera { crate::theme::camera() } else { Color32::from_gray(225) });
        let resp = child.add(egui::Button::new(texte).fill(Color32::from_black_alpha(150)).corner_radius(4.0));
        let resp = crate::bulles::bulle(resp, "Point de vue de cette vue", "Perspective libre, une vue orthographique (dessus, dessous, face, arrière, gauche, droite) ou la caméra du plan. Double-clic : agrandir cette vue en plein cadre, ou revenir.", "");
        let id = child.make_persistent_id(("e3_vue_points", i));
        if resp.double_clicked() {
            child.memory_mut(|mem| mem.close_popup());
            self.espace3d.vues.sous_pointeur = Some(i);
            self.e3_vues_agrandir();
            return;
        }
        if resp.clicked() {
            child.memory_mut(|mem| mem.toggle_popup(id));
        }
        let choix = egui::popup_below_widget(&child, id, &resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
            ui.set_min_width(170.0);
            let mut choix = None;
            for p in PointDeVue::ALL {
                let nom = if p.est_ortho() { format!("{}  (orthographique)", p.nom()) } else { p.nom().to_string() };
                if ui.selectable_label(p == point, nom).clicked() {
                    choix = Some(p);
                }
            }
            choix
        })
        .flatten();
        if let Some(p) = choix {
            self.e3_vues_point(i, p);
        }
    }

    /// Les bordures entre les vues : un trait, et une poignée qu'on glisse pour changer leur taille.
    fn e3_vues_bordures(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let v = &self.espace3d.vues;
        if v.agrandie.is_some() {
            return;
        }
        let [px, py] = v.partage.map(|p| p.clamp(PARTAGE_MIN, PARTAGE_MAX));
        let (x, y) = (rect.left() + rect.width() * px, rect.top() + rect.height() * py);
        let mut bords: Vec<(usize, Rect)> = Vec::new();
        if matches!(v.disposition, Disposition::CoteACote | Disposition::Quatre) {
            bords.push((0, Rect::from_x_y_ranges(x - 3.0..=x + 3.0, rect.y_range())));
        }
        if matches!(v.disposition, Disposition::LUneSurLAutre | Disposition::Quatre) {
            bords.push((1, Rect::from_x_y_ranges(rect.x_range(), y - 3.0..=y + 3.0)));
        }
        let painter = ui.painter_at(rect);
        for (k, br) in bords {
            let resp = ui.interact(br, ui.id().with(("e3_bord", k)), Sense::drag());
            let chaud = resp.hovered() || resp.dragged();
            let trait_ = if k == 0 { [pos2(x, rect.top()), pos2(x, rect.bottom())] } else { [pos2(rect.left(), y), pos2(rect.right(), y)] };
            painter.line_segment(trait_, Stroke::new(2.0_f32, if chaud { ACCENT.gamma_multiply(0.8) } else { Color32::from_gray(58) }));
            if chaud {
                ui.ctx().set_cursor_icon(if k == 0 { CursorIcon::ResizeHorizontal } else { CursorIcon::ResizeVertical });
            }
            if resp.dragged() {
                if let Some(p) = resp.interact_pointer_pos() {
                    let f = if k == 0 { (p.x - rect.left()) / rect.width().max(1.0) } else { (p.y - rect.top()) / rect.height().max(1.0) };
                    self.espace3d.vues.partage[k] = f.clamp(PARTAGE_MIN, PARTAGE_MAX);
                }
            }
            if resp.drag_stopped() {
                self.e3_vues_retenir();
            }
        }
    }

    /// La commande « Vues » de la barre de la vue 3D : son icône (la disposition du moment) ouvre les quatre choix.
    pub(crate) fn e3_vues_bouton(&mut self, ui: &mut egui::Ui) {
        let d = self.espace3d.vues.disposition;
        let resp = self.case_icone(
            ui,
            d.icone(),
            None,
            "Vues",
            "Partage la vue 3D en 1 vue, 2 vues côte à côte, 2 vues l'une sur l'autre ou 4 vues. Chaque vue choisit son point de vue par le menu de son coin. Le raccourci passe à la disposition suivante.",
            "espace3d.vues",
        );
        let id = ui.make_persistent_id("e3_vues_dispositions");
        if resp.clicked() {
            ui.memory_mut(|mem| mem.toggle_popup(id));
        }
        let choix = egui::popup_below_widget(ui, id, &resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
            ui.set_min_width(210.0);
            let mut choix = None;
            for k in Disposition::ALL {
                if ui.selectable_label(k == d, format!("{}  {}", k.icone(), k.nom())).clicked() {
                    choix = Some(k);
                }
            }
            choix
        })
        .flatten();
        if let Some(k) = choix {
            self.e3_vues_poser(k);
        }
    }
}

/// Le repère des axes, en bas à gauche de la vue : X rouge, Y vert, Z bleu, tels que la vue les voit.
fn peindre_axes(painter: &egui::Painter, r: Rect, view: &View) {
    let o = pos2(r.left() + 30.0, r.bottom() - 30.0);
    let mut ordre = [0usize, 1, 2];
    // (Le plus loin d'abord : celui qui vient vers nous passe devant.)
    ordre.sort_by(|&a, &b| view.forward[b].total_cmp(&view.forward[a]));
    painter.circle_filled(o, 3.0, Color32::from_gray(200));
    for k in ordre {
        let d = vec2(view.right[k], -view.up[k]);
        let nom = ["X", "Y", "Z"][k];
        if d.length() < 0.08 {
            // De bout : un rond (plein s'il vient vers nous).
            if view.forward[k] < 0.0 {
                painter.circle_filled(o, 6.0, AXES[k]);
            } else {
                painter.circle_stroke(o, 6.0, Stroke::new(1.5_f32, AXES[k]));
            }
            painter.text(o + vec2(9.0, -9.0), Align2::LEFT_BOTTOM, nom, FontId::proportional(10.0), AXES[k]);
            continue;
        }
        let tip = o + d * 20.0;
        painter.line_segment([o, tip], Stroke::new(2.0_f32, AXES[k]));
        painter.text(o + d * 28.0, Align2::CENTER_CENTER, nom, FontId::proportional(10.0), AXES[k]);
    }
}
