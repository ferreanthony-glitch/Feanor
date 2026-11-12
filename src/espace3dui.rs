//! L'Espace 3D dans l'application (`docs/ESPACE3D.md`) : le mode « Espace 3D » (une vue d'édition à la place du dessin,
//! où les calques du plan sont des cartes qu'on déplace, tourne et met à l'échelle, avec la caméra du plan), ses
//! commandes, la barre d'outils, le panneau Réglages et l'annulation.
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
        self.status = if on && self.doc.plan_en_espace(self.cur_panel) {
            "Espace 3D : ce plan est en Espace 3D. La caméra est libre (choisis-la pour la déplacer ou la tourner), ce qui est devant cache ce qui est derrière, et « Nouvelle surface » dans Réglages pose un calque face à la caméra, au sol ou en mur.".into()
        } else if on {
            "Espace 3D : clic sur une carte (un calque) ou sur la caméra pour la choisir, glisser pour la déplacer, clic droit pour tourner la vue, bouton du milieu pour la glisser, molette pour avancer. Réglages : panneau Réglages.".into()
        } else {
            "Espace 3D quitté.".into()
        };
    }

    /// Les dimensions du monde de ce projet.
    pub(crate) fn e3_world(&self) -> Monde {
        Monde::of(&self.doc)
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
                Some((id, world.card_xf(&layer.pose(frame), layer.depth_at(frame))))
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
                let cam = world.camera_from_xf(&self.current_camera(), xf);
                self.set_current_camera(cam);
            } else if let Some(li) = self.e3_card_layer(*id).filter(|&li| !self.layers()[li].locked && !self.layers()[li].position_locked) {
                // (Un calque verrouillé, ou de position verrouillée, ne bouge pas, même avec les poignées d'un groupe.)
                // (Plan en Espace 3D : l'inclinaison est libre, la carte se couche au sol ou fait un demi-tour.)
                if self.doc.plan_en_espace(self.cur_panel) {
                    world.apply_card_xf_libre(&mut self.layers_mut()[li], frame, xf);
                } else {
                    world.apply_card_xf(&mut self.layers_mut()[li], frame, xf);
                }
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
                points.extend(world.card_corners(&world.card_xf(&layer.pose(self.cur_frame), layer.depth_at(self.cur_frame))));
            }
        }
        points.push(world.eye(&self.current_camera()));
        let bounds = crate::espace3dmath::bounds_of(points).filter(|b| crate::espace3dmath::length(crate::espace3dmath::sub(b.1, b.0)) > 0.1).unwrap_or(([-3.0, 0.0, -3.0], [3.0, 2.0, 3.0]));
        let rect = self.canvas_rect;
        let mut cam = Orbite::fit_at(bounds, [rect.width().max(1.0), rect.height().max(1.0)], 30.0, 22.0);
        cam.distance = cam.distance.max(2.0);
        self.espace3d.cam = Some(cam);
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
        order.sort_by(|&a, &b| layers[b].depth_at(frame).total_cmp(&layers[a].depth_at(frame)));
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

    /// Coche ou décoche « Plan en Espace 3D » pour le plan courant (`Panel::espace`, `docs/ESPACE-LIBRE.md`), en une étape
    /// d'annulation. Étape A1 : la case est retenue, enregistrée et suit le plan (couper, dupliquer, versions) ; l'image
    /// n'en dépend pas encore. Refusé si le cadre ne tient pas dans la feuille (`Document::accepte_plan_en_espace`).
    pub(crate) fn espace3d_basculer_plan_en_espace(&mut self) {
        let on = !self.panel().espace;
        if on && !self.doc.accepte_plan_en_espace() {
            self.status = "Plan en Espace 3D refusé : le cadre du projet est plus grand que la feuille. Agrandis la feuille (Fichier > Réglages du projet).".into();
            return;
        }
        let pi = self.cur_panel;
        self.espace3d_step("Plan en Espace 3D", |s| s.doc.panels[pi].espace = on);
        self.renderer.invalidate_all();
        self.status = if on {
            "Plan en Espace 3D : coché pour ce plan. (En construction : l'image ne change pas encore.)".into()
        } else {
            "Plan en Espace 3D : décoché, le plan est de nouveau un plan ordinaire.".into()
        };
    }

    /// La distance où une surface neuve se pose (mètres) : celle des Réglages, sinon la distance de référence.
    pub(crate) fn espace3d_distance_surface(&self) -> f32 {
        let d = self.espace3d.surface_distance;
        if d.is_finite() && d > 0.0 { d } else { self.e3_world().d0 }
    }

    /// **Pose une surface à dessiner** dans le plan en Espace 3D courant (`docs/ESPACE-LIBRE.md`, étape A6) : un calque
    /// neuf, juste au-dessus du calque courant, du même genre que lui (vectoriel ou bitmap), posé face à la caméra, au sol
    /// ou en mur (`Monde::surface_xf`) ; il devient le calque courant et la carte choisie. Une étape d'annulation.
    /// Refusé hors d'un plan en Espace 3D, storyboard verrouillé, ou si la surface tomberait derrière la caméra de référence.
    pub(crate) fn espace3d_nouvelle_surface(&mut self, sorte: espace3d::Surface) {
        if !self.doc.plan_en_espace(self.cur_panel) {
            self.status = format!("{} : ce plan n'est pas en Espace 3D. Coche d'abord « Plan en Espace 3D » dans Réglages.", sorte.nom());
            return;
        }
        if self.content_refuses() {
            return;
        }
        let world = self.e3_world();
        let distance = self.espace3d_distance_surface();
        let Some(xf) = world.surface_xf(&self.current_camera(), sorte, distance) else {
            self.status = format!("{} refusée : à {distance:.1} m devant cette caméra, elle tomberait derrière la caméra d'origine du plan, où une carte ne peut pas être. Rapproche-la ou tourne la caméra.", sorte.nom());
            return;
        };
        self.end_stroke();
        let snap = self.snapshot();
        let frame = self.cur_frame;
        let kind = match self.layers().get(self.cur_layer) {
            Some(l) if !l.folder && l.effet.is_none() => l.kind,
            _ => crate::document::LayerKind::Vector,
        };
        let deja = self.layers().iter().filter(|l| l.name.starts_with(sorte.nom())).count();
        let nom = if deja == 0 { sorte.nom().to_string() } else { format!("{} {}", sorte.nom(), deja + 1) };
        let mut layer = crate::document::Layer::new(nom.clone(), kind);
        layer.parent = self.layers().get(self.cur_layer).and_then(|l| if l.folder { None } else { l.parent });
        world.apply_card_xf_libre(&mut layer, frame, &xf);
        let id = layer.id;
        let at = (self.cur_layer + 1).min(self.layers().len());
        self.layers_mut().insert(at, layer);
        self.cur_layer = at;
        self.vsel = None;
        let before = self.history.context.replace(sorte.nom().to_string());
        self.commit_structure(snap);
        self.history.context = before;
        self.espace3d.sel = vec![espace3d::card_selection(id)];
        self.espace3d.last_layer = id;
        self.status = format!("« {nom} » posée à {distance:.1} m : c'est le calque courant, dessine dessus.");
    }

    /// Les commandes de l'Espace 3D. Vrai si l'action est la sienne.
    pub(crate) fn espace3d_action(&mut self, action: &str) -> bool {
        // Les anciens noms (plateau en 3D, scène 3D) mènent à l'original (`keymap::ALIASES`).
        let action = crate::keymap::ALIASES.iter().find(|(alias, original)| *alias == action && original.starts_with("espace3d.")).map_or(action, |(_, original)| *original);
        if !action.starts_with("espace3d.") {
            return false;
        }
        match action {
            "espace3d.basculer" => self.set_espace3d(!(self.espace3d.on && !self.camera_view)),
            "espace3d.outil_choisir" => self.espace3d_mode(GizmoMode::Select),
            "espace3d.outil_deplacer" => self.espace3d_mode(GizmoMode::Move),
            "espace3d.outil_tourner" => self.espace3d_mode(GizmoMode::Rotate),
            "espace3d.outil_echelle" => self.espace3d_mode(GizmoMode::Scale),
            "espace3d.outil_camera" => self.espace3d_pick_camera(),
            "espace3d.vue_camera" => {
                self.set_espace3d(true);
                self.espace3d.through_camera = !self.espace3d.through_camera;
            }
            "espace3d.cadrer" => {
                self.set_espace3d(true);
                self.espace3d_fit();
            }
            "espace3d.ordonner_profondeur" => self.espace3d_order_by_depth(),
            "espace3d.plan_en_espace" => self.espace3d_basculer_plan_en_espace(),
            "espace3d.surface_face" => self.espace3d_nouvelle_surface(espace3d::Surface::Face),
            "espace3d.surface_sol" => self.espace3d_nouvelle_surface(espace3d::Surface::Sol),
            "espace3d.surface_mur" => self.espace3d_nouvelle_surface(espace3d::Surface::Mur),
            "espace3d.aimanter" => {
                self.espace3d.snap = !self.espace3d.snap;
                self.status = if self.espace3d.snap { "Aimantation à la grille : allumée.".into() } else { "Aimantation : éteinte.".into() };
            }
            "espace3d.grille" => self.espace3d.show_grid = !self.espace3d.show_grid,
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

    /// Choisit la caméra du plan (pour la déplacer ou la tourner), avec l'outil Déplacer si aucun outil de manipulation
    /// n'est actif.
    fn espace3d_pick_camera(&mut self) {
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

    /// Outils de l'Espace 3D, dans la barre au-dessus du dessin : une seule rangée.
    pub(crate) fn espace3d_tools(&mut self, ui: &mut egui::Ui) {
        // « Dessiner sur la carte » (`espace3ddessin.rs`, partie 146) : allumé, le pointeur dessine et les outils de la
        // vue sont éteints ; un outil de la vue lui rend les cartes.
        let dessine = self.e3_dessine();
        self.espace3d_bouton_dessiner(ui);
        for mode in GizmoMode::ALL {
            if self.case_icone(ui, mode_icon(mode), Some(self.espace3d.mode == mode && !dessine), mode.label(), mode_description(mode), mode.action()).clicked() {
                self.espace3d.mode = mode;
                self.e3_manipuler();
            }
        }
        let cam_chosen = self.espace3d.sel.contains(&CAMERA_ID) && !dessine;
        if self.case_icone(ui, ph::CAMERA, Some(cam_chosen), "Caméra", "Choisit la caméra du plan pour la déplacer ou la tourner dans la vue (sa clé de l'image courante si elle est animée).", "espace3d.outil_camera").clicked() {
            self.espace3d_pick_camera();
        }
        ui.add_space(crate::barres::MARGE_TRAIT);
        ui.separator();
        ui.add_space(crate::barres::MARGE_TRAIT);
        if self.case_icone(ui, ph::CORNERS_OUT, None, "Recadrer", "Tout l'espace dans la vue : les cartes et la caméra.", "espace3d.cadrer").clicked() {
            self.espace3d_fit();
        }
        if self.case_icone(ui, ph::BINOCULARS, Some(self.espace3d.through_camera), "Regarder par la caméra", "Ce que la caméra du plan filme, au format du cadre. Glisser ou tourner la vue déplace alors la caméra (sa clé de l'image courante si elle est animée).", "espace3d.vue_camera").clicked() {
            self.espace3d.through_camera = !self.espace3d.through_camera;
        }
    }

    // ------------------------------------------------------------------------ panneau Réglages

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

    /// Réglages de l'Espace 3D (panneau Réglages quand la vue 3D est ouverte).
    pub(crate) fn espace3d_options_panel(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::interrupteur;
        let note = |ui: &mut egui::Ui, text: &str| {
            ui.label(RichText::new(text).small().weak());
        };
        ui.horizontal(|ui| {
            ui.heading(format!("{}  Espace 3D", ph::CUBE));
            ui.label(RichText::new("cartes et caméra").small().weak());
        });
        // Repère des poignées, aimantation, grille.
        ui.horizontal(|ui| {
            interrupteur(ui, &mut self.espace3d.local, ph::CUBE_FOCUS, "Manipulateur dans le repère de la carte", "Les axes suivent la carte (une seule carte choisie) au lieu de ceux de l'espace.", "");
            let r = self.bouton_etat(ui, self.espace3d.snap, ph::MAGNET, "Aimanter à la grille", "Déplacements au pas de la grille, rotations et tailles par paliers.", "espace3d.aimanter");
            if r.clicked() {
                self.espace3d.snap = !self.espace3d.snap;
            }
            let r = self.bouton_etat(ui, self.espace3d.show_grid, ph::GRID_FOUR, "Grille du sol", "Quadrillage au sol, un trait plus marqué tous les cinq.", "espace3d.grille");
            if r.clicked() {
                self.espace3d.show_grid = !self.espace3d.show_grid;
            }
            // Plan en Espace 3D (`docs/ESPACE-LIBRE.md`) : une case du plan courant. Étape A1 : l'image n'en dépend pas encore.
            let r = self.bouton_etat(
                ui,
                self.panel().espace,
                ph::CUBE_TRANSPARENT,
                "Plan en Espace 3D",
                "Ce plan devient un plan en Espace 3D : caméra libre, cartes qui se cachent selon leur distance. En construction : pour l'instant la case est retenue et enregistrée avec le plan, l'image ne change pas encore.",
                "espace3d.plan_en_espace",
            );
            if r.clicked() {
                self.espace3d_basculer_plan_en_espace();
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
        // Poser une surface à dessiner (plan en Espace 3D seulement) : face à la caméra, au sol, en mur, à la distance dite.
        if self.doc.plan_en_espace(self.cur_panel) {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{}  Nouvelle surface", ph::PLUS)).strong());
                for (sorte, icone, action, description) in [
                    (espace3d::Surface::Face, ph::FRAME_CORNERS, "espace3d.surface_face", "Un calque neuf posé face à la caméra, à la distance choisie : le cadre tombe en son centre."),
                    (espace3d::Surface::Sol, ph::ALIGN_BOTTOM, "espace3d.surface_sol", "Un calque neuf couché au sol, devant la caméra, à la distance choisie."),
                    (espace3d::Surface::Mur, ph::WALL, "espace3d.surface_mur", "Un calque neuf debout sur le sol, face à la caméra, à la distance choisie."),
                ] {
                    if self.bouton(ui, icone, sorte.nom(), description, action).clicked() {
                        self.espace3d_nouvelle_surface(sorte);
                    }
                }
                let mut d = self.espace3d_distance_surface();
                let r = ui.add(egui::DragValue::new(&mut d).speed(0.05).range(0.3..=500.0).suffix(" m"));
                if crate::bulles::bulle(r, "Distance de la surface", "À combien de mètres devant la caméra la surface neuve se pose. La distance d'origine de la caméra : celle de la feuille.", "").changed() {
                    self.espace3d.surface_distance = d;
                }
            });
        }
        ui.separator();

        // Les cartes : les calques du plan, le dessus de la pile d'abord.
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{}  Cartes (calques)", ph::STACK)).strong());
            interrupteur(ui, &mut self.espace3d.show_cards, ph::EYE, "Montrer les cartes", "Les calques du plan sont montrés dans l'espace. Ils restent dans l'image, montrés ici ou non.", "");
            if self.bouton(ui, ph::ARROWS_DOWN_UP, "Ordonner la pile selon la profondeur", "Met les calques du plus loin (en bas de la pile) au plus près : l'image compose les cartes dans l'ordre de la pile.", "espace3d.ordonner_profondeur").clicked() {
                self.espace3d_order_by_depth();
            }
        });
        let frame = self.cur_frame;
        let cards: Vec<(usize, u64, String, f32, bool)> = self
            .e3_visible_cards()
            .into_iter()
            .rev()
            .map(|li| {
                let l = &self.layers()[li];
                (li, l.id, l.name.clone(), l.depth_at(frame), l.locked || l.position_locked)
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
            ui.separator();
            self.espace3d_card_fields(ui, *li);
        } else if chosen_cards.len() > 1 {
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} cartes choisies", chosen_cards.len())).strong());
                // (Les clés de position des cartes choisies, toutes ensemble : `espace3dcles.rs`.)
                self.espace3d_boutons_cle_position(ui);
            });
            note(ui, "Elles se déplacent, tournent et grandissent ensemble ; choisis-en une seule pour ses réglages.");
        }
        ui.separator();
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
        let d0 = layer.depth_at(frame);
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
                ui.horizontal(|ui| moved |= self.vec3_fields(ui, &mut xf.pos, 0.02, "Placer la carte dans l'espace 3D", -1e4, 1e4));
                ui.end_row();
                ui.label("Rotation (°)");
                ui.horizontal(|ui| moved |= self.vec3_fields(ui, &mut xf.rot, 0.5, "Tourner la carte dans l'espace 3D", -1e4, 1e4));
                ui.end_row();
                ui.label("Échelle");
                ui.horizontal(|ui| {
                    let mut pct = t.scale * 100.0;
                    let r = ui.add(egui::DragValue::new(&mut pct).speed(0.5).range(1.0..=10000.0).max_decimals(1).suffix(" %"));
                    if self.edited3d(&r, "Échelle de la carte") {
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
                        if self.edited3d(&r, "Étirement de la carte") {
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
                    let r = ui.add(egui::DragValue::new(&mut depth).speed(0.01).range(Layer::MIN_DEPTH..=1000.0).max_decimals(2));
                    deepened |= self.edited3d(&r, "Profondeur de la carte");
                    crate::bulles::bulle(r, "Profondeur (multiplan)", "Comme dans le panneau Calques : la carte recule en gardant, vue de la caméra de référence, la taille qu'elle a. Le manipulateur, lui, la recule en gardant sa taille dans l'espace.", "");
                    if crate::bulles::icone_si(ui, t.is_tilted(), ph::ARROW_COUNTER_CLOCKWISE, "Remettre à plat", "Annule l'inclinaison : la carte redevient parallèle à la feuille.", "").clicked() {
                        flat = true;
                    }
                });
                ui.end_row();
            });
        });
        if moved || scaled || deepened || flat {
            self.modified = true;
            let libre = self.doc.plan_en_espace(self.cur_panel);
            let layer = &mut self.layers_mut()[li];
            if moved {
                if libre {
                    world.apply_card_xf_libre(layer, frame, &xf);
                } else {
                    world.apply_card_xf(layer, frame, &xf);
                }
            }
            if scaled || flat {
                let mut p = layer.pose(frame);
                if scaled {
                    (p.scale, p.stretch_x, p.stretch_y) = (t.scale, t.stretch_x, t.stretch_y);
                }
                if flat {
                    (p.tilt_x, p.tilt_y) = (0.0, 0.0);
                }
                layer.set_pose(frame, p);
            }
            if deepened {
                layer.set_depth_at(frame, depth);
            }
        }
        ui.label(RichText::new("Rotation Z : le tour dans le plan de la carte (antihoraire) ; X et Y inclinent la carte.").small().weak());
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
            if ui.selectable_label(selected, "choisir").on_hover_text("Choisir la caméra pour la déplacer ou la tourner dans la vue").clicked() {
                self.espace3d.sel = vec![CAMERA_ID];
            }
            // (La clé de caméra de cette image : la poser, la retirer. `espace3dcles.rs`.)
            self.espace3d_boutons_cle_camera(ui);
        });
        egui::Grid::new("espace3d_camera").num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
            ui.label("Position (m)");
            ui.horizontal(|ui| changed |= self.vec3_fields(ui, &mut cam.pos, 0.02, "Placer la caméra", -1e4, 1e4));
            ui.end_row();
            ui.label("Panoramique");
            let r = ui.add(egui::DragValue::new(&mut cam.pan).speed(0.3).suffix("°"));
            changed |= self.edited3d(&r, "Orienter la caméra");
            ui.end_row();
            ui.label("Bascule");
            let r = ui.add(egui::DragValue::new(&mut cam.tilt).speed(0.3).range(-MAX_TILT..=MAX_TILT).suffix("°"));
            changed |= self.edited3d(&r, "Orienter la caméra");
            ui.end_row();
            ui.label("Rotation");
            let r = ui.add(egui::DragValue::new(&mut cam.roll).speed(0.3).suffix("°"));
            changed |= self.edited3d(&r, "Orienter la caméra");
            ui.end_row();
            ui.label("Focale");
            ui.horizontal(|ui| {
                let r = ui.add(egui::DragValue::new(&mut cam.focal).speed(0.5).range(MIN_FOCAL..=MAX_FOCAL).suffix(" mm"));
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
        ui.label(RichText::new(format!("Champ horizontal {:.0}° (pellicule de 36 mm).{}", cam.angle_of_view(), if keys >= 2 { format!(" {keys} clés de caméra : on règle la clé de l'image courante.") } else { String::new() })).small().weak());
        if self.doc.plan_en_espace(self.cur_panel) {
            ui.label(RichText::new("Plan en Espace 3D : la caméra est libre. Elle tourne tout autour (panoramique sans borne), passe derrière les cartes, bascule jusqu'à la verticale (90° : plongée ou contre-plongée totale), et l'image la suit.").small().weak());
        }
        if changed {
            let sc = cam.sanitized();
            let mut new = world.camera_from_scene(&sc);
            // (Le zoom ne change que si la focale a changé : un zoom hors des bornes de l'objectif ne se perd pas.)
            if (sc.focal - base.focal().clamp(MIN_FOCAL, MAX_FOCAL)).abs() < 1e-3 {
                new.zoom = base.zoom;
            }
            self.set_current_camera(new);
            self.modified = true;
        }
        ui.horizontal(|ui| {
            if self.bouton_etat(ui, self.espace3d.through_camera, ph::BINOCULARS, "Regarder par la caméra", "La vue montre ce que la caméra du plan filme, au format du cadre.", "espace3d.vue_camera").clicked() {
                self.espace3d.through_camera = !self.espace3d.through_camera;
            }
            let some = self.espace3d.sel.iter().any(|i| *i != CAMERA_ID);
            if crate::bulles::icone_si(ui, some, ph::CROSSHAIR, "Viser la sélection", "Tourne la caméra vers les cartes choisies.", "").clicked() {
                let ids: Vec<u64> = self.espace3d.sel.iter().copied().filter(|i| *i != CAMERA_ID).collect();
                let target = gz::pivot_of(&self.e3_xfs(&ids));
                let mut sc = world.scene_cam(&self.current_camera());
                sc.aim_at(target);
                let aimed = Camera { pan: sc.pan, tilt: sc.tilt, ..self.current_camera() };
                self.espace3d_step("Viser avec la caméra", |s| s.set_current_camera(aimed));
            }
        });
    }
}
