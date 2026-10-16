//! Panneau de la pelure d'oignon (13f), la « table lumineuse » de TVPaint 12 : mode, devant,
//! curseurs liés, colorisation et images d'avant / d'après une à une (montrée, opacité), opacité
//! globale, tout activer / désactiver, réglages rangés sous un nom, décalage temporaire.
//!
//! Le décalage ne touche pas aux dessins et ne s'enregistre pas : tant que « Décaler » est
//! allumé, glisser sur le dessin déplace les images de la pelure ; « Remettre » les replace.

use eframe::egui::{self, vec2, Align2, RichText, Vec2};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles::bulle;
use crate::onion::{OnionMode, OnionSettings, MAX_NEIGHBOURS};

/// Nom proposé pour ranger des réglages.
const DEFAULT_PRESET: &str = "Ma pelure";

/// Une colonne du panneau : les images d'avant (`after` faux) ou d'après.
fn side_ui(ui: &mut egui::Ui, s: &mut OnionSettings, after: bool) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(if after { "Après" } else { "Avant" }).strong());
        let count = if after { &mut s.after } else { &mut s.before };
        let resp = ui.add(egui::DragValue::new(count).range(0..=MAX_NEIGHBOURS));
        bulle(resp, "Nombre d'images", "Combien d'images de ce côté.", "");
    });
    ui.horizontal(|ui| {
        let (tint, color, gradient, color2) = if after {
            (&mut s.tint_after, &mut s.after_color, &mut s.gradient_after, &mut s.after_color2)
        } else {
            (&mut s.tint_before, &mut s.before_color, &mut s.gradient_before, &mut s.before_color2)
        };
        let what = if after { "suivantes" } else { "précédentes" };
        ui.checkbox(tint, "Coloriser").on_hover_text(format!("Montrer les images {what} dans la couleur choisie (sinon : leurs vraies couleurs)."));
        ui.add_enabled_ui(*tint, |ui| {
            ui.color_edit_button_srgb(color);
            ui.checkbox(gradient, "Dégradé").on_hover_text("De la première couleur pour l'image la plus proche à la seconde pour la plus lointaine.");
            ui.add_enabled_ui(*gradient, |ui| ui.color_edit_button_srgb(color2));
        });
    });
    let n = if after { s.after } else { s.before };
    for k in 0..n {
        ui.horizontal(|ui| {
            let label = if after { format!("+{}", k + 1) } else { format!("-{}", k + 1) };
            let shown = s.shown(after, k);
            let resp = ui.add_sized([34.0, 18.0], egui::SelectableLabel::new(shown, RichText::new(label).monospace()));
            if resp.on_hover_text("Montrer / cacher cette image").clicked() {
                let list = if after { &mut s.show_after } else { &mut s.show_before };
                list.resize(MAX_NEIGHBOURS, true);
                list[k] = !shown;
            }
            let mut v = s.rank_opacity(after, k);
            let resp = ui.add_enabled(shown, egui::Slider::new(&mut v, 0.0..=1.0).show_value(false));
            if resp.changed() {
                s.set_rank_opacity(after, k, v);
            }
            ui.label(RichText::new(format!("{:>3} %", (v * 100.0).round())).monospace().weak());
        });
    }
}

impl App {
    /// Le panneau de la pelure d'oignon, s'il est ouvert.
    pub(crate) fn onion_window_ui(&mut self, ctx: &egui::Context) {
        if !self.onion_window {
            return;
        }
        let mut open = true;
        egui::Window::new("Pelure d'oignon")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_pos(ctx.screen_rect().center() - vec2(260.0, 240.0))
            .pivot(Align2::LEFT_TOP)
            .show(ctx, |ui| self.onion_panel(ui));
        if !open {
            self.onion_window = false;
        }
    }

    fn onion_panel(&mut self, ui: &mut egui::Ui) {
        let key = self.raccourci("vue.pelure");
        ui.horizontal(|ui| {
            crate::bulles::interrupteur_a(ui, &mut self.onion.enabled, ph::CIRCLES_THREE, "Pelure d'oignon", "Montrer les dessins d'avant et d'après.", &key, "vue.pelure");
            let s = &mut self.onion;
            egui::ComboBox::from_id_salt("mode_pelure").selected_text(format!("Mode {}", s.mode.label().to_lowercase())).show_ui(ui, |ui| {
                for m in OnionMode::ALL {
                    ui.selectable_value(&mut s.mode, m, m.label()).on_hover_text(m.help());
                }
            });
            ui.checkbox(&mut s.front, "Devant").on_hover_text("Les images de la pelure par-dessus le dessin courant.");
            let mut linked = s.linked;
            if ui.checkbox(&mut linked, "Lier les curseurs").on_hover_text("Opacité en ligne droite de la plus proche à la plus lointaine : tirer un curseur les règle tous.").changed() {
                s.set_linked(linked);
            }
        });
        ui.separator();
        ui.columns(2, |cols| {
            side_ui(&mut cols[0], &mut self.onion, false);
            side_ui(&mut cols[1], &mut self.onion, true);
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Opacité globale");
            ui.add(egui::Slider::new(&mut self.onion.opacity, 0.05..=1.0).show_value(false));
            ui.label(RichText::new(format!("{:>3} %", (self.onion.opacity * 100.0).round())).monospace().weak());
        });
        ui.horizontal(|ui| {
            if ui.button("Activer tout").on_hover_text("Montrer toutes les images, avant et après.").clicked() {
                self.onion.show_all(true);
            }
            if ui.button("Désactiver tout").on_hover_text("Cacher toutes les images, avant et après.").clicked() {
                self.onion.show_all(false);
            }
            if ui.button("Réinitialiser").on_hover_text("3 avant en rouge, 3 après en vert, par dessin (tes préférences Storyboard Pro).").clicked() {
                let (enabled, bin) = (self.onion.enabled, std::mem::take(&mut self.onion.bin));
                self.onion = OnionSettings { enabled, bin, ..OnionSettings::default() };
            }
        });
        ui.separator();
        // Décalage temporaire.
        ui.horizontal(|ui| {
            let key = self.raccourci("pelure.decaler");
            crate::bulles::interrupteur_a(ui, &mut self.onion_shift, ph::ARROWS_OUT_CARDINAL, "Décaler la pelure", "Glisser sur le dessin déplace les images de la pelure, pas le dessin (rien n'est enregistré).", &key, "pelure.decaler");
            let moved = self.onion_offset != Vec2::ZERO;
            if ui.add_enabled(moved, egui::Button::new("Remettre")).on_hover_text("Les images de la pelure reviennent à leur place.").clicked() {
                self.reset_onion_offset();
            }
            if moved {
                ui.label(RichText::new(format!("{:+.0}, {:+.0} px", self.onion_offset.x, self.onion_offset.y)).weak());
            }
        });
        ui.separator();
        self.onion_bin_ui(ui);
    }

    /// Réglages rangés sous un nom (le « Bin »).
    fn onion_bin_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Réglages rangés").strong());
        let names: Vec<String> = self.onion.bin.iter().map(|p| p.name.clone()).collect();
        if names.is_empty() {
            ui.label(RichText::new("Aucun : range les réglages courants sous un nom.").weak());
        }
        for name in names {
            ui.horizontal(|ui| {
                if ui.button(&name).on_hover_text("Reprendre ces réglages").clicked() && self.onion.recall(&name) {
                    self.status = format!("Pelure : réglages « {name} » repris.");
                }
                let resp = ui.add(egui::Button::new(RichText::new(ph::TRASH).size(13.0)).frame(false));
                if bulle(resp, "Effacer", "Retirer ces réglages rangés.", "").clicked() {
                    self.onion.bin.retain(|p| p.name != name);
                }
            });
        }
        ui.horizontal(|ui| {
            if self.onion_preset_name.is_empty() {
                self.onion_preset_name = DEFAULT_PRESET.into();
            }
            ui.add(egui::TextEdit::singleline(&mut self.onion_preset_name).desired_width(140.0));
            if ui.button("Ranger").on_hover_text("Ranger les réglages courants sous ce nom (remplace un rangement du même nom).").clicked() {
                let name = self.onion_preset_name.trim().to_string();
                if !name.is_empty() {
                    self.onion.store(&name);
                    self.status = format!("Pelure : réglages rangés sous « {name} ».");
                }
            }
        });
    }

    /// Les images de la pelure reviennent à leur place.
    pub(crate) fn reset_onion_offset(&mut self) {
        self.onion_offset = Vec2::ZERO;
        self.status = "Pelure remise en place.".into();
    }

    /// Change le mode de la pelure (action du clavier).
    pub(crate) fn set_onion_mode(&mut self, mode: OnionMode) {
        self.onion.mode = mode;
        self.status = format!("Pelure d'oignon : mode {}.", mode.label().to_lowercase());
    }
}
