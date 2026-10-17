//! Panneau « Pile d'effets » (13i) du calque courant : ajouter, activer, ordonner, retirer des
//! effets ; régler leurs paramètres ; créer et supprimer des clés (TVPaint 000025314–316), avec
//! la création automatique des clés. Avec des clés, changer un paramètre pose (ou met à jour) la
//! clé de l'image courante ; sans clé, il change l'effet sur tout le calque.
//!
//! Les clés d'effets se voient aussi dans la timeline : de petits losanges en haut de la ligne
//! du calque.

use eframe::egui::{self, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::fx::{Effect, EffectKind, FAMILIES};

impl App {
    /// L'effet `kind` est-il là et actif sur le calque `li` ?
    pub(crate) fn fx_on(&self, li: usize, kind: EffectKind) -> bool {
        self.layers().get(li).is_some_and(|l| l.fx.iter().any(|e| e.kind == kind && e.enabled))
    }

    /// Coche ou décoche l'effet `kind` du calque `li` (une étape d'annulation, le dessin ne
    /// change pas) : absent, il s'ajoute avec ses réglages par défaut ; présent, il s'active ou
    /// se désactive, ses réglages restent.
    pub(crate) fn toggle_fx(&mut self, li: usize, kind: EffectKind) {
        if li >= self.layers().len() || self.content_refuses() {
            return;
        }
        let on = self.fx_on(li, kind);
        self.record_step();
        let fx = &mut self.layers_mut()[li].fx;
        match fx.iter().position(|e| e.kind == kind) {
            Some(_) if on => fx.iter_mut().filter(|e| e.kind == kind).for_each(|e| e.enabled = false),
            Some(i) => fx[i].enabled = true,
            None => fx.push(Effect::new(kind)),
        }
        if let Some(i) = self.layers()[li].fx.iter().position(|e| e.kind == kind) {
            self.fx_selected = i;
        }
        self.renderer.invalidate_all();
        self.status = format!("{} : {}.", kind.label(), if on { "décoché (le dessin n'a pas changé)" } else { "coché" });
    }

    /// Les effets par familles, en sous-menus de cases à cocher (menu d'un calque) ; on les
    /// règle dans le panneau Effets (Anthony, 2026-10-17 : « enlève Fenêtre Effets du clic
    /// droit »).
    pub(crate) fn fx_menu_items(&mut self, ui: &mut egui::Ui, li: usize) {
        for family in FAMILIES {
            ui.menu_button(family, |ui| {
                for kind in EffectKind::ALL.into_iter().filter(|k| k.family() == family) {
                    let on = self.fx_on(li, kind);
                    if ui.checkbox(&mut on.clone(), kind.label()).clicked() {
                        self.toggle_fx(li, kind);
                    }
                }
            });
        }

    }

    /// Pose ou met à jour la clé de l'effet `i` du calque courant à l'image courante, avec ses
    /// paramètres à cette image (une étape d'annulation).
    pub(crate) fn fx_create_key(&mut self) {
        let (li, f, i) = (self.cur_layer, self.cur_frame, self.fx_selected);
        let Some((values, name)) = self.layers().get(li).and_then(|l| l.fx.get(i)).map(|e| (e.values_at(f), e.kind.label())) else {
            self.status = "Choisis d'abord un effet dans la pile du calque.".into();
            return;
        };
        if self.content_refuses() {
            return;
        }
        self.record_step();
        self.layers_mut()[li].fx[i].set_key(f, values);
        self.status = format!("Clé de « {name} » à l'image {}.", self.frame_label(f));
    }

    /// Retire la clé de l'effet choisi à l'image courante.
    pub(crate) fn fx_delete_key(&mut self) {
        let (li, f, i) = (self.cur_layer, self.cur_frame, self.fx_selected);
        let has = self.layers().get(li).and_then(|l| l.fx.get(i)).is_some_and(|e| e.key_at(f).is_some());
        if !has {
            self.status = "Pas de clé de cet effet à l'image courante.".into();
            return;
        }
        if self.content_refuses() {
            return;
        }
        self.record_step();
        self.layers_mut()[li].fx[i].remove_key(f);
        self.status = "Clé supprimée.".into();
    }

    /// Va à la clé précédente (`dir` < 0) ou suivante de l'effet choisi.
    fn fx_goto_key(&mut self, dir: i32) {
        let Some(e) = self.layer().fx.get(self.fx_selected) else { return };
        let f = self.cur_frame;
        let to = if dir > 0 { e.keys.iter().map(|k| k.frame).find(|&k| k > f) } else { e.keys.iter().rev().map(|k| k.frame).find(|&k| k < f) };
        match to {
            Some(k) => self.goto_frame(k),
            None => self.status = if dir > 0 { "Pas de clé après." } else { "Pas de clé avant." }.into(),
        }
    }


    pub(crate) fn fx_panel(&mut self, ui: &mut egui::Ui) {
        let layer = self.layer();
        ui.label(RichText::new(format!("Calque « {} », image {}", layer.name, self.frame_label(self.cur_frame))).strong());
        if layer.folder {
            ui.label(RichText::new("Un dossier n'a pas d'effets : choisis un de ses calques.").weak());
            return;
        }
        let locked = self.doc.storyboard_locked;
        if locked {
            ui.label(RichText::new("Storyboard verrouillé : les effets ne changent pas.").weak());
        }
        ui.add_enabled_ui(!locked, |ui| self.fx_stack_ui(ui));
    }

    fn fx_stack_ui(&mut self, ui: &mut egui::Ui) {
        let li = self.cur_layer;
        let f = self.cur_frame;
        ui.horizontal(|ui| {
            ui.menu_button(format!("{}  Ajouter un effet", ph::PLUS), |ui| {
                // Par familles, comme le menu Filtre de Photoshop.
                for family in FAMILIES {
                    ui.menu_button(family, |ui| {
                        for k in EffectKind::ALL.into_iter().filter(|k| k.family() == family) {
                            if ui.button(k.label()).clicked() {
                                self.record_step();
                                self.layers_mut()[li].fx.push(Effect::new(k));
                                self.fx_selected = self.layers()[li].fx.len() - 1;
                                ui.close_menu();
                            }
                        }
                    });
                }
            });
            let any = !self.layers()[li].fx.is_empty();
            if ui.add_enabled(any, egui::Button::new("Tout réinitialiser")).on_hover_text("Chaque effet reprend ses réglages par défaut, sans clé.").clicked() {
                self.record_step();
                for e in &mut self.layers_mut()[li].fx {
                    *e = Effect { enabled: e.enabled, ..Effect::new(e.kind) };
                }
            }
            if ui.add_enabled(any, egui::Button::new("Tout supprimer")).clicked() {
                self.record_step();
                self.layers_mut()[li].fx.clear();
            }
        });
        let key = self.raccourci("fx.cle_auto");
        let resp = ui.checkbox(&mut self.fx_auto_key, "Création automatique des clés");
        crate::bulles::bulle(resp, "Création automatique des clés", "Une clé est posée à l'image courante à chaque changement d'un paramètre.", &key);
        ui.checkbox(&mut self.fx_keys_shown, "Clés d'effets dans la timeline");
        ui.separator();
        let n = self.layers()[li].fx.len();
        if n == 0 {
            ui.label(RichText::new("Aucun effet sur ce calque.").weak());
            return;
        }
        let mut remove = None;
        let mut swap = None;
        for i in 0..n {
            let e = self.layers()[li].fx[i].clone();
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let mut on = e.enabled;
                    if ui.checkbox(&mut on, "").on_hover_text("Effet actif").changed() {
                        self.record_step();
                        self.layers_mut()[li].fx[i].enabled = on;
                    }
                    if ui.selectable_label(self.fx_selected == i, RichText::new(e.kind.label()).strong()).on_hover_text("Effet choisi : les boutons de clé et les actions du clavier le visent.").clicked() {
                        self.fx_selected = i;
                    }
                    if ui.add_enabled(i > 0, egui::Button::new(ph::ARROW_UP).frame(false)).on_hover_text("Plus tôt dans la pile").clicked() {
                        swap = Some((i, i - 1));
                    }
                    if ui.add_enabled(i + 1 < n, egui::Button::new(ph::ARROW_DOWN).frame(false)).on_hover_text("Plus tard dans la pile").clicked() {
                        swap = Some((i, i + 1));
                    }
                    if ui.add(egui::Button::new(ph::TRASH).frame(false)).on_hover_text("Retirer l'effet").clicked() {
                        remove = Some(i);
                    }
                });
                let values = e.values_at(f);
                egui::Grid::new(("parametres_effet", i)).num_columns(2).show(ui, |ui| {
                    for (k, p) in e.kind.params().iter().enumerate() {
                        ui.label(p.name);
                        let mut v = values[k];
                        let resp = ui.add(egui::Slider::new(&mut v, p.min..=p.max).suffix(p.unit));
                        if self.edited(&resp) {
                            let mut new = values.clone();
                            new[k] = v;
                            let auto = self.fx_auto_key;
                            let effect = &mut self.layers_mut()[li].fx[i];
                            if auto || !effect.keys.is_empty() {
                                effect.set_key(f, new);
                            } else {
                                effect.values = new;
                            }
                            self.fx_selected = i;
                        }
                        ui.end_row();
                    }
                });
                ui.horizontal(|ui| {
                    let here = e.key_at(f).is_some();
                    let key = self.raccourci("fx.cle_creer");
                    let resp = ui.button(format!("{}  Créer une clé", ph::DIAMOND));
                    if crate::bulles::bulle(resp, "Créer une clé", "À l'image courante, avec les paramètres de l'effet.", &key).clicked() {
                        self.fx_selected = i;
                        self.fx_create_key();
                    }
                    let key = self.raccourci("fx.cle_supprimer");
                    let resp = ui.add_enabled(here, egui::Button::new("Supprimer la clé"));
                    if crate::bulles::bulle(resp, "Supprimer la clé", "La clé de cet effet à l'image courante.", &key).clicked() {
                        self.fx_selected = i;
                        self.fx_delete_key();
                    }
                    if !e.keys.is_empty() {
                        if ui.button(ph::CARET_LEFT).on_hover_text("Clé précédente").clicked() {
                            self.fx_selected = i;
                            self.fx_goto_key(-1);
                        }
                        if ui.button(ph::CARET_RIGHT).on_hover_text("Clé suivante").clicked() {
                            self.fx_selected = i;
                            self.fx_goto_key(1);
                        }
                        let s = if e.keys.len() > 1 { "s" } else { "" };
                        ui.label(RichText::new(format!("{} clé{s}", e.keys.len())).weak());
                    }
                });
            });
        }
        if let Some((a, b)) = swap {
            self.record_step();
            self.layers_mut()[li].fx.swap(a, b);
            if self.fx_selected == a {
                self.fx_selected = b;
            }
        }
        if let Some(i) = remove {
            self.record_step();
            self.layers_mut()[li].fx.remove(i);
            self.fx_selected = self.fx_selected.min(self.layers()[li].fx.len().saturating_sub(1));
        }
    }
}
