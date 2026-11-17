//! Les graphiques du panneau Espace 3D (`docs/INTERFACE.md`, partie 162). Anthony, 2026-11-15 : « des panneaux moins dans les
//! chiffres, plus visuels, plus facilement compréhensibles, qui donnent envie d'être manipulés », et, devant la maquette : « si
//! l'on doit interagir avec les graphiques pour modifier directement la scène c'est super ».
//!
//! Le panneau devient **contextuel** (`App::espace3d_options_panel`, `espace3dui.rs`) :
//!
//! - **rien n'est choisi : la scène.** « Ajouter à la scène » (quatre vignettes dessinées : Surface, Sol, Mur, Ciel), « Le
//!   plateau, vu de dessus » (la caméra, son cône, chaque calque en trait à sa place : on glisse un trait pour l'avancer ou le
//!   reculer, on le clique pour le choisir, on clique la caméra pour la choisir), « Le plus proche passe devant » ;
//! - **un calque choisi.** « Comment il est posé » (Face, Au sol, Mur g., Mur d.), « Distance à la caméra » (une réglette qu'on
//!   glisse), « Taille dans le cadre » (un curseur entre deux carrés), puis « Précis », replié : tous les champs d'avant ;
//! - **la caméra choisie.** « Objectif » (cinq cadrages, du grand-angle au télé, et le cône qu'on glisse pour une focale libre),
//!   « Hauteur et plongée » (de côté, à côté d'un personnage de 1,70 m : on monte la caméra, on tire sa visée), « Panoramique et
//!   horizon » (un cadran qu'on tourne, un horizon qu'on penche), « Regarder par elle », « Viser la sélection », puis « Précis ».
//!
//! Tout est peint ici (egui `Painter`), aux couleurs du thème (gris, l'accent de la sélection, la couleur de la caméra du
//! thème). Chaque geste passe par le chemin du champ qu'il remplace (`Layer::set_depth_selon`, `Monde::apply_card_xf_libre`,
//! `App::set_current_camera_libre`) et fait **une étape d'annulation par geste** (`App::edited`, la clé du geste étant le
//! graphique). Rien de ceci ne se montre hors de la vue de la scène (partie 159) : le panneau ne le peint que vue ouverte.

use eframe::egui::{self, pos2, vec2, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, RichText, Sense, Shape, Stroke, StrokeKind};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::espace3d::{self, SceneCam, MAX_FOCAL, MAX_TILT, MIN_FOCAL};
use crate::espace3ddecor::Sorte3d;
use crate::espace3dgizmo::CAMERA_ID;
use crate::espace3dmath as m;
use crate::theme;

/// La taille des textes peints dans les graphiques.
const TEXTE: f32 = 10.5;
/// L'écart entre deux cases d'une rangée de vignettes.
const ECART: f32 = 4.0;
/// La hauteur du plateau vu de dessus.
pub(crate) const HAUTEUR_PLATEAU: f32 = 170.0;
/// Le côté du cadran (et la hauteur de l'horizon), et la hauteur de leur centre sous leur bord du haut.
pub(crate) const COTE_TOUR: f32 = 72.0;
pub(crate) const CENTRE_TOUR: f32 = 32.0;
/// Les objectifs proposés en vignettes, et leur légende (la maquette validée par Anthony).
pub(crate) const OBJECTIFS: [f32; 5] = [18.0, 35.0, 50.0, 85.0, 135.0];
pub(crate) const LEGENDE_OBJECTIFS: &str = "grand-angle · large · normal · portrait · télé";

// ------------------------------------------------------------------------------------ couleurs (celles du thème)

fn fond() -> Color32 {
    theme::champ()
}

fn trait_doux() -> Color32 {
    theme::bord_fort()
}

fn encre() -> Color32 {
    theme::texte()
}

fn encre_douce() -> Color32 {
    theme::texte_doux()
}

fn accent() -> Color32 {
    theme::accent_texte()
}

fn camera() -> Color32 {
    theme::camera()
}

fn police() -> FontId {
    FontId::proportional(TEXTE)
}

/// Un nombre à la française, `d` décimales.
pub(crate) fn nombre(v: f32, d: usize) -> String {
    format!("{v:.d$}").replace('.', ",")
}

/// L'écart d'angle de `b` à `a`, ramené entre −180° et 180°.
fn ecart(a: f32, b: f32) -> f32 {
    let mut d = (a - b) % 360.0;
    if d > 180.0 {
        d -= 360.0;
    } else if d < -180.0 {
        d += 360.0;
    }
    d
}

/// Ce que dit la bascule de la caméra, en mots.
pub(crate) fn plongee(tilt: f32) -> &'static str {
    match tilt {
        t if t <= -45.0 => "forte plongée",
        t if t <= -15.0 => "plongée",
        t if t < -1.5 => "légère plongée",
        t if t <= 1.5 => "à plat",
        t if t < 15.0 => "légère contre-plongée",
        t if t < 45.0 => "contre-plongée",
        _ => "forte contre-plongée",
    }
}

/// Une zone de dessin de la largeur du panneau, qu'on clique et qu'on glisse, avec sa bulle.
fn zone(ui: &mut egui::Ui, largeur: f32, hauteur: f32, actif: bool, nom: &str, description: &str) -> (Rect, Response) {
    let sens = if actif { Sense::click_and_drag() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(largeur, hauteur), sens);
    ui.painter().rect_filled(rect, 4.0, fond());
    (rect, crate::bulles::bulle(resp, nom, description, ""))
}

/// Où le geste de `resp` a commencé : là où le bouton s'est enfoncé (le glissé ne commence que quelques points plus loin).
pub(crate) fn depart(ui: &egui::Ui, resp: &Response) -> Option<Pos2> {
    ui.input(|i| i.pointer.press_origin()).or_else(|| resp.interact_pointer_pos())
}

/// Un point d'un dessin fait dans une boîte de `vb` (largeur, hauteur), mis à l'échelle et centré dans `r`.
fn pt(r: Rect, vb: [f32; 2], x: f32, y: f32) -> Pos2 {
    let k = (r.width() / vb[0]).min(r.height() / vb[1]);
    let o = r.center() - vec2(vb[0], vb[1]) * k / 2.0;
    o + vec2(x, y) * k
}

fn ligne(p: &egui::Painter, pts: Vec<Pos2>, s: Stroke) {
    p.add(Shape::line(pts, s));
}

fn boucle(p: &egui::Painter, pts: Vec<Pos2>, s: Stroke) {
    p.add(Shape::closed_line(pts, s));
}

fn courbe(p: &egui::Painter, a: Pos2, c: Pos2, b: Pos2, s: Stroke) {
    p.add(egui::epaint::QuadraticBezierShape::from_points_stroke([a, c, b], false, Color32::TRANSPARENT, s));
}

/// Une case à vignette : un dessin (`dessin`, peint dans la boîte du haut) et son nom dessous. Allumée quand `choisie`.
fn case(ui: &mut egui::Ui, largeur: f32, hauteur: f32, choisie: bool, nom: &str, dessin: impl FnOnce(&egui::Painter, Rect, Stroke)) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(largeur, hauteur), Sense::click());
    let p = ui.painter();
    let survol = resp.hovered();
    let fond = if choisie {
        theme::selection()
    } else if survol {
        theme::controle_survol()
    } else {
        theme::controle()
    };
    let bord = if choisie { accent() } else { theme::bord() };
    let ink = if choisie { theme::sur_selection() } else { encre() };
    p.rect_filled(rect, 5.0, fond);
    p.rect_stroke(rect, 5.0, Stroke::new(1.0_f32, bord), StrokeKind::Inside);
    let haut = Rect::from_min_max(rect.min + vec2(3.0, 3.0), pos2(rect.right() - 3.0, rect.bottom() - TEXTE - 4.0));
    dessin(p, haut, Stroke::new(1.2_f32, ink));
    p.text(pos2(rect.center().x, rect.bottom() - 2.0), Align2::CENTER_BOTTOM, nom, police(), ink);
    resp
}

/// La largeur qu'un graphique peut prendre : la place qui reste, sans passer le bord visible du panneau (sa marge de droite).
pub(crate) fn largeur_utile(ui: &egui::Ui) -> f32 {
    let visible = ui.clip_rect().right() - ui.cursor().left() - 4.0;
    ui.available_width().min(visible).max(20.0)
}

/// La largeur d'une case d'une rangée de `n` cases dans `ui`.
fn largeur_de_case(ui: &egui::Ui, n: usize) -> f32 {
    ((largeur_utile(ui) - ECART * (n as f32 - 1.0)) / n as f32).floor().max(20.0)
}

/// Un titre de section du panneau.
fn titre(ui: &mut egui::Ui, texte: &str) {
    ui.label(RichText::new(texte).strong());
}

/// Une légende discrète au-dessus d'un graphique.
fn legende(ui: &mut egui::Ui, texte: &str) {
    ui.label(RichText::new(texte).small().color(encre_douce()));
}

// ------------------------------------------------------------------------------------ les dessins des cases

/// Ajouter à la scène : Surface, Sol, Mur, Ciel (boîte de 54 × 34).
fn dessin_ajout(k: usize, p: &egui::Painter, r: Rect, s: Stroke) {
    let vb = [54.0, 34.0];
    let q = |x, y| pt(r, vb, x, y);
    let faible = Stroke::new(1.0_f32, trait_doux());
    match k {
        0 => {
            boucle(p, vec![q(15.0, 5.0), q(39.0, 5.0), q(39.0, 23.0), q(15.0, 23.0)], s);
            courbe(p, q(22.0, 18.0), q(27.0, 8.0), q(32.0, 18.0), s);
        }
        1 => {
            boucle(p, vec![q(6.0, 28.0), q(18.0, 16.0), q(36.0, 16.0), q(48.0, 28.0)], s);
            ligne(p, vec![q(12.0, 22.0), q(42.0, 22.0)], faible);
        }
        2 => {
            boucle(p, vec![q(14.0, 6.0), q(30.0, 10.0), q(30.0, 30.0), q(14.0, 28.0)], s);
            ligne(p, vec![q(6.0, 30.0), q(48.0, 30.0)], faible);
        }
        _ => {
            courbe(p, q(4.0, 22.0), q(27.0, 2.0), q(50.0, 22.0), s);
            let rayon = 3.0 * (r.width() / vb[0]).min(r.height() / vb[1]);
            p.circle_stroke(q(38.0, 12.0), rayon, s);
        }
    }
}

/// Comment il est posé : Face, Au sol, Mur gauche, Mur droit (boîte de 40 × 28).
fn dessin_pose(k: usize, p: &egui::Painter, r: Rect, s: Stroke) {
    let vb = [40.0, 28.0];
    let q = |x, y| pt(r, vb, x, y);
    let pts = match k {
        0 => vec![q(10.0, 4.0), q(30.0, 4.0), q(30.0, 20.0), q(10.0, 20.0)],
        1 => vec![q(4.0, 22.0), q(12.0, 12.0), q(28.0, 12.0), q(36.0, 22.0)],
        2 => vec![q(8.0, 4.0), q(18.0, 8.0), q(18.0, 24.0), q(8.0, 26.0)],
        _ => vec![q(32.0, 4.0), q(22.0, 8.0), q(22.0, 24.0), q(32.0, 26.0)],
    };
    boucle(p, pts, s);
}

/// Le cadrage d'un objectif : un personnage plus ou moins gros dans le cadre (boîte de 42 × 28).
fn dessin_objectif(k: usize, p: &egui::Painter, r: Rect, s: Stroke) {
    let vb = [42.0, 28.0];
    let q = |x, y| pt(r, vb, x, y);
    let e = (r.width() / vb[0]).min(r.height() / vb[1]);
    boucle(p, vec![q(1.0, 1.0), q(41.0, 1.0), q(41.0, 27.0), q(1.0, 27.0)], Stroke::new(1.0_f32, trait_doux()));
    // (La tête : centre, rayon ; les épaules : début, sommet de la courbe, fin.)
    let (tete, rayon, epaules): ((f32, f32), f32, Option<[(f32, f32); 3]>) = match k {
        0 => ((21.0, 14.0), 2.5, Some([(18.0, 27.0), (21.0, 18.0), (24.0, 27.0)])),
        1 => ((21.0, 12.0), 3.5, Some([(16.0, 27.0), (21.0, 13.0), (26.0, 27.0)])),
        2 => ((21.0, 11.0), 4.5, Some([(14.0, 27.0), (21.0, 9.0), (28.0, 27.0)])),
        3 => ((21.0, 10.0), 6.5, Some([(10.0, 27.0), (21.0, 11.0), (32.0, 27.0)])),
        _ => ((21.0, 12.0), 10.0, None),
    };
    p.circle_stroke(q(tete.0, tete.1), rayon * e, s);
    if let Some([a, c, b]) = epaules {
        courbe(p, q(a.0, a.1), q(c.0, c.1), q(b.0, b.1), s);
    }
}

/// La caméra, de dessus ou de côté : un triangle plein dont la pointe est en `bout`, tourné vers `dir` (unitaire).
fn triangle_camera(p: &egui::Painter, bout: Pos2, dir: egui::Vec2, taille: f32) {
    let n = vec2(-dir.y, dir.x);
    let base = bout - dir * taille;
    p.add(Shape::convex_polygon(vec![bout, base + n * taille * 0.55, base - n * taille * 0.55], camera(), Stroke::NONE));
}

// ------------------------------------------------------------------------------------ le plateau vu de dessus

/// Comment le plateau montre l'espace : le point du monde (x, z) au centre, et les points par mètre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Echelle {
    pub cx: f32,
    pub cz: f32,
    pub ppm: f32,
    pub centre: Pos2,
}

impl Echelle {
    /// Le point (x, z) du monde à l'écran : le fond (−Z) en haut, la droite (+X) à droite.
    pub fn ecran(&self, x: f32, z: f32) -> Pos2 {
        pos2(self.centre.x + (x - self.cx) * self.ppm, self.centre.y + (z - self.cz) * self.ppm)
    }

    /// Tous ces points (x, z) dans `rect`, avec une marge.
    fn cadrer(rect: Rect, pts: &[(f32, f32)]) -> Echelle {
        let (mut x0, mut x1, mut z0, mut z1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for &(x, z) in pts.iter().filter(|(x, z)| x.is_finite() && z.is_finite()) {
            (x0, x1, z0, z1) = (x0.min(x), x1.max(x), z0.min(z), z1.max(z));
        }
        if x0 > x1 {
            (x0, x1, z0, z1) = (-3.0, 3.0, -3.0, 3.0);
        }
        let (lx, lz) = ((x1 - x0).max(1.0), (z1 - z0).max(1.0));
        // (Une marge pour les noms à droite des traits, et pour la caméra en bas.)
        let ppm = ((rect.width() - 90.0).max(40.0) / lx).min((rect.height() - 30.0).max(40.0) / lz);
        Echelle { cx: (x0 + x1) / 2.0, cz: (z0 + z1) / 2.0, ppm, centre: rect.center() - vec2(28.0, 0.0) }
    }
}

/// Un calque du plateau : son rang, ses quatre coins vus de dessus, à l'écran.
#[derive(Clone, Debug)]
pub(crate) struct TraitDuPlateau {
    pub li: usize,
    pub nom: String,
    pub coins: [Pos2; 4],
    pub verrou: bool,
}

impl TraitDuPlateau {
    /// La distance de `p` au trait (0 dedans).
    fn distance(&self, p: Pos2) -> f32 {
        let c = &self.coins;
        let mut d = f32::MAX;
        for k in 0..4 {
            let (a, b) = (c[k], c[(k + 1) % 4]);
            let ab = b - a;
            let t = if ab.length_sq() > 1e-6 { ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
            d = d.min((a + ab * t - p).length());
        }
        d
    }

    fn milieu(&self) -> Pos2 {
        let c = &self.coins;
        pos2((c[0].x + c[1].x + c[2].x + c[3].x) / 4.0, (c[0].y + c[1].y + c[2].y + c[3].y) / 4.0)
    }

    fn droite(&self) -> Pos2 {
        let k = (0..4).max_by(|&a, &b| self.coins[a].x.total_cmp(&self.coins[b].x)).unwrap_or(0);
        pos2(self.coins[k].x, self.milieu().y)
    }
}

/// Le geste en cours sur le plateau : le calque tiré, l'échelle gelée, d'où part le pointeur, la profondeur d'où part le calque.
#[derive(Clone, Copy, Debug)]
struct GestePlateau {
    li: usize,
    echelle: Echelle,
    y0: f32,
    profondeur: f32,
}

/// Le geste en cours sur une réglette ou une vue de côté : l'échelle gelée et ce d'où part la valeur.
#[derive(Clone, Copy, Debug)]
struct GesteReglette {
    ppm: f32,
    x0: f32,
    valeur: f32,
    /// Vue de côté : vrai, on monte la caméra ; faux, on tire sa visée.
    hauteur: bool,
}

/// Un geste qui tourne (`App::e3_tour`) : le dernier angle du pointeur, le tour fait depuis le début, la valeur de départ.
#[derive(Clone, Copy, Debug)]
struct Tour {
    prec: Option<f32>,
    cumul: f32,
    depart: f32,
}

impl App {
    /// Les calques que le plateau montre, ses points à cadrer (les calques sauf le ciel, l'œil, un point devant la caméra), et
    /// l'œil de la caméra.
    fn e3_plateau_contenu(&self) -> (Vec<(usize, String, [m::V3; 4], bool)>, Vec<(f32, f32)>, m::V3, SceneCam) {
        let world = self.e3_world();
        let frame = self.cur_frame;
        let cam = self.current_camera();
        let sc = world.scene_cam(&cam);
        let eye = sc.pos;
        let mut cadre = vec![(eye[0], eye[2])];
        let (sp, cp) = sc.pan.to_radians().sin_cos();
        cadre.push((eye[0] + sp * world.d0, eye[2] - cp * world.d0));
        let mut calques = Vec::new();
        for li in self.e3_visible_cards().into_iter().rev() {
            let l = &self.layers()[li];
            let coins = world.card_corners(&world.carte(l, frame));
            // (Le ciel est loin derrière tout : il ne compte pas pour cadrer, il se montre au bord.)
            if !l.calque_3d.is_some_and(|c| c.sorte == Sorte3d::Ciel) {
                cadre.extend(coins.iter().map(|c| (c[0], c[2])));
            }
            calques.push((li, l.name.clone(), coins, l.locked || l.position_locked));
        }
        (calques, cadre, eye, sc)
    }

    /// « Le plateau, vu de dessus » : la caméra, son cône, chaque calque en trait à sa place. Glisser un trait : avancer ou
    /// reculer le calque (sa profondeur, comme le champ Profondeur), une étape par geste ; un clic : le choisir ; un clic sur la
    /// caméra : la choisir.
    pub(crate) fn e3_plateau(&mut self, ui: &mut egui::Ui) {
        let largeur = largeur_utile(ui);
        let (rect, resp) = zone(
            ui,
            largeur,
            HAUTEUR_PLATEAU,
            true,
            "Le plateau, vu de dessus",
            "La caméra en bas, son cône (ce qu'elle voit) et chaque calque à sa place. Glisse un calque vers le haut pour le reculer, vers le bas pour l'avancer : il garde sa taille dans le cadre. Clique un calque, ou la caméra, pour le choisir.",
        );
        let id = resp.id;
        let world = self.e3_world();
        let frame = self.cur_frame;
        let (calques, cadre, eye, sc) = self.e3_plateau_contenu();
        // (Un geste n'est lu que pendant qu'il dure : un état resté d'un glissé interrompu, le panneau ayant changé d'état, est oublié.)
        if !(resp.dragged() || resp.drag_stopped()) {
            ui.data_mut(|d| d.remove::<GestePlateau>(id));
        }
        let geste: Option<GestePlateau> = ui.data(|d| d.get_temp(id));
        let echelle = geste.map(|g| g.echelle).unwrap_or_else(|| Echelle::cadrer(rect, &cadre));
        let interieur = rect.shrink(3.0);
        let borne = |p: Pos2| pos2(p.x.clamp(interieur.left(), interieur.right()), p.y.clamp(interieur.top(), interieur.bottom()));
        let traits: Vec<TraitDuPlateau> = calques
            .iter()
            .map(|(li, nom, coins, verrou)| TraitDuPlateau { li: *li, nom: nom.clone(), coins: coins.map(|c| borne(echelle.ecran(c[0], c[2]))), verrou: *verrou })
            .collect();
        let oeil = echelle.ecran(eye[0], eye[2]);
        let pointeur = resp.hover_pos();
        // (Les calques au même endroit, d'ordinaire tous à la profondeur de la feuille, font un seul groupe.)
        let groupe_sous = |p: Pos2| -> Vec<usize> {
            let mut proches: Vec<(usize, f32)> = traits.iter().enumerate().map(|(k, t)| (k, t.distance(p))).filter(|(_, d)| *d < 7.0).collect();
            proches.sort_by(|a, b| a.1.total_cmp(&b.1));
            let Some(&(k0, _)) = proches.first() else { return Vec::new() };
            let y0 = traits[k0].milieu().y;
            traits.iter().enumerate().filter(|(_, t)| (t.milieu().y - y0).abs() < 3.0 && t.distance(p) < 7.0).map(|(k, _)| k).collect()
        };
        let sur_la_camera = |p: Pos2| (p - oeil).length() < 10.0;
        // Le calque qu'un glissé au point `p` prend : le calque courant s'il est dans le groupe et libre, sinon le premier libre
        // du haut de la pile (un calque verrouillé ne masque pas un calque libre au même endroit), sinon le premier.
        let cur = self.cur_layer;
        let pris = |p: Pos2| -> Option<usize> {
            let g = groupe_sous(p);
            let libres: Vec<usize> = g.iter().filter(|&&k| !traits[k].verrou).map(|&k| traits[k].li).collect();
            libres.iter().copied().find(|&li| li == cur).or_else(|| libres.first().copied()).or_else(|| g.first().map(|&k| traits[k].li))
        };
        // Le dernier calque qu'un clic a pris sur le plateau : cliquer de nouveau le même groupe prend le suivant.
        let id_clic = id.with("dernier_clic");

        // ---- le geste
        if resp.drag_started() {
            // (Là où le bouton s'est enfoncé, pas là où le glissé a commencé, quelques points plus loin.)
            if let Some(p) = depart(ui, &resp) {
                if let Some(li) = pris(p).filter(|_| !sur_la_camera(p)) {
                    let profondeur = world.profondeur(&self.layers()[li], frame);
                    ui.data_mut(|d| d.insert_temp(id, GestePlateau { li, echelle, y0: p.y, profondeur }));
                }
            }
        }
        let geste: Option<GestePlateau> = ui.data(|d| d.get_temp(id));
        if let (Some(g), true) = (geste, resp.dragged()) {
            let l = &self.layers()[g.li];
            if l.locked || l.position_locked {
                self.status = format!("« {} » est verrouillé : il ne bouge pas.", l.name);
            } else if let Some(p) = resp.interact_pointer_pos() {
                // (Vers le bas de l'écran : vers la caméra d'origine, +Z. La profondeur est −z / d0, `Monde::card_xf`.)
                let mut dz = (p.y - g.y0) / g.echelle.ppm;
                if self.espace3d.snap && self.espace3d.step_move > 0.0 {
                    dz = m::snap(dz, self.espace3d.step_move);
                }
                let nouvelle = g.profondeur - dz / world.d0;
                let avant = world.profondeur(l, frame);
                let mut r = resp.clone();
                if (nouvelle - avant).abs() > 1e-6 {
                    r.mark_changed();
                }
                if self.edited3d(&r, "Distance du calque") {
                    self.layers_mut()[g.li].set_depth_selon(frame, nouvelle, true);
                    self.trajectoire_espace_apres_pose(g.li);
                    self.renderer.invalidate_all();
                }
            }
        } else if resp.dragged() {
            let _ = self.edited3d(&resp, "Distance du calque");
        }
        if resp.drag_stopped() {
            ui.data_mut(|d| d.remove::<GestePlateau>(id));
        }
        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                if sur_la_camera(p) {
                    self.e3_choisir_la_camera(false);
                } else {
                    let g = groupe_sous(p);
                    // (Un groupe : le calque courant d'abord ; s'il est celui que le dernier clic a pris, le calque d'après dans
                    // le groupe. Le clic choisit le calque, le panneau passe à lui ; « Revenir à la scène » puis un nouveau clic
                    // prend le suivant.)
                    let ici = g.iter().position(|&k| traits[k].li == cur);
                    let dernier: Option<usize> = ui.data(|d| d.get_temp(id_clic));
                    let k = match ici {
                        Some(i) if dernier == Some(cur) => Some(g[(i + 1) % g.len()]),
                        Some(i) => Some(g[i]),
                        None => g.first().copied(),
                    };
                    if let Some(k) = k {
                        let li = traits[k].li;
                        ui.data_mut(|d| d.insert_temp(id_clic, li));
                        self.e3_choisir_le_calque(li);
                    }
                }
            }
        }

        // ---- le dessin
        let p = ui.painter_at(rect);
        // Le cône : ce que voit la caméra.
        let demi = sc.angle_of_view().to_radians() / 2.0;
        let a = sc.pan.to_radians();
        let dir = |t: f32| vec2(t.sin(), -t.cos());
        let loin = 4000.0;
        let cone = vec![oeil, oeil + dir(a - demi) * loin, oeil + dir(a + demi) * loin];
        p.add(Shape::convex_polygon(cone.clone(), accent().gamma_multiply(0.16), Stroke::NONE));
        ligne(&p, vec![cone[1], oeil, cone[2]], Stroke::new(0.8_f32, accent().gamma_multiply(0.8)));
        // Les calques.
        let sous = pointeur.map(&groupe_sous).unwrap_or_default();
        let tire = geste.filter(|_| resp.dragged()).map(|g| g.li);
        let mut deja: Vec<f32> = Vec::new();
        for (k, t) in traits.iter().enumerate() {
            let chaud = sous.contains(&k) || tire == Some(t.li);
            let courant = t.li == cur;
            let couleur = if chaud { accent() } else if courant { encre() } else { encre_douce() };
            boucle(&p, t.coins.to_vec(), Stroke::new(if chaud || courant { 3.0_f32 } else { 2.2 }, couleur));
            // Son nom à droite ; un groupe n'a qu'un nom (le calque courant s'il y est), et le nombre des autres.
            let y = t.milieu().y;
            if deja.iter().any(|v| (v - y).abs() < 3.0) {
                continue;
            }
            deja.push(y);
            let groupe: Vec<&TraitDuPlateau> = traits.iter().filter(|u| (u.milieu().y - y).abs() < 3.0).collect();
            let tete = groupe.iter().find(|u| u.li == cur).copied().unwrap_or(t);
            let mut nom = tete.nom.clone();
            if groupe.len() > 1 {
                nom = format!("{nom} +{}", groupe.len() - 1);
            }
            if tete.verrou {
                nom = format!("{} {nom}", ph::LOCK);
            }
            let d = tete.droite();
            let (ancre, x) = if d.x + 70.0 < rect.right() { (Align2::LEFT_CENTER, d.x + 5.0) } else { (Align2::RIGHT_BOTTOM, d.x - 2.0) };
            let c = if groupe.iter().any(|u| sous.iter().any(|&k| traits[k].li == u.li)) { accent() } else if groupe.iter().any(|u| u.li == cur) { encre() } else { encre_douce() };
            p.text(pos2(x, y), ancre, nom, police(), c);
        }
        // La caméra : un triangle dont la pointe est l'œil, tourné où elle regarde.
        let choisie = self.espace3d.sel.contains(&CAMERA_ID);
        triangle_camera(&p, oeil, dir(a), if choisie || pointeur.is_some_and(sur_la_camera) { 14.0 } else { 12.0 });
        // Survol d'un groupe : tous ses noms, en bas.
        if sous.len() > 1 {
            let noms: Vec<&str> = sous.iter().map(|&k| traits[k].nom.as_str()).collect();
            p.text(rect.left_bottom() + vec2(5.0, -4.0), Align2::LEFT_BOTTOM, noms.join(", "), police(), encre());
        }
    }

    /// Choisit le calque `li` du plateau : sa carte, et il devient le calque courant (comme la liste des cartes d'avant).
    pub(crate) fn e3_choisir_le_calque(&mut self, li: usize) {
        let Some(id) = self.layers().get(li).map(|l| l.id) else { return };
        self.espace3d.sel = vec![espace3d::card_selection(id)];
        self.select_layer(li);
        self.espace3d.sel = vec![espace3d::card_selection(id)];
        self.espace3d.last_layer = id;
    }

    // ------------------------------------------------------------------------------------ « Ajouter à la scène »

    /// Les quatre vignettes de « Ajouter à la scène » : Surface (face à la caméra), Sol, Mur (les surfaces d'avant, menu « + »
    /// des cartes) et Ciel (le calque 3D de « Nouveau calque »). Dessous, à quelle distance ils se posent.
    pub(crate) fn e3_ajouter_a_la_scene(&mut self, ui: &mut egui::Ui) {
        titre(ui, "Ajouter à la scène");
        let w = largeur_de_case(ui, 4);
        let mut demande = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = ECART;
            let cases: [(&str, &str, &str); 4] = [
                ("Surface", "espace3d.surface_face", "Un calque neuf posé face à la caméra, à la distance dite dessous : le cadre tombe en son centre."),
                ("Sol", "espace3d.surface_sol", "Un calque neuf couché au sol, devant la caméra, à la distance dite dessous."),
                ("Mur", "espace3d.surface_mur", "Un calque neuf debout sur le sol, face à la caméra, à la distance dite dessous."),
                ("Ciel", Sorte3d::Ciel.action(), "Un calque neuf tout au fond, face à la caméra, centré sur l'horizon (vingt fois la distance dite dessous)."),
            ];
            for (k, (nom, action, description)) in cases.into_iter().enumerate() {
                let r = case(ui, w, 52.0, false, nom, |p, r, s| dessin_ajout(k, p, r, s));
                if self.aide(r, nom, description, action).clicked() {
                    demande = Some(k);
                }
            }
        });
        match demande {
            Some(0) => self.espace3d_nouvelle_surface(espace3d::Surface::Face),
            Some(1) => self.espace3d_nouvelle_surface(espace3d::Surface::Sol),
            Some(2) => self.espace3d_nouvelle_surface(espace3d::Surface::Mur),
            Some(_) => self.nouveau_calque_3d(Sorte3d::Ciel),
            None => {}
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new("posés à").small().color(encre_douce()));
            let mut d = self.espace3d_distance_surface();
            let r = ui.add(egui::DragValue::new(&mut d).speed(0.05).range(0.3..=500.0).max_decimals(1).suffix(" m"));
            if crate::bulles::bulle(r, "Distance de la surface", "À combien de mètres devant la caméra un calque ajouté se pose. D'origine : celle de la feuille.", "").changed() {
                self.espace3d.surface_distance = d;
            }
            ui.label(RichText::new("de la caméra").small().color(encre_douce()));
        });
    }

    // ------------------------------------------------------------------------------------ un calque choisi

    /// Les axes (droite, haut, normale) que prend une carte posée Face, Au sol, en Mur gauche ou en Mur droit, pour la caméra du
    /// plan (comme `Monde::surface_xf` : Face suit la bascule et la rotation de la caméra, les autres sont à l'horizontale).
    pub(crate) fn e3_axes_de_pose(&self, k: usize) -> [m::V3; 3] {
        let cam = self.current_camera();
        let (sr, cr) = cam.angle.to_radians().sin_cos();
        let (st, ct) = cam.tilt.to_radians().sin_cos();
        let (sp, cp) = cam.pan.to_radians().sin_cos();
        let right = [cp, 0.0, sp];
        let up = [-sp * st, ct, cp * st];
        let back = [-sp * ct, -st, cp * ct];
        let fond = [sp, 0.0, -cp];
        let haut = [0.0, 1.0, 0.0];
        match k {
            0 => [m::add(m::scale(right, cr), m::scale(up, -sr)), m::add(m::scale(right, sr), m::scale(up, cr)), back],
            1 => [right, fond, haut],
            // (Mur à gauche : il regarde vers la droite de la caméra ; sa droite file vers le fond.)
            2 => [fond, haut, right],
            _ => [m::scale(fond, -1.0), haut, m::scale(right, -1.0)],
        }
    }

    /// La pose (0 Face, 1 Au sol, 2 Mur g., 3 Mur d.) que la carte `li` a déjà, à trois degrés près.
    pub(crate) fn e3_pose_du_calque(&self, li: usize) -> Option<usize> {
        let world = self.e3_world();
        let xf = world.carte(self.layers().get(li)?, self.cur_frame);
        let [_, y, z] = m::axes_of(&m::rotation(xf.rot)).map(m::normalize);
        let proche = 3.0_f32.to_radians().cos();
        (0..4).find(|&k| {
            let [_, ay, az] = self.e3_axes_de_pose(k);
            m::dot(z, az) > proche && m::dot(y, ay) > proche
        })
    }

    /// Pose la carte `li` Face, Au sol, en Mur g. ou en Mur d. : sa rotation dans l'espace, à sa place et à sa distance. Une
    /// étape d'annulation.
    pub(crate) fn e3_poser_le_calque(&mut self, li: usize, k: usize) {
        if self.e3_pose_du_calque(li) == Some(k) {
            // (Déjà posé ainsi : rien à faire, pas d'étape.)
            return;
        }
        let Some(l) = self.layers().get(li) else { return };
        if l.locked || l.position_locked {
            self.status = format!("« {} » est verrouillé : il ne bouge pas.", l.name);
            return;
        }
        let world = self.e3_world();
        let frame = self.cur_frame;
        let mut xf = world.carte(l, frame);
        let [x, y, z] = self.e3_axes_de_pose(k);
        #[rustfmt::skip]
        let axes: m::M4 = [
            x[0], x[1], x[2], 0.0,
            y[0], y[1], y[2], 0.0,
            z[0], z[1], z[2], 0.0,
            0.0, 0.0, 0.0, 1.0,
        ];
        xf.rot = m::euler_of(&axes);
        let nom = ["Calque de face", "Calque au sol", "Calque en mur à gauche", "Calque en mur à droite"][k.min(3)];
        self.espace3d_step(nom, |s| {
            world.apply_card_xf_libre(&mut s.layers_mut()[li], frame, &xf);
            s.trajectoire_espace_apres_pose(li);
        });
        self.renderer.invalidate_all();
    }

    /// « Comment il est posé » : quatre pictogrammes ; allumé, celui que la carte a déjà.
    pub(crate) fn e3_comment_il_est_pose(&mut self, ui: &mut egui::Ui, li: usize, actif: bool) {
        legende(ui, "Comment il est posé");
        let pose = self.e3_pose_du_calque(li);
        let w = largeur_de_case(ui, 4);
        let mut demande = None;
        ui.add_enabled_ui(actif, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = ECART;
                for (k, (nom, long, description)) in [
                    ("Face", "Face à la caméra", "Le calque parallèle à la caméra, à sa place."),
                    ("Au sol", "Au sol", "Le calque couché à l'horizontale, à sa place, son haut vers le fond."),
                    ("Mur g.", "Mur à gauche", "Le calque debout, tourné vers la droite de la caméra, à sa place."),
                    ("Mur d.", "Mur à droite", "Le calque debout, tourné vers la gauche de la caméra, à sa place."),
                ]
                .into_iter()
                .enumerate()
                {
                    let r = case(ui, w, 42.0, pose == Some(k), nom, |p, r, s| dessin_pose(k, p, r, s));
                    if crate::bulles::bulle(r, long, description, "").clicked() {
                        demande = Some(k);
                    }
                }
            });
        });
        if let Some(k) = demande {
            self.e3_poser_le_calque(li, k);
        }
    }

    /// « Distance à la caméra » : une réglette, la caméra à gauche, le calque qu'on glisse (sa profondeur, comme le champ
    /// Profondeur). La distance est mesurée le long de la profondeur de la scène.
    pub(crate) fn e3_reglette_distance(&mut self, ui: &mut egui::Ui, li: usize, actif: bool) {
        legende(ui, "Distance à la caméra");
        let world = self.e3_world();
        let frame = self.cur_frame;
        let eye = world.scene_cam(&self.current_camera()).pos;
        let Some(l) = self.layers().get(li) else { return };
        let z = world.carte(l, frame).pos[2];
        let d = eye[2] - z;
        let largeur = largeur_utile(ui);
        let (rect, resp) = zone(ui, largeur, 40.0, actif, "Distance à la caméra", "Glisse le trait : le calque s'éloigne ou s'approche de la caméra, en gardant sa taille dans le cadre (comme le champ Profondeur). En mètres, le long de la profondeur de la scène.");
        let id = resp.id;
        let (x0, x1) = (rect.left() + 18.0, rect.right() - 12.0);
        if !(resp.dragged() || resp.drag_stopped()) {
            ui.data_mut(|dd| dd.remove::<GesteReglette>(id));
        }
        let geste: Option<GesteReglette> = ui.data(|dd| dd.get_temp(id));
        let ppm = geste.map(|g| g.ppm).unwrap_or_else(|| (x1 - x0) / (d.abs() * 1.6).max(world.d0 * 2.0).max(1.0));
        if resp.drag_started() {
            if let Some(p) = depart(ui, &resp) {
                ui.data_mut(|dd| dd.insert_temp(id, GesteReglette { ppm, x0: p.x, valeur: d, hauteur: false }));
            }
        }
        let geste: Option<GesteReglette> = ui.data(|dd| dd.get_temp(id));
        if let (Some(g), Some(p)) = (geste.filter(|_| resp.dragged()), resp.interact_pointer_pos()) {
            let mut delta = (p.x - g.x0) / g.ppm;
            if self.espace3d.snap && self.espace3d.step_move > 0.0 {
                delta = m::snap(delta, self.espace3d.step_move);
            }
            // (z = −d0 · profondeur, `Monde::card_xf` : à la distance D de l'œil, la profondeur est (D − œil.z) / d0.)
            let nouvelle = (g.valeur + delta - eye[2]) / world.d0;
            let mut r = resp.clone();
            if (nouvelle - world.profondeur(l, frame)).abs() > 1e-6 {
                r.mark_changed();
            }
            if self.edited3d(&r, "Distance du calque") {
                self.layers_mut()[li].set_depth_selon(frame, nouvelle, true);
                self.trajectoire_espace_apres_pose(li);
                self.renderer.invalidate_all();
            }
        } else if resp.dragged() {
            let _ = self.edited3d(&resp, "Distance du calque");
        }
        if resp.drag_stopped() {
            ui.data_mut(|dd| dd.remove::<GesteReglette>(id));
        }
        // Le dessin.
        let p = ui.painter_at(rect);
        let y = rect.center().y + 6.0;
        ligne(&p, vec![pos2(x0, y), pos2(x1, y)], Stroke::new(1.0_f32, trait_doux()));
        // (Une graduation tous les 1, 2, 5, 10… mètres, une dizaine sur la réglette.)
        let portee = (x1 - x0) / ppm;
        let pas = [0.5_f32, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0].into_iter().find(|s| portee / s <= 12.0).unwrap_or(1000.0);
        let mut v = pas;
        while v < portee {
            let x = x0 + v * ppm;
            ligne(&p, vec![pos2(x, y - 2.5), pos2(x, y + 2.5)], Stroke::new(1.0_f32, trait_doux()));
            v += pas;
        }
        triangle_camera(&p, pos2(x0, y), vec2(1.0, 0.0), 11.0);
        let x = (x0 + d * ppm).clamp(x0, x1);
        let chaud = resp.hovered() || resp.dragged();
        let c = if !actif { encre_douce() } else if chaud { accent() } else { encre() };
        ligne(&p, vec![pos2(x, y - 11.0), pos2(x, y + 11.0)], Stroke::new(3.0_f32, c));
        let texte = if d < 0.0 { format!("derrière, {} m", nombre(-d, 1)) } else { format!("{} m", nombre(d, 1)) };
        let ancre = if x > rect.right() - 50.0 { Align2::RIGHT_TOP } else { Align2::CENTER_TOP };
        p.text(pos2(x, rect.top() + 1.0), ancre, texte, police(), encre_douce());
    }

    /// « Taille dans le cadre » : un curseur entre deux carrés (l'échelle du calque, comme le champ Échelle).
    pub(crate) fn e3_taille_dans_le_cadre(&mut self, ui: &mut egui::Ui, li: usize, actif: bool) {
        legende(ui, "Taille dans le cadre");
        let frame = self.cur_frame;
        let Some(l) = self.layers().get(li) else { return };
        let mut t = l.pose(frame);
        let mut pct = t.scale * 100.0;
        let carre = |ui: &mut egui::Ui, cote: f32| {
            let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
            ui.painter().rect_stroke(Rect::from_center_size(r.center(), vec2(cote, cote)), 0.0, Stroke::new(1.0_f32, encre_douce()), StrokeKind::Inside);
        };
        let mut change = false;
        ui.add_enabled_ui(actif, |ui| {
            ui.horizontal(|ui| {
                carre(ui, 8.0);
                ui.spacing_mut().slider_width = (largeur_utile(ui) - 18.0 - ui.spacing().item_spacing.x * 2.0).max(40.0);
                let r = ui.add(egui::Slider::new(&mut pct, 10.0..=1000.0).logarithmic(true).show_value(false).clamping(egui::SliderClamping::Never));
                let r = crate::bulles::bulle(r, "Taille dans le cadre", &format!("{} % : glisse vers le grand carré pour agrandir le calque, vers le petit pour le réduire.", nombre(pct, 0)), "");
                change = self.edited3d(&r, "Échelle du calque");
                carre(ui, 16.0);
            });
        });
        if change {
            t.scale = (pct / 100.0).clamp(0.01, 100.0);
            let layer = &mut self.layers_mut()[li];
            let mut p = layer.pose(frame);
            p.scale = t.scale;
            layer.set_pose_selon(frame, p, true);
            self.trajectoire_espace_apres_pose(li);
            self.renderer.invalidate_all();
        }
    }

    // ------------------------------------------------------------------------------------ la caméra choisie

    /// Un geste sur la caméra du plan : `f` change la prise de vue ; vrai si elle a changé. Le chemin du champ de la caméra
    /// (`set_current_camera_libre`, sa clé de l'image courante si elle est animée), une étape d'annulation par geste.
    pub(crate) fn e3_geste_camera(&mut self, resp: &Response, nom: &str, f: impl FnOnce(&mut SceneCam)) -> bool {
        let world = self.e3_world();
        let base = self.current_camera();
        let avant = world.scene_cam(&base);
        let mut sc = avant;
        f(&mut sc);
        let mut r = resp.clone();
        if sc != avant {
            r.mark_changed();
        }
        if !self.edited3d(&r, nom) {
            return false;
        }
        let sc = sc.sanitized();
        let mut new = world.camera_from_scene(&sc);
        // (Le zoom ne change que si la focale a changé : un zoom hors des bornes de l'objectif ne se perd pas.)
        if (sc.focal - base.focal().clamp(MIN_FOCAL, MAX_FOCAL)).abs() < 1e-3 {
            new.zoom = base.zoom;
        }
        self.set_current_camera_libre(new);
        self.modified = true;
        true
    }

    /// « Objectif » : cinq vignettes de cadrage (un clic, la focale), leur légende, et le cône vu de dessus qu'on glisse pour une
    /// focale libre.
    pub(crate) fn e3_objectif(&mut self, ui: &mut egui::Ui) {
        legende(ui, "Objectif");
        let world = self.e3_world();
        let sc = world.scene_cam(&self.current_camera());
        let w = largeur_de_case(ui, OBJECTIFS.len());
        let mut demande = None;
        let mut reponse = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = ECART;
            for (k, mm) in OBJECTIFS.into_iter().enumerate() {
                let r = case(ui, w, 36.0, (sc.focal - mm).abs() < 0.5, &format!("{mm:.0}"), |p, r, s| dessin_objectif(k, p, r, s));
                let mots = LEGENDE_OBJECTIFS.split(" · ").nth(k).unwrap_or("");
                let r = crate::bulles::bulle(r, &format!("Objectif {mm:.0} mm"), &format!("{} : un clic, la caméra prend cette focale.", mots), "");
                if r.clicked() {
                    demande = Some(mm);
                    reponse = Some(r);
                }
            }
        });
        if let (Some(mm), Some(r)) = (demande, reponse) {
            self.e3_geste_camera(&r, "Objectif de la caméra", |c| c.focal = mm);
        }
        ui.label(RichText::new(LEGENDE_OBJECTIFS).small().color(encre_douce()));
        // Le cône : la caméra à gauche, le champ ouvert vers la droite ; glisser à droite resserre (focale plus longue).
        let largeur = largeur_utile(ui);
        let (rect, resp) = zone(ui, largeur, 38.0, true, "Champ de la caméra", "Le champ de la caméra, vu de dessus. Glisse vers la droite pour resserrer (focale plus longue), vers la gauche pour l'élargir.");
        if resp.dragged() {
            let dx = resp.drag_delta().x;
            self.e3_geste_camera(&resp, "Focale de la caméra", |c| c.focal = (c.focal * (dx * 0.01).exp()).clamp(MIN_FOCAL, MAX_FOCAL));
        }
        let sc = world.scene_cam(&self.current_camera());
        let p = ui.painter_at(rect);
        let o = pos2(rect.left() + 14.0, rect.center().y);
        let demi = (sc.angle_of_view().to_radians() / 2.0).min(1.5);
        let loin = 2000.0;
        let haut = o + vec2(demi.cos(), -demi.sin()) * loin;
        let bas = o + vec2(demi.cos(), demi.sin()) * loin;
        let chaud = resp.hovered() || resp.dragged();
        p.add(Shape::convex_polygon(vec![o, haut, bas], accent().gamma_multiply(if chaud { 0.26 } else { 0.18 }), Stroke::NONE));
        ligne(&p, vec![haut, o, bas], Stroke::new(1.0_f32, accent()));
        triangle_camera(&p, o, vec2(1.0, 0.0), 11.0);
        let texte = format!("{} mm · champ {}°", nombre(sc.focal, 0), nombre(sc.angle_of_view(), 0));
        p.text(rect.right_bottom() + vec2(-6.0, -3.0), Align2::RIGHT_BOTTOM, texte, police(), encre());
    }

    /// « Hauteur et plongée » : la caméra de côté, à côté d'un personnage de 1,70 m. Glisser la caméra : sa hauteur ; glisser
    /// ailleurs : sa ligne de visée (la bascule).
    pub(crate) fn e3_hauteur_et_plongee(&mut self, ui: &mut egui::Ui) {
        legende(ui, "Hauteur et plongée");
        let world = self.e3_world();
        let sc = world.scene_cam(&self.current_camera());
        let sol = world.sol_y();
        let h = sc.pos[1] - sol;
        let largeur = largeur_utile(ui);
        let (rect, resp) = zone(ui, largeur, 76.0, true, "Hauteur et plongée", "Monte ou descends la caméra en la glissant ; glisse ailleurs pour tirer sa ligne de visée : plongée vers le bas, contre-plongée vers le haut. Le personnage mesure 1,70 m.");
        let id = resp.id;
        let sol_y = rect.bottom() - 9.0;
        let haut_utile = sol_y - rect.top() - 16.0;
        if !(resp.dragged() || resp.drag_stopped()) {
            ui.data_mut(|d| d.remove::<GesteReglette>(id));
        }
        let geste: Option<GesteReglette> = ui.data(|d| d.get_temp(id));
        let ppm = geste.map(|g| g.ppm).unwrap_or_else(|| haut_utile / (h + 0.6).max(3.0));
        let cx = rect.left() + 28.0;
        let cam_y = |h: f32| (sol_y - h * ppm).clamp(rect.top() + 6.0, sol_y + 4.0);
        let cam = pos2(cx, cam_y(h));
        if resp.drag_started() {
            if let Some(p) = depart(ui, &resp) {
                let sur = (p - cam).length() < 16.0;
                ui.data_mut(|d| d.insert_temp(id, GesteReglette { ppm, x0: p.y, valeur: h, hauteur: sur }));
            }
        }
        let geste: Option<GesteReglette> = ui.data(|d| d.get_temp(id));
        if let (Some(g), Some(p)) = (geste.filter(|_| resp.dragged()), resp.interact_pointer_pos()) {
            if g.hauteur {
                let mut dh = (g.x0 - p.y) / g.ppm;
                if self.espace3d.snap && self.espace3d.step_move > 0.0 {
                    dh = m::snap(dh, self.espace3d.step_move);
                }
                let nouvelle = g.valeur + dh;
                self.e3_geste_camera(&resp, "Hauteur de la caméra", |c| c.pos[1] = sol + nouvelle);
            } else {
                let v = p - cam;
                let t = if v.x > 3.0 { (-v.y).atan2(v.x).to_degrees() } else if v.y < 0.0 { MAX_TILT } else { -MAX_TILT };
                let t = t.clamp(-MAX_TILT, MAX_TILT);
                self.e3_geste_camera(&resp, "Bascule de la caméra", |c| c.tilt = t);
            }
        } else if resp.dragged() {
            let _ = self.edited3d(&resp, "Hauteur de la caméra");
        }
        if resp.drag_stopped() {
            ui.data_mut(|d| d.remove::<GesteReglette>(id));
        }
        // Le dessin, avec la caméra d'après le geste.
        let sc = world.scene_cam(&self.current_camera());
        let h = sc.pos[1] - sol;
        let cam = pos2(cx, cam_y(h));
        let p = ui.painter_at(rect);
        ligne(&p, vec![pos2(rect.left(), sol_y), pos2(rect.right(), sol_y)], Stroke::new(1.0_f32, trait_doux()));
        // Le personnage de référence, 1,70 m.
        let px = rect.left() + rect.width() * 0.63;
        let taille = 1.7 * ppm;
        let tete = (taille * 0.075).clamp(2.5, 9.0);
        let s = Stroke::new(1.2_f32, encre_douce());
        let cou = sol_y - taille + tete * 2.0;
        let bassin = sol_y - taille * 0.47;
        p.circle_stroke(pos2(px, sol_y - taille + tete), tete, s);
        ligne(&p, vec![pos2(px, cou), pos2(px, bassin)], s);
        ligne(&p, vec![pos2(px - taille * 0.14, cou + taille * 0.1), pos2(px + taille * 0.14, cou + taille * 0.1)], s);
        ligne(&p, vec![pos2(px - taille * 0.11, sol_y), pos2(px, bassin), pos2(px + taille * 0.11, sol_y)], s);
        // La ligne de visée.
        let t = sc.tilt.to_radians();
        let dir = vec2(t.cos(), -t.sin());
        let chaud = resp.hovered() || resp.dragged();
        let visee = Stroke::new(if chaud { 1.6_f32 } else { 1.1 }, accent());
        p.extend(Shape::dashed_line(&[cam, cam + dir * 1000.0], visee, 4.0, 3.0));
        p.circle_filled(cam + dir * 70.0, 3.0, accent());
        // La caméra, et son aplomb.
        p.extend(Shape::dashed_line(&[pos2(cx, cam.y + 6.0), pos2(cx, sol_y)], Stroke::new(1.0_f32, camera()), 2.0, 3.0));
        let corps = Rect::from_center_size(cam, vec2(18.0, 12.0));
        p.rect_filled(corps, CornerRadius::same(2), camera());
        let texte = format!("Hauteur {} m · {}", nombre(h, 1), plongee(sc.tilt));
        p.text(rect.left_top() + vec2(6.0, 3.0), Align2::LEFT_TOP, texte, police(), encre());
    }

    /// « Panoramique et horizon » : un cadran qu'on tourne (le panoramique) et un horizon qu'on penche (la rotation).
    pub(crate) fn e3_panoramique_et_horizon(&mut self, ui: &mut egui::Ui) {
        legende(ui, "Panoramique et horizon");
        let world = self.e3_world();
        let largeur = largeur_utile(ui);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            // Le cadran : la caméra vue de dessus au centre, son cône vers où elle regarde (le haut : le fond de la scène).
            let (rect, resp) = zone(ui, COTE_TOUR, COTE_TOUR, true, "Panoramique", "Tourne le cadran : la caméra pivote sur elle-même, vers la gauche ou la droite. En haut : le fond de la scène.");
            let c = pos2(rect.center().x, rect.top() + CENTRE_TOUR);
            let angle = |p: Pos2| (p.x - c.x).atan2(-(p.y - c.y)).to_degrees();
            let depart = world.scene_cam(&self.current_camera()).pan;
            if let Some(cible) = self.e3_tour(ui, &resp, c, 6.0, angle, depart) {
                self.e3_geste_camera(&resp, "Panoramique de la caméra", |s| s.pan = cible);
            }
            let sc = world.scene_cam(&self.current_camera());
            let p = ui.painter_at(rect);
            let r = CENTRE_TOUR - 6.0;
            p.circle_stroke(c, r, Stroke::new(1.0_f32, trait_doux()));
            ligne(&p, vec![c + vec2(0.0, -r - 3.0), c + vec2(0.0, -r + 3.0)], Stroke::new(1.0_f32, trait_doux()));
            let a = sc.pan.to_radians();
            let demi = (sc.angle_of_view().to_radians() / 2.0).min(1.5);
            let dir = |t: f32| vec2(t.sin(), -t.cos());
            let chaud = resp.hovered() || resp.dragged();
            ligne(&p, vec![c + dir(a - demi) * r, c, c + dir(a + demi) * r], Stroke::new(if chaud { 1.6_f32 } else { 1.0 }, accent()));
            p.circle_filled(c + dir(a) * r, 4.5, camera());
            p.text(pos2(c.x, rect.bottom() - 2.0), Align2::CENTER_BOTTOM, format!("pano {}°", nombre(sc.pan, 0)), police(), encre());

            // L'horizon : le cadre de la caméra fixe, l'horizon tel que l'image le montre.
            let reste = (largeur - COTE_TOUR - 8.0).max(60.0);
            let (rect, resp) = zone(ui, reste, COTE_TOUR, true, "Horizon", "Penche l'horizon en le tournant : la caméra tourne autour de sa visée. L'horizon est montré tel que l'image le verra.");
            let c = pos2(rect.center().x, rect.top() + CENTRE_TOUR);
            // (Le cadre tourne dans le sens horaire quand la rotation grandit : l'horizon de l'image, lui, dans l'autre sens. On
            // tourne donc l'horizon, et la rotation de la caméra va à l'opposé.)
            let angle = |p: Pos2| -(p.y - c.y).atan2(p.x - c.x).to_degrees();
            let depart = world.scene_cam(&self.current_camera()).roll;
            if let Some(cible) = self.e3_tour(ui, &resp, c, 8.0, angle, depart) {
                self.e3_geste_camera(&resp, "Rotation de la caméra", |s| s.roll = cible);
            }
            let sc = world.scene_cam(&self.current_camera());
            let p = ui.painter_at(rect);
            let fw = world.frame[0].max(1) as f32;
            let fh = world.frame[1].max(1) as f32;
            let cw = (rect.width() * 0.62).min(54.0 * fw / fh);
            let cadre = Rect::from_center_size(c, vec2(cw, cw * fh / fw));
            p.rect_stroke(cadre, 0.0, Stroke::new(1.0_f32, camera()), StrokeKind::Inside);
            let t = (-sc.roll).to_radians();
            let d = vec2(t.cos(), t.sin()) * rect.width();
            let chaud = resp.hovered() || resp.dragged();
            ligne(&p, vec![c - d, c + d], Stroke::new(if chaud { 2.4_f32 } else { 1.8 }, encre()));
            p.text(pos2(c.x, rect.bottom() - 2.0), Align2::CENTER_BOTTOM, format!("horizon {}°", nombre(sc.roll, 0)), police(), encre());
        });
    }

    /// Un geste qui tourne autour du point `c` (le cadran, l'horizon) : l'angle du pointeur (`angle`, en degrés) est suivi d'une
    /// image à l'autre sans saut (plus d'un tour compris), et la valeur cible est `depart` (lue au début du geste) plus le tour
    /// fait, au pas de l'aimantation si elle est allumée. Près du centre (`mort`), l'angle ne compte pas.
    fn e3_tour(&mut self, ui: &egui::Ui, resp: &Response, c: Pos2, mort: f32, angle: impl Fn(Pos2) -> f32, depart: f32) -> Option<f32> {
        let id = resp.id;
        if !(resp.dragged() || resp.drag_stopped()) {
            ui.data_mut(|d| d.remove::<Tour>(id));
        }
        if resp.drag_started() {
            // (Un appui près du centre ne dit pas d'angle : le premier angle est pris hors de la zone morte.)
            let p = crate::espace3dgraphes::depart(ui, resp);
            let prec = p.filter(|p| (*p - c).length() > mort).map(&angle);
            ui.data_mut(|d| d.insert_temp(id, Tour { prec, cumul: 0.0, depart }));
        }
        if resp.drag_stopped() {
            ui.data_mut(|d| d.remove::<Tour>(id));
            return None;
        }
        if !resp.dragged() {
            return None;
        }
        let (Some(p), Some(mut t)) = (resp.interact_pointer_pos(), ui.data(|d| d.get_temp::<Tour>(id))) else {
            let _ = self.edited3d(resp, "Orienter la caméra");
            return None;
        };
        if (p - c).length() > mort {
            let a = angle(p);
            if let Some(prec) = t.prec {
                t.cumul += ecart(a, prec);
            }
            t.prec = Some(a);
            ui.data_mut(|d| d.insert_temp(id, t));
        }
        let tour = if self.espace3d.snap && self.espace3d.step_rot > 0.0 { m::snap(t.cumul, self.espace3d.step_rot) } else { t.cumul };
        Some(t.depart + tour)
    }

    /// La cible de « Viser la sélection » : les cartes choisies, ou, la caméra choisie seule, le calque courant.
    pub(crate) fn e3_cible_de_la_visee(&self) -> Vec<u64> {
        let ids: Vec<u64> = self.espace3d.sel.iter().copied().filter(|i| *i != CAMERA_ID && self.e3_card_layer(*i).is_some()).collect();
        if !ids.is_empty() {
            return ids;
        }
        let l = self.layer();
        if espace3d::is_card(l) && crate::folders::effect(self.layers(), self.cur_layer).is_some() {
            vec![espace3d::card_selection(l.id)]
        } else {
            Vec::new()
        }
    }

    /// « Regarder par elle » et « Viser la sélection », en gros boutons.
    pub(crate) fn e3_boutons_camera(&mut self, ui: &mut egui::Ui) {
        let w = largeur_de_case(ui, 2);
        let mut regarder = false;
        let mut viser = false;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let on = self.e3_par_la_camera();
            let b = egui::Button::new(crate::bulles::encre_etat(RichText::new(format!("{}  Regarder par elle", ph::BINOCULARS)), on)).selected(on).min_size(vec2(w, 22.0));
            let r = ui.add(b);
            regarder = self.aide(r, "Regarder par la caméra", "La vue montre ce que la caméra du plan filme, au format du cadre (avec plusieurs vues : la vue active).", "espace3d.vue_camera").clicked();
            let cible = !self.e3_cible_de_la_visee().is_empty();
            let r = ui.add_enabled(cible, egui::Button::new(format!("{}  Viser la sélection", ph::CROSSHAIR)).min_size(vec2(w, 22.0)));
            viser = crate::bulles::bulle(r, "Viser la sélection", "Tourne la caméra vers les calques choisis (la caméra choisie seule : vers le calque courant).", "").clicked();
        });
        if regarder {
            self.e3_basculer_par_la_camera();
        }
        if viser {
            let ids = self.e3_cible_de_la_visee();
            let world = self.e3_world();
            let target = crate::espace3dgizmo::pivot_of(&self.e3_xfs(&ids));
            let mut sc = world.scene_cam(&self.current_camera());
            sc.aim_at(target);
            let aimed = crate::camera::Camera { pan: sc.pan, tilt: sc.tilt, ..self.current_camera() };
            self.espace3d_step("Viser avec la caméra", |s| s.set_current_camera_libre(aimed));
        }
    }

    /// « ▸ Précis » : un titre qu'on clique pour déplier ou replier ce qui suit ; vrai s'il est déplié.
    pub(crate) fn e3_precis(ui: &mut egui::Ui, ouvert: &mut bool, quoi: &str) -> bool {
        // (Partie 164.4 : le même titre que les panneaux des parties 163 et 164, un triangle peint.)
        let r = crate::visuels164::titre_precis(ui, *ouvert, &format!("Précis : {quoi}"));
        if crate::bulles::bulle(r, "Précis", "Les mêmes réglages en chiffres, à taper ou à glisser.", "").clicked() {
            *ouvert = !*ouvert;
        }
        *ouvert
    }
}
