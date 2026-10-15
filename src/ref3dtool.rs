//! Outil « Repère 3D » (2d) : importer un modèle 3D (glTF), le regarder en orbite depuis le cadre
//! d'un panneau, l'afficher en fil de fer comme un guide, et le dessiner dans un calque bitmap.
//!
//! Le fil de fer d'un gros décor (des centaines de milliers d'arêtes) ne peut pas être tracé
//! segment par segment à chaque image sans ralentir le pinceau : il est calculé sur un autre fil,
//! tracé une fois dans une image, et cette image est posée sur le cadre (une seule texture à
//! afficher). Pendant qu'on tourne, et le temps du calcul, un aperçu léger (les arêtes les plus
//! longues, sans masquer les cachées) le remplace.

use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect, RichText, Shape, Stroke, TextureHandle};

use crate::app::{App, Tool};
use crate::document::{Document, Layer, LayerKind, Ref3d};
use crate::history::Edit;
use crate::model3d::Model3d;
use crate::raster::{premultiply, TILE};
use crate::view3d::{self, Cam3d, MAX_ELEVATION};

/// Arêtes de l'aperçu pendant qu'on tourne.
const PREVIEW_EDGES: usize = 6000;
/// Épaisseur du trait du fil de fer affiché, en pixels de l'écran.
const LINE_WIDTH: f32 = 1.3;
/// Degrés par pixel d'écran quand on tourne.
const ORBIT_SPEED: f32 = 0.3;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Ref3dMove {
    Orbit,
    Pan,
    Dolly,
}

pub(crate) struct Ref3dDrag {
    kind: Ref3dMove,
    /// Point de départ (écran).
    start: Pos2,
    cam0: Cam3d,
    snap: Edit,
    changed: bool,
}

/// Ce dont dépend le fil de fer (arêtes cachées comprises).
#[derive(Clone, PartialEq, Debug)]
struct WireKey {
    model: u64,
    cam: Cam3d,
    frame: [usize; 2],
    /// Taille de la feuille (le cadre est au milieu).
    stage: [usize; 2],
    hidden: bool,
}

/// Segments en pixels de la feuille.
type Segments = Arc<Vec<[Pos2; 2]>>;

/// Fil de fer de `model` sur toute la feuille `stage` (cadre `frame` au milieu), en pixels de
/// la feuille : le repère continue dans le hors-champ, comme un décor dessiné sur la feuille.
fn stage_wireframe(model: &Model3d, cam: &Cam3d, frame: [usize; 2], stage: [usize; 2], hidden: bool) -> Vec<[Pos2; 2]> {
    let offset = vec2(
        stage[0].saturating_sub(frame[0]) as f32 / 2.0,
        stage[1].saturating_sub(frame[1]) as f32 / 2.0,
    );
    let area = Rect::from_min_size(Pos2::ZERO - offset, vec2(stage[0] as f32, stage[1] as f32));
    let mut segs = view3d::wireframe_in(model, cam, [frame[0] as f32, frame[1] as f32], area, hidden);
    for s in &mut segs {
        *s = [s[0] + offset, s[1] + offset];
    }
    segs
}

/// Fil de fer tracé dans une image (`zoom` : pixels de l'image par pixel de la feuille).
struct WireImage {
    key: WireKey,
    zoom: f32,
    texture: TextureHandle,
}

struct WireJob {
    key: WireKey,
    zoom: f32,
    handle: JoinHandle<(Segments, egui::ColorImage)>,
}

#[derive(Default)]
pub(crate) struct Ref3dState {
    drag: Option<Ref3dDrag>,
    image: Option<WireImage>,
    /// Derniers segments calculés : un simple changement de zoom ne refait que l'image.
    segs: Option<(WireKey, Segments)>,
    job: Option<WireJob>,
}

impl Ref3dState {
    pub(crate) fn dragging(&self) -> bool {
        self.drag.is_some()
    }
}

/// Image du fil de fer sur la feuille : blanc, transparence = couverture du trait (teinté à
/// l'affichage).
fn wire_image(segs: &[[Pos2; 2]], stage: [usize; 2], zoom: f32) -> egui::ColorImage {
    let size = [
        ((stage[0] as f32 * zoom).ceil() as usize).max(1),
        ((stage[1] as f32 * zoom).ceil() as usize).max(1),
    ];
    let mask = view3d::rasterize(segs, size, zoom, LINE_WIDTH);
    egui::ColorImage { size, pixels: mask.into_iter().map(Color32::from_white_alpha).collect() }
}

/// Dessine le repère 3D du panneau dans un nouveau calque bitmap, tout en bas, exposé sur toute
/// la durée du panneau (sur toute la feuille, hors-champ compris) ; renvoie l'indice du calque
/// (rien si le panneau n'a pas de repère).
pub(crate) fn bake(doc: &mut Document, panel: usize, width: f32) -> Option<usize> {
    let r = doc.panels.get(panel)?.ref3d.clone()?;
    let model = doc.models.get(&r.model)?.clone();
    let stage = [doc.width, doc.height];
    let segs = stage_wireframe(&model, &r.cam, doc.frame, stage, r.hidden_lines);
    let mask = view3d::rasterize(&segs, stage, 1.0, width);
    let length = doc.panels[panel].frame_count();
    let cel = doc.new_cel(LayerKind::Bitmap);
    let raster = &mut doc.cels.get_mut(&cel)?.raster;
    for (i, &a) in mask.iter().enumerate() {
        let (x, y) = (i % stage[0], i / stage[0]);
        if a == 0 || x >= raster.width || y >= raster.height {
            continue;
        }
        let idx = raster.tile_index_of(x, y);
        raster.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = premultiply(r.color[0], r.color[1], r.color[2], a);
    }
    let mut layer = Layer::new("Repère 3D", LayerKind::Bitmap);
    layer.insert(0, Some(cel), length);
    doc.panels[panel].layers.insert(0, layer);
    Some(0)
}

impl App {
    fn current_ref3d(&self) -> Option<&Ref3d> {
        self.doc.panels.get(self.cur_panel)?.ref3d.as_ref()
    }

    fn frame_f(&self) -> [f32; 2] {
        [self.doc.frame[0] as f32, self.doc.frame[1] as f32]
    }

    /// Fichier > Importer un modèle 3D : lu, ajouté au projet, posé dans le panneau courant.
    pub(crate) fn import_model3d(&mut self) {
        self.end_stroke();
        self.playing = false;
        let Some(path) = rfd::FileDialog::new()
            .set_title("Importer un modèle 3D (glTF)")
            .add_filter("Modèles 3D glTF", &["glb", "gltf"])
            .pick_file()
        else {
            return;
        };
        self.import_model3d_path(&path);
    }

    /// Importe le fichier glTF `path` dans le panneau courant.
    pub(crate) fn import_model3d_path(&mut self, path: &std::path::Path) {
        let mesh = match crate::model3d::load_gltf(path) {
            Ok(m) => m,
            Err(e) => {
                self.status = format!("Import du modèle impossible : {e}");
                return;
            }
        };
        let name = path.file_name().map_or_else(|| "modèle".into(), |n| n.to_string_lossy().into_owned());
        let snap = self.snapshot();
        let id = self.doc.add_model(name.clone(), mesh);
        let model = self.doc.models[&id].clone();
        let mut r = Ref3d::new(id, Cam3d::fit(model.bounds, self.frame_f()));
        // Le nouveau modèle remplace celui du panneau, avec les mêmes couleurs.
        if let Some(old) = self.current_ref3d() {
            (r.color, r.opacity) = (old.color, old.opacity);
        }
        self.doc.panels[self.cur_panel].ref3d = Some(r);
        self.commit_structure(snap);
        self.tool = Tool::Ref3d;
        self.status = format!(
            "Modèle importé : {name} ({} triangles, {} arêtes dessinées). Glisser : tourner · Maj : \
             déplacer · Ctrl : avancer ou reculer.",
            model.mesh.triangles.len(),
            model.edges.len()
        );
    }

    /// Change le modèle du panneau (cadré à neuf) ou le retire.
    fn set_panel_model(&mut self, model: Option<u64>) {
        let frame = self.frame_f();
        let next = model.and_then(|id| self.doc.models.get(&id)).map(|m| {
            let mut r = Ref3d::new(m.id, Cam3d::fit(m.bounds, frame));
            if let Some(old) = self.current_ref3d() {
                (r.color, r.opacity, r.hidden_lines) = (old.color, old.opacity, old.hidden_lines);
            }
            r
        });
        self.record_step();
        self.doc.panels[self.cur_panel].ref3d = next;
    }

    pub(crate) fn ref3d_press(&mut self, screen: Pos2, mods: egui::Modifiers) {
        self.end_stroke();
        self.playing = false;
        let Some(r) = self.current_ref3d() else {
            self.status = "Pas de repère 3D dans ce panneau : Fichier > Importer un modèle 3D (glTF)…, \
                           ou choisis un modèle déjà importé dans le panneau Outils."
                .into();
            return;
        };
        let kind = if mods.shift {
            Ref3dMove::Pan
        } else if mods.ctrl || mods.command {
            Ref3dMove::Dolly
        } else {
            Ref3dMove::Orbit
        };
        self.ref3d.drag = Some(Ref3dDrag { kind, start: screen, cam0: r.cam, snap: self.snapshot(), changed: false });
    }

    pub(crate) fn ref3d_drag_to(&mut self, screen: Pos2) {
        let Some(d) = &mut self.ref3d.drag else { return };
        let (kind, start, mut cam) = (d.kind, d.start, d.cam0);
        d.changed = true;
        let delta = screen - start;
        match kind {
            // Comme dans un logiciel 3D : le modèle suit la souris.
            Ref3dMove::Orbit => cam.orbit(-delta.x * ORBIT_SPEED, delta.y * ORBIT_SPEED),
            Ref3dMove::Pan => {
                let (rect, size) = (self.canvas_rect, self.canvas_size());
                let moved = self.view.to_canvas(rect, size, screen) - self.view.to_canvas(rect, size, start);
                cam.pan(moved.x, moved.y, self.doc.frame[0] as f32);
            }
            Ref3dMove::Dolly => cam.dolly((delta.y * 0.005).exp()),
        }
        if let Some(r) = self.doc.panels[self.cur_panel].ref3d.as_mut() {
            r.cam = cam;
        }
        self.status = format!(
            "Repère 3D : azimut {:.0}°, hauteur {:.0}°, distance {:.2}, focale {:.0} mm",
            cam.azimuth, cam.elevation, cam.distance, cam.focal
        );
    }

    pub(crate) fn finish_ref3d_drag(&mut self) {
        let Some(d) = self.ref3d.drag.take() else { return };
        if d.changed {
            self.history.push(d.snap);
            self.modified = true;
        }
    }

    /// Ce que le fil de fer affiché doit être (rien : pas de repère visible).
    fn wire_key(&self) -> Option<WireKey> {
        let r = self.current_ref3d().filter(|r| r.visible && self.doc.models.contains_key(&r.model))?;
        Some(WireKey {
            model: r.model,
            cam: r.cam,
            frame: self.doc.frame,
            stage: [self.doc.width, self.doc.height],
            hidden: r.hidden_lines,
        })
    }

    /// Pixels de l'image du fil de fer par pixel de la feuille : ceux de l'écran, par paliers (on
    /// ne refait pas l'image à chaque cran de molette), sans dépasser 4096 pixels de côté.
    fn wire_zoom(&self) -> f32 {
        let screen = (self.view.zoom * self.ppp).max(1e-3);
        let step = 2f32.powf((screen.log2() * 2.0).round() / 2.0).clamp(0.125, 2.0);
        step.min(4096.0 / self.doc.width.max(self.doc.height).max(1) as f32)
    }

    /// Avant d'afficher : récupère le fil de fer calculé, et lance le calcul s'il le faut.
    pub(crate) fn update_ref3d(&mut self, ctx: &egui::Context) {
        if self.ref3d.job.as_ref().is_some_and(|j| j.handle.is_finished()) {
            if let Some(job) = self.ref3d.job.take() {
                if let Ok((segs, image)) = job.handle.join() {
                    let texture = ctx.load_texture("repere3d", image, egui::TextureOptions::LINEAR);
                    self.ref3d.segs = Some((job.key.clone(), segs));
                    self.ref3d.image = Some(WireImage { key: job.key, zoom: job.zoom, texture });
                }
            }
        }
        let Some(key) = self.wire_key() else { return };
        if self.ref3d.drag.is_some() {
            return; // aperçu seulement pendant qu'on tourne
        }
        let zoom = self.wire_zoom();
        if self.ref3d.image.as_ref().is_some_and(|i| i.key == key && i.zoom == zoom) {
            return;
        }
        if self.ref3d.job.is_some() {
            // Un calcul est en cours (peut-être pour une vue déjà dépassée) : on attend sa fin.
            ctx.request_repaint_after(Duration::from_millis(20));
            return;
        }
        let Some(model) = self.doc.models.get(&key.model).cloned() else { return };
        let known = self.ref3d.segs.as_ref().filter(|(k, _)| *k == key).map(|(_, s)| s.clone());
        let k = key.clone();
        let handle = std::thread::spawn(move || {
            let segs = known.unwrap_or_else(|| Arc::new(stage_wireframe(&model, &k.cam, k.frame, k.stage, k.hidden)));
            let image = wire_image(&segs, k.stage, zoom);
            (segs, image)
        });
        self.ref3d.job = Some(WireJob { key, zoom, handle });
        ctx.request_repaint_after(Duration::from_millis(20));
    }

    /// Le repère 3D du panneau, posé sur le cadre comme un guide.
    pub(crate) fn paint_ref3d(&self, painter: &egui::Painter, rect: Rect) {
        let Some(key) = self.wire_key() else { return };
        let Some(r) = self.current_ref3d() else { return };
        let size = self.canvas_size();
        let to_s = |p: Pos2| self.view.to_screen(rect, size, p);
        let fr = self.doc.frame_rect();
        let alpha = (r.opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
        let tint = Color32::from_rgba_unmultiplied(r.color[0], r.color[1], r.color[2], alpha);
        match &self.ref3d.image {
            Some(img) if img.key == key && self.ref3d.drag.is_none() => {
                let mut mesh = egui::Mesh::with_texture(img.texture.id());
                let stage = Rect::from_min_size(Pos2::ZERO, size);
                let corners = [stage.left_top(), stage.right_top(), stage.right_bottom(), stage.left_bottom()];
                let uvs = [pos2(0.0, 0.0), pos2(1.0, 0.0), pos2(1.0, 1.0), pos2(0.0, 1.0)];
                for (p, uv) in corners.into_iter().zip(uvs) {
                    mesh.vertices.push(egui::epaint::Vertex { pos: to_s(p), uv, color: tint });
                }
                mesh.add_triangle(0, 1, 2);
                mesh.add_triangle(0, 2, 3);
                painter.add(Shape::mesh(mesh));
            }
            _ => {
                let Some(model) = self.doc.models.get(&key.model) else { return };
                let segs = view3d::preview(model, &r.cam, self.frame_f(), PREVIEW_EDGES);
                let stroke = Stroke::new(1.0_f32, tint);
                let origin = fr.min.to_vec2();
                painter.extend(segs.iter().map(|s| Shape::line_segment([to_s(s[0] + origin), to_s(s[1] + origin)], stroke)));
            }
        }
    }

    /// « Dessiner dans un calque » : le fil de fer devient un dessin (calque bitmap tout en bas),
    /// et le repère est masqué.
    pub(crate) fn bake_ref3d(&mut self) {
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        let Some(li) = bake(&mut self.doc, self.cur_panel, 1.5) else { return };
        if let Some(r) = self.doc.panels[self.cur_panel].ref3d.as_mut() {
            r.visible = false;
        }
        if self.cur_layer >= li {
            self.cur_layer += 1;
        }
        self.commit_structure(snap);
        self.status = "Repère 3D dessiné dans le calque « Repère 3D » (tout en bas) ; le repère est masqué.".into();
    }

    /// Réglages de l'outil Repère 3D (panneau Outils).
    pub(crate) fn ref3d_panel(&mut self, ui: &mut egui::Ui) {
        let note = |ui: &mut egui::Ui, text: &str| {
            ui.label(RichText::new(text).small().weak());
        };
        use crate::bulles::{bulle, icone, icone_si, interrupteur};
        use egui_phosphor::regular as ph;
        let mut models: Vec<(u64, String)> = self.doc.models.values().map(|m| (m.id, m.name.clone())).collect();
        models.sort();
        let current = self.current_ref3d().map(|r| r.model);
        let mut chosen = None;
        let mut import = false;
        // Importer, et le modèle du panneau, sur une ligne.
        ui.horizontal(|ui| {
            import = icone(ui, ph::FILE_ARROW_DOWN, "Importer un modèle 3D", "glTF (.glb ou .gltf). Depuis Blender : Fichier > Exporter > glTF 2.0.", "").clicked();
            if models.is_empty() {
                return;
            }
            let shown = current.and_then(|id| models.iter().find(|m| m.0 == id)).map_or("Aucun".to_string(), |m| m.1.clone());
            let stats = current
                .and_then(|id| self.doc.models.get(&id))
                .map(|m| format!("{} triangles, {} arêtes dessinées.", m.mesh.triangles.len(), m.edges.len()));
            let combo = egui::ComboBox::from_id_salt("repere3d_modele").selected_text(shown).width(130.0).show_ui(ui, |ui| {
                if ui.selectable_label(current.is_none(), "Aucun").clicked() {
                    chosen = Some(None);
                }
                for (id, name) in &models {
                    if ui.selectable_label(current == Some(*id), name).clicked() {
                        chosen = Some(Some(*id));
                    }
                }
            });
            bulle(combo.response, "Modèle du panneau", stats.as_deref().unwrap_or(""), "");
        });
        if import {
            self.import_model3d();
        }
        if models.is_empty() {
            note(
                ui,
                "Aucun modèle 3D dans le projet. Exporte ton décor en glTF (.glb) depuis ton logiciel 3D, \
                 puis importe-le : il s'affiche en fil de fer dans le cadre, comme un guide.",
            );
            return;
        }
        if let Some(c) = chosen.filter(|c| *c != current) {
            self.set_panel_model(c);
        }
        let Some(r) = self.current_ref3d().cloned() else {
            note(ui, "Ce panneau n'a pas de repère 3D : choisis un modèle.");
            return;
        };
        let mut next = r.clone();
        let mut changed = false;
        ui.horizontal(|ui| {
            let resp = interrupteur(ui, &mut next.visible, ph::EYE, "Visible", "Montrer le repère dans le cadre.", "");
            changed |= self.edited(&resp);
            let resp = ui.color_edit_button_srgb(&mut next.color);
            changed |= self.edited(&bulle(resp, "Couleur du repère", "", ""));
            let mut percent = next.opacity * 100.0;
            let resp = ui.add(egui::DragValue::new(&mut percent).speed(1.0).range(5.0..=100.0).max_decimals(0).suffix(" %"));
            if self.edited(&bulle(resp, "Opacité du repère", "", "")) {
                next.opacity = percent / 100.0;
                changed = true;
            }
            let resp = interrupteur(ui, &mut next.hidden_lines, ph::CUBE, "Masquer les arêtes cachées", "Les arêtes derrière une face ne sont pas dessinées (calcul plus long).", "");
            changed |= self.edited(&resp);
        });
        egui::Grid::new("repere3d_camera").num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
            let cam = &mut next.cam;
            ui.label("Azimut");
            let resp = ui.add(egui::DragValue::new(&mut cam.azimuth).speed(0.5).suffix("°"));
            changed |= self.edited(&resp);
            ui.end_row();
            ui.label("Hauteur");
            let resp = ui.add(
                egui::DragValue::new(&mut cam.elevation).speed(0.5).range(-MAX_ELEVATION..=MAX_ELEVATION).suffix("°"),
            );
            changed |= self.edited(&resp);
            ui.end_row();
            ui.label("Distance");
            let speed = (cam.distance * 0.01).max(1e-4) as f64;
            let resp = ui.add(egui::DragValue::new(&mut cam.distance).speed(speed).range(1e-3..=1e7).max_decimals(3));
            changed |= self.edited(&resp);
            ui.end_row();
            ui.label("Focale");
            let resp = ui.add(egui::DragValue::new(&mut cam.focal).speed(0.5).range(8.0..=300.0).suffix(" mm"));
            changed |= self.edited(&resp);
            ui.end_row();
        });
        if changed {
            next.cam.azimuth = next.cam.azimuth.rem_euclid(360.0);
            self.doc.panels[self.cur_panel].ref3d = Some(next);
        }
        let mut bake = false;
        ui.horizontal_wrapped(|ui| {
            if icone(ui, ph::CORNERS_OUT, "Cadrer le modèle", "Le modèle entier dans le cadre.", "").clicked() {
                if let Some(m) = self.doc.models.get(&r.model) {
                    let cam = Cam3d::fit(m.bounds, self.frame_f());
                    self.record_step();
                    if let Some(x) = self.doc.panels[self.cur_panel].ref3d.as_mut() {
                        x.cam = cam;
                    }
                }
            }
            let previous = self.cur_panel.checked_sub(1).and_then(|i| self.doc.panels[i].ref3d.clone());
            if icone_si(ui, previous.is_some(), ph::ARROW_BEND_UP_LEFT, "Comme le panneau précédent", "Même modèle, même point de vue que le panneau d'avant.", "").clicked() {
                self.record_step();
                self.doc.panels[self.cur_panel].ref3d = previous;
            }
            if icone(ui, ph::X_CIRCLE, "Retirer", "Ce panneau n'a plus de repère 3D.", "").clicked() {
                self.set_panel_model(None);
            }
            bake = icone(
                ui,
                ph::PENCIL_LINE,
                "Dessiner dans un calque",
                "Le fil de fer devient un dessin bitmap (calque « Repère 3D », tout en bas), qui s'exporte.",
                "",
            )
            .clicked();
        });
        if bake {
            self.bake_ref3d();
        }
        note(
            ui,
            "Glisser : tourner. Maj : déplacer. Ctrl : avancer, reculer. Un guide par panneau, \
             absent des exports.",
        );
    }
}
