//! Scène 3D de blocage dans l'application : le mode « Scène 3D » (une vue d'édition à la place du
//! dessin), ses commandes, le panneau Réglages, l'annulation, et l'envoi de la scène vers un calque
//! de fond (rendu par le processeur sur un autre fil, jamais pendant qu'on dessine).
//!
//! La vue elle-même (souris, manipulateurs, tracé) est dans `scene3dview.rs`.

use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use eframe::egui::{self, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::camera::{focal_label, FOCAL_PRESETS};
use crate::document::{Layer, LayerKind};
use crate::history::Edit;
use crate::raster::Raster;
use crate::scene3d::{Mannequin, Obj3d, Pose, RenderArea, RenderStyle, SceneCam, Shape3d, Target, Xf, JOINTS, JOINT_NAMES, MAX_FOCAL, MAX_STEPS, MIN_FOCAL, MIN_STEPS};
use crate::scene3dgizmo::{self as gz, Gizmo, Handle, Mode as GizmoMode, Originals, CAMERA_ID};
use crate::scene3dgpu::Gpu3d;
use crate::scene3dmath::{Ray, V3};
use crate::scene3dmesh::MeshLib;
use crate::view3d::Cam3d;

/// Nom du calque où la scène est envoyée.
pub const LAYER_NAME: &str = "Fond 3D";

/// Un geste en cours dans la vue.
pub(crate) enum Drag {
    /// Tourner autour de la scène (la vue d'édition, pas un objet).
    Orbit,
    /// Une poignée (ou un objet tiré directement) : les placements d'origine, et de quoi les refaire.
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

/// Ce dont dépend l'image envoyée au calque : si rien de cela ne change, elle est à jour.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BakeKey {
    pub panel: u64,
    pub rev: u64,
    pub stage: [usize; 2],
    pub frame: [usize; 2],
    pub cel: u64,
}

pub(crate) struct BakeJob {
    key: BakeKey,
    handle: JoinHandle<Raster>,
}

/// État de la scène 3D dans l'application.
pub(crate) struct Scene3dState {
    pub on: bool,
    pub mode: GizmoMode,
    /// Manipulateur dans le repère de l'objet (un seul objet choisi).
    pub local: bool,
    pub target: Target,
    /// Objets choisis (`CAMERA_ID` : la caméra du plateau).
    pub sel: Vec<u64>,
    /// Vue d'édition.
    pub cam: Option<Cam3d>,
    /// « Vue par la caméra » : la vue est celle de la caméra du plateau, au format du cadre.
    pub through_camera: bool,
    pub show_grid: bool,
    pub snap: bool,
    pub snap_floor: bool,
    pub step_move: f32,
    pub step_rot: f32,
    pub step_scale: f32,
    /// Articulation affichée dans le panneau (mannequin).
    pub joint: usize,
    pub drag: Option<Drag>,
    pub meshes: MeshLib,
    pub gpu: Option<Gpu3d>,
    pub hover: Option<Handle>,
    pub job: Option<BakeJob>,
    /// Image finie, en attente d'un moment où l'on ne dessine pas.
    pub ready: Option<(BakeKey, Raster)>,
    /// Ce que le calque montre en ce moment.
    pub applied: Option<BakeKey>,
    /// Un envoi demandé à la main même si « Suivre » est éteint.
    pub force: bool,
    /// Nombre d'images rendues et posées dans un calque depuis le début (essais).
    pub baked: u64,
    /// Plan de la dernière image (la sélection ne passe pas d'un plan à l'autre).
    pub last_panel: u64,
}

impl Default for Scene3dState {
    fn default() -> Scene3dState {
        Scene3dState {
            on: false,
            mode: GizmoMode::Move,
            local: false,
            target: Target::Plan,
            sel: Vec::new(),
            cam: None,
            through_camera: false,
            show_grid: true,
            snap: true,
            snap_floor: true,
            step_move: 0.25,
            step_rot: 15.0,
            step_scale: 0.1,
            joint: 0,
            drag: None,
            meshes: MeshLib::default(),
            gpu: None,
            hover: None,
            job: None,
            ready: None,
            applied: None,
            force: false,
            baked: 0,
            last_panel: 0,
        }
    }
}

/// Les shapes proposées en haut du panneau, avec leur icône.
fn shape_icon(shape: &Shape3d) -> &'static str {
    match shape {
        Shape3d::Cube => ph::CUBE,
        Shape3d::Plane => ph::SQUARE,
        Shape3d::Cylinder => ph::CYLINDER,
        Shape3d::Sphere => ph::SPHERE,
        Shape3d::Cone => ph::TRIANGLE,
        Shape3d::Stairs { .. } => ph::STAIRS,
        Shape3d::Mannequin(_) => ph::PERSON_SIMPLE,
        Shape3d::Model { .. } => ph::CUBE_FOCUS,
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

fn mode_description(mode: GizmoMode) -> &'static str {
    match mode {
        GizmoMode::Select => "Cliquer un objet pour le choisir (Maj : en ajouter). Glisser un objet : le poser ailleurs sur le sol (Maj : le monter).",
        GizmoMode::Move => "Flèches : déplacer le long d'un axe ; carrés : dans un plan.",
        GizmoMode::Rotate => "Cercles : tourner autour d'un axe.",
        GizmoMode::Scale => "Cubes : étirer le long d'un axe ; cube du centre : tout ensemble.",
    }
}

impl App {
    /// Numéros du plan courant et de sa séquence.
    pub(crate) fn s3_ids(&self) -> (u64, u64) {
        (self.doc.panels[self.cur_panel].id, self.doc.sequence_of(self.cur_panel))
    }

    /// Nouveau projet, ou feuille changée : la sélection, la vue et le calcul en cours sont oubliés.
    pub(crate) fn scene3d_reset(&mut self) {
        let s = &mut self.scene3d;
        s.sel.clear();
        s.cam = None;
        s.drag = None;
        s.job = None;
        s.ready = None;
        s.applied = None;
        s.force = false;
        s.last_panel = 0;
    }

    /// Entre dans la scène 3D, ou en sort.
    pub(crate) fn set_scene3d(&mut self, on: bool) {
        if on {
            self.set_camera_view(false);
            self.end_stroke();
            self.stage3d.on = false;
            self.playing = false;
        }
        self.scene3d.on = on;
        self.scene3d.drag = None;
        self.status = if on {
            "Scène 3D : clic sur un objet pour le choisir, glisser pour le poser, clic droit pour tourner la vue, bouton du milieu pour la glisser, molette pour avancer. Réglages : panneau Réglages.".into()
        } else {
            "Scène 3D quittée.".into()
        };
    }

    /// Fait `f` comme une étape d'annulation nommée `name`.
    pub(crate) fn scene3d_step<R>(&mut self, name: &str, f: impl FnOnce(&mut Self) -> R) -> R {
        self.end_stroke();
        let snap = self.snapshot();
        let r = f(self);
        let before = self.history.context.replace(name.to_string());
        self.history.push(snap);
        self.history.context = before;
        self.modified = true;
        r
    }

    /// Les placements des objets `ids` (la caméra comprise).
    pub(crate) fn s3_xfs(&self, ids: &[u64]) -> Originals {
        let (panel, seq) = self.s3_ids();
        ids.iter()
            .filter_map(|&id| {
                if id == CAMERA_ID {
                    return Some((id, gz::cam_xf(&self.doc.scenes3d.plan(panel)?.cam)));
                }
                self.doc.scenes3d.find(panel, seq, id).map(|(o, _)| (id, o.xf))
            })
            .collect()
    }

    /// Pose ces placements (sans étape d'annulation : l'appelant s'en charge).
    pub(crate) fn s3_set_xfs(&mut self, new: &[(u64, Xf)]) {
        let (panel, seq) = self.s3_ids();
        let scenes = &mut self.doc.scenes3d;
        for (id, xf) in new {
            if *id == CAMERA_ID {
                let plan = scenes.plan_mut(panel);
                plan.cam = gz::cam_from_xf(&plan.cam, xf);
            } else if let Some(o) = scenes.find_mut(panel, seq, *id).filter(|o| !o.locked) {
                // (Un objet verrouillé ne bouge pas, même avec les poignées d'un groupe.)
                o.xf = xf.sanitized();
            }
        }
        scenes.touch();
        self.modified = true;
    }

    /// Ajoute un objet de la forme `shape` au centre de la vue, sur le sol, et le choisit.
    pub(crate) fn scene3d_add(&mut self, shape: Shape3d) -> Option<u64> {
        let (panel, seq) = self.s3_ids();
        let at = self.scene3d.cam.map_or([0.0; 3], |c| [(c.target[0] * 4.0).round() / 4.0, 0.0, (c.target[2] * 4.0).round() / 4.0]);
        let target = self.scene3d.target;
        let name = format!("Ajouter : {}", shape.label().to_lowercase());
        let id = self.scene3d_step(&name, |s| s.doc.scenes3d.add(panel, seq, target, shape, at));
        match id {
            Some(id) => {
                self.scene3d.sel = vec![id];
                self.status = "Objet ajouté. Déplacer : flèches et carrés ; tourner : cercles.".into();
            }
            None => {
                // (Rien n'a changé : l'étape inutile est retirée.)
                self.undo();
                self.status = format!("Trop d'objets dans la scène ({}).", crate::scene3d::MAX_OBJECTS);
            }
        }
        id
    }

    /// Les objets choisis (copies), la caméra non comprise.
    pub(crate) fn scene3d_selected(&self) -> Vec<Obj3d> {
        let (panel, seq) = self.s3_ids();
        self.scene3d.sel.iter().filter_map(|id| self.doc.scenes3d.find(panel, seq, *id).map(|(o, _)| o.clone())).collect()
    }

    pub(crate) fn scene3d_delete_selection(&mut self) {
        let ids: Vec<u64> = self.scene3d.sel.iter().copied().filter(|id| *id != CAMERA_ID).collect();
        if ids.is_empty() {
            self.status = "Aucun objet choisi.".into();
            return;
        }
        let (panel, seq) = self.s3_ids();
        let n = self.scene3d_step("Supprimer des objets 3D", |s| s.doc.scenes3d.delete(panel, seq, &ids));
        self.scene3d.sel.clear();
        self.status = format!("{n} objet(s) supprimé(s).");
    }

    pub(crate) fn scene3d_duplicate_selection(&mut self) {
        let ids: Vec<u64> = self.scene3d.sel.iter().copied().filter(|id| *id != CAMERA_ID).collect();
        if ids.is_empty() {
            self.status = "Aucun objet choisi.".into();
            return;
        }
        let (panel, seq) = self.s3_ids();
        let copies = self.scene3d_step("Dupliquer des objets 3D", |s| s.doc.scenes3d.duplicate(panel, seq, &ids, [0.6, 0.0, 0.6]));
        self.status = format!("{} objet(s) dupliqué(s).", copies.len());
        if !copies.is_empty() {
            self.scene3d.sel = copies;
        }
    }

    pub(crate) fn scene3d_group_selection(&mut self, group: bool) {
        let ids: Vec<u64> = self.scene3d.sel.iter().copied().filter(|id| *id != CAMERA_ID).collect();
        let (panel, seq) = self.s3_ids();
        if group {
            if ids.len() < 2 {
                self.status = "Choisis au moins deux objets pour les grouper.".into();
                return;
            }
            self.scene3d_step("Grouper des objets 3D", |s| s.doc.scenes3d.group(panel, seq, &ids));
            self.status = "Objets groupés : ils se choisissent et se déplacent ensemble.".into();
        } else {
            if ids.is_empty() {
                return;
            }
            let n = self.scene3d_step("Dégrouper des objets 3D", |s| s.doc.scenes3d.ungroup(panel, seq, &ids));
            self.status = format!("{n} objet(s) dégroupé(s).");
        }
    }

    /// Importe un modèle (glTF) comme objet de la scène.
    pub(crate) fn scene3d_import_model_path(&mut self, path: &std::path::Path) {
        let mesh = match crate::model3d::load_gltf(path) {
            Ok(m) => m,
            Err(e) => {
                self.status = format!("Import du modèle impossible : {e}");
                return;
            }
        };
        let name = path.file_name().map_or_else(|| "modèle".into(), |n| n.to_string_lossy().into_owned());
        let (panel, seq) = self.s3_ids();
        let target = self.scene3d.target;
        let id = self.scene3d_step("Importer un modèle dans la scène 3D", |s| {
            let model = s.doc.add_model(name.clone(), mesh);
            let id = s.doc.scenes3d.add(panel, seq, target, Shape3d::Model { id: model }, [0.0; 3])?;
            if let Some(o) = s.doc.scenes3d.find_mut(panel, seq, id) {
                o.name = name.clone();
            }
            Some(id)
        });
        if let Some(id) = id {
            self.scene3d.sel = vec![id];
            self.status = format!("Modèle « {name} » ajouté à la scène.");
        }
    }

    fn scene3d_import_model(&mut self) {
        self.end_stroke();
        let Some(path) = rfd::FileDialog::new().set_title("Importer un modèle 3D (glTF) dans la scène").add_filter("Modèles 3D glTF", &["glb", "gltf"]).pick_file() else { return };
        self.scene3d_import_model_path(&path);
    }

    /// Cadre la vue d'édition sur la scène (ou sur la caméra du plateau si la scène est vide).
    pub(crate) fn scene3d_fit(&mut self) {
        let (panel, seq) = self.s3_ids();
        let mut points: Vec<V3> = Vec::new();
        for (o, _) in self.doc.scenes3d.visible_objects(panel, seq) {
            let (_, mesh) = self.scene3d.meshes.get_for_view(&o.shape, &self.doc.models);
            let b = crate::scene3dmesh::world_bounds(&mesh, &o.xf.matrix());
            points.extend([b.0, b.1]);
        }
        if let Some(plan) = self.doc.scenes3d.plan(panel) {
            points.push(plan.cam.pos);
        }
        let bounds = crate::scene3dmath::bounds_of(points).filter(|b| crate::scene3dmath::length(crate::scene3dmath::sub(b.1, b.0)) > 0.1).unwrap_or(([-3.0, 0.0, -3.0], [3.0, 2.0, 3.0]));
        let rect = self.canvas_rect;
        let mut cam = Cam3d::fit_at(bounds, [rect.width().max(1.0), rect.height().max(1.0)], 30.0, 22.0);
        cam.distance = cam.distance.max(2.0);
        self.scene3d.cam = Some(cam);
    }

    /// La caméra 2D du plan à l'image courante, posée dans la scène (une copie, pas un lien).
    pub(crate) fn scene3d_camera_from_plan(&mut self) {
        let (panel, _) = self.s3_ids();
        let cam2d = self.current_camera();
        let (stage, frame) = ([self.doc.width, self.doc.height], self.doc.frame);
        self.scene3d_step("Caler la caméra 3D sur la caméra du plan", |s| {
            s.doc.scenes3d.plan_mut(panel).cam = SceneCam::from_plan_camera(&cam2d, stage, frame);
            s.doc.scenes3d.touch();
        });
        self.status = "Caméra du plateau calée sur la caméra 2D du plan (position, focale, panoramique, bascule, rotation). Les clés de la caméra 2D ne sont pas suivies.".into();
    }

    /// Les commandes de la scène 3D. Vrai si l'action est la sienne.
    pub(crate) fn scene3d_action(&mut self, action: &str) -> bool {
        if !action.starts_with("scene3d.") {
            // Suppr, quand la vue 3D a la main (le dernier clic n'était ni dans le montage, ni dans la
            // timeline, ni dans les effets), supprime les objets choisis.
            let canvas_has_the_hand = !self.panels_have_focus() && !self.images_focus && !self.key_focus && !self.fx_pane_focus;
            if self.scene3d.on && !self.camera_view && canvas_has_the_hand && action == "edition.supprimer" && !self.scene3d.sel.is_empty() {
                self.scene3d_delete_selection();
                return true;
            }
            return false;
        }
        match action {
            "scene3d.basculer" => self.set_scene3d(!(self.scene3d.on && !self.camera_view)),
            "scene3d.outil_choisir" => self.scene3d_mode(GizmoMode::Select),
            "scene3d.outil_deplacer" => self.scene3d_mode(GizmoMode::Move),
            "scene3d.outil_tourner" => self.scene3d_mode(GizmoMode::Rotate),
            "scene3d.outil_echelle" => self.scene3d_mode(GizmoMode::Scale),
            "scene3d.ajouter_cube" => self.scene3d_add_shape(Shape3d::Cube),
            "scene3d.ajouter_plan" => self.scene3d_add_shape(Shape3d::Plane),
            "scene3d.ajouter_cylindre" => self.scene3d_add_shape(Shape3d::Cylinder),
            "scene3d.ajouter_sphere" => self.scene3d_add_shape(Shape3d::Sphere),
            "scene3d.ajouter_cone" => self.scene3d_add_shape(Shape3d::Cone),
            "scene3d.ajouter_escalier" => self.scene3d_add_shape(Shape3d::Stairs { steps: 5 }),
            "scene3d.ajouter_mannequin" => self.scene3d_add_shape(Shape3d::Mannequin(Box::new(Mannequin::new(Pose::Standing, 1.75)))),
            "scene3d.importer_modele" => {
                self.set_scene3d(true);
                self.scene3d_import_model();
            }
            "scene3d.dupliquer" => self.scene3d_duplicate_selection(),
            "scene3d.supprimer" => self.scene3d_delete_selection(),
            "scene3d.grouper" => self.scene3d_group_selection(true),
            "scene3d.degrouper" => self.scene3d_group_selection(false),
            "scene3d.vue_camera" => {
                self.set_scene3d(true);
                self.scene3d.through_camera = !self.scene3d.through_camera;
            }
            "scene3d.cadrer" => {
                self.set_scene3d(true);
                self.scene3d_fit();
            }
            "scene3d.envoyer" => self.scene3d_send_to_layer(),
            "scene3d.camera_depuis_plan" => self.scene3d_camera_from_plan(),
            "scene3d.aimanter" => {
                self.scene3d.snap = !self.scene3d.snap;
                self.status = if self.scene3d.snap { "Aimantation à la grille et au sol : allumée.".into() } else { "Aimantation : éteinte.".into() };
            }
            "scene3d.grille" => self.scene3d.show_grid = !self.scene3d.show_grid,
            "scene3d.decor" => {
                self.scene3d.target = if self.scene3d.target == Target::Plan { Target::Decor } else { Target::Plan };
                self.scene3d.sel.clear();
            }
            _ => return false,
        }
        true
    }

    fn scene3d_mode(&mut self, mode: GizmoMode) {
        if !self.scene3d.on {
            self.set_scene3d(true);
        }
        self.scene3d.mode = mode;
    }

    fn scene3d_add_shape(&mut self, shape: Shape3d) {
        if !self.scene3d.on {
            self.set_scene3d(true);
        }
        self.scene3d_add(shape);
    }

    // --------------------------------------------------------------- envoi vers un calque

    /// Le calque du plan courant qui montre le dessin `cel`.
    fn layer_of_cel(&self, cel: u64) -> Option<usize> {
        self.layers().iter().position(|l| l.frames.iter().flatten().any(|c| *c == cel))
    }

    /// « Envoyer vers un calque de fond » : la scène, vue par la caméra du plateau, devient le dessin
    /// d'un calque « Fond 3D » tout en bas du plan (bitmap ordinaire, verrouillé pour qu'on ne dessine
    /// pas dessus). Si le plan a déjà ce calque, c'est lui qui est remis à jour. Ensuite il suit la
    /// scène (option « Suivre ») : voir `update_scene3d`.
    pub(crate) fn scene3d_send_to_layer(&mut self) {
        self.end_stroke();
        self.playing = false;
        let (panel, _) = self.s3_ids();
        let existing = self.doc.scenes3d.plan(panel).and_then(|p| p.bake_cel).filter(|c| self.layer_of_cel(*c).is_some());
        let created = existing.is_none();
        self.scene3d_step("Envoyer la scène 3D vers un calque de fond", |s| {
            let cel = match existing {
                Some(c) => c,
                None => {
                    let length = s.doc.panels[s.cur_panel].frame_count();
                    let cel = s.doc.new_cel(LayerKind::Bitmap);
                    let mut layer = Layer::new(LAYER_NAME, LayerKind::Bitmap);
                    layer.locked = true;
                    layer.insert(0, Some(cel), length);
                    s.doc.panels[s.cur_panel].layers.insert(0, layer);
                    s.cur_layer += 1;
                    cel
                }
            };
            let plan = s.doc.scenes3d.plan_mut(panel);
            plan.bake_cel = Some(cel);
            plan.follow = true;
            s.doc.scenes3d.touch();
        });
        self.scene3d.applied = None;
        self.scene3d.force = true;
        self.status = if created {
            format!("Scène envoyée vers le calque « {LAYER_NAME} » (tout en bas, verrouillé) : l'image se calcule en arrière-plan ; il suit la scène tant que « Suivre » est coché.")
        } else {
            format!("Calque « {LAYER_NAME} » remis à jour en arrière-plan.")
        };
    }

    /// L'image de la scène du plan courant dépend de ceci.
    pub(crate) fn bake_key(&self) -> Option<BakeKey> {
        let (panel, _) = self.s3_ids();
        let plan = self.doc.scenes3d.plan(panel)?;
        let cel = plan.bake_cel?;
        Some(BakeKey { panel, rev: self.doc.scenes3d.rev, stage: [self.doc.width, self.doc.height], frame: self.doc.frame, cel })
    }

    /// Une fois par image, avant d'afficher : pose l'image finie dans le calque, et lance le calcul si
    /// la scène a changé depuis le dernier. Rien ne se fait pendant un trait ou un geste dans la vue
    /// 3D : le dessin garde le processeur. Au repos, cette fonction ne demande aucun affichage.
    pub(crate) fn update_scene3d(&mut self, ctx: &egui::Context) {
        let (panel, seq) = self.s3_ids();
        if self.scene3d.last_panel != panel {
            self.scene3d.last_panel = panel;
            self.scene3d.sel.clear();
            self.scene3d.cam = None;
        }
        // Un calcul fini attend son moment.
        if self.scene3d.job.as_ref().is_some_and(|j| j.handle.is_finished()) {
            if let Some(job) = self.scene3d.job.take() {
                if let Ok(raster) = job.handle.join() {
                    self.scene3d.ready = Some((job.key, raster));
                }
            }
        }
        let busy = self.stroke.is_some() || self.vstroke.is_some() || self.scene3d.drag.is_some() || self.playing;
        let want = self.bake_key();
        if self.scene3d.ready.is_some() && !busy {
            if let Some((key, raster)) = self.scene3d.ready.take() {
                // Périmée (la scène a encore changé, ou un autre plan) : on la jette.
                if Some(&key) == want.as_ref() {
                    if let Some(cel) = self.doc.cels.get_mut(&key.cel) {
                        cel.raster = raster;
                        cel.version += 1;
                        self.renderer.invalidate_all();
                        self.scene3d.applied = Some(key);
                        self.scene3d.baked += 1;
                    }
                }
            }
        }
        let Some(want) = want else { return };
        let follow = self.doc.scenes3d.plan(panel).is_some_and(|p| p.follow);
        if !(follow || self.scene3d.force) || self.scene3d.applied.as_ref() == Some(&want) {
            return;
        }
        if self.layer_of_cel(want.cel).is_none() {
            return;
        }
        if self.scene3d.job.is_some() {
            // Un calcul est en cours (peut-être pour une scène déjà dépassée) : on attend sa fin.
            ctx.request_repaint_after(Duration::from_millis(40));
            return;
        }
        if self.scene3d.ready.as_ref().is_some_and(|r| r.0 == want) || busy {
            return;
        }
        self.scene3d.force = false;
        let scenes = self.doc.scenes3d.clone();
        let models: std::collections::HashMap<u64, Arc<crate::model3d::Model3d>> = self.doc.models.clone();
        let (stage, frame) = (want.stage, want.frame);
        // La moitié des cœurs : le dessin garde le reste.
        let threads = (std::thread::available_parallelism().map_or(2, |n| n.get()) / 2).max(1);
        let handle = std::thread::spawn(move || {
            // Lissage des bords : moins d'échantillons quand la feuille est très grande (4K : 33 millions de pixels).
            let ss = match stage[0] * stage[1] {
                0..=2_500_000 => 3,
                2_500_001..=12_000_000 => 2,
                _ => 1,
            };
            let plate = crate::scene3drender::render_plate(&scenes, panel, seq, &models, stage, frame, ss, threads);
            crate::scene3drender::plate_raster(&plate, stage)
        });
        self.scene3d.job = Some(BakeJob { key: want, handle });
        ctx.request_repaint_after(Duration::from_millis(40));
    }

    // ------------------------------------------------------------------------- barre d'outils

    /// Outils de la scène 3D, dans la barre au-dessus du dessin.
    pub(crate) fn scene3d_tools(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        for mode in GizmoMode::ALL {
            if self.bouton_etat(ui, self.scene3d.mode == mode, mode_icon(mode), mode.label(), mode_description(mode), mode.action()).clicked() {
                self.scene3d.mode = mode;
            }
        }
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);
        for shape in Shape3d::basics() {
            if self.bouton(ui, shape_icon(&shape), &format!("Ajouter : {}", shape.label().to_lowercase()), "Posé sur le sol, au centre de la vue.", shape.action()).clicked() {
                self.scene3d_add(shape);
            }
        }
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);
        if self.bouton(ui, ph::CORNERS_OUT, "Cadrer", "Toute la scène dans la vue.", "scene3d.cadrer").clicked() {
            self.scene3d_fit();
        }
        if self.bouton_etat(ui, self.scene3d.through_camera, ph::VIDEO_CAMERA, "Vue par la caméra", "Ce que la caméra du plateau filme, au format du cadre. Glisser ou tourner la vue déplace alors la caméra.", "scene3d.vue_camera").clicked() {
            self.scene3d.through_camera = !self.scene3d.through_camera;
        }
        if self.bouton(ui, ph::IMAGE, "Envoyer vers un calque de fond", "Rend la scène par la caméra du plateau dans le calque « Fond 3D », sous les calques de dessin. Il se met à jour tout seul quand la scène change.", "scene3d.envoyer").clicked() {
            self.scene3d_send_to_layer();
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

    /// Réglages de la scène 3D (panneau Réglages quand la vue 3D est ouverte).
    pub(crate) fn scene3d_options_panel(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::{bulle, choix, interrupteur};
        let (panel, seq) = self.s3_ids();
        let note = |ui: &mut egui::Ui, text: &str| {
            ui.label(RichText::new(text).small().weak());
        };
        ui.horizontal(|ui| {
            ui.heading(format!("{}  Scène 3D", ph::CUBE));
            ui.label(RichText::new("blocage du plan").small().weak());
        });
        // Plan ou décor.
        ui.horizontal(|ui| {
            ui.label("Modifier");
            let before = self.scene3d.target;
            choix(ui, &mut self.scene3d.target, Target::Plan, ph::FILM_SLATE, "Les objets du plan", "Personnages et accessoires de ce plan seulement.", "");
            choix(ui, &mut self.scene3d.target, Target::Decor, ph::MOUNTAINS, "Le décor de la séquence", "Le décor est commun à tous les plans de la séquence qui l'utilisent : le modifier ici le change partout.", "");
            if before != self.scene3d.target {
                self.scene3d.sel.clear();
            }
        });
        let mut use_decor = self.doc.scenes3d.plan(panel).is_none_or(|p| p.use_decor);
        let r = interrupteur(ui, &mut use_decor, ph::MOUNTAINS, "Montrer le décor de la séquence", "Ce plan montre le décor de sa séquence (estompé quand on modifie le plan).", "");
        if r.changed() {
            self.scene3d_step("Montrer ou cacher le décor 3D", |s| {
                s.doc.scenes3d.plan_mut(panel).use_decor = use_decor;
                s.doc.scenes3d.touch();
            });
        }
        ui.separator();

        // Ajouter.
        ui.horizontal_wrapped(|ui| {
            for shape in Shape3d::basics() {
                if self.bouton(ui, shape_icon(&shape), &format!("Ajouter : {}", shape.label().to_lowercase()), "Posé sur le sol, au centre de la vue.", shape.action()).clicked() {
                    self.scene3d_add(shape);
                }
            }
            if self.bouton(ui, ph::FILE_ARROW_DOWN, "Importer un modèle", "Un modèle glTF (.glb, .gltf) comme objet de la scène.", "scene3d.importer_modele").clicked() {
                self.scene3d_import_model();
            }
        });
        // Un modèle déjà importé dans le projet.
        let mut models: Vec<(u64, String)> = self.doc.models.values().map(|m| (m.id, m.name.clone())).collect();
        models.sort();
        if !models.is_empty() {
            ui.horizontal(|ui| {
                ui.label("Modèle du projet");
                let mut picked = None;
                egui::ComboBox::from_id_salt("scene3d_modele").selected_text("Ajouter…").width(120.0).show_ui(ui, |ui| {
                    for (id, name) in &models {
                        if ui.selectable_label(false, name).clicked() {
                            picked = Some((*id, name.clone()));
                        }
                    }
                });
                if let Some((id, name)) = picked {
                    let target = self.scene3d.target;
                    let added = self.scene3d_step("Ajouter un modèle à la scène 3D", |s| {
                        let id = s.doc.scenes3d.add(panel, seq, target, Shape3d::Model { id }, [0.0; 3])?;
                        if let Some(o) = s.doc.scenes3d.find_mut(panel, seq, id) {
                            o.name = name;
                        }
                        Some(id)
                    });
                    if let Some(id) = added {
                        self.scene3d.sel = vec![id];
                    }
                }
            });
        }
        // Outil, repère, aimantation.
        ui.horizontal(|ui| {
            for mode in GizmoMode::ALL {
                let r = self.bouton_etat(ui, self.scene3d.mode == mode, mode_icon(mode), mode.label(), mode_description(mode), mode.action());
                if r.clicked() {
                    self.scene3d.mode = mode;
                }
            }
            interrupteur(ui, &mut self.scene3d.local, ph::CUBE_FOCUS, "Manipulateur dans le repère de l'objet", "Les axes suivent l'objet (un seul objet choisi) au lieu de ceux de la scène.", "");
        });
        ui.horizontal(|ui| {
            let r = self.bouton_etat(ui, self.scene3d.snap, ph::MAGNET, "Aimanter à la grille et au sol", "Déplacements au pas de la grille, rotations par paliers, objets posés sur le sol quand ils en sont proches.", "scene3d.aimanter");
            if r.clicked() {
                self.scene3d.snap = !self.scene3d.snap;
            }
            interrupteur(ui, &mut self.scene3d.snap_floor, ph::ARROW_LINE_DOWN,"Poser sur le sol", "Un objet qui passe près du sol s'y pose.", "");
            let r = self.bouton_etat(ui, self.scene3d.show_grid, ph::GRID_FOUR, "Grille du sol", "Quadrillage au sol, un trait plus marqué tous les cinq.", "scene3d.grille");
            if r.clicked() {
                self.scene3d.show_grid = !self.scene3d.show_grid;
            }
        });
        if self.scene3d.snap {
            ui.horizontal(|ui| {
                let r = ui.add(egui::DragValue::new(&mut self.scene3d.step_move).speed(0.01).range(0.01..=10.0).suffix(" m").prefix("pas "));
                bulle(r, "Pas de la grille", "Déplacements aimantés à ce pas.", "");
                let r = ui.add(egui::DragValue::new(&mut self.scene3d.step_rot).speed(0.5).range(1.0..=90.0).suffix("°"));
                bulle(r, "Pas de rotation", "Rotations aimantées à ce pas.", "");
                let r = ui.add(egui::DragValue::new(&mut self.scene3d.step_scale).speed(0.01).range(0.01..=2.0));
                bulle(r, "Pas de taille", "Tailles aimantées à ce pas.", "");
            });
        }
        ui.separator();

        // Liste des objets.
        let target = self.scene3d.target;
        let list: Vec<(u64, String, Shape3d, bool, bool)> =
            self.doc.scenes3d.list(panel, seq, target).iter().map(|o| (o.id, o.name.clone(), o.shape.clone(), o.hidden, o.locked)).collect();
        ui.label(RichText::new(format!("{} objet(s) {}", list.len(), if target == Target::Decor { "dans le décor" } else { "dans le plan" })).small().weak());
        egui::ScrollArea::vertical().id_salt("scene3d_liste").max_height(150.0).auto_shrink([true, true]).show(ui, |ui| {
            for (id, name, shape, hidden, locked) in &list {
                ui.horizontal(|ui| {
                    let selected = self.scene3d.sel.contains(id);
                    let mut hide = *hidden;
                    if ui.small_button(if hide { ph::EYE_SLASH } else { ph::EYE }).on_hover_text("Montrer ou cacher l'objet").clicked() {
                        hide = !hide;
                        let id = *id;
                        self.scene3d_step("Montrer ou cacher un objet 3D", |s| {
                            if let Some(o) = s.doc.scenes3d.find_mut(panel, seq, id) {
                                o.hidden = hide;
                            }
                            s.doc.scenes3d.touch();
                        });
                    }
                    if ui.small_button(if *locked { ph::LOCK } else { ph::LOCK_OPEN }).on_hover_text("Verrouiller l'objet : on ne le choisit plus à la souris").clicked() {
                        let (id, lock) = (*id, !*locked);
                        self.scene3d_step("Verrouiller un objet 3D", |s| {
                            if let Some(o) = s.doc.scenes3d.find_mut(panel, seq, id) {
                                o.locked = lock;
                            }
                            s.doc.scenes3d.touch();
                        });
                    }
                    let r = ui.selectable_label(selected, format!("{} {}", shape_icon(shape), name));
                    if r.clicked() {
                        let additive = ui.input(|i| i.modifiers.shift || i.modifiers.command);
                        let ids = self.doc.scenes3d.expand_groups(panel, seq, &[*id]);
                        if additive {
                            for i in ids {
                                if let Some(p) = self.scene3d.sel.iter().position(|s| *s == i) {
                                    self.scene3d.sel.remove(p);
                                } else {
                                    self.scene3d.sel.push(i);
                                }
                            }
                        } else {
                            self.scene3d.sel = ids;
                        }
                    }
                });
            }
        });
        ui.horizontal(|ui| {
            let some = self.scene3d.sel.iter().any(|i| *i != CAMERA_ID);
            if self.bouton_si(ui, some, ph::COPY, "Dupliquer", "Copie les objets choisis, décalés.", "scene3d.dupliquer").clicked() {
                self.scene3d_duplicate_selection();
            }
            if self.bouton_si(ui, some, ph::TRASH, "Supprimer", "Retire les objets choisis (Ctrl+Z les rend).", "scene3d.supprimer").clicked() {
                self.scene3d_delete_selection();
            }
            if self.bouton_si(ui, self.scene3d.sel.len() >= 2, ph::UNITE, "Grouper", "Les objets choisis se choisissent et se manient ensemble.", "scene3d.grouper").clicked() {
                self.scene3d_group_selection(true);
            }
            if self.bouton_si(ui, some, ph::LINK_BREAK, "Dégrouper", "Défait le groupe des objets choisis.", "scene3d.degrouper").clicked() {
                self.scene3d_group_selection(false);
            }
        });

        // L'objet choisi.
        let selected = self.scene3d_selected();
        if let [obj] = selected.as_slice() {
            ui.separator();
            self.scene3d_object_fields(ui, obj.clone());
        } else if selected.len() > 1 {
            ui.separator();
            ui.label(RichText::new(format!("{} objets choisis", selected.len())).strong());
            note(ui, "Ils se déplacent, tournent et grandissent ensemble (échelle uniforme) ; choisis-en un seul pour ses réglages.");
        }

        ui.separator();
        self.scene3d_camera_fields(ui, panel);
        ui.separator();
        self.scene3d_light_and_render_fields(ui, panel);
    }

    /// Les réglages d'un objet : nom, placement, couleur, forme.
    fn scene3d_object_fields(&mut self, ui: &mut egui::Ui, obj: Obj3d) {
        let (panel, seq) = self.s3_ids();
        let mut o = obj.clone();
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label(shape_icon(&o.shape));
            let r = ui.add(egui::TextEdit::singleline(&mut o.name).desired_width(140.0));
            changed |= self.edited3d(&r, "Renommer l'objet 3D");
            let r = ui.color_edit_button_srgb(&mut o.color);
            changed |= self.edited3d(&r, "Colorier l'objet 3D");
            let r = crate::bulles::interrupteur(ui, &mut o.gray, ph::CIRCLE_HALF, "Ombrer en niveaux de gris", "Cet objet est ombré en gris, quel que soit le style du plan.", "");
            changed |= self.edited3d(&r, "Ombrage de l'objet 3D");
        });
        egui::Grid::new("scene3d_objet").num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
            ui.label("Position");
            ui.horizontal(|ui| changed |= self.vec3_fields(ui, &mut o.xf.pos, 0.02, "Placer l'objet 3D", -1e5, 1e5));
            ui.end_row();
            ui.label("Rotation");
            ui.horizontal(|ui| changed |= self.vec3_fields(ui, &mut o.xf.rot, 0.5, "Tourner l'objet 3D", -1e4, 1e4));
            ui.end_row();
            ui.label("Taille");
            ui.horizontal(|ui| changed |= self.vec3_fields(ui, &mut o.xf.size, 0.01, "Dimensionner l'objet 3D", -1e4, 1e4));
            ui.end_row();
        });
        match &mut o.shape {
            Shape3d::Stairs { steps } => {
                let r = ui.add(egui::DragValue::new(steps).range(MIN_STEPS..=MAX_STEPS).speed(0.1).suffix(" marches"));
                changed |= self.edited3d(&r, "Marches de l'escalier 3D");
            }
            Shape3d::Mannequin(mq) => {
                let mut mq2 = (**mq).clone();
                changed |= self.mannequin_fields(ui, &mut mq2);
                **mq = mq2;
            }
            _ => {}
        }
        if changed {
            let new = o;
            if let Some(slot) = self.doc.scenes3d.find_mut(panel, seq, obj.id) {
                *slot = Obj3d { id: obj.id, group: obj.group, hidden: obj.hidden, locked: obj.locked, xf: new.xf.sanitized(), shape: new.shape.sanitized(), ..new };
            }
            self.doc.scenes3d.touch();
            self.modified = true;
        }
        if obj.group.is_some() {
            ui.label(RichText::new("Cet objet fait partie d'un groupe.").small().weak());
        }
    }

    fn mannequin_fields(&mut self, ui: &mut egui::Ui, mq: &mut Mannequin) -> bool {
        let mut changed = false;
        ui.horizontal(|ui| {
            let r = ui.add(egui::DragValue::new(&mut mq.height).range(Mannequin::MIN_HEIGHT..=Mannequin::MAX_HEIGHT).speed(0.01).max_decimals(2).suffix(" m").prefix("taille "));
            changed |= self.edited3d(&r, "Taille du mannequin");
            let mut pose = mq.pose;
            egui::ComboBox::from_id_salt("scene3d_pose").selected_text(pose.label()).width(90.0).show_ui(ui, |ui| {
                for p in Pose::PRESETS {
                    ui.selectable_value(&mut pose, p, p.label());
                }
            });
            if pose != mq.pose {
                self.record_named("Pose du mannequin");
                mq.set_pose(pose);
                changed = true;
            }
        });
        ui.horizontal(|ui| {
            let mut j = self.scene3d.joint.min(JOINTS - 1);
            egui::ComboBox::from_id_salt("scene3d_articulation").selected_text(JOINT_NAMES[j]).width(120.0).show_ui(ui, |ui| {
                for (i, name) in JOINT_NAMES.iter().enumerate() {
                    ui.selectable_value(&mut j, i, *name);
                }
            });
            self.scene3d.joint = j;
            let mut rot = mq.joints[j];
            let labels = ["avant", "torsion", "écart"];
            let mut edited = false;
            for k in 0..3 {
                let r = ui.add(egui::DragValue::new(&mut rot[k]).speed(0.5).range(-360.0..=360.0).suffix("°").prefix(format!("{} ", labels[k])));
                edited |= self.edited3d(&r, "Articuler le mannequin");
            }
            if edited {
                mq.set_joint(j, rot);
                changed = true;
            }
        });
        ui.label(RichText::new("Avant : positif porte le bout du membre vers l'avant (le genou qui plie est négatif) ; écart : positif l'éloigne du corps.").small().weak());
        changed
    }

    /// La caméra du plateau : placement, focale (objectifs du plan), vue par la caméra.
    fn scene3d_camera_fields(&mut self, ui: &mut egui::Ui, panel: u64) {
        let mut cam = self.doc.scenes3d.plan(panel).map_or_else(SceneCam::default, |p| p.cam);
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{}  Caméra du plateau", ph::VIDEO_CAMERA)).strong());
            let selected = self.scene3d.sel.contains(&CAMERA_ID);
            if ui.selectable_label(selected, "choisir").on_hover_text("Choisir la caméra pour la déplacer ou la tourner dans la vue").clicked() {
                self.scene3d.sel = vec![CAMERA_ID];
            }
        });
        egui::Grid::new("scene3d_camera").num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
            ui.label("Position");
            ui.horizontal(|ui| changed |= self.vec3_fields(ui, &mut cam.pos, 0.02, "Placer la caméra 3D", -1e4, 1e4));
            ui.end_row();
            ui.label("Panoramique");
            let r = ui.add(egui::DragValue::new(&mut cam.pan).speed(0.3).suffix("°"));
            changed |= self.edited3d(&r, "Orienter la caméra 3D");
            ui.end_row();
            ui.label("Bascule");
            let r = ui.add(egui::DragValue::new(&mut cam.tilt).speed(0.3).range(-crate::scene3d::MAX_TILT..=crate::scene3d::MAX_TILT).suffix("°"));
            changed |= self.edited3d(&r, "Orienter la caméra 3D");
            ui.end_row();
            ui.label("Rotation");
            let r = ui.add(egui::DragValue::new(&mut cam.roll).speed(0.3).suffix("°"));
            changed |= self.edited3d(&r, "Orienter la caméra 3D");
            ui.end_row();
            ui.label("Focale");
            ui.horizontal(|ui| {
                let r = ui.add(egui::DragValue::new(&mut cam.focal).speed(0.5).range(MIN_FOCAL..=MAX_FOCAL).suffix(" mm"));
                changed |= self.edited3d(&r, "Focale de la caméra 3D");
                let current = FOCAL_PRESETS.iter().find(|p| (p.mm - cam.focal).abs() < 0.01).map_or_else(|| format!("{:.0}°", cam.angle_of_view()), |p| p.name.to_string());
                egui::ComboBox::from_id_salt("scene3d_objectif").selected_text(current).width(110.0).show_ui(ui, |ui| {
                    for p in FOCAL_PRESETS.iter().filter(|p| !p.anamorphic) {
                        if ui.selectable_label((p.mm - cam.focal).abs() < 0.01, focal_label(p)).on_hover_text(p.note).clicked() {
                            cam.focal = p.mm;
                            changed = true;
                            self.record_named("Objectif de la caméra 3D");
                        }
                    }
                });
            });
            ui.end_row();
        });
        ui.label(RichText::new(format!("Champ horizontal {:.0}° (pellicule de 36 mm, comme la caméra 2D).", cam.angle_of_view())).small().weak());
        if changed {
            let cam = cam.sanitized();
            self.doc.scenes3d.plan_mut(panel).cam = cam;
            self.doc.scenes3d.touch();
            self.modified = true;
        }
        ui.horizontal(|ui| {
            if self.bouton_etat(ui, self.scene3d.through_camera, ph::VIDEO_CAMERA, "Vue par la caméra", "La vue montre ce que la caméra filme, au format du cadre.", "scene3d.vue_camera").clicked() {
                self.scene3d.through_camera = !self.scene3d.through_camera;
            }
            if self.bouton(ui, ph::FRAME_CORNERS, "Caler sur la caméra du plan", "Copie la caméra 2D du plan, à l'image courante (position, focale, panoramique, bascule, rotation).", "scene3d.camera_depuis_plan").clicked() {
                self.scene3d_camera_from_plan();
            }
            let some = self.scene3d.sel.iter().any(|i| *i != CAMERA_ID);
            if crate::bulles::icone_si(ui, some, ph::CROSSHAIR, "Viser la sélection", "Tourne la caméra vers les objets choisis.", "").clicked() {
                let xfs = self.s3_xfs(&self.scene3d.sel.iter().copied().filter(|i| *i != CAMERA_ID).collect::<Vec<_>>());
                let target = gz::pivot_of(&xfs);
                self.scene3d_step("Viser avec la caméra 3D", |s| {
                    let plan = s.doc.scenes3d.plan_mut(panel);
                    plan.cam.aim_at(target);
                    s.doc.scenes3d.touch();
                });
            }
        });
    }

    fn scene3d_light_and_render_fields(&mut self, ui: &mut egui::Ui, panel: u64) {
        let plan = self.doc.scenes3d.plan(panel).cloned().unwrap_or_default();
        let (mut light, mut render) = (plan.light, plan.render);
        let mut changed = false;
        ui.label(RichText::new(format!("{}  Lumière et rendu", ph::SUN)).strong());
        egui::Grid::new("scene3d_lumiere").num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
            ui.label("Direction");
            ui.horizontal(|ui| {
                let r = ui.add(egui::DragValue::new(&mut light.azimuth).speed(0.5).suffix("°").prefix("azimut "));
                changed |= self.edited3d(&r, "Régler la lumière 3D");
                let r = ui.add(egui::DragValue::new(&mut light.elevation).speed(0.5).range(-90.0..=90.0).suffix("°").prefix("hauteur "));
                changed |= self.edited3d(&r, "Régler la lumière 3D");
            });
            ui.end_row();
            ui.label("Force");
            ui.horizontal(|ui| {
                let r = ui.add(egui::DragValue::new(&mut light.intensity).speed(0.01).range(0.0..=3.0).max_decimals(2));
                changed |= self.edited3d(&r, "Régler la lumière 3D");
                let r = ui.add(egui::DragValue::new(&mut light.ambient).speed(0.01).range(0.0..=1.0).max_decimals(2).prefix("ambiante "));
                changed |= self.edited3d(&r, "Régler la lumière 3D");
            });
            ui.end_row();
            ui.label("Style");
            ui.horizontal(|ui| {
                let mut style = render.style;
                egui::ComboBox::from_id_salt("scene3d_style").selected_text(style.label()).width(120.0).show_ui(ui, |ui| {
                    for s in RenderStyle::ALL {
                        ui.selectable_value(&mut style, s, s.label());
                    }
                });
                if style != render.style {
                    render.style = style;
                    changed = true;
                    self.record_named("Style de rendu 3D");
                }
            });
            ui.end_row();
            ui.label("Traits");
            ui.horizontal(|ui| {
                let r = crate::bulles::interrupteur(ui, &mut render.lines, ph::PENCIL_LINE, "Traits de contour", "Trace un trait noir autour des objets et aux cassures (toujours avec le style Contours).", "");
                changed |= self.edited3d(&r, "Traits de la scène 3D");
                let r = ui.add(egui::DragValue::new(&mut render.line_px).speed(0.1).range(0.5..=12.0).suffix(" px"));
                changed |= self.edited3d(&r, "Traits de la scène 3D");
            });
            ui.end_row();
            ui.label("Zone");
            ui.horizontal(|ui| {
                for area in [RenderArea::Frame, RenderArea::Sheet] {
                    let r = crate::bulles::choix(ui, &mut render.area, area, if area == RenderArea::Frame { ph::FRAME_CORNERS } else { ph::PICTURE_IN_PICTURE }, area.label(), if area == RenderArea::Frame { "Seulement l'image finale." } else { "Toute la feuille : la caméra 2D peut s'éloigner ou se décaler sans sortir du rendu." }, "");
                    if r.clicked() {
                        changed = true;
                        self.record_named("Zone de rendu 3D");
                    }
                }
            });
            ui.end_row();
        });
        if changed {
            let plan = self.doc.scenes3d.plan_mut(panel);
            plan.light = light.sanitized();
            plan.render = render.sanitized();
            self.doc.scenes3d.touch();
            self.modified = true;
        }
        ui.horizontal(|ui| {
            if self.bouton(ui, ph::IMAGE, "Envoyer vers un calque de fond", "Rend la scène par la caméra du plateau dans le calque « Fond 3D ».", "scene3d.envoyer").clicked() {
                self.scene3d_send_to_layer();
            }
            let mut follow = plan.follow;
            let r = crate::bulles::interrupteur(ui, &mut follow, ph::LINK, "Suivre la scène", "Le calque « Fond 3D » se remet à jour, en arrière-plan, quand la scène ou la caméra change.", "");
            if r.changed() {
                self.scene3d_step("Suivre la scène 3D", |s| {
                    s.doc.scenes3d.plan_mut(panel).follow = follow;
                    s.doc.scenes3d.touch();
                });
            }
            let working = self.scene3d.job.is_some() || self.scene3d.ready.is_some();
            if working {
                ui.label(RichText::new("calcul…").small().weak());
            }
        });
        ui.label(RichText::new("Le plan z = 0 de la scène est la feuille : un objet posé là a la taille d'un dessin du plan de référence. Le rendu est un calque ordinaire, sous les calques de dessin ; dessine par-dessus.").small().weak());
    }
}
