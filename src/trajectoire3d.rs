//! La Trajectoire dans l'Espace 3D (`docs/INTERFACE.md`, partie 149). Anthony, 2026-11-12 : « Je voudrais qu'on essaye
//! d'intégrer l'outil trajectoire dans l'environnement 3D, afin de générer des mouvements dans l'espace. » Puis : « Un calque
//! ou la caméra, les deux. Ce qui est sélectionné est sujet à la trajectoire. »
//!
//! **Où.** Dans la vue Espace 3D d'un « Plan en Espace 3D », avec l'outil Trajectoire. Sur la feuille, l'outil est celui de
//! toujours (`trajectoireui.rs`) ; dans un plan ordinaire, la vue répond qu'il faut un plan en Espace 3D.
//!
//! **Le chemin est posé sur une surface.** À l'appui, le regard qui passe sous le pointeur choisit le plan qui porte tout le
//! trait : la carte la plus proche qu'il touche et qui n'est pas choisie ; sinon le sol (`Monde::sol_y`) ; sinon un plan face
//! à la vue, par la place de ce qui est choisi. Le modèle est celui de la feuille (`trajectoire::Trajectoire`), avec ce plan
//! en plus (`trajectoire::Support`) : marques, temps, tirage et recalage sont les mêmes calculs, dans les coordonnées du plan.
//!
//! **Ce qui est choisi suit le chemin** (`Sujet`) : chaque carte choisie et la caméra du plan ; rien de choisi, la carte du
//! calque courant. Chacun reçoit sa copie de la trajectoire et se déplace, dans l'espace, du déplacement du chemin depuis son
//! départ : il garde sa place au départ, son orientation et sa taille dans l'espace.
//! * **Une carte** reçoit des clés de position (`cles_dans_l_espace`) : une par image en clés d'interpolation, une par jalon,
//!   tenue, en clés d'images. En clés d'images elle reçoit aussi, comme sur la feuille, **une clé de dessin à chaque jalon**
//!   (`trajectoire_poser_les_dessins_dans_l_espace`, partie 152 ; Anthony, 2026-11-13 : « Clés d'images ne crée pas de clés
//!   d'images dans la timeline ») : une copie du dessin du départ, ni déplacée ni agrandie ni tournée, puisque c'est la carte
//!   qui porte le mouvement. Sa pose propre est sa pose fixe (`Layer::transform`, `Layer::depth`), que la trajectoire ne
//!   touche jamais.
//! * **La caméra** ne reçoit aucune clé : sa trajectoire (`Panel::trajectoire_camera`) s'ajoute à la caméra des clés quand on
//!   l'évalue (`Document::panel_camera_reelle`, `App::current_camera`).
//!
//! **Les gestes** sont ceux de la feuille, et s'appliquent à tout ce qui est choisi, en une étape d'annulation. Les
//! tolérances (« près du chemin », « tirée loin ») se mesurent en pixels d'écran, sur le chemin tel que la vue le montre
//! (`CheminVu`), jamais avec le zoom de la feuille.

use eframe::egui::{self, vec2, Align2, Color32, CursorIcon, Event, FontId, PointerButton, Pos2, Rect, RichText, Shape, Stroke, Vec2};

use crate::app::{App, Tool};
use crate::document::{Document, Layer, LayerTransform};
use crate::espace3d::{self, Monde, Xf};
use crate::espace3ddessin::Regard3d;
use crate::espace3dgizmo::CAMERA_ID;
use crate::espace3dmath as m;
use crate::espace3dui::Drag;
use crate::history::Edit;
use crate::poskeys::PosKey;
use crate::trajectoire::{Cles, DessinJalon, Support, Trajectoire, MARQUE_FRACTION_MAX};
use crate::trajectoireui::{ARRACHE_ECRAN, COULEUR, TOL_ECRAN};

pub(crate) const PLAN_ORDINAIRE: &str = "Trajectoire dans l'espace : ce plan n'est pas un « Plan en Espace 3D ». Coche « Plan en Espace 3D » dans le panneau Espace 3D ; ou trace la trajectoire sur la feuille, hors de cette vue.";
pub(crate) const PRISE: &str = "Trajectoire dans l'espace : trace d'un trait, sur une carte ou sur le sol, le chemin de ce qui est choisi (cartes, caméra). Maj + un trait depuis un bout du chemin : le prolonger. Ctrl+clic : choisir une carte ou la caméra, seule ; Maj+Ctrl+clic : en ajouter une, ou la lâcher. Alt+glisser ou clic droit : tourner la vue.";
pub(crate) const RIEN_A_DEPLACER: &str = "Trajectoire : rien n'est choisi qui puisse suivre un chemin. Choisis une carte ou la caméra (Ctrl+clic dans la vue, ou la liste des cartes et « choisir » dans le panneau Espace 3D).";
pub(crate) const PAS_DE_SURFACE: &str = "Trajectoire : pas de surface sous le pointeur pour y poser le chemin. Tourne la vue (Alt+glisser ou clic droit) pour voir le sol ou une carte.";
pub(crate) const TROP_COURT: &str = "Trajectoire : trace d'un trait plus long, sur une carte ou sur le sol, le chemin de ce qui est choisi ; puis de petits traits en travers pour les marques.";
pub(crate) const SUR_LA_FEUILLE: &str = "Cette trajectoire est tracée dans l'espace : elle se retouche dans la vue Espace 3D, avec ce même outil.";
pub(crate) const ABANDON: &str = "Geste de trajectoire abandonné.";

/// Un trait plus court que ça à l'écran n'est qu'un clic : ni chemin, ni marque.
const CLIC_ECRAN: f32 = 8.0;
/// Un petit trait qui ne croise pas le chemin est une marque s'il passe à moins de tant de pixels d'écran de lui.
const MARQUE_ECRAN: f32 = 6.0;
/// La demi-longueur d'une marque à l'écran.
const DEMI_MARQUE: f32 = 7.0;

/// Ce qui suit une trajectoire dans l'espace : la carte d'un calque (son numéro), ou la caméra du plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Sujet {
    Carte(u64),
    Camera,
}

/// Un sujet tel qu'il était avant un geste (pour Échap) : sa trajectoire, et les clés de position d'une carte.
pub(crate) struct Avant {
    sujet: Sujet,
    trajectoire: Trajectoire,
    cles: Vec<PosKey>,
}

/// Un geste de l'outil en cours dans la vue Espace 3D.
pub(crate) enum GesteEspace {
    /// Un trait : le chemin, ou une marque, on le saura au lâcher. Le plan qui le porte, ses points à l'écran et dans ce plan.
    Trait { support: Support, ecran: Vec<Pos2>, plan: Vec<[f32; 2]> },
    /// La marque `i` prise : glissée le long du chemin, ou arrachée.
    Marque { i: usize, avant: Vec<Avant>, snap: Edit, bouge: bool, arrachee: bool },
    /// Le chemin pris en `fraction` (le pointeur étant alors en `depart`, dans le plan du chemin) : tiré là, ou déplacé en bloc.
    Chemin { depart: [f32; 2], fraction: f32, bloc: bool, avant: Vec<Avant>, snap: Edit, bouge: bool },
    /// Maj + appui sur un bout du chemin, puis un trait sur le plan qui porte le chemin : il le prolonge (`devant` : depuis
    /// son début), au lâcher, pour tout ce qui est choisi (`trajectoireprolonge.rs`, partie 151).
    Prolonger { support: Support, devant: bool, ecran: Vec<Pos2>, plan: Vec<[f32; 2]> },
}

fn pluriel(n: usize) -> &'static str {
    if n > 1 { "s" } else { "" }
}

// ------------------------------------------------------------------------------------------------ le chemin vu

/// Le croisement des segments `ab` et `cd` à l'écran : la position sur `ab` (0 à 1), ou rien.
fn croisement(a: Pos2, b: Pos2, c: Pos2, d: Pos2) -> Option<f32> {
    let (r, s) = (b - a, d - c);
    let den = r.x * s.y - r.y * s.x;
    if den.abs() < 1e-9 {
        return None;
    }
    let ac = c - a;
    let t = (ac.x * s.y - ac.y * s.x) / den;
    let u = (ac.x * r.y - ac.y * r.x) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some(t)
}

/// **Le chemin tel qu'une vue le montre** : chaque point du chemin à l'écran (rien : derrière l'œil), et la fraction du
/// chemin où il est. C'est sur lui que se mesurent, en pixels d'écran, « près du chemin », « en travers du chemin » et
/// « tirée loin du chemin ».
pub(crate) struct CheminVu {
    pts: Vec<Option<Pos2>>,
    fr: Vec<f32>,
}

impl CheminVu {
    pub(crate) fn de(t: &Trajectoire, ecran: &dyn Fn([f32; 2]) -> Option<Pos2>) -> CheminVu {
        let total = t.longueur().max(1e-6);
        let mut parcouru = 0.0;
        let mut fr = Vec::with_capacity(t.points.len());
        for (i, p) in t.points.iter().enumerate() {
            if i > 0 {
                parcouru += crate::trajectoire::distance(t.points[i - 1], *p);
            }
            fr.push((parcouru / total).clamp(0.0, 1.0));
        }
        CheminVu { pts: t.points.iter().map(|p| ecran(*p)).collect(), fr }
    }

    /// Les segments visibles : leurs deux bouts à l'écran, et la fraction du chemin de chacun.
    fn segments(&self) -> impl Iterator<Item = (Pos2, Pos2, f32, f32)> + '_ {
        (0..self.pts.len().saturating_sub(1)).filter_map(move |i| Some((self.pts[i]?, self.pts[i + 1]?, self.fr[i], self.fr[i + 1])))
    }

    /// La longueur du chemin à l'écran (pixels).
    pub(crate) fn longueur(&self) -> f32 {
        self.segments().map(|(a, b, ..)| a.distance(b)).sum()
    }

    /// La fraction du chemin la plus proche du point `p` de l'écran, et la distance (pixels) qui les sépare.
    pub(crate) fn plus_proche(&self, p: Pos2) -> Option<(f32, f32)> {
        let mut meilleur: Option<(f32, f32)> = None;
        for (a, b, fa, fb) in self.segments() {
            let d = b - a;
            let l2 = d.length_sq();
            let w = if l2 > 0.0 { ((p - a).dot(d) / l2).clamp(0.0, 1.0) } else { 0.0 };
            let dist = (a + d * w).distance(p);
            if meilleur.is_none_or(|(_, autre)| dist < autre) {
                meilleur = Some((fa + (fb - fa) * w, dist));
            }
        }
        meilleur
    }

    /// Ce trait (points de l'écran) est-il une marque ? Il l'est s'il est court devant le chemin tel qu'on le voit et le
    /// croise, ou le frôle : la fraction du chemin où il le croise.
    pub(crate) fn marque_de(&self, coup: &[Pos2]) -> Option<f32> {
        let l: f32 = coup.windows(2).map(|w| w[0].distance(w[1])).sum();
        if l < CLIC_ECRAN || l > self.longueur() * MARQUE_FRACTION_MAX {
            return None;
        }
        for (a, b, fa, fb) in self.segments() {
            for s in coup.windows(2) {
                if let Some(w) = croisement(a, b, s[0], s[1]) {
                    return Some(fa + (fb - fa) * w);
                }
            }
        }
        coup.iter().filter_map(|p| self.plus_proche(*p)).filter(|(_, d)| *d <= MARQUE_ECRAN).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(f, _)| f)
    }
}

/// La marque de `t` dont le point, à l'écran, est à moins de `TOL_ECRAN` pixels de `p` (la plus proche).
fn marque_sous(t: &Trajectoire, ecran: &dyn Fn([f32; 2]) -> Option<Pos2>, p: Pos2) -> Option<usize> {
    t.marques
        .iter()
        .enumerate()
        .filter_map(|(i, &f)| Some((i, ecran(t.point_a(f))?.distance(p))))
        .filter(|(_, d)| *d <= TOL_ECRAN)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// **Le chemin d'une trajectoire dans l'espace, tracé par-dessus une vue** (`ecran` : du point du chemin à l'écran ; rien :
/// derrière l'œil) : la ligne, la barre du départ, la flèche de l'arrivée, les marques, le numéro de l'image de chaque jalon
/// (`images`), et un point là où l'on en est (`courante`, une fraction du chemin). `tenue` : la marque prise ou survolée.
pub(crate) fn peindre_chemin(painter: &egui::Painter, t: &Trajectoire, images: &[usize], courante: Option<f32>, ecran: &dyn Fn([f32; 2]) -> Option<Pos2>, tenue: Option<usize>) {
    // La ligne, par morceaux : ce qui passe derrière l'œil n'est pas tracé.
    let tracer = |pts: &mut Vec<Pos2>| {
        if pts.len() >= 2 {
            painter.add(Shape::line(pts.clone(), Stroke::new(3.5_f32, Color32::from_black_alpha(90))));
            painter.add(Shape::line(std::mem::take(pts), Stroke::new(1.5_f32, COULEUR)));
        } else {
            pts.clear();
        }
    };
    let mut morceau: Vec<Pos2> = Vec::new();
    for p in &t.points {
        match ecran(*p) {
            Some(q) => morceau.push(q),
            None => tracer(&mut morceau),
        }
    }
    tracer(&mut morceau);
    // La direction du chemin à l'écran, à la fraction `f`.
    let direction = |f: f32| -> Option<Vec2> {
        let (a, b) = (ecran(t.point_a((f - 0.01).max(0.0)))?, ecran(t.point_a((f + 0.01).min(1.0)))?);
        let d = b - a;
        Some(if d.length() > 1e-3 { d.normalized() } else { vec2(1.0, 0.0) })
    };
    let jalons = t.jalons();
    let dernier = jalons.len() - 1;
    for (k, &f) in jalons.iter().enumerate() {
        let (Some(c), Some(d)) = (ecran(t.point_a(f)), direction(f)) else { continue };
        let n = vec2(-d.y, d.x);
        if k == 0 {
            painter.line_segment([c - n * (DEMI_MARQUE + 2.0), c + n * (DEMI_MARQUE + 2.0)], Stroke::new(2.5_f32, COULEUR));
        } else if k == dernier {
            let (g, h) = (c - d * 11.0 + n * 6.0, c - d * 11.0 - n * 6.0);
            painter.add(Shape::line(vec![g, c, h], Stroke::new(3.5_f32, Color32::from_black_alpha(90))));
            painter.add(Shape::line(vec![g, c, h], Stroke::new(2.0_f32, COULEUR)));
        } else {
            let prise = tenue == Some(k - 1);
            let (a, b) = (c - n * DEMI_MARQUE, c + n * DEMI_MARQUE);
            painter.line_segment([a, b], Stroke::new(if prise { 5.0_f32 } else { 3.5_f32 }, Color32::from_black_alpha(90)));
            painter.line_segment([a, b], Stroke::new(2.0_f32, if prise { Color32::WHITE } else { COULEUR }));
        }
        if let Some(img) = images.get(k) {
            let cote = if n.y <= 0.0 { n } else { -n };
            painter.text(c + cote * (DEMI_MARQUE + 4.0), Align2::CENTER_CENTER, format!("{}", img + 1), FontId::proportional(10.0), COULEUR);
        }
    }
    if let Some(c) = courante.and_then(|f| ecran(t.point_a(f))) {
        painter.circle(c, 4.0, COULEUR, Stroke::new(1.5_f32, Color32::from_black_alpha(140)));
    }
}

// ------------------------------------------------------------------------------------------------ les clés d'une carte

/// La pose propre d'un calque qui suit une trajectoire dans l'espace : sa pose fixe et sa profondeur, hors de ses clés
/// (c'est la trajectoire qui pose ses clés, elle ne touche jamais à sa pose fixe).
fn pose_propre(l: &Layer) -> (LayerTransform, f32) {
    (LayerTransform { dz: 0.0, ..l.transform }, Layer::clamp_depth_libre(l.depth + l.transform.dz))
}

/// **Les clés de position qu'une trajectoire dans l'espace pose sur une carte** (`t` : avec son support), dans un plan en
/// Espace 3D de `duree` images. À chaque image que le chemin couvre (clés d'interpolation) ou à chaque jalon (clés d'images,
/// tenues), la carte est sa pose propre déplacée, dans l'espace, du déplacement du chemin depuis son départ : sa place en
/// largeur, en hauteur et en profondeur change, son orientation et sa taille dans l'espace restent (`Monde::card_from_xf_libre`
/// règle son échelle pour cela : elle rapetisse en reculant, et l'ordre par distance la range toute seule).
pub(crate) fn cles_dans_l_espace(world: &Monde, l: &Layer, t: &Trajectoire, duree: usize) -> Vec<PosKey> {
    let (base, profondeur) = pose_propre(l);
    let propre = world.card_xf(&base, profondeur);
    let tenue = t.cles == Cles::Images;
    t.images_et_fractions(duree)
        .into_iter()
        .filter_map(|(image, fraction)| {
            let d = t.deplacement_a(fraction)?;
            let xf = Xf { pos: m::add(propre.pos, d), ..propre };
            let (mut pose, la) = world.card_from_xf_libre(&base, profondeur, &xf);
            pose.dz = la - l.depth;
            Some(PosKey { tenue, ..PosKey::new(image, pose) })
        })
        .collect()
}

impl App {
    // -------------------------------------------------------------------------------------------- l'outil dans la vue

    /// Dans la vue Espace 3D, le pointeur **trace des trajectoires** (l'outil Trajectoire y a été pris) ; sinon il manipule
    /// les cartes, ou il dessine (`e3_dessine`).
    pub(crate) fn e3_trace(&self) -> bool {
        self.e3d.dessin && self.e3_vue_montree() && self.tool == Tool::Trajectoire
    }

    /// L'outil Trajectoire vient d'être pris dans la vue Espace 3D (`e3_outil_pris`) : le pointeur trace. Ce qui est choisi
    /// n'est pas touché : c'est lui qui suivra le chemin.
    pub(crate) fn e3_tracer(&mut self) {
        self.e3d.dessin = true;
        self.e3d.vive = None;
        self.espace3d.drag = None;
        self.traj.espace = None;
        self.status = if self.doc.plan_en_espace(self.cur_panel) { PRISE.into() } else { PLAN_ORDINAIRE.into() };
    }

    /// Ce qui est choisi dans la vue et peut suivre une trajectoire : les cartes choisies, la caméra du plan ; rien de
    /// choisi : la carte du calque courant. Ce repli ne vaut que quand **rien** n'est choisi : la caméra choisie seule
    /// reste seule, aucun calque ne s'y ajoute (partie 152).
    pub(crate) fn e3_traj_sujets(&self) -> Vec<Sujet> {
        let mut out: Vec<Sujet> = Vec::new();
        for &s in &self.espace3d.sel {
            let sujet = if s == CAMERA_ID { Some(Sujet::Camera) } else { self.e3_card_layer(s).map(|li| Sujet::Carte(self.layers()[li].id)) };
            if let Some(sujet) = sujet.filter(|x| !out.contains(x)) {
                out.push(sujet);
            }
        }
        if out.is_empty() && espace3d::is_card(self.layer()) {
            out.push(Sujet::Carte(self.layer().id));
        }
        out
    }

    /// Les sujets que les réglages de l'outil visent : dans la vue Espace 3D, ce qui est choisi ; sur la feuille, le calque
    /// courant s'il suit une trajectoire dans l'espace.
    pub(crate) fn traj_sujets_des_reglages(&self) -> Vec<Sujet> {
        if self.e3_vue_montree() {
            return self.e3_traj_sujets();
        }
        let l = self.layer();
        if l.trajectoire.as_ref().is_some_and(|t| t.support.is_some()) { vec![Sujet::Carte(l.id)] } else { Vec::new() }
    }

    /// Le rang, dans le plan courant, du calque de numéro `id`.
    fn traj_rang(&self, id: u64) -> Option<usize> {
        self.layers().iter().position(|l| l.id == id)
    }

    /// La trajectoire dans l'espace du sujet `s`, s'il en suit une.
    pub(crate) fn e3_traj_de(&self, s: Sujet) -> Option<&Trajectoire> {
        match s {
            Sujet::Camera => self.panel().trajectoire_camera.as_ref(),
            Sujet::Carte(id) => self.layers().iter().find(|l| l.id == id)?.trajectoire.as_ref().filter(|t| t.support.is_some()),
        }
    }

    /// La durée sur laquelle se lit la trajectoire du sujet `s` : celle du plan (pour la caméra, davantage si la fin du
    /// chemin est après la dernière image, `Document::duree_de_la_trajectoire_camera`).
    pub(crate) fn e3_traj_duree(&self, s: Sujet) -> usize {
        match s {
            Sujet::Camera => Document::duree_de_la_trajectoire_camera(self.panel()),
            Sujet::Carte(_) => self.frame_count(),
        }
    }

    /// Donne la trajectoire `t` au sujet `s` : une carte voit ses clés de position reposées, et en clés d'images ses
    /// dessins des jalons (`trajectoire_recalculer`) ; la caméra n'a rien d'autre à faire (sa trajectoire s'ajoute à ses
    /// clés quand on l'évalue).
    pub(crate) fn e3_traj_poser(&mut self, s: Sujet, t: Trajectoire) {
        match s {
            Sujet::Camera => self.panel_mut().trajectoire_camera = Some(t),
            Sujet::Carte(id) => {
                if let Some(li) = self.traj_rang(id) {
                    self.layers_mut()[li].trajectoire = Some(t);
                    self.trajectoire_recalculer(li);
                }
            }
        }
    }

    /// Comme `e3_traj_poser`, **pendant un geste** (une marque glissée, le chemin tiré) : la carte suit à chaque image
    /// d'écran par ses clés de position, mais ses dessins des jalons attendent le lâcher (`e3_traj_lacher`), comme sur la
    /// feuille : rien n'est copié ni rangé dans la timeline tant que le geste dure, et Échap n'a que le chemin et les clés
    /// de position à rendre.
    pub(crate) fn e3_traj_poser_en_direct(&mut self, s: Sujet, t: Trajectoire) {
        match s {
            Sujet::Camera => self.panel_mut().trajectoire_camera = Some(t),
            Sujet::Carte(id) => {
                if let Some(li) = self.traj_rang(id) {
                    let pi = self.cur_panel;
                    self.layers_mut()[li].trajectoire = Some(t);
                    self.trajectoire_cles_dans_l_espace(pi, li);
                }
            }
        }
    }

    /// Le geste est lâché : chaque carte du geste a ses clés et ses dessins des jalons posés pour de bon.
    fn e3_traj_finir(&mut self, avant: &[Avant]) {
        for a in avant {
            if let Sujet::Carte(id) = a.sujet {
                if let Some(li) = self.traj_rang(id) {
                    self.trajectoire_recalculer(li);
                }
            }
        }
    }

    /// Parmi ce qui est choisi, ce qui peut recevoir un chemin maintenant. Une carte verrouillée, accrochée à la caméra, liée
    /// à son calque de trait, ou qui a déjà une trajectoire tracée sur la feuille est laissée, et la barre d'état dit pourquoi.
    pub(crate) fn e3_traj_sujets_libres(&mut self) -> Vec<Sujet> {
        let mut refus: Option<String> = None;
        let sujets: Vec<Sujet> = self
            .e3_traj_sujets()
            .into_iter()
            .filter(|s| {
                let Sujet::Carte(id) = *s else { return true };
                let Some(l) = self.layers().iter().find(|l| l.id == id) else { return false };
                let raison = if l.locked {
                    Some(format!("Le calque « {} » est verrouillé : il ne suit pas le chemin.", l.name))
                } else if l.position_locked {
                    Some(format!("La position du calque « {} » est verrouillée : il ne suit pas le chemin.", l.name))
                } else if l.accroche {
                    Some(format!("Le calque « {} » est accroché à la caméra : décroche-le pour qu'il suive un chemin dans l'espace.", l.name))
                } else if l.keys_follow_line() {
                    Some(format!("Le calque de couleur « {} » suit son calque de trait : choisis le calque de trait.", l.name))
                } else if l.trajectoire.as_ref().is_some_and(|t| t.support.is_none()) {
                    Some(format!("Le calque « {} » a déjà une trajectoire tracée sur la feuille : efface-la d'abord (réglages de l'outil, hors de la vue Espace 3D).", l.name))
                } else {
                    None
                };
                match raison {
                    Some(r) => {
                        refus = Some(r);
                        false
                    }
                    None => true,
                }
            })
            .collect();
        if let Some(r) = refus {
            self.status = r;
        } else if sujets.is_empty() {
            self.status = RIEN_A_DEPLACER.into();
        }
        sujets
    }

    /// La trajectoire de référence d'un geste : celle du premier sujet qui en suit une. C'est elle qu'on voit et qu'on prend ;
    /// le geste s'applique ensuite à la trajectoire de chacun.
    pub(crate) fn e3_traj_reference(&self, sujets: &[Sujet]) -> Option<(Sujet, Trajectoire)> {
        sujets.iter().find_map(|&s| self.e3_traj_de(s).map(|t| (s, t.clone())))
    }

    /// Les sujets qui suivent une trajectoire, tels qu'ils sont (avant un geste).
    fn e3_traj_avant(&self, sujets: &[Sujet]) -> Vec<Avant> {
        sujets
            .iter()
            .filter_map(|&s| {
                let trajectoire = self.e3_traj_de(s)?.clone();
                let cles = match s {
                    Sujet::Carte(id) => self.layers().iter().find(|l| l.id == id).map(|l| l.pos_keys.clone()).unwrap_or_default(),
                    Sujet::Camera => Vec::new(),
                };
                Some(Avant { sujet: s, trajectoire, cles })
            })
            .collect()
    }

    /// **Le plan qui portera le trait** qui commence au point `pos` de l'écran : la carte la plus proche que le regard touche
    /// et qui n'est pas un sujet (si le sol n'est pas devant elle) ; sinon le sol ; sinon (le regard ne descend pas vers le
    /// sol) un plan face à la vue, par la place du premier sujet. Une unité du chemin y vaut `Monde::k` mètres.
    fn e3_traj_support(&self, regard: &Regard3d, pos: Pos2, sujets: &[Sujet]) -> Option<Support> {
        let world = self.e3_world();
        let ray = regard.rayon(pos);
        let frame = self.cur_frame;
        let mut carte: Option<(f32, Xf)> = None;
        if self.espace3d.show_cards {
            for li in self.e3_visible_cards() {
                let layer = &self.layers()[li];
                if sujets.contains(&Sujet::Carte(layer.id)) {
                    continue;
                }
                let xf = world.carte(layer, frame);
                if let Some((t, _)) = world.pick_card(&xf, &ray) {
                    if carte.is_none_or(|(autre, _)| t < autre) {
                        carte = Some((t, xf));
                    }
                }
            }
        }
        // (Une carte est plus grande que le cadre : elle descend sous le sol. Là où le sol est devant elle, c'est lui qu'on
        // voit et qu'on touche : la carte ne l'emporte que si le regard la rencontre d'abord.)
        let sol_avant = m::ray_plane(&ray, [0.0, world.sol_y(), 0.0], [0.0, 1.0, 0.0]);
        if let Some((_, xf)) = carte.filter(|(t, _)| sol_avant.is_none_or(|s| *t <= s + 1e-3)) {
            // (Les axes de la carte : sa droite, et son bas, comme les pixels de son dessin.)
            let mx = xf.matrix();
            return Support::nouveau(xf.pos, m::dir(&mx, [1.0, 0.0, 0.0]), m::dir(&mx, [0.0, -1.0, 0.0]), world.k);
        }
        // Le sol : vers la droite, et vers la caméra d'origine du plan (le bas de l'écran, vu de dessus).
        let sol = Support::nouveau([0.0, world.sol_y(), 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], world.k)?;
        if let Some(p) = sol.couper(ray.o, ray.d) {
            return Some(sol.deplace(sol.deplacement(p)));
        }
        let centre = match sujets.first()? {
            Sujet::Camera => world.eye(&self.current_camera()),
            Sujet::Carte(id) => world.carte(self.layers().iter().find(|l| l.id == *id)?, frame).pos,
        };
        Support::nouveau(centre, regard.view.right, m::scale(regard.view.up, -1.0), world.k)
    }

    // -------------------------------------------------------------------------------------------- le pointeur

    /// Le pointeur dans la vue Espace 3D quand il trace des trajectoires : appui, glisser, lâcher. Alt+appui tourne la vue
    /// (`Drag::Orbit`, suivi par `e3_input`), Ctrl+clic choisit, seule, la carte (ou la caméra) sous le pointeur, et
    /// Maj+Ctrl+clic l'ajoute à ce qui est choisi ou l'en retire (`e3_traj_choisir`).
    pub(crate) fn e3_trajectoire_pointeur(&mut self, ctx: &egui::Context, response: &egui::Response, regard: Regard3d) {
        self.e3d.regard = Some(regard);
        let (events, primary_down, hover) = ctx.input(|i| (i.events.clone(), i.pointer.primary_down(), i.pointer.hover_pos()));
        let modal = self.modal_open();
        let dans_la_vue = |pos: Pos2| regard.rect.contains(pos) && response.interact_rect.contains(pos) && ctx.layer_id_at(pos) == Some(response.layer_id);
        for event in &events {
            match event {
                Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers, .. } => {
                    if modal || !dans_la_vue(*pos) {
                        continue;
                    }
                    // Un appui dans la vue reprend la main au montage, comme dans le dessin.
                    self.panel_focus = false;
                    self.images_focus = false;
                    self.key_focus = false;
                    self.spring_used();
                    if modifiers.alt {
                        self.e3_traj_lacher();
                        self.espace3d.drag = Some(Drag::Orbit);
                    } else if modifiers.command || modifiers.ctrl {
                        // (Maj n'ajoute qu'avec Ctrl : Maj seule, à l'appui, reste au chemin, qu'elle déplace en bloc.)
                        self.e3_traj_choisir(regard, *pos, modifiers.shift);
                    } else {
                        self.e3_traj_presser(regard, *pos, modifiers.shift);
                    }
                }
                Event::PointerMoved(pos) => self.e3_traj_glisser(regard, *pos),
                Event::PointerButton { button: PointerButton::Primary, pressed: false, .. } | Event::PointerGone => self.e3_traj_lacher(),
                _ => {}
            }
        }
        // Relâchement manqué (perte de focus…) : on clôt le geste quand même.
        if !primary_down {
            self.e3_traj_lacher();
        }
        self.e3d.pointeur = hover.filter(|p| regard.rect.contains(*p));
    }

    /// Ctrl+clic : la carte (ou la caméra) sous le pointeur est choisie, **seule** : ce qui était choisi est lâché, et les
    /// gestes de l'outil n'agissent plus que sur son chemin (partie 152 ; Anthony, 2026-11-13 : « Éditer la trajectoire de la
    /// caméra ne fonctionne pas puisqu'un calque est toujours sélectionné en même temps » : avant, Ctrl+clic ajoutait la
    /// caméra à la carte du calque courant). Une carte choisie ainsi devient le calque courant, comme d'un clic avec les
    /// outils de la vue. `ajouter` (Maj tenue avec Ctrl) : elle rejoint ce qui est choisi, ou le quitte : c'est ainsi qu'on
    /// choisit exprès la caméra avec des cartes.
    ///
    /// **La caméra l'emporte quand le pointeur est sur son dessin** (son corps, sa pyramide) : il est tracé par-dessus les
    /// cartes, et une carte passe presque toujours derrière lui (le sol, sous une caméra vue de haut). Avant la partie 150,
    /// la carte l'emportait : Ctrl+clic sur la caméra choisissait le sol.
    fn e3_traj_choisir(&mut self, regard: Regard3d, pos: Pos2, ajouter: bool) {
        let px = [pos.x - regard.rect.left(), pos.y - regard.rect.top()];
        let centre = [regard.rect.width() / 2.0, regard.rect.height() / 2.0];
        let camera = !self.espace3d.through_camera && self.e3_pick_camera(&regard.view, px, centre);
        let touche = if camera { Some(CAMERA_ID) } else { self.e3_pick(&regard.rayon(pos)) };
        let Some(id) = touche else {
            self.status = "Ctrl+clic : ni carte ni caméra sous le pointeur.".into();
            return;
        };
        if id == CAMERA_ID {
            self.e3_choisir_la_camera(ajouter);
        } else if ajouter {
            match self.espace3d.sel.iter().position(|s| *s == id) {
                Some(k) => {
                    self.espace3d.sel.remove(k);
                }
                None => self.espace3d.sel.push(id),
            }
        } else {
            self.espace3d.sel = vec![id];
            self.e3_follow_card(id);
        }
        // (Ce qui est choisi, sans le repli sur la carte du calque courant : la barre d'état dit ce qui l'est vraiment.)
        let cartes = self.espace3d.sel.iter().filter(|&&s| self.e3_card_layer(s).is_some()).count();
        let camera = self.espace3d.sel.contains(&CAMERA_ID);
        self.status = match (cartes, camera) {
            (0, true) => "La caméra est choisie, seule : le chemin ne déplacera qu'elle. Maj+Ctrl+clic sur une carte : l'ajouter.".into(),
            (0, false) => "Plus rien n'est choisi : le chemin déplacera la carte du calque courant.".into(),
            (n, true) => format!("Le chemin déplacera {n} carte{} et la caméra.", pluriel(n)),
            (n, false) => format!("Le chemin déplacera {n} carte{}. Maj+Ctrl+clic : en ajouter une, ou la caméra.", pluriel(n)),
        };
    }

    /// Appui avec l'outil : une marque, le chemin (`maj` : pour le déplacer en bloc ; sinon pour le tirer), ou le début d'un
    /// trait sur la surface que le regard touche.
    fn e3_traj_presser(&mut self, regard: Regard3d, pos: Pos2, maj: bool) {
        self.end_stroke();
        self.playing = false;
        if !self.doc.plan_en_espace(self.cur_panel) {
            self.status = PLAN_ORDINAIRE.into();
            return;
        }
        if self.content_refuses() {
            return;
        }
        let sujets = self.e3_traj_sujets_libres();
        if sujets.is_empty() {
            return;
        }
        if let Some((_, t)) = self.e3_traj_reference(&sujets) {
            let Some(support) = t.support else { return };
            let ecran = |p: [f32; 2]| regard.ecran(support.monde(p));
            // Maj + appui sur un bout du chemin : le trait qui suit le prolonge, sur le plan du chemin (partie 151).
            if let Some(g) = Self::e3_traj_prolonger_depuis(&t, support, &regard, pos).filter(|_| maj) {
                self.traj.espace = Some(g);
                self.status = crate::trajectoireprolonge::PROLONGER.into();
                return;
            }
            let marque = marque_sous(&t, &ecran, pos);
            let pres = CheminVu::de(&t, &ecran).plus_proche(pos).filter(|(_, d)| *d <= TOL_ECRAN);
            if marque.is_some() || pres.is_some() {
                let avant = self.e3_traj_avant(&sujets);
                let snap = self.snapshot();
                let ray = regard.rayon(pos);
                self.traj.espace = match (marque, pres, support.couper(ray.o, ray.d)) {
                    (Some(i), ..) => Some(GesteEspace::Marque { i, avant, snap, bouge: false, arrachee: false }),
                    (None, Some((fraction, _)), Some(depart)) => Some(GesteEspace::Chemin { depart, fraction, bloc: maj, avant, snap, bouge: false }),
                    _ => None,
                };
                return;
            }
        }
        let Some(support) = self.e3_traj_support(&regard, pos, &sujets) else {
            self.status = PAS_DE_SURFACE.into();
            return;
        };
        let ray = regard.rayon(pos);
        let plan: Vec<[f32; 2]> = support.couper(ray.o, ray.d).into_iter().collect();
        self.traj.espace = Some(GesteEspace::Trait { support, ecran: vec![pos], plan });
    }

    /// Le pointeur bouge pendant un geste : le trait s'allonge sur sa surface ; une marque glisse le long du chemin, ou est
    /// arrachée ; le chemin est tiré, ou déplacé. Les cartes suivent à chaque image d'écran.
    fn e3_traj_glisser(&mut self, regard: Regard3d, pos: Pos2) {
        let Some(mut g) = self.traj.espace.take() else { return };
        let ray = regard.rayon(pos);
        match &mut g {
            GesteEspace::Trait { support, ecran, plan } | GesteEspace::Prolonger { support, ecran, plan, .. } => {
                if ecran.last().is_none_or(|l| l.distance(pos) > 0.5) {
                    if let Some(p) = support.couper(ray.o, ray.d) {
                        ecran.push(pos);
                        plan.push(p);
                    }
                }
            }
            GesteEspace::Marque { i, avant, bouge, arrachee, .. } => {
                let pres = avant.first().and_then(|a| {
                    let support = a.trajectoire.support?;
                    CheminVu::de(&a.trajectoire, &|p| regard.ecran(support.monde(p))).plus_proche(pos)
                });
                if let Some((f, d)) = pres {
                    let loin = d > ARRACHE_ECRAN;
                    for a in avant.iter() {
                        let mut t = a.trajectoire.clone();
                        if *i < t.marques.len() {
                            if loin {
                                t.retirer_marque(*i);
                            } else {
                                t.deplacer_marque(*i, f);
                            }
                        }
                        self.e3_traj_poser_en_direct(a.sujet, t);
                    }
                    (*arrachee, *bouge) = (loin, true);
                    self.status = if loin { "Marque arrachée : lâcher la retire.".into() } else { format!("Marque à {} % du chemin.", (f * 100.0).round()) };
                }
            }
            GesteEspace::Chemin { depart, fraction, bloc, avant, bouge, .. } => {
                let ici = avant.first().and_then(|a| a.trajectoire.support).and_then(|s| s.couper(ray.o, ray.d));
                if let Some(p) = ici {
                    let (dx, dy) = (p[0] - depart[0], p[1] - depart[1]);
                    for a in avant.iter() {
                        let mut t = a.trajectoire.clone();
                        if *bloc {
                            t.deplacer(dx, dy);
                        } else {
                            let rayon = t.rayon_de_tirage();
                            t.tirer(*fraction, dx, dy, rayon);
                        }
                        self.e3_traj_poser_en_direct(a.sujet, t);
                    }
                    *bouge = true;
                    self.status = if *bloc { "Chemin déplacé en bloc (Maj).".into() } else { "Chemin tiré : il se tord là où on le prend. Maj à l'appui : le déplacer en bloc.".into() };
                }
            }
        }
        self.renderer.invalidate_params();
        self.traj.espace = Some(g);
    }

    /// Lâcher : le trait devient marque ou chemin ; une marque glissée ou arrachée, un chemin tiré ou déplacé sont gardés, en
    /// une étape d'annulation pour tout ce qui est choisi.
    pub(crate) fn e3_traj_lacher(&mut self) {
        let Some(g) = self.traj.espace.take() else { return };
        match g {
            GesteEspace::Trait { support, ecran, plan } => self.e3_traj_poser_le_trait(support, &ecran, &plan),
            GesteEspace::Prolonger { devant, ecran, plan, .. } => self.e3_traj_prolonger(&ecran, &plan, devant),
            GesteEspace::Marque { avant, snap, bouge, arrachee, .. } => {
                if arrachee || bouge {
                    // (Les dessins des jalons, en clés d'images, se posent maintenant : pas pendant le geste.)
                    self.e3_traj_finir(&avant);
                    self.modified = true;
                    self.commit_named(snap, if arrachee { "Retirer une marque" } else { "Glisser une marque" });
                    if arrachee {
                        self.status = self.e3_traj_resume("Marque retirée");
                    }
                }
            }
            GesteEspace::Chemin { avant, snap, bouge, bloc, .. } => {
                if bouge {
                    self.e3_traj_finir(&avant);
                    self.modified = true;
                    self.commit_named(snap, if bloc { "Déplacer la trajectoire" } else { "Tirer la trajectoire" });
                }
            }
        }
    }

    /// Un trait lâché : une marque s'il est court et croise le chemin tel qu'on le voit ; sinon le chemin de tout ce qui est
    /// choisi (neuf, ou retracé : les marques et le temps de chacun restent) ; sinon rien.
    fn e3_traj_poser_le_trait(&mut self, support: Support, ecran: &[Pos2], plan: &[[f32; 2]]) {
        let longueur: f32 = ecran.windows(2).map(|w| w[0].distance(w[1])).sum();
        if longueur < CLIC_ECRAN {
            return;
        }
        let sujets = self.e3_traj_sujets_libres();
        if sujets.is_empty() {
            return;
        }
        // Une marque ?
        let marque = self.e3_traj_reference(&sujets).zip(self.e3d.regard).and_then(|((_, t), regard)| {
            let s = t.support?;
            CheminVu::de(&t, &|p| regard.ecran(s.monde(p))).marque_de(ecran)
        });
        if let Some(f) = marque {
            let snap = self.snapshot();
            let mut posees = 0;
            for &s in &sujets {
                let Some(mut t) = self.e3_traj_de(s).cloned() else { continue };
                if t.ajouter_marque(f).is_some() {
                    self.e3_traj_poser(s, t);
                    posees += 1;
                }
            }
            if posees > 0 {
                self.modified = true;
                self.commit_named(snap, "Marque de trajectoire");
                self.status = self.e3_traj_resume("Marque posée");
            } else {
                self.status = "Une marque est déjà là (ou c'est un bout du chemin) : glisse-la plutôt.".into();
            }
            return;
        }
        let Some(neuve) = Trajectoire::nouvelle_dans_l_espace(plan, support) else {
            self.status = TROP_COURT.into();
            return;
        };
        let snap = self.snapshot();
        let retrace = sujets.iter().all(|&s| self.e3_traj_de(s).is_some());
        let (image, duree) = (self.cur_frame, self.frame_count());
        for &s in &sujets {
            let mut t = neuve.clone();
            match self.e3_traj_de(s).cloned() {
                // (Retracé : les marques, le temps, la sorte des clés et les dessins des jalons restent, comme sur la feuille.)
                // (Et le lissage du chemin, partie 151.)
                Some(e) => {
                    (t.marques, t.depart, t.fin, t.temps, t.cles, t.source, t.dessins) = (e.marques, e.depart, e.fin, e.temps, e.cles, e.source, e.dessins);
                    t.lisser_a(e.lissage);
                }
                None => {
                    // Le départ est à la tête de lecture, la fin sur la dernière image du plan (posée : elle suit les images
                    // insérées ou retirées). Une carte garde la pose qu'elle a au départ : c'est sa pose propre désormais.
                    t.depart = image;
                    t.fin = (duree >= image + 2).then(|| duree - 1);
                    t.cles = self.traj.cles_propose;
                    // (Le lissage du curseur s'applique au nouveau tracé, partie 151.)
                    t.lisser_a(self.traj.lissage);
                    if let Sujet::Carte(id) = s {
                        if let Some(li) = self.traj_rang(id) {
                            let l = &mut self.layers_mut()[li];
                            let (pose, profondeur) = (l.pose(image), l.depth_libre_at(image));
                            l.pos_keys.clear();
                            l.transform = LayerTransform { dz: 0.0, ..pose };
                            l.depth = profondeur;
                        }
                    }
                }
            }
            self.e3_traj_poser(s, t);
        }
        self.modified = true;
        self.commit_named(snap, if retrace { "Retracer la trajectoire" } else { "Trajectoire" });
        self.status = self.e3_traj_resume(if retrace { "Chemin retracé, marques gardées" } else { "Trajectoire tracée" });
    }

    /// « Trajectoire tracée : 2 cartes et la caméra, 1 marque, de l'image 1 à l'image 24. »
    pub(crate) fn e3_traj_resume(&self, debut: &str) -> String {
        let avec: Vec<Sujet> = self.e3_traj_sujets().into_iter().filter(|&s| self.e3_traj_de(s).is_some()).collect();
        let Some((s, t)) = self.e3_traj_reference(&avec) else { return debut.to_string() };
        let cartes = avec.iter().filter(|s| matches!(s, Sujet::Carte(_))).count();
        let qui = match (cartes, avec.contains(&Sujet::Camera)) {
            (0, _) => "la caméra".to_string(),
            (n, true) => format!("{n} carte{} et la caméra", pluriel(n)),
            (n, false) => format!("{n} carte{}", pluriel(n)),
        };
        let images = t.images_des_jalons(self.e3_traj_duree(s));
        let n = t.marques.len();
        let mut texte = match (images.first(), images.last()) {
            (Some(a), Some(b)) => format!("{debut} : {qui}, {n} marque{}, de l'image {} à l'image {}.", pluriel(n), a + 1, b + 1),
            _ => format!("{debut} : {qui}, {n} marque{} ; le plan s'arrête avant l'image {} du départ, rien ne bouge encore.", pluriel(n), t.depart + 1),
        };
        // En clés d'images, une carte sans dessin au départ n'a rien à copier aux jalons : on le dit, comme sur la feuille.
        let sans_dessin = avec.iter().find_map(|s| match s {
            Sujet::Carte(_) => self.e3_traj_de(*s).filter(|t| t.cles == Cles::Images && t.dessins.is_empty() && t.images_des_jalons(self.frame_count()).len() >= 2),
            Sujet::Camera => None,
        });
        if let Some(t) = sans_dessin {
            texte.push_str(&format!(" Il manque un dessin au départ : dessine d'abord à l'image {}, la trajectoire en fera une copie à chaque marque.", t.depart + 1));
        }
        texte
    }

    /// Échap pendant un geste de l'outil dans la vue : tout revient comme avant le geste ; vrai si la touche a servi.
    pub(crate) fn e3_traj_abandonner(&mut self) -> bool {
        let Some(g) = self.traj.espace.take() else { return false };
        if let GesteEspace::Marque { avant, .. } | GesteEspace::Chemin { avant, .. } = g {
            for a in avant {
                match a.sujet {
                    Sujet::Camera => self.panel_mut().trajectoire_camera = Some(a.trajectoire),
                    Sujet::Carte(id) => {
                        if let Some(li) = self.traj_rang(id) {
                            let l = &mut self.layers_mut()[li];
                            l.trajectoire = Some(a.trajectoire);
                            l.pos_keys = a.cles;
                        }
                    }
                }
            }
            self.renderer.invalidate_params();
        }
        self.status = ABANDON.into();
        true
    }

    // -------------------------------------------------------------------------------------------- les clés des cartes

    /// Ce que la trajectoire dans l'espace du calque `li` du plan `pi` pose sur lui, reposé d'après elle : **ses clés de
    /// position** (`cles_dans_l_espace`) ; et, en **clés d'images**, **un dessin à chaque jalon**
    /// (`trajectoire_poser_les_dessins_dans_l_espace`). En clés d'interpolation, les dessins qu'elle avait posés s'en vont
    /// (leurs images reprennent le dessin du départ), comme sur la feuille. Un chemin qui ne couvre aucune image (plan trop
    /// court) laisse la carte à sa pose propre.
    ///
    /// C'est par ici (`App::trajectoire_recalculer`, `trajectoire_recalculer_dans`) que passent le tracé, les marques, le
    /// recalage sur la timeline, le changement de sorte de clés et la pose à la main d'une carte : poser les clés de
    /// position seules (`cles_dans_l_espace`) ne pose pas les dessins.
    pub(crate) fn trajectoire_recalculer_dans_l_espace(&mut self, pi: usize, li: usize) {
        let Some((cles, des_dessins)) = self.doc.panels.get(pi).and_then(|p| p.layers.get(li)).and_then(|l| l.trajectoire.as_ref()).filter(|t| t.support.is_some()).map(|t| (t.cles, !t.dessins.is_empty())) else { return };
        self.trajectoire_cles_dans_l_espace(pi, li);
        match cles {
            Cles::Images => {
                self.trajectoire_poser_les_dessins_dans_l_espace(pi, li);
            }
            Cles::Interpolation if des_dessins => self.trajectoire_defaire_les_dessins(pi, li),
            Cles::Interpolation => {}
        }
    }

    /// Les clés de position du calque `li` du plan `pi` reposées d'après sa trajectoire dans l'espace
    /// (`cles_dans_l_espace`), sans toucher à ses dessins : ce qu'un geste en cours refait à chaque image d'écran.
    fn trajectoire_cles_dans_l_espace(&mut self, pi: usize, li: usize) {
        let Some(p) = self.doc.panels.get(pi) else { return };
        let duree = p.frame_count();
        let Some(l) = p.layers.get(li) else { return };
        let Some(t) = l.trajectoire.as_ref().filter(|t| t.support.is_some()) else { return };
        let cles = cles_dans_l_espace(&Monde::du_plan(&self.doc, pi), l, t, duree);
        self.doc.panels[pi].layers[li].pos_keys = cles;
    }

    /// **Clés d'images d'une trajectoire dans l'espace** (partie 152 ; Anthony, 2026-11-13 : « Clés d'images ne crée pas de
    /// clés d'images dans la timeline ») : **un dessin à chaque jalon** (le départ, chaque marque, l'arrivée), exposé
    /// jusqu'au jalon suivant (le dernier, jusqu'à la fin du plan), comme sur la feuille
    /// (`trajectoire_poser_les_dessins`). La timeline se scinde aux jalons, et chaque pose se redessine.
    ///
    /// Le départ garde le dessin d'origine (noté dans `source`) ; chaque autre jalon reçoit **une copie de ce dessin, telle
    /// quelle** : ni déplacée, ni agrandie, ni tournée. C'est la différence avec la feuille : ici c'est la carte qui porte
    /// le mouvement, par sa clé de position tenue du jalon (`cles_dans_l_espace`).
    ///
    /// Les dessins posés sont suivis (`Trajectoire::dessins`, un par jalon, **chacun à son jalon**) : un dessin déjà posé est
    /// gardé tel qu'il est, avec ce qu'on y a redessiné ; seul un jalon de plus reçoit une copie neuve, celle du dessin du
    /// jalon d'avant tel qu'on l'a laissé (ce que l'image montrait là) ; une marque retirée emporte le sien. Glisser une
    /// marque, tirer le chemin ou recaler un jalon dans le temps ne refait donc aucun dessin : sa clé change seulement
    /// d'image. Faux, avec la raison dans la barre d'état, s'il n'y a pas de dessin au départ.
    fn trajectoire_poser_les_dessins_dans_l_espace(&mut self, pi: usize, li: usize) -> bool {
        let duree = self.doc.panels[pi].frame_count();
        let Some(t) = self.doc.panels[pi].layers[li].trajectoire.clone() else { return false };
        let images = t.images_des_jalons(duree);
        if images.len() < 2 {
            return false;
        }
        let Some(source) = t.source.filter(|c| self.doc.cels.contains_key(c)).or_else(|| self.doc.panels[pi].layers[li].cel_at(t.depart)) else {
            self.status = format!("Clés d'images : dessine d'abord à l'image {} du départ, la trajectoire en fera une copie à chaque marque.", t.depart + 1);
            return false;
        };
        let mut dessins = Vec::with_capacity(images.len());
        for k in 0..images.len() {
            let ancien = t.dessins.get(k).filter(|a| self.doc.cels.contains_key(&a.dessin)).copied();
            // (Le même dessin que le jalon d'avant : une marque vient d'être posée entre les deux, `Trajectoire::ajouter_marque` ;
            // elle reçoit sa copie à elle de ce dessin, tel qu'on l'a laissé.)
            let neuf = k > 0 && ancien.is_some_and(|a| t.dessins.get(k - 1).is_some_and(|p| p.dessin == a.dessin));
            let dessin = match ancien {
                Some(a) if neuf => self.doc.duplicate_cel(a.dessin),
                Some(a) => a.dessin,
                None if k == 0 => source,
                None => self.doc.duplicate_cel(source),
            };
            dessins.push(DessinJalon { dessin, dx: 0.0, dy: 0.0, angle: 0.0 });
        }
        let l = &mut self.doc.panels[pi].layers[li];
        if l.frames.len() < duree {
            l.frames.resize(duree, None);
        }
        for k in 0..images.len() {
            let fin = if k + 1 < images.len() { images[k + 1] } else { duree };
            for f in images[k]..fin.min(duree) {
                l.frames[f] = Some(dessins[k].dessin);
            }
        }
        if let Some(t) = l.trajectoire.as_mut() {
            t.source = Some(source);
            t.dessins = dessins;
        }
        true
    }

    /// Le calque `li`, qui suit une trajectoire dans l'espace, vient d'être posé à la main à l'image courante (manipulateur
    /// ou champs de la vue Espace 3D, outil Transformer) : c'est **sa pose propre** qui prend le geste (il bouge, tourne ou
    /// grandit à toutes les images), **son chemin se déplace avec lui**, et ses clés sont reposées. Vrai si le calque suit une
    /// trajectoire dans l'espace (faux : rien n'est fait).
    pub(crate) fn trajectoire_espace_apres_pose(&mut self, li: usize) -> bool {
        let (pi, f, duree) = (self.cur_panel, self.cur_frame, self.frame_count());
        let world = self.e3_world();
        let Some(l) = self.layers().get(li) else { return false };
        let Some(t) = l.trajectoire.as_ref().filter(|t| t.support.is_some()) else { return false };
        let d = t.deplacement_a_l_image(f, duree).unwrap_or([0.0; 3]);
        let (pose, profondeur) = (l.pose(f), l.depth_libre_at(f));
        let ici = world.card_xf(&pose, profondeur);
        let propre = Xf { pos: m::sub(ici.pos, d), ..ici };
        let (base_avant, profondeur_avant) = pose_propre(l);
        let glisse = m::sub(propre.pos, world.card_xf(&base_avant, profondeur_avant).pos);
        let (base, la) = world.card_from_xf_libre(&pose, profondeur, &propre);
        let l = &mut self.layers_mut()[li];
        l.transform = LayerTransform { dz: 0.0, ..base };
        l.depth = la;
        if let Some(s) = l.trajectoire.as_mut().and_then(|t| t.support.as_mut()) {
            *s = s.deplace(glisse);
        }
        self.trajectoire_recalculer_dans(pi, li);
        true
    }

    /// « Plan en Espace 3D » vient d'être décoché pour le plan `pi` : une trajectoire dans l'espace n'a plus de sens dans un
    /// plan ordinaire. Celles des calques sont détachées (leurs clés restent, à la main) et celle de la caméra devient ses
    /// clés. À appeler avant de changer la case.
    pub(crate) fn trajectoires_espace_quitter(doc: &mut Document, pi: usize) {
        doc.cuire_trajectoires_camera(&[pi]);
        if let Some(p) = doc.panels.get_mut(pi) {
            for l in &mut p.layers {
                if l.trajectoire.as_ref().is_some_and(|t| t.support.is_some()) {
                    l.trajectoire = None;
                }
            }
        }
    }

    // -------------------------------------------------------------------------------------------- par-dessus la vue

    /// La légende du bas de la vue quand le pointeur trace des trajectoires.
    pub(crate) fn e3_trajectoire_legende(&self) -> &'static str {
        if self.doc.plan_en_espace(self.cur_panel) {
            "Trajectoire · un trait sur une carte ou le sol : le chemin de ce qui est choisi · petit trait en travers : une marque · tirer le chemin : le tordre (Maj : le déplacer) · Maj + trait depuis un bout : le prolonger · glisser une marque (loin du chemin : la retirer) · Ctrl+clic : choisir une carte ou la caméra, seule (Maj+Ctrl+clic : ajouter) · Alt+glisser ou clic droit : tourner la vue · Échap : abandonner"
        } else {
            "Trajectoire dans l'espace : il faut un « Plan en Espace 3D » (panneau Espace 3D) · Alt+glisser ou clic droit : tourner la vue"
        }
    }

    /// Ce que l'outil ajoute à l'image de la vue : le chemin de ce qui est choisi (une fois chaque chemin différent), ses
    /// marques, l'image de chaque jalon, le point de l'image courante, le trait en cours, la légende et le curseur.
    pub(crate) fn e3_trajectoire_peindre(&mut self, ctx: &egui::Context, painter: &egui::Painter, response: &egui::Response, rect: Rect, regard: Regard3d) {
        self.e3d.regard = Some(regard);
        painter.text(rect.left_bottom() + vec2(8.0, -6.0), Align2::LEFT_BOTTOM, self.e3_trajectoire_legende(), FontId::proportional(11.0), Color32::from_gray(190));
        if let Some(GesteEspace::Trait { ecran, .. } | GesteEspace::Prolonger { ecran, .. }) = &self.traj.espace {
            if ecran.len() >= 2 {
                painter.add(Shape::line(ecran.clone(), Stroke::new(1.5_f32, COULEUR.gamma_multiply(0.7))));
            }
        }
        let survol = response.hover_pos().filter(|_| !self.modal_open());
        let sujets = self.e3_traj_sujets();
        let mut vues: Vec<Trajectoire> = Vec::new();
        let (mut sur_marque, mut sur_chemin) = (false, false);
        for &s in &sujets {
            let Some(t) = self.e3_traj_de(s) else { continue };
            let Some(support) = t.support else { continue };
            if vues.iter().any(|v| v.points == t.points && v.support == t.support && v.marques == t.marques && v.depart == t.depart && v.fin == t.fin) {
                continue;
            }
            let ecran = |p: [f32; 2]| regard.ecran(support.monde(p));
            let duree = self.e3_traj_duree(s);
            let tenue = match &self.traj.espace {
                Some(GesteEspace::Marque { i, arrachee: false, .. }) => Some(*i),
                Some(_) => None,
                None => survol.and_then(|p| marque_sous(t, &ecran, p)),
            };
            if self.traj.espace.is_none() {
                sur_marque |= tenue.is_some();
                sur_chemin |= survol.is_some_and(|p| CheminVu::de(t, &ecran).plus_proche(p).is_some_and(|(_, d)| d <= TOL_ECRAN));
            }
            let courante = t.fraction_suivie(self.cur_frame, duree).filter(|_| self.cur_frame >= t.depart);
            peindre_chemin(painter, t, &t.images_des_jalons(duree), courante, &ecran, tenue);
            vues.push(t.clone());
        }
        if survol.is_none() {
            return;
        }
        let (alt, ctrl) = ctx.input(|i| (i.modifiers.alt, i.modifiers.command || i.modifiers.ctrl));
        let icone = match (&self.espace3d.drag, &self.traj.espace) {
            (Some(Drag::Orbit), _) => CursorIcon::Grabbing,
            (_, Some(GesteEspace::Marque { .. } | GesteEspace::Chemin { .. })) => CursorIcon::Grabbing,
            (_, Some(GesteEspace::Trait { .. } | GesteEspace::Prolonger { .. })) => CursorIcon::Crosshair,
            _ if alt => CursorIcon::Grab,
            _ if ctrl => CursorIcon::PointingHand,
            _ if !self.doc.plan_en_espace(self.cur_panel) => CursorIcon::NotAllowed,
            _ if sur_marque => CursorIcon::Grab,
            _ if sur_chemin => CursorIcon::PointingHand,
            _ => CursorIcon::Crosshair,
        };
        ctx.set_cursor_icon(icone);
    }

    /// Sur la feuille d'un plan en Espace 3D (la vue par la caméra du plan), avec l'outil : le chemin dans l'espace du calque
    /// courant, **montré seulement** (il se retouche dans la vue Espace 3D). Vrai si le calque courant suit une trajectoire
    /// dans l'espace (la feuille n'a alors rien d'autre à peindre pour l'outil).
    pub(crate) fn paint_trajectoire_dans_l_espace(&self, painter: &egui::Painter) -> bool {
        let Some(t) = self.layers().get(self.cur_layer).and_then(|l| l.trajectoire.as_ref()) else { return false };
        let Some(support) = t.support else { return false };
        if !self.doc.plan_en_espace(self.cur_panel) {
            return true;
        }
        let world = self.e3_world();
        let cam = self.doc.panel_camera_reelle(self.cur_panel, self.cur_frame);
        // (Le cadre est au centre de la feuille : `Monde::to_frame` rend des pixels du cadre.)
        let coin = vec2((world.stage[0] - world.frame[0] as f32) / 2.0, (world.stage[1] - world.frame[1] as f32) / 2.0);
        let ecran = |p: [f32; 2]| world.to_frame(&cam, support.monde(p)).map(|q| self.stage_to_screen(q + coin));
        let duree = self.frame_count();
        let courante = t.fraction_suivie(self.cur_frame, duree).filter(|_| self.cur_frame >= t.depart);
        peindre_chemin(painter, t, &t.images_des_jalons(duree), courante, &ecran, None);
        true
    }

    // -------------------------------------------------------------------------------------------- réglages

    /// Fait `f` sur la trajectoire de chaque sujet qui en suit une (`f` rend vrai si elle a changé), en **une** étape
    /// d'annulation nommée `nom` ; rend le nombre de trajectoires changées.
    fn e3_traj_pour_tous(&mut self, sujets: &[Sujet], nom: &str, f: impl Fn(&mut Trajectoire, usize) -> bool) -> usize {
        let snap = self.snapshot();
        let mut n = 0;
        for &s in sujets {
            let Some(mut t) = self.e3_traj_de(s).cloned() else { continue };
            if f(&mut t, self.e3_traj_duree(s)) {
                self.e3_traj_poser(s, t);
                n += 1;
            }
        }
        if n > 0 {
            self.modified = true;
            self.commit_named(snap, nom);
        }
        n
    }

    /// Les réglages de l'outil pour les trajectoires dans l'espace, dans le panneau Réglages comme ceux de tout outil
    /// (`trajectoire_options_ui` ; partie 150 : ils ne sont plus en tête des réglages de la vue). Montrés quand la vue
    /// Espace 3D est ouverte, et sur la feuille pour un calque qui suit une trajectoire dans l'espace : ils agissent sur la
    /// trajectoire de **tout ce qui est choisi**, caméra comprise. Vrai s'ils ont été montrés (faux : rien à régler ici,
    /// les réglages de la feuille s'appliquent).
    pub(crate) fn trajectoire_espace_options_ui(&mut self, ui: &mut egui::Ui) -> bool {
        let dans_la_vue = self.e3_vue_montree();
        let sujets = self.traj_sujets_des_reglages();
        if !dans_la_vue && sujets.is_empty() {
            return false;
        }
        ui.label(RichText::new("Trajectoire dans l'espace").strong());
        if !self.doc.plan_en_espace(self.cur_panel) {
            ui.label("Ce plan n'est pas un « Plan en Espace 3D » (case du panneau Espace 3D).");
            return true;
        }
        let avec: Vec<Sujet> = sujets.iter().copied().filter(|&s| self.e3_traj_de(s).is_some()).collect();
        let cartes = sujets.iter().filter(|s| matches!(s, Sujet::Carte(_))).count();
        let qui = match (cartes, sujets.contains(&Sujet::Camera)) {
            (0, true) => "la caméra".to_string(),
            (0, false) => "rien".to_string(),
            (n, true) => format!("{n} carte{} et la caméra", pluriel(n)),
            (n, false) => format!("{n} carte{}", pluriel(n)),
        };
        let Some((premier, t)) = self.e3_traj_reference(&avec) else {
            ui.label(format!("Choisi : {qui}."));
            let mut cles = self.traj.cles_propose;
            if cles_espace_ui(ui, &mut cles) {
                self.traj.cles_propose = cles;
            }
            self.trajectoire_lissage_ui(ui, true);
            return true;
        };
        let duree = self.e3_traj_duree(premier);
        let images = t.images_des_jalons(duree);
        let n = t.marques.len();
        ui.label(match (images.first(), images.last()) {
            (Some(a), Some(b)) => format!("Choisi : {qui}. {n} marque{} · le chemin dure de l'image {} à l'image {}.", pluriel(n), a + 1, b + 1),
            _ => format!("Choisi : {qui}. {n} marque{} · le plan s'arrête avant l'image {} du départ : rien ne bouge encore.", pluriel(n), t.depart + 1),
        });
        if avec.len() < sujets.len() {
            ui.label(RichText::new("Tout ce qui est choisi ne suit pas encore un chemin : un trait long le donne à tous.").small().weak());
        }
        let f = self.cur_frame;
        ui.horizontal_wrapped(|ui| {
            if images.first().is_some_and(|&a| a != f) && ui.button("Partir de la tête de lecture").on_hover_text("Le départ du chemin passe à l'image courante, pour tout ce qui est choisi.").clicked() {
                if self.e3_traj_pour_tous(&avec, "Départ de la trajectoire", |t, duree| t.recaler(0, f, duree).is_some()) == 0 {
                    self.status = "Le plan est trop court pour recaler la trajectoire ici.".into();
                } else {
                    self.status = self.e3_traj_resume("Départ à la tête de lecture");
                }
            }
            if images.last().is_some_and(|&b| b != f) && ui.button("Finir à la tête de lecture").on_hover_text("La fin du chemin passe à l'image courante, pour tout ce qui est choisi.").clicked() {
                if self.e3_traj_pour_tous(&avec, "Fin de la trajectoire", |t, duree| t.recaler(t.marques.len() + 1, f, duree).is_some()) == 0 {
                    self.status = "Le plan est trop court pour recaler la trajectoire ici.".into();
                } else {
                    self.status = self.e3_traj_resume("Fin à la tête de lecture");
                }
            }
            let recales = avec.iter().any(|&s| self.e3_traj_de(s).is_some_and(|t| t.temps.is_some()));
            if recales && ui.button("Temps égaux entre les marques").on_hover_text("Oublie les marques recalées à la main : le temps est de nouveau partagé en parts égales.").clicked() {
                self.e3_traj_pour_tous(&avec, "Temps égaux entre les marques", |t, _| {
                    t.temps_egaux();
                    true
                });
            }
        });
        let mut cles = t.cles;
        if cles_espace_ui(ui, &mut cles) {
            self.e3_traj_sorte(&avec, cles);
        }
        // Le lissage du chemin, pour tout ce qui est choisi (partie 151).
        self.trajectoire_lissage_ui(ui, true);
        ui.horizontal_wrapped(|ui| {
            let marques = avec.iter().any(|&s| self.e3_traj_de(s).is_some_and(|t| !t.marques.is_empty()));
            if marques && ui.button("Retirer les marques").on_hover_text("Le chemin reste, à vitesse constante.").clicked() {
                self.e3_traj_retirer_les_marques(&avec);
            }
            if ui.button("Détacher").on_hover_text("Le chemin s'en va, le mouvement reste : les clés de position des cartes telles quelles, et pour la caméra une clé par image, à retoucher à la main.").clicked() {
                self.e3_traj_retirer(&avec, false);
            }
            if ui.button("Effacer").on_hover_text("Le chemin et le mouvement qu'il donnait s'en vont, pour tout ce qui est choisi : les cartes gardent la pose du départ, la caméra retrouve celle de ses clés.").clicked() {
                self.e3_traj_retirer(&avec, true);
            }
        });
        // (Le mode d'emploi est dans l'animation de l'outil et le registre des explications : `aidecatalogue.rs`, partie 153.)
        true
    }

    /// La sorte des clés (réglages de l'outil) de la trajectoire dans l'espace de chaque sujet, en une étape d'annulation ;
    /// elle est proposée au chemin suivant. « Clés d'images » : une clé de position tenue et, pour une carte, une clé de
    /// dessin à chaque jalon. « Clés d'interpolation » : une clé de position par image, les dessins des jalons s'en vont.
    pub(crate) fn e3_traj_sorte(&mut self, sujets: &[Sujet], cles: Cles) {
        self.traj.cles_propose = cles;
        self.e3_traj_pour_tous(sujets, "Sorte des clés de la trajectoire", |t, _| {
            t.cles = cles;
            true
        });
    }

    /// « Retirer les marques » de la trajectoire dans l'espace de chaque sujet, en une étape d'annulation : le chemin reste, à
    /// vitesse constante ; en clés d'images, le départ et l'arrivée d'une carte gardent leurs dessins
    /// (`Trajectoire::retirer_les_marques`).
    pub(crate) fn e3_traj_retirer_les_marques(&mut self, sujets: &[Sujet]) {
        self.e3_traj_pour_tous(sujets, "Retirer les marques", |t, _| {
            t.retirer_les_marques();
            true
        });
    }

    /// « Détacher » (`effacer` faux : le mouvement reste, en clés) ou « Effacer » (le mouvement s'en va) la trajectoire dans
    /// l'espace de chaque sujet, en une étape d'annulation.
    pub(crate) fn e3_traj_retirer(&mut self, sujets: &[Sujet], effacer: bool) {
        if sujets.is_empty() {
            return;
        }
        let snap = self.snapshot();
        let pi = self.cur_panel;
        for &s in sujets {
            match s {
                Sujet::Camera if effacer => self.panel_mut().trajectoire_camera = None,
                Sujet::Camera => {
                    self.doc.cuire_trajectoires_camera(&[pi]);
                }
                Sujet::Carte(id) => {
                    if let Some(li) = self.traj_rang(id) {
                        // (« Effacer » : les dessins qu'elle a posés aux jalons, en clés d'images, s'en vont avec elle, leurs
                        // images reprennent le dessin du départ. « Détacher » les laisse, comme les clés de position.)
                        if effacer {
                            self.trajectoire_defaire_les_dessins(pi, li);
                        }
                        let l = &mut self.layers_mut()[li];
                        l.trajectoire = None;
                        if effacer {
                            // (La pose propre du calque est sa pose fixe : sans clés, il y revient.)
                            l.pos_keys.clear();
                        }
                    }
                }
            }
        }
        self.modified = true;
        self.commit_named(snap, if effacer { "Effacer la trajectoire" } else { "Détacher la trajectoire" });
        self.status = if effacer { "Trajectoire effacée : ce qui la suivait reste où il était au départ.".into() } else { "Trajectoire détachée : le mouvement qu'elle donnait reste, à ta main.".into() };
    }
}

/// Les deux sortes de clés d'une trajectoire dans l'espace ; vrai si le choix a changé.
fn cles_espace_ui(ui: &mut egui::Ui, cles: &mut Cles) -> bool {
    let avant = *cles;
    ui.horizontal(|ui| {
        ui.radio_value(cles, Cles::Images, Cles::Images.label()).on_hover_text("Une pose tenue à chaque marque : ce qui est choisi saute de marque en marque, sans glisser entre deux. Une carte reçoit aussi une clé de dessin à chaque marque, copie du dessin du départ, exposée jusqu'à la marque suivante : la timeline se scinde, chaque pose se redessine. D'origine.");
        ui.radio_value(cles, Cles::Interpolation, Cles::Interpolation.label()).on_hover_text("Ce qui est choisi glisse le long du chemin, à la vitesse que donnent les marques : une clé de position par image pour une carte.");
    });
    *cles != avant
}
