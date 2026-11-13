//! Les poignées de la trajectoire (`docs/INTERFACE.md`, partie 154). Anthony, 2026-11-13 : « avec l'outil trajectoire
//! ctrl+clic sur la trajectoire permet d'ajuster la trajectoire en ajoutant une poignée ». C'est aussi la suite de sa
//! demande de la veille : « éditer une trajectoire tracée dans un environnement 3D… lui redonner des directions dans tous
//! les axes x, y et z ».
//!
//! **Les gestes**, avec l'outil Trajectoire, sur la feuille comme dans la vue Espace 3D :
//! * **Ctrl+clic sur le chemin** (à `TOL_ECRAN` pixels d'écran, la prise du tirage) pose une poignée là ; **Ctrl+clic sur
//!   une poignée** la retire ; Ctrl+clic ailleurs garde son rôle (le lasso sur la feuille, choisir une carte ou la caméra
//!   dans la vue) ;
//! * **tirer une poignée** tord le chemin en douceur autour d'elle (`Trajectoire::tirer_poignee`, une cloche en cosinus
//!   sur le rayon du tirage de toujours) ; les bouts et les autres poignées restent fixes, ils bornent l'influence ; les
//!   marques gardent leurs fractions ;
//! * **dans l'Espace 3D**, la poignée choisie montre **trois flèches** x (rouge), y (vert), z (bleu), les axes du monde,
//!   dans les teintes du manipulateur des cartes (`espace3dview::AXIS_COLORS`). Tirer une flèche déplace la poignée sur cet
//!   axe, **hors du plan porteur aussi** (`Trajectoire::hauteurs`) ; ce qui suit le chemin, carte ou caméra, monte avec lui.
//!
//! Un geste = une étape d'annulation, pour tout ce qui est choisi (« la sélection agit sur tout ») ; Échap abandonne. Les
//! calques verrouillés sont laissés de côté, la barre d'état le dit. Aucune boîte de dialogue.

use eframe::egui::{self, vec2, Align2, Color32, FontId, Pos2, Shape, Stroke};

use crate::app::App;
use crate::espace3ddessin::Regard3d;
use crate::espace3dgizmo::Gizmo;
use crate::espace3dmath as m;
use crate::espace3dview::{AXIS_COLORS, HOT};
use crate::trajectoire::Trajectoire;
use crate::trajectoire3d::{Avant, CheminVu, GesteEspace, Sujet};
use crate::trajectoireui::{Geste, TOL_ECRAN};

/// Le nom de l'étape d'annulation d'une poignée tirée (sur la feuille, à plat ; dans l'espace, à plat ou par une flèche).
pub(crate) const TIRER: &str = "Tirer une poignée";
/// … d'une poignée posée, retirée.
pub(crate) const POSER: &str = "Poser une poignée";
pub(crate) const RETIRER: &str = "Retirer une poignée";
/// Ce que dit la barre d'état pendant qu'on tire une poignée.
pub(crate) const TIREE: &str = "Poignée tirée : le chemin se tord en douceur autour d'elle ; les bouts et les autres poignées restent.";
// (La barre d'état ne montre que le début d'un long message, `messages::resume` : l'essentiel vient en premier.)
pub(crate) const POSEE_FEUILLE: &str = "Poignée posée : tire-la ; Ctrl+clic sur elle : la retirer. Le chemin se tord en douceur, les bouts et les autres poignées restent.";
pub(crate) const POSEE_ESPACE: &str = "Poignée posée : tire-la, ou ses flèches x, y, z. Ctrl+clic sur elle : la retirer. Une flèche la déplace sur son axe, hors du plan du chemin aussi.";
pub(crate) const RETIREE: &str = "Poignée retirée.";
pub(crate) const DEJA: &str = "Une poignée est déjà là : tire-la, ou Ctrl+clic sur elle pour la retirer.";
pub(crate) const PRISE_ESPACE: &str = "Poignée choisie : tire-la, ou une de ses flèches x, y, z.";
pub(crate) const PAR_LA_TRANCHE: &str = "Vue par la tranche : tire une flèche x, y ou z de la poignée.";

/// Les axes du monde, et leurs noms.
const AXES: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
const NOMS: [&str; 3] = ["x", "y", "z"];
/// Le rayon d'une poignée à l'écran (pixels).
const RAYON: f32 = 5.0;

/// Le numéro de celui à qui est un chemin dans l'espace (pour la poignée choisie, `Etat::poignee`) : le calque de la
/// carte, ou la caméra.
pub(crate) fn proprietaire(s: Sujet) -> u64 {
    match s {
        Sujet::Carte(id) => id,
        Sujet::Camera => crate::espace3dgizmo::CAMERA_ID,
    }
}

/// Une poignée à l'écran : un petit rond, jade quand elle est choisie, à l'encre sinon, cerné de sombre.
pub(crate) fn peindre_poignee(painter: &egui::Painter, c: Pos2, choisie: bool) {
    let fond = if choisie { crate::theme::JADE } else { crate::theme::ATELIER_ENCRE };
    painter.circle(c, RAYON, Color32::from_rgb(fond[0], fond[1], fond[2]), Stroke::new(1.5_f32, Color32::from_black_alpha(170)));
}

/// La poignée de `t` dont le point, à l'écran (`ecran` : un point du chemin et sa hauteur), est à moins de `TOL_ECRAN` de
/// `p` (la plus proche).
pub(crate) fn poignee_sous(t: &Trajectoire, ecran: &dyn Fn([f32; 3]) -> Option<Pos2>, p: Pos2) -> Option<usize> {
    t.poignees
        .iter()
        .enumerate()
        .filter_map(|(i, &f)| Some((i, ecran(t.point3_a(f))?.distance(p))))
        .filter(|(_, d)| *d <= TOL_ECRAN)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Sur une autre copie `t` de la trajectoire (un autre sujet choisi), la poignée qui répond à celle de rang `rang` du chemin
/// de référence, qui en a `n`, à la fraction `fraction` : la même par son rang quand les deux en ont autant (les fractions
/// d'un chemin plus long ou plus court ne restent pas les mêmes après un tirage), sinon la plus proche de la fraction.
fn rang_chez(t: &Trajectoire, rang: usize, n: usize, fraction: f32) -> Option<usize> {
    if t.poignees.len() == n && rang < n {
        Some(rang)
    } else {
        t.poignee_a_la_fraction(fraction)
    }
}

/// La longueur des flèches dans l'espace, pour la poignée en `o` : à peu près la même à l'écran de près et de loin, et
/// dans une vue orthographique (`Gizmo::size_for`).
fn longueur_fleche(view: &m::View, o: [f32; 3]) -> f32 {
    Gizmo::size_for(view, o) * 0.75
}

/// La distance du point `p` au segment `ab` (pixels).
fn au_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let d = b - a;
    let l2 = d.length_sq();
    let w = if l2 > 0.0 { ((p - a).dot(d) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (a + d * w).distance(p)
}

/// La flèche (son axe) de la poignée `choisie` de `t` sous le point `pos` de l'écran, vue par `regard` : la poignée et la
/// flèche, ou rien. Tout près de la poignée, c'est elle qu'on prend, pas une flèche ; une flèche vue de bout (son axe
/// regarde l'œil) ne se prend pas.
pub(crate) fn fleche_sous(choisie: Option<usize>, t: &Trajectoire, regard: &Regard3d, pos: Pos2) -> Option<(usize, usize)> {
    let i = choisie.filter(|&i| i < t.poignees.len())?;
    let o = t.monde_a(t.poignees[i])?;
    let po = regard.ecran(o)?;
    if pos.distance(po) <= RAYON + 3.0 {
        return None;
    }
    let len = longueur_fleche(&regard.view, o);
    (0..3)
        .filter_map(|k| {
            let bout = regard.ecran(m::add(o, m::scale(AXES[k], len)))?;
            if po.distance(bout) < 6.0 {
                return None;
            }
            let pointe = bout + (bout - po).normalized() * 9.0;
            Some((k, au_segment(pos, po, pointe)))
        })
        .filter(|(_, d)| *d <= TOL_ECRAN)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(k, _)| (i, k))
}

impl App {
    // -------------------------------------------------------------------------------------------- sur la feuille

    /// La poignée choisie, si elle est sur le chemin de `proprietaire` (un calque ; dans l'espace, la caméra aussi,
    /// `proprietaire`) : changer de calque ou de choix ne la fait pas passer sur un autre chemin.
    pub(crate) fn poignee_de(&self, proprietaire: u64) -> Option<usize> {
        self.traj.poignee.filter(|(p, _)| *p == proprietaire).map(|(_, i)| i)
    }

    /// Le chemin de référence de la vue Espace 3D : celui du premier sujet choisi qui peut suivre un chemin (les calques
    /// verrouillés laissés), comme à l'appui (`e3_traj_presser`). C'est sur lui que les poignées se voient et se prennent.
    pub(crate) fn e3_traj_reference_libre(&self) -> Option<(Sujet, Trajectoire)> {
        self.e3_traj_reference(&self.e3_traj_sujets_libres_et_refus().0)
    }

    /// La poignée du chemin du calque courant sous le point `stage` de la feuille (à `TOL_ECRAN` pixels d'écran).
    pub(crate) fn trajectoire_poignee_sous(&self, stage: Pos2) -> Option<usize> {
        let t = self.layers().get(self.cur_layer)?.trajectoire.as_ref().filter(|t| t.sur_la_feuille())?;
        t.poignee_pres_de([stage.x, stage.y], TOL_ECRAN / self.trajectoire_echelle(stage))
    }

    /// Sur la feuille, à l'appui : Ctrl+clic sur le chemin du calque courant pose une poignée, sur une poignée la retire ;
    /// sans Ctrl, une poignée prise commence à se tirer. Vrai si l'appui a servi aux poignées (faux : ailleurs, l'appui
    /// fait ce qu'il faisait).
    pub(crate) fn trajectoire_presser_poignee(&mut self, stage: Pos2, ctrl: bool) -> bool {
        let li = self.cur_layer;
        let Some(t) = self.layers().get(li).and_then(|l| l.trajectoire.clone()).filter(|t| t.sur_la_feuille()) else { return false };
        let poignee = self.trajectoire_poignee_sous(stage);
        let p = [stage.x, stage.y];
        let id = self.layers()[li].id;
        if ctrl {
            match poignee {
                Some(i) => {
                    let snap = self.snapshot();
                    if let Some(t) = self.layers_mut()[li].trajectoire.as_mut() {
                        t.retirer_poignee(i);
                    }
                    self.trajectoire_valider(snap, RETIRER);
                    self.traj.poignee = None;
                    self.status = RETIREE.into();
                }
                None => {
                    if !self.trajectoire_sous(stage).1 {
                        return false;
                    }
                    let f = t.plus_proche(p).0;
                    let snap = self.snapshot();
                    let posee = self.layers_mut()[li].trajectoire.as_mut().and_then(|t| t.ajouter_poignee(f));
                    match posee {
                        Some(i) => {
                            self.trajectoire_valider(snap, POSER);
                            self.traj.poignee = Some((id, i));
                            self.status = POSEE_FEUILLE.into();
                        }
                        None => self.status = DEJA.into(),
                    }
                }
            }
            return true;
        }
        let Some(i) = poignee else { return false };
        self.traj.poignee = Some((id, i));
        let cles_avant = self.layers()[li].pos_keys.clone();
        let snap = self.snapshot();
        self.traj.geste = Some(Geste::Poignee { i, depart: p, avant: t, cles_avant, snap, bouge: false });
        self.status = "Poignée prise : tire-la pour tordre le chemin en douceur.".into();
        true
    }

    // -------------------------------------------------------------------------------------------- dans l'Espace 3D

    /// Dans la vue Espace 3D, Ctrl+clic au point `pos` : sur le chemin de référence, une poignée posée pour tout ce qui est
    /// choisi ; sur une de ses poignées, retirée. Vrai si l'appui a servi aux poignées (faux : ni chemin ni poignée sous le
    /// pointeur, Ctrl+clic choisit une carte ou la caméra, comme avant).
    pub(crate) fn e3_traj_poignee_ctrl(&mut self, regard: Regard3d, pos: Pos2) -> bool {
        if !self.doc.plan_en_espace(self.cur_panel) {
            return false;
        }
        let Some((reference, t)) = self.e3_traj_reference_libre() else { return false };
        let Some(support) = t.support else { return false };
        let ecran = |p: [f32; 3]| regard.ecran(support.monde3(p));
        let poignee = poignee_sous(&t, &ecran, pos);
        let pres = CheminVu::de(&t, &ecran).plus_proche(pos).filter(|(_, d)| *d <= TOL_ECRAN);
        if poignee.is_none() && pres.is_none() {
            return false;
        }
        self.end_stroke();
        self.playing = false;
        if self.content_refuses() {
            return true;
        }
        let (sujets, refus) = self.e3_traj_sujets_libres_et_refus();
        if sujets.is_empty() {
            self.status = refus.unwrap_or_else(|| crate::trajectoire3d::RIEN_A_DEPLACER.into());
            return true;
        }
        let snap = self.snapshot();
        let mut n = 0;
        let (nom, fraction) = match poignee {
            Some(i) => (RETIRER, t.poignees[i]),
            None => (POSER, pres.map_or(0.0, |(f, _)| f)),
        };
        for &s in &sujets {
            let Some(mut u) = self.e3_traj_de(s).cloned() else { continue };
            let change = match poignee {
                Some(i) => rang_chez(&u, i, t.poignees.len(), fraction).map(|k| u.retirer_poignee(k)).is_some(),
                None => u.ajouter_poignee(fraction).is_some(),
            };
            if change {
                // (Le chemin ne change pas : ni clé ni dessin à reposer.)
                self.e3_traj_poser_en_direct(s, u);
                n += 1;
            }
        }
        if n == 0 {
            self.status = DEJA.into();
            return true;
        }
        self.modified = true;
        self.commit_named(snap, nom);
        self.traj.poignee = match poignee {
            Some(_) => None,
            None => self.e3_traj_de(reference).and_then(|t| t.poignee_a_la_fraction(fraction)).map(|i| (proprietaire(reference), i)),
        };
        let dit = if poignee.is_some() { RETIREE } else { POSEE_ESPACE };
        // (Un calque laissé de côté se dit en tête : la barre d'état ne montre que le début d'un long message.)
        self.status = match refus {
            Some(r) => format!("{} un calque laissé de côté. {r} {dit}", if poignee.is_some() { "Poignée retirée," } else { "Poignée posée," }),
            None => dit.into(),
        };
        true
    }

    /// Dans la vue Espace 3D, à l'appui sans Maj (`sujets` : les sujets libres) : une flèche de la poignée choisie du chemin
    /// de référence, ou une de ses poignées, commence à se tirer. Vrai si l'appui a servi aux poignées.
    pub(crate) fn e3_traj_presser_poignee(&mut self, sujets: &[Sujet], regard: Regard3d, pos: Pos2) -> bool {
        let Some((reference, t)) = self.e3_traj_reference(sujets) else { return false };
        let Some(support) = t.support else { return false };
        let qui = proprietaire(reference);
        if let Some((rang, axe)) = fleche_sous(self.poignee_de(qui), &t, &regard, pos) {
            let fraction = t.poignees[rang];
            let Some(origine) = t.monde_a(fraction) else { return false };
            let (avant, snap) = (self.e3_traj_avant(sujets), self.snapshot());
            self.traj.espace = Some(GesteEspace::Fleche { rang, fraction, axe, depart: regard.rayon(pos), origine, avant, snap, bouge: false });
            self.status = format!("Flèche {} de la poignée : tire-la pour déplacer la poignée sur cet axe.", NOMS[axe]);
            return true;
        }
        let ecran = |p: [f32; 3]| regard.ecran(support.monde3(p));
        let Some(rang) = poignee_sous(&t, &ecran, pos) else { return false };
        self.traj.poignee = Some((qui, rang));
        let fraction = t.poignees[rang];
        let porteur = support.a_la_hauteur(t.hauteur_a(fraction));
        let ray = regard.rayon(pos);
        match porteur.couper(ray.o, ray.d) {
            Some(depart) => {
                let (avant, snap) = (self.e3_traj_avant(sujets), self.snapshot());
                self.traj.espace = Some(GesteEspace::Poignee { rang, fraction, porteur, depart, avant, snap, bouge: false });
                self.status = PRISE_ESPACE.into();
            }
            // (Une vue qui regarde le plan du chemin par la tranche, une vue de face d'un chemin au sol : seules les flèches.)
            None => self.status = PAR_LA_TRANCHE.into(),
        }
        true
    }

    /// Le pointeur bouge pendant qu'une poignée (ou une de ses flèches) est tirée : le regard `ray` passe sous lui.
    pub(crate) fn e3_traj_glisser_poignee(&mut self, g: &mut GesteEspace, ray: &m::Ray) {
        match g {
            GesteEspace::Poignee { rang, fraction, porteur, depart, avant, bouge, .. } => {
                let Some(p) = porteur.couper(ray.o, ray.d) else { return };
                let monde = porteur.deplacement([p[0] - depart[0], p[1] - depart[1]]);
                self.e3_traj_tirer_les_poignees(avant, *rang, *fraction, monde);
                *bouge = true;
                self.status = TIREE.into();
            }
            GesteEspace::Fleche { rang, fraction, axe, depart, origine, avant, bouge, .. } => {
                let monde = m::drag_along_axis(depart, ray, *origine, AXES[*axe]);
                self.e3_traj_tirer_les_poignees(avant, *rang, *fraction, monde);
                *bouge = true;
                let v = format!("{:+.2}", monde[*axe]).replace('.', ",");
                self.status = format!("Poignée sur l'axe {} : {v} m. Le chemin suit en douceur.", NOMS[*axe]);
            }
            _ => {}
        }
    }

    /// Chaque sujet du geste (`avant` : tels qu'ils étaient à l'appui, la référence en premier) a sa poignée qui répond à
    /// celle de rang `rang` (à la fraction `fraction`) du chemin de référence tirée du déplacement `monde` (mètres), lu dans
    /// son propre plan : dans le plan et en hauteur. Ses clés de position suivent ; ses dessins des jalons attendent le lâcher.
    fn e3_traj_tirer_les_poignees(&mut self, avant: &[Avant], rang: usize, fraction: f32, monde: [f32; 3]) {
        let n = avant.first().map_or(0, |a| a.trajectoire.poignees.len());
        for a in avant {
            let mut t = a.trajectoire.clone();
            let Some(s) = t.support else { continue };
            if let Some(k) = rang_chez(&t, rang, n, fraction) {
                let rayon = t.rayon_de_tirage();
                t.tirer_poignee(k, s.decomposer(monde), rayon);
            }
            self.e3_traj_poser_en_direct(a.sujet, t);
        }
    }

    /// Par-dessus une vue (`regard`) : les flèches x, y, z de la poignée choisie du chemin de référence, la flèche tenue ou
    /// survolée éclairée. Vrai si le pointeur (`survol`) est sur une poignée ou une flèche.
    pub(crate) fn e3_traj_peindre_fleches(&self, painter: &egui::Painter, regard: &Regard3d, survol: Option<Pos2>) -> bool {
        let Some((reference, t)) = self.e3_traj_reference_libre() else { return false };
        let Some(support) = t.support else { return false };
        let ecran = |p: [f32; 3]| regard.ecran(support.monde3(p));
        let choisie = self.poignee_de(proprietaire(reference));
        let fleche = survol.and_then(|p| fleche_sous(choisie, &t, regard, p));
        let sur = survol.is_some_and(|p| poignee_sous(&t, &ecran, p).is_some()) || fleche.is_some();
        let Some(i) = choisie.filter(|&i| i < t.poignees.len()) else { return sur };
        let Some(o) = t.monde_a(t.poignees[i]) else { return sur };
        let Some(po) = regard.ecran(o) else { return sur };
        let tenue = match &self.traj.espace {
            Some(GesteEspace::Fleche { axe, .. }) => Some(*axe),
            Some(_) => None,
            None => fleche.map(|(_, k)| k),
        };
        let len = longueur_fleche(&regard.view, o);
        for k in 0..3 {
            let Some(bout) = regard.ecran(m::add(o, m::scale(AXES[k], len))) else { continue };
            if po.distance(bout) < 6.0 {
                continue;
            }
            let chaude = tenue == Some(k);
            let c = if chaude { HOT } else { AXIS_COLORS[k] };
            let d = (bout - po).normalized();
            let n = vec2(-d.y, d.x);
            painter.line_segment([po, bout], Stroke::new(if chaude { 3.0_f32 } else { 2.0 }, c));
            painter.add(Shape::convex_polygon(vec![bout + d * 9.0, bout - d * 3.0 + n * 5.0, bout - d * 3.0 - n * 5.0], c, Stroke::NONE));
            painter.text(bout + d * 17.0, Align2::CENTER_CENTER, NOMS[k], FontId::proportional(11.0), c);
        }
        // (La poignée par-dessus ses flèches.)
        peindre_poignee(painter, po, true);
        sur
    }
}
