//! Effets de caméra et focales proposées dans les réglages de la caméra : la section « Effets de
//! caméra » (préréglage, intensité, durée, fréquence, graine, « Convertir en clés ») et la liste des
//! objectifs à côté de la focale. La logique est dans `camfx.rs` (effet vivant, clés) et `camera.rs` (focales).
//!
//! Depuis le 8 novembre 2026 (partie 129), l'effet choisi est **vivant** : un réglage du plan (`Panel::effet_camera`),
//! visible tout de suite, que chaque champ change aussitôt, sans clé. « Convertir en clés » le cuit en clés.

use eframe::egui::{self, RichText};

use crate::app::{App, Mode};
use crate::camera::{self, Camera};
use crate::camfx::{self, Generator, Params, Preset, Vivant};
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

    /// L'effet de caméra vivant du plan courant (partie 129).
    pub(crate) fn effet_vivant(&self) -> Option<Vivant> {
        self.doc.panels.get(self.cur_panel).and_then(|p| p.effet_camera)
    }

    /// Range l'effet vivant du plan courant (l'appelant a enregistré l'étape d'annulation).
    fn set_effet_vivant(&mut self, v: Option<Vivant>) {
        let fps = self.doc.fps;
        if let Some(p) = self.doc.panels.get_mut(self.cur_panel) {
            p.effet_camera = v.map(|v| v.sanitized(fps));
            self.modified = true;
        }
        if let Some(v) = v {
            // (Le prochain effet repart de ces réglages.)
            self.cam_fx = v.params;
        }
    }

    /// Choisit l'effet du plan courant dans la liste : il se voit tout de suite, sans clé (Anthony, 2026-11-08). `None` :
    /// « Aucun effet », l'effet vivant part (les clés déjà converties restent). Une étape d'annulation.
    pub(crate) fn pick_camera_effect(&mut self, preset: Option<Preset>) {
        let fps = self.doc.fps;
        let current = self.effet_vivant();
        let Some(pr) = preset else {
            if current.is_some() {
                self.record_named_step("Retirer l'effet de caméra");
                self.set_effet_vivant(None);
                self.status = "Effet de caméra retiré. Les clés de caméra du plan n'ont pas changé.".into();
            }
            return;
        };
        let v = match current {
            // Un autre préréglage à la même place : sa durée et sa fréquence repartent de ses valeurs, l'intensité et la graine restent.
            Some(v) => Vivant { params: v.params.with_preset(pr, fps), ..v },
            None => {
                let (start, duration, from_sel) = self.camera_effect_range();
                let params = self.cam_fx.with_preset(pr, fps);
                Vivant { params: Params { duration: if from_sel { duration } else { params.duration }, ..params }, start }
            }
        };
        if current == Some(v) {
            return;
        }
        self.record_named_step(&format!("Effet de caméra : {}", pr.label()));
        self.set_effet_vivant(Some(v));
        let len = self.frame_count();
        self.status = format!(
            "{} : se voit tout de suite, des images {} à {}, sans clé. « Convertir en clés » le fige en clés de caméra.",
            pr.label(),
            self.frame_label(v.start.min(len.saturating_sub(1))),
            self.frame_label(v.last_frame(len))
        );
    }

    /// Recale l'effet vivant sur la tête de lecture, ou sur les images choisies dans la timeline (sa durée devient alors
    /// celle du choix). Une étape d'annulation.
    pub(crate) fn place_camera_effect(&mut self) {
        let Some(v) = self.effet_vivant() else { return };
        let (start, duration, from_sel) = self.camera_effect_range();
        let next = Vivant { start, params: Params { duration: if from_sel { duration } else { v.params.duration }, ..v.params } };
        if next == v {
            return;
        }
        self.record_named_step("Placer l'effet de caméra");
        self.set_effet_vivant(Some(next));
    }

    /// « Convertir en clés » (`camera.effet_appliquer`) : l'effet vivant du plan courant devient des clés de caméra
    /// ordinaires, exactement celles que montrait l'effet (`camfx::cuire`), puis l'effet vivant part. Sans effet vivant,
    /// pose directement les clés de l'effet proposé, comme avant (le raccourci). Une étape d'annulation.
    pub(crate) fn apply_camera_effect(&mut self) {
        let fps = self.doc.fps;
        let pi = self.cur_panel;
        let vivant = self.effet_vivant();
        let v = vivant.unwrap_or_else(|| {
            let (start, duration, _) = self.camera_effect_range();
            Vivant { params: Params { duration, ..self.cam_fx }, start }
        });
        let v = v.sanitized(fps);
        // (La même durée de plan que l'effet vivant, `Document::cles_montrees` : la même image, à l'image près.)
        let len = self.doc.panels[pi].frame_count();
        let base = |f: usize| self.doc.panel_camera_douce(pi, f);
        let Some((keys, plan)) = camfx::cuire(&v, fps, self.doc.frame, len, &self.doc.panels[pi].camera, &base) else {
            self.status = "Effet de caméra : l'image de départ est après la fin du plan, rien n'est posé.".into();
            return;
        };
        let last = plan.last_frame().unwrap_or(v.start);
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        self.doc.panels[pi].camera = keys;
        self.doc.panels[pi].effet_camera = None;
        let panel = self.doc.panels[pi].id;
        let n = plan.keys.len();
        let chosen: Vec<CamKeyRef> = plan.keys.iter().map(|k| CamKeyRef { panel, frame: k.frame }).collect();
        self.choose_cam_keys(chosen);
        self.status = format!(
            "{} : {n} clé{} de caméra, des images {} à {}{}. Elles sont choisies : les déplacer, les copier ou Suppr ; Ctrl+Z annule tout l'effet.",
            v.params.preset.label(),
            if n > 1 { "s" } else { "" },
            self.frame_label(v.start),
            self.frame_label(last),
            if plan.truncated { " (coupé par la fin du plan)" } else { "" }
        );
        let name = if vivant.is_some() {
            format!("Convertir en clés : {}", v.params.preset.label())
        } else {
            format!("Effet de caméra : {}", v.params.preset.label())
        };
        self.commit_as(snap, &name);
    }

    /// La section « Effets de caméra » des réglages de la caméra.
    pub(crate) fn camera_effects_ui(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::bulle;
        use egui_phosphor::regular as ph;
        let fps = self.doc.fps;
        let current = self.effet_vivant();
        // La liste : « Aucun effet », puis les préréglages. Choisir, c'est voir tout de suite (sans clé).
        let mut picked: Option<Option<Preset>> = None;
        let resp = egui::ComboBox::from_id_salt("effet_camera")
            .selected_text(current.map_or("Aucun effet", |v| v.params.preset.label()))
            .width(ui.available_width() - 8.0)
            .show_ui(ui, |ui| {
                if ui.selectable_label(current.is_none(), "Aucun effet").on_hover_text("Pas d'effet vivant sur ce plan (ses clés de caméra restent).").clicked() {
                    picked = Some(None);
                }
                for pr in Preset::ALL {
                    if ui.selectable_label(current.is_some_and(|v| v.params.preset == pr), pr.label()).on_hover_text(pr.note()).clicked() {
                        picked = Some(Some(pr));
                    }
                }
            })
            .response;
        bulle(resp, "Effet de caméra", "Choisir un effet : il se voit tout de suite sur le plan, en direct, sans poser de clé. « Convertir en clés » le fige en clés de caméra.", "");
        if let Some(choice) = picked {
            self.pick_camera_effect(choice);
        }
        // (Sans effet : son mode d'emploi est dans l'animation « Effets de caméra sur une plage d'images », partie 153.)
        let Some(v) = self.effet_vivant() else {
            return;
        };
        let mut p = v.params;
        let mut changed = false;
        let name = format!("Régler l'effet de caméra : {}", p.preset.label());
        egui::Grid::new("effet_camera_grille").num_columns(2).show(ui, |ui| {
            if p.preset.is_shake() {
                ui.label("Intensité");
                let mut pct = p.intensity * 100.0;
                let r = ui
                    .add(egui::DragValue::new(&mut pct).range(5.0..=500.0).speed(1.0).max_decimals(0).suffix(" %"))
                    .on_hover_text("100 % : la force proposée pour cet effet. Les décalages se comptent en part de la largeur du cadre filmé.");
                if self.edited_as(&r, &name) {
                    p.intensity = pct / 100.0;
                    changed = true;
                }
                ui.end_row();
            }
            ui.label("Durée");
            ui.horizontal(|ui| {
                let mut d = p.duration;
                let r = ui
                    .add(egui::DragValue::new(&mut d).range(1..=5000).speed(0.5).suffix(" i"))
                    .on_hover_text("Images de l'effet, depuis sa première image.");
                if self.edited_as(&r, &name) {
                    p.duration = d;
                    changed = true;
                }
                ui.label(RichText::new(crate::storyboard::timecode(p.duration, fps)).small().weak());
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
                changed |= self.edited_as(&r, &name);
                ui.end_row();
                ui.label("Graine");
                ui.horizontal(|ui| {
                    let r = ui.add(egui::DragValue::new(&mut p.seed).range(0..=999_999).speed(0.2)).on_hover_text("Même graine, même secousse : elle se refait à l'identique.");
                    changed |= self.edited_as(&r, &name);
                    if crate::bulles::icone(ui, ph::DICE_FIVE, "Autre graine", "Tirer une autre secousse.", "").clicked() {
                        self.record_named_step(&name);
                        p.seed = (p.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223)) % 1_000_000;
                        changed = true;
                    }
                });
                ui.end_row();
            } else {
                ui.label("Zoom");
                ui.horizontal(|ui| {
                    let r = ui
                        .add(egui::DragValue::new(&mut p.zoom_factor).range(1.05..=10.0).speed(0.01).max_decimals(2).prefix("× "))
                        .on_hover_text("Focale d'arrivée / focale de départ (avant : 1,5 donne 35 → 52,5 mm).");
                    changed |= self.edited_as(&r, &name);
                    let base = self.doc.panel_camera_douce(self.cur_panel, v.start).focal();
                    let end = if p.preset == Preset::ZoomOut { base / p.zoom_factor } else { base * p.zoom_factor };
                    ui.label(RichText::new(format!("{base:.0} → {end:.0} mm")).small().weak());
                });
                ui.end_row();
            }
        });
        if p.preset == Preset::Explosion {
            let r = ui.checkbox(&mut p.punch, "Coup de zoom").on_hover_text("Un petit zoom au moment de l'impact, qui retombe avec la secousse.");
            changed |= self.edited_as(&r, &name);
        }
        if changed {
            self.set_effet_vivant(Some(Vivant { params: p, ..v }));
        }
        let v = self.effet_vivant().unwrap_or(v);
        let (n, label, bounds) = {
            let g = Generator::new(&v.params, fps);
            (g.frames().len(), v.params.preset.label(), g.bounds())
        };
        if v.params.preset.is_shake() {
            let punch = if bounds.punch > 0.0 { format!(", zoom +{:.0} %", bounds.punch * 100.0) } else { String::new() };
            ui.label(RichText::new(format!("Au plus ±{:.1} % de la largeur du cadre, ±{:.1}°{punch}.", bounds.pos * 100.0, bounds.angle)).small().weak());
        }
        let len = self.frame_count();
        let cut = v.start + v.params.duration > len.saturating_sub(1);
        ui.label(
            RichText::new(if v.start >= len {
                format!("L'effet commence à l'image {}, après la fin du plan : il ne se voit pas. « Placer » le ramène à la tête de lecture.", self.frame_label(v.start))
            } else {
                format!(
                    "Vivant, sans clé : des images {} à {}{}.",
                    self.frame_label(v.start),
                    self.frame_label(v.last_frame(len)),
                    if cut { " (coupé par la fin du plan)" } else { "" }
                )
            })
            .small()
            .weak(),
        );
        ui.horizontal(|ui| {
            let (_, _, from_sel) = self.camera_effect_range();
            let r = ui.button(format!("{} Placer", ph::CROSSHAIR));
            let quoi = if from_sel { "Met l'effet sur les images choisies dans la timeline (sa durée devient celle du choix)." } else { "Fait commencer l'effet à la tête de lecture. Choisir des images dans la timeline pour viser une plage." };
            if bulle(r, "Placer l'effet", quoi, "").clicked() {
                self.place_camera_effect();
            }
            let key = self.raccourci("camera.effet_appliquer");
            let resp = ui.add(egui::Button::new(format!("{} Convertir en clés", ph::MAGIC_WAND)));
            let resp = bulle(
                resp,
                "Convertir en clés",
                &format!("{label} : pose {n} clés de caméra qui donnent exactement ce que montre l'effet, puis l'effet vivant s'en va (les clés se règlent ensuite une à une) ; une seule étape d'annulation."),
                &key,
            );
            crate::raccourcisui::shortcut_menu(&resp, "camera.effet_appliquer", &key);
            if resp.clicked() {
                self.run_action("camera.effet_appliquer");
            }
        });
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
