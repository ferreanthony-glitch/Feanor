//! L'outil Dégradé, côté interface (`degrade.rs` : le noyau). Anthony, 2026-10-24 : « un outil dégradé, vecto et bitmap ».
//!
//! * **Le geste** : appui, glisser, lâcher. Le glisser trace l'axe, du point d'appui (première pastille) au pointeur (dernière
//!   pastille). Par défaut : de la couleur d'avant-plan à celle d'arrière-plan. Maj : l'axe par pas de 15°.
//! * **Où il s'applique**, dans cet ordre : la sélection s'il y en a une (lasso, rectangle, couleur : bitmap ; zones
//!   remplies choisies : vectoriel) ; sinon la zone fermée du trait où l'on appuie, trouvée comme le fait le pot de
//!   peinture (`fill.rs`) ; Alt tenu (ou « Tout le calque » dans Réglages) : toute la feuille. Sur l'aplat visé et sur un
//!   calque de couleur, la zone est celle que le pot de peinture y remplit.
//! * **Bitmap** : les pixels s'écrivent au lâcher (`degrade::peindre`, par tuiles, sur les cœurs), bords anticrénelés comme
//!   avec le pot de peinture ou la sélection. **Vectoriel** : la zone garde un dégradé (`Element::Fill::gradient`), jamais
//!   des pixels ; il se rend avec elle, suit sa transformation, s'enregistre (format 38, puis 44 pour les pastilles).
//! * **Sans latence** : pendant le glisser, rien du moteur de pinceaux ne tourne. L'aperçu est un maillage de sommets
//!   colorés (le dégradé, calculé aux sommets d'une grille) posé sur une petite image de la zone (sa couverture, faite une
//!   fois à l'appui) : quelques milliers de sommets par image d'écran, quelle que soit la taille de la zone.
//! * **Retouche après coup** (2026-10-30) : après le lâcher, le geste devient une *séance* (`Phase::Pose`) : l'axe reste affiché
//!   avec ses poignées. Déplacer une poignée, changer les pastilles, la forme, l'angle ou l'échelle dans Réglages **défait**
//!   l'écriture (la retouche, `Phase::Retouche` : l'aperçu par maillage reprend), puis la **réécrit** au relâché : une seule étape
//!   d'annulation « Dégradé », quel que soit le nombre de retouches (Ctrl+Z l'enlève en entier). Changer d'outil, de calque ou
//!   d'image, ou Entrée / Échap, **fige** le dégradé (Échap fige, n'annule pas). Sur un calque vectoriel, la zone garde son
//!   dégradé pour toujours : la choisir (Sélection) puis reprendre l'outil Dégradé remontre l'axe (`degrade_retrouver_axe`).

use eframe::egui::{self, pos2, vec2, Color32, Id, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui};

use crate::app::App;
use crate::degrade::{self, Degrade, Fin, Genre, Pastille, PastilleCouleur, PastilleOpacite, Portee, Reglages, MAX_PASTILLES, MILIEU_MIN};
use crate::document::{CelId, LayerKind};
use crate::pixsel::PixelMask;
use crate::raster::Raster;
use crate::vector::{self, Element};

/// Où le dégradé s'écrit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Dest {
    /// Le dessin du calque.
    Calque,
    /// La peinture de l'aplat du calque.
    Aplat,
    /// Le dessin d'un calque de couleur.
    Couleur,
}

/// Ce que le dégradé couvre, en pixels (calques bitmap, aplat, calque de couleur).
pub(crate) enum PorteeGeste {
    Tout,
    Zone { region: Vec<bool>, grown: Vec<bool> },
    Masque(PixelMask),
}

impl PorteeGeste {
    fn vue(&self) -> Portee<'_> {
        match self {
            PorteeGeste::Tout => Portee::Tout,
            PorteeGeste::Zone { region, grown } => Portee::Zone { region, grown },
            PorteeGeste::Masque(m) => Portee::Masque(m),
        }
    }
}

/// Ce que le geste va changer.
pub(crate) enum Cible {
    /// Des pixels.
    Pixels { dest: Dest, portee: PorteeGeste },
    /// Des zones remplies déjà là (vectoriel) : elles reçoivent le dégradé.
    Elements { cel: CelId, items: Vec<usize> },
    /// Une zone neuve (vectoriel), tracée au lâcher d'après la zone trouvée.
    Zone { grown: Vec<bool> },
    /// Toute la feuille (vectoriel) : une zone neuve qui la couvre.
    Feuille,
}

/// L'image de la couverture de la zone, pour l'aperçu : blanche, d'opacité la couverture, sur le rectangle `rect` (pixels
/// du dessin).
pub(crate) struct Apercu {
    pub rect: Rect,
    pub tex: Option<egui::TextureHandle>,
}

/// Où en est le geste.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Phase {
    /// Le pointeur est tenu : l'axe se trace, rien n'est écrit.
    Trace,
    /// Le dégradé est écrit (une étape d'annulation en haut de l'historique) ; l'axe et ses poignées restent, on peut le retoucher.
    Pose,
    /// Une retouche est en cours : l'écriture est défaite, l'aperçu montre le dégradé ; elle se réécrit au relâché.
    Retouche,
}

/// La poignée de l'axe qu'on tire.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Poignee {
    Depart,
    Arrivee,
}

/// Le geste en cours, puis la séance de retouche.
pub(crate) struct Geste {
    pub depart: Pos2,
    pub fin: Pos2,
    pub reglages: Reglages,
    pub avant: [u8; 3],
    pub arriere: [u8; 3],
    pub cible: Cible,
    pub apercu: Apercu,
    pub phase: Phase,
    /// La poignée tirée (retouche).
    pub poignee: Option<Poignee>,
    /// Plan, calque et image où le dégradé a été posé : en changer fige le dégradé.
    pub contexte: (usize, usize, usize),
    /// Le numéro de l'historique juste après l'étape du dégradé (`Pose`), ou rien quand le dégradé n'a pas d'étape à lui (une zone
    /// vectorielle retrouvée) : toute autre étape fige le dégradé.
    pub etape: Option<u64>,
    /// La longueur d'axe qui vaut 100 % d'échelle.
    pub base: f32,
    /// Le dégradé tel qu'il est écrit.
    pub applique: Option<Degrade>,
}

impl Geste {
    /// Le dégradé tel que l'axe tiré le donne maintenant.
    pub(crate) fn degrade(&self) -> Degrade {
        self.reglages.degrade(self.avant, self.arriere, self.depart, self.fin)
    }
}

/// L'état de l'interface des Réglages du dégradé : la pastille choisie, le sélecteur de couleur, ce qu'on a déjà proposé de retoucher.
#[derive(Default)]
pub(crate) struct EtatUi {
    /// La pastille de couleur choisie.
    pub sel_couleur: Option<usize>,
    /// La pastille d'opacité choisie.
    pub sel_opacite: Option<usize>,
    /// Le sélecteur de couleur de la pastille choisie est ouvert (double-clic).
    pub selecteur: bool,
    /// La couleur du panneau Couleur à la dernière image : une autre, avec une pastille choisie, la lui donne.
    pub derniere_couleur: Option<[u8; 3]>,
    /// La dernière image d'écran où les Réglages du dégradé étaient visibles.
    pub vu: u64,
    /// La bande du dégradé telle qu'elle est à l'écran (dernière image) : où cliquer pour les pastilles (et pour les essais).
    pub bande: Option<Rect>,
    /// Les vignettes de « Mes dégradés » et le bouton « + » à l'écran (dernière image) : où cliquer (et pour les essais).
    pub vignettes: Vec<Rect>,
    pub plus: Option<Rect>,
    /// Les zones vectorielles dont on a déjà proposé de retoucher le dégradé (il ne revient pas après Échap).
    pub offre: Option<(CelId, Vec<usize>)>,
}

/// Au plus ce nombre de pixels dans l'image de couverture de l'aperçu : au-delà, elle est réduite (le bord de la zone y
/// est un peu plus doux, le dégradé n'en dépend pas).
const APERCU_PIXELS: usize = 4_000_000;
/// Cases de la grille de l'aperçu, par côté.
const GRILLE: usize = 56;
/// Cases de la grille pour les formes qui ne sont pas des pentes simples (angle, losange, réfléchi).
const GRILLE_FINE: usize = 96;
/// Longueur d'axe (pixels de l'écran) sous laquelle un lâcher n'est pas un dégradé (un clic).
const AXE_MIN: f32 = 3.0;
/// Rayon (pixels de l'écran) où l'on prend une poignée de l'axe.
const PRISE: f32 = 10.0;

/// L'image de couverture de `cover` sur le rectangle (`x0`, `y0`, `w`, `h`) du dessin.
fn apercu_de(ctx: &egui::Context, (x0, y0, w, h): (usize, usize, usize, usize), cover: &dyn Fn(usize, usize) -> f32) -> Apercu {
    let rect = Rect::from_min_size(pos2(x0 as f32, y0 as f32), vec2(w as f32, h as f32));
    if w == 0 || h == 0 {
        return Apercu { rect, tex: None };
    }
    let step = ((w * h) as f32 / APERCU_PIXELS as f32).sqrt().ceil().max(1.0) as usize;
    let (tw, th) = (w.div_ceil(step), h.div_ceil(step));
    let mut bytes = Vec::with_capacity(tw * th * 4);
    for j in 0..th {
        for i in 0..tw {
            let c = (cover((x0 + i * step).min(x0 + w - 1), (y0 + j * step).min(y0 + h - 1)).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            bytes.extend_from_slice(&[c, c, c, c]);
        }
    }
    let image = egui::ColorImage::from_rgba_premultiplied([tw, th], &bytes);
    Apercu { rect, tex: Some(ctx.load_texture("degrade-apercu", image, egui::TextureOptions::LINEAR)) }
}

/// Le rectangle (`x0`, `y0`, `w`, `h`) des pixels vrais de `mask` (`w` × `h` : la feuille), ou le rectangle vide.
fn boite_de(mask: &[bool], w: usize, h: usize) -> (usize, usize, usize, usize) {
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
    for y in 0..h {
        let row = &mask[y * w..(y + 1) * w];
        if let Some(a) = row.iter().position(|&b| b) {
            let b = row.iter().rposition(|&b| b).unwrap_or(a);
            x0 = x0.min(a);
            x1 = x1.max(b + 1);
            y0 = y0.min(y);
            y1 = y1.max(y + 1);
        }
    }
    if x0 >= x1 {
        (0, 0, 0, 0)
    } else {
        (x0, y0, x1 - x0, y1 - y0)
    }
}

/// La fin de l'axe qui part de `depart` dans la direction `angle_degres` (0 : vers la droite, sens des aiguilles d'une montre), sur
/// `longueur` pixels : de quoi taper l'angle et l'échelle en chiffres.
pub(crate) fn axe_depuis(depart: Pos2, angle_degres: f32, longueur: f32) -> Pos2 {
    let a = angle_degres.to_radians();
    depart + vec2(a.cos(), a.sin()) * longueur
}

/// L'axe tiré avec Maj : par pas de 15° autour de `depart`.
fn axe_par_15(depart: Pos2, p: Pos2) -> Pos2 {
    let d = p - depart;
    let angle = (d.y.atan2(d.x) / (std::f32::consts::PI / 12.0)).round() * (std::f32::consts::PI / 12.0);
    depart + vec2(angle.cos(), angle.sin()) * d.length()
}

impl App {
    /// Appui avec l'outil Dégradé, au point `p` du dessin ; `alt` : tout le calque.
    pub(crate) fn degrade_press(&mut self, p: Pos2, alt: bool) {
        self.end_stroke();
        self.playing = false;
        // Un dégradé déjà posé, dont on prend une poignée : la retouche commence (même étape d'annulation).
        if self.degrade_session_valide() {
            let z = self.layer_zoom();
            let prise = self.degrade_geste.as_ref().filter(|g| g.phase == Phase::Pose).and_then(|g| {
                let (d0, d1) = (g.depart.distance(p) * z, g.fin.distance(p) * z);
                if d1 <= PRISE && d1 <= d0 {
                    Some(Poignee::Arrivee)
                } else if d0 <= PRISE {
                    Some(Poignee::Depart)
                } else {
                    None
                }
            });
            if let Some(poignee) = prise {
                self.degrade_ouvrir_retouche();
                if let Some(g) = self.degrade_geste.as_mut() {
                    g.poignee = Some(poignee);
                }
                return;
            }
        }
        // Sinon, le dégradé posé est figé et un autre geste commence.
        self.degrade_geste = None;
        if self.layer().couleur.as_ref().is_some_and(|c| c.griffonner) {
            self.status = "Le dégradé ne se pose pas en mode « Griffonner » : éteins-le (menu du calque de couleur).".into();
            return;
        }
        if !self.layer_editable() {
            return;
        }
        let tout = alt || self.degrade.tout_le_calque;
        let (w, h) = (self.doc.width, self.doc.height);
        let dedans = p.x >= 0.0 && p.y >= 0.0 && p.x < w as f32 && p.y < h as f32;
        let hors = "Le dégradé remplit dans la feuille : appuie sur une zone fermée de la feuille (Alt : tout le calque).";
        let aplat = self.aplat_en_cible();
        let couleur = self.layer().couleur.is_some();
        let cible_apercu: Option<(Cible, Apercu)> = if aplat {
            self.degrade_cible_aplat(p, tout, dedans, hors)
        } else if couleur {
            self.degrade_cible_couleur(p, tout, dedans, hors)
        } else if self.layer().kind == LayerKind::Vector {
            self.degrade_cible_vecteur(p, tout, dedans, hors)
        } else {
            self.degrade_cible_bitmap(p, tout, dedans, hors)
        };
        let Some((cible, apercu)) = cible_apercu else { return };
        self.degrade_geste = Some(Geste {
            depart: p,
            fin: p,
            reglages: self.degrade.clone(),
            avant: self.color,
            arriere: self.couleurs.prefs.back,
            cible,
            apercu,
            phase: Phase::Trace,
            poignee: None,
            contexte: (self.cur_panel, self.cur_layer, self.cur_frame),
            etape: None,
            base: 0.0,
            applique: None,
        });
    }

    fn degrade_apercu_pixels(&self, portee: &PorteeGeste) -> Apercu {
        let (w, h) = (self.doc.width, self.doc.height);
        match portee {
            PorteeGeste::Tout => Apercu { rect: Rect::from_min_size(Pos2::ZERO, vec2(w as f32, h as f32)), tex: None },
            PorteeGeste::Zone { grown, .. } => apercu_de(&self.ctx, boite_de(grown, w, h), &|x, y| if grown[y * w + x] { 1.0 } else { 0.0 }),
            PorteeGeste::Masque(m) => apercu_de(&self.ctx, (m.x0, m.y0, m.w, m.h), &|x, y| m.at(x, y)),
        }
    }

    /// Une zone trouvée comme le fait le pot de peinture sur le dessin du calque (ou sur tous les calques, d'après ses
    /// réglages), mais bornée par le trait seul : elle traverse un dégradé ou une peinture douce (`degrade::zone_douce`).
    /// La zone et la zone élargie du débord.
    fn degrade_zone_du_pot(&self, p: Pos2) -> (Vec<bool>, Vec<bool>) {
        let (w, h) = (self.doc.width, self.doc.height);
        let source = self.fill_source();
        let tolerance = (self.fill.tolerance * 255.0).round() as u8;
        let region = degrade::zone_douce(&source, w, h, p.x as usize, p.y as usize, tolerance);
        let grown = crate::fill::dilate(&region, w, h, self.fill.expand);
        (region, grown)
    }

    fn degrade_cible_bitmap(&mut self, p: Pos2, tout: bool, dedans: bool, hors: &str) -> Option<(Cible, Apercu)> {
        // Une sélection bitmap : le dégradé n'en remplit que la zone.
        if let Some(sel) = self.bsel.as_ref().filter(|s| Some(s.cel) == self.cel_here()) {
            if sel.lifted.is_some() {
                self.status = "La zone est soulevée : pose-la d'abord (Échap ou autre outil), puis fais le dégradé.".into();
                return None;
            }
            let portee = PorteeGeste::Masque(sel.mask.clone());
            let apercu = self.degrade_apercu_pixels(&portee);
            return Some((Cible::Pixels { dest: Dest::Calque, portee }, apercu));
        }
        let portee = if tout {
            PorteeGeste::Tout
        } else if !dedans {
            self.status = hors.into();
            return None;
        } else {
            let (region, grown) = self.degrade_zone_du_pot(p);
            PorteeGeste::Zone { region, grown }
        };
        let apercu = self.degrade_apercu_pixels(&portee);
        Some((Cible::Pixels { dest: Dest::Calque, portee }, apercu))
    }

    fn degrade_cible_aplat(&mut self, p: Pos2, tout: bool, dedans: bool, hors: &str) -> Option<(Cible, Apercu)> {
        let Some(r) = self.layer().fond else { return None };
        let Some(cel) = self.cel_here() else {
            self.status = "Aplat : pas de dessin à cette image.".into();
            return None;
        };
        let portee = if tout {
            PorteeGeste::Tout
        } else if !dedans {
            self.status = hors.into();
            return None;
        } else {
            // La zone fermée par le trait, celle que le pot de peinture remplit dans l'aplat (`fondui.rs`).
            let Some(mask) = self.doc.cels.get(&cel).and_then(|c| crate::fond::zone(&c.raster, p.x as usize, p.y as usize, r.trous)) else {
                self.status = "Aplat : clique dans une zone fermée par le trait (pas sur le trait).".into();
                return None;
            };
            PorteeGeste::Zone { grown: mask.clone(), region: mask }
        };
        let apercu = self.degrade_apercu_pixels(&portee);
        Some((Cible::Pixels { dest: Dest::Aplat, portee }, apercu))
    }

    fn degrade_cible_couleur(&mut self, p: Pos2, tout: bool, dedans: bool, hors: &str) -> Option<(Cible, Apercu)> {
        let (w, h) = (self.doc.width, self.doc.height);
        let portee = if tout {
            PorteeGeste::Tout
        } else if !dedans {
            self.status = hors.into();
            return None;
        } else {
            let zone = match self.zone_couleur(p) {
                Ok(z) => z,
                Err(e) => {
                    self.status = e;
                    return None;
                }
            };
            let mut mask = vec![false; w * h];
            for y in 0..h {
                for x in 0..w {
                    mask[y * w + x] = zone.contient(x, y);
                }
            }
            PorteeGeste::Zone { grown: mask.clone(), region: mask }
        };
        let apercu = self.degrade_apercu_pixels(&portee);
        Some((Cible::Pixels { dest: Dest::Couleur, portee }, apercu))
    }

    fn degrade_cible_vecteur(&mut self, p: Pos2, tout: bool, dedans: bool, hors: &str) -> Option<(Cible, Apercu)> {
        let (w, h) = (self.doc.width, self.doc.height);
        self.flush_flottante();
        // Une sélection de zones remplies : elles reçoivent le dégradé.
        if let Some(sel) = self.vsel.as_ref().filter(|s| Some(s.cel) == self.cel_here()) {
            let cel = sel.cel;
            let items: Vec<usize> = sel
                .items
                .iter()
                .copied()
                .filter(|&i| self.drawing(cel).and_then(|d| d.elements.get(i)).is_some_and(|e| matches!(e, Element::Fill { .. })))
                .collect();
            if items.is_empty() {
                self.status = "Le dégradé remplit des zones : choisis une zone remplie, ou ôte la sélection pour remplir une zone fermée du trait.".into();
                return None;
            }
            let apercu = self.degrade_apercu_elements(cel, &items);
            return Some((Cible::Elements { cel, items }, apercu));
        }
        if tout {
            return Some((Cible::Feuille, Apercu { rect: Rect::from_min_size(Pos2::ZERO, vec2(w as f32, h as f32)), tex: None }));
        }
        if !dedans {
            self.status = hors.into();
            return None;
        }
        // Une zone déjà remplie sous le pointeur : elle reçoit le dégradé (le pot de peinture en change la couleur).
        if let Some(cel) = self.cel_here() {
            let tol = 2.0 / self.layer_zoom();
            if let Some(i) = self.drawing(cel).and_then(|d| d.hit_test(p, tol)) {
                if matches!(self.drawing(cel).map(|d| &d.elements[i]), Some(Element::Fill { .. })) {
                    let apercu = self.degrade_apercu_elements(cel, &[i]);
                    return Some((Cible::Elements { cel, items: vec![i] }, apercu));
                }
                self.status = "Le dégradé remplit des zones : appuie dans une zone fermée du trait, pas sur le trait lui-même.".into();
                return None;
            }
        }
        let (_, grown) = self.degrade_zone_du_pot(p);
        let apercu = apercu_de(&self.ctx, boite_de(&grown, w, h), &|x, y| if grown[y * w + x] { 1.0 } else { 0.0 });
        Some((Cible::Zone { grown }, apercu))
    }

    /// L'image de couverture des zones `items` du dessin vectoriel `cel` (les seules zones remplies sont rendues).
    fn degrade_apercu_elements(&self, cel: CelId, items: &[usize]) -> Apercu {
        let (w, h) = (self.doc.width, self.doc.height);
        let Some(d) = self.drawing(cel) else { return Apercu { rect: Rect::NOTHING, tex: None } };
        let mut r = Raster::new(w, h);
        let mut boite = Rect::NOTHING;
        for &i in items {
            if let Some(e) = d.elements.get(i) {
                vector::render_element(&mut r, e);
                boite = boite.union(e.bounds());
            }
        }
        if !boite.is_finite() {
            return Apercu { rect: Rect::NOTHING, tex: None };
        }
        let x0 = (boite.min.x.floor().max(0.0) as usize).min(w);
        let y0 = (boite.min.y.floor().max(0.0) as usize).min(h);
        let x1 = (boite.max.x.ceil().max(0.0) as usize).min(w);
        let y1 = (boite.max.y.ceil().max(0.0) as usize).min(h);
        if x0 >= x1 || y0 >= y1 {
            return Apercu { rect: Rect::NOTHING, tex: None };
        }
        apercu_de(&self.ctx, (x0, y0, x1 - x0, y1 - y0), &|x, y| r.get(x, y).a() as f32 / 255.0)
    }

    /// Le pointeur est en `p` : l'axe suit (Maj : par pas de 15°), ou la poignée tirée. Vrai si un geste de dégradé est en cours.
    pub(crate) fn degrade_drag(&mut self, p: Pos2, shift: bool) -> bool {
        let Some(g) = self.degrade_geste.as_mut() else { return false };
        match (g.phase, g.poignee) {
            (Phase::Trace, _) | (Phase::Retouche, Some(Poignee::Arrivee)) => g.fin = if shift { axe_par_15(g.depart, p) } else { p },
            (Phase::Retouche, Some(Poignee::Depart)) => g.depart = p,
            _ => return false,
        }
        true
    }

    /// Lâcher : le dégradé s'écrit (une étape d'annulation). Un simple clic ne fait rien. Pendant une retouche : elle se réécrit.
    pub(crate) fn degrade_release(&mut self) {
        let Some(g) = self.degrade_geste.as_ref() else { return };
        match g.phase {
            Phase::Pose => {}
            Phase::Trace => {
                if g.depart.distance(g.fin) * self.layer_zoom() < AXE_MIN {
                    self.degrade_geste = None;
                    self.status = "Glisse pour tracer l'axe du dégradé : le point d'appui a la couleur d'avant-plan, le pointeur celle d'arrière-plan.".into();
                    return;
                }
                self.degrade_valider();
            }
            Phase::Retouche => self.degrade_valider(),
        }
    }

    /// Un geste abandonné (une autre action commence) : le tracé en cours est jeté, une retouche en cours s'écrit, un dégradé posé reste.
    pub(crate) fn degrade_abandonner(&mut self) {
        match self.degrade_geste.as_ref().map(|g| g.phase) {
            Some(Phase::Trace) => self.degrade_geste = None,
            Some(Phase::Retouche) => self.degrade_valider(),
            _ => {}
        }
    }

    /// Le dégradé posé n'est plus retouchable : il est figé (Entrée, Échap, autre outil, autre calque, autre image…).
    pub(crate) fn degrade_figer(&mut self) {
        self.degrade_abandonner();
        self.degrade_geste = None;
    }

    /// Les réglages d'une séance suivent ceux du panneau (pastilles, forme, opacité). Les carrés avant-plan et arrière-plan, eux, sont ceux
    /// du moment où le dégradé a été tracé : choisir une autre couleur pour le dégradé suivant ne change pas celui-ci (sauf par une
    /// pastille choisie, qui devient alors la sienne).
    fn degrade_synchroniser(&mut self) {
        let reglages = self.degrade.clone();
        if let Some(g) = self.degrade_geste.as_mut().filter(|g| g.phase != Phase::Trace) {
            g.reglages = reglages;
        }
    }

    /// Écrit le dégradé du geste (premier lâcher, ou fin d'une retouche) ; la séance continue (`Phase::Pose`).
    fn degrade_valider(&mut self) {
        self.degrade_synchroniser();
        let Some(mut g) = self.degrade_geste.take() else { return };
        let premiere = g.phase == Phase::Trace;
        // Une retouche ne s'écrit que là où le dégradé a été posé (l'écriture était défaite : sans cela, elle serait perdue).
        if g.phase == Phase::Retouche && g.contexte != (self.cur_panel, self.cur_layer, self.cur_frame) {
            return;
        }
        let rebase = premiere || g.poignee.is_some();
        g.poignee = None;
        if self.degrade_appliquer(&g, premiere) {
            g.phase = Phase::Pose;
            g.etape = Some(self.history.serial());
            g.applique = Some(g.degrade());
            if rebase || g.base <= 0.0 {
                g.base = g.depart.distance(g.fin);
            }
            self.degrade_geste = Some(g);
        }
    }

    /// Défait l'écriture du dégradé posé (l'étape du dégradé, sans la garder pour « Rétablir ») : l'aperçu la remplace pendant la retouche.
    fn degrade_defaire(&mut self) {
        let sel = self.vsel.as_ref().map(|s| (s.cel, s.items.clone()));
        let mut nav = crate::history::Nav { panel: self.cur_panel, layer: self.cur_layer };
        if self.history.undo_and_forget(&mut self.doc, &mut nav) {
            self.rejeu_after_history();
            self.renderer.invalidate_all();
            self.modified = true;
            if let Some((cel, items)) = sel {
                self.vsel = Some(crate::vtools::VSelection { cel, items });
            }
        }
    }

    /// Une retouche commence : l'écriture est défaite (si le dégradé en a une), l'aperçu reprend.
    fn degrade_ouvrir_retouche(&mut self) {
        let Some(g) = self.degrade_geste.as_ref() else { return };
        if g.phase != Phase::Pose {
            return;
        }
        if g.etape.is_some() {
            self.degrade_defaire();
        }
        if let Some(g) = self.degrade_geste.as_mut() {
            g.etape = None;
            g.phase = Phase::Retouche;
        }
    }

    /// La séance est toujours celle de ce plan, de ce calque, de cette image, avec l'outil Dégradé, et son étape est toujours la dernière.
    pub(crate) fn degrade_session_valide(&self) -> bool {
        let Some(g) = self.degrade_geste.as_ref().filter(|g| g.phase != Phase::Trace) else { return false };
        self.tool == crate::app::Tool::Gradient && g.contexte == (self.cur_panel, self.cur_layer, self.cur_frame) && g.etape.is_none_or(|s| s == self.history.serial())
    }

    /// Une fois par image, avec le dessin : la séance de retouche suit les réglages ; Entrée ou Échap la figent ; un dégradé qui n'est
    /// plus d'actualité (autre outil, calque, image, étape) est figé ; une zone vectorielle choisie remontre son axe.
    pub(crate) fn degrade_tick(&mut self) {
        if self.tool != crate::app::Tool::Gradient {
            self.degrade_ui.offre = None;
        }
        let Some(phase) = self.degrade_geste.as_ref().map(|g| g.phase) else {
            self.degrade_retrouver_axe();
            return;
        };
        if phase == Phase::Trace {
            return;
        }
        if !self.degrade_session_valide() {
            self.degrade_figer();
            return;
        }
        let fige = self.ctx.input(|i| i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Enter)) && !self.ctx.wants_keyboard_input();
        if fige && phase == Phase::Pose {
            self.degrade_figer();
            self.status = "Dégradé figé : il n'est plus retouchable ici (une zone vectorielle garde son dégradé : choisis-la, puis reprends l'outil).".into();
            return;
        }
        self.degrade_synchroniser();
        let sale = self.degrade_geste.as_ref().is_some_and(|g| g.phase == Phase::Pose && g.applique.as_ref() != Some(&g.degrade()));
        if sale {
            self.degrade_ouvrir_retouche();
        }
    }

    /// Une zone vectorielle en dégradé est choisie, avec l'outil Dégradé : son axe revient, avec ses poignées. Une seule fois par choix.
    fn degrade_retrouver_axe(&mut self) {
        if self.tool != crate::app::Tool::Gradient || self.aplat_en_cible() || self.layer().couleur.is_some() || self.layer().kind != LayerKind::Vector {
            return;
        }
        let Some(cel) = self.cel_here() else { return };
        let Some(sel) = self.vsel.as_ref().filter(|s| s.cel == cel) else { return };
        let choix: Vec<usize> = sel.items.clone();
        let items: Vec<usize> = choix.iter().copied().filter(|&i| self.drawing(cel).and_then(|d| d.elements.get(i)).is_some_and(|e| e.gradient().is_some())).collect();
        if items.is_empty() || self.degrade_ui.offre.as_ref() == Some(&(cel, choix.clone())) {
            return;
        }
        self.degrade_ui.offre = Some((cel, choix));
        let Some(g) = self.drawing(cel).and_then(|d| d.elements.get(items[0])).and_then(|e| e.gradient()).cloned() else { return };
        let apercu = self.degrade_apercu_elements(cel, &items);
        let reglages = Reglages::depuis(&g);
        self.degrade = Reglages { tout_le_calque: self.degrade.tout_le_calque, ..reglages.clone() };
        self.degrade_geste = Some(Geste {
            depart: pos2(g.depart[0], g.depart[1]),
            fin: pos2(g.arrivee[0], g.arrivee[1]),
            reglages: self.degrade.clone(),
            avant: self.color,
            arriere: self.couleurs.prefs.back,
            cible: Cible::Elements { cel, items },
            apercu,
            phase: Phase::Pose,
            poignee: None,
            contexte: (self.cur_panel, self.cur_layer, self.cur_frame),
            etape: None,
            base: g.longueur(),
            applique: Some(g),
        });
    }

    /// Écrit le dégradé du geste `g` ; vrai si une étape d'annulation a été enregistrée. `premiere` : le premier lâcher (les couleurs
    /// du dégradé entrent dans les couleurs récentes).
    fn degrade_appliquer(&mut self, g: &Geste, premiere: bool) -> bool {
        let degrade = g.degrade();
        if premiere {
            for p in &degrade.couleurs {
                self.color_used(p.rgb);
            }
        }
        let (w, h) = (self.doc.width, self.doc.height);
        match &g.cible {
            Cible::Pixels { dest: Dest::Calque, portee } => {
                if !self.layer_editable() {
                    return false;
                }
                let (cel, prefix) = self.ensure_cel();
                let keep = self.layer().keep_alpha;
                let stencil = if matches!(portee, PorteeGeste::Zone { .. }) { None } else { self.stencil_for(cel) };
                let Some(c) = self.doc.cels.get_mut(&cel) else { return false };
                let tiles = degrade::peindre(&mut c.raster, &portee.vue(), &degrade, keep, stencil.as_ref());
                if tiles.is_empty() && prefix.is_none() {
                    self.status = "Rien n'a changé : le dégradé tombe hors de la zone, ou sur ce qui avait déjà ces pixels.".into();
                    return false;
                }
                self.named_step("Dégradé", |app| app.push_pixels(cel, prefix, tiles));
                self.status = "Dégradé posé (tu peux le retoucher : poignées de l'axe, ou Réglages).".into();
                true
            }
            Cible::Pixels { dest: Dest::Couleur, portee } => {
                if !self.layer_editable() {
                    return false;
                }
                let (cel, prefix) = self.ensure_colour_cel();
                let Some(c) = self.doc.cels.get_mut(&cel) else { return false };
                let tiles = degrade::peindre(&mut c.raster, &portee.vue(), &degrade, false, None);
                self.named_step("Dégradé", |app| app.push_pixels(cel, prefix, tiles));
                self.status = "Dégradé posé sur le calque de couleur, sous le trait.".into();
                true
            }
            Cible::Pixels { dest: Dest::Aplat, portee } => {
                if !self.layer_editable() {
                    return false;
                }
                let Some(cel) = self.cel_here() else { return false };
                let Some(c) = self.doc.cels.get_mut(&cel) else { return false };
                let s = c.fond.get_or_insert_with(|| Box::new(crate::document::Strate::new(w, h)));
                let tiles = degrade::peindre(&mut s.peinture.raster, &portee.vue(), &degrade, false, None);
                let mut edits = vec![crate::history::Edit::FondTiles { cel, tiles, coupe: false }];
                // La zone recoud l'aplat s'il était coupé là (comme le pot de peinture).
                if let PorteeGeste::Zone { region, .. } = portee {
                    let old = crate::fond::paint_mask(&mut s.coupe, region, None);
                    if !old.is_empty() {
                        edits.push(crate::history::Edit::FondTiles { cel, tiles: old, coupe: true });
                    }
                }
                s.peinture.version += 1;
                c.version += 1;
                let avant = self.history.context.replace("Aplat : dégradé".to_string());
                self.history.push(crate::history::Edit::Group(edits));
                self.history.context = avant;
                self.modified = true;
                self.renderer.invalidate_all();
                self.status = "Aplat : dégradé posé, sous le trait.".into();
                true
            }
            Cible::Elements { cel, items } => {
                let cel = *cel;
                let Some(d) = self.drawing_mut(cel) else { return false };
                let before = d.clone();
                let mut region = Rect::NOTHING;
                for &i in items {
                    if let Some(Element::Fill { color, opacity, gradient, .. }) = d.elements.get_mut(i) {
                        *gradient = Some(degrade.clone());
                        *color = degrade.c0();
                        *opacity = 1.0;
                    }
                    if let Some(e) = d.elements.get(i) {
                        region = region.union(e.bounds());
                    }
                }
                self.refresh_cel_region(cel, region);
                self.named_step("Dégradé", |app| app.push_vector(cel, before, None));
                self.status = "Dégradé posé sur la zone (il reste modifiable : l'outil Sélection la déplace avec son axe).".into();
                true
            }
            Cible::Zone { grown } => {
                let contours = vector::trace_contours(grown, w, h);
                self.degrade_nouvelle_zone(contours, degrade)
            }
            Cible::Feuille => {
                let (fw, fh) = (w as f32, h as f32);
                self.degrade_nouvelle_zone(vec![vec![[0.0, 0.0], [fw, 0.0], [fw, fh], [0.0, fh]]], degrade)
            }
        }
    }

    /// Une zone vectorielle neuve en dégradé, sous les traits (comme le pot de peinture).
    fn degrade_nouvelle_zone(&mut self, contours: Vec<Vec<[f32; 2]>>, degrade: Degrade) -> bool {
        if contours.is_empty() || !self.layer_editable() {
            return false;
        }
        let (cel, prefix) = self.ensure_cel();
        let Some(d) = self.drawing_mut(cel) else { return false };
        let before = d.clone();
        let element = Element::Fill { contours, color: degrade.c0(), opacity: 1.0, gradient: Some(degrade) };
        let region = element.bounds();
        let at = d.fill_insert_index();
        d.elements.insert(at, element);
        // L'insertion décale les éléments : une sélection garderait de mauvais indices.
        self.vsel = None;
        self.refresh_cel_region(cel, region);
        self.named_step("Dégradé", |app| app.push_vector(cel, before, prefix));
        self.status = "Dégradé posé : une zone vectorielle, sous les traits.".into();
        true
    }

    // ------------------------------------------------------------------ l'aperçu

    /// L'aperçu du geste en cours, par-dessus le dessin : le dégradé sur la zone (pendant le tracé et les retouches), et l'axe avec ses
    /// poignées (tant que le dégradé n'est pas figé).
    pub(crate) fn paint_degrade_overlay(&self, painter: &egui::Painter) {
        let Some(g) = &self.degrade_geste else { return };
        let degrade = g.degrade();
        let r = g.apercu.rect;
        if g.phase != Phase::Pose && r.is_finite() && r.width() > 0.0 && r.height() > 0.0 {
            let n = if matches!(degrade.genre, Genre::Lineaire | Genre::Radial) { GRILLE } else { GRILLE_FINE };
            let (nx, ny) = (n, n);
            let couleurs = degrade::sommets(&degrade, r.min, r.max, nx, ny, 1.0);
            let mut mesh = match &g.apercu.tex {
                Some(t) => egui::Mesh::with_texture(t.id()),
                None => egui::Mesh::default(),
            };
            for j in 0..=ny {
                for i in 0..=nx {
                    let (u, v) = (i as f32 / nx as f32, j as f32 / ny as f32);
                    let p = pos2(r.min.x + r.width() * u, r.min.y + r.height() * v);
                    let uv = if g.apercu.tex.is_some() { pos2(u, v) } else { egui::epaint::WHITE_UV };
                    mesh.vertices.push(egui::epaint::Vertex { pos: self.layer_to_screen(p), uv, color: couleurs[j * (nx + 1) + i] });
                }
            }
            for j in 0..ny {
                for i in 0..nx {
                    let a = (j * (nx + 1) + i) as u32;
                    let (b, c, d) = (a + 1, a + nx as u32 + 1, a + nx as u32 + 2);
                    mesh.add_triangle(a, b, d);
                    mesh.add_triangle(a, d, c);
                }
            }
            painter.add(Shape::mesh(mesh));
        }
        // L'axe : un trait avec un rond à chaque bout (le rond de départ plein, celui d'arrivée vide) ; en radial, le cercle d'arrivée ;
        // en réfléchi, l'arrivée miroir ; en losange, le losange.
        let (a, b) = (self.layer_to_screen(g.depart), self.layer_to_screen(g.fin));
        let ombre = Stroke::new(3.0_f32, Color32::from_black_alpha(150));
        let trait_ = Stroke::new(1.5_f32, Color32::WHITE);
        let fin_trait = Stroke::new(1.0_f32, Color32::from_white_alpha(200));
        match degrade.genre {
            Genre::Radial => {
                let rayon = a.distance(b);
                painter.circle_stroke(a, rayon, ombre);
                painter.circle_stroke(a, rayon, fin_trait);
            }
            Genre::Reflechi => {
                let m = a - (b - a);
                painter.line_segment([a, m], ombre);
                painter.line_segment([a, m], fin_trait);
                painter.circle_stroke(m, 3.5, Stroke::new(1.0_f32, Color32::from_white_alpha(200)));
            }
            Genre::Losange => {
                let d = b - a;
                let p = vec2(-d.y, d.x);
                let pts = vec![a + d, a + p, a - d, a - p, a + d];
                painter.add(Shape::line(pts.clone(), ombre));
                painter.add(Shape::line(pts, fin_trait));
            }
            Genre::Lineaire | Genre::Angle => {}
        }
        painter.line_segment([a, b], ombre);
        painter.line_segment([a, b], trait_);
        painter.circle_filled(a, 4.5, Color32::WHITE);
        painter.circle_stroke(a, 4.5, Stroke::new(1.0_f32, Color32::BLACK));
        painter.circle_stroke(b, 4.5, Stroke::new(1.5_f32, Color32::BLACK));
        painter.circle_stroke(b, 3.5, Stroke::new(1.5_f32, Color32::WHITE));
    }

    // ------------------------------------------------------------------ Réglages

    /// Rend les deux séries de pastilles les leurs (les couleurs ne suivent plus les carrés du panneau Couleur) ; l'inversion en vigueur
    /// y est passée.
    fn degrade_detacher(&mut self, avant: [u8; 3]) {
        if self.degrade.couleurs.is_some() && self.degrade.transparence.is_some() && !self.degrade.inverse {
            return;
        }
        let g = self.degrade.degrade(avant, self.couleurs.prefs.back, Pos2::ZERO, pos2(1.0, 0.0));
        let o = self.degrade.opacite.clamp(0.0, 1.0);
        let mut op = g.opacites;
        if o > 0.0 {
            for p in &mut op {
                p.opacite = (p.opacite / o).clamp(0.0, 1.0);
            }
        }
        self.degrade.couleurs = Some(g.couleurs);
        self.degrade.transparence = Some(op);
        self.degrade.inverse = false;
    }

    /// Les réglages de l'outil : la forme, les pastilles de couleur et d'opacité, l'angle et l'échelle, inverser, tramage, tout le calque,
    /// et les dégradés enregistrés.
    pub(crate) fn degrade_options_ui(&mut self, ui: &mut Ui) {
        use crate::bulles::{bulle, choix};
        use egui_phosphor::regular as ph;
        // (Les Réglages ne sont plus montrés depuis un moment : la pastille choisie ne l'est plus, la couleur d'avant est oubliée.)
        let passe = ui.ctx().cumulative_pass_nr();
        if passe > self.degrade_ui.vu + 3 {
            self.degrade_ui.sel_couleur = None;
            self.degrade_ui.sel_opacite = None;
            self.degrade_ui.derniere_couleur = None;
            self.degrade_ui.selecteur = false;
        }
        self.degrade_ui.vu = passe;
        // Une couleur choisie dans le panneau Couleur va à la pastille choisie.
        let couleur_panneau = self.color;
        if let Some(derniere) = self.degrade_ui.derniere_couleur {
            if derniere != couleur_panneau {
                if let Some(i) = self.degrade_ui.sel_couleur {
                    // (Liées aux carrés, les pastilles deviennent les leurs avec les couleurs d'avant le changement.)
                    self.degrade_detacher(derniere);
                    if let Some(p) = self.degrade.couleurs.as_mut().and_then(|c| c.get_mut(i)) {
                        p.rgb = couleur_panneau;
                    }
                }
            }
        }
        self.degrade_ui.derniere_couleur = Some(couleur_panneau);

        ui.horizontal(|ui| {
            ui.label("Forme");
            choix(ui, &mut self.degrade.genre, Genre::Lineaire, ph::GRADIENT, "Linéaire", "La couleur change le long de l'axe tracé.", "");
            choix(ui, &mut self.degrade.genre, Genre::Radial, ph::TARGET, "Radial", "Le point d'appui est le centre, le pointeur est sur le cercle d'arrivée.", "");
            choix(ui, &mut self.degrade.genre, Genre::Angle, ph::CIRCLE_HALF, "Angle", "En éventail autour du point d'appui : l'axe tracé donne la direction de départ.", "");
            choix(ui, &mut self.degrade.genre, Genre::Reflechi, ph::ARROWS_HORIZONTAL, "Réfléchi", "Le miroir de part et d'autre du point d'appui : la couleur d'arrivée est des deux côtés.", "");
            choix(ui, &mut self.degrade.genre, Genre::Losange, ph::DIAMOND, "Losange", "Des losanges autour du point d'appui : l'arrivée est une pointe du losange.", "");
        });
        // Les deux raccourcis simples : avant-plan vers arrière-plan, avant-plan vers transparent (les pastilles redeviennent liées au
        // panneau Couleur).
        ui.vertical(|ui| {
            let lie = self.degrade.couleurs.is_none() && self.degrade.transparence.is_none();
            let resp = ui.radio(lie && self.degrade.fin == Fin::VersFond, "Vers la couleur de fond");
            if resp.clicked() {
                self.degrade.fin = Fin::VersFond;
                self.degrade.couleurs = None;
                self.degrade.transparence = None;
            }
            bulle(resp, "Vers la couleur de fond", "Du carré d'avant-plan au carré d'arrière-plan du panneau Couleur (deux pastilles liées à ces carrés).", "");
            let resp = ui.radio(lie && self.degrade.fin == Fin::VersTransparent, "Vers le transparent");
            if resp.clicked() {
                self.degrade.fin = Fin::VersTransparent;
                self.degrade.couleurs = None;
                self.degrade.transparence = None;
            }
            bulle(resp, "Vers le transparent", "La couleur d'avant-plan, de plus en plus transparente (la bande d'opacité descend de 100 % à 0 %).", "");
        });
        let mut pct = (self.degrade.opacite * 100.0).round();
        if ui.add(egui::Slider::new(&mut pct, 0.0..=100.0).text("Opacité").suffix(" %").fixed_decimals(0)).changed() {
            self.degrade.opacite = (pct / 100.0).clamp(0.0, 1.0);
        }

        self.degrade_bandes_ui(ui);

        // L'angle et l'échelle de l'axe, en chiffres : ils modifient l'axe du dégradé posé.
        let (angle, echelle, actif) = match self.degrade_geste.as_ref().filter(|g| g.phase != Phase::Trace) {
            Some(g) => {
                let d = Degrade::simple(Genre::Lineaire, [g.depart.x, g.depart.y], [g.fin.x, g.fin.y], [0; 3], [0; 3], 1.0, 1.0);
                (d.angle_degres(), if g.base > 1e-3 { d.longueur() / g.base * 100.0 } else { 100.0 }, true)
            }
            None => (0.0, 100.0, false),
        };
        let (mut angle_n, mut echelle_n) = (angle.round(), echelle.round());
        ui.horizontal(|ui| {
            ui.add_enabled_ui(actif, |ui| {
                let r = ui.add(egui::DragValue::new(&mut angle_n).range(-180.0..=180.0).suffix(" °").speed(1.0));
                bulle(r, "Angle", "La direction de l'axe, en degrés (0 : vers la droite, sens des aiguilles d'une montre). Tape un chiffre : le dégradé posé suit.", "");
                let r = ui.add(egui::DragValue::new(&mut echelle_n).range(1.0..=2000.0).suffix(" %").speed(1.0));
                bulle(r, "Échelle", "La longueur de l'axe, en pour cent de celle que tu as tracée. Tape un chiffre : le dégradé posé suit.", "");
            });
            if !actif {
                ui.label(egui::RichText::new("(pose d'abord un dégradé)").small().weak());
            }
        });
        if actif && (angle_n != angle.round() || echelle_n != echelle.round()) {
            if let Some(g) = self.degrade_geste.as_mut() {
                let longueur = if echelle_n != echelle.round() { g.base.max(1.0) * echelle_n / 100.0 } else { g.depart.distance(g.fin) };
                let a = if angle_n != angle.round() { angle_n } else { (g.fin.y - g.depart.y).atan2(g.fin.x - g.depart.x).to_degrees() };
                g.fin = axe_depuis(g.depart, a, longueur);
            }
        }

        ui.horizontal(|ui| {
            let resp = ui.button("Inverser");
            if resp.clicked() {
                if self.degrade.couleurs.is_none() && self.degrade.transparence.is_none() {
                    self.degrade.inverse = !self.degrade.inverse;
                } else {
                    let (c, o) = (self.degrade.couleurs_effectives(self.color, self.couleurs.prefs.back), self.degrade.transparence_effective());
                    let (g_c, g_o) = {
                        let g = Degrade { genre: Genre::Lineaire, depart: [0.0; 2], arrivee: [1.0, 0.0], couleurs: c, opacites: o, tramage: false }.reversed();
                        (g.couleurs, g.opacites)
                    };
                    self.degrade.couleurs = Some(g_c);
                    self.degrade.transparence = Some(g_o);
                }
                self.degrade_ui.sel_couleur = None;
                self.degrade_ui.sel_opacite = None;
            }
            bulle(resp, "Inverser", "Échanger les deux bouts : le point d'appui prend la couleur d'arrivée (toutes les pastilles, couleurs et opacités, se retournent).", "");
            let resp = ui.checkbox(&mut self.degrade.tramage, "Tramage");
            bulle(resp, "Tramage", "Un très léger bruit régulier, le même à chaque rendu, qui évite les marches visibles d'un dégradé en 8 bits (à l'écran comme à l'export).", "");
        });
        ui.horizontal(|ui| {
            let resp = ui.checkbox(&mut self.degrade.tout_le_calque, "Tout le calque");
            bulle(resp, "Tout le calque", "Remplir toute la feuille au lieu de la sélection ou de la zone fermée (Alt tenu : la même chose, le temps d'un geste).", "");
        });

        self.degrade_enregistres_ui(ui);
        ui.label(egui::RichText::new("Glisser : tracer l'axe (Maj : par 15°). Ensuite l'axe reste : tire ses poignées ou change les réglages, jusqu'à Entrée, Échap ou un autre outil. Dans la sélection, sinon dans la zone fermée où l'on appuie ; Alt : tout le calque.").small().weak());
    }

    /// Les deux bandes : l'opacité (pastilles au-dessus) et les couleurs (pastilles dessous), autour d'une seule bande qui montre le
    /// dégradé ; le détail de la pastille choisie dessous.
    fn degrade_bandes_ui(&mut self, ui: &mut Ui) {
        use crate::bulles::bulle;
        use egui_phosphor::regular as ph;
        let (avant, arriere) = (self.color, self.couleurs.prefs.back);
        let largeur = (ui.available_width() - 16.0).clamp(80.0, 240.0);
        let apercu = self.degrade.degrade(avant, arriere, pos2(0.0, 0.0), pos2(1.0, 0.0));
        // Les pastilles affichées : celles du dégradé tel qu'il sera (inversion comprise), l'opacité du réglage à part.
        let mut cs = apercu.couleurs.clone();
        let o = self.degrade.opacite.clamp(0.0, 1.0);
        let mut os: Vec<PastilleOpacite> = apercu.opacites.iter().map(|p| PastilleOpacite { opacite: if o > 0.0 { (p.opacite / o).clamp(0.0, 1.0) } else { p.opacite }, ..*p }).collect();
        let (mut ch_c, mut ch_o) = (false, false);
        let id = ui.id().with("degrade-bandes");
        let (zone, _) = ui.allocate_exact_size(vec2(largeur + 16.0, 2.0 * RANGEE + BANDE), Sense::hover());
        let x0 = zone.left() + 8.0;
        let bande = Rect::from_min_size(pos2(x0, zone.top() + RANGEE), vec2(largeur, BANDE));
        self.degrade_ui.bande = Some(bande);
        peindre_bande(&ui.painter_at(zone), bande, &apercu);
        ui.painter().rect_stroke(bande, 2.0, Stroke::new(1.0_f32, crate::theme::bord()), egui::StrokeKind::Inside);
        // Opacité, au-dessus de la bande.
        let y_o = zone.top();
        ch_o |= rangee_pastilles(ui, id.with("o"), x0, largeur, y_o, true, &mut os, &mut self.degrade_ui.sel_opacite, &|p: &PastilleOpacite| Color32::from_gray((p.opacite * 255.0).round() as u8), &|t, s: &[PastilleOpacite]| PastilleOpacite::new(t, degrade::opacite_en(s, t)));
        // Couleurs, sous la bande.
        let y_c = zone.top() + RANGEE + BANDE;
        ch_c |= rangee_pastilles(ui, id.with("c"), x0, largeur, y_c, false, &mut cs, &mut self.degrade_ui.sel_couleur, &|p: &PastilleCouleur| Color32::from_rgb(p.rgb[0], p.rgb[1], p.rgb[2]), &|t, s: &[PastilleCouleur]| PastilleCouleur::new(t, degrade::couleur_en(s, t).map(|v| v.round().clamp(0.0, 255.0) as u8)));
        if ch_c || ch_o {
            // (Les deux séries deviennent les siennes : ni l'une ni l'autre ne suit plus les carrés de couleur.)
            self.degrade.inverse = false;
            self.degrade.couleurs = Some(cs.clone());
            self.degrade.transparence = Some(os.clone());
        }
        // Un double-clic sur une pastille de couleur : le sélecteur.
        if ui.ctx().input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary)) && self.degrade_ui.sel_couleur.is_some() && ui.rect_contains_pointer(Rect::from_min_size(pos2(zone.left(), y_c), vec2(zone.width(), RANGEE))) {
            self.degrade_ui.selecteur = true;
        }
        if self.degrade.couleurs.is_none() {
            ui.label(egui::RichText::new("Les deux pastilles de couleur suivent les carrés avant-plan et arrière-plan du panneau Couleur ; touche à une pastille pour les détacher.").small().weak());
        } else {
            ui.label(egui::RichText::new("Clic sous la bande : ajouter une couleur. Clic au-dessus : ajouter une opacité. Glisser : déplacer ; tirer vers le bas ou le haut : supprimer. Losange : milieu (double-clic : au centre).").small().weak());
        }
        // La pastille choisie : sa couleur (ou son opacité), sa position, la corbeille.
        let cible = match (self.degrade_ui.sel_couleur, self.degrade_ui.sel_opacite) {
            (Some(i), _) if i < cs.len() => Some((true, i)),
            (_, Some(i)) if i < os.len() => Some((false, i)),
            _ => None,
        };
        let mut supprimer = None;
        let mut modif: Option<(bool, usize, Option<[u8; 3]>, Option<f32>, f32)> = None;
        if let Some((est_couleur, i)) = cible {
            ui.horizontal(|ui| {
                let (pos, prev, suiv) = if est_couleur {
                    (cs[i].pos, if i > 0 { cs[i - 1].pos } else { 0.0 }, cs.get(i + 1).map_or(1.0, |p| p.pos))
                } else {
                    (os[i].pos, if i > 0 { os[i - 1].pos } else { 0.0 }, os.get(i + 1).map_or(1.0, |p| p.pos))
                };
                let mut rgb = if est_couleur { cs[i].rgb } else { [0; 3] };
                let mut opa = if est_couleur { 0.0 } else { (os[i].opacite * 100.0).round() };
                let mut pos_pct = (pos * 100.0).round();
                if est_couleur {
                    let r = crate::champcouleur::srgb(ui, &mut rgb);
                    bulle(r, "Couleur de la pastille", "Choisis la couleur de la pastille (ou prends-en une dans le panneau Couleur : elle va à la pastille choisie).", "");
                } else {
                    let r = ui.add(egui::DragValue::new(&mut opa).range(0.0..=100.0).suffix(" %").speed(1.0));
                    bulle(r, "Opacité de la pastille", "100 % : opaque, 0 % : transparent.", "");
                }
                let r = ui.add(egui::DragValue::new(&mut pos_pct).range((prev * 100.0).ceil()..=(suiv * 100.0).floor()).suffix(" %").speed(1.0));
                bulle(r, "Position", "Où la pastille est sur la bande, de 0 % (départ) à 100 % (arrivée).", "");
                let n = if est_couleur { cs.len() } else { os.len() };
                let r = ui.add_enabled(n > 2, egui::Button::new(egui::RichText::new(ph::TRASH).size(crate::bulles::TAILLE)));
                if r.clicked() {
                    supprimer = Some((est_couleur, i));
                }
                bulle(r, "Supprimer la pastille", "Il en faut deux au moins.", "");
                let rgb_change = est_couleur && rgb != cs[i].rgb;
                let opa_change = !est_couleur && ((opa / 100.0) - os[i].opacite).abs() > 1e-4;
                let pos_change = (pos_pct / 100.0 - pos).abs() > 1e-4;
                if rgb_change || opa_change || pos_change {
                    modif = Some((est_couleur, i, rgb_change.then_some(rgb), opa_change.then_some(opa / 100.0), pos_pct / 100.0));
                }
            });
        }
        if let Some((est_couleur, i)) = supprimer {
            if est_couleur {
                cs.remove(i);
                self.degrade_ui.sel_couleur = None;
            } else {
                os.remove(i);
                self.degrade_ui.sel_opacite = None;
            }
            self.degrade.inverse = false;
            self.degrade.couleurs = Some(cs.clone());
            self.degrade.transparence = Some(os.clone());
        } else if let Some((est_couleur, i, rgb, opa, pos)) = modif {
            if est_couleur {
                if let Some(c) = rgb {
                    cs[i].rgb = c;
                }
                cs[i].pos = pos.clamp(0.0, 1.0);
            } else {
                if let Some(a) = opa {
                    os[i].opacite = a.clamp(0.0, 1.0);
                }
                os[i].pos = pos.clamp(0.0, 1.0);
            }
            self.degrade.inverse = false;
            self.degrade.couleurs = Some(cs);
            self.degrade.transparence = Some(os);
        }
        // Le sélecteur de couleur (double-clic sur une pastille) : une fenêtre qui se ferme au bouton.
        if self.degrade_ui.selecteur {
            let Some(i) = self.degrade_ui.sel_couleur else {
                self.degrade_ui.selecteur = false;
                return;
            };
            let courante = self.degrade.couleurs_effectives(avant, arriere).get(i).map(|p| p.rgb).unwrap_or([0; 3]);
            let mut rgb = courante;
            let mut ouvert = true;
            egui::Window::new("Couleur de la pastille").collapsible(false).resizable(false).open(&mut ouvert).show(ui.ctx(), |ui| {
                crate::champcouleur::srgb_en_ligne(ui, &mut rgb);
            });
            if rgb != courante {
                self.degrade_detacher(avant);
                if let Some(p) = self.degrade.couleurs.as_mut().and_then(|c| c.get_mut(i)) {
                    p.rgb = rgb;
                }
            }
            if !ouvert {
                self.degrade_ui.selecteur = false;
            }
        }
        let _ = (MAX_PASTILLES, MILIEU_MIN);
    }

    /// La liste des dégradés à soi : des vignettes de bande, « + » pour enregistrer le dégradé courant, clic pour le rappeler, clic droit
    /// pour renommer ou supprimer. Rangée dans les réglages de l'application (pas dans le projet).
    fn degrade_enregistres_ui(&mut self, ui: &mut Ui) {
        use crate::bulles::bulle;
        use egui_phosphor::regular as ph;
        let (avant, arriere) = (self.color, self.couleurs.prefs.back);
        let liste = self.settings.degrades.clone().unwrap_or_else(degrade::enregistres_d_origine);
        let mut rappel: Option<usize> = None;
        let mut supprime: Option<usize> = None;
        let mut renomme: Option<(usize, String)> = None;
        let mut ajoute = false;
        ui.label(egui::RichText::new("Mes dégradés").strong());
        let mut vignettes = Vec::new();
        let mut plus = None;
        ui.horizontal_wrapped(|ui| {
            for (i, e) in liste.iter().enumerate() {
                let (rect, resp) = ui.allocate_exact_size(vec2(56.0, 18.0), Sense::click());
                vignettes.push(rect);
                peindre_bande(&ui.painter_at(rect), rect, &e.apercu(avant, arriere));
                ui.painter().rect_stroke(rect, 2.0, Stroke::new(1.0_f32, crate::theme::bord()), egui::StrokeKind::Inside);
                if resp.clicked() {
                    rappel = Some(i);
                }
                let nom = e.nom.clone();
                let resp = bulle(resp, &e.nom, "Clic : reprendre ce dégradé. Clic droit : renommer ou supprimer.", "");
                resp.context_menu(|ui| {
                    let mut nouveau = nom.clone();
                    ui.label("Nom");
                    if ui.text_edit_singleline(&mut nouveau).changed() {
                        renomme = Some((i, nouveau));
                    }
                    if ui.button("Supprimer").clicked() {
                        supprime = Some(i);
                        ui.close_menu();
                    }
                });
            }
            let r = ui.add(egui::Button::new(egui::RichText::new(ph::PLUS).size(crate::bulles::TAILLE)).min_size(vec2(24.0, 18.0)));
            plus = Some(r.rect);
            if r.clicked() {
                ajoute = true;
            }
            bulle(r, "Enregistrer ce dégradé", "Ajoute le dégradé de ces réglages à ta liste (il reste d'une séance à l'autre, pour tous tes projets).", "");
        });
        self.degrade_ui.vignettes = vignettes;
        self.degrade_ui.plus = plus;
        if let Some(i) = rappel {
            if let Some(e) = liste.get(i) {
                self.degrade.appliquer(e);
                self.degrade_ui.sel_couleur = None;
                self.degrade_ui.sel_opacite = None;
            }
        }
        if ajoute || supprime.is_some() || renomme.is_some() {
            let mut l = liste;
            if ajoute {
                let nom = format!("Dégradé {}", l.len() + 1);
                l.push(self.degrade.enregistrer(&nom));
            }
            if let Some(i) = supprime {
                if i < l.len() {
                    l.remove(i);
                }
            }
            if let Some((i, nom)) = renomme {
                if let Some(e) = l.get_mut(i) {
                    e.nom = nom;
                }
            }
            self.settings.degrades = Some(l);
            let _ = self.settings.save();
        }
    }
}

/// La hauteur d'une rangée de pastilles, celle de la bande.
const RANGEE: f32 = 20.0;
const BANDE: f32 = 16.0;

/// Le damier gris sous une bande : le transparent se voit.
fn damier(painter: &Painter, rect: Rect) {
    painter.rect_filled(rect, 2.0, Color32::from_gray(96));
    let case = 6.0;
    let (nx, ny) = ((rect.width() / case).ceil() as i32, (rect.height() / case).ceil() as i32);
    for j in 0..ny {
        for i in 0..nx {
            if (i + j) % 2 == 0 {
                continue;
            }
            let r = Rect::from_min_size(pos2(rect.left() + i as f32 * case, rect.top() + j as f32 * case), vec2(case, case)).intersect(rect);
            painter.rect_filled(r, 0.0, Color32::from_gray(132));
        }
    }
}

/// Une bande qui montre le dégradé `g` (de gauche à droite sur l'axe unité) sur un damier : le transparent se voit.
pub(crate) fn peindre_bande(painter: &Painter, rect: Rect, g: &Degrade) {
    damier(painter, rect);
    let n = 48usize;
    let mut ts: Vec<f32> = (0..=n).map(|i| i as f32 / n as f32).collect();
    ts.extend(g.couleurs.iter().map(|p| p.pos));
    ts.extend(g.opacites.iter().map(|p| p.pos));
    ts.sort_by(|a, b| a.total_cmp(b));
    ts.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
    let mut mesh = egui::Mesh::default();
    for &t in &ts {
        let (rgb, a) = g.couleur_a(t);
        let color = degrade::premultiplie(rgb, a);
        let x = rect.left() + rect.width() * t;
        mesh.vertices.push(egui::epaint::Vertex { pos: pos2(x, rect.top()), uv: egui::epaint::WHITE_UV, color });
        mesh.vertices.push(egui::epaint::Vertex { pos: pos2(x, rect.bottom()), uv: egui::epaint::WHITE_UV, color });
    }
    for i in 0..ts.len().saturating_sub(1) {
        let a = (2 * i) as u32;
        mesh.add_triangle(a, a + 1, a + 2);
        mesh.add_triangle(a + 1, a + 3, a + 2);
    }
    painter.add(Shape::mesh(mesh));
}

/// Une rangée de pastilles le long de la bande (`haut` : au-dessus de la bande, sinon dessous), avec les losanges de milieu entre
/// deux pastilles. Clic sur le vide : ajouter une pastille (sa valeur prise sur le dégradé à cet endroit) ; glisser : déplacer (entre
/// ses voisines) ; tirer loin de la bande : supprimer (deux au moins) ; losange : le milieu, double-clic au centre. Vrai si quelque
/// chose a changé.
#[allow(clippy::too_many_arguments)]
fn rangee_pastilles<T: Pastille>(
    ui: &mut Ui,
    id: Id,
    x0: f32,
    largeur: f32,
    y: f32,
    haut: bool,
    stops: &mut Vec<T>,
    sel: &mut Option<usize>,
    couleur: &dyn Fn(&T) -> Color32,
    nouveau: &dyn Fn(f32, &[T]) -> T,
) -> bool {
    let mut change = false;
    let rangee = Rect::from_min_size(pos2(x0 - 8.0, y), vec2(largeur + 16.0, RANGEE));
    let x_de = |p: f32| x0 + largeur * p;
    let pos_de = |x: f32| ((x - x0) / largeur).clamp(0.0, 1.0);
    // Les pastilles à 12 de haut du côté loin de la bande, les losanges à 8 de la bande.
    let (y_pastille, y_losange) = if haut { (y + 6.0, y + RANGEE - 4.0) } else { (y + RANGEE - 6.0, y + 4.0) };
    let fond = ui.interact(rangee, id.with("fond"), Sense::click());
    let mut touche = false;
    // Les losanges de milieu.
    for i in 0..stops.len().saturating_sub(1) {
        let (a, b) = (stops[i].pos(), stops[i + 1].pos());
        if b - a < 0.02 {
            continue;
        }
        let cx = x_de(a + (b - a) * stops[i].milieu());
        let r = Rect::from_center_size(pos2(cx, y_losange), vec2(10.0, 10.0));
        let resp = ui.interact(r, id.with(("m", i)), Sense::click_and_drag());
        if resp.dragged() {
            if let Some(p) = resp.interact_pointer_pos() {
                let m = ((pos_de(p.x) - a) / (b - a)).clamp(MILIEU_MIN, 1.0 - MILIEU_MIN);
                stops[i].set_milieu(m);
                change = true;
            }
        }
        if resp.double_clicked() {
            stops[i].set_milieu(0.5);
            change = true;
        }
        touche |= resp.clicked() || resp.dragged() || resp.drag_started();
        let c = pos2(cx, y_losange);
        let pts = vec![c + vec2(0.0, -4.5), c + vec2(4.5, 0.0), c + vec2(0.0, 4.5), c + vec2(-4.5, 0.0)];
        let (remplissage, trait_) = if resp.hovered() || resp.dragged() { (Color32::WHITE, Color32::BLACK) } else { (Color32::from_gray(190), Color32::from_gray(40)) };
        ui.painter().add(Shape::convex_polygon(pts, remplissage, Stroke::new(1.0_f32, trait_)));
    }
    // Les pastilles.
    let mut a_supprimer: Option<usize> = None;
    for i in 0..stops.len() {
        let c = pos2(x_de(stops[i].pos()), y_pastille);
        let r = Rect::from_center_size(c, vec2(14.0, 14.0));
        let resp = ui.interact(r, id.with(("p", i)), Sense::click_and_drag());
        if resp.clicked() || resp.drag_started() {
            *sel = Some(i);
        }
        touche |= resp.clicked() || resp.dragged() || resp.drag_started();
        if resp.dragged() {
            if let Some(p) = resp.interact_pointer_pos() {
                if (p.y - y_pastille).abs() > 34.0 && stops.len() > 2 {
                    a_supprimer = Some(i);
                } else {
                    let (prev, suiv) = (if i > 0 { stops[i - 1].pos() } else { 0.0 }, stops.get(i + 1).map_or(1.0, |q| q.pos()));
                    stops[i].set_pos(pos_de(p.x).clamp(prev, suiv));
                    change = true;
                }
            }
        }
        let choisie = *sel == Some(i);
        let carre = Rect::from_center_size(c, vec2(11.0, 11.0));
        let (bord, epaisseur) = if choisie { (Color32::WHITE, 2.0) } else { (Color32::from_gray(30), 1.0) };
        // Une petite pointe vers la bande.
        let sens = if haut { 1.0 } else { -1.0 };
        let pointe = vec![pos2(c.x - 4.0, c.y + sens * 5.5), pos2(c.x + 4.0, c.y + sens * 5.5), pos2(c.x, c.y + sens * 9.5)];
        ui.painter().add(Shape::convex_polygon(pointe, bord, Stroke::NONE));
        ui.painter().rect_filled(carre, 2.0, couleur(&stops[i]));
        ui.painter().rect_stroke(carre, 2.0, Stroke::new(epaisseur as f32, bord), egui::StrokeKind::Outside);
    }
    if let Some(i) = a_supprimer {
        stops.remove(i);
        *sel = None;
        change = true;
    } else if fond.clicked() && !touche && stops.len() < MAX_PASTILLES {
        if let Some(p) = fond.interact_pointer_pos() {
            let t = pos_de(p.x);
            let neuve = nouveau(t, stops);
            let i = degrade::inserer(stops, neuve);
            *sel = Some(i);
            change = true;
        }
    }
    change
}
