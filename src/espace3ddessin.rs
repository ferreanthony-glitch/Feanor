//! Dessiner dans la vue Espace 3D (`docs/INTERFACE.md`, partie 146). Anthony, 2026-11-12 : « il est impossible de
//! dessiner dans l'espace 3D pour l'instant. C'est quelque chose qui doit être faisable en permanence, le trait
//! s'appliquant sur la carte ».
//!
//! **L'outil décide.** Dans la vue Espace 3D (vue d'orbite ou « Regarder par la caméra »), prendre le pinceau, le
//! crayon, la gomme ou le mélangeur (leur case, leur touche, ou le bouton « Dessiner sur la carte » de la barre de la
//! vue) fait dessiner le pointeur sur la carte du calque courant ; prendre un outil de la vue (choisir, déplacer,
//! tourner, taille, caméra) lui rend les cartes. Tourner et glisser la vue restent sur Alt+glisser, le clic droit, le
//! bouton du milieu et la molette. Ctrl+clic prend la carte sous le pointeur sans quitter le dessin.
//!
//! **Un seul chemin de dessin.** Le trait passe par le moteur de toujours (`begin_paint`, `stroke_point`,
//! `end_stroke` : appui, lissage, annulation, enregistrement). Seule change la conversion de l'écran au dessin du
//! calque : le rayon du regard de la vue (`Regard3d`) coupe le plan de la carte (`CarteVue`). `App::layer_to_screen` et
//! `App::layer_zoom` passent par là tant qu'on dessine dans la vue : la pointe du trait, le segment droit (Maj) et la
//! taille du pinceau suivent.
//!
//! **Jamais d'encre qu'on ne voit pas.** Un trait ne commence, et ne reçoit de points, que là où la carte le montrera :
//! dans ses bords, vue de face (`CarteVue::sur_la_carte`, la seule règle, `carte_montre`). Ailleurs rien n'est
//! enregistré et la barre d'état dit pourquoi.
//!
//! `Regard3d` est le regard d'**une** vue (son rectangle à l'écran, sa prise de vue) : quand la vue Espace 3D en
//! montrera plusieurs côte à côte, chacune donnera le sien à `e3_dessin_pointeur`.
//!
//! **Le trait se voit pendant qu'on le trace** sans rejeu du moteur : la texture de la carte sur laquelle on dessine
//! (`CarteVive`) est comparée tuile par tuile au dessin (les tuiles sont partagées à la copie, `raster::Tile::same_as`),
//! et seule la zone des tuiles changées est refaite (`render::cel_image_zone`).

use eframe::egui::{self, pos2, vec2, Align2, Color32, CursorIcon, Event, FontId, PointerButton, Pos2, Rect, Shape, Stroke, TextureHandle, TouchPhase};
use egui_phosphor::regular as ph;

use crate::app::{App, Tool};
use crate::document::CelId;
use crate::espace3d;
use crate::espace3dmath::{self as m, Ray, View, M4, V3};
use crate::espace3dui::Drag;
use crate::raster::{Tile, TILE};

/// La commande du bouton « Dessiner sur la carte ».
pub(crate) const ACTION: &str = "espace3d.dessiner";

/// Largeur (pixels) de la texture de la carte sur laquelle on dessine : plus fine que celle des autres cartes (1 024).
const LARGEUR_VIVE: usize = 2048;
/// En deçà de ce cosinus entre le regard et la normale de la carte (deux degrés), elle est vue par la tranche.
const COS_TRANCHE: f32 = 0.035;

pub(crate) const HORS_CARTE: &str = "Rien à peindre ici : le pointeur n'est pas sur la carte du calque courant (celle qui est entourée). Dessine sur elle, ou Ctrl+clic sur une autre carte pour la prendre.";
pub(crate) const CARTE_DE_DOS: &str = "Rien à peindre ici : la carte du calque courant est vue de dos. Tourne la vue (Alt+glisser ou clic droit) pour la voir de face.";
pub(crate) const CARTE_PAR_LA_TRANCHE: &str = "Rien à peindre ici : la carte du calque courant est vue par la tranche. Tourne la vue (Alt+glisser ou clic droit).";
pub(crate) const CARTE_DERRIERE: &str = "Rien à peindre ici : la carte du calque courant est derrière l'œil de la vue. Recule ou tourne la vue.";
pub(crate) const PAS_UNE_CARTE: &str = "Ce calque n'est pas une carte de l'Espace 3D (un dossier ou un calque d'effet) : choisis un calque de dessin.";
pub(crate) const CARTES_CACHEES: &str = "Les cartes sont cachées dans cette vue (« Montrer les cartes », dans Réglages) : montre-les pour dessiner.";
pub(crate) const AUCUNE_CARTE_ICI: &str = "Ctrl+clic : aucune carte sous le pointeur.";
/// Sur la feuille (hors de la vue Espace 3D), plan en Espace 3D : les deux raisons de `App::to_layer_opt` de ne rien
/// rendre (`App::rien_a_peindre`).
pub(crate) const HORS_CARTE_FEUILLE: &str = "Rien à peindre ici : le pointeur est hors des bords de la carte du calque courant. Dans un plan en Espace 3D, une carte ne montre que sa feuille : un trait au-delà ne se verrait pas.";
pub(crate) const PAS_SOUS_LE_POINTEUR: &str = "Rien à peindre ici : la carte du calque courant n'est pas sous le pointeur (elle est derrière la caméra, ou vue par la tranche).";
pub(crate) const MANIPULE: &str = "Espace 3D : le pointeur manipule de nouveau les cartes. Le pinceau, le crayon ou la gomme (leur case ou leur touche) le font dessiner sur la carte.";

/// L'outil dessine un trait : pinceau, mélangeur, crayon, gomme.
pub(crate) fn outil_de_dessin(tool: Tool) -> bool {
    matches!(tool, Tool::Brush | Tool::Mixer | Tool::Pencil | Tool::Eraser)
}

/// Le regard d'**une** vue de l'Espace 3D : son rectangle à l'écran et sa prise de vue.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Regard3d {
    pub rect: Rect,
    pub view: View,
}

impl Regard3d {
    fn centre(&self) -> [f32; 2] {
        [self.rect.width() / 2.0, self.rect.height() / 2.0]
    }

    /// Le rayon de l'œil de la vue qui passe par ce point de l'écran.
    pub fn rayon(&self, ecran: Pos2) -> Ray {
        self.view.ray([ecran.x - self.rect.left(), ecran.y - self.rect.top()], self.centre())
    }

    /// Où ce point du monde tombe à l'écran ; rien s'il est derrière l'œil.
    pub fn ecran(&self, p: V3) -> Option<Pos2> {
        self.view.project(p, self.centre()).map(|q| pos2(self.rect.left() + q[0], self.rect.top() + q[1]))
    }
}

/// **Ce qu'une carte montre de son dessin** : la feuille, bords compris (`taille` : la feuille, en pixels). C'est le
/// rectangle que `Monde::card_warp_m` pose dans l'image (son pixel de marge est le bord adouci) et celui que la vue
/// Espace 3D texture. Hors de là, un trait serait de l'encre que la carte ne montre pas : il ne commence pas, et ses
/// points sont laissés. **La seule règle « où peut-on peindre sur une carte »** : la vue Espace 3D
/// (`CarteVue::sur_la_carte`) et la feuille d'un plan en Espace 3D (`App::to_layer_opt`) l'appellent toutes deux.
pub(crate) fn carte_montre(p: Pos2, taille: [f32; 2]) -> bool {
    p.x.is_finite() && p.y.is_finite() && p.x >= 0.0 && p.y >= 0.0 && p.x <= taille[0] && p.y <= taille[1]
}

/// Pourquoi on ne peut pas peindre à cet endroit de l'écran.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Manque {
    /// Le plan de la carte est derrière l'œil.
    Derriere,
    /// La carte est vue par la tranche.
    Tranche,
    /// La carte est vue de dos.
    Dos,
    /// Le regard touche le plan de la carte hors de ses bords : la carte ne montrerait pas ce trait.
    HorsCarte,
}

impl Manque {
    pub fn message(self) -> &'static str {
        match self {
            Manque::Derriere => CARTE_DERRIERE,
            Manque::Tranche => CARTE_PAR_LA_TRANCHE,
            Manque::Dos => CARTE_DE_DOS,
            Manque::HorsCarte => HORS_CARTE,
        }
    }
}

/// Une carte vue par une vue : de l'écran au dessin de la carte, et retour.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CarteVue {
    pub regard: Regard3d,
    /// La matrice de la carte (de son carré unité vers le monde) et son inverse.
    mx: M4,
    inv: M4,
    /// La taille du dessin de la carte (la feuille), en pixels.
    pub taille: [f32; 2],
}

impl CarteVue {
    pub fn new(regard: Regard3d, mx: M4, taille: [f32; 2]) -> CarteVue {
        CarteVue { regard, mx, inv: m::inverse_affine(&mx), taille }
    }

    /// Le point du monde où est le point `p` du dessin de la carte.
    pub fn monde(&self, p: Pos2) -> V3 {
        let [w, h] = self.taille;
        m::point(&self.mx, [p.x / w - 0.5, -(p.y / h - 0.5), 0.0])
    }

    /// Point du dessin de la carte → écran ; rien s'il est derrière l'œil de la vue.
    pub fn vers_ecran(&self, p: Pos2) -> Option<Pos2> {
        self.regard.ecran(self.monde(p))
    }

    /// Écran → point du dessin où l'on peut peindre : là où le regard qui passe par ce point de l'écran traverse la
    /// carte vue de face, **dans ses bords** (`carte_montre`). C'est par ici que passent le début d'un trait, chacun de
    /// ses points et le curseur du pinceau.
    pub fn sur_la_carte(&self, ecran: Pos2) -> Result<Pos2, Manque> {
        self.vers_dessin(ecran).and_then(|p| if carte_montre(p, self.taille) { Ok(p) } else { Err(Manque::HorsCarte) })
    }

    /// Écran → point du plan de la carte (pixels de son dessin), là où le regard qui passe par ce point de l'écran le
    /// traverse, carte vue de face. Le point peut tomber hors des bords de la carte : pour peindre, `sur_la_carte`.
    pub fn vers_dessin(&self, ecran: Pos2) -> Result<Pos2, Manque> {
        let ray = self.regard.rayon(ecran);
        // (La face de la carte regarde vers son axe Z : c'est de là que la caméra de référence la voit.)
        let normale = m::normalize(m::dir(&self.mx, [0.0, 0.0, 1.0]));
        let cos = m::dot(ray.d, normale);
        if !cos.is_finite() || cos.abs() < COS_TRANCHE {
            return Err(Manque::Tranche);
        }
        let (o, d) = ray.to_local(&self.inv);
        if d[2].abs() < 1e-9 {
            return Err(Manque::Tranche);
        }
        let t = -o[2] / d[2];
        if !(t > 0.0) {
            return Err(Manque::Derriere);
        }
        if cos > 0.0 {
            return Err(Manque::Dos);
        }
        let [w, h] = self.taille;
        let p = pos2((o[0] + d[0] * t + 0.5) * w, (0.5 - (o[1] + d[1] * t)) * h);
        // (Près de l'horizon d'une carte au sol, le point part à l'infini : ce n'est plus un point de la carte.)
        if !(p.x.is_finite() && p.y.is_finite() && p.x.abs() < 1e7 && p.y.abs() < 1e7) {
            return Err(Manque::HorsCarte);
        }
        Ok(p)
    }

    /// Pixels d'écran par pixel du dessin au point `p` de la carte (la racine de l'aire qu'un pixel y occupe).
    pub fn zoom_en(&self, p: Pos2) -> Option<f32> {
        let (o, a, b) = (self.vers_ecran(p)?, self.vers_ecran(p + vec2(1.0, 0.0))?, self.vers_ecran(p + vec2(0.0, 1.0))?);
        let (u, v) = (a - o, b - o);
        let aire = (u.x * v.y - u.y * v.x).abs();
        (aire.is_finite() && aire > 1e-12).then(|| aire.sqrt())
    }
}

/// La texture de la carte sur laquelle on dessine, tenue à jour tuile par tuile.
pub(crate) struct CarteVive {
    pub cel: CelId,
    tex: TextureHandle,
    /// Taille de la texture, et taille du dessin dont elle vient.
    taille: [usize; 2],
    dessin: [usize; 2],
    /// Les tuiles du dessin quand la texture a été mise à jour pour la dernière fois.
    tuiles: Vec<Option<Tile>>,
}

/// L'état du dessin dans la vue Espace 3D.
#[derive(Default)]
pub(crate) struct Dessin3d {
    /// Un outil de dessin a été pris dans la vue : le pointeur dessine. Sinon il manipule les cartes.
    pub dessin: bool,
    /// Le regard de la vue qui reçoit le pointeur, à la dernière image.
    pub regard: Option<Regard3d>,
    /// Le pointeur dans la vue (point de l'écran), à la dernière image ; rien hors de la vue.
    pub pointeur: Option<Pos2>,
    /// La texture de la carte sur laquelle on dessine.
    pub vive: Option<CarteVive>,
    /// Combien de fois la texture vive a été refaite en entier, et par morceau (essais et diagnostic) ; la dernière
    /// zone refaite (pixels de la texture : x0, y0, x1, y1).
    pub constructions: usize,
    pub morceaux: usize,
    pub dernier_morceau: Option<[usize; 4]>,
}

impl Dessin3d {
    /// La vue n'est plus montrée : son regard et la texture de la carte ne valent plus. (Le choix « dessiner ou
    /// manipuler » reste : on le retrouve en revenant.)
    pub(crate) fn oublier_la_vue(&mut self) {
        self.regard = None;
        self.pointeur = None;
        self.vive = None;
    }
}

impl App {
    /// La vue Espace 3D est celle que montre la zone du dessin.
    pub(crate) fn e3_vue_montree(&self) -> bool {
        self.espace3d.on && !self.camera_view && !self.in_card
    }

    /// Dans la vue Espace 3D, le pointeur **dessine** sur la carte du calque courant (sinon il manipule les cartes).
    pub(crate) fn e3_dessine(&self) -> bool {
        self.e3d.dessin && self.e3_vue_montree() && outil_de_dessin(self.tool)
    }

    /// Un outil vient d'être pris (`App::set_tool`). Dans la vue Espace 3D, c'est lui qui décide : un outil de dessin
    /// fait dessiner le pointeur, un autre lui rend les cartes.
    pub(crate) fn e3_outil_pris(&mut self, tool: Tool) {
        if !(self.espace3d.on && !self.camera_view) {
            return;
        }
        if outil_de_dessin(tool) {
            self.e3_dessiner();
        } else if self.e3d.dessin {
            self.e3_manipuler();
            self.status = MANIPULE.into();
        }
    }

    /// Le pointeur dessine : la carte du calque courant est choisie (elle est entourée, on la reconnaît).
    fn e3_dessiner(&mut self) {
        let deja = self.e3d.dessin;
        self.e3d.dessin = true;
        self.espace3d.drag = None;
        let (id, nom, carte) = (self.layer().id, self.layer().name.clone(), espace3d::is_card(self.layer()));
        if carte && self.espace3d.show_cards {
            self.espace3d.sel = vec![espace3d::card_selection(id)];
            self.espace3d.last_layer = id;
        }
        if !deja {
            self.status = if carte {
                format!("Espace 3D : {} sur la carte « {nom} » (elle est entourée). Ctrl+clic : prendre une autre carte. Alt+glisser ou clic droit : tourner la vue. Un outil de la vue rend la main aux cartes.", self.tool.label())
            } else {
                PAS_UNE_CARTE.into()
            };
        }
    }

    /// Un outil de la vue est repris : le pointeur manipule les cartes.
    pub(crate) fn e3_manipuler(&mut self) {
        if self.e3d.dessin {
            self.end_stroke();
            self.e3d.dessin = false;
            self.e3d.vive = None;
        }
    }

    /// La commande `espace3d.dessiner` (le bouton de la barre de la vue) : dessiner sur la carte, ou rendre la main aux
    /// cartes. Si l'outil courant ne dessine pas, le dernier outil de dessin pris revient (le pinceau à défaut).
    pub(crate) fn espace3d_basculer_dessin(&mut self) {
        if !(self.espace3d.on && !self.camera_view) {
            self.set_espace3d(true);
        }
        if self.e3_dessine() {
            self.e3_manipuler();
            self.status = MANIPULE.into();
            return;
        }
        let outil = if outil_de_dessin(self.tool) { self.tool } else { self.tool_recent.iter().copied().find(|t| outil_de_dessin(*t)).unwrap_or(Tool::Brush) };
        // (`set_tool` allume le dessin : `e3_outil_pris`.)
        self.set_tool(outil);
    }

    /// Le bouton « Dessiner sur la carte », en tête des outils de la vue : allumé quand le pointeur dessine.
    pub(crate) fn espace3d_bouton_dessiner(&mut self, ui: &mut egui::Ui) {
        let on = self.e3_dessine();
        let description = "Le pointeur trace sur la carte du calque courant (elle est entourée), avec le pinceau, le crayon ou la gomme : leurs cases et leurs touches l'allument aussi. Ctrl+clic : prendre une autre carte. Alt+glisser, clic droit, milieu, molette : la vue. Un outil de la vue (choisir, déplacer, tourner, taille, caméra) rend la main aux cartes.";
        if self.case_icone(ui, ph::PEN_NIB, Some(on), "Dessiner sur la carte", description, ACTION).clicked() {
            self.espace3d_basculer_dessin();
        }
    }

    /// La carte du calque courant vue par `regard` ; rien si le calque n'est pas une carte (dossier, calque d'effet).
    pub(crate) fn e3_carte_du_calque(&self, regard: Regard3d) -> Option<CarteVue> {
        let l = self.layers().get(self.cur_layer).filter(|l| espace3d::is_card(l))?;
        let world = self.e3_world();
        let f = self.cur_frame;
        // (La pose que la vue trace, `e3_paint_cards` : le trait tombe où l'on voit la carte.)
        Some(CarteVue::new(regard, world.card_xf(&l.pose(f), l.depth_at(f)).matrix(), world.stage))
    }

    /// La carte du calque courant vue par la vue où l'on dessine ; rien quand on n'y dessine pas.
    pub(crate) fn e3_carte_vue(&self) -> Option<CarteVue> {
        if !self.e3_dessine() {
            return None;
        }
        self.e3_carte_du_calque(self.e3d.regard?)
    }

    /// Pixels d'écran par pixel du dessin de la carte, sous le pointeur (au centre de la carte s'il n'y est pas) ; rien
    /// quand on ne dessine pas dans la vue (`App::layer_zoom`).
    pub(crate) fn e3_zoom_du_dessin(&self) -> Option<f32> {
        let c = self.e3_carte_vue()?;
        let centre = pos2(c.taille[0] / 2.0, c.taille[1] / 2.0);
        let ici = self.e3d.pointeur.and_then(|pos| c.vers_dessin(pos).ok()).unwrap_or(centre);
        c.zoom_en(ici).or_else(|| c.zoom_en(centre))
    }

    /// Le pointeur dans la vue Espace 3D quand il dessine : appui, trait, fin du trait, sur la carte du calque courant
    /// vue par `regard`. Alt+appui tourne la vue (`Drag::Orbit`, suivi par `e3_input`), Ctrl+clic prend la carte sous le
    /// pointeur. Les mêmes événements que le dessin à plat (`canvas_input`) : l'appui du stylet, l'instant de chacun.
    pub(crate) fn e3_dessin_pointeur(&mut self, ctx: &egui::Context, response: &egui::Response, regard: Regard3d) {
        self.e3d.regard = Some(regard);
        let (events, mods, primary_down, hover, now) = ctx.input(|i| (i.events.clone(), i.modifiers, i.pointer.primary_down(), i.pointer.hover_pos(), i.time));
        let modal = self.modal_open();
        let dans_la_vue = |pos: Pos2| regard.rect.contains(pos) && response.interact_rect.contains(pos) && ctx.layer_id_at(pos) == Some(response.layer_id);
        let pointer_events = events.iter().filter(|e| matches!(e, Event::PointerMoved(_) | Event::PointerButton { .. })).count();
        let mut k = 0;
        for event in &events {
            if matches!(event, Event::PointerMoved(_) | Event::PointerButton { .. }) {
                k += 1;
                self.input_time = crate::brush::spread_time(self.last_frame_time, now, k, pointer_events);
            }
            match event {
                Event::Touch { phase, force, .. } => {
                    if let Some(f) = force {
                        self.pen_pressure = f.clamp(0.0, 1.0);
                    }
                    if *phase == TouchPhase::Start {
                        self.pen_touch = true;
                    }
                }
                Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers, .. } => {
                    let pen = std::mem::take(&mut self.pen_touch);
                    if modal || !dans_la_vue(*pos) {
                        continue;
                    }
                    // Un appui dans la vue reprend la main au montage, comme dans le dessin à plat.
                    self.panel_focus = false;
                    self.images_focus = false;
                    self.key_focus = false;
                    self.spring_used();
                    if modifiers.alt {
                        self.end_stroke();
                        self.espace3d.drag = Some(Drag::Orbit);
                    } else if modifiers.command || modifiers.ctrl {
                        self.e3_prendre_la_carte(regard, *pos);
                    } else {
                        self.e3_debut_du_trait(regard, *pos, pen);
                    }
                }
                Event::PointerMoved(pos) => self.e3_suite_du_trait(regard, *pos, &mods),
                Event::PointerButton { button: PointerButton::Primary, pressed: false, .. } | Event::PointerGone => self.e3_fin_du_trait(),
                _ => {}
            }
        }
        self.input_time = now;
        self.last_frame_time = now;
        // Relâchement manqué (perte de focus…) : on clôt le geste quand même.
        if !primary_down {
            self.e3_fin_du_trait();
        }
        self.e3d.pointeur = hover.filter(|p| regard.rect.contains(*p));
        // Maj tenue (ou relâchée) sans que la souris bouge : le segment droit suit quand même (`tracedroit.rs`).
        let pen = self.stroke.as_ref().map(|s| s.pen).or(self.erase_pen()).unwrap_or(false);
        let sur_la_carte = if primary_down { self.e3d.pointeur.and_then(|pos| self.e3_carte_du_calque(regard)?.sur_la_carte(pos).ok()) } else { None };
        self.droit_follow_calque(&mods, primary_down, sur_la_carte, if pen { self.pen_pressure } else { 1.0 });
    }

    /// Ctrl+clic : la carte sous le pointeur devient le calque courant (on continue de dessiner, sur elle).
    fn e3_prendre_la_carte(&mut self, regard: Regard3d, pos: Pos2) {
        self.end_stroke();
        match self.e3_pick(&regard.rayon(pos)) {
            Some(id) => {
                self.espace3d.sel = vec![id];
                self.e3_follow_card(id);
                self.status = format!("Carte « {} » : c'est le calque courant, le trait y va.", self.layer().name);
            }
            None => self.status = AUCUNE_CARTE_ICI.into(),
        }
    }

    /// L'appui : le trait commence au point de la carte que le regard touche. Hors de la carte, carte de dos, par la
    /// tranche ou derrière l'œil : rien ne commence, la barre d'état dit pourquoi.
    fn e3_debut_du_trait(&mut self, regard: Regard3d, pos: Pos2, pen: bool) {
        if !self.espace3d.show_cards {
            self.status = CARTES_CACHEES.into();
            return;
        }
        let Some(carte) = self.e3_carte_du_calque(regard) else {
            // (Un dossier, un calque verrouillé… : le refus habituel ; sinon, un calque d'effet.)
            if self.layer_editable() {
                self.status = PAS_UNE_CARTE.into();
            }
            return;
        };
        // (Seulement là où la carte montrera le trait : jamais d'encre qu'on ne voit pas.)
        let p = match carte.sur_la_carte(pos) {
            Ok(p) => p,
            Err(manque) => {
                self.status = manque.message().into();
                return;
            }
        };
        // (La taille du pinceau se lit sous le pointeur : `e3_zoom_du_dessin`.)
        self.e3d.pointeur = Some(pos);
        let pressure = if pen { self.pen_pressure } else { 1.0 };
        self.begin_paint(p, pressure, pen);
        if self.stroke.is_some() || self.erase_pen().is_some() {
            // Maj tenue pendant le trait : un segment droit (`tracedroit.rs`).
            self.droit_start(p);
        }
    }

    /// Le pointeur bouge pendant un trait : un point de plus, là où le regard touche la carte. Les points où il la
    /// quitte (hors de ses bords, de dos, par la tranche) sont laissés : le trait reprend quand il y revient.
    fn e3_suite_du_trait(&mut self, regard: Regard3d, pos: Pos2, mods: &egui::Modifiers) {
        let (pen, erase) = match (self.stroke.as_ref().map(|s| s.pen), self.erase_pen()) {
            (Some(pen), _) => (pen, false),
            (None, Some(pen)) => (pen, true),
            (None, None) => return,
        };
        let Some(carte) = self.e3_carte_du_calque(regard) else { return };
        if let Ok(l) = carte.sur_la_carte(pos) {
            self.e3d.pointeur = Some(pos);
            let pressure = if pen { self.pen_pressure } else { 1.0 };
            self.stroke_point(l, pressure, mods, erase);
        }
    }

    /// Le bouton est relâché : le trait en cours est fini (une étape d'annulation, comme à plat).
    fn e3_fin_du_trait(&mut self) {
        if self.stroke.is_some() || self.verase.is_some() {
            self.end_stroke();
        }
    }

    /// Ce que le dessin ajoute à l'image de la vue : la pointe du trait et le segment droit en attente (comme à plat),
    /// le cercle du pinceau **posé sur la carte** (il a la forme et la largeur du trait qu'il va y laisser), et la
    /// légende des gestes en bas de la vue (`rect`).
    pub(crate) fn e3_dessin_peindre(&mut self, ctx: &egui::Context, painter: &egui::Painter, response: &egui::Response, rect: Rect, regard: Regard3d) {
        self.e3d.regard = Some(regard);
        self.paint_stroke_tip(painter);
        self.paint_droit(painter);
        painter.text(rect.left_bottom() + vec2(8.0, -6.0), Align2::LEFT_BOTTOM, self.e3_dessin_legende(), FontId::proportional(11.0), Color32::from_gray(190));
        let Some(pos) = response.hover_pos() else { return };
        if self.modal_open() {
            return;
        }
        let (alt, ctrl) = ctx.input(|i| (i.modifiers.alt, i.modifiers.command || i.modifiers.ctrl));
        if matches!(self.espace3d.drag, Some(Drag::Orbit)) {
            ctx.set_cursor_icon(CursorIcon::Grabbing);
            return;
        }
        let en_trait = self.stroke.is_some() || self.verase.is_some();
        if !en_trait && alt {
            ctx.set_cursor_icon(CursorIcon::Grab);
            return;
        }
        if !en_trait && ctrl {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
            return;
        }
        let Some(brush) = self.current_brush() else { return };
        let touche = self.e3_carte_du_calque(regard).filter(|_| self.espace3d.show_cards).and_then(|c| c.sur_la_carte(pos).ok().map(|p| (c, p)));
        let Some((carte, p)) = touche else {
            ctx.set_cursor_icon(CursorIcon::NotAllowed);
            return;
        };
        // La croix du système suit le stylet sans retard ; le cercle, dessiné dans l'image, arrive une image plus tard.
        let system = self.settings.system_cursor;
        ctx.set_cursor_icon(if system { CursorIcon::Crosshair } else { CursorIcon::None });
        let r = (brush.size * 0.5).max(0.5);
        const N: usize = 40;
        let bord: Vec<Pos2> = (0..N)
            .filter_map(|k| {
                let a = k as f32 / N as f32 * std::f32::consts::TAU;
                carte.vers_ecran(p + vec2(a.cos(), a.sin()) * r)
            })
            .collect();
        let petit = bord.iter().all(|q| q.distance(pos) < 1.5);
        if bord.len() == N && !petit {
            painter.add(Shape::closed_line(bord.clone(), Stroke::new(3.0_f32, Color32::from_white_alpha(110))));
            painter.add(Shape::closed_line(bord, Stroke::new(1.0_f32, Color32::from_black_alpha(190))));
        } else {
            painter.circle_stroke(pos, 1.5, Stroke::new(1.0_f32, Color32::from_black_alpha(170)));
            painter.circle_stroke(pos, 2.5, Stroke::new(1.0_f32, Color32::from_white_alpha(150)));
        }
        if !system {
            let cross = Stroke::new(1.0_f32, Color32::from_gray(230));
            painter.line_segment([pos - vec2(4.0, 0.0), pos + vec2(4.0, 0.0)], cross);
            painter.line_segment([pos - vec2(0.0, 4.0), pos + vec2(0.0, 4.0)], cross);
        }
    }

    /// La légende du bas de la vue quand le pointeur dessine.
    pub(crate) fn e3_dessin_legende(&self) -> String {
        let outil = self.tool.label();
        let carte = if espace3d::is_card(self.layer()) { format!("la carte « {} »", self.layer().name) } else { "ce calque (pas une carte)".to_string() };
        format!("{outil} sur {carte} · Ctrl+clic : prendre une autre carte · Alt+glisser ou clic droit : tourner la vue · milieu : glisser la vue · molette : avancer · outils de la vue : manipuler")
    }

    /// La texture de la carte sur laquelle on dessine (le dessin `cel` : celui du calque courant à l'image courante, ou
    /// celui du trait en cours), tenue à jour à chaque image sans rejeu du moteur : seules les tuiles que le trait
    /// vient de changer sont refaites. Rien pour un autre dessin, ou quand on ne dessine pas dans la vue.
    pub(crate) fn e3_texture_vive(&mut self, ctx: &egui::Context, cel: CelId) -> Option<egui::TextureId> {
        if !self.e3_dessine() {
            return None;
        }
        let en_cours = self.stroke.as_ref().map(|s| s.cel).or(self.verase.as_ref().map(|v| v.cel));
        let courant = self.layers().get(self.cur_layer).and_then(|l| l.shown_at(self.cur_frame));
        if en_cours != Some(cel) && courant != Some(cel) {
            return None;
        }
        let raster = &self.doc.cels.get(&cel)?.raster;
        let taille = crate::render::cel_image_size(raster, LARGEUR_VIVE);
        let dessin = [raster.width, raster.height];
        let d = &mut self.e3d;
        if let Some(v) = d.vive.as_mut().filter(|v| v.cel == cel && v.taille == taille && v.dessin == dessin && v.tuiles.len() == raster.tiles.len()) {
            // Les tuiles qui ne sont plus les mêmes qu'à la dernière image : la zone à refaire (pixels du dessin).
            let mut zone: Option<[usize; 4]> = None;
            for (i, (a, b)) in raster.tiles.iter().zip(v.tuiles.iter_mut()).enumerate() {
                let meme = match (a, &*b) {
                    (None, None) => true,
                    (Some(x), Some(y)) => x.same_as(y),
                    _ => false,
                };
                if meme {
                    continue;
                }
                *b = a.clone();
                let (x0, y0) = ((i % raster.tiles_x.max(1)) * TILE, (i / raster.tiles_x.max(1)) * TILE);
                zone = Some(match zone {
                    None => [x0, y0, x0 + TILE, y0 + TILE],
                    Some(z) => [z[0].min(x0), z[1].min(y0), z[2].max(x0 + TILE), z[3].max(y0 + TILE)],
                });
            }
            if let Some(z) = zone {
                // (Deux pixels de texture de plus de chaque côté : chacun mêle ses voisins du dessin.)
                let (sx, sy) = (dessin[0].max(1) as f32 / taille[0] as f32, dessin[1].max(1) as f32 / taille[1] as f32);
                let t = [
                    ((z[0] as f32 / sx).floor() as usize).saturating_sub(2),
                    ((z[1] as f32 / sy).floor() as usize).saturating_sub(2),
                    ((z[2] as f32 / sx).ceil() as usize + 2).min(taille[0]),
                    ((z[3] as f32 / sy).ceil() as usize + 2).min(taille[1]),
                ];
                let image = crate::render::cel_image_zone(raster, LARGEUR_VIVE, t);
                if image.size[0] > 0 && image.size[1] > 0 {
                    v.tex.set_partial([t[0], t[1]], image, egui::TextureOptions::LINEAR);
                    d.morceaux += 1;
                    d.dernier_morceau = Some(t);
                }
            }
            return d.vive.as_ref().map(|v| v.tex.id());
        }
        let tex = ctx.load_texture(format!("espace 3d, carte dessinée {cel}"), crate::render::cel_image(raster, LARGEUR_VIVE), egui::TextureOptions::LINEAR);
        let id = tex.id();
        d.vive = Some(CarteVive { cel, tex, taille, dessin, tuiles: raster.tiles.clone() });
        d.constructions += 1;
        Some(id)
    }
}
