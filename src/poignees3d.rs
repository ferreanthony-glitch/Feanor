//! **Les poignées dans la vue** (`docs/INTERFACE.md`, partie 184) : dans la vue Espace 3D, les objets du décor 3D de la séquence
//! (`decor3d.rs`, parties 180 et 181) se choisissent au clic et se manient avec le même manipulateur que les cartes et la caméra
//! (déplacer, tourner, mettre à l'échelle, autour de leur origine) ; et « Modifier l'origine » déplace l'origine d'un calque
//! (partie 182) ou d'un objet du décor sans le bouger.
//!
//! - **Le choix** : un rayon de la souris contre les triangles de chaque objet montré (dans le repère de son modèle, où la
//!   distance le long du rayon reste celle du monde), après sa boîte ; un objet de plus de `TRIANGLES_CHOIX` triangles se prend
//!   par sa boîte. Le plus proche de l'œil, carte (dont le dessin est touché) ou objet, l'emporte (`App::e3_pick_tout`) ; une
//!   carte « Toujours devant le décor 3D » (partie 183) passe devant l'objet, comme à l'écran.
//! - **Ce qui est choisi** : un objet a son numéro de sélection (`objet_selection`) dans la même liste que les cartes et la
//!   caméra (`Espace3dState::sel`) ; cartes, caméra et objets se mêlent (Maj). L'objet choisi dans la vue l'est dans la liste
//!   du panneau, et l'inverse.
//! - **Le geste** : un objet est vu par le manipulateur comme une carte (`xf_de_l_objet` : la place de son origine, sa rotation
//!   autour de la verticale, son échelle) ; son nouveau placement passe par le chemin de la retouche du panneau
//!   (`decor3d_appliquer` : l'aimant des objets, puis les cartes aimantées qui reposent dessus), et le geste entier est **une
//!   étape d'annulation** (l'instantané du projet pris au début du geste, comme pour les cartes). Un objet ne tourne qu'autour
//!   de la verticale et grandit d'un bloc (`Place3d`) : les anneaux X et Z sont gris, l'échelle est uniforme. Un objet aimanté
//!   garde la hauteur que l'aimant lui donne : la flèche Y est grise (comme le champ du panneau), et un glisser dans un plan
//!   vertical le laisse reposé par l'aimant sur ce qui est sous lui.
//! - **L'origine** (`Espace3dState::origine`) : le manipulateur, posé sur l'origine de ce qui est choisi seul, la déplace seule.
//!   Une carte : dans le plan de son dessin (flèches X et Y, carré bleu), par `origine::deplacer_origine` (le dessin ne bouge
//!   pas, à aucune clé). Un objet : dans l'espace, par `Objet3d::changer_origine` (comme les champs du panneau). Maj l'accroche
//!   aux points de la boîte (`origine::points_d_aimantation` pour un calque, `points_de_la_boite` pour un objet). Au lâcher,
//!   l'aimant repose ce qui est aimanté par sa nouvelle origine ; une étape d'annulation.

use eframe::egui::{pos2, Rect};

use crate::app::App;
use crate::decor3d::Objet3d;
use crate::document::Layer;
use crate::espace3d::{self, Xf};
use crate::espace3dgizmo::{self as gz, Gizmo, Handle, Mode as GizmoMode, CAMERA_ID};
use crate::espace3dmath::{self as m, Ray, View, V3};
use crate::espace3dui::Drag;

/// La marque des numéros de sélection des objets du décor (celle des cartes est `espace3d::CARD_TAG`).
pub const OBJET_TAG: u64 = 1 << 61;

/// Le numéro de sélection de l'objet du décor `id` (`Objet3d::id`).
pub fn objet_selection(id: u64) -> u64 {
    OBJET_TAG | id
}

/// Le numéro de l'objet du décor d'un numéro de sélection, si c'en est un.
pub fn objet_of(selection: u64) -> Option<u64> {
    (selection != CAMERA_ID && selection & espace3d::CARD_TAG == 0 && selection & OBJET_TAG != 0).then_some(selection & !OBJET_TAG)
}

/// La commande « Modifier l'origine » (palette, barre d'outils de la vue), sans touche d'office.
pub(crate) const ORIGINE_ACTION: &str = "espace3d.modifier_origine";
/// Son nom, partout le même.
pub(crate) const ORIGINE_NOM: &str = "Modifier l'origine";
/// Sa phrase (bulles).
pub(crate) const ORIGINE_PHRASE: &str = "Le manipulateur déplace l'origine du calque ou de l'objet du décor 3D choisi, sans le bouger : le dessin ou l'objet reste où il est. Maj : l'origine s'accroche au centre, aux coins et aux milieux de la boîte du dessin (un calque) ou de l'objet (un objet du décor). Rappuie, ou prends un outil de la vue, pour revenir aux poignées.";
/// La légende de la vue pendant ce temps.
pub(crate) const LEGENDE_ORIGINE: &str = "Modifier l'origine · flèches et carrés : déplacer l'origine seule, l'objet ne bouge pas (Maj : l'accrocher aux points de sa boîte) · clic : choisir · Alt+glisser ou clic droit : tourner la vue";
/// La barre d'état au survol d'une poignée de l'origine.
pub(crate) const SURVOL_ORIGINE: &str = "Tirer : déplacer l'origine seule (le dessin ou l'objet ne bouge pas). Maj : l'accrocher au centre, aux coins ou aux milieux de sa boîte.";
/// Un objet aimanté qu'on voudrait monter ou descendre.
pub(crate) const HAUTEUR_TENUE: &str = "Aimanté au sol : la hauteur de l'objet est tenue par l'aimant, il glisse sur le sol. Décoche « Aimanté au sol » dans le panneau Espace 3D pour la régler.";
/// Un objet du décor qu'on tournerait hors de la verticale.
pub(crate) const VERTICALE_SEULE: &str = "Un objet du décor 3D ne tourne qu'autour de la verticale : prends l'anneau vert (Y).";
/// L'origine d'un calque qu'on sortirait de son dessin.
pub(crate) const HORS_DU_DESSIN: &str = "L'origine d'un calque est un point de son dessin : elle reste dans le plan de la carte (flèches rouge et verte, carré bleu).";

/// Au-delà, un objet se choisit par sa boîte (le rayon contre chaque triangle coûterait trop à l'appui).
pub const TRIANGLES_CHOIX: usize = 2_000_000;

/// L'objet `o` vu par le manipulateur, comme une carte : la place de son origine, sa rotation (autour de la verticale) et son
/// échelle (la même sur les trois axes). `Xf::matrix` est alors `Place3d::matrice`.
pub fn xf_de_l_objet(o: &Objet3d) -> Xf {
    Xf { pos: o.place.position, rot: [0.0, o.place.rotation_y, 0.0], size: [o.place.echelle_sure(); 3] }
}

/// Où le rayon (`o` + t·`d`) entre dans la boîte `lo`–`hi` (t ≥ 0) ; rien s'il la manque.
fn rayon_boite(o: V3, d: V3, lo: V3, hi: V3) -> Option<f32> {
    // (Une boîte plate, un carré posé à plat, a une épaisseur nulle : un dixième de millimètre de jeu.)
    const JEU: f32 = 1e-4;
    let (mut t0, mut t1) = (f32::NEG_INFINITY, f32::INFINITY);
    for k in 0..3 {
        let (a, b) = (lo[k] - JEU, hi[k] + JEU);
        if d[k].abs() < 1e-12 {
            if o[k] < a || o[k] > b {
                return None;
            }
            continue;
        }
        let (u, v) = ((a - o[k]) / d[k], (b - o[k]) / d[k]);
        t0 = t0.max(u.min(v));
        t1 = t1.min(u.max(v));
    }
    (t1 >= t0.max(0.0) && t1.is_finite()).then_some(t0.max(0.0))
}

/// Où le rayon (`o` + t·`d`) traverse le triangle `a`, `b`, `c`, des deux côtés (t > 0).
fn rayon_triangle(o: V3, d: V3, a: V3, b: V3, c: V3) -> Option<f32> {
    let (e1, e2) = (m::sub(b, a), m::sub(c, a));
    let p = m::cross(d, e2);
    let det = m::dot(e1, p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = m::sub(o, a);
    let u = m::dot(s, p) * inv;
    if !(-1e-6..=1.0 + 1e-6).contains(&u) {
        return None;
    }
    let q = m::cross(s, e1);
    let v = m::dot(d, q) * inv;
    if v < -1e-6 || u + v > 1.0 + 1e-6 {
        return None;
    }
    let t = m::dot(e2, q) * inv;
    (t > 1e-6 && t.is_finite()).then_some(t)
}

/// **Le rayon `ray` touche l'objet `obj`** : la distance (mètres, le long du rayon) du point touché le plus proche de l'œil, sur
/// ses triangles (sa boîte seule au-delà de `TRIANGLES_CHOIX`). Le rayon passe dans le repère du modèle sans être renormalisé :
/// la distance y reste celle du monde.
pub fn toucher_objet(obj: &Objet3d, ray: &Ray) -> Option<f32> {
    let inv = m::inverse_affine(&obj.matrice());
    let (o, d) = ray.to_local(&inv);
    if !o.iter().chain(d.iter()).all(|v| v.is_finite()) {
        return None;
    }
    let (lo, hi) = obj.modele.bornes;
    let entree = rayon_boite(o, d, lo, hi)?;
    if obj.modele.scene.triangles() > TRIANGLES_CHOIX {
        return Some(entree);
    }
    let mut best: Option<f32> = None;
    for prim in &obj.modele.scene.primitives {
        for t in prim.indices.chunks_exact(3) {
            let (Some(a), Some(b), Some(c)) = (prim.positions.get(t[0] as usize), prim.positions.get(t[1] as usize), prim.positions.get(t[2] as usize)) else { continue };
            if let Some(t) = rayon_triangle(o, d, *a, *b, *c) {
                if best.is_none_or(|b| t < b) {
                    best = Some(t);
                }
            }
        }
    }
    best
}

/// Les points de la boîte `lo`–`hi` (repère du modèle) où l'origine d'un objet tirée avec Maj s'accroche : le centre, le
/// centre de chacune des six faces (le centre du dessous d'abord, l'origine d'un objet neuf), les huit coins.
pub fn points_de_la_boite((lo, hi): (V3, V3)) -> Vec<V3> {
    let c = [0, 1, 2].map(|k| (lo[k] + hi[k]) / 2.0);
    let mut out = vec![c, crate::decor3d::origine_au_sol((lo, hi))];
    for k in 0..3 {
        for v in [lo[k], hi[k]] {
            let mut p = c;
            p[k] = v;
            if !out.iter().any(|q| *q == p) {
                out.push(p);
            }
        }
    }
    for i in 0..8 {
        out.push([0, 1, 2].map(|k| if i >> k & 1 == 0 { lo[k] } else { hi[k] }));
    }
    out
}

/// Le point de `points` le plus proche de `p` (`p` lui-même s'il n'y en a pas).
pub fn plus_proche(points: &[V3], p: V3) -> V3 {
    points.iter().copied().min_by(|a, b| m::length(m::sub(*a, p)).total_cmp(&m::length(m::sub(*b, p)))).unwrap_or(p)
}

/// Après `espace3dgizmo::rotate_xfs` autour de `axis` : l'angle d'un objet du décor est le sien plus celui du geste (il ne
/// tourne qu'autour de la verticale ; repasser par les angles d'Euler le ramènerait entre −180° et 180°).
pub fn tourner_les_objets(avant: &[(u64, Xf)], apres: &mut [(u64, Xf)], axis: V3, deg: f32) {
    if axis[1].abs() < 0.99 {
        return;
    }
    for ((id, a), (_, b)) in avant.iter().zip(apres.iter_mut()) {
        if objet_of(*id).is_some() {
            b.rot = [0.0, a.rot[1] + deg * axis[1].signum(), 0.0];
        }
    }
}

/// À qui est l'origine qu'on déplace dans la vue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cible {
    /// La carte du calque de ce rang dans le plan courant.
    Carte(usize),
    /// L'objet du décor de ce numéro (`Objet3d::id`).
    Objet(u64),
}

/// Ce que le geste « Modifier l'origine » garde de son début : l'origine est reposée depuis lui à chaque pas (rien ne
/// s'accumule).
#[derive(Clone)]
pub(crate) enum GesteOrigine {
    Carte {
        li: usize,
        avant: Layer,
        /// Le placement de la carte au début du geste (elle ne bouge pas).
        xf: Xf,
        /// La boîte de ce qui est dessiné à l'image courante (coordonnées du dessin), pour Maj.
        boite: Option<Rect>,
    },
    Objet {
        seq: u64,
        avant: Objet3d,
    },
}

impl App {
    /// L'objet `id` du décor de la séquence du plan courant.
    pub(crate) fn e3_objet(&self, id: u64) -> Option<&Objet3d> {
        self.doc.decor_du_plan(self.cur_panel)?.objet(id)
    }

    /// Les objets du décor choisis dans la vue (ceux qui existent encore : une annulation a pu en retirer).
    pub(crate) fn e3_objets_choisis(&self) -> Vec<u64> {
        self.espace3d.sel.iter().filter_map(|&s| objet_of(s)).filter(|&id| self.e3_objet(id).is_some()).collect()
    }

    /// L'objet du décor montré le plus proche que touche le rayon : sa distance et son numéro.
    pub(crate) fn e3_pick_objet(&self, ray: &Ray) -> Option<(f32, u64)> {
        let d = self.doc.decor_du_plan(self.cur_panel)?;
        // (Partie 198 : un objet verrouillé dans « Scène 3D » ne se prend pas.)
        d.vus().filter(|o| !o.verrou).filter_map(|o| toucher_objet(o, ray).map(|t| (t, o.id))).min_by(|a, b| a.0.total_cmp(&b.0))
    }

    /// **Ce que touche le rayon** : une carte ou un objet du décor (son numéro de sélection). Le plus proche de l'œil l'emporte,
    /// entre la carte dont le dessin est touché et l'objet ; une carte « Toujours devant le décor 3D » passe devant l'objet,
    /// comme à l'écran. Sans dessin ni objet touché, la feuille la plus proche (pour attraper un calque vide), comme
    /// `e3_pick`.
    pub(crate) fn e3_pick_tout(&mut self, ray: &Ray) -> Option<u64> {
        // (Partie 200 : la marque d'une lampe est peinte par-dessus tout : elle l'emporte.)
        if let Some((_, id)) = self.e3_pick_lampe(ray) {
            return Some(crate::miseenscene3dui::lampe_selection(id));
        }
        let (dessin, feuille) = self.e3_pick_cartes(ray);
        let objet = self.e3_pick_objet(ray).map(|(t, id)| (t, objet_selection(id)));
        match (dessin, objet) {
            (Some((tc, c, partage)), Some((to, o))) => Some(if tc <= to || !partage { c } else { o }),
            (Some((_, c, _)), None) => Some(c),
            (None, Some((_, o))) => Some(o),
            (None, None) => feuille.map(|f| f.1),
        }
    }

    /// Choisit l'objet du décor `id` dans la vue (`ajouter` : Maj, il rejoint ce qui est choisi, ou le quitte s'il y était),
    /// et dans la liste du panneau Espace 3D.
    pub(crate) fn e3_choisir_objet(&mut self, id: u64, ajouter: bool) {
        let s = objet_selection(id);
        // (Ce choix est le dernier mot : un changement de calque que la vue n'a pas encore vu ne le remplace pas, comme pour la
        // caméra, `e3_choisir_la_camera`.)
        self.espace3d.last_layer = self.layer().id;
        if !ajouter {
            self.espace3d.sel = vec![s];
        } else if let Some(k) = self.espace3d.sel.iter().position(|x| *x == s) {
            self.espace3d.sel.remove(k);
        } else {
            self.espace3d.sel.push(s);
        }
        if self.espace3d.sel.contains(&s) {
            self.decor3d.choisi = Some(id);
            if let Some(o) = self.e3_objet(id) {
                let aimant = if o.aimante { " Aimanté au sol : il glisse sur le sol, l'aimant tient sa hauteur." } else { "" };
                self.status = format!("Objet 3D « {} » choisi : les poignées le déplacent, le tournent (autour de la verticale) et le mettent à l'échelle autour de son origine.{aimant}", o.nom);
            }
        }
    }

    /// Pose les placements des objets du décor de `new` (les autres numéros sont laissés à `e3_set_xfs`), par le chemin de la
    /// retouche (`decor3d_appliquer` : l'aimant des objets, les cartes aimantées qui reposent dessus). Sans étape d'annulation :
    /// le geste a la sienne.
    pub(crate) fn e3_poser_objets(&mut self, new: &[(u64, Xf)]) {
        let objets: Vec<(u64, Xf)> = new.iter().filter_map(|(id, xf)| objet_of(*id).map(|o| (o, *xf))).collect();
        if objets.is_empty() {
            return;
        }
        let seq = self.doc.env_cle(self.cur_panel);
        let Some(mut d) = self.doc.decor_du_plan(self.cur_panel).cloned() else { return };
        let mut bouge = false;
        for (id, xf) in objets {
            let Some(o) = d.objet_mut(id) else { continue };
            if !xf.pos.iter().all(|v| v.is_finite()) {
                continue;
            }
            o.place.position = xf.pos;
            if xf.rot[1].is_finite() {
                o.place.rotation_y = xf.rot[1];
            }
            let e = xf.size[0].abs();
            if e.is_finite() && e > 0.0 {
                o.place.echelle = e.clamp(0.001, 1000.0);
            }
            bouge = true;
        }
        if bouge {
            self.decor3d_appliquer(seq, d);
        }
    }

    /// **Pourquoi cette poignée ne peut rien ici** (rien : elle peut) : la hauteur d'un objet du décor aimanté au sol (la flèche
    /// Y, comme le champ grisé du panneau), un objet du décor qu'on tournerait hors de la verticale (les anneaux X et Z), et,
    /// pendant « Modifier l'origine », l'origine d'un calque qu'on sortirait de son dessin ou qu'on ne peut pas déplacer. Les
    /// poignées refusées sont grises ; un appui dessus le dit dans la barre d'état.
    pub(crate) fn e3_poignee_refusee(&self, mode: GizmoMode, handle: Handle) -> Option<String> {
        if self.espace3d.origine {
            return match self.e3_cible_origine()? {
                Cible::Carte(_) if matches!(handle, Handle::Axis(2) | Handle::Plane(0) | Handle::Plane(1)) => Some(HORS_DU_DESSIN.into()),
                Cible::Carte(li) => self.e3_origine_carte_refusee(li),
                Cible::Objet(_) => None,
            };
        }
        // (Partie 200 : une lampe se déplace seulement.)
        if !self.espace3d.sel.is_empty() && self.espace3d.sel.iter().all(|&s| crate::miseenscene3dui::lampe_of(s).is_some()) && mode != GizmoMode::Move {
            return Some("Une lampe se déplace seulement : prends l'outil Déplacer.".into());
        }
        let objets = self.e3_objets_choisis();
        if objets.is_empty() {
            return None;
        }
        match (mode, handle) {
            (GizmoMode::Move, Handle::Axis(1)) => objets.iter().filter_map(|&id| self.e3_objet(id)).find(|o| o.aimante).map(|o| format!("« {} » : {}", o.nom, HAUTEUR_TENUE)),
            (GizmoMode::Rotate, Handle::Ring(0) | Handle::Ring(2)) => Some(VERTICALE_SEULE.into()),
            _ => None,
        }
    }

    /// Pourquoi l'origine du calque `li` ne se déplace pas (les règles de `poser_origine`).
    fn e3_origine_carte_refusee(&self, li: usize) -> Option<String> {
        let l = self.layers().get(li)?;
        if l.keys_follow_line() {
            return Some("Un calque de couleur a l'origine de son calque de trait : place celle du trait, la couleur suit.".into());
        }
        if l.locked {
            return Some(format!("Le calque « {} » est verrouillé.", l.name));
        }
        if l.position_locked {
            return Some(crate::app::POSITION_LOCKED.into());
        }
        None
    }

    /// Le pointeur vient d'arriver sur une poignée : la barre d'état dit pourquoi elle ne peut rien, ou, pendant « Modifier
    /// l'origine », ce qu'elle fait.
    pub(crate) fn e3_survol_poignee(&mut self) {
        let Some(h) = self.espace3d.hover else { return };
        let mode = if self.espace3d.origine { GizmoMode::Move } else { self.espace3d.mode };
        if let Some(raison) = self.e3_poignee_refusee(mode, h) {
            self.status = raison;
        } else if self.espace3d.origine {
            self.status = SURVOL_ORIGINE.into();
        }
    }

    /// « Modifier l'origine » (`espace3d.modifier_origine`) : allumé, le manipulateur déplace l'origine de ce qui est choisi
    /// seul ; éteint, il le déplace, le tourne et le met à l'échelle de nouveau.
    pub(crate) fn e3_basculer_origine(&mut self) {
        self.montrer_espace3d();
        self.espace3d.origine = !self.espace3d.origine;
        self.espace3d.drag = None;
        // (Un outil de la vue : le pointeur manipule, il ne dessine plus.)
        self.e3_manipuler();
        self.status = if !self.espace3d.origine {
            "Modifier l'origine : éteint. Les poignées déplacent, tournent et mettent à l'échelle de nouveau.".into()
        } else if self.e3_cible_origine().is_some() {
            format!("{ORIGINE_NOM} : tire les flèches ou les carrés pour déplacer l'origine seule, l'objet ne bouge pas (Maj : l'accrocher aux points de sa boîte).")
        } else {
            format!("{ORIGINE_NOM} : choisis un seul calque ou un seul objet du décor 3D dans la vue ; ses flèches déplaceront son origine.")
        };
    }

    /// Ce dont « Modifier l'origine » déplace l'origine : ce qui est choisi, s'il est seul et a une origine (une carte, un objet
    /// du décor ; pas la caméra).
    pub(crate) fn e3_cible_origine(&self) -> Option<Cible> {
        let [s] = self.espace3d.sel.as_slice() else { return None };
        if let Some(id) = objet_of(*s) {
            return self.e3_objet(id).map(|_| Cible::Objet(id));
        }
        self.e3_card_layer(*s).map(Cible::Carte)
    }

    /// Le manipulateur de « Modifier l'origine » : des flèches et des carrés sur l'origine de la cible. Une carte : dans son
    /// repère (son origine est sur son dessin) ; un objet : dans celui de l'espace, ou le sien (« Poignées dans le repère du
    /// calque »).
    pub(crate) fn e3_gizmo_origine(&self, view: &View) -> Option<Gizmo> {
        let (origin, axes) = match self.e3_cible_origine()? {
            Cible::Carte(li) => {
                let world = self.e3_world();
                let l = &self.layers()[li];
                let xf = world.carte(l, self.cur_frame);
                (world.origine_monde(&xf, world.ecart(l)), gz::axes_for(true, &xf))
            }
            Cible::Objet(id) => {
                let o = self.e3_objet(id)?;
                (o.place.position, gz::axes_for(self.espace3d.local, &xf_de_l_objet(o)))
            }
        };
        origin.iter().all(|v| v.is_finite()).then(|| Gizmo { origin, axes, len: Gizmo::size_for(view, origin), mode: GizmoMode::Move })
    }

    /// Le début d'un geste « Modifier l'origine » : ce qu'il faut garder de la cible.
    pub(crate) fn e3_geste_origine(&self) -> Option<GesteOrigine> {
        match self.e3_cible_origine()? {
            Cible::Carte(li) => {
                let avant = self.layers()[li].clone();
                let xf = self.e3_world().carte(&avant, self.cur_frame);
                Some(GesteOrigine::Carte { li, avant, xf, boite: self.layer_content_box(li, self.cur_frame) })
            }
            Cible::Objet(id) => Some(GesteOrigine::Objet { seq: self.doc.env_cle(self.cur_panel), avant: self.e3_objet(id)?.clone() }),
        }
    }

    /// Le geste « Modifier l'origine » suit la souris : l'origine passe au point où la poignée la mène, reposée depuis la
    /// cible du début du geste ; ni la carte ni l'objet ne bougent.
    pub(crate) fn e3_drag_origine(&mut self, view: &View, center: [f32; 2], px: [f32; 2]) {
        let ray = view.ray(px, center);
        let Some(Drag::Handle { handle, gizmo, start, start_px, origine: Some(g), .. }) = &self.espace3d.drag else { return };
        let (handle, gizmo, start, start_px, geste) = (*handle, *gizmo, *start, *start_px, (**g).clone());
        let visee = start.d;
        let d = match handle {
            Handle::Axis(k) => {
                if m::dot(visee, gizmo.axes[k]).abs() > 0.97 {
                    self.status = format!("L'axe {} est vu de bout dans cette vue : ce geste ne le change pas. Prends une autre vue.", ["X", "Y", "Z"][k]);
                    return;
                }
                m::drag_along_axis(&start, &ray, gizmo.origin, gizmo.axes[k])
            }
            Handle::Plane(k) => {
                let n = gizmo.axes[k];
                if m::dot(visee, n).abs() < 0.15 {
                    // (Le plan est vu de profil : on glisse dans le plan de l'écran, à la profondeur de l'origine.)
                    match (view.screen_to_plane(start_px, center, gizmo.origin), view.screen_to_plane(px, center, gizmo.origin)) {
                        (Some(a), Some(b)) => m::sub(b, a),
                        _ => [0.0; 3],
                    }
                } else {
                    let d = m::drag_in_plane(&start, &ray, gizmo.origin, n);
                    m::sub(d, m::scale(n, m::dot(d, n)))
                }
            }
            _ => return,
        };
        if !d.iter().all(|v| v.is_finite()) {
            return;
        }
        let w = m::add(gizmo.origin, d);
        let maj = self.espace3d.maj;
        let accroche = if maj { " ; accrochée à sa boîte" } else { "" };
        match geste {
            GesteOrigine::Carte { li, avant, xf, boite } => {
                // Du point du monde au point du dessin : le carré unité de la carte, puis les pixels de la feuille.
                let world = self.e3_world();
                let [sw, sh] = world.stage;
                let local = m::point(&m::inverse_affine(&xf.matrix()), w);
                let mut p = pos2((local[0] + 0.5) * sw, (0.5 - local[1]) * sh);
                if !(p.x.is_finite() && p.y.is_finite()) {
                    return;
                }
                if let Some(b) = boite.filter(|_| maj) {
                    p = crate::origine::aimanter_a_la_boite(b, p);
                }
                if li >= self.layers().len() {
                    return;
                }
                let mut l = avant;
                crate::origine::deplacer_origine(&mut l, Some([p.x, p.y]), &world);
                self.layers_mut()[li] = l;
                self.status = format!("Origine du calque en x {:.0}, y {:.0} px (le dessin ne bouge pas){accroche}.", p.x, p.y);
            }
            GesteOrigine::Objet { seq, avant } => {
                let mut o = m::point(&m::inverse_affine(&avant.matrice()), w);
                if maj {
                    o = plus_proche(&points_de_la_boite(avant.modele.bornes), o);
                }
                if !o.iter().all(|v| v.is_finite()) {
                    return;
                }
                let mut neuf = avant;
                neuf.changer_origine(o);
                let nom = neuf.nom.clone();
                if let Some(slot) = self.doc.decors3d.par_sequence.get_mut(&seq).and_then(|d| d.objet_mut(neuf.id)) {
                    *slot = neuf;
                }
                self.status = format!("Origine de « {nom} » en {:.2} · {:.2} · {:.2} m du modèle (l'objet ne bouge pas){accroche}.", o[0], o[1], o[2]);
            }
        }
        self.modified = true;
        if let Some(Drag::Handle { changed, .. }) = &mut self.espace3d.drag {
            *changed = true;
        }
    }

    /// Le geste « Modifier l'origine » lâché : ce qui est aimanté se repose par sa nouvelle origine (une carte aimantée au sol ;
    /// un objet du décor, et les cartes posées sur lui), puis une étape d'annulation, nommée `nom`.
    pub(crate) fn e3_origine_lachee(&mut self, geste: GesteOrigine, snap: crate::history::Edit, nom: &str) {
        match geste {
            GesteOrigine::Carte { li, .. } => self.aimanter_si(li),
            GesteOrigine::Objet { seq, .. } => {
                if let Some(d) = self.doc.decors3d.par_sequence.get(&seq).cloned() {
                    self.decor3d_appliquer(seq, d);
                }
            }
        }
        self.commit_named(snap, nom);
        self.renderer.invalidate_all();
    }
}
