//! L'Espace 3D dans l'application (`docs/ESPACE3D.md`) : le mode « Espace 3D » (une vue d'édition à la place du dessin,
//! où les calques du plan sont des cartes qu'on déplace, tourne et met à l'échelle, avec la caméra du plan), ses
//! commandes, la barre d'outils, les réglages de la vue (montrés par le panneau « Espace 3D », `espace3dpanneau.rs`) et
//! l'annulation.
//!
//! Depuis la simplification voulue par Anthony (2026-10-21), il n'y a plus d'objets 3D : l'espace ne montre que les
//! cartes et la caméra. La vue elle-même (souris, manipulateurs, tracé) est dans `espace3dview.rs`.

use std::collections::HashMap;

use eframe::egui::{self, RichText, TextureHandle};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::camera::{focal_label, Camera, FOCAL_PRESETS};
use crate::document::{CelId, Layer, LayerTransform};
use crate::espace3d::{self, Monde, SceneCam, Xf, MAX_FOCAL, MAX_TILT, MIN_FOCAL};
use crate::espace3dgizmo::{self as gz, Gizmo, Handle, Mode as GizmoMode, Originals, CAMERA_ID};
use crate::espace3dmath::{Orbite, Ray, V3};
use crate::history::Edit;

/// L'espace d'un filet entre deux sections du panneau Espace 3D (partie 161.4 : 2 points au lieu des 6 d'egui, pour que la
/// caméra entière se voie dans la colonne de 290 points).
const SEPARATEUR: f32 = 2.0;

/// Un geste en cours dans la vue.
pub(crate) enum Drag {
    /// Tourner autour de l'espace (la vue d'édition, pas une carte).
    Orbit,
    /// Une poignée (ou une carte tirée directement) : les placements d'origine, et de quoi les refaire.
    Handle {
        handle: Handle,
        mode: GizmoMode,
        gizmo: Gizmo,
        start: Ray,
        start_px: [f32; 2],
        originals: Originals,
        snap: Edit,
        changed: bool,
        name: &'static str,
    },
}

/// État de l'Espace 3D dans l'application.
pub(crate) struct Espace3dState {
    pub on: bool,
    pub mode: GizmoMode,
    /// Manipulateur dans le repère de la carte (une seule carte choisie).
    pub local: bool,
    /// Ce qui est choisi : des cartes (`espace3d::card_selection`), la caméra du plan (`CAMERA_ID`).
    pub sel: Vec<u64>,
    /// Vue d'édition.
    pub cam: Option<Orbite>,
    /// « Vue par la caméra » : la vue est celle de la caméra du plan, au format du cadre.
    pub through_camera: bool,
    /// Les cartes (les calques du plan) sont montrées dans la vue.
    pub show_cards: bool,
    /// Textures des dessins des cartes, par dessin et version (refaites quand le dessin change).
    pub card_textures: HashMap<CelId, (u64, TextureHandle)>,
    /// Le calque courant à la dernière image : le choix des cartes suit le panneau Calques quand il change.
    pub last_layer: u64,
    pub show_grid: bool,
    pub snap: bool,
    pub step_move: f32,
    pub step_rot: f32,
    pub step_scale: f32,
    /// Distance (mètres) où poser une surface neuve dans un plan en Espace 3D ; 0 : la distance de référence de la caméra.
    pub surface_distance: f32,
    pub drag: Option<Drag>,
    pub hover: Option<Handle>,
    /// Une, deux ou quatre vues, et leurs points de vue (`espace3dvues.rs`).
    pub vues: crate::espace3dvues::Vues,
    /// Plan de la dernière image (la sélection ne passe pas d'un plan à l'autre).
    pub last_panel: u64,
}

impl Default for Espace3dState {
    fn default() -> Espace3dState {
        Espace3dState {
            on: false,
            mode: GizmoMode::Move,
            local: false,
            sel: Vec::new(),
            cam: None,
            through_camera: false,
            show_cards: true,
            card_textures: HashMap::new(),
            last_layer: 0,
            show_grid: true,
            snap: true,
            step_move: 0.25,
            step_rot: 15.0,
            step_scale: 0.1,
            surface_distance: 0.0,
            drag: None,
            hover: None,
            vues: Default::default(),
            last_panel: 0,
        }
    }
}

fn mode_icon(mode: GizmoMode) -> &'static str {
    match mode {
        GizmoMode::Select => ph::CURSOR,
        GizmoMode::Move => ph::ARROWS_OUT_CARDINAL,
        GizmoMode::Rotate => ph::ARROWS_CLOCKWISE,
        GizmoMode::Scale => ph::ARROWS_OUT_SIMPLE,
    }
}

pub(crate) fn mode_description(mode: GizmoMode) -> &'static str {
    match mode {
        GizmoMode::Select => "Cliquer une carte ou la caméra pour la choisir (Maj : en ajouter). Glisser une carte : la déplacer dans son plan (Maj : en profondeur).",
        GizmoMode::Move => "Flèches : déplacer le long d'un axe ; carrés : dans un plan.",
        GizmoMode::Rotate => "Cercles : tourner autour d'un axe (X et Y inclinent la carte).",
        GizmoMode::Scale => "Cubes : étirer le long d'un axe ; cube du centre : tout ensemble.",
    }
}

impl App {
    /// Nouveau projet, ou feuille changée : la sélection et la vue sont oubliées.
    pub(crate) fn espace3d_reset(&mut self) {
        let s = &mut self.espace3d;
        s.sel.clear();
        s.cam = None;
        s.drag = None;
        s.last_panel = 0;
        s.last_layer = 0;
        s.card_textures.clear();
        // (Les vues retenues par le projet ; un projet qui n'en dit rien s'ouvre en une vue.)
        self.espace3d.vues = crate::espace3dvues::Vues::depuis(self.doc.vues_3d.as_ref());
        // (Le maillage montré ou non : ce que le projet retient, `espace3ddecor.rs` ; un projet qui n'en dit rien le montre.)
        self.espace3d.show_grid = self.e3_environnement().maillage;
    }

    /// Entre dans l'Espace 3D, ou en sort.
    pub(crate) fn set_espace3d(&mut self, on: bool) {
        if on {
            self.set_camera_view(false);
            self.end_stroke();
            self.playing = false;
            // (À l'entrée, rien n'est choisi : seul un changement de calque choisit sa carte.)
            if !self.espace3d.on {
                self.espace3d.last_layer = self.layer().id;
            }
        }
        self.espace3d.on = on;
        self.espace3d.drag = None;
        // (Le regard de la vue et la texture de la carte qu'on y dessine ne valent que pour la vue montrée, `espace3ddessin.rs`.)
        self.e3d.oublier_la_vue();
        self.status = if on {
            "Espace 3D : clic sur une carte (un calque) ou sur la caméra pour la choisir, glisser pour la déplacer, clic droit pour tourner la vue, bouton du milieu pour la glisser, molette pour avancer. Réglages de la vue : le panneau Espace 3D.".into()
        } else {
            // (Hors de la vue de la scène, rien ne dit la 3D, partie 159.)
            "Retour au dessin.".into()
        };
    }

    /// Les dimensions du monde de ce projet, pour le plan courant : dans un plan en Espace 3D, la profondeur des cartes
    /// est libre (`Monde::libre`, partie 148).
    pub(crate) fn e3_world(&self) -> Monde {
        Monde::du_plan(&self.doc, self.cur_panel)
    }

    /// Le rang dans le plan courant du calque dont la carte porte ce numéro de sélection.
    pub(crate) fn e3_card_layer(&self, selection: u64) -> Option<usize> {
        let id = espace3d::card_of(selection)?;
        self.layers().iter().position(|l| l.id == id && espace3d::is_card(l))
    }

    /// Les rangs des cartes visibles du plan courant (ni cachées, ni dans un dossier caché), du fond vers le dessus :
    /// l'ordre dans lequel l'image les compose.
    pub(crate) fn e3_visible_cards(&self) -> Vec<usize> {
        let layers = self.layers();
        (0..layers.len()).filter(|&i| espace3d::is_card(&layers[i]) && crate::folders::effect(layers, i).is_some()).collect()
    }

    /// Fait `f` comme une étape d'annulation nommée `name`.
    pub(crate) fn espace3d_step<R>(&mut self, name: &str, f: impl FnOnce(&mut Self) -> R) -> R {
        self.end_stroke();
        let snap = self.snapshot();
        let r = f(self);
        let before = self.history.context.replace(name.to_string());
        self.history.push(snap);
        self.history.context = before;
        self.modified = true;
        r
    }

    /// Les placements de ce qui est choisi (`ids`) : les cartes et la caméra du plan.
    pub(crate) fn e3_xfs(&self, ids: &[u64]) -> Originals {
        let world = self.e3_world();
        let frame = self.cur_frame;
        ids.iter()
            .filter_map(|&id| {
                if id == CAMERA_ID {
                    return Some((id, world.camera_xf(&self.current_camera())));
                }
                let layer = &self.layers()[self.e3_card_layer(id)?];
                Some((id, world.carte(layer, frame)))
            })
            .collect()
    }

    /// Pose ces placements (sans étape d'annulation : l'appelant s'en charge).
    pub(crate) fn e3_set_xfs(&mut self, new: &[(u64, Xf)]) {
        let world = self.e3_world();
        let frame = self.cur_frame;
        for (id, xf) in new {
            if *id == CAMERA_ID {
                // La caméra du plan : réglée à l'image courante, ou sa clé de cette image si elle est animée.
                // (Telle quelle : la caméra de la vue de la scène est libre, partie 159.)
                let cam = world.camera_from_xf(&self.current_camera(), xf);
                self.set_current_camera_libre(cam);
            } else if let Some(li) = self.e3_card_layer(*id).filter(|&li| !self.layers()[li].locked && !self.layers()[li].position_locked) {
                // (Un calque verrouillé, ou de position verrouillée, ne bouge pas, même avec les poignées d'un groupe.)
                // (Tout plan est dans l'espace, partie 159 : l'inclinaison est libre, la carte se couche au sol ou fait un
                // demi-tour, passe la caméra d'origine.)
                world.apply_card_xf_libre(&mut self.layers_mut()[li], frame, xf);
                // (Une carte qui suit une trajectoire dans l'espace : sa pose propre prend le geste, son chemin la suit,
                // `trajectoire3d.rs`, partie 149.)
                self.trajectoire_espace_apres_pose(li);
            }
        }
        self.modified = true;
    }

    /// Cadre la vue d'édition sur tout l'espace : les cartes et la caméra du plan.
    pub(crate) fn espace3d_fit(&mut self) {
        let world = self.e3_world();
        let mut points: Vec<V3> = Vec::new();
        if self.espace3d.show_cards {
            for li in self.e3_visible_cards() {
                let layer = &self.layers()[li];
                points.extend(world.card_corners(&world.carte(layer, self.cur_frame)));
            }
        }
        points.push(world.eye(&self.current_camera()));
        let bounds = crate::espace3dmath::bounds_of(points).filter(|b| crate::espace3dmath::length(crate::espace3dmath::sub(b.1, b.0)) > 0.1).unwrap_or(([-3.0, 0.0, -3.0], [3.0, 2.0, 3.0]));
        let rect = self.canvas_rect;
        let mut cam = Orbite::fit_at(bounds, [rect.width().max(1.0), rect.height().max(1.0)], 30.0, 22.0);
        cam.distance = cam.distance.max(2.0);
        self.espace3d.cam = Some(cam);
        // (Avec plusieurs vues, chacune se recadre aussi.)
        if self.e3_vues_multiples() {
            self.e3_vues_cadrer(false);
        }
    }

    /// Met les cartes dans l'ordre de la profondeur : du plus loin (en bas de la pile) au plus près. Une étape
    /// d'annulation. Refusé si la pile a des dossiers, des calques d'effet, de couleur ou de toute la scène : leur
    /// ordre a un sens que la profondeur ne connaît pas.
    pub(crate) fn espace3d_order_by_depth(&mut self) {
        let frame = self.cur_frame;
        let layers = self.layers();
        let plain = layers.iter().all(|l| espace3d::is_card(l) && l.parent.is_none() && l.couleur.is_none() && l.scene_wide.is_none());
        if !plain {
            self.status = "Ordre par profondeur refusé : la pile a des dossiers ou des calques liés (effet, couleur, toute la scène). Range-les à la main.".into();
            return;
        }
        let mut order: Vec<usize> = (0..layers.len()).collect();
        // (Stable : deux calques à la même profondeur gardent leur ordre.)
        let world = self.e3_world();
        order.sort_by(|&a, &b| world.profondeur(&layers[b], frame).total_cmp(&world.profondeur(&layers[a], frame)));
        if order.iter().enumerate().all(|(i, &k)| i == k) {
            self.status = "Les cartes sont déjà dans l'ordre de la profondeur.".into();
            return;
        }
        let current = layers[self.cur_layer].id;
        self.espace3d_step("Ordonner la pile selon la profondeur", |s| {
            let old = std::mem::take(s.layers_mut());
            *s.layers_mut() = order.iter().map(|&k| old[k].clone()).collect();
            s.cur_layer = s.layers().iter().position(|l| l.id == current).unwrap_or(0);
        });
        self.status = "Les cartes sont dans l'ordre de la profondeur : le plus loin en bas de la pile.".into();
    }

    /// **Ce qui est devant cache ce qui est derrière** (`Panel::ordre_distance`, partie 159), oui ou non, pour le plan courant :
    /// un réglage de la vue de la scène, en une étape d'annulation. Allumé, les calques se posent du plus loin au plus près de
    /// la caméra ; éteint, dans l'ordre de la pile, comme tout plan dessiné à plat (d'origine). L'image change si la pile et les
    /// distances ne disent pas la même chose : c'est le but, et Ctrl+Z le rend.
    ///
    /// (Un plan venu du format 55 en Espace 3D, `Panel::espace`, avait toujours l'ordre des distances. L'éteindre en fait un
    /// plan du format 56 dont la caméra est libre (`camera_libre`) : il garde ce qu'il sait faire, sauf cet ordre.)
    pub(crate) fn espace3d_basculer_ordre(&mut self) {
        let pi = self.cur_panel;
        let on = !self.doc.ordre_par_la_distance(pi);
        self.espace3d_step("Devant cache derrière", |s| {
            let p = &mut s.doc.panels[pi];
            if !on && p.espace {
                p.espace = false;
                p.camera_libre = true;
            }
            p.ordre_distance = on;
        });
        self.renderer.invalidate_all();
        self.status = if on {
            "Devant cache derrière : allumé pour ce plan. Ses calques se posent du plus loin au plus près de la caméra.".into()
        } else {
            "Devant cache derrière : éteint pour ce plan. Ses calques se posent dans l'ordre de la pile.".into()
        };
    }
    /// La distance où une surface neuve se pose (mètres) : celle du panneau Espace 3D, sinon la distance de référence.
    pub(crate) fn espace3d_distance_surface(&self) -> f32 {
        let d = self.espace3d.surface_distance;
        if d.is_finite() && d > 0.0 { d } else { self.e3_world().d0 }
    }

    /// **Pose une surface à dessiner** dans le plan en Espace 3D courant (`docs/ESPACE-LIBRE.md`, étape A6) : un calque
    /// neuf, juste au-dessus du calque courant, du même genre que lui (vectoriel ou bitmap), posé face à la caméra, au sol
    /// ou en mur (`Monde::surface_xf`) ; il devient le calque courant et la carte choisie. Une étape d'annulation.
    /// Refusé hors d'un plan en Espace 3D, storyboard verrouillé, ou si la surface tomberait hors de l'espace où une carte
    /// peut être. Elle se pose aussi derrière la caméra d'origine du plan (partie 148) ; si la carte ne peut pas être aussi
    /// grande que demandé, la barre d'état le dit.
    pub(crate) fn espace3d_nouvelle_surface(&mut self, sorte: espace3d::Surface) {
        // (Un calque de la scène se pose dans la vue de la scène : demandé d'ailleurs, il l'ouvre d'abord, partie 159.)
        if !self.e3_vue_montree() {
            self.set_espace3d(true);
        }
        let distance = self.espace3d_distance_surface();
        let Some((nom, plus_petite, pile)) = self.espace3d_poser_surface(sorte, sorte.nom(), None) else {
            if self.status.is_empty() {
                self.status = format!("{} refusée : à {distance:.1} m devant cette caméra, elle tomberait trop loin de la feuille pour être une carte. Rapproche la caméra de la feuille.", sorte.nom());
            }
            return;
        };
        self.status = if plus_petite {
            format!("« {nom} » posée à {distance:.1} m, mais plus petite que le cadre : près de la caméra d'origine du plan et derrière elle, une carte ne dépasse pas dix fois la taille de la feuille. Pose-la moins loin de ta caméra pour qu'elle remplisse le cadre.")
        } else {
            format!("« {nom} » posée à {distance:.1} m : c'est le calque courant, dessine dessus.")
        };
        if pile {
            self.status = format!("{} {}", self.status, crate::espace3ddirect::PILE_GARDEE);
        }
    }

    /// Le travail de `espace3d_nouvelle_surface`, partagé avec les calques 3D du menu « Nouveau calque »
    /// (`espace3ddecor.rs`, partie 157) : un calque neuf nommé `nom_base` (puis « `nom_base` 2 »…), posé par
    /// `Monde::surface_xf`, marqué `calque_3d`, juste au-dessus du calque courant ; une seule étape d'annulation. Rend
    /// son nom, s'il est plus petit que demandé, et si le plan garde l'ordre de sa pile (`e3_ordre_pour_un_calque_de_la_scene`) ;
    /// rien s'il est refusé (la barre d'état est vidée, ou dit pourquoi). Sur tout plan (partie 159).
    pub(crate) fn espace3d_poser_surface(&mut self, sorte: espace3d::Surface, nom_base: &str, calque_3d: Option<crate::espace3ddecor::Calque3d>) -> Option<(String, bool, bool)> {
        self.status.clear();
        if self.content_refuses() {
            return None;
        }
        self.end_stroke();
        let snap = self.snapshot();
        let world = self.e3_world();
        let distance = self.espace3d_distance_surface();
        let Some(xf) = world.surface_xf(&self.current_camera(), sorte, distance) else {
            return None;
        };
        let frame = self.cur_frame;
        let kind = match self.layers().get(self.cur_layer) {
            Some(l) if !l.folder && l.effet.is_none() => l.kind,
            _ => crate::document::LayerKind::Vector,
        };
        let deja = self.layers().iter().filter(|l| l.name.starts_with(nom_base)).count();
        let nom = if deja == 0 { nom_base.to_string() } else { format!("{nom_base} {}", deja + 1) };
        let mut layer = crate::document::Layer::new(nom.clone(), kind);
        layer.parent = self.layers().get(self.cur_layer).and_then(|l| if l.folder { None } else { l.parent });
        layer.calque_3d = calque_3d;
        world.apply_card_xf_libre(&mut layer, frame, &xf);
        // (Ce qui est devant cache ce qui est derrière, si la pile du plan le permet sans changer son image : dans l'étape
        // de ce calque, partie 159.)
        let pile = self.e3_ordre_pour_un_calque_de_la_scene();
        // (Près de la caméra d'origine du plan et derrière elle, une carte ne dépasse pas dix fois la feuille : son échelle
        // s'arrête à 100, et sous `Layer::MIN_DEPTH` sa taille ne grandit plus avec sa profondeur. Une surface demandée
        // plus grande est posée plus petite : on le dit.)
        let posee = world.carte(&layer, frame);
        let plus_petite = posee.size[0].abs() < xf.size[0].abs() * 0.99;
        let id = layer.id;
        let at = (self.cur_layer + 1).min(self.layers().len());
        self.layers_mut().insert(at, layer);
        self.cur_layer = at;
        self.vsel = None;
        let before = self.history.context.replace(nom_base.to_string());
        self.commit_structure(snap);
        self.history.context = before;
        self.espace3d.sel = vec![espace3d::card_selection(id)];
        self.espace3d.last_layer = id;
        Some((nom, plus_petite, pile))
    }

    /// Les commandes de l'Espace 3D. Vrai si l'action est la sienne.
    pub(crate) fn espace3d_action(&mut self, action: &str) -> bool {
        // Les anciens noms (plateau en 3D, scène 3D) mènent à l'original (`keymap::ALIASES`).
        let action = crate::keymap::ALIASES.iter().find(|(alias, original)| *alias == action && original.starts_with("espace3d.")).map_or(action, |(_, original)| *original);
        if !action.starts_with("espace3d.") {
            return false;
        }
        // Les vues (une, deux, quatre) : `espace3dvues.rs`.
        if self.e3_vues_action(action) {
            return true;
        }
        match action {
            "espace3d.basculer" => self.set_espace3d(!(self.espace3d.on && !self.camera_view)),
            "espace3d.outil_choisir" => self.espace3d_mode(GizmoMode::Select),
            "espace3d.outil_deplacer" => self.espace3d_mode(GizmoMode::Move),
            "espace3d.outil_tourner" => self.espace3d_mode(GizmoMode::Rotate),
            "espace3d.outil_echelle" => self.espace3d_mode(GizmoMode::Scale),
            "espace3d.outil_camera" => self.espace3d_pick_camera(false),
            "espace3d.vue_camera" => {
                self.set_espace3d(true);
                self.espace3d.through_camera = !self.espace3d.through_camera;
            }
            "espace3d.cadrer" => {
                self.set_espace3d(true);
                self.espace3d_fit();
            }
            "espace3d.ordonner_profondeur" => self.espace3d_order_by_depth(),
            "espace3d.ordre_distance" => self.espace3d_basculer_ordre(),
            "espace3d.surface_face" => self.espace3d_nouvelle_surface(espace3d::Surface::Face),
            "espace3d.surface_sol" => self.espace3d_nouvelle_surface(espace3d::Surface::Sol),
            "espace3d.surface_mur" => self.espace3d_nouvelle_surface(espace3d::Surface::Mur),
            "espace3d.aimanter" => {
                self.espace3d.snap = !self.espace3d.snap;
                self.status = if self.espace3d.snap { "Aimantation à la grille : allumée.".into() } else { "Aimantation : éteinte.".into() };
            }
            // (Le projet retient le maillage montré ou non : `espace3ddecor.rs`, partie 157.)
            "espace3d.grille" => self.e3_maillage_basculer(),
            // Dessiner sur la carte, ou rendre la main aux cartes (`espace3ddessin.rs`, partie 146).
            crate::espace3ddessin::ACTION => self.espace3d_basculer_dessin(),
            _ => return false,
        }
        true
    }

    fn espace3d_mode(&mut self, mode: GizmoMode) {
        if !self.espace3d.on {
            self.set_espace3d(true);
        }
        self.espace3d.mode = mode;
        // (Un outil de la vue : le pointeur manipule, il ne dessine plus.)
        self.e3_manipuler();
    }

    // ------------------------------------------------------------------------- la caméra choisie seule (partie 152)

    /// **La caméra du plan est choisie seule** dans la vue Espace 3D : elle est choisie, et aucune carte ne l'est avec
    /// elle. Anthony, 2026-11-13 : « La partie dédiée à la caméra dans la timeline doit être sélectionnée quand la caméra
    /// l'est, et plus un calque. » Tant que c'est le cas, la timeline et le montage montrent la piste « Caméra » comme la
    /// rangée choisie, et aucune rangée de calque (`calque_montre_courant`).
    pub(crate) fn e3_camera_seule(&self) -> bool {
        self.e3_vue_montree() && self.espace3d.sel.contains(&CAMERA_ID) && !self.espace3d.sel.iter().any(|&s| self.e3_card_layer(s).is_some())
    }

    /// Le calque `li` est celui que les rangées de la timeline et du montage montrent comme le calque courant : le calque
    /// courant, sauf quand la caméra du plan est choisie seule (c'est alors la piste « Caméra » qui est montrée choisie).
    /// Le calque courant reste le calque courant pour tout le reste (le panneau Calques, le dessin).
    pub(crate) fn calque_montre_courant(&self, li: usize) -> bool {
        li == self.cur_layer && !self.e3_camera_seule()
    }

    /// Choisit la caméra du plan. **Seule** : les cartes sont lâchées, et les gestes de la Trajectoire n'agissent plus que
    /// sur son chemin (Anthony, 2026-11-13 : « Éditer la trajectoire de la caméra ne fonctionne pas puisqu'un calque est
    /// toujours sélectionné en même temps »). `ajouter` (Maj tenue) : elle rejoint ce qui est choisi, ou le quitte si elle
    /// y était.
    pub(crate) fn e3_choisir_la_camera(&mut self, ajouter: bool) {
        // (Ce choix est le dernier mot : un changement de calque que la vue n'a pas encore vu, `espace3d_panel`, ne le
        // remplace pas par la carte de ce calque à l'image d'écran suivante.)
        self.espace3d.last_layer = self.layer().id;
        if !ajouter {
            self.espace3d.sel = vec![CAMERA_ID];
        } else if let Some(k) = self.espace3d.sel.iter().position(|s| *s == CAMERA_ID) {
            self.espace3d.sel.remove(k);
        } else {
            self.espace3d.sel.push(CAMERA_ID);
        }
    }

    /// **Un clic sur la piste « Caméra »** de la timeline ou du montage (son nom, le vide de sa rangée, une de ses clés, une
    /// poignée de lissage) : la caméra du plan est choisie seule, comme « choisir » du panneau Espace 3D
    /// (`e3_choisir_la_camera`), et sa piste devient la rangée choisie (Anthony, 2026-11-13 : « La partie dédiée à la caméra
    /// dans la timeline doit être sélectionnée quand la caméra l'est, et plus un calque »). Seulement quand la vue Espace 3D
    /// est montrée, dans tout plan (en Espace 3D ou non) : c'est pour elle que la caméra se choisit. Vue fermée (ou cachée par
    /// la vue caméra), rien de plus ne change : le clic fait ce qu'il faisait (la tête de lecture, les clés, les menus).
    pub(crate) fn e3_piste_camera_cliquee(&mut self) {
        if self.e3_vue_montree() {
            self.e3_choisir_la_camera(false);
        }
    }

    /// Le calque `li` vient d'être choisi à la main (`App::select_layer` : le panneau Calques, la timeline, le montage…),
    /// même s'il était déjà le calque courant : **la caméra du plan est lâchée**, et la carte de ce calque est choisie à sa
    /// place (vue ouverte, cartes montrées). La timeline et le montage rendent alors leur rangée à ce calque. Sans caméra
    /// choisie, rien ne change (le choix des cartes suit le calque courant comme avant, `espace3d_panel`).
    pub(crate) fn e3_calque_choisi(&mut self, li: usize) {
        if !self.espace3d.sel.contains(&CAMERA_ID) {
            return;
        }
        let carte = self.layers().get(li).filter(|l| espace3d::is_card(l)).map(|l| espace3d::card_selection(l.id));
        match carte.filter(|_| self.espace3d.on && self.espace3d.show_cards) {
            Some(c) => self.espace3d.sel = vec![c],
            None => self.espace3d.sel.retain(|s| *s != CAMERA_ID),
        }
    }

    /// Choisit la caméra du plan (pour la déplacer ou la tourner), avec l'outil Déplacer si aucun outil de manipulation
    /// n'est actif : le bouton « Caméra » de la barre de la vue, et sa commande.
    ///
    /// **Tant que l'outil Trajectoire a le pointeur dans la vue** (`e3_trace`), il le garde (Anthony, 2026-11-13 : « Le
    /// bouton « Caméra » de la barre de la vue fait quitter l'outil Trajectoire. On garde l'outil » ; avant, le bouton
    /// rendait le pointeur aux cartes, comme un outil de la vue). La caméra est alors choisie seule, exactement comme par
    /// « choisir » du panneau Espace 3D (`e3_choisir_la_camera`), et le prochain trait trace son chemin. `ajouter` (Maj
    /// tenue au clic) : elle rejoint ce qui est choisi, ou le quitte. Avec tout autre outil, rien ne change : `ajouter` ne
    /// sert pas, et les autres outils de la vue (choisir, déplacer, tourner, taille) rendent toujours le pointeur aux cartes.
    fn espace3d_pick_camera(&mut self, ajouter: bool) {
        if self.e3_trace() {
            self.e3_choisir_la_camera(ajouter);
            self.e3_traj_dire_ce_qui_suivra();
            return;
        }
        if !self.espace3d.on {
            self.set_espace3d(true);
        }
        self.espace3d.sel = vec![CAMERA_ID];
        if self.espace3d.mode == GizmoMode::Select {
            self.espace3d.mode = GizmoMode::Move;
        }
        self.espace3d.through_camera = false;
        self.e3_manipuler();
    }

    // ------------------------------------------------------------------------- barre d'outils

    /// Les options de vue de l'Espace 3D, dans la barre au-dessus du dessin : recadrer, regarder par la caméra, une, deux ou
    /// quatre vues. Ses outils (dessiner sur la carte, choisir, déplacer, tourner, taille, caméra) sont en tête de la barre
    /// d'outils de gauche tant que la vue est ouverte (partie 160, `espace3d_outils`).
    pub(crate) fn espace3d_tools(&mut self, ui: &mut egui::Ui) {
        if self.case_icone(ui, ph::CORNERS_OUT, None, "Recadrer", "Tout l'espace dans la vue : les cartes et la caméra.", "espace3d.cadrer").clicked() {
            self.espace3d_fit();
        }
        if self.case_icone(ui, ph::BINOCULARS, Some(self.e3_par_la_camera()), "Regarder par la caméra", "Ce que la caméra du plan filme, au format du cadre (avec plusieurs vues : dans la vue active). Glisser ou tourner la vue déplace alors la caméra (sa clé de l'image courante si elle est animée).", "espace3d.vue_camera").clicked() {
            self.e3_basculer_par_la_camera();
        }
        // Une, deux ou quatre vues (`espace3dvues.rs`).
        self.e3_vues_bouton(ui);
    }

    /// Les outils de la vue Espace 3D, en tête de la barre d'outils de gauche quand la vue est ouverte (partie 160 ; avant,
    /// dans la barre au-dessus du dessin) : une colonne, dans le même ordre qu'avant.
    pub(crate) fn espace3d_outils(&mut self, ui: &mut egui::Ui) {
        // « Dessiner sur la carte » (`espace3ddessin.rs`, partie 146) : allumé, le pointeur dessine et les outils de la
        // vue sont éteints ; un outil de la vue lui rend les cartes.
        // (L'outil Trajectoire, pris dans la vue, a lui aussi le pointeur : les outils de la vue sont éteints, partie 149.
        // L'outil Sélection de même, partie 152.)
        let dessine = self.e3_dessine() || self.e3_trace() || self.e3_selectionne();
        self.espace3d_bouton_dessiner(ui);
        for mode in GizmoMode::ALL {
            if self.case_icone(ui, mode_icon(mode), Some(self.espace3d.mode == mode && !dessine), mode.label(), mode_description(mode), mode.action()).clicked() {
                self.espace3d.mode = mode;
                self.e3_manipuler();
            }
        }
        // (Avec l'outil Trajectoire, la case « Caméra » ne lui reprend pas le pointeur : elle choisit la caméra, seule, et
        // reste allumée tant qu'elle l'est. Anthony, 2026-11-13 : « On garde l'outil ».)
        let cam_chosen = self.espace3d.sel.contains(&CAMERA_ID) && (!dessine || self.e3_trace());
        let description = "Choisit la caméra du plan pour la déplacer ou la tourner dans la vue (sa clé de l'image courante si elle est animée). Avec l'outil Trajectoire : la caméra est choisie seule et l'outil reste en main, le prochain trait trace son chemin (Maj+clic : avec les cartes choisies, ou la lâcher).";
        let r = self.case_icone(ui, ph::CAMERA, Some(cam_chosen), "Caméra", description, "espace3d.outil_camera");
        if r.clicked() {
            let ajouter = ui.input(|i| i.modifiers.shift);
            self.espace3d_pick_camera(ajouter);
        }
    }

    // ------------------------------------------------------------------------ panneau Espace 3D

    /// Une étape d'annulation de l'état d'avant, au nom `name` (à appeler AVANT le changement).
    fn record_named(&mut self, name: &str) {
        let before = self.history.context.replace(name.to_string());
        self.record_step();
        self.history.context = before;
    }

    fn edited3d(&mut self, resp: &egui::Response, name: &str) -> bool {
        let before = self.history.context.replace(name.to_string());
        let changed = self.edited(resp);
        self.history.context = before;
        changed
    }

    /// Trois champs X, Y, Z pour un vecteur ; vrai si l'un a changé.
    fn vec3_fields(&mut self, ui: &mut egui::Ui, v: &mut V3, speed: f64, name: &str, lo: f64, hi: f64) -> bool {
        let mut changed = false;
        for (k, label) in ["X", "Y", "Z"].into_iter().enumerate() {
            let r = ui.add(egui::DragValue::new(&mut v[k]).speed(speed).range(lo..=hi).max_decimals(3).prefix(format!("{label} ")));
            changed |= self.edited3d(&r, name);
        }
        changed
    }

    /// Les réglages de la vue Espace 3D : ce que montre le panneau « Espace 3D » quand la vue est ouverte
    /// (`espace3dpanneau.rs`, partie 150 ; avant elle, ils prenaient la place des réglages de l'outil dans le panneau
    /// Réglages). Les réglages de l'outil en main n'y sont pas : ceux de la Trajectoire dans l'espace, comme ceux du pinceau,
    /// sont dans le panneau Réglages (`trajectoire_espace_options_ui`).
    pub(crate) fn espace3d_options_panel(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::interrupteur;
        // Le titre, puis sur la même ligne le repère des poignées, l'aimantation, la grille, les vues, le plan en Espace 3D
        // (partie 161.4 : une ligne de moins, pour que la caméra entière se voie dans la colonne de 290 points ; « cartes et
        // caméra », qui suivait le titre, est dans sa bulle). La ligne passe à la suivante si la colonne est trop étroite.
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("{}  Espace 3D", ph::CUBE)).strong()).on_hover_text("Cartes et caméra du plan.");
            interrupteur(ui, &mut self.espace3d.local, ph::CUBE_FOCUS, "Manipulateur dans le repère de la carte", "Les axes suivent la carte (une seule carte choisie) au lieu de ceux de l'espace.", "");
            let r = self.bouton_etat(ui, self.espace3d.snap, ph::MAGNET, "Aimanter à la grille", "Déplacements au pas de la grille, rotations et tailles par paliers.", "espace3d.aimanter");
            if r.clicked() {
                self.espace3d.snap = !self.espace3d.snap;
            }
            let r = self.bouton_etat(ui, self.espace3d.show_grid, ph::GRID_FOUR, "Grille du sol", "Quadrillage au sol, un trait plus marqué tous les cinq.", "espace3d.grille");
            if r.clicked() {
                self.e3_maillage_basculer();
            }
            // Une, deux ou quatre vues (`espace3dvues.rs`, partie 144) : un réglage de la vue, donc ici et dans la barre
            // de la vue, pas dans le panneau Réglages (partie 150).
            self.e3_vues_bouton(ui);
            // Ce qui est devant cache ce qui est derrière (`Panel::ordre_distance`, partie 159) : un réglage du plan courant.
            let r = self.bouton_etat(
                ui,
                self.doc.ordre_par_la_distance(self.cur_panel),
                ph::CUBE_TRANSPARENT,
                "Devant cache derrière",
                "Allumé : les calques de ce plan se posent du plus loin au plus près de la caméra, où qu'elle regarde. Éteint (d'origine) : dans l'ordre de la pile. Sol, Mur, Ciel et Nouvelle surface l'allument d'eux-mêmes quand l'image du plan n'en change pas.",
                "espace3d.ordre_distance",
            );
            if r.clicked() {
                self.espace3d_basculer_ordre();
            }
        });
        if self.espace3d.snap {
            ui.horizontal(|ui| {
                let r = ui.add(egui::DragValue::new(&mut self.espace3d.step_move).speed(0.01).range(0.01..=10.0).suffix(" m").prefix("pas "));
                crate::bulles::bulle(r, "Pas de la grille", "Déplacements aimantés à ce pas.", "");
                let r = ui.add(egui::DragValue::new(&mut self.espace3d.step_rot).speed(0.5).range(1.0..=90.0).suffix("°"));
                crate::bulles::bulle(r, "Pas de rotation", "Rotations aimantées à ce pas.", "");
                let r = ui.add(egui::DragValue::new(&mut self.espace3d.step_scale).speed(0.01).range(0.01..=2.0));
                crate::bulles::bulle(r, "Pas de taille", "Tailles aimantées à ce pas.", "");
            });
        }
        // L'environnement : le fond de la vue, le maillage du sol (`espace3ddecor.rs`, partie 157).
        self.e3_environnement_ui(ui);
        ui.add(egui::Separator::default().spacing(SEPARATEUR));

        // Les cartes : les calques du plan, le dessus de la pile d'abord. Sur la même ligne, poser une surface à dessiner
        // (partie 159 : face à la caméra, au sol, en mur, à la distance dite) — une ligne de moins, pour que la caméra entière
        // se voie dans la colonne de 290 points (partie 161.4). La ligne passe à la suivante si la colonne est trop étroite.
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("{}  Cartes (calques)", ph::STACK)).strong());
            interrupteur(ui, &mut self.espace3d.show_cards, ph::EYE, "Montrer les cartes", "Les calques du plan sont montrés dans l'espace. Ils restent dans l'image, montrés ici ou non.", "");
            if self.bouton(ui, ph::ARROWS_DOWN_UP, "Ordonner la pile selon la profondeur", "Met les calques du plus loin (en bas de la pile) au plus près : l'image compose les cartes dans l'ordre de la pile.", "espace3d.ordonner_profondeur").clicked() {
                self.espace3d_order_by_depth();
            }
            // (Nouvelle surface, partie 159 : un menu, pour que la ligne tienne dans la colonne de 290 points, partie 161.4.)
            let bouton = egui::Button::new(RichText::new(ph::PLUS).size(crate::bulles::TAILLE));
            let resp = egui::menu::menu_custom_button(ui, bouton, |ui| {
                for (sorte, icone, action, description) in [
                    (espace3d::Surface::Face, ph::FRAME_CORNERS, "espace3d.surface_face", "Un calque neuf posé face à la caméra, à la distance choisie : le cadre tombe en son centre."),
                    (espace3d::Surface::Sol, ph::ALIGN_BOTTOM, "espace3d.surface_sol", "Un calque neuf couché au sol, devant la caméra, à la distance choisie."),
                    (espace3d::Surface::Mur, ph::WALL, "espace3d.surface_mur", "Un calque neuf debout sur le sol, face à la caméra, à la distance choisie."),
                ] {
                    let r = ui.button(format!("{icone}  {}", sorte.nom()));
                    if crate::bulles::bulle(r, sorte.nom(), description, action).clicked() {
                        self.espace3d_nouvelle_surface(sorte);
                        ui.close_menu();
                    }
                }
                ui.horizontal(|ui| {
                    ui.label("Distance");
                    let mut d = self.espace3d_distance_surface();
                    let r = ui.add(egui::DragValue::new(&mut d).speed(0.05).range(0.3..=500.0).suffix(" m"));
                    if crate::bulles::bulle(r, "Distance de la surface", "À combien de mètres devant la caméra la surface neuve se pose. La distance d'origine de la caméra : celle de la feuille.", "").changed() {
                        self.espace3d.surface_distance = d;
                    }
                });
            })
            .response;
            crate::bulles::bulle(resp, "Nouvelle surface", "Un calque neuf à dessiner, posé face à la caméra, au sol ou en mur, à la distance choisie.", "");
        });
        let frame = self.cur_frame;
        let world = self.e3_world();
        let cards: Vec<(usize, u64, String, f32, bool)> = self
            .e3_visible_cards()
            .into_iter()
            .rev()
            .map(|li| {
                let l = &self.layers()[li];
                (li, l.id, l.name.clone(), world.profondeur(l, frame), l.locked || l.position_locked)
            })
            .collect();
        egui::ScrollArea::vertical().id_salt("espace3d_cartes").max_height(130.0).auto_shrink([true, true]).show(ui, |ui| {
            for (li, id, name, depth, locked) in &cards {
                let key = espace3d::card_selection(*id);
                let selected = self.espace3d.sel.contains(&key);
                let r = ui.selectable_label(selected, format!("{} {}  ({:+.2})", if *locked { ph::LOCK } else { ph::SQUARE }, name, depth));
                if r.clicked() {
                    if ui.input(|i| i.modifiers.shift || i.modifiers.command) {
                        match self.espace3d.sel.iter().position(|s| *s == key) {
                            Some(p) => {
                                self.espace3d.sel.remove(p);
                            }
                            None => self.espace3d.sel.push(key),
                        }
                    } else {
                        self.espace3d.sel = vec![key];
                        self.select_layer(*li);
                        self.espace3d.last_layer = *id;
                    }
                }
            }
        });
        let chosen_cards: Vec<usize> = self.espace3d.sel.iter().filter_map(|&s| self.e3_card_layer(s)).collect();
        if let [li] = chosen_cards.as_slice() {
            ui.add(egui::Separator::default().spacing(SEPARATEUR));
            self.espace3d_card_fields(ui, *li);
            // (Un calque Sol, Mur ou Ciel : sa grille de repère, `espace3ddecor.rs`.)
            self.e3_grille_carte_ui(ui, *li);
        } else if chosen_cards.len() > 1 {
            ui.add(egui::Separator::default().spacing(SEPARATEUR));
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} cartes choisies", chosen_cards.len())).strong());
                // (Les clés de position des cartes choisies, toutes ensemble : `espace3dcles.rs`.)
                self.espace3d_boutons_cle_position(ui);
            });
            // (Ce qu'elles font ensemble est dans le registre des explications, rubrique Espace 3D, partie 153.)
        }
        ui.add(egui::Separator::default().spacing(SEPARATEUR));
        self.espace3d_camera_fields(ui);
    }

    /// Les réglages d'une carte (un calque du plan) : position et rotation dans l'espace, échelle, étirements, profondeur
    /// du multiplan. Un calque animé règle la clé de l'image courante (comme l'outil Transformer).
    fn espace3d_card_fields(&mut self, ui: &mut egui::Ui, li: usize) {
        let world = self.e3_world();
        let frame = self.cur_frame;
        let layer = &self.layers()[li];
        let (name, locked, animated) = (layer.name.clone(), layer.locked || layer.position_locked, layer.is_animated());
        let t0 = layer.pose(frame);
        // (Plan en Espace 3D : la profondeur est libre, la carte peut être derrière la caméra d'origine du plan.)
        let d0 = world.profondeur(layer, frame);
        let mut xf = world.card_xf(&t0, d0);
        let mut t = t0;
        let mut depth = d0;
        let (mut moved, mut scaled, mut deepened, mut flat) = (false, false, false, false);
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} {}", ph::SQUARE, name)).strong());
            // (La clé de position de cette image : la poser, la retirer. `espace3dcles.rs`.)
            self.espace3d_boutons_cle_position(ui);
            if locked {
                ui.label(RichText::new("verrouillée").small().weak());
            }
            if animated {
                ui.label(RichText::new("animée : on règle la clé de cette image").small().weak());
            }
        });
        ui.add_enabled_ui(!locked, |ui| {
            egui::Grid::new("espace3d_carte").num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
                ui.label("Position (m)");
                ui.horizontal(|ui| moved |= self.vec3_fields(ui, &mut xf.pos, 0.02, "Placer le calque", -1e4, 1e4));
                ui.end_row();
                ui.label("Rotation (°)");
                ui.horizontal(|ui| moved |= self.vec3_fields(ui, &mut xf.rot, 0.5, "Tourner le calque", -1e4, 1e4));
                ui.end_row();
                ui.label("Échelle");
                ui.horizontal(|ui| {
                    let mut pct = t.scale * 100.0;
                    let r = ui.add(egui::DragValue::new(&mut pct).speed(0.5).range(1.0..=10000.0).max_decimals(1).suffix(" %"));
                    if self.edited3d(&r, "Échelle du calque") {
                        t.scale = pct / 100.0;
                        scaled = true;
                    }
                });
                ui.end_row();
                ui.label("Étirement");
                ui.horizontal(|ui| {
                    for (k, label) in ["X", "Y"].into_iter().enumerate() {
                        let mut pct = if k == 0 { t.stretch_x } else { t.stretch_y } * 100.0;
                        let r = ui.add(egui::DragValue::new(&mut pct).speed(0.5).range(-10000.0..=10000.0).max_decimals(1).suffix(" %").prefix(format!("{label} ")));
                        if self.edited3d(&r, "Étirement du calque") {
                            let v = LayerTransform::clamp_stretch(pct / 100.0);
                            if k == 0 {
                                t.stretch_x = v;
                            } else {
                                t.stretch_y = v;
                            }
                            scaled = true;
                        }
                    }
                });
                ui.end_row();
                ui.label("Profondeur");
                ui.horizontal(|ui| {
                    // (Plan en Espace 3D : le champ descend sous la borne des plans ordinaires. Il le faut aussi pour le
                    // montrer : un champ borné ramènerait à sa borne, sans qu'on y touche, une carte qui l'a passée.)
                    // (Tout plan est dans l'espace, partie 159 : la vue de la scène a toujours la profondeur libre.)
                    let r = ui.add(egui::DragValue::new(&mut depth).speed(0.01).range(-Layer::MAX_DEPTH_LIBRE..=1000.0).max_decimals(2));
                    deepened |= self.edited3d(&r, "Profondeur du calque");
                    let phrase = "Comme dans le panneau Calques : la carte recule en gardant, vue de la caméra de référence, la taille qu'elle a. Le manipulateur, lui, la recule en gardant sa taille dans l'espace. Sous −0,9 la carte ne change plus de taille : elle avance jusqu'à cette caméra, la caméra d'origine du plan (−1), puis passe derrière elle.";
                    crate::bulles::bulle(r, "Profondeur (multiplan)", phrase, "");
                    if crate::bulles::icone_si(ui, t.is_tilted(), ph::ARROW_COUNTER_CLOCKWISE, "Remettre à plat", "Annule l'inclinaison : la carte redevient parallèle à la feuille.", "").clicked() {
                        flat = true;
                    }
                });
                ui.end_row();
            });
        });
        if moved || scaled || deepened || flat {
            self.modified = true;
            // (Tout plan est dans l'espace, partie 159 : la profondeur et l'inclinaison sont libres dans la vue de la scène.)
            let libre = true;
            let layer = &mut self.layers_mut()[li];
            if moved {
                world.apply_card_xf_libre(layer, frame, &xf);
            }
            if scaled || flat {
                let mut p = layer.pose(frame);
                if scaled {
                    (p.scale, p.stretch_x, p.stretch_y) = (t.scale, t.stretch_x, t.stretch_y);
                }
                if flat {
                    (p.tilt_x, p.tilt_y) = (0.0, 0.0);
                }
                layer.set_pose_selon(frame, p, libre);
            }
            if deepened {
                layer.set_depth_selon(frame, depth, libre);
            }
            // (Une carte qui suit une trajectoire dans l'espace : ce réglage vaut pour sa pose propre, à toutes les images.)
            self.trajectoire_espace_apres_pose(li);
        }
        // (Les axes de rotation d'une carte : l'animation « Une carte et ses axes de rotation », registre des explications.)
    }

    /// La caméra du plan, seule caméra de l'espace : placement, focale (objectifs du plan), vue par la caméra.
    /// Ce qu'on règle ici est réglé à l'image courante, ou sur la clé de cette image si la caméra est animée.
    fn espace3d_camera_fields(&mut self, ui: &mut egui::Ui) {
        let world = self.e3_world();
        let base = self.current_camera();
        let mut cam: SceneCam = world.scene_cam(&base);
        let mut changed = false;
        let keys = self.doc.panels[self.cur_panel].camera.len();
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{}  Caméra du plan", ph::VIDEO_CAMERA)).strong());
            let selected = self.espace3d.sel.contains(&CAMERA_ID);
            // (Seule : les cartes sont lâchées, la timeline montre la piste « Caméra » choisie. Maj+clic : avec les cartes.)
            if ui.selectable_label(selected, "choisir").on_hover_text("Choisir la caméra, seule, pour la déplacer, la tourner, ou tracer son chemin dans la vue : les cartes sont lâchées. Maj+clic : l'ajouter aux cartes choisies, ou la lâcher.").clicked() {
                let ajouter = ui.input(|i| i.modifiers.shift);
                self.e3_choisir_la_camera(ajouter);
            }
            // (La clé de caméra de cette image : la poser, la retirer. `espace3dcles.rs`.)
            self.espace3d_boutons_cle_camera(ui);
        });
        egui::Grid::new("espace3d_camera").num_columns(2).spacing([6.0, 2.0]).show(ui, |ui| {
            ui.label("Position (m)");
            ui.horizontal(|ui| changed |= self.vec3_fields(ui, &mut cam.pos, 0.02, "Placer la caméra", -1e4, 1e4));
            ui.end_row();
            // (Partie 161.4 : panoramique, bascule et rotation sur une seule ligne, dans cet ordre, pour que la caméra entière se
            // voie sans faire défiler le panneau dans la colonne de 290 points.)
            ui.label("Angles (°)");
            ui.horizontal(|ui| {
                let r = ui.add(egui::DragValue::new(&mut cam.pan).speed(0.3).max_decimals(1).prefix("pan "));
                let r = crate::bulles::bulle(r, "Panoramique", "En degrés : positif, la caméra regarde vers la droite.", "");
                changed |= self.edited3d(&r, "Orienter la caméra");
                let r = ui.add(egui::DragValue::new(&mut cam.tilt).speed(0.3).max_decimals(1).range(-MAX_TILT..=MAX_TILT).prefix("basc. "));
                let r = crate::bulles::bulle(r, "Bascule", "En degrés : la caméra lève ou baisse le nez.", "");
                changed |= self.edited3d(&r, "Orienter la caméra");
                let r = ui.add(egui::DragValue::new(&mut cam.roll).speed(0.3).max_decimals(1).prefix("rot. "));
                let r = crate::bulles::bulle(r, "Rotation", "En degrés : la caméra tourne autour de son axe de visée.", "");
                changed |= self.edited3d(&r, "Orienter la caméra");
            });
            ui.end_row();
            ui.label("Focale");
            ui.horizontal(|ui| {
                let r = ui.add(egui::DragValue::new(&mut cam.focal).speed(0.5).range(MIN_FOCAL..=MAX_FOCAL).suffix(" mm"));
                let r = crate::bulles::bulle(r, "Focale de la caméra", "En millimètres, pour une pellicule de 36 mm : plus courte, le champ s'élargit ; plus longue, il se resserre.", "");
                changed |= self.edited3d(&r, "Focale de la caméra");
                let current = FOCAL_PRESETS.iter().find(|p| (p.mm - cam.focal).abs() < 0.01).map_or_else(|| format!("{:.0}°", cam.angle_of_view()), |p| p.name.to_string());
                egui::ComboBox::from_id_salt("espace3d_objectif").selected_text(current).width(110.0).show_ui(ui, |ui| {
                    for p in FOCAL_PRESETS.iter().filter(|p| !p.anamorphic) {
                        if ui.selectable_label((p.mm - cam.focal).abs() < 0.01, focal_label(p)).on_hover_text(p.note).clicked() {
                            cam.focal = p.mm;
                            changed = true;
                            self.record_named("Objectif de la caméra");
                        }
                    }
                });
            });
            ui.end_row();
        });
        // (Ce que peut la caméra libre : dans la bulle de cette ligne, pour que la caméra entière se voie dans la colonne de
        // 290 points, partie 161.4 ; et l'animation « La caméra libre », registre des explications.)
        ui.label(RichText::new(format!("Champ horizontal {:.0}° (pellicule de 36 mm).{}", cam.angle_of_view(), if keys >= 2 { format!(" {keys} clés de caméra : on règle la clé de l'image courante.") } else { String::new() })).small().weak())
            .on_hover_text("La caméra est libre : elle se retourne, bascule jusqu'à la verticale, passe la feuille.");
        if changed {
            let sc = cam.sanitized();
            let mut new = world.camera_from_scene(&sc);
            // (Le zoom ne change que si la focale a changé : un zoom hors des bornes de l'objectif ne se perd pas.)
            if (sc.focal - base.focal().clamp(MIN_FOCAL, MAX_FOCAL)).abs() < 1e-3 {
                new.zoom = base.zoom;
            }
            self.set_current_camera_libre(new);
            self.modified = true;
        }
        ui.horizontal(|ui| {
            if self.bouton_etat(ui, self.e3_par_la_camera(), ph::BINOCULARS, "Regarder par la caméra", "La vue montre ce que la caméra du plan filme, au format du cadre (avec plusieurs vues : la vue active).", "espace3d.vue_camera").clicked() {
                self.e3_basculer_par_la_camera();
            }
            let some = self.espace3d.sel.iter().any(|i| *i != CAMERA_ID);
            if crate::bulles::icone_si(ui, some, ph::CROSSHAIR, "Viser la sélection", "Tourne la caméra vers les cartes choisies.", "").clicked() {
                let ids: Vec<u64> = self.espace3d.sel.iter().copied().filter(|i| *i != CAMERA_ID).collect();
                let target = gz::pivot_of(&self.e3_xfs(&ids));
                let mut sc = world.scene_cam(&self.current_camera());
                sc.aim_at(target);
                let aimed = Camera { pan: sc.pan, tilt: sc.tilt, ..self.current_camera() };
                self.espace3d_step("Viser avec la caméra", |s| s.set_current_camera_libre(aimed));
            }
        });
    }
}
