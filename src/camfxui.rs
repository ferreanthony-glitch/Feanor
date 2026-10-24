//! Effets de caméra et focales proposées dans les réglages de la caméra : la section « Effets de
//! caméra » (préréglage, intensité, durée, fréquence, graine, « Appliquer ») et la liste des
//! objectifs à côté de la focale. La logique est dans `camfx.rs` (clés) et `camera.rs` (focales).

use eframe::egui::{self, RichText};

use crate::app::{App, Mode};
use crate::camera::{self, Camera};
use crate::camfx::{self, Generator, Params, Preset};
use crate::camkeys::CamKeyRef;

impl App {
    /// Où l'effet sera posé : (première image, durée en images, d'après les images choisies).
    /// Les images choisies dans la timeline (au moins deux) donnent la plage ; sinon l'effet part
    /// de l'image courante et dure ce que dit le réglage.
    pub(crate) fn camera_effect_range(&self) -> (usize, usize, bool) {
        match (self.mode, self.frame_sel) {
            (Mode::Animation, Some((a, b))) if b > a + 1 => (a, b - 1 - a, true),
            _ => (self.cur_frame, self.cam_fx.duration.max(1), false),
        }
    }

    /// Pose les clés de l'effet choisi sur le plan courant : une étape d'annulation.
    pub(crate) fn apply_camera_effect(&mut self) {
        let fps = self.doc.fps;
        let pi = self.cur_panel;
        let (start, duration, _) = self.camera_effect_range();
        let params = Params { duration, ..self.cam_fx }.sanitized(fps);
        let g = Generator::new(&params, fps);
        let len = self.frame_count();
        let extra: Vec<usize> = self.doc.panels[pi].camera.iter().map(|k| k.frame).collect();
        let plan = camfx::plan(&g, self.doc.frame, start, len, &extra, !extra.is_empty(), &|f| self.doc.panel_camera_douce(pi, f));
        let Some(last) = plan.last_frame() else {
            self.status = "Effet de caméra : l'image de départ est après la fin du plan, rien n'est posé.".into();
            return;
        };
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        camfx::write(&mut self.doc.panels[pi].camera, &plan);
        let panel = self.doc.panels[pi].id;
        let n = plan.keys.len();
        let chosen: Vec<CamKeyRef> = plan.keys.iter().map(|k| CamKeyRef { panel, frame: k.frame }).collect();
        self.choose_cam_keys(chosen);
        self.status = format!(
            "{} : {n} clé{} de caméra, des images {} à {}{}. Elles sont choisies : les déplacer, les copier ou Suppr ; Ctrl+Z annule tout l'effet.",
            params.preset.label(),
            if n > 1 { "s" } else { "" },
            self.frame_label(start),
            self.frame_label(last),
            if plan.truncated { " (coupé par la fin du plan)" } else { "" }
        );
        let name = format!("Effet de caméra : {}", params.preset.label());
        self.commit_as(snap, &name);
    }

    /// La section « Effets de caméra » des réglages de la caméra.
    pub(crate) fn camera_effects_ui(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::bulle;
        use egui_phosphor::regular as ph;
        let fps = self.doc.fps;
        let mut p = self.cam_fx;
        let mut picked = None;
        let resp = egui::ComboBox::from_id_salt("effet_camera")
            .selected_text(p.preset.label())
            .width(ui.available_width() - 8.0)
            .show_ui(ui, |ui| {
                for pr in Preset::ALL {
                    if ui.selectable_label(p.preset == pr, pr.label()).on_hover_text(pr.note()).clicked() {
                        picked = Some(pr);
                    }
                }
            })
            .response;
        bulle(resp, "Effet de caméra", p.preset.note(), "");
        if let Some(pr) = picked {
            p = p.with_preset(pr, fps);
        }
        let (start, _, from_sel) = self.camera_effect_range();
        egui::Grid::new("effet_camera_grille").num_columns(2).show(ui, |ui| {
            if p.preset.is_shake() {
                ui.label("Intensité");
                let mut pct = p.intensity * 100.0;
                let r = ui
                    .add(egui::DragValue::new(&mut pct).range(5.0..=500.0).speed(1.0).max_decimals(0).suffix(" %"))
                    .on_hover_text("100 % : la force proposée pour cet effet. Les décalages se comptent en part de la largeur du cadre filmé.");
                if r.changed() {
                    p.intensity = pct / 100.0;
                }
                ui.end_row();
            }
            ui.label("Durée");
            ui.horizontal(|ui| {
                if from_sel {
                    ui.label(format!("{} i (images choisies)", self.camera_effect_range().1));
                } else {
                    let mut d = p.duration;
                    let r = ui
                        .add(egui::DragValue::new(&mut d).range(1..=5000).speed(0.5).suffix(" i"))
                        .on_hover_text("Images entre la première et la dernière clé de l'effet.");
                    if r.changed() {
                        p.duration = d;
                    }
                    ui.label(RichText::new(crate::storyboard::timecode(p.duration, fps)).small().weak());
                }
            });
            ui.end_row();
            if p.preset.is_shake() {
                ui.label(if p.preset == Preset::Shoulder { "Vitesse" } else { "Fréquence" });
                let r = ui
                    .add(egui::DragValue::new(&mut p.freq).range(0.1..=(fps as f32 / 2.0).max(1.0)).speed(0.05).max_decimals(1).suffix(" Hz"))
                    .on_hover_text(if p.preset == Preset::Shoulder {
                        "Vitesse de la dérive : plus haut, la caméra bouge plus vite."
                    } else {
                        "Vitesse des à-coups, en secousses par seconde."
                    });
                let _ = r;
                ui.end_row();
                ui.label("Graine");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut p.seed).range(0..=999_999).speed(0.2))
                        .on_hover_text("Même graine, mêmes clés : la secousse se refait à l'identique.");
                    if crate::bulles::icone(ui, ph::DICE_FIVE, "Autre graine", "Tirer une autre secousse.", "").clicked() {
                        p.seed = (p.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223)) % 1_000_000;
                    }
                });
                ui.end_row();
            } else {
                ui.label("Zoom");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut p.zoom_factor).range(1.05..=10.0).speed(0.01).max_decimals(2).prefix("× "))
                        .on_hover_text("Focale d'arrivée / focale de départ (avant : 1,5 donne 35 → 52,5 mm).");
                    let base = self.current_camera().focal();
                    let end = if p.preset == Preset::ZoomOut { base / p.zoom_factor } else { base * p.zoom_factor };
                    ui.label(RichText::new(format!("{base:.0} → {end:.0} mm")).small().weak());
                });
                ui.end_row();
            }
        });
        if p.preset == Preset::Explosion {
            ui.checkbox(&mut p.punch, "Coup de zoom").on_hover_text("Un petit zoom au moment de l'impact, qui retombe avec la secousse.");
        }
        self.cam_fx = p;
        let (n, label, bounds) = {
            let params = Params { duration: self.camera_effect_range().1, ..p }.sanitized(fps);
            let g = Generator::new(&params, fps);
            (g.frames().len(), params.preset.label(), g.bounds())
        };
        if p.preset.is_shake() {
            let punch = if bounds.punch > 0.0 { format!(", zoom +{:.0} %", bounds.punch * 100.0) } else { String::new() };
            ui.label(RichText::new(format!("Au plus ±{:.1} % de la largeur du cadre, ±{:.1}°{punch}.", bounds.pos * 100.0, bounds.angle)).small().weak());
        }
        ui.label(
            RichText::new(if from_sel {
                format!("Sur les images {} à {} choisies : {n} clés.", self.frame_label(start), self.frame_label(start + self.camera_effect_range().1))
            } else {
                format!("À partir de l'image {} : {n} clés. Choisir des images dans la timeline pour viser une plage.", self.frame_label(start))
            })
            .small()
            .weak(),
        );
        let key = self.raccourci("camera.effet_appliquer");
        let resp = ui.add(egui::Button::new(format!("{} Appliquer", ph::MAGIC_WAND)));
        let resp = bulle(resp, "Appliquer l'effet", &format!("{label} : pose des clés de caméra à partir de la caméra actuelle (rien n'est remis à zéro) ; une seule étape d'annulation."), &key);
        crate::raccourcisui::shortcut_menu(&resp, "camera.effet_appliquer", &key);
        if resp.clicked() {
            self.run_action("camera.effet_appliquer");
        }
    }

    /// Choisit un objectif dans la liste des focales : règle la focale comme le champ de la focale
    /// (une clé à l'image courante si la caméra est animée, sinon la focale du plan). L'anamorphique
    /// pose aussi le cadre 2,39:1.
    pub(crate) fn pick_focal_preset(&mut self, index: usize) {
        let Some(p) = camera::FOCAL_PRESETS.get(index).copied() else { return };
        self.record_named_step(&format!("Focale : {} ({} mm)", p.name, p.mm));
        let cam = self.current_camera();
        self.set_current_camera(Camera { zoom: camera::zoom_for_focal(p.mm), ..cam });
        if p.anamorphic {
            self.layout.frame_format = Some(camera::ANAMORPHIC_FORMAT);
            self.format_custom = false;
        }
        self.status = format!(
            "Focale {} mm ({}), angle de champ {:.0}°{}.",
            p.mm,
            p.name,
            camera::angle_of_view(p.mm),
            if p.anamorphic { " ; cadre 2,39:1 posé (repère d'affichage, le dessin n'est pas déformé)" } else { "" }
        );
    }

    /// La liste des focales proposées, à côté du champ de la focale (`focal` : la focale actuelle).
    pub(crate) fn focal_presets_ui(&mut self, ui: &mut egui::Ui, focal: f32) {
        use egui_phosphor::regular as ph;
        let current = camera::FOCAL_PRESETS.iter().position(|p| (p.mm - focal).abs() < 0.05);
        let mut picked = None;
        let resp = egui::ComboBox::from_id_salt("focale_prereglages")
            .selected_text(format!("{} {}", ph::APERTURE, current.map_or("Objectifs", |i| camera::FOCAL_PRESETS[i].name)))
            .width(150.0)
            .show_ui(ui, |ui| {
                for (i, p) in camera::FOCAL_PRESETS.iter().enumerate() {
                    if ui.selectable_label(current == Some(i), camera::focal_label(p)).on_hover_text(p.note).clicked() {
                        picked = Some(i);
                    }
                }
            })
            .response;
        crate::bulles::bulle(resp, "Objectifs", "Focales courantes : champ large (courte) à téléobjectif (longue), et l'anamorphique avec son cadre 2,39:1. L'angle de champ est celui d'un capteur de 36 mm de large.", "");
        if let Some(i) = picked {
            self.pick_focal_preset(i);
        }
    }
}
